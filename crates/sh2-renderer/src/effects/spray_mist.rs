//! 3D billboard particle pipeline for Hyper Spray & Lying Figure mist clouds
//! (`SLUS_202.28` `enEfctDrawSpray` @ `0x00150000`, `poison.tbn2` / `fog.tex`).

use bytemuck::{Pod, Zeroable};
use sh2_core::lighting::SprayParticle;

/// Maximum simultaneous spray mist billboard instances in the reusable GPU buffer.
pub const MAX_GPU_SPRAY_PARTICLES: usize = sh2_core::lighting::MAX_SPRAY_PARTICLES;

/// Instanced 3D billboard record for [`SprayMistPipeline`] (`32` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuSprayParticle {
    /// World-space center `(x, y, z)` in PS2 mm and billboard radius `w` in mm.
    pub pos_radius: [f32; 4],
    /// Linear RGBA color (reflecting the 4 Hyper Spray tiers + dark-world modulation) and alpha.
    pub color_rgba: [f32; 4],
}

impl From<&SprayParticle> for GpuSprayParticle {
    fn from(p: &SprayParticle) -> Self {
        Self {
            pos_radius: [p.position.x, p.position.y, p.position.z, p.radius_mm],
            color_rgba: [p.color_rgb[0], p.color_rgb[1], p.color_rgb[2], p.alpha],
        }
    }
}

/// Depth-tested, alpha-blended 3D billboard pipeline for Hyper Spray & Lying Figure mist clouds.
pub struct SprayMistPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl SprayMistPipeline {
    const WGSL_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var mist_tex: texture_2d<f32>;
@group(0) @binding(2) var mist_sampler: sampler;

struct InstanceInput {
    @location(0) pos_radius: vec4<f32>,
    @location(1) color_rgba: vec4<f32>,
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
    let c = corners[vid];

    // Camera-facing billboard basis in PS2 Y-down space (guarded against collinear/zero vectors)
    let raw_to_cam = camera.cam_pos.xyz - inst.pos_radius.xyz;
    let to_cam_len = length(raw_to_cam);
    let to_cam = select(vec3<f32>(0.0, 0.0, 1.0), raw_to_cam / to_cam_len, to_cam_len > 1e-4);
    let world_up = select(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_cam.y) > 0.99);
    let raw_right = cross(world_up, to_cam);
    let right_len = length(raw_right);
    let right = select(vec3<f32>(1.0, 0.0, 0.0), raw_right / right_len, right_len > 1e-4);
    let up = cross(to_cam, right);

    let r = inst.pos_radius.w;
    let world_pos = inst.pos_radius.xyz + (right * c.x + up * c.y) * r;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = c * 0.5 + vec2<f32>(0.5, 0.5);
    out.color = inst.color_rgba;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(mist_tex, mist_sampler, in.uv);
    let dist = length(in.uv * 2.0 - vec2<f32>(1.0, 1.0));
    let radial = smoothstep(1.0, 0.15, dist);
    let alpha = tex.a * radial * in.color.a;
    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(tex.rgb * in.color.rgb, alpha);
}
"#;

    /// Creates the Hyper Spray / mist billboard render pipeline.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        hdr_color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hyper Spray Mist Billboard Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hyper Spray Mist Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Hyper Spray Mist Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuSprayParticle>() as u64,
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
            ],
        }];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Hyper Spray Mist Pipeline"),
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }
}
