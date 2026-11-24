//! Toluca Lake water surface & native planar reflection subsystem (`SH2-80`).
//!
//! - Strict 1:1 Vanilla PS2 mode (`enable_water_reflections = false`, `enable_oar_ripples = false`):
//!   Renders the 48x48 `WaterGridSim` (`Effect2/hh_class_water_00.c` `0x0025ba70` / `0x0025bd00`)
//!   with PS2 `Specular_Calculator` (`0x0025bb80`).
//! - Native-Upgrade mode (`enable_water_reflections = true` / `enable_oar_ripples = true`):
//!   Renders a mirrored planar reflection pass across `y = water_height_mm` into `reflection_view`
//!   and perturbs reflections/normals with interactive oar ripple rings driven by `BoatCheckEffect`.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use sh2_core::boat::{BOAT_WATER_BASE_OFFSET_PC, BOAT_WORLD_SCALE_MM, WATER_GRID_SIZE};
use wgpu::util::DeviceExt;

/// Maximum simultaneous oar ripple rings tracked on the GPU.
pub const MAX_OAR_RIPPLES: usize = 8;

/// Planar reflection render target resolution (width/height).
pub const WATER_REFLECTION_SIZE: u32 = 512;

/// `hh_class_water_00.c` `Object_Draw` (`0x0025bd00`): `500.0` mm per grid cell.
pub const WATER_GRID_SPACING_MM: f32 = BOAT_WORLD_SCALE_MM;

/// Default retail PC Toluca Lake water height in mm (`7.38 * 500.0 = 3690.0`, `sh2pc.exe` `0x0053efa0`).
pub const DEFAULT_WATER_HEIGHT_MM: f32 = BOAT_WATER_BASE_OFFSET_PC * BOAT_WORLD_SCALE_MM;

/// GPU vertex for the 48x48 Toluca Lake `WaterGridSim` mesh (`Effect2/hh_class_water_00.c` `0x0025bd00`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub grid_xy: [u32; 2],
}

/// GPU uniform block for Toluca Lake water rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WaterUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub light_dir: [f32; 4],
    pub water_tint: [f32; 4],
    pub fog_color: [f32; 4],
    /// `[water_height_mm, 0.0, enable_water_reflections, enable_oar_ripples]`
    pub params: [f32; 4],
    /// `[fog_start_mm, fog_end_mm, reflection_strength, specular_strength]`
    pub fog_and_spec: [f32; 4],
}

impl Default for WaterUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            cam_pos: [0.0, DEFAULT_WATER_HEIGHT_MM + BOAT_WORLD_SCALE_MM, 0.0, 1.0],
            light_dir: [0.0, 1.0, 0.0, 0.0],
            water_tint: [0.04, 0.08, 0.10, 0.88],
            fog_color: [0.12, 0.14, 0.16, 1.0],
            params: [DEFAULT_WATER_HEIGHT_MM, 0.0, 0.0, 0.0],
            fog_and_spec: [4000.0, 45000.0, 0.75, 0.35],
        }
    }
}

/// Single GPU oar ripple entry (`OarRipple` in `water/shader.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuOarRipple {
    /// `[x_mm, y_mm, z_mm, age_secs]` (`age_secs < 0.0` = inactive).
    pub pos_age: [f32; 4],
    /// `[radius_mm, amplitude, wavelength_mm, max_age_secs]`
    pub params: [f32; 4],
}

impl Default for GpuOarRipple {
    fn default() -> Self {
        Self {
            pos_age: [0.0, 0.0, 0.0, -1.0],
            params: [0.0, 0.0, BOAT_WORLD_SCALE_MM * 2.0, 3.0],
        }
    }
}

/// Uniform buffer of 8 active oar ripples.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WaterRippleBuffer {
    pub ripples: [GpuOarRipple; MAX_OAR_RIPPLES],
}

impl Default for WaterRippleBuffer {
    fn default() -> Self {
        Self {
            ripples: [GpuOarRipple::default(); MAX_OAR_RIPPLES],
        }
    }
}

/// Computes the planar reflection view-projection matrix mirrored across horizontal plane `y = plane_y`.
#[inline]
pub fn mirrored_reflection_view_proj(view_proj: Mat4, plane_y: f32) -> Mat4 {
    let reflect = Mat4::from_cols_array(&[
        1.0,
        0.0,
        0.0,
        0.0, //
        0.0,
        -1.0,
        0.0,
        0.0, //
        0.0,
        0.0,
        1.0,
        0.0, //
        0.0,
        2.0 * plane_y,
        0.0,
        1.0,
    ]);
    view_proj * reflect
}

/// Toluca Lake water surface & planar reflection renderer.
pub struct LakeWaterPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: wgpu::Buffer,
    pub ripple_buffer: wgpu::Buffer,
    pub grid_height_buffer: wgpu::Buffer,
    pub reflection_texture: wgpu::Texture,
    pub reflection_view: wgpu::TextureView,
    pub reflection_depth_texture: wgpu::Texture,
    pub reflection_depth_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl LakeWaterPipeline {
    /// Creates the Toluca Lake 48x48 `WaterGridSim` pipeline and planar reflection render target.
    pub fn new(device: &wgpu::Device, hdr_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Toluca Lake Water Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let reflection_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Water Planar Reflection Texture"),
            size: wgpu::Extent3d {
                width: WATER_REFLECTION_SIZE,
                height: WATER_REFLECTION_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let reflection_view =
            reflection_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let reflection_depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Water Planar Reflection Depth"),
            size: wgpu::Extent3d {
                width: WATER_REFLECTION_SIZE,
                height: WATER_REFLECTION_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let reflection_depth_view =
            reflection_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let reflection_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Water Reflection Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Uniforms Buffer"),
            contents: bytemuck::bytes_of(&WaterUniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let ripple_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Oar Ripples Buffer"),
            contents: bytemuck::bytes_of(&WaterRippleBuffer::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let zero_heights = vec![0.0f32; WATER_GRID_SIZE * WATER_GRID_SIZE];
        let grid_height_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water GridSim Height Buffer"),
            contents: bytemuck::cast_slice(&zero_heights),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Water Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ripple_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&reflection_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&reflection_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: grid_height_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Water Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Toluca Lake Water Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<WaterVertex>() as wgpu::BufferAddress,
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
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint32x2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Build 48x48 vertex grid (`hh_class_water_00.c` `0x0025bd00`: 500.0 mm spacing, centered around origin).
        let half_span = ((WATER_GRID_SIZE - 1) as f32 * WATER_GRID_SPACING_MM) * 0.5;
        let mut vertices = Vec::with_capacity(WATER_GRID_SIZE * WATER_GRID_SIZE);
        for gz in 0..WATER_GRID_SIZE {
            for gx in 0..WATER_GRID_SIZE {
                let x = (gx as f32) * WATER_GRID_SPACING_MM - half_span;
                let z = (gz as f32) * WATER_GRID_SPACING_MM - half_span;
                vertices.push(WaterVertex {
                    position: [x, 0.0, z],
                    uv: [
                        gx as f32 / (WATER_GRID_SIZE - 1) as f32,
                        gz as f32 / (WATER_GRID_SIZE - 1) as f32,
                    ],
                    grid_xy: [gx as u32, gz as u32],
                });
            }
        }
        let mut indices = Vec::with_capacity((WATER_GRID_SIZE - 1) * (WATER_GRID_SIZE - 1) * 6);
        for gz in 0..(WATER_GRID_SIZE - 1) {
            for gx in 0..(WATER_GRID_SIZE - 1) {
                let i0 = (gx + gz * WATER_GRID_SIZE) as u16;
                let i1 = i0 + 1;
                let i2 = i0 + WATER_GRID_SIZE as u16;
                let i3 = i2 + 1;
                indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            }
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Grid Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Grid Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            uniform_buffer,
            ripple_buffer,
            grid_height_buffer,
            reflection_texture,
            reflection_view,
            reflection_depth_texture,
            reflection_depth_view,
            bind_group,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    /// Uploads updated water uniforms and oar ripple state.
    pub fn update(
        &self,
        queue: &wgpu::Queue,
        uniforms: &WaterUniforms,
        ripples: &WaterRippleBuffer,
    ) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
        queue.write_buffer(&self.ripple_buffer, 0, bytemuck::bytes_of(ripples));
    }

    /// Uploads the 48x48 `WaterGridSim` heightfield (`hh_class_water_00.c` `0x0025ba70`).
    pub fn update_grid(&self, queue: &wgpu::Queue, heights: &[f32]) {
        if heights.len() == WATER_GRID_SIZE * WATER_GRID_SIZE {
            queue.write_buffer(&self.grid_height_buffer, 0, bytemuck::cast_slice(heights));
        }
    }

    /// Encodes the water surface draw call inside an active scene render pass.
    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// Returns the mirrored camera position across `y = plane_y`.
    #[inline]
    pub fn mirrored_camera_pos(cam_pos: Vec3, plane_y: f32) -> Vec3 {
        Vec3::new(cam_pos.x, 2.0 * plane_y - cam_pos.y, cam_pos.z)
    }
}
