//! Interactive Desktop Playable Test Build for Silent Hill 2 Open Port.
//! Runs the pure PS2 camera, 60 Hz simulation core, volumetric fog, character skeletal animation, and gamepad controls.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use sh2_app::AppMenuController;
use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices, evaluate_pose, interpolate_frames, parse_anm,
    AnimationFile,
};
use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_core::{ControlScheme, ScenarioKind, SimulationCore};
use sh2_renderer::{GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig, SurfaceBlitter};
use sh2_ui::menu::{MenuNavAction, MenuSoundCue};

/// Loaded character asset representation (model hierarchy, bind locals, and animation tracks).
struct CharacterAsset {
    #[allow(dead_code)]
    name: &'static str,
    gpu_mesh: GpuSkeletalMesh,
    model: ModelFile,
    bind_locals: Vec<[[f32; 4]; 4]>,
    anm: AnimationFile,
}

/// Generate an authentic 3D Silent Hill enclosed room with floor, textured walls, ceiling, and columns.
fn create_fallback_room_geometry() -> MapFile {
    let mut map = MapFile::default();
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 0,
        color: 0xFF888888,
        overlay: 0,
        specularity: 0.5,
    });

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let mut add_quad =
        |p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], normal: [f32; 3], col: [u8; 4]| {
            let base_idx = positions.len() as u32;
            positions.extend_from_slice(&[p0, p1, p2, p3]);
            normals.extend_from_slice(&[normal, normal, normal, normal]);
            uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            colors.extend_from_slice(&[col, col, col, col]);
            indices.extend_from_slice(&[
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx,
                base_idx + 2,
                base_idx + 3,
            ]);
        };

    let w = 8000.0;
    let l = 12000.0;
    let h = -2400.0; // Y down in PS2 coordinate system

    // Floor (Y = 0.0, normal points UP = -Y)
    add_quad(
        [-w, 0.0, -l],
        [w, 0.0, -l],
        [w, 0.0, l],
        [-w, 0.0, l],
        [0.0, -1.0, 0.0],
        [160, 150, 140, 255],
    );

    // Ceiling (Y = h, normal points DOWN = +Y)
    add_quad(
        [-w, h, l],
        [w, h, l],
        [w, h, -l],
        [-w, h, -l],
        [0.0, 1.0, 0.0],
        [110, 100, 95, 255],
    );

    // North Wall (Z = -l)
    add_quad(
        [-w, h, -l],
        [w, h, -l],
        [w, 0.0, -l],
        [-w, 0.0, -l],
        [0.0, 0.0, 1.0],
        [180, 170, 160, 255],
    );

    // South Wall (Z = l)
    add_quad(
        [w, h, l],
        [-w, h, l],
        [-w, 0.0, l],
        [w, 0.0, l],
        [0.0, 0.0, -1.0],
        [180, 170, 160, 255],
    );

    // West Wall (X = -w)
    add_quad(
        [-w, h, l],
        [-w, h, -l],
        [-w, 0.0, -l],
        [-w, 0.0, l],
        [1.0, 0.0, 0.0],
        [175, 165, 155, 255],
    );

    // East Wall (X = w)
    add_quad(
        [w, h, -l],
        [w, h, l],
        [w, 0.0, l],
        [w, 0.0, -l],
        [-1.0, 0.0, 0.0],
        [175, 165, 155, 255],
    );

    // Pillar / Column in center of room
    let pw = 600.0;
    let pz = -3000.0;
    // Front face
    add_quad(
        [-pw, h, pz - pw],
        [pw, h, pz - pw],
        [pw, 0.0, pz - pw],
        [-pw, 0.0, pz - pw],
        [0.0, 0.0, -1.0],
        [220, 200, 180, 255],
    );
    // Back face
    add_quad(
        [pw, h, pz + pw],
        [-pw, h, pz + pw],
        [-pw, 0.0, pz + pw],
        [pw, 0.0, pz + pw],
        [0.0, 0.0, 1.0],
        [220, 200, 180, 255],
    );
    // Left face
    add_quad(
        [-pw, h, pz + pw],
        [-pw, h, pz - pw],
        [-pw, 0.0, pz - pw],
        [-pw, 0.0, pz + pw],
        [-1.0, 0.0, 0.0],
        [200, 180, 160, 255],
    );
    // Right face
    add_quad(
        [pw, h, pz - pw],
        [pw, h, pz + pw],
        [pw, 0.0, pz + pw],
        [pw, 0.0, pz - pw],
        [1.0, 0.0, 0.0],
        [200, 180, 160, 255],
    );

    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 1,
        positions,
        uvs,
        colors,
        normals,
        indices,
    });

    map
}

/// Locate authentic Silent Hill 2 level map on disk
fn find_map_file() -> Option<(PathBuf, Vec<u8>)> {
    let candidate_paths = [
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map",
        "/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map",
        "/home/matthewb/dev/SH2/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map",
    ];

    for path in &candidate_paths {
        let p = PathBuf::from(path);
        if let Ok(bytes) = std::fs::read(&p) {
            return Some((p, bytes));
        }
    }
    None
}

/// Locate authentic character model and animation on disk
fn load_character_asset(
    ctx: &RenderContext,
    character_name: &'static str,
    mdl_rel_paths: &[&str],
    anm_rel_paths: &[&str],
) -> Option<CharacterAsset> {
    let base_roots = [
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "/home/matthewb/dev/SH2/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
    ];

    let mut mdl_bytes = None;
    let mut anm_bytes = None;

    for base in &base_roots {
        for mdl_sub in mdl_rel_paths {
            let p = PathBuf::from(base).join(mdl_sub);
            if let Ok(bytes) = std::fs::read(&p) {
                tracing::info!("Found character model: {:?}", p);
                mdl_bytes = Some(bytes);
                break;
            }
        }
        if mdl_bytes.is_some() {
            break;
        }
    }

    for base in &base_roots {
        for anm_sub in anm_rel_paths {
            let p = PathBuf::from(base).join(anm_sub);
            if let Ok(bytes) = std::fs::read(&p) {
                tracing::info!("Found character animation: {:?}", p);
                anm_bytes = Some(bytes);
                break;
            }
        }
        if anm_bytes.is_some() {
            break;
        }
    }

    let (mdl_bytes, anm_bytes) = (mdl_bytes?, anm_bytes?);
    let model = parse_mdl(&mdl_bytes).ok()?;
    let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).ok()?;
    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let gpu_mesh = ctx.compile_skeletal_mesh(&model);

    tracing::info!(
        "Loaded character '{}': bones={}, primitives={}, vertices={}, anm_frames={}",
        character_name,
        model.bone_count,
        model.primitives.len(),
        model.vertices.positions.len(),
        anm.frames.len()
    );

    Some(CharacterAsset {
        name: character_name,
        gpu_mesh,
        model,
        bind_locals,
        anm,
    })
}

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_ctx: RenderContext,
    blitter: SurfaceBlitter,
    blit_bind_group: wgpu::BindGroup,
    sim_core: SimulationCore,
    gpu_map: GpuMap,
    character: Option<CharacterAsset>,
    maria: Option<CharacterAsset>,
    active_maria: bool,
    menu_ctrl: AppMenuController,
    key_forward: bool,
    key_backward: bool,
    key_left: bool,
    key_right: bool,
    last_menu_state_log: String,
    last_frame: Instant,
}

#[derive(Default)]
struct SilentHillApp {
    state: Option<AppState>,
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
            .create_surface(window.clone())
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
            device.clone(),
            queue.clone(),
            render_config,
        );

        let (map_file, is_authentic) = match find_map_file() {
            Some((path, bytes)) => {
                tracing::info!("Loaded authentic level geometry from {:?}", path);
                (
                    MapFile::parse(&bytes).unwrap_or_else(|_| create_fallback_room_geometry()),
                    true,
                )
            }
            None => {
                tracing::info!("Built-in 3D Silent Hill room geometry generated.");
                (create_fallback_room_geometry(), false)
            }
        };

        let gpu_map = GpuMap::from_map(&device, &map_file);
        let blitter = SurfaceBlitter::new(&device, surface_format);
        let blit_bind_group = blitter.create_bind_group(&device, &render_ctx.ldr_output_view);

        // Load James Sunderland character model & animations
        let character = load_character_asset(
            &render_ctx,
            "James Sunderland",
            &["data/chr/jms/hll_jms.mdl", "data/chr/jms/hhh_jms.mdl"],
            &["data/chr/jms/jms_basic1.anm"],
        );

        // Load Maria character model & animations (for character switching or Born From a Wish)
        let maria = load_character_asset(
            &render_ctx,
            "Maria",
            &["data/chr/mar/lll_mar.mdl", "data/chr/mar/hhh_mar.mdl"],
            &["data/chr2/mar/xmar_wpnone.anm", "data/chr/mar/lll_mar.anm"],
        );

        let mut sim_core = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim_core.input.set_control_scheme(ControlScheme::Tank);
        sim_core.scenario.flashlight_on = true;

        if is_authentic {
            // rr21 authentic bathroom floor coordinates (X: -100200, Y: -100, Z: -60000)
            sim_core.scenario.player_position = glam::Vec3::new(-100200.0, -100.0, -60000.0);
            sim_core.scenario.player_yaw = 0.0; // Face forward into the bathroom (+Z)
        } else {
            // Built-in 3D room center
            sim_core.scenario.player_position = glam::Vec3::new(0.0, -800.0, 2000.0);
            sim_core.scenario.player_yaw = std::f32::consts::PI; // Face toward the room and column
        }
        sim_core.camera.reset_to_player(
            sim_core.scenario.player_position,
            sim_core.scenario.player_yaw,
        );

        // Atmospheric Silent Hill lighting & fog (calibrated for room size)
        let fog_uniforms = sh2_renderer::fog::FogUniforms {
            fog_color: [0.08, 0.09, 0.11, 1.0],
            fog_near: 800.0,
            fog_far: 6000.0,
            density: 0.35,
            _padding: 0.0,
        };
        render_ctx.update_fog(&fog_uniforms);

        tracing::info!(
            "Simulation, Character Models, and Renderer initialized successfully on Vulkan!"
        );
        tracing::info!("Controls: WASD / Stick: Move & Turn (Tank) | Shift: Run | F: Flashlight | C: Switch Character (James/Maria)");

        let menu_ctrl = AppMenuController::new("config.json");

        self.state = Some(AppState {
            window,
            surface,
            surface_config,
            render_ctx,
            blitter,
            blit_bind_group,
            sim_core,
            gpu_map,
            character,
            maria,
            active_maria: false,
            menu_ctrl,
            key_forward: false,
            key_backward: false,
            key_left: false,
            key_right: false,
            last_menu_state_log: String::new(),
            last_frame: Instant::now(),
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
                                _ => MenuSoundCue::None,
                            };
                            if sound != MenuSoundCue::None {
                                tracing::info!("Menu sound emitted: {:?}", sound);
                            }
                        }
                        return;
                    }

                    match key_code {
                        winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => {
                            state.key_forward = pressed;
                        }
                        winit::keyboard::KeyCode::KeyS | winit::keyboard::KeyCode::ArrowDown => {
                            state.key_backward = pressed;
                        }
                        winit::keyboard::KeyCode::KeyA | winit::keyboard::KeyCode::ArrowLeft => {
                            state.key_left = pressed;
                        }
                        winit::keyboard::KeyCode::KeyD | winit::keyboard::KeyCode::ArrowRight => {
                            state.key_right = pressed;
                        }
                        winit::keyboard::KeyCode::ShiftLeft
                        | winit::keyboard::KeyCode::ShiftRight => {
                            state.sim_core.input.state.is_running = pressed;
                        }
                        winit::keyboard::KeyCode::KeyF if pressed => {
                            state.sim_core.scenario.flashlight_on =
                                !state.sim_core.scenario.flashlight_on;
                        }
                        winit::keyboard::KeyCode::KeyC if pressed => {
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
                        _ => {}
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

                    state.sim_core.input.state.left_stick_y = kb_y;
                    state.sim_core.input.state.left_stick_x = kb_x;
                }
            }
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested, shutting down...");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state
                        .surface
                        .configure(&state.render_ctx.device, &state.surface_config);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame);
                state.last_frame = now;

                // Advance menu transitions and fade state
                state.menu_ctrl.tick(dt.as_secs_f32());

                // When menu is active, log formatted state changes so console/headless/testing builds observe menu navigation
                if !state.menu_ctrl.in_game_active {
                    let current_text = state.menu_ctrl.overlay_text().join(" | ");
                    if current_text != state.last_menu_state_log {
                        tracing::info!("[SH2 MENU HUD] {}", current_text);
                        state.last_menu_state_log = current_text;
                    }
                }

                // 1. Advance simulation at fixed 60 Hz tick (runs animation controller update)
                state.sim_core.update(dt);

                // 2. Compute smooth sub-frame interpolated view matrix for high refresh OLED
                let alpha = state.sim_core.interpolation_alpha();
                let view = state.sim_core.camera.interpolated_view_matrix(alpha);
                let fov = state.sim_core.camera.interpolated_fov(alpha);
                let aspect =
                    state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;
                let proj = glam::Mat4::perspective_rh(fov, aspect, 10.0, 30000.0);
                let view_proj = proj * view;
                let inv_proj = proj.inverse();

                let cam_pos = state.sim_core.camera.curr_snapshot.eye;
                state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);

                // 3. Dynamic Flashlight tracking character position & facing angle
                let player_pos = state.sim_core.scenario.player_position;
                let player_yaw = state.sim_core.scenario.player_yaw;
                let light_dir =
                    glam::Vec3::new(player_yaw.sin(), 0.05, player_yaw.cos()).normalize_or_zero();
                let spot_pos = player_pos + glam::Vec3::new(0.0, -800.0, 0.0);

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

                // 4. Sample skeletal animation and compute character skinning matrices
                let active_char = if state.active_maria {
                    state.maria.as_ref().or(state.character.as_ref())
                } else {
                    state.character.as_ref().or(state.maria.as_ref())
                };

                let mut skin_matrices = Vec::new();
                if let Some(c) = active_char {
                    let (prev_cursor, cur_cursor, blend) =
                        state.sim_core.animation.evaluate_blended_cursor();

                    let frame_count = c.anm.frames.len();
                    if frame_count > 0 {
                        // Clamp current cursor frame indices to available track
                        let f0 = (cur_cursor.0 as usize) % frame_count;
                        let f1 = (cur_cursor.1 as usize) % frame_count;
                        let mut blended_frame =
                            interpolate_frames(&c.anm.frames[f0], &c.anm.frames[f1], cur_cursor.2);

                        // If in transition crossfade, blend with previous clip pose
                        if let Some(prev) = prev_cursor {
                            let pf0 = (prev.0 as usize) % frame_count;
                            let pf1 = (prev.1 as usize) % frame_count;
                            let prev_frame =
                                interpolate_frames(&c.anm.frames[pf0], &c.anm.frames[pf1], prev.2);
                            blended_frame = interpolate_frames(&prev_frame, &blended_frame, blend);
                        }

                        let pose = evaluate_pose(&c.model.parents, &c.bind_locals, &blended_frame);
                        let local_skin = compute_skin_matrices(&pose, &c.model.initial_matrices);

                        // Transform character mesh from model space to player world space:
                        // World Matrix = Translation(player_pos) * RotationY(player_yaw)
                        let (s, c_yaw) = player_yaw.sin_cos();
                        let world_mat = [
                            [c_yaw, 0.0, s, player_pos.x],
                            [0.0, 1.0, 0.0, player_pos.y],
                            [-s, 0.0, c_yaw, player_pos.z],
                            [0.0, 0.0, 0.0, 1.0],
                        ];

                        skin_matrices.reserve(local_skin.len());
                        for m_skin in &local_skin {
                            // C = World * M_skin
                            let mut combined = [[0.0; 4]; 4];
                            for r in 0..4 {
                                for col in 0..4 {
                                    combined[r][col] = world_mat[r][0] * m_skin[0][col]
                                        + world_mat[r][1] * m_skin[1][col]
                                        + world_mat[r][2] * m_skin[2][col]
                                        + world_mat[r][3] * m_skin[3][col];
                                }
                            }
                            skin_matrices.push(combined);
                        }
                    }
                }

                // 5. Render level map and skinned character model in a unified scene pass
                let skeletal_draw: Vec<(&GpuSkeletalMesh, &[[[f32; 4]; 4]])> =
                    if let Some(c) = active_char {
                        if !skin_matrices.is_empty() {
                            vec![(&c.gpu_mesh, &skin_matrices)]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    };

                state
                    .render_ctx
                    .render_scene(Some(&state.gpu_map), &skeletal_draw);

                // 6. Present to window surface swapchain
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

                state.blitter.render_with_bind_group(
                    &mut encoder,
                    &state.blit_bind_group,
                    &surface_view,
                );

                state
                    .render_ctx
                    .queue
                    .submit(std::iter::once(encoder.finish()));
                surface_texture.present();

                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(
        "Starting Silent Hill 2 Open Port Interactive Playable Test with Skeletal Mesh Runtime..."
    );

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp::default();
    event_loop.run_app(&mut app).expect("Event loop error");
}
