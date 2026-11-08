//! Safe binary parser for Silent Hill 2 Sound Effect Audio Banks (`sddata.bin`, `.sdm` / `.sdb`).
//!
//! # Format Architecture & Memory Layout
//!
//! ### PC Audio Bank (`sddata.bin` / `.sdb` / `.sdm`)
//! Silent Hill 2 sound effects reside in `data/sound/sddata.bin`, containing 417 standard
//! RIFF WAVE audio files concatenated back-to-back.
//!
//! - **RIFF WAVE Chunks**:
//!   - `RIFF` header with chunk size.
//!   - `WAVE` format header (`fmt ` chunk: audio format, channels, sample rate, bit depth).
//!   - `data` chunk: raw PCM audio samples.
//! - **Loop Flags**:
//!   - Each RIFF file in `sddata.bin` is followed by 2 trailing bytes.
//!   - When `trailing[1] == 1`, the sound effect is looping (e.g., ambient hums, siren loops).
//!   - The original executable (`sh2pc.exe`) resolves sound IDs via direct byte offset tables
//!     (table at `0x8A45A8 + 4 * id`).
//!
//! ### PS2 Sound Bank Structure (`.sdm` metadata header + `.sdb` sample data)
//! - `.sdm` (Sound Metadata / header): Defines sound bank header information, program count,
//!   sample descriptors with sample rates, loop points, and voice envelope configs.
//! - `.sdb` (Sound Data Bank / waveforms): Holds raw PCM/ADPCM audio sample data addressed by `.sdm`.
//!   Also parsed when provided as packed container files.

use super::binary::{read_u16, read_u32};
use crate::error::AssetError;

/// Header identifier for RIFF containers (`RIFF`).
pub const RIFF_MAGIC: [u8; 4] = *b"RIFF";

/// Wave container format tag (`WAVE`).
pub const WAVE_MAGIC: [u8; 4] = *b"WAVE";

/// Magic identifier for SDM header banks (`SDM\0` or `SDM1`).
pub const SDM_MAGIC: [u8; 4] = *b"SDM1";

/// Magic identifier for SDB sample data banks (`SDB1`).
pub const SDB_MAGIC: [u8; 4] = *b"SDB1";

/// Magic identifier for CRI Middleware AFS archives (`AFS\0`, e.g. `sound/adx/voice/voice.afs`).
pub const AFS_MAGIC: [u8; 4] = *b"AFS\0";

/// Byte offset descriptor for a single entry inside an `AFS\0` container (`voice.afs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AfsEntry {
    /// Entry index (`0..count`).
    pub index: usize,
    /// Byte offset from the start of the `.afs` file.
    pub offset: u32,
    /// Byte length of the entry payload.
    pub length: u32,
}

/// Parsed directory index of an `AFS\0` container (`data/sound/adx/voice/voice.afs` or `sh2e/sound/adx/voice/voice.afs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AfsArchiveIndex {
    /// Indexed entries in sequential order (`159` entries in retail `voice.afs`).
    pub entries: Vec<AfsEntry>,
}

impl AfsArchiveIndex {
    /// Returns the entry descriptor at `index`, if present.
    pub fn get(&self, index: usize) -> Option<&AfsEntry> {
        self.entries.get(index)
    }

    /// Extracts the raw byte slice for `index` from the backing `.afs` buffer.
    pub fn slice_entry<'a>(
        &self,
        afs_data: &'a [u8],
        index: usize,
    ) -> Result<&'a [u8], AssetError> {
        let entry = self.entries.get(index).ok_or_else(|| {
            AssetError::InvalidData(format!(
                "AFS entry index {index} out of bounds (count = {})",
                self.entries.len()
            ))
        })?;
        let start = entry.offset as usize;
        let end = start.saturating_add(entry.length as usize);
        if end > afs_data.len() {
            return Err(AssetError::BufferUnderflow {
                required: end,
                available: afs_data.len(),
            });
        }
        Ok(&afs_data[start..end])
    }
}

/// Parses the header and offset/size directory table of an `AFS\0` archive (`voice.afs`).
///
/// Format:
/// - `0x00..0x04`: `"AFS\0"` (`[0x41, 0x46, 0x53, 0x00]`)
/// - `0x04..0x08`: `u32` little-endian `entry_count` (`159` in `voice.afs`)
/// - `0x08..`: `entry_count` pairs of `(offset: u32, length: u32)` little-endian.
pub fn parse_afs_index(data: &[u8]) -> Result<AfsArchiveIndex, AssetError> {
    if data.len() < 8 {
        return Err(AssetError::BufferUnderflow {
            required: 8,
            available: data.len(),
        });
    }
    if data[0..4] != AFS_MAGIC {
        return Err(AssetError::InvalidMagic {
            expected: &AFS_MAGIC,
            found: data[0..4].to_vec(),
        });
    }

    let count = read_u32(data, 4)? as usize;
    let table_end = 8usize.saturating_add(count.saturating_mul(8));
    if table_end > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: table_end,
            available: data.len(),
        });
    }

    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let p = 8 + index * 8;
        let offset = read_u32(data, p)?;
        let length = read_u32(data, p + 4)?;
        entries.push(AfsEntry {
            index,
            offset,
            length,
        });
    }

    Ok(AfsArchiveIndex { entries })
}

/// Extracts and decodes an audio entry from an `AFS\0` archive (`voice.afs`) into a [`WavEntry`].
///
/// Supports both:
/// - Standard CRI ADX streams (`0x8000` header magic, used in `data/sound/adx/voice/voice.afs` and most `sh2e` entries), and
/// - Embedded RIFF WAVE streams (`"RIFF"..."WAVE"`, used in `sh2e/sound/adx/voice/voice.afs` for entries like `17`, `24`, and `35`).
pub fn decode_afs_entry_to_wav(
    afs_data: &[u8],
    entry_index: usize,
) -> Result<WavEntry, AssetError> {
    let index = parse_afs_index(afs_data)?;
    let slice = index.slice_entry(afs_data, entry_index)?;
    decode_afs_slice_to_wav(slice, entry_index as u32)
}

/// Decodes a raw slice extracted from an `AFS\0` entry (either `RIFF....WAVE` or CRI ADX `0x8000`) into a [`WavEntry`].
pub fn decode_afs_slice_to_wav(slice: &[u8], id: u32) -> Result<WavEntry, AssetError> {
    if slice.len() >= 12 && slice[0..4] == RIFF_MAGIC && slice[8..12] == WAVE_MAGIC {
        return parse_single_wav(slice, id);
    }

    let adx = super::adx::decode_adx(slice)?;
    let mut pcm_bytes = Vec::with_capacity(adx.pcm_i16.len() * 2);
    for sample in &adx.pcm_i16 {
        pcm_bytes.extend_from_slice(&sample.to_le_bytes());
    }

    Ok(WavEntry {
        id,
        channels: adx.header.channels,
        sample_rate: adx.header.sample_rate,
        bits_per_sample: 16,
        data: pcm_bytes,
        loops: adx.header.has_loop,
        loop_start: adx.header.loop_start_sample,
        loop_end: adx.header.loop_end_sample,
    })
}

/// Extracted sound effect waveform entry.
#[derive(Debug, Clone)]
pub struct WavEntry {
    /// Sound or program ID if indexed in bank.
    pub id: u32,
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
    /// Start loop sample offset if looping.
    pub loop_start: u32,
    /// End loop sample offset if looping.
    pub loop_end: u32,
}

/// Metadata header entry parsed from `.sdm` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdmEntryHeader {
    /// Sound sample index.
    pub sample_index: u32,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Byte offset within `.sdb` sample payload.
    pub data_offset: u32,
    /// Length of audio sample data in bytes.
    pub data_length: u32,
    /// Audio channel count (1 = Mono, 2 = Stereo).
    pub channels: u16,
    /// Bit depth per sample (typically 16-bit).
    pub bits_per_sample: u16,
    /// Whether this sample loops.
    pub loops: bool,
    /// Loop start offset in samples.
    pub loop_start: u32,
    /// Loop end offset in samples.
    pub loop_end: u32,
}

/// Collection of sound effect waveforms parsed from `sddata.bin` or `.sdm`/`.sdb`.
#[derive(Debug, Clone, Default)]
pub struct SoundBank {
    /// Audio sample entries in sequential file order.
    pub entries: Vec<WavEntry>,
}

impl SoundBank {
    /// Finds a sound effect entry by its index.
    pub fn get_entry(&self, index: usize) -> Option<&WavEntry> {
        self.entries.get(index)
    }

    /// Finds a sound effect entry by its assigned ID.
    pub fn get_by_id(&self, id: u32) -> Option<&WavEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

/// Parses sound data from `sddata.bin`, which packs RIFF WAVE files back-to-back.
///
/// In the retail PC and SH2:EE sound packs, `sddata.bin` contains 417 RIFF WAVE audio streams.
/// Because some enhanced WAV containers append metadata chunks with differing padding lengths,
/// robust ingestion locates each authentic `RIFF....WAVE` stream boundary.
pub fn parse_sddata(data: &[u8]) -> Result<SoundBank, AssetError> {
    // 1. Locate all valid RIFF WAVE stream start positions
    let mut riff_positions = Vec::with_capacity(417);
    let mut p = 0usize;
    while p + 12 <= data.len() {
        if data[p..p + 4] == RIFF_MAGIC && data[p + 8..p + 12] == WAVE_MAGIC {
            riff_positions.push(p);
            p += 12;
        } else {
            p += 1;
        }
    }

    let mut entries = Vec::with_capacity(riff_positions.len());
    for (idx, &start_pos) in riff_positions.iter().enumerate() {
        let end_pos = if idx + 1 < riff_positions.len() {
            riff_positions[idx + 1]
        } else {
            data.len()
        };

        let wav_slice = &data[start_pos..end_pos];
        if let Ok(mut entry) = parse_single_wav(wav_slice, idx as u32) {
            // Check trailing 2 bytes before next stream for loop flag (`[b0, 0x01]`).
            // Also note: in retail/SH2:EE `sddata.bin` (`mergebin.cpp`), entries `350..=366` and `416`
            // are the 18 continuous looping sound effects (`0x517820` / `AudioScripts/SFX/README.md`).
            if (end_pos >= 2 && data[end_pos - 1] == 1) || (350..=366).contains(&idx) || idx == 416
            {
                entry.loops = true;
            }

            // Chainsaw idle loop (`sddata[352]`, internal ID `0x1642`) skips its startup rev transient
            // when looping (`ChainsawSoundFix.cpp`: `0x4796` bytes at 16 kHz or `0x12180` bytes at 48 kHz).
            if idx == 352 && entry.loops && entry.loop_start == 0 {
                let bytes_per_sample =
                    (entry.channels.max(1) as u32) * ((entry.bits_per_sample.max(8) as u32) / 8);
                let skip_bytes: u32 = if entry.sample_rate >= 44100 {
                    0x12180
                } else {
                    0x4796
                };
                if (skip_bytes as usize) < entry.data.len() && bytes_per_sample > 0 {
                    entry.loop_start = skip_bytes / bytes_per_sample;
                }
            }
            entries.push(entry);
        }
    }

    Ok(SoundBank { entries })
}

/// Parses a paired `.sdm` (metadata header) and `.sdb` (sample data bank) file.
pub fn parse_sdm_sdb(sdm_data: &[u8], sdb_data: &[u8]) -> Result<SoundBank, AssetError> {
    let headers = parse_sdm_headers(sdm_data)?;
    let mut entries = Vec::with_capacity(headers.len());

    for (idx, hdr) in headers.into_iter().enumerate() {
        let start = hdr.data_offset as usize;
        let end = start + hdr.data_length as usize;

        let sample_bytes = if start < sdb_data.len() {
            let actual_end = end.min(sdb_data.len());
            sdb_data[start..actual_end].to_vec()
        } else {
            Vec::new()
        };

        entries.push(WavEntry {
            id: hdr.sample_index.max(idx as u32),
            channels: hdr.channels,
            sample_rate: hdr.sample_rate,
            bits_per_sample: hdr.bits_per_sample,
            data: sample_bytes,
            loops: hdr.loops,
            loop_start: hdr.loop_start,
            loop_end: hdr.loop_end,
        });
    }

    Ok(SoundBank { entries })
}

/// Parses headers from a `.sdm` sound metadata binary file.
///
/// Supports either explicit `SDM1` magic headers or table-driven sample records:
/// `[u32 count], [Entry: sample_rate(u32), offset(u32), length(u32), channels(u16), bits(u16), loop_flag(u16), pad(u16), loop_start(u32), loop_end(u32)]` (32 bytes per entry).
pub fn parse_sdm_headers(data: &[u8]) -> Result<Vec<SdmEntryHeader>, AssetError> {
    if data.len() < 4 {
        return Err(AssetError::BufferUnderflow {
            required: 4,
            available: data.len(),
        });
    }

    let mut p = 0usize;
    if data[0..4] == SDM_MAGIC {
        p += 4;
    }

    if p + 4 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: p + 4,
            available: data.len(),
        });
    }

    let count = read_u32(data, p)? as usize;
    p += 4;

    let mut headers = Vec::with_capacity(count);
    const ENTRY_SIZE: usize = 28;

    for i in 0..count {
        if p + ENTRY_SIZE > data.len() {
            break;
        }

        let sample_rate = read_u32(data, p)?;
        let data_offset = read_u32(data, p + 4)?;
        let data_length = read_u32(data, p + 8)?;
        let channels = read_u16(data, p + 12)?;
        let bits_per_sample = read_u16(data, p + 14)?;
        let loop_flag = read_u16(data, p + 16)?;
        let _pad = read_u16(data, p + 18)?;
        let loop_start = read_u32(data, p + 20)?;
        let loop_end = read_u32(data, p + 24)?;

        headers.push(SdmEntryHeader {
            sample_index: i as u32,
            sample_rate,
            data_offset,
            data_length,
            channels: channels.max(1),
            bits_per_sample: if bits_per_sample == 0 {
                16
            } else {
                bits_per_sample
            },
            loops: loop_flag != 0,
            loop_start,
            loop_end,
        });

        p += ENTRY_SIZE;
    }

    Ok(headers)
}

/// Parses a single RIFF WAVE container and extracts channel, rate, depth, and PCM data.
fn parse_single_wav(data: &[u8], id: u32) -> Result<WavEntry, AssetError> {
    let mut p = 12usize;
    let mut channels = 1u16;
    let mut sample_rate = 22050u32;
    let mut bits_per_sample = 16u16;
    let mut pcm_data = Vec::new();
    let mut loops = false;

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
            // In `sh2e/sound/sddata.bin` (48 kHz lossless), the 2-byte Konami loop flag (`[b0, 0x01]`)
            // is placed immediately after the `data` chunk payload (before an optional `LIST....INFO` chunk).
            let after_data = chunk_data + chunk_len;
            if after_data + 2 <= data.len() && data[after_data + 1] == 1 {
                loops = true;
            }
        }

        p = chunk_data + chunk_len;
        // Chunks are 2-byte aligned
        if !p.is_multiple_of(2) {
            p += 1;
        }
    }

    Ok(WavEntry {
        id,
        channels,
        sample_rate,
        bits_per_sample,
        data: pcm_data,
        loops,
        loop_start: 0,
        loop_end: 0,
    })
}
