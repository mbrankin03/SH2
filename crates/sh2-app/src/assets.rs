//! Level, collision, camera, character model, texture, and animation asset loading helpers.

mod character;
mod character_textures;
mod cutscene;
mod enemies;

pub(crate) use character::{
    load_anm_file, load_character_asset, load_viewer_entry_asset, CharacterAsset,
};
pub(crate) use cutscene::{
    load_cutscene_voice_wav, open_drama_demo_session, ActiveDramaDemoSession,
};
pub(crate) use enemies::{EnemyAssetCache, EnemyAssetSlot};

use std::path::PathBuf;
use std::sync::Arc;

use sh2_app::synthetic_geometry::{
    create_fallback_room_geometry, create_model_viewer_pedestal_geometry,
    create_surface_sfx_test_room_cld, create_surface_sfx_test_room_geometry,
    create_synthetic_fog_runway_geometry,
};
use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::TextureLoader;
use sh2_core::config::WorldTextureQuality;
use sh2_renderer::RenderContext;
use sh2_ui::menu::TestRoomKind;

/// Locate and load authentic `.cld` collision file for the given test room, falling back to procedural bounds.
pub(crate) fn find_cld_file_for_room(room: TestRoomKind) -> sh2_core::collision::CollisionWorld {
    let resolver = sh2_assets::AssetPathResolver::resolve();

    let cld_relative = match room {
        TestRoomKind::ObservationDeck | TestRoomKind::AudioDebugRoom => Some("bg/ob/ob01.cld"),
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

    if room == TestRoomKind::SurfaceSfxTestRoom {
        let cld = create_surface_sfx_test_room_cld();
        tracing::info!(
            "Created 8-material SurfaceSfxTestRoom collision geometry ({} floor triangles, {} walls)",
            cld.floors.len(),
            cld.walls.len()
        );
        return sh2_core::collision::CollisionWorld::from_cld(&cld);
    }

    if room == TestRoomKind::CombatArena || room == TestRoomKind::HorrorTriggersRoom {
        tracing::info!(
            "Using open unobstructed arena collision bounds for {:?}",
            room
        );
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

/// Returns the retail room camera identifier (`<room>`) for a `TestRoomKind`, if backed by an authentic stage (`SH2-50`).
pub(crate) const fn room_cam_id_for_test_room(room: TestRoomKind) -> Option<&'static str> {
    match room {
        TestRoomKind::ObservationDeck | TestRoomKind::AudioDebugRoom => Some("ob01"),
        TestRoomKind::RestroomStart => Some("er01"),
        TestRoomKind::HotelBoilerRoom => Some("rr21"),
        _ => None,
    }
}

/// Loads a single `.map` tier (`OriginalPs2`, `Sh2eeHd`, or `Ai4kMaster`) from the resolved game
/// roots with graceful fallback if the preferred tier file is unavailable (`SH2-181`).
pub(crate) fn load_map_for_quality_from_roots(
    rel_map_path: &str,
    map_name: &str,
    quality: WorldTextureQuality,
) -> Option<MapFile> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];
    let (prefer_sh2ee, apply_4k) = match quality {
        WorldTextureQuality::Ai4kMaster => (true, true),
        WorldTextureQuality::Sh2eeHd => (true, false),
        WorldTextureQuality::OriginalPs2 => (false, false),
    };
    for base in &base_roots {
        let loader = TextureLoader::new(
            PathBuf::from(base).join("data"),
            Some(PathBuf::from(base).join("sh2e")),
        );
        let loaded = loader
            .load_map_for_enhancement(rel_map_path, map_name, prefer_sh2ee, apply_4k)
            .or_else(|err| match quality {
                WorldTextureQuality::Sh2eeHd => {
                    loader.load_map_with_hd_overrides(rel_map_path, map_name)
                }
                WorldTextureQuality::OriginalPs2 => loader.load_sh2e_map(rel_map_path, map_name),
                WorldTextureQuality::Ai4kMaster => Err(err),
            });
        if let Ok(map) = loaded {
            return Some(map);
        }
    }
    None
}

/// Loads ONLY the active `quality` tier `.map` geometry/textures plus spawn pose and fog uniforms
/// for `room` (`SH2-181`).
pub(crate) fn find_map_file_for_room_for_quality(
    room: TestRoomKind,
    quality: WorldTextureQuality,
) -> (MapFile, glam::Vec3, f32, sh2_renderer::fog::FogUniforms) {
    match room {
        TestRoomKind::ObservationDeck | TestRoomKind::AudioDebugRoom => {
            if let Some(map) = load_map_for_quality_from_roots("bg/ob/ob01.map", "ob01", quality) {
                tracing::info!(
                    "Loaded authentic Observation Deck level geometry (ob01.map, tier={:?})",
                    quality
                );
                // Toluca Lake vista outdoor fog parameters calibrated for full ob01 draw distance
                let eval = sh2_core::fog::evaluate_room_fog(&sh2_core::fog::RoomFogContext {
                    room_id: sh2_core::fog::R_OBSV_DECK_ID,
                    ..Default::default()
                });
                let fog = sh2_renderer::fog::FogUniforms::from_room_evaluation(
                    &eval,
                    sh2_core::config::FogQuality::ClassicPs2,
                    false,
                );
                // Authentic parking lot overlook coordinates: X: 0.0, Y: -100.0, Z: -5000.0
                return (map, glam::Vec3::new(0.0, -100.0, -5000.0), 0.0, fog);
            }
            tracing::warn!("Failed to load ob01.map; using synthetic fog runway fallback");
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.65, 0.68, 0.72, 1.0],
                fog_near: 2000.0,
                fog_far: 25000.0,
                density: 0.80,
                step_count: 32,
                curve_type: 1,
                speed_mult: sh2_core::fog::FOG_SPEED_OUTDOOR_SLOW,
                ..Default::default()
            };
            (
                create_synthetic_fog_runway_geometry(),
                glam::Vec3::new(0.0, -100.0, 0.0),
                0.0,
                fog,
            )
        }
        TestRoomKind::RestroomStart => {
            if let Some(map) = load_map_for_quality_from_roots("bg/er/er01.map", "er01", quality) {
                tracing::info!(
                    "Loaded authentic Opening Restroom level geometry (er01.map, tier={:?})",
                    quality
                );
                let mut fog = sh2_renderer::fog::FogUniforms {
                    fog_color: [0.12, 0.13, 0.14, 1.0], // Dim authentic restroom lighting
                    fog_near: 800.0,
                    fog_far: 5000.0,
                    density: 0.30,
                    step_count: 24,
                    curve_type: 0,
                    speed_mult: 1.0,
                    ..Default::default()
                };
                fog.layer2_params[3] = 0.0;
                // Authentic PS2 stage 2 (er01) post-prologue mirror spawn coordinates:
                // James stands at the sink: X: -19606.6, Y: 0.0 (floor), Z: 20429.55, Facing: -PI/2 (-1.5707964 rad, West towards mirror)
                return (
                    map,
                    glam::Vec3::new(-19606.6, 0.0, 20429.55),
                    -std::f32::consts::FRAC_PI_2,
                    fog,
                );
            }
            tracing::warn!("Failed to load er01.map; using fallback room geometry");
            let mut fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.12, 0.13, 0.14, 1.0],
                fog_near: 800.0,
                fog_far: 5000.0,
                density: 0.30,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                ..Default::default()
            };
            fog.layer2_params[3] = 0.0;
            (
                create_fallback_room_geometry(),
                glam::Vec3::new(0.0, -800.0, 2000.0),
                std::f32::consts::PI,
                fog,
            )
        }
        TestRoomKind::HotelBoilerRoom => {
            if let Some(map) = load_map_for_quality_from_roots("bg/rr/rr21.map", "rr21", quality) {
                tracing::info!(
                    "Loaded authentic Lakeview Hotel Boiler Room level geometry (rr21.map, tier={:?})",
                    quality
                );
                let mut fog = sh2_renderer::fog::FogUniforms {
                    fog_color: [0.08, 0.09, 0.11, 1.0], // Dark basement boiler tone
                    fog_near: 800.0,
                    fog_far: 6000.0,
                    density: 0.35,
                    step_count: 24,
                    curve_type: 0,
                    speed_mult: 1.0,
                    ..Default::default()
                };
                fog.layer2_params[3] = 0.0;
                // Authentic rr21 hotel boiler room entrance coordinates (open doorway corridor, outside boiler collision hulls):
                // Floor Y = 0.0, X = -100850.0, Z = -59400.0, facing North-East into the room (yaw = 2.0 rad)
                return (map, glam::Vec3::new(-100850.0, 0.0, -59400.0), 2.0, fog);
            }
            tracing::warn!("Failed to load rr21.map; using fallback room geometry");
            let mut fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.08, 0.09, 0.11, 1.0],
                fog_near: 800.0,
                fog_far: 6000.0,
                density: 0.35,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                ..Default::default()
            };
            fog.layer2_params[3] = 0.0;
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
                step_count: 32,
                curve_type: 1, // Exponential
                speed_mult: sh2_core::fog::FOG_SPEED_OUTDOOR_SLOW,
                ..Default::default()
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
            let mut fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.08, 0.09, 0.11, 1.0],
                fog_near: 800.0,
                fog_far: 6000.0,
                density: 0.35,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                ..Default::default()
            };
            fog.layer2_params[3] = 0.0;
            (
                create_fallback_room_geometry(),
                glam::Vec3::new(0.0, -800.0, 2000.0),
                std::f32::consts::PI,
                fog,
            )
        }
        TestRoomKind::ModelAndTextureViewer
        | TestRoomKind::FmvCutsceneViewer
        | TestRoomKind::LoadingScreenTest
        | TestRoomKind::GrappleAnimationRoom
        | TestRoomKind::InventoryAndItemsRoom
        | TestRoomKind::PuzzleAndCutawayRoom
        | TestRoomKind::StoryStagesAndEndingsRoom => {
            let mut fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.06, 0.07, 0.08, 1.0],
                fog_near: 5000.0,
                fog_far: 30000.0,
                density: 0.05, // Clean neutral inspection visibility
                step_count: 16,
                curve_type: 0,
                speed_mult: 0.0,
                ..Default::default()
            };
            fog.layer2_params[3] = 0.0;
            (
                create_model_viewer_pedestal_geometry(),
                glam::Vec3::new(0.0, 0.0, 0.0),
                0.0,
                fog,
            )
        }
        TestRoomKind::CombatArena | TestRoomKind::HorrorTriggersRoom => {
            tracing::info!(
                "Generated Combat / Horror Testing Arena Stage ({:?}).",
                room
            );
            let mut fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.12, 0.13, 0.16, 1.0],
                fog_near: 2500.0,
                fog_far: 20000.0,
                density: 0.12,
                step_count: 24,
                curve_type: 0,
                speed_mult: 1.0,
                ..Default::default()
            };
            fog.layer2_params[3] = 0.0;
            (
                create_fallback_room_geometry(),
                // Spawn James at Z = -1500.0, facing North (0.0 rad) directly looking toward the enemies (Z = 1200..2000)
                glam::Vec3::new(0.0, 0.0, -1500.0),
                0.0,
                fog,
            )
        }
        TestRoomKind::SurfaceSfxTestRoom => {
            tracing::info!(
                "Generated 8-Material Surface Footstep & 3D SFX Attenuation Test Room (SH2-61)."
            );
            let mut fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.09, 0.10, 0.13, 1.0],
                fog_near: 4500.0,
                fog_far: 26000.0,
                density: 0.08,
                step_count: 20,
                curve_type: 0,
                speed_mult: 0.5,
                ..Default::default()
            };
            fog.layer2_params[3] = 0.0;
            (
                create_surface_sfx_test_room_geometry(),
                // Spawn James on Pad 0 (Concrete, X = -4300, Z = -3200) facing East (+X) across the 8 material pads
                glam::Vec3::new(-4300.0, 0.0, -3200.0),
                std::f32::consts::FRAC_PI_2,
                fog,
            )
        }
    }
}

/// Paired GPU geometry and per-material diffuse texture bind groups compiled from the same
/// [`MapFile`] so `GpuMapBatch::material_index` can never index into a mismatched material table
/// when switching between `Original PS2 / PC`, `SH2:EE`, and `4K AI Master` (`SH2-121`).
#[derive(Clone)]
pub(crate) struct MapTierAssets {
    pub gpu_map: Arc<sh2_renderer::GpuMap>,
    pub texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
}

impl MapTierAssets {
    /// Compiles both the [`sh2_renderer::GpuMap`] vertex/index buffers and matching per-material
    /// diffuse texture bind groups from an owned `map_file` in lockstep, consuming `map_file.textures`
    /// without cloning 16 MB 4K surfaces (`SH2-181`).
    pub fn from_map(render_ctx: &RenderContext, mut map_file: MapFile) -> Self {
        let gpu_map = Arc::new(sh2_renderer::GpuMap::from_map(
            &render_ctx.device,
            &map_file,
        ));
        let texture_bind_groups = build_map_texture_bind_groups(render_ctx, &mut map_file);
        debug_assert!(
            gpu_map.all_batches_within_material_table(texture_bind_groups.len()),
            "GpuMap '{}' has batch material_index >= texture_bind_groups.len() ({})",
            map_file.name,
            texture_bind_groups.len()
        );
        Self {
            gpu_map,
            texture_bind_groups,
        }
    }
}

/// Builds GPU diffuse texture bind groups for every material in a [`MapFile`], caching by `texture_id`
/// to avoid redundant VRAM allocations, consuming `map_file.textures` via zero-copy `into_rgba()`,
/// and parallelizing CPU mipmap generation / BC1/BC3 compression with bounded worker concurrency (`SH2-181`).
pub(crate) fn build_map_texture_bind_groups(
    render_ctx: &RenderContext,
    map_file: &mut MapFile,
) -> Vec<Arc<wgpu::BindGroup>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    // Drain unique textures referenced by materials so each texture is owned by a single worker
    // and dropped immediately after CPU mip/BC preparation.
    let mut work_items: Vec<(usize, u32, sh2_assets::formats::map::MapTexture)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (m_idx, mat) in map_file.materials.iter().enumerate() {
        let tex_id = mat.texture_id as u32;
        if seen.insert(tex_id) {
            if let Some(tex) = map_file.textures.remove(&tex_id) {
                work_items.push((m_idx, tex_id, tex));
            }
        }
    }

    let supports_bc = render_ctx.supports_bc_compression();
    const MAX_MAP_TEX_PREP_WORKERS: usize = 4;
    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_MAP_TEX_PREP_WORKERS)
        .min(work_items.len().max(1));
    let work_slots: Vec<Mutex<Option<(usize, u32, sh2_assets::formats::map::MapTexture)>>> =
        work_items
            .into_iter()
            .map(|it| Mutex::new(Some(it)))
            .collect();
    let next_idx = AtomicUsize::new(0);
    let prepared_map: Mutex<Vec<(usize, u32, sh2_renderer::PreparedGpuTexture)>> =
        Mutex::new(Vec::with_capacity(work_slots.len()));

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                if idx >= work_slots.len() {
                    break;
                }
                let item = work_slots[idx].lock().ok().and_then(|mut g| g.take());
                if let Some((m_idx, tex_id, tex)) = item {
                    let (w, h) = (tex.width as u32, tex.height as u32);
                    if let Ok(rgba) = tex.into_rgba() {
                        let prepared =
                            sh2_renderer::prepare_map_texture_cpu_owned(w, h, rgba, supports_bc);
                        if let Ok(mut guard) = prepared_map.lock() {
                            guard.push((m_idx, tex_id, prepared));
                        }
                    }
                }
            });
        }
    });

    let mut tex_bg_cache: std::collections::HashMap<u32, Arc<wgpu::BindGroup>> =
        std::collections::HashMap::with_capacity(work_slots.len());
    if let Ok(entries) = prepared_map.into_inner() {
        for (m_idx, tex_id, prepared) in entries {
            let (wrap_u, wrap_v) = map_file.texture_uv_wrap_axes(tex_id);
            let bg = render_ctx.upload_prepared_texture_bind_group_with_wrap(
                &prepared,
                wrap_u,
                wrap_v,
                Some(&format!("Map Material {m_idx} Texture {tex_id}")),
            );
            tex_bg_cache.insert(tex_id, bg);
        }
    }

    let mut map_texture_bind_groups = Vec::with_capacity(map_file.materials.len());
    for mat in &map_file.materials {
        let tex_id = mat.texture_id as u32;
        if let Some(bg) = tex_bg_cache.get(&tex_id) {
            map_texture_bind_groups.push(Arc::clone(bg));
        } else {
            map_texture_bind_groups.push(Arc::clone(&render_ctx.default_texture_bind_group));
        }
    }

    map_texture_bind_groups
}
