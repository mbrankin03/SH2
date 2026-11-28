//! SH2-185: Deterministic 1:1 parity and real retail-asset integration tests for
//! `stg_forest.c` (Stage 4: Forest Trail & Graveyard `ca11..ca28`, `sh2pc.exe` `0x0059f2b0..0x005a0804`).
//!
//! Verifies:
//! - Complete `ACT1_FOREST_EVENT_LIST` (22 entries, `0x008e6ff8`), `ACT1_FOREST_EV_POS` (`0x008e6e88`),
//!   `ACT1_FOREST_EV_PROGS` (`0x008e7438`), and `ACT1_FOREST_MODEL_LIST` (`0x008e7470`: `0x0300`, `0x0303`,
//!   `0x0746`, `0x055d`).
//! - All 7 `ev_prog` coroutines (`0x0059f2b0`, `0x005a0570`, `0x0059f4a0`, `0x0059f5c0`,
//!   `0x0059fac0`, `0x0059f6e0`, `0x0059fb00`).
//! - `EvRoomInit` (`0x0059fb60`), `EvSoundCallAfterLoad` (`0x0059fc70`), `EvAllTime` (`0x0059fcf0`),
//!   and `EvBgmControl` (`0x005a0260`).
//! - End-to-end progression `ob01 (Stage 3) -> ca11 (Stage 4) -> Well Save -> Angela Graveyard cutscene
//!   (real retail haka_agl.dds frame-by-frame playback) -> Ranch -> cb54 (Stage 5 Town East)` plus
//!   `ca11 -> ob01` return transition and real retail `.cld` footstep materials / `.mes` / `.mdl` / `.aix`.

use std::path::PathBuf;

use glam::Vec3;
use sh2_assets::{decode_aix, parse_cld, parse_demo_dds, parse_mdl, parse_mes};
use sh2_core::campaign::cutscene_handoff::apply_post_demo_stage_flags_with_rng;
use sh2_core::campaign::{
    resolve_stage_manifest, CampaignOrchestrator, CampaignPhase, CampaignTickInput, OverlayMenuKind,
};
use sh2_core::collision::SurfaceMaterial;
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::cutscene::{find_drama_demo_by_no, DramaDemoPlayer, DramaDemoStep};
use sh2_core::events::stages_act1::{
    act1_eval_alltime_and_room, act1_eval_bgm_control_with_demo_no, act1_stage_descriptor,
    Act1StageId, Act1VanillaFixesConfig,
};
use sh2_core::events::{
    stage4_eval_room_init_angela, stage4_eval_sound_call_after_load, EventFlags,
    ForestAmbientState, Sh2Rng, StageProgramRunner, PROG_KEY_A, STG4_ANGELA_ANIM_AFTER_DEMO,
    STG4_ANGELA_ANIM_BEFORE_DEMO, STG4_ANGELA_GRAVE_DEMO_NO, STG4_ANGELA_POS_AFTER_DEMO,
    STG4_ANGELA_POS_BEFORE_DEMO, STG4_ANGELA_TALK_DEMO_NO, STG4_ANGELA_YAW, STG4_CHAINSAW_SE_POS,
    STG4_CHARA_ANGELA, STG4_FLAG_AMBIENT_MUTE, STG4_FLAG_ANGELA_MET, STG4_FLAG_ANGELA_TALK_VARIANT,
    STG4_FLAG_CHAINSAW_TAKEN, STG4_FLAG_FIRST_SAVE_EXPLAINED, STG4_FLAG_LEAVE_ENDING_DONE,
    STG4_FLAG_LEAVE_ENDING_REQ, STG4_FLAG_NG_PLUS, STG4_ITEM_CHAINSAW, STG4_JAMES_DEMO3_YAW,
    STG4_JAMES_POST_DEMO3_POS, STG4_JAMES_POST_PROG3_POS, STG4_JAMES_POST_PROG3_YAW,
    STG4_JAMES_POST_PROG4_POS, STG4_JAMES_POST_PROG4_YAW, STG4_LEAVE_ENDING_DEMO_NO,
    STG4_MSG_GET_CHAINSAW, STG4_MSG_WELL_SAVE, STG4_RUSTLE_RADIUS_MM, STG4_SE_AMBIENT_CROW,
    STG4_SE_AMBIENT_PASSER, STG4_SE_AMBIENT_RUSTLE, STG4_SE_CHAINSAW_IDLE, STG4_SE_CHAINSAW_PICKUP,
    STG4_SE_FOREST_WIND, STG4_SE_LEAVE_LETTER_A, STG4_SE_LEAVE_LETTER_B, STG4_SE_SAVE_CHIME,
    STG4_SE_TRIAL_GATE_EXIT, STG4_WELL_PICTURE, STG4_WELL_SAVE_POINT_ID, STG4_WIND_MIN_VOLUME,
};
use sh2_core::scenario::{ScenarioKind, ScenarioState};
use sh2_core::simulation::SimulationCore;

fn retail_data_root() -> Option<PathBuf> {
    if let Ok(env_dir) = std::env::var("SH2_DATA_DIR") {
        let path = PathBuf::from(env_dir);
        if path.is_dir() {
            return Some(path);
        }
    }
    let workspace_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data");
    workspace_path.is_dir().then_some(workspace_path)
}

#[test]
fn test_stage4_tables_vas_and_model_list_match_sh2pc() {
    let forest = act1_stage_descriptor(4).expect("Stage 4 Forest descriptor");

    // 22 active entries + sentinel (`0x008e6ff8`)
    assert_eq!(forest.event_list.len(), 23);
    assert!(forest.event_list[22].is_sentinel());
    // 368 bytes `ev_pos` (`0x008e6e88`)
    assert_eq!(forest.ev_pos.len(), 368);

    // All 7 `ev_prog` VAs (`0x008e7438`)
    let expected_vas = [
        (1, 0x0059_f2b0),
        (2, 0x005a_0570),
        (3, 0x0059_f4a0),
        (4, 0x0059_f5c0),
        (5, 0x0059_fac0),
        (6, 0x0059_f6e0),
        (7, 0x0059_fb00),
    ];
    assert_eq!(forest.ev_prog_addrs.len(), 8);
    for (prog_no, va) in expected_vas {
        assert_eq!(StageProgramRunner::resolve_ev_prog_va(4, prog_no), Some(va));
    }
    assert_eq!(StageProgramRunner::resolve_ev_prog_va(4, 0), None);
    assert_eq!(StageProgramRunner::resolve_ev_prog_va(4, 8), None);

    // `ACT1_FOREST_MODEL_LIST` (`0x008e7470`): `0x0300`, `0x0303`, `0x0746` (NG+ chainsaw), `0x055d` (log)
    assert_eq!(forest.model_list.len(), 4);
    assert_eq!(forest.model_list[0].kind as u16, 0x0300);
    assert_eq!(forest.model_list[1].kind as u16, 0x0303);
    assert_eq!(forest.model_list[2].kind as u16, 0x0746);
    assert_eq!(forest.model_list[2].flag_1, 12); // NG+ required
    assert_eq!(forest.model_list[2].flag_0, 38); // hidden once taken
    assert_eq!(forest.model_list[3].kind as u16, 0x055d);
    assert_eq!(forest.model_list[3].flag_1, 12); // NG+ required
    assert_eq!(forest.model_list[3].flag_0, 513);

    // Stage transition targets in `ACT1_FOREST_EVENT_LIST`:
    // [0] `rslt0 = 0x400e2060` -> `(0x60 >> 5) = 3` (Stage 3 Observation Deck `ob01`)
    assert_eq!((forest.event_list[0].rslt0 >> 5) & 0x3f, 3);
    // [10] `rslt0 = 0x40a050a0` -> `(0xa0 >> 5) = 5` (Stage 5 Town East `cb54`)
    assert_eq!((forest.event_list[10].rslt0 >> 5) & 0x3f, 5);
}

#[test]
fn test_stage4_ev_prog_1_well_save_point_handoff() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
    state.prog_runner.start(4, 1, 0);

    // Step 0: opens `out_p_redpaper` (`data/pic/out/p_redpaper.tex`) and locks player
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
        state.cutaway_overlay.active_picture(),
        Some(STG4_WELL_PICTURE)
    );

    // Step 1: confirm opens stage message 0
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
        Some(STG4_MSG_WELL_SAVE)
    );

    // Step 2: dismiss message -> SE 0x2743, flag(5), SavePointSet(2), closes picture, finishes
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
    assert_eq!(state.prog_runner.sound_effects(), &[STG4_SE_SAVE_CHIME]);
    assert!(state.flags.get(STG4_FLAG_FIRST_SAVE_EXPLAINED));
    assert_eq!(
        state.prog_runner.take_open_save_point_id(),
        Some(STG4_WELL_SAVE_POINT_ID)
    );
    assert_eq!(state.cutaway_overlay.active_picture(), None);
    assert!(!state.prog_runner.is_player_locked());
}

#[test]
fn test_stage4_ev_prog_2_3_4_angela_and_room_init() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);

    // `EvRoomInit` (`0x0059fb60`) before `flag(36)`: Angela at pre-demo pos, anim `0x0f3d`
    let pre_angela = stage4_eval_room_init_angela(&state.flags).expect("Angela spawns");
    assert_eq!(pre_angela.chara_kind, STG4_CHARA_ANGELA);
    assert_eq!(pre_angela.anim_id, STG4_ANGELA_ANIM_BEFORE_DEMO);
    assert_eq!(pre_angela.position_mm, STG4_ANGELA_POS_BEFORE_DEMO);
    assert!((pre_angela.yaw_rad - STG4_ANGELA_YAW).abs() < 1e-6);

    // `ev_prog[2]` (`0x005a0570`): requests `demo_no = 3` + post-pose, WITHOUT setting `flag(36)`
    // prematurely (so `forest/bgm_114_b.aix` remains active during `haka_agl.dds` playback).
    state.prog_runner.start(4, 2, STG4_FLAG_ANGELA_MET as u16);
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
        Some(STG4_ANGELA_GRAVE_DEMO_NO)
    );
    assert_eq!(
        state.prog_runner.take_requested_demo_post_pose(),
        Some((STG4_JAMES_POST_DEMO3_POS, STG4_JAMES_DEMO3_YAW))
    );
    assert!(
        !state.flags.get(STG4_FLAG_ANGELA_MET),
        "flag(36) must not be set until demo_no=3 completes"
    );

    // Completing `demo_no = 3` sets `flag(36)` (`0x005a07fe`) and coin-flips `flag(37)` (`0x005a07a0`)
    let mut rng = Sh2Rng::new(0x0000_0001);
    apply_post_demo_stage_flags_with_rng(STG4_ANGELA_GRAVE_DEMO_NO, &mut state.flags, &mut rng);
    assert!(state.flags.get(STG4_FLAG_ANGELA_MET));

    // `EvRoomInit` after `flag(36)`: Angela at post-demo pos, anim `0x0f3e`
    let post_angela = stage4_eval_room_init_angela(&state.flags).expect("Angela spawns");
    assert_eq!(post_angela.anim_id, STG4_ANGELA_ANIM_AFTER_DEMO);
    assert_eq!(post_angela.position_mm, STG4_ANGELA_POS_AFTER_DEMO);

    // `ev_prog[3]` (`0x0059f4a0`) & `ev_prog[4]` (`0x0059f5c0`): `demo_no = 4` + authentic post-poses,
    // and completion re-rolls `flag(37)` via `shRand() & 1` (`0x0059f52e` / `0x0059f64e`).
    state.prog_runner.start(4, 3, 0);
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
        Some(STG4_ANGELA_TALK_DEMO_NO)
    );
    assert_eq!(
        state.prog_runner.take_requested_demo_post_pose(),
        Some((STG4_JAMES_POST_PROG3_POS, STG4_JAMES_POST_PROG3_YAW))
    );

    state.prog_runner.start(4, 4, 0);
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
        Some(STG4_ANGELA_TALK_DEMO_NO)
    );
    assert_eq!(
        state.prog_runner.take_requested_demo_post_pose(),
        Some((STG4_JAMES_POST_PROG4_POS, STG4_JAMES_POST_PROG4_YAW))
    );

    // Verify `shRand() & 1` re-roll on `demo_no = 4` completion produces both true & false across seeds
    let mut saw_true = false;
    let mut saw_false = false;
    for _ in 0..32 {
        apply_post_demo_stage_flags_with_rng(STG4_ANGELA_TALK_DEMO_NO, &mut state.flags, &mut rng);
        if state.flags.get(STG4_FLAG_ANGELA_TALK_VARIANT) {
            saw_true = true;
        } else {
            saw_false = true;
        }
    }
    assert!(
        saw_true && saw_false,
        "demo_no=4 must re-roll flag(37) via RNG"
    );

    // Leave Ending mode (`flag(512)`): Angela does not spawn
    state.flags.set(STG4_FLAG_LEAVE_ENDING_REQ, true);
    assert_eq!(stage4_eval_room_init_angela(&state.flags), None);
}

#[test]
fn test_stage4_ev_prog_5_ng_plus_chainsaw_and_sound_after_load() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);

    // Regular New Game (`!flag(12)`): only wind loop `0x2742`, no chainsaw idle `0x4268`
    let (se_2d, se_pos) = stage4_eval_sound_call_after_load(&state.flags);
    assert_eq!(se_2d, vec![STG4_SE_FOREST_WIND]);
    assert!(se_pos.is_empty());

    // NG+ (`flag(12) && !flag(38)`): wind loop `0x2742` + positional chainsaw idle `0x4268`
    state.flags.set(STG4_FLAG_NG_PLUS, true);
    let (se_2d_ng, se_pos_ng) = stage4_eval_sound_call_after_load(&state.flags);
    assert_eq!(se_2d_ng, vec![STG4_SE_FOREST_WIND]);
    assert_eq!(
        se_pos_ng,
        vec![(STG4_SE_CHAINSAW_IDLE, STG4_CHAINSAW_SE_POS)]
    );

    let snap = act1_eval_alltime_and_room(
        Act1StageId::Forest,
        0,
        [0.0; 3],
        0,
        false,
        false,
        Act1VanillaFixesConfig::default(),
        &mut state.flags,
        &state.puzzle_state,
    );
    assert_eq!(
        snap.triggered_sound_effects,
        vec![STG4_SE_FOREST_WIND, STG4_SE_CHAINSAW_IDLE]
    );

    // `ev_prog[5]` (`0x0059fac0`): grants Chainsaw (`0x10`), msg `4`, stops `0x4268`, plays `0x426b`
    state
        .prog_runner
        .start(4, 5, STG4_FLAG_CHAINSAW_TAKEN as u16);
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
    assert_eq!(state.inventory_mgr.count(STG4_ITEM_CHAINSAW), 1);
    assert_eq!(
        state.prog_runner.active_message_id(),
        Some(STG4_MSG_GET_CHAINSAW)
    );
    assert_eq!(
        state.prog_runner.stop_sound_effects(),
        &[STG4_SE_CHAINSAW_IDLE]
    );
    assert_eq!(
        state.prog_runner.positional_sound_effects(),
        &[(STG4_SE_CHAINSAW_PICKUP, STG4_CHAINSAW_SE_POS)]
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
    assert!(state.flags.get(STG4_FLAG_CHAINSAW_TAKEN));

    // After pickup (`flag(38)`), `EvSoundCallAfterLoad` no longer starts `0x4268`;
    // when `flag(618)` muted, neither `0x2742` nor `0x4268` starts.
    let (se_2d_after, se_pos_after) = stage4_eval_sound_call_after_load(&state.flags);
    assert_eq!(se_2d_after, vec![STG4_SE_FOREST_WIND]);
    assert!(se_pos_after.is_empty());

    state.flags.set(STG4_FLAG_AMBIENT_MUTE, true);
    let (se_2d_muted, se_pos_muted) = stage4_eval_sound_call_after_load(&state.flags);
    assert!(se_2d_muted.is_empty() && se_pos_muted.is_empty());
}

#[test]
fn test_stage4_ev_prog_6_leave_ending_and_prog_7_trial_exit() {
    let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);

    // Without `flag(512)`, `ev_prog[6]` does not trigger Leave Ending
    state
        .prog_runner
        .start(4, 6, STG4_FLAG_LEAVE_ENDING_DONE as u16);
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
    assert!(!state.flags.get(STG4_FLAG_LEAVE_ENDING_DONE));

    // With `flag(512)`, `ev_prog[6]` requests `demo_no = 90`, SEs `0x3f4`/`0xeae1`, ending bit `0x01`
    state.flags.set(STG4_FLAG_LEAVE_ENDING_REQ, true);
    state
        .prog_runner
        .start(4, 6, STG4_FLAG_LEAVE_ENDING_DONE as u16);
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
        Some(STG4_LEAVE_ENDING_DEMO_NO)
    );
    assert_eq!(
        state.prog_runner.sound_effects(),
        &[STG4_SE_LEAVE_LETTER_A, STG4_SE_LEAVE_LETTER_B]
    );
    assert_eq!(state.prog_runner.take_triggered_ending(), Some(0x01));
    assert!(state.flags.get(STG4_FLAG_LEAVE_ENDING_DONE));

    // `ev_prog[7]` (`0x0059fb00`): plays `0x2744`
    state.prog_runner.start(4, 7, 0);
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
        state.prog_runner.sound_effects(),
        &[STG4_SE_TRIAL_GATE_EXIT]
    );
}

#[test]
fn test_stage4_ev_all_time_and_ev_bgm_control() {
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(0x1234_5678);
    let mut ambient = ForestAmbientState::init_from_rng(&mut rng);
    ambient.passer_timer = 0.01;

    // `EvBgmControl` (`0x005a0260`): `demo_no == 3` -> `4`, else `flag(36) ? 1 : 0`
    assert_eq!(
        act1_eval_bgm_control_with_demo_no(Act1StageId::Forest, &flags, 0, false, None),
        0
    );
    assert_eq!(
        act1_eval_bgm_control_with_demo_no(Act1StageId::Forest, &flags, 0, true, Some(3)),
        4
    );
    flags.set(STG4_FLAG_ANGELA_MET, true);
    assert_eq!(
        act1_eval_bgm_control_with_demo_no(Act1StageId::Forest, &flags, 0, false, None),
        1
    );

    // `EvAllTime` (`0x0059fcf0`): wind distance attenuation clamps to `0.001` at `(5000, 0, 5000)`
    let fx_atten = ambient.tick_all_time(
        [5000.0, 0.0, 5000.0],
        0.0,
        0.0,
        0.016,
        false,
        &flags,
        &mut rng,
    );
    assert!(!fx_atten.chainsaw_log_visible);
    assert!(!fx_atten.demo_prop_visible);
    assert!((fx_atten.wind_volume.unwrap() - STG4_WIND_MIN_VOLUME).abs() < 1e-6);

    // Crow `0x9c88` spawns in `x > 0, -90000 < z < -30000`
    ambient.crow_timer = 0.0;
    let fx_crow = ambient.tick_all_time(
        [10_000.0, 0.0, -50_000.0],
        0.0,
        0.0,
        0.016,
        true,
        &flags,
        &mut rng,
    );
    assert!(fx_crow.demo_prop_visible);
    assert!(fx_crow
        .spawned_positional_se
        .iter()
        .any(|&(id, _)| id == STG4_SE_AMBIENT_CROW));

    // Passing footsteps `0x9c87` spawns in `x > 0, z < -25000` when crow not playing & sweeps X
    ambient.crow_playing = false;
    ambient.passer_timer = 0.01;
    let fx_passer = ambient.tick_all_time(
        [10_000.0, 0.0, -40_000.0],
        0.0,
        0.0,
        0.02,
        false,
        &flags,
        &mut rng,
    );
    assert!(fx_passer
        .spawned_positional_se
        .iter()
        .any(|&(id, _)| id == STG4_SE_AMBIENT_PASSER));
    assert!(fx_passer
        .updated_positional_se
        .iter()
        .any(|&(id, pos)| id == STG4_SE_AMBIENT_PASSER && pos[0] > 9000.0));

    // Rustle `0x9c86` spawns in `x < -20000` & yaw in `(-3π/4, -π/4)` using `cos` for X & `sin`
    // for Z (`0x005a0050`/`0x005a0075`), and stops when `speed < 1.2` (`0x005a0120..0x005a0156`).
    ambient.rustle_timer = 0.0;
    let fx_rustle = ambient.tick_all_time(
        [-30_000.0, 0.0, 0.0],
        -1.5,
        2.0,
        0.016,
        false,
        &flags,
        &mut rng,
    );
    let rustle_pos = fx_rustle
        .spawned_positional_se
        .iter()
        .find_map(|&(id, pos)| (id == STG4_SE_AMBIENT_RUSTLE).then_some(pos))
        .expect("rustle SE 0x9c86 spawned");
    let expected_x = -30_000.0 + ambient.rustle_angle.cos() * STG4_RUSTLE_RADIUS_MM;
    let expected_z = ambient.rustle_angle.sin() * STG4_RUSTLE_RADIUS_MM;
    assert!((rustle_pos[0] - expected_x).abs() < 1e-3);
    assert!((rustle_pos[2] - expected_z).abs() < 1e-3);
    assert!(fx_rustle.stopped_se.is_empty());

    let fx_stop = ambient.tick_all_time(
        [-30_000.0, 0.0, 0.0],
        -1.5,
        0.5,
        0.016,
        false,
        &flags,
        &mut rng,
    );
    assert_eq!(fx_stop.stopped_se, vec![STG4_SE_AMBIENT_RUSTLE]);
}

#[test]
fn test_stage4_retail_end_to_end_progression_and_assets() {
    let Some(data_root) = retail_data_root() else {
        eprintln!("Skipping retail asset integration test: retail data/ not found");
        return;
    };
    let game_root = data_root.parent().unwrap();

    // 1. Verify real retail Forest assets decode cleanly (`stage_forest_msg_e.mes`, `p_redpaper.tex`,
    //    `agl.mdl`, `x_wp_csaw.mdl`, `forest/bgm_114_b.aix`, `forest/bgm_115.aix`, `ca*.cld`).
    let mes_bytes = std::fs::read(data_root.join("etc/message/stage_forest_msg_e.mes"))
        .expect("read stage_forest_msg_e.mes");
    let mes = parse_mes(&mes_bytes).expect("parse stage_forest_msg_e.mes");
    assert!(
        mes.messages.len() >= 5,
        "expected >= 5 forest messages, got {}",
        mes.messages.len()
    );

    let redpaper_bytes =
        std::fs::read(data_root.join("pic/out/p_redpaper.tex")).expect("read p_redpaper.tex");
    assert!(!redpaper_bytes.is_empty());

    let agl_bytes = std::fs::read(data_root.join("chr/agl/agl.mdl")).expect("read agl.mdl");
    let agl_mdl = parse_mdl(&agl_bytes).expect("parse agl.mdl");
    assert!(agl_mdl.bone_count > 0);

    let csaw_bytes =
        std::fs::read(data_root.join("chr/item/x_wp_csaw.mdl")).expect("read x_wp_csaw.mdl");
    let csaw_mdl = parse_mdl(&csaw_bytes).expect("parse x_wp_csaw.mdl");
    assert!(csaw_mdl.bone_count > 0);

    let bgm_114_b_bytes = std::fs::read(data_root.join("sound/adx/forest/bgm_114_b.aix"))
        .expect("read bgm_114_b.aix");
    let bgm_114_b = decode_aix(&bgm_114_b_bytes).expect("decode bgm_114_b.aix");
    assert!(bgm_114_b.len() >= 4);

    let bgm_115_bytes =
        std::fs::read(data_root.join("sound/adx/forest/bgm_115.aix")).expect("read bgm_115.aix");
    let bgm_115 = decode_aix(&bgm_115_bytes).expect("decode bgm_115.aix");
    assert!(!bgm_115.is_empty());

    // Inspect retail Forest `.cld` floor surface materials across `ca*.cld`
    let mut saw_concrete = false;
    let mut saw_grass = false;
    let mut saw_gravel_or_dirt = false;
    for entry in std::fs::read_dir(data_root.join("bg/ca")).expect("read bg/ca") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("cld") {
            continue;
        }
        let cld_bytes = std::fs::read(&path).expect("read ca*.cld");
        let cld = parse_cld(&cld_bytes).expect("parse ca*.cld");
        for tri in &cld.floors {
            match SurfaceMaterial::from_cld_material(tri.material) {
                SurfaceMaterial::Concrete => saw_concrete = true,
                SurfaceMaterial::Grass => saw_grass = true,
                SurfaceMaterial::Gravel => saw_gravel_or_dirt = true,
                _ => {}
            }
        }
    }
    assert!(
        saw_concrete,
        "expected Concrete/Stone material (.cld 0/12) in retail ca*.cld"
    );
    assert!(
        saw_grass,
        "expected Grass material (.cld 2) in retail ca*.cld"
    );
    assert!(
        saw_gravel_or_dirt,
        "expected Dirt/Gravel material (.cld 1/3/4) in retail ca*.cld"
    );

    // 2. End-to-end campaign flow: Stage 3 (`ob01`) -> Stage 4 (`ca11`) -> Well Save ->
    //    real `haka_agl.dds` Angela Graveyard cutscene -> Stage 5 (`cb54`) + `ca11 -> ob01` return.
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 4;
    sim.scenario.current_room = "ca11".to_string();
    let mut orch = CampaignOrchestrator::from_saved_stage(
        4,
        Some("ca11".to_string()),
        [24_000.0, 0.0, -95_000.0],
        0.0,
        OutdoorStreamingMode::default(),
        false,
    );

    // Load Stage 4 -> verifies `EvSoundCallAfterLoad` emits `0x2742` wind SE
    let idle = CampaignTickInput::default();
    let mut saw_wind_se = false;
    for _ in 0..12 {
        if matches!(orch.phase, CampaignPhase::RoomLoading { .. }) {
            orch.notify_room_assets_loaded();
        }
        let eff = orch.step(&mut sim, &idle, 1.0 / 60.0);
        if eff
            .looping_se_starts
            .iter()
            .any(|&(id, _)| id == STG4_SE_FOREST_WIND)
        {
            saw_wind_se = true;
        }
        if matches!(orch.phase, CampaignPhase::ActiveGameplay) {
            break;
        }
    }
    assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));
    assert!(
        saw_wind_se,
        "Stage 4 RoomLoading must emit EvSoundCallAfterLoad wind SE 0x2742"
    );

    // Well Save Point (`ev_prog[1]`) -> opens Save Menu overlay cleanly
    sim.scenario.prog_runner.start(4, 1, 0);
    let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
    // Advance cutaway fade-out & fade-in
    for _ in 0..120 {
        let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
        if sim.scenario.cutaway_overlay.active_picture() == Some(STG4_WELL_PICTURE)
            && orch.fade.is_fully_clear()
        {
            break;
        }
    }
    let press_a = CampaignTickInput {
        action_pressed: true,
        ..CampaignTickInput::default()
    };
    let eff_msg = orch.step(&mut sim, &press_a, 1.0 / 60.0);
    assert_eq!(eff_msg.stage_message, Some(STG4_MSG_WELL_SAVE as u16));
    let eff_save = orch.step(&mut sim, &press_a, 1.0 / 60.0);
    assert!(eff_save.sound_effects.contains(&STG4_SE_SAVE_CHIME));
    assert!(matches!(
        orch.phase,
        CampaignPhase::OverlayMenu(OverlayMenuKind::Save)
    ));
    // Close Save Menu
    let _ = orch.step(
        &mut sim,
        &CampaignTickInput {
            cancel_pressed: true,
            ..CampaignTickInput::default()
        },
        1.0 / 60.0,
    );
    assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));

    // Trigger Angela Graveyard Cutscene (`ev_prog[2]` -> `demo_no = 3`, real `haka_agl.dds`)
    sim.scenario
        .prog_runner
        .start(4, 2, STG4_FLAG_ANGELA_MET as u16);
    let eff_start_demo = orch.step(&mut sim, &idle, 1.0 / 60.0);
    assert!(matches!(
        orch.phase,
        CampaignPhase::DramaDemo { demo_no: 3, .. }
    ));
    assert_eq!(eff_start_demo.bgm_control_mask, Some(4));

    // Drive REAL `haka_agl.dds` through `DramaDemoPlayer` frame-by-frame to natural completion!
    let haka_info = find_drama_demo_by_no(3).expect("demo_no 3 (haka_agl) in catalog");
    let haka_path = game_root.join(haka_info.dds_path);
    let haka_bytes = std::fs::read(&haka_path).expect("read haka_agl.dds");
    let haka_script = parse_demo_dds(&haka_bytes).expect("parse haka_agl.dds");
    let mut player = DramaDemoPlayer::new(*haka_info, haka_script, false);
    assert!(player.total_frames() > 100);

    let mut ticks = 0u32;
    while player.step != DramaDemoStep::Finished && ticks < 20_000 {
        player.tick(1.0 / 30.0, true);
        let eff = orch.step(
            &mut sim,
            &CampaignTickInput {
                drama_demo_frame: Some(player.scene_frame),
                drama_demo_finished: player.step == DramaDemoStep::Finished,
                ..CampaignTickInput::default()
            },
            1.0 / 30.0,
        );
        if player.step != DramaDemoStep::Finished {
            assert_eq!(eff.bgm_control_mask, Some(4));
        }
        ticks += 1;
    }
    assert_eq!(player.step, DramaDemoStep::Finished);
    assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));
    assert!(sim.scenario.flags.get(STG4_FLAG_ANGELA_MET));
    assert_eq!(
        sim.scenario.player_position,
        Vec3::from_array(STG4_JAMES_POST_DEMO3_POS)
    );
    assert!((sim.scenario.player_yaw - STG4_JAMES_DEMO3_YAW).abs() < 1e-5);

    // Post-cutscene `EvBgmControl` switches from `4` -> `1`
    let eff_post = orch.step(&mut sim, &idle, 1.0 / 60.0);
    assert_eq!(eff_post.bgm_control_mask, Some(1));

    // Verify Stage 4 -> Stage 5 (`cb54` Town East) and Stage 4 -> Stage 3 (`ob01`) manifests
    let town_east_manifest = resolve_stage_manifest(5, Some("cb54"), false, true);
    assert_eq!(town_east_manifest.stage_id, 5);
    assert_eq!(town_east_manifest.room_id, "cb54");
    assert!(data_root.join(&town_east_manifest.cld_rel_path).is_file());

    let ob01_manifest = resolve_stage_manifest(3, Some("ob01"), false, true);
    assert_eq!(ob01_manifest.stage_id, 3);
    assert_eq!(ob01_manifest.room_id, "ob01");
    assert!(data_root.join(&ob01_manifest.cld_rel_path).is_file());
}
