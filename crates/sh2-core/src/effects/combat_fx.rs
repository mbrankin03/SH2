//! Combat Brass Shell Casings, Bullet Impact Sparks & Shattering Glass (`SH2-100`).
//!
//! Ported 1:1 from:
//! - `Effect/ef_cartridge.c` (`SLUS_202.28` `0x00222420..0x002237b0`)
//! - `Effect/ef_sparks.c` (`SLUS_202.28` `0x00232590..0x00232d30`)
//! - `Effect/ef_broken_glass.c` (`SLUS_202.28` `0x0013a8f0..0x0013bc80`)
//! - `Effect2/hh_class_glass_break_phenomenon_00.c` (`SLUS_202.28` `0x00258090..0x002581f0`)
//! - `Effect2/hh_class_glass_piece_00.c` (`SLUS_202.28` `0x00258a10..0x002592e0`)

use glam::Vec3;
use std::f32::consts::PI;

use super::FxRng;

/// Weapon kind for `EFCTSetDischargeCartridge` (`0x00222420`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShellWeaponKind {
    Handgun = 1,
    Shotgun = 2,
    Rifle = 3,
}

/// Brass shell casing (`Effect/ef_cartridge.c` `0x00222420..0x00222bf0`).
#[derive(Debug, Clone, Copy)]
pub struct ShellCasing {
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot_rad: Vec3,
    pub ang_vel: Vec3,
    pub weapon: ShellWeaponKind,
    pub life_ticks: f32,
    pub moving: bool,
    pub alive: bool,
}

/// Shell casing simulation (`ef_cartridge.c`).
#[derive(Debug, Clone, Default)]
pub struct ShellCasingSystem {
    pub shells: Vec<ShellCasing>,
}

impl ShellCasingSystem {
    /// Spawns an ejected shell casing (`EFCTSetDischargeCartridge` `0x00222420`).
    pub fn spawn_shell(&mut self, origin: Vec3, weapon: ShellWeaponKind, rng: &mut FxRng) {
        let vel = match weapon {
            ShellWeaponKind::Rifle => Vec3::new(
                ((rng.next_f32() - 0.5) * 500.0) / 3.0,
                0.0,
                ((rng.next_f32() - 0.5) * 500.0) / 3.0,
            ),
            ShellWeaponKind::Handgun | ShellWeaponKind::Shotgun => Vec3::new(
                ((rng.next_f32() - 0.5) * 500.0) / 2.0 + 750.0,
                ((rng.next_f32() - 0.5) * 500.0) / 2.0 - 1000.0,
                ((rng.next_f32() - 0.5) * 500.0) / 2.0 - 400.0,
            ),
        };
        self.shells.push(ShellCasing {
            pos: origin,
            vel,
            rot_rad: Vec3::ZERO,
            ang_vel: Vec3::new(rng.next_f32() - 9.075_712, 0.0, rng.next_f32() + 7.330_383),
            weapon,
            life_ticks: 2.0,
            moving: true,
            alive: true,
        });
    }

    /// Steps shell physics against `floor_y_mm` (`efCartridgeDischargeDisp` `0x00222bf0`).
    ///
    /// On PS2 at 30 Hz (`dt = 1/30`), `10.0 / dt = 300.0` mm/s per frame (`9000.0` mm/s^2).
    /// Bounces with `vy *= -0.3`, `vx *= 0.3`, `vz *= 0.3` when `|vy| > 750.0` mm/s, else rests.
    pub fn step(&mut self, dt: f32, floor_y_mm: f32) {
        for s in &mut self.shells {
            if !s.alive {
                continue;
            }
            s.life_ticks -= dt;
            if s.life_ticks <= 0.0 {
                s.alive = false;
                continue;
            }
            if !s.moving {
                continue;
            }
            s.pos += s.vel * dt;
            s.vel.y += 9000.0 * dt;
            s.rot_rad += s.ang_vel * dt;
            if s.pos.y >= floor_y_mm {
                s.pos.y = floor_y_mm;
                if s.vel.y.abs() > 750.0 {
                    s.vel.y *= -0.3;
                    s.vel.x *= 0.3;
                    s.vel.z *= 0.3;
                } else {
                    s.vel = Vec3::ZERO;
                    s.ang_vel = Vec3::ZERO;
                    s.rot_rad.x = 0.0;
                    s.rot_rad.z = 0.0;
                    s.moving = false;
                }
            }
        }
        self.shells.retain(|s| s.alive);
    }
}

/// Bullet impact spark (`Effect/ef_sparks.c` `0x00232590..0x00232d30`).
#[derive(Debug, Clone, Copy)]
pub struct ImpactSpark {
    pub origin: Vec3,
    pub start_pos: Vec3,
    pub speed: Vec3,
    pub pos: Vec3,
    pub size_w: f32,
    pub size_h: f32,
    pub base_rgba: [f32; 4],
    pub rgba: [f32; 4],
    pub age: f32,
    pub life: f32,
}

/// Micro-point-light flash emitted at bullet impact (`spark.tbn2` enhancement).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparkFlashLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub near_mm: f32,
    pub far_mm: f32,
}

/// Bullet impact spark system (`ef_sparks.c`).
#[derive(Debug, Clone, Default)]
pub struct ImpactSparkSystem {
    pub sparks: Vec<ImpactSpark>,
    pub flashes: Vec<(Vec3, f32, f32)>,
}

impl ImpactSparkSystem {
    /// Spawns 2 spark quads at `origin` (`MoveSparks` `0x00232c00`), plus a micro-point-light flash
    /// only when `native_upgrade` is enabled (strict Vanilla PS2 mode emits zero dynamic spark lights).
    pub fn spawn_impact(&mut self, origin: Vec3, native_upgrade: bool, rng: &mut FxRng) {
        for _ in 0..2 {
            let kind_pos = rng.next_f32() > 0.5;
            let sign = if kind_pos { 1.0 } else { -1.0 };
            let start_pos = Vec3::new(
                sign * rng.next_f32() * 150.0,
                rng.next_f32() * -375.0,
                rng.sway(-50.0, 50.0),
            );
            let speed = Vec3::new(
                sign * (rng.next_f32() * 100.0 + 50.0),
                rng.next_f32() * -150.0 - 625.0,
                rng.sway(-100.0, 100.0),
            );
            let bright = rng.next_f32() * 0.7 + 0.3;
            let base_rgba = [bright * 128.0, bright * 120.0, bright * 120.0, 98.0];
            self.sparks.push(ImpactSpark {
                origin,
                start_pos,
                speed,
                pos: origin + start_pos,
                size_w: rng.next_f32() * 5.0 + 20.0,
                size_h: rng.next_f32() * 5.0 + 20.0,
                base_rgba,
                rgba: base_rgba,
                age: 0.0,
                life: 0.28,
            });
        }
        if native_upgrade {
            self.flashes.push((origin, 0.0, 0.14));
        }
    }

    pub fn step(&mut self, dt: f32) {
        for s in &mut self.sparks {
            s.age += dt;
            let t = (s.age / s.life.max(1e-4)).clamp(0.0, 1.0);
            let sin_arc = (t * PI).sin();
            let rgb_s = ((t + 1.0) * 0.5 * PI).sin();
            s.pos = s.origin
                + s.start_pos
                + Vec3::new(s.speed.x * sin_arc, s.speed.y * t, s.speed.z * sin_arc);
            s.rgba = [
                s.base_rgba[0] * rgb_s,
                s.base_rgba[1] * rgb_s,
                s.base_rgba[2] * rgb_s,
                s.base_rgba[3] * sin_arc,
            ];
        }
        self.sparks.retain(|s| s.age < s.life);
        for (_, age, _) in &mut self.flashes {
            *age += dt;
        }
        self.flashes.retain(|(_, age, max_age)| *age < *max_age);
    }

    /// Writes active micro-point-light flashes for metallic bullet impacts into caller-owned
    /// `out` when `native_upgrade` is enabled; clears `out` in strict 1:1 Vanilla PS2 mode.
    pub fn active_flash_lights_into(&self, native_upgrade: bool, out: &mut Vec<SparkFlashLight>) {
        out.clear();
        if !native_upgrade {
            return;
        }
        for (pos, age, max_age) in &self.flashes {
            let k = (1.0 - age / max_age).clamp(0.0, 1.0);
            out.push(SparkFlashLight {
                position: [pos.x, pos.y, pos.z],
                color: [1.0 * k, 0.78 * k, 0.35 * k],
                near_mm: 200.0,
                far_mm: 1800.0,
            });
        }
    }

    /// Returns active micro-point-light flashes for metallic bullet impacts when `native_upgrade`
    /// is enabled; returns empty in strict 1:1 Vanilla PS2 mode.
    #[must_use]
    pub fn active_flash_lights(&self, native_upgrade: bool) -> Vec<SparkFlashLight> {
        if !native_upgrade || self.flashes.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(self.flashes.len());
        self.active_flash_lights_into(native_upgrade, &mut out);
        out
    }
}

/// Gravity for shattering glass shards (`0x45192000` = `2450.0` mm/s^2, `hh_class_glass_piece_00.c` `0x00258a10`).
pub const GLASS_GRAVITY_MM_S2: f32 = 2450.0;
/// Specular cosine threshold (`cos_beta_min$102` @ `0x0036ed00`).
pub const GLASS_SPECULAR_COS_MIN: f32 = 0.996_194;
/// UV step (`0.0625` = `1/16`, `ef_broken_glass.c` `0x0013af40`).
pub const GLASS_UV_STEP: f32 = 0.0625;
/// Max shards per `hh_class_glass_break_phenomenon_00.c` burst (`0x32` = 50).
pub const GLASS_MAX_SHARDS_PER_BURST: usize = 50;
/// Maximum simultaneous glass shards retained in [`GlassBreakSystem::shards`] (`SH2-176`).
///
/// Equals `4 × GLASS_MAX_SHARDS_PER_BURST` (`200`), capping 4 concurrent shatter bursts well within
/// the 600-instance global Effect2 pool at `Effect_Object_Initialize` (`0x0026a130`; note that in
/// `SLUS_202.28` `0x00370910`, the 200-slot `0xc8` entry belongs to `class 0x0a`
/// `HH_Class_Blood_00`, while `class 0x14`/`0x15` `Glass_Break_Phenomenon_00`/`GlassPiece_00` have
/// `mode = 0, count = 1` in the prototype table). When full, the oldest shard is evicted FIFO
/// matching `Instance_OldestEntry_Search` (`0x00259c00`).
pub const MAX_GLASS_SHARDS: usize = 200;
/// Maximum simultaneous active burst emitters retained in [`GlassBreakSystem::bursts`] (`SH2-176`).
pub const MAX_GLASS_BURSTS: usize = 16;

/// Single tumbling glass shard (`hh_class_glass_piece_00.c` `0x00258a10..0x002592e0`).
#[derive(Debug, Clone, Copy)]
pub struct GlassShard {
    pub origin: Vec3,
    pub pos: Vec3,
    pub vel: Vec3,
    pub spin_x: f32,
    pub spin_y: f32,
    pub age: f32,
    pub uv_rect: [f32; 4],
    pub settled: bool,
}

/// Burst emitter (`hh_class_glass_break_phenomenon_00.c` `0x002581f0`).
#[derive(Debug, Clone)]
pub struct GlassBreakPhenomenon {
    pub origin: Vec3,
    pub base_vel: Vec3,
    pub elapsed: f32,
    pub next_spawn: f32,
    pub spawned: usize,
}

/// Glass shatter simulation (`ef_broken_glass.c` + `hh_class_glass_piece_00.c`).
#[derive(Debug, Clone, Default)]
pub struct GlassBreakSystem {
    pub bursts: Vec<GlassBreakPhenomenon>,
    pub shards: Vec<GlassShard>,
}

impl GlassBreakSystem {
    /// Clears all active burst emitters and settled/airborne glass shards on room transition
    /// (`HH_Effect_Object_DesignateClassInstance_Clear` `0x0026a220` / `MemoryPool_AllClear` `0x00269ff0`, `SH2-176`).
    pub fn clear(&mut self) {
        self.bursts.clear();
        self.shards.clear();
    }

    /// Triggers a glass-shatter burst at `origin` with initial velocity `base_vel`,
    /// evicting the oldest burst emitter if [`MAX_GLASS_BURSTS`] is reached (`SH2-176`).
    pub fn trigger_break(&mut self, origin: Vec3, base_vel: Vec3) {
        if self.bursts.len() >= MAX_GLASS_BURSTS {
            self.bursts.remove(0);
        }
        self.bursts.push(GlassBreakPhenomenon {
            origin,
            base_vel,
            elapsed: 0.0,
            next_spawn: 0.0,
            spawned: 0,
        });
    }

    pub fn step(&mut self, dt: f32, floor_y_mm: f32, rng: &mut FxRng) {
        for b in &mut self.bursts {
            while b.elapsed > b.next_spawn && b.spawned < GLASS_MAX_SHARDS_PER_BURST {
                let col = (rng.next_u32() % 3) as f32;
                let row = (rng.next_u32() % 3) as f32;
                let vel = b.base_vel
                    + Vec3::new(
                        rng.sway(-200.0, 200.0),
                        rng.sway(-300.0, 100.0),
                        rng.sway(-200.0, 200.0),
                    );
                if self.shards.len() >= MAX_GLASS_SHARDS {
                    self.shards.remove(0);
                }
                self.shards.push(GlassShard {
                    origin: b.origin,
                    pos: b.origin,
                    vel,
                    spin_x: (rng.next_f32() * 360.0 * 0.017_453_292 - PI) * 15.0,
                    spin_y: (rng.next_f32() * 360.0 * 0.017_453_292 - PI) * 15.0,
                    age: 0.0,
                    uv_rect: [
                        col * GLASS_UV_STEP,
                        row * GLASS_UV_STEP,
                        (col + 1.0) * GLASS_UV_STEP,
                        (row + 1.0) * GLASS_UV_STEP,
                    ],
                    settled: false,
                });
                b.spawned += 1;
                b.next_spawn += 0.006;
            }
            b.elapsed += dt;
        }
        self.bursts.retain(|b| b.elapsed <= 0.3);

        for s in &mut self.shards {
            if s.settled {
                continue;
            }
            s.age += dt;
            let t = s.age;
            let next_pos = s.origin
                + Vec3::new(
                    s.vel.x * t,
                    s.vel.y * t + GLASS_GRAVITY_MM_S2 * t * t,
                    s.vel.z * t,
                );
            if next_pos.y >= floor_y_mm {
                s.pos = Vec3::new(next_pos.x, floor_y_mm, next_pos.z);
                s.settled = true;
            } else {
                s.pos = next_pos;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_casing_bounces_on_floor_with_0_3_restitution() {
        let mut rng = FxRng::new(9);
        let mut sys = ShellCasingSystem::default();
        sys.spawn_shell(
            Vec3::new(0.0, -1000.0, 0.0),
            ShellWeaponKind::Handgun,
            &mut rng,
        );
        sys.shells[0].vel = Vec3::new(500.0, 1000.0, 0.0);
        sys.shells[0].pos = Vec3::new(0.0, -5.0, 0.0);
        sys.step(0.02, 0.0);
        assert!(sys.shells[0].vel.y < 0.0);
        assert!((sys.shells[0].vel.x - 150.0).abs() < 1e-3);
    }

    #[test]
    fn glass_break_spawns_up_to_50_shards_with_parabolic_gravity() {
        let mut rng = FxRng::new(13);
        let mut glass = GlassBreakSystem::default();
        glass.trigger_break(Vec3::new(0.0, -1500.0, 0.0), Vec3::new(0.0, 0.0, 400.0));
        for _ in 0..15 {
            glass.step(1.0 / 30.0, 0.0, &mut rng);
        }
        assert_eq!(glass.shards.len(), GLASS_MAX_SHARDS_PER_BURST);
        assert!(glass.shards.iter().any(|s| s.pos.y > -1500.0));
    }

    #[test]
    fn glass_break_enforces_pool_cap_and_clear_resets_state() {
        let mut rng = FxRng::new(99);
        let mut glass = GlassBreakSystem::default();
        // Trigger 6 bursts (300 shards total > MAX_GLASS_SHARDS = 200)
        for i in 0..6 {
            glass.trigger_break(Vec3::new(i as f32 * 100.0, -1500.0, 0.0), Vec3::ZERO);
        }
        for _ in 0..20 {
            glass.step(1.0 / 30.0, 0.0, &mut rng);
            assert!(glass.shards.len() <= MAX_GLASS_SHARDS);
        }
        assert_eq!(glass.shards.len(), MAX_GLASS_SHARDS);

        // Trigger beyond MAX_GLASS_BURSTS and confirm bursts stay bounded
        for _ in 0..(MAX_GLASS_BURSTS + 5) {
            glass.trigger_break(Vec3::ZERO, Vec3::ZERO);
        }
        assert_eq!(glass.bursts.len(), MAX_GLASS_BURSTS);

        glass.clear();
        assert!(glass.shards.is_empty());
        assert!(glass.bursts.is_empty());
    }

    #[test]
    fn spark_flash_lights_gate_on_native_upgrade() {
        let mut rng = FxRng::new(21);
        let mut vanilla = ImpactSparkSystem::default();
        vanilla.spawn_impact(Vec3::ZERO, false, &mut rng);
        assert!(vanilla.active_flash_lights(false).is_empty());
        assert!(vanilla.active_flash_lights(true).is_empty());

        let mut native = ImpactSparkSystem::default();
        native.spawn_impact(Vec3::ZERO, true, &mut rng);
        assert!(native.active_flash_lights(false).is_empty());
        assert_eq!(native.active_flash_lights(true).len(), 1);
    }
}
