//! Free-cam noclip controller for stage inspection and camera debug.
//!
//! # Debug Camera Controls
//! Decouples the active viewport from James Sunderland's camera script rail (`.cam`),
//! allowing arbitrary six-degrees-of-freedom navigation throughout the level geometry.

use glam::Vec3;

/// Free-flying debug camera state.
#[derive(Debug, Default)]
pub struct FreeCam {
    /// World position $(X, Y, Z)$.
    pub position: Vec3,
    /// Pitch angle in radians (vertical look).
    pub pitch: f32,
    /// Yaw angle in radians (horizontal heading).
    pub yaw: f32,
    /// Translation movement speed in world units per second.
    pub speed: f32,
}
