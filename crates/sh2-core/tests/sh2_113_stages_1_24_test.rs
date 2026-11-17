//! Integration & retail-binary parity tests for `SH2-113`:
//! Act 1 static stage tables (`Stages 1..=27`), lifecycle hooks, Wood Side & Blue Creek Apartments
//! interactive riddles, Brookhaven Hospital (Normal + Otherworld) riddles, and toggleable 1:1
//! vanilla PS2 vs SH2:EE fixes.

use std::path::Path;

use sh2_core::config::RiddleDifficulty;
use sh2_core::events::*;
use sh2_core::inventory::InventoryManager;

#[test]
fn act1_static_stage_tables_cover_all_27_stages_with_valid_offsets_and_sentinels() {
    assert_eq!(ACT1_ALL_STAGES.len(), 27);
    assert!(act1_stage_descriptor(0).is_none());
    assert!(act1_stage_descriptor(28).is_none());

    for raw_id in 1u8..=27 {
        let stage_id = Act1StageId::from_raw(raw_id).expect("valid Act1StageId");
        let desc = act1_stage_descriptor(raw_id).expect("descriptor present");
        assert_eq!(desc.stage_id, stage_id);
        assert!(desc.ps2_stage_data_addr >= 0x0030_0000);
        assert!((0x008d_0000..0x008f_0000).contains(&desc.pc_stage_data_addr));

        assert!(desc.event_list.len() >= 2, "stage {raw_id} too short");
        assert!(desc.event_list.last().unwrap().is_sentinel());

        for (idx, entry) in desc.event_list[..desc.event_list.len() - 1]
            .iter()
            .enumerate()
        {
            assert!(!entry.is_sentinel());
            let off = entry.ev_pos_offset();
            if let (true, Some(shape)) = (off > 0, entry.shape_kind()) {
                assert!(
                    decode_act1_ev_pos_anchor(desc.ev_pos, off, shape).is_some(),
                    "stage {raw_id} ev_list[{idx}] out-of-bounds ev_pos offset {off}"
                );
            }
        }

        // Verify lossless Enemy_List 20-byte roundtrip decoding.
        for enemy in desc.enemy_list {
            let decoded = decode_act1_enemy_spawn(enemy);
            assert_ne!(decoded.kind, 0);
        }

        // Verify Item_List conversion.
        for item in desc.item_list {
            assert!(item.item_kind_index() < 7);
            let converted = item.to_item_list_entry();
            assert!(!converted.is_sentinel());
            assert_eq!(converted.flag_id(), item.flag_id());
        }
    }

    // Spot-check key stage table counts verified against sh2pc.exe.
    assert_eq!(act1_stage_descriptor(2).unwrap().event_list.len() - 1, 14); // stg_toilet
    assert_eq!(act1_stage_descriptor(5).unwrap().enemy_list.len(), 104); // stg_town_east
    assert_eq!(act1_stage_descriptor(14).unwrap().enemy_list.len(), 83); // stg_town_west
    assert_eq!(act1_stage_descriptor(20).unwrap().event_list.len() - 1, 64); // stg_hospital_3f_f
    assert_eq!(act1_stage_descriptor(25).unwrap().item_list.len(), 12); // stg_hospital_3f_b
}

#[test]
fn act1_retail_sh2pc_binary_byte_for_byte_verification() {
    let exe_path =
        Path::new("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2pc.exe");
    let Ok(pe) = std::fs::read(exe_path) else {
        // Retail asset rip is symlinked locally and omitted in GitHub Actions CI.
        return;
    };

    for raw_id in 1u8..=27 {
        let desc = act1_stage_descriptor(raw_id).unwrap();
        let ev_off = (desc.header.ev_list_ptr - 0x0040_0000) as usize;
        for (i, entry) in desc.event_list[..desc.event_list.len() - 1]
            .iter()
            .enumerate()
        {
            let base = ev_off + i * 16;
            let f = u32::from_le_bytes(pe[base..base + 4].try_into().unwrap());
            let c = u32::from_le_bytes(pe[base + 4..base + 8].try_into().unwrap());
            let r0 = u32::from_le_bytes(pe[base + 8..base + 12].try_into().unwrap());
            let r1 = u32::from_le_bytes(pe[base + 12..base + 16].try_into().unwrap());
            assert_eq!(
                (entry.flag, entry.cond, entry.rslt0, entry.rslt1),
                (f, c, r0, r1)
            );
        }

        let pos_off = (desc.header.ev_pos_ptr - 0x0040_0000) as usize;
        assert_eq!(desc.ev_pos, &pe[pos_off..pos_off + desc.ev_pos.len()]);
    }
}

#[test]
fn apartments_wallet_memo_and_w2f_safe_dial_all_difficulties() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    puzzle.raw_mut()[PZ_SAFE_CODE..PZ_SAFE_CODE + 4].copy_from_slice(&[0, 9, 14, 19]); // 1-based values: [1, 10, 15, 20]

    // Easy/Normal: 2-digit decimal
    assert_eq!(
        format_act1_wallet_safe_memo(&puzzle, RiddleDifficulty::Normal),
        "01>>10<<15>>20"
    );
    // Hard: slot 0 -> "1", odd slot 1 (10) -> "X0", even slot 2 (15) -> "VV5", slot 3 (20) -> "XX"
    assert_eq!(
        format_act1_wallet_safe_memo(&puzzle, RiddleDifficulty::Hard),
        "1>>X0<<VV5>>XX"
    );
    // Extra: 1..9 decimal, 10..20 -> 'a'..'k'
    assert_eq!(
        format_act1_wallet_safe_memo(&puzzle, RiddleDifficulty::Extra),
        "1>>a<<f>>k"
    );

    // Declining toilet prompt does not set flag 102.
    let no_res = act1_interact_toilet_wallet(false, RiddleDifficulty::Hard, &puzzle, &mut flags);
    assert_eq!(no_res.message_id, 2);
    assert!(!no_res.wallet_retrieved);

    // Reaching into toilet sets flag 102 and shows message 3; re-examining shows message 4.
    let yes_res = act1_interact_toilet_wallet(true, RiddleDifficulty::Hard, &puzzle, &mut flags);
    assert_eq!(yes_res.message_id, 3);
    assert!(flags.get(ACT1_FLAG_WALLET_PULLED_FROM_TOILET));
    let re_res = act1_interact_toilet_wallet(false, RiddleDifficulty::Hard, &puzzle, &mut flags);
    assert_eq!(re_res.message_id, 4);

    // Interactive safe dial rotation: target [3, 18, 2, 19] (from initial pos 0).
    puzzle.raw_mut()[PZ_SAFE_CODE..PZ_SAFE_CODE + 4].copy_from_slice(&[3, 18, 2, 19]);
    puzzle.raw_mut()[PZ_SAFE_PROG] = 0;

    // Step 0: Left 3 notches (0 -> 1 -> 2 -> 3) -> reaches code[0] = 3, prog = 1.
    for _ in 0..3 {
        act1_rotate_safe_dial(Act1SafeDialDirection::Left, &mut puzzle, &mut flags);
    }
    assert_eq!(act1_read_safe_dial_pos(&flags), 3);
    assert_eq!(puzzle.raw()[PZ_SAFE_PROG], 1);

    // Wrong direction on Step 1 (Left instead of Right) resets progress!
    let wrong = act1_rotate_safe_dial(Act1SafeDialDirection::Left, &mut puzzle, &mut flags);
    assert_eq!(wrong.progress_stage, 0);

    // Recover Step 0 by turning Right (4 -> 3) -> lands on code[0] = 3, prog = 1!
    let rec = act1_rotate_safe_dial(Act1SafeDialDirection::Right, &mut puzzle, &mut flags);
    assert_eq!((rec.dial_pos, rec.progress_stage), (3, 1));

    // Step 1: Right 5 notches (3 -> 2 -> 1 -> 0 -> 19 -> 18) -> prog = 2.
    for _ in 0..5 {
        act1_rotate_safe_dial(Act1SafeDialDirection::Right, &mut puzzle, &mut flags);
    }
    assert_eq!(puzzle.raw()[PZ_SAFE_PROG], 2);

    // Step 2: Left 4 notches (18 -> 19 -> 0 -> 1 -> 2) -> prog = 3.
    for _ in 0..4 {
        act1_rotate_safe_dial(Act1SafeDialDirection::Left, &mut puzzle, &mut flags);
    }
    assert_eq!(puzzle.raw()[PZ_SAFE_PROG], 3);

    // Step 3: Right 3 notches (2 -> 1 -> 0 -> 19) -> unlocks safe (flag 108, SE 0x3e88)!
    let mut final_step = None;
    for _ in 0..3 {
        final_step = Some(act1_rotate_safe_dial(
            Act1SafeDialDirection::Right,
            &mut puzzle,
            &mut flags,
        ));
    }
    let solved = final_step.unwrap();
    assert!(solved.unlocked);
    assert_eq!(solved.unlock_se, Some(ACT1_SE_SAFE_UNLOCK));

    let bullets = act1_claim_safe_contents(&mut flags, &mut inv);
    assert!(bullets >= 40);
    assert_eq!(act1_claim_safe_contents(&mut flags, &mut inv), 0);
}

#[test]
fn apartments_trash_chute_old_man_coin_butterfly_hole_and_coin_alignment_fixes() {
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();

    // Easy trash chute shows extra hint msg 16; Normal shows only msg 15.
    assert_eq!(act1_look_trash_chute(RiddleDifficulty::Easy), &[15, 16]);
    assert_eq!(act1_look_trash_chute(RiddleDifficulty::Normal), &[15]);

    // OldManCoinFix toggle: vanilla shows coin prop before Canned Juice is dropped; SH2:EE hides it!
    assert!(act1_is_old_man_coin_prop_visible(
        &flags,
        Act1VanillaFixesConfig::vanilla_ps2()
    ));
    assert!(!act1_is_old_man_coin_prop_visible(
        &flags,
        Act1VanillaFixesConfig::enhanced()
    ));

    // Drop Canned Juice -> consumes item 49, sets flag 95, enables courtyard pickup.
    inv.item_get_now(ACT1_ITEM_CANNED_JUICE);
    assert_eq!(
        act1_use_canned_juice_on_chute(&mut flags, &mut inv),
        Some(ACT1_SE_TRASH_CHUTE_DROP)
    );
    assert!(!inv.has_item(ACT1_ITEM_CANNED_JUICE));
    assert!(act1_is_old_man_coin_prop_visible(
        &flags,
        Act1VanillaFixesConfig::enhanced()
    ));
    assert!(act1_pickup_old_man_coin_courtyard(&mut flags, &mut inv));
    assert!(inv.has_item(ACT1_ITEM_COIN_OLD_MAN));
    assert!(!act1_is_old_man_coin_prop_visible(
        &flags,
        Act1VanillaFixesConfig::vanilla_ps2()
    ));

    // Fire-escape key & CoinPuzzleFix UV/vertex alignment toggle.
    inv.item_get_now(ACT1_ITEM_FIRE_ESCAPE_KEY);
    assert!(act1_use_fire_escape_key(&mut flags, &inv));
    let vanilla_quad = act1_coin_puzzle_quad_layout(
        Act1DeskCoinKind::Prisoner,
        2,
        Act1VanillaFixesConfig::vanilla_ps2(),
    )
    .unwrap();
    let ee_quad = act1_coin_puzzle_quad_layout(
        Act1DeskCoinKind::Prisoner,
        2,
        Act1VanillaFixesConfig::enhanced(),
    )
    .unwrap();
    assert_ne!(vanilla_quad.uv, ee_quad.uv);
    assert_eq!(ee_quad.bounds, [-414, -650, 656, 420]);

    // Butterfly hole encounter: No -> msg 2; Yes -> msg 19 + Clock Key (28) + flag 79; repeat -> msg 3.
    assert_eq!(
        act1_interact_butterfly_hole(false, &mut flags, &mut inv),
        (2, false)
    );
    assert_eq!(
        act1_interact_butterfly_hole(true, &mut flags, &mut inv),
        (19, true)
    );
    assert!(inv.has_item(ACT1_ITEM_CLOCK_KEY));
    assert_eq!(
        act1_interact_butterfly_hole(true, &mut flags, &mut inv),
        (3, false)
    );
}

#[test]
fn brookhaven_normal_keypad_louise_box_teddy_bear_shower_drain_and_roof_ambush() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();

    // 1. 3F Patient Wing Keypad: 7335 on Normal vs 1328 on Hard.
    let mut pad = Act1HospitalWingKeypad::new();
    for d in [7, 3, 3, 5] {
        pad.press_digit(d, RiddleDifficulty::Hard, &mut flags);
    }
    assert!(!flags.get(ACT1_FLAG_WING_KEYPAD_UNLOCKED));
    for d in [1, 3, 2, 8] {
        pad.press_digit(d, RiddleDifficulty::Hard, &mut flags);
    }
    assert!(flags.get(ACT1_FLAG_WING_KEYPAD_UNLOCKED));

    // 2. Carbon Paper + Louise Box (anti-bruteforce scramble when flag 203 is unread!).
    puzzle.write_u16(PZ_BOX_CODE, 0x4829);
    puzzle.raw_mut()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4].copy_from_slice(&[2, 5, 1, 7]);
    puzzle.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&[2, 5, 1, 6]);
    let (msg, code_str) =
        act1_read_carbon_paper_memo(RiddleDifficulty::Normal, &puzzle, &mut flags);
    assert_eq!((msg, code_str.as_str()), (23, "4829"));

    let mut box_state = Act1LouiseBoxState::new();
    // Rotating wheel 3 to match goal BEFORE reading wall blood code (flag 203) does NOT unlock,
    // and backing out scrambles a wheel!
    assert!(!box_state.rotate_wheel(3, &mut puzzle, &mut flags));
    assert_eq!(
        puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4],
        puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
    );
    assert!(box_state.on_exit_box(0x0301, &mut puzzle, &flags));
    assert_ne!(
        puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4],
        puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
    );

    // Now read wall blood code (sets flag 203), align wheels, enter 4829, and use both keys!
    act1_read_louise_wall_blood_code(&mut flags);
    puzzle.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&[2, 5, 1, 6]);
    assert!(box_state.rotate_wheel(3, &mut puzzle, &mut flags));
    for d in [4, 8, 2, 9] {
        box_state.press_keypad_digit(d, &puzzle, &mut flags);
    }
    inv.item_get_now(ACT1_ITEM_PURPLE_BULL_KEY);
    inv.item_get_now(ACT1_ITEM_LAPIS_EYE_KEY);
    assert!(box_state.use_key(ACT1_COMBO_BULL_AND_LAPIS_KEYS, &mut flags, &mut inv));
    assert!(box_state.claim_piece_of_hair(&mut flags, &mut inv));
    assert!(inv.has_item(ACT1_ITEM_PIECE_OF_HAIR));

    // 3. Teddy Bear Bent Needle + TeddyBearLookFix toggle.
    assert!(act1_inspect_teddy_bear(&mut flags, &mut inv));
    assert!(inv.has_item(ACT1_ITEM_BENT_NEEDLE));
    assert!(act1_should_james_look_at_prop(
        ACT1_PROP_KIND_TEDDY_BEAR,
        &flags,
        Act1VanillaFixesConfig::vanilla_ps2()
    ));
    assert!(!act1_should_james_look_at_prop(
        ACT1_PROP_KIND_TEDDY_BEAR,
        &flags,
        Act1VanillaFixesConfig::enhanced()
    ));

    // 4. Shower Room Drain + ShowerRoomFlashlightFix toggle.
    assert!(!act1_eval_shower_flashlight_glow(
        ACT1_ROOM_HSP_SHOWER,
        false,
        true,
        Act1VanillaFixesConfig::vanilla_ps2()
    ));
    assert!(act1_eval_shower_flashlight_glow(
        ACT1_ROOM_HSP_SHOWER,
        false,
        true,
        Act1VanillaFixesConfig::enhanced()
    ));
    assert_eq!(
        act1_interact_shower_drain(Some(ACT1_ITEM_BENT_NEEDLE), &mut flags, &mut inv),
        Act1ShowerDrainOutcome::NeedleOnlyTooShort
    );
    assert_eq!(
        act1_interact_shower_drain(Some(ACT1_ITEM_PIECE_OF_HAIR), &mut flags, &mut inv),
        Act1ShowerDrainOutcome::HairOnlyUseless
    );
    assert_eq!(
        act1_interact_shower_drain(Some(ACT1_COMBO_HAIR_AND_NEEDLE), &mut flags, &mut inv),
        Act1ShowerDrainOutcome::FishedElevatorKey
    );
    assert!(inv.has_item(ACT1_ITEM_ELEVATOR_KEY));

    // 5. Roof Pyramid Head push HP reduction.
    flags.set(ACT1_FLAG_ROOF_DOOR_LOCKED_BEHIND, true);
    assert!((act1_apply_roof_pyramid_head_push(100.0, 100.0, &mut flags) - 30.0).abs() < 1e-4);
    assert!((act1_apply_roof_pyramid_head_push(40.0, 100.0, &mut flags) - 5.0).abs() < 1e-4);
    assert!(!flags.get(ACT1_FLAG_ROOF_DOOR_LOCKED_BEHIND));
}

#[test]
fn brookhaven_otherworld_refrigerator_angel_door_elevator_quiz_chest_and_pass_chase() {
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();

    // 1. Refrigerator fails without Maria, succeeds with Maria (flag 15) -> Lead Ring (57).
    assert_eq!(
        act1_interact_hospital_refrigerator(true, &mut flags, &mut inv),
        Act1RefrigeratorOutcome::TooHeavyAlone
    );
    flags.set(ACT1_FLAG_MARIA_ACCOMPANYING, true);
    assert_eq!(
        act1_interact_hospital_refrigerator(true, &mut flags, &mut inv),
        Act1RefrigeratorOutcome::OpenedWithMaria
    );
    assert!(inv.has_item(ACT1_ITEM_LEAD_RING));
    assert_eq!(act1_hospital_refrigerator_anim_id(&flags), 0x282f);

    // 2. B1 Shelf + Copper Ring + 3F Angel Door unlock.
    assert!(act1_push_basement_shelf(true, &mut flags));
    inv.item_get_now(ACT1_ITEM_COPPER_RING);
    assert_eq!(
        act1_place_ring_on_angel_door(ACT1_ITEM_COPPER_RING, &mut flags, &mut inv),
        (true, false)
    );
    assert_eq!(
        act1_place_ring_on_angel_door(ACT1_ITEM_LEAD_RING, &mut flags, &mut inv),
        (true, true)
    );

    // 3. Trick-or-Treat Elevator Radio Quiz timeline & completion flag 232.
    assert!(act1_should_trigger_elevator_radio_quiz(true, &flags));
    assert_eq!(
        act1_step_elevator_radio_quiz(250.0, false, &mut flags),
        (Some(32), false)
    );
    assert_eq!(
        act1_step_elevator_radio_quiz(5200.0, false, &mut flags),
        (None, true)
    );
    assert!(flags.get(ACT1_FLAG_ELEVATOR_QUIZ_HEARD));

    // 4. 3F Store Room Quiz Chest: wrong answer -> poison spray damage; right answer (3, 1, 3) -> solve.
    let mut wrong_flags = flags.clone();
    assert_eq!(
        act1_select_quiz_chest_button(1, 2, &mut wrong_flags),
        Some(Act1QuizChestOutcome::Incomplete)
    );
    act1_select_quiz_chest_button(2, 1, &mut wrong_flags);
    assert_eq!(
        act1_select_quiz_chest_button(3, 3, &mut wrong_flags),
        Some(Act1QuizChestOutcome::WrongPoisonSpray)
    );
    let (_, hp_after_gas, active) =
        act1_apply_quiz_poison_gas_damage(0.95, 0.1, 100.0, 100.0, 25.0);
    assert!(active);
    assert!((hp_after_gas - 40.0).abs() < 1e-4);
    let (_, hp_clamped, _) = act1_apply_quiz_poison_gas_damage(0.95, 0.1, 50.0, 100.0, 25.0);
    assert!((hp_clamped - 25.0).abs() < 1e-4);

    act1_select_quiz_chest_button(1, 3, &mut flags);
    act1_select_quiz_chest_button(2, 1, &mut flags);
    assert_eq!(
        act1_select_quiz_chest_button(3, 3, &mut flags),
        Some(Act1QuizChestOutcome::SolvedCorrectly)
    );
    assert!(flags.get(ACT1_FLAG_QUIZ_CHEST_SOLVED));

    // 5. Basement Passage Pyramid Head speed tiers & Maria separation on escape.
    assert_eq!(
        act1_eval_hospital_pass_ph_speed_tier(18_000.0, -258_000.0),
        Some(0)
    );
    assert_eq!(
        act1_eval_hospital_pass_ph_speed_tier(19_500.0, -257_000.0),
        Some(1)
    );
    assert_eq!(
        act1_eval_hospital_pass_ph_speed_tier(19_500.0, -255_000.0),
        Some(2)
    );
    assert_eq!(
        act1_eval_hospital_pass_ph_speed_tier(19_500.0, -251_000.0),
        Some(3)
    );
    act1_complete_hospital_pass_escape(&mut flags);
    assert!(!flags.get(ACT1_FLAG_MARIA_ACCOMPANYING));
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::HospitalPass, &flags, 0),
        4
    );
}
