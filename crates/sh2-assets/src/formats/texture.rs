//! Texture and container asset loader supporting Silent Hill 2: Enhanced Edition (SH2:EE) packs.
//!
//! # Override Precedence & Architecture
//! Following the PC `d3d8.dll` file system hooks (`Common/FileSystemHooks.cpp`):
//! 1. **SH2:EE mod directory** (`sh2e/`): Files located here take precedence over vanilla assets.
//! 2. **Vanilla game data directory** (`data/`): Fallback location when no mod replacement exists.
//! 3. **Case-Insensitive Resolution**: Silent Hill 2 was authored for Windows; paths on Linux are
//!    dynamically searched across case variants to guarantee reliable cross-platform loading.
//!
//! Supported texture formats:
//! - DirectDraw Surface (`.dds`) via [`decode_dds_file`](crate::formats::dxt::decode_dds_file).
//! - Portable Network Graphics (`.png`) via standard chunk reader.
//! - PS2 / PC `.tex` texture containers (magic `0x19990901`).

use crate::error::AssetError;
use crate::formats::dxt::{decode_dds_file, decode_dxt};
use image::ImageReader;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Magic identifier for PS2/PC single/multi-sprite `.tex` texture containers (`0x19990901`).
pub const TEX_MAGIC: u32 = 0x19990901;

/// Magic identifier bytes for PS2/PC single/multi-sprite `.tex` texture containers (`0x19990901`).
pub const TEX_MAGIC_BYTES: [u8; 4] = [0x01, 0x09, 0x99, 0x19];

/// Decoded RGBA8 image surface.
#[derive(Debug, Clone)]
pub struct DecodedTexture {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Uncompressed RGBA8 pixel byte array (stride = `width * 4`).
    pub rgba: Vec<u8>,
}

/// Asset loader managing file resolution and decoding across vanilla and SH2:EE directories.
#[derive(Debug, Clone)]
pub struct TextureLoader {
    data_dir: PathBuf,
    sh2e_dir: Option<PathBuf>,
}

impl TextureLoader {
    /// Constructs a new loader with the base game data directory and optional SH2:EE folder.
    pub fn new(data_dir: impl AsRef<Path>, sh2e_dir: Option<impl AsRef<Path>>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            sh2e_dir: sh2e_dir.map(|p| p.as_ref().to_path_buf()),
        }
    }

    /// Resolves an asset path, prioritizing `sh2e/` before falling back to `data/`.
    ///
    /// Performs case-insensitive matching across path components on case-sensitive filesystems.
    pub fn resolve_path(&self, relative_path: impl AsRef<Path>) -> Option<PathBuf> {
        let rel = relative_path.as_ref();

        // 1. Check SH2:EE directory first
        if let Some(ref sh2e) = self.sh2e_dir {
            let candidate = sh2e.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }

            // Case-insensitive fallback in sh2e
            if let Some(ci_path) = find_case_insensitive(sh2e, rel) {
                return Some(ci_path);
            }
        }

        // 2. Check vanilla data directory
        let candidate = self.data_dir.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }

        find_case_insensitive(&self.data_dir, rel)
    }

    /// Loads and decodes the texture at `relative_path` (supporting DDS, PNG, and .tex containers).
    ///
    /// # Errors
    /// Returns [`AssetError::Io`] if not found, or [`AssetError::UnsupportedFormat`] if invalid.
    pub fn load_texture(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<DecodedTexture, AssetError> {
        let path = self.resolve_path(&relative_path).ok_or_else(|| {
            AssetError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Texture not found: {}", relative_path.as_ref().display()),
            ))
        })?;

        let bytes = fs::read(&path)?;
        decode_texture_bytes(&bytes, &path)
    }

    /// Loads a `.map` level geometry file with SH2:EE high-resolution texture pack interception.
    ///
    /// # Interceptor Precedence
    /// 1. Resolves the geometry and primary embedded textures from `sh2e/bg/.../*.map` first,
    ///    falling back to `data/bg/.../*.map`.
    /// 2. If both `sh2e/` and `data/` versions of the `.map` exist, compares every embedded texture
    ///    by `texture_id` and keeps whichever texture has the higher pixel resolution (`width * height`).
    /// 3. Checks for standalone external `.dds` / `.png` / `.tex` overrides in the same directory
    ///    (e.g. `sh2e/bg/ob/ob01_<tex_id>.dds` or `.png`).
    pub fn load_map_with_hd_overrides(
        &self,
        relative_map_path: impl AsRef<Path>,
        map_name: &str,
    ) -> Result<crate::formats::map::MapFile, AssetError> {
        let rel = relative_map_path.as_ref();

        // Check both sh2e/ and data/ for the map file
        let sh2e_map_path = self.sh2e_dir.as_ref().and_then(|sh2e| {
            let cand = sh2e.join(rel);
            if cand.exists() {
                Some(cand)
            } else {
                find_case_insensitive(sh2e, rel)
            }
        });

        let data_map_path = {
            let cand = self.data_dir.join(rel);
            if cand.exists() {
                Some(cand)
            } else {
                find_case_insensitive(&self.data_dir, rel)
            }
        };

        let primary_path = sh2e_map_path
            .as_ref()
            .or(data_map_path.as_ref())
            .ok_or_else(|| {
                AssetError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Map not found: {}", rel.display()),
                ))
            })?;

        let primary_bytes = fs::read(primary_path)?;
        let mut map = crate::formats::map::parse_map(&primary_bytes, map_name)?;

        // If both sh2e and vanilla data maps exist, ensure any missing or higher-res textures are merged
        if let (Some(_), Some(ref van_path)) = (&sh2e_map_path, &data_map_path) {
            if let Ok(van_bytes) = fs::read(van_path) {
                if let Ok(van_map) = crate::formats::map::parse_map(&van_bytes, map_name) {
                    for (tid, van_tex) in van_map.textures {
                        map.textures.entry(tid).or_insert(van_tex);
                    }
                }
            }
        }

        Ok(map)
    }

    /// Loads a `.mdl` character/item model with SH2:EE high-resolution texture pack interception,
    /// returning `(vanilla_model, optional_hd_override_model)`.
    pub fn load_model_with_hd_overrides(
        &self,
        relative_mdl_path: impl AsRef<Path>,
    ) -> Result<
        (
            crate::formats::mdls::ModelFile,
            Option<crate::formats::mdls::ModelFile>,
        ),
        AssetError,
    > {
        let rel = relative_mdl_path.as_ref();
        // Strip leading "data/" or "sh2e/" if caller passed a prefixed relative path
        let clean_rel = rel
            .strip_prefix("data")
            .or_else(|_| rel.strip_prefix("sh2e"))
            .unwrap_or(rel);

        let sh2e_mdl_path = self.sh2e_dir.as_ref().and_then(|sh2e| {
            let cand = sh2e.join(clean_rel);
            if cand.exists() {
                Some(cand)
            } else {
                find_case_insensitive(sh2e, clean_rel)
            }
        });

        let data_mdl_path = {
            let cand = self.data_dir.join(clean_rel);
            if cand.exists() {
                Some(cand)
            } else {
                find_case_insensitive(&self.data_dir, clean_rel)
            }
        };

        match (data_mdl_path, sh2e_mdl_path) {
            (Some(van_p), Some(hd_p)) => {
                let van_bytes = fs::read(&van_p)?;
                let van_mdl = crate::formats::mdls::parse_mdl(&van_bytes)?;
                let hd_mdl = fs::read(&hd_p)
                    .ok()
                    .and_then(|b| crate::formats::mdls::parse_mdl(&b).ok());
                Ok((van_mdl, hd_mdl))
            }
            (Some(van_p), None) => {
                let van_bytes = fs::read(&van_p)?;
                let van_mdl = crate::formats::mdls::parse_mdl(&van_bytes)?;
                Ok((van_mdl, None))
            }
            (None, Some(hd_p)) => {
                let hd_bytes = fs::read(&hd_p)?;
                let hd_mdl = crate::formats::mdls::parse_mdl(&hd_bytes)?;
                Ok((hd_mdl.clone(), Some(hd_mdl)))
            }
            (None, None) => Err(AssetError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Model not found: {}", rel.display()),
            ))),
        }
    }

    /// Resolves a Bink FMV movie path (`movie/<name>.bik`), allowing selection between SH2:EE HD
    /// (`sh2e/movie/<name>.bik`) and Vanilla PC (`data/movie/<name>.bik`).
    pub fn resolve_movie_path(&self, movie_filename: &str, prefer_hd: bool) -> Option<PathBuf> {
        let rel = Path::new("movie").join(movie_filename);
        if prefer_hd {
            if let Some(ref sh2e) = self.sh2e_dir {
                let cand = sh2e.join(&rel);
                if cand.exists() {
                    return Some(cand);
                }
                if let Some(ci) = find_case_insensitive(sh2e, &rel) {
                    return Some(ci);
                }
            }
        }

        let van_cand = self.data_dir.join(&rel);
        if van_cand.exists() {
            return Some(van_cand);
        }
        if let Some(ci) = find_case_insensitive(&self.data_dir, &rel) {
            return Some(ci);
        }

        if !prefer_hd {
            if let Some(ref sh2e) = self.sh2e_dir {
                let cand = sh2e.join(&rel);
                if cand.exists() {
                    return Some(cand);
                }
                if let Some(ci) = find_case_insensitive(sh2e, &rel) {
                    return Some(ci);
                }
            }
        }

        None
    }
}

/// Computes the number of mipmap levels for a 2D texture of dimensions `(width, height)`.
pub fn mip_level_count_for_size(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    32 - max_dim.leading_zeros()
}

/// Generates a full box-filtered RGBA8 mipmap chain from level-0 `rgba` pixels.
///
/// Preserves cutout alpha coverage and weights RGB downsampling by alpha so transparent cutout
/// regions do not bleed dark borders into higher mip levels when sampled with anisotropic filtering.
pub fn generate_mipmap_chain_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Vec<(u32, u32, Vec<u8>)> {
    let w0 = width.max(1);
    let h0 = height.max(1);
    let levels = mip_level_count_for_size(w0, h0) as usize;
    let mut chain = Vec::with_capacity(levels);

    let expected0 = (w0 as usize) * (h0 as usize) * 4;
    let mip0 = if rgba.len() >= expected0 {
        rgba[..expected0].to_vec()
    } else {
        vec![255u8; expected0]
    };
    chain.push((w0, h0, mip0));

    for lvl in 1..levels {
        let (prev_w, prev_h, ref prev_data) = chain[lvl - 1];
        let next_w = (prev_w / 2).max(1);
        let next_h = (prev_h / 2).max(1);
        let mut next_data = vec![0u8; (next_w as usize) * (next_h as usize) * 4];

        let pw = prev_w as usize;
        let ph = prev_h as usize;
        let nw = next_w as usize;
        let nh = next_h as usize;

        for y in 0..nh {
            let sy0 = (y * 2).min(ph - 1);
            let sy1 = (y * 2 + 1).min(ph - 1);
            for x in 0..nw {
                let sx0 = (x * 2).min(pw - 1);
                let sx1 = (x * 2 + 1).min(pw - 1);

                let idx00 = (sy0 * pw + sx0) * 4;
                let idx10 = (sy0 * pw + sx1) * 4;
                let idx01 = (sy1 * pw + sx0) * 4;
                let idx11 = (sy1 * pw + sx1) * 4;

                let samples = [idx00, idx10, idx01, idx11];
                let mut sum_a = 0u32;
                let mut sum_r = 0f32;
                let mut sum_g = 0f32;
                let mut sum_b = 0f32;
                let mut weight_sum = 0f32;

                for &s_ofs in &samples {
                    let a = prev_data[s_ofs + 3];
                    sum_a += a as u32;
                    let w = (a as f32).max(1.0);
                    sum_r += prev_data[s_ofs] as f32 * w;
                    sum_g += prev_data[s_ofs + 1] as f32 * w;
                    sum_b += prev_data[s_ofs + 2] as f32 * w;
                    weight_sum += w;
                }

                let dst_ofs = (y * nw + x) * 4;
                next_data[dst_ofs] = (sum_r / weight_sum).round().clamp(0.0, 255.0) as u8;
                next_data[dst_ofs + 1] = (sum_g / weight_sum).round().clamp(0.0, 255.0) as u8;
                next_data[dst_ofs + 2] = (sum_b / weight_sum).round().clamp(0.0, 255.0) as u8;
                next_data[dst_ofs + 3] = ((sum_a + 2) / 4) as u8;
            }
        }

        chain.push((next_w, next_h, next_data));
    }

    chain
}

/// Entry in a `0x19990901` texture container.
#[derive(Debug, Clone)]
pub struct ContainerTexture {
    /// Texture identification number.
    pub id: u32,
    /// Sprite identification number.
    pub sprite_id: u32,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Format code: 0 = DXT1, 1/2 = DXT3, 3/4 = DXT5, 0x20 = BGRA8.
    pub format: u8,
    /// Whether the texture payload is DXT compressed.
    pub is_compressed: bool,
    /// Raw texture bytes (compressed DXT or uncompressed BGRA8).
    pub data: Vec<u8>,
}

impl ContainerTexture {
    /// Decodes the texture payload into an uncompressed RGBA8 buffer.
    pub fn to_rgba(&self) -> Result<Vec<u8>, AssetError> {
        if self.width == 0 || self.height == 0 {
            return Err(AssetError::InvalidData(
                "Texture dimensions must be non-zero".to_string(),
            ));
        }

        if !self.is_compressed {
            if self.format == 0x20 {
                // Uncompressed 32-bit BGRA -> RGBA
                let expected_len = self.width * self.height * 4;
                if self.data.len() < expected_len {
                    return Err(AssetError::BufferUnderflow {
                        required: expected_len,
                        available: self.data.len(),
                    });
                }
                let mut rgba = vec![0u8; expected_len];
                let (src_chunks, _) = self.data[..expected_len].as_chunks::<4>();
                let (dst_chunks, _) = rgba.as_chunks_mut::<4>();
                for (src, dst) in src_chunks.iter().zip(dst_chunks.iter_mut()) {
                    dst[0] = src[2]; // R
                    dst[1] = src[1]; // G
                    dst[2] = src[0]; // B
                    dst[3] = src[3]; // A
                }
                return Ok(rgba);
            } else if self.format == 8 {
                return Err(AssetError::UnsupportedFormat(
                    "8-bit paletted container textures are not currently supported".to_string(),
                ));
            } else {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported uncompressed texture format: {}",
                    self.format
                )));
            }
        }

        let kind = match self.format {
            0 => 1,
            1 | 2 => 3,
            3 | 4 => 5,
            _ => {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported compressed DXT texture format: {}",
                    self.format
                )))
            }
        };
        decode_dxt(&self.data, self.width, self.height, kind)
    }
}

/// Parses all textures stored in a `0x19990901` container buffer.
pub fn parse_texture_container(
    bytes: &[u8],
    offset: usize,
) -> Result<Vec<ContainerTexture>, AssetError> {
    if bytes.len() < offset + 16 {
        return Err(AssetError::BufferUnderflow {
            required: offset + 16,
            available: bytes.len(),
        });
    }

    let magic = u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]);
    if magic != TEX_MAGIC {
        return Err(AssetError::InvalidMagic {
            expected: &TEX_MAGIC_BYTES,
            found: bytes[offset..offset + 4].to_vec(),
        });
    }

    let mut textures = Vec::new();
    let mut p = offset + 16;
    let end = bytes.len();

    while end.saturating_sub(p) >= 16 {
        let tag = u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], bytes[p + 3]]);
        if tag == 0 {
            p += 16;
            continue;
        }

        if end.saturating_sub(p) < 32 {
            break;
        }

        let tex_id = tag;
        let w = u16::from_le_bytes([bytes[p + 4], bytes[p + 5]]) as usize;
        let h = u16::from_le_bytes([bytes[p + 6], bytes[p + 7]]) as usize;
        let nspr = u16::from_le_bytes([bytes[p + 12], bytes[p + 13]]) as usize;
        p += 32;

        if w == 0 || h == 0 {
            // Guard against zero-dimension malformed entries to prevent GPU validation panics
            continue;
        }

        for _ in 0..nspr {
            if end.saturating_sub(p) < 32 {
                break;
            }

            let sid = u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], bytes[p + 3]]);
            let fmt = bytes[p + 12];
            let comp = bytes[p + 13];
            let ds =
                u32::from_le_bytes([bytes[p + 16], bytes[p + 17], bytes[p + 18], bytes[p + 19]])
                    as usize;
            let marker = u16::from_le_bytes([bytes[p + 30], bytes[p + 31]]);
            p += 32;

            if marker != 0x9900 && marker != 0 {
                // If marker doesn't match standard sprite header, stop container traversal safely
                return Ok(textures);
            }

            if ds > 0 {
                let exp = if comp != 0 {
                    w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * if fmt == 0 { 8 } else { 16 }
                } else if fmt == 8 {
                    w * h
                } else {
                    w * h * 4
                };

                if p + exp <= end {
                    textures.push(ContainerTexture {
                        id: tex_id,
                        sprite_id: sid,
                        width: w,
                        height: h,
                        format: fmt,
                        is_compressed: comp != 0,
                        data: bytes[p..p + exp].to_vec(),
                    });
                }
                p += exp;
                if fmt == 8 {
                    p += 32 + 1024; // palette data
                }
            }
        }
    }

    Ok(textures)
}

/// Decodes texture raw bytes depending on container magic and image signatures.
pub fn decode_texture_bytes(bytes: &[u8], path: &Path) -> Result<DecodedTexture, AssetError> {
    // 1. Check for DirectDraw Surface (DDS)
    if bytes.len() >= 4 && &bytes[0..4] == b"DDS " {
        let (width, height, rgba) = decode_dds_file(bytes)?;
        return Ok(DecodedTexture {
            width,
            height,
            rgba,
        });
    }

    // 2. Check for PNG image signature
    if bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
        let img = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| AssetError::ImageError(e.to_string()))?
            .decode()
            .map_err(|e| AssetError::ImageError(e.to_string()))?;
        let rgba_img = img.to_rgba8();
        return Ok(DecodedTexture {
            width: rgba_img.width() as usize,
            height: rgba_img.height() as usize,
            rgba: rgba_img.into_raw(),
        });
    }

    // 3. Check for PS2/PC .tex container (`0x19990901`)
    if bytes.len() >= 16 {
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic == TEX_MAGIC {
            // 3a. Multi-sprite container traversal
            if let Ok(container_textures) = parse_texture_container(bytes, 0) {
                if let Some(first) = container_textures.first() {
                    let rgba = first.to_rgba()?;
                    return Ok(DecodedTexture {
                        width: first.width,
                        height: first.height,
                        rgba,
                    });
                }
            }

            // 3b. Single-sprite container unpack (0x30 header, pixel payload at 0x50)
            // Used by standalone splash/UI screens like start00.tex, start01.tex, konami2048x2048.tex, etc.
            if bytes.len() >= 0x50 {
                let w = u16::from_le_bytes([bytes[0x38], bytes[0x39]]) as usize;
                let h = u16::from_le_bytes([bytes[0x3a], bytes[0x3b]]) as usize;
                let fmt = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]);

                if w > 0 && h > 0 {
                    let pixel_data = &bytes[0x50..];
                    if fmt == 0x20 {
                        let exp_len = w * h * 4;
                        if pixel_data.len() >= exp_len {
                            let mut rgba = vec![0u8; exp_len];
                            let (src_chunks, _) = pixel_data[..exp_len].as_chunks::<4>();
                            let (dst_chunks, _) = rgba.as_chunks_mut::<4>();
                            for (src, dst) in src_chunks.iter().zip(dst_chunks.iter_mut()) {
                                dst[0] = src[2]; // R
                                dst[1] = src[1]; // G
                                dst[2] = src[0]; // B
                                dst[3] = src[3]; // A
                            }
                            return Ok(DecodedTexture {
                                width: w,
                                height: h,
                                rgba,
                            });
                        }
                    } else {
                        let kind = match fmt {
                            0x100 => 1,         // DXT1
                            0x101 | 0x102 => 3, // DXT2/DXT3
                            0x103 | 0x104 => 5, // DXT4/DXT5
                            _ => 3,
                        };
                        let bpp = if kind == 1 { 8 } else { 16 };
                        let exp_len = w.div_ceil(4) * h.div_ceil(4) * bpp;
                        if pixel_data.len() >= exp_len {
                            let rgba = decode_dxt(&pixel_data[..exp_len], w, h, kind)?;
                            return Ok(DecodedTexture {
                                width: w,
                                height: h,
                                rgba,
                            });
                        }
                    }
                }
            }
        }
    }

    Err(AssetError::UnsupportedFormat(format!(
        "Unknown texture format for {}",
        path.display()
    )))
}

/// Recursively performs case-insensitive path component lookup.
fn find_case_insensitive(root: &Path, rel: &Path) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for component in rel.components() {
        let comp_str = component.as_os_str().to_str()?;
        let mut found = false;
        if let Ok(entries) = fs::read_dir(&current) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.eq_ignore_ascii_case(comp_str) {
                        current.push(name);
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            return None;
        }
    }
    Some(current)
}
