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

mod cutaway;
use cutaway::decode_ps2_tex;
pub use cutaway::{probe_tex_dimensions, CutawayTextureAsset, CutawayViewportTransform};

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

type ZipEntryIndex = std::collections::HashMap<String, (u64, u64)>;
type ZipIndexCache =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<PathBuf, ZipEntryIndex>>>;

/// Asset loader managing file resolution and decoding across vanilla, SH2:EE, and 4K AI Master directories.
#[derive(Debug, Clone)]
pub struct TextureLoader {
    data_dir: PathBuf,
    sh2e_dir: Option<PathBuf>,
    ai_4k_dir: Option<PathBuf>,
    zip_index_cache: ZipIndexCache,
}

impl TextureLoader {
    /// Constructs a new loader with the base game data directory and optional SH2:EE folder.
    pub fn new(data_dir: impl AsRef<Path>, sh2e_dir: Option<impl AsRef<Path>>) -> Self {
        let resolver = crate::resolver::AssetPathResolver::resolve();
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            sh2e_dir: sh2e_dir.map(|p| p.as_ref().to_path_buf()),
            ai_4k_dir: resolver.resolve_4k_texture_pack_dir(),
            zip_index_cache: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Constructs a new loader with an explicit 4K AI texture pack directory.
    pub fn with_4k_dir(
        data_dir: impl AsRef<Path>,
        sh2e_dir: Option<impl AsRef<Path>>,
        ai_4k_dir: Option<impl AsRef<Path>>,
    ) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
            sh2e_dir: sh2e_dir.map(|p| p.as_ref().to_path_buf()),
            ai_4k_dir: ai_4k_dir.map(|p| p.as_ref().to_path_buf()),
            zip_index_cache: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Returns the resolved 4K AI texture pack root directory or `.zip` archive, if present.
    pub fn ai_4k_dir(&self) -> Option<&Path> {
        self.ai_4k_dir.as_deref()
    }

    fn load_4k_override_from_sub(&self, sub: &str, tex_id: u32) -> Option<DecodedTexture> {
        let root = self.ai_4k_dir.as_ref()?;

        // 1. If `root` is a directory, check extracted files `<root>/<sub>/<tex_id>.{dds,png}` first
        if root.is_dir() {
            for ext in ["dds", "png"] {
                let p = root.join(sub).join(format!("{tex_id}.{ext}"));
                if p.is_file() {
                    if let Ok(bytes) = fs::read(&p) {
                        if let Ok(decoded) = decode_texture_bytes(&bytes, &p) {
                            return Some(decoded);
                        }
                    }
                }
            }
            // Check if a `.zip` archive is inside the directory
            let zip_name = if sub == "normal" {
                "sh2_4k_normals.zip"
            } else {
                "sh2_4k_textures.zip"
            };
            let nested_zip = root.join(zip_name);
            if nested_zip.is_file() {
                if let Some(dec) = self.read_texture_from_stored_zip(&nested_zip, sub, tex_id) {
                    return Some(dec);
                }
            }
        }

        // 2. If `root` itself is a `.zip` archive, read directly from it
        if root.is_file() && root.extension().is_some_and(|e| e == "zip") {
            if let Some(dec) = self.read_texture_from_stored_zip(root, sub, tex_id) {
                return Some(dec);
            }
        }

        None
    }

    /// Reads a `<sub_dir>/<tex_id>.{dds,png}` entry directly from a zero-compression (`ZIP_STORED`,
    /// supporting standard 32-bit and ZIP64 local/central headers) `.zip` texture pack archive,
    /// caching the parsed Central Directory index in `self.zip_index_cache`.
    fn read_texture_from_stored_zip(
        &self,
        zip_path: &Path,
        sub: &str,
        tex_id: u32,
    ) -> Option<DecodedTexture> {
        use std::io::{Read, Seek, SeekFrom};

        let cached_entry = {
            let mut guard = self.zip_index_cache.lock().ok()?;
            if !guard.contains_key(zip_path) {
                if let Some(index) = build_stored_zip_central_directory_index(zip_path) {
                    guard.insert(zip_path.to_path_buf(), index);
                }
            }
            let index = guard.get(zip_path)?;
            let dds_key = format!("{sub}/{tex_id}.dds");
            let png_key = format!("{sub}/{tex_id}.png");
            index
                .get(&dds_key)
                .copied()
                .map(|v| (dds_key, v))
                .or_else(|| index.get(&png_key).copied().map(|v| (png_key, v)))
        };

        let (entry_name, (local_hdr_ofs, comp_size)) = cached_entry?;
        let mut file = fs::File::open(zip_path).ok()?;
        let file_len = file.metadata().ok()?.len();

        let mut lhdr = [0u8; 30];
        file.seek(SeekFrom::Start(local_hdr_ofs)).ok()?;
        file.read_exact(&mut lhdr).ok()?;
        if &lhdr[0..4] != b"PK\x03\x04" {
            return None;
        }
        let lname_len = u16::from_le_bytes(lhdr[26..28].try_into().ok()?) as u64;
        let lextra_len = u16::from_le_bytes(lhdr[28..30].try_into().ok()?) as u64;
        let data_start = local_hdr_ofs
            .checked_add(30)?
            .checked_add(lname_len)?
            .checked_add(lextra_len)?;
        let data_end = data_start.checked_add(comp_size)?;
        if data_end > file_len || comp_size > 128 * 1024 * 1024 {
            return None;
        }
        file.seek(SeekFrom::Start(data_start)).ok()?;
        let mut payload = vec![0u8; comp_size as usize];
        file.read_exact(&mut payload).ok()?;
        decode_texture_bytes(&payload, Path::new(&entry_name)).ok()
    }

    /// Resolves and decodes a 4K AI upscaled world `.map` texture override (`map/<tex_id>.dds` or `.png`), if available.
    pub fn load_4k_map_texture_override(&self, tex_id: u32) -> Option<DecodedTexture> {
        self.load_4k_override_from_sub("map", tex_id)
    }

    /// Resolves and decodes a 4K AI upscaled character `.mdl` texture override (`chr/<tex_id>.dds` or `.png`), if available.
    pub fn load_4k_chr_texture_override(&self, tex_id: u32) -> Option<DecodedTexture> {
        self.load_4k_override_from_sub("chr", tex_id)
    }

    /// Resolves and decodes a 4K AI synthesized normal detail map (`normal/<tex_id>.dds` or `.png`), if available.
    pub fn load_4k_normal_map_override(&self, tex_id: u32) -> Option<DecodedTexture> {
        self.load_4k_override_from_sub("normal", tex_id)
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

    /// Loads a `.map` level geometry file from vanilla `data/` (merging shared `<area>.map` textures from `data/`)
    /// without applying `sh2e/` or 4K AI Master texture overrides, for side-by-side Original PS2 vs 4K comparison.
    pub fn load_vanilla_map(
        &self,
        relative_map_path: impl AsRef<Path>,
        map_name: &str,
    ) -> Result<crate::formats::map::MapFile, AssetError> {
        let rel = relative_map_path.as_ref();
        let clean_rel = rel
            .strip_prefix("data")
            .or_else(|_| rel.strip_prefix("sh2e"))
            .unwrap_or(rel);

        let data_map_path = self
            .resolve_in_data(clean_rel)
            .or_else(|| self.resolve_in_sh2e(clean_rel))
            .ok_or_else(|| {
                AssetError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Vanilla map not found: {}", clean_rel.display()),
                ))
            })?;

        let bytes = fs::read(&data_map_path)?;
        let mut map = crate::formats::map::parse_map(&bytes, map_name)?;

        if let Some(parent_dir) = clean_rel.parent() {
            if let Some(area_name) = parent_dir.file_name().and_then(|s| s.to_str()) {
                if !area_name.is_empty() {
                    let shared_rel = parent_dir.join(format!("{area_name}.map"));
                    if shared_rel != clean_rel {
                        if let Some(data_shared) = self.resolve_in_data(&shared_rel) {
                            self.merge_map_textures_from_path(&data_shared, area_name, &mut map);
                        }
                    }
                }
            }
        }

        Ok(map)
    }

    /// Loads a `.map` level geometry file with SH2:EE high-resolution texture pack (`sh2e/`)
    /// and shared stage area texture bank merging (`<area>.map`), without applying 4K AI Master overrides.
    pub fn load_sh2e_map(
        &self,
        relative_map_path: impl AsRef<Path>,
        map_name: &str,
    ) -> Result<crate::formats::map::MapFile, AssetError> {
        let rel = relative_map_path.as_ref();
        let clean_rel = rel
            .strip_prefix("data")
            .or_else(|_| rel.strip_prefix("sh2e"))
            .unwrap_or(rel);

        let sh2e_map_path = self.resolve_in_sh2e(clean_rel);
        let data_map_path = self.resolve_in_data(clean_rel);

        let primary_path = sh2e_map_path
            .as_ref()
            .or(data_map_path.as_ref())
            .ok_or_else(|| {
                AssetError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Map not found: {}", clean_rel.display()),
                ))
            })?;

        let primary_bytes = fs::read(primary_path)?;
        let mut map = crate::formats::map::parse_map(&primary_bytes, map_name)?;

        // If both sh2e and vanilla data maps exist, ensure any missing or higher-res textures are merged
        if let (Some(_), Some(ref van_path)) = (&sh2e_map_path, &data_map_path) {
            self.merge_map_textures_from_path(van_path, map_name, &mut map);
        }

        // Merge shared area texture bank `<area>.map` in the same directory (e.g. `bg/ob/ob.map` for `bg/ob/ob01.map`)
        self.merge_shared_area_textures(clean_rel, &mut map);

        Ok(map)
    }

    /// Loads a `.map` level geometry file with SH2:EE high-resolution texture pack interception,
    /// shared stage area texture bank merging (`<area>.map`), and 4K AI Master overrides.
    ///
    /// # Interceptor Precedence
    /// 1. Resolves the geometry and primary embedded textures from `sh2e/bg/.../*.map` first,
    ///    falling back to `data/bg/.../*.map`.
    /// 2. If both `sh2e/` and `data/` versions of the `.map` exist, compares every embedded texture
    ///    by `texture_id` and keeps whichever texture has the higher pixel resolution (`width * height`).
    /// 3. Automatically locates and merges the shared stage texture bank `<area>.map` in the same
    ///    directory (e.g. `bg/ob/ob.map` for `bg/ob/ob01.map`, or `bg/rr/rr.map` for `bg/rr/rr21.map`),
    ///    resolving shared area foliage, sign, and environment textures referenced by room materials.
    /// 4. Applies 4K AI Master texture overrides from `ai_4k_dir/map/<tex_id>.dds` or `.zip` if present.
    pub fn load_map_with_hd_overrides(
        &self,
        relative_map_path: impl AsRef<Path>,
        map_name: &str,
    ) -> Result<crate::formats::map::MapFile, AssetError> {
        let mut map = self.load_sh2e_map(relative_map_path, map_name)?;
        self.apply_4k_map_texture_overrides(&mut map);
        Ok(map)
    }

    /// Applies 4K AI Master texture overrides (`map/<tex_id>.dds` or `.png`, or `sh2_4k_textures.zip`) to a parsed [`MapFile`](crate::formats::map::MapFile),
    /// replacing any embedded texture where the 4K AI pack provides a higher-resolution surface.
    pub fn apply_4k_map_texture_overrides(&self, target_map: &mut crate::formats::map::MapFile) {
        let Some(root) = self.ai_4k_dir.as_ref() else {
            return;
        };
        let is_valid_source = (root.is_file() && root.extension().is_some_and(|e| e == "zip"))
            || root.join("map").is_dir()
            || root.join("sh2_4k_textures.zip").is_file();
        if !is_valid_source {
            return;
        }

        let mut overridden_count = 0usize;

        // Collect all texture IDs referenced by materials or already embedded in the map
        let mut candidate_tids: Vec<u32> = target_map.textures.keys().copied().collect();
        for mat in &target_map.materials {
            let tid = mat.texture_id as u32;
            if tid != 0 && !candidate_tids.contains(&tid) {
                candidate_tids.push(tid);
            }
        }

        for tid in candidate_tids {
            if let Some(decoded) = self.load_4k_map_texture_override(tid) {
                let existing_area = target_map
                    .textures
                    .get(&tid)
                    .map(|t| t.width.saturating_mul(t.height))
                    .unwrap_or(0);
                let new_area = decoded.width.saturating_mul(decoded.height);
                if new_area >= existing_area && new_area > 0 {
                    // Convert RGBA8 to BGRA8 (`kind == 0`) for `MapTexture` storage
                    let mut bgra = decoded.rgba;
                    let (chunks, _) = bgra.as_chunks_mut::<4>();
                    for px in chunks {
                        px.swap(0, 2);
                    }
                    target_map.textures.insert(
                        tid,
                        crate::formats::map::MapTexture {
                            id: tid,
                            width: decoded.width,
                            height: decoded.height,
                            kind: 0,
                            data: bgra,
                        },
                    );
                    overridden_count += 1;
                }
            }
        }
        if overridden_count > 0 {
            tracing::info!(
                "Applied {} 4K AI Master map texture overrides from {}",
                overridden_count,
                root.display()
            );
        }
    }

    pub(super) fn resolve_in_sh2e(&self, rel: &Path) -> Option<PathBuf> {
        self.sh2e_dir.as_ref().and_then(|sh2e| {
            let cand = sh2e.join(rel);
            if cand.exists() {
                Some(cand)
            } else {
                find_case_insensitive(sh2e, rel)
            }
        })
    }

    pub(super) fn resolve_in_data(&self, rel: &Path) -> Option<PathBuf> {
        let cand = self.data_dir.join(rel);
        if cand.exists() {
            Some(cand)
        } else {
            find_case_insensitive(&self.data_dir, rel)
        }
    }

    fn merge_map_textures_from_path(
        &self,
        path: &Path,
        map_name: &str,
        target_map: &mut crate::formats::map::MapFile,
    ) {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(donor_map) = crate::formats::map::parse_map(&bytes, map_name) {
                for (tid, donor_tex) in donor_map.textures {
                    match target_map.textures.get(&tid) {
                        Some(existing)
                            if existing.width.saturating_mul(existing.height)
                                >= donor_tex.width.saturating_mul(donor_tex.height) => {}
                        _ => {
                            target_map.textures.insert(tid, donor_tex);
                        }
                    }
                }
            }
        }
    }

    fn merge_shared_area_textures(
        &self,
        clean_rel: &Path,
        target_map: &mut crate::formats::map::MapFile,
    ) {
        let Some(parent_dir) = clean_rel.parent() else {
            return;
        };
        let Some(area_name) = parent_dir.file_name().and_then(|s| s.to_str()) else {
            return;
        };
        if area_name.is_empty() {
            return;
        }

        let shared_rel = parent_dir.join(format!("{area_name}.map"));
        if shared_rel == clean_rel {
            return;
        }

        if let Some(sh2e_shared) = self.resolve_in_sh2e(&shared_rel) {
            self.merge_map_textures_from_path(&sh2e_shared, area_name, target_map);
        }
        if let Some(data_shared) = self.resolve_in_data(&shared_rel) {
            self.merge_map_textures_from_path(&data_shared, area_name, target_map);
        }
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

/// Parses the Central Directory of a `ZIP_STORED` (32-bit or ZIP64) archive once and returns
/// a lookup map `entry_name -> (local_header_offset, compressed_size)`.
fn build_stored_zip_central_directory_index(
    zip_path: &Path,
) -> Option<std::collections::HashMap<String, (u64, u64)>> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = fs::File::open(zip_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len < 22 {
        return None;
    }

    // Read the last 65 KB to locate the End of Central Directory (EOCD / ZIP64 EOCD Locator)
    let tail_len = file_len.min(65557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64))).ok()?;
    let mut tail = vec![0u8; tail_len];
    file.read_exact(&mut tail).ok()?;

    let mut eocd_pos = None;
    for i in (0..=tail_len.saturating_sub(22)).rev() {
        if &tail[i..i + 4] == b"PK\x05\x06" {
            eocd_pos = Some(i);
            break;
        }
    }
    let eocd_idx = eocd_pos?;

    let mut cd_size =
        u32::from_le_bytes(tail[eocd_idx + 12..eocd_idx + 16].try_into().ok()?) as u64;
    let mut cd_offset =
        u32::from_le_bytes(tail[eocd_idx + 16..eocd_idx + 20].try_into().ok()?) as u64;

    // Check for ZIP64 End of Central Directory Locator (`PK\x06\x07`) 20 bytes before EOCD
    if eocd_idx >= 20 && &tail[eocd_idx - 20..eocd_idx - 16] == b"PK\x06\x07" {
        let zip64_eocd_ofs = u64::from_le_bytes(tail[eocd_idx - 12..eocd_idx - 4].try_into().ok()?);
        let mut z64_hdr = [0u8; 56];
        if file.seek(SeekFrom::Start(zip64_eocd_ofs)).is_ok()
            && file.read_exact(&mut z64_hdr).is_ok()
            && &z64_hdr[0..4] == b"PK\x06\x06"
        {
            cd_size = u64::from_le_bytes(z64_hdr[40..48].try_into().ok()?);
            cd_offset = u64::from_le_bytes(z64_hdr[48..56].try_into().ok()?);
        }
    }

    if cd_offset.checked_add(cd_size)? > file_len || cd_size > 32 * 1024 * 1024 {
        return None;
    }

    file.seek(SeekFrom::Start(cd_offset)).ok()?;
    let mut cd_buf = vec![0u8; cd_size as usize];
    file.read_exact(&mut cd_buf).ok()?;

    let mut index = std::collections::HashMap::new();
    let mut pos = 0usize;
    while pos + 46 <= cd_buf.len() && &cd_buf[pos..pos + 4] == b"PK\x01\x02" {
        let compression = u16::from_le_bytes(cd_buf[pos + 10..pos + 12].try_into().ok()?);
        let mut comp_size = u32::from_le_bytes(cd_buf[pos + 20..pos + 24].try_into().ok()?) as u64;
        let uncomp_size = u32::from_le_bytes(cd_buf[pos + 24..pos + 28].try_into().ok()?) as u64;
        let name_len = u16::from_le_bytes(cd_buf[pos + 28..pos + 30].try_into().ok()?) as usize;
        let extra_len = u16::from_le_bytes(cd_buf[pos + 30..pos + 32].try_into().ok()?) as usize;
        let comment_len = u16::from_le_bytes(cd_buf[pos + 32..pos + 34].try_into().ok()?) as usize;
        let mut local_hdr_ofs =
            u32::from_le_bytes(cd_buf[pos + 42..pos + 46].try_into().ok()?) as u64;

        let name_start = pos + 46;
        let extra_start = name_start + name_len;
        let next_pos = extra_start + extra_len + comment_len;
        if next_pos > cd_buf.len() {
            break;
        }

        if compression == 0 {
            if let Ok(entry_name) = std::str::from_utf8(&cd_buf[name_start..extra_start]) {
                // Parse ZIP64 extra field (`0x0001`) if any 32-bit field is `0xFFFF_FFFF`
                if uncomp_size == 0xFFFF_FFFF
                    || comp_size == 0xFFFF_FFFF
                    || local_hdr_ofs == 0xFFFF_FFFF
                {
                    let extra = &cd_buf[extra_start..extra_start + extra_len];
                    let mut ep = 0usize;
                    while ep + 4 <= extra.len() {
                        let header_id = u16::from_le_bytes(extra[ep..ep + 2].try_into().ok()?);
                        let data_sz =
                            u16::from_le_bytes(extra[ep + 2..ep + 4].try_into().ok()?) as usize;
                        ep += 4;
                        if header_id == 0x0001 && ep + data_sz <= extra.len() {
                            let mut zp = ep;
                            if uncomp_size == 0xFFFF_FFFF && zp + 8 <= ep + data_sz {
                                zp += 8;
                            }
                            if comp_size == 0xFFFF_FFFF && zp + 8 <= ep + data_sz {
                                comp_size = u64::from_le_bytes(extra[zp..zp + 8].try_into().ok()?);
                                zp += 8;
                            }
                            if local_hdr_ofs == 0xFFFF_FFFF && zp + 8 <= ep + data_sz {
                                local_hdr_ofs =
                                    u64::from_le_bytes(extra[zp..zp + 8].try_into().ok()?);
                            }
                            break;
                        }
                        ep += data_sz;
                    }
                }
                index.insert(entry_name.to_string(), (local_hdr_ofs, comp_size));
            }
        }
        pos = next_pos;
    }

    Some(index)
}

/// Computes the number of mipmap levels for a 2D texture of dimensions `(width, height)`.
pub fn mip_level_count_for_size(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    32 - max_dim.leading_zeros()
}

/// Alpha-test cutout threshold (`128` out of `255` = `0.5` normalized) used when preserving
/// cutout alpha coverage across mipmap levels (`SH2-92`).
pub const ALPHA_CUTOUT_MIP_REF_THRESHOLD: u8 = 128;

/// Computes the fraction `[0.0, 1.0]` of texels in an RGBA8 buffer whose alpha channel is `>= threshold`.
pub fn compute_alpha_coverage_rgba(rgba: &[u8], threshold: u8) -> f32 {
    let total_px = rgba.len() / 4;
    if total_px == 0 {
        return 1.0;
    }
    let mut passing = 0usize;
    for px in rgba.as_chunks::<4>().0 {
        if px[3] >= threshold {
            passing += 1;
        }
    }
    (passing as f32) / (total_px as f32)
}

/// Rescales the alpha channel of `rgba` via binary search so that its cutout coverage at `threshold`
/// matches `target_coverage` (Castano's alpha-to-coverage mipmap preservation method).
///
/// Prevents distant chain-link fences, grate floors, tree foliage, and character hair fringes from
/// thinning out or vanishing at higher mip levels under 16x anisotropic filtering.
pub fn preserve_alpha_coverage_in_place(rgba: &mut [u8], target_coverage: f32, threshold: u8) {
    let total_px = rgba.len() / 4;
    if total_px == 0 || target_coverage <= 0.0 || target_coverage >= 1.0 {
        return;
    }

    let current_cov = compute_alpha_coverage_rgba(rgba, threshold);
    if (current_cov - target_coverage).abs() <= (1.0 / (total_px as f32).max(1.0)) {
        return;
    }

    // Binary search for a multiplier `scale` in `[0.25, 8.0]` that brings coverage closest to `target_coverage`.
    // If all texels fell below `threshold` after uniform box averaging (so `current_cov == 0.0`),
    // ensure the lower bound starts at the exact scale needed to lift the peak alpha in this mip level
    // back to `threshold`.
    let max_a = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| px[3])
        .max()
        .unwrap_or(0);
    if max_a == 0 {
        return;
    }

    let min_lift = (threshold as f32) / (max_a as f32);
    let mut lo = 0.25f32;
    let mut hi = (4.0f32).max(min_lift * 1.5);
    let mut best_scale = min_lift.max(1.0);
    let mut best_diff = (current_cov - target_coverage).abs();

    for _ in 0..12 {
        let mid = 0.5 * (lo + hi);
        let mut passing = 0usize;
        for px in rgba.as_chunks::<4>().0 {
            let scaled_a = (px[3] as f32 * mid).round().clamp(0.0, 255.0) as u8;
            if scaled_a >= threshold {
                passing += 1;
            }
        }
        let cov = (passing as f32) / (total_px as f32);
        let diff = (cov - target_coverage).abs();
        if diff < best_diff || (cov > 0.0 && best_diff == target_coverage) {
            best_diff = diff;
            best_scale = mid;
        }
        if cov < target_coverage {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    if (best_scale - 1.0).abs() > 1e-3 {
        for px in rgba.as_chunks_mut::<4>().0 {
            px[3] = (px[3] as f32 * best_scale).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Generates a full box-filtered RGBA8 mipmap chain from level-0 `rgba` pixels.
///
/// Preserves cutout alpha coverage (`ALPHA_CUTOUT_MIP_REF_THRESHOLD`) and weights RGB downsampling
/// by alpha so transparent cutout regions neither thin out at distance nor bleed dark borders into
/// higher mip levels when sampled with 16x anisotropic filtering.
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
    let mip0_coverage = compute_alpha_coverage_rgba(&mip0, ALPHA_CUTOUT_MIP_REF_THRESHOLD);
    // Only apply Castano cutout coverage scaling when the texture actually contains transparent
    // cutout texels (`alpha < 4`). Non-cutout DXT5 textures that store specular/gloss intensity
    // in the alpha channel (where `min_alpha > 0` across the entire surface, such as Pyramid Head's
    // helmet `1303`/`1307`, Great Knife `1305`, or Lying Figure `1282`) must not have their alpha
    // rescaled toward `ALPHA_CUTOUT_MIP_REF_THRESHOLD` across mip levels.
    let has_transparent_cutout_texels = mip0.as_chunks::<4>().0.iter().any(|px| px[3] < 4);
    let has_cutout_alpha =
        has_transparent_cutout_texels && mip0_coverage > 0.0 && mip0_coverage < 1.0;
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

        if has_cutout_alpha {
            preserve_alpha_coverage_in_place(
                &mut next_data,
                mip0_coverage,
                ALPHA_CUTOUT_MIP_REF_THRESHOLD,
            );
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
                let pixel_count = self.width * self.height;
                let required = pixel_count + 32 + 1024;
                if self.data.len() < required {
                    return Err(AssetError::BufferUnderflow {
                        required,
                        available: self.data.len(),
                    });
                }
                let pal = &self.data[pixel_count + 32..pixel_count + 32 + 1024];
                let mut rgba = vec![0u8; pixel_count * 4];
                for (i, &idx) in self.data[..pixel_count].iter().enumerate() {
                    let p_ofs = (idx as usize) * 4;
                    let d_ofs = i * 4;
                    rgba[d_ofs] = pal[p_ofs];
                    rgba[d_ofs + 1] = pal[p_ofs + 1];
                    rgba[d_ofs + 2] = pal[p_ofs + 2];
                    rgba[d_ofs + 3] = pal[p_ofs + 3];
                }
                return Ok(rgba);
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
                    w * h + 32 + 1024
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

    // 3. Check for PS2-format .tex (`bytes[0x0c..0x10] == b"\xa7\xa7\xa7\xa7"`)
    if bytes.len() >= 0xc0 && &bytes[0x0c..0x10] == b"\xa7\xa7\xa7\xa7" {
        return decode_ps2_tex(bytes);
    }

    // 4. Check for PS2/PC .tex container (`0x19990901`)
    if bytes.len() >= 16 {
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic == TEX_MAGIC {
            // 4a. Multi-sprite container traversal
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

            // 4b. Single-sprite container unpack (0x30 header, pixel payload at 0x50)
            // Used by standalone splash/UI screens like start00.tex, start01.tex, konami2048x2048.tex, etc.
            if bytes.len() >= 0x50 {
                let w = u16::from_le_bytes([bytes[0x38], bytes[0x39]]) as usize;
                let h = u16::from_le_bytes([bytes[0x3a], bytes[0x3b]]) as usize;
                let fmt = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]);
                let length =
                    u32::from_le_bytes([bytes[0x40], bytes[0x41], bytes[0x42], bytes[0x43]])
                        as usize;

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
                    } else if fmt == 8 {
                        let idx_len = w * h;
                        let pal_ofs = length.max(idx_len) + 0x20;
                        if pixel_data.len() >= pal_ofs + 1024 {
                            let pal = &pixel_data[pal_ofs..pal_ofs + 1024];
                            let mut rgba = vec![0u8; idx_len * 4];
                            for (i, &idx) in pixel_data[..idx_len].iter().enumerate() {
                                let p_ofs = (idx as usize) * 4;
                                let d_ofs = i * 4;
                                rgba[d_ofs] = pal[p_ofs];
                                rgba[d_ofs + 1] = pal[p_ofs + 1];
                                rgba[d_ofs + 2] = pal[p_ofs + 2];
                                rgba[d_ofs + 3] = pal[p_ofs + 3];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_coverage_preserving_mipmap_chain() {
        // Construct a 64x64 cutout chain-link fence wire grid where wire crossings (`x % 4 == 0 || y % 4 == 0`)
        // have alpha=220 and gaps have alpha=0 (~43.75% coverage at mip 0).
        // Simulate a soft-edged chain-link fence / foliage cutout texture (`64x64`).
        // Each `2x2` cell contains two opaque/semi-opaque wire texels (`160..240`) and two transparent texels (`0`),
        // so naive `2x2` box-averaging drops most cells below `128` (`(180 + 180 + 0 + 0) / 4 = 90 < 128`),
        // causing thin cutouts to thin out and vanish in mipmaps.
        // With `generate_mipmap_chain_rgba` alpha-coverage preservation, mip 1 coverage is restored to match mip 0.
        let w = 64u32;
        let h = 64u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let ofs = ((y * w + x) * 4) as usize;
                rgba[ofs] = 180;
                rgba[ofs + 1] = 190;
                rgba[ofs + 2] = 200;
                let cell_idx = ((y / 2) * 37 + (x / 2) * 53) % 100;
                let is_wire_cell = cell_idx < 70; // 70% of 2x2 cells have wire strands
                rgba[ofs + 3] = if is_wire_cell && (x % 2 == 0) {
                    (160 + (cell_idx % 80)) as u8
                } else {
                    0
                };
            }
        }

        let cov0 = compute_alpha_coverage_rgba(&rgba, ALPHA_CUTOUT_MIP_REF_THRESHOLD);
        assert!(
            (cov0 - 0.35).abs() < 0.05,
            "Expected mip 0 coverage near 0.35, got {cov0}"
        );

        let chain = generate_mipmap_chain_rgba(&rgba, w, h);
        assert_eq!(chain.len(), 7); // 64, 32, 16, 8, 4, 2, 1
        let (mip1_w, mip1_h, ref mip1_data) = chain[1];
        assert_eq!((mip1_w, mip1_h), (32, 32));
        let cov1 = compute_alpha_coverage_rgba(mip1_data, ALPHA_CUTOUT_MIP_REF_THRESHOLD);
        assert!(
            (cov1 - cov0).abs() < 0.08,
            "Mip 1 cutout coverage should match mip 0 ({cov0}) instead of collapsing to 0.0 under naive box averaging, got {cov1}"
        );
    }

    #[test]
    fn test_gloss_map_alpha_not_distorted_by_cutout_mipmap_scaling() {
        // Construct a 64x64 non-cutout gloss/specular texture (like Pyramid Head's helmet `1303`/`1307`),
        // where `min_alpha >= 12` (zero transparent cutout texels) and ~84% of pixels have `alpha < 128`.
        // Verify `generate_mipmap_chain_rgba` does NOT apply Castano cutout scaling at threshold 128
        // and preserves the smooth box-averaged gloss alpha distribution across mip levels.
        let w = 64u32;
        let h = 64u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let ofs = ((y * w + x) * 4) as usize;
                rgba[ofs] = 120;
                rgba[ofs + 1] = 60;
                rgba[ofs + 2] = 40;
                let highlight = (x % 8 == 0) && (y % 8 == 0);
                rgba[ofs + 3] = if highlight { 220 } else { 16 };
            }
        }

        let chain = generate_mipmap_chain_rgba(&rgba, w, h);
        assert_eq!(chain.len(), 7);
        for (lvl, (_, _, ref mip_data)) in chain.iter().enumerate() {
            let min_a = mip_data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|px| px[3])
                .min()
                .unwrap_or(0);
            assert!(
                min_a >= 16,
                "Mip level {lvl} of non-cutout gloss texture must preserve min_alpha >= 16, got {min_a}"
            );
        }
    }

    #[test]
    fn test_load_4k_texture_overrides_from_generated_pack() {
        let resolver = crate::resolver::AssetPathResolver::resolve();
        let Some(pack_dir) = resolver.resolve_4k_texture_pack_dir() else {
            return;
        };
        let loader = TextureLoader::with_4k_dir(&resolver.data_dir, None::<&Path>, Some(&pack_dir));

        // World map (`er01.map`) primary texture `3332` upscaled from 512x512 -> 2048x2048
        let map_tex = loader
            .load_4k_map_texture_override(3332)
            .expect("Expected 4K map texture override for tex_id 3332");
        assert_eq!(map_tex.width, 2048);
        assert_eq!(map_tex.height, 2048);
        assert_eq!(map_tex.rgba.len(), 2048 * 2048 * 4);

        // Optional normal/roughness detail map for `3332` (present in raw pipeline output)
        if let Some(norm_tex) = loader.load_4k_normal_map_override(3332) {
            assert_eq!(norm_tex.width, 2048);
            assert_eq!(norm_tex.height, 2048);
        }

        // Character (`hhh_jms.mdl`) primary texture `1239` upscaled from 512x512 -> 2048x2048
        let chr_tex = loader
            .load_4k_chr_texture_override(1239)
            .expect("Expected 4K character texture override for tex_id 1239");
        assert_eq!(chr_tex.width, 2048);
        assert_eq!(chr_tex.height, 2048);

        // Verify direct zero-copy reading from the modular ZIP64 archive (`mods/sh2_4k_textures/sh2_4k_textures.zip`)
        let zip_path = if pack_dir.is_file() {
            pack_dir.clone()
        } else {
            pack_dir.join("sh2_4k_textures.zip")
        };
        if zip_path.is_file() {
            let zip_loader =
                TextureLoader::with_4k_dir(&resolver.data_dir, None::<&Path>, Some(&zip_path));
            let zip_map_tex = zip_loader
                .load_4k_map_texture_override(1850)
                .expect("Expected 4K map texture 1850 from sh2_4k_textures.zip");
            assert_eq!(zip_map_tex.width, 2048);
            assert_eq!(zip_map_tex.height, 2048);

            let zip_chr_tex = zip_loader
                .load_4k_chr_texture_override(1260)
                .expect("Expected 4K character texture 1260 (Angela) from sh2_4k_textures.zip");
            assert_eq!(zip_chr_tex.width, 2048);
            assert_eq!(zip_chr_tex.height, 2048);
        }

        // Verify full `load_map_with_hd_overrides` on Restroom (`bg/er/er01.map`) resolves 2048x2048 4K textures from `mods/`
        if resolver.data_dir.is_dir() {
            let auto_loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
            assert!(
                auto_loader.ai_4k_dir().is_some(),
                "Expected TextureLoader::new to auto-resolve mods/sh2_4k_textures"
            );
            let er01_map = auto_loader
                .load_map_with_hd_overrides("bg/er/er01.map", "er01")
                .expect("Should load er01.map with 4K overrides");
            let tex_3332 = er01_map
                .textures
                .get(&3332)
                .expect("er01.map should contain texture 3332");
            assert_eq!((tex_3332.width, tex_3332.height), (2048, 2048));
        }
    }
}
