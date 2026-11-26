//! Deterministic localized 3D proximity fog particle volume around James and the camera (`SH2-32`).
//!
//! Mirrors PS2 `fogInitParticle` (`0x00169d90`), `fogMoveParticle` (`0x0016a540`),
//! `fog_part_clamp` (`0x0016ba50`), `fog_part_alp` (`0x0016b9a0`), `fog_part_obj` (`0x0016b780`),
//! `fogChangeWind` (`0x00169380`), and `shSway1f_asm` (`0x0016f0a0`, called from `fog_part_alp` @ `0x0016ba00`).

use crate::cutscene::{OBSERVATION_DECK_FOG_BORDER_WIDTH, OBSERVATION_DECK_FOG_PARTICLE_MAX};

use super::PS2_DEFAULT_FOG_HEIGHT_BAND_MM;

/// Single localized 3D proximity fog particle (`fogInitParticle` @ `0x00169d90`,
/// `fogMoveParticle` @ `0x0016a540` in `SLUS_202.28`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogParticle {
    /// Local offset relative to the volume anchor in PS2 mm `(x, y, z)`.
    pub local_pos: [f32; 3],
    /// Particle drift velocity in PS2 mm/s `(vx, vy, vz)`.
    pub velocity: [f32; 3],
    /// Billboard quad half-size in PS2 mm.
    pub radius_mm: f32,
    /// Per-particle phase offset in radians for subtle vertical sway (`shSway1f_asm` @ `0x0016f0a0`).
    pub sway_phase: f32,
    /// Evaluated alpha in `0.0..=1.0` after border fade (`fog_part_alp` @ `0x0016b9a0`) and proximity parting.
    pub alpha: f32,
}

/// Wraps a coordinate into `[-half_extent, +half_extent]` regardless of displacement magnitude,
/// returning `(wrapped_value, wrapped_odd_count)` so X-axis wraps can flip Z (`ClampFogParticleXPosASM`).
#[inline]
fn wrap_symmetric_with_parity(val: f32, half_extent: f32) -> (f32, bool) {
    let span = 2.0 * half_extent;
    if val >= -half_extent && val <= half_extent {
        return (val, false);
    }
    let shifted = val + half_extent;
    let wraps = shifted.div_euclid(span) as i64;
    let wrapped = shifted.rem_euclid(span) - half_extent;
    (wrapped, (wraps & 1) != 0)
}

/// Deterministic localized 3D proximity fog particle volume around James and the camera.
///
/// Mirrors PS2 `fogInitParticle` (`0x00169d90`), `fogMoveParticle` (`0x0016a540`),
/// `fog_part_clamp` (`0x0016ba50`), `fog_part_alp` (`0x0016b9a0`), and `fog_part_obj` (`0x0016b780`).
#[derive(Debug, Clone)]
pub struct FogParticleVolume {
    particles: Vec<FogParticle>,
    anchor_world: [f32; 3],
    range_x_mm: f32,
    range_z_mm: f32,
    wind_velocity: [f32; 3],
    elapsed_secs: f32,
}

impl FogParticleVolume {
    /// Creates a deterministic particle volume populated with `count` sprites inside
    /// `[-range_x_mm, +range_x_mm] x [-100, +100] x [-range_z_mm, +range_z_mm]`.
    #[must_use]
    pub fn new(count: usize, range_x_mm: f32, range_z_mm: f32) -> Self {
        let clamped_count = count.min(OBSERVATION_DECK_FOG_PARTICLE_MAX);
        let rx = range_x_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        let rz = range_z_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);

        let mut rng_state: u32 = 0x51A2_F061;
        let mut next_unit = || -> f32 {
            // Deterministic LCG (Numerical Recipes constants) for 100% reproducible particle layout.
            rng_state = rng_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            ((rng_state >> 8) as f32) / ((1u32 << 24) as f32)
        };

        let mut particles = Vec::with_capacity(clamped_count);
        for _ in 0..clamped_count {
            let x = (next_unit() * 2.0 - 1.0) * rx;
            let y = (next_unit() * 2.0 - 1.0) * PS2_DEFAULT_FOG_HEIGHT_BAND_MM;
            let z = (next_unit() * 2.0 - 1.0) * rz;
            let vx = (next_unit() - 0.5) * 80.0;
            let vy = (next_unit() - 0.5) * 18.0;
            let vz = (next_unit() - 0.5) * 80.0;
            let radius_mm = 300.0 + next_unit() * 140.0;
            let sway_phase = next_unit() * std::f32::consts::TAU;
            particles.push(FogParticle {
                local_pos: [x, y, z],
                velocity: [vx, vy, vz],
                radius_mm,
                sway_phase,
                alpha: 0.0,
            });
        }

        Self {
            particles,
            anchor_world: [0.0, 0.0, 0.0],
            range_x_mm: rx,
            range_z_mm: rz,
            // Gentle PS2 outdoor breeze (`fogChangeWind` @ `0x00169380`)
            wind_velocity: [110.0, 6.0, -75.0],
            elapsed_secs: 0.0,
        }
    }

    /// Reconfigures the particle count and volume extents when switching rooms.
    pub fn reconfigure(&mut self, count: usize, range_x_mm: f32, range_z_mm: f32) {
        let clamped_count = count.min(OBSERVATION_DECK_FOG_PARTICLE_MAX);
        let rx = range_x_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        let rz = range_z_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        if self.particles.len() != clamped_count
            || (self.range_x_mm - rx).abs() > 1.0
            || (self.range_z_mm - rz).abs() > 1.0
        {
            *self = Self::new(clamped_count, rx, rz);
        }
    }

    /// Sets the active room's wind velocity in PS2 mm/s (`fogChangeWind` @ `0x00169380`).
    pub fn set_wind_velocity(&mut self, wind_mm_per_sec: [f32; 3]) {
        self.wind_velocity = wind_mm_per_sec;
    }

    /// Returns the immutable slice of localized particles.
    #[must_use]
    pub fn particles(&self) -> &[FogParticle] {
        &self.particles
    }

    /// Returns the current world-space anchor `(x, y, z)` in PS2 mm.
    #[must_use]
    pub fn anchor_world(&self) -> [f32; 3] {
        self.anchor_world
    }

    /// Advances all localized particles by `dt_secs` (decoupled simulation delta time), wrapping
    /// toroidally around `anchor_world` even across large teleports, applying wind drift scaled by
    /// `speed_mult`, fading near volume borders (`kFogBorderWidth = 300.0` mm), and parting particles
    /// radially away from `player_world_pos` (`fog_part_obj` @ `0x0016b780`, `JamesFogInfluence`).
    pub fn step(
        &mut self,
        dt_secs: f32,
        speed_mult: f32,
        anchor_world: [f32; 3],
        player_world_pos: [f32; 3],
        james_fog_influence_mm: f32,
    ) {
        if self.particles.is_empty() {
            self.anchor_world = anchor_world;
            return;
        }

        let delta_anchor = [
            anchor_world[0] - self.anchor_world[0],
            anchor_world[1] - self.anchor_world[1],
            anchor_world[2] - self.anchor_world[2],
        ];
        self.anchor_world = anchor_world;
        let scaled_dt = dt_secs.max(0.0) * speed_mult.max(0.0);
        self.elapsed_secs += scaled_dt;

        let rx = self.range_x_mm;
        let rz = self.range_z_mm;
        let border = OBSERVATION_DECK_FOG_BORDER_WIDTH;
        let influence = james_fog_influence_mm.max(1.0);
        let influence_sq = influence * influence;

        for p in &mut self.particles {
            p.local_pos[0] += (self.wind_velocity[0] + p.velocity[0]) * scaled_dt - delta_anchor[0];
            p.local_pos[1] += (self.wind_velocity[1] + p.velocity[1]) * scaled_dt - delta_anchor[1];
            p.local_pos[2] += (self.wind_velocity[2] + p.velocity[2]) * scaled_dt - delta_anchor[2];

            // Modulo toroidal wrap along X with Z-flip (`ClampFogParticleXPosASM`, `ObservationDeckFogFix.cpp:77-119`)
            let (wx_local, flip_z) = wrap_symmetric_with_parity(p.local_pos[0], rx);
            p.local_pos[0] = wx_local;
            if flip_z {
                p.local_pos[2] = -p.local_pos[2];
            }

            // Modulo toroidal wrap along Z and vertical height band (handles large teleports cleanly)
            p.local_pos[2] = wrap_symmetric_with_parity(p.local_pos[2], rz).0;
            p.local_pos[1] =
                wrap_symmetric_with_parity(p.local_pos[1], PS2_DEFAULT_FOG_HEIGHT_BAND_MM).0;

            // Radial parting around James (`fog_part_obj` @ `0x0016b780`, `JamesFogInfluence`)
            let wx = self.anchor_world[0] + p.local_pos[0];
            let wz = self.anchor_world[2] + p.local_pos[2];
            let dx = wx - player_world_pos[0];
            let dz = wz - player_world_pos[2];
            let dist_sq = dx * dx + dz * dz;
            let mut parting_alpha = 1.0f32;
            if dist_sq < influence_sq {
                let dist = dist_sq.sqrt().max(1e-3);
                let push = (influence - dist) * 0.35;
                p.local_pos[0] =
                    wrap_symmetric_with_parity(p.local_pos[0] + (dx / dist) * push, rx).0;
                p.local_pos[2] =
                    wrap_symmetric_with_parity(p.local_pos[2] + (dz / dist) * push, rz).0;
                parting_alpha = (dist / influence).clamp(0.15, 1.0);
            }

            // Smooth border alpha fade (`fog_part_alp` @ `0x0016b9a0`, `FadeFogParticleXPosASM`)
            let ax = if p.local_pos[0] < -rx + border {
                ((rx + p.local_pos[0]) / border).clamp(0.0, 1.0)
            } else if p.local_pos[0] > rx - border {
                ((rx - p.local_pos[0]) / border).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let az = if p.local_pos[2] < -rz + border {
                ((rz + p.local_pos[2]) / border).clamp(0.0, 1.0)
            } else if p.local_pos[2] > rz - border {
                ((rz - p.local_pos[2]) / border).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let sway = 0.85 + 0.15 * (self.elapsed_secs * 1.1 + p.sway_phase).sin();
            p.alpha = (ax * az * parting_alpha * sway).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_volume_wraps_and_parts_around_james() {
        let mut vol = FogParticleVolume::new(64, 2000.0, 2000.0);
        vol.particles[0].local_pos = [10.0, 0.0, 0.0];
        vol.particles[0].velocity = [0.0, 0.0, 0.0];
        vol.step(1.0 / 60.0, 0.25, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 200.0);
        assert!(
            vol.particles[0].local_pos[0] > 10.0,
            "particle near James should be pushed outward"
        );
        for p in vol.particles() {
            assert!(p.local_pos[0].abs() <= 2000.0 + 1e-3);
            assert!(p.local_pos[2].abs() <= 2000.0 + 1e-3);
            assert!((0.0..=1.0).contains(&p.alpha));
        }
    }

    #[test]
    fn particle_volume_survives_large_anchor_teleport() {
        let mut vol = FogParticleVolume::new(128, 2000.0, 2000.0);
        // Simulate teleporting 100,000 mm across town/room transition
        vol.step(
            1.0 / 60.0,
            0.25,
            [100_000.0, -5000.0, -85_000.0],
            [100_000.0, -5000.0, -85_000.0],
            200.0,
        );
        let visible_count = vol.particles().iter().filter(|p| p.alpha > 0.1).count();
        assert!(
            visible_count > 64,
            "particles must remain wrapped and visible after large teleport"
        );
        for p in vol.particles() {
            assert!(p.local_pos[0].abs() <= 2000.0 + 1e-3);
            assert!(p.local_pos[1].abs() <= PS2_DEFAULT_FOG_HEIGHT_BAND_MM + 1e-3);
            assert!(p.local_pos[2].abs() <= 2000.0 + 1e-3);
        }
    }
}
