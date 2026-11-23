//! Combat hitbox geometry, swept melee blade quads, 3D cone shotgun pellets, and 5-ray Hyper Spray.
//!
//! # Ground Truth & Binary References
//! - **`shBattleAttackByHumanFightType` (`SLUS_202.28` `0x00121370` / `sh2pc.exe` `0x00543ec0`):**
//!   Swept blade quad `(curr_base, prev_tip, curr_base, curr_tip)` constructed from `min_range..max_range`
//!   and the previous frame's weapon tip (`chara + 0x150` / `sh2pc +0x128`).
//! - **`shBattleAttackByHumanGunshotTypeB` (`SLUS_202.28` `0x00120f30` / `sh2pc.exe` `0x00543bc0`):**
//!   10 randomized 3D cone pellets with `30°` half-angle (`0.5235988` rad) and `±π` azimuth (`3.1415927` rad),
//!   rotated into world space via `RotY(aim_yaw) * RotX(-aim_pitch)`.
//! - **`shGetHumanAttackSprayPos` (`SLUS_202.28` `0x00121630` / `sh2pc.exe` `0x00545694`) &
//!   `shBattleAttackByHumanFog` (`0x00121a40`):**
//!   5-ray aerosol cone (`±0.41887903` rad = `24°`, `900.0` mm step) with linear range tapering below `10.0` s.

use glam::Vec3;

/// Number of pellets fired per Shotgun blast (`shBattleAttackByHumanGunshotTypeB` `0x00120f30` line 559).
pub const SHOTGUN_PELLET_COUNT: usize = 10;
/// Half-angle of the 3D Shotgun spread cone in radians (`30°` = `FRAC_PI_6` = `0.5235988`, `0x00120f30` line 561).
pub const SHOTGUN_CONE_HALF_ANGLE_RAD: f32 = std::f32::consts::FRAC_PI_6;
/// Half-angle of the 5-ray Hyper Spray cone in radians (`24°` = `0.41887903`, `0x00121630` line 732).
pub const HYPER_SPRAY_HALF_ANGLE_RAD: f32 = 0.418_879_03;
/// Forward step distance used to normalize Hyper Spray rays in mm (`900.0`, `0x00121630` line 733).
pub const HYPER_SPRAY_STEP_MM: f32 = 900.0;
/// Full Hyper Spray charge duration in seconds (`20.0` = `0x41a00000`, `m3_play.c` `0x0010fcf0` line 4871).
pub const HYPER_SPRAY_MAX_CHARGE_SECS: f32 = 20.0;
/// Charge threshold in seconds below which Hyper Spray range tapers linearly (`10.0`, `0x00121a40` line 839).
pub const HYPER_SPRAY_TAPER_THRESHOLD_SECS: f32 = 10.0;
/// Initial Hyper Spray shake recharge counter (`200`, `m3_play_common.c` `0x0021ef50` line 1426).
pub const HYPER_SPRAY_INITIAL_SHAKES: u8 = 200;

/// Deterministic LCG PRNG matching Team Silent's `shRandF()` (`[0.0, 1.0)`) for reproducible
/// 3D Shotgun cone spread in deterministic 60 Hz simulation and unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatRng {
    state: u32,
}

impl Default for CombatRng {
    fn default() -> Self {
        Self::new(0x5348_3201)
    }
}

impl CombatRng {
    /// Create a new deterministic combat PRNG with the given seed.
    pub const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x5348_3201 } else { seed },
        }
    }

    /// Generate the next uniform float in `[0.0, 1.0)`.
    pub fn next_f32(&mut self) -> f32 {
        // Standard 32-bit LCG (Numerical Recipes constants, same family as PS2 shRandI).
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        ((self.state >> 8) as f32) * (1.0 / 16_777_216.0)
    }
}

/// Swept melee blade quad emitted by `shBattleAttackByHumanFightType` (`SLUS_202.28` `0x00121370`
/// / `sh2pc.exe` `0x00543ec0`).
///
/// On each active hit frame, the blade spans from `curr_base = origin + dir * min_range` to
/// `curr_tip = origin + dir * max_range`, sweeping a triangle `(curr_base, prev_tip, curr_tip)`
/// from the previous frame's weapon tip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweptBladeQuad {
    /// Current frame blade base (`origin + dir * min_range`, PS2 world mm).
    pub curr_base: Vec3,
    /// Previous frame blade tip (`chara + 0x150` / `sh2pc +0x128`, PS2 world mm).
    pub prev_tip: Vec3,
    /// Current frame blade tip (`origin + dir * max_range`, PS2 world mm).
    pub curr_tip: Vec3,
}

impl SweptBladeQuad {
    /// Construct a swept blade quad from weapon origin, normalized weapon direction,
    /// attack `min_range..max_range` (mm), and the previous frame's tip position.
    pub fn new(
        origin: Vec3,
        dir: Vec3,
        min_range: f32,
        max_range: f32,
        prev_tip: Option<Vec3>,
    ) -> Self {
        let curr_base = origin + dir * min_range;
        let curr_tip = origin + dir * max_range;
        Self {
            curr_base,
            prev_tip: prev_tip.unwrap_or(curr_tip),
            curr_tip,
        }
    }
}

/// Generate the 10 randomized 3D cone pellet directions for a Shotgun blast
/// (`shBattleAttackByHumanGunshotTypeB`, `SLUS_202.28` `0x00120f30` / `sh2pc.exe` `0x00543bc0`).
///
/// Each pellet draws:
/// - `theta = rand_f() * 0.5235988` (`0..30°` cone half-angle)
/// - `phi = (rand_f() * 2.0 - 1.0) * 3.1415927` (`-π..π` roll around aim axis)
/// - local dir `(sin(theta) * sin(phi), -sin(theta) * cos(phi), cos(theta))`
/// - world dir `RotY(aim_yaw) * RotX(-aim_pitch) * local_dir`
pub fn generate_shotgun_cone_rays(
    aim_dir: Vec3,
    rng: &mut CombatRng,
) -> [Vec3; SHOTGUN_PELLET_COUNT] {
    let dir = aim_dir.normalize_or_zero();
    let horiz = (dir.x * dir.x + dir.z * dir.z).sqrt();
    let aim_yaw = dir.x.atan2(dir.z);
    let aim_pitch = -dir.y.atan2(horiz);

    let (sin_x, cos_x) = aim_pitch.sin_cos();
    let (sin_y, cos_y) = aim_yaw.sin_cos();

    let mut rays = [Vec3::Z; SHOTGUN_PELLET_COUNT];
    for ray in &mut rays {
        let theta = rng.next_f32() * SHOTGUN_CONE_HALF_ANGLE_RAD;
        let phi = (rng.next_f32() * 2.0 - 1.0) * std::f32::consts::PI;

        let (sin_t, cos_t) = theta.sin_cos();
        let (sin_p, cos_p) = phi.sin_cos();

        let lx = sin_t * sin_p;
        let ly = -sin_t * cos_p;
        let lz = cos_t;

        // shRotMatrixX(-pitch): y' = y*cos_x - z*sin_x, z' = y*sin_x + z*cos_x
        let rx = lx;
        let ry = ly * cos_x - lz * sin_x;
        let rz = ly * sin_x + lz * cos_x;

        // shRotMatrixY(yaw): x'' = rx*cos_y + rz*sin_y, z'' = -rx*sin_y + rz*cos_y
        let wx = rx * cos_y + rz * sin_y;
        let wy = ry;
        let wz = -rx * sin_y + rz * cos_y;

        *ray = Vec3::new(wx, wy, wz).normalize_or_zero();
    }
    rays
}

/// Generate the 5 normalized ray directions for Hyper Spray
/// (`shGetHumanAttackSprayPos`, `SLUS_202.28` `0x00121630` / `sh2pc.exe` `0x00545694`).
///
/// Ray ordering matches `param_1 = 0..5` in `0x00121630`:
/// - `0`: center `(yaw, pitch)`
/// - `1`: pitch `+0.41887903` rad
/// - `2`: yaw `+0.41887903` rad
/// - `3`: pitch `-0.41887903` rad
/// - `4`: yaw `-0.41887903` rad
pub fn generate_hyper_spray_rays(aim_dir: Vec3) -> [Vec3; 5] {
    let dir = aim_dir.normalize_or_zero();
    let horiz = (dir.x * dir.x + dir.z * dir.z).sqrt();
    let yaw = dir.x.atan2(dir.z);
    let pitch = dir.y.atan2(horiz);

    let offsets: [(f32, f32); 5] = [
        (0.0, 0.0),
        (0.0, HYPER_SPRAY_HALF_ANGLE_RAD),
        (HYPER_SPRAY_HALF_ANGLE_RAD, 0.0),
        (0.0, -HYPER_SPRAY_HALF_ANGLE_RAD),
        (-HYPER_SPRAY_HALF_ANGLE_RAD, 0.0),
    ];

    let mut rays = [Vec3::Z; 5];
    for (i, &(dy, dp)) in offsets.iter().enumerate() {
        let vx = (yaw + dy).sin() * HYPER_SPRAY_STEP_MM;
        let vy = (pitch + dp).sin() * HYPER_SPRAY_STEP_MM;
        let vz = (yaw + dy).cos() * HYPER_SPRAY_STEP_MM;
        rays[i] = Vec3::new(vx, vy, vz).normalize_or_zero();
    }
    rays
}

/// Compute tapered `(min_range, max_range)` in mm for Hyper Spray based on remaining charge
/// (`shBattleAttackByHumanFog`, `SLUS_202.28` `0x00121a40` lines 839–848).
#[inline]
pub fn hyper_spray_effective_range(charge_secs: f32, base_min: f32, base_max: f32) -> (f32, f32) {
    if charge_secs <= 0.0 {
        (0.0, 0.0)
    } else if charge_secs >= HYPER_SPRAY_TAPER_THRESHOLD_SECS {
        (base_min, base_max)
    } else {
        let scale = charge_secs * 0.1;
        (base_min * scale, base_max * scale)
    }
}

/// 3D Collision Hitbox Capsule / Cylinder for characters and enemies (`clDbgDrawHitPolygon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatHitbox {
    pub center_x: f32,
    pub center_z: f32,
    pub radius: f32,
    pub top_y: f32,    // In PS2 world units, Y is negative-up (-1800.0 is head level)
    pub bottom_y: f32, // Floor level (0.0)
    pub is_downed: bool,
}

impl CombatHitbox {
    /// Create vertical character hitbox standing at (x, y, z).
    pub fn new_standing(pos: Vec3, radius: f32, height: f32) -> Self {
        Self {
            center_x: pos.x,
            center_z: pos.z,
            radius,
            top_y: pos.y - height,
            bottom_y: pos.y,
            is_downed: false,
        }
    }

    /// Create downed / fallen character hitbox (wide horizontal torso cylinder).
    pub fn new_downed(pos: Vec3, radius: f32, height: f32) -> Self {
        Self {
            center_x: pos.x,
            center_z: pos.z,
            radius: radius * 1.5,
            top_y: pos.y - height * 0.3,
            bottom_y: pos.y,
            is_downed: true,
        }
    }

    /// Test if a 2D line segment (e.g. melee blade from base to tip) intersects this cylinder.
    pub fn intersects_segment(&self, a: Vec3, b: Vec3) -> bool {
        let seg_y_min = a.y.min(b.y);
        let seg_y_max = a.y.max(b.y);
        let cyl_y_min = self.top_y.min(self.bottom_y);
        let cyl_y_max = self.top_y.max(self.bottom_y);
        if seg_y_max < cyl_y_min || seg_y_min > cyl_y_max {
            return false;
        }

        // Distance from point to segment in XZ plane
        let vx = b.x - a.x;
        let vz = b.z - a.z;
        let wx = self.center_x - a.x;
        let wz = self.center_z - a.z;
        let l_sq = vx * vx + vz * vz;

        let t = if l_sq > 0.0 {
            ((wx * vx + wz * vz) / l_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let proj_x = a.x + vx * t;
        let proj_z = a.z + vz * t;
        let dx = proj_x - self.center_x;
        let dz = proj_z - self.center_z;

        (dx * dx + dz * dz) <= (self.radius * self.radius)
    }

    /// Test if a swept melee blade quad (`shBattleAttackByHumanFightType` `0x00121370`)
    /// intersects this cylinder.
    ///
    /// Tests vertical overlap, all three edges of the swept triangle `(curr_base, prev_tip, curr_tip)`,
    /// and XZ point-in-triangle containment for the cylinder center.
    pub fn intersects_swept_quad(&self, quad: &SweptBladeQuad) -> bool {
        let quad_y_min = quad.curr_base.y.min(quad.prev_tip.y).min(quad.curr_tip.y);
        let quad_y_max = quad.curr_base.y.max(quad.prev_tip.y).max(quad.curr_tip.y);
        let cyl_y_min = self.top_y.min(self.bottom_y);
        let cyl_y_max = self.top_y.max(self.bottom_y);
        if quad_y_max < cyl_y_min || quad_y_min > cyl_y_max {
            return false;
        }

        // Edge checks cover both the current blade segment, the previous blade segment,
        // and the swept tip arc.
        if self.intersects_segment(quad.curr_base, quad.curr_tip)
            || self.intersects_segment(quad.curr_base, quad.prev_tip)
            || self.intersects_segment(quad.prev_tip, quad.curr_tip)
        {
            return true;
        }

        // Point-in-triangle check in XZ plane (for wide arcs sweeping completely across cylinder center).
        let px = self.center_x;
        let pz = self.center_z;
        let ax = quad.curr_base.x;
        let az = quad.curr_base.z;
        let bx = quad.prev_tip.x;
        let bz = quad.prev_tip.z;
        let cx = quad.curr_tip.x;
        let cz = quad.curr_tip.z;

        let d1 = (px - bx) * (az - bz) - (ax - bx) * (pz - bz);
        let d2 = (px - cx) * (bz - cz) - (bx - cx) * (pz - cz);
        let d3 = (px - ax) * (cz - az) - (cx - ax) * (pz - az);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
        !(has_neg && has_pos)
    }

    /// Test if a ray (e.g. gunshot or spray ray) intersects this cylinder between `min_dist` and `max_dist`.
    /// Returns distance `t` along ray direction if hit occurs.
    pub fn intersects_ray(
        &self,
        origin: Vec3,
        dir: Vec3,
        min_dist: f32,
        max_dist: f32,
    ) -> Option<f32> {
        let hx = dir.x;
        let hz = dir.z;
        let h2 = hx * hx + hz * hz;
        if h2 < 1e-6 {
            return None;
        }

        let wx = self.center_x - origin.x;
        let wz = self.center_z - origin.z;
        let t = (wx * hx + wz * hz) / h2;

        let px = origin.x + hx * t - self.center_x;
        let pz = origin.z + hz * t - self.center_z;
        let miss2 = px * px + pz * pz;

        if miss2 > self.radius * self.radius {
            return None;
        }

        let back = ((self.radius * self.radius - miss2) / h2).sqrt();
        let t_exit = t + back;
        if t_exit < min_dist {
            return None;
        }
        let t_enter = (t - back).max(min_dist);

        if t_enter > max_dist {
            return None;
        }

        let y_at_hit = origin.y + dir.y * t_enter;
        let cyl_y_min = self.top_y.min(self.bottom_y);
        let cyl_y_max = self.top_y.max(self.bottom_y);
        if y_at_hit < cyl_y_min - 50.0 || y_at_hit > cyl_y_max + 50.0 {
            return None;
        }

        Some(t_enter)
    }
}
