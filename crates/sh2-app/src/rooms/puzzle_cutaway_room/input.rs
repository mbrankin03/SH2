//! Keyboard input handler for [`super::PuzzleCutawayRoomController`] (`SH2-15`).

use winit::keyboard::KeyCode;

use super::{play_confirm_sound, play_cursor_sound, sync_overlay_to_preset, CUTAWAY_PRESETS};
use crate::state::AppState;
use sh2_core::{
    evaluate_baldwin_tombstone_boards, format_carbon_box_code, init_new_game, place_baldwin_board,
    place_coin_in_desk, step_safe_dial, try_open_safe, use_key_on_hospital_box,
    verify_hospital_box_keypad, verify_hospital_box_wheels, verify_hospital_corridor_keypad,
    ActionDifficulty, ClearData, MessageInsertState, RiddleDifficulty, Sh2Rng, PROG_KEY_A,
    PZ_CLOCK, PZ_DRUM, PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL,
};
use sh2_ui::menu::TitlePhase;

pub(super) fn handle_puzzle_cutaway_key_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) {
    match key_code {
        KeyCode::ArrowLeft | KeyCode::KeyA => {
            state.key_left = pressed;
        }
        KeyCode::ArrowRight | KeyCode::KeyD => {
            state.key_right = pressed;
        }
        _ => {}
    }

    if !pressed {
        return;
    }

    match key_code {
        KeyCode::Escape | KeyCode::Backspace if !repeat => {
            state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
            tracing::info!("Returned from Puzzle & Cutaway Overlays Room to Test Rooms menu.");
            return;
        }
        KeyCode::Tab | KeyCode::PageDown if !repeat => {
            state.puzzle_cutaway_room.preset_idx =
                (state.puzzle_cutaway_room.preset_idx + 1) % CUTAWAY_PRESETS.len();
            sync_overlay_to_preset(state);
            play_cursor_sound(state);
            return;
        }
        KeyCode::PageUp if !repeat => {
            if state.puzzle_cutaway_room.preset_idx == 0 {
                state.puzzle_cutaway_room.preset_idx = CUTAWAY_PRESETS.len() - 1;
            } else {
                state.puzzle_cutaway_room.preset_idx -= 1;
            }
            sync_overlay_to_preset(state);
            play_cursor_sound(state);
            return;
        }
        // [P]: Toggle SH2:EE HD (sh2e/pic) vs Original PS2/PC (data/pic)
        KeyCode::KeyP if !repeat => {
            state.puzzle_cutaway_room.prefer_sh2ee = !state.puzzle_cutaway_room.prefer_sh2ee;
            state.puzzle_cutaway_room.loaded_base_key = None;
            let label = if state.puzzle_cutaway_room.prefer_sh2ee {
                "SH2:EE HD (sh2e/pic)"
            } else {
                "Original PS2/PC (data/pic)"
            };
            state.puzzle_cutaway_room.last_event_desc =
                format!("Switched Cutaway Asset Source -> {label}");
            play_confirm_sound(state);
            return;
        }
        // [R]: Cycle RiddleDifficulty (Easy -> Normal -> Hard -> Extra) & re-run FlagInit
        KeyCode::KeyR if !repeat => {
            let next_riddle = match state.sim_core.scenario.riddle_difficulty {
                RiddleDifficulty::Easy => RiddleDifficulty::Normal,
                RiddleDifficulty::Normal => RiddleDifficulty::Hard,
                RiddleDifficulty::Hard => RiddleDifficulty::Extra,
                RiddleDifficulty::Extra => RiddleDifficulty::Easy,
            };
            state.sim_core.scenario.riddle_difficulty = next_riddle;
            state.puzzle_cutaway_room.seed_counter =
                state.puzzle_cutaway_room.seed_counter.wrapping_add(1);
            state.puzzle_cutaway_room.rng = Sh2Rng::new(
                0x2001_0924u32.wrapping_add(state.puzzle_cutaway_room.seed_counter * 7919),
            );
            let mut clear = ClearData::default();
            if next_riddle == RiddleDifficulty::Extra {
                clear.cleared_levels = 0x07;
                clear.endings_seen = 0x07;
            }
            init_new_game(
                &mut state.sim_core.scenario.flags,
                &mut state.sim_core.scenario.puzzle_state,
                &mut state.puzzle_cutaway_room.rng,
                next_riddle,
                ActionDifficulty::Normal,
                clear,
            );
            sync_overlay_to_preset(state);
            state.puzzle_cutaway_room.last_event_desc = format!(
                "Re-initialized FlagInit with RiddleDifficulty::{:?} (Seed #{})",
                next_riddle, state.puzzle_cutaway_room.seed_counter
            );
            play_confirm_sound(state);
            return;
        }
        // [Space]: Start Stage 3 Car Map Program (#1 if flag 28 is clear, #2 if flag 28 is set)
        KeyCode::Space if !repeat => {
            let has_map = state.sim_core.scenario.flags.get(28);
            if !has_map {
                state.sim_core.scenario.prog_runner.start(3, 1, 28);
                state.puzzle_cutaway_room.preset_idx = 0;
                state.puzzle_cutaway_room.last_event_desc =
                    "Started Stage 3 ev_prog #1 (s03_item_in_car). Press [Enter] to take Silent Hill map!"
                        .to_string();
            } else {
                state.sim_core.scenario.prog_runner.start(3, 2, 0);
                state.puzzle_cutaway_room.last_event_desc =
                    "Started Stage 3 ev_prog #2 (s03_nothing_in_car: map already taken)."
                        .to_string();
            }
            play_confirm_sound(state);
            return;
        }
        // [Enter]: Send PROG_KEY_A to StageProgramRunner if running
        KeyCode::Enter if !repeat && state.sim_core.scenario.prog_runner.is_running() => {
            let s = &mut state.sim_core.scenario;
            s.prog_runner.tick(
                PROG_KEY_A,
                0,
                0.016,
                &mut s.flags,
                &mut s.puzzle_state,
                &mut s.inventory_mgr,
                &mut s.cutaway_overlay,
                s.riddle_difficulty,
            );
            if let Some(&se_id) = s.prog_runner.sound_effects().first() {
                state.puzzle_cutaway_room.last_event_desc =
                    format!("ev_prog triggered SE 0x{se_id:04X} ({se_id})");
            }
            play_confirm_sound(state);
            return;
        }
        _ => {}
    }

    // Preset-specific interactive controls
    let preset_pic = CUTAWAY_PRESETS[state.puzzle_cutaway_room.preset_idx].0;
    match preset_pic {
        "out_p_incar" if key_code == KeyCode::KeyS && !repeat => {
            state.sim_core.scenario.flags.set(28, false);
            state
                .sim_core
                .scenario
                .cutaway_overlay
                .set_picture(Some("out_p_incar"), &state.sim_core.scenario.flags);
            state.puzzle_cutaway_room.last_event_desc =
                "Reset Flag 28 (Map returned to car seat; incar_alpha = 1.0)".to_string();
        }
        "apt_clock_close" | "apt_clock_open" if key_code == KeyCode::KeyS && !repeat => {
            state
                .sim_core
                .scenario
                .puzzle_state
                .write_u16(PZ_CLOCK, 0x8980);
            state.sim_core.scenario.flags.set(117, true);
            state.puzzle_cutaway_room.last_event_desc =
                "Snapped Clock hands to 9:10 (0x8980) -> SOLVED (Flag 117 set)!".to_string();
            play_confirm_sound(state);
        }
        "apt_p_safe_close" => {
            if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                let pos = step_safe_dial(
                    &mut state.sim_core.scenario.flags,
                    &mut state.sim_core.scenario.puzzle_state,
                    false,
                );
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Turned Safe Dial Left (<<) -> points at {} (Solved={})",
                    pos + 1,
                    state.sim_core.scenario.flags.get(108)
                );
                play_cursor_sound(state);
            } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat {
                let pos = step_safe_dial(
                    &mut state.sim_core.scenario.flags,
                    &mut state.sim_core.scenario.puzzle_state,
                    true,
                );
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Turned Safe Dial Right (>>) -> points at {} (Solved={})",
                    pos + 1,
                    state.sim_core.scenario.flags.get(108)
                );
                play_cursor_sound(state);
            } else if key_code == KeyCode::KeyS && !repeat {
                let code: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()
                    [PZ_SAFE_CODE..PZ_SAFE_CODE + 4]
                    .try_into()
                    .unwrap_or([0; 4]);
                state.sim_core.scenario.puzzle_state.raw_mut()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4]
                    .copy_from_slice(&code);
                for b in 0..5 {
                    state
                        .sim_core
                        .scenario
                        .flags
                        .set(103 + b, ((code[3] >> b) & 1) != 0);
                }
                let opened = try_open_safe(
                    &mut state.sim_core.scenario.flags,
                    &state.sim_core.scenario.puzzle_state,
                    &mut state.sim_core.scenario.inventory_mgr,
                );
                state.sim_core.scenario.sync_legacy_views();
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Auto-solved Safe Combination -> opened={opened}, Flag 108={}, Handgun Bullets={}",
                    state.sim_core.scenario.flags.get(108),
                    state.sim_core.scenario.inventory_mgr.count(5)
                );
                play_confirm_sound(state);
            }
        }
        "apt_p_desk_coin" => {
            if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                state.puzzle_cutaway_room.selected_coin_hole =
                    (state.puzzle_cutaway_room.selected_coin_hole + 4) % 5;
            } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat {
                state.puzzle_cutaway_room.selected_coin_hole =
                    (state.puzzle_cutaway_room.selected_coin_hole + 1) % 5;
            } else if matches!(
                key_code,
                KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3
            ) && !repeat
            {
                let coin_id = match key_code {
                    KeyCode::Digit1 => 50u8, // Snake
                    KeyCode::Digit2 => 51u8, // Old Man
                    _ => 52u8,               // Prisoner
                };
                let hole = state.puzzle_cutaway_room.selected_coin_hole;
                if !state.sim_core.scenario.inventory_mgr.has_item(coin_id) {
                    state.sim_core.scenario.inventory_mgr.item_get_now(coin_id);
                }
                let s = &mut state.sim_core.scenario;
                let res = place_coin_in_desk(
                    &mut s.flags,
                    &mut s.inventory_mgr,
                    s.riddle_difficulty,
                    hole,
                    coin_id,
                );
                s.sync_legacy_views();
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Placed Coin #{coin_id} into Hole #{} -> result={res:?}, Solved={}",
                    hole + 1,
                    s.flags.get(142)
                );
                play_confirm_sound(state);
            } else if key_code == KeyCode::KeyS && !repeat {
                for f in 126..=142 {
                    state.sim_core.scenario.flags.set(f, false);
                }
                for c in 50..=52 {
                    if !state.sim_core.scenario.inventory_mgr.has_item(c) {
                        state.sim_core.scenario.inventory_mgr.item_get_now(c);
                    }
                }
                let placements = match state.sim_core.scenario.riddle_difficulty {
                    RiddleDifficulty::Easy => [(50u8, 2usize), (51, 0), (52, 4)],
                    RiddleDifficulty::Normal => [(50, 4), (51, 1), (52, 2)],
                    RiddleDifficulty::Hard => [(50, 3), (51, 1), (52, 4)],
                    RiddleDifficulty::Extra => [(50, 2), (51, 0), (52, 3)],
                };
                let s = &mut state.sim_core.scenario;
                for (coin_id, hole) in placements {
                    place_coin_in_desk(
                        &mut s.flags,
                        &mut s.inventory_mgr,
                        s.riddle_difficulty,
                        hole,
                        coin_id,
                    );
                }
                s.sync_legacy_views();
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Auto-placed {:?} 3-Coin Solution -> Flag 142={}, Lyne Key (#31)={}",
                    s.riddle_difficulty,
                    s.flags.get(142),
                    s.inventory_mgr.has_item(31)
                );
                play_confirm_sound(state);
            }
        }
        "hsp_p_box" => {
            if (key_code == KeyCode::ArrowUp || key_code == KeyCode::KeyW) && !repeat {
                state.puzzle_cutaway_room.selected_box_wheel =
                    (state.puzzle_cutaway_room.selected_box_wheel + 3) % 4;
            } else if (key_code == KeyCode::ArrowDown || key_code == KeyCode::KeyJ) && !repeat {
                state.puzzle_cutaway_room.selected_box_wheel =
                    (state.puzzle_cutaway_room.selected_box_wheel + 1) % 4;
            } else if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                let w = state.puzzle_cutaway_room.selected_box_wheel;
                let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_WHEEL + w];
                state.sim_core.scenario.puzzle_state.raw_mut()[PZ_WHEEL + w] = (cur + 8) % 9;
            } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat {
                let w = state.puzzle_cutaway_room.selected_box_wheel;
                let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_WHEEL + w];
                state.sim_core.scenario.puzzle_state.raw_mut()[PZ_WHEEL + w] = (cur + 1) % 9;
            } else if key_code == KeyCode::Digit1 && !repeat {
                let v = !state.sim_core.scenario.flags.get(195);
                state.sim_core.scenario.flags.set(195, v);
                state.puzzle_cutaway_room.last_event_desc =
                    format!("Toggled Hospital Box Key Locks (Flag 195) -> {v}");
            } else if key_code == KeyCode::Digit2 && !repeat {
                let v = !state.sim_core.scenario.flags.get(197);
                state.sim_core.scenario.flags.set(197, v);
                state.puzzle_cutaway_room.last_event_desc =
                    format!("Toggled Hospital Box Wheel Lock (Flag 197) -> {v}");
            } else if key_code == KeyCode::Digit3 && !repeat {
                let v = !state.sim_core.scenario.flags.get(198);
                state.sim_core.scenario.flags.set(198, v);
                state.puzzle_cutaway_room.last_event_desc =
                    format!("Toggled Hospital Box Keypad Lock (Flag 198) -> {v}");
            } else if key_code == KeyCode::KeyS && !repeat {
                let s = &mut state.sim_core.scenario;
                s.flags.set(203, true);
                let goals: [u8; 4] = s.puzzle_state.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
                    .try_into()
                    .unwrap_or([0; 4]);
                s.puzzle_state.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&goals);
                verify_hospital_box_wheels(
                    &mut s.flags,
                    &mut s.puzzle_state,
                    &mut state.puzzle_cutaway_room.rng,
                );
                let mut ins = MessageInsertState::default();
                let code_str = format_carbon_box_code(&s.puzzle_state, &mut ins);
                let digits: [u8; 4] = [
                    code_str.as_bytes()[0] - b'0',
                    code_str.as_bytes()[1] - b'0',
                    code_str.as_bytes()[2] - b'0',
                    code_str.as_bytes()[3] - b'0',
                ];
                verify_hospital_box_keypad(&mut s.flags, &s.puzzle_state, digits);
                s.inventory_mgr.item_get_now(35);
                s.inventory_mgr.item_get_now(36);
                use_key_on_hospital_box(&mut s.flags, &mut s.inventory_mgr, 86);
                s.sync_legacy_views();
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Unlocked all 4 Hospital Box locks -> Flag 199={}, Piece of Hair (#53)={}",
                    s.flags.get(199),
                    s.inventory_mgr.has_item(53)
                );
                play_confirm_sound(state);
            }
        }
        "hsp_p_quiz" => {
            if matches!(
                key_code,
                KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3
            ) && !repeat
            {
                let r = match key_code {
                    KeyCode::Digit1 => 0usize,
                    KeyCode::Digit2 => 1,
                    _ => 2,
                };
                let a = state.sim_core.scenario.flags.get(234 + 2 * r);
                let b = state.sim_core.scenario.flags.get(235 + 2 * r);
                let cur = (if a { 1 } else { 0 }) | (if b { 2 } else { 0 });
                let next = (cur + 1) % 4;
                state
                    .sim_core
                    .scenario
                    .flags
                    .set(234 + 2 * r, (next & 1) != 0);
                state
                    .sim_core
                    .scenario
                    .flags
                    .set(235 + 2 * r, (next & 2) != 0);
            } else if key_code == KeyCode::KeyS && !repeat {
                let s = &mut state.sim_core.scenario;
                let code = match s.riddle_difficulty {
                    RiddleDifficulty::Easy | RiddleDifficulty::Normal => 7335,
                    RiddleDifficulty::Hard | RiddleDifficulty::Extra => 1328,
                };
                let ok = verify_hospital_corridor_keypad(&mut s.flags, s.riddle_difficulty, code);
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
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Verified 3F Keypad ({code} -> {ok}, Flag 181={}) & set Trick Chest Quiz answers",
                    s.flags.get(181)
                );
                play_confirm_sound(state);
            }
        }
        "htl_p_marker" => {
            let mut w = 0usize;
            for b in 0..5usize {
                if state.sim_core.scenario.flags.get(406 + b) {
                    w |= 1 << b;
                }
            }
            if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                w = (w + 18) % 19;
                for b in 0..5usize {
                    state
                        .sim_core
                        .scenario
                        .flags
                        .set(406 + b, ((w >> b) & 1) != 0);
                }
            } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat {
                w = (w + 1) % 19;
                for b in 0..5usize {
                    state
                        .sim_core
                        .scenario
                        .flags
                        .set(406 + b, ((w >> b) & 1) != 0);
                }
            } else if key_code == KeyCode::KeyS && !repeat {
                let gone = !state.sim_core.scenario.flags.get(404);
                state.sim_core.scenario.flags.set(404, gone);
                state.sim_core.scenario.flags.set(411, gone);
                state.puzzle_cutaway_room.last_event_desc =
                    format!("Toggled Thinner Marker Wipe (Flags 404/411) -> marker_hidden={gone}");
            }
        }
        "htl_p_trank" => {
            state.sim_core.scenario.flags.set(405, true);
            if (key_code == KeyCode::ArrowUp || key_code == KeyCode::KeyW) && !repeat {
                state.puzzle_cutaway_room.selected_drum_idx =
                    (state.puzzle_cutaway_room.selected_drum_idx + 3) % 4;
            } else if (key_code == KeyCode::ArrowDown || key_code == KeyCode::KeyJ) && !repeat {
                state.puzzle_cutaway_room.selected_drum_idx =
                    (state.puzzle_cutaway_room.selected_drum_idx + 1) % 4;
            } else if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                let d = state.puzzle_cutaway_room.selected_drum_idx;
                let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_DRUM + d];
                state.sim_core.scenario.puzzle_state.raw_mut()[PZ_DRUM + d] = (cur + 7) & 7;
            } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat {
                let d = state.puzzle_cutaway_room.selected_drum_idx;
                let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_DRUM + d];
                state.sim_core.scenario.puzzle_state.raw_mut()[PZ_DRUM + d] = (cur + 1) & 7;
            } else if key_code == KeyCode::KeyS && !repeat {
                let v = !state.sim_core.scenario.flags.get(405);
                state.sim_core.scenario.flags.set(405, v);
            }
        }
        "htl_p_egghollow" => {
            if key_code == KeyCode::Digit1 && !repeat {
                let v = !state.sim_core.scenario.flags.get(380);
                state.sim_core.scenario.flags.set(380, v);
            } else if key_code == KeyCode::Digit2 && !repeat {
                let v = !state.sim_core.scenario.flags.get(381);
                state.sim_core.scenario.flags.set(381, v);
            } else if key_code == KeyCode::KeyS && !repeat {
                state.sim_core.scenario.flags.set(380, true);
                state.sim_core.scenario.flags.set(381, true);
            }
        }
        "htl_p_video" => {
            if key_code == KeyCode::Digit1 && !repeat {
                let v = !state.sim_core.scenario.flags.get(388);
                state.sim_core.scenario.flags.set(388, v);
            } else if key_code == KeyCode::Digit2 && !repeat {
                let v = !state.sim_core.scenario.flags.get(389);
                state.sim_core.scenario.flags.set(389, v);
            } else if key_code == KeyCode::KeyS && !repeat {
                state.sim_core.scenario.flags.set(388, true);
                state
                    .sim_core
                    .scenario
                    .flags
                    .set(389, !state.sim_core.scenario.flags.get(389));
            }
        }
        "add_p_teien" => {
            if matches!(
                key_code,
                KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3
            ) && !repeat
            {
                let b = match key_code {
                    KeyCode::Digit1 => 0usize,
                    KeyCode::Digit2 => 1,
                    _ => 2,
                };
                let item_id = 79 + (b as u8);
                let rot = (state.puzzle_cutaway_room.baldwin_rotations[b] + 1) & 3;
                state.puzzle_cutaway_room.baldwin_rotations[b] = rot;
                if !state.sim_core.scenario.inventory_mgr.has_item(item_id) {
                    state.sim_core.scenario.inventory_mgr.item_get_now(item_id);
                }
                place_baldwin_board(
                    &mut state.sim_core.scenario.flags,
                    &mut state.sim_core.scenario.inventory_mgr,
                    item_id,
                    rot,
                );
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Placed Board #{item_id} at rotation {} ({}°)",
                    rot,
                    (rot as u32) * 90
                );
            } else if key_code == KeyCode::KeyS && !repeat {
                let s = &mut state.sim_core.scenario;
                for id in 79..=81 {
                    if !s.inventory_mgr.has_item(id) {
                        s.inventory_mgr.item_get_now(id);
                    }
                }
                place_baldwin_board(&mut s.flags, &mut s.inventory_mgr, 79, 2);
                place_baldwin_board(&mut s.flags, &mut s.inventory_mgr, 80, 0);
                place_baldwin_board(&mut s.flags, &mut s.inventory_mgr, 81, 1);
                let solved = evaluate_baldwin_tombstone_boards(&mut s.flags, &mut s.inventory_mgr);
                s.sync_legacy_views();
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Solved Baldwin Tombstone Boards -> solved={solved}, Flag 704={}, Acacia Key (#82)={}",
                    s.flags.get(704),
                    s.inventory_mgr.has_item(82)
                );
                play_confirm_sound(state);
            }
        }
        "hsp_p_h_elevator" | "hsp_p_h_elevator_2"
            if matches!(
                key_code,
                KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 | KeyCode::Digit4
            ) && !repeat =>
        {
            let bit = match key_code {
                KeyCode::Digit1 => 0,
                KeyCode::Digit2 => 1,
                KeyCode::Digit3 => 2,
                _ => 3,
            };
            state.puzzle_cutaway_room.elevator_floor_mask ^= 1 << bit;
        }
        "dls_p_manacles_on" if key_code == KeyCode::KeyS && !repeat => {
            let next = !state.sim_core.scenario.flags.get(304);
            state.sim_core.scenario.flags.set(304, next);
            if !next {
                state
                    .sim_core
                    .scenario
                    .cutaway_overlay
                    .set_picture(Some("dls_p_manacles_on"), &state.sim_core.scenario.flags);
            }
            state.puzzle_cutaway_room.last_event_desc =
                format!("Toggled Labyrinth Manacles Flag 304 -> {next}");
        }
        _ => {}
    }
}
