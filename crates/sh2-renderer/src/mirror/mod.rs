//! Stenciled planar mirror reflection subsystem (`SH2-101`).
//!
//! Renders true planar reflections into `hdr_color_view` masked by `Depth24PlusStencil8`
//! (`stencil == 1`) with reverse triangle winding (`FrontFace::Cw`) and world-space
//! mirror-plane clipping so pre-baked PS2 duplicate room geometry behind the mirror plane
//! never bleeds in front of the mirror.

pub mod math;
pub mod pipeline;

pub use math::{
    find_mirror_decal_batch, oblique_near_plane_projection, reflect_point,
    reflected_camera_uniforms, reflection_matrix, PlanarMirrorAxis, PlanarMirrorSpec,
    AP84_FALLBACK_QUAD, ER01_FALLBACK_QUAD, MIRROR_WALL_AP84_Z_MM, MIRROR_WALL_BFW_D5_Z_MM,
    MIRROR_WALL_ER01_X_MM,
};
pub use pipeline::{MirrorQuadVertex, PlanarMirrorPipeline};
