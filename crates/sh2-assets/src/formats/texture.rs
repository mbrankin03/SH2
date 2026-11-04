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
