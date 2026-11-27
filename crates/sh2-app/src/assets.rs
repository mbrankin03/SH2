//! Level, collision, camera, character model, texture, and animation asset loading helpers.

mod cutscene;
mod enemies;

pub(crate) use cutscene::{
    load_cutscene_voice_wav, open_drama_demo_session, ActiveDramaDemoSession,
};
pub(crate) use enemies::{EnemyAssetCache, EnemyAssetSlot};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sh2_app::synthetic_geometry::{
    create_fallback_room_geometry, create_model_viewer_pedestal_geometry,
    create_surface_sfx_test_room_cld, create_surface_sfx_test_room_geometry,
    create_synthetic_fog_runway_geometry,
};
use sh2_assets::formats::anm::{compute_bind_locals, parse_anm, AnimationFile, AnimationFrame};
use sh2_assets::formats::cls::{parse_cls, ClsFile};
use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::mdls::{companion_texture_mdl_path, parse_mdl, ModelFile, ModelTexture};
use sh2_assets::formats::TextureLoader;
use sh2_core::config::WorldTextureQuality;
use sh2_core::SecondaryBonePhysics;
use sh2_debug::ModelViewerEntry;
use sh2_renderer::{GpuSkeletalMesh, RenderContext, SkeletalVertex};
use sh2_ui::menu::TestRoomKind;

/// Source metadata allowing [`CharacterAsset`] to reload ONLY its GPU texture bind groups on
/// demand when `WorldTextureQuality` changes, without keeping raw CPU `model.textures` or
/// inactive texture tiers resident in RAM/VRAM (`SH2-181`).
#[derive(Debug, Clone)]
pub(crate) struct CharacterTextureSource {
    pub base_root: PathBuf,
    pub mdl_rel_path: String,
    pub unique_tex_ids: Vec<u16>,
}

/// Loaded character asset representation (model hierarchy, bind locals, animation tracks, and facial clusters).
pub(crate) struct CharacterAsset {
    pub name: &'static str,
    pub gpu_mesh: GpuSkeletalMesh,
    pub model: ModelFile,
    /// Number of embedded textures originally present in `model.textures` before CPU drop (`SH2-181`).
    pub texture_count: usize,
    pub bind_locals: Vec<[[f32; 4]; 4]>,
    pub pose_scratch: Vec<[[f32; 4]; 4]>,
    pub skin_scratch: Vec<[[f32; 4]; 4]>,
    pub frame_scratch_a: AnimationFrame,
    pub frame_scratch_b: AnimationFrame,
    pub anm: AnimationFile,
    /// Optional companion `.cls` facial morph cluster animation file (`SH2-63`).
    pub cls: Option<ClsFile>,
    /// Reusable scratch buffer for evaluated `.cls` facial cluster weights (`0..n_clusters`).
    pub cluster_weights_scratch: Vec<f32>,
    /// Reusable scratch buffer for morphed model-bind-space vertex positions.
    pub morphed_positions_scratch: Vec<[f32; 3]>,
    /// Reusable scratch buffer for GPU `SkeletalVertex` upload.
    pub gpu_vertex_scratch: Vec<SkeletalVertex>,
    /// Single-tier active GPU diffuse texture bind groups matching `loaded_texture_quality` (`SH2-181`).
    pub texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    /// Active [`WorldTextureQuality`] tier currently resident in `texture_bind_groups` (`SH2-181`).
    pub loaded_texture_quality: WorldTextureQuality,
    /// Disk source metadata used to reload `texture_bind_groups` on demand when quality changes.
    pub texture_source: Option<CharacterTextureSource>,
    pub secondary_physics: SecondaryBonePhysics,
    pub head_look: sh2_core::HeadLookTracker,
    pub foot_ik: sh2_core::FootIkSolver,
}

impl CharacterAsset {
    /// Reloads ONLY the active GPU diffuse texture bind groups for `quality` when it differs from
    /// [`Self::loaded_texture_quality`], avoiding keeping all 3 texture tiers resident in VRAM (`SH2-181`).
    pub fn reload_textures_for_quality(
        &mut self,
        ctx: &RenderContext,
        quality: WorldTextureQuality,
    ) {
        if self.loaded_texture_quality == quality {
            return;
        }
        let Some(ref src) = self.texture_source else {
            self.loaded_texture_quality = quality;
            return;
        };
        let van_textures = load_vanilla_model_textures(&src.base_root, &src.mdl_rel_path);
        self.texture_bind_groups = build_character_texture_bind_groups_for_quality(
            ctx,
            self.name,
            &src.base_root,
            &src.mdl_rel_path,
            &src.unique_tex_ids,
            &van_textures,
            quality,
        );
        self.loaded_texture_quality = quality;
    }
}

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

/// Generates a high-resolution procedural HD inspection variation from an RGBA8 texture
/// when an external SH2:EE `.dds`/`.png` replacement texture is not found on disk,
/// allowing side-by-side filtering & detail comparison in the Model Viewer.
pub(crate) fn upscale_hd_preview_rgba(
    rgba: &[u8],
    width: usize,
    height: usize,
) -> (u32, u32, Vec<u8>) {
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

/// Reloads embedded vanilla textures from disk (plus companion `.mdl` textures for `no_tex = 1`
/// variants) for on-demand character texture tier switching (`SH2-181`).
fn load_vanilla_model_textures(
    base_root: &std::path::Path,
    mdl_rel: &str,
) -> HashMap<u32, ModelTexture> {
    let van_rel = mdl_rel
        .strip_prefix("sh2e/")
        .map(|s| format!("data/{s}"))
        .unwrap_or_else(|| mdl_rel.to_string());
    let primary_path = base_root.join(&van_rel);
    let bytes = std::fs::read(&primary_path)
        .or_else(|_| std::fs::read(base_root.join(mdl_rel)))
        .ok();
    let Some(mut model) = bytes.and_then(|b| parse_mdl(&b).ok()) else {
        return HashMap::new();
    };
    if model.textures.is_empty() {
        if let Some(comp_rel) = companion_texture_mdl_path(mdl_rel) {
            if let Ok(comp_bytes) = std::fs::read(base_root.join(comp_rel)) {
                if let Ok(comp_mdl) = parse_mdl(&comp_bytes) {
                    model.textures = comp_mdl.textures;
                }
            }
        }
    }
    model.textures
}

/// Loads optional `sh2e/` override textures for `mdl_rel` (or its companion `.mdl`), returning
/// `None` when `quality == WorldTextureQuality::OriginalPs2` or no `sh2e/` override exists.
fn load_sh2e_model_textures(
    base_root: &std::path::Path,
    mdl_rel: &str,
) -> Option<HashMap<u32, ModelTexture>> {
    let sh2e_cand = if let Some(stripped) = mdl_rel.strip_prefix("data/") {
        Some(base_root.join("sh2e").join(stripped))
    } else if mdl_rel.starts_with("sh2e/") {
        Some(base_root.join(mdl_rel))
    } else {
        None
    };

    if let Some(p) = sh2e_cand {
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(mdl) = parse_mdl(&bytes) {
                if !mdl.textures.is_empty() {
                    return Some(mdl.textures);
                }
            }
        }
    }

    if let Some(comp_rel) = companion_texture_mdl_path(mdl_rel) {
        if let Some(stripped) = comp_rel.strip_prefix("data/") {
            let sh2e_comp = base_root.join("sh2e").join(stripped);
            if let Ok(bytes) = std::fs::read(&sh2e_comp) {
                if let Ok(mdl) = parse_mdl(&bytes) {
                    if !mdl.textures.is_empty() {
                        return Some(mdl.textures);
                    }
                }
            }
        }
    }

    None
}

/// Builds ONLY the single active [`WorldTextureQuality`] tier of GPU diffuse texture bind groups
/// for a character/enemy model (`SH2-181`).
fn build_character_texture_bind_groups_for_quality(
    ctx: &RenderContext,
    character_name: &'static str,
    base_root: &std::path::Path,
    mdl_rel: &str,
    unique_tex_ids: &[u16],
    van_textures: &HashMap<u32, ModelTexture>,
    quality: WorldTextureQuality,
) -> Vec<Arc<wgpu::BindGroup>> {
    let sh2e_textures = if quality != WorldTextureQuality::OriginalPs2 {
        load_sh2e_model_textures(base_root, mdl_rel)
    } else {
        None
    };
    let tex_loader = if quality == WorldTextureQuality::Ai4kMaster {
        Some(TextureLoader::new(
            base_root.join("data"),
            Some(base_root.join("sh2e")),
        ))
    } else {
        None
    };

    let mut bind_groups = Vec::with_capacity(unique_tex_ids.len());

    for &raw_tid in unique_tex_ids {
        let tidx = raw_tid as u32;
        if let Some(tex) = van_textures.get(&tidx) {
            if let Ok(mut rgba) = tex.to_rgba() {
                // Non-cutout character textures (such as Pyramid Head's helmet `1303`/`1307`, Great Knife `1305`,
                // Lying Figure `1282`, and Abstract Daddy `1293`) store PS2 specular/gloss intensity in the DXT5
                // alpha channel with `van_min_alpha > 0` across 100% of texels. Preserve that non-zero alpha floor
                // on any HD / 4K override so bicubic ringing or compression artifacts never punch cutout holes.
                let van_min_alpha = rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|px| px[3])
                    .min()
                    .unwrap_or(255);

                match quality {
                    WorldTextureQuality::OriginalPs2 => {
                        sh2_renderer::dilate_transparent_border_rgb(
                            tex.width as u32,
                            tex.height as u32,
                            &mut rgba,
                            2,
                        );
                        bind_groups.push(ctx.create_texture_bind_group(
                            tex.width as u32,
                            tex.height as u32,
                            &rgba,
                            Some(&format!("{character_name} Vanilla Texture {tidx}")),
                        ));
                    }
                    WorldTextureQuality::Sh2eeHd => {
                        let bg = if let Some(hd_tex) =
                            sh2e_textures.as_ref().and_then(|m| m.get(&tidx))
                        {
                            if let Ok(mut hd_rgba) = hd_tex.to_rgba() {
                                if van_min_alpha > 0 {
                                    for px in hd_rgba.as_chunks_mut::<4>().0 {
                                        px[3] = px[3].max(van_min_alpha);
                                    }
                                }
                                let sh2e_passes = sh2_renderer::scaled_border_dilation_passes(
                                    tex.width as u32,
                                    tex.height as u32,
                                    hd_tex.width as u32,
                                    hd_tex.height as u32,
                                    2,
                                );
                                sh2_renderer::dilate_transparent_border_rgb(
                                    hd_tex.width as u32,
                                    hd_tex.height as u32,
                                    &mut hd_rgba,
                                    sh2e_passes,
                                );
                                ctx.create_texture_bind_group(
                                    hd_tex.width as u32,
                                    hd_tex.height as u32,
                                    &hd_rgba,
                                    Some(&format!("{character_name} SH2:EE HD Texture {tidx}")),
                                )
                            } else {
                                sh2_renderer::dilate_transparent_border_rgb(
                                    tex.width as u32,
                                    tex.height as u32,
                                    &mut rgba,
                                    2,
                                );
                                ctx.create_texture_bind_group(
                                    tex.width as u32,
                                    tex.height as u32,
                                    &rgba,
                                    Some(&format!("{character_name} Vanilla Texture {tidx}")),
                                )
                            }
                        } else {
                            sh2_renderer::dilate_transparent_border_rgb(
                                tex.width as u32,
                                tex.height as u32,
                                &mut rgba,
                                2,
                            );
                            let (hw, hh, mut hd_rgba) =
                                upscale_hd_preview_rgba(&rgba, tex.width, tex.height);
                            sh2_renderer::dilate_transparent_border_rgb(hw, hh, &mut hd_rgba, 2);
                            ctx.create_texture_bind_group(
                                hw,
                                hh,
                                &hd_rgba,
                                Some(&format!("{character_name} Enhanced Texture {tidx}")),
                            )
                        };
                        bind_groups.push(bg);
                    }
                    WorldTextureQuality::Ai4kMaster => {
                        let sh2e_raw: Option<(u32, u32, Vec<u8>)> = sh2e_textures
                            .as_ref()
                            .and_then(|m| m.get(&tidx))
                            .and_then(|hd_tex| {
                                hd_tex.to_rgba().ok().map(|mut hd_rgba| {
                                    if van_min_alpha > 0 {
                                        for px in hd_rgba.as_chunks_mut::<4>().0 {
                                            px[3] = px[3].max(van_min_alpha);
                                        }
                                    }
                                    (hd_tex.width as u32, hd_tex.height as u32, hd_rgba)
                                })
                            });

                        if let Some(mut dec) = tex_loader
                            .as_ref()
                            .and_then(|tl| tl.load_4k_chr_texture_override(tidx))
                        {
                            if van_min_alpha > 0 {
                                for px in dec.rgba.as_chunks_mut::<4>().0 {
                                    px[3] = px[3].max(van_min_alpha);
                                }
                            }
                            if van_min_alpha == 0 || sh2e_raw.is_some() {
                                sh2_renderer::refine_4k_chr_texture_from_sources(
                                    tex.width as u32,
                                    tex.height as u32,
                                    &rgba,
                                    sh2e_raw.as_ref().map(|(w, h, b)| (*w, *h, b.as_slice())),
                                    dec.width as u32,
                                    dec.height as u32,
                                    &mut dec.rgba,
                                );
                            }
                            let hd_passes = sh2_renderer::scaled_border_dilation_passes(
                                tex.width as u32,
                                tex.height as u32,
                                dec.width as u32,
                                dec.height as u32,
                                2,
                            );
                            sh2_renderer::dilate_transparent_border_rgb(
                                dec.width as u32,
                                dec.height as u32,
                                &mut dec.rgba,
                                hd_passes,
                            );
                            tracing::info!(
                                "Loaded 4K AI Master character texture override for '{}' tex_id={} ({}x{})",
                                character_name,
                                tidx,
                                dec.width,
                                dec.height
                            );
                            bind_groups.push(ctx.create_texture_bind_group(
                                dec.width as u32,
                                dec.height as u32,
                                &dec.rgba,
                                Some(&format!("{character_name} 4K AI Texture {tidx}")),
                            ));
                        } else if let Some((sw, sh, mut hd_rgba)) = sh2e_raw {
                            let sh2e_passes = sh2_renderer::scaled_border_dilation_passes(
                                tex.width as u32,
                                tex.height as u32,
                                sw,
                                sh,
                                2,
                            );
                            sh2_renderer::dilate_transparent_border_rgb(
                                sw,
                                sh,
                                &mut hd_rgba,
                                sh2e_passes,
                            );
                            bind_groups.push(ctx.create_texture_bind_group(
                                sw,
                                sh,
                                &hd_rgba,
                                Some(&format!("{character_name} SH2:EE HD Texture {tidx}")),
                            ));
                        } else {
                            sh2_renderer::dilate_transparent_border_rgb(
                                tex.width as u32,
                                tex.height as u32,
                                &mut rgba,
                                2,
                            );
                            let (hw, hh, mut hd_rgba) =
                                upscale_hd_preview_rgba(&rgba, tex.width, tex.height);
                            sh2_renderer::dilate_transparent_border_rgb(hw, hh, &mut hd_rgba, 2);
                            bind_groups.push(ctx.create_texture_bind_group(
                                hw,
                                hh,
                                &hd_rgba,
                                Some(&format!("{character_name} Enhanced Texture {tidx}")),
                            ));
                        }
                    }
                }
                continue;
            }
        }
        bind_groups.push(Arc::clone(&ctx.default_texture_bind_group));
    }

    bind_groups
}

/// Locate authentic character, enemy, boss, or weapon model and animation on disk for the single
/// active [`WorldTextureQuality`] tier (`SH2-181`).
///
/// Supports static/rigid weapon and item models by synthesizing an identity bind-pose animation track
/// if no `.anm` file is present or compatible with the model's bone count. Drops `model.textures`
/// from CPU RAM immediately after GPU upload.
pub(crate) fn load_character_asset(
    ctx: &RenderContext,
    character_name: &'static str,
    mdl_rel_paths: &[&str],
    anm_rel_paths: &[&str],
    quality: WorldTextureQuality,
) -> Option<CharacterAsset> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    let mut mdl_bytes = None;
    let mut loaded_mdl_rel = None;
    let mut loaded_base_root = None;
    let mut anm_bytes = None;
    let mut cls_bytes = None;

    for base in &base_roots {
        for &mdl_sub in mdl_rel_paths {
            let p = PathBuf::from(base).join(mdl_sub);
            if let Ok(bytes) = std::fs::read(&p) {
                tracing::info!("Found model asset: {:?}", p);
                mdl_bytes = Some(bytes);
                loaded_mdl_rel = Some(mdl_sub);
                loaded_base_root = Some(PathBuf::from(base));

                // If caller passed an `sh2e/` path, prefer the vanilla `data/` `.mdl` as base
                // geometry/texture source so `OriginalPs2` and alpha-floor checks always use retail data.
                if let Some(stripped) = mdl_sub.strip_prefix("sh2e/") {
                    let data_cand = PathBuf::from(base).join("data").join(stripped);
                    if let Ok(van_b) = std::fs::read(&data_cand) {
                        mdl_bytes = Some(van_b);
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
                    if let Some(cls_rel) = anm_sub.strip_suffix(".anm").map(|s| format!("{s}.cls"))
                    {
                        let cls_p = PathBuf::from(base).join(&cls_rel);
                        if let Ok(c_bytes) = std::fs::read(&cls_p) {
                            if !c_bytes.is_empty() {
                                tracing::info!("Found companion facial cluster asset: {:?}", cls_p);
                                cls_bytes = Some(c_bytes);
                            }
                        }
                    }
                    break;
                }
            }
        }
        if anm_bytes.is_some() {
            break;
        }
    }

    let mdl_bytes = mdl_bytes?;
    let mut model = parse_mdl(&mdl_bytes).ok()?;

    // If this is a `no_tex = 1` reflection or untextured variant (e.g. `ragl.mdl`, `rhhh_edi.mdl`,
    // `rlau.mdl`, `rmry.mdl`), bind the embedded textures from its primary companion `.mdl`.
    if model.textures.is_empty() {
        if let (Some(mdl_rel), Some(base)) = (loaded_mdl_rel, loaded_base_root.as_ref()) {
            if let Some(comp_rel) = companion_texture_mdl_path(mdl_rel) {
                let comp_path = base.join(comp_rel);
                if let Ok(comp_bytes) = std::fs::read(&comp_path) {
                    if let Ok(comp_mdl) = parse_mdl(&comp_bytes) {
                        tracing::info!(
                            "Loaded {} externalized companion textures from '{}' for '{}'",
                            comp_mdl.textures.len(),
                            comp_rel,
                            mdl_rel
                        );
                        model.textures = comp_mdl.textures;
                    }
                }
            }
        }
    }

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
            keyframe_spans: vec![(0, 0)],
        });

    let cls = cls_bytes
        .as_deref()
        .and_then(|bytes| parse_cls(bytes).ok())
        .filter(|c| !c.blocks.is_empty());

    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let mut gpu_mesh = ctx.compile_skeletal_mesh(&model);

    // Compact unique sprite IDs (`model.primitives[*].texture_index`, e.g. 1239..1772)
    // into a dense 0..N slot array so we only allocate GPU bind groups for textures actually
    // used by this model instead of 1,700+ dummy bind groups.
    let mut unique_tex_ids: Vec<u16> = model.primitives.iter().map(|pr| pr.texture_index).collect();
    unique_tex_ids.sort_unstable();
    unique_tex_ids.dedup();

    let texture_bind_groups = match (loaded_base_root.as_deref(), loaded_mdl_rel) {
        (Some(base), Some(mdl_rel)) => build_character_texture_bind_groups_for_quality(
            ctx,
            character_name,
            base,
            mdl_rel,
            &unique_tex_ids,
            &model.textures,
            quality,
        ),
        _ => vec![Arc::clone(&ctx.default_texture_bind_group); unique_tex_ids.len()],
    };

    // Remap each GPU primitive's `texture_index` to its dense 0..N slot in `texture_bind_groups`
    for prim in &mut gpu_mesh.primitives {
        if let Ok(dense_slot) = unique_tex_ids.binary_search(&prim.texture_index) {
            prim.texture_index = dense_slot as u16;
        }
    }

    let texture_count = model.textures.len();
    // Release CPU texture payload buffers once uploaded to GPU (`SH2-181`).
    model.textures.clear();
    model.textures.shrink_to_fit();

    tracing::info!(
        "Loaded model '{}' (tier={:?}): bones={}, primitives={}, vertices={}, textures={}, anm_frames={}",
        character_name,
        quality,
        model.bone_count,
        model.primitives.len(),
        model.vertices.positions.len(),
        texture_count,
        anm.frames.len()
    );

    let texture_source = loaded_base_root
        .zip(loaded_mdl_rel)
        .map(|(base_root, mdl_rel)| CharacterTextureSource {
            base_root,
            mdl_rel_path: mdl_rel.to_string(),
            unique_tex_ids,
        });

    let (secondary_physics, foot_ik) = if character_name.contains("Maria") {
        (
            SecondaryBonePhysics::for_maria(),
            sh2_core::FootIkSolver::for_maria(),
        )
    } else {
        (
            SecondaryBonePhysics::for_james(),
            sh2_core::FootIkSolver::for_james(),
        )
    };

    let pose_scratch = Vec::with_capacity(model.bone_count);
    let skin_scratch = Vec::with_capacity(model.bone_count);
    let frame_scratch_a = AnimationFrame {
        bones: Vec::with_capacity(model.bone_count),
    };
    let frame_scratch_b = AnimationFrame {
        bones: Vec::with_capacity(model.bone_count),
    };
    let cluster_weights_scratch = Vec::with_capacity(model.clusters.len());
    let morphed_positions_scratch = model.vertices.positions.clone();
    let gpu_vertex_scratch = Vec::with_capacity(model.vertices.positions.len());

    Some(CharacterAsset {
        name: character_name,
        gpu_mesh,
        model,
        texture_count,
        bind_locals,
        pose_scratch,
        skin_scratch,
        frame_scratch_a,
        frame_scratch_b,
        anm,
        cls,
        cluster_weights_scratch,
        morphed_positions_scratch,
        gpu_vertex_scratch,
        texture_bind_groups,
        loaded_texture_quality: quality,
        texture_source,
        secondary_physics,
        head_look: sh2_core::HeadLookTracker::default(),
        foot_ik,
    })
}

/// Loads a standalone `.anm` animation file compatible with a model skeleton.
pub(crate) fn load_anm_file(
    rel_path: &str,
    bone_count: usize,
    parents: &[u8],
) -> Option<sh2_assets::formats::anm::AnimationFile> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];
    for base in &base_roots {
        let p = PathBuf::from(base).join(rel_path);
        if let Ok(bytes) = std::fs::read(&p) {
            if !bytes.is_empty() {
                match parse_anm(&bytes, bone_count, parents) {
                    Ok(anm) => {
                        tracing::info!(
                            "Loaded stage animation {}: {} frames",
                            rel_path,
                            anm.frames.len()
                        );
                        return Some(anm);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse stage animation {}: {:?}", rel_path, e);
                    }
                }
            }
        }
    }
    None
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

/// Loads a specific [`ModelViewerEntry`] and its selected `.anm` track index from disk at `quality`.
pub(crate) fn load_viewer_entry_asset(
    ctx: &RenderContext,
    entry: &'static ModelViewerEntry,
    anm_index: usize,
    quality: WorldTextureQuality,
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
    load_character_asset(
        ctx,
        entry.display_name,
        entry.mdl_paths,
        &anm_slice,
        quality,
    )
}
