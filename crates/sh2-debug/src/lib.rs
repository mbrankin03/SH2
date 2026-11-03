//! sh2-debug: Developer tools, free-cam noclip, room warper, hitbox visualizer.

pub mod camera;
pub mod visualizer;

#[derive(Debug, Default)]
pub struct DebugState {
    pub noclip: bool,
    pub wireframe_collision: bool,
    pub show_hitboxes: bool,
    pub show_fog_params: bool,
}
