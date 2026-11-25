//! 1:1 port of the PS2 / PC `Effect2` Impact Queue Blood System:
//! - `Effect2/hh_class_blood_00.c` (`0x00253b00`): Floor blood stain from falling drops (Class 10)
//! - `Effect2/hh_class_blood_01.c` (`0x002542e0`): Airborne hit spray & falling drops (Class 5)
//! - `Effect2/hh_class_blood_02.c` (`0x00254af0`): 3-stage expanding death blood pool (Class 12)
//! - `Effect2/hh_class_blood_footmark_00.c` (`0x00255be0`): Bloody footprints (Class 9)
//! - `Effect2/hh_class_blood_pool_phenomenon_00.c` (`0x00256050`): 3-pool death emitter (Class 3)
//! - `Effect2/hh_class_blood_splash_phenomenon_00.c` (`0x00256370`): Hit spray emitter (Class 11)
//! - `Effect2/hh_class_blood_stick_00.c` (`0x00257400`): Shoe bloodiness tracker (10 steps)
//! - `Effect2/hh_class_object_execute.c` (`0x0025a780..0x0025a8d0`): Impact Queue posts

pub mod tables;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use glam::Vec3;

use crate::config::BloodColor;
pub use tables::{
    blood_color_non_subtractive_rgba_norm, blood_color_subtractive_rgb_norm,
    BLOOD_00_HOLD_DURATION_SEC, BLOOD_00_MOTION_INFO, BLOOD_00_SCALE_END_MM, BLOOD_00_UV_RECT,
    BLOOD_01_ALPHA_END_GS, BLOOD_01_ALPHA_START_GS, BLOOD_01_DROP_UV_RECTS, BLOOD_01_GRAVITY_MM_S2,
    BLOOD_01_HALF_EXTENT_MM, BLOOD_01_LIFETIME_SEC, BLOOD_01_MIST_UV_RECTS, BLOOD_01_SCALE_END,
    BLOOD_01_SCALE_START, BLOOD_01_SUPPRESS_COFF, BLOOD_01_VEL_XZ_SCALE, BLOOD_01_VEL_Y_SCALE,
    BLOOD_02_COLLISION_SCALE_SQ, BLOOD_02_MOTION_INFO, BLOOD_02_SCALE_END_MM, BLOOD_02_UV_RECTS,
    BLOOD_COLOR_NON_SUBTRACTIVE_RGBA_GS, BLOOD_COLOR_SUBTRACTIVE_RGB_GS, BLOOD_DT_30HZ,
    BLOOD_DT_60HZ, BLOOD_FOOTMARK_BASE_ALPHA_GS, BLOOD_FOOTMARK_FADE_PER_30HZ,
    BLOOD_FOOTMARK_HALF_LENGTH_MM, BLOOD_FOOTMARK_HALF_WIDTH_MM, BLOOD_FOOTMARK_HOLD_SEC,
    BLOOD_FOOTMARK_STANCE_OFFSET_MM, BLOOD_FOOTMARK_UV_RECTS, BLOOD_POOL_00_MIN_OFFSET_MM,
    BLOOD_POOL_00_OFFSET_RANGE_MM, BLOOD_POOL_00_TIME_TABLE, BLOOD_SPLASH_00_DISCHARGE_TIME,
    BLOOD_SPLASH_00_DURATION_SEC, BLOOD_SPLASH_00_FULL_NUMBER, BLOOD_STICK_MAX_LIFETIME_SEC,
    EN_EFCT_BLOOD_POOL_Y_OFFSET_MM, MAX_BLOODY_STEPS, SH2EE_BLOOD_SIZE_SCALE,
};

/// Maximum simultaneous airborne blood spray/drop particles (`Class 5`).
pub const MAX_BLOOD_DROPS: usize = 128;

/// Maximum simultaneous floor blood stains (`Class 10`).
pub const MAX_BLOOD_STAINS: usize = 128;

/// Maximum simultaneous expanding death blood pools (`Class 12`).
pub const MAX_BLOOD_POOLS: usize = 64;

/// Maximum simultaneous bloody footprints (`Class 9`).
pub const MAX_BLOOD_FOOTMARKS: usize = 64;

/// Blood color mode wrapper.
pub type BloodColorMode = BloodColor;

/// Runtime configuration for [`BloodEffectSystem`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodSystemConfig {
    /// PS2 Extra Options Blood Color (`Normal`, `Green`, `Violet`, `Black`).
    pub blood_color: BloodColor,
    /// SH2:EE `IncreaseBlood` scale multiplier (`1.0` when enabled, `1.0` default).
    pub blood_size_scale: f32,
    /// Native-upgrade Persistent Room Decals & Coagulation (`false` = strict 1:1 PS2 `blood_clear()`).
    pub persistent_room_decals: bool,
    /// Use non-subtractive alpha blending fallback (`DAT_00a3339c == 0`).
    pub non_subtractive_fallback: bool,
}

impl Default for BloodSystemConfig {
    fn default() -> Self {
        Self {
            blood_color: BloodColor::Normal,
            blood_size_scale: SH2EE_BLOOD_SIZE_SCALE,
            persistent_room_decals: false,
            non_subtractive_fallback: false,
        }
    }
}

/// Impact Queue event type (`Effect2/hh_class_manager.c` `ImpactDescriptor_Post`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactKind {
    /// `Class 3` (`HH_Class_Blood_Pool_Phenomenon_00` `0x002560a0`).
    BloodPoolEmitter,
    /// `Class 11` (`HH_Class_Blood_Splash_Phenomenon_00` `0x00256ba0`).
    BloodSplashEmitter,
    /// `Class 5` (`HH_Class_Blood_01` `0x002548e0`).
    BloodParticle,
    /// `Class 10` (`HH_Class_Blood_00` `0x00253b00`).
    FloorStain,
    /// `Class 12` (`HH_Class_Blood_02` `0x002551f0`).
    ExpandingPool,
    /// `Class 9` (`HH_Class_Blood_FootMark_00` `0x00255f40`).
    Footmark,
}

/// Descriptor queued into `HH_Effect_Object_Impact_Post` (`0x0026a1e0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactDescriptor {
    pub kind: ImpactKind,
    pub position_mm: Vec3,
    pub velocity_mm_s: Vec3,
    pub yaw_rad: f32,
    /// Sub-mode (`0` = mist/right foot, `1` = falling drop/left foot).
    pub mode: u8,
}

/// Renderable blood quad classification for `sh2-renderer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BloodQuadKind {
    /// Camera-facing 3D billboard (`Class 5` `Blood01`).
    Billboard,
    /// Floor-aligned decal (`Class 10` `Blood00`, `Class 12` `Blood02`, `Class 9` `FootMark00`).
    FloorDecal,
}

/// Renderable blood quad emitted by [`BloodEffectSystem::build_render_items`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodQuadRenderItem {
    pub kind: BloodQuadKind,
    /// World-space center in PS2 mm.
    pub position_mm: Vec3,
    /// Half-extents `(hx, hz)` in PS2 mm (for billboards, `hx == hz` radius).
    pub half_extents_mm: [f32; 2],
    /// Y-axis rotation in radians (for floor decals).
    pub yaw_rad: f32,
    /// Sub-atlas UV rect `[u_min, v_min, u_max, v_max]` in `bloodtex0.tbn2`.
    pub uv_rect: [f32; 4],
    /// Normalized RGB in `[0.0, 1.0]` (subtractive `Cs` or fallback color).
    pub color_rgb: [f32; 3],
    /// Normalized alpha `As` in `[0.0, 1.0]` (`alpha_gs / 128.0`).
    pub alpha: f32,
    /// `true` for PS2 GS subtractive (`Cd - Cs * As`), `false` for normal alpha blend.
    pub subtractive: bool,
}

#[inline]
fn next_lcg_u32(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

#[inline]
fn next_lcg_f32(state: &mut u32) -> f32 {
    (next_lcg_u32(state) >> 8) as f32 / (1u32 << 24) as f32
}

/// Evaluates a piecewise-linear 3-stage motion envelope `[dt_sec, dscale, dalpha]`
/// matching `Object_Motion_00` in `hh_class_blood_00.c` (`0x00253c80`) & `hh_class_blood_02.c` (`0x00254c70`).
#[must_use]
pub fn evaluate_three_stage_envelope(elapsed_sec: f32, keyframes: &[[f32; 3]; 4]) -> (f32, f32) {
    let mut t_acc = 0.0f32;
    let mut s_acc = 0.0f32;
    let mut a_acc = 0.0f32;

    for kf in &keyframes[1..] {
        let dt = kf[0];
        let ds = kf[1];
        let da = kf[2];
        let next_t = t_acc + dt;
        if elapsed_sec <= next_t || dt <= 1e-6 {
            let frac = if dt > 1e-6 {
                ((elapsed_sec - t_acc) / dt).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return (
                (s_acc + ds * frac).clamp(0.0, 1.0),
                (a_acc + da * frac).clamp(0.0, 1.0),
            );
        }
        t_acc = next_t;
        s_acc += ds;
        a_acc += da;
    }

    (s_acc.clamp(0.0, 1.0), a_acc.clamp(0.0, 1.0))
}

/// Exact port of `ParabolaMotion_Calculator` (`hh_class_blood_01.c` `0x00254380`):
/// `offset = vel * t + (0, gravity_y * t * t, 0)`.
#[must_use]
pub fn evaluate_blood_parabola(vel_mm_s: Vec3, gravity_y_mm_s2: f32, t_sec: f32) -> Vec3 {
    Vec3::new(
        vel_mm_s.x * t_sec,
        vel_mm_s.y * t_sec + gravity_y_mm_s2 * t_sec * t_sec,
        vel_mm_s.z * t_sec,
    )
}

#[derive(Debug, Clone)]
struct BloodSplashEmitter {
    origin_mm: Vec3,
    velocity_mm_s: Vec3,
    elapsed_sec: f32,
    next_emit_sec: [f32; 2],
    emitted_count: [u16; 2],
}

#[derive(Debug, Clone)]
struct BloodPoolEmitter {
    origin_mm: Vec3,
    elapsed_sec: f32,
    table_row: usize,
    spawned_count: usize,
}

#[derive(Debug, Clone)]
struct BloodParticle {
    origin_mm: Vec3,
    vel_mm_s: Vec3,
    elapsed_sec: f32,
    mode: u8,
    quadrant: u8,
    scale: f32,
    alpha_gs: f32,
    active: bool,
}

#[derive(Debug, Clone)]
struct FloorStain {
    position_mm: Vec3,
    yaw_rad: f32,
    variant: usize,
    elapsed_sec: f32,
    fade_alpha: f32,
    coagulation: f32,
}

#[derive(Debug, Clone)]
struct ExpandingPool {
    position_mm: Vec3,
    yaw_rad: f32,
    variant: usize,
    uv_variant: usize,
    elapsed_sec: f32,
    size_scale: f32,
    coagulation: f32,
}

impl ExpandingPool {
    /// Current `[hx, hz]` half-extents in mm (`_scale_end_list` * envelope * `IncreaseBlood`).
    fn current_half_extents(&self) -> [f32; 2] {
        let (s_t, _) = evaluate_three_stage_envelope(self.elapsed_sec, &BLOOD_02_MOTION_INFO);
        let end = BLOOD_02_SCALE_END_MM[self.variant % BLOOD_02_SCALE_END_MM.len()];
        [
            end[0] * s_t * self.size_scale,
            end[1] * s_t * self.size_scale,
        ]
    }

    /// Exact port of `HH_Class_Blood_02_DesignateLocation_CollisionCheck` (`0x002552c0`):
    /// `dist_sq(player_xz, pool_xz) < min_radius_sq * 0.95`.
    fn contains_xz(&self, pos_mm: Vec3) -> bool {
        let [hx, hz] = self.current_half_extents();
        let min_r = hx.min(hz);
        if min_r <= 1.0 {
            return false;
        }
        let dx = pos_mm.x - self.position_mm.x;
        let dz = pos_mm.z - self.position_mm.z;
        let dist_sq = dx * dx + dz * dz;
        dist_sq < (min_r * min_r) * BLOOD_02_COLLISION_SCALE_SQ
    }
}

#[derive(Debug, Clone)]
struct BloodFootmark {
    position_mm: Vec3,
    yaw_rad: f32,
    side: u8,
    variant: u8,
    elapsed_sec: f32,
    alpha_scale: f32,
    coagulation: f32,
}

/// Archived per-room floor decals for Persistent Room Decals & Coagulation mode.
#[derive(Debug, Clone, Default)]
struct RoomBloodArchive {
    stains: Vec<FloorStain>,
    pools: Vec<ExpandingPool>,
    footmarks: Vec<BloodFootmark>,
}

/// Deterministic 60 Hz PS2/PC `Effect2` Impact Queue Blood System.
#[derive(Debug, Clone)]
pub struct BloodEffectSystem {
    rng_state: u32,
    splash_emitters: Vec<BloodSplashEmitter>,
    pool_emitters: Vec<BloodPoolEmitter>,
    particles: Vec<BloodParticle>,
    stains: Vec<FloorStain>,
    pools: Vec<ExpandingPool>,
    footmarks: Vec<BloodFootmark>,
    /// `HH_Class_Blood_Stick_00` (`0x00257400`): remaining bloody footsteps (`0..=10`).
    bloody_steps_remaining: u8,
    /// `HH_Class_Blood_Stick_00` (`0x00257750`): elapsed seconds since last blood pool contact.
    stick_timer_sec: f32,
    /// Alternating foot side (`0` = right, `1` = left).
    next_foot_side: u8,
    /// Archived decals per room ID when `persistent_room_decals` is enabled.
    room_archives: HashMap<String, RoomBloodArchive>,
    /// Pending floor decals queued during `Blood01` floor collisions.
    pending_impacts: Vec<ImpactDescriptor>,
}

impl Default for BloodEffectSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl BloodEffectSystem {
    /// Creates an empty Impact Queue Blood System (`HH_Class_Object_Initialize`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            rng_state: 0x1999_0901,
            splash_emitters: Vec::new(),
            pool_emitters: Vec::new(),
            particles: Vec::with_capacity(MAX_BLOOD_DROPS),
            stains: Vec::with_capacity(MAX_BLOOD_STAINS),
            pools: Vec::with_capacity(MAX_BLOOD_POOLS),
            footmarks: Vec::with_capacity(MAX_BLOOD_FOOTMARKS),
            bloody_steps_remaining: 0,
            stick_timer_sec: 0.0,
            next_foot_side: 0,
            room_archives: HashMap::new(),
            pending_impacts: Vec::new(),
        }
    }

    /// Strict 1:1 PS2 `blood_clear()` (`HH_Class_Object_Initialize` in `connectRoomInit` `0x0020be10`).
    pub fn blood_clear(&mut self) {
        self.splash_emitters.clear();
        self.pool_emitters.clear();
        self.particles.clear();
        self.stains.clear();
        self.pools.clear();
        self.footmarks.clear();
        self.pending_impacts.clear();
        self.bloody_steps_remaining = 0;
        self.stick_timer_sec = 0.0;
    }

    /// Clears all active blood effects AND persistent room archives (e.g. New Game / Load Game).
    pub fn reset_all(&mut self) {
        self.blood_clear();
        self.room_archives.clear();
    }

    /// Handles a room transition:
    /// - When `persistent_room_decals == false`: calls strict 1:1 PS2 `blood_clear()`.
    /// - When `persistent_room_decals == true`: archives `prev_room` floor decals with
    ///   coagulation darkening and restores `next_room`'s decals.
    pub fn on_room_change(
        &mut self,
        prev_room: &str,
        next_room: &str,
        persistent_room_decals: bool,
    ) {
        if !persistent_room_decals {
            self.blood_clear();
            self.room_archives.clear();
            return;
        }

        if !prev_room.is_empty() {
            let mut stains = std::mem::take(&mut self.stains);
            let mut pools = std::mem::take(&mut self.pools);
            let mut footmarks = std::mem::take(&mut self.footmarks);
            for s in &mut stains {
                s.coagulation = (s.coagulation + 0.35).min(1.0);
            }
            for p in &mut pools {
                p.coagulation = (p.coagulation + 0.35).min(1.0);
                // Settle pools to full expansion when revisited
                p.elapsed_sec = p.elapsed_sec.max(16.0);
            }
            for f in &mut footmarks {
                f.coagulation = (f.coagulation + 0.35).min(1.0);
            }
            self.room_archives.insert(
                prev_room.to_string(),
                RoomBloodArchive {
                    stains,
                    pools,
                    footmarks,
                },
            );
        }

        // Clear transient airborne particles & emitters
        self.splash_emitters.clear();
        self.pool_emitters.clear();
        self.particles.clear();
        self.pending_impacts.clear();

        if let Some(archive) = self.room_archives.remove(next_room) {
            self.stains = archive.stains;
            self.pools = archive.pools;
            self.footmarks = archive.footmarks;
        } else {
            self.stains.clear();
            self.pools.clear();
            self.footmarks.clear();
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        next_lcg_u32(&mut self.rng_state)
    }

    #[inline]
    fn next_f32(&mut self) -> f32 {
        next_lcg_f32(&mut self.rng_state)
    }

    /// `enEfctBloodPool` (`0x00146bb0`) -> `HH_Effect_Object_Blood_Pool_Impact_Post` (`0x0025a780`).
    pub fn post_enemy_death_pool(&mut self, enemy_pos_mm: Vec3) {
        self.post_impact(ImpactDescriptor {
            kind: ImpactKind::BloodPoolEmitter,
            position_mm: Vec3::new(
                enemy_pos_mm.x,
                enemy_pos_mm.y + EN_EFCT_BLOOD_POOL_Y_OFFSET_MM,
                enemy_pos_mm.z,
            ),
            velocity_mm_s: Vec3::ZERO,
            yaw_rad: 0.0,
            mode: 0,
        });
    }

    /// `HH_Effect_Object_Blood_Splash_Impact_Post` (`0x0025a8d0`).
    pub fn post_hit_splash(&mut self, hit_pos_mm: Vec3, hit_dir: Vec3) {
        let dir = if hit_dir.length_squared() > 1e-6 {
            hit_dir.normalize()
        } else {
            Vec3::new(0.0, -0.3, 1.0).normalize()
        };
        self.post_impact(ImpactDescriptor {
            kind: ImpactKind::BloodSplashEmitter,
            position_mm: hit_pos_mm,
            velocity_mm_s: dir * 2000.0, // `sceVu0ScaleVectorXYZ(0x44fa0000 = 2000.0)`
            yaw_rad: 0.0,
            mode: 0,
        });
    }

    /// `HH_Effect_Object_Impact_Post` (`0x0026a1e0`).
    pub fn post_impact(&mut self, desc: ImpactDescriptor) {
        match desc.kind {
            ImpactKind::BloodPoolEmitter => {
                let row = (self.next_u32() as usize) % BLOOD_POOL_00_TIME_TABLE.len();
                self.pool_emitters.push(BloodPoolEmitter {
                    origin_mm: desc.position_mm,
                    elapsed_sec: 0.0,
                    table_row: row,
                    spawned_count: 0,
                });
            }
            ImpactKind::BloodSplashEmitter => {
                self.splash_emitters.push(BloodSplashEmitter {
                    origin_mm: desc.position_mm,
                    velocity_mm_s: desc.velocity_mm_s,
                    elapsed_sec: 0.0,
                    next_emit_sec: [0.0, 0.0],
                    emitted_count: [0, 0],
                });
            }
            ImpactKind::BloodParticle => {
                if self.particles.len() >= MAX_BLOOD_DROPS {
                    self.particles.remove(0);
                }
                let vx = desc.velocity_mm_s.x * BLOOD_01_VEL_XZ_SCALE;
                let vy = desc.velocity_mm_s.y * BLOOD_01_VEL_Y_SCALE;
                let vz = desc.velocity_mm_s.z * BLOOD_01_VEL_XZ_SCALE;
                let quadrant = (self.next_u32() & 3) as u8;
                let mode = desc.mode.min(1);
                self.particles.push(BloodParticle {
                    origin_mm: desc.position_mm,
                    vel_mm_s: Vec3::new(vx, vy, vz),
                    elapsed_sec: 0.0,
                    mode,
                    quadrant,
                    scale: BLOOD_01_SCALE_START[usize::from(mode)],
                    alpha_gs: BLOOD_01_ALPHA_START_GS[usize::from(mode)],
                    active: true,
                });
            }
            ImpactKind::FloorStain => {
                if self.stains.len() >= MAX_BLOOD_STAINS {
                    self.stains.remove(0);
                }
                let variant = (self.next_u32() as usize) % BLOOD_00_SCALE_END_MM.len();
                let yaw_rad = self.next_f32() * std::f32::consts::TAU - std::f32::consts::PI;
                self.stains.push(FloorStain {
                    position_mm: desc.position_mm,
                    yaw_rad,
                    variant,
                    elapsed_sec: 0.0,
                    fade_alpha: 1.0,
                    coagulation: 0.0,
                });
            }
            ImpactKind::ExpandingPool => {
                if self.pools.len() >= MAX_BLOOD_POOLS {
                    self.pools.remove(0);
                }
                let variant = (self.next_u32() as usize) % BLOOD_02_SCALE_END_MM.len();
                let uv_variant = (self.next_u32() as usize) % BLOOD_02_UV_RECTS.len();
                let yaw_rad = self.next_f32() * std::f32::consts::TAU - std::f32::consts::PI;
                self.pools.push(ExpandingPool {
                    position_mm: desc.position_mm,
                    yaw_rad,
                    variant,
                    uv_variant,
                    elapsed_sec: 0.0,
                    size_scale: SH2EE_BLOOD_SIZE_SCALE,
                    coagulation: 0.0,
                });
            }
            ImpactKind::Footmark => {
                if self.footmarks.len() >= MAX_BLOOD_FOOTMARKS {
                    self.footmarks.remove(0);
                }
                let variant = (self.next_u32() & 1) as u8;
                self.footmarks.push(BloodFootmark {
                    position_mm: desc.position_mm,
                    yaw_rad: desc.yaw_rad,
                    side: desc.mode & 1,
                    variant,
                    elapsed_sec: 0.0,
                    alpha_scale: 1.0,
                    coagulation: 0.0,
                });
            }
        }
    }

    /// Advances the Impact Queue blood simulation by `dt_sec`.
    pub fn step<F>(
        &mut self,
        dt_sec: f32,
        mut floor_y_at_xz: F,
        player_pos_mm: Vec3,
        player_yaw: f32,
        footstep_triggered: bool,
        config: BloodSystemConfig,
    ) where
        F: FnMut(Vec3) -> f32,
    {
        if dt_sec <= 0.0 {
            return;
        }

        // 1. Step Class 11 BloodSplashPhenomenon00 emitters
        let mut new_particles = Vec::new();
        let rng = &mut self.rng_state;
        for emitter in &mut self.splash_emitters {
            for ch in 0..2usize {
                while emitter.elapsed_sec >= emitter.next_emit_sec[ch]
                    && emitter.emitted_count[ch] < BLOOD_SPLASH_00_FULL_NUMBER[ch]
                {
                    let spread_x = (next_lcg_f32(rng) - 0.5) * 900.0;
                    let spread_z = (next_lcg_f32(rng) - 0.5) * 900.0;
                    let base_vel = if ch == 1 {
                        // `hh_class_blood_splash_phenomenon_00.c` line 224: `uStack_c = 0xc3fa0000` (-500.0)
                        Vec3::new(spread_x, -500.0, spread_z)
                    } else {
                        emitter.velocity_mm_s + Vec3::new(spread_x, -250.0, spread_z)
                    };
                    new_particles.push(ImpactDescriptor {
                        kind: ImpactKind::BloodParticle,
                        position_mm: emitter.origin_mm,
                        velocity_mm_s: base_vel,
                        yaw_rad: 0.0,
                        mode: ch as u8,
                    });
                    emitter.emitted_count[ch] += 1;
                    emitter.next_emit_sec[ch] += BLOOD_SPLASH_00_DISCHARGE_TIME[ch];
                }
            }
            emitter.elapsed_sec += dt_sec;
        }
        self.splash_emitters
            .retain(|e| e.elapsed_sec <= BLOOD_SPLASH_00_DURATION_SEC);

        // 2. Step Class 3 BloodPoolPhenomenon00 emitters -> Class 12 ExpandingPool
        let mut new_pools = Vec::new();
        for emitter in &mut self.pool_emitters {
            let times = BLOOD_POOL_00_TIME_TABLE[emitter.table_row];
            while emitter.spawned_count < 3 && emitter.elapsed_sec >= times[emitter.spawned_count] {
                let angle = next_lcg_f32(rng) * std::f32::consts::TAU;
                let radius =
                    BLOOD_POOL_00_MIN_OFFSET_MM + next_lcg_f32(rng) * BLOOD_POOL_00_OFFSET_RANGE_MM;
                let offset = Vec3::new(angle.sin() * radius, 0.0, angle.cos() * radius);
                new_pools.push(ImpactDescriptor {
                    kind: ImpactKind::ExpandingPool,
                    position_mm: emitter.origin_mm + offset,
                    velocity_mm_s: Vec3::ZERO,
                    yaw_rad: 0.0,
                    mode: 0,
                });
                emitter.spawned_count += 1;
            }
            emitter.elapsed_sec += dt_sec;
        }
        self.pool_emitters.retain(|e| e.spawned_count < 3);

        for p in new_particles {
            self.post_impact(p);
        }
        for p in new_pools {
            self.post_impact(p);
        }
        for p in &mut self.pools {
            p.size_scale = config.blood_size_scale;
            p.elapsed_sec += dt_sec;
        }

        // 3. Step Class 5 Blood01 parabolic particles -> Class 10 FloorStain on floor strike
        for p in &mut self.particles {
            if !p.active {
                continue;
            }
            let idx = usize::from(p.mode);
            let lifetime = BLOOD_01_LIFETIME_SEC[idx];
            let t_norm = (p.elapsed_sec / lifetime).clamp(0.0, 1.0);
            p.scale = BLOOD_01_SCALE_START[idx] * (1.0 - t_norm) + BLOOD_01_SCALE_END[idx] * t_norm;
            p.alpha_gs =
                BLOOD_01_ALPHA_START_GS[idx] * (1.0 - t_norm) + BLOOD_01_ALPHA_END_GS[idx] * t_norm;

            let world_pos = p.origin_mm
                + evaluate_blood_parabola(p.vel_mm_s, BLOOD_01_GRAVITY_MM_S2, p.elapsed_sec);

            if p.mode == 1 {
                let floor_y = floor_y_at_xz(world_pos);
                if world_pos.y >= floor_y && p.elapsed_sec > 0.02 {
                    self.pending_impacts.push(ImpactDescriptor {
                        kind: ImpactKind::FloorStain,
                        position_mm: Vec3::new(world_pos.x, floor_y, world_pos.z),
                        velocity_mm_s: Vec3::ZERO,
                        yaw_rad: 0.0,
                        mode: 0,
                    });
                    p.active = false;
                    continue;
                }
            } else if p.elapsed_sec >= lifetime {
                p.active = false;
                continue;
            }

            p.elapsed_sec += dt_sec;
        }
        self.particles.retain(|p| p.active);

        let pending = std::mem::take(&mut self.pending_impacts);
        for desc in pending {
            self.post_impact(desc);
        }

        // 4. Step Class 10 FloorStain
        for s in &mut self.stains {
            s.elapsed_sec += dt_sec;
            if !config.persistent_room_decals && s.elapsed_sec > BLOOD_00_HOLD_DURATION_SEC {
                s.fade_alpha = (s.fade_alpha - dt_sec * 0.5).max(0.0);
            }
        }
        if !config.persistent_room_decals {
            self.stains.retain(|s| s.fade_alpha > 0.0);
        }

        // 5. BloodStick00: check if James stepped inside any active Blood02 pool
        let stepped_in_pool = self
            .pools
            .iter()
            .any(|pool| pool.contains_xz(player_pos_mm));
        if stepped_in_pool {
            self.bloody_steps_remaining = MAX_BLOODY_STEPS;
            self.stick_timer_sec = 0.0;
        } else if self.bloody_steps_remaining > 0 {
            self.stick_timer_sec += dt_sec;
            if self.stick_timer_sec > BLOOD_STICK_MAX_LIFETIME_SEC {
                self.bloody_steps_remaining = 0;
            }
        }

        // 6. Hook James's locomotion heel strikes -> Class 9 BloodFootMark00
        if footstep_triggered && self.bloody_steps_remaining > 0 && !stepped_in_pool {
            self.bloody_steps_remaining -= 1;
            let side = self.next_foot_side & 1;
            self.next_foot_side = side ^ 1;
            // Lateral stance offset perpendicular to facing yaw
            let sign = if side == 0 { 1.0 } else { -1.0 };
            let right_vec = Vec3::new(player_yaw.cos(), 0.0, -player_yaw.sin());
            let foot_xz = player_pos_mm + right_vec * (sign * BLOOD_FOOTMARK_STANCE_OFFSET_MM);
            let floor_y = floor_y_at_xz(foot_xz);
            self.post_impact(ImpactDescriptor {
                kind: ImpactKind::Footmark,
                position_mm: Vec3::new(foot_xz.x, floor_y, foot_xz.z),
                velocity_mm_s: Vec3::ZERO,
                yaw_rad: player_yaw,
                mode: side,
            });
        }

        // 7. Step Class 9 BloodFootMark00
        let fade_rate = BLOOD_FOOTMARK_FADE_PER_30HZ / BLOOD_DT_30HZ;
        for f in &mut self.footmarks {
            f.elapsed_sec += dt_sec;
            if !config.persistent_room_decals && f.elapsed_sec > BLOOD_FOOTMARK_HOLD_SEC {
                f.alpha_scale = (f.alpha_scale - fade_rate * dt_sec).max(0.0);
            }
        }
        if !config.persistent_room_decals {
            self.footmarks.retain(|f| f.alpha_scale > 0.0);
        }
    }

    /// Returns the number of bloody footsteps currently remaining on James's shoes (`0..=10`).
    #[must_use]
    pub fn bloody_steps_remaining(&self) -> u8 {
        self.bloody_steps_remaining
    }

    /// Returns the active `(particles, stains, pools, footmarks)` counts for diagnostics/tests.
    #[must_use]
    pub fn active_counts(&self) -> (usize, usize, usize, usize) {
        (
            self.particles.len(),
            self.stains.len(),
            self.pools.len(),
            self.footmarks.len(),
        )
    }

    /// Builds GPU render items for `sh2-renderer`'s [`BloodEffectPipeline`].
    pub fn build_render_items(
        &self,
        config: BloodSystemConfig,
        out: &mut Vec<BloodQuadRenderItem>,
    ) {
        out.clear();
        let subtractive = !config.non_subtractive_fallback;
        let base_rgb = if subtractive {
            blood_color_subtractive_rgb_norm(config.blood_color)
        } else {
            let rgba = blood_color_non_subtractive_rgba_norm(config.blood_color);
            [rgba[0], rgba[1], rgba[2]]
        };

        // 1. Expanding death pools (Class 12)
        for p in &self.pools {
            let (_, a_t) = evaluate_three_stage_envelope(p.elapsed_sec, &BLOOD_02_MOTION_INFO);
            let [hx, hz] = p.current_half_extents();
            if hx <= 0.5 || hz <= 0.5 {
                continue;
            }
            // Coagulation darkens/intensifies subtractive absorption slightly
            let coag_mul = 1.0 - 0.15 * p.coagulation;
            let rgb = [
                base_rgb[0] * coag_mul,
                base_rgb[1] * coag_mul,
                base_rgb[2] * coag_mul,
            ];
            let alpha = (0.35 + 0.65 * a_t).clamp(0.0, 1.0);
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::FloorDecal,
                position_mm: p.position_mm,
                half_extents_mm: [hx, hz],
                yaw_rad: p.yaw_rad,
                uv_rect: BLOOD_02_UV_RECTS[p.uv_variant % BLOOD_02_UV_RECTS.len()],
                color_rgb: rgb,
                alpha,
                subtractive,
            });
        }

        // 2. Floor blood stains (Class 10)
        for s in &self.stains {
            let (s_t, _) = evaluate_three_stage_envelope(s.elapsed_sec, &BLOOD_00_MOTION_INFO);
            let end = BLOOD_00_SCALE_END_MM[s.variant % BLOOD_00_SCALE_END_MM.len()];
            let hx = end[0] * s_t * config.blood_size_scale;
            let hz = end[1] * s_t * config.blood_size_scale;
            if hx <= 0.5 || hz <= 0.5 {
                continue;
            }
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::FloorDecal,
                position_mm: s.position_mm,
                half_extents_mm: [hx, hz],
                yaw_rad: s.yaw_rad,
                uv_rect: BLOOD_00_UV_RECT,
                color_rgb: base_rgb,
                alpha: s.fade_alpha.clamp(0.0, 1.0),
                subtractive,
            });
        }

        // 3. Bloody footprints (Class 9)
        for f in &self.footmarks {
            if f.alpha_scale <= 0.004 {
                continue;
            }
            let side_idx = usize::from(f.side & 1);
            let var_idx = usize::from(f.variant & 1);
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::FloorDecal,
                position_mm: f.position_mm,
                half_extents_mm: [BLOOD_FOOTMARK_HALF_WIDTH_MM, BLOOD_FOOTMARK_HALF_LENGTH_MM],
                yaw_rad: f.yaw_rad,
                uv_rect: BLOOD_FOOTMARK_UV_RECTS[side_idx][var_idx],
                color_rgb: base_rgb,
                alpha: (BLOOD_FOOTMARK_BASE_ALPHA_GS / 128.0 * f.alpha_scale).clamp(0.0, 1.0),
                subtractive,
            });
        }

        // 4. Airborne hit spray & falling drops (Class 5)
        for p in &self.particles {
            let world_pos = p.origin_mm
                + evaluate_blood_parabola(p.vel_mm_s, BLOOD_01_GRAVITY_MM_S2, p.elapsed_sec);
            let r = BLOOD_01_HALF_EXTENT_MM * p.scale;
            let uv_rect = if p.mode == 0 {
                BLOOD_01_MIST_UV_RECTS[usize::from(p.quadrant & 3)]
            } else {
                BLOOD_01_DROP_UV_RECTS[usize::from(p.quadrant & 3)]
            };
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::Billboard,
                position_mm: world_pos,
                half_extents_mm: [r, r],
                yaw_rad: 0.0,
                uv_rect,
                color_rgb: base_rgb,
                alpha: (p.alpha_gs / 128.0).clamp(0.0, 1.0),
                subtractive,
            });
        }
    }
}
