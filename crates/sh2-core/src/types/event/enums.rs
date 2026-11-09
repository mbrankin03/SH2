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
