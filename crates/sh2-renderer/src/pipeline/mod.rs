//! Geometry rendering pipeline for level meshes (.map) with Depth32Float & HDR (Rgba16Float).

use bytemuck::{Pod, Zeroable};
mod debug_line;
pub use debug_line::DebugLinePipeline;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
    /// Camera eye position `(x, y, z)` and debug shading mode in `w` (`0.0` = Textured, `1.0` = Clay, `2.0` = Normals, `3.0` = Studio 3-Point).
    pub cam_pos: [f32; 4],
    /// Inverse view-projection matrix for world-space ray reconstruction in volumetric compute passes (`SH2-32`).
    pub inv_view_proj: [[f32; 4]; 4],
    /// Optional world-space mirror clip plane `(nx, ny, nz, d)` (`dot(n, p) + d < 0 => discard`).
    /// All zeros disables clipping.
    pub clip_plane: [f32; 4],
}

impl Default for CameraUniforms {
    fn default() -> Self {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            view_proj: id,
            inv_proj: id,
            cam_pos: [0.0, 0.0, 0.0, 1.0],
            inv_view_proj: id,
            clip_plane: [0.0; 4],
        }
    }
}

/// Authentic PS2/PC quantized 8-bit alpha-test cutoff (`alpha > 0` in `[0..=255]`):
/// - `SLUS_202.28` `shGs_InitDefaultRegsEnv` (`0x001909e0`) & `InitEnv1` (`0x002a9780`):
///   `TEST_1 = 0x5000d` (`ATE = 1`, `ATST = 6 [GREATER]`, `AREF = 0`, `AFAIL = 0 [KEEP]`).
/// - `sh2pc.exe` `FUN_005b4cc0` (`0x005b4cc0`, Opaque/Transparent `.map` pass) and
///   `0x005b1694` (`sh2pc_005a0000.c:7565-7569`, Decal pass):
///   `D3DRS_ALPHATESTENABLE = 1`, `D3DRS_ALPHAFUNC = 5 (D3DCMP_GREATER)`, `D3DRS_ALPHAREF = 0`.
pub const MAP_ALPHA_TEST_THRESHOLD: f32 = 0.5 / 255.0;

/// Verified hardware render-state contract for a `.map` [`sh2_assets::formats::map::BatchGroup`].
///
/// Derived 1:1 from `sh2pc.exe`:
/// - `D3DRS_CULLMODE (0x16) = 2 (D3DCULL_CW)` -> CCW front-face with hardware backface culling
///   across `Opaque`, `Transparent` (`FUN_005b4cc0` `0x005b4cc0`), and `Decal`
///   (`sh2pc_005a0000.c:7534,7547`).
/// - `D3DRS_ALPHATESTENABLE = 1`, `D3DCMP_GREATER`, `D3DRS_ALPHAREF = 0` across all three groups
///   (`FUN_005b4cc0` `0x005b4cc0` & `SLUS_202.28` `shGs_InitDefaultRegsEnv` `0x001909e0`).
/// - `MapMaterial.mode` (`FUN_005b2d40` `0x005b2d40` / `DAT_008f0338`) selects lighting/specular
///   shader mode only and never overrides `D3DRS_CULLMODE` or `D3DRS_ALPHATESTENABLE`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapBatchRenderState {
    /// Hardware primitive cull mode (`Some(Face::Back)` = `D3DCULL_CW` with `FrontFace::Ccw`).
    pub cull_mode: Option<wgpu::Face>,
    /// Front-face winding (`FrontFace::Ccw`, matching `D3DCULL_CW` in `sh2pc.exe`).
    pub front_face: wgpu::FrontFace,
    /// Whether depth writes are enabled (`true` for `Opaque`, `false` for `Decal`/`Transparent`).
    pub depth_write_enabled: bool,
    /// Whether alpha blending is enabled (`false` for `Opaque`, `true` for `Decal`/`Transparent`).
    pub alpha_blending: bool,
    /// Quantized alpha-test discard threshold (`MAP_ALPHA_TEST_THRESHOLD` = `0.5 / 255.0`).
    pub alpha_test_threshold: f32,
}

/// Returns the 1:1 `sh2pc.exe` / `SLUS_202.28` hardware render state for `group`.
#[must_use]
pub const fn map_batch_render_state(
    group: sh2_assets::formats::map::BatchGroup,
) -> MapBatchRenderState {
    use sh2_assets::formats::map::BatchGroup;
    match group {
        BatchGroup::Opaque => MapBatchRenderState {
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            depth_write_enabled: true,
            alpha_blending: false,
            alpha_test_threshold: MAP_ALPHA_TEST_THRESHOLD,
        },
        BatchGroup::Decal => MapBatchRenderState {
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            // 1:1 `sh2pc.exe` `0x005b1694` (`sh2pc_005a0000.c:7536,7549,7558,7570`):
            // `D3DRS_ZWRITEENABLE (0x0e) = 1` with `D3DCMP_LESSEQUAL (0x17 = 4)` so foreground
            // decals (e.g. `ob01` power lines at `Z = -4884 mm`) write their true depth and never
            // inherit far-vista fog (`Z = 10,500 mm`) from the skybox behind them.
            depth_write_enabled: true,
            alpha_blending: true,
            alpha_test_threshold: MAP_ALPHA_TEST_THRESHOLD,
        },
        BatchGroup::Transparent => MapBatchRenderState {
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            depth_write_enabled: false,
            alpha_blending: true,
            alpha_test_threshold: MAP_ALPHA_TEST_THRESHOLD,
        },
    }
}

pub struct GeometryPipeline {
    /// Primary opaque + alpha-cutout level geometry pipeline (`depth_write_enabled: true`).
    pub pipeline: wgpu::RenderPipeline,
    /// Stenciled planar mirror reflected opaque pipeline (`FrontFace::Cw`, `Depth24PlusStencil8`, `stencil == 1`).
    pub mirror_reflected_pipeline: wgpu::RenderPipeline,
    /// Coplanar decal pipeline (`ALPHA_BLENDING`, `depth_write_enabled: false`, negative depth bias).
    pub decal_pipeline: wgpu::RenderPipeline,
    /// Semi-transparent glass/water/foliage fringe pipeline (`ALPHA_BLENDING`, `depth_write_enabled: false`).
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl GeometryPipeline {
    pub const WGSL_SHADER: &'static str = include_str!("geometry.wgsl");

    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader_src = format!(
            "{}\n{}",
            crate::shadow::WGSL_PCSS_FUNCTIONS,
            Self::WGSL_SHADER
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geometry Shader Module"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Geometry Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Geometry Diffuse Texture Bind Group Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_attributes = [
            // position
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            // normal
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            // uv
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            // color
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            // shader_mode (`MapMaterial.mode & 0xff`)
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 4,
                format: wgpu::VertexFormat::Uint32,
            },
        ];

        let vertex_buffer_layout = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<sh2_assets::formats::map::MapVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        }];

        let opaque_state = map_batch_render_state(sh2_assets::formats::map::BatchGroup::Opaque);
        // Hardware backface culling matching `sh2pc.exe` `D3DRS_CULLMODE (0x16) = 2 (D3DCULL_CW)`
        // inherited by `FUN_005b4cc0` (`0x005b4cc0`) and restored for decals (`0x005b1694`).
        let primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: opaque_state.front_face,
            cull_mode: opaque_state.cull_mode,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Opaque & Cutout Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffer_layout,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_opaque"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: primitive_state,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let decal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Coplanar Decal Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffer_layout,
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
            primitive: primitive_state,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Transparent Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffer_layout,
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
            primitive: primitive_state,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let stencil_equal_keep = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let mut mirror_primitive = primitive_state;
        mirror_primitive.front_face = wgpu::FrontFace::Cw;
        let mirror_reflected_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Geometry Stenciled Mirror Reflected Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &vertex_buffer_layout,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_opaque"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: mirror_primitive,
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::shadow::StencilShadowPipeline::STENCIL_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState {
                        front: stencil_equal_keep,
                        back: stencil_equal_keep,
                        read_mask: 0xff,
                        write_mask: 0x00,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Self {
            pipeline,
            mirror_reflected_pipeline,
            decal_pipeline,
            transparent_pipeline,
            bind_group_layout,
            texture_bind_group_layout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GeometryPipeline;

    #[test]
    fn map_shader_uses_slus_draw_env_base_ambient_002ad900() {
        let src = GeometryPipeline::WGSL_SHADER;
        assert!(src.contains("vec3<f32>(0.045, 0.045, 0.045)"));
        assert!(src.contains("vec3<f32>(0.18, 0.18, 0.18)"));
        assert!(!src.contains("0.07, 0.075, 0.085"));
        assert!(!src.contains("0.18, 0.19, 0.21"));
    }
}
