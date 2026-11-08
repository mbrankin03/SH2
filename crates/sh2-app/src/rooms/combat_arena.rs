//! 3D Stage Exploration & Combat Testing Arena room controller (`SH2-13` / `SH2-57`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::formats::anm::{
    blend_frame_in_place, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into,
};
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_ui::menu::TestRoomKind;

pub(crate) struct CombatArenaRoomController;

impl RoomController for CombatArenaRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
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

                // Play any combat sound effects requested by the combat state machine
                if let Some(sfx_id) = state.combat_arena.player.pending_sfx.take() {
                    tracing::info!("[Combat SFX] Triggered sound effect ID: {}", sfx_id);
                }
            }
        }

        // 2. Compute smooth sub-frame interpolated view matrix for high refresh OLED (60–240 Hz+)
        let alpha = state.sim_core.interpolation_alpha();
        let view = state.sim_core.camera.interpolated_view_matrix(alpha);
        let fov = state.sim_core.camera.interpolated_fov(alpha);
        let proj = glam::Mat4::perspective_rh(fov, aspect, 10.0, 30000.0);
        let view_proj = proj * view;
        let inv_proj = proj.inverse();

        let cam_pos = state
            .sim_core
            .camera
            .prev_snapshot
            .eye
            .lerp(state.sim_core.camera.curr_snapshot.eye, alpha);
        state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);

        // 3. Dynamic Flashlight tracking sub-frame interpolated character position & sweep inertia
        let (player_pos, player_yaw) = state.sim_core.interpolated_player_pose(alpha);
        let (spot_pos, light_dir) = state.sim_core.interpolated_flashlight_ray(alpha);

        let light_color = if state.sim_core.scenario.flashlight_on {
            [2.5, 2.3, 1.8, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        };

        let spot_uniforms = sh2_renderer::light::SpotlightUniforms {
            position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
            direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
            color_outer: [light_color[0], light_color[1], light_color[2], 0.70],
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
                122,
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
                    142,
                    1,
                    color,
                    true,
                );

                state.menu_rasterizer.draw_text(
                    "[Press TAB to Cycle Active Test Creature]",
                    20,
                    162,
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

        state.finish_hud_overlay(encoder, surface_view);
    }
}
