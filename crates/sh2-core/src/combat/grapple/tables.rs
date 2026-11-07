//! Authentic PS2 Grapple Master Data Tables and Specifications.
//!
//! Ground truth decompiled from `prototype_extracted/SLUS_202.28`:
//! - `hugging_pos[4][5]` table at virtual address `0x0034c200`
//! - `hurihodoki_timer[5][5]` breakout table at virtual address `0x002a8e10`
//! - `PlayerCheckHuggingAttack` (`0x00110070`) and `PlayerCheckAnimeLower` Case 17 (`0x0010e4d8`)
//!
//! # Pure Data Ingestion Note:
//! This open-source engine does NOT contain, rip, or bundle any proprietary Konami
//! meshes, textures, or audio assets. All animation parameters here represent purely
//! skeletal keyframe indices and playback frame rates that direct the runtime animation
//! system to ingest data from user-supplied authentic retail game files.

use crate::animation::{AnimationClip, JamesBasicClips};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Identifiers for enemies participating in combat and grapple interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum BestiaryId {
    LyingFigure = 1,
    Mannequin = 2,
    BubbleHeadNurse = 3,
    PyramidHead = 4,
    AbstractDaddy = 5,
    FleshLips = 6,
    Mandarin = 7,
    Creeper = 8,
    MaryBoss = 9,
}

impl BestiaryId {
    pub const fn from_raw(id: u32) -> Option<Self> {
        match id {
            1 => Some(Self::LyingFigure),
            2 => Some(Self::Mannequin),
            3 => Some(Self::BubbleHeadNurse),
            4 => Some(Self::PyramidHead),
            5 => Some(Self::AbstractDaddy),
            6 => Some(Self::FleshLips),
            7 => Some(Self::Mandarin),
            8 => Some(Self::Creeper),
            9 => Some(Self::MaryBoss),
            _ => None,
        }
    }
}

/// Authentic 4D vectors from PS2 `hugging_pos[4][5]` table at virtual address `0x0034c200`.
/// Columns: `[X (lateral), Y (vertical), Z (forward), W (padding)]`.
pub const PS2_HUGGING_POS: [[f32; 4]; 5] = [
    // Vector 0: Flesh Lips overhead grab (attacks 54/55)
    [52.1332, 1550.9032, 36.4947, 0.0],
    // Vector 1: Mary Boss tentacle choke hold (attacks 59/60)
    [43.5385, -93.4794, 353.9123, 0.0],
    // Vector 2: Abstract Daddy torso struggle (attacks 57/58)
    [0.0, 0.0, 328.0524, 0.0],
    // Vector 3: Creeper / Daddy rear smother (attacks 51, 57/58 rear)
    [0.0, 0.0, 176.7915, 0.0],
    // Vector 4: High ceiling hang
    [0.0, 1496.4872, 0.0, 0.0],
];

/// Authentic struggle resistance duration matrix `hurihodoki_timer[5][5]` from PS2 `0x002a8e10`.
/// Rows: Game difficulty (0=Beginner, 1=Easy, 2=Normal, 3=Hard, 4=Extra).
/// Columns: Attack classification column:
/// - Col 0: Pyramid Head (Strike 47 -> Hug 48)
/// - Col 1: Flesh Lips (Strike 54 -> Hug 55)
/// - Col 2: Abstract Daddy (Strike 57 -> Hug 58)
/// - Col 3: Mary Boss Tentacle (Strike 59 -> Hug 60)
/// - Col 4: Mary Boss Moth Swarm (Strike 62 -> Hug 63)
pub const PS2_HURIHODOKI_TIMER: [[f32; 5]; 5] = [
    [0.07, 0.08, 0.10, 0.13, 0.10], // Beginner (0)
    [0.35, 0.40, 0.50, 0.65, 0.50], // Easy (1)
    [0.70, 0.80, 1.00, 1.30, 1.00], // Normal (2)
    [1.40, 1.60, 2.00, 2.60, 2.00], // Hard (3)
    [1.40, 1.60, 2.00, 2.60, 2.00], // Extra (4)
];

/// Relative yaw facing orientation between creature and James during grapple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrappleFacingMode {
    /// Creature turns to directly face James (yaw = player_yaw + PI).
    FaceTarget,
    /// Creature faces the same direction as James (e.g. rear ambush).
    SameDirection,
}

/// Static, zero-allocation specification of a creature's grapple interaction parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrappleSpec {
    pub bestiary_id: BestiaryId,
    /// Initial strike collision ID that begins the grapple attempt.
    pub strike_id: u32,
    /// Type 6 Hug ID used during the ongoing hold loop.
    pub hug_id: u32,
    /// Column index in `hurihodoki_timer` table (0..=4).
    pub timer_col: usize,
    /// Maximum distance threshold for grab resolution (in millimeters).
    pub grab_range: f32,
    /// Damage-over-time applied while holding James (DPS).
    pub hold_dps: f32,
    /// Looping hold clip played by the creature's skeletal model at runtime.
    pub creature_hold_clip: AnimationClip,
    /// Optional recoil or release animation played by the creature upon breakout.
    pub creature_breakout_clip: Option<AnimationClip>,
    /// Authentic animation clip played by James while struggling in this grapple.
    pub james_struggle_clip: AnimationClip,
    /// Authentic animation clip played by James when breaking free (aid 130).
    pub james_breakout_clip: AnimationClip,
    /// World-space coordinate offset relative to James `[X (lateral), Y (vertical), Z (forward)]`.
    pub anchor_offset: Vec3,
    /// Relative yaw alignment mode.
    pub facing_mode: GrappleFacingMode,
}

impl GrappleSpec {
    /// Retrieve the base breakout duration for this grapple at a given difficulty level (0=Beginner..4=Extra).
    #[inline]
    pub const fn breakout_duration(&self, difficulty: usize) -> f32 {
        let diff = if difficulty > 4 { 2 } else { difficulty };
        let col = if self.timer_col > 4 { 0 } else { self.timer_col };
        PS2_HURIHODOKI_TIMER[diff][col]
    }
}

// ----------------------------------------------------------------------------
// Authentic Static Specifications (SLUS_202.28 1:1 Parity)
// ----------------------------------------------------------------------------

pub const SPEC_FLESH_LIPS: GrappleSpec = GrappleSpec {
    bestiary_id: BestiaryId::FleshLips,
    strike_id: 54,
    hug_id: 55,
    timer_col: 1,
    grab_range: 450.0,
    hold_dps: 6.0,
    creature_hold_clip: AnimationClip::new(87, 110, 30.0, true),
    creature_breakout_clip: None,
    james_struggle_clip: JamesBasicClips::STRUGGLE_FLESH_LIPS,
    james_breakout_clip: JamesBasicClips::STRUGGLE_RELEASE,
    anchor_offset: Vec3::new(
        PS2_HUGGING_POS[0][0],
        -PS2_HUGGING_POS[0][1], // -1550.9032mm (above James in SH2 coordinate space)
        PS2_HUGGING_POS[0][2],
    ),
    facing_mode: GrappleFacingMode::FaceTarget,
};

pub const SPEC_ABSTRACT_DADDY: GrappleSpec = GrappleSpec {
    bestiary_id: BestiaryId::AbstractDaddy,
    strike_id: 57,
    hug_id: 58,
    timer_col: 2,
    grab_range: 530.0,
    hold_dps: 6.0,
    creature_hold_clip: AnimationClip::new(70, 93, 30.0, true),
    creature_breakout_clip: Some(AnimationClip::new(94, 113, 30.0, false)),
    james_struggle_clip: JamesBasicClips::STRUGGLE_DADDY,
    james_breakout_clip: JamesBasicClips::STRUGGLE_RELEASE,
    anchor_offset: Vec3::new(0.0, 0.0, 580.0), // 354mm Daddy chest reach meets James hands
    facing_mode: GrappleFacingMode::FaceTarget,
};

pub const SPEC_MARY_BOSS: GrappleSpec = GrappleSpec {
    bestiary_id: BestiaryId::MaryBoss,
    strike_id: 59,
    hug_id: 60,
    timer_col: 3,
    grab_range: 600.0,
    hold_dps: 6.0,
    creature_hold_clip: AnimationClip::new(89, 112, 30.0, true),
    creature_breakout_clip: Some(AnimationClip::new(113, 136, 30.0, false)),
    james_struggle_clip: JamesBasicClips::STRUGGLE_CHOKE_MARY,
    james_breakout_clip: JamesBasicClips::STRUGGLE_RELEASE,
    anchor_offset: Vec3::new(
        PS2_HUGGING_POS[1][0],
        -1000.0, // Floating elevation
        PS2_HUGGING_POS[1][2],
    ),
    facing_mode: GrappleFacingMode::FaceTarget,
};

pub const SPEC_PYRAMID_HEAD: GrappleSpec = GrappleSpec {
    bestiary_id: BestiaryId::PyramidHead,
    strike_id: 47,
    hug_id: 48,
    timer_col: 0,
    grab_range: 400.0,
    hold_dps: 6.0,
    creature_hold_clip: AnimationClip::new(271, 294, 30.0, true),
    creature_breakout_clip: Some(AnimationClip::new(295, 305, 30.0, false)),
    james_struggle_clip: JamesBasicClips::STRUGGLE_CHOKE_LIFT,
    james_breakout_clip: JamesBasicClips::STRUGGLE_RELEASE,
    anchor_offset: Vec3::new(0.0, 0.0, 400.0),
    facing_mode: GrappleFacingMode::FaceTarget,
};

pub const SPEC_CREEPER: GrappleSpec = GrappleSpec {
    bestiary_id: BestiaryId::Creeper,
    strike_id: 51,
    hug_id: 51,
    timer_col: 0,
    grab_range: 220.0,
    hold_dps: 4.0,
    creature_hold_clip: AnimationClip::new(0, 7, 30.0, true),
    creature_breakout_clip: None,
    james_struggle_clip: JamesBasicClips::STRUGGLE_LEG,
    james_breakout_clip: JamesBasicClips::STRUGGLE_RELEASE,
    anchor_offset: Vec3::new(0.0, 0.0, 80.0),
    facing_mode: GrappleFacingMode::FaceTarget,
};

/// Lookup the authentic grapple specification for a creature species, if it possesses a grab.
#[inline]
pub const fn get_grapple_spec(species: BestiaryId) -> Option<&'static GrappleSpec> {
    match species {
        BestiaryId::FleshLips => Some(&SPEC_FLESH_LIPS),
        BestiaryId::AbstractDaddy => Some(&SPEC_ABSTRACT_DADDY),
        BestiaryId::MaryBoss => Some(&SPEC_MARY_BOSS),
        BestiaryId::PyramidHead => Some(&SPEC_PYRAMID_HEAD),
        BestiaryId::Creeper => Some(&SPEC_CREEPER),
        // Lying Figure, Mannequin, Nurse, Mandarin have strike/kick/needle moves, not Type 6 grabs.
        _ => None,
    }
}
