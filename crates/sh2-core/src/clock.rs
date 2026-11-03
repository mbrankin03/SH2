//! Simulation tick clock supporting decoupled 60+ FPS ticks.
//!
//! Provides fixed-timestep simulation ticking with smooth subframe interpolation (alpha).

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EngineClock {
    /// Target simulation ticks per second (clamped to at least 1 TPS).
    pub target_tps: u32,
    /// Duration of each discrete simulation tick.
    pub tick_duration: Duration,
    /// Accumulated real time waiting to be simulated.
    pub accumulated: Duration,
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
        }
    }

    /// Advance the accumulator with real elapsed delta time.
    pub fn advance(&mut self, delta: Duration) {
        self.accumulated += delta;
    }

    /// Check if enough time has elapsed to consume a fixed simulation tick.
    pub fn step(&mut self) -> bool {
        if self.accumulated >= self.tick_duration {
            self.accumulated -= self.tick_duration;
            true
        } else {
            false
        }
    }

    /// Compute the subframe interpolation factor (0.0 .. 1.0) for smooth rendering between physics ticks.
    pub fn alpha(&self) -> f32 {
        if self.tick_duration.is_zero() {
            return 1.0;
        }
        (self.accumulated.as_secs_f64() / self.tick_duration.as_secs_f64()).clamp(0.0, 1.0) as f32
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
}
