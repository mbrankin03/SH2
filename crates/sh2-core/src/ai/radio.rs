//! Proximity-based Pocket Radio static audio modulation (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c` (`enSetRadioVolume`), `RadioNoise`, `sd_radio`, `shSdRadio`
//! - Symbol addresses:
//!   - `enSetRadioVolume`: `0x00143890` (calculates closest enemy distance and outputs radio gain / frequency)
//!   - `RadioNoise`: `0x001ab300` (drives noise generator parameters)
//!
//! # Mechanics:
//! - The pocket radio emits static white noise when active creatures are within range.
//! - Outer threshold: ~4,500.0 units (inaudible beyond this).
//! - Maximum volume threshold: ~800.0 units (max loudness).
//! - Static intensity formula: $I = \text{clamp}\left(1.0 - \frac{d - d_{\min}}{d_{\max} - d_{\min}}, 0.0, 1.0\right)$
//! - Frequency jitter / modulation is calculated to emulate radio tuning instability.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Radio static state computed every tick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RadioStaticOutput {
    /// Whether radio is powered on in inventory.
    pub is_active: bool,
    /// Proximity volume gain [0.0, 1.0].
    pub volume: f32,
    /// Distance to the nearest menacing enemy (units).
    pub nearest_enemy_dist: f32,
    /// Radio static pitch / noise frequency jitter modulation [0.8, 1.2].
    pub frequency_mod: f32,
}

impl Default for RadioStaticOutput {
    fn default() -> Self {
        Self {
            is_active: true,
            volume: 0.0,
            nearest_enemy_dist: f32::MAX,
            frequency_mod: 1.0,
        }
    }
}

/// Evaluator for pocket radio proximity noise (`0x00143890`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioNoiseEvaluator {
    /// Min distance where static reaches maximum volume (units).
    pub min_dist: f32,
    /// Max distance where static first becomes audible (units).
    pub max_dist: f32,
    /// Internal frame phase accumulator for noise pitch jitter.
    pub phase: f32,
}

impl Default for RadioNoiseEvaluator {
    fn default() -> Self {
        Self {
            min_dist: 800.0,
            max_dist: 4500.0,
            phase: 0.0,
        }
    }
}

impl RadioNoiseEvaluator {
    pub fn new(min_dist: f32, max_dist: f32) -> Self {
        Self {
            min_dist,
            max_dist,
            phase: 0.0,
        }
    }

    /// Update radio static modulation based on player position and active enemies.
    pub fn evaluate(
        &mut self,
        radio_enabled: bool,
        player_pos: Vec3,
        enemy_positions: &[Vec3],
        dt: f32,
    ) -> RadioStaticOutput {
        if !radio_enabled || enemy_positions.is_empty() {
            return RadioStaticOutput {
                is_active: radio_enabled,
                volume: 0.0,
                nearest_enemy_dist: f32::MAX,
                frequency_mod: 1.0,
            };
        }

        self.phase += dt * 8.0;
        let jitter = 1.0 + 0.1 * (self.phase.sin() + (self.phase * 2.7).cos() * 0.5);

        let mut nearest_sq = f32::MAX;
        for &ep in enemy_positions {
            let dx = ep.x - player_pos.x;
            let dz = ep.z - player_pos.z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq < nearest_sq {
                nearest_sq = dist_sq;
            }
        }

        let nearest_dist = nearest_sq.sqrt();
        let volume = if nearest_dist >= self.max_dist {
            0.0
        } else if nearest_dist <= self.min_dist {
            1.0
        } else {
            1.0 - (nearest_dist - self.min_dist) / (self.max_dist - self.min_dist)
        };

        RadioStaticOutput {
            is_active: radio_enabled,
            volume,
            nearest_enemy_dist: nearest_dist,
            frequency_mod: jitter.clamp(0.8, 1.2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radio_proximity_attenuation() {
        let mut radio = RadioNoiseEvaluator::default();
        let player = Vec3::new(0.0, 0.0, 0.0);

        // Outside max range (5000 units): volume is 0
        let out_far = radio.evaluate(true, player, &[Vec3::new(0.0, 0.0, 5000.0)], 0.016);
        assert_eq!(out_far.volume, 0.0);

        // Close range (500 units <= 800): volume is 1.0
        let out_close = radio.evaluate(true, player, &[Vec3::new(0.0, 0.0, 500.0)], 0.016);
        assert_eq!(out_close.volume, 1.0);

        // Mid-range (2650 units): half-way between 800 and 4500
        let out_mid = radio.evaluate(true, player, &[Vec3::new(0.0, 0.0, 2650.0)], 0.016);
        assert!((out_mid.volume - 0.5).abs() < 0.05);

        // Disabled radio gives 0 volume
        let out_disabled = radio.evaluate(false, player, &[Vec3::new(0.0, 0.0, 500.0)], 0.016);
        assert_eq!(out_disabled.volume, 0.0);
        assert!(!out_disabled.is_active);
    }
}
