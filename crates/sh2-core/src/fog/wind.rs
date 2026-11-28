//! 1:1 PS2/PC fog wind simulation (`fogInitWind` `0x00169330`, `fogChangeWind` `0x00169380`,
//! `sh2pc.exe` `FUN_00487c40` `0x00487c40`) and deterministic PS2 VU0 PRNG (`SH2_common/sh_vu0.c`).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, PI};

/// Maximum horizontal wind direction turn angle per `fogChangeWind` (`0x00169380`, `Fog/fog.c:547,552`):
/// `60° = π / 3 = 1.0471976 rad` (`0x3f860a92`, bit-identical to `std::f32::consts::FRAC_PI_3`).
pub const FOG_WIND_MAX_TURN_RAD: f32 = FRAC_PI_3;

/// Minimum wind-change timer duration in 60 Hz frames (`Fog/fog.c:568`, `300.0` frames = 5 s).
pub const FOG_WIND_TIMER_MIN_FRAMES: i32 = 300;

/// Maximum wind-change timer duration in 60 Hz frames (`Fog/fog.c:568`, `900.0` frames = 15 s).
pub const FOG_WIND_TIMER_MAX_FRAMES: i32 = 900;

/// Fallback wind-change timer duration in 60 Hz frames when out of `[300, 900]` (`Fog/fog.c:569`).
pub const FOG_WIND_TIMER_FALLBACK_FRAMES: i32 = 600;

/// `shSway1f` amplitude (`0xc3960000 = -300.0f`) for wind timer jitter (`Fog/fog.c:566`).
pub const FOG_WIND_TIMER_SWAY_AMP: f32 = -300.0;

/// `shSway1f` bias (`0x3dcccccd = 0.1f`) for wind timer jitter (`Fog/fog.c:566`).
pub const FOG_WIND_TIMER_SWAY_BIAS: f32 = 0.1;

/// Default wind acceleration magnitude (`0x3ecccccd = 0.4f` mm/frame²) for `wind_type = 0` (`Fog/fog.c:385`).
pub const FOG_WIND_DEFAULT_ACCEL: f32 = 0.4;

/// Default random wind jitter magnitude (`0x3dcccccd = 0.1f` mm/frame²) (`Fog/fog.c:386`).
pub const FOG_WIND_DEFAULT_JITTER: f32 = 0.1;

/// Velocity damping factor per 60 Hz frame in `fog_part_grid` (`sh2pc.exe` `FUN_00486260`
/// `0x00486260`, `sh2pc_00480000.c:3958-3961`: `vel *= 0.9`).
pub const FOG_PARTICLE_VELOCITY_DAMPING: f32 = 0.9;

/// Exact PS2/PC LCG multiplier (`0x41c64e6d = 1103515245`, `shRandI` `SLUS_202.28` `0x0016ef30`,
/// `sh2pc.exe` `FUN_00448690` `0x00448690`).
const PS2_LCG_MUL: u32 = 0x41C6_4E6D;

/// Exact PS2/PC LCG increment (`0x3039 = 12345`, `shRandI` `0x0016ef30`).
const PS2_LCG_ADD: u32 = 0x0000_3039;

/// Exact PS2/PC LCG mask (`0x7fffffff`, `shRandI` `0x0016ef30`).
const PS2_LCG_MASK: u32 = 0x7FFF_FFFF;

/// `1.0 / 2^31` (`4.656613e-10`, `sh2pc.exe` `FUN_00448690` `0x00448690`).
const PS2_RAND_INV_2_31: f32 = 4.656_613e-10;

/// `2.0 / 2^31` (`9.313226e-10`, `sh2pc.exe` `FUN_004486d0` `0x004486d0`).
const PS2_RAND_INV_2_30: f32 = 9.313_226e-10;

/// Deterministic 1:1 port of Team Silent's `shRandI` / `shRandF` / `shRandV_Scale` / `shSway1f`
/// (`SLUS_202.28` `SH2_common/sh_vu0.c` `0x0016ef30..0x0016f0a0`, `sh2pc.exe` `0x00448690..0x00448770`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ps2FogRng {
    seed: u32,
}

impl Default for Ps2FogRng {
    fn default() -> Self {
        Self::new(0x0000_0001)
    }
}

impl Ps2FogRng {
    /// Creates a new PS2 fog PRNG with the given initial `seed`.
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            seed: seed & PS2_LCG_MASK,
        }
    }

    /// Steps the LCG once (`shRandI` `0x0016ef30`) and returns the raw 31-bit value.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(PS2_LCG_MUL)
            .wrapping_add(PS2_LCG_ADD)
            & PS2_LCG_MASK;
        self.seed
    }

    /// Returns a uniform float in `[0.0, 1.0)` (`shRandF` `0x0016ef60`, `FUN_00448690` `0x00448690`).
    #[inline]
    pub fn rand_f(&mut self) -> f32 {
        (self.next_u32() as f32) * PS2_RAND_INV_2_31
    }

    /// Returns a 3D random vector with each component in `[-scale, +scale)`
    /// (`shRandV_Scale` `0x0016efd0`, `FUN_004486d0` `0x004486d0`).
    #[inline]
    pub fn rand_v_scale(&mut self, scale: f32) -> [f32; 3] {
        let u0 = self.next_u32();
        let u1 = self.next_u32();
        let u2 = self.next_u32();
        [
            ((u0 as f32) * PS2_RAND_INV_2_30 - 1.0) * scale,
            ((u1 as f32) * PS2_RAND_INV_2_30 - 1.0) * scale,
            ((u2 as f32) * PS2_RAND_INV_2_30 - 1.0) * scale,
        ]
    }

    /// Exact port of `shSway1f` (`SLUS_202.28` `0x0016f080`, `sh2pc.exe` `FUN_00448770` `0x00448770`).
    ///
    /// When `amplitude < 0.0`, randomly flips sign with 50% probability (`(raw & 0x1000) != 0`).
    #[inline]
    pub fn sway_1f(&mut self, mut amplitude: f32, bias: f32) -> f32 {
        if amplitude < 0.0 {
            let raw = self
                .seed
                .wrapping_mul(PS2_LCG_MUL)
                .wrapping_add(PS2_LCG_ADD);
            self.seed = raw & PS2_LCG_MASK;
            if (raw & 0x1000) != 0 {
                amplitude = -amplitude;
            }
        }
        let r = self.rand_f();
        (((bias + 1.0) * bias) / (r + bias) - bias) * amplitude
    }
}

/// Wraps an angle in radians to `[-π, +π]` (`shAngleRegulate` `SLUS_202.28` `0x0016e970`,
/// `sh2pc.exe` `FUN_00448280` `0x00448280`).
#[must_use]
#[inline]
pub fn angle_regulate(angle: f32) -> f32 {
    let scaled = angle * (1.0 / std::f32::consts::TAU);
    let mut frac = scaled - scaled.floor();
    if frac > 0.5 {
        frac -= 1.0;
    }
    frac * std::f32::consts::TAU
}

/// Normalizes a 3D vector matching `FUN_00487bc0` (`0x00487bc0`, `sh2pc_00480000.c:4678-4693`).
#[inline]
fn normalize_vec3_safe(v: [f32; 3]) -> [f32; 3] {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len_sq <= 1e-12 {
        return [0.0, 0.0, 0.0];
    }
    let inv = 1.0 / len_sq.sqrt();
    if !inv.is_finite() {
        return [0.0, 0.0, 0.0];
    }
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

/// Live 1:1 PS2/PC fog wind controller (`fogInitWind` `0x00169330`, `fogChangeWind` `0x00169380`,
/// `fogSetEnvironment` timer countdown `0x00168dd0`, `Fog/fog.c:226-231, 346-572`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogWindState {
    /// Active `.fcl` `FogEnv.wind_type` (`0..=0x12`, `_DAT_00581530` / `DAT_01f7a6a0`).
    pub wind_type: u8,
    /// Current per-frame wind acceleration vector in PS2 mm/frame² (`_DAT_005814b0` / `_DAT_01f7a620`).
    pub wind_vec: [f32; 3],
    /// Remaining 60 Hz frames until next `fogChangeWind` call (`_DAT_00581548` / `DAT_01f7a6b8`).
    pub timer_frames: f32,
}

impl Default for FogWindState {
    fn default() -> Self {
        Self {
            wind_type: 0,
            wind_vec: [0.0, 0.0, 0.0],
            timer_frames: 0.0,
        }
    }
}

impl FogWindState {
    /// Initializes wind from zero (`fogInitWind` `SLUS_202.28` `0x00169330`, `Fog/fog.c:346-361`)
    /// and immediately selects an initial wind vector for `wind_type`.
    pub fn init_wind(&mut self, wind_type: u8, rng: &mut Ps2FogRng) {
        self.wind_type = wind_type;
        self.wind_vec = [0.0, 0.0, 0.0];
        self.timer_frames = 0.0;
        self.change_wind(wind_type, rng);
    }

    /// Advances the wind timer by `dt_scale` (in 60 Hz frames, `fogSetEnvironment` `0x00168dd0`
    /// `Fog/fog.c:227-231`). Triggers [`Self::change_wind`] when `timer_frames <= 0.0` and returns
    /// `true` if the wind changed this tick.
    pub fn tick(&mut self, dt_scale: f32, rng: &mut Ps2FogRng) -> bool {
        self.timer_frames -= dt_scale.max(0.0);
        if self.timer_frames < 1.0 {
            let wt = self.wind_type;
            self.change_wind(wt, rng);
            true
        } else {
            false
        }
    }

    /// Exact 1:1 port of `fogChangeWind(wind_type)` (`SLUS_202.28` `0x00169380`, `Fog/fog.c:368-572`;
    /// cross-checked against `sh2pc.exe` `FUN_00487c40` `0x00487c40`, `sh2pc_00480000.c:4700-4900`).
    ///
    /// Note on PS2 vs PC drift: for `wind_type = 0x0e`, `SLUS_202.28` uses `speed = 1.5` (`0x3fc00000`)
    /// and `z = rand * 0.5 + 0.8`, whereas `sh2pc.exe` drifted to `speed = 1.0` and `z = -(rand * 0.5) - 0.8`.
    /// Primary PS2 `SLUS_202.28` ground truth is used here.
    pub fn change_wind(&mut self, wind_type: u8, rng: &mut Ps2FogRng) {
        self.wind_type = wind_type;
        let mut speed = FOG_WIND_DEFAULT_ACCEL; // 0x3ecccccd = 0.4
        let mut jitter_mag = FOG_WIND_DEFAULT_JITTER; // 0x3dcccccd = 0.1

        let raw_dir: [f32; 3] = match wind_type {
            1 | 3 => {
                let x = rng.rand_f() - 0.5;
                let z = rng.rand_f() - 0.5;
                [x, 0.0, z]
            }
            2 => {
                jitter_mag = 0.0;
                [0.0, 0.0, 0.0]
            }
            4 => {
                let x = (rng.rand_f() - 0.5) * 0.1;
                speed = 0.5; // 0x3f000000
                jitter_mag = 0.0;
                [x, 0.0, 1.0]
            }
            5 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 + PI;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            6 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            7 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 - FRAC_PI_2;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            8 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 + FRAC_PI_2;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            9 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 + 3.0 * FRAC_PI_4;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            10 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 + FRAC_PI_4;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            11 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 - FRAC_PI_4;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            12 => {
                let ang = (rng.rand_f() - 0.5) * FRAC_PI_4 - 3.0 * FRAC_PI_4;
                let y = rng.rand_f() * 0.1;
                [ang.sin(), y, ang.cos()]
            }
            13 => {
                // `0x3dcccccd3f000000` -> x = 0.5, y = 0.1, z = 0.0 (`Fog/fog.c:467`, `sh2pc_00480000.c:4776-4779`)
                speed = 0.2; // 0x3e4ccccd
                [0.5, 0.1, 0.0]
            }
            14 => {
                let x = -(rng.rand_f() * 0.2) - 0.2;
                let y = rng.rand_f() * 0.2 + 0.1;
                let z = rng.rand_f() * 0.5 + 0.8;
                speed = 1.5; // 0x3fc00000
                [x, y, z]
            }
            15 => {
                // `0.08726646` = 5° (`PI / 36`), `2.6179938` = 150° (`5 * PI / 6`), `y = -0.1` (`0xbdcccccd`)
                let ang = (rng.rand_f() - 0.5) * 0.087_266_46 + 2.617_993_8;
                speed = 0.3; // 0x3e99999a
                jitter_mag = 0.0;
                [ang.sin(), -0.1, ang.cos()]
            }
            16 => {
                let x = rng.rand_f() - 0.5;
                let z = rng.rand_f() - 0.5;
                speed = 0.1; // 0x3dcccccd
                jitter_mag = 0.0;
                [x, 0.0, z]
            }
            17 => {
                let x = rng.rand_f() - 0.5;
                let z = rng.rand_f() - 0.5;
                speed = 0.2; // 0x3e4ccccd
                jitter_mag = 0.0;
                [x, 0.0, z]
            }
            18 => {
                // `obGB.fcl` `wind_type = 0x12`: valley breeze (`Fog/fog.c:504-512`)
                let x = rng.rand_f() + 1.0;
                let y = rng.rand_f() * 0.1;
                let z = rng.rand_f() * 0.1 + 0.1;
                speed = 0.7; // 0x3f333333
                [x, y, z]
            }
            _ => {
                // Default (`wind_type = 0`): omni horizontal with gentle vertical drift (`Fog/fog.c:388-395`)
                let x = rng.rand_f() - 0.5;
                let y = rng.rand_f() * 0.1;
                let z = rng.rand_f() - 0.5;
                [x, y, z]
            }
        };

        let unit_dir = normalize_vec3_safe(raw_dir);
        let mut new_wind = [
            unit_dir[0] * speed,
            unit_dir[1] * speed,
            unit_dir[2] * speed,
        ];

        // Random directional perturbation: `shRandV_Scale(0.5)` normalized and scaled by `jitter_mag`
        let raw_jitter = rng.rand_v_scale(0.5);
        let unit_jitter = normalize_vec3_safe(raw_jitter);
        new_wind[0] += unit_jitter[0] * jitter_mag;
        new_wind[1] += unit_jitter[1] * jitter_mag;
        new_wind[2] += unit_jitter[2] * jitter_mag;

        // Clamp horizontal turn angle to ±60° (`1.0471976 rad`) relative to previous non-zero wind
        // (`Fog/fog.c:543-561`, `sh2pc_00480000.c:4856-4879`).
        if self.wind_vec[0] != 0.0 || self.wind_vec[1] != 0.0 || self.wind_vec[2] != 0.0 {
            let prev_ang = self.wind_vec[0].atan2(self.wind_vec[2]);
            let new_ang = new_wind[0].atan2(new_wind[2]);
            let diff = angle_regulate(prev_ang - new_ang);
            if diff.abs() > FOG_WIND_MAX_TURN_RAD {
                let sign = if diff > 0.0 {
                    1.0
                } else if diff < 0.0 {
                    -1.0
                } else {
                    0.0
                };
                let horiz_len = (new_wind[0] * new_wind[0] + new_wind[2] * new_wind[2]).sqrt();
                let clamped_ang = prev_ang + sign * FOG_WIND_MAX_TURN_RAD;
                new_wind[0] = clamped_ang.sin() * horiz_len;
                new_wind[2] = clamped_ang.cos() * horiz_len;
            }
        }

        self.wind_vec = new_wind;

        // Update countdown timer (`Fog/fog.c:566-570`): `timer += shSway1f(-300.0, 0.1) as i32`
        let delta = rng.sway_1f(FOG_WIND_TIMER_SWAY_AMP, FOG_WIND_TIMER_SWAY_BIAS) as i32;
        let next_timer = (self.timer_frames as i32) + delta;
        self.timer_frames =
            if !(FOG_WIND_TIMER_MIN_FRAMES..=FOG_WIND_TIMER_MAX_FRAMES).contains(&next_timer) {
                FOG_WIND_TIMER_FALLBACK_FRAMES as f32
            } else {
                next_timer as f32
            };
    }

    /// Computes the exact steady-state PS2 particle terminal speed in mm/s for a given wind
    /// acceleration magnitude `accel_mm_per_frame2` under `vel = (vel + accel) * 0.9` at 60 Hz:
    /// `v_term = (0.9 / (1.0 - 0.9)) * accel * 60.0 = 540.0 * accel` mm/s.
    #[must_use]
    #[inline]
    pub fn terminal_speed_mm_per_sec(accel_mm_per_frame2: f32) -> f32 {
        (FOG_PARTICLE_VELOCITY_DAMPING / (1.0 - FOG_PARTICLE_VELOCITY_DAMPING))
            * accel_mm_per_frame2
            * 60.0
    }
}
