//! Lock-free, zero-allocation 3D SFX voice synthesis (`SpatialDrySound` & `SpatialWetSendSound`) (`SH2-84`).
//!
//! Splits every 3D point-source sound effect into two real-time Kira `Sound` streams sharing a
//! lock-free [`SpatialVoiceControl`] atomic parameter block:
//! 1. **Dry Direct Path (`SpatialDrySoundData`)**:
//!    - Dynamically applies `.cld` raycast wall/door low-pass occlusion (`~650–850 Hz` smoothed over `100 ms`)
//!      and direct-path attenuation (`-6 dB` to `-10 dB`) when `occlusion_enabled_flag` is `true`
//!      (or transparent `20 kHz` / `0 dB` when `false` for strict 1:1 original PS2 behavior).
//!    - Spatializes via either **Binaural HRTF** (ITD + head-shadow ILD + pinna elevation/front-back cues)
//!      or **Classic PS2 Stereo** (`SeCallPosDistanceF` `0x00192bc0` / `0x5155E0` azimuth panning),
//!      switchable live via atomic flag.
//! 2. **Wet Corridor Reverb Send (`SpatialWetSendSoundData`)**:
//!    - Feeds the dedicated 100%-wet `RoomAcousticsProfile` (`SH2-55`) reverb bus via lightweight
//!      [`SpatialWetSendCore`] without allocating unused occlusion SVF or HRTF delay buffers.
//!
//! Strictly enforces `SH2-49` real-time callback compliance: zero heap allocations, zero mutexes,
//! and zero deallocations inside `Sound::on_start_processing` and `Sound::process`.

mod dsp_core;
mod kira_sounds;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use sh2_core::SpatialAudioMode;

use crate::hrtf::HrtfSphericalAngles;
use crate::occlusion::{OcclusionDspParams, UNOCCLUDED_CUTOFF_HZ};

pub use dsp_core::{SpatialDryWetCore, SpatialWetSendCore};
pub use kira_sounds::{SpatialDrySoundData, SpatialWetSendSoundData};

#[inline]
pub(super) fn sanitize_f32(val: f32, fallback: f32) -> f32 {
    if val.is_finite() {
        val
    } else {
        fallback
    }
}

#[inline]
pub(super) fn store_f32(slot: &AtomicU32, val: f32, fallback: f32) {
    slot.store(sanitize_f32(val, fallback).to_bits(), Ordering::Relaxed);
}

#[inline]
pub(super) fn load_f32(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

/// Initial spatial, distance, and `.cld` occlusion parameters for spawning a 3D SFX voice (`SH2-84`).
#[derive(Debug, Clone, Copy)]
pub struct SpatialVoiceInitParams {
    /// Distance-attenuated linear amplitude gain (`SeCallPosDistanceF` `effective_gain` in `[0.0, 1.0]`;
    /// master/SFX slider scaling is applied once on Kira's `sfx_dry_track` / `sfx_reverb_bus`).
    pub base_distance_gain: f32,
    /// Classic PS2 horizontal stereo pan in `[-1.0, 1.0]` (`+1.0` = right ear, `-1.0` = left ear).
    pub stereo_pan: f32,
    /// Listener-relative 3D spherical angles (radians & seconds) for Binaural HRTF mode.
    pub angles: HrtfSphericalAngles,
    /// Evaluated `.cld` wall/door/slab raycast occlusion parameters (`Hz` & `dB`).
    pub occlusion: OcclusionDspParams,
}

/// Lock-free shared atomic control block between gameplay simulation and the real-time audio callback thread (`SH2-84`).
#[derive(Debug)]
pub struct SpatialVoiceControl {
    pub(super) base_distance_gain: AtomicU32,
    pub(super) stereo_pan: AtomicU32,
    pub(super) azimuth_rad: AtomicU32,
    pub(super) elevation_rad: AtomicU32,
    pub(super) occlusion_cutoff_hz: AtomicU32,
    pub(super) occlusion_dry_scale: AtomicU32,
    pub(super) wet_bleed_scale: AtomicU32,
    pub(super) paused: AtomicBool,
    pub(super) stop_requested: AtomicBool,
    pub(super) dry_finished: AtomicBool,
    pub(super) binaural_hrtf_flag: Arc<AtomicBool>,
    pub(super) occlusion_enabled_flag: Arc<AtomicBool>,
}

impl SpatialVoiceControl {
    /// Creates a lock-free control block initialized with `params`, linked to `binaural_hrtf_flag`,
    /// with `.cld` occlusion enabled by default (for standalone DSP unit tests).
    pub fn new(params: &SpatialVoiceInitParams, binaural_hrtf_flag: Arc<AtomicBool>) -> Arc<Self> {
        Self::new_with_flags(
            params,
            binaural_hrtf_flag,
            Arc::new(AtomicBool::new(true)),
            false,
        )
    }

    /// Creates a lock-free control block linked to both the engine's live `binaural_hrtf_flag` and
    /// `occlusion_enabled_flag`, marking `dry_finished` immediately if `frames_empty` is `true`.
    pub fn new_with_flags(
        params: &SpatialVoiceInitParams,
        binaural_hrtf_flag: Arc<AtomicBool>,
        occlusion_enabled_flag: Arc<AtomicBool>,
        frames_empty: bool,
    ) -> Arc<Self> {
        let base_gain = sanitize_f32(params.base_distance_gain, 0.0).max(0.0);
        let pan = sanitize_f32(params.stereo_pan, 0.0).clamp(-1.0, 1.0);
        let az = sanitize_f32(params.angles.azimuth_rad, 0.0);
        let el = sanitize_f32(params.angles.elevation_rad, 0.0);
        let fc = sanitize_f32(params.occlusion.dry_cutoff_hz, UNOCCLUDED_CUTOFF_HZ);
        let dry_scale = sanitize_f32(params.occlusion.dry_gain_scale, 1.0);
        let wet_scale = sanitize_f32(params.occlusion.wet_reverb_bleed_scale, 1.0);

        Arc::new(Self {
            base_distance_gain: AtomicU32::new(base_gain.to_bits()),
            stereo_pan: AtomicU32::new(pan.to_bits()),
            azimuth_rad: AtomicU32::new(az.to_bits()),
            elevation_rad: AtomicU32::new(el.to_bits()),
            occlusion_cutoff_hz: AtomicU32::new(fc.to_bits()),
            occlusion_dry_scale: AtomicU32::new(dry_scale.to_bits()),
            wet_bleed_scale: AtomicU32::new(wet_scale.to_bits()),
            paused: AtomicBool::new(false),
            stop_requested: AtomicBool::new(false),
            dry_finished: AtomicBool::new(frames_empty),
            binaural_hrtf_flag,
            occlusion_enabled_flag,
        })
    }

    /// Updates distance gain, stereo pan, HRTF angles, and `.cld` occlusion targets atomically from the main thread.
    #[inline]
    pub fn update_spatial_and_occlusion(&self, params: &SpatialVoiceInitParams) {
        let prev_az = load_f32(&self.azimuth_rad);
        store_f32(
            &self.base_distance_gain,
            sanitize_f32(params.base_distance_gain, 0.0).max(0.0),
            0.0,
        );
        store_f32(
            &self.stereo_pan,
            sanitize_f32(params.stereo_pan, 0.0).clamp(-1.0, 1.0),
            0.0,
        );
        store_f32(&self.azimuth_rad, params.angles.azimuth_rad, prev_az);
        store_f32(&self.elevation_rad, params.angles.elevation_rad, 0.0);
        store_f32(
            &self.occlusion_cutoff_hz,
            params.occlusion.dry_cutoff_hz,
            UNOCCLUDED_CUTOFF_HZ,
        );
        store_f32(
            &self.occlusion_dry_scale,
            params.occlusion.dry_gain_scale,
            1.0,
        );
        store_f32(
            &self.wet_bleed_scale,
            params.occlusion.wet_reverb_bleed_scale,
            1.0,
        );
    }

    /// Requests a smooth click-free fade-out and termination of both dry and wet streams.
    #[inline]
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Relaxed);
    }

    /// Pauses or resumes playback atomically.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Returns `true` if playback is currently paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Returns `true` if the voice has finished playing or was stopped.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.dry_finished.load(Ordering::Relaxed)
    }

    /// Returns the active [`SpatialAudioMode`] observed by this voice.
    #[inline]
    pub fn active_spatial_mode(&self) -> SpatialAudioMode {
        if self.binaural_hrtf_flag.load(Ordering::Relaxed) {
            SpatialAudioMode::BinauralHrtf
        } else {
            SpatialAudioMode::ClassicPs2Stereo
        }
    }

    /// Returns `true` when `.cld` acoustic wall/door occlusion DSP is enabled on this voice.
    #[inline]
    pub fn is_occlusion_enabled(&self) -> bool {
        self.occlusion_enabled_flag.load(Ordering::Relaxed)
    }
}

/// Handle returned to gameplay/bank code for controlling or stopping an active 3D SFX emitter (`SH2-84`).
#[derive(Debug, Clone)]
pub struct SpatialVoiceHandle {
    control: Arc<SpatialVoiceControl>,
}

impl SpatialVoiceHandle {
    /// Wraps a shared [`SpatialVoiceControl`] block.
    #[inline]
    pub fn new(control: Arc<SpatialVoiceControl>) -> Self {
        Self { control }
    }

    /// Borrows the underlying lock-free control block.
    #[inline]
    pub fn control(&self) -> &Arc<SpatialVoiceControl> {
        &self.control
    }

    /// Atomically updates spatialization and `.cld` occlusion targets.
    #[inline]
    pub fn update(&self, params: &SpatialVoiceInitParams) {
        self.control.update_spatial_and_occlusion(params);
    }

    /// Smoothly fades out and stops the voice.
    #[inline]
    pub fn stop(&self) {
        self.control.request_stop();
    }

    /// Pauses or resumes playback.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.control.set_paused(paused);
    }

    /// Returns `true` once playback has completed.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.control.is_finished()
    }
}
