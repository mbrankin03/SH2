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

    /// Candidate paths for the AI 4K VSR pipeline output directory.
    pub const DEFAULT_VSR_OUTPUT_PATHS: [&'static str; 2] =
        ["tools/vsr_pipeline/output", "../tools/vsr_pipeline/output"];

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

    /// Resolves a cutaway still or overlay sprite sheet (e.g. `"out_p_incar"`, `"apt_clock_close"`,
    /// or `"out/p_incar_map"`), honoring `prefer_sh2ee` with graceful fallback.
    pub fn resolve_cutaway_file(
        &self,
        pic_or_sheet_name: &str,
        prefer_sh2ee: bool,
    ) -> Option<PathBuf> {
        let loader =
            crate::formats::texture::TextureLoader::new(&self.data_dir, self.sh2e_dir.as_ref());
        let rel =
            crate::formats::texture::TextureLoader::normalize_cutaway_rel_path(pic_or_sheet_name);
        loader
            .resolve_path_with_preference(&rel, prefer_sh2ee)
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
        let loader =
            crate::formats::texture::TextureLoader::new(&self.data_dir, self.sh2e_dir.as_ref());
        let (resolved_path, is_sh2ee) =
            loader.resolve_path_with_preference(&rel_path, prefer_sh2ee)?;
        let source = if is_sh2ee {
            crate::formats::cam::CamSourceKind::Sh2eeOverride
        } else {
            crate::formats::cam::CamSourceKind::VanillaRetail
        };
        Some((resolved_path, source, room_id, rel_path))
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
