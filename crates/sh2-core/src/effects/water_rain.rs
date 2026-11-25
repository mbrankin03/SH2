//! Flooded Corridors, Rain & Footstep Water Ripples/Splashes (`SH2-100`).
//!
//! Ported 1:1 from:
//! - `Effect2/hh_class_water_pool_00.c` (`SLUS_202.28` `0x00282ec0..0x00283660`)
//! - `Effect2/hh_class_water_splash_phenomenon_00.c` (`SLUS_202.28` `0x00268ad0..0x00268b80`)
//! - `Effect2/hh_class_water_common.c` (`SLUS_202.28` `0x00268020..0x00268ab0`)
//! - `Effect2/hh_class_water_00.c` (`SLUS_202.28` `0x0025ba70..0x0025bb80`)
//! - `Effect/ef_rain.c` (`SLUS_202.28` `0x0022c4b0..0x0022cce0`)

use glam::Vec3;
use std::f32::consts::{PI, TAU};

use super::FxRng;

/// `hh_class_water_pool_00.c` `_motion_info` (`0x00379e70`): `(duration_s, scale_weight, rgba_weight)`.
pub const WATER_POOL_MOTION_INFO: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [0.4, 0.5, 0.3],
    [0.8, 0.5, 0.5],
    [1.2, 0.5, 0.2],
];

/// `_scale_end_list` (`0x00379e30`) in PS2 mm for 3 ripple size variants.
pub const WATER_POOL_SCALE_END: [[f32; 3]; 3] = [
    [200.0, 1.0, 200.0],
    [250.0, 1.0, 250.0],
    [275.0, 1.0, 275.0],
];

/// `_rgba_start_list` (`0x00379de0`) & `_rgba_end_list` (`0x00379df0`).
pub const WATER_POOL_RGBA_START: [f32; 4] = [255.0, 255.0, 255.0, 255.0];
pub const WATER_POOL_RGBA_END: [f32; 4] = [188.0, 188.0, 218.0, 50.0];

/// 4x4 UV grid (`0.25` step, `0.24804688` max) from `_square_stq_list` (`0x00379da0`) for `water00..40.tbn2`.
pub const WATER_POOL_UV_GRID: [[f32; 4]; 16] = {
    let mut out = [[0.0f32; 4]; 16];
    let mut i = 0;
    while i < 16 {
        let col = (i % 4) as f32;
        let row = (i / 4) as f32;
        out[i] = [
            col * 0.25,
            row * 0.25,
            col * 0.25 + 0.24804688,
            row * 0.25 + 0.24804688,
        ];
        i += 1;
    }
    out
};

/// Single expanding concentric water ripple (`hh_class_water_pool_00.c` `0x00282ec0`).
#[derive(Debug, Clone, Copy)]
pub struct WaterPoolRipple {
    pub pos: Vec3,
    pub quadrant: u8,
    pub size_variant: u8,
    pub stage: u8,
    pub age: f32,
    pub yaw_rad: f32,
    pub scale_mm: [f32; 3],
    pub rgba: [f32; 4],
    pub active: bool,
}

impl WaterPoolRipple {
    /// Spawns a new ripple (`Object_Initialize` `0x00282ec0`).
    #[must_use]
    pub fn new(pos: Vec3, rng: &mut FxRng) -> Self {
        let quadrant = (rng.next_u32() & 0xf) as u8;
        let size_variant = (rng.next_u32() % 3) as u8;
        let yaw_rad = ((rng.next_u32() % 360) as f32 * 0.017_453_292) - PI;
        Self {
            pos,
            quadrant,
            size_variant,
            stage: 0,
            age: 0.0,
            yaw_rad,
            scale_mm: [0.0, 0.0, 0.0],
            rgba: WATER_POOL_RGBA_START,
            active: true,
        }
    }

    /// Evaluates `Object_Motion_00` (`0x00283020`).
    pub fn step(&mut self, dt: f32) {
        if !self.active {
            return;
        }
        let clamped_stage = (self.stage as usize).min(2);
        let mut t_accum = 0.0f32;
        let mut s_accum = 0.0f32;
        let mut c_accum = 0.0f32;
        for row in WATER_POOL_MOTION_INFO.iter().take(clamped_stage + 1) {
            t_accum += row[0];
            s_accum += row[1];
            c_accum += row[2];
        }
        let next = WATER_POOL_MOTION_INFO[clamped_stage + 1];
        let t_end = t_accum + next[0];
        if self.age > t_end {
            self.stage += 1;
            if self.stage > 2 {
                self.active = false;
                return;
            }
        }
        let frac = ((self.age - t_accum) / (t_end - t_accum).max(1e-4)).clamp(0.0, 1.0);
        let s_t = (s_accum + frac * next[1]).clamp(0.0, 1.0);
        let c_t = (c_accum + frac * next[2]).clamp(0.0, 1.0);
        let end_scale = WATER_POOL_SCALE_END[self.size_variant as usize % 3];
        for i in 0..3 {
            self.scale_mm[i] = end_scale[i] * s_t;
        }
        for i in 0..4 {
            self.rgba[i] = WATER_POOL_RGBA_START[i]
                + (WATER_POOL_RGBA_END[i] - WATER_POOL_RGBA_START[i]) * c_t;
        }
        self.age += dt;
    }
}

/// Manages active footstep/rain water pool ripples (`hh_class_water_pool_00.c`).
#[derive(Debug, Clone, Default)]
pub struct WaterPoolSystem {
    pub ripples: Vec<WaterPoolRipple>,
}

impl WaterPoolSystem {
    pub fn spawn_ripple(&mut self, pos: Vec3, rng: &mut FxRng) {
        self.ripples.push(WaterPoolRipple::new(pos, rng));
    }

    pub fn step(&mut self, dt: f32) {
        for r in &mut self.ripples {
            r.step(dt);
        }
        self.ripples.retain(|r| r.active);
    }
}

/// Droplet from `hh_class_water_splash_phenomenon_00.c` (`0x00268b80`).
#[derive(Debug, Clone, Copy)]
pub struct WaterSplashDroplet {
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub life: f32,
}

/// 6-droplet radial footstep splash (`hh_class_water_splash_phenomenon_00.c` `0x00268b80`).
#[derive(Debug, Clone, Default)]
pub struct WaterSplashSystem {
    pub droplets: Vec<WaterSplashDroplet>,
}

impl WaterSplashSystem {
    /// Spawns 6 radial droplets (`uVar4 < 6`, `0x00268b80`).
    pub fn spawn_footstep_splash(&mut self, pos: Vec3, impact_vel: Vec3, rng: &mut FxRng) {
        let speed = impact_vel.length().max(250.0);
        let base_pitch = (impact_vel.y / speed).asin().abs() * 0.5;
        for _ in 0..6 {
            let yaw = ((rng.next_u32() % 360) as f32 * 0.017_453_292) - PI;
            let radial_r = (rng.next_u32() % 30) as f32 + 10.0;
            let pitch = base_pitch + ((rng.next_u32() % 10) as f32 * 0.017_453_292 - 0.087_266_46);
            let dir = Vec3::new(
                yaw.sin() * pitch.cos(),
                -pitch.sin().abs(),
                yaw.cos() * pitch.cos(),
            );
            self.droplets.push(WaterSplashDroplet {
                pos: pos + Vec3::new(yaw.sin() * radial_r, 0.0, yaw.cos() * radial_r),
                vel: dir * speed * 0.5,
                age: 0.0,
                life: 0.45,
            });
        }
    }

    pub fn step(&mut self, dt: f32) {
        for d in &mut self.droplets {
            d.pos += d.vel * dt;
            d.vel.y += 2450.0 * dt;
            d.age += dt;
        }
        self.droplets.retain(|d| d.age < d.life);
    }
}

/// Wave element for `hh_class_water_common.c` (`Wave_Calculator` `0x00268020`).
#[derive(Debug, Clone, Copy)]
pub struct WaveElement {
    pub center_x: usize,
    pub center_z: usize,
    pub radius_mm: f32,
    pub age: f32,
    pub max_age: f32,
    pub amplitude_mm: f32,
    pub freq_rad: f32,
    pub wavelength_mm: f32,
    pub speed_mm_s: f32,
    pub phase: f32,
}

impl WaveElement {
    /// Evaluates `Wave_Calculator` (`0x00268020`) at distance `dist_mm` and attenuation `atten`.
    #[must_use]
    pub fn eval_height(&self, dist_mm: f32, atten: f32) -> f32 {
        let mut theta = self.freq_rad * (self.phase + dist_mm / self.wavelength_mm.max(1e-3));
        theta %= TAU;
        if theta > PI {
            theta -= TAU;
        } else if theta < -PI {
            theta += TAU;
        }
        atten * self.amplitude_mm * theta.sin()
    }
}

/// 48x48 ping-pong water height grid (`hh_class_water_00.c` `Object_Motion_00` `0x0025ba70`,
/// `dev_0 = 2.0`, `dev_1 = 16.0`, `Specular_Calculator` `0x0025bb80` threshold `0.939692`).
#[derive(Debug, Clone)]
pub struct WaterGrid2D {
    pub buffers: [Vec<f32>; 2],
    pub active_idx: usize,
}

impl Default for WaterGrid2D {
    fn default() -> Self {
        Self {
            buffers: [vec![0.0; 48 * 48], vec![0.0; 48 * 48]],
            active_idx: 0,
        }
    }
}

impl WaterGrid2D {
    /// Steps `Object_Motion_00` (`0x0025ba70`): `v = (N+S+E+W)/2.0 - prev; next = v - v/16.0`.
    pub fn step(&mut self) {
        let cur = self.active_idx;
        let nxt = cur ^ 1;
        let (src, dst) = if cur == 0 {
            let (a, b) = self.buffers.split_at_mut(1);
            (&a[0], &mut b[0])
        } else {
            let (a, b) = self.buffers.split_at_mut(1);
            (&b[0], &mut a[0])
        };
        for z in 1..47 {
            for x in 1..47 {
                let idx = x + z * 48;
                let sum = src[idx - 1] + src[idx + 1] + src[idx - 48] + src[idx + 48];
                let v = sum / 2.0 - dst[idx];
                dst[idx] = v - v / 16.0;
            }
        }
        self.active_idx = nxt;
    }

    /// Evaluates `Specular_Calculator` (`0x0025bb80`, `cos_min = 0.939692`).
    #[must_use]
    pub fn eval_specular(view_dir: Vec3, light_dir: Vec3, normal: Vec3) -> f32 {
        let ndl = light_dir.dot(normal).abs();
        let refl = (normal * (ndl * 2.0) - light_dir).normalize_or_zero();
        let vdr = refl.dot(view_dir).abs();
        if vdr < 0.939_692 {
            0.0
        } else {
            1.0 - (1.0 - vdr) / 0.060_307_98
        }
    }
}

/// `ef_rain.c` `EFCTRainDropMain` (`0x0022c5b0`): `4000.0` mm/s fall speed, `100.0` mm streak.
pub const RAIN_FALL_SPEED_MM_S: f32 = 4000.0;
pub const RAIN_STREAK_LEN_MM: f32 = 100.0;

#[derive(Debug, Clone, Copy)]
pub struct RainDrop {
    pub pos: Vec3,
    pub spray_timer: u8,
    pub spraying: bool,
}

/// Rain streak & ground splash system (`Effect/ef_rain.c` `0x0022c4b0..0x0022cce0`).
#[derive(Debug, Clone, Default)]
pub struct RainDropSystem {
    pub drops: Vec<RainDrop>,
}

impl RainDropSystem {
    /// Spawns `count` raindrops around `cam_pos` (`EFCTRainDropMain` `0x0022c5b0`).
    pub fn spawn_around(&mut self, cam_pos: Vec3, count: usize, rng: &mut FxRng) {
        for _ in 0..count {
            self.drops.push(RainDrop {
                pos: Vec3::new(
                    cam_pos.x + (rng.next_f32() - 0.5) * 2500.0,
                    cam_pos.y - (rng.next_f32() - 0.5) * 4000.0 - 1000.0,
                    cam_pos.z + (rng.next_f32() - 0.5) * 2500.0,
                ),
                spray_timer: 0,
                spraying: false,
            });
        }
    }

    pub fn step(&mut self, cam_pos: Vec3, floor_y: f32, dt: f32, rng: &mut FxRng) {
        for d in &mut self.drops {
            if d.spraying {
                if d.spray_timer == 0 {
                    d.spraying = false;
                    d.pos = Vec3::new(
                        cam_pos.x + (rng.next_f32() - 0.5) * 2500.0,
                        cam_pos.y - rng.next_f32() * 3000.0 - 1000.0,
                        cam_pos.z + (rng.next_f32() - 0.5) * 2500.0,
                    );
                } else {
                    d.spray_timer -= 1;
                }
            } else {
                d.pos.y += RAIN_FALL_SPEED_MM_S * dt;
                if d.pos.y >= floor_y {
                    d.pos.y = floor_y;
                    d.spraying = true;
                    d.spray_timer = (rng.next_u32() & 1) as u8;
                } else if d.pos.y >= cam_pos.y + 2500.0 {
                    d.pos.y = cam_pos.y - 2500.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_pool_ripple_matches_motion_info_table() {
        let mut rng = FxRng::new(11);
        let mut ripple = WaterPoolRipple::new(Vec3::ZERO, &mut rng);
        ripple.size_variant = 0;
        ripple.age = 0.4;
        ripple.step(0.0);
        assert!((ripple.scale_mm[0] - 100.0).abs() < 1e-2);
        assert!((ripple.rgba[3] - (255.0 + (50.0 - 255.0) * 0.3)).abs() < 1.0);
    }

    #[test]
    fn water_grid_ping_pong_and_specular_threshold() {
        let mut grid = WaterGrid2D::default();
        grid.buffers[0][24 + 24 * 48] = 10.0;
        grid.step();
        assert!((grid.buffers[1][23 + 24 * 48] - 4.6875).abs() < 1e-4);
        assert_eq!(WaterGrid2D::eval_specular(Vec3::Y, Vec3::Y, Vec3::Y), 1.0);
        assert_eq!(WaterGrid2D::eval_specular(Vec3::X, Vec3::Y, Vec3::Y), 0.0);
    }
}
