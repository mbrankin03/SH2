//! Interactive Desktop Playable Test Build for Silent Hill 2 Open Port.
//! Runs the pure PS2 camera, 60 Hz simulation core, volumetric fog, character skeletal animation, and gamepad controls.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use sh2_app::synthetic_geometry::{
    create_fallback_room_geometry, create_model_viewer_pedestal_geometry,
    create_synthetic_fog_runway_geometry,
};
use sh2_app::AppMenuController;
use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames, parse_anm, AnimationFile, AnimationFrame,
};
use sh2_assets::formats::bink::{decode_bink_audio_track, BinkHeader, BinkVideoStream};
use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_assets::formats::TextureLoader;
use sh2_audio::{play_menu_sound, AudioEngine, MenuAudioBank, SoundCueKind};
use sh2_core::{ControlScheme, ScenarioKind, SecondaryBonePhysics, SimulationCore};
use sh2_debug::{
    build_skeleton_debug_lines, build_wireframe_lines, FmvCatalogEntry, FmvVideoSource,
    FmvViewerState, ModelViewerEntry, ModelViewerState, StageLightingRig, TextureSourceMode,
    ViewerRenderMode, BESTIARY_CATALOG, FMV_CATALOG,
};
use sh2_renderer::{
    FmvUniforms, FmvVideoPipeline, GpuMap, GpuSkeletalMesh, GpuYuvVideoSurface, RenderContext,
    RendererConfig, SurfaceBlitter,
};
use sh2_ui::menu::{MenuNavAction, MenuSoundCue, TestRoomKind, TitlePhase};
use sh2_ui::pause::{PauseAction, PauseSubPhase};
use sh2_ui::{ButtonSpriteSheet, LoadingScreen, TextRasterizer};

/// Loaded character asset representation (model hierarchy, bind locals, and animation tracks).
struct CharacterAsset {
    #[allow(dead_code)]
    name: &'static str,
    gpu_mesh: GpuSkeletalMesh,
    model: ModelFile,
    bind_locals: Vec<[[f32; 4]; 4]>,
    dynamic_locals: Vec<[[f32; 4]; 4]>,
    pose_scratch: Vec<[[f32; 4]; 4]>,
    skin_scratch: Vec<[[f32; 4]; 4]>,
    anm: AnimationFile,
    vanilla_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    hd_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    secondary_physics: SecondaryBonePhysics,
}

/// Locate and load authentic `.cld` collision file for the given test room, falling back to procedural bounds.
fn find_cld_file_for_room(room: TestRoomKind) -> sh2_core::collision::CollisionWorld {
    let resolver = sh2_assets::AssetPathResolver::resolve();

    let cld_relative = match room {
        TestRoomKind::ObservationDeck => Some("bg/ob/ob01.cld"),
        TestRoomKind::RestroomStart => Some("bg/er/er01.cld"),
        TestRoomKind::HotelBoilerRoom => Some("bg/rr/rr21.cld"),
        _ => None,
    };

    if let Some(rel) = cld_relative {
        if let Some(p) = resolver.resolve_file(rel) {
            if let Ok(data) = std::fs::read(&p) {
                if let Ok(cld) = sh2_assets::formats::cld::parse_cld(&data) {
                    tracing::info!(
                        "Loaded authentic .cld collision file ({}) with {} floors and {} walls",
                        p.display(),
                        cld.floors.len(),
                        cld.walls.len()
                    );
                    return sh2_core::collision::CollisionWorld::from_cld(&cld);
                }
            }
        }
    }

    if room == TestRoomKind::CombatArena {
        tracing::info!("Using open unobstructed arena collision bounds for CombatArena");
        // Wide 12000x16000 room with 3500 height and NO center pillar (pw=0, pl=0)
        // so the camera raycast is never blocked or clamped near the center.
        return sh2_core::collision::CollisionWorld::create_synthetic_room(
            12000.0, 16000.0, 3500.0, 0.0, 0.0,
        );
    }

    tracing::warn!(
        "Using synthetic fallback room collision bounds for {:?}",
        room
    );
    sh2_core::collision::CollisionWorld::create_synthetic_room(
        8000.0, 12000.0, 3000.0, 600.0, 600.0,
    )
}

/// Locate and load authentic `.cam` camera volume file for the given test room, checking SH2:EE overrides first.
fn find_cam_file_for_room(room: TestRoomKind) -> Option<sh2_assets::formats::cam::CamFile> {
    let resolver = sh2_assets::AssetPathResolver::resolve();

    let cam_rel = match room {
        TestRoomKind::RestroomStart => Some("bg/er/er01.cam"),
        TestRoomKind::HotelBoilerRoom => Some("bg/rr/rr21.cam"),
        _ => None,
    };

    if let Some(rel) = cam_rel {
        if let Some(p) = resolver.resolve_file(rel) {
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(cam) = sh2_assets::formats::cam::parse_cam(&bytes) {
                    tracing::info!(
                        "Loaded authentic .cam camera file ({}) with {} road records",
                        p.display(),
                        cam.records.len()
                    );
                    return Some(cam);
                }
            }
        }
    }

    None
}
fn find_map_file_for_room(
    room: TestRoomKind,
) -> (MapFile, glam::Vec3, f32, sh2_renderer::fog::FogUniforms) {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    match room {
        TestRoomKind::ObservationDeck => {
            for base in &base_roots {
                let loader = TextureLoader::new(
                    PathBuf::from(base).join("data"),
                    Some(PathBuf::from(base).join("sh2e")),
                );
                if let Ok(map) = loader.load_map_with_hd_overrides("bg/ob/ob01.map", "ob01") {
                    tracing::info!(
                        "Loaded authentic Observation Deck level geometry (ob01.map with SH2:EE HD overrides): base={}",
                        base
                    );
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
                let loader = TextureLoader::new(
                    PathBuf::from(base).join("data"),
                    Some(PathBuf::from(base).join("sh2e")),
                );
                if let Ok(map) = loader.load_map_with_hd_overrides("bg/er/er01.map", "er01") {
                    tracing::info!(
                        "Loaded authentic Opening Restroom level geometry (er01.map with SH2:EE HD overrides): base={}",
                        base
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
                let loader = TextureLoader::new(
                    PathBuf::from(base).join("data"),
                    Some(PathBuf::from(base).join("sh2e")),
                );
                if let Ok(map) = loader.load_map_with_hd_overrides("bg/rr/rr21.map", "rr21") {
                    tracing::info!(
                        "Loaded authentic Lakeview Hotel Boiler Room level geometry (rr21.map with SH2:EE HD overrides): base={}",
                        base
                    );
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
                    // Authentic rr21 hotel boiler room entrance coordinates (open doorway corridor, outside boiler collision hulls):
                    // Floor Y = 0.0, X = -100850.0, Z = -59400.0, facing North-East into the room (yaw = 2.0 rad)
                    return (map, glam::Vec3::new(-100850.0, 0.0, -59400.0), 2.0, fog);
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
        TestRoomKind::ModelAndTextureViewer
        | TestRoomKind::FmvCutsceneViewer
        | TestRoomKind::LoadingScreenTest => {
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
        TestRoomKind::CombatArena => {
            tracing::info!("Generated Combat Testing Arena Stage (SH2-13).");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.12, 0.13, 0.16, 1.0],
                fog_near: 2500.0,
                fog_far: 20000.0,
                density: 0.12,
                time: 0.0,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                _padding: 0.0,
            };
            (
                create_fallback_room_geometry(),
                // Spawn James at Z = -1500.0, facing North (0.0 rad) directly looking toward the enemies (Z = 1200..2000)
                glam::Vec3::new(0.0, 0.0, -1500.0),
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
            let a00 = rgba[p00 + 3] as f32;
            let a10 = rgba[p10 + 3] as f32;
            let a01 = rgba[p01 + 3] as f32;
            let a11 = rgba[p11 + 3] as f32;

            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;

            let out_a = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
            out[dst + 3] = out_a.clamp(0.0, 255.0) as u8;

            // Weight RGB interpolation by alpha when mixing cutout (a == 0) and visible texels
            // so transparent cutout texels never darken or fringe adjacent hair/alpha edges.
            let aw00 = w00 * a00.max(1.0);
            let aw10 = w10 * a10.max(1.0);
            let aw01 = w01 * a01.max(1.0);
            let aw11 = w11 * a11.max(1.0);
            let aw_sum = (aw00 + aw10 + aw01 + aw11).max(1e-5);

            for c in 0..3 {
                let v00 = rgba[p00 + c] as f32;
                let v10 = rgba[p10 + c] as f32;
                let v01 = rgba[p01 + c] as f32;
                let v11 = rgba[p11 + c] as f32;
                let mut val = (v00 * aw00 + v10 * aw10 + v01 * aw01 + v11 * aw11) / aw_sum;
                // Subtle micro-contrast sharpening on RGB channels only when all 4 neighbors are non-transparent
                if a00 > 0.0 && a10 > 0.0 && a01 > 0.0 && a11 > 0.0 {
                    let lap = v00 * 4.0 - (v10 + v01 + v11 + v00);
                    val = (val + lap * 0.15).clamp(0.0, 255.0);
                }
                out[dst + c] = val.clamp(0.0, 255.0) as u8;
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
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
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
                            Arc::clone(&van_bg)
                        }
                    } else {
                        Arc::clone(&van_bg)
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
        vanilla_texture_bind_groups.push(Arc::clone(&ctx.default_texture_bind_group));
        hd_texture_bind_groups.push(Arc::clone(&ctx.default_texture_bind_group));
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

    let dynamic_locals = bind_locals.clone();
    let pose_scratch = Vec::with_capacity(model.bone_count);
    let skin_scratch = Vec::with_capacity(model.bone_count);

    Some(CharacterAsset {
        name: character_name,
        gpu_mesh,
        model,
        bind_locals,
        dynamic_locals,
        pose_scratch,
        skin_scratch,
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

/// Active decoded FMV movie stream, GPU planar YUV surface, and metadata.
struct ActiveFmvSession {
    entry: &'static FmvCatalogEntry,
    header: BinkHeader,
    stream: BinkVideoStream,
    gpu_surface: GpuYuvVideoSurface,
    #[allow(dead_code)]
    is_hd_source: bool,
    frame_accumulator_sec: f32,
    /// Cached decoded audio (sample rate, s16le PCM) so loops/restarts can replay it.
    audio: Option<(u32, Vec<u8>)>,
    /// Cutscene paired background music (ADX/AIX decoded data, retail volume, fade) to replay on restart.
    bgm_stream: Option<(sh2_assets::DecodedAdx, f32, std::time::Duration)>,
}

/// Locate and open a `.bik` movie stream for the requested [`FmvCatalogEntry`] and source preference.
fn open_fmv_session(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fmv_pipeline: &FmvVideoPipeline,
    audio_engine: &mut Option<AudioEngine>,
    fmv_viewer: &mut FmvViewerState,
    entry: &'static FmvCatalogEntry,
    source_pref: FmvVideoSource,
) -> Option<ActiveFmvSession> {
    let upscaled_mp4_roots = ["tools/vsr_pipeline/output", "movie", "sh2e/movie"];

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    let og_hq_roots = ["sh2_enhanced_edition_og_hq_fmvs-50-1-0-1770228966/movie"];

    // MP4 candidate filenames for locally upscaled VSR masters (e.g. `toilet_4k_master.mp4`, `toilet.mp4`)
    let mp4_cands = [
        format!("{}_4k_master.mp4", entry.id),
        format!("{}.mp4", entry.id),
    ];

    let mut chosen_path: Option<(PathBuf, bool)> = None;
    match source_pref {
        FmvVideoSource::Upscaled4kMp4 => {
            for dir in &upscaled_mp4_roots {
                for mp4_name in &mp4_cands {
                    let cand = PathBuf::from(dir).join(mp4_name);
                    if cand.exists() {
                        chosen_path = Some((cand, true));
                        break;
                    }
                }
                if chosen_path.is_some() {
                    break;
                }
            }
        }
        FmvVideoSource::OgHqPs2Masters => {
            for dir in &og_hq_roots {
                let cand = PathBuf::from(dir).join(entry.filename);
                if cand.exists() {
                    chosen_path = Some((cand, true));
                    break;
                }
            }
        }
        FmvVideoSource::Enhanced1080p60 => {
            for base in &base_roots {
                let sh2e_cand = PathBuf::from(base).join("sh2e/movie").join(entry.filename);
                if sh2e_cand.exists() {
                    chosen_path = Some((sh2e_cand, true));
                    break;
                }
            }
        }
        FmvVideoSource::VanillaPc => {
            for base in &base_roots {
                let data_cand = PathBuf::from(base).join("data/movie").join(entry.filename);
                if data_cand.exists() {
                    chosen_path = Some((data_cand, false));
                    break;
                }
            }
        }
    }

    // Graceful fallback chain if user preference is missing for this cutscene
    if chosen_path.is_none() {
        for dir in &upscaled_mp4_roots {
            for mp4_name in &mp4_cands {
                let cand = PathBuf::from(dir).join(mp4_name);
                if cand.exists() {
                    chosen_path = Some((cand, true));
                    break;
                }
            }
            if chosen_path.is_some() {
                break;
            }
        }
    }
    if chosen_path.is_none() {
        for dir in &og_hq_roots {
            let cand = PathBuf::from(dir).join(entry.filename);
            if cand.exists() {
                chosen_path = Some((cand, true));
                break;
            }
        }
    }
    if chosen_path.is_none() {
        for base in &base_roots {
            let sh2e_cand = PathBuf::from(base).join("sh2e/movie").join(entry.filename);
            if sh2e_cand.exists() {
                chosen_path = Some((sh2e_cand, true));
                break;
            }
        }
    }
    if chosen_path.is_none() {
        for base in &base_roots {
            let data_cand = PathBuf::from(base).join("data/movie").join(entry.filename);
            if data_cand.exists() {
                chosen_path = Some((data_cand, false));
                break;
            }
        }
    }

    let synth_header = BinkHeader {
        magic: *b"BIKi",
        file_size: 1920 * 1080 * 300,
        frame_count: 600,
        largest_frame_size: 65536,
        width: 1920,
        height: 1080,
        fps_num: 2997,
        fps_den: 50,
        video_flags: 0,
        audio_tracks: Vec::new(),
        indexed_frames: 600,
    };

    let mut fmv_audio: Option<(u32, Vec<u8>)> = None;
    let mut cutscene_bgm_data: Option<(sh2_assets::DecodedAdx, f32, std::time::Duration)> = None;
    let (mut stream, is_hd_source) = if let Some((ref path, is_hd)) = chosen_path {
        match BinkVideoStream::open(path) {
            Ok(s) => {
                tracing::info!(
                    "[FMV Player] Opened Bink movie '{}' ({}) -> {}x{} @ {:.2} fps ({} frames, synthetic={})",
                    entry.display_name,
                    path.display(),
                    s.header.width,
                    s.header.height,
                    s.header.fps(),
                    s.header.frame_count,
                    s.path.is_none()
                );
                if let Some(ref mut engine) = audio_engine {
                    // sh2e murder.bik: track 1 is the restored uncut VHS audio (Mary), matching
                    // the Enhancements VHSAudio patch; open.bik track 1 is the voiced mix.
                    let default_track = if matches!(entry.filename, "murder.bik" | "open.bik") {
                        1
                    } else {
                        0
                    };
                    let track = fmv_viewer.audio_track.unwrap_or(default_track);
                    tracing::info!(
                        "[FMV Player] Audio track {} of {}",
                        track,
                        s.header.audio_tracks.len()
                    );
                    match decode_bink_audio_track(path, &s.header, track) {
                        Ok(decoded) if !decoded.pcm_s16le.is_empty() => {
                            if let Err(e) =
                                engine.play_fmv_pcm(decoded.sample_rate, &decoded.pcm_s16le, 1.0)
                            {
                                tracing::warn!("[FMV Player] Audio playback warning: {:?}", e);
                            }
                            fmv_audio = Some((decoded.sample_rate, decoded.pcm_s16le));
                        }
                        _ => {
                            engine.stop_fmv_audio(std::time::Duration::from_millis(50));
                        }
                    }

                    // Handle paired cutscene BGM streaming (SH2-39 CRI ADX / AIX containers)
                    if let Some(cutscene_map) =
                        sh2_audio::cutscene_bgm::find_cutscene_audio_mapping(entry.id)
                    {
                        match cutscene_map.bgm_action {
                            sh2_audio::cutscene_bgm::CutsceneBgmAction::StartNew => {
                                if let Some(rel_bgm) = cutscene_map.bgm_file {
                                    let mut resolved_bgm: Option<PathBuf> = None;
                                    for base in &base_roots {
                                        if let Some(p) =
                                            sh2_audio::cutscene_bgm::resolve_bgm_stream_path(
                                                base, rel_bgm,
                                            )
                                        {
                                            resolved_bgm = Some(p);
                                            break;
                                        }
                                    }
                                    if let Some(full_bgm_path) = resolved_bgm {
                                        if let Ok(bytes) = std::fs::read(&full_bgm_path) {
                                            let decoded_res = if rel_bgm.ends_with(".aix") {
                                                sh2_assets::formats::aix::decode_aix_mixed(&bytes)
                                            } else {
                                                sh2_assets::decode_adx(&bytes)
                                            };
                                            match decoded_res {
                                                Ok(adx) => {
                                                    tracing::info!(
                                                        "[FMV Player] Playing cutscene BGM '{}' (vol {:.2}, fade {:?})",
                                                        rel_bgm,
                                                        cutscene_map.retail_master_volume,
                                                        cutscene_map.fade_in
                                                    );
                                                    if let Err(e) = engine.play_bgm_with_fade(
                                                        &adx,
                                                        cutscene_map.retail_master_volume,
                                                        cutscene_map.fade_in,
                                                    ) {
                                                        tracing::warn!("[FMV Player] BGM playback warning: {:?}", e);
                                                    }
                                                    cutscene_bgm_data = Some((
                                                        adx,
                                                        cutscene_map.retail_master_volume,
                                                        cutscene_map.fade_in,
                                                    ));
                                                }
                                                Err(e) => {
                                                    tracing::warn!("[FMV Player] Failed to decode cutscene BGM '{}': {:?}", rel_bgm, e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            sh2_audio::cutscene_bgm::CutsceneBgmAction::KeepActive => {
                                tracing::info!(
                                    "[FMV Player] Cutscene '{}' keeps active ambient BGM (retail parity)",
                                    entry.id
                                );
                            }
                            sh2_audio::cutscene_bgm::CutsceneBgmAction::Silence => {
                                tracing::info!(
                                    "[FMV Player] Cutscene '{}' silences BGM track (self-contained mix)",
                                    entry.id
                                );
                                engine.stop_bgm(cutscene_map.fade_out);
                            }
                        }
                    }
                }
                (s, is_hd)
            }
            Err(e) => {
                tracing::warn!(
                    "[FMV Player] Failed to parse Bink header at {:?}: {:?}; using synthetic 1080p60 stream",
                    path,
                    e
                );
                (
                    BinkVideoStream::new_synthetic(synth_header.clone()),
                    source_pref != FmvVideoSource::VanillaPc,
                )
            }
        }
    } else {
        tracing::info!(
            "[FMV Player] Movie '{}' not found on disk; launching synthetic 1080p60 test stream",
            entry.filename
        );
        if let Some(ref mut engine) = audio_engine {
            engine.stop_bgm(std::time::Duration::from_millis(50));
        }
        (
            BinkVideoStream::new_synthetic(synth_header),
            source_pref != FmvVideoSource::VanillaPc,
        )
    };

    let header = stream.header.clone();
    fmv_viewer.configure_stream_meta(
        header.width,
        header.height,
        header.frame_count,
        header.fps(),
    );
    let gpu_surface = fmv_pipeline.create_video_surface(device, header.width, header.height);

    // Decode and upload initial frame 0 immediately so the first render pass shows video
    if let Some(first_frame) = stream.next_frame() {
        fmv_pipeline.upload_frame(queue, &gpu_surface, first_frame);
    }

    Some(ActiveFmvSession {
        entry,
        header,
        stream,
        gpu_surface,
        is_hd_source,
        frame_accumulator_sec: 0.0,
        audio: fmv_audio,
        bgm_stream: cutscene_bgm_data,
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
    combat_arena: sh2_core::combat::CombatArena,
    enemy_scu_asset: Option<CharacterAsset>,
    enemy_mkn_asset: Option<CharacterAsset>,
    enemy_nse_asset: Option<CharacterAsset>,
    enemy_red_asset: Option<CharacterAsset>,
    enemy_pap_asset: Option<CharacterAsset>,
    enemy_ike_asset: Option<CharacterAsset>,
    enemy_arm_asset: Option<CharacterAsset>,
    enemy_tyu_asset: Option<CharacterAsset>,
    enemy_bos_asset: Option<CharacterAsset>,
    key_ready_held: bool,
    key_attack_pressed: bool,
    key_attack_held: bool,
    combat_enemy_idx: usize,
    struggle_button_press_count: u32,
    model_viewer: ModelViewerState,
    viewer_asset: Option<CharacterAsset>,
    loaded_viewer_model_idx: usize,
    loaded_viewer_anm_idx: usize,
    fmv_pipeline: FmvVideoPipeline,
    fmv_viewer: FmvViewerState,
    fmv_session: Option<ActiveFmvSession>,
    loaded_fmv_movie_idx: usize,
    loaded_fmv_source: FmvVideoSource,
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
    last_menu_revision: u64,
    active_audio_device: Option<String>,
    loading_screen: Option<LoadingScreen>,
    debug_collision_overlay: bool,
    debug_camera_hud: bool,
    skin_matrices_scratch: Vec<[[f32; 4]; 4]>,
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

                    // If in the 1080p60 FMV Cutscene Viewer / Theater, route hotkeys to FmvViewerState
                    if state.active_test_room == TestRoomKind::FmvCutsceneViewer {
                        if pressed {
                            match key_code {
                                winit::keyboard::KeyCode::Escape
                                | winit::keyboard::KeyCode::Backspace => {
                                    if let Some(ref mut engine) = state.audio_engine {
                                        engine.stop_bgm(std::time::Duration::from_millis(120));
                                        engine
                                            .stop_fmv_audio(std::time::Duration::from_millis(120));
                                    }
                                    state.fmv_session = None;
                                    state.menu_ctrl.in_game_active = false;
                                    state.menu_ctrl.title_sm.phase = TitlePhase::OptionsMenu;
                                    state.menu_ctrl.title_sm.fade_alpha = 1.0;
                                    tracing::info!(
                                        "Returned from FMV Cutscene Player to Options menu."
                                    );
                                }
                                winit::keyboard::KeyCode::KeyN
                                | winit::keyboard::KeyCode::PageDown
                                | winit::keyboard::KeyCode::ArrowRight
                                | winit::keyboard::KeyCode::KeyD => {
                                    state.fmv_viewer.audio_track = None;
                                    state.fmv_viewer.next_movie();
                                    tracing::info!(
                                        "[FMV Player] Selected cutscene [{}/{}]: {}",
                                        state.fmv_viewer.selected_movie_index + 1,
                                        FMV_CATALOG.len(),
                                        state.fmv_viewer.current_entry().display_name
                                    );
                                }
                                winit::keyboard::KeyCode::KeyP
                                | winit::keyboard::KeyCode::PageUp
                                | winit::keyboard::KeyCode::ArrowLeft
                                | winit::keyboard::KeyCode::KeyA => {
                                    state.fmv_viewer.audio_track = None;
                                    state.fmv_viewer.prev_movie();
                                    tracing::info!(
                                        "[FMV Player] Selected cutscene [{}/{}]: {}",
                                        state.fmv_viewer.selected_movie_index + 1,
                                        FMV_CATALOG.len(),
                                        state.fmv_viewer.current_entry().display_name
                                    );
                                }
                                winit::keyboard::KeyCode::Space => {
                                    state.fmv_viewer.toggle_pause();
                                    if let Some(ref mut engine) = state.audio_engine {
                                        engine.set_bgm_paused(state.fmv_viewer.is_paused);
                                    }
                                    tracing::info!(
                                        "[FMV Player] Playback {}",
                                        if state.fmv_viewer.is_paused {
                                            "PAUSED"
                                        } else {
                                            "PLAYING"
                                        }
                                    );
                                }
                                winit::keyboard::KeyCode::KeyT => {
                                    let n = state
                                        .fmv_session
                                        .as_ref()
                                        .map(|x| x.header.audio_tracks.len())
                                        .unwrap_or(1)
                                        .max(1);
                                    let cur = state.fmv_viewer.audio_track.unwrap_or(
                                        if matches!(
                                            state.fmv_viewer.current_entry().filename,
                                            "murder.bik" | "open.bik"
                                        ) {
                                            1
                                        } else {
                                            0
                                        },
                                    );
                                    state.fmv_viewer.audio_track = Some((cur + 1) % n);
                                    state.fmv_viewer.is_paused = false;
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
                                }
                                winit::keyboard::KeyCode::KeyU => {
                                    state.fmv_viewer.sharpen_level =
                                        (state.fmv_viewer.sharpen_level + 1) % 4;
                                    tracing::info!(
                                        "[FMV Player] Upscale sharpening level: {}",
                                        state.fmv_viewer.sharpen_level
                                    );
                                }
                                winit::keyboard::KeyCode::KeyR => {
                                    state.fmv_viewer.is_paused = false;
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
                                    tracing::info!(
                                        "[FMV Player] Restarted cutscene: {}",
                                        state.fmv_viewer.current_entry().display_name
                                    );
                                }
                                winit::keyboard::KeyCode::KeyH => {
                                    state.fmv_viewer.toggle_hd_source();
                                    tracing::info!(
                                        "[FMV Player] Switched video source preference: {}",
                                        if state.fmv_viewer.prefer_hd_source {
                                            "SH2:EE 1080p60 (sh2e/movie/)"
                                        } else {
                                            "Vanilla PC 30fps (data/movie/)"
                                        }
                                    );
                                }
                                winit::keyboard::KeyCode::KeyV => {
                                    state.fmv_viewer.aspect_mode =
                                        state.fmv_viewer.aspect_mode.next();
                                    tracing::info!(
                                        "[FMV Player] Aspect ratio mode: {}",
                                        state.fmv_viewer.aspect_mode.label()
                                    );
                                }
                                winit::keyboard::KeyCode::KeyG => {
                                    state.fmv_viewer.film_grain_enabled =
                                        !state.fmv_viewer.film_grain_enabled;
                                    tracing::info!(
                                        "[FMV Player] Film grain over cutscene: {}",
                                        state.fmv_viewer.film_grain_enabled
                                    );
                                }
                                winit::keyboard::KeyCode::KeyS => {
                                    state.fmv_viewer.subtitles_enabled =
                                        !state.fmv_viewer.subtitles_enabled;
                                    tracing::info!(
                                        "[FMV Player] Cutscene subtitles: {}",
                                        state.fmv_viewer.subtitles_enabled
                                    );
                                }
                                winit::keyboard::KeyCode::Tab => {
                                    state.fmv_viewer.show_hud = !state.fmv_viewer.show_hud;
                                }
                                _ => {}
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
                            winit::keyboard::KeyCode::KeyK if pressed => {
                                state.debug_collision_overlay = !state.debug_collision_overlay;
                                tracing::info!(
                                    "[Debug] Collision wireframe overlay (clDbgDrawBGColiPoly): {}",
                                    state.debug_collision_overlay
                                );
                            }
                            _ => {}
                        }
                        return;
                    }

                    match key_code {
                        winit::keyboard::KeyCode::Escape | winit::keyboard::KeyCode::Backspace
                            if pressed =>
                        {
                            let stage_name = state.active_test_room.label();
                            // Check user configuration: save_anywhere (PC mode) vs restricted to red squares (PS2 mode)
                            let can_save =
                                state.menu_ctrl.options_menu.draft_config.game.save_anywhere;
                            state.menu_ctrl.pause_menu.open(
                                can_save,
                                stage_name,
                                state.sim_time as f64,
                            );
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
                        winit::keyboard::KeyCode::KeyK if pressed => {
                            state.debug_collision_overlay = !state.debug_collision_overlay;
                            tracing::info!(
                                "[Debug] Collision wireframe overlay (clDbgDrawBGColiPoly): {}",
                                state.debug_collision_overlay
                            );
                        }
                        winit::keyboard::KeyCode::F1 | winit::keyboard::KeyCode::F3 if pressed => {
                            state.debug_camera_hud = !state.debug_camera_hud;
                            tracing::info!(
                                "[Debug] Camera & Player telemetry HUD: {}",
                                state.debug_camera_hud
                            );
                        }
                        // Authentic Combat controls (Space / F / Enter to Attack, R / Shift to Ready, 1..6 to switch weapons)
                        winit::keyboard::KeyCode::KeyR => {
                            state.key_ready_held = pressed;
                        }
                        winit::keyboard::KeyCode::Space => {
                            if pressed && !state.key_attack_held {
                                state.key_attack_pressed = true;
                            }
                            state.key_attack_held = pressed;
                        }
                        winit::keyboard::KeyCode::Digit1 if pressed => {
                            state.combat_arena.player.weapon =
                                sh2_core::combat::WeaponGrip::WoodenPlank;
                            tracing::info!("[Combat] Equipped Wooden Plank");
                        }
                        winit::keyboard::KeyCode::Digit2 if pressed => {
                            state.combat_arena.player.weapon =
                                sh2_core::combat::WeaponGrip::SteelPipe;
                            tracing::info!("[Combat] Equipped Steel Pipe");
                        }
                        winit::keyboard::KeyCode::Digit3 if pressed => {
                            state.combat_arena.player.weapon =
                                sh2_core::combat::WeaponGrip::Handgun;
                            state.combat_arena.player.loaded_rounds[0] = 10;
                            state.combat_arena.player.spare_rounds[0] = 50;
                            tracing::info!("[Combat] Equipped Handgun (10/50)");
                        }
                        winit::keyboard::KeyCode::Digit4 if pressed => {
                            state.combat_arena.player.weapon =
                                sh2_core::combat::WeaponGrip::Shotgun;
                            state.combat_arena.player.loaded_rounds[1] = 6;
                            state.combat_arena.player.spare_rounds[1] = 30;
                            tracing::info!("[Combat] Equipped Shotgun (6/30)");
                        }
                        winit::keyboard::KeyCode::Digit5 if pressed => {
                            state.combat_arena.player.weapon =
                                sh2_core::combat::WeaponGrip::HuntingRifle;
                            state.combat_arena.player.loaded_rounds[2] = 4;
                            state.combat_arena.player.spare_rounds[2] = 20;
                            tracing::info!("[Combat] Equipped Hunting Rifle (4/20)");
                        }
                        winit::keyboard::KeyCode::Digit6 if pressed => {
                            state.combat_arena.player.weapon =
                                sh2_core::combat::WeaponGrip::GreatKnife;
                            tracing::info!("[Combat] Equipped Great Knife");
                        }
                        // Teleport / cycle selected enemy right in front of James for testing combat & grab interactions
                        winit::keyboard::KeyCode::Tab
                            if pressed && !state.combat_arena.enemies.is_empty() =>
                        {
                            let fwd = glam::Vec3::new(
                                state.sim_core.scenario.player_yaw.sin(),
                                0.0,
                                state.sim_core.scenario.player_yaw.cos(),
                            );
                            let target_pos = state.combat_arena.player_position + fwd * 350.0;
                            let facing_yaw =
                                state.sim_core.scenario.player_yaw + std::f32::consts::PI;

                            // Advance focus to next enemy
                            state.combat_enemy_idx =
                                (state.combat_enemy_idx + 1) % state.combat_arena.enemies.len();
                            let target_enemy =
                                &mut state.combat_arena.enemies[state.combat_enemy_idx];

                            // Teleport selected enemy in front of player and engage
                            target_enemy.position = target_pos;
                            target_enemy.yaw = facing_yaw;
                            target_enemy.is_aggroed = true;
                            target_enemy.flinch_timer = 0.0;
                            target_enemy.attack_cooldown = 0.2;

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

                    // Count any key press while held in grapple for authentic struggle meter drain (0x5565c0)
                    if pressed
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

                // Check and apply committed options configuration
                if let Some(cfg) = state.menu_ctrl.config_applied_pending.take() {
                    tracing::info!(
                        "[Options] Applying engine configuration: Resolution: {:?}, DisplayMode: {:?}, VSync: {}, AudioDevice: {:?}, MasterVol: {:.0}%, BGMVol: {:.0}%",
                        cfg.video.resolution,
                        cfg.video.display_mode,
                        cfg.video.vsync,
                        cfg.audio.selected_output_device.as_deref().unwrap_or("Default"),
                        cfg.audio.master_volume * 100.0,
                        cfg.audio.bgm_volume * 100.0,
                    );

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
                    state.sim_core.scenario.player_position = new_pos;
                    state.sim_core.scenario.player_yaw = new_yaw;
                    state.sim_core.camera.reset_to_player(new_pos, new_yaw);
                    state.active_test_room = new_room;
                    if new_room == TestRoomKind::CombatArena {
                        state.combat_arena = sh2_core::combat::CombatArena::new();
                        state.combat_arena.player.weapon =
                            sh2_core::combat::WeaponGrip::WoodenPlank;
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
                let is_fmv_viewer =
                    is_in_game && state.active_test_room == TestRoomKind::FmvCutsceneViewer;

                let is_paused = is_in_game && state.menu_ctrl.pause_menu.is_open;

                // Advance simulation time and frame counter for animated post-processing
                let dt_sec = if is_paused { 0.0 } else { dt.as_secs_f32() };
                if !is_paused {
                    state.frame_counter = state.frame_counter.wrapping_add(1);
                    state.sim_time += dt_sec;
                }

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

                if is_fmv_viewer {
                    // --- 1080P60 FMV CUTSCENE PLAYER PASS (SH2-10) ---
                    if let Some(ref mut session) = state.fmv_session {
                        let fps = (session.header.fps() as f32).max(1.0);
                        let frame_duration = 1.0 / fps;
                        if !state.fmv_viewer.is_paused {
                            session.frame_accumulator_sec += dt_sec.min(0.1);
                            let mut advanced = 0;
                            while session.frame_accumulator_sec >= frame_duration && advanced < 4 {
                                session.frame_accumulator_sec -= frame_duration;
                                advanced += 1;
                                if let Some(frame) = session.stream.next_frame() {
                                    state.fmv_pipeline.upload_frame(
                                        &state.render_ctx.queue,
                                        &session.gpu_surface,
                                        frame,
                                    );
                                } else {
                                    // Loop seamlessly on completion in theater mode
                                    session.stream.restart();
                                    if let Some(engine) = state.audio_engine.as_mut() {
                                        if let Some((sr, pcm)) = session.audio.as_ref() {
                                            let _ = engine.play_fmv_pcm(*sr, pcm, 1.0);
                                        }
                                        if let Some((adx, vol, fade)) = session.bgm_stream.as_ref()
                                        {
                                            let _ = engine.play_bgm_with_fade(adx, *vol, *fade);
                                        }
                                    }
                                    if let Some(frame) = session.stream.next_frame() {
                                        state.fmv_pipeline.upload_frame(
                                            &state.render_ctx.queue,
                                            &session.gpu_surface,
                                            frame,
                                        );
                                    }
                                    break;
                                }
                            }
                        }

                        let (uv_y_offset, uv_y_scale, video_aspect) =
                            state.fmv_viewer.compute_uv_crop_and_aspect();
                        let fmv_grain = if state.fmv_viewer.film_grain_enabled {
                            ((grain_pct as f32) / 100.0) * 0.45
                        } else {
                            0.0
                        };
                        let fmv_uniforms = FmvUniforms {
                            viewport_aspect: aspect,
                            video_aspect,
                            uv_y_offset,
                            uv_y_scale,
                            grain_intensity: fmv_grain,
                            time: state.sim_time,
                            fade_alpha: 1.0,
                            sharpen: match state.fmv_viewer.sharpen_level {
                                0 => 0.0,
                                1 => 0.35,
                                2 => 0.7,
                                _ => 1.0,
                            },
                        };
                        state
                            .fmv_pipeline
                            .update_uniforms(&state.render_ctx.queue, &fmv_uniforms);

                        let mut fmv_encoder = state.render_ctx.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("FMV Render Encoder"),
                            },
                        );
                        state.fmv_pipeline.render(
                            &mut fmv_encoder,
                            &session.gpu_surface,
                            &state.render_ctx.ldr_output_view,
                        );
                        state
                            .render_ctx
                            .queue
                            .submit(std::iter::once(fmv_encoder.finish()));
                    }
                } else if is_viewer {
                    // --- 3D MODEL, ANIMATION & TEXTURE VIEWER PASS (SH2-36) ---
                    let mut orbit_yaw_delta = 0.0f32;
                    let mut orbit_pitch_delta = 0.0f32;
                    let mut zoom_delta = 0.0f32;
                    let mut pan_up_delta = 0.0f32;

                    if !is_paused {
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
                    }

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

                    state.skin_matrices_scratch.clear();
                    let mut debug_lines = Vec::new();

                    if let Some(ref mut c) = state.viewer_asset {
                        let (f0, f1, alpha_frame) = state.model_viewer.sample_cursor();
                        let frame_count = c.anm.frames.len();
                        if frame_count > 0 {
                            let blended_frame = interpolate_frames(
                                &c.anm.frames[f0 % frame_count],
                                &c.anm.frames[f1 % frame_count],
                                alpha_frame,
                            );
                            evaluate_pose_into(
                                &c.model.parents,
                                &c.bind_locals,
                                &blended_frame,
                                &mut c.pose_scratch,
                            );
                            compute_skin_matrices_with_inv_into(
                                &c.pose_scratch,
                                &c.model.inv_initial_matrices,
                                &mut c.skin_scratch,
                            );

                            let (s, c_yaw) = state.model_viewer.turntable_yaw.sin_cos();
                            let world_mat = [
                                [c_yaw, 0.0, s, 0.0],
                                [0.0, 1.0, 0.0, 0.0],
                                [-s, 0.0, c_yaw, 0.0],
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

                            if state.model_viewer.render_mode == ViewerRenderMode::Wireframe
                                || state.model_viewer.wireframe_overlay
                            {
                                let has_variant_poses =
                                    c.model.primitives.iter().any(|pr| pr.pose_index > 0);
                                let has_pose_14 =
                                    c.model.primitives.iter().any(|pr| pr.pose_index == 14);
                                let has_pose_6 =
                                    c.model.primitives.iter().any(|pr| pr.pose_index == 6);
                                let mut all_tris = Vec::new();
                                for (k, pr) in c.model.primitives.iter().enumerate() {
                                    let is_active = if has_variant_poses {
                                        pr.pose_index == 0
                                            || pr.pose_index == 3
                                            || (has_pose_14 && pr.pose_index == 14)
                                            || (!has_pose_14 && has_pose_6 && pr.pose_index == 6)
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
                                    &state.skin_matrices_scratch,
                                    &all_tris,
                                    [0.15, 0.92, 0.65, 1.0],
                                ));
                            }

                            if state.model_viewer.render_mode == ViewerRenderMode::SkeletonJoints
                                || state.model_viewer.skeleton_overlay
                            {
                                debug_lines.extend(build_skeleton_debug_lines(
                                    &c.model.parents,
                                    &c.pose_scratch,
                                    &world_mat,
                                    45.0,
                                ));
                            }
                        }
                    }

                    let skeletal_draw = if let Some(ref c) = state.viewer_asset {
                        if !state.skin_matrices_scratch.is_empty() {
                            let tex_slice = match state.model_viewer.texture_source {
                                TextureSourceMode::EnhancedHd => {
                                    c.hd_texture_bind_groups.as_slice()
                                }
                                TextureSourceMode::VanillaPs2 => {
                                    c.vanilla_texture_bind_groups.as_slice()
                                }
                            };
                            vec![(
                                &c.gpu_mesh,
                                state.skin_matrices_scratch.as_slice(),
                                Some(tex_slice),
                            )]
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
                    if !is_paused {
                        state.sim_core.update(dt);

                        if state.active_test_room == TestRoomKind::CombatArena {
                            state.combat_arena.player_position =
                                state.sim_core.scenario.player_position;
                            state.combat_arena.player_yaw = state.sim_core.scenario.player_yaw;
                            let dt_sec = dt.as_secs_f32().min(0.05);
                            let atk_pressed = state.key_attack_pressed;
                            state.key_attack_pressed = false;

                            // Process struggle input if currently held by a creature (0x5565c0)
                            let struggle_presses = state.struggle_button_press_count;
                            state.struggle_button_press_count = 0;
                            if state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held
                            {
                                state.combat_arena.player.process_struggle_inputs(
                                    struggle_presses,
                                    dt_sec,
                                    false,
                                );
                            }

                            state.combat_arena.tick_isolated(
                                state.key_ready_held,
                                atk_pressed,
                                state.key_attack_held,
                                dt_sec,
                                state.combat_enemy_idx,
                            );

                            // Sync James player position and yaw back from arena (e.g. grapple turn/facing & enemy pushback)
                            state.sim_core.scenario.player_position =
                                state.combat_arena.player_position;
                            if state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held
                            {
                                state.sim_core.scenario.player_yaw = state.combat_arena.player_yaw;
                            }

                            // Drive James's skeletal animation when in combat states (Aim, Attack, Held struggle loop 457..=461, Hurt flinch, Fall)
                            match state.combat_arena.player.mode {
                                sh2_core::combat::CombatMode::Held => {
                                    state.sim_core.animation_locked = true;
                                    state.sim_core.animation.transition_to(
                                        sh2_core::animation::AnimationStateKind::Held,
                                        sh2_core::animation::JamesBasicClips::STRUGGLE_HELD,
                                    );
                                }
                                sh2_core::combat::CombatMode::Hurt => {
                                    state.sim_core.animation_locked = true;
                                    state.sim_core.animation.transition_to(
                                        sh2_core::animation::AnimationStateKind::Damage,
                                        sh2_core::animation::JamesBasicClips::HURT,
                                    );
                                }
                                sh2_core::combat::CombatMode::Dying
                                | sh2_core::combat::CombatMode::Dead => {
                                    state.sim_core.animation_locked = true;
                                    state.sim_core.animation.transition_to(
                                        sh2_core::animation::AnimationStateKind::Fall,
                                        sh2_core::animation::JamesBasicClips::FALL,
                                    );
                                }
                                sh2_core::combat::CombatMode::Ready
                                | sh2_core::combat::CombatMode::Raise => {
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
                                sh2_core::combat::CombatMode::Free
                                | sh2_core::combat::CombatMode::Lower => {
                                    state.sim_core.animation_locked = false;
                                }
                                _ => {}
                            }

                            // Play any combat sound effects requested by the combat state machine
                            if let Some(sfx_id) = state.combat_arena.player.pending_sfx.take() {
                                tracing::info!(
                                    "[Combat SFX] Triggered sound effect ID: {}",
                                    sfx_id
                                );
                            }
                        }
                    }

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

                    state.skin_matrices_scratch.clear();
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
                            c.dynamic_locals.copy_from_slice(&c.bind_locals);
                            c.secondary_physics.apply_to_locals(&mut c.dynamic_locals);

                            evaluate_pose_into(
                                &c.model.parents,
                                &c.dynamic_locals,
                                &blended_frame,
                                &mut c.pose_scratch,
                            );
                            compute_skin_matrices_with_inv_into(
                                &c.pose_scratch,
                                &c.model.inv_initial_matrices,
                                &mut c.skin_scratch,
                            );

                            let (s, c_yaw) = player_yaw.sin_cos();
                            let world_mat = [
                                [c_yaw, 0.0, s, player_pos.x],
                                [0.0, 1.0, 0.0, player_pos.y],
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

                    let mut skeletal_draw = Vec::new();
                    if let Some(c) = active_char {
                        if !state.skin_matrices_scratch.is_empty() {
                            skeletal_draw.push((
                                &c.gpu_mesh,
                                state.skin_matrices_scratch.as_slice(),
                                Some(c.texture_bind_groups.as_slice()),
                            ));
                        }
                    }

                    // Render enemy skeletal models when in Combat Arena (only the actively selected enemy is rendered and active)
                    let mut enemy_skin_matrices: Vec<Vec<[[f32; 4]; 4]>> = Vec::new();
                    if state.active_test_room == TestRoomKind::CombatArena
                        && !state.combat_arena.enemies.is_empty()
                    {
                        let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
                        let enemy = &state.combat_arena.enemies[active_idx];
                        let asset = match enemy.id {
                            1 => state.enemy_scu_asset.as_ref(),
                            2 => state.enemy_mkn_asset.as_ref(),
                            3 => state.enemy_nse_asset.as_ref(),
                            4 => state.enemy_red_asset.as_ref(),
                            5 => state.enemy_pap_asset.as_ref(),
                            6 => state.enemy_ike_asset.as_ref(),
                            7 => state.enemy_arm_asset.as_ref(),
                            8 => state.enemy_tyu_asset.as_ref(),
                            9 => state.enemy_bos_asset.as_ref(),
                            _ => None,
                        };

                        if let Some(ea) = asset {
                            let frame_count = ea.anm.frames.len();
                            if frame_count > 0 {
                                // Evaluate synchronized skeletal animation frame from creature FSM
                                let (prev_cursor, cur_cursor, blend) =
                                    enemy.anim.evaluate_blended_cursor();
                                let f0 = (cur_cursor.0 as usize) % frame_count;
                                let f1 = (cur_cursor.1 as usize) % frame_count;
                                let alpha = cur_cursor.2;

                                let cur_frame = interpolate_frames(
                                    &ea.anm.frames[f0],
                                    &ea.anm.frames[f1],
                                    alpha,
                                );

                                let frame = if let Some(prev) = prev_cursor {
                                    if blend < 1.0 {
                                        let pf0 = (prev.0 as usize) % frame_count;
                                        let pf1 = (prev.1 as usize) % frame_count;
                                        let prev_frame = interpolate_frames(
                                            &ea.anm.frames[pf0],
                                            &ea.anm.frames[pf1],
                                            prev.2,
                                        );
                                        interpolate_frames(&prev_frame, &cur_frame, blend)
                                    } else {
                                        cur_frame
                                    }
                                } else {
                                    cur_frame
                                };

                                let mut pose = Vec::with_capacity(ea.model.bone_count);
                                evaluate_pose_into(
                                    &ea.model.parents,
                                    &ea.bind_locals,
                                    &frame,
                                    &mut pose,
                                );

                                let mut skin = Vec::with_capacity(ea.model.bone_count);
                                compute_skin_matrices_with_inv_into(
                                    &pose,
                                    &ea.model.inv_initial_matrices,
                                    &mut skin,
                                );

                                let (es, ec_yaw) = enemy.yaw.sin_cos();
                                // Height adjustments for specific monsters:
                                // - Flesh Lips (6): hung from ceiling frame (elevate Y by -1200 so it hangs down into room)
                                // - Mandarin (7): in retail hangs below floor grates (root Y +880); elevate by -1000 so it crawls visibly above ground
                                // - Mary Boss (9): hovering cage boss (elevate Y by -1000)
                                let y_offset = match enemy.id {
                                    6 => -1200.0,
                                    7 => -1000.0,
                                    9 => -1000.0,
                                    _ => 0.0,
                                };
                                let ey = enemy.position.y + y_offset;
                                let world_mat = [
                                    [ec_yaw, 0.0, es, enemy.position.x],
                                    [0.0, 1.0, 0.0, ey],
                                    [-es, 0.0, ec_yaw, enemy.position.z],
                                    [0.0, 0.0, 0.0, 1.0],
                                ];

                                let mut enemy_mats = Vec::with_capacity(skin.len());
                                for m_skin in &skin {
                                    let mut combined = [[0.0; 4]; 4];
                                    for r in 0..4 {
                                        for col in 0..4 {
                                            combined[r][col] = world_mat[r][0] * m_skin[0][col]
                                                + world_mat[r][1] * m_skin[1][col]
                                                + world_mat[r][2] * m_skin[2][col]
                                                + world_mat[r][3] * m_skin[3][col];
                                        }
                                    }
                                    enemy_mats.push(combined);
                                }
                                enemy_skin_matrices.push(enemy_mats);
                            }
                        }

                        if let Some(ea) = asset {
                            if !enemy_skin_matrices.is_empty() {
                                skeletal_draw.push((
                                    &ea.gpu_mesh,
                                    &enemy_skin_matrices[0],
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
                                let active_idx =
                                    state.combat_enemy_idx % state.combat_arena.enemies.len();
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
                    } else if is_fmv_viewer {
                        let tint = [1.0f32, 1.0, 1.0, 1.0];
                        state.render_ctx.queue.write_buffer(
                            &state.menu_uniform_buffer,
                            0,
                            bytemuck::cast_slice(&tint),
                        );
                        state.menu_rasterizer.clear();

                        let white = [235, 235, 235, 255];
                        let yellow = [245, 210, 65, 255];
                        let cyan = [105, 215, 235, 255];
                        let green = [120, 230, 150, 255];
                        let muted = [165, 165, 170, 255];

                        if let Some(ref session) = state.fmv_session {
                            let cur_frame = session.stream.current_frame_index.saturating_sub(1);
                            let total_frames = session.header.frame_count.max(1);
                            let fps = session.header.fps();
                            let cur_sec = (cur_frame as f64) / fps.max(1.0);
                            let tot_sec = session.header.duration_secs();

                            // Render synchronized subtitle cue near bottom of frame
                            if state.fmv_viewer.subtitles_enabled {
                                let movie_path = session.stream.path.as_deref();
                                if let Some(sub_line) =
                                    sh2_assets::formats::bink::active_subtitle_text(
                                        session.entry.id,
                                        &session.header,
                                        cur_frame,
                                        movie_path,
                                    )
                                {
                                    let game_root = movie_path.and_then(
                                        sh2_assets::formats::bink::game_root_from_movie_path,
                                    );
                                    let w = state.menu_rasterizer.width;
                                    sh2_ui::draw_subtitle(
                                        &mut state.menu_rasterizer,
                                        game_root.as_deref(),
                                        &sub_line,
                                        w / 2,
                                        645,
                                        32,
                                        white,
                                    );
                                }
                            }

                            if state.fmv_viewer.show_hud {
                                let total_movies = FMV_CATALOG.len();
                                let cur_idx = state.fmv_viewer.selected_movie_index + 1;
                                let src_label = if session.stream.path.is_none() {
                                    "SYNTHETIC 1080P60 STREAM"
                                } else {
                                    state.fmv_viewer.video_source.label()
                                };
                                let play_str = if state.fmv_viewer.is_paused {
                                    "PAUSED"
                                } else {
                                    "PLAYING"
                                };

                                state.menu_rasterizer.draw_text(
                                    "SILENT HILL 2 - 1080P60 FMV CUTSCENE THEATER [SH2-10]",
                                    24,
                                    18,
                                    2,
                                    yellow,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    &format!(
                                        "CUTSCENE [{}/{}] : {} ({}) - {}",
                                        cur_idx,
                                        total_movies,
                                        session.entry.display_name,
                                        session.entry.filename,
                                        session.entry.description
                                    ),
                                    24,
                                    46,
                                    2,
                                    white,
                                    true,
                                );
                                let audio_label = state.fmv_viewer.audio_track_label(
                                    session.entry.filename,
                                    session.header.audio_tracks.len().max(1),
                                );
                                let sharpen_label = match state.fmv_viewer.sharpen_level {
                                    0 => "OFF",
                                    1 => "SOFT (0.35)",
                                    2 => "MEDIUM (0.70)",
                                    _ => "STRONG (1.00)",
                                };

                                state.menu_rasterizer.draw_text(
                                    &format!(
                                        "STREAM : {} | {}x{} @ {:.2} FPS | {} | {:05.1}s / {:05.1}s (FRAME {}/{})",
                                        src_label,
                                        session.header.width,
                                        session.header.height,
                                        fps,
                                        play_str,
                                        cur_sec,
                                        tot_sec,
                                        cur_frame,
                                        total_frames
                                    ),
                                    24,
                                    74,
                                    1,
                                    green,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    &format!("AUDIO  : {}", audio_label),
                                    24,
                                    92,
                                    1,
                                    yellow,
                                    true,
                                );
                                let cutscene_map =
                                    sh2_audio::cutscene_bgm::find_cutscene_audio_mapping(
                                        session.entry.id,
                                    );
                                let bgm_desc =
                                    cutscene_map.map(|m| m.description).unwrap_or("None");
                                let bgm_file = cutscene_map.and_then(|m| m.bgm_file);
                                let bgm_label =
                                    state.fmv_viewer.bgm_stream_label(bgm_file, bgm_desc);
                                state.menu_rasterizer.draw_text(
                                    &format!("MUSIC  : {}", bgm_label),
                                    24,
                                    108,
                                    1,
                                    yellow,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    &format!(
                                        "ASPECT : {} | CAS SHARPEN : {} | GRAIN : {} | SUBS : {}",
                                        state.fmv_viewer.aspect_mode.label(),
                                        sharpen_label,
                                        if state.fmv_viewer.film_grain_enabled {
                                            "ON"
                                        } else {
                                            "OFF"
                                        },
                                        if state.fmv_viewer.subtitles_enabled {
                                            "ON"
                                        } else {
                                            "OFF"
                                        }
                                    ),
                                    24,
                                    126,
                                    1,
                                    cyan,
                                    true,
                                );
                                state.menu_rasterizer.draw_text(
                                    "[N/P or Left/Right] Movie  [Space] Pause  [R] Restart  [H] Source (1080p60/OG-HQ/PC)  [T] Audio Track  [U] Sharpen  [V] Aspect  [G] Grain  [S] Subs  [Tab] HUD  [Esc] Options",
                                    24,
                                    688,
                                    1,
                                    muted,
                                    true,
                                );
                            }
                        }

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

                    // If in-game pause menu is active, render authentic pause overlay
                    if is_in_game && state.menu_ctrl.pause_menu.is_open {
                        let tint = [1.0f32, 1.0, 1.0, 1.0];
                        state.render_ctx.queue.write_buffer(
                            &state.menu_uniform_buffer,
                            0,
                            bytemuck::cast_slice(&tint),
                        );
                        state.menu_rasterizer.clear();
                        let font = sh2_ui::AtlasFont::shared(None);

                        if state.menu_ctrl.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu {
                            // Sub-phase: Options Menu
                            sh2_ui::draw_options_screen(
                                &state.menu_ctrl.options_menu,
                                font,
                                &mut state.menu_rasterizer,
                            );
                        } else {
                            // Authentic Pause Menu Screen with 50% dark backdrop & PS2 font
                            sh2_ui::draw_pause_screen(
                                &state.menu_ctrl.pause_menu,
                                font,
                                &mut state.menu_rasterizer,
                            );
                        }

                        // Upload rasterized menu text overlay to GPU and blit over the frame
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
                    } else if is_in_game && state.debug_camera_hud {
                        // Render in-game player coordinates and camera telemetry debug HUD
                        let tint = [1.0f32, 1.0, 1.0, 1.0];
                        state.render_ctx.queue.write_buffer(
                            &state.menu_uniform_buffer,
                            0,
                            bytemuck::cast_slice(&tint),
                        );
                        state.menu_rasterizer.clear();

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
                                p_pos.x, p_pos.y, p_pos.z, p_yaw, p_yaw.to_degrees()
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
                            let active_idx =
                                state.combat_enemy_idx % state.combat_arena.enemies.len();
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

                            let cur_anim_clip = state.sim_core.animation.current_clip();
                            let anim_state_name = format!("{:?}", state.sim_core.animation.state());
                            state.menu_rasterizer.draw_text(
                                &format!(
                                    "[ANIM STATE: {}] CLIP: frames {}..={} @ {:.0}fps | TIME: {:.2}s",
                                    anim_state_name,
                                    cur_anim_clip.start_frame,
                                    cur_anim_clip.end_frame,
                                    cur_anim_clip.fps,
                                    state.sim_core.animation.clip_time()
                                ),
                                20,
                                645,
                                1,
                                cyan,
                                true,
                            );

                            state.menu_rasterizer.draw_text(
                                "[Controls: Right-Click/R: Ready | Space: Attack | 1..6: Weapons | Tab: Cycle Enemy | WASD: Move]",
                                20,
                                670,
                                1,
                                muted,
                                true,
                            );
                        }

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
                "fmv" | "fmv-viewer" | "cutscenes" | "movies" => {
                    Some(TestRoomKind::FmvCutsceneViewer)
                }
                "combat" | "arena" | "battle" => Some(TestRoomKind::CombatArena),
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
