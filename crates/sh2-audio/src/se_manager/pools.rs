//! Per-frame `Se2dManager` (`0x001932f0`), `Se3dManager` (`0x00193790`), and `SeStop` (`0x00193a80`)
//! pool update and control methods on [`super::SePoolManager`].

use glam::Vec3;

use crate::bgm_manager::BgmDuckingMode;
use crate::sfx_bank::{se_call_pos_direction, se_call_pos_distance};

use super::types::{
    SeBackendAction, SeListenerContext, SE_2D_SLOT_COUNT, SE_3D_CHANNEL_COUNT, SE_3D_ID_THRESHOLD,
    SE_FLAG_NO_DISTANCE, SE_FLAG_PRIO_1,
};
use super::SePoolManager;

impl SePoolManager {
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

    /// Steps the 8-channel 3D SFX pool (`Se3dManager` `0x00193790`).
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
