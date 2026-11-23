//! Chase, Settle, vertical Y-tracking, and base-angle camera algorithms (`SLUS_202.28` `view/vc_main.c` & `view/vc_calc.c`).
//!
//! Ground truth:
//! - `vcMakeIdealCamPosUseVC_ROAD_DATA` (`0x0019b160`)
//! - `vcRetNearRatioSwitchAreaInXZPos` (`0x001a67f0`)
//! - `vcAdjTgtMvVecYByCurNearRoad` (`0x0019b990`)
//! - `vcMakeNewBaseCamAng` (`0x0019cbe0`)
//! - `vcRetMaxTgtMvXzLen` (`0x0019a2d0`)
//! - `vcMakeBasicCamTgtMvVec` (`0x0019b850`)
//! - `vcMakeNormalWatchTgtPos` (`0x00198d80`)

use crate::camera::kinematics::{sh_angle_regulate, vw_oresen_hokan};
use crate::widescreen_rail::{
    clamp_axis_with_margin, compute_aspect_rail_margin_mm, AspectRailConfig,
};
use glam::Vec3;
use sh2_assets::formats::cam::{CamMovementParams, CamMovementType, RoadRecord};
use std::f32::consts::{FRAC_PI_2, PI};

/// `mv_nml_chr2cam_r` (`SLUS_202.28` `0x003905d0`, identical in `sh2pc.exe` `file 0x2c6ac0`)
/// in PS2 world millimeters, indexed by `AreaSizeType`.
pub const MV_NML_CHR2CAM_R: [f32; 4] = [600.0, 800.0, 1000.0, 1250.0];

/// `mv_nml_no_adj_max_dist` (`SLUS_202.28` `0x003905e0`, identical in `sh2pc.exe` `file 0x2c6ad0`)
/// in PS2 world millimeters, indexed by `AreaSizeType`.
pub const MV_NML_NO_ADJ_MAX_DIST: [f32; 4] = [400.0, 600.0, 800.0, 1000.0];

/// `mv_nml_full_adj_min_dist` (`SLUS_202.28` `0x003905f0`, identical in `sh2pc.exe` `file 0x2c6ae0`)
/// in PS2 world millimeters, indexed by `AreaSizeType`.
pub const MV_NML_FULL_ADJ_MIN_DIST: [f32; 4] = [1200.0, 1600.0, 2400.0, 2800.0];

/// `nml_tgt_watch_cir_r` (retail `sh2pc.exe` `file 0x2c6a90`: `[400, 600, 1000, 1250]`;
/// `SLUS_202.28` `0x003905a0` had `1200.0` for `Outdoor`) in PS2 world millimeters.
pub const NML_TGT_WATCH_CIR_R: [f32; 4] = [400.0, 600.0, 1000.0, 1250.0];

/// `nml_cam2wth_min_dist` (`SLUS_202.28` `0x003905b0`, identical in `sh2pc.exe` `file 0x2c6aa0`)
/// in PS2 world millimeters, indexed by `AreaSizeType`.
pub const NML_CAM2WTH_MIN_DIST: [f32; 4] = [1400.0, 1800.0, 2800.0, 3200.0];

/// Chase camera ±12° deadband angle in radians (`0.20943952` rad, `SLUS_202.28` `0x0019b160`).
pub const CHASE_DEADBAND_RAD: f32 = 0.209_439_52;

/// Character angular speed threshold (`20°/s` = `0.34906584` rad/s) gating chase deadband
/// (`SLUS_202.28` `vcMakeIdealCamPosUseVC_ROAD_DATA` `0x0019b160`).
pub const CHASE_ANG_SPD_THRESHOLD: f32 = 0.349_065_84;

/// Minimum compressed character-to-camera chase radius in mm (`200.0`, `SLUS_202.28` `0x0019b160`).
pub const CHASE_MIN_COMPRESSED_R_MM: f32 = 200.0;

/// Indoor `ideal_cam_pos_h` offset from `chara_top_y` in mm (`-200.0`, `SLUS_202.28` `0x00196be0`).
pub const IDEAL_CAM_POS_H_INDOOR_OFS_MM: f32 = -200.0;

/// Outdoor `ideal_cam_pos_h` offset from `chara_top_y` in mm (`-450.0`, `SLUS_202.28` `0x00196be0`).
pub const IDEAL_CAM_POS_H_OUTDOOR_OFS_MM: f32 = -450.0;

/// Standing character top offset from `chara_pos.y` in mm (`-925.0`, `SLUS_202.28` `vcMoveAndSetCamera` `0x0019df50`).
pub const CHARA_TOP_Y_OFFSET_MM: f32 = -925.0;

/// Crouching character bottom offset from `chara_pos.y` in mm (`-250.0`, `SLUS_202.28` `vcMoveAndSetCamera` `0x0019df50`).
pub const CHARA_CROUCH_BOTTOM_Y_OFFSET_MM: f32 = -250.0;

/// 5-point Q12 base pitch curve for `Settle` mode (`SLUS_202.28` `0x002b9420`,
/// identical in `sh2pc.exe` `file 0x4b9de0`), domain `0..=0x1921` (`0..=PI/2` in Q12 radians).
pub const BASE_ANG_ORESEN_TABLE: [i32; 5] = [0, 1072, 4289, 450560, 450560];

/// Upper bound of `BASE_ANG_ORESEN_TABLE` domain (`0x1921` = `6433` ≈ `PI/2 * 4096`).
const BASE_ANG_ORESEN_MAX_Q12: i32 = 0x1921;

/// Computes `ideal_cam_pos_h` from `chara_top_y` (`SLUS_202.28` `vcPreSetDataInVC_WORK` `0x00196be0`).
#[inline]
pub fn compute_ideal_cam_pos_h(chara_top_y: f32, is_outdoor: bool) -> f32 {
    chara_top_y
        + if is_outdoor {
            IDEAL_CAM_POS_H_OUTDOOR_OFS_MM
        } else {
            IDEAL_CAM_POS_H_INDOOR_OFS_MM
        }
}

/// Computes character vertical bounds `(chara_bottom_y, chara_top_y, chara_center_y)` in PS2 world mm
/// (`SLUS_202.28` `vcMoveAndSetCamera` `0x0019df50` & `vcSetSubjChara` `0x00195f00`).
///
/// `crouch_rate` is `0.0` when standing (`chara_bottom_y == chara_pos_y`) and `1.0` when fully
/// crouching (`chara_bottom_y == chara_pos_y - 250.0`). `chara_top_y` is always `chara_pos_y - 925.0`.
#[inline]
pub fn compute_chara_y_bounds(chara_pos_y: f32, crouch_rate: f32) -> (f32, f32, f32) {
    let bottom_y = chara_pos_y + crouch_rate * CHARA_CROUCH_BOTTOM_Y_OFFSET_MM;
    let top_y = chara_pos_y + CHARA_TOP_Y_OFFSET_MM;
    let center_y = (bottom_y + top_y) * 0.5;
    (bottom_y, top_y, center_y)
}

/// Computes the ideal camera position for `Chase` (`0`) and `Settle` (`1`) roads
/// (`SLUS_202.28` `vcMakeIdealCamPosUseVC_ROAD_DATA` `0x0019b160`).
#[allow(clippy::too_many_arguments)]
pub fn vc_make_ideal_cam_pos_use_vc_road_data(
    rec: &RoadRecord,
    chara_pos: Vec3,
    chara_eye_ang_y: f32,
    chara_ang_spd_y: f32,
    cur_cam_pos: Vec3,
    ideal_cam_pos_h: f32,
    cam_chara2ideal_ang_y: &mut f32,
    is_outdoor: bool,
    aspect_config: &AspectRailConfig,
) -> Vec3 {
    // 1. ±12° deadband around behind-character yaw (`chara_eye_ang_y + PI`).
    let behind_ang = chara_eye_ang_y + PI;
    let mut diff = sh_angle_regulate(*cam_chara2ideal_ang_y - behind_ang);
    if chara_ang_spd_y.abs() > CHASE_ANG_SPD_THRESHOLD {
        diff = diff.clamp(-CHASE_DEADBAND_RAD, CHASE_DEADBAND_RAD);
    } else if diff >= 0.0 {
        diff = CHASE_DEADBAND_RAD;
    } else {
        diff = -CHASE_DEADBAND_RAD;
    }
    *cam_chara2ideal_ang_y = sh_angle_regulate(diff + behind_ang);

    // 2. Base character-to-camera horizontal distance (`mv_nml_chr2cam_r`).
    let size_idx = rec.area_size_type as usize;
    let mut r = MV_NML_CHR2CAM_R[size_idx];

    // 3. Ideal Y height clamped to `[lim_rd.max_hy, lim_rd.min_hy]`.
    let raw_y = match rec.cam_mv_type {
        CamMovementType::Settle => ideal_cam_pos_h,
        CamMovementType::Chase => {
            let ofs_hy = match rec.params {
                CamMovementParams::Chase(ref chs) => chs.ofs_hy,
                _ => 0.0,
            };
            ideal_cam_pos_h + ofs_hy
        }
        _ => ideal_cam_pos_h,
    };
    let (y_hi, y_lo) = rec.lim_rd.y_bounds();
    let clamped_y = raw_y.clamp(y_hi, y_lo);

    // 4. Compression: measure 3D distance from character to oriented `lim_rd` clamped point.
    let margin = compute_aspect_rail_margin_mm(is_outdoor, aspect_config);
    let ((min_lx, max_lx), (min_lz, max_lz)) = rec.lim_rd.local_xz_bounds();
    let (chr_lx, chr_lz) = rec.lim_rd.world_to_local_xz(chara_pos.x, chara_pos.z);
    let clamped_chr_lx = clamp_axis_with_margin(min_lx, max_lx, margin, chr_lx);
    let clamped_chr_lz = clamp_axis_with_margin(min_lz, max_lz, margin, chr_lz);
    let (clamped_chr_wx, clamped_chr_wz) =
        rec.lim_rd.local_to_world_xz(clamped_chr_lx, clamped_chr_lz);

    let y_excess = ((cur_cam_pos.y - chara_pos.y).abs() - 750.0).max(0.0);
    let dist_to_rail = Vec3::new(
        clamped_chr_wx - chara_pos.x,
        y_excess,
        clamped_chr_wz - chara_pos.z,
    )
    .length();

    let no_adj = MV_NML_NO_ADJ_MAX_DIST[size_idx];
    let full_adj = MV_NML_FULL_ADJ_MIN_DIST[size_idx];
    if dist_to_rail > full_adj {
        r = CHASE_MIN_COMPRESSED_R_MM;
    } else if dist_to_rail > no_adj && (full_adj - no_adj).abs() > f32::EPSILON {
        r += ((CHASE_MIN_COMPRESSED_R_MM - r) * (dist_to_rail - no_adj)) / (full_adj - no_adj);
    }

    // 5. Place camera at compressed radius along `cam_chara2ideal_ang_y` and clamp to oriented `lim_rd`.
    let (s, c) = cam_chara2ideal_ang_y.sin_cos();
    let raw_x = chara_pos.x + r * s;
    let raw_z = chara_pos.z + r * c;
    let (raw_lx, raw_lz) = rec.lim_rd.world_to_local_xz(raw_x, raw_z);
    let final_lx = clamp_axis_with_margin(min_lx, max_lx, margin, raw_lx);
    let final_lz = clamp_axis_with_margin(min_lz, max_lz, margin, raw_lz);
    let (final_wx, final_wz) = rec.lim_rd.local_to_world_xz(final_lx, final_lz);

    Vec3::new(final_wx, clamped_y, final_wz)
}

/// Computes normalized switch-area progress ratio `[0.0, 1.0]` along the road's primary axis
/// (`SLUS_202.28` `vcRetNearRatioSwitchAreaInXZPos` `0x001a67f0`).
pub fn vc_ret_near_ratio_switch_area_in_xz_pos(
    rec: &RoadRecord,
    chara_pos: Vec3,
    cam_tgt_pos: Vec3,
) -> f32 {
    let dir_ang =
        sh_angle_regulate((chara_pos.x - cam_tgt_pos.x).atan2(chara_pos.z - cam_tgt_pos.z));
    let (lx, lz) = rec.lim_sw.world_to_local_xz(chara_pos.x, chara_pos.z);
    let ((min_x, max_x), (min_z, max_z)) = rec.lim_sw.local_xz_bounds();

    if rec.rd_dir_type == 1 {
        let span = max_z - min_z;
        if span.abs() <= f32::EPSILON {
            return 0.0;
        }
        if !(-FRAC_PI_2..FRAC_PI_2).contains(&dir_ang) {
            (lz - min_z) / span
        } else {
            (max_z - lz) / span
        }
    } else if rec.rd_dir_type == 0 {
        let span = max_x - min_x;
        if span.abs() <= f32::EPSILON {
            return 0.0;
        }
        if !(0.0..PI).contains(&dir_ang) {
            (lx - min_x) / span
        } else {
            (max_x - lx) / span
        }
    } else {
        0.0
    }
}

/// Adjusts `tgt_mv_vec.y` according to `rec.mv_y_type` (`0..=5`) (`SLUS_202.28`
/// `vcAdjTgtMvVecYByCurNearRoad` `0x0019b990`).
#[allow(clippy::too_many_arguments)]
pub fn vc_adj_tgt_mv_vec_y_by_cur_near_road(
    tgt_mv_vec: &mut Vec3,
    rec: &RoadRecord,
    cam_tgt_pos: Vec3,
    chara_pos: Vec3,
    chara_top_y: f32,
    ideal_cam_pos_h: f32,
    is_outdoor: bool,
) {
    let dist_xz = (chara_pos.x - cam_tgt_pos.x).hypot(chara_pos.z - cam_tgt_pos.z);
    let ratio = if rec.mv_y_type == 4 || rec.mv_y_type == 5 {
        vc_ret_near_ratio_switch_area_in_xz_pos(rec, chara_pos, cam_tgt_pos)
    } else {
        let (min_d, max_d) = (600.0_f32, 3500.0_f32);
        let clamped_d = dist_xz.clamp(min_d, max_d);
        (max_d - clamped_d) / (max_d - min_d)
    }
    .clamp(0.0, 1.0);

    // Note: in PS2 Y-down coords, `lim_rd.max_hy` (`+0x3c`) is numerically smaller (higher),
    // and `lim_rd.min_hy` (`+0x38`) is numerically larger (lower).
    let lim_min_hy = rec.lim_rd.min_hy;
    let lim_max_hy = rec.lim_rd.max_hy;

    let (y_lo, y_hi) = match rec.mv_y_type {
        0 => {
            let band = ((dist_xz - 150.0) * 0.25).max(0.0);
            let center = chara_top_y - 250.0;
            (center - band, center + band)
        }
        1 => {
            let y = lim_max_hy * ratio + lim_min_hy * (1.0 - ratio);
            (y, y)
        }
        2 => {
            let y = lim_min_hy * ratio + lim_max_hy * (1.0 - ratio);
            (y, y)
        }
        3 => (lim_max_hy, lim_min_hy),
        4 => {
            let out_ofs = if is_outdoor { -250.0 } else { 0.0 };
            let y = ideal_cam_pos_h * ratio + (1.0 - ratio) * (out_ofs + lim_min_hy + chara_pos.y);
            (y, y)
        }
        5 => {
            let out_ofs = if is_outdoor { -250.0 } else { 0.0 };
            let y = ideal_cam_pos_h * ratio + (1.0 - ratio) * (out_ofs + lim_max_hy + chara_pos.y);
            (y, y)
        }
        _ => return,
    };

    let lo = y_lo.min(y_hi);
    let hi = y_lo.max(y_hi);
    let desired_y = (cam_tgt_pos.y + tgt_mv_vec.y).clamp(lo, hi);
    tgt_mv_vec.y = desired_y - cam_tgt_pos.y;
}

/// Computes maximum per-tick horizontal displacement for `cam_tgt_pos` (`SLUS_202.28`
/// `vcRetMaxTgtMvXzLen` `0x0019a2d0`).
pub fn vc_ret_max_tgt_mv_xz_len(
    chara_mv_spd: f32,
    chara_ang_spd_y: f32,
    max_spd_xz: f32,
    is_outdoor: bool,
    dt: f32,
) -> f32 {
    let dyn_spd = chara_mv_spd + 500.0 + (chara_ang_spd_y * 8.0).abs();
    let floor_spd = if is_outdoor { 3300.0 } else { 1200.0 };
    dyn_spd.max(floor_spd).min(max_spd_xz) * dt
}

/// Computes the capped horizontal + vertical move delta from `cam_tgt_pos` to `ideal_pos`
/// (`SLUS_202.28` `vcMakeBasicCamTgtMvVec` `0x0019b850`).
pub fn vc_make_basic_cam_tgt_mv_vec(
    max_xz_len: f32,
    ideal_pos: Vec3,
    cam_tgt_pos: Vec3,
    dt: f32,
    warp_tgt: bool,
) -> Vec3 {
    let delta = ideal_pos - cam_tgt_pos;
    let xz_len = delta.x.hypot(delta.z);
    let (mv_x, mv_z) = if xz_len < max_xz_len || xz_len <= f32::EPSILON {
        (delta.x, delta.z)
    } else {
        let ang = delta.x.atan2(delta.z);
        let (s, c) = ang.sin_cos();
        (max_xz_len * s, max_xz_len * c)
    };
    let mv_y = if dt == 0.0 && !warp_tgt { 0.0 } else { delta.y };
    Vec3::new(mv_x, mv_y, mv_z)
}

/// Computes the base camera Euler angles `(pitch_x, yaw_y, 0.0)` for `Settle` (`1`) mode
/// (`SLUS_202.28` `vcMakeNewBaseCamAng` `0x0019cbe0`).
///
/// For all modes other than `Settle` (or when `user_watch_active` is `true`), returns `Vec3::ZERO`.
#[allow(clippy::too_many_arguments)]
pub fn vc_make_new_base_cam_ang(
    rec: &RoadRecord,
    cam_pos: Vec3,
    watch_tgt_pos: Vec3,
    prev_base_ang: Vec3,
    chara_mv_spd: f32,
    user_watch_active: bool,
    warp_watch: bool,
    dt: f32,
) -> Vec3 {
    if user_watch_active || rec.cam_mv_type != CamMovementType::Settle {
        return Vec3::ZERO;
    }

    let delta = watch_tgt_pos - cam_pos;
    let r_xz = delta.x.hypot(delta.z);
    let pitch = (-delta.y).atan2(r_xz);
    let mut yaw = delta.x.atan2(delta.z);

    if let CamMovementParams::Settle(ref stl) = rec.params {
        let ofs_sta = sh_angle_regulate(yaw - stl.sta_base_ang_y);
        let ofs_end = sh_angle_regulate(yaw - stl.end_base_ang_y);
        if ofs_sta < 0.0 || ofs_end > 0.0 {
            yaw = if ofs_sta.abs() < ofs_end.abs() {
                stl.sta_base_ang_y
            } else {
                stl.end_base_ang_y
            };
        }
    }

    if !warp_watch {
        // `0x37800000` ≈ 1.5258789e-5 sub-epsilon flush in `SLUS_202.28` `0x0019cbe0`.
        let spd = if chara_mv_spd.abs() < 1.525_878_9e-5 {
            0.0
        } else {
            chara_mv_spd
        };
        if spd == 0.0 || pitch.abs() >= 1.308_996_9 {
            yaw = prev_base_ang.y;
        } else {
            let dyaw = sh_angle_regulate(yaw - prev_base_ang.y);
            let max_step = dt * 2.094_395_2;
            yaw = prev_base_ang.y + dyaw.clamp(-max_step, max_step);
        }
    }

    let abs_pitch_q12 = (pitch.abs() * 4096.0) as i32;
    let mapped_q12 = vw_oresen_hokan(
        &BASE_ANG_ORESEN_TABLE,
        abs_pitch_q12,
        0,
        BASE_ANG_ORESEN_MAX_Q12,
    );
    let mut base_pitch = ((mapped_q12 as f32) / 4096.0).clamp(0.0, FRAC_PI_2);
    if pitch < 0.0 {
        base_pitch = -base_pitch;
    }

    Vec3::new(base_pitch, yaw, 0.0)
}

/// Computes the normal gameplay watch target position (`SLUS_202.28` `vcMakeNormalWatchTgtPos`
/// `0x00198d80` & `vcSetWatchTgtXzPos` `0x00199ca0`).
pub fn vc_make_normal_watch_tgt_pos(
    rec: &RoadRecord,
    cam_pos: Vec3,
    chara_pos: Vec3,
    chara_bottom_y: f32,
    chara_eye_ang_y: f32,
) -> Vec3 {
    let size_idx = rec.area_size_type as usize;
    let (mut cir_r, min_dist) = (
        NML_TGT_WATCH_CIR_R[size_idx],
        NML_CAM2WTH_MIN_DIST[size_idx],
    );

    let dx = chara_pos.x - cam_pos.x;
    let dz = chara_pos.z - cam_pos.z;
    let cam2chr_dist = dx.hypot(dz);

    let extra_dist = if rec.cam_mv_type == CamMovementType::Settle {
        cir_r *= 0.5;
        0.0
    } else {
        ((cir_r + min_dist) - cam2chr_dist).max(0.0)
    };

    let cam2chr_ang = dx.atan2(dz);
    let tgt_x = chara_pos.x + cam2chr_ang.sin() * extra_dist + chara_eye_ang_y.sin() * cir_r;
    let tgt_z = chara_pos.z + cam2chr_ang.cos() * extra_dist + chara_eye_ang_y.cos() * cir_r;
    let tgt_y = rec.ofs_watch_hy + chara_bottom_y;
    Vec3::new(tgt_x, tgt_y, tgt_z)
}
