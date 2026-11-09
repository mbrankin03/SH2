//! Shared synthetic WAV and ADX generators for `sh2-audio` integration tests.

use sh2_assets::WavEntry;
#[cfg(not(target_os = "windows"))]
use sh2_assets::{AdxHeader, DecodedAdx};

pub fn make_test_wav(sample_rate: u32, channels: u16, loops: bool) -> WavEntry {
    // Generate 0.1s test tone (440Hz sine wave)
    let total_samples = (sample_rate as f32 * 0.1) as usize;
    let mut pcm_bytes = Vec::with_capacity(total_samples * channels as usize * 2);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let val = ((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 10000.0) as i16;
        let bytes = val.to_le_bytes();
        for _ in 0..channels {
            pcm_bytes.extend_from_slice(&bytes);
        }
    }

    WavEntry {
        id: 11000,
        channels,
        sample_rate,
        bits_per_sample: 16,
        data: pcm_bytes,
        loops,
        loop_start: 0,
        loop_end: total_samples as u32,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn make_test_adx(sample_rate: u32, has_loop: bool) -> DecodedAdx {
    let total_samples = (sample_rate as f32 * 0.1) as usize;
    let mut frames = Vec::with_capacity(total_samples);
    let mut pcm_i16 = Vec::with_capacity(total_samples * 2);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let val = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
        frames.push((val, val));
        let s16 = (val * 32767.0) as i16;
        pcm_i16.push(s16);
        pcm_i16.push(s16);
    }

    DecodedAdx {
        header: AdxHeader {
            channels: 2,
            sample_rate,
            total_samples: total_samples as u32,
            highpass_frequency: 500,
            version: 3,
            data_offset: 0x20,
            has_loop,
            loop_start_sample: if has_loop { 100 } else { 0 },
            loop_end_sample: if has_loop { total_samples as u32 } else { 0 },
        },
        pcm_i16,
        frames,
    }
}
