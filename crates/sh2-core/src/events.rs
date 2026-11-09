//! Stage Event Engine (`ev_prog` / `EventExecProgram`), Condition Flags (`16384`),
//! `FlagInit` Randomization, and Interactive Puzzle State Machines (`SH2-15`).
//!
//! Grounded in `sh23ds/port/source/prog.c`, `prog_town.c`, `prog_apart.c`, `prog_hospital.c`,
//! `prog_wish.c`, and `events.c`.

use crate::config::{ActionDifficulty, RiddleDifficulty};

mod puzzles;
mod runner;
pub use puzzles::*;
pub use runner::*;

/// Maximum number of event condition flags (`0..16384`, 2048 bytes at `0x1f7d5e0`).
pub const EV_MAX_FLAGS: usize = 16384;

/// Byte length of the event flag block (`16384 / 8 = 2048` bytes).
pub const EV_FLAG_BYTES: usize = EV_MAX_FLAGS / 8;

/// Puzzle state offset (`0x00`): Apartment W2F safe 4-position code (`0..19`).
pub const PZ_SAFE_CODE: usize = 0x00;
/// Puzzle state offset (`0x04`): Apartment W2F safe dial progress (`s8[4]`, `0xff` = not reached).
pub const PZ_SAFE_PROG: usize = 0x04;
/// Puzzle state offset (`0x08`): Hospital box 4 wheel goal symbols on the wall (`0..8`).
pub const PZ_WHEEL_GOAL: usize = 0x08;
/// Puzzle state offset (`0x0c`): Hospital box 4 wheel current symbols (`0..8`).
pub const PZ_WHEEL: usize = 0x0c;
/// Puzzle state offset (`0x10`): Apartment Room 208 clock time in `1/64` minutes (`0..0xb400`).
pub const PZ_CLOCK: usize = 0x10;
/// Puzzle state offset (`0x12`): Hospital box 4-digit keypad code (`u16`, 4 nibbles `1..9`).
pub const PZ_BOX_CODE: usize = 0x12;
/// Puzzle state offset (`0x14`): Delusion 3 panel lock (`u8[3]`, `0..8`).
pub const PZ_PANEL: usize = 0x14;
/// Puzzle state offset (`0x18`): Labyrinth hangman permutation (`u16`, `(v / 6^k) % 6`).
pub const PZ_HANGMAN: usize = 0x18;
/// Puzzle state offset (`0x1c`): Hotel briefcase 4 drum positions (`u8[4]`, `0..7`).
pub const PZ_DRUM: usize = 0x1c;

/// Input button bit: Confirm / Action (`KEY_A`).
pub const PROG_KEY_A: u32 = 1 << 0;
/// Input button bit: Cancel / Back (`KEY_B`).
pub const PROG_KEY_B: u32 = 1 << 1;
/// Input button bit: D-Pad Up (`KEY_DUP`).
pub const PROG_KEY_UP: u32 = 1 << 2;
/// Input button bit: D-Pad Down (`KEY_DDOWN`).
pub const PROG_KEY_DOWN: u32 = 1 << 3;
/// Input button bit: D-Pad Left (`KEY_DLEFT`).
pub const PROG_KEY_LEFT: u32 = 1 << 4;
/// Input button bit: D-Pad Right (`KEY_DRIGHT`).
pub const PROG_KEY_RIGHT: u32 = 1 << 5;

/// Converts [`RiddleDifficulty`] to its `0..=3` index (`Easy=0, Normal=1, Hard=2, Extra=3`).
pub const fn riddle_level_index(riddle: RiddleDifficulty) -> u8 {
    match riddle {
        RiddleDifficulty::Easy => 0,
        RiddleDifficulty::Normal => 1,
        RiddleDifficulty::Hard => 2,
        RiddleDifficulty::Extra => 3,
    }
}

/// Bitset of `16384` stage progression and event flags (`0x1f7d5e0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventFlags {
    bytes: [u8; EV_FLAG_BYTES],
}

impl Default for EventFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFlags {
    /// Creates a zeroed `16384`-flag bitset.
    pub const fn new() -> Self {
        Self {
            bytes: [0u8; EV_FLAG_BYTES],
        }
    }

    /// Resets all 16,384 flags to `false`.
    pub fn reset(&mut self) {
        self.bytes.fill(0);
    }

    /// Returns the state of flag `n` (`0..16384`).
    pub fn get(&self, n: usize) -> bool {
        if n >= EV_MAX_FLAGS {
            return false;
        }
        ((self.bytes[n >> 3] >> (n & 7)) & 1) != 0
    }

    /// Sets or clears flag `n` (`0..16384`).
    pub fn set(&mut self, n: usize, value: bool) {
        if n >= EV_MAX_FLAGS {
            return;
        }
        if value {
            self.bytes[n >> 3] |= 1 << (n & 7);
        } else {
            self.bytes[n >> 3] &= !(1 << (n & 7));
        }
    }

    /// Raw byte slice of the 2048-byte flag block.
    pub fn as_bytes(&self) -> &[u8; EV_FLAG_BYTES] {
        &self.bytes
    }

    /// Mutable byte slice of the 2048-byte flag block.
    pub fn as_bytes_mut(&mut self) -> &mut [u8; EV_FLAG_BYTES] {
        &mut self.bytes
    }
}

/// 32-byte puzzle state block (`s_puzzle` at `0x1f7da60..0x1f7da7f`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PuzzleState {
    bytes: [u8; 32],
}

impl PuzzleState {
    pub const fn new() -> Self {
        Self { bytes: [0u8; 32] }
    }

    pub fn reset(&mut self) {
        self.bytes = [0u8; 32];
    }

    pub fn raw(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn raw_mut(&mut self) -> &mut [u8; 32] {
        &mut self.bytes
    }

    pub fn read_u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.bytes[offset], self.bytes[offset + 1]])
    }

    pub fn write_u16(&mut self, offset: usize, val: u16) {
        let le = val.to_le_bytes();
        self.bytes[offset] = le[0];
        self.bytes[offset + 1] = le[1];
    }
}

/// Authentic Silent Hill 2 31-bit linear congruential generator (`0x448810` & `0x448830`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sh2Rng {
    seed: u32,
}

impl Default for Sh2Rng {
    fn default() -> Self {
        Self::new(1)
    }
}

impl Sh2Rng {
    pub const fn new(seed: u32) -> Self {
        Self {
            seed: seed & 0x7fffffff,
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Advances the LCG (`0x448810`) and returns a 31-bit non-negative integer.
    pub fn next_i32(&mut self) -> i32 {
        self.seed = (self.seed.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fffffff;
        self.seed as i32
    }

    /// Advances the LCG and scales to `[0.0, 1.0)` (`0x448830`).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_i32() as f32) * (1.0f32 / 2_147_483_648.0f32)
    }
}

/// Clear-data persistence across playthroughs (`prog_clear_get` / `prog_clear_set`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClearData {
    /// Cleared action/riddle difficulty bitmask:
    /// - Bits `0..=3`: Riddle difficulty clears (`Easy = 0x01`, `Normal = 0x02`, `Hard = 0x04`, `Extra = 0x08`).
    /// - Bits `5..=7`: Action difficulty clears (`Easy = 0x20`, `Normal = 0x40`, `Hard = 0x80`).
    pub cleared_levels: u8,
    /// Seen endings bitmask (`0x01` Leave, `0x02` In Water, `0x04` Maria, `0x08` Rebirth,
    /// `0x10` UFO, `0x20` Dog, `0x40` Born from a Wish).
    pub endings_seen: u8,
    /// Total main-scenario clears (`0..=99`).
    pub clear_count: u8,
}

impl ClearData {
    /// Riddle difficulty clear bit: Easy (`1 << 0`).
    pub const RIDDLE_EASY_CLEARED: u8 = 1 << 0;
    /// Riddle difficulty clear bit: Normal (`1 << 1`).
    pub const RIDDLE_NORMAL_CLEARED: u8 = 1 << 1;
    /// Riddle difficulty clear bit: Hard (`1 << 2`).
    pub const RIDDLE_HARD_CLEARED: u8 = 1 << 2;
    /// Riddle difficulty clear bit: Extra (`1 << 3`).
    pub const RIDDLE_EXTRA_CLEARED: u8 = 1 << 3;
    /// Combined mask of Easy, Normal, and Hard riddle difficulty clears (`0x07`).
    pub const ALL_BASE_RIDDLES_CLEARED: u8 =
        Self::RIDDLE_EASY_CLEARED | Self::RIDDLE_NORMAL_CLEARED | Self::RIDDLE_HARD_CLEARED;

    /// Action difficulty clear bit: Easy (`0x20`).
    pub const ACTION_EASY_CLEARED: u8 = 0x20;
    /// Action difficulty clear bit: Normal (`0x40`).
    pub const ACTION_NORMAL_CLEARED: u8 = 0x40;
    /// Action difficulty clear bit: Hard (`0x80`).
    pub const ACTION_HARD_CLEARED: u8 = 0x80;

    /// Seen ending bit: Leave (`0x01`).
    pub const ENDING_LEAVE: u8 = 0x01;
    /// Seen ending bit: In Water (`0x02`).
    pub const ENDING_IN_WATER: u8 = 0x02;
    /// Seen ending bit: Maria (`0x04`).
    pub const ENDING_MARIA: u8 = 0x04;
    /// Seen ending bit: Rebirth (`0x08`).
    pub const ENDING_REBIRTH: u8 = 0x08;
    /// Seen ending bit: UFO (`0x10`).
    pub const ENDING_UFO: u8 = 0x10;
    /// Seen ending bit: Dog (`0x20`).
    pub const ENDING_DOG: u8 = 0x20;
    /// Seen ending bit: *Born from a Wish* (`0x40`).
    pub const ENDING_BORN_FROM_A_WISH: u8 = 0x40;

    /// Returns `true` if selecting `RiddleDifficulty::Hard` is promoted to
    /// `RiddleDifficulty::Extra` (`riddle 3`) during `FlagInit` (`0x48aed0`).
    ///
    /// Per `0x48aed0`, Extra Riddle is unlocked once Easy, Normal, and Hard riddle levels have
    /// all been cleared (`(self.cleared_levels & Self::ALL_BASE_RIDDLES_CLEARED) == Self::ALL_BASE_RIDDLES_CLEARED`,
    /// recorded by [`Self::record_ending`]), and alternates between Extra and Hard on subsequent
    /// playthroughs once Extra itself has been cleared (`(self.cleared_levels & Self::RIDDLE_EXTRA_CLEARED) == 0 || (self.clear_count & 1) == 0`).
    pub fn is_extra_riddle_unlocked(&self) -> bool {
        (self.cleared_levels & Self::ALL_BASE_RIDDLES_CLEARED) == Self::ALL_BASE_RIDDLES_CLEARED
            && ((self.cleared_levels & Self::RIDDLE_EXTRA_CLEARED) == 0
                || (self.clear_count & 1) == 0)
    }

    /// Records a completed ending into the clear data (`0x48d7e0`).
    pub fn record_ending(
        &mut self,
        ending_bit: u8,
        riddle: RiddleDifficulty,
        action: ActionDifficulty,
    ) {
        self.endings_seen |= ending_bit;
        if ending_bit == Self::ENDING_BORN_FROM_A_WISH {
            return;
        }
        let action_bit = match action {
            ActionDifficulty::Easy => Self::ACTION_EASY_CLEARED,
            ActionDifficulty::Normal => Self::ACTION_NORMAL_CLEARED,
            ActionDifficulty::Hard => Self::ACTION_HARD_CLEARED,
        };
        let riddle_idx = riddle_level_index(riddle);
        self.cleared_levels |= action_bit;
        self.cleared_levels |= 1 << riddle_idx;
        self.clear_count = (self.clear_count + 1).min(99);
    }
}

/// Dynamic string insertion slots (`<e0>..<e3>`, `ev_msg_set_insert` / `ev_msg_expand`).
#[derive(Debug, Clone, Default)]
pub struct MessageInsertState {
    slots: [String; 4],
}

impl MessageInsertState {
    pub fn set(&mut self, index: usize, value: impl Into<String>) {
        if index < 4 {
            self.slots[index] = value.into();
        }
    }

    pub fn get(&self, index: usize) -> &str {
        self.slots.get(index).map(|s| s.as_str()).unwrap_or("")
    }

    /// Replaces `<e0>..<e3>` placeholders in `raw` with the current insertion strings.
    pub fn expand(&self, raw: &str) -> String {
        let mut out = raw.to_string();
        for i in 0..4 {
            let tag = format!("<e{i}>");
            if out.contains(&tag) {
                out = out.replace(&tag, &self.slots[i]);
            }
        }
        out
    }
}

/// Initializes flags and randomized puzzle state for a new *Letter from Silent Heaven* (James)
/// playthrough (`FlagInit` at `0x48aed0`).
pub fn init_new_game(
    flags: &mut EventFlags,
    puzzle: &mut PuzzleState,
    rng: &mut Sh2Rng,
    riddle: RiddleDifficulty,
    action: ActionDifficulty,
    clear: ClearData,
) -> RiddleDifficulty {
    flags.reset();
    puzzle.reset();

    let effective_riddle = if riddle == RiddleDifficulty::Hard && clear.is_extra_riddle_unlocked() {
        RiddleDifficulty::Extra
    } else {
        riddle
    };

    let endings = clear.endings_seen;
    let cleared = clear.cleared_levels;
    let clears = clear.clear_count;
    let n = (endings & 1) + ((endings >> 1) & 1) + ((endings >> 2) & 1);
    if n == 1 {
        if (endings & 1) != 0 {
            flags.set(if (rng.next_i32() & 1) != 0 { 8 } else { 9 }, true);
        }
        if (endings & 2) != 0 {
            flags.set(if (rng.next_i32() & 1) != 0 { 9 } else { 7 }, true);
        }
        if (endings & 4) != 0 {
            if (rng.next_i32() & 1) != 0 {
                flags.set(7, true);
            }
            flags.set(8, true);
        }
    } else if n == 2 {
        if (endings & 1) == 0 {
            flags.set(7, true);
        }
        if (endings & 2) == 0 {
            flags.set(8, true);
        }
        if (endings & 4) == 0 {
            flags.set(9, true);
        }
    }

    if n > 0 {
        flags.set(10, true);
        flags.set(12, true); // Chainsaw at the grave (New Game+)
    }
    if n > 0 && (endings & 0x40) != 0 {
        flags.set(723, true);
    }
    if n == 3 || (endings & 8) != 0 {
        flags.set(11, true);
    }
    if (cleared & 0x80) != 0 || ((cleared & 0x40) != 0 && clears >= 2) || clears >= 3 {
        flags.set(631, true);
    }

    // Room 208 clock: random time outside 8:40..9:40 (answer is 9:10 = 550 minutes = 0x8980)
    let mut t = (rng.next_i32() % 660) as u16;
    if t > 520 {
        t += 60;
    }
    puzzle.write_u16(PZ_CLOCK, t << 6);

    // Apartment W2F safe: 4 dial positions 0..19, never adjacent duplicates
    for i in 0..4usize {
        let r = (rng.next_i32() % 20) as u8;
        puzzle.raw_mut()[PZ_SAFE_CODE + i] = r;
        if i > 0 && puzzle.raw()[PZ_SAFE_CODE + i - 1] == r {
            puzzle.raw_mut()[PZ_SAFE_CODE + i] = (r + 10) % 20;
        }
    }
    let dial = ((rng.next_i32() % 19) as u8 + puzzle.raw()[PZ_SAFE_CODE]) % 20;
    for b in 0..5usize {
        flags.set(103 + b, ((dial >> b) & 1) != 0);
    }
    puzzle.raw_mut()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4].fill(0xff);

    // Hospital box 4-digit keypad code (PZ_BOX_CODE) & 4 symbol wheels (PZ_WHEEL_GOAL, PZ_WHEEL)
    let mut v72: u16 = 0;
    for i in 0..4usize {
        v72 = ((rng.next_i32() % 9 + 1) as u16).wrapping_add(v72.wrapping_mul(16));
        let goal = (rng.next_i32() % 9) as u8;
        puzzle.raw_mut()[PZ_WHEEL_GOAL + i] = goal;
        puzzle.raw_mut()[PZ_WHEEL + i] = (((rng.next_i32() & 7) as u8) + 1 + goal) % 9;
    }
    puzzle.write_u16(PZ_BOX_CODE, v72);
    if effective_riddle == RiddleDifficulty::Easy {
        if puzzle.raw()[PZ_WHEEL_GOAL] == puzzle.raw()[PZ_WHEEL_GOAL + 1] {
            let mut next = puzzle.raw()[PZ_WHEEL_GOAL + 1] + 1;
            if next == 9 {
                next = 0;
            }
            puzzle.raw_mut()[PZ_WHEEL_GOAL + 1] = next;
        }
        puzzle.raw_mut()[PZ_WHEEL_GOAL + 2] = puzzle.raw()[PZ_WHEEL_GOAL];
        puzzle.raw_mut()[PZ_WHEEL_GOAL + 3] = puzzle.raw()[PZ_WHEEL_GOAL + 1];
    }

    // Delusion 3 panel lock (PZ_PANEL: 3 distinct panels 0..8)
    let a = (rng.next_i32() % 9) as u8;
    let b = (rng.next_i32() & 7) as u8;
    let c = (rng.next_i32() % 7) as u8;
    let b_adj = b + u8::from(a <= b);
    let c_adj = c + u8::from(a <= c) + u8::from(b_adj <= c);
    puzzle.raw_mut()[PZ_PANEL] = a;
    puzzle.raw_mut()[PZ_PANEL + 1] = b_adj;
    puzzle.raw_mut()[PZ_PANEL + 2] = c_adj;

    // Labyrinth hangman permutation (PZ_HANGMAN)
    let mut perm = [0u16, 1, 2, 3, 4, 5];
    let mut v78: u16 = 0;
    for i in 0..6usize {
        let left = (6 - i) as i32;
        let k = i + (rng.next_i32() % left) as usize;
        v78 = v78 * 6 + perm[k];
        perm[k] = perm[i];
    }
    puzzle.write_u16(PZ_HANGMAN, v78);

    // Prison & Hotel randomization flags
    let mut u = (rng.next_i32() % 3 + 1) as u8;
    if riddle_level_index(effective_riddle) < 2 {
        u = 1;
    }
    if (u & 1) != 0 {
        flags.set(319, true);
        flags.set(323, true);
    }
    if (u & 2) != 0 {
        flags.set(320, true);
        flags.set(324, true);
    }
    u = (rng.next_i32() % 3 + 1) as u8;
    if riddle_level_index(effective_riddle) < 2 {
        u = 2;
    }
    if (u & 1) != 0 {
        flags.set(317, true);
        flags.set(321, true);
    }
    if (u & 2) != 0 {
        flags.set(318, true);
        flags.set(322, true);
    }

    // Hotel briefcase 4-letter word index (0..18 in flags 406..410)
    let word = (rng.next_i32() % 19) as usize;
    for i in 0..5usize {
        if ((word >> i) & 1) != 0 {
            flags.set(406 + i, true);
        }
    }

    if effective_riddle == RiddleDifficulty::Easy {
        flags.set(13, true);
    } else if effective_riddle == RiddleDifficulty::Normal {
        flags.set(617, true);
    }

    if (rng.next_i32() & 3) == 0 {
        flags.set(543, true);
    }
    if (rng.next_i32() & 1) != 0 {
        flags.set(547, true);
    }
    if rng.next_f32() < 0.7 {
        flags.set(551, true);
    }

    let _ = action;
    effective_riddle
}

/// Initializes flags and puzzle state for a new *Born from a Wish* (`Maria` sub-scenario) game.
pub fn init_new_game_wish(flags: &mut EventFlags, puzzle: &mut PuzzleState) {
    flags.reset();
    puzzle.reset();
    // FlagInit: gChapterID == 1 sets flags 5 and 24 (0x1000020)
    flags.set(5, true);
    flags.set(24, true);
}
