//! Seamless camera handoff coordinator across gameplay `.cam` volumes, real-time `.dds`
//! cutscenes (`DramaDemoPlayer`), hybrid cutscene chains, and Bink/MPEG FMV playback (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` references:
//! - `SCNowDemoEventSwitch` (`0x0011e730`): toggles `0x2000` demo mode flag on player
//! - `shCharacterSetPosAfterDemo` (`0x0011f770`): commits final `.dds` actor pose (`pos` & `rot_y`)
//! - `vcReturnPreAutoCamWork` (`0x00195ea0`) & `vcSetCameraUseWarp` (`0x0019de50`)
//! - `EvSubMovieReady` (`0x001ad680`) .. `EvSubMovieEnd` (`0x001ad830`), `MovieMain` (`0x002877b0`)
//! - `DramaDemoMain` (`0x001b0f00`), `DdsPlay` (`0x001b1980`), `DramaDemoSkipLast` (`0x001b2f10`)

pub use super::handoff_types::{
    CutsceneHandoffPhase, DemoCameraTransitionKind, DemoFrameSyncState, FmvAdvanceSignal,
    FmvPlaybackState, HandoffStepOutcome, PostSequenceRestorePolicy, SavedGameplayCameraState,
};

use super::enemy_framing::{EnemyCameraTarget, EnemyEncounterFraming, PlayerCameraSuppression};
use super::handoff_types::smoothstep_camera_snapshot;
use super::script_override::rolled_ps2_up_vector;
use crate::camera::{CameraSnapshot, Ps2CameraController};
use crate::cutscene::{CutsceneSequenceStep, DramaDemoPlayer, DramaDemoStep};
use glam::Vec3;

/// Coordinator managing seamless camera handoffs across `.cam` gameplay, `.dds` cutscenes,
/// hybrid sequence chains (`SEQ_DEAI`, `SEQ_VTR`, `SEQ_BP_EDI`), and FMV playback.
#[derive(Debug, Clone, Default)]
pub struct CutsceneCameraHandoff {
    /// Active lifecycle phase.
    pub phase: CutsceneHandoffPhase,
    /// Saved pre-sequence gameplay camera & player pose.
    pub saved_gameplay: Option<SavedGameplayCameraState>,
    /// Restoration policy applied when the sequence completes.
    pub restore_policy: PostSequenceRestorePolicy,
    /// Optional hybrid sequence chain (`DramaDemoPlayInfo::sequence_chain`).
    sequence_chain: &'static [CutsceneSequenceStep],
    /// Current 0-based cursor inside `sequence_chain`.
    chain_cursor: usize,
    /// Latest player handoff `(pos_mm, yaw_rad)` extracted from `.dds` actors (`shCharacterSetPosAfterDemo`).
    pending_player_handoff: Option<(Vec3, f32)>,
}

impl CutsceneCameraHandoff {
    /// Creates an idle handoff coordinator in `CutsceneHandoffPhase::Gameplay`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when gameplay locomotion/camera rails are suspended by a cutscene, FMV, or blend.
    #[inline]
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, CutsceneHandoffPhase::Gameplay)
    }

    /// Captures gameplay camera state and begins a real-time 3D `.dds` cutscene (`DramaDemoInit` `0x001b1130`),
    /// immediately syncing Frame 0 onto `camera` with warp-safe `prev_snapshot == curr_snapshot`.
    pub fn begin_drama_demo(
        &mut self,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        demo: &DramaDemoPlayer,
        restore_policy: PostSequenceRestorePolicy,
    ) {
        self.saved_gameplay = Some(SavedGameplayCameraState::capture(
            camera, player_pos, player_yaw,
        ));
        self.restore_policy = restore_policy;
        self.sequence_chain = demo.info.sequence_chain;
        self.chain_cursor = self
            .sequence_chain
            .iter()
            .position(|step| {
                matches!(
                    step,
                    CutsceneSequenceStep::InEngineDemo { demo_no } if *demo_no == demo.info.demo_no
                )
            })
            .unwrap_or(0);
        self.pending_player_handoff = None;
        self.phase = CutsceneHandoffPhase::InEngineDemo {
            demo_no: demo.info.demo_no,
            sync_state: DemoFrameSyncState::AwaitingFirstFrame,
        };
        let _ = self.sync_drama_demo_frame(camera, demo, None, &[]);
    }

    /// Captures gameplay camera state and begins standalone Bink/MPEG FMV playback
    /// (`EvSubMovieReady` `0x001ad680` -> `EvSubMovieStart` `0x001ad780`).
    pub fn begin_fmv(
        &mut self,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        movie_id: &'static str,
        duration_secs: f32,
        restore_policy: PostSequenceRestorePolicy,
    ) {
        self.saved_gameplay = Some(SavedGameplayCameraState::capture(
            camera, player_pos, player_yaw,
        ));
        self.restore_policy = restore_policy;
        self.sequence_chain = &[];
        self.chain_cursor = 0;
        self.pending_player_handoff = None;
        camera.search_yaw_offset = 0.0;
        camera.search_pitch_offset = 0.0;
        camera.prev_snapshot = camera.curr_snapshot;
        self.phase = CutsceneHandoffPhase::FmvPlayback(FmvPlaybackState {
            movie_id,
            elapsed_secs: 0.0,
            duration_secs: duration_secs.max(0.0),
        });
    }

    /// Supplies the newly loaded [`DramaDemoPlayer`] when a hybrid sequence chain transitions
    /// from an FMV step (e.g. `"deai"` or `"murder"`) into a follow-up `.dds` demo (`demo_no == 22` or `82`).
    pub fn attach_chained_demo(
        &mut self,
        camera: &mut Ps2CameraController,
        demo: &DramaDemoPlayer,
    ) {
        self.phase = CutsceneHandoffPhase::InEngineDemo {
            demo_no: demo.info.demo_no,
            sync_state: DemoFrameSyncState::AwaitingFirstFrame,
        };
        let _ = self.sync_drama_demo_frame(camera, demo, None, &[]);
    }

    /// Synchronizes `camera` with the current frame of `demo` (`DdsPlay` at `0x001b1df4..0x001b1e54`),
    /// handling roll-rotated up vectors, `Key 0x10` hard-cut warp safety, and completion handoff.
    pub fn sync_drama_demo_frame(
        &mut self,
        camera: &mut Ps2CameraController,
        demo: &DramaDemoPlayer,
        enemy_framing: Option<&mut EnemyEncounterFraming>,
        enemies: &[EnemyCameraTarget],
    ) -> HandoffStepOutcome {
        let CutsceneHandoffPhase::InEngineDemo {
            demo_no,
            sync_state,
        } = self.phase
        else {
            return HandoffStepOutcome::GameplayActive;
        };

        if demo.step == DramaDemoStep::Finished {
            if let Some(pose) = demo.player_handoff_transform() {
                self.pending_player_handoff = Some(pose);
            }
            return self.advance_chain_or_restore(camera, None, enemy_framing, enemies);
        }

        let cs = demo.camera;
        let next_snap = CameraSnapshot {
            eye: cs.eye,
            target: cs.interest,
            up: rolled_ps2_up_vector(cs.eye, cs.interest, cs.roll_radians),
            fov_y: cs.fov_y_radians,
        };

        let transition = if sync_state == DemoFrameSyncState::AwaitingFirstFrame || cs.is_hard_cut {
            DemoCameraTransitionKind::HardCutSnap
        } else {
            DemoCameraTransitionKind::InterpolatedFrame
        };
        camera.prev_snapshot = match transition {
            DemoCameraTransitionKind::HardCutSnap => next_snap,
            DemoCameraTransitionKind::InterpolatedFrame => camera.curr_snapshot,
        };
        camera.curr_snapshot = next_snap;
        camera.ideal_eye = cs.eye;
        camera.ideal_target = cs.interest;
        camera.search_yaw_offset = 0.0;
        camera.search_pitch_offset = 0.0;

        self.phase = CutsceneHandoffPhase::InEngineDemo {
            demo_no,
            sync_state: DemoFrameSyncState::Streaming,
        };
        HandoffStepOutcome::RunningDemo {
            demo_no,
            transition,
        }
    }

    /// Advances active FMV playback by `dt_secs` seconds (`MovieMain` `0x002877b0` -> `EvSubMovieEnd` `0x001ad830`).
    pub fn tick_fmv(
        &mut self,
        camera: &mut Ps2CameraController,
        dt_secs: f32,
        signal: FmvAdvanceSignal,
        post_fmv_pose_override: Option<(Vec3, f32)>,
        enemy_framing: Option<&mut EnemyEncounterFraming>,
        enemies: &[EnemyCameraTarget],
    ) -> HandoffStepOutcome {
        let CutsceneHandoffPhase::FmvPlayback(mut fmv) = self.phase else {
            return HandoffStepOutcome::GameplayActive;
        };
        camera.search_yaw_offset = 0.0;
        camera.search_pitch_offset = 0.0;
        camera.prev_snapshot = camera.curr_snapshot;

        fmv.elapsed_secs += dt_secs.max(0.0);
        if signal == FmvAdvanceSignal::Skip || fmv.elapsed_secs >= fmv.duration_secs {
            return self.advance_chain_or_restore(
                camera,
                post_fmv_pose_override,
                enemy_framing,
                enemies,
            );
        }
        self.phase = CutsceneHandoffPhase::FmvPlayback(fmv);
        HandoffStepOutcome::RunningFmv {
            movie_id: fmv.movie_id,
        }
    }

    /// Finishes an active `VoiceOnlySegment` step and advances the sequence chain or restores gameplay.
    pub fn finish_voice_only_segment(
        &mut self,
        camera: &mut Ps2CameraController,
        enemy_framing: Option<&mut EnemyEncounterFraming>,
        enemies: &[EnemyCameraTarget],
    ) -> HandoffStepOutcome {
        if !matches!(self.phase, CutsceneHandoffPhase::VoiceOnlySegment { .. }) {
            return HandoffStepOutcome::GameplayActive;
        }
        self.advance_chain_or_restore(camera, None, enemy_framing, enemies)
    }

    /// Steps an active `BlendingToGameplay` transition by one 60 Hz tick.
    pub fn tick_blend_out(&mut self, camera: &mut Ps2CameraController) -> HandoffStepOutcome {
        let CutsceneHandoffPhase::BlendingToGameplay {
            from_snapshot,
            target_snapshot,
            mut elapsed_ticks,
            total_ticks,
        } = self.phase
        else {
            return HandoffStepOutcome::GameplayActive;
        };

        camera.prev_snapshot = camera.curr_snapshot;
        elapsed_ticks = elapsed_ticks.saturating_add(1);
        let snap =
            smoothstep_camera_snapshot(from_snapshot, target_snapshot, elapsed_ticks, total_ticks);
        camera.curr_snapshot = snap;
        camera.ideal_eye = snap.eye;
        camera.ideal_target = snap.target;

        if elapsed_ticks >= total_ticks {
            let (player_pos, player_yaw) = self.resolved_handoff_pose(None);
            self.finish_to_gameplay(player_pos, player_yaw)
        } else {
            self.phase = CutsceneHandoffPhase::BlendingToGameplay {
                from_snapshot,
                target_snapshot,
                elapsed_ticks,
                total_ticks,
            };
            HandoffStepOutcome::BlendingOut {
                remaining_ticks: total_ticks - elapsed_ticks,
            }
        }
    }

    fn advance_chain_or_restore(
        &mut self,
        camera: &mut Ps2CameraController,
        pose_override: Option<(Vec3, f32)>,
        enemy_framing: Option<&mut EnemyEncounterFraming>,
        enemies: &[EnemyCameraTarget],
    ) -> HandoffStepOutcome {
        if let Some(pose) = pose_override {
            self.pending_player_handoff = Some(pose);
        }
        let next_idx = self.chain_cursor.saturating_add(1);
        if let Some(&next_step) = self.sequence_chain.get(next_idx) {
            self.chain_cursor = next_idx;
            self.phase = match next_step {
                CutsceneSequenceStep::InEngineDemo { demo_no } => {
                    CutsceneHandoffPhase::InEngineDemo {
                        demo_no,
                        sync_state: DemoFrameSyncState::AwaitingFirstFrame,
                    }
                }
                CutsceneSequenceStep::FmvMovie { movie_id } => {
                    camera.prev_snapshot = camera.curr_snapshot;
                    CutsceneHandoffPhase::FmvPlayback(FmvPlaybackState {
                        movie_id,
                        elapsed_secs: 0.0,
                        duration_secs: 30.0,
                    })
                }
                CutsceneSequenceStep::VoiceOnly {
                    stream_no,
                    mes_path,
                    msg_start,
                } => CutsceneHandoffPhase::VoiceOnlySegment {
                    stream_no,
                    mes_path,
                    msg_start,
                },
            };
            return HandoffStepOutcome::TransitionedChainStep { next_step };
        }

        let (player_pos, player_yaw) = self.resolved_handoff_pose(pose_override);
        let cutscene_end_snap = camera.curr_snapshot;

        match self.restore_policy {
            PostSequenceRestorePolicy::RestoreExactSavedSnapshot => {
                if let Some(ref saved) = self.saved_gameplay {
                    saved.restore_onto(camera);
                } else {
                    Self::reset_at_pose(camera, player_pos, player_yaw, enemy_framing, enemies);
                }
                self.finish_to_gameplay(player_pos, player_yaw)
            }
            PostSequenceRestorePolicy::ReevaluateAtPlayerPose => {
                if let Some(ref saved) = self.saved_gameplay {
                    camera.raw_state = saved.raw_state;
                }
                Self::reset_at_pose(camera, player_pos, player_yaw, enemy_framing, enemies);
                self.finish_to_gameplay(player_pos, player_yaw)
            }
            PostSequenceRestorePolicy::SmoothBlendToGameplay { blend_ticks }
                if blend_ticks >= 2 =>
            {
                if let Some(ref saved) = self.saved_gameplay {
                    camera.raw_state = saved.raw_state;
                }
                Self::reset_at_pose(camera, player_pos, player_yaw, enemy_framing, enemies);
                let target_snapshot = camera.curr_snapshot;
                camera.curr_snapshot = cutscene_end_snap;
                camera.prev_snapshot = cutscene_end_snap;
                camera.ideal_eye = cutscene_end_snap.eye;
                camera.ideal_target = cutscene_end_snap.target;
                self.phase = CutsceneHandoffPhase::BlendingToGameplay {
                    from_snapshot: cutscene_end_snap,
                    target_snapshot,
                    elapsed_ticks: 0,
                    total_ticks: blend_ticks,
                };
                HandoffStepOutcome::BlendingOut {
                    remaining_ticks: blend_ticks,
                }
            }
            PostSequenceRestorePolicy::SmoothBlendToGameplay { .. } => {
                Self::reset_at_pose(camera, player_pos, player_yaw, enemy_framing, enemies);
                self.finish_to_gameplay(player_pos, player_yaw)
            }
        }
    }

    fn resolved_handoff_pose(&self, pose_override: Option<(Vec3, f32)>) -> (Vec3, f32) {
        pose_override
            .or(self.pending_player_handoff)
            .or_else(|| {
                self.saved_gameplay
                    .as_ref()
                    .map(|s| (s.player_pos, s.player_yaw))
            })
            .unwrap_or((Vec3::ZERO, 0.0))
    }

    fn reset_at_pose(
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        enemy_framing: Option<&mut EnemyEncounterFraming>,
        enemies: &[EnemyCameraTarget],
    ) {
        if let Some(ef) = enemy_framing {
            ef.reset_camera_with_enemies(
                camera,
                player_pos,
                player_yaw,
                PlayerCameraSuppression::Active,
                enemies,
            );
        } else {
            camera.reset_to_player(player_pos, player_yaw);
        }
    }

    fn finish_to_gameplay(&mut self, player_pos: Vec3, player_yaw: f32) -> HandoffStepOutcome {
        self.phase = CutsceneHandoffPhase::Gameplay;
        self.sequence_chain = &[];
        self.chain_cursor = 0;
        self.pending_player_handoff = None;
        HandoffStepOutcome::RestoredToGameplay {
            player_pos,
            player_yaw,
        }
    }
}
