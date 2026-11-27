//! Real-time streaming Kira [`Sound`](kira::sound::Sound) decoders for `.adx`/`.aix` BGM
//! and `.bik`/`.mp4` FMV cutscene audio (`SH2-180`).
//!
//! Replaces full-file `StaticSoundData` materialization (which previously consumed ~322–800 MiB
//! RAM for 7-layer `.aix` tracks and ~150–250 MiB transient RAM for FMV audio) with bounded
//! lock-free [`AudioRingBuffer`](crate::ringbuffer::AudioRingBuffer)-backed background streams
//! using < 1 MiB resident memory and zero heap allocations on the audio callback thread.

mod bgm;
mod bgm_control;
mod fmv;

pub use bgm::{StreamingBgmSoundData, StreamingBgmSource, BGM_STREAM_RINGBUFFER_FRAMES};
pub use bgm_control::{StreamingBgmControl, StreamingBgmHandle};
pub use fmv::{
    StreamingFmvControl, StreamingFmvHandle, StreamingFmvSoundData, FMV_STREAM_RINGBUFFER_FRAMES,
};

use kira::Frame;
use std::sync::atomic::{AtomicU32, Ordering};

#[inline]
pub(super) fn store_f32(slot: &AtomicU32, val: f32) {
    let clean = if val.is_finite() { val } else { 0.0 };
    slot.store(clean.to_bits(), Ordering::Relaxed);
}

#[inline]
pub(super) fn store_f32_release(slot: &AtomicU32, val: f32) {
    let clean = if val.is_finite() { val } else { 0.0 };
    slot.store(clean.to_bits(), Ordering::Release);
}

#[inline]
pub(super) fn load_f32(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

#[inline]
pub(super) fn load_f32_acquire(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Acquire))
}

/// Stateful 2nd-order Butterworth stereo low-pass biquad filter for streaming PS2 SPU2 DAC
/// reconstruction (`< 44,100 Hz` original PS2/PC ADX/AIX streams).
#[derive(Debug, Clone, Copy)]
struct StreamingBiquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1_l: f32,
    x2_l: f32,
    y1_l: f32,
    y2_l: f32,
    x1_r: f32,
    x2_r: f32,
    y1_r: f32,
    y2_r: f32,
}

impl StreamingBiquad {
    fn lowpass(fs: f32, fc: f32, q: f32) -> Self {
        let omega = 2.0 * std::f32::consts::PI * (fc / fs).clamp(0.001, 0.49);
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha = sin_w / (2.0 * q);

        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos_w) * 0.5) / a0,
            b1: (1.0 - cos_w) / a0,
            b2: ((1.0 - cos_w) * 0.5) / a0,
            a1: (-2.0 * cos_w) / a0,
            a2: (1.0 - alpha) / a0,
            x1_l: 0.0,
            x2_l: 0.0,
            y1_l: 0.0,
            y2_l: 0.0,
            x1_r: 0.0,
            x2_r: 0.0,
            y1_r: 0.0,
            y2_r: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, frame: Frame) -> Frame {
        let xl = frame.left;
        let yl = self.b0 * xl + self.b1 * self.x1_l + self.b2 * self.x2_l
            - self.a1 * self.y1_l
            - self.a2 * self.y2_l;
        self.x2_l = self.x1_l;
        self.x1_l = xl;
        self.y2_l = self.y1_l;
        self.y1_l = yl;

        let xr = frame.right;
        let yr = self.b0 * xr + self.b1 * self.x1_r + self.b2 * self.x2_r
            - self.a1 * self.y1_r
            - self.a2 * self.y2_r;
        self.x2_r = self.x1_r;
        self.x1_r = xr;
        self.y2_r = self.y1_r;
        self.y1_r = yr;

        Frame::new(yl.clamp(-1.0, 1.0), yr.clamp(-1.0, 1.0))
    }
}

/// Stateful streaming 48 kHz upsampler + 2-stage Butterworth anti-imaging filter for low-rate
/// (`< 44,100 Hz`) original PS2/PC ADX/AIX streams, passing `>= 44,100 Hz` SH2:EE streams through
/// untouched (`SH2-180`).
#[derive(Debug, Clone)]
pub struct StreamingResampler48k {
    dst_rate: u32,
    passthrough: bool,
    gain_trim: f32,
    ratio: f64,
    phase: f64,
    prev: Frame,
    curr: Frame,
    initialized: bool,
    stage1: StreamingBiquad,
    stage2: StreamingBiquad,
}

impl StreamingResampler48k {
    /// Creates a streaming resampler for a source stream at `src_rate` Hz.
    #[must_use]
    pub fn new(src_rate: u32, gain_trim: f32) -> Self {
        let safe_src = src_rate.max(8_000);
        let passthrough = safe_src >= 44_100;
        let dst_rate = if passthrough { safe_src } else { 48_000 };
        let cutoff_hz = (safe_src as f32 * 0.40).clamp(3200.0, 6200.0);
        let stage = StreamingBiquad::lowpass(dst_rate as f32, cutoff_hz, 0.70710677);
        Self {
            dst_rate,
            passthrough,
            gain_trim,
            ratio: safe_src as f64 / dst_rate as f64,
            phase: 0.0,
            prev: Frame::ZERO,
            curr: Frame::ZERO,
            initialized: false,
            stage1: stage,
            stage2: stage,
        }
    }

    /// Returns the output sample rate in Hz (`48_000` for low-rate PS2 streams, `src_rate` otherwise).
    #[inline]
    #[must_use]
    pub fn output_sample_rate(&self) -> u32 {
        self.dst_rate
    }

    /// Processes a chunk of `(left, right)` source frames and appends resampled [`Frame`]s to `out`.
    pub fn process_chunk(&mut self, input: &[(f32, f32)], out: &mut Vec<Frame>) {
        out.clear();
        if input.is_empty() {
            return;
        }
        if self.passthrough {
            out.reserve(input.len());
            for &(l, r) in input {
                out.push(Frame::new(l, r));
            }
            return;
        }

        let est = ((input.len() as f64) / self.ratio).ceil() as usize + 2;
        out.reserve(est);

        for &(l, r) in input {
            let next = Frame::new(l * self.gain_trim, r * self.gain_trim);
            if !self.initialized {
                self.prev = next;
                self.curr = next;
                self.initialized = true;
            } else {
                self.prev = self.curr;
                self.curr = next;
            }

            while self.phase < 1.0 {
                let t = self.phase as f32;
                let interp = Frame::new(
                    self.prev.left + (self.curr.left - self.prev.left) * t,
                    self.prev.right + (self.curr.right - self.prev.right) * t,
                );
                let filtered = self.stage2.process(self.stage1.process(interp));
                out.push(filtered);
                self.phase += self.ratio;
            }
            self.phase -= 1.0;
        }
    }
}
