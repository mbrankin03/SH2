//! Menu navigation, pause screen routing, and live options configuration application.

use winit::keyboard::KeyCode;

use crate::state::AppState;
use sh2_audio::{play_menu_sound, AudioEngine, MenuAudioBank, SoundCueKind};
use sh2_core::config::AppConfig;
use sh2_ui::menu::{MenuNavAction, MenuSoundCue, TitlePhase};
use sh2_ui::pause::PauseAction;

/// Emits a menu navigation sound effect through the active `AudioEngine`.
pub(crate) fn emit_menu_sound(state: &mut AppState, sound: MenuSoundCue, log_prefix: &str) {
    if sound == MenuSoundCue::None {
        return;
    }
    tracing::info!("{} sound emitted: {:?}", log_prefix, sound);
    if let Some(ref mut engine) = state.audio_engine {
        let cue_kind = match sound {
            MenuSoundCue::CursorMove => Some(SoundCueKind::CursorMove),
            MenuSoundCue::Confirm => Some(SoundCueKind::Confirm),
            MenuSoundCue::Cancel => Some(SoundCueKind::Cancel),
            MenuSoundCue::GameStartTransition => Some(SoundCueKind::GameStartTransition),
            MenuSoundCue::None => None,
        };
        if let Some(kind) = cue_kind {
            let _ = play_menu_sound(engine, state.audio_bank.as_ref(), kind);
        }
    }
}

/// Resolves [`sh2_core::FmvCutsceneSource`] to a concrete [`sh2_debug::FmvVideoSource`].
pub(crate) fn resolve_fmv_video_source(
    pref: sh2_core::FmvCutsceneSource,
    has_4k_vsr_pack: bool,
) -> sh2_debug::FmvVideoSource {
    match pref {
        sh2_core::FmvCutsceneSource::Auto => {
            if has_4k_vsr_pack {
                sh2_debug::FmvVideoSource::Upscaled4kMp4
            } else {
                sh2_debug::FmvVideoSource::Enhanced1080p60
            }
        }
        sh2_core::FmvCutsceneSource::Upscaled4kMp4 => sh2_debug::FmvVideoSource::Upscaled4kMp4,
        sh2_core::FmvCutsceneSource::Enhanced1080p60 => sh2_debug::FmvVideoSource::Enhanced1080p60,
        sh2_core::FmvCutsceneSource::OgHqPs2Masters => sh2_debug::FmvVideoSource::OgHqPs2Masters,
        sh2_core::FmvCutsceneSource::VanillaPc => sh2_debug::FmvVideoSource::VanillaPc,
    }
}

/// Applies audio volume levels and audio pack source hot-swapping from a committed `AppConfig`.
pub(crate) fn sync_audio_config(state: &mut AppState, cfg: &AppConfig) {
    if let Some(ref mut audio) = state.audio_engine {
        audio.set_master_volume(cfg.audio.master_volume);
        audio.set_bgm_volume(cfg.audio.bgm_volume);
        audio.set_sfx_volume(cfg.audio.sfx_volume);
        audio.set_voice_volume(cfg.audio.dialogue_volume);
        audio.set_spatial_audio_mode(cfg.audio.spatial_audio_mode);
        audio.set_acoustic_occlusion_enabled(cfg.audio.enable_acoustic_occlusion);
        audio.set_dynamic_room_acoustics_enabled(cfg.audio.enable_dynamic_room_acoustics);
    }
    state.fmv_viewer.enable_restored_cut_audio = cfg.audio.enable_restored_cut_audio;
    let resolver = sh2_assets::AssetPathResolver::resolve();
    state.fmv_viewer.video_source =
        resolve_fmv_video_source(cfg.video.fmv_source, resolver.has_4k_vsr_fmv_pack());
    state.fmv_viewer.prefer_hd_source = !matches!(
        state.fmv_viewer.video_source,
        sh2_debug::FmvVideoSource::VanillaPc
    );
    state.bgm_manager.event_flags.enable_restored_cut_audio = cfg.audio.enable_restored_cut_audio;
    let resolved_pack = state
        .menu_ctrl
        .audio_pack_status
        .resolve_source(cfg.audio.audio_pack_source);
    if state.bgm_manager.audio_pack_source != resolved_pack {
        let _ = state
            .bgm_manager
            .set_audio_pack_source(resolved_pack, state.audio_engine.as_mut());
        let resolver = sh2_assets::AssetPathResolver::resolve();
        state.audio_bank = Some(MenuAudioBank::load_from_dirs_for_source(
            &resolver.data_dir,
            resolver.sh2e_dir.as_deref(),
            resolved_pack,
        ));
    }
}

/// Handles keyboard input when the Title Screen / Main Menu is active (`!state.menu_ctrl.in_game_active`).
pub(crate) fn handle_title_menu_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool) {
    if !pressed {
        return;
    }

    // Intercept Confirm button if loading screen is active
    if state.menu_ctrl.title_sm.phase == TitlePhase::TransitionToGame
        && matches!(
            key_code,
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space
        )
    {
        if let Some(ref mut ls) = state.loading_screen {
            if ls.handle_confirm() {
                tracing::info!("Loading Screen: Player pressed continue.");
                if let Some(ref mut engine) = state.audio_engine {
                    let _ =
                        play_menu_sound(engine, state.audio_bank.as_ref(), SoundCueKind::Confirm);
                }
            }
            return;
        }
    }

    let sound = match key_code {
        KeyCode::KeyW | KeyCode::ArrowUp => state.menu_ctrl.handle_nav(MenuNavAction::Up),
        KeyCode::KeyS | KeyCode::ArrowDown => state.menu_ctrl.handle_nav(MenuNavAction::Down),
        KeyCode::KeyA | KeyCode::ArrowLeft => state.menu_ctrl.handle_nav(MenuNavAction::Left),
        KeyCode::KeyD | KeyCode::ArrowRight => state.menu_ctrl.handle_nav(MenuNavAction::Right),
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
            state.menu_ctrl.handle_nav(MenuNavAction::Confirm)
        }
        KeyCode::Escape | KeyCode::Backspace => state.menu_ctrl.handle_nav(MenuNavAction::Cancel),
        KeyCode::KeyQ => state.menu_ctrl.switch_options_tab(false),
        KeyCode::KeyE => state.menu_ctrl.switch_options_tab(true),
        KeyCode::PageUp | KeyCode::BracketLeft
            if state.menu_ctrl.title_sm.phase == TitlePhase::OptionsMenu
                && state.menu_ctrl.options_menu.active_tab == sh2_ui::OptionsTab::Mods =>
        {
            if state.menu_ctrl.options_menu.move_selected_mod(-1)
                == sh2_ui::OptionsSoundCue::ValueChange
            {
                MenuSoundCue::CursorMove
            } else {
                MenuSoundCue::None
            }
        }
        KeyCode::PageDown | KeyCode::BracketRight
            if state.menu_ctrl.title_sm.phase == TitlePhase::OptionsMenu
                && state.menu_ctrl.options_menu.active_tab == sh2_ui::OptionsTab::Mods =>
        {
            if state.menu_ctrl.options_menu.move_selected_mod(1)
                == sh2_ui::OptionsSoundCue::ValueChange
            {
                MenuSoundCue::CursorMove
            } else {
                MenuSoundCue::None
            }
        }
        _ => match state.menu_ctrl.title_sm.phase {
            TitlePhase::SplashIntro | TitlePhase::TitleLogo => {
                state.menu_ctrl.handle_nav(MenuNavAction::Confirm)
            }
            _ => MenuSoundCue::None,
        },
    };

    if let Some(cfg) = state.menu_ctrl.config_applied_pending.clone() {
        sync_audio_config(state, &cfg);
    }
    emit_menu_sound(state, sound, "Menu");
}

/// Handles keyboard input when the in-game Pause Menu is open (`state.menu_ctrl.pause_menu.is_open`).
pub(crate) fn handle_pause_menu_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool) {
    if !pressed {
        return;
    }

    let (sound, action) = match key_code {
        KeyCode::KeyW | KeyCode::ArrowUp => state.menu_ctrl.handle_pause_nav(MenuNavAction::Up),
        KeyCode::KeyS | KeyCode::ArrowDown => state.menu_ctrl.handle_pause_nav(MenuNavAction::Down),
        KeyCode::KeyA | KeyCode::ArrowLeft => state.menu_ctrl.handle_pause_nav(MenuNavAction::Left),
        KeyCode::KeyD | KeyCode::ArrowRight => {
            state.menu_ctrl.handle_pause_nav(MenuNavAction::Right)
        }
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
            state.menu_ctrl.handle_pause_nav(MenuNavAction::Confirm)
        }
        KeyCode::Escape | KeyCode::Backspace => {
            state.menu_ctrl.handle_pause_nav(MenuNavAction::Cancel)
        }
        KeyCode::PageUp | KeyCode::BracketLeft
            if state.menu_ctrl.pause_menu.sub_phase == sh2_ui::PauseSubPhase::InOptionsMenu
                && state.menu_ctrl.options_menu.active_tab == sh2_ui::OptionsTab::Mods =>
        {
            let cue = if state.menu_ctrl.options_menu.move_selected_mod(-1)
                == sh2_ui::OptionsSoundCue::ValueChange
            {
                MenuSoundCue::CursorMove
            } else {
                MenuSoundCue::None
            };
            (cue, PauseAction::None)
        }
        KeyCode::PageDown | KeyCode::BracketRight
            if state.menu_ctrl.pause_menu.sub_phase == sh2_ui::PauseSubPhase::InOptionsMenu
                && state.menu_ctrl.options_menu.active_tab == sh2_ui::OptionsTab::Mods =>
        {
            let cue = if state.menu_ctrl.options_menu.move_selected_mod(1)
                == sh2_ui::OptionsSoundCue::ValueChange
            {
                MenuSoundCue::CursorMove
            } else {
                MenuSoundCue::None
            };
            (cue, PauseAction::None)
        }
        _ => (MenuSoundCue::None, PauseAction::None),
    };

    if let Some(cfg) = state.menu_ctrl.config_applied_pending.clone() {
        sync_audio_config(state, &cfg);
    }
    emit_menu_sound(state, sound, "Pause menu");

    match action {
        PauseAction::Resume => {
            if let Some(ref mut engine) = state.audio_engine {
                engine.set_bgm_paused(false);
            }
            if let Some(ref mut bank) = state.audio_bank {
                bank.set_spatial_handles_paused(false);
            }
            tracing::info!("Pause Menu: Resumed active gameplay.");
        }
        PauseAction::Save => {
            state.menu_ctrl.pause_menu.close();
            crate::ingame_menus::open_save_load(state, sh2_ui::SaveLoadMode::Save);
            state.menu_ctrl.revision = state.menu_ctrl.revision.wrapping_add(1);
            tracing::info!("Pause Menu: Opened multi-slot SaveLoadScreen (Save mode).");
        }
        PauseAction::Load => {
            state.menu_ctrl.pause_menu.close();
            crate::ingame_menus::open_save_load(state, sh2_ui::SaveLoadMode::Load);
            state.menu_ctrl.revision = state.menu_ctrl.revision.wrapping_add(1);
            tracing::info!("Pause Menu: Opened multi-slot SaveLoadScreen (Load mode).");
        }
        PauseAction::QuitToTitle => {
            state.exit_room_to_menu(TitlePhase::MainMenu);
            tracing::info!("Pause Menu: Exited to Main Menu.");
        }
        PauseAction::None | PauseAction::OpenOptions => {}
    }
}

/// Applies any pending committed options configuration (audio device migration, display resolution/VSync, and control scheme).
pub(crate) fn apply_pending_engine_config(state: &mut AppState) {
    let Some(cfg) = state.menu_ctrl.config_applied_pending.take() else {
        return;
    };

    tracing::info!(
        "[Options] Applying engine configuration: Resolution: {:?}, DisplayMode: {:?}, VSync: {}, FrameRateCap: {:?}, AudioDevice: {:?}, MasterVol: {:.0}%, BGMVol: {:.0}%",
        cfg.video.resolution,
        cfg.video.display_mode,
        cfg.video.vsync,
        cfg.video.framerate_cap,
        cfg.audio.selected_output_device.as_deref().unwrap_or("Default"),
        cfg.audio.master_volume * 100.0,
        cfg.audio.bgm_volume * 100.0,
    );
    state.frame_pacer.set_cap(cfg.video.framerate_cap);

    // 1. Audio Engine volume updates, audio pack source hot-swap, and device re-binding
    sync_audio_config(state, &cfg);

    if cfg.audio.selected_output_device != state.active_audio_device {
        if let Some(ref mut audio) = state.audio_engine {
            audio.migrate_stream_to_device(cfg.audio.selected_output_device.as_deref());
            tracing::info!(
                "[Options] Live migrated audio output stream to {:?}",
                cfg.audio
                    .selected_output_device
                    .as_deref()
                    .unwrap_or("Default (System)")
            );
        } else {
            match AudioEngine::new_with_device(cfg.audio.selected_output_device.as_deref()) {
                Ok(mut new_engine) => {
                    new_engine.set_master_volume(cfg.audio.master_volume);
                    new_engine.set_bgm_volume(cfg.audio.bgm_volume);
                    new_engine.set_sfx_volume(cfg.audio.sfx_volume);
                    new_engine.set_voice_volume(cfg.audio.dialogue_volume);
                    new_engine.set_spatial_audio_mode(cfg.audio.spatial_audio_mode);
                    new_engine.set_acoustic_occlusion_enabled(cfg.audio.enable_acoustic_occlusion);
                    new_engine.set_dynamic_room_acoustics_enabled(
                        cfg.audio.enable_dynamic_room_acoustics,
                    );
                    state.audio_engine = Some(new_engine);
                }
                Err(e) => {
                    tracing::warn!("[Options] Failed to initialize audio device: {:?}", e);
                }
            }
        }
        state.active_audio_device = cfg.audio.selected_output_device.clone();
    }

    // 2. Video & Display Mode re-configuration
    let new_present_mode = if cfg.video.vsync {
        wgpu::PresentMode::AutoVsync
    } else {
        wgpu::PresentMode::AutoNoVsync
    };

    let (target_w, target_h) = match cfg.video.resolution {
        sh2_core::config::InternalResolution::Res1080p => (1920, 1080),
        sh2_core::config::InternalResolution::Res1440p => (2560, 1440),
        sh2_core::config::InternalResolution::Res4k => (3840, 2160),
        sh2_core::config::InternalResolution::Native => {
            let s = state.window.inner_size();
            (s.width.max(640), s.height.max(480))
        }
    };

    match cfg.video.display_mode {
        sh2_core::config::WindowDisplayMode::BorderlessFullscreen => {
            state
                .window
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
        sh2_core::config::WindowDisplayMode::ExclusiveFullscreen => {
            let monitor = state.window.current_monitor();
            let video_mode = monitor.and_then(|m| {
                m.video_modes()
                    .find(|vm| vm.size().width == target_w && vm.size().height == target_h)
            });
            if let Some(vm) = video_mode {
                state
                    .window
                    .set_fullscreen(Some(winit::window::Fullscreen::Exclusive(vm)));
            } else {
                state
                    .window
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            }
        }
        sh2_core::config::WindowDisplayMode::Windowed => {
            state.window.set_fullscreen(None);
            let _ = state
                .window
                .request_inner_size(winit::dpi::PhysicalSize::new(target_w, target_h));
        }
    }

    if state.surface_config.present_mode != new_present_mode {
        state.surface_config.present_mode = new_present_mode;
        state
            .surface
            .configure(&state.render_ctx.device, &state.surface_config);
    }

    state.apply_texture_quality(cfg.video.texture_quality);

    // 3. Control & Modern Camera settings dispatch (`SH2-93`)
    state.sim_core.apply_control_options(&cfg.controls);
    state.sync_cursor_grab_for_camera();
    tracing::info!(
        "[Options] Synced Controls & Camera: Mode={:?}, Cycle={:?}, Shoulder={:?}, Reticle={:?}, FpsFov={}°, Sway={}",
        cfg.controls.default_camera_mode,
        cfg.controls.camera_cycle_preset,
        cfg.controls.ots_shoulder,
        cfg.controls.ots_reticle,
        cfg.controls.first_person_fov_deg,
        cfg.controls.first_person_head_bob,
    );
}
