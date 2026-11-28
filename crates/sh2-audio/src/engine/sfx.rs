//! 2D, classic panned, Kira spatial, and split dry/wet 3D SFX playback methods on [`AudioEngine`] (`SH2-55` & `SH2-84`).

use std::sync::Arc;
use std::time::Duration;

use glam::Vec3;
use kira::{
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    spatial::emitter::{EmitterDistances, EmitterSettings},
    tween::{Easing, Tween},
    Volume,
};
use mint::Vector3;
use sh2_assets::WavEntry;

use super::{prepare_wav_entry_48k, AudioEngine, AudioError, PreparedSfx};
use crate::spatial_voice::{
    SpatialDrySoundData, SpatialVoiceControl, SpatialVoiceHandle, SpatialVoiceInitParams,
    SpatialWetSendSoundData,
};

impl AudioEngine {
    /// Plays a pre-resampled 2D sound effect via zero-copy `Arc::clone` (`SH2-179`).
    pub fn play_prepared_sfx_2d(
        &mut self,
        prepared: &PreparedSfx,
        volume: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let sfx_track = self
            .sfx_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No sfx track".into()))?;

        self.output_ringbuffer
            .push_frames(&prepared.frames[..prepared.frames.len().min(512)]);

        let mut settings = StaticSoundSettings::new()
            .output_destination(sfx_track)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ));

        if let Some(region) = prepared.loop_region {
            settings = settings.loop_region(region.to_kira_region());
        }

        let sound_data = StaticSoundData {
            sample_rate: prepared.sample_rate,
            frames: Arc::clone(&prepared.frames),
            settings,
            slice: None,
        };

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Plays a 2D sound effect waveform from a parsed `WavEntry` (e.g. menu sounds, item pickups).
    pub fn play_sfx_2d(
        &mut self,
        wav: &WavEntry,
        volume: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        let prepared = prepare_wav_entry_48k(wav);
        self.play_prepared_sfx_2d(&prepared, volume)
    }

    /// Plays a pre-resampled 2D ambient sound effect in continuous loop mode (`loop_region(..)`,
    /// or the waveform's embedded `smpl` loop points if present) for persistent stage ambience
    /// (`SeCallPos` with `flags & 8 != 0`, e.g., Forest wind `0x2742`, Rain `0x4DB8`, Lake `0x3CF0`).
    pub fn play_prepared_sfx_2d_looping(
        &mut self,
        prepared: &PreparedSfx,
        volume: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let sfx_track = self
            .sfx_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No sfx track".into()))?;

        self.output_ringbuffer
            .push_frames(&prepared.frames[..prepared.frames.len().min(512)]);

        let mut settings = StaticSoundSettings::new()
            .output_destination(sfx_track)
            .volume(Volume::Amplitude(
                (volume.max(0.0) * self.sfx_volume * self.master_volume) as f64,
            ));

        if let Some(region) = prepared.loop_region {
            settings = settings.loop_region(region.to_kira_region());
        } else {
            settings = settings.loop_region(..);
        }

        let sound_data = StaticSoundData {
            sample_rate: prepared.sample_rate,
            frames: Arc::clone(&prepared.frames),
            settings,
            slice: None,
        };

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Smoothly updates the linear amplitude volume of an active 2D looping sound effect handle
    /// (`SeVolume` `sh2pc.exe` `0x00516d50`, e.g. Forest wind gust oscillation `0x2742`).
    pub fn set_sfx_2d_volume(&self, handle: &mut StaticSoundHandle, volume: f32) {
        let tween = Tween {
            duration: Duration::from_millis(16),
            easing: Easing::Linear,
            ..Default::default()
        };
        let target_vol = (volume.max(0.0) * self.sfx_volume * self.master_volume) as f64;
        handle.set_volume(Volume::Amplitude(target_vol), tween);
    }

    /// Plays a pre-resampled 3D spatialized sound effect via Kira's `SpatialScene` using zero-copy `Arc::clone` (`SH2-179`).
    pub fn play_prepared_sfx_3d(
        &mut self,
        prepared: &PreparedSfx,
        position: Vec3,
        volume: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let spatial_scene = self
            .spatial_scene
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No spatial scene".into()))?;

        // Exact `SeCallPosDistanceF` (`0x00192bc0` / `sh2pc 0x005155e0`) radii scaled by 0.01 (1500..11500 mm -> 15..115).
        // Do NOT scale by `spatial_spread` (which was a synthetic heuristic absent from PS2/PC).
        let emitter_settings = EmitterSettings::new()
            .distances(EmitterDistances {
                min_distance: crate::sfx_bank::SFX_MIN_DISTANCE * 0.01,
                max_distance: crate::sfx_bank::SFX_MAX_DISTANCE * 0.01,
            })
            .attenuation_function(Some(Easing::Linear))
            .enable_spatialization(true)
            .persist_until_sounds_finish(true);

        let emitter = spatial_scene
            .add_emitter(
                Vector3 {
                    x: position.x * 0.01,
                    y: position.y * 0.01,
                    z: position.z * 0.01,
                },
                emitter_settings,
            )
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let mut settings = StaticSoundSettings::new()
            .output_destination(&emitter)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ));

        if let Some(region) = prepared.loop_region {
            settings = settings.loop_region(region.to_kira_region());
        }

        let sound_data = StaticSoundData {
            sample_rate: prepared.sample_rate,
            frames: Arc::clone(&prepared.frames),
            settings,
            slice: None,
        };

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Plays a 3D spatialized sound effect at a world-space position via Kira's `SpatialScene`.
    pub fn play_sfx_3d(
        &mut self,
        wav: &WavEntry,
        position: Vec3,
        volume: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        let prepared = prepare_wav_entry_48k(wav);
        self.play_prepared_sfx_3d(&prepared, position, volume)
    }

    /// Plays a pre-resampled sound effect on the SFX sub-track with explicit linear `volume`
    /// (`0.0..=1.0`) and `stereo_pan` (`-1.0..=1.0`) via zero-copy `Arc::clone` (`SH2-179`).
    pub fn play_prepared_sfx_panned(
        &mut self,
        prepared: &PreparedSfx,
        volume: f32,
        stereo_pan: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        self.play_prepared_sfx_panned_with_loop(prepared, volume, stereo_pan, false)
    }

    /// Plays a pre-resampled sound effect on the SFX sub-track with explicit `volume`,
    /// `stereo_pan`, and `looping` (`SE_FLAG_MANAGED_2D` managed 2D loops whose retail
    /// `sddata.bin` entries lack an embedded `smpl` chunk, e.g., `0x3E80`, `0x4268`, `0x4269`,
    /// `0x465A`, `0x42CB`, `0x44C6`, `0x3CF0`).
    pub fn play_prepared_sfx_panned_with_loop(
        &mut self,
        prepared: &PreparedSfx,
        volume: f32,
        stereo_pan: f32,
        looping: bool,
    ) -> Result<StaticSoundHandle, AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let sfx_track = self
            .sfx_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No sfx track".into()))?;

        self.output_ringbuffer
            .push_frames(&prepared.frames[..prepared.frames.len().min(512)]);

        let pan_01 = ((stereo_pan.clamp(-1.0, 1.0) + 1.0) * 0.5) as f64;
        let mut settings = StaticSoundSettings::new()
            .output_destination(sfx_track)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ))
            .panning(pan_01);

        if let Some(region) = prepared.loop_region {
            settings = settings.loop_region(region.to_kira_region());
        } else if looping {
            settings = settings.loop_region(..);
        }

        let sound_data = StaticSoundData {
            sample_rate: prepared.sample_rate,
            frames: Arc::clone(&prepared.frames),
            settings,
            slice: None,
        };

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Plays a sound effect on the SFX sub-track (with room reverb DSP) using explicit PS2
    /// `SeCallPosDistanceF` linear amplitude `volume` (`0.0..=1.0`) and `stereo_pan` (`-1.0..=1.0`).
    pub fn play_sfx_panned(
        &mut self,
        wav: &WavEntry,
        volume: f32,
        stereo_pan: f32,
    ) -> Result<StaticSoundHandle, AudioError> {
        let prepared = prepare_wav_entry_48k(wav);
        self.play_prepared_sfx_panned(&prepared, volume, stereo_pan)
    }

    /// Updates the live volume and stereo panning of an active sound effect handle (`SePosMove` / `SePosSet`).
    pub fn update_sound_spatial(
        &self,
        handle: &mut StaticSoundHandle,
        volume: f32,
        stereo_pan: f32,
    ) {
        let tween = Tween {
            duration: Duration::from_millis(16),
            easing: Easing::Linear,
            ..Default::default()
        };
        let target_vol = (volume.max(0.0) * self.sfx_volume * self.master_volume) as f64;
        let pan_01 = ((stereo_pan.clamp(-1.0, 1.0) + 1.0) * 0.5) as f64;
        handle.set_volume(Volume::Amplitude(target_vol), tween);
        handle.set_panning(pan_01, tween);
    }

    /// Plays a pre-resampled 3D point-source SFX using the split dry/wet real-time DSP pipeline
    /// via zero-copy `Arc::clone` (`SH2-84`, `SH2-179`).
    pub fn play_prepared_sfx_spatial_3d(
        &mut self,
        prepared: &PreparedSfx,
        mut init: SpatialVoiceInitParams,
    ) -> Result<SpatialVoiceHandle, AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let dry_track_id = self
            .sfx_dry_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No sfx dry track".into()))?
            .id();
        let reverb_bus_id = self
            .sfx_reverb_bus
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No sfx reverb bus".into()))?
            .id();

        self.output_ringbuffer
            .push_frames(&prepared.frames[..prepared.frames.len().min(512)]);

        init.base_distance_gain = if init.base_distance_gain.is_finite() {
            init.base_distance_gain.max(0.0)
        } else {
            0.0
        };

        let frames_empty = prepared.frames.is_empty();
        let control = SpatialVoiceControl::new_with_flags(
            &init,
            Arc::clone(&self.binaural_hrtf_flag),
            Arc::clone(&self.occlusion_enabled_flag),
            frames_empty,
        );

        let dry_data = SpatialDrySoundData {
            frames: Arc::clone(&prepared.frames),
            sample_rate: prepared.sample_rate,
            loop_region: prepared.loop_region,
            init,
            control: Arc::clone(&control),
            destination: dry_track_id,
        };
        let wet_data = SpatialWetSendSoundData {
            frames: Arc::clone(&prepared.frames),
            sample_rate: prepared.sample_rate,
            loop_region: prepared.loop_region,
            init,
            control,
            reverb_bus: reverb_bus_id,
        };

        let handle = manager
            .play(dry_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
        if let Err(e) = manager.play(wet_data) {
            handle.stop();
            return Err(AudioError::PlaybackError(e.to_string()));
        }

        Ok(handle)
    }

    /// Plays a 3D point-source SFX using the split dry (`.cld` wall-occluded + Binaural HRTF / Classic PS2 Stereo)
    /// and wet corridor reverb send (`RoomAcousticsProfile`) real-time DSP pipeline (`SH2-84`).
    ///
    /// Note: `sfx_dry_track` and `sfx_reverb_bus` already scale output amplitude by `sfx_volume * master_volume`,
    /// so `init.base_distance_gain` remains pure `SeCallPosDistanceF` distance gain (`[0.0, 1.0]`).
    pub fn play_sfx_spatial_3d(
        &mut self,
        wav: &WavEntry,
        init: SpatialVoiceInitParams,
    ) -> Result<SpatialVoiceHandle, AudioError> {
        let prepared = prepare_wav_entry_48k(wav);
        self.play_prepared_sfx_spatial_3d(&prepared, init)
    }

    /// Updates a live [`SpatialVoiceHandle`] with fresh distance gain, azimuth/elevation, and `.cld` occlusion (`SH2-84`).
    pub fn update_spatial_voice(
        &self,
        handle: &SpatialVoiceHandle,
        mut params: SpatialVoiceInitParams,
    ) {
        params.base_distance_gain = if params.base_distance_gain.is_finite() {
            params.base_distance_gain.max(0.0)
        } else {
            0.0
        };
        handle.update(&params);
    }
}
