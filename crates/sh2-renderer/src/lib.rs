//! sh2-renderer: Modern wgpu/Vulkan pipeline, 32-bit float depth, volumetric compute fog.

pub mod blitter;
pub mod effects;
pub mod fmv;
pub mod fog;
pub mod gpu_map;
pub mod light;
mod lighting_ops;
pub mod pipeline;
pub mod post;
pub mod scene;
pub mod shadow;
pub mod skeletal;
pub mod tile_pool;

pub use blitter::SurfaceBlitter;
pub use fmv::{FmvUniforms, FmvVideoPipeline, GpuYuvVideoSurface};
pub use gpu_map::{GpuMap, GpuMapBatch};
use sh2_assets::formats::mdls::ModelFile;
use sh2_assets::formats::texture::generate_mipmap_chain_rgba;
pub use skeletal::{
    classify_primitive_body_region, cranial_bone_mask, decode_model_textures_for_inspection,
    dilate_transparent_border_rgb, discover_pose_presets, dominant_vertex_bone,
    find_cranial_root_bone, is_default_active_pose, is_opaque_cutout_primitive,
    partition_first_person_triangles, refine_4k_chr_texture_from_sources, DecodedModelTextureMap,
    GpuSkeletalMesh, GpuSkeletalPrimitive, PoseVariantPreset, SkeletalBodyRegion,
    SkeletalMaterialUniforms, SkeletalPassStage, SkeletalPipeline, SkeletalVertex,
    SkeletonBoneUniforms, MAX_SKELETON_BONES,
};
use std::sync::Arc;
pub use tile_pool::GpuOutdoorTilePool;
use wgpu::util::DeviceExt;

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

    pub geometry_pipeline: pipeline::GeometryPipeline,
    pub skeletal_pipeline: skeletal::SkeletalPipeline,
    pub debug_line_pipeline: pipeline::DebugLinePipeline,
    pub fog_pipeline: fog::FogComputePipeline,
    pub fog_particle_pipeline: fog::FogParticlePipeline,
    pub spotlight_shadow_pipeline: fog::SpotlightShadowPipeline,
    pub pcss_shadow_pipeline: shadow::PcssShadowPipeline,
    pub stencil_shadow_pipeline: shadow::StencilShadowPipeline,
    pub lens_flare_pipeline: effects::LensFlarePipeline,
    pub spray_mist_pipeline: effects::SprayMistPipeline,
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
    pub fog_noise_texture: wgpu::Texture,
    pub fog_noise_view: wgpu::TextureView,
    pub fog_noise_sampler: wgpu::Sampler,
    pub lens_flare_texture: wgpu::Texture,
    pub lens_flare_view: wgpu::TextureView,
    pub spray_mist_texture: wgpu::Texture,
    pub spray_mist_view: wgpu::TextureView,

    pub geometry_bind_group: wgpu::BindGroup,
    pub spotlight_shadow_bind_group: wgpu::BindGroup,
    pub pcss_shadow_bind_group: wgpu::BindGroup,
    pub shadow_darken_bind_group: wgpu::BindGroup,
    pub lens_flare_bind_group: wgpu::BindGroup,
    pub spray_mist_bind_group: wgpu::BindGroup,
    pub fog_particle_bind_group: wgpu::BindGroup,
    pub fog_bind_group: wgpu::BindGroup,
    pub post_bind_group_fog: wgpu::BindGroup,
    pub post_bind_group_direct: wgpu::BindGroup,

    pub fog_particle_count: std::sync::atomic::AtomicU32,
    pub shadow_volume_vertex_count: std::sync::atomic::AtomicU32,
    pub drop_shadow_vertex_count: std::sync::atomic::AtomicU32,
    pub lens_flare_quad_count: std::sync::atomic::AtomicU32,
    pub spray_mist_particle_count: std::sync::atomic::AtomicU32,
    pub pcss_enabled: std::sync::atomic::AtomicBool,
    last_shadow_view_proj: std::sync::Mutex<[[f32; 4]; 4]>,
    last_pcss_params: std::sync::Mutex<[f32; 4]>,

    pub default_sampler: wgpu::Sampler,
    pub default_view: wgpu::TextureView,
    pub default_texture_bind_group: Arc<wgpu::BindGroup>,
}

impl RenderContext {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    /// Capacity of the reusable GPU vertex buffer for 3D debug line overlays (65,536 vertices = ~2 MB).
    pub const MAX_DEBUG_LINE_VERTICES: usize = 65_536;

    pub fn new_headless(config: RendererConfig) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })?;

        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
        }
        if adapter_features.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC) {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_ASTC;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("SH2 Render Device"),
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        Some(Self::from_device(instance, adapter, device, queue, config))
    }

    #[allow(clippy::type_complexity)]
    fn create_offscreen_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (
        (wgpu::Texture, wgpu::TextureView),
        (wgpu::Texture, wgpu::TextureView),
        (wgpu::Texture, wgpu::TextureView),
        (wgpu::Texture, wgpu::TextureView),
    ) {
        let extent = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("32-bit Float Depth Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let hdr_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Color Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hdr_color_view = hdr_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let fog_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fog Output Color Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::HDR_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fog_color_view = fog_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let ldr_output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("LDR Output Color Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let ldr_output_view =
            ldr_output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        (
            (depth_texture, depth_view),
            (hdr_color_texture, hdr_color_view),
            (fog_color_texture, fog_color_view),
            (ldr_output_texture, ldr_output_view),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_compute_bind_groups(
        device: &wgpu::Device,
        fog_pipeline: &fog::FogComputePipeline,
        post_pipeline: &post::PostProcessPipeline,
        fog_buffer: &wgpu::Buffer,
        camera_buffer: &wgpu::Buffer,
        spotlight_buffer: &wgpu::Buffer,
        post_buffer: &wgpu::Buffer,
        depth_view: &wgpu::TextureView,
        hdr_color_view: &wgpu::TextureView,
        fog_color_view: &wgpu::TextureView,
        shadow_depth_view: &wgpu::TextureView,
        fog_noise_view: &wgpu::TextureView,
        fog_noise_sampler: &wgpu::Sampler,
        ldr_output_view: &wgpu::TextureView,
    ) -> (wgpu::BindGroup, wgpu::BindGroup, wgpu::BindGroup) {
        let fog_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Bind Group"),
            layout: &fog_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: fog_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(hdr_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(fog_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: spotlight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(shadow_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(fog_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(fog_noise_sampler),
                },
            ],
        });

        let post_bind_group_fog = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group (With Fog)"),
            layout: &post_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: post_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(fog_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(ldr_output_view),
                },
            ],
        });

        let post_bind_group_direct = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group (Direct HDR)"),
            layout: &post_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: post_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(hdr_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(ldr_output_view),
                },
            ],
        });

        (fog_bind_group, post_bind_group_fog, post_bind_group_direct)
    }

    pub fn from_device(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: RendererConfig,
    ) -> Self {
        let (
            (depth_texture, depth_view),
            (hdr_color_texture, hdr_color_view),
            (fog_color_texture, fog_color_view),
            (ldr_output_texture, ldr_output_view),
        ) = Self::create_offscreen_targets(&device, config.width, config.height);

        let geometry_pipeline =
            pipeline::GeometryPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let skeletal_pipeline = skeletal::SkeletalPipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
            &geometry_pipeline.texture_bind_group_layout,
        );
        let debug_line_pipeline = pipeline::DebugLinePipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
        let fog_pipeline = fog::FogComputePipeline::new(&device);
        let fog_particle_pipeline =
            fog::FogParticlePipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let spotlight_shadow_pipeline =
            fog::SpotlightShadowPipeline::new(&device, Self::DEPTH_FORMAT);
        let pcss_shadow_pipeline = shadow::PcssShadowPipeline::new(
            &device,
            Self::DEPTH_FORMAT,
            &skeletal_pipeline.bone_bind_group_layout,
        );
        let stencil_shadow_pipeline = shadow::StencilShadowPipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
        let lens_flare_pipeline = effects::LensFlarePipeline::new(&device, Self::HDR_FORMAT);
        let spray_mist_pipeline =
            effects::SprayMistPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let post_pipeline = post::PostProcessPipeline::new(&device);

        let camera_uniforms = pipeline::CameraUniforms::default();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&camera_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let spotlight_uniforms = light::SpotlightUniforms::default();
        let spotlight_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Spotlight Uniform Buffer"),
            contents: bytemuck::bytes_of(&spotlight_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let initial_shadow_vp =
            fog::compute_spotlight_view_proj(&spotlight_uniforms).to_cols_array_2d();
        let spotlight_shadow_uniforms = fog::SpotlightShadowUniforms {
            light_view_proj: initial_shadow_vp,
        };
        let spotlight_shadow_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Spotlight Shadow Uniform Buffer"),
                contents: bytemuck::bytes_of(&spotlight_shadow_uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let initial_pcss_uniforms = shadow::PcssShadowUniforms {
            light_view_proj: initial_shadow_vp,
            ..shadow::PcssShadowUniforms::default()
        };
        let pcss_shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PCSS Shadow Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial_pcss_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shadow_darken_uniforms = shadow::ShadowDarkenUniforms::default();
        let shadow_darken_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Darken Uniform Buffer"),
            contents: bytemuck::bytes_of(&shadow_darken_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shadow_volume_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PS2 Stencil Shadow Volume Vertex Buffer"),
            size: (shadow::MAX_SHADOW_VOLUME_VERTICES
                * std::mem::size_of::<shadow::ShadowVolumeVertex>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let drop_shadow_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PS2 Floor Drop Shadow Vertex Buffer"),
            size: (shadow::MAX_SHADOW_VOLUME_VERTICES
                * std::mem::size_of::<shadow::ShadowVolumeVertex>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lens_flare_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lens Flare Quad Instance Buffer"),
            size: (effects::MAX_GPU_LENS_FLARE_QUADS
                * std::mem::size_of::<effects::GpuLensFlareQuad>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let spray_mist_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hyper Spray Mist Instance Buffer"),
            size: (effects::MAX_GPU_SPRAY_PARTICLES
                * std::mem::size_of::<effects::GpuSprayParticle>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let fog_uniforms = fog::FogUniforms {
            shadow_view_proj: initial_shadow_vp,
            ..fog::FogUniforms::default()
        };
        let fog_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fog Uniform Buffer"),
            contents: bytemuck::bytes_of(&fog_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let fog_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fog Particle Instance Buffer"),
            size: (fog::MAX_GPU_FOG_PARTICLES * std::mem::size_of::<fog::GpuFogParticle>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Spotlight Shadow Depth Texture"),
            size: wgpu::Extent3d {
                width: fog::SPOTLIGHT_SHADOW_MAP_SIZE,
                height: fog::SPOTLIGHT_SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_depth_view =
            shadow_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let pcss_shadow_depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PCSS 2048x2048 Spotlight Shadow Depth Texture"),
            size: wgpu::Extent3d {
                width: shadow::PCSS_SHADOW_MAP_SIZE,
                height: shadow::PCSS_SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let pcss_shadow_depth_view =
            pcss_shadow_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let stencil_depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PS2 Stencil Shadow Depth24PlusStencil8 Texture"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: shadow::StencilShadowPipeline::STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let stencil_depth_view =
            stencil_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let fog_noise_size = 256u32;
        let fog_noise_rgba = fog::generate_procedural_fog_noise_rgba(fog_noise_size);
        let fog_noise_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Tileable Fog Noise Texture"),
            size: wgpu::Extent3d {
                width: fog_noise_size,
                height: fog_noise_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &fog_noise_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &fog_noise_rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(fog_noise_size * 4),
                rows_per_image: Some(fog_noise_size),
            },
            wgpu::Extent3d {
                width: fog_noise_size,
                height: fog_noise_size,
                depth_or_array_layers: 1,
            },
        );
        let fog_noise_view = fog_noise_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let fog_noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tileable Fog Noise Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Procedural fallback textures for Lens Flare (`lens_flare.tbn2`) and Hyper Spray (`poison.tbn2`)
        let flare_size = 128u32;
        let flare_rgba = effects::generate_procedural_lens_flare_rgba(flare_size);
        let lens_flare_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Lens Flare Atlas Texture"),
            size: wgpu::Extent3d {
                width: flare_size,
                height: flare_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &lens_flare_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &flare_rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(flare_size * 4),
                rows_per_image: Some(flare_size),
            },
            wgpu::Extent3d {
                width: flare_size,
                height: flare_size,
                depth_or_array_layers: 1,
            },
        );
        let lens_flare_view =
            lens_flare_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let spray_mist_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Hyper Spray Mist Texture"),
            size: wgpu::Extent3d {
                width: fog_noise_size,
                height: fog_noise_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &spray_mist_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &fog_noise_rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(fog_noise_size * 4),
                rows_per_image: Some(fog_noise_size),
            },
            wgpu::Extent3d {
                width: fog_noise_size,
                height: fog_noise_size,
                depth_or_array_layers: 1,
            },
        );
        let spray_mist_view =
            spray_mist_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let post_uniforms = post::PostUniforms::default();
        let post_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Post Uniform Buffer"),
            contents: bytemuck::bytes_of(&post_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let debug_line_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Persistent Debug Line Vertex Buffer"),
            size: (Self::MAX_DEBUG_LINE_VERTICES
                * std::mem::size_of::<sh2_debug::DebugLineVertex>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let geometry_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Geometry Bind Group"),
            layout: &geometry_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: spotlight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: pcss_shadow_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&pcss_shadow_depth_view),
                },
            ],
        });

        let spotlight_shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Spotlight Shadow Bind Group"),
            layout: &spotlight_shadow_pipeline.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: spotlight_shadow_buffer.as_entire_binding(),
            }],
        });

        let pcss_shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PCSS Shadow Bind Group"),
            layout: &pcss_shadow_pipeline.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: pcss_shadow_buffer.as_entire_binding(),
            }],
        });

        let shadow_darken_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Darken Bind Group"),
            layout: &stencil_shadow_pipeline.darken_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_darken_buffer.as_entire_binding(),
            }],
        });

        let lens_flare_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lens Flare Texture Bind Group"),
            layout: &lens_flare_pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&lens_flare_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&fog_noise_sampler),
                },
            ],
        });

        let spray_mist_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hyper Spray Mist Bind Group"),
            layout: &spray_mist_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&spray_mist_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&fog_noise_sampler),
                },
            ],
        });

        let fog_particle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Particle Bind Group"),
            layout: &fog_particle_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&fog_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&fog_noise_sampler),
                },
            ],
        });

        let (fog_bind_group, post_bind_group_fog, post_bind_group_direct) =
            Self::create_compute_bind_groups(
                &device,
                &fog_pipeline,
                &post_pipeline,
                &fog_buffer,
                &camera_buffer,
                &spotlight_buffer,
                &post_buffer,
                &depth_view,
                &hdr_color_view,
                &fog_color_view,
                &shadow_depth_view,
                &fog_noise_view,
                &fog_noise_sampler,
                &ldr_output_view,
            );

        // Default 1x1 white texture for untextured geometry fallback
        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default 1x1 White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let default_view = default_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Default Anisotropic 16x Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        let default_texture_bind_group =
            Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Diffuse Texture Bind Group"),
                layout: &geometry_pipeline.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&default_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&default_sampler),
                    },
                ],
            }));

        Self {
            instance,
            adapter,
            device,
            queue,
            config,
            depth_format: Self::DEPTH_FORMAT,
            hdr_format: Self::HDR_FORMAT,
            output_format: Self::OUTPUT_FORMAT,
            depth_texture,
            depth_view,
            hdr_color_texture,
            hdr_color_view,
            fog_color_texture,
            fog_color_view,
            ldr_output_texture,
            ldr_output_view,
            geometry_pipeline,
            skeletal_pipeline,
            debug_line_pipeline,
            fog_pipeline,
            fog_particle_pipeline,
            spotlight_shadow_pipeline,
            pcss_shadow_pipeline,
            stencil_shadow_pipeline,
            lens_flare_pipeline,
            spray_mist_pipeline,
            post_pipeline,
            camera_buffer,
            spotlight_buffer,
            spotlight_shadow_buffer,
            pcss_shadow_buffer,
            shadow_darken_buffer,
            shadow_volume_buffer,
            drop_shadow_buffer,
            lens_flare_buffer,
            spray_mist_buffer,
            fog_buffer,
            fog_particle_buffer,
            post_buffer,
            debug_line_buffer,
            shadow_depth_texture,
            shadow_depth_view,
            pcss_shadow_depth_texture,
            pcss_shadow_depth_view,
            stencil_depth_texture,
            stencil_depth_view,
            fog_noise_texture,
            fog_noise_view,
            fog_noise_sampler,
            lens_flare_texture,
            lens_flare_view,
            spray_mist_texture,
            spray_mist_view,
            geometry_bind_group,
            spotlight_shadow_bind_group,
            pcss_shadow_bind_group,
            shadow_darken_bind_group,
            lens_flare_bind_group,
            spray_mist_bind_group,
            fog_particle_bind_group,
            fog_bind_group,
            post_bind_group_fog,
            post_bind_group_direct,
            fog_particle_count: std::sync::atomic::AtomicU32::new(0),
            shadow_volume_vertex_count: std::sync::atomic::AtomicU32::new(0),
            drop_shadow_vertex_count: std::sync::atomic::AtomicU32::new(0),
            lens_flare_quad_count: std::sync::atomic::AtomicU32::new(0),
            spray_mist_particle_count: std::sync::atomic::AtomicU32::new(0),
            pcss_enabled: std::sync::atomic::AtomicBool::new(true),
            last_shadow_view_proj: std::sync::Mutex::new(initial_shadow_vp),
            last_pcss_params: std::sync::Mutex::new(initial_pcss_uniforms.params),
            default_sampler,
            default_view,
            default_texture_bind_group,
        }
    }

    pub fn update_camera(&self, view_proj: glam::Mat4, inv_proj: glam::Mat4, cam_pos: glam::Vec3) {
        self.update_camera_with_mode(view_proj, inv_proj, cam_pos, 0.0);
    }

    /// Builds a right-handed `Depth32Float` perspective `(proj, inv_proj)` pair honoring a custom
    /// `near_clip` in SH2 world units (e.g. `8.0` in First-Person vs `25.0` in Third-Person/Rail) (`SH2-93`).
    #[inline]
    pub fn perspective_projection(
        &self,
        fov_y_radians: f32,
        near_clip: f32,
        far_clip: f32,
    ) -> (glam::Mat4, glam::Mat4) {
        let aspect = (self.config.width.max(1) as f32) / (self.config.height.max(1) as f32);
        let z_near = near_clip.max(1.0);
        let z_far = far_clip.max(z_near + 100.0);
        let proj = glam::Mat4::perspective_rh(fov_y_radians, aspect, z_near, z_far);
        (proj, proj.inverse())
    }

    pub fn update_camera_with_mode(
        &self,
        view_proj: glam::Mat4,
        inv_proj: glam::Mat4,
        cam_pos: glam::Vec3,
        shade_mode: f32,
    ) {
        let inv_vp = if view_proj.determinant().abs() > 1e-12 {
            view_proj.inverse()
        } else {
            glam::Mat4::IDENTITY
        };
        let uniforms = pipeline::CameraUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_proj: inv_proj.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, shade_mode],
            inv_view_proj: inv_vp.to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn update_fog(&self, uniforms: &fog::FogUniforms) {
        let mut effective = *uniforms;
        if let Ok(guard) = self.last_shadow_view_proj.lock() {
            effective.shadow_view_proj = *guard;
        }
        self.queue
            .write_buffer(&self.fog_buffer, 0, bytemuck::bytes_of(&effective));
    }

    pub fn update_spotlight(&self, uniforms: &light::SpotlightUniforms) {
        self.queue
            .write_buffer(&self.spotlight_buffer, 0, bytemuck::bytes_of(uniforms));
        let shadow_vp = fog::compute_spotlight_view_proj(uniforms).to_cols_array_2d();
        let shadow_u = fog::SpotlightShadowUniforms {
            light_view_proj: shadow_vp,
        };
        self.queue.write_buffer(
            &self.spotlight_shadow_buffer,
            0,
            bytemuck::bytes_of(&shadow_u),
        );
        if let Ok(mut guard) = self.last_shadow_view_proj.lock() {
            *guard = shadow_vp;
        }
        let pcss_vp = shadow::compute_pcss_spotlight_view_proj(uniforms).to_cols_array_2d();
        let pcss_params =
            self.last_pcss_params
                .lock()
                .map(|g| *g)
                .unwrap_or([1.0, 170.0 / 255.0, 0.024, 0.5]);
        let pcss_u = shadow::PcssShadowUniforms {
            light_view_proj: pcss_vp,
            params: pcss_params,
        };
        self.queue
            .write_buffer(&self.pcss_shadow_buffer, 0, bytemuck::bytes_of(&pcss_u));
        let vp_offset = std::mem::offset_of!(fog::FogUniforms, shadow_view_proj) as u64;
        self.queue
            .write_buffer(&self.fog_buffer, vp_offset, bytemuck::bytes_of(&shadow_vp));
    }

    /// Uploads localized 3D proximity fog sprite instances (`fogMakePacket` @ `0x0016bb70`)
    /// for rendering in the next scene pass (`SH2-32`).
    pub fn update_fog_particles(&self, particles: &[fog::GpuFogParticle]) {
        let count = particles.len().min(fog::MAX_GPU_FOG_PARTICLES);
        if count > 0 {
            self.queue.write_buffer(
                &self.fog_particle_buffer,
                0,
                bytemuck::cast_slice(&particles[..count]),
            );
        }
        self.fog_particle_count
            .store(count as u32, std::sync::atomic::Ordering::Relaxed);
    }

    /// Replaces the tileable fog noise texture with decoded RGBA8 data (e.g. retail `data/pic/effect/fog.tex`).
    pub fn upload_fog_noise_texture(&mut self, rgba: &[u8], width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Retail Fog Noise Texture (fog.tex)"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..(w * h * 4) as usize],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.fog_noise_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.fog_noise_texture = tex;

        self.fog_particle_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Particle Bind Group"),
            layout: &self.fog_particle_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.fog_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.fog_noise_sampler),
                },
            ],
        });

        let (fog_bind_group, post_bind_group_fog, post_bind_group_direct) =
            Self::create_compute_bind_groups(
                &self.device,
                &self.fog_pipeline,
                &self.post_pipeline,
                &self.fog_buffer,
                &self.camera_buffer,
                &self.spotlight_buffer,
                &self.post_buffer,
                &self.depth_view,
                &self.hdr_color_view,
                &self.fog_color_view,
                &self.shadow_depth_view,
                &self.fog_noise_view,
                &self.fog_noise_sampler,
                &self.ldr_output_view,
            );
        self.fog_bind_group = fog_bind_group;
        self.post_bind_group_fog = post_bind_group_fog;
        self.post_bind_group_direct = post_bind_group_direct;
    }

    /// Reconfigures offscreen intermediate targets (Depth, HDR, Fog, and LDR surfaces) when window resolution changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if self.config.width == width && self.config.height == height {
            return;
        }

        self.config.width = width;
        self.config.height = height;

        let (
            (depth_texture, depth_view),
            (hdr_color_texture, hdr_color_view),
            (fog_color_texture, fog_color_view),
            (ldr_output_texture, ldr_output_view),
        ) = Self::create_offscreen_targets(&self.device, width, height);

        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.hdr_color_texture = hdr_color_texture;
        self.hdr_color_view = hdr_color_view;
        self.fog_color_texture = fog_color_texture;
        self.fog_color_view = fog_color_view;
        self.ldr_output_texture = ldr_output_texture;
        self.ldr_output_view = ldr_output_view;

        let stencil_depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PS2 Stencil Shadow Depth24PlusStencil8 Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: shadow::StencilShadowPipeline::STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.stencil_depth_view =
            stencil_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.stencil_depth_texture = stencil_depth_texture;

        let (fog_bind_group, post_bind_group_fog, post_bind_group_direct) =
            Self::create_compute_bind_groups(
                &self.device,
                &self.fog_pipeline,
                &self.post_pipeline,
                &self.fog_buffer,
                &self.camera_buffer,
                &self.spotlight_buffer,
                &self.post_buffer,
                &self.depth_view,
                &self.hdr_color_view,
                &self.fog_color_view,
                &self.shadow_depth_view,
                &self.fog_noise_view,
                &self.fog_noise_sampler,
                &self.ldr_output_view,
            );

        self.fog_bind_group = fog_bind_group;
        self.post_bind_group_fog = post_bind_group_fog;
        self.post_bind_group_direct = post_bind_group_direct;
    }

    /// Creates a GPU diffuse texture bind group with bilinear filtering using a specific texture view.
    pub fn create_texture_bind_group_from_view(
        &self,
        view: &wgpu::TextureView,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        Arc::new(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &self.geometry_pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
            ],
        }))
    }

    /// Creates a GPU diffuse texture bind group with full mipmap chain and 16x anisotropic filtering from raw RGBA8 pixels.
    pub fn create_texture_bind_group(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        self.create_mipmapped_texture_bind_group(width, height, rgba, label)
    }

    /// Creates a GPU diffuse texture bind group generating a full alpha-coverage-preserving mipmap chain
    /// and 16x anisotropic sampler. Uses hardware BC1/BC3 (`DXT1`/`DXT5`) block compression when
    /// `wgpu::Features::TEXTURE_COMPRESSION_BC` is enabled and the dimensions are multiples of 4
    /// (`>= 16x16`), reducing GPU VRAM usage by 4x–8x (`SH2-92`).
    pub fn create_mipmapped_texture_bind_group(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        let w0 = width.max(1);
        let h0 = height.max(1);
        let mips = generate_mipmap_chain_rgba(rgba, w0, h0);

        let supports_bc = self
            .device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let can_use_bc =
            supports_bc && w0 >= 16 && h0 >= 16 && w0.is_multiple_of(4) && h0.is_multiple_of(4);

        if can_use_bc {
            let valid_bc_mips: Vec<&(u32, u32, Vec<u8>)> = mips
                .iter()
                .take_while(|(mw, mh, _)| {
                    *mw >= 4 && *mh >= 4 && mw.is_multiple_of(4) && mh.is_multiple_of(4)
                })
                .collect();

            if !valid_bc_mips.is_empty() {
                let has_alpha = rgba.as_chunks::<4>().0.iter().any(|px| px[3] < 255);
                let (bc_format, dxt_kind, block_bytes) = if has_alpha {
                    (wgpu::TextureFormat::Bc3RgbaUnorm, 5u8, 16u32)
                } else {
                    (wgpu::TextureFormat::Bc1RgbaUnorm, 1u8, 8u32)
                };

                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label,
                    size: wgpu::Extent3d {
                        width: w0,
                        height: h0,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: valid_bc_mips.len() as u32,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: bc_format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                for (level, (mip_w, mip_h, mip_rgba)) in valid_bc_mips.iter().enumerate() {
                    let bc_bytes = sh2_assets::formats::dxt::encode_dxt_rgba(
                        mip_rgba,
                        *mip_w as usize,
                        *mip_h as usize,
                        dxt_kind,
                    );
                    let blocks_x = mip_w / 4;
                    let blocks_y = mip_h / 4;
                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &texture,
                            mip_level: level as u32,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &bc_bytes,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(blocks_x * block_bytes),
                            rows_per_image: Some(blocks_y),
                        },
                        wgpu::Extent3d {
                            width: *mip_w,
                            height: *mip_h,
                            depth_or_array_layers: 1,
                        },
                    );
                }

                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                return self.create_texture_bind_group_from_view(&view, label);
            }
        }

        let mip_level_count = (mips.len() as u32).max(1);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: w0,
                height: h0,
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (level, (mip_w, mip_h, mip_rgba)) in mips.iter().enumerate() {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                mip_rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * (*mip_w)),
                    rows_per_image: Some(*mip_h),
                },
                wgpu::Extent3d {
                    width: *mip_w,
                    height: *mip_h,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.create_texture_bind_group_from_view(&view, label)
    }

    /// Creates a 2D UI presentation texture bind group from raw RGBA8 pixels using the SurfaceBlitter bind group layout.
    pub fn create_blitter_bind_group(
        &self,
        blitter: &SurfaceBlitter,
        width: u32,
        height: u32,
        rgba: &[u8],
        uniform_buffer: &wgpu::Buffer,
        label: Option<&str>,
    ) -> Arc<wgpu::BindGroup> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Arc::new(blitter.create_bind_group_with_uniform_buffer(&self.device, &view, uniform_buffer))
    }

    /// Update post-processing uniforms buffer.
    pub fn update_post(&self, uniforms: &post::PostUniforms) {
        self.queue
            .write_buffer(&self.post_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Compiles a ModelFile into a GpuSkeletalMesh.
    pub fn compile_skeletal_mesh(&self, model: &ModelFile) -> GpuSkeletalMesh {
        GpuSkeletalMesh::from_model(
            &self.device,
            &self.skeletal_pipeline.bone_bind_group_layout,
            &self.skeletal_pipeline.material_bind_group_layout,
            model,
        )
    }
}
