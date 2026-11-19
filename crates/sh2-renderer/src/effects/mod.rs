//! Visual lighting effects pipelines (`SH2-81`):
//! - [`lens_flare`]: Additive screen-space 7-sprite halo flares (`lens_flare.tbn2`).
//! - [`spray_mist`]: Alpha-blended 3D billboard clouds for Hyper Spray & Lying Figure mist.

pub mod lens_flare;
pub mod spray_mist;

pub use lens_flare::{
    generate_procedural_lens_flare_rgba, GpuLensFlareQuad, LensFlarePipeline,
    MAX_GPU_LENS_FLARE_QUADS,
};
pub use spray_mist::{GpuSprayParticle, SprayMistPipeline, MAX_GPU_SPRAY_PARTICLES};
