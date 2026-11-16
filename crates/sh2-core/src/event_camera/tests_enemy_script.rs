//! Unit and retail-asset integration tests for `SH2-52`: enemy encounter `.cam` framing
//! (`VC_RD_USE_NEAR_ENEMY_F` `0x0040` / `VC_RD_USE_NO_ENEMY_F` `0x0020`) and scripted event /
//! `.mes` dialogue camera overrides.

use super::*;
use crate::camera::Ps2CameraController;
use crate::events::{CameraOverrideCommand, APART_E3FE_ROOM13_CAMERA};
use glam::Vec3;
use sh2_assets::formats::cam::{
    parse_cam, RoadType, VC_RD_END_DATA_F, VC_RD_USE_NEAR_ENEMY_F, VC_RD_USE_NO_ENEMY_F,
    VC_RD_WARP_IN_F, VC_RD_WARP_OUT_F,
};

#[test]
fn test_ps2_nearest_enemy_radius_dead_grace_and_alert_priority() {
    let player_pos = Vec3::new(1000.0, 0.0, 2000.0);

    // 1. Exact 7500.0 mm XZ boundary (`0x45ea6000` in `vcSetNearestEnemyDataInVC_WORK` `0x00196f20`).
    let outside = EnemyCameraTarget {
        slot_id: 0,
        position: Vec3::new(8500.0, 0.0, 2000.0), // dx == 7500.0 -> rejected by c.olt.s
        alert_state: EnemyCameraAlertState::Alerted,
        life_state: EnemyLifeCameraState::Alive,
    };
    assert_eq!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[outside]),
        EnemyProximityState::Clear
    );

    let inside = EnemyCameraTarget {
        slot_id: 1,
        position: Vec3::new(8499.0, 0.0, 2000.0), // dx == 7499.0 < 7500.0
        alert_state: EnemyCameraAlertState::Passive,
        life_state: EnemyLifeCameraState::Alive,
    };
    assert_eq!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[inside]),
        EnemyProximityState::NearEnemy {
            slot_id: 1,
            xz_distance_mm: 7499.0,
            alert_state: EnemyCameraAlertState::Passive,
        }
    );

    // 2. Alerted enemy within 7500 mm beats closer Passive enemy (`0x00197130`).
    let passive_close = EnemyCameraTarget {
        slot_id: 2,
        position: Vec3::new(2500.0, 0.0, 2000.0),
        alert_state: EnemyCameraAlertState::Passive,
        life_state: EnemyLifeCameraState::Alive,
    };
    let alerted_farther = EnemyCameraTarget {
        slot_id: 3,
        position: Vec3::new(6500.0, 0.0, 2000.0),
        alert_state: EnemyCameraAlertState::Alerted,
        life_state: EnemyLifeCameraState::Alive,
    };
    assert_eq!(
        evaluate_nearest_enemy(
            player_pos,
            PlayerCameraSuppression::Active,
            &[passive_close, alerted_farther]
        ),
        EnemyProximityState::NearEnemy {
            slot_id: 3,
            xz_distance_mm: 5500.0,
            alert_state: EnemyCameraAlertState::Alerted,
        }
    );

    // 3. Post-death 1.5s grace window (`vcSetAllNpcDeadTimer` `0x001962a0` + `0x00196fd4`).
    let mut dying_state = EnemyLifeCameraState::DyingOrDead {
        dead_timer_secs: 1.4,
    };
    let mut dead_target = EnemyCameraTarget {
        slot_id: 4,
        position: Vec3::new(3000.0, 0.0, 2000.0),
        alert_state: EnemyCameraAlertState::Alerted,
        life_state: dying_state,
    };
    assert!(matches!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[dead_target]),
        EnemyProximityState::NearEnemy { slot_id: 4, .. }
    ));

    dying_state.step_dead_timer(0.2); // 1.6s > 1.5s grace
    dead_target.life_state = dying_state;
    assert_eq!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[dead_target]),
        EnemyProximityState::Clear
    );

    // 4. Player dying/event suppression clears nearest enemy (`0x00196f6c`).
    assert_eq!(
        evaluate_nearest_enemy(
            player_pos,
            PlayerCameraSuppression::SuppressedByDyingOrEvent,
            &[alerted_farther]
        ),
        EnemyProximityState::Clear
    );
}

#[test]
fn test_binary_cam_flags_0x0020_0x0040_and_retail_room_enemy_switching() {
    let mut raw = [0u8; 128 * 3];
    let write_box = |buf: &mut [u8], coords: [f32; 8]| {
        for (idx, val) in coords.into_iter().enumerate() {
            buf[idx * 4..(idx + 1) * 4].copy_from_slice(&val.to_le_bytes());
        }
    };
    // Record 0: Exploration camera (`VC_RD_USE_NO_ENEMY_F | VC_RD_WARP_OUT_F`)
    write_box(
        &mut raw[0..32],
        [
            -2000.0, -2000.0, 2000.0, 2000.0, 2000.0, 2000.0, 500.0, -2500.0,
        ],
    );
    write_box(
        &mut raw[32..64],
        [
            -800.0, -800.0, -800.0, -800.0, -800.0, -800.0, -1100.0, -1100.0,
        ],
    );
    raw[0x44..0x46].copy_from_slice(&(VC_RD_USE_NO_ENEMY_F | VC_RD_WARP_OUT_F).to_le_bytes());
    raw[0x6C..0x70].copy_from_slice(&1i32.to_le_bytes());

    // Record 1: Combat encounter camera (`VC_RD_USE_NEAR_ENEMY_F | VC_RD_WARP_IN_F`)
    let r1 = &mut raw[128..256];
    write_box(
        &mut r1[0..32],
        [
            -2000.0, -2000.0, 2000.0, 2000.0, 2000.0, 2000.0, 500.0, -2500.0,
        ],
    );
    write_box(
        &mut r1[32..64],
        [
            1500.0, -2200.0, 1500.0, -2200.0, 1500.0, -2200.0, -2100.0, -2100.0,
        ],
    );
    r1[0x44..0x46].copy_from_slice(&(VC_RD_USE_NEAR_ENEMY_F | VC_RD_WARP_IN_F).to_le_bytes());
    r1[0x6C..0x70].copy_from_slice(&1i32.to_le_bytes());
    raw[256 + 0x44..256 + 0x46].copy_from_slice(&VC_RD_END_DATA_F.to_le_bytes());

    let parsed = parse_cam(&raw).expect("binary .cam with 0x0020/0x0040 must parse");
    let mut cam = Ps2CameraController::new();
    cam.set_cam_file(Some(parsed));
    let mut framing = EnemyEncounterFraming::new();
    let player_pos = Vec3::ZERO;

    framing.reset_camera_with_enemies(
        &mut cam,
        player_pos,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
    assert_eq!(cam.active_road_index, Some(0));
    assert!((cam.curr_snapshot.eye.x - (-800.0)).abs() < 1.0);

    // Spawn Lying Figure at 3,200 mm (< 7,500 mm): warp-cut cleanly to Record 1!
    let lying_figure = EnemyCameraTarget {
        slot_id: 0,
        position: Vec3::new(3200.0, 0.0, 0.0),
        alert_state: EnemyCameraAlertState::Alerted,
        life_state: EnemyLifeCameraState::Alive,
    };
    framing.tick_camera_with_enemies(
        &mut cam,
        player_pos,
        0.0,
        0.0,
        0.0,
        PlayerCameraSuppression::Active,
        &[lying_figure],
    );
    assert_eq!(cam.active_road_index, Some(1));
    assert!((cam.curr_snapshot.eye.x - 1500.0).abs() < 1.0);
    assert_eq!(cam.prev_snapshot, cam.curr_snapshot);

    // Retail `.cam` verification on `cb54.cam` (First Lying Figure tunnel) and `ap42.cam` (`RoadType::Event`).
    let mut tunnel_cam = Ps2CameraController::new();
    if tunnel_cam.load_room_cam("cb54", false) {
        if let Some(ref mut cf) = tunnel_cam.cam_file {
            let mut combat_rec = cf.records[0].clone();
            combat_rec.flags |= VC_RD_USE_NEAR_ENEMY_F | VC_RD_WARP_IN_F;
            combat_rec.projection = 384.0;
            cf.records[0].flags |= VC_RD_USE_NO_ENEMY_F;
            cf.records.push(combat_rec);
        }
        let tunnel_pos = Vec3::new(47_000.0, 0.0, 50_500.0);
        framing.reset_camera_with_enemies(
            &mut tunnel_cam,
            tunnel_pos,
            0.0,
            PlayerCameraSuppression::Active,
            &[],
        );
        assert_eq!(tunnel_cam.active_road_index, Some(0));
        let calm_fov = tunnel_cam.curr_snapshot.fov_y;

        let tunnel_enemy = EnemyCameraTarget {
            slot_id: 0,
            position: Vec3::new(47_000.0, 0.0, 46_000.0),
            alert_state: EnemyCameraAlertState::Alerted,
            life_state: EnemyLifeCameraState::Alive,
        };
        framing.tick_camera_with_enemies(
            &mut tunnel_cam,
            tunnel_pos,
            0.0,
            0.0,
            0.0,
            PlayerCameraSuppression::Active,
            &[tunnel_enemy],
        );
        let last_idx = tunnel_cam.cam_file.as_ref().map(|c| c.records.len() - 1);
        assert_eq!(tunnel_cam.active_road_index, last_idx);
        assert!(tunnel_cam.curr_snapshot.fov_y > calm_fov);
    }

    let mut ap42_cam = Ps2CameraController::new();
    if ap42_cam.load_room_cam("ap42", false) {
        if let Some(ref cf) = ap42_cam.cam_file {
            if let Some((ev_idx, ev_rec)) = cf
                .records
                .iter()
                .enumerate()
                .find(|(_, r)| r.rd_type == RoadType::Event)
            {
                let c = ev_rec.lim_sw.center();
                let pos = Vec3::new(c[0], c[1], c[2]);
                assert_eq!(
                    select_best_road_index(
                        cf,
                        pos,
                        50.0,
                        None,
                        EnemyProximityState::Clear,
                        StageEventRoadGate::Active,
                        SelfViewGate::Inactive,
                    ),
                    Some(ev_idx)
                );
                assert_ne!(
                    select_best_road_index(
                        cf,
                        pos,
                        50.0,
                        None,
                        EnemyProximityState::Clear,
                        StageEventRoadGate::Completed,
                        SelfViewGate::Inactive,
                    ),
                    Some(ev_idx)
                );
            }
        }
    }
}

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
