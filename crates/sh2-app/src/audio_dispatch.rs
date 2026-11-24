//! Shared [`SePoolManager`] (`Se2dManager` / `Se3dManager`) backend action dispatcher
//! and 3D bank synchronization for `sh2-app`.

use crate::state::AppState;
use sh2_audio::SeBackendAction;

/// Synchronizes `state.se_pool.active_3d_bank` (`_se_3d_load_data` in `SeSoundEffect3dLoad`
/// `SLUS_202.28` `0x00192fd0` / `sh2pc.exe` `0x008a4b90`) with the active `(stage_id, room_id, is_night)`.
///
/// `is_night` corresponds to `flag(251)` (`_DAT_0112644c >> 0x1b & 1` at `flag_work + 0x1C`).
pub(crate) fn sync_se_pool_3d_bank(state: &mut AppState) {
    let (stage_id, _) = state.active_bgm_stage_and_flags();
    let room_id = state.bgm_manager.room_id;
    let is_night = state.sim_core.scenario.has_flag(251);
    state
        .se_pool
        .set_active_3d_bank_for_stage(stage_id, room_id, is_night);
}

/// Dispatches all [`SeBackendAction`]s emitted by [`sh2_audio::SePoolManager::call_pos`],
/// `tick_2d`, `tick_3d`, or `stop` to [`AppState::audio_engine`] and [`AppState::audio_bank`].
pub(crate) fn dispatch_se_pool_actions(state: &mut AppState, actions: &[SeBackendAction]) {
    if actions.is_empty() {
        return;
    }
    let player_pos = state.sim_core.scenario.player_position;
    let (listener_yaw, listener_pitch) = state.sim_core.active_listener_yaw_pitch(1.0);
    let is_maria = state.active_maria;
    let collision = &state.sim_core.collision;

    let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_mut()) else {
        return;
    };

    for &action in actions {
        match action {
            SeBackendAction::Play2d {
                se_id,
                attenuation_byte,
                ..
            } => {
                let vol = ((255.0 - f32::from(attenuation_byte)) / 255.0).clamp(0.0, 1.0);
                let _ = bank.play_se_id(eng, se_id, vol);
            }
            SeBackendAction::Update2d {
                se_id,
                direction_deg,
                attenuation_byte,
            } => {
                if let Some(handle) = bank.active_spatial_handles.get_mut(&se_id) {
                    let vol = ((255.0 - f32::from(attenuation_byte)) / 255.0).clamp(0.0, 1.0);
                    let pan = (-(direction_deg as f32) / 70.0).clamp(-1.0, 1.0);
                    eng.update_sound_spatial(handle, vol, pan);
                }
            }
            SeBackendAction::Stop2d { se_id } | SeBackendAction::Stop3d { se_id, .. } => {
                bank.stop_spatial_se(se_id);
            }
            SeBackendAction::Play3d { channel, se_id, .. } => {
                if let Some(ch) = state.se_pool.channels_3d.get(channel).and_then(|c| *c) {
                    let _ = bank.play_se_id_3d_tracked_with_occlusion(
                        eng,
                        se_id,
                        player_pos,
                        listener_yaw,
                        listener_pitch,
                        is_maria,
                        ch.position,
                        ch.base_volume,
                        Some(collision),
                    );
                }
            }
            SeBackendAction::Update3d { channel, se_id, .. } => {
                if let Some(ch) = state.se_pool.channels_3d.get(channel).and_then(|c| *c) {
                    let _ = bank.update_se_id_3d_with_occlusion(
                        eng,
                        se_id,
                        player_pos,
                        listener_yaw,
                        listener_pitch,
                        is_maria,
                        ch.position,
                        ch.base_volume,
                        Some(collision),
                    );
                }
            }
        }
    }
}
