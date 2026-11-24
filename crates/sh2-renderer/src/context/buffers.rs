//! Initial GPU uniform and vertex/instance buffers allocated by [`RenderContext::from_device`].

use wgpu::util::DeviceExt;

use crate::{effects, fog, light, pipeline, post, shadow};

use super::RenderContext;

pub(super) struct InitialBuffers {
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
    pub fog_buffer: wgpu::Buffer,
    pub fog_particle_buffer: wgpu::Buffer,
    pub post_buffer: wgpu::Buffer,
    pub debug_line_buffer: wgpu::Buffer,
    pub initial_shadow_vp: [[f32; 4]; 4],
    pub initial_pcss_params: [f32; 4],
}

pub(super) fn create_initial_buffers(device: &wgpu::Device) -> InitialBuffers {
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
    let spotlight_shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
        size: (effects::MAX_GPU_LENS_FLARE_QUADS * std::mem::size_of::<effects::GpuLensFlareQuad>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let spray_mist_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Hyper Spray Mist Instance Buffer"),
        size: (effects::MAX_GPU_SPRAY_PARTICLES * std::mem::size_of::<effects::GpuSprayParticle>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let dust_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Airborne Dust Mote Instance Buffer"),
        size: (effects::MAX_GPU_DUST_PARTICLES * std::mem::size_of::<effects::GpuDustParticle>())
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

    let post_uniforms = post::PostUniforms::default();
    let post_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Post Uniform Buffer"),
        contents: bytemuck::bytes_of(&post_uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let debug_line_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Persistent Debug Line Vertex Buffer"),
        size: (RenderContext::MAX_DEBUG_LINE_VERTICES
            * std::mem::size_of::<sh2_debug::DebugLineVertex>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    InitialBuffers {
        camera_buffer,
        spotlight_buffer,
        spotlight_shadow_buffer,
        pcss_shadow_buffer,
        shadow_darken_buffer,
        shadow_volume_buffer,
        drop_shadow_buffer,
        lens_flare_buffer,
        spray_mist_buffer,
        dust_buffer,
        fog_buffer,
        fog_particle_buffer,
        post_buffer,
        debug_line_buffer,
        initial_shadow_vp,
        initial_pcss_params: initial_pcss_uniforms.params,
    }
}
