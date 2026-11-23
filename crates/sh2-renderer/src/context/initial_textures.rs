//! Initial shadow depth/stencil targets and procedural textures allocated by [`RenderContext::from_device`].

use crate::{effects, fog, shadow, skeletal};

use super::{RenderContext, RendererConfig};

pub(super) struct InitialTextures {
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
    pub specular_sphere_texture: wgpu::Texture,
    pub specular_sphere_view: wgpu::TextureView,
    pub specular_sphere_sampler: wgpu::Sampler,
}

pub(super) fn create_initial_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    config: RendererConfig,
    shadow_blur_pipeline: &shadow::ShadowBlurPipeline,
) -> InitialTextures {
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
        format: RenderContext::DEPTH_FORMAT,
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
        format: RenderContext::DEPTH_FORMAT,
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

    let shadow_mask_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("PS2 Soft Stencil Shadow Mask Texture"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: shadow::ShadowBlurPipeline::WORK_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_mask_view = shadow_mask_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let shadow_mask_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("PS2 Soft Stencil Shadow Mask Bind Group"),
        layout: &shadow_blur_pipeline.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&shadow_mask_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&shadow_blur_pipeline.linear_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: shadow_blur_pipeline.uniform_buffer.as_entire_binding(),
            },
        ],
    });

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
    let lens_flare_view = lens_flare_texture.create_view(&wgpu::TextureViewDescriptor::default());

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
    let spray_mist_view = spray_mist_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // 1:1 PS2/PC 64x64 sphere-map specular texture (`mdl_SpecularMappingMakeTexture` @ 0x0012f0c0)
    let spec_dim = skeletal::SPECULAR_SPHERE_MAP_DIM;
    let spec_rgba = skeletal::generate_specular_sphere_map_rgba();
    let specular_sphere_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("PS2 64x64 Specular Sphere-Map Texture"),
        size: wgpu::Extent3d {
            width: spec_dim,
            height: spec_dim,
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
            texture: &specular_sphere_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &spec_rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(spec_dim * 4),
            rows_per_image: Some(spec_dim),
        },
        wgpu::Extent3d {
            width: spec_dim,
            height: spec_dim,
            depth_or_array_layers: 1,
        },
    );
    let specular_sphere_view =
        specular_sphere_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let specular_sphere_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("PS2 Specular Sphere-Map Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    InitialTextures {
        shadow_depth_texture,
        shadow_depth_view,
        pcss_shadow_depth_texture,
        pcss_shadow_depth_view,
        stencil_depth_texture,
        stencil_depth_view,
        shadow_mask_texture,
        shadow_mask_view,
        shadow_mask_bind_group,
        fog_noise_texture,
        fog_noise_view,
        fog_noise_sampler,
        lens_flare_texture,
        lens_flare_view,
        spray_mist_texture,
        spray_mist_view,
        specular_sphere_texture,
        specular_sphere_view,
        specular_sphere_sampler,
    }
}
