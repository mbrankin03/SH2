//! Unified retail stage asset manifest resolver bridging `stages_act1` (Stages 1–27),
//! `stages_act2` (Stages 28–58), `room_loader` (`.cam`), `.map`, `.cld`, `Enemy_List`,
//! and 3D `Item_List` (`0x04xx`) pickups (`connect.c` `0x0020bd60..0x0020bf30`).

use crate::events::stages_act1::{act1_stage_descriptor, decode_act1_enemy_spawn};
use crate::events::stages_act2::stage_table_act2;
use crate::room_loader::{resolve_room_id_for_stage_or_override, stage_room_descriptor};
use crate::world::outdoor_grid::OutdoorArea;

/// Normalized enemy spawn entry extracted from a stage's retail `Enemy_List` (`en_list`)
/// for consumption by `connectRoomInit` (`0x0020be10`) / `ConnectCharaWorkAdminIn` (`0x001cb8c0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CampaignEnemySpawn {
    /// Team Silent creature kind ID (`0x0201` Lying Figure, `0x0202` Mannequin, `0x0203` Nurse, etc.).
    pub kind: i16,
    /// Per-stage instance index (`0..`).
    pub id: i16,
    /// World position `[x, y, z]` in PS2 millimeters.
    pub position_mm: [f32; 3],
    /// Initial yaw rotation in raw Q12/i16 units.
    pub rot_y_raw: i16,
    /// Initial AI status/posture word.
    pub status: i16,
    /// Spawn difficulty/story condition bitfield.
    pub condition: u16,
}

/// Normalized 3D room item pickup entry (`0x04xx`) extracted from a stage's retail `Item_List`
/// (`gi_list`, `SLUS_202.28` `ItemListElement` `0x001a9320`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CampaignPickupSpawn {
    /// World position `[x, y, z]` in PS2 millimeters.
    pub position_mm: [f32; 3],
    /// Yaw rotation in raw i16 units.
    pub rot_y_raw: i16,
    /// Index into `ITEM_KIND_TABLE` (`(st >> 29) & 7`).
    pub item_kind_index: u8,
    /// Action-difficulty filter (`(st >> 26) & 7`).
    pub difficulty_condition: u8,
    /// Pickup event flag ID (`st & 0x3fff`).
    pub flag_id: u16,
}

/// Complete asset & entity manifest emitted by [`resolve_stage_manifest`] when entering a
/// stage/room during `connectMain` (`0x0020bf80`).
#[derive(Debug, Clone, PartialEq)]
pub struct RoomLoadManifest {
    /// 1-based retail stage ID (`1..=58`).
    pub stage_id: u16,
    /// Canonical stage display name.
    pub stage_name: &'static str,
    /// Canonical lowercase room identifier (e.g. `"er01"`, `"ob01"`, `"ap01"`, `"th09"`).
    pub room_id: String,
    /// Relative `.map` path under `data/` (e.g. `"bg/er/er01.map"`, `"bg2/ma/ma01.map"`).
    pub map_rel_path: String,
    /// Relative `.cld` path under `data/` (e.g. `"bg/er/er01.cld"`).
    pub cld_rel_path: String,
    /// Relative `.cam` path under `data/` or `sh2e/` (e.g. `"bg/er/er01.cam"`).
    pub cam_rel_path: String,
    /// `true` when `stage_id` belongs to an outdoor multi-tile area (`BgIsOut` `0x001b0cd0`).
    pub is_outdoor: bool,
    /// `true` when transitioning across a connected sub-stage pair (`CONNECT_STAGE_PAIRS`).
    pub is_connected_stage: bool,
    /// `true` when `NowLoadingEnable` (`0x00298fa0`) should display the PS2 footstep loading screen.
    pub show_loading_screen: bool,
    /// Number of active `Event_List` entries in this stage (excluding `0x3fff` sentinel).
    pub event_count: usize,
    /// Retail `Enemy_List` spawns for this stage.
    pub enemy_spawns: Vec<CampaignEnemySpawn>,
    /// Retail 3D `Item_List` (`0x04xx`) room pickups for this stage.
    pub room_pickups: Vec<CampaignPickupSpawn>,
}

/// Derives the area folder prefix (`"bg/er"`, `"bg2/ma"`, etc.) for a canonical room ID.
///
/// *Born from a Wish* Baldwin Mansion rooms (`ma*`) live under `bg2/ma/`; all other retail
/// rooms live under `bg/<2-letter-prefix>/`.
#[must_use]
pub fn room_area_dir(room_id: &str) -> String {
    let lower = room_id.to_ascii_lowercase();
    let prefix: String = lower
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if prefix == "ma" {
        format!("bg2/{prefix}")
    } else if prefix.is_empty() {
        "bg/er".to_string()
    } else {
        format!("bg/{prefix}")
    }
}

/// Resolves the exact PS2 outdoor tile room identifier (e.g. `"ca04"`, `"cb29"`) for an outdoor
/// multi-tile stage (`BgIsOut` `0x001b0cd0`) from world coordinates `pos` (PS2 mm) using exact
/// `SLUS_202.28` `BlockNumber` (`0x001b0ae0..0x001b0b8c`) and area block tables (`0x002b9df0`+).
///
/// Returns `None` for indoor single-room stages or when `(gx, gz)` is outside the area's block table.
#[must_use]
pub fn resolve_outdoor_room_id(stage_id: u16, pos: [f32; 3]) -> Option<String> {
    let area = OutdoorArea::from_stage_id(stage_id)?;
    let (gx, gz) = OutdoorArea::world_to_grid(pos[0], pos[2]);
    let block_no = area.block_no_at(gx, gz)?;
    Some(format!("{}{block_no:02}", area.prefix()))
}

/// Resolves the canonical lowercase room identifier for `(stage_id, pos)` across BOTH outdoor
/// multi-tile stages ([`resolve_outdoor_room_id`], `BlockNumber` `0x001b09f0`) and indoor
/// multi-room stages ([`super::indoor_rooms::resolve_indoor_room_id`], retail `.cld` floor match).
#[must_use]
pub fn resolve_room_id_for_stage_pos(stage_id: u16, pos: [f32; 3]) -> Option<String> {
    resolve_outdoor_room_id(stage_id, pos)
        .or_else(|| super::indoor_rooms::resolve_indoor_room_id(stage_id, pos))
}

/// Resolves the unified [`RoomLoadManifest`] for `stage_id` (`1..=58`) and optional `room_override`.
///
/// Aggregates `stage_room_descriptor` (`.cam`), `.map`/`.cld` relative paths, outdoor area status
/// (`OutdoorArea::from_stage_id`), and retail `Event_List` / `Enemy_List` / `Item_List` tables
/// across both Act 1 (`stages_act1`, stages `1..=27`) and Act 2 (`stages_act2`, stages `28..=58`).
#[must_use]
pub fn resolve_stage_manifest(
    stage_id: u16,
    room_override: Option<&str>,
    is_connected_stage: bool,
    show_loading_screen: bool,
) -> RoomLoadManifest {
    let desc = stage_room_descriptor(stage_id);
    let stage_name = desc.map_or("Unknown Stage", |d| d.stage_name);
    let room_id = resolve_room_id_for_stage_or_override(stage_id, room_override)
        .unwrap_or_else(|| "er01".to_string());
    let dir = room_area_dir(&room_id);
    let map_rel_path = format!("{dir}/{room_id}.map");
    let cld_rel_path = format!("{dir}/{room_id}.cld");
    let cam_rel_path = format!("{dir}/{room_id}.cam");
    let is_outdoor = OutdoorArea::from_stage_id(stage_id).is_some();

    let (event_count, enemy_spawns, room_pickups) = collect_stage_entities(stage_id);

    RoomLoadManifest {
        stage_id,
        stage_name,
        room_id,
        map_rel_path,
        cld_rel_path,
        cam_rel_path,
        is_outdoor,
        is_connected_stage,
        show_loading_screen,
        event_count,
        enemy_spawns,
        room_pickups,
    }
}

/// Extracts active `Event_List` count, `Enemy_List` spawns, and 3D `Item_List` pickups for any
/// retail stage `1..=58` from `stages_act1` / `stages_act2`.
fn collect_stage_entities(
    stage_id: u16,
) -> (usize, Vec<CampaignEnemySpawn>, Vec<CampaignPickupSpawn>) {
    // Prefer Act 2 tables for stages 28..=58, and Act 1 tables for stages 1..=27.
    if stage_id <= 27 {
        if let Some(act1) = act1_stage_descriptor(stage_id as u8) {
            let event_count = act1.event_list.len().saturating_sub(1);
            let enemies = act1
                .enemy_list
                .iter()
                .map(|raw| {
                    let e = decode_act1_enemy_spawn(raw);
                    CampaignEnemySpawn {
                        kind: e.kind,
                        id: e.id,
                        position_mm: [e.pos_x_mm as f32, e.pos_y_mm as f32, e.pos_z_mm as f32],
                        rot_y_raw: e.rot_y,
                        status: e.status,
                        condition: e.condition,
                    }
                })
                .collect();
            let pickups = act1
                .item_list
                .iter()
                .map(|gi| CampaignPickupSpawn {
                    position_mm: [gi.pos_x, gi.pos_y as f32, gi.pos_z],
                    rot_y_raw: gi.rot_y,
                    item_kind_index: gi.item_kind_index(),
                    difficulty_condition: gi.difficulty_condition(),
                    flag_id: gi.flag_id(),
                })
                .collect();
            return (event_count, enemies, pickups);
        }
    }

    if let Some(act2) = stage_table_act2(stage_id) {
        let event_count = act2.active_event_count();
        let enemies = act2
            .retail_enemies
            .iter()
            .map(|e| CampaignEnemySpawn {
                kind: e.kind,
                id: e.id,
                position_mm: e.position_mm(),
                rot_y_raw: e.rot_y,
                status: e.cond_type as i16,
                condition: e.diff_cond,
            })
            .collect();
        let pickups = act2
            .retail_items
            .iter()
            .map(|gi| CampaignPickupSpawn {
                position_mm: [gi.pos_x(), gi.pos_y(), gi.pos_z()],
                rot_y_raw: gi.rot_y_hi16 as i16,
                item_kind_index: gi.item_kind_index(),
                difficulty_condition: gi.difficulty_filter(),
                flag_id: gi.flag_id(),
            })
            .collect();
        return (event_count, enemies, pickups);
    }

    (0, Vec::new(), Vec::new())
}
