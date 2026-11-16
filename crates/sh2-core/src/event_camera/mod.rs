//! Scripted event camera triggers, enemy encounter `.cam` framing, and cutscene/FMV camera
//! handoffs (`SH2-52`).
//!
//! Unifies three PS2 `SLUS_202.28` / Retail PC `sh2pc.exe` subsystems without mutating the core
//! rail solver:
//! 1. [`enemy_framing`]: `VC_RD_USE_NEAR_ENEMY_F` (`0x0040`) & `VC_RD_USE_NO_ENEMY_F` (`0x0020`)
//!    filtering (`vcSetNearestEnemyDataInVC_WORK` at `0x00196f20`, `vcSetNearRoadAryByCharaPos`
//!    at `0x00197310`) and dynamic stage-event road priority (`vcRetRoadUsePriority` at `0x00197a10`).
//! 2. [`script_override`]: `vcSetEventCamParamRefView` (`0x0019db10` / `vcSetCameraScript`),
//!    `vcUserCamTarget` (`0x00195ca0`), `vcUserWatchTarget` (`0x00195bd0`), `vcReturnPreAutoCamWork`
//!    (`0x00195ea0`), `vcSetProjectionValue` (`0x0019db80`), and `.mes` dialogue camera locks.
//! 3. [`cutscene_handoff`]: Seamless warp-safe transitions across gameplay `.cam` volumes,
//!    real-time `.dds` cutscenes (`DramaDemoPlayer`), hybrid chains, and Bink/MPEG FMV playback.

pub mod cutscene_handoff;
pub mod enemy_framing;
pub mod enemy_road_eval;
pub mod handoff_types;
pub mod script_kinematics;
pub mod script_override;

#[cfg(test)]
mod tests_enemy_script;
#[cfg(test)]
mod tests_handoff;

pub use cutscene_handoff::{
    CutsceneCameraHandoff, CutsceneHandoffPhase, DemoCameraTransitionKind, DemoFrameSyncState,
    FmvAdvanceSignal, FmvPlaybackState, HandoffStepOutcome, PostSequenceRestorePolicy,
    SavedGameplayCameraState,
};
pub use enemy_framing::{
    effective_road_priority, evaluate_nearest_enemy, is_road_eligible_for_enemy,
    old_road_advantage_mm, select_best_road_index, EnemyCameraAlertState, EnemyCameraTarget,
    EnemyEncounterFraming, EnemyLifeCameraState, EnemyProximityState, PlayerCameraSuppression,
    SelfViewGate, StageEventRoadGate, VC_DEAD_ENEMY_GRACE_SECS, VC_DEAD_ENEMY_TIMER_MAX_SECS,
    VC_EVENT_EFFECT_ADVANTAGE_MM, VC_EXTRA_BOUNDARY_WIDTH_MM, VC_MAX_CAMERA_TARGETS,
    VC_NEAR_ENEMY_MAX_DIST_MM,
};
pub use script_override::{
    fov_y_from_ps2_projection, ps2_projection_from_fov_y, rolled_ps2_up_vector,
    ProjectionChangeTiming, ProjectionTransitionState, ScriptCameraController, ScriptCameraMode,
    ScriptTransitionKind, ScriptedCameraShot, BOWLING_ALLEY_EDDIE_CAM_1, BOWLING_ALLEY_EDDIE_CAM_2,
    BOWLING_ALLEY_ENTRANCE_CAM, DEFAULT_USER_CAM_MV_PARAM, DEFAULT_USER_WATCH_MV_PARAM,
    FOREST_CEMETERY_ANGELA_CAM, PS2_DEFAULT_PROJECTION_SCR_Z, VC_ALL_WARP_FLAGS,
    VC_INHIBIT_FAR_WATCH_F, VC_PROJ_MOMENT_CHANGE_F, VC_USER_CAM_F, VC_USER_MODE_FLAGS,
    VC_USER_WATCH_F, VC_WARP_CAM_F, VC_WARP_CAM_TGT_F, VC_WARP_WATCH_F,
};

use crate::camera::Ps2CameraController;
use crate::cutscene::DramaDemoPlayer;
use crate::events::CameraOverrideCommand;
use glam::Vec3;

/// Active camera authority tier resolved on each 60 Hz simulation tick (`vcMoveAndSetCamera` / `EventMain`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCameraAuthority {
    /// Automatic `.cam` rail & enemy-encounter volume solver (`vcExecCamera` `0x00195f90`).
    GameplayRail,
    /// Scripted stage camera or `.mes` dialogue camera override (`vcSetEventCamParamRefView` `0x0019db10`).
    ScriptOverride,
    /// Real-time `.dds` cutscene, FMV movie, or post-cutscene blend (`DramaDemoMain` / `MovieMain`).
    CutsceneOrFmv,
}

/// Unified 60 Hz coordinator orchestrating enemy-proximity `.cam` selection, scripted event /
/// `.mes` dialogue overrides, and `.dds` cutscene / FMV camera handoffs (`SH2-52`).
#[derive(Debug, Clone, Default)]
pub struct EventCameraSystem {
    /// Enemy-proximity `.cam` road selector (`VC_RD_USE_NEAR_ENEMY_F` / `VC_RD_USE_NO_ENEMY_F`).
    pub enemy_framing: EnemyEncounterFraming,
    /// Scripted event & `.mes` dialogue camera controller (`vcSetEventCamParamRefView`).
    pub script_camera: ScriptCameraController,
    /// `.dds` cutscene & FMV handoff state machine (`DramaDemoPlayer` / `MovieMain`).
    pub cutscene_handoff: CutsceneCameraHandoff,
}

impl EventCameraSystem {
    /// Creates a new unified event camera coordinator in default gameplay state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the highest-priority subsystem currently driving the camera.
    #[inline]
    pub const fn active_authority(&self) -> EventCameraAuthority {
        if self.cutscene_handoff.is_active() {
            EventCameraAuthority::CutsceneOrFmv
        } else if self.script_camera.is_overriding() {
            EventCameraAuthority::ScriptOverride
        } else {
            EventCameraAuthority::GameplayRail
        }
    }

    /// Forwards a stage `CameraOverrideCommand` (`StageHookOutput::camera_override`) to the
    /// scripted camera subsystem (`sh2pc.exe` `0x0051d280` / `0x0051d520`).
    pub fn apply_stage_override(
        &mut self,
        cmd: CameraOverrideCommand,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        self.script_camera
            .apply_override_command(cmd, camera, player_pos, player_yaw);
    }

    /// Steps one deterministic 60 Hz tick (`dt_secs = 1.0 / 60.0` s) across cutscene handoffs,
    /// enemy-aware `.cam` volume selection, and scripted/dialogue camera overrides.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        right_stick_x: f32,
        right_stick_y: f32,
        dt_secs: f32,
        player_suppression: PlayerCameraSuppression,
        enemies: &[EnemyCameraTarget],
        active_demo: Option<&DramaDemoPlayer>,
    ) -> HandoffStepOutcome {
        match self.cutscene_handoff.phase {
            CutsceneHandoffPhase::InEngineDemo { .. } => {
                if let Some(demo) = active_demo {
                    return self.cutscene_handoff.sync_drama_demo_frame(
                        camera,
                        demo,
                        Some(&mut self.enemy_framing),
                        enemies,
                    );
                }
            }
            CutsceneHandoffPhase::FmvPlayback(_) => {
                return self.cutscene_handoff.tick_fmv(
                    camera,
                    dt_secs,
                    FmvAdvanceSignal::Play,
                    None,
                    Some(&mut self.enemy_framing),
                    enemies,
                );
            }
            CutsceneHandoffPhase::VoiceOnlySegment {
                stream_no,
                msg_start,
                ..
            } => {
                camera.prev_snapshot = camera.curr_snapshot;
                return HandoffStepOutcome::RunningVoiceOnly {
                    stream_no,
                    msg_start,
                };
            }
            CutsceneHandoffPhase::BlendingToGameplay { .. } => {
                return self.cutscene_handoff.tick_blend_out(camera);
            }
            CutsceneHandoffPhase::Gameplay => {}
        }

        // Advance underlying `.cam` road evaluation with enemy-proximity filtering (`0x0020` / `0x0040`),
        // then layer any active scripted shot, `.mes` dialogue lock, or projection transition on top.
        self.enemy_framing.tick_camera_with_enemies(
            camera,
            player_pos,
            player_yaw,
            right_stick_x,
            right_stick_y,
            player_suppression,
            enemies,
        );
        self.script_camera.apply_tick(camera, dt_secs);
        HandoffStepOutcome::GameplayActive
    }
}
