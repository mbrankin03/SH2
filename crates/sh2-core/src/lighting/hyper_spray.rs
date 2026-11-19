//! Hyper Spray & Lying Figure mist particle simulation (`SLUS_202.28` `enEfctSetSpray` @ `0x0014fa70`,
//! `enEfctMoveSpray` @ `0x0014fcb0`, `shBattleAttackByHumanFog` @ `0x00121da0`, `sh2pc.exe` `0x4a4c00` /
//! `0x4a3f60`, and SH2:EE `Patches/SprayEffect.cpp` 60 Hz speed parity).

use glam::Vec3;

/// Maximum simultaneous spray mist puff particles in the pool.
pub const MAX_SPRAY_PARTICLES: usize = 128;

/// Authentic PS2 Hyper Spray lifetime in 30 Hz frames (`0x00121e44`: `addiu $7, $zero, 0x0c` = 12 frames = 0.40 s).
pub const HYPER_SPRAY_LIFETIME_30HZ: f32 = 12.0;

/// SH2:EE 60 Hz spray speed factors (`Patches/SprayEffect.cpp:23-28`).
pub const HYPER_SPRAY_SPEED_FACTOR_0_1: f32 = 0.15;
pub const HYPER_SPRAY_SPEED_FACTOR_2: f32 = 0.25;
pub const LYING_FIGURE_SPEED_FACTOR_0: f32 = 0.1;
pub const LYING_FIGURE_SPEED_FACTOR_1: f32 = 0.125;
pub const LYING_FIGURE_SPEED_FACTOR_2: f32 = 0.15;
pub const SPRAY_Y_ADJUST_60HZ: f32 = 50.0;

/// 4 unlockable Hyper Spray color tiers (`SLUS_202.28` `shBattleAttackByHumanFog` @ `0x00121da0`,
/// `sh2pc.exe` `0x1dbeb89` signed byte `-1, 0, 1, 2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HyperSprayColorTier {
    /// `spray_power == -1`: `0xff400040` (`R=64, G=0, B=64`, `0x00121dd8`).
    Purple,
    /// `spray_power == 0`: `0xffc0c0c0` (`R=192, G=192, B=192`, `0x00121de8`).
    #[default]
    White,
    /// `spray_power == 1`: `0xff40c0c0` (`R=192, G=192, B=64`, `0x00121df8`).
    Yellow,
    /// `spray_power == 2`: `0xff40ff40` (`R=64, G=255, B=64`, `0x00121e08`).
    Green,
}

impl HyperSprayColorTier {
    /// Decodes the signed byte stored at `SLUS_202.28` `0x0122b319` / `sh2pc.exe` `0x1dbeb89`.
    #[must_use]
    pub const fn from_spray_power(power: i8) -> Self {
        match power {
            -1 => Self::Purple,
            1 => Self::Yellow,
            2 => Self::Green,
            _ => Self::White,
        }
    }

    /// Returns the exact PS2 GS linear RGB color (`0.0..=1.0`) from `shBattleAttackByHumanFog` (`0x00121dd8..0x00121e0c`).
    #[must_use]
    pub const fn rgb(self) -> [f32; 3] {
        match self {
            Self::Purple => [0x40 as f32 / 255.0, 0.0, 0x40 as f32 / 255.0],
            Self::White => [
                0xc0 as f32 / 255.0,
                0xc0 as f32 / 255.0,
                0xc0 as f32 / 255.0,
            ],
            Self::Yellow => [
                0xc0 as f32 / 255.0,
                0xc0 as f32 / 255.0,
                0x40 as f32 / 255.0,
            ],
            Self::Green => [
                0x40 as f32 / 255.0,
                0xff as f32 / 255.0,
                0x40 as f32 / 255.0,
            ],
        }
    }
}

/// Lifecycle phase of a single spray particle (`enEfctSetSpray` @ `0x0014fbb0`: `0.6 / 0.3 / 0.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SprayParticlePhase {
    /// Phase 0 (`0.6 * lifetime`): rapid jet expansion.
    Expand,
    /// Phase 1 (`0.3 * lifetime`): cloud linger & slow drift.
    Linger,
    /// Phase 2 (`0.3 * lifetime`): dissipation & alpha fade-out.
    Dissipate,
}

/// One active 3D spray mist billboard particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SprayParticle {
    /// World-space position in PS2 mm (Y-down).
    pub position: Vec3,
    /// Velocity vector in PS2 mm per 30 Hz frame.
    pub velocity: Vec3,
    /// Current billboard radius in PS2 mm.
    pub radius_mm: f32,
    /// Rotation angle in radians.
    pub rotation: f32,
    /// Elapsed age in seconds.
    pub age_secs: f32,
    /// Total lifetime in seconds.
    pub lifetime_secs: f32,
    /// Base RGB color (including dark-world modulation `48/128` or `24/128`, `0x0014fc58` / `0x0014fc74`).
    pub color_rgb: [f32; 3],
    /// Current alpha opacity (`0.0..=1.0`).
    pub alpha: f32,
    /// True for player Hyper Spray; false for Lying Figure acid mist.
    pub is_hyper_spray: bool,
}

impl SprayParticle {
    /// Returns the current lifecycle phase (`0.6 / 0.3 / 0.3` split from `enEfctSetSpray`).
    #[must_use]
    pub fn phase(&self) -> SprayParticlePhase {
        let t = (self.age_secs / self.lifetime_secs.max(1e-4)).clamp(0.0, 1.0);
        if t < 0.5 {
            SprayParticlePhase::Expand
        } else if t < 0.75 {
            SprayParticlePhase::Linger
        } else {
            SprayParticlePhase::Dissipate
        }
    }
}

/// Fixed-capacity pool managing Hyper Spray and Lying Figure mist clouds.
#[derive(Debug, Clone)]
pub struct HyperSprayEmitter {
    particles: Vec<SprayParticle>,
    rng_state: u32,
    spawn_frame_parity: bool,
}

impl Default for HyperSprayEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl HyperSprayEmitter {
    /// Creates an empty spray particle pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(MAX_SPRAY_PARTICLES),
            rng_state: 0x51A7_2001,
            spawn_frame_parity: false,
        }
    }

    /// Returns the slice of currently active spray particles.
    #[inline]
    #[must_use]
    pub fn particles(&self) -> &[SprayParticle] {
        &self.particles
    }

    /// Clears all active spray particles.
    pub fn clear(&mut self) {
        self.particles.clear();
    }

    /// Spawns a Hyper Spray puff (`enEfctSetSpray` @ `0x0014fa70`, `sh2pc.exe` `0x4a4c00`).
    ///
    /// Applies SH2:EE's every-other-60Hz-frame spawn gate (`SprayEffect.cpp:42`) when `rate_limit_60hz`
    /// is true, and dark-world color modulation (`0x30/0x80` lit, `0x18/0x80` unlit, `0x0014fc58`).
    pub fn spawn_hyper_spray(
        &mut self,
        origin: Vec3,
        forward_dir: Vec3,
        tier: HyperSprayColorTier,
        dark_world: bool,
        flashlight_on: bool,
        rate_limit_60hz: bool,
    ) {
        if rate_limit_60hz {
            self.spawn_frame_parity = !self.spawn_frame_parity;
            if !self.spawn_frame_parity && !self.particles.is_empty() {
                return;
            }
        }
        if self.particles.len() >= MAX_SPRAY_PARTICLES {
            self.particles.remove(0);
        }

        let dir = forward_dir.normalize_or(Vec3::NEG_Z);
        // `enEfctSetSpray` @ `0x0014fae8`: `shSway1f(-50.0, 1.0)` jitter per axis
        let jitter = Vec3::new(
            self.next_signed() * 50.0,
            self.next_signed() * 50.0,
            self.next_signed() * 50.0,
        );
        let velocity = dir * 240.0 + jitter;

        let mut rgb = tier.rgb();
        if dark_world {
            let scale = if flashlight_on {
                48.0 / 128.0
            } else {
                24.0 / 128.0
            };
            rgb = [rgb[0] * scale, rgb[1] * scale, rgb[2] * scale];
        }

        let lifetime_secs = (HYPER_SPRAY_LIFETIME_30HZ * 1.2) / 30.0; // 0.6 + 0.3 + 0.3 = 1.2x
        let rotation = self.next_unit() * std::f32::consts::TAU;
        self.particles.push(SprayParticle {
            position: origin,
            velocity,
            radius_mm: 95.0,
            rotation,
            age_secs: 0.0,
            lifetime_secs,
            color_rgb: rgb,
            alpha: 0.85,
            is_hyper_spray: true,
        });
    }

    /// Advances all active spray particles by `dt_seconds` (`enEfctMoveSpray` @ `0x0014fcb0`,
    /// `sh2pc.exe` `0x4a3f60`, SH2:EE `SprayEffect.cpp`).
    pub fn update(&mut self, dt_seconds: f32) {
        let dt = dt_seconds.max(0.0);
        let ticks_60hz = dt * 60.0;

        self.particles.retain_mut(|p| {
            p.age_secs += dt;
            if p.age_secs >= p.lifetime_secs {
                return false;
            }
            let speed_factor = match (p.is_hyper_spray, p.phase()) {
                (true, SprayParticlePhase::Expand | SprayParticlePhase::Linger) => {
                    HYPER_SPRAY_SPEED_FACTOR_0_1
                }
                (true, SprayParticlePhase::Dissipate) => HYPER_SPRAY_SPEED_FACTOR_2,
                (false, SprayParticlePhase::Expand) => LYING_FIGURE_SPEED_FACTOR_0,
                (false, SprayParticlePhase::Linger) => LYING_FIGURE_SPEED_FACTOR_1,
                (false, SprayParticlePhase::Dissipate) => LYING_FIGURE_SPEED_FACTOR_2,
            };

            p.position += p.velocity * (speed_factor * ticks_60hz);
            // Gentle upward billow in PS2 Y-down space
            p.position.y -= SPRAY_Y_ADJUST_60HZ * 0.05 * ticks_60hz;
            p.radius_mm += 260.0 * dt;
            p.rotation += 0.9 * dt;

            let t = (p.age_secs / p.lifetime_secs).clamp(0.0, 1.0);
            p.alpha = if t < 0.25 {
                (t / 0.25) * 0.85
            } else {
                ((1.0 - t) / 0.75) * 0.85
            };
            true
        });
    }

    fn next_unit(&mut self) -> f32 {
        // Xorshift32 deterministic PRNG (zero external RNG dependencies)
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        (x as f32) / (u32::MAX as f32)
    }

    fn next_signed(&mut self) -> f32 {
        self.next_unit() * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyper_spray_color_tiers_match_slus_202_28_0x121da0() {
        assert_eq!(
            HyperSprayColorTier::from_spray_power(-1),
            HyperSprayColorTier::Purple
        );
        assert_eq!(
            HyperSprayColorTier::from_spray_power(0),
            HyperSprayColorTier::White
        );
        assert_eq!(
            HyperSprayColorTier::from_spray_power(1),
            HyperSprayColorTier::Yellow
        );
        assert_eq!(
            HyperSprayColorTier::from_spray_power(2),
            HyperSprayColorTier::Green
        );

        let green = HyperSprayColorTier::Green.rgb();
        assert!((green[1] - 1.0).abs() < 1e-4);
        assert!((green[0] - 64.0 / 255.0).abs() < 1e-4);
    }

    #[test]
    fn emitter_spawns_advances_and_expires_particles() {
        let mut emitter = HyperSprayEmitter::new();
        emitter.spawn_hyper_spray(
            Vec3::ZERO,
            Vec3::NEG_Z,
            HyperSprayColorTier::Green,
            false,
            true,
            false,
        );
        assert_eq!(emitter.particles().len(), 1);
        let initial_r = emitter.particles()[0].radius_mm;

        emitter.update(0.1);
        assert_eq!(emitter.particles().len(), 1);
        assert!(emitter.particles()[0].radius_mm > initial_r);

        emitter.update(1.0);
        assert!(emitter.particles().is_empty());
    }
}
