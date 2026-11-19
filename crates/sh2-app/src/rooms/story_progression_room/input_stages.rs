//! Keyboard controls for Tab 1 (Act 1 Stages 1–27) and Tab 2 (Act 2 & *Born from a Wish* Stages 25–58).

use winit::keyboard::KeyCode;

use super::{play_confirm_se, play_cursor_se, StoryProgressionRoomState, ACT2_TRG10_KEY_DOORS};
use crate::state::AppState;
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::events::stages_act1::{
    act1_apply_quiz_poison_gas_damage, act1_apply_roof_pyramid_head_push,
    act1_hospital_wing_keypad_goal, act1_pickup_old_man_coin_courtyard,
    act1_read_louise_wall_blood_code, act1_read_safe_dial_pos, act1_rotate_safe_dial,
    act1_select_quiz_chest_button, act1_step_elevator_radio_quiz, act1_use_canned_juice_on_chute,
    act1_use_fire_escape_key, Act1SafeDialDirection, Act1VanillaFixesConfig, ACT1_ALL_STAGES,
    ACT1_FLAG_LOUISE_WHEEL_OPEN, ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, ACT1_FLAG_QUIZ_CHEST_SOLVED,
    ACT1_FLAG_QUIZ_CHEST_WRONG, ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED, ACT1_ITEM_CANNED_JUICE,
    ACT1_ITEM_FIRE_ESCAPE_KEY,
};
use sh2_core::events::stages_act2::{
    apply_prison_scaffold_tablet, hangman_innocent_slot, normalize_retail_event_entry_for_input,
    pull_hangman_noose, rotate_labyrinth_face_cube, stage_table_act2, turn_briefcase_drum,
    Act2EnhancementToggles, BoardRotation, CubeTurnDirection, GravestoneBoard, MusicBoxKind,
    RetailTriggerKind, ALL_ACT2_STAGE_IDS, BRIEFCASE_TRUNK_DRUMS, BRIEFCASE_TRUNK_WORDS,
    FLAG_BRIEFCASE_WORD_INDEX_BASE, ITEM_BLACK_BOARD, ITEM_MUSIC_BOX_CINDERELLA,
    ITEM_MUSIC_BOX_LITTLE_MERMAID, ITEM_MUSIC_BOX_SNOW_WHITE, ITEM_RED_BOARD,
    ITEM_TABLET_GLUTTONOUS_PIG, ITEM_TABLET_OPPRESSOR, ITEM_TABLET_SEDUCTRESS, ITEM_WHITE_BOARD,
};
use sh2_core::events::{
    evaluate_event_entry, EventEvalInput, EventTriggerState, PZ_DRUM, PZ_SAFE_CODE, PZ_SAFE_PROG,
    PZ_WHEEL, PZ_WHEEL_GOAL,
};
use sh2_core::InventoryManager;

pub(super) fn handle_act1_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::PageDown | KeyCode::PageUp => {
            let n = ACT1_ALL_STAGES.len();
            rm.act1_stage_idx = (rm.act1_stage_idx
                + if key_code == KeyCode::PageDown {
                    1
                } else {
                    n - 1
                })
                % n;
            play_cursor_se(state);
        }
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let dir = if key_code == KeyCode::ArrowLeft {
                Act1SafeDialDirection::Left
            } else {
                Act1SafeDialDirection::Right
            };
            let out = act1_rotate_safe_dial(dir, &mut s.puzzle_state, &mut s.flags);
            rm.status_banner = format!(
                "Safe Dial {dir:?}: Notch={}, Stage={}/4, Unlocked={}",
                out.dial_pos + 1,
                out.progress_stage,
                out.unlocked
            );
            play_cursor_se(state);
        }
        KeyCode::Space => {
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
                "Stepped Safe Dial to Step #{}: Notch={}, Unlocked={}",
                step_idx + 1,
                out.dial_pos + 1,
                out.unlocked
            );
            play_confirm_se(state);
        }
        KeyCode::KeyV => {
            rm.act1_fixes = if rm.act1_fixes == Act1VanillaFixesConfig::vanilla_ps2() {
                Act1VanillaFixesConfig::enhanced()
            } else {
                Act1VanillaFixesConfig::vanilla_ps2()
            };
            play_confirm_se(state);
        }
        KeyCode::KeyC => {
            s.inventory_mgr.give_raw(ACT1_ITEM_CANNED_JUICE, 1);
            s.inventory_mgr.give_raw(ACT1_ITEM_FIRE_ESCAPE_KEY, 1);
            let _ = act1_use_canned_juice_on_chute(&mut s.flags, &mut s.inventory_mgr);
            let _ = act1_pickup_old_man_coin_courtyard(&mut s.flags, &mut s.inventory_mgr);
            let _ = act1_use_fire_escape_key(&mut s.flags, &mut s.inventory_mgr);
            rm.act1_ph_zone = (rm.act1_ph_zone + 1) % 4;
            rm.act1_ph_hp_ratio = if rm.act1_ph_hp_ratio <= 0.4 {
                1.0
            } else {
                rm.act1_ph_hp_ratio - 0.3
            };
            play_confirm_se(state);
        }
        KeyCode::Digit1 => {
            for d in act1_hospital_wing_keypad_goal(s.riddle_difficulty) {
                let _ = rm
                    .act1_wing_keypad
                    .press_digit(d, s.riddle_difficulty, &mut s.flags);
            }
            play_confirm_se(state);
        }
        KeyCode::Digit2 => {
            act1_read_louise_wall_blood_code(&mut s.flags);
            let goals: [u8; 4] = s.puzzle_state.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
                .try_into()
                .unwrap_or([0; 4]);
            s.puzzle_state.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&goals);
            if !s.flags.get(ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED) {
                let _ = rm
                    .act1_louise_box
                    .on_exit_box(0x1357_2468, &mut s.puzzle_state, &s.flags);
                rm.act1_james_hp = act1_apply_roof_pyramid_head_push(100.0, 100.0, &mut s.flags);
            } else {
                s.flags.set(ACT1_FLAG_LOUISE_WHEEL_OPEN, false);
                s.puzzle_state.raw_mut()[PZ_WHEEL + 3] = (goals[3] + 8) % 9;
                let _ = rm
                    .act1_louise_box
                    .rotate_wheel(3, &mut s.puzzle_state, &mut s.flags);
            }
            play_confirm_se(state);
        }
        KeyCode::Digit3 => {
            rm.act1_radio_quiz_ticks = if rm.act1_radio_quiz_ticks + 140.0 >= 5150.0 {
                220.0
            } else {
                rm.act1_radio_quiz_ticks + 140.0
            };
            let _ = act1_step_elevator_radio_quiz(rm.act1_radio_quiz_ticks, false, &mut s.flags);
            play_cursor_se(state);
        }
        KeyCode::Digit4 => {
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_ATTEMPTED, false);
            let q1_choice = if !s.flags.get(ACT1_FLAG_QUIZ_CHEST_WRONG) {
                2
            } else {
                3
            };
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_WRONG, false);
            s.flags.set(ACT1_FLAG_QUIZ_CHEST_SOLVED, false);
            for (q, c) in [(1, q1_choice), (2, 1), (3, 3)] {
                let _ = act1_select_quiz_chest_button(q, c, &mut s.flags);
            }
            if q1_choice == 2 {
                let (_, hp, _) = act1_apply_quiz_poison_gas_damage(0.5, 0.6, 100.0, 100.0, 10.0);
                rm.act1_james_hp = hp;
            }
            play_confirm_se(state);
        }
        _ => {}
    }
}

pub(super) fn handle_act2_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::PageDown | KeyCode::PageUp => {
            let n = ALL_ACT2_STAGE_IDS.len();
            rm.act2_stage_idx = (rm.act2_stage_idx
                + if key_code == KeyCode::PageDown {
                    1
                } else {
                    n - 1
                })
                % n;
            play_cursor_se(state);
        }
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let n = ACT2_TRG10_KEY_DOORS.len();
            rm.act2_trg10_door_idx = (rm.act2_trg10_door_idx
                + if key_code == KeyCode::ArrowRight {
                    1
                } else {
                    n - 1
                })
                % n;
            play_cursor_se(state);
        }
        KeyCode::KeyK => {
            rm.act2_carrying_key = !rm.act2_carrying_key;
            play_cursor_se(state);
        }
        KeyCode::KeyA | KeyCode::KeyU => {
            test_act2_trg10_door(s, rm, key_code == KeyCode::KeyU);
            play_confirm_se(state);
        }
        KeyCode::KeyV => {
            rm.act2_toggles = if rm.act2_toggles == Act2EnhancementToggles::vanilla_ps2() {
                Act2EnhancementToggles::enhanced()
            } else {
                Act2EnhancementToggles::vanilla_ps2()
            };
            play_confirm_se(state);
        }
        KeyCode::Digit1 => {
            for id in [
                ITEM_TABLET_OPPRESSOR,
                ITEM_TABLET_GLUTTONOUS_PIG,
                ITEM_TABLET_SEDUCTRESS,
            ] {
                s.inventory_mgr.give_raw(id, 1);
            }
            let _ = apply_prison_scaffold_tablet(92, &mut s.flags, &mut s.inventory_mgr);
            play_confirm_se(state);
        }
        KeyCode::Digit2 => {
            let c = rotate_labyrinth_face_cube(&mut s.flags, CubeTurnDirection::Right);
            if c.connectivity_level == 0 {
                let _ = rotate_labyrinth_face_cube(&mut s.flags, CubeTurnDirection::Up);
            }
            play_confirm_se(state);
        }
        KeyCode::Digit3 => {
            if let Some(slot) = hangman_innocent_slot(&s.puzzle_state) {
                let _ = pull_hangman_noose(slot, &s.puzzle_state, &mut s.flags);
            }
            play_confirm_se(state);
        }
        KeyCode::Digit4 => {
            for id in [
                ITEM_MUSIC_BOX_LITTLE_MERMAID,
                ITEM_MUSIC_BOX_SNOW_WHITE,
                ITEM_MUSIC_BOX_CINDERELLA,
            ] {
                s.inventory_mgr.give_raw(id, 1);
            }
            rm.act2_music_box
                .return_boxes_to_inventory(&mut s.flags, &mut s.inventory_mgr);
            let order = if s.riddle_difficulty == RiddleDifficulty::Normal {
                [
                    MusicBoxKind::LittleMermaid,
                    MusicBoxKind::SnowWhite,
                    MusicBoxKind::Cinderella,
                ]
            } else {
                [
                    MusicBoxKind::SnowWhite,
                    MusicBoxKind::LittleMermaid,
                    MusicBoxKind::Cinderella,
                ]
            };
            for (seat, kind) in order.into_iter().enumerate() {
                let _ = rm
                    .act2_music_box
                    .place_box(seat, kind, &mut s.flags, &mut s.inventory_mgr);
            }
            let _ = rm.act2_music_box.play_melody(
                s.riddle_difficulty,
                rm.act2_toggles.music_box_volume_fix,
                &mut s.flags,
                &mut s.inventory_mgr,
            );
            play_confirm_se(state);
        }
        KeyCode::Digit5 => {
            if !s.flags.get(446) {
                s.inventory_mgr.give_raw(5, 120);
                s.inventory_mgr.give_raw(1, 5);
                let _ = rm
                    .act2_cupboard
                    .deposit_all(&mut s.flags, &mut s.inventory_mgr);
            } else {
                let _ = rm
                    .act2_cupboard
                    .withdraw_all(&mut s.flags, &mut s.inventory_mgr);
            }
            play_confirm_se(state);
        }
        KeyCode::Digit6 => {
            let mut idx = 0usize;
            for b in 0..5 {
                if s.flags.get(FLAG_BRIEFCASE_WORD_INDEX_BASE + b) {
                    idx |= 1 << b;
                }
            }
            let next = (idx + 1) % BRIEFCASE_TRUNK_WORDS.len();
            for b in 0..5 {
                s.flags
                    .set(FLAG_BRIEFCASE_WORD_INDEX_BASE + b, ((next >> b) & 1) != 0);
            }
            for (w, &ch) in BRIEFCASE_TRUNK_WORDS[next].as_bytes().iter().enumerate() {
                while BRIEFCASE_TRUNK_DRUMS[w][(s.puzzle_state.raw()[PZ_DRUM + w] & 7) as usize]
                    != ch
                {
                    let _ = turn_briefcase_drum(
                        w,
                        1,
                        &mut s.flags,
                        &mut s.puzzle_state,
                        &mut s.inventory_mgr,
                    );
                }
            }
            play_confirm_se(state);
        }
        KeyCode::Digit7 => {
            rm.act2_corridor_room_idx = (rm.act2_corridor_room_idx + 1) % 17;
            play_cursor_se(state);
        }
        KeyCode::Digit8 => {
            s.flags.set(704, false);
            rm.act2_gravestone = sh2_core::events::stages_act2::WishGravestonePuzzle::new();
            for id in [ITEM_BLACK_BOARD, ITEM_WHITE_BOARD, ITEM_RED_BOARD] {
                s.inventory_mgr.give_raw(id, 1);
            }
            for b in [
                GravestoneBoard::Black,
                GravestoneBoard::White,
                GravestoneBoard::Red,
            ] {
                let _ = rm.act2_gravestone.place_board(
                    b,
                    BoardRotation::Deg0,
                    &mut s.flags,
                    &mut s.inventory_mgr,
                );
            }
            let _ = rm
                .act2_gravestone
                .pop_board(&mut s.flags, &mut s.inventory_mgr);
            let _ = rm.act2_gravestone.place_board(
                GravestoneBoard::Red,
                BoardRotation::Left90,
                &mut s.flags,
                &mut s.inventory_mgr,
            );
            rm.act2_gravestone_rot = match rm.act2_gravestone_rot {
                BoardRotation::Deg0 => BoardRotation::Right90,
                BoardRotation::Right90 => BoardRotation::Deg180,
                BoardRotation::Deg180 => BoardRotation::Left90,
                BoardRotation::Left90 => BoardRotation::Deg0,
            };
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn test_act2_trg10_door(
    s: &mut sh2_core::scenario::ScenarioState,
    rm: &mut StoryProgressionRoomState,
    is_menu: bool,
) {
    let (stg_id, ev_idx, key_id, label) = ACT2_TRG10_KEY_DOORS[rm.act2_trg10_door_idx];
    let Some(table) = stage_table_act2(stg_id) else {
        return;
    };
    let mut raw = table.event_list[ev_idx];
    raw.cond &= !0x0fff_f000;
    let mut flags = s.flags.clone();
    if raw.flag_0() != 0 {
        flags.set(raw.flag_0() as usize, !raw.flag_0_inverted());
    }
    if raw.flag_1() != 0 {
        flags.set(raw.flag_1() as usize, !raw.flag_1_inverted());
    }
    let mut inv = InventoryManager::new();
    if rm.act2_carrying_key || is_menu {
        inv.give_raw(key_id, 1);
    }
    let input = EventEvalInput {
        player_pos: [0.0, 0.0, -200.0],
        player_rot_y: 0.0,
        ev_active: 0,
        action_pressed: !is_menu,
        map_disabled_pressed: false,
        room_changed: false,
        stage_changed: false,
        used_item_id: is_menu.then_some(key_id),
        is_demo_mode: false,
        action_difficulty: ActionDifficulty::Normal,
        player_can_search: true,
    };
    let pre = RetailTriggerKind::UseOrActionCarryingItem.matches_input(key_id, &input, &inv);
    let norm = normalize_retail_event_entry_for_input(raw, &input, &inv);
    let mut ts = EventTriggerState::new();
    let score = evaluate_event_entry(
        0,
        &norm,
        &[0, 0, 0, 0, 0, 0, 0x90, 0x01],
        &flags,
        &input,
        &mut ts,
    );
    rm.act2_trg10_last_result = format!(
        "{label}: matches={pre}, norm={:?}, fired={}",
        norm.trigger_kind(),
        score > 0.0
    );
    rm.status_banner = rm.act2_trg10_last_result.clone();
}
