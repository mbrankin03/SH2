//! 1:1 Silent Hill 2 2D/3D SFX Channel Pool Managers (`Se2dManager` & `Se3dManager`, `SH2-137`).
//!
//! Ported directly from `SLUS_202.28` (`sound/sh_sound.c`) and cross-checked against `sh2pc.exe`
//! (`sh2pc_00510000.c`):
//! - **`SeCallPos`** (`0x00192270` / `sh2pc 0x00516e90`): Allocates 2D managed looping SFX slots
//!   (`flags & 8 != 0 && flags & 3 == 0`) and 3D channels (`se_id >= 40000`), handling priority-1
//!   (`flags & 3 == 1`) 2D↔3D promotion/demotion (`SeChange2Dto3D` `0x00194d90`, `SeChange3Dto2D`
//!   `0x00194e30`) and automatic fallback to 2D when 3D channels are exhausted.
//! - **`Se2dManager`** (`0x001932f0` / `sh2pc 0x00516b90`): Manages the 4-slot persistent 2D SFX
//!   pool (`se_2d_manage_data` `0x010e0750`) with room-change auto-stop, pause (`0.5`) / demo (`0.7`)
//!   / normal (`1.0`) ducking, and `SeCallPosChange` (`0x001926e0`) `dt * 128.0` slew-rate limiting.
//! - **`Se3dManager`** (`0x00193790` / `sh2pc 0x00516dc0`): Manages the 8-channel 3D SFX pool
//!   (`se_3d_channel_data` `0x010e05d0`, max `4` concurrent priority-1 channels via `se_3d_channel_max`
//!   `0x010e58d0`), room-change auto-stop, and per-frame spatial updates.
//! - **`SeStop`** (`0x00193a80` / `sh2pc 0x00515f10`): Stops a sound effect across both 2D and paired
//!   3D channels.
//!
//! # Prototype (`SLUS_202.28`) vs. Retail PC (`sh2pc.exe`) Drift
//! - In `SLUS_202.28` (`Se2dManager` `0x001932f0`), 2D managed sounds duck to `0.5` during pause/menus,
//!   `0.7` during non-exempt drama demos, and `1.0` otherwise, ramping at `±dt`.
//! - In `sh2pc.exe` (`0x00516b90`), `Se2dManager` dropped the `0.7` demo ducking tier and slowed the
//!   ramp to `±dt * 0.5`. This module implements the full `SLUS_202.28` 3-tier (`0.5 / 0.7 / 1.0`)
//!   model via [`BgmDuckingMode`] at `±dt`.
//! - In `sh2pc.exe` (`0x00516e90`), calling `SeCallPos` with a `se_id` already present in
//!   `se_2d_manage_data` updates the existing slot and dispatches `SeCallPosChange`, whereas
//!   `SLUS_202.28` (`0x00192270` line 195) returned `0xf` without modifying the slot.

use glam::{Mat4, Vec3};

use crate::bgm_manager::BgmDuckingMode;
use crate::sfx_bank::{se_call_pos_direction, se_call_pos_distance};

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

/// Action emitted by [`SePoolManager`] when dispatching to the underlying audio backend.
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

/// Outcome of [`SePoolManager::call_pos`] (`SeCallPos` `0x00192270`).
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

/// Listener spatial context passed into [`SePoolManager`] calls.
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

/// Combined 4-slot `Se2dManager` (`0x001932f0`) and 8-channel `Se3dManager` (`0x00193790`) pool state.
#[derive(Debug, Clone)]
pub struct SePoolManager {
    /// 4-slot persistent 2D managed SFX table (`se_2d_manage_data`).
    pub slots_2d: [Option<Se2dSlot>; SE_2D_SLOT_COUNT],
    /// 8-channel 3D SFX table (`se_3d_channel_data`).
    pub channels_3d: [Option<Se3dChannel>; SE_3D_CHANNEL_COUNT],
    /// Maximum concurrent priority-1 (`flags & 3 == 1`) 3D channels (`se_3d_channel_max`, default `4`).
    pub se_3d_channel_max: usize,
    /// Current count of active priority-1 3D channels (`se_3d_channel_number`).
    pub se_3d_channel_number: usize,
    /// Active loaded 3D sound bank ID (`_se_3d_load_data`, `0` = none).
    pub active_3d_bank: u32,
    /// 2D↔3D sound ID mapping table (`change_list` `0x002b8880`).
    change_list: Vec<Se2d3dMapping>,
}

impl Default for SePoolManager {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl SePoolManager {
    /// Creates a new `SePoolManager` initialized per `SeCallReset` (`0x00192060`).
    pub fn new(change_list: Vec<Se2d3dMapping>) -> Self {
        Self {
            slots_2d: [None; SE_2D_SLOT_COUNT],
            channels_3d: [None; SE_3D_CHANNEL_COUNT],
            se_3d_channel_max: SE_3D_PRIORITY1_MAX,
            se_3d_channel_number: 0,
            active_3d_bank: 0,
            change_list,
        }
    }

    /// Resets all 2D slots and 3D channels (`SeCallReset` `0x00192060`).
    pub fn reset(&mut self) {
        self.slots_2d = [None; SE_2D_SLOT_COUNT];
        self.channels_3d = [None; SE_3D_CHANNEL_COUNT];
        self.se_3d_channel_max = SE_3D_PRIORITY1_MAX;
        self.se_3d_channel_number = 0;
        self.active_3d_bank = 0;
    }

    /// Looks up the 3D counterpart of a 2D `se_id` in the active 3D bank (`SeChange2Dto3D` `0x00194d90`).
    pub fn change_2d_to_3d(&self, se_id: u32) -> u32 {
        if se_id >= SE_3D_ID_THRESHOLD {
            return se_id;
        }
        for entry in &self.change_list {
            if entry.bank_id == self.active_3d_bank && entry.id_2d == se_id {
                return entry.id_3d;
            }
        }
        0
    }

    /// Looks up the 2D counterpart of a 3D `se_id` (`SeChange3Dto2D` `0x00194e30`).
    pub fn change_3d_to_2d(&self, se_id: u32) -> u32 {
        if se_id < SE_3D_ID_THRESHOLD {
            return se_id;
        }
        for entry in &self.change_list {
            if entry.id_3d == se_id {
                return entry.id_2d;
            }
        }
        0
    }

    /// Returns `true` if `se_id` is currently active in any 3D channel (`Se3dPlayCheck` `0x00192cf0`).
    pub fn is_3d_playing(&self, se_id: u32) -> bool {
        self.channels_3d
            .iter()
            .any(|ch| ch.is_some_and(|c| c.se_id == se_id))
    }

    /// Dispatches a positional sound effect (`SeCallPos` `0x00192270`), recording backend actions into `actions`.
    pub fn call_pos(
        &mut self,
        base_volume: f32,
        mut se_id: u32,
        position: Option<Vec3>,
        flags: u32,
        listener: &SeListenerContext,
        actions: &mut Vec<SeBackendAction>,
    ) -> SeCallResult {
        if se_id == 0 {
            return SeCallResult::NotPlayed;
        }

        // 1. Register managed 2D looping sound (`(flags & 8) != 0 && (flags & 3) == 0`, lines 192-230)
        let mut managed_slot_idx: Option<usize> = None;
        if (flags & SE_FLAG_MANAGED_2D) != 0 && (flags & 3) == 0 {
            if self
                .slots_2d
                .iter()
                .any(|s| s.is_some_and(|slot| slot.se_id == se_id))
            {
                return SeCallResult::NotPlayed;
            }
            let Some(free_idx) = self.slots_2d.iter().position(|s| s.is_none()) else {
                return SeCallResult::NotPlayed;
            };
            self.slots_2d[free_idx] = Some(Se2dSlot {
                se_id,
                room_id: listener.room_id,
                position,
                base_volume: base_volume.clamp(0.0, 1.0),
                flags,
                timer_secs: 0.0,
                duck_fade: 0.0,
                last_attenuation_byte: 0,
                last_direction_deg: 0,
            });
            managed_slot_idx = Some(free_idx);
        }

        // 2. Priority-1 2D↔3D routing (`(flags & 3) == 1`, lines 231-244)
        if (flags & 3) == SE_FLAG_PRIO_1 {
            if self.se_3d_channel_number < self.se_3d_channel_max {
                let promoted = self.change_2d_to_3d(se_id);
                if promoted != 0 {
                    se_id = promoted;
                }
            } else {
                se_id = self.change_3d_to_2d(se_id);
                if se_id == 0 {
                    return SeCallResult::NotPlayed;
                }
            }
        }

        // 3. Evaluate spatial attenuation & direction (lines 245-260)
        let atten_byte = if (flags & SE_FLAG_NO_DISTANCE) == 0 {
            if let Some(pos) = position {
                se_call_pos_distance(base_volume, listener.camera_pos, listener.player_pos, pos)
            } else {
                (255.0 - base_volume.clamp(0.0, 1.0) * 255.0).clamp(0.0, 255.0) as u8
            }
        } else {
            (255.0 - base_volume.clamp(0.0, 1.0) * 255.0).clamp(0.0, 255.0) as u8
        };
        let dir_deg = match position {
            Some(pos) => se_call_pos_direction(listener.wvs_matrix, pos),
            None => 0,
        };

        if let Some(idx) = managed_slot_idx {
            if let Some(slot) = &mut self.slots_2d[idx] {
                slot.last_attenuation_byte = atten_byte;
                slot.last_direction_deg = dir_deg as i16;
            }
        }

        // 4. Dispatch 2D (< 40000) vs 3D (>= 40000) (lines 261-293)
        if se_id < SE_3D_ID_THRESHOLD {
            actions.push(SeBackendAction::Play2d {
                se_id,
                direction_deg: dir_deg,
                attenuation_byte: atten_byte,
            });
            return SeCallResult::Played2d { se_id };
        }

        if let Some(ch_idx) = self.channels_3d.iter().position(|c| c.is_none()) {
            self.channels_3d[ch_idx] = Some(Se3dChannel {
                se_id,
                room_id: listener.room_id,
                flags,
                position: position.unwrap_or(Vec3::ZERO),
                base_volume: base_volume.clamp(0.0, 1.0),
            });
            if (flags & 3) == SE_FLAG_PRIO_1 {
                self.se_3d_channel_number += 1;
            }
            actions.push(SeBackendAction::Play3d {
                channel: ch_idx,
                se_id,
                direction_deg: dir_deg,
                attenuation_byte: atten_byte,
            });
            SeCallResult::Played3d {
                channel: ch_idx,
                se_id,
            }
        } else if (flags & 3) == SE_FLAG_PRIO_1 {
            // All 8 3D channels occupied: demote priority-1 sound back to 2D (`flags - 1`, line 271)
            let fallback_id = self.change_3d_to_2d(se_id);
            if fallback_id == 0 {
                return SeCallResult::NotPlayed;
            }
            self.call_pos(
                base_volume,
                fallback_id,
                position,
                flags - 1,
                listener,
                actions,
            )
        } else {
            SeCallResult::NotPlayed
        }
    }

    /// Updates a live 2D sound with `dt * 128.0` slew-rate limiting when `flags & 8 != 0`
    /// (`SeCallPosChange` `0x001926e0`).
    #[allow(clippy::too_many_arguments)]
    pub fn call_pos_change(
        &mut self,
        base_volume: f32,
        se_id: u32,
        position: Option<Vec3>,
        flags: u32,
        listener: &SeListenerContext,
        dt_secs: f32,
        actions: &mut Vec<SeBackendAction>,
    ) {
        if se_id == 0 {
            return;
        }
        let (mut dir_deg, mut atten_i32) = match position {
            None => (
                0,
                (255.0 - base_volume.clamp(0.0, 1.0) * 255.0).clamp(0.0, 255.0) as i32,
            ),
            Some(pos) => {
                let d = se_call_pos_direction(listener.wvs_matrix, pos);
                let a = if (flags & SE_FLAG_NO_DISTANCE) == 0 {
                    se_call_pos_distance(base_volume, listener.camera_pos, listener.player_pos, pos)
                        as i32
                } else {
                    (255.0 - base_volume.clamp(0.0, 1.0) * 255.0).clamp(0.0, 255.0) as i32
                };
                (d, a)
            }
        };

        if (flags & SE_FLAG_MANAGED_2D) != 0 {
            if let Some(Some(slot)) = self
                .slots_2d
                .iter_mut()
                .find(|s| s.is_some_and(|sl| sl.se_id == se_id))
            {
                let max_step = (dt_secs.max(0.0) * SE_2D_SLEW_RATE_PER_SEC) as i32;
                let prev_atten = slot.last_attenuation_byte as i32;
                atten_i32 = atten_i32.clamp(prev_atten - max_step, prev_atten + max_step);
                slot.last_attenuation_byte = atten_i32.clamp(0, 255) as u8;

                let prev_dir = slot.last_direction_deg as i32;
                dir_deg = dir_deg.clamp(prev_dir - max_step, prev_dir + max_step);
                slot.last_direction_deg = dir_deg as i16;
            }
        }

        actions.push(SeBackendAction::Update2d {
            se_id,
            direction_deg: dir_deg,
            attenuation_byte: atten_i32.clamp(0, 255) as u8,
        });
    }

    /// Steps the 4-slot 2D SFX pool (`Se2dManager` `0x001932f0`):
    /// - Auto-stops any slot whose `room_id` differs from `listener.room_id` (when `room_id != 0`).
    /// - Ramps each slot's `duck_fade` toward `ducking_mode.target_volume()` (`0.5`/`0.7`/`1.0`) at `±dt`.
    /// - Calls `SeCallPosChange` with `base_volume * duck_fade` and `dt * 128.0` slew limit.
    /// - Advances `timer_secs += dt`.
    pub fn tick_2d(
        &mut self,
        listener: &SeListenerContext,
        ducking_mode: BgmDuckingMode,
        dt_secs: f32,
        actions: &mut Vec<SeBackendAction>,
    ) {
        let dt = dt_secs.max(0.0);
        let target_duck = ducking_mode.target_volume();

        for idx in 0..SE_2D_SLOT_COUNT {
            let Some(mut slot) = self.slots_2d[idx] else {
                continue;
            };
            if listener.room_id != 0 && listener.room_id != slot.room_id {
                self.stop(slot.se_id, actions);
                continue;
            }

            if target_duck < slot.duck_fade {
                slot.duck_fade = (slot.duck_fade - dt).max(target_duck);
            } else if slot.duck_fade < target_duck {
                slot.duck_fade = (slot.duck_fade + dt).min(target_duck);
            }
            let effective_vol = slot.base_volume * slot.duck_fade;
            let se_id = slot.se_id;
            let pos = slot.position;
            let flags = slot.flags;
            slot.timer_secs += dt;
            self.slots_2d[idx] = Some(slot);

            self.call_pos_change(effective_vol, se_id, pos, flags, listener, dt, actions);
        }
    }

    /// Updates a managed 2D slot's base volume (`Se2dManageDataVolumeChange` `0x00193630`).
    /// Passing a negative volume stops the sound.
    pub fn set_2d_volume(&mut self, se_id: u32, volume: f32, actions: &mut Vec<SeBackendAction>) {
        if let Some(slot_opt) = self
            .slots_2d
            .iter_mut()
            .find(|s| s.is_some_and(|sl| sl.se_id == se_id))
        {
            if volume < 0.0 {
                self.stop(se_id, actions);
            } else if let Some(slot) = slot_opt {
                slot.base_volume = volume.min(1.0);
            }
        }
    }

    /// Queries (and optionally resets) a managed 2D slot's elapsed timer (`Se2dManageDataTimer` `0x001936f0`).
    /// Returns `-1.0` if `se_id` is not active.
    pub fn query_2d_timer(&mut self, se_id: u32, reset: bool) -> f32 {
        if let Some(Some(slot)) = self
            .slots_2d
            .iter_mut()
            .find(|s| s.is_some_and(|sl| sl.se_id == se_id))
        {
            if reset {
                slot.timer_secs = 0.0;
            }
            slot.timer_secs
        } else {
            -1.0
        }
    }

    /// Steps the 8-channel 3D SFX pool (`Se3dManager` `0x00193790`):
    /// - Auto-stops any channel whose `room_id != listener.room_id`.
    /// - Recomputes `SeCallPosDirection` and `SeCallPosDistance` and emits `Update3d`.
    /// - `finished_channels` marks channels whose backend stream ended (`shSd3dMove == -1`), freeing them.
    pub fn tick_3d(
        &mut self,
        listener: &SeListenerContext,
        finished_channels: &[bool; SE_3D_CHANNEL_COUNT],
        actions: &mut Vec<SeBackendAction>,
    ) {
        for ch_idx in 0..SE_3D_CHANNEL_COUNT {
            let Some(ch) = self.channels_3d[ch_idx] else {
                continue;
            };
            if listener.room_id != ch.room_id {
                self.stop(ch.se_id, actions);
                continue;
            }

            if finished_channels[ch_idx] {
                self.channels_3d[ch_idx] = None;
                if (ch.flags & 3) == SE_FLAG_PRIO_1 {
                    self.se_3d_channel_number = self.se_3d_channel_number.saturating_sub(1);
                }
                continue;
            }

            let dir_deg = se_call_pos_direction(listener.wvs_matrix, ch.position);
            let atten_byte = if (ch.flags & SE_FLAG_NO_DISTANCE) == 0 {
                se_call_pos_distance(
                    ch.base_volume,
                    listener.camera_pos,
                    listener.player_pos,
                    ch.position,
                )
            } else {
                (255.0 - ch.base_volume.clamp(0.0, 1.0) * 255.0).clamp(0.0, 255.0) as u8
            };
            actions.push(SeBackendAction::Update3d {
                channel: ch_idx,
                se_id: ch.se_id,
                direction_deg: dir_deg,
                attenuation_byte: atten_byte,
            });
        }
    }

    /// Updates the position and base volume of an active 3D channel (`Se3dControl` `0x00193980`).
    /// Returns `true` if found and updated.
    pub fn control_3d(&mut self, base_volume: f32, se_id: u32, position: Vec3) -> bool {
        if se_id == 0 {
            return false;
        }
        if let Some(Some(ch)) = self
            .channels_3d
            .iter_mut()
            .find(|c| c.is_some_and(|ch| ch.se_id == se_id))
        {
            ch.position = position;
            ch.base_volume = base_volume.clamp(0.0, 1.0);
            true
        } else {
            false
        }
    }

    /// Stops `se_id` across both 2D slots and paired 3D channels (`SeStop` `0x00193a80`).
    pub fn stop(&mut self, mut se_id: u32, actions: &mut Vec<SeBackendAction>) {
        if se_id == 0 {
            return;
        }
        if se_id < SE_3D_ID_THRESHOLD {
            actions.push(SeBackendAction::Stop2d { se_id });
            for slot in &mut self.slots_2d {
                if slot.is_some_and(|s| s.se_id == se_id) {
                    *slot = None;
                    break;
                }
            }
            se_id = self.change_2d_to_3d(se_id);
            if se_id == 0 {
                return;
            }
        } else {
            let id_2d = self.change_3d_to_2d(se_id);
            if id_2d != 0 {
                actions.push(SeBackendAction::Stop2d { se_id: id_2d });
            }
        }

        for (ch_idx, ch_opt) in self.channels_3d.iter_mut().enumerate() {
            if let Some(ch) = *ch_opt {
                if ch.se_id == se_id {
                    actions.push(SeBackendAction::Stop3d {
                        channel: ch_idx,
                        se_id,
                    });
                    *ch_opt = None;
                    if (ch.flags & 3) == SE_FLAG_PRIO_1 {
                        self.se_3d_channel_number = self.se_3d_channel_number.saturating_sub(1);
                    }
                }
            }
        }
    }
}
