//! 3D billboard particle pipeline for `Effect2/hh_class_dust_00.c` airborne dust motes
//! (`SLUS_202.28` `0x00257b30..0x00257f30`, `data/pic/effect/playable0.tbn2`).

use bytemuck::{Pod, Zeroable};
use sh2_core::lighting::{
    DustMote, DustSpotlightParams, DUST_BASE_HALF_EXTENT_MM, DUST_BASE_RGB_NORM, DUST_UV_QUADRANTS,
    MAX_DUST_MOTES,
};

/// Maximum simultaneous airborne dust mote billboard instances in the reusable GPU buffer.
pub const MAX_GPU_DUST_PARTICLES: usize = MAX_DUST_MOTES;

/// Instanced 3D billboard record for [`DustParticlePipeline`] (`48` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuDustParticle {
    /// World-space center `(x, y, z)` in PS2 mm and billboard half-extent (`200.0 * scale`) in `w`.
    pub pos_half_extent: [f32; 4],
    /// Sub-atlas UV rect `[u_min, v_min, u_max, v_max]` from `_square_0x_stq_list` (`0x0036e890`).
    pub uv_rect: [f32; 4],
    /// Base dust RGB (`_85` @ `0x0036e900`) and evaluated GS alpha (`alpha_gs / 128.0`) in `w`.
    pub color_alpha: [f32; 4],
}

impl GpuDustParticle {
    /// Converts a simulated [`DustMote`] into a GPU billboard instance using `spot` illumination.
    #[must_use]
    pub fn from_mote(mote: &DustMote, spot: &DustSpotlightParams) -> Self {
        let uv = DUST_UV_QUADRANTS[usize::from(mote.quadrant & 3)];
        let alpha = mote.eval_alpha_norm(spot);
        Self {
            pos_half_extent: [
                mote.world_pos.x,
                mote.world_pos.y,
                mote.world_pos.z,
                DUST_BASE_HALF_EXTENT_MM * mote.scale,
            ],
            uv_rect: uv,
            color_alpha: [
                DUST_BASE_RGB_NORM[0],
                DUST_BASE_RGB_NORM[1],
                DUST_BASE_RGB_NORM[2],
                alpha,
            ],
        }
    }
}

/// Depth-tested, alpha-blended 3D billboard pipeline for `hh_class_dust_00.c` airborne dust motes.
pub struct DustParticlePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl DustParticlePipeline {
    const WGSL_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var dust_tex: texture_2d<f32>;
@group(0) @binding(2) var dust_sampler: sampler;

struct InstanceInput {
    @location(0) pos_half_extent: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) color_alpha: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color_alpha: vec4<f32>,
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

    // Camera-facing billboard basis (`HH_ClassWrapper_AlwaysFront_WorldView_Matrix_Get`)
    let raw_to_cam = camera.cam_pos.xyz - inst.pos_half_extent.xyz;
    let to_cam_len = length(raw_to_cam);
    let to_cam = select(vec3<f32>(0.0, 0.0, 1.0), raw_to_cam / to_cam_len, to_cam_len > 1e-4);
    let world_up = select(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_cam.y) > 0.99);
    let raw_right = cross(world_up, to_cam);
    let right_len = length(raw_right);
    let right = select(vec3<f32>(1.0, 0.0, 0.0), raw_right / right_len, right_len > 1e-4);
    let up = cross(to_cam, right);

    let r = inst.pos_half_extent.w;
    let world_pos = inst.pos_half_extent.xyz + (right * c.x + up * c.y) * r;

    let u = mix(inst.uv_rect.x, inst.uv_rect.z, c.x * 0.5 + 0.5);
    let v = mix(inst.uv_rect.y, inst.uv_rect.w, c.y * 0.5 + 0.5);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = vec2<f32>(u, v);
    out.color_alpha = inst.color_alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(dust_tex, dust_sampler, in.uv);
    let alpha = tex.a * in.color_alpha.a;
    if (alpha < 0.004) {
        discard;
    }
    return vec4<f32>(tex.rgb * in.color_alpha.rgb, alpha);
}
"#;

    /// Creates the airborne dust mote billboard render pipeline.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        hdr_color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Airborne Dust Mote Billboard Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Airborne Dust Mote Bind Group Layout"),
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
            label: Some("Airborne Dust Mote Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuDustParticle>() as u64,
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

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Airborne Dust Mote Pipeline"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_dust_particle_layout() {
        assert_eq!(std::mem::size_of::<GpuDustParticle>(), 48);
        assert_eq!(std::mem::align_of::<GpuDustParticle>(), 4);
    }
}
