//! Skeletal animation sampling, `.cls` facial morphing, secondary cloth/hair physics, enemy
//! skinning, and planar mirror reflection matrices.

use crate::state::AppState;
use sh2_assets::formats::anm::{
    blend_frame_in_place, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into,
};
use sh2_ui::menu::TestRoomKind;

pub(super) type EnemySkinSnapshot = (
    u32,
    glam::Vec3,
    f32,
    bool,
    (Option<(u32, u32, f32)>, (u32, u32, f32), f32),
);

/// Mirror reflection plane coordinate along X in `er01.map` (`PlayerSetReverseMode` at `0x24c190`: `mirror_mode = 0`, `wall_pos = -20000.0`).
const ER01_MIRROR_WALL_X: f32 = -20000.0;

/// Reflects a 4x4 row-major skinning matrix across the vertical mirror plane `X = wall_x`.
#[inline]
fn reflect_skin_matrix_across_x(m: [[f32; 4]; 4], wall_x: f32) -> [[f32; 4]; 4] {
    let mut refl = m;
    refl[0][0] = -m[0][0];
    refl[0][1] = -m[0][1];
    refl[0][2] = -m[0][2];
    refl[0][3] = 2.0 * wall_x - m[0][3];
    refl
}

pub(super) fn evaluate_player_skinning(
    state: &mut AppState,
    alpha: f32,
    is_paused: bool,
    ticks_processed: u32,
    player_pos: glam::Vec3,
    player_yaw: f32,
) {
    let grappling_enemy_id =
        if !state.active_maria && state.active_test_room == TestRoomKind::CombatArena {
            state
                .combat_arena
                .grappling_enemy_idx
                .and_then(|idx| state.combat_arena.ai.enemies.get(idx))
                .map(|e| e.id())
        } else {
            None
        };

    let active_grapple_anm_overlay =
        if !state.active_maria && state.active_test_room == TestRoomKind::CombatArena {
            state.combat_arena.active_grapple_james_anm_overlay()
        } else {
            None
        };
    let has_apart_stair_anm = state
        .james_anm_cache
        .contains_key(&sh2_debug::JamesAnmSource::ApartStair);

    let auth_player_pos = state.sim_core.scenario.player_position;
    let auth_player_yaw = state.sim_core.scenario.player_yaw;

    let mut active_char = if state.active_maria {
        state.maria.as_mut().or(state.character.as_mut())
    } else {
        state.character.as_mut().or(state.maria.as_mut())
    };

    state.skin_matrices_scratch.clear();
    if let Some(ref mut c) = active_char {
        let (prev_cursor, cur_cursor, blend) = state
            .sim_core
            .animation
            .evaluate_blended_cursor_at_alpha(alpha);

        let active_anm = active_grapple_anm_overlay
            .and_then(sh2_debug::JamesAnmSource::from_rel_path)
            .and_then(|src| state.james_anm_cache.get(&src))
            .unwrap_or(&c.anm);

        let frame_count = active_anm.frames.len();
        if frame_count > 0 {
            let f0 = (cur_cursor.0 as usize) % frame_count;
            let f1 = (cur_cursor.1 as usize) % frame_count;
            interpolate_frames_into(
                &active_anm.frames[f0],
                &active_anm.frames[f1],
                cur_cursor.2,
                &mut c.frame_scratch_a,
            );

            if let Some(prev) = prev_cursor {
                if blend < 1.0 {
                    let pf0 = (prev.0 as usize) % frame_count;
                    let pf1 = (prev.1 as usize) % frame_count;
                    interpolate_frames_into(
                        &active_anm.frames[pf0],
                        &active_anm.frames[pf1],
                        prev.2,
                        &mut c.frame_scratch_b,
                    );
                    blend_frame_in_place(&c.frame_scratch_b, blend, &mut c.frame_scratch_a);
                }
            }

            let secondary_physics_enabled = state
                .menu_ctrl
                .options_menu
                .draft_config
                .game
                .enable_secondary_physics;
            if !is_paused && ticks_processed > 0 {
                let dt_physics = 1.0 / 60.0;
                for _ in 0..ticks_processed {
                    c.secondary_physics.step_if_enabled(
                        secondary_physics_enabled,
                        dt_physics,
                        auth_player_pos,
                        auth_player_yaw,
                    );
                }
            }
            c.dynamic_locals.copy_from_slice(&c.bind_locals);
            c.secondary_physics.apply_to_locals_if_enabled(
                secondary_physics_enabled,
                &mut c.dynamic_locals,
                alpha,
            );

            evaluate_pose_into(
                &c.model.parents,
                &c.dynamic_locals,
                &c.frame_scratch_a,
                &mut c.pose_scratch,
            );
            compute_skin_matrices_with_inv_into(
                &c.pose_scratch,
                &c.model.inv_initial_matrices,
                &mut c.skin_scratch,
            );

            let james_grapple_y_offset = if grappling_enemy_id == Some(4) {
                if has_apart_stair_anm {
                    sh2_core::combat::PS2_HUGGING_POS[1][1]
                } else {
                    -550.0
                }
            } else {
                0.0
            };
            let (s, c_yaw) = player_yaw.sin_cos();
            let world_mat = [
                [c_yaw, 0.0, s, player_pos.x],
                [0.0, 1.0, 0.0, player_pos.y + james_grapple_y_offset],
                [-s, 0.0, c_yaw, player_pos.z],
                [0.0, 0.0, 0.0, 1.0],
            ];

            state.skin_matrices_scratch.reserve(c.skin_scratch.len());
            state.mirror_skin_matrices_scratch.clear();
            let build_mirror = state.active_test_room == TestRoomKind::RestroomStart
                || state
                    .sim_core
                    .scenario
                    .current_room
                    .eq_ignore_ascii_case("er01");
            if build_mirror {
                state
                    .mirror_skin_matrices_scratch
                    .reserve(c.skin_scratch.len());
            }
            for m_skin in &c.skin_scratch {
                let mut combined = [[0.0; 4]; 4];
                for r in 0..4 {
                    for col in 0..4 {
                        combined[r][col] = world_mat[r][0] * m_skin[0][col]
                            + world_mat[r][1] * m_skin[1][col]
                            + world_mat[r][2] * m_skin[2][col]
                            + world_mat[r][3] * m_skin[3][col];
                    }
                }
                state.skin_matrices_scratch.push(combined);
                if build_mirror {
                    state
                        .mirror_skin_matrices_scratch
                        .push(reflect_skin_matrix_across_x(combined, ER01_MIRROR_WALL_X));
                }
            }
        }
    }
}

pub(super) fn evaluate_cutscene_actors_skinning(state: &mut AppState) {
    let Some(ref mut session) = state.drama_demo_session else {
        return;
    };
    if !session.player.locks_gameplay_input() {
        return;
    }
    let build_mirror = state.active_test_room == TestRoomKind::RestroomStart
        || session.player.info.stage_name == "er01";

    let has_flashlight = state.sim_core.scenario.has_flashlight();
    let scene_frame = session.player.scene_frame.max(0.0);
    for (idx, (actor, asset_opt)) in session
        .player
        .actors
        .iter()
        .zip(session.actor_assets.iter_mut())
        .enumerate()
    {
        let Some(ref mut asset) = asset_opt else {
            continue;
        };
        if !actor.visible {
            continue;
        }
        // `DemoJamesCheckModelParts` (`SLUS_202.28` `0x00104514`): hide pocket flashlight (`0x15` = 21)
        // on cutscene James when the player does not possess the flashlight item yet.
        if actor.actor_name.to_ascii_lowercase().contains("jms") {
            asset.gpu_mesh.set_pose_visible(0x15, has_flashlight);
        }

        // 1. Evaluate `.cls` facial morph cluster weights & upload morphed vertices if present (`clani.c`)
        let cls_frame = actor.global_cls_frame(scene_frame);
        if let Some(ref cls) = asset.cls {
            cls.evaluate_global_frame_into(cls_frame, false, &mut asset.cluster_weights_scratch);
            asset.model.apply_cluster_weights_into(
                &asset.cluster_weights_scratch,
                &mut asset.morphed_positions_scratch,
            );
            asset.gpu_mesh.update_morphed_positions(
                &state.render_ctx.queue,
                &asset.model,
                &asset.morphed_positions_scratch,
                &mut asset.gpu_vertex_scratch,
            );
        }

        // 2. Evaluate `.anm` skeletal bone transforms at `actor_frame` (`scene_frame + anm_frame_offset`),
        // bridging sparse 10 Hz / 15 Hz hold frames (`t == 0`) via `sample_span_at` so motion plays
        // smoothly at full 30/60+ FPS instead of stepping 1 frame and freezing for 2 hold frames.
        if asset.anm.frames.is_empty() {
            continue;
        }
        let actor_frame = actor.global_anm_frame(scene_frame);
        let (f0, f1, frac) = asset.anm.sample_span_at(actor_frame);

        interpolate_frames_into(
            &asset.anm.frames[f0],
            &asset.anm.frames[f1],
            frac,
            &mut asset.frame_scratch_a,
        );
        evaluate_pose_into(
            &asset.model.parents,
            &asset.bind_locals,
            &asset.frame_scratch_a,
            &mut asset.pose_scratch,
        );
        compute_skin_matrices_with_inv_into(
            &asset.pose_scratch,
            &asset.model.inv_initial_matrices,
            &mut asset.skin_scratch,
        );

        let world_mat = actor.world_matrix_row_major();
        if let Some(mirror_buf) = session.actor_mirror_skin_scratch.get_mut(idx) {
            mirror_buf.clear();
            if build_mirror {
                mirror_buf.reserve(asset.skin_scratch.len());
            }
        }
        for m_skin in &mut asset.skin_scratch {
            let orig = *m_skin;
            for r in 0..4 {
                for col in 0..4 {
                    m_skin[r][col] = world_mat[r][0] * orig[0][col]
                        + world_mat[r][1] * orig[1][col]
                        + world_mat[r][2] * orig[2][col]
                        + world_mat[r][3] * orig[3][col];
                }
            }
            if build_mirror {
                if let Some(mirror_buf) = session.actor_mirror_skin_scratch.get_mut(idx) {
                    mirror_buf.push(reflect_skin_matrix_across_x(*m_skin, ER01_MIRROR_WALL_X));
                }
            }
        }
    }
}

pub(super) fn evaluate_enemy_skinning(
    state: &mut AppState,
    alpha: f32,
) -> Option<EnemySkinSnapshot> {
    let active_enemy_snapshot = if state.active_test_room == TestRoomKind::CombatArena
        && !state.combat_arena.ai.enemies.is_empty()
    {
        let active_idx = state.combat_enemy_idx % state.combat_arena.ai.enemies.len();
        let enemy = &state.combat_arena.ai.enemies[active_idx];
        let (enemy_pos, enemy_yaw) = enemy.interpolated_pose(alpha);
        Some((
            enemy.id(),
            enemy_pos,
            enemy_yaw,
            enemy.is_grappling_player,
            enemy.anim().evaluate_blended_cursor_at_alpha(alpha),
        ))
    } else {
        None
    };

    if let Some((enemy_id, enemy_pos, enemy_yaw, is_grappling, (prev_cursor, cur_cursor, blend))) =
        active_enemy_snapshot
    {
        if let Some(ea) = state.enemy_asset_mut(enemy_id) {
            let frame_count = ea.anm.frames.len();
            if frame_count > 0 {
                let f0 = (cur_cursor.0 as usize) % frame_count;
                let f1 = (cur_cursor.1 as usize) % frame_count;
                let alpha = cur_cursor.2;

                interpolate_frames_into(
                    &ea.anm.frames[f0],
                    &ea.anm.frames[f1],
                    alpha,
                    &mut ea.frame_scratch_a,
                );

                if let Some(prev) = prev_cursor {
                    if blend < 1.0 {
                        let pf0 = (prev.0 as usize) % frame_count;
                        let pf1 = (prev.1 as usize) % frame_count;
                        interpolate_frames_into(
                            &ea.anm.frames[pf0],
                            &ea.anm.frames[pf1],
                            prev.2,
                            &mut ea.frame_scratch_b,
                        );
                        blend_frame_in_place(&ea.frame_scratch_b, blend, &mut ea.frame_scratch_a);
                    }
                }

                evaluate_pose_into(
                    &ea.model.parents,
                    &ea.bind_locals,
                    &ea.frame_scratch_a,
                    &mut ea.pose_scratch,
                );
                compute_skin_matrices_with_inv_into(
                    &ea.pose_scratch,
                    &ea.model.inv_initial_matrices,
                    &mut ea.skin_scratch,
                );

                let (es, ec_yaw) = enemy_yaw.sin_cos();
                let y_offset = sh2_core::combat::BestiaryId::from_raw(enemy_id)
                    .map(|species| {
                        let anchor = sh2_core::combat::GrappleAnchor::for_species(species);
                        if is_grappling {
                            anchor.vertical_offset
                        } else {
                            anchor.default_height_offset
                        }
                    })
                    .unwrap_or(0.0);
                let ey = enemy_pos.y + y_offset;
                let world_mat = [
                    [ec_yaw, 0.0, es, enemy_pos.x],
                    [0.0, 1.0, 0.0, ey],
                    [-es, 0.0, ec_yaw, enemy_pos.z],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                for m_skin in &mut ea.skin_scratch {
                    let orig = *m_skin;
                    for r in 0..4 {
                        for col in 0..4 {
                            m_skin[r][col] = world_mat[r][0] * orig[0][col]
                                + world_mat[r][1] * orig[1][col]
                                + world_mat[r][2] * orig[2][col]
                                + world_mat[r][3] * orig[3][col];
                        }
                    }
                }
            }
        }
    }

    active_enemy_snapshot
}
