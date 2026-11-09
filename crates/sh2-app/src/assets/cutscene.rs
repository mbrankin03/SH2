//! Cutscene session loader (`ActiveDramaDemoSession`, `open_drama_demo_session`, `load_cutscene_voice_wav`) (`SH2-63`).

use std::path::PathBuf;

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
