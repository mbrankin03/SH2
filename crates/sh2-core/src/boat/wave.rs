//! Toluca Lake wave ring system (`Chacter/lake_wave.c`) and PS2 water grid simulation
//! (`Effect2/hh_class_water_00.c` & `hh_class_water_common.c`).
//!
//! Verified 1:1 against:
//! - PS2 Prototype ELF (`SLUS_202.28`):
//!   - `mizTestLakeWaveInit` (`0x0026ef30`) / `FUN_0026f018` (`0x0026f018`)
//!   - `mizTestLakeWaveMain` (`0x0026f050`)
//!   - `Object_Motion_00` (`0x0025ba70`, `Effect2/hh_class_water_00.c`)
//!   - `Specular_Calculator` (`0x0025bb80`, `Effect2/hh_class_water_00.c`)
//! - Retail PC Binary (`sh2pc.exe`):
//!   - `FUN_00559b70` (`0x00559b70`, `mizTestLakeWaveInit`)
//!   - `FUN_00559ef0` (`0x00559ef0`, `mizTestLakeWaveMain`)

use glam::Vec3;
use std::f32::consts::PI;

/// Number of wave rings in `lake_wave.c` (`0x14` = `20` at `0x0026f018` / `0x00559b70`).
pub const LAKE_WAVE_COUNT: usize = 20;

/// Maximum wave ring radius before wrapping (`240.0` at `0x0026f050` / `0x00559ef0`).
pub const LAKE_WAVE_MAX_POS: f32 = 240.0;

/// Wave ring radial expansion speed (`3.0` per second at `0x0026f050` / `0x00559ef0`).
pub const LAKE_WAVE_SPEED: f32 = 3.0;

/// Wave ring initial half-width (`3.0` at `0x0026f018` / `0x00559b70`).
pub const LAKE_WAVE_BASE_WIDTH: f32 = 3.0;

/// Wave ring width growth rate (`0.03` per second at `0x0026f018` / `0x00559b70`).
pub const LAKE_WAVE_WIDTH_GROWTH: f32 = 0.03;

/// Wave ring initial power (`0.5` at `0x0026f018` / `0x00559b70`).
pub const LAKE_WAVE_BASE_POWER: f32 = 0.5;

/// Wave ring power decay rate (`0.015` per second at `0x0026f018` / `0x00559b70`).
pub const LAKE_WAVE_POWER_DECAY: f32 = 0.015;

/// Wave ring respawn amplitude modulation (`0.25` at `0x0026f050` / `0x00559ef0`).
pub const LAKE_WAVE_RESPAWN_MOD: f32 = 0.25;

/// Initial time spacing between consecutive wave rings (`4.0` s at `0x0026f018` / `0x00559b70`).
pub const LAKE_WAVE_INIT_DT: f32 = 4.0;

/// World-to-wave-space scale factor (`1.0 / 500.0 = 0.002` at `0x0026f050` / `0x00559ef0`).
pub const LAKE_WORLD_TO_WAVE_SCALE: f32 = 0.002;

/// Retail PC wave origin X in scaled lake coordinates (`-20.0` at `0x00559ef0`).
pub const LAKE_WAVE_ORIGIN_X: f32 = -20.0;

/// Retail PC wave origin Z in scaled lake coordinates (`-120.0` at `0x00559ef0`).
pub const LAKE_WAVE_ORIGIN_Z: f32 = -120.0;

/// Grid dimension of `hh_class_water_00.c` (`0x30` = `48` vertices per axis at `0x0025ba70`).
pub const WATER_GRID_SIZE: usize = 48;

/// Cosine of the 20° specular cutoff angle in `Specular_Calculator` (`0.939692` at `0x0025bb80`).
pub const WATER_SPECULAR_COS_CUTOFF: f32 = 0.939692;

/// Normalization denominator `1.0 - 0.939692` in `Specular_Calculator` (`0.06030798` at `0x0025bb80`).
pub const WATER_SPECULAR_INV_RANGE: f32 = 0.06030798;

/// Single concentric lake wave ring (`0x14` bytes per entry in `lake_wave.c`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LakeWaveRing {
    /// Current radial distance from the lake wave origin (`wave_pos`, offset `+0x00`).
    pub pos: f32,
    /// Wave half-width (`wave_width`, offset `+0x04`).
    pub width: f32,
    /// Wave amplitude / power (`wave_power`, offset `+0x08`).
    pub power: f32,
    /// Elapsed wave timer in seconds (offset `+0x0c`).
    pub timer: f32,
    /// Next ring index in circular list (`+0x10`).
    pub next: usize,
    /// Previous ring index in circular list (`+0x11`).
    pub prev: usize,
}

/// Output of [`LakeWaveSystem::update`] when the boat intersects an active wave ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LakeWaveSample {
    /// Cosine-bell modulated wave height (`power * (1.0 + cos(|r - d| / width * PI))`).
    pub height: f32,
    /// Direction angle from wave origin to boat (`atan2(dx, dz)` in radians).
    pub angle: f32,
    /// Ring power at impact.
    pub power: f32,
}

/// Concentric Toluca Lake wave ring simulator (`Chacter/lake_wave.c` `0x0026ef30`..`0x0026f050`).
#[derive(Debug, Clone)]
pub struct LakeWaveSystem {
    /// 20-entry circular ring buffer (`0x14` entries).
    pub rings: [LakeWaveRing; LAKE_WAVE_COUNT],
    /// Index of the most recently respawned wave ring (`_newest_wave`).
    pub newest_wave: usize,
    /// Deterministic degree counter used on wave respawn (`_rand_seed`).
    pub rand_seed: i32,
}

impl Default for LakeWaveSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl LakeWaveSystem {
    /// Initializes all 20 wave rings (`mizTestLakeWaveInit` `0x0026ef30` / `0x00559b70`).
    pub fn new() -> Self {
        let mut rings = [LakeWaveRing {
            pos: 0.0,
            width: LAKE_WAVE_BASE_WIDTH,
            power: LAKE_WAVE_BASE_POWER,
            timer: 0.0,
            next: 0,
            prev: 0,
        }; LAKE_WAVE_COUNT];

        for (i, ring) in rings.iter_mut().enumerate() {
            let t = (i as f32) * LAKE_WAVE_INIT_DT;
            ring.timer = t;
            ring.pos = t * LAKE_WAVE_SPEED;
            ring.width = t * LAKE_WAVE_WIDTH_GROWTH + LAKE_WAVE_BASE_WIDTH;
            ring.power = LAKE_WAVE_BASE_POWER - t * LAKE_WAVE_POWER_DECAY;
            ring.next = (i + 1) % LAKE_WAVE_COUNT;
            ring.prev = if i == 0 { LAKE_WAVE_COUNT - 1 } else { i - 1 };
        }

        Self {
            rings,
            newest_wave: 0,
            rand_seed: 0,
        }
    }

    /// Advances all wave rings by `dt` seconds and samples the wave at `boat_world_pos` (mm).
    ///
    /// Matches `mizTestLakeWaveMain` (`0x0026f050` SLUS / `0x00559ef0` sh2pc).
    pub fn update(&mut self, boat_world_pos: Vec3, dt: f32) -> Option<LakeWaveSample> {
        for i in 0..LAKE_WAVE_COUNT {
            let ring = &mut self.rings[i];
            ring.timer += dt;
            ring.pos = ring.timer * LAKE_WAVE_SPEED;
            ring.width = ring.timer * LAKE_WAVE_WIDTH_GROWTH + LAKE_WAVE_BASE_WIDTH;
            ring.power -= dt * LAKE_WAVE_POWER_DECAY;
            if ring.pos >= LAKE_WAVE_MAX_POS {
                self.newest_wave = i;
                ring.pos = 0.0;
                ring.width = LAKE_WAVE_BASE_WIDTH;
                ring.timer = 0.0;
                let s = ((self.rand_seed as f32) * PI / 180.0).sin();
                ring.power = s * LAKE_WAVE_RESPAWN_MOD + LAKE_WAVE_BASE_POWER;
            }
        }

        // Convert boat world position (mm) to wave-space coordinates (`0x00559ef0`).
        let dx = boat_world_pos.x * LAKE_WORLD_TO_WAVE_SCALE - LAKE_WAVE_ORIGIN_X;
        let dz = boat_world_pos.z * LAKE_WORLD_TO_WAVE_SCALE - LAKE_WAVE_ORIGIN_Z;
        let dist = (dx * dx + dz * dz).sqrt();

        let start = self.newest_wave;
        let mut cur = start;
        loop {
            cur = self.rings[cur].next;
            if cur == start {
                return None;
            }
            let r = &self.rings[cur];
            if (r.pos - r.width < dist) && (dist <= r.pos + r.width) {
                let delta = (r.pos - dist).abs();
                let bell = ((delta / r.width) * PI).cos();
                let height = r.power + r.power * bell;
                let angle = dx.atan2(dz);
                self.rand_seed = self.rand_seed.wrapping_add(1);
                return Some(LakeWaveSample {
                    height,
                    angle,
                    power: r.power,
                });
            }
        }
    }
}

/// `dev_0_105` neighbor sum divisor (`2.0` = `0x40000000` at `SLUS_202.28` `0x0036ed80`, `Object_Motion_00` `0x0025ba70`).
pub const WATER_GRID_NEIGHBOR_DIVISOR: f32 = 2.0;

/// `dev_1_106` damping divisor (`16.0` = `0x41800000` at `SLUS_202.28` `0x0036ed88`, `Object_Motion_00` `0x0025ba70`).
pub const WATER_GRID_DAMPING_DIVISOR: f32 = 16.0;

/// 48x48 finite-difference heightfield water grid (`Effect2/hh_class_water_00.c` `0x0025ba70`).
#[derive(Debug, Clone)]
pub struct WaterGridSim {
    buffers: [Vec<f32>; 2],
    active: usize,
    /// Neighbor sum divisor (`dev_0_105` at `SLUS_202.28` `0x0036ed80`, `2.0`).
    pub neighbor_divisor: f32,
    /// Damping divisor (`dev_1_106` at `SLUS_202.28` `0x0036ed88`, `16.0`).
    pub damping_divisor: f32,
}

impl Default for WaterGridSim {
    fn default() -> Self {
        Self::new(WATER_GRID_NEIGHBOR_DIVISOR, WATER_GRID_DAMPING_DIVISOR)
    }
}

impl WaterGridSim {
    /// Creates a zeroed 48x48 double-buffered heightfield (`Grid_Work_Initialize` `0x0025b920`).
    pub fn new(neighbor_divisor: f32, damping_divisor: f32) -> Self {
        let len = WATER_GRID_SIZE * WATER_GRID_SIZE;
        Self {
            buffers: [vec![0.0; len], vec![0.0; len]],
            active: 0,
            neighbor_divisor,
            damping_divisor,
        }
    }

    /// Injects a localized ripple impulse at grid `(gx, gz)`.
    pub fn add_impulse(&mut self, gx: usize, gz: usize, strength: f32) {
        if gx < WATER_GRID_SIZE && gz < WATER_GRID_SIZE {
            let idx = gx + gz * WATER_GRID_SIZE;
            self.buffers[self.active][idx] += strength;
        }
    }

    /// Advances one simulation step (`Object_Motion_00` `0x0025ba70`).
    pub fn step(&mut self) {
        let src_idx = self.active;
        let dst_idx = self.active ^ 1;
        let (left, right) = self.buffers.split_at_mut(1);
        let (src, dst) = if src_idx == 0 {
            (&left[0], &mut right[0])
        } else {
            (&right[0], &mut left[0])
        };

        for z in 1..(WATER_GRID_SIZE - 1) {
            for x in 1..(WATER_GRID_SIZE - 1) {
                let i = x + z * WATER_GRID_SIZE;
                let sum = src[i - 1]
                    + src[i + 1]
                    + src[x + (z - 1) * WATER_GRID_SIZE]
                    + src[x + (z + 1) * WATER_GRID_SIZE];
                let next = sum / self.neighbor_divisor - dst[i];
                dst[i] = next - next / self.damping_divisor;
            }
        }
        self.active = dst_idx;
    }

    /// Returns the current active height buffer (`48 x 48` floats).
    pub fn heights(&self) -> &[f32] {
        &self.buffers[self.active]
    }
}

/// PS2 water specular intensity (`Effect2/hh_class_water_00.c` `Specular_Calculator` `0x0025bb80`).
#[inline]
pub fn ps2_water_specular(view_dir: Vec3, light_dir: Vec3, normal: Vec3) -> f32 {
    let n_dot_l = light_dir.dot(normal).abs();
    let reflected = (normal * (n_dot_l * 2.0) - light_dir).normalize_or_zero();
    let cos_v = reflected.dot(view_dir).abs();
    if cos_v < WATER_SPECULAR_COS_CUTOFF {
        0.0
    } else {
        1.0 - (1.0 - cos_v) / WATER_SPECULAR_INV_RANGE
    }
}
