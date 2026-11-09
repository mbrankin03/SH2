//! DSP resampling, Butterworth warm reconstruction filtering, WAV `smpl` loop parsing,
//! and sample-accurate raised-cosine loop-boundary crossfade tests (`SH2-55` / `SH2-58`).

use super::common::make_test_wav;
use sh2_assets::{AdxHeader, DecodedAdx};
use sh2_audio::*;

#[test]
fn test_ps2_low_rate_upsampling_and_warm_lowpass_filter() {
    // 1. A 16 kHz original PS2/PC SFX WAV should be upsampled 3x to 48 kHz and trimmed
    let wav_16k = make_test_wav(16000, 1, false);
    let (out_rate, out_frames) = sh2_audio::engine::wav_entry_to_frames_48k(&wav_16k);
    assert_eq!(out_rate, 48000);
    assert_eq!(out_frames.len(), 1600 * 3);

    // 2. A 48 kHz Enhanced Lossless SFX WAV should pass through untouched without resampling
    let wav_48k = make_test_wav(48000, 2, false);
    let (rate_48k, frames_48k) = sh2_audio::engine::wav_entry_to_frames_48k(&wav_48k);
    assert_eq!(rate_48k, 48000);
    assert_eq!(frames_48k.len(), 4800);

    // 3. A 22,050 Hz original ADX stream with a harsh 9.5 kHz Nyquist-adjacent tone
    // should have its high-frequency fizz strongly attenuated by the reconstruction filter
    let sr = 22050u32;
    let n = 2205usize; // 0.1s
    let mut frames = Vec::with_capacity(n);
    let mut pcm_i16 = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let hf = (t * 9500.0 * 2.0 * std::f32::consts::PI).sin() * 0.8;
        frames.push((hf, hf));
        let s = (hf * 32767.0) as i16;
        pcm_i16.push(s);
        pcm_i16.push(s);
    }
    let harsh_adx = DecodedAdx {
        header: AdxHeader {
            channels: 2,
            sample_rate: sr,
            total_samples: n as u32,
            highpass_frequency: 500,
            version: 3,
            data_offset: 0x20,
            has_loop: false,
            loop_start_sample: 0,
            loop_end_sample: 0,
        },
        pcm_i16,
        frames,
    };
    let (adx_rate, adx_frames) = sh2_audio::engine::adx_to_frames_48k(&harsh_adx);
    assert_eq!(adx_rate, 48000);
    // Skip first 100 samples of filter transient, measure RMS of 9.5 kHz tone after 6.2 kHz 4th-order Butterworth LPF
    let rms: f32 = (adx_frames[100..]
        .iter()
        .map(|f| f.left * f.left)
        .sum::<f32>()
        / (adx_frames.len() - 100) as f32)
        .sqrt();
    assert!(
        rms < 0.18,
        "Expected high-frequency 9.5 kHz harshness to be attenuated below 0.18 RMS, got {rms}"
    );
}

#[test]
fn test_sh2_55_wav_smpl_loop_header_parsing_and_loop_region_scaling() {
    // 1. Construct a synthetic RIFF WAVE with a standard 60-byte `smpl` chunk specifying loop_start = 400, loop_end = 1200
    let mut smpl_payload = vec![0u8; 60];
    smpl_payload[28..32].copy_from_slice(&1u32.to_le_bytes()); // num_sample_loops = 1
    smpl_payload[44..48].copy_from_slice(&400u32.to_le_bytes()); // loop_start = 400
    smpl_payload[48..52].copy_from_slice(&1200u32.to_le_bytes()); // loop_end = 1200
    assert_eq!(
        sh2_assets::parse_wav_smpl_loop(&smpl_payload),
        Some((400, 1200))
    );

    // 2. Verify LoopRegionSamples proportional upsample scaling (16,000 Hz -> 48,000 Hz = 3x)
    let reg = LoopRegionSamples::from_source_samples(400, 1200, 16_000, 48_000, 4800)
        .expect("Expected valid LoopRegionSamples");
    assert_eq!(reg.loop_start_frame, 1200);
    assert_eq!(reg.loop_end_frame, 3600);
    assert_eq!(reg.sample_rate, 48_000);
}

#[test]
fn test_sh2_55_sample_accurate_loop_crossfade_eliminates_discontinuity() {
    // Create a 48 kHz buffer with a severe step discontinuity between loop_start and loop_end - 1:
    // frames[loop_start] = (+0.75, -0.75), while frames[loop_end - 1] = (-0.80, +0.80).
    let total = 4800usize;
    let loop_start = 0usize;
    let loop_end = 4000usize;
    let mut frames = Vec::with_capacity(total);
    for i in 0..total {
        if i < 2000 {
            frames.push(kira::Frame::new(0.75, -0.75));
        } else {
            frames.push(kira::Frame::new(-0.80, 0.80));
        }
    }

    let initial_step = measure_loop_boundary_step_delta(&frames, loop_start, loop_end);
    assert!(
        initial_step > 2.0,
        "Expected large un-crossfaded step discontinuity > 2.0, got {initial_step}"
    );

    let region = LoopRegionSamples {
        sample_rate: 48_000,
        loop_start_frame: loop_start,
        loop_end_frame: loop_end,
        total_frames: total,
    };
    apply_loop_boundary_crossfade(&mut frames, region, DEFAULT_LOOP_CROSSFADE_SAMPLES_48K);

    // Buffer should be truncated to `loop_end` (4000) and `frames[loop_end - 1]` must match `frames[loop_start]`
    assert_eq!(frames.len(), loop_end);
    let post_step = measure_loop_boundary_step_delta(&frames, loop_start, loop_end);
    assert!(
        post_step < 1e-5,
        "Expected crossfaded loop boundary step delta < 1e-5, got {post_step}"
    );

    // Also verify Case A (`loop_start >= window`, pre-loop lead-in crossfade) achieves zero step discontinuity
    let mut lead_in_frames = Vec::with_capacity(total);
    for i in 0..total {
        let phase = (i as f32) * 0.025;
        if i < 2400 {
            lead_in_frames.push(kira::Frame::new(phase.sin(), phase.cos()));
        } else {
            lead_in_frames.push(kira::Frame::new(-0.90, 0.85));
        }
    }
    let lead_in_region = LoopRegionSamples {
        sample_rate: 48_000,
        loop_start_frame: 600,
        loop_end_frame: 4200,
        total_frames: total,
    };
    apply_loop_boundary_crossfade(
        &mut lead_in_frames,
        lead_in_region,
        DEFAULT_LOOP_CROSSFADE_SAMPLES_48K,
    );
    assert_eq!(lead_in_frames.len(), 4200);
    let case_a_step = measure_loop_boundary_step_delta(&lead_in_frames, 600, 4200);
    assert!(
        case_a_step < 1e-5,
        "Expected Case A pre-loop lead-in crossfade step delta < 1e-5, got {case_a_step}"
    );
}
