//! PS2 / PC Stage Event Data Structures (`Stage_Data`, `Event_List`, `Item_List`, `Model_List`,
//! `Enemy_List`, `Event_DoorSound`, `Event_CamSetData`).
//!
//! Verified 1:1 against the PS2 prototype (`SLUS_202.28`, `src/Event/event.c` at `0x001a77b0..0x001ab300`)
//! and retail PC binary (`sh2pc.exe`).

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

/// Trigger condition type (`(cond >> 28) & 0xf`, `EventListElement` selector `7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventTriggerKind {
    /// `0`: Triggered every frame while the player is inside/touching the shape (`ev_active != 2`).
    TouchContinuous = 0,
    /// `1`: Triggered on the rising edge when the player first touches the shape (`ev_active != 2`).
    TouchEdge = 1,
    /// `2`: Triggered when pressing the Action/Confirm button while looking at the shape (`ev_active == 0`).
    ActionLook = 2,
    /// `3`: Triggered when pressing the Action/Confirm button inside the shape (`ev_active == 0`).
    ActionTouch = 3,
    /// `4`: Triggered automatically when entering a new room (`RoomChange != 0`, `ev_active != 2`).
    RoomInit = 4,
    /// `5`: Triggered automatically when loading a new stage (`StageChange != 0`, `ev_active != 2`).
    StageInit = 5,
    /// `6`: Triggered when using the specified inventory item (`cond.item_id`) while looking at the shape.
    UseItemLook = 6,
    /// `7`: Triggered on the rising edge of entering the zone (`ev_active == 0`).
    ZoneEnterEdge = 7,
    /// `8`: Triggered when using the specified inventory item (`cond.item_id`) inside the shape.
    UseItemTouch = 8,
    /// `9`: Triggered unconditionally every frame (`ev_active == 0`).
    Always = 9,
    /// `10`: Triggered when pressing the Map button while map viewing is disabled (`ev_active == 0`).
    MapDisabled = 10,
}

impl EventTriggerKind {
    /// Converts a 4-bit raw trigger nibble (`0..=10`) into [`EventTriggerKind`].
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::TouchContinuous),
            1 => Some(Self::TouchEdge),
            2 => Some(Self::ActionLook),
            3 => Some(Self::ActionTouch),
            4 => Some(Self::RoomInit),
            5 => Some(Self::StageInit),
            6 => Some(Self::UseItemLook),
            7 => Some(Self::ZoneEnterEdge),
            8 => Some(Self::UseItemTouch),
            9 => Some(Self::Always),
            10 => Some(Self::MapDisabled),
            _ => None,
        }
    }
}

/// Spatial shape type in `ev_pos` (`(cond >> 12) & 0xf`, `EventListElement` selector `9`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventShapeKind {
    /// `0`: Single 3D point (`(x, y, z, radius)` = 8 bytes).
    Point = 0,
    /// `1`: Axis-aligned line segment along `-Z` (`(x, y, z, radius, length)` = 10 bytes).
    LineNegZ = 1,
    /// `2`: Axis-aligned line segment along `+X` (`(x, y, z, radius, length)` = 10 bytes).
    LinePosX = 2,
    /// `3`: Axis-aligned line segment along `+Z` (`(x, y, z, radius, length)` = 10 bytes).
    LinePosZ = 3,
    /// `4`: Axis-aligned line segment along `-X` (`(x, y, z, radius, length)` = 10 bytes).
    LineNegX = 4,
    /// `5`: Arbitrary 2-vertex 2D segment (`(x0, y, z0, radius, x1, z1)` = 12 bytes).
    Polyline2 = 5,
    /// `6`: 3-vertex polyline (2 segments, 16 bytes).
    Polyline3 = 6,
    /// `7`: 4-vertex polyline (3 segments, 20 bytes).
    Polyline4 = 7,
    /// `8`: 5-vertex polyline (4 segments, 24 bytes).
    Polyline5 = 8,
    /// `9`: 6-vertex polyline (5 segments, 28 bytes).
    Polyline6 = 9,
    /// `10`: 7-vertex polyline (6 segments, 32 bytes).
    Polyline7 = 10,
    /// `11`: Axis-aligned 2D rectangle (`(x, y, z, w, d)` = 10 bytes).
    Rectangle = 11,
    /// `12`: 2D triangle (`(x0, y, z0, x1, z1, x2, z2)` = 14 bytes).
    Triangle = 12,
    /// `13`: Convex 2D quad split into two triangles (`(x0, y, z0, x1, z1, x2, z2, x3, z3)` = 18 bytes).
    Quad = 13,
}

impl EventShapeKind {
    /// Converts a 4-bit raw shape nibble (`0..=13`) into [`EventShapeKind`].
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Point),
            1 => Some(Self::LineNegZ),
            2 => Some(Self::LinePosX),
            3 => Some(Self::LinePosZ),
            4 => Some(Self::LineNegX),
            5 => Some(Self::Polyline2),
            6 => Some(Self::Polyline3),
            7 => Some(Self::Polyline4),
            8 => Some(Self::Polyline5),
            9 => Some(Self::Polyline6),
            10 => Some(Self::Polyline7),
            11 => Some(Self::Rectangle),
            12 => Some(Self::Triangle),
            13 => Some(Self::Quad),
            _ => None,
        }
    }

    /// Byte size of this shape record in the stage's `ev_pos` byte array.
    pub const fn byte_len(self) -> usize {
        match self {
            Self::Point => 8,
            Self::LineNegZ | Self::LinePosX | Self::LinePosZ | Self::LineNegX => 10,
            Self::Polyline2 => 12,
            Self::Polyline3 => 16,
            Self::Polyline4 => 20,
            Self::Polyline5 => 24,
            Self::Polyline6 => 28,
            Self::Polyline7 => 32,
            Self::Rectangle => 10,
            Self::Triangle => 14,
            Self::Quad => 18,
        }
    }
}

/// Destination placement shape in `EventResultMovePosition` (`(rslt0 >> 12) & 0xf`, selector `15`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventDestShapeKind {
    /// `0`: Explicit point + yaw angle in degrees (`(x, y, z, deg)` = 8 bytes).
    PointWithAngle = 0,
    /// `1`: Door line along `-Z`: places player at `(x - 250, y, z - len/2)` facing `-PI/2`.
    DoorLineNegZ = 1,
    /// `2`: Door line along `+X`: places player at `(x + len/2, y, z - 250)` facing `PI`.
    DoorLinePosX = 2,
    /// `3`: Door line along `+Z`: places player at `(x + 250, y, z + len/2)` facing `+PI/2`.
    DoorLinePosZ = 3,
    /// `4`: Door line along `-X`: places player at `(x - len/2, y, z + 250)` facing `0.0`.
    DoorLineNegX = 4,
    /// `5`: Arbitrary door segment `(x, y, z, r, dx, dz)`: places player at midpoint + `250.0` normal offset.
    DoorSegment = 5,
}

impl EventDestShapeKind {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::PointWithAngle),
            1 => Some(Self::DoorLineNegZ),
            2 => Some(Self::DoorLinePosX),
            3 => Some(Self::DoorLinePosZ),
            4 => Some(Self::DoorLineNegX),
            5 => Some(Self::DoorSegment),
            _ => None,
        }
    }
}

/// Result action type (`(rslt0 >> 28) & 0xf`, `EventListElement` selector `13`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventResultKind {
    /// `1`: Set flag (`rslt1 & 0x3fff`) via `EventExecFlag`.
    FlagSet = 1,
    /// `2`: Clear flag (`rslt1 & 0x3fff`) via `EventExecFlag`.
    FlagClear = 2,
    /// `3`: Move player to destination (`ev_pos[dest_offset]`), optionally changing stage (`EventExecMove`).
    Move = 3,
    /// `4`: Display stage `.mes` message (`(rslt1 >> 14) & 0xff`) via `EventExecMessage`.
    Message = 4,
    /// `5`: Execute stage program `ev_prog[(rslt1 >> 14) & 0x7f]` via `EventExecProgram`.
    Program = 5,
    /// `6`: Move player (`EventExecMove`) and then chain directly into `EventExecProgram`.
    MovePlusProgram = 6,
    /// `7`: Locked door feedback (`EventExecDoor`: locked sound + message).
    DoorLocked = 7,
    /// `8`: Broken lock feedback (`EventExecDoor`: broken sound + common message 4).
    DoorBroken = 8,
    /// `9`: Unlocked door feedback (`EventExecDoor`: unlock sound + message).
    DoorUnlocked = 9,
    /// `10`: Save point interaction (`EventExecSave`).
    SavePoint = 10,
}

impl EventResultKind {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::FlagSet),
            2 => Some(Self::FlagClear),
            3 => Some(Self::Move),
            4 => Some(Self::Message),
            5 => Some(Self::Program),
            6 => Some(Self::MovePlusProgram),
            7 => Some(Self::DoorLocked),
            8 => Some(Self::DoorBroken),
            9 => Some(Self::DoorUnlocked),
            10 => Some(Self::SavePoint),
            _ => None,
        }
    }
}

/// 16-byte stage event descriptor (`Event_List` in `event.h` / `event.c`).
///
/// Terminated in stage tables by an entry where `(flag & 0xffff) == 0xffff` (`flag_1 == 0x3fff`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventListEntry {
    pub flag: u32,
    pub cond: u32,
    pub rslt0: u32,
    pub rslt1: u32,
}

impl EventListEntry {
    /// Returns `true` if this entry is the end-of-table sentinel (`(flag & 0xffff) == 0xffff`
    /// or `flag_1() == 0x3fff`, matching `EventCheck` at `0x001a8ee8`).
    #[inline]
    pub const fn is_sentinel(&self) -> bool {
        (self.flag & 0x3fff) == 0x3fff
    }

    /// Extracts field `kind` (`1..=22`) exactly as `EventListElement` (`SLUS_202.28` `0x001a9090`).
    pub const fn element(&self, kind: u32) -> i32 {
        match kind {
            1 => ((self.flag >> 31) & 1) as i32,
            2 => ((self.flag >> 30) & 1) as i32,
            3 => ((self.flag >> 16) & 0x3fff) as i32,
            4 => ((self.flag >> 15) & 1) as i32,
            5 => ((self.flag >> 14) & 1) as i32,
            6 => (self.flag & 0x3fff) as i32,
            7 => ((self.cond >> 28) & 0xf) as i32,
            8 => ((self.cond >> 16) & 0xfff) as i32,
            9 => ((self.cond >> 12) & 0xf) as i32,
            10 => ((self.cond >> 4) & 0xff) as i32,
            11 => ((self.cond >> 3) & 1) as i32,
            12 => ((self.cond >> 2) & 1) as i32,
            13 => ((self.rslt0 >> 28) & 0xf) as i32,
            14 => ((self.rslt0 >> 16) & 0xfff) as i32,
            15 => ((self.rslt0 >> 12) & 0xf) as i32,
            16 => ((self.rslt0 >> 5) & 0x7f) as i32,
            17 => ((self.rslt0 >> 4) & 1) as i32,
            18 => ((self.rslt1 >> 22) & 0x3f) as i32,
            19 => ((self.rslt1 >> 14) & 0x7f) as i32,
            20 => ((self.rslt1 >> 14) & 0xff) as i32,
            21 => ((self.rslt1 >> 14) & 0xff) as i32,
            22 => (self.rslt1 & 0x3fff) as i32,
            _ => -1,
        }
    }

    /// Selector `1`: `true` if condition `flag_0` is inverted (`!flag(flag_0)` required).
    #[inline]
    pub const fn flag_0_inverted(&self) -> bool {
        ((self.flag >> 31) & 1) != 0
    }

    /// Selector `2`: `true` if `EventExecSubFlagSet` toggles `flag_0` upon event execution.
    #[inline]
    pub const fn flag_0_toggle_on_exec(&self) -> bool {
        ((self.flag >> 30) & 1) != 0
    }

    /// Selector `3`: Primary condition flag ID (`(flag >> 16) & 0x3fff`).
    #[inline]
    pub const fn flag_0(&self) -> u16 {
        ((self.flag >> 16) & 0x3fff) as u16
    }

    /// Selector `4`: `true` if condition `flag_1` is inverted (`!flag(flag_1)` required).
    #[inline]
    pub const fn flag_1_inverted(&self) -> bool {
        ((self.flag >> 15) & 1) != 0
    }

    /// Selector `5`: `true` if `EventExecSubFlagSet` toggles `flag_1` upon event execution.
    #[inline]
    pub const fn flag_1_toggle_on_exec(&self) -> bool {
        ((self.flag >> 14) & 1) != 0
    }

    /// Selector `6`: Secondary condition flag ID (`flag & 0x3fff`).
    #[inline]
    pub const fn flag_1(&self) -> u16 {
        (self.flag & 0x3fff) as u16
    }

    /// Selector `7`: Trigger condition kind (`(cond >> 28) & 0xf`).
    #[inline]
    pub const fn trigger_kind(&self) -> Option<EventTriggerKind> {
        EventTriggerKind::from_raw(((self.cond >> 28) & 0xf) as u8)
    }

    /// Selector `8`: Byte offset into `Stage_Data::ev_pos` for the trigger shape (`(cond >> 16) & 0xfff`).
    #[inline]
    pub const fn ev_pos_offset(&self) -> usize {
        ((self.cond >> 16) & 0xfff) as usize
    }

    /// Selector `9`: Trigger geometry shape kind (`(cond >> 12) & 0xf`).
    #[inline]
    pub const fn shape_kind(&self) -> Option<EventShapeKind> {
        EventShapeKind::from_raw(((self.cond >> 12) & 0xf) as u8)
    }

    /// Selector `10`: Required inventory item ID for `UseItemLook` / `UseItemTouch` (`(cond >> 4) & 0xff`).
    #[inline]
    pub const fn required_item_id(&self) -> u8 {
        ((self.cond >> 4) & 0xff) as u8
    }

    /// Selector `11`: Whether the used item is consumed automatically (`(cond >> 3) & 1`).
    #[inline]
    pub const fn consume_item(&self) -> bool {
        ((self.cond >> 3) & 1) != 0
    }

    /// Selector `12`: Whether this event is disabled during Demo/Auto mode (`(cond >> 2) & 1`).
    #[inline]
    pub const fn disabled_in_demo(&self) -> bool {
        ((self.cond >> 2) & 1) != 0
    }

    /// Selector `13`: Result action kind (`(rslt0 >> 28) & 0xf`).
    #[inline]
    pub const fn result_kind(&self) -> Option<EventResultKind> {
        EventResultKind::from_raw(((self.rslt0 >> 28) & 0xf) as u8)
    }

    /// Selector `14`: Destination byte offset into `ev_pos` (`(rslt0 >> 16) & 0xfff`).
    #[inline]
    pub const fn dest_ev_pos_offset(&self) -> usize {
        ((self.rslt0 >> 16) & 0xfff) as usize
    }

    /// Selector `15`: Destination placement shape (`(rslt0 >> 12) & 0xf`).
    #[inline]
    pub const fn dest_shape_kind(&self) -> Option<EventDestShapeKind> {
        EventDestShapeKind::from_raw(((self.rslt0 >> 12) & 0xf) as u8)
    }

    /// Selector `16`: Stage transition / fade mode (`(rslt0 >> 5) & 0x7f`).
    #[inline]
    pub const fn stage_change_mode(&self) -> u8 {
        ((self.rslt0 >> 5) & 0x7f) as u8
    }

    /// Selector `17`: Observation mode flag (`(rslt0 >> 4) & 1`).
    #[inline]
    pub const fn observation_mode(&self) -> bool {
        ((self.rslt0 >> 4) & 1) != 0
    }

    /// Selector `18`: Destination stage ID or door sound table index (`(rslt1 >> 22) & 0x3f`).
    #[inline]
    pub const fn dest_stage_or_door_se(&self) -> u8 {
        ((self.rslt1 >> 22) & 0x3f) as u8
    }

    /// Selector `19`: Stage `ev_prog` function table index (`(rslt1 >> 14) & 0x7f`).
    #[inline]
    pub const fn prog_index(&self) -> u8 {
        ((self.rslt1 >> 14) & 0x7f) as u8
    }

    /// Selector `20` / `21`: Stage `.mes` message ID or save point name ID (`(rslt1 >> 14) & 0xff`).
    #[inline]
    pub const fn message_or_save_id(&self) -> u8 {
        ((self.rslt1 >> 14) & 0xff) as u8
    }

    /// Selector `22`: Completion/result flag ID (`rslt1 & 0x3fff`).
    #[inline]
    pub const fn result_flag(&self) -> u16 {
        (self.rslt1 & 0x3fff) as u16
    }
}

/// 16-byte stage item pickup descriptor (`Item_List` in `event.c`, `ItemListElement` at `0x001a9320`).
///
/// Terminated by an entry where `(flag & 0x3fff0000) == 0x3fff0000` (`flag_id() == 0x3fff`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemListEntry {
    pub flag: u32,
    pub pos_x: i16,
    pub pos_y: i16,
    pub pos_z: i16,
    pub rot_x: i8,
    pub rot_y: i8,
    pub rot_z: i8,
    pub pad: [u8; 3],
}

impl ItemListEntry {
    /// Returns `true` if this entry is the end-of-table sentinel (`flag_id() == 0x3fff`).
    #[inline]
    pub const fn is_sentinel(&self) -> bool {
        self.flag_id() == 0x3fff
    }

    /// Extracts field `kind` (`0..=4`) exactly as `ItemListElement` (`SLUS_202.28` `0x001a9320`).
    pub const fn element(&self, kind: u32) -> i32 {
        match kind {
            0 => (self.flag & 0xff) as i32,
            1 => ((self.flag >> 8) & 0xf) as i32,
            2 => ((self.flag >> 12) & 0xf) as i32,
            3 => ((self.flag >> 16) & 0x3fff) as i32,
            4 => ((self.flag >> 30) & 0x3) as i32,
            _ => -1,
        }
    }

    /// Selector `0`: Index into [`ITEM_KIND_TABLE`] (`flag & 0xff`, `0..=7`).
    #[inline]
    pub const fn item_kind_index(&self) -> u8 {
        (self.flag & 0xff) as u8
    }

    /// Selector `1`: Action difficulty condition (`(flag >> 8) & 0xf`).
    #[inline]
    pub const fn difficulty_condition(&self) -> u8 {
        ((self.flag >> 8) & 0xf) as u8
    }

    /// Selector `2`: Extra flag condition (`(flag >> 12) & 0xf`).
    #[inline]
    pub const fn extra_condition(&self) -> u8 {
        ((self.flag >> 12) & 0xf) as u8
    }

    /// Selector `3`: Base visibility flag ID (`(flag >> 16) & 0x3fff`); picking up sets `flag_id + 1`.
    #[inline]
    pub const fn flag_id(&self) -> u16 {
        ((self.flag >> 16) & 0x3fff) as u16
    }

    /// Selector `4`: Visibility mode (`(flag >> 30) & 0x3`).
    #[inline]
    pub const fn visibility_mode(&self) -> u8 {
        ((self.flag >> 30) & 0x3) as u8
    }

    /// World-space 3D position `[x, y, z]` decoded via [`char_to_float2`].
    #[inline]
    pub const fn position(&self) -> [f32; 3] {
        [self.pos_x as f32, self.pos_y as f32, self.pos_z as f32]
    }
}

/// 8-byte pickup kind descriptor (`item_kind[8]` at `SLUS_202.28` `0x002b97e0`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemKindEntry {
    /// Inventory item ID granted by `ItemGet`.
    pub item_id: i16,
    /// Common `.mes` message ID displayed on pickup.
    pub msg_id: u16,
    /// 3D character/prop model ID (`0x4xx`) deleted when picked up.
    pub chr_id: i32,
}

/// Canonical PS2 `item_kind[8]` pickup table (`SLUS_202.28` `0x002b97e0`).
pub const ITEM_KIND_TABLE: [ItemKindEntry; 8] = [
    ItemKindEntry {
        item_id: 1,
        msg_id: 10,
        chr_id: 0x0401,
    }, // Health drink
    ItemKindEntry {
        item_id: 2,
        msg_id: 11,
        chr_id: 0x0402,
    }, // First-aid kit
    ItemKindEntry {
        item_id: 3,
        msg_id: 12,
        chr_id: 0x0403,
    }, // Ampoule
    ItemKindEntry {
        item_id: 5,
        msg_id: 13,
        chr_id: 0x0405,
    }, // Handgun bullets
    ItemKindEntry {
        item_id: 7,
        msg_id: 14,
        chr_id: 0x0407,
    }, // Shotgun shells
    ItemKindEntry {
        item_id: 9,
        msg_id: 15,
        chr_id: 0x0409,
    }, // Rifle shells
    ItemKindEntry {
        item_id: 11,
        msg_id: 50,
        chr_id: 0x040b,
    }, // Revolver bullets (Born from a Wish)
    ItemKindEntry {
        item_id: 0,
        msg_id: 0,
        chr_id: 0,
    }, // Sentinel
];

/// 12-byte door sound effect entry (`door_se[22]` at `SLUS_202.28` `0x002b96d0`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDoorSound {
    pub open: i16,
    pub close: i16,
    pub lock: i16,
    pub unlock: i16,
    pub broken: i16,
    pub pad: i16,
}

/// Canonical PS2 `door_se[22]` table (`SLUS_202.28` `0x002b96d0`).
pub const DOOR_SE_TABLE: [EventDoorSound; 22] = [
    EventDoorSound {
        open: 0,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
    EventDoorSound {
        open: 11000,
        close: 11001,
        lock: 11002,
        unlock: 11003,
        broken: 11004,
        pad: 0,
    },
    EventDoorSound {
        open: 11092,
        close: 11093,
        lock: 11094,
        unlock: 11095,
        broken: 11096,
        pad: 0,
    },
    EventDoorSound {
        open: 11097,
        close: 11098,
        lock: 11099,
        unlock: 11100,
        broken: 11101,
        pad: 0,
    },
    EventDoorSound {
        open: 11056,
        close: 11057,
        lock: 11058,
        unlock: 11059,
        broken: 11060,
        pad: 0,
    },
    EventDoorSound {
        open: 11061,
        close: 11062,
        lock: 11063,
        unlock: 11064,
        broken: 11065,
        pad: 0,
    },
    EventDoorSound {
        open: 11066,
        close: 11067,
        lock: 11068,
        unlock: 11069,
        broken: 11070,
        pad: 0,
    },
    EventDoorSound {
        open: 11071,
        close: 11072,
        lock: 11073,
        unlock: 11074,
        broken: 11075,
        pad: 0,
    },
    EventDoorSound {
        open: 11076,
        close: 11077,
        lock: 11078,
        unlock: 11079,
        broken: 11080,
        pad: 0,
    },
    EventDoorSound {
        open: 11081,
        close: 11082,
        lock: 11083,
        unlock: 11084,
        broken: 11085,
        pad: 0,
    },
    EventDoorSound {
        open: 11086,
        close: 11087,
        lock: 11088,
        unlock: 11089,
        broken: 11090,
        pad: 0,
    },
    EventDoorSound {
        open: 11091,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
    EventDoorSound {
        open: 11102,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
    EventDoorSound {
        open: 11103,
        close: 11104,
        lock: 11105,
        unlock: 11106,
        broken: 11107,
        pad: 0,
    },
    EventDoorSound {
        open: 11108,
        close: 11109,
        lock: 11110,
        unlock: 11111,
        broken: 11112,
        pad: 0,
    },
    EventDoorSound {
        open: 11113,
        close: 11114,
        lock: 11115,
        unlock: 11116,
        broken: 11117,
        pad: 0,
    },
    EventDoorSound {
        open: 11118,
        close: 11119,
        lock: 11120,
        unlock: 11121,
        broken: 11122,
        pad: 0,
    },
    EventDoorSound {
        open: 11123,
        close: 11124,
        lock: 11125,
        unlock: 11126,
        broken: 11127,
        pad: 0,
    },
    EventDoorSound {
        open: 11128,
        close: 11129,
        lock: 11130,
        unlock: 11131,
        broken: 11132,
        pad: 0,
    },
    EventDoorSound {
        open: 11133,
        close: 11134,
        lock: 11135,
        unlock: 11136,
        broken: 11137,
        pad: 0,
    },
    EventDoorSound {
        open: 11138,
        close: 11139,
        lock: 11140,
        unlock: 11141,
        broken: 11142,
        pad: 0,
    },
    EventDoorSound {
        open: 0,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
];

/// Connected stage pairs (`SLUS_202.28` `0x002b9820`) that share seamless sub-stage assets
/// during `EventExecMove` (setting transition flag bit `0x08`).
pub const CONNECT_STAGE_PAIRS: [(i16, i16); 11] = [
    (6, 7),   // Apart E1F <-> E2F
    (8, 9),   // Apart E3FW <-> E3FE
    (10, 11), // Apart W1F <-> W2F
    (16, 17), // Hospital 1F <-> 2F
    (18, 19), // Hospital 3F <-> RF
    (21, 22), // Hospital B1F <-> B2F
    (29, 30), // Labyrinth W <-> E
    (36, 37), // Hotel 1F <-> 2F
    (38, 39), // Hotel 3F <-> B1F
    (42, 43), // Hotel Fire 1F <-> 2F
    (0, 0),   // Sentinel
];

/// 48-byte stage static/animated prop descriptor (`Model_List` in PS2 `SLUS_202.28`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModelListEntry {
    pub kind: i16,
    pub room: u8,
    pub state: i8,
    pub flag_0: i16,
    pub flag_1: i16,
    pub pos: [f32; 4],
    pub rot: [f32; 4],
    pub mark: u32,
    pub reserve: i32,
}

/// 20-byte stage enemy spawn descriptor (`Enemy_List` in PS2 `SLUS_202.28`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnemyListEntry {
    pub kind: i16,
    pub id: i16,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub rot: i16,
    pub room: u8,
    pub cond: u8,
    pub a_type: u8,
    pub b_type: u8,
    pub flag_0: u16,
    pub flag_1: u16,
}

/// 64-byte scripted camera override structure (`Event_CamSetData` in `stg_apart_e3fe.c`, etc.).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EventCamSetData {
    pub flag: u32,
    pub pad: [u32; 3],
    pub cam_pos: [f32; 4],
    pub cam_watch: [f32; 4],
    pub fov: f32,
    pub roll: f32,
    pub reserve: [f32; 2],
}

/// 52-byte PS2/PC `Stage_Data` header (`Stage_Data` in `event.h`, `SLUS_202.28` `0x01126420`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageDataHeader {
    pub ev_list_ptr: u32,
    pub ev_pos_ptr: u32,
    pub ev_prog_ptr: u32,
    pub item_list_ptr: u32,
    pub model_list_ptr: u32,
    pub enemy_list_ptr: u32,
    pub stage_init_fn: u32,
    pub room_init_fn: u32,
    pub all_time_fn: u32,
    pub glb_crd: i32,
    pub sound_call_after_load_fn: u32,
    pub bgm_control_fn: u32,
    pub chara_data_clear_fn: u32,
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
