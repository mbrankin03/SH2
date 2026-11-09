//! 60 Hz simulation step, skeletal skinning, cutscene camera/lighting overrides, and 3D scene rendering
//! for 3D Stage Exploration & Combat Arena (`SH2-13` / `SH2-57` / `SH2-63`).

use std::time::Duration;

use crate::assets::{load_cutscene_voice_wav, open_drama_demo_session};
use crate::state::AppState;
use sh2_assets::formats::anm::{
    blend_frame_in_place, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into,
};
use sh2_assets::formats::demo::{DemoLightKind, DemoLightState};
use sh2_core::{CutsceneCameraSnapshot, CutsceneSequenceStep, DRAMA_DEMO_CATALOG};
use sh2_ui::menu::TestRoomKind;

type EnemySkinSnapshot = (
    u32,
    glam::Vec3,
    f32,
    bool,
    (Option<(u32, u32, f32)>, (u32, u32, f32), f32),
);

pub(super) fn tick_and_render_combat_arena_3d(
    state: &mut AppState,
    dt: Duration,
    aspect: f32,
    is_paused: bool,
    is_in_game: bool,
) {
    // 1. Advance 60 Hz simulation, combat state machine, and streaming BGM
    let ticks_processed = if !is_paused {
        step_simulation_and_audio(state, dt, is_in_game)
    } else {
        0
    };

    // 2. Advance active in-engine 3D cutscene (`DramaDemoPlayer`, `SH2-63`) or sub-frame gameplay camera
    let alpha = state.sim_core.interpolation_alpha();
    let (cutscene_camera_override, cutscene_light_override) =
        step_cutscene_player(state, dt, is_paused);

    update_camera_uniforms(state, aspect, alpha, cutscene_camera_override);

    // 3. Dynamic Flashlight or `.dds` Cutscene Scene Light (`DdsPlayLight`)
    let (player_pos, player_yaw) = state.sim_core.interpolated_player_pose(alpha);
    update_spotlight_uniforms(state, alpha, player_pos, cutscene_light_override);

    // 4. Sample player, cutscene actor, & enemy skeletal animations at sub-frame alpha
    evaluate_player_skinning(
        state,
        alpha,
        is_paused,
        ticks_processed,
        player_pos,
        player_yaw,
    );
    evaluate_cutscene_actors_skinning(state);
    let active_enemy_snapshot = evaluate_enemy_skinning(state, alpha);

    // 5. Submit 3D map, skeletal meshes (plus planar mirror reflections in RestroomStart), and optional collision wireframe overlay
    submit_scene_draw(state, is_in_game, player_pos, active_enemy_snapshot);
}

fn step_simulation_and_audio(state: &mut AppState, dt: Duration, is_in_game: bool) -> u32 {
    if state.active_test_room == TestRoomKind::CombatArena
        && state.combat_arena.player.mode != sh2_core::combat::CombatMode::Free
    {
        state.sim_core.input.state.left_stick_x = 0.0;
        state.sim_core.input.state.left_stick_y = 0.0;
    }
    let ticks_processed = state.sim_core.update(dt);

    if ticks_processed > 0 {
        update_listener_acoustics_and_footsteps(state);
    }

    if state.active_test_room == TestRoomKind::CombatArena && ticks_processed > 0 {
        step_combat_arena_ticks(state, ticks_processed);
        sync_combat_player_animation_and_sfx(state);
    }

    if is_in_game {
        step_bgm_manager(state, dt);
    }

    ticks_processed
}

fn update_listener_acoustics_and_footsteps(state: &mut AppState) {
    let p_pos = state.sim_core.scenario.player_position;
    let p_yaw = state.sim_core.scenario.player_yaw;
    let fwd = glam::Vec3::new(p_yaw.sin(), 0.0, p_yaw.cos());
    let is_interior_map = !matches!(
        state.active_test_room,
        TestRoomKind::ObservationDeck | TestRoomKind::SurfaceSfxTestRoom
    );
    let active_cam_record = state.sim_core.camera.cam_file.as_ref().and_then(|cam| {
        state
            .sim_core
            .camera
            .active_road_index
            .and_then(|idx| cam.records.get(idx))
            .or_else(|| cam.find_active_record([p_pos.x, p_pos.y, p_pos.z], 50.0))
    });
    if let Some(ref mut eng) = state.audio_engine {
        eng.set_listener_transform(p_pos, fwd);
        if active_cam_record.is_some() {
            eng.update_room_acoustics_from_record(active_cam_record, is_interior_map);
        }
    }
    if state.sim_core.footstep_triggered_since_last_poll {
        state.sim_core.footstep_triggered_since_last_poll = false;
        if let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_mut()) {
            let _ = bank.play_footstep(
                eng,
                state.active_maria,
                state.sim_core.active_floor_material,
                state.sim_core.last_locomotion.state.is_running(),
                state.sim_core.last_locomotion.footstep_volume,
            );
        }
    }
}

fn step_combat_arena_ticks(state: &mut AppState, ticks_processed: u32) {
    let tick_dt = 1.0 / 60.0;
    let mut atk_pressed = state.key_attack_pressed;
    state.key_attack_pressed = false;
    let mut struggle_presses = state.struggle_button_press_count;
    state.struggle_button_press_count = 0;

    for _ in 0..ticks_processed {
        state.combat_arena.player_position = state.sim_core.scenario.player_position;
        state.combat_arena.player_yaw = state.sim_core.scenario.player_yaw;

        if struggle_presses > 0 {
            if state.combat_arena.grapple_session.is_some() {
                state
                    .combat_arena
                    .register_grapple_struggle_input(struggle_presses);
            } else if state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held {
                state
                    .combat_arena
                    .player
                    .process_struggle_inputs(struggle_presses, tick_dt, false);
            }
            struggle_presses = 0;
        }

        state.combat_arena.tick_isolated(
            state.key_ready_held,
            atk_pressed,
            state.key_attack_held,
            tick_dt,
            state.combat_enemy_idx,
        );
        atk_pressed = false;

        state.sim_core.scenario.player_position = state.combat_arena.player_position;
        if state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held {
            state.sim_core.scenario.player_yaw = state.combat_arena.player_yaw;
        }
    }
}

fn sync_combat_player_animation_and_sfx(state: &mut AppState) {
    match state.combat_arena.player.mode {
        sh2_core::combat::CombatMode::Held => {
            state.sim_core.animation_locked = true;
            let stage_anm_available = match state.combat_arena.active_grapple_james_anm_overlay() {
                None => true,
                Some(rel) => sh2_debug::JamesAnmSource::from_rel_path(rel)
                    .map(|src| state.james_anm_cache.contains_key(&src))
                    .unwrap_or(false),
            };
            let struggle_clip = if stage_anm_available {
                state
                    .combat_arena
                    .active_grapple_player_clip()
                    .unwrap_or(sh2_core::animation::JamesBasicClips::STRUGGLE_TORSO)
            } else {
                state
                    .combat_arena
                    .grappling_enemy_idx
                    .and_then(|idx| state.combat_arena.enemies.get(idx))
                    .and_then(|e| sh2_core::combat::BestiaryId::from_raw(e.id))
                    .map(|sp| sh2_core::combat::GrappleAnchor::for_species(sp).struggle_clip)
                    .unwrap_or(sh2_core::animation::JamesBasicClips::STRUGGLE_TORSO)
            };
            if state.sim_core.animation.current_clip().start_frame != struggle_clip.start_frame {
                tracing::info!(
                    "[Combat Grapple] James held by Enemy #{}: transitioning to clip frames {}..={} @ {:.0}fps",
                    state
                        .combat_arena
                        .grappling_enemy_idx
                        .and_then(|idx| state.combat_arena.enemies.get(idx).map(|e| e.id))
                        .unwrap_or(0),
                    struggle_clip.start_frame,
                    struggle_clip.end_frame,
                    struggle_clip.fps
                );
            }
            state
                .sim_core
                .animation
                .transition_to(sh2_core::animation::AnimationStateKind::Held, struggle_clip);
        }
        sh2_core::combat::CombatMode::Hurt => {
            state.sim_core.animation_locked = true;
            state.sim_core.animation.transition_to(
                sh2_core::animation::AnimationStateKind::Damage,
                sh2_core::animation::JamesBasicClips::HURT,
            );
        }
        sh2_core::combat::CombatMode::Dying | sh2_core::combat::CombatMode::Dead => {
            state.sim_core.animation_locked = true;
            state.sim_core.animation.transition_to(
                sh2_core::animation::AnimationStateKind::Fall,
                sh2_core::animation::JamesBasicClips::FALL,
            );
        }
        sh2_core::combat::CombatMode::Ready | sh2_core::combat::CombatMode::Raise => {
            state.sim_core.animation_locked = true;
            state.sim_core.animation.transition_to(
                sh2_core::animation::AnimationStateKind::Aim,
                sh2_core::animation::JamesBasicClips::AIM,
            );
        }
        sh2_core::combat::CombatMode::Swing
        | sh2_core::combat::CombatMode::Shot
        | sh2_core::combat::CombatMode::Recover
        | sh2_core::combat::CombatMode::Stomp => {
            state.sim_core.animation_locked = true;
            state.sim_core.animation.transition_to(
                sh2_core::animation::AnimationStateKind::Attack,
                sh2_core::animation::JamesBasicClips::ATTACK,
            );
        }
        sh2_core::combat::CombatMode::Free | sh2_core::combat::CombatMode::Lower => {
            state.sim_core.animation_locked = false;
        }
        _ => {}
    }

    if let Some(sfx_id) = state.combat_arena.player.pending_sfx.take() {
        if let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_ref()) {
            let _ = bank.play_se_id(eng, sfx_id, 0.95);
        }
        tracing::info!("[Combat SFX] Triggered sound effect ID: {}", sfx_id);
    }
}

fn step_bgm_manager(state: &mut AppState, dt: Duration) {
    let (bgm_stage_id, bgm_flags) = state.active_bgm_stage_and_flags();
    state.bgm_manager.event_flags = bgm_flags;
    let (hp_ratio, min_enemy_dist_xz) = if state.active_test_room == TestRoomKind::CombatArena {
        let p_hp = (state.combat_arena.player.health
            / state.combat_arena.player.max_health.max(1.0))
        .clamp(0.0, 1.0);
        let p_pos = state.sim_core.scenario.player_position;
        let min_dist = state
            .combat_arena
            .enemies
            .iter()
            .filter(|e| !e.is_dead && e.is_aggroed)
            .map(|e| {
                let dx = e.position.x - p_pos.x;
                let dz = e.position.z - p_pos.z;
                (dx * dx + dz * dz).sqrt()
            })
            .fold(None::<f32>, |acc, d| Some(acc.map_or(d, |a| a.min(d))));
        (p_hp, min_dist)
    } else {
        (1.0, None)
    };
    if let Err(e) = state.bgm_manager.tick(
        bgm_stage_id,
        state.sim_core.scenario.player_position,
        hp_ratio,
        min_enemy_dist_xz,
        dt.as_secs_f32(),
        state.audio_engine.as_mut(),
    ) {
        tracing::warn!("[BGM] SeBgmManager tick warning: {:?}", e);
    }
}

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

fn step_cutscene_player(
    state: &mut AppState,
    dt: Duration,
    is_paused: bool,
) -> (Option<CutsceneCameraSnapshot>, Option<DemoLightState>) {
    let mut cutscene_camera_override = None;
    let mut cutscene_light_override = None;

    if !is_paused {
        let subs_enabled = state.menu_ctrl.options_menu.draft_config.game.subtitles;
        let voice_pos_secs = state
            .audio_engine
            .as_ref()
            .and_then(|eng| eng.voice_playback_position_secs());
        let mut voice_afs_to_play = None;
        let mut stop_voice_now = false;
        let mut finished_handoff = None;

        if let Some(ref mut session) = state.drama_demo_session {
            session
                .player
                .tick_with_voice_sync(dt.as_secs_f32(), subs_enabled, voice_pos_secs);
            if session.player.pending_voice_trigger {
                session.player.pending_voice_trigger = false;
                voice_afs_to_play = session.player.info.voice_afs_index();
            }
            if session.player.pending_voice_stop {
                session.player.pending_voice_stop = false;
                stop_voice_now = true;
            }
            if let Some((actor_pos, actor_yaw)) = session.player.player_handoff_transform() {
                state.sim_core.snap_player_transform(actor_pos, actor_yaw);
            }
            if !session.player.locks_gameplay_input() {
                finished_handoff = session.player.player_handoff_transform();
            } else {
                cutscene_camera_override = Some(session.player.camera);
                cutscene_light_override = session.player.primary_active_light().copied();
            }
        }

        if let Some(afs_idx) = voice_afs_to_play {
            if let Some(wav) = load_cutscene_voice_wav(afs_idx) {
                if let Some(ref mut eng) = state.audio_engine {
                    let _ = eng.play_cutscene_voice(&wav);
                }
            }
        }
        if stop_voice_now {
            if let Some(ref mut eng) = state.audio_engine {
                eng.stop_voice(Duration::from_millis(50));
            }
        }
        if let Some((final_pos, final_yaw)) = finished_handoff {
            state.sim_core.snap_player_transform(final_pos, final_yaw);
            state.sim_core.camera.reset_to_player(final_pos, final_yaw);
            // If this cutscene is part of a multi-segment `.dds` sequence chain (e.g. `papa_agl_a` -> `papa_agl`,
            // `piano_lau_b` -> `piano_lau_e`, `bp_edi_1` -> `bp_edi_2`), automatically chain into the next in-engine step!
            let next_catalog_idx = state.drama_demo_session.as_ref().and_then(|s| {
                let cur_no = s.player.info.demo_no;
                let chain = s.player.info.sequence_chain;
                let pos_in_chain = chain.iter().position(|step| {
                    matches!(step, CutsceneSequenceStep::InEngineDemo { demo_no } if *demo_no == cur_no)
                })?;
                if let Some(CutsceneSequenceStep::InEngineDemo { demo_no: next_no }) =
                    chain.get(pos_in_chain + 1)
                {
                    DRAMA_DEMO_CATALOG.iter().position(|d| d.demo_no == *next_no)
                } else {
                    None
                }
            });
            if let Some(next_idx) = next_catalog_idx {
                state.selected_drama_demo_idx = next_idx;
                state.drama_demo_session =
                    open_drama_demo_session(&state.render_ctx, next_idx, false);
                if let Some(ref s) = state.drama_demo_session {
                    cutscene_camera_override = Some(s.player.camera);
                    cutscene_light_override = s.player.primary_active_light().copied();
                }
            } else {
                state.drama_demo_session = None;
            }
        } else if state
            .drama_demo_session
            .as_ref()
            .is_some_and(|s| !s.player.locks_gameplay_input())
        {
            state.drama_demo_session = None;
        }
    } else if let Some(ref session) = state.drama_demo_session {
        if session.player.locks_gameplay_input() {
            cutscene_camera_override = Some(session.player.camera);
            cutscene_light_override = session.player.primary_active_light().copied();
        }
    }

    (cutscene_camera_override, cutscene_light_override)
}

fn update_camera_uniforms(
    state: &mut AppState,
    aspect: f32,
    alpha: f32,
    cutscene_camera_override: Option<CutsceneCameraSnapshot>,
) {
    let (view, fov, cam_pos) = if let Some(cs_cam) = cutscene_camera_override {
        (cs_cam.view_matrix(), cs_cam.fov_y_radians, cs_cam.eye)
    } else {
        let v = state.sim_core.camera.interpolated_view_matrix(alpha);
        let f = state.sim_core.camera.interpolated_fov(alpha);
        let cp = state
            .sim_core
            .camera
            .prev_snapshot
            .eye
            .lerp(state.sim_core.camera.curr_snapshot.eye, alpha);
        (v, f, cp)
    };
    let proj = glam::Mat4::perspective_rh(
        fov,
        aspect,
        crate::rooms::DEFAULT_CAMERA_Z_NEAR,
        crate::rooms::DEFAULT_CAMERA_Z_FAR,
    );
    let view_proj = proj * view;
    let inv_proj = proj.inverse();
    state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);
}

fn update_spotlight_uniforms(
    state: &mut AppState,
    alpha: f32,
    player_pos: glam::Vec3,
    cutscene_light_override: Option<DemoLightState>,
) {
    let (spot_pos, light_dir) = state.sim_core.interpolated_flashlight_ray(alpha);

    let spot_uniforms = if let Some(cs_light) = cutscene_light_override {
        let l_pos = glam::Vec3::from_array(cs_light.position);
        let l_tgt = glam::Vec3::from_array(cs_light.interest);
        // Evaluate authentic PS2/PC 9-bucket linear distance decay ramp (`DEMO_LIGHT_DECAY_RAMP` at `0x50cff0`)
        let (_near_decay, far_decay) = match cs_light.kind {
            DemoLightKind::Point | DemoLightKind::Spot => {
                sh2_core::demo_light_decay_bounds(cs_light.decay[1])
            }
            DemoLightKind::Directional => (3000.0, 7500.0),
        };
        let range_boost = (far_decay / 3500.0).clamp(0.8, 2.2);
        let (pos, dir, inner_cos, outer_cos) = match cs_light.kind {
            DemoLightKind::Spot => {
                let d = (l_tgt - l_pos).normalize_or_zero();
                let safe_d = if d.length_squared() > 1e-5 {
                    d
                } else {
                    glam::Vec3::new(0.0, 0.0, 1.0)
                };
                (
                    l_pos,
                    safe_d,
                    cs_light.cone[0].cos().clamp(0.1, 0.99),
                    cs_light.cone[1].cos().clamp(0.05, 0.95),
                )
            }
            DemoLightKind::Point => {
                let d = (player_pos - l_pos).normalize_or_zero();
                let safe_d = if d.length_squared() > 1e-5 {
                    d
                } else {
                    glam::Vec3::new(0.0, 1.0, 0.0)
                };
                (l_pos, safe_d, 0.25, -0.65)
            }
            DemoLightKind::Directional => {
                // `DdsPlay` (`0x4a1bf5..0x4a1c8a`) interprets `light.position` as the light vector
                // and normalizes `-pos` as the incoming ray direction.
                let d = (-l_pos).normalize_or_zero();
                let safe_d = if d.length_squared() > 1e-5 {
                    d
                } else {
                    glam::Vec3::new(0.0, 1.0, 0.0)
                };
                (player_pos - safe_d * 1800.0, safe_d, 0.25, -0.65)
            }
        };
        sh2_renderer::light::SpotlightUniforms {
            position: [pos.x, pos.y, pos.z, 1.0],
            direction_inner: [dir.x, dir.y, dir.z, inner_cos],
            color_outer: [
                cs_light.color[0].max(0.25) * 1.8 * range_boost,
                cs_light.color[1].max(0.25) * 1.8 * range_boost,
                cs_light.color[2].max(0.25) * 1.8 * range_boost,
                outer_cos,
            ],
        }
    } else if state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input() && s.player.info.stage_name == "er01")
        || (state.active_test_room == TestRoomKind::RestroomStart
            && !state.sim_core.scenario.flashlight_on)
    {
        // In `RestroomStart` (`er01.map` / `first_toilet`), James has not found the flashlight in Room 205 yet;
        // provide the overhead sink fluorescent fixture illumination above the mirror (`X = -20000.0, Z = 20429.5`)
        // so both James and his planar mirror reflection are clearly visible.
        sh2_renderer::light::SpotlightUniforms {
            position: [-19200.0, -1950.0, 20429.5, 1.0],
            direction_inner: [-0.65, 0.76, 0.0, 0.35],
            color_outer: [1.85, 1.90, 1.80, -0.25],
        }
    } else if let Some(session) = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
    {
        // Cutscene with `0` dynamic `.dds` lights outside `er01`: place a wide ambient stage key light
        // above the active camera/actor focus so characters and level geometry are clearly illuminated.
        let cam_eye = session.player.camera.eye;
        let cam_tgt = session.player.camera.interest;
        let key_pos = cam_eye + glam::Vec3::new(0.0, -600.0, 0.0);
        let key_dir = (cam_tgt - key_pos).normalize_or_zero();
        let safe_dir = if key_dir.length_squared() > 1e-5 {
            key_dir
        } else {
            glam::Vec3::new(0.0, 0.5, 0.86)
        };
        sh2_renderer::light::SpotlightUniforms {
            position: [key_pos.x, key_pos.y, key_pos.z, 1.0],
            direction_inner: [safe_dir.x, safe_dir.y, safe_dir.z, 0.25],
            color_outer: [1.75, 1.72, 1.68, -0.50],
        }
    } else {
        let light_color = if state.sim_core.scenario.flashlight_on {
            [2.5, 2.3, 1.8, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        };
        sh2_renderer::light::SpotlightUniforms {
            position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
            direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
            color_outer: [light_color[0], light_color[1], light_color[2], 0.70],
        }
    };
    state.render_ctx.update_spotlight(&spot_uniforms);
}

fn evaluate_player_skinning(
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
                .and_then(|idx| state.combat_arena.enemies.get(idx))
                .map(|e| e.id)
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

            if !is_paused && ticks_processed > 0 {
                let dt_physics = 1.0 / 60.0;
                for _ in 0..ticks_processed {
                    c.secondary_physics
                        .step(dt_physics, auth_player_pos, auth_player_yaw);
                }
            }
            c.dynamic_locals.copy_from_slice(&c.bind_locals);
            c.secondary_physics
                .apply_to_locals_interpolated(&mut c.dynamic_locals, alpha);

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
            let build_mirror = state.active_test_room == TestRoomKind::RestroomStart;
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

fn evaluate_cutscene_actors_skinning(state: &mut AppState) {
    let Some(ref mut session) = state.drama_demo_session else {
        return;
    };
    if !session.player.locks_gameplay_input() {
        return;
    }
    let build_mirror = state.active_test_room == TestRoomKind::RestroomStart
        || session.player.info.stage_name == "er01";

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

        // 1. Evaluate `.cls` facial morph cluster weights & upload morphed vertices if present (`clani.c`)
        let actor_frame = actor.global_anm_frame(scene_frame);
        if let Some(ref cls) = asset.cls {
            cls.evaluate_global_frame_into(actor_frame, false, &mut asset.cluster_weights_scratch);
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

        // 2. Evaluate `.anm` skeletal bone transforms at `actor_frame` (`scene_frame + anm_frame_offset`)
        let frame_count = asset.anm.frames.len();
        if frame_count == 0 {
            continue;
        }
        let f0 = (actor_frame.floor() as usize).min(frame_count - 1);
        let f1 = (f0 + 1).min(frame_count - 1);
        let frac = actor_frame.fract();

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

fn evaluate_enemy_skinning(state: &mut AppState, alpha: f32) -> Option<EnemySkinSnapshot> {
    let active_enemy_snapshot = if state.active_test_room == TestRoomKind::CombatArena
        && !state.combat_arena.enemies.is_empty()
    {
        let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
        let enemy = &state.combat_arena.enemies[active_idx];
        let (enemy_pos, enemy_yaw) = enemy.interpolated_pose(alpha);
        Some((
            enemy.id,
            enemy_pos,
            enemy_yaw,
            enemy.is_grappling_player,
            enemy.anim.evaluate_blended_cursor_at_alpha(alpha),
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

fn submit_scene_draw(
    state: &mut AppState,
    is_in_game: bool,
    player_pos: glam::Vec3,
    active_enemy_snapshot: Option<EnemySkinSnapshot>,
) {
    let mut skeletal_draw = Vec::new();
    let is_restroom_mirror = state.active_test_room == TestRoomKind::RestroomStart
        || state
            .drama_demo_session
            .as_ref()
            .is_some_and(|s| s.player.locks_gameplay_input() && s.player.info.stage_name == "er01");
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

        if let Some((enemy_id, _, _, _, _)) = active_enemy_snapshot {
            if let Some(ea) = state.enemy_asset(enemy_id) {
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
                .as_ref()
                .zip(s.stage_texture_bind_groups.as_ref())
        })
        .unwrap_or((&state.gpu_map, &state.map_texture_bind_groups));

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
            && !state.combat_arena.enemies.is_empty()
        {
            let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
            let e = &state.combat_arena.enemies[active_idx];
            let enemy_radius = match e.id {
                4 => 200.0,
                5 => 220.0,
                _ => 130.0,
            };
            let enemy_lines = sh2_debug::build_chara_collision_debug_lines(
                [e.position.x, e.position.y, e.position.z],
                enemy_radius,
                1800.0,
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
}
