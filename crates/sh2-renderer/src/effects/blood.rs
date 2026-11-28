//! Subtractive & alpha-blended 3D/floor-decal pipeline for `Effect2/hh_class_blood_*.c`
//! (`SLUS_202.28` `0x00253b00..0x00257750`, `data/pic/effect/bloodtex0.tbn2`).

use bytemuck::{Pod, Zeroable};
use sh2_core::effects::{BloodQuadKind, BloodQuadRenderItem};

/// Maximum simultaneous blood quads (billboards + floor decals) in the reusable GPU buffer.
pub const MAX_GPU_BLOOD_QUADS: usize = 512;

/// Instanced blood quad record for [`BloodEffectPipeline`] (`64` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuBloodQuad {
    /// World-space center `(x, y, z)` in PS2 mm and Y-rotation `yaw_rad` in `w`.
    pub pos_yaw: [f32; 4],
    /// Half-extents `(hx, hz)` in PS2 mm, `is_billboard` (`1.0` or `0.0`) in `z`, `0.0` in `w`.
    pub extents_kind: [f32; 4],
    /// Sub-atlas UV rect `[u_min, v_min, u_max, v_max]` in `bloodtex0.tbn2`.
    pub uv_rect: [f32; 4],
    /// Normalized RGB color and alpha `As` (`alpha_gs / 128.0`) in `w`.
    pub color_alpha: [f32; 4],
}

impl GpuBloodQuad {
    /// Converts a [`BloodQuadRenderItem`] from `sh2-core` into a GPU instance record.
    #[must_use]
    pub fn from_render_item(item: &BloodQuadRenderItem) -> Self {
        let is_billboard = match item.kind {
            BloodQuadKind::Billboard => 1.0,
            BloodQuadKind::FloorDecal => 0.0,
        };
        Self {
            pos_yaw: [
                item.position_mm.x,
                item.position_mm.y,
                item.position_mm.z,
                item.yaw_rad,
            ],
            extents_kind: [
                item.half_extents_mm[0],
                item.half_extents_mm[1],
                is_billboard,
                0.0,
            ],
            uv_rect: item.uv_rect,
            color_alpha: [
                item.color_rgb[0],
                item.color_rgb[1],
                item.color_rgb[2],
                item.alpha,
            ],
        }
    }
}

/// Subtractive (`Cd - Cs * As`, PS2 GS `ALPHA_1 = 0x8000000042`) and non-subtractive fallback
/// pipeline for `Effect2` blood pools, hit sprays, falling drops, and bloody footprints.
pub struct BloodEffectPipeline {
    pub subtractive_pipeline: wgpu::RenderPipeline,
    pub alpha_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl BloodEffectPipeline {
    const WGSL_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

struct FogUniforms {
    fog_color: vec4<f32>,
    fog_near: f32,
    fog_far: f32,
    density: f32,
    time: f32,
    step_count: u32,
    curve_type: u32,
    speed_mult: f32,
    ambient_fog_coupling: f32,
    layer1_uv_scale: vec4<f32>,
    layer2_params: vec4<f32>,
    layer_transparencies: vec4<f32>,
    shadow_view_proj: mat4x4<f32>,
    depth_fog_params: vec4<f32>,
    screen_fog_uv: vec4<f32>,
    wind_vec: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var blood_tex: texture_2d<f32>;
@group(0) @binding(2) var blood_sampler: sampler;
@group(0) @binding(3) var<uniform> fog: FogUniforms;

struct InstanceInput {
    @location(0) pos_yaw: vec4<f32>,
    @location(1) extents_kind: vec4<f32>,
    @location(2) uv_rect: vec4<f32>,
    @location(3) color_alpha: vec4<f32>,
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
    let center = inst.pos_yaw.xyz;
    let yaw = inst.pos_yaw.w;
    let hx = inst.extents_kind.x;
    let hz = inst.extents_kind.y;
    let is_billboard = inst.extents_kind.z > 0.5;

    var world_pos: vec3<f32>;
    if (is_billboard) {
        let raw_to_cam = camera.cam_pos.xyz - center;
        let to_cam_len = length(raw_to_cam);
        let to_cam = select(vec3<f32>(0.0, 0.0, 1.0), raw_to_cam / to_cam_len, to_cam_len > 1e-4);
        let world_up = select(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_cam.y) > 0.99);
        let raw_right = cross(world_up, to_cam);
        let right_len = length(raw_right);
        let right = select(vec3<f32>(1.0, 0.0, 0.0), raw_right / right_len, right_len > 1e-4);
        let up = cross(to_cam, right);
        world_pos = center + right * (c.x * hx) + up * (c.y * hz);
    } else {
        let sin_y = sin(yaw);
        let cos_y = cos(yaw);
        let lx = c.x * hx;
        let lz = c.y * hz;
        let rx = lx * cos_y + lz * sin_y;
        let rz = -lx * sin_y + lz * cos_y;
        // Small -2.0 mm Y-down bias to prevent z-fighting with room floor polygons
        world_pos = vec3<f32>(center.x + rx, center.y - 2.0, center.z + rz);
    }

    let u = mix(inst.uv_rect.x, inst.uv_rect.z, c.x * 0.5 + 0.5);
    let v = mix(inst.uv_rect.y, inst.uv_rect.w, c.y * 0.5 + 0.5);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = vec2<f32>(u, v);

    // PS2 `Object_Draw` (`hh_class_blood_00.c` `0x00253eb0`, `02.c` `0x00254ea0`, `footmark_00.c` `0x00255c20`):
    // scales RGBA by `clamp(FogParameter_A + (1/z) * FogParameter_B, 0, 255) / 255.0`.
    var fog_vis = 1.0;
    let z = out.clip_position.w;
    if (fog.density > 0.0001 && z > 0.001) {
        let persp_fa = fog.depth_fog_params.x;
        let persp_fb = fog.depth_fog_params.y;
        let fog_min = fog.depth_fog_params.z;
        let fog_max = fog.depth_fog_params.w;
        var fog_raw = 0.0;
        if (abs(persp_fa) > 0.0001) {
            fog_raw = clamp(persp_fa / z + persp_fb, fog_min, fog_max);
        } else {
            let span = max(fog.fog_far - fog.fog_near, 1.0);
            fog_raw = clamp((z - fog.fog_near) / span, 0.0, 1.0);
        }
        fog_vis = 1.0 - clamp(fog_raw, 0.0, 1.0) * clamp(fog.density, 0.0, 1.0);
    }
    out.color_alpha = inst.color_alpha * fog_vis;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(blood_tex, blood_sampler, in.uv);
    let alpha = tex.a * in.color_alpha.a;
    if (alpha < 0.004) {
        discard;
    }
    return vec4<f32>(in.color_alpha.rgb * tex.rgb, alpha);
}
"#;

    /// Creates the subtractive and alpha-blended blood rendering pipelines.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Effect2 Blood Quad Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Effect2 Blood Bind Group Layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Effect2 Blood Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Note: `blood_bind_group` binds `camera` (0), `blood_tex` (1), `blood_sampler` (2), `fog` (3).
        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuBloodQuad>() as wgpu::BufferAddress,
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
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }];

        // PS2 GS `ALPHA_1 = 0x8000000042`: `Cd - Cs * As` (ReverseSubtract)
        let subtractive_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::ReverseSubtract,
            },
            alpha: wgpu::BlendComponent::OVER,
        };

        let make_pipeline = |label: &'static str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &vertex_buffers,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
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
            })
        };

        let subtractive_pipeline = make_pipeline(
            "Effect2 Blood Subtractive Pipeline (0x8000000042)",
            subtractive_blend,
        );
        let alpha_pipeline = make_pipeline(
            "Effect2 Blood Alpha Fallback Pipeline",
            wgpu::BlendState::ALPHA_BLENDING,
        );

        Self {
            subtractive_pipeline,
            alpha_pipeline,
            bind_group_layout,
        }
    }
}
