//! Free-cam noclip controller.

use glam::Vec3;

#[derive(Debug, Default)]
pub struct FreeCam {
    pub position: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub speed: f32,
}
