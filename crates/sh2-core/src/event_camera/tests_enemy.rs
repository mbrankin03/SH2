//! Unit and retail-asset integration tests for `SH2-52`: enemy encounter `.cam` framing
//! (`VC_RD_USE_NEAR_ENEMY_F` `0x0040`, `VC_RD_USE_NO_ENEMY_F` `0x0020`), `SelfViewGate`,
//! `StageEventRoadGate`, and `old_road_advantage_mm` boundary hysteresis.

use super::*;
use crate::camera::Ps2CameraController;
use glam::Vec3;
use sh2_assets::formats::cam::{
    parse_cam, AreaSizeType, CamFile, CamMovementType, RoadArea, RoadRecord, RoadType,
    VC_RD_END_DATA_F, VC_RD_NO_EXTRA_AREA_F, VC_RD_USE_NEAR_ENEMY_F, VC_RD_USE_NO_ENEMY_F,
    VC_RD_WARP_IN_F, VC_RD_WARP_OUT_F,
};

fn make_settle_road(
    x_min: f32,
    x_max: f32,
    eye_x: f32,
    rd_type: RoadType,
    area_size_type: AreaSizeType,
    flags: u16,
) -> RoadRecord {
    RoadRecord {
        lim_sw: RoadArea {
            x0: x_min,
            z0: -2000.0,
            x1: x_min,
            z1: 2000.0,
            x2: x_max,
            z2: 2000.0,
            min_hy: 500.0,
            max_hy: -2500.0,
        },
        lim_rd: RoadArea {
            x0: eye_x,
            z0: -1200.0,
            x1: eye_x,
            z1: -1200.0,
            x2: eye_x,
            z2: -1200.0,
            min_hy: -1300.0,
            max_hy: -1300.0,
        },
        flags,
        area_size_type,
        rd_type,
        projection: 448.0,
        cam_mv_type: CamMovementType::Settle,
        ..RoadRecord::default()
    }
}

fn alive_enemy(slot_id: u16, x: f32, z: f32, alerted: bool) -> EnemyCameraTarget {
    EnemyCameraTarget {
        slot_id,
        position: Vec3::new(x, 0.0, z),
        alert_state: if alerted {
            EnemyCameraAlertState::Alerted
        } else {
            EnemyCameraAlertState::Passive
        },
        life_state: EnemyLifeCameraState::Alive,
    }
}

fn tick_clear(framing: &mut EnemyEncounterFraming, cam: &mut Ps2CameraController, pos: Vec3) {
    framing.tick_camera_with_enemies(
        cam,
        pos,
        0.0,
        0.0,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
}

#[test]
fn test_ps2_nearest_enemy_radius_dead_grace_and_alert_priority() {
    let player_pos = Vec3::new(1000.0, 0.0, 2000.0);
    let outside = alive_enemy(0, 8500.0, 2000.0, true);
    assert_eq!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[outside]),
        EnemyProximityState::Clear
    );

    let inside = alive_enemy(1, 8499.0, 2000.0, false);
    assert_eq!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[inside]),
        EnemyProximityState::NearEnemy {
            slot_id: 1,
            xz_distance_mm: 7499.0,
            alert_state: EnemyCameraAlertState::Passive,
        }
    );

    let passive_close = alive_enemy(2, 2500.0, 2000.0, false);
    let alerted_far = alive_enemy(3, 6500.0, 2000.0, true);
    assert_eq!(
        evaluate_nearest_enemy(
            player_pos,
            PlayerCameraSuppression::Active,
            &[passive_close, alerted_far]
        ),
        EnemyProximityState::NearEnemy {
            slot_id: 3,
            xz_distance_mm: 5500.0,
            alert_state: EnemyCameraAlertState::Alerted,
        }
    );

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

    dying_state.step_dead_timer(0.2);
    dead_target.life_state = dying_state;
    assert_eq!(
        evaluate_nearest_enemy(player_pos, PlayerCameraSuppression::Active, &[dead_target]),
        EnemyProximityState::Clear
    );
    assert_eq!(
        evaluate_nearest_enemy(
            player_pos,
            PlayerCameraSuppression::SuppressedByDyingOrEvent,
            &[alerted_far]
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
    framing.reset_camera_with_enemies(
        &mut cam,
        Vec3::ZERO,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
    assert_eq!(cam.active_road_index, Some(0));
    assert!((cam.curr_snapshot.eye.x - (-800.0)).abs() < 1.0);

    let lying_figure = alive_enemy(0, 3200.0, 0.0, true);
    framing.tick_camera_with_enemies(
        &mut cam,
        Vec3::ZERO,
        0.0,
        0.0,
        0.0,
        PlayerCameraSuppression::Active,
        &[lying_figure],
    );
    assert_eq!(cam.active_road_index, Some(1));
    assert!((cam.curr_snapshot.eye.x - 1500.0).abs() < 1.0);
    assert_eq!(cam.prev_snapshot, cam.curr_snapshot);

    // Retail `cb54.cam` (First Lying Figure tunnel) and `ap42.cam` (`RoadType::Event`).
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

        let tunnel_enemy = alive_enemy(0, 47_000.0, 46_000.0, true);
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
        if let Some((ev_idx, pos)) = ap42_cam.cam_file.as_ref().and_then(|cf| {
            let (i, r) = cf
                .records
                .iter()
                .enumerate()
                .find(|(_, r)| r.rd_type == RoadType::Event)?;
            let c = r.lim_sw.center();
            Some((i, Vec3::new(c[0], c[1], c[2])))
        }) {
            framing.event_gate = StageEventRoadGate::Active;
            framing.reset_camera_with_enemies(
                &mut ap42_cam,
                pos,
                0.0,
                PlayerCameraSuppression::Active,
                &[],
            );
            assert_eq!(ap42_cam.active_road_index, Some(ev_idx));

            framing.event_gate = StageEventRoadGate::Completed;
            tick_clear(&mut framing, &mut ap42_cam, pos);
            assert_ne!(ap42_cam.active_road_index, Some(ev_idx));
        }
    }
}

#[test]
fn test_self_view_gate_and_old_road_hysteresis_in_tick_camera_with_enemies() {
    // 1. `SelfViewGate::Inactive` excludes `SelfViewOnly` even when it is the ONLY road in `.cam`.
    let mut cam = Ps2CameraController::new();
    cam.set_cam_file(Some(CamFile {
        records: vec![make_settle_road(
            -1000.0,
            1000.0,
            900.0,
            RoadType::SelfViewOnly,
            AreaSizeType::Small,
            VC_RD_WARP_IN_F,
        )],
    }));
    let mut framing = EnemyEncounterFraming::new();
    framing.self_view = SelfViewGate::Inactive;
    tick_clear(&mut framing, &mut cam, Vec3::ZERO);
    assert_eq!(cam.active_road_index, None);

    // 2. `SelfViewGate::Active` (`prio = 4`) beats `RoadPrioHigh` (`prio = 3`) & `Road` (`prio = 2`).
    cam.set_cam_file(Some(CamFile {
        records: vec![
            make_settle_road(
                -1000.0,
                1000.0,
                -700.0,
                RoadType::RoadPrioHigh,
                AreaSizeType::Small,
                0,
            ),
            make_settle_road(
                -1000.0,
                1000.0,
                1400.0,
                RoadType::SelfViewOnly,
                AreaSizeType::Small,
                VC_RD_WARP_IN_F,
            ),
        ],
    }));
    framing.reset_camera_with_enemies(
        &mut cam,
        Vec3::ZERO,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
    assert_eq!(cam.active_road_index, Some(0));

    framing.self_view = SelfViewGate::Active;
    tick_clear(&mut framing, &mut cam, Vec3::ZERO);
    assert_eq!(cam.active_road_index, Some(1));
    assert!((cam.curr_snapshot.eye.x - 1400.0).abs() < 1.0);

    // 3. `old_road_advantage_mm` (`250.0` mm for `Outdoor`) hysteresis in `tick_camera_with_enemies`.
    cam.set_cam_file(Some(CamFile {
        records: vec![
            make_settle_road(
                1050.0,
                3000.0,
                2200.0,
                RoadType::Road,
                AreaSizeType::Outdoor,
                0,
            ),
            make_settle_road(
                -1000.0,
                1000.0,
                -900.0,
                RoadType::Road,
                AreaSizeType::Outdoor,
                0,
            ),
        ],
    }));
    framing.self_view = SelfViewGate::Inactive;
    framing.reset_camera_with_enemies(
        &mut cam,
        Vec3::ZERO,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
    assert_eq!(cam.active_road_index, Some(1));

    // At x = 1300 mm: outside 1000 + 150 = 1150 mm, inside 1000 + 150 + 250 = 1400 mm!
    tick_clear(&mut framing, &mut cam, Vec3::new(1300.0, 0.0, 0.0));
    assert_eq!(cam.active_road_index, Some(1));

    // At x = 1450 mm (> 1400 mm): exits hysteresis band and switches to Record 0.
    tick_clear(&mut framing, &mut cam, Vec3::new(1450.0, 0.0, 0.0));
    assert_eq!(cam.active_road_index, Some(0));

    // With `VC_RD_NO_EXTRA_AREA_F` set on Record 1, x = 1300 mm switches immediately to Record 0.
    if let Some(ref mut cf) = cam.cam_file {
        cf.records[1].flags |= VC_RD_NO_EXTRA_AREA_F;
    }
    framing.reset_camera_with_enemies(
        &mut cam,
        Vec3::ZERO,
        0.0,
        PlayerCameraSuppression::Active,
        &[],
    );
    assert_eq!(cam.active_road_index, Some(1));
    tick_clear(&mut framing, &mut cam, Vec3::new(1300.0, 0.0, 0.0));
    assert_eq!(cam.active_road_index, Some(0));
}
