//! Retail PC (`sh2pc.exe`) `Enemy_List`, `Item_List`, `ev_pos` half-float geometry decoding,
//! and `Event_List` trigger/result nibble normalization (`0x0048b400..0x00493100`).

use std::f32::consts::PI;

use crate::events::triggers::{decode_ev_pos_shape, EvPosShape, EVENT_LOOK_RADIUS_MM};
use crate::types::{char_to_float2, EnemyListEntry, EventListEntry, EventShapeKind, ItemListEntry};

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

/// Lossless 16-byte retail `Item_List` (`gi_list`) pickup entry (`SLUS_202.28` `ItemListElement`
/// `0x001a9320`, `ItemCheckLookPoint` `0x001a93b0`; `sh2pc.exe` `0x0048b480`, `0x0048b4d0`, `0x0048e3b0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetailItemSpawn {
    /// Raw IEEE-754 bits of world X coordinate in PS2 millimeters (`+0x00`, `CharToFloat4`).
    pub pos_x_bits: u32,
    /// Raw IEEE-754 bits of world Z coordinate in PS2 millimeters (`+0x04`, `CharToFloat4`).
    pub pos_z_bits: u32,
    /// Team Silent `f16` world Y coordinate (`+0x08`, decoded via `CharToFloat2` `0x001a9280` / `0x0048b400`).
    pub pos_y_hi16: u16,
    /// Team Silent `f16` Y rotation in radians (`+0x0a`, decoded via `CharToFloat2` `0x001a9280` / `0x0048b400`).
    pub rot_y_hi16: u16,
    /// Packed pickup metadata (`+0x0c`, decoded by `ItemListElement` `0x001a9320` / `0x0048b480`).
    pub flags_packed: u32,
}

impl RetailItemSpawn {
    /// World X coordinate in PS2 millimeters.
    #[inline]
    pub const fn pos_x(&self) -> f32 {
        f32::from_bits(self.pos_x_bits)
    }

    /// World Y coordinate in PS2 millimeters (`CharToFloat2` at `SLUS_202.28` `0x001a9280`, `sh2pc.exe` `0x0048b400`).
    #[inline]
    pub const fn pos_y(&self) -> f32 {
        char_to_float2(self.pos_y_hi16.to_le_bytes())
    }

    /// World Z coordinate in PS2 millimeters.
    #[inline]
    pub const fn pos_z(&self) -> f32 {
        f32::from_bits(self.pos_z_bits)
    }

    /// Pickup model rotation around Y in radians (`CharToFloat2` at `SLUS_202.28` `0x001a9280`, `sh2pc.exe` `0x0048b400`).
    #[inline]
    pub const fn rot_y(&self) -> f32 {
        char_to_float2(self.rot_y_hi16.to_le_bytes())
    }

    /// Selector `0` (`flags_packed >> 29`, `1..=7`): supply kind index into `ITEM_KIND_TABLE`
    /// (`ItemListElement` `SLUS_202.28` `0x001a9320`, `sh2pc.exe` `0x0048b480` & `0x0048e4a4`).
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

    /// Selector `0` (`flags_packed >> 29`, `1..=7`): supply kind index into `ITEM_KIND_TABLE`
    /// (`ItemListElement` `SLUS_202.28` `0x001a9320`, `sh2pc.exe` `0x0048b480`).
    #[inline]
    pub const fn item_kind_index(&self) -> u8 {
        (self.flags_packed >> 29) as u8
    }

    /// Selector `1` (`(flags_packed >> 26) & 0x7`): action-difficulty spawn filter
    /// (`ItemListElement` `SLUS_202.28` `0x001a9320`, `EventItemConditionCheck` `0x001ab1a0` / `0x0048c670`).
    #[inline]
    pub const fn difficulty_condition(&self) -> u8 {
        ((self.flags_packed >> 26) & 0x7) as u8
    }

    /// Selector `2` (`(flags_packed >> 20) & 0x1f`): extra story flag condition (`1=flag 219`,
    /// `2=flag 108`, `3=flag 208`; `ItemListElement` `0x001a9320`, `EventItemConditionCheck` `0x001ab1a0`).
    #[inline]
    pub const fn extra_condition(&self) -> u8 {
        ((self.flags_packed >> 20) & 0x1f) as u8
    }

    /// Selector `3` (`flags_packed & 0x3fff`): base visibility flag (`flag_id + 1` = collected).
    #[inline]
    pub const fn flag_id(&self) -> u16 {
        (self.flags_packed & 0x3fff) as u16
    }

    /// Projects losslessly onto [`ItemListEntry`] (`SLUS_202.28` `0x001a9320`, `sh2pc.exe` `0x0048b480`).
    pub fn to_ps2_clamped(&self) -> ItemListEntry {
        ItemListEntry {
            pos_x: self.pos_x(),
            pos_z: self.pos_z(),
            pos_y: self.pos_y_hi16,
            rot_y: self.rot_y_hi16,
            flag: self.flags_packed,
        }
    }
}

/// Decodes a 2-byte retail `ev_pos` half-precision float (`CharToFloat2` at `SLUS_202.28` `0x001a9280`,
/// `sh2pc.exe` `0x0048b400`).
#[inline]
pub const fn char_to_float2_retail(bytes: [u8; 2]) -> f32 {
    char_to_float2(bytes)
}

/// Default horizontal interaction radius (`400.0` mm, `400.0^2 = 160_000.0` mm²) used by retail
/// `EventCheck` point/line look checks (`SLUS_202.28` `0x001a8b00`, `sh2pc.exe` `0x0048b007` -> `0x006a91e8`).
pub const RETAIL_LOOK_RADIUS_MM: f32 = EVENT_LOOK_RADIUS_MM;

/// Decodes a spatial shape from a retail `ev_pos` byte buffer (`SLUS_202.28` `0x001a8780` & `0x001a8d70`,
/// `sh2pc.exe` `0x0048d4d0` & `0x0048da90`).
pub fn decode_retail_ev_pos_shape(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
) -> Option<EvPosShape> {
    decode_ev_pos_shape(ev_pos, offset, shape)
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
