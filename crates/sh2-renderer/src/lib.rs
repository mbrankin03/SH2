//! sh2-renderer: Modern wgpu/Vulkan pipeline, 32-bit float depth, volumetric compute fog.

pub mod fog;
pub mod light;
pub mod pipeline;
pub mod post;

use sh2_assets::formats::map::MapFile;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy)]
pub struct RendererConfig {
    pub width: u32,
    pub height: u32,
    pub enable_hdr: bool,
    pub enable_fog: bool,
    pub enable_post: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        }
    }
}

pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: RendererConfig,

    pub depth_format: wgpu::TextureFormat,
    pub hdr_format: wgpu::TextureFormat,
    pub output_format: wgpu::TextureFormat,

    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,

    pub hdr_color_texture: wgpu::Texture,
    pub hdr_color_view: wgpu::TextureView,

    pub fog_color_texture: wgpu::Texture,
    pub fog_color_view: wgpu::TextureView,

    pub ldr_output_texture: wgpu::Texture,
    pub ldr_output_view: wgpu::TextureView,

    pub geometry_pipeline: pipeline::GeometryPipeline,
    pub fog_pipeline: fog::FogComputePipeline,
    pub post_pipeline: post::PostProcessPipeline,

    pub camera_buffer: wgpu::Buffer,
    pub spotlight_buffer: wgpu::Buffer,
    pub fog_buffer: wgpu::Buffer,
    pub post_buffer: wgpu::Buffer,

    pub geometry_bind_group: wgpu::BindGroup,
    pub fog_bind_group: wgpu::BindGroup,
    pub post_bind_group: wgpu::BindGroup,
}

impl RenderContext {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub fn new_headless(config: RendererConfig) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("SH2 Render Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .ok()?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("32-bit Float Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let hdr_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hdr_color_view = hdr_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let fog_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fog Output Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::HDR_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fog_color_view = fog_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let ldr_output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("LDR Output Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let ldr_output_view =
            ldr_output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let geometry_pipeline =
            pipeline::GeometryPipeline::new(&device, Self::HDR_FORMAT, Self::DEPTH_FORMAT);
        let fog_pipeline = fog::FogComputePipeline::new(&device);
        let post_pipeline = post::PostProcessPipeline::new(&device);

        let camera_uniforms = pipeline::CameraUniforms::default();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&camera_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let spotlight_uniforms = light::SpotlightUniforms::default();
        let spotlight_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Spotlight Uniform Buffer"),
            contents: bytemuck::bytes_of(&spotlight_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let fog_uniforms = fog::FogUniforms::default();
        let fog_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fog Uniform Buffer"),
            contents: bytemuck::bytes_of(&fog_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let post_uniforms = post::PostUniforms::default();
        let post_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Post Uniform Buffer"),
            contents: bytemuck::bytes_of(&post_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let geometry_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Geometry Bind Group"),
            layout: &geometry_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: spotlight_buffer.as_entire_binding(),
                },
            ],
        });

        let fog_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Bind Group"),
            layout: &fog_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: fog_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&hdr_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&fog_color_view),
                },
            ],
        });

        let post_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group"),
            layout: &post_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: post_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&fog_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ldr_output_view),
                },
            ],
        });

        Some(Self {
            instance,
            adapter,
            device,
            queue,
            config,
            depth_format: Self::DEPTH_FORMAT,
            hdr_format: Self::HDR_FORMAT,
            output_format: Self::OUTPUT_FORMAT,
            depth_texture,
            depth_view,
            hdr_color_texture,
            hdr_color_view,
            fog_color_texture,
            fog_color_view,
            ldr_output_texture,
            ldr_output_view,
            geometry_pipeline,
            fog_pipeline,
            post_pipeline,
            camera_buffer,
            spotlight_buffer,
            fog_buffer,
            post_buffer,
            geometry_bind_group,
            fog_bind_group,
            post_bind_group,
        })
    }

    pub fn update_camera(&self, view_proj: glam::Mat4, inv_proj: glam::Mat4, cam_pos: glam::Vec3) {
        let uniforms = pipeline::CameraUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_proj: inv_proj.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn update_fog(&self, uniforms: &fog::FogUniforms) {
        self.queue
            .write_buffer(&self.fog_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn update_spotlight(&self, uniforms: &light::SpotlightUniforms) {
        self.queue
            .write_buffer(&self.spotlight_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn update_post(&self, uniforms: &post::PostUniforms) {
        self.queue
            .write_buffer(&self.post_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Render a MapFile room into the framebuffers and run the compute passes.
    pub fn render_map(&self, map: &MapFile) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Command Encoder"),
            });

        // 1. Geometry Pass: Render into HDR buffer with Depth32Float
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Geometry Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.01,
                            g: 0.01,
                            b: 0.015,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.geometry_pipeline.pipeline);
            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);

            for batch in &map.batches {
                if batch.indices.is_empty() || batch.vertices.is_empty() {
                    continue;
                }
                let v_buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Batch Vertex Buffer"),
                        contents: bytemuck::cast_slice(&batch.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let i_buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Batch Index Buffer"),
                        contents: bytemuck::cast_slice(&batch.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });

                rpass.set_vertex_buffer(0, v_buf.slice(..));
                rpass.set_index_buffer(i_buf.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..batch.indices.len() as u32, 0, 0..1);
            }
        }

        // 2. Volumetric Fog Compute Pass
        if self.config.enable_fog {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Volumetric Fog Compute Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.fog_pipeline.pipeline);
            cpass.set_bind_group(0, &self.fog_bind_group, &[]);
            let workgroups_x = self.config.width.div_ceil(16);
            let workgroups_y = self.config.height.div_ceil(16);
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        // 3. Post-Processing Compute Pass (ACES tonemapping & film grain)
        if self.config.enable_post {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Post Process Compute Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.post_pipeline.pipeline);
            cpass.set_bind_group(0, &self.post_bind_group, &[]);
            let workgroups_x = self.config.width.div_ceil(16);
            let workgroups_y = self.config.height.div_ceil(16);
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
