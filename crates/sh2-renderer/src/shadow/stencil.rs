//! Classic PS2 / SH2:EE `.kg1`/`.kg2` shadow volume & floor drop-shadow GPU pipeline (`0x445540`, `0x5a7c90`).
//!
//! Renders:
//! 1. Extruded `.kg1`/`.kg2` shadow volumes with two-pass Z-fail stencil counting
//!    (`IncrementWrap` on back faces, `DecrementWrap` on front faces) followed by a full-screen
//!    darkening quad where `stencil != 0`, matching `sh2pc.exe` `0x445540` and SH2:EE
//!    `EnableSoftShadows` (`170/255` default opacity + `~33`-frame fade-in).
//! 2. Flattened `.kg1` floor drop shadows (`0x5a7c90`) directly onto the scene HDR target in
//!    lit rooms or dark rooms with `shadow_darkness == 0`.

use bytemuck::{Pod, Zeroable};

use super::volume::ShadowVolumeVertex;
use crate::gpu_map::GpuMap;
use sh2_assets::formats::map::BatchGroup;

/// Maximum vertices reserved in the reusable GPU shadow volume / drop shadow vertex buffer.
pub const MAX_SHADOW_VOLUME_VERTICES: usize = 65_536;

/// Uniform buffer controlling shadow darkening opacity & tint (`16` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowDarkenUniforms {
    /// Shadow tint RGB (`[0.0, 0.0, 0.0]` for pure dark shadow) and alpha opacity (`w`, e.g. `170/255`).
    pub color_opacity: [f32; 4],
}

impl Default for ShadowDarkenUniforms {
    fn default() -> Self {
        Self {
            color_opacity: [0.0, 0.0, 0.0, 170.0 / 255.0],
        }
    }
}

/// GPU pipelines for authentic PS2 `.kg1`/`.kg2` two-pass stencil shadow volumes (`0x445540`)
/// and lit-room floor drop shadows (`0x5a7c90`).
pub struct StencilShadowPipeline {
    /// Depth pre-pass into the `Depth24PlusStencil8` buffer from static map geometry.
    pub depth_prepass_pipeline: wgpu::RenderPipeline,
    /// Two-pass Z-fail stencil volume counting pipeline (`Back: IncrementWrap`, `Front: DecrementWrap`).
    pub volume_stencil_pipeline: wgpu::RenderPipeline,
    /// Full-screen darkening quad (`CompareFunction::NotEqual` ref `0`, `ALPHA_BLENDING`).
    pub darken_quad_pipeline: wgpu::RenderPipeline,
    /// Direct floor drop-shadow pipeline (depth-tested against `Depth32Float`, `ALPHA_BLENDING`).
    pub drop_shadow_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for `ShadowDarkenUniforms` (`@group(1) @binding(0)`).
    pub darken_bind_group_layout: wgpu::BindGroupLayout,
}

impl StencilShadowPipeline {
    /// Depth+stencil format used for authentic two-pass stencil shadow volume counting.
    pub const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

    const WGSL_VOLUME_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

struct ShadowDarkenUniforms {
    color_opacity: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(0) var<uniform> darken: ShadowDarkenUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

@vertex
fn vs_volume(in: VertexInput) -> @builtin(position) vec4<f32> {
    return camera.view_proj * vec4<f32>(in.position, 1.0);
}

@fragment
fn fs_drop() -> @location(0) vec4<f32> {
    return darken.color_opacity;
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vid << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vid & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_darken() -> @location(0) vec4<f32> {
    return darken.color_opacity;
}
"#;

    /// Creates the two-pass stencil shadow volume and floor drop-shadow GPU pipelines.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        hdr_color_format: wgpu::TextureFormat,
        scene_depth_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PS2 Stencil & Drop Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_VOLUME_SHADER.into()),
        });

        let darken_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Darken Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Stencil Shadow Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &darken_bind_group_layout],
            push_constant_ranges: &[],
        });

        let map_pos_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<sh2_assets::formats::map::MapVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }];

        let vol_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowVolumeVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }];

        // 1. Depth prepass into Depth24PlusStencil8
        let depth_prepass_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Stencil Shadow Depth Prepass Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_volume"),
                    buffers: &map_pos_vb,
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Self::STENCIL_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // 2. Z-fail two-sided stencil counting (back faces increment on depth fail, front faces decrement)
        let volume_stencil_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("PS2 Two-Pass Stencil Shadow Volume Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_volume"),
                    buffers: &vol_vb,
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Self::STENCIL_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Always,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::DecrementWrap,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        back: wgpu::StencilFaceState {
                            compare: wgpu::CompareFunction::Always,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::IncrementWrap,
                            pass_op: wgpu::StencilOperation::Keep,
                        },
                        read_mask: 0xFF,
                        write_mask: 0xFF,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // 3. Fullscreen darkening quad where stencil != 0 (`0x445540`)
        let darken_quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PS2 Stencil Shadow Darkening Quad Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_darken"),
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
                format: Self::STENCIL_FORMAT,
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

        // 4. Floor drop-shadow pipeline (`0x5a7c90`) targeting main HDR color + Depth32Float
        let drop_shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PS2 Floor Drop Shadow Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_volume"),
                buffers: &vol_vb,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_drop"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                format: scene_depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            depth_prepass_pipeline,
            volume_stencil_pipeline,
            darken_quad_pipeline,
            drop_shadow_pipeline,
            darken_bind_group_layout,
        }
    }

    /// Encodes the two-pass stencil shadow volume pass + darkening quad onto `hdr_color_view`
    /// (`0x445540`) using the dedicated `Depth24PlusStencil8` target.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_stencil_volume_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        hdr_color_view: &wgpu::TextureView,
        stencil_depth_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        darken_bind_group: &wgpu::BindGroup,
        gpu_map: Option<&GpuMap>,
        vis_filter: Option<&sh2_core::events::stageparts::MapPartVisibilityFilter>,
        volume_buffer: &wgpu::Buffer,
        volume_vertex_count: u32,
    ) {
        if volume_vertex_count == 0 {
            return;
        }

        // Pass A: Depth prepass + Z-fail stencil counting
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PS2 Stencil Shadow Volume Count Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: stencil_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(map) = gpu_map {
                rpass.set_pipeline(&self.depth_prepass_pipeline);
                rpass.set_bind_group(0, camera_bind_group, &[]);
                rpass.set_bind_group(1, darken_bind_group, &[]);
                for batch in &map.batches {
                    if batch.group != BatchGroup::Opaque
                        || !map.is_batch_visible_with(batch, vis_filter)
                    {
                        continue;
                    }
                    rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
                }
            }

            let byte_len =
                (volume_vertex_count as usize * std::mem::size_of::<ShadowVolumeVertex>()) as u64;
            rpass.set_pipeline(&self.volume_stencil_pipeline);
            rpass.set_bind_group(0, camera_bind_group, &[]);
            rpass.set_bind_group(1, darken_bind_group, &[]);
            rpass.set_vertex_buffer(0, volume_buffer.slice(0..byte_len));
            rpass.draw(0..volume_vertex_count, 0..1);
        }

        // Pass B: Full-screen darkening quad where stencil != 0
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PS2 Stencil Shadow Darkening Quad Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: hdr_color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: stencil_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.darken_quad_pipeline);
            rpass.set_bind_group(0, camera_bind_group, &[]);
            rpass.set_bind_group(1, darken_bind_group, &[]);
            rpass.set_stencil_reference(0);
            rpass.draw(0..3, 0..1);
        }
    }

    /// Encodes Z-fail stencil counting, extracts the binary stencil mask into `mask_view`, and
    /// executes the PS2 4-pass ping-pong soft shadow blur + dual GS composite mode
    /// (`SLUS_202.28` `shdw_shadow_filter_main` `0x001767e0`, `sh2gfw_Set_FilterData` `0x00175f90`).
    #[allow(clippy::too_many_arguments)]
    pub fn encode_soft_stencil_blur_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        hdr_color_view: &wgpu::TextureView,
        stencil_depth_view: &wgpu::TextureView,
        mask_view: &wgpu::TextureView,
        mask_bind_group: &wgpu::BindGroup,
        blur_pipeline: &super::blur::ShadowBlurPipeline,
        composite_mode: super::blur::ShadowCompositeMode,
        camera_bind_group: &wgpu::BindGroup,
        darken_bind_group: &wgpu::BindGroup,
        gpu_map: Option<&GpuMap>,
        vis_filter: Option<&sh2_core::events::stageparts::MapPartVisibilityFilter>,
        volume_buffer: &wgpu::Buffer,
        volume_vertex_count: u32,
    ) {
        if volume_vertex_count == 0 {
            return;
        }

        // Pass A: Depth prepass + Z-fail stencil counting
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PS2 Soft Stencil Shadow Volume Count Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: stencil_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(map) = gpu_map {
                rpass.set_pipeline(&self.depth_prepass_pipeline);
                rpass.set_bind_group(0, camera_bind_group, &[]);
                rpass.set_bind_group(1, darken_bind_group, &[]);
                for batch in &map.batches {
                    if batch.group != BatchGroup::Opaque
                        || !map.is_batch_visible_with(batch, vis_filter)
                    {
                        continue;
                    }
                    rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
                }
            }

            let byte_len =
                (volume_vertex_count as usize * std::mem::size_of::<ShadowVolumeVertex>()) as u64;
            rpass.set_pipeline(&self.volume_stencil_pipeline);
            rpass.set_bind_group(0, camera_bind_group, &[]);
            rpass.set_bind_group(1, darken_bind_group, &[]);
            rpass.set_vertex_buffer(0, volume_buffer.slice(0..byte_len));
            rpass.draw(0..volume_vertex_count, 0..1);
        }

        // Pass B: Extract binary stencil mask (stencil != 0 -> 1.0)
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PS2 Soft Stencil Mask Extract Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: mask_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: stencil_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&blur_pipeline.mask_extract_pipeline);
            rpass.set_stencil_reference(0);
            rpass.draw(0..3, 0..1);
        }

        // Pass C: 4-pass ping-pong bilinear blur + dual GS composite (`0x001767e0` + `0x00175f90`)
        blur_pipeline.encode_blur_and_composite(
            encoder,
            mask_bind_group,
            hdr_color_view,
            composite_mode,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_darken_uniforms_alignment() {
        assert_eq!(std::mem::size_of::<ShadowDarkenUniforms>(), 16);
        assert_eq!(std::mem::align_of::<ShadowDarkenUniforms>(), 4);
    }
}
