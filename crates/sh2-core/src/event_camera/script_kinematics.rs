//! Scripted event camera constants, stage shot presets, state enums, projection math, and
//! velocity-bounded tracking helpers (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` & Retail PC `sh2pc.exe` references:
//! - `cam_mv_prm_user` (`0x00390560` PS2), `watch_mv_prm_user` (`0x00390510` PS2)
//! - `vcRenewalCamData` (`0x0019c840` PS2)
//! - `vcSetProjectionValue` (`0x0019db80` PS2) & `vcChangeProjectionValue` (`0x0019dc50` PS2)
//! - `stg_forest.c` (`sh2pc.exe` `0x008e77ac`), `stg_bowling.c` (`sh2pc.exe` `0x008de458..0x008de4cc`)

use crate::camera::CameraSnapshot;
use crate::types::camera::{CameraMoveParam, CameraWatchParam};
use crate::types::EventCamSetData;
use glam::{Mat4, Vec3};
use std::f32::consts::PI;

/// `_VC_FLAGS` bit `0x0001`: user/scripted camera position override active (`vcWork.flags` at `0x010e5ad8`).
pub const VC_USER_CAM_F: u32 = 0x0001;
/// `_VC_FLAGS` bit `0x0002`: user/scripted watch target override active (`vcWork.flags` at `0x010e5ad8`).
pub const VC_USER_WATCH_F: u32 = 0x0002;
/// `_VC_FLAGS` bit `0x0004`: warp camera eye immediately on next renewal (`vcWork.flags` at `0x010e5ad8`).
pub const VC_WARP_CAM_F: u32 = 0x0004;
/// `_VC_FLAGS` bit `0x0008`: warp camera watch target immediately (`vcWork.flags` at `0x010e5ad8`).
pub const VC_WARP_WATCH_F: u32 = 0x0008;
/// `_VC_FLAGS` bit `0x0010`: warp camera target velocity state (`vcWork.flags` at `0x010e5ad8`).
pub const VC_WARP_CAM_TGT_F: u32 = 0x0010;
/// `_VC_FLAGS` bit `0x0040`: snap projection value on next `vcSetProjectionValue` (`0x0019dbc8`).
pub const VC_PROJ_MOMENT_CHANGE_F: u32 = 0x0040;
/// `_VC_FLAGS` bit `0x0080`: inhibit far-watch lookahead (`vcUserWatchTarget` at `0x00195bd8`).
pub const VC_INHIBIT_FAR_WATCH_F: u32 = 0x0080;
/// Combined user camera + watch override mask (`VC_USER_CAM_F | VC_USER_WATCH_F`, cleared at `0x00195ecc`).
pub const VC_USER_MODE_FLAGS: u32 = VC_USER_CAM_F | VC_USER_WATCH_F;
/// Combined warp flags set by `vcReturnPreAutoCamWork(1)` (`0x001c` at `0x00195eb8`).
pub const VC_ALL_WARP_FLAGS: u32 = VC_WARP_CAM_F | VC_WARP_WATCH_F | VC_WARP_CAM_TGT_F;

/// Default PS2 projection screen Z distance in pixels (`0x43e00000` = `448.0` in `vcSetProjectionValue` `0x0019db98`).
pub const PS2_DEFAULT_PROJECTION_SCR_Z: f32 = 448.0;

/// Default scripted camera acceleration/speed profile (`cam_mv_prm_user` at `SLUS_202.28` `0x00390560`,
/// units in PS2 mm/s² and mm/s).
pub const DEFAULT_USER_CAM_MV_PARAM: CameraMoveParam = CameraMoveParam {
    accel_xz: 5000.0,
    accel_y: 1500.0,
    max_spd_xz: 3000.0,
    max_spd_y: 1000.0,
};

/// Default scripted watch angular profile (`watch_mv_prm_user` at `SLUS_202.28` `0x00390510`,
/// units in rad/s² and rad/s).
pub const DEFAULT_USER_WATCH_MV_PARAM: CameraWatchParam = CameraWatchParam {
    ang_accel_x: 7.539_823,
    ang_accel_y: 13.823_008,
    max_ang_spd: 1.382_300_7,
};

/// Typed descriptor for a scripted event camera shot (`vcSetEventCamParamRefView` at `0x0019db10`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScriptedCameraShot {
    /// World-space camera eye position in PS2 millimeters (Y-down).
    pub eye_mm: Vec3,
    /// World-space look-at target in PS2 millimeters (Y-down).
    pub watch_mm: Vec3,
    /// Roll angle around the forward view axis in radians (`watch_tgt_ang_z` at `vcWork+0x0cc`).
    pub roll_radians: f32,
    /// Optional PS2 projection focal distance `scr_z` (`448.0` default -> `~53.1°` vertical FOV).
    pub projection_scr_z: Option<f32>,
}

impl ScriptedCameraShot {
    /// Converts a 64-byte stage `EventCamSetData` (`Event_CamSetData`, e.g. `APART_E3FE_ROOM13_CAMERA`)
    /// into a [`ScriptedCameraShot`].
    pub fn from_event_cam_set_data(data: &EventCamSetData) -> Self {
        let projection_scr_z = if data.fov > 1.0 {
            let half_rad = (data.fov.to_radians() * 0.5).clamp(0.15, 1.35);
            Some(224.0 / half_rad.tan())
        } else {
            None
        };
        Self {
            eye_mm: Vec3::new(data.cam_pos[0], data.cam_pos[1], data.cam_pos[2]),
            watch_mm: Vec3::new(data.cam_watch[0], data.cam_watch[1], data.cam_watch[2]),
            roll_radians: data.roll,
            projection_scr_z,
        }
    }
}

/// Forest Cemetery / Angela graveyard scripted event shot (`stg_forest.c` at `sh2pc.exe` `0x0059f7ac`,
/// `.rdata` `0x008e77ac..0x008e77cb`).
pub const FOREST_CEMETERY_ANGELA_CAM: ScriptedCameraShot = ScriptedCameraShot {
    eye_mm: Vec3::new(-10_268.779, 1_299.030_3, 375.759_9),
    watch_mm: Vec3::new(259.542_6, 2_452.223_9, 3_037.364_7),
    roll_radians: 0.0,
    projection_scr_z: Some(PS2_DEFAULT_PROJECTION_SCR_Z),
};

/// Pete's Bowl-O-Rama / Eddie & Laura scripted shot #1 (`stg_bowling.c` at `sh2pc.exe` `0x00594170`,
/// `.rdata` `0x008de458`, `scr_z = 716.0` / `0x44330000`).
pub const BOWLING_ALLEY_EDDIE_CAM_1: ScriptedCameraShot = ScriptedCameraShot {
    eye_mm: Vec3::new(-73_750.0, -2_000.0, 62_250.0),
    watch_mm: Vec3::new(-73_998.6, -1_568.7, 62_203.0),
    roll_radians: 0.171_806,
    projection_scr_z: Some(716.0),
};

/// Pete's Bowl-O-Rama scripted shot #2 (`sh2pc.exe` `0x005941d5`, `.rdata` `0x008de48c`, `scr_z = 822.0`).
pub const BOWLING_ALLEY_EDDIE_CAM_2: ScriptedCameraShot = ScriptedCameraShot {
    eye_mm: Vec3::new(-69_500.0, -2_700.0, 62_800.0),
    watch_mm: Vec3::new(-69_718.27, -2_278.82, 62_642.0),
    roll_radians: 0.0,
    projection_scr_z: Some(822.0),
};

/// Pete's Bowl-O-Rama entrance scripted shot #4 (`sh2pc.exe` `0x005943a0`, `.rdata` `0x008de4cc`, `scr_z = 600.0`).
pub const BOWLING_ALLEY_ENTRANCE_CAM: ScriptedCameraShot = ScriptedCameraShot {
    eye_mm: Vec3::new(-22_860.0, -1_000.0, 54_484.0),
    watch_mm: Vec3::new(-23_047.426, -858.912, 54_042.45),
    roll_radians: 0.0,
    projection_scr_z: Some(600.0),
};

/// Transition timing policy for scripted camera entry and release (`warp_f` in `vcSetEventCamParamRefView`
/// `0x0019db10` and `vcReturnPreAutoCamWork` `0x00195ea0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTransitionKind {
    /// Instant camera cut (`warp_f != 0`): snaps both `curr_snapshot` and `prev_snapshot` with zero smear.
    WarpCut,
    /// Velocity-bounded smooth camera pan/dolly (`warp_f == 0`) driven by `CameraMoveParam` & `CameraWatchParam`.
    SmoothBlend,
}

/// Timing policy for `vcSetProjectionValue` (`0x0019db80`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionChangeTiming {
    /// Snap immediately on the next tick (`frame_count < 2` or `VC_PROJ_MOMENT_CHANGE_F`, `mode = 2`).
    Instant,
    /// Linearly interpolate `scr_z` over `ticks` 60 Hz frames (`mode = 1` at `0x0019dc08`).
    OverTicks(u16),
}

/// State machine mirroring `vcProjectionParam` (`0x010e5a90..0x010e5a9f` in `SLUS_202.28`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ProjectionTransitionState {
    /// No active projection transition (`vcProjectionParam.mode == 0`).
    #[default]
    Idle,
    /// Snap `scr_z` immediately on the current tick (`vcProjectionParam.mode == 2`).
    InstantSnap {
        /// Target focal length `scr_z`.
        target_scr_z: f32,
    },
    /// Step `scr_z` linearly per tick (`vcProjectionParam.mode == 1` in `vcChangeProjectionValue` `0x0019dc50`).
    Interpolating {
        /// Current focal length `scr_z`.
        current_scr_z: f32,
        /// Start focal length `scr_z` (`+0x04`).
        start_scr_z: f32,
        /// Target focal length `scr_z` (`+0x00`).
        target_scr_z: f32,
        /// Per-tick delta (`(target - start) / frames` at `+0x08`).
        step_per_tick: f32,
    },
}

/// Active state machine for scripted event & `.mes` dialogue camera control (`vcWork.flags & 0x03`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ScriptCameraMode {
    /// Standard automatic `.cam` rail framing (`(vcWork.flags & VC_USER_MODE_FLAGS) == 0`).
    #[default]
    AutoGameplay,
    /// Holds gameplay camera steady and locks out right-stick search panning while a `.mes`
    /// dialogue box is open (`EventExecMessage` at `0x001a9cc0` / `EvSubMessage` at `0x001abf50`).
    DialogueHold {
        /// Active `.mes` message identifier.
        message_id: u16,
        /// Frozen camera snapshot captured when dialogue began.
        held_snapshot: CameraSnapshot,
    },
    /// Active scripted override (`VC_USER_CAM_F` and/or `VC_USER_WATCH_F` set via `vcSetCameraScript`
    /// / `vcSetEventCamParamRefView` `0x0019db10`).
    ScriptedOverride {
        /// Optional scripted eye target in PS2 millimeters (`VC_USER_CAM_F`).
        target_eye_mm: Option<Vec3>,
        /// Optional scripted look-at target in PS2 millimeters (`VC_USER_WATCH_F`).
        target_watch_mm: Option<Vec3>,
        /// Roll angle around the forward view axis in radians (`watch_tgt_ang_z`).
        roll_radians: f32,
        /// Pending transition mode (`WarpCut` snaps once then settles; `SmoothBlend` tracks continuously).
        transition: ScriptTransitionKind,
        /// Active eye velocity in PS2 mm/s (`vcWork.cam_velo`).
        eye_velocity_mm_per_s: Vec3,
        /// Acceleration & speed caps (`vcWork.user_cam_mv_prm`).
        cam_prm: CameraMoveParam,
        /// Angular acceleration & speed caps (`vcWork.user_watch_mv_prm`).
        watch_prm: CameraWatchParam,
    },
    /// Smoothly blending back from a scripted override into automatic `.cam` framing after
    /// `vcReturnPreAutoCamWork(0)` (`0x00195ea0`).
    ReturningToAuto {
        /// Snapshot at the moment `vcReturnPreAutoCamWork(0)` was invoked.
        from_snapshot: CameraSnapshot,
        /// Ticks elapsed in the return blend (`0..total_ticks`).
        elapsed_ticks: u16,
        /// Total 60 Hz ticks for the return blend.
        total_ticks: u16,
    },
}

/// Converts PS2 projection focal distance `scr_z` into vertical FOV in radians (`2 * atan(224.0 / scr_z)`).
#[inline]
pub fn fov_y_from_ps2_projection(scr_z: f32) -> f32 {
    let safe_z = if scr_z > 1.0 {
        scr_z
    } else {
        PS2_DEFAULT_PROJECTION_SCR_Z
    };
    (2.0 * (224.0 / safe_z).atan()).clamp(0.25, 1.45)
}

/// Inverse conversion from vertical FOV in radians back to PS2 `scr_z` (`224.0 / tan(fov_y * 0.5)`).
#[inline]
pub fn ps2_projection_from_fov_y(fov_y_rad: f32) -> f32 {
    let half = (fov_y_rad * 0.5).clamp(0.10, 1.30);
    224.0 / half.tan()
}

/// Computes a PS2 Y-down `up` vector rotated by `roll_radians` around `forward` (`VbSetCameraMatrix`).
#[inline]
pub fn rolled_ps2_up_vector(eye: Vec3, target: Vec3, roll_radians: f32) -> Vec3 {
    let base_up = Vec3::new(0.0, -1.0, 0.0);
    if roll_radians.abs() <= 1e-6 {
        return base_up;
    }
    let fwd = (target - eye).normalize_or_zero();
    if fwd.length_squared() <= 1e-6 {
        return base_up;
    }
    let rolled = Mat4::from_axis_angle(fwd, roll_radians)
        .transform_vector3(base_up)
        .normalize_or_zero();
    if rolled.length_squared() > 1e-6 {
        rolled
    } else {
        base_up
    }
}

pub(crate) fn step_projection_state(
    projection: &mut ProjectionTransitionState,
    fallback_fov_y: f32,
) -> f32 {
    match projection {
        ProjectionTransitionState::Idle => fallback_fov_y,
        ProjectionTransitionState::InstantSnap { target_scr_z } => {
            let fov = fov_y_from_ps2_projection(*target_scr_z);
            *projection = ProjectionTransitionState::Idle;
            fov
        }
        ProjectionTransitionState::Interpolating {
            current_scr_z,
            start_scr_z,
            target_scr_z,
            step_per_tick,
        } => {
            let next = *current_scr_z + *step_per_tick;
            let reached = if *target_scr_z >= *start_scr_z {
                next >= *target_scr_z
            } else {
                next <= *target_scr_z
            };
            if reached {
                let fov = fov_y_from_ps2_projection(*target_scr_z);
                *projection = ProjectionTransitionState::Idle;
                fov
            } else {
                *current_scr_z = next;
                fov_y_from_ps2_projection(next)
            }
        }
    }
}

pub(crate) fn step_user_eye_pos(
    cur_eye: Vec3,
    tgt_eye: Vec3,
    vel: &mut Vec3,
    prm: &CameraMoveParam,
    dt: f32,
) -> Vec3 {
    let delta = tgt_eye - cur_eye;
    let dist_xz = (delta.x * delta.x + delta.z * delta.z).sqrt();
    if dist_xz > 0.5 {
        let dir_x = delta.x / dist_xz;
        let dir_z = delta.z / dist_xz;
        let desired_spd_xz = (dist_xz * 4.0).min(prm.max_spd_xz);
        let max_dv_xz = prm.accel_xz * dt;
        vel.x += (dir_x * desired_spd_xz - vel.x).clamp(-max_dv_xz, max_dv_xz);
        vel.z += (dir_z * desired_spd_xz - vel.z).clamp(-max_dv_xz, max_dv_xz);
    } else {
        vel.x = 0.0;
        vel.z = 0.0;
    }

    let desired_spd_y = (delta.y * 4.0).clamp(-prm.max_spd_y, prm.max_spd_y);
    let max_dv_y = prm.accel_y * dt;
    vel.y += (desired_spd_y - vel.y).clamp(-max_dv_y, max_dv_y);

    let step = *vel * dt;
    Vec3::new(
        if step.x.abs() >= delta.x.abs() {
            tgt_eye.x
        } else {
            cur_eye.x + step.x
        },
        if step.y.abs() >= delta.y.abs() {
            tgt_eye.y
        } else {
            cur_eye.y + step.y
        },
        if step.z.abs() >= delta.z.abs() {
            tgt_eye.z
        } else {
            cur_eye.z + step.z
        },
    )
}

pub(crate) fn step_user_watch_target(
    eye: Vec3,
    cur_target: Vec3,
    tgt_watch: Vec3,
    prm: &CameraWatchParam,
    dt: f32,
) -> Vec3 {
    let cur_vec = cur_target - eye;
    let dst_vec = tgt_watch - eye;
    if cur_vec.length() <= 1.0 || dst_vec.length() <= 1.0 {
        return tgt_watch;
    }
    let mut dyaw = dst_vec.x.atan2(dst_vec.z) - cur_vec.x.atan2(cur_vec.z);
    while dyaw < -PI {
        dyaw += 2.0 * PI;
    }
    while dyaw >= PI {
        dyaw -= 2.0 * PI;
    }
    let max_step = (prm.max_ang_spd * 2.5 * dt).max(0.01);
    if dyaw.abs() <= max_step && (dst_vec.length() - cur_vec.length()).abs() <= 50.0 {
        return tgt_watch;
    }
    let blend = (max_step / dyaw.abs().max(max_step)).clamp(0.18, 1.0);
    cur_target.lerp(tgt_watch, blend)
}
