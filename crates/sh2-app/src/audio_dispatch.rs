//! Shared [`SePoolManager`] (`Se2dManager` / `Se3dManager`), Pocket Radio 4-track (`shSdRadio`
//! `0x00291420`), and Chainsaw (`ChainsawSoundController`) backend audio dispatchers for `sh2-app`.

use crate::state::AppState;
use sh2_audio::{
    se_call_pos_direction, se_call_pos_distance, slice_chainsaw_wav, ChainsawAudioOutput,
    SeBackendAction, SE_CHAINSAW_ATTACK, SE_CHAINSAW_HIT, SE_CHAINSAW_IDLE_LOOP,
    SE_CHAINSAW_IDLE_START, SE_DIR_PAN_SCALE_DEG,
};

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
                direction_deg,
                attenuation_byte,
            } => {
                let vol = ((255.0 - f32::from(attenuation_byte)) / 255.0).clamp(0.0, 1.0);
                let pan = (-(direction_deg as f32) / SE_DIR_PAN_SCALE_DEG).clamp(-1.0, 1.0);
                let _ = bank.play_se_id_panned_tracked(eng, se_id, vol, pan);
            }
            SeBackendAction::Update2d {
                se_id,
                direction_deg,
                attenuation_byte,
            } => {
                if let Some(handle) = bank.active_spatial_handles.get_mut(&se_id) {
                    let vol = ((255.0 - f32::from(attenuation_byte)) / 255.0).clamp(0.0, 1.0);
                    let pan = (-(direction_deg as f32) / SE_DIR_PAN_SCALE_DEG).clamp(-1.0, 1.0);
                    eng.update_sound_spatial(handle, vol, pan);
                }
            }
            SeBackendAction::Stop2d { se_id } | SeBackendAction::Stop3d { se_id, .. } => {
                bank.stop_spatial_se(se_id);
            }
            SeBackendAction::Play3d {
                channel,
                se_id,
                direction_deg,
                attenuation_byte,
            } => {
                if let Some(ch) = state.se_pool.channels_3d.get(channel).and_then(|c| *c) {
                    let _ = bank.play_se_id_3d_tracked_with_precomputed_spatial(
                        eng,
                        se_id,
                        direction_deg,
                        attenuation_byte,
                        player_pos,
                        listener_yaw,
                        listener_pitch,
                        is_maria,
                        ch.position,
                        Some(collision),
                    );
                }
            }
            SeBackendAction::Update3d {
                channel,
                se_id,
                direction_deg,
                attenuation_byte,
            } => {
                if let Some(ch) = state.se_pool.channels_3d.get(channel).and_then(|c| *c) {
                    bank.update_se_id_3d_with_precomputed_spatial(
                        eng,
                        se_id,
                        direction_deg,
                        attenuation_byte,
                        player_pos,
                        listener_yaw,
                        listener_pitch,
                        is_maria,
                        ch.position,
                        Some(collision),
                    );
                }
            }
        }
    }
}

/// Dispatches the live 4-track Pocket Radio static state (`RadioNoise` `0x001ab300` -> `shSdRadio`
/// `0x00291420` / `FUN_005186d0` `0x005186d0`) to [`AppState::audio_engine`] (`SH2-167`).
pub(crate) fn dispatch_pocket_radio(state: &mut AppState) {
    let output = state.combat_arena.ai.radio_output;
    let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_ref()) else {
        return;
    };

    if output.is_active {
        if !eng.is_radio_playing() {
            if let Some(tracks) = bank.radio_static_wavs() {
                let _ = eng.start_radio_tracks(tracks);
            }
        }
        eng.update_radio_tracks(output.driver_volumes, output.driver_pans);
    } else if eng.is_radio_playing() {
        eng.stop_radio_tracks();
    }
}

/// Dispatches [`ChainsawAudioOutput`] (`0x2B24` / `0x1642` / `0x2B25` / `0x2B26`) to
/// [`AppState::audio_engine`] using [`slice_chainsaw_wav`] and PS2 `SeCallPosDirection` /
/// `SeCallPosDistance` spatialization (`SH2-167`).
pub(crate) fn dispatch_chainsaw_audio(
    state: &mut AppState,
    output: ChainsawAudioOutput,
    enable_chainsaw_sound_fix: bool,
) {
    let camera_pos = state.sim_core.active_camera_eye(1.0);
    let player_pos = state.sim_core.scenario.player_position;
    let wvs_matrix = state.sim_core.active_ps2_wvs_matrix(1.0);

    let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_mut()) else {
        return;
    };

    let dir_deg = se_call_pos_direction(wvs_matrix, output.world_pos_mm);
    let stereo_pan = (-(dir_deg as f32) / SE_DIR_PAN_SCALE_DEG).clamp(-1.0, 1.0);
    let spatial_vol = |base_vol: f32| -> f32 {
        let atten = se_call_pos_distance(base_vol, camera_pos, player_pos, output.world_pos_mm);
        ((255.0 - f32::from(atten)) / 255.0).clamp(0.0, 1.0)
    };

    // 1. `SE_CHAINSAW_IDLE_START` (`0x2B24`)
    if output.idle_start_volume > 0.001 {
        let vol = spatial_vol(output.idle_start_volume);
        if bank.is_2d_se_playing(SE_CHAINSAW_IDLE_START) {
            if let Some(handle) = bank.active_spatial_handles.get_mut(&SE_CHAINSAW_IDLE_START) {
                eng.update_sound_spatial(handle, vol, stereo_pan);
            }
        } else {
            let _ = bank.play_se_id_panned_tracked(eng, SE_CHAINSAW_IDLE_START, vol, stereo_pan);
        }
    } else if bank
        .active_spatial_handles
        .contains_key(&SE_CHAINSAW_IDLE_START)
    {
        bank.stop_spatial_se(SE_CHAINSAW_IDLE_START);
    }

    // 2. `SE_CHAINSAW_IDLE_LOOP` (`0x1642`, sliced via `compute_chainsaw_wav_slice`)
    if output.idle_loop_volume > 0.001 {
        let vol = spatial_vol(output.idle_loop_volume);
        if bank.is_2d_se_playing(SE_CHAINSAW_IDLE_LOOP) {
            if let Some(handle) = bank.active_spatial_handles.get_mut(&SE_CHAINSAW_IDLE_LOOP) {
                eng.update_sound_spatial(handle, vol, stereo_pan);
            }
        } else if let Some(raw_wav) = bank.resolve_se_id(SE_CHAINSAW_IDLE_LOOP).cloned() {
            let mut sliced = slice_chainsaw_wav(
                &raw_wav,
                SE_CHAINSAW_IDLE_LOOP,
                false,
                enable_chainsaw_sound_fix,
            );
            sliced.loops = true;
            let _ =
                bank.play_wav_panned_tracked(eng, SE_CHAINSAW_IDLE_LOOP, &sliced, vol, stereo_pan);
        }
    } else if bank
        .active_spatial_handles
        .contains_key(&SE_CHAINSAW_IDLE_LOOP)
    {
        bank.stop_spatial_se(SE_CHAINSAW_IDLE_LOOP);
    }

    // 3. `SE_CHAINSAW_ATTACK` (`0x2B25`) directional one-shot
    if let Some(atk_vol) = output.trigger_attack_sfx_volume {
        let vol = spatial_vol(atk_vol);
        let _ = bank.play_se_id_panned_tracked(eng, SE_CHAINSAW_ATTACK, vol, stereo_pan);
    }

    // 4. `SE_CHAINSAW_HIT` (`0x2B26`) impact one-shot
    if let Some(hit_vol) = output.trigger_hit_sfx_volume {
        let vol = spatial_vol(hit_vol);
        let _ = bank.play_se_id_panned_tracked(eng, SE_CHAINSAW_HIT, vol, stereo_pan);
    }
}
