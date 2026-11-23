//! 1:1 port of `Fog/fog_blow.c` (`SLUS_202.28` `0x00274ca0..0x00275480`):
//! Eddie boss meat-locker / refrigerated corridor steam-vent blow particles.
//!
//! All three retail emitter tables (`blow_point_ps185` @ `0x00372b70`, `blow_point_ps189` @ `0x00372d30`,
//! `blow_point_ps193` @ `0x00372e70`) are extracted directly from `SLUS_202.28`.

use glam::Vec3;

use super::particles::GpuFogParticle;

/// Maximum simultaneous blow-fog particles (`if (_fwork2 < 200)` in `fogBlow` `0x00274dc0`).
pub const MAX_BLOW_FOG_PARTICLES: usize = 200;

/// XZ culling distance from the active fog origin (`ABS(dx) <= 2000.0 && ABS(dz) <= 2000.0` in `0x00274dc0`).
pub const BLOW_SPAWN_CULL_EXTENT_MM: f32 = 2000.0;

/// Vertical buoyancy acceleration per second (`dt * 4.9` in `fogMoveParticle2` `0x00274f30`).
pub const BLOW_BUOYANCY_PER_SEC: f32 = 4.9;

/// Particle radius growth rate per second (`dt * 50.0` in `fogMoveParticle2` `0x00274f30`).
pub const BLOW_SIZE_GROWTH_PER_SEC: f32 = 50.0;

/// Per-tick velocity damping (`0x3f733333` = `0.95` in `fog_part_blow` `0x00275310`).
pub const BLOW_VELOCITY_DRAG: f32 = 0.95;

/// Per-tick fade-out alpha multiplier when despawning (`* 0.75` in `fogMoveParticle2` `0x00274f30`).
pub const BLOW_FADEOUT_ALPHA_MULT: f32 = 0.75;

/// Room IDs handled by `fogSetBlowPoint` (`0x00274cb0`):
/// - `0x8e`: Meat locker corridor (`ps185`, 14 emitters, interval `20` = `0x14`)
/// - `0x8f`: Eddie boss room 1 (`ps189`, 10 emitters, interval `30` = `0x1e`)
/// - `0x90`: Eddie boss room 2 (`ps193`, 16 emitters, interval `60` = `0x3c`)
pub const ROOM_BLOW_PS185: u32 = 0x8e;
pub const ROOM_BLOW_PS189: u32 = 0x8f;
pub const ROOM_BLOW_PS193: u32 = 0x90;

/// Single 32-byte emitter entry from `blow_point_ps185` / `ps189` / `ps193` (`SLUS_202.28`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogBlowEmitter {
    /// World-space vent origin in PS2 mm.
    pub position: [f32; 3],
    /// Initial exhaust velocity in PS2 mm/frame.
    pub velocity: [f32; 3],
    /// Initial sprite size in PS2 mm (`param_1[1] + 0xc`).
    pub initial_size_mm: f32,
}

/// Retail `blow_point_ps185` table (`SLUS_202.28` `0x00372b70`, 14 entries, room `0x8e`).
pub const BLOW_POINT_PS185: [FogBlowEmitter; 14] = [
    FogBlowEmitter {
        position: [87_700.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [90_100.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [92_500.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [94_900.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [97_300.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [99_700.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [102_100.0, -300.0, -59_400.0],
        velocity: [0.0, 1.0, -3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [87_700.0, -300.0, -60_600.0],
        velocity: [0.0, 0.0, 3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [90_100.0, -300.0, -60_600.0],
        velocity: [0.0, 1.0, 3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [92_500.0, -300.0, -60_600.0],
        velocity: [0.0, 1.0, 3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [94_900.0, -300.0, -60_600.0],
        velocity: [0.0, 1.0, 3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [97_300.0, -300.0, -60_600.0],
        velocity: [0.0, 1.0, 3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [99_700.0, -300.0, -60_600.0],
        velocity: [0.0, 1.0, 3.0],
        initial_size_mm: 10.0,
    },
    FogBlowEmitter {
        position: [102_100.0, -300.0, -60_600.0],
        velocity: [0.0, 1.0, 3.0],
        initial_size_mm: 10.0,
    },
];

/// Retail `blow_point_ps189` table (`SLUS_202.28` `0x00372d30`, 10 entries, room `0x8f`).
pub const BLOW_POINT_PS189: [FogBlowEmitter; 10] = [
    FogBlowEmitter {
        position: [101_600.0, -975.0, -99_600.0],
        velocity: [-5.0, 1.0, 0.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [100_950.0, -50.0, -99_750.0],
        velocity: [-4.0, 0.0, 2.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [100_850.0, -50.0, -100_450.0],
        velocity: [-5.0, 0.0, -1.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [101_400.0, -300.0, -100_800.0],
        velocity: [0.0, 2.0, 0.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [101_000.0, -650.0, -101_200.0],
        velocity: [0.0, 2.0, 0.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [100_500.0, -50.0, -100_600.0],
        velocity: [-1.0, 0.0, 5.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [99_600.0, -50.0, -100_750.0],
        velocity: [2.0, 0.0, 3.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [99_000.0, -300.0, -101_250.0],
        velocity: [0.0, 2.0, 0.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [99_150.0, -50.0, -100_900.0],
        velocity: [3.0, 0.0, 1.0],
        initial_size_mm: 40.0,
    },
    FogBlowEmitter {
        position: [99_000.0, -50.0, -100_200.0],
        velocity: [3.0, 0.0, 2.0],
        initial_size_mm: 40.0,
    },
];

/// Retail `blow_point_ps193` table (`SLUS_202.28` `0x00372e70`, 16 entries, room `0x90`).
pub const BLOW_POINT_PS193: [FogBlowEmitter; 16] = [
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -96_800.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -97_200.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -98_000.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -98_400.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -99_400.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -99_800.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -100_600.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -101_000.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -102_000.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -102_400.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -103_200.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_900.0, -1400.0, -103_600.0],
        velocity: [-3.0, 2.0, 0.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [142_200.0, -1400.0, -104_100.0],
        velocity: [0.0, 2.0, 3.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [141_800.0, -1400.0, -104_100.0],
        velocity: [0.0, 2.0, 3.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [138_600.0, -1400.0, -104_100.0],
        velocity: [0.0, 2.0, 3.0],
        initial_size_mm: 60.0,
    },
    FogBlowEmitter {
        position: [138_200.0, -1400.0, -104_100.0],
        velocity: [0.0, 2.0, 3.0],
        initial_size_mm: 60.0,
    },
];

/// Active blow-fog particle (`fwork[500..700]`, stride `0x60` in `Fog/fog_blow.c`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogBlowParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size_mm: f32,
    pub alpha: f32,
    pub alpha_vel: f32,
    /// `0` = live, `1` = fading out, `100` = out-of-bounds immediate despawn (`fog_part_clamp2` `0x002753c0`).
    pub state: i32,
}

/// 1:1 port of `Fog/fog_blow.c` (`fogInitParticle2`, `fogSetBlowPoint`, `fogBlow`, `fogMoveParticle2`).
#[derive(Debug, Clone)]
pub struct FogBlowSystem {
    pub active_room_id: u32,
    pub spawn_interval_frames: i32,
    pub emitter_timers: [i32; 16],
    pub particles: Vec<FogBlowParticle>,
    pub max_size_mm: f32,
    pub target_alpha: f32,
}

impl Default for FogBlowSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FogBlowSystem {
    /// Initializes an empty blow-fog system (`fogInitParticle2` `0x00274ca0`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_room_id: 0,
            spawn_interval_frames: 0,
            emitter_timers: [0; 16],
            particles: Vec::with_capacity(MAX_BLOW_FOG_PARTICLES),
            max_size_mm: 400.0,
            target_alpha: 64.0,
        }
    }

    /// Selects the emitter table for `room_id` (`fogSetBlowPoint` `0x00274cb0`).
    pub fn set_room(&mut self, room_id: u32) {
        self.active_room_id = room_id;
        self.spawn_interval_frames = match room_id {
            ROOM_BLOW_PS193 => 60, // 0x3c
            ROOM_BLOW_PS189 => 30, // 0x1e
            ROOM_BLOW_PS185 => 20, // 0x14
            _ => 0,
        };
        self.emitter_timers = [0; 16];
    }

    /// Returns the static emitter slice for the active room (`fogSetBlowPoint` `0x00274cb0`).
    #[must_use]
    pub fn active_emitters(&self) -> &'static [FogBlowEmitter] {
        match self.active_room_id {
            ROOM_BLOW_PS193 => &BLOW_POINT_PS193,
            ROOM_BLOW_PS189 => &BLOW_POINT_PS189,
            ROOM_BLOW_PS185 => &BLOW_POINT_PS185,
            _ => &[],
        }
    }

    /// Spawns one particle from `emitter` if within 2000 mm XZ of `local_origin` (`fogBlow` `0x00274dc0`).
    pub fn spawn_from_emitter(&mut self, emitter: &FogBlowEmitter, local_origin: Vec3) -> bool {
        if self.particles.len() >= MAX_BLOW_FOG_PARTICLES {
            return false;
        }
        let pos = Vec3::from_array(emitter.position);
        let delta = pos - local_origin;
        if delta.x.abs() > BLOW_SPAWN_CULL_EXTENT_MM || delta.z.abs() > BLOW_SPAWN_CULL_EXTENT_MM {
            return false;
        }
        self.particles.push(FogBlowParticle {
            position: pos,
            velocity: Vec3::from_array(emitter.velocity),
            size_mm: emitter.initial_size_mm,
            alpha: (self.target_alpha as i32 >> 2) as f32,
            alpha_vel: 0.0,
            state: 0,
        });
        true
    }

    /// Steps one frame of `fogMoveParticle2` (`0x00274f30`) + `fog_part_blow` (`0x00275310`).
    pub fn tick(&mut self, dt_sec: f32, df_frames: i32, local_origin: Vec3) {
        let emitters = self.active_emitters();
        for (i, emitter) in emitters.iter().enumerate() {
            self.emitter_timers[i] -= df_frames;
            if self.emitter_timers[i] < 1 {
                self.emitter_timers[i] = self.spawn_interval_frames;
                self.spawn_from_emitter(emitter, local_origin);
            }
        }

        let buoyancy = dt_sec * BLOW_BUOYANCY_PER_SEC;
        let size_step = dt_sec * BLOW_SIZE_GROWTH_PER_SEC;
        let dt_scale = dt_sec * 60.0;
        let target_a = self.target_alpha;
        let max_sz = self.max_size_mm;

        let mut i = 0;
        while i < self.particles.len() {
            if self.particles[i].state != 0 {
                self.particles[i].alpha *= BLOW_FADEOUT_ALPHA_MULT;
                if self.particles[i].state == 100 || self.particles[i].alpha <= 1.0 {
                    self.particles.swap_remove(i);
                    continue;
                }
            }

            let p = &mut self.particles[i];
            // `fog_part_blow` (`0x00275310`): buoyancy + growth + 0.95 velocity drag + integration
            p.velocity.y += buoyancy;
            p.size_mm = (p.size_mm + size_step).min(max_sz);
            p.velocity *= BLOW_VELOCITY_DRAG;
            p.position += p.velocity * dt_scale;

            // Spring-damped alpha from `fogMoveParticle2` (`0x00274f30` line 261)
            p.alpha_vel = (p.alpha_vel - (p.alpha - target_a) * 0.01) * 0.9;
            p.alpha = (p.alpha + p.alpha_vel).clamp(0.0, 127.0);

            // `fog_part_clamp2` (`0x002753c0`): despawn if outside 2000 mm XZ volume
            let rel = p.position - local_origin;
            if rel.x.abs() > BLOW_SPAWN_CULL_EXTENT_MM || rel.z.abs() > BLOW_SPAWN_CULL_EXTENT_MM {
                p.state = 100;
            }
            i += 1;
        }
    }

    /// Exports active blow particles as [`GpuFogParticle`] instances for [`super::FogParticlePipeline`].
    pub fn write_gpu_instances(&self, fog_rgb: [f32; 3], out: &mut Vec<GpuFogParticle>) {
        for p in &self.particles {
            out.push(GpuFogParticle {
                world_pos_radius: [p.position.x, p.position.y, p.position.z, p.size_mm],
                color_alpha: [
                    fog_rgb[0],
                    fog_rgb[1],
                    fog_rgb[2],
                    (p.alpha / 127.0).clamp(0.0, 1.0),
                ],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fog_blow_room_tables_and_lifecycle() {
        let mut sys = FogBlowSystem::new();
        sys.set_room(ROOM_BLOW_PS193);
        assert_eq!(sys.active_emitters().len(), 16);
        assert_eq!(sys.spawn_interval_frames, 60);

        sys.set_room(ROOM_BLOW_PS189);
        assert_eq!(sys.active_emitters().len(), 10);
        assert_eq!(sys.spawn_interval_frames, 30);

        sys.set_room(ROOM_BLOW_PS185);
        assert_eq!(sys.active_emitters().len(), 14);
        assert_eq!(sys.spawn_interval_frames, 20);

        // Place camera near emitter 0 of ps185 and tick
        let origin = Vec3::new(87_700.0, -300.0, -59_400.0);
        sys.tick(1.0 / 60.0, 1, origin);
        assert!(!sys.particles.is_empty());
        assert!(sys.particles.len() <= MAX_BLOW_FOG_PARTICLES);
    }
}
