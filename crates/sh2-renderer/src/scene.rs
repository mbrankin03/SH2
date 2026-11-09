//! Scene render pass encoding, map batch submission, skeletal mesh passes, and compute dispatch for [`RenderContext`].

use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::gpu_map::GpuMap;
use crate::{RenderContext, SkeletalDrawItem};
use sh2_assets::formats::map::{BatchGroup, MapFile};

impl RenderContext {
    /// Render a pre-compiled `GpuMap` room into the framebuffers and run the compute passes.
    pub fn render_gpu_map(&self, gpu_map: &GpuMap) {
        self.render_scene(Some(gpu_map), None, &[]);
    }

    /// Convenience method for rendering directly from `MapFile`, compiling to `GpuMap`.
    pub fn render_map(&self, map: &MapFile) {
        let gpu_map = GpuMap::from_map(&self.device, map);
        self.render_gpu_map(&gpu_map);
    }

    /// Renders static world level map batches and dynamic animated skeletal meshes in a unified pass.
    pub fn render_scene(
        &self,
        gpu_map: Option<&GpuMap>,
        map_texture_bind_groups: Option<&[Arc<wgpu::BindGroup>]>,
        skeletal_meshes: &[SkeletalDrawItem],
    ) {
        self.render_scene_with_debug_lines(gpu_map, map_texture_bind_groups, skeletal_meshes, &[]);
    }

    /// Renders static world level map batches, dynamic animated skeletal meshes, and 3D debug lines
    /// (skeleton joints, wireframe overlays, bounding boxes) in a unified pass.
    pub fn render_scene_with_debug_lines(
        &self,
        gpu_map: Option<&GpuMap>,
        map_texture_bind_groups: Option<&[Arc<wgpu::BindGroup>]>,
        skeletal_meshes: &[SkeletalDrawItem],
        debug_lines: &[sh2_debug::DebugLineVertex],
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Render Encoder"),
            });

        // Upload debug lines into the persistent buffer (or allocate an overflow buffer if exceeding capacity)
        let overflow_debug_line_buffer = if debug_lines.len() > Self::MAX_DEBUG_LINE_VERTICES {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Overflow Debug Line Vertex Buffer"),
                        contents: bytemuck::cast_slice(debug_lines),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            if !debug_lines.is_empty() {
                self.queue.write_buffer(
                    &self.debug_line_buffer,
                    0,
                    bytemuck::cast_slice(debug_lines),
                );
            }
            None
        };

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

            // 1a. Render static level batches (Opaque + Alpha-Cutout, then Coplanar Decals)
            if let Some(map) = gpu_map {
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Opaque,
                    &self.geometry_pipeline.pipeline,
                );
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Decal,
                    &self.geometry_pipeline.decal_pipeline,
                );
            }

            // 1b. Render skeletal character meshes (opaque pass first, then alpha-blended transparent pass)
            if !skeletal_meshes.is_empty() {
                self.draw_skeletal_meshes(&mut rpass, skeletal_meshes);
            }

            // 1c. Render static level semi-transparent batches (glass, water, transparent foliage fringes)
            if let Some(map) = gpu_map {
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Transparent,
                    &self.geometry_pipeline.transparent_pipeline,
                );
            }

            // 1d. Render 3D debug lines (skeleton hierarchy, joint axes, wireframe overlay)
            if !debug_lines.is_empty() {
                let line_buf = overflow_debug_line_buffer
                    .as_ref()
                    .unwrap_or(&self.debug_line_buffer);
                let byte_len = std::mem::size_of_val(debug_lines) as wgpu::BufferAddress;
                rpass.set_pipeline(&self.debug_line_pipeline.pipeline);
                rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                rpass.set_vertex_buffer(0, line_buf.slice(0..byte_len));
                rpass.draw(0..(debug_lines.len() as u32), 0..1);
            }
        }

        // 2. Volumetric Fog & Post-Processing Compute Passes
        self.dispatch_compute_passes(&mut encoder);

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn draw_skeletal_meshes<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        skeletal_meshes: &'a [SkeletalDrawItem<'a>],
    ) {
        for (mesh, matrices, _) in skeletal_meshes {
            mesh.update_bone_matrices(&self.queue, matrices);
        }

        for transparent_pass in [false, true] {
            let has_any = skeletal_meshes.iter().any(|(mesh, _, _)| {
                mesh.primitives.iter().any(|p| {
                    p.index_count > 0 && p.is_active && p.is_transparent == transparent_pass
                })
            });
            if !has_any {
                continue;
            }

            if transparent_pass {
                rpass.set_pipeline(&self.skeletal_pipeline.transparent_pipeline);
            } else {
                rpass.set_pipeline(&self.skeletal_pipeline.pipeline);
            }
            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);

            for (mesh, _matrices, texture_groups) in skeletal_meshes {
                rpass.set_bind_group(1, &mesh.bone_bind_group, &[]);
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                let mut current_tex: Option<u16> = None;
                for prim in &mesh.primitives {
                    if prim.index_count > 0
                        && prim.is_active
                        && prim.is_transparent == transparent_pass
                    {
                        let tex_idx = prim.texture_index;
                        if current_tex != Some(tex_idx) {
                            let bg = match texture_groups {
                                Some(groups) if (tex_idx as usize) < groups.len() => {
                                    &groups[tex_idx as usize]
                                }
                                _ => &self.default_texture_bind_group,
                            };
                            rpass.set_bind_group(2, &**bg, &[]);
                            current_tex = Some(tex_idx);
                        }

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

    /// Dispatches the Volumetric Fog and ACES Post-Processing compute passes.
    fn dispatch_compute_passes(&self, encoder: &mut wgpu::CommandEncoder) {
        let workgroups_x = self.config.width.div_ceil(16);
        let workgroups_y = self.config.height.div_ceil(16);

        if self.config.enable_fog {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Volumetric Fog Compute Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.fog_pipeline.pipeline);
            cpass.set_bind_group(0, &self.fog_bind_group, &[]);
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

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
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
    }

    /// Helper to draw all `GpuMapBatch` items belonging to a specific [`BatchGroup`]
    /// using the designated pipeline (`pipeline`, `decal_pipeline`, or `transparent_pipeline`).
    fn draw_map_batches_for_group<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        map: &'a GpuMap,
        map_texture_bind_groups: Option<&'a [Arc<wgpu::BindGroup>]>,
        target_group: BatchGroup,
        pipeline: &'a wgpu::RenderPipeline,
    ) {
        if !map.batches.iter().any(|b| b.group == target_group) {
            return;
        }

        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
        let mut current_bg: Option<usize> = None;

        for batch in &map.batches {
            if batch.group != target_group {
                continue;
            }
            let bg_idx = batch.material_index;
            let bind_group = match map_texture_bind_groups {
                Some(groups) if bg_idx < groups.len() => &groups[bg_idx],
                _ => &self.default_texture_bind_group,
            };

            if current_bg != Some(bg_idx) {
                rpass.set_bind_group(1, &**bind_group, &[]);
                current_bg = Some(bg_idx);
            }

            rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
            rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
        }
    }
}
