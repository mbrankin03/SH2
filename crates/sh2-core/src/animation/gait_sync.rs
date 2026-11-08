//! Pure locomotion-to-animation synchronization and Mode 10 gait phase preservation (`0x531a00`).
//!
//! # Ground Truth Reference
//! - PlayStation 2 Debug Prototype (`SLUS_202.28`, `Chacter/m3_play.c`) & PC Director's Cut (`sh2pc.exe`):
//!   - `0x54bd70` (`Walk`): `fps = base_fps * ty` where `ty = linear_speed / 750.0` (`1.5 m/s * 500`).
//!   - `0x54c0e0` (`Back`): `fps = base_fps * ty` where `ty = linear_speed / 600.0` (`1.2 m/s * 500`).
//!   - `0x54bbb0` (`TurnLeft` / `TurnRight`): spot-turn playback rate scales with angular velocity `tx`
//!     (and `1.5x` when run is held).
//!   - `0x531a00`: Selects animation clip from `_AnimeInfo` table `0x6ceac4` (`101..127`) and transition mode:
//!     - **Mode 10 (`32` frames = `1.0667s`)**: Preserves normalized stride phase across 24-frame walk,
//!       strafe, and run stages (`phase_kept`).
//!     - **Mode 4 (`8` frames = `0.2667s`)**: Standard pose crossfade from the current frame.
//!     - **Mode 2 (`0` frames = immediate cut)**: Used for 180° quick turns (`Clips 109/110`).

use crate::locomotion::{LocomotionOutput, LocomotionState, SH2_WORLD_UNIT_SCALE};

use super::clip::{
    AnimationClip, AnimationStateKind, BlendMode, MODE10_GAIT_SYNC_BLEND_DURATION_SECS,
    MODE4_POSE_BLEND_DURATION_SECS,
};
use super::tables::CharacterLocomotionClips;

/// Maximum forward walking linear velocity in world units/s (`1.5 m/s * 500 = 750.0 u/s`, `0x54bd70`).
pub const MAX_WALK_LINEAR_SPEED: f32 = 1.5 * SH2_WORLD_UNIT_SCALE;

/// Maximum backward step linear velocity in world units/s (`1.2 m/s * 500 = 600.0 u/s`, `0x54c0e0`).
pub const MAX_BACK_LINEAR_SPEED: f32 = 1.2 * SH2_WORLD_UNIT_SCALE;

/// Base angular turning velocity on the spot in radians/s (`2.4 rad/s`, `0x54bbb0`).
pub const BASE_SPOT_TURN_RAD_PER_SEC: f32 = 2.4;

/// Authentic PS2/PC forward walk fixed-point base speed (`1408` = `20.625 FPS`, `0x6ceac4[10]`).
pub const PS2_WALK_BASE_SPEED_FP: f32 = 1408.0;
/// Authentic PS2/PC forward walk analog tilt floor (`1408 - 896 = 512` = `7.5 FPS`, `0x54c186`):
/// `effective_speed = 1408 - (1.0 - ty) * 896 = 512 + 896 * ty`.
pub const PS2_WALK_MIN_SPEED_FP: f32 = 512.0;

/// Authentic PS2/PC backward walk fixed-point base speed (`1536` = `22.5 FPS`, `0x6ceac4[11]`).
pub const PS2_BACK_BASE_SPEED_FP: f32 = 1536.0;
/// Authentic PS2/PC backward walk analog tilt floor (`1536 - 1024 = 512` = `7.5 FPS`, `0x54c508`):
/// `effective_speed = 1536 - (1.0 - ty) * 1024 = 512 + 1024 * ty`.
pub const PS2_BACK_MIN_SPEED_FP: f32 = 512.0;

/// Authentic PS2/PC spot-turn fixed-point speed scale (`(1280 - 384) / 1280 = 896 / 1280 = 0.7`, `0x54bf90`).
pub const PS2_TURN_EFFECTIVE_SCALE: f32 = 896.0 / 1280.0;

/// Declarative animation synchronization command produced from a single locomotion tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocomotionAnimCommand {
    /// Target animation state kind.
    pub state: AnimationStateKind,
    /// Target animation clip from the character's locomotion table.
    pub clip: AnimationClip,
    /// Dynamic playback rate multiplier (`1.0` = clip's base FPS; scaled by stick tilt/speed for Walk/Back/Turn).
    pub playback_rate_scale: f32,
    /// Authentic PS2 transition blend mode (`Mode 2` Cut, `Mode 4` 8-frame pose fade, or `Mode 10` 32-frame gait sync).
    pub blend_mode: BlendMode,
}

/// Stateless resolver mapping `LocomotionOutput` and transition history to `LocomotionAnimCommand`.
pub struct LocomotionAnimationSync;

impl LocomotionAnimationSync {
    /// Returns `true` if transitioning from `from_state` to `to_state` preserves the 24-frame
    /// stride phase under PS2/PC `0x531a00` (`Mode 10` crossfade over 32 frames).
    ///
    /// Authentic PS2 `phase_kept` matrix (`0x531a00`):
    /// - `Walk (104)` keeps phase from `{Run1 (106), Run2 (107), Run3 (108), StrafeRightWalk (124), StrafeLeftWalk (125), StrafeRightRun (126), StrafeLeftRun (127)}`
    /// - `StrafeLeftWalk (125)` keeps phase from `{Walk (104), Run1 (106), Run2 (107), Run3 (108), StrafeLeftRun (127)}`
    /// - `StrafeRightWalk (124)` keeps phase from `{Walk (104), Run1 (106), Run2 (107), Run3 (108), StrafeRightRun (126)}`
    /// - `Run1 (106)` keeps phase from `{Walk (104), Run2 (107), Run3 (108), StrafeRightWalk (124), StrafeLeftWalk (125), StrafeRightRun (126), StrafeLeftRun (127)}`
    /// - `Run2 (107)` keeps phase from `{Walk (104), Run1 (106), Run3 (108), StrafeRightWalk (124), StrafeLeftWalk (125), StrafeRightRun (126), StrafeLeftRun (127)}`
    /// - `Run3 (108)` keeps phase from `{Walk (104), Run1 (106), Run2 (107), StrafeRightWalk (124), StrafeLeftWalk (125), StrafeRightRun (126), StrafeLeftRun (127)}`
    /// - `StrafeLeftRun (127)` keeps phase from `{Walk (104), Run1 (106), Run2 (107), Run3 (108), StrafeLeftWalk (125)}`
    /// - `StrafeRightRun (126)` keeps phase from `{Walk (104), Run1 (106), Run2 (107), Run3 (108), StrafeRightWalk (124)}`
    #[inline]
    pub const fn preserves_gait_phase(
        from_state: AnimationStateKind,
        to_state: AnimationStateKind,
    ) -> bool {
        use AnimationStateKind::*;
        match to_state {
            Walk => matches!(
                from_state,
                Run | Run2
                    | Run3
                    | StrafeRightWalk
                    | StrafeLeftWalk
                    | StrafeRightRun
                    | StrafeLeftRun
            ),
            StrafeLeftWalk => matches!(from_state, Walk | Run | Run2 | Run3 | StrafeLeftRun),
            StrafeRightWalk => matches!(from_state, Walk | Run | Run2 | Run3 | StrafeRightRun),
            Run => matches!(
                from_state,
                Walk | Run2
                    | Run3
                    | StrafeRightWalk
                    | StrafeLeftWalk
                    | StrafeRightRun
                    | StrafeLeftRun
            ),
            Run2 => matches!(
                from_state,
                Walk | Run
                    | Run3
                    | StrafeRightWalk
                    | StrafeLeftWalk
                    | StrafeRightRun
                    | StrafeLeftRun
            ),
            Run3 => matches!(
                from_state,
                Walk | Run
                    | Run2
                    | StrafeRightWalk
                    | StrafeLeftWalk
                    | StrafeRightRun
                    | StrafeLeftRun
            ),
            StrafeLeftRun => matches!(from_state, Walk | Run | Run2 | Run3 | StrafeLeftWalk),
            StrafeRightRun => matches!(from_state, Walk | Run | Run2 | Run3 | StrafeRightWalk),
            _ => false,
        }
    }

    /// Resolves the target animation state, clip, playback rate scale, and transition blend mode
    /// from the current `LocomotionOutput` and active `current_anim_state`.
    #[inline]
    pub fn resolve(
        current_anim_state: AnimationStateKind,
        locomotion: &LocomotionOutput,
        clips: &CharacterLocomotionClips,
    ) -> LocomotionAnimCommand {
        let (state, clip, playback_rate_scale) = match locomotion.state {
            LocomotionState::Stand => (AnimationStateKind::Idle, clips.idle, 1.0),
            LocomotionState::Relax => (AnimationStateKind::Relax, clips.relax, 1.0),
            LocomotionState::Alert => (AnimationStateKind::Alert, clips.alert, 1.0),
            LocomotionState::Tired => (AnimationStateKind::Tired, clips.tired, 1.0),
            LocomotionState::TurnLeft => {
                // Spot turn (0x54bf90): base speed 1280 + delta (-384 / 0xfffffe80) = 896 (13.125 fps),
                // scaled by 1.5x when run is held (yaw_rate > 2.4 rad/s).
                let run_boost =
                    if locomotion.turn_speed_rad_per_sec.abs() > BASE_SPOT_TURN_RAD_PER_SEC + 0.1 {
                        1.5
                    } else {
                        1.0
                    };
                (
                    AnimationStateKind::TurnLeft,
                    clips.turn_left,
                    PS2_TURN_EFFECTIVE_SCALE * run_boost,
                )
            }
            LocomotionState::TurnRight => {
                // Spot turn (0x54c070): base speed 1280 + delta (-384 / 0xfffffe80) = 896 (13.125 fps),
                // scaled by 1.5x when run is held (yaw_rate > 2.4 rad/s).
                let run_boost =
                    if locomotion.turn_speed_rad_per_sec.abs() > BASE_SPOT_TURN_RAD_PER_SEC + 0.1 {
                        1.5
                    } else {
                        1.0
                    };
                (
                    AnimationStateKind::TurnRight,
                    clips.turn_right,
                    PS2_TURN_EFFECTIVE_SCALE * run_boost,
                )
            }
            LocomotionState::Walk => {
                // Forward walk (0x54c186): base speed 1408 + (1.0 - ty) * (-896) = 512 + 896 * ty (7.5..20.625 fps)
                let ty = (locomotion.linear_speed / MAX_WALK_LINEAR_SPEED).clamp(0.0, 1.0);
                let rate_scale = (PS2_WALK_MIN_SPEED_FP
                    + (PS2_WALK_BASE_SPEED_FP - PS2_WALK_MIN_SPEED_FP) * ty)
                    / PS2_WALK_BASE_SPEED_FP;
                (AnimationStateKind::Walk, clips.walk, rate_scale)
            }
            LocomotionState::Back => {
                // Backward walk (0x54c508): base speed 1536 + (1.0 - ty) * (-1024) = 512 + 1024 * ty (7.5..22.5 fps)
                let ty = (locomotion.linear_speed / MAX_BACK_LINEAR_SPEED).clamp(0.0, 1.0);
                let rate_scale = (PS2_BACK_MIN_SPEED_FP
                    + (PS2_BACK_BASE_SPEED_FP - PS2_BACK_MIN_SPEED_FP) * ty)
                    / PS2_BACK_BASE_SPEED_FP;
                (AnimationStateKind::Back, clips.back, rate_scale)
            }
            LocomotionState::StrafeLeftWalk => (
                AnimationStateKind::StrafeLeftWalk,
                clips.strafe_left_walk,
                1.0,
            ),
            LocomotionState::StrafeRightWalk => (
                AnimationStateKind::StrafeRightWalk,
                clips.strafe_right_walk,
                1.0,
            ),
            LocomotionState::Run1 => {
                // Run 1 (Clip 106): speed 1536 (22.5 fps)
                (AnimationStateKind::Run, clips.run, 1.0)
            }
            LocomotionState::Run2 => {
                // Run 2 (Clip 107): speed 1792 (26.25 fps)
                (AnimationStateKind::Run2, clips.run2, 1.0)
            }
            LocomotionState::Run3 => {
                // Run 3 (Clip 108): speed 2048 (30.0 fps)
                (AnimationStateKind::Run3, clips.run3, 1.0)
            }
            LocomotionState::StrafeLeftRun => (
                AnimationStateKind::StrafeLeftRun,
                clips.strafe_left_run,
                1.0,
            ),
            LocomotionState::StrafeRightRun => (
                AnimationStateKind::StrafeRightRun,
                clips.strafe_right_run,
                1.0,
            ),
            LocomotionState::QuickTurnLeft => (
                AnimationStateKind::QuickTurnLeft,
                clips.quick_turn_left,
                1.0,
            ),
            LocomotionState::QuickTurnRight => (
                AnimationStateKind::QuickTurnRight,
                clips.quick_turn_right,
                1.0,
            ),
        };

        let blend_mode = if matches!(
            state,
            AnimationStateKind::QuickTurnLeft | AnimationStateKind::QuickTurnRight
        ) {
            // Mode 2: Quick 180° turns cut in immediately without crossfade
            BlendMode::Cut
        } else if Self::preserves_gait_phase(current_anim_state, state) {
            // Mode 10: 32-frame (1.0667s) gait-synchronized crossfade keeping stride phase
            BlendMode::GaitPhaseSync {
                duration_secs: MODE10_GAIT_SYNC_BLEND_DURATION_SECS,
            }
        } else {
            // Mode 4: 8-frame (0.2667s) standard pose crossfade
            BlendMode::PoseCrossfade {
                duration_secs: MODE4_POSE_BLEND_DURATION_SECS,
            }
        };

        LocomotionAnimCommand {
            state,
            clip,
            playback_rate_scale,
            blend_mode,
        }
    }
}
