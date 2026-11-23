//! Offscreen render targets, compute bind group wiring, and window resize handling for [`RenderContext`].

use crate::{fog, post, shadow};

use super::RenderContext;

impl RenderContext {
    #[allow(clippy::type_complexity)]
    pub(super) fn create_offscreen_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (
        (wgpu::Texture, wgpu::TextureView),
        (wgpu::Texture, wgpu::TextureView),
        (wgpu::Texture, wgpu::TextureView),
        (wgpu::Texture, wgpu::TextureView),
    ) {
        let extent = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("32-bit Float Depth Texture"),
            size: extent,
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
            size: extent,
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
            size: extent,
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
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let ldr_output_view =
            ldr_output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        (
            (depth_texture, depth_view),
            (hdr_color_texture, hdr_color_view),
            (fog_color_texture, fog_color_view),
            (ldr_output_texture, ldr_output_view),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn create_compute_bind_groups(
        device: &wgpu::Device,
        fog_pipeline: &fog::FogComputePipeline,
        post_pipeline: &post::PostProcessPipeline,
        fog_buffer: &wgpu::Buffer,
        camera_buffer: &wgpu::Buffer,
        spotlight_buffer: &wgpu::Buffer,
        post_buffer: &wgpu::Buffer,
        depth_view: &wgpu::TextureView,
        hdr_color_view: &wgpu::TextureView,
        fog_color_view: &wgpu::TextureView,
        shadow_depth_view: &wgpu::TextureView,
        fog_noise_view: &wgpu::TextureView,
        fog_noise_sampler: &wgpu::Sampler,
        ldr_output_view: &wgpu::TextureView,
    ) -> (wgpu::BindGroup, wgpu::BindGroup, wgpu::BindGroup) {
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
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(hdr_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(fog_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: spotlight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(shadow_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(fog_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(fog_noise_sampler),
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
                    resource: wgpu::BindingResource::TextureView(fog_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(ldr_output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&post_pipeline.ps2_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&post_pipeline.ps2_noise_sampler),
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
                    resource: wgpu::BindingResource::TextureView(hdr_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(ldr_output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&post_pipeline.ps2_noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&post_pipeline.ps2_noise_sampler),
                },
            ],
        });

        (fog_bind_group, post_bind_group_fog, post_bind_group_direct)
    }

    /// Reconfigures offscreen intermediate targets (Depth, HDR, Fog, and LDR surfaces) when window resolution changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if self.config.width == width && self.config.height == height {
            return;
        }

        self.config.width = width;
        self.config.height = height;

        let (
            (depth_texture, depth_view),
            (hdr_color_texture, hdr_color_view),
            (fog_color_texture, fog_color_view),
            (ldr_output_texture, ldr_output_view),
        ) = Self::create_offscreen_targets(&self.device, width, height);

        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.hdr_color_texture = hdr_color_texture;
        self.hdr_color_view = hdr_color_view;
        self.fog_color_texture = fog_color_texture;
        self.fog_color_view = fog_color_view;
        self.ldr_output_texture = ldr_output_texture;
        self.ldr_output_view = ldr_output_view;

        let stencil_depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PS2 Stencil Shadow Depth24PlusStencil8 Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: shadow::StencilShadowPipeline::STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.stencil_depth_view =
            stencil_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.stencil_depth_texture = stencil_depth_texture;

        let shadow_mask_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PS2 Soft Stencil Shadow Mask Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: shadow::ShadowBlurPipeline::WORK_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_mask_view =
            shadow_mask_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.shadow_mask_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PS2 Soft Stencil Shadow Mask Bind Group"),
            layout: &self.shadow_blur_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        &self.shadow_blur_pipeline.linear_sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.shadow_blur_pipeline.uniform_buffer.as_entire_binding(),
                },
            ],
        });
        self.shadow_mask_view = shadow_mask_view;
        self.shadow_mask_texture = shadow_mask_texture;

        let (fog_bind_group, post_bind_group_fog, post_bind_group_direct) =
            Self::create_compute_bind_groups(
                &self.device,
                &self.fog_pipeline,
                &self.post_pipeline,
                &self.fog_buffer,
                &self.camera_buffer,
                &self.spotlight_buffer,
                &self.post_buffer,
                &self.depth_view,
                &self.hdr_color_view,
                &self.fog_color_view,
                &self.shadow_depth_view,
                &self.fog_noise_view,
                &self.fog_noise_sampler,
                &self.ldr_output_view,
            );

        self.fog_bind_group = fog_bind_group;
        self.post_bind_group_fog = post_bind_group_fog;
        self.post_bind_group_direct = post_bind_group_direct;
    }
}
