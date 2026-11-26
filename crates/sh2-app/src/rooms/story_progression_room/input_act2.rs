//! Keyboard controls for Tab 2 (Act 2 & *Born from a Wish* Visual Puzzles & Key-Door Simulator, `SH2-114`).

use winit::keyboard::KeyCode;

use super::{
    play_confirm_se, play_cursor_se, Act2VisualPuzzle, StoryProgressionRoomState,
    ACT2_TRG10_KEY_DOORS,
};
use crate::state::AppState;
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::events::stages_act2::{
    apply_prison_scaffold_tablet, hangman_innocent_slot, normalize_retail_event_entry_for_input,
    pull_hangman_noose, rotate_labyrinth_face_cube, selected_briefcase_word, stage_table_act2,
    turn_briefcase_drum, Act2EnhancementToggles, BoardRotation, CubeTurnDirection, GravestoneBoard,
    MusicBoxKind, RetailTriggerKind, ALL_ACT2_STAGE_IDS, BRIEFCASE_TRUNK_DRUMS,
    ITEM_MUSIC_BOX_CINDERELLA, ITEM_MUSIC_BOX_LITTLE_MERMAID, ITEM_MUSIC_BOX_SNOW_WHITE,
};
use sh2_core::events::{evaluate_event_entry, EventEvalInput, EventTriggerState, PZ_DRUM};
use sh2_core::InventoryManager;

pub(crate) fn handle_act2_key(state: &mut AppState, key_code: KeyCode) {
    if let Some((pz, msg)) = match key_code {
        KeyCode::Digit1 => Some((
            Act2VisualPuzzle::WishGravestone,
            "Act 2 Puzzle [1]: Baldwin Mansion 3-Board Gravestone ([V] snap quads)",
        )),
        KeyCode::Digit2 => Some((
            Act2VisualPuzzle::HotelBriefcase,
            "Act 2 Puzzle [2]: Hotel Rm 204 Briefcase 4-Drum Lock & Photo Marker",
        )),
        KeyCode::Digit3 => Some((
            Act2VisualPuzzle::HotelEggsAndVcr,
            "Act 2 Puzzle [3]: Hotel Otherworld Egg Doors, Rm 312 VCR & Cupboard",
        )),
        KeyCode::Digit4 => Some((
            Act2VisualPuzzle::LabyrinthAndGallows,
            "Act 2 Puzzle [4]: Labyrinth Rotating Face Cube, 6 Hanged Men & Gallows",
        )),
        _ => None,
    } {
        state.story_progression_room.act2_puzzle = pz;
        state.story_progression_room.status_banner = msg.to_string();
        play_cursor_se(state);
        return;
    }

    match key_code {
        KeyCode::PageDown | KeyCode::PageUp => {
            let n = ALL_ACT2_STAGE_IDS.len();
            let step = if key_code == KeyCode::PageDown {
                1
            } else {
                n - 1
            };
            state.story_progression_room.act2_stage_idx =
                (state.story_progression_room.act2_stage_idx + step) % n;
            play_cursor_se(state);
            return;
        }
        KeyCode::BracketLeft | KeyCode::BracketRight => {
            let n = ACT2_TRG10_KEY_DOORS.len();
            let step = if key_code == KeyCode::BracketRight {
                1
            } else {
                n - 1
            };
            state.story_progression_room.act2_trg10_door_idx =
                (state.story_progression_room.act2_trg10_door_idx + step) % n;
            play_cursor_se(state);
            return;
        }
        KeyCode::KeyK => {
            let rm = &mut state.story_progression_room;
            rm.act2_carrying_key = !rm.act2_carrying_key;
            let (_, _, kid, lbl) = ACT2_TRG10_KEY_DOORS[rm.act2_trg10_door_idx];
            rm.status_banner = format!("{lbl}: Carrying Key #{kid} -> {}", rm.act2_carrying_key);
            play_cursor_se(state);
            return;
        }
        KeyCode::KeyA | KeyCode::KeyU => {
            test_act2_trg10_door(
                &mut state.sim_core.scenario,
                &mut state.story_progression_room,
                key_code == KeyCode::KeyU,
            );
            play_confirm_se(state);
            return;
        }
        KeyCode::KeyN => {
            state.story_progression_room.act2_corridor_room_idx =
                (state.story_progression_room.act2_corridor_room_idx + 1) % 17;
            play_cursor_se(state);
            return;
        }
        KeyCode::KeyV => {
            let rm = &mut state.story_progression_room;
            rm.act2_toggles = if rm.act2_toggles == Act2EnhancementToggles::vanilla_ps2() {
                Act2EnhancementToggles::enhanced()
            } else {
                Act2EnhancementToggles::vanilla_ps2()
            };
            rm.status_banner = format!(
                "Toggled Act 2 Fixes -> GravestoneAlign={}, MusicBoxVol={}, Prisoner60Hz={}",
                rm.act2_toggles.gravestone_boards_alignment_fix,
                rm.act2_toggles.music_box_volume_fix,
                rm.act2_toggles.prisoner_timer_60fps_fix
            );
            play_confirm_se(state);
            return;
        }
        _ => {}
    }

    match state.story_progression_room.act2_puzzle {
        Act2VisualPuzzle::WishGravestone => handle_act2_gravestone_key(state, key_code),
        Act2VisualPuzzle::HotelBriefcase => handle_act2_briefcase_key(state, key_code),
        Act2VisualPuzzle::HotelEggsAndVcr => handle_act2_eggs_vcr_key(state, key_code),
        Act2VisualPuzzle::LabyrinthAndGallows => handle_act2_labyrinth_key(state, key_code),
    }
}

fn rebuild_gravestone_stack(
    s: &mut sh2_core::scenario::ScenarioState,
    rm: &mut StoryProgressionRoomState,
) {
    s.flags.set(704, false);
    rm.act2_gravestone = sh2_core::events::stages_act2::WishGravestonePuzzle::new();
    for (i, b) in [
        GravestoneBoard::White,
        GravestoneBoard::Black,
        GravestoneBoard::Red,
    ]
    .into_iter()
    .enumerate()
    {
        s.inventory_mgr.give_raw(b.item_id(), 1);
        let _ = rm.act2_gravestone.place_board(
            b,
            rm.act2_gravestone_rotations[i],
            &mut s.flags,
            &mut s.inventory_mgr,
        );
    }
}

fn handle_act2_gravestone_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    let boards = [
        GravestoneBoard::White,
        GravestoneBoard::Black,
        GravestoneBoard::Red,
    ];
    match key_code {
        KeyCode::KeyZ | KeyCode::KeyX | KeyCode::KeyC => {
            let idx = match key_code {
                KeyCode::KeyZ => 0usize,
                KeyCode::KeyX => 1,
                _ => 2,
            };
            rm.act2_gravestone_rotations[idx] = match rm.act2_gravestone_rotations[idx] {
                BoardRotation::Deg0 => BoardRotation::Right90,
                BoardRotation::Right90 => BoardRotation::Deg180,
                BoardRotation::Deg180 => BoardRotation::Left90,
                BoardRotation::Left90 => BoardRotation::Deg0,
            };
            rebuild_gravestone_stack(s, rm);
            rm.status_banner = format!(
                "Rotated {:?} Board -> {:?} (Solved={})",
                boards[idx],
                rm.act2_gravestone_rotations[idx],
                s.flags.get(704)
            );
            play_confirm_se(state);
        }
        KeyCode::KeyB => {
            s.flags.set(704, false);
            let popped = rm
                .act2_gravestone
                .pop_board(&mut s.flags, &mut s.inventory_mgr);
            rm.status_banner = format!("Popped Top Gravestone Board (LIFO) -> {popped:?}");
            play_cursor_se(state);
        }
        KeyCode::Space => {
            rm.act2_gravestone_rotations = [
                BoardRotation::Deg0,
                BoardRotation::Deg0,
                BoardRotation::Left90,
            ];
            rebuild_gravestone_stack(s, rm);
            rm.status_banner =
                "Solved Baldwin Mansion 3-Board Gravestone -> Flag 704 & Acacia Key (#82) Awarded!"
                    .to_string();
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_act2_briefcase_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::ArrowUp | KeyCode::ArrowDown => {
            let step = if key_code == KeyCode::ArrowDown { 1 } else { 3 };
            rm.act2_drum_idx = (rm.act2_drum_idx + step) % 4;
            play_cursor_se(state);
        }
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let delta = if key_code == KeyCode::ArrowRight {
                1
            } else {
                -1
            };
            let _ = turn_briefcase_drum(
                rm.act2_drum_idx,
                delta,
                &mut s.flags,
                &mut s.puzzle_state,
                &mut s.inventory_mgr,
            );
            play_cursor_se(state);
        }
        KeyCode::KeyB => {
            rm.act2_show_marker_still = !rm.act2_show_marker_still;
            play_confirm_se(state);
        }
        KeyCode::Space => {
            let word = selected_briefcase_word(&s.flags);
            for (w, &ch) in word.as_bytes().iter().enumerate() {
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
            rm.status_banner =
                format!("Aligned Briefcase Drums to \"{word}\" -> Unlocked Flag 413!");
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_act2_eggs_vcr_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::KeyZ | KeyCode::KeyX => {
            let f = if key_code == KeyCode::KeyZ { 380 } else { 381 };
            let v = !s.flags.get(f);
            s.flags.set(f, v);
            play_confirm_se(state);
        }
        KeyCode::KeyC => {
            let v = !s.flags.get(389);
            s.flags.set(388, true);
            s.flags.set(389, v);
            play_confirm_se(state);
        }
        KeyCode::KeyB => {
            rm.act2_show_vcr_still = !rm.act2_show_vcr_still;
            play_cursor_se(state);
        }
        KeyCode::KeyM => {
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
            let fix = rm.act2_toggles.music_box_volume_fix;
            let _ = rm.act2_music_box.play_melody(
                s.riddle_difficulty,
                fix,
                &mut s.flags,
                &mut s.inventory_mgr,
            );
            rm.status_banner =
                "Solved Hotel Lobby 3-Seat Music Box -> Flag 472 & Stairway Key!".to_string();
            play_confirm_se(state);
        }
        KeyCode::KeyW => {
            if !s.flags.get(446) {
                s.inventory_mgr.give_raw(5, 120);
                s.inventory_mgr.give_raw(1, 5);
                let _ = rm
                    .act2_cupboard
                    .deposit_all(&mut s.flags, &mut s.inventory_mgr);
                rm.status_banner =
                    "Deposited Inventory into 2F Elevator Cupboard (Flag 400 Weight OK!)"
                        .to_string();
            } else {
                let _ = rm
                    .act2_cupboard
                    .withdraw_all(&mut s.flags, &mut s.inventory_mgr);
                rm.status_banner = "Withdrew Stored Items from 2F Elevator Cupboard!".to_string();
            }
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_act2_labyrinth_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let dir = match key_code {
                KeyCode::ArrowUp => CubeTurnDirection::Up,
                KeyCode::ArrowDown => CubeTurnDirection::Down,
                KeyCode::ArrowLeft => CubeTurnDirection::Left,
                _ => CubeTurnDirection::Right,
            };
            let c = rotate_labyrinth_face_cube(&mut s.flags, dir);
            let st = if c.passage_open { "OPEN" } else { "SEALED" };
            rm.status_banner = format!(
                "Rotated Face Cube ({dir:?}) -> Face={}, Roll={}, Door={st}",
                c.plane, c.roll
            );
            play_confirm_se(state);
        }
        KeyCode::KeyZ | KeyCode::KeyX => {
            let step = if key_code == KeyCode::KeyX { 1 } else { 5 };
            rm.act2_hangman_selected_slot = (rm.act2_hangman_selected_slot + step) % 6;
            play_cursor_se(state);
        }
        KeyCode::Enter => {
            let slot = rm.act2_hangman_selected_slot;
            let out = pull_hangman_noose(slot, &s.puzzle_state, &mut s.flags);
            rm.status_banner = format!("Pulled Hangman Noose #{} -> {out:?}", slot + 1);
            play_confirm_se(state);
        }
        KeyCode::KeyB => {
            rm.act2_show_manacles_still = !rm.act2_show_manacles_still;
            play_cursor_se(state);
        }
        KeyCode::Space => {
            for id in [59u8, 60, 61] {
                s.inventory_mgr.give_raw(id, 1);
            }
            let _ = apply_prison_scaffold_tablet(92, &mut s.flags, &mut s.inventory_mgr);
            if let Some(slot) = hangman_innocent_slot(&s.puzzle_state) {
                rm.act2_hangman_selected_slot = slot;
                let _ = pull_hangman_noose(slot, &s.puzzle_state, &mut s.flags);
            }
            rm.status_banner =
                "Inserted 3 Gallows Tablets (Flag 304) & Freed Innocent Man!".to_string();
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
        flags.set(raw.flag_0() as usize, raw.flag_0_expected_set());
    }
    if raw.flag_1() != 0 {
        flags.set(raw.flag_1() as usize, raw.flag_1_expected_set());
    }
    let mut inv = InventoryManager::new();
    if rm.act2_carrying_key {
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
        used_item_id: (is_menu && rm.act2_carrying_key).then_some(key_id),
        is_demo_mode: false,
        action_difficulty: ActionDifficulty::Normal,
        player_can_search: true,
    };
    let pre = RetailTriggerKind::UseOrActionCarryingItem.matches_input(key_id, &input, &inv);
    let norm = normalize_retail_event_entry_for_input(raw, &input, &inv);
    let mut ts = EventTriggerState::new();
    let box_pos = [0u8; 10];
    let fired = pre && evaluate_event_entry(0, &norm, &box_pos, &flags, &input, &mut ts) > 0.0;
    rm.act2_trg10_unlocked = Some(fired);
    let mode = if is_menu {
        "Inventory Menu Use"
    } else {
        "Walk Up + Action Button"
    };
    rm.act2_trg10_last_result = if fired {
        format!("UNLOCKED! {label} via {mode} (Key #{key_id})")
    } else {
        format!("LOCKED! {label} requires Key #{key_id} (Press [K] to pick up)")
    };
    rm.status_banner = rm.act2_trg10_last_result.clone();
}
