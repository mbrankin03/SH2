//! PS2 / PC Stage Event Data Structures (`Stage_Data`, `Event_List`, `Item_List`, `Model_List`,
//! `Enemy_List`, `Event_DoorSound`, `Event_CamSetData`).
//!
//! Verified 1:1 against the PS2 prototype (`SLUS_202.28`, `src/Event/event.c` at `0x001a77b0..0x001ab300`)
//! and retail PC binary (`sh2pc.exe`).

pub mod entries;
pub mod enums;
pub mod tables;

pub use entries::{
    EnemyListEntry, EventCamSetData, EventListEntry, ItemListEntry, ModelListEntry, StageDataHeader,
};
pub use enums::{EventDestShapeKind, EventResultKind, EventShapeKind, EventTriggerKind};
pub use tables::{
    pickup_model_rel_path, EventDoorSound, ItemKindEntry, CONNECT_STAGE_PAIRS, DOOR_SE_TABLE,
    ITEM_KIND_TABLE,
};

use std::f32::consts::{PI, TAU};

/// Decodes a 2-byte Team Silent half-precision float (`f16`) into IEEE-754 `f32`.
///
/// Exact 1:1 parity with `CharToFloat2` (`SLUS_202.28` `0x001a9280`, `sh2pc.exe` `0x0048b400`):
/// ```text
/// bits = ((raw & 0x8000) << 16) | ((((raw >> 10) & 0x1f) + 0x70) << 23) | ((raw & 0x03ff) << 13)
/// ```
/// Note: Team Silent's decoder always adds `0x70` (`112`) to the 5-bit exponent without a
/// subnormal/zero branch, so raw `0x0000` decodes to `+2^-15` (`3.0517578e-5`) and `0x8000`
/// decodes to `-2^-15` (`-3.0517578e-5`).
#[inline]
pub const fn char_to_float2(bytes: [u8; 2]) -> f32 {
    let raw = u16::from_le_bytes(bytes) as u32;
    let sign = (raw & 0x8000) << 16;
    let exp = (((raw >> 10) & 0x1f) + 0x70) << 23;
    let mant = (raw & 0x03ff) << 13;
    f32::from_bits(sign | exp | mant)
}

/// Encodes an IEEE-754 `f32` into the 2-byte Team Silent half-precision float (`f16`) format
/// consumed by [`char_to_float2`] (`SLUS_202.28` `0x001a9280`, `sh2pc.exe` `0x0048b400`).
#[inline]
pub const fn float_to_char2(value: f32) -> [u8; 2] {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp32 = ((bits >> 23) & 0xff) as i32;
    let exp16 = exp32 - 0x70;
    if exp16 <= 0 {
        return sign.to_le_bytes();
    }
    if exp16 >= 0x1f {
        return (sign | (0x1f << 10) | 0x03ff).to_le_bytes();
    }
    let mant = ((bits >> 13) & 0x03ff) as u16;
    (sign | ((exp16 as u16) << 10) | mant).to_le_bytes()
}

/// Reads an unaligned 4-byte IEEE-754 little-endian `f32` from raw byte data.
///
/// Matches `CharToFloat4` (`SLUS_202.28` `0x001a92e0`).
#[inline]
pub const fn char_to_float4(bytes: [u8; 4]) -> f32 {
    f32::from_le_bytes(bytes)
}

/// Wraps an angle in radians into the canonical `[-PI, PI)` interval.
///
/// Matches `shAngleRegulate` (`SLUS_202.28` `0x0016e970`).
#[inline]
pub fn wrap_angle(mut angle: f32) -> f32 {
    angle = (angle + PI).rem_euclid(TAU) - PI;
    angle
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn ps2_event_struct_sizes_match_binary_abi() {
        assert_eq!(size_of::<EventListEntry>(), 16);
        assert_eq!(size_of::<ItemListEntry>(), 16);
        assert_eq!(size_of::<ItemKindEntry>(), 8);
        assert_eq!(size_of::<EventDoorSound>(), 12);
        assert_eq!(size_of::<ModelListEntry>(), 48);
        assert_eq!(size_of::<EnemyListEntry>(), 20);
        assert_eq!(size_of::<EventCamSetData>(), 64);
        assert_eq!(size_of::<StageDataHeader>(), 52);
    }
}
