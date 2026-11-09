//! Application initialization and asset bootstrap for `AppState` (`bootstrap_app_state`).

use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::assets::{
    build_map_texture_bind_groups, find_cam_file_for_room, find_cld_file_for_room,
    find_map_file_for_room, load_anm_file, load_character_asset, load_viewer_entry_asset,
};
use crate::cli::CliArgs;
use crate::rooms;
use crate::rooms::fmv_room::open_fmv_session;
use crate::state::AppState;
use sh2_app::AppMenuController;
use sh2_assets::formats::TextureLoader;
use sh2_audio::{AudioEngine, BgmManager, MenuAudioBank};
use sh2_core::{ControlScheme, ScenarioKind, SimulationCore};
use sh2_debug::{FmvViewerState, GrappleViewerState, ModelViewerState};
use sh2_renderer::{FmvVideoPipeline, GpuMap, RenderContext, RendererConfig, SurfaceBlitter};
use sh2_ui::menu::{TestRoomKind, TitlePhase};
use sh2_ui::{ButtonSpriteSheet, TextRasterizer};

/// Initializes the Vulkan window surface, `RenderContext`, audio engine, character skeletons,
/// bestiary models, and UI pipelines into a ready-to-run [`AppState`].
pub(crate) fn bootstrap_app_state(event_loop: &ActiveEventLoop, cli: &CliArgs) -> AppState {
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

    let initial_room = cli
        .initial_test_room
        .unwrap_or(TestRoomKind::ObservationDeck);
    let (map_file, player_pos, player_yaw, fog_uniforms) = find_map_file_for_room(initial_room);

    let gpu_map = GpuMap::from_map(&device, &map_file);
    let blitter = SurfaceBlitter::new(&device, surface_format);
    let blit_bind_group = blitter.create_bind_group(&device, &render_ctx.ldr_output_view);

    // Build level diffuse texture bind groups per material (cached by texture_id to avoid redundant VRAM allocations)
    let map_texture_bind_groups = build_map_texture_bind_groups(&render_ctx, &map_file);

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
            if let Some(anm) = load_anm_file(src.rel_path(), c.model.bone_count, &c.model.parents) {
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

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let data_dir = resolver.data_dir.clone();
    let ee_dir = resolver.sh2e_dir.clone();
    let detected_audio_packs =
        sh2_core::AudioPackDetectionStatus::detect_from_root(&resolver.game_root);
    tracing::info!(
        "[Audio Pack Detection] {} (SH2:EE Lossless: {}, PS2 Original: {}, SH2:EE Extra: {})",
        detected_audio_packs.status_label(),
        detected_audio_packs.sh2ee_lossless_present,
        detected_audio_packs.ps2_original_present,
        detected_audio_packs.sh2ee_extra_present
    );

    let host_devices = AudioEngine::enumerate_output_devices();
    let config_candidates = ["config.json", "../config.json"];
    let chosen_config_path = config_candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .unwrap_or(&"config.json");
    let mut menu_ctrl =
        AppMenuController::new_with_audio_packs(chosen_config_path, detected_audio_packs);
    menu_ctrl
        .options_menu
        .set_audio_devices(host_devices.clone());

    let initial_audio_pack = menu_ctrl.resolved_audio_pack_source();
    let audio_bank = Some(MenuAudioBank::load_from_dirs_for_source(
        &data_dir,
        ee_dir.as_deref(),
        initial_audio_pack,
    ));

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

    if let Some(room) = cli.initial_test_room {
        menu_ctrl.title_sm.selected_test_room = room;
        if let Some(idx) = TestRoomKind::ALL.iter().position(|r| *r == room) {
            menu_ctrl.title_sm.test_room_index = idx;
        }
    }
    if cli.initial_test_room.is_some() || cli.initial_fog_debug {
        // Immediate boot into the requested test room if specified via CLI
        menu_ctrl.in_game_active = true;
        menu_ctrl.title_sm.phase = TitlePhase::TransitionToGame;
        menu_ctrl.title_sm.fade_alpha = 0.0;
    }

    let mut model_viewer = ModelViewerState::new();
    if let Some(ref model_query) = cli.initial_viewer_model {
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
    fmv_viewer.enable_restored_cut_audio = menu_ctrl
        .options_menu
        .original_config
        .audio
        .enable_restored_cut_audio;
    if let Some(ref movie_query) = cli.initial_fmv_movie {
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
    let initial_fps_cap = menu_ctrl.options_menu.original_config.video.framerate_cap;
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
            initial_audio_pack,
        )
    } else {
        None
    };
    let loaded_fmv_movie_idx = fmv_viewer.selected_movie_index;
    let loaded_fmv_source = fmv_viewer.video_source;

    let mut bgm_manager = BgmManager::new(&resolver.game_root, initial_audio_pack);
    bgm_manager.event_flags.enable_restored_cut_audio = menu_ctrl
        .options_menu
        .original_config
        .audio
        .enable_restored_cut_audio;

    AppState {
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
        bgm_manager,
        bgm_stage_override: None,
        audio_debug: rooms::audio_room::AudioDebugState::default(),
        surface_sfx_room: rooms::surface_sfx_room::SurfaceSfxRoomState::default(),
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
        drama_demo_session: None,
        selected_drama_demo_idx: 0,
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
    }
}
