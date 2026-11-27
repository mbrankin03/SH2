//! Main-pass skeletal mesh draw helper for [`RenderContext`].

use crate::light::{
    CharacterLightPool, CharacterLightType, SpotlightUniforms, CHARACTER_POOL_LINEAR_SCALE,
    MODEL3_LIGHT_ENV_RADIUS_MM,
};
use crate::skeletal::SkeletalPassStage;
use crate::{RenderContext, SkeletalDrawItem};

/// Evaluates `Model3UpdateLightEnv` (`SLUS_202.28` `0x00128e00`) for a single skeletal character
/// against `base_pool`, returning per-character [`SpotlightUniforms`] without double-counting
/// parallel slot 0 (`skeletal/shader.rs:202-210` already applies `spotlight.color_outer` for
/// slot 0; auxiliary parallel slots `1..p_count` match `camera_wiring.rs:410` and `cutscene.rs:202`).
pub(crate) fn apply_character_light_pool_to_uniforms(
    base_uniforms: SpotlightUniforms,
    base_pool: &CharacterLightPool,
    char_pos: glam::Vec3,
    is_night: bool,
    is_cutscene: bool,
) -> SpotlightUniforms {
    let mut pool = base_pool.clone();
    pool.update_fake_lights(char_pos);
    pool.update_parallels();
    pool.update_extras_with_context(
        MODEL3_LIGHT_ENV_RADIUS_MM,
        char_pos,
        is_night,
        is_cutscene,
        false,
    );

    let mut u = base_uniforms;
    // Reset all parallel slots so stale auxiliary slots never leak when `p_count` shrinks,
    // and keep `parallel_dirs[0][3] == 0.0` so slot 0 is never double-counted with `color_outer`.
    u.parallel_dirs = [[0.0, 0.0, 1.0, 0.0]; 3];
    u.parallel_colors = [[0.0; 4]; 3];
    if let (Some(nlm), Some(lcm)) = (pool.nth_parallel_nlm(0), pool.nth_parallel_lcm(0)) {
        let nlm_cols = nlm.to_cols_array_2d();
        let lcm_cols = lcm.to_cols_array_2d();
        let p_count = pool.valid_parallel_count().min(3);
        for slot in 1..p_count {
            u.parallel_dirs[slot] = [nlm_cols[0][slot], nlm_cols[1][slot], nlm_cols[2][slot], 1.0];
            u.parallel_colors[slot] = [
                lcm_cols[slot][0] * CHARACTER_POOL_LINEAR_SCALE,
                lcm_cols[slot][1] * CHARACTER_POOL_LINEAR_SCALE,
                lcm_cols[slot][2] * CHARACTER_POOL_LINEAR_SCALE,
                0.0,
            ];
        }
    }
    // Apply top-3 per-character point lights (`UpdateExtras` `0x00127360`).
    u.point_positions = [[0.0; 4]; 3];
    u.point_colors = [[0.0; 4]; 3];
    let mut pt_out = 0usize;
    for ei in 0..pool.valid_extra_count() {
        if pt_out >= 3 {
            break;
        }
        if let Some(entry) = pool.nth_valid_extra(ei) {
            if entry.kind == CharacterLightType::Point {
                u.point_positions[pt_out] = [
                    entry.world_pos.x,
                    entry.world_pos.y,
                    entry.world_pos.z,
                    entry.falloff_near_mm,
                ];
                u.point_colors[pt_out] = [
                    entry.color.x * CHARACTER_POOL_LINEAR_SCALE,
                    entry.color.y * CHARACTER_POOL_LINEAR_SCALE,
                    entry.color.z * CHARACTER_POOL_LINEAR_SCALE,
                    entry.falloff_far_mm,
                ];
                pt_out += 1;
            }
        }
    }
    // Apply `UpdateExtras` night reflection dimming (`0.2` when no extra lights active).
    let refl = pool.reflection_color();
    u.reflection_color[0] = refl.x;
    u.reflection_color[1] = refl.y;
    u.reflection_color[2] = refl.z;
    u
}

impl RenderContext {
    /// Evaluates `Model3UpdateLightEnv` (`SLUS_202.28` `0x00128e00`) per skeletal character using
    /// the active gameplay [`crate::light::CharacterLightPool`] (`UpdateFake_DirandColor`
    /// `0x0012f450`, `UpdateParallels` `0x00126d30`, `UpdateExtras` `0x00127360`), populating
    /// `self.skeletal_light_buffers[i]` prior to the main render pass.
    pub(super) fn prepare_skeletal_light_slots(&self, skeletal_meshes: &[SkeletalDrawItem<'_>]) {
        let pool_snapshot = self
            .gameplay_char_light_pool
            .lock()
            .ok()
            .and_then(|g| g.clone());
        let Some((base_pool, is_night, is_cutscene)) = pool_snapshot else {
            self.skeletal_light_active_slots
                .store(0, std::sync::atomic::Ordering::Relaxed);
            return;
        };
        let base_uniforms = self
            .last_spotlight_uniforms
            .lock()
            .map(|g| *g)
            .unwrap_or_default();

        let count = skeletal_meshes.len().min(Self::MAX_SKELETAL_LIGHT_SLOTS);
        for (i, (_mesh, matrices, _)) in skeletal_meshes.iter().take(count).enumerate() {
            // Root bone skinning matrix translation in PS2 world mm (row-major `[row][col]`).
            let char_pos = matrices
                .first()
                .map(|m| glam::Vec3::new(m[0][3], m[1][3], m[2][3]))
                .unwrap_or(glam::Vec3::ZERO);

            let u = apply_character_light_pool_to_uniforms(
                base_uniforms,
                &base_pool,
                char_pos,
                is_night,
                is_cutscene,
            );

            self.queue
                .write_buffer(&self.skeletal_light_buffers[i], 0, bytemuck::bytes_of(&u));
        }
        self.skeletal_light_active_slots
            .store(count, std::sync::atomic::Ordering::Relaxed);
    }

    pub(super) fn draw_skeletal_meshes<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        skeletal_meshes: &'a [SkeletalDrawItem<'a>],
    ) {
        self.draw_skeletal_meshes_with_camera(
            rpass,
            skeletal_meshes,
            &self.geometry_bind_group,
            false,
            true,
        );
    }

    /// Draws skeletal meshes into a `Depth32Float` render pass using `common_bind_group` at
    /// bind group 0. When `invert_winding` is `true` (e.g. negative-determinant
    /// `mirrored_reflection_view_proj` in `water_pass.rs`), front/back culling is flipped
    /// (`CullFront` <-> `CullBack`) so reflected characters are not inside-out.
    pub(super) fn draw_skeletal_meshes_with_camera<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        skeletal_meshes: &'a [SkeletalDrawItem<'a>],
        common_bind_group: &'a wgpu::BindGroup,
        invert_winding: bool,
        use_per_char_light_slots: bool,
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
                        && p.draw_range(
                            mesh.player_visibility,
                            is_mirror_duplicate || invert_winding,
                        )
                        .is_some()
                })
            });
            if !has_any {
                continue;
            }

            let active_light_slots = if use_per_char_light_slots {
                self.skeletal_light_active_slots
                    .load(std::sync::atomic::Ordering::Relaxed)
            } else {
                0
            };
            let mut active_pipeline_key: Option<(bool, bool)> = None;
            rpass.set_bind_group(0, common_bind_group, &[]);

            for (i, (mesh, _matrices, texture_groups)) in skeletal_meshes.iter().enumerate() {
                if i < active_light_slots {
                    rpass.set_bind_group(0, &self.skeletal_light_bind_groups[i], &[]);
                } else {
                    rpass.set_bind_group(0, common_bind_group, &[]);
                }
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
                    if let Some(range) = prim.draw_range(
                        mesh.player_visibility,
                        is_mirror_duplicate || invert_winding,
                    ) {
                        let use_flipped_cull = is_mirror_duplicate ^ invert_winding;
                        let pkey = (prim.backface_culling, use_flipped_cull);
                        if active_pipeline_key != Some(pkey) {
                            rpass.set_pipeline(self.skeletal_pipeline.select_pipeline(
                                stage,
                                prim.backface_culling,
                                use_flipped_cull,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_light_pool_skips_parallel_slot_0_and_clears_stale_slots() {
        // Seed stale auxiliary parallel slots 0..3 to verify they are cleared.
        let base = SpotlightUniforms {
            parallel_dirs: [
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
            ],
            parallel_colors: [[0.5; 4]; 3],
            ..Default::default()
        };

        // Pool with 2 parallel lights: slot 0 stays disabled (`w == 0.0`, already in `color_outer`),
        // slot 1 is populated (`w == 1.0`), and stale slot 2 is cleared (`w == 0.0`).
        let mut pool = CharacterLightPool::new();
        pool.set_parallel(0, glam::Vec3::Y, glam::Vec3::new(0.8, 0.8, 0.8));
        pool.set_parallel(1, glam::Vec3::X, glam::Vec3::new(0.4, 0.5, 0.6));
        let u2 =
            apply_character_light_pool_to_uniforms(base, &pool, glam::Vec3::ZERO, false, false);
        assert_eq!(u2.parallel_dirs[0][3], 0.0);
        assert_eq!(u2.parallel_colors[0], [0.0; 4]);
        assert_eq!(u2.parallel_dirs[1][3], 1.0);
        assert!((u2.parallel_colors[1][0] - 0.4 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-6);
        assert_eq!(u2.parallel_dirs[2][3], 0.0);
        assert_eq!(u2.parallel_colors[2], [0.0; 4]);

        // When pool shrinks to 1 parallel light, auxiliary slot 1 is also cleared.
        let mut pool1 = CharacterLightPool::new();
        pool1.set_parallel(0, glam::Vec3::Y, glam::Vec3::ONE);
        let u1 = apply_character_light_pool_to_uniforms(u2, &pool1, glam::Vec3::ZERO, false, false);
        assert_eq!(u1.parallel_dirs[0][3], 0.0);
        assert_eq!(u1.parallel_dirs[1][3], 0.0);
        assert_eq!(u1.parallel_dirs[2][3], 0.0);
        assert_eq!(u1.parallel_colors, [[0.0; 4]; 3]);
    }
}
