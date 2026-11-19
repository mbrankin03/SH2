//! Keyboard controls for Tab 1 (Act 1 Visual Cutaway Puzzles & Stage Riddles, `SH2-113`).

use winit::keyboard::KeyCode;

use super::{play_confirm_se, play_cursor_se, Act1VisualPuzzle};
use crate::state::AppState;
use sh2_core::config::RiddleDifficulty;
use sh2_core::events::stages_act1::{
    act1_apply_quiz_poison_gas_damage, act1_apply_roof_pyramid_head_push,
    act1_hospital_wing_keypad_goal, act1_pickup_old_man_coin_courtyard,
    act1_read_louise_wall_blood_code, act1_read_safe_dial_pos, act1_rotate_safe_dial,
    act1_select_quiz_chest_button, act1_use_canned_juice_on_chute, act1_use_fire_escape_key,
    Act1QuizChestOutcome, Act1SafeDialDirection, Act1VanillaFixesConfig, ACT1_ALL_STAGES,
    ACT1_FLAG_LOUISE_KEY1_OPEN, ACT1_FLAG_LOUISE_KEY2_OPEN, ACT1_FLAG_LOUISE_KEYPAD_OPEN,
    ACT1_FLAG_LOUISE_WHEEL_OPEN, ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, ACT1_FLAG_QUIZ_CHEST_SOLVED,
    ACT1_FLAG_QUIZ_CHEST_WRONG, ACT1_ITEM_CANNED_JUICE, ACT1_ITEM_FIRE_ESCAPE_KEY,
};
use sh2_core::events::{place_coin_in_desk, PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL};

pub(crate) fn handle_act1_key(state: &mut AppState, key_code: KeyCode) {
    match key_code {
        KeyCode::Digit1 => {
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::SafeDial;
            state.story_progression_room.status_banner =
                "Selected Act 1 Puzzle [1]: W2F Rm 203 Safe Dial & Wallet Memo".to_string();
            play_cursor_se(state);
            return;
        }
        KeyCode::Digit2 => {
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::CoinDesk;
            state.story_progression_room.status_banner =
                "Selected Act 1 Puzzle [2]: Rm 105 Five-Hole Coin Desk (Press [V] to shift quads!)"
                    .to_string();
            play_cursor_se(state);
            return;
        }
        KeyCode::Digit3 => {
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::LouiseBox;
            state.story_progression_room.status_banner =
                "Selected Act 1 Puzzle [3]: Brookhaven S14 Louise Locked Box & Wall Blood Code"
                    .to_string();
            play_cursor_se(state);
            return;
        }
        KeyCode::Digit4 => {
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::QuizChest;
            state.story_progression_room.status_banner =
                "Selected Act 1 Puzzle [4]: Trick-or-Treat Radio Quiz & 3F Store Room Quiz Chest"
                    .to_string();
            play_cursor_se(state);
            return;
        }
        KeyCode::PageDown | KeyCode::PageUp => {
            let n = ACT1_ALL_STAGES.len();
            let rm = &mut state.story_progression_room;
            rm.act1_stage_idx = (rm.act1_stage_idx
                + if key_code == KeyCode::PageDown {
                    1
                } else {
                    n - 1
                })
                % n;
            play_cursor_se(state);
            return;
        }
        KeyCode::KeyV => {
            let rm = &mut state.story_progression_room;
            rm.act1_fixes = if rm.act1_fixes == Act1VanillaFixesConfig::vanilla_ps2() {
                Act1VanillaFixesConfig::enhanced()
            } else {
                Act1VanillaFixesConfig::vanilla_ps2()
            };
            rm.status_banner = format!(
                "Toggled Act 1 Fixes -> CoinAlign={}, OldManCoin={}, TeddyLook={}",
                rm.act1_fixes.coin_puzzle_alignment_fix,
                rm.act1_fixes.old_man_coin_fix,
                rm.act1_fixes.teddy_bear_look_fix
            );
            play_confirm_se(state);
            return;
        }
        KeyCode::KeyK => {
            let s = &mut state.sim_core.scenario;
            let rm = &mut state.story_progression_room;
            for d in act1_hospital_wing_keypad_goal(s.riddle_difficulty) {
                let _ = rm
                    .act1_wing_keypad
                    .press_digit(d, s.riddle_difficulty, &mut s.flags);
            }
            s.inventory_mgr.give_raw(ACT1_ITEM_CANNED_JUICE, 1);
            s.inventory_mgr.give_raw(ACT1_ITEM_FIRE_ESCAPE_KEY, 1);
            let _ = act1_use_canned_juice_on_chute(&mut s.flags, &mut s.inventory_mgr);
            let _ = act1_pickup_old_man_coin_courtyard(&mut s.flags, &mut s.inventory_mgr);
            let _ = act1_use_fire_escape_key(&mut s.flags, &mut s.inventory_mgr);
            rm.act1_ph_zone = (rm.act1_ph_zone + 1) % 4;
            rm.status_banner =
                "Unlocked 3F Patient Wing Keypad, Dropped Chute Canned Juice & Unlocked Fire Escape!"
                    .to_string();
            play_confirm_se(state);
            return;
        }
        _ => {}
    }

    match state.story_progression_room.act1_puzzle {
        Act1VisualPuzzle::SafeDial => handle_act1_safe_key(state, key_code),
        Act1VisualPuzzle::CoinDesk => handle_act1_coin_key(state, key_code),
        Act1VisualPuzzle::LouiseBox => handle_act1_louise_key(state, key_code),
        Act1VisualPuzzle::QuizChest => handle_act1_quiz_key(state, key_code),
    }
}

fn handle_act1_safe_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let dir = if key_code == KeyCode::ArrowLeft {
                Act1SafeDialDirection::Left
            } else {
                Act1SafeDialDirection::Right
            };
            let out = act1_rotate_safe_dial(dir, &mut s.puzzle_state, &mut s.flags);
            rm.status_banner = format!(
                "Safe Dial {dir:?}: Notch {:02}, Stage {}/4, Open={}",
                out.dial_pos + 1,
                out.progress_stage,
                out.unlocked
            );
            play_cursor_se(state);
        }
        KeyCode::Space | KeyCode::Enter => {
            let p = &s.puzzle_state.raw()[PZ_SAFE_PROG..PZ_SAFE_PROG + 3];
            let (step_idx, dir) = if (p[0] as i8) < 0 {
                (0usize, Act1SafeDialDirection::Right)
            } else if (p[1] as i8) < 0 {
                (1, Act1SafeDialDirection::Right)
            } else if (p[2] as i8) < 0 {
                (2, Act1SafeDialDirection::Left)
            } else {
                (3, Act1SafeDialDirection::Right)
            };
            let target = s.puzzle_state.raw()[PZ_SAFE_CODE + step_idx];
            let mut out = act1_rotate_safe_dial(dir, &mut s.puzzle_state, &mut s.flags);
            for _ in 0..20 {
                if act1_read_safe_dial_pos(&s.flags) == target {
                    break;
                }
                out = act1_rotate_safe_dial(dir, &mut s.puzzle_state, &mut s.flags);
            }
            rm.status_banner = format!(
                "Stepped Safe Dial to Tumbler #{}: Notch {:02}, Unlocked={}",
                step_idx + 1,
                out.dial_pos + 1,
                out.unlocked
            );
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_act1_coin_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::ArrowLeft => {
            rm.act1_coin_slot = (rm.act1_coin_slot + 4) % 5;
            play_cursor_se(state);
        }
        KeyCode::ArrowRight => {
            rm.act1_coin_slot = (rm.act1_coin_slot + 1) % 5;
            play_cursor_se(state);
        }
        KeyCode::KeyZ | KeyCode::KeyX | KeyCode::KeyC => {
            let coin_id = match key_code {
                KeyCode::KeyZ => 50u8,
                KeyCode::KeyX => 51u8,
                _ => 52u8,
            };
            let slot = rm.act1_coin_slot;
            for c in 0..3usize {
                s.flags.set(126 + 5 * c + slot, false);
            }
            s.inventory_mgr.give_raw(coin_id, 1);
            let res = place_coin_in_desk(
                &mut s.flags,
                &mut s.inventory_mgr,
                s.riddle_difficulty,
                slot,
                coin_id,
            );
            rm.status_banner = format!(
                "Placed Coin #{coin_id} into Desk Slot #{} -> {res:?}",
                slot + 1
            );
            play_confirm_se(state);
        }
        KeyCode::Backspace | KeyCode::KeyD => {
            let slot = rm.act1_coin_slot;
            for c in 0..3usize {
                s.flags.set(126 + 5 * c + slot, false);
            }
            play_cursor_se(state);
        }
        KeyCode::Space => {
            for f in 126..=142 {
                s.flags.set(f, false);
            }
            let sol = match s.riddle_difficulty {
                RiddleDifficulty::Easy => [(50u8, 2usize), (51, 0), (52, 4)],
                RiddleDifficulty::Normal => [(50, 4), (51, 1), (52, 2)],
                RiddleDifficulty::Hard => [(50, 3), (51, 1), (52, 4)],
                RiddleDifficulty::Extra => [(50, 2), (51, 0), (52, 3)],
            };
            for (cid, hole) in sol {
                s.inventory_mgr.give_raw(cid, 1);
                let _ = place_coin_in_desk(
                    &mut s.flags,
                    &mut s.inventory_mgr,
                    s.riddle_difficulty,
                    hole,
                    cid,
                );
            }
            rm.status_banner = format!(
                "Solved {:?} 5-Hole Coin Desk -> Flag 142={}, Lyne House Key (#31) Awarded!",
                s.riddle_difficulty,
                s.flags.get(142)
            );
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_act1_louise_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::ArrowUp => {
            rm.act1_louise_wheel_idx = (rm.act1_louise_wheel_idx + 3) % 4;
            play_cursor_se(state);
        }
        KeyCode::ArrowDown => {
            rm.act1_louise_wheel_idx = (rm.act1_louise_wheel_idx + 1) % 4;
            play_cursor_se(state);
        }
        KeyCode::ArrowLeft => {
            let w = rm.act1_louise_wheel_idx;
            let cur = s.puzzle_state.raw()[PZ_WHEEL + w];
            s.puzzle_state.raw_mut()[PZ_WHEEL + w] = (cur + 7) % 9;
            let _ = rm
                .act1_louise_box
                .rotate_wheel(w, &mut s.puzzle_state, &mut s.flags);
            play_cursor_se(state);
        }
        KeyCode::ArrowRight => {
            let w = rm.act1_louise_wheel_idx;
            let _ = rm
                .act1_louise_box
                .rotate_wheel(w, &mut s.puzzle_state, &mut s.flags);
            play_cursor_se(state);
        }
        KeyCode::KeyB => {
            act1_read_louise_wall_blood_code(&mut s.flags);
            rm.act1_show_wall_blood_still = !rm.act1_show_wall_blood_still;
            rm.status_banner =
                "Inspected 3F Special Treatment Wall Blood Code (Flag 192 set)".to_string();
            play_confirm_se(state);
        }
        KeyCode::KeyP => {
            rm.act1_james_hp = act1_apply_roof_pyramid_head_push(100.0, 100.0, &mut s.flags);
            rm.status_banner = format!(
                "Triggered Roof Pyramid Head Push (Flag 203 set, James HP -> {:.0})",
                rm.act1_james_hp
            );
            play_confirm_se(state);
        }
        KeyCode::KeyE => {
            let goals: [u8; 4] = s.puzzle_state.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
                .try_into()
                .unwrap_or([0; 4]);
            s.puzzle_state.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&goals);
            s.flags.set(ACT1_FLAG_LOUISE_WHEEL_OPEN, false);
            let scrambled =
                rm.act1_louise_box
                    .on_exit_box(0x1357_2468, &mut s.puzzle_state, &s.flags);
            rm.status_banner = format!(
                "Exited Louise Box before Flag 203 -> Anti-Bruteforce Wheel Scramble={scrambled}!"
            );
            play_confirm_se(state);
        }
        KeyCode::Space => {
            act1_read_louise_wall_blood_code(&mut s.flags);
            rm.act1_james_hp = act1_apply_roof_pyramid_head_push(100.0, 100.0, &mut s.flags);
            let goals: [u8; 4] = s.puzzle_state.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
                .try_into()
                .unwrap_or([0; 4]);
            s.puzzle_state.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&goals);
            s.flags.set(ACT1_FLAG_LOUISE_KEY1_OPEN, true);
            s.flags.set(ACT1_FLAG_LOUISE_KEY2_OPEN, true);
            s.flags.set(ACT1_FLAG_LOUISE_WHEEL_OPEN, true);
            s.flags.set(ACT1_FLAG_LOUISE_KEYPAD_OPEN, true);
            let _ = rm
                .act1_louise_box
                .claim_piece_of_hair(&mut s.flags, &mut s.inventory_mgr);
            rm.status_banner =
                "Unlocked All 4 Louise Box Locks -> Claimed Piece of Hair (#53)!".to_string();
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_act1_quiz_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::ArrowUp => {
            rm.act1_quiz_row = (rm.act1_quiz_row + 2) % 3;
            play_cursor_se(state);
        }
        KeyCode::ArrowDown => {
            rm.act1_quiz_row = (rm.act1_quiz_row + 1) % 3;
            play_cursor_se(state);
        }
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let r = rm.act1_quiz_row;
            let base = 234 + 2 * r;
            let cur = u8::from(s.flags.get(base)) | (u8::from(s.flags.get(base + 1)) << 1);
            let next = if key_code == KeyCode::ArrowRight {
                if cur >= 3 {
                    1
                } else {
                    cur + 1
                }
            } else if cur <= 1 {
                3
            } else {
                cur - 1
            };
            s.flags.set(base, (next & 1) != 0);
            s.flags.set(base + 1, (next & 2) != 0);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, false);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_SOLVED, false);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_WRONG, false);
            play_cursor_se(state);
        }
        KeyCode::Enter => {
            let q1 = (u8::from(s.flags.get(234)) | (u8::from(s.flags.get(235)) << 1)).max(1);
            let q2 = (u8::from(s.flags.get(236)) | (u8::from(s.flags.get(237)) << 1)).max(1);
            let q3 = (u8::from(s.flags.get(238)) | (u8::from(s.flags.get(239)) << 1)).max(1);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, false);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_SOLVED, false);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_WRONG, false);
            let _ = act1_select_quiz_chest_button(1, q1, &mut s.flags);
            let _ = act1_select_quiz_chest_button(2, q2, &mut s.flags);
            let out = act1_select_quiz_chest_button(3, q3, &mut s.flags);
            if out == Some(Act1QuizChestOutcome::WrongPoisonSpray) {
                let (_, hp, _) = act1_apply_quiz_poison_gas_damage(0.5, 0.6, 100.0, 100.0, 10.0);
                rm.act1_james_hp = hp;
                rm.act1_poison_flash_timer = 1.2;
                rm.status_banner = format!(
                    "WRONG QUIZ ANSWERS ({q1}, {q2}, {q3}) -> POISON GAS TRAP! James HP={hp:.0}"
                );
            } else {
                rm.status_banner =
                    "CORRECT QUIZ ANSWERS (3, 1, 3) -> Quiz Chest Unlocked (+5 Shells, +2 Ampoules)!"
                        .to_string();
            }
            play_confirm_se(state);
        }
        KeyCode::KeyP => {
            s.flags.set(234, false);
            s.flags.set(235, true);
            s.flags.set(236, true);
            s.flags.set(237, false);
            s.flags.set(238, true);
            s.flags.set(239, true);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, true);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_SOLVED, false);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_WRONG, true);
            let (_, hp, _) = act1_apply_quiz_poison_gas_damage(0.5, 0.6, 100.0, 100.0, 10.0);
            rm.act1_james_hp = hp;
            rm.act1_poison_flash_timer = 1.2;
            rm.status_banner =
                "Triggered Quiz Chest Poison Gas Spray Trap (SE 0x4DB7 -> HP=40)!".to_string();
            play_confirm_se(state);
        }
        KeyCode::Space => {
            for (f, val) in [
                (234, true),
                (235, true),
                (236, true),
                (237, false),
                (238, true),
                (239, true),
            ] {
                s.flags.set(f, val);
            }
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, true);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_SOLVED, true);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_WRONG, false);
            rm.status_banner =
                "Solved 3F Store Room Quiz Chest (3, 1, 3) -> Flag 240 Set!".to_string();
            play_confirm_se(state);
        }
        _ => {}
    }
}
