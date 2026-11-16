//! Typed state machines and snapshots for `.dds` cutscene and FMV camera handoffs (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` references:
//! - `shCharacterSetPosAfterDemo` (`0x0011f770`)
//! - `vcReturnPreAutoCamWork` (`0x00195ea0`)
//! - `EvSubMovieReady` (`0x001ad680`) .. `EvSubMovieEnd` (`0x001ad830`), `MovieMain` (`0x002877b0`)
//! - `DdsPlay` (`0x001b1980`) & `DramaDemoSkipLast` (`0x001b2f10`)

use crate::camera::{CameraMovementType, CameraSnapshot, CameraSourceKind, Ps2CameraController};
use crate::cutscene::CutsceneSequenceStep;
use crate::types::camera::CameraState;
use glam::Vec3;

/// Saved pre-sequence gameplay camera & player orientation state captured prior to entering a
/// `.dds` cutscene or FMV movie.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedGameplayCameraState {
    /// Active room identifier (`"er01"`, `"ca11"`, `"bw01"`, etc.) at capture time.
    pub room_id: Option<String>,
    /// Active `.cam` source pack (`Sh2eeOverride` vs `VanillaRetail`).
    pub cam_source: Option<CameraSourceKind>,
    /// Active `.cam` road volume index at capture time.
    pub active_road_index: Option<usize>,
    /// Active camera movement algorithm (`Chase`, `Settle`, `FixedAngle`, etc.).
    pub mv_type: CameraMovementType,
    /// Captured gameplay camera transform snapshot (`eye`, `target`, `up`, `fov_y`).
    pub snapshot: CameraSnapshot,
    /// Captured smoothing accumulator eye position in PS2 millimeters.
    pub ideal_eye: Vec3,
    /// Captured smoothing accumulator target position in PS2 millimeters.
    pub ideal_target: Vec3,
    /// Player world position in PS2 millimeters (Y-down) prior to cutscene/FMV entry.
    pub player_pos: Vec3,
    /// Player yaw orientation in radians prior to cutscene/FMV entry.
    pub player_yaw: f32,
    /// Low-level 416-byte `_VC_WORK` workspace snapshot.
    pub raw_state: CameraState,
}

impl SavedGameplayCameraState {
    /// Captures the complete gameplay camera state from `camera` and player `(player_pos, player_yaw)`.
    pub fn capture(camera: &Ps2CameraController, player_pos: Vec3, player_yaw: f32) -> Self {
        Self {
            room_id: camera.active_room_id.clone(),
            cam_source: camera.active_cam_source,
            active_road_index: camera.active_road_index,
            mv_type: camera.mv_type,
            snapshot: camera.curr_snapshot,
            ideal_eye: camera.ideal_eye,
            ideal_target: camera.ideal_target,
            player_pos,
            player_yaw,
            raw_state: camera.raw_state,
        }
    }

    /// Restores the exact saved camera snapshot and `_VC_WORK` state onto `camera` warp-safely.
    pub fn restore_onto(&self, camera: &mut Ps2CameraController) {
        camera.active_road_index = self.active_road_index;
        camera.mv_type = self.mv_type;
        camera.ideal_eye = self.ideal_eye;
        camera.ideal_target = self.ideal_target;
        camera.curr_snapshot = self.snapshot;
        camera.prev_snapshot = self.snapshot;
        camera.raw_state = self.raw_state;
        camera.search_yaw_offset = 0.0;
        camera.search_pitch_offset = 0.0;
    }
}

/// Smoothstep-interpolates two [`CameraSnapshot`] values at `(elapsed_ticks / total_ticks)`.
pub(crate) fn smoothstep_camera_snapshot(
    from: CameraSnapshot,
    to: CameraSnapshot,
    elapsed_ticks: u16,
    total_ticks: u16,
) -> CameraSnapshot {
    let t = (f32::from(elapsed_ticks) / f32::from(total_ticks.max(1))).clamp(0.0, 1.0);
    let s = t * t * (3.0 - 2.0 * t);
    CameraSnapshot {
        eye: from.eye.lerp(to.eye, s),
        target: from.target.lerp(to.target, s),
        up: from
            .up
            .lerp(to.up, s)
            .normalize_or(Vec3::new(0.0, -1.0, 0.0)),
        fov_y: from.fov_y + (to.fov_y - from.fov_y) * s,
    }
}

/// Policy governing how gameplay `.cam` framing is restored when a `.dds` cutscene or FMV ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PostSequenceRestorePolicy {
    /// Re-evaluates the room `.cam` volume at the final player handoff `(pos, yaw)` and warp-snaps
    /// both `prev_snapshot` and `curr_snapshot` (`shCharacterSetPosAfterDemo` `0x0011f770` +
    /// `vcReturnPreAutoCamWork(1)` `0x00195ea0`).
    #[default]
    ReevaluateAtPlayerPose,
    /// Restores the exact saved pre-sequence `CameraSnapshot`, `active_road_index`, `mv_type`,
    /// and `_VC_WORK` state with `prev_snapshot == curr_snapshot` (ideal for standalone flashbacks
    /// or FMVs where player coordinates do not change).
    RestoreExactSavedSnapshot,
    /// Re-evaluates the target gameplay `.cam` pose and smoothly blends from the final cutscene
    /// camera over `blend_ticks` 60 Hz simulation ticks.
    SmoothBlendToGameplay {
        /// Duration of the cross-blend in 60 Hz ticks.
        blend_ticks: u16,
    },
}

/// Frame-entry state for an active `.dds` cutscene (`DdsPlay` at `0x001b1df4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoFrameSyncState {
    /// First `.dds` frame has not yet been committed; forces `prev_snapshot = curr_snapshot`
    /// so sub-frame interpolation never smears from the pre-cutscene gameplay camera.
    AwaitingFirstFrame,
    /// Subsequent `.dds` frames are streaming; hard cuts (`Key 0x10`) snap `prev_snapshot`, while
    /// continuous camera tracks preserve smooth `prev_snapshot -> curr_snapshot` interpolation.
    Streaming,
}

/// Advance control signal for FMV movie ticks (`MovieMain` / `pssCheckMovieCancel` at `0x0028bbd0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmvAdvanceSignal {
    /// Continue normal playback by `dt_secs`.
    Play,
    /// User pressed Start/Esc to skip FMV immediately (`pssGetPssAbortFlag` at `0x0028bba0`).
    Skip,
}

/// Active FMV playback state (`EvSubMovieReady` `0x001ad680` .. `EvSubMovieEnd` `0x001ad830`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FmvPlaybackState {
    /// Canonical FMV movie identifier (e.g. `"deai"`, `"murder"`, `"hakaba"`, `"end"`).
    pub movie_id: &'static str,
    /// Elapsed playback time in seconds.
    pub elapsed_secs: f32,
    /// Total movie duration in seconds.
    pub duration_secs: f32,
}

/// Explicit lifecycle state machine for gameplay <-> `.dds` cutscene <-> FMV camera handoffs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CutsceneHandoffPhase {
    /// Normal gameplay `.cam` framing owns the camera.
    #[default]
    Gameplay,
    /// Real-time 3D `.dds` cutscene (`DramaDemoPlayer`) owns the camera (`DdsPlay` `0x001b1980`).
    InEngineDemo {
        /// Active `DramaDemoPlayInfo::demo_no` (`1..=130`).
        demo_no: u16,
        /// First-frame warp synchronization guard.
        sync_state: DemoFrameSyncState,
    },
    /// Pre-rendered Bink/MPEG FMV playback owns the viewport (`MovieMain` `0x002877b0`).
    FmvPlayback(FmvPlaybackState),
    /// Voice-only `.mes` narration sequence without `.dds` motion (`SubtitlesExec` `0x001b34d0`).
    VoiceOnlySegment {
        /// `voice.afs` stream ID.
        stream_no: i32,
        /// `.mes` file relative path.
        mes_path: &'static str,
        /// Starting message index.
        msg_start: u16,
    },
    /// Smoothly blending from the final cutscene frame back into gameplay `.cam` framing.
    BlendingToGameplay {
        /// Final cutscene camera snapshot at handoff start.
        from_snapshot: CameraSnapshot,
        /// Target gameplay `.cam` snapshot at player handoff pose.
        target_snapshot: CameraSnapshot,
        /// Elapsed 60 Hz ticks (`0..total_ticks`).
        elapsed_ticks: u16,
        /// Total 60 Hz blend ticks.
        total_ticks: u16,
    },
}

/// Frame-to-frame camera transition kind on an active `.dds` cutscene step (`Key 0x10` in `DdsPlay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoCameraTransitionKind {
    /// Entry frame or `Key 0x10` hard cut (`prev_snapshot` snapped to `curr_snapshot`).
    HardCutSnap,
    /// Continuous keyframe track (`prev_snapshot` preserved for sub-frame interpolation).
    InterpolatedFrame,
}

/// Result returned by each 60 Hz step of [`super::cutscene_handoff::CutsceneCameraHandoff`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HandoffStepOutcome {
    /// No cutscene or FMV is active.
    GameplayActive,
    /// Real-time `.dds` cutscene camera was synchronized onto `Ps2CameraController`.
    RunningDemo {
        /// Active `demo_no`.
        demo_no: u16,
        /// Cutscene frame transition kind (`HardCutSnap` vs `InterpolatedFrame`).
        transition: DemoCameraTransitionKind,
    },
    /// FMV movie is actively playing while the 3D camera remains cleanly parked.
    RunningFmv {
        /// Active FMV identifier.
        movie_id: &'static str,
    },
    /// Voice-only narration step is active.
    RunningVoiceOnly {
        /// Active `voice.afs` stream ID.
        stream_no: i32,
        /// Starting `.mes` message ID.
        msg_start: u16,
    },
    /// Hybrid sequence chain advanced to `next_step` (e.g. `demo 21 -> FMV "deai" -> demo 22`).
    TransitionedChainStep {
        /// Newly activated step in `DramaDemoPlayInfo::sequence_chain`.
        next_step: CutsceneSequenceStep,
    },
    /// Post-cutscene camera is smoothly blending back into gameplay `.cam` framing.
    BlendingOut {
        /// Remaining 60 Hz ticks before `Gameplay` resumes.
        remaining_ticks: u16,
    },
    /// Sequence finished and gameplay camera + player pose `(pos_mm, yaw_rad)` were restored.
    RestoredToGameplay {
        /// Final player world position in PS2 millimeters (Y-down).
        player_pos: Vec3,
        /// Final player yaw angle in radians.
        player_yaw: f32,
    },
}
