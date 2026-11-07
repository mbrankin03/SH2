//! Character animation state machine (Locomotion, Combat, Actions) and skeletal track evaluation.

use std::time::Duration;

/// High-level character animation locomotion and action states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnimationStateKind {
    /// Idle standing stance.
    #[default]
    Idle,
    /// Forward walking movement.
    Walk,
    /// Backward walking movement.
    Back,
    /// High-speed sprint or running movement.
    Run,
    /// Weapon raised / aiming stance.
    Aim,
    /// Melee strike or firearm discharge action.
    Attack,
    /// Stumble or damage reaction.
    Damage,
    /// Grabbed by creature and struggling to break free (`0x533d00`).
    Held,
    /// Knocked down or collapsed on the floor.
    Fall,
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
/// Authentic frame ranges matching original game bytecode / PS2 engine:
/// - Stand (Idle): frames 1..=10 (subtle breathing loop)
/// - Walk: frames 51..=74 (looping forward stride)
/// - Back: frames 75..=98 (looping backward step)
/// - Run: frames 99..=146 (looping sprint cycle)
/// - Aim: frames 200..=230 (looping)
/// - Attack: frames 231..=270 (single-shot)
pub struct JamesBasicClips;

impl JamesBasicClips {
    /// Authentic standing idle loop (frames 1..=10, 15 fps subtle breathing).
    pub const IDLE: AnimationClip = AnimationClip::new(1, 10, 15.0, true);
    /// Forward walk cycle (frames 51..=74, 30 fps).
    pub const WALK: AnimationClip = AnimationClip::new(51, 74, 30.0, true);
    /// Backward step cycle (frames 75..=98, 30 fps).
    pub const BACK: AnimationClip = AnimationClip::new(75, 98, 30.0, true);
    /// Full sprint run cycle (frames 99..=146, 30 fps).
    pub const RUN: AnimationClip = AnimationClip::new(99, 146, 30.0, true);
    /// Weapon aim readiness stance (frames 200..=230).
    pub const AIM: AnimationClip = AnimationClip::new(200, 230, 30.0, true);
    /// Weapon strike / attack execution (frames 231..=270).
    pub const ATTACK: AnimationClip = AnimationClip::new(231, 270, 30.0, false);
    /// Damage / hurt flinch reaction (frames 453..=470, PS2 index 122, speed 1024 = 15.0 fps).
    pub const HURT: AnimationClip = AnimationClip::new(453, 470, 15.0, false);
    /// Authentic PS2/PC James struggle clip against overhead neck chokes / Flesh Lips (frames 457..=468, PS2 lower_hold lines 0x21dc80-0x21dca4, speed 1024 = 15.0 fps).
    /// James tilts his head upward with mouth gasping and raises both hands up reaching for the creature's legs strangling his throat.
    pub const STRUGGLE_OVERHEAD: AnimationClip = AnimationClip::new(457, 468, 15.0, true);
    /// Generic struggle held animation (used as fallback: frames 457..=461 inside J_HURT, 15.0 fps).
    pub const STRUGGLE_HELD: AnimationClip = AnimationClip::new(457, 461, 15.0, true);
    /// Authentic PS2/PC James struggle clip against leg attacks / Creepers
    /// (pjames_anim aid 127 frames 588..=611, speed 2048 = 30.0 fps).
    pub const STRUGGLE_LEG: AnimationClip = AnimationClip::new(588, 611, 30.0, true);
    /// Authentic PS2/PC James struggle clip against torso grabs / Lying Figure & Abstract Daddy
    /// (pjames_anim aid 128 frames 612..=629, speed 1024 = 15.0 fps).
    pub const STRUGGLE_TORSO: AnimationClip = AnimationClip::new(612, 629, 15.0, true);
    /// Authentic PS2/PC James struggle clip against standing throat / tentacle choke holds / Mary Boss
    /// (pjames_anim aid 129 frames 632..=649, speed 1024 = 15.0 fps).
    /// James desperately reaches up with both hands clutching at his throat and fighting against the tentacles strangling him.
    pub const STRUGGLE_CHOKE_MARY: AnimationClip = AnimationClip::new(632, 649, 15.0, true);
    /// Authentic PS2/PC James breakout release push when breaking free from grab
    /// (pjames_anim aid 130 frames 652..=670, speed 2048 = 30.0 fps).
    pub const STRUGGLE_RELEASE: AnimationClip = AnimationClip::new(652, 670, 30.0, false);
    /// Authentic PS2/PC James struggle clip against torso pinning / standing grapples / Abstract Daddy & Lying Figure
    /// (pjames_anim aid 128 frames 612..=629, PS2 speed 1024 = 15.0 fps).
    pub const STRUGGLE_DADDY: AnimationClip = AnimationClip::new(612, 629, 15.0, true);
    /// Authentic PS2/PC James struggle clip against Flesh Lips overhead ceiling choke
    /// (pjames_anim aid 132 frames 682..=701, speed 768 = 11.25 fps).
    pub const STRUGGLE_FLESH_LIPS: AnimationClip = AnimationClip::new(682, 701, 11.25, true);
    /// Authentic PS2/PC James struggle clip against Pyramid Head one-handed overhead choke lift
    /// (pjames_anim aid 133 frames 702..=719, speed 512 = 7.5 fps).
    pub const STRUGGLE_CHOKE_LIFT: AnimationClip = AnimationClip::new(702, 719, 7.5, true);
    /// Authentic PS2/PC James breakout release push from Mary / Lying Figure choke (pjames_anim aid 130 frames 652..=670, speed 2048 = 30.0 fps).
    pub const STRUGGLE_CHOKE_RELEASE: AnimationClip = AnimationClip::new(652, 670, 30.0, false);
    /// Collapse / death fall animation (frames 256..=280, PS2 index 112, speed 1024 = 15.0 fps).
    pub const FALL: AnimationClip = AnimationClip::new(256, 280, 15.0, false);
}

/// Standard locomotion animation clips for Maria in `xmar_wpnone.anm` (Born From a Wish).
pub struct MariaLocomotionClips;

impl MariaLocomotionClips {
    /// Standing idle loop (frames 1..=10).
    pub const IDLE: AnimationClip = AnimationClip::new(1, 10, 15.0, true);
    /// Forward walk cycle (frames 51..=74).
    pub const WALK: AnimationClip = AnimationClip::new(51, 74, 30.0, true);
    /// Backward step cycle (frames 75..=98).
    pub const BACK: AnimationClip = AnimationClip::new(75, 98, 30.0, true);
    /// Full run cycle (frames 99..=146).
    pub const RUN: AnimationClip = AnimationClip::new(99, 146, 30.0, true);
}

/// Locomotion clip set configuration for the active playable character.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterLocomotionClips {
    pub idle: AnimationClip,
    pub walk: AnimationClip,
    pub back: AnimationClip,
    pub run: AnimationClip,
}

impl Default for CharacterLocomotionClips {
    fn default() -> Self {
        Self {
            idle: JamesBasicClips::IDLE,
            walk: JamesBasicClips::WALK,
            back: JamesBasicClips::BACK,
            run: JamesBasicClips::RUN,
        }
    }
}

impl CharacterLocomotionClips {
    /// James Sunderland standard clips.
    pub const JAMES: Self = Self {
        idle: JamesBasicClips::IDLE,
        walk: JamesBasicClips::WALK,
        back: JamesBasicClips::BACK,
        run: JamesBasicClips::RUN,
    };

    /// Maria locomotion clips.
    pub const MARIA: Self = Self {
        idle: MariaLocomotionClips::IDLE,
        walk: MariaLocomotionClips::WALK,
        back: MariaLocomotionClips::BACK,
        run: MariaLocomotionClips::RUN,
    };
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
        assert_eq!(idle.frame_count(), 10);
        assert!(idle.looping);

        let walk = JamesBasicClips::WALK;
        assert_eq!(walk.frame_count(), 24);
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
}
