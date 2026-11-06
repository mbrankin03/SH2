//! Multi-character collision cylinders and character-to-character separation (`clCheckColumn2ColumnHit`).
//!
//! On the original PS2, `clCharaList[32][2]` tracked all active characters (James, Maria,
//! enemies) as vertical columns:
//! - Position $(X, Y, Z)$
//! - Cylinder radius (James/Maria ~250-260 mm, Abstract Daddy ~500 mm, Nurses/Lying Figures ~240 mm)
//! - Cylinder height (~1800 mm)
//! - Push priority / mass weight (e.g. Bosses do not get pushed by James)
//!
//! `clCheckColumn2ColumnHit` performs 2D circle-circle separation with vertical overlap gating:
//! when two cylinders overlap in $XZ$ and vertical extents overlap, they are separated
//! elastically along their difference normal.

use glam::Vec3;

/// Handle/index identifying a character cylinder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharacterId(pub u32);

/// Character collision cylinder representation matching PS2 `_CL_HITPOLY_COLUMN`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterCylinder {
    pub id: CharacterId,
    pub position: Vec3,
    pub radius: f32,
    pub height: f32,
    /// Mass / push weight. Characters with higher mass push lighter characters.
    /// Default player mass is 1.0. Heavy bosses use mass >= 10.0 (immovable by player).
    pub mass: f32,
    pub active: bool,
}

impl CharacterCylinder {
    pub const fn new(id: CharacterId, position: Vec3, radius: f32, height: f32, mass: f32) -> Self {
        Self {
            id,
            position,
            radius,
            height,
            mass,
            active: true,
        }
    }

    /// Check if two cylinders overlap along the vertical axis (world $+Y$ pointing downward).
    /// Feet are at `pos.y`, head is at `pos.y - height`.
    #[inline]
    pub fn overlaps_vertical(&self, other: &Self) -> bool {
        let top_a = self.position.y - self.height;
        let bottom_a = self.position.y;
        let top_b = other.position.y - other.height;
        let bottom_b = other.position.y;

        // Overlap test in Y
        bottom_a > top_b && top_a < bottom_b
    }

    /// Resolve mutual 2D circle-circle elastic collision separation between two cylinders (`clCheckColumn2ColumnHit`).
    ///
    /// Returns `(delta_a, delta_b)` movement adjustments.
    pub fn resolve_separation(&self, other: &Self) -> (Vec3, Vec3) {
        if !self.active || !other.active || !self.overlaps_vertical(other) {
            return (Vec3::ZERO, Vec3::ZERO);
        }

        let dx = other.position.x - self.position.x;
        let dz = other.position.z - self.position.z;
        let dist_sq = dx * dx + dz * dz;
        let min_dist = self.radius + other.radius;

        if dist_sq >= min_dist * min_dist || dist_sq < 1e-6 {
            // Overlapping exactly on top or outside radius
            if dist_sq < 1e-6 {
                // Degenerate concentric case: push along +X
                let half_pen = min_dist * 0.5;
                return (
                    Vec3::new(-half_pen, 0.0, 0.0),
                    Vec3::new(half_pen, 0.0, 0.0),
                );
            }
            return (Vec3::ZERO, Vec3::ZERO);
        }

        let dist = dist_sq.sqrt();
        let penetration = min_dist - dist;
        let nx = dx / dist;
        let nz = dz / dist;

        // Mass-weighted pushback distribution
        let total_mass = self.mass + other.mass;
        let weight_a = if total_mass > 0.0 {
            other.mass / total_mass
        } else {
            0.5
        };
        let weight_b = if total_mass > 0.0 {
            self.mass / total_mass
        } else {
            0.5
        };

        let delta_a = Vec3::new(
            -nx * penetration * weight_a,
            0.0,
            -nz * penetration * weight_a,
        );
        let delta_b = Vec3::new(
            nx * penetration * weight_b,
            0.0,
            nz * penetration * weight_b,
        );

        (delta_a, delta_b)
    }
}
