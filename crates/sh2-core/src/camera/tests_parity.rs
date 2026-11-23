//! Golden-vector unit tests and retail `.cam` asset integration tests for `SH2-133`.

use crate::camera::chase::{
    compute_chara_y_bounds, vc_adj_tgt_mv_vec_y_by_cur_near_road,
    vc_make_ideal_cam_pos_use_vc_road_data, vc_make_new_base_cam_ang, BASE_ANG_ORESEN_TABLE,
    MV_NML_CHR2CAM_R,
};
use crate::camera::exclusion::{vc_flip_from_cam_exclusion_area, EXCL_MAX_RATE};
use crate::camera::fixed_angle::{
    vc_make_ideal_cam_pos_for_fix_ang_cam, vc_warp_for_fix_ang_cam, FIX_ANG_WARP_RAIL_SHIFT_MM,
};
use crate::camera::kinematics::{
    compute_screen_half_angles, fov_y_from_ps2_projection_unclamped, sh_angle_regulate,
    vc_adj_cam_ofs_ang_by_chara_in_screen, vc_adj_cam_ofs_ang_by_ofs_ang_spd,
    vc_get_use_watch_and_cam_mv_param, vc_make_cam_mat_and_cam_ang_by_base_ang_and_ofs_ang,
    vc_make_ofs_cam_tgt_ang, vc_renewal_base_cam_ang_and_adjust_ofs_cam_ang, vc_renewal_cam_data,
    vc_ret_smooth_cam_mv_f, vw_matrix_to_angle_yxz, vw_oresen_hokan,
    vw_renewal_xz_velocity_to_target_pos, vw_ret_new_ang_spd_to_target_ang,
    vw_ret_new_velocity_to_target_val, vw_rot_matrix_yxz, CAM_MV_PRM_NRML, CAM_MV_PRM_OUTDOOR,
    WATCH_MV_PRM_NRML, WATCH_MV_PRM_USER,
};
use crate::types::camera::FixCamManager;
use crate::widescreen_rail::{switch_hysteresis_advantage_mm, AspectRailConfig};
use glam::Vec3;
use sh2_assets::formats::cam::{
    parse_cam, AreaSizeType, CamMovementParams, CamMovementType, ChaseCamParam, FixAngCamParam,
    RoadArea, RoadRecord, RoadType, SettleCamParam,
};
use sh2_assets::AssetPathResolver;
use std::f32::consts::{FRAC_PI_4, PI};

fn rect_area(x0: f32, x1: f32, z0: f32, z1: f32, min_hy: f32, max_hy: f32) -> RoadArea {
    RoadArea {
        x0,
        z0,
        x1: x0,
        z1,
        x2: x1,
        z2: z1,
        min_hy,
        max_hy,
    }
}

#[test]
fn test_vw_calc_velocity_and_angular_golden_vectors() {
    // 1. vwRetNewVelocityToTargetVal (0x0019ea30): ramp, max cap, and braking knee.
    let v1 = vw_ret_new_velocity_to_target_val(0.0, 0.0, 10.0, 1500.0, 1000.0, 1500.0, 1.0 / 60.0);
    assert!((v1 - 25.0).abs() < 1e-4);
    let v_cap =
        vw_ret_new_velocity_to_target_val(990.0, 0.0, 10.0, 1500.0, 1000.0, 1500.0, 1.0 / 60.0);
    assert!((v_cap - 1000.0).abs() < 1e-4);
    let v_brake =
        vw_ret_new_velocity_to_target_val(800.0, 0.0, 0.1, 1500.0, 1000.0, 1500.0, 1.0 / 60.0);
    assert!((v_brake - 150.0).abs() < 1e-4);

    // 2. vwRetNewAngSpdToTargetAng (0x0019eb40): shortest-arc wrap across ±PI.
    let ang_spd = vw_ret_new_ang_spd_to_target_ang(
        0.0,
        3.0,
        -3.0, // shortest delta is +(TAU - 6.0) ≈ +0.283 rad
        WATCH_MV_PRM_USER.ang_accel_y,
        WATCH_MV_PRM_USER.max_ang_spd_y,
        WATCH_MV_PRM_USER.ang_accel_y * 3.0,
        1.0 / 60.0,
    );
    assert!(ang_spd > 0.0 && ang_spd <= WATCH_MV_PRM_USER.max_ang_spd_y);

    // 3. vwRenewalXZVelocityToTargetPos (0x0019e640): lateral damping + speed cap.
    let mut vel = Vec3::new(3000.0, 0.0, 0.0); // pure lateral relative to +Z target
    vw_renewal_xz_velocity_to_target_pos(
        0.1,
        4000.0,
        2400.0,
        1600.0,
        6000.0,
        &mut vel,
        Vec3::ZERO,
        Vec3::new(0.0, 0.0, 5000.0),
        1.0 / 60.0,
    );
    assert!(vel.x < 2400.0, "lateral velocity must be capped and damped");
    assert!(vel.z > 0.0, "forward velocity must accelerate toward +Z");

    // 4. vcRenewalCamData (0x0019c840, XZ / 500.0 at 0x0019c8d4..0x0019c908 + sh2pc.exe 0x0051c100):
    //    Verify authentic braking envelope (~800 mm onset, 50 mm XZ deadband for CAM_MV_PRM_NRML).
    let mut brake_pos = Vec3::ZERO;
    let mut brake_vel = Vec3::new(0.0, 0.0, CAM_MV_PRM_NRML.max_spd_xz);
    let mut mv_ang = 0.0;
    // Above 800 mm (e.g. 900 mm): lim_len = 1600 * (900/500 - 0.1) = 2720 > 2400 -> holds max_spd_xz.
    vc_renewal_cam_data(
        &mut brake_pos,
        Vec3::new(0.0, 0.0, 900.0),
        &mut brake_vel,
        &mut mv_ang,
        &CAM_MV_PRM_NRML,
        false,
        1.0 / 60.0,
    );
    assert!(
        (brake_vel.z - CAM_MV_PRM_NRML.max_spd_xz).abs() < 1e-3,
        "above 800 mm braking onset, speed should hold at max_spd_xz (got {})",
        brake_vel.z
    );

    // Inside 800 mm braking envelope (450 mm): lim_len = 1600 * (450/500 - 0.1) = 1280 mm/s.
    brake_pos = Vec3::ZERO;
    brake_vel = Vec3::new(0.0, 0.0, CAM_MV_PRM_NRML.max_spd_xz);
    vc_renewal_cam_data(
        &mut brake_pos,
        Vec3::new(0.0, 0.0, 450.0),
        &mut brake_vel,
        &mut mv_ang,
        &CAM_MV_PRM_NRML,
        false,
        1.0 / 60.0,
    );
    assert!(
        (brake_vel.z - 1280.0).abs() < 1e-2,
        "at 450 mm, braking envelope must cap XZ speed to 1280 mm/s (got {})",
        brake_vel.z
    );

    // Inside 50 mm deadband (40 mm <= 0.1 * 500): lim_len = 0 -> XZ velocity clamps to 0.
    brake_pos = Vec3::ZERO;
    brake_vel = Vec3::new(0.0, 0.0, 500.0);
    vc_renewal_cam_data(
        &mut brake_pos,
        Vec3::new(0.0, 0.0, 40.0),
        &mut brake_vel,
        &mut mv_ang,
        &CAM_MV_PRM_NRML,
        false,
        1.0 / 60.0,
    );
    assert!(
        brake_vel.x.hypot(brake_vel.z) < 1e-4,
        "inside 50 mm deadband, XZ velocity must clamp to 0 (got {:?})",
        brake_vel
    );

    // Full convergence check: Y converges to < 1 mm; XZ settles at the authentic 50 mm deadband.
    let mut pos = Vec3::ZERO;
    let mut cvel = Vec3::ZERO;
    let tgt = Vec3::new(400.0, -300.0, 600.0);
    for _ in 0..180 {
        vc_renewal_cam_data(
            &mut pos,
            tgt,
            &mut cvel,
            &mut mv_ang,
            &CAM_MV_PRM_NRML,
            false,
            1.0 / 60.0,
        );
    }
    let xz_err = (tgt.x - pos.x).hypot(tgt.z - pos.z);
    assert!(
        (pos.y - tgt.y).abs() < 1.0,
        "Y should converge to < 1 mm (got {})",
        (pos.y - tgt.y).abs()
    );
    assert!(
        xz_err <= 51.0,
        "XZ should converge to the 50 mm deadband (got {} mm)",
        xz_err
    );

    let mut ofs_ang = Vec3::ZERO;
    let mut ofs_spd = Vec3::ZERO;
    let tgt_ang = Vec3::new(0.25, -0.75, 0.1);
    for _ in 0..120 {
        vc_adj_cam_ofs_ang_by_ofs_ang_spd(
            &mut ofs_ang,
            &mut ofs_spd,
            tgt_ang,
            &WATCH_MV_PRM_NRML,
            1.0 / 60.0,
        );
    }
    assert!((ofs_ang - tgt_ang).length() < 1e-3);
}

#[test]
fn test_vc_get_use_watch_and_cam_mv_param_and_smooth_flag() {
    let (w_in, c_in) = vc_get_use_watch_and_cam_mv_param(0.0, 1.0, false, false, 1.5, None, None);
    // Indoor far_watch_rate=1.0 scales cam_mv_prm_nrml by (1 + 1) = 2.0 and adds +50% to watch_prm + 1.5 to ang_accel_y.
    assert!((c_in.accel_xz - CAM_MV_PRM_NRML.accel_xz * 2.0).abs() < 1e-3);
    assert!((w_in.ang_accel_y - (WATCH_MV_PRM_NRML.ang_accel_y * 1.5 + 1.5)).abs() < 1e-3);

    let (_w_out, c_out) = vc_get_use_watch_and_cam_mv_param(0.0, 1.0, true, false, 0.0, None, None);
    assert_eq!(c_out, CAM_MV_PRM_OUTDOOR);

    // vcRetSmoothCamMvF (0x00196360) exact cut thresholds.
    assert!(vc_ret_smooth_cam_mv_f(
        Vec3::ZERO,
        Vec3::new(99.0, 0.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        1.0
    ));
    assert!(!vc_ret_smooth_cam_mv_f(
        Vec3::ZERO,
        Vec3::new(101.0, 0.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        1.0
    ));
    assert!(!vc_ret_smooth_cam_mv_f(
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::new(0.36, 0.0, 0.0),
        1.0
    ));
    assert!(!vc_ret_smooth_cam_mv_f(
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::new(0.0, 0.53, 0.0),
        1.0
    ));
    assert!(vc_ret_smooth_cam_mv_f(
        Vec3::ZERO,
        Vec3::new(350.0, 0.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        4.0
    ));
}

#[test]
fn test_chase_ideal_pos_deadband_compression_and_mv_y_types() {
    let rec = RoadRecord {
        lim_sw: rect_area(-5000.0, 5000.0, -5000.0, 5000.0, 500.0, -3000.0),
        lim_rd: rect_area(-5000.0, 5000.0, -5000.0, 5000.0, -800.0, -2500.0),
        area_size_type: AreaSizeType::Wide,
        cam_mv_type: CamMovementType::Chase,
        params: CamMovementParams::Chase(ChaseCamParam {
            ofs_hy: -500.0,
            ..ChaseCamParam::default()
        }),
        ..RoadRecord::default()
    };

    // Uncompressed Wide radius = 1000 mm (`cur_cam_pos.y = -500.0` -> `y_excess = 0.0`),
    // ideal_cam_pos_h = -1125, + ofs_hy (-500) = -1625.
    let mut c2i = PI;
    let pos = vc_make_ideal_cam_pos_use_vc_road_data(
        &rec,
        Vec3::ZERO,
        0.0,
        0.0,
        Vec3::new(0.0, -500.0, -1000.0),
        -1125.0,
        &mut c2i,
        false,
        &AspectRailConfig::authentic_ps2_4x3(),
    );
    assert!((pos.y - (-1625.0)).abs() < 1e-3);
    assert!((pos.x.hypot(pos.z) - MV_NML_CHR2CAM_R[AreaSizeType::Wide as usize]).abs() < 1e-2);
    // Deadband holds ±12° (0.20943952 rad) when angular speed <= 20°/s.
    assert!((sh_angle_regulate(c2i - PI).abs() - 0.209_439_52).abs() < 1e-4);

    // Height-compressed Wide radius: `cur_cam_pos.y = -1625.0` -> `y_excess = 875.0` mm > `800.0`,
    // linearly compressing `r` from `1000.0` toward `200.0` over `[800, 2400]` -> `962.5` mm.
    let pos_comp = vc_make_ideal_cam_pos_use_vc_road_data(
        &rec,
        Vec3::ZERO,
        0.0,
        0.0,
        Vec3::new(0.0, -1625.0, -1000.0),
        -1125.0,
        &mut c2i,
        false,
        &AspectRailConfig::authentic_ps2_4x3(),
    );
    assert!((pos_comp.x.hypot(pos_comp.z) - 962.5).abs() < 1e-2);

    // mv_y_type 0..=5 golden checks (`vcAdjTgtMvVecYByCurNearRoad` 0x0019b990).
    for mv_y in 0..=5 {
        let mut r = rec.clone();
        r.mv_y_type = mv_y;
        let mut mv = Vec3::new(0.0, 500.0, 0.0);
        vc_adj_tgt_mv_vec_y_by_cur_near_road(
            &mut mv,
            &r,
            Vec3::new(0.0, -1500.0, -1000.0),
            Vec3::ZERO,
            -925.0,
            -1125.0,
            false,
        );
        assert!(mv.y.is_finite());
    }
}

#[test]
fn test_exclusion_area_curve_and_shrink_rate_limit() {
    let mut old_r = -1.0;
    // Directly in front of character (`ang_diff = 0`): curve = 16384/4096 * 500 = 2000.0 * 1.0 (Outdoor).
    let (push_front, _) = vc_flip_from_cam_exclusion_area(
        0.0,
        &mut old_r,
        Vec3::new(0.0, 0.0, 500.0),
        Vec3::ZERO,
        AreaSizeType::Outdoor,
        1.0 / 60.0,
    );
    assert!((old_r - 2000.0 * EXCL_MAX_RATE[3]).abs() < 1e-2);
    assert!((push_front - 1500.0).abs() < 1e-2);

    // Next tick behind character: radius shrinks at most 250.0 * dt (~4.167 mm).
    let prev_r = old_r;
    let _ = vc_flip_from_cam_exclusion_area(
        0.0,
        &mut old_r,
        Vec3::new(0.0, 0.0, -500.0),
        Vec3::ZERO,
        AreaSizeType::Outdoor,
        1.0 / 60.0,
    );
    assert!((old_r - (prev_r - 250.0 / 60.0)).abs() < 1e-2);
}

#[test]
fn test_fixed_angle_ideal_pos_and_180_warp() {
    let rec = RoadRecord {
        lim_sw: rect_area(-4000.0, 4000.0, -4000.0, 4000.0, 500.0, -3000.0),
        lim_rd: rect_area(-2000.0, 2000.0, -2000.0, 2000.0, -1000.0, -1800.0),
        kind_id: 1,
        rd_dir_type: 0,
        cam_mv_type: CamMovementType::FixedAngle,
        params: CamMovementParams::FixedAngle(FixAngCamParam {
            ang_x: -0.3,
            ang_y: 0.0,
            ofs_hy: -250.0,
            cam2wth_dist: 2000.0,
        }),
        ..RoadRecord::default()
    };
    let mut mng = FixCamManager::default();
    let mut chara_top_y = -925.0;
    let eye = vc_make_ideal_cam_pos_for_fix_ang_cam(
        &rec,
        Vec3::new(0.0, 50.0, 0.0),
        0.0,
        Vec3::new(0.0, -1400.0, -1000.0),
        &mut chara_top_y,
        &mut mng,
        true,
        false,
        &AspectRailConfig::authentic_ps2_4x3(),
    );
    assert!(
        eye.z < 0.0,
        "FixedAngle eye backs away opposite ang_y=0 (+Z)"
    );
    // `vc_main.c:2580-2582`: `ideal_pos.y == chara_pos.y` (unclamped) & `chara_top_y += fix.ofs_hy`.
    assert!((eye.y - 50.0).abs() < 1e-4);
    assert!((chara_top_y - (-1175.0)).abs() < 1e-4);

    // Facing 180° opposite (`PI` vs `0.0` >= 135°) triggers `vcWarpForFixAngCam` (`0x00252980`).
    assert!(vc_warp_for_fix_ang_cam(&rec, PI, &mut mng));
    assert_eq!(mng.exception_f, 1);
    assert!((mng.add_ang_y - PI).abs() < 1e-5);
    assert!((mng.add_rd_z - FIX_ANG_WARP_RAIL_SHIFT_MM).abs() < 1e-5);
}

#[test]
fn test_settle_base_angle_bounds_framing_fov_and_self_view_hysteresis() {
    // Character bounds (`0x0019df50`): standing bottom = pos.y, top = pos.y - 925.0; crouch = -250.0.
    assert_eq!(compute_chara_y_bounds(100.0, 0.0), (100.0, -825.0, -362.5));
    assert_eq!(compute_chara_y_bounds(100.0, 1.0), (-150.0, -825.0, -487.5));

    // Unclamped FOV + horizontal `scr_half_ang_wx` framing.
    let wide_fov = fov_y_from_ps2_projection_unclamped(100.0);
    assert!(
        wide_fov > 1.5,
        "FOV must not be artificially clamped at 1.15/1.45"
    );
    let (half_wx, half_wy) = compute_screen_half_angles(448.0);
    assert!(half_wx > 0.0 && half_wy > 0.0);
    let mut ofs = Vec3::ZERO;
    vc_adj_cam_ofs_ang_by_chara_in_screen(
        &mut ofs,
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        half_wx,
        half_wy,
    );
    assert!((ofs.y - (1.0 - half_wx)).abs() < 1e-4);

    // Settle base angle (`0x0019cbe0`): `BASE_ANG_ORESEN_TABLE` and yaw sector clamp.
    assert_eq!(vw_oresen_hokan(&BASE_ANG_ORESEN_TABLE, 0, 0, 0x1921), 0);
    let settle_rec = RoadRecord {
        cam_mv_type: CamMovementType::Settle,
        params: CamMovementParams::Settle(SettleCamParam {
            sta_base_ang_y: -0.5,
            end_base_ang_y: 0.5,
            lr_lim_ang_y: -17.451_548,
            rr_lim_ang_y: 17.451_548,
        }),
        ..RoadRecord::default()
    };
    let base = vc_make_new_base_cam_ang(
        &settle_rec,
        Vec3::ZERO,
        Vec3::new(1000.0, 0.0, 0.0), // yaw = +PI/2 > 0.5 -> clamps to 0.5
        Vec3::ZERO,
        500.0,
        false,
        true,
        1.0 / 60.0,
    );
    assert!((base.y - 0.5).abs() < 1e-4);

    // SelfViewOnly hysteresis (`0x00198ae0` + `0x00390600`): [75, 125, 175, 250] mm, NOT 0.0!
    for (sz, expected) in [
        (AreaSizeType::Tiny, 75.0),
        (AreaSizeType::Small, 125.0),
        (AreaSizeType::Wide, 175.0),
        (AreaSizeType::Outdoor, 250.0),
    ] {
        let r = RoadRecord {
            rd_type: RoadType::SelfViewOnly,
            area_size_type: sz,
            ..RoadRecord::default()
        };
        assert_eq!(switch_hysteresis_advantage_mm(&r), expected);
    }
}

#[test]
fn test_oriented_retail_cam_volumes_ap30_and_ca03() -> Result<(), Box<dyn std::error::Error>> {
    let resolver = AssetPathResolver::resolve();
    let ap30_path = resolver.data_dir.join("bg/ap/ap30.cam");
    if !ap30_path.exists() {
        return Ok(());
    }
    // `bg/ap/ap30.cam` record #5 is a 45° rotated square (`[-20000, 20800] -> [-20565.7, 20234.3] -> [-20000, 19668.6]`).
    let ap30 = parse_cam(&std::fs::read(&ap30_path)?)?;
    let sw = ap30.records[5].lim_sw;
    assert!((sw.orientation_angle_y() - FRAC_PI_4).abs() < 0.02);

    // Center of the diamond is inside; world AABB corner `(-20565.7, 20800.0)` is OUTSIDE the rotated diamond!
    let center = sw.center();
    assert!(sw.contains_point(center, 0.0));
    assert!(
        !sw.contains_point([-20_565.7, center[1], 20_800.0], 0.0),
        "AABB corner outside 45° rotated volume must NOT be contained"
    );

    // Round-trip `world_to_local_xz` -> `local_to_world_xz` on `bg/ca/ca03.cam` (18° diagonal trail).
    let ca03 = parse_cam(&std::fs::read(resolver.data_dir.join("bg/ca/ca03.cam"))?)?;
    for rec in &ca03.records {
        for area in [rec.lim_sw, rec.lim_rd] {
            let (lx, lz) = area.world_to_local_xz(area.x0, area.z0);
            let (wx, wz) = area.local_to_world_xz(lx, lz);
            assert!((wx - area.x0).abs() < 1e-2 && (wz - area.z0).abs() < 1e-2);
        }
    }
    Ok(())
}

#[test]
fn test_yxz_matrix_renewal_base_cam_ang_world_orientation_continuity() {
    // 1. Round-trip `vwRotMatrixYXZ` (0x0019edf0) <-> `vwMatrixToAngleYXZ` (0x0019ee60).
    let ang = Vec3::new(0.35, -1.1, 0.22);
    let mat = vw_rot_matrix_yxz(ang);
    let extracted = vw_matrix_to_angle_yxz(mat);
    assert!((extracted - ang).length() < 1e-5);

    // Gimbal lock ±PI/2 extraction matches `vw_calc.c:305-314`.
    let gimbal_pos = vw_matrix_to_angle_yxz(vw_rot_matrix_yxz(Vec3::new(
        std::f32::consts::FRAC_PI_2,
        0.7,
        0.0,
    )));
    assert!((gimbal_pos.x - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert!((gimbal_pos.y - 0.7).abs() < 1e-5);
    assert!(gimbal_pos.z.abs() < 1e-5);

    // 2. `vcRenewalBaseCamAngAndAdjustOfsCamAng` (0x0019cfa0): changing `base_cam_ang` from
    // non-zero pitch/yaw to a new base must keep world camera orientation `R(base) * R(ofs)` exact!
    let mut base_ang = Vec3::new(0.4, 0.8, 0.0);
    let mut ofs_ang = Vec3::new(-0.25, 0.35, 0.12);
    let (before_mat, _before_ang) =
        vc_make_cam_mat_and_cam_ang_by_base_ang_and_ofs_ang(base_ang, ofs_ang);

    let new_base_ang = Vec3::new(0.55, -0.45, 0.0);
    vc_renewal_base_cam_ang_and_adjust_ofs_cam_ang(&mut base_ang, &mut ofs_ang, new_base_ang);
    assert_eq!(base_ang, new_base_ang);

    let (after_mat, _after_ang) =
        vc_make_cam_mat_and_cam_ang_by_base_ang_and_ofs_ang(base_ang, ofs_ang);
    assert!((after_mat.x_axis - before_mat.x_axis).length() < 1e-5);
    assert!((after_mat.y_axis - before_mat.y_axis).length() < 1e-5);
    assert!((after_mat.z_axis - before_mat.z_axis).length() < 1e-5);

    // 3. `vcMakeOfsCamTgtAng` (0x0019d0f0) + `vcMakeCamMatAndCamAngByBaseAngAndOfsAng` (0x0019d560):
    // Transforming a world watch vector into `base_mat^T` and composing back recovers exact world forward.
    let cam_pos = Vec3::new(100.0, -1500.0, -500.0);
    let watch_tgt = Vec3::new(900.0, -900.0, 1200.0);
    let base_mat_t = vw_rot_matrix_yxz(base_ang).transpose();
    let tgt_ofs = vc_make_ofs_cam_tgt_ang(base_mat_t, watch_tgt, cam_pos, 0.0);
    let (composed_mat, _) = vc_make_cam_mat_and_cam_ang_by_base_ang_and_ofs_ang(base_ang, tgt_ofs);
    let expected_fwd = (watch_tgt - cam_pos).normalize();
    assert!((composed_mat.z_axis - expected_fwd).length() < 1e-5);
}

#[test]
fn test_fixed_angle_warp_clamps_y_via_adj_tgt_mv_vec_y() {
    use crate::camera::Ps2CameraController;
    use sh2_assets::formats::cam::{CamFile, VC_RD_WARP_IN_F};

    // FixedAngle road with `mv_y_type = 3` (`lim_rd.max_hy = -1800.0`, `lim_rd.min_hy = -1200.0`).
    // `vcMakeIdealCamPosForFixAngCam` sets `ideal_pos.y = chara_pos.y` (floor = 50.0), so on warp
    // `vcAutoRenewalCamTgtPos` (0x00199f80) MUST still run `vcAdjTgtMvVecYByCurNearRoad` and clamp
    // `cam_tgt_pos.y` (and `cam_pos.y`) into `[-1800.0, -1200.0]` instead of snapping to `50.0`.
    let fixed_rec = RoadRecord {
        lim_sw: rect_area(-4000.0, 4000.0, -4000.0, 4000.0, 500.0, -3000.0),
        lim_rd: rect_area(-3000.0, 3000.0, -3000.0, 3000.0, -1200.0, -1800.0),
        flags: VC_RD_WARP_IN_F,
        cam_mv_type: CamMovementType::FixedAngle,
        mv_y_type: 3,
        projection: 448.0,
        params: CamMovementParams::FixedAngle(FixAngCamParam {
            ang_x: -0.2,
            ang_y: 0.0,
            ofs_hy: -200.0,
            cam2wth_dist: 2000.0,
        }),
        ..RoadRecord::default()
    };

    let mut ctrl = Ps2CameraController::new();
    ctrl.set_cam_file(Some(CamFile {
        records: vec![fixed_rec],
    }));

    // Warp via `reset_to_player` (`warp = true`, `dt = 0.0`).
    ctrl.reset_to_player(Vec3::new(0.0, 50.0, 0.0), 0.0);
    assert!(
        (ctrl.curr_snapshot.eye.y - (-1200.0)).abs() < 1e-3,
        "FixedAngle warp must clamp Y via vcAdjTgtMvVecYByCurNearRoad to -1200.0, not floor 50.0 (got {})",
        ctrl.curr_snapshot.eye.y
    );
    assert!(
        (ctrl.raw_state.cam_tgt_pos[1] - (-1200.0)).abs() < 1e-3,
        "cam_tgt_pos.y must also be clamped to -1200.0 (got {})",
        ctrl.raw_state.cam_tgt_pos[1]
    );
    assert!(ctrl.raw_state.cam_tgt_velo.iter().all(|v| v.is_finite()));
    assert!(ctrl.raw_state.cam_tgt_spd.is_finite());
}
