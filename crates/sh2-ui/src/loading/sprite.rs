//! Retail `data/etc/effect/footmark.tbn2` texture loader (`sh2pc.exe` `FUN_004c8500`)
//! and procedural 64×64 sole silhouette fallback for CI environments without retail assets.

use sh2_assets::formats::texture::{parse_texture_container, TextureLoader};
use sh2_assets::resolver::AssetPathResolver;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Static 4K AI Master footstep sprite (`mods/sh2_4k_textures/effect/footmark.dds` -> SH2:EE 256×256 -> vanilla 64×64 fallback).
static FOOTSTEP_SPRITE_4K: OnceLock<FootstepSprite> = OnceLock::new();
/// Static HD-preferred footstep sprite (`sh2e/effect/footmark.tbn2` 256×256 -> vanilla 64×64 fallback).
static FOOTSTEP_SPRITE_HD: OnceLock<FootstepSprite> = OnceLock::new();
/// Static strict 1:1 Vanilla PS2/PC footstep sprite (`data/etc/effect/footmark.tbn2` 64×64).
static FOOTSTEP_SPRITE_VANILLA: OnceLock<FootstepSprite> = OnceLock::new();

/// Pre-decoded footstep RGBA texture sprite (`64×64` vanilla `data/etc/effect/footmark.tbn2`
/// or `256×256` SH2:EE HD `sh2e/effect/footmark.tbn2`).
///
/// `sh2pc.exe` `FUN_004c87e0` (`0x004c87e0`) renders `footmark.tbn2` with
/// `D3DTSS_COLORARG1 = D3DTA_TEXTURE`, `D3DTSS_ALPHAARG1 = D3DTA_TEXTURE`,
/// `D3DRS_SRCBLEND = D3DBLEND_SRCALPHA (5)`, `D3DRS_DESTBLEND = D3DBLEND_INVSRCALPHA (6)`.
/// Storing full RGBA preserves the retail boot-sole tread shading in RGB alongside the alpha mask.
pub struct FootstepSprite {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// `true` when loaded from retail `footmark.tbn2`, `false` when using the CI fallback.
    pub from_retail_asset: bool,
    /// `true` when loaded from the SH2:EE HD `sh2e/effect/footmark.tbn2` (256×256) override.
    pub is_hd_override: bool,
}

impl FootstepSprite {
    /// Loads the 4K AI Master footstep sprite (`mods/sh2_4k_textures/effect/footmark.dds` 1024×1024 when present,
    /// falling back to SH2:EE HD `sh2e/effect/footmark.tbn2` 256×256, then vanilla `data/etc/effect/footmark.tbn2` 64×64).
    pub fn get_4k() -> &'static Self {
        FOOTSTEP_SPRITE_4K.get_or_init(|| {
            if let Some(sprite) = Self::load_from_4k_pack() {
                return sprite;
            }
            if let Some(sprite) = Self::load_from_retail_assets_with_hd(true) {
                return sprite;
            }
            Self::generate_procedural_fallback()
        })
    }

    /// Selects the footstep sprite for the active `WorldTextureQuality` tier (`SH2-142`):
    /// - `prefer_sh2ee == false`: Strict 1:1 Vanilla (`data/etc/effect/footmark.tbn2` 64×64).
    /// - `prefer_sh2ee == true && !apply_4k`: SH2:EE HD (`sh2e/effect/footmark.tbn2` 256×256).
    /// - `prefer_sh2ee == true && apply_4k`: 4K AI Master (`mods/sh2_4k_textures/effect/footmark.dds`).
    pub fn get_for_quality(prefer_sh2ee: bool, apply_4k: bool) -> &'static Self {
        if !prefer_sh2ee {
            Self::get_vanilla()
        } else if apply_4k {
            Self::get_4k()
        } else {
            Self::get()
        }
    }

    /// Attempts to load `effect/footmark.dds` from the 4K AI Master pack (`mods/sh2_4k_textures/`).
    pub fn load_from_4k_pack() -> Option<Self> {
        let resolver = AssetPathResolver::resolve();
        let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
        let decoded = loader.load_4k_relative_texture_override("effect/footmark.tbn2")?;
        let width = decoded.width as u32;
        let height = decoded.height as u32;
        if width == 0 || height == 0 || decoded.rgba.len() != (width * height * 4) as usize {
            return None;
        }
        Some(Self {
            width,
            height,
            rgba: decoded.rgba,
            from_retail_asset: true,
            is_hd_override: true,
        })
    }

    /// Loads the HD-preferred footstep sprite (`sh2e/effect/footmark.tbn2` 256×256 when present,
    /// falling back to vanilla `data/etc/effect/footmark.tbn2` 64×64, then procedural CI fallback).
    pub fn get() -> &'static Self {
        FOOTSTEP_SPRITE_HD.get_or_init(|| {
            if let Some(sprite) = Self::load_from_retail_assets_with_hd(true) {
                return sprite;
            }
            Self::generate_procedural_fallback()
        })
    }

    /// Loads the strict 1:1 Vanilla footstep sprite (`data/etc/effect/footmark.tbn2` 64×64).
    pub fn get_vanilla() -> &'static Self {
        FOOTSTEP_SPRITE_VANILLA.get_or_init(|| {
            if let Some(sprite) = Self::load_from_retail_assets_with_hd(false) {
                return sprite;
            }
            Self::generate_procedural_fallback()
        })
    }

    /// Selects either the SH2:EE HD 256×256 sprite (`prefer_hd = true`) or the strict 1:1 Vanilla
    /// 64×64 sprite (`prefer_hd = false`).
    pub fn get_for_hd(prefer_hd: bool) -> &'static Self {
        if prefer_hd {
            Self::get()
        } else {
            Self::get_vanilla()
        }
    }

    /// Attempts to resolve and decode `footmark.tbn2` preferring SH2:EE HD (`sh2e/effect/footmark.tbn2`).
    pub fn load_from_retail_assets() -> Option<Self> {
        Self::load_from_retail_assets_with_hd(true)
    }

    /// Attempts to resolve and decode `footmark.tbn2` (`sh2pc.exe` `FUN_004c8500`).
    /// When `prefer_hd` is `true`, checks `sh2e/effect/footmark.tbn2` (256×256) first;
    /// when `false`, loads strict 1:1 vanilla `data/etc/effect/footmark.tbn2` (64×64).
    pub fn load_from_retail_assets_with_hd(prefer_hd: bool) -> Option<Self> {
        let resolver = AssetPathResolver::resolve();
        let mut tbn2_path: Option<PathBuf> = None;

        if prefer_hd {
            // `AssetPathResolver::resolve_file` checks `sh2e_dir` first, so `effect/footmark.tbn2`
            // resolves to `sh2e/effect/footmark.tbn2` (256×256 SH2:EE HD) when installed.
            for rel in &[
                "effect/footmark.tbn2",
                "etc/effect/footmark.tbn2",
                "pic/effect/footmark.tbn2",
            ] {
                if let Some(path) = resolver.resolve_file(rel) {
                    tbn2_path = Some(path);
                    break;
                }
            }
        } else {
            // Strict 1:1 Vanilla PS2/PC: read directly from `data_dir`, skipping `sh2e` overrides.
            for rel in &[
                "etc/effect/footmark.tbn2",
                "pic/effect/footmark.tbn2",
                "effect/footmark.tbn2",
            ] {
                let cand = resolver.data_dir.join(rel);
                if cand.is_file() {
                    tbn2_path = Some(cand);
                    break;
                }
            }
        }

        if tbn2_path.is_none() {
            let direct_hd = [
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/effect/footmark.tbn2",
            ];
            let direct_vanilla = [
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
            ];
            let list: &[&str] = if prefer_hd { &direct_hd } else { &[] };
            for cand in list.iter().chain(direct_vanilla.iter()) {
                let p = Path::new(cand);
                if p.is_file() {
                    tbn2_path = Some(p.to_path_buf());
                    break;
                }
            }
        }

        let path = tbn2_path?;
        let is_hd_override = path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
            .contains("/sh2e/");
        let bytes = std::fs::read(&path).ok()?;
        let textures = parse_texture_container(&bytes, 0).ok()?;
        let tex = textures.first()?;
        let rgba = tex.to_rgba().ok()?;

        let width = tex.width as u32;
        let height = tex.height as u32;
        if rgba.len() != (width * height * 4) as usize {
            return None;
        }

        Some(Self {
            width,
            height,
            rgba,
            from_retail_asset: true,
            is_hd_override,
        })
    }

    /// Procedural 64×64 sole fallback used only when `footmark.tbn2` is not present on disk.
    fn generate_procedural_fallback() -> Self {
        let width = 64u32;
        let height = 64u32;
        let mut rgba = vec![0u8; (width * height * 4) as usize];

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
                    let idx = ((y * width + x) * 4) as usize;
                    let a = (intensity * 255.0).clamp(0.0, 255.0) as u8;
                    rgba[idx] = 160;
                    rgba[idx + 1] = 160;
                    rgba[idx + 2] = 160;
                    rgba[idx + 3] = a;
                }
            }
        }

        Self {
            width,
            height,
            rgba,
            from_retail_asset: false,
            is_hd_override: false,
        }
    }
}
