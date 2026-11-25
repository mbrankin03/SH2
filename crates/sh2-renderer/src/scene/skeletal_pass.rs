//! Main-pass skeletal mesh draw helper for [`RenderContext`].

use crate::skeletal::SkeletalPassStage;
use crate::{RenderContext, SkeletalDrawItem};

impl RenderContext {
    pub(super) fn draw_skeletal_meshes<'a>(
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
            let has_any = skeletal_meshes.iter().enumerate().any(|(i, (mesh, _, _))| {
                let is_mirror_duplicate = skeletal_meshes[..i]
                    .iter()
                    .any(|(prev_mesh, _, _)| std::ptr::eq(*prev_mesh, *mesh));
                mesh.primitives.iter().any(|p| {
                    p.is_transparent == want_transparent
                        && p.draw_range(mesh.player_visibility, is_mirror_duplicate)
                            .is_some()
                })
            });
            if !has_any {
                continue;
            }

            let mut active_pipeline_key: Option<(bool, bool)> = None;
            rpass.set_bind_group(0, &self.geometry_bind_group, &[]);

            for (i, (mesh, _matrices, texture_groups)) in skeletal_meshes.iter().enumerate() {
                let is_mirror_duplicate = skeletal_meshes[..i]
                    .iter()
                    .any(|(prev_mesh, _, _)| std::ptr::eq(*prev_mesh, *mesh));
                let bone_bg = if is_mirror_duplicate {
                    &mesh.mirror_bone_bind_group
                } else {
                    &mesh.bone_bind_group
                };
                rpass.set_bind_group(1, bone_bg, &[]);
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                let mut current_tex: Option<u16> = None;
                for prim in &mesh.primitives {
                    if prim.is_transparent != want_transparent {
                        continue;
                    }
                    if let Some(range) =
                        prim.draw_range(mesh.player_visibility, is_mirror_duplicate)
                    {
                        let pkey = (prim.backface_culling, is_mirror_duplicate);
                        if active_pipeline_key != Some(pkey) {
                            rpass.set_pipeline(self.skeletal_pipeline.select_pipeline(
                                stage,
                                prim.backface_culling,
                                is_mirror_duplicate,
                            ));
                            active_pipeline_key = Some(pkey);
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
