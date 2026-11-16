//! Scripted event camera controller (`vcSetEventCamParamRefView` / `vcSetCameraScript`,
//! `vcUserCamTarget`, `vcUserWatchTarget`, `vcReturnPreAutoCamWork`), projection blending
//! (`vcSetProjectionValue`, `vcChangeProjectionValue`), and `.mes` dialogue camera locks (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` & Retail PC `sh2pc.exe` references:
//! - `vcUserWatchTarget` (`0x00195bd0` PS2 / `0x00519950` PC)
//! - `vcUserCamTarget` (`0x00195ca0` PS2 / `0x00519a00` PC)
//! - `vcReturnPreAutoCamWork` (`0x00195ea0` PS2 / `0x0051d520` PC)
//! - `vcSetEventCamParamRefView` (`0x0019db10` PS2 / `0x0051d280` PC; angle variant `0x0051d1c0` PC)
//! - `vcSetProjectionValue` (`0x0019db80` PS2 / `0x0051d2b0` PC)
//! - `EventExecMessage` (`0x001a9cc0` PS2) & `EvSubMessage` (`0x001abf50` PS2)

pub use super::script_kinematics::{
    fov_y_from_ps2_projection, ps2_projection_from_fov_y, rolled_ps2_up_vector,
    ProjectionChangeTiming, ProjectionTransitionState, ScriptCameraMode, ScriptTransitionKind,
    ScriptedCameraShot, BOWLING_ALLEY_EDDIE_CAM_1, BOWLING_ALLEY_EDDIE_CAM_2,
    BOWLING_ALLEY_ENTRANCE_CAM, DEFAULT_USER_CAM_MV_PARAM, DEFAULT_USER_WATCH_MV_PARAM,
    FOREST_CEMETERY_ANGELA_CAM, PS2_DEFAULT_PROJECTION_SCR_Z, VC_ALL_WARP_FLAGS,
    VC_INHIBIT_FAR_WATCH_F, VC_PROJ_MOMENT_CHANGE_F, VC_USER_CAM_F, VC_USER_MODE_FLAGS,
    VC_USER_WATCH_F, VC_WARP_CAM_F, VC_WARP_CAM_TGT_F, VC_WARP_WATCH_F,
};

use super::handoff_types::smoothstep_camera_snapshot;
use super::script_kinematics::{step_projection_state, step_user_eye_pos, step_user_watch_target};
use crate::camera::{CameraSnapshot, Ps2CameraController};
use crate::events::CameraOverrideCommand;
use crate::types::camera::{CameraMoveParam, CameraWatchParam};
use glam::{EulerRot, Quat, Vec3};

/// Deterministic 60 Hz scripted event & `.mes` dialogue camera controller (`vc_main.c` / `vc_util.c`).
#[derive(Debug, Clone, Default)]
pub struct ScriptCameraController {
    /// Current override / dialogue state machine.
    pub mode: ScriptCameraMode,
    /// Current projection interpolation state (`vcProjectionParam`).
    pub projection: ProjectionTransitionState,
    /// Simulated `_VC_FLAGS` register (`vcWork.flags` at `0x010e5ad8`).
    pub vc_flags: u32,
}

impl ScriptCameraController {
    /// Creates an idle scripted camera controller in `AutoGameplay` mode.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` while a scripted shot or `.mes` dialogue hold owns the camera.
    #[inline]
    pub const fn is_overriding(&self) -> bool {
        !matches!(self.mode, ScriptCameraMode::AutoGameplay)
    }

    /// Locks or blends both camera eye and watch target to `shot` (`vcSetEventCamParamRefView`
    /// at `SLUS_202.28` `0x0019db10` / `sh2pc.exe` `0x0051d280`, `vcSetCameraScript`).
    pub fn set_camera_script(
        &mut self,
        shot: ScriptedCameraShot,
        transition: ScriptTransitionKind,
        current_fov_y: f32,
    ) {
        self.vc_flags = (self.vc_flags & !(VC_INHIBIT_FAR_WATCH_F | VC_ALL_WARP_FLAGS))
            | VC_USER_CAM_F
            | VC_USER_WATCH_F;
        if transition == ScriptTransitionKind::WarpCut {
            self.vc_flags |= VC_WARP_CAM_F | VC_WARP_WATCH_F;
        }
        if let Some(scr_z) = shot.projection_scr_z {
            let timing = match transition {
                ScriptTransitionKind::WarpCut => ProjectionChangeTiming::Instant,
                ScriptTransitionKind::SmoothBlend => ProjectionChangeTiming::OverTicks(30),
            };
            self.set_projection_value(current_fov_y, scr_z, timing);
        }
        self.mode = ScriptCameraMode::ScriptedOverride {
            target_eye_mm: Some(shot.eye_mm),
            target_watch_mm: Some(shot.watch_mm),
            roll_radians: shot.roll_radians,
            transition,
            eye_velocity_mm_per_s: Vec3::ZERO,
            cam_prm: DEFAULT_USER_CAM_MV_PARAM,
            watch_prm: DEFAULT_USER_WATCH_MV_PARAM,
        };
    }

    /// Angle-based scripted camera override (`sh2pc.exe` `0x0051d1c0`): derives the watch target
    /// from `eye_mm` and PS2 Euler angles `(pitch_x, yaw_y, roll_z)` in radians.
    pub fn set_camera_script_from_angles(
        &mut self,
        eye_mm: Vec3,
        euler_xyz_rad: Vec3,
        projection_scr_z: Option<f32>,
        transition: ScriptTransitionKind,
        current_fov_y: f32,
    ) {
        let rot = Quat::from_euler(EulerRot::YXZ, euler_xyz_rad.y, euler_xyz_rad.x, 0.0);
        let forward = rot * Vec3::new(0.0, 0.0, 1.0);
        let shot = ScriptedCameraShot {
            eye_mm,
            watch_mm: eye_mm + forward * 2000.0,
            roll_radians: euler_xyz_rad.z,
            projection_scr_z,
        };
        self.set_camera_script(shot, transition, current_fov_y);
    }

    /// Overrides only the camera eye target (`vcUserCamTarget` at `SLUS_202.28` `0x00195ca0`),
    /// allowing automatic `.cam` watch-target tracking to continue framing the player.
    pub fn set_user_cam_target(
        &mut self,
        target_eye_mm: Vec3,
        cam_prm: Option<CameraMoveParam>,
        transition: ScriptTransitionKind,
    ) {
        self.vc_flags = (self.vc_flags & !VC_WARP_CAM_F) | VC_USER_CAM_F;
        if transition == ScriptTransitionKind::WarpCut {
            self.vc_flags |= VC_WARP_CAM_F;
        }
        self.ensure_override_slot(transition);
        if let ScriptCameraMode::ScriptedOverride {
            target_eye_mm: eye_slot,
            transition: tr_slot,
            cam_prm: prm_slot,
            ..
        } = &mut self.mode
        {
            *eye_slot = Some(target_eye_mm);
            *tr_slot = transition;
            *prm_slot = cam_prm.unwrap_or(DEFAULT_USER_CAM_MV_PARAM);
        }
    }

    /// Overrides only the camera look-at target (`vcUserWatchTarget` at `SLUS_202.28` `0x00195bd0`).
    pub fn set_user_watch_target(
        &mut self,
        target_watch_mm: Vec3,
        watch_prm: Option<CameraWatchParam>,
        roll_radians: f32,
        transition: ScriptTransitionKind,
    ) {
        self.vc_flags = (self.vc_flags & !VC_INHIBIT_FAR_WATCH_F) | VC_USER_WATCH_F;
        if transition == ScriptTransitionKind::WarpCut {
            self.vc_flags |= VC_WARP_WATCH_F;
        }
        self.ensure_override_slot(transition);
        if let ScriptCameraMode::ScriptedOverride {
            target_watch_mm: watch_slot,
            roll_radians: roll_slot,
            transition: tr_slot,
            watch_prm: prm_slot,
            ..
        } = &mut self.mode
        {
            *watch_slot = Some(target_watch_mm);
            *roll_slot = roll_radians;
            *tr_slot = transition;
            *prm_slot = watch_prm.unwrap_or(DEFAULT_USER_WATCH_MV_PARAM);
        }
    }

    fn ensure_override_slot(&mut self, transition: ScriptTransitionKind) {
        if !matches!(self.mode, ScriptCameraMode::ScriptedOverride { .. }) {
            self.mode = ScriptCameraMode::ScriptedOverride {
                target_eye_mm: None,
                target_watch_mm: None,
                roll_radians: 0.0,
                transition,
                eye_velocity_mm_per_s: Vec3::ZERO,
                cam_prm: DEFAULT_USER_CAM_MV_PARAM,
                watch_prm: DEFAULT_USER_WATCH_MV_PARAM,
            };
        }
    }

    /// Schedules a PS2 projection focal change (`vcSetProjectionValue` at `SLUS_202.28` `0x0019db80`).
    pub fn set_projection_value(
        &mut self,
        current_fov_y: f32,
        scr_z: f32,
        timing: ProjectionChangeTiming,
    ) {
        let target_scr_z = if scr_z.abs() < f32::EPSILON {
            PS2_DEFAULT_PROJECTION_SCR_Z
        } else {
            scr_z
        };
        let moment_flag = (self.vc_flags & VC_PROJ_MOMENT_CHANGE_F) != 0;
        if moment_flag {
            self.vc_flags &= !VC_PROJ_MOMENT_CHANGE_F;
        }
        match timing {
            ProjectionChangeTiming::Instant => {
                self.projection = ProjectionTransitionState::InstantSnap { target_scr_z };
            }
            ProjectionChangeTiming::OverTicks(ticks) if moment_flag || ticks < 2 => {
                self.projection = ProjectionTransitionState::InstantSnap { target_scr_z };
            }
            ProjectionChangeTiming::OverTicks(ticks) => {
                let start_scr_z = ps2_projection_from_fov_y(current_fov_y);
                let step_per_tick = (target_scr_z - start_scr_z) / f32::from(ticks);
                self.projection = ProjectionTransitionState::Interpolating {
                    current_scr_z: start_scr_z,
                    start_scr_z,
                    target_scr_z,
                    step_per_tick,
                };
            }
        }
    }

    /// Consumes a stage `CameraOverrideCommand` (`StageEventDispatcher` / `APART_E3FE_ROOM13_CAMERA`).
    pub fn apply_override_command(
        &mut self,
        cmd: CameraOverrideCommand,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        match cmd {
            CameraOverrideCommand::Lock(data) => {
                let shot = ScriptedCameraShot::from_event_cam_set_data(&data);
                self.set_camera_script(
                    shot,
                    ScriptTransitionKind::WarpCut,
                    camera.curr_snapshot.fov_y,
                );
                self.apply_tick(camera, 1.0 / 60.0);
            }
            CameraOverrideCommand::Release => {
                self.return_pre_auto_cam_work(
                    ScriptTransitionKind::WarpCut,
                    camera,
                    player_pos,
                    player_yaw,
                );
            }
        }
    }

    /// Locks or blends the camera when a `.mes` stage/common dialogue event begins
    /// (`EventExecMessage` at `0x001a9cc0` / `EvSubMessage` at `0x001abf50`).
    pub fn begin_mes_dialogue_camera(
        &mut self,
        message_id: u16,
        camera: &mut Ps2CameraController,
        scripted_shot: Option<(ScriptedCameraShot, ScriptTransitionKind)>,
    ) {
        camera.search_yaw_offset = 0.0;
        camera.search_pitch_offset = 0.0;
        if let Some((shot, transition)) = scripted_shot {
            self.set_camera_script(shot, transition, camera.curr_snapshot.fov_y);
            if transition == ScriptTransitionKind::WarpCut {
                self.apply_tick(camera, 1.0 / 60.0);
            }
        } else {
            self.mode = ScriptCameraMode::DialogueHold {
                message_id,
                held_snapshot: camera.curr_snapshot,
            };
            camera.prev_snapshot = camera.curr_snapshot;
        }
    }

    /// Releases `.mes` dialogue or scripted camera control back to automatic `.cam` volume framing
    /// (`vcReturnPreAutoCamWork` at `SLUS_202.28` `0x00195ea0` / `sh2pc.exe` `0x0051d520`).
    pub fn return_pre_auto_cam_work(
        &mut self,
        transition: ScriptTransitionKind,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        self.vc_flags &= !VC_USER_MODE_FLAGS;
        let active_proj = camera
            .active_road_index
            .and_then(|idx| camera.cam_file.as_ref()?.records.get(idx))
            .map_or(PS2_DEFAULT_PROJECTION_SCR_Z, |r| r.projection);
        self.set_projection_value(
            camera.curr_snapshot.fov_y,
            active_proj,
            ProjectionChangeTiming::Instant,
        );

        match transition {
            ScriptTransitionKind::WarpCut => {
                self.vc_flags |= VC_ALL_WARP_FLAGS;
                self.mode = ScriptCameraMode::AutoGameplay;
                self.projection = ProjectionTransitionState::Idle;
                camera.raw_state.flags = self.vc_flags;
                camera.reset_to_player(player_pos, player_yaw);
            }
            ScriptTransitionKind::SmoothBlend => {
                let from_snapshot = camera.curr_snapshot;
                self.mode = ScriptCameraMode::ReturningToAuto {
                    from_snapshot,
                    elapsed_ticks: 0,
                    total_ticks: 24,
                };
                camera.raw_state.flags = self.vc_flags;
            }
        }
    }

    /// Applies one 60 Hz tick (`dt_secs` seconds) of scripted camera override / return-blend /
    /// projection stepping onto `camera`.
    pub fn apply_tick(&mut self, camera: &mut Ps2CameraController, dt_secs: f32) {
        let dt = dt_secs.clamp(0.0, 0.1);
        let next_fov = step_projection_state(&mut self.projection, camera.curr_snapshot.fov_y);

        match &mut self.mode {
            ScriptCameraMode::AutoGameplay => {
                camera.curr_snapshot.fov_y = next_fov;
            }
            ScriptCameraMode::DialogueHold { held_snapshot, .. } => {
                camera.search_yaw_offset = 0.0;
                camera.search_pitch_offset = 0.0;
                camera.ideal_eye = held_snapshot.eye;
                camera.ideal_target = held_snapshot.target;
                camera.curr_snapshot = *held_snapshot;
                camera.prev_snapshot = *held_snapshot;
            }
            ScriptCameraMode::ScriptedOverride {
                target_eye_mm,
                target_watch_mm,
                roll_radians,
                transition,
                eye_velocity_mm_per_s,
                cam_prm,
                watch_prm,
            } => {
                camera.search_yaw_offset = 0.0;
                camera.search_pitch_offset = 0.0;
                let is_warp = *transition == ScriptTransitionKind::WarpCut;
                let prev_snap = camera.prev_snapshot;

                let new_eye = match *target_eye_mm {
                    Some(tgt_eye) if is_warp => {
                        *eye_velocity_mm_per_s = Vec3::ZERO;
                        tgt_eye
                    }
                    Some(tgt_eye) => step_user_eye_pos(
                        prev_snap.eye,
                        tgt_eye,
                        eye_velocity_mm_per_s,
                        cam_prm,
                        dt,
                    ),
                    None => camera.curr_snapshot.eye,
                };
                let new_target = match *target_watch_mm {
                    Some(tgt_watch) if is_warp => tgt_watch,
                    Some(tgt_watch) => {
                        step_user_watch_target(new_eye, prev_snap.target, tgt_watch, watch_prm, dt)
                    }
                    None => camera.curr_snapshot.target,
                };

                let snap = CameraSnapshot {
                    eye: new_eye,
                    target: new_target,
                    up: rolled_ps2_up_vector(new_eye, new_target, *roll_radians),
                    fov_y: next_fov,
                };
                camera.ideal_eye = new_eye;
                camera.ideal_target = new_target;
                camera.curr_snapshot = snap;
                if is_warp {
                    camera.prev_snapshot = snap;
                    *transition = ScriptTransitionKind::SmoothBlend;
                    self.vc_flags &= !(VC_WARP_CAM_F | VC_WARP_WATCH_F);
                }
            }
            ScriptCameraMode::ReturningToAuto {
                from_snapshot,
                elapsed_ticks,
                total_ticks,
            } => {
                *elapsed_ticks = elapsed_ticks.saturating_add(1);
                let blended = smoothstep_camera_snapshot(
                    *from_snapshot,
                    camera.curr_snapshot,
                    *elapsed_ticks,
                    *total_ticks,
                );
                camera.ideal_eye = blended.eye;
                camera.ideal_target = blended.target;
                camera.curr_snapshot = blended;
                if *elapsed_ticks >= *total_ticks {
                    self.mode = ScriptCameraMode::AutoGameplay;
                }
            }
        }
        camera.raw_state.flags = self.vc_flags;
    }
}
