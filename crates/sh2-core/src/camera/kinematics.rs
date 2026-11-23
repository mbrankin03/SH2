//! 1:1 velocity-bounded camera kinematics and angular renewal (`view/vw_calc.c` & `view/vc_main.c`).
//!
//! Ground truth verified against:
//! - PS2 prototype `SLUS_202.28` (`docs/decomp/slus/view/{vw_calc.c,vc_main.c,vc_newest.c}`)
//! - Retail PC `sh2pc.exe` (`docs/decomp/sh2pc/sh2pc_00510000.c` `FUN_0051c100` & `.rdata` `0x2c6a00..0x2c6b0f`)
//!
//! Prototype-vs-retail drift noted:
//! - `watch_mv_prm_on_boat`: `SLUS_202.28` (`0x00390540`) `[2.261947, 4.146903, 1.105841, 3.518584]`
//!   vs retail `sh2pc.exe` (`file 0x2c6a30`) `[0.879646, 2.827433, 1.105841, 3.534292]`.
//! - `cam_mv_prm_on_boat`: `SLUS_202.28` (`0x00390590`) `[1500.0, 450.0, 2400.0, 800.0]`
//!   vs retail `sh2pc.exe` (`file 0x2c6a80`) `[3600.0, 1200.0, 4500.0, 1500.0]`.
//! - `vcRenewalCamData`: retail `sh2pc.exe` (`0x0051c100`) scales XZ target delta by `0.002` (`/ 500.0`,
//!   matching Y's `/ 500.0` in `SLUS_202.28` `0x0019c840`) and clamps zero-crossing overshoot per axis.

use crate::types::camera::{CameraMoveParam, CameraWatchParam};
use glam::{Mat3, Vec3};
use sh2_assets::formats::cam::{
    CamMovementParams, CamMovementType, RoadRecord, VC_RD_CAM_LIKE_SETTLE_F,
};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI, TAU};

/// Default PS2 projection screen Z distance in pixels (`SLUS_202.28` `vcSetProjectionValue` `0x0019db80`: `0x43e00000`).
pub const PS2_DEFAULT_PROJECTION_SCR_Z: f32 = 448.0;

/// PS2 effective horizontal screen span in pixels (`SLUS_202.28` `sh2gfw_set_ViewScreenM` `0x001741b0`:
/// `DAT_002ad7e0 (0.88) * 512.0 = 450.56`, stored at `_DAT_010e5914`).
pub const PS2_SCREEN_WIDTH_PX: f32 = 0.88 * 512.0;

/// PS2 effective vertical screen span in pixels (`SLUS_202.28` `sh2gfw_set_ViewScreenM` `0x001741b0`:
/// `DAT_002ad7e4 (1.0) * 448.0 = 448.0`, stored at `_DAT_010e5918`).
pub const PS2_SCREEN_HEIGHT_PX: f32 = 448.0;

/// `watch_mv_prm_user` (`SLUS_202.28` `0x00390510`, identical in `sh2pc.exe` `file 0x2c6a00`),
/// units in rad/s² and rad/s.
pub const WATCH_MV_PRM_USER: CameraWatchParam = CameraWatchParam {
    ang_accel_x: 7.539_823,
    ang_accel_y: 13.823_008,
    max_ang_spd_x: 1.382_301,
    max_ang_spd_y: 4.398_23,
};

/// `watch_mv_prm_nrml` (`SLUS_202.28` `0x00390520`, identical in `sh2pc.exe` `file 0x2c6a10`),
/// units in rad/s² and rad/s.
pub const WATCH_MV_PRM_NRML: CameraWatchParam = CameraWatchParam {
    ang_accel_x: 6.031_858,
    ang_accel_y: 11.058_406,
    max_ang_spd_x: 1.105_841,
    max_ang_spd_y: 3.518_584,
};

/// `watch_mv_prm_outdoor` (`SLUS_202.28` `0x00390530`, identical in `sh2pc.exe` `file 0x2c6a20`),
/// units in rad/s² and rad/s.
pub const WATCH_MV_PRM_OUTDOOR: CameraWatchParam = CameraWatchParam {
    ang_accel_x: 8.482_3,
    ang_accel_y: 15.550_882,
    max_ang_spd_x: 1.555_088,
    max_ang_spd_y: 4.948_009,
};

/// `watch_mv_prm_on_boat` (retail `sh2pc.exe` `file 0x2c6a30`; `SLUS_202.28` `0x00390540` had
/// `[2.261947, 4.146903, 1.105841, 3.518584]`), units in rad/s² and rad/s.
pub const WATCH_MV_PRM_ON_BOAT: CameraWatchParam = CameraWatchParam {
    ang_accel_x: 0.879_646,
    ang_accel_y: 2.827_433,
    max_ang_spd_x: 1.105_841,
    max_ang_spd_y: 3.534_292,
};

/// `self_view_watch_mv_prm` (`SLUS_202.28` `0x00390550`, identical in `sh2pc.exe` `file 0x2c6a40`),
/// units in rad/s² and rad/s.
pub const SELF_VIEW_WATCH_MV_PRM: CameraWatchParam = CameraWatchParam {
    ang_accel_x: 10.555_752,
    ang_accel_y: 19.352_21,
    max_ang_spd_x: 2.764_601,
    max_ang_spd_y: 8.796_459,
};

/// `cam_mv_prm_user` (`SLUS_202.28` `0x00390560`, identical in `sh2pc.exe` `file 0x2c6a50`),
/// units in PS2 mm/s² and mm/s.
pub const CAM_MV_PRM_USER: CameraMoveParam = CameraMoveParam {
    accel_xz: 5000.0,
    accel_y: 1500.0,
    max_spd_xz: 3000.0,
    max_spd_y: 1000.0,
};

/// `cam_mv_prm_nrml` (`SLUS_202.28` `0x00390570`, identical in `sh2pc.exe` `file 0x2c6a60`),
/// units in PS2 mm/s² and mm/s.
pub const CAM_MV_PRM_NRML: CameraMoveParam = CameraMoveParam {
    accel_xz: 4000.0,
    accel_y: 1500.0,
    max_spd_xz: 2400.0,
    max_spd_y: 1000.0,
};

/// `cam_mv_prm_outdoor` (`SLUS_202.28` `0x00390580`, identical in `sh2pc.exe` `file 0x2c6a70`),
/// units in PS2 mm/s² and mm/s.
pub const CAM_MV_PRM_OUTDOOR: CameraMoveParam = CameraMoveParam {
    accel_xz: 5625.0,
    accel_y: 1687.5,
    max_spd_xz: 3375.0,
    max_spd_y: 1125.0,
};

/// `cam_mv_prm_on_boat` (retail `sh2pc.exe` `file 0x2c6a80`; `SLUS_202.28` `0x00390590` had
/// `[1500.0, 450.0, 2400.0, 800.0]`), units in PS2 mm/s² and mm/s.
pub const CAM_MV_PRM_ON_BOAT: CameraMoveParam = CameraMoveParam {
    accel_xz: 3600.0,
    accel_y: 1200.0,
    max_spd_xz: 4500.0,
    max_spd_y: 1500.0,
};

/// Roll angular acceleration and speed cap (`0x4020d97c` = `2.5132742` rad/s² and rad/s,
/// `SLUS_202.28` `vcAdjCamOfsAngByOfsAngSpd` `0x0019d440`).
pub const ROLL_ANG_ACCEL_AND_MAX_SPD: f32 = 2.513_274_2;

/// Roll angular braking gain (`0x4196cbe4` = `18.849556` = `6 * PI`,
/// `SLUS_202.28` `vcAdjCamOfsAngByOfsAngSpd` `0x0019d440`).
pub const ROLL_ANG_LIM_GAIN: f32 = 18.849_556;

/// Sentinel yaw limit value indicating unrestricted yaw (`-17.451548` / `+17.451548` rad ≈ ±1000°,
/// `SLUS_202.28` `vcCorrectCamMatAngForcibly` `0x002527d0`).
pub const UNRESTRICTED_YAW_LIMIT_RAD: f32 = 17.451_548;

/// Normalizes `angle` (radians) into `[-PI, PI)` (`SLUS_202.28` `shAngleRegulate` `0x0016e970`).
#[inline]
pub fn sh_angle_regulate(mut angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    while angle < -PI {
        angle += TAU;
    }
    while angle >= PI {
        angle -= TAU;
    }
    angle
}

/// Converts PS2 projection focal distance `scr_z` into vertical FOV in radians without
/// artificial clamps (`2 * atan((0.5 * 448.0) / scr_z)`).
///
/// Matches `SLUS_202.28` `vcSetProjectionValue` (`0x0019db80`) and `vbCalcViewScreenMatrix`
/// (`0x00195230`): falls back to `448.0` only when `scr_z <= 0.0`.
#[inline]
pub fn fov_y_from_ps2_projection_unclamped(scr_z: f32) -> f32 {
    let safe_z = if scr_z > 0.0 {
        scr_z
    } else {
        PS2_DEFAULT_PROJECTION_SCR_Z
    };
    2.0 * ((0.5 * PS2_SCREEN_HEIGHT_PX) / safe_z).atan()
}

/// Computes `(scr_half_ang_wx, scr_half_ang_wy)` in radians (`SLUS_202.28` `vcPreSetDataInVC_WORK`
/// `0x00196be0` & `vcChangeProjectionValue` `0x0019dc50`).
///
/// Formula: `shAtan2(scr_z, scr_dim) / 2.0` = `scr_dim.atan2(scr_z) * 0.5`.
#[inline]
pub fn compute_screen_half_angles(scr_z: f32) -> (f32, f32) {
    let safe_z = if scr_z > 0.0 {
        scr_z
    } else {
        PS2_DEFAULT_PROJECTION_SCR_Z
    };
    let half_wx = PS2_SCREEN_WIDTH_PX.atan2(safe_z) * 0.5;
    let half_wy = PS2_SCREEN_HEIGHT_PX.atan2(safe_z) * 0.5;
    (half_wx, half_wy)
}

/// Piecewise-linear integer table interpolation (`SLUS_202.28` `vwOresenHokan` `0x0019ed10`).
pub fn vw_oresen_hokan(table: &[i32], value: i32, min_val: i32, max_val: i32) -> i32 {
    if table.is_empty() {
        return 0;
    }
    let last_idx = (table.len() - 1) as i32;
    if last_idx <= 0 || value < min_val {
        return table[0];
    }
    if value >= max_val {
        return table[last_idx as usize];
    }
    let step = (max_val - min_val) / last_idx;
    if step <= 0 {
        return table[0];
    }
    let offset = value - min_val;
    let seg = offset / step;
    if seg >= last_idx {
        return table[last_idx as usize];
    }
    if seg < 0 {
        return table[0];
    }
    let rem = offset % step;
    let idx = seg as usize;
    (rem * table[idx + 1] + table[idx] * (step - rem)) / step
}

/// Caps the component of `vel` along `ang_y` to `lim_len` (`SLUS_202.28` `vwLimitOverLimVector` `0x0019e840`).
pub fn vw_limit_over_lim_vector(lim_len: f32, ang_y: f32, vel: &mut Vec3) {
    let (s, c) = ang_y.sin_cos();
    let over = (vel.z * c + vel.x * s) - lim_len;
    if over > 0.0 {
        vel.x -= over * s;
        vel.z -= over * c;
    }
}

/// Dampens the lateral (perpendicular to `ang_y`) component of `vel` (`SLUS_202.28`
/// `vwDecreaseSideOfVector` `0x0019e8e0`).
pub fn vw_decrease_side_of_vector(dec_val: f32, lim_side: f32, ang_y: f32, vel: &mut Vec3) {
    let side_ang = ang_y + FRAC_PI_2;
    let (s, c) = side_ang.sin_cos();
    let cur_side = vel.x * s + vel.z * c;
    let clamped = cur_side.clamp(-lim_side, lim_side);
    let new_side = if clamped > dec_val {
        clamped - dec_val
    } else if clamped < -dec_val {
        clamped + dec_val
    } else {
        0.0
    };
    let delta = new_side - cur_side;
    vel.x += delta * s;
    vel.z += delta * c;
}

/// Velocity-bounded XZ acceleration toward `tgt_pos` (`SLUS_202.28` `vwRenewalXZVelocityToTargetPos`
/// `0x0019e640`).
///
/// Units: `cur_pos` and `tgt_pos` in PS2 world mm; `vel` in mm/s; `accel`, `side_dec` in mm/s²;
/// `max_spd` in mm/s; `dist_ofs` in mm (`0.1` mm in `SLUS_202.28` `vcRenewalCamData` `0x0019c840`);
/// `dt` in seconds.
///
/// Note on retail PC drift: `sh2pc.exe` (`0x0051c100`) multiplies `(tgt - cur)` by `0.002` (`/ 500.0`),
/// turning `dist_ofs = 0.1` into a `50.0` mm deadband. Using `SLUS_202.28`'s exact mm-space equation
/// converges to within `0.1` mm while preserving identical velocity/acceleration bounds.
#[allow(clippy::too_many_arguments)]
pub fn vw_renewal_xz_velocity_to_target_pos(
    dist_ofs: f32,
    accel: f32,
    max_spd: f32,
    lim_gain: f32,
    side_dec: f32,
    vel: &mut Vec3,
    cur_pos: Vec3,
    tgt_pos: Vec3,
    dt: f32,
) {
    let dx = tgt_pos.x - cur_pos.x;
    let dz = tgt_pos.z - cur_pos.z;
    let to_tgt_ang = dx.atan2(dz);
    let (s, c) = to_tgt_ang.sin_cos();

    vel.x += accel * dt * s;
    vel.z += accel * dt * c;

    let spd = vel.x.hypot(vel.z);
    if spd > max_spd && spd > 0.0 {
        let vel_ang = vel.x.atan2(vel.z);
        let (vs, vc) = vel_ang.sin_cos();
        let excess = spd - max_spd;
        vel.x -= excess * vs;
        vel.z -= excess * vc;
    }

    let dist = dx.hypot(dz);
    let lim_len = (lim_gain * (dist - dist_ofs)).max(0.0);
    vw_limit_over_lim_vector(lim_len, to_tgt_ang, vel);
    vw_decrease_side_of_vector(side_dec * dt, lim_len * 0.5, to_tgt_ang, vel);
}

/// Velocity-bounded 1D scalar kinematics (`SLUS_202.28` `vwRetNewVelocityToTargetVal` `0x0019ea30`).
pub fn vw_ret_new_velocity_to_target_val(
    now_spd: f32,
    now_val: f32,
    tgt_val: f32,
    accel: f32,
    max_spd: f32,
    lim_gain: f32,
    dt: f32,
) -> f32 {
    let diff = tgt_val - now_val;
    let next_spd = if diff >= 0.0 {
        now_spd + accel * dt
    } else {
        now_spd - accel * dt
    };
    let clamped = next_spd.clamp(-max_spd, max_spd);
    let lim = lim_gain * diff.abs();
    if diff >= 0.0 {
        clamped.min(lim)
    } else {
        clamped.max(-lim)
    }
}

/// Velocity-bounded angular speed toward `tgt_ang` (`SLUS_202.28` `vwRetNewAngSpdToTargetAng` `0x0019eb40`).
///
/// Wraps angular difference via `shAngleRegulate` and normalizes by `PI` before delegating to
/// `vwRetNewVelocityToTargetVal`.
pub fn vw_ret_new_ang_spd_to_target_ang(
    now_spd: f32,
    now_ang: f32,
    tgt_ang: f32,
    accel: f32,
    max_spd: f32,
    lim_gain: f32,
    dt: f32,
) -> f32 {
    let diff = sh_angle_regulate(tgt_ang - now_ang);
    vw_ret_new_velocity_to_target_val(now_spd, 0.0, diff / PI, accel, max_spd, lim_gain, dt)
}

/// Resolves active watch and camera movement parameter profiles (`SLUS_202.28`
/// `vcGetUseWatchAndCamMvParam` `0x0019c4b0`).
pub fn vc_get_use_watch_and_cam_mv_param(
    self_view_rate: f32,
    far_watch_rate: f32,
    is_outdoor: bool,
    is_on_boat: bool,
    chara_mv_spd: f32,
    user_watch_override: Option<CameraWatchParam>,
    user_cam_override: Option<CameraMoveParam>,
) -> (CameraWatchParam, CameraMoveParam) {
    let watch_prm = if let Some(user_w) = user_watch_override {
        user_w
    } else {
        let base = if !is_outdoor {
            WATCH_MV_PRM_NRML
        } else if !is_on_boat {
            WATCH_MV_PRM_OUTDOOR
        } else {
            WATCH_MV_PRM_ON_BOAT
        };
        let mut w = CameraWatchParam {
            ang_accel_x: base.ang_accel_x
                + self_view_rate * (SELF_VIEW_WATCH_MV_PRM.ang_accel_x - base.ang_accel_x),
            ang_accel_y: base.ang_accel_y
                + self_view_rate * (SELF_VIEW_WATCH_MV_PRM.ang_accel_y - base.ang_accel_y),
            max_ang_spd_x: base.max_ang_spd_x
                + self_view_rate * (SELF_VIEW_WATCH_MV_PRM.max_ang_spd_x - base.max_ang_spd_x),
            max_ang_spd_y: base.max_ang_spd_y
                + self_view_rate * (SELF_VIEW_WATCH_MV_PRM.max_ang_spd_y - base.max_ang_spd_y),
        };
        if !is_outdoor {
            w.ang_accel_x += base.ang_accel_x * far_watch_rate * 0.5;
            w.ang_accel_y += base.ang_accel_y * far_watch_rate * 0.5;
            w.max_ang_spd_x += base.max_ang_spd_x * far_watch_rate * 0.5;
            w.max_ang_spd_y += base.max_ang_spd_y * far_watch_rate * 0.5;
        }
        w.ang_accel_y += chara_mv_spd.clamp(0.0, 2.0);
        w
    };

    let cam_prm = if let Some(user_c) = user_cam_override {
        user_c
    } else {
        let (base, scale) = if !is_outdoor {
            (CAM_MV_PRM_NRML, far_watch_rate + 1.0)
        } else if !is_on_boat {
            (CAM_MV_PRM_OUTDOOR, 1.0)
        } else {
            (CAM_MV_PRM_ON_BOAT, 1.0)
        };
        CameraMoveParam {
            accel_xz: base.accel_xz * scale,
            accel_y: base.accel_y * scale,
            max_spd_xz: base.max_spd_xz * scale,
            max_spd_y: base.max_spd_y * scale,
        }
    };

    (watch_prm, cam_prm)
}

/// Integrates camera position toward `cam_tgt_pos` using velocity-bounded XZ/Y kinematics
/// (`SLUS_202.28` `vcRenewalCamData` `0x0019c840` + retail `sh2pc.exe` `0x0051c100`).
pub fn vc_renewal_cam_data(
    cam_pos: &mut Vec3,
    cam_tgt_pos: Vec3,
    cam_velo: &mut Vec3,
    cam_mv_ang_y: &mut f32,
    prm: &CameraMoveParam,
    warp: bool,
    dt: f32,
) {
    if warp {
        *cam_mv_ang_y = (cam_tgt_pos.x - cam_pos.x).atan2(cam_tgt_pos.z - cam_pos.z);
        *cam_pos = cam_tgt_pos;
        *cam_velo = Vec3::ZERO;
        return;
    }

    let prev_delta = cam_tgt_pos - *cam_pos;
    vw_renewal_xz_velocity_to_target_pos(
        0.1,
        prm.accel_xz,
        prm.max_spd_xz,
        prm.accel_xz * 0.4,
        6000.0,
        cam_velo,
        *cam_pos,
        cam_tgt_pos,
        dt,
    );
    cam_velo.y = vw_ret_new_velocity_to_target_val(
        cam_velo.y,
        cam_pos.y / 500.0,
        cam_tgt_pos.y / 500.0,
        prm.accel_y,
        prm.max_spd_y,
        prm.accel_y,
        dt,
    );
    *cam_mv_ang_y = cam_velo.x.atan2(cam_velo.z);
    *cam_pos += *cam_velo * dt;

    // Retail `sh2pc.exe` (`0x0051c100`) per-axis zero-crossing clamp to prevent overshoot.
    let next_delta = cam_tgt_pos - *cam_pos;
    if (next_delta.x < 0.0 && prev_delta.x >= 0.0) || (next_delta.x >= 0.0 && prev_delta.x < 0.0) {
        cam_pos.x = cam_tgt_pos.x;
    }
    if (next_delta.y < 0.0 && prev_delta.y >= 0.0) || (next_delta.y >= 0.0 && prev_delta.y < 0.0) {
        cam_pos.y = cam_tgt_pos.y;
    }
    if (next_delta.z < 0.0 && prev_delta.z >= 0.0) || (next_delta.z >= 0.0 && prev_delta.z < 0.0) {
        cam_pos.z = cam_tgt_pos.z;
    }
}

/// Integrates offset camera Euler angles `(pitch_x, yaw_y, roll_z)` toward `tgt_ofs_ang`
/// (`SLUS_202.28` `vcAdjCamOfsAngByOfsAngSpd` `0x0019d440`).
pub fn vc_adj_cam_ofs_ang_by_ofs_ang_spd(
    ofs_ang: &mut Vec3,
    ofs_ang_spd: &mut Vec3,
    tgt_ofs_ang: Vec3,
    prm: &CameraWatchParam,
    dt: f32,
) {
    ofs_ang_spd.x = vw_ret_new_ang_spd_to_target_ang(
        ofs_ang_spd.x,
        ofs_ang.x,
        tgt_ofs_ang.x,
        prm.ang_accel_x,
        prm.max_ang_spd_x,
        prm.ang_accel_x * 8.0,
        dt,
    );
    ofs_ang_spd.y = vw_ret_new_ang_spd_to_target_ang(
        ofs_ang_spd.y,
        ofs_ang.y,
        tgt_ofs_ang.y,
        prm.ang_accel_y,
        prm.max_ang_spd_y,
        prm.ang_accel_y * 3.0,
        dt,
    );
    ofs_ang_spd.z = vw_ret_new_ang_spd_to_target_ang(
        ofs_ang_spd.z,
        ofs_ang.z,
        tgt_ofs_ang.z,
        ROLL_ANG_ACCEL_AND_MAX_SPD,
        ROLL_ANG_ACCEL_AND_MAX_SPD,
        ROLL_ANG_LIM_GAIN,
        dt,
    );
    *ofs_ang += *ofs_ang_spd * dt;
}

/// Adjusts offset camera pitch (`ofs_ang.x`) and yaw (`ofs_ang.y`) so the character's bottom and
/// top bounds remain inside the screen frustum (`SLUS_202.28` `vcAdjCamOfsAngByCharaInScreen` `0x0019d240`).
///
/// Includes both horizontal `scr_half_ang_wx` framing (yaw) and vertical `scr_half_ang_wy` framing
/// (pitch clamped to `±FRAC_PI_6` = ±30°).
pub fn vc_adj_cam_ofs_ang_by_chara_in_screen(
    ofs_ang: &mut Vec3,
    btm_ang: Vec3,
    top_ang: Vec3,
    scr_half_ang_wx: f32,
    scr_half_ang_wy: f32,
) {
    let dyaw = sh_angle_regulate(top_ang.y - ofs_ang.y);
    let dpitch_btm = sh_angle_regulate(btm_ang.x - ofs_ang.x);
    let dpitch_top = sh_angle_regulate(top_ang.x - ofs_ang.x);

    let adj_y = if dyaw > scr_half_ang_wx {
        sh_angle_regulate(dyaw - scr_half_ang_wx)
    } else if dyaw < -scr_half_ang_wx {
        sh_angle_regulate(dyaw + scr_half_ang_wx)
    } else {
        0.0
    };

    let mut adj_x = if dpitch_btm < -scr_half_ang_wy {
        dpitch_btm + scr_half_ang_wy
    } else {
        0.0
    };
    if dpitch_top - adj_x > scr_half_ang_wy {
        adj_x += (dpitch_top - adj_x) - scr_half_ang_wy;
    }
    adj_x = adj_x.clamp(-FRAC_PI_6, FRAC_PI_6);

    ofs_ang.y += adj_y;
    ofs_ang.x += adj_x;
}

/// Forcibly clamps camera yaw within `[lr_lim_ang_y, rr_lim_ang_y]` for `Settle` or
/// `VC_RD_CAM_LIKE_SETTLE_F` `Chase` roads (`SLUS_202.28` `vcCorrectCamMatAngForcibly` `0x002527d0`).
pub fn vc_correct_cam_mat_ang_forcibly(ang: &mut Vec3, rec: &RoadRecord) {
    let settle_or_like_settle = rec.cam_mv_type == CamMovementType::Settle
        || (rec.cam_mv_type == CamMovementType::Chase
            && (rec.flags & VC_RD_CAM_LIKE_SETTLE_F) != 0);
    if !settle_or_like_settle {
        return;
    }
    let (lr_lim, rr_lim) = match rec.params {
        CamMovementParams::Chase(ref chs) => (chs.lr_lim_ang_y, chs.rr_lim_ang_y),
        CamMovementParams::Settle(ref stl) => (stl.lr_lim_ang_y, stl.rr_lim_ang_y),
        _ => (-UNRESTRICTED_YAW_LIMIT_RAD, UNRESTRICTED_YAW_LIMIT_RAD),
    };
    if lr_lim == -UNRESTRICTED_YAW_LIMIT_RAD && rr_lim == UNRESTRICTED_YAW_LIMIT_RAD {
        return;
    }
    let ofs_lr = sh_angle_regulate(ang.y - lr_lim);
    let ofs_rr = sh_angle_regulate(ang.y - rr_lim);
    if ofs_lr < 0.0 || ofs_rr > 0.0 {
        if ofs_lr.abs() < ofs_rr.abs() {
            ang.y = sh_angle_regulate(lr_lim);
        } else {
            ang.y = sh_angle_regulate(rr_lim);
        }
    } else {
        ang.y = sh_angle_regulate(ang.y);
    }
}

/// Builds the PS2 YXZ rotation matrix `Ry(ang.y) * Rx(ang.x) * Rz(ang.z)`
/// (`SLUS_202.28` `vwRotMatrixYXZ` `0x0019edf0`).
#[inline]
pub fn vw_rot_matrix_yxz(ang: Vec3) -> Mat3 {
    Mat3::from_rotation_y(ang.y) * Mat3::from_rotation_x(ang.x) * Mat3::from_rotation_z(ang.z)
}

/// Extracts PS2 YXZ Euler angles `(pitch_x, yaw_y, roll_z)` in radians from a rotation matrix
/// (`SLUS_202.28` `vwMatrixToAngleYXZ` `0x0019ee60`).
pub fn vw_matrix_to_angle_yxz(mat: Mat3) -> Vec3 {
    // Column 2 is forward `M * (0, 0, 1) = (cos(x)*sin(y), -sin(x), cos(x)*cos(y))`.
    let col0 = mat.x_axis;
    let col1 = mat.y_axis;
    let col2 = mat.z_axis;

    let pitch_x = (-col2.y).atan2(col2.x.hypot(col2.z));
    let (yaw_y, roll_z) = if (pitch_x - FRAC_PI_2).abs() <= 1e-6 {
        (col1.x.atan2(col1.z), 0.0)
    } else if (pitch_x + FRAC_PI_2).abs() <= 1e-6 {
        ((-col1.x).atan2(-col1.z), 0.0)
    } else {
        (col2.x.atan2(col2.z), col0.y.atan2(col1.y))
    };
    Vec3::new(pitch_x, yaw_y, roll_z)
}

/// Updates `base_cam_ang` to `new_base_ang` while re-expressing `ofs_cam_ang` so world camera
/// orientation `R_YXZ(base) * R_YXZ(ofs)` remains continuous (`SLUS_202.28`
/// `vcRenewalBaseCamAngAndAdjustOfsCamAng` `0x0019cfa0`).
pub fn vc_renewal_base_cam_ang_and_adjust_ofs_cam_ang(
    base_cam_ang: &mut Vec3,
    ofs_cam_ang: &mut Vec3,
    new_base_ang: Vec3,
) {
    let old_base_mat = vw_rot_matrix_yxz(*base_cam_ang);
    let new_base_mat_t = vw_rot_matrix_yxz(new_base_ang).transpose();
    let ofs_mat = vw_rot_matrix_yxz(*ofs_cam_ang);
    let new_ofs_mat = (new_base_mat_t * old_base_mat) * ofs_mat;
    *ofs_cam_ang = vw_matrix_to_angle_yxz(new_ofs_mat);
    *base_cam_ang = new_base_ang;
}

/// Computes the target offset Euler angles `(pitch_x, yaw_y, watch_tgt_ang_z)` in the transposed
/// base camera frame `base_mat_t` (`SLUS_202.28` `vcMakeOfsCamTgtAng` `0x0019d0f0`).
#[inline]
pub fn vc_make_ofs_cam_tgt_ang(
    base_mat_t: Mat3,
    watch_tgt_pos: Vec3,
    cam_pos: Vec3,
    watch_tgt_ang_z: f32,
) -> Vec3 {
    let local_vec = base_mat_t * (watch_tgt_pos - cam_pos);
    let mut ang = vw_vector_to_angle(local_vec);
    ang.z = watch_tgt_ang_z;
    ang
}

/// Computes character bottom and top target angles in the transposed base camera frame
/// `base_mat_t` (`SLUS_202.28` `vcMakeOfsCam2CharaBottomAndTopAngByBaseMatT` `0x0019d160`).
#[inline]
pub fn vc_make_ofs_cam_2_chara_bottom_and_top_ang_by_base_mat_t(
    chara_btm_y: f32,
    chara_top_y: f32,
    base_mat_t: Mat3,
    cam_pos: Vec3,
    chara_pos: Vec3,
) -> (Vec3, Vec3) {
    let btm_world = Vec3::new(
        chara_pos.x - cam_pos.x,
        chara_btm_y - cam_pos.y,
        chara_pos.z - cam_pos.z,
    );
    let top_world = Vec3::new(
        chara_pos.x - cam_pos.x,
        chara_top_y - cam_pos.y,
        chara_pos.z - cam_pos.z,
    );
    (
        vw_vector_to_angle(base_mat_t * btm_world),
        vw_vector_to_angle(base_mat_t * top_world),
    )
}

/// Composes final camera rotation matrix `R_YXZ(base_ang) * R_YXZ(ofs_ang)` and extracts its
/// YXZ Euler angles (`SLUS_202.28` `vcMakeCamMatAndCamAngByBaseAngAndOfsAng` `0x0019d560`).
#[inline]
pub fn vc_make_cam_mat_and_cam_ang_by_base_ang_and_ofs_ang(
    base_ang: Vec3,
    ofs_ang: Vec3,
) -> (Mat3, Vec3) {
    let cam_mat = vw_rot_matrix_yxz(base_ang) * vw_rot_matrix_yxz(ofs_ang);
    let cam_ang = vw_matrix_to_angle_yxz(cam_mat);
    (cam_mat, cam_ang)
}

/// Converts a world-space direction vector into PS2 `(pitch_x, yaw_y, 0.0)` angles in radians
/// (`SLUS_202.28` `vwVectorToAngle` `0x0019ec70`).
#[inline]
pub fn vw_vector_to_angle(v: Vec3) -> Vec3 {
    let r_xz = v.x.hypot(v.z);
    let pitch_x = (-v.y).atan2(r_xz);
    let yaw_y = v.x.atan2(v.z);
    Vec3::new(pitch_x, yaw_y, 0.0)
}

/// Converts PS2 `(pitch_x, yaw_y)` angles and `length` into a world-space vector
/// (`SLUS_202.28` `vwAngleToVector` `0x0019ebd0`).
#[inline]
pub fn vw_angle_to_vector(length: f32, ang: Vec3) -> Vec3 {
    let horiz = length * ang.x.cos();
    Vec3::new(
        horiz * ang.y.sin(),
        -length * ang.x.sin(),
        horiz * ang.y.cos(),
    )
}

/// Determines whether camera motion between the previous and current frame was smooth
/// (`SLUS_202.28` `vcRetSmoothCamMvF` `0x00196360`).
///
/// Returns `false` (cut/jump) when per-frame position delta exceeds `100.0` mm, pitch delta
/// exceeds `0.34906584` rad (`20°`), or pitch-cosine-weighted yaw delta exceeds `0.5235988` rad (`30°`).
pub fn vc_ret_smooth_cam_mv_f(
    prev_pos: Vec3,
    cur_pos: Vec3,
    prev_ang: Vec3,
    cur_ang: Vec3,
    delta_frames: f32,
) -> bool {
    let df = delta_frames.clamp(1.0, 4.0).trunc().max(1.0);
    let pos_per_frame = (cur_pos - prev_pos).length() / df;
    if pos_per_frame > 100.0 {
        return false;
    }
    let pitch_per_frame = (cur_ang.x - prev_ang.x).abs() / df;
    if pitch_per_frame > 0.349_065_84 {
        return false;
    }
    let dyaw = sh_angle_regulate(cur_ang.y - prev_ang.y).abs();
    let yaw_per_frame = (dyaw / df) * cur_ang.x.cos();
    yaw_per_frame <= FRAC_PI_6
}
