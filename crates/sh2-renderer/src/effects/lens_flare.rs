//! Screen-space additive 7-sprite lens flare pipeline (`Lens/lens_flare.c`, `lens_flare.tbn2`).

use bytemuck::{Pod, Zeroable};
use sh2_core::lighting::LensFlareQuad;

/// Maximum simultaneous lens flare quads in the reusable GPU vertex buffer (`16 lights * 7 sprites = 112`).
pub const MAX_GPU_LENS_FLARE_QUADS: usize = 128;

/// Instanced GPU quad record for [`LensFlarePipeline`] (`48` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuLensFlareQuad {
    /// `(ndc_cx, ndc_cy, ndc_half_w, ndc_half_h)`.
    pub rect: [f32; 4],
    /// `(u_min, v_min, u_max, v_max)` in `lens_flare.tbn2`.
    pub uv_rect: [f32; 4],
    /// Additive linear RGBA tint.
    pub color: [f32; 4],
}

impl From<LensFlareQuad> for GpuLensFlareQuad {
    fn from(q: LensFlareQuad) -> Self {
        Self {
            rect: [
                q.ndc_center[0],
                q.ndc_center[1],
                q.ndc_half_size[0],
                q.ndc_half_size[1],
            ],
            uv_rect: [q.uv_min[0], q.uv_min[1], q.uv_max[0], q.uv_max[1]],
            color: q.color_rgba,
        }
    }
}

/// Additive-blended screen-space pipeline for 7-sprite `lens_flare.tbn2` halo flares.
pub struct LensFlarePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl LensFlarePipeline {
    const WGSL_SHADER: &'static str = r#"
@group(0) @binding(0) var flare_tex: texture_2d<f32>;
@group(0) @binding(1) var flare_sampler: sampler;

struct InstanceInput {
    @location(0) rect: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceInput) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );
    let c = corners[vid];
    let t = uvs[vid];
    let pos = inst.rect.xy + c * inst.rect.zw;
    let uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, t);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample_rgba = textureSample(flare_tex, flare_sampler, in.uv);
    let rgb = sample_rgba.rgb * in.color.rgb * sample_rgba.a;
    return vec4<f32>(rgb, sample_rgba.a * in.color.a);
}
"#;

    /// Creates the additive-blended screen-space lens flare render pipeline.
    #[must_use]
    pub fn new(device: &wgpu::Device, hdr_color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lens Flare Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lens Flare Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lens Flare Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuLensFlareQuad>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }];

        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lens Flare Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &instance_vb,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_color_format,
                    blend: Some(additive),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            texture_bind_group_layout,
        }
    }
}

/// Generates a procedural 128x128 radial halo fallback texture matching the 4-quadrant layout of
/// `lens_flare.tbn2` when retail assets are unavailable (e.g. headless unit tests).
#[must_use]
pub fn generate_procedural_lens_flare_rgba(size: u32) -> Vec<u8> {
    let s = size.max(16);
    let mut out = vec![0u8; (s * s * 4) as usize];
    let half = (s as f32) * 0.5;
    for y in 0..s {
        for x in 0..s {
            // 2x2 sub-atlas of soft radial glows
            let lx = ((x as f32) % half) / half * 2.0 - 1.0;
            let ly = ((y as f32) % half) / half * 2.0 - 1.0;
            let r = (lx * lx + ly * ly).sqrt();
            let v = (1.0 - r).clamp(0.0, 1.0).powf(2.2);
            let b = (v * 255.0).round() as u8;
            let idx = ((y * s + x) * 4) as usize;
            out[idx] = b;
            out[idx + 1] = b;
            out[idx + 2] = b;
            out[idx + 3] = b;
        }
    }
    out
}
