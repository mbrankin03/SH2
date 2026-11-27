//! Lock-free atomic control block and handle for streaming ADX/AIX BGM playback (`SH2-180`).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::bgm_manager::BGM_MAX_TRACKS;

use super::{store_f32, store_f32_release};

/// Lock-free atomic control block shared between [`StreamingBgmHandle`], the background decoder
/// thread, and the real-time Kira [`Sound`](kira::sound::Sound) callback (`SH2-180`).
#[derive(Debug)]
pub struct StreamingBgmControl {
    pub(super) layer_count: usize,
    pub(super) layer_target_gains: [AtomicU32; BGM_MAX_TRACKS],
    pub(super) layer_fade_secs: [AtomicU32; BGM_MAX_TRACKS],
    pub(super) master_target_gain: AtomicU32,
    pub(super) master_fade_secs: AtomicU32,
    pub(super) paused: AtomicBool,
    pub(super) stop_requested: AtomicBool,
    pub(super) stop_fade_secs: AtomicU32,
    pub(super) decoder_stop: AtomicBool,
    pub(super) eof_reached: AtomicBool,
    pub(super) finished: AtomicBool,
}

impl StreamingBgmControl {
    pub(super) fn new(layer_count: usize, initial_gains: &[f32], initial_master: f32) -> Arc<Self> {
        let layer_target_gains = std::array::from_fn(|i| {
            let g = initial_gains.get(i).copied().unwrap_or(0.0).max(0.0);
            AtomicU32::new(g.to_bits())
        });
        let layer_fade_secs = std::array::from_fn(|_| AtomicU32::new(0.0f32.to_bits()));
        Arc::new(Self {
            layer_count: layer_count.clamp(1, BGM_MAX_TRACKS),
            layer_target_gains,
            layer_fade_secs,
            master_target_gain: AtomicU32::new(initial_master.max(0.0).to_bits()),
            master_fade_secs: AtomicU32::new(0.0f32.to_bits()),
            paused: AtomicBool::new(false),
            stop_requested: AtomicBool::new(false),
            stop_fade_secs: AtomicU32::new(0.05f32.to_bits()),
            decoder_stop: AtomicBool::new(false),
            eof_reached: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        })
    }

    /// Updates target linear amplitude gain for `layer_idx` with linear fade duration.
    ///
    /// Stores `fade_secs` before `target_gain` (`Release`) so the audio callback acquires a
    /// consistent `(target_gain, fade_secs)` pair without tearing.
    #[inline]
    pub fn set_layer_gain(&self, layer_idx: usize, gain: f32, fade_duration: Duration) {
        if layer_idx < BGM_MAX_TRACKS {
            store_f32(
                &self.layer_fade_secs[layer_idx],
                fade_duration.as_secs_f32().max(0.0),
            );
            store_f32_release(&self.layer_target_gains[layer_idx], gain.max(0.0));
        }
    }

    /// Updates master gain multiplier for the stream with linear fade duration.
    ///
    /// Stores `master_fade_secs` before `master_target_gain` (`Release`) to prevent tearing.
    #[inline]
    pub fn set_master_gain(&self, gain: f32, fade_duration: Duration) {
        store_f32(&self.master_fade_secs, fade_duration.as_secs_f32().max(0.0));
        store_f32_release(&self.master_target_gain, gain.max(0.0));
    }

    /// Pauses or resumes playback atomically.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Requests a smooth linear fade-out; the background decoder remains alive until the audio
    /// callback completes the fade-out and marks `finished` / `decoder_stop`.
    #[inline]
    pub fn request_stop(&self, fade_out: Duration) {
        store_f32(&self.stop_fade_secs, fade_out.as_secs_f32().max(0.0));
        self.stop_requested.store(true, Ordering::Release);
    }

    /// Returns the number of synchronized layers in this stream.
    #[inline]
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layer_count
    }

    /// Returns `true` once the audio callback has finished playback or completed fade-out.
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    #[inline]
    pub(super) fn mark_finished(&self) {
        self.finished.store(true, Ordering::Release);
        self.decoder_stop.store(true, Ordering::Release);
    }
}

/// Handle for controlling an active streaming ADX/AIX BGM playback session (`SH2-180`).
///
/// Dropping the handle without calling [`stop`](Self::stop) requests an immediate stop so the
/// Kira `Sound` slot finishes cleanly and the background decoder thread exits. If [`stop`](Self::stop)
/// was already called, dropping the handle allows any in-progress fade-out to complete smoothly.
pub struct StreamingBgmHandle {
    pub(super) control: Arc<StreamingBgmControl>,
    pub(super) _worker: Option<JoinHandle<()>>,
}

impl StreamingBgmHandle {
    /// Updates the linear gain of a single BGM layer with smooth linear ramping.
    #[inline]
    pub fn set_layer_gain(&self, layer_idx: usize, gain: f32, fade_duration: Duration) {
        self.control.set_layer_gain(layer_idx, gain, fade_duration);
    }

    /// Updates the overall stream master gain with smooth linear ramping.
    #[inline]
    pub fn set_master_gain(&self, gain: f32, fade_duration: Duration) {
        self.control.set_master_gain(gain, fade_duration);
    }

    /// Pauses or resumes playback.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.control.set_paused(paused);
    }

    /// Fades out and stops playback; the background decoder exits automatically once the fade completes.
    #[inline]
    pub fn stop(&self, fade_out: Duration) {
        self.control.request_stop(fade_out);
    }

    /// Returns the number of active synchronized layers.
    #[inline]
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.control.layer_count()
    }

    /// Returns `true` if the stream has finished playing.
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.control.is_finished()
    }
}

impl Drop for StreamingBgmHandle {
    fn drop(&mut self) {
        if !self.control.stop_requested.load(Ordering::Acquire) {
            self.control.request_stop(Duration::ZERO);
        }
    }
}
