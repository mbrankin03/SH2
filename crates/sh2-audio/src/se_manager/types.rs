//! Types, constants, and backend action definitions for `Se2dManager` & `Se3dManager` (`SH2-137`).
//!
//! Extracted from `SLUS_202.28` (`sound/sh_sound.c`) and cross-checked against `sh2pc.exe`
//! (`sh2pc_00510000.c`).

use glam::{Mat4, Vec3};

/// Number of persistent 2D managed SFX slots (`se_2d_manage_data` `0x010e0750`, 4 × `0x40` bytes).
pub const SE_2D_SLOT_COUNT: usize = 4;

/// Number of 3D SFX channels (`se_3d_channel_data` `0x010e05d0`, 8 × `0x30` bytes).
pub const SE_3D_CHANNEL_COUNT: usize = 8;

/// Maximum concurrent priority-1 (`flags & 3 == 1`) 3D channels (`se_3d_channel_max` `0x010e58d0`).
pub const SE_3D_PRIORITY1_MAX: usize = 4;

/// Threshold separating 2D SPU2 sound IDs (`< 40000`) from 3D sound IDs (`>= 40000`)
/// (`SeCallPos` `0x00192270`, `SeStop` `0x00193a80`, `SeChange2Dto3D` `0x00194d90`).
pub const SE_3D_ID_THRESHOLD: u32 = 40_000;

/// Slew-rate limit in driver units per second (`(int)(dt * 128.0)`) applied to attenuation byte and
/// direction angle in `SeCallPosChange` (`SLUS_202.28` `0x001926e0` / `sh2pc.exe` `0x005169f0`).
pub const SE_2D_SLEW_RATE_PER_SEC: f32 = 128.0;

/// `SeCallPos` flag bit: Priority-1 3D-eligible sound (`flags & 3 == 1`, `0x00192270`).
pub const SE_FLAG_PRIO_1: u32 = 0x01;

/// `SeCallPos` flag bit: Bypass 3D distance attenuation (`flags & 4 != 0`, `0x00192270`).
pub const SE_FLAG_NO_DISTANCE: u32 = 0x04;

/// `SeCallPos` flag bit: Register in 4-slot `Se2dManager` pool (`flags & 8 != 0`, `0x00192270`).
pub const SE_FLAG_MANAGED_2D: u32 = 0x08;

/// Mapping entry between a 2D sound ID (`< 40000`) and its 3D counterpart (`>= 40000`) for a loaded
/// 3D bank (`change_list` at `0x002b8880` in `SLUS_202.28`, `0x008a4b90` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Se2d3dMapping {
    /// 2D sound ID (`< 40000`).
    pub id_2d: u32,
    /// Paired 3D sound ID (`>= 40000`).
    pub id_3d: u32,
    /// Loaded 3D sound bank ID (`_se_3d_load_data`, `1..=34`).
    pub bank_id: u32,
}

/// Action emitted by [`super::SePoolManager`] when dispatching to the underlying audio backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeBackendAction {
    /// Start a 2D voice (`shSdCall` `0x00192270`).
    Play2d {
        se_id: u32,
        direction_deg: i32,
        attenuation_byte: u8,
    },
    /// Update an active 2D voice (`shSdSeChange` `0x001926e0`).
    Update2d {
        se_id: u32,
        direction_deg: i32,
        attenuation_byte: u8,
    },
    /// Stop a 2D voice (`shSdSeStop` `0x00193a80`).
    Stop2d { se_id: u32 },
    /// Start a 3D channel (`shSd3dCall` `0x00192270`).
    Play3d {
        channel: usize,
        se_id: u32,
        direction_deg: i32,
        attenuation_byte: u8,
    },
    /// Update an active 3D channel (`shSd3dMove` `0x00193790`).
    Update3d {
        channel: usize,
        se_id: u32,
        direction_deg: i32,
        attenuation_byte: u8,
    },
    /// Stop a 3D channel (`shSd3dStop` `0x00193a80`).
    Stop3d { channel: usize, se_id: u32 },
}

/// Outcome of [`super::SePoolManager::call_pos`] (`SeCallPos` `0x00192270`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeCallResult {
    /// Dispatched as a 2D sound (`0x10`).
    Played2d { se_id: u32 },
    /// Dispatched on a 3D channel index (`0..8`).
    Played3d { channel: usize, se_id: u32 },
    /// Rejected / not played (`0xf` or `-1`: `se_id == 0`, duplicate 2D slot, pool full, or no fallback).
    NotPlayed,
}

/// Single slot in the 4-slot `Se2dManager` pool (`se_2d_manage_data` `0x010e0750`, `0x40` bytes/slot).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Se2dSlot {
    /// Active sound ID (`0` = slot free).
    pub se_id: u32,
    /// Room ID where the sound was spawned (`RoomNameJms()` at `+0x24`).
    pub room_id: u16,
    /// Optional 3D world position (`+0x10`, `pos_valid` at `+0x26`).
    pub position: Option<Vec3>,
    /// Caller base volume `[0.0, 1.0]` (`+0x20`).
    pub base_volume: f32,
    /// `SeCallPos` flags (`+0x2a`).
    pub flags: u32,
    /// Elapsed active time in seconds (`+0x2c`, queried by `Se2dManageDataTimer` `0x001936f0`).
    pub timer_secs: f32,
    /// Current pause/demo ducking scalar (`+0x30`, ramps toward `0.5`/`0.7`/`1.0` at `±dt`).
    pub duck_fade: f32,
    /// Last slewed driver attenuation byte `0..=255` (`+0x27`).
    pub last_attenuation_byte: u8,
    /// Last slewed direction angle in degrees (`+0x28`).
    pub last_direction_deg: i16,
}

/// Single channel in the 8-channel `Se3dManager` pool (`se_3d_channel_data` `0x010e05d0`, `0x30` bytes/ch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Se3dChannel {
    /// Active 3D sound ID (`>= 40000`, `0` = channel free).
    pub se_id: u32,
    /// Room ID where the channel was spawned (`+0x04`).
    pub room_id: u16,
    /// `SeCallPos` flags (`+0x08`).
    pub flags: u32,
    /// 3D world position (`+0x10`).
    pub position: Vec3,
    /// Caller base volume `[0.0, 1.0]` (`+0x20`).
    pub base_volume: f32,
}

/// Listener spatial context passed into [`super::SePoolManager`] calls.
#[derive(Debug, Clone, Copy)]
pub struct SeListenerContext {
    /// Active camera position (`vcWork.cam_pos` `0x010e5b40`).
    pub camera_pos: Vec3,
    /// Active player position (`sh2jms + 0x20`).
    pub player_pos: Vec3,
    /// World-to-view matrix (`VbWvsMatrix` `0x010e59d0`).
    pub wvs_matrix: Mat4,
    /// Active room ID (`RoomNameJms()`).
    pub room_id: u16,
}
