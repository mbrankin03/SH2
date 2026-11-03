//! Collision and hitbox visualizer definitions.

use glam::Vec3;

#[derive(Debug, Clone)]
pub struct HitboxDebugDraw {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub color: [f32; 4],
}
