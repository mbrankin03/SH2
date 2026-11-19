//! Comprehensive unit and retail-binary integration tests for `SH2-114`:
//! Stages `16` & `25..=58` static stage tables (`StageDataHeader`, `Event_List`, `Pos_List`,
//! `ev_prog`, `Item_List`, `Enemy_List`) and interactive `ev_prog` riddles across Toluca Prison,
//! Labyrinth, Lakeview Hotel, and *Born from a Wish*.

use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::events::*;
use sh2_core::inventory::InventoryManager;
use sh2_core::types::{EventListEntry, EventTriggerKind};

#[test]
fn static_stage_tables_25_to_58_and_16_match_retail_counts_and_decode_cleanly() {
    let (mut total_events, mut total_items, mut total_enemies) = (0usize, 0usize, 0usize);
    let eval = EventEvalInput {
        player_pos: [0.0; 3],
        player_rot_y: 0.0,
        ev_active: 0,
        action_pressed: false,
        map_disabled_pressed: false,
        room_changed: false,
        stage_changed: false,
        used_item_id: None,
        is_demo_mode: false,
        action_difficulty: ActionDifficulty::Normal,
        player_can_search: true,
    };

    for &stage_id in &ALL_ACT2_STAGE_IDS {
        let kind = Act2StageId::from_u16(stage_id).expect("valid Act2StageId");
        let table = stage_table_act2(stage_id).expect("table present");
        assert_eq!(
            (kind.as_u16(), table.stage_id, table.stage_kind),
            (stage_id, stage_id, kind)
        );
        assert!(table.event_list.last().is_some_and(|e| e.is_sentinel()));

        total_events += table.active_event_count();
        total_items += table.retail_items.len();
        total_enemies += table.retail_enemies.len();

        for entry in &table.event_list[..table.active_event_count()] {
            if ((entry.cond >> 28) & 0xf) >= 2 {
                if let Some(sh) = entry.shape_kind() {
                    let dec = decode_retail_ev_pos_shape(table.pos_list, entry.ev_pos_offset(), sh);
                    assert!(
                        dec.is_some(),
                        "stage {} ({}) {:?}",
                        stage_id,
                        table.name,
                        sh
                    );
                }
            }
        }

        let (ev_list, gi_list) = (
            table.engine_event_list(),
            table.ps2_item_list_with_sentinel(),
        );
        let (mut engine, mut flags, mut inv) = (
            EventEngine::new(),
            EventFlags::new(),
            InventoryManager::new(),
        );
        let _ = engine.tick_standard(
            &ev_list,
            &gi_list,
            table.pos_list,
            stage_id as u8,
            &eval,
            false,
            false,
            false,
            false,
            &mut flags,
            &mut inv,
        );

        for it in table.retail_items {
            assert!(it.pos_x().is_finite() && it.pos_y().is_finite() && it.pos_z().is_finite());
            assert!(it.pickup_model_kind().is_some());
        }
        for en in table.retail_enemies {
            assert!(en.kind != 0 && en.yaw_radians().is_finite());
            let _ = en.to_ps2_clamped();
        }
    }
    assert_eq!((total_events, total_items, total_enemies), (1173, 119, 145));
}

#[test]
fn delusion3_roach_keypad_and_prison_trapdoor_and_scaffold_riddles() {
    let (mut flags, mut puzzle, mut rng) = (
        EventFlags::new(),
        PuzzleState::new(),
        Sh2Rng::new(0x1234_5678),
    );
    init_new_game(
        &mut flags,
        &mut puzzle,
        &mut rng,
        RiddleDifficulty::Normal,
        ActionDifficulty::Normal,
        ClearData::default(),
    );

    let mut inv = InventoryManager::new();
    inv.give_raw(ITEM_SPIRAL_WRITING_KEY, 1);
    inv.give_raw(ITEM_DRY_CELL_BATTERY, 1);
    assert!(apply_delusion3_spiral_key_trap(&mut flags, &mut inv));
    assert!(flags.get(FLAG_DELUSION3_BUG_DOOR_LOCKED) && flags.get(FLAG_FLASHLIGHT_BATTERY_DEAD));
    assert!(
        apply_dry_cell_battery(&mut flags, &mut inv) && !flags.get(FLAG_FLASHLIGHT_BATTERY_DEAD)
    );

    let lit = RoachKeypadState::highlighted_buttons(&puzzle);
    let mut keypad = RoachKeypadState::new();
    let _ = keypad.press_button(lit[2], &puzzle, &mut flags);
    let _ = keypad.press_button(lit[1], &puzzle, &mut flags);
    let rej = RoachKeypadOutcome::Rejected {
        se_id: SE_ROACH_KEYPAD_ERROR,
    };
    assert_eq!(keypad.press_button(lit[0], &puzzle, &mut flags), rej);
    let _ = keypad.press_button(lit[0], &puzzle, &mut flags);
    let _ = keypad.press_button(lit[1], &puzzle, &mut flags);
    let unl = RoachKeypadOutcome::Unlocked {
        se_id: SE_ROACH_KEYPAD_UNLOCK,
    };
    assert_eq!(keypad.press_button(lit[2], &puzzle, &mut flags), unl);
    assert!(!flags.get(FLAG_DELUSION3_BUG_DOOR_LOCKED));

    inv.give_raw(ITEM_TABLET_OPPRESSOR, 1);
    inv.give_raw(ITEM_TABLET_GLUTTONOUS_PIG, 1);
    inv.give_raw(ITEM_TABLET_SEDUCTRESS, 1);
    let r1 = apply_prison_scaffold_tablet(89, &mut flags, &mut inv).unwrap();
    assert_eq!((r1.newly_mounted_count, r1.puzzle_completed), (2, false));
    let r2 = apply_prison_scaffold_tablet(ITEM_TABLET_SEDUCTRESS, &mut flags, &mut inv).unwrap();
    assert!(r2.puzzle_completed && r2.completion_se_id == Some(SE_SCAFFOLD_GALLOWS_SCREAM));
    assert!(flags.get(FLAG_SCAFFOLD_ALL_SET));

    inv.give_raw(ITEM_HORSESHOE, 1);
    inv.give_raw(ITEM_LIGHTER, 1);
    inv.give_raw(ITEM_WAX_DOLL, 1);
    let hint = try_open_prison_trapdoor(95, &mut flags, &mut inv);
    assert!(matches!(hint, TrapdoorUseOutcome::IncompleteHint { .. }));
    inv.give_raw(ITEM_COMBO_TRAPDOOR_HANDLE, 1);
    let opened = try_open_prison_trapdoor(ITEM_COMBO_TRAPDOOR_HANDLE, &mut flags, &mut inv);
    assert!(matches!(opened, TrapdoorUseOutcome::Opened { .. }));
    assert!(flags.get(FLAG_PRISON_TRAPDOOR_OPENED));
    assert_eq!(effective_prisoner_step_threshold(45, 60, false), 45);
    assert_eq!(effective_prisoner_step_threshold(45, 60, true), 90);
}

#[test]
fn labyrinth_face_cube_wire_cutter_and_hangman_across_all_difficulties() {
    let (mut flags, mut puzzle, mut rng) = (EventFlags::new(), PuzzleState::new(), Sh2Rng::new(99));
    init_new_game(
        &mut flags,
        &mut puzzle,
        &mut rng,
        RiddleDifficulty::Hard,
        ActionDifficulty::Normal,
        ClearData::default(),
    );

    let mut cur = sync_labyrinth_face_cube(&mut flags);
    while cur.plane != 0 {
        cur = rotate_labyrinth_face_cube(&mut flags, CubeTurnDirection::Up);
    }
    while cur.roll != 0 && cur.roll != 2 {
        cur = rotate_labyrinth_face_cube(&mut flags, CubeTurnDirection::Right);
    }
    assert_eq!(cur.connectivity_level, 2);
    assert!(flags.get(FLAG_CUBE_PASSAGE_OPEN) && flags.get(FLAG_CUBE_MARIA_PATH_OPEN));
    let blocked = rotate_labyrinth_face_cube(&mut flags, CubeTurnDirection::Down);
    assert!(blocked.plane == 3 && !flags.get(FLAG_CUBE_PASSAGE_OPEN));

    let mut inv = InventoryManager::new();
    inv.give_raw(ITEM_WIRE_CUTTER, 1);
    assert!(apply_labyrinth_wire_cutter(&mut flags, &mut inv));
    assert!(flags.get(FLAG_LABYRINTH_WIRE_CUT) && flags.get(FLAG_LABYRINTH_SPARK_DEACTIVATED));

    let diffs = [
        RiddleDifficulty::Easy,
        RiddleDifficulty::Normal,
        RiddleDifficulty::Hard,
        RiddleDifficulty::Extra,
    ];
    for (idx, diff) in diffs.into_iter().enumerate() {
        let (mut f, mut p, mut r) = (
            EventFlags::new(),
            PuzzleState::new(),
            Sh2Rng::new(1000 + idx as u32),
        );
        let clear = ClearData {
            cleared_levels: ClearData::ALL_BASE_RIDDLES_CLEARED,
            endings_seen: 1,
            clear_count: 2,
        };
        let eff = init_new_game(
            &mut f,
            &mut p,
            &mut r,
            diff,
            ActionDifficulty::Normal,
            clear,
        );
        let (_, poem_msg) = read_hangman_poem_hint(eff, &mut f);
        assert!((9..=12).contains(&poem_msg));
        let innocent = hangman_innocent_slot(&p).expect("innocent digit 5 exists");
        let wrong = (innocent + 1) % 6;
        assert!(inspect_hanged_corpse_message(innocent, eff, &p, &mut f).is_some());
        assert!(matches!(
            pull_hangman_noose(wrong, &p, &mut f),
            HangmanNooseOutcome::WrongGuilty { .. }
        ));
        assert!(on_enter_hangman_corpse_room(&mut f) && f.get(FLAG_HANGMAN_WRONG_ROPE_AMBUSH));
        assert!(matches!(
            pull_hangman_noose(innocent, &p, &mut f),
            HangmanNooseOutcome::CorrectInnocent { .. }
        ));
        assert!(!on_enter_hangman_corpse_room(&mut f));
    }

    inv.give_raw(ITEM_KEY_OF_PERSECUTED, 1);
    assert!(apply_key_of_persecuted(&mut flags, &mut inv).is_some());
    assert!(flags.get(FLAG_HANGMAN_HANDCUFFS_UNLOCKED));
}

#[test]
fn hotel_normal_music_boxes_cupboard_briefcase_and_room312_vcr() {
    let (mut flags, mut inv) = (EventFlags::new(), InventoryManager::new());

    inv.give_raw(ITEM_CAN_OPENER, 1);
    inv.give_raw(ITEM_BAR_KEY, 1);
    assert!(!apply_bar_key(&mut flags, &mut inv));
    assert!(apply_bar_can_opener(&mut flags, &mut inv) && inv.has_item(ITEM_LIGHTBULB));
    assert!(apply_bar_lightbulb(&mut flags, &mut inv) && apply_bar_key(&mut flags, &mut inv));
    assert!(flags.get(FLAG_BAR_DOOR_UNLOCKED));

    inv.give_raw(ITEM_FISH_KEY, 1);
    assert!(apply_cloakroom_fish_key(&mut flags, &mut inv) && inv.has_item(ITEM_HOTEL_204_KEY));

    for (word_idx, &expected_word) in BRIEFCASE_TRUNK_WORDS.iter().enumerate() {
        let (mut bf, mut bp, mut brng, mut binv) = (
            EventFlags::new(),
            PuzzleState::new(),
            Sh2Rng::new(42 + word_idx as u32),
            InventoryManager::new(),
        );
        for b in 0..5 {
            bf.set(
                FLAG_BRIEFCASE_WORD_INDEX_BASE + b,
                ((word_idx >> b) & 1) != 0,
            );
        }
        binv.give_raw(ITEM_THINNER, 1);
        assert_eq!(
            apply_room202_thinner(&mut bf, &mut binv),
            Some(expected_word)
        );
        init_briefcase_drums_if_needed(&mut bf, &mut bp, &mut brng);
        for (w, &ch) in expected_word.as_bytes().iter().enumerate() {
            while BRIEFCASE_TRUNK_DRUMS[w][(bp.raw()[PZ_DRUM + w] & 7) as usize] != ch {
                let _ = turn_briefcase_drum(w, 1, &mut bf, &mut bp, &mut binv);
            }
        }
        assert!(bf.get(FLAG_BRIEFCASE_UNLOCKED) && binv.has_item(ITEM_MUSIC_BOX_CINDERELLA));
    }

    inv.give_raw(ITEM_MUSIC_BOX_CINDERELLA, 1);
    inv.give_raw(ITEM_MUSIC_BOX_LITTLE_MERMAID, 1);
    inv.give_raw(ITEM_MUSIC_BOX_SNOW_WHITE, 1);
    let mut orgel = HotelMusicBoxPuzzle::new();
    orgel.place_box(0, MusicBoxKind::SnowWhite, &mut flags, &mut inv);
    orgel.place_box(1, MusicBoxKind::LittleMermaid, &mut flags, &mut inv);
    orgel.place_box(2, MusicBoxKind::Cinderella, &mut flags, &mut inv);
    let fail = orgel.play_melody(RiddleDifficulty::Normal, false, &mut flags, &mut inv);
    assert!(!fail.solved);
    orgel.return_boxes_to_inventory(&mut flags, &mut inv);
    orgel.place_box(0, MusicBoxKind::SnowWhite, &mut flags, &mut inv);
    orgel.place_box(1, MusicBoxKind::LittleMermaid, &mut flags, &mut inv);
    orgel.place_box(2, MusicBoxKind::Cinderella, &mut flags, &mut inv);
    let win = orgel.play_melody(RiddleDifficulty::Hard, true, &mut flags, &mut inv);
    assert!(win.solved && win.hotel_2f_bgm_bank == 4 && inv.has_item(ITEM_HOTEL_STAIRWAY_KEY));

    inv.give_raw(5, 140);
    inv.give_raw(1, 7);
    inv.give_raw(14, 1);
    assert_eq!(
        try_board_employee_elevator(&mut flags, &inv),
        Err(SE_ELEVATOR_OVERWEIGHT)
    );
    let mut cupboard = ElevatorCupboardState::new();
    assert!(cupboard.deposit_all(&mut flags, &mut inv) >= 3);
    assert_eq!(try_board_employee_elevator(&mut flags, &inv), Ok(()));
    assert!(cupboard.withdraw_all(&mut flags, &mut inv) >= 3);
    assert_eq!((inv.count(5), inv.count(1)), (140, 7));

    inv.give_raw(ITEM_VIDEOTAPE, 1);
    let vcr = trigger_room312_vcr(true, &mut flags, &mut inv).unwrap();
    assert!(vcr.audio_track_index == 1 && flags.get(FLAG_HOTEL_TRANSITIONED_TO_OTHERWORLD));
    assert!(can_toggle_flashlight_in_room312(true, true, false));
    assert!(!can_toggle_flashlight_in_room312(true, true, true));
}

#[test]
fn hotel_otherworld_and_born_from_a_wish_gravestone_and_mansion_triggers() {
    let (mut flags, mut puzzle) = (EventFlags::new(), PuzzleState::new());
    init_new_game_wish(&mut flags, &mut puzzle);
    assert!(flags.get(5) && flags.get(24));

    let mut inv = InventoryManager::new();
    inv.give_raw(ITEM_SCARLET_EGG, 1);
    inv.give_raw(ITEM_RUST_COLORED_EGG, 1);
    let e1 =
        apply_otherworld_hotel_egg(ITEM_RUST_COLORED_EGG, false, &mut flags, &mut inv).unwrap();
    assert!(
        !e1.0
            && apply_otherworld_hotel_egg(ITEM_SCARLET_EGG, false, &mut flags, &mut inv).is_none()
    );
    let e2 = apply_otherworld_hotel_egg(ITEM_SCARLET_EGG, true, &mut flags, &mut inv).unwrap();
    assert!(
        e2.0 && flags.get(FLAG_EGG_RUST_PLACED_LEFT) && flags.get(FLAG_EGG_SCARLET_PLACED_RIGHT)
    );

    assert!(
        trigger_reading_room_headphones(&mut flags) && !trigger_reading_room_headphones(&mut flags)
    );
    assert!(is_hotel_2f_teleport_corridor_room(0x29) && !is_hotel_2f_teleport_corridor_room(0x2a));
    let teleporting: Vec<u8> = (0x29..=0x39)
        .filter(|&r| is_hotel_2f_teleport_corridor_room(r))
        .collect();
    assert_eq!(teleporting, [0x29, 0x2d, 0x31, 0x39]);

    inv.give_raw(ITEM_DOG_KEY, 1);
    assert!(apply_hotel_dog_key(&mut flags, &mut inv) && flags.get(FLAG_DOG_KEY_DOOR_UNLOCKED));

    inv.give_raw(ITEM_BLACK_BOARD, 1);
    inv.give_raw(ITEM_WHITE_BOARD, 1);
    inv.give_raw(ITEM_RED_BOARD, 1);
    let mut grave = WishGravestonePuzzle::new();
    grave.place_board(
        GravestoneBoard::Black,
        BoardRotation::Deg0,
        &mut flags,
        &mut inv,
    );
    grave.place_board(
        GravestoneBoard::White,
        BoardRotation::Deg0,
        &mut flags,
        &mut inv,
    );
    let bad = grave
        .place_board(
            GravestoneBoard::Red,
            BoardRotation::Deg0,
            &mut flags,
            &mut inv,
        )
        .unwrap();
    assert!(!bad.0 && grave.pop_board(&mut flags, &mut inv).unwrap().0 == GravestoneBoard::Red);
    let ok = grave
        .place_board(
            GravestoneBoard::Red,
            BoardRotation::Left90,
            &mut flags,
            &mut inv,
        )
        .unwrap();
    assert!(ok.0 && flags.get(FLAG_WISH_GRAVESTONE_SOLVED) && inv.has_item(ITEM_ACACIA_KEY));
    assert!(
        apply_wish_acacia_key(&mut flags, &mut inv) && flags.get(FLAG_WISH_ACACIA_DOOR_UNLOCKED)
    );

    inv.give_raw(ITEM_MATCHES, 1);
    assert!(apply_wish_matches_to_candle(&mut flags, &mut inv) && flags.get(FLAG_WISH_CANDLE_LIT));
    let (uv_van, uv_fix) = (
        gravestone_board_uv(GravestoneBoard::White, false),
        gravestone_board_uv(GravestoneBoard::White, true),
    );
    assert_ne!(uv_van.u1, uv_fix.u1);
}

#[test]
fn retail_trg1_always_events_fire_mid_room_and_gravestone_verts_toggle() {
    // Retail `trg == 0` and `trg == 1` (`EventCheck` `0x0048e1db`) both fire every frame (`Always`).
    let raw0 = EventListEntry {
        flag: 0,
        cond: 0x0000_0000,
        rslt0: 0x1000_0000,
        rslt1: 0,
    };
    let raw = EventListEntry {
        flag: 0,
        cond: 0x1000_0000,
        rslt0: 0x1000_0000,
        rslt1: 0,
    };
    assert_eq!(
        normalize_retail_event_entry(raw0).trigger_kind(),
        Some(EventTriggerKind::Always)
    );
    let norm = normalize_retail_event_entry(raw);
    assert_eq!(norm.trigger_kind(), Some(EventTriggerKind::Always));
    let eval = EventEvalInput {
        player_pos: [0.0; 3],
        player_rot_y: 0.0,
        ev_active: 0,
        action_pressed: false,
        map_disabled_pressed: false,
        room_changed: false,
        stage_changed: false,
        used_item_id: None,
        is_demo_mode: false,
        action_difficulty: ActionDifficulty::Normal,
        player_can_search: true,
    };
    let mut ts = EventTriggerState::new();
    assert!(evaluate_event_entry(0, &norm, &[], &EventFlags::new(), &eval, &mut ts) > 0.0);
    let s53 = stage_table_act2(53).expect("stage 53");
    let s53_ev0 = normalize_retail_event_entry(s53.event_list[0]);
    if (s53.event_list[0].cond >> 28) & 0xf == 1 {
        assert_eq!(s53_ev0.trigger_kind(), Some(EventTriggerKind::Always));
    }

    // Gravestone quad vertices: vanilla `0x008c4670` orderings vs SH2:EE `GravestoneBoardsFix`.
    let [x0, y0, x1, y1] = GRAVESTONE_BOARD_VERT_EXTENTS;
    assert_eq!([x0, y0, x1, y1], [-1285, -2110, 1275, 1218]);
    let (v0_fix, v0_van) = (
        gravestone_board_verts(BoardRotation::Deg0, true),
        gravestone_board_verts(BoardRotation::Deg0, false),
    );
    assert_eq!(v0_fix, v0_van);
    assert_eq!(
        gravestone_board_verts(BoardRotation::Left90, false),
        [[x0, y1], [x0, y0], [x1, y1], [x1, y0]]
    );
    assert_eq!(
        gravestone_board_verts(BoardRotation::Right90, true)[0],
        [1355, -2030]
    );
    assert_eq!(
        gravestone_board_verts(BoardRotation::Deg180, true)[3],
        [-1267, -2066]
    );
    assert_eq!(
        gravestone_board_verts(BoardRotation::Left90, true)[1],
        [-1353, -2005]
    );
}
