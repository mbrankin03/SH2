//! Retail PC (`sh2pc.exe`) `Enemy_List`, `Item_List`, `ev_pos` half-float geometry decoding,
//! and `Event_List` trigger/result nibble normalization (`0x0048b400..0x00493100`).

use std::f32::consts::PI;

use crate::events::triggers::EvPosShape;
use crate::types::{char_to_float4, EnemyListEntry, EventListEntry, EventShapeKind, ItemListEntry};

/// Sentinel `EventListEntry` (`flag_1 == 0x3fff`, `cond == 0`, `rslt0 == 0`) terminating both
/// PS2 `SLUS_202.28` (`0x001a8ee8`) and retail PC `sh2pc.exe` (`0x0048e181`) event scans.
pub const EV_SENTINEL: EventListEntry = EventListEntry {
    flag: 0x0000_3fff,
    cond: 0,
    rslt0: 0,
    rslt1: 0,
};

/// Base flag index in `EventFlags` (`0x01f7ade0` vs `0x01f7a9e0` = `+0x400` bytes = `8192` bits)
/// where per-enemy permanent defeat bits are stored (`sh2pc.exe` `0x0048f785`).
pub const ENEMY_DEFEAT_FLAG_BASE: usize = 8192;

/// Lossless 20-byte retail `Enemy_List` entry (`sh2pc.exe` `0x0048f780` & `0x00493050`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetailEnemySpawn {
    /// Character class ID (`+0x00`, e.g. `0x0200` Lying Figure, `0x0202` Mannequin).
    pub kind: i16,
    /// Stage-unique enemy defeat bit index (`+0x02`, stored at `0x01f7ade0` = `8192 + id`).
    pub id: i16,
    /// World X coordinate in PS2 world millimeters (`+0x04`, full 32-bit `i32`).
    pub pos_x: i32,
    /// World Z coordinate in PS2 world millimeters (`+0x08`, full 32-bit `i32`).
    pub pos_z: i32,
    /// World Y coordinate in PS2 world millimeters (`+0x0c`).
    pub pos_y: i16,
    /// Initial yaw angle in signed Q15 half-turns (`+0x0e`, `rot_y * PI / 32768.0` radians).
    pub rot_y: i16,
    /// AI behavior / wake condition word (`+0x10`).
    pub cond_type: u16,
    /// Action-difficulty and flag spawn filter word (`+0x12`, evaluated by `0x0048f100`).
    pub diff_cond: u16,
}

impl RetailEnemySpawn {
    /// Permanent defeat flag index in [`crate::events::EventFlags`] (`8192 + id`).
    #[inline]
    pub const fn defeat_flag_id(&self) -> Option<usize> {
        if self.id < 0 {
            None
        } else {
            Some(ENEMY_DEFEAT_FLAG_BASE + (self.id as usize))
        }
    }

    /// World-space `[x, y, z]` spawn position in PS2 millimeters.
    #[inline]
    pub const fn position_mm(&self) -> [f32; 3] {
        [self.pos_x as f32, self.pos_y as f32, self.pos_z as f32]
    }

    /// Initial yaw angle in radians (`[-PI, PI)`).
    #[inline]
    pub fn yaw_radians(&self) -> f32 {
        (self.rot_y as f32) * (PI / 32768.0)
    }

    /// Projects onto the 20-byte PS2 prototype [`EnemyListEntry`] (clamping `pos_x`/`pos_z` to `i16`).
    pub const fn to_ps2_clamped(&self) -> EnemyListEntry {
        EnemyListEntry {
            kind: self.kind,
            id: self.id,
            x: clamp_i32_to_i16(self.pos_x),
            y: self.pos_y,
            z: clamp_i32_to_i16(self.pos_z),
            rot: self.rot_y,
            room: 0,
            cond: (self.cond_type & 0xff) as u8,
            a_type: ((self.cond_type >> 8) & 0xff) as u8,
            b_type: 0,
            flag_0: self.diff_cond,
            flag_1: 0,
        }
    }
}

/// Lossless 16-byte retail `Item_List` (`gi_list`) pickup entry (`sh2pc.exe` `0x0048b480` & `0x0048e3b0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetailItemSpawn {
    /// Raw IEEE-754 bits of world X coordinate in PS2 millimeters (`+0x00`).
    pub pos_x_bits: u32,
    /// Raw IEEE-754 bits of world Z coordinate in PS2 millimeters (`+0x04`).
    pub pos_z_bits: u32,
    /// Upper 16 bits of IEEE-754 `f32` world Y coordinate (`+0x08`).
    pub pos_y_hi16: u16,
    /// Upper 16 bits of IEEE-754 `f32` Y rotation in radians (`+0x0a`).
    pub rot_y_hi16: u16,
    /// Packed pickup metadata (`+0x0c`, decoded by `ItemListElement` `0x0048b480`).
    pub flags_packed: u32,
}

impl RetailItemSpawn {
    /// World X coordinate in PS2 millimeters.
    #[inline]
    pub const fn pos_x(&self) -> f32 {
        f32::from_bits(self.pos_x_bits)
    }

    /// World Y coordinate in PS2 millimeters.
    #[inline]
    pub const fn pos_y(&self) -> f32 {
        f32::from_bits((self.pos_y_hi16 as u32) << 16)
    }

    /// World Z coordinate in PS2 millimeters.
    #[inline]
    pub const fn pos_z(&self) -> f32 {
        f32::from_bits(self.pos_z_bits)
    }

    /// Pickup model rotation around Y in radians.
    #[inline]
    pub const fn rot_y(&self) -> f32 {
        f32::from_bits((self.rot_y_hi16 as u32) << 16)
    }

    /// Selector `0` (`flags_packed >> 29`, `1..=7`): generic supply type (`sh2pc.exe` `0x0048e4a4`).
    #[inline]
    pub const fn generic_supply_code(&self) -> u8 {
        (self.flags_packed >> 29) as u8
    }

    /// Maps [`Self::generic_supply_code`] to its 3D character model kind (`0x0048e4b0`).
    pub const fn pickup_model_kind(&self) -> Option<u16> {
        match self.generic_supply_code() {
            1 => Some(0x0703),
            2 => Some(0x0724),
            3 => Some(0x0723),
            4 => Some(0x0700),
            5 => Some(0x0701),
            6 => Some(0x0733),
            7 => Some(0x0749),
            _ => None,
        }
    }

    /// Selector `1` (`(flags_packed >> 26) & 0x7`): index into `ITEM_KIND_TABLE` (`0..=7`).
    #[inline]
    pub const fn item_kind_index(&self) -> u8 {
        ((self.flags_packed >> 26) & 0x7) as u8
    }

    /// Selector `2` (`(flags_packed >> 20) & 0x1f`): action-difficulty & extra spawn filter (`0x0048c670`).
    #[inline]
    pub const fn difficulty_filter(&self) -> u8 {
        ((self.flags_packed >> 20) & 0x1f) as u8
    }

    /// Selector `3` (`flags_packed & 0x3fff`): base visibility flag (`flag_id + 1` = collected).
    #[inline]
    pub const fn flag_id(&self) -> u16 {
        (self.flags_packed & 0x3fff) as u16
    }

    /// Projects onto PS2 prototype [`ItemListEntry`] bit layout (`SLUS_202.28` `0x001a9320`).
    pub fn to_ps2_clamped(&self) -> ItemListEntry {
        let kind = u32::from(self.item_kind_index());
        let diff = u32::from(self.difficulty_filter() & 0xf) << 8;
        let fid = u32::from(self.flag_id()) << 16;
        let vis = u32::from(self.generic_supply_code().min(3)) << 30;
        ItemListEntry {
            flag: kind | diff | fid | vis,
            pos_x: clamp_f32_to_i16(self.pos_x()),
            pos_y: clamp_f32_to_i16(self.pos_y()),
            pos_z: clamp_f32_to_i16(self.pos_z()),
            rot_x: 0,
            rot_y: 0,
            rot_z: 0,
            pad: [0; 3],
        }
    }
}

/// Decodes a 2-byte retail `ev_pos` half-precision float (`CharToFloat2` at `sh2pc.exe` `0x0048b400`).
#[inline]
pub const fn char_to_float2_retail(bytes: [u8; 2]) -> f32 {
    let raw = u16::from_le_bytes(bytes) as u32;
    let sign_mant = ((raw & 0x8000) << 3) | (raw & 0x03ff);
    let exp = (raw & 0x7c00).wrapping_add(0x0001_c000);
    f32::from_bits((sign_mant << 13) | (exp << 13))
}

/// Default horizontal interaction radius (`400.0` mm, `400.0^2 = 160_000.0` mm²) used by retail
/// `EventCheck` point/line look checks (`sh2pc.exe` `0x0048b007` -> `0x006a91e8`, `0x0048b051` -> `0x006a91e4`).
pub const RETAIL_LOOK_RADIUS_MM: f32 = 400.0;

/// Decodes a spatial shape from a retail `ev_pos` byte buffer (`sh2pc.exe` `0x0048d4d0` & `0x0048da90`).
pub fn decode_retail_ev_pos_shape(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
) -> Option<EvPosShape> {
    let req_len = match shape {
        EventShapeKind::Point => 10,
        EventShapeKind::LineNegZ
        | EventShapeKind::LinePosX
        | EventShapeKind::LinePosZ
        | EventShapeKind::LineNegX => 12,
        EventShapeKind::Polyline2
        | EventShapeKind::Polyline3
        | EventShapeKind::Polyline4
        | EventShapeKind::Polyline5
        | EventShapeKind::Polyline6
        | EventShapeKind::Polyline7 => 10 + ((shape as usize) - 4) * 4,
        EventShapeKind::Rectangle => 14,
        EventShapeKind::Triangle => 18,
        EventShapeKind::Quad => 22,
    };
    let s = ev_pos.get(offset..offset.checked_add(req_len)?)?;
    let x0 = char_to_float4([s[0], s[1], s[2], s[3]]);
    let y = char_to_float2_retail([s[4], s[5]]);
    let z0 = char_to_float4([s[6], s[7], s[8], s[9]]);

    match shape {
        EventShapeKind::Point => Some(EvPosShape::Point {
            x: x0,
            y,
            z: z0,
            radius: RETAIL_LOOK_RADIUS_MM,
        }),
        EventShapeKind::LineNegZ
        | EventShapeKind::LinePosX
        | EventShapeKind::LinePosZ
        | EventShapeKind::LineNegX => {
            let len = char_to_float2_retail([s[10], s[11]]);
            let end = match shape {
                EventShapeKind::LineNegZ => [x0 + len, z0],
                EventShapeKind::LinePosX => [x0 - len, z0],
                EventShapeKind::LinePosZ => [x0, z0 - len],
                _ => [x0, z0 + len],
            };
            Some(EvPosShape::Polyline {
                y,
                radius: RETAIL_LOOK_RADIUS_MM,
                vertices: vec![[x0, z0], end],
            })
        }
        EventShapeKind::Polyline2
        | EventShapeKind::Polyline3
        | EventShapeKind::Polyline4
        | EventShapeKind::Polyline5
        | EventShapeKind::Polyline6
        | EventShapeKind::Polyline7 => {
            let extra = (shape as usize) - 4;
            let mut vertices = Vec::with_capacity(extra + 1);
            vertices.push([x0, z0]);
            for i in 0..extra {
                let b = 10 + i * 4;
                let dx = char_to_float2_retail([s[b], s[b + 1]]);
                let dz = char_to_float2_retail([s[b + 2], s[b + 3]]);
                vertices.push([x0 + dx, z0 + dz]);
            }
            Some(EvPosShape::Polyline {
                y,
                radius: RETAIL_LOOK_RADIUS_MM,
                vertices,
            })
        }
        EventShapeKind::Rectangle => Some(EvPosShape::Rectangle {
            x: x0,
            y,
            z: z0,
            w: char_to_float2_retail([s[10], s[11]]),
            d: char_to_float2_retail([s[12], s[13]]),
        }),
        EventShapeKind::Triangle => Some(EvPosShape::Triangle {
            y,
            vertices: [
                [x0, z0],
                [
                    x0 + char_to_float2_retail([s[10], s[11]]),
                    z0 + char_to_float2_retail([s[12], s[13]]),
                ],
                [
                    x0 + char_to_float2_retail([s[14], s[15]]),
                    z0 + char_to_float2_retail([s[16], s[17]]),
                ],
            ],
        }),
        EventShapeKind::Quad => Some(EvPosShape::Quad {
            y,
            vertices: [
                [x0, z0],
                [
                    x0 + char_to_float2_retail([s[10], s[11]]),
                    z0 + char_to_float2_retail([s[12], s[13]]),
                ],
                [
                    x0 + char_to_float2_retail([s[14], s[15]]),
                    z0 + char_to_float2_retail([s[16], s[17]]),
                ],
                [
                    x0 + char_to_float2_retail([s[18], s[19]]),
                    z0 + char_to_float2_retail([s[20], s[21]]),
                ],
            ],
        }),
    }
}

/// Maps a retail `sh2pc.exe` [`EventListEntry`] (`0x0048e5b4` & `0x0048e7a4`) onto `SLUS_202.28`
/// trigger and result discriminants expected by [`crate::events::EventEngine::tick_standard`].
///
/// Retail `trg == 1` maps to `Always` (`9`): in `EventCheck` (`0x0048e1db..0x0048e2cb`) it falls
/// through both the button/item jump (`ecx = trg - 2 > 8u` -> `ja 0x0048e219`) and the spatial
/// jump (`ja 0x0048e572`), firing every frame whenever `flag_0`/`flag_1` hold.
///
/// Note: only the trigger/result nibbles are normalized. The `ev_pos` offsets still index the
/// retail 10-byte `f32`/`f16` geometry in [`super::StageTableAct2::pos_list`], which must be
/// decoded with [`decode_retail_ev_pos_shape`] (the PS2 `tick_standard` `i16` decoder does not
/// understand that layout).
pub const fn normalize_retail_event_entry(entry: EventListEntry) -> EventListEntry {
    if entry.is_sentinel() {
        return entry;
    }
    let trg = match (entry.cond >> 28) & 0xf {
        0 => 5,
        1 => 9,
        2 | 7 => 2,
        3 | 4 | 8 | 9 => 0,
        5 | 10 => 6,
        6 => 8,
        other => other,
    };
    let rslt = match (entry.rslt0 >> 28) & 0xf {
        3 => 5,
        4 => 3,
        5 => 6,
        6 => 4,
        other => other,
    };
    EventListEntry {
        flag: entry.flag,
        cond: (entry.cond & 0x0fff_ffff) | (trg << 28),
        rslt0: (entry.rslt0 & 0x0fff_ffff) | (rslt << 28),
        rslt1: entry.rslt1,
    }
}

#[inline]
const fn clamp_i32_to_i16(v: i32) -> i16 {
    if v < i16::MIN as i32 {
        i16::MIN
    } else if v > i16::MAX as i32 {
        i16::MAX
    } else {
        v as i16
    }
}

#[inline]
fn clamp_f32_to_i16(v: f32) -> i16 {
    if !v.is_finite() {
        0
    } else {
        clamp_i32_to_i16(v.round() as i32)
    }
}
