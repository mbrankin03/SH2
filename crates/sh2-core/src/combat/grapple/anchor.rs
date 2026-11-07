//! Pure Spatial Kinematics and Anchor Positioning for Silent Hill 2 Grapples.
//!
//! Ground truth decompiled from `prototype_extracted/SLUS_202.28`:
//! - `PlayerCheckHuggedPos` (`0x0021bdc0`): Evaluates 3D coordinates relative to James's look vector
//!   and character yaw, anchoring the creature to James (or James to the creature).

use super::tables::{get_grapple_spec, BestiaryId, GrappleFacingMode, PS2_HUGGING_POS};
use crate::animation::{AnimationClip, JamesBasicClips};
use glam::Vec3;

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
                forward_dist: 580.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_DADDY,
            },
            BestiaryId::FleshLips => Self {
                forward_dist: PS2_HUGGING_POS[0][2],
                lateral_dist: PS2_HUGGING_POS[0][0],
                vertical_offset: -PS2_HUGGING_POS[0][1], // -1550.9mm
                default_height_offset: -2140.0,
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
                forward_dist: 80.0,
                lateral_dist: 0.0,
                vertical_offset: 0.0,
                default_height_offset: 0.0,
                struggle_clip: JamesBasicClips::STRUGGLE_LEG,
            },
            BestiaryId::MaryBoss => Self {
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

/// Compute creature world-space position and yaw coupling relative to James.
///
/// Implements 1:1 `PlayerCheckHuggedPos` (`0x0021bdc0`):
/// - Rotates lateral ($X$) and forward ($Z$) offsets by James's current yaw angle.
/// - Adjusts vertical ($Y$) position in world space.
/// - Determines creature yaw alignment based on facing mode.
#[inline]
pub fn compute_grapple_coupling(
    player_pos: Vec3,
    player_yaw: f32,
    offset: Vec3,
    facing: GrappleFacingMode,
) -> (Vec3, f32) {
    let sin = player_yaw.sin();
    let cos = player_yaw.cos();

    // SH2 coordinate frame: local X is lateral (+right), Z is forward
    let world_x = player_pos.x + (offset.x * cos + offset.z * sin);
    let world_y = player_pos.y + offset.y;
    let world_z = player_pos.z + (-offset.x * sin + offset.z * cos);

    let creature_yaw = match facing {
        GrappleFacingMode::FaceTarget => player_yaw + std::f32::consts::PI,
        GrappleFacingMode::SameDirection => player_yaw,
    };

    (Vec3::new(world_x, world_y, world_z), creature_yaw)
}

/// Helper to compute creature transform directly from its `BestiaryId`.
#[inline]
pub fn compute_creature_grapple_transform(
    species: BestiaryId,
    player_pos: Vec3,
    player_yaw: f32,
) -> (Vec3, f32) {
    if let Some(spec) = get_grapple_spec(species) {
        compute_grapple_coupling(player_pos, player_yaw, spec.anchor_offset, spec.facing_mode)
    } else {
        let anchor = GrappleAnchor::for_species(species);
        let pos = anchor.compute_world_anchor(player_pos, player_yaw);
        (pos, player_yaw + std::f32::consts::PI)
    }
}
