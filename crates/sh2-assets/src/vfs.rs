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
    build_stored_zip_central_directory_index, read_entry_bytes_from_stored_zip, ZipIndexCache,
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

    /// Resolves a relative asset path to an on-disk file across directory-backed mods, `sh2e/`,
    /// and `data/` (useful for streaming media such as `.sfd`, `.mp4`, `.mkv`, or `.bik`).
    #[must_use]
    pub fn resolve_disk_path(&self, rel_path: impl AsRef<Path>) -> Option<PathBuf> {
        let rel = normalize_rel_path(rel_path.as_ref());
        for m in &self.active_mods {
            if let ModSource::Directory(ref root) = m.source {
                if let Some(p) = resolve_in_dir_case_insensitive(root, &rel) {
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
                    if let Some(p) = resolve_in_dir_case_insensitive(root, &rel) {
                        if let Ok(bytes) = fs::read(&p) {
                            return Some((m.id.clone(), bytes));
                        }
                    }
                }
                ModSource::StoredZip(zip_path) => {
                    if let Some(bytes) =
                        read_entry_bytes_from_stored_zip(&self.zip_index_cache, zip_path, &rel_str)
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
            | "sfd"
            | "mp4"
            | "mkv"
            | "bik"
            | "sdm"
            | "wav"
            | "ogg"
            | "adx"
            | "aix"
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
            let cache: ZipIndexCache = Arc::new(Mutex::new(HashMap::new()));
            let mut metadata = read_entry_bytes_from_stored_zip(&cache, &path, "mod.json")
                .and_then(|b: Vec<u8>| serde_json::from_slice::<ModMetadata>(&b).ok())
                .unwrap_or_default();
            if metadata.name.is_empty() {
                metadata.name = id.clone();
            }
            let mut overrides = Vec::new();
            if let Some(index) = build_stored_zip_central_directory_index(&path) {
                for key in index.keys() {
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

        let packages = discover_mods(&mods_dir, &[]);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].id, "high_prio");
        assert_eq!(packages[0].metadata.name, "High Mod");
        assert_eq!(packages[0].overrides, vec!["pic/title.png".to_string()]);

        let vfs = ModVfs::from_packages(&packages, Some(sh2e_dir), data_dir);
        assert_eq!(
            vfs.read_bytes("pic/TITLE.PNG").as_deref(),
            Some(b"HIGH_MOD".as_slice())
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
}
