//! sh2-debug: Developer tools, free-cam noclip, room warper, hitbox visualizer, and 3D Model Viewer.

pub mod camera;
pub mod model_viewer;
pub mod visualizer;

pub use model_viewer::{
    ModelCategory, ModelViewerEntry, ModelViewerState, OrbitCamera, StageLightingRig,
    TextureSourceMode, ViewerRenderMode, BESTIARY_CATALOG,
};
pub use visualizer::{build_skeleton_debug_lines, build_wireframe_lines, DebugLineVertex};

#[derive(Debug, Default)]
pub struct DebugState {
    pub noclip: bool,
    pub wireframe_collision: bool,
    pub show_hitboxes: bool,
    pub show_fog_params: bool,
}
