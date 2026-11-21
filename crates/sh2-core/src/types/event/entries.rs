//! Stage descriptor entries (`EventListEntry`, `ItemListEntry`, `ModelListEntry`,
//! `EnemyListEntry`, `EventCamSetData`, `StageDataHeader`).

use super::enums::{EventDestShapeKind, EventResultKind, EventShapeKind, EventTriggerKind};

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

/// 16-byte stage item pickup descriptor (`Item_List` in `event.c`).
///
/// Exact 1:1 binary layout verified against `ItemCheckLookPoint` (`SLUS_202.28` `0x001a93b0`,
/// `sh2pc.exe` `0x0048b4d0`), `ItemListElement` (`SLUS_202.28` `0x001a9320`, `sh2pc.exe`
/// `0x0048b480`), and `EventExecItem` (`SLUS_202.28` `0x001aa260`):
/// - `+0x00`: `f32 pos_x` (PS2 world mm)
/// - `+0x04`: `f32 pos_z` (PS2 world mm)
/// - `+0x08`: `u16 pos_y` (Team Silent `f16` decoded via `CharToFloat2`)
/// - `+0x0A`: `u16 rot_y` (Team Silent `f16` decoded via `CharToFloat2`)
/// - `+0x0C`: `u32 flag` (packed kind, difficulty, extra condition, and base flag ID)
///
/// Prototype vs retail PC sentinel drift: `SLUS_202.28` (`0x001a8584`) terminates `Item_List`
/// when `(flag >> 29) == 7`, whereas retail Director's Cut `sh2pc.exe` (`0x0048e3b0`) shifted
/// Revolver Bullets into kind `7` and terminates when `(flag >> 29) == 0`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemListEntry {
    /// `+0x00`: World X coordinate in PS2 millimeters (`f32`).
    pub pos_x: f32,
    /// `+0x04`: World Z coordinate in PS2 millimeters (`f32`).
    pub pos_z: f32,
    /// `+0x08`: World Y coordinate encoded as Team Silent `f16` (decoded via [`super::char_to_float2`]).
    pub pos_y: u16,
    /// `+0x0A`: Yaw rotation encoded as Team Silent `f16` (decoded via [`super::char_to_float2`]).
    pub rot_y: u16,
    /// `+0x0C`: Packed kind (`>> 29`), difficulty (`>> 26 & 7`), extra condition (`>> 20 & 0x1f`),
    /// and base flag ID (`& 0x3fff`).
    pub flag: u32,
}

impl ItemListEntry {
    /// Retail PC `sh2pc.exe` (`0x0048e3b0`) end-of-table sentinel (`(flag >> 29) == 0`).
    pub const SENTINEL: Self = Self {
        pos_x: 0.0,
        pos_z: 0.0,
        pos_y: 0,
        rot_y: 0,
        flag: 0,
    };

    /// Returns `true` if this entry is an end-of-table sentinel (`(flag >> 29) == 0` in retail
    /// `sh2pc.exe` `0x0048e3b0`, or `flag_id() == 0x3fff`).
    #[inline]
    pub const fn is_sentinel(&self) -> bool {
        self.item_kind_index() == 0 || self.flag_id() == 0x3fff
    }

    /// Extracts field `kind` (`0..=3`) exactly as `ItemListElement` (`SLUS_202.28` `0x001a9320`,
    /// `sh2pc.exe` `0x0048b480`).
    pub const fn element(&self, kind: u32) -> i32 {
        match kind {
            0 => (self.flag >> 29) as i32,
            1 => ((self.flag >> 26) & 0x7) as i32,
            2 => ((self.flag >> 20) & 0x1f) as i32,
            3 => (self.flag & 0x3fff) as i32,
            _ => 0,
        }
    }

    /// Selector `0`: Supply kind index (`flag >> 29`, `1..=7` in retail `sh2pc.exe`; `0` = sentinel).
    #[inline]
    pub const fn item_kind_index(&self) -> u8 {
        (self.flag >> 29) as u8
    }

    /// Selector `1`: Action difficulty condition (`(flag >> 26) & 0x7`).
    #[inline]
    pub const fn difficulty_condition(&self) -> u8 {
        ((self.flag >> 26) & 0x7) as u8
    }

    /// Selector `2`: Extra flag condition (`(flag >> 20) & 0x1f`).
    #[inline]
    pub const fn extra_condition(&self) -> u8 {
        ((self.flag >> 20) & 0x1f) as u8
    }

    /// Selector `3`: Base visibility flag ID (`flag & 0x3fff`); picking up sets `flag_id + 1`.
    #[inline]
    pub const fn flag_id(&self) -> u16 {
        (self.flag & 0x3fff) as u16
    }

    /// World Y coordinate in PS2 millimeters decoded from `+0x08` via [`super::char_to_float2`]
    /// (`ItemCheckLookPoint` `0x001a93b0`, `EventExecItem` `0x001aa260`).
    #[inline]
    pub const fn pos_y_mm(&self) -> f32 {
        super::char_to_float2(self.pos_y.to_le_bytes())
    }

    /// Yaw rotation in radians decoded from `+0x0A` via [`super::char_to_float2`].
    #[inline]
    pub const fn rot_y_rad(&self) -> f32 {
        super::char_to_float2(self.rot_y.to_le_bytes())
    }

    /// World-space 3D position `[x, y, z]` in PS2 millimeters.
    #[inline]
    pub const fn position(&self) -> [f32; 3] {
        [self.pos_x, self.pos_y_mm(), self.pos_z]
    }
}

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
