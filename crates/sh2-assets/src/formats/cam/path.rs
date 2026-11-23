//! Path normalization and resolved `.cam` asset metadata helpers.

use super::types::CamFile;

/// Indicates whether a `.cam` camera trigger volume file was resolved from the
/// Silent Hill 2: Enhanced Edition override folder (`sh2e/bg/...` or `sh2e/bg2/...`)
/// or the vanilla retail folder (`data/bg/...` or `data/bg2/...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CamSourceKind {
    /// Loaded from `sh2e/bg/<area>/<room>.cam` or `sh2e/bg2/<area>/<room>.cam`
    /// (widescreen/high-framerate corrected trigger volumes).
    Sh2eeOverride,
    /// Loaded from vanilla retail `data/bg/<area>/<room>.cam` or `data/bg2/<area>/<room>.cam`.
    #[default]
    VanillaRetail,
}

impl CamSourceKind {
    /// Short display label suitable for HUD telemetry and logging.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sh2eeOverride => "SH2:EE Override",
            Self::VanillaRetail => "Vanilla Retail",
        }
    }
}

/// Normalizes a room identifier (`"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`, `"ccGB"`)
/// or relative stage path (`"bg/er/er01.map"`, `"data/bg2/ma/ma61.cam"`, `"sh2e/bg/ap/ap01.cam"`)
/// into a canonical relative `.cam` path (`"bg/<area>/<room>.cam"` or `"bg2/ma/<room>.cam"`).
///
/// Returns `(canonical_room_id, relative_cam_path)` on valid input, or `None` if the identifier
/// does not contain at least a 2-letter area prefix.
pub fn normalize_room_cam_rel_path(room_or_rel_path: &str) -> Option<(String, String)> {
    let trimmed = room_or_rel_path
        .trim()
        .trim_start_matches("data/")
        .trim_start_matches("sh2e/")
        .trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    // Extract the bare filename stem (e.g. "bg/er/er01.map" -> "er01", "ma61.cam" -> "ma61")
    let file_part = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let stem = file_part
        .strip_suffix(".cam")
        .or_else(|| file_part.strip_suffix(".CAM"))
        .or_else(|| file_part.strip_suffix(".map"))
        .or_else(|| file_part.strip_suffix(".MAP"))
        .or_else(|| file_part.strip_suffix(".cld"))
        .or_else(|| file_part.strip_suffix(".CLD"))
        .unwrap_or(file_part);

    if stem.len() < 2
        || !stem.as_bytes()[0..2]
            .iter()
            .all(|b| b.is_ascii_alphabetic())
    {
        return None;
    }

    let area = stem[0..2].to_ascii_lowercase();
    // Preserve canonical room ID (lowercase area + suffix; note caGB/cbGB/ccGB/cdGB exist on disk)
    let room_id = if stem.len() == 4 && stem[2..4].eq_ignore_ascii_case("gb") {
        format!("{area}GB")
    } else {
        stem.to_ascii_lowercase()
    };

    let bg_root = if area == "ma" || trimmed.starts_with("bg2/") {
        "bg2"
    } else {
        "bg"
    };

    let rel_path = format!("{bg_root}/{area}/{room_id}.cam");
    Some((room_id.to_ascii_lowercase(), rel_path))
}

/// Returns the optional global area `.cam` relative path (`"bg/<area>/<area>GB.cam"`) for town
/// outdoor areas (`ca`, `cb`, `cc`, `cd`) when `room_or_rel_path` belongs to one of those areas
/// and is not already the `*GB.cam` file itself.
pub fn global_area_cam_rel_path(room_or_rel_path: &str) -> Option<String> {
    let (room_id, _) = normalize_room_cam_rel_path(room_or_rel_path)?;
    if room_id.ends_with("gb") || room_id.len() < 2 {
        return None;
    }
    let area = &room_id[0..2];
    match area {
        "ca" | "cb" | "cc" | "cd" => Some(format!("bg/{area}/{area}GB.cam")),
        _ => None,
    }
}

/// Resolved and parsed `.cam` asset along with its room identifier, source pack, and disk path (`SH2-50`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCamAsset {
    /// Canonical lowercase room identifier (e.g. `"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`).
    pub room_id: String,
    /// Relative stage path under `data/` or `sh2e/` (e.g. `"bg/er/er01.cam"`, `"bg2/ma/ma61.cam"`).
    pub relative_path: String,
    /// Full resolved filesystem path.
    pub resolved_path: std::path::PathBuf,
    /// Whether the `.cam` file came from `sh2e/` (`Sh2eeOverride`) or `data/` (`VanillaRetail`).
    pub source: CamSourceKind,
    /// Parsed camera road records.
    pub cam_file: CamFile,
}
