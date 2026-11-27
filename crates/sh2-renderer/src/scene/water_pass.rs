//! Toluca Lake planar water reflection pass (`SH2-80`, unified single-encoder pass `SH2-182`).

use sh2_assets::formats::map::BatchGroup;
use sh2_core::events::stageparts::MapPartVisibilityFilter;

use crate::frustum::Frustum;
use crate::{MapDrawItem, RenderContext, SkeletalDrawItem};

impl RenderContext {
    /// Encodes Pass 0c (Toluca Lake Planar Reflection Pass) into `encoder` when both
    /// `lake_water_enabled` and `water_reflection_pass_enabled` are active, using the dedicated
    /// [`Self::water_camera_buffer`] and [`Self::water_geometry_bind_group`] so the entire frame
    /// submits in a single `CommandEncoder` (`SH2-182`).
    pub(super) fn encode_water_reflection_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        maps: &[MapDrawItem<'_>],
        skeletal_meshes: &[SkeletalDrawItem],
        vis_filter: Option<&MapPartVisibilityFilter>,
    ) {
        if !self
            .lake_water_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
            || !self
                .water_reflection_pass_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        let Ok(main_cam) = self.last_camera_uniforms.lock().map(|g| *g) else {
            return;
        };
        let plane_y = f32::from_bits(
            self.water_plane_height_mm
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        let main_vp = glam::Mat4::from_cols_array_2d(&main_cam.view_proj);
        let refl_vp = crate::water::mirrored_reflection_view_proj(main_vp, plane_y);
        let refl_frustum = Frustum::from_view_proj(refl_vp);
        let refl_cam_pos = crate::water::LakeWaterPipeline::mirrored_camera_pos(
            glam::Vec3::new(
                main_cam.cam_pos[0],
                main_cam.cam_pos[1],
                main_cam.cam_pos[2],
            ),
            plane_y,
        );
        let refl_uniforms = crate::pipeline::CameraUniforms {
            view_proj: refl_vp.to_cols_array_2d(),
            inv_proj: main_cam.inv_proj,
            cam_pos: [
                refl_cam_pos.x,
                refl_cam_pos.y,
                refl_cam_pos.z,
                main_cam.cam_pos[3],
            ],
            inv_view_proj: if refl_vp.determinant().abs() > 1e-12 {
                refl_vp.inverse().to_cols_array_2d()
            } else {
                main_cam.inv_view_proj
            },
            clip_plane: [0.0; 4],
        };
        self.queue.write_buffer(
            &self.water_camera_buffer,
            0,
            bytemuck::bytes_of(&refl_uniforms),
        );

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Water Planar Reflection Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.water_pipeline.reflection_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.room_clear_color()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.water_pipeline.reflection_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // Negative-determinant `mirrored_reflection_view_proj` flips CCW triangles to CW,
        // requiring CW front-face (`water_reflected_pipeline`) and inverted skeletal culling
        // (matching `mirror_pass.rs`).
        for item in maps {
            self.draw_map_batches_for_group_with_camera(
                &mut rpass,
                item.gpu_map,
                item.texture_bind_groups,
                BatchGroup::Opaque,
                &self.geometry_pipeline.water_reflected_pipeline,
                &self.water_geometry_bind_group,
                vis_filter,
                refl_frustum.as_ref(),
            );
        }
        if !skeletal_meshes.is_empty() {
            self.draw_skeletal_meshes_with_camera(
                &mut rpass,
                skeletal_meshes,
                &self.water_geometry_bind_group,
                true,
                false,
            );
        }
    }
}
