//! Stenciled planar mirror GPU pipelines (`stencil_mask`, `depth_write`, and `glas03.tbn2` overlay).

use sh2_assets::formats::map::MapVertex;
use wgpu::util::DeviceExt;

use super::glass::{create_glass_texture, MirrorQuadVertex, MIRROR_WGSL_SHADER};
use crate::shadow::StencilShadowPipeline;

/// GPU pipelines and buffers for the stenciled planar mirror pass (`SH2-101`).
pub struct PlanarMirrorPipeline {
    /// Writes `stencil = 1` on `Depth24PlusStencil8` for the mirror surface (`color_writes: empty`).
    pub stencil_mask_pipeline: wgpu::RenderPipeline,
    /// Writes `stencil = 1` using a `.map` [`MapVertex`] decal batch (`er01.map` `mat=10`, `ap84.map` `mat=13`).
    pub map_stencil_mask_pipeline: wgpu::RenderPipeline,
    /// Writes mirror-plane depth into `Depth32Float` in the main pass for fallback mirror quads
    /// (`th09.map`) so pre-baked duplicate room geometry behind the mirror opening is depth-rejected.
    pub depth_write_pipeline: wgpu::RenderPipeline,
    /// Writes mirror-plane depth into `Depth32Float` using a `.map` [`MapVertex`] decal batch
    /// (`er01.map` `mat=10`, `ap84.map` `mat=13`) before opaque room geometry draws.
    pub map_depth_write_pipeline: wgpu::RenderPipeline,
    /// Optional dirty mirror glass overlay (`glas03.tbn2`) drawn with alpha blending over the mirror quad.
    pub glass_overlay_pipeline: wgpu::RenderPipeline,
    /// Dynamic 4-vertex quad buffer for the active [`super::PlanarMirrorSpec`].
    pub quad_vertex_buffer: wgpu::Buffer,
    /// 6-index quad index buffer (`[0, 1, 2, 0, 2, 3]`).
    pub quad_index_buffer: wgpu::Buffer,
    /// Bind group layout for `@group(1)` dirty glass texture (`glas03.tbn2`).
    pub glass_bind_group_layout: wgpu::BindGroupLayout,
    /// GPU texture storing decoded `glas03.tbn2` (or procedural subtle tint fallback).
    pub glass_texture: wgpu::Texture,
    /// Texture view for [`Self::glass_texture`].
    pub glass_view: wgpu::TextureView,
    /// Bind group for `@group(1)` in [`Self::glass_overlay_pipeline`].
    pub glass_bind_group: wgpu::BindGroup,
}

impl PlanarMirrorPipeline {
    /// Creates the stenciled planar mirror pipelines and default `glas03.tbn2` fallback texture.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hdr_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        common_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Planar Mirror WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(MIRROR_WGSL_SHADER.into()),
        });

        let glass_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Mirror Glass Overlay Bind Group Layout"),
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

        let mask_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mirror Mask Pipeline Layout"),
            bind_group_layouts: &[common_bind_group_layout],
            push_constant_ranges: &[],
        });

        let glass_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mirror Glass Overlay Pipeline Layout"),
            bind_group_layouts: &[common_bind_group_layout, &glass_bind_group_layout],
            push_constant_ranges: &[],
        });

        let quad_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MirrorQuadVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        };

        let map_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MapVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let stencil_replace = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Replace,
        };

        let stencil_mask_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Mirror Stencil Mask Quad Pipeline"),
                layout: Some(&mask_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_quad"),
                    buffers: std::slice::from_ref(&quad_layout),
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: StencilShadowPipeline::STENCIL_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState {
                        front: stencil_replace,
                        back: stencil_replace,
                        read_mask: 0xff,
                        write_mask: 0xff,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let map_stencil_mask_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Mirror Stencil Mask Map Decal Pipeline"),
                layout: Some(&mask_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_map"),
                    buffers: std::slice::from_ref(&map_layout),
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: StencilShadowPipeline::STENCIL_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState {
                        front: stencil_replace,
                        back: stencil_replace,
                        read_mask: 0xff,
                        write_mask: 0xff,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let depth_write_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mirror Plane Depth-Only Pipeline"),
            layout: Some(&mask_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_quad"),
                buffers: std::slice::from_ref(&quad_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mask"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let map_depth_write_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Mirror Plane Map Decal Depth-Only Pipeline"),
                layout: Some(&mask_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_map"),
                    buffers: std::slice::from_ref(&map_layout),
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_mask"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::empty(),
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
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let glass_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Mirror Dirty Glass Overlay Pipeline"),
                layout: Some(&glass_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_quad"),
                    buffers: &[quad_layout],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_glass"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
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
                    format: depth_format,
                    depth_write_enabled: false,
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

        let default_quad = [
            MirrorQuadVertex {
                position: [0.0; 3],
                uv: [0.0, 0.0],
            },
            MirrorQuadVertex {
                position: [0.0; 3],
                uv: [1.0, 0.0],
            },
            MirrorQuadVertex {
                position: [0.0; 3],
                uv: [1.0, 1.0],
            },
            MirrorQuadVertex {
                position: [0.0; 3],
                uv: [0.0, 1.0],
            },
        ];
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mirror Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&default_quad),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let quad_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mirror Quad Index Buffer"),
            contents: bytemuck::cast_slice(&quad_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let glass_texture = create_glass_texture(device, queue, 1, 1, &[128, 138, 128, 0]);
        let glass_view = glass_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Mirror Glass Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let glass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mirror Glass Overlay Bind Group"),
            layout: &glass_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&glass_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            stencil_mask_pipeline,
            map_stencil_mask_pipeline,
            depth_write_pipeline,
            map_depth_write_pipeline,
            glass_overlay_pipeline,
            quad_vertex_buffer,
            quad_index_buffer,
            glass_bind_group_layout,
            glass_texture,
            glass_view,
            glass_bind_group,
        }
    }
}
