//! Cutscene session loader (`ActiveDramaDemoSession`, `open_drama_demo_session`, `load_cutscene_voice_wav`) (`SH2-63`).

use std::path::PathBuf;
use std::sync::Arc;

use crate::assets::{
    load_character_asset, load_map_for_quality_from_roots, CharacterAsset, MapTierAssets,
};
use sh2_core::config::WorldTextureQuality;
use sh2_renderer::{GpuMap, RenderContext};

/// Active in-engine real-time 3D cutscene session (`DramaDemoPlayer` + loaded `.mes` table + actor models/animations + stage `.map`).
pub(crate) struct ActiveDramaDemoSession {
    pub player: sh2_core::DramaDemoPlayer,
    pub messages: Option<sh2_assets::formats::mes::MessageTable>,
    /// Preloaded cutscene voice track (`voice.afs[stream_no - 60000]`, matching `DramaDemoMain`
    /// `_demo_status |= 0x80` stream preload at `SLUS_202.28` `0x001b0f00` / `sh2pc.exe` `0x004a2110`)
    /// so playback begins at `scene_frame >= stream_start` with zero mid-cutscene disk I/O hitch (`SH2-188`).
    pub preloaded_voice_wav: Option<sh2_assets::WavEntry>,
    /// Loaded 3D character/prop assets (`.mdl`, `.anm`, `.cls`) corresponding 1:1 with `player.actors`.
    pub actor_assets: Vec<Option<CharacterAsset>>,
    /// Per-actor planar mirror reflection skinning scratch buffers (`0..player.actors.len()`).
    pub actor_mirror_skin_scratch: Vec<Vec<[[f32; 4]; 4]>>,
    /// Authentic stage `.map` geometry and textures loaded for the single active quality tier (`SH2-181`).
    pub stage_gpu_map: Option<Arc<GpuMap>>,
    pub stage_texture_bind_groups: Option<Vec<Arc<wgpu::BindGroup>>>,
    /// Active [`WorldTextureQuality`] tier currently resident for this cutscene session (`SH2-181`).
    pub loaded_quality: WorldTextureQuality,
    #[allow(dead_code)]
    pub catalog_index: usize,
}

impl ActiveDramaDemoSession {
    /// Reloads cutscene actor textures and stage `.map` geometry/textures on demand when
    /// `WorldTextureQuality` changes mid-cutscene (`SH2-181`).
    pub fn reload_for_quality(&mut self, render_ctx: &RenderContext, quality: WorldTextureQuality) {
        if self.loaded_quality == quality {
            return;
        }
        for actor in self.actor_assets.iter_mut().flatten() {
            actor.reload_textures_for_quality(render_ctx, quality);
        }
        let info = self.player.info;
        if !info.stage_map_path.is_empty() {
            if let Some(map) =
                load_map_for_quality_from_roots(info.stage_map_path, info.stage_name, quality)
            {
                let assets = MapTierAssets::from_map(render_ctx, map);
                self.stage_gpu_map = Some(assets.gpu_map);
                self.stage_texture_bind_groups = Some(assets.texture_bind_groups);
            }
        }
        self.loaded_quality = quality;
    }
}

/// Loads a [`sh2_core::DramaDemoPlayInfo`] cutscene script (`.dds`), its stage `.mes` dialogue table,
/// its `voice.afs` audio track (`stream_no - 60000` per `sh2pc.exe` `FUN_00514b40` `0x00514b40`),
/// and all actor 3D models (`.mdl`), motion clips (`.anm`), and facial morph clusters (`.cls`) from disk
/// for the single active `quality` tier (`SH2-181`, `SH2-188`).
pub(crate) fn open_drama_demo_session(
    render_ctx: &RenderContext,
    catalog_index: usize,
    reduce_widescreen_fov: bool,
    quality: WorldTextureQuality,
    audio_pack: sh2_core::AudioPackSource,
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
                quality,
            )
        } else {
            None
        };
        let bone_cap = loaded.as_ref().map(|a| a.model.bone_count).unwrap_or(0);
        actor_mirror_skin_scratch.push(Vec::with_capacity(bone_cap));
        actor_assets.push(loaded);
    }

    let loaded_actor_count = actor_assets.iter().filter(|a| a.is_some()).count();

    let mut stage_gpu_map = None;
    let mut stage_texture_bind_groups = None;
    if !info.stage_map_path.is_empty() {
        if let Some(map) =
            load_map_for_quality_from_roots(info.stage_map_path, info.stage_name, quality)
        {
            let assets = MapTierAssets::from_map(render_ctx, map);
            stage_gpu_map = Some(assets.gpu_map);
            stage_texture_bind_groups = Some(assets.texture_bind_groups);
        }
    }

    let preloaded_voice_wav = player
        .info
        .voice_afs_index()
        .and_then(|afs_idx| load_cutscene_voice_wav_for_source(afs_idx, audio_pack));

    tracing::info!(
        "[Cutscene Player] Opened DramaDemo #{}: '{}' ({}, stage={}, tier={:?}) -> {} frames, {}/{} actors loaded, {} lights, voice_afs={:?} (preloaded={})",
        info.demo_no,
        info.title,
        info.dds_path,
        info.stage_map_path,
        quality,
        player.total_frames(),
        loaded_actor_count,
        player.actors.len(),
        player.evaluator.script.header.total_lights(),
        info.voice_afs_index(),
        preloaded_voice_wav.is_some(),
    );

    Some(ActiveDramaDemoSession {
        player,
        messages,
        preloaded_voice_wav,
        actor_assets,
        actor_mirror_skin_scratch,
        stage_gpu_map,
        stage_texture_bind_groups,
        loaded_quality: quality,
        catalog_index: idx,
    })
}

/// Decodes a cutscene voice track (`voice.afs` entry `afs_index` = `stream_no - 60000` per
/// `sh2pc.exe` `FUN_00514b40` `0x00514b40`) from disk into a [`sh2_assets::WavEntry`] by parsing
/// only the `AFS\0` header/TOC and seek-reading the single requested entry (`SH2-175`, `SH2-188`).
///
/// Honors `audio_pack` (`Ps2Original` prefers `data/sound/adx/voice/voice.afs`; `EnhancedLossless`
/// prefers `sh2e/sound/adx/voice/voice.afs`), falling back to the alternate pack if absent or unreadable.
pub(crate) fn load_cutscene_voice_wav_for_source(
    afs_index: usize,
    audio_pack: sh2_core::AudioPackSource,
) -> Option<sh2_assets::WavEntry> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];
    let rel_candidates = match audio_pack {
        sh2_core::AudioPackSource::Ps2Original => [
            "data/sound/adx/voice/voice.afs",
            "sh2e/sound/adx/voice/voice.afs",
        ],
        sh2_core::AudioPackSource::Auto | sh2_core::AudioPackSource::EnhancedLossless => [
            "sh2e/sound/adx/voice/voice.afs",
            "data/sound/adx/voice/voice.afs",
        ],
    };
    for base in &base_roots {
        for rel in &rel_candidates {
            let p = PathBuf::from(base).join(rel);
            if let Ok(mut file) = std::fs::File::open(&p) {
                match sh2_assets::decode_afs_entry_from_reader(&mut file, afs_index) {
                    Ok(wav) => return Some(wav),
                    Err(e) => {
                        tracing::warn!(
                            "[Cutscene Voice] Failed to decode voice.afs[{}] from '{}': {}",
                            afs_index,
                            p.display(),
                            e
                        );
                    }
                }
            }
        }
    }
    tracing::warn!(
        "[Cutscene Voice] Could not locate/decode voice.afs[{}] (pack={:?}, game_root='{}')",
        afs_index,
        audio_pack,
        resolver.game_root.display()
    );
    None
}

/// Decodes a cutscene voice track (`voice.afs` entry `afs_index`) preferring SH2:EE lossless audio.
#[allow(dead_code)]
pub(crate) fn load_cutscene_voice_wav(afs_index: usize) -> Option<sh2_assets::WavEntry> {
    load_cutscene_voice_wav_for_source(afs_index, sh2_core::AudioPackSource::EnhancedLossless)
}
