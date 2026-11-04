//! Parser and decoder for CRI ADX audio streams (.adx).
//!
//! # Format Architecture
//! CRI ADX is a lossy audio coding format standard in Dreamcast and PS2 era games,
//! used extensively in Silent Hill 2 for background music (BGM), ambient streams,
//! and voice tracks in `voice.afs`.
//!
//! ### ADX Stream Header Layout:
//! - `[0x00..0x02]`: Magic identifier `0x8000` (big-endian).
//! - `[0x02..0x04]`: Copyright offset `cofs` (big-endian u16). First audio frame starts at `cofs + 4`.
//! - `[0x04]`: Encoding type (3 = standard ADPCM).
//! - `[0x05]`: Block size (typically 18 bytes per channel).
//! - `[0x06]`: Sample bit depth (typically 4 bits per nibble).
//! - `[0x07]`: Channel count (1 = mono, 2 = stereo).
//! - `[0x08..0x0C]`: Sample rate in Hz (big-endian u32, e.g. 22050, 44100, 48000).
//! - `[0x0C..0x10]`: Total sample count (big-endian u32).
//! - `[0x10..0x12]`: High-pass cut-off frequency (big-endian u16).
//! - `[0x12]`: ADX format version (3 or 4).
//! - Loop flags and markers:
//!   - For version 3: loop enabled at `[0x18..0x1C]`, loop start sample at `[0x1C..0x20]`, loop end sample at `[0x24..0x28]`.
//!   - For version 4: loop enabled at `[0x24..0x28]`, loop start sample at `[0x28..0x2C]`, loop end sample at `[0x30..0x34]`.

use crate::error::AssetError;
use std::f64::consts::PI;

/// Header metadata parsed from an ADX stream container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdxHeader {
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Sample rate in Hertz (e.g. 22050, 44100, 48000).
    pub sample_rate: u32,
    /// Total samples per channel in stream.
    pub total_samples: u32,
    /// High-pass cutoff frequency parameter.
    pub highpass_frequency: u16,
    /// ADX codec format version (3 or 4).
    pub version: u8,
    /// Byte offset where audio frame packets begin.
    pub data_offset: usize,
    /// Whether continuous looping is enabled in header metadata.
    pub has_loop: bool,
    /// Loop start sample index.
    pub loop_start_sample: u32,
    /// Loop end sample index.
    pub loop_end_sample: u32,
}

/// Decoded ADX audio track containing PCM samples and stream properties.
#[derive(Debug, Clone)]
pub struct DecodedAdx {
    /// Header metadata parsed from stream.
    pub header: AdxHeader,
    /// Interleaved PCM16 audio samples: length is `total_samples * channels`.
    pub pcm_i16: Vec<i16>,
    /// Normalized stereo floating-point frames `(left, right)` in `[-1.0, 1.0]`.
    pub frames: Vec<(f32, f32)>,
}

/// Parses an ADX audio stream container header.
pub fn parse_adx_header(data: &[u8]) -> Result<AdxHeader, AssetError> {
    if data.len() < 0x20 {
        return Err(AssetError::BufferUnderflow {
            required: 0x20,
            available: data.len(),
        });
    }

    let magic = u16::from_be_bytes([data[0], data[1]]);
    if magic != 0x8000 {
        return Err(AssetError::InvalidMagic {
            expected: &[0x80, 0x00],
            found: data[0..2].to_vec(),
        });
    }

    let copyright_ofs = u16::from_be_bytes([data[2], data[3]]) as usize;
    let encoding_type = data[4];
    let block_size = data[5];
    let bit_depth = data[6];
    let channels = data[7] as u16;

    if encoding_type != 3 || block_size != 18 || bit_depth != 4 {
        return Err(AssetError::UnsupportedFormat(
            "Only standard 4-bit CRI ADX streams (type 3, block size 18) are supported".to_string(),
        ));
    }

    if channels == 0 || channels > 8 {
        return Err(AssetError::InvalidData(format!(
            "Invalid ADX channel count: {channels}"
        )));
    }

    let sample_rate = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let total_samples = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    let highpass_frequency = u16::from_be_bytes([data[16], data[17]]);
    let version = data[18];

    let data_offset = copyright_ofs + 4;
    let mut has_loop = false;
    let mut loop_start_sample = 0u32;
    let mut loop_end_sample = total_samples;

    if version == 3 && copyright_ofs >= 0x28 && data.len() >= 0x28 {
        let loop_flag = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
        if loop_flag != 0 {
            has_loop = true;
            loop_start_sample = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);
            if data.len() >= 0x2C {
                loop_end_sample = u32::from_be_bytes([data[36], data[37], data[38], data[39]]);
            }
        }
    } else if version == 4 && copyright_ofs >= 0x34 && data.len() >= 0x34 {
        let loop_flag = u32::from_be_bytes([data[36], data[37], data[38], data[39]]);
        if loop_flag != 0 {
            has_loop = true;
            loop_start_sample = u32::from_be_bytes([data[40], data[41], data[42], data[43]]);
            if data.len() >= 0x38 {
                loop_end_sample = u32::from_be_bytes([data[48], data[49], data[50], data[51]]);
            }
        }
    }

    Ok(AdxHeader {
        channels,
        sample_rate,
        total_samples,
        highpass_frequency,
        version,
        data_offset,
        has_loop,
        loop_start_sample,
        loop_end_sample,
    })
}

/// Decodes an entire CRI ADX audio stream into uncompressed 16-bit PCM and normalized stereo frames.
pub fn decode_adx(data: &[u8]) -> Result<DecodedAdx, AssetError> {
    let header = parse_adx_header(data)?;

    let channels = header.channels as usize;
    let total_samples = header.total_samples as usize;
    let rate = header.sample_rate as f64;
    let hp = header.highpass_frequency as f64;

    // Calculate prediction filter coefficients (standard ADPCM filter)
    let z = (2.0 * PI * hp / rate).cos();
    let a = std::f64::consts::SQRT_2 - z;
    let b = std::f64::consts::SQRT_2 - 1.0;
    let c = (a - ((a + b) * (a - b)).max(0.0).sqrt()) / b;
    let c1 = (c * 8192.0) as i32;
    let c2 = (c * c * -4096.0) as i32;

    let mut pos = header.data_offset;
    let mut hist = vec![[0i32, 0i32]; channels];
    let mut chan_samples: Vec<Vec<i16>> = vec![Vec::with_capacity(total_samples); channels];
    let is_v3 = header.version == 3;

    let mut samples_decoded = 0usize;

    while samples_decoded < total_samples && pos + 18 * channels <= data.len() {
        for k in 0..channels {
            let frame_slice = &data[pos + 18 * k..pos + 18 * k + 18];
            let raw_scale = i16::from_be_bytes([frame_slice[0], frame_slice[1]]) as i32;
            let scale = raw_scale + 1;
            let [mut h1, mut h2] = hist[k];

            for i in 0..32 {
                if samples_decoded + i >= total_samples {
                    break;
                }
                let byte_idx = 2 + (i / 2);
                let b = frame_slice[byte_idx];
                let mut nib = if (i & 1) == 0 {
                    (b >> 4) as i32
                } else {
                    (b & 0x0F) as i32
                };
                if (nib & 8) != 0 {
                    nib -= 16;
                }

                let v = if is_v3 {
                    nib * scale + ((c1 * h1) >> 12) + ((c2 * h2) >> 12)
                } else {
                    nib * scale + ((c1 * h1 + c2 * h2) >> 12)
                };

                let clamped = v.clamp(-32768, 32767) as i16;
                chan_samples[k].push(clamped);
                h2 = h1;
                h1 = clamped as i32;
            }
            hist[k] = [h1, h2];
        }

        pos += 18 * channels;
        samples_decoded += 32;
    }

    let actual_samples = chan_samples[0].len();
    let mut pcm_i16 = Vec::with_capacity(actual_samples * channels);
    let mut frames = Vec::with_capacity(actual_samples);

    let left_iter = &chan_samples[0];
    let right_opt = if channels > 1 {
        Some(&chan_samples[1])
    } else {
        None
    };

    for (s, &left_s16) in left_iter.iter().enumerate() {
        let right_s16 = match right_opt {
            Some(right_ch) => right_ch[s],
            None => left_s16,
        };

        pcm_i16.push(left_s16);
        if channels > 1 {
            pcm_i16.push(right_s16);
        }

        let left_f32 = left_s16 as f32 / 32768.0;
        let right_f32 = right_s16 as f32 / 32768.0;
        frames.push((left_f32, right_f32));
    }

    Ok(DecodedAdx {
        header,
        pcm_i16,
        frames,
    })
}
