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

    /// Candidate paths for the 4K AI Super-Resolution World & Character Texture Pack (`SH2-92`).
    ///
    /// Supports both extracted modular directories (`mods/sh2_4k_textures/`)
    /// and direct zero-compression `.zip` archives (`mods/sh2_4k_textures.zip` or `mods/sh2_4k_textures/sh2_4k_textures.zip`).
    pub const DEFAULT_4K_TEXTURE_PACK_PATHS: [&'static str; 10] = [
        "mods/sh2_4k_textures.zip",
        "../mods/sh2_4k_textures.zip",
        "mods/sh2_4k_textures/sh2_4k_textures.zip",
        "../mods/sh2_4k_textures/sh2_4k_textures.zip",
        "mods/sh2_4k_textures",
        "../mods/sh2_4k_textures",
        "/home/matthewb/Documents/SH2-Port/mods/sh2_4k_textures/sh2_4k_textures.zip",
        "/home/matthewb/Documents/SH2-Port/mods/sh2_4k_textures",
        "tools/texture_pipeline/output",
        "/home/matthewb/Documents/SH2-Port/tools/texture_pipeline/output",
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

    /// Resolves the active 4K AI Super-Resolution Texture Pack directory or `.zip` archive (`SH2-92`),
    /// prioritizing `SH2_4K_TEX_DIR` environment variable, then checking `<game_root>/sh2e_4k`,
    /// `<game_root>/mods/sh2_4k_textures(.zip)`, and workspace candidates.
    pub fn resolve_4k_texture_pack_dir(&self) -> Option<PathBuf> {
        if let Ok(env_dir) = std::env::var("SH2_4K_TEX_DIR") {
            let p = PathBuf::from(env_dir);
            if p.is_dir() || (p.is_file() && p.extension().is_some_and(|e| e == "zip")) {
                return Some(p);
            }
        }
        for rel in [
            "mods/sh2_4k_textures.zip",
            "mods/sh2_4k_textures/sh2_4k_textures.zip",
            "mods/sh2_4k_textures",
            "sh2e_4k",
        ] {
            let cand = self.game_root.join(rel);
            if cand.is_dir() || (cand.is_file() && cand.extension().is_some_and(|e| e == "zip")) {
                return Some(cand);
            }
        }
        for cand in Self::DEFAULT_4K_TEXTURE_PACK_PATHS {
            let p = PathBuf::from(cand);
            if p.is_dir() || (p.is_file() && p.extension().is_some_and(|e| e == "zip")) {
                return Some(p);
            }
        }
        None
    }

    /// Returns `true` when the 4K AI Super-Resolution Texture Pack (`map/` or `chr/` folder, or `.zip` archive) is present on disk.
    pub fn has_4k_texture_pack(&self) -> bool {
        self.resolve_4k_texture_pack_dir().is_some_and(|dir| {
            (dir.is_file() && dir.extension().is_some_and(|e| e == "zip"))
                || dir.join("map").is_dir()
                || dir.join("chr").is_dir()
                || dir.join("sh2_4k_textures.zip").is_file()
        })
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
}
