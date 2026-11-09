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
    EventDoorSound, ItemKindEntry, CONNECT_STAGE_PAIRS, DOOR_SE_TABLE, ITEM_KIND_TABLE,
};

use std::f32::consts::{PI, TAU};

/// Converts a 2-byte signed integer (little-endian `i16`) into a single-precision `f32`.
///
/// Matches `CharToFloat2` (`SLUS_202.28` `0x001a9280`).
#[inline]
pub const fn char_to_float2(bytes: [u8; 2]) -> f32 {
    i16::from_le_bytes(bytes) as f32
}

/// Reads an unaligned 4-byte IEEE-754 little-endian `f32` from raw byte data.
///
/// Matches `CharToFloat4` (`SLUS_202.28` `0x001a92b0`).
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
