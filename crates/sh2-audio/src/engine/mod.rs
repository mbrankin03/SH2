//! Audio engine wrapper around `kira` for music, 3D sound effects, and dialogue voice.
//!
//! # Architecture & Sound Pipeline (`SH2-55` & `SH2-84`)
//! - **Engine Submodules**:
//!   - [`sfx`]: 2D, classic panned, Kira spatial, and split dry/wet `.cld`-occluded + Binaural HRTF 3D SFX voices.
//!   - [`bgm_fmv`]: Single-stream ADX, 7-layer AIX interactive BGM, and `.bik` FMV PCM playback.
//!   - [`voice`]: `.mes`-synchronized dialogue, `voice.afs` cutscene streams, and automatic BGM ducking.
//! - **DSP Subsystems**:
//!   - [`crate::acoustics`]: Dynamic room acoustics and reverb modeling from `.cam` volume properties (`rd_type`, `area_size_type`).
//!   - [`crate::occlusion`]: `.cld` 3D raycast wall/door/slab occlusion with `100 ms` click-free TPT SVF smoothing (`SH2-84`).
//!   - [`crate::hrtf`]: Zero-allocation Woodworth spherical-head ITD, head-shadow ILD, and pinna elevation cues (`SH2-84`).
//!   - [`crate::spatial_voice`]: Lock-free real-time 3D SFX voice synthesis (`SH2-49` zero-allocation callback compliant).

mod bgm_fmv;
mod radio;
mod sfx;
mod voice;

pub use radio::{radio_driver_to_volume_pan, RADIO_STATIC_SDDATA_INDICES};

use glam::Vec3;
use kira::{
    effect::reverb::{ReverbBuilder, ReverbHandle},
    manager::{
        backend::{cpal::CpalBackendSettings, DefaultBackend},
        AudioManager, AudioManagerSettings,
    },
    sound::static_sound::StaticSoundHandle,
    spatial::{
        listener::{ListenerHandle, ListenerSettings},
        scene::{SpatialSceneHandle, SpatialSceneSettings},
    },
    track::{TrackBuilder, TrackHandle},
    tween::{Easing, Tween},
    Volume,
};
use mint::{Quaternion, Vector3};
use sh2_assets::{CamFile, RoadRecord};
use sh2_core::SpatialAudioMode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub use crate::acoustics::{
    evaluate_room_acoustics_for_pos_gated, evaluate_room_acoustics_gated, EnvironmentalPreset,
    RoomAcousticsProfile,
};
pub use crate::dialogue::ActiveDialogue;
pub use crate::dsp_resample::{adx_to_frames_48k, wav_entry_to_frames, wav_entry_to_frames_48k};
pub use crate::looping::LoopRegionSamples;
pub use crate::ringbuffer::{AudioRingBuffer, DEFAULT_RINGBUFFER_FRAMES};

/// Error type for audio playback and management.
#[derive(Error, Debug)]
pub enum AudioError {
    /// Audio hardware backend failed to initialize.
    #[error("Audio backend initialization failed: {0}")]
    InitError(String),

    /// Kira track, emitter, or sound capacity was exceeded.
    #[error("Resource limit reached: {0}")]
    ResourceLimit(String),

    /// Sound playback submission failed.
    #[error("Playback failed: {0}")]
    PlaybackError(String),

    /// Audio asset decoding or validation failed.
    #[error("Asset error: {0}")]
    AssetError(String),
}

/// Master audio engine managing streaming BGM, 3D sound effects, environmental DSP, and dialogue.
pub struct AudioEngine {
    pub(super) manager: Option<AudioManager<DefaultBackend>>,

    pub(super) bgm_track: Option<TrackHandle>,
    pub(super) fmv_track: Option<TrackHandle>,
    pub(super) sfx_track: Option<TrackHandle>,
    pub(super) sfx_dry_track: Option<TrackHandle>,
    pub(super) sfx_reverb_bus: Option<TrackHandle>,
    pub(super) voice_track: Option<TrackHandle>,

    pub(super) spatial_scene: Option<SpatialSceneHandle>,
    pub(super) listener: Option<ListenerHandle>,

    pub(super) reverb_handle: Option<ReverbHandle>,
    pub(super) spatial_reverb_handle: Option<ReverbHandle>,
    pub(super) current_preset: EnvironmentalPreset,
    pub(super) current_acoustics: RoomAcousticsProfile,
    pub(super) spatial_audio_mode: SpatialAudioMode,
    pub(super) binaural_hrtf_flag: Arc<AtomicBool>,
    pub(super) acoustic_occlusion_enabled: bool,
    pub(super) occlusion_enabled_flag: Arc<AtomicBool>,
    pub(super) dynamic_room_acoustics_enabled: bool,

    pub(super) output_ringbuffer: Arc<AudioRingBuffer>,

    pub(super) active_bgm_handle: Option<StaticSoundHandle>,
    pub(super) active_bgm_layers: Vec<StaticSoundHandle>,
    pub(super) active_radio_tracks: [Option<StaticSoundHandle>; 4],
    pub(super) active_fmv_handle: Option<StaticSoundHandle>,
    pub(super) active_voice_handle: Option<StaticSoundHandle>,
    pub(super) active_dialogue: Option<ActiveDialogue>,

    pub(super) master_volume: f32,
    pub(super) bgm_volume: f32,
    pub(super) sfx_volume: f32,
    pub(super) voice_volume: f32,
}

impl AudioEngine {
    /// Enumerates all currently available audio output device names on the host system.
    pub fn enumerate_output_devices() -> Vec<String> {
        crate::device_routing::enumerate_output_devices()
    }

    /// Initializes the Kira audio manager with dedicated tracks, spatial scene, and DSP effects.
    pub fn new_with_device(device_name: Option<&str>) -> Result<Self, AudioError> {
        const DEFAULT_MASTER_VOL: f32 = 1.0;
        const DEFAULT_BGM_VOL: f32 = 0.85;
        const DEFAULT_SFX_VOL: f32 = 0.9;
        const DEFAULT_VOICE_VOL: f32 = 1.0;

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

        // Initialize `sfx_dry_track` to `(DEFAULT_SFX_VOL * DEFAULT_MASTER_VOL)` so 3D spatial voices
        // apply master/SFX slider scaling exactly once via the Kira sub-track.
        let sfx_dry_track = manager
            .add_sub_track(TrackBuilder::new().volume(Volume::Amplitude(
                (DEFAULT_SFX_VOL * DEFAULT_MASTER_VOL) as f64,
            )))
            .map_err(|e| AudioError::ResourceLimit(e.to_string()))?;

        let mut reverb_bus_builder = TrackBuilder::new().volume(Volume::Amplitude(0.0));
        let spatial_reverb_handle =
            reverb_bus_builder.add_effect(ReverbBuilder::new().feedback(0.5).damping(0.4).mix(1.0));
        let sfx_reverb_bus = manager
            .add_sub_track(reverb_bus_builder)
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
            sfx_dry_track: Some(sfx_dry_track),
            sfx_reverb_bus: Some(sfx_reverb_bus),
            voice_track: Some(voice_track),
            spatial_scene: Some(spatial_scene),
            listener: Some(listener),
            reverb_handle: Some(reverb_handle),
            spatial_reverb_handle: Some(spatial_reverb_handle),
            current_preset: EnvironmentalPreset::Off,
            current_acoustics: RoomAcousticsProfile::from_preset(EnvironmentalPreset::Off),
            spatial_audio_mode: SpatialAudioMode::ClassicPs2Stereo,
            binaural_hrtf_flag: Arc::new(AtomicBool::new(false)),
            acoustic_occlusion_enabled: false,
            occlusion_enabled_flag: Arc::new(AtomicBool::new(false)),
            dynamic_room_acoustics_enabled: false,
            output_ringbuffer: Arc::new(AudioRingBuffer::with_capacity(DEFAULT_RINGBUFFER_FRAMES)),
            active_bgm_handle: None,
            active_bgm_layers: Vec::new(),
            active_radio_tracks: [None, None, None, None],
            active_fmv_handle: None,
            active_voice_handle: None,
            active_dialogue: None,
            master_volume: DEFAULT_MASTER_VOL,
            bgm_volume: DEFAULT_BGM_VOL,
            sfx_volume: DEFAULT_SFX_VOL,
            voice_volume: DEFAULT_VOICE_VOL,
        };

        if let Some(target) = clean_device_name {
            engine.migrate_stream_to_device(Some(target));
        }

        Ok(engine)
    }

    /// Initializes the Kira audio manager using the default host output device.
    pub fn new() -> Result<Self, AudioError> {
        Self::new_with_device(None)
    }

    /// Returns `true` if the audio backend was successfully initialized and is active.
    pub fn is_active(&self) -> bool {
        self.manager.is_some()
    }

    /// Returns a handle to the engine's lock-free zero-allocation SPSC output ringbuffer (`SH2-55`).
    pub fn output_ringbuffer(&self) -> &Arc<AudioRingBuffer> {
        &self.output_ringbuffer
    }

    /// Updates Kira spatial scene listener position (SH2 world units * `0.01`) and forward orientation.
    pub fn set_listener_transform(&mut self, player_pos: Vec3, forward: Vec3) {
        if let Some(ref mut listener) = self.listener {
            listener.set_position(
                Vector3 {
                    x: player_pos.x * 0.01,
                    y: player_pos.y * 0.01,
                    z: player_pos.z * 0.01,
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
    /// or a discrete preset) to both the 2D SFX track and the 100%-wet 3D corridor reverb bus (`SH2-55` & `SH2-84`).
    pub fn apply_acoustics_profile(&mut self, profile: RoomAcousticsProfile) {
        if profile == self.current_acoustics {
            return;
        }
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
        if let Some(ref mut spatial_reverb) = self.spatial_reverb_handle {
            spatial_reverb.set_feedback(profile.feedback, tween);
            spatial_reverb.set_damping(profile.damping, tween);
        }
        if let Some(ref mut reverb_bus) = self.sfx_reverb_bus {
            let wet_gain = if profile.wet_mix > 0.0 {
                profile.wet_mix.clamp(0.0, 1.0).sqrt()
                    * (self.sfx_volume * self.master_volume) as f64
            } else {
                0.0
            };
            reverb_bus.set_volume(Volume::Amplitude(wet_gain), tween);
        }
    }

    /// Sets the active 3D SFX spatialization mode (`ClassicPs2Stereo` vs `BinauralHrtf`),
    /// atomically switching all currently playing 3D SFX voices without heap allocation (`SH2-84`).
    pub fn set_spatial_audio_mode(&mut self, mode: SpatialAudioMode) {
        self.spatial_audio_mode = mode;
        self.binaural_hrtf_flag
            .store(mode.is_binaural_hrtf(), Ordering::Relaxed);
    }

    /// Returns the active 3D SFX [`SpatialAudioMode`] (`SH2-84`).
    pub fn spatial_audio_mode(&self) -> SpatialAudioMode {
        self.spatial_audio_mode
    }

    /// Returns a cloneable reference to the engine's lock-free Binaural HRTF toggle flag (`SH2-84`).
    pub fn binaural_hrtf_flag(&self) -> &Arc<AtomicBool> {
        &self.binaural_hrtf_flag
    }

    /// Enables or disables real-time `.cld` raycast acoustic wall/door/slab occlusion (`SH2-84`),
    /// atomically ramping all active 3D SFX voices over `100 ms` without clicks or heap allocation.
    pub fn set_acoustic_occlusion_enabled(&mut self, enabled: bool) {
        self.acoustic_occlusion_enabled = enabled;
        self.occlusion_enabled_flag
            .store(enabled, Ordering::Relaxed);
    }

    /// Returns `true` when `.cld` acoustic wall/door/slab occlusion is enabled (`false` = strict 1:1 PS2 vanilla).
    pub fn acoustic_occlusion_enabled(&self) -> bool {
        self.acoustic_occlusion_enabled
    }

    /// Returns a cloneable reference to the engine's lock-free `.cld` acoustic occlusion toggle flag (`SH2-84`).
    pub fn occlusion_enabled_flag(&self) -> &Arc<AtomicBool> {
        &self.occlusion_enabled_flag
    }

    /// Enables or disables dynamic `.cam`-aspect room reverb heuristics (`SH2-137` `native-upgrade`).
    /// Disabling immediately resets the reverb bus to dry [`EnvironmentalPreset::Off`].
    pub fn set_dynamic_room_acoustics_enabled(&mut self, enabled: bool) {
        self.dynamic_room_acoustics_enabled = enabled;
        if !enabled {
            self.apply_acoustics_profile(RoomAcousticsProfile::from_preset(
                EnvironmentalPreset::Off,
            ));
        }
    }

    /// Returns `true` when dynamic `.cam`-aspect room reverb heuristics are enabled (`false` = strict 1:1 PS2 vanilla).
    pub fn dynamic_room_acoustics_enabled(&self) -> bool {
        self.dynamic_room_acoustics_enabled
    }

    /// Evaluates and applies dynamic room acoustics from the active `.cam` [`RoadRecord`] at `player_pos` (`SH2-55`),
    /// gated by `self.dynamic_room_acoustics_enabled` (`SH2-137`).
    pub fn update_room_acoustics_from_cam(
        &mut self,
        cam: &CamFile,
        player_pos: Vec3,
        is_interior_map: bool,
    ) -> RoomAcousticsProfile {
        let profile = evaluate_room_acoustics_for_pos_gated(
            cam,
            player_pos,
            is_interior_map,
            self.dynamic_room_acoustics_enabled,
        );
        self.apply_acoustics_profile(profile);
        profile
    }

    /// Evaluates and applies dynamic room acoustics directly from an active [`RoadRecord`] (`SH2-55`),
    /// gated by `self.dynamic_room_acoustics_enabled` (`SH2-137`).
    pub fn update_room_acoustics_from_record(
        &mut self,
        record: Option<&RoadRecord>,
        is_interior_map: bool,
    ) -> RoomAcousticsProfile {
        let profile = evaluate_room_acoustics_gated(
            record,
            is_interior_map,
            self.dynamic_room_acoustics_enabled,
        );
        self.apply_acoustics_profile(profile);
        profile
    }

    /// Returns the current active discrete environmental preset.
    pub fn current_preset(&self) -> EnvironmentalPreset {
        self.current_preset
    }

    /// Returns the current active continuous [`RoomAcousticsProfile`] (`SH2-55`).
    pub fn current_acoustics(&self) -> RoomAcousticsProfile {
        self.current_acoustics
    }

    /// Sets the master volume level in `[0.0, 1.0]` and refreshes BGM, SFX, and Dialogue sub-tracks.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
        let bgm_v = self.bgm_volume;
        let sfx_v = self.sfx_volume;
        let voice_v = self.voice_volume;
        self.set_bgm_volume(bgm_v);
        self.set_sfx_volume(sfx_v);
        self.set_voice_volume(voice_v);
    }

    /// Migrates the active running game audio stream to the requested device in real-time.
    pub fn migrate_stream_to_device(&mut self, device_name: Option<&str>) {
        crate::device_routing::migrate_stream_to_device(device_name);
    }

    /// Sets the background music volume level in `[0.0, 1.0]`.
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

    /// Sets the sound effects volume level in `[0.0, 1.0]` across 2D SFX, dry 3D SFX, and wet reverb bus.
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
        let tween = Tween {
            duration: Duration::from_millis(50),
            easing: Easing::Linear,
            ..Default::default()
        };
        let amp = (self.sfx_volume * self.master_volume) as f64;
        if let Some(ref mut track) = self.sfx_track {
            track.set_volume(Volume::Amplitude(amp), tween);
        }
        if let Some(ref mut dry_track) = self.sfx_dry_track {
            dry_track.set_volume(Volume::Amplitude(amp), tween);
        }
        if let Some(ref mut reverb_bus) = self.sfx_reverb_bus {
            let wet = if self.current_acoustics.wet_mix > 0.0 {
                self.current_acoustics.wet_mix.clamp(0.0, 1.0).sqrt() * amp
            } else {
                0.0
            };
            reverb_bus.set_volume(Volume::Amplitude(wet), tween);
        }
    }

    /// Sets the dialogue voice volume level in `[0.0, 1.0]`.
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

    /// Sets the environmental DSP reverb mix ratio in `[0.0, 1.0]`.
    pub fn set_reverb_mix(&mut self, mix: f32) {
        let clamped = mix.clamp(0.0, 1.0) as f64;
        let tween = Tween {
            duration: Duration::from_millis(50),
            easing: Easing::Linear,
            ..Default::default()
        };
        if let Some(ref mut reverb) = self.reverb_handle {
            reverb.set_mix(clamped, tween);
        }
        if let Some(ref mut reverb_bus) = self.sfx_reverb_bus {
            let wet = clamped.sqrt() * (self.sfx_volume * self.master_volume) as f64;
            reverb_bus.set_volume(Volume::Amplitude(wet), tween);
        }
    }
}
