//! Wood Side & Blue Creek Apartments interactive riddles & horror sequences (`prog_apart.c`,
//! `stg_apart_e2f.c`, `stg_apart_e3fw.c`, `stg_apart_w1f.c`, `stg_apart_w2f.c`, `stg_apart_out.c`).
//!
//! Disassembled 1:1 from `sh2pc.exe` (`0x00595810..0x0059b0f0`) and `SLUS_202.28`
//! (`0x01f02000..0x01f04100`), plus `OldManCoinFix.cpp` and `PuzzleAlignmentFixes.cpp`.

use crate::config::RiddleDifficulty;
use crate::events::{EventFlags, PuzzleState, PZ_SAFE_CODE, PZ_SAFE_PROG};
use crate::inventory::InventoryManager;

/// Item ID for Handgun Bullets (`5`, awarded 4x from Blue Creek Rm 203 Safe).
pub const ACT1_ITEM_HANDGUN_BULLETS: u8 = 5;
/// Item ID for Clock Key (`28`, `0x1c`, pulled from Rm 202 Butterfly Hole, `sh2pc.exe` `0x0059aa3f`).
pub const ACT1_ITEM_CLOCK_KEY: u8 = 28;
/// Item ID for Courtyard Key (`29`, `0x1d`, `inventory/catalog.rs`, `sh2pc.exe` `0x0059b8aa`).
pub const ACT1_ITEM_COURTYARD_KEY: u8 = 29;
/// Item ID for Apartment Fire-Escape Key (`30`, `0x1e`, `ACT1_APART_E2F_EVENT_LIST[31]` `cond = 0xa12831ec`).
pub const ACT1_ITEM_FIRE_ESCAPE_KEY: u8 = 30;
/// Item ID for Lyne House Key (`31`, `0x1f`, awarded by Blue Creek Rm 105 3-Coin Desk).
pub const ACT1_ITEM_LYNE_HOUSE_KEY: u8 = 31;
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
/// Flag `93` (`0x5d`): Wood Side 2F West Fire-Escape door unlocked (`ACT1_APART_E2F_EVENT_LIST[30..=31]`: `0x805d0000` / `0x0001c05d`).
pub const ACT1_FLAG_FIRE_ESCAPE_UNLOCKED: usize = 93;
/// Flag `95` (`0x5f`): Canned Juice dropped down Wood Side 2F Trash Chute (`0x0059b010`, `OldManCoinFix.cpp`).
pub const ACT1_FLAG_CANNED_JUICE_DROPPED: usize = 95;
/// Flag `96` (`0x60`): Coin [Old Man] picked up from Courtyard trash pile (`0x00595810`).
pub const ACT1_FLAG_OLD_MAN_COIN_TAKEN: usize = 96;
/// Flag `102` (`0x66`, `0x1f7a9ec |= 0x40`): Wallet fished out of Rm 203 Toilet (`0x00597776`).
pub const ACT1_FLAG_WALLET_PULLED_FROM_TOILET: usize = 102;
/// Base flag `103` (`103..=107`, `0x1f7a9ec` bits `7..=11`): 5-bit Rm 203 Safe dial position (`0..=19`, `0x00596fd0`).
pub const ACT1_FLAG_SAFE_DIAL_BIT0: usize = 103;
/// Flag `108` (`0x6c`, `0x1f7a9ec |= 0x1000`): Rm 203 Safe unlocked (`0x00597155`).
pub const ACT1_FLAG_SAFE_UNLOCKED: usize = 108;
/// Flag `109` (`0x6d`): Rm 203 Safe ammunition contents collected.
pub const ACT1_FLAG_SAFE_LOOTED: usize = 109;
/// Flag `118` (`0x76`): Old Man Coin `mdl_list[1]` `flag_0` (`flag_off`) in `stg_apart_out` (`0x008de900`, `OldManCoinFix.cpp:30`).
pub const ACT1_FLAG_OLD_MAN_COIN_SUB: usize = 118;
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

/// Rotates the W2F Rm 203 Safe dial by one notch, matching `EvProgSubSafeLockRotate`
/// (`sh2pc.exe` `0x00597121..0x00597270`, where `PZ_SAFE_PROG` at `0x1f7ae64..67` is `i8[4]`
/// initialized to `[-1, -1, -1, -1]` / `0xffffffff` by `FlagInit`).
///
/// - When `prog[0] < 0`: accepts `Left` or `Right`; reaching `code[0]` writes `prog[0] = next`.
/// - When `prog[0] >= 0 && prog[1] < 0`: requires `Right` (`>>`); `Left` resets `PZ_SAFE_PROG` to `0xffffffff` (`0x59723b`).
/// - When `prog[1] >= 0 && prog[2] < 0`: requires `Left` (`<<`); `Right` resets `PZ_SAFE_PROG` to `0xffffffff` (`0x5971a1`).
/// - When `prog[2] >= 0`: requires `Right` (`>>`); reaching `code[3]` sets `flag(108)` (`0x597155`), while `Left` resets `PZ_SAFE_PROG` (`0x597207`).
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
    let p0 = puzzle.raw()[PZ_SAFE_PROG] as i8;
    let p1 = puzzle.raw()[PZ_SAFE_PROG + 1] as i8;
    let p2 = puzzle.raw()[PZ_SAFE_PROG + 2] as i8;

    match direction {
        Act1SafeDialDirection::Right => {
            if p2 >= 0 {
                if next == code[3] {
                    flags.set(ACT1_FLAG_SAFE_UNLOCKED, true);
                }
            } else if p1 >= 0 {
                puzzle.raw_mut()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4].fill(0xff);
            } else if p0 >= 0 {
                if next == code[1] {
                    puzzle.raw_mut()[PZ_SAFE_PROG + 1] = next;
                }
            } else if next == code[0] {
                puzzle.raw_mut()[PZ_SAFE_PROG] = next;
            }
        }
        Act1SafeDialDirection::Left => {
            if p2 >= 0 {
                puzzle.raw_mut()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4].fill(0xff);
            } else if p1 >= 0 {
                if next == code[2] {
                    puzzle.raw_mut()[PZ_SAFE_PROG + 2] = next;
                }
            } else if p0 >= 0 {
                puzzle.raw_mut()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4].fill(0xff);
            } else if next == code[0] {
                puzzle.raw_mut()[PZ_SAFE_PROG] = next;
            }
        }
    }

    let unlocked = flags.get(ACT1_FLAG_SAFE_UNLOCKED);
    let raw_p = &puzzle.raw()[PZ_SAFE_PROG..PZ_SAFE_PROG + 3];
    let progress_stage = if unlocked {
        4
    } else if (raw_p[2] as i8) >= 0 {
        3
    } else if (raw_p[1] as i8) >= 0 {
        2
    } else if (raw_p[0] as i8) >= 0 {
        1
    } else {
        0
    };
    Act1SafeStepOutcome {
        dial_pos: next,
        progress_stage,
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
