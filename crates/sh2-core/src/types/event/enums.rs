//! Stage event trigger, spatial shape, destination placement, and result action enumerations
//! (`SLUS_202.28` `src/Event/event.c`).

/// Trigger condition type (`(cond >> 28) & 0xf`, `EventListElement` selector `7`).
///
/// Verified 1:1 against `EventCheck` (`SLUS_202.28` `0x001a7ed0`, `sh2pc.exe` `0x0048e070`):
/// - `0`, `1`: Unconditional every frame when `flag_0`/`flag_1` hold (no button or shape test).
/// - `2`: Action button pressed + `EventCheckIn` (`shape 11..=13`) or `EventCheckLook` (`0..=10`).
/// - `3`: Passive look (`EventCheckLook`, `shape 0..=10`, no button required).
/// - `4`: Passive area (`EventCheckIn`, `shape 11..=13`, no button required).
/// - `5`: Menu item use (`used_item_id == required_item_id`) + `EventCheckLook` (`0..=10`).
/// - `6`: Menu item use (`used_item_id == required_item_id`) + `EventCheckIn` (`11..=13`).
/// - `7`: Action button pressed while carrying `required_item_id` + `In` (`11..=13`) or `Look` (`0..=10`).
/// - `8`: Carrying `required_item_id` + passive `EventCheckLook` (`0..=10`).
/// - `9`: Carrying `required_item_id` + passive `EventCheckIn` (`11..=13`).
/// - `10`: `(Action + carrying required_item_id) || (menu use required_item_id)` + `In` or `Look`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventTriggerKind {
    /// `0`: Unconditional every frame when `flag_0`/`flag_1` hold (`SLUS_202.28` `0x001a81a8`, `sh2pc.exe` `0x0048e1de`).
    Unconditional0 = 0,
    /// `1`: Unconditional every frame when `flag_0`/`flag_1` hold (`SLUS_202.28` `0x001a81a8`, `sh2pc.exe` `0x0048e1de`).
    Unconditional1 = 1,
    /// `2`: Action button pressed + `EventCheckIn` (`shape 11..=13`) or `EventCheckLook` (`shape 0..=10`).
    Action = 2,
    /// `3`: Passive `EventCheckLook` (`shape 0..=10`, no button required).
    PassiveLook = 3,
    /// `4`: Passive `EventCheckIn` (`shape 11..=13`, no button required).
    PassiveIn = 4,
    /// `5`: Menu item use (`used_item_id == required_item_id`) + `EventCheckLook` (`shape 0..=10`).
    UseItemLook = 5,
    /// `6`: Menu item use (`used_item_id == required_item_id`) + `EventCheckIn` (`shape 11..=13`).
    UseItemIn = 6,
    /// `7`: Action button pressed while carrying `required_item_id` + `In` (`11..=13`) or `Look` (`0..=10`).
    ActionHasItem = 7,
    /// `8`: Carrying `required_item_id` + passive `EventCheckLook` (`shape 0..=10`).
    PassiveHasItemLook = 8,
    /// `9`: Carrying `required_item_id` + passive `EventCheckIn` (`shape 11..=13`).
    PassiveHasItemIn = 9,
    /// `10`: `(Action + carrying required_item_id) || (menu use required_item_id)` + `In` (`11..=13`) or `Look` (`0..=10`).
    ActionHasItemOrUseItem = 10,
}

impl EventTriggerKind {
    /// Converts a 4-bit raw trigger nibble (`0..=10`) into [`EventTriggerKind`].
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Unconditional0),
            1 => Some(Self::Unconditional1),
            2 => Some(Self::Action),
            3 => Some(Self::PassiveLook),
            4 => Some(Self::PassiveIn),
            5 => Some(Self::UseItemLook),
            6 => Some(Self::UseItemIn),
            7 => Some(Self::ActionHasItem),
            8 => Some(Self::PassiveHasItemLook),
            9 => Some(Self::PassiveHasItemIn),
            10 => Some(Self::ActionHasItemOrUseItem),
            _ => None,
        }
    }

    /// Returns `true` if `EventCheck` (`SLUS_202.28` `0x001a8330`, `sh2pc.exe` `0x0048e2d8`)
    /// consumes `required_item_id` when `entry.consume_item()` is set (triggers `5..=10`).
    #[inline]
    pub const fn supports_consume_item(self) -> bool {
        matches!(
            self,
            Self::UseItemLook
                | Self::UseItemIn
                | Self::ActionHasItem
                | Self::PassiveHasItemLook
                | Self::PassiveHasItemIn
                | Self::ActionHasItemOrUseItem
        )
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
///
/// Verified 1:1 against `EventMainStandard` (`SLUS_202.28` `0x001a7b50`, `sh2pc.exe` `0x0048e610`):
/// `1=FlagSet`, `2=FlagClear`, `3=Program`, `4=Move`, `5=MovePlusProgram`, `6=Message`,
/// `7=DoorLocked`, `8=DoorBroken`, `9=DoorUnlocked`, `10=SavePoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventResultKind {
    /// `1`: Set flag (`rslt1 & 0x3fff`) via `EventExecFlag` (`_ev_m_step = 1`).
    FlagSet = 1,
    /// `2`: Clear flag (`rslt1 & 0x3fff`) via `EventExecFlag` (`_ev_m_step = 1`).
    FlagClear = 2,
    /// `3`: Execute stage program `ev_prog[(rslt1 >> 14) & 0x7f]` via `EventExecProgram` (`_ev_m_step = 2`).
    Program = 3,
    /// `4`: Move player to destination (`ev_pos[dest_offset]`), optionally changing stage (`EventExecMove`, `_ev_m_step = 5`).
    Move = 4,
    /// `5`: Move player (`EventExecMove`, `_ev_m_step = 3`) and then chain directly into `EventExecProgram` (`_ev_m_step = 4`).
    MovePlusProgram = 5,
    /// `6`: Display stage `.mes` message (`(rslt1 >> 14) & 0xff`) via `EventExecMessage` (`_ev_m_step = 6`).
    Message = 6,
    /// `7`: Locked door feedback (`EventExecDoor`: locked sound + message, `_ev_m_step = 7`).
    DoorLocked = 7,
    /// `8`: Broken lock feedback (`EventExecDoor`: broken sound + common message 4, `_ev_m_step = 7`).
    DoorBroken = 8,
    /// `9`: Unlocked door feedback (`EventExecDoor`: unlock sound + message, `_ev_m_step = 7`).
    DoorUnlocked = 9,
    /// `10`: Save point interaction (`EventExecSave`, `_ev_m_step = 9`).
    SavePoint = 10,
}

impl EventResultKind {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::FlagSet),
            2 => Some(Self::FlagClear),
            3 => Some(Self::Program),
            4 => Some(Self::Move),
            5 => Some(Self::MovePlusProgram),
            6 => Some(Self::Message),
            7 => Some(Self::DoorLocked),
            8 => Some(Self::DoorBroken),
            9 => Some(Self::DoorUnlocked),
            10 => Some(Self::SavePoint),
            _ => None,
        }
    }
}
