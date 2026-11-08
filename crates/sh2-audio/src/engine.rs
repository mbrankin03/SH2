//! Audio engine wrapper around `kira` for music, 3D sound effects, and dialogue voice.
//!
//! # Architecture & Sound Pipeline (`SH2-55`)
//! - **Modular Subsystems**:
//!   - [`crate::acoustics`]: Dynamic room acoustics and reverb modeling from `.cam` volume properties (`rd_type`, `area_size_type`).
//!   - [`crate::looping`]: Sample-accurate loop-point evaluation and raised-cosine loop-boundary crossfading.
//!   - [`crate::ringbuffer`]: Lock-free, zero-allocation SPSC stereo [`AudioRingBuffer`] for real-time platform callbacks.
//!   - [`crate::dsp_resample`]: Catmull-Rom 48 kHz upsampling and Butterworth anti-imaging reconstruction filtering.
//!   - [`crate::device_routing`]: Platform output device enumeration and PipeWire/PulseAudio live sink migration.
//!   - [`crate::dialogue`]: `.mes` subtitle page synchronization and automatic BGM ducking.

use glam::Vec3;
use kira::{
    effect::reverb::{ReverbBuilder, ReverbHandle},
    manager::{
        backend::{cpal::CpalBackendSettings, DefaultBackend},
        AudioManager, AudioManagerSettings,
    },
    sound::{
        static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
        EndPosition, PlaybackPosition, Region,
    },
    spatial::{
        emitter::{EmitterDistances, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
        scene::{SpatialSceneHandle, SpatialSceneSettings},
    },
    track::{TrackBuilder, TrackHandle},
    tween::{Easing, Tween},
    Volume,
};
use mint::{Quaternion, Vector3};
use sh2_assets::{CamFile, DecodedAdx, DialogueMessage, RoadRecord, WavEntry};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub use crate::acoustics::{
    evaluate_room_acoustics, evaluate_room_acoustics_for_pos, EnvironmentalPreset,
    RoomAcousticsProfile,
};
pub use crate::dialogue::ActiveDialogue;
pub use crate::dsp_resample::{adx_to_frames_48k, wav_entry_to_frames, wav_entry_to_frames_48k};
pub use crate::looping::LoopRegionSamples;
pub use crate::ringbuffer::{AudioRingBuffer, DEFAULT_RINGBUFFER_FRAMES};

/// Error type for audio playback and management.
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Audio backend initialization failed: {0}")]
    InitError(String),

    #[error("Resource limit reached: {0}")]
    ResourceLimit(String),

    #[error("Playback failed: {0}")]
    PlaybackError(String),

    #[error("Asset error: {0}")]
    AssetError(String),
}

/// Master audio engine managing streaming BGM, 3D sound effects, environmental DSP, and dialogue.
pub struct AudioEngine {
    /// Underlying Kira backend manager.
    manager: Option<AudioManager<DefaultBackend>>,

    /// Sub-track handle for streaming background music.
    bgm_track: Option<TrackHandle>,
    /// Dedicated sub-track handle for FMV cutscene audio stream (.bik decoded audio).
    fmv_track: Option<TrackHandle>,
    /// Sub-track handle for 3D and 2D sound effects.
    sfx_track: Option<TrackHandle>,
    /// Sub-track handle for dialogue voice lines.
    voice_track: Option<TrackHandle>,

    /// Spatial audio scene for 3D positional effects.
    spatial_scene: Option<SpatialSceneHandle>,
    /// Spatial listener attached to the camera/player.
    listener: Option<ListenerHandle>,

    /// Reverb DSP effect handle on SFX track.
    reverb_handle: Option<ReverbHandle>,
    /// Current environmental reverb preset.
    current_preset: EnvironmentalPreset,
    /// Current continuous room acoustics profile derived from active `.cam` volume (`SH2-55`).
    current_acoustics: RoomAcousticsProfile,

    /// Thread-safe, zero-allocation SPSC stereo audio ringbuffer (`SH2-55`).
    output_ringbuffer: Arc<AudioRingBuffer>,

    /// Handle for currently playing background ambient/music stream (single-stream ADX).
    active_bgm_handle: Option<StaticSoundHandle>,
    /// Handles for currently playing multi-layer AIX background music streams (up to 7 synchronized stereo layers).
    active_bgm_layers: Vec<StaticSoundHandle>,
    /// Handle for currently playing FMV movie audio stream.
    active_fmv_handle: Option<StaticSoundHandle>,
    /// Currently playing voice clip handle.
    active_voice_handle: Option<StaticSoundHandle>,

    /// Currently active dialogue with subtitles.
    active_dialogue: Option<ActiveDialogue>,

    /// Master volume level (0.0 to 1.0).
    master_volume: f32,
    /// BGM volume multiplier.
    bgm_volume: f32,
    /// SFX volume multiplier.
    sfx_volume: f32,
    /// Voice volume multiplier.
    voice_volume: f32,

    /// Target ducking factor applied to BGM during voice playback (e.g. 0.35).
    ducking_attenuation: f32,
}

impl AudioEngine {
    /// Enumerate all currently available audio output device names on the host system.
    pub fn enumerate_output_devices() -> Vec<String> {
        crate::device_routing::enumerate_output_devices()
    }

    /// Initializes the Kira audio manager with dedicated tracks, spatial scene, and DSP effects.
    pub fn new_with_device(device_name: Option<&str>) -> Result<Self, AudioError> {
        let (clean_device_name, device_opt) =
            crate::device_routing::select_cpal_output_device(device_name);
        let device = device_opt
            .ok_or_else(|| AudioError::InitError("No audio output device available".into()))?;

        let settings = AudioManagerSettings {
            backend_settings: CpalBackendSettings {
                device: Some(device),
                buffer_size: cpal::BufferSize::Default,
            },
            ..Default::default()
        };

        let mut manager = AudioManager::<DefaultBackend>::new(settings)
            .map_err(|e| AudioError::InitError(e.to_string()))?;

        let bgm_track = manager
            .add_sub_track(TrackBuilder::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let fmv_track = manager
            .add_sub_track(TrackBuilder::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let mut sfx_track_builder = TrackBuilder::default();
        let reverb_handle =
            sfx_track_builder.add_effect(ReverbBuilder::new().feedback(0.5).damping(0.4).mix(0.0));

        let sfx_track = manager
            .add_sub_track(sfx_track_builder)
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let voice_track = manager
            .add_sub_track(TrackBuilder::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let mut spatial_scene = manager
            .add_spatial_scene(SpatialSceneSettings::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let listener = spatial_scene
            .add_listener(
                Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Quaternion {
                    v: Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    s: 1.0,
                },
                ListenerSettings::default(),
            )
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let mut engine = Self {
            manager: Some(manager),
            bgm_track: Some(bgm_track),
            fmv_track: Some(fmv_track),
            sfx_track: Some(sfx_track),
            voice_track: Some(voice_track),
            spatial_scene: Some(spatial_scene),
            listener: Some(listener),
            reverb_handle: Some(reverb_handle),
            current_preset: EnvironmentalPreset::Off,
            current_acoustics: RoomAcousticsProfile::from_preset(EnvironmentalPreset::Off),
            output_ringbuffer: Arc::new(AudioRingBuffer::with_capacity(DEFAULT_RINGBUFFER_FRAMES)),
            active_bgm_handle: None,
            active_bgm_layers: Vec::new(),
            active_fmv_handle: None,
            active_voice_handle: None,
            active_dialogue: None,
            master_volume: 1.0,
            bgm_volume: 0.85,
            sfx_volume: 0.9,
            voice_volume: 1.0,
            ducking_attenuation: 0.35,
        };

        if let Some(target) = clean_device_name {
            engine.migrate_stream_to_device(Some(target));
        }

        Ok(engine)
    }

    /// Initializes the Kira audio manager with dedicated tracks, spatial scene, and DSP effects using the default output device.
    pub fn new() -> Result<Self, AudioError> {
        Self::new_with_device(None)
    }

    /// Checks if the audio backend was successfully initialized and active.
    pub fn is_active(&self) -> bool {
        self.manager.is_some()
    }

    /// Returns a handle to the engine's lock-free zero-allocation SPSC output ringbuffer (`SH2-55`).
    pub fn output_ringbuffer(&self) -> &Arc<AudioRingBuffer> {
        &self.output_ringbuffer
    }

    /// Updates listener position and orientation for 3D positional audio.
    pub fn set_listener_transform(&mut self, player_pos: Vec3, forward: Vec3) {
        if let Some(ref mut listener) = self.listener {
            listener.set_position(
                Vector3 {
                    x: player_pos.x,
                    y: player_pos.y,
                    z: player_pos.z,
                },
                Tween::default(),
            );

            let yaw = (-forward.x).atan2(-forward.z);
            let half_yaw = yaw * 0.5;
            let q = Quaternion {
                v: Vector3 {
                    x: 0.0,
                    y: half_yaw.sin(),
                    z: 0.0,
                },
                s: half_yaw.cos(),
            };
            listener.set_orientation(q, Tween::default());
        }
    }

    /// Sets the environmental DSP reverb preset.
    pub fn set_environmental_preset(&mut self, preset: EnvironmentalPreset) {
        self.apply_acoustics_profile(RoomAcousticsProfile::from_preset(preset));
    }

    /// Applies a continuous [`RoomAcousticsProfile`] (derived from `.cam` `rd_type` / `area_size_type`
    /// or a discrete preset) to the SFX reverb processor (`SH2-55`).
    pub fn apply_acoustics_profile(&mut self, profile: RoomAcousticsProfile) {
        self.current_preset = profile.preset;
        self.current_acoustics = profile;
        let tween = Tween {
            duration: Duration::from_millis(150),
            easing: Easing::Linear,
            ..Default::default()
        };
        if let Some(ref mut reverb) = self.reverb_handle {
            if profile.wet_mix > 0.0 {
                reverb.set_feedback(profile.feedback, tween);
                reverb.set_damping(profile.damping, tween);
                reverb.set_mix(profile.wet_mix, tween);
            } else {
                reverb.set_mix(0.0, tween);
            }
        }
    }

    /// Evaluates and applies dynamic room acoustics from the active `.cam` [`RoadRecord`]
    /// at `player_pos` (`SH2-55`).
    pub fn update_room_acoustics_from_cam(
        &mut self,
        cam: &CamFile,
        player_pos: Vec3,
        is_interior_map: bool,
    ) -> RoomAcousticsProfile {
        let profile = evaluate_room_acoustics_for_pos(cam, player_pos, is_interior_map);
        self.apply_acoustics_profile(profile);
        profile
    }

    /// Evaluates and applies dynamic room acoustics directly from an active [`RoadRecord`] (`SH2-55`).
    pub fn update_room_acoustics_from_record(
        &mut self,
        record: Option<&RoadRecord>,
        is_interior_map: bool,
    ) -> RoomAcousticsProfile {
        let profile = evaluate_room_acoustics(record, is_interior_map);
        self.apply_acoustics_profile(profile);
        profile
    }

    /// Current active environmental preset.
    pub fn current_preset(&self) -> EnvironmentalPreset {
        self.current_preset
    }

    /// Current active continuous [`RoomAcousticsProfile`] (`SH2-55`).
    pub fn current_acoustics(&self) -> RoomAcousticsProfile {
        self.current_acoustics
    }

    /// Plays a 2D sound effect waveform from a parsed `WavEntry` (e.g. menu sounds, item pickups).
    pub fn play_sfx_2d(
        &mut self,
        wav: &WavEntry,
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

        let (sample_rate, frames) = wav_entry_to_frames_48k(wav);
        self.output_ringbuffer
            .push_frames(&frames[..frames.len().min(512)]);

        let mut settings = StaticSoundSettings::new()
            .output_destination(sfx_track)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ));

        if wav.loops {
            if let Some(region) = LoopRegionSamples::from_source_samples(
                wav.loop_start,
                wav.loop_end,
                wav.sample_rate,
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

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Plays a 3D spatialized sound effect at a world-space position.
    pub fn play_sfx_3d(
        &mut self,
        wav: &WavEntry,
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

        let spread = self.current_acoustics.spatial_spread.clamp(0.5, 1.5);
        let emitter_settings = EmitterSettings::new()
            .distances(EmitterDistances {
                min_distance: 15.0 * spread,
                max_distance: 115.0 * spread,
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

        let (sample_rate, frames) = wav_entry_to_frames_48k(wav);
        let mut settings = StaticSoundSettings::new()
            .output_destination(&emitter)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ));

        if wav.loops {
            if let Some(region) = LoopRegionSamples::from_source_samples(
                wav.loop_start,
                wav.loop_end,
                wav.sample_rate,
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

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

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

    /// Triggers a dialogue voice clip synchronized with `.mes` subtitles and volume ducking.
    pub fn play_dialogue(
        &mut self,
        voice_adx: &DecodedAdx,
        message: DialogueMessage,
        voice_id: Option<u32>,
    ) -> Result<(), AudioError> {
        self.apply_ducking(true);

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

    /// Applies or releases volume ducking on the BGM track.
    fn apply_ducking(&mut self, ducked: bool) {
        if let Some(ref mut track) = self.bgm_track {
            let target_gain = if ducked {
                (self.bgm_volume * self.ducking_attenuation * self.master_volume) as f64
            } else {
                (self.bgm_volume * self.master_volume) as f64
            };
            track.set_volume(
                Volume::Amplitude(target_gain),
                Tween {
                    duration: Duration::from_millis(200),
                    easing: Easing::Linear,
                    ..Default::default()
                },
            );
        }
    }

    /// Per-frame update step: advances active dialogue subtitle triggers and releases ducking on completion.
    pub fn update(&mut self, dt: Duration) {
        if let Some(ref mut dialogue) = self.active_dialogue {
            dialogue.update(dt);
            if dialogue.is_finished {
                self.apply_ducking(false);
            }
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

    /// Sets the master volume level [0.0..1.0].
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    /// Migrates the active running game audio stream to the requested device in real-time.
    pub fn migrate_stream_to_device(&mut self, device_name: Option<&str>) {
        crate::device_routing::migrate_stream_to_device(device_name);
    }

    /// Sets the background music volume level [0.0..1.0].
    pub fn set_bgm_volume(&mut self, volume: f32) {
        self.bgm_volume = volume.clamp(0.0, 1.0);
        if let Some(ref mut track) = self.bgm_track {
            track.set_volume(
                Volume::Amplitude((self.bgm_volume * self.master_volume) as f64),
                Tween {
                    duration: Duration::from_millis(50),
                    easing: Easing::Linear,
                    ..Default::default()
                },
            );
        }
    }

    /// Sets the sound effects volume level [0.0..1.0].
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
        if let Some(ref mut track) = self.sfx_track {
            track.set_volume(
                Volume::Amplitude((self.sfx_volume * self.master_volume) as f64),
                Tween {
                    duration: Duration::from_millis(50),
                    easing: Easing::Linear,
                    ..Default::default()
                },
            );
        }
    }

    /// Sets the dialogue voice volume level [0.0..1.0].
    pub fn set_voice_volume(&mut self, volume: f32) {
        self.voice_volume = volume.clamp(0.0, 1.0);
        if let Some(ref mut track) = self.voice_track {
            track.set_volume(
                Volume::Amplitude((self.voice_volume * self.master_volume) as f64),
                Tween {
                    duration: Duration::from_millis(50),
                    easing: Easing::Linear,
                    ..Default::default()
                },
            );
        }
    }

    /// Sets the environmental DSP reverb mix ratio [0.0..1.0].
    pub fn set_reverb_mix(&mut self, mix: f32) {
        if let Some(ref mut reverb) = self.reverb_handle {
            reverb.set_mix(
                mix.clamp(0.0, 1.0) as f64,
                Tween {
                    duration: Duration::from_millis(50),
                    easing: Easing::Linear,
                    ..Default::default()
                },
            );
        }
    }
}
