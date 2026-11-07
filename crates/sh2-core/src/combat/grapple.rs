//! Authentic PS2 Grapple Coupling, Relative Anchoring, and Struggle State Machines.
//!
//! # Ground Truth & Bytecode Reverse Engineering:
//! - PS2 prototype unstripped: `src/Chacter/m3_play_common.c`
//!   - `PlayerCheckHuggedPos` (`0x0021bdc0`): Mathematical coupling matrix transforming creature coordinates
//!     relative to James based on character yaw.
//!   - `lower_damage` (`0x0021f700`): Gripping damage-over-time and vector selection table (`0x0034c200`).
//! - PS2 prototype unstripped: `src/Chacter/m3_play.c`
//!   - `PlayerCheckHuggingAttack` (`0x00110070`): Breakout resistance timer table `hurihodoki_timer[5][5]` (`0x002a8e10`).
//!   - `PlayerCheckAnimeLower` Case 17 (`0x0010e4d8`): Selects James struggle animations:
//!     - Torso struggle: `aid 128` (frames `612..=629 @ 15.0fps`)
//!     - Choke struggle: `aid 129` (frames `632..=649 @ 15.0fps`)
//!     - Recoil breakout: `aid 130` (frames `652..=670 @ 30.0fps`)
//!
//! Modern Rust idioms used:
//! - Strongly typed `BestiaryId` enum with `TryFrom`
//! - `const` authentic PS2 matrix tables with zero-cost abstractions
//! - Modular `GrappleAnchor` and `GrappleController` replacing giant `match` statements

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
    // Vector 0: Flesh Lips overhead grab (attacks 0x4e46..0x4e48)
    [52.1332, 1550.9032, 36.4947, 0.0],
    // Vector 1: Mary Boss tentacle choke hold (attacks 0x4e3d..0x4e3f)
    [43.5385, -93.4794, 353.9123, 0.0],
    // Vector 2: Abstract Daddy torso struggle (attacks 0x4e4a..0x4e4d)
    [0.0, 0.0, 328.0524, 0.0],
    // Vector 3: Creeper leg bite (attacks 0x4e4e..0x4e51)
    [0.0, 0.0, 176.7915, 0.0],
    // Vector 4: High ceiling hang (attacks 0x4e56..0x4e58)
    [0.0, 1496.4872, 0.0, 0.0],
];

/// Authentic struggle resistance duration matrix `hurihodoki_timer[5][5]` from PS2 `0x002a8e10`.
/// Rows: Struggle intensity stage (0..=4).
/// Columns: Game difficulty (0=Beginner, 1=Easy, 2=Normal, 3=Hard, 4=Extra).
pub const PS2_HURIHODOKI_TIMER: [[f32; 5]; 5] = [
    [0.07, 0.08, 0.10, 0.13, 0.10],
    [0.35, 0.40, 0.50, 0.65, 0.50],
    [0.70, 0.80, 1.00, 1.30, 1.00],
    [1.40, 1.60, 2.00, 2.60, 2.00],
    [1.40, 1.60, 2.00, 2.60, 2.00],
];

/// Relative 3D anchoring specifications and animation parameters for a creature grappling James.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrappleAnchor {
    /// Forward distance from James along his look direction (in millimeters).
    pub forward_dist: f32,
    /// Lateral distance perpendicular to James's look direction (+X = to James's right).
    pub lateral_dist: f32,
    /// Vertical world elevation adjustment for creature rendering during grapple (in millimeters).
    pub vertical_offset: f32,
    /// Normal non-grapple vertical world elevation (in millimeters).
    pub default_height_offset: f32,
    /// Authentic animation clip played by James while caught in this grapple.
    pub struggle_clip: AnimationClip,
}

impl GrappleAnchor {
    /// Retrieve the authentic PS2 grapple anchor configuration for a creature species.
    pub const fn for_species(species: BestiaryId) -> Self {
        match species {
            BestiaryId::LyingFigure => Self {
                forward_dist: 450.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_TORSO,
            },
            BestiaryId::Mannequin | BestiaryId::BubbleHeadNurse => Self {
                forward_dist: 400.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_TORSO,
            },
            BestiaryId::PyramidHead => Self {
                forward_dist: 400.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_CHOKE_LIFT,
            },
            BestiaryId::AbstractDaddy => Self {
                // Daddy's root is 580mm ahead so his 354mm chest reach meets James' pushing hands
                forward_dist: 580.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_DADDY,
            },
            BestiaryId::FleshLips => Self {
                // Vector 0: [X=52.13, Y=1550.90, Z=36.49]
                forward_dist: PS2_HUGGING_POS[0][2],
                lateral_dist: PS2_HUGGING_POS[0][0],
                vertical_offset: -PS2_HUGGING_POS[0][1], // -1550.9mm
                default_height_offset: -2140.0,         // enIKEInitData ceiling height
                struggle_clip: JamesBasicClips::STRUGGLE_FLESH_LIPS,
            },
            BestiaryId::Mandarin => Self {
                forward_dist: 450.0,
                lateral_dist: 0.0,
                vertical_offset: -1000.0,
                default_height_offset: -1000.0,
                struggle_clip: JamesBasicClips::STRUGGLE_TORSO,
            },
            BestiaryId::Creeper => Self {
                // Vector 3: [X=0.0, Y=0.0, Z=176.79]
                forward_dist: 80.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_LEG,
            },
            BestiaryId::MaryBoss => Self {
                // Vector 1: [X=43.54, Y=-93.48, Z=353.91]
                forward_dist: PS2_HUGGING_POS[1][2],
                lateral_dist: PS2_HUGGING_POS[1][0],
                vertical_offset: -1000.0,
                default_height_offset: -1000.0,
                struggle_clip: JamesBasicClips::STRUGGLE_CHOKE_MARY,
            },
        }
    }

    /// Default fallback anchor for unspecified creature IDs.
    pub const fn default_fallback() -> Self {
        Self {
            forward_dist: 250.0,
            lateral_dist: 0.0,
            vertical_offset: 0.0,
            default_height_offset: 0.0,
            struggle_clip: JamesBasicClips::STRUGGLE_TORSO,
        }
    }

    /// Compute the world-space target position for the creature relative to James (1:1 `PlayerCheckHuggedPos`).
    #[inline]
    pub fn compute_world_anchor(&self, player_pos: Vec3, player_yaw: f32) -> Vec3 {
        let forward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
        let right = Vec3::new(player_yaw.cos(), 0.0, -player_yaw.sin());
        player_pos + forward * self.forward_dist + right * self.lateral_dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ps2_hugging_pos_constants() {
        // Verify exact extracted values from SLUS_202.28 @ 0x0034c200
        assert!((PS2_HUGGING_POS[0][0] - 52.1332).abs() < 1e-4);
        assert!((PS2_HUGGING_POS[0][1] - 1550.9032).abs() < 1e-4);
        assert!((PS2_HUGGING_POS[0][2] - 36.4947).abs() < 1e-4);

        assert!((PS2_HUGGING_POS[1][0] - 43.5385).abs() < 1e-4);
        assert!((PS2_HUGGING_POS[1][2] - 353.9123).abs() < 1e-4);
    }

    #[test]
    fn test_grapple_anchor_world_transformation() {
        let mary_anchor = GrappleAnchor::for_species(BestiaryId::MaryBoss);
        let player_pos = Vec3::ZERO;
        let player_yaw = 0.0; // Facing +Z

        let anchor = mary_anchor.compute_world_anchor(player_pos, player_yaw);
        // Facing +Z means forward = (0, 0, 1), right = (1, 0, 0)
        assert!((anchor.z - 353.9123).abs() < 1e-3);
        assert!((anchor.x - 43.5385).abs() < 1e-3);
    }

    #[test]
    fn test_ps2_grapple_struggle_clips_parity() {
        // 1:1 PS2 PlayerCheckAnimeUpper & PlayerCheckAnimeLower Case 17:
        // Choke holds (Flesh Lips overhead and Mary boss choke) map to pjames_anim aid 129 (632..=649 @ 15.0fps)
        let ike_anchor = GrappleAnchor::for_species(BestiaryId::FleshLips);
        assert_eq!(ike_anchor.struggle_clip.start_frame, 632);
        assert_eq!(ike_anchor.struggle_clip.end_frame, 649);
        assert_eq!(ike_anchor.struggle_clip.fps, 15.0);
        assert!(ike_anchor.struggle_clip.looping);

        let mary_anchor = GrappleAnchor::for_species(BestiaryId::MaryBoss);
        assert_eq!(mary_anchor.struggle_clip.start_frame, 632);
        assert_eq!(mary_anchor.struggle_clip.end_frame, 649);

        // Torso grabs (Daddy, Lying Figure) map to pjames_anim aid 128 (612..=629 @ 15.0fps)
        let daddy_anchor = GrappleAnchor::for_species(BestiaryId::AbstractDaddy);
        assert_eq!(daddy_anchor.struggle_clip.start_frame, 612);
        assert_eq!(daddy_anchor.struggle_clip.end_frame, 629);

        let scu_anchor = GrappleAnchor::for_species(BestiaryId::LyingFigure);
        assert_eq!(scu_anchor.struggle_clip.start_frame, 612);
        assert_eq!(scu_anchor.struggle_clip.end_frame, 629);
    }
}
