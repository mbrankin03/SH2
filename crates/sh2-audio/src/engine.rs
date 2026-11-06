//! Audio engine wrapper around `kira` for music, 3D sound effects, and dialogue voice.
//!
//! # Architecture & Sound Pipeline
//! - DirectSound / PS2 driver emulation:
//!   - High-priority 2D effect routing (`shSdCall`).
//!   - Positional 3D attenuation calculations (`SeCallPosDistanceF`).
//!   - Dedicated channels: BGM streaming channel, 3D sound effects channel, and dialogue voice channel.
//!   - Environmental DSP / Reverb matching PS2/PC presets (interiors, foggy streets, outdoor open areas).
//!   - Subtitle synchronization with `.mes` event cues and automatic BGM/SFX volume ducking during dialogue.

use cpal::traits::{DeviceTrait, HostTrait};
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
    Frame, Volume,
};
use mint::{Quaternion, Vector3};
use sh2_assets::{DecodedAdx, DialogueMessage, WavEntry};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

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

/// Environmental reverb preset matching original PS2 / PC acoustics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentalPreset {
    /// Dry direct sound with no reverb (default / outdoor open area).
    Off,
    /// Thick foggy exterior with soft low-frequency damping.
    FoggyStreets,
    /// Enclosed interior room with short reverberation.
    InteriorRoom,
    /// Resonant metallic hallway or apartment corridor.
    Corridor,
    /// Wet cavernous echo (e.g. Prison basement, labyrinth).
    CavernousEcho,
}

impl EnvironmentalPreset {
    /// Returns the corresponding `(feedback, damping, mix)` parameters for Kira reverb.
    pub fn reverb_params(&self) -> Option<(f64, f64, f64)> {
        match self {
            Self::Off => None,
            Self::FoggyStreets => Some((0.35, 0.70, 0.20)),
            Self::InteriorRoom => Some((0.55, 0.40, 0.30)),
            Self::Corridor => Some((0.75, 0.25, 0.45)),
            Self::CavernousEcho => Some((0.90, 0.15, 0.60)),
        }
    }
}

/// Active dialogue playback state tracking subtitle synchronization and voice timing.
#[derive(Debug, Clone)]
pub struct ActiveDialogue {
    /// Current dialogue message being spoken.
    pub message: DialogueMessage,
    /// Voice track identifier or index if known.
    pub voice_id: Option<u32>,
    /// Total duration of the voice line.
    pub total_duration: Duration,
    /// Elapsed playback time in seconds.
    pub elapsed: Duration,
    /// Index of the current active page of subtitle text.
    pub current_page_index: usize,
    /// Whether this dialogue has completed playback.
    pub is_finished: bool,
}

impl ActiveDialogue {
    /// Returns the currently displayed subtitle page text, if any.
    pub fn current_subtitle(&self) -> Option<&str> {
        self.message
            .pages
            .get(self.current_page_index)
            .map(|s| s.as_str())
    }

    /// Advances the dialogue timer and calculates active subtitle page based on progress.
    pub fn update(&mut self, dt: Duration) {
        if self.is_finished {
            return;
        }

        self.elapsed += dt;
        if self.elapsed >= self.total_duration {
            self.is_finished = true;
            return;
        }

        if !self.message.pages.is_empty() {
            let num_pages = self.message.pages.len();
            let page_duration = self.total_duration.as_secs_f32() / (num_pages as f32);
            let cur_idx = (self.elapsed.as_secs_f32() / page_duration).floor() as usize;
            self.current_page_index = cur_idx.min(num_pages - 1);
        }
    }
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

    /// Handle for currently playing background ambient/music stream.
    active_bgm_handle: Option<StaticSoundHandle>,
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
    ///
    /// Resolves human-readable device names (e.g. "MiniFuse 2", "Headphones", "HDMI") via PulseAudio / PipeWire
    /// when available on Linux desktop systems, cleanly filtering out internal ALSA DSP plugins like "speex",
    /// "speexrate", "samplerate", or "upmix".
    pub fn enumerate_output_devices() -> Vec<String> {
        let mut devices = Vec::new();

        // 1. Attempt querying PulseAudio / PipeWire sinks for friendly display descriptions
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = std::process::Command::new("pactl")
                .args(["-f", "json", "list", "sinks"])
                .output()
            {
                if output.status.success() {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        if let Some(sinks) = val.as_array() {
                            for sink in sinks {
                                let name = sink.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                let desc = sink
                                    .get("description")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or(name);

                                // Filter out non-hardware / internal DSP filter sinks
                                if name.contains("spotifast")
                                    || desc.contains("Soloist")
                                    || name.contains("Loopback")
                                    || desc.contains("Loopback")
                                {
                                    continue;
                                }

                                let clean_name = desc.trim().to_string();
                                if !clean_name.is_empty() && !devices.contains(&clean_name) {
                                    devices.push(clean_name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Query CPAL host devices if pactl is unavailable or returns no sinks
        if devices.is_empty() {
            let host = cpal::default_host();
            if let Ok(iter) = host.output_devices() {
                for device in iter {
                    if let Ok(name) = device.name() {
                        // Filter out raw ALSA software rate/resample plugins that confuse users
                        let lower = name.to_lowercase();
                        if lower.contains("speex")
                            || lower.contains("rate")
                            || lower.contains("samplerate")
                            || lower.contains("upmix")
                            || lower.contains("vdownmix")
                        {
                            continue;
                        }
                        if !devices.contains(&name) {
                            devices.push(name);
                        }
                    }
                }
            }
        }

        devices
    }

    /// Initializes the Kira audio manager with dedicated tracks, spatial scene, and DSP effects.
    ///
    /// If `device_name` is provided, attempts to open that specific audio output device;
    /// otherwise falls back to the system default device.
    pub fn new_with_device(device_name: Option<&str>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = if let Some(target_name) = device_name {
            let mut found = None;
            if let Ok(iter) = host.output_devices() {
                for dev in iter {
                    if let Ok(name) = dev.name() {
                        if name == target_name {
                            found = Some(dev);
                            break;
                        }
                    }
                }
            }
            if found.is_none() {
                tracing::warn!(
                    "Requested audio output device '{target_name}' not found; falling back to default."
                );
            }
            found.or_else(|| host.default_output_device())
        } else {
            host.default_output_device()
        }
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

        // 1. Create BGM Track
        let bgm_track = manager
            .add_sub_track(TrackBuilder::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        // 1b. Create FMV Track
        let fmv_track = manager
            .add_sub_track(TrackBuilder::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        // 2. Create Reverb Effect and SFX Track
        let mut sfx_track_builder = TrackBuilder::default();
        let reverb_handle = sfx_track_builder.add_effect(
            ReverbBuilder::new().feedback(0.5).damping(0.4).mix(0.0), // Start dry (Off)
        );

        let sfx_track = manager
            .add_sub_track(sfx_track_builder)
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        // 3. Create Voice Track
        let voice_track = manager
            .add_sub_track(TrackBuilder::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        // 4. Create Spatial Audio Scene for 3D Positional Effects
        let mut spatial_scene = manager
            .add_spatial_scene(SpatialSceneSettings::default())
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        // Add default listener at origin facing -Z
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

        Ok(Self {
            manager: Some(manager),
            bgm_track: Some(bgm_track),
            fmv_track: Some(fmv_track),
            sfx_track: Some(sfx_track),
            voice_track: Some(voice_track),
            spatial_scene: Some(spatial_scene),
            listener: Some(listener),
            reverb_handle: Some(reverb_handle),
            current_preset: EnvironmentalPreset::Off,
            active_bgm_handle: None,
            active_fmv_handle: None,
            active_voice_handle: None,
            active_dialogue: None,
            master_volume: 1.0,
            bgm_volume: 0.85,
            sfx_volume: 0.9,
            voice_volume: 1.0,
            ducking_attenuation: 0.35,
        })
    }

    /// Initializes the Kira audio manager with dedicated tracks, spatial scene, and DSP effects using the default output device.
    pub fn new() -> Result<Self, AudioError> {
        Self::new_with_device(None)
    }

    /// Checks if the audio backend was successfully initialized and active.
    pub fn is_active(&self) -> bool {
        self.manager.is_some()
    }

    /// Updates listener position and orientation for 3D positional audio.
    ///
    /// - `player_pos`: Character or listener position in 3D world space.
    /// - `forward`: Normalized forward camera view vector.
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

            // Compute yaw orientation from forward vector (ignoring pitch/roll for ear panning)
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
        self.current_preset = preset;
        if let Some(ref mut reverb) = self.reverb_handle {
            if let Some((feedback, damping, mix)) = preset.reverb_params() {
                reverb.set_feedback(
                    feedback,
                    Tween {
                        duration: Duration::from_millis(150),
                        easing: Easing::Linear,
                        ..Default::default()
                    },
                );
                reverb.set_damping(
                    damping,
                    Tween {
                        duration: Duration::from_millis(150),
                        easing: Easing::Linear,
                        ..Default::default()
                    },
                );
                reverb.set_mix(
                    mix,
                    Tween {
                        duration: Duration::from_millis(150),
                        easing: Easing::Linear,
                        ..Default::default()
                    },
                );
            } else {
                reverb.set_mix(
                    0.0,
                    Tween {
                        duration: Duration::from_millis(150),
                        easing: Easing::Linear,
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Current active environmental preset.
    pub fn current_preset(&self) -> EnvironmentalPreset {
        self.current_preset
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

        let frames = wav_entry_to_frames(wav);
        let mut settings = StaticSoundSettings::new()
            .output_destination(sfx_track)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ));

        if wav.loops {
            settings = settings.loop_region(Region {
                start: PlaybackPosition::Seconds(wav.loop_start as f64 / wav.sample_rate as f64),
                end: if wav.loop_end > 0 {
                    EndPosition::Custom(PlaybackPosition::Seconds(
                        wav.loop_end as f64 / wav.sample_rate as f64,
                    ))
                } else {
                    EndPosition::EndOfAudio
                },
            });
        }

        let sound_data = StaticSoundData {
            sample_rate: wav.sample_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Plays a 3D spatialized sound effect at a world-space position.
    ///
    /// Applies distance attenuation curve matching Silent Hill 2:
    /// - Audible within 1500 to 11500 world units.
    /// - Full volume at <= 1500 units, attenuating linearly to zero at 11500 units.
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

        let emitter_settings = EmitterSettings::new()
            .distances(EmitterDistances {
                min_distance: 15.0, // Scaled world units
                max_distance: 115.0,
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

        let frames = wav_entry_to_frames(wav);
        let mut settings = StaticSoundSettings::new()
            .output_destination(&emitter)
            .volume(Volume::Amplitude(
                (volume * self.sfx_volume * self.master_volume) as f64,
            ));

        if wav.loops {
            settings = settings.loop_region(Region {
                start: PlaybackPosition::Seconds(wav.loop_start as f64 / wav.sample_rate as f64),
                end: if wav.loop_end > 0 {
                    EndPosition::Custom(PlaybackPosition::Seconds(
                        wav.loop_end as f64 / wav.sample_rate as f64,
                    ))
                } else {
                    EndPosition::EndOfAudio
                },
            });
        }

        let sound_data = StaticSoundData {
            sample_rate: wav.sample_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))
    }

    /// Streams a background music or ambient track from a decoded ADX stream.
    ///
    /// Automatically applies seamless looping if specified in the ADX metadata header.
    pub fn play_bgm(&mut self, adx: &DecodedAdx, volume: f32) -> Result<(), AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let bgm_track = self
            .bgm_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No bgm track".into()))?;

        // Stop existing BGM stream with smooth fade
        if let Some(mut old_handle) = self.active_bgm_handle.take() {
            old_handle.stop(Tween {
                duration: Duration::from_millis(300),
                easing: Easing::Linear,
                ..Default::default()
            });
        }

        let frames: Vec<Frame> = adx.frames.iter().map(|&(l, r)| Frame::new(l, r)).collect();
        let mut settings = StaticSoundSettings::new()
            .output_destination(bgm_track)
            .volume(Volume::Amplitude(
                (volume * self.bgm_volume * self.master_volume) as f64,
            ))
            .fade_in_tween(Some(Tween {
                duration: Duration::from_millis(250),
                easing: Easing::Linear,
                ..Default::default()
            }));

        if adx.header.has_loop {
            let start_sec = adx.header.loop_start_sample as f64 / adx.header.sample_rate as f64;
            let end_sec = adx.header.loop_end_sample as f64 / adx.header.sample_rate as f64;
            settings = settings.loop_region(Region {
                start: PlaybackPosition::Seconds(start_sec),
                end: EndPosition::Custom(PlaybackPosition::Seconds(end_sec)),
            });
        }

        let sound_data = StaticSoundData {
            sample_rate: adx.header.sample_rate,
            frames: Arc::from(frames),
            settings,
            slice: None,
        };

        let handle = manager
            .play(sound_data)
            .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
        self.active_bgm_handle = Some(handle);
        Ok(())
    }

    /// Plays decoded ADX audio stream with explicit master volume scaling and optional fade-in.
    pub fn play_bgm_with_fade(
        &mut self,
        adx: &DecodedAdx,
        master_volume_factor: f32,
        fade_in: Duration,
    ) -> Result<(), AudioError> {
        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let bgm_track = self
            .bgm_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No bgm track".into()))?;

        if let Some(mut old_handle) = self.active_bgm_handle.take() {
            old_handle.stop(Tween {
                duration: Duration::from_millis(50),
                easing: Easing::Linear,
                ..Default::default()
            });
        }

        let frames: Vec<Frame> = adx.frames.iter().map(|&(l, r)| Frame::new(l, r)).collect();
        let target_vol = (master_volume_factor * self.bgm_volume * self.master_volume) as f64;
        let mut settings = StaticSoundSettings::new().output_destination(bgm_track);

        if fade_in.is_zero() {
            settings = settings.volume(Volume::Amplitude(target_vol));
        } else {
            settings = settings.volume(Volume::Amplitude(0.0));
        }

        if adx.header.has_loop {
            let start_sec = adx.header.loop_start_sample as f64 / adx.header.sample_rate as f64;
            let end_sec = adx.header.loop_end_sample as f64 / adx.header.sample_rate as f64;
            settings = settings.loop_region(Region {
                start: PlaybackPosition::Seconds(start_sec),
                end: EndPosition::Custom(PlaybackPosition::Seconds(end_sec)),
            });
        }

        let sound_data = StaticSoundData {
            sample_rate: adx.header.sample_rate,
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

    /// Returns whether background music is currently actively playing.
    pub fn is_bgm_playing(&self) -> bool {
        self.active_bgm_handle.is_some()
    }

    /// Stops currently playing background music with a fade out.
    pub fn stop_bgm(&mut self, fade_out: Duration) {
        if let Some(mut handle) = self.active_bgm_handle.take() {
            handle.stop(Tween {
                duration: fade_out,
                easing: Easing::Linear,
                ..Default::default()
            });
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
        if let Some(ref mut handle) = self.active_fmv_handle {
            if paused {
                handle.pause(tween);
            } else {
                handle.resume(tween);
            }
        }
    }

    /// Triggers a dialogue voice clip synchronized with `.mes` subtitles and volume ducking.
    ///
    /// Automatically ducks BGM volume to `ducking_attenuation` during speech.
    pub fn play_dialogue(
        &mut self,
        voice_adx: &DecodedAdx,
        message: DialogueMessage,
        voice_id: Option<u32>,
    ) -> Result<(), AudioError> {
        // 1. Duck BGM Volume
        self.apply_ducking(true);

        // 2. Play voice audio
        let voice_track = self
            .voice_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No voice track".into()))?;
        let frames: Vec<Frame> = voice_adx
            .frames
            .iter()
            .map(|&(l, r)| Frame::new(l, r))
            .collect();
        let total_samples = voice_adx.frames.len();
        let duration_secs = total_samples as f64 / voice_adx.header.sample_rate as f64;
        let total_duration = Duration::from_secs_f64(duration_secs);

        let settings = StaticSoundSettings::new()
            .output_destination(voice_track)
            .volume(Volume::Amplitude(
                (self.voice_volume * self.master_volume) as f64,
            ));

        let sound_data = StaticSoundData {
            sample_rate: voice_adx.header.sample_rate,
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
                // Restore un-ducked BGM volume
                self.apply_ducking(false);
            }
        }

        // Clean up finished dialogue
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

/// Converts uncompressed 16-bit PCM bytes from a `WavEntry` into Kira `Frame` samples.
fn wav_entry_to_frames(wav: &WavEntry) -> Vec<Frame> {
    let channels = wav.channels.max(1) as usize;
    let byte_count = wav.data.len();
    let num_samples = byte_count / 2;
    let num_frames = num_samples / channels;

    let mut frames = Vec::with_capacity(num_frames);

    for f in 0..num_frames {
        let left_byte_idx = f * channels * 2;
        let left_val = if left_byte_idx + 2 <= byte_count {
            i16::from_le_bytes([wav.data[left_byte_idx], wav.data[left_byte_idx + 1]]) as f32
                / 32768.0
        } else {
            0.0
        };

        let right_val = if channels > 1 {
            let right_byte_idx = left_byte_idx + 2;
            if right_byte_idx + 2 <= byte_count {
                i16::from_le_bytes([wav.data[right_byte_idx], wav.data[right_byte_idx + 1]]) as f32
                    / 32768.0
            } else {
                left_val
            }
        } else {
            left_val
        };

        frames.push(Frame::new(left_val, right_val));
    }

    frames
}
