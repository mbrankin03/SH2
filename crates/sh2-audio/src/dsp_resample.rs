//! Sample conversion, Catmull-Rom cubic upsampling, Butterworth anti-imaging filtering,
//! and sample-accurate loop-boundary crossfade preparation (`SH2-55`).

use crate::looping::{
    apply_loop_boundary_crossfade, LoopRegionSamples, DEFAULT_LOOP_CROSSFADE_SAMPLES_48K,
};
use kira::Frame;
use sh2_assets::{DecodedAdx, WavEntry};

/// Converts uncompressed 16-bit PCM bytes from a `WavEntry` into Kira `Frame` samples.
pub fn wav_entry_to_frames(wav: &WavEntry) -> Vec<Frame> {
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

/// Converts a `WavEntry` into Kira `Frame` samples and upsamples low-sample-rate
/// original PS2/PC SFX (`< 44,100 Hz`, typically `13,000–16,000 Hz`) to `48,000 Hz`
/// using 4-point Catmull-Rom cubic interpolation and a 2-stage Butterworth low-pass
/// reconstruction filter to eliminate ultrasonic stair-step aliasing and harshness.
///
/// When `wav.loops == true`, also applies sample-accurate raised-cosine loop-boundary
/// crossfading (`SH2-55`) so looping SFX wrap without clicks or pops at `44.1 kHz / 48 kHz`.
pub fn wav_entry_to_frames_48k(wav: &WavEntry) -> (u32, Vec<Frame>) {
    let raw_frames = wav_entry_to_frames(wav);
    let src_rate = wav.sample_rate.max(8000);
    // Original PC/PS2 `data/sound/sddata.bin` SFX are mastered ~3.5 dB hotter than `sh2e/sound/sddata.bin`
    let (dst_rate, mut frames) = upsample_and_warm_filter_48k(raw_frames, src_rate, 0.65);

    if wav.loops {
        if let Some(region) = LoopRegionSamples::from_source_samples(
            wav.loop_start,
            wav.loop_end,
            src_rate,
            dst_rate,
            frames.len(),
        ) {
            apply_loop_boundary_crossfade(&mut frames, region, DEFAULT_LOOP_CROSSFADE_SAMPLES_48K);
        }
    }

    (dst_rate, frames)
}

/// Converts a `DecodedAdx` stream into Kira `Frame` samples and upsamples low-sample-rate
/// original PS2/PC ADX/AIX streams (`< 44,100 Hz`, typically `22,050–24,000 Hz`) to `48,000 Hz`
/// with anti-imaging reconstruction filtering and level matching.
///
/// When `adx.header.has_loop == true`, also applies sample-accurate raised-cosine loop-boundary
/// crossfading (`SH2-55`) to prevent loop clicks or pops at `44.1 kHz / 48 kHz`.
pub fn adx_to_frames_48k(adx: &DecodedAdx) -> (u32, Vec<Frame>) {
    let raw_frames: Vec<Frame> = adx.frames.iter().map(|&(l, r)| Frame::new(l, r)).collect();
    let src_rate = adx.header.sample_rate.max(8000);
    let (dst_rate, mut frames) = upsample_and_warm_filter_48k(raw_frames, src_rate, 0.78);

    if adx.header.has_loop && adx.header.loop_end_sample > adx.header.loop_start_sample {
        if let Some(region) = LoopRegionSamples::from_source_samples(
            adx.header.loop_start_sample,
            adx.header.loop_end_sample,
            src_rate,
            dst_rate,
            frames.len(),
        ) {
            apply_loop_boundary_crossfade(&mut frames, region, DEFAULT_LOOP_CROSSFADE_SAMPLES_48K);
        }
    }

    (dst_rate, frames)
}

/// Upsamples low-rate (`< 44,100 Hz`) audio buffers to `48,000 Hz` using 4-point Catmull-Rom
/// cubic spline interpolation followed by a 2-stage cascaded 2nd-order Butterworth low-pass
/// filter (`24 dB/oct` roll-off below Nyquist) to emulate the PS2 SPU2 Gaussian DAC and remove
/// high-frequency linear interpolation imaging ("tinny" metallic harshness).
pub fn upsample_and_warm_filter_48k(
    frames: Vec<Frame>,
    src_rate: u32,
    gain_trim: f32,
) -> (u32, Vec<Frame>) {
    const TARGET_RATE: u32 = 48_000;
    if src_rate >= 44_100 || frames.len() < 4 {
        return (src_rate, frames);
    }

    let in_len = frames.len();
    let ratio = src_rate as f64 / TARGET_RATE as f64;
    let out_len = ((in_len as f64) / ratio).ceil() as usize;
    let mut upsampled = Vec::with_capacity(out_len);

    // 1. 4-point Catmull-Rom cubic interpolation to 48,000 Hz
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as isize;
        let t = (src_pos - idx as f64) as f32;

        let p0 = frames[(idx - 1).clamp(0, in_len as isize - 1) as usize];
        let p1 = frames[idx.clamp(0, in_len as isize - 1) as usize];
        let p2 = frames[(idx + 1).clamp(0, in_len as isize - 1) as usize];
        let p3 = frames[(idx + 2).clamp(0, in_len as isize - 1) as usize];

        let t2 = t * t;
        let t3 = t2 * t;

        let interp = |y0: f32, y1: f32, y2: f32, y3: f32| -> f32 {
            0.5 * ((2.0 * y1)
                + (-y0 + y2) * t
                + (2.0 * y0 - 5.0 * y1 + 4.0 * y2 - y3) * t2
                + (-y0 + 3.0 * y1 - 3.0 * y2 + y3) * t3)
        };

        let l = interp(p0.left, p1.left, p2.left, p3.left) * gain_trim;
        let r = interp(p0.right, p1.right, p2.right, p3.right) * gain_trim;
        upsampled.push(Frame::new(l, r));
    }

    // 2. 2-stage cascaded Butterworth low-pass reconstruction filter at 40% of source rate
    // (clamped to 3,200..6,200 Hz) to strip ultrasonic stair-step images and ADPCM quantization fizz.
    let cutoff_hz = (src_rate as f32 * 0.40).clamp(3200.0, 6200.0);
    for _stage in 0..2 {
        apply_stereo_lowpass_biquad(&mut upsampled, TARGET_RATE as f32, cutoff_hz, 0.70710677);
    }

    (TARGET_RATE, upsampled)
}

/// Applies an in-place 2nd-order Butterworth low-pass biquad filter to stereo `Frame` samples.
pub fn apply_stereo_lowpass_biquad(frames: &mut [Frame], fs: f32, fc: f32, q: f32) {
    let omega = 2.0 * std::f32::consts::PI * (fc / fs).clamp(0.001, 0.49);
    let sin_w = omega.sin();
    let cos_w = omega.cos();
    let alpha = sin_w / (2.0 * q);

    let b0 = (1.0 - cos_w) * 0.5;
    let b1 = 1.0 - cos_w;
    let b2 = (1.0 - cos_w) * 0.5;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w;
    let a2 = 1.0 - alpha;

    let b0_n = b0 / a0;
    let b1_n = b1 / a0;
    let b2_n = b2 / a0;
    let a1_n = a1 / a0;
    let a2_n = a2 / a0;

    let (mut x1_l, mut x2_l, mut y1_l, mut y2_l) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (mut x1_r, mut x2_r, mut y1_r, mut y2_r) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);

    for frame in frames.iter_mut() {
        let xl = frame.left;
        let yl = b0_n * xl + b1_n * x1_l + b2_n * x2_l - a1_n * y1_l - a2_n * y2_l;
        x2_l = x1_l;
        x1_l = xl;
        y2_l = y1_l;
        y1_l = yl;

        let xr = frame.right;
        let yr = b0_n * xr + b1_n * x1_r + b2_n * x2_r - a1_n * y1_r - a2_n * y2_r;
        x2_r = x1_r;
        x1_r = xr;
        y2_r = y1_r;
        y1_r = yr;

        frame.left = yl.clamp(-1.0, 1.0);
        frame.right = yr.clamp(-1.0, 1.0);
    }
}
