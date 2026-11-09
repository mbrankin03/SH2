//! 3D Stage Exploration & Combat Testing Arena room controller (`SH2-13` / `SH2-57`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::assets::{load_cutscene_voice_wav, open_drama_demo_session};
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::formats::anm::{
    blend_frame_in_place, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into,
};
use sh2_assets::formats::demo::DemoLightKind;
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::DRAMA_DEMO_CATALOG;
use sh2_ui::menu::TestRoomKind;

pub(crate) struct CombatArenaRoomController;

impl RoomController for CombatArenaRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        // When an in-engine real-time 3D cutscene (`DramaDemoPlayer`) is active, lock gameplay
        // locomotion and pause menu (`SH2-63`); allow `Escape` / `Enter` to skip (`DramaDemoSkipLast`)
        // or `[` / `]` to cycle the cutscene catalog.
        if state
            .drama_demo_session
            .as_ref()
            .is_some_and(|s| s.player.locks_gameplay_input())
        {
            state.key_forward = false;
            state.key_backward = false;
            state.key_left = false;
            state.key_right = false;
            state.sim_core.input.state.left_stick_x = 0.0;
            state.sim_core.input.state.left_stick_y = 0.0;

            if pressed && !repeat {
                match key_code {
                    KeyCode::Escape | KeyCode::Backspace | KeyCode::Enter | KeyCode::KeyG => {
                        if let Some(mut session) = state.drama_demo_session.take() {
                            session.player.skip();
                            if let Some((final_pos, final_yaw)) =
                                session.player.player_handoff_transform()
                            {
                                state.sim_core.snap_player_transform(final_pos, final_yaw);
                                state.sim_core.camera.reset_to_player(final_pos, final_yaw);
                            }
                            if let Some(ref mut eng) = state.audio_engine {
                                eng.stop_voice(Duration::from_millis(40));
                            }
                            tracing::info!(
                                "[Cutscene Player] Skipped DramaDemo #{}: '{}' -> returned control to gameplay.",
                                session.player.info.demo_no,
                                session.player.info.title
                            );
                        }
                    }
                    KeyCode::Period => {
                        state.selected_drama_demo_idx =
                            (state.selected_drama_demo_idx + 1) % DRAMA_DEMO_CATALOG.len();
                        if let Some(ref mut eng) = state.audio_engine {
                            eng.stop_voice(Duration::from_millis(30));
                        }
                        state.drama_demo_session =
                            open_drama_demo_session(state.selected_drama_demo_idx, false);
                    }
                    KeyCode::Comma => {
                        state.selected_drama_demo_idx = if state.selected_drama_demo_idx == 0 {
                            DRAMA_DEMO_CATALOG.len() - 1
                        } else {
                            state.selected_drama_demo_idx - 1
                        };
                        if let Some(ref mut eng) = state.audio_engine {
                            eng.stop_voice(Duration::from_millis(30));
                        }
                        state.drama_demo_session =
                            open_drama_demo_session(state.selected_drama_demo_idx, false);
                    }
                    _ => {}
                }
            }
            return;
        }

        match key_code {
            KeyCode::Escape | KeyCode::Backspace if pressed => {
                let stage_name = state.active_test_room.label();
                // Check user configuration: save_anywhere (PC mode) vs restricted to red squares (PS2 mode)
                let can_save = state.menu_ctrl.options_menu.draft_config.game.save_anywhere;
                state
                    .menu_ctrl
                    .pause_menu
                    .open(can_save, stage_name, state.sim_time as f64);
                if let Some(ref mut engine) = state.audio_engine {
                    engine.set_bgm_paused(true);
                    let _ = play_menu_sound(
                        engine,
                        state.audio_bank.as_ref(),
                        SoundCueKind::CursorMove,
                    );
                }
                tracing::info!("Paused gameplay -> In-Game Pause Menu opened.");
                return;
            }
            KeyCode::KeyW | KeyCode::ArrowUp => {
                state.key_forward = pressed;
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                state.key_backward = pressed;
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                state.key_left = pressed;
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                state.key_right = pressed;
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                state.sim_core.input.state.is_running = pressed;
            }
            KeyCode::KeyF if pressed => {
                state.sim_core.scenario.flashlight_on = !state.sim_core.scenario.flashlight_on;
                if let (Some(eng), Some(bank)) =
                    (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                {
                    let _ = bank.play_flashlight_toggle(eng, state.sim_core.scenario.flashlight_on);
                }
            }
            KeyCode::KeyC if pressed => {
                state.active_maria = !state.active_maria;
                let locomotion_clips = if state.active_maria {
                    sh2_core::CharacterLocomotionClips::MARIA
                } else {
                    sh2_core::CharacterLocomotionClips::JAMES
                };
                state.sim_core.set_locomotion_clips(locomotion_clips);
                tracing::info!(
                    "Switched active character to: {}",
                    if state.active_maria { "Maria" } else { "James" }
                );
            }
            // Volumetric Fog calibration shortcuts (matching PS2 dbg_fogparm_ctl / dbg_ambifog_ctl)
            KeyCode::BracketLeft if pressed => {
                state.fog_uniforms.density = (state.fog_uniforms.density - 0.05).max(0.0);
                state.render_ctx.update_fog(&state.fog_uniforms);
                tracing::info!("[Fog Tweak] Density: {:.2}", state.fog_uniforms.density);
            }
            KeyCode::BracketRight if pressed => {
                state.fog_uniforms.density = (state.fog_uniforms.density + 0.05).min(2.0);
                state.render_ctx.update_fog(&state.fog_uniforms);
                tracing::info!("[Fog Tweak] Density: {:.2}", state.fog_uniforms.density);
            }
            KeyCode::Minus if pressed => {
                state.fog_uniforms.fog_far =
                    (state.fog_uniforms.fog_far - 1000.0).max(state.fog_uniforms.fog_near + 500.0);
                state.render_ctx.update_fog(&state.fog_uniforms);
                tracing::info!("[Fog Tweak] fog_far: {:.0}", state.fog_uniforms.fog_far);
            }
            KeyCode::Equal if pressed => {
                state.fog_uniforms.fog_far = (state.fog_uniforms.fog_far + 1000.0).min(60000.0);
                state.render_ctx.update_fog(&state.fog_uniforms);
                tracing::info!("[Fog Tweak] fog_far: {:.0}", state.fog_uniforms.fog_far);
            }
            KeyCode::KeyM if pressed => {
                // Cycle curve type (0 = Linear, 1 = Exp, 2 = Exp Squared)
                state.fog_uniforms.curve_type = (state.fog_uniforms.curve_type + 1) % 3;
                state.render_ctx.update_fog(&state.fog_uniforms);
                let curve_name = match state.fog_uniforms.curve_type {
                    0 => "Linear",
                    1 => "Exponential",
                    _ => "Exponential Squared",
                };
                tracing::info!("[Fog Tweak] Falloff curve: {}", curve_name);
            }
            KeyCode::KeyK if pressed => {
                state.debug_collision_overlay = !state.debug_collision_overlay;
                tracing::info!(
                    "[Debug] Collision wireframe overlay (clDbgDrawBGColiPoly): {}",
                    state.debug_collision_overlay
                );
            }
            KeyCode::F1 | KeyCode::F3 if pressed => {
                state.debug_camera_hud = !state.debug_camera_hud;
                tracing::info!(
                    "[Debug] Camera & Player telemetry HUD: {}",
                    state.debug_camera_hud
                );
            }
            // Authentic Combat controls (Space / F / Enter to Attack, R / Shift to Ready, 1..6 to switch weapons)
            KeyCode::KeyR => {
                state.key_ready_held = pressed;
            }
            KeyCode::Space => {
                if pressed && !state.key_attack_held {
                    state.key_attack_pressed = true;
                }
                state.key_attack_held = pressed;
            }
            KeyCode::Digit1 if pressed => {
                state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::WoodenPlank;
                tracing::info!("[Combat] Equipped Wooden Plank");
            }
            KeyCode::Digit2 if pressed => {
                state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::SteelPipe;
                tracing::info!("[Combat] Equipped Steel Pipe");
            }
            KeyCode::Digit3 if pressed => {
                state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::Handgun;
                state.combat_arena.player.loaded_rounds[0] = 10;
                state.combat_arena.player.spare_rounds[0] = 50;
                tracing::info!("[Combat] Equipped Handgun (10/50)");
            }
            KeyCode::Digit4 if pressed => {
                state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::Shotgun;
                state.combat_arena.player.loaded_rounds[1] = 6;
                state.combat_arena.player.spare_rounds[1] = 30;
                tracing::info!("[Combat] Equipped Shotgun (6/30)");
            }
            KeyCode::Digit5 if pressed => {
                state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::HuntingRifle;
                state.combat_arena.player.loaded_rounds[2] = 4;
                state.combat_arena.player.spare_rounds[2] = 20;
                tracing::info!("[Combat] Equipped Hunting Rifle (4/20)");
            }
            KeyCode::Digit6 if pressed => {
                state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::GreatKnife;
                tracing::info!("[Combat] Equipped Great Knife");
            }
            // Teleport / cycle selected enemy right in front of James for testing combat & grab interactions
            KeyCode::Tab if pressed && !state.combat_arena.enemies.is_empty() => {
                let fwd = glam::Vec3::new(
                    state.sim_core.scenario.player_yaw.sin(),
                    0.0,
                    state.sim_core.scenario.player_yaw.cos(),
                );
                let target_pos = state.combat_arena.player_position + fwd * 350.0;
                let facing_yaw = state.sim_core.scenario.player_yaw + std::f32::consts::PI;

                // Advance focus to next enemy
                state.combat_enemy_idx =
                    (state.combat_enemy_idx + 1) % state.combat_arena.enemies.len();
                let target_enemy = &mut state.combat_arena.enemies[state.combat_enemy_idx];

                // Teleport selected enemy in front of player and engage
                target_enemy.snap_pose(target_pos, facing_yaw);
                target_enemy.is_aggroed = true;
                target_enemy.flinch_timer = 0.0;
                target_enemy.attack_cooldown = 0.2;

                // Revive / reset James on creature cycling so arena testing is always fresh
                state.combat_arena.player.health = state.combat_arena.player.max_health;
                state.combat_arena.player.mode = sh2_core::combat::CombatMode::Free;
                state.combat_arena.player.invulnerability_timer = 0.0;
                state.combat_arena.player.struggle_meter = 0.0;
                state.combat_arena.grapple_session = None;
                state.combat_arena.grappling_enemy_idx = None;
                state.sim_core.animation_locked = false;

                tracing::info!(
                    "[Combat] Cycled to Enemy #{}: {} (teleported to X:{:.1} Z:{:.1})",
                    target_enemy.id,
                    target_enemy.name,
                    target_pos.x,
                    target_pos.z
                );
            }
            // Toggle between SH2:EE 48kHz Lossless (`sh2e/sound/`) and Original PS2/PC (`data/sound/`) BGM & SFX packs (`SH2-60` / `SH2-61`)
            KeyCode::KeyB if pressed && !repeat => {
                let next_source = state
                    .bgm_manager
                    .audio_pack_source
                    .toggle_concrete(&state.menu_ctrl.audio_pack_status);
                if let Err(e) = state
                    .bgm_manager
                    .set_audio_pack_source(next_source, state.audio_engine.as_mut())
                {
                    tracing::warn!("[BGM] Failed to switch audio pack source: {:?}", e);
                } else {
                    let resolver = sh2_assets::AssetPathResolver::resolve();
                    state.audio_bank = Some(sh2_audio::MenuAudioBank::load_from_dirs_for_source(
                        &resolver.data_dir,
                        resolver.sh2e_dir.as_deref(),
                        next_source,
                    ));
                    tracing::info!(
                        "[Audio] Switched AudioPackSource -> {:?} ({})",
                        next_source,
                        state.bgm_manager.hud_summary()
                    );
                }
            }
            // Cycle BGM stage presets (`SeBgmChange` / `SeBgmManager` multi-channel AIX & ADX verification)
            KeyCode::KeyN if pressed && !repeat => {
                const STAGE_PRESETS: &[u32] = &[3, 2, 18, 24, 45, 7, 14, 32, 47, 48];
                let current_stage = state.active_bgm_stage_and_flags().0;
                let next_idx = STAGE_PRESETS
                    .iter()
                    .position(|&s| s == current_stage)
                    .map(|i| (i + 1) % STAGE_PRESETS.len())
                    .unwrap_or(0);
                let next_stage = STAGE_PRESETS[next_idx];
                state.bgm_stage_override = Some(next_stage);
                let (stg, flags) = state.active_bgm_stage_and_flags();
                state.bgm_manager.event_flags = flags;
                let page = state
                    .bgm_manager
                    .update_stage_and_position(stg, state.sim_core.scenario.player_position);
                tracing::info!(
                    "[BGM] Cycled BGM stage preset -> Stage {} (BGM Page {})",
                    stg,
                    page
                );
            }
            // Launch or cycle in-engine real-time 3D cutscene (`DramaDemoPlayer`, `SH2-63`)
            KeyCode::KeyG if pressed && !repeat => {
                state.drama_demo_session =
                    open_drama_demo_session(state.selected_drama_demo_idx, false);
            }
            KeyCode::Period if pressed && !repeat => {
                state.selected_drama_demo_idx =
                    (state.selected_drama_demo_idx + 1) % DRAMA_DEMO_CATALOG.len();
                tracing::info!(
                    "[Cutscene Player] Selected DramaDemo [{}/{}]: {} (Press [G] to Play)",
                    state.selected_drama_demo_idx + 1,
                    DRAMA_DEMO_CATALOG.len(),
                    DRAMA_DEMO_CATALOG[state.selected_drama_demo_idx].title
                );
            }
            KeyCode::Comma if pressed && !repeat => {
                state.selected_drama_demo_idx = if state.selected_drama_demo_idx == 0 {
                    DRAMA_DEMO_CATALOG.len() - 1
                } else {
                    state.selected_drama_demo_idx - 1
                };
                tracing::info!(
                    "[Cutscene Player] Selected DramaDemo [{}/{}]: {} (Press [G] to Play)",
                    state.selected_drama_demo_idx + 1,
                    DRAMA_DEMO_CATALOG.len(),
                    DRAMA_DEMO_CATALOG[state.selected_drama_demo_idx].title
                );
            }
            _ => {}
        }

        // Count any key press while held in grapple for authentic struggle meter drain (0x5565c0).
        // Filter out OS key repeat so holding a movement/run key does not drain 4s in 0.45s.
        if pressed
            && !repeat
            && state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held
        {
            state.struggle_button_press_count += 1;
        }

        // Compute accumulated directional keyboard input
        let mut kb_y = 0.0f32;
        let mut kb_x = 0.0f32;
        if state.key_forward {
            kb_y += 1.0;
        }
        if state.key_backward {
            kb_y -= 1.0;
        }
        if state.key_right {
            kb_x += 1.0;
        }
        if state.key_left {
            kb_x -= 1.0;
        }

        if state.active_test_room == TestRoomKind::CombatArena
            && state.combat_arena.player.mode != sh2_core::combat::CombatMode::Free
        {
            kb_x = 0.0;
            kb_y = 0.0;
        }

        state.sim_core.input.state.left_stick_y = kb_y;
        state.sim_core.input.state.left_stick_x = kb_x;
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        _dt_sec: f32,
        aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        // 1. Advance simulation at fixed 60 Hz tick (runs animation controller update)
        let mut ticks_processed = 0u32;
        if !is_paused {
            if state.active_test_room == TestRoomKind::CombatArena
                && state.combat_arena.player.mode != sh2_core::combat::CombatMode::Free
            {
                state.sim_core.input.state.left_stick_x = 0.0;
                state.sim_core.input.state.left_stick_y = 0.0;
            }
            ticks_processed = state.sim_core.update(dt);

            if ticks_processed > 0 {
                // Update 3D audio listener transform (`SeCallPosDistanceF` `0x5155E0`), dynamic `.cam` room acoustics (`SH2-55`), & material footsteps (`SH2-61`)
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
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
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

            if state.active_test_room == TestRoomKind::CombatArena && ticks_processed > 0 {
                let tick_dt = 1.0 / 60.0;
                let mut atk_pressed = state.key_attack_pressed;
                state.key_attack_pressed = false;
                let mut struggle_presses = state.struggle_button_press_count;
                state.struggle_button_press_count = 0;

                for _ in 0..ticks_processed {
                    state.combat_arena.player_position = state.sim_core.scenario.player_position;
                    state.combat_arena.player_yaw = state.sim_core.scenario.player_yaw;

                    // Process struggle input if currently held by a creature (0x5565c0)
                    if struggle_presses > 0 {
                        if state.combat_arena.grapple_session.is_some() {
                            state
                                .combat_arena
                                .register_grapple_struggle_input(struggle_presses);
                        } else if state.combat_arena.player.mode
                            == sh2_core::combat::CombatMode::Held
                        {
                            state.combat_arena.player.process_struggle_inputs(
                                struggle_presses,
                                tick_dt,
                                false,
                            );
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

                    // Sync James player position and yaw back from arena (e.g. grapple turn/facing & enemy pushback)
                    state.sim_core.scenario.player_position = state.combat_arena.player_position;
                    if state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held {
                        state.sim_core.scenario.player_yaw = state.combat_arena.player_yaw;
                    }
                }

                // Drive James's skeletal animation when in combat states (Aim, Attack, Held struggle loops, Hurt flinch, Fall)
                match state.combat_arena.player.mode {
                    sh2_core::combat::CombatMode::Held => {
                        state.sim_core.animation_locked = true;
                        let stage_anm_available =
                            match state.combat_arena.active_grapple_james_anm_overlay() {
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
                                .map(|sp| {
                                    sh2_core::combat::GrappleAnchor::for_species(sp).struggle_clip
                                })
                                .unwrap_or(sh2_core::animation::JamesBasicClips::STRUGGLE_TORSO)
                        };
                        if state.sim_core.animation.current_clip().start_frame
                            != struggle_clip.start_frame
                        {
                            tracing::info!(
                                "[Combat Grapple] James held by Enemy #{}: transitioning to clip frames {}..={} @ {:.0}fps",
                                state
                                    .combat_arena
                                    .grappling_enemy_idx
                                    .map(|idx| state
                                        .combat_arena
                                        .enemies
                                        .get(idx)
                                        .map(|e| e.id)
                                        .unwrap_or(0))
                                    .unwrap_or(0),
                                struggle_clip.start_frame,
                                struggle_clip.end_frame,
                                struggle_clip.fps
                            );
                        }
                        state.sim_core.animation.transition_to(
                            sh2_core::animation::AnimationStateKind::Held,
                            struggle_clip,
                        );
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
                    | sh2_core::combat::CombatMode::Recover => {
                        state.sim_core.animation_locked = true;
                        state.sim_core.animation.transition_to(
                            sh2_core::animation::AnimationStateKind::Attack,
                            sh2_core::animation::JamesBasicClips::ATTACK,
                        );
                    }
                    sh2_core::combat::CombatMode::Stomp => {
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

                // Play any combat sound effects requested by the combat state machine (`SH2-61`)
                if let Some(sfx_id) = state.combat_arena.player.pending_sfx.take() {
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        let _ = bank.play_se_id(eng, sfx_id, 0.95);
                    }
                    tracing::info!("[Combat SFX] Triggered sound effect ID: {}", sfx_id);
                }
            }

            // Advance `SeBgmManager` (`0x516080`) streaming BGM & multi-channel AIX spatial mixing when in-game
            if is_in_game {
                let (bgm_stage_id, bgm_flags) = state.active_bgm_stage_and_flags();
                state.bgm_manager.event_flags = bgm_flags;
                let (hp_ratio, min_enemy_dist_xz) =
                    if state.active_test_room == TestRoomKind::CombatArena {
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
        }

        // 2. Advance active in-engine 3D cutscene (`DramaDemoPlayer`, `SH2-63`) if running,
        // or compute smooth sub-frame interpolated gameplay camera for high refresh OLED (60–240 Hz+).
        let alpha = state.sim_core.interpolation_alpha();
        let mut cutscene_camera_override = None;
        let mut cutscene_light_override = None;

        if !is_paused {
            let subs_enabled = state.menu_ctrl.options_menu.draft_config.game.subtitles;
            let mut voice_afs_to_play = None;
            let mut stop_voice_now = false;
            let mut finished_handoff = None;

            if let Some(ref mut session) = state.drama_demo_session {
                session.player.tick(dt.as_secs_f32(), subs_enabled);
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
            if finished_handoff.is_some() {
                if let Some((final_pos, final_yaw)) = finished_handoff {
                    state.sim_core.snap_player_transform(final_pos, final_yaw);
                    state.sim_core.camera.reset_to_player(final_pos, final_yaw);
                }
                state.drama_demo_session = None;
            }
        } else if let Some(ref session) = state.drama_demo_session {
            if session.player.locks_gameplay_input() {
                cutscene_camera_override = Some(session.player.camera);
                cutscene_light_override = session.player.primary_active_light().copied();
            }
        }

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
            super::DEFAULT_CAMERA_Z_NEAR,
            super::DEFAULT_CAMERA_Z_FAR,
        );
        let view_proj = proj * view;
        let inv_proj = proj.inverse();
        state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);

        // 3. Dynamic Flashlight or `.dds` Cutscene Scene Light (`DdsPlayLight`)
        let (player_pos, player_yaw) = state.sim_core.interpolated_player_pose(alpha);
        let (spot_pos, light_dir) = state.sim_core.interpolated_flashlight_ray(alpha);

        let spot_uniforms = if let Some(cs_light) = cutscene_light_override {
            let l_pos = glam::Vec3::from_array(cs_light.position);
            let l_tgt = glam::Vec3::from_array(cs_light.interest);
            let dir = match cs_light.kind {
                DemoLightKind::Spot => (l_tgt - l_pos).normalize_or_zero(),
                DemoLightKind::Directional => (-l_pos).normalize_or_zero(),
                DemoLightKind::Point => (player_pos - l_pos).normalize_or_zero(),
            };
            let safe_dir = if dir.length_squared() > 1e-5 {
                dir
            } else {
                glam::Vec3::new(0.0, 0.0, 1.0)
            };
            let (inner_cos, outer_cos) = if cs_light.kind == DemoLightKind::Spot {
                (
                    cs_light.cone[0].cos().clamp(0.1, 0.99),
                    cs_light.cone[1].cos().clamp(0.05, 0.95),
                )
            } else {
                (-1.0, -1.0)
            };
            sh2_renderer::light::SpotlightUniforms {
                position: [l_pos.x, l_pos.y, l_pos.z, 1.0],
                direction_inner: [safe_dir.x, safe_dir.y, safe_dir.z, inner_cos],
                color_outer: [
                    cs_light.color[0].max(0.2) * 2.0,
                    cs_light.color[1].max(0.2) * 2.0,
                    cs_light.color[2].max(0.2) * 2.0,
                    outer_cos,
                ],
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

        // 4. Sample skeletal animation at continuous sub-frame alpha and compute character skinning matrices
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

            // When James is caught in a boss grapple in the Combat Arena, sample his
            // stage-specific overlay ANM file (`pjames_stage_anim`) if applicable.
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

                // Step secondary cloth/jacket spring physics strictly on 60 Hz simulation ticks
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
                }
            }
        }

        // 5. Evaluate active enemy skeletal model in Combat Arena with sub-frame alpha interpolation
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

        if let Some((
            enemy_id,
            enemy_pos,
            enemy_yaw,
            is_grappling,
            (prev_cursor, cur_cursor, blend),
        )) = active_enemy_snapshot
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
                            blend_frame_in_place(
                                &ea.frame_scratch_b,
                                blend,
                                &mut ea.frame_scratch_a,
                            );
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

        let mut skeletal_draw = Vec::new();
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

        if is_in_game {
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
                    [0.0, 1.0, 0.8, 1.0], // Cyan/green floor wireframes
                    [1.0, 0.2, 0.2, 1.0], // Red wall wireframes
                    &mut debug_lines,
                );

                // Draw player cylinder
                let chara_lines = sh2_debug::build_chara_collision_debug_lines(
                    player_pos.into(),
                    state.sim_core.collision_config.radius,
                    state.sim_core.collision_config.height,
                    16,
                    [1.0, 0.9, 0.1, 1.0], // Yellow player collision cylinder
                );
                debug_lines.extend(chara_lines);

                // In Combat Arena, draw active enemy cylinder
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
                        [1.0, 0.2, 0.8, 1.0], // Magenta enemy cylinder
                    );
                    debug_lines.extend(enemy_lines);
                }

                state.render_ctx.render_scene_with_debug_lines(
                    Some(&state.gpu_map),
                    Some(&state.map_texture_bind_groups),
                    &skeletal_draw,
                    &debug_lines,
                );
            } else {
                state.render_ctx.render_scene(
                    Some(&state.gpu_map),
                    Some(&state.map_texture_bind_groups),
                    &skeletal_draw,
                );
            }
        }
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        state.begin_hud_overlay();

        let yellow = [245, 210, 65, 255];
        let cyan = [105, 215, 235, 255];
        let green = [120, 230, 150, 255];
        let white = [225, 225, 225, 255];
        let muted = [165, 165, 170, 255];

        let p_pos = state.sim_core.scenario.player_position;
        let p_yaw = state.sim_core.scenario.player_yaw;
        let cam_eye = state.sim_core.camera.curr_snapshot.eye;
        let cam_tgt = state.sim_core.camera.curr_snapshot.target;
        let cam_fov = state.sim_core.camera.curr_snapshot.fov_y.to_degrees();
        let active_rec = state.sim_core.camera.active_road_index;
        let mv_type = state.sim_core.camera.mv_type;
        let room_label = state.active_test_room.label();

        state.menu_rasterizer.draw_text(
            &format!(
                "[ROOM: {}] CAMERA & PLAYER TELEMETRY (F1/F3 Toggle)",
                room_label
            ),
            20,
            18,
            1,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "PLAYER POS: X: {:.1}  Y: {:.1}  Z: {:.1} | YAW: {:.2} rad ({:.1}°)",
                p_pos.x,
                p_pos.y,
                p_pos.z,
                p_yaw,
                p_yaw.to_degrees()
            ),
            20,
            38,
            1,
            green,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "CAMERA EYE: X: {:.1}  Y: {:.1}  Z: {:.1}",
                cam_eye.x, cam_eye.y, cam_eye.z
            ),
            20,
            58,
            1,
            cyan,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "CAMERA TGT: X: {:.1}  Y: {:.1}  Z: {:.1}",
                cam_tgt.x, cam_tgt.y, cam_tgt.z
            ),
            20,
            78,
            1,
            white,
            true,
        );
        let rec_str = match active_rec {
            Some(idx) => format!("Record #{}", idx),
            None => "Fallback (None)".to_string(),
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "CAM MODE: {:?} | ACTIVE ROAD: {} | FOV: {:.1}°",
                mv_type, rec_str, cam_fov
            ),
            20,
            98,
            1,
            muted,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!("{} [B: Pack | N: Stage]", state.bgm_manager.hud_summary()),
            20,
            116,
            1,
            cyan,
            true,
        );

        if state.active_test_room == TestRoomKind::CombatArena {
            let p = &state.combat_arena.player;
            let weapon_str = format!("{:?}", p.weapon);
            let ammo_str = if p.weapon.is_firearm() {
                format!("AMMO: {} / {}", p.loaded_ammo(), p.spare_ammo())
            } else {
                "MELEE".to_string()
            };
            let mode_str = format!("{:?}", p.mode);
            let atk_str = if let Some(atk) = p.active_attack {
                format!("ATTACKING (AP: {:.0}, SP: {:.0})", atk.ap, atk.sp)
            } else if p.mode == sh2_core::combat::CombatMode::Ready {
                "AIM / READY".to_string()
            } else {
                "IDLE".to_string()
            };

            state.menu_rasterizer.draw_text(
                &format!(
                    "[COMBAT ARENA] WEAPON: {} | {} | MODE: {} [{}]",
                    weapon_str, ammo_str, mode_str, atk_str
                ),
                20,
                138,
                1,
                yellow,
                true,
            );

            // Render active enemy status line
            if !state.combat_arena.enemies.is_empty() {
                let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
                let enemy = &state.combat_arena.enemies[active_idx];
                let status_str = if enemy.is_dead {
                    "DEAD (Dispatched)".to_string()
                } else if enemy.is_downed {
                    format!(
                        "DOWNED (Stomp to Finish!) [Recover in {:.1}s]",
                        enemy.down_timer.max(0.0)
                    )
                } else if enemy.flinch_timer > 0.0 {
                    "STAGGERED / FLINCHING".to_string()
                } else if enemy.is_attacking {
                    "ATTACKING / GRAPPLING".to_string()
                } else if enemy.is_aggroed {
                    "ENGAGED (Approaching)".to_string()
                } else {
                    "STANDBY (In Section)".to_string()
                };

                let color = if enemy.is_dead {
                    muted
                } else if enemy.is_downed {
                    cyan
                } else if enemy.is_attacking || enemy.is_aggroed {
                    green
                } else {
                    muted
                };

                state.menu_rasterizer.draw_text(
                    &format!(
                        "ACTIVE TARGET [{}/{}]: Enemy #{}: {} | HP: {:.0}/{:.0} | STATUS: {}",
                        active_idx + 1,
                        state.combat_arena.enemies.len(),
                        enemy.id,
                        enemy.name,
                        enemy.health,
                        enemy.max_health,
                        status_str
                    ),
                    20,
                    158,
                    1,
                    color,
                    true,
                );

                state.menu_rasterizer.draw_text(
                    "[Press TAB to Cycle Active Test Creature]",
                    20,
                    178,
                    1,
                    yellow,
                    true,
                );
            }

            let cur_anim_clip = state.sim_core.animation.current_clip();
            let anim_state_name = format!("{:?}", state.sim_core.animation.state());
            state.menu_rasterizer.draw_text(
                &format!(
                    "[JAMES ANIM: {}] CLIP: frames {}..={} @ {:.0}fps | TIME: {:.2}s",
                    anim_state_name,
                    cur_anim_clip.start_frame,
                    cur_anim_clip.end_frame,
                    cur_anim_clip.fps,
                    state.sim_core.animation.clip_time()
                ),
                20,
                625,
                1,
                cyan,
                true,
            );

            if !state.combat_arena.enemies.is_empty() {
                let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
                let enemy = &state.combat_arena.enemies[active_idx];
                let enemy_clip = enemy.anim.current_clip();
                let enemy_anim_state = format!("{:?}", enemy.anim.state());
                state.menu_rasterizer.draw_text(
                    &format!(
                        "[ENEMY ANIM: {}] CLIP: frames {}..={} @ {:.0}fps | TIME: {:.2}s",
                        enemy_anim_state,
                        enemy_clip.start_frame,
                        enemy_clip.end_frame,
                        enemy_clip.fps,
                        enemy.anim.clip_time()
                    ),
                    20,
                    645,
                    1,
                    green,
                    true,
                );
            }

            state.menu_rasterizer.draw_text(
                "[Controls: Right-Click/R: Ready | Space: Attack | 1..6: Weapons | Tab: Cycle Enemy | WASD: Move]",
                20,
                670,
                1,
                muted,
                true,
            );
        }

        // Render active `.dds` In-Engine 3D Cutscene (`DramaDemoPlayer`) telemetry & `.mes` subtitles (`SH2-63`)
        if let Some(ref session) = state.drama_demo_session {
            let p = &session.player;
            let actors_str: Vec<String> = p
                .actors
                .iter()
                .map(|a| {
                    format!(
                        "{}(aid={},vis={})",
                        a.actor_name,
                        a.current_aid,
                        if a.visible { "Y" } else { "N" }
                    )
                })
                .collect();
            state.menu_rasterizer.draw_text(
                &format!(
                    "[IN-ENGINE 3D CUTSCENE #{}: {}] FRAME {:04.0}/{:04} | FOV {:.1}° | ROLL {:.2} rad | [Esc/Enter/G] Skip  [,/.] Prev/Next",
                    p.info.demo_no,
                    p.info.title,
                    p.scene_frame.floor(),
                    p.total_frames(),
                    p.camera.fov_y_radians.to_degrees(),
                    p.camera.roll_radians
                ),
                20,
                575,
                1,
                yellow,
                true,
            );
            state.menu_rasterizer.draw_text(
                &format!(
                    "ACTORS ({}) : {} | VOICE STREAM: {} (START @ {})",
                    p.actors.len(),
                    actors_str.join(", "),
                    p.info.stream_no,
                    p.info.stream_start
                ),
                20,
                595,
                1,
                cyan,
                true,
            );

            if let Some(msg_id) = p.active_subtitle_msg_id {
                let sub_text = session
                    .messages
                    .as_ref()
                    .and_then(|m| m.get_text(msg_id as usize))
                    .map(|s| s.replace('\x0C', "\n"))
                    .unwrap_or_else(|| format!("[Subtitle #{}]", msg_id));
                let cx = state.menu_rasterizer.width / 2;
                let by = state.menu_rasterizer.height.saturating_sub(56);
                sh2_ui::draw_subtitle(
                    &mut state.menu_rasterizer,
                    None,
                    &sub_text,
                    cx,
                    by,
                    28,
                    [240, 240, 240, 255],
                );
            }
        } else {
            let demo_entry =
                &DRAMA_DEMO_CATALOG[state.selected_drama_demo_idx % DRAMA_DEMO_CATALOG.len()];
            state.menu_rasterizer.draw_text(
                &format!(
                    "[3D CUTSCENE READY [{}/{}]: {} (demo_no={}) — Press [G] to Play, [, / .] to Cycle]",
                    state.selected_drama_demo_idx + 1,
                    DRAMA_DEMO_CATALOG.len(),
                    demo_entry.title,
                    demo_entry.demo_no
                ),
                20,
                692,
                1,
                muted,
                true,
            );
        }

        state.finish_hud_overlay(encoder, surface_view);
    }
}
