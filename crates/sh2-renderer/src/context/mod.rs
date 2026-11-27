//! Core [`RenderContext`] struct definition, configuration, and submodule wiring.

mod buffers;
mod init;
mod initial_textures;
mod targets;
mod textures;
mod uniforms;

use std::sync::Arc;

use crate::{effects, fog, pipeline, post, shadow, skeletal, water, GpuSkeletalMesh};

pub use textures::{
    prepare_map_texture_cpu, prepare_map_texture_cpu_owned, prepare_mipmapped_texture_cpu,
    prepare_mipmapped_texture_cpu_owned, PreparedGpuTexture,
};

#[derive(Debug, Clone, Copy)]
pub struct RendererConfig {
    pub width: u32,
    pub height: u32,
    pub enable_hdr: bool,
    pub enable_fog: bool,
    pub enable_post: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        }
    }
}

/// Tuple representing a skeletal mesh draw call: (mesh, bone_skin_matrices, optional_texture_bind_groups)
pub type SkeletalDrawItem<'a> = (
    &'a GpuSkeletalMesh,
    &'a [[[f32; 4]; 4]],
    Option<&'a [Arc<wgpu::BindGroup>]>,
);

pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: RendererConfig,

    pub depth_format: wgpu::TextureFormat,
    pub hdr_format: wgpu::TextureFormat,
    pub output_format: wgpu::TextureFormat,

    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,

    pub hdr_color_texture: wgpu::Texture,
    pub hdr_color_view: wgpu::TextureView,

    pub fog_color_texture: wgpu::Texture,
    pub fog_color_view: wgpu::TextureView,

    pub ldr_output_texture: wgpu::Texture,
    pub ldr_output_view: wgpu::TextureView,

    /// Previous-frame LDR feedback texture (`Rgba8Unorm`) for PS2 `sh2gfw_Filter_Retain`
    /// (`0x00179b10`, `ALPHA_1 = 0x8000000062`) pause-freeze and `sh2gfw_Filter_Blur`
    /// (`0x00177440`, `ALPHA_1 = 0x44`) temporal motion blur.
    pub prev_frame_texture: wgpu::Texture,
    pub prev_frame_view: wgpu::TextureView,

    pub geometry_pipeline: pipeline::GeometryPipeline,
    pub skeletal_pipeline: skeletal::SkeletalPipeline,
    pub debug_line_pipeline: pipeline::DebugLinePipeline,
    pub fog_pipeline: fog::FogComputePipeline,
    pub fog_particle_pipeline: fog::FogParticlePipeline,
    pub spotlight_shadow_pipeline: fog::SpotlightShadowPipeline,
    pub pcss_shadow_pipeline: shadow::PcssShadowPipeline,
    pub stencil_shadow_pipeline: shadow::StencilShadowPipeline,
    pub shadow_blur_pipeline: shadow::ShadowBlurPipeline,
    pub lens_flare_pipeline: effects::LensFlarePipeline,
    pub spray_mist_pipeline: effects::SprayMistPipeline,
    pub water_pipeline: water::LakeWaterPipeline,
    pub mirror_pipeline: crate::mirror::PlanarMirrorPipeline,
    pub dust_pipeline: effects::DustParticlePipeline,
    pub billboard_fx_pipeline: effects::BillboardFxPipeline,
    pub blood_pipeline: effects::BloodEffectPipeline,
    pub post_pipeline: post::PostProcessPipeline,

    pub camera_buffer: wgpu::Buffer,
    pub spotlight_buffer: wgpu::Buffer,
    pub spotlight_shadow_buffer: wgpu::Buffer,
    pub pcss_shadow_buffer: wgpu::Buffer,
    pub shadow_darken_buffer: wgpu::Buffer,
    pub shadow_volume_buffer: wgpu::Buffer,
    pub drop_shadow_buffer: wgpu::Buffer,
    pub lens_flare_buffer: wgpu::Buffer,
    pub spray_mist_buffer: wgpu::Buffer,
    pub dust_buffer: wgpu::Buffer,
    pub billboard_fx_additive_buffer: wgpu::Buffer,
    pub billboard_fx_alpha_buffer: wgpu::Buffer,
    pub blood_buffer: wgpu::Buffer,
    pub fog_buffer: wgpu::Buffer,
    pub fog_particle_buffer: wgpu::Buffer,
    pub post_buffer: wgpu::Buffer,
    pub debug_line_buffer: wgpu::Buffer,

    pub shadow_depth_texture: wgpu::Texture,
    pub shadow_depth_view: wgpu::TextureView,
    pub pcss_shadow_depth_texture: wgpu::Texture,
    pub pcss_shadow_depth_view: wgpu::TextureView,
    pub stencil_depth_texture: wgpu::Texture,
    pub stencil_depth_view: wgpu::TextureView,
    pub shadow_mask_texture: wgpu::Texture,
    pub shadow_mask_view: wgpu::TextureView,
    pub shadow_mask_bind_group: wgpu::BindGroup,
    pub fog_noise_texture: wgpu::Texture,
    pub fog_noise_view: wgpu::TextureView,
    pub fog_noise_sampler: wgpu::Sampler,
    pub lens_flare_texture: wgpu::Texture,
    pub lens_flare_view: wgpu::TextureView,
    pub spray_mist_texture: wgpu::Texture,
    pub spray_mist_view: wgpu::TextureView,
    pub dust_texture: wgpu::Texture,
    pub dust_view: wgpu::TextureView,
    pub blood_texture: wgpu::Texture,
    pub blood_view: wgpu::TextureView,
    pub specular_sphere_texture: wgpu::Texture,
    pub specular_sphere_view: wgpu::TextureView,
    pub specular_sphere_sampler: wgpu::Sampler,

    pub geometry_bind_group: wgpu::BindGroup,
    pub spotlight_shadow_bind_group: wgpu::BindGroup,
    pub pcss_shadow_bind_group: wgpu::BindGroup,
    pub shadow_darken_bind_group: wgpu::BindGroup,
    pub lens_flare_bind_group: wgpu::BindGroup,
    pub spray_mist_bind_group: wgpu::BindGroup,
    pub dust_bind_group: wgpu::BindGroup,
    pub billboard_fx_bind_groups: [wgpu::BindGroup; effects::FX_TEXTURE_SLOT_COUNT],
    pub blood_bind_group: wgpu::BindGroup,
    pub fog_particle_bind_group: wgpu::BindGroup,
    pub fog_bind_group: wgpu::BindGroup,
    pub post_bind_group_fog: wgpu::BindGroup,
    pub post_bind_group_direct: wgpu::BindGroup,

    /// Per-character `SpotlightUniforms` buffers and bind groups for `Model3UpdateLightEnv`
    /// (`SLUS_202.28` `0x00128e00`) gameplay lighting in `skeletal_pass.rs`.
    pub(crate) skeletal_light_buffers: Vec<wgpu::Buffer>,
    pub(crate) skeletal_light_bind_groups: Vec<wgpu::BindGroup>,
    pub(crate) skeletal_light_active_slots: std::sync::atomic::AtomicUsize,
    pub(crate) last_spotlight_uniforms: std::sync::Mutex<crate::light::SpotlightUniforms>,
    /// Optional gameplay `(CharacterLightPool, is_night, is_cutscene)` evaluated per skeletal mesh.
    pub(crate) gameplay_char_light_pool:
        std::sync::Mutex<Option<(crate::light::CharacterLightPool, bool, bool)>>,
    /// True while `POST_FLAG_FILTER_RETAIN` (`sh2gfw_Filter_Retain` `0x00179b10`) is active,
    /// freezing `prev_frame_texture` updates until pause-retain is released.
    pub(crate) filter_retain_frozen: std::sync::atomic::AtomicBool,

    pub fog_particle_count: std::sync::atomic::AtomicU32,
    pub shadow_volume_vertex_count: std::sync::atomic::AtomicU32,
    pub drop_shadow_vertex_count: std::sync::atomic::AtomicU32,
    pub lens_flare_quad_count: std::sync::atomic::AtomicU32,
    pub spray_mist_particle_count: std::sync::atomic::AtomicU32,
    pub dust_particle_count: std::sync::atomic::AtomicU32,
    pub billboard_fx_batches: std::sync::Mutex<effects::FxDrawBatches>,
    pub blood_quad_count: std::sync::atomic::AtomicU32,
    pub blood_subtractive_mode: std::sync::atomic::AtomicBool,
    pub pcss_enabled: std::sync::atomic::AtomicBool,
    pub lake_water_enabled: std::sync::atomic::AtomicBool,
    pub water_reflection_pass_enabled: std::sync::atomic::AtomicBool,
    /// True when `ModernVolumetric` + `volumetric_light` + `density > 0.0` require the 512x512 god-ray shadow pass (`SH2-32`).
    pub volumetric_god_rays_enabled: std::sync::atomic::AtomicBool,
    pub soft_shadow_blur_enabled: std::sync::atomic::AtomicBool,
    pub shadow_composite_mode: std::sync::atomic::AtomicU8,
    pub shadow_draw_order: std::sync::atomic::AtomicU8,
    pub clear_color_rgba: std::sync::atomic::AtomicU32,
    pub(crate) lens_flare_smoother: std::sync::Mutex<effects::LensFlareSmoother>,
    pub(crate) last_shadow_view_proj: std::sync::Mutex<[[f32; 4]; 4]>,
    pub(crate) last_pcss_params: std::sync::Mutex<[f32; 4]>,
    pub(crate) last_camera_uniforms: std::sync::Mutex<pipeline::CameraUniforms>,
    pub(crate) planar_mirror: std::sync::Mutex<Option<crate::mirror::PlanarMirrorSpec>>,
    /// Active `.map` `geo_id` part-group/variant visibility filter (`EvDispControlModelEntry`
    /// `SLUS_202.28` `0x001ad560` / `sh2pc.exe` `0x004a2f10`, `SetDispOnOffMask` `0x005b4760`).
    pub(crate) map_visibility_filter:
        std::sync::Mutex<Option<sh2_core::events::stageparts::MapPartVisibilityFilter>>,
    pub mirror_glass_overlay_enabled: std::sync::atomic::AtomicBool,
    pub(crate) water_plane_height_mm: std::sync::atomic::AtomicU32,

    pub default_sampler: wgpu::Sampler,
    /// 16x anisotropic clamped sampler (`AddressMode::ClampToEdge`) for `[0, 1]`-bounded `.map`
    /// textures (`sh2pc.exe` `FUN_005b4cc0` `0x005b4cc0`, `D3DTSS_ADDRESSU/V = D3DTADDRESS_CLAMP`).
    pub clamp_sampler: wgpu::Sampler,
    /// 16x anisotropic sampler with `Repeat` U and `ClampToEdge` V.
    pub repeat_u_clamp_v_sampler: wgpu::Sampler,
    /// 16x anisotropic sampler with `ClampToEdge` U and `Repeat` V.
    pub clamp_u_repeat_v_sampler: wgpu::Sampler,
    /// 1x bilinear sampler (`anisotropy_clamp = 1`, `mipmap_filter = Nearest`) matching PS2
    /// `SLUS_202.28` `shGs_InitDefaultRegsEnv` (`0x001909e0`, `TEX1_1 = 0x60`: `MXL = 0`,
    /// `MMAG/MMIN = LINEAR`) and PC `sh2pc.exe` `FUN_005b4cc0` (`0x005b4cc0`) for single-level
    /// packed cutout `.map` atlases (`ob.map` `tid=4656`, etc.).
    pub bilinear_repeat_sampler: wgpu::Sampler,
    /// 1x bilinear clamped sampler (`ClampToEdge` U and V) for single-level cutout `.map` atlases.
    pub bilinear_clamp_sampler: wgpu::Sampler,
    /// 1x bilinear sampler (`Repeat` U, `ClampToEdge` V) for single-level cutout `.map` atlases.
    pub bilinear_repeat_u_clamp_v_sampler: wgpu::Sampler,
    /// 1x bilinear sampler (`ClampToEdge` U, `Repeat` V) for single-level cutout `.map` atlases.
    pub bilinear_clamp_u_repeat_v_sampler: wgpu::Sampler,
    pub default_view: wgpu::TextureView,
    pub default_texture_bind_group: Arc<wgpu::BindGroup>,
}

impl RenderContext {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    /// Capacity of the reusable GPU vertex buffer for 3D debug line overlays (65,536 vertices = ~2 MB).
    pub const MAX_DEBUG_LINE_VERTICES: usize = 65_536;
    /// Maximum number of per-character `Model3UpdateLightEnv` (`0x00128e00`) light uniform slots.
    pub const MAX_SKELETAL_LIGHT_SLOTS: usize = 16;
}
