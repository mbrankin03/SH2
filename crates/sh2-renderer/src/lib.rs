//! sh2-renderer: Modern wgpu/Vulkan pipeline, 32-bit float depth, volumetric compute fog.

pub mod fog;
pub mod light;
pub mod pipeline;
pub mod post;
pub mod skeletal;

use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::mdls::ModelFile;
pub use skeletal::{
    GpuSkeletalMesh, GpuSkeletalPrimitive, SkeletalPipeline, SkeletalVertex, SkeletonBoneUniforms,
    MAX_SKELETON_BONES,
};
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

/// GPU-resident static mesh buffers compiled once during map loading to avoid per-frame allocations.
pub struct GpuMapBatch {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub is_transparent: bool,
}

pub struct GpuMap {
    pub batches: Vec<GpuMapBatch>,
}

impl GpuMap {
    pub fn from_map(device: &wgpu::Device, map: &MapFile) -> Self {
        let mut batches = Vec::with_capacity(map.batches.len());
        for batch in &map.batches {
            let interleaved = batch.interleaved_vertices();
            if batch.indices.is_empty() || interleaved.is_empty() {
                continue;
            }
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuMap Batch Vertex Buffer"),
                contents: bytemuck::cast_slice(&interleaved),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuMap Batch Index Buffer"),
                contents: bytemuck::cast_slice(&batch.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            batches.push(GpuMapBatch {
                vertex_buffer,
                index_buffer,
                num_indices: batch.indices.len() as u32,
                is_transparent: matches!(
                    batch.group,
                    sh2_assets::formats::map::BatchGroup::Transparent
                ),
            });
        }
        Self { batches }
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
    pub skeletal_pipeline: skeletal::SkeletalPipeline,
    pub fog_pipeline: fog::FogComputePipeline,
    pub post_pipeline: post::PostProcessPipeline,

    pub camera_buffer: wgpu::Buffer,
    pub spotlight_buffer: wgpu::Buffer,
    pub fog_buffer: wgpu::Buffer,
    pub post_buffer: wgpu::Buffer,

    pub geometry_bind_group: wgpu::BindGroup,
    pub fog_bind_group: wgpu::BindGroup,
    pub post_bind_group_fog: wgpu::BindGroup,
    pub post_bind_group_direct: wgpu::BindGroup,
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
        let skeletal_pipeline = skeletal::SkeletalPipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
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

        // Bind group 1: Samples fog_color_view when fog is enabled
        let post_bind_group_fog = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group (With Fog)"),
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

        // Bind group 2: Directly samples hdr_color_view when fog is disabled
        let post_bind_group_direct = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group (Direct HDR)"),
            layout: &post_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: post_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&hdr_color_view),
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
            skeletal_pipeline,
            fog_pipeline,
            post_pipeline,
            camera_buffer,
            spotlight_buffer,
            fog_buffer,
            post_buffer,
            geometry_bind_group,
            fog_bind_group,
            post_bind_group_fog,
            post_bind_group_direct,
        })
    }

    pub fn from_device(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: RendererConfig,
    ) -> Self {
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
        let skeletal_pipeline = skeletal::SkeletalPipeline::new(
            &device,
            Self::HDR_FORMAT,
            Self::DEPTH_FORMAT,
            &geometry_pipeline.bind_group_layout,
        );
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

        let post_bind_group_fog = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group (With Fog)"),
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

        let post_bind_group_direct = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Post Bind Group (Direct HDR)"),
            layout: &post_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: post_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&hdr_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ldr_output_view),
                },
            ],
        });

        Self {
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
            skeletal_pipeline,
            fog_pipeline,
            post_pipeline,
            camera_buffer,
            spotlight_buffer,
            fog_buffer,
            post_buffer,
            geometry_bind_group,
            fog_bind_group,
            post_bind_group_fog,
            post_bind_group_direct,
        }
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

    /// Render a pre-compiled GpuMap room into the framebuffers and run the compute passes.
    pub fn render_gpu_map(&self, gpu_map: &GpuMap) {
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

            for batch in &gpu_map.batches {
                rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
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
            let post_bind_group = if self.config.enable_fog {
                &self.post_bind_group_fog
            } else {
                &self.post_bind_group_direct
            };
            cpass.set_bind_group(0, post_bind_group, &[]);
            let workgroups_x = self.config.width.div_ceil(16);
            let workgroups_y = self.config.height.div_ceil(16);
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Convenience method for rendering directly from MapFile, compiling to GpuMap.
    pub fn render_map(&self, map: &MapFile) {
        let gpu_map = GpuMap::from_map(&self.device, map);
        self.render_gpu_map(&gpu_map);
    }

    /// Renders static world level map batches and dynamic animated skeletal meshes in a unified pass.
    pub fn render_scene(
        &self,
        gpu_map: Option<&GpuMap>,
        skeletal_meshes: &[(&GpuSkeletalMesh, &[[[f32; 4]; 4]])],
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Render Encoder"),
            });

        // 1. Geometry & Skeletal Meshes Render Pass
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Geometry & Skeletal Render Pass"),
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

            // 1a. Render static level batches if provided
            if let Some(map) = gpu_map {
                rpass.set_pipeline(&self.geometry_pipeline.pipeline);
                rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                for batch in &map.batches {
                    rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
                }
            }

            // 1b. Render skeletal character meshes
            if !skeletal_meshes.is_empty() {
                rpass.set_pipeline(&self.skeletal_pipeline.pipeline);
                rpass.set_bind_group(0, &self.geometry_bind_group, &[]);

                for (mesh, matrices) in skeletal_meshes {
                    mesh.update_bone_matrices(&self.queue, matrices);
                    rpass.set_bind_group(1, &mesh.bone_bind_group, &[]);
                    rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                    for prim in &mesh.primitives {
                        if prim.index_count > 0 {
                            rpass.draw_indexed(
                                prim.first_index..prim.first_index + prim.index_count,
                                0,
                                0..1,
                            );
                        }
                    }
                }
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
            let post_bind_group = if self.config.enable_fog {
                &self.post_bind_group_fog
            } else {
                &self.post_bind_group_direct
            };
            cpass.set_bind_group(0, post_bind_group, &[]);
            let workgroups_x = self.config.width.div_ceil(16);
            let workgroups_y = self.config.height.div_ceil(16);
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Compiles a ModelFile into a GpuSkeletalMesh.
    pub fn compile_skeletal_mesh(&self, model: &ModelFile) -> GpuSkeletalMesh {
        GpuSkeletalMesh::from_model(
            &self.device,
            &self.skeletal_pipeline.bone_bind_group_layout,
            model,
        )
    }
}

/// Window surface presentation blitter
pub struct SurfaceBlitter {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
}

impl SurfaceBlitter {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Surface Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Surface Blit Bind Group Layout"),
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
            label: Some("Surface Blit Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Surface Blit Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Surface Blit Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    /// Creates a cached bind group for the given source texture view.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        src_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Surface Blit Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Renders using a pre-cached bind group to eliminate per-frame descriptor allocations.
    pub fn render_with_bind_group(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &wgpu::BindGroup,
        dst_view: &wgpu::TextureView,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Surface Blit Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    /// Convenience render method that generates a bind group on demand.
    pub fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        src_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
    ) {
        let bind_group = self.create_bind_group(device, src_view);
        self.render_with_bind_group(encoder, &bind_group, dst_view);
    }
}
