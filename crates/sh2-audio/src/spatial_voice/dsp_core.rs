//! Zero-allocation real-time dry/wet DSP cores (`SpatialDryWetCore` & `SpatialWetSendCore`) (`SH2-84` & `SH2-49`).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use kira::Frame;

use super::{load_f32, sanitize_f32, SpatialVoiceControl, SpatialVoiceInitParams};
use crate::hrtf::BinauralHrtfProcessor;
use crate::looping::LoopRegionSamples;
use crate::occlusion::{OcclusionFilterState, UNOCCLUDED_CUTOFF_HZ};

#[inline]
fn gain_smooth_for_dt(dt: f64, fallback_sr: u32) -> (f32, f32) {
    let out_fs = if dt.is_finite() && dt > 1e-7 {
        ((1.0 / dt) as f32).clamp(8_000.0, 384_000.0)
    } else {
        fallback_sr.max(8_000) as f32
    };
    let smooth = (0.004 * (48_000.0 / out_fs)).clamp(0.0005, 0.03);
    (out_fs, smooth)
}

/// Shared zero-allocation sample cursor, linear resampler, and loop-region reader (`SH2-84`).
#[derive(Debug, Clone)]
struct SampleStreamReader {
    frames: Arc<[Frame]>,
    sample_rate: u32,
    loop_region: Option<LoopRegionSamples>,
    cursor: f64,
    finished: bool,
}

impl SampleStreamReader {
    #[inline]
    fn new(frames: Arc<[Frame]>, sample_rate: u32, loop_region: Option<LoopRegionSamples>) -> Self {
        let empty = frames.is_empty();
        Self {
            frames,
            sample_rate: sample_rate.max(8_000),
            loop_region,
            cursor: 0.0,
            finished: empty,
        }
    }

    #[inline]
    fn next_frame(&mut self, dt: f64) -> Option<Frame> {
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

        if dt.is_finite() && dt > 0.0 {
            self.cursor += dt * (self.sample_rate as f64);
        }
        Some(sample)
    }
}

/// Lightweight zero-allocation wet corridor reverb send core without dry SVF or HRTF state (`SH2-84`).
#[derive(Debug, Clone)]
pub struct SpatialWetSendCore {
    reader: SampleStreamReader,
    smoothed_base_gain: f32,
    smoothed_pan: f32,
    smoothed_wet_bleed: f32,
    stop_fade_gain: f32,
}

impl SpatialWetSendCore {
    /// Creates a lightweight wet reverb send reader initialized to `init`.
    pub fn new(
        frames: Arc<[Frame]>,
        sample_rate: u32,
        loop_region: Option<LoopRegionSamples>,
        init: &SpatialVoiceInitParams,
    ) -> Self {
        Self {
            reader: SampleStreamReader::new(frames, sample_rate, loop_region),
            smoothed_base_gain: sanitize_f32(init.base_distance_gain, 0.0).max(0.0),
            smoothed_pan: sanitize_f32(init.stereo_pan, 0.0).clamp(-1.0, 1.0),
            smoothed_wet_bleed: sanitize_f32(init.occlusion.wet_reverb_bleed_scale, 1.0)
                .clamp(0.0, 1.5),
            stop_fade_gain: 1.0,
        }
    }

    /// Synthesizes one stereo wet corridor reverb send [`Frame`] with zero heap allocations (`SH2-49`).
    #[inline]
    pub fn process_wet_only(&mut self, dt: f64, control: &SpatialVoiceControl) -> Frame {
        if control.paused.load(Ordering::Relaxed) {
            return Frame::ZERO;
        }
        if control.stop_requested.load(Ordering::Relaxed) {
            let fade_step = ((dt as f32) * 50.0).clamp(0.0005, 0.25);
            self.stop_fade_gain = (self.stop_fade_gain - fade_step).max(0.0);
            if self.stop_fade_gain <= 1e-4 {
                self.reader.finished = true;
                return Frame::ZERO;
            }
        }
        let Some(raw) = self.reader.next_frame(dt) else {
            return Frame::ZERO;
        };

        let (_out_fs, gain_smooth) = gain_smooth_for_dt(dt, self.reader.sample_rate);
        let target_gain = load_f32(&control.base_distance_gain);
        let target_pan = load_f32(&control.stereo_pan);
        let target_wet = if control.is_occlusion_enabled() {
            load_f32(&control.wet_bleed_scale)
        } else {
            1.0
        };

        self.smoothed_base_gain += (target_gain - self.smoothed_base_gain) * gain_smooth;
        self.smoothed_pan += (target_pan - self.smoothed_pan) * gain_smooth;
        self.smoothed_wet_bleed += (target_wet - self.smoothed_wet_bleed) * gain_smooth;

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
        self.reader.finished
    }
}

/// Pure, zero-allocation real-time 3D SFX DSP core usable both inside Kira's audio thread
/// and directly in deterministic / zero-allocation offline unit tests (`SH2-84` & `SH2-49`).
#[derive(Debug, Clone)]
pub struct SpatialDryWetCore {
    reader: SampleStreamReader,
    smoothed_base_gain: f32,
    smoothed_pan: f32,
    smoothed_wet_bleed: f32,
    stop_fade_gain: f32,
    /// Stereo 2-pole Butterworth TPT low-pass filter (`Hz` cutoff & linear dry gain, `100 ms` ramp).
    pub occlusion_filter: OcclusionFilterState,
    /// Binaural spherical-head ITD, head-shadow ILD, and pinna spectral elevation processor.
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
        Self::new_with_occlusion_enabled(frames, sample_rate, loop_region, init, true)
    }

    /// Initializes a zero-allocation DSP core, snapping to transparent `20 kHz` / `0 dB` when
    /// `occlusion_enabled` is `false` (`OFF (Original PS2)`).
    pub fn new_with_occlusion_enabled(
        frames: Arc<[Frame]>,
        sample_rate: u32,
        loop_region: Option<LoopRegionSamples>,
        init: &SpatialVoiceInitParams,
        occlusion_enabled: bool,
    ) -> Self {
        let fs = sample_rate.max(8_000) as f32;
        let (init_fc, init_dry_gain, init_wet_bleed) = if occlusion_enabled {
            (
                init.occlusion.dry_cutoff_hz,
                init.occlusion.dry_gain_scale,
                sanitize_f32(init.occlusion.wet_reverb_bleed_scale, 1.0).clamp(0.0, 1.5),
            )
        } else {
            (UNOCCLUDED_CUTOFF_HZ, 1.0, 1.0)
        };
        let mut occlusion_filter = OcclusionFilterState::new(init_fc, init_dry_gain);
        occlusion_filter.snap_to(init_fc, init_dry_gain, fs);
        let mut hrtf = BinauralHrtfProcessor::new();
        hrtf.update_angles(init.angles.azimuth_rad, init.angles.elevation_rad, fs, true);

        Self {
            reader: SampleStreamReader::new(frames, sample_rate, loop_region),
            smoothed_base_gain: sanitize_f32(init.base_distance_gain, 0.0).max(0.0),
            smoothed_pan: sanitize_f32(init.stereo_pan, 0.0).clamp(-1.0, 1.0),
            smoothed_wet_bleed: init_wet_bleed,
            stop_fade_gain: 1.0,
            occlusion_filter,
            hrtf,
        }
    }

    /// Synchronizes targets from `control` at the start of an audio callback block (zero allocations).
    #[inline]
    pub fn sync_from_control(&mut self, control: &SpatialVoiceControl, output_sample_rate: f32) {
        if control.is_occlusion_enabled() {
            let cutoff = load_f32(&control.occlusion_cutoff_hz);
            let dry_scale = load_f32(&control.occlusion_dry_scale);
            self.occlusion_filter.set_target(cutoff, dry_scale);
        } else {
            self.occlusion_filter.set_target(UNOCCLUDED_CUTOFF_HZ, 1.0);
        }

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
            let fade_step = ((dt as f32) * 50.0).clamp(0.0005, 0.25);
            self.stop_fade_gain = (self.stop_fade_gain - fade_step).max(0.0);
            if self.stop_fade_gain <= 1e-4 {
                self.reader.finished = true;
                control.dry_finished.store(true, Ordering::Relaxed);
                return (Frame::ZERO, Frame::ZERO);
            }
        }
        let Some(raw) = self.reader.next_frame(dt) else {
            control.dry_finished.store(true, Ordering::Relaxed);
            return (Frame::ZERO, Frame::ZERO);
        };

        let (out_fs, gain_smooth) = gain_smooth_for_dt(dt, self.reader.sample_rate);
        let target_gain = load_f32(&control.base_distance_gain);
        let target_pan = load_f32(&control.stereo_pan);
        let target_wet = if control.is_occlusion_enabled() {
            load_f32(&control.wet_bleed_scale)
        } else {
            1.0
        };

        self.smoothed_base_gain += (target_gain - self.smoothed_base_gain) * gain_smooth;
        self.smoothed_pan += (target_pan - self.smoothed_pan) * gain_smooth;
        self.smoothed_wet_bleed += (target_wet - self.smoothed_wet_bleed) * gain_smooth;

        let base_scaled = raw * (self.smoothed_base_gain * self.stop_fade_gain);

        let wet_pan_angle =
            (self.smoothed_pan.clamp(-1.0, 1.0) * 0.35 + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
        let (wet_sin, wet_cos) = wet_pan_angle.sin_cos();
        let mono_wet = 0.5 * (base_scaled.left + base_scaled.right) * self.smoothed_wet_bleed;
        let wet_frame = Frame::new(mono_wet * wet_cos, mono_wet * wet_sin);

        let occluded_dry = self.occlusion_filter.process_frame(base_scaled, out_fs);
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

    /// Returns `true` when playback has completed.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.reader.finished
    }
}
