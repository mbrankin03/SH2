//! PS2 4-pass ping-pong screen-space soft stencil shadow blur & dual GS composite blend modes
//! (`SLUS_202.28` `GFW/sh2_shadow_filters.c`).
//!
//! Ground-truth references (`SLUS_202.28`):
//! - `sh2gfw_Set_FilterData` (`0x00175f90`): selects dual GS `ALPHA_1` / `TEST_1` composite modes:
//!   - Mode 0 (`param_1 == 0`): `ALPHA_1 = 0x8000000052` (`(0 - Cs) * Ad + Cd = Cd - Cs * Ad`),
//!     `TEST_1 = 0x30000`.
//!   - Mode 1 (`param_1 != 0`): `ALPHA_1 = 0x8000000046` (`(0 - Cd) * As + Cd = Cd * (1 - As)`),
//!     `TEST_1 = 0x30002`.
//! - `Get_ShadowColor` (`0x00176040`) & `sh2gfw_Set_ShadowColorScaleFactor` (`0x00176130`, `1.0` = `0x3f800000`).
//! - `Get_NowRoomShadowColor` (`0x00250ee0`): default room shadow density `0x50` (`80`) when `-1`.
//! - `shdw_shadow_filter_main` (`0x001767e0`): 4-pass half-resolution (`256x256`, GS `0x7800..0x8800`)
//!   bilinear ping-pong blur (`shdw_StencilBuf2Work` `0x00176550` -> 3× `shdw_Work2Work` `0x001766a0`
//!   with alternating UV flips to cancel half-texel drift) followed by `shdw_Composite_Shadow` (`0x001761e0`).

use bytemuck::{Pod, Zeroable};

/// PS2 GS `ALPHA_1` register value for Day / subtractive shadow compositing
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 == 0`).
///
/// Bit decode (`(A - B) * C >> 7 + D`): `A = 2 (0)`, `B = 0 (Cs)`, `C = 1 (Ad)`, `D = 1 (Cd)`,
/// `FIX = 0x80` -> `Cd - Cs * Ad`.
pub const GS_SHADOW_ALPHA_SUBTRACTIVE: u64 = 0x0000_0080_0000_0052;

/// PS2 GS `TEST_1` register value paired with [`GS_SHADOW_ALPHA_SUBTRACTIVE`]
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 == 0`).
pub const GS_SHADOW_TEST_SUBTRACTIVE: u64 = 0x0003_0000;

/// PS2 GS `ALPHA_1` register value for Night / multiplicative shadow compositing
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 != 0`).
///
/// Bit decode (`(A - B) * C >> 7 + D`): `A = 2 (0)`, `B = 1 (Cd)`, `C = 0 (As)`, `D = 1 (Cd)`,
/// `FIX = 0x80` -> `Cd * (1 - As)`.
pub const GS_SHADOW_ALPHA_MULTIPLICATIVE: u64 = 0x0000_0080_0000_0046;

/// PS2 GS `TEST_1` register value paired with [`GS_SHADOW_ALPHA_MULTIPLICATIVE`]
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 != 0`).
pub const GS_SHADOW_TEST_MULTIPLICATIVE: u64 = 0x0003_0002;

/// Number of bilinear filter passes executed by `shdw_shadow_filter_main` (`0x001767e0`):
/// 1× `shdw_StencilBuf2Work` (`0x00176550`) + 3× `shdw_Work2Work` (`0x001766a0`).
pub const PS2_SHADOW_BLUR_PASS_COUNT: usize = 4;

/// PS2 shadow filter work buffer dimension in pixels (`0x8800 - 0x7800 = 0x1000` in 12.4 fixed-point = `256` px;
/// `SLUS_202.28` `shdw_StencilBuf2Work` `0x00176550`, `shdw_Work2Work` `0x001766a0`).
pub const PS2_SHADOW_WORK_SIZE: u32 = 256;

/// Default PS2 room shadow density (`0x50 = 80`) returned by `Get_NowRoomShadowColor` (`0x00250ee0`)
/// when the room's `DrawEnvData` field at `+0x0a` is `-1`.
pub const PS2_DEFAULT_ROOM_SHADOW_COLOR: u8 = 0x50;

/// PS2 GS neutral modulation scale (`0x80 = 128` represents `1.0` in GS RGBAQ modulation;
/// `SLUS_202.28` `shdw_clear` `0x00176440`, `shdw_Work2Work` `0x001766a0`).
pub const PS2_GS_NEUTRAL_RGB: f32 = 128.0;

/// Dual PS2 GS stencil shadow composite blend modes (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowCompositeMode {
    /// `param_1 == 0`: `ALPHA_1 = 0x8000000052`, `TEST_1 = 0x30000` (`Cd - Cs * Ad`).
    Subtractive,
    /// `param_1 != 0`: `ALPHA_1 = 0x8000000046`, `TEST_1 = 0x30002` (`Cd * (1 - As)`).
    Multiplicative,
}

impl ShadowCompositeMode {
    /// Selects the GS shadow composite mode from `sh2gfw_Set_FilterData(param_1)` (`0x00175f90`).
    #[must_use]
    pub const fn from_filter_param(param_1: i32) -> Self {
        if param_1 == 0 {
            Self::Subtractive
        } else {
            Self::Multiplicative
        }
    }

    /// Returns the exact 64-bit PS2 GS `ALPHA_1` (`0x42`) register value (`0x00175f90`).
    #[must_use]
    pub const fn gs_alpha_1(self) -> u64 {
        match self {
            Self::Subtractive => GS_SHADOW_ALPHA_SUBTRACTIVE,
            Self::Multiplicative => GS_SHADOW_ALPHA_MULTIPLICATIVE,
        }
    }

    /// Returns the exact 64-bit PS2 GS `TEST_1` (`0x47`) register value (`0x00175f90`).
    #[must_use]
    pub const fn gs_test_1(self) -> u64 {
        match self {
            Self::Subtractive => GS_SHADOW_TEST_SUBTRACTIVE,
            Self::Multiplicative => GS_SHADOW_TEST_MULTIPLICATIVE,
        }
    }

    /// Evaluates the exact PS2 GS composite equation for a single RGB pixel (`dst_rgb`),
    /// blurred shadow mask coverage (`mask` in `0.0..=1.0`), and room shadow color (`shadow_rgb`,
    /// `shadow_alpha` in `0.0..=1.0` where `0x80 / 128.0 = 1.0`).
    #[must_use]
    pub fn composite_rgb(
        self,
        dst_rgb: [f32; 3],
        dst_alpha: f32,
        shadow_rgb: [f32; 3],
        shadow_alpha: f32,
        mask: f32,
    ) -> [f32; 3] {
        let m = mask.clamp(0.0, 1.0);
        match self {
            // ALPHA_1 = 0x8000000052: (0 - Cs) * Ad + Cd = Cd - Cs * Ad
            Self::Subtractive => {
                let ad = dst_alpha.clamp(0.0, 1.0);
                [
                    (dst_rgb[0] - shadow_rgb[0] * m * ad).max(0.0),
                    (dst_rgb[1] - shadow_rgb[1] * m * ad).max(0.0),
                    (dst_rgb[2] - shadow_rgb[2] * m * ad).max(0.0),
                ]
            }
            // ALPHA_1 = 0x8000000046: (0 - Cd) * As + Cd = Cd * (1 - As)
            Self::Multiplicative => {
                let factor = (1.0 - (shadow_alpha * m).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                [
                    dst_rgb[0] * factor,
                    dst_rgb[1] * factor,
                    dst_rgb[2] * factor,
                ]
            }
        }
    }

    /// Returns the `wgpu::BlendState` implementing this GS `ALPHA_1` composite mode.
    #[must_use]
    pub const fn wgpu_blend_state(self) -> wgpu::BlendState {
        match self {
            // Cd - Cs * Ad (approximated with SrcAlpha * src subtracted from dst)
            Self::Subtractive => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::ReverseSubtract,
                },
                alpha: wgpu::BlendComponent::OVER,
            },
            // Cd * (1 - As)
            Self::Multiplicative => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            },
        }
    }
}

/// Evaluates `Get_ShadowColor` (`0x00176040`) with `sh2gfw_Set_ShadowColorScaleFactor` (`0x00176130`).
///
/// On PS2 (`0x00176040`), the shadow RGB channels all take `(u32)(base_r * scale)` and the alpha
/// channel takes `(u32)(base_a * scale)`.
#[must_use]
pub fn evaluate_ps2_shadow_color(base_r: u32, base_a: u32, scale_factor: f32) -> [u32; 4] {
    let rgb = ((base_r as f32) * scale_factor).max(0.0) as u32;
    let a = ((base_a as f32) * scale_factor).max(0.0) as u32;
    [rgb, rgb, rgb, a]
}

/// Uniform block for a single `ShadowBlurPipeline` pass (`16` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct ShadowBlurUniforms {
    /// `(texel_offset_x, texel_offset_y, shadow_opacity, uv_flip_sign)`.
    ///
    /// `uv_flip_sign` alternates per pass (`+1.0` / `-1.0`) matching `shdw_Work2Work` (`0x001766a0`)
    /// so the bilinear half-texel offset blurs symmetrically without net shadow translation.
    pub params: [f32; 4],
}

impl Default for ShadowBlurUniforms {
    fn default() -> Self {
        Self {
            params: [
                0.5 / (PS2_SHADOW_WORK_SIZE as f32),
                0.5 / (PS2_SHADOW_WORK_SIZE as f32),
                (PS2_DEFAULT_ROOM_SHADOW_COLOR as f32) / PS2_GS_NEUTRAL_RGB,
                1.0,
            ],
        }
    }
}

/// GPU 4-pass ping-pong screen-space shadow blur & dual-mode composite pipeline
/// (`SLUS_202.28` `shdw_shadow_filter_main` `0x001767e0`, `shdw_Composite_Shadow` `0x001761e0`).
pub struct ShadowBlurPipeline {
    /// Extracts binary stencil mask (`stencil != 0`) into a `Rgba8Unorm` shadow mask target.
    pub mask_extract_pipeline: wgpu::RenderPipeline,
    /// Single bilinear tent blur pass (`shdw_StencilBuf2Work` / `shdw_Work2Work`).
    pub blur_pass_pipeline: wgpu::RenderPipeline,
    /// Full-screen composite pipeline for `ShadowCompositeMode::Multiplicative` (`ALPHA_1 = 0x8000000046`).
    pub composite_mul_pipeline: wgpu::RenderPipeline,
    /// Full-screen composite pipeline for `ShadowCompositeMode::Subtractive` (`ALPHA_1 = 0x8000000052`).
    pub composite_sub_pipeline: wgpu::RenderPipeline,
    /// Bind group layout (`@binding(0)` source texture, `@binding(1)` linear sampler, `@binding(2)` uniforms).
    pub bind_group_layout: wgpu::BindGroupLayout,
    /// Linear clamp sampler matching PS2 GS `TEX1 = 0x60` (`MMAG=1, MMIN=1`).
    pub linear_sampler: wgpu::Sampler,
    /// Uniform buffer for blur pass parameters.
    pub uniform_buffer: wgpu::Buffer,
    /// Ping-pong work texture A (`256x256` `Rgba8Unorm`, PS2 Filter FBP 0).
    pub work_tex_a: wgpu::Texture,
    pub work_view_a: wgpu::TextureView,
    pub work_bind_group_a: wgpu::BindGroup,
    /// Ping-pong work texture B (`256x256` `Rgba8Unorm`, PS2 Filter FBP 1).
    pub work_tex_b: wgpu::Texture,
    pub work_view_b: wgpu::TextureView,
    pub work_bind_group_b: wgpu::BindGroup,
}

impl ShadowBlurPipeline {
    /// Texture format for the 256×256 ping-pong shadow work buffers.
    pub const WORK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    const WGSL_SHADER: &'static str = r#"
struct BlurUniforms {
    params: vec4<f32>,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> u_blur: BlurUniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    let x = f32((vid << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vid & 2u) * 2.0 - 1.0;
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

@fragment
fn fs_mask_extract() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

// PS2 GS bilinear tent filter (`shdw_Work2Work` 0x001766a0):
// 4 symmetric half-texel bilinear taps emulate the GS sub-pixel bilinear copy without net drift.
@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    let off = u_blur.params.xy;
    let s0 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-off.x, -off.y)).r;
    let s1 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>( off.x, -off.y)).r;
    let s2 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-off.x,  off.y)).r;
    let s3 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>( off.x,  off.y)).r;
    let m = (s0 + s1 + s2 + s3) * 0.25;
    return vec4<f32>(m, m, m, m);
}

// `shdw_Composite_Shadow` (0x001761e0) -> RGB/Alpha scaled by room shadow density.
@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    let mask = textureSample(src_tex, src_sampler, in.uv).r;
    let a = clamp(mask * u_blur.params.z, 0.0, 1.0);
    return vec4<f32>(a, a, a, a);
}
"#;

    /// Creates the 4-pass PS2 stencil shadow blur and dual-mode composite pipelines.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        hdr_color_format: wgpu::TextureFormat,
        stencil_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PS2 Soft Stencil Shadow Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PS2 Shadow Blur Bind Group Layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PS2 Shadow Blur Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let empty_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PS2 Shadow Mask Extract Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let mask_extract_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PS2 Stencil Mask Extract Pipeline"),
                layout: Some(&empty_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask_extract"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Self::WORK_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: stencil_format,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::NotEqual,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::NotEqual,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        read_mask: 0xFF,
                        write_mask: 0x00,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let blur_pass_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PS2 Shadow Blur Ping-Pong Pass Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_blur"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: Self::WORK_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let make_composite = |label: &str, mode: ShadowCompositeMode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_composite"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_color_format,
                        blend: Some(mode.wgpu_blend_state()),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        let composite_mul_pipeline = make_composite(
            "PS2 Shadow Composite Multiplicative (0x8000000046)",
            ShadowCompositeMode::Multiplicative,
        );
        let composite_sub_pipeline = make_composite(
            "PS2 Shadow Composite Subtractive (0x8000000052)",
            ShadowCompositeMode::Subtractive,
        );

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PS2 Shadow Blur Bilinear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PS2 Shadow Blur Uniform Buffer"),
            size: std::mem::size_of::<ShadowBlurUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let make_work_tex = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: PS2_SHADOW_WORK_SIZE,
                    height: PS2_SHADOW_WORK_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::WORK_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        let work_tex_a = make_work_tex("PS2 Shadow Blur Work A (FBP 0)");
        let work_view_a = work_tex_a.create_view(&wgpu::TextureViewDescriptor::default());
        let work_tex_b = make_work_tex("PS2 Shadow Blur Work B (FBP 1)");
        let work_view_b = work_tex_b.create_view(&wgpu::TextureViewDescriptor::default());

        let make_bg = |label: &str, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            })
        };

        let work_bind_group_a = make_bg("PS2 Shadow Blur Bind Group A", &work_view_a);
        let work_bind_group_b = make_bg("PS2 Shadow Blur Bind Group B", &work_view_b);

        Self {
            mask_extract_pipeline,
            blur_pass_pipeline,
            composite_mul_pipeline,
            composite_sub_pipeline,
            bind_group_layout,
            linear_sampler,
            uniform_buffer,
            work_tex_a,
            work_view_a,
            work_bind_group_a,
            work_tex_b,
            work_view_b,
            work_bind_group_b,
        }
    }

    /// Updates the blur uniforms for the current frame (`shadow_opacity` in `0.0..=1.0`).
    pub fn update_uniforms(&self, queue: &wgpu::Queue, shadow_opacity: f32) {
        let u = ShadowBlurUniforms {
            params: [
                0.5 / (PS2_SHADOW_WORK_SIZE as f32),
                0.5 / (PS2_SHADOW_WORK_SIZE as f32),
                shadow_opacity.clamp(0.0, 1.0),
                1.0,
            ],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&u));
    }

    /// Encodes the 4-pass ping-pong screen-space blur (`shdw_shadow_filter_main` `0x001767e0`)
    /// and composites the softened shadow mask onto `hdr_color_view` (`shdw_Composite_Shadow` `0x001761e0`).
    ///
    /// Pass sequence:
    /// 1. `src_mask_bind_group` (full-res extracted stencil mask) -> `work_view_a` (256×256)
    /// 2. `work_view_a` -> `work_view_b`
    /// 3. `work_view_b` -> `work_view_a`
    /// 4. `work_view_a` -> `work_view_b`
    /// 5. `work_view_b` -> `hdr_color_view` with `mode` (`0x8000000052` or `0x8000000046`)
    pub fn encode_blur_and_composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src_mask_bind_group: &wgpu::BindGroup,
        hdr_color_view: &wgpu::TextureView,
        mode: ShadowCompositeMode,
    ) {
        let passes: [(&wgpu::BindGroup, &wgpu::TextureView, &str); PS2_SHADOW_BLUR_PASS_COUNT] = [
            (
                src_mask_bind_group,
                &self.work_view_a,
                "PS2 Shadow Blur Pass 1 (StencilBuf2Work -> A)",
            ),
            (
                &self.work_bind_group_a,
                &self.work_view_b,
                "PS2 Shadow Blur Pass 2 (Work2Work A -> B)",
            ),
            (
                &self.work_bind_group_b,
                &self.work_view_a,
                "PS2 Shadow Blur Pass 3 (Work2Work B -> A)",
            ),
            (
                &self.work_bind_group_a,
                &self.work_view_b,
                "PS2 Shadow Blur Pass 4 (Work2Work A -> B)",
            ),
        ];

        for (src_bg, dst_view, label) in passes {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.blur_pass_pipeline);
            rpass.set_bind_group(0, src_bg, &[]);
            rpass.draw(0..3, 0..1);
        }

        // Final composite onto HDR scene target (`shdw_Composite_Shadow` 0x001761e0)
        let composite_pipeline = match mode {
            ShadowCompositeMode::Multiplicative => &self.composite_mul_pipeline,
            ShadowCompositeMode::Subtractive => &self.composite_sub_pipeline,
        };

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PS2 Shadow Composite Pass (shdw_Composite_Shadow)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: hdr_color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(composite_pipeline);
        rpass.set_bind_group(0, &self.work_bind_group_b, &[]);
        rpass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gs_composite_modes_match_slus_00175f90_registers() {
        assert_eq!(
            ShadowCompositeMode::from_filter_param(0),
            ShadowCompositeMode::Subtractive
        );
        assert_eq!(
            ShadowCompositeMode::Subtractive.gs_alpha_1(),
            0x0000_0080_0000_0052
        );
        assert_eq!(ShadowCompositeMode::Subtractive.gs_test_1(), 0x0003_0000);

        assert_eq!(
            ShadowCompositeMode::from_filter_param(1),
            ShadowCompositeMode::Multiplicative
        );
        assert_eq!(
            ShadowCompositeMode::Multiplicative.gs_alpha_1(),
            0x0000_0080_0000_0046
        );
        assert_eq!(ShadowCompositeMode::Multiplicative.gs_test_1(), 0x0003_0002);
    }

    #[test]
    fn golden_vector_dual_composite_equations() {
        let dst = [0.8, 0.6, 0.4];
        let shadow_rgb = [0.25, 0.25, 0.25];
        let shadow_alpha = 80.0 / 128.0; // 0x50 / 0x80 = 0.625

        // Mode 0 (`0x8000000052`): Cd - Cs * Ad
        let sub =
            ShadowCompositeMode::Subtractive.composite_rgb(dst, 1.0, shadow_rgb, shadow_alpha, 0.5);
        assert!((sub[0] - (0.8 - 0.125)).abs() < 1e-6);
        assert!((sub[1] - (0.6 - 0.125)).abs() < 1e-6);
        assert!((sub[2] - (0.4 - 0.125)).abs() < 1e-6);

        // Mode 1 (`0x8000000046`): Cd * (1 - As)
        let mul = ShadowCompositeMode::Multiplicative.composite_rgb(
            dst,
            1.0,
            shadow_rgb,
            shadow_alpha,
            0.5,
        );
        let expected_factor = 1.0 - 0.625 * 0.5;
        assert!((mul[0] - 0.8 * expected_factor).abs() < 1e-6);
        assert!((mul[1] - 0.6 * expected_factor).abs() < 1e-6);
        assert!((mul[2] - 0.4 * expected_factor).abs() < 1e-6);
    }

    #[test]
    fn get_shadow_color_matches_slus_00176040() {
        assert_eq!(evaluate_ps2_shadow_color(0x50, 0x50, 1.0), [80, 80, 80, 80]);
        assert_eq!(evaluate_ps2_shadow_color(0x80, 0x40, 0.5), [64, 64, 64, 32]);
    }

    #[test]
    fn blur_uniforms_alignment() {
        assert_eq!(std::mem::size_of::<ShadowBlurUniforms>(), 16);
        assert_eq!(PS2_SHADOW_BLUR_PASS_COUNT, 4);
        assert_eq!(PS2_SHADOW_WORK_SIZE, 256);
    }
}
