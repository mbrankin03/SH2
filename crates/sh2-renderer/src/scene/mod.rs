//! Scene render pass encoding, map batch submission, skeletal mesh passes, and compute dispatch for [`RenderContext`].

mod mirror_pass;
mod skeletal_pass;
mod water_pass;

use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::gpu_map::GpuMap;
use crate::{RenderContext, SkeletalDrawItem};
use sh2_assets::formats::map::{BatchGroup, MapFile};
use sh2_core::events::stageparts::MapPartVisibilityFilter;

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

        // Upload skeletal bone matrices up-front so BOTH the PCSS spotlight shadow pass and main scene pass
        // observe the exact posed character skeletons for self-shadowing (`SH2-81`).
        for (i, (mesh, matrices, _)) in skeletal_meshes.iter().enumerate() {
            let is_mirror_duplicate = skeletal_meshes[..i]
                .iter()
                .any(|(prev_mesh, _, _)| std::ptr::eq(*prev_mesh, *mesh));
            if is_mirror_duplicate {
                mesh.update_mirror_bone_matrices(&self.queue, matrices);
            } else {
                mesh.update_bone_matrices(&self.queue, matrices);
            }
        }

        // Snapshot the active `.map` `geo_id` visibility filter once per frame (`EvDispControlModelExec`).
        let vis_filter = self.map_visibility_filter();

        // 0. Spotlight Shadow Depth Passes:
        //    0a. 512x512 volumetric god-ray occlusion shadow map (`SH2-32`)
        if self.config.enable_fog
            && self
                .volumetric_god_rays_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.spotlight_shadow_pipeline.encode_pass(
                &mut encoder,
                &self.shadow_depth_view,
                &self.spotlight_shadow_bind_group,
                gpu_map,
                vis_filter.as_ref(),
            );
        }
        //    0b. 2048x2048 Native PCSS spotlight shadow map (static map + animated skeletal self-shadowing, `SH2-81`)
        if self.pcss_enabled.load(std::sync::atomic::Ordering::Relaxed) {
            self.pcss_shadow_pipeline.encode_pass(
                &mut encoder,
                &self.pcss_shadow_depth_view,
                &self.pcss_shadow_bind_group,
                gpu_map,
                skeletal_meshes,
                vis_filter.as_ref(),
            );
        }

        // 0c. Native-Upgrade Toluca Lake Planar Reflection Pass (`SH2-80`)
        self.encode_water_reflection_pass(
            gpu_map,
            map_texture_bind_groups,
            skeletal_meshes,
            vis_filter.as_ref(),
        );

        // 0d. Stenciled Planar Mirror Render Pass (`SH2-101` / `SH2-151`)
        let active_mirror = self.encode_planar_mirror_pass(
            gpu_map,
            map_texture_bind_groups,
            skeletal_meshes,
            vis_filter.as_ref(),
        );

        // Determine PS2 `Check_DrawOrder` (`0x001a08a0`): on maps `0x000a007d` / `0x00090016`
        // during gameplay, stencil shadows are composited onto the static room BEFORE characters.
        let stencil_verts = self
            .shadow_volume_vertex_count
            .load(std::sync::atomic::Ordering::Relaxed);
        let stencil_active =
            stencil_verts > 0 && !self.pcss_enabled.load(std::sync::atomic::Ordering::Relaxed);
        let shadow_before_character = stencil_active
            && self.shadow_draw_order() == crate::shadow::ShadowDrawOrder::ShadowBeforeCharacter;

        // 1. Geometry & Skeletal Meshes Render Pass
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Geometry & Skeletal Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if active_mirror.is_some() {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(self.room_clear_color())
                        },
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

            // 1a-0. Prime mirror-plane depth into `depth_view` ONLY inside the authentic mirror
            //       opening before drawing opaque room geometry (`SH2-151`). This hardware depth-
            //       rejects pre-baked duplicate reflected room geometry behind the opening while
            //       leaving the surrounding concrete upper wall (at `X = -20064.1` in `er01.map`)
            //       and all foreground geometry (`X > -20000.0`) completely intact, and also gives
            //       volumetric fog the mirror-wall depth rather than the far plane.
            if let Some(mirror_spec) = active_mirror {
                let mut wrote_decal_depth = false;
                if let (Some(map), Some(batch_idx)) = (gpu_map, mirror_spec.decal_batch_index) {
                    if let Some(batch) = map.batches.get(batch_idx) {
                        if map.is_batch_visible_with(batch, vis_filter.as_ref()) {
                            rpass.set_pipeline(&self.mirror_pipeline.map_depth_write_pipeline);
                            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                            rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            rpass.set_index_buffer(
                                batch.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
                            wrote_decal_depth = true;
                        }
                    }
                }
                if !wrote_decal_depth {
                    rpass.set_pipeline(&self.mirror_pipeline.depth_write_pipeline);
                    rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                    rpass.set_vertex_buffer(0, self.mirror_pipeline.quad_vertex_buffer.slice(..));
                    rpass.set_index_buffer(
                        self.mirror_pipeline.quad_index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    rpass.draw_indexed(0..6, 0, 0..1);
                }
            }

            // 1a. Render static level Opaque + Alpha-Cutout batches first
            if let Some(map) = gpu_map {
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Opaque,
                    &self.geometry_pipeline.pipeline,
                    vis_filter.as_ref(),
                );
            }

            // 1b. Render skeletal character meshes in the main pass unless `Check_DrawOrder`
            // requested `ShadowBeforeCharacter` (`0x001a08a0`).
            if !shadow_before_character && !skeletal_meshes.is_empty() {
                self.draw_skeletal_meshes(&mut rpass, skeletal_meshes);
            }

            // 1c. Render static level Coplanar Decals
            if let Some(map) = gpu_map {
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Decal,
                    &self.geometry_pipeline.decal_pipeline,
                    vis_filter.as_ref(),
                );
            }

            // 1c-2. Optional `glas03.tbn2` dirty mirror glass overlay when explicitly enabled.
            if active_mirror.is_some()
                && self
                    .mirror_glass_overlay_enabled
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                rpass.set_pipeline(&self.mirror_pipeline.glass_overlay_pipeline);
                rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                rpass.set_bind_group(1, &self.mirror_pipeline.glass_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.mirror_pipeline.quad_vertex_buffer.slice(..));
                rpass.set_index_buffer(
                    self.mirror_pipeline.quad_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                rpass.draw_indexed(0..6, 0, 0..1);
            }

            // 1d. Render static level semi-transparent batches
            if let Some(map) = gpu_map {
                self.draw_map_batches_for_group(
                    &mut rpass,
                    map,
                    map_texture_bind_groups,
                    BatchGroup::Transparent,
                    &self.geometry_pipeline.transparent_pipeline,
                    vis_filter.as_ref(),
                );
            }

            // 1d-1. Render Toluca Lake water surface (`SH2-80`)
            if self
                .lake_water_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                self.water_pipeline.draw(&mut rpass);
            }

            // 1d-2. Render flattened PS2 `.kg1` floor drop shadows (`0x5a7c90`) when active (`SH2-81`)
            let drop_verts = self
                .drop_shadow_vertex_count
                .load(std::sync::atomic::Ordering::Relaxed);
            if drop_verts > 0 {
                let byte_len = (drop_verts as usize
                    * std::mem::size_of::<crate::shadow::ShadowVolumeVertex>())
                    as wgpu::BufferAddress;
                rpass.set_pipeline(&self.stencil_shadow_pipeline.drop_shadow_pipeline);
                rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                rpass.set_bind_group(1, &self.shadow_darken_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.drop_shadow_buffer.slice(0..byte_len));
                rpass.draw(0..drop_verts, 0..1);
            }

            // 1d-3. Render localized 3D proximity fog particle billboards (`fogMakePacket` @ 0x0016bb70)
            if self.config.enable_fog {
                let particle_count = self
                    .fog_particle_count
                    .load(std::sync::atomic::Ordering::Relaxed);
                if particle_count > 0 {
                    let byte_len = (particle_count as usize
                        * std::mem::size_of::<crate::fog::GpuFogParticle>())
                        as wgpu::BufferAddress;
                    rpass.set_pipeline(&self.fog_particle_pipeline.pipeline);
                    rpass.set_bind_group(0, &self.fog_particle_bind_group, &[]);
                    rpass.set_vertex_buffer(0, self.fog_particle_buffer.slice(0..byte_len));
                    rpass.draw(0..6, 0..particle_count);
                }
            }

            // 1d-4. Render Hyper Spray / Lying Figure 3D mist billboards (`enEfctDrawSpray`, `SH2-81`)
            let spray_count = self
                .spray_mist_particle_count
                .load(std::sync::atomic::Ordering::Relaxed);
            if spray_count > 0 {
                let byte_len = (spray_count as usize
                    * std::mem::size_of::<crate::effects::GpuSprayParticle>())
                    as wgpu::BufferAddress;
                rpass.set_pipeline(&self.spray_mist_pipeline.pipeline);
                rpass.set_bind_group(0, &self.spray_mist_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.spray_mist_buffer.slice(0..byte_len));
                rpass.draw(0..6, 0..spray_count);
            }

            // 1d-5. Render `Effect2/hh_class_dust_00.c` airborne dust motes (`SH2-98`)
            let dust_count = self
                .dust_particle_count
                .load(std::sync::atomic::Ordering::Relaxed);
            if dust_count > 0 {
                let byte_len = (dust_count as usize
                    * std::mem::size_of::<crate::effects::GpuDustParticle>())
                    as wgpu::BufferAddress;
                rpass.set_pipeline(&self.dust_pipeline.pipeline);
                rpass.set_bind_group(0, &self.dust_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.dust_buffer.slice(0..byte_len));
                rpass.draw(0..6, 0..dust_count);
            }

            // 1d-6. Render `SH2-100` alpha-blended & additive 3D FX batches with per-sheet `.tbn2` bind groups
            if let Ok(fx_batches) = self.billboard_fx_batches.lock().map(|g| g.clone()) {
                let stride = std::mem::size_of::<crate::effects::GpuFxQuad>() as u64;
                if !fx_batches.alpha.is_empty() {
                    rpass.set_pipeline(&self.billboard_fx_pipeline.pipeline_alpha);
                    for batch in &fx_batches.alpha {
                        let byte_start = u64::from(batch.start) * stride;
                        let byte_end = u64::from(batch.start + batch.count) * stride;
                        rpass.set_bind_group(
                            0,
                            &self.billboard_fx_bind_groups[batch.slot as usize],
                            &[],
                        );
                        rpass.set_vertex_buffer(
                            0,
                            self.billboard_fx_alpha_buffer.slice(byte_start..byte_end),
                        );
                        rpass.draw(0..6, 0..batch.count);
                    }
                }
                if !fx_batches.additive.is_empty() {
                    rpass.set_pipeline(&self.billboard_fx_pipeline.pipeline_additive);
                    for batch in &fx_batches.additive {
                        let byte_start = u64::from(batch.start) * stride;
                        let byte_end = u64::from(batch.start + batch.count) * stride;
                        rpass.set_bind_group(
                            0,
                            &self.billboard_fx_bind_groups[batch.slot as usize],
                            &[],
                        );
                        rpass.set_vertex_buffer(
                            0,
                            self.billboard_fx_additive_buffer
                                .slice(byte_start..byte_end),
                        );
                        rpass.draw(0..6, 0..batch.count);
                    }
                }
            }

            // 1d-7. Render `Effect2/hh_class_blood_*.c` subtractive blood pools, footprints, & sprays (`SH2-79`)
            let blood_count = self
                .blood_quad_count
                .load(std::sync::atomic::Ordering::Relaxed);
            if blood_count > 0 {
                let byte_len = (blood_count as usize
                    * std::mem::size_of::<crate::effects::GpuBloodQuad>())
                    as wgpu::BufferAddress;
                let pipe = if self
                    .blood_subtractive_mode
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    &self.blood_pipeline.subtractive_pipeline
                } else {
                    &self.blood_pipeline.alpha_pipeline
                };
                rpass.set_pipeline(pipe);
                rpass.set_bind_group(0, &self.blood_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.blood_buffer.slice(0..byte_len));
                rpass.draw(0..6, 0..blood_count);
            }

            // 1e. Render 3D debug lines (skeleton hierarchy, joint axes, wireframe overlay)
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

        // 1.5. PS2 / SH2:EE Two-Pass Stencil Shadow Volume + 4-Pass Soft Blur (`0x001767e0`, `0x00175f90`)
        if stencil_active {
            if self
                .soft_shadow_blur_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                self.stencil_shadow_pipeline.encode_soft_stencil_blur_pass(
                    &mut encoder,
                    &self.hdr_color_view,
                    &self.stencil_depth_view,
                    &self.shadow_mask_view,
                    &self.shadow_mask_bind_group,
                    &self.shadow_blur_pipeline,
                    self.shadow_composite_mode(),
                    &self.geometry_bind_group,
                    &self.shadow_darken_bind_group,
                    gpu_map,
                    vis_filter.as_ref(),
                    &self.shadow_volume_buffer,
                    stencil_verts,
                );
            } else {
                self.stencil_shadow_pipeline.encode_stencil_volume_pass(
                    &mut encoder,
                    &self.hdr_color_view,
                    &self.stencil_depth_view,
                    &self.geometry_bind_group,
                    &self.shadow_darken_bind_group,
                    gpu_map,
                    vis_filter.as_ref(),
                    &self.shadow_volume_buffer,
                    stencil_verts,
                );
            }
        }

        // 1.6. Deferred skeletal character pass when `Check_DrawOrder` (`0x001a08a0`) returned
        // `ShadowBeforeCharacter` (`0x000a007d` / `0x00090016` during gameplay).
        if shadow_before_character && !skeletal_meshes.is_empty() {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PS2 Check_DrawOrder Deferred Character Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_color_view,
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
            self.draw_skeletal_meshes(&mut rpass, skeletal_meshes);
        }

        // 1.75. Additive 7-Sprite Screen-Space Lens Flares (`Lens/lens_flare.c`, `SH2-81`)
        let flare_count = self
            .lens_flare_quad_count
            .load(std::sync::atomic::Ordering::Relaxed);
        if flare_count > 0 {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lens Flare Additive Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let byte_len = (flare_count as usize
                * std::mem::size_of::<crate::effects::GpuLensFlareQuad>())
                as wgpu::BufferAddress;
            rpass.set_pipeline(&self.lens_flare_pipeline.pipeline);
            rpass.set_bind_group(0, &self.lens_flare_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.lens_flare_buffer.slice(0..byte_len));
            rpass.draw(0..6, 0..flare_count);
        }

        // 2. Volumetric Fog & Post-Processing Compute Passes
        self.dispatch_compute_passes(&mut encoder);

        self.queue.submit(std::iter::once(encoder.finish()));
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
    pub(super) fn draw_map_batches_for_group<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        map: &'a GpuMap,
        map_texture_bind_groups: Option<&'a [Arc<wgpu::BindGroup>]>,
        target_group: BatchGroup,
        pipeline: &'a wgpu::RenderPipeline,
        vis_filter: Option<&MapPartVisibilityFilter>,
    ) {
        if !map.batches.iter().any(|b| b.group == target_group) {
            return;
        }

        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
        let mut current_bg: Option<usize> = None;

        for batch in &map.batches {
            if batch.group != target_group || !map.is_batch_visible_with(batch, vis_filter) {
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
