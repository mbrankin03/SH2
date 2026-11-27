//! Main HDR geometry, skeletal, decal, transparent, particle, blood, and debug-line pass encoding
//! plus consolidated `GpuMap` batch submission (`SH2-182`).

use std::sync::Arc;

use sh2_assets::formats::map::BatchGroup;
use sh2_core::events::stageparts::MapPartVisibilityFilter;

use crate::frustum::Frustum;
use crate::gpu_map::GpuMap;
use crate::mirror::PlanarMirrorSpec;
use crate::{RenderContext, SkeletalDrawItem};

impl RenderContext {
    /// Encodes Pass 1 (Main HDR Geometry, Skeletal, Decals, Transparent, Effects, and Debug Lines).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_main_geometry_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        gpu_map: Option<&GpuMap>,
        map_texture_bind_groups: Option<&[Arc<wgpu::BindGroup>]>,
        skeletal_meshes: &[SkeletalDrawItem],
        debug_lines: &[sh2_debug::DebugLineVertex],
        overflow_debug_line_buffer: Option<&wgpu::Buffer>,
        vis_filter: Option<&MapPartVisibilityFilter>,
        main_frustum: Option<&Frustum>,
        active_mirror: Option<PlanarMirrorSpec>,
        shadow_before_character: bool,
    ) {
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
            if let Some(map) = gpu_map {
                if let (Some(batch), Some(vbuf), Some(ibuf)) = (
                    map.resolve_mirror_decal_batch(&mirror_spec),
                    &map.vertex_buffer,
                    &map.index_buffer,
                ) {
                    if map.is_batch_visible_with(batch, vis_filter) {
                        if let Some(index_range) = batch.index_range() {
                            rpass.set_pipeline(&self.mirror_pipeline.map_depth_write_pipeline);
                            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
                            rpass.set_vertex_buffer(0, vbuf.slice(..));
                            rpass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                            rpass.draw_indexed(index_range, batch.base_vertex, 0..1);
                            wrote_decal_depth = true;
                        }
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
            self.draw_map_batches_for_group_with_camera(
                &mut rpass,
                map,
                map_texture_bind_groups,
                BatchGroup::Opaque,
                &self.geometry_pipeline.pipeline,
                &self.geometry_bind_group,
                vis_filter,
                main_frustum,
            );
        }

        // 1b. Render skeletal character meshes in the main pass unless `Check_DrawOrder`
        // requested `ShadowBeforeCharacter` (`0x001a08a0`).
        if !shadow_before_character && !skeletal_meshes.is_empty() {
            self.draw_skeletal_meshes(&mut rpass, skeletal_meshes);
        }

        // 1c. Render static level Coplanar Decals
        if let Some(map) = gpu_map {
            self.draw_map_batches_for_group_with_camera(
                &mut rpass,
                map,
                map_texture_bind_groups,
                BatchGroup::Decal,
                &self.geometry_pipeline.decal_pipeline,
                &self.geometry_bind_group,
                vis_filter,
                main_frustum,
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
            self.draw_map_batches_for_group_with_camera(
                &mut rpass,
                map,
                map_texture_bind_groups,
                BatchGroup::Transparent,
                &self.geometry_pipeline.transparent_pipeline,
                &self.geometry_bind_group,
                vis_filter,
                main_frustum,
            );
        }

        // 1d-1. Render Toluca Lake water surface (`SH2-80`) in Pass 1 only when the depth-fog
        // compute pass is disabled; when `enable_fog` is true, water is rendered post-depth-fog
        // in `dispatch_compute_passes` so it is never double-fogged (`SH2-168`).
        if !self.config.enable_fog
            && self
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

        // 1d-3. (3D proximity fog particle billboards are rendered post-depth-fog in
        // `dispatch_compute_passes` matching `sh2pc.exe` `FUN_00486fc0` `0x00486fc0`
        // `D3DRS_FOGENABLE = 0` so dark depth-fog `FOGCOL` never darkens bright mist sprites.)

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
        if let Ok(fx_batches) = self.billboard_fx_batches.lock() {
            if !fx_batches.alpha.is_empty() || !fx_batches.additive.is_empty() {
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
            let line_buf = overflow_debug_line_buffer.unwrap_or(&self.debug_line_buffer);
            let byte_len = std::mem::size_of_val(debug_lines) as wgpu::BufferAddress;
            rpass.set_pipeline(&self.debug_line_pipeline.pipeline);
            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);
            rpass.set_vertex_buffer(0, line_buf.slice(0..byte_len));
            rpass.draw(0..(debug_lines.len() as u32), 0..1);
        }
    }

    /// Helper to draw all `GpuMapBatch` items belonging to a specific [`BatchGroup`]
    /// using the designated pipeline (`pipeline`, `decal_pipeline`, or `transparent_pipeline`),
    /// binding [`GpuMap::vertex_buffer`] and [`GpuMap::index_buffer`] once and culling against
    /// `frustum` (`SH2-182`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_map_batches_for_group_with_camera<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        map: &'a GpuMap,
        map_texture_bind_groups: Option<&'a [Arc<wgpu::BindGroup>]>,
        target_group: BatchGroup,
        pipeline: &'a wgpu::RenderPipeline,
        common_bind_group: &'a wgpu::BindGroup,
        vis_filter: Option<&MapPartVisibilityFilter>,
        frustum: Option<&Frustum>,
    ) {
        let batches = map.batches_for_group(target_group);
        if batches.is_empty() {
            return;
        }
        let (Some(vbuf), Some(ibuf)) = (&map.vertex_buffer, &map.index_buffer) else {
            return;
        };

        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, common_bind_group, &[]);
        rpass.set_vertex_buffer(0, vbuf.slice(..));
        rpass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
        let mut current_bg: Option<usize> = None;

        for batch in batches {
            if !map.is_batch_visible_with(batch, vis_filter) {
                continue;
            }
            if let Some(f) = frustum {
                if !f.intersects_aabb(batch.bounds_min, batch.bounds_max) {
                    continue;
                }
            }
            let Some(index_range) = batch.index_range() else {
                continue;
            };
            let bg_idx = batch.material_index;
            let bind_group = match map_texture_bind_groups {
                Some(groups) if bg_idx < groups.len() => &groups[bg_idx],
                _ => &self.default_texture_bind_group,
            };

            if current_bg != Some(bg_idx) {
                rpass.set_bind_group(1, &**bind_group, &[]);
                current_bg = Some(bg_idx);
            }

            rpass.draw_indexed(index_range, batch.base_vertex, 0..1);
        }
    }
}
