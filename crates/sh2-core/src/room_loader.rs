//! Automated Room & Stage `.cam` Trigger Volume Hook (`SH2-50`).
//!
//! Maps Silent Hill 2 retail stage IDs (`1..=58`), room identifiers (`"er01"`, `"ob01"`, `"rr21"`,
//! `"ap01"`, `"ma61"`, etc.), and relative map/camera paths into canonical `.cam` room identifiers,
//! resolving SH2:EE widescreen/high-framerate override files (`sh2e/bg/<area>/<room>.cam` and
//! `sh2e/bg2/<area>/<room>.cam`) before falling back to vanilla retail `data/bg/...` (`SH2-50`).

use sh2_assets::formats::cam::normalize_room_cam_rel_path;

/// Descriptor for a stage's default entry room and canonical `.cam` path (`SH2-50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageRoomDescriptor {
    /// Retail stage index (`1..=58`).
    pub stage_id: u16,
    /// Stage display name.
    pub stage_name: &'static str,
    /// Canonical default room identifier (e.g. `"er01"`, `"ob01"`, `"ap01"`, `"th09"`, `"ma01"`).
    pub default_room_id: &'static str,
    /// Canonical relative `.cam` path under `data/` or `sh2e/` (e.g. `"bg/er/er01.cam"`, `"bg2/ma/ma01.cam"`).
    pub default_cam_rel_path: &'static str,
}

/// Returns the canonical [`StageRoomDescriptor`] for retail stages `1..=58`
/// (*Letter from Silent Heaven* stages `1..=52` and *Born from a Wish* stages `53..=58`).
#[must_use]
pub const fn stage_room_descriptor(stage_id: u16) -> Option<StageRoomDescriptor> {
    match stage_id {
        1 => Some(StageRoomDescriptor {
            stage_id: 1,
            stage_name: "Observation Deck (Prologue)",
            default_room_id: "ob01",
            default_cam_rel_path: "bg/ob/ob01.cam",
        }),
        2 => Some(StageRoomDescriptor {
            stage_id: 2,
            stage_name: "Opening Restroom",
            default_room_id: "er01",
            default_cam_rel_path: "bg/er/er01.cam",
        }),
        3 => Some(StageRoomDescriptor {
            stage_id: 3,
            stage_name: "Observation Deck",
            default_room_id: "ob01",
            default_cam_rel_path: "bg/ob/ob01.cam",
        }),
        4 => Some(StageRoomDescriptor {
            stage_id: 4,
            stage_name: "Forest Trail & Graveyard",
            default_room_id: "ca11",
            default_cam_rel_path: "bg/ca/ca11.cam",
        }),
        5 => Some(StageRoomDescriptor {
            stage_id: 5,
            stage_name: "Silent Hill Town East",
            default_room_id: "cb54",
            default_cam_rel_path: "bg/cb/cb54.cam",
        }),
        6 => Some(StageRoomDescriptor {
            stage_id: 6,
            stage_name: "Wood Side Apartments 1F",
            default_room_id: "ap01",
            default_cam_rel_path: "bg/ap/ap01.cam",
        }),
        7 => Some(StageRoomDescriptor {
            stage_id: 7,
            stage_name: "Wood Side Apartments 2F",
            default_room_id: "ap34",
            default_cam_rel_path: "bg/ap/ap34.cam",
        }),
        8 => Some(StageRoomDescriptor {
            stage_id: 8,
            stage_name: "Wood Side Apartments 3F West",
            default_room_id: "ap104",
            default_cam_rel_path: "bg/ap/ap104.cam",
        }),
        9 => Some(StageRoomDescriptor {
            stage_id: 9,
            stage_name: "Wood Side Apartments 3F East",
            default_room_id: "ap18",
            default_cam_rel_path: "bg/ap/ap18.cam",
        }),
        10 => Some(StageRoomDescriptor {
            stage_id: 10,
            stage_name: "Blue Creek Apartments 1F",
            default_room_id: "ap84",
            default_cam_rel_path: "bg/ap/ap84.cam",
        }),
        11 => Some(StageRoomDescriptor {
            stage_id: 11,
            stage_name: "Blue Creek Apartments 2F",
            default_room_id: "ap58",
            default_cam_rel_path: "bg/ap/ap58.cam",
        }),
        12 => Some(StageRoomDescriptor {
            stage_id: 12,
            stage_name: "Blue Creek Apartments Stairwell",
            default_room_id: "ap108",
            default_cam_rel_path: "bg/ap/ap108.cam",
        }),
        13 => Some(StageRoomDescriptor {
            stage_id: 13,
            stage_name: "Blue Creek Apartments Exit",
            default_room_id: "cc04",
            default_cam_rel_path: "bg/cc/cc04.cam",
        }),
        14 => Some(StageRoomDescriptor {
            stage_id: 14,
            stage_name: "Silent Hill Town West & Rosewater Park",
            default_room_id: "cc11",
            default_cam_rel_path: "bg/cc/cc11.cam",
        }),
        15 => Some(StageRoomDescriptor {
            stage_id: 15,
            stage_name: "Pete's Bowl-O-Rama",
            default_room_id: "bw01",
            default_cam_rel_path: "bg/bw/bw01.cam",
        }),
        16 => Some(StageRoomDescriptor {
            stage_id: 16,
            stage_name: "Heaven's Night Alley",
            default_room_id: "th01",
            default_cam_rel_path: "bg/th/th01.cam",
        }),
        17 => Some(StageRoomDescriptor {
            stage_id: 17,
            stage_name: "Heaven's Night Interior",
            default_room_id: "th09",
            default_cam_rel_path: "bg/th/th09.cam",
        }),
        18 => Some(StageRoomDescriptor {
            stage_id: 18,
            stage_name: "Brookhaven Hospital 1F",
            default_room_id: "hp01",
            default_cam_rel_path: "bg/hp/hp01.cam",
        }),
        19 => Some(StageRoomDescriptor {
            stage_id: 19,
            stage_name: "Brookhaven Hospital 2F",
            default_room_id: "hp25",
            default_cam_rel_path: "bg/hp/hp25.cam",
        }),
        20 => Some(StageRoomDescriptor {
            stage_id: 20,
            stage_name: "Brookhaven Hospital 3F",
            default_room_id: "hp61",
            default_cam_rel_path: "bg/hp/hp61.cam",
        }),
        21 => Some(StageRoomDescriptor {
            stage_id: 21,
            stage_name: "Brookhaven Hospital Roof",
            default_room_id: "hp97",
            default_cam_rel_path: "bg/hp/hp97.cam",
        }),
        22 => Some(StageRoomDescriptor {
            stage_id: 22,
            stage_name: "Brookhaven Hospital 1F (Otherworld)",
            default_room_id: "hp101",
            default_cam_rel_path: "bg/hp/hp101.cam",
        }),
        23 => Some(StageRoomDescriptor {
            stage_id: 23,
            stage_name: "Brookhaven Hospital 2F (Otherworld)",
            default_room_id: "hp121",
            default_cam_rel_path: "bg/hp/hp121.cam",
        }),
        24 => Some(StageRoomDescriptor {
            stage_id: 24,
            stage_name: "Brookhaven Hospital 3F (Otherworld)",
            default_room_id: "hp150",
            default_cam_rel_path: "bg/hp/hp150.cam",
        }),
        25 => Some(StageRoomDescriptor {
            stage_id: 25,
            stage_name: "Brookhaven Hospital Basement (Otherworld)",
            default_room_id: "hp186",
            default_cam_rel_path: "bg/hp/hp186.cam",
        }),
        26 => Some(StageRoomDescriptor {
            stage_id: 26,
            stage_name: "Brookhaven Hospital 1F East (Otherworld)",
            default_room_id: "hp202",
            default_cam_rel_path: "bg/hp/hp202.cam",
        }),
        27 => Some(StageRoomDescriptor {
            stage_id: 27,
            stage_name: "Brookhaven Hospital Stairwell",
            default_room_id: "hp218",
            default_cam_rel_path: "bg/hp/hp218.cam",
        }),
        28 => Some(StageRoomDescriptor {
            stage_id: 28,
            stage_name: "Brookhaven Basement Chase",
            default_room_id: "hp234",
            default_cam_rel_path: "bg/hp/hp234.cam",
        }),
        29 => Some(StageRoomDescriptor {
            stage_id: 29,
            stage_name: "Silent Hill Town Dark",
            default_room_id: "cd01",
            default_cam_rel_path: "bg/cd/cd01.cam",
        }),
        30 => Some(StageRoomDescriptor {
            stage_id: 30,
            stage_name: "Silent Hill Historical Society",
            default_room_id: "ps01",
            default_cam_rel_path: "bg/ps/ps01.cam",
        }),
        31 => Some(StageRoomDescriptor {
            stage_id: 31,
            stage_name: "Toluca Prison Spiral Descent",
            default_room_id: "ps21",
            default_cam_rel_path: "bg/ps/ps21.cam",
        }),
        32 => Some(StageRoomDescriptor {
            stage_id: 32,
            stage_name: "Toluca Prison North",
            default_room_id: "ps45",
            default_cam_rel_path: "bg/ps/ps45.cam",
        }),
        33 => Some(StageRoomDescriptor {
            stage_id: 33,
            stage_name: "Toluca Prison South",
            default_room_id: "ps66",
            default_cam_rel_path: "bg/ps/ps66.cam",
        }),
        34 => Some(StageRoomDescriptor {
            stage_id: 34,
            stage_name: "Labyrinth West",
            default_room_id: "ps101",
            default_cam_rel_path: "bg/ps/ps101.cam",
        }),
        35 => Some(StageRoomDescriptor {
            stage_id: 35,
            stage_name: "Labyrinth East",
            default_room_id: "ps97",
            default_cam_rel_path: "bg/ps/ps97.cam",
        }),
        36 => Some(StageRoomDescriptor {
            stage_id: 36,
            stage_name: "Labyrinth North",
            default_room_id: "ps141",
            default_cam_rel_path: "bg/ps/ps141.cam",
        }),
        37 => Some(StageRoomDescriptor {
            stage_id: 37,
            stage_name: "Meat Locker (Eddie Boss)",
            default_room_id: "ps189",
            default_cam_rel_path: "bg/ps/ps189.cam",
        }),
        38 => Some(StageRoomDescriptor {
            stage_id: 38,
            stage_name: "Toluca Lake",
            default_room_id: "qt01",
            default_cam_rel_path: "bg/qt/qt01.cam",
        }),
        39 => Some(StageRoomDescriptor {
            stage_id: 39,
            stage_name: "Lakeview Hotel Garden",
            default_room_id: "rr01",
            default_cam_rel_path: "bg/rr/rr01.cam",
        }),
        40 => Some(StageRoomDescriptor {
            stage_id: 40,
            stage_name: "Lakeview Hotel 1F",
            default_room_id: "rr25",
            default_cam_rel_path: "bg/rr/rr25.cam",
        }),
        41 => Some(StageRoomDescriptor {
            stage_id: 41,
            stage_name: "Lakeview Hotel 2F",
            default_room_id: "rr49",
            default_cam_rel_path: "bg/rr/rr49.cam",
        }),
        42 => Some(StageRoomDescriptor {
            stage_id: 42,
            stage_name: "Lakeview Hotel 3F (Room 312)",
            default_room_id: "rr91",
            default_cam_rel_path: "bg/rr/rr91.cam",
        }),
        43 => Some(StageRoomDescriptor {
            stage_id: 43,
            stage_name: "Lakeview Hotel Basement",
            default_room_id: "rr21",
            default_cam_rel_path: "bg/rr/rr21.cam",
        }),
        44 => Some(StageRoomDescriptor {
            stage_id: 44,
            stage_name: "Lakeview Hotel 1F (Otherworld)",
            default_room_id: "ru01",
            default_cam_rel_path: "bg/ru/ru01.cam",
        }),
        45 => Some(StageRoomDescriptor {
            stage_id: 45,
            stage_name: "Lakeview Hotel 2F (Otherworld)",
            default_room_id: "ru25",
            default_cam_rel_path: "bg/ru/ru25.cam",
        }),
        46 => Some(StageRoomDescriptor {
            stage_id: 46,
            stage_name: "Lakeview Hotel 3F (Otherworld)",
            default_room_id: "ru53",
            default_cam_rel_path: "bg/ru/ru53.cam",
        }),
        47..=52 => Some(StageRoomDescriptor {
            stage_id,
            stage_name: "Lakeview Hotel Final Boss",
            default_room_id: "ru73",
            default_cam_rel_path: "bg/ru/ru73.cam",
        }),
        53 => Some(StageRoomDescriptor {
            stage_id: 53,
            stage_name: "Heaven's Night",
            default_room_id: "th09",
            default_cam_rel_path: "bg/th/th09.cam",
        }),
        54 => Some(StageRoomDescriptor {
            stage_id: 54,
            stage_name: "Ernest Study End",
            default_room_id: "ma69",
            default_cam_rel_path: "bg2/ma/ma69.cam",
        }),
        55 => Some(StageRoomDescriptor {
            stage_id: 55,
            stage_name: "Baldwin Mansion 1F",
            default_room_id: "ma01",
            default_cam_rel_path: "bg2/ma/ma01.cam",
        }),
        56 => Some(StageRoomDescriptor {
            stage_id: 56,
            stage_name: "Baldwin Mansion 2F",
            default_room_id: "ma61",
            default_cam_rel_path: "bg2/ma/ma61.cam",
        }),
        57 => Some(StageRoomDescriptor {
            stage_id: 57,
            stage_name: "Blue Creek Transition",
            default_room_id: "ap64",
            default_cam_rel_path: "bg/ap/ap64.cam",
        }),
        58 => Some(StageRoomDescriptor {
            stage_id: 58,
            stage_name: "Baldwin Mansion Attic",
            default_room_id: "ma85",
            default_cam_rel_path: "bg2/ma/ma85.cam",
        }),
        _ => None,
    }
}

/// Returns the canonical default room identifier (`"er01"`, `"ob01"`, `"ap01"`, `"ma61"`, etc.)
/// for a retail `stage_id` (`1..=58`).
#[inline]
#[must_use]
pub const fn default_room_id_for_stage(stage_id: u16) -> Option<&'static str> {
    match stage_room_descriptor(stage_id) {
        Some(desc) => Some(desc.default_room_id),
        None => None,
    }
}

/// Normalizes an explicit room identifier or falls back to the stage's canonical default room.
#[must_use]
pub fn resolve_room_id_for_stage_or_override(
    stage_id: u16,
    room_override: Option<&str>,
) -> Option<String> {
    if let Some(ov) = room_override {
        if let Some((room_id, _)) = normalize_room_cam_rel_path(ov) {
            return Some(room_id);
        }
    }
    default_room_id_for_stage(stage_id).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::AssetPathResolver;

    #[test]
    fn test_all_58_stages_resolve_to_existing_retail_cam_files() {
        let resolver = AssetPathResolver::resolve();
        if !resolver.data_dir.exists() {
            return;
        }

        for stage_id in 1..=58u16 {
            let desc = stage_room_descriptor(stage_id)
                .unwrap_or_else(|| panic!("Stage {stage_id} must have a StageRoomDescriptor"));
            let loaded = resolver
                .load_cam_for_room(desc.default_room_id, true)
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to parse .cam for stage {stage_id} ({}): {e}",
                        desc.default_room_id
                    )
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Missing .cam on disk for stage {stage_id} ({})",
                        desc.default_room_id
                    )
                });
            assert_eq!(loaded.room_id, desc.default_room_id);
            assert_eq!(loaded.relative_path, desc.default_cam_rel_path);
            assert!(!loaded.cam_file.records.is_empty());
        }
    }
}
