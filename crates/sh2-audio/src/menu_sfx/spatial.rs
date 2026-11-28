//! 2D panned and 3D `.cld`-occluded / Binaural HRTF tracked voice playback and updates
//! on [`MenuAudioBank`] (`SH2-47`, `SH2-61`, `SH2-84`, `SH2-167`).

use glam::Vec3;
use sh2_assets::formats::sound::WavEntry;
use sh2_core::collision::CollisionWorld;
use sh2_core::listener_head_position;

use super::{synthesize_fallback_cue, MenuAudioBank, SoundCueKind};
use crate::hrtf::compute_listener_spherical_angles;
use crate::occlusion::{evaluate_world_occlusion, OcclusionDspParams};
use crate::sfx_bank::{
    compute_se_call_pos_distance_f, resolve_se_id_to_voice_afs_index, SfxSpatialEval,
};
use crate::spatial_voice::{SpatialVoiceHandle, SpatialVoiceInitParams};
use crate::{AudioEngine, AudioError};

/// Evaluates `.cld` world occlusion only when `occlusion_enabled` is `true`, skipping per-frame
/// raycasts in strict 1:1 Vanilla PS2 mode (`SH2-179`).
#[inline]
pub(crate) fn evaluate_world_occlusion_gated(
    collision: Option<&CollisionWorld>,
    source_pos: Vec3,
    listener_pos: Vec3,
    is_maria: bool,
    occlusion_enabled: bool,
) -> OcclusionDspParams {
    if occlusion_enabled {
        if let Some(world) = collision {
            return evaluate_world_occlusion(world, source_pos, listener_pos, is_maria);
        }
    }
    OcclusionDspParams::unoccluded()
}

impl MenuAudioBank {
    /// Prunes completed (`PlaybackState::Stopped`) 2D/panned handles and finished 3D spatial voices
    /// so one-shot handles never accumulate across rooms (`SH2-176`).
    pub fn prune_stopped_spatial_handles(&mut self) {
        self.active_spatial_handles
            .retain(|_, h| h.state() != kira::sound::PlaybackState::Stopped);
        self.active_spatial_voices.retain(|_, v| !v.is_finished());
    }

    /// Returns `true` if a tracked 2D/panned handle for `se_id` is still playing (`!Stopped`).
    pub fn is_2d_se_playing(&self, se_id: u32) -> bool {
        self.active_spatial_handles
            .get(&se_id)
            .is_some_and(|h| h.state() != kira::sound::PlaybackState::Stopped)
    }

    /// Plays a 2D/panned sound effect by `se_id` with explicit linear `volume` (`0.0..=1.0`) and
    /// `stereo_pan` (`-1.0..=1.0`), storing the handle in `active_spatial_handles` so `Update2d`
    /// (`SeCallPosChange` `0x001926e0`) can update its pan and volume every frame (`SH2-167`).
    pub fn play_se_id_panned_tracked(
        &mut self,
        engine: &mut AudioEngine,
        se_id: u32,
        volume: f32,
        stereo_pan: f32,
    ) -> Result<bool, AudioError> {
        self.prune_stopped_spatial_handles();
        if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
            if !self.voice_afs_sfx.contains_key(&afs_idx) {
                let _ = self.load_voice_afs_entry(afs_idx);
            }
        }
        if let Some(mut prev) = self.active_spatial_handles.remove(&se_id) {
            prev.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(25),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
        let Some(prepared) = self.resolve_prepared_se_id(se_id) else {
            return Ok(false);
        };
        let handle = engine.play_prepared_sfx_panned(&prepared, volume, stereo_pan)?;
        self.active_spatial_handles.insert(se_id, handle);
        Ok(true)
    }

    /// Plays a custom/sliced `WavEntry` under `se_id` with explicit `volume` and `stereo_pan`,
    /// storing the handle in `active_spatial_handles` (used by `ChainsawSoundController`, `SH2-167`).
    pub fn play_wav_panned_tracked(
        &mut self,
        engine: &mut AudioEngine,
        se_id: u32,
        wav: &WavEntry,
        volume: f32,
        stereo_pan: f32,
    ) -> Result<(), AudioError> {
        self.prune_stopped_spatial_handles();
        if let Some(mut prev) = self.active_spatial_handles.remove(&se_id) {
            prev.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(25),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
        let handle = engine.play_sfx_panned(wav, volume, stereo_pan)?;
        self.active_spatial_handles.insert(se_id, handle);
        Ok(())
    }

    /// Plays and tracks a 3D positional sound effect by `se_id` using precomputed `SePoolManager`
    /// `(direction_deg, attenuation_byte)` (`SeCallPosDistanceF` `0x00192bc0` & `SeCallPosDirection`
    /// `0x00192990`), preserving full dual-distance `(camera_pos, player_pos)` attenuation while
    /// still evaluating `.cld` occlusion and Binaural HRTF spherical angles (`SH2-167`).
    #[allow(clippy::too_many_arguments)]
    pub fn play_se_id_3d_tracked_with_precomputed_spatial(
        &mut self,
        engine: &mut AudioEngine,
        se_id: u32,
        direction_deg: i32,
        attenuation_byte: u8,
        listener_pos: Vec3,
        listener_yaw: f32,
        listener_pitch: f32,
        is_maria: bool,
        source_pos: Vec3,
        collision: Option<&CollisionWorld>,
    ) -> Result<(), AudioError> {
        self.prune_stopped_spatial_handles();
        if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
            if !self.voice_afs_sfx.contains_key(&afs_idx) {
                let _ = self.load_voice_afs_entry(afs_idx);
            }
        }
        if let Some(mut prev) = self.active_spatial_handles.remove(&se_id) {
            prev.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(25),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
        if let Some(prev_voice) = self.active_spatial_voices.remove(&se_id) {
            prev_voice.stop();
        }

        let base_distance_gain = ((255.0 - f32::from(attenuation_byte)) / 255.0).clamp(0.0, 1.0);
        if base_distance_gain <= 0.001 {
            return Ok(());
        }
        let stereo_pan =
            (-(direction_deg as f32) / crate::sfx_bank::SE_DIR_PAN_SCALE_DEG).clamp(-1.0, 1.0);
        let occlusion = evaluate_world_occlusion_gated(
            collision,
            source_pos,
            listener_pos,
            is_maria,
            engine.acoustic_occlusion_enabled(),
        );
        let head_pos = listener_head_position(listener_pos, is_maria);
        let angles =
            compute_listener_spherical_angles(head_pos, listener_yaw, listener_pitch, source_pos);
        let init = SpatialVoiceInitParams {
            base_distance_gain,
            stereo_pan,
            angles,
            occlusion,
        };

        if let Some(prepared) = self.resolve_prepared_se_id(se_id) {
            let handle = engine.play_prepared_sfx_spatial_3d(&prepared, init)?;
            self.active_spatial_voices.insert(se_id, handle);
        }
        Ok(())
    }

    /// Updates a tracked 3D positional sound effect by `se_id` using precomputed `SePoolManager`
    /// `(direction_deg, attenuation_byte)` without recomputing `d_cam = 0` (`SH2-167`).
    #[allow(clippy::too_many_arguments)]
    pub fn update_se_id_3d_with_precomputed_spatial(
        &mut self,
        engine: &AudioEngine,
        se_id: u32,
        direction_deg: i32,
        attenuation_byte: u8,
        listener_pos: Vec3,
        listener_yaw: f32,
        listener_pitch: f32,
        is_maria: bool,
        source_pos: Vec3,
        collision: Option<&CollisionWorld>,
    ) {
        let base_distance_gain = ((255.0 - f32::from(attenuation_byte)) / 255.0).clamp(0.0, 1.0);
        let stereo_pan =
            (-(direction_deg as f32) / crate::sfx_bank::SE_DIR_PAN_SCALE_DEG).clamp(-1.0, 1.0);
        let occlusion = evaluate_world_occlusion_gated(
            collision,
            source_pos,
            listener_pos,
            is_maria,
            engine.acoustic_occlusion_enabled(),
        );
        self.prune_stopped_spatial_handles();
        if let Some(handle) = self.active_spatial_handles.get_mut(&se_id) {
            engine.update_sound_spatial(
                handle,
                base_distance_gain * occlusion.dry_gain_scale,
                stereo_pan,
            );
        }
        if let Some(voice) = self.active_spatial_voices.get(&se_id) {
            let head_pos = listener_head_position(listener_pos, is_maria);
            let angles = compute_listener_spherical_angles(
                head_pos,
                listener_yaw,
                listener_pitch,
                source_pos,
            );
            engine.update_spatial_voice(
                voice,
                SpatialVoiceInitParams {
                    base_distance_gain,
                    stereo_pan,
                    angles,
                    occlusion,
                },
            );
        }
    }

    /// Plays a 3D positional sound effect by its `se_id` using `SeCallPosDistanceF` (`0x5155E0`) distance attenuation
    /// and the active spatialization mode (`ClassicPs2Stereo` vs `BinauralHrtf`) (`SH2-61` & `SH2-84`).
    pub fn play_se_id_3d(
        &self,
        engine: &mut AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        source_pos: Vec3,
        base_volume: f32,
    ) -> Result<(SfxSpatialEval, Option<SpatialVoiceHandle>), AudioError> {
        let (eval, _occ, voice_opt) = self.play_se_id_3d_with_occlusion(
            engine,
            se_id,
            listener_pos,
            listener_yaw,
            0.0,
            false,
            source_pos,
            base_volume,
            None,
        )?;
        Ok((eval, voice_opt))
    }

    /// Plays a 3D positional sound effect by `se_id` with real-time `.cld` raycast wall/door occlusion
    /// and `ClassicPs2Stereo` / `BinauralHrtf` 3D spatialization (`SH2-84`).
    #[allow(clippy::too_many_arguments)]
    pub fn play_se_id_3d_with_occlusion(
        &self,
        engine: &mut AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        listener_pitch: f32,
        is_maria: bool,
        source_pos: Vec3,
        base_volume: f32,
        collision: Option<&CollisionWorld>,
    ) -> Result<
        (
            SfxSpatialEval,
            OcclusionDspParams,
            Option<SpatialVoiceHandle>,
        ),
        AudioError,
    > {
        let eval =
            compute_se_call_pos_distance_f(listener_pos, listener_yaw, source_pos, base_volume);
        let head_pos = listener_head_position(listener_pos, is_maria);
        let occlusion = evaluate_world_occlusion_gated(
            collision,
            source_pos,
            listener_pos,
            is_maria,
            engine.acoustic_occlusion_enabled(),
        );
        if eval.effective_gain <= 0.001 {
            return Ok((eval, occlusion, None));
        }

        let angles =
            compute_listener_spherical_angles(head_pos, listener_yaw, listener_pitch, source_pos);
        let init = SpatialVoiceInitParams {
            base_distance_gain: eval.effective_gain,
            stereo_pan: eval.stereo_pan,
            angles,
            occlusion,
        };

        if let Some(prepared) = self.resolve_prepared_se_id(se_id) {
            let handle = engine.play_prepared_sfx_spatial_3d(&prepared, init)?;
            Ok((eval, occlusion, Some(handle)))
        } else {
            let _ = synthesize_fallback_cue(engine, SoundCueKind::Footstep)?;
            Ok((eval, occlusion, None))
        }
    }

    /// Plays a 3D positional sound effect by `se_id` (lazily loading `voice.afs` streams if needed),
    /// replacing any previous instance of the same `se_id` and storing its handle so `SePosMove` / `SePosSet`
    /// can update its stereo pan / HRTF angles and `.cld` occlusion every frame (`SH2-47` & `SH2-84`).
    pub fn play_se_id_3d_tracked(
        &mut self,
        engine: &mut AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        source_pos: Vec3,
        base_volume: f32,
    ) -> Result<SfxSpatialEval, AudioError> {
        let (eval, _occ) = self.play_se_id_3d_tracked_with_occlusion(
            engine,
            se_id,
            listener_pos,
            listener_yaw,
            0.0,
            false,
            source_pos,
            base_volume,
            None,
        )?;
        Ok(eval)
    }

    /// Plays and tracks a 3D positional sound effect by `se_id` with live `.cld` raycast wall/door occlusion
    /// and Binaural HRTF / Classic PS2 Stereo spatialization (`SH2-84`).
    #[allow(clippy::too_many_arguments)]
    pub fn play_se_id_3d_tracked_with_occlusion(
        &mut self,
        engine: &mut AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        listener_pitch: f32,
        is_maria: bool,
        source_pos: Vec3,
        base_volume: f32,
        collision: Option<&CollisionWorld>,
    ) -> Result<(SfxSpatialEval, OcclusionDspParams), AudioError> {
        self.prune_stopped_spatial_handles();
        // Lazily populate `voice_afs_sfx` if `se_id` maps to a `voice.afs` index not yet cached
        if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
            if !self.voice_afs_sfx.contains_key(&afs_idx) {
                let _ = self.load_voice_afs_entry(afs_idx);
            }
        }

        // Stop previous handle for this `se_id` so re-triggers don't stack
        if let Some(mut prev) = self.active_spatial_handles.remove(&se_id) {
            prev.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(25),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
        if let Some(prev_voice) = self.active_spatial_voices.remove(&se_id) {
            prev_voice.stop();
        }

        let (eval, occ, voice_opt) = self.play_se_id_3d_with_occlusion(
            engine,
            se_id,
            listener_pos,
            listener_yaw,
            listener_pitch,
            is_maria,
            source_pos,
            base_volume,
            collision,
        )?;
        if let Some(voice) = voice_opt {
            self.active_spatial_voices.insert(se_id, voice);
        }
        Ok((eval, occ))
    }

    /// Updates the live stereo panning / HRTF angles and distance-attenuated volume of a tracked `se_id` handle (`SePosMove` / `SePosSet`).
    pub fn update_se_id_3d(
        &mut self,
        engine: &AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        source_pos: Vec3,
        base_volume: f32,
    ) -> SfxSpatialEval {
        let (eval, _occ) = self.update_se_id_3d_with_occlusion(
            engine,
            se_id,
            listener_pos,
            listener_yaw,
            0.0,
            false,
            source_pos,
            base_volume,
            None,
        );
        eval
    }

    /// Updates a tracked `se_id` voice with fresh 3D `.cld` wall/door raycast occlusion and HRTF spherical angles (`SH2-84`).
    #[allow(clippy::too_many_arguments)]
    pub fn update_se_id_3d_with_occlusion(
        &mut self,
        engine: &AudioEngine,
        se_id: u32,
        listener_pos: Vec3,
        listener_yaw: f32,
        listener_pitch: f32,
        is_maria: bool,
        source_pos: Vec3,
        base_volume: f32,
        collision: Option<&CollisionWorld>,
    ) -> (SfxSpatialEval, OcclusionDspParams) {
        let eval =
            compute_se_call_pos_distance_f(listener_pos, listener_yaw, source_pos, base_volume);
        let occlusion = evaluate_world_occlusion_gated(
            collision,
            source_pos,
            listener_pos,
            is_maria,
            engine.acoustic_occlusion_enabled(),
        );
        self.prune_stopped_spatial_handles();
        if let Some(handle) = self.active_spatial_handles.get_mut(&se_id) {
            engine.update_sound_spatial(
                handle,
                eval.effective_gain * occlusion.dry_gain_scale,
                eval.stereo_pan,
            );
        }
        if let Some(voice) = self.active_spatial_voices.get(&se_id) {
            let head_pos = listener_head_position(listener_pos, is_maria);
            let angles = compute_listener_spherical_angles(
                head_pos,
                listener_yaw,
                listener_pitch,
                source_pos,
            );
            engine.update_spatial_voice(
                voice,
                SpatialVoiceInitParams {
                    base_distance_gain: eval.effective_gain,
                    stereo_pan: eval.stereo_pan,
                    angles,
                    occlusion,
                },
            );
        }
        (eval, occlusion)
    }

    /// Pauses or resumes all tracked 3D spatial sound handles (e.g. when opening/closing the Pause Menu) (`SH2-84`).
    pub fn set_spatial_handles_paused(&mut self, paused: bool) {
        let tween = kira::tween::Tween {
            duration: std::time::Duration::from_millis(30),
            easing: kira::tween::Easing::Linear,
            ..Default::default()
        };
        for handle in self.active_looping_2d_handles.values_mut() {
            if paused {
                handle.pause(tween);
            } else {
                handle.resume(tween);
            }
        }
        for handle in self.active_spatial_handles.values_mut() {
            if paused {
                handle.pause(tween);
            } else {
                handle.resume(tween);
            }
        }
        for voice in self.active_spatial_voices.values() {
            voice.set_paused(paused);
        }
    }

    /// Stops a single tracked 3D spatial sound handle or voice by `se_id` (`SeStop` `0x00193a80`).
    pub fn stop_spatial_se(&mut self, se_id: u32) {
        let tween = kira::tween::Tween {
            duration: std::time::Duration::from_millis(40),
            easing: kira::tween::Easing::Linear,
            ..Default::default()
        };
        if let Some(mut handle) = self.active_spatial_handles.remove(&se_id) {
            handle.stop(tween);
        }
        if let Some(voice) = self.active_spatial_voices.remove(&se_id) {
            voice.stop();
        }
    }

    /// Stops all tracked 3D spatial sound handles (`SH2-47` & `SH2-84`).
    pub fn stop_all_spatial_handles(&mut self) {
        for (_, mut handle) in self.active_spatial_handles.drain() {
            handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(40),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
        for (_, voice) in self.active_spatial_voices.drain() {
            voice.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kira::manager::backend::mock::{MockBackend, MockBackendSettings};
    use kira::manager::{AudioManager, AudioManagerSettings};
    use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
    use kira::Frame;
    use sh2_core::AudioPackPreference;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Arc;

    fn empty_bank() -> MenuAudioBank {
        MenuAudioBank {
            loaded_source: AudioPackPreference::Auto,
            sddata_path: None,
            voice_afs_path: None,
            voice_afs_index: None,
            data_root: None,
            ee_root: None,
            entries: Vec::new(),
            prepared_sddata: Vec::new(),
            voice_afs_sfx: HashMap::new(),
            voice_afs_lru: VecDeque::new(),
            cursor_move: None,
            confirm: None,
            cancel: None,
            game_start: None,
            save_sound: None,
            flashlight_on: None,
            flashlight_off: None,
            closet_bonus_audio: None,
            machi_steps_audio: None,
            footstep: None,
            step_parity: false,
            last_footstep_handle: None,
            active_cut_audio_handle: None,
            active_looping_2d_handles: HashMap::new(),
            active_spatial_handles: HashMap::new(),
            active_spatial_voices: HashMap::new(),
        }
    }

    #[test]
    fn prune_stopped_spatial_handles_removes_finished_and_keeps_playing() {
        let mut manager = AudioManager::<MockBackend>::new(AudioManagerSettings {
            backend_settings: MockBackendSettings { sample_rate: 48000 },
            ..Default::default()
        })
        .expect("MockBackend audio manager");

        // Short 4-frame one-shot and long 48000-frame sound
        let short_sound = StaticSoundData {
            sample_rate: 48000,
            frames: Arc::from(vec![Frame::ZERO; 4]),
            settings: StaticSoundSettings::new(),
            slice: None,
        };
        let long_sound = StaticSoundData {
            sample_rate: 48000,
            frames: Arc::from(vec![Frame::ZERO; 48000]),
            settings: StaticSoundSettings::new(),
            slice: None,
        };

        let short_handle = manager.play(short_sound).expect("play short");
        let long_handle = manager.play(long_sound).expect("play long");

        let mut bank = empty_bank();
        bank.active_spatial_handles.insert(1001, short_handle);
        bank.active_spatial_handles.insert(1002, long_handle);
        assert_eq!(bank.active_spatial_handles.len(), 2);

        // Advance MockBackend by 64 samples so the 4-frame one-shot finishes (`PlaybackState::Stopped`)
        let backend = manager.backend_mut();
        backend.on_start_processing();
        for _ in 0..64 {
            let _ = backend.process();
        }

        bank.prune_stopped_spatial_handles();
        assert!(!bank.active_spatial_handles.contains_key(&1001));
        assert!(bank.active_spatial_handles.contains_key(&1002));
        assert_eq!(bank.active_spatial_handles.len(), 1);
    }

    #[test]
    fn evaluate_world_occlusion_gated_skips_raycast_in_vanilla_ps2_mode() {
        let world = CollisionWorld::create_synthetic_room(6000.0, 6000.0, 2000.0, 600.0, 600.0);
        let listener = Vec3::new(0.0, 0.0, -1500.0);
        let source = Vec3::new(0.0, -400.0, 1500.0);

        // Vanilla PS2 mode (`occlusion_enabled = false`) skips raycasting and returns unoccluded params.
        let vanilla = evaluate_world_occlusion_gated(Some(&world), source, listener, false, false);
        assert_eq!(vanilla, OcclusionDspParams::unoccluded());

        // Enhanced occlusion mode (`occlusion_enabled = true`) raycasts the `.cld` pillar and attenuates.
        let enhanced = evaluate_world_occlusion_gated(Some(&world), source, listener, false, true);
        assert!(enhanced.occluded);
        assert!(enhanced.dry_cutoff_hz < OcclusionDspParams::unoccluded().dry_cutoff_hz);
        assert!(enhanced.dry_gain_scale < 1.0);
    }
}
