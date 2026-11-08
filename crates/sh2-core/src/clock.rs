//! Simulation tick clock supporting decoupled 60+ FPS ticks and zero-jitter frame pacing.
//!
//! Provides fixed-timestep simulation ticking with smooth subframe interpolation (`alpha`)
//! and deadline-scheduled frame rate limiting for 30, 60, 120, 144, 240 Hz, and Uncapped VRR.

use crate::config::FrameRateCap;
use std::time::{Duration, Instant};

/// Default maximum accumulated time (`~250 ms` = 15 ticks at 60 Hz) to prevent spiral-of-death
/// stalls when a window is dragged, minimized, or breakpoints are hit.
pub const DEFAULT_MAX_ACCUMULATED: Duration = Duration::from_nanos(250_000_005);

#[derive(Debug, Clone)]
pub struct EngineClock {
    /// Target simulation ticks per second (clamped to at least 1 TPS).
    pub target_tps: u32,
    /// Duration of each discrete simulation tick.
    pub tick_duration: Duration,
    /// Accumulated real time waiting to be simulated.
    pub accumulated: Duration,
    /// Upper bound on accumulated time to prevent spiral-of-death stalls.
    pub max_accumulated: Duration,
}

impl Default for EngineClock {
    fn default() -> Self {
        Self::new(60)
    }
}

impl EngineClock {
    /// Create a new simulation clock for the given target ticks per second.
    ///
    /// `target_tps` is clamped to a minimum of 1 to prevent division-by-zero panics.
    pub fn new(target_tps: u32) -> Self {
        let safe_tps = target_tps.max(1);
        Self {
            target_tps: safe_tps,
            tick_duration: Duration::from_secs_f64(1.0 / safe_tps as f64),
            accumulated: Duration::ZERO,
            max_accumulated: DEFAULT_MAX_ACCUMULATED,
        }
    }

    /// Advance the accumulator with real elapsed delta time, clamped to `max_accumulated`.
    #[inline]
    pub fn advance(&mut self, delta: Duration) {
        self.accumulated = (self.accumulated + delta).min(self.max_accumulated);
    }

    /// Check if enough time has elapsed to consume a fixed simulation tick.
    #[inline]
    pub fn step(&mut self) -> bool {
        if self.accumulated >= self.tick_duration {
            self.accumulated -= self.tick_duration;
            true
        } else {
            false
        }
    }

    /// Compute the subframe interpolation factor (0.0 .. 1.0) for smooth rendering between physics ticks.
    #[inline]
    pub fn alpha(&self) -> f32 {
        if self.tick_duration.is_zero() {
            return 1.0;
        }
        (self.accumulated.as_secs_f64() / self.tick_duration.as_secs_f64()).clamp(0.0, 1.0) as f32
    }

    /// Computes continuous deterministic simulation time in seconds from elapsed discrete `tick_counter`
    /// plus sub-frame `accumulated` remainder.
    #[inline]
    pub fn deterministic_time_seconds(&self, tick_counter: u64) -> f64 {
        tick_counter as f64 * self.tick_duration.as_secs_f64() + self.accumulated.as_secs_f64()
    }
}

/// Zero-jitter deadline-based frame pacer supporting `30`, `60`, `120`, `144`, `240` FPS and `Uncapped` VRR.
///
/// Tracks exact monotonic target deadlines (`next_deadline += frame_interval`) rather than relative
/// per-frame sleeps so OS scheduler wake-up variance never accumulates drift.
#[derive(Debug, Clone)]
pub struct FramePacer {
    /// Active frame rate cap mode.
    pub cap: FrameRateCap,
    /// Monotonic deadline for the next frame presentation.
    next_deadline: Option<Instant>,
    /// Timestamp of the most recent frame start.
    last_frame_instant: Option<Instant>,
    /// Exponentially smoothed frame time in milliseconds.
    pub ema_frame_time_ms: f32,
    /// Exponentially smoothed frame-to-frame jitter in milliseconds.
    pub ema_jitter_ms: f32,
}

impl Default for FramePacer {
    fn default() -> Self {
        Self::new(FrameRateCap::default())
    }
}

impl FramePacer {
    /// Creates a new [`FramePacer`] configured with the specified [`FrameRateCap`].
    pub fn new(cap: FrameRateCap) -> Self {
        let default_ms = cap
            .target_frame_duration()
            .map(|d| d.as_secs_f32() * 1000.0)
            .unwrap_or(16.666_668);
        Self {
            cap,
            next_deadline: None,
            last_frame_instant: None,
            ema_frame_time_ms: default_ms,
            ema_jitter_ms: 0.0,
        }
    }

    /// Updates the active [`FrameRateCap`] at runtime without allocation.
    pub fn set_cap(&mut self, cap: FrameRateCap) {
        if self.cap != cap {
            self.cap = cap;
            self.next_deadline = None;
        }
    }

    /// Records the start of a rendered frame at `now`, updating frame-time and jitter telemetry
    /// and advancing the monotonic deadline for the next frame.
    pub fn begin_frame(&mut self, now: Instant) {
        if let Some(prev) = self.last_frame_instant {
            let dt_ms = now.saturating_duration_since(prev).as_secs_f32() * 1000.0;
            let jitter = (dt_ms - self.ema_frame_time_ms).abs();
            self.ema_frame_time_ms = self.ema_frame_time_ms * 0.9 + dt_ms * 0.1;
            self.ema_jitter_ms = self.ema_jitter_ms * 0.9 + jitter * 0.1;
        }
        self.last_frame_instant = Some(now);

        if let Some(interval) = self.cap.target_frame_duration() {
            let next = match self.next_deadline {
                Some(prev_deadline) => {
                    let candidate = prev_deadline + interval;
                    // If we fell behind by more than 1 full frame interval, snap forward cleanly
                    if candidate <= now {
                        now + interval
                    } else {
                        candidate
                    }
                }
                None => now + interval,
            };
            self.next_deadline = Some(next);
        } else {
            self.next_deadline = None;
        }
    }

    /// Returns the monotonic [`Instant`] at which the next frame should begin, or `None` if `Uncapped`.
    #[inline]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.next_deadline
    }

    /// Returns true if a new frame is ready to be rendered at `now`.
    #[inline]
    pub fn is_ready_for_frame(&self, now: Instant) -> bool {
        match self.next_deadline {
            Some(deadline) => now >= deadline,
            None => true,
        }
    }

    /// Hybrid sleep + sub-millisecond spin wait until the current frame's target deadline.
    /// No-op when `FrameRateCap::Uncapped`.
    pub fn wait_for_deadline(&self) {
        let Some(deadline) = self.next_deadline else {
            return;
        };
        let spin_margin = Duration::from_micros(1200);
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline - now;
            if remaining > spin_margin {
                std::thread::sleep(remaining - spin_margin);
            } else {
                std::hint::spin_loop();
            }
        }
    }

    /// Exponentially smoothed frames per second.
    #[inline]
    pub fn smoothed_fps(&self) -> f32 {
        if self.ema_frame_time_ms > 0.001 {
            1000.0 / self.ema_frame_time_ms
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_default_and_zero_protection() {
        let default_clock = EngineClock::default();
        assert_eq!(default_clock.target_tps, 60);

        // Zero target_tps must not panic and clamp safely to 1
        let zero_clock = EngineClock::new(0);
        assert_eq!(zero_clock.target_tps, 1);
        assert_eq!(zero_clock.tick_duration, Duration::from_secs(1));
    }

    #[test]
    fn test_clock_ticks_and_alpha() {
        let mut clock = EngineClock::new(60);
        let tick = clock.tick_duration;
        let half_tick = tick / 2;

        clock.advance(half_tick);
        assert!(!clock.step(), "Should not step on half tick");
        let alpha = clock.alpha();
        assert!((alpha - 0.5).abs() < 0.05);

        // Advance remainder so total accumulated >= tick
        clock.advance(tick - half_tick);
        assert!(clock.step(), "Should step when full tick duration is met");
        assert!(
            !clock.step(),
            "Should not step again without more accumulated time"
        );
    }

    #[test]
    fn test_sh2_56_spiral_of_death_clamp_and_frame_pacer_deadlines() {
        let mut clock = EngineClock::new(60);
        // Simulate a 5-second window freeze
        clock.advance(Duration::from_secs(5));
        assert_eq!(clock.accumulated, DEFAULT_MAX_ACCUMULATED);

        let mut steps = 0;
        while clock.step() {
            steps += 1;
        }
        assert_eq!(
            steps, 15,
            "250ms clamp should cap catch-up to at most 15 ticks at 60 Hz"
        );

        // Test FramePacer monotonic deadlines at 240 FPS
        let mut pacer = FramePacer::new(FrameRateCap::Fps240);
        let t0 = Instant::now();
        pacer.begin_frame(t0);
        let d1 = pacer
            .next_deadline()
            .expect("240 FPS must produce deadline");
        let interval = FrameRateCap::Fps240.target_frame_duration().unwrap();
        assert_eq!(d1, t0 + interval);

        // Simulate next frame arriving slightly early/on-time: deadline advances without drift
        pacer.begin_frame(t0 + interval);
        assert_eq!(pacer.next_deadline(), Some(t0 + interval * 2));

        // Switching to Uncapped clears deadline
        pacer.set_cap(FrameRateCap::Uncapped);
        pacer.begin_frame(t0 + interval * 2);
        assert_eq!(pacer.next_deadline(), None);
        assert!(pacer.is_ready_for_frame(t0));
    }
}
