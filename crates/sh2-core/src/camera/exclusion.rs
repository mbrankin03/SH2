//! Character front camera exclusion zone pushout (`SLUS_202.28` `view/vc_main.c`).
//!
//! Ground truth:
//! - `vcFlipFromCamExclusionArea` (`0x0019c240`)
//! - `vcCamTgtMvVecIsFlipedFromCharaFront` (`0x0019be70`)
//! - `excl_max_rate` (`0x00390610`, identical in `sh2pc.exe` `file 0x2c6b00`)
//! - `excl_oresen` (`0x002b93f0`, identical in `sh2pc.exe` `file 0x4b9dbc`)

use crate::camera::kinematics::{sh_angle_regulate, vw_oresen_hokan};
use crate::widescreen_rail::{
    clamp_axis_with_margin, compute_aspect_rail_margin_mm, AspectRailConfig,
};
use glam::Vec3;
use sh2_assets::formats::cam::{AreaSizeType, RoadArea};

/// `excl_max_rate` (`SLUS_202.28` `0x00390610`, `sh2pc.exe` `file 0x2c6b00`) indexed by `AreaSizeType`.
pub const EXCL_MAX_RATE: [f32; 4] = [0.7, 0.85, 0.95, 1.0];

/// 9-point Q12 exclusion radius curve over `0..=0x3243` (`0..=PI` in Q12 radians,
/// `SLUS_202.28` `0x002b93f0`, `sh2pc.exe` `file 0x4b9dbc`).
pub const EXCL_ORESEN_TABLE: [i32; 9] = [16384, 16384, 15155, 13107, 11059, 9420, 7782, 6144, 5324];

/// Upper bound of `vwOresenHokan` domain in `vcFlipFromCamExclusionArea` (`0x3243` = `12867` ≈ `PI * 4096`).
const EXCL_ORESEN_MAX_Q12: i32 = 0x3243;

/// Computes how far `cam_pos` penetrates the character's exclusion zone and updates
/// `old_excl_area_r` (`SLUS_202.28` `vcFlipFromCamExclusionArea` `0x0019c240`).
///
/// Returns `(pushout_dist_mm, pushout_ang_y_rad)`:
/// - `pushout_dist_mm > 0.0` when `cam_pos` is closer than the angularly-shaped exclusion radius.
/// - `old_excl_area_r` is initialized on first call (`-1.0`) and shrinks at most `250.0 * dt` mm/s.
pub fn vc_flip_from_cam_exclusion_area(
    chara_eye_ang_y: f32,
    old_excl_area_r: &mut f32,
    cam_pos: Vec3,
    chara_pos: Vec3,
    area_size_type: AreaSizeType,
    dt: f32,
) -> (f32, f32) {
    let dx = cam_pos.x - chara_pos.x;
    let dz = cam_pos.z - chara_pos.z;
    let push_ang = dx.atan2(dz);
    let ang_diff = sh_angle_regulate(push_ang - chara_eye_ang_y).abs();

    let max_rate = EXCL_MAX_RATE[area_size_type as usize];
    let q12_ang = (ang_diff * 4096.0) as i32;
    let curve_q12 = vw_oresen_hokan(&EXCL_ORESEN_TABLE, q12_ang, 0, EXCL_ORESEN_MAX_Q12);
    let mut excl_r = max_rate * ((curve_q12 as f32 * 500.0) / 4096.0);

    if *old_excl_area_r != -1.0 {
        let delta = (excl_r - *old_excl_area_r).max(-250.0 * dt);
        excl_r = *old_excl_area_r + delta;
    }
    *old_excl_area_r = excl_r;

    let dist = dx.hypot(dz);
    let pushout = (excl_r - dist).max(0.0);
    (pushout, push_ang)
}

/// Pushes `tgt_mv_vec` (XZ move delta from `cam_tgt_pos`) outside the character's front exclusion
/// zone while clamping the resulting camera target inside `lim_rd` (`SLUS_202.28`
/// `vcCamTgtMvVecIsFlipedFromCharaFront` `0x0019be70`).
#[allow(clippy::too_many_arguments)]
pub fn vc_cam_tgt_mv_vec_is_fliped_from_chara_front(
    max_mv_xz_len: f32,
    tgt_mv_vec: &mut Vec3,
    cam_tgt_pos: Vec3,
    chara_pos: Vec3,
    chara_eye_ang_y: f32,
    old_excl_area_r: &mut f32,
    lim_rd: &RoadArea,
    area_size_type: AreaSizeType,
    is_outdoor: bool,
    aspect_config: &AspectRailConfig,
    dt: f32,
) {
    let probe = Vec3::new(
        cam_tgt_pos.x + tgt_mv_vec.x,
        0.0,
        cam_tgt_pos.z + tgt_mv_vec.z,
    );
    let (pushout, push_ang) = vc_flip_from_cam_exclusion_area(
        chara_eye_ang_y,
        old_excl_area_r,
        probe,
        chara_pos,
        area_size_type,
        dt,
    );
    if pushout <= 0.0 {
        return;
    }

    let (s, c) = push_ang.sin_cos();
    let pushed_x = probe.x + pushout * s;
    let pushed_z = probe.z + pushout * c;

    // Clamp pushed position inside oriented `lim_rd` using `vcGetMinInRoadDist`.
    let margin = compute_aspect_rail_margin_mm(is_outdoor, aspect_config);
    let (lx, lz) = lim_rd.world_to_local_xz(pushed_x, pushed_z);
    let ((min_lx, max_lx), (min_lz, max_lz)) = lim_rd.local_xz_bounds();
    let clamped_lx = clamp_axis_with_margin(min_lx, max_lx, margin, lx);
    let clamped_lz = clamp_axis_with_margin(min_lz, max_lz, margin, lz);
    let (wx, wz) = lim_rd.local_to_world_xz(clamped_lx, clamped_lz);

    tgt_mv_vec.x = wx - cam_tgt_pos.x;
    tgt_mv_vec.z = wz - cam_tgt_pos.z;

    let mv_len = tgt_mv_vec.x.hypot(tgt_mv_vec.z);
    if mv_len > max_mv_xz_len && mv_len > 0.0 {
        let mv_ang = tgt_mv_vec.x.atan2(tgt_mv_vec.z);
        let (ms, mc) = mv_ang.sin_cos();
        tgt_mv_vec.x = max_mv_xz_len * ms;
        tgt_mv_vec.z = max_mv_xz_len * mc;
    }
}
