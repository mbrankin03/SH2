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
    AnimationFile, AnimationFrame,
};
use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_assets::formats::TextureLoader;
use sh2_audio::{play_menu_sound, AudioEngine, MenuAudioBank, SoundCueKind};
use sh2_core::{ControlScheme, ScenarioKind, SecondaryBonePhysics, SimulationCore};
use sh2_debug::{
    build_skeleton_debug_lines, build_wireframe_lines, ModelViewerEntry, ModelViewerState,
    StageLightingRig, TextureSourceMode, ViewerRenderMode, BESTIARY_CATALOG,
};
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
    vanilla_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    hd_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
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

/// Generate a calibrated neutral 3D Model Viewer pedestal, metric floor grid, and vertical height scale pole.
///
/// Coordinates follow PS2 conventions:
/// - Floor plane at $Y = 0.0$ (normal pointing UP = $(0, -1, 0)$).
/// - 1 meter = $1000.0$ world units.
/// - Metric height scale pole at $X = -1100.0, Z = -300.0$ with tick markers at $0.5\text{m}$, $1.0\text{m}$,
///   $1.5\text{m}$, $1.8\text{m}$ (human eye height), and $2.0\text{m}$.
fn create_model_viewer_pedestal_geometry() -> MapFile {
    let mut map = MapFile::default();
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 0,
        color: 0xFF666666,
        overlay: 0,
        specularity: 0.3,
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

    // 1. Dark Studio Floor Plane (6m x 6m)
    let floor_ext = 3500.0;
    add_quad(
        [-floor_ext, 4.0, -floor_ext],
        [floor_ext, 4.0, -floor_ext],
        [floor_ext, 4.0, floor_ext],
        [-floor_ext, 4.0, floor_ext],
        [0.0, -1.0, 0.0],
        [36, 38, 42, 255],
    );

    // 2. Metric Floor Grid Lines (every 0.5m = 500 units, major 1.0m = 1000 units)
    for step in -6..=6 {
        let coord = step as f32 * 500.0;
        let is_axis = step == 0;
        let is_meter = step % 2 == 0;
        let half_w = if is_axis {
            10.0
        } else if is_meter {
            6.0
        } else {
            3.0
        };
        let col_x = if is_axis {
            [180, 70, 70, 255] // X-axis red-tinted line
        } else if is_meter {
            [85, 92, 102, 255]
        } else {
            [55, 60, 68, 255]
        };
        let col_z = if is_axis {
            [70, 110, 185, 255] // Z-axis blue-tinted line
        } else if is_meter {
            [85, 92, 102, 255]
        } else {
            [55, 60, 68, 255]
        };

        // Line parallel to X at Z = coord
        add_quad(
            [-3000.0, 1.5, coord - half_w],
            [3000.0, 1.5, coord - half_w],
            [3000.0, 1.5, coord + half_w],
            [-3000.0, 1.5, coord + half_w],
            [0.0, -1.0, 0.0],
            col_x,
        );

        // Line parallel to Z at X = coord
        add_quad(
            [coord - half_w, 1.5, -3000.0],
            [coord + half_w, 1.5, -3000.0],
            [coord + half_w, 1.5, 3000.0],
            [coord - half_w, 1.5, 3000.0],
            [0.0, -1.0, 0.0],
            col_z,
        );
    }

    // 3. Octagonal Turntable Pedestal Disc (Radius = 750mm, Top surface at Y = 0.0)
    let segments = 16;
    let radius = 750.0f32;
    let rim_radius = 790.0f32;
    let ped_depth = 85.0f32; // Extends downward from Y = 0.5 to Y = +85.5

    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();

        // Top disc wedge (alternating subtle slate tones for turntable rotation visibility)
        let top_col = if i % 2 == 0 {
            [68, 72, 80, 255]
        } else {
            [60, 64, 72, 255]
        };
        add_quad(
            [0.0, 0.5, 0.0],
            [c0 * radius, 0.5, s0 * radius],
            [c1 * radius, 0.5, s1 * radius],
            [0.0, 0.5, 0.0],
            [0.0, -1.0, 0.0],
            top_col,
        );

        // Outer metallic bevel ring
        add_quad(
            [c0 * radius, 0.5, s0 * radius],
            [c0 * rim_radius, 8.0, s0 * rim_radius],
            [c1 * rim_radius, 8.0, s1 * rim_radius],
            [c1 * radius, 0.5, s1 * radius],
            [0.0, -1.0, 0.0],
            [120, 125, 135, 255],
        );

        // Vertical side skirt of pedestal
        let nx = (c0 + c1) * 0.5;
        let nz = (s0 + s1) * 0.5;
        add_quad(
            [c0 * rim_radius, 8.0, s0 * rim_radius],
            [c0 * rim_radius, ped_depth, s0 * rim_radius],
            [c1 * rim_radius, ped_depth, s1 * rim_radius],
            [c1 * rim_radius, 8.0, s1 * rim_radius],
            [nx, 0.0, nz],
            [48, 52, 58, 255],
        );
    }

    // 4. Metric Height Scale Post (at X = -950mm, Z = -200mm, height = 2.2m = -2200 units)
    let post_x = -950.0;
    let post_z = -200.0;
    let post_w = 14.0;
    let post_h = -2200.0;

    // Vertical scale mast
    add_quad(
        [post_x - post_w, post_h, post_z],
        [post_x + post_w, post_h, post_z],
        [post_x + post_w, 0.0, post_z],
        [post_x - post_w, 0.0, post_z],
        [0.0, 0.0, 1.0],
        [140, 145, 155, 255],
    );

    // Height tick crossbars every 0.25m (250 units), major ticks at 0.5m, 1.0m, 1.5m, 1.8m (human height), 2.0m
    let ticks: &[(f32, f32, [u8; 4])] = &[
        (-250.0, 50.0, [110, 115, 120, 255]),
        (-500.0, 95.0, [180, 185, 195, 255]), // 0.5m
        (-750.0, 50.0, [110, 115, 120, 255]),
        (-1000.0, 130.0, [240, 205, 55, 255]), // 1.0m (Gold marker)
        (-1250.0, 50.0, [110, 115, 120, 255]),
        (-1500.0, 95.0, [180, 185, 195, 255]), // 1.5m
        (-1750.0, 65.0, [100, 210, 230, 255]), // 1.75m (Cyan reference)
        (-2000.0, 130.0, [240, 205, 55, 255]), // 2.0m (Gold marker)
    ];

    for &(ty, width, col) in ticks {
        add_quad(
            [post_x, ty - 6.0, post_z + 2.0],
            [post_x + width, ty - 6.0, post_z + 2.0],
            [post_x + width, ty + 6.0, post_z + 2.0],
            [post_x, ty + 6.0, post_z + 2.0],
            [0.0, 0.0, 1.0],
            col,
        );
    }

    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 3,
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
                        // Authentic PS2 stage 2 (er01) post-prologue mirror spawn coordinates:
                        // James stands at the sink: X: -19606.6, Y: 0.0 (floor), Z: 20429.55, Facing: -PI/2 (-1.5707964 rad, West towards mirror)
                        return (
                            map,
                            glam::Vec3::new(-19606.6, 0.0, 20429.55),
                            -std::f32::consts::FRAC_PI_2,
                            fog,
                        );
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
        TestRoomKind::ModelAndTextureViewer => {
            tracing::info!("Generated 3D Model & Texture Viewer Pedestal Stage (SH2-36).");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.06, 0.07, 0.08, 1.0],
                fog_near: 5000.0,
                fog_far: 30000.0,
                density: 0.05, // Clean neutral inspection visibility
                time: 0.0,
                step_count: 16,
                curve_type: 0,
                speed_mult: 0.0,
                _padding: 0.0,
            };
            (
                create_model_viewer_pedestal_geometry(),
                glam::Vec3::new(0.0, 0.0, 0.0),
                0.0,
                fog,
            )
        }
    }
}

/// Generates a high-resolution procedural HD inspection variation from an RGBA8 texture
/// when an external SH2:EE `.dds`/`.png` replacement texture is not found on disk,
/// allowing side-by-side filtering & detail comparison in the Model Viewer.
fn upscale_hd_preview_rgba(rgba: &[u8], width: usize, height: usize) -> (u32, u32, Vec<u8>) {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return (1, 1, vec![255, 255, 255, 255]);
    }
    let out_w = width * 2;
    let out_h = height * 2;
    let mut out = vec![0u8; out_w * out_h * 4];

    for y in 0..out_h {
        let sy = (y / 2).min(height - 1);
        let sy_next = (sy + 1).min(height - 1);
        let fy = if y % 2 == 1 { 0.5f32 } else { 0.0f32 };

        for x in 0..out_w {
            let sx = (x / 2).min(width - 1);
            let sx_next = (sx + 1).min(width - 1);
            let fx = if x % 2 == 1 { 0.5f32 } else { 0.0f32 };

            let p00 = (sy * width + sx) * 4;
            let p10 = (sy * width + sx_next) * 4;
            let p01 = (sy_next * width + sx) * 4;
            let p11 = (sy_next * width + sx_next) * 4;

            let dst = (y * out_w + x) * 4;
            for c in 0..4 {
                let v00 = rgba[p00 + c] as f32;
                let v10 = rgba[p10 + c] as f32;
                let v01 = rgba[p01 + c] as f32;
                let v11 = rgba[p11 + c] as f32;
                let top = v00 * (1.0 - fx) + v10 * fx;
                let bot = v01 * (1.0 - fx) + v11 * fx;
                let mut val = top * (1.0 - fy) + bot * fy;
                // Subtle micro-contrast sharpening on RGB channels for HD mode inspection
                if c < 3 {
                    let lap = v00 * 4.0 - (v10 + v01 + v11 + v00);
                    val = (val + lap * 0.15).clamp(0.0, 255.0);
                }
                out[dst + c] = val as u8;
            }
        }
    }

    (out_w as u32, out_h as u32, out)
}

/// Locate authentic character, enemy, boss, or weapon model and animation on disk.
/// Supports static/rigid weapon and item models by synthesizing an identity bind-pose animation track
/// if no `.anm` file is present or compatible with the model's bone count.
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
    let mut hd_mdl_bytes = None;
    let mut anm_bytes = None;

    for base in &base_roots {
        for mdl_sub in mdl_rel_paths {
            let p = PathBuf::from(base).join(mdl_sub);
            if let Ok(bytes) = std::fs::read(&p) {
                tracing::info!("Found model asset: {:?}", p);
                mdl_bytes = Some(bytes);

                // Check if a corresponding vanilla or sh2e override model exists for texture comparison
                if let Some(stripped) = mdl_sub.strip_prefix("data/") {
                    let sh2e_cand = PathBuf::from(base).join("sh2e").join(stripped);
                    if let Ok(hd_b) = std::fs::read(&sh2e_cand) {
                        hd_mdl_bytes = Some(hd_b);
                    }
                } else if let Some(stripped) = mdl_sub.strip_prefix("sh2e/") {
                    let data_cand = PathBuf::from(base).join("data").join(stripped);
                    if let Ok(van_b) = std::fs::read(&data_cand) {
                        // Keep `mdl_bytes` as vanilla and `hd_mdl_bytes` as sh2e
                        hd_mdl_bytes = mdl_bytes.replace(van_b);
                    }
                }
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
                if !bytes.is_empty() {
                    tracing::info!("Found animation asset: {:?}", p);
                    anm_bytes = Some(bytes);
                    break;
                }
            }
        }
        if anm_bytes.is_some() {
            break;
        }
    }

    let mdl_bytes = mdl_bytes?;
    let model = parse_mdl(&mdl_bytes).ok()?;
    let hd_model = hd_mdl_bytes.as_deref().and_then(|b| parse_mdl(b).ok());

    // Parse animation if available and compatible, otherwise synthesize a 1-frame bind-pose track
    let anm = anm_bytes
        .as_deref()
        .and_then(|bytes| parse_anm(bytes, model.bone_count, &model.parents).ok())
        .filter(|a| !a.frames.is_empty())
        .unwrap_or_else(|| AnimationFile {
            bone_count: model.bone_count,
            frames: vec![AnimationFrame {
                bones: vec![None; model.bone_count],
            }],
        });

    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let gpu_mesh = ctx.compile_skeletal_mesh(&model);

    // Build both Vanilla PS2 and SH2:EE HD diffuse texture bind groups.
    // Compact unique sprite IDs (`model.primitives[*].texture_index`, e.g. 1239..1772)
    // into a dense 0..N slot array so we only allocate GPU bind groups for textures actually
    // used by this model instead of 1,700+ dummy bind groups.
    let mut unique_tex_ids: Vec<u16> = model.primitives.iter().map(|pr| pr.texture_index).collect();
    unique_tex_ids.sort_unstable();
    unique_tex_ids.dedup();

    let mut vanilla_texture_bind_groups = Vec::with_capacity(unique_tex_ids.len());
    let mut hd_texture_bind_groups = Vec::with_capacity(unique_tex_ids.len());

    for &raw_tid in &unique_tex_ids {
        let tidx = raw_tid as u32;
        if let Some(tex) = model.textures.get(&tidx) {
            if let Ok(rgba) = tex.to_rgba() {
                let van_bg = ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&format!("{character_name} Vanilla Texture {tidx}")),
                );

                // Check if `hd_model` provides an override texture, else generate sharpened 2x HD surface
                let hd_bg = if let Some(ref hdm) = hd_model {
                    if let Some(hd_tex) = hdm.textures.get(&tidx) {
                        if let Ok(hd_rgba) = hd_tex.to_rgba() {
                            ctx.create_texture_bind_group(
                                hd_tex.width as u32,
                                hd_tex.height as u32,
                                &hd_rgba,
                                Some(&format!("{character_name} SH2:EE HD Texture {tidx}")),
                            )
                        } else {
                            van_bg.clone()
                        }
                    } else {
                        van_bg.clone()
                    }
                } else {
                    let (hw, hh, hd_rgba) = upscale_hd_preview_rgba(&rgba, tex.width, tex.height);
                    ctx.create_texture_bind_group(
                        hw,
                        hh,
                        &hd_rgba,
                        Some(&format!("{character_name} Enhanced Texture {tidx}")),
                    )
                };

                vanilla_texture_bind_groups.push(van_bg);
                hd_texture_bind_groups.push(hd_bg);
                continue;
            }
        }
        vanilla_texture_bind_groups.push(ctx.default_texture_bind_group.clone());
        hd_texture_bind_groups.push(ctx.default_texture_bind_group.clone());
    }

    // Remap each GPU primitive's `texture_index` to its dense 0..N slot in `*_texture_bind_groups`
    let mut gpu_mesh = gpu_mesh;
    for prim in &mut gpu_mesh.primitives {
        if let Ok(dense_slot) = unique_tex_ids.binary_search(&prim.texture_index) {
            prim.texture_index = dense_slot as u16;
        }
    }

    let texture_bind_groups = hd_texture_bind_groups.clone();

    tracing::info!(
        "Loaded model '{}': bones={}, primitives={}, vertices={}, textures={}, anm_frames={}",
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
        vanilla_texture_bind_groups,
        hd_texture_bind_groups,
        texture_bind_groups,
        secondary_physics,
    })
}

/// Loads a specific [`ModelViewerEntry`] and its selected `.anm` track index from disk.
fn load_viewer_entry_asset(
    ctx: &RenderContext,
    entry: &'static ModelViewerEntry,
    anm_index: usize,
) -> Option<CharacterAsset> {
    let anm_slice: Vec<&str> = if entry.anm_paths.is_empty() {
        Vec::new()
    } else {
        let idx = anm_index % entry.anm_paths.len();
        let mut list = vec![entry.anm_paths[idx]];
        for &p in entry.anm_paths {
            if p != entry.anm_paths[idx] {
                list.push(p);
            }
        }
        list
    };
    load_character_asset(ctx, entry.display_name, entry.mdl_paths, &anm_slice)
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
    model_viewer: ModelViewerState,
    viewer_asset: Option<CharacterAsset>,
    loaded_viewer_model_idx: usize,
    loaded_viewer_anm_idx: usize,
    mouse_left_pressed: bool,
    mouse_right_pressed: bool,
    last_cursor_pos: Option<(f64, f64)>,
    key_forward: bool,
    key_backward: bool,
    key_left: bool,
    key_right: bool,
    key_zoom_in: bool,
    key_zoom_out: bool,
    key_pan_up: bool,
    key_pan_down: bool,
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
    initial_viewer_model: Option<String>,
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
            model_viewer,
            viewer_asset,
            loaded_viewer_model_idx,
            loaded_viewer_anm_idx,
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

                    // If in the 3D Model, Animation & Texture Viewer room, route hotkeys to ModelViewerState
                    if state.active_test_room == TestRoomKind::ModelAndTextureViewer {
                        match key_code {
                            winit::keyboard::KeyCode::Escape
                            | winit::keyboard::KeyCode::Backspace
                                if pressed =>
                            {
                                state.menu_ctrl.in_game_active = false;
                                state.menu_ctrl.title_sm.phase = TitlePhase::TestRoomsSelect;
                                state.menu_ctrl.title_sm.fade_alpha = 1.0;
                                tracing::info!(
                                    "Returned from Model & Texture Viewer to Test Rooms menu."
                                );
                                return;
                            }
                            winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => {
                                state.key_forward = pressed;
                            }
                            winit::keyboard::KeyCode::KeyS
                            | winit::keyboard::KeyCode::ArrowDown => {
                                state.key_backward = pressed;
                            }
                            winit::keyboard::KeyCode::KeyA
                            | winit::keyboard::KeyCode::ArrowLeft => {
                                state.key_left = pressed;
                            }
                            winit::keyboard::KeyCode::KeyD
                            | winit::keyboard::KeyCode::ArrowRight => {
                                state.key_right = pressed;
                            }
                            winit::keyboard::KeyCode::KeyQ => {
                                state.key_zoom_in = pressed;
                            }
                            winit::keyboard::KeyCode::KeyE => {
                                state.key_zoom_out = pressed;
                            }
                            winit::keyboard::KeyCode::KeyZ => {
                                state.key_pan_up = pressed;
                            }
                            winit::keyboard::KeyCode::KeyX => {
                                state.key_pan_down = pressed;
                            }
                            winit::keyboard::KeyCode::KeyN
                            | winit::keyboard::KeyCode::PageDown
                            | winit::keyboard::KeyCode::Period
                                if pressed =>
                            {
                                state.model_viewer.next_model();
                                tracing::info!(
                                    "[Model Viewer] Selected model [{}/{}]: {}",
                                    state.model_viewer.selected_model_index + 1,
                                    BESTIARY_CATALOG.len(),
                                    state.model_viewer.current_entry().display_name
                                );
                            }
                            winit::keyboard::KeyCode::KeyP
                            | winit::keyboard::KeyCode::PageUp
                            | winit::keyboard::KeyCode::Comma
                                if pressed =>
                            {
                                state.model_viewer.prev_model();
                                tracing::info!(
                                    "[Model Viewer] Selected model [{}/{}]: {}",
                                    state.model_viewer.selected_model_index + 1,
                                    BESTIARY_CATALOG.len(),
                                    state.model_viewer.current_entry().display_name
                                );
                            }
                            winit::keyboard::KeyCode::KeyM | winit::keyboard::KeyCode::Tab
                                if pressed =>
                            {
                                state.model_viewer.next_animation_clip();
                                tracing::info!(
                                    "[Model Viewer] Switched animation clip index to {}",
                                    state.model_viewer.selected_anm_index
                                );
                            }
                            winit::keyboard::KeyCode::Space if pressed => {
                                state.model_viewer.toggle_pause();
                                tracing::info!(
                                    "[Model Viewer] Playback {}",
                                    if state.model_viewer.is_paused {
                                        "PAUSED"
                                    } else {
                                        "PLAYING"
                                    }
                                );
                            }
                            winit::keyboard::KeyCode::BracketLeft if pressed => {
                                state.model_viewer.step_frame(-1);
                                tracing::info!(
                                    "[Model Viewer] Step backward -> Frame {:.0}/{}",
                                    state.model_viewer.current_frame,
                                    state.model_viewer.total_frames
                                );
                            }
                            winit::keyboard::KeyCode::BracketRight if pressed => {
                                state.model_viewer.step_frame(1);
                                tracing::info!(
                                    "[Model Viewer] Step forward -> Frame {:.0}/{}",
                                    state.model_viewer.current_frame,
                                    state.model_viewer.total_frames
                                );
                            }
                            winit::keyboard::KeyCode::Minus if pressed => {
                                state.model_viewer.adjust_playback_speed(-0.1);
                                tracing::info!(
                                    "[Model Viewer] Playback speed: {:.1}x",
                                    state.model_viewer.playback_speed
                                );
                            }
                            winit::keyboard::KeyCode::Equal if pressed => {
                                state.model_viewer.adjust_playback_speed(0.1);
                                tracing::info!(
                                    "[Model Viewer] Playback speed: {:.1}x",
                                    state.model_viewer.playback_speed
                                );
                            }
                            winit::keyboard::KeyCode::KeyR if pressed => {
                                state.model_viewer.reverse_playback();
                                tracing::info!(
                                    "[Model Viewer] Reversed playback speed: {:.1}x",
                                    state.model_viewer.playback_speed
                                );
                            }
                            winit::keyboard::KeyCode::KeyT if pressed => {
                                state.model_viewer.turntable_enabled =
                                    !state.model_viewer.turntable_enabled;
                                tracing::info!(
                                    "[Model Viewer] Turntable auto-spin: {}",
                                    state.model_viewer.turntable_enabled
                                );
                            }
                            winit::keyboard::KeyCode::KeyV if pressed => {
                                state.model_viewer.render_mode =
                                    state.model_viewer.render_mode.next();
                                tracing::info!(
                                    "[Model Viewer] Render mode: {}",
                                    state.model_viewer.render_mode.label()
                                );
                            }
                            winit::keyboard::KeyCode::KeyB if pressed => {
                                state.model_viewer.skeleton_overlay =
                                    !state.model_viewer.skeleton_overlay;
                                tracing::info!(
                                    "[Model Viewer] Bone skeleton & joint axes overlay: {}",
                                    state.model_viewer.skeleton_overlay
                                );
                            }
                            winit::keyboard::KeyCode::KeyG if pressed => {
                                state.model_viewer.wireframe_overlay =
                                    !state.model_viewer.wireframe_overlay;
                                tracing::info!(
                                    "[Model Viewer] Wireframe mesh overlay: {}",
                                    state.model_viewer.wireframe_overlay
                                );
                            }
                            winit::keyboard::KeyCode::KeyH if pressed => {
                                state.model_viewer.texture_source =
                                    state.model_viewer.texture_source.toggle();
                                tracing::info!(
                                    "[Model Viewer] Texture source: {}",
                                    state.model_viewer.texture_source.label()
                                );
                            }
                            winit::keyboard::KeyCode::KeyL if pressed => {
                                state.model_viewer.lighting_rig =
                                    state.model_viewer.lighting_rig.next();
                                tracing::info!(
                                    "[Model Viewer] Stage lighting rig: {}",
                                    state.model_viewer.lighting_rig.label()
                                );
                            }
                            winit::keyboard::KeyCode::KeyO if pressed => {
                                state
                                    .model_viewer
                                    .camera
                                    .reset_for_entry(state.model_viewer.current_entry());
                                state.model_viewer.turntable_yaw = 0.0;
                                tracing::info!("[Model Viewer] Reset orbit camera framing.");
                            }
                            _ => {}
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
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((last_x, last_y)) = state.last_cursor_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;
                    if state.menu_ctrl.in_game_active
                        && state.active_test_room == TestRoomKind::ModelAndTextureViewer
                    {
                        if state.mouse_left_pressed {
                            state.model_viewer.camera.orbit(-dx * 0.007, dy * 0.007);
                        } else if state.mouse_right_pressed {
                            state.model_viewer.camera.pan(-dx * 2.0, dy * 2.0);
                        }
                    }
                }
                state.last_cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if state.menu_ctrl.in_game_active
                    && state.active_test_room == TestRoomKind::ModelAndTextureViewer
                {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 150.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 1.5,
                    };
                    state.model_viewer.camera.zoom(-scroll);
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

                // When menu is active, log formatted state changes so console/headless/testing builds observe menu navigation
                if !state.menu_ctrl.in_game_active {
                    let current_text = state.menu_ctrl.overlay_text().join(" | ");
                    if current_text != state.last_menu_state_log {
                        tracing::info!("[SH2 MENU HUD] {}", current_text);
                        state.last_menu_state_log = current_text;
                    }
                }

                let is_in_game = state.menu_ctrl.in_game_active;
                let fade_alpha = state.menu_ctrl.title_sm.fade_alpha;
                let phase = state.menu_ctrl.title_sm.phase;
                let is_viewer =
                    is_in_game && state.active_test_room == TestRoomKind::ModelAndTextureViewer;

                // Advance simulation time and frame counter for animated post-processing
                let dt_sec = dt.as_secs_f32();
                state.frame_counter = state.frame_counter.wrapping_add(1);
                state.sim_time += dt_sec;

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
                let grain_intensity = if is_viewer {
                    0.0
                } else {
                    (grain_pct as f32) / 100.0
                };

                state
                    .render_ctx
                    .update_post(&sh2_renderer::post::PostUniforms {
                        time: state.sim_time,
                        grain_intensity,
                        exposure: 1.0,
                        frame_count: state.frame_counter,
                    });

                let aspect =
                    state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;

                if is_viewer {
                    // --- 3D MODEL, ANIMATION & TEXTURE VIEWER PASS (SH2-36) ---
                    let mut orbit_yaw_delta = 0.0f32;
                    let mut orbit_pitch_delta = 0.0f32;
                    let mut zoom_delta = 0.0f32;
                    let mut pan_up_delta = 0.0f32;

                    if state.key_left {
                        orbit_yaw_delta -= 1.8 * dt_sec;
                    }
                    if state.key_right {
                        orbit_yaw_delta += 1.8 * dt_sec;
                    }
                    if state.key_forward {
                        orbit_pitch_delta += 1.2 * dt_sec;
                    }
                    if state.key_backward {
                        orbit_pitch_delta -= 1.2 * dt_sec;
                    }
                    if state.key_zoom_in {
                        zoom_delta -= 1400.0 * dt_sec;
                    }
                    if state.key_zoom_out {
                        zoom_delta += 1400.0 * dt_sec;
                    }
                    if state.key_pan_up {
                        pan_up_delta += 800.0 * dt_sec;
                    }
                    if state.key_pan_down {
                        pan_up_delta -= 800.0 * dt_sec;
                    }

                    state
                        .model_viewer
                        .camera
                        .orbit(orbit_yaw_delta, orbit_pitch_delta);
                    state.model_viewer.camera.zoom(zoom_delta);
                    state.model_viewer.camera.pan(0.0, pan_up_delta);
                    state.model_viewer.tick(dt_sec);

                    let view = state.model_viewer.camera.view_matrix();
                    let proj = glam::Mat4::perspective_rh(
                        state.model_viewer.camera.fov_y,
                        aspect,
                        10.0,
                        30000.0,
                    );
                    let view_proj = proj * view;
                    let inv_proj = proj.inverse();
                    let eye_pos = state.model_viewer.camera.eye_position();

                    let shade_mode = match state.model_viewer.render_mode {
                        ViewerRenderMode::UntexturedClay => 1.0,
                        ViewerRenderMode::SurfaceNormals => 2.0,
                        ViewerRenderMode::Textured
                        | ViewerRenderMode::Wireframe
                        | ViewerRenderMode::SkeletonJoints => {
                            if state.model_viewer.lighting_rig == StageLightingRig::FlashlightCone {
                                0.0
                            } else {
                                3.0
                            }
                        }
                    };
                    state
                        .render_ctx
                        .update_camera_with_mode(view_proj, inv_proj, eye_pos, shade_mode);

                    // Stage spotlight configuration
                    let target = state.model_viewer.camera.target;
                    let to_target = (target - eye_pos).normalize_or_zero();
                    let spot_uniforms = match state.model_viewer.lighting_rig {
                        StageLightingRig::StudioNeutral => sh2_renderer::light::SpotlightUniforms {
                            position: [eye_pos.x, eye_pos.y - 300.0, eye_pos.z, 1.0],
                            direction_inner: [to_target.x, to_target.y, to_target.z, 0.70],
                            color_outer: [1.3, 1.28, 1.22, 0.35],
                        },
                        StageLightingRig::HighContrastRim => {
                            sh2_renderer::light::SpotlightUniforms {
                                position: [eye_pos.x, eye_pos.y - 500.0, eye_pos.z, 1.0],
                                direction_inner: [to_target.x, to_target.y, to_target.z, 0.85],
                                color_outer: [2.8, 2.5, 2.1, 0.60],
                            }
                        }
                        StageLightingRig::FlashlightCone => {
                            sh2_renderer::light::SpotlightUniforms {
                                position: [eye_pos.x, eye_pos.y, eye_pos.z, 1.0],
                                direction_inner: [to_target.x, to_target.y, to_target.z, 0.92],
                                color_outer: [2.6, 2.35, 1.85, 0.75],
                            }
                        }
                        StageLightingRig::FullBright => sh2_renderer::light::SpotlightUniforms {
                            position: [eye_pos.x, eye_pos.y, eye_pos.z, 1.0],
                            direction_inner: [to_target.x, to_target.y, to_target.z, -1.0],
                            color_outer: [2.0, 2.0, 2.0, -1.0],
                        },
                    };
                    state.render_ctx.update_spotlight(&spot_uniforms);

                    let mut skin_matrices = Vec::new();
                    let mut debug_lines = Vec::new();

                    if let Some(ref c) = state.viewer_asset {
                        let (f0, f1, alpha_frame) = state.model_viewer.sample_cursor();
                        let frame_count = c.anm.frames.len();
                        if frame_count > 0 {
                            let blended_frame = interpolate_frames(
                                &c.anm.frames[f0 % frame_count],
                                &c.anm.frames[f1 % frame_count],
                                alpha_frame,
                            );
                            let pose =
                                evaluate_pose(&c.model.parents, &c.bind_locals, &blended_frame);
                            let local_skin =
                                compute_skin_matrices(&pose, &c.model.initial_matrices);

                            let (s, c_yaw) = state.model_viewer.turntable_yaw.sin_cos();
                            let world_mat = [
                                [c_yaw, 0.0, s, 0.0],
                                [0.0, 1.0, 0.0, 0.0],
                                [-s, 0.0, c_yaw, 0.0],
                                [0.0, 0.0, 0.0, 1.0],
                            ];

                            skin_matrices.reserve(local_skin.len());
                            for m_skin in &local_skin {
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

                            if state.model_viewer.render_mode == ViewerRenderMode::Wireframe
                                || state.model_viewer.wireframe_overlay
                            {
                                let has_variant_poses =
                                    c.model.primitives.iter().any(|pr| pr.pose_index > 0);
                                let mut all_tris = Vec::new();
                                for (k, pr) in c.model.primitives.iter().enumerate() {
                                    let is_active = if has_variant_poses {
                                        pr.pose_index == 0
                                            || pr.pose_index == 3
                                            || pr.pose_index == 14
                                    } else {
                                        true
                                    };
                                    if is_active {
                                        all_tris.extend(c.model.primitive_triangles(k));
                                    }
                                }
                                debug_lines.extend(build_wireframe_lines(
                                    &c.model.vertices.positions,
                                    &c.model.vertices.bone_indices,
                                    &c.model.vertices.weights,
                                    &skin_matrices,
                                    &all_tris,
                                    [0.15, 0.92, 0.65, 1.0],
                                ));
                            }

                            if state.model_viewer.render_mode == ViewerRenderMode::SkeletonJoints
                                || state.model_viewer.skeleton_overlay
                            {
                                debug_lines.extend(build_skeleton_debug_lines(
                                    &c.model.parents,
                                    &pose,
                                    &world_mat,
                                    45.0,
                                ));
                            }
                        }
                    }

                    let skeletal_draw = if let Some(ref c) = state.viewer_asset {
                        if !skin_matrices.is_empty() {
                            let tex_slice = match state.model_viewer.texture_source {
                                TextureSourceMode::EnhancedHd => {
                                    c.hd_texture_bind_groups.as_slice()
                                }
                                TextureSourceMode::VanillaPs2 => {
                                    c.vanilla_texture_bind_groups.as_slice()
                                }
                            };
                            vec![(&c.gpu_mesh, skin_matrices.as_slice(), Some(tex_slice))]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    };

                    state.render_ctx.render_scene_with_debug_lines(
                        Some(&state.gpu_map),
                        Some(&state.map_texture_bind_groups),
                        &skeletal_draw,
                        &debug_lines,
                    );
                } else {
                    // 1. Advance simulation at fixed 60 Hz tick (runs animation controller update)
                    state.sim_core.update(dt);

                    // 2. Compute smooth sub-frame interpolated view matrix for high refresh OLED
                    let alpha = state.sim_core.interpolation_alpha();
                    let view = state.sim_core.camera.interpolated_view_matrix(alpha);
                    let fov = state.sim_core.camera.interpolated_fov(alpha);
                    let proj = glam::Mat4::perspective_rh(fov, aspect, 10.0, 30000.0);
                    let view_proj = proj * view;
                    let inv_proj = proj.inverse();

                    let cam_pos = state.sim_core.camera.curr_snapshot.eye;
                    state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);

                    // 3. Dynamic Flashlight tracking character position & facing angle
                    let player_pos = state.sim_core.scenario.player_position;
                    let player_yaw = state.sim_core.scenario.player_yaw;
                    let forward = glam::Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
                    let right = glam::Vec3::new(player_yaw.cos(), 0.0, -player_yaw.sin());
                    let light_dir = glam::Vec3::new(player_yaw.sin(), 0.05, player_yaw.cos())
                        .normalize_or_zero();
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
                            let f0 = (cur_cursor.0 as usize) % frame_count;
                            let f1 = (cur_cursor.1 as usize) % frame_count;
                            let mut blended_frame = interpolate_frames(
                                &c.anm.frames[f0],
                                &c.anm.frames[f1],
                                cur_cursor.2,
                            );

                            if let Some(prev) = prev_cursor {
                                let pf0 = (prev.0 as usize) % frame_count;
                                let pf1 = (prev.1 as usize) % frame_count;
                                let prev_frame = interpolate_frames(
                                    &c.anm.frames[pf0],
                                    &c.anm.frames[pf1],
                                    prev.2,
                                );
                                blended_frame =
                                    interpolate_frames(&prev_frame, &blended_frame, blend);
                            }

                            let dt_physics = 1.0 / 60.0;
                            c.secondary_physics.step(dt_physics, player_pos, player_yaw);
                            let mut dynamic_locals = c.bind_locals.clone();
                            c.secondary_physics.apply_to_locals(&mut dynamic_locals);

                            let pose =
                                evaluate_pose(&c.model.parents, &dynamic_locals, &blended_frame);
                            let local_skin =
                                compute_skin_matrices(&pose, &c.model.initial_matrices);

                            let (s, c_yaw) = player_yaw.sin_cos();
                            let world_mat = [
                                [c_yaw, 0.0, s, player_pos.x],
                                [0.0, 1.0, 0.0, player_pos.y],
                                [-s, 0.0, c_yaw, player_pos.z],
                                [0.0, 0.0, 0.0, 1.0],
                            ];

                            skin_matrices.reserve(local_skin.len());
                            for m_skin in &local_skin {
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

                    if is_in_game {
                        state.render_ctx.render_scene(
                            Some(&state.gpu_map),
                            Some(&state.map_texture_bind_groups),
                            &skeletal_draw,
                        );
                    }
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

                    // If in 3D Model, Animation & Texture Viewer room, render interactive HUD overlay
                    if is_viewer {
                        let tint = [1.0f32, 1.0, 1.0, 1.0];
                        state.render_ctx.queue.write_buffer(
                            &state.menu_uniform_buffer,
                            0,
                            bytemuck::cast_slice(&tint),
                        );
                        state.menu_rasterizer.clear();

                        let white = [225, 225, 225, 255];
                        let yellow = [245, 210, 65, 255];
                        let cyan = [105, 215, 235, 255];
                        let green = [120, 230, 150, 255];
                        let muted = [165, 165, 170, 255];

                        let entry = state.model_viewer.current_entry();
                        let cat_str = entry.category.label();
                        let total_models = BESTIARY_CATALOG.len();
                        let cur_idx = state.model_viewer.selected_model_index + 1;

                        state.menu_rasterizer.draw_text(
                            "3D MODEL, ANIMATION & TEXTURE VIEWER [BESTIARY DEBUG]",
                            24,
                            18,
                            2,
                            yellow,
                            true,
                        );
                        state.menu_rasterizer.draw_text(
                            &format!(
                                "MODEL [{}/{}] ({}) : {}",
                                cur_idx, total_models, cat_str, entry.display_name
                            ),
                            24,
                            46,
                            2,
                            white,
                            true,
                        );

                        if let Some(ref c) = state.viewer_asset {
                            let tri_count = (c.gpu_mesh.total_indices / 3) as usize;
                            state.menu_rasterizer.draw_text(
                                &format!(
                                    "MESH STATS : {} BONES | {} VERTICES | {} TRIS | {} SUBMESHES | {} TEXTURES",
                                    c.model.bone_count,
                                    c.model.vertices.positions.len(),
                                    tri_count,
                                    c.model.primitives.len(),
                                    c.model.textures.len()
                                ),
                                24,
                                72,
                                1,
                                cyan,
                                true,
                            );
                        }

                        let anm_name = if entry.anm_paths.is_empty() {
                            "STATIC BIND POSE"
                        } else {
                            entry.anm_paths
                                [state.model_viewer.selected_anm_index % entry.anm_paths.len()]
                        };
                        let play_status = if state.model_viewer.is_paused {
                            "PAUSED"
                        } else {
                            "PLAYING"
                        };
                        state.menu_rasterizer.draw_text(
                            &format!(
                                "ANIM CLIP [{}/{}] : {} | {} | FRAME {:03.0}/{:03} | SPEED {:.1}X",
                                state.model_viewer.selected_anm_index + 1,
                                entry.anm_paths.len().max(1),
                                anm_name,
                                play_status,
                                state.model_viewer.current_frame.floor(),
                                state.model_viewer.total_frames,
                                state.model_viewer.playback_speed
                            ),
                            24,
                            92,
                            1,
                            green,
                            true,
                        );

                        state.menu_rasterizer.draw_text(
                            &format!(
                                "SHADING : {} | OVERLAYS: BONES={} WIRE={}",
                                state.model_viewer.render_mode.label(),
                                if state.model_viewer.skeleton_overlay {
                                    "ON"
                                } else {
                                    "OFF"
                                },
                                if state.model_viewer.wireframe_overlay {
                                    "ON"
                                } else {
                                    "OFF"
                                }
                            ),
                            24,
                            112,
                            1,
                            white,
                            true,
                        );

                        state.menu_rasterizer.draw_text(
                            &format!(
                                "TEXTURES : {} | STAGE LIGHT : {} | TURNTABLE : {}",
                                state.model_viewer.texture_source.label(),
                                state.model_viewer.lighting_rig.label(),
                                if state.model_viewer.turntable_enabled {
                                    "AUTO-SPIN ON"
                                } else {
                                    "OFF"
                                }
                            ),
                            24,
                            132,
                            1,
                            cyan,
                            true,
                        );

                        // Bottom hotkey legend
                        state.menu_rasterizer.draw_text(
                            "[N/P or PgDn/PgUp] Next/Prev Model  [M/Tab] Next .ANM Clip  [Space] Play/Pause  [[ / ]] Step Frame  [-/+] Speed  [R] Reverse",
                            24,
                            666,
                            1,
                            muted,
                            true,
                        );
                        state.menu_rasterizer.draw_text(
                            "[V] Render Mode  [B] Bone Skeleton  [G] Wireframe  [H] Vanilla/HD Textures  [L] Stage Light  [T] Turntable  [WASD/Mouse] Orbit  [Q/E] Zoom  [Esc] Exit",
                            24,
                            688,
                            1,
                            muted,
                            true,
                        );

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
                                    let y = 250 + idx * 54;
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

    // Parse CLI arguments for test rooms, model viewer, & fog testing
    let args: Vec<String> = std::env::args().collect();
    let mut initial_test_room = None;
    let mut initial_fog_debug = false;
    let mut initial_viewer_model = None;

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
        } else if let Some(room_str) = arg.strip_prefix("--test-room=") {
            initial_test_room = match room_str {
                "ob01" | "observation" | "outdoor" => Some(TestRoomKind::ObservationDeck),
                "er01" | "restroom" | "toilet" => Some(TestRoomKind::RestroomStart),
                "rr21" | "boiler" => Some(TestRoomKind::HotelBoilerRoom),
                "runway" | "fog" => Some(TestRoomKind::SyntheticFogRunway),
                "fallback" | "sandbox" => Some(TestRoomKind::FallbackSandbox),
                "viewer" | "model-viewer" | "models" | "bestiary" => {
                    Some(TestRoomKind::ModelAndTextureViewer)
                }
                _ => {
                    tracing::warn!(
                        "Unknown test room: '{}', defaulting to Observation Deck",
                        room_str
                    );
                    Some(TestRoomKind::ObservationDeck)
                }
            };
        }
        i += 1;
    }

    if let Some(r) = initial_test_room {
        tracing::info!("CLI parameter selected test room: {:?}", r);
    }
    if let Some(ref m) = initial_viewer_model {
        tracing::info!("CLI parameter selected initial model viewer entry: '{}'", m);
    }

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp {
        state: None,
        initial_test_room,
        initial_fog_debug,
        initial_viewer_model,
    };
    event_loop.run_app(&mut app).expect("Event loop error");
}
