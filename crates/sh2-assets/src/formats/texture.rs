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
use std::fs;
use std::path::{Path, PathBuf};

mod container;
mod cutaway;
mod mipmap;
mod zip_archive;

pub use container::{decode_texture_bytes, parse_texture_container, ContainerTexture};
pub use cutaway::{probe_tex_dimensions, CutawayTextureAsset, CutawayViewportTransform};
pub use mipmap::{
    compute_alpha_coverage_rgba, generate_mipmap_chain_rgba, mip_level_count_for_size,
    preserve_alpha_coverage_in_place, ALPHA_CUTOUT_MIP_REF_THRESHOLD,
};
use zip_archive::{read_texture_from_stored_zip, ZipIndexCache};

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
                if let Some(dec) =
                    read_texture_from_stored_zip(&self.zip_index_cache, &nested_zip, sub, tex_id)
                {
                    return Some(dec);
                }
            }
        }

        // 2. If `root` itself is a `.zip` archive, read directly from it
        if root.is_file() && root.extension().is_some_and(|e| e == "zip") {
            if let Some(dec) =
                read_texture_from_stored_zip(&self.zip_index_cache, root, sub, tex_id)
            {
                return Some(dec);
            }
        }

        None
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
