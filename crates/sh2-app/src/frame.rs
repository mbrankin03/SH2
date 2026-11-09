//! Frame rendering, room hot-swapping, and 2D menu/pause HUD presentation (`redraw_frame`).

use std::time::Instant;

use crate::assets::{
    build_map_texture_bind_groups, find_cld_file_for_room, find_map_file_for_room,
    load_viewer_entry_asset, room_cam_id_for_test_room,
};
use crate::menu_events::apply_pending_engine_config;
use crate::rooms;
use crate::rooms::fmv_room::open_fmv_session;
use crate::state::AppState;
use sh2_renderer::GpuMap;
use sh2_ui::menu::{TestRoomKind, TitlePhase};
use sh2_ui::pause::PauseSubPhase;
use sh2_ui::LoadingScreen;

/// Advances a single frame of simulation, room switching, 3D/FMV rendering, and 2D swapchain presentation.
pub(crate) fn redraw_frame(state: &mut AppState) {
    let now = Instant::now();
    let dt = now.duration_since(state.last_frame);
    state.last_frame = now;
    state.frame_pacer.begin_frame(now);

    // 1. Check and apply committed options configuration
    apply_pending_engine_config(state);

    // 2. Advance menu transitions and loading screen fade state
    tick_menu_and_loading_screen(state, dt.as_secs_f32());

    // 3. Synchronize active test room, model viewer selection, and FMV stream if changed
    sync_active_room_and_viewers(state);

    // 4. Log menu HUD changes when menu revision updates
    if !state.menu_ctrl.in_game_active && state.last_menu_revision != state.menu_ctrl.revision {
        state.last_menu_revision = state.menu_ctrl.revision;
        let current_text = state.menu_ctrl.overlay_text().join(" | ");
        tracing::info!("[SH2 MENU HUD] {}", current_text);
        state.last_menu_state_log = current_text;
    }

    let is_in_game = state.menu_ctrl.in_game_active;
    let is_viewer = is_in_game && state.active_test_room == TestRoomKind::ModelAndTextureViewer;
    let is_grapple_room =
        is_in_game && state.active_test_room == TestRoomKind::GrappleAnimationRoom;
    let is_paused = is_in_game && state.menu_ctrl.pause_menu.is_open;

    // 5. Advance simulation time and frame counter for animated post-processing
    let dt_sec = if is_paused { 0.0 } else { dt.as_secs_f32() };
    if !is_paused {
        state.frame_counter = state.frame_counter.wrapping_add(1);
        state.sim_time += dt_sec;
    }

    let grain_pct = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .grain_intensity;
    let grain_intensity = if is_viewer || is_grapple_room {
        0.0
    } else {
        (grain_pct as f32) / 100.0
    };

    let aspect = state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;

    // 6. Dispatch 3D scene or video pass to the active room controller (advances 60 Hz sim ticks)
    rooms::dispatch_tick_and_render_3d(state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game);

    // 7. Update fog flow velocity time deterministically from elapsed 60 Hz simulation ticks + sub-frame alpha
    let det_fog_time = state
        .sim_core
        .deterministic_fog_time(state.sim_core.interpolation_alpha());
    state.fog_uniforms.time = det_fog_time;
    let mut effective_fog = state.fog_uniforms;
    if let Some(ref session) = state.drama_demo_session {
        if session.player.locks_gameplay_input() {
            if let Some(rgb) = session.player.atmosphere.horizon_fog_rgb_override {
                effective_fog.fog_color[0] = rgb[0];
                effective_fog.fog_color[1] = rgb[1];
                effective_fog.fog_color[2] = rgb[2];
            }
            effective_fog.fog_color[3] *= session.player.atmosphere.fog_density_scale;
        }
    }
    state.render_ctx.update_fog(&effective_fog);

    let (dof_tap_offset_px, dof_strength, motion_blur_strength) = if is_viewer || is_grapple_room {
        ([1.0 / 3.0, 2.0 / 3.0], 0.0, 0.0)
    } else if let Some(session) = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
    {
        let fx = &session.player.special_fx;
        (
            fx.dof_tap_offset_px,
            fx.dof_strength,
            fx.motion_blur_strength,
        )
    } else {
        ([1.0 / 3.0, 2.0 / 3.0], 0.0, 0.0)
    };

    state
        .render_ctx
        .update_post(&sh2_renderer::post::PostUniforms {
            time: det_fog_time,
            grain_intensity,
            exposure: 1.0,
            frame_count: state.frame_counter,
            dof_tap_offset_px,
            dof_strength,
            motion_blur_strength,
        });

    // 8. Present to window surface swapchain
    present_to_surface(state, is_in_game);

    // 9. Enforce zero-jitter frame rate cap (30 / 60 / 120 / 144 / 240 FPS / Uncapped VRR)
    state.frame_pacer.wait_for_deadline();
    state.window.request_redraw();
}

fn tick_menu_and_loading_screen(state: &mut AppState, dt_sec: f32) {
    if state.menu_ctrl.title_sm.phase == TitlePhase::TransitionToGame {
        let min_dur = state
            .menu_ctrl
            .options_menu
            .draft_config
            .debug
            .min_loading_screen_seconds as f32;
        let require_press = state
            .menu_ctrl
            .options_menu
            .draft_config
            .debug
            .require_press_to_continue;
        let instant = state
            .menu_ctrl
            .options_menu
            .draft_config
            .debug
            .instant_room_transitions;

        if instant {
            state.loading_screen = None;
            state.menu_ctrl.tick(dt_sec);
        } else {
            if state.loading_screen.is_none() {
                let label = state.menu_ctrl.title_sm.selected_test_room.label();
                let mut ls = LoadingScreen::new(label, min_dur, require_press);
                ls.mark_completed();
                state.loading_screen = Some(ls);
            }

            if let Some(ref mut ls) = state.loading_screen {
                ls.tick(dt_sec);
                if ls.transition_ready {
                    state.loading_screen = None;
                    state.menu_ctrl.tick(dt_sec);
                }
            }
        }
    } else {
        state.loading_screen = None;
        state.menu_ctrl.tick(dt_sec);
    }
}

fn sync_active_room_and_viewers(state: &mut AppState) {
    // If user transitioned into game from Test Rooms menu, switch level map if needed
    if state.menu_ctrl.in_game_active
        && state.active_test_room != state.menu_ctrl.title_sm.selected_test_room
    {
        let new_room = state.menu_ctrl.title_sm.selected_test_room;
        tracing::info!("Switching test environment to: {:?}", new_room);
        let (new_map, new_pos, new_yaw, new_fog) = find_map_file_for_room(new_room);
        state.gpu_map = GpuMap::from_map(&state.render_ctx.device, &new_map);
        state.map_texture_bind_groups = build_map_texture_bind_groups(&state.render_ctx, &new_map);
        state.fog_uniforms = new_fog;
        state.render_ctx.update_fog(&state.fog_uniforms);
        state.sim_core.collision = find_cld_file_for_room(new_room);
        if let Some(room_id) = room_cam_id_for_test_room(new_room) {
            state
                .sim_core
                .transition_to_room(room_id, new_pos, new_yaw, true);
        } else {
            state.sim_core.camera.set_cam_file(None);
            state.sim_core.snap_player_transform(new_pos, new_yaw);
            state.sim_core.camera.reset_to_player(new_pos, new_yaw);
        }
        state.active_test_room = new_room;
        state.bgm_stage_override = None;
        state.bgm_manager.stop_all(state.audio_engine.as_mut());
        state.drama_demo_session = None;
        if let Some(ref mut eng) = state.audio_engine {
            eng.stop_voice(std::time::Duration::from_millis(30));
        }
        state.selected_drama_demo_idx = match new_room {
            TestRoomKind::RestroomStart => 0,   // `first_toilet` (demo_no = 1)
            TestRoomKind::ObservationDeck => 1, // `tenbou` (demo_no = 2)
            _ => state.selected_drama_demo_idx,
        };
        if new_room == TestRoomKind::CombatArena {
            state.combat_arena = sh2_core::combat::CombatArena::new();
            state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::WoodenPlank;
        }
        if new_room == TestRoomKind::GrappleAnimationRoom {
            state.grapple_viewer.reset_for_current_entry();
        }
        if new_room == TestRoomKind::AudioDebugRoom {
            state.audio_debug = rooms::audio_room::AudioDebugState::default();
        }
        if new_room == TestRoomKind::SurfaceSfxTestRoom {
            state.surface_sfx_room = rooms::surface_sfx_room::SurfaceSfxRoomState::default();
        }
        if new_room == TestRoomKind::InventoryAndItemsRoom {
            state.inventory_room = rooms::inventory_room::InventoryDebugRoomState::default();
        }
        if new_room == TestRoomKind::PuzzleAndCutawayRoom {
            state.puzzle_cutaway_room =
                rooms::puzzle_cutaway_room::PuzzleCutawayRoomState::default();
        }
        if new_room == TestRoomKind::HorrorTriggersRoom {
            state.horror_triggers_room =
                rooms::horror_triggers_room::HorrorTriggersRoomState::default();
        }
        if new_room == TestRoomKind::ModelAndTextureViewer && state.viewer_asset.is_none() {
            let asset = load_viewer_entry_asset(
                &state.render_ctx,
                state.model_viewer.current_entry(),
                state.model_viewer.selected_anm_index,
            );
            if let Some(ref a) = asset {
                state.model_viewer.set_total_frames(a.anm.frames.len());
            }
            state.viewer_asset = asset;
            state.loaded_viewer_model_idx = state.model_viewer.selected_model_index;
            state.loaded_viewer_anm_idx = state.model_viewer.selected_anm_index;
        }
    }

    // Hot-swap model or animation clip in Model & Texture Viewer when selection changes
    if state.menu_ctrl.in_game_active
        && state.active_test_room == TestRoomKind::ModelAndTextureViewer
        && (state.viewer_asset.is_none()
            || state.loaded_viewer_model_idx != state.model_viewer.selected_model_index
            || state.loaded_viewer_anm_idx != state.model_viewer.selected_anm_index)
    {
        let asset = load_viewer_entry_asset(
            &state.render_ctx,
            state.model_viewer.current_entry(),
            state.model_viewer.selected_anm_index,
        );
        if let Some(ref a) = asset {
            state.model_viewer.set_total_frames(a.anm.frames.len());
        }
        state.viewer_asset = asset;
        state.loaded_viewer_model_idx = state.model_viewer.selected_model_index;
        state.loaded_viewer_anm_idx = state.model_viewer.selected_anm_index;
    }

    // Hot-swap or initialize FMV cutscene stream when entering FMV Cutscene Viewer or changing movie / HD preference
    if state.menu_ctrl.in_game_active
        && state.active_test_room == TestRoomKind::FmvCutsceneViewer
        && (state.fmv_session.is_none()
            || state.loaded_fmv_movie_idx != state.fmv_viewer.selected_movie_index
            || state.loaded_fmv_source != state.fmv_viewer.video_source)
    {
        let entry = state.fmv_viewer.current_entry();
        let source_pref = state.fmv_viewer.video_source;
        let audio_pack = state.bgm_manager.audio_pack_source;
        state.fmv_session = open_fmv_session(
            &state.render_ctx.device,
            &state.render_ctx.queue,
            &state.fmv_pipeline,
            &mut state.audio_engine,
            &mut state.fmv_viewer,
            entry,
            source_pref,
            audio_pack,
        );
        state.loaded_fmv_movie_idx = state.fmv_viewer.selected_movie_index;
        state.loaded_fmv_source = state.fmv_viewer.video_source;
        state.fmv_viewer.is_paused = false;
    }
}

fn present_to_surface(state: &mut AppState, is_in_game: bool) {
    let surface_texture = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(wgpu::SurfaceError::Outdated) => {
            state
                .surface
                .configure(&state.render_ctx.device, &state.surface_config);
            return;
        }
        Err(e) => {
            tracing::error!("Surface error: {:?}", e);
            return;
        }
    };

    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder =
        state
            .render_ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Surface Present Encoder"),
            });

    if is_in_game {
        state
            .blitter
            .render_with_bind_group(&mut encoder, &state.blit_bind_group, &surface_view);

        // Render room-specific 2D HUD overlay
        rooms::dispatch_render_hud(state, &mut encoder, &surface_view);

        // If in-game pause menu is active, render authentic pause overlay
        if state.menu_ctrl.pause_menu.is_open {
            state.begin_hud_overlay();
            let font = sh2_ui::AtlasFont::shared(None);

            if state.menu_ctrl.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu {
                sh2_ui::draw_options_screen(
                    &state.menu_ctrl.options_menu,
                    font,
                    &mut state.menu_rasterizer,
                );
            } else {
                sh2_ui::draw_pause_screen(
                    &state.menu_ctrl.pause_menu,
                    font,
                    &mut state.menu_rasterizer,
                );
            }

            state.finish_hud_overlay(&mut encoder, &surface_view);
        }
    } else {
        render_title_menu_overlay(state, &mut encoder, &surface_view);
    }

    state
        .render_ctx
        .queue
        .submit(std::iter::once(encoder.finish()));
    surface_texture.present();
}

fn render_title_menu_overlay(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    let fade_alpha = state.menu_ctrl.title_sm.fade_alpha;
    let phase = state.menu_ctrl.title_sm.phase;

    // Update menu blit tint buffer with current fade alpha
    let tint = [fade_alpha, fade_alpha, fade_alpha, fade_alpha];
    state
        .render_ctx
        .queue
        .write_buffer(&state.menu_uniform_buffer, 0, bytemuck::cast_slice(&tint));

    let bg_to_draw = match phase {
        TitlePhase::SplashIntro => state
            .splash_konami_bg
            .as_ref()
            .or(state.splash_title_bg.as_ref()),
        TitlePhase::OptionsMenu | TitlePhase::TestRoomsSelect | TitlePhase::TransitionToGame => {
            None // Authentic PS2 draws over pure black backdrop
        }
        _ => state
            .splash_title_bg
            .as_ref()
            .or(state.splash_konami_bg.as_ref()),
    };

    if let Some(bg) = bg_to_draw {
        state.blitter.render_with_bind_group_and_load_op(
            encoder,
            bg,
            surface_view,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
    } else {
        let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Menu Clear Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    // Render dynamic 2D text overlay for interactive menus (Main Menu, Scenario Select, Options, Credits)
    if phase != TitlePhase::SplashIntro {
        state.menu_rasterizer.clear();
        let font = sh2_ui::AtlasFont::shared(None);

        match phase {
            TitlePhase::TitleLogo | TitlePhase::MainMenu | TitlePhase::Credits => {
                sh2_ui::draw_title_screen(
                    &state.menu_ctrl.title_sm,
                    state.button_sprites.as_ref(),
                    font,
                    &mut state.menu_rasterizer,
                );
            }
            TitlePhase::ScenarioSelect => {
                sh2_ui::draw_scenario_selection(
                    &state.menu_ctrl.title_sm,
                    state.button_sprites.as_ref(),
                    font,
                    &mut state.menu_rasterizer,
                );
            }
            TitlePhase::ActionDifficultySelect | TitlePhase::RiddleDifficultySelect => {
                sh2_ui::draw_difficulty_selection(
                    &state.menu_ctrl.title_sm,
                    state.button_sprites.as_ref(),
                    font,
                    &mut state.menu_rasterizer,
                );
            }
            TitlePhase::OptionsMenu => {
                sh2_ui::draw_options_screen(
                    &state.menu_ctrl.options_menu,
                    font,
                    &mut state.menu_rasterizer,
                );
            }
            TitlePhase::TestRoomsSelect => {
                sh2_ui::draw_test_rooms_screen(
                    &state.menu_ctrl.title_sm,
                    font,
                    &mut state.menu_rasterizer,
                );
            }
            TitlePhase::TransitionToGame => {
                if let Some(ref ls) = state.loading_screen {
                    ls.render(font, &mut state.menu_rasterizer);
                } else {
                    let label = state.menu_ctrl.title_sm.selected_test_room.label();
                    let center_x = state.menu_rasterizer.width / 2;
                    font.draw_line_centered(
                        &mut state.menu_rasterizer,
                        &format!("LOADING STAGE: {}...", label),
                        (center_x, 500),
                        24,
                        sh2_ui::Sh2TextColor::White,
                        true,
                    );
                }
            }
            _ => {}
        }
        state.finish_hud_overlay(encoder, surface_view);
    }
}
