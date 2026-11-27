//! Parser, demuxer, and incremental multi-layer streaming decoder for CRI AIX containers (`.aix`).
//!
//! # Format Architecture
//! CRI AIX (`AIXF` / `AIXP` / `AIXE`) is an interleaved multi-stream container format used
//! extensively in Silent Hill 2 for 5-to-7-layer dynamic background music and ambient soundscapes.
//! Each layer inside an AIX file is a full CRI ADX audio stream (`0x8000` magic) interleaved in
//! small `AIXP` packets so all layers can be decoded incrementally in sample-accurate lockstep
//! without buffering the entire file in RAM (`SH2-180`).

use crate::error::AssetError;
use crate::formats::adx::{
    decode_adx, decode_adx_channel_block, parse_adx_header, AdxChannelState, AdxFilterCoeffs,
    AdxHeader, DecodedAdx, ADX_BLOCK_BYTES, ADX_MAX_CHANNELS, ADX_SAMPLES_PER_BLOCK,
};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

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

/// Decodes and downmixes all non-silent layers of an AIX file into a single stereo [`DecodedAdx`].
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

    for frame in &mut mixed_frames {
        frame.0 = frame.0.clamp(-1.0, 1.0);
        frame.1 = frame.1.clamp(-1.0, 1.0);
    }

    let mut header = layers[0].header.clone();
    header.total_samples = max_len as u32;

    Ok(DecodedAdx {
        header,
        frames: mixed_frames,
    })
}

/// Per-layer incremental ADX decoder state inside [`AixStreamDecoder`].
struct AixLayerState {
    header: Option<AdxHeader>,
    coeffs: AdxFilterCoeffs,
    hist: [AdxChannelState; ADX_MAX_CHANNELS],
    raw_buf: Vec<u8>,
    raw_cursor: usize,
    header_skipped: bool,
    samples_decoded: u64,
    pcm_queue: VecDeque<(f32, f32)>,
}

impl AixLayerState {
    fn new() -> Self {
        Self {
            header: None,
            coeffs: AdxFilterCoeffs { c1: 0, c2: 0 },
            hist: [AdxChannelState::default(); ADX_MAX_CHANNELS],
            raw_buf: Vec::with_capacity(4096),
            raw_cursor: 0,
            header_skipped: false,
            samples_decoded: 0,
            pcm_queue: VecDeque::with_capacity(3072),
        }
    }

    fn reset_for_loop(&mut self) {
        self.hist = [AdxChannelState::default(); ADX_MAX_CHANNELS];
        self.raw_buf.clear();
        self.raw_cursor = 0;
        self.header_skipped = false;
        self.samples_decoded = 0;
        self.pcm_queue.clear();
    }

    fn push_payload(&mut self, payload: &[u8]) -> Result<(), AssetError> {
        self.raw_buf.extend_from_slice(payload);

        // Parse embedded ADX header once enough leading bytes have arrived.
        if !self.header_skipped {
            let avail = &self.raw_buf[self.raw_cursor..];
            if avail.len() < 0x20 {
                return Ok(());
            }
            let copyright_ofs = u16::from_be_bytes([avail[2], avail[3]]) as usize;
            let data_offset = copyright_ofs + 4;
            if avail.len() < data_offset {
                return Ok(());
            }
            let header = parse_adx_header(avail)?;
            self.coeffs = AdxFilterCoeffs::from_header(&header);
            self.raw_cursor += header.data_offset;
            self.header = Some(header);
            self.header_skipped = true;
        }

        // Decode all complete ADX packets currently buffered for this layer.
        let Some(ref header) = self.header else {
            return Ok(());
        };
        let channels = header.channels as usize;
        let packet_bytes = ADX_BLOCK_BYTES * channels;
        let total_samples = header.total_samples as u64;
        let is_v3 = header.version == 3;
        let mut decoded_i16 = [[0i16; ADX_SAMPLES_PER_BLOCK]; ADX_MAX_CHANNELS];

        while self.samples_decoded < total_samples
            && self.raw_buf.len() - self.raw_cursor >= packet_bytes
        {
            let count =
                ((total_samples - self.samples_decoded) as usize).min(ADX_SAMPLES_PER_BLOCK);
            for (k, ch_out) in decoded_i16.iter_mut().enumerate().take(channels) {
                let start = self.raw_cursor + k * ADX_BLOCK_BYTES;
                let mut block = [0u8; ADX_BLOCK_BYTES];
                block.copy_from_slice(&self.raw_buf[start..start + ADX_BLOCK_BYTES]);
                decode_adx_channel_block(
                    &block,
                    self.coeffs,
                    is_v3,
                    &mut self.hist[k],
                    count,
                    ch_out,
                );
            }

            for i in 0..count {
                let left = decoded_i16[0][i] as f32 / 32768.0;
                let right = if channels > 1 {
                    decoded_i16[1][i] as f32 / 32768.0
                } else {
                    left
                };
                self.pcm_queue.push_back((left, right));
            }

            self.raw_cursor += packet_bytes;
            self.samples_decoded += count as u64;
        }

        // Compact consumed bytes so `raw_buf` stays tiny (< 4 KiB).
        if self.raw_cursor > 0 {
            if self.raw_cursor >= self.raw_buf.len() {
                self.raw_buf.clear();
            } else {
                self.raw_buf.drain(..self.raw_cursor);
            }
            self.raw_cursor = 0;
        }

        Ok(())
    }
}

/// Incremental multi-layer streaming decoder for CRI AIX containers (`.aix`) (`SH2-180`).
///
/// Streams interleaved `AIXP` chunks directly from a seekable reader (such as a `BufReader<File>`),
/// decoding all layers in sample-accurate lockstep with ~170 KiB resident memory instead of
/// decoding 300+ MiB of uncompressed PCM up front.
pub struct AixStreamDecoder<R: Read + Seek> {
    reader: R,
    first_packet_pos: u64,
    current_pos: u64,
    layers: Vec<AixLayerState>,
    payload_scratch: Vec<u8>,
    loop_enabled: bool,
    at_eof: bool,
}

impl AixStreamDecoder<BufReader<File>> {
    /// Opens an `.aix` file from disk for incremental multi-layer streaming.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AssetError> {
        let file = File::open(path)?;
        Self::new(BufReader::with_capacity(32 * 1024, file))
    }
}

impl<R: Read + Seek> AixStreamDecoder<R> {
    /// Creates a new streaming AIX decoder from any seekable byte reader and parses all layer headers.
    pub fn new(mut reader: R) -> Result<Self, AssetError> {
        reader.seek(SeekFrom::Start(0))?;
        let mut head = [0u8; 8];
        if !read_exact_or_eof(&mut reader, &mut head)? {
            return Err(AssetError::BufferUnderflow {
                required: 16,
                available: 0,
            });
        }
        if &head[0..4] != b"AIXF" {
            return Err(AssetError::InvalidMagic {
                expected: b"AIXF",
                found: head[0..4].to_vec(),
            });
        }
        let next_offset = u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as u64;
        let first_packet_pos = 8 + next_offset;
        reader.seek(SeekFrom::Start(first_packet_pos))?;

        let mut decoder = Self {
            reader,
            first_packet_pos,
            current_pos: first_packet_pos,
            layers: Vec::new(),
            payload_scratch: Vec::with_capacity(4096),
            loop_enabled: true,
            at_eof: false,
        };

        // Read leading `AIXP` chunks until all layers have parsed their embedded ADX headers.
        while !decoder.at_eof {
            decoder.read_next_chunk()?;
            if !decoder.layers.is_empty() && decoder.layers.iter().all(|l| l.header.is_some()) {
                break;
            }
        }

        if decoder.layers.is_empty() || decoder.layers.iter().any(|l| l.header.is_none()) {
            return Err(AssetError::BufferUnderflow {
                required: 32,
                available: 0,
            });
        }

        Ok(decoder)
    }

    /// Returns the number of synchronized ADX layers in this AIX container.
    #[inline]
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Returns the parsed [`AdxHeader`] of the given layer index, if in bounds.
    #[inline]
    #[must_use]
    pub fn layer_header(&self, layer_idx: usize) -> Option<&AdxHeader> {
        self.layers.get(layer_idx).and_then(|l| l.header.as_ref())
    }

    /// Returns the sample rate in Hz of the primary layer (all AIX layers share the same rate).
    #[inline]
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.layer_header(0).map(|h| h.sample_rate).unwrap_or(0)
    }

    /// Enables or disables seamless whole-stream looping across all synchronized layers.
    pub fn set_looping(&mut self, enabled: bool) {
        self.loop_enabled = enabled;
    }

    fn read_next_chunk(&mut self) -> Result<bool, AssetError> {
        let mut chunk_hdr = [0u8; 8];
        if !read_exact_or_eof(&mut self.reader, &mut chunk_hdr)? {
            self.at_eof = true;
            return Ok(false);
        }
        if &chunk_hdr[0..3] != b"AIX" {
            self.at_eof = true;
            return Ok(false);
        }

        let chunk_type = chunk_hdr[3];
        let next_offset =
            u32::from_be_bytes([chunk_hdr[4], chunk_hdr[5], chunk_hdr[6], chunk_hdr[7]]) as u64;
        let next_pos = self.current_pos + 8 + next_offset;

        if chunk_type == b'E' {
            self.at_eof = true;
            return Ok(false);
        }

        if chunk_type == b'P' {
            let mut pkt_meta = [0u8; 8];
            if !read_exact_or_eof(&mut self.reader, &mut pkt_meta)? {
                self.at_eof = true;
                return Ok(false);
            }
            let channel = pkt_meta[0] as usize;
            let channel_count = pkt_meta[1] as usize;
            let size = u16::from_be_bytes([pkt_meta[2], pkt_meta[3]]) as usize;

            if self.layers.len() < channel_count {
                self.layers.resize_with(channel_count, AixLayerState::new);
            }

            self.payload_scratch.resize(size, 0);
            if !read_exact_or_eof(&mut self.reader, &mut self.payload_scratch)? {
                self.at_eof = true;
                return Ok(false);
            }

            if let Some(layer) = self.layers.get_mut(channel) {
                layer.push_payload(&self.payload_scratch)?;
            }

            let consumed = self.current_pos + 16 + size as u64;
            if next_pos > consumed {
                self.reader.seek(SeekFrom::Start(next_pos))?;
            } else if next_pos < consumed {
                self.at_eof = true;
                return Ok(false);
            }
        } else {
            if next_pos <= self.current_pos {
                self.at_eof = true;
                return Ok(false);
            }
            self.reader.seek(SeekFrom::Start(next_pos))?;
        }

        self.current_pos = next_pos;
        Ok(true)
    }

    fn reset_to_loop_start(&mut self) -> Result<(), AssetError> {
        self.reader.seek(SeekFrom::Start(self.first_packet_pos))?;
        self.current_pos = self.first_packet_pos;
        self.at_eof = false;
        for layer in &mut self.layers {
            layer.reset_for_loop();
        }
        Ok(())
    }

    /// Reads up to `max_frames` synchronized stereo frames across all layers into `out_layers`.
    ///
    /// `out_layers` must have length `>= self.layer_count()`. Each `out_layers[k]` is cleared and
    /// populated with the exact same number `N` of decoded stereo `(left, right)` frames so all
    /// layers stay 100% sample-accurate synchronized across playback and loop wrap-around.
    /// Returns `N` (`0` only when non-looping playback reaches EOF).
    pub fn read_synchronized_frames(
        &mut self,
        out_layers: &mut [Vec<(f32, f32)>],
        max_frames: usize,
    ) -> Result<usize, AssetError> {
        let layer_count = self.layers.len();
        for buf in out_layers.iter_mut().take(layer_count) {
            buf.clear();
        }
        if layer_count == 0 || max_frames == 0 {
            return Ok(0);
        }

        let mut total_emitted = 0usize;
        while total_emitted < max_frames {
            let needed = max_frames - total_emitted;

            // Pump `AIXP` chunks until every layer has at least `needed` queued frames (or EOF).
            while !self.at_eof && self.layers.iter().any(|l| l.pcm_queue.len() < needed) {
                if !self.read_next_chunk()? {
                    break;
                }
            }

            let min_avail = self
                .layers
                .iter()
                .map(|l| l.pcm_queue.len())
                .min()
                .unwrap_or(0);
            let max_avail = self
                .layers
                .iter()
                .map(|l| l.pcm_queue.len())
                .max()
                .unwrap_or(0);

            let take = if min_avail > 0 {
                min_avail.min(needed)
            } else if self.at_eof && max_avail > 0 {
                // Drain any trailing frames on slightly longer layers, padding shorter ones with silence.
                max_avail.min(needed)
            } else {
                0
            };

            if take > 0 {
                for (k, layer) in self.layers.iter_mut().enumerate() {
                    let dst = &mut out_layers[k];
                    for _ in 0..take {
                        dst.push(layer.pcm_queue.pop_front().unwrap_or((0.0, 0.0)));
                    }
                }
                total_emitted += take;
                continue;
            }

            // Reached end of all queued frames at EOF.
            if self.at_eof && self.loop_enabled {
                self.reset_to_loop_start()?;
                // Ensure we don't spin forever on a degenerate empty stream.
                let mut made_progress = false;
                while !self.at_eof {
                    if !self.read_next_chunk()? {
                        break;
                    }
                    if self.layers.iter().any(|l| !l.pcm_queue.is_empty()) {
                        made_progress = true;
                        break;
                    }
                }
                if !made_progress {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(total_emitted)
    }
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool, AssetError> {
    let mut filled = 0usize;
    while filled < buf.len() {
        let n = reader.read(&mut buf[filled..])?;
        if n == 0 {
            return Ok(false);
        }
        filled += n;
    }
    Ok(true)
}
