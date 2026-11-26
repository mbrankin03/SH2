//! Stenciled planar mirror render pass (`SH2-101`) and mirror skeletal draw helper.

use std::sync::Arc;

use sh2_assets::formats::map::BatchGroup;
use sh2_core::events::stageparts::MapPartVisibilityFilter;

use crate::gpu_map::GpuMap;
use crate::mirror::PlanarMirrorSpec;
use crate::skeletal::SkeletalPassStage;
use crate::{RenderContext, SkeletalDrawItem};

impl RenderContext {
    /// Runs Pass 0d (Stenciled Planar Mirror Render Pass) if an active [`PlanarMirrorSpec`] is set.
    ///
    /// Returns `Some(mirror_spec)` when the mirror pass ran and restored the unclipped main camera
    /// into `self.camera_buffer` (so the caller should use `LoadOp::Load` on `hdr_color_view` and
    /// prime `depth_view` with the mirror opening depth before drawing opaque room geometry).
    pub(super) fn encode_planar_mirror_pass(
        &self,
        gpu_map: Option<&GpuMap>,
        map_texture_bind_groups: Option<&[Arc<wgpu::BindGroup>]>,
        skeletal_meshes: &[SkeletalDrawItem],
        vis_filter: Option<&MapPartVisibilityFilter>,
    ) -> Option<PlanarMirrorSpec> {
        let mirror_spec = self.planar_mirror.lock().ok().and_then(|g| *g)?;
        let main_cam = self.last_camera_uniforms.lock().map(|g| *g).ok()?;

        let refl_cam = crate::mirror::reflected_camera_uniforms(&main_cam, &mirror_spec);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&refl_cam));

        let mut mirror_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Stenciled Planar Mirror Encoder"),
                });
        {
            let mut rpass = mirror_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Stenciled Planar Mirror Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.room_clear_color()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.stencil_depth_view,
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

            rpass.set_stencil_reference(1);

            // Step 1: Write stencil = 1 inside the mirror surface footprint.
            let mut drew_decal_mask = false;
            if let (Some(map), Some(batch_idx)) = (gpu_map, mirror_spec.decal_batch_index) {
                if let Some(batch) = map.batches.get(batch_idx) {
                    if map.is_batch_visible_with(batch, vis_filter) {
                        rpass.set_pipeline(&self.mirror_pipeline.map_stencil_mask_pipeline);
                        rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                        rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            batch.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
                        drew_decal_mask = true;
                    }
                }
            }
            if !drew_decal_mask {
                rpass.set_pipeline(&self.mirror_pipeline.stencil_mask_pipeline);
                rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.mirror_pipeline.quad_vertex_buffer.slice(..));
                rpass.set_index_buffer(
                    self.mirror_pipeline.quad_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                rpass.draw_indexed(0..6, 0, 0..1);
            }

            // Step 2: Render reflected opaque map geometry & skeletal meshes where stencil == 1.
            if let Some(map) = gpu_map {
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Opaque,
                    &self.geometry_pipeline.mirror_reflected_pipeline,
                    vis_filter,
                );
            }
            if !skeletal_meshes.is_empty() {
                self.draw_skeletal_meshes_mirror(&mut rpass, skeletal_meshes);
            }
        }
        self.queue.submit(Some(mirror_encoder.finish()));

        // Restore the unclipped main camera for Pass 1 (`er01.map`'s recessed solid concrete upper
        // wall sits at `X = -20064.1` behind the `X = -20000.0` mirror plane, so clipping the main
        // pass would erase the wall and expose the clear color). Instead, Pass 1 writes mirror-plane
        // depth into the exact mirror opening before `BatchGroup::Opaque` so pre-baked reflected
        // geometry behind the opening is hardware depth-rejected while the real wall stays intact.
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&main_cam));
        Some(mirror_spec)
    }

    pub(super) fn draw_skeletal_meshes_mirror<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        skeletal_meshes: &'a [SkeletalDrawItem<'a>],
    ) {
        let stages = [
            SkeletalPassStage::Opaque,
            SkeletalPassStage::TransparentCore,
            SkeletalPassStage::TransparentFringe,
        ];
        for stage in stages {
            let want_transparent = stage != SkeletalPassStage::Opaque;
            let has_any = skeletal_meshes.iter().any(|(mesh, _, _)| {
                mesh.primitives
                    .iter()
                    .any(|p| p.is_transparent == want_transparent && p.index_count > 0)
            });
            if !has_any {
                continue;
            }
            let mut active_cull: Option<bool> = None;
            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
            for (mesh, _matrices, texture_groups) in skeletal_meshes {
                rpass.set_bind_group(1, &mesh.bone_bind_group, &[]);
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                let mut current_tex: Option<u16> = None;
                for prim in &mesh.primitives {
                    if prim.is_transparent != want_transparent {
                        continue;
                    }
                    // Always draw the full character (including head & hair) inside the mirror reflection.
                    if let Some(range) = prim.draw_range(mesh.player_visibility, true) {
                        if active_cull != Some(prim.backface_culling) {
                            rpass.set_pipeline(
                                self.skeletal_pipeline
                                    .select_mirror_pipeline(stage, prim.backface_culling),
                            );
                            active_cull = Some(prim.backface_culling);
                        }
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
                        rpass.set_bind_group(
                            3,
                            &mesh.material_bind_group,
                            &[prim.material_dynamic_offset],
                        );
                        rpass.draw_indexed(range, 0, 0..1);
                    }
                }
            }
        }
    }
}
