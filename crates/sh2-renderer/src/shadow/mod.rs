//! Shadow rendering subsystem (`SH2-81`):
//! - [`volume`]: CPU `.kg1`/`.kg2` stencil shadow volume extrusion (`0x444b20` / `0x4449f0`)
//!   and floor drop-shadow flattening (`0x5a7c90`).
//! - [`stencil`]: Two-pass Z-fail stencil shadow volume & drop-shadow GPU pipeline (`0x445540`).
//! - [`pcss`]: Native Enhanced `2048x2048` GPU PCSS spotlight shadow map (static map + 4-bone LBS
//!   skeletal self-shadowing) with contact-hardening penumbras.

pub mod pcss;
pub mod stencil;
pub mod volume;

pub use pcss::{
    compute_pcss_spotlight_view_proj, PcssShadowPipeline, PcssShadowUniforms, PCSS_SHADOW_MAP_SIZE,
    WGSL_PCSS_FUNCTIONS,
};
pub use stencil::{ShadowDarkenUniforms, StencilShadowPipeline, MAX_SHADOW_VOLUME_VERTICES};
pub use volume::{
    extrude_kg1_stencil_volume, extrude_kg2_stencil_volume, flatten_kg1_drop_shadow,
    ShadowVolumeVertex, DEFAULT_SHADOW_EXTRUDE_DIST_MM, DROP_SHADOW_FLOOR_BIAS_MM,
    MAX_KG1_SHADOW_CASTERS,
};
