//! Post-geometry compute passes: Volumetric Fog, post-depth-fog Toluca Lake water & 3D fog
//! particles, ACES/PS2 Post-Processing, and `prev_frame_texture` feedback copy.

use crate::{RenderContext, SkeletalDrawItem};

impl RenderContext {
    /// Dispatches the Volumetric Fog and ACES/PS2 Post-Processing compute passes.
    pub(super) fn dispatch_compute_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        skeletal_meshes: &[SkeletalDrawItem<'_>],
    ) {
        let workgroups_x = self.config.width.div_ceil(16);
        let workgroups_y = self.config.height.div_ceil(16);

        if self.config.enable_fog {
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Volumetric Fog Compute Pass"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&self.fog_pipeline.pipeline);
                cpass.set_bind_group(0, &self.fog_bind_group, &[]);
                cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }

            // Render semi-transparent skeletal hair/eyelash fringe (`1 <= alpha_u8 <= 254`,
            // `depth_write_enabled: false`) AFTER `FogComputePipeline` with in-shader depth fog at
            // each hair fragment's own `view_z` (`SLUS_202.28` `Model3DrawVu0Parts` `0x0012bd50`,
            // `SortEnvPrim` `0x00128550`, `sh2pc.exe` `FUN_00503800` `0x00503800` `D3DRS_FOGENABLE = 1`),
            // preventing screen-space depth fog from overwriting hair tips over far backgrounds (`SH2-192`).
            if !skeletal_meshes.is_empty()
                && self.has_transparent_skeletal_primitives(skeletal_meshes)
            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Skeletal Transparent Fringe Post-Depth-Fog Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.fog_color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.draw_skeletal_transparent_fringe_post_fog(&mut rpass, skeletal_meshes);
            }

            // Render Toluca Lake water surface (`SH2-80`) AFTER `FogComputePipeline` when depth fog
            // is enabled so the water's own distance fog is never double-fogged by `FogComputePipeline`
            // (which otherwise sees `depth = 1.0` because water has `depth_write_enabled: false`, `SH2-168`).
            if self
                .lake_water_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Toluca Lake Water Post-Depth-Fog Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.fog_color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.water_pipeline.draw(&mut rpass);
            }

            // Render localized 3D proximity fog particle billboards (`fogMakePacket` `0x0016bb70`,
            // `sh2pc.exe` `FUN_00486fc0` `0x00486fc0` `sh2pc_00480000.c:4277,4307`:
            // `D3DRS_FOGENABLE (0x1c) = 0`) AFTER depth fog so dark `FOGCOL` never darkens them.
            let particle_count = self
                .fog_particle_count
                .load(std::sync::atomic::Ordering::Relaxed);
            if particle_count > 0 {
                let byte_len = (particle_count as usize
                    * std::mem::size_of::<crate::fog::GpuFogParticle>())
                    as wgpu::BufferAddress;
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("3D Fog Particle Post-Depth-Fog Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.fog_color_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rpass.set_pipeline(&self.fog_particle_pipeline.pipeline);
                rpass.set_bind_group(0, &self.fog_particle_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.fog_particle_buffer.slice(0..byte_len));
                rpass.draw(0..6, 0..particle_count);
            }
        }

        if self.config.enable_post {
            {
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
                cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }

            // Copy finished LDR output into `prev_frame_texture` for next frame's
            // `sh2gfw_Filter_Retain` (`0x00179b10`) / `sh2gfw_Filter_Blur` (`0x00177440`),
            // unless `POST_FLAG_FILTER_RETAIN` pause-freeze is active.
            if !self
                .filter_retain_frozen
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                encoder.copy_texture_to_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.ldr_output_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::ImageCopyTexture {
                        texture: &self.prev_frame_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: self.config.width.max(1),
                        height: self.config.height.max(1),
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
    }
}
