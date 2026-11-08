//! Skeletal animation clips, state synchronization, root motion extraction,
//! and hit/particle window tracking for creatures and companions (SH2-57).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_scu.c`, `src/Enemy/en_mkn.c`, `src/Enemy/en_nse.c`, `src/Chacter/m3_maria_sub.c`
//! - PC executable `sh2pc.exe` Director's Cut:
//!   - `0x893130`: Lying Figure animation table (scu, 30 bones, 779 retail frames):
//!     - Stand (idle): frames 0..=14 (loop, 15 fps)
//!     - Walk: frames 15..=45 (loop, 30 fps, authentic root motion 13.4 units/frame = 402 units/s)
//!     - Prone Crawl: frames 46..=62 (loop, 30 fps, authentic root motion 50.0 units/frame = 1500 units/s)
//!     - Acid Spray Attack: frames 77..=100 (single-shot, 30 fps; particle spawn window 84..=92, damage window 85..=94)
//!     - Flinch (light): frames 101..=120 (single-shot, 30 fps)
//!     - Knockdown / Writhing: frames 315..=500 (single-shot, 30 fps; writhing loop 400..=500)
//!     - Recovery: frames 501..=583 (single-shot, 30 fps)
//!   - `0x893070`: Mannequin animation table (mkn, 17 bones, 720 retail frames):
//!     - Stand / Stay: frames 0..=23 (loop, 15 fps)
//!     - Walk: frames 24..=54 (loop, 30 fps, authentic root motion 19.2 units/frame = 576 units/s)
//!     - Arm Swing Attack: frames 55..=89 (single-shot, 30 fps; hit window 65..=75)
//!     - Kick Attack: frames 90..=124 (single-shot, 30 fps; hit window 100..=112)
//!     - Flinch / Stagger: frames 125..=155 (single-shot, 30 fps)
//!     - Knockdown / Writhing: frames 324..=470 (single-shot, 30 fps)
//!     - Recovery / Writhing loop: frames 646..=677 (loop, 30 fps)
//!   - Bubble Head Nurse animation table (nse, 30 bones, 658 retail frames):
//!     - Stand (idle): frames 0..=17 (loop, 15 fps)
//!     - Twitch Walk / Advance: frames 18..=48 (loop, 30 fps, erratic root motion avg 16.0 units/frame = 480 units/s)
//!     - Pipe Swing Attack: frames 62..=98 (single-shot, 30 fps; hit window 74..=86)
//!     - Flinch: frames 99..=125 (single-shot, 30 fps)
//!     - Knockdown: frames 210..=310 (single-shot, 30 fps)
//!   - Maria Companion animation table (mar, 36 bones, 851 retail frames in `lll_mar.anm`):
//!     - Stand (idle): frames 0..=29 (loop, 15 fps)
//!     - Walk: frames 30..=60 (loop, 30 fps, root motion 10.0 units/frame = 300 units/s)
//!     - Run: frames 61..=85 (loop, 30 fps, root motion 20.0 units/frame = 600 units/s)
//!     - Cower / Panic: frames 86..=120 (loop, 20 fps)
//!     - Flinch / Downed: frames 121..=165 (single-shot, 30 fps)

use crate::ai::creatures::lying_figure::ScuState;
use crate::ai::creatures::mannequin::MknState;
use crate::ai::creatures::nurse::NseState;
use crate::ai::maria::MariaState;
pub use crate::ai::npcs::{
    AngelaAnimClips, EdbState, EddieBossAnimClips, EddieNpcAnimClips, LauraAnimClips,
    MaryNpcAnimClips, NpcAnimState,
};
use crate::animation::{AnimationClip, BlendedFrameCursor, FrameCursor};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Clip definitions for Lying Figure (`chr/scu`, 30 bones, 0x893130).
pub struct LyingFigureAnimClips;

impl LyingFigureAnimClips {
    pub const STAND: AnimationClip = AnimationClip::new(0, 14, 15.0, true);
    pub const WALK: AnimationClip = AnimationClip::new(15, 45, 30.0, true);
    pub const PRONE_CRAWL: AnimationClip = AnimationClip::new(46, 62, 30.0, true);
    pub const ACID_SPRAY: AnimationClip = AnimationClip::new(77, 100, 30.0, false);
    pub const FLINCH: AnimationClip = AnimationClip::new(101, 120, 30.0, false);
    pub const KNOCKDOWN: AnimationClip = AnimationClip::new(315, 500, 30.0, false);
    pub const RECOVERY: AnimationClip = AnimationClip::new(501, 583, 30.0, false);

    /// Authentic root motion per frame in units along local forward (+Z in model space).
    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 13.4;
    pub const CRAWL_ROOT_MOTION_PER_FRAME: f32 = 50.0; // 1500 units/s at 30 fps

    /// Keyframe ranges for acid spray particle spawning and combat damage window.
    pub const ACID_SPRAY_SPAWN_FRAME_START: u32 = 84;
    pub const ACID_SPRAY_SPAWN_FRAME_END: u32 = 92;
    pub const ACID_SPRAY_DAMAGE_FRAME_START: u32 = 85;
    pub const ACID_SPRAY_DAMAGE_FRAME_END: u32 = 94;
}

/// Clip definitions for Mannequin (`chr/mkn`, 17 bones, 0x893070).
pub struct MannequinAnimClips;

impl MannequinAnimClips {
    pub const STAY: AnimationClip = AnimationClip::new(0, 23, 15.0, true);
    pub const WALK: AnimationClip = AnimationClip::new(24, 54, 30.0, true);
    pub const ARM_SWING: AnimationClip = AnimationClip::new(55, 89, 30.0, false);
    pub const KICK: AnimationClip = AnimationClip::new(90, 124, 30.0, false);
    pub const FLINCH: AnimationClip = AnimationClip::new(125, 155, 30.0, false);
    pub const KNOCKDOWN: AnimationClip = AnimationClip::new(324, 470, 30.0, false);
    pub const WRITHING: AnimationClip = AnimationClip::new(646, 677, 30.0, true);

    /// Authentic root motion per frame: 19.2 units/frame (576 units/s at 30 fps).
    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 19.2;

    /// Keyframe ranges for arm swing and kick attacks.
    pub const ARM_SWING_HIT_START: u32 = 65;
    pub const ARM_SWING_HIT_END: u32 = 75;
    pub const KICK_HIT_START: u32 = 100;
    pub const KICK_HIT_END: u32 = 112;
}

/// Clip definitions for Bubble Head Nurse (`chr/nse`, 30 bones).
pub struct NurseAnimClips;

impl NurseAnimClips {
    pub const STAND: AnimationClip = AnimationClip::new(0, 17, 15.0, true);
    pub const TWITCH_WALK: AnimationClip = AnimationClip::new(18, 48, 30.0, true);
    pub const PIPE_SWING: AnimationClip = AnimationClip::new(62, 98, 30.0, false);
    pub const FLINCH: AnimationClip = AnimationClip::new(99, 125, 30.0, false);
    pub const KNOCKDOWN: AnimationClip = AnimationClip::new(210, 310, 30.0, false);

    /// Authentic root motion per frame average for twitch walk: 16.0 units/frame (480 units/s).
    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 16.0;

    /// Keyframe range for pipe swing attack strike.
    pub const PIPE_SWING_HIT_START: u32 = 74;
    pub const PIPE_SWING_HIT_END: u32 = 86;
}

/// Clip definitions for Maria companion (`chr/mar`, 36 bones, `lll_mar.anm`).
pub struct MariaAnimClips;

impl MariaAnimClips {
    pub const STAND: AnimationClip = AnimationClip::new(0, 29, 15.0, true);
    pub const WALK: AnimationClip = AnimationClip::new(30, 60, 30.0, true);
    pub const RUN: AnimationClip = AnimationClip::new(61, 85, 30.0, true);
    pub const COWER_PANIC: AnimationClip = AnimationClip::new(86, 120, 20.0, true);
    pub const FLINCH_DOWNED: AnimationClip = AnimationClip::new(121, 165, 30.0, false);

    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 10.0; // 300 units/s
    pub const RUN_ROOT_MOTION_PER_FRAME: f32 = 20.0; // 600 units/s
}

/// Clip definitions for Pyramid Head (`chr/red`, 45 bones, `red.anm`).
/// Keyframe ranges correspond 1:1 with authentic PS2/PC `RED_ANIM` (0x892fa0, `red_anim` 0x6dfc70).
pub struct PyramidHeadAnimClips;

impl PyramidHeadAnimClips {
    /// Authentic standing idle (frames 306..=323, 15 fps).
    pub const STAND: AnimationClip = AnimationClip::new(306, 323, 15.0, true);
    /// Authentic walking stride (frames 0..=29, 30 fps).
    pub const WALK: AnimationClip = AnimationClip::new(0, 29, 30.0, true);
    /// Great Knife heavy horizontal/cleave swing (frames 30..=96, 30 fps).
    pub const KNIFE_SWING: AnimationClip = AnimationClip::new(30, 96, 30.0, false);
    /// Great Knife overhead execution swing (frames 97..=216, 30 fps).
    pub const OVERHEAD_EXECUTION: AnimationClip = AnimationClip::new(97, 216, 30.0, false);
    /// One-handed neck grab lift (frames 256..=270, 15 fps).
    pub const CHOKE_LIFT: AnimationClip = AnimationClip::new(256, 270, 15.0, false);
    /// Authentic looping choke hold while James struggles (frames 271..=294, 30 fps, looping).
    pub const CHOKE_HOLD: AnimationClip = AnimationClip::new(271, 294, 30.0, true);
    /// Release after grab/struggle breakout (frames 295..=305, 30 fps).
    pub const CHOKE_RELEASE: AnimationClip = AnimationClip::new(295, 305, 30.0, false);

    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 14.3; // 429 units/s
    pub const ATTACK_HIT_START: u32 = 45;
    pub const ATTACK_HIT_END: u32 = 70;
}

/// Clip definitions for Abstract Daddy (`chr/pap`, 29 bones, `pap.anm`, 686 frames).
/// Keyframe ranges correspond 1:1 with authentic PS2/PC `PAP_ANIM` (0x892d90, `pap_anim` 0x6e0018).
pub struct AbstractDaddyAnimClips;

impl AbstractDaddyAnimClips {
    /// Authentic standing idle breathing (frames 0..=21, 15 fps).
    pub const STAND: AnimationClip = AnimationClip::new(0, 21, 15.0, true);
    /// Authentic forward walk / stalk (frames 22..=49, 30 fps).
    pub const WALK: AnimationClip = AnimationClip::new(22, 49, 30.0, true);
    /// Forward lunge attack grab (frames 50..=69, 30 fps).
    pub const LUNGE_GRAB: AnimationClip = AnimationClip::new(50, 69, 30.0, false);
    /// Authentic looping struggle hold while smothering/strangling James (frames 70..=93, 30 fps, looping).
    pub const STRUGGLE_HOLD: AnimationClip = AnimationClip::new(70, 93, 30.0, true);
    /// Breakout recoil / release after James breaks free (frames 94..=113, 30 fps).
    pub const STRUGGLE_RELEASE: AnimationClip = AnimationClip::new(94, 113, 30.0, false);
    /// Flinch from light weapon impact (frames 114..=126, 30 fps).
    pub const FLINCH: AnimationClip = AnimationClip::new(114, 126, 30.0, false);
    /// Stagger / heavy flinch (frames 127..=155, 30 fps).
    pub const HEAVY_FLINCH: AnimationClip = AnimationClip::new(127, 155, 30.0, false);
    /// Knockdown collapse onto floor (frames 294..=325, 30 fps).
    pub const KNOCKDOWN: AnimationClip = AnimationClip::new(294, 325, 30.0, false);
    /// Final death collapse (frames 604..=621, 30 fps).
    pub const DEATH: AnimationClip = AnimationClip::new(604, 621, 30.0, false);

    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 20.8; // 624 units/s
    pub const ATTACK_HIT_START: u32 = 56;
    pub const ATTACK_HIT_END: u32 = 68;
}

/// Clip definitions for Flesh Lips (`chr/ike`, 16 bones, `ike.anm`, 541 frames).
/// Keyframe ranges correspond 1:1 with authentic PS2/PC `IKE_ANIM` (0x892ce0, `ike_anim` 0x6de2a8).
pub struct FleshLipsAnimClips;

impl FleshLipsAnimClips {
    /// Authentic hanging idle with swaying legs (frames 0..=22, 15 fps).
    pub const HANG_IDLE: AnimationClip = AnimationClip::new(0, 22, 15.0, true);
    /// Ceiling rail gliding approach (frames 0..=22, 30 fps).
    pub const GLIDE_ADVANCE: AnimationClip = AnimationClip::new(0, 22, 30.0, true);
    /// Attack grab: legs descend to grab James by the throat (frames 23..=54, 30 fps).
    pub const GRAB_DESCEND: AnimationClip = AnimationClip::new(23, 54, 30.0, false);
    /// Authentic looping choke hold while straddling and strangling James (frames 87..=110, 30 fps, looping).
    pub const CHOKE_HOLD: AnimationClip = AnimationClip::new(87, 110, 30.0, true);
    /// Release after struggle breakout (frames 111..=125, 30 fps).
    pub const RELEASE: AnimationClip = AnimationClip::new(111, 125, 30.0, false);
    /// Ceiling kick thrash (frames 466..=493, 30 fps).
    pub const KICK_THRASH: AnimationClip = AnimationClip::new(466, 493, 30.0, false);
    /// Flinch from gunfire (frames 126..=144, 30 fps).
    pub const FLINCH: AnimationClip = AnimationClip::new(126, 144, 30.0, false);
    /// Falling death from ceiling rails (frames 494..=516, 30 fps).
    pub const FALL_DEATH: AnimationClip = AnimationClip::new(494, 516, 30.0, false);

    pub const SWING_ROOT_MOTION_PER_FRAME: f32 = 12.0; // 360 units/s
    pub const ATTACK_HIT_START: u32 = 28;
    pub const ATTACK_HIT_END: u32 = 32;
}

/// Clip definitions for Mandarin (`chr/arm`, 15 bones, `arm.anm`, 405 frames).
/// Keyframe ranges correspond 1:1 with authentic PS2/PC `ARM_ANIM` (0x892e20, `arm_anim` 0x6df9a0).
pub struct MandarinAnimClips;

impl MandarinAnimClips {
    /// Hanging idle below floor grates (frames 0..=48, 15 fps).
    pub const HANG_IDLE: AnimationClip = AnimationClip::new(0, 48, 15.0, true);
    /// Hand-over-hand under-grate crawl step 1 (frames 49..=65, 30 fps).
    pub const CRAWL: AnimationClip = AnimationClip::new(49, 65, 30.0, true);
    /// Tentacle strike upwards through floor grate (frames 83..=99, 30 fps).
    pub const TENTACLE_STRIKE: AnimationClip = AnimationClip::new(83, 99, 30.0, false);
    /// Flinch (frames 117..=132, 30 fps).
    pub const FLINCH: AnimationClip = AnimationClip::new(117, 132, 30.0, false);
    /// Death fall / drop (frames 273..=338, 30 fps).
    pub const KNOCKDOWN: AnimationClip = AnimationClip::new(273, 338, 30.0, false);

    pub const CRAWL_ROOT_MOTION_PER_FRAME: f32 = 19.5; // ~590 units/step
    pub const ATTACK_HIT_START: u32 = 88;
    pub const ATTACK_HIT_END: u32 = 96;
}

/// Clip definitions for Creeper cockroach swarm (`chr/tyu`, 22 bones, `tyu.anm`, 8 frames).
pub struct CreeperAnimClips;

impl CreeperAnimClips {
    pub const SCUTTLE: AnimationClip = AnimationClip::new(0, 7, 30.0, true);
    pub const BITE: AnimationClip = AnimationClip::new(0, 7, 30.0, false);

    pub const SCUTTLE_ROOT_MOTION_PER_FRAME: f32 = 40.0; // 1200 units/s
}

/// Clip definitions for Mary / Maria Final Boss (`chr/bos`, 45 bones, `bos.anm`, 228 frames).
/// Keyframe ranges correspond 1:1 with authentic PS2/PC `BOS_ANIM` (0x892e80, `bos_anim` 0x6de5b8).
pub struct MaryBossAnimClips;

impl MaryBossAnimClips {
    /// Authentic hovering idle in cage (frames 0..=58, 15 fps).
    pub const HOVER_IDLE: AnimationClip = AnimationClip::new(0, 58, 15.0, true);
    /// Forward lunge to catch James (frames 59..=73, 30 fps, PS2 aid 5802).
    pub const LUNGE_CATCH: AnimationClip = AnimationClip::new(59, 73, 30.0, false);
    /// Lift / carry James upward (frames 74..=88, 15 fps, speed 1024, PS2 aid 5803).
    pub const LIFT_CARRY: AnimationClip = AnimationClip::new(74, 88, 15.0, false);
    /// Authentic looping choke hold while strangling James (frames 89..=112, 30 fps, looping, PS2 aid 5804).
    pub const CHOKE_HOLD: AnimationClip = AnimationClip::new(89, 112, 30.0, true);
    /// Release after struggle breakout (frames 113..=136, 30 fps, PS2 aid 5805).
    pub const RELEASE: AnimationClip = AnimationClip::new(113, 136, 30.0, false);
    /// Flinch reaction (frames 137..=145, 30 fps, PS2 aid 5806).
    pub const FLINCH: AnimationClip = AnimationClip::new(137, 145, 30.0, false);
    /// Stagger reaction (frames 146..=172, 30 fps, PS2 aid 5807).
    pub const STAGGER: AnimationClip = AnimationClip::new(146, 172, 30.0, false);
    /// Death fall (frames 173..=196, 30 fps, PS2 aid 5808).
    pub const DEATH_FALL: AnimationClip = AnimationClip::new(173, 196, 30.0, false);
    /// Defeated body on floor (frames 197..=227, 30 fps, PS2 aid 5809).
    pub const FLOOR_BODY: AnimationClip = AnimationClip::new(197, 227, 30.0, true);

    pub const HOVER_ROOT_MOTION_PER_FRAME: f32 = 14.0; // 420 units/s
    pub const ATTACK_HIT_START: u32 = 64;
    pub const ATTACK_HIT_END: u32 = 72;
}

/// Generalized creature animation state synchronized with AI FSMs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreatureAnimStateKind {
    Idle,
    Locomotion,
    ProneCrawl,
    AttackPrimary,
    AttackSecondary,
    Flinch,
    Knockdown,
    Recovery,
    Cower,
}

/// Runtime creature animation controller with smooth cross-fading,
/// root motion displacement extraction, and hit/particle window triggers.
#[derive(Debug, Clone)]
pub struct CreatureAnimationController {
    /// Active animation state kind.
    state_kind: CreatureAnimStateKind,
    /// Active animation clip.
    current_clip: AnimationClip,
    /// Current playhead position in seconds within active clip.
    clip_time: f32,
    /// Previous clip during crossfades: `(state, clip, time)`.
    previous_state: Option<(CreatureAnimStateKind, AnimationClip, f32)>,
    /// Transition blend factor in `[0.0, 1.0]`.
    blend_factor: f32,
    /// Crossfade duration in seconds (default 0.15s = 150ms).
    transition_duration: f32,
    /// Root motion displacement accumulated in the most recent update tick (world units).
    root_motion_delta: Vec3,
    /// Root motion forward velocity per frame for the current clip.
    root_motion_per_frame: f32,
    /// Optional hit window bounds `(start_frame, end_frame)` for attack states.
    hit_window: Option<(u32, u32)>,
    /// Optional particle spawn window `(start_frame, end_frame)` (e.g. acid spray).
    particle_window: Option<(u32, u32)>,
}

impl CreatureAnimationController {
    /// Creates a controller with an initial clip and state.
    pub fn new(
        state_kind: CreatureAnimStateKind,
        clip: AnimationClip,
        root_motion_per_frame: f32,
    ) -> Self {
        Self {
            state_kind,
            current_clip: clip,
            clip_time: 0.0,
            previous_state: None,
            blend_factor: 1.0,
            transition_duration: 0.15,
            root_motion_delta: Vec3::ZERO,
            root_motion_per_frame,
            hit_window: None,
            particle_window: None,
        }
    }

    /// Factory for Lying Figure controller.
    pub fn new_lying_figure() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            LyingFigureAnimClips::STAND,
            0.0,
        )
    }

    /// Factory for Mannequin controller.
    pub fn new_mannequin() -> Self {
        Self::new(CreatureAnimStateKind::Idle, MannequinAnimClips::STAY, 0.0)
    }

    /// Factory for Bubble Head Nurse controller.
    pub fn new_nurse() -> Self {
        Self::new(CreatureAnimStateKind::Idle, NurseAnimClips::STAND, 0.0)
    }

    /// Factory for Maria companion controller.
    pub fn new_maria() -> Self {
        Self::new(CreatureAnimStateKind::Idle, MariaAnimClips::STAND, 0.0)
    }

    /// Factory for Pyramid Head controller.
    pub fn new_pyramid_head() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            PyramidHeadAnimClips::STAND,
            0.0,
        )
    }

    /// Factory for Abstract Daddy controller.
    pub fn new_abstract_daddy() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            AbstractDaddyAnimClips::STAND,
            0.0,
        )
    }

    /// Factory for Flesh Lips controller.
    pub fn new_flesh_lips() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            FleshLipsAnimClips::HANG_IDLE,
            0.0,
        )
    }

    /// Factory for Mandarin controller.
    pub fn new_mandarin() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            MandarinAnimClips::HANG_IDLE,
            0.0,
        )
    }

    /// Factory for Creeper controller.
    pub fn new_creeper() -> Self {
        Self::new(
            CreatureAnimStateKind::Locomotion,
            CreeperAnimClips::SCUTTLE,
            CreeperAnimClips::SCUTTLE_ROOT_MOTION_PER_FRAME,
        )
    }

    /// Factory for Mary Boss controller.
    pub fn new_mary_boss() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            MaryBossAnimClips::HOVER_IDLE,
            0.0,
        )
    }

    pub fn state(&self) -> CreatureAnimStateKind {
        self.state_kind
    }

    pub fn current_clip(&self) -> AnimationClip {
        self.current_clip
    }

    pub fn clip_time(&self) -> f32 {
        self.clip_time
    }

    pub fn blend_factor(&self) -> f32 {
        self.blend_factor
    }

    pub fn root_motion_delta(&self) -> Vec3 {
        self.root_motion_delta
    }

    pub fn set_transition_duration(&mut self, duration_seconds: f32) {
        self.transition_duration = duration_seconds.max(0.001);
    }

    /// Crossfade transition to a new animation state and clip.
    pub fn transition_to(
        &mut self,
        new_state: CreatureAnimStateKind,
        new_clip: AnimationClip,
        root_motion_per_frame: f32,
        hit_window: Option<(u32, u32)>,
        particle_window: Option<(u32, u32)>,
    ) {
        if self.state_kind == new_state && self.current_clip == new_clip {
            return;
        }

        self.previous_state = Some((self.state_kind, self.current_clip, self.clip_time));
        self.state_kind = new_state;
        self.current_clip = new_clip;
        self.clip_time = 0.0;
        self.blend_factor = 0.0;
        self.root_motion_per_frame = root_motion_per_frame;
        self.hit_window = hit_window;
        self.particle_window = particle_window;
    }

    /// Advance the animation by `dt` seconds and calculate root motion displacement along `yaw`.
    pub fn update(&mut self, dt: f32, yaw: f32) {
        // 1. Advance crossfade blend factor
        if self.blend_factor < 1.0 {
            self.blend_factor = (self.blend_factor + dt / self.transition_duration).min(1.0);
            if self.blend_factor >= 1.0 {
                self.previous_state = None;
            }
        }

        // 2. Advance active clip time
        let prev_clip_time = self.clip_time;
        let clip_dur = self.current_clip.duration_seconds();
        if clip_dur > 0.0 {
            self.clip_time += dt;
            if self.current_clip.looping {
                self.clip_time %= clip_dur;
            } else if self.clip_time >= clip_dur {
                self.clip_time = clip_dur;
            }
        }

        // 3. Advance previous clip time during blend to prevent frozen poses
        if let Some((_, prev_clip, ref mut prev_time)) = self.previous_state {
            let prev_dur = prev_clip.duration_seconds();
            if prev_dur > 0.0 {
                *prev_time += dt;
                if prev_clip.looping {
                    *prev_time %= prev_dur;
                } else if *prev_time >= prev_dur {
                    *prev_time = prev_dur;
                }
            }
        }

        // 4. Calculate root motion translation delta for this tick
        if self.root_motion_per_frame > 0.0 && self.current_clip.fps > 0.0 {
            let frames_elapsed = (self.clip_time - prev_clip_time).max(0.0) * self.current_clip.fps;
            // When looping wraps around, use dt * fps
            let effective_frames = if self.current_clip.looping && self.clip_time < prev_clip_time {
                dt * self.current_clip.fps
            } else {
                frames_elapsed
            };

            let dist = effective_frames * self.root_motion_per_frame;
            let (sy, cy) = yaw.sin_cos();
            self.root_motion_delta = Vec3::new(sy * dist, 0.0, cy * dist);
        } else {
            self.root_motion_delta = Vec3::ZERO;
        }
    }

    /// Evaluates current sampling frame cursor `(frame_0, frame_1, alpha)`.
    pub fn evaluate_sample_cursor(&self) -> FrameCursor {
        Self::calc_cursor(&self.current_clip, self.clip_time)
    }

    /// Evaluates blended frame cursor `(previous_cursor, current_cursor, blend_factor)`.
    pub fn evaluate_blended_cursor(&self) -> BlendedFrameCursor {
        let cur = self.evaluate_sample_cursor();
        let prev = self
            .previous_state
            .as_ref()
            .map(|(_, prev_clip, prev_time)| Self::calc_cursor(prev_clip, *prev_time));
        (prev, cur, self.blend_factor)
    }

    /// Current integer frame index in playback.
    pub fn current_frame_index(&self) -> u32 {
        let (f0, _, _) = self.evaluate_sample_cursor();
        f0
    }

    /// Checks if the current playback frame is within the active attack hit window.
    pub fn is_in_hit_window(&self) -> bool {
        if let Some((start, end)) = self.hit_window {
            let f = self.current_frame_index();
            f >= start && f <= end
        } else {
            false
        }
    }

    /// Checks if the current playback frame is within the active particle spawn window.
    pub fn is_in_particle_window(&self) -> bool {
        if let Some((start, end)) = self.particle_window {
            let f = self.current_frame_index();
            f >= start && f <= end
        } else {
            false
        }
    }

    /// Checks if a non-looping animation clip has finished playing its full duration.
    pub fn is_clip_finished(&self) -> bool {
        if self.current_clip.looping {
            false
        } else {
            let dur = self.current_clip.duration_seconds();
            dur > 0.0 && self.clip_time >= dur - 0.001
        }
    }

    /// Synchronize controller state from Lying Figure FSM state (`ScuState`).
    pub fn sync_lying_figure(&mut self, state: ScuState) {
        match state {
            ScuState::Wander | ScuState::Precaution | ScuState::Backward => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    LyingFigureAnimClips::WALK,
                    LyingFigureAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            ScuState::Chase => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    LyingFigureAnimClips::WALK,
                    LyingFigureAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            ScuState::Crawl => {
                self.transition_to(
                    CreatureAnimStateKind::ProneCrawl,
                    LyingFigureAnimClips::PRONE_CRAWL,
                    LyingFigureAnimClips::CRAWL_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            ScuState::Attack => {
                self.transition_to(
                    CreatureAnimStateKind::AttackPrimary,
                    LyingFigureAnimClips::ACID_SPRAY,
                    0.0,
                    Some((
                        LyingFigureAnimClips::ACID_SPRAY_DAMAGE_FRAME_START,
                        LyingFigureAnimClips::ACID_SPRAY_DAMAGE_FRAME_END,
                    )),
                    Some((
                        LyingFigureAnimClips::ACID_SPRAY_SPAWN_FRAME_START,
                        LyingFigureAnimClips::ACID_SPRAY_SPAWN_FRAME_END,
                    )),
                );
            }
            ScuState::Damage => {
                self.transition_to(
                    CreatureAnimStateKind::Flinch,
                    LyingFigureAnimClips::FLINCH,
                    0.0,
                    None,
                    None,
                );
            }
            ScuState::Down => {
                self.transition_to(
                    CreatureAnimStateKind::Knockdown,
                    LyingFigureAnimClips::KNOCKDOWN,
                    0.0,
                    None,
                    None,
                );
            }
            ScuState::Dying => {
                self.transition_to(
                    CreatureAnimStateKind::Knockdown,
                    LyingFigureAnimClips::KNOCKDOWN,
                    0.0,
                    None,
                    None,
                );
            }
            ScuState::Ambush | ScuState::WaitCar | ScuState::Confuse => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    LyingFigureAnimClips::STAND,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize controller state from Mannequin FSM state (`MknState`).
    pub fn sync_mannequin(&mut self, state: MknState, is_kick_next: bool) {
        match state {
            MknState::Stay => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    MannequinAnimClips::STAY,
                    0.0,
                    None,
                    None,
                );
            }
            MknState::Wander | MknState::Precaution | MknState::Chase | MknState::Circle => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    MannequinAnimClips::WALK,
                    MannequinAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            MknState::Attack => {
                if is_kick_next {
                    self.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        MannequinAnimClips::KICK,
                        0.0,
                        Some((
                            MannequinAnimClips::KICK_HIT_START,
                            MannequinAnimClips::KICK_HIT_END,
                        )),
                        None,
                    );
                } else {
                    self.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        MannequinAnimClips::ARM_SWING,
                        0.0,
                        Some((
                            MannequinAnimClips::ARM_SWING_HIT_START,
                            MannequinAnimClips::ARM_SWING_HIT_END,
                        )),
                        None,
                    );
                }
            }
            MknState::Damage => {
                self.transition_to(
                    CreatureAnimStateKind::Flinch,
                    MannequinAnimClips::FLINCH,
                    0.0,
                    None,
                    None,
                );
            }
            MknState::Down => {
                self.transition_to(
                    CreatureAnimStateKind::Knockdown,
                    MannequinAnimClips::KNOCKDOWN,
                    0.0,
                    None,
                    None,
                );
            }
            MknState::Dying => {
                self.transition_to(
                    CreatureAnimStateKind::Knockdown,
                    MannequinAnimClips::KNOCKDOWN,
                    0.0,
                    None,
                    None,
                );
            }
            MknState::Event | MknState::Fall | MknState::Fall2 | MknState::Confuse => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    MannequinAnimClips::STAY,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize controller state from Bubble Head Nurse FSM state (`NseState`).
    pub fn sync_nurse(&mut self, state: NseState) {
        match state {
            NseState::Wander | NseState::Precaution | NseState::Chase => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    NurseAnimClips::TWITCH_WALK,
                    NurseAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            NseState::Attack => {
                self.transition_to(
                    CreatureAnimStateKind::AttackPrimary,
                    NurseAnimClips::PIPE_SWING,
                    0.0,
                    Some((
                        NurseAnimClips::PIPE_SWING_HIT_START,
                        NurseAnimClips::PIPE_SWING_HIT_END,
                    )),
                    None,
                );
            }
            NseState::Damage => {
                self.transition_to(
                    CreatureAnimStateKind::Flinch,
                    NurseAnimClips::FLINCH,
                    0.0,
                    None,
                    None,
                );
            }
            NseState::Down | NseState::Dying => {
                self.transition_to(
                    CreatureAnimStateKind::Knockdown,
                    NurseAnimClips::KNOCKDOWN,
                    0.0,
                    None,
                    None,
                );
            }
            NseState::Confuse => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    NurseAnimClips::STAND,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize controller state from Maria companion state (`MariaState`).
    pub fn sync_maria(&mut self, state: MariaState) {
        match state {
            MariaState::Stand => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    MariaAnimClips::STAND,
                    0.0,
                    None,
                    None,
                );
            }
            MariaState::Walk => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    MariaAnimClips::WALK,
                    MariaAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            MariaState::Run => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    MariaAnimClips::RUN,
                    MariaAnimClips::RUN_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            MariaState::Panic => {
                self.transition_to(
                    CreatureAnimStateKind::Cower,
                    MariaAnimClips::COWER_PANIC,
                    0.0,
                    None,
                    None,
                );
            }
            MariaState::Downed | MariaState::Dead => {
                self.transition_to(
                    CreatureAnimStateKind::Knockdown,
                    MariaAnimClips::FLINCH_DOWNED,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize controller state from Pyramid Head FSM state (`RedState`).
    pub fn sync_pyramid_head(&mut self, state: crate::ai::creatures::pyramid_head::RedState) {
        use crate::ai::creatures::pyramid_head::RedState;
        match state {
            RedState::Stand | RedState::Confuse | RedState::Leave => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    PyramidHeadAnimClips::STAND,
                    0.0,
                    None,
                    None,
                );
            }
            RedState::Wander | RedState::Chase | RedState::Hunt | RedState::Stairs => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    PyramidHeadAnimClips::WALK,
                    PyramidHeadAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            RedState::Attack => {
                self.transition_to(
                    CreatureAnimStateKind::AttackPrimary,
                    PyramidHeadAnimClips::KNIFE_SWING,
                    0.0,
                    Some((
                        PyramidHeadAnimClips::ATTACK_HIT_START,
                        PyramidHeadAnimClips::ATTACK_HIT_END,
                    )),
                    None,
                );
            }
            RedState::Grab => {
                self.transition_to(
                    CreatureAnimStateKind::AttackSecondary,
                    PyramidHeadAnimClips::CHOKE_HOLD,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Helper for calculating sampling frame cursor given clip and current time.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lying_figure_animation_bounds_and_sync() {
        let mut anim = CreatureAnimationController::new_lying_figure();
        assert_eq!(anim.state(), CreatureAnimStateKind::Idle);
        assert_eq!(anim.current_clip().start_frame, 0);
        assert_eq!(anim.current_clip().end_frame, 14);

        // Sync to Walk
        anim.sync_lying_figure(ScuState::Chase);
        assert_eq!(anim.state(), CreatureAnimStateKind::Locomotion);
        assert_eq!(anim.current_clip().start_frame, 15);
        assert_eq!(anim.current_clip().end_frame, 45);

        // Advance 0.5s walking at yaw 0 (forward along Z)
        anim.update(0.5, 0.0);
        let cursor = anim.evaluate_sample_cursor();
        assert!(cursor.0 >= 15 && cursor.0 <= 45);
        assert!(anim.root_motion_delta().z > 0.0);

        // Sync to Acid Spray Attack
        anim.sync_lying_figure(ScuState::Attack);
        assert_eq!(anim.state(), CreatureAnimStateKind::AttackPrimary);
        assert_eq!(anim.current_clip().start_frame, 77);
        assert_eq!(anim.current_clip().end_frame, 100);

        // Advance to particle/damage window (frame 85: approx 8/30s = 0.27s)
        anim.update(0.28, 0.0);
        let frame = anim.current_frame_index();
        assert!((84..=95).contains(&frame));
        assert!(anim.is_in_hit_window() || anim.is_in_particle_window());
    }

    #[test]
    fn test_mannequin_attack_and_kick_sync() {
        let mut anim = CreatureAnimationController::new_mannequin();
        assert_eq!(anim.state(), CreatureAnimStateKind::Idle);

        // Sync to Arm Swing Attack (is_kick_next = false)
        anim.sync_mannequin(MknState::Attack, false);
        assert_eq!(anim.state(), CreatureAnimStateKind::AttackPrimary);
        assert_eq!(anim.current_clip().start_frame, 55);
        assert_eq!(anim.current_clip().end_frame, 89);

        // Advance into arm swing hit window (frame 65..=75)
        // 65 is 10 frames into clip -> 10/30 = 0.333s
        anim.update(0.35, 0.0);
        assert!(anim.is_in_hit_window());

        // Sync to Kick Attack (is_kick_next = true)
        anim.sync_mannequin(MknState::Attack, true);
        assert_eq!(anim.state(), CreatureAnimStateKind::AttackSecondary);
        assert_eq!(anim.current_clip().start_frame, 90);
        assert_eq!(anim.current_clip().end_frame, 124);

        // Advance into kick hit window (frame 100..=112)
        // 100 is 10 frames into clip -> 0.35s
        anim.update(0.35, 0.0);
        assert!(anim.is_in_hit_window());
    }

    #[test]
    fn test_crossfade_blending() {
        let mut anim = CreatureAnimationController::new_nurse();
        assert_eq!(anim.blend_factor(), 1.0);

        // Transition from Idle to Twitch Walk
        anim.sync_nurse(NseState::Chase);
        assert_eq!(anim.blend_factor(), 0.0);

        let (prev, cur, blend) = anim.evaluate_blended_cursor();
        assert!(prev.is_some());
        assert_eq!(blend, 0.0);
        assert_eq!(cur.0, 18); // Start frame of nurse walk

        // Step halfway (75ms of 150ms)
        anim.update(0.075, 0.0);
        assert!((anim.blend_factor() - 0.5).abs() < 0.05);

        // Step past transition
        anim.update(0.1, 0.0);
        assert_eq!(anim.blend_factor(), 1.0);
        let (prev, _, blend) = anim.evaluate_blended_cursor();
        assert!(prev.is_none());
        assert_eq!(blend, 1.0);
    }
}
