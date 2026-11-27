//! Scene render pass orchestration for [`RenderContext`]: shadow depth passes, planar water &
//! mirror reflections, main HDR geometry/skeletal/effects pass, stencil shadows, lens flares,
//! and compute fog/post-processing dispatch.

mod compute_pass;
mod main_pass;
mod mirror_pass;
mod skeletal_pass;
mod water_pass;

use std::sync::Arc;
use wgpu::util::DeviceExt;

use crate::frustum::Frustum;
use crate::gpu_map::GpuMap;
use crate::{RenderContext, SkeletalDrawItem};
use sh2_assets::formats::map::MapFile;

/// Borrowed static `.map` geometry paired with its per-material diffuse texture bind groups
/// for unified single-pass multi-tile outdoor scene rendering (`SH2-183`).
#[derive(Clone, Copy)]
pub struct MapDrawItem<'a> {
    /// Compiled static level geometry for a room or outdoor tile.
    pub gpu_map: &'a GpuMap,
    /// Per-material diffuse texture bind groups indexed by [`crate::GpuMapBatch::material_index`].
    pub texture_bind_groups: Option<&'a [Arc<wgpu::BindGroup>]>,
}

impl<'a> MapDrawItem<'a> {
    /// Creates a [`MapDrawItem`] pairing `gpu_map` with optional per-material texture bind groups.
    #[inline]
    #[must_use]
    pub const fn new(
        gpu_map: &'a GpuMap,
        texture_bind_groups: Option<&'a [Arc<wgpu::BindGroup>]>,
    ) -> Self {
        Self {
            gpu_map,
            texture_bind_groups,
        }
    }
}

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

    /// Encodes static world level map batches and dynamic animated skeletal meshes into an
    /// existing `CommandEncoder` without submitting (`SH2-182`).
    pub fn encode_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        gpu_map: Option<&GpuMap>,
        map_texture_bind_groups: Option<&[Arc<wgpu::BindGroup>]>,
        skeletal_meshes: &[SkeletalDrawItem],
    ) {
        self.encode_scene_with_debug_lines(
            encoder,
            gpu_map,
            map_texture_bind_groups,
            skeletal_meshes,
            &[],
        );
    }

    /// Renders static world level map batches, dynamic animated skeletal meshes, and 3D debug lines
    /// (skeleton joints, wireframe overlays, bounding boxes) in a unified pass and submits once.
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
        self.encode_scene_with_debug_lines(
            &mut encoder,
            gpu_map,
            map_texture_bind_groups,
            skeletal_meshes,
            debug_lines,
        );
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Encodes all 3D scene passes (spotlight & PCSS shadows, water & planar mirror reflections,
    /// main geometry/skeletal/effects pass, stencil shadows, lens flares, fog, and post-processing)
    /// into a single `CommandEncoder` without submitting (`SH2-182`).
    pub fn encode_scene_with_debug_lines(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        gpu_map: Option<&GpuMap>,
        map_texture_bind_groups: Option<&[Arc<wgpu::BindGroup>]>,
        skeletal_meshes: &[SkeletalDrawItem],
        debug_lines: &[sh2_debug::DebugLineVertex],
    ) {
        let single;
        let maps: &[MapDrawItem<'_>] = if let Some(map) = gpu_map {
            single = [MapDrawItem::new(map, map_texture_bind_groups)];
            &single
        } else {
            &[]
        };
        self.encode_scene_multi_map_with_debug_lines(encoder, maps, skeletal_meshes, debug_lines);
    }

    /// Renders multiple static [`GpuMap`] tiles (e.g. streamed outdoor `ca*` / `cb*` tiles) and
    /// dynamic animated skeletal meshes in a single unified pass (`SH2-183`).
    pub fn render_scene_multi_map(
        &self,
        maps: &[MapDrawItem<'_>],
        skeletal_meshes: &[SkeletalDrawItem],
    ) {
        self.render_scene_multi_map_with_debug_lines(maps, skeletal_meshes, &[]);
    }

    /// Renders multiple static [`GpuMap`] tiles, dynamic animated skeletal meshes, and 3D debug
    /// lines in a single unified pass and submits once (`SH2-183`).
    pub fn render_scene_multi_map_with_debug_lines(
        &self,
        maps: &[MapDrawItem<'_>],
        skeletal_meshes: &[SkeletalDrawItem],
        debug_lines: &[sh2_debug::DebugLineVertex],
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Multi-Map Scene Render Encoder"),
            });
        self.encode_scene_multi_map_with_debug_lines(
            &mut encoder,
            maps,
            skeletal_meshes,
            debug_lines,
        );
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Encodes multiple static [`GpuMap`] tiles and dynamic animated skeletal meshes into an
    /// existing `CommandEncoder` without submitting (`SH2-183`).
    pub fn encode_scene_multi_map(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        maps: &[MapDrawItem<'_>],
        skeletal_meshes: &[SkeletalDrawItem],
    ) {
        self.encode_scene_multi_map_with_debug_lines(encoder, maps, skeletal_meshes, &[]);
    }

    /// Encodes all 3D scene passes across multiple static [`GpuMap`] tiles (e.g. streamed outdoor
    /// `ca*` / `cb*` tiles) into a single `CommandEncoder` with frustum culling (`SH2-183`).
    pub fn encode_scene_multi_map_with_debug_lines(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        maps: &[MapDrawItem<'_>],
        skeletal_meshes: &[SkeletalDrawItem],
        debug_lines: &[sh2_debug::DebugLineVertex],
    ) {
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
        self.prepare_skeletal_light_slots(skeletal_meshes);

        // Snapshot the active `.map` `geo_id` visibility filter and main-camera frustum once per frame.
        let vis_filter = self.map_visibility_filter();
        let main_frustum = self.last_camera_uniforms.lock().ok().and_then(|cam| {
            Frustum::from_view_proj(glam::Mat4::from_cols_array_2d(&cam.view_proj))
        });

        // 0. Spotlight Shadow Depth Passes:
        //    0a. 512x512 volumetric god-ray occlusion shadow map (`SH2-32`)
        if self.config.enable_fog
            && self
                .volumetric_god_rays_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            let shadow_frustum = self
                .last_shadow_view_proj
                .lock()
                .ok()
                .and_then(|vp| Frustum::from_view_proj(glam::Mat4::from_cols_array_2d(&vp)));
            self.spotlight_shadow_pipeline.encode_pass_multi(
                encoder,
                &self.shadow_depth_view,
                &self.spotlight_shadow_bind_group,
                maps,
                vis_filter.as_ref(),
                shadow_frustum.as_ref(),
            );
        }
        //    0b. 2048x2048 Native PCSS spotlight shadow map (static map + animated skeletal self-shadowing, `SH2-81`)
        if self.pcss_enabled.load(std::sync::atomic::Ordering::Relaxed) {
            let pcss_frustum = self
                .last_pcss_view_proj
                .lock()
                .ok()
                .and_then(|vp| Frustum::from_view_proj(glam::Mat4::from_cols_array_2d(&vp)));
            self.pcss_shadow_pipeline.encode_pass_multi(
                encoder,
                &self.pcss_shadow_depth_view,
                &self.pcss_shadow_bind_group,
                maps,
                skeletal_meshes,
                vis_filter.as_ref(),
                pcss_frustum.as_ref(),
            );
        }

        // 0c. Native-Upgrade Toluca Lake Planar Reflection Pass (`SH2-80`)
        self.encode_water_reflection_pass(encoder, maps, skeletal_meshes, vis_filter.as_ref());

        // 0d. Stenciled Planar Mirror Render Pass (`SH2-101` / `SH2-151`, indoor rooms only)
        let primary_map = maps.first().map(|m| m.gpu_map);
        let primary_tex_bgs = maps.first().and_then(|m| m.texture_bind_groups);
        let active_mirror = self.encode_planar_mirror_pass(
            encoder,
            primary_map,
            primary_tex_bgs,
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

        // 1. Main HDR Geometry, Skeletal, Decals, Transparent, Effects, and Debug Lines Pass
        self.encode_main_geometry_pass(
            encoder,
            maps,
            skeletal_meshes,
            debug_lines,
            overflow_debug_line_buffer.as_ref(),
            vis_filter.as_ref(),
            main_frustum.as_ref(),
            active_mirror,
            shadow_before_character,
        );

        // 1.5. PS2 / SH2:EE Two-Pass Stencil Shadow Volume + 4-Pass Soft Blur (`0x001767e0`, `0x00175f90`)
        if stencil_active {
            if self
                .soft_shadow_blur_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                self.stencil_shadow_pipeline
                    .encode_soft_stencil_blur_pass_multi(
                        encoder,
                        &self.hdr_color_view,
                        &self.stencil_depth_view,
                        &self.shadow_mask_view,
                        &self.shadow_mask_bind_group,
                        &self.shadow_blur_pipeline,
                        self.shadow_composite_mode(),
                        &self.geometry_bind_group,
                        &self.shadow_darken_bind_group,
                        maps,
                        vis_filter.as_ref(),
                        &self.shadow_volume_buffer,
                        stencil_verts,
                        main_frustum.as_ref(),
                    );
            } else {
                self.stencil_shadow_pipeline
                    .encode_stencil_volume_pass_multi(
                        encoder,
                        &self.hdr_color_view,
                        &self.stencil_depth_view,
                        &self.geometry_bind_group,
                        &self.shadow_darken_bind_group,
                        maps,
                        vis_filter.as_ref(),
                        &self.shadow_volume_buffer,
                        stencil_verts,
                        main_frustum.as_ref(),
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
        self.dispatch_compute_passes(encoder, skeletal_meshes);
    }
}
