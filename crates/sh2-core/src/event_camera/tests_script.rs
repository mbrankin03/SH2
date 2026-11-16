//! Unit tests for `SH2-52` scripted event & `.mes` dialogue camera overrides, standalone
//! multi-tick `apply_tick` convergence, and `.cam` warp-cut insulation during `SmoothBlend`.

use super::*;
use crate::camera::Ps2CameraController;
use crate::events::{CameraOverrideCommand, APART_E3FE_ROOM13_CAMERA};
use glam::Vec3;
use sh2_assets::formats::cam::{
    AreaSizeType, CamFile, CamMovementType, RoadArea, RoadRecord, RoadType, VC_RD_WARP_IN_F,
    VC_RD_WARP_OUT_F,
};

#[test]
fn test_script_camera_override_angela_cemetery_eddie_bowling_and_mes_dialogue() {
    let mut sys = EventCameraSystem::new();
    let mut cam = Ps2CameraController::new();
    let _ = cam.load_room_cam("ca11", false);
    let player_pos = Vec3::new(8_000.0, 0.0, -11_000.0);
    cam.reset_to_player(player_pos, 0.0);
    let auto_eye = cam.curr_snapshot.eye;

    // 1. Angela Forest Cemetery scripted shot (`FOREST_CEMETERY_ANGELA_CAM`, `sh2pc.exe` `0x008e77ac`).
    sys.script_camera.set_camera_script(
        FOREST_CEMETERY_ANGELA_CAM,
        ScriptTransitionKind::WarpCut,
        cam.curr_snapshot.fov_y,
    );
    assert_eq!(sys.active_authority(), EventCameraAuthority::ScriptOverride);
    let _ = sys.tick(
        &mut cam,
        player_pos,
        0.0,
        0.8,
        -0.5,
        1.0 / 60.0,
        PlayerCameraSuppression::Active,
        &[],
        None,
    );
    assert!((cam.curr_snapshot.eye - FOREST_CEMETERY_ANGELA_CAM.eye_mm).length() < 1e-2);
    assert_eq!(cam.prev_snapshot, cam.curr_snapshot);
    assert_eq!(cam.search_yaw_offset, 0.0);

    // 2. Smooth blend into Eddie Bowling Alley tilted shot (`BOWLING_ALLEY_EDDIE_CAM_1`).
    sys.script_camera.set_camera_script(
        BOWLING_ALLEY_EDDIE_CAM_1,
        ScriptTransitionKind::SmoothBlend,
        cam.curr_snapshot.fov_y,
    );
    let prev_dist = (cam.curr_snapshot.eye - BOWLING_ALLEY_EDDIE_CAM_1.eye_mm).length();
    for _ in 0..10 {
        let _ = sys.tick(
            &mut cam,
            player_pos,
            0.0,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            None,
        );
    }
    assert!((cam.curr_snapshot.eye - BOWLING_ALLEY_EDDIE_CAM_1.eye_mm).length() < prev_dist);
    assert!(cam.curr_snapshot.up.x.abs() > 1e-3);

    // 3. `vcReturnPreAutoCamWork(1)` warp release restores exact `.cam` rail framing.
    sys.script_camera.return_pre_auto_cam_work(
        ScriptTransitionKind::WarpCut,
        &mut cam,
        player_pos,
        0.0,
    );
    assert_eq!(sys.active_authority(), EventCameraAuthority::GameplayRail);
    assert!((cam.curr_snapshot.eye - auto_eye).length() < 1.0);
    assert_eq!(cam.prev_snapshot, cam.curr_snapshot);

    // 4. `.mes` dialogue hold + `APART_E3FE_ROOM13_CAMERA` stage override.
    sys.script_camera
        .begin_mes_dialogue_camera(42, &mut cam, None);
    let held = cam.curr_snapshot;
    let _ = sys.tick(
        &mut cam,
        player_pos + Vec3::new(300.0, 0.0, 0.0),
        0.5,
        1.0,
        1.0,
        1.0 / 60.0,
        PlayerCameraSuppression::Active,
        &[],
        None,
    );
    assert_eq!(cam.curr_snapshot, held);
    sys.apply_stage_override(
        CameraOverrideCommand::Lock(APART_E3FE_ROOM13_CAMERA),
        &mut cam,
        player_pos,
        0.0,
    );
    assert!((cam.curr_snapshot.eye.x - (-21_966.0)).abs() < 1.0);
    sys.apply_stage_override(CameraOverrideCommand::Release, &mut cam, player_pos, 0.0);
    assert_eq!(sys.active_authority(), EventCameraAuthority::GameplayRail);
}

#[test]
fn test_standalone_apply_tick_multi_tick_convergence_and_prev_snapshot_advancement() {
    let mut cam = Ps2CameraController::new();
    let player_pos = Vec3::new(0.0, 0.0, 0.0);
    cam.reset_to_player(player_pos, 0.0);
    let tick0_snap = cam.curr_snapshot;

    let target_shot = ScriptedCameraShot {
        eye_mm: Vec3::new(600.0, -1500.0, -1200.0),
        watch_mm: Vec3::new(200.0, -900.0, 400.0),
        roll_radians: 0.05,
        projection_scr_z: Some(600.0),
    };
    let mut sc = ScriptCameraController::new();
    sc.set_camera_script(
        target_shot,
        ScriptTransitionKind::SmoothBlend,
        tick0_snap.fov_y,
    );

    // Tick 1 standalone: prev_snapshot == tick0_snap, curr_snapshot steps toward target_shot.
    sc.apply_tick(&mut cam, 1.0 / 60.0);
    let tick1_curr = cam.curr_snapshot;
    assert_eq!(cam.prev_snapshot, tick0_snap);
    assert_ne!(tick1_curr.eye, tick0_snap.eye);

    // Tick 2 standalone: prev_snapshot MUST advance to tick1_curr (never stuck at tick0_snap!).
    sc.apply_tick(&mut cam, 1.0 / 60.0);
    assert_eq!(
        cam.prev_snapshot, tick1_curr,
        "Standalone apply_tick must advance camera.prev_snapshot on every tick"
    );
    assert!(cam.interpolated_view_matrix(0.5).is_finite());

    // Converge to exact target shot over standalone ticks and retain target FOV after 30 ticks.
    for _ in 0..180 {
        sc.apply_tick(&mut cam, 1.0 / 60.0);
    }
    assert!((cam.curr_snapshot.eye - target_shot.eye_mm).length() < 1.0);
    assert!((cam.curr_snapshot.target - target_shot.watch_mm).length() < 1.0);
    let expected_fov = fov_y_from_ps2_projection(600.0);
    assert!((cam.curr_snapshot.fov_y - expected_fov).abs() < 1e-4);

    // Standalone `return_pre_auto_cam_work(SmoothBlend)` also advances prev_snapshot & converges.
    sc.return_pre_auto_cam_work(ScriptTransitionKind::SmoothBlend, &mut cam, player_pos, 0.0);
    sc.apply_tick(&mut cam, 1.0 / 60.0);
    let b1 = cam.curr_snapshot;
    sc.apply_tick(&mut cam, 1.0 / 60.0);
    assert_eq!(cam.prev_snapshot, b1);
    for _ in 0..22 {
        sc.apply_tick(&mut cam, 1.0 / 60.0);
    }
    assert!(!sc.is_overriding());
    assert!((cam.curr_snapshot.eye - tick0_snap.eye).length() < 1.0);
}

#[test]
fn test_smooth_script_override_immune_to_underlying_cam_warp_in_cut() {
    let mk_road = |x_min: f32, x_max: f32, eye_x: f32, eye_z: f32, flags: u16| RoadRecord {
        lim_sw: RoadArea {
            x0: x_min,
            z0: -2000.0,
            x1: x_max,
            z1: 2000.0,
            x2: x_max,
            z2: 2000.0,
            min_hy: 500.0,
            max_hy: -2500.0,
        },
        lim_rd: RoadArea {
            x0: eye_x,
            z0: eye_z,
            x1: eye_x,
            z1: eye_z,
            x2: eye_x,
            z2: eye_z,
            min_hy: -1300.0,
            max_hy: -1300.0,
        },
        flags,
        area_size_type: AreaSizeType::Tiny,
        rd_type: RoadType::Road,
        projection: 448.0,
        cam_mv_type: CamMovementType::Settle,
        ..RoadRecord::default()
    };

    // Road 0 at x in [-1000..0] (eye = (-500, -1300, -1000), WARP_OUT)
    // Road 1 at x in [100..2000] (eye = (9000, -1300, 9000), WARP_IN — huge jump if leaked!)
    let mut cam = Ps2CameraController::new();
    cam.set_cam_file(Some(CamFile {
        records: vec![
            mk_road(-1000.0, 0.0, -500.0, -1000.0, VC_RD_WARP_OUT_F),
            mk_road(100.0, 2000.0, 9000.0, 9000.0, VC_RD_WARP_IN_F),
        ],
    }));

    let mut sys = EventCameraSystem::new();
    let start_pos = Vec3::new(-400.0, 0.0, 0.0);
    sys.enemy_framing.reset_camera_with_enemies(
        &mut cam,
        start_pos,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
    assert_eq!(cam.active_road_index, Some(0));

    let shot = ScriptedCameraShot {
        eye_mm: Vec3::new(-1800.0, -1600.0, -2200.0),
        watch_mm: Vec3::new(-200.0, -1000.0, 0.0),
        roll_radians: 0.0,
        projection_scr_z: Some(580.0),
    };
    sys.script_camera.set_camera_script(
        shot,
        ScriptTransitionKind::SmoothBlend,
        cam.curr_snapshot.fov_y,
    );

    // Step 5 ticks in Road 0, then cross into Road 1 (`VC_RD_WARP_IN_F`) on Tick 6!
    for step in 0..10 {
        let player_x = if step < 5 { -400.0 } else { 600.0 };
        let pre_eye = cam.curr_snapshot.eye;
        let _ = sys.tick(
            &mut cam,
            Vec3::new(player_x, 0.0, 0.0),
            0.0,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            None,
        );
        let post_eye = cam.curr_snapshot.eye;
        let step_dist = (post_eye - pre_eye).length();
        assert!(
            step_dist < 80.0,
            "SmoothBlend scripted camera must never teleport across underlying WARP_IN .cam cut (step {step}: {step_dist} mm)"
        );
        assert_eq!(
            cam.prev_snapshot.eye, pre_eye,
            "prev_snapshot must equal previous scripted tick eye, not underlying .cam rail"
        );
    }
    // Underlying active_road_index updated to Road 1 in the background while scripted shot stayed smooth.
    assert_eq!(cam.active_road_index, Some(1));
}
