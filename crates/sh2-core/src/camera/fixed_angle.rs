//! Fixed-angle camera positioning and 180° search-button warp flip (`SLUS_202.28` `view/vc_main.c` & `view/vc_newest.c`).
//!
//! Ground truth:
//! - `vcMakeIdealCamPosForFixAngCam` (`0x0019a4c0`)
//! - `vcWarpForFixAngCam` (`0x00252980`)
//! - `vcMakeNormalWatchTgtPos` Mode 2 branch (`0x00198d80`)

use crate::camera::kinematics::{sh_angle_regulate, vw_angle_to_vector};
use crate::types::camera::FixCamManager;
use crate::widescreen_rail::{
    clamp_axis_with_margin, compute_aspect_rail_margin_mm, AspectRailConfig,
};
use glam::Vec3;
use sh2_assets::formats::cam::{CamMovementParams, RoadRecord};
use std::f32::consts::{FRAC_PI_2, PI};

/// Angle threshold (`135°` = `2.3561945` rad = `3*PI/4`) triggering `vcWarpForFixAngCam`
/// (`SLUS_202.28` `0x00252980`).
pub const FIX_ANG_WARP_THRESHOLD_RAD: f32 = 2.356_194_5;

/// Local rail shift in mm (`800.0`) applied during `vcWarpForFixAngCam` (`SLUS_202.28` `0x00252980`).
pub const FIX_ANG_WARP_RAIL_SHIFT_MM: f32 = 800.0;

/// Computes the ideal camera eye position for `FixedAngle` (Mode 2) roads
/// (`SLUS_202.28` `vcMakeIdealCamPosForFixAngCam` `0x0019a4c0`).
///
/// Resets `fix_mng` when `road_changed` (`_VC_FLAGS & 0x20`) is `true`.
#[allow(clippy::too_many_arguments)]
pub fn vc_make_ideal_cam_pos_for_fix_ang_cam(
    rec: &RoadRecord,
    chara_pos: Vec3,
    chara_eye_ang_y: f32,
    cur_cam_pos: Vec3,
    fix_mng: &mut FixCamManager,
    road_changed: bool,
    is_outdoor: bool,
    aspect_config: &AspectRailConfig,
) -> Vec3 {
    if road_changed {
        *fix_mng = FixCamManager::default();
    }

    let (fix_ang_y, cam2wth_dist) = match rec.params {
        CamMovementParams::FixedAngle(ref fix) => (fix.ang_y, fix.cam2wth_dist),
        _ => (0.0, 2000.0),
    };

    let ang_y = sh_angle_regulate(fix_ang_y + fix_mng.add_ang_y);
    let ((min_lx, max_lx), (min_lz, max_lz)) = rec.lim_rd.local_xz_bounds();
    let shifted_min_x = min_lx + fix_mng.add_rd_x;
    let shifted_max_x = max_lx + fix_mng.add_rd_x;
    let shifted_min_z = min_lz + fix_mng.add_rd_z;
    let shifted_max_z = max_lz + fix_mng.add_rd_z;

    // Distance from character to unshifted oriented `lim_rd` bounds.
    let (_sum, ofs_x, ofs_z) =
        rec.lim_rd
            .xz_sum_dist_from_lim_area(chara_pos.x, chara_pos.z, false);
    let back_dist = ofs_x.abs().max(ofs_z.abs()) * 0.5 + 1000.0;

    // Character-facing forward lead scaled by `cam2wth_dist / 2000.0`.
    let scale = cam2wth_dist / 2000.0;
    let max_lead = scale * 350.0;
    let far_cutoff = scale * 3500.0;
    let near_cutoff = scale * 750.0;
    let cam2chr_xz = (chara_pos.x - cur_cam_pos.x).hypot(chara_pos.z - cur_cam_pos.z);
    let lead = if cam2chr_xz >= far_cutoff {
        0.0
    } else if cam2chr_xz > near_cutoff && (near_cutoff - far_cutoff).abs() > f32::EPSILON {
        ((cam2chr_xz - far_cutoff) / (near_cutoff - far_cutoff)) * max_lead
    } else {
        max_lead
    };

    let back_ang = ang_y + PI;
    let raw_x = chara_pos.x + lead * chara_eye_ang_y.sin() + back_dist * back_ang.sin();
    let raw_z = chara_pos.z + lead * chara_eye_ang_y.cos() + back_dist * back_ang.cos();

    let margin = compute_aspect_rail_margin_mm(is_outdoor, aspect_config);
    let (raw_lx, raw_lz) = rec.lim_rd.world_to_local_xz(raw_x, raw_z);
    let clamped_lx = clamp_axis_with_margin(shifted_min_x, shifted_max_x, margin, raw_lx);
    let clamped_lz = clamp_axis_with_margin(shifted_min_z, shifted_max_z, margin, raw_lz);
    let (final_wx, final_wz) = rec.lim_rd.local_to_world_xz(clamped_lx, clamped_lz);

    let (y_hi, y_lo) = rec.lim_rd.y_bounds();
    Vec3::new(final_wx, chara_pos.y.clamp(y_hi, y_lo), final_wz)
}

/// Computes the watch target position for `FixedAngle` (Mode 2) roads
/// (`SLUS_202.28` `vcMakeNormalWatchTgtPos` `0x00198d80`).
pub fn vc_make_fix_ang_watch_tgt_pos(
    rec: &RoadRecord,
    cam_pos: Vec3,
    fix_mng: &FixCamManager,
) -> Vec3 {
    let (ang_x, ang_y, dist) = match rec.params {
        CamMovementParams::FixedAngle(ref fix) => (
            fix.ang_x,
            sh_angle_regulate(fix.ang_y + fix_mng.add_ang_y),
            fix.cam2wth_dist,
        ),
        _ => (0.0, 0.0, 2000.0),
    };
    let dir = vw_angle_to_vector(dist, Vec3::new(ang_x, ang_y, 0.0));
    cam_pos + dir
}

/// Evaluates the 180° search-button flip warp for `FixedAngle` (Mode 2) roads with `kind_id == 1`
/// (`SLUS_202.28` `vcWarpForFixAngCam` `0x00252980`).
///
/// Returns `true` when the camera flipped and warp flags (`0x1c`) must be raised.
pub fn vc_warp_for_fix_ang_cam(
    rec: &RoadRecord,
    chara_eye_ang_y: f32,
    fix_mng: &mut FixCamManager,
) -> bool {
    let fix_ang_y = match rec.params {
        CamMovementParams::FixedAngle(ref fix) => fix.ang_y,
        _ => return false,
    };

    let cur_ang_y = sh_angle_regulate(fix_ang_y + fix_mng.add_ang_y);
    let ang_diff = sh_angle_regulate(chara_eye_ang_y - cur_ang_y);

    if ang_diff.abs() < FIX_ANG_WARP_THRESHOLD_RAD || rec.kind_id != 1 {
        return false;
    }

    fix_mng.exception_f = i32::from(fix_mng.exception_f == 0);
    if fix_mng.exception_f == 0 {
        fix_mng.add_ang_y -= PI;
    } else {
        fix_mng.add_ang_y += PI;
    }

    if rec.rd_dir_type == 1 {
        if cur_ang_y > 0.0 {
            fix_mng.add_rd_x += FIX_ANG_WARP_RAIL_SHIFT_MM;
        } else {
            fix_mng.add_rd_x -= FIX_ANG_WARP_RAIL_SHIFT_MM;
        }
    } else if rec.rd_dir_type == 0 {
        if !(-FRAC_PI_2..FRAC_PI_2).contains(&cur_ang_y) {
            fix_mng.add_rd_z -= FIX_ANG_WARP_RAIL_SHIFT_MM;
        } else {
            fix_mng.add_rd_z += FIX_ANG_WARP_RAIL_SHIFT_MM;
        }
    }

    true
}
