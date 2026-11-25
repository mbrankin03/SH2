//! Visual lighting effects pipelines (`SH2-81`):
//! - [`lens_flare`]: Additive screen-space 7-sprite halo flares (`lens_flare.tbn2`).
//! - [`spray_mist`]: Alpha-blended 3D billboard clouds for Hyper Spray & Lying Figure mist.

pub mod billboard_fx;
pub mod blood;
pub mod dust;
pub mod lens_flare;
pub mod spray_mist;

pub use billboard_fx::{
    BillboardFxPipeline, FxDrawBatches, FxDrawRange, FxTextureSlot, GpuFxQuad,
    FX_TEXTURE_SLOT_COUNT, MAX_GPU_FX_QUADS,
};
pub use blood::{BloodEffectPipeline, GpuBloodQuad, MAX_GPU_BLOOD_QUADS};
pub use dust::{DustParticlePipeline, GpuDustParticle, MAX_GPU_DUST_PARTICLES};

pub use lens_flare::{
    apply_ps2_lens_flare_smoothing, generate_procedural_lens_flare_rgba, GpuLensFlareQuad,
    LensFlarePipeline, LensFlareSmoother, MAX_GPU_LENS_FLARE_QUADS, PS2_LENS_FLARE_FLICKER_FRAMES,
    PS2_LENS_FLARE_SMOOTH_RATE, PS2_LENS_FLARE_Y_ASPECT,
};
pub use spray_mist::{GpuSprayParticle, SprayMistPipeline, MAX_GPU_SPRAY_PARTICLES};
