//! Frame rendering, room hot-swapping, and 2D menu/pause HUD presentation (`redraw_frame`).

use std::time::Instant;

use crate::assets::{
    find_cld_file_for_room, find_map_file_for_room, find_sh2e_map_file_for_room,
    find_vanilla_map_file_for_room, load_viewer_entry_asset, open_drama_demo_session,
    room_cam_id_for_test_room,
};
use crate::menu_events::apply_pending_engine_config;
use crate::rooms;
use crate::rooms::fmv_room::open_fmv_session;
use crate::state::AppState;
use sh2_ui::menu::{LaunchTarget, TestRoomKind, TitlePhase};
use sh2_ui::pause::PauseSubPhase;

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

    // 3. Synchronize active test room, model viewer selection, FMV stream, and camera cursor grab
    sync_active_room_and_viewers(state);
    state.sync_cursor_grab_for_camera();

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
    let is_loading_test = is_in_game && state.active_test_room == TestRoomKind::LoadingScreenTest;
    let is_paused =
        is_in_game && (state.menu_ctrl.pause_menu.is_open || state.ingame_menus.is_any_open());

    if is_in_game && state.ingame_menus.is_any_open() {
        let snapshot = crate::ingame_menus::build_player_snapshot(state);
        state.ingame_menus.tick(dt.as_secs_f32(), &snapshot);
    }

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
    let grain_intensity = if is_viewer || is_grapple_room || is_loading_test {
        0.0
    } else {
        (grain_pct as f32) / 100.0
    };

    let aspect = state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;

    let shadow_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .shadow_quality;
    state.render_ctx.set_soft_shadow_blur_enabled(
        shadow_quality != sh2_core::config::ShadowQualityMode::NativePcss,
    );

    // 6. Dispatch 3D scene or video pass to the active room controller (advances 60 Hz sim ticks)
    let is_attract_fmv = !is_in_game
        && state.menu_ctrl.title_sm.attract_movie_requested
        && !state.menu_ctrl.title_sm.is_fading_out;
    if is_attract_fmv {
        if state.fmv_session.is_none() {
            if let Some((idx, entry)) = sh2_debug::FMV_CATALOG
                .iter()
                .enumerate()
                .find(|(_, m)| m.id.eq_ignore_ascii_case("open"))
            {
                state.fmv_viewer.selected_movie_index = idx;
                state.loaded_fmv_movie_idx = idx;
                let source = state.fmv_viewer.video_source;
                let audio_pack = state.bgm_manager.audio_pack_source;
                state.fmv_session = open_fmv_session(
                    &state.render_ctx.device,
                    &state.render_ctx.queue,
                    &state.fmv_pipeline,
                    &mut state.audio_engine,
                    &mut state.fmv_viewer,
                    entry,
                    source,
                    audio_pack,
                );
            }
        }
        let eof =
            rooms::fmv_room::tick_and_render_fmv_frame(state, dt_sec, aspect, grain_pct, false);
        if eof || state.fmv_session.is_none() {
            state.menu_ctrl.title_sm.attract_movie_requested = false;
            state.menu_ctrl.title_sm.phase = TitlePhase::TitleLogo;
            state.menu_ctrl.title_sm.start_fade_in();
        }
    } else {
        rooms::dispatch_tick_and_render_3d(
            state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
        );
    }

    // 7. Update fog flow velocity time and proximity particles deterministically from 60 Hz sim ticks
    let alpha = state.sim_core.interpolation_alpha();
    rooms::combat_arena::render::update_atmosphere_fog_uniforms(state, alpha);
    let det_fog_time = state.fog_uniforms.time;

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

    let aces_enabled = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .aces_tonemapping;
    let brightness_level = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .brightness_level;
    let mut post_flags = sh2_renderer::post::POST_FLAG_PS2_LCG_NOISE;
    if aces_enabled {
        post_flags |= sh2_renderer::post::POST_FLAG_ACES_TONEMAP;
    }

    state
        .render_ctx
        .update_post(&sh2_renderer::post::PostUniforms {
            time: det_fog_time,
            grain_intensity,
            exposure: sh2_renderer::post::ps2_brightness_scale(brightness_level),
            frame_count: state.frame_counter,
            dof_tap_offset_px,
            dof_strength,
            motion_blur_strength,
            flags: post_flags,
            ..sh2_renderer::post::PostUniforms::default()
        });

    // 8. Present to window surface swapchain
    present_to_surface(state, is_in_game);

    // 9. Enforce zero-jitter frame rate cap (30 / 60 / 120 / 144 / 240 FPS / Uncapped VRR)
    state.frame_pacer.wait_for_deadline();
    state.window.request_redraw();
}

/// Plays a single synchronized footstep SFX for a spawned loading-screen footprint (`SH2-75`).
pub(crate) fn trigger_loading_footstep_sfx(
    state: &mut AppState,
    material: sh2_core::SurfaceMaterial,
) {
    if let Some(ref mut engine) = state.audio_engine {
        if let Some(ref mut bank) = state.audio_bank {
            let _ = bank.play_footstep(engine, state.active_maria, material, false, 0.85);
        } else {
            let _ = sh2_audio::play_menu_sound(engine, None, sh2_audio::SoundCueKind::Footstep);
        }
    }
}

fn tick_menu_and_loading_screen(state: &mut AppState, dt_sec: f32) {
    let step = state
        .menu_ctrl
        .tick_with_loading(&mut state.loading_screen, dt_sec);

    // 1:1 Retail Parity (`SLUS_202.28` `now_loading.c` & `sh2pc.exe` `FUN_00406be0` /
    // `FUN_004c8bc0`): neither PS2 nor PC retail plays footstep SFX during loading screens.
    let _ = step.footstep_spawned;

    if step.needs_room_preload {
        preload_launch_target_for_loading_screen(state);
        if let Some(ref mut ls) = state.loading_screen {
            ls.mark_completed();
        }
    }
}

/// Preloads target room `.map`, `.cld`, `.cam`, and GPU bind groups while the loading screen is
/// visible (`SH2-75`), eliminating the post-loading-screen freeze.
fn preload_launch_target_for_loading_screen(state: &mut AppState) {
    match state.menu_ctrl.title_sm.launch_target {
        LaunchTarget::Campaign { is_continue, .. } => {
            let requested_target = state.menu_ctrl.title_sm.launch_target;
            if state.active_launch_target != requested_target
                || state.campaign_orchestrator.is_none()
            {
                state.active_launch_target = requested_target;
                rooms::campaign_room::CampaignRoomController::start_session(state, is_continue);
            }
        }
        LaunchTarget::TestRoom(room) => {
            if state.active_test_room != room {
                load_test_room_into_state(state, room);
            }
        }
    }
}

fn sync_active_room_and_viewers(state: &mut AppState) {
    if state.menu_ctrl.in_game_active {
        let requested_target = state.menu_ctrl.title_sm.launch_target;
        if let LaunchTarget::Campaign { is_continue, .. } = requested_target {
            if state.active_launch_target != requested_target
                || state.campaign_orchestrator.is_none()
            {
                state.active_launch_target = requested_target;
                rooms::campaign_room::CampaignRoomController::start_session(state, is_continue);
            }
            return;
        }
        state.active_launch_target = requested_target;
    }

    // If user transitioned into game from Test Rooms menu (e.g. instant cuts), switch level map if needed
    if state.menu_ctrl.in_game_active
        && state.active_test_room != state.menu_ctrl.title_sm.selected_test_room
    {
        let new_room = state.menu_ctrl.title_sm.selected_test_room;
        load_test_room_into_state(state, new_room);
    }

    // Hot-swap model or animation clip in Model & Texture Viewer when selection changes
    if state.menu_ctrl.in_game_active
        && state.active_test_room == TestRoomKind::ModelAndTextureViewer
        && (state.viewer_asset.is_none()
            || state.loaded_viewer_model_idx != state.model_viewer.selected_model_index
            || state.loaded_viewer_anm_idx != state.model_viewer.selected_anm_index)
    {
        let mut asset = load_viewer_entry_asset(
            &state.render_ctx,
            state.model_viewer.current_entry(),
            state.model_viewer.selected_anm_index,
        );
        if let Some(ref mut a) = asset {
            state.model_viewer.set_total_frames(a.anm.frames.len());
            let presets = sh2_renderer::discover_pose_presets(&a.model);
            if let Some(preset) = presets.get(state.model_viewer.selected_pose_preset_index) {
                a.gpu_mesh.apply_pose_preset(preset);
            }
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

/// Loads a [`TestRoomKind`]'s `.map`, `.cld`, `.cam`, GPU bind groups, and room-specific state.
fn load_test_room_into_state(state: &mut AppState, new_room: TestRoomKind) {
    tracing::info!("Switching test environment to: {:?}", new_room);
    let (new_map, new_pos, new_yaw, new_fog) = find_map_file_for_room(new_room);
    state.map_hd_assets = crate::assets::MapTierAssets::from_map(&state.render_ctx, &new_map);
    state.map_sh2e_assets = find_sh2e_map_file_for_room(new_room)
        .map(|sh2e_map| crate::assets::MapTierAssets::from_map(&state.render_ctx, &sh2e_map));
    state.map_vanilla_assets = find_vanilla_map_file_for_room(new_room)
        .map(|van_map| crate::assets::MapTierAssets::from_map(&state.render_ctx, &van_map));
    let active_tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;
    state.apply_texture_quality(active_tex_quality);
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
        state.sim_core.reset_cameras_to_player(new_pos, new_yaw);
    }
    let prev_room_key = format!("{:?}", state.active_test_room);
    let next_room_key = format!("{new_room:?}");
    let persistent_blood = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .enable_persistent_blood_decals;
    state
        .blood_system
        .on_room_change(&prev_room_key, &next_room_key, persistent_blood);
    state.active_test_room = new_room;
    rooms::combat_arena::story_progression_3d::sync_checkpoint_with_room_entry(state, new_room);
    state.bgm_stage_override = None;
    state.bgm_manager.stop_all(state.audio_engine.as_mut());
    state.se_pool.reset();
    crate::audio_dispatch::sync_se_pool_3d_bank(state);
    if let Some(ref mut bank) = state.audio_bank {
        bank.stop_all_spatial_handles();
        bank.stop_active_cut_audio();
    }
    state.drama_demo_session = None;
    if let Some(ref mut eng) = state.audio_engine {
        eng.stop_voice(std::time::Duration::from_millis(30));
    }
    state.selected_drama_demo_idx = match new_room {
        TestRoomKind::RestroomStart => 0,   // `first_toilet` (demo_no = 1)
        TestRoomKind::ObservationDeck => 1, // `tenbou` (demo_no = 2)
        _ => state.selected_drama_demo_idx,
    };
    if new_room == TestRoomKind::RestroomStart
        && state.story_progression_room.checkpoint_3d_idx == 0
        && !state.story_progression_room.restroom_mirror_inspected
    {
        state.drama_demo_session = open_drama_demo_session(&state.render_ctx, 0, false);
        state.apply_texture_quality(active_tex_quality);
    }
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
        state.puzzle_cutaway_room = rooms::puzzle_cutaway_room::PuzzleCutawayRoomState::default();
    }
    if new_room == TestRoomKind::HorrorTriggersRoom {
        state.horror_triggers_room =
            rooms::horror_triggers_room::HorrorTriggersRoomState::default();
    }
    if new_room == TestRoomKind::StoryStagesAndEndingsRoom {
        state.story_progression_room =
            rooms::story_progression_room::StoryProgressionRoomState::default();
    }
    if new_room == TestRoomKind::LoadingScreenTest {
        state.loading_screen_test = rooms::loading_screen_room::LoadingScreenTestState::default();
    }
    if new_room == TestRoomKind::ModelAndTextureViewer && state.viewer_asset.is_none() {
        let mut asset = load_viewer_entry_asset(
            &state.render_ctx,
            state.model_viewer.current_entry(),
            state.model_viewer.selected_anm_index,
        );
        if let Some(ref mut a) = asset {
            state.model_viewer.set_total_frames(a.anm.frames.len());
            let presets = sh2_renderer::discover_pose_presets(&a.model);
            if let Some(preset) = presets.get(state.model_viewer.selected_pose_preset_index) {
                a.gpu_mesh.apply_pose_preset(preset);
            }
        }
        state.viewer_asset = asset;
        state.loaded_viewer_model_idx = state.model_viewer.selected_model_index;
        state.loaded_viewer_anm_idx = state.model_viewer.selected_anm_index;
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
        // 1:1 PS2 Status/Inventory (`look_hp` @ `0x001d9660`): pure black screen with the live
        // frozen 3D gameplay snapshot blitted strictly into the top-left `Status` box (omitted
        // during full-screen `ExamineOverlay` so nothing bleeds behind the examined item).
        if state.ingame_menus.inventory.is_open && !state.menu_ctrl.pause_menu.is_open {
            if state.ingame_menus.inventory.is_fullscreen_examine() {
                let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Inventory Examine Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
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
            } else {
                let vp = sh2_ui::inventory::HealthEcgState::snapshot_viewport_rect(
                    state.surface_config.width as f32,
                    state.surface_config.height as f32,
                );
                state.blitter.render_with_bind_group_in_viewport(
                    &mut encoder,
                    &state.blit_bind_group,
                    &surface_view,
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    vp,
                );
            }
        } else {
            state.blitter.render_with_bind_group(
                &mut encoder,
                &state.blit_bind_group,
                &surface_view,
            );
        }

        // Render room-specific 2D HUD overlay (suppressed while single-screen menus or pause are open)
        if !state.ingame_menus.is_any_open() && !state.menu_ctrl.pause_menu.is_open {
            rooms::dispatch_render_hud(state, &mut encoder, &surface_view);
        }

        // If single-screen Status/Inventory, Paper Map, or Memo Reader is active, render it
        if state.ingame_menus.is_any_open() && !state.menu_ctrl.pause_menu.is_open {
            crate::ingame_menus::render_overlay(state, &mut encoder, &surface_view);
        }

        // If in-game pause menu is active, dim the full widescreen window edge-to-edge, then
        // render the crisp 1920x1440 4:3 Pause/Options UI centered on top with 1:1 square pixels.
        if state.menu_ctrl.pause_menu.is_open {
            state.draw_fullscreen_dim_scrim(&mut encoder, &surface_view, 165);
            state.inventory_rasterizer.clear();
            let font = sh2_ui::AtlasFont::shared(None);

            if state.menu_ctrl.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu {
                sh2_ui::draw_options_screen(
                    &state.menu_ctrl.options_menu,
                    font,
                    &mut state.inventory_rasterizer,
                );
            } else {
                sh2_ui::pause::draw_pause_screen_with_scrim(
                    &state.menu_ctrl.pause_menu,
                    font,
                    &mut state.inventory_rasterizer,
                    false,
                );
            }

            let vp = state.ps2_ui_viewport();
            if let Some((mx, my)) = state.last_cursor_pos {
                let norm_x = (mx as f32 - vp.0) / vp.2.max(1.0);
                let norm_y = (my as f32 - vp.1) / vp.3.max(1.0);
                sh2_ui::menu::draw_pc_mouse_cursor(&mut state.inventory_rasterizer, norm_x, norm_y);
            }
            state.finish_inventory_overlay_in_viewport(&mut encoder, &surface_view, vp);
        }
    } else if state.menu_ctrl.title_sm.attract_movie_requested
        && !state.menu_ctrl.title_sm.is_fading_out
        && state.fmv_session.is_some()
    {
        state
            .blitter
            .render_with_bind_group(&mut encoder, &state.blit_bind_group, &surface_view);
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

    // Update menu blit tint buffer: full brightness (`[1.0, 1.0, 1.0, ls.fade_alpha]`) while
    // rendering `LoadingScreen` so it is never dimmed by `title_sm.fade_alpha` (`SH2-75`).
    let tint = if phase == TitlePhase::TransitionToGame {
        if let Some(ref ls) = state.loading_screen {
            [1.0, 1.0, 1.0, ls.fade_alpha]
        } else {
            [fade_alpha, fade_alpha, fade_alpha, fade_alpha]
        }
    } else {
        [fade_alpha, fade_alpha, fade_alpha, fade_alpha]
    };
    state
        .render_ctx
        .queue
        .write_buffer(&state.menu_uniform_buffer, 0, bytemuck::cast_slice(&tint));

    let enhanced_title = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .enable_enhanced_title_screen;
    let active_title_bg = if enhanced_title {
        state
            .splash_title_bg
            .as_ref()
            .or(state.splash_title_bg_original.as_ref())
    } else {
        state
            .splash_title_bg_original
            .as_ref()
            .or(state.splash_title_bg.as_ref())
    };
    let has_active_title_bg = active_title_bg.is_some();
    let bg_to_draw = match phase {
        TitlePhase::SplashIntro => state
            .menu_ctrl
            .title_sm
            .boot_sequence
            .current_logo()
            .and_then(|logo| {
                let idx = match logo {
                    sh2_ui::menu::BootLogoKind::CesaWarning => 0,
                    sh2_ui::menu::BootLogoKind::SceWarning => 1,
                    sh2_ui::menu::BootLogoKind::KonamiLogo => 2,
                    sh2_ui::menu::BootLogoKind::SplashLogo => 3,
                };
                state.boot_logo_bgs[idx].as_ref()
            }),
        TitlePhase::OptionsMenu | TitlePhase::TestRoomsSelect | TitlePhase::TransitionToGame => {
            None // Authentic PS2 draws over pure black backdrop
        }
        _ => active_title_bg.or(state.splash_konami_bg.as_ref()),
    };

    let ui_vp = state.ps2_ui_viewport();
    let win_w = state.surface_config.width.max(1) as f32;
    let win_h = state.surface_config.height.max(1) as f32;
    // Aspect-preserving UV crop for `sh2e/pic/etc/start00.tex` (`2732×2048` = 4:3) so it fills
    // 4:3, 16:9, and 21:9 natively without horizontal stretching or black bars.
    const TEX_ASPECT: f32 = 2732.0 / 2048.0;
    let win_aspect = win_w / win_h;
    let (u0, v0, u1, v1) = if win_aspect >= TEX_ASPECT {
        let clamped_aspect = win_aspect.min(2.15);
        let v_span = (TEX_ASPECT / clamped_aspect).clamp(0.62, 1.0);
        let pad = (1.0 - v_span) * 0.5;
        (0.0f32, pad, 1.0f32, 1.0 - pad)
    } else {
        let u_span = (win_aspect / TEX_ASPECT).clamp(0.45, 1.0);
        let pad = (1.0 - u_span) * 0.5;
        (pad, 0.0f32, 1.0 - pad, 1.0f32)
    };
    let bg_uniforms = [fade_alpha, fade_alpha, fade_alpha, 1.0, u0, v0, u1, v1];
    state.render_ctx.queue.write_buffer(
        &state.title_bg_uniform_buffer,
        0,
        bytemuck::cast_slice(&bg_uniforms),
    );

    let bg_w = if win_aspect > 2.15 {
        win_h * 2.15
    } else {
        win_w
    };
    let bg_x = (win_w - bg_w) * 0.5;
    let bg_vp = (bg_x, 0.0, bg_w, win_h);

    if let Some(bg) = bg_to_draw {
        if phase == TitlePhase::SplashIntro {
            state.blitter.render_with_bind_group_in_viewport(
                encoder,
                bg,
                surface_view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                ui_vp,
            );
        } else {
            state.blitter.render_with_bind_group_in_viewport(
                encoder,
                bg,
                surface_view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                bg_vp,
            );
        }
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

    // Render dynamic 2D overlay at 1920x1440 (exact 4:3) into centered `ps2_ui_viewport()`
    // so Title, Difficulty, Options, Credits, ExitConfirm, and Loading stay 4K-sharp with zero stretch on any monitor.
    // During `SplashIntro`, only render the 2D fallback text if the `.tex` logo is unavailable on disk.
    if phase != TitlePhase::SplashIntro || bg_to_draw.is_none() {
        if phase == TitlePhase::ExitConfirm {
            state.draw_fullscreen_dim_scrim(encoder, surface_view, 165);
        }
        state.inventory_rasterizer.clear();
        let font = sh2_ui::AtlasFont::shared(None);

        match phase {
            TitlePhase::TitleLogo
            | TitlePhase::MainMenu
            | TitlePhase::Credits
            | TitlePhase::ExitConfirm => {
                if has_active_title_bg
                    && enhanced_title
                    && matches!(phase, TitlePhase::TitleLogo | TitlePhase::MainMenu)
                {
                    if let Some(ref atlas) = state.button_sprites {
                        sh2_ui::menu::draw_pulsing_title_logo_with_style(
                            &state.menu_ctrl.title_sm,
                            atlas,
                            &mut state.inventory_rasterizer,
                            (u0, v0, u1, v1),
                            bg_vp,
                            ui_vp,
                            true,
                        );
                    }
                }
                sh2_ui::draw_title_screen(
                    &state.menu_ctrl.title_sm,
                    state.button_sprites.as_ref(),
                    font,
                    &mut state.inventory_rasterizer,
                );
            }
            TitlePhase::ScenarioSelect => {
                if has_active_title_bg && enhanced_title {
                    if let Some(ref atlas) = state.button_sprites {
                        sh2_ui::menu::draw_pulsing_title_logo_with_style(
                            &state.menu_ctrl.title_sm,
                            atlas,
                            &mut state.inventory_rasterizer,
                            (u0, v0, u1, v1),
                            bg_vp,
                            ui_vp,
                            true,
                        );
                    }
                }
                sh2_ui::draw_scenario_selection(
                    &state.menu_ctrl.title_sm,
                    state.button_sprites.as_ref(),
                    font,
                    &mut state.inventory_rasterizer,
                );
            }
            TitlePhase::ActionDifficultySelect | TitlePhase::RiddleDifficultySelect => {
                if has_active_title_bg && enhanced_title {
                    if let Some(ref atlas) = state.button_sprites {
                        sh2_ui::menu::draw_pulsing_title_logo_with_style(
                            &state.menu_ctrl.title_sm,
                            atlas,
                            &mut state.inventory_rasterizer,
                            (u0, v0, u1, v1),
                            bg_vp,
                            ui_vp,
                            true,
                        );
                    }
                }
                sh2_ui::draw_difficulty_selection(
                    &state.menu_ctrl.title_sm,
                    state.button_sprites.as_ref(),
                    font,
                    &mut state.inventory_rasterizer,
                );
            }
            TitlePhase::OptionsMenu => {
                sh2_ui::draw_options_screen(
                    &state.menu_ctrl.options_menu,
                    font,
                    &mut state.inventory_rasterizer,
                );
            }
            TitlePhase::TestRoomsSelect => {
                sh2_ui::draw_test_rooms_screen(
                    &state.menu_ctrl.title_sm,
                    font,
                    &mut state.inventory_rasterizer,
                );
            }
            TitlePhase::TransitionToGame => {
                let quality = state
                    .menu_ctrl
                    .options_menu
                    .original_config
                    .video
                    .texture_quality;
                let use_hd_footmark = quality != sh2_core::WorldTextureQuality::OriginalPs2;
                let use_4k_footmark = quality == sh2_core::WorldTextureQuality::Ai4kMaster;
                if let Some(ref mut ls) = state.loading_screen {
                    ls.use_hd_footmark = use_hd_footmark;
                    ls.use_4k_footmark = use_4k_footmark;
                    ls.render(font, &mut state.inventory_rasterizer);
                } else if !matches!(
                    state.menu_ctrl.title_sm.launch_target,
                    sh2_ui::menu::LaunchTarget::Campaign {
                        is_continue: false,
                        ..
                    }
                ) {
                    let label = state.menu_ctrl.title_sm.selected_test_room.label();
                    let center_x = state.inventory_rasterizer.width / 2;
                    let center_y = (state.inventory_rasterizer.height * 7) / 10;
                    let font_sz = (state.inventory_rasterizer.height / 30).max(16);
                    font.draw_line_centered(
                        &mut state.inventory_rasterizer,
                        &format!("LOADING STAGE: {}...", label),
                        (center_x, center_y),
                        font_sz,
                        sh2_ui::Sh2TextColor::White,
                        true,
                    );
                }
            }
            _ => {}
        }
        if let Some((mx, my)) = state.last_cursor_pos {
            let norm_x = (mx as f32 - ui_vp.0) / ui_vp.2.max(1.0);
            let norm_y = (my as f32 - ui_vp.1) / ui_vp.3.max(1.0);
            sh2_ui::menu::draw_pc_mouse_cursor(&mut state.inventory_rasterizer, norm_x, norm_y);
        }
        state.finish_inventory_overlay_in_viewport(encoder, surface_view, ui_vp);
        let fade = state.menu_ctrl.title_sm.fade_alpha.clamp(0.0, 1.0);
        if fade < 0.999 && phase != TitlePhase::TransitionToGame {
            let scrim_alpha = ((1.0 - fade) * 255.0).round() as u8;
            if scrim_alpha > 0 {
                state.draw_fullscreen_dim_scrim(encoder, surface_view, scrim_alpha);
            }
        }
    }
}
