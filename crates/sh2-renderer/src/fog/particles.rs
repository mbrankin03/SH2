//! Instanced camera-facing billboard renderer for localized 3D proximity fog particles
//! (`fogMakePacket` @ `0x0016bb70` in `SLUS_202.28`, `ObservationDeckFogFix.cpp`) (`SH2-32`).

use bytemuck::{Pod, Zeroable};

/// Maximum GPU fog particle capacity matching `kFogParticleCountMax` (`ObservationDeckFogFix.cpp:27`).
pub const MAX_GPU_FOG_PARTICLES: usize = 2000;

/// Per-instance GPU data for one localized 3D proximity fog sprite (32 bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuFogParticle {
    /// World-space center `(x, y, z)` in PS2 mm and billboard half-radius in `w`.
    pub world_pos_radius: [f32; 4],
    /// Particle mist tint RGB (`xyz`) and evaluated border/parting alpha (`w` in `0.0..=1.0`).
    pub color_alpha: [f32; 4],
}

impl Default for GpuFogParticle {
    fn default() -> Self {
        Self {
            world_pos_radius: [0.0, 0.0, 0.0, 300.0],
            color_alpha: [0.65, 0.68, 0.72, 0.0],
        }
    }
}

/// Instanced billboard render pipeline for localized 3D proximity fog sprites.
pub struct FogParticlePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl FogParticlePipeline {
    const WGSL_PARTICLE_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var fog_tex: texture_2d<f32>;
@group(0) @binding(2) var fog_sampler: sampler;

struct InstanceInput {
    @location(0) world_pos_radius: vec4<f32>,
    @location(1) color_alpha: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color_alpha: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    inst: InstanceInput,
) -> VertexOutput {
    // Two-triangle billboard quad corners in [-1, 1]
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    let corner = corners[vid % 6u];

    // Camera-facing basis vectors with near-zero distance and pole-alignment NaN guards
    let center = inst.world_pos_radius.xyz;
    let radius = inst.world_pos_radius.w;
    let to_cam_raw = camera.cam_pos.xyz - center;
    let to_cam_len = length(to_cam_raw);
    let to_cam = select(vec3<f32>(0.0, 0.0, 1.0), to_cam_raw / to_cam_len, to_cam_len > 1e-4);
    let ref_up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_cam.y) > 0.99);
    let right = normalize(cross(ref_up, to_cam));
    let up = cross(to_cam, right);

    let world_vertex = center + (right * corner.x + up * corner.y) * radius;
    // Fade smoothly as particles approach the camera lens so billboards never clip or pop
    let near_fade = clamp((to_cam_len - 350.0) / 1100.0, 0.0, 1.0);

    var out: VertexOutput;
    out.clip_pos = camera.view_proj * vec4<f32>(world_vertex, 1.0);
    out.uv = corner * 0.5 + 0.5;
    out.color_alpha = vec4<f32>(inst.color_alpha.rgb, inst.color_alpha.a * near_fade);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let centered = in.uv * 2.0 - 1.0;
    let r2 = dot(centered, centered);
    if (r2 >= 1.0) {
        discard;
    }
    let falloff = 1.0 - r2;
    let radial = falloff * falloff;
    let texel = textureSample(fog_tex, fog_sampler, in.uv);
    // Exact `sh2pc.exe` `FUN_00486fc0` (`0x00486fc0`, `sh2pc_00480000.c:4285-4290`):
    // `D3DTSS_COLOROP = D3DTOP_MODULATE2X (5)` with `diffuse.rgb = 0x80/255` -> `texel.rgb * in.color_alpha.rgb`
    // (preserving `fog.tex`'s ~0.51 soft-gray mist tone instead of pure white),
    // and `D3DTSS_ALPHAOP = D3DTOP_MODULATE (4)` with `D3DBLEND_SRCALPHA` (`48/255` on `ob01`).
    let alpha = radial * texel.r * in.color_alpha.a;
    if (alpha < 0.003) {
        discard;
    }
    return vec4<f32>(in.color_alpha.rgb * texel.rgb, alpha);
}
"#;

    /// Creates the instanced alpha-blended billboard pipeline for localized 3D proximity fog sprites.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fog Particle Billboard Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_PARTICLE_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fog Particle Bind Group Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fog Particle Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_layout = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuFogParticle>() as wgpu::BufferAddress,
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
            label: Some("Fog Particle Billboard Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &instance_layout,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_fog_particle_layout() {
        assert_eq!(std::mem::size_of::<GpuFogParticle>(), 32);
        assert_eq!(std::mem::align_of::<GpuFogParticle>(), 4);
    }
}
