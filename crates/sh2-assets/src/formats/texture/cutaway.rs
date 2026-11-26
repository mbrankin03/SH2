//! Cutaway still & overlay sprite sheet `.tex` loader, aspect-ratio viewport transform,
//! and PS2 `0xa7a7a7a7` GS-swizzled CLUT `.tex` decoder (`SH2-15`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AssetError;

use super::{decode_texture_bytes, DecodedTexture, TextureLoader, TEX_MAGIC};

/// Quality tier for resolving cutaway, puzzle, and map assets (`SH2-91`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutawayPackTier {
    /// 4K AI Super-Resolution Master Pack (`mods/4k_cutaways/`), falling back to `Sh2eeHd` -> `OriginalPs2`.
    Ai4kMaster,
    /// SH2:EE HD Pack (`sh2e/pic/`), falling back to `OriginalPs2`.
    #[default]
    Sh2eeHd,
    /// Original PS2 / Retail PC (`data/pic/`), falling back to `Sh2eeHd`.
    OriginalPs2,
}

/// Decoded cutaway still or overlay sprite sheet with original PS2/PC texel metadata.
#[derive(Debug, Clone)]
pub struct CutawayTextureAsset {
    /// Decoded RGBA8 texture (original PS2/PC, SH2:EE 4x upscaled, or 4K AI Master).
    pub texture: DecodedTexture,
    /// Original PS2/PC width in texels (used to normalize sprite sheet UV coordinates).
    pub original_width: u32,
    /// Original PS2/PC height in texels (used to normalize sprite sheet UV coordinates).
    pub original_height: u32,
    /// Whether this asset was loaded from the SH2:EE `sh2e/pic/` pack or 4K AI pack.
    pub is_sh2ee_hd: bool,
}

/// Viewport aspect-ratio and centering parameters for rendering 4:3 (`512x512` stretched to 4:3)
/// or widescreen SH2:EE cutaway stills inside an arbitrary output viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutawayViewportTransform {
    /// Normalized horizontal scale factor (`1.0` fills the full viewport width).
    pub scale_x: f32,
    /// Normalized vertical scale factor (`1.0` fills the full viewport height).
    pub scale_y: f32,
    /// Horizontal offset in pixels from the viewport's left edge.
    pub offset_x: f32,
    /// Vertical offset in pixels from the viewport's top edge.
    pub offset_y: f32,
    /// Active render width in pixels inside the viewport.
    pub draw_width: f32,
    /// Active render height in pixels inside the viewport.
    pub draw_height: f32,
}

impl CutawayTextureAsset {
    /// Normalizes pixel coordinates `(u_px, v_px)` authored against the original PS2/PC sprite
    /// sheet dimensions into `[0.0, 1.0]` UV space, working seamlessly across both original and
    /// SH2:EE 4x upscaled sprite sheets.
    pub fn normalize_uv(&self, u_px: f32, v_px: f32) -> (f32, f32) {
        let ow = (self.original_width.max(1)) as f32;
        let oh = (self.original_height.max(1)) as f32;
        (u_px / ow, v_px / oh)
    }

    /// Computes centering and aspect-ratio correction for displaying this still inside a viewport
    /// of `(viewport_w, viewport_h)`.
    ///
    /// Original PC `512x512` stills are authored to be displayed at a `4:3` aspect ratio
    /// (`640x480`), whereas SH2:EE widescreen stills with non-square dimensions use their native
    /// aspect ratio.
    pub fn compute_viewport_transform(
        &self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> CutawayViewportTransform {
        let vw = viewport_w.max(1.0);
        let vh = viewport_h.max(1.0);
        let target_aspect = if self.texture.width == self.texture.height {
            4.0 / 3.0
        } else {
            (self.texture.width as f32) / (self.texture.height.max(1) as f32)
        };
        let view_aspect = vw / vh;

        let (draw_width, draw_height) = if view_aspect > target_aspect {
            let dh = vh;
            let dw = dh * target_aspect;
            (dw, dh)
        } else {
            let dw = vw;
            let dh = dw / target_aspect;
            (dw, dh)
        };

        CutawayViewportTransform {
            scale_x: draw_width / vw,
            scale_y: draw_height / vh,
            offset_x: (vw - draw_width) * 0.5,
            offset_y: (vh - draw_height) * 0.5,
            draw_width,
            draw_height,
        }
    }
}

impl TextureLoader {
    /// Resolves a relative asset path with explicit SH2:EE vs original PS2/PC preference and
    /// automatic fallback if the asset only exists in one pack.
    pub fn resolve_path_with_preference(
        &self,
        relative_path: impl AsRef<Path>,
        prefer_sh2ee: bool,
    ) -> Option<(PathBuf, bool)> {
        let rel = relative_path.as_ref();
        let clean_rel = rel
            .strip_prefix("data")
            .or_else(|_| rel.strip_prefix("sh2e"))
            .unwrap_or(rel);

        if prefer_sh2ee {
            if let Some(p) = self.resolve_in_sh2e(clean_rel) {
                return Some((p, true));
            }
            if let Some(p) = self.resolve_in_data(clean_rel) {
                return Some((p, false));
            }
        } else {
            if let Some(p) = self.resolve_in_data(clean_rel) {
                return Some((p, false));
            }
            if let Some(p) = self.resolve_in_sh2e(clean_rel) {
                return Some((p, true));
            }
        }
        None
    }

    /// Converts a stage program picture identifier (e.g. `"out_p_incar"`, `"apt_clock_close"`,
    /// or `"out/p_incar_map"`) into a relative `.tex` path under `pic/` (e.g. `"pic/out/p_incar.tex"`).
    pub fn normalize_cutaway_rel_path(pic_or_sheet_name: &str) -> PathBuf {
        let trimmed = pic_or_sheet_name
            .trim()
            .trim_start_matches("data/")
            .trim_start_matches("sh2e/")
            .trim_start_matches("pic/")
            .trim_end_matches(".tex");

        let rel_body = if trimmed.contains('/') {
            trimmed.to_string()
        } else if let Some((dir, rest)) = trimmed.split_once('_') {
            format!("{dir}/{rest}")
        } else {
            trimmed.to_string()
        };

        Path::new("pic").join(format!("{rel_body}.tex"))
    }

    /// Resolves and decodes a pre-rendered cutaway still or puzzle overlay sprite sheet,
    /// supporting runtime switching between SH2:EE HD upscaled assets (`sh2e/pic`) and original
    /// PS2/PC (`data/pic`) textures while preserving original texel dimensions for UV normalization.
    pub fn load_cutaway_texture(
        &self,
        pic_or_sheet_name: &str,
        prefer_sh2ee: bool,
    ) -> Result<CutawayTextureAsset, AssetError> {
        self.load_cutaway_texture_for_quality(pic_or_sheet_name, prefer_sh2ee, false)
    }

    /// Resolves and decodes a pre-rendered cutaway still or puzzle overlay sprite sheet across
    /// all three `WorldTextureQuality` tiers (`OriginalPs2`, `Sh2eeHd`, `Ai4kMaster`) while
    /// preserving original PS2/PC texel dimensions for UV normalization (`SH2-142`).
    pub fn load_cutaway_texture_for_quality(
        &self,
        pic_or_sheet_name: &str,
        prefer_sh2ee: bool,
        apply_4k: bool,
    ) -> Result<CutawayTextureAsset, AssetError> {
        let tier = if apply_4k {
            CutawayPackTier::Ai4kMaster
        } else if prefer_sh2ee {
            CutawayPackTier::Sh2eeHd
        } else {
            CutawayPackTier::OriginalPs2
        };
        self.load_cutaway_texture_with_tier(pic_or_sheet_name, tier, None)
    }

    /// Resolves and decodes a cutaway still or overlay sprite sheet using the 3-tier
    /// [`CutawayPackTier`] (`Ai4kMaster`, `Sh2eeHd`, `OriginalPs2`) and optional `mods/sh2_4k_textures`
    /// or `mods/4k_cutaways` pack root (`SH2-91`), always preserving original PS2/PC texel dimensions
    /// for `-256..256` UV normalization.
    pub fn load_cutaway_texture_with_tier(
        &self,
        pic_or_sheet_name: &str,
        tier: CutawayPackTier,
        pack_4k_dir: Option<&Path>,
    ) -> Result<CutawayTextureAsset, AssetError> {
        let rel = Self::normalize_cutaway_rel_path(pic_or_sheet_name);

        if tier == CutawayPackTier::Ai4kMaster {
            if let Some(texture) = self.load_4k_relative_texture_override(&rel) {
                let (original_width, original_height) = self
                    .resolve_in_data(&rel)
                    .and_then(|vp| fs::read(vp).ok())
                    .and_then(|vb| probe_tex_dimensions(&vb))
                    .unwrap_or((
                        (texture.width as u32 / 8).max(1),
                        (texture.height as u32 / 8).max(1),
                    ));
                return Ok(CutawayTextureAsset {
                    original_width,
                    original_height,
                    is_sh2ee_hd: true,
                    texture,
                });
            }
        }

        if tier == CutawayPackTier::Ai4kMaster {
            let resolved_4k_root = pack_4k_dir
                .map(PathBuf::from)
                .or_else(|| {
                    self.data_dir
                        .parent()
                        .map(|r| r.join("mods/sh2_4k_textures"))
                        .filter(|p| p.join("pic").is_dir())
                })
                .or_else(|| {
                    self.data_dir
                        .parent()
                        .map(|r| r.join("mods/4k_cutaways"))
                        .filter(|p| p.join("pic").is_dir())
                })
                .or_else(|| {
                    crate::resolver::AssetPathResolver::resolve().resolve_4k_cutaway_pack_dir()
                });

            if let Some(root_4k) = resolved_4k_root {
                for ext in ["dds", "png"] {
                    let cand_img = root_4k.join(rel.with_extension(ext));
                    if cand_img.is_file() {
                        let bytes = fs::read(&cand_img)?;
                        let texture = decode_texture_bytes(&bytes, &cand_img)?;
                        let (original_width, original_height) = self
                            .resolve_in_data(&rel)
                            .and_then(|vp| fs::read(vp).ok())
                            .and_then(|vb| probe_tex_dimensions(&vb))
                            .unwrap_or((
                                (texture.width as u32 / 8).max(1),
                                (texture.height as u32 / 8).max(1),
                            ));
                        return Ok(CutawayTextureAsset {
                            original_width,
                            original_height,
                            is_sh2ee_hd: true,
                            texture,
                        });
                    }
                }
            }
        }

        let prefer_sh2ee = tier != CutawayPackTier::OriginalPs2;
        let (chosen_path, is_sh2ee_hd) = self
            .resolve_path_with_preference(&rel, prefer_sh2ee)
            .ok_or_else(|| {
                AssetError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Cutaway texture not found: {}", rel.display()),
                ))
            })?;

        let bytes = fs::read(&chosen_path)?;
        let base_texture = decode_texture_bytes(&bytes, &chosen_path)?;

        // Determine the original PS2/PC texel dimensions so sprite sheet UVs authored in original
        // texels (e.g. 256x256 or 512x512) normalize accurately even when an HD 4x or 4K AI sheet is active.
        let (original_width, original_height) = if is_sh2ee_hd {
            if let Some(van_path) = self.resolve_in_data(&rel) {
                if let Ok(van_bytes) = fs::read(&van_path) {
                    probe_tex_dimensions(&van_bytes)
                        .unwrap_or((base_texture.width as u32, base_texture.height as u32))
                } else {
                    (base_texture.width as u32, base_texture.height as u32)
                }
            } else {
                (
                    (base_texture.width as u32 / 4).max(1),
                    (base_texture.height as u32 / 4).max(1),
                )
            }
        } else {
            (base_texture.width as u32, base_texture.height as u32)
        };

        let texture = if tier == CutawayPackTier::Ai4kMaster {
            self.load_4k_relative_texture_override(&rel)
                .unwrap_or(base_texture)
        } else {
            base_texture
        };

        Ok(CutawayTextureAsset {
            original_width,
            original_height,
            is_sh2ee_hd,
            texture,
        })
    }
}

/// Probes the pixel dimensions `(width, height)` of a PC or PS2 `.tex` header without full decoding.
pub fn probe_tex_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 0x50 && &bytes[0x0c..0x10] == b"\xa7\xa7\xa7\xa7" {
        let w = u16::from_le_bytes([bytes[0x48], bytes[0x49]]) as u32;
        let h = u16::from_le_bytes([bytes[0x4a], bytes[0x4b]]) as u32;
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    if bytes.len() >= 0x40 {
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic == TEX_MAGIC {
            let w = u16::from_le_bytes([bytes[0x38], bytes[0x39]]) as u32;
            let h = u16::from_le_bytes([bytes[0x3a], bytes[0x3b]]) as u32;
            if w > 0 && h > 0 {
                return Some((w, h));
            }
        }
    }
    None
}

/// Unswizzles PS2 GS `PSMT8` 8-bit indices uploaded as 32-bit pixels.
pub(super) fn ps2_unswizzle8(buf: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        let block_y = (y & !0x0f) * w;
        let swap = (((y + 2) >> 2) & 1) * 4;
        let posy = (((y & !3) >> 1) + (y & 1)) & 7;
        let col_y = posy * w * 2;
        let byte_y = (y >> 1) & 1;
        for x in 0..w {
            let block = block_y + (x & !0x0f) * 2;
            let col = col_y + ((x + swap) & 7) * 4;
            let byte = byte_y + ((x >> 2) & 2);
            let src_idx = block + col + byte;
            if src_idx < buf.len() {
                out[y * w + x] = buf[src_idx];
            }
        }
    }
    out
}

/// Decodes a PS2-format `.tex` container (`bytes[0x0c..0x10] == b"\xa7\xa7\xa7\xa7"`),
/// supporting both 32-bit (`bpp == 0x18`) and GS-swizzled 8-bit (`PSMT8` + CSM1 CLUT) textures.
///
/// On PS2 Graphics Synthesizer (GS), RGB channels are already full-range `0..=255` while Alpha
/// uses `0x00..=0x80` (`0x80` = 1.0 opaque), so only the Alpha channel is doubled (`(a * 2).min(255)`).
pub(super) fn decode_ps2_tex(bytes: &[u8]) -> Result<DecodedTexture, AssetError> {
    if bytes.len() < 0xc0 {
        return Err(AssetError::BufferUnderflow {
            required: 0xc0,
            available: bytes.len(),
        });
    }
    let w = u16::from_le_bytes([bytes[0x48], bytes[0x49]]) as usize;
    let h = u16::from_le_bytes([bytes[0x4a], bytes[0x4b]]) as usize;
    let bpp = bytes[0x4c];
    if w == 0 || h == 0 {
        return Err(AssetError::InvalidData(
            "PS2 texture dimensions must be non-zero".to_string(),
        ));
    }

    let scale_ps2_alpha = |a: u8| -> u8 { ((a as u16) * 2).min(255) as u8 };

    if bpp == 0x18 {
        let required = 0xc0 + w * h * 4;
        if bytes.len() < required {
            return Err(AssetError::BufferUnderflow {
                required,
                available: bytes.len(),
            });
        }
        let mut rgba = vec![0u8; w * h * 4];
        let src = &bytes[0xc0..required];
        let (src_chunks, _) = src.as_chunks::<4>();
        let (dst_chunks, _) = rgba.as_chunks_mut::<4>();
        for (src_px, dst_px) in src_chunks.iter().zip(dst_chunks.iter_mut()) {
            dst_px[0] = src_px[0];
            dst_px[1] = src_px[1];
            dst_px[2] = src_px[2];
            dst_px[3] = scale_ps2_alpha(src_px[3]);
        }
        Ok(DecodedTexture {
            width: w,
            height: h,
            rgba,
        })
    } else {
        let clut_ofs = 0xc0 + w * h + 0x30;
        // The 16x16 CLUT sits in the top-left of a 64-wide 32-bit block (at least 16 rows of 64*4 bytes)
        let min_clut_bytes = 15 * 64 * 4 + 16 * 4;
        if bytes.len() < clut_ofs + min_clut_bytes {
            return Err(AssetError::BufferUnderflow {
                required: clut_ofs + min_clut_bytes,
                available: bytes.len(),
            });
        }
        let indices = ps2_unswizzle8(&bytes[0xc0..0xc0 + w * h], w, h);
        let raw_clut = &bytes[clut_ofs..];
        let mut pal = [[0u8; 4]; 256];
        for r in 0..16 {
            for c in 0..16 {
                let s_ofs = (r * 64 + c) * 4;
                let idx = r * 16 + c;
                pal[idx] = [
                    raw_clut[s_ofs],
                    raw_clut[s_ofs + 1],
                    raw_clut[s_ofs + 2],
                    scale_ps2_alpha(raw_clut[s_ofs + 3]),
                ];
            }
        }

        let mut rgba = vec![0u8; w * h * 4];
        for (i, &raw_idx) in indices.iter().enumerate() {
            let idx = raw_idx as usize;
            // CSM1 palette permutation
            let perm = (idx & !0x18) | ((idx & 8) << 1) | ((idx & 16) >> 1);
            let color = pal[perm];
            let d_ofs = i * 4;
            rgba[d_ofs..d_ofs + 4].copy_from_slice(&color);
        }
        Ok(DecodedTexture {
            width: w,
            height: h,
            rgba,
        })
    }
}
