//! Interactive puzzle evaluators and memo string formatters (`prog_apart.c`, `prog_hospital.c`, `prog_wish.c`).

use crate::config::RiddleDifficulty;
use crate::inventory::InventoryManager;

use super::{
    riddle_level_index, EventFlags, MessageInsertState, PuzzleState, Sh2Rng, PROG_KEY_LEFT,
    PROG_KEY_RIGHT, PZ_BOX_CODE, PZ_CLOCK, PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL,
};

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
        || (0..4usize).all(|i| puzzle.raw()[PZ_SAFE_PROG + i] == puzzle.raw()[PZ_SAFE_CODE + i]);
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
    let matches_goal =
        (0..4usize).all(|i| puzzle.raw()[PZ_WHEEL + i] == puzzle.raw()[PZ_WHEEL_GOAL + i]);
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
pub fn try_complete_hospital_box(flags: &mut EventFlags, inventory: &mut InventoryManager) -> bool {
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
        (if flags.get(694 + b * 2) { 1 } else { 0 }) | (if flags.get(695 + b * 2) { 2 } else { 0 })
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
