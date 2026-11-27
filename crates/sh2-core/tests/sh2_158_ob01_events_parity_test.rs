//! SH2-158: Deterministic 1:1 parity tests for `stg_ovservation.c` (`sh2pc.exe` `0x005a0860..0x005a13b8`).
//!
//! Verifies:
//! - All 6 `ev_prog` handlers (`ACT1_OBSERVATION_EV_PROGS` `0x008e7a18`):
//!   - `(3, 1)` (`0x005a0860`): `tenbou` intro cutscene (`demo_no = 2`, flags `33`, `1504`, `1553`)
//!   - `(3, 2)` (`0x005a09b0`): Car Map pickup (`out_p_incar` + `out/p_incar_map`, `SeCall(0x2b17)`,
//!     stage message `28` (`0x1c`), `1.0 s` alpha fade, sets flag `24` (`0x18`))
//!   - `(3, 3)` (`0x005a0ca0`): Re-examining car after pickup (`out_p_incar` empty + stage message `3`)
//!   - `(3, 4)` (`0x005a0cb0`): Forest stairs warning without map (`!flag(24)`): stage message `2`,
//!     `-500.0 mm` Z walkback (`0x0062d230`), sets flag `34` (`0x22`)
//!   - `(3, 5)` (`0x005a0de0`): Blocked road/tunnel warning: stage message `0`, `-500.0 mm` X walkback
//!   - `(3, 6)` (`0x005a0f10`): `flag(512) && !flag(513)` extra demo `0x5d` (`93`), sets flag `513`
//! - Post-`tenbou` spawn pose `(19350.25, 0.0, -507.74)` (`0x008e7b38`) + yaw `0.0` (`0x005a08aa`)
//!   and post-demo flag writes (`1504` / `1553`) in `CampaignOrchestrator`.
//! - `tenbou` mid-cutscene `1400.0` (`0x006b61d8`) BGM switch (`flag 517`) and `EvBgmControl`
//!   (`0x005a1390`) demo-active (`0x04` -> `0x0e`) vs gameplay (`0` / `1`) branches.

use glam::Vec3;
use sh2_core::campaign::cutscene_handoff::{
    TENBOU_BGM_SWITCH_FLAG, TENBOU_BGM_SWITCH_FRAME, TENBOU_POST_FLAG_A, TENBOU_POST_FLAG_B,
    TENBOU_POST_POS, TENBOU_POST_YAW,
};
use sh2_core::campaign::{
    CampaignOrchestrator, CampaignPhase, CampaignTickInput, AUTO_STAGE_DEMO_RULES,
};
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::events::stages_act1::{
    act1_eval_bgm_control, act1_eval_bgm_control_with_demo, Act1StageId,
};
use sh2_core::events::{
    EventFlags, StageProgramRunner, PROG_KEY_A, STG3_EXTRA_DEMO_NO, STG3_FLAG_EXTRA_DEMO_DONE,
    STG3_FLAG_EXTRA_DEMO_REQ, STG3_FLAG_HAS_MAP, STG3_FLAG_STAIRS_WARNED, STG3_FLAG_TENBOU_DONE,
    STG3_FLAG_TENBOU_POST_A, STG3_FLAG_TENBOU_POST_B, STG3_MSG_BLOCKED_ROAD, STG3_MSG_CAR_EMPTY,
    STG3_MSG_GOT_MAP, STG3_MSG_STAIRS_NO_MAP, STG3_SE_MAP_PICKUP, STG3_TENBOU_DEMO_NO,
    STG3_WALKBACK_DISTANCE_MM,
};
use sh2_core::scenario::{ScenarioKind, ScenarioState};
use sh2_core::simulation::SimulationCore;

#[test]
fn test_stage3_ev_prog_va_table_matches_sh2pc() {
    let expected = [
        (1, 0x005a_0860),
        (2, 0x005a_09b0),
        (3, 0x005a_0ca0),
        (4, 0x005a_0cb0),
        (5, 0x005a_0de0),
        (6, 0x005a_0f10),
    ];
    for (prog_no, va) in expected {
        assert_eq!(StageProgramRunner::resolve_ev_prog_va(3, prog_no), Some(va));
    }
    assert_eq!(StageProgramRunner::resolve_ev_prog_va(3, 0), None);
    assert_eq!(StageProgramRunner::resolve_ev_prog_va(3, 7), None);
}

#[test]
fn test_stage3_ev_prog_1_tenbou_intro() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
    state.prog_runner.start(3, 1, STG3_FLAG_TENBOU_DONE as u16);

    let running = state.prog_runner.tick(
        0,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    );
    assert!(!running);
    assert_eq!(
        state.prog_runner.take_requested_demo(),
        Some(STG3_TENBOU_DEMO_NO)
    );
    assert!(state.flags.get(STG3_FLAG_TENBOU_DONE));
    assert!(state.flags.get(STG3_FLAG_TENBOU_POST_A));
    assert!(state.flags.get(STG3_FLAG_TENBOU_POST_B));
}

#[test]
fn test_stage3_ev_prog_2_car_map_pickup_and_prog_3_reexamine() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);

    // (3, 2) `0x005a09b0`: Car map pickup
    state.prog_runner.start(3, 2, STG3_FLAG_HAS_MAP as u16);
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
    assert!((state.cutaway_overlay.incar_alpha() - 1.0).abs() < 1e-5);

    // Confirm -> SE 0x2b17 + stage message 28 (`0x1c`)
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
    assert_eq!(state.prog_runner.sound_effects(), &[STG3_SE_MAP_PICKUP]);
    assert_eq!(
        state.prog_runner.active_message_id(),
        Some(STG3_MSG_GOT_MAP)
    );

    // Dismiss message 28 -> starts 1.0 s alpha fade
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
    assert_eq!(state.prog_runner.active_message_id(), None);

    // 0.5 s into fade -> alpha 0.5
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

    // Remaining 0.5 s -> closes cutaway, unlocks player, sets flag 24 (`0x18`)
    assert!(!state.prog_runner.tick(
        0,
        0,
        0.5,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert!(!state.prog_runner.is_player_locked());
    assert!(state.flags.get(STG3_FLAG_HAS_MAP));
    assert_eq!(state.cutaway_overlay.active_picture(), None);

    // (3, 3) `0x005a0ca0`: Re-examining car after pickup -> empty out_p_incar + stage message 3
    state.prog_runner.start(3, 3, 0);
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
    assert_eq!(state.cutaway_overlay.active_picture(), Some("out_p_incar"));
    assert!((state.cutaway_overlay.incar_alpha() - 0.0).abs() < 1e-5);

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
    assert_eq!(
        state.prog_runner.active_message_id(),
        Some(STG3_MSG_CAR_EMPTY)
    );

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
    assert_eq!(state.prog_runner.active_message_id(), None);
    assert_eq!(state.cutaway_overlay.active_picture(), None);
    assert!(!state.prog_runner.is_player_locked());
}

#[test]
fn test_stage3_ev_prog_4_stairs_walkback_z_and_prog_5_road_walkback_x() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);

    // (3, 4) `0x005a0cb0`: Forest stairs without map -> message 2, -500 mm Z walkback, flag 34
    state
        .prog_runner
        .start(3, 4, STG3_FLAG_STAIRS_WARNED as u16);
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
    assert_eq!(
        state.prog_runner.active_message_id(),
        Some(STG3_MSG_STAIRS_NO_MAP)
    );

    // Dismissing message 2 emits the -500 mm Z walkback target and stays locked in Step 2
    // until `PlayerEventMoveIsEnd()` (`0x0053b6d0`) completes (`SH2-189`).
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
    assert!(state.prog_runner.is_player_locked());
    assert_eq!(
        state.prog_runner.take_walkback_delta(),
        Some([0.0, 0.0, -STG3_WALKBACK_DISTANCE_MM])
    );
    state.prog_runner.notify_event_move_finished();
    assert!(!state.prog_runner.tick(
        0,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert!(!state.prog_runner.is_player_locked());
    assert!(state.flags.get(STG3_FLAG_STAIRS_WARNED));

    // (3, 5) `0x005a0de0`: Blocked road/tunnel -> message 0, -500 mm X walkback
    state.prog_runner.start(3, 5, 0);
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
    assert_eq!(
        state.prog_runner.active_message_id(),
        Some(STG3_MSG_BLOCKED_ROAD)
    );

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
    assert!(state.prog_runner.is_player_locked());
    assert_eq!(
        state.prog_runner.take_walkback_delta(),
        Some([-STG3_WALKBACK_DISTANCE_MM, 0.0, 0.0])
    );
    state.prog_runner.notify_event_move_finished();
    assert!(!state.prog_runner.tick(
        0,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert!(!state.prog_runner.is_player_locked());
}

#[test]
fn test_stage3_ev_prog_6_extra_demo_93() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);

    // Without flag 512, demo 0x5d does not fire and flag 513 stays false.
    state
        .prog_runner
        .start(3, 6, STG3_FLAG_EXTRA_DEMO_DONE as u16);
    assert!(!state.prog_runner.tick(
        0,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert_eq!(state.prog_runner.take_requested_demo(), None);
    assert!(!state.flags.get(STG3_FLAG_EXTRA_DEMO_DONE));

    // With flag 512 && !flag 513, requests demo 0x5d (93) and sets flag 513.
    state.flags.set(STG3_FLAG_EXTRA_DEMO_REQ, true);
    state
        .prog_runner
        .start(3, 6, STG3_FLAG_EXTRA_DEMO_DONE as u16);
    assert!(!state.prog_runner.tick(
        0,
        0,
        0.016,
        &mut state.flags,
        &mut state.puzzle_state,
        &mut state.inventory_mgr,
        &mut state.cutaway_overlay,
        state.riddle_difficulty,
    ));
    assert_eq!(
        state.prog_runner.take_requested_demo(),
        Some(STG3_EXTRA_DEMO_NO)
    );
    assert!(state.flags.get(STG3_FLAG_EXTRA_DEMO_DONE));
}

#[test]
fn test_ev_bgm_control_observation_deck_demo_and_gameplay() {
    let mut flags = EventFlags::new();

    // Gameplay (`demo_active = false`): 0 when !flag(36), 1 when flag(36), regardless of flag(517).
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::Observation, &flags, 0),
        0
    );
    flags.set(TENBOU_BGM_SWITCH_FLAG, true);
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::Observation, &flags, 0),
        0
    );
    flags.set(36, true);
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::Observation, &flags, 0),
        1
    );

    // Demo active (`0x00932194 & 0x40 != 0`): 0x04 before flag(517), 0x0e after flag(517).
    let mut demo_flags = EventFlags::new();
    assert_eq!(
        act1_eval_bgm_control_with_demo(Act1StageId::Observation, &demo_flags, 0, true),
        0x04
    );
    demo_flags.set(TENBOU_BGM_SWITCH_FLAG, true);
    assert_eq!(
        act1_eval_bgm_control_with_demo(Act1StageId::Observation, &demo_flags, 0, true),
        0x0e
    );
}

#[test]
fn test_tenbou_handoff_pose_flags_and_1400_frame_bgm_switch() {
    // Verify AUTO_STAGE_DEMO_RULES Stage 3 entry matches sh2pc.exe 0x008e7b38 & 0x005a08aa
    let rule = AUTO_STAGE_DEMO_RULES
        .iter()
        .find(|r| r.stage_id == 3)
        .expect("stage 3 auto demo rule");
    assert_eq!(rule.demo_no, 2);
    assert_eq!(rule.completion_flag, 33);
    assert_eq!(rule.post_pos, Some(TENBOU_POST_POS));
    assert_eq!(rule.post_yaw, Some(TENBOU_POST_YAW));

    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 3;
    let mut orch = CampaignOrchestrator::from_saved_stage(
        3,
        Some("ob01".to_string()),
        [0.0, 0.0, 0.0],
        1.25,
        OutdoorStreamingMode::default(),
        false,
    );

    // Drive RoomLoading -> DramaDemo (`tenbou`, demo_no = 2)
    let idle = CampaignTickInput::default();
    for _ in 0..8 {
        if matches!(orch.phase, CampaignPhase::RoomLoading { .. }) {
            orch.notify_room_assets_loaded();
        }
        let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
        if matches!(orch.phase, CampaignPhase::DramaDemo { demo_no: 2, .. }) {
            break;
        }
    }
    assert!(
        matches!(orch.phase, CampaignPhase::DramaDemo { demo_no: 2, .. }),
        "expected tenbou DramaDemo, got {:?}",
        orch.phase
    );

    // Frame <= 1400.0 -> flag(517) stays false, EvBgmControl returns 0x04
    let at_threshold = CampaignTickInput {
        drama_demo_frame: Some(TENBOU_BGM_SWITCH_FRAME),
        ..CampaignTickInput::default()
    };
    let eff_before = orch.step(&mut sim, &at_threshold, 1.0 / 60.0);
    assert!(!sim.scenario.flags.get(TENBOU_BGM_SWITCH_FLAG));
    assert_eq!(eff_before.bgm_control_mask, Some(0x04));

    // Frame > 1400.0 -> sets flag(517), EvBgmControl switches to 0x0e
    let after_threshold = CampaignTickInput {
        drama_demo_frame: Some(TENBOU_BGM_SWITCH_FRAME + 1.0),
        ..CampaignTickInput::default()
    };
    let eff_after = orch.step(&mut sim, &after_threshold, 1.0 / 60.0);
    assert!(sim.scenario.flags.get(TENBOU_BGM_SWITCH_FLAG));
    assert_eq!(eff_after.bgm_control_mask, Some(0x0e));

    // Finish tenbou -> snaps James to (19350.25, 0.0, -507.74), yaw 0.0, sets flags 33, 1504, 1553,
    // and returns EvBgmControl to gameplay mode (0 while !flag(36)).
    let finish_input = CampaignTickInput {
        drama_demo_finished: true,
        ..CampaignTickInput::default()
    };
    let eff_done = orch.step(&mut sim, &finish_input, 1.0 / 60.0);
    assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));
    assert_eq!(
        sim.scenario.player_position,
        Vec3::from_array(TENBOU_POST_POS)
    );
    assert!((sim.scenario.player_yaw - TENBOU_POST_YAW).abs() < 1e-6);
    assert!(sim.scenario.flags.get(STG3_FLAG_TENBOU_DONE));
    assert!(sim.scenario.flags.get(TENBOU_POST_FLAG_A));
    assert!(sim.scenario.flags.get(TENBOU_POST_FLAG_B));
    assert_eq!(eff_done.bgm_control_mask, Some(0));

    // Verify campaign gameplay smoothly turns James around and walks him `-500 mm` along Z
    // via `PlayerEventMove` (`0x0053b3c0`) instead of snapping in a single frame (`SH2-189`).
    sim.scenario
        .prog_runner
        .start(3, 4, STG3_FLAG_STAIRS_WARNED as u16);
    let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
    let confirm = CampaignTickInput {
        action_pressed: true,
        ..CampaignTickInput::default()
    };
    let eff_start = orch.step(&mut sim, &confirm, 1.0 / 60.0);
    assert!(!eff_start.allow_gameplay_sim);
    assert!(sim.event_move.is_active());

    for _ in 0..180 {
        let eff = orch.step(&mut sim, &idle, 1.0 / 60.0);
        if eff.allow_gameplay_sim && !sim.event_move.is_active() {
            break;
        }
    }
    assert!(!sim.event_move.is_active());
    assert!(sim.scenario.flags.get(STG3_FLAG_STAIRS_WARNED));
    let expected_z = TENBOU_POST_POS[2] - STG3_WALKBACK_DISTANCE_MM;
    assert!(
        (sim.scenario.player_position.z - expected_z).abs() < 100.0,
        "expected James near {expected_z} mm after PlayerEventMove, got {}",
        sim.scenario.player_position.z
    );
}
