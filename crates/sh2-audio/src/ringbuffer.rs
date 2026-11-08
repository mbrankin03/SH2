//! Thread-safe, zero-allocation SPSC stereo audio ringbuffer (`SH2-55`).
//!
//! Feeds platform real-time audio backends (ALSA, PulseAudio/PipeWire, WASAPI, AAudio)
//! with zero heap allocations on the audio callback thread and 100% safe Rust (`#[forbid(unsafe_code)]`).
//!
//! # Design
//! - Pre-allocates a power-of-two array of `AtomicU64` slots at initialization.
//! - Each `AtomicU64` packs a stereo [`Frame`] (`left: f32`, `right: f32`) bit-identically via
//!   `f32::to_bits()` / `f32::from_bits()`.
//! - Producer (`write_pos`) and consumer (`read_pos`) monotonically advance `AtomicUsize` cursors
//!   using `Acquire`/`Release` memory ordering, eliminating mutex contention and priority inversion.

use kira::Frame;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Default ringbuffer capacity in stereo frames (`8192` frames = `~170.6 ms` at `48,000 Hz`).
pub const DEFAULT_RINGBUFFER_FRAMES: usize = 8192;

/// Packs a stereo [`Frame`] (`left: f32`, `right: f32`) into a single `u64` word.
#[inline]
pub fn pack_stereo_frame(frame: Frame) -> u64 {
    let l = frame.left.to_bits() as u64;
    let r = frame.right.to_bits() as u64;
    l | (r << 32)
}

/// Unpacks a `u64` word back into a stereo [`Frame`].
#[inline]
pub fn unpack_stereo_frame(bits: u64) -> Frame {
    let l = f32::from_bits(bits as u32);
    let r = f32::from_bits((bits >> 32) as u32);
    Frame::new(l, r)
}

/// Lock-free, zero-allocation Single-Producer Single-Consumer (SPSC) stereo audio ringbuffer (`SH2-55`).
#[derive(Debug)]
pub struct AudioRingBuffer {
    /// Pre-allocated power-of-two circular slot storage (packed stereo `f32` frames).
    slots: Box<[AtomicU64]>,
    /// Bitmask `capacity - 1` for fast modulo indexing.
    mask: usize,
    /// Monotonically increasing write cursor (frames pushed by producer).
    write_pos: AtomicUsize,
    /// Monotonically increasing read cursor (frames popped by platform consumer).
    read_pos: AtomicUsize,
    /// Low-watermark cue threshold in frames (triggers streaming refill when `available_read() <= low_watermark`).
    low_watermark: usize,
    /// Cumulative number of consumer underrun events (when `pop_frames` requested more frames than available).
    underrun_count: AtomicUsize,
    /// Cumulative number of producer overrun events (when `push_frames` could not fit all input frames).
    overrun_count: AtomicUsize,
}

impl Default for AudioRingBuffer {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_RINGBUFFER_FRAMES)
    }
}

impl AudioRingBuffer {
    /// Creates a new lock-free stereo ringbuffer rounding `min_capacity_frames` up to the next power of two.
    ///
    /// Performs a single allocation up front; all subsequent `push_frames` and `pop_frames` calls are zero-allocation.
    pub fn with_capacity(min_capacity_frames: usize) -> Self {
        let cap = min_capacity_frames.max(64).next_power_of_two();
        let mut vec = Vec::with_capacity(cap);
        for _ in 0..cap {
            vec.push(AtomicU64::new(0));
        }
        Self {
            slots: vec.into_boxed_slice(),
            mask: cap - 1,
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            low_watermark: cap / 4,
            underrun_count: AtomicUsize::new(0),
            overrun_count: AtomicUsize::new(0),
        }
    }

    /// Returns the total capacity in stereo frames (always a power of two).
    #[inline]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Returns the low-watermark threshold in frames below which a streaming cue refill should be scheduled.
    #[inline]
    pub fn low_watermark(&self) -> usize {
        self.low_watermark
    }

    /// Configures a custom streaming refill low-watermark (clamped to `0..=capacity()`).
    #[inline]
    pub fn set_low_watermark(&mut self, frames: usize) {
        self.low_watermark = frames.min(self.capacity());
    }

    /// Returns the number of stereo frames currently queued and ready to be read by the consumer.
    #[inline]
    pub fn available_read(&self) -> usize {
        let w = self.write_pos.load(Ordering::Acquire);
        let r = self.read_pos.load(Ordering::Acquire);
        w.wrapping_sub(r).min(self.capacity())
    }

    /// Returns the number of free frame slots available for the producer to write without overwriting unread data.
    #[inline]
    pub fn available_write(&self) -> usize {
        self.capacity().saturating_sub(self.available_read())
    }

    /// Returns `true` when the queued frame count has dropped to or below `low_watermark()`,
    /// signaling the streaming cue manager to decode the next chunk.
    #[inline]
    pub fn needs_refill(&self) -> bool {
        self.available_read() <= self.low_watermark
    }

    /// Pushes up to `src.len()` stereo frames into the ringbuffer without blocking or allocating.
    ///
    /// Returns the exact number of frames written. If fewer than `src.len()` frames fit, increments `overrun_count`.
    pub fn push_frames(&self, src: &[Frame]) -> usize {
        let cap = self.capacity();
        let w = self.write_pos.load(Ordering::Relaxed);
        let r = self.read_pos.load(Ordering::Acquire);
        let used = w.wrapping_sub(r);
        let free = cap.saturating_sub(used);

        let to_write = src.len().min(free);
        if to_write < src.len() {
            self.overrun_count.fetch_add(1, Ordering::Relaxed);
        }

        for (i, &frame) in src.iter().take(to_write).enumerate() {
            let slot_idx = w.wrapping_add(i) & self.mask;
            self.slots[slot_idx].store(pack_stereo_frame(frame), Ordering::Relaxed);
        }

        self.write_pos
            .store(w.wrapping_add(to_write), Ordering::Release);
        to_write
    }

    /// Pops up to `dst.len()` stereo frames into `dst` without blocking or allocating.
    ///
    /// If fewer than `dst.len()` frames are available, zero-fills the remainder of `dst` (silence)
    /// to prevent stale buffer buzz on ALSA/PulseAudio/WASAPI/AAudio callbacks and increments `underrun_count`.
    ///
    /// Returns the number of valid audio frames read from the ringbuffer.
    pub fn pop_frames(&self, dst: &mut [Frame]) -> usize {
        let w = self.write_pos.load(Ordering::Acquire);
        let r = self.read_pos.load(Ordering::Relaxed);
        let avail = w.wrapping_sub(r).min(self.capacity());

        let to_read = dst.len().min(avail);
        for (i, out_slot) in dst.iter_mut().take(to_read).enumerate() {
            let slot_idx = r.wrapping_add(i) & self.mask;
            let bits = self.slots[slot_idx].load(Ordering::Relaxed);
            *out_slot = unpack_stereo_frame(bits);
        }

        if to_read < dst.len() {
            for out_slot in dst.iter_mut().skip(to_read) {
                *out_slot = Frame::new(0.0, 0.0);
            }
            self.underrun_count.fetch_add(1, Ordering::Relaxed);
        }

        self.read_pos
            .store(r.wrapping_add(to_read), Ordering::Release);
        to_read
    }

    /// Clears all queued frames and resets read/write cursors to zero.
    pub fn clear(&self) {
        let w = self.write_pos.load(Ordering::Acquire);
        self.read_pos.store(w, Ordering::Release);
    }

    /// Returns the total number of consumer underruns recorded since creation.
    #[inline]
    pub fn underrun_count(&self) -> usize {
        self.underrun_count.load(Ordering::Relaxed)
    }

    /// Returns the total number of producer overruns recorded since creation.
    #[inline]
    pub fn overrun_count(&self) -> usize {
        self.overrun_count.load(Ordering::Relaxed)
    }
}
