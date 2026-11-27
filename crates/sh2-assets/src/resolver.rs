//! Centralized asset directory and retail path resolution for Silent Hill 2.

use std::path::{Path, PathBuf};

/// Centralized resolver for finding retail asset directories, movies, and configuration files.
#[derive(Debug, Clone)]
pub struct AssetPathResolver {
    /// Resolved base directory for vanilla retail assets (containing `data/`).
    pub game_root: PathBuf,
    /// Resolved path to the `data/` subdirectory.
    pub data_dir: PathBuf,
    /// Resolved path to the `sh2e/` subdirectory (if present).
    pub sh2e_dir: Option<PathBuf>,
}

impl Default for AssetPathResolver {
    fn default() -> Self {
        Self::resolve()
    }
}

impl AssetPathResolver {
    /// Standard candidate relative paths for the game root directory.
    pub const DEFAULT_CANDIDATE_PATHS: [&'static str; 3] = [
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
    ];

    /// Candidate paths for the Enhanced Edition OG HQ FMVs folder.
    pub const DEFAULT_OG_HQ_FMV_PATHS: [&'static str; 2] = [
        "sh2_enhanced_edition_og_hq_fmvs-50-1-0-1770228966/movie",
        "../sh2_enhanced_edition_og_hq_fmvs-50-1-0-1770228966/movie",
    ];

    /// Candidate paths for the AI 4K VSR pipeline output directory (`SH2-90`).
    pub const DEFAULT_VSR_OUTPUT_PATHS: [&'static str; 7] = [
        "mods/4k_vsr_fmvs/movie",
        "mods/4k_vsr_fmvs",
        "../mods/4k_vsr_fmvs/movie",
        "sh2e/movie_4k",
        "../sh2e/movie_4k",
        "tools/vsr_pipeline/output",
        "../../Documents/SH2-Port/tools/vsr_pipeline/output",
    ];

    /// Candidate paths for the 4K AI Super-Resolution World & Character Texture Pack (`SH2-92`).
    ///
    /// Supports both extracted modular directories (`mods/sh2_4k_textures/`)
    /// and direct zero-compression `.zip` archives (`mods/sh2_4k_textures.zip` or `mods/sh2_4k_textures/sh2_4k_textures.zip`).
    pub const DEFAULT_4K_TEXTURE_PACK_PATHS: [&'static str; 10] = [
        "mods/sh2_4k_textures",
        "../mods/sh2_4k_textures",
        "../../Documents/SH2-Port/mods/sh2_4k_textures",
        "mods/sh2_4k_textures.zip",
        "../mods/sh2_4k_textures.zip",
        "mods/sh2_4k_textures/sh2_4k_textures.zip",
        "../mods/sh2_4k_textures/sh2_4k_textures.zip",
        "../../Documents/SH2-Port/mods/sh2_4k_textures/sh2_4k_textures.zip",
        "tools/texture_pipeline/output",
        "../../Documents/SH2-Port/tools/texture_pipeline/output",
    ];

    /// Candidate paths for the 4K AI Super-Resolution Cutaway, Puzzle & Map Pack (`SH2-91`),
    /// prioritizing the unified `mods/sh2_4k_textures/` pack before legacy `mods/4k_cutaways/`.
    pub const DEFAULT_4K_CUTAWAY_PACK_PATHS: [&'static str; 8] = [
        "mods/sh2_4k_textures",
        "../mods/sh2_4k_textures",
        "../../Documents/SH2-Port/mods/sh2_4k_textures",
        "mods/4k_cutaways",
        "../mods/4k_cutaways",
        "../../Documents/SH2-Port/mods/4k_cutaways",
        "../../../Documents/SH2-Port/mods/4k_cutaways",
        "tools/cutaway_pipeline/output",
    ];

    /// Resolves the active asset directories, prioritizing `SH2_DATA_DIR` environment variable,
    /// then checking standard local workspace candidates.
    pub fn resolve() -> Self {
        // 1. Environment variable override
        if let Ok(env_path) = std::env::var("SH2_DATA_DIR") {
            let p = PathBuf::from(env_path);
            if p.exists() {
                let (game_root, data_dir) = if p.join("data").is_dir() {
                    (p.clone(), p.join("data"))
                } else {
                    let parent = p.parent().unwrap_or(&p).to_path_buf();
                    (parent, p.clone())
                };
                let sh2e = game_root.join("sh2e");
                let sh2e_dir = if sh2e.is_dir() { Some(sh2e) } else { None };
                return Self {
                    game_root,
                    data_dir,
                    sh2e_dir,
                };
            }
        }

        // 2. Search candidate locations
        for cand in Self::DEFAULT_CANDIDATE_PATHS {
            let p = Path::new(cand);
            if p.join("data").is_dir() {
                let sh2e = p.join("sh2e");
                let sh2e_dir = if sh2e.is_dir() { Some(sh2e) } else { None };
                return Self {
                    game_root: p.to_path_buf(),
                    data_dir: p.join("data"),
                    sh2e_dir,
                };
            }
        }

        // Fallback default
        let fallback = PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
        Self {
            data_dir: fallback.join("data"),
            sh2e_dir: Some(fallback.join("sh2e")),
            game_root: fallback,
        }
    }

    /// Resolves an asset file path relative to `data_dir` or `sh2e_dir`.
    pub fn resolve_file(&self, rel_path: impl AsRef<Path>) -> Option<PathBuf> {
        let rel = rel_path.as_ref();
        if let Some(ref sh2e) = self.sh2e_dir {
            let cand = sh2e.join(rel);
            if cand.exists() {
                return Some(cand);
            }
        }
        let cand = self.data_dir.join(rel);
        if cand.exists() {
            Some(cand)
        } else {
            None
        }
    }

    /// Returns `true` when the SH2:EE lossless audio pack (`sh2e/sound/adx/` or `sh2e/sound/sddata.bin`) is present.
    pub fn has_sh2ee_sound_pack(&self) -> bool {
        self.sh2e_dir.as_ref().is_some_and(|sh2e| {
            let snd = sh2e.join("sound");
            snd.join("adx").is_dir() || snd.join("sddata.bin").is_file()
        })
    }

    /// Returns `true` when the original PS2/PC retail audio pack (`data/sound/adx/` or `data/sound/sddata.bin`) is present.
    pub fn has_ps2_sound_pack(&self) -> bool {
        let snd = self.data_dir.join("sound");
        snd.join("adx").is_dir() || snd.join("sddata.bin").is_file()
    }

    /// Returns `true` when both SH2:EE lossless (`sh2e/sound/`) and original PS2/PC (`data/sound/`) audio packs are present.
    pub fn has_dual_audio_packs(&self) -> bool {
        self.has_sh2ee_sound_pack() && self.has_ps2_sound_pack()
    }

    /// Returns `true` when the SH2:EE Image Enhancement Pack (`sh2e/pic/`) is present.
    pub fn has_sh2ee_pic_pack(&self) -> bool {
        self.sh2e_dir
            .as_ref()
            .is_some_and(|sh2e| sh2e.join("pic").is_dir())
    }

    /// Returns `true` when the original PS2/PC picture pack (`data/pic/`) is present.
    pub fn has_ps2_pic_pack(&self) -> bool {
        self.data_dir.join("pic").is_dir()
    }

    /// Resolves the user-accessible `mods/` directory (`SH2_MODS_DIR`, local `mods/`,
    /// `<game_root>/mods/`, or shared workspace `SH2-Port/mods/`).
    #[must_use]
    pub fn resolve_mods_dir(&self) -> PathBuf {
        if let Ok(env_dir) = std::env::var("SH2_MODS_DIR") {
            let p = PathBuf::from(env_dir);
            if p.is_dir() {
                return p;
            }
        }
        let mut candidates = vec![
            PathBuf::from("mods"),
            PathBuf::from("../mods"),
            self.game_root.join("mods"),
            PathBuf::from("../../Documents/SH2-Port/mods"),
            PathBuf::from("../../../Documents/SH2-Port/mods"),
        ];
        if let Ok(canon) = self.game_root.canonicalize() {
            if let Some(port_root) = canon.parent().and_then(|p| p.parent()) {
                candidates.push(port_root.join("mods"));
            }
        }
        for cand in candidates {
            if cand.is_dir() {
                return cand;
            }
        }
        PathBuf::from("mods")
    }

    /// Resolves the active 4K AI VSR FMV directory (`SH2-90`), prioritizing `SH2_VSR_DIR`,
    /// then `mods/4k_vsr_fmvs/movie/`, `mods/4k_vsr_fmvs/`, `<game_root>/sh2e/movie_4k`, and workspace candidates.
    #[must_use]
    pub fn resolve_4k_vsr_fmv_dir(&self) -> Option<PathBuf> {
        if let Ok(env_dir) = std::env::var("SH2_VSR_DIR") {
            let p = PathBuf::from(env_dir);
            if p.is_dir() {
                return Some(p);
            }
        }
        let mods_dir = self.resolve_mods_dir();
        for cand in [
            mods_dir.join("4k_vsr_fmvs/movie"),
            mods_dir.join("4k_vsr_fmvs"),
            self.game_root.join("sh2e/movie_4k"),
            self.game_root.join("mods/4k_vsr_fmvs/movie"),
            self.game_root.join("mods/4k_vsr_fmvs"),
        ] {
            if cand.is_dir() && dir_has_4k_vsr_fmv(&cand) {
                return Some(cand);
            }
        }
        for cand in Self::DEFAULT_VSR_OUTPUT_PATHS {
            let p = PathBuf::from(cand);
            if p.is_dir() && dir_has_4k_vsr_fmv(&p) {
                return Some(p);
            }
        }
        None
    }

    /// Returns `true` when at least one `*_4k_master.mp4` or `manifest.json` is detected on disk (`SH2-90`).
    #[must_use]
    pub fn has_4k_vsr_fmv_pack(&self) -> bool {
        self.resolve_4k_vsr_fmv_dir().is_some()
    }

    /// Resolves the active 4K AI Super-Resolution Texture Pack directory or `.zip` archive (`SH2-92`),
    /// prioritizing `SH2_4K_TEX_DIR` / `SH2_4K_TEXTURE_DIR` environment variables, then checking
    /// `<game_root>/sh2e_4k`, `<game_root>/mods/sh2_4k_textures(.zip)`, and workspace candidates.
    pub fn resolve_4k_texture_pack_dir(&self) -> Option<PathBuf> {
        if let Ok(env_dir) =
            std::env::var("SH2_4K_TEX_DIR").or_else(|_| std::env::var("SH2_4K_TEXTURE_DIR"))
        {
            let p = PathBuf::from(env_dir);
            if p.is_dir() || (p.is_file() && p.extension().is_some_and(|e| e == "zip")) {
                return Some(p);
            }
        }
        let mods_dir = self.resolve_mods_dir();
        let mods_4k = mods_dir.join("sh2_4k_textures");
        if mods_4k.is_dir() {
            return Some(mods_4k);
        }
        for rel in [
            "mods/sh2_4k_textures",
            "sh2e_4k",
            "mods/sh2_4k_textures.zip",
            "mods/sh2_4k_textures/sh2_4k_textures.zip",
        ] {
            let cand = self.game_root.join(rel);
            if cand.is_dir() || (cand.is_file() && cand.extension().is_some_and(|e| e == "zip")) {
                return Some(cand);
            }
        }
        let mods_4k_zip = mods_dir.join("sh2_4k_textures.zip");
        if mods_4k_zip.is_file() {
            return Some(mods_4k_zip);
        }
        for cand in Self::DEFAULT_4K_TEXTURE_PACK_PATHS {
            let p = PathBuf::from(cand);
            if p.is_dir() || (p.is_file() && p.extension().is_some_and(|e| e == "zip")) {
                return Some(p);
            }
        }
        None
    }

    /// Returns `true` when the 4K AI Super-Resolution Texture Pack (`map/`, `chr/`, `effect/`, `pic/` folder, or `.zip` archive) is present on disk.
    pub fn has_4k_texture_pack(&self) -> bool {
        self.resolve_4k_texture_pack_dir().is_some_and(|dir| {
            (dir.is_file() && dir.extension().is_some_and(|e| e == "zip"))
                || dir.join("map").is_dir()
                || dir.join("chr").is_dir()
                || dir.join("effect").is_dir()
                || dir.join("pic").is_dir()
                || dir.join("sh2_4k_textures.zip").is_file()
        })
    }

    /// Resolves the active 4K AI Cutaway, Puzzle & Map Pack directory (`mods/sh2_4k_textures/` or `mods/4k_cutaways/`, `SH2-91`).
    pub fn resolve_4k_cutaway_pack_dir(&self) -> Option<PathBuf> {
        if let Ok(env_dir) = std::env::var("SH2_4K_CUTAWAY_DIR") {
            let p = PathBuf::from(env_dir);
            if p.join("pic").is_dir() {
                return Some(p);
            }
        }
        let mods_dir = self.resolve_mods_dir();
        for cand in [
            mods_dir.join("sh2_4k_textures"),
            mods_dir.join("4k_cutaways"),
            self.game_root.join("mods/sh2_4k_textures"),
            self.game_root.join("mods/4k_cutaways"),
        ] {
            if cand.join("pic").is_dir() {
                return Some(cand);
            }
        }
        for cand in Self::DEFAULT_4K_CUTAWAY_PACK_PATHS {
            let p = PathBuf::from(cand);
            if p.join("pic").is_dir() {
                return Some(p);
            }
        }
        None
    }

    /// Returns `true` when the 4K AI Cutaway Pack (`mods/4k_cutaways/pic/`) is present on disk.
    pub fn has_4k_cutaway_pack(&self) -> bool {
        self.resolve_4k_cutaway_pack_dir().is_some()
    }

    /// Resolves a cutaway still or overlay sprite sheet (e.g. `"out_p_incar"`, `"apt_clock_close"`,
    /// or `"out/p_incar_map"`), honoring `prefer_sh2ee` with graceful fallback.
    pub fn resolve_cutaway_file(
        &self,
        pic_or_sheet_name: &str,
        prefer_sh2ee: bool,
    ) -> Option<PathBuf> {
        let rel =
            crate::formats::texture::TextureLoader::normalize_cutaway_rel_path(pic_or_sheet_name);
        crate::formats::texture::resolve_path_with_preference_in_dirs(
            &self.data_dir,
            self.sh2e_dir.as_deref(),
            &rel,
            prefer_sh2ee,
        )
        .map(|(p, _)| p)
    }

    /// Resolves a sound asset file (e.g. `sound/sddata.bin`, `sound/extra/g_start.wav`, `sound/adx/...`),
    /// prioritizing `sh2e/` when `prefer_sh2ee` is `true`, or `data/` when `prefer_sh2ee` is `false`
    /// (with graceful fallback to the other pack if a specific file only exists in one location).
    pub fn resolve_sound_file(
        &self,
        rel_path: impl AsRef<Path>,
        prefer_sh2ee: bool,
    ) -> Option<PathBuf> {
        let rel = rel_path.as_ref();
        if prefer_sh2ee {
            if let Some(ref sh2e) = self.sh2e_dir {
                let cand = sh2e.join(rel);
                if cand.exists() {
                    return Some(cand);
                }
            }
            let cand = self.data_dir.join(rel);
            if cand.exists() {
                return Some(cand);
            }
        } else {
            let cand = self.data_dir.join(rel);
            if cand.exists() {
                return Some(cand);
            }
            if let Some(ref sh2e) = self.sh2e_dir {
                let cand = sh2e.join(rel);
                if cand.exists() {
                    return Some(cand);
                }
            }
        }
        None
    }

    /// Resolves the movie file path for FMVs, checking 4K VSR, OG HQ, or vanilla retail movies.
    pub fn resolve_movie_file(movie_name: &str) -> Option<PathBuf> {
        let resolver = Self::resolve();
        // 1. Check data/movie/
        let direct = resolver.data_dir.join("movie").join(movie_name);
        if direct.exists() {
            return Some(direct);
        }

        // 2. Check OG HQ FMVs
        for cand in Self::DEFAULT_OG_HQ_FMV_PATHS {
            let p = Path::new(cand).join(movie_name);
            if p.exists() {
                return Some(p);
            }
        }

        None
    }

    /// Resolves a room camera trigger volume file (`.cam`) from a room identifier (`"er01"`, `"ob01"`,
    /// `"rr21"`, `"ap01"`, `"ma61"`) or relative path (`"bg/er/er01.cam"`, `"bg2/ma/ma61.cam"`),
    /// prioritizing SH2:EE overrides (`sh2e/bg/<area>/<room>.cam` or `sh2e/bg2/<area>/<room>.cam`)
    /// when `prefer_sh2ee` is `true` before falling back to vanilla retail `data/bg/...` (`SH2-50`).
    ///
    /// Returns `(resolved_path, source_kind, canonical_room_id, canonical_rel_path)`.
    pub fn resolve_cam_file(
        &self,
        room_or_rel_path: &str,
        prefer_sh2ee: bool,
    ) -> Option<(PathBuf, crate::formats::cam::CamSourceKind, String, String)> {
        let (room_id, rel_path) =
            crate::formats::cam::normalize_room_cam_rel_path(room_or_rel_path)?;
        let (resolved_path, is_sh2ee) =
            crate::formats::texture::resolve_path_with_preference_in_dirs(
                &self.data_dir,
                self.sh2e_dir.as_deref(),
                Path::new(&rel_path),
                prefer_sh2ee,
            )?;
        let source = if is_sh2ee {
            crate::formats::cam::CamSourceKind::Sh2eeOverride
        } else {
            crate::formats::cam::CamSourceKind::VanillaRetail
        };
        Some((resolved_path, source, room_id, rel_path))
    }

    /// Resolves and parses ONLY the requested single `.cam` trigger volume file (`room_or_rel_path`)
    /// without implicitly appending `<area>GB.cam`.
    ///
    /// Used by `Ps2CameraController`'s multi-slot `OutdoorCamBank` (`SH2-184`), which keeps
    /// `<area>GB.cam` resident in Slot 0 (`SLUS_202.28` `loadBgCAM_ReplaceG` `0x00271d20` /
    /// `sh2pc.exe` `FUN_005132c0` `0x005132c0`) separately from active per-tile `.cam` slots
    /// (`loadBgCAM_Replace` `0x00271e10`).
    pub fn load_single_cam_file(
        &self,
        room_or_rel_path: &str,
        prefer_sh2ee: bool,
    ) -> Result<Option<crate::formats::cam::ResolvedCamAsset>, crate::error::AssetError> {
        let Some((resolved_path, source, room_id, relative_path)) =
            self.resolve_cam_file(room_or_rel_path, prefer_sh2ee)
        else {
            return Ok(None);
        };

        let bytes = std::fs::read(&resolved_path)?;
        let cam_file = crate::formats::cam::parse_cam(&bytes)?;

        Ok(Some(crate::formats::cam::ResolvedCamAsset {
            room_id,
            relative_path,
            resolved_path,
            source,
            cam_file,
        }))
    }

    /// Resolves and parses the `.cam` trigger volume file for `room_or_rel_path`, prioritizing
    /// SH2:EE widescreen/high-framerate overrides (`sh2e/bg/...` / `sh2e/bg2/...`) when
    /// `prefer_sh2ee` is `true` before falling back to vanilla retail (`data/bg/...` / `data/bg2/...`).
    ///
    /// For town outdoor rooms (`ca*`, `cb*`, `cc*`, `cd*`), if the area's global `*GB.cam` file is
    /// present on disk, its supplementary road records are automatically appended after the room's
    /// primary records (`SH2-50`).
    pub fn load_cam_for_room(
        &self,
        room_or_rel_path: &str,
        prefer_sh2ee: bool,
    ) -> Result<Option<crate::formats::cam::ResolvedCamAsset>, crate::error::AssetError> {
        let Some((resolved_path, source, room_id, relative_path)) =
            self.resolve_cam_file(room_or_rel_path, prefer_sh2ee)
        else {
            return Ok(None);
        };

        let bytes = std::fs::read(&resolved_path)?;
        let mut cam_file = crate::formats::cam::parse_cam(&bytes)?;

        // If this is an outdoor town room (ca/cb/cc/cd), append global area `<area>GB.cam` fallback records if present
        if let Some(gb_rel) = crate::formats::cam::global_area_cam_rel_path(&room_id) {
            if let Some((gb_path, _, _, _)) = self.resolve_cam_file(&gb_rel, prefer_sh2ee) {
                if gb_path != resolved_path {
                    if let Ok(gb_bytes) = std::fs::read(&gb_path) {
                        if let Ok(gb_cam) = crate::formats::cam::parse_cam(&gb_bytes) {
                            cam_file.extend_records(gb_cam);
                        }
                    }
                }
            }
        }

        Ok(Some(crate::formats::cam::ResolvedCamAsset {
            room_id,
            relative_path,
            resolved_path,
            source,
            cam_file,
        }))
    }

    /// Resolves an optional SH2:EE 3D replacement model under `sh2e/model/<filename>`
    /// (e.g., `"ty3.glb"`, `"mon.glb"`, `"dmr.glb"`), returning `None` if `sh2e/` or the
    /// requested `.glb` file is absent on disk.
    pub fn resolve_sh2e_model(&self, filename: &str) -> Option<PathBuf> {
        let sh2e = self.sh2e_dir.as_deref()?;
        let model_dir = sh2e.join("model");
        let exact = model_dir.join(filename);
        if exact.is_file() {
            return Some(exact);
        }
        if let Ok(entries) = std::fs::read_dir(&model_dir) {
            let target_lower = filename.to_ascii_lowercase();
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.to_ascii_lowercase() == target_lower)
                {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Returns `true` when an optional SH2:EE `.glb` replacement model exists in `sh2e/model/`.
    pub fn has_sh2e_model(&self, filename: &str) -> bool {
        self.resolve_sh2e_model(filename).is_some()
    }
}

fn dir_has_4k_vsr_fmv(dir: &Path) -> bool {
    if dir.join("manifest.json").is_file() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_name()
            .to_str()
            .is_some_and(|n| n.ends_with("_4k_master.mp4"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::cam::CamSourceKind;

    #[test]
    fn test_sh2_50_resolve_and_load_cam_with_sh2ee_override_priority() {
        let resolver = AssetPathResolver::resolve();
        if !resolver.data_dir.exists() {
            return;
        }

        // 1. Rooms that have SH2:EE override .cam files in `sh2e/bg/` and `sh2e/bg2/`
        let sh2ee_override_rooms = [
            ("er01", "bg/er/er01.cam"),
            ("ob01", "bg/ob/ob01.cam"),
            ("ap01", "bg/ap/ap01.cam"),
            ("ap108", "bg/ap/ap108.cam"),
            ("cc11", "bg/cc/cc11.cam"),
            ("hp121", "bg/hp/hp121.cam"),
            ("ps66", "bg/ps/ps66.cam"),
            ("rr25", "bg/rr/rr25.cam"),
            ("ma61", "bg2/ma/ma61.cam"),
            ("ma69", "bg2/ma/ma69.cam"),
        ];

        for (room_id, expected_rel) in sh2ee_override_rooms {
            // With `prefer_sh2ee = true`, must resolve to `sh2e/` (`Sh2eeOverride`)
            let asset = resolver
                .load_cam_for_room(room_id, true)
                .expect("Should parse .cam without error")
                .unwrap_or_else(|| panic!("Expected .cam asset for {room_id}"));
            assert_eq!(asset.room_id, room_id);
            assert_eq!(asset.relative_path, expected_rel);
            assert_eq!(
                asset.source,
                CamSourceKind::Sh2eeOverride,
                "Expected {room_id} to resolve from sh2e/"
            );
            assert!(!asset.cam_file.records.is_empty());

            // With `prefer_sh2ee = false`, must resolve to `data/` (`VanillaRetail`)
            let vanilla_asset = resolver
                .load_cam_for_room(room_id, false)
                .expect("Should parse vanilla .cam without error")
                .unwrap_or_else(|| panic!("Expected vanilla .cam asset for {room_id}"));
            assert_eq!(vanilla_asset.source, CamSourceKind::VanillaRetail);
            assert!(!vanilla_asset.cam_file.records.is_empty());
        }

        // 2. Rooms that only exist in vanilla `data/bg/` or `data/bg2/` must gracefully fall back
        let vanilla_only_rooms = [
            ("rr21", "bg/rr/rr21.cam"),
            ("th09", "bg/th/th09.cam"),
            ("ma01", "bg2/ma/ma01.cam"),
            ("bw01", "bg/bw/bw01.cam"),
        ];

        for (room_id, expected_rel) in vanilla_only_rooms {
            let asset = resolver
                .load_cam_for_room(room_id, true)
                .expect("Should parse fallback .cam")
                .unwrap_or_else(|| panic!("Expected fallback .cam for {room_id}"));
            assert_eq!(asset.room_id, room_id);
            assert_eq!(asset.relative_path, expected_rel);
            assert_eq!(asset.source, CamSourceKind::VanillaRetail);
            assert!(!asset.cam_file.records.is_empty());
        }
    }
}
