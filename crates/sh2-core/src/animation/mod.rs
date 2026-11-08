//! Character animation state machine (Locomotion, Combat, Actions) and skeletal track evaluation.
//!
//! # Architecture & Submodules
//! - [`clip`]: Core types (`AnimationClip`, `AnimationStateKind`, `BlendMode`, `FrameCursor`, `BlendedFrameCursor`).
//! - [`tables`]: Authentic PS2/PC `_AnimeInfo` tables (`JamesBasicClips`, `JamesStageClips`, `MariaLocomotionClips`).
//! - [`gait_sync`]: Pure locomotion-to-animation synchronization (`LocomotionAnimationSync`, `LocomotionAnimCommand`,
//!   Mode 10 gait phase preservation across 24-frame stride cycles).

pub mod clip;
pub mod gait_sync;
pub mod tables;

pub use clip::{
    ps2_speed_to_fps, AnimationClip, AnimationStateKind, BlendMode, BlendedFrameCursor,
    FrameCursor, MODE10_GAIT_SYNC_BLEND_DURATION_SECS, MODE4_POSE_BLEND_DURATION_SECS,
    PS2_BASE_FPS, PS2_SPEED_UNIT_30FPS,
};
pub use gait_sync::{
    LocomotionAnimCommand, LocomotionAnimationSync, BASE_SPOT_TURN_RAD_PER_SEC,
    MAX_BACK_LINEAR_SPEED, MAX_WALK_LINEAR_SPEED,
};
pub use tables::{
    CharacterLocomotionClips, JamesBasicClips, JamesStageClips, MariaLocomotionClips,
};

use std::time::Duration;

/// Previous state snapshot during an active crossfade transition.
#[derive(Debug, Clone, Copy)]
struct PreviousTransitionState {
    /// Outgoing state kind.
    _state: AnimationStateKind,
    /// Outgoing animation clip.
    clip: AnimationClip,
    /// Outgoing playhead time in seconds (used in Mode 4 pose crossfade).
    time: f32,
    /// Outgoing playback rate scale.
    rate_scale: f32,
    /// Whether this transition is Mode 10 gait phase-locked (`0x531a00`), where the outgoing clip
    /// stays locked to the incoming clip's relative frame offset `(0..len)` across 32 frames.
    gait_phase_sync: bool,
}

/// Runtime animation state machine driving character skeletal pose evaluation.
#[derive(Debug, Clone)]
pub struct CharacterAnimationController {
    /// Current active state kind.
    current_state: AnimationStateKind,
    /// Clip configuration associated with current state.
    current_clip: AnimationClip,
    /// Playhead time in seconds within the active clip.
    clip_time: f32,
    /// Dynamic playback speed multiplier (`1.0` = clip's base FPS; scaled by stick tilt/speed for Walk/Back/Turn).
    playback_rate_scale: f32,
    /// Previous state during a crossfade transition, if blending.
    previous_state: Option<PreviousTransitionState>,
    /// Transition blend factor in `[0.0, 1.0]`, where `1.0` is fully transitioned to current state.
    blend_factor: f32,
    /// Total duration of the active state transition crossfade in seconds.
    transition_duration: f32,
}

impl Default for CharacterAnimationController {
    fn default() -> Self {
        Self::new(AnimationStateKind::Idle, JamesBasicClips::IDLE)
    }
}

impl CharacterAnimationController {
    /// Creates a new character animation controller with the initial state and clip.
    pub fn new(initial_state: AnimationStateKind, initial_clip: AnimationClip) -> Self {
        Self {
            current_state: initial_state,
            current_clip: initial_clip,
            clip_time: 0.0,
            playback_rate_scale: 1.0,
            previous_state: None,
            blend_factor: 1.0,
            transition_duration: 0.15, // Default 150ms smooth transition crossfade
        }
    }

    /// Current active animation state.
    #[inline]
    pub fn state(&self) -> AnimationStateKind {
        self.current_state
    }

    /// Current active animation clip.
    #[inline]
    pub fn current_clip(&self) -> AnimationClip {
        self.current_clip
    }

    /// Current playhead position in seconds within active clip.
    #[inline]
    pub fn clip_time(&self) -> f32 {
        self.clip_time
    }

    /// Current playback rate multiplier (`1.0` = clip's base FPS).
    #[inline]
    pub fn playback_rate_scale(&self) -> f32 {
        self.playback_rate_scale
    }

    /// Effective playback rate in frames per second (`current_clip.fps * playback_rate_scale`).
    #[inline]
    pub fn effective_fps(&self) -> f32 {
        self.current_clip.fps * self.playback_rate_scale
    }

    /// Current relative frame offset (`0.0 .. frame_count`) within the active clip.
    #[inline]
    pub fn relative_frame_offset(&self) -> f32 {
        if self.current_clip.fps > 0.0 {
            let total = self.current_clip.frame_count() as f32;
            let raw = self.clip_time * self.current_clip.fps;
            if self.current_clip.looping && total > 0.0 {
                raw.rem_euclid(total)
            } else {
                raw.clamp(0.0, (total - 1.0).max(0.0))
            }
        } else {
            0.0
        }
    }

    /// Whether the active crossfade is currently using PS2 Mode 10 gait phase synchronization.
    #[inline]
    pub fn is_gait_phase_synced_blend(&self) -> bool {
        self.previous_state
            .as_ref()
            .is_some_and(|prev| prev.gait_phase_sync)
    }

    /// Sets the dynamic playback rate multiplier (`>= 0.0`).
    #[inline]
    pub fn set_playback_rate_scale(&mut self, scale: f32) {
        self.playback_rate_scale = scale.max(0.0);
    }

    /// Sets crossfade transition duration between states (e.g. `0.1s - 1.0667s`).
    #[inline]
    pub fn set_transition_duration(&mut self, duration_seconds: f32) {
        self.transition_duration = duration_seconds.max(0.001);
    }

    /// Transitions to a new animation state and clip using the controller's current `transition_duration`.
    ///
    /// If the new state and clip match the current state, this is a no-op.
    pub fn transition_to(&mut self, new_state: AnimationStateKind, new_clip: AnimationClip) {
        let mode = BlendMode::PoseCrossfade {
            duration_secs: self.transition_duration,
        };
        self.transition_to_with_mode(new_state, new_clip, mode);
    }

    /// Transitions to a new animation state and clip using an explicit PS2 `BlendMode`
    /// (`Mode 2` Cut, `Mode 4` Pose Crossfade, or `Mode 10` Gait Phase Sync).
    pub fn transition_to_with_mode(
        &mut self,
        new_state: AnimationStateKind,
        new_clip: AnimationClip,
        blend_mode: BlendMode,
    ) {
        if self.current_state == new_state && self.current_clip == new_clip {
            return;
        }

        let old_state = self.current_state;
        let old_clip = self.current_clip;
        let old_time = self.clip_time;
        let old_rate_scale = self.playback_rate_scale;
        let old_rel_frame = self.relative_frame_offset();

        self.current_state = new_state;
        self.current_clip = new_clip;
        self.playback_rate_scale = 1.0;

        match blend_mode {
            BlendMode::Cut => {
                // Mode 2 (0x531a00): Immediate cut with zero crossfade
                self.previous_state = None;
                self.clip_time = 0.0;
                self.blend_factor = 1.0;
            }
            BlendMode::GaitPhaseSync { duration_secs }
                if old_clip.frame_count() == new_clip.frame_count() && new_clip.fps > 0.0 =>
            {
                // Mode 10 (0x531a00): 32-frame crossfade keeping identical stride phase across 24-frame gaits
                self.transition_duration = duration_secs.max(0.001);
                self.clip_time = old_rel_frame / new_clip.fps;
                self.blend_factor = 0.0;
                self.previous_state = Some(PreviousTransitionState {
                    _state: old_state,
                    clip: old_clip,
                    time: old_time,
                    rate_scale: old_rate_scale,
                    gait_phase_sync: true,
                });
            }
            BlendMode::PoseCrossfade { duration_secs }
            | BlendMode::GaitPhaseSync { duration_secs } => {
                // Mode 4 (0x538920): Standard crossfade starting incoming clip at frame 0
                self.transition_duration = duration_secs.max(0.001);
                self.clip_time = 0.0;
                self.blend_factor = 0.0;
                self.previous_state = Some(PreviousTransitionState {
                    _state: old_state,
                    clip: old_clip,
                    time: old_time,
                    rate_scale: old_rate_scale,
                    gait_phase_sync: false,
                });
            }
        }
    }

    /// Applies a resolved `LocomotionAnimCommand` from [`LocomotionAnimationSync::resolve`].
    #[inline]
    pub fn apply_locomotion_command(&mut self, cmd: LocomotionAnimCommand) {
        self.transition_to_with_mode(cmd.state, cmd.clip, cmd.blend_mode);
        self.set_playback_rate_scale(cmd.playback_rate_scale);
    }

    /// Advances the animation playhead by the specified delta time.
    pub fn update(&mut self, dt: Duration) {
        let dt_sec = dt.as_secs_f32();

        // Advance blend factor if in transition (crossfade duration is always in real wall-clock time)
        if self.blend_factor < 1.0 {
            self.blend_factor = (self.blend_factor + dt_sec / self.transition_duration).min(1.0);
            if self.blend_factor >= 1.0 {
                self.previous_state = None;
            }
        }

        // Advance active clip time scaled by playback_rate_scale
        let clip_dur = self.current_clip.duration_seconds();
        if clip_dur > 0.0 {
            self.clip_time += dt_sec * self.playback_rate_scale;
            if self.current_clip.looping {
                self.clip_time %= clip_dur;
            } else if self.clip_time >= clip_dur {
                self.clip_time = clip_dur;
            }
        }

        // Advance previous clip time during blend to prevent frozen poses
        let current_rel_frame = self.relative_frame_offset();
        if let Some(ref mut prev) = self.previous_state {
            if prev.gait_phase_sync && prev.clip.fps > 0.0 {
                // Mode 10 (chr.c chr_pose_blend): lock outgoing clip's relative frame to active clip's relative frame
                prev.time = current_rel_frame / prev.clip.fps;
            } else {
                let prev_dur = prev.clip.duration_seconds();
                if prev_dur > 0.0 {
                    prev.time += dt_sec * prev.rate_scale;
                    if prev.clip.looping {
                        prev.time %= prev_dur;
                    } else if prev.time >= prev_dur {
                        prev.time = prev_dur;
                    }
                }
            }
        }
    }

    /// Computes the active track sampling frame indices and sub-frame interpolation alpha:
    /// returns `(frame_0, frame_1, alpha)`.
    pub fn evaluate_sample_cursor(&self) -> FrameCursor {
        Self::calc_cursor(&self.current_clip, self.clip_time)
    }

    /// Computes the continuous sub-tick sampling cursor `(frame_0, frame_1, alpha)` for
    /// high-refresh rendering (120–240 Hz+) given the simulation clock remainder `sim_alpha` in `[0.0, 1.0]`.
    pub fn evaluate_sample_cursor_at_alpha(&self, sim_alpha: f32) -> FrameCursor {
        let sub_dt = sim_alpha.clamp(0.0, 1.0) * (1.0 / 60.0) * self.playback_rate_scale;
        let clip_dur = self.current_clip.duration_seconds();
        let effective_time = if clip_dur > 0.0 {
            let t = self.clip_time + sub_dt;
            if self.current_clip.looping {
                t % clip_dur
            } else {
                t.min(clip_dur)
            }
        } else {
            self.clip_time
        };
        Self::calc_cursor(&self.current_clip, effective_time)
    }

    /// Computes sampling cursor for a given clip and playback time.
    fn calc_cursor(clip: &AnimationClip, time: f32) -> FrameCursor {
        let total_frames = clip.frame_count();
        if total_frames <= 1 || clip.fps <= 0.0 {
            return (clip.start_frame, clip.start_frame, 0.0);
        }

        let raw_frame = time * clip.fps;
        let frame_offset = raw_frame.floor() as u32;
        let alpha = raw_frame - raw_frame.floor();

        if clip.looping {
            let f0 = clip.start_frame + (frame_offset % total_frames);
            let f1 = clip.start_frame + ((frame_offset + 1) % total_frames);
            (f0, f1, alpha)
        } else {
            let f0 = (clip.start_frame + frame_offset).min(clip.end_frame);
            let f1 = (clip.start_frame + frame_offset + 1).min(clip.end_frame);
            (f0, f1, alpha)
        }
    }

    /// Returns the transition blending factor `(previous_cursor, current_cursor, blend_factor)`.
    ///
    /// When `blend_factor == 1.0`, only `current_cursor` is needed.
    pub fn evaluate_blended_cursor(&self) -> BlendedFrameCursor {
        let cur = self.evaluate_sample_cursor();
        let prev = self
            .previous_state
            .as_ref()
            .map(|prev| Self::calc_cursor(&prev.clip, prev.time));

        (prev, cur, self.blend_factor)
    }

    /// Returns the continuous sub-tick transition blending cursor `(previous_cursor, current_cursor, blend_factor)`
    /// interpolated by `sim_alpha` (`0.0..1.0`) between 60 Hz simulation ticks for 120–240 Hz+ rendering.
    pub fn evaluate_blended_cursor_at_alpha(&self, sim_alpha: f32) -> BlendedFrameCursor {
        let clamped_alpha = sim_alpha.clamp(0.0, 1.0);
        if clamped_alpha <= 0.0 {
            return self.evaluate_blended_cursor();
        }
        let sub_dt_wall = clamped_alpha * (1.0 / 60.0);
        let cur = self.evaluate_sample_cursor_at_alpha(clamped_alpha);
        let effective_blend = if self.blend_factor < 1.0 {
            (self.blend_factor + sub_dt_wall / self.transition_duration).min(1.0)
        } else {
            1.0
        };
        let prev = if effective_blend < 1.0 {
            self.previous_state.as_ref().map(|prev| {
                if prev.gait_phase_sync {
                    // In Mode 10 gait phase sync, outgoing relative frame is identical to current relative frame
                    let rel_frame = (cur.0 - self.current_clip.start_frame) as f32 + cur.2;
                    let total = prev.clip.frame_count();
                    let offset = (rel_frame.floor() as u32) % total.max(1);
                    let f0 = prev.clip.start_frame + offset;
                    let f1 = prev.clip.start_frame + ((offset + 1) % total.max(1));
                    (f0, f1, cur.2)
                } else {
                    let prev_dur = prev.clip.duration_seconds();
                    let t = if prev_dur > 0.0 {
                        let raw = prev.time + sub_dt_wall * prev.rate_scale;
                        if prev.clip.looping {
                            raw % prev_dur
                        } else {
                            raw.min(prev_dur)
                        }
                    } else {
                        prev.time
                    };
                    Self::calc_cursor(&prev.clip, t)
                }
            })
        } else {
            None
        };
        (prev, cur, effective_blend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locomotion::{LocomotionOutput, LocomotionState};
    use glam::Vec3;

    #[test]
    fn test_clip_durations_and_frames() {
        let idle = JamesBasicClips::IDLE;
        assert_eq!(idle.frame_count(), 10);
        assert!(idle.looping);

        let walk = JamesBasicClips::WALK;
        assert_eq!(walk.frame_count(), 24);
        assert!(walk.looping);
        assert!((walk.fps - 20.625).abs() < 1e-4);

        let back = JamesBasicClips::BACK;
        assert_eq!(back.frame_count(), 24);
        assert!((back.fps - 22.5).abs() < 1e-4);

        let run1 = JamesBasicClips::RUN1;
        let run2 = JamesBasicClips::RUN2;
        let run3 = JamesBasicClips::RUN3;
        assert_eq!(run1.frame_count(), 24);
        assert_eq!(run2.frame_count(), 24);
        assert_eq!(run3.frame_count(), 24);
        assert!((run1.fps - 22.5).abs() < 1e-4);
        assert!((run2.fps - 26.25).abs() < 1e-4);
        assert!((run3.fps - 30.0).abs() < 1e-4);

        let attack = JamesBasicClips::ATTACK;
        assert_eq!(attack.frame_count(), 40);
        assert!(!attack.looping);
    }

    #[test]
    fn test_animation_controller_transitions() {
        let mut controller =
            CharacterAnimationController::new(AnimationStateKind::Idle, JamesBasicClips::IDLE);
        assert_eq!(controller.state(), AnimationStateKind::Idle);

        // Advance 0.2s in idle
        controller.update(Duration::from_millis(200));
        let (f0, f1, alpha) = controller.evaluate_sample_cursor();
        assert!((1..=10).contains(&f0));
        assert!((1..=10).contains(&f1));
        assert!((0.0..=1.0).contains(&alpha));

        // Transition to Walk
        controller.transition_to(AnimationStateKind::Walk, JamesBasicClips::WALK);
        assert_eq!(controller.state(), AnimationStateKind::Walk);

        let (prev, cur, blend) = controller.evaluate_blended_cursor();
        assert!(prev.is_some());
        assert_eq!(cur.0, 51); // First frame of walk
        assert_eq!(blend, 0.0);

        // Step halfway through transition (75ms out of 150ms)
        controller.update(Duration::from_millis(75));
        let (_, _, blend) = controller.evaluate_blended_cursor();
        assert!((blend - 0.5).abs() < 0.05);

        // Step past transition
        controller.update(Duration::from_millis(100));
        let (prev, cur, blend) = controller.evaluate_blended_cursor();
        assert!(prev.is_none());
        assert_eq!(blend, 1.0);
        assert!(cur.0 >= 51 && cur.0 <= 74);
    }

    #[test]
    fn test_sh2_26_tilt_and_linear_speed_playback_scaling() {
        let clips = CharacterLocomotionClips::JAMES;

        // 1. Half-tilt walk (375 units/s = 0.5 * 750 units/s) vs Full-tilt walk (750 units/s)
        let mut ctrl_half = CharacterAnimationController::default();
        let mut ctrl_full = CharacterAnimationController::default();

        let out_half = LocomotionOutput {
            delta_position: Vec3::ZERO,
            turn_speed_rad_per_sec: 0.0,
            state: LocomotionState::Walk,
            linear_speed: 375.0,
            footstep_triggered: false,
            footstep_volume: 0.5,
        };
        let out_full = LocomotionOutput {
            linear_speed: 750.0,
            ..out_half
        };

        ctrl_half.apply_locomotion_command(LocomotionAnimationSync::resolve(
            ctrl_half.state(),
            &out_half,
            &clips,
        ));
        ctrl_full.apply_locomotion_command(LocomotionAnimationSync::resolve(
            ctrl_full.state(),
            &out_full,
            &clips,
        ));

        assert!((ctrl_half.playback_rate_scale() - 0.5).abs() < 1e-4);
        assert!((ctrl_full.playback_rate_scale() - 1.0).abs() < 1e-4);
        assert!((ctrl_half.effective_fps() - 10.3125).abs() < 1e-4);
        assert!((ctrl_full.effective_fps() - 20.625).abs() < 1e-4);

        // Advance both by 0.5 seconds
        ctrl_half.update(Duration::from_millis(500));
        ctrl_full.update(Duration::from_millis(500));

        // Full tilt should have advanced exactly twice as many frames as half tilt
        let rel_half = ctrl_half.relative_frame_offset();
        let rel_full = ctrl_full.relative_frame_offset();
        assert!((rel_full - rel_half * 2.0).abs() < 1e-3);

        // 2. Turn-on-spot scaling (Clip 102/103 @ 18.75 fps base, scaled by turn_speed / 2.4)
        let out_turn_run = LocomotionOutput {
            delta_position: Vec3::ZERO,
            turn_speed_rad_per_sec: 3.6, // 2.4 * 1.5 with run held
            state: LocomotionState::TurnLeft,
            linear_speed: 0.0,
            footstep_triggered: false,
            footstep_volume: 0.5,
        };
        let cmd_turn =
            LocomotionAnimationSync::resolve(AnimationStateKind::Idle, &out_turn_run, &clips);
        assert_eq!(cmd_turn.state, AnimationStateKind::TurnLeft);
        assert_eq!(cmd_turn.clip, JamesBasicClips::TURN_LEFT);
        assert!((cmd_turn.playback_rate_scale - 1.5).abs() < 1e-4);
    }

    #[test]
    fn test_sh2_26_mode10_gait_phase_sync_across_walk_strafe_and_run_stages() {
        let clips = CharacterLocomotionClips::JAMES;
        let mut controller =
            CharacterAnimationController::new(AnimationStateKind::Walk, clips.walk);

        // Advance Walk (51..=74, 24 frames @ 20.625 fps) to exactly relative frame 10.5
        let target_rel_frame = 10.5f32;
        let walk_time = target_rel_frame / clips.walk.fps;
        controller.update(Duration::from_secs_f32(walk_time));
        assert!((controller.relative_frame_offset() - target_rel_frame).abs() < 1e-3);

        // Transition Walk (Clip 104) -> Run 1 (Clip 106, 99..=122 @ 22.5 fps)
        let out_run1 = LocomotionOutput {
            delta_position: Vec3::ZERO,
            turn_speed_rad_per_sec: 0.0,
            state: LocomotionState::Run1,
            linear_speed: 1750.0,
            footstep_triggered: false,
            footstep_volume: 0.8,
        };
        let cmd_run1 = LocomotionAnimationSync::resolve(controller.state(), &out_run1, &clips);
        assert_eq!(
            cmd_run1.blend_mode,
            BlendMode::GaitPhaseSync {
                duration_secs: MODE10_GAIT_SYNC_BLEND_DURATION_SECS
            }
        );
        controller.apply_locomotion_command(cmd_run1);

        // Verify Mode 10 preserved exact relative stride frame 10.5 in Run 1 (frame 99 + 10 = 109, alpha 0.5)
        assert!(controller.is_gait_phase_synced_blend());
        assert!((controller.relative_frame_offset() - target_rel_frame).abs() < 1e-3);
        let (prev_cur, run1_cur, blend) = controller.evaluate_blended_cursor();
        assert_eq!(blend, 0.0);
        assert_eq!(run1_cur.0, 99 + 10);
        assert!((run1_cur.2 - 0.5).abs() < 1e-3);
        let prev_cur = prev_cur.expect("previous cursor must be present during Mode 10 blend");
        assert_eq!(prev_cur.0, 51 + 10);
        assert!((prev_cur.2 - 0.5).abs() < 1e-3);

        // Advance 16 frames at 30Hz (0.5333s = exactly 50% of the 32-frame Mode 10 crossfade)
        controller.update(Duration::from_secs_f32(16.0 / 30.0));
        let (prev_mid, run1_mid, blend_mid) = controller.evaluate_blended_cursor();
        assert!((blend_mid - 0.5).abs() < 1e-3);
        let prev_mid = prev_mid.expect("still in 32-frame crossfade");
        // Outgoing Walk clip and incoming Run1 clip must remain locked at the exact same relative frame offset!
        assert_eq!(
            prev_mid.0 - clips.walk.start_frame,
            run1_mid.0 - clips.run.start_frame
        );
        assert!((prev_mid.2 - run1_mid.2).abs() < 1e-3);

        // Transition Run 1 (Clip 106) -> Run 2 (Clip 107, 123..=146 @ 26.25 fps) -> Run 3 (Clip 108, 147..=170 @ 30.0 fps)
        let rel_before_run2 = controller.relative_frame_offset();
        let out_run2 = LocomotionOutput {
            state: LocomotionState::Run2,
            linear_speed: 2000.0,
            ..out_run1
        };
        controller.apply_locomotion_command(LocomotionAnimationSync::resolve(
            controller.state(),
            &out_run2,
            &clips,
        ));
        assert_eq!(controller.state(), AnimationStateKind::Run2);
        assert_eq!(controller.current_clip(), JamesBasicClips::RUN2);
        assert!((controller.relative_frame_offset() - rel_before_run2).abs() < 1e-3);

        let out_run3 = LocomotionOutput {
            state: LocomotionState::Run3,
            linear_speed: 2400.0,
            ..out_run1
        };
        controller.apply_locomotion_command(LocomotionAnimationSync::resolve(
            controller.state(),
            &out_run3,
            &clips,
        ));
        assert_eq!(controller.state(), AnimationStateKind::Run3);
        assert_eq!(controller.current_clip(), JamesBasicClips::RUN3);
        assert!((controller.relative_frame_offset() - rel_before_run2).abs() < 1e-3);
    }
}
