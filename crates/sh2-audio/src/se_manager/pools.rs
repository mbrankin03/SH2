//! Per-frame `Se2dManager` (`SLUS_202.28` `0x001932f0` / `sh2pc.exe` `FUN_00516b90` `0x00516b90`),
//! `Se3dManager` (`0x00193790`), and `SeStop` (`0x00193a80`) pool update and control methods on
//! [`super::SePoolManager`].

use glam::Vec3;

use crate::bgm_manager::BgmDuckingMode;
use crate::sfx_bank::{se_call_pos_direction, se_call_pos_distance};

use super::types::{
    SeBackendAction, SeListenerContext, SE_2D_SLOT_COUNT, SE_3D_CHANNEL_COUNT, SE_3D_ID_THRESHOLD,
    SE_FLAG_MANAGED_2D, SE_FLAG_NO_DISTANCE, SE_FLAG_PRIO_1,
};
use super::SePoolManager;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, lines 5089/5096) 2D managed ducking ramp rate (`±dt * 0.5`).
pub const SE_2D_DUCK_RAMP_RATE: f32 = 0.5;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, line 5050) room ID override (`0xa7`).
pub const SE_2D_OVERRIDE_ROOM_0XA7: u16 = 0x00a7;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, line 5050) sound ID override (`0x44c6`) in room `0xa7`.
pub const SE_2D_OVERRIDE_ID_0X44C6: u32 = 0x44c6;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, `DAT_008a49d8..0x008a49e4`) fixed world position
/// for `0x44c6` in room `0xa7`.
pub const SE_2D_0X44C6_POSITION: Vec3 = Vec3::new(-100_000.0, -500.0, -60_500.0);

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, line 5059) flags (`0x0c` = `NO_DISTANCE | MANAGED_2D`).
pub const SE_2D_0X44C6_FLAGS: u32 = SE_FLAG_NO_DISTANCE | SE_FLAG_MANAGED_2D;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, lines 5061–5063) attenuation byte step (`-8` when `> 8`).
pub const SE_2D_0X44C6_ATTEN_STEP: u8 = 8;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, line 5066) sound ID override (`0x3cf0`).
pub const SE_2D_OVERRIDE_ID_0X3CF0: u32 = 0x3cf0;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, line 5067) player Z threshold (`-10000.0` mm) for `0x3cf0`.
pub const SE_2D_0X3CF0_PLAYER_Z_THRESHOLD: f32 = -10_000.0;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, lines 5070–5077: `0x45160000` = `2400.0`, `0xc61c4000` = `-10000.0`).
pub const SE_2D_0X3CF0_FIXED_Y: f32 = 2400.0;

/// Retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`, lines 5074–5077: `0xc61c4000` = `-10000.0`).
pub const SE_2D_0X3CF0_FIXED_Z: f32 = -10_000.0;

impl SePoolManager {
    /// Steps the 4-slot 2D SFX pool (`SLUS_202.28` `Se2dManager` `0x001932f0` / retail `sh2pc.exe`
    /// `FUN_00516b90` `0x00516b90`) assuming no backend voices have finished.
    pub fn tick_2d(
        &mut self,
        listener: &SeListenerContext,
        ducking_mode: BgmDuckingMode,
        dt_secs: f32,
        actions: &mut Vec<SeBackendAction>,
    ) {
        self.tick_2d_with_finished(
            listener,
            ducking_mode,
            dt_secs,
            &[false; SE_2D_SLOT_COUNT],
            actions,
        );
    }

    /// Steps the 4-slot 2D SFX pool with retail `sh2pc.exe` `FUN_00516b90` (`0x00516b90`) parity:
    /// - Auto-stops any slot whose `room_id` differs from `listener.room_id` (when `room_id != 0`).
    /// - Applies retail room `0xa7` / SE `0x44c6` override (`DAT_008a49d8..0x008a49e4`, `base_volume = 1.0`,
    ///   `flags = 0x0c`, `last_attenuation_byte -= 8` if `> 8`).
    /// - Applies retail SE `0x3cf0` override (`player_pos.z >= -10000.0` -> `Some((player_pos.x, 2400.0, -10000.0))`,
    ///   otherwise `None`).
    /// - Ramps each slot's `duck_fade` toward `ducking_mode.target_volume()` at `±dt * 0.5`.
    /// - Auto-stops any slot whose backend voice finished (`shSdSeChange == -1`, `FUN_00516b90` lines 5114–5116).
    /// - Calls `SeCallPosChange` with `base_volume * duck_fade` and `dt * 128.0` slew limit.
    /// - Advances `timer_secs += dt`.
    pub fn tick_2d_with_finished(
        &mut self,
        listener: &SeListenerContext,
        ducking_mode: BgmDuckingMode,
        dt_secs: f32,
        finished_slots: &[bool; SE_2D_SLOT_COUNT],
        actions: &mut Vec<SeBackendAction>,
    ) {
        let dt = dt_secs.max(0.0);
        let duck_step = dt * SE_2D_DUCK_RAMP_RATE;
        let target_duck = ducking_mode.target_volume();

        for idx in 0..SE_2D_SLOT_COUNT {
            let Some(mut slot) = self.slots_2d[idx] else {
                continue;
            };
            if listener.room_id != 0 && listener.room_id != slot.room_id {
                self.stop(slot.se_id, actions);
                continue;
            }

            // Retail `sh2pc.exe` `FUN_00516b90` lines 5050–5064: room `0xa7` + SE `0x44c6` override.
            if listener.room_id == SE_2D_OVERRIDE_ROOM_0XA7
                && slot.se_id == SE_2D_OVERRIDE_ID_0X44C6
            {
                slot.position = Some(SE_2D_0X44C6_POSITION);
                slot.base_volume = 1.0;
                slot.flags = SE_2D_0X44C6_FLAGS;
                if slot.last_attenuation_byte > SE_2D_0X44C6_ATTEN_STEP {
                    slot.last_attenuation_byte -= SE_2D_0X44C6_ATTEN_STEP;
                }
            }

            // Retail `sh2pc.exe` `FUN_00516b90` lines 5066–5082: SE `0x3cf0` player-Z position override.
            if slot.se_id == SE_2D_OVERRIDE_ID_0X3CF0 {
                if listener.player_pos.z >= SE_2D_0X3CF0_PLAYER_Z_THRESHOLD {
                    slot.position = Some(Vec3::new(
                        listener.player_pos.x,
                        SE_2D_0X3CF0_FIXED_Y,
                        SE_2D_0X3CF0_FIXED_Z,
                    ));
                } else {
                    slot.position = None;
                }
            }

            if target_duck < slot.duck_fade {
                slot.duck_fade = (slot.duck_fade - duck_step).max(target_duck);
            } else if slot.duck_fade < target_duck {
                slot.duck_fade = (slot.duck_fade + duck_step).min(target_duck);
            }
            let effective_vol = slot.base_volume * slot.duck_fade;
            let se_id = slot.se_id;
            let pos = slot.position;
            let flags = slot.flags;
            slot.timer_secs += dt;
            self.slots_2d[idx] = Some(slot);

            if finished_slots[idx] {
                // `shSdSeChange` returned `-1` (`FUN_00516b90` lines 5114–5116): auto-stop and free slot.
                self.stop(se_id, actions);
                continue;
            }

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

    /// Updates the world position and base volume of an active positional SE across either the
    /// 8-channel 3D pool (`Se3dControl` `0x00193980`) or the 4-slot managed 2D pool (`SePosMove`
    /// `0x00515910` / `SePosSet` `0x00515740`). Returns `true` if a matching active voice was updated.
    pub fn update_positional(&mut self, base_volume: f32, se_id: u32, position: Vec3) -> bool {
        if se_id == 0 {
            return false;
        }
        if self.control_3d(base_volume, se_id, position) {
            return true;
        }
        if let Some(Some(slot)) = self
            .slots_2d
            .iter_mut()
            .find(|s| s.is_some_and(|sl| sl.se_id == se_id))
        {
            slot.position = Some(position);
            slot.base_volume = base_volume.clamp(0.0, 1.0);
            return true;
        }
        false
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::se_manager::SE_FLAG_MANAGED_2D;

    #[test]
    fn non_positional_managed_2d_ambient_loops_and_ducks_on_overlay() {
        let mut pool = SePoolManager::new(Vec::new());
        let listener = SeListenerContext {
            camera_pos: Vec3::ZERO,
            player_pos: Vec3::ZERO,
            wvs_matrix: glam::Mat4::IDENTITY,
            room_id: 0x03,
        };
        let mut actions = Vec::new();
        // Start Forest wind `0x2742` non-positionally (`pos = None`, `flags = 0x08`).
        pool.call_pos(
            0.0,
            0x2742,
            None,
            SE_FLAG_MANAGED_2D,
            &listener,
            &mut actions,
        );
        assert!(matches!(
            actions.as_slice(),
            [SeBackendAction::Play2d {
                se_id: 0x2742,
                direction_deg: 0,
                attenuation_byte: 255,
                looping: true
            }]
        ));

        // Modulate base volume via `Se2dManageDataVolumeChange` (`0x00193630`) to 0.8 and ramp to Normal (1.0).
        actions.clear();
        pool.set_2d_volume(0x2742, 0.8, &mut actions);
        pool.tick_2d(&listener, BgmDuckingMode::Normal, 4.0, &mut actions);
        let normal_atten = actions
            .iter()
            .rev()
            .find_map(|a| match *a {
                SeBackendAction::Update2d {
                    se_id: 0x2742,
                    attenuation_byte,
                    ..
                } => Some(attenuation_byte),
                _ => None,
            })
            .expect("Normal tick should emit Update2d");
        // Effective volume = 0.8 * 1.0 = 0.8 -> atten ≈ (1.0 - 0.8) * 255 = 51
        assert!((i32::from(normal_atten) - 51).abs() <= 1);

        // Open Inventory/Map/Memo overlay (`BgmDuckingMode::PausedOrMenu` -> target 0.5).
        actions.clear();
        pool.tick_2d(&listener, BgmDuckingMode::PausedOrMenu, 4.0, &mut actions);
        let ducked_atten = actions
            .iter()
            .rev()
            .find_map(|a| match *a {
                SeBackendAction::Update2d {
                    se_id: 0x2742,
                    attenuation_byte,
                    ..
                } => Some(attenuation_byte),
                _ => None,
            })
            .expect("Overlay ducking tick should emit Update2d");
        // Effective volume = 0.8 * 0.5 = 0.4 -> atten ≈ (1.0 - 0.4) * 255 = 153
        assert!((i32::from(ducked_atten) - 153).abs() <= 1);
    }
}
