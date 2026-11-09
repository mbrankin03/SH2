//! Level, collision, camera, character model, texture, and animation asset loading helpers.

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
use sh2_assets::formats::mdls::{companion_texture_mdl_path, parse_mdl, ModelFile};
use sh2_assets::formats::TextureLoader;
use sh2_core::SecondaryBonePhysics;
use sh2_debug::ModelViewerEntry;
use sh2_renderer::{GpuSkeletalMesh, RenderContext, SkeletalVertex};
use sh2_ui::menu::TestRoomKind;

/// Loaded character asset representation (model hierarchy, bind locals, animation tracks, and facial clusters).
pub(crate) struct CharacterAsset {
    #[allow(dead_code)]
    pub name: &'static str,
    pub gpu_mesh: GpuSkeletalMesh,
    pub model: ModelFile,
    pub bind_locals: Vec<[[f32; 4]; 4]>,
    pub dynamic_locals: Vec<[[f32; 4]; 4]>,
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
    pub vanilla_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub hd_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub secondary_physics: SecondaryBonePhysics,
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
pub(crate) fn find_cam_file_for_room(
    room: TestRoomKind,
) -> Option<sh2_assets::formats::cam::CamFile> {
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

pub(crate) fn find_map_file_for_room(
    room: TestRoomKind,
) -> (MapFile, glam::Vec3, f32, sh2_renderer::fog::FogUniforms) {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    match room {
        TestRoomKind::ObservationDeck | TestRoomKind::AudioDebugRoom => {
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
                    // Toluca Lake vista outdoor fog parameters calibrated for full ob01 draw distance
                    let fog = sh2_renderer::fog::FogUniforms {
                        fog_color: [0.65, 0.68, 0.72, 1.0], // Daylight Toluca Lake mist
                        fog_near: 2000.0,
                        fog_far: 32000.0,
                        density: 0.90,
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
        | TestRoomKind::LoadingScreenTest
        | TestRoomKind::GrappleAnimationRoom => {
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
        TestRoomKind::SurfaceSfxTestRoom => {
            tracing::info!(
                "Generated 8-Material Surface Footstep & 3D SFX Attenuation Test Room (SH2-61)."
            );
            let fog = sh2_renderer::fog::FogUniforms {
                fog_color: [0.09, 0.10, 0.13, 1.0],
                fog_near: 4500.0,
                fog_far: 26000.0,
                density: 0.08,
                time: 0.0,
                step_count: 20,
                curve_type: 0,
                speed_mult: 0.5,
                _padding: 0.0,
            };
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

/// Locate authentic character, enemy, boss, or weapon model and animation on disk.
/// Supports static/rigid weapon and item models by synthesizing an identity bind-pose animation track
/// if no `.anm` file is present or compatible with the model's bone count.
pub(crate) fn load_character_asset(
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
                loaded_base_root = Some(base.clone());

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
    let mut hd_model = hd_mdl_bytes.as_deref().and_then(|b| parse_mdl(b).ok());

    // If this is a `no_tex = 1` reflection or untextured variant (e.g. `ragl.mdl`, `rhhh_edi.mdl`,
    // `rlau.mdl`, `rmry.mdl`), bind the embedded textures from its primary companion `.mdl`.
    if model.textures.is_empty() {
        if let (Some(mdl_rel), Some(ref base)) = (loaded_mdl_rel, loaded_base_root) {
            if let Some(comp_rel) = companion_texture_mdl_path(mdl_rel) {
                let comp_path = PathBuf::from(base).join(comp_rel);
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
                if hd_model.as_ref().is_none_or(|h| h.textures.is_empty()) {
                    if let Some(stripped) = comp_rel.strip_prefix("data/") {
                        let sh2e_comp = PathBuf::from(base).join("sh2e").join(stripped);
                        if let Ok(hd_comp_bytes) = std::fs::read(&sh2e_comp) {
                            if let Ok(hd_comp_mdl) = parse_mdl(&hd_comp_bytes) {
                                hd_model = Some(hd_comp_mdl);
                            }
                        }
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
        });

    let cls = cls_bytes
        .as_deref()
        .and_then(|bytes| parse_cls(bytes).ok())
        .filter(|c| !c.blocks.is_empty());

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
        bind_locals,
        dynamic_locals,
        pose_scratch,
        skin_scratch,
        frame_scratch_a,
        frame_scratch_b,
        anm,
        cls,
        cluster_weights_scratch,
        morphed_positions_scratch,
        gpu_vertex_scratch,
        vanilla_texture_bind_groups,
        hd_texture_bind_groups,
        texture_bind_groups,
        secondary_physics,
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

/// Builds GPU diffuse texture bind groups for every material in a [`MapFile`], caching by `texture_id`
/// to avoid redundant VRAM allocations and falling back to [`RenderContext::default_texture_bind_group`].
pub(crate) fn build_map_texture_bind_groups(
    render_ctx: &RenderContext,
    map_file: &MapFile,
) -> Vec<Arc<wgpu::BindGroup>> {
    let mut map_texture_bind_groups = Vec::with_capacity(map_file.materials.len());
    let mut tex_bg_cache: std::collections::HashMap<u32, Arc<wgpu::BindGroup>> =
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

    map_texture_bind_groups
}

/// Loads a specific [`ModelViewerEntry`] and its selected `.anm` track index from disk.
pub(crate) fn load_viewer_entry_asset(
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

/// Active in-engine real-time 3D cutscene session (`DramaDemoPlayer` + loaded `.mes` table).
pub(crate) struct ActiveDramaDemoSession {
    pub player: sh2_core::DramaDemoPlayer,
    pub messages: Option<sh2_assets::formats::mes::MessageTable>,
    #[allow(dead_code)]
    pub catalog_index: usize,
}

/// Loads a [`sh2_core::DramaDemoPlayInfo`] cutscene script (`.dds`) and its stage `.mes` dialogue table from disk.
pub(crate) fn open_drama_demo_session(
    catalog_index: usize,
    reduce_widescreen_fov: bool,
) -> Option<ActiveDramaDemoSession> {
    let catalog = sh2_core::DRAMA_DEMO_CATALOG;
    if catalog.is_empty() {
        return None;
    }
    let idx = catalog_index % catalog.len();
    let info = catalog[idx];

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    let mut dds_bytes = None;
    for base in &base_roots {
        let p = PathBuf::from(base).join(info.dds_path);
        if let Ok(bytes) = std::fs::read(&p) {
            dds_bytes = Some(bytes);
            break;
        }
    }

    let dds_bytes = dds_bytes?;
    let script = sh2_assets::formats::demo::parse_demo_dds(&dds_bytes).ok()?;

    let mut messages = None;
    for base in &base_roots {
        let p = PathBuf::from(base).join(info.mes_path);
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(tbl) = sh2_assets::formats::mes::parse_mes(&bytes) {
                messages = Some(tbl);
                break;
            }
        }
    }

    tracing::info!(
        "[Cutscene Player] Opened DramaDemo #{}: '{}' ({}) -> {} frames, {} actors, {} lights",
        info.demo_no,
        info.title,
        info.dds_path,
        script.header.total_frames,
        script.header.character_names.len(),
        script.header.total_lights()
    );

    let player = sh2_core::DramaDemoPlayer::new(info, script, reduce_widescreen_fov);
    Some(ActiveDramaDemoSession {
        player,
        messages,
        catalog_index: idx,
    })
}

/// Decodes a cutscene voice track (`voice.afs` entry `afs_index`) from disk into a [`sh2_assets::WavEntry`].
pub(crate) fn load_cutscene_voice_wav(afs_index: usize) -> Option<sh2_assets::WavEntry> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];
    let rel_candidates = [
        "sh2e/sound/adx/voice/voice.afs",
        "data/sound/adx/voice/voice.afs",
    ];
    for base in &base_roots {
        for rel in &rel_candidates {
            let p = PathBuf::from(base).join(rel);
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(wav) = sh2_assets::decode_afs_entry_to_wav(&bytes, afs_index) {
                    return Some(wav);
                }
            }
        }
    }
    None
}
