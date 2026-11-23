//! Unit and retail `.cam` integration tests for `Ps2CameraController`.

use super::*;
use glam::Vec3;
use sh2_assets::formats::cam::CamFile;

#[test]
fn test_ps2_camera_reset_and_tick() {
    let mut cam = Ps2CameraController::new();
    let p_pos = Vec3::new(1000.0, 0.0, 2000.0);
    cam.reset_to_player(p_pos, 0.0);

    assert_eq!(cam.mv_type, CameraMovementType::Chase);
    assert!((cam.curr_snapshot.target.x - 1000.0).abs() < f32::EPSILON);

    cam.tick(p_pos + Vec3::new(10.0, 0.0, 10.0), 0.1, 0.5, 0.0);
    let view_mat = cam.interpolated_view_matrix(0.5);
    assert!(view_mat.is_finite());
}

#[test]
fn test_camera_er01_ps2_parity_framing() {
    let mut controller = Ps2CameraController::new();
    if !controller.load_room_cam("er01", false) {
        return;
    }
    assert_eq!(controller.active_room_id.as_deref(), Some("er01"));
    assert_eq!(
        controller.active_cam_source,
        Some(CameraSourceKind::VanillaRetail)
    );

    // Exact player coordinates from screenshot at opening restroom:
    // PLAYER POS: X: -19390.1, Y: 0.0, Z: 20317.8, YAW: -2.23 rad (-127.6 deg)
    let player_pos = Vec3::new(-19390.1, 0.0, 20317.8);
    controller.reset_to_player(player_pos, -2.23);

    assert_eq!(controller.active_road_index, Some(2));
    assert_eq!(controller.mv_type, CameraMovementType::Chase);

    // Ground-truth PS2 camera eye: (-18684.0, -1003.0, 20361.0)
    assert!((controller.curr_snapshot.eye.x - (-18684.0)).abs() < 1.0);
    assert!((controller.curr_snapshot.eye.y - (-1003.0)).abs() < 1.0);
    assert!((controller.curr_snapshot.eye.z - 20361.0).abs() < 1.0);

    // Ground truth look target: points towards sink/mirror and downward into chest/reflection
    // (-Y is up, so target.y must be below the eye: target.y > eye.y (-1003.0))
    assert!(controller.curr_snapshot.target.y > controller.curr_snapshot.eye.y);
    assert!(controller.curr_snapshot.target.x < controller.curr_snapshot.eye.x);
    assert!(controller.curr_snapshot.fov_y < 0.6);
}

#[test]
fn test_sh2_50_warp_in_out_flags_snap_prev_and_curr_snapshots() {
    use sh2_assets::formats::cam::{
        AreaSizeType, CamMovementParams, CamMovementType as AssetCamMovementType, ChaseCamParam,
        RoadArea, RoadRecord, RoadType, VC_RD_END_DATA_F, VC_RD_WARP_IN_F, VC_RD_WARP_OUT_F,
    };

    let make_area = |min_x: f32, max_x: f32| RoadArea {
        x0: min_x,
        z0: -1000.0,
        x1: min_x,
        z1: 1000.0,
        x2: max_x,
        z2: 1000.0,
        min_hy: -2000.0,
        max_hy: 2000.0,
    };

    let make_fixed_record = |min_x: f32, max_x: f32, eye: Vec3, flags: u16| RoadRecord {
        lim_sw: make_area(min_x, max_x),
        lim_rd: RoadArea {
            x0: eye.x,
            z0: eye.z,
            x1: eye.x,
            z1: eye.z,
            x2: eye.x,
            z2: eye.z,
            min_hy: eye.y,
            max_hy: eye.y,
        },
        kind_id: 0,
        flags,
        area_size_type: AreaSizeType::Wide,
        rd_type: RoadType::Event,
        mv_y_type: 3,
        ofs_watch_hy: -300.0,
        trace_btm_hy: 0.0,
        rd_dir_type: 0,
        projection: 448.0,
        proj_volume: 0.0,
        proj_sec: 1.0,
        cam_mv_type: AssetCamMovementType::Settle,
        params: CamMovementParams::Chase(ChaseCamParam::default()),
    };

    let cam_file = CamFile {
        records: vec![
            make_fixed_record(
                -1000.0,
                0.0,
                Vec3::new(-500.0, -1400.0, -800.0),
                VC_RD_WARP_OUT_F,
            ),
            make_fixed_record(
                0.0,
                1000.0,
                Vec3::new(500.0, -2200.0, 800.0),
                VC_RD_WARP_IN_F | VC_RD_END_DATA_F,
            ),
        ],
    };

    let mut controller = Ps2CameraController::new();
    controller.set_cam_file(Some(cam_file));

    // Spawn inside Road #0
    controller.reset_to_player(Vec3::new(-250.0, 0.0, 0.0), 0.0);
    assert_eq!(controller.active_road_index, Some(0));
    assert!((controller.curr_snapshot.eye.x - (-500.0)).abs() < 1e-3);

    // Cross boundary into Road #1 (`VC_RD_WARP_IN_F` / `VC_RD_WARP_OUT_F` active):
    // Both `curr_snapshot` and `prev_snapshot` must snap immediately with zero lerp smear.
    controller.tick(Vec3::new(250.0, 0.0, 0.0), 0.0, 0.0, 0.0);
    assert_eq!(controller.active_road_index, Some(1));
    assert!((controller.curr_snapshot.eye.x - 500.0).abs() < 1e-3);
    assert!((controller.prev_snapshot.eye.x - 500.0).abs() < 1e-3);
    assert!((controller.curr_snapshot.eye.y - (-2200.0)).abs() < 1e-3);
    assert!((controller.prev_snapshot.eye.y - (-2200.0)).abs() < 1e-3);
}

#[test]
fn test_vc_set_watch_tgt_xz_pos_aligns_with_camera_to_player_ray() {
    // Regression test for `SLUS_202.28` `vcSetWatchTgtXzPos` (`0x00199ca0`, `shAtan2(dz, dx)`
    // = `dx.atan2(dz)`): when James faces `+Z` (`yaw = 0`) and the chase camera sits directly
    // behind him in `-Z`, the watch target must project ahead along `+Z`, not 90° sideways
    // along `+X`.
    let mut controller = Ps2CameraController::new();
    if !controller.load_room_cam("ca04", true) {
        return; // Skip gracefully in CI without retail assets
    }

    let player_pos = Vec3::new(26_547.9, -13_319.5, -95_962.2);
    let player_yaw = -0.03f32;
    controller.reset_to_player(player_pos, player_yaw);

    let eye = controller.curr_snapshot.eye;
    let tgt = controller.curr_snapshot.target;
    let to_player = Vec3::new(player_pos.x - eye.x, 0.0, player_pos.z - eye.z).normalize();
    let to_tgt = Vec3::new(tgt.x - eye.x, 0.0, tgt.z - eye.z).normalize();
    let alignment = to_player.dot(to_tgt);
    assert!(
        alignment > 0.95,
        "camera target must align with camera->player ray (dot={alignment:.3}, eye={eye:?}, tgt={tgt:?})"
    );
}
