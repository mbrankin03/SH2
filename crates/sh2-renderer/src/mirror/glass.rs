//! Procedural mirror quad vertex, WGSL shader source, and `glas03.tbn2` texture helpers.

use bytemuck::{Pod, Zeroable};

use super::math::PlanarMirrorSpec;
use super::pipeline::PlanarMirrorPipeline;

/// GPU vertex for the procedural mirror quad (`position` + `uv`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MirrorQuadVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

pub(super) const MIRROR_WGSL_SHADER: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    clip_plane: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(0) var glass_tex: texture_2d<f32>;
@group(1) @binding(1) var glass_sampler: sampler;

struct QuadVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct MapVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_quad(in: QuadVertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    return out;
}

@vertex
fn vs_map(in: MapVertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_mask(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}

// Direct alpha-blended dirty glass sample (`sh2pc.exe` `0x005b1694`: `D3DBLEND_SRCALPHA` /
// `D3DBLEND_INVSRCALPHA` using the texture's own alpha channel, matching `er01.map` `batch[17]`
// and `ap84.map` `batch[1]` vertex alpha = `255`).
@fragment
fn fs_glass(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(glass_tex, glass_sampler, in.uv);
}
"#;

pub(super) fn create_glass_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::Texture {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Mirror Dirty Glass Texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width.max(1)),
            rows_per_image: Some(height.max(1)),
        },
        wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
    );
    tex
}

impl PlanarMirrorPipeline {
    /// Uploads a decoded `glas03.tbn2` RGBA8 texture for the optional dirty glass overlay.
    pub fn upload_glass_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        self.glass_texture = create_glass_texture(device, queue, width, height, rgba);
        self.glass_view = self
            .glass_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Mirror Glass Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        self.glass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mirror Glass Overlay Bind Group"),
            layout: &self.glass_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.glass_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
    }

    /// Updates [`Self::quad_vertex_buffer`] from `spec.quad_vertices`.
    pub fn update_quad(&self, queue: &wgpu::Queue, spec: &PlanarMirrorSpec) {
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let verts: [MirrorQuadVertex; 4] = std::array::from_fn(|i| MirrorQuadVertex {
            position: spec.quad_vertices[i],
            uv: uvs[i],
        });
        queue.write_buffer(&self.quad_vertex_buffer, 0, bytemuck::cast_slice(&verts));
    }
}
