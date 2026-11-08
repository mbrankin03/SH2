//! Sample-accurate loop-point evaluation and loop-boundary crossfading (`SH2-55`).
//!
//! Eliminates pre-baked loop file bloat and prevents audible clicking or pop artifacts
//! when looping `.adx` and `sddata.bin` / `.wav` streams at high sample rates (`44.1 kHz` / `48 kHz`).
//!
//! # Key Capabilities
//! 1. **Sample-Accurate Loop Region Mapping (`LoopRegionSamples`)**:
//!    - Evaluates `loop_start` and `loop_end` directly in sample frames, scaling them proportionally
//!      when low-sample-rate PS2/PC audio (`16 kHz / 22.05 kHz / 24 kHz`) is upsampled to `48,000 Hz`.
//! 2. **Raised-Cosine Loop Boundary Crossfader (`apply_loop_boundary_crossfade`)**:
//!    - Blends the tail window approaching `loop_end` (`frames[loop_end - W .. loop_end]`) toward
//!      the loop entry waveform (`frames[loop_start]`) using a smooth raised-cosine (Hann) window
//!      so `frames[loop_end - 1]` converges continuously to `frames[loop_start]`, eliminating step
//!      discontinuities ($\Delta \to 0$) without altering phase inside the body of the loop.

use kira::{
    sound::{EndPosition, PlaybackPosition, Region},
    Frame,
};

/// Default crossfade window duration (`3.0 ms` = `144` samples at `48 kHz`, `132` samples at `44.1 kHz`)
/// used to eliminate step discontinuities at loop boundaries (`SH2-55`).
pub const DEFAULT_LOOP_CROSSFADE_SAMPLES_48K: usize = 144;

/// Sample-accurate loop region expressed in both exact sample frame indices and Kira [`Region`] seconds (`SH2-55`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopRegionSamples {
    /// Target playback sample rate in Hz (e.g. `44100` or `48000`).
    pub sample_rate: u32,
    /// Inclusive loop start frame index (`0..loop_end_frame`).
    pub loop_start_frame: usize,
    /// Exclusive loop end frame index (`loop_start_frame + 1 ..= total_frames`).
    pub loop_end_frame: usize,
    /// Total number of frames in the buffer.
    pub total_frames: usize,
}

impl LoopRegionSamples {
    /// Resolves and clamps raw `(loop_start_sample, loop_end_sample)` from a source header (`src_rate`)
    /// into exact target sample indices at `dst_rate` bounded by `total_dst_frames`.
    pub fn from_source_samples(
        loop_start_src: u32,
        loop_end_src: u32,
        src_rate: u32,
        dst_rate: u32,
        total_dst_frames: usize,
    ) -> Option<Self> {
        if total_dst_frames < 2 {
            return None;
        }
        let s_rate = src_rate.max(1) as f64;
        let d_rate = dst_rate.max(1) as f64;
        let scale = d_rate / s_rate;

        let start_scaled = ((loop_start_src as f64) * scale).round() as usize;
        let end_scaled = if loop_end_src > loop_start_src {
            ((loop_end_src as f64) * scale).round() as usize
        } else {
            total_dst_frames
        };

        let loop_start_frame = start_scaled.min(total_dst_frames.saturating_sub(2));
        let loop_end_frame = end_scaled.clamp(loop_start_frame + 1, total_dst_frames);

        Some(Self {
            sample_rate: dst_rate.max(1),
            loop_start_frame,
            loop_end_frame,
            total_frames: total_dst_frames,
        })
    }

    /// Converts this sample-exact loop region into a Kira [`Region`] for [`StaticSoundSettings::loop_region`].
    #[inline]
    pub fn to_kira_region(self) -> Region {
        let rate = self.sample_rate.max(1) as f64;
        let start_sec = self.loop_start_frame as f64 / rate;
        let end_pos = if self.loop_end_frame > 0 && self.loop_end_frame < self.total_frames {
            EndPosition::Custom(PlaybackPosition::Seconds(self.loop_end_frame as f64 / rate))
        } else {
            EndPosition::EndOfAudio
        };
        Region {
            start: PlaybackPosition::Seconds(start_sec),
            end: end_pos,
        }
    }
}

/// Applies a sample-accurate raised-cosine crossfade across the `[loop_end - window .. loop_end]`
/// boundary of `frames` so that wrapping from `frames[loop_end - 1]` back to `frames[loop_start]`
/// has zero step or slope discontinuity (`SH2-55`).
///
/// - When `loop_start > 0` and pre-loop lead-in samples `frames[loop_start - W .. loop_start]` exist,
///   crossfades the tail `frames[loop_end - W .. loop_end]` with the authentic pre-loop lead-in
///   `frames[loop_start - W .. loop_start]`.
/// - When `loop_start == 0` (whole-file loop), smoothly tapers the DC/amplitude offset over the final
///   `W` samples toward `frames[loop_start]` using a raised-cosine curve $w(t) = \frac{1}{2}(1 - \cos(\pi t))$.
/// - Also truncates any trailing dead samples past `loop_end` when `loop_end < frames.len()` so
///   backends using `EndOfAudio` wrap on the exact `loop_end` sample.
pub fn apply_loop_boundary_crossfade(
    frames: &mut Vec<Frame>,
    loop_region: LoopRegionSamples,
    max_crossfade_samples: usize,
) {
    let start = loop_region.loop_start_frame;
    let end = loop_region.loop_end_frame.min(frames.len());
    if end <= start + 4 || frames.is_empty() {
        return;
    }

    // Truncate dead trailing samples beyond `loop_end` so the buffer ends cleanly on `loop_end`
    if end < frames.len() {
        frames.truncate(end);
    }

    let loop_len = end - start;
    let window = max_crossfade_samples.min(loop_len / 4).max(1);

    let target_start_frame = frames[start];

    if start >= window {
        // Case A: We have `window` pre-loop lead-in samples `[start - window .. start]` that naturally
        // lead into `frames[start]`. Blend `[end - window .. end]` into `[start - window .. start]`
        // using a raised-cosine equal-amplitude crossfade so `frames[end - 1]` seamlessly matches `frames[start - 1]`.
        for i in 0..window {
            // t goes from 0.0 at the start of the crossfade window to 1.0 at `end - 1`
            let t = (i + 1) as f32 / window as f32;
            let alpha = 0.5 * (1.0 - (std::f32::consts::PI * t).cos());
            let tail_idx = end - window + i;
            let lead_in = frames[start - window + i];
            let tail = frames[tail_idx];
            frames[tail_idx] = Frame::new(
                tail.left * (1.0 - alpha) + lead_in.left * alpha,
                tail.right * (1.0 - alpha) + lead_in.right * alpha,
            );
        }
    } else {
        // Case B: `loop_start < window` (e.g. `loop_start == 0`).
        // Smoothly C1-stitch the tail `[end - window .. end]` into `frames[start]` using raised-cosine interpolation
        // so `frames[end - 1]` equals `frames[start]` with zero step click.
        for i in 0..window {
            let t = (i + 1) as f32 / window as f32;
            let alpha = 0.5 * (1.0 - (std::f32::consts::PI * t).cos());
            let tail_idx = end - window + i;
            let tail = frames[tail_idx];
            frames[tail_idx] = Frame::new(
                tail.left * (1.0 - alpha) + target_start_frame.left * alpha,
                tail.right * (1.0 - alpha) + target_start_frame.right * alpha,
            );
        }
    }
}

/// Measures the Euclidean stereo step discontinuity $\|\text{frames}[\text{end}-1] - \text{frames}[\text{start}]\|$
/// across a loop boundary (`0.0` = perfectly seamless wrap).
#[inline]
pub fn measure_loop_boundary_step_delta(
    frames: &[Frame],
    loop_start: usize,
    loop_end: usize,
) -> f32 {
    if frames.is_empty() || loop_end == 0 || loop_end > frames.len() || loop_start >= loop_end {
        return 0.0;
    }
    let a = frames[loop_end - 1];
    let b = frames[loop_start];
    let dl = a.left - b.left;
    let dr = a.right - b.right;
    (dl * dl + dr * dr).sqrt()
}
