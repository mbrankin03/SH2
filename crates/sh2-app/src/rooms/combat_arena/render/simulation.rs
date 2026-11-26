//! 60 Hz simulation step, combat arena ticks, player combat animation/SFX sync, listener acoustics,
//! footsteps, and dynamic BGM manager stepping.

use std::time::Duration;

use crate::state::AppState;
use sh2_ui::menu::TestRoomKind;

pub(super) fn step_simulation_and_audio(
    state: &mut AppState,
    dt: Duration,
    is_in_game: bool,
) -> u32 {
    let ticks_processed = state.sim_core.update(dt);

    if ticks_processed > 0 {
        update_listener_acoustics_and_footsteps(state);
    }

    if (state.active_test_room == TestRoomKind::CombatArena
        || state.campaign_orchestrator.is_some())
        && ticks_processed > 0
    {
        step_combat_arena_ticks(state, ticks_processed);
        sync_combat_player_animation_and_sfx(state);
    }

    if is_in_game {
        step_audio_managers(state, dt, false);
    }

    ticks_processed
}

/// Steps `BgmManager` (`SeBgmManager` `0x00194300` / `0x00516430`), `SePoolManager`
/// (`Se2dManager` `0x001932f0` / `Se3dManager` `0x00193790`), and the 4-track Pocket Radio
/// (`RadioNoise` `0x001ab300` / `shSdRadio` `0x00291420`), including global pause/demo/map ducking.
pub(crate) fn step_audio_managers(state: &mut AppState, dt: Duration, is_paused: bool) {
    let ducking_mode = resolve_audio_ducking_mode(state, is_paused);
    step_bgm_manager(state, dt, ducking_mode);
    step_se_pool_manager(state, dt, ducking_mode);
    step_pocket_radio(state, dt, is_paused);
}

fn resolve_radio_ducking_state(
    state: &AppState,
    is_paused: bool,
) -> sh2_core::ai::radio::RadioDuckingState {
    if state.ingame_menus.map_viewer.is_open || state.ingame_menus.memo_reader.is_open {
        sh2_core::ai::radio::RadioDuckingState::MapOrMemo
    } else if is_paused
        || state.menu_ctrl.pause_menu.is_open
        || state.ingame_menus.is_any_open()
        || matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(sh2_core::campaign::CampaignPhase::OverlayMenu(_))
        )
    {
        sh2_core::ai::radio::RadioDuckingState::PauseOrInventory
    } else {
        sh2_core::ai::radio::RadioDuckingState::Normal
    }
}

fn sync_pocket_radio_inputs(state: &mut AppState, is_paused: bool) {
    let camera_pos = state.sim_core.active_camera_eye(1.0);
    let wvs_matrix = state.sim_core.active_ps2_wvs_matrix(1.0);
    let ducking_state = resolve_radio_ducking_state(state, is_paused);
    let radio_volume = state.ingame_menus.inventory.radio_volume;
    let radio_active_room = state.active_test_room == TestRoomKind::CombatArena
        || state.campaign_orchestrator.is_some();
    let radio_enabled = radio_active_room
        && (state.active_test_room == TestRoomKind::CombatArena
            || state.sim_core.scenario.radio_on);
    state.combat_arena.radio_enabled = radio_enabled;
    state
        .combat_arena
        .ai
        .set_radio_listener(camera_pos, wvs_matrix);
    state.combat_arena.ai.radio_evaluator.ducking_state = ducking_state;
    state.combat_arena.ai.radio_evaluator.volume_level = radio_volume;
}

fn step_pocket_radio(state: &mut AppState, dt: Duration, is_paused: bool) {
    sync_pocket_radio_inputs(state, is_paused);
    let radio_enabled = state.combat_arena.radio_enabled;
    if is_paused || !radio_enabled {
        let player_pos = state.sim_core.scenario.player_position;
        state
            .combat_arena
            .ai
            .step_radio_only(radio_enabled, player_pos, dt.as_secs_f32());
    }
    crate::audio_dispatch::dispatch_pocket_radio(state);
}

fn resolve_audio_ducking_mode(state: &AppState, is_paused: bool) -> sh2_audio::BgmDuckingMode {
    let in_overlay = matches!(
        state.campaign_orchestrator.as_ref().map(|o| &o.phase),
        Some(sh2_core::campaign::CampaignPhase::OverlayMenu(_))
    );
    let game_mode = if is_paused
        || state.menu_ctrl.pause_menu.is_open
        || state.ingame_menus.is_any_open()
        || in_overlay
    {
        5 // Pause / Inventory / Map / Memo / Options (`0.5` ducking)
    } else {
        4 // Normal gameplay
    };
    let drama_demo_id = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.step != sh2_core::cutscene::DramaDemoStep::Finished)
        .map_or(0, |s| s.player.info.demo_no as i32);
    sh2_audio::bgm_ducking_mode_from_game_state(game_mode, drama_demo_id)
}

fn update_listener_acoustics_and_footsteps(state: &mut AppState) {
    let p_pos = state.sim_core.scenario.player_position;
    let (listener_yaw, _listener_pitch) = state.sim_core.active_listener_yaw_pitch(1.0);
    let fwd = glam::Vec3::new(listener_yaw.sin(), 0.0, listener_yaw.cos());
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
        eng.update_room_acoustics_from_record(active_cam_record, is_interior_map);
    }
    if state.sim_core.footstep_triggered_since_last_poll {
        state.sim_core.footstep_triggered_since_last_poll = false;
        let is_running = state.sim_core.last_locomotion.state.is_running();
        if let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_mut()) {
            let _ = bank.play_footstep(
                eng,
                state.active_maria,
                state.sim_core.active_floor_material,
                is_running,
                state.sim_core.last_locomotion.footstep_volume,
            );
        }
        // Kick up `Effect2/hh_class_raise_dust_phenomenon_00.c` (`0x0025b2e0`) airborne dust motes
        // on running footsteps using exact `HH_Effect_Object_Ground_Impact_Post` (`0x0025aa30`
        // line 221: `y - 50.0`) and `HH_Class_Plural_Phenomenon_01` (`0x00286ea0` line 216,
        // `0x0037b490`: `[0.0, -2500.0, 0.0]`).
        if is_running {
            let spawn_pos = p_pos
                + glam::Vec3::new(0.0, sh2_core::lighting::GROUND_IMPACT_FOOT_Y_OFFSET_MM, 0.0);
            let impact_vel =
                glam::Vec3::from_array(sh2_core::lighting::PLURAL_PHENOMENON_RAISE_DUST_VEL);
            state
                .dust_system
                .spawn_raise_dust(spawn_pos, impact_vel, p_pos.y);
        }
    }
}

fn step_combat_arena_ticks(state: &mut AppState, ticks_processed: u32) {
    let tick_dt = 1.0 / 60.0;
    let ready_held = state.key_ready_held || state.sim_core.input.state.ready_weapon_pressed;
    let atk_held = state.key_attack_held || state.sim_core.input.state.action_pressed;
    let mut atk_pressed = state.key_attack_pressed || state.sim_core.input.state.action_pressed;
    state.key_attack_pressed = false;
    let mut struggle_presses = state.struggle_button_press_count;
    state.struggle_button_press_count = 0;

    sync_pocket_radio_inputs(state, false);
    for _ in 0..ticks_processed {
        state.combat_arena.player_position = state.sim_core.scenario.player_position;
        if state.sim_core.active_camera_mode().is_modern() && ready_held {
            let (_aim_origin, aim_dir) = state.sim_core.active_aim_ray();
            if aim_dir.x.abs() + aim_dir.z.abs() > 1e-4 {
                state.combat_arena.player_yaw = aim_dir.x.atan2(aim_dir.z);
            } else {
                state.combat_arena.player_yaw = state.sim_core.scenario.player_yaw;
            }
        } else {
            state.combat_arena.player_yaw = state.sim_core.scenario.player_yaw;
        }

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

        state
            .combat_arena
            .player
            .apply_control_options(&state.menu_ctrl.options_menu.draft_config.controls);

        let r2 = state.sim_core.input.state.r2_analog_pressure;
        let attack_pressure = (r2 > 0.0).then(|| (r2 * 255.0).round().clamp(1.0, 255.0) as u8);
        let combat_input = sh2_core::combat::CombatInputSnapshot {
            ready_held,
            action_pressed: atk_pressed,
            action_held: atk_held,
            has_downed_enemy_nearby: false,
            is_running: state.sim_core.last_locomotion.state.is_running()
                || state.sim_core.input.state.is_running,
            attack_pressure,
            native_light_pressed: state.sim_core.input.state.native_light_attack_pressed,
            native_heavy_pressed: state.sim_core.input.state.native_heavy_attack_pressed,
        };

        let prev_enemy_states: Vec<(f32, bool)> = state
            .combat_arena
            .ai
            .enemies
            .iter()
            .map(|e| (e.controller.health(), e.is_dead()))
            .collect();

        if state.campaign_orchestrator.is_some() {
            state.combat_arena.tick_with_collision_input(
                combat_input,
                Some(&state.sim_core.collision),
                tick_dt,
            );
            let boss_events = state.combat_arena.ai.consume_boss_events();
            state.sim_core.scenario.apply_boss_events(&boss_events);
        } else {
            state
                .combat_arena
                .tick_isolated_input(combat_input, tick_dt, state.combat_enemy_idx);
        }

        // Dispatch `Effect2` hit splashes & dying blood pools (`enDyingExec` -> `enEfctBloodPool`)
        for (idx, enemy) in state.combat_arena.ai.enemies.iter().enumerate() {
            if let Some(&(prev_hp, prev_dead)) = prev_enemy_states.get(idx) {
                if enemy.controller.health() < prev_hp - 0.01 {
                    let hit_pos = enemy.position() + glam::Vec3::new(0.0, -700.0, 0.0);
                    let hit_dir = enemy.position() - state.combat_arena.player_position;
                    state.blood_system.post_hit_splash(hit_pos, hit_dir);
                }
                if !prev_dead && enemy.is_dead() {
                    state.blood_system.post_enemy_death_pool(enemy.position());
                }
            }
        }
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
                    .and_then(|idx| state.combat_arena.ai.enemies.get(idx))
                    .and_then(|e| sh2_core::combat::BestiaryId::from_raw(e.id()))
                    .map(|sp| sh2_core::combat::GrappleAnchor::for_species(sp).struggle_clip)
                    .unwrap_or(sh2_core::animation::JamesBasicClips::STRUGGLE_TORSO)
            };
            if state.sim_core.animation.current_clip().start_frame != struggle_clip.start_frame {
                tracing::info!(
                    "[Combat Grapple] James held by Enemy #{}: transitioning to clip frames {}..={} @ {:.0}fps",
                    state
                        .combat_arena
                        .grappling_enemy_idx
                        .and_then(|idx| state.combat_arena.ai.enemies.get(idx).map(|e| e.id()))
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

fn step_bgm_manager(state: &mut AppState, dt: Duration, ducking_mode: sh2_audio::BgmDuckingMode) {
    let (bgm_stage_id, bgm_flags) = state.active_bgm_stage_and_flags();
    state.bgm_manager.event_flags = bgm_flags;
    state.bgm_manager.set_ducking_mode(ducking_mode);
    let (hp_ratio, min_enemy_dist_xz) = if state.active_test_room == TestRoomKind::CombatArena
        || state.campaign_orchestrator.is_some()
    {
        let p_hp = if state.campaign_orchestrator.is_some() {
            (state.sim_core.scenario.health / state.sim_core.scenario.max_health.max(1.0))
                .clamp(0.0, 1.0)
        } else {
            (state.combat_arena.player.health / state.combat_arena.player.max_health.max(1.0))
                .clamp(0.0, 1.0)
        };
        let p_pos = state.sim_core.scenario.player_position;
        let min_dist = state
            .combat_arena
            .ai
            .enemies
            .iter()
            .filter(|e| !e.is_dead() && e.is_aggroed())
            .map(|e| {
                let pos = e.position();
                let dx = pos.x - p_pos.x;
                let dz = pos.z - p_pos.z;
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

fn step_se_pool_manager(
    state: &mut AppState,
    dt: Duration,
    ducking_mode: sh2_audio::BgmDuckingMode,
) {
    crate::audio_dispatch::sync_se_pool_3d_bank(state);
    let listener = sh2_audio::SeListenerContext {
        camera_pos: state.sim_core.active_camera_eye(1.0),
        player_pos: state.sim_core.scenario.player_position,
        wvs_matrix: state.sim_core.active_ps2_wvs_matrix(1.0),
        room_id: state.bgm_manager.room_id as u16,
    };
    let mut actions = Vec::new();
    let mut finished_2d = [false; sh2_audio::SE_2D_SLOT_COUNT];
    if let Some(ref bank) = state.audio_bank {
        for (idx, slot_opt) in state.se_pool.slots_2d.iter().enumerate() {
            if let Some(slot) = slot_opt {
                if !bank.is_2d_se_playing(slot.se_id) {
                    finished_2d[idx] = true;
                }
            }
        }
    }
    state.se_pool.tick_2d_with_finished(
        &listener,
        ducking_mode,
        dt.as_secs_f32(),
        &finished_2d,
        &mut actions,
    );
    let mut finished_channels = [false; sh2_audio::SE_3D_CHANNEL_COUNT];
    if let Some(ref bank) = state.audio_bank {
        for (idx, ch_opt) in state.se_pool.channels_3d.iter().enumerate() {
            if let Some(ch) = ch_opt {
                let playing = bank.is_2d_se_playing(ch.se_id)
                    || bank
                        .active_spatial_voices
                        .get(&ch.se_id)
                        .is_some_and(|v| !v.is_finished());
                if !playing {
                    finished_channels[idx] = true;
                }
            }
        }
    }
    state
        .se_pool
        .tick_3d(&listener, &finished_channels, &mut actions);
    crate::audio_dispatch::dispatch_se_pool_actions(state, &actions);
}
