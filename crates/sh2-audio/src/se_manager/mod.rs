//! 1:1 Silent Hill 2 2D/3D SFX Channel Pool Managers (`Se2dManager` & `Se3dManager`, `SH2-137`).
//!
//! Ported directly from `SLUS_202.28` (`sound/sh_sound.c`) and cross-checked against `sh2pc.exe`
//! (`sh2pc_00510000.c`):
//! - **`SeCallPos`** (`0x00192270` / `sh2pc 0x00516e90`): Allocates 2D managed looping SFX slots
//!   (`flags & 8 != 0 && flags & 3 == 0`) and 3D channels (`se_id >= 40000`), handling priority-1
//!   (`flags & 3 == 1`) 2D↔3D promotion/demotion (`SeChange2Dto3D` `0x00194d90`, `SeChange3Dto2D`
//!   `0x00194e30`) and safe 2D demotion when all 8 3D channels are occupied.
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
//! - **Intentional fix on 8-channel exhaustion**: When `shSd3dCall` returns `-1` for a priority-1
//!   sound, both `SLUS_202.28` (`0x00192270` line 271) and `sh2pc.exe` (`0x00516e90` line 5309)
//!   passed `param_2` (`>= 40000`) unchanged into the recursive `flags - 1` call (a retail bug that
//!   retried 3D and returned `-1`). [`SePoolManager::call_pos`] intentionally demotes via
//!   `change_3d_to_2d(se_id)` before recursing with `flags - 1` so the 2D fallback actually succeeds.

pub mod change_list;
mod pools;
pub mod types;

pub use change_list::{se_3d_bank_for_stage, SE_CHANGE_LIST};
pub use pools::{
    SE_2D_0X3CF0_FIXED_Y, SE_2D_0X3CF0_FIXED_Z, SE_2D_0X3CF0_PLAYER_Z_THRESHOLD,
    SE_2D_0X44C6_ATTEN_STEP, SE_2D_0X44C6_FLAGS, SE_2D_0X44C6_POSITION, SE_2D_DUCK_RAMP_RATE,
    SE_2D_OVERRIDE_ID_0X3CF0, SE_2D_OVERRIDE_ID_0X44C6, SE_2D_OVERRIDE_ROOM_0XA7,
};
pub use types::{
    Se2d3dMapping, Se2dSlot, Se3dChannel, SeBackendAction, SeCallResult, SeListenerContext,
    SE_2D_SLEW_RATE_PER_SEC, SE_2D_SLOT_COUNT, SE_3D_CHANNEL_COUNT, SE_3D_ID_THRESHOLD,
    SE_3D_PRIORITY1_MAX, SE_FLAG_MANAGED_2D, SE_FLAG_NO_DISTANCE, SE_FLAG_PRIO_1,
};

use glam::Vec3;

use crate::sfx_bank::{se_call_pos_direction, se_call_pos_distance};

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

    /// Creates a new `SePoolManager` preloaded with the 241-entry retail `change_list`
    /// (`sh2pc.exe` `0x008a4b90`, `SLUS_202.28` `0x002b8880`).
    pub fn with_retail_change_list() -> Self {
        Self::new(SE_CHANGE_LIST.to_vec())
    }

    /// Resets all 2D slots and 3D channels (`SeCallReset` `0x00192060`).
    pub fn reset(&mut self) {
        self.slots_2d = [None; SE_2D_SLOT_COUNT];
        self.channels_3d = [None; SE_3D_CHANNEL_COUNT];
        self.se_3d_channel_max = SE_3D_PRIORITY1_MAX;
        self.se_3d_channel_number = 0;
        self.active_3d_bank = 0;
    }

    /// Sets `active_3d_bank` (`_se_3d_load_data`) for `(stage_id, room_id, is_night)`
    /// (`SeSoundEffect3dLoad` `SLUS_202.28` `0x00192fd0` / `sh2pc.exe` `0x008a4b90`).
    pub fn set_active_3d_bank_for_stage(&mut self, stage_id: u32, room_id: u32, is_night: bool) {
        self.active_3d_bank = se_3d_bank_for_stage(stage_id, room_id, is_night);
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
            // Intentional fix over `SLUS_202.28` (`0x00192270` line 271) / `sh2pc.exe` (`0x00516e90`
            // line 5309), which passed the 3D `se_id >= 40000` unchanged into the `flags - 1` retry
            // and returned `-1`: demote back to 2D via `change_3d_to_2d` so the fallback succeeds.
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
}
