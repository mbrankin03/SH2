//! Cutscene session loader (`ActiveDramaDemoSession`, `open_drama_demo_session`, `load_cutscene_voice_wav`) (`SH2-63`).

use std::path::PathBuf;

use crate::assets::{load_character_asset, CharacterAsset};
use sh2_renderer::RenderContext;

/// Active in-engine real-time 3D cutscene session (`DramaDemoPlayer` + loaded `.mes` table + actor models/animations).
pub(crate) struct ActiveDramaDemoSession {
    pub player: sh2_core::DramaDemoPlayer,
    pub messages: Option<sh2_assets::formats::mes::MessageTable>,
    /// Loaded 3D character/prop assets (`.mdl`, `.anm`, `.cls`) corresponding 1:1 with `player.actors`.
    pub actor_assets: Vec<Option<CharacterAsset>>,
    /// Per-actor planar mirror reflection skinning scratch buffers (`0..player.actors.len()`).
    pub actor_mirror_skin_scratch: Vec<Vec<[[f32; 4]; 4]>>,
    #[allow(dead_code)]
    pub catalog_index: usize,
}

/// Loads a [`sh2_core::DramaDemoPlayInfo`] cutscene script (`.dds`), its stage `.mes` dialogue table,
/// and all actor 3D models (`.mdl`), motion clips (`.anm`), and facial morph clusters (`.cls`) from disk.
pub(crate) fn open_drama_demo_session(
    render_ctx: &RenderContext,
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

    let player = sh2_core::DramaDemoPlayer::new(info, script, reduce_widescreen_fov);
    let mut actor_assets = Vec::with_capacity(player.actors.len());
    let mut actor_mirror_skin_scratch = Vec::with_capacity(player.actors.len());

    let is_born_from_a_wish = info.dds_path.starts_with("data/demo2/");
    for actor in &player.actors {
        let loaded = if let Some(anim_info) = actor.anim_info {
            let dc_override = anim_info
                .mdl_path
                .strip_prefix("data/chr/")
                .map(|rest| format!("data/chr2/{rest}"));
            let mdl_candidates: Vec<&str> = if is_born_from_a_wish {
                if let Some(ref dc) = dc_override {
                    vec![dc.as_str(), anim_info.mdl_path]
                } else {
                    vec![anim_info.mdl_path]
                }
            } else {
                vec![anim_info.mdl_path]
            };
            let anm_candidates = [actor.anm_path.as_str()];
            load_character_asset(
                render_ctx,
                anim_info.actor_name,
                &mdl_candidates,
                &anm_candidates,
            )
        } else {
            None
        };
        let bone_cap = loaded.as_ref().map(|a| a.model.bone_count).unwrap_or(0);
        actor_mirror_skin_scratch.push(Vec::with_capacity(bone_cap));
        actor_assets.push(loaded);
    }

    let loaded_actor_count = actor_assets.iter().filter(|a| a.is_some()).count();
    tracing::info!(
        "[Cutscene Player] Opened DramaDemo #{}: '{}' ({}) -> {} frames, {}/{} actors loaded, {} lights",
        info.demo_no,
        info.title,
        info.dds_path,
        player.total_frames(),
        loaded_actor_count,
        player.actors.len(),
        player.evaluator.script.header.total_lights()
    );

    Some(ActiveDramaDemoSession {
        player,
        messages,
        actor_assets,
        actor_mirror_skin_scratch,
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
