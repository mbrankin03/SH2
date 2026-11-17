//! Wood Side & Blue Creek Apartments interactive riddles & horror sequences (`prog_apart.c`,
//! `stg_apart_e2f.c`, `stg_apart_e3fw.c`, `stg_apart_w1f.c`, `stg_apart_w2f.c`, `stg_apart_out.c`).
//!
//! Disassembled 1:1 from `sh2pc.exe` (`0x00595810..0x0059b0f0`) and `SLUS_202.28`
//! (`0x01f02000..0x01f04100`), plus `OldManCoinFix.cpp` and `PuzzleAlignmentFixes.cpp`.

use super::Act1VanillaFixesConfig;
use crate::config::RiddleDifficulty;
use crate::events::{EventFlags, PuzzleState, PZ_SAFE_CODE, PZ_SAFE_PROG};
use crate::inventory::InventoryManager;

/// Item ID for Handgun Bullets (`5`, awarded 4x from Blue Creek Rm 203 Safe).
pub const ACT1_ITEM_HANDGUN_BULLETS: u8 = 5;
/// Item ID for Clock Key (`28`, `0x1c`, pulled from Rm 202 Butterfly Hole, `sh2pc.exe` `0x0059aa3f`).
pub const ACT1_ITEM_CLOCK_KEY: u8 = 28;
/// Item ID for Apartment Fire-Escape Key (`29`, `0x1d`).
pub const ACT1_ITEM_FIRE_ESCAPE_KEY: u8 = 29;
/// Item ID for Lyne House Key (`30`, `0x1e`, awarded by Blue Creek Rm 105 3-Coin Desk).
pub const ACT1_ITEM_LYNE_HOUSE_KEY: u8 = 30;
/// Item ID for Canned Juice (`49`, `0x31`, dropped down Wood Side 2F Trash Chute, `0x0059b010`).
pub const ACT1_ITEM_CANNED_JUICE: u8 = 49;
/// Item ID for Coin [Snake] (`50`, `0x32`).
pub const ACT1_ITEM_COIN_SNAKE: u8 = 50;
/// Item ID for Coin [Old Man] (`51`, `0x33`, `EvProgGetCoinOfOldman` at `0x00595810`).
pub const ACT1_ITEM_COIN_OLD_MAN: u8 = 51;
/// Item ID for Coin [Prisoner] (`52`, `0x34`).
pub const ACT1_ITEM_COIN_PRISONER: u8 = 52;

/// Flag `79` (`0x4f`, `0x1f7a9e8 |= 0x8000`): Clock Key retrieved from Rm 202 Butterfly Hole (`0x0059aa71`).
pub const ACT1_FLAG_BUTTERFLY_HOLE_TAKEN: usize = 79;
/// Flag `95` (`0x5f`): Canned Juice dropped down Wood Side 2F Trash Chute (`0x0059b010`, `OldManCoinFix.cpp`).
pub const ACT1_FLAG_CANNED_JUICE_DROPPED: usize = 95;
/// Flag `96` (`0x60`): Coin [Old Man] picked up from Courtyard trash pile (`0x00595810`).
pub const ACT1_FLAG_OLD_MAN_COIN_TAKEN: usize = 96;
/// Flag `102` (`0x66`, `0x1f7a9ec |= 0x40`): Wallet fished out of Rm 203 Toilet (`0x00597776`).
pub const ACT1_FLAG_WALLET_PULLED_FROM_TOILET: usize = 102;
/// Base flag `103` (`103..=107`, `0x1f7a9ec` bits `7..=11`): 5-bit Rm 203 Safe dial position (`0..=19`, `0x00596fd0`).
pub const ACT1_FLAG_SAFE_DIAL_BIT0: usize = 103;
/// Flag `108` (`0x6c`, `0x1f7a9ec |= 0x1000`): Rm 203 Safe unlocked (`0x00597128`).
pub const ACT1_FLAG_SAFE_UNLOCKED: usize = 108;
/// Flag `109` (`0x6d`): Rm 203 Safe ammunition contents collected.
pub const ACT1_FLAG_SAFE_LOOTED: usize = 109;
/// Flag `118` (`0x76`): Secondary Old Man Coin completion flag (`0x00595810`).
pub const ACT1_FLAG_OLD_MAN_COIN_SUB: usize = 118;
/// Flag `138` (`0x8a`): Apartment Fire-Escape door unlocked (`stg_apart_e2f` -> `stg_apart_w2f`).
pub const ACT1_FLAG_FIRE_ESCAPE_UNLOCKED: usize = 138;
/// Flag `142` (`0x8e`, `0x1f7a9f0` bit `14`): Blue Creek Rm 105 3-Coin Desk solved (`0x0059864a`).
pub const ACT1_FLAG_COIN_DESK_SOLVED: usize = 142;

/// Sound effect `16005` (`0x3e85`): Trash compacting/falling down chute (`0x0059b010`).
pub const ACT1_SE_TRASH_CHUTE_DROP: u32 = 0x3e85;
/// Sound effect `16007` (`0x3e87`): Rm 203 Safe dial click (`EvProgSubSafeLockRotate` `0x0059705b`).
pub const ACT1_SE_SAFE_DIAL_CLICK: u32 = 0x3e87;
/// Sound effect `16008` (`0x3e88`): Rm 203 Safe heavy latch unlock (`EvProgOpenSafe` `0x00597b6d`).
pub const ACT1_SE_SAFE_UNLOCK: u32 = 0x3e88;

/// Formats a single 1-based safe code token (`1..=20`, where `raw_slot` is `0..=19`) exactly as
/// `EvProgSubWalletMemo` (`sh2pc.exe` `0x00596c00`, `SLUS_202.28` `0x01f028a0`).
///
/// - **Easy / Normal (`0 | 1`):** Two-digit decimal `"01".."20"`.
/// - **Hard (`2`):** `1..=9` -> `"1".."9"`; `10..=19` -> `"VV0".."VV9"` on even slots (`0, 2`) and
///   `"X0".."X9"` on odd slots (`1, 3`); `20` -> `"XX"`.
/// - **Extra (`3`):** `1..=9` -> `"1".."9"`; `10..=20` -> `'a'..'k'` (`raw_slot + 0x58`).
#[must_use]
pub fn format_act1_wallet_safe_slot(
    slot_index: usize,
    raw_slot: u8,
    difficulty: RiddleDifficulty,
) -> String {
    let v = raw_slot.min(19) + 1;
    match difficulty {
        RiddleDifficulty::Easy | RiddleDifficulty::Normal => format!("{v:02}"),
        RiddleDifficulty::Hard => {
            if v >= 20 {
                "XX".to_string()
            } else if v >= 10 {
                let ones = v % 10;
                if slot_index.is_multiple_of(2) {
                    format!("VV{ones}")
                } else {
                    format!("X{ones}")
                }
            } else {
                format!("{v}")
            }
        }
        RiddleDifficulty::Extra => {
            if v < 10 {
                format!("{v}")
            } else {
                char::from(b'a' + (v - 10)).to_string()
            }
        }
    }
}

/// Formats the complete 4-step Rm 203 Wallet safe combination memo (`>>` right, `<<` left, `>>` right).
///
/// Matches `EvProgSubWalletMemo` (`sh2pc.exe` `0x00596c00`).
#[must_use]
pub fn format_act1_wallet_safe_memo(puzzle: &PuzzleState, difficulty: RiddleDifficulty) -> String {
    let raw = puzzle.raw();
    let code = [
        raw[PZ_SAFE_CODE],
        raw[PZ_SAFE_CODE + 1],
        raw[PZ_SAFE_CODE + 2],
        raw[PZ_SAFE_CODE + 3],
    ];
    let t0 = format_act1_wallet_safe_slot(0, code[0], difficulty);
    let t1 = format_act1_wallet_safe_slot(1, code[1], difficulty);
    let t2 = format_act1_wallet_safe_slot(2, code[2], difficulty);
    let t3 = format_act1_wallet_safe_slot(3, code[3], difficulty);
    format!("{t0}>>{t1}<<{t2}>>{t3}")
}

/// Outcome of interacting with the Rm 203 clogged toilet (`EvProgWalletInToilet` `0x00597540` /
/// `EvProgWalletOutToilet` `0x005977d0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act1ToiletWalletResult {
    /// Stage `.mes` message ID displayed (`2` prompt, `3` fished out, `4` re-examined).
    pub message_id: u8,
    /// Formatted wallet combination string if inspected.
    pub memo_text: Option<String>,
    /// Whether `ACT1_FLAG_WALLET_PULLED_FROM_TOILET` (`102`) is now set.
    pub wallet_retrieved: bool,
}

/// Executes `EvProgWalletInToilet` (`0x00597540`) or `EvProgWalletOutToilet` (`0x005977d0`).
pub fn act1_interact_toilet_wallet(
    chose_yes: bool,
    difficulty: RiddleDifficulty,
    puzzle: &PuzzleState,
    flags: &mut EventFlags,
) -> Act1ToiletWalletResult {
    if flags.get(ACT1_FLAG_WALLET_PULLED_FROM_TOILET) {
        return Act1ToiletWalletResult {
            message_id: 4,
            memo_text: Some(format_act1_wallet_safe_memo(puzzle, difficulty)),
            wallet_retrieved: true,
        };
    }
    if !chose_yes {
        return Act1ToiletWalletResult {
            message_id: 2,
            memo_text: None,
            wallet_retrieved: false,
        };
    }
    flags.set(ACT1_FLAG_WALLET_PULLED_FROM_TOILET, true);
    Act1ToiletWalletResult {
        message_id: 3,
        memo_text: Some(format_act1_wallet_safe_memo(puzzle, difficulty)),
        wallet_retrieved: true,
    }
}

/// Dial rotation direction in `EvProgSubSafeLockRotate` (`sh2pc.exe` `0x00596fd0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act1SafeDialDirection {
    /// Counter-clockwise / `<<` (`pos = (pos + 1) % 20`).
    Left,
    /// Clockwise / `>>` (`pos = (pos + 19) % 20`).
    Right,
}

/// Result of a single notch rotation on the Blue Creek W2F Rm 203 Safe dial (`0x00596fd0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act1SafeStepOutcome {
    /// Updated 0-based dial position (`0..=19`, displayed on dial as `1..=20`).
    pub dial_pos: u8,
    /// Updated `PZ_SAFE_PROG` stage (`0..=4`).
    pub progress_stage: u8,
    /// Whether the safe unlocked on this notch (`ACT1_FLAG_SAFE_UNLOCKED`).
    pub unlocked: bool,
    /// Sound effects emitted (`ACT1_SE_SAFE_DIAL_CLICK`, plus `ACT1_SE_SAFE_UNLOCK` on solve).
    pub unlock_se: Option<u32>,
}

/// Reads the 5-bit safe dial position (`0..=19`) stored in `EventFlags` `103..=107` (`0x00596fd0`).
#[must_use]
pub fn act1_read_safe_dial_pos(flags: &EventFlags) -> u8 {
    let mut raw = 0u8;
    for bit in 0..5 {
        if flags.get(ACT1_FLAG_SAFE_DIAL_BIT0 + bit) {
            raw |= 1 << bit;
        }
    }
    raw % 20
}

fn write_safe_dial_pos(flags: &mut EventFlags, pos: u8) {
    let clamped = pos % 20;
    for bit in 0..5 {
        flags.set(ACT1_FLAG_SAFE_DIAL_BIT0 + bit, ((clamped >> bit) & 1) != 0);
    }
}

/// Rotates the W2F Rm 203 Safe dial by one notch, matching `EvProgSubSafeLockRotate` (`0x00596fd0`).
///
/// - Stage `0`: accepts `Left` or `Right`; reaching `code[0]` advances `PZ_SAFE_PROG` to `1`.
/// - Stage `1`: requires `Right` (`>>`); rotating `Left` resets `PZ_SAFE_PROG` to `0` (or `1` if landing on `code[0]`).
/// - Stage `2`: requires `Left` (`<<`); rotating `Right` resets `PZ_SAFE_PROG`.
/// - Stage `3`: requires `Right` (`>>`); reaching `code[3]` sets `flag(108)` and plays `0x3e88`.
pub fn act1_rotate_safe_dial(
    direction: Act1SafeDialDirection,
    puzzle: &mut PuzzleState,
    flags: &mut EventFlags,
) -> Act1SafeStepOutcome {
    let cur = act1_read_safe_dial_pos(flags);
    let next = match direction {
        Act1SafeDialDirection::Left => (cur + 1) % 20,
        Act1SafeDialDirection::Right => (cur + 19) % 20,
    };
    write_safe_dial_pos(flags, next);

    let code = [
        puzzle.raw()[PZ_SAFE_CODE],
        puzzle.raw()[PZ_SAFE_CODE + 1],
        puzzle.raw()[PZ_SAFE_CODE + 2],
        puzzle.raw()[PZ_SAFE_CODE + 3],
    ];
    let mut prog = puzzle.raw()[PZ_SAFE_PROG];

    match prog {
        0 => {
            if next == code[0] {
                prog = 1;
            }
        }
        1 => {
            if direction == Act1SafeDialDirection::Right {
                if next == code[1] {
                    prog = 2;
                }
            } else {
                prog = u8::from(next == code[0]);
            }
        }
        2 => {
            if direction == Act1SafeDialDirection::Left {
                if next == code[2] {
                    prog = 3;
                }
            } else {
                prog = u8::from(next == code[0]);
            }
        }
        3 => {
            if direction == Act1SafeDialDirection::Right {
                if next == code[3] {
                    prog = 4;
                    flags.set(ACT1_FLAG_SAFE_UNLOCKED, true);
                }
            } else {
                prog = u8::from(next == code[0]);
            }
        }
        _ => {}
    }

    puzzle.raw_mut()[PZ_SAFE_PROG] = prog;
    let unlocked = flags.get(ACT1_FLAG_SAFE_UNLOCKED);
    Act1SafeStepOutcome {
        dial_pos: next,
        progress_stage: prog,
        unlocked,
        unlock_se: unlocked.then_some(ACT1_SE_SAFE_UNLOCK),
    }
}

/// Collects the 4 boxes of Handgun Bullets inside the unlocked W2F Rm 203 Safe (`gi_list` in `stg_apart_w2f`).
pub fn act1_claim_safe_contents(flags: &mut EventFlags, inventory: &mut InventoryManager) -> u16 {
    if !flags.get(ACT1_FLAG_SAFE_UNLOCKED) || flags.get(ACT1_FLAG_SAFE_LOOTED) {
        return 0;
    }
    flags.set(ACT1_FLAG_SAFE_LOOTED, true);
    let before = inventory.count(ACT1_ITEM_HANDGUN_BULLETS);
    for _ in 0..4 {
        inventory.item_get_now(ACT1_ITEM_HANDGUN_BULLETS);
    }
    inventory.count(ACT1_ITEM_HANDGUN_BULLETS) - before
}

/// Inspects the Wood Side 2F Trash Chute (`EvProgLookDustChute` at `sh2pc.exe` `0x0059ade0`).
///
/// Displays message `15`, plus hint message `16` exclusively on `RiddleDifficulty::Easy` (`riddle == 0`).
#[must_use]
pub fn act1_look_trash_chute(difficulty: RiddleDifficulty) -> &'static [u8] {
    if difficulty == RiddleDifficulty::Easy {
        &[15, 16]
    } else {
        &[15]
    }
}

/// Uses `Canned juice` (`49`) on the Wood Side 2F Trash Chute (`EvProgUseCannedJuice` `0x0059b010`).
pub fn act1_use_canned_juice_on_chute(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> Option<u32> {
    if flags.get(ACT1_FLAG_CANNED_JUICE_DROPPED) || !inventory.has_item(ACT1_ITEM_CANNED_JUICE) {
        return None;
    }
    inventory.item_lose(ACT1_ITEM_CANNED_JUICE);
    flags.set(ACT1_FLAG_CANNED_JUICE_DROPPED, true);
    Some(ACT1_SE_TRASH_CHUTE_DROP)
}

/// Evaluates whether the 3D `Coin [Old Man]` model (`kind = 0x071f`, `Stage 13` `mdl_list[1]` at `0x008de900`)
/// is visible in the Courtyard trash pile.
///
/// - **Vanilla PS2 (`fixes.old_man_coin_fix == false`):** `flag_on = 0, flag_off = 96` -> visible even before dropping Canned Juice!
/// - **SH2:EE (`fixes.old_man_coin_fix == true`):** `flag_on = 95, flag_off = 96` (`OldManCoinFix.cpp`).
#[must_use]
pub fn act1_is_old_man_coin_prop_visible(
    flags: &EventFlags,
    fixes: Act1VanillaFixesConfig,
) -> bool {
    if flags.get(ACT1_FLAG_OLD_MAN_COIN_TAKEN) {
        return false;
    }
    if fixes.old_man_coin_fix {
        flags.get(ACT1_FLAG_CANNED_JUICE_DROPPED)
    } else {
        true
    }
}

/// Picks up `Coin [Old Man]` (`51`) from the Wood Side Courtyard chute (`EvProgGetCoinOfOldman` `0x00595810`).
pub fn act1_pickup_old_man_coin_courtyard(
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> bool {
    if !flags.get(ACT1_FLAG_CANNED_JUICE_DROPPED) || flags.get(ACT1_FLAG_OLD_MAN_COIN_TAKEN) {
        return false;
    }
    inventory.item_get_now(ACT1_ITEM_COIN_OLD_MAN);
    flags.set(ACT1_FLAG_OLD_MAN_COIN_TAKEN, true);
    flags.set(ACT1_FLAG_OLD_MAN_COIN_SUB, true);
    true
}

/// Unlocks the Wood Side 2F West Fire-Escape door into Blue Creek (`ACT1_ITEM_FIRE_ESCAPE_KEY` `29`).
pub fn act1_use_fire_escape_key(flags: &mut EventFlags, inventory: &InventoryManager) -> bool {
    if !inventory.has_item(ACT1_ITEM_FIRE_ESCAPE_KEY) {
        return false;
    }
    flags.set(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED, true);
    true
}

/// Which of the three Apartment Coins is being rendered in the Rm 105 5-slot desk cutaway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act1DeskCoinKind {
    /// `Coin [Old Man]` (`51`).
    OldMan,
    /// `Coin [Snake]` (`50`).
    Snake,
    /// `Coin [Prisoner]` (`52`).
    Prisoner,
}

/// 2D cutaway quad UV (`[u1, v1, u2, v2]`) and screen vertex bounds (`[x0, y0, x1, y1]`) for a coin
/// in `R_APT_W_RM_105` (`0x22`), preserving both 1:1 vanilla PS2 and SH2:EE `CoinPuzzleFix`
/// (`Patches/PuzzleAlignmentFixes.cpp` lines 98–181).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Act1CoinQuadLayout {
    /// Texture coordinates `[u1, v1, u2, v2]` in Q4 texel space.
    pub uv: [f32; 4],
    /// Screen-space quad corners `[x0, y0, x1, y1]` in Q4 pixel units.
    pub bounds: [i16; 4],
}

/// Computes the exact coin sprite UVs and slot bounds for `slot` (`0..=4`), gated by `fixes.coin_puzzle_alignment_fix`.
#[must_use]
pub const fn act1_coin_puzzle_quad_layout(
    coin: Act1DeskCoinKind,
    slot: usize,
    fixes: Act1VanillaFixesConfig,
) -> Option<Act1CoinQuadLayout> {
    if slot >= 5 {
        return None;
    }
    let uv = if fixes.coin_puzzle_alignment_fix {
        match coin {
            Act1DeskCoinKind::OldMan => [0.0, 0.0, 1024.0, 1024.0],
            Act1DeskCoinKind::Snake => [0.0, 1055.0, 1024.0, 2075.0],
            Act1DeskCoinKind::Prisoner => [-4.0, 2082.0, 1028.0, 3108.0],
        }
    } else {
        match coin {
            Act1DeskCoinKind::OldMan => [0.0, 0.0, 752.0, 1008.0],
            Act1DeskCoinKind::Snake => [0.0, 1024.0, 752.0, 2032.0],
            Act1DeskCoinKind::Prisoner => [0.0, 2048.0, 752.0, 3056.0],
        }
    };
    let bounds = if fixes.coin_puzzle_alignment_fix {
        match slot {
            0 => [-3160, -708, -2090, 362],
            1 => [-1790, -682, -720, 388],
            2 => [-414, -650, 656, 420],
            3 => [962, -612, 2032, 458],
            _ => [2348, -580, 3418, 490],
        }
    } else {
        match slot {
            0 => [-3150, -680, -2126, 344],
            1 => [-1750, -650, -726, 374],
            2 => [-450, -620, 574, 404],
            3 => [950, -590, 1974, 434],
            _ => [2350, -560, 3374, 464],
        }
    };
    Some(Act1CoinQuadLayout { uv, bounds })
}

/// Evaluates the Rm 202 Butterfly Hole encounter (`EvProgAnyoneInHole` `0x0059a860` / `EvProgNooneInHole` `0x0059abb0`).
///
/// Returns `(message_id, awarded_clock_key)`.
pub fn act1_interact_butterfly_hole(
    chose_yes: bool,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> (u8, bool) {
    if flags.get(ACT1_FLAG_BUTTERFLY_HOLE_TAKEN) {
        // `EvProgNooneInHole` (`0x0059abb0`): displays message `3`.
        return (3, false);
    }
    if !chose_yes {
        return (2, false);
    }
    // `EvProgAnyoneInHole` (`0x0059a860`): sets tension BGM `540`, awards Clock Key `28`, sets `flag(79)`, clears `540`.
    flags.set(540, true);
    inventory.item_get_now(ACT1_ITEM_CLOCK_KEY);
    flags.set(ACT1_FLAG_BUTTERFLY_HOLE_TAKEN, true);
    flags.set(540, false);
    (19, true)
}
