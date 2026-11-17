//! Integration & retail-binary parity tests for `SH2-113`:
//! Act 1 static stage tables (`Stages 1..=27`), lifecycle hooks, Wood Side & Blue Creek Apartments
//! interactive riddles, Brookhaven Hospital (Normal + Otherworld) riddles, and toggleable 1:1
//! vanilla PS2 vs SH2:EE fixes.

use std::path::Path;

use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::events::*;
use sh2_core::inventory::InventoryManager;
use sh2_core::types::StageDataHeader;

fn u32_le(pe: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(pe[off..off + 4].try_into().unwrap())
}

fn i16_le(pe: &[u8], off: usize) -> i16 {
    i16::from_le_bytes(pe[off..off + 2].try_into().unwrap())
}

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
        assert!(desc.event_list.len() >= 2);
        assert!(desc.event_list.last().unwrap().is_sentinel());

        for entry in &desc.event_list[..desc.event_list.len() - 1] {
            assert!(!entry.is_sentinel());
            let off = entry.ev_pos_offset();
            if let (true, Some(shape)) = (off > 0, entry.shape_kind()) {
                assert!(decode_act1_ev_pos_anchor(desc.ev_pos, off, shape).is_some());
            }
        }
        for enemy in desc.enemy_list {
            assert_ne!(decode_act1_enemy_spawn(enemy).kind, 0);
        }
        for item in desc.item_list {
            assert!(item.item_kind_index() < 7);
            let converted = item.to_item_list_entry();
            assert!(!converted.is_sentinel());
            assert_eq!(converted.flag_id(), item.flag_id());
        }
    }

    assert_eq!(act1_stage_descriptor(2).unwrap().event_list.len() - 1, 14);
    assert_eq!(act1_stage_descriptor(5).unwrap().enemy_list.len(), 104);
    assert_eq!(act1_stage_descriptor(14).unwrap().enemy_list.len(), 83);
    assert_eq!(act1_stage_descriptor(20).unwrap().event_list.len() - 1, 64);
    assert_eq!(act1_stage_descriptor(25).unwrap().item_list.len(), 12);
}

#[test]
fn act1_retail_sh2pc_binary_byte_for_byte_verification() {
    let exe = Path::new("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2pc.exe");
    let Ok(pe) = std::fs::read(exe) else {
        return; // Retail asset rip is symlinked locally and omitted in GitHub Actions CI.
    };

    for raw_id in 1u8..=27 {
        let desc = act1_stage_descriptor(raw_id).unwrap();
        let sd = (desc.pc_stage_data_addr - 0x0040_0000) as usize;
        let f: Vec<u32> = (0..17).map(|k| u32_le(&pe, sd + k * 4)).collect();
        let expected_hdr = StageDataHeader {
            ev_list_ptr: f[0],
            ev_pos_ptr: f[1],
            ev_prog_ptr: f[2],
            item_list_ptr: f[3],
            model_list_ptr: f[4],
            enemy_list_ptr: f[5],
            stage_init_fn: f[6],
            room_init_fn: f[7],
            all_time_fn: f[8],
            glb_crd: f[9] as i32,
            sound_call_after_load_fn: f[15],
            bgm_control_fn: f[12],
            chara_data_clear_fn: f[14],
        };
        assert_eq!(desc.header, expected_hdr);

        let ev_off = (desc.header.ev_list_ptr - 0x0040_0000) as usize;
        for (i, e) in desc.event_list[..desc.event_list.len() - 1]
            .iter()
            .enumerate()
        {
            let b = ev_off + i * 16;
            let raw = (
                u32_le(&pe, b),
                u32_le(&pe, b + 4),
                u32_le(&pe, b + 8),
                u32_le(&pe, b + 12),
            );
            assert_eq!((e.flag, e.cond, e.rslt0, e.rslt1), raw);
        }

        let pos_off = (desc.header.ev_pos_ptr - 0x0040_0000) as usize;
        assert_eq!(desc.ev_pos, &pe[pos_off..pos_off + desc.ev_pos.len()]);

        for (i, &addr) in desc.ev_prog_addrs.iter().enumerate() {
            let p_off = (desc.header.ev_prog_ptr - 0x0040_0000) as usize + i * 4;
            assert_eq!(addr, u32_le(&pe, p_off));
        }

        for (i, it) in desc.item_list.iter().enumerate() {
            let b = (desc.header.item_list_ptr - 0x0040_0000) as usize + i * 16;
            assert_eq!(
                (it.pos_x.to_bits(), it.pos_z.to_bits(), it.st),
                (u32_le(&pe, b), u32_le(&pe, b + 4), u32_le(&pe, b + 12))
            );
            assert_eq!(
                (it.pos_y, it.rot_y),
                (i16_le(&pe, b + 8), i16_le(&pe, b + 10))
            );
        }

        for (i, m) in desc.model_list.iter().enumerate() {
            let b = (desc.header.model_list_ptr - 0x0040_0000) as usize + i * 40;
            let id = i16::from_le_bytes([m.room, m.state as u8]);
            assert_eq!(
                (m.kind, id, m.flag_0, m.flag_1),
                (
                    i16_le(&pe, b),
                    i16_le(&pe, b + 2),
                    i16_le(&pe, b + 4),
                    i16_le(&pe, b + 6)
                )
            );
            for c in 0..4 {
                assert_eq!(m.pos[c].to_bits(), u32_le(&pe, b + 8 + c * 4));
                assert_eq!(m.rot[c].to_bits(), u32_le(&pe, b + 24 + c * 4));
            }
        }

        for (i, en_raw) in desc.enemy_list.iter().enumerate() {
            let b = (desc.header.enemy_list_ptr - 0x0040_0000) as usize + i * 20;
            let d = decode_act1_enemy_spawn(en_raw);
            assert_eq!((d.kind, d.id), (i16_le(&pe, b), i16_le(&pe, b + 2)));
            assert_eq!(
                (d.pos_x_mm as u32, d.pos_z_mm as u32),
                (u32_le(&pe, b + 4), u32_le(&pe, b + 8))
            );
            assert_eq!(
                (d.pos_y_mm, d.rot_y, d.status, d.condition),
                (
                    i16_le(&pe, b + 12),
                    i16_le(&pe, b + 14),
                    i16_le(&pe, b + 16),
                    i16_le(&pe, b + 18) as u16
                )
            );
        }
    }
}

#[test]
fn apartments_wallet_memo_and_w2f_safe_dial_after_init_new_game() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut rng = Sh2Rng::new(0x5113_2001);
    let mut inv = InventoryManager::new();

    init_new_game(
        &mut flags,
        &mut puzzle,
        &mut rng,
        RiddleDifficulty::Normal,
        ActionDifficulty::Normal,
        ClearData::default(),
    );
    assert_eq!(&puzzle.raw()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4], &[0xff; 4]);

    let c = [
        puzzle.raw()[PZ_SAFE_CODE],
        puzzle.raw()[PZ_SAFE_CODE + 1],
        puzzle.raw()[PZ_SAFE_CODE + 2],
        puzzle.raw()[PZ_SAFE_CODE + 3],
    ];
    while act1_read_safe_dial_pos(&flags) != c[0] {
        act1_rotate_safe_dial(Act1SafeDialDirection::Left, &mut puzzle, &mut flags);
    }
    assert_eq!(puzzle.raw()[PZ_SAFE_PROG], c[0]);

    // Wrong direction (`Left` instead of `Right`) resets all 4 bytes of `PZ_SAFE_PROG` to `0xff`.
    let wrong = act1_rotate_safe_dial(Act1SafeDialDirection::Left, &mut puzzle, &mut flags);
    assert_eq!(wrong.progress_stage, 0);
    assert_eq!(&puzzle.raw()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4], &[0xff; 4]);

    let rec = act1_rotate_safe_dial(Act1SafeDialDirection::Right, &mut puzzle, &mut flags);
    assert_eq!((rec.dial_pos, rec.progress_stage), (c[0], 1));
    while act1_read_safe_dial_pos(&flags) != c[1] {
        act1_rotate_safe_dial(Act1SafeDialDirection::Right, &mut puzzle, &mut flags);
    }
    while act1_read_safe_dial_pos(&flags) != c[2] {
        act1_rotate_safe_dial(Act1SafeDialDirection::Left, &mut puzzle, &mut flags);
    }
    let mut last = None;
    while act1_read_safe_dial_pos(&flags) != c[3] {
        last = Some(act1_rotate_safe_dial(
            Act1SafeDialDirection::Right,
            &mut puzzle,
            &mut flags,
        ));
    }
    assert!(last.unwrap().unlocked);
    assert!(act1_claim_safe_contents(&mut flags, &mut inv) >= 40);
    assert_eq!(act1_claim_safe_contents(&mut flags, &mut inv), 0);

    puzzle.raw_mut()[PZ_SAFE_CODE..PZ_SAFE_CODE + 4].copy_from_slice(&[0, 9, 14, 19]);
    assert_eq!(
        format_act1_wallet_safe_memo(&puzzle, RiddleDifficulty::Normal),
        "01>>10<<15>>20"
    );
    assert_eq!(
        format_act1_wallet_safe_memo(&puzzle, RiddleDifficulty::Hard),
        "1>>X0<<VV5>>XX"
    );
    assert_eq!(
        format_act1_wallet_safe_memo(&puzzle, RiddleDifficulty::Extra),
        "1>>a<<f>>k"
    );
    let r0 = act1_interact_toilet_wallet(false, RiddleDifficulty::Hard, &puzzle, &mut flags);
    let r1 = act1_interact_toilet_wallet(true, RiddleDifficulty::Hard, &puzzle, &mut flags);
    let r2 = act1_interact_toilet_wallet(false, RiddleDifficulty::Hard, &puzzle, &mut flags);
    assert_eq!((r0.message_id, r1.message_id, r2.message_id), (2, 3, 4));
}

#[test]
fn apartments_trash_chute_old_man_coin_butterfly_hole_and_coin_alignment_fixes() {
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();
    let van = Act1VanillaFixesConfig::vanilla_ps2();
    let enh = Act1VanillaFixesConfig::enhanced();

    assert_eq!(
        (
            ACT1_ITEM_COURTYARD_KEY,
            ACT1_ITEM_FIRE_ESCAPE_KEY,
            ACT1_ITEM_LYNE_HOUSE_KEY
        ),
        (29, 30, 31)
    );
    assert_eq!(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED, 93);
    assert_eq!(act1_look_trash_chute(RiddleDifficulty::Easy), &[15, 16]);
    assert_eq!(act1_look_trash_chute(RiddleDifficulty::Normal), &[15]);

    assert!(act1_is_old_man_coin_prop_visible(&flags, van));
    assert!(!act1_is_old_man_coin_prop_visible(&flags, enh));
    flags.set(ACT1_FLAG_OLD_MAN_COIN_SUB, true);
    assert!(!act1_is_old_man_coin_prop_visible(&flags, van));
    flags.set(ACT1_FLAG_OLD_MAN_COIN_SUB, false);

    inv.item_get_now(ACT1_ITEM_CANNED_JUICE);
    assert_eq!(
        act1_use_canned_juice_on_chute(&mut flags, &mut inv),
        Some(ACT1_SE_TRASH_CHUTE_DROP)
    );
    assert!(act1_is_old_man_coin_prop_visible(&flags, enh));
    assert!(act1_pickup_old_man_coin_courtyard(&mut flags, &mut inv));
    assert!(inv.has_item(ACT1_ITEM_COIN_OLD_MAN));
    assert!(!act1_is_old_man_coin_prop_visible(&flags, van));

    inv.item_get_now(ACT1_ITEM_FIRE_ESCAPE_KEY);
    assert!(act1_use_fire_escape_key(&mut flags, &mut inv));
    assert!(flags.get(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED) && !flags.get(138));
    assert!(!inv.has_item(ACT1_ITEM_FIRE_ESCAPE_KEY));

    let v_p = act1_coin_puzzle_quad_layout(Act1DeskCoinKind::Prisoner, 2, van).unwrap();
    let e_p = act1_coin_puzzle_quad_layout(Act1DeskCoinKind::Prisoner, 2, enh).unwrap();
    assert_eq!(v_p.bounds, [-416, -671, -416 + 768, -671 + 1024]);
    assert_eq!(e_p.bounds, [-414, -650, 656, 420]);

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
    let van = Act1VanillaFixesConfig::vanilla_ps2();
    let enh = Act1VanillaFixesConfig::enhanced();

    let mut pad = Act1HospitalWingKeypad::new();
    for d in [7, 3, 3, 5] {
        pad.press_digit(d, RiddleDifficulty::Hard, &mut flags);
    }
    assert!(!flags.get(ACT1_FLAG_WING_KEYPAD_UNLOCKED));
    for d in [1, 3, 2, 8] {
        pad.press_digit(d, RiddleDifficulty::Hard, &mut flags);
    }
    assert!(flags.get(ACT1_FLAG_WING_KEYPAD_UNLOCKED));

    puzzle.write_u16(PZ_BOX_CODE, 0x4829);
    puzzle.raw_mut()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4].copy_from_slice(&[2, 5, 1, 7]);
    puzzle.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&[2, 5, 1, 6]);
    let (msg, s) = act1_read_carbon_paper_memo(RiddleDifficulty::Normal, &puzzle, &mut flags);
    assert_eq!((msg, s.as_str()), (23, "4829"));

    let mut box_state = Act1LouiseBoxState::new();
    assert!(!box_state.rotate_wheel(3, &mut puzzle, &mut flags));
    assert!(box_state.on_exit_box(0x0301, &mut puzzle, &flags));
    assert_ne!(
        puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4],
        puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
    );

    flags.set(ACT1_FLAG_ROOF_DOOR_LOCKED_BEHIND, true);
    assert!((act1_apply_roof_pyramid_head_push(100.0, 100.0, &mut flags) - 30.0).abs() < 1e-4);
    assert!((act1_apply_roof_pyramid_head_push(40.0, 100.0, &mut flags) - 5.0).abs() < 1e-4);
    assert!(!flags.get(ACT1_FLAG_ROOF_DOOR_LOCKED_BEHIND));
    assert!(flags.get(ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED));
    act1_read_louise_wall_blood_code(&mut flags);
    assert!(flags.get(ACT1_FLAG_WALL_BLOOD_CODE_READ));

    puzzle.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&[2, 5, 1, 6]);
    assert!(box_state.rotate_wheel(3, &mut puzzle, &mut flags));
    for d in [4, 8, 2, 9] {
        box_state.press_keypad_digit(d, &puzzle, &mut flags);
    }
    inv.item_get_now(ACT1_ITEM_PURPLE_BULL_KEY);
    inv.item_get_now(ACT1_ITEM_LAPIS_EYE_KEY);
    assert!(box_state.use_key(ACT1_COMBO_BULL_AND_LAPIS_KEYS, &mut flags, &mut inv));
    assert!(box_state.claim_piece_of_hair(&mut flags, &mut inv));

    assert!(act1_inspect_teddy_bear(&mut flags, &mut inv));
    let bear = ACT1_PROP_KIND_TEDDY_BEAR;
    assert!(act1_should_james_look_at_prop(bear, &flags, van));
    assert!(!act1_should_james_look_at_prop(bear, &flags, enh));
    let shw = ACT1_ROOM_HSP_SHOWER;
    assert!(!act1_eval_shower_flashlight_glow(shw, false, true, van));
    assert!(act1_eval_shower_flashlight_glow(shw, false, true, enh));
    assert_eq!(
        act1_interact_shower_drain(Some(ACT1_COMBO_HAIR_AND_NEEDLE), &mut flags, &mut inv),
        Act1ShowerDrainOutcome::FishedElevatorKey
    );
    assert!(inv.has_item(ACT1_ITEM_ELEVATOR_KEY));
}

#[test]
fn brookhaven_otherworld_refrigerator_angel_door_elevator_quiz_chest_and_pass_chase() {
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();

    assert_eq!(
        act1_interact_hospital_refrigerator(true, &mut flags, &mut inv),
        Act1RefrigeratorOutcome::TooHeavyAlone
    );
    flags.set(ACT1_FLAG_MARIA_ACCOMPANYING, true);
    assert_eq!(
        act1_interact_hospital_refrigerator(true, &mut flags, &mut inv),
        Act1RefrigeratorOutcome::OpenedWithMaria
    );
    assert_eq!(act1_hospital_refrigerator_anim_id(&flags), 0x282f);

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

    assert!(act1_should_trigger_elevator_radio_quiz(true, &flags));
    assert_eq!(
        act1_step_elevator_radio_quiz(250.0, false, &mut flags),
        (Some(32), false)
    );
    assert_eq!(
        act1_step_elevator_radio_quiz(5200.0, false, &mut flags),
        (None, true)
    );

    let mut wf = flags.clone();
    act1_select_quiz_chest_button(1, 2, &mut wf);
    act1_select_quiz_chest_button(2, 1, &mut wf);
    assert_eq!(
        act1_select_quiz_chest_button(3, 3, &mut wf),
        Some(Act1QuizChestOutcome::WrongPoisonSpray)
    );
    let (_, hp, active) = act1_apply_quiz_poison_gas_damage(0.95, 0.1, 100.0, 100.0, 25.0);
    assert!(active && (hp - 40.0).abs() < 1e-4);

    act1_select_quiz_chest_button(1, 3, &mut flags);
    act1_select_quiz_chest_button(2, 1, &mut flags);
    assert_eq!(
        act1_select_quiz_chest_button(3, 3, &mut flags),
        Some(Act1QuizChestOutcome::SolvedCorrectly)
    );

    for (x, z, exp) in [
        (18_000.0, -258_000.0, 0),
        (19_500.0, -257_000.0, 1),
        (19_500.0, -255_000.0, 2),
        (19_500.0, -251_000.0, 3),
    ] {
        assert_eq!(act1_eval_hospital_pass_ph_speed_tier(x, z), Some(exp));
    }
    assert_eq!(
        (
            act1_eval_red_pyramid_walk_speed_q12(100.0, 100.0, 1.0, 0),
            act1_eval_red_pyramid_walk_speed_q12(100.0, 100.0, 1.0, 1)
        ),
        (4096, 4915)
    );
    act1_complete_hospital_pass_escape(&mut flags);
    assert!(!flags.get(ACT1_FLAG_MARIA_ACCOMPANYING));
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::HospitalPass, &flags, 0),
        4
    );
}
