//! Multi-pass skeletal render pipeline supporting per-primitive backface culling,
//! planar mirror winding inversion, and two-pass hair transparency (`SH2-37`).

use std::num::NonZeroU64;

use super::material::SkeletalMaterialUniforms;
use super::mesh::SkeletalVertex;
use super::shader::WGSL_SKINNING_SHADER;

/// Render pass stage for skeletal character submeshes (`SH2-37`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkeletalPassStage {
    /// Opaque submeshes (`is_transparent == false`): depth-write enabled, `BlendState::REPLACE`.
    Opaque,
    /// Opaque hair/eyelash core (`is_transparent == true`, `alpha_u8 == 255`): depth-write enabled,
    /// `BlendState::ALPHA_BLENDING`, establishing front-to-back hair layer occlusion
    /// (`SLUS_202.28` `InitEnv1` `0x0012c6d0` / `0x002a9780` `TEST_1 = 0x5000d`).
    TransparentCore,
    /// Semi-transparent hair/eyelash fringe (`is_transparent == true`, `1 <= alpha_u8 <= 254`):
    /// depth-write disabled (`LessEqual`), `BlendState::ALPHA_BLENDING`
    /// (`SLUS_202.28` `0x002a9780` / `sh2pc.exe` `FUN_00503800` `0x00503800` `alpha_u8 > 0`).
    TransparentFringe,
    /// Post-depth-fog semi-transparent hair/eyelash fringe (`1 <= alpha_u8 <= 254`, `SH2-192`):
    /// rendered after `FogComputePipeline` into `fog_color_view` with `depth_write_enabled: false`
    /// and in-shader depth fog evaluated at the hair fragment's own `view_z` (`SLUS_202.28`
    /// `Model3DrawVu0Parts` `0x0012bd50`, `SortEnvPrim` `0x00128550`, `sh2_Model_MakeMatrixParams`
    /// `0x001281d0`, `sh2gfw_Set_DepthFog` `0x00175df0`; `sh2pc.exe` `FUN_00503800` `0x00503800`
    /// `D3DRS_FOGENABLE = 1`) so screen-space depth fog never erases hair tips over far backgrounds.
    TransparentFringeFogged,
}

/// Rendering pipeline collection for skeletal character meshes with GPU Linear Blend Skinning (LBS).
pub struct SkeletalPipeline {
    /// Bind group layout `@group(1)` for the 128-bone `SkeletonBoneUniforms` buffer.
    pub bone_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout `@group(3)` for dynamic-offset per-primitive [`SkeletalMaterialUniforms`].
    pub material_bind_group_layout: wgpu::BindGroupLayout,
    /// `[DoubleSided, CullBack, CullFront]` pipelines for [`SkeletalPassStage::Opaque`].
    opaque_pipelines: [wgpu::RenderPipeline; 3],
    /// `[DoubleSided, CullBack, CullFront]` pipelines for [`SkeletalPassStage::TransparentCore`].
    transparent_core_pipelines: [wgpu::RenderPipeline; 3],
    /// `[DoubleSided, CullBack, CullFront]` pipelines for [`SkeletalPassStage::TransparentFringe`].
    transparent_fringe_pipelines: [wgpu::RenderPipeline; 3],
    /// `[DoubleSided, CullBack, CullFront]` pipelines for [`SkeletalPassStage::TransparentFringeFogged`].
    transparent_fringe_fogged_pipelines: [wgpu::RenderPipeline; 3],
    /// Stenciled planar mirror `[DoubleSided, CullBack]` pipelines (`FrontFace::Cw`, `Depth24PlusStencil8`, `stencil == 1`).
    mirror_opaque_pipelines: [wgpu::RenderPipeline; 2],
    mirror_core_pipelines: [wgpu::RenderPipeline; 2],
    mirror_fringe_pipelines: [wgpu::RenderPipeline; 2],
}

impl SkeletalPipeline {
    /// Raw WGSL skinning and PS2/PC material shader source.
    pub const WGSL_SKINNING_SHADER: &'static str = WGSL_SKINNING_SHADER;

    /// Compiles all cull-mode and transparency-stage skeletal pipelines sharing a single WGSL module.
    pub fn new(
        device: &wgpu::Device,
        hdr_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        common_bind_group_layout: &wgpu::BindGroupLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bone_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Skeletal Bone Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Skeletal Material Dynamic Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<
                            SkeletalMaterialUniforms,
                        >() as u64),
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skeletal Render Pipeline Layout"),
            bind_group_layouts: &[
                common_bind_group_layout,
                &bone_bind_group_layout,
                texture_bind_group_layout,
                &material_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader_src = format!(
            "{}\n{}",
            crate::shadow::WGSL_PCSS_FUNCTIONS,
            Self::WGSL_SKINNING_SHADER
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skeletal WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let build_cull_triplet = |label_prefix: &str,
                                  fs_entry: &str,
                                  blend: wgpu::BlendState,
                                  depth_write_enabled: bool|
         -> [wgpu::RenderPipeline; 3] {
            let cull_modes = [
                ("DoubleSided", None),
                ("CullBack", Some(wgpu::Face::Back)),
                ("CullFront", Some(wgpu::Face::Front)),
            ];
            cull_modes.map(|(cull_name, cull_mode)| {
                let label = format!("Skeletal {label_prefix} Pipeline ({cull_name})");
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&label),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[SkeletalVertex::desc()],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fs_entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: hdr_format,
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: depth_format,
                        depth_write_enabled,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                })
            })
        };

        let opaque_pipelines =
            build_cull_triplet("Opaque", "fs_opaque", wgpu::BlendState::REPLACE, true);
        let transparent_core_pipelines = build_cull_triplet(
            "Transparent Core",
            "fs_transparent_core",
            wgpu::BlendState::ALPHA_BLENDING,
            true,
        );
        let transparent_fringe_pipelines = build_cull_triplet(
            "Transparent Fringe",
            "fs_transparent_fringe",
            wgpu::BlendState::ALPHA_BLENDING,
            false,
        );
        let transparent_fringe_fogged_pipelines = build_cull_triplet(
            "Transparent Fringe Fogged",
            "fs_transparent_fringe_fogged",
            wgpu::BlendState::ALPHA_BLENDING,
            false,
        );

        let stencil_equal_keep = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let build_mirror_pair = |label_prefix: &str,
                                 fs_entry: &str,
                                 blend: wgpu::BlendState,
                                 depth_write_enabled: bool|
         -> [wgpu::RenderPipeline; 2] {
            let cull_modes = [("DoubleSided", None), ("CullBack", Some(wgpu::Face::Back))];
            cull_modes.map(|(cull_name, cull_mode)| {
                let label = format!("Skeletal Mirror {label_prefix} Pipeline ({cull_name})");
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&label),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[SkeletalVertex::desc()],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(fs_entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: hdr_format,
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Cw,
                        cull_mode,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::shadow::StencilShadowPipeline::STENCIL_FORMAT,
                        depth_write_enabled,
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
                })
            })
        };

        let mirror_opaque_pipelines =
            build_mirror_pair("Opaque", "fs_opaque", wgpu::BlendState::REPLACE, true);
        let mirror_core_pipelines = build_mirror_pair(
            "Transparent Core",
            "fs_transparent_core",
            wgpu::BlendState::ALPHA_BLENDING,
            true,
        );
        let mirror_fringe_pipelines = build_mirror_pair(
            "Transparent Fringe",
            "fs_transparent_fringe",
            wgpu::BlendState::ALPHA_BLENDING,
            false,
        );

        Self {
            bone_bind_group_layout,
            material_bind_group_layout,
            opaque_pipelines,
            transparent_core_pipelines,
            transparent_fringe_pipelines,
            transparent_fringe_fogged_pipelines,
            mirror_opaque_pipelines,
            mirror_core_pipelines,
            mirror_fringe_pipelines,
        }
    }

    /// Selects the stenciled planar mirror pipeline (`FrontFace::Cw`, `Depth24PlusStencil8`, `stencil == 1`).
    #[inline]
    pub fn select_mirror_pipeline(
        &self,
        stage: SkeletalPassStage,
        backface_culling: bool,
    ) -> &wgpu::RenderPipeline {
        let slot = usize::from(backface_culling);
        match stage {
            SkeletalPassStage::Opaque => &self.mirror_opaque_pipelines[slot],
            SkeletalPassStage::TransparentCore => &self.mirror_core_pipelines[slot],
            SkeletalPassStage::TransparentFringe | SkeletalPassStage::TransparentFringeFogged => {
                &self.mirror_fringe_pipelines[slot]
            }
        }
    }

    /// Selects the render pipeline matching `stage`, `backface_culling` (`ModelPrimitive::backface_culling != 0`),
    /// and `is_mirror_duplicate` (where negative-determinant planar reflection across `X = -20000.0` flips
    /// front-facing CCW triangles to CW, requiring `Face::Front` culling instead of `Face::Back`).
    #[inline]
    pub fn select_pipeline(
        &self,
        stage: SkeletalPassStage,
        backface_culling: bool,
        is_mirror_duplicate: bool,
    ) -> &wgpu::RenderPipeline {
        let cull_slot = match (backface_culling, is_mirror_duplicate) {
            (false, _) => 0,
            (true, false) => 1,
            (true, true) => 2,
        };
        match stage {
            SkeletalPassStage::Opaque => &self.opaque_pipelines[cull_slot],
            SkeletalPassStage::TransparentCore => &self.transparent_core_pipelines[cull_slot],
            SkeletalPassStage::TransparentFringe => &self.transparent_fringe_pipelines[cull_slot],
            SkeletalPassStage::TransparentFringeFogged => {
                &self.transparent_fringe_fogged_pipelines[cull_slot]
            }
        }
    }
}
