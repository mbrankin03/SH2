//! Layered Virtual File System (VFS) and drag-and-drop mod package discovery (`SH2-29`).
//!
//! # Override Precedence (`Common/FileSystemHooks.cpp` extended for `mods/`)
//! 1. **Active Mods (`mods/<mod_name>/` or `mods/<mod_name>.zip`)**: Ordered by descending
//!    load-order priority (higher priority wins).
//! 2. **SH2:EE Enhancements (`sh2e/`)**: HD texture, audio, and FMV overrides.
//! 3. **Vanilla Game Data (`data/`)**: Retail base assets.
//!
//! All path lookups are case-insensitive across path components on Linux/macOS to match
//! retail `sh2pc.exe` Windows filesystem semantics.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::formats::texture::zip_archive::{
    build_stored_zip_central_directory_index, read_indexed_entry_bytes,
    read_mod_entry_bytes_from_stored_zip, ZipIndexCache,
};

/// Optional `mod.json` metadata manifest placed at the root of a mod folder or `.zip` archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModMetadata {
    /// Human-readable mod title (defaults to folder or `.zip` stem if omitted).
    #[serde(default)]
    pub name: String,
    /// Mod creator or team name.
    #[serde(default = "default_unknown_author")]
    pub author: String,
    /// Semantic or release version string.
    #[serde(default = "default_mod_version")]
    pub version: String,
    /// Short description displayed in the Options -> MODS detail box.
    #[serde(default)]
    pub description: String,
    /// Default load-order priority (higher values override lower values).
    #[serde(default)]
    pub priority: i32,
    /// Optional relative path to a preview thumbnail inside the mod package.
    #[serde(default)]
    pub preview_thumbnail: Option<String>,
}

fn default_unknown_author() -> String {
    "Unknown".to_string()
}

fn default_mod_version() -> String {
    "1.0".to_string()
}

impl Default for ModMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            author: default_unknown_author(),
            version: default_mod_version(),
            description: String::new(),
            priority: 0,
            preview_thumbnail: None,
        }
    }
}

/// Backing storage format for a discovered drag-and-drop mod package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModSource {
    /// Uncompressed directory under `mods/<mod_id>/`.
    Directory(PathBuf),
    /// Zero-compression (`ZIP_STORED`) archive under `mods/<mod_id>.zip`.
    StoredZip(PathBuf),
}

impl ModSource {
    /// Returns `true` if the mod is packaged as a `.zip` archive.
    #[must_use]
    pub fn is_zip(&self) -> bool {
        matches!(self, Self::StoredZip(_))
    }

    /// Returns the filesystem path to the mod directory or `.zip` archive.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Directory(p) | Self::StoredZip(p) => p.as_path(),
        }
    }
}

/// Discovered mod package with resolved metadata, load order, and override file inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackage {
    /// Stable identifier (folder name or `.zip` stem).
    pub id: String,
    /// Parsed `mod.json` metadata (or synthesized defaults).
    pub metadata: ModMetadata,
    /// Backing directory or `.zip` archive.
    pub source: ModSource,
    /// Whether the mod is currently enabled in `config.json`.
    pub enabled: bool,
    /// Effective load-order priority (higher values take precedence).
    pub priority: i32,
    /// Normalized relative asset paths provided by this mod (e.g. `pic/title.png`).
    pub overrides: Vec<String>,
}

/// Layered Virtual File System routing asset reads through `mods/` -> `sh2e/` -> `data/`.
#[derive(Debug, Clone)]
pub struct ModVfs {
    active_mods: Vec<ModPackage>,
    sh2e_dir: Option<PathBuf>,
    data_dir: PathBuf,
    zip_index_cache: ZipIndexCache,
}

impl ModVfs {
    /// Constructs a [`ModVfs`] from discovered [`ModPackage`] entries, retaining only enabled
    /// mods sorted by descending `priority` (then ascending `id` for determinism).
    #[must_use]
    pub fn from_packages(
        packages: &[ModPackage],
        sh2e_dir: Option<PathBuf>,
        data_dir: PathBuf,
    ) -> Self {
        let mut active_mods: Vec<ModPackage> =
            packages.iter().filter(|m| m.enabled).cloned().collect();
        active_mods.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Self {
            active_mods,
            sh2e_dir,
            data_dir,
            zip_index_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns the active enabled mods in descending priority order (highest precedence first).
    #[must_use]
    pub fn active_mods(&self) -> &[ModPackage] {
        &self.active_mods
    }

    /// Resolves a relative asset path to an on-disk file across directory-backed active `mods/`
    /// layers only (in descending priority order), checking `sh2e/` > `data/` > root.
    #[must_use]
    pub fn resolve_mod_file(&self, rel_path: impl AsRef<Path>) -> Option<PathBuf> {
        let rel = normalize_rel_path(rel_path.as_ref());
        for m in &self.active_mods {
            if let ModSource::Directory(ref root) = m.source {
                if let Some(p) = resolve_mod_dir_candidates(root, &rel) {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Resolves a relative asset path to an on-disk file across directory-backed mods
    /// (`sh2e/` > `data/` > root), `sh2e/`, and `data/` (useful for streaming media).
    #[must_use]
    pub fn resolve_disk_path(&self, rel_path: impl AsRef<Path>) -> Option<PathBuf> {
        let rel = normalize_rel_path(rel_path.as_ref());
        for m in &self.active_mods {
            if let ModSource::Directory(ref root) = m.source {
                if let Some(p) = resolve_mod_dir_candidates(root, &rel) {
                    return Some(p);
                }
            }
        }
        if let Some(ref sh2e) = self.sh2e_dir {
            if let Some(p) = resolve_in_dir_case_insensitive(sh2e, &rel) {
                return Some(p);
            }
        }
        resolve_in_dir_case_insensitive(&self.data_dir, &rel)
    }

    /// Reads raw bytes from active `mods/` layers only, returning `(mod_id, bytes)` for the
    /// highest-priority mod that contains `rel_path`.
    #[must_use]
    pub fn read_mod_bytes(&self, rel_path: impl AsRef<Path>) -> Option<(String, Vec<u8>)> {
        let rel = normalize_rel_path(rel_path.as_ref());
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        for m in &self.active_mods {
            match &m.source {
                ModSource::Directory(root) => {
                    if let Some(p) = resolve_mod_dir_candidates(root, &rel) {
                        if let Ok(bytes) = fs::read(&p) {
                            return Some((m.id.clone(), bytes));
                        }
                    }
                }
                ModSource::StoredZip(zip_path) => {
                    if let Some(bytes) =
                        read_mod_zip_entry(&self.zip_index_cache, zip_path, &rel_str)
                    {
                        return Some((m.id.clone(), bytes));
                    }
                }
            }
        }
        None
    }

    /// Reads raw bytes for `rel_path` across the full VFS hierarchy:
    /// `mods/` (descending priority) -> `sh2e/` -> `data/`.
    #[must_use]
    pub fn read_bytes(&self, rel_path: impl AsRef<Path>) -> Option<Vec<u8>> {
        if let Some((_mod_id, bytes)) = self.read_mod_bytes(&rel_path) {
            return Some(bytes);
        }
        let rel = normalize_rel_path(rel_path.as_ref());
        if let Some(ref sh2e) = self.sh2e_dir {
            if let Some(p) = resolve_in_dir_case_insensitive(sh2e, &rel) {
                if let Ok(bytes) = fs::read(&p) {
                    return Some(bytes);
                }
            }
        }
        if let Some(p) = resolve_in_dir_case_insensitive(&self.data_dir, &rel) {
            return fs::read(&p).ok();
        }
        None
    }
}

/// Returns `true` if `ext` (case-insensitive, without leading dot) is a supported SH2 mod asset extension.
#[must_use]
pub fn is_supported_mod_asset_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "dds"
            | "png"
            | "tex"
            | "tbn2"
            | "sfd"
            | "mp4"
            | "mkv"
            | "bik"
            | "sdm"
            | "wav"
            | "ogg"
            | "adx"
            | "aix"
            | "afs"
            | "bin"
            | "map"
            | "anm"
            | "mdl"
            | "cls"
            | "cam"
            | "cld"
    )
}

/// Scans `mods_dir` for uncompressed mod folders and `.zip` archives, merging persisted
/// `(id, enabled, priority)` settings from `config.json`.
#[must_use]
pub fn discover_mods(mods_dir: &Path, saved_entries: &[(String, bool, i32)]) -> Vec<ModPackage> {
    if !mods_dir.is_dir() {
        return Vec::new();
    }

    let saved_map: HashMap<&str, (bool, i32)> = saved_entries
        .iter()
        .map(|(id, en, prio)| (id.as_str(), (*en, *prio)))
        .collect();

    let mut packages = Vec::new();
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return packages;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
                continue;
            };
            let mut metadata = read_dir_mod_metadata(&path).unwrap_or_default();
            if metadata.name.is_empty() {
                metadata.name = id.clone();
            }
            let overrides = collect_dir_overrides(&path);
            let (enabled, priority) = saved_map
                .get(id.as_str())
                .copied()
                .unwrap_or((true, metadata.priority));
            packages.push(ModPackage {
                id,
                metadata,
                source: ModSource::Directory(path),
                enabled,
                priority,
                overrides,
            });
        } else if path.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        {
            let Some(id) = path.file_stem().and_then(|n| n.to_str()).map(String::from) else {
                continue;
            };
            let index = build_stored_zip_central_directory_index(&path);
            let mut metadata = index
                .as_ref()
                .and_then(|idx| read_indexed_entry_bytes(&path, idx, "mod.json"))
                .and_then(|b: Vec<u8>| serde_json::from_slice::<ModMetadata>(&b).ok())
                .unwrap_or_default();
            if metadata.name.is_empty() {
                metadata.name = id.clone();
            }
            let mut overrides = Vec::new();
            if let Some(ref index) = index {
                for key in &index.original_names {
                    if key.eq_ignore_ascii_case("mod.json") || key.ends_with('/') {
                        continue;
                    }
                    if let Some(ext) = Path::new(key).extension().and_then(|e| e.to_str()) {
                        if is_supported_mod_asset_ext(ext) {
                            overrides.push(key.clone());
                        }
                    }
                }
            }
            overrides.sort();
            let (enabled, priority) = saved_map
                .get(id.as_str())
                .copied()
                .unwrap_or((true, metadata.priority));
            packages.push(ModPackage {
                id,
                metadata,
                source: ModSource::StoredZip(path),
                enabled,
                priority,
                overrides,
            });
        }
    }

    packages.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
    packages
}

fn read_mod_zip_entry(
    zip_index_cache: &ZipIndexCache,
    zip_path: &Path,
    rel_str: &str,
) -> Option<Vec<u8>> {
    read_mod_entry_bytes_from_stored_zip(zip_index_cache, zip_path, rel_str)
}

fn resolve_mod_dir_candidates(root: &Path, rel: &Path) -> Option<PathBuf> {
    for cand in [
        PathBuf::from("sh2e").join(rel),
        PathBuf::from("data").join(rel),
        rel.to_path_buf(),
    ] {
        if let Some(p) = resolve_in_dir_case_insensitive(root, &cand) {
            return Some(p);
        }
    }
    None
}

fn read_dir_mod_metadata(mod_root: &Path) -> Option<ModMetadata> {
    let meta_path = resolve_in_dir_case_insensitive(mod_root, Path::new("mod.json"))?;
    let bytes = fs::read(meta_path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn collect_dir_overrides(mod_root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_dir_overrides_rec(mod_root, mod_root, &mut out);
    out.sort();
    out
}

fn collect_dir_overrides_rec(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_dir_overrides_rec(root, &p, out);
        } else if p.is_file() {
            if let Ok(rel) = p.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if rel_str.eq_ignore_ascii_case("mod.json") {
                    continue;
                }
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if is_supported_mod_asset_ext(ext) {
                        out.push(rel_str);
                    }
                }
            }
        }
    }
}

fn normalize_rel_path(rel: &Path) -> PathBuf {
    let s = rel.to_string_lossy().replace('\\', "/");
    let trimmed = s.trim_start_matches('/');
    let stripped = trimmed
        .strip_prefix("data/")
        .or_else(|| trimmed.strip_prefix("sh2e/"))
        .unwrap_or(trimmed);
    PathBuf::from(stripped)
}

fn resolve_in_dir_case_insensitive(base: &Path, rel: &Path) -> Option<PathBuf> {
    let direct = base.join(rel);
    if direct.exists() {
        return Some(direct);
    }
    let mut current = base.to_path_buf();
    for component in rel.components() {
        let target = component.as_os_str().to_string_lossy();
        let exact = current.join(target.as_ref());
        if exact.exists() {
            current = exact;
            continue;
        }
        let entries = fs::read_dir(&current).ok()?;
        let mut matched = None;
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.eq_ignore_ascii_case(target.as_ref()) {
                    matched = Some(entry.path());
                    break;
                }
            }
        }
        current = matched?;
    }
    if current.exists() {
        Some(current)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfs_prioritizes_mods_over_sh2e_and_data_with_zip_and_dir_support() {
        let tmp = std::env::temp_dir().join(format!(
            "sh2_29_vfs_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        let mods_dir = tmp.join("mods");
        let sh2e_dir = tmp.join("sh2e");
        let data_dir = tmp.join("data");
        fs::create_dir_all(mods_dir.join("high_prio/pic")).unwrap();
        fs::create_dir_all(mods_dir.join("low_prio/pic")).unwrap();
        fs::create_dir_all(sh2e_dir.join("pic")).unwrap();
        fs::create_dir_all(data_dir.join("pic")).unwrap();

        fs::write(
            mods_dir.join("high_prio/mod.json"),
            r#"{"name":"High Mod","author":"Tester","version":"1.2","description":"Overrides title","priority":20}"#,
        )
        .unwrap();
        fs::write(mods_dir.join("high_prio/pic/title.png"), b"HIGH_MOD").unwrap();
        fs::write(mods_dir.join("low_prio/pic/title.png"), b"LOW_MOD").unwrap();
        fs::write(mods_dir.join("low_prio/pic/other.dds"), b"LOW_OTHER").unwrap();
        fs::write(sh2e_dir.join("pic/title.png"), b"SH2E").unwrap();
        fs::write(sh2e_dir.join("pic/sh2e_only.dds"), b"SH2E_ONLY").unwrap();
        fs::write(data_dir.join("pic/vanilla.tex"), b"VANILLA").unwrap();

        let zip_bytes = build_test_stored_zip(&[
            (
                "mod.json",
                br#"{"name":"Zip Mod","author":"ZipTester","version":"2.0","description":"Zip override","priority":30}"#,
            ),
            ("pic/title.png", b"ZIP_MOD"),
            ("pic/zip_only.dds", b"ZIP_ONLY"),
        ]);
        fs::write(mods_dir.join("zip_mod.zip"), zip_bytes).unwrap();

        let packages = discover_mods(&mods_dir, &[]);
        assert_eq!(packages.len(), 3);
        assert_eq!(packages[0].id, "zip_mod");
        assert!(packages[0].source.is_zip());
        assert_eq!(packages[0].metadata.name, "Zip Mod");
        assert_eq!(packages[1].id, "high_prio");
        assert_eq!(packages[1].metadata.name, "High Mod");
        assert_eq!(packages[1].overrides, vec!["pic/title.png".to_string()]);

        let vfs = ModVfs::from_packages(&packages, Some(sh2e_dir), data_dir);
        assert_eq!(
            vfs.read_bytes("pic/TITLE.PNG").as_deref(),
            Some(b"ZIP_MOD".as_slice())
        );
        assert_eq!(
            vfs.read_bytes("pic/zip_only.dds").as_deref(),
            Some(b"ZIP_ONLY".as_slice())
        );
        assert_eq!(
            vfs.read_bytes("pic/other.dds").as_deref(),
            Some(b"LOW_OTHER".as_slice())
        );
        assert_eq!(
            vfs.read_bytes("pic/sh2e_only.dds").as_deref(),
            Some(b"SH2E_ONLY".as_slice())
        );
        assert_eq!(
            vfs.read_bytes("pic/vanilla.tex").as_deref(),
            Some(b"VANILLA".as_slice())
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    fn build_test_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for &(name, data) in entries {
            let offset = out.len() as u32;
            let name_bytes = name.as_bytes();
            let size = data.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // ZIP_STORED
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(data);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes()); // ZIP_STORED
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name_bytes);
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        let count = entries.len() as u16;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    #[test]
    fn vfs_reads_deflated_nexus_zip_with_wrapper_and_sh2e_priority() {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;

        let deflate = |raw: &[u8]| -> Vec<u8> {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
            enc.write_all(raw).unwrap();
            enc.finish().unwrap()
        };

        let entries: [(&str, &[u8], Vec<u8>); 3] = [
            (
                "Borat Mod/data/chr/jms/hhh_jms.mdl",
                b"BORAT_MDL",
                deflate(b"BORAT_MDL"),
            ),
            (
                "Borat Mod/data/menu/mc/savebg.tbn2",
                b"DATA_SAVEBG",
                deflate(b"DATA_SAVEBG"),
            ),
            (
                "Borat Mod/sh2e/menu/mc/savebg.tbn2",
                b"SH2E_SAVEBG",
                deflate(b"SH2E_SAVEBG"),
            ),
        ];

        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, raw, comp) in &entries {
            let offset = out.len() as u32;
            let name_bytes = name.as_bytes();
            let comp_sz = comp.len() as u32;
            let uncomp_sz = raw.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&8u16.to_le_bytes()); // DEFLATE
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&comp_sz.to_le_bytes());
            out.extend_from_slice(&uncomp_sz.to_le_bytes());
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(comp);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&8u16.to_le_bytes()); // DEFLATE
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&comp_sz.to_le_bytes());
            central.extend_from_slice(&uncomp_sz.to_le_bytes());
            central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name_bytes);
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        let count = entries.len() as u16;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());

        let tmp = std::env::temp_dir().join(format!(
            "sh2_borat_deflate_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        let mods_dir = tmp.join("mods");
        fs::create_dir_all(&mods_dir).unwrap();
        fs::write(mods_dir.join("borat.zip"), &out).unwrap();

        let pkgs = discover_mods(&mods_dir, &[]);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].overrides.len(), 3);

        let vfs = ModVfs::from_packages(&pkgs, None, tmp.join("data"));
        assert_eq!(
            vfs.read_mod_bytes("chr/jms/hhh_jms.mdl")
                .map(|(_, b)| b)
                .as_deref(),
            Some(b"BORAT_MDL".as_slice())
        );
        assert_eq!(
            vfs.read_mod_bytes("menu/mc/savebg.tbn2")
                .map(|(_, b)| b)
                .as_deref(),
            Some(b"SH2E_SAVEBG".as_slice())
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn vfs_resolves_dir_sh2e_data_root_priority_and_zip_backslashes() {
        let tmp = std::env::temp_dir().join(format!(
            "sh2_vfs_dir_priority_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        let mods_dir = tmp.join("mods");
        let dir_mod = mods_dir.join("dir_mod");
        fs::create_dir_all(dir_mod.join("sh2e/movie")).unwrap();
        fs::create_dir_all(dir_mod.join("data/movie")).unwrap();
        fs::create_dir_all(dir_mod.join("movie")).unwrap();
        fs::write(dir_mod.join("sh2e/movie/intro.mp4"), b"SH2E_MP4").unwrap();
        fs::write(dir_mod.join("data/movie/intro.mp4"), b"DATA_MP4").unwrap();
        fs::write(dir_mod.join("movie/intro.mp4"), b"ROOT_MP4").unwrap();
        fs::write(dir_mod.join("data/movie/ending.mp4"), b"DATA_ONLY_MP4").unwrap();

        let backslash_zip =
            build_test_stored_zip(&[(r"Borat Mod\sh2e\menu\mc\savebg.tbn2", b"BS_SAVEBG")]);
        fs::write(mods_dir.join("bs_mod.zip"), backslash_zip).unwrap();

        let pkgs = discover_mods(&mods_dir, &[]);
        let vfs = ModVfs::from_packages(&pkgs, None, tmp.join("data"));

        let resolved_intro = vfs.resolve_mod_file("movie/intro.mp4").unwrap();
        assert!(resolved_intro.ends_with("sh2e/movie/intro.mp4"));
        let disk_ending = vfs.resolve_disk_path("movie/ending.mp4").unwrap();
        assert!(disk_ending.ends_with("data/movie/ending.mp4"));
        assert_eq!(
            vfs.read_mod_bytes("menu/mc/savebg.tbn2")
                .map(|(_, b)| b)
                .as_deref(),
            Some(b"BS_SAVEBG".as_slice())
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    fn build_minimal_valid_mdl(bone_count: u32) -> Vec<u8> {
        let mut buf = vec![0u8; 0x40 + 0x98 + (bone_count as usize) * 65];
        buf[0x10..0x14].copy_from_slice(&0x40u32.to_le_bytes()); // mdl_ofs = 0x40
        buf[0x40..0x44].copy_from_slice(&[0x03, 0x00, 0xFF, 0xFF]); // MDL_MAGIC
        buf[0x48..0x4C].copy_from_slice(&0x98u32.to_le_bytes()); // init_ofs
        buf[0x4C..0x50].copy_from_slice(&bone_count.to_le_bytes());
        buf[0x50..0x54].copy_from_slice(&(0x98 + bone_count * 64).to_le_bytes()); // skel_ofs
        buf
    }

    #[test]
    fn texture_loader_load_model_with_hd_overrides_uses_mod_vfs_and_falls_back_on_corrupt() {
        let tmp = std::env::temp_dir().join(format!(
            "sh2_mdl_vfs_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        let mods_dir = tmp.join("mods");
        let data_dir = tmp.join("data");
        fs::create_dir_all(mods_dir.join("valid_mod/chr/jms")).unwrap();
        fs::create_dir_all(data_dir.join("chr/jms")).unwrap();

        let vanilla_mdl = build_minimal_valid_mdl(0);
        let mod_mdl = build_minimal_valid_mdl(1);
        fs::write(data_dir.join("chr/jms/hhh_jms.mdl"), &vanilla_mdl).unwrap();
        fs::write(mods_dir.join("valid_mod/chr/jms/hhh_jms.mdl"), &mod_mdl).unwrap();

        let pkgs = discover_mods(&mods_dir, &[]);
        let vfs = Arc::new(ModVfs::from_packages(&pkgs, None, data_dir.clone()));
        let loader = crate::formats::texture::TextureLoader::new(&data_dir, None::<&Path>)
            .with_mod_vfs(Arc::clone(&vfs));

        let (van, hd) = loader
            .load_model_with_hd_overrides("chr/jms/hhh_jms.mdl")
            .expect("valid mod .mdl should load");
        assert_eq!(van.bone_count, 1, "mod override should load bone_count = 1");
        assert!(hd.is_some());

        // Overwrite mod .mdl with corrupt bytes -> must fall back cleanly to vanilla .mdl
        fs::write(mods_dir.join("valid_mod/chr/jms/hhh_jms.mdl"), b"CORRUPT").unwrap();
        let (van_fb, hd_fb) = loader
            .load_model_with_hd_overrides("chr/jms/hhh_jms.mdl")
            .expect("corrupt mod .mdl must fall back to vanilla");
        assert_eq!(
            van_fb.bone_count, 0,
            "fallback should load vanilla bone_count = 0"
        );
        assert!(hd_fb.is_none());

        let _ = fs::remove_dir_all(&tmp);
    }
}
