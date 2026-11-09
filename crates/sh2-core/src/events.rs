//! Stage Event Engine (`ev_prog` / `EventExecProgram`), Condition Flags (`16384`),
//! `FlagInit` Randomization, and Interactive Puzzle State Machines (`SH2-15`).
//!
//! Grounded in `sh23ds/port/source/prog.c`, `prog_town.c`, `prog_apart.c`, `prog_hospital.c`,
//! `prog_wish.c`, and `events.c`.

use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::cutaway::CutawayOverlayEngine;
use crate::inventory::InventoryManager;

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
    /// Cleared action/riddle difficulty bitmask.
    pub cleared_levels: u8,
    /// Seen endings bitmask (`0x01` Leave, `0x02` In Water, `0x04` Maria, `0x08` Rebirth,
    /// `0x10` UFO, `0x20` Dog, `0x40` Born from a Wish).
    pub endings_seen: u8,
    /// Total main-scenario clears (`0..=99`).
    pub clear_count: u8,
}

impl ClearData {
    /// Returns `true` if Hard riddle difficulty is replaced by Extra riddle difficulty (`riddle 3`)
    /// (`0x48aed0`: once Leave, In Water, and Maria have all been seen, unless the last clear was on Extra).
    pub fn is_extra_riddle_unlocked(&self) -> bool {
        (self.cleared_levels & 7) == 7
            && ((self.cleared_levels & 8) == 0 || (self.clear_count & 1) == 0)
    }

    /// Records a completed ending into the clear data (`0x48d7e0`).
    pub fn record_ending(
        &mut self,
        ending_bit: u8,
        riddle: RiddleDifficulty,
        action: ActionDifficulty,
    ) {
        self.endings_seen |= ending_bit;
        if ending_bit == 0x40 {
            return;
        }
        let action_bit = match action {
            ActionDifficulty::Easy => 0x20,
            ActionDifficulty::Normal => 0x40,
            ActionDifficulty::Hard => 0x80,
        };
        let riddle_idx = match riddle {
            RiddleDifficulty::Easy => 0u8,
            RiddleDifficulty::Normal => 1u8,
            RiddleDifficulty::Hard => 2u8,
            RiddleDifficulty::Extra => 3u8,
        };
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

/// Formats the 4 safe dial numbers into `inserts` (`<e0>..<e3>`) according to the active riddle
/// difficulty (`prog_wallet_insert` at `0x595730`).
pub fn format_wallet_safe_code(
    puzzle: &PuzzleState,
    riddle: RiddleDifficulty,
    inserts: &mut MessageInsertState,
) {
    const ROMAN: [&str; 9] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX"];
    for i in 0..4usize {
        let v = (puzzle.raw()[PZ_SAFE_CODE + i] as usize) + 1;
        let formatted = match riddle {
            RiddleDifficulty::Easy | RiddleDifficulty::Normal => format!("{v:02}"),
            RiddleDifficulty::Hard => {
                // 0x595730: 10 is X, 20 is VV, 11..19 are X + I..IX
                if v == 10 {
                    "X".to_string()
                } else if v == 20 {
                    "VV".to_string()
                } else if v > 10 {
                    format!("X{}", ROMAN[(v - 11).min(8)])
                } else {
                    ROMAN[(v - 1).min(8)].to_string()
                }
            }
            RiddleDifficulty::Extra => {
                if v < 10 {
                    format!("{}", (b'0' + v as u8) as char)
                } else {
                    format!("{}", (b'a' + (v as u8 - 10)) as char)
                }
            }
        };
        inserts.set(i, formatted);
    }
}

/// Formats the Hospital 4-digit carbon paper code (`PZ_BOX_CODE`) into `<e0>` (`prog_memo_text`).
pub fn format_carbon_box_code(puzzle: &PuzzleState, inserts: &mut MessageInsertState) -> String {
    let v = puzzle.read_u16(PZ_BOX_CODE);
    let s = format!(
        "{}{}{}{}",
        (v >> 12) & 0x0f,
        (v >> 8) & 0x0f,
        (v >> 4) & 0x0f,
        v & 0x0f
    );
    inserts.set(0, s.clone());
    s
}

// ============================================================================
// Interactive Puzzle State Machines (Apartments, Hospital, Born from a Wish)
// ============================================================================

/// Validates and advances the Apartment Room 208 Grandfather Clock hands (`clock_needles` / `0x59a690`).
///
/// Returns `true` when the hands are set inside the `9:10` target window (`0x88c1..=0x8a3f`).
pub fn is_clock_solved(puzzle: &PuzzleState) -> bool {
    let t = puzzle.read_u16(PZ_CLOCK);
    (0x88c1..=0x8a3f).contains(&t)
}

/// Steps the Apartment Room 208 clock hands by input (`held` D-Pad Left/Right), returning any
/// tick sound effect (`0x2b0a` or `0x2b0b`) triggered during the step.
pub fn step_clock_needles(
    puzzle: &mut PuzzleState,
    held_keys: u32,
    dt: f32,
    accel_timer: &mut f32,
) -> Option<u32> {
    let before = puzzle.read_u16(PZ_CLOCK);
    let mut t = before as i32;
    if (held_keys & (PROG_KEY_LEFT | PROG_KEY_RIGHT)) != 0 {
        *accel_timer += dt;
        let step = if *accel_timer >= 2.0 {
            64
        } else if *accel_timer >= 0.5 {
            8
        } else {
            1
        };
        let d = if (held_keys & PROG_KEY_LEFT) != 0 {
            step
        } else {
            -step
        };
        t = (t + d + 0xb400) % 0xb400;
    } else {
        *accel_timer = 0.0;
    }
    let after = t as u16;
    puzzle.write_u16(PZ_CLOCK, after);

    if (before >> 4) != (after >> 4) {
        Some(if ((after >> 4) & 1) != 0 {
            0x2b0a
        } else {
            0x2b0b
        })
    } else {
        None
    }
}

/// Evaluates whether the Apartment Room 105 five-slot coin desk matches the active riddle
/// difficulty's solution (`s10_set_three_coin` in `prog_apart.c`).
///
/// Coin slots are 5 holes (`0..5`), each holding `0` = Snake (`50`), `1` = Old Man (`51`),
/// or `2` = Prisoner (`52`), stored in `COIN_FLAG(c, h) = 126 + 5 * c + h`.
pub fn is_coin_desk_solved(flags: &EventFlags, riddle: RiddleDifficulty) -> bool {
    const SOLUTIONS: [[usize; 3]; 4] = [
        [128, 131, 140], // Easy:   Hole 0 Old Man (131), Hole 2 Snake (128), Hole 4 Prisoner (140)
        [130, 132, 138], // Normal: Hole 1 Old Man (132), Hole 2 Prisoner (138), Hole 4 Snake (130)
        [129, 132, 140], // Hard:   Hole 1 Old Man (132), Hole 3 Snake (129), Hole 4 Prisoner (140)
        [128, 131, 139], // Extra:  Hole 0 Old Man (131), Hole 2 Snake (128), Hole 3 Prisoner (139)
    ];
    let r = (riddle_level_index(riddle) as usize).min(3);
    SOLUTIONS[r].iter().all(|&f| flags.get(f))
}

/// Places a coin (`50` = Snake, `51` = Old Man, `52` = Prisoner) into `slot` (`0..5`) of the
/// Apartment Room 105 coin desk, consuming the coin from `inventory` and setting the corresponding
/// desk flag (`COIN_FLAG(c, slot) = 126 + 5 * c + slot`, plus flag `141` indicating any coin is in
/// the desk). If all 3 coins are placed, resolves the puzzle: on success, sets flag `142` (and `112`)
/// and awards `Lyne house key` (`31`); on failure, returns the 3 coins to `inventory` and clears
/// flags `126..=141`.
pub fn place_coin_in_desk(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    riddle: RiddleDifficulty,
    slot: usize,
    coin_item_id: u8,
) -> Option<bool> {
    if slot >= 5 {
        return None;
    }
    let kind = match coin_item_id {
        50 => 0usize, // Snake
        51 => 1usize, // Old Man
        52 => 2usize, // Prisoner
        _ => return None,
    };
    // Ensure the target hole does not already hold a coin
    if (0..3).any(|c| flags.get(126 + 5 * c + slot)) {
        return None;
    }
    if !inventory.has_item(coin_item_id) {
        return None;
    }

    inventory.item_lose(coin_item_id);
    flags.set(126 + 5 * kind + slot, true);
    flags.set(141, true);

    // Count total coins placed across the 5 holes (flags 126..141)
    let placed_count = (126..141).filter(|&f| flags.get(f)).count();
    if placed_count < 3 {
        return Some(false);
    }

    if is_coin_desk_solved(flags, riddle) {
        flags.set(142, true);
        flags.set(112, true);
        inventory.item_get_now(31); // Lyne house key
        Some(true)
    } else {
        for f in 126..=141 {
            flags.set(f, false);
        }
        inventory.item_get_now(50);
        inventory.item_get_now(51);
        inventory.item_get_now(52);
        Some(false)
    }
}

/// Rotates the Apartment W2F safe dial (`safe_step` / `0x5971a0`) by one notch (`turn_right = true`
/// turns right / decreases `pos`, `turn_right = false` turns left / increases `pos`), updating
/// flags `103..=107`, tracking the 4-step combination in `PZ_SAFE_PROG`, and setting flag `108`
/// when the fourth number is reached.
pub fn step_safe_dial(flags: &mut EventFlags, puzzle: &mut PuzzleState, turn_right: bool) -> u8 {
    let mut v = 0i32;
    for b in 0..5usize {
        if flags.get(103 + b) {
            v |= 1 << b;
        }
    }

    let code = [
        puzzle.raw()[PZ_SAFE_CODE] as i32,
        puzzle.raw()[PZ_SAFE_CODE + 1] as i32,
        puzzle.raw()[PZ_SAFE_CODE + 2] as i32,
        puzzle.raw()[PZ_SAFE_CODE + 3] as i32,
    ];
    let mut prog = [
        puzzle.raw()[PZ_SAFE_PROG] as i8,
        puzzle.raw()[PZ_SAFE_PROG + 1] as i8,
        puzzle.raw()[PZ_SAFE_PROG + 2] as i8,
        puzzle.raw()[PZ_SAFE_PROG + 3] as i8,
    ];

    if turn_right {
        v = if v != 0 { v - 1 } else { 19 };
        if prog[2] >= 0 {
            if v == code[3] {
                prog[3] = v as i8;
                flags.set(108, true);
            }
        } else if prog[1] >= 0 {
            prog = [-1, -1, -1, -1];
        } else if prog[0] >= 0 {
            if v == code[1] {
                prog[1] = v as i8;
            }
        } else if v == code[0] {
            prog[0] = v as i8;
        }
    } else {
        v = if v == 19 { 0 } else { v + 1 };
        if prog[2] >= 0 {
            prog = [-1, -1, -1, -1];
        } else if prog[1] >= 0 {
            if v == code[2] {
                prog[2] = v as i8;
            }
        } else if prog[0] >= 0 {
            prog = [-1, -1, -1, -1];
        } else if v == code[0] {
            prog[0] = v as i8;
        }
    }

    for b in 0..5usize {
        flags.set(103 + b, ((v >> b) & 1) != 0);
    }
    for (i, &p_val) in prog.iter().enumerate() {
        puzzle.raw_mut()[PZ_SAFE_PROG + i] = p_val as u8;
    }
    v as u8
}

/// Evaluates whether the Apartment W2F safe dial combination is satisfied (`s11_open_safe`).
/// If satisfied, unlocks flag `108` and grants 4 boxes of Handgun bullets (`5`).
pub fn try_open_safe(
    flags: &mut EventFlags,
    puzzle: &PuzzleState,
    inventory: &mut InventoryManager,
) -> bool {
    let solved = flags.get(108)
        || (0..4usize)
            .all(|i| puzzle.raw()[PZ_SAFE_PROG + i] == puzzle.raw()[PZ_SAFE_CODE + i]);
    if solved {
        flags.set(108, true);
        for _ in 0..4 {
            inventory.item_get_now(5);
        }
        true
    } else {
        false
    }
}

/// Validates the Hospital 3F corridor keypad code (`s20_pass_number_t`):
/// - Easy / Normal: `7335`
/// - Hard / Extra: `1328`
///
/// Sets flag `181` on success.
pub fn verify_hospital_corridor_keypad(
    flags: &mut EventFlags,
    riddle: RiddleDifficulty,
    entered_code: u16,
) -> bool {
    let expected = match riddle {
        RiddleDifficulty::Easy | RiddleDifficulty::Normal => 7335,
        RiddleDifficulty::Hard | RiddleDifficulty::Extra => 1328,
    };
    if entered_code == expected {
        flags.set(181, true);
        true
    } else {
        false
    }
}

/// Validates the Hospital 2F patient room box 4-digit push-button lock (`s20_box_with_key`):
/// compares `entered_digits` (`[1..=9; 4]`) against `PZ_BOX_CODE`, setting flag `198` on match.
pub fn verify_hospital_box_keypad(
    flags: &mut EventFlags,
    puzzle: &PuzzleState,
    entered_digits: [u8; 4],
) -> bool {
    let v = puzzle.read_u16(PZ_BOX_CODE);
    let expected = [
        ((v >> 12) & 0x0f) as u8,
        ((v >> 8) & 0x0f) as u8,
        ((v >> 4) & 0x0f) as u8,
        (v & 0x0f) as u8,
    ];
    if entered_digits == expected {
        flags.set(198, true);
        true
    } else {
        false
    }
}

/// Evaluates the Hospital 2F patient room box 4-wheel symbol lock (`s20_box_with_key` at `0x58edf0`).
///
/// Requires flag `203` (having read the blood symbols on the wall in `s20_guruguru_number`);
/// if `PZ_WHEEL == PZ_WHEEL_GOAL` and flag `203` is not set, resets one matching wheel so the
/// lock cannot be brute-forced before reading the wall.
pub fn verify_hospital_box_wheels(
    flags: &mut EventFlags,
    puzzle: &mut PuzzleState,
    rng: &mut Sh2Rng,
) -> bool {
    let matches_goal = (0..4usize)
        .all(|i| puzzle.raw()[PZ_WHEEL + i] == puzzle.raw()[PZ_WHEEL_GOAL + i]);
    if matches_goal && flags.get(203) {
        flags.set(197, true);
        return true;
    }
    if matches_goal && !flags.get(203) {
        let idx = (rng.next_i32() & 3) as usize;
        puzzle.raw_mut()[PZ_WHEEL + idx] = (puzzle.raw()[PZ_WHEEL + idx] + 1) % 9;
    }
    false
}

/// Uses a key (`35` = Purple Bull, `36` = Lapis Eye, `86` = Combined) on the Hospital 2F locked
/// box, unlocking the corresponding chain flags (`192..=195`). When all 4 locks (`193, 195, 197, 198`)
/// are open, sets flag `199` and grants `Piece of hair` (`53`).
pub fn use_key_on_hospital_box(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    item_id: u8,
) -> bool {
    match item_id {
        35 => {
            inventory.item_lose(35);
            flags.set(192, true);
            flags.set(193, true);
        }
        36 => {
            inventory.item_lose(36);
            flags.set(194, true);
            flags.set(195, true);
        }
        86 => {
            inventory.item_lose(86);
            flags.set(192, true);
            flags.set(193, true);
            flags.set(194, true);
            flags.set(195, true);
        }
        _ => return false,
    }
    try_complete_hospital_box(flags, inventory)
}

/// Checks if all 4 locks on the Hospital 2F box (`193`, `195`, `197`, `198`) are unlocked; if so,
/// sets flag `199` and awards `Piece of hair` (`53`).
pub fn try_complete_hospital_box(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if flags.get(193) && flags.get(195) && flags.get(197) && flags.get(198) {
        if !flags.get(199) {
            flags.set(199, true);
            inventory.item_get_now(53); // Piece of hair
        }
        true
    } else {
        false
    }
}

/// Evaluates the *Born from a Wish* Baldwin Mansion garden tombstone board puzzle
/// (`s55_board_puzzle` at `0x5722b0`).
///
/// Board `b` (`0` = White `79`, `1` = Black `80`, `2` = Red `81`) is placed when flag `691 + b` is
/// set, with 2-bit rotation `0..=3` stored in flags `694 + b*2` (bit 0) and `695 + b*2` (bit 1).
/// Solved when all 3 boards are placed and `(rot_white & 1) == 0 && rot_black == 0 && rot_red == 1`.
/// On solve, clears `691..=699`, sets flag `704`, and grants `Acacia key` (`82`).
pub fn evaluate_baldwin_tombstone_boards(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if !(flags.get(691) && flags.get(692) && flags.get(693)) {
        return false;
    }
    let rot = |b: usize| -> u8 {
        (if flags.get(694 + b * 2) { 1 } else { 0 })
            | (if flags.get(695 + b * 2) { 2 } else { 0 })
    };
    let solved = (rot(0) & 1) == 0 && rot(1) == 0 && rot(2) == 1;
    if solved {
        for f in 691..=699 {
            flags.set(f, false);
        }
        flags.set(704, true);
        inventory.item_get_now(82); // Acacia key
        true
    } else {
        // Return boards to inventory and clear board flags
        for f in 691..=699 {
            flags.set(f, false);
        }
        inventory.item_get_now(79);
        inventory.item_get_now(80);
        inventory.item_get_now(81);
        false
    }
}

/// Places a board (`79` = White, `80` = Black, `81` = Red) onto the Baldwin Mansion garden
/// tombstone with `rotation` (`0..=3`), consuming the board from `inventory` and setting flags
/// `691 + b` and `694 + b*2 ..= 695 + b*2`.
pub fn place_baldwin_board(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    board_item_id: u8,
    rotation: u8,
) -> bool {
    let b = match board_item_id {
        79 => 0usize,
        80 => 1usize,
        81 => 2usize,
        _ => return false,
    };
    if flags.get(691 + b) || !inventory.has_item(board_item_id) {
        return false;
    }
    inventory.item_lose(board_item_id);
    flags.set(691 + b, true);
    let r = rotation & 3;
    flags.set(694 + b * 2, (r & 1) != 0);
    flags.set(695 + b * 2, (r & 2) != 0);
    true
}

// ============================================================================
// Step-based Stage Program Runner (`ev_prog` / `EventExecProgram`)
// ============================================================================

/// Execution context for a running stage program (`Prog` in `prog_int.h`).
#[derive(Debug, Clone)]
pub struct ProgContext {
    /// Stage number (`2..=58`).
    pub stage: u8,
    /// Program index in the stage's `ev_prog` table (`1..`).
    pub prog_no: u8,
    /// Completion flag (`rslt1 & 0x3fff`) set when the program finishes with `flag_set == true`.
    pub completion_flag: u16,
    /// `ev_prog_flag_set` (`0x1f7d554`): cleared by inspection programs that should not set `completion_flag`.
    pub flag_set: bool,
    /// `JmsEventMode(jms, 1)`: whether player movement/control is locked by the running program.
    pub locked: bool,
    /// Primary coroutine step (`ev_p_step` at `0x1f7d548`).
    pub step: i32,
    /// Sub-step within the current operation (`ev_s_step` at `0x1f7d544`).
    pub sub: i32,
    /// Elapsed timer in seconds (`0x1f7d564`).
    pub timer: f32,
    /// Last question/choice answer (`0` = Yes / first choice).
    pub answer: i32,
    /// Per-program scratch registers (`work[4]`).
    pub work: [i32; 4],
}

impl ProgContext {
    pub fn new(stage: u8, prog_no: u8, completion_flag: u16) -> Self {
        Self {
            stage,
            prog_no,
            completion_flag,
            flag_set: true,
            locked: false,
            step: 0,
            sub: 0,
            timer: 0.0,
            answer: 0,
            work: [0; 4],
        }
    }
}

/// Step-based stage program runner (`EventExecProgram`) coordinating `ProgContext`, `EventFlags`,
/// `PuzzleState`, `InventoryManager`, and `CutawayOverlayEngine`.
#[derive(Debug, Clone, Default)]
pub struct StageProgramRunner {
    /// Currently running stage program context, if any.
    active: Option<ProgContext>,
    /// Currently displayed stage message ID (`-1` if none).
    active_message_id: Option<i32>,
    /// Sound effects queued during the current frame (`p_se`).
    sound_effects: Vec<u32>,
    /// Cutscene demo ID requested by `p_demo`, if any.
    requested_demo: Option<u32>,
    /// Sub-scenario / main game ending bit triggered by `p_game_end`, if any.
    triggered_ending: Option<u8>,
}

impl StageProgramRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts stage program `(stage, prog_no)` with `completion_flag` (`rslt1 & 0x3fff`).
    pub fn start(&mut self, stage: u8, prog_no: u8, completion_flag: u16) {
        self.active = Some(ProgContext::new(stage, prog_no, completion_flag));
        self.active_message_id = None;
        self.sound_effects.clear();
        self.requested_demo = None;
    }

    /// Aborts any running stage program and hides any active cutaway still.
    pub fn abort(&mut self, overlay: &mut CutawayOverlayEngine, flags: &EventFlags) {
        self.active = None;
        self.active_message_id = None;
        overlay.set_picture(None, flags);
    }

    /// Returns `true` if a stage program is currently executing.
    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    /// Returns `true` if the player is locked (`JmsEventMode(jms, 1)`) by the active program.
    pub fn is_player_locked(&self) -> bool {
        self.active.as_ref().is_some_and(|p| p.locked)
    }

    /// Returns a reference to the active `ProgContext`, if any.
    pub fn context(&self) -> Option<&ProgContext> {
        self.active.as_ref()
    }

    /// Returns the currently open stage message ID, if any.
    pub fn active_message_id(&self) -> Option<i32> {
        self.active_message_id
    }

    /// Returns the sound effects triggered during the last `tick`.
    pub fn sound_effects(&self) -> &[u32] {
        &self.sound_effects
    }

    /// Returns any ending triggered by `p_game_end`.
    pub fn triggered_ending(&self) -> Option<u8> {
        self.triggered_ending
    }

    /// Advances the active stage program by one frame (`dt` seconds, `down` newly pressed keys,
    /// `held` currently held keys). Returns `true` while the program continues running, or `false`
    /// when the program finishes (setting `completion_flag` if `flag_set` is still `true`).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        down: u32,
        held: u32,
        dt: f32,
        flags: &mut EventFlags,
        puzzle: &mut PuzzleState,
        inventory: &mut InventoryManager,
        overlay: &mut CutawayOverlayEngine,
        riddle: RiddleDifficulty,
    ) -> bool {
        self.sound_effects.clear();
        let Some(mut p) = self.active.take() else {
            return false;
        };

        let finished = match (p.stage, p.prog_no) {
            // Stage 2 #1: Opening observation deck restroom start (s02_game_start)
            (2, 1) => {
                p.locked = true;
                self.requested_demo = Some(201);
                flags.set(18, true);
                p.locked = false;
                true
            }
            // Stage 3 #1: Observation deck car map pickup (s03_item_in_car, 0x5a0bd0)
            // Displays out_p_incar with out/p_incar_map sprite, waits for button, fades map over 1.0s
            // with SE 0x2b17, shows message 49 ("Got the map of Silent Hill."), and sets flag 28.
            (3, 1) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("out_p_incar"), flags);
                    overlay.set_incar_alpha(1.0);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.sound_effects.push(0x2b17);
                        p.timer = 0.0;
                        p.step = 2;
                    }
                    false
                }
                2 => {
                    p.timer += dt;
                    let a = (1.0 - p.timer).max(0.0);
                    overlay.set_incar_alpha(a);
                    if p.timer >= 1.0 {
                        self.active_message_id = Some(49);
                        p.step = 3;
                    }
                    false
                }
                3 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = None;
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 3 #2: Re-examining the car after map pickup (s03_nothing_in_car, 0x5a0d80)
            (3, 2) => match p.step {
                0 => {
                    p.locked = true;
                    p.flag_set = false;
                    overlay.set_picture(Some("out_p_incar"), flags);
                    overlay.set_incar_alpha(0.0);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = Some(50);
                        p.step = 2;
                    }
                    false
                }
                2 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = None;
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 7 #4: Room 208 closed clock inspection (s07_clock_time)
            (7, 4) => match p.step {
                0 => {
                    p.locked = true;
                    p.flag_set = false;
                    overlay.set_picture(Some("apt_clock_close"), flags);
                    self.active_message_id = Some(58);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = None;
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 7 #5: Use Clock Key on Room 208 clock (s07_use_clock_key)
            (7, 5) => {
                p.locked = true;
                inventory.item_lose(28); // Clock key
                self.sound_effects.push(11033);
                flags.set(71, true);
                p.locked = false;
                true
            }
            // Stage 7 #6: Room 208 open clock hands puzzle (s07_try_move_clock, 0x59a690)
            (7, 6) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("apt_clock_open"), flags);
                    p.timer = 0.0;
                    p.step = 1;
                    false
                }
                1 => {
                    if let Some(se) = step_clock_needles(puzzle, held, dt, &mut p.timer) {
                        self.sound_effects.push(se);
                    }
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        if is_clock_solved(puzzle) {
                            self.sound_effects.push(0x2b1b);
                            flags.set(72, true);
                        } else {
                            p.flag_set = false;
                        }
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 19 #4: Hospital 3F carbon paper memo (s19_carbon_number)
            (19, 4) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("hsp_p_carbon"), flags);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 20 #5: Hospital 3F wall blood symbols (s20_guruguru_number)
            (20, 5) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("hsp_p_boxnumber"), flags);
                    flags.set(203, true);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 20 #10: Shower drain key fishing with combined Bent needle + Piece of hair (87)
            (20, 10) => {
                p.locked = true;
                inventory.item_lose(87);
                inventory.item_get_now(37); // Elevator key
                flags.set(210, true);
                self.sound_effects.push(0x2b21);
                p.locked = false;
                true
            }
            // Stage 53 #1: Born from a Wish start (s53_wish_start)
            (53, 1) => {
                inventory.item_get_now(10); // Revolver
                inventory.item_get_now(11); // Revolver bullets x2 boxes
                inventory.item_get_now(11);
                inventory.item_get_now(2); // First-aid kit
                true
            }
            // Stage 53 #2: Born from a Wish Chinese cleaver pickup (s53_get_cleaver)
            (53, 2) => {
                inventory.item_get_now(17);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 54 #1: Born from a Wish ending (s54_end_wish)
            (54, 1) => {
                self.triggered_ending = Some(0x40);
                true
            }
            // Stage 55 #1: Baldwin Mansion map pickup (s55_get_map)
            (55, 1) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("add_mapboldwin"), flags);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.sound_effects.push(0x2b17);
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 55 #4: Baldwin Mansion Black Board pickup (s55_get_black_board)
            (55, 4) => {
                inventory.item_get_now(80);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 55 #5: Baldwin Mansion Red Board pickup (s55_get_red_board)
            (55, 5) => {
                inventory.item_get_now(81);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 55 #6: Baldwin Mansion garden tombstone board puzzle (s55_board_puzzle)
            (55, 6) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("add_p_teien"), flags);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        if !evaluate_baldwin_tombstone_boards(flags, inventory) {
                            p.flag_set = false;
                        }
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 55 #8: Give Birthday card to Ernest (s55_ernest_card)
            (55, 8) => {
                inventory.item_lose(84);
                true
            }
            // Stage 55 #10: Give White liquid to Ernest (s55_ernest_liquid)
            (55, 10) => {
                inventory.item_lose(85);
                true
            }
            // Stage 56 #3: Baldwin Mansion White Board pickup (s56_get_white_board)
            (56, 3) => {
                inventory.item_get_now(79);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 56 #4: Use Acacia key (s56_use_acacia_key)
            (56, 4) => {
                inventory.item_lose(82);
                self.sound_effects.push(11011);
                true
            }
            // Stage 56 #5: Matches pickup (s56_get_matches)
            (56, 5) => {
                inventory.item_get_now(83);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 56 #6: Light attic candlestick with Matches (s56_light_candle)
            (56, 6) => {
                inventory.item_lose(83);
                self.sound_effects.push(21000);
                flags.set(712, true);
                true
            }
            // Stage 56 #7: Birthday card pickup (s56_birthday_card)
            (56, 7) => {
                inventory.item_get_now(84);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 58 #3: Blue Creek White liquid pickup (s58_get_white_liquid)
            (58, 3) => {
                inventory.item_get_now(85);
                self.sound_effects.push(0x2b21);
                true
            }
            _ => {
                let _ = riddle;
                true
            }
        };

        if finished {
            if p.flag_set && p.completion_flag != 0 {
                flags.set(p.completion_flag as usize, true);
            }
            false
        } else {
            overlay.evaluate_frame(flags, puzzle, riddle_level_index(riddle), 0);
            self.active = Some(p);
            true
        }
    }
}
