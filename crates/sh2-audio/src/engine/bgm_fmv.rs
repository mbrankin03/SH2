//! Single-stream ADX, multi-layer AIX interactive BGM, and `.bik` FMV PCM playback methods on [`AudioEngine`].

use std::sync::Arc;
use std::time::Duration;

use kira::{
    sound::{
        static_sound::{StaticSoundData, StaticSoundSettings},
        EndPosition, PlaybackPosition, Region,
    },
    tween::{Easing, Tween},
    Volume,
};
use sh2_assets::{DecodedAdx, WavEntry};

use super::{adx_to_frames_48k, wav_entry_to_frames, AudioEngine, AudioError, LoopRegionSamples};

impl AudioEngine {
    /// Streams a background music or ambient track from a decoded ADX stream.
    pub fn play_bgm(&mut self, adx: &DecodedAdx, volume: f32) -> Result<(), AudioError> {
        self.play_bgm_with_fade(adx, volume, Duration::from_millis(250))
    }

    /// Plays decoded ADX audio stream with explicit master volume scaling and optional fade-in.
    pub fn play_bgm_with_fade(
        &mut self,
        adx: &DecodedAdx,
        master_volume_factor: f32,
        fade_in: Duration,
    ) -> Result<(), AudioError> {
        self.stop_bgm(Duration::from_millis(50));

        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let bgm_track = self
            .bgm_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No bgm track".into()))?;

        let (sample_rate, frames) = adx_to_frames_48k(adx);
        self.output_ringbuffer
            .push_frames(&frames[..frames.len().min(1024)]);

        let target_vol = (master_volume_factor * self.bgm_volume * self.master_volume) as f64;
        let mut settings = StaticSoundSettings::new().output_destination(bgm_track);

        if fade_in.is_zero() {
            settings = settings.volume(Volume::Amplitude(target_vol));
        } else {
            settings = settings.volume(Volume::Amplitude(0.0));
        }

        if adx.header.has_loop {
            if let Some(region) = LoopRegionSamples::from_source_samples(
                adx.header.loop_start_sample,
                adx.header.loop_end_sample,
                adx.header.sample_rate,
                sample_rate,
                frames.len(),
            ) {
                settings = settings.loop_region(region.to_kira_region());
            }
        }

        let sound_data = StaticSoundData {
            sample_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        let mut handle = manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))?;

        if !fade_in.is_zero() {
            handle.set_volume(
                Volume::Amplitude(target_vol),
                Tween {
                    duration: fade_in,
                    easing: Easing::Linear,
                    ..Default::default()
                },
            );
        }

        self.active_bgm_handle = Some(handle);
        Ok(())
    }

    /// Starts synchronized playback of a multi-layer AIX stream (or single-layer ADX stream)
    /// with independent per-layer volume controls (`active_bgm_layers`).
    pub fn play_bgm_layers(
        &mut self,
        layers: &[DecodedAdx],
        initial_gains: &[f32],
        loop_whole_stream: bool,
    ) -> Result<(), AudioError> {
        self.stop_bgm(Duration::from_millis(40));

        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let bgm_track = self
            .bgm_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No bgm track".into()))?;

        let mut handles = Vec::with_capacity(layers.len());
        for (idx, layer) in layers.iter().enumerate() {
            let (sample_rate, frames) = adx_to_frames_48k(layer);
            let gain = initial_gains.get(idx).copied().unwrap_or(0.0).max(0.0);
            let mut settings = StaticSoundSettings::new()
                .output_destination(bgm_track)
                .volume(Volume::Amplitude(gain as f64));

            if layer.header.has_loop
                && layer.header.loop_end_sample > layer.header.loop_start_sample
            {
                if let Some(region) = LoopRegionSamples::from_source_samples(
                    layer.header.loop_start_sample,
                    layer.header.loop_end_sample,
                    layer.header.sample_rate,
                    sample_rate,
                    frames.len(),
                ) {
                    settings = settings.loop_region(region.to_kira_region());
                }
            } else if loop_whole_stream {
                settings = settings.loop_region(Region {
                    start: PlaybackPosition::Seconds(0.0),
                    end: EndPosition::EndOfAudio,
                });
            }

            let sound_data = StaticSoundData {
                sample_rate,
                frames: Arc::from(frames),
                settings,
                slice: None,
            };

            let handle = manager
                .play(sound_data)
                .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
            handles.push(handle);
        }

        self.active_bgm_layers = handles;
        Ok(())
    }

    /// Updates the linear amplitude gain (`0.0..=1.0`, relative to the BGM sub-track) of a single
    /// active BGM layer (`layer_idx` in `0..active_bgm_layer_count()`).
    pub fn set_bgm_layer_gain(&mut self, layer_idx: usize, gain: f32, fade_duration: Duration) {
        let tween = Tween {
            duration: fade_duration,
            easing: Easing::Linear,
            ..Default::default()
        };
        if let Some(handle) = self.active_bgm_layers.get_mut(layer_idx) {
            handle.set_volume(Volume::Amplitude(gain.max(0.0) as f64), tween);
        } else if layer_idx == 0 {
            if let Some(ref mut handle) = self.active_bgm_handle {
                handle.set_volume(Volume::Amplitude(gain.max(0.0) as f64), tween);
            }
        }
    }

    /// Returns the number of currently active multi-layer BGM tracks.
    pub fn active_bgm_layer_count(&self) -> usize {
        if !self.active_bgm_layers.is_empty() {
            self.active_bgm_layers.len()
        } else if self.active_bgm_handle.is_some() {
            1
        } else {
            0
        }
    }

    /// Returns whether background music is currently actively playing.
    pub fn is_bgm_playing(&self) -> bool {
        self.active_bgm_handle.is_some() || !self.active_bgm_layers.is_empty()
    }

    /// Stops currently playing background music (both single-stream ADX and multi-layer AIX) with a fade out.
    pub fn stop_bgm(&mut self, fade_out: Duration) {
        let tween = Tween {
            duration: fade_out,
            easing: Easing::Linear,
            ..Default::default()
        };
        if let Some(mut handle) = self.active_bgm_handle.take() {
            handle.stop(tween);
        }
        for mut handle in self.active_bgm_layers.drain(..) {
            handle.stop(tween);
        }
    }

    /// Stops currently playing FMV cutscene audio stream.
    pub fn stop_fmv_audio(&mut self, fade_out: Duration) {
        if let Some(mut handle) = self.active_fmv_handle.take() {
            handle.stop(Tween {
                duration: fade_out,
                easing: Easing::Linear,
                ..Default::default()
            });
        }
    }

    /// Plays decoded 16-bit signed little-endian stereo PCM audio for an FMV cutscene on the dedicated FMV track.
    pub fn play_fmv_pcm(
        &mut self,
        sample_rate: u32,
        pcm_s16le: &[u8],
        volume: f32,
    ) -> Result<(), AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let fmv_track = self
            .fmv_track
            .as_ref()
            .or(self.bgm_track.as_ref())
            .ok_or_else(|| AudioError::InitError("No fmv/bgm track".into()))?;

        if let Some(mut old_handle) = self.active_fmv_handle.take() {
            old_handle.stop(Tween {
                duration: Duration::from_millis(50),
                easing: Easing::Linear,
                ..Default::default()
            });
        }

        let wav = WavEntry {
            id: 0,
            sample_rate: sample_rate.max(8000),
            channels: 2,
            bits_per_sample: 16,
            loops: false,
            loop_start: 0,
            loop_end: 0,
            data: pcm_s16le.to_vec(),
        };
        let frames = wav_entry_to_frames(&wav);
        let settings = StaticSoundSettings::new()
            .output_destination(fmv_track)
            .volume(Volume::Amplitude(
                (volume * self.bgm_volume * self.master_volume) as f64,
            ));

        let sound_data = StaticSoundData {
            sample_rate: wav.sample_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        let handle = manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
        self.active_fmv_handle = Some(handle);
        Ok(())
    }

    /// Pauses or resumes active FMV / BGM audio playback.
    pub fn set_bgm_paused(&mut self, paused: bool) {
        let tween = Tween {
            duration: Duration::from_millis(40),
            easing: Easing::Linear,
            ..Default::default()
        };
        if let Some(ref mut handle) = self.active_bgm_handle {
            if paused {
                handle.pause(tween);
            } else {
                handle.resume(tween);
            }
        }
        for handle in &mut self.active_bgm_layers {
            if paused {
                handle.pause(tween);
            } else {
                handle.resume(tween);
            }
        }
        if let Some(ref mut handle) = self.active_fmv_handle {
            if paused {
                handle.pause(tween);
            } else {
                handle.resume(tween);
            }
        }
    }
}
