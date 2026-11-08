//! Core animation clip, cursor, state, and crossfade blend mode types.
//!
//! # Ground Truth Reference
//! - PlayStation 2 Debug Prototype (`SLUS_202.28`) & PC Director's Cut (`sh2pc.exe`):
//!   - `0x6ceac4` (`pjames_anim[34]`): Fixed-point animation speed where `2048 = 30.0 FPS`.
//!   - `0x538920` / `0x551820`: Transition blend modes (`Mode 2` cut, `Mode 4` 8-frame pose crossfade,
//!     `Mode 10` 32-frame gait phase-synchronized crossfade).

/// Fixed-point animation speed divisor in PS2/PC `_AnimeInfo` (`2048 = 30.0 FPS`).
pub const PS2_SPEED_UNIT_30FPS: f32 = 2048.0;

/// Base reference frame rate of PS2/PC `_AnimeInfo` tables (`30.0 FPS`).
pub const PS2_BASE_FPS: f32 = 30.0;

/// Duration of an 8-frame standard pose crossfade at 30 FPS (`Mode 4`, `0x538920`).
pub const MODE4_POSE_BLEND_DURATION_SECS: f32 = 8.0 / PS2_BASE_FPS;

/// Duration of a 32-frame gait-synchronized crossfade at 30 FPS (`Mode 10`, `0x531a00`).
pub const MODE10_GAIT_SYNC_BLEND_DURATION_SECS: f32 = 32.0 / PS2_BASE_FPS;

/// Converts an authentic PS2/PC `_AnimeInfo` fixed-point `speed` (`s16`) into frames per second.
#[inline]
pub const fn ps2_speed_to_fps(speed: i16) -> f32 {
    (speed as f32 / PS2_SPEED_UNIT_30FPS) * PS2_BASE_FPS
}

/// High-level character animation locomotion and action states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnimationStateKind {
    /// Idle standing stance (`Clip 101`).
    #[default]
    Idle,
    /// Subtle relax breathing sequence after standing still 10s (`Clips 115..117`).
    Relax,
    /// Alert look-around stance when an enemy is nearby (`Clip 118`).
    Alert,
    /// Heavy panting / exhausted stamina stance (`Clips 113..114`).
    Tired,
    /// Turn left on the spot (`Clip 103`).
    TurnLeft,
    /// Turn right on the spot (`Clip 102`).
    TurnRight,
    /// Forward walking movement (`Clip 104`).
    Walk,
    /// Backward walking movement (`Clip 105`).
    Back,
    /// Left strafe walking movement (`Clip 125`).
    StrafeLeftWalk,
    /// Right strafe walking movement (`Clip 124`).
    StrafeRightWalk,
    /// High-speed sprint or running movement (`Clips 106..108`, generic/stage 1 alias).
    Run,
    /// Running stage 2 (`Clip 107`).
    Run2,
    /// Running stage 3 full sprint (`Clip 108`).
    Run3,
    /// Left strafe running movement (`Clip 127`).
    StrafeLeftRun,
    /// Right strafe running movement (`Clip 126`).
    StrafeRightRun,
    /// Quick 180° turn left (`Clip 110`).
    QuickTurnLeft,
    /// Quick 180° turn right (`Clip 109`).
    QuickTurnRight,
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

/// Authentic PS2/PC animation transition blend mode (`0x531a00` / `0x538920`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    /// Mode 2: Immediate cut (`0` frames, `blend_factor = 1.0`).
    /// Used for 180° quick turns (`Clips 109/110`), weapon swings, shots, kicks, and hits.
    Cut,
    /// Mode 4: Pose crossfade from the previous clip's playhead over `duration_secs`
    /// (authentically `8.0 / 30.0 = 0.2667s` for locomotion transitions without shared gait phase).
    PoseCrossfade { duration_secs: f32 },
    /// Mode 10: Gait phase-locked crossfade over `duration_secs` (`32.0 / 30.0 = 1.0667s`).
    /// Preserves the normalized stride phase `[0.0, 1.0)` when transitioning between 24-frame
    /// walk, strafe, and run cycles so footsteps never hitch or cross mid-stride.
    GaitPhaseSync { duration_secs: f32 },
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::PoseCrossfade {
            duration_secs: 0.15,
        }
    }
}

/// Definition of a contiguous keyframe animation clip within a character `.anm` file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationClip {
    /// First frame index in the animation track.
    pub start_frame: u32,
    /// Last frame index in the animation track (inclusive).
    pub end_frame: u32,
    /// Base frame playback rate (frames per second, derived from PS2 `speed / 2048.0 * 30.0`).
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

    /// Constructs a new animation clip definition from a PS2 fixed-point `speed` (`2048 = 30.0 FPS`).
    pub const fn from_ps2_speed(
        start_frame: u32,
        end_frame: u32,
        ps2_speed: i16,
        looping: bool,
    ) -> Self {
        Self {
            start_frame,
            end_frame,
            fps: ps2_speed_to_fps(ps2_speed),
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

    /// Total duration in seconds of one playback cycle at base `fps`.
    pub fn duration_seconds(&self) -> f32 {
        if self.fps > 0.0 {
            self.frame_count() as f32 / self.fps
        } else {
            0.0
        }
    }
}
