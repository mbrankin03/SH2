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
    if bytes.len() >= 0x40 {
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic == TEX_MAGIC {
            let width = u16::from_le_bytes([bytes[0x38], bytes[0x39]]) as usize;
            let height = u16::from_le_bytes([bytes[0x3a], bytes[0x3b]]) as usize;
            let fmt = bytes[0x3c];
            let kind = match fmt {
                0x00 => 1,
                0x01 | 0x02 => 3,
                _ => 5,
            };
            if bytes.len() > 0x40 {
                let rgba = decode_dxt(&bytes[0x40..], width, height, kind)?;
                return Ok(DecodedTexture {
                    width,
                    height,
                    rgba,
                });
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
