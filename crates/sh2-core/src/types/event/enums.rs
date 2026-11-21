//! Stage event trigger, spatial shape, destination placement, and result action enumerations
//! (`SLUS_202.28` `src/Event/event.c`).

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
///
/// Binary layout verified 1:1 against `EventCheckLook` (`SLUS_202.28` `0x001a8780`,
/// `sh2pc.exe` `0x0048d4d0`) and `EventCheckIn` (`SLUS_202.28` `0x001a8d70`,
/// `sh2pc.exe` `0x0048da90`): every record begins with a 10-byte header
/// `[f32 x0 (+0), f16 y (+4), f32 z0 (+6)]` with no per-shape radius field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventShapeKind {
    /// `0`: Single 3D point (`10` bytes: `f32 x0, f16 y, f32 z0`).
    Point = 0,
    /// `1`: Axis-aligned line segment along `+X` (`12` bytes: header + `f16 len` at `+10`, end `(x0 + len, z0)`).
    LinePosX = 1,
    /// `2`: Axis-aligned line segment along `-X` (`12` bytes: header + `f16 len` at `+10`, end `(x0 - len, z0)`).
    LineNegX = 2,
    /// `3`: Axis-aligned line segment along `-Z` (`12` bytes: header + `f16 len` at `+10`, end `(x0, z0 - len)`).
    LineNegZ = 3,
    /// `4`: Axis-aligned line segment along `+Z` (`12` bytes: header + `f16 len` at `+10`, end `(x0, z0 + len)`).
    LinePosZ = 4,
    /// `5`: Arbitrary 2-vertex 2D segment (`14` bytes: header + 1 relative `(f16 dx, f16 dz)` at `+10`).
    Polyline2 = 5,
    /// `6`: 3-vertex polyline (2 segments, `18` bytes: header + 2 relative `(f16 dx, f16 dz)` pairs).
    Polyline3 = 6,
    /// `7`: 4-vertex polyline (3 segments, `22` bytes: header + 3 relative `(f16 dx, f16 dz)` pairs).
    Polyline4 = 7,
    /// `8`: 5-vertex polyline (4 segments, `26` bytes: header + 4 relative `(f16 dx, f16 dz)` pairs).
    Polyline5 = 8,
    /// `9`: 6-vertex polyline (5 segments, `30` bytes: header + 5 relative `(f16 dx, f16 dz)` pairs).
    Polyline6 = 9,
    /// `10`: 7-vertex polyline (6 segments, `34` bytes: header + 6 relative `(f16 dx, f16 dz)` pairs).
    Polyline7 = 10,
    /// `11`: Axis-aligned 2D rectangle (`14` bytes: header + `f16 w` at `+10`, `f16 d` at `+12`).
    Rectangle = 11,
    /// `12`: 2D triangle (`18` bytes: header + 2 relative `(f16 dx, f16 dz)` pairs at `+10`).
    Triangle = 12,
    /// `13`: Convex 2D quad (`22` bytes: header + 3 relative `(f16 dx, f16 dz)` pairs at `+10`).
    Quad = 13,
}

impl EventShapeKind {
    /// Converts a 4-bit raw shape nibble (`0..=13`) into [`EventShapeKind`].
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Point),
            1 => Some(Self::LinePosX),
            2 => Some(Self::LineNegX),
            3 => Some(Self::LineNegZ),
            4 => Some(Self::LinePosZ),
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

    /// Byte size of this shape record in the stage's `ev_pos` byte array
    /// (`EventCheckLook` `SLUS_202.28` `0x001a8780`, `EventCheckIn` `0x001a8d70`).
    pub const fn byte_len(self) -> usize {
        match self {
            Self::Point => 10,
            Self::LinePosX | Self::LineNegX | Self::LineNegZ | Self::LinePosZ => 12,
            Self::Polyline2 | Self::Rectangle => 14,
            Self::Polyline3 | Self::Triangle => 18,
            Self::Polyline4 | Self::Quad => 22,
            Self::Polyline5 => 26,
            Self::Polyline6 => 30,
            Self::Polyline7 => 34,
        }
    }
}

/// Destination placement shape in `EventResultMovePosition` (`(rslt0 >> 12) & 0xf`, selector `15`).
///
/// Verified 1:1 against `EventResultMovePosition` (`SLUS_202.28` `0x001a9730`,
/// `sh2pc.exe` `0x0048b780`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventDestShapeKind {
    /// `0` (`SLUS_202.28` `0x001a9730`): places player at `(x0, y, z0)` preserving current `rot_y` (`10` bytes).
    Point = 0,
    /// `1` (`SLUS_202.28` `0x001a97f0`, `sh2pc.exe` `0x0048b855`): `+X` door line; places player at `(x0 + len/2, y, z0 + 250)` facing `0.0` (`12` bytes).
    DoorLinePosX = 1,
    /// `2` (`SLUS_202.28` `0x001a983c`, `sh2pc.exe` `0x0048b888`): `-X` door line; places player at `(x0 - len/2, y, z0 - 250)` facing `PI` (`12` bytes).
    DoorLineNegX = 2,
    /// `3` (`SLUS_202.28` `0x001a9890`, `sh2pc.exe` `0x0048b8bb`): `-Z` door line; places player at `(x0 + 250, y, z0 - len/2)` facing `+PI/2` (`12` bytes).
    DoorLineNegZ = 3,
    /// `4` (`SLUS_202.28` `0x001a98e4`, `sh2pc.exe` `0x0048b8ee`): `+Z` door line; places player at `(x0 - 250, y, z0 + len/2)` facing `-PI/2` (`12` bytes).
    DoorLinePosZ = 4,
    /// `5` (`SLUS_202.28` `0x001a9938`, `sh2pc.exe` `0x0048b921`): arbitrary door segment `(x0, y, z0, dx, dz)`: places player at midpoint + `250.0` normal offset (`14` bytes).
    DoorSegment = 5,
}

impl EventDestShapeKind {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Point),
            1 => Some(Self::DoorLinePosX),
            2 => Some(Self::DoorLineNegX),
            3 => Some(Self::DoorLineNegZ),
            4 => Some(Self::DoorLinePosZ),
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
