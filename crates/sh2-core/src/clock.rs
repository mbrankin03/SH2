//! Simulation tick clock supporting decoupled 60+ FPS ticks.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EngineClock {
    pub target_tps: u32,
    pub tick_duration: Duration,
    pub accumulated: Duration,
}

impl Default for EngineClock {
    fn default() -> Self {
        Self::new(60)
    }
}

impl EngineClock {
    pub fn new(target_tps: u32) -> Self {
        Self {
            target_tps,
            tick_duration: Duration::from_secs_f64(1.0 / target_tps as f64),
            accumulated: Duration::ZERO,
        }
    }
}
