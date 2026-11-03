use crate::error::AssetError;
use crate::formats::dxt::{decode_dds_file, decode_dxt};
use image::ImageReader;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub const TEX_MAGIC: u32 = 0x19990901;

#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TextureLoader {
    data_dir: PathBuf,
    sh2e_dir: Option<PathBuf>,
}

impl TextureLoader {
    pub fn new(data_dir: impl AsRef<Path>, sh2e_dir: Option<impl AsRef<Path>>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            sh2e_dir: sh2e_dir.map(|p| p.as_ref().to_path_buf()),
        }
    }

    /// Resolves file path checking the SH2:EE override directory first, falling back to vanilla data.
    pub fn resolve_path(&self, relative_path: impl AsRef<Path>) -> Option<PathBuf> {
        let rel = relative_path.as_ref();

        // Check SH2:EE directory first
        if let Some(ref sh2e) = self.sh2e_dir {
            let candidate = sh2e.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }

            // Case-insensitive fallback
            if let Some(ci_path) = find_case_insensitive(sh2e, rel) {
                return Some(ci_path);
            }
        }

        // Vanilla data directory
        let candidate = self.data_dir.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }

        find_case_insensitive(&self.data_dir, rel)
    }

    /// Loads and decodes texture at relative path (supports DDS, PNG, and .tex containers).
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

/// Decodes texture raw bytes depending on magic and extension.
pub fn decode_texture_bytes(bytes: &[u8], path: &Path) -> Result<DecodedTexture, AssetError> {
    if bytes.len() >= 4 && &bytes[0..4] == b"DDS " {
        let (width, height, rgba) = decode_dds_file(bytes)?;
        return Ok(DecodedTexture {
            width,
            height,
            rgba,
        });
    }

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

    // Check for PS2/PC .tex container
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
