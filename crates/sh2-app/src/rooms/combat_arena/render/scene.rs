//! 3D scene draw submission: skeletal meshes, planar mirror reflections, active GPU map selection,
//! and optional collision wireframe overlay.

use super::skinning::EnemySkinSnapshot;
use crate::state::AppState;
use sh2_core::events::stages_act1::{
    act1_eval_alltime_and_room, Act1StageId, Act1VanillaFixesConfig,
};
use sh2_ui::menu::TestRoomKind;

/// Fast FNV-1a 64-bit hash over the 2048-byte `EventFlags` bitset (`SH2-182`).
#[inline]
fn event_flags_fingerprint(flags: &sh2_core::EventFlags) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let (chunks, _) = flags.as_bytes().as_chunks::<8>();
    for chunk in chunks {
        h ^= u64::from_le_bytes(*chunk);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Syncs Act 1 `EvRoomInit` / `EvAllTimeFunc` `.map` `geo_id` display-control overrides
/// (`Act1LifecycleSnapshot::door_overrides`, e.g. `stg_ovservation.c` `0x005a1350` hiding
/// group `0x03` in `ob01.map` once `flag(24)` is set) to [`sh2_renderer::RenderContext`].
///
/// Guarded by a dirty key `(cutscene_active, stage, room_id, flags_hash)` so unchanged frames
/// skip cloning `EventFlags`, evaluating lifecycle hooks, and reallocating filter maps (`SH2-182`).
fn sync_map_geo_visibility_overrides(
    state: &mut AppState,
    active_room_name: &str,
    cutscene_active: bool,
    player_pos: glam::Vec3,
) {
    let act1_stage = if cutscene_active {
        None
    } else if state.active_test_room == TestRoomKind::ObservationDeck
        || active_room_name.eq_ignore_ascii_case("ob01")
    {
        Some(Act1StageId::Observation)
    } else {
        Act1StageId::from_raw(state.sim_core.scenario.current_stage as u8)
    };
    let room_id = sh2_renderer::GpuMap::parse_base_room_id_from_map_name(active_room_name) as u8;
    let flags_hash = if act1_stage.is_some() {
        event_flags_fingerprint(&state.sim_core.scenario.flags)
    } else {
        0
    };
    let key = (
        cutscene_active,
        act1_stage.map(|s| s as u8),
        room_id,
        flags_hash,
    );
    if state.last_map_geo_vis_key == Some(key) {
        return;
    }
    state.last_map_geo_vis_key = Some(key);

    let Some(stage) = act1_stage else {
        state.render_ctx.set_map_visibility_filter(None);
        return;
    };

    // Clone flags so render-phase lifecycle evaluation never mutates gameplay progression state.
    let mut flags_snapshot = state.sim_core.scenario.flags.clone();
    let snap = act1_eval_alltime_and_room(
        stage,
        room_id,
        [player_pos.x, player_pos.y, player_pos.z],
        0,
        state.sim_core.scenario.flashlight_on,
        state.sim_core.scenario.flashlight_on,
        Act1VanillaFixesConfig::default(),
        &mut flags_snapshot,
        &state.sim_core.scenario.puzzle_state,
    );
    state
        .render_ctx
        .set_map_geo_overrides(stage as u16, &snap.door_overrides);
}

pub(super) fn submit_scene_draw(
    state: &mut AppState,
    is_in_game: bool,
    player_pos: glam::Vec3,
    active_enemy_snapshot: Option<EnemySkinSnapshot>,
) {
    if !is_in_game {
        return;
    }

    let cutscene_active = state.drama_demo_session.as_ref().is_some_and(|s| {
        s.player.locks_gameplay_input() && s.actor_assets.iter().any(|a| a.is_some())
    });

    // Update mirror spec & map geo visibility overrides BEFORE borrowing `state` for skeletal draws.
    let (mirror_room_id, room_name_is_ob01) = {
        let active_room_name = state
            .drama_demo_session
            .as_ref()
            .filter(|s| s.player.locks_gameplay_input())
            .map(|s| s.player.info.stage_name)
            .unwrap_or(state.sim_core.scenario.current_room.as_str());
        let mirror_id = if state.active_test_room == TestRoomKind::RestroomStart
            || active_room_name.eq_ignore_ascii_case("er01")
        {
            Some(0x01)
        } else if active_room_name.eq_ignore_ascii_case("ap84")
            || active_room_name.eq_ignore_ascii_case("ap88")
        {
            Some(0x24)
        } else if active_room_name.eq_ignore_ascii_case("th09") {
            Some(0xd5)
        } else {
            None
        };
        (mirror_id, active_room_name.to_owned())
    };
    sync_map_geo_visibility_overrides(state, &room_name_is_ob01, cutscene_active, player_pos);

    let mut frame_encoder = state.frame_encoder.take();

    // Collect skeletal draw items without heap allocation on gameplay frames (stack array of 2)
    // and with pre-sized capacity only during multi-actor cutscenes (`SH2-182`).
    let mut gameplay_stack: [Option<sh2_renderer::SkeletalDrawItem<'_>>; 2] = [None, None];
    let mut gameplay_len = 0usize;
    let mut cutscene_draw: Vec<sh2_renderer::SkeletalDrawItem<'_>> = Vec::new();

    if cutscene_active {
        if let Some(ref session) = state.drama_demo_session {
            cutscene_draw.reserve(session.actor_assets.len());
            for (actor, asset_opt) in session
                .player
                .actors
                .iter()
                .zip(session.actor_assets.iter())
            {
                if !actor.visible {
                    continue;
                }
                if let Some(ref asset) = asset_opt {
                    if !asset.skin_scratch.is_empty() {
                        cutscene_draw.push((
                            &asset.gpu_mesh,
                            asset.skin_scratch.as_slice(),
                            Some(asset.texture_bind_groups.as_slice()),
                        ));
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
                gameplay_stack[gameplay_len] = Some((
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(c.texture_bind_groups.as_slice()),
                ));
                gameplay_len += 1;
            }
        }

        if let Some((enemy_id, enemy_species, _, _, _, _)) = active_enemy_snapshot {
            if let Some(ea) = state
                .enemy_asset_for_species(enemy_species, false)
                .or_else(|| state.enemy_asset(enemy_id))
            {
                if !ea.skin_scratch.is_empty() && gameplay_len < gameplay_stack.len() {
                    gameplay_stack[gameplay_len] = Some((
                        &ea.gpu_mesh,
                        ea.skin_scratch.as_slice(),
                        Some(ea.texture_bind_groups.as_slice()),
                    ));
                }
            }
        }
    }

    let gameplay_slice: [sh2_renderer::SkeletalDrawItem<'_>; 2];
    let skeletal_draw: &[sh2_renderer::SkeletalDrawItem<'_>] = if cutscene_active {
        &cutscene_draw
    } else {
        match gameplay_stack {
            [Some(a), Some(b)] => {
                gameplay_slice = [a, b];
                &gameplay_slice[..2]
            }
            [Some(a), None] => {
                gameplay_slice = [a, a];
                &gameplay_slice[..1]
            }
            _ => &[],
        }
    };

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

    let planar_mirror_spec = mirror_room_id.and_then(|room_id| {
        sh2_renderer::mirror::PlanarMirrorSpec::for_room_and_gpu_map(room_id, active_gpu_map)
    });
    state.render_ctx.set_planar_mirror(planar_mirror_spec);

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

        if let Some(ref mut encoder) = frame_encoder {
            state.render_ctx.encode_scene_with_debug_lines(
                encoder,
                Some(active_gpu_map),
                Some(active_map_tex_bgs),
                skeletal_draw,
                &debug_lines,
            );
        } else {
            state.render_ctx.render_scene_with_debug_lines(
                Some(active_gpu_map),
                Some(active_map_tex_bgs),
                skeletal_draw,
                &debug_lines,
            );
        }
    } else if let Some(ref mut encoder) = frame_encoder {
        state.render_ctx.encode_scene(
            encoder,
            Some(active_gpu_map),
            Some(active_map_tex_bgs),
            skeletal_draw,
        );
    } else {
        state.render_ctx.render_scene(
            Some(active_gpu_map),
            Some(active_map_tex_bgs),
            skeletal_draw,
        );
    }

    state.frame_encoder = frame_encoder;
    crate::rooms::sh2ee_glb_hooks::render_room_glb_replacements(state, 1.0 / 60.0);
}
