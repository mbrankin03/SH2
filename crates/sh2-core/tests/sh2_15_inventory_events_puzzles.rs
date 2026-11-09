//! Comprehensive integration & unit tests for `SH2-15`:
//! Inventory, Stage Event Engine (`ev_prog`), Puzzle Logic & Pre-rendered Cutaway Subsystem.

use sh2_core::{
    combine_items, evaluate_baldwin_tombstone_boards, format_carbon_box_code,
    format_wallet_safe_code, init_new_game, init_new_game_wish, is_clock_solved,
    is_coin_desk_solved, item_metadata, original_sheet_dimensions, place_baldwin_board,
    place_coin_in_desk, step_clock_needles, step_safe_dial, try_open_safe, use_key_on_hospital_box,
    verify_hospital_box_keypad, verify_hospital_box_wheels, verify_hospital_corridor_keypad,
    ActionDifficulty, ClearData, EventFlags, InventoryManager, ItemCategory, MessageInsertState,
    OverlayKind, PuzzleState, RiddleDifficulty, ScenarioKind, ScenarioState, Sh2Rng,
    StageProgramRunner, AMPOULE_STAMINA_DURATION_SECS, CATALOG_ITEM_COUNT, EV_MAX_FLAGS,
    ITEM_CATALOG, ITEM_COMBOS, MAX_ITEM_STACK, PROG_KEY_A, PROG_KEY_LEFT, PZ_BOX_CODE, PZ_CLOCK,
    PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL,
};

#[test]
fn test_full_85_item_catalog_and_metadata() {
    assert_eq!(ITEM_CATALOG.len(), CATALOG_ITEM_COUNT);
    for (idx, meta) in ITEM_CATALOG.iter().enumerate() {
        let expected_id = (idx + 1) as u8;
        assert_eq!(meta.id, expected_id);
        assert!(!meta.name.is_empty(), "Item {expected_id} has empty name");
        assert!(
            !meta.description.is_empty(),
            "Item {expected_id} has empty description"
        );
        assert_eq!(item_metadata(expected_id), Some(meta));
    }
    assert_eq!(item_metadata(0), None);
    assert_eq!(item_metadata(86), None);

    // Spot-check key items, weapons, 3D models, 2D examination textures, and Born from a Wish items
    let handgun = item_metadata(4).unwrap();
    assert_eq!(handgun.name, "Handgun");
    assert_eq!(handgun.category, ItemCategory::Weapon);
    assert_eq!(handgun.model_path, Some("data/chr/item/x_handgun.mdl"));

    let mary_letter = item_metadata(21).unwrap();
    assert_eq!(mary_letter.name, "Letter from Mary");
    assert_eq!(
        mary_letter.inspect_texture_path,
        Some("pic/item/p_letterm.tex")
    );
    assert!(mary_letter
        .examine_text
        .unwrap()
        .contains("In my restless dreams"));

    let white_board = item_metadata(79).unwrap();
    assert_eq!(white_board.name, "White Board");
    assert_eq!(white_board.category, ItemCategory::SubScenarioItem);
    assert_eq!(
        white_board.model_path,
        Some("data/chr2/item/x_houban_white.mdl")
    );
    assert_eq!(
        white_board.inspect_texture_path,
        Some("pic/add/xwhitehouban.tex")
    );
}

#[test]
fn test_inventory_limits_bullet_adjust_and_secret_items() {
    let mut inv = InventoryManager::new();
    assert_eq!(inv.bullet_adjust(), 1);

    // Handgun bullets (id 5): base 10 * bullet_adjust
    assert_eq!(inv.item_get_now(5), 10);
    inv.set_bullet_adjust(3);
    assert_eq!(inv.item_get_now(5), 40); // 10 + 30

    // Shotgun shells (id 7): base 6 * 3 = 18
    assert_eq!(inv.item_get_now(7), 18);

    // Rifle shells (id 9): base 4 * 3 = 12
    assert_eq!(inv.item_get_now(9), 12);

    // Revolver bullets (id 11): base 6 * 3 = 18
    assert_eq!(inv.item_get_now(11), 18);

    // Stack limit capped at 999
    for _ in 0..40 {
        inv.item_get_now(5);
    }
    assert_eq!(inv.count(5), MAX_ITEM_STACK);

    // Secret items bitmask (8 secret items) vs standard pickup counter
    let before_total = inv.total_items_picked_up();
    for &secret_id in &[0x10u8, 0x0c, 0x19, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e] {
        inv.item_get_now(secret_id);
    }
    assert_eq!(inv.secret_items_mask(), 0xff);
    assert_eq!(inv.secret_items_count(), 8);
    // Secret items do not increment standard item pickup count
    assert_eq!(inv.total_items_picked_up(), before_total);

    // Mary's photo (20) and letter (21) do not increment standard item pickup count
    inv.item_get_now(20);
    inv.item_get_now(21);
    assert_eq!(inv.total_items_picked_up(), before_total);
}

#[test]
fn test_item_combinations_and_part_consumption() {
    assert_eq!(ITEM_COMBOS.len(), 11);

    // Order-independent 2-item and 3-item combinations
    assert_eq!(combine_items(35, 36, 0), Some(86));
    assert_eq!(combine_items(36, 0, 35), Some(86));
    assert_eq!(combine_items(53, 54, 0), Some(87));
    assert_eq!(combine_items(56, 57, 0), Some(88));
    assert_eq!(combine_items(59, 60, 61), Some(92));
    assert_eq!(combine_items(64, 62, 63), Some(96));
    assert_eq!(combine_items(35, 35, 0), None);

    // Incomplete wax doll / horseshoe / lighter pairs (93..=95) do not consume parts on use,
    // whereas full 3-item combination (96) consumes all three parts (62, 63, 64).
    let mut inv = InventoryManager::new();
    inv.item_get_now(62);
    inv.item_get_now(63);
    inv.item_get_now(64);
    assert_eq!(inv.try_combine_owned(62, 63, 0), Some(93));
    inv.item_lose(93);
    assert!(inv.has_item(62) && inv.has_item(63) && inv.has_item(64));

    assert_eq!(inv.try_combine_owned(62, 63, 64), Some(96));
    inv.item_lose(96);
    assert!(!inv.has_item(62) && !inv.has_item(63) && !inv.has_item(64));
}

#[test]
fn test_healing_items_and_examination_scoring() {
    let mut inv = InventoryManager::new();
    inv.item_get_now(1); // Health drink (+25%)
    inv.item_get_now(2); // First-aid kit (+50%)
    inv.item_get_now(3); // Ampoule (+100% + 600s stamina)
    inv.item_get_now(20); // Photo of Mary
    inv.item_get_now(24); // Angela's knife

    let r1 = inv.use_healing_item(1, 50.0, 100.0).unwrap();
    assert!((r1.new_hp - 75.0).abs() < 1e-5);
    assert!((r1.overheal_fraction - 0.0).abs() < 1e-5);
    assert!(!r1.activated_ampoule_stamina);

    // First-aid kit from 75/100 -> 100/100 with 0.25 overheal fraction
    let r2 = inv.use_healing_item(2, r1.new_hp, 100.0).unwrap();
    assert!((r2.new_hp - 100.0).abs() < 1e-5);
    assert!((r2.overheal_fraction - 0.25).abs() < 1e-5);

    // Ampoule at full HP -> 1.0 overheal fraction + 600s stamina timer
    let r3 = inv.use_healing_item(3, r2.new_hp, 100.0).unwrap();
    assert!((r3.new_hp - 100.0).abs() < 1e-5);
    assert!((r3.overheal_fraction - 1.0).abs() < 1e-5);
    assert!(r3.activated_ampoule_stamina);
    assert!((inv.ampoule_stamina_timer() - AMPOULE_STAMINA_DURATION_SECS).abs() < 1e-5);
    assert!((inv.overheal_score() - 1.25).abs() < 1e-5);

    // Examining Mary's photo/letter caps at 2; examining Angela's knife flags 630
    for _ in 0..5 {
        let (_, flag_630) = inv.examine_item(20).unwrap();
        assert!(!flag_630);
    }
    assert_eq!(inv.mary_examine_count(), 2);

    let (_, knife_flag) = inv.examine_item(24).unwrap();
    assert!(knife_flag);
}

#[test]
fn test_flag_init_randomization_and_extra_riddle_unlock() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut rng = Sh2Rng::new(0x1234_5678);

    // Test Easy riddle pairing on Hospital wheel goals
    let eff = init_new_game(
        &mut flags,
        &mut puzzle,
        &mut rng,
        RiddleDifficulty::Easy,
        ActionDifficulty::Normal,
        ClearData::default(),
    );
    assert_eq!(eff, RiddleDifficulty::Easy);
    assert!(flags.get(13));
    assert!(!is_clock_solved(&puzzle)); // Clock never starts inside the 9:10 window
    assert_eq!(puzzle.raw()[PZ_WHEEL_GOAL], puzzle.raw()[PZ_WHEEL_GOAL + 2]);
    assert_eq!(
        puzzle.raw()[PZ_WHEEL_GOAL + 1],
        puzzle.raw()[PZ_WHEEL_GOAL + 3]
    );
    assert_ne!(puzzle.raw()[PZ_WHEEL_GOAL], puzzle.raw()[PZ_WHEEL_GOAL + 1]);

    // Safe code never has two identical adjacent numbers
    for i in 1..4 {
        assert_ne!(
            puzzle.raw()[PZ_SAFE_CODE + i],
            puzzle.raw()[PZ_SAFE_CODE + i - 1]
        );
    }

    // Extra riddle unlocks once Easy, Normal, and Hard riddle levels have all been cleared (`0x48aed0`)
    let mut clear = ClearData::default();
    clear.record_ending(
        ClearData::ENDING_LEAVE,
        RiddleDifficulty::Easy,
        ActionDifficulty::Normal,
    );
    clear.record_ending(
        ClearData::ENDING_IN_WATER,
        RiddleDifficulty::Normal,
        ActionDifficulty::Normal,
    );
    assert!(!clear.is_extra_riddle_unlocked());
    clear.record_ending(
        ClearData::ENDING_MARIA,
        RiddleDifficulty::Hard,
        ActionDifficulty::Normal,
    );
    assert_eq!(
        clear.cleared_levels & ClearData::ALL_BASE_RIDDLES_CLEARED,
        ClearData::ALL_BASE_RIDDLES_CLEARED
    );
    assert!(clear.is_extra_riddle_unlocked());

    let eff_extra = init_new_game(
        &mut flags,
        &mut puzzle,
        &mut rng,
        RiddleDifficulty::Hard,
        ActionDifficulty::Normal,
        clear,
    );
    assert_eq!(eff_extra, RiddleDifficulty::Extra);

    // Once Extra riddle itself has been cleared (`RIDDLE_EXTRA_CLEARED`), `is_extra_riddle_unlocked`
    // alternates with `clear_count` parity so the player can still play standard Hard riddle.
    clear.record_ending(
        ClearData::ENDING_REBIRTH,
        RiddleDifficulty::Extra,
        ActionDifficulty::Normal,
    );
    assert_eq!(clear.clear_count, 4);
    assert!(clear.is_extra_riddle_unlocked()); // even clear_count -> Extra
    clear.record_ending(
        ClearData::ENDING_LEAVE,
        RiddleDifficulty::Extra,
        ActionDifficulty::Normal,
    );
    assert_eq!(clear.clear_count, 5);
    assert!(!clear.is_extra_riddle_unlocked()); // odd clear_count -> standard Hard

    // Born from a Wish initialization sets flags 5 and 24
    init_new_game_wish(&mut flags, &mut puzzle);
    assert!(flags.get(5) && flags.get(24));
    assert!(!flags.get(EV_MAX_FLAGS - 1));
}

#[test]
fn test_apartment_clock_coin_desk_and_safe_puzzles() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();

    // 1. Room 208 Clock Puzzle: 9:10 = 550 minutes = 550 * 64 = 35200 (0x8980)
    puzzle.write_u16(PZ_CLOCK, 0x8970);
    let mut accel = 0.0;
    for _ in 0..16 {
        step_clock_needles(&mut puzzle, PROG_KEY_LEFT, 0.016, &mut accel);
    }
    assert_eq!(puzzle.read_u16(PZ_CLOCK), 0x8980);
    assert!(is_clock_solved(&puzzle));

    // 2. Room 105 Coin Desk Puzzle across all 4 difficulties
    // Easy: [128 (hole 2 Snake=50), 131 (hole 0 Old Man=51), 140 (hole 4 Prisoner=52)]
    inv.item_get_now(50);
    inv.item_get_now(51);
    inv.item_get_now(52);
    assert_eq!(
        place_coin_in_desk(&mut flags, &mut inv, RiddleDifficulty::Easy, 0, 51),
        Some(false)
    );
    assert_eq!(
        place_coin_in_desk(&mut flags, &mut inv, RiddleDifficulty::Easy, 2, 50),
        Some(false)
    );
    assert_eq!(
        place_coin_in_desk(&mut flags, &mut inv, RiddleDifficulty::Easy, 4, 52),
        Some(true)
    );
    assert!(flags.get(142) && flags.get(112) && inv.has_item(31)); // Lyne house key awarded

    // Wrong coin order returns the 3 coins to inventory
    flags.reset();
    inv.reset();
    inv.item_get_now(50);
    inv.item_get_now(51);
    inv.item_get_now(52);
    assert_eq!(
        place_coin_in_desk(&mut flags, &mut inv, RiddleDifficulty::Normal, 0, 50),
        Some(false)
    );
    assert_eq!(
        place_coin_in_desk(&mut flags, &mut inv, RiddleDifficulty::Normal, 1, 51),
        Some(false)
    );
    assert_eq!(
        place_coin_in_desk(&mut flags, &mut inv, RiddleDifficulty::Normal, 2, 52),
        Some(false)
    );
    assert!(!flags.get(142));
    assert!(inv.has_item(50) && inv.has_item(51) && inv.has_item(52));

    // Check Normal, Hard, Extra coin flag patterns directly
    for (diff, sol) in [
        (RiddleDifficulty::Normal, [130, 132, 138]),
        (RiddleDifficulty::Hard, [129, 132, 140]),
        (RiddleDifficulty::Extra, [128, 131, 139]),
    ] {
        flags.reset();
        for f in sol {
            flags.set(f, true);
        }
        assert!(is_coin_desk_solved(&flags, diff));
    }

    // 3. Room 203 Wallet Safe Code Formatting & Safe Dial Rotation
    puzzle.raw_mut()[PZ_SAFE_CODE..PZ_SAFE_CODE + 4].copy_from_slice(&[9, 19, 0, 14]); // 10, 20, 1, 15
    let mut inserts = MessageInsertState::default();
    format_wallet_safe_code(&puzzle, RiddleDifficulty::Normal, &mut inserts);
    assert_eq!(inserts.expand("<e0>>><e1><<<e2>>><e3>"), "10>>20<<01>>15");
    format_wallet_safe_code(&puzzle, RiddleDifficulty::Hard, &mut inserts);
    assert_eq!(inserts.expand("<e0>>><e1><<<e2>>><e3>"), "X>>VV<<I>>XV");
    format_wallet_safe_code(&puzzle, RiddleDifficulty::Extra, &mut inserts);
    assert_eq!(inserts.expand("<e0>>><e1><<<e2>>><e3>"), "a>>k<<1>>f");

    // Safe dial step: set code [2, 1, 2, 1], start dial at 3
    flags.reset();
    puzzle.raw_mut()[PZ_SAFE_CODE..PZ_SAFE_CODE + 4].copy_from_slice(&[2, 1, 2, 1]);
    puzzle.raw_mut()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4].fill(0xff);
    for b in 0..5 {
        flags.set(103 + b, ((3 >> b) & 1) != 0);
    }
    // Step right (3 -> 2) hits code[0] = 2
    assert_eq!(step_safe_dial(&mut flags, &mut puzzle, true), 2);
    // Step 1: want_dir = -1 (turn right: 2 -> 1) hits code[1] = 1
    assert_eq!(step_safe_dial(&mut flags, &mut puzzle, true), 1);
    // Step 2: want_dir = +1 (turn left: 1 -> 2) hits code[2] = 2
    assert_eq!(step_safe_dial(&mut flags, &mut puzzle, false), 2);
    // Step 3: want_dir = -1 (turn right: 2 -> 1) hits code[3] = 1
    assert_eq!(step_safe_dial(&mut flags, &mut puzzle, true), 1);

    inv.reset();
    assert!(try_open_safe(&mut flags, &puzzle, &mut inv));
    assert!(flags.get(108));
    assert_eq!(inv.count(5), 40); // 4 boxes of Handgun bullets
}

#[test]
fn test_hospital_and_born_from_a_wish_puzzles() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut rng = Sh2Rng::new(99);

    // 1. Hospital 3F Corridor Keypad
    assert!(!verify_hospital_corridor_keypad(
        &mut flags,
        RiddleDifficulty::Normal,
        1328
    ));
    assert!(verify_hospital_corridor_keypad(
        &mut flags,
        RiddleDifficulty::Normal,
        7335
    ));
    assert!(flags.get(181));

    flags.reset();
    assert!(verify_hospital_corridor_keypad(
        &mut flags,
        RiddleDifficulty::Hard,
        1328
    ));
    assert!(flags.get(181));

    // 2. Hospital 2F Locked Box (4 locks: 2 keys, 4 wheels requiring flag 203, 4-digit keypad)
    flags.reset();
    puzzle.write_u16(PZ_BOX_CODE, 0x4829);
    let mut inserts = MessageInsertState::default();
    assert_eq!(format_carbon_box_code(&puzzle, &mut inserts), "4829");
    assert!(verify_hospital_box_keypad(
        &mut flags,
        &puzzle,
        [4, 8, 2, 9]
    ));
    assert!(flags.get(198));

    puzzle.raw_mut()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4].copy_from_slice(&[2, 5, 2, 5]);
    puzzle.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&[2, 5, 2, 5]);
    // Without flag 203 (wall blood symbols read), wheel check fails and scrambles one wheel
    assert!(!verify_hospital_box_wheels(
        &mut flags,
        &mut puzzle,
        &mut rng
    ));
    assert!(!flags.get(197));
    assert_ne!(
        &puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4],
        &puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
    );

    // With flag 203 set, matching wheels unlock flag 197
    flags.set(203, true);
    puzzle.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&[2, 5, 2, 5]);
    assert!(verify_hospital_box_wheels(
        &mut flags,
        &mut puzzle,
        &mut rng
    ));
    assert!(flags.get(197));

    // Use combined Purple Bull + Lapis Eye keys (86) to unlock remaining chains and award Piece of hair (53)
    inv.item_get_now(35);
    inv.item_get_now(36);
    assert!(use_key_on_hospital_box(&mut flags, &mut inv, 86));
    assert!(flags.get(199));
    assert!(inv.has_item(53)); // Piece of hair

    // 3. Born from a Wish Baldwin Mansion Tombstone Board Puzzle
    inv.item_get_now(79); // White Board
    inv.item_get_now(80); // Black Board
    inv.item_get_now(81); // Red Board
    assert!(place_baldwin_board(&mut flags, &mut inv, 79, 2)); // White: rot 2 (even -> valid)
    assert!(place_baldwin_board(&mut flags, &mut inv, 80, 0)); // Black: rot 0 (valid)
    assert!(place_baldwin_board(&mut flags, &mut inv, 81, 1)); // Red: rot 1 (valid)
    assert!(evaluate_baldwin_tombstone_boards(&mut flags, &mut inv));
    assert!(flags.get(704));
    assert!(inv.has_item(82)); // Acacia key awarded
}

#[test]
fn test_cutaway_overlay_engine_and_car_map_pickup_program() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
    assert!(state.inventory_mgr.has_item(20));
    assert!(state.inventory_mgr.has_item(21));

    // Run Stage 3 #1: Observation deck car map pickup (s03_item_in_car, completion flag 28)
    state.prog_runner.start(3, 1, 28);
    assert!(state.prog_runner.is_running());

    // Step 0 -> 1: opens out_p_incar with out/p_incar_map at alpha 1.0, locks player
    assert!(state.prog_runner.tick(
        0,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert!(state.prog_runner.is_player_locked());
    assert_eq!(state.cutaway_overlay.active_picture(), Some("out_p_incar"));
    assert_eq!(state.cutaway_overlay.kind(), OverlayKind::InCar);
    assert_eq!(state.cutaway_overlay.frame_sprites().len(), 1);
    assert!((state.cutaway_overlay.frame_sprites()[0].alpha - 1.0).abs() < 1e-5);

    // Press A -> triggers SE 0x2b17 and begins 1.0s map fade
    assert!(state.prog_runner.tick(
        PROG_KEY_A,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert_eq!(state.prog_runner.sound_effects(), &[0x2b17]);

    // Advance 0.5s -> alpha fades to 0.5
    assert!(state.prog_runner.tick(
        0,
        0,
        0.5,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert!((state.cutaway_overlay.incar_alpha() - 0.5).abs() < 1e-4);

    // Advance another 0.6s -> fade finishes (alpha 0.0), opens message 49 ("Got the map of Silent Hill.")
    assert!(state.prog_runner.tick(
        0,
        0,
        0.6,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert_eq!(state.prog_runner.active_message_id(), Some(49));
    assert_eq!(state.cutaway_overlay.frame_sprites().len(), 0);

    // Press A to close message -> hides picture, unlocks player, sets completion flag 28
    assert!(!state.prog_runner.tick(
        PROG_KEY_A,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert!(!state.prog_runner.is_player_locked_check_helper());
    assert!(state.has_flag(28));
    assert_eq!(state.cutaway_overlay.active_picture(), None);

    // Verify all interactive puzzle overlays emit valid sprite quads
    for (pic, expected_min_quads) in [
        ("apt_clock_close", 4usize),
        ("apt_clock_open", 4),
        ("apt_p_safe_close", 1),
        ("hsp_p_box", 15),
        ("hsp_p_boxnumber", 4),
        ("htl_p_trank", 12),
        ("add_p_teien", 1),
        ("htl_p_marker", 6),
    ] {
        state.cutaway_overlay.set_picture(Some(pic), &state.flags);
        let sprites = state
            .cutaway_overlay
            .evaluate_frame(&state.flags, &state.puzzle_state, 0, 0);
        assert!(
            sprites.len() >= expected_min_quads,
            "Overlay {pic} emitted {} sprites, expected >= {expected_min_quads}",
            sprites.len()
        );
        let (sw, sh) = original_sheet_dimensions(sprites[0].sheet_name);
        assert!(sw > 0 && sh > 0);
    }
}

trait RunnerLockExt {
    fn is_player_locked_check_helper(&self) -> bool;
}
impl RunnerLockExt for StageProgramRunner {
    fn is_player_locked_check_helper(&self) -> bool {
        self.is_player_locked()
    }
}
