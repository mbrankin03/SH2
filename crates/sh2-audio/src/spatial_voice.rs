//! Lock-free, zero-allocation 3D SFX voice synthesis (`SpatialDrySound` & `SpatialWetSendSound`) (`SH2-84`).
//!
//! Splits every 3D point-source sound effect into two real-time Kira [`Sound`] streams sharing a
//! lock-free [`SpatialVoiceControl`] atomic parameter block:
//! 1. **Dry Direct Path (`SpatialDrySound`)**:
//!    - Dynamically applies `.cld` raycast wall/door low-pass occlusion (`~650–850 Hz` smoothed over `100 ms`)
//!      and direct-path attenuation (`-6 dB` to `-10 dB`).
//!    - Spatializes via either **Binaural HRTF** (ITD + head-shadow ILD + pinna elevation/front-back cues)
//!      or **Classic PS2 Stereo** (`SeCallPosDistanceF` `0x517D10` azimuth panning), switchable live via atomic flag.
//! 2. **Wet Corridor Reverb Send (`SpatialWetSendSound`)**:
//!    - Feeds the dedicated 100%-wet `RoomAcousticsProfile` (`SH2-55`) reverb bus without dry wall muffling,
//!      so enemies behind apartment/hospital walls retain authentic corridor reverb bleed.
//!
//! Strictly enforces `SH2-49` real-time callback compliance: zero heap allocations, zero mutexes,
//! and zero deallocations inside [`Sound::on_start_processing`] and [`Sound::process`].

use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use kira::{
    clock::clock_info::ClockInfoProvider,
    modulator::value_provider::ModulatorValueProvider,
    sound::{Sound, SoundData},
    track::TrackId,
    Frame, OutputDestination,
};
use sh2_core::SpatialAudioMode;

use crate::hrtf::{BinauralHrtfProcessor, HrtfSphericalAngles};
use crate::looping::LoopRegionSamples;
use crate::occlusion::{OcclusionDspParams, OcclusionFilterState};

#[inline]
fn store_f32(slot: &AtomicU32, val: f32) {
    slot.store(val.to_bits(), Ordering::Relaxed);
}

#[inline]
fn load_f32(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

/// Initial spatial, distance, and `.cld` occlusion parameters for spawning a 3D SFX voice (`SH2-84`).
#[derive(Debug, Clone, Copy)]
pub struct SpatialVoiceInitParams {
    /// Distance-attenuated linear amplitude gain (`SeCallPosDistanceF` `effective_gain` * master/sfx gain).
    pub base_distance_gain: f32,
    /// Classic PS2 horizontal stereo pan in `[-1.0, 1.0]`.
    pub stereo_pan: f32,
    /// Listener-relative 3D spherical angles for Binaural HRTF mode.
    pub angles: HrtfSphericalAngles,
    /// Initial `.cld` wall/door raycast occlusion parameters.
    pub occlusion: OcclusionDspParams,
}

/// Lock-free shared atomic control block between gameplay simulation and the real-time audio callback thread (`SH2-84`).
#[derive(Debug)]
pub struct SpatialVoiceControl {
    base_distance_gain: AtomicU32,
    stereo_pan: AtomicU32,
    azimuth_rad: AtomicU32,
    elevation_rad: AtomicU32,
    occlusion_cutoff_hz: AtomicU32,
    occlusion_dry_scale: AtomicU32,
    wet_bleed_scale: AtomicU32,
    paused: AtomicBool,
    stop_requested: AtomicBool,
    dry_finished: AtomicBool,
    binaural_hrtf_flag: Arc<AtomicBool>,
}

impl SpatialVoiceControl {
    /// Creates a new lock-free control block initialized with `params` and linked to the engine's live HRTF flag.
    pub fn new(params: &SpatialVoiceInitParams, binaural_hrtf_flag: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new(Self {
            base_distance_gain: AtomicU32::new(params.base_distance_gain.max(0.0).to_bits()),
            stereo_pan: AtomicU32::new(params.stereo_pan.clamp(-1.0, 1.0).to_bits()),
            azimuth_rad: AtomicU32::new(params.angles.azimuth_rad.to_bits()),
            elevation_rad: AtomicU32::new(params.angles.elevation_rad.to_bits()),
            occlusion_cutoff_hz: AtomicU32::new(params.occlusion.dry_cutoff_hz.to_bits()),
            occlusion_dry_scale: AtomicU32::new(params.occlusion.dry_gain_scale.to_bits()),
            wet_bleed_scale: AtomicU32::new(params.occlusion.wet_reverb_bleed_scale.to_bits()),
            paused: AtomicBool::new(false),
            stop_requested: AtomicBool::new(false),
            dry_finished: AtomicBool::new(false),
            binaural_hrtf_flag,
        })
    }

    /// Updates distance gain, stereo pan, HRTF angles, and `.cld` occlusion targets atomically from the main thread.
    #[inline]
    pub fn update_spatial_and_occlusion(&self, params: &SpatialVoiceInitParams) {
        store_f32(&self.base_distance_gain, params.base_distance_gain.max(0.0));
        store_f32(&self.stereo_pan, params.stereo_pan.clamp(-1.0, 1.0));
        store_f32(
            &self.azimuth_rad,
            params.azimuth_rad_or(params.angles.azimuth_rad),
        );
        store_f32(&self.elevation_rad, params.angles.elevation_rad);
        store_f32(&self.occlusion_cutoff_hz, params.occlusion.dry_cutoff_hz);
        store_f32(&self.occlusion_dry_scale, params.occlusion.dry_gain_scale);
        store_f32(
            &self.wet_bleed_scale,
            params.occlusion.wet_reverb_bleed_scale,
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
}

impl SpatialVoiceInitParams {
    #[inline]
    fn azimuth_rad_or(&self, fallback: f32) -> f32 {
        if self.angles.azimuth_rad.is_finite() {
            self.angles.azimuth_rad
        } else {
            fallback
        }
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

/// Pure, zero-allocation real-time 3D SFX DSP core usable both inside Kira's audio thread
/// and directly in deterministic / zero-allocation offline unit tests (`SH2-84` & `SH2-49`).
#[derive(Debug, Clone)]
pub struct SpatialDryWetCore {
    frames: Arc<[Frame]>,
    sample_rate: u32,
    loop_region: Option<LoopRegionSamples>,
    cursor: f64,
    smoothed_base_gain: f32,
    smoothed_pan: f32,
    smoothed_wet_bleed: f32,
    stop_fade_gain: f32,
    finished: bool,
    pub occlusion_filter: OcclusionFilterState,
    pub hrtf: BinauralHrtfProcessor,
}

impl SpatialDryWetCore {
    /// Initializes a zero-allocation DSP core snapped immediately to `init` parameters.
    pub fn new(
        frames: Arc<[Frame]>,
        sample_rate: u32,
        loop_region: Option<LoopRegionSamples>,
        init: &SpatialVoiceInitParams,
    ) -> Self {
        let fs = sample_rate.max(8_000) as f32;
        let mut occlusion_filter =
            OcclusionFilterState::new(init.occlusion.dry_cutoff_hz, init.occlusion.dry_gain_scale);
        occlusion_filter.snap_to(
            init.occlusion.dry_cutoff_hz,
            init.occlusion.dry_gain_scale,
            fs,
        );
        let mut hrtf = BinauralHrtfProcessor::new();
        hrtf.update_angles(init.angles.azimuth_rad, init.angles.elevation_rad, fs, true);

        let empty = frames.is_empty();
        Self {
            frames,
            sample_rate: sample_rate.max(8_000),
            loop_region,
            cursor: 0.0,
            smoothed_base_gain: init.base_distance_gain.max(0.0),
            smoothed_pan: init.stereo_pan.clamp(-1.0, 1.0),
            smoothed_wet_bleed: init.occlusion.wet_reverb_bleed_scale.clamp(0.0, 1.5),
            stop_fade_gain: 1.0,
            finished: empty,
            occlusion_filter,
            hrtf,
        }
    }

    /// Reads the next source frame at `dt` seconds with linear interpolation and sample-accurate looping.
    #[inline]
    fn next_source_frame(&mut self, dt: f64) -> Option<Frame> {
        let len = self.frames.len();
        if self.finished || len == 0 {
            self.finished = true;
            return None;
        }

        if let Some(region) = self.loop_region {
            let loop_end = region.loop_end_frame.min(len);
            let loop_start = region.loop_start_frame.min(loop_end.saturating_sub(1));
            if loop_end > loop_start && self.cursor >= loop_end as f64 {
                let span = (loop_end - loop_start) as f64;
                self.cursor = loop_start as f64 + (self.cursor - loop_start as f64) % span;
            }
        } else if self.cursor >= len as f64 {
            self.finished = true;
            return None;
        }

        let i0 = (self.cursor as usize).min(len - 1);
        let frac = (self.cursor - (i0 as f64)) as f32;
        let i1 = if let Some(region) = self.loop_region {
            let loop_end = region.loop_end_frame.min(len);
            if i0 + 1 >= loop_end && loop_end > region.loop_start_frame {
                region.loop_start_frame.min(len - 1)
            } else {
                (i0 + 1).min(len - 1)
            }
        } else {
            (i0 + 1).min(len - 1)
        };

        let f0 = self.frames[i0];
        let f1 = self.frames[i1];
        let sample = Frame::new(
            f0.left + (f1.left - f0.left) * frac,
            f0.right + (f1.right - f0.right) * frac,
        );

        self.cursor += dt * (self.sample_rate as f64);
        Some(sample)
    }

    /// Synchronizes targets from `control` at the start of an audio callback block (zero allocations).
    #[inline]
    pub fn sync_from_control(&mut self, control: &SpatialVoiceControl, output_sample_rate: f32) {
        let cutoff = load_f32(&control.occlusion_cutoff_hz);
        let dry_scale = load_f32(&control.occlusion_dry_scale);
        self.occlusion_filter.set_target(cutoff, dry_scale);

        let az = load_f32(&control.azimuth_rad);
        let el = load_f32(&control.elevation_rad);
        self.hrtf.update_angles(az, el, output_sample_rate, false);
    }

    /// Synthesizes one `(dry_frame, wet_reverb_send_frame)` pair on the real-time callback thread
    /// with strictly zero heap allocations (`SH2-49`).
    #[inline]
    pub fn process_pair(&mut self, dt: f64, control: &SpatialVoiceControl) -> (Frame, Frame) {
        if control.paused.load(Ordering::Relaxed) {
            return (Frame::ZERO, Frame::ZERO);
        }

        if control.stop_requested.load(Ordering::Relaxed) {
            // Smooth ~20 ms fade-out before marking finished to avoid stop clicks
            let fade_step = (dt as f32 * 50.0).clamp(0.0005, 0.25);
            self.stop_fade_gain = (self.stop_fade_gain - fade_step).max(0.0);
            if self.stop_fade_gain <= 1e-4 {
                self.finished = true;
                control.dry_finished.store(true, Ordering::Relaxed);
                return (Frame::ZERO, Frame::ZERO);
            }
        }

        let Some(raw) = self.next_source_frame(dt) else {
            control.dry_finished.store(true, Ordering::Relaxed);
            return (Frame::ZERO, Frame::ZERO);
        };

        let out_fs = if dt > 1e-7 {
            (1.0 / dt) as f32
        } else {
            self.sample_rate as f32
        };

        let target_gain = load_f32(&control.base_distance_gain);
        let target_pan = load_f32(&control.stereo_pan);
        let target_wet = load_f32(&control.wet_bleed_scale);

        const GAIN_SMOOTH: f32 = 0.004;
        self.smoothed_base_gain += (target_gain - self.smoothed_base_gain) * GAIN_SMOOTH;
        self.smoothed_pan += (target_pan - self.smoothed_pan) * GAIN_SMOOTH;
        self.smoothed_wet_bleed += (target_wet - self.smoothed_wet_bleed) * GAIN_SMOOTH;

        let base_scaled = raw * (self.smoothed_base_gain * self.stop_fade_gain);

        // 1. Wet corridor reverb send: retains high-frequency hallway reflections without dry wall LPF
        let wet_pan_angle =
            (self.smoothed_pan.clamp(-1.0, 1.0) * 0.35 + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
        let (wet_sin, wet_cos) = wet_pan_angle.sin_cos();
        let mono_wet = 0.5 * (base_scaled.left + base_scaled.right) * self.smoothed_wet_bleed;
        let wet_frame = Frame::new(mono_wet * wet_cos, mono_wet * wet_sin);

        // 2. Dry direct path: `.cld` wall occlusion LPF (`650–850 Hz` smoothed over `100 ms`) + `-6..-10 dB` cut
        let occluded_dry = self.occlusion_filter.process_frame(base_scaled, out_fs);

        // 3. Spatialize dry direct path via Binaural HRTF or Classic PS2 Stereo panning
        let dry_frame = if control.binaural_hrtf_flag.load(Ordering::Relaxed) {
            self.hrtf.process_frame(occluded_dry)
        } else {
            let pan_angle =
                (self.smoothed_pan.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
            let (sin_p, cos_p) = pan_angle.sin_cos();
            Frame::new(occluded_dry.left * cos_p, occluded_dry.right * sin_p)
        };

        (dry_frame, wet_frame)
    }

    /// Synthesizes only the wet corridor reverb send [`Frame`] on the audio callback thread,
    /// bypassing dry occlusion SVF and HRTF stages to save CPU cycles (`SH2-84` & `SH2-49`).
    #[inline]
    pub fn process_wet_only(&mut self, dt: f64, control: &SpatialVoiceControl) -> Frame {
        if control.paused.load(Ordering::Relaxed) {
            return Frame::ZERO;
        }

        if control.stop_requested.load(Ordering::Relaxed) {
            let fade_step = (dt as f32 * 50.0).clamp(0.0005, 0.25);
            self.stop_fade_gain = (self.stop_fade_gain - fade_step).max(0.0);
            if self.stop_fade_gain <= 1e-4 {
                self.finished = true;
                return Frame::ZERO;
            }
        }

        let Some(raw) = self.next_source_frame(dt) else {
            return Frame::ZERO;
        };

        let target_gain = load_f32(&control.base_distance_gain);
        let target_pan = load_f32(&control.stereo_pan);
        let target_wet = load_f32(&control.wet_bleed_scale);

        const GAIN_SMOOTH: f32 = 0.004;
        self.smoothed_base_gain += (target_gain - self.smoothed_base_gain) * GAIN_SMOOTH;
        self.smoothed_pan += (target_pan - self.smoothed_pan) * GAIN_SMOOTH;
        self.smoothed_wet_bleed += (target_wet - self.smoothed_wet_bleed) * GAIN_SMOOTH;

        let base_scaled = raw * (self.smoothed_base_gain * self.stop_fade_gain);
        let wet_pan_angle =
            (self.smoothed_pan.clamp(-1.0, 1.0) * 0.35 + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
        let (wet_sin, wet_cos) = wet_pan_angle.sin_cos();
        let mono_wet = 0.5 * (base_scaled.left + base_scaled.right) * self.smoothed_wet_bleed;
        Frame::new(mono_wet * wet_cos, mono_wet * wet_sin)
    }

    /// Returns `true` when playback has completed.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

/// Kira [`SoundData`] descriptor for the dry occluded + HRTF/PS2-panned 3D SFX stream.
pub struct SpatialDrySoundData {
    pub frames: Arc<[Frame]>,
    pub sample_rate: u32,
    pub loop_region: Option<LoopRegionSamples>,
    pub init: SpatialVoiceInitParams,
    pub control: Arc<SpatialVoiceControl>,
    pub destination: TrackId,
}

impl SoundData for SpatialDrySoundData {
    type Error = Infallible;
    type Handle = SpatialVoiceHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let handle = SpatialVoiceHandle::new(Arc::clone(&self.control));
        let sound = SpatialDrySound {
            core: SpatialDryWetCore::new(
                self.frames,
                self.sample_rate,
                self.loop_region,
                &self.init,
            ),
            control: self.control,
            destination: OutputDestination::Track(self.destination),
            approx_fs: self.sample_rate.max(8_000) as f32,
        };
        Ok((Box::new(sound), handle))
    }
}

struct SpatialDrySound {
    core: SpatialDryWetCore,
    control: Arc<SpatialVoiceControl>,
    destination: OutputDestination,
    approx_fs: f32,
}

impl Sound for SpatialDrySound {
    fn output_destination(&mut self) -> OutputDestination {
        self.destination
    }

    fn on_start_processing(&mut self) {
        self.core.sync_from_control(&self.control, self.approx_fs);
    }

    fn process(
        &mut self,
        dt: f64,
        _clock_info_provider: &ClockInfoProvider,
        _modulator_value_provider: &ModulatorValueProvider,
    ) -> Frame {
        if dt > 1e-7 {
            self.approx_fs = (1.0 / dt) as f32;
        }
        let (dry, _wet) = self.core.process_pair(dt, &self.control);
        dry
    }

    fn finished(&self) -> bool {
        self.core.is_finished()
    }
}

/// Kira [`SoundData`] descriptor for the parallel wet corridor reverb send stream (`SH2-84`).
pub struct SpatialWetSendSoundData {
    pub frames: Arc<[Frame]>,
    pub sample_rate: u32,
    pub loop_region: Option<LoopRegionSamples>,
    pub init: SpatialVoiceInitParams,
    pub control: Arc<SpatialVoiceControl>,
    pub reverb_bus: TrackId,
}

impl SoundData for SpatialWetSendSoundData {
    type Error = Infallible;
    type Handle = ();

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let sound = SpatialWetSendSound {
            core: SpatialDryWetCore::new(
                self.frames,
                self.sample_rate,
                self.loop_region,
                &self.init,
            ),
            control: self.control,
            destination: OutputDestination::Track(self.reverb_bus),
        };
        Ok((Box::new(sound), ()))
    }
}

struct SpatialWetSendSound {
    core: SpatialDryWetCore,
    control: Arc<SpatialVoiceControl>,
    destination: OutputDestination,
}

impl Sound for SpatialWetSendSound {
    fn output_destination(&mut self) -> OutputDestination {
        self.destination
    }

    fn process(
        &mut self,
        dt: f64,
        _clock_info_provider: &ClockInfoProvider,
        _modulator_value_provider: &ModulatorValueProvider,
    ) -> Frame {
        self.core.process_wet_only(dt, &self.control)
    }

    fn finished(&self) -> bool {
        self.core.is_finished()
    }
}
