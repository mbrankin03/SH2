//! Safe binary parser for Silent Hill 2 Sound Effect Audio Banks (`sddata.bin`).
//!
//! # Format Architecture & Memory Layout
//! Silent Hill 2 sound effects reside in `data/sound/sddata.bin`, containing 417 standard
//! RIFF WAVE audio files concatenated back-to-back.
//!
//! ### Audio Bank Structure
//! - **RIFF WAVE Chunks**:
//!   - `RIFF` header with chunk size.
//!   - `WAVE` format header (`fmt ` chunk: audio format, channels, sample rate, bit depth).
//!   - `data` chunk: raw PCM audio samples.
//! - **Loop Flags**:
//!   - Each RIFF file in `sddata.bin` is followed by 2 trailing bytes.
//!   - When `trailing[1] == 1`, the sound effect is looping (e.g., ambient hums, siren loops).
//!   - The original executable (`sh2pc.exe`) resolves sound IDs via direct byte offset tables
//!     (table at `0x8A45A8 + 4 * id`).

use crate::error::AssetError;

/// Header identifier for RIFF containers (`RIFF`).
pub const RIFF_MAGIC: [u8; 4] = *b"RIFF";

/// Wave container format tag (`WAVE`).
pub const WAVE_MAGIC: [u8; 4] = *b"WAVE";

/// Extracted sound effect waveform entry.
#[derive(Debug, Clone)]
pub struct WavEntry {
    /// Number of audio channels (typically 1 for mono, 2 for stereo).
    pub channels: u16,
    /// Sample rate in Hertz (e.g. 11025, 22050, 44100, 48000).
    pub sample_rate: u32,
    /// Quantization bit depth (typically 16-bit PCM).
    pub bits_per_sample: u16,
    /// Raw uncompressed PCM sample data bytes.
    pub data: Vec<u8>,
    /// Indicates whether this sound effect loops continuously during playback.
    pub loops: bool,
}

/// Collection of sound effect waveforms parsed from `sddata.bin`.
#[derive(Debug, Clone, Default)]
pub struct SoundBank {
    /// Audio sample entries in sequential file order.
    pub entries: Vec<WavEntry>,
}

/// Parses sound data from `sddata.bin`, which packs RIFF WAVE files back-to-back.
///
/// Each RIFF WAVE is followed by two trailing bytes where `trailing[1] == 1` indicates looping.
pub fn parse_sddata(data: &[u8]) -> Result<SoundBank, AssetError> {
    let mut entries = Vec::new();
    let mut p = 0usize;

    while p + 8 <= data.len() {
        if data[p..p + 4] != RIFF_MAGIC {
            // Scan forward for next RIFF header
            if let Some(next_riff) = data[p..].windows(4).position(|w| w == RIFF_MAGIC) {
                p += next_riff;
            } else {
                break;
            }
        }

        let riff_len = read_u32(data, p + 4)? as usize;
        let total_wav_len = 8 + riff_len;

        if p + total_wav_len > data.len() {
            break;
        }

        let wav_slice = &data[p..p + total_wav_len];
        if wav_slice.len() > 12 && wav_slice[8..12] == WAVE_MAGIC {
            if let Ok(wav_entry) = parse_single_wav(wav_slice) {
                let mut entry = wav_entry;
                // Check trailing 2 bytes for loop flag
                let trail_offset = p + total_wav_len;
                if trail_offset + 2 <= data.len() {
                    entry.loops = data[trail_offset + 1] == 1;
                }
                entries.push(entry);
            }
        }

        p += total_wav_len;
        // Skip trailing bytes if present
        if p + 2 <= data.len() && data[p..p.min(data.len().saturating_sub(4)) + 4] != RIFF_MAGIC {
            p += 2;
        }
    }

    Ok(SoundBank { entries })
}

/// Parses a single RIFF WAVE container and extracts channel, rate, depth, and PCM data.
fn parse_single_wav(data: &[u8]) -> Result<WavEntry, AssetError> {
    let mut p = 12usize;
    let mut channels = 1u16;
    let mut sample_rate = 22050u32;
    let mut bits_per_sample = 16u16;
    let mut pcm_data = Vec::new();

    while p + 8 <= data.len() {
        let chunk_id = &data[p..p + 4];
        let chunk_len = read_u32(data, p + 4)? as usize;
        let chunk_data = p + 8;

        if chunk_data + chunk_len > data.len() {
            break;
        }

        if chunk_id == b"fmt " && chunk_len >= 14 {
            channels = read_u16(data, chunk_data + 2)?;
            sample_rate = read_u32(data, chunk_data + 4)?;
            if chunk_len >= 16 {
                bits_per_sample = read_u16(data, chunk_data + 14)?;
            }
        } else if chunk_id == b"data" {
            pcm_data = data[chunk_data..chunk_data + chunk_len].to_vec();
        }

        p = chunk_data + chunk_len;
        // Chunks are 2-byte aligned
        if !p.is_multiple_of(2) {
            p += 1;
        }
    }

    Ok(WavEntry {
        channels,
        sample_rate,
        bits_per_sample,
        data: pcm_data,
        loops: false,
    })
}

#[inline(always)]
fn read_u16(data: &[u8], ofs: usize) -> Result<u16, AssetError> {
    if ofs + 2 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        })
    } else {
        Ok(u16::from_le_bytes([data[ofs], data[ofs + 1]]))
    }
}

#[inline(always)]
fn read_u32(data: &[u8], ofs: usize) -> Result<u32, AssetError> {
    if ofs + 4 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        })
    } else {
        Ok(u32::from_le_bytes([
            data[ofs],
            data[ofs + 1],
            data[ofs + 2],
            data[ofs + 3],
        ]))
    }
}
