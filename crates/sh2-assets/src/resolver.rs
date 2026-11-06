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
