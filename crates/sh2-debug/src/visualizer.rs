//! Collision and hitbox visualizer definitions for debugging physics hulls.
//!
//! # Collision Debugger
//! Visualizes `.cld` convex collision hulls, enemy attack bounding boxes, and interaction
//! trigger volumes rendered as wireframe lines overlaying the 3D scene.

use glam::Vec3;

/// Axis-aligned or oriented bounding box debug visualizer.
#[derive(Debug, Clone)]
pub struct HitboxDebugDraw {
    /// Bounding box center coordinates $(X, Y, Z)$.
    pub center: Vec3,
    /// Half-extents along each coordinate axis $(dX, dY, dZ)$.
    pub half_extents: Vec3,
    /// Wireframe display color in RGBA format.
    pub color: [f32; 4],
}
