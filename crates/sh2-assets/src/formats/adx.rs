//! Parser, batch decoder, and incremental streaming decoder for CRI ADX audio streams (`.adx`).
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
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

/// Number of PCM samples encoded in a single 18-byte CRI ADX ADPCM channel block.
pub const ADX_SAMPLES_PER_BLOCK: usize = 32;

/// Byte size of a single CRI ADX ADPCM channel block (2-byte scale + 16 bytes of 4-bit nibbles).
pub const ADX_BLOCK_BYTES: usize = 18;

/// Maximum channel count supported by CRI ADX streams.
pub const ADX_MAX_CHANNELS: usize = 8;

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

/// Decoded ADX audio track containing normalized stereo PCM frames and header metadata (`SH2-180`).
#[derive(Debug, Clone)]
pub struct DecodedAdx {
    /// Header metadata parsed from stream.
    pub header: AdxHeader,
    /// Normalized stereo floating-point frames `(left, right)` in `[-1.0, 1.0]`.
    pub frames: Vec<(f32, f32)>,
}

/// Fixed-point Q12 ADPCM prediction filter coefficients derived from an [`AdxHeader`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdxFilterCoeffs {
    /// First-order history coefficient (`c * 8192` truncated to `i32`).
    pub c1: i32,
    /// Second-order history coefficient (`-c^2 * 4096` truncated to `i32`).
    pub c2: i32,
}

impl AdxFilterCoeffs {
    /// Computes the standard CRI ADX second-order prediction filter coefficients from `header`.
    #[must_use]
    pub fn from_header(header: &AdxHeader) -> Self {
        let rate = header.sample_rate.max(1) as f64;
        let hp = header.highpass_frequency as f64;
        let z = (2.0 * PI * hp / rate).cos();
        let a = std::f64::consts::SQRT_2 - z;
        let b = std::f64::consts::SQRT_2 - 1.0;
        let c = (a - ((a + b) * (a - b)).max(0.0).sqrt()) / b;
        Self {
            c1: (c * 8192.0) as i32,
            c2: (c * c * -4096.0) as i32,
        }
    }
}

/// Per-channel ADPCM decoder history (`h1 = s[n-1]`, `h2 = s[n-2]`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdxChannelState {
    /// Most recent decoded sample `s[n-1]`.
    pub h1: i32,
    /// Second most recent decoded sample `s[n-2]`.
    pub h2: i32,
}

/// Decodes a single 18-byte CRI ADX ADPCM block (up to 32 samples) for one channel.
///
/// Updates `state` in place and writes `sample_count.min(32)` decoded `i16` samples into `out`.
pub fn decode_adx_channel_block(
    block: &[u8; ADX_BLOCK_BYTES],
    coeffs: AdxFilterCoeffs,
    is_v3: bool,
    state: &mut AdxChannelState,
    sample_count: usize,
    out: &mut [i16; ADX_SAMPLES_PER_BLOCK],
) {
    let raw_scale = i16::from_be_bytes([block[0], block[1]]) as i32;
    let scale = raw_scale + 1;
    let mut h1 = state.h1;
    let mut h2 = state.h2;
    let count = sample_count.min(ADX_SAMPLES_PER_BLOCK);

    for (i, slot) in out.iter_mut().enumerate().take(count) {
        let byte_idx = 2 + (i / 2);
        let b = block[byte_idx];
        let mut nib = if (i & 1) == 0 {
            (b >> 4) as i32
        } else {
            (b & 0x0F) as i32
        };
        if (nib & 8) != 0 {
            nib -= 16;
        }

        let v = if is_v3 {
            nib * scale + ((coeffs.c1 * h1) >> 12) + ((coeffs.c2 * h2) >> 12)
        } else {
            nib * scale + ((coeffs.c1 * h1 + coeffs.c2 * h2) >> 12)
        };

        let clamped = v.clamp(-32768, 32767) as i16;
        *slot = clamped;
        h2 = h1;
        h1 = clamped as i32;
    }

    state.h1 = h1;
    state.h2 = h2;
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

    if encoding_type != 3 || block_size != ADX_BLOCK_BYTES as u8 || bit_depth != 4 {
        return Err(AssetError::UnsupportedFormat(
            "Only standard 4-bit CRI ADX streams (type 3, block size 18) are supported".to_string(),
        ));
    }

    if channels == 0 || channels > ADX_MAX_CHANNELS as u16 {
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

/// Incremental streaming CRI ADX decoder that reads 18-byte ADPCM blocks on demand (`SH2-180`).
///
/// Supports seamless sample-accurate looping via ADX header loop markers (`loop_start_sample`..`loop_end_sample`)
/// or whole-stream looping without materializing the full uncompressed PCM buffer in RAM.
pub struct AdxStreamDecoder<R: Read + Seek> {
    reader: R,
    header: AdxHeader,
    coeffs: AdxFilterCoeffs,
    hist: [AdxChannelState; ADX_MAX_CHANNELS],
    loop_start_hist: Option<[AdxChannelState; ADX_MAX_CHANNELS]>,
    loop_enabled: bool,
    loop_start_sample: u64,
    loop_end_sample: u64,
    current_sample: u64,
    packet_buf: [u8; ADX_BLOCK_BYTES * ADX_MAX_CHANNELS],
    decoded_block: [(f32, f32); ADX_SAMPLES_PER_BLOCK],
    decoded_i16: [[i16; ADX_SAMPLES_PER_BLOCK]; ADX_MAX_CHANNELS],
    block_cursor: usize,
    block_len: usize,
    eof: bool,
}

impl AdxStreamDecoder<BufReader<File>> {
    /// Opens an `.adx` file from disk for incremental streaming with a 16 KiB read buffer.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AssetError> {
        let file = File::open(path)?;
        Self::new(BufReader::with_capacity(16 * 1024, file))
    }
}

impl<R: Read + Seek> AdxStreamDecoder<R> {
    /// Creates a new incremental ADX stream decoder from any seekable byte reader.
    pub fn new(mut reader: R) -> Result<Self, AssetError> {
        reader.seek(SeekFrom::Start(0))?;
        let mut hdr_buf = [0u8; 64];
        let mut read_total = 0usize;
        while read_total < hdr_buf.len() {
            let n = reader.read(&mut hdr_buf[read_total..])?;
            if n == 0 {
                break;
            }
            read_total += n;
        }
        let header = parse_adx_header(&hdr_buf[..read_total])?;
        reader.seek(SeekFrom::Start(header.data_offset as u64))?;

        let coeffs = AdxFilterCoeffs::from_header(&header);
        let loop_enabled = header.has_loop
            && header.loop_end_sample > header.loop_start_sample
            && header.total_samples > 0;
        let loop_start_sample = header.loop_start_sample as u64;
        let loop_end_sample = if loop_enabled {
            (header.loop_end_sample as u64).min(header.total_samples as u64)
        } else {
            header.total_samples as u64
        };

        Ok(Self {
            reader,
            header,
            coeffs,
            hist: [AdxChannelState::default(); ADX_MAX_CHANNELS],
            loop_start_hist: None,
            loop_enabled,
            loop_start_sample,
            loop_end_sample,
            current_sample: 0,
            packet_buf: [0u8; ADX_BLOCK_BYTES * ADX_MAX_CHANNELS],
            decoded_block: [(0.0, 0.0); ADX_SAMPLES_PER_BLOCK],
            decoded_i16: [[0i16; ADX_SAMPLES_PER_BLOCK]; ADX_MAX_CHANNELS],
            block_cursor: 0,
            block_len: 0,
            eof: false,
        })
    }

    /// Returns the parsed [`AdxHeader`] metadata.
    #[inline]
    #[must_use]
    pub fn header(&self) -> &AdxHeader {
        &self.header
    }

    /// Enables or disables seamless looping.
    ///
    /// When `enabled` is `true` and the header has no explicit loop points, loops the entire
    /// stream from `0..total_samples`. When `enabled` is `false`, plays once to `total_samples`.
    pub fn set_looping(&mut self, enabled: bool) {
        if !enabled {
            self.loop_enabled = false;
            return;
        }
        if self.header.has_loop && self.header.loop_end_sample > self.header.loop_start_sample {
            self.loop_enabled = true;
            self.loop_start_sample = self.header.loop_start_sample as u64;
            self.loop_end_sample =
                (self.header.loop_end_sample as u64).min(self.header.total_samples as u64);
        } else if self.header.total_samples > 0 {
            self.loop_enabled = true;
            self.loop_start_sample = 0;
            self.loop_end_sample = self.header.total_samples as u64;
        }
    }

    /// Returns the current playback position in samples from stream start.
    #[inline]
    #[must_use]
    pub fn current_sample(&self) -> u64 {
        self.current_sample
    }

    fn seek_to_loop_start(&mut self) -> Result<(), AssetError> {
        let channels = self.header.channels as u64;
        let start_packet = self.loop_start_sample / ADX_SAMPLES_PER_BLOCK as u64;
        let intra_offset = (self.loop_start_sample % ADX_SAMPLES_PER_BLOCK as u64) as usize;
        let byte_pos =
            self.header.data_offset as u64 + start_packet * (ADX_BLOCK_BYTES as u64) * channels;
        self.reader.seek(SeekFrom::Start(byte_pos))?;
        self.hist = self.loop_start_hist.unwrap_or_default();
        self.current_sample = start_packet * ADX_SAMPLES_PER_BLOCK as u64;
        self.block_cursor = 0;
        self.block_len = 0;
        self.eof = false;

        if intra_offset > 0 && self.decode_next_packet()? {
            let skip = intra_offset.min(self.block_len);
            self.block_cursor = skip;
            self.current_sample += skip as u64;
        }
        Ok(())
    }

    /// Decodes the next 32-sample ADX packet into `self.decoded_block` and `self.decoded_i16`.
    /// Returns `Ok(false)` on EOF (after checking `total_samples` / `loop_end_sample`).
    fn decode_next_packet(&mut self) -> Result<bool, AssetError> {
        let limit = if self.loop_enabled {
            self.loop_end_sample
        } else {
            self.header.total_samples as u64
        };

        if self.current_sample >= limit {
            return Ok(false);
        }

        let channels = self.header.channels as usize;
        let packet_bytes = ADX_BLOCK_BYTES * channels;

        // Snapshot ADPCM history at the packet containing `loop_start_sample` on first pass
        // so subsequent loop wraps produce bit-exact samples.
        if self.loop_enabled && self.loop_start_hist.is_none() {
            let loop_packet_sample = (self.loop_start_sample / ADX_SAMPLES_PER_BLOCK as u64)
                * ADX_SAMPLES_PER_BLOCK as u64;
            if self.current_sample == loop_packet_sample {
                self.loop_start_hist = Some(self.hist);
            }
        }

        // Read exactly one packet; treat partial read at EOF cleanly.
        let mut filled = 0usize;
        while filled < packet_bytes {
            let n = self
                .reader
                .read(&mut self.packet_buf[filled..packet_bytes])?;
            if n == 0 {
                return Ok(false);
            }
            filled += n;
        }

        let remaining = (limit - self.current_sample) as usize;
        let count = remaining.min(ADX_SAMPLES_PER_BLOCK);
        let is_v3 = self.header.version == 3;

        for k in 0..channels {
            let mut block = [0u8; ADX_BLOCK_BYTES];
            block.copy_from_slice(&self.packet_buf[k * ADX_BLOCK_BYTES..(k + 1) * ADX_BLOCK_BYTES]);
            decode_adx_channel_block(
                &block,
                self.coeffs,
                is_v3,
                &mut self.hist[k],
                count,
                &mut self.decoded_i16[k],
            );
        }

        for i in 0..count {
            let left = self.decoded_i16[0][i] as f32 / 32768.0;
            let right = if channels > 1 {
                self.decoded_i16[1][i] as f32 / 32768.0
            } else {
                left
            };
            self.decoded_block[i] = (left, right);
        }

        self.block_cursor = 0;
        self.block_len = count;
        Ok(true)
    }

    /// Reads up to `out.len()` decoded normalized stereo `(left, right)` frames into `out`.
    ///
    /// When looping is enabled, seamlessly wraps at `loop_end_sample` back to `loop_start_sample`
    /// within the same call. Returns `0` only when non-looping playback reaches EOF.
    pub fn read_frames(&mut self, out: &mut [(f32, f32)]) -> Result<usize, AssetError> {
        if self.eof || out.is_empty() {
            return Ok(0);
        }

        let mut written = 0usize;
        while written < out.len() {
            if self.block_cursor < self.block_len {
                let avail = self.block_len - self.block_cursor;
                let take = avail.min(out.len() - written);
                out[written..written + take].copy_from_slice(
                    &self.decoded_block[self.block_cursor..self.block_cursor + take],
                );
                self.block_cursor += take;
                self.current_sample += take as u64;
                written += take;
                continue;
            }

            if !self.decode_next_packet()? {
                if self.loop_enabled && self.loop_end_sample > self.loop_start_sample {
                    self.seek_to_loop_start()?;
                    if self.block_cursor >= self.block_len && !self.decode_next_packet()? {
                        self.eof = true;
                        break;
                    }
                } else {
                    self.eof = true;
                    break;
                }
            }
        }

        Ok(written)
    }

    /// Decodes the entire remaining non-looping stream into interleaved little-endian `i16` PCM bytes.
    fn decode_all_pcm_i16_le(&mut self) -> Result<Vec<u8>, AssetError> {
        self.loop_enabled = false;
        let channels = self.header.channels as usize;
        let cap = (self.header.total_samples as usize).saturating_mul(channels.min(2) * 2);
        let mut out = Vec::with_capacity(cap);
        while self.decode_next_packet()? {
            for i in 0..self.block_len {
                let left = self.decoded_i16[0][i];
                out.extend_from_slice(&left.to_le_bytes());
                if channels > 1 {
                    let right = self.decoded_i16[1][i];
                    out.extend_from_slice(&right.to_le_bytes());
                }
            }
            self.current_sample += self.block_len as u64;
            self.block_cursor = self.block_len;
        }
        Ok(out)
    }
}

/// Decodes an entire CRI ADX audio stream into normalized stereo floating-point frames (`SH2-180`).
pub fn decode_adx(data: &[u8]) -> Result<DecodedAdx, AssetError> {
    let mut decoder = AdxStreamDecoder::new(Cursor::new(data))?;
    decoder.set_looping(false);
    let header = decoder.header().clone();
    let mut frames = Vec::with_capacity(header.total_samples as usize);
    let mut chunk = [(0.0f32, 0.0f32); 1024];
    loop {
        let n = decoder.read_frames(&mut chunk)?;
        if n == 0 {
            break;
        }
        frames.extend_from_slice(&chunk[..n]);
    }
    Ok(DecodedAdx { header, frames })
}

/// Decodes a CRI ADX byte slice directly into `(AdxHeader, interleaved s16le PCM bytes)` without
/// allocating an intermediate `Vec<(f32, f32)>` buffer (`SH2-180`).
pub fn decode_adx_to_pcm_i16_le(data: &[u8]) -> Result<(AdxHeader, Vec<u8>), AssetError> {
    let mut decoder = AdxStreamDecoder::new(Cursor::new(data))?;
    let header = decoder.header().clone();
    let pcm_bytes = decoder.decode_all_pcm_i16_le()?;
    Ok((header, pcm_bytes))
}
