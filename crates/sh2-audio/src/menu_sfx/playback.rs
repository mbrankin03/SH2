//! 2D SFX, surface-material footsteps, flashlight clicks, and cut/unused audio preview playback
//! on [`MenuAudioBank`] (`SH2-61`, `SH2-62`).

use kira::sound::static_sound::StaticSoundHandle;
use sh2_assets::formats::sound::slice_wav_time_range;
use sh2_core::SurfaceMaterial;

use super::{synthesize_fallback_cue, MenuAudioBank, SoundCueKind};
use crate::beta_audio_catalog::{
    CutSfxCatalogEntry, LAURA_LETTER_VOICEOVER_SLICES, VOICE_AFS_LAURA_LETTER_INDEX,
};
use crate::sfx_bank::{footstep_sddata_indices, resolve_se_id_to_voice_afs_index};
use crate::{AudioEngine, AudioError};

impl MenuAudioBank {
    /// Plays a sound effect by its 0-based `sddata.bin` index (`0..417`) at 2D `volume`.
    pub fn play_sddata_index(
        &self,
        engine: &mut AudioEngine,
        index: usize,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        if let Some(prepared) = self.get_or_prepare_sddata(index) {
            let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
            Ok(Some(handle))
        } else {
            synthesize_fallback_cue(engine, SoundCueKind::Footstep)
        }
    }

    /// Plays a game sound effect by its `se_id` (`0x5147E0` / `0x5148B0`, e.g., `11048` Handgun, `11051` Shotgun, `11032` Plank, `11040` Pipe/Kick).
    pub fn play_se_id(
        &self,
        engine: &mut AudioEngine,
        se_id: u32,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        if let Some(prepared) = self.resolve_prepared_se_id(se_id) {
            let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
            Ok(Some(handle))
        } else {
            synthesize_fallback_cue(engine, SoundCueKind::Confirm)
        }
    }

    /// Starts a looping 2D ambient sound effect keyed by `se_id` (`SeCallPos` with `flags & 8 != 0`
    /// and `pos = NULL`, e.g. Forest wind `0x2742`, Rain `0x4DB8`, Toluca Lake `0x3CF0`).
    ///
    /// Idempotent: if `se_id` is already playing in `active_looping_2d_handles`, updates its live
    /// volume instead of stacking a duplicate voice. Lazily loads `voice.afs` streams on demand.
    pub fn play_looping_se_id(
        &mut self,
        engine: &mut AudioEngine,
        se_id: u32,
        volume: f32,
    ) -> Result<bool, AudioError> {
        self.active_looping_2d_handles
            .retain(|_, h| h.state() != kira::sound::PlaybackState::Stopped);
        if let Some(handle) = self.active_looping_2d_handles.get_mut(&se_id) {
            engine.set_sfx_2d_volume(handle, volume);
            return Ok(true);
        }
        if let Some(afs_idx) = resolve_se_id_to_voice_afs_index(se_id) {
            if !self.voice_afs_sfx.contains_key(&afs_idx) {
                let _ = self.load_voice_afs_entry(afs_idx);
            }
        }
        let Some(prepared) = self.resolve_prepared_se_id(se_id) else {
            return Ok(false);
        };
        let handle = engine.play_prepared_sfx_2d_looping(&prepared, volume)?;
        self.active_looping_2d_handles.insert(se_id, handle);
        Ok(true)
    }

    /// Smoothly modulates the linear volume of an active looping 2D ambient SE (`SeVolume`
    /// `sh2pc.exe` `0x00516d50`). Returns `true` if `se_id` was active.
    pub fn set_looping_se_volume(&mut self, engine: &AudioEngine, se_id: u32, volume: f32) -> bool {
        if let Some(handle) = self.active_looping_2d_handles.get_mut(&se_id) {
            if handle.state() == kira::sound::PlaybackState::Stopped {
                self.active_looping_2d_handles.remove(&se_id);
                return false;
            }
            engine.set_sfx_2d_volume(handle, volume);
            true
        } else {
            false
        }
    }

    /// Smoothly fades out and stops a single looping 2D ambient SE by `se_id` (`SeStop`
    /// `0x00193a80` / `0x00515f10`). Returns `true` if `se_id` was active.
    pub fn stop_looping_se_id(&mut self, se_id: u32) -> bool {
        if let Some(mut handle) = self.active_looping_2d_handles.remove(&se_id) {
            handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(40),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
            true
        } else {
            false
        }
    }

    /// Smoothly fades out and stops all active looping 2D ambient SE handles (e.g. on stage or
    /// room transition or returning to the Title Screen).
    pub fn stop_all_looping_se(&mut self) {
        for (_, mut handle) in self.active_looping_2d_handles.drain() {
            handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(40),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
    }

    /// Returns `true` if `se_id` is currently playing as a tracked looping 2D ambient SE.
    pub fn is_looping_2d_se_playing(&self, se_id: u32) -> bool {
        self.active_looping_2d_handles
            .get(&se_id)
            .is_some_and(|h| h.state() != kira::sound::PlaybackState::Stopped)
    }

    /// Returns `true` if `se_id` is currently playing in any tracked 2D looping, 2D panned, or 3D
    /// spatial handle (`SeCheck` `sh2pc.exe` `0x00515890`).
    pub fn is_any_se_playing(&self, se_id: u32) -> bool {
        self.is_looping_2d_se_playing(se_id)
            || self.is_2d_se_playing(se_id)
            || self
                .active_spatial_voices
                .get(&se_id)
                .is_some_and(|v| !v.is_finished())
    }

    /// Appends all currently playing tracked `se_id`s (2D looping, 2D panned, and 3D spatial)
    /// into `out` without duplicates (`SeCheck` `sh2pc.exe` `0x00515890`).
    pub fn collect_active_se_ids(&self, out: &mut Vec<u32>) {
        for (&se_id, handle) in &self.active_looping_2d_handles {
            if handle.state() != kira::sound::PlaybackState::Stopped && !out.contains(&se_id) {
                out.push(se_id);
            }
        }
        for (&se_id, handle) in &self.active_spatial_handles {
            if handle.state() != kira::sound::PlaybackState::Stopped && !out.contains(&se_id) {
                out.push(se_id);
            }
        }
        for (&se_id, voice) in &self.active_spatial_voices {
            if !voice.is_finished() && !out.contains(&se_id) {
                out.push(se_id);
            }
        }
    }

    /// Plays a surface-material-specific footstep for James or Maria, alternating left/right step samples (`SH2-61`).
    ///
    /// Also enforces `sh2pc.exe` `0x518270` voice-replacement behavior: if the previous footstep is still
    /// ringing, its tail is faded out (`25 ms`) before the new step triggers so footsteps never stack.
    ///
    /// Returns the `sddata.bin` index that was triggered.
    pub fn play_footstep(
        &mut self,
        engine: &mut AudioEngine,
        is_maria: bool,
        material: SurfaceMaterial,
        is_running: bool,
        volume: f32,
    ) -> Result<(usize, Option<StaticSoundHandle>), AudioError> {
        let (left_idx, right_idx) = footstep_sddata_indices(is_maria, material, is_running);
        let chosen_idx = if self.step_parity {
            right_idx
        } else {
            left_idx
        };
        self.step_parity = !self.step_parity;

        // `0x518270`: Stop previous footstep instance if still playing so tails do not overlap
        if let Some(mut prev_handle) = self.last_footstep_handle.take() {
            prev_handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(25),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }

        if let Some(prepared) = self.get_or_prepare_sddata(chosen_idx).or_else(|| {
            self.footstep
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
        }) {
            let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
            self.last_footstep_handle = Some(handle);
            Ok((chosen_idx, None))
        } else {
            let handle = synthesize_fallback_cue(engine, SoundCueKind::Footstep)?;
            self.last_footstep_handle = handle;
            Ok((chosen_idx, None))
        }
    }

    /// Plays a flashlight toggle click (`FlashlightOn` when `turned_on == true`, `FlashlightOff` when `false`).
    pub fn play_flashlight_toggle(
        &self,
        engine: &mut AudioEngine,
        turned_on: bool,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.play(
            engine,
            if turned_on {
                SoundCueKind::FlashlightOn
            } else {
                SoundCueKind::FlashlightOff
            },
        )
    }

    /// Plays a sound cue through the audio engine using authentic audio.
    pub fn play(
        &self,
        engine: &mut AudioEngine,
        cue: SoundCueKind,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        let entry = match cue {
            SoundCueKind::CursorMove => self.cursor_move.as_ref(),
            SoundCueKind::Confirm => self.confirm.as_ref(),
            SoundCueKind::Cancel => self.cancel.as_ref(),
            SoundCueKind::GameStartTransition => self.game_start.as_ref(),
            SoundCueKind::SavePoint => self.save_sound.as_ref(),
            SoundCueKind::FlashlightOn => self.flashlight_on.as_ref(),
            SoundCueKind::FlashlightOff => self.flashlight_off.as_ref(),
            SoundCueKind::Footstep => self.footstep.as_ref(),
        };

        if let Some(prepared) = entry.and_then(|src| self.prepare_source_ref(src)) {
            let handle = engine.play_prepared_sfx_2d(&prepared, 1.0)?;
            Ok(Some(handle))
        } else {
            // Fallback synthetic generator if asset was unavailable
            synthesize_fallback_cue(engine, cue)
        }
    }

    /// Stops any currently playing cut/beta audio or voiceover slice (`SH2-62`).
    pub fn stop_active_cut_audio(&mut self) {
        if let Some(mut handle) = self.active_cut_audio_handle.take() {
            handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(60),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
    }

    /// Plays one of the 11 timed voiceover slices (`page_index` `0..=10`) of Mary's unused letter
    /// to Laura (`voice.afs[119]` / `SCENE_45` in `PlayUnusedAudio.cpp`) (`SH2-62`).
    pub fn play_laura_letter_slice(
        &mut self,
        engine: &mut AudioEngine,
        page_index: usize,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.stop_active_cut_audio();
        let slice_meta = LAURA_LETTER_VOICEOVER_SLICES
            .get(page_index)
            .copied()
            .unwrap_or(LAURA_LETTER_VOICEOVER_SLICES[0]);

        if let Some(full_wav) = self.load_voice_afs_entry(VOICE_AFS_LAURA_LETTER_INDEX) {
            let sliced = slice_wav_time_range(&full_wav, slice_meta.start_sec, slice_meta.end_sec);
            if !sliced.data.is_empty() {
                let handle = engine.play_sfx_2d(&sliced, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        let handle = synthesize_fallback_cue(engine, SoundCueKind::Confirm)?;
        self.active_cut_audio_handle = handle;
        Ok(None)
    }

    /// Resolves and plays any [`CutSfxCatalogEntry`] (`sddata.bin` unreferenced/3D cue, `voice.afs` stream,
    /// or `sh2e/sound/extra/*.wav` bonus cue), stopping any previously auditioned cut audio first (`SH2-62`).
    pub fn play_cut_catalog_entry(
        &mut self,
        engine: &mut AudioEngine,
        entry: &CutSfxCatalogEntry,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.stop_active_cut_audio();

        if let Some(sd_idx) = entry.sddata_index {
            if let Some(prepared) = self.get_or_prepare_sddata(sd_idx) {
                let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        if let Some(afs_idx) = entry.voice_afs_index {
            // For `SCENE_45` (119), play the opening slice (0..11.7s) when auditioned from the catalog
            if afs_idx == VOICE_AFS_LAURA_LETTER_INDEX {
                if let Some(full_wav) = self.load_voice_afs_entry(afs_idx) {
                    let preview = slice_wav_time_range(&full_wav, 0.0, 11.7);
                    let handle = engine.play_sfx_2d(&preview, volume)?;
                    self.active_cut_audio_handle = Some(handle);
                    return Ok(None);
                }
            } else if let Some(wav) = self.load_voice_afs_entry(afs_idx) {
                let handle = engine.play_sfx_2d(&wav, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        if let Some(rel) = entry.extra_rel_path {
            let wav_opt = if rel.ends_with("sankaku_toujyou_2.wav") {
                self.closet_bonus_audio.clone()
            } else if rel.ends_with("machi.wav") {
                self.machi_steps_audio.clone()
            } else {
                let mut slot = None;
                if let Some(ref dr) = self.data_root {
                    Self::load_extra_wav_override(
                        dr,
                        self.ee_root.as_deref(),
                        true,
                        rel,
                        &mut slot,
                    );
                }
                slot
            };
            if let Some(wav) = wav_opt {
                let handle = engine.play_sfx_2d(&wav, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        let handle = synthesize_fallback_cue(engine, SoundCueKind::Confirm)?;
        self.active_cut_audio_handle = handle;
        Ok(None)
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
    fn looping_2d_se_stays_playing_past_buffer_end_and_stops_cleanly() {
        let mut manager = AudioManager::<MockBackend>::new(AudioManagerSettings {
            backend_settings: MockBackendSettings { sample_rate: 48000 },
            ..Default::default()
        })
        .expect("MockBackend audio manager");

        // 16-frame looping clip (`0x2742` Forest wind stand-in) and second looping clip (`0x4DB8` Rain)
        let loop_wind = StaticSoundData {
            sample_rate: 48000,
            frames: Arc::from(vec![Frame::ZERO; 16]),
            settings: StaticSoundSettings::new().loop_region(..),
            slice: None,
        };
        let loop_rain = StaticSoundData {
            sample_rate: 48000,
            frames: Arc::from(vec![Frame::ZERO; 16]),
            settings: StaticSoundSettings::new().loop_region(..),
            slice: None,
        };

        let wind_handle = manager.play(loop_wind).expect("play looping wind");
        let rain_handle = manager.play(loop_rain).expect("play looping rain");

        let mut bank = empty_bank();
        bank.active_looping_2d_handles.insert(0x2742, wind_handle);
        bank.active_looping_2d_handles.insert(0x4DB8, rain_handle);

        // Advance well past the 16-frame clip length: looping handles MUST stay Playing.
        let backend = manager.backend_mut();
        backend.on_start_processing();
        for _ in 0..256 {
            let _ = backend.process();
        }
        assert!(bank.is_looping_2d_se_playing(0x2742));
        assert!(bank.is_looping_2d_se_playing(0x4DB8));
        assert!(bank.is_any_se_playing(0x2742));

        // Stop single looping SE (`0x2742`) while keeping `0x4DB8` active.
        assert!(bank.stop_looping_se_id(0x2742));
        assert!(!bank.stop_looping_se_id(0x2742));
        assert!(!bank.is_looping_2d_se_playing(0x2742));
        assert!(bank.is_looping_2d_se_playing(0x4DB8));

        // Stop all remaining looping SEs cleanly (e.g. stage transition).
        bank.stop_all_looping_se();
        assert!(!bank.is_looping_2d_se_playing(0x4DB8));
        assert!(bank.active_looping_2d_handles.is_empty());
    }
}
