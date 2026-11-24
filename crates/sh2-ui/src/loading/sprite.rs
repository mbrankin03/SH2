//! Retail `data/etc/effect/footmark.tbn2` texture loader (`sh2pc.exe` `FUN_004c8500`)
//! and procedural 64×64 sole silhouette fallback for CI environments without retail assets.

use sh2_assets::formats::texture::parse_texture_container;
use sh2_assets::resolver::AssetPathResolver;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Static footstep sprite loaded from `data/etc/effect/footmark.tbn2` (`FUN_004c8500`).
static FOOTSTEP_SPRITE: OnceLock<FootstepSprite> = OnceLock::new();

/// Pre-decoded footstep alpha silhouette sprite (`64×64` from `footmark.tbn2`).
pub struct FootstepSprite {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
    /// `true` when loaded from retail `footmark.tbn2`, `false` when using the CI fallback.
    pub from_retail_asset: bool,
}

impl FootstepSprite {
    /// Loads `data/etc/effect/footmark.tbn2` (`sh2pc.exe` `FUN_004c8500`), falling back to a
    /// procedural 64×64 sole silhouette only when retail assets are absent (e.g. CI).
    pub fn get() -> &'static Self {
        FOOTSTEP_SPRITE.get_or_init(|| {
            if let Some(sprite) = Self::load_from_retail_assets() {
                return sprite;
            }
            Self::generate_procedural_fallback()
        })
    }

    /// Attempts to resolve and decode `data/etc/effect/footmark.tbn2` (`sh2pc.exe` `FUN_004c8500`).
    pub fn load_from_retail_assets() -> Option<Self> {
        let resolver = AssetPathResolver::resolve();
        // Exact retail path first: `data/etc/effect/footmark.tbn2` (`FUN_004c8500`).
        let candidate_rel_paths = [
            "etc/effect/footmark.tbn2",
            "pic/effect/footmark.tbn2",
            "effect/footmark.tbn2",
        ];

        let mut tbn2_path: Option<PathBuf> = None;
        for rel in &candidate_rel_paths {
            if let Some(path) = resolver.resolve_file(rel) {
                tbn2_path = Some(path);
                break;
            }
        }

        if tbn2_path.is_none() {
            let direct_candidates = [
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
            ];
            for cand in &direct_candidates {
                let p = Path::new(cand);
                if p.is_file() {
                    tbn2_path = Some(p.to_path_buf());
                    break;
                }
            }
        }

        let path = tbn2_path?;
        let bytes = std::fs::read(&path).ok()?;
        let textures = parse_texture_container(&bytes, 0).ok()?;
        let tex = textures.first()?;
        let rgba = tex.to_rgba().ok()?;

        let width = tex.width as u32;
        let height = tex.height as u32;
        let mut alpha = Vec::with_capacity((width * height) as usize);
        let (chunks, _) = rgba.as_chunks::<4>();
        for chunk in chunks {
            alpha.push(chunk[3]);
        }

        Some(Self {
            width,
            height,
            alpha,
            from_retail_asset: true,
        })
    }

    /// Procedural 64×64 sole fallback used only when `footmark.tbn2` is not present on disk.
    fn generate_procedural_fallback() -> Self {
        let width = 64u32;
        let height = 64u32;
        let mut alpha = vec![0u8; (width * height) as usize];

        for y in 0..height {
            let ny = y as f32 / height as f32;
            for x in 0..width {
                let nx = (x as f32 - 32.0) / 32.0;
                let mut intensity = 0.0f32;
                if (0.08..=0.52).contains(&ny) {
                    let dx = nx / 0.36;
                    let dy = (ny - 0.30) / 0.22;
                    let d = dx * dx + dy * dy;
                    if d <= 1.0 {
                        intensity = 1.0 - ((d - 0.75) / 0.25).clamp(0.0, 1.0);
                    }
                } else if (0.52..=0.66).contains(&ny) && (-0.28..=0.14).contains(&nx) {
                    intensity = 1.0;
                } else if (0.66..=0.92).contains(&ny) {
                    let dx = nx / 0.28;
                    let dy = (ny - 0.79) / 0.13;
                    let d = dx * dx + dy * dy;
                    if d <= 1.0 {
                        intensity = 1.0 - ((d - 0.75) / 0.25).clamp(0.0, 1.0);
                    }
                }
                if intensity > 0.0 {
                    alpha[(y * width + x) as usize] = (intensity * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }

        Self {
            width,
            height,
            alpha,
            from_retail_asset: false,
        }
    }
}
