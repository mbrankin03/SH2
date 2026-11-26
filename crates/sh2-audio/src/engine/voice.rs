//! `.mes`-synchronized dialogue and `voice.afs` cutscene voice streams on [`AudioEngine`].
//!
//! Cutscene BGM ducking (`0.7` for non-exempt `DramaDemo` cutscenes, `0.5` for pause/menus,
//! ramped at `±dt * 0.5`) is owned exclusively by [`crate::BgmManager`] (`SeBgmManager`
//! `SLUS_202.28` `0x00194300` / `sh2pc.exe` `0x00516430`); voice playback does not apply a
//! secondary BGM ducking multiplier (`SH2-167`).

use std::sync::Arc;
use std::time::Duration;

use kira::{
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
    tween::{Easing, Tween},
    Volume,
};
use sh2_assets::{DecodedAdx, DialogueMessage, WavEntry};

use super::{adx_to_frames_48k, wav_entry_to_frames_48k, ActiveDialogue, AudioEngine, AudioError};

impl AudioEngine {
    /// Triggers a dialogue voice clip synchronized with `.mes` subtitles.
    pub fn play_dialogue(
        &mut self,
        voice_adx: &DecodedAdx,
        message: DialogueMessage,
        voice_id: Option<u32>,
    ) -> Result<(), AudioError> {
        let voice_track = self
            .voice_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No voice track".into()))?;
        let (play_rate, frames) = adx_to_frames_48k(voice_adx);
        let total_samples = voice_adx.frames.len();
        let duration_secs = total_samples as f64 / voice_adx.header.sample_rate as f64;
        let total_duration = Duration::from_secs_f64(duration_secs);

        let settings = StaticSoundSettings::new()
            .output_destination(voice_track)
            .volume(Volume::Amplitude(
                (self.voice_volume * self.master_volume) as f64,
            ));

        let sound_data = StaticSoundData {
            sample_rate: play_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let handle = manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
        self.active_voice_handle = Some(handle);

        self.active_dialogue = Some(ActiveDialogue {
            message,
            voice_id,
            total_duration,
            elapsed: Duration::ZERO,
            current_page_index: 0,
            is_finished: false,
        });

        Ok(())
    }

    /// Plays a decoded cutscene voice stream (`WavEntry` from `voice.afs` entry `stream_no - 60001`)
    /// on the dedicated voice track (`SH2-63`, `SH2-167`).
    ///
    /// BGM ducking during cutscenes is handled solely by [`crate::BgmManager`] (`0.7` `DramaDemo`).
    pub fn play_cutscene_voice(&mut self, wav: &WavEntry) -> Result<(), AudioError> {
        self.stop_voice(Duration::from_millis(20));

        let voice_track = self
            .voice_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No voice track".into()))?;
        let (play_rate, frames) = wav_entry_to_frames_48k(wav);

        let settings = StaticSoundSettings::new()
            .output_destination(voice_track)
            .volume(Volume::Amplitude(
                (self.voice_volume * self.master_volume) as f64,
            ));

        let sound_data = StaticSoundData {
            sample_rate: play_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let handle = manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
        self.active_voice_handle = Some(handle);
        Ok(())
    }

    /// Stops any currently playing voice stream (`SeCall(0x3F4)` on cutscene skip).
    pub fn stop_voice(&mut self, fade_out: Duration) {
        if let Some(mut handle) = self.active_voice_handle.take() {
            handle.stop(Tween {
                duration: fade_out,
                easing: Easing::Linear,
                ..Default::default()
            });
        }
        self.active_dialogue = None;
    }

    /// Returns whether a voice stream is currently active on the voice track.
    pub fn is_voice_playing(&self) -> bool {
        self.active_voice_handle
            .as_ref()
            .is_some_and(|h| h.state() == kira::sound::PlaybackState::Playing)
    }

    /// Returns the current playback position in seconds of the active cutscene voice stream, if playing (`SH2-63`).
    pub fn voice_playback_position_secs(&self) -> Option<f32> {
        self.active_voice_handle
            .as_ref()
            .filter(|h| h.state() == kira::sound::PlaybackState::Playing)
            .map(|h| h.position() as f32)
    }

    /// Per-frame update step: advances active dialogue subtitle triggers.
    pub fn update(&mut self, dt: Duration) {
        if let Some(ref mut dialogue) = self.active_dialogue {
            dialogue.update(dt);
        }

        if let Some(ref dialogue) = self.active_dialogue {
            if dialogue.is_finished {
                self.active_dialogue = None;
            }
        }
    }

    /// Returns the currently active dialogue and subtitle text, if playing.
    pub fn active_dialogue(&self) -> Option<&ActiveDialogue> {
        self.active_dialogue.as_ref()
    }
}
