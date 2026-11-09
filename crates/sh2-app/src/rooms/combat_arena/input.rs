//! Keyboard input handling for 3D Stage Exploration & Combat Arena (`SH2-13` / `SH2-57` / `SH2-63`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::assets::open_drama_demo_session;
use crate::state::AppState;
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::DRAMA_DEMO_CATALOG;
use sh2_ui::menu::TestRoomKind;

pub(super) fn handle_combat_arena_key_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) {
    // When an in-engine real-time 3D cutscene (`DramaDemoPlayer`) is active, lock gameplay
    // locomotion and pause menu (`SH2-63`); allow `Escape` / `Enter` to skip (`DramaDemoSkipLast`)
    // or `,` / `.` to cycle the cutscene catalog.
    if state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input())
    {
        handle_cutscene_locked_input(state, key_code, pressed, repeat);
        return;
    }

    match key_code {
        KeyCode::Escape | KeyCode::Backspace if pressed => {
            let stage_name = state.active_test_room.label();
            let can_save = state.menu_ctrl.options_menu.draft_config.game.save_anywhere;
            state
                .menu_ctrl
                .pause_menu
                .open(can_save, stage_name, state.sim_time as f64);
            if let Some(ref mut engine) = state.audio_engine {
                engine.set_bgm_paused(true);
                let _ =
                    play_menu_sound(engine, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
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
        // Authentic Combat controls (Space to Attack, R to Ready, 1..6 to switch weapons)
        KeyCode::KeyR => {
            state.key_ready_held = pressed;
        }
        KeyCode::Space => {
            if pressed && !state.key_attack_held {
                state.key_attack_pressed = true;
            }
            state.key_attack_held = pressed;
        }
        KeyCode::Digit1
        | KeyCode::Digit2
        | KeyCode::Digit3
        | KeyCode::Digit4
        | KeyCode::Digit5
        | KeyCode::Digit6
            if pressed =>
        {
            equip_weapon_shortcut(state, key_code);
        }
        // Teleport / cycle selected enemy right in front of James for testing combat & grab interactions
        KeyCode::Tab if pressed && !state.combat_arena.enemies.is_empty() => {
            cycle_and_teleport_active_enemy(state);
        }
        // Toggle between SH2:EE 48kHz Lossless (`sh2e/sound/`) and Original PS2/PC (`data/sound/`) BGM & SFX packs (`SH2-60` / `SH2-61`)
        KeyCode::KeyB if pressed && !repeat => {
            toggle_audio_pack_source(state);
        }
        // Cycle BGM stage presets (`SeBgmChange` / `SeBgmManager` multi-channel AIX & ADX verification)
        KeyCode::KeyN if pressed && !repeat => {
            cycle_bgm_stage_preset(state);
        }
        // Launch or cycle in-engine real-time 3D cutscene (`DramaDemoPlayer`, `SH2-63`)
        KeyCode::KeyG if pressed && !repeat => {
            if let Some(ref mut eng) = state.audio_engine {
                eng.stop_voice(Duration::from_millis(30));
            }
            state.drama_demo_session =
                open_drama_demo_session(&state.render_ctx, state.selected_drama_demo_idx, false);
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
    if pressed && !repeat && state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held {
        state.struggle_button_press_count += 1;
    }

    update_accumulated_directional_input(state);
}

fn handle_cutscene_locked_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) {
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
                    if let Some((final_pos, final_yaw)) = session.player.player_handoff_transform()
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
                state.drama_demo_session = open_drama_demo_session(
                    &state.render_ctx,
                    state.selected_drama_demo_idx,
                    false,
                );
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
                state.drama_demo_session = open_drama_demo_session(
                    &state.render_ctx,
                    state.selected_drama_demo_idx,
                    false,
                );
            }
            KeyCode::KeyV => {
                if let Some(ref mut session) = state.drama_demo_session {
                    let next_mode = session.player.widescreen_mode.next();
                    let aspect = session.player.viewport_aspect;
                    session.player.set_widescreen_config(aspect, next_mode);
                    tracing::info!(
                        "[Cutscene Widescreen] Switched CutsceneWidescreenMode -> {:?} ({})",
                        next_mode,
                        next_mode.label()
                    );
                }
            }
            _ => {}
        }
    }
}

fn equip_weapon_shortcut(state: &mut AppState, key_code: KeyCode) {
    match key_code {
        KeyCode::Digit1 => {
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::WoodenPlank;
            tracing::info!("[Combat] Equipped Wooden Plank");
        }
        KeyCode::Digit2 => {
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::SteelPipe;
            tracing::info!("[Combat] Equipped Steel Pipe");
        }
        KeyCode::Digit3 => {
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::Handgun;
            state.combat_arena.player.loaded_rounds[0] = 10;
            state.combat_arena.player.spare_rounds[0] = 50;
            tracing::info!("[Combat] Equipped Handgun (10/50)");
        }
        KeyCode::Digit4 => {
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::Shotgun;
            state.combat_arena.player.loaded_rounds[1] = 6;
            state.combat_arena.player.spare_rounds[1] = 30;
            tracing::info!("[Combat] Equipped Shotgun (6/30)");
        }
        KeyCode::Digit5 => {
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::HuntingRifle;
            state.combat_arena.player.loaded_rounds[2] = 4;
            state.combat_arena.player.spare_rounds[2] = 20;
            tracing::info!("[Combat] Equipped Hunting Rifle (4/20)");
        }
        KeyCode::Digit6 => {
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::GreatKnife;
            tracing::info!("[Combat] Equipped Great Knife");
        }
        _ => {}
    }
}

fn cycle_and_teleport_active_enemy(state: &mut AppState) {
    let fwd = glam::Vec3::new(
        state.sim_core.scenario.player_yaw.sin(),
        0.0,
        state.sim_core.scenario.player_yaw.cos(),
    );
    let target_pos = state.combat_arena.player_position + fwd * 350.0;
    let facing_yaw = state.sim_core.scenario.player_yaw + std::f32::consts::PI;

    state.combat_enemy_idx = (state.combat_enemy_idx + 1) % state.combat_arena.enemies.len();
    let target_enemy = &mut state.combat_arena.enemies[state.combat_enemy_idx];

    target_enemy.snap_pose(target_pos, facing_yaw);
    target_enemy.is_aggroed = true;
    target_enemy.flinch_timer = 0.0;
    target_enemy.attack_cooldown = 0.2;

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

fn toggle_audio_pack_source(state: &mut AppState) {
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

fn cycle_bgm_stage_preset(state: &mut AppState) {
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

fn update_accumulated_directional_input(state: &mut AppState) {
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
