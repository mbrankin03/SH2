//! Interactive Desktop Playable Test Build for Silent Hill 2 Open Port.
//! Runs the pure PS2 camera, 60 Hz simulation core, volumetric fog, character skeletal animation, and gamepad controls.

mod assets;
mod rooms;
mod state;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::assets::{
    find_cam_file_for_room, find_cld_file_for_room, find_map_file_for_room, load_anm_file,
    load_character_asset, load_viewer_entry_asset,
};
use crate::rooms::fmv_room::open_fmv_session;
use crate::state::AppState;
use sh2_app::AppMenuController;
use sh2_assets::formats::TextureLoader;
use sh2_audio::{play_menu_sound, AudioEngine, MenuAudioBank, SoundCueKind};
use sh2_core::{ControlScheme, ScenarioKind, SimulationCore};
use sh2_debug::{FmvViewerState, GrappleViewerState, ModelViewerState};
use sh2_renderer::{FmvVideoPipeline, GpuMap, RenderContext, RendererConfig, SurfaceBlitter};
use sh2_ui::menu::{MenuNavAction, MenuSoundCue, TestRoomKind, TitlePhase};
use sh2_ui::pause::{PauseAction, PauseSubPhase};
use sh2_ui::{ButtonSpriteSheet, LoadingScreen, TextRasterizer};

#[derive(Default)]
struct SilentHillApp {
    state: Option<AppState>,
    initial_test_room: Option<TestRoomKind>,
    initial_fog_debug: bool,
    initial_viewer_model: Option<String>,
    initial_fmv_movie: Option<String>,
}

impl ApplicationHandler for SilentHillApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("Silent Hill 2 Open Port - Interactive Character & Skeletal Animation")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_visible(true);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let size = window.inner_size();

        // Use Vulkan instance explicitly for desktop Linux Wayland
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        // Create surface safely bound to the window
        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create window surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find suitable Vulkan GPU adapter compatible with window surface");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("SH2 Render Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .expect("Failed to acquire Vulkan logical device");

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let render_config = RendererConfig {
            width: size.width.max(1),
            height: size.height.max(1),
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        };

        let render_ctx = RenderContext::from_device(
            instance,
            adapter,
            Arc::clone(&device),
            Arc::clone(&queue),
            render_config,
        );

        let initial_room = self
            .initial_test_room
            .unwrap_or(TestRoomKind::ObservationDeck);
        let (map_file, player_pos, player_yaw, fog_uniforms) = find_map_file_for_room(initial_room);

        let gpu_map = GpuMap::from_map(&device, &map_file);
        let blitter = SurfaceBlitter::new(&device, surface_format);
        let blit_bind_group = blitter.create_bind_group(&device, &render_ctx.ldr_output_view);

        // Build level diffuse texture bind groups per material (cached by texture_id to avoid redundant VRAM allocations)
        let mut map_texture_bind_groups = Vec::with_capacity(map_file.materials.len());
        let mut tex_bg_cache: std::collections::HashMap<u32, std::sync::Arc<wgpu::BindGroup>> =
            std::collections::HashMap::new();

        for (m_idx, mat) in map_file.materials.iter().enumerate() {
            let tex_id = mat.texture_id as u32;
            if let Some(bg) = tex_bg_cache.get(&tex_id) {
                map_texture_bind_groups.push(Arc::clone(bg));
                continue;
            }

            if let Some(tex) = map_file.textures.get(&tex_id) {
                if let Ok(rgba) = tex.to_rgba() {
                    let bg = render_ctx.create_texture_bind_group(
                        tex.width as u32,
                        tex.height as u32,
                        &rgba,
                        Some(&format!("Map Material {m_idx} Texture {tex_id}")),
                    );
                    tex_bg_cache.insert(tex_id, Arc::clone(&bg));
                    map_texture_bind_groups.push(bg);
                    continue;
                }
            }
            map_texture_bind_groups.push(Arc::clone(&render_ctx.default_texture_bind_group));
        }

        // Load James Sunderland character model & animations (prioritizing high-fidelity 71-bone hhh_jms.mdl)
        let character = load_character_asset(
            &render_ctx,
            "James Sunderland",
            &["data/chr/jms/hhh_jms.mdl", "data/chr/jms/hll_jms.mdl"],
            &["data/chr/jms/jms_basic1.anm"],
        );

        let mut james_anm_cache = std::collections::HashMap::new();
        if let Some(ref c) = character {
            for &src in sh2_debug::JamesAnmSource::all() {
                if src == sh2_debug::JamesAnmSource::Basic {
                    continue;
                }
                if let Some(anm) =
                    load_anm_file(src.rel_path(), c.model.bone_count, &c.model.parents)
                {
                    james_anm_cache.insert(src, anm);
                }
            }
        }

        // Load Maria character model & animations (for character switching or Born From a Wish)
        let maria = load_character_asset(
            &render_ctx,
            "Maria",
            &["data/chr/mar/lll_mar.mdl", "data/chr/mar/hhh_mar.mdl"],
            &["data/chr2/mar/xmar_wpnone.anm", "data/chr/mar/lll_mar.anm"],
        );

        // Preload authentic enemy models for Combat Testing Arena (SH2-13 / SH2-57)
        let enemy_scu_asset = load_character_asset(
            &render_ctx,
            "Lying Figure (scu)",
            &["data/chr/scu/scu.mdl"],
            &["data/chr/scu/scu.anm"],
        );
        let enemy_mkn_asset = load_character_asset(
            &render_ctx,
            "Mannequin (mkn)",
            &["data/chr/mkn/mkn.mdl"],
            &["data/chr/mkn/mkn.anm"],
        );
        let enemy_nse_asset = load_character_asset(
            &render_ctx,
            "Bubble Head Nurse (nse)",
            &["data/chr/nse/nse.mdl"],
            &["data/chr/nse/nse.anm"],
        );
        let enemy_red_asset = load_character_asset(
            &render_ctx,
            "Pyramid Head (red)",
            &["data/chr/red/red.mdl"],
            &["data/chr/red/red.anm"],
        );
        let enemy_pap_asset = load_character_asset(
            &render_ctx,
            "Abstract Daddy (pap)",
            &["data/chr/pap/pap.mdl"],
            &["data/chr/pap/pap.anm"],
        );
        let enemy_ike_asset = load_character_asset(
            &render_ctx,
            "Flesh Lips (ike)",
            &["data/chr/ike/ike.mdl"],
            &["data/chr/ike/ike.anm"],
        );
        let enemy_arm_asset = load_character_asset(
            &render_ctx,
            "Mandarin (arm)",
            &["data/chr/arm/arm.mdl"],
            &["data/chr/arm/arm.anm"],
        );
        let enemy_tyu_asset = load_character_asset(
            &render_ctx,
            "Creeper (tyu)",
            &["data/chr/tyu/tyu.mdl"],
            &["data/chr/tyu/tyu.anm"],
        );
        let enemy_bos_asset = load_character_asset(
            &render_ctx,
            "Mary Final Boss (bos)",
            &["data/chr/bos/bos.mdl"],
            &["data/chr/bos/bos.anm"],
        );

        let mut combat_arena = sh2_core::combat::CombatArena::new();
        // Give James his trusty wooden plank for the testing arena
        combat_arena.player.weapon = sh2_core::combat::WeaponGrip::WoodenPlank;

        let mut sim_core = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim_core.collision = find_cld_file_for_room(initial_room);
        sim_core
            .camera
            .set_cam_file(find_cam_file_for_room(initial_room));
        sim_core.input.set_control_scheme(ControlScheme::Tank);
        sim_core.scenario.flashlight_on = true;
        sim_core.snap_player_transform(player_pos, player_yaw);
        sim_core.camera.reset_to_player(
            sim_core.scenario.player_position,
            sim_core.scenario.player_yaw,
        );

        // Atmospheric Silent Hill lighting & fog (calibrated for current room)
        render_ctx.update_fog(&fog_uniforms);

        // Preload authentic 2D UI / Title screen textures
        let tex_loader = TextureLoader::new(
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data",
            Some("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e"),
        );

        use wgpu::util::DeviceExt;
        let initial_tint = [1.0f32, 1.0, 1.0, 1.0];
        let menu_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Menu Blit Uniform Buffer"),
            contents: bytemuck::cast_slice(&initial_tint),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let splash_konami_bg = tex_loader
            .load_texture("pic/etc/konami2048x2048.tex")
            .or_else(|_| tex_loader.load_texture("pic/etc/start00.tex"))
            .ok()
            .map(|t| {
                render_ctx.create_blitter_bind_group(
                    &blitter,
                    t.width as u32,
                    t.height as u32,
                    &t.rgba,
                    &menu_uniform_buffer,
                    Some("Konami Splash Texture"),
                )
            });

        let splash_title_bg = tex_loader
            .load_texture("pic/etc/start00.tex")
            .ok()
            .map(|t| {
                render_ctx.create_blitter_bind_group(
                    &blitter,
                    t.width as u32,
                    t.height as u32,
                    &t.rgba,
                    &menu_uniform_buffer,
                    Some("Title Background Texture"),
                )
            });

        // Dynamic UI overlay texture and text rasterizer for Authentic Menus & HUD
        let text_w = 1280u32;
        let text_h = 720u32;
        let menu_rasterizer = TextRasterizer::new(text_w as usize, text_h as usize);

        let menu_text_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Menu Text Overlay Texture"),
            size: wgpu::Extent3d {
                width: text_w,
                height: text_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let menu_text_view = menu_text_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let menu_text_bind_group = blitter.create_bind_group_with_uniform_buffer(
            &device,
            &menu_text_view,
            &menu_uniform_buffer,
        );

        // Preload authentic 1:1 button sprite atlas from start01e.tex / start01.tex
        let button_sprites = tex_loader
            .load_texture("pic/etc/start01e.tex")
            .or_else(|_| tex_loader.load_texture("pic/etc/start01.tex"))
            .ok()
            .map(|t| ButtonSpriteSheet::new(t.width, t.height, t.rgba));

        let data_dir =
            PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data");
        let ee_dir =
            PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e");
        let audio_bank = Some(MenuAudioBank::load_from_dirs(&data_dir, Some(&ee_dir)));

        let host_devices = AudioEngine::enumerate_output_devices();
        let config_candidates = ["config.json", "../config.json"];
        let chosen_config_path = config_candidates
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .unwrap_or(&"config.json");
        let mut menu_ctrl = AppMenuController::new(chosen_config_path);
        menu_ctrl
            .options_menu
            .set_audio_devices(host_devices.clone());

        let target_audio_device = menu_ctrl
            .options_menu
            .original_config
            .audio
            .selected_output_device
            .clone();

        let audio_engine = match AudioEngine::new_with_device(target_audio_device.as_deref()) {
            Ok(engine) => {
                tracing::info!(
                    "Audio engine successfully initialized with Kira runtime (Device: {:?}).",
                    target_audio_device.as_deref().unwrap_or("Default")
                );
                Some(engine)
            }
            Err(e) => {
                tracing::warn!("Audio device initialization warning: {:?}", e);
                None
            }
        };

        tracing::info!(
            "Simulation, Character Models, and Renderer initialized successfully on Vulkan!"
        );
        tracing::info!("Controls: WASD / Stick: Move & Turn (Tank) | Shift: Run | F: Flashlight | C: Switch Character (James/Maria)");

        if let Some(room) = self.initial_test_room {
            menu_ctrl.title_sm.selected_test_room = room;
            if let Some(idx) = TestRoomKind::ALL.iter().position(|r| *r == room) {
                menu_ctrl.title_sm.test_room_index = idx;
            }
        }
        if self.initial_test_room.is_some() || self.initial_fog_debug {
            // Immediate boot into the requested test room if specified via CLI
            menu_ctrl.in_game_active = true;
            menu_ctrl.title_sm.phase = TitlePhase::TransitionToGame;
            menu_ctrl.title_sm.fade_alpha = 0.0;
        }

        let mut model_viewer = ModelViewerState::new();
        if let Some(ref model_query) = self.initial_viewer_model {
            if model_viewer.select_by_name(model_query) {
                tracing::info!(
                    "Model Viewer selected initial model from CLI: '{}' -> {}",
                    model_query,
                    model_viewer.current_entry().display_name
                );
            } else {
                tracing::warn!(
                    "Model Viewer could not match '{}' in Bestiary catalog; defaulting to '{}'",
                    model_query,
                    model_viewer.current_entry().display_name
                );
            }
        }

        let viewer_asset = if initial_room == TestRoomKind::ModelAndTextureViewer {
            let asset = load_viewer_entry_asset(
                &render_ctx,
                model_viewer.current_entry(),
                model_viewer.selected_anm_index,
            );
            if let Some(ref a) = asset {
                model_viewer.set_total_frames(a.anm.frames.len());
            }
            asset
        } else {
            None
        };
        let loaded_viewer_model_idx = model_viewer.selected_model_index;
        let loaded_viewer_anm_idx = model_viewer.selected_anm_index;

        let fmv_pipeline = FmvVideoPipeline::new(&device, RenderContext::OUTPUT_FORMAT);
        let mut fmv_viewer = FmvViewerState::new();
        if let Some(ref movie_query) = self.initial_fmv_movie {
            if fmv_viewer.select_by_name(movie_query) {
                tracing::info!(
                    "FMV Viewer selected initial movie from CLI: '{}' -> {}",
                    movie_query,
                    fmv_viewer.current_entry().display_name
                );
            } else {
                tracing::warn!(
                    "FMV Viewer could not match '{}' in FMV catalog; defaulting to '{}'",
                    movie_query,
                    fmv_viewer.current_entry().display_name
                );
            }
        }
        let mut audio_engine = audio_engine;
        let current_fmv_entry = fmv_viewer.current_entry();
        let source_pref = fmv_viewer.video_source;
        let fmv_session = if initial_room == TestRoomKind::FmvCutsceneViewer {
            open_fmv_session(
                &device,
                &queue,
                &fmv_pipeline,
                &mut audio_engine,
                &mut fmv_viewer,
                current_fmv_entry,
                source_pref,
            )
        } else {
            None
        };
        let loaded_fmv_movie_idx = fmv_viewer.selected_movie_index;
        let loaded_fmv_source = fmv_viewer.video_source;

        let initial_fps_cap = menu_ctrl.options_menu.original_config.video.framerate_cap;

        self.state = Some(AppState {
            window,
            surface,
            surface_config,
            render_ctx,
            blitter,
            blit_bind_group,
            splash_konami_bg,
            splash_title_bg,
            menu_uniform_buffer,
            menu_text_texture,
            menu_text_bind_group,
            menu_rasterizer,
            button_sprites,
            audio_bank,
            audio_engine,
            sim_core,
            gpu_map,
            map_texture_bind_groups,
            character,
            maria,
            james_anm_cache,
            active_maria: false,
            active_test_room: initial_room,
            fog_uniforms,
            menu_ctrl,
            combat_arena,
            enemy_scu_asset,
            enemy_mkn_asset,
            enemy_nse_asset,
            enemy_red_asset,
            enemy_pap_asset,
            enemy_ike_asset,
            enemy_arm_asset,
            enemy_tyu_asset,
            enemy_bos_asset,
            key_ready_held: false,
            key_attack_pressed: false,
            key_attack_held: false,
            combat_enemy_idx: 0,
            struggle_button_press_count: 0,
            model_viewer,
            grapple_viewer: GrappleViewerState::new(),
            viewer_asset,
            loaded_viewer_model_idx,
            loaded_viewer_anm_idx,
            fmv_pipeline,
            fmv_viewer,
            fmv_session,
            loaded_fmv_movie_idx,
            loaded_fmv_source,
            mouse_left_pressed: false,
            mouse_right_pressed: false,
            last_cursor_pos: None,
            key_forward: false,
            key_backward: false,
            key_left: false,
            key_right: false,
            key_zoom_in: false,
            key_zoom_out: false,
            key_pan_up: false,
            key_pan_down: false,
            last_menu_state_log: String::new(),
            last_menu_revision: u64::MAX,
            active_audio_device: target_audio_device,
            loading_screen: None,
            debug_collision_overlay: false,
            debug_camera_hud: true,
            skin_matrices_scratch: Vec::with_capacity(128),
            frame_pacer: sh2_core::FramePacer::new(initial_fps_cap),
            last_frame: Instant::now(),
            frame_counter: 0,
            sim_time: 0.0,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut state) = self.state else {
            return;
        };

        match event {
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if let winit::keyboard::PhysicalKey::Code(key_code) = key_event.physical_key {
                    let pressed = key_event.state == winit::event::ElementState::Pressed;

                    // If menu is active, route navigation inputs to the menu system
                    if !state.menu_ctrl.in_game_active {
                        if pressed {
                            // Intercept Confirm button if loading screen is active
                            if state.menu_ctrl.title_sm.phase == TitlePhase::TransitionToGame
                                && matches!(
                                    key_code,
                                    winit::keyboard::KeyCode::Enter
                                        | winit::keyboard::KeyCode::NumpadEnter
                                        | winit::keyboard::KeyCode::Space
                                )
                            {
                                if let Some(ref mut ls) = state.loading_screen {
                                    if ls.handle_confirm() {
                                        tracing::info!("Loading Screen: Player pressed continue.");
                                        if let Some(ref mut engine) = state.audio_engine {
                                            let _ = play_menu_sound(
                                                engine,
                                                state.audio_bank.as_ref(),
                                                SoundCueKind::Confirm,
                                            );
                                        }
                                    }
                                    return;
                                }
                            }

                            let sound = match key_code {
                                winit::keyboard::KeyCode::KeyW
                                | winit::keyboard::KeyCode::ArrowUp => {
                                    state.menu_ctrl.handle_nav(MenuNavAction::Up)
                                }
                                winit::keyboard::KeyCode::KeyS
                                | winit::keyboard::KeyCode::ArrowDown => {
                                    state.menu_ctrl.handle_nav(MenuNavAction::Down)
                                }
                                winit::keyboard::KeyCode::KeyA
                                | winit::keyboard::KeyCode::ArrowLeft => {
                                    state.menu_ctrl.handle_nav(MenuNavAction::Left)
                                }
                                winit::keyboard::KeyCode::KeyD
                                | winit::keyboard::KeyCode::ArrowRight => {
                                    state.menu_ctrl.handle_nav(MenuNavAction::Right)
                                }
                                winit::keyboard::KeyCode::Enter
                                | winit::keyboard::KeyCode::NumpadEnter
                                | winit::keyboard::KeyCode::Space => {
                                    state.menu_ctrl.handle_nav(MenuNavAction::Confirm)
                                }
                                winit::keyboard::KeyCode::Escape
                                | winit::keyboard::KeyCode::Backspace => {
                                    state.menu_ctrl.handle_nav(MenuNavAction::Cancel)
                                }
                                winit::keyboard::KeyCode::KeyQ => {
                                    state.menu_ctrl.switch_options_tab(false)
                                }
                                winit::keyboard::KeyCode::KeyE => {
                                    state.menu_ctrl.switch_options_tab(true)
                                }
                                _ => match state.menu_ctrl.title_sm.phase {
                                    TitlePhase::SplashIntro | TitlePhase::TitleLogo => {
                                        state.menu_ctrl.handle_nav(MenuNavAction::Confirm)
                                    }
                                    _ => MenuSoundCue::None,
                                },
                            };
                            if sound != MenuSoundCue::None {
                                tracing::info!("Menu sound emitted: {:?}", sound);
                                if let Some(ref mut engine) = state.audio_engine {
                                    let cue_kind = match sound {
                                        MenuSoundCue::CursorMove => Some(SoundCueKind::CursorMove),
                                        MenuSoundCue::Confirm => Some(SoundCueKind::Confirm),
                                        MenuSoundCue::Cancel => Some(SoundCueKind::Cancel),
                                        MenuSoundCue::GameStartTransition => {
                                            Some(SoundCueKind::GameStartTransition)
                                        }
                                        MenuSoundCue::None => None,
                                    };
                                    if let Some(kind) = cue_kind {
                                        let _ = play_menu_sound(
                                            engine,
                                            state.audio_bank.as_ref(),
                                            kind,
                                        );
                                    }
                                }
                            }
                        }
                        return;
                    }

                    // If in-game pause menu is active, route navigation inputs to the pause system
                    if state.menu_ctrl.in_game_active && state.menu_ctrl.pause_menu.is_open {
                        if pressed {
                            let (sound, action) = match key_code {
                                winit::keyboard::KeyCode::KeyW
                                | winit::keyboard::KeyCode::ArrowUp => {
                                    state.menu_ctrl.handle_pause_nav(MenuNavAction::Up)
                                }
                                winit::keyboard::KeyCode::KeyS
                                | winit::keyboard::KeyCode::ArrowDown => {
                                    state.menu_ctrl.handle_pause_nav(MenuNavAction::Down)
                                }
                                winit::keyboard::KeyCode::KeyA
                                | winit::keyboard::KeyCode::ArrowLeft => {
                                    state.menu_ctrl.handle_pause_nav(MenuNavAction::Left)
                                }
                                winit::keyboard::KeyCode::KeyD
                                | winit::keyboard::KeyCode::ArrowRight => {
                                    state.menu_ctrl.handle_pause_nav(MenuNavAction::Right)
                                }
                                winit::keyboard::KeyCode::Enter
                                | winit::keyboard::KeyCode::NumpadEnter
                                | winit::keyboard::KeyCode::Space => {
                                    state.menu_ctrl.handle_pause_nav(MenuNavAction::Confirm)
                                }
                                winit::keyboard::KeyCode::Escape
                                | winit::keyboard::KeyCode::Backspace => {
                                    state.menu_ctrl.handle_pause_nav(MenuNavAction::Cancel)
                                }
                                winit::keyboard::KeyCode::KeyQ => {
                                    let s = state.menu_ctrl.switch_options_tab(false);
                                    (s, PauseAction::None)
                                }
                                winit::keyboard::KeyCode::KeyE => {
                                    let s = state.menu_ctrl.switch_options_tab(true);
                                    (s, PauseAction::None)
                                }
                                _ => (MenuSoundCue::None, PauseAction::None),
                            };

                            if sound != MenuSoundCue::None {
                                tracing::info!("Pause menu sound emitted: {:?}", sound);
                                if let Some(ref mut engine) = state.audio_engine {
                                    let cue_kind = match sound {
                                        MenuSoundCue::CursorMove => Some(SoundCueKind::CursorMove),
                                        MenuSoundCue::Confirm => Some(SoundCueKind::Confirm),
                                        MenuSoundCue::Cancel => Some(SoundCueKind::Cancel),
                                        MenuSoundCue::GameStartTransition => {
                                            Some(SoundCueKind::GameStartTransition)
                                        }
                                        MenuSoundCue::None => None,
                                    };
                                    if let Some(kind) = cue_kind {
                                        let _ = play_menu_sound(
                                            engine,
                                            state.audio_bank.as_ref(),
                                            kind,
                                        );
                                    }
                                }
                            }

                            if action == PauseAction::Resume {
                                if let Some(ref mut engine) = state.audio_engine {
                                    engine.set_bgm_paused(false);
                                }
                                tracing::info!("Pause Menu: Resumed active gameplay.");
                            } else if action == PauseAction::QuitToTitle {
                                if let Some(ref mut engine) = state.audio_engine {
                                    engine.stop_bgm(std::time::Duration::from_millis(150));
                                    engine.set_bgm_paused(false);
                                }
                                state.menu_ctrl.in_game_active = false;
                                state.menu_ctrl.pause_menu.close();
                                state.menu_ctrl.title_sm.phase = TitlePhase::MainMenu;
                                state.menu_ctrl.title_sm.fade_alpha = 1.0;
                                state.menu_ctrl.title_sm.is_fading_out = false;
                                state.menu_ctrl.revision = state.menu_ctrl.revision.wrapping_add(1);
                                tracing::info!("Pause Menu: Exited to Main Menu.");
                            }
                        }
                        return;
                    }

                    // Delegate active room keyboard handling to the modular room controllers
                    rooms::dispatch_key_input(state, key_code, pressed, key_event.repeat);
                }
            }
            WindowEvent::MouseInput {
                state: element_state,
                button,
                ..
            } => {
                let pressed = element_state == winit::event::ElementState::Pressed;
                if button == winit::event::MouseButton::Left {
                    state.mouse_left_pressed = pressed;
                } else if button == winit::event::MouseButton::Right {
                    state.mouse_right_pressed = pressed;
                }

                if pressed && !state.menu_ctrl.in_game_active {
                    let sound = state.menu_ctrl.handle_nav(MenuNavAction::Confirm);
                    if sound != MenuSoundCue::None {
                        tracing::info!("Menu sound emitted: {:?}", sound);
                        if let Some(ref mut engine) = state.audio_engine {
                            let cue_kind = match sound {
                                MenuSoundCue::CursorMove => Some(SoundCueKind::CursorMove),
                                MenuSoundCue::Confirm => Some(SoundCueKind::Confirm),
                                MenuSoundCue::Cancel => Some(SoundCueKind::Cancel),
                                MenuSoundCue::GameStartTransition => {
                                    Some(SoundCueKind::GameStartTransition)
                                }
                                MenuSoundCue::None => None,
                            };
                            if let Some(kind) = cue_kind {
                                let _ = play_menu_sound(engine, state.audio_bank.as_ref(), kind);
                            }
                        }
                    }
                } else if state.menu_ctrl.in_game_active
                    && state.active_test_room == TestRoomKind::CombatArena
                {
                    // PC/Modern combat controls: Right Click = Ready weapon, Left Click = Attack / Stomp
                    if button == winit::event::MouseButton::Right {
                        state.key_ready_held = pressed;
                    } else if button == winit::event::MouseButton::Left {
                        if pressed && !state.key_attack_held {
                            state.key_attack_pressed = true;
                        }
                        state.key_attack_held = pressed;
                    }
                    if pressed
                        && state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held
                    {
                        state.struggle_button_press_count += 1;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((last_x, last_y)) = state.last_cursor_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;
                    if state.menu_ctrl.in_game_active {
                        if state.active_test_room == TestRoomKind::ModelAndTextureViewer {
                            if state.mouse_left_pressed {
                                state.model_viewer.camera.orbit(-dx * 0.007, dy * 0.007);
                            } else if state.mouse_right_pressed {
                                state.model_viewer.camera.pan(-dx * 2.0, dy * 2.0);
                            }
                        } else if state.active_test_room == TestRoomKind::GrappleAnimationRoom {
                            if state.mouse_left_pressed {
                                state.grapple_viewer.camera.orbit(-dx * 0.007, dy * 0.007);
                            } else if state.mouse_right_pressed {
                                state.grapple_viewer.camera.pan(-dx * 2.0, dy * 2.0);
                            }
                        }
                    }
                }
                state.last_cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if state.menu_ctrl.in_game_active {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 150.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 1.5,
                    };
                    if state.active_test_room == TestRoomKind::ModelAndTextureViewer {
                        state.model_viewer.camera.zoom(-scroll);
                    } else if state.active_test_room == TestRoomKind::GrappleAnimationRoom {
                        state.grapple_viewer.camera.zoom(-scroll);
                    }
                }
            }
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested, shutting down...");
                event_loop.exit();
            }
            WindowEvent::Focused(focused) => {
                let mute_in_bg = state
                    .menu_ctrl
                    .options_menu
                    .original_config
                    .audio
                    .mute_in_background;
                if mute_in_bg {
                    if let Some(ref mut audio) = state.audio_engine {
                        if !focused {
                            audio.set_master_volume(0.0);
                            tracing::info!("[Options] Focus lost; muted audio in background.");
                        } else {
                            let master_vol = state
                                .menu_ctrl
                                .options_menu
                                .original_config
                                .audio
                                .master_volume;
                            audio.set_master_volume(master_vol);
                            tracing::info!(
                                "[Options] Focus restored; restored audio volume to {:.0}%.",
                                master_vol * 100.0
                            );
                        }
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state
                        .surface
                        .configure(&state.render_ctx.device, &state.surface_config);
                    state.render_ctx.resize(new_size.width, new_size.height);
                    state.blit_bind_group = state.blitter.create_bind_group(
                        &state.render_ctx.device,
                        &state.render_ctx.ldr_output_view,
                    );
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame);
                state.last_frame = now;
                state.frame_pacer.begin_frame(now);

                // Check and apply committed options configuration
                if let Some(cfg) = state.menu_ctrl.config_applied_pending.take() {
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

                    // 1. Audio Engine volume updates and device re-binding
                    if let Some(ref mut audio) = state.audio_engine {
                        audio.set_master_volume(cfg.audio.master_volume);
                        audio.set_bgm_volume(cfg.audio.bgm_volume);
                        audio.set_sfx_volume(cfg.audio.sfx_volume);
                        audio.set_voice_volume(cfg.audio.dialogue_volume);
                    }

                    // Check if audio device changed and live-migrate audio manager stream
                    if cfg.audio.selected_output_device != state.active_audio_device {
                        if let Some(ref mut audio) = state.audio_engine {
                            audio.migrate_stream_to_device(
                                cfg.audio.selected_output_device.as_deref(),
                            );
                            tracing::info!(
                                "[Options] Live migrated audio output stream to {:?}",
                                cfg.audio
                                    .selected_output_device
                                    .as_deref()
                                    .unwrap_or("Default (System)")
                            );
                        } else {
                            match AudioEngine::new_with_device(
                                cfg.audio.selected_output_device.as_deref(),
                            ) {
                                Ok(mut new_engine) => {
                                    new_engine.set_master_volume(cfg.audio.master_volume);
                                    new_engine.set_bgm_volume(cfg.audio.bgm_volume);
                                    new_engine.set_sfx_volume(cfg.audio.sfx_volume);
                                    new_engine.set_voice_volume(cfg.audio.dialogue_volume);
                                    state.audio_engine = Some(new_engine);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "[Options] Failed to initialize audio device: {:?}",
                                        e
                                    );
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
                                m.video_modes().find(|vm| {
                                    vm.size().width == target_w && vm.size().height == target_h
                                })
                            });
                            if let Some(vm) = video_mode {
                                state
                                    .window
                                    .set_fullscreen(Some(winit::window::Fullscreen::Exclusive(vm)));
                            } else {
                                state.window.set_fullscreen(Some(
                                    winit::window::Fullscreen::Borderless(None),
                                ));
                            }
                        }
                        sh2_core::config::WindowDisplayMode::Windowed => {
                            state.window.set_fullscreen(None);
                            let _ = state
                                .window
                                .request_inner_size(winit::dpi::PhysicalSize::new(
                                    target_w, target_h,
                                ));
                        }
                    }

                    if state.surface_config.present_mode != new_present_mode {
                        state.surface_config.present_mode = new_present_mode;
                        state
                            .surface
                            .configure(&state.render_ctx.device, &state.surface_config);
                    }

                    // 3. Control settings dispatch
                    state.sim_core.input.config.stick_deadzone = cfg.controls.stick_deadzone;
                    state.sim_core.input.config.vibration_intensity =
                        cfg.controls.vibration_intensity;
                    state
                        .sim_core
                        .input
                        .set_control_scheme(if cfg.controls.tank_controls {
                            sh2_core::input::ControlScheme::Tank
                        } else {
                            sh2_core::input::ControlScheme::AnalogDirectional
                        });
                }

                // Advance menu transitions and fade state
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
                        state.menu_ctrl.tick(dt.as_secs_f32());
                    } else {
                        if state.loading_screen.is_none() {
                            let label = state.menu_ctrl.title_sm.selected_test_room.label();
                            let mut ls = LoadingScreen::new(label, min_dur, require_press);
                            ls.mark_completed(); // Engine and assets are ready, begin loading display
                            state.loading_screen = Some(ls);
                        }

                        if let Some(ref mut ls) = state.loading_screen {
                            ls.tick(dt.as_secs_f32());
                            if ls.transition_ready {
                                state.loading_screen = None;
                                state.menu_ctrl.tick(dt.as_secs_f32());
                            }
                        }
                    }
                } else {
                    state.loading_screen = None;
                    state.menu_ctrl.tick(dt.as_secs_f32());
                }

                // If user transitioned into game from Test Rooms menu, switch level map if needed
                if state.menu_ctrl.in_game_active
                    && state.active_test_room != state.menu_ctrl.title_sm.selected_test_room
                {
                    let new_room = state.menu_ctrl.title_sm.selected_test_room;
                    tracing::info!("Switching test environment to: {:?}", new_room);
                    let (new_map, new_pos, new_yaw, new_fog) = find_map_file_for_room(new_room);
                    state.gpu_map = GpuMap::from_map(&state.render_ctx.device, &new_map);
                    state.fog_uniforms = new_fog;
                    state.render_ctx.update_fog(&state.fog_uniforms);
                    state.sim_core.collision = find_cld_file_for_room(new_room);
                    state
                        .sim_core
                        .camera
                        .set_cam_file(find_cam_file_for_room(new_room));
                    state.sim_core.snap_player_transform(new_pos, new_yaw);
                    state.sim_core.camera.reset_to_player(new_pos, new_yaw);
                    state.active_test_room = new_room;
                    if new_room == TestRoomKind::CombatArena {
                        state.combat_arena = sh2_core::combat::CombatArena::new();
                        state.combat_arena.player.weapon =
                            sh2_core::combat::WeaponGrip::WoodenPlank;
                    }
                    if new_room == TestRoomKind::GrappleAnimationRoom {
                        state.grapple_viewer.reset_for_current_entry();
                    }
                    if new_room == TestRoomKind::ModelAndTextureViewer
                        && state.viewer_asset.is_none()
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
                    state.fmv_session = open_fmv_session(
                        &state.render_ctx.device,
                        &state.render_ctx.queue,
                        &state.fmv_pipeline,
                        &mut state.audio_engine,
                        &mut state.fmv_viewer,
                        entry,
                        source_pref,
                    );
                    state.loaded_fmv_movie_idx = state.fmv_viewer.selected_movie_index;
                    state.loaded_fmv_source = state.fmv_viewer.video_source;
                    state.fmv_viewer.is_paused = false;
                }

                // When menu is active, log formatted state changes only when menu revision changes
                if !state.menu_ctrl.in_game_active
                    && state.last_menu_revision != state.menu_ctrl.revision
                {
                    state.last_menu_revision = state.menu_ctrl.revision;
                    let current_text = state.menu_ctrl.overlay_text().join(" | ");
                    tracing::info!("[SH2 MENU HUD] {}", current_text);
                    state.last_menu_state_log = current_text;
                }

                let is_in_game = state.menu_ctrl.in_game_active;
                let fade_alpha = state.menu_ctrl.title_sm.fade_alpha;
                let phase = state.menu_ctrl.title_sm.phase;
                let is_viewer =
                    is_in_game && state.active_test_room == TestRoomKind::ModelAndTextureViewer;
                let is_grapple_room =
                    is_in_game && state.active_test_room == TestRoomKind::GrappleAnimationRoom;
                let is_paused = is_in_game && state.menu_ctrl.pause_menu.is_open;

                // Advance simulation time and frame counter for animated post-processing
                let dt_sec = if is_paused { 0.0 } else { dt.as_secs_f32() };
                if !is_paused {
                    state.frame_counter = state.frame_counter.wrapping_add(1);
                    state.sim_time += dt_sec;
                }

                // Retrieve active grain intensity percentage from options configuration
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

                let aspect =
                    state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;

                // Dispatch 3D scene or video pass to the active room controller (advances 60 Hz sim ticks)
                rooms::dispatch_tick_and_render_3d(
                    state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
                );

                // Update fog flow velocity time deterministically from elapsed 60 Hz simulation ticks + sub-frame alpha
                let det_fog_time = state
                    .sim_core
                    .deterministic_fog_time(state.sim_core.interpolation_alpha());
                state.fog_uniforms.time = det_fog_time;
                state.render_ctx.update_fog(&state.fog_uniforms);

                state
                    .render_ctx
                    .update_post(&sh2_renderer::post::PostUniforms {
                        time: det_fog_time,
                        grain_intensity,
                        exposure: 1.0,
                        frame_count: state.frame_counter,
                    });

                // Present to window surface swapchain
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
                let mut encoder = state.render_ctx.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Surface Present Encoder"),
                    },
                );

                if is_in_game {
                    state.blitter.render_with_bind_group(
                        &mut encoder,
                        &state.blit_bind_group,
                        &surface_view,
                    );

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
                    // Update menu blit tint buffer with current fade alpha
                    let tint = [fade_alpha, fade_alpha, fade_alpha, fade_alpha];
                    state.render_ctx.queue.write_buffer(
                        &state.menu_uniform_buffer,
                        0,
                        bytemuck::cast_slice(&tint),
                    );

                    let bg_to_draw = match phase {
                        TitlePhase::SplashIntro => state
                            .splash_konami_bg
                            .as_ref()
                            .or(state.splash_title_bg.as_ref()),
                        TitlePhase::OptionsMenu
                        | TitlePhase::TestRoomsSelect
                        | TitlePhase::TransitionToGame => None, // Authentic PS2 draws over pure black backdrop
                        _ => state
                            .splash_title_bg
                            .as_ref()
                            .or(state.splash_konami_bg.as_ref()),
                    };

                    if let Some(bg) = bg_to_draw {
                        state.blitter.render_with_bind_group_and_load_op(
                            &mut encoder,
                            bg,
                            &surface_view,
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        );
                    } else {
                        // Clear to black if no texture found
                        let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Menu Clear Pass"),
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
                            TitlePhase::ActionDifficultySelect
                            | TitlePhase::RiddleDifficultySelect => {
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
                        state.finish_hud_overlay(&mut encoder, &surface_view);
                    }
                }

                state
                    .render_ctx
                    .queue
                    .submit(std::iter::once(encoder.finish()));
                surface_texture.present();

                // Enforce zero-jitter frame rate cap (30 / 60 / 120 / 144 / 240 FPS / Uncapped VRR)
                state.frame_pacer.wait_for_deadline();
                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            if let Some(deadline) = state.frame_pacer.next_deadline() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            } else {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            state.window.request_redraw();
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(
        "Starting Silent Hill 2 Open Port Interactive Playable Test with Skeletal Mesh Runtime..."
    );

    // Parse CLI arguments for test rooms, model viewer, FMV player, & fog testing
    let args: Vec<String> = std::env::args().collect();
    let mut initial_test_room = None;
    let mut initial_fog_debug = false;
    let mut initial_viewer_model = None;
    let mut initial_fmv_movie = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--test-fog" {
            initial_fog_debug = true;
            if initial_test_room.is_none() {
                initial_test_room = Some(TestRoomKind::SyntheticFogRunway);
            }
        } else if arg == "--model-viewer" {
            initial_test_room = Some(TestRoomKind::ModelAndTextureViewer);
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                initial_viewer_model = Some(args[i + 1].clone());
                i += 1;
            }
        } else if let Some(model_str) = arg.strip_prefix("--model-viewer=") {
            initial_test_room = Some(TestRoomKind::ModelAndTextureViewer);
            if !model_str.is_empty() {
                initial_viewer_model = Some(model_str.to_string());
            }
        } else if arg == "--fmv-viewer" || arg == "--fmv" {
            initial_test_room = Some(TestRoomKind::FmvCutsceneViewer);
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                initial_fmv_movie = Some(args[i + 1].clone());
                i += 1;
            }
        } else if let Some(movie_str) = arg.strip_prefix("--fmv-viewer=") {
            initial_test_room = Some(TestRoomKind::FmvCutsceneViewer);
            if !movie_str.is_empty() {
                initial_fmv_movie = Some(movie_str.to_string());
            }
        } else if arg == "--grapple" || arg == "--grapples" || arg == "--grapple-viewer" {
            initial_test_room = Some(TestRoomKind::GrappleAnimationRoom);
        } else if let Some(room_str) = arg
            .strip_prefix("--test-room=")
            .or_else(|| arg.strip_prefix("--room="))
        {
            initial_test_room = match room_str {
                "ob01" | "observation" | "outdoor" => Some(TestRoomKind::ObservationDeck),
                "er01" | "restroom" | "toilet" => Some(TestRoomKind::RestroomStart),
                "rr21" | "boiler" => Some(TestRoomKind::HotelBoilerRoom),
                "runway" | "fog" => Some(TestRoomKind::SyntheticFogRunway),
                "fallback" | "sandbox" => Some(TestRoomKind::FallbackSandbox),
                "viewer" | "model-viewer" | "models" | "bestiary" => {
                    Some(TestRoomKind::ModelAndTextureViewer)
                }
                "fmv" | "fmv-viewer" | "cutscenes" | "movies" => {
                    Some(TestRoomKind::FmvCutsceneViewer)
                }
                "combat" | "arena" | "battle" => Some(TestRoomKind::CombatArena),
                "grapple" | "grapples" | "grapple-room" | "grapple-viewer" => {
                    Some(TestRoomKind::GrappleAnimationRoom)
                }
                _ => {
                    tracing::warn!(
                        "Unknown test room: '{}', defaulting to Observation Deck",
                        room_str
                    );
                    Some(TestRoomKind::ObservationDeck)
                }
            };
        } else if arg == "--room" && i + 1 < args.len() && !args[i + 1].starts_with("--") {
            let room_str = &args[i + 1];
            initial_test_room = match room_str.as_str() {
                "ob01" | "observation" | "outdoor" => Some(TestRoomKind::ObservationDeck),
                "er01" | "restroom" | "toilet" => Some(TestRoomKind::RestroomStart),
                "rr21" | "boiler" => Some(TestRoomKind::HotelBoilerRoom),
                "runway" | "fog" => Some(TestRoomKind::SyntheticFogRunway),
                "fallback" | "sandbox" => Some(TestRoomKind::FallbackSandbox),
                "viewer" | "model-viewer" | "models" | "bestiary" => {
                    Some(TestRoomKind::ModelAndTextureViewer)
                }
                "fmv" | "fmv-viewer" | "cutscenes" | "movies" => {
                    Some(TestRoomKind::FmvCutsceneViewer)
                }
                "combat" | "arena" | "battle" => Some(TestRoomKind::CombatArena),
                "grapple" | "grapples" | "grapple-room" | "grapple-viewer" => {
                    Some(TestRoomKind::GrappleAnimationRoom)
                }
                _ => Some(TestRoomKind::ObservationDeck),
            };
            i += 1;
        }
        i += 1;
    }

    if let Some(r) = initial_test_room {
        tracing::info!("CLI parameter selected test room: {:?}", r);
    }
    if let Some(ref m) = initial_viewer_model {
        tracing::info!("CLI parameter selected initial model viewer entry: '{}'", m);
    }
    if let Some(ref f) = initial_fmv_movie {
        tracing::info!("CLI parameter selected initial FMV cutscene: '{}'", f);
    }

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp {
        state: None,
        initial_test_room,
        initial_fog_debug,
        initial_viewer_model,
        initial_fmv_movie,
    };
    event_loop.run_app(&mut app).expect("Event loop error");
}
