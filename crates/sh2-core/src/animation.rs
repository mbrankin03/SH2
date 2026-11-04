//! Character animation state machine (Locomotion, Combat, Actions) and skeletal track evaluation.

use std::time::Duration;

/// High-level character animation locomotion and action states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnimationStateKind {
    /// Idle standing stance.
    #[default]
    Idle,
    /// Forward or turn walking movement.
    Walk,
    /// High-speed sprint or running movement.
    Run,
    /// Weapon raised / aiming stance.
    Aim,
    /// Melee strike or firearm discharge action.
    Attack,
    /// Stumble or damage reaction.
    Damage,
}

/// Tuple representing an animation cursor: `(frame_0, frame_1, alpha)`.
pub type FrameCursor = (u32, u32, f32);

/// Tuple representing an animation blend cursor: `(previous_cursor, current_cursor, blend_factor)`.
pub type BlendedFrameCursor = (Option<FrameCursor>, FrameCursor, f32);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationClip {
    /// First frame index in the animation track.
    pub start_frame: u32,
    /// Last frame index in the animation track (inclusive).
    pub end_frame: u32,
    /// Frame playback rate (frames per second, typically 30.0 or 60.0).
    pub fps: f32,
    /// Whether the animation loops continuously.
    pub looping: bool,
}

impl AnimationClip {
    /// Constructs a new animation clip definition.
    pub const fn new(start_frame: u32, end_frame: u32, fps: f32, looping: bool) -> Self {
        Self {
            start_frame,
            end_frame,
            fps,
            looping,
        }
    }

    /// Total number of frames spanned by this clip.
    pub const fn frame_count(&self) -> u32 {
        if self.end_frame >= self.start_frame {
            self.end_frame - self.start_frame + 1
        } else {
            1
        }
    }

    /// Total duration in seconds of one playback cycle.
    pub fn duration_seconds(&self) -> f32 {
        if self.fps > 0.0 {
            self.frame_count() as f32 / self.fps
        } else {
            0.0
        }
    }
}

/// Default standard animation clips for James Sunderland in `jms_basic1.anm`.
///
/// Format verified from reverse engineering data:
/// - Idle: frames 0..=50 (looping)
/// - Walk: frames 51..=91 (looping)
/// - Run: frames 92..=170 (looping)
/// - Aim: frames 200..=230 (looping)
/// - Attack: frames 231..=270 (single-shot)
pub struct JamesBasicClips;

impl JamesBasicClips {
    /// Standard breathing / standing idle loop (frames 0..=50).
    pub const IDLE: AnimationClip = AnimationClip::new(0, 50, 30.0, true);
    /// Forward walk cycle (frames 51..=91).
    pub const WALK: AnimationClip = AnimationClip::new(51, 91, 30.0, true);
    /// Full sprint run cycle (frames 92..=170).
    pub const RUN: AnimationClip = AnimationClip::new(92, 170, 30.0, true);
    /// Weapon aim readiness stance (frames 200..=230).
    pub const AIM: AnimationClip = AnimationClip::new(200, 230, 30.0, true);
    /// Weapon strike / attack execution (frames 231..=270).
    pub const ATTACK: AnimationClip = AnimationClip::new(231, 270, 30.0, false);
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
    /// Previous state during a crossfade transition, if blending.
    previous_state: Option<(AnimationStateKind, AnimationClip, f32)>,
    /// Transition blend factor in `[0.0, 1.0]`, where 1.0 is fully transitioned to current state.
    blend_factor: f32,
    /// Total duration of state transition crossfades.
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
            previous_state: None,
            blend_factor: 1.0,
            transition_duration: 0.15, // 150ms smooth transition crossfade
        }
    }

    /// Current active animation state.
    pub fn state(&self) -> AnimationStateKind {
        self.current_state
    }

    /// Current active animation clip.
    pub fn current_clip(&self) -> AnimationClip {
        self.current_clip
    }

    /// Current playhead position in seconds within active clip.
    pub fn clip_time(&self) -> f32 {
        self.clip_time
    }

    /// Sets crossfade transition duration between states (e.g. 0.1s - 0.25s).
    pub fn set_transition_duration(&mut self, duration_seconds: f32) {
        self.transition_duration = duration_seconds.max(0.001);
    }

    /// Transitions to a new animation state and clip.
    ///
    /// If the new state matches the current state, this is a no-op.
    pub fn transition_to(&mut self, new_state: AnimationStateKind, new_clip: AnimationClip) {
        if self.current_state == new_state {
            return;
        }

        self.previous_state = Some((self.current_state, self.current_clip, self.clip_time));
        self.current_state = new_state;
        self.current_clip = new_clip;
        self.clip_time = 0.0;
        self.blend_factor = 0.0;
    }

    /// Advances the animation playhead by the specified delta time.
    pub fn update(&mut self, dt: Duration) {
        let dt_sec = dt.as_secs_f32();

        // Advance blend factor if in transition
        if self.blend_factor < 1.0 {
            self.blend_factor = (self.blend_factor + dt_sec / self.transition_duration).min(1.0);
            if self.blend_factor >= 1.0 {
                self.previous_state = None;
            }
        }

        // Advance active clip time
        let clip_dur = self.current_clip.duration_seconds();
        if clip_dur > 0.0 {
            self.clip_time += dt_sec;
            if self.current_clip.looping {
                self.clip_time %= clip_dur;
            } else if self.clip_time >= clip_dur {
                self.clip_time = clip_dur;
            }
        }

        // Advance previous clip time during blend to prevent frozen poses
        if let Some((_, prev_clip, ref mut prev_time)) = self.previous_state {
            let prev_dur = prev_clip.duration_seconds();
            if prev_dur > 0.0 {
                *prev_time += dt_sec;
                if prev_clip.looping {
                    *prev_time %= prev_dur;
                } else if *prev_time >= prev_dur {
                    *prev_time = prev_dur;
                }
            }
        }
    }

    /// Computes the active track sampling frame indices and sub-frame interpolation alpha:
    /// returns `(frame_0, frame_1, alpha)`.
    pub fn evaluate_sample_cursor(&self) -> FrameCursor {
        Self::calc_cursor(&self.current_clip, self.clip_time)
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
            .map(|(_, prev_clip, prev_time)| Self::calc_cursor(prev_clip, *prev_time));

        (prev, cur, self.blend_factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_durations_and_frames() {
        let idle = JamesBasicClips::IDLE;
        assert_eq!(idle.frame_count(), 51);
        assert!((idle.duration_seconds() - 1.7).abs() < 1e-4);

        let walk = JamesBasicClips::WALK;
        assert_eq!(walk.frame_count(), 41);
        assert!(walk.looping);

        let attack = JamesBasicClips::ATTACK;
        assert_eq!(attack.frame_count(), 40);
        assert!(!attack.looping);
    }

    #[test]
    fn test_animation_controller_transitions() {
        let mut controller =
            CharacterAnimationController::new(AnimationStateKind::Idle, JamesBasicClips::IDLE);
        assert_eq!(controller.state(), AnimationStateKind::Idle);

        // Advance 0.5s in idle
        controller.update(Duration::from_millis(500));
        let (f0, f1, alpha) = controller.evaluate_sample_cursor();
        assert_eq!(f0, 15);
        assert_eq!(f1, 16);
        assert!((alpha - 0.0).abs() < 1e-4);

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
        assert!(cur.0 >= 51 && cur.0 <= 91);
    }
}
