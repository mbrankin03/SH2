//! Sample-accurate multi-layer streaming BGM Kira [`Sound`] & [`SoundData`] (`SH2-180`).
//!
//! Decodes `.adx` (1-layer) and `.aix` (up to 8 synchronized layers) incrementally on a dedicated
//! background thread into bounded per-layer [`AudioRingBuffer`]s. A single real-time Kira `Sound`
//! pops all layers in lockstep and mixes them with smooth per-layer linear gain ramps, guaranteeing
//! zero inter-layer drift across playback, pause/resume, and loop boundaries with zero heap
//! allocations in `Sound::process`.

use std::convert::Infallible;
use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use kira::{
    clock::clock_info::ClockInfoProvider,
    modulator::value_provider::ModulatorValueProvider,
    sound::{Sound, SoundData},
    track::TrackId,
    Frame, OutputDestination,
};
use sh2_assets::{AdxStreamDecoder, AixStreamDecoder};

use crate::bgm_manager::BGM_MAX_TRACKS;
use crate::ringbuffer::AudioRingBuffer;

use super::bgm_control::{StreamingBgmControl, StreamingBgmHandle};
use super::{load_f32, load_f32_acquire, StreamingResampler48k};

/// Per-layer ringbuffer capacity in stereo frames (`16,384` frames ≈ `341 ms` at `48 kHz`).
///
/// For a 7-layer `.aix` track, total ringbuffer memory is `7 * 16,384 * 8 = 917 KiB` (< 1 MiB)
/// compared to ~322–800 MiB for full-file `StaticSoundData`.
pub const BGM_STREAM_RINGBUFFER_FRAMES: usize = 16_384;

/// Number of source frames decoded per background worker iteration.
const BGM_DECODE_CHUNK_FRAMES: usize = 512;

/// Number of stereo frames popped per lock-free batch inside `Sound::process`.
const AUDIO_CALLBACK_BATCH: usize = 64;

/// Custom multi-layer BGM producer callback type for deterministic unit tests.
pub type CustomBgmReadFn = Box<dyn FnMut(&mut [Vec<(f32, f32)>], usize) -> usize + Send>;

/// Backing incremental decoder source for [`StreamingBgmSoundData`].
pub enum StreamingBgmSource {
    /// Single-layer incremental `.adx` stream decoder.
    Adx(Box<AdxStreamDecoder<BufReader<File>>>),
    /// Multi-layer synchronized incremental `.aix` stream decoder.
    Aix(AixStreamDecoder<BufReader<File>>),
    /// Custom producer closure for deterministic unit tests without disk assets.
    Custom {
        /// Source sample rate in Hz.
        sample_rate: u32,
        /// Number of synchronized layers (`1..=BGM_MAX_TRACKS`).
        layer_count: usize,
        /// Callback filling `out_layers[0..layer_count]` with up to `max_frames` synchronized frames.
        read_fn: CustomBgmReadFn,
    },
}

impl StreamingBgmSource {
    /// Returns the number of synchronized layers (`1..=BGM_MAX_TRACKS`).
    #[must_use]
    pub fn layer_count(&self) -> usize {
        match self {
            Self::Adx(_) => 1,
            Self::Aix(dec) => dec.layer_count().clamp(1, BGM_MAX_TRACKS),
            Self::Custom { layer_count, .. } => (*layer_count).clamp(1, BGM_MAX_TRACKS),
        }
    }

    /// Returns the source sample rate in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        match self {
            Self::Adx(dec) => dec.header().sample_rate.max(8_000),
            Self::Aix(dec) => dec.sample_rate().max(8_000),
            Self::Custom { sample_rate, .. } => (*sample_rate).max(8_000),
        }
    }

    fn read_synchronized(
        &mut self,
        out_layers: &mut [Vec<(f32, f32)>],
        max_frames: usize,
    ) -> usize {
        match self {
            Self::Adx(dec) => {
                out_layers[0].resize(max_frames, (0.0, 0.0));
                match dec.read_frames(&mut out_layers[0]) {
                    Ok(n) => {
                        out_layers[0].truncate(n);
                        n
                    }
                    Err(_) => {
                        out_layers[0].clear();
                        0
                    }
                }
            }
            Self::Aix(dec) => dec
                .read_synchronized_frames(out_layers, max_frames)
                .unwrap_or(0),
            Self::Custom { read_fn, .. } => read_fn(out_layers, max_frames),
        }
    }
}

/// Kira [`SoundData`] descriptor for a streaming single-layer ADX or multi-layer AIX BGM stream (`SH2-180`).
pub struct StreamingBgmSoundData {
    /// Incremental ADX/AIX source decoded on the background thread.
    pub source: StreamingBgmSource,
    /// Initial linear gain per layer (`0.0..=1.0`).
    pub initial_gains: Vec<f32>,
    /// Initial master gain multiplier (`0.0` if fading in).
    pub initial_master_gain: f32,
    /// Target master gain multiplier after `fade_in`.
    pub target_master_gain: f32,
    /// Optional fade-in duration from `initial_master_gain` to `target_master_gain`.
    pub fade_in: Duration,
    /// Destination Kira BGM sub-track ID.
    pub destination: TrackId,
}

impl SoundData for StreamingBgmSoundData {
    type Error = Infallible;
    type Handle = StreamingBgmHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let layer_count = self.source.layer_count();
        let src_rate = self.source.sample_rate();
        let control =
            StreamingBgmControl::new(layer_count, &self.initial_gains, self.initial_master_gain);
        if !self.fade_in.is_zero() {
            control.set_master_gain(self.target_master_gain, self.fade_in);
        } else {
            control.set_master_gain(self.target_master_gain, Duration::ZERO);
        }

        let mut layer_buffers = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            layer_buffers.push(Arc::new(AudioRingBuffer::with_capacity(
                BGM_STREAM_RINGBUFFER_FRAMES,
            )));
        }

        let mut resamplers: Vec<StreamingResampler48k> = (0..layer_count)
            .map(|_| StreamingResampler48k::new(src_rate, 0.78))
            .collect();
        let out_sample_rate = resamplers[0].output_sample_rate();

        // Pre-fill one chunk synchronously (~10 ms of audio, < 0.2 ms decode) so the first
        // audio callback starts with audio ready and zero initial underrun.
        let mut source = self.source;
        let mut raw_chunks: Vec<Vec<(f32, f32)>> = (0..layer_count)
            .map(|_| Vec::with_capacity(BGM_DECODE_CHUNK_FRAMES))
            .collect();
        let mut resampled: Vec<Vec<Frame>> = (0..layer_count)
            .map(|_| Vec::with_capacity(BGM_DECODE_CHUNK_FRAMES * 2))
            .collect();

        let prefill_n = source.read_synchronized(&mut raw_chunks, BGM_DECODE_CHUNK_FRAMES);
        if prefill_n > 0 {
            let mut min_out = usize::MAX;
            for k in 0..layer_count {
                resamplers[k].process_chunk(&raw_chunks[k], &mut resampled[k]);
                min_out = min_out.min(resampled[k].len());
            }
            if min_out > 0 && min_out != usize::MAX {
                for k in 0..layer_count {
                    layer_buffers[k].push_frames(&resampled[k][..min_out]);
                }
            }
        } else {
            control.eof_reached.store(true, Ordering::Release);
        }

        let worker_buffers = layer_buffers.clone();
        let worker_control = Arc::clone(&control);
        let worker = thread::Builder::new()
            .name("sh2-bgm-decoder".to_string())
            .spawn(move || {
                bgm_decoder_loop(
                    source,
                    resamplers,
                    raw_chunks,
                    resampled,
                    worker_buffers,
                    worker_control,
                );
            })
            .ok();

        let mut current_layer_gains = [0.0f32; BGM_MAX_TRACKS];
        for (i, slot) in current_layer_gains.iter_mut().enumerate().take(layer_count) {
            *slot = self.initial_gains.get(i).copied().unwrap_or(0.0).max(0.0);
        }

        let sound = StreamingBgmSound {
            layer_buffers,
            layer_count,
            sample_rate: out_sample_rate,
            control: Arc::clone(&control),
            destination: OutputDestination::Track(self.destination),
            current_layer_gains,
            current_master_gain: self.initial_master_gain.max(0.0),
            stop_envelope: 1.0,
            local_batches: [[Frame::ZERO; AUDIO_CALLBACK_BATCH]; BGM_MAX_TRACKS],
            batch_cursor: 0,
            batch_len: 0,
            prev_frames: [Frame::ZERO; BGM_MAX_TRACKS],
            curr_frames: [Frame::ZERO; BGM_MAX_TRACKS],
            has_started: false,
            sample_phase: 0.0,
            is_finished: false,
        };

        let handle = StreamingBgmHandle {
            control,
            _worker: worker,
        };
        Ok((Box::new(sound), handle))
    }
}

fn bgm_decoder_loop(
    mut source: StreamingBgmSource,
    mut resamplers: Vec<StreamingResampler48k>,
    mut raw_chunks: Vec<Vec<(f32, f32)>>,
    mut resampled: Vec<Vec<Frame>>,
    buffers: Vec<Arc<AudioRingBuffer>>,
    control: Arc<StreamingBgmControl>,
) {
    let layer_count = buffers.len();
    while !control.decoder_stop.load(Ordering::Acquire) {
        let min_free = buffers
            .iter()
            .map(|rb| rb.available_write())
            .min()
            .unwrap_or(0);
        if min_free < BGM_DECODE_CHUNK_FRAMES * 3 {
            thread::sleep(Duration::from_millis(3));
            continue;
        }

        let n = source.read_synchronized(&mut raw_chunks, BGM_DECODE_CHUNK_FRAMES);
        if n == 0 {
            control.eof_reached.store(true, Ordering::Release);
            break;
        }

        let mut min_out = usize::MAX;
        for k in 0..layer_count {
            resamplers[k].process_chunk(&raw_chunks[k], &mut resampled[k]);
            min_out = min_out.min(resampled[k].len());
        }
        if min_out == 0 || min_out == usize::MAX {
            continue;
        }

        let mut offset = 0usize;
        while offset < min_out && !control.decoder_stop.load(Ordering::Acquire) {
            let free = buffers
                .iter()
                .map(|rb| rb.available_write())
                .min()
                .unwrap_or(0);
            if free == 0 {
                thread::sleep(Duration::from_millis(2));
                continue;
            }
            let take = (min_out - offset).min(free);
            for k in 0..layer_count {
                buffers[k].push_frames(&resampled[k][offset..offset + take]);
            }
            offset += take;
        }
    }
}

struct StreamingBgmSound {
    layer_buffers: Vec<Arc<AudioRingBuffer>>,
    layer_count: usize,
    sample_rate: u32,
    control: Arc<StreamingBgmControl>,
    destination: OutputDestination,
    current_layer_gains: [f32; BGM_MAX_TRACKS],
    current_master_gain: f32,
    stop_envelope: f32,
    local_batches: [[Frame; AUDIO_CALLBACK_BATCH]; BGM_MAX_TRACKS],
    batch_cursor: usize,
    batch_len: usize,
    prev_frames: [Frame; BGM_MAX_TRACKS],
    curr_frames: [Frame; BGM_MAX_TRACKS],
    has_started: bool,
    sample_phase: f64,
    is_finished: bool,
}

impl Drop for StreamingBgmSound {
    fn drop(&mut self) {
        self.control.mark_finished();
    }
}

impl StreamingBgmSound {
    /// Pops one synchronized frame across all layers (zero allocation).
    fn pop_next_synced_frame(&mut self) -> bool {
        if self.batch_cursor >= self.batch_len {
            let min_avail = self
                .layer_buffers
                .iter()
                .map(|rb| rb.available_read())
                .min()
                .unwrap_or(0);
            if min_avail == 0 {
                return false;
            }
            let take = min_avail.min(AUDIO_CALLBACK_BATCH);
            for k in 0..self.layer_count {
                self.layer_buffers[k].pop_frames(&mut self.local_batches[k][..take]);
            }
            self.batch_cursor = 0;
            self.batch_len = take;
        }

        let idx = self.batch_cursor;
        self.batch_cursor += 1;
        for k in 0..self.layer_count {
            self.prev_frames[k] = self.curr_frames[k];
            self.curr_frames[k] = self.local_batches[k][idx];
        }
        true
    }

    #[inline]
    fn finish_now(&mut self) {
        self.is_finished = true;
        self.control.mark_finished();
    }
}

impl Sound for StreamingBgmSound {
    fn output_destination(&mut self) -> OutputDestination {
        self.destination
    }

    fn process(
        &mut self,
        dt: f64,
        _clock_info_provider: &ClockInfoProvider,
        _modulator_value_provider: &ModulatorValueProvider,
    ) -> Frame {
        if self.is_finished {
            return Frame::ZERO;
        }

        let dt_f32 = if dt.is_finite() && dt > 0.0 {
            dt as f32
        } else {
            1.0 / 48_000.0
        };

        let stop_req = self.control.stop_requested.load(Ordering::Acquire);
        if self.control.paused.load(Ordering::Relaxed) && !stop_req {
            return Frame::ZERO;
        }

        if stop_req {
            let fade_secs = load_f32(&self.control.stop_fade_secs);
            if fade_secs <= 1e-4 {
                self.stop_envelope = 0.0;
            } else {
                self.stop_envelope = (self.stop_envelope - dt_f32 / fade_secs).max(0.0);
            }
            if self.stop_envelope <= 0.0 {
                self.finish_now();
                return Frame::ZERO;
            }
        }

        // Prime initial frame pair.
        if !self.has_started {
            if !self.pop_next_synced_frame() {
                if self.control.eof_reached.load(Ordering::Acquire) {
                    self.finish_now();
                }
                return Frame::ZERO;
            }
            self.prev_frames = self.curr_frames;
            self.has_started = true;
        }

        // Advance source sample cursor by `dt * sample_rate`.
        self.sample_phase += dt * self.sample_rate as f64;
        while self.sample_phase >= 1.0 {
            if self.pop_next_synced_frame() {
                self.sample_phase -= 1.0;
            } else {
                if self.control.eof_reached.load(Ordering::Acquire) {
                    self.finish_now();
                }
                self.sample_phase = 0.0;
                return Frame::ZERO;
            }
        }

        // Acquire `target_gain` before loading `fade_secs` to match the `Release` store order.
        let target_master = load_f32_acquire(&self.control.master_target_gain);
        let master_fade = load_f32(&self.control.master_fade_secs);
        self.current_master_gain =
            slew_gain(self.current_master_gain, target_master, master_fade, dt_f32);

        let frac = self.sample_phase as f32;
        let mut mix_l = 0.0f32;
        let mut mix_r = 0.0f32;

        for k in 0..self.layer_count {
            let target_g = load_f32_acquire(&self.control.layer_target_gains[k]);
            let fade_s = load_f32(&self.control.layer_fade_secs[k]);
            let g = slew_gain(self.current_layer_gains[k], target_g, fade_s, dt_f32);
            self.current_layer_gains[k] = g;

            if g > 1e-6 {
                let p = self.prev_frames[k];
                let c = self.curr_frames[k];
                let l = p.left + (c.left - p.left) * frac;
                let r = p.right + (c.right - p.right) * frac;
                mix_l += l * g;
                mix_r += r * g;
            }
        }

        let scale = self.current_master_gain * self.stop_envelope;
        Frame::new(mix_l * scale, mix_r * scale)
    }

    fn finished(&self) -> bool {
        self.is_finished
    }
}

#[inline]
fn slew_gain(current: f32, target: f32, fade_secs: f32, dt: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= 1e-6 || fade_secs <= 1e-4 {
        return target;
    }
    let step = dt / fade_secs;
    if diff > 0.0 {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}
