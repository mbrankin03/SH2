//! Cross-platform save directory resolution, overrides, and legacy save migration (`SH2-143`).

use sh2_core::config::AppConfig;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Environment variable overriding the default save directory.
pub const SH2_SAVE_DIR_ENV: &str = "SH2_SAVE_DIR";

/// Default relative directory under the user's Documents folder on desktop platforms.
const DOCUMENTS_DIR_NAME: &str = "Documents";
const GAME_DIR_NAME: &str = "Silent Hill 2";
const SAVES_SUBDIR_NAME: &str = "saves";

static ANDROID_FILES_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Registers the Android application internal files directory (from `AndroidApp::internal_data_path()`).
pub fn set_android_files_dir(path: PathBuf) {
    let _ = ANDROID_FILES_DIR.set(path);
}

/// Returns the registered Android application files directory, if set.
#[must_use]
pub fn android_files_dir() -> Option<&'static Path> {
    ANDROID_FILES_DIR.get().map(PathBuf::as_path)
}

/// Target OS family for save directory resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePlatform {
    Linux,
    Windows,
    Android,
}

impl SavePlatform {
    /// Returns the compile-time target [`SavePlatform`].
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(target_os = "android")]
        {
            Self::Android
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(not(any(target_os = "android", target_os = "windows")))]
        {
            Self::Linux
        }
    }
}

/// Pure inputs for resolving the default save directory without reading global process state.
#[derive(Debug, Clone, Default)]
pub struct SaveDirEnv<'a> {
    /// Value of `SH2_SAVE_DIR` environment variable.
    pub env_save_dir: Option<&'a str>,
    /// Value of `game.save_dir_override` from `config.json`.
    pub config_save_dir: Option<&'a str>,
    /// `HOME` environment variable (Linux/Unix).
    pub home_dir: Option<&'a str>,
    /// `USERPROFILE` environment variable (Windows).
    pub user_profile: Option<&'a str>,
    /// Android app internal files directory.
    pub android_files_dir: Option<&'a Path>,
}

fn expand_tilde(raw: &str, home: Option<&str>) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(h) = home.map(str::trim).filter(|s| !s.is_empty()) {
            return PathBuf::from(h).join(rest);
        }
    } else if trimmed == "~" {
        if let Some(h) = home.map(str::trim).filter(|s| !s.is_empty()) {
            return PathBuf::from(h);
        }
    }
    PathBuf::from(trimmed)
}

/// Resolves the target save directory for `platform` given `env` overrides and home paths.
///
/// Precedence:
/// 1. `SH2_SAVE_DIR` environment variable (if non-empty)
/// 2. `config.json` `game.save_dir_override` (if non-empty)
/// 3. Platform default:
///    - Linux / Windows: `~/Documents/Silent Hill 2/saves`
///    - Android: `<android_files_dir>/saves`
#[must_use]
pub fn resolve_default_save_dir(platform: SavePlatform, env: &SaveDirEnv<'_>) -> PathBuf {
    let home_for_tilde = match platform {
        SavePlatform::Windows => env.user_profile.or(env.home_dir),
        SavePlatform::Linux | SavePlatform::Android => env.home_dir,
    };

    if let Some(raw) = env.env_save_dir.map(str::trim).filter(|s| !s.is_empty()) {
        return expand_tilde(raw, home_for_tilde);
    }

    if let Some(raw) = env.config_save_dir.map(str::trim).filter(|s| !s.is_empty()) {
        return expand_tilde(raw, home_for_tilde);
    }

    match platform {
        SavePlatform::Linux => {
            if let Some(home) = env.home_dir.map(str::trim).filter(|s| !s.is_empty()) {
                PathBuf::from(home)
                    .join(DOCUMENTS_DIR_NAME)
                    .join(GAME_DIR_NAME)
                    .join(SAVES_SUBDIR_NAME)
            } else {
                PathBuf::from(SAVES_SUBDIR_NAME)
            }
        }
        SavePlatform::Windows => {
            if let Some(profile) = env
                .user_profile
                .or(env.home_dir)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                PathBuf::from(profile)
                    .join(DOCUMENTS_DIR_NAME)
                    .join(GAME_DIR_NAME)
                    .join(SAVES_SUBDIR_NAME)
            } else {
                PathBuf::from(SAVES_SUBDIR_NAME)
            }
        }
        SavePlatform::Android => {
            if let Some(files_dir) = env.android_files_dir {
                files_dir.join(SAVES_SUBDIR_NAME)
            } else {
                PathBuf::from(SAVES_SUBDIR_NAME)
            }
        }
    }
}

/// Returns `true` if `file_name` matches a legacy save or clear-data filename.
#[must_use]
pub fn is_legacy_save_filename(file_name: &str) -> bool {
    file_name == "save_slot0.json"
        || file_name == sh2_core::campaign::CLEAR_DATA_FILENAME
        || file_name == sh2_core::campaign::QUICKSAVE_FILENAME
        || file_name == sh2_core::campaign::AUTOSAVE_FILENAME
        || (file_name.starts_with("save_card")
            && file_name.contains("_file")
            && file_name.ends_with(".json"))
}

/// Returns the legacy save directory adjacent to `config_path`.
#[must_use]
pub fn legacy_save_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn collect_legacy_save_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(is_legacy_save_filename)
        })
        .collect()
}

/// Migrates any legacy save/clear_data files from `legacy_dir` into `target_dir` when
/// `target_dir != legacy_dir`.
///
/// Existing files in `target_dir` are never overwritten. If creating `target_dir` or copying
/// a legacy file fails, logs a warning and falls back to `legacy_dir` so existing saves are
/// never lost.
#[must_use]
pub fn migrate_or_fallback_save_dir(target_dir: &Path, legacy_dir: &Path) -> PathBuf {
    if target_dir == legacy_dir {
        return target_dir.to_path_buf();
    }

    let legacy_files = collect_legacy_save_files(legacy_dir);
    if legacy_files.is_empty() {
        return target_dir.to_path_buf();
    }

    if let Err(err) = fs::create_dir_all(target_dir) {
        tracing::warn!(
            "Failed to create save directory {:?} ({}); falling back to legacy directory {:?}",
            target_dir,
            err,
            legacy_dir
        );
        return legacy_dir.to_path_buf();
    }

    for src in legacy_files {
        let Some(file_name) = src.file_name() else {
            continue;
        };
        let dest = target_dir.join(file_name);
        if dest.exists() {
            continue;
        }
        match fs::copy(&src, &dest) {
            Ok(_) => {
                tracing::info!("Migrated legacy save file {:?} -> {:?}", src, dest);
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to migrate legacy save {:?} -> {:?} ({}); falling back to {:?}",
                    src,
                    dest,
                    err,
                    legacy_dir
                );
                return legacy_dir.to_path_buf();
            }
        }
    }

    target_dir.to_path_buf()
}

/// Resolves the active save directory for `config_path` and `config`, applying `SH2_SAVE_DIR`,
/// `config.game.save_dir_override`, platform defaults, and legacy save migration/fallback.
#[must_use]
pub fn resolve_save_dir(config_path: &Path, config: &AppConfig) -> PathBuf {
    let env_save_dir = std::env::var(SH2_SAVE_DIR_ENV).ok();
    let home_dir = std::env::var("HOME").ok();
    let user_profile = std::env::var("USERPROFILE").ok();
    let env = SaveDirEnv {
        env_save_dir: env_save_dir.as_deref(),
        config_save_dir: config.game.save_dir_override.as_deref(),
        home_dir: home_dir.as_deref(),
        user_profile: user_profile.as_deref(),
        android_files_dir: android_files_dir(),
    };
    let target_dir = resolve_default_save_dir(SavePlatform::current(), &env);
    let legacy_dir = legacy_save_dir(config_path);
    migrate_or_fallback_save_dir(&target_dir, &legacy_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sh2_143_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    #[test]
    fn resolves_linux_default_documents_saves() {
        let env = SaveDirEnv {
            home_dir: Some("/home/james"),
            ..Default::default()
        };
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Linux, &env),
            PathBuf::from("/home/james/Documents/Silent Hill 2/saves")
        );
    }

    #[test]
    fn resolves_windows_default_documents_saves() {
        let env = SaveDirEnv {
            user_profile: Some("C:\\Users\\James"),
            ..Default::default()
        };
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Windows, &env),
            PathBuf::from("C:\\Users\\James")
                .join("Documents")
                .join("Silent Hill 2")
                .join("saves")
        );
    }

    #[test]
    fn resolves_android_app_files_dir() {
        let android_dir = Path::new("/data/user/0/org.sh2.port/files");
        let env = SaveDirEnv {
            android_files_dir: Some(android_dir),
            ..Default::default()
        };
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Android, &env),
            PathBuf::from("/data/user/0/org.sh2.port/files/saves")
        );
    }

    #[test]
    fn env_override_takes_precedence_over_config_and_platform() {
        let env = SaveDirEnv {
            env_save_dir: Some("/mnt/saves/sh2"),
            config_save_dir: Some("/custom/config/saves"),
            home_dir: Some("/home/james"),
            user_profile: Some("C:\\Users\\James"),
            android_files_dir: Some(Path::new("/data/user/0/org.sh2.port/files")),
        };
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Linux, &env),
            PathBuf::from("/mnt/saves/sh2")
        );
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Windows, &env),
            PathBuf::from("/mnt/saves/sh2")
        );
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Android, &env),
            PathBuf::from("/mnt/saves/sh2")
        );
    }

    #[test]
    fn config_override_used_when_env_unset_and_expands_tilde() {
        let env = SaveDirEnv {
            env_save_dir: Some("   "),
            config_save_dir: Some("~/custom_sh2_saves"),
            home_dir: Some("/home/james"),
            ..Default::default()
        };
        assert_eq!(
            resolve_default_save_dir(SavePlatform::Linux, &env),
            PathBuf::from("/home/james/custom_sh2_saves")
        );
    }

    #[test]
    fn legacy_migration_copies_existing_saves_without_overwriting() {
        let root = temp_test_dir("migrate");
        let legacy_dir = root.join("legacy");
        let target_dir = root.join("target");
        fs::create_dir_all(&legacy_dir).expect("create legacy");
        fs::create_dir_all(&target_dir).expect("create target");

        fs::write(legacy_dir.join("save_slot0.json"), "legacy_slot0").expect("write slot0");
        fs::write(legacy_dir.join("save_card0_file01.json"), "legacy_c0f1").expect("write c0f1");
        fs::write(legacy_dir.join("clear_data.json"), "legacy_clear").expect("write clear");
        fs::write(legacy_dir.join("unrelated.txt"), "ignore").expect("write unrelated");
        fs::write(target_dir.join("save_card0_file01.json"), "newer_c0f1").expect("write newer");

        let resolved = migrate_or_fallback_save_dir(&target_dir, &legacy_dir);
        assert_eq!(resolved, target_dir);
        assert_eq!(
            fs::read_to_string(target_dir.join("save_slot0.json")).expect("read slot0"),
            "legacy_slot0"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("clear_data.json")).expect("read clear"),
            "legacy_clear"
        );
        assert_eq!(
            fs::read_to_string(target_dir.join("save_card0_file01.json")).expect("read c0f1"),
            "newer_c0f1"
        );
        assert!(!target_dir.join("unrelated.txt").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_fallback_triggers_when_target_dir_cannot_be_created() {
        let root = temp_test_dir("fallback");
        let legacy_dir = root.join("legacy");
        fs::create_dir_all(&legacy_dir).expect("create legacy");
        fs::write(legacy_dir.join("save_slot0.json"), "legacy_slot0").expect("write slot0");

        // A regular file as parent of target_dir forces create_dir_all to fail cross-platform.
        let blocking_file = root.join("not_a_dir");
        fs::write(&blocking_file, "blocker").expect("write blocker");
        let impossible_target = blocking_file.join("saves");

        let resolved = migrate_or_fallback_save_dir(&impossible_target, &legacy_dir);
        assert_eq!(resolved, legacy_dir);

        let _ = fs::remove_dir_all(&root);
    }
}
