//! Unified instanced 3D billboard & oriented-quad GPU pipeline for `SH2-100` effects:
//! - Angela's staircase fire (`fire00.tbn2`, `flame003.tbn2`) & smoke (`ef_smoke.c`)
//! - Flooded corridor ripples (`water00.tbn2`..`water40.tbn2`), splashes (`sprash.tbn2`), rain streaks
//! - Brass shell casings (`ef_cartridge.c`), metallic impact sparks (`spark.tbn2`), glass shards (`glas03.tbn2`)

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use sh2_core::effects::{
    FireEmber, FlameParticle, GlassShard, ImpactSpark, RainDrop, ShellCasing, SmokeParticle,
    WaterPoolRipple, FLAME_UV_QUADRANTS, RAIN_STREAK_LEN_MM, WATER_POOL_UV_GRID,
};

/// Maximum simultaneous `SH2-100` FX quads (additive + alpha) in the reusable GPU buffer.
pub const MAX_GPU_FX_QUADS: usize = 2048;

/// Instanced 3D billboard / oriented-quad record for [`BillboardFxPipeline`] (`80` bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuFxQuad {
    /// World-space center `(x, y, z)` in PS2 mm and mode in `w`:
    /// `0.0` = camera-facing billboard (`basis_x.xy` = `[half_w, half_h]`),
    /// `1.0` = explicit 3D oriented quad (`basis_x.xyz` = right * half_w, `basis_y.xyz` = up * half_h).
    pub pos_mode: [f32; 4],
    /// Billboard half-extents `[half_w, half_h, 0, 0]` (mode 0) or world right vector (mode 1).
    pub basis_x: [f32; 4],
    /// Unused (mode 0) or world up vector (mode 1).
    pub basis_y: [f32; 4],
    /// Sub-atlas UV rect `[u_min, v_min, u_max, v_max]`.
    pub uv_rect: [f32; 4],
    /// Linear RGBA color & opacity `[r, g, b, a]` in `[0.0, 1.0]`.
    pub color_alpha: [f32; 4],
}

impl GpuFxQuad {
    /// Converts a [`FlameParticle`] (`hh_class_flame_00..03.c`) into an additive billboard quad.
    #[must_use]
    pub fn from_flame(origin: Vec3, p: &FlameParticle) -> Self {
        let pos = origin + p.local_pos + p.world_offset;
        Self {
            pos_mode: [pos.x, pos.y, pos.z, 0.0],
            basis_x: [p.scale, p.scale, 0.0, 0.0],
            basis_y: [0.0; 4],
            uv_rect: FLAME_UV_QUADRANTS[usize::from(p.quadrant & 3)],
            color_alpha: [
                (p.rgba[0] / 255.0).clamp(0.0, 1.0),
                (p.rgba[1] / 255.0).clamp(0.0, 1.0),
                (p.rgba[2] / 255.0).clamp(0.0, 1.0),
                (p.rgba[3] / 128.0).clamp(0.0, 1.0),
            ],
        }
    }

    /// Converts a native-upgrade [`FireEmber`] into a glowing additive billboard quad.
    #[must_use]
    pub fn from_ember(e: &FireEmber) -> Self {
        let alpha = (1.0 - e.age / e.life.max(1e-4)).clamp(0.0, 1.0);
        Self {
            pos_mode: [e.pos.x, e.pos.y, e.pos.z, 0.0],
            basis_x: [e.size_mm, e.size_mm, 0.0, 0.0],
            basis_y: [0.0; 4],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            color_alpha: [1.0, 0.55, 0.12, alpha],
        }
    }

    /// Converts a [`SmokeParticle`] (`ef_smoke.c`) into an alpha-blended billboard quad.
    #[must_use]
    pub fn from_smoke(p: &SmokeParticle) -> Self {
        Self {
            pos_mode: [p.pos.x, p.pos.y, p.pos.z, 0.0],
            basis_x: [p.size_x * 0.5, p.size_y * 0.5, 0.0, 0.0],
            basis_y: [0.0; 4],
            uv_rect: p.uv_rect,
            color_alpha: [
                (p.rgba[0] / 128.0).clamp(0.0, 1.0),
                (p.rgba[1] / 128.0).clamp(0.0, 1.0),
                (p.rgba[2] / 128.0).clamp(0.0, 1.0),
                (p.rgba[3] / 128.0).clamp(0.0, 1.0),
            ],
        }
    }

    /// Converts a [`WaterPoolRipple`] (`hh_class_water_pool_00.c`) into a horizontal XZ floor quad.
    #[must_use]
    pub fn from_water_ripple(r: &WaterPoolRipple) -> Self {
        let (s, c) = r.yaw_rad.sin_cos();
        let hw = r.scale_mm[0] * 0.5;
        let hd = r.scale_mm[2] * 0.5;
        Self {
            pos_mode: [r.pos.x, r.pos.y - 4.0, r.pos.z, 1.0],
            basis_x: [c * hw, 0.0, s * hw, 0.0],
            basis_y: [-s * hd, 0.0, c * hd, 0.0],
            uv_rect: WATER_POOL_UV_GRID[usize::from(r.quadrant & 15)],
            color_alpha: [
                (r.rgba[0] / 255.0).clamp(0.0, 1.0),
                (r.rgba[1] / 255.0).clamp(0.0, 1.0),
                (r.rgba[2] / 255.0).clamp(0.0, 1.0),
                (r.rgba[3] / 255.0).clamp(0.0, 1.0),
            ],
        }
    }

    /// Converts a [`RainDrop`] (`ef_rain.c`) into a thin vertical streak quad.
    #[must_use]
    pub fn from_rain_drop(d: &RainDrop) -> Self {
        Self {
            pos_mode: [d.pos.x, d.pos.y + RAIN_STREAK_LEN_MM * 0.5, d.pos.z, 0.0],
            basis_x: [2.5, RAIN_STREAK_LEN_MM * 0.5, 0.0, 0.0],
            basis_y: [0.0; 4],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            color_alpha: [0.55, 0.62, 0.72, 0.45],
        }
    }

    /// Converts a [`ShellCasing`] (`ef_cartridge.c`) into a spinning brass 3D quad.
    #[must_use]
    pub fn from_shell(s: &ShellCasing) -> Self {
        let (sx, cx) = s.rot_rad.x.sin_cos();
        let (sz, cz) = s.rot_rad.z.sin_cos();
        let right = Vec3::new(cz * 14.0, sz * 14.0, 0.0);
        let up = Vec3::new(-sz * cx * 6.0, cz * cx * 6.0, sx * 6.0);
        Self {
            pos_mode: [s.pos.x, s.pos.y, s.pos.z, 1.0],
            basis_x: [right.x, right.y, right.z, 0.0],
            basis_y: [up.x, up.y, up.z, 0.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            color_alpha: [0.88, 0.72, 0.28, 1.0],
        }
    }

    /// Converts an [`ImpactSpark`] (`ef_sparks.c`) into an additive spark billboard (`spark.tbn2`).
    #[must_use]
    pub fn from_spark(s: &ImpactSpark) -> Self {
        Self {
            pos_mode: [s.pos.x, s.pos.y, s.pos.z, 0.0],
            basis_x: [s.size_w * 0.5, s.size_h * 0.5, 0.0, 0.0],
            basis_y: [0.0; 4],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            color_alpha: [
                (s.rgba[0] / 128.0).clamp(0.0, 1.0),
                (s.rgba[1] / 128.0).clamp(0.0, 1.0),
                (s.rgba[2] / 128.0).clamp(0.0, 1.0),
                (s.rgba[3] / 128.0).clamp(0.0, 1.0),
            ],
        }
    }

    /// Converts a [`GlassShard`] (`hh_class_glass_piece_00.c`) into a tumbling translucent quad (`glas03.tbn2`).
    #[must_use]
    pub fn from_glass_shard(g: &GlassShard) -> Self {
        let rx = g.spin_x * g.age;
        let ry = g.spin_y * g.age;
        let (sx, cx) = rx.sin_cos();
        let (sy, cy) = ry.sin_cos();
        let right = Vec3::new(cy * 26.0, 0.0, -sy * 26.0);
        let up = Vec3::new(sy * sx * 26.0, cx * 26.0, cy * sx * 26.0);
        Self {
            pos_mode: [g.pos.x, g.pos.y, g.pos.z, 1.0],
            basis_x: [right.x, right.y, right.z, 0.0],
            basis_y: [up.x, up.y, up.z, 0.0],
            uv_rect: g.uv_rect,
            color_alpha: [0.85, 0.92, 1.0, 0.72],
        }
    }
}

/// Depth-tested additive & alpha-blended 3D effect quad pipeline (`SH2-100`).
pub struct BillboardFxPipeline {
    pub pipeline_additive: wgpu::RenderPipeline,
    pub pipeline_alpha: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl BillboardFxPipeline {
    const WGSL_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var fx_tex: texture_2d<f32>;
@group(0) @binding(2) var fx_sampler: sampler;

struct InstanceInput {
    @location(0) pos_mode: vec4<f32>,
    @location(1) basis_x: vec4<f32>,
    @location(2) basis_y: vec4<f32>,
    @location(3) uv_rect: vec4<f32>,
    @location(4) color_alpha: vec4<f32>,
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

    var offset: vec3<f32>;
    if (inst.pos_mode.w < 0.5) {
        let raw_to_cam = camera.cam_pos.xyz - inst.pos_mode.xyz;
        let to_cam_len = length(raw_to_cam);
        let to_cam = select(vec3<f32>(0.0, 0.0, 1.0), raw_to_cam / to_cam_len, to_cam_len > 1e-4);
        let world_up = select(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(to_cam.y) > 0.99);
        let raw_right = cross(world_up, to_cam);
        let right_len = length(raw_right);
        let right = select(vec3<f32>(1.0, 0.0, 0.0), raw_right / right_len, right_len > 1e-4);
        let up = cross(to_cam, right);
        offset = right * (c.x * inst.basis_x.x) + up * (c.y * inst.basis_x.y);
    } else {
        offset = inst.basis_x.xyz * c.x + inst.basis_y.xyz * c.y;
    }

    let world_pos = inst.pos_mode.xyz + offset;
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
    let tex = textureSample(fx_tex, fx_sampler, in.uv);
    let alpha = tex.a * in.color_alpha.a;
    if (alpha < 0.004) {
        discard;
    }
    return vec4<f32>(tex.rgb * in.color_alpha.rgb, alpha);
}
"#;

    /// Builds both additive and alpha-blended FX pipelines sharing the same shader and layout.
    pub fn new(
        device: &wgpu::Device,
        hdr_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Billboard FX Shader (SH2-100)"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Billboard FX Bind Group Layout"),
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
            label: Some("Billboard FX Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let make_pipeline = |label: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GpuFxQuad>() as wgpu::BufferAddress,
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
                            wgpu::VertexAttribute {
                                offset: 64,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                        ],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
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

        let additive_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };

        Self {
            pipeline_additive: make_pipeline("Billboard FX Additive Pipeline", additive_blend),
            pipeline_alpha: make_pipeline(
                "Billboard FX Alpha Pipeline",
                wgpu::BlendState::ALPHA_BLENDING,
            ),
            bind_group_layout,
        }
    }
}
