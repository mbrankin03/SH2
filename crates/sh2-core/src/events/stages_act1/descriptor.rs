//! Stage table descriptor types (`Act1StageDescriptor`, `Act1StageItemEntry`, `Act1EnemySpawn`),
//! binary decoders (`decode_act1_enemy_spawn`, `decode_act1_ev_pos_anchor`), and `const fn`
//! table builders shared by Act 1 static stage modules.

use super::Act1StageId;
use crate::types::{
    char_to_float2, EnemyListEntry, EventListEntry, EventShapeKind, ItemListEntry, ModelListEntry,
    StageDataHeader,
};

/// Exact 16-byte retail `Item_List` (`gi_list`) pickup record (`SLUS_202.28` `ItemListElement` `0x001a9320`,
/// `sh2pc.exe` `.data`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Act1StageItemEntry {
    /// World X coordinate in PS2 millimeters (`CharToFloat4`).
    pub pos_x: f32,
    /// World Z coordinate in PS2 millimeters (`CharToFloat4`).
    pub pos_z: f32,
    /// World Y coordinate in PS2 millimeters (`i16`).
    pub pos_y: i16,
    /// Item yaw angle in Q12 / raw units (`i16`).
    pub rot_y: i16,
    /// Packed status/condition bitfield (`(st >> 29) & 7` = `ITEM_KIND_TABLE` index, `st & 0x3fff` = `flag_id`).
    pub st: u32,
}

impl Act1StageItemEntry {
    /// Index into `ITEM_KIND_TABLE` (`(st >> 29) & 7`).
    #[must_use]
    pub const fn item_kind_index(&self) -> u8 {
        ((self.st >> 29) & 0x7) as u8
    }

    /// Action-difficulty filter (`(st >> 26) & 7`).
    #[must_use]
    pub const fn difficulty_condition(&self) -> u8 {
        ((self.st >> 26) & 0x7) as u8
    }

    /// Extra stage condition (`(st >> 20) & 0x1f`, e.g. `3` = Quiz Chest reward in Stage 25).
    #[must_use]
    pub const fn extra_condition(&self) -> u8 {
        ((self.st >> 20) & 0x1f) as u8
    }

    /// Base pickup flag ID (`st & 0x3fff`).
    #[must_use]
    pub const fn flag_id(&self) -> u16 {
        (self.st & 0x3fff) as u16
    }

    /// Projects this retail `gi_list` entry into [`ItemListEntry`] for `event_item_condition_check`.
    #[must_use]
    pub const fn to_item_list_entry(&self) -> ItemListEntry {
        let packed_flag = (self.item_kind_index() as u32)
            | ((self.difficulty_condition() as u32) << 8)
            | (((self.extra_condition() & 0x0f) as u32) << 12)
            | ((self.flag_id() as u32) << 16);
        ItemListEntry {
            flag: packed_flag,
            pos_x: self.pos_x.clamp(-32_768.0, 32_767.0) as i16,
            pos_y: self.pos_y,
            pos_z: self.pos_z.clamp(-32_768.0, 32_767.0) as i16,
            rot_x: 0,
            rot_y: (self.rot_y >> 8) as i8,
            rot_z: 0,
            pad: [0; 3],
        }
    }
}

/// Decoded 20-byte `Enemy_List` spawn record matching PS2 DWARF struct `0x77e1e` (`SLUS_202.28`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Act1EnemySpawn {
    /// Character/enemy kind ID (`0x0201` Lying Figure, `0x0202` Mannequin, `0x0203` Nurse, `0x0208`/`0x0209` Pyramid Head, etc.).
    pub kind: i16,
    /// Per-stage instance ID (`0..`).
    pub id: i16,
    /// World X coordinate in PS2 millimeters (`i32`, full outdoor range).
    pub pos_x_mm: i32,
    /// World Z coordinate in PS2 millimeters (`i32`, full outdoor range).
    pub pos_z_mm: i32,
    /// World Y coordinate in PS2 millimeters (`i16`).
    pub pos_y_mm: i16,
    /// Initial yaw rotation (`i16`).
    pub rot_y: i16,
    /// Initial AI wake/posture status (`i16`).
    pub status: i16,
    /// Spawn difficulty/story condition bitfield (`u16`).
    pub condition: u16,
}

/// Decodes an [`EnemyListEntry`] back into its lossless 20-byte PS2/PC [`Act1EnemySpawn`] fields.
#[must_use]
pub const fn decode_act1_enemy_spawn(entry: &EnemyListEntry) -> Act1EnemySpawn {
    let pos_x_mm = ((entry.x as u16 as u32) | ((entry.y as u16 as u32) << 16)) as i32;
    let pos_z_mm = ((entry.z as u16 as u32) | ((entry.rot as u16 as u32) << 16)) as i32;
    let pos_y_mm = u16::from_le_bytes([entry.room, entry.cond]) as i16;
    let rot_y = u16::from_le_bytes([entry.a_type, entry.b_type]) as i16;
    Act1EnemySpawn {
        kind: entry.kind,
        id: entry.id,
        pos_x_mm,
        pos_z_mm,
        pos_y_mm,
        rot_y,
        status: entry.flag_0 as i16,
        condition: entry.flag_1,
    }
}

/// Decodes the anchor `(x, y, z, param)` of a retail `ev_pos` record (`EventCheckLook` `0x001a8780`,
/// `EventCheckIn` `0x001a8d70`, `EventPositionSet` `0x001a9570` in `SLUS_202.28`:
/// 10-byte base `[f32 x (4B), i16 y (2B), f32 z (4B)]`, followed by shape-dependent `i16` deltas).
#[must_use]
pub fn decode_act1_ev_pos_anchor(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
) -> Option<([f32; 3], f32)> {
    let min_len = match shape {
        EventShapeKind::Point => 10,
        EventShapeKind::LineNegZ
        | EventShapeKind::LinePosX
        | EventShapeKind::LinePosZ
        | EventShapeKind::LineNegX => 12,
        EventShapeKind::Polyline2 | EventShapeKind::Rectangle => 14,
        EventShapeKind::Polyline3 | EventShapeKind::Triangle => 18,
        EventShapeKind::Polyline4 | EventShapeKind::Quad => 22,
        EventShapeKind::Polyline5 => 26,
        EventShapeKind::Polyline6 => 30,
        EventShapeKind::Polyline7 => 34,
    };
    if offset.checked_add(min_len)? > ev_pos.len() {
        return None;
    }
    let x = f32::from_le_bytes(ev_pos[offset..offset + 4].try_into().ok()?);
    let y = char_to_float2([ev_pos[offset + 4], ev_pos[offset + 5]]);
    let z = f32::from_le_bytes(ev_pos[offset + 6..offset + 10].try_into().ok()?);
    let param = if matches!(shape, EventShapeKind::Point) {
        0.0
    } else {
        char_to_float2([ev_pos[offset + 10], ev_pos[offset + 11]])
    };
    Some(([x, y, z], param))
}

/// Complete static retail descriptor for an Act 1 stage (`Stages 1..=27`).
#[derive(Debug, Clone, Copy)]
pub struct Act1StageDescriptor {
    /// Typed 1-based stage identifier.
    pub stage_id: Act1StageId,
    /// `Stage_Data` symbol VA in `SLUS_202.28`.
    pub ps2_stage_data_addr: u32,
    /// `Stage_Data` symbol VA in `sh2pc.exe`.
    pub pc_stage_data_addr: u32,
    /// Raw `Stage_Data` pointer header.
    pub header: StageDataHeader,
    /// Retail `Event_List` terminated by `is_sentinel()` (`flag = 0x3fff`).
    pub event_list: &'static [EventListEntry],
    /// Raw retail `Pos_List` (`ev_pos`) byte buffer.
    pub ev_pos: &'static [u8],
    /// Retail `ev_prog` function pointer VAs in `sh2pc.exe` (`index 0` is `0`).
    pub ev_prog_addrs: &'static [u32],
    /// Retail `Item_List` (`gi_list`) pickups.
    pub item_list: &'static [Act1StageItemEntry],
    /// Retail `Model_List` (`mdl_list`) static/animated stage props.
    pub model_list: &'static [ModelListEntry],
    /// Retail `Enemy_List` (`en_list`) spawns (decode via [`decode_act1_enemy_spawn`]).
    pub enemy_list: &'static [EnemyListEntry],
}

pub(crate) const fn ev(flag: u32, cond: u32, rslt0: u32, rslt1: u32) -> EventListEntry {
    EventListEntry {
        flag,
        cond,
        rslt0,
        rslt1,
    }
}

pub(crate) const fn ev_sentinel() -> EventListEntry {
    EventListEntry {
        flag: 0x3fff,
        cond: 0,
        rslt0: 0,
        rslt1: 0,
    }
}

pub(crate) const fn gi(
    pos_x: f32,
    pos_z: f32,
    pos_y: i16,
    rot_y: i16,
    st: u32,
) -> Act1StageItemEntry {
    Act1StageItemEntry {
        pos_x,
        pos_z,
        pos_y,
        rot_y,
        st,
    }
}

pub(crate) const fn mdl(
    kind: i16,
    id: i16,
    flag_off: i16,
    flag_on: i16,
    pos: [f32; 4],
    rot: [f32; 4],
) -> ModelListEntry {
    let id_bytes = (id as u16).to_le_bytes();
    ModelListEntry {
        kind,
        room: id_bytes[0],
        state: id_bytes[1] as i8,
        flag_0: flag_off,
        flag_1: flag_on,
        pos,
        rot,
        mark: 0,
        reserve: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn en(
    kind: i16,
    id: i16,
    pos_x: i32,
    pos_z: i32,
    pos_y: i16,
    rot_y: i16,
    status: i16,
    condition: u16,
) -> EnemyListEntry {
    let py = (pos_y as u16).to_le_bytes();
    let ry = (rot_y as u16).to_le_bytes();
    EnemyListEntry {
        kind,
        id,
        x: (pos_x as u32 & 0xffff) as u16 as i16,
        y: ((pos_x as u32 >> 16) & 0xffff) as u16 as i16,
        z: (pos_z as u32 & 0xffff) as u16 as i16,
        rot: ((pos_z as u32 >> 16) & 0xffff) as u16 as i16,
        room: py[0],
        cond: py[1],
        a_type: ry[0],
        b_type: ry[1],
        flag_0: status as u16,
        flag_1: condition,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn hdr(
    ev_list_ptr: u32,
    ev_pos_ptr: u32,
    ev_prog_ptr: u32,
    item_list_ptr: u32,
    model_list_ptr: u32,
    enemy_list_ptr: u32,
    stage_init_fn: u32,
    room_init_fn: u32,
    all_time_fn: u32,
    glb_crd: i32,
    sound_call_after_load_fn: u32,
    bgm_control_fn: u32,
    chara_data_clear_fn: u32,
) -> StageDataHeader {
    StageDataHeader {
        ev_list_ptr,
        ev_pos_ptr,
        ev_prog_ptr,
        item_list_ptr,
        model_list_ptr,
        enemy_list_ptr,
        stage_init_fn,
        room_init_fn,
        all_time_fn,
        glb_crd,
        sound_call_after_load_fn,
        bgm_control_fn,
        chara_data_clear_fn,
    }
}
