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
use sh2_assets::formats::TextureLoader;
use sh2_audio::{play_menu_sound, AudioEngine, MenuAudioBank, SoundCueKind};
use sh2_core::{ControlScheme, ScenarioKind, SecondaryBonePhysics, SimulationCore};
use sh2_renderer::{GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig, SurfaceBlitter};
use sh2_ui::menu::{MainMenuOption, MenuNavAction, MenuSoundCue, TestRoomKind, TitlePhase};
use sh2_ui::{AuthenticButtonSprite, ButtonSpriteSheet, TextRasterizer};

/// Loaded character asset representation (model hierarchy, bind locals, and animation tracks).
struct CharacterAsset {
    #[allow(dead_code)]
    name: &'static str,
    gpu_mesh: GpuSkeletalMesh,
    model: ModelFile,
    bind_locals: Vec<[[f32; 4]; 4]>,
    anm: AnimationFile,
    texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    secondary_physics: SecondaryBonePhysics,
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

/// Generate an extended outdoor synthetic fog calibration runway with distance markers.
/// Floor runs from Z = 0 to Z = 120,000 (120m) with crossbar markers at 5m, 10m, 25m, 50m, 75m, 100m
/// and vertical light poles / torches at measured intervals to calibrate volumetric fog extinction.
fn create_synthetic_fog_runway_geometry() -> MapFile {
    let mut map = MapFile::default();
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 0,
        color: 0xFF555555,
        overlay: 0,
        specularity: 0.2,
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

    let w = 4000.0; // 4 meters wide
    let length = 120000.0; // 120 meters long runway

    // Main asphalt runway floor (Y = 0, normal points UP = -Y in PS2 coordinate space)
    add_quad(
        [-w, 0.0, -5000.0],
        [w, 0.0, -5000.0],
        [w, 0.0, length],
        [-w, 0.0, length],
        [0.0, -1.0, 0.0],
        [70, 75, 80, 255],
    );

    // Distance marker crossbars every 5m, 10m, 25m, 50m, 75m, 100m
    let distance_markers = [5000.0, 10000.0, 25000.0, 50000.0, 75000.0, 100000.0];
    for &dist in &distance_markers {
        let m_thick = 300.0;
        let col = if dist == 50000.0 || dist == 100000.0 {
            [240, 220, 50, 255] // Bright yellow major line
        } else {
            [220, 220, 220, 255] // White distance marker line
        };

        add_quad(
            [-w * 0.9, -2.0, dist - m_thick],
            [w * 0.9, -2.0, dist - m_thick],
            [w * 0.9, -2.0, dist + m_thick],
            [-w * 0.9, -2.0, dist + m_thick],
            [0.0, -1.0, 0.0],
            col,
        );

        // Measured distance marker pylons on left and right sides
        for &side in &[-1.0f32, 1.0f32] {
            let px = side * (w + 800.0);
            let pz = dist;
            let ph = -3200.0; // 3.2m tall light pole
            let pw = 150.0;

            // Light pole column
            add_quad(
                [px - pw, ph, pz - pw],
                [px + pw, ph, pz - pw],
                [px + pw, 0.0, pz - pw],
                [px - pw, 0.0, pz - pw],
                [0.0, 0.0, -1.0],
                [180, 140, 60, 255],
            );
            add_quad(
                [px + pw, ph, pz + pw],
                [px - pw, ph, pz + pw],
                [px - pw, 0.0, pz + pw],
                [px + pw, 0.0, pz + pw],
                [0.0, 0.0, 1.0],
                [180, 140, 60, 255],
            );
            // Lamp glowing beacon at top of pole
            let bw = 250.0;
            add_quad(
                [px - bw, ph - bw * 2.0, pz - bw],
                [px + bw, ph - bw * 2.0, pz - bw],
                [px + bw, ph, pz - bw],
                [px - bw, ph, pz - bw],
                [0.0, 0.0, -1.0],
                [255, 230, 160, 255],
            );
        }
    }

    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 2,
        positions,
        uvs,
        colors,
        normals,
        indices,
    });

    map
}

/// Locate authentic Silent Hill 2 level map on disk based on requested test room or fallback
fn find_map_file_for_room(
    room: TestRoomKind,
) -> (MapFile, glam::Vec3, f32, sh2_renderer::fog::FogUniforms) {
    let base_roots = [
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "/home/matthewb/dev/SH2/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
    ];

    match room {
        TestRoomKind::ObservationDeck => {
            for base in &base_roots {
                let p = PathBuf::from(base).join("data/bg/ob/ob01.map");
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Ok(map) = MapFile::parse(&bytes) {
                        tracing::info!("Loaded authentic Observation Deck level geometry: {:?}", p);
                        // Toluca Lake vista outdoor fog parameters
                        let fog = sh2_renderer::fog::FogUniforms {
                            fog_color: [0.65, 0.68, 0.72, 1.0], // Daylight Toluca Lake mist
                            fog_near: 1500.0,
                            fog_far: 18000.0,
                            density: 0.85,
                            time: 0.0,
                            step_count: 32,
                            curve_type: 1, // Exponential curve
                            speed_mult: 1.0,
                            _padding: 0.0,
                        };
                        // Authentic parking lot overlook coordinates: X: 0.0, Y: -100.0, Z: -5000.0
                        return (map, glam::Vec3::new(0.0, -100.0, -5000.0), 0.0, fog);
                    }
                }
            }
            tracing::warn!("Failed to load ob01.map; using synthetic fog runway fallback");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.65, 0.68, 0.72, 1.0],
                fog_near: 2000.0,
                fog_far: 25000.0,
                density: 0.80,
                time: 0.0,
                step_count: 32,
                curve_type: 1,
                speed_mult: 1.0,
                _padding: 0.0,
            };
            (
                create_synthetic_fog_runway_geometry(),
                glam::Vec3::new(0.0, -100.0, 0.0),
                0.0,
                fog,
            )
        }
        TestRoomKind::RestroomStart => {
            for base in &base_roots {
                let p = PathBuf::from(base).join("data/bg/er/er01.map");
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Ok(map) = MapFile::parse(&bytes) {
                        tracing::info!(
                            "Loaded authentic Opening Restroom level geometry (er01): {:?}",
                            p
                        );
                        let fog = sh2_renderer::fog::FogUniforms {
                            fog_color: [0.12, 0.13, 0.14, 1.0], // Dim authentic restroom lighting
                            fog_near: 800.0,
                            fog_far: 5000.0,
                            density: 0.30,
                            time: 0.0,
                            step_count: 24,
                            curve_type: 0,
                            speed_mult: 1.0,
                            _padding: 0.0,
                        };
                        // Authentic er01 restroom camera/spawn coordinates: X: -18968.0, Y: -1168.0, Z: 21025.0
                        return (map, glam::Vec3::new(-18968.0, -1168.0, 21025.0), 0.0, fog);
                    }
                }
            }
            tracing::warn!("Failed to load er01.map; using fallback room geometry");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.12, 0.13, 0.14, 1.0],
                fog_near: 800.0,
                fog_far: 5000.0,
                density: 0.30,
                time: 0.0,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                _padding: 0.0,
            };
            (
                create_fallback_room_geometry(),
                glam::Vec3::new(0.0, -800.0, 2000.0),
                std::f32::consts::PI,
                fog,
            )
        }
        TestRoomKind::HotelBoilerRoom => {
            for base in &base_roots {
                let p = PathBuf::from(base).join("data/bg/rr/rr21.map");
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Ok(map) = MapFile::parse(&bytes) {
                        tracing::info!("Loaded authentic Lakeview Hotel Boiler Room level geometry (rr21): {:?}", p);
                        let fog = sh2_renderer::fog::FogUniforms {
                            fog_color: [0.08, 0.09, 0.11, 1.0], // Dark basement boiler tone
                            fog_near: 800.0,
                            fog_far: 6000.0,
                            density: 0.35,
                            time: 0.0,
                            step_count: 24,
                            curve_type: 0,
                            speed_mult: 1.0,
                            _padding: 0.0,
                        };
                        // Authentic rr21 hotel boiler room coordinates: X: -100000.0, Y: -500.0, Z: -60000.0
                        return (map, glam::Vec3::new(-100000.0, -500.0, -60000.0), 0.0, fog);
                    }
                }
            }
            tracing::warn!("Failed to load rr21.map; using fallback room geometry");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.08, 0.09, 0.11, 1.0],
                fog_near: 800.0,
                fog_far: 6000.0,
                density: 0.35,
                time: 0.0,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                _padding: 0.0,
            };
            (
                create_fallback_room_geometry(),
                glam::Vec3::new(0.0, -800.0, 2000.0),
                std::f32::consts::PI,
                fog,
            )
        }
        TestRoomKind::SyntheticFogRunway => {
            tracing::info!("Generated Synthetic Fog Runway with calibrated distance markers.");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.60, 0.64, 0.70, 1.0],
                fog_near: 2000.0,
                fog_far: 30000.0,
                density: 0.85,
                time: 0.0,
                step_count: 32,
                curve_type: 1, // Exponential
                speed_mult: 1.0,
                _padding: 0.0,
            };
            (
                create_synthetic_fog_runway_geometry(),
                glam::Vec3::new(0.0, -100.0, 0.0),
                0.0,
                fog,
            )
        }
        TestRoomKind::FallbackSandbox => {
            tracing::info!("Generated built-in Fallback Sandbox room.");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.08, 0.09, 0.11, 1.0],
                fog_near: 800.0,
                fog_far: 6000.0,
                density: 0.35,
                time: 0.0,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                _padding: 0.0,
            };
            (
                create_fallback_room_geometry(),
                glam::Vec3::new(0.0, -800.0, 2000.0),
                std::f32::consts::PI,
                fog,
            )
        }
    }
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

    // Build diffuse texture bind groups indexed by primitive texture_index
    let max_tex_idx = model
        .primitives
        .iter()
        .map(|pr| pr.texture_index as usize)
        .max()
        .unwrap_or(0);
    let mut texture_bind_groups = Vec::with_capacity(max_tex_idx + 1);
    for tidx in 0..=max_tex_idx {
        if let Some(tex) = model.textures.get(&(tidx as u32)) {
            if let Ok(rgba) = tex.to_rgba() {
                let bg = ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&format!("{character_name} Texture {tidx}")),
                );
                texture_bind_groups.push(bg);
                continue;
            }
        }
        texture_bind_groups.push(ctx.default_texture_bind_group.clone());
    }

    tracing::info!(
        "Loaded character '{}': bones={}, primitives={}, vertices={}, textures={}, anm_frames={}",
        character_name,
        model.bone_count,
        model.primitives.len(),
        model.vertices.positions.len(),
        model.textures.len(),
        anm.frames.len()
    );

    let secondary_physics = if character_name.contains("Maria") {
        SecondaryBonePhysics::for_maria()
    } else {
        SecondaryBonePhysics::for_james()
    };

    Some(CharacterAsset {
        name: character_name,
        gpu_mesh,
        model,
        bind_locals,
        anm,
        texture_bind_groups,
        secondary_physics,
    })
}

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_ctx: RenderContext,
    blitter: SurfaceBlitter,
    blit_bind_group: wgpu::BindGroup,
    splash_konami_bg: Option<Arc<wgpu::BindGroup>>,
    splash_title_bg: Option<Arc<wgpu::BindGroup>>,
    menu_uniform_buffer: wgpu::Buffer,
    menu_text_texture: wgpu::Texture,
    menu_text_bind_group: wgpu::BindGroup,
    menu_rasterizer: TextRasterizer,
    button_sprites: Option<ButtonSpriteSheet>,
    audio_bank: Option<MenuAudioBank>,
    audio_engine: Option<AudioEngine>,
    sim_core: SimulationCore,
    gpu_map: GpuMap,
    map_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    character: Option<CharacterAsset>,
    maria: Option<CharacterAsset>,
    active_maria: bool,
    active_test_room: TestRoomKind,
    fog_uniforms: sh2_renderer::fog::FogUniforms,
    menu_ctrl: AppMenuController,
    key_forward: bool,
    key_backward: bool,
    key_left: bool,
    key_right: bool,
    last_menu_state_log: String,
    last_frame: Instant,
    frame_counter: u32,
    sim_time: f32,
}

#[derive(Default)]
struct SilentHillApp {
    state: Option<AppState>,
    initial_test_room: Option<TestRoomKind>,
    initial_fog_debug: bool,
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
                map_texture_bind_groups.push(bg.clone());
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
                    tex_bg_cache.insert(tex_id, bg.clone());
                    map_texture_bind_groups.push(bg);
                    continue;
                }
            }
            map_texture_bind_groups.push(render_ctx.default_texture_bind_group.clone());
        }

        // Load James Sunderland character model & animations (prioritizing high-fidelity 71-bone hhh_jms.mdl)
        let character = load_character_asset(
            &render_ctx,
            "James Sunderland",
            &["data/chr/jms/hhh_jms.mdl", "data/chr/jms/hll_jms.mdl"],
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
        sim_core.scenario.player_position = player_pos;
        sim_core.scenario.player_yaw = player_yaw;
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

        let audio_engine = match AudioEngine::new() {
            Ok(engine) => {
                tracing::info!("Audio engine successfully initialized with Kira runtime.");
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

        let mut menu_ctrl = AppMenuController::new("config.json");
        if self.initial_test_room.is_some() || self.initial_fog_debug {
            // Immediate boot into the requested test room if specified via CLI
            menu_ctrl.in_game_active = true;
            menu_ctrl.title_sm.phase = TitlePhase::TransitionToGame;
            menu_ctrl.title_sm.fade_alpha = 0.0;
        }

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
            active_maria: false,
            active_test_room: initial_room,
            fog_uniforms,
            menu_ctrl,
            key_forward: false,
            key_backward: false,
            key_left: false,
            key_right: false,
            last_menu_state_log: String::new(),
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
                                _ => {
                                    // Any key advances from Splash or Title Logo
                                    match state.menu_ctrl.title_sm.phase {
                                        TitlePhase::SplashIntro | TitlePhase::TitleLogo => {
                                            state.menu_ctrl.handle_nav(MenuNavAction::Confirm)
                                        }
                                        _ => MenuSoundCue::None,
                                    }
                                }
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
                        // Volumetric Fog calibration shortcuts (matching PS2 dbg_fogparm_ctl / dbg_ambifog_ctl)
                        winit::keyboard::KeyCode::BracketLeft if pressed => {
                            state.fog_uniforms.density =
                                (state.fog_uniforms.density - 0.05).max(0.0);
                            state.render_ctx.update_fog(&state.fog_uniforms);
                            tracing::info!(
                                "[Fog Tweak] Density: {:.2}",
                                state.fog_uniforms.density
                            );
                        }
                        winit::keyboard::KeyCode::BracketRight if pressed => {
                            state.fog_uniforms.density =
                                (state.fog_uniforms.density + 0.05).min(2.0);
                            state.render_ctx.update_fog(&state.fog_uniforms);
                            tracing::info!(
                                "[Fog Tweak] Density: {:.2}",
                                state.fog_uniforms.density
                            );
                        }
                        winit::keyboard::KeyCode::Minus if pressed => {
                            state.fog_uniforms.fog_far = (state.fog_uniforms.fog_far - 1000.0)
                                .max(state.fog_uniforms.fog_near + 500.0);
                            state.render_ctx.update_fog(&state.fog_uniforms);
                            tracing::info!(
                                "[Fog Tweak] fog_far: {:.0}",
                                state.fog_uniforms.fog_far
                            );
                        }
                        winit::keyboard::KeyCode::Equal if pressed => {
                            state.fog_uniforms.fog_far =
                                (state.fog_uniforms.fog_far + 1000.0).min(60000.0);
                            state.render_ctx.update_fog(&state.fog_uniforms);
                            tracing::info!(
                                "[Fog Tweak] fog_far: {:.0}",
                                state.fog_uniforms.fog_far
                            );
                        }
                        winit::keyboard::KeyCode::KeyM if pressed => {
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
            WindowEvent::MouseInput {
                state: element_state,
                ..
            } => {
                if element_state == winit::event::ElementState::Pressed
                    && !state.menu_ctrl.in_game_active
                {
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
                    state.sim_core.scenario.player_position = new_pos;
                    state.sim_core.scenario.player_yaw = new_yaw;
                    state.sim_core.camera.reset_to_player(new_pos, new_yaw);
                    state.active_test_room = new_room;
                }

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
                // Authentic chest pocket mount: clipped to James' front breast pocket (Bone 3, Y ≈ -640, shifted forward and slightly left)
                let player_pos = state.sim_core.scenario.player_position;
                let player_yaw = state.sim_core.scenario.player_yaw;
                let forward = glam::Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
                let right = glam::Vec3::new(player_yaw.cos(), 0.0, -player_yaw.sin());
                let light_dir =
                    glam::Vec3::new(player_yaw.sin(), 0.05, player_yaw.cos()).normalize_or_zero();
                let spot_pos = player_pos
                    + glam::Vec3::new(0.0, -640.0, 0.0)
                    + forward * 45.0
                    + right * (-12.0);

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
                let mut active_char = if state.active_maria {
                    state.maria.as_mut().or(state.character.as_mut())
                } else {
                    state.character.as_mut().or(state.maria.as_mut())
                };

                let mut skin_matrices = Vec::new();
                if let Some(ref mut c) = active_char {
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

                        // Apply procedural secondary spring-damper inertia physics (coat tails, flaps, skirt, chest)
                        let dt_physics = 1.0 / 60.0;
                        c.secondary_physics.step(dt_physics, player_pos, player_yaw);
                        let mut dynamic_locals = c.bind_locals.clone();
                        c.secondary_physics.apply_to_locals(&mut dynamic_locals);

                        let pose = evaluate_pose(&c.model.parents, &dynamic_locals, &blended_frame);
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
                let skeletal_draw = if let Some(c) = active_char {
                    if !skin_matrices.is_empty() {
                        vec![(
                            &c.gpu_mesh,
                            skin_matrices.as_slice(),
                            Some(c.texture_bind_groups.as_slice()),
                        )]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                // 5. Render either 3D gameplay scene or 2D Authentic Splash / Title Screen
                let is_in_game = state.menu_ctrl.in_game_active;
                let fade_alpha = state.menu_ctrl.title_sm.fade_alpha;
                let phase = state.menu_ctrl.title_sm.phase;

                // Advance simulation time and frame counter for animated post-processing
                state.frame_counter = state.frame_counter.wrapping_add(1);
                state.sim_time += dt.as_secs_f32();

                // Update fog animation time and drift
                state.fog_uniforms.time = state.sim_time;
                state.render_ctx.update_fog(&state.fog_uniforms);

                // Retrieve active grain intensity percentage from options configuration
                let grain_pct = state
                    .menu_ctrl
                    .options_menu
                    .draft_config
                    .video
                    .grain_intensity;
                let grain_intensity = (grain_pct as f32) / 100.0;

                state
                    .render_ctx
                    .update_post(&sh2_renderer::post::PostUniforms {
                        time: state.sim_time,
                        grain_intensity,
                        exposure: 1.0,
                        frame_count: state.frame_counter,
                    });

                if is_in_game {
                    state.render_ctx.render_scene(
                        Some(&state.gpu_map),
                        Some(&state.map_texture_bind_groups),
                        &skeletal_draw,
                    );
                }

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

                if is_in_game {
                    state.blitter.render_with_bind_group(
                        &mut encoder,
                        &state.blit_bind_group,
                        &surface_view,
                    );
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

                        let white = [220, 220, 220, 255];
                        let yellow = [240, 205, 55, 255];
                        let muted = [150, 150, 150, 255];
                        let cyan = [100, 210, 230, 255];

                        match phase {
                            TitlePhase::TitleLogo => {
                                if let Some(ref atlas) = state.button_sprites {
                                    let center_x = (state.menu_rasterizer.width / 2) as i32;
                                    let rect = AuthenticButtonSprite::StartPrompt.atlas_rect();
                                    let scale = 0.44f32;
                                    let w = (rect.width as f32 * scale) as i32;
                                    let x = center_x - (w / 2);
                                    let y = 610;

                                    // Subtle pulse alpha matching title breathing glow
                                    let pulse = (state.menu_ctrl.title_sm.splash_timer * 3.5).sin()
                                        * 0.15
                                        + 0.85;

                                    atlas.blit_sprite(
                                        AuthenticButtonSprite::StartPrompt,
                                        &mut state.menu_rasterizer.pixels,
                                        state.menu_rasterizer.width,
                                        state.menu_rasterizer.height,
                                        x,
                                        y,
                                        scale,
                                        [1.0, 1.0, 1.0, pulse],
                                    );
                                } else {
                                    state.menu_rasterizer.draw_text(
                                        "PRESS ANY KEY OR ENTER TO START",
                                        410,
                                        620,
                                        2,
                                        yellow,
                                        true,
                                    );
                                }
                            }
                            TitlePhase::MainMenu => {
                                for (idx, opt) in MainMenuOption::ALL.iter().enumerate() {
                                    let is_selected =
                                        idx == state.menu_ctrl.title_sm.main_menu_index;
                                    let color = if is_selected { yellow } else { white };
                                    let y = 350 + idx * 45;
                                    let text = opt.label();
                                    if is_selected {
                                        state
                                            .menu_rasterizer
                                            .draw_text(">", 520, y, 3, yellow, true);
                                    }
                                    state
                                        .menu_rasterizer
                                        .draw_text(text, 550, y, 3, color, true);
                                }
                                state.menu_rasterizer.draw_text(
                                    "W / S or Up / Down to Navigate | Enter to Confirm",
                                    400,
                                    660,
                                    1,
                                    muted,
                                    true,
                                );
                            }
                            TitlePhase::ScenarioSelect => {
                                if let Some(ref atlas) = state.button_sprites {
                                    let center_x = (state.menu_rasterizer.width / 2) as i32;
                                    let scenarios = [
                                        (0, AuthenticButtonSprite::MainScenario),
                                        (1, AuthenticButtonSprite::SubScenario),
                                    ];
                                    for (idx, sprite) in &scenarios {
                                        let is_selected =
                                            *idx == state.menu_ctrl.title_sm.scenario_index;
                                        let y = 390 + *idx as i32 * 75;
                                        let rect = sprite.atlas_rect();
                                        let scale = 0.48f32;
                                        let w = (rect.width as f32 * scale) as i32;
                                        let x = center_x - (w / 2);

                                        if is_selected {
                                            let glow_rect =
                                                AuthenticButtonSprite::GlowCursor.atlas_rect();
                                            let glow_scale = 0.65f32;
                                            let glow_w =
                                                (glow_rect.width as f32 * glow_scale) as i32;
                                            let glow_h =
                                                (glow_rect.height as f32 * glow_scale) as i32;
                                            let glow_x = center_x - (glow_w / 2);
                                            let btn_h = (rect.height as f32 * scale) as i32;
                                            let glow_y = y + (btn_h / 2) - (glow_h / 2);
                                            atlas.blit_sprite(
                                                AuthenticButtonSprite::GlowCursor,
                                                &mut state.menu_rasterizer.pixels,
                                                state.menu_rasterizer.width,
                                                state.menu_rasterizer.height,
                                                glow_x,
                                                glow_y,
                                                glow_scale,
                                                [1.0, 1.0, 1.0, 0.90],
                                            );
                                        }

                                        let tint = if is_selected {
                                            [1.0f32, 0.95, 0.75, 1.0]
                                        } else {
                                            [0.70f32, 0.70, 0.72, 0.75]
                                        };

                                        atlas.blit_sprite(
                                            *sprite,
                                            &mut state.menu_rasterizer.pixels,
                                            state.menu_rasterizer.width,
                                            state.menu_rasterizer.height,
                                            x,
                                            y,
                                            scale,
                                            tint,
                                        );
                                    }
                                } else {
                                    state.menu_rasterizer.draw_text(
                                        "SELECT SCENARIO",
                                        500,
                                        320,
                                        3,
                                        yellow,
                                        true,
                                    );
                                    let scenarios = [
                                        "LETTER FROM SILENT HEAVEN (JAMES)",
                                        "BORN FROM A WISH (MARIA)",
                                    ];
                                    for (idx, name) in scenarios.iter().enumerate() {
                                        let is_selected =
                                            idx == state.menu_ctrl.title_sm.scenario_index;
                                        let color = if is_selected { yellow } else { white };
                                        let y = 410 + idx * 50;
                                        if is_selected {
                                            state
                                                .menu_rasterizer
                                                .draw_text(">", 360, y, 2, yellow, true);
                                        }
                                        state
                                            .menu_rasterizer
                                            .draw_text(name, 390, y, 2, color, true);
                                    }
                                }
                                state.menu_rasterizer.draw_text(
                                    "[Up/Down: Select | Enter: Confirm | Esc: Back]",
                                    430,
                                    660,
                                    1,
                                    muted,
                                    true,
                                );
                            }
                            TitlePhase::ActionDifficultySelect => {
                                if let Some(ref atlas) = state.button_sprites {
                                    let center_x = (state.menu_rasterizer.width / 2) as i32;
                                    // Title header
                                    let hdr_rect = AuthenticButtonSprite::ActionLevel.atlas_rect();
                                    let hdr_scale = 0.55f32;
                                    let hdr_w = (hdr_rect.width as f32 * hdr_scale) as i32;
                                    atlas.blit_sprite(
                                        AuthenticButtonSprite::ActionLevel,
                                        &mut state.menu_rasterizer.pixels,
                                        state.menu_rasterizer.width,
                                        state.menu_rasterizer.height,
                                        center_x - (hdr_w / 2),
                                        260,
                                        hdr_scale,
                                        [1.0, 1.0, 1.0, 1.0],
                                    );

                                    let choices = [
                                        (
                                            sh2_core::config::ActionDifficulty::Hard,
                                            AuthenticButtonSprite::Hard,
                                        ),
                                        (
                                            sh2_core::config::ActionDifficulty::Normal,
                                            AuthenticButtonSprite::Normal,
                                        ),
                                        (
                                            sh2_core::config::ActionDifficulty::Easy,
                                            AuthenticButtonSprite::Easy,
                                        ),
                                    ];

                                    for (idx, (diff, sprite)) in choices.iter().enumerate() {
                                        let is_selected =
                                            *diff == state.menu_ctrl.title_sm.action_difficulty;
                                        let y = 360 + idx as i32 * 65;
                                        let rect = sprite.atlas_rect();
                                        let scale = 0.52f32;
                                        let w = (rect.width as f32 * scale) as i32;
                                        let x = center_x - (w / 2);

                                        if is_selected {
                                            let glow_rect =
                                                AuthenticButtonSprite::GlowCursor.atlas_rect();
                                            let glow_scale = 0.46f32;
                                            let glow_w =
                                                (glow_rect.width as f32 * glow_scale) as i32;
                                            let glow_h =
                                                (glow_rect.height as f32 * glow_scale) as i32;
                                            let glow_x = center_x - (glow_w / 2);
                                            let btn_h = (rect.height as f32 * scale) as i32;
                                            let glow_y = y + (btn_h / 2) - (glow_h / 2);
                                            atlas.blit_sprite(
                                                AuthenticButtonSprite::GlowCursor,
                                                &mut state.menu_rasterizer.pixels,
                                                state.menu_rasterizer.width,
                                                state.menu_rasterizer.height,
                                                glow_x,
                                                glow_y,
                                                glow_scale,
                                                [1.0, 1.0, 1.0, 0.90],
                                            );
                                        }

                                        let tint = if is_selected {
                                            [1.0f32, 0.95, 0.75, 1.0]
                                        } else {
                                            [0.70f32, 0.70, 0.72, 0.75]
                                        };

                                        atlas.blit_sprite(
                                            *sprite,
                                            &mut state.menu_rasterizer.pixels,
                                            state.menu_rasterizer.width,
                                            state.menu_rasterizer.height,
                                            x,
                                            y,
                                            scale,
                                            tint,
                                        );
                                    }
                                } else {
                                    state.menu_rasterizer.draw_text(
                                        "ACTION DIFFICULTY",
                                        480,
                                        320,
                                        3,
                                        yellow,
                                        true,
                                    );
                                    let diff_str =
                                        format!("{:?}", state.menu_ctrl.title_sm.action_difficulty)
                                            .to_uppercase();
                                    state
                                        .menu_rasterizer
                                        .draw_text(&diff_str, 570, 420, 3, yellow, true);
                                }
                                state.menu_rasterizer.draw_text(
                                    "[Up/Down: Change | Enter: Next | Esc: Back]",
                                    440,
                                    660,
                                    1,
                                    muted,
                                    true,
                                );
                            }
                            TitlePhase::RiddleDifficultySelect => {
                                if let Some(ref atlas) = state.button_sprites {
                                    let center_x = (state.menu_rasterizer.width / 2) as i32;
                                    // Title header
                                    let hdr_rect = AuthenticButtonSprite::RiddleLevel.atlas_rect();
                                    let hdr_scale = 0.55f32;
                                    let hdr_w = (hdr_rect.width as f32 * hdr_scale) as i32;
                                    atlas.blit_sprite(
                                        AuthenticButtonSprite::RiddleLevel,
                                        &mut state.menu_rasterizer.pixels,
                                        state.menu_rasterizer.width,
                                        state.menu_rasterizer.height,
                                        center_x - (hdr_w / 2),
                                        260,
                                        hdr_scale,
                                        [1.0, 1.0, 1.0, 1.0],
                                    );

                                    let choices = [
                                        (
                                            sh2_core::config::RiddleDifficulty::Hard,
                                            AuthenticButtonSprite::Hard,
                                        ),
                                        (
                                            sh2_core::config::RiddleDifficulty::Normal,
                                            AuthenticButtonSprite::Normal,
                                        ),
                                        (
                                            sh2_core::config::RiddleDifficulty::Easy,
                                            AuthenticButtonSprite::Easy,
                                        ),
                                        (
                                            sh2_core::config::RiddleDifficulty::Extra,
                                            AuthenticButtonSprite::Beginner,
                                        ),
                                    ];

                                    for (idx, (diff, sprite)) in choices.iter().enumerate() {
                                        let is_selected =
                                            *diff == state.menu_ctrl.title_sm.riddle_difficulty;
                                        let y = 350 + idx as i32 * 58;
                                        let rect = sprite.atlas_rect();
                                        let scale = 0.52f32;
                                        let w = (rect.width as f32 * scale) as i32;
                                        let x = center_x - (w / 2);

                                        if is_selected {
                                            let glow_rect =
                                                AuthenticButtonSprite::GlowCursor.atlas_rect();
                                            let glow_scale = 0.46f32;
                                            let glow_w =
                                                (glow_rect.width as f32 * glow_scale) as i32;
                                            let glow_h =
                                                (glow_rect.height as f32 * glow_scale) as i32;
                                            let glow_x = center_x - (glow_w / 2);
                                            let btn_h = (rect.height as f32 * scale) as i32;
                                            let glow_y = y + (btn_h / 2) - (glow_h / 2);
                                            atlas.blit_sprite(
                                                AuthenticButtonSprite::GlowCursor,
                                                &mut state.menu_rasterizer.pixels,
                                                state.menu_rasterizer.width,
                                                state.menu_rasterizer.height,
                                                glow_x,
                                                glow_y,
                                                glow_scale,
                                                [1.0, 1.0, 1.0, 0.90],
                                            );
                                        }

                                        let tint = if is_selected {
                                            [1.0f32, 0.95, 0.75, 1.0]
                                        } else {
                                            [0.70f32, 0.70, 0.72, 0.75]
                                        };

                                        atlas.blit_sprite(
                                            *sprite,
                                            &mut state.menu_rasterizer.pixels,
                                            state.menu_rasterizer.width,
                                            state.menu_rasterizer.height,
                                            x,
                                            y,
                                            scale,
                                            tint,
                                        );
                                    }
                                } else {
                                    state.menu_rasterizer.draw_text(
                                        "RIDDLE DIFFICULTY",
                                        480,
                                        320,
                                        3,
                                        yellow,
                                        true,
                                    );
                                    let diff_str =
                                        format!("{:?}", state.menu_ctrl.title_sm.riddle_difficulty)
                                            .to_uppercase();
                                    state
                                        .menu_rasterizer
                                        .draw_text(&diff_str, 570, 420, 3, yellow, true);
                                }
                                state.menu_rasterizer.draw_text(
                                    "[Up/Down: Change | Enter: Start Game | Esc: Back]",
                                    420,
                                    660,
                                    1,
                                    muted,
                                    true,
                                );
                            }
                            TitlePhase::OptionsMenu => {
                                let tab_name = state.menu_ctrl.options_menu.active_tab.label();
                                state.menu_rasterizer.draw_text(
                                    &format!("OPTIONS: {}", tab_name.to_uppercase()),
                                    440,
                                    120,
                                    3,
                                    yellow,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "[Q / E : Switch Category]",
                                    500,
                                    160,
                                    2,
                                    cyan,
                                    true,
                                );

                                for i in 0..state.menu_ctrl.options_menu.item_count() {
                                    if let Some((label, val)) =
                                        state.menu_ctrl.options_menu.item_display(i)
                                    {
                                        let is_sel =
                                            i == state.menu_ctrl.options_menu.selected_index;
                                        let color = if is_sel { yellow } else { white };
                                        let y = 220 + i * 40;
                                        if is_sel {
                                            state
                                                .menu_rasterizer
                                                .draw_text(">", 280, y, 2, yellow, true);
                                        }
                                        state
                                            .menu_rasterizer
                                            .draw_text(label, 310, y, 2, color, true);
                                        state
                                            .menu_rasterizer
                                            .draw_text(":", 640, y, 2, muted, true);
                                        state
                                            .menu_rasterizer
                                            .draw_text(&val, 670, y, 2, color, true);
                                    }
                                }

                                if state.menu_ctrl.options_menu.is_dirty {
                                    state.menu_rasterizer.draw_text(
                                        "* UNSAVED CHANGES [Enter to Save | Esc to Discard]",
                                        380,
                                        660,
                                        2,
                                        yellow,
                                        true,
                                    );
                                } else {
                                    state.menu_rasterizer.draw_text(
                                        "[Esc / Backspace: Return to Main Menu]",
                                        460,
                                        660,
                                        2,
                                        muted,
                                        true,
                                    );
                                }
                            }
                            TitlePhase::Credits => {
                                state.menu_rasterizer.draw_text(
                                    "SILENT HILL 2 CREDITS",
                                    420,
                                    240,
                                    3,
                                    yellow,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "DIRECTOR: KEIICHIRO TOYAMA",
                                    400,
                                    330,
                                    2,
                                    white,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "CGI & CHARACTER DESIGN: TAKAYOSHI SATO",
                                    310,
                                    380,
                                    2,
                                    white,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "MUSIC & SOUND DESIGN: AKIRA YAMAOKA",
                                    330,
                                    430,
                                    2,
                                    white,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "[Press Esc / Cancel to Return]",
                                    470,
                                    620,
                                    2,
                                    muted,
                                    true,
                                );
                            }
                            TitlePhase::TestRoomsSelect => {
                                state.menu_rasterizer.draw_text(
                                    "TEST ROOMS & FOG CALIBRATION",
                                    390,
                                    150,
                                    3,
                                    yellow,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "[Select Stage to Boot into Simulation]",
                                    420,
                                    195,
                                    2,
                                    cyan,
                                    true,
                                );

                                for (idx, room) in TestRoomKind::ALL.iter().enumerate() {
                                    let is_sel = idx == state.menu_ctrl.title_sm.test_room_index;
                                    let color = if is_sel { yellow } else { white };
                                    let y = 270 + idx * 60;
                                    if is_sel {
                                        state
                                            .menu_rasterizer
                                            .draw_text(">", 280, y, 2, yellow, true);
                                    }
                                    state.menu_rasterizer.draw_text(
                                        room.label(),
                                        320,
                                        y,
                                        2,
                                        color,
                                        true,
                                    );
                                }

                                state.menu_rasterizer.draw_text(
                                    "[Up/Down: Select Room | Enter: Boot Stage | Esc: Back]",
                                    350,
                                    660,
                                    2,
                                    muted,
                                    true,
                                );
                            }
                            TitlePhase::TransitionToGame => {
                                let label = state.menu_ctrl.title_sm.selected_test_room.label();
                                state.menu_rasterizer.draw_text(
                                    &format!("LOADING STAGE: {}...", label),
                                    350,
                                    500,
                                    2,
                                    white,
                                    true,
                                );
                            }
                            _ => {}
                        }

                        // Upload rasterized menu text overlay to GPU and blit over the background
                        state.render_ctx.queue.write_texture(
                            wgpu::ImageCopyTexture {
                                texture: &state.menu_text_texture,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            &state.menu_rasterizer.pixels,
                            wgpu::ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * state.menu_rasterizer.width as u32),
                                rows_per_image: Some(state.menu_rasterizer.height as u32),
                            },
                            wgpu::Extent3d {
                                width: state.menu_rasterizer.width as u32,
                                height: state.menu_rasterizer.height as u32,
                                depth_or_array_layers: 1,
                            },
                        );

                        state.blitter.render_with_bind_group_and_load_op(
                            &mut encoder,
                            &state.menu_text_bind_group,
                            &surface_view,
                            wgpu::LoadOp::Load,
                        );
                    }
                }

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

    // Parse CLI arguments for test rooms & fog testing
    let args: Vec<String> = std::env::args().collect();
    let mut initial_test_room = None;
    let mut initial_fog_debug = false;

    for arg in &args[1..] {
        if arg == "--test-fog" {
            initial_fog_debug = true;
            if initial_test_room.is_none() {
                initial_test_room = Some(TestRoomKind::SyntheticFogRunway);
            }
        } else if let Some(room_str) = arg.strip_prefix("--test-room=") {
            initial_test_room = match room_str {
                "ob01" | "observation" | "outdoor" => Some(TestRoomKind::ObservationDeck),
                "er01" | "restroom" | "toilet" => Some(TestRoomKind::RestroomStart),
                "rr21" | "boiler" => Some(TestRoomKind::HotelBoilerRoom),
                "runway" | "fog" => Some(TestRoomKind::SyntheticFogRunway),
                "fallback" | "sandbox" => Some(TestRoomKind::FallbackSandbox),
                _ => {
                    tracing::warn!(
                        "Unknown test room: '{}', defaulting to Observation Deck",
                        room_str
                    );
                    Some(TestRoomKind::ObservationDeck)
                }
            };
        }
    }

    if let Some(r) = initial_test_room {
        tracing::info!("CLI parameter selected test room: {:?}", r);
    }

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp {
        state: None,
        initial_test_room,
        initial_fog_debug,
    };
    event_loop.run_app(&mut app).expect("Event loop error");
}
