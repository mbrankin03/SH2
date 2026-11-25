//! 3D scene draw submission: skeletal meshes, planar mirror reflections, active GPU map selection,
//! and optional collision wireframe overlay.

use super::skinning::EnemySkinSnapshot;
use crate::state::AppState;
use sh2_ui::menu::TestRoomKind;

pub(super) fn submit_scene_draw(
    state: &mut AppState,
    is_in_game: bool,
    player_pos: glam::Vec3,
    active_enemy_snapshot: Option<EnemySkinSnapshot>,
) {
    let mut skeletal_draw = Vec::new();
    let active_room_name = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
        .map(|s| s.player.info.stage_name)
        .unwrap_or(state.sim_core.scenario.current_room.as_str());
    let planar_mirror_spec = if state.active_test_room == TestRoomKind::RestroomStart
        || active_room_name.eq_ignore_ascii_case("er01")
    {
        sh2_renderer::mirror::PlanarMirrorSpec::for_room(0x01)
    } else if active_room_name.eq_ignore_ascii_case("ap84")
        || active_room_name.eq_ignore_ascii_case("ap88")
    {
        sh2_renderer::mirror::PlanarMirrorSpec::for_room(0x24)
    } else {
        None
    };
    state.render_ctx.set_planar_mirror(planar_mirror_spec);
    let is_restroom_mirror = false;
    let cutscene_active = state.drama_demo_session.as_ref().is_some_and(|s| {
        s.player.locks_gameplay_input() && s.actor_assets.iter().any(|a| a.is_some())
    });

    if cutscene_active {
        if let Some(ref session) = state.drama_demo_session {
            for (idx, (actor, asset_opt)) in session
                .player
                .actors
                .iter()
                .zip(session.actor_assets.iter())
                .enumerate()
            {
                if !actor.visible {
                    continue;
                }
                if let Some(ref asset) = asset_opt {
                    if !asset.skin_scratch.is_empty() {
                        skeletal_draw.push((
                            &asset.gpu_mesh,
                            asset.skin_scratch.as_slice(),
                            Some(asset.texture_bind_groups.as_slice()),
                        ));
                        if is_restroom_mirror {
                            if let Some(mirror_buf) = session.actor_mirror_skin_scratch.get(idx) {
                                if !mirror_buf.is_empty() {
                                    skeletal_draw.push((
                                        &asset.gpu_mesh,
                                        mirror_buf.as_slice(),
                                        Some(asset.texture_bind_groups.as_slice()),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        let active_char_ref = if state.active_maria {
            state.maria.as_ref().or(state.character.as_ref())
        } else {
            state.character.as_ref().or(state.maria.as_ref())
        };
        if let Some(c) = active_char_ref {
            if !state.skin_matrices_scratch.is_empty() {
                skeletal_draw.push((
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(c.texture_bind_groups.as_slice()),
                ));
                if is_restroom_mirror && !state.mirror_skin_matrices_scratch.is_empty() {
                    skeletal_draw.push((
                        &c.gpu_mesh,
                        state.mirror_skin_matrices_scratch.as_slice(),
                        Some(c.texture_bind_groups.as_slice()),
                    ));
                }
            }
        }

        if let Some((enemy_id, enemy_species, _, _, _, _)) = active_enemy_snapshot {
            if let Some(ea) = state
                .enemy_asset_for_species(enemy_species, false)
                .or_else(|| state.enemy_asset(enemy_id))
            {
                if !ea.skin_scratch.is_empty() {
                    skeletal_draw.push((
                        &ea.gpu_mesh,
                        ea.skin_scratch.as_slice(),
                        Some(ea.texture_bind_groups.as_slice()),
                    ));
                }
            }
        }
    }

    if !is_in_game {
        return;
    }

    let (active_gpu_map, active_map_tex_bgs) = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
        .and_then(|s| {
            s.stage_gpu_map
                .as_deref()
                .zip(s.stage_texture_bind_groups.as_ref())
        })
        .unwrap_or((state.gpu_map.as_ref(), &state.map_texture_bind_groups));

    if state.debug_collision_overlay {
        let floors_iter = state.sim_core.collision.floors.iter().map(|f| f.v);
        let walls_iter = state
            .sim_core
            .collision
            .all_walls()
            .map(|w| (w.start, w.end, w.y_top, w.y_bottom));

        let mut debug_lines = Vec::with_capacity(
            state.sim_core.collision.floors.len() * 6
                + state.sim_core.collision.walls.len() * 8
                + 72,
        );

        sh2_debug::build_collision_debug_lines_iter(
            floors_iter,
            walls_iter,
            [0.0, 1.0, 0.8, 1.0],
            [1.0, 0.2, 0.2, 1.0],
            &mut debug_lines,
        );

        let chara_lines = sh2_debug::build_chara_collision_debug_lines(
            player_pos.into(),
            state.sim_core.collision_config.radius,
            state.sim_core.collision_config.height,
            16,
            [1.0, 0.9, 0.1, 1.0],
        );
        debug_lines.extend(chara_lines);

        if state.active_test_room == TestRoomKind::CombatArena
            && !state.combat_arena.ai.enemies.is_empty()
        {
            let active_idx = state.combat_enemy_idx % state.combat_arena.ai.enemies.len();
            let e = &state.combat_arena.ai.enemies[active_idx];
            let sz = e.size();
            let pos = e.position();
            let enemy_lines = sh2_debug::build_chara_collision_debug_lines(
                [pos.x, pos.y, pos.z],
                sz.radius,
                sz.height * 2.0,
                16,
                [1.0, 0.2, 0.8, 1.0],
            );
            debug_lines.extend(enemy_lines);
        }

        state.render_ctx.render_scene_with_debug_lines(
            Some(active_gpu_map),
            Some(active_map_tex_bgs),
            &skeletal_draw,
            &debug_lines,
        );
    } else {
        state.render_ctx.render_scene(
            Some(active_gpu_map),
            Some(active_map_tex_bgs),
            &skeletal_draw,
        );
    }
    crate::rooms::sh2ee_glb_hooks::render_room_glb_replacements(state, 1.0 / 60.0);
}
