//! Ringbuffer-backed streaming FMV audio Kira [`Sound`] & [`SoundData`] (`SH2-180`).
//!
//! Streams 48 kHz stereo PCM chunks directly from [`BinkAudioStream`] (`ffmpeg` stdout) through a
//! bounded lock-free [`AudioRingBuffer`] (~256 KiB RAM), eliminating the ~150–250 MiB transient
//! 3-copy PCM spike and startup hitch while guaranteeing `ffmpeg` child termination on stop/drop.

use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use kira::{
    clock::clock_info::ClockInfoProvider,
    modulator::value_provider::ModulatorValueProvider,
    sound::{Sound, SoundData},
    track::TrackId,
    Frame, OutputDestination,
};
use sh2_assets::BinkAudioStream;

use crate::ringbuffer::AudioRingBuffer;

use super::{load_f32, store_f32};

/// Ringbuffer capacity for streaming FMV audio (`32,768` stereo frames ≈ `682 ms` at `48 kHz`, `256 KiB`).
pub const FMV_STREAM_RINGBUFFER_FRAMES: usize = 32_768;

/// Chunk size read per background reader iteration (`1024` stereo frames = `4 KiB` of s16le PCM).
const FMV_READ_CHUNK_FRAMES: usize = 1024;

/// Local zero-allocation callback pop batch size.
const FMV_CALLBACK_BATCH: usize = 64;

/// Lock-free atomic control block shared between [`StreamingFmvHandle`], the background `ffmpeg`
/// reader thread, and the real-time Kira [`Sound`] callback (`SH2-180`).
#[derive(Debug)]
pub struct StreamingFmvControl {
    volume: AtomicU32,
    paused: AtomicBool,
    stop_requested: AtomicBool,
    stop_fade_secs: AtomicU32,
    reader_stop: AtomicBool,
    eof_reached: AtomicBool,
    finished: AtomicBool,
}

impl StreamingFmvControl {
    fn new(volume: f32) -> Arc<Self> {
        Arc::new(Self {
            volume: AtomicU32::new(volume.max(0.0).to_bits()),
            paused: AtomicBool::new(false),
            stop_requested: AtomicBool::new(false),
            stop_fade_secs: AtomicU32::new(0.05f32.to_bits()),
            reader_stop: AtomicBool::new(false),
            eof_reached: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        })
    }

    /// Updates linear volume multiplier atomically.
    #[inline]
    pub fn set_volume(&self, volume: f32) {
        store_f32(&self.volume, volume.max(0.0));
    }

    /// Pauses or resumes FMV audio playback atomically.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Requests a smooth fade-out and immediately signals the background `ffmpeg` reader to exit.
    #[inline]
    pub fn request_stop(&self, fade_out: Duration) {
        store_f32(&self.stop_fade_secs, fade_out.as_secs_f32().max(0.0));
        self.stop_requested.store(true, Ordering::Release);
        self.reader_stop.store(true, Ordering::Release);
    }

    /// Returns `true` once FMV audio playback has finished or faded out.
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

/// Handle controlling an active streaming FMV audio session (`SH2-180`).
///
/// Dropping the handle signals the background reader thread to exit immediately, which drops the
/// owned [`BinkAudioStream`] and kills/reaps `ffmpeg` so no zombie processes ever leak.
pub struct StreamingFmvHandle {
    control: Arc<StreamingFmvControl>,
    _worker: Option<JoinHandle<()>>,
}

impl StreamingFmvHandle {
    /// Updates linear amplitude volume.
    #[inline]
    pub fn set_volume(&self, volume: f32) {
        self.control.set_volume(volume);
    }

    /// Pauses or resumes playback.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.control.set_paused(paused);
    }

    /// Stops playback with a linear fade-out and shuts down the background reader / `ffmpeg` process.
    #[inline]
    pub fn stop(&self, fade_out: Duration) {
        self.control.request_stop(fade_out);
    }

    /// Returns `true` if the FMV audio stream has finished playing.
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.control.is_finished()
    }
}

impl Drop for StreamingFmvHandle {
    fn drop(&mut self) {
        self.control.reader_stop.store(true, Ordering::Release);
    }
}

type CustomFmvReadFn = Box<dyn FnMut(&mut [(f32, f32)]) -> usize + Send>;

/// Backing producer for [`StreamingFmvSoundData`].
enum FmvProducer {
    Bink(BinkAudioStream),
    Custom(CustomFmvReadFn),
}

impl FmvProducer {
    fn read_frames(&mut self, out: &mut [(f32, f32)]) -> usize {
        match self {
            Self::Bink(stream) => stream.read_frames(out),
            Self::Custom(f) => f(out),
        }
    }
}

/// Kira [`SoundData`] descriptor for ringbuffer-backed streaming FMV cutscene audio (`SH2-180`).
pub struct StreamingFmvSoundData {
    sample_rate: u32,
    volume: f32,
    destination: TrackId,
    producer: FmvProducer,
}

impl StreamingFmvSoundData {
    /// Creates a streaming FMV sound backed by a live [`BinkAudioStream`] (`ffmpeg` stdout).
    #[must_use]
    pub fn from_bink_stream(stream: BinkAudioStream, volume: f32, destination: TrackId) -> Self {
        let sample_rate = stream.sample_rate().max(8_000);
        Self {
            sample_rate,
            volume,
            destination,
            producer: FmvProducer::Bink(stream),
        }
    }

    /// Creates a streaming FMV sound backed by a custom frame producer closure (for unit tests).
    #[must_use]
    pub fn from_custom_producer<F>(
        sample_rate: u32,
        volume: f32,
        destination: TrackId,
        producer: F,
    ) -> Self
    where
        F: FnMut(&mut [(f32, f32)]) -> usize + Send + 'static,
    {
        Self {
            sample_rate: sample_rate.max(8_000),
            volume,
            destination,
            producer: FmvProducer::Custom(Box::new(producer)),
        }
    }
}

impl SoundData for StreamingFmvSoundData {
    type Error = Infallible;
    type Handle = StreamingFmvHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let control = StreamingFmvControl::new(self.volume);
        let ringbuffer = Arc::new(AudioRingBuffer::with_capacity(FMV_STREAM_RINGBUFFER_FRAMES));

        let worker_rb = Arc::clone(&ringbuffer);
        let worker_control = Arc::clone(&control);
        let mut producer = self.producer;
        let worker = thread::Builder::new()
            .name("sh2-fmv-audio-reader".to_string())
            .spawn(move || {
                let mut raw = vec![(0.0f32, 0.0f32); FMV_READ_CHUNK_FRAMES];
                let mut frames = Vec::with_capacity(FMV_READ_CHUNK_FRAMES);

                while !worker_control.reader_stop.load(Ordering::Acquire) {
                    if worker_rb.available_write() < FMV_READ_CHUNK_FRAMES {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    let n = producer.read_frames(&mut raw);
                    if n == 0 {
                        worker_control.eof_reached.store(true, Ordering::Release);
                        break;
                    }

                    frames.clear();
                    for &(l, r) in &raw[..n] {
                        frames.push(Frame::new(l, r));
                    }

                    let mut offset = 0usize;
                    while offset < frames.len()
                        && !worker_control.reader_stop.load(Ordering::Acquire)
                    {
                        let pushed = worker_rb.push_frames(&frames[offset..]);
                        if pushed == 0 {
                            thread::sleep(Duration::from_millis(2));
                        } else {
                            offset += pushed;
                        }
                    }
                }
                // `producer` (and its `BinkAudioStream` / `ffmpeg` child) drops cleanly here.
            })
            .ok();

        let sound = StreamingFmvSound {
            ringbuffer,
            sample_rate: self.sample_rate,
            control: Arc::clone(&control),
            destination: OutputDestination::Track(self.destination),
            stop_envelope: 1.0,
            local_batch: [Frame::ZERO; FMV_CALLBACK_BATCH],
            batch_cursor: 0,
            batch_len: 0,
            prev_frame: Frame::ZERO,
            curr_frame: Frame::ZERO,
            has_started: false,
            sample_phase: 0.0,
            is_finished: false,
        };

        let handle = StreamingFmvHandle {
            control,
            _worker: worker,
        };
        Ok((Box::new(sound), handle))
    }
}

struct StreamingFmvSound {
    ringbuffer: Arc<AudioRingBuffer>,
    sample_rate: u32,
    control: Arc<StreamingFmvControl>,
    destination: OutputDestination,
    stop_envelope: f32,
    local_batch: [Frame; FMV_CALLBACK_BATCH],
    batch_cursor: usize,
    batch_len: usize,
    prev_frame: Frame,
    curr_frame: Frame,
    has_started: bool,
    sample_phase: f64,
    is_finished: bool,
}

impl StreamingFmvSound {
    fn pop_next_frame(&mut self) -> bool {
        if self.batch_cursor >= self.batch_len {
            let avail = self.ringbuffer.available_read();
            if avail == 0 {
                return false;
            }
            let take = avail.min(FMV_CALLBACK_BATCH);
            self.ringbuffer.pop_frames(&mut self.local_batch[..take]);
            self.batch_cursor = 0;
            self.batch_len = take;
        }
        self.prev_frame = self.curr_frame;
        self.curr_frame = self.local_batch[self.batch_cursor];
        self.batch_cursor += 1;
        true
    }
}

impl Sound for StreamingFmvSound {
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

        if self.control.paused.load(Ordering::Relaxed)
            && !self.control.stop_requested.load(Ordering::Acquire)
        {
            return Frame::ZERO;
        }

        if self.control.stop_requested.load(Ordering::Acquire) {
            let fade_secs = load_f32(&self.control.stop_fade_secs);
            if fade_secs <= 1e-4 {
                self.stop_envelope = 0.0;
            } else {
                self.stop_envelope = (self.stop_envelope - dt_f32 / fade_secs).max(0.0);
            }
            if self.stop_envelope <= 0.0 {
                self.is_finished = true;
                self.control.finished.store(true, Ordering::Release);
                return Frame::ZERO;
            }
        }

        if !self.has_started {
            if !self.pop_next_frame() {
                if self.control.eof_reached.load(Ordering::Acquire) {
                    self.is_finished = true;
                    self.control.finished.store(true, Ordering::Release);
                }
                return Frame::ZERO;
            }
            self.prev_frame = self.curr_frame;
            self.has_started = true;
        }

        self.sample_phase += dt * self.sample_rate as f64;
        while self.sample_phase >= 1.0 {
            if self.pop_next_frame() {
                self.sample_phase -= 1.0;
            } else {
                if self.control.eof_reached.load(Ordering::Acquire) {
                    self.is_finished = true;
                    self.control.finished.store(true, Ordering::Release);
                }
                self.sample_phase = 0.0;
                return Frame::ZERO;
            }
        }

        let frac = self.sample_phase as f32;
        let l = self.prev_frame.left + (self.curr_frame.left - self.prev_frame.left) * frac;
        let r = self.prev_frame.right + (self.curr_frame.right - self.prev_frame.right) * frac;
        let gain = load_f32(&self.control.volume) * self.stop_envelope;
        Frame::new(l * gain, r * gain)
    }

    fn finished(&self) -> bool {
        self.is_finished
    }
}
