//! Parser and demuxer for CRI AIX multi-channel / multi-layer audio containers (.aix).
//!
//! # Format Architecture
//! CRI AIX (AIXF / AIXP / AIXE) is a multi-stream container format used extensively in
//! Silent Hill 2 for multi-layer background music, ambient soundscapes, and multi-track cutscene audio.
//! Each stream inside an AIX file is a full CRI ADX audio stream (0x8000 magic).

use crate::error::AssetError;
use crate::formats::adx::{decode_adx, DecodedAdx};

/// Demuxes an AIX multi-channel stream into individual raw ADX byte buffers.
pub fn demux_aix(data: &[u8]) -> Result<Vec<Vec<u8>>, AssetError> {
    if data.len() < 16 || &data[0..4] != b"AIXF" {
        return Err(AssetError::InvalidMagic {
            expected: b"AIXF",
            found: data.get(0..4).unwrap_or_default().to_vec(),
        });
    }

    let mut pos = 0;
    let mut streams: Vec<Vec<u8>> = Vec::new();

    while pos + 8 <= data.len() {
        if &data[pos..pos + 3] != b"AIX" {
            break;
        }

        let chunk_type = data[pos + 3];
        let next_offset =
            u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let next_hdr = pos + 8 + next_offset;

        if chunk_type == b'P' {
            if pos + 16 > data.len() {
                break;
            }
            let channel = data[pos + 8] as usize;
            let channel_count = data[pos + 9] as usize;
            let size = u16::from_be_bytes([data[pos + 10], data[pos + 11]]) as usize;

            if streams.len() < channel_count {
                streams.resize(channel_count, Vec::new());
            }

            if pos + 16 + size <= data.len() {
                let payload = &data[pos + 16..pos + 16 + size];
                if channel < streams.len() {
                    streams[channel].extend_from_slice(payload);
                }
            }
        }

        if next_hdr <= pos || next_hdr > data.len() {
            break;
        }
        pos = next_hdr;
    }

    if streams.is_empty() {
        return Err(AssetError::BufferUnderflow {
            required: 32,
            available: data.len(),
        });
    }

    Ok(streams)
}

/// Decodes an AIX container into decoded ADX streams for each contained track/channel.
pub fn decode_aix(data: &[u8]) -> Result<Vec<DecodedAdx>, AssetError> {
    let raw_adx_streams = demux_aix(data)?;
    let mut decoded = Vec::with_capacity(raw_adx_streams.len());
    for raw in &raw_adx_streams {
        if !raw.is_empty() {
            decoded.push(decode_adx(raw)?);
        }
    }
    Ok(decoded)
}

/// Decodes and downmixes all non-silent layers of an AIX file into a single stereo DecodedAdx.
pub fn decode_aix_mixed(data: &[u8]) -> Result<DecodedAdx, AssetError> {
    let layers = decode_aix(data)?;
    if layers.is_empty() {
        return Err(AssetError::BufferUnderflow {
            required: 32,
            available: data.len(),
        });
    }

    if layers.len() == 1 {
        return Ok(layers.into_iter().next().unwrap());
    }

    let max_len = layers.iter().map(|l| l.frames.len()).max().unwrap_or(0);
    let mut mixed_frames: Vec<(f32, f32)> = vec![(0.0, 0.0); max_len];

    let active_layers: Vec<&DecodedAdx> = layers
        .iter()
        .filter(|l| {
            l.frames
                .iter()
                .take(48000)
                .any(|&(left, right)| left.abs() > 1e-4 || right.abs() > 1e-4)
        })
        .collect();

    let layer_pool = if active_layers.is_empty() {
        layers.iter().collect::<Vec<_>>()
    } else {
        active_layers
    };

    let weight = 1.0 / (layer_pool.len().max(1) as f32).sqrt();

    for layer in &layer_pool {
        for (i, &(l, r)) in layer.frames.iter().enumerate() {
            if i < mixed_frames.len() {
                mixed_frames[i].0 += l * weight;
                mixed_frames[i].1 += r * weight;
            }
        }
    }

    let mut pcm_i16 = Vec::with_capacity(mixed_frames.len() * 2);
    for frame in &mut mixed_frames {
        frame.0 = frame.0.clamp(-1.0, 1.0);
        frame.1 = frame.1.clamp(-1.0, 1.0);
        pcm_i16.push((frame.0 * 32767.0) as i16);
        pcm_i16.push((frame.1 * 32767.0) as i16);
    }

    let mut header = layers[0].header.clone();
    header.total_samples = max_len as u32;

    Ok(DecodedAdx {
        header,
        pcm_i16,
        frames: mixed_frames,
    })
}
