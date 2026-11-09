//! Integration tests for `SH2-105`:
//! - Wood Side Rm 307 closet cutscene slat darkening (`TimeOfDay = 15`), DoF blur (`ap18_ap21_Closet`),
//!   flashlight suppression, `b_doo` door replacement (`closetXForm`), and bonus audio trigger (`ClosetCutscene.cpp` & `SpecialFX.cpp`).
//! - Abstract Daddy 24-piston array kinematics (`0x01D83570..0x01D835D0`), `PistonList[1]` `+2.50` housing retraction latch,
//!   Flesh Lips death/cutscene motion blur, and universal PS2 motion blur / DoF scaling (`PistonRoom.cpp` & `SpecialFX.cpp`).
//! - Lakeview Hotel Room 312 pre-VHS sunset grading (`RGB(255, 255, 227)` + zero fog density), Leave/Maria ending
//!   graveyard tree lighting (`mode = 2` + `RGB(125, 135, 150)` horizon fog), and Final Boss draw distance (`TreeLighting.cpp` & `FinalBossRoomFix.cpp`).

use sh2_assets::formats::demo::{
    parse_demo_dds, CutsceneWidescreenMode, STANDARD_16X9_ASPECT, ULTRAWIDE_21X9_ASPECT,
};
use sh2_core::{
    evaluate_eddie_boss_death_blur, evaluate_mary_boss_blur_factor, find_drama_demo_by_id,
    AtmosphereEvalInput, AtmosphereGradingEvaluation, ClosetDoorReplacementState, DramaDemoPlayer,
    FinalBossRoomFixState, PistonArrayRuntime, PistonAxisDir, ScreenTransitionState,
    SpecialFxEvalInput, SpecialFxEvaluation, TreeLightingMode, CLOSET_BLURRED_BARS_HIDE_CAM_X,
    CLOSET_BONUS_AUDIO_TRIGGER_FRAME, CLOSET_BONUS_AUDIO_WAV_PATH, CLOSET_DARKENED_AMBIENT_SCALE,
    CLOSET_DISABLE_FLASHLIGHT_CAM_X, CLOSET_DOOR_HIDE_END_FRAME, CLOSET_DOOR_HIDE_START_FRAME,
    CLOSET_DOOR_WORLD_MATRIX, CS_APT_RPT_CLOSET_DEMO_NO, CS_END_LEAVE_LETTER_DEMO_NO,
    CS_END_MARIA_LETTER_DEMO_NO, CS_HSP_BOSS_FINISH_DEMO_NO, CS_INTRO_BATHROOM_DEMO_NO,
    CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, CS_PS_ANGELA_BOSS_START_DEMO_NO, CS_PS_MARIA_MEET_DEMO_NO,
    CS_TUNNEL_RADIO_DEMO_NO, DOORWAY_PISTON_INDEX, FLESH_LIPS_CUTSCENE_MOTION_BLUR_FACTOR,
    FLESH_LIPS_DEATH_MOTION_BLUR_FACTOR, HOTEL_ROOM_312_SUNSET_TINT_RGB,
    MARIA_ENDING_HORIZON_FOG_RGB, PISTON_ARRAY_COUNT, PISTON_AXIS_DIR_TABLE,
    PISTON_DOORWAY_PROP_OFFSET, PISTON_ROOM_DOOR_OPEN_CAM_X, PISTON_SPEED_TABLE,
    PS2_MOTION_BLUR_SCALE, R_APT_E_RM_307_ID, R_FINAL_BOSS_RM_ID, R_HTL_RM_312_ID, R_LAB_TOP_H_ID,
    TIME_OF_DAY_APARTMENT_DAY, TIME_OF_DAY_CLOSET_DARKENED,
};

#[test]
fn test_wood_side_rm_307_closet_cutscene_grading_dof_door_and_audio() {
    // 1. During CS_APT_RPT_CLOSET (demo_no = 14), TimeOfDay switches from 11 to 15 (darkened slats)
    let grading_in_closet = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        room_id: R_APT_E_RM_307_ID,
        demo_no: CS_APT_RPT_CLOSET_DEMO_NO,
        scene_frame: 500.0,
        camera_pos_x: CLOSET_DISABLE_FLASHLIGHT_CAM_X,
        viewport_aspect: STANDARD_16X9_ASPECT,
        ..AtmosphereEvalInput::default()
    });
    assert_eq!(
        grading_in_closet.time_of_day_preset,
        TIME_OF_DAY_CLOSET_DARKENED
    );
    assert!((grading_in_closet.ambient_tint_rgba[3] - CLOSET_DARKENED_AMBIENT_SCALE).abs() < 1e-6);
    assert!(grading_in_closet.flashlight_suppressed);

    // Outside the cutscene, TimeOfDay restores to 11 (ambient scale 1.0)
    let grading_after_closet = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        room_id: R_APT_E_RM_307_ID,
        demo_no: 0,
        scene_frame: 0.0,
        camera_pos_x: -21_000.0,
        viewport_aspect: STANDARD_16X9_ASPECT,
        ..AtmosphereEvalInput::default()
    });
    assert_eq!(
        grading_after_closet.time_of_day_preset,
        TIME_OF_DAY_APARTMENT_DAY
    );
    assert!((grading_after_closet.ambient_tint_rgba[3] - 1.0).abs() < 1e-6);
    assert!(!grading_after_closet.flashlight_suppressed);

    // 2. DoF filter (`ap18_ap21_Closet`) at 4K (2160p, display_ratio = 2.0) and 21:9 ultrawide:
    let fx_closet_bars = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        room_id: R_APT_E_RM_307_ID,
        demo_no: CS_APT_RPT_CLOSET_DEMO_NO,
        scene_frame: 600.0,
        camera_pos_x: CLOSET_BLURRED_BARS_HIDE_CAM_X,
        viewport_height: 2160.0,
        viewport_aspect: ULTRAWIDE_21X9_ASPECT,
        ..SpecialFxEvalInput::default()
    });
    assert!((fx_closet_bars.display_ratio - 2.0).abs() < 1e-6);
    assert!(fx_closet_bars.dof_active);
    assert!(fx_closet_bars.closet_dof_byte_write_suppressed);
    assert!(fx_closet_bars.render_grain_above_blur);
    assert!((fx_closet_bars.dof_tap_offset_px[1] - (-3.0)).abs() < 1e-5);
    assert!(fx_closet_bars.dof_tap_offset_px[0] > 3.0);

    // 3. `b_doo` closet door replacement (`closetXForm`) visibility window (< 359.5 || > 1695.0) & bonus audio (> 1900.0)
    let door_before = ClosetDoorReplacementState::evaluate(
        R_APT_E_RM_307_ID,
        CS_APT_RPT_CLOSET_DEMO_NO,
        350.0,
        true,
    );
    assert!(door_before.render_replacement_door);
    assert_eq!(door_before.world_matrix, CLOSET_DOOR_WORLD_MATRIX);
    assert!(!door_before.bonus_audio_triggered);

    let door_inside_pov = ClosetDoorReplacementState::evaluate(
        R_APT_E_RM_307_ID,
        CS_APT_RPT_CLOSET_DEMO_NO,
        (CLOSET_DOOR_HIDE_START_FRAME + CLOSET_DOOR_HIDE_END_FRAME) * 0.5,
        true,
    );
    assert!(!door_inside_pov.render_replacement_door);

    let door_after_with_audio = ClosetDoorReplacementState::evaluate(
        R_APT_E_RM_307_ID,
        CS_APT_RPT_CLOSET_DEMO_NO,
        CLOSET_BONUS_AUDIO_TRIGGER_FRAME + 10.0,
        true,
    );
    assert!(door_after_with_audio.render_replacement_door);
    assert!(door_after_with_audio.bonus_audio_triggered);
    assert_eq!(
        CLOSET_BONUS_AUDIO_WAV_PATH,
        "data/sound/adx/amb/sankaku_toujyou_2.wav"
    );
}

#[test]
fn test_abstract_daddy_24_piston_array_kinematics_and_doorway_retraction_latch() {
    assert_eq!(PISTON_SPEED_TABLE.len(), PISTON_ARRAY_COUNT);
    assert_eq!(PISTON_AXIS_DIR_TABLE.len(), PISTON_ARRAY_COUNT);
    assert_eq!(PISTON_AXIS_DIR_TABLE[0], PistonAxisDir::PosZ);
    assert_eq!(
        PISTON_AXIS_DIR_TABLE[DOORWAY_PISTON_INDEX],
        PistonAxisDir::PosX
    );

    let mut pistons = PistonArrayRuntime::new();
    let initial_phases = pistons.phases;

    // 1. Before Angela opens the door (CameraX != 20278.414), all 24 pistons oscillate along their axes
    pistons.step(0.5, CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, 19_500.0);
    assert!(!pistons.fix_state.doorway_piston_culled);
    for (&phase, &init_phase) in pistons.phases.iter().zip(&initial_phases) {
        assert!((phase - init_phase).abs() > 1e-4);
    }

    // 2. When CameraX hits 20278.414 (`0x469E6CD4`), PistonList[1] latches at +2.50 (`0x40200000`)
    pistons.step(
        0.5,
        CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        PISTON_ROOM_DOOR_OPEN_CAM_X,
    );
    assert!(pistons.fix_state.doorway_piston_culled);
    assert!((pistons.phases[DOORWAY_PISTON_INDEX] - PISTON_DOORWAY_PROP_OFFSET).abs() < 1e-6);
    let latched_disp_1 = pistons.displacements[DOORWAY_PISTON_INDEX];

    // 3. Subsequent camera cuts keep PistonList[1] retracted at +2.50 while pistons 0 and 2..24 keep moving
    let prev_phase_0 = pistons.phases[0];
    pistons.step(0.5, CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, 18_200.0);
    assert!(pistons.fix_state.doorway_piston_culled);
    assert!((pistons.phases[DOORWAY_PISTON_INDEX] - PISTON_DOORWAY_PROP_OFFSET).abs() < 1e-6);
    assert_eq!(pistons.displacements[DOORWAY_PISTON_INDEX], latched_disp_1);
    assert!((pistons.phases[0] - prev_phase_0).abs() > 1e-4);
}

#[test]
fn test_flesh_lips_and_boss_cutscene_ps2_motion_blur_and_dof_scaling() {
    // 1. Flesh Lips boss death (`enIKECtrlDead` at `0x004B9EA5`): active only when enemies_remaining == 0,
    // demo_no == 0, and death sub-state in {2, 4, 6}
    for sub in [2u8, 4, 6] {
        let fx = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
            demo_no: 0,
            enemies_remaining: 0,
            flesh_lips_death_substate: Some(sub),
            ..SpecialFxEvalInput::default()
        });
        assert_eq!(
            fx.raw_motion_blur_factor,
            FLESH_LIPS_DEATH_MOTION_BLUR_FACTOR
        );
        assert!(
            (fx.scaled_motion_blur_factor
                - FLESH_LIPS_DEATH_MOTION_BLUR_FACTOR * PS2_MOTION_BLUR_SCALE)
                .abs()
                < 1e-5
        );
        assert!(fx.motion_blur_strength > 0.30);
    }

    // Inactive when another Flesh Lips is still alive (`enemies_remaining > 0`) or in non-blur sub-state
    let fx_still_alive = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: 0,
        enemies_remaining: 1,
        flesh_lips_death_substate: Some(2),
        ..SpecialFxEvalInput::default()
    });
    assert_eq!(fx_still_alive.raw_motion_blur_factor, 0.0);

    // 2. Flesh Lips post-boss cutscene (`CS_HSP_BOSS_FINISH = 0x2A`, `demo_no = 42`):
    // factor = 300 (`75.0` scaled) and `force_motion_blur_alpha_lsb == true`
    let fx_hsp_finish = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_HSP_BOSS_FINISH_DEMO_NO,
        scene_frame: 120.0,
        ..SpecialFxEvalInput::default()
    });
    assert_eq!(
        fx_hsp_finish.raw_motion_blur_factor,
        FLESH_LIPS_CUTSCENE_MOTION_BLUR_FACTOR
    );
    assert!((fx_hsp_finish.scaled_motion_blur_factor - 75.0).abs() < 1e-5);
    assert!(fx_hsp_finish.force_motion_blur_alpha_lsb);

    // 3. Angela Abstract Daddy (`CS_PS_ANGELA_BOSS_START = 67` & `CS_PS_ANGELA_BOSS_FINISH = 68`) ramps:
    let fx_angela_r1_start = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        scene_frame: 1.0,
        ..SpecialFxEvalInput::default()
    });
    assert!((fx_angela_r1_start.raw_motion_blur_factor - 400.0).abs() < 1e-5);

    let fx_angela_r1_end = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        scene_frame: 191.0,
        ..SpecialFxEvalInput::default()
    });
    assert!((fx_angela_r1_end.raw_motion_blur_factor - 30.0).abs() < 1e-5);

    let fx_angela_r4_mid = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_PS_ANGELA_BOSS_START_DEMO_NO,
        scene_frame: 105.5,
        ..SpecialFxEvalInput::default()
    });
    assert!((fx_angela_r4_mid.raw_motion_blur_factor - 280.0).abs() < 1e-4);

    // 4. Maria Jail Cell (`CS_PS_MARIA_MEET = 52`) 3-tier motion blur (80, 100, 400)
    let fx_maria_1 = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_PS_MARIA_MEET_DEMO_NO,
        scene_frame: 100.0,
        ..SpecialFxEvalInput::default()
    });
    let fx_maria_3 = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_PS_MARIA_MEET_DEMO_NO,
        scene_frame: 2000.0,
        ..SpecialFxEvalInput::default()
    });
    assert_eq!(fx_maria_1.raw_motion_blur_factor, 80.0);
    assert_eq!(fx_maria_3.raw_motion_blur_factor, 400.0);

    // 5. Eddie Boss death timer (`* 0.375`, `>= 8.0`) & Mary Boss blur (`* 1.25 * 0.25`)
    let (eddie_t1, done1) = evaluate_eddie_boss_death_blur(0.0, 16.0);
    assert!((eddie_t1 - 6.0).abs() < 1e-5);
    assert!(!done1);
    let (eddie_t2, done2) = evaluate_eddie_boss_death_blur(eddie_t1, 8.0);
    assert!((eddie_t2 - 9.0).abs() < 1e-5);
    assert!(done2);
    assert!((evaluate_mary_boss_blur_factor(160.0) - 50.0).abs() < 1e-5);

    // 6. Resolution-independent DoF offsets for Bathroom (`demo_no = 1`), Tunnel Radio (`demo_no = 6`), and Room 312 (`0xA2`)
    let fx_bathroom_1440p = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_INTRO_BATHROOM_DEMO_NO,
        viewport_height: 1440.0,
        ..SpecialFxEvalInput::default()
    });
    let ratio_1440 = 1440.0 / 1080.0;
    assert!((fx_bathroom_1440p.dof_tap_offset_px[0] - (-3.0 * ratio_1440)).abs() < 1e-5);
    assert!((fx_bathroom_1440p.dof_tap_offset_px[1] - (-1.5 * ratio_1440)).abs() < 1e-5);

    let fx_tunnel_1080p = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
        demo_no: CS_TUNNEL_RADIO_DEMO_NO,
        viewport_height: 1080.0,
        ..SpecialFxEvalInput::default()
    });
    assert_eq!(fx_tunnel_1080p.dof_tap_offset_px, [-3.0, -1.0]);
}

#[test]
fn test_room_312_sunset_grading_leave_maria_tree_lighting_and_final_boss_draw_distance() {
    // 1. Hotel Room 312 (`R_HTL_RM_312 = 0xA2`) before watching the VHS tape (`event_index != 0`):
    // Warm sunset tint `RGB(255, 255, 227)` + zero fog density (`RunHotelRoom312FogVolumeFix`)
    let rm312_pre_vhs = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        room_id: R_HTL_RM_312_ID,
        event_index: 1,
        transition_state: ScreenTransitionState::Normal,
        ..AtmosphereEvalInput::default()
    });
    assert!(rm312_pre_vhs.room_312_sunset_active);
    assert_eq!(
        [
            rm312_pre_vhs.ambient_tint_rgba[0],
            rm312_pre_vhs.ambient_tint_rgba[1],
            rm312_pre_vhs.ambient_tint_rgba[2]
        ],
        HOTEL_ROOM_312_SUNSET_TINT_RGB
    );
    assert_eq!(rm312_pre_vhs.fog_density_scale, 0.0);

    // After watching the VHS tape (`event_index == 0`) or during `FadeBlackSrc`, sunset grading is off
    let rm312_post_vhs = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        room_id: R_HTL_RM_312_ID,
        event_index: 0,
        transition_state: ScreenTransitionState::Normal,
        ..AtmosphereEvalInput::default()
    });
    assert!(!rm312_post_vhs.room_312_sunset_active);
    assert_eq!(rm312_post_vhs.ambient_tint_rgba, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(rm312_post_vhs.fog_density_scale, 1.0);

    // 2. Leave (`0x5A` / `90`) & Maria (`0x5D` / `93`) Ending Graveyard Tree Lighting & Horizon Fog
    let leave_ending = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        demo_no: CS_END_LEAVE_LETTER_DEMO_NO,
        ..AtmosphereEvalInput::default()
    });
    assert_eq!(
        leave_ending.tree_lighting_mode,
        TreeLightingMode::EndingGraveyard
    );
    assert_eq!(leave_ending.horizon_fog_rgb_override, None);

    let maria_ending = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        demo_no: CS_END_MARIA_LETTER_DEMO_NO,
        ..AtmosphereEvalInput::default()
    });
    assert_eq!(
        maria_ending.tree_lighting_mode,
        TreeLightingMode::EndingGraveyard
    );
    assert_eq!(
        maria_ending.horizon_fog_rgb_override,
        Some(MARIA_ENDING_HORIZON_FOG_RGB)
    );

    // 3. Final Boss Room (`R_FINAL_BOSS_RM = 0xBB` / `ru73`) draw distance and spawn thresholds
    let boss_room_atm = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
        room_id: R_FINAL_BOSS_RM_ID,
        james_pos_y: -15_000.0,
        ..AtmosphereEvalInput::default()
    });
    assert_eq!(boss_room_atm.far_clip_override, Some(30_000.0));
    let boss_fix = FinalBossRoomFixState::evaluate(R_FINAL_BOSS_RM_ID, -15_000.0);
    assert_eq!(boss_fix.bottom_walkway_spawn_y, -30_000.0);
    assert_eq!(boss_fix.bottom_floor_spawn_y, -30_000.0);
    assert_eq!(boss_fix.black_box_spawn_y, -30_000.0);
    assert_eq!(boss_fix.draw_distance, 30_000.0);
}

#[test]
fn test_drama_demo_player_end_to_end_sh2_105_with_retail_sankaku1_and_papa_agl() {
    let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
    let sankaku_path = format!("{root}/data/demo/sankaku1/sankaku.dds");
    if !std::path::Path::new(&sankaku_path).exists() {
        return;
    }

    // 1. Wood Side Room 307 Closet (`sankaku1`, `demo_no = 14`)
    let sankaku_info = find_drama_demo_by_id("sankaku1").expect("sankaku1 in catalog");
    let sankaku_dds = parse_demo_dds(&std::fs::read(&sankaku_path).unwrap()).unwrap();
    let mut closet_player = DramaDemoPlayer::new_with_widescreen(
        *sankaku_info,
        sankaku_dds,
        ULTRAWIDE_21X9_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    closet_player.set_viewport_dimensions(
        3440.0,
        1440.0,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );

    closet_player.seek_to_frame(300.0, true);
    assert_eq!(
        closet_player.atmosphere.time_of_day_preset,
        TIME_OF_DAY_CLOSET_DARKENED
    );
    assert!(closet_player.special_fx.dof_active);
    assert!(closet_player.closet_door.render_replacement_door);

    closet_player.seek_to_frame(800.0, true);
    assert!(!closet_player.closet_door.render_replacement_door);

    closet_player.seek_to_frame(1950.0, true);
    assert!(closet_player.closet_door.render_replacement_door);
    assert!(closet_player.closet_door.bonus_audio_triggered);

    // 2. Labyrinth Abstract Daddy / Angela (`papa_agl`, `demo_no = 68`)
    let papa_info = find_drama_demo_by_id("papa_agl").expect("papa_agl in catalog");
    let papa_path = format!("{root}/{}", papa_info.dds_path);
    let papa_dds = parse_demo_dds(&std::fs::read(&papa_path).unwrap()).unwrap();
    let mut papa_player = DramaDemoPlayer::new_with_widescreen(
        *papa_info,
        papa_dds,
        ULTRAWIDE_21X9_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );

    // Frame 100 is inside Ramp 1 (1.0..=191.0, 400.0 -> 30.0) before the doorway shot
    papa_player.seek_to_frame(100.0, true);
    assert!(papa_player.special_fx.raw_motion_blur_factor > 150.0);
    assert!(!papa_player.piston_array.fix_state.doorway_piston_culled);

    // Scrub to the end of papa_agl after Angela opens the exit door (`CameraX == 20278.414`)
    papa_player.seek_to_frame((papa_player.total_frames() - 10) as f32, true);
    assert!(papa_player.piston_array.fix_state.doorway_piston_culled);
    assert!(
        (papa_player.piston_array.phases[DOORWAY_PISTON_INDEX] - PISTON_DOORWAY_PROP_OFFSET).abs()
            < 1e-6
    );
    assert_eq!(
        sh2_core::resolve_cutscene_room_id(papa_info.demo_no, papa_info.stage_name),
        R_LAB_TOP_H_ID
    );
}
