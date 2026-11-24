//! 1:1 port of `Effect2/hh_class_dust_00.c` (`SLUS_202.28` `0x00257840..0x00257f30`) and
//! `Effect2/hh_class_raise_dust_phenomenon_00.c` (`SLUS_202.28` `0x0025b230..0x0025b2e0`):
//! Airborne dust motes kicked up by impacts/footsteps and illuminated by James's flashlight cone
//! via `HH_ClassWrapper_SpotLight_ColorRatio_Calculator` (`0x00269840`).

use glam::Vec3;

/// Maximum simultaneous airborne dust mote instances (`Object_Group_InstanceTableInfomeation_Set`
/// @ `0x0026a130` allocates a 600-slot instance pool at `0x011ebd80`).
pub const MAX_DUST_MOTES: usize = 600;

/// Vertical foot spawn offset in PS2 Y-down mm from `HH_Effect_Object_Ground_Impact_Post`
/// (`Effect2/hh_class_object_execute.c` `0x0025aa30` line 221: `fStack_7c = fStack_7c - 50.0`).
pub const GROUND_IMPACT_FOOT_Y_OFFSET_MM: f32 = -50.0;

/// Initial impact velocity `[x, y, z]` in PS2 mm/s passed to `HH_Class_Raise_Dust_Phenomenon_00`
/// (`0x10`) by `HH_Class_Plural_Phenomenon_01` (`Effect2/hh_class_plural_phenomenon_01.c`
/// `0x00286ea0` line 216, static vector at `SLUS_202.28` `0x0037b490`).
pub const PLURAL_PHENOMENON_RAISE_DUST_VEL: [f32; 3] = [0.0, -2500.0, 0.0];

/// Horizontal XZ velocity scale applied in `Object_Initialize` (`hh_class_dust_00.c` `0x00257840`).
pub const DUST_VEL_XZ_SCALE: f32 = 0.054_945_05;

/// Vertical Y velocity scale applied in `Object_Initialize` (`hh_class_dust_00.c` `0x00257840`).
pub const DUST_VEL_Y_SCALE: f32 = 0.164_835_16;

/// Downward parabolic acceleration in PS2 Y-down mm/s² (`0x43069d89` = `134.61537` in
/// `Object_Initialize` `0x00257840` & `ParabolaMotion_Calculator` `0x002578d0`).
pub const DUST_GRAVITY_Y_MM_S2: f32 = f32::from_bits(0x4306_9d89);

/// Fixed 60 Hz time step per tick in `HH_Class_Dust_00` (`0x00257f30` line 307).
pub const DUST_DT_60HZ: f32 = 0.016_666_668;

/// Base billboard half-extent in PS2 mm (`_vertex_info_list[0]` @ `SLUS_202.28` `0x0036e710`).
pub const DUST_BASE_HALF_EXTENT_MM: f32 = 200.0;

/// Starting scale multiplier (`_scale_start_list` @ `SLUS_202.28` `0x0036e8c0`).
pub const DUST_SCALE_START: f32 = 0.1;

/// Ending scale multiplier (`_scale_end_list` @ `SLUS_202.28` `0x0036e8c8`).
pub const DUST_SCALE_END: f32 = 1.0;

/// Starting GS alpha (`_rgba_start_list` @ `SLUS_202.28` `0x0036e8b0`).
pub const DUST_RGBA_START: f32 = 0.0;

/// Peak GS alpha (`_rgba_end_list` @ `SLUS_202.28` `0x0036e8b8`).
pub const DUST_RGBA_END: f32 = 120.0;

/// Ambient baseline GS alpha added regardless of spotlight illumination (`Object_Draw` `0x00257b30` line 193).
pub const DUST_AMBIENT_ALPHA_GS: f32 = 12.0;

/// Base dust RGB tint in PS2 0..255 GS space (`_85` @ `SLUS_202.28` `0x0036e900` = `(80, 93, 70)`).
pub const DUST_BASE_RGB_GS: [f32; 3] = [80.0, 93.0, 70.0];

/// Base dust RGB tint normalized to `[0.0, 1.0]` linear/sRGB display space.
pub const DUST_BASE_RGB_NORM: [f32; 3] = [80.0 / 255.0, 93.0 / 255.0, 70.0 / 255.0];

/// Total motion envelope duration in seconds (`0.6 + 1.5` from `_motion_info` @ `0x0036e8d0`).
pub const DUST_TOTAL_LIFETIME_SEC: f32 = 2.1;

/// Keyframe deltas `[dt_sec, dscale_t, dalpha_t]` from `_motion_info` (`SLUS_202.28` `0x0036e8d0`).
pub const DUST_MOTION_INFO: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [0.6, 0.8, 1.0], [1.5, 0.8, -1.0]];

/// Four sub-atlas UV rects `[u_min, v_min, u_max, v_max]` in `data/pic/effect/playable0.tbn2`
/// (`_square_0x_stq_list` @ `SLUS_202.28` `0x0036e890`, `0x0036e790..0x0036e850`).
pub const DUST_UV_QUADRANTS: [[f32; 4]; 4] = [
    [0.5, 0.5, 0.748_046_9, 0.748_046_9],
    [0.75, 0.5, 0.998_046_9, 0.748_046_9],
    [0.5, 0.75, 0.748_046_9, 0.998_046_9],
    [0.75, 0.75, 0.998_046_9, 0.998_046_9],
];

/// Number of dust motes spawned per `HH_Class_Raise_Dust_Phenomenon_00` (`0x0025b2e0`) burst.
pub const RAISE_DUST_BURST_COUNT: usize = 3;

/// Active spotlight cone parameters passed to `HH_ClassWrapper_SpotLight_ColorRatio_Calculator`
/// (`0x00269840`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DustSpotlightParams {
    /// Whether the spotlight is currently enabled (`HH_ClassWrapper_SpotLight_Enable_Check`).
    pub enabled: bool,
    /// Spotlight world-space origin in PS2 mm.
    pub position_mm: Vec3,
    /// Unit forward direction vector of the spotlight.
    pub direction: Vec3,
    /// Cosine of the outer cone cutoff angle (`param_1` in `0x00269840`).
    pub outer_cos: f32,
    /// Maximum spotlight range in PS2 mm (`param_2` in `0x00269840`).
    pub range_mm: f32,
}

impl Default for DustSpotlightParams {
    fn default() -> Self {
        Self {
            enabled: false,
            position_mm: Vec3::ZERO,
            direction: Vec3::NEG_Z,
            // Default 45-degree half-angle fallback matching `camera_wiring.rs`, inert while `enabled == false`.
            outer_cos: std::f32::consts::FRAC_PI_4.cos(),
            range_mm: super::fixes::PS2_FLASHLIGHT_RANGE_MM,
        }
    }
}

/// Exact port of `HH_ClassWrapper_SpotLight_ColorRatio_Calculator` (`Effect2/hh_class_wrapper.c` `0x00269840`).
///
/// Returns `[0.0, 1.0]` attenuation factor based on distance and angle from the flashlight cone.
#[must_use]
pub fn spotlight_color_ratio(spot: &DustSpotlightParams, world_pos: Vec3) -> f32 {
    if !spot.enabled || spot.range_mm <= 1e-4 {
        return 0.0;
    }
    let to_pt = world_pos - spot.position_mm;
    let dist = to_pt.length();
    if dist > spot.range_mm || dist <= 1e-4 {
        return 0.0;
    }
    let dir = to_pt / dist;
    let cos_angle = dir.dot(spot.direction);
    if cos_angle <= spot.outer_cos {
        return 0.0;
    }
    let denom = (1.0 - spot.outer_cos).max(1e-4);
    ((1.0 - dist / spot.range_mm) * ((cos_angle - spot.outer_cos) / denom)).clamp(0.0, 1.0)
}

/// Evaluates `Object_Motion_00` (`hh_class_dust_00.c` `0x00257950`) for elapsed time `age_sec`
/// and returns `(scale, gs_alpha)`.
#[must_use]
pub fn evaluate_dust_motion(age_sec: f32) -> (f32, f32) {
    let t = age_sec.max(0.0);
    let stage = if t <= DUST_MOTION_INFO[1][0] {
        0usize
    } else {
        1usize
    };

    let mut t_acc = 0.0f32;
    let mut scale_acc = 0.0f32;
    let mut alpha_acc = 0.0f32;
    for row in DUST_MOTION_INFO.iter().take(stage + 1) {
        t_acc += row[0];
        scale_acc += row[1];
        alpha_acc += row[2];
    }

    let next = DUST_MOTION_INFO[stage + 1];
    let t_next = t_acc + next[0];
    let frac = ((t - t_acc) / (t_next - t_acc).max(1e-5)).clamp(0.0, 1.0);
    let scale_t = scale_acc + frac * next[1];
    let alpha_t = alpha_acc + frac * next[2];

    let scale = DUST_SCALE_START * (1.0 - scale_t) + DUST_SCALE_END * scale_t;
    let gs_alpha = DUST_RGBA_START * (1.0 - alpha_t) + DUST_RGBA_END * alpha_t;
    (scale, gs_alpha)
}

/// Single airborne dust mote (`Effect2/hh_class_dust_00.c` `0x00257840..0x00257f30`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DustMote {
    /// Spawn origin in PS2 mm.
    pub origin: Vec3,
    /// Floor Y cutoff in PS2 Y-down mm (particle despawns when `world_pos.y > floor_y`).
    pub floor_y: f32,
    /// Scaled initial velocity in PS2 mm/s (`Object_Initialize` `0x00257840`).
    pub velocity: Vec3,
    /// Elapsed lifetime in seconds (`param_1 + 0x18`).
    pub age_sec: f32,
    /// Sub-atlas quadrant index `0..4` (`param_1 + 0x14`).
    pub quadrant: u8,
    /// Current evaluated scale multiplier (`param_1 + 0x30`).
    pub scale: f32,
    /// Current evaluated motion GS alpha (`param_1 + 0x34`).
    pub motion_alpha_gs: f32,
    /// Current world-space position in PS2 mm.
    pub world_pos: Vec3,
}

impl DustMote {
    /// Initializes a dust mote identically to `Object_Initialize` (`0x00257840`).
    #[must_use]
    pub fn new(origin: Vec3, impact_vel: Vec3, floor_y: f32, quadrant: u8) -> Self {
        let velocity = Vec3::new(
            impact_vel.x * DUST_VEL_XZ_SCALE,
            impact_vel.y * DUST_VEL_Y_SCALE,
            impact_vel.z * DUST_VEL_XZ_SCALE,
        );
        let (scale, motion_alpha_gs) = evaluate_dust_motion(0.0);
        Self {
            origin,
            floor_y,
            velocity,
            age_sec: 0.0,
            quadrant: quadrant & 3,
            scale,
            motion_alpha_gs,
            world_pos: origin,
        }
    }

    /// Evaluates `ParabolaMotion_Calculator` (`0x002578d0`) at `self.age_sec`.
    #[must_use]
    pub fn parabola_offset(&self) -> Vec3 {
        let t = self.age_sec;
        Vec3::new(
            self.velocity.x * t,
            self.velocity.y * t + DUST_GRAVITY_Y_MM_S2 * t * t,
            self.velocity.z * t,
        )
    }

    /// Evaluates final normalized `[0.0, 1.0]` opacity for GPU alpha blending (`Object_Draw`
    /// `0x00257b30` line 193 + GS `ALPHA_1` `0x80` = 1.0).
    #[must_use]
    pub fn eval_alpha_norm(&self, spot: &DustSpotlightParams) -> f32 {
        let ratio = spotlight_color_ratio(spot, self.world_pos);
        let gs_alpha = (self.motion_alpha_gs * ratio + DUST_AMBIENT_ALPHA_GS).clamp(0.0, 255.0);
        (gs_alpha / 128.0).clamp(0.0, 1.0)
    }
}

/// Manager for `HH_Class_Dust_00` and `HH_Class_Raise_Dust_Phenomenon_00` airborne dust motes.
#[derive(Debug, Clone)]
pub struct DustSystem {
    pub motes: Vec<DustMote>,
    rng_state: u32,
}

impl Default for DustSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DustSystem {
    /// Creates an empty dust mote system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            motes: Vec::with_capacity(MAX_DUST_MOTES),
            rng_state: 0x1357_9BDF,
        }
    }

    fn next_rand(&mut self) -> u32 {
        // Numerical Recipes LCG matching deterministic PRNG style across `sh2-core`.
        self.rng_state = self
            .rng_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.rng_state >> 16
    }

    /// Spawns a single `HH_Class_Dust_00` mote (`0x00257840`).
    pub fn spawn_mote(&mut self, origin: Vec3, impact_vel: Vec3, floor_y: f32) -> bool {
        if self.motes.len() >= MAX_DUST_MOTES {
            return false;
        }
        let q = (self.next_rand() & 3) as u8;
        self.motes
            .push(DustMote::new(origin, impact_vel, floor_y, q));
        true
    }

    /// Spawns a 3-mote radial burst matching `HH_Class_Raise_Dust_Phenomenon_00` (`0x0025b2e0`).
    pub fn spawn_raise_dust(&mut self, origin: Vec3, impact_vel: Vec3, floor_y: f32) {
        let speed = impact_vel.length().max(1.0);
        let base_pitch = (impact_vel.y / speed).clamp(-1.0, 1.0).asin().abs() * 0.5;

        for _ in 0..RAISE_DUST_BURST_COUNT {
            let yaw = ((self.next_rand() % 360) as f32) * 0.017_453_292;
            let pitch =
                base_pitch + (((self.next_rand() % 10) as f32) * 0.017_453_292 - 0.087_266_46);
            let radius = ((self.next_rand() % 50) as f32) + 10.0;

            let (sy, cy) = yaw.sin_cos();
            let spawn_pos = origin + Vec3::new(sy * radius, 0.0, cy * radius);

            // `sceVu0RotMatrixX(pitch)` + `sceVu0RotMatrixY(yaw)` applied to `(0, 0, speed)`
            let (sp, cp) = pitch.sin_cos();
            let vel_local = Vec3::new(0.0, -sp * speed, cp * speed);
            let burst_vel = Vec3::new(vel_local.z * sy, vel_local.y, vel_local.z * cy);

            self.spawn_mote(spawn_pos, burst_vel, floor_y);
        }
    }

    /// Advances all active dust motes by `dt_sec` (`HH_Class_Dust_00` `0x00257f30`).
    pub fn tick(&mut self, dt_sec: f32) {
        let dt = dt_sec.max(0.0);
        let mut i = 0;
        while i < self.motes.len() {
            let mote = &mut self.motes[i];
            mote.age_sec += dt;
            let offset = mote.parabola_offset();
            mote.world_pos = mote.origin + offset;
            // `HH_Class_Dust_00` (`0x00257f30` lines 302-305): despawn strictly on floor contact
            // (`0.0 < fStack_1c + origin.y`); `Object_Draw` (`0x00257b30`) continues drawing with
            // `+ 12.0` ambient GS alpha even after `motion_alpha` reaches `0.0`.
            if mote.world_pos.y > mote.floor_y {
                self.motes.swap_remove(i);
                continue;
            }
            let (scale, motion_alpha_gs) = evaluate_dust_motion(mote.age_sec);
            mote.scale = scale;
            mote.motion_alpha_gs = motion_alpha_gs;
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dust_motion_and_parabola_match_slus_00257840() {
        let m0 = DustMote::new(
            Vec3::new(100.0, -200.0, 50.0),
            Vec3::new(100.0, -300.0, -100.0),
            0.0,
            2,
        );
        assert!((m0.velocity.x - 100.0 * DUST_VEL_XZ_SCALE).abs() < 1e-5);
        assert!((m0.velocity.y - (-300.0) * DUST_VEL_Y_SCALE).abs() < 1e-5);
        assert_eq!(m0.quadrant, 2);

        let (s0, a0) = evaluate_dust_motion(0.0);
        assert!((s0 - DUST_SCALE_START).abs() < 1e-5);
        assert!((a0 - DUST_RGBA_START).abs() < 1e-5);

        let (s_peak, a_peak) = evaluate_dust_motion(0.6);
        assert!((s_peak - (0.1 * 0.2 + 1.0 * 0.8)).abs() < 1e-4);
        assert!((a_peak - DUST_RGBA_END).abs() < 1e-4);

        let (_s_end, a_end) = evaluate_dust_motion(2.1);
        assert!(a_end.abs() < 1e-4);
    }

    #[test]
    fn spotlight_color_ratio_boosts_dust_alpha_inside_cone() {
        let mote = DustMote {
            origin: Vec3::new(0.0, -500.0, -2000.0),
            floor_y: 0.0,
            velocity: Vec3::ZERO,
            age_sec: 0.6,
            quadrant: 0,
            scale: 0.82,
            motion_alpha_gs: 120.0,
            world_pos: Vec3::new(0.0, -500.0, -2000.0),
        };
        let spot_off = DustSpotlightParams::default();
        let spot_on = DustSpotlightParams {
            enabled: true,
            position_mm: Vec3::new(0.0, -500.0, 0.0),
            direction: Vec3::NEG_Z,
            outer_cos: 0.75,
            range_mm: 7000.0,
        };
        let alpha_off = mote.eval_alpha_norm(&spot_off);
        let alpha_on = mote.eval_alpha_norm(&spot_on);
        assert!((alpha_off - 12.0 / 128.0).abs() < 1e-5);
        assert!(alpha_on > alpha_off * 5.0);
    }

    #[test]
    fn raise_dust_spawns_three_motes_and_despawns_on_floor_contact() {
        let mut sys = DustSystem::new();
        sys.spawn_raise_dust(
            Vec3::new(0.0, -50.0, 0.0),
            Vec3::from_array(PLURAL_PHENOMENON_RAISE_DUST_VEL),
            0.0,
        );
        assert_eq!(sys.motes.len(), 3);
        // After 2.1s (126 ticks), motes are still airborne and retain `+12.0` ambient GS alpha
        for _ in 0..126 {
            sys.tick(DUST_DT_60HZ);
        }
        assert_eq!(sys.motes.len(), 3);
        // Advance until parabolic gravity returns all motes to floor_y = 0.0
        for _ in 0..120 {
            sys.tick(DUST_DT_60HZ);
        }
        assert!(sys.motes.is_empty());
    }
}
