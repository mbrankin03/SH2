//! Character, enemy, weapon, and Model Viewer `.mdl`/`.anm`/`.cls` asset loading (`SH2-181`).

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::anm::{compute_bind_locals, parse_anm, AnimationFile, AnimationFrame};
use sh2_assets::formats::cls::{parse_cls, ClsFile};
use sh2_assets::formats::mdls::{companion_texture_mdl_path, parse_mdl, ModelFile};
use sh2_core::config::WorldTextureQuality;
use sh2_core::SecondaryBonePhysics;
use sh2_debug::ModelViewerEntry;
use sh2_renderer::{GpuSkeletalMesh, RenderContext, SkeletalVertex};

use super::character_textures::{
    build_character_texture_bind_groups_for_quality, load_vanilla_model_textures,
};

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
) -> Option<AnimationFile> {
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
