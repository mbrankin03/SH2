//! `BloodEffectSystem` simulation state, Impact Queue, and Persistent Room Decals.

use std::collections::HashMap;

use glam::Vec3;

use super::tables::{
    BLOOD_00_HOLD_DURATION_SEC, BLOOD_00_SCALE_END_MM, BLOOD_01_ALPHA_END_GS,
    BLOOD_01_ALPHA_START_GS, BLOOD_01_GRAVITY_MM_S2, BLOOD_01_LIFETIME_SEC, BLOOD_01_SCALE_END,
    BLOOD_01_SCALE_START, BLOOD_01_VEL_XZ_SCALE, BLOOD_01_VEL_Y_SCALE, BLOOD_02_COLLISION_SCALE_SQ,
    BLOOD_02_MOTION_INFO, BLOOD_02_SCALE_END_MM, BLOOD_02_UV_RECTS, BLOOD_DT_30HZ,
    BLOOD_FOOTMARK_FADE_PER_30HZ, BLOOD_FOOTMARK_HOLD_SEC, BLOOD_FOOTMARK_STANCE_OFFSET_MM,
    BLOOD_POOL_00_MIN_OFFSET_MM, BLOOD_POOL_00_OFFSET_RANGE_MM, BLOOD_POOL_00_TIME_TABLE,
    BLOOD_SPLASH_00_DISCHARGE_TIME, BLOOD_SPLASH_00_DURATION_SEC, BLOOD_SPLASH_00_FULL_NUMBER,
    BLOOD_STICK_MAX_LIFETIME_SEC, EN_EFCT_BLOOD_POOL_Y_OFFSET_MM, MAX_BLOODY_STEPS,
    SH2EE_BLOOD_SIZE_SCALE,
};
use super::types::{
    evaluate_blood_parabola, evaluate_three_stage_envelope, BloodSystemConfig, ImpactDescriptor,
    ImpactKind, MAX_BLOOD_DROPS, MAX_BLOOD_FOOTMARKS, MAX_BLOOD_POOLS, MAX_BLOOD_STAINS,
};

#[inline]
fn next_lcg_u32(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

#[inline]
fn next_lcg_f32(state: &mut u32) -> f32 {
    (next_lcg_u32(state) >> 8) as f32 / (1u32 << 24) as f32
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
pub(super) struct BloodParticle {
    pub(super) origin_mm: Vec3,
    pub(super) vel_mm_s: Vec3,
    pub(super) elapsed_sec: f32,
    pub(super) mode: u8,
    pub(super) quadrant: u8,
    pub(super) scale: f32,
    pub(super) alpha_gs: f32,
    pub(super) active: bool,
}

#[derive(Debug, Clone)]
pub(super) struct FloorStain {
    pub(super) position_mm: Vec3,
    pub(super) yaw_rad: f32,
    pub(super) variant: usize,
    pub(super) elapsed_sec: f32,
    pub(super) fade_alpha: f32,
    pub(super) coagulation: f32,
}

#[derive(Debug, Clone)]
pub(super) struct ExpandingPool {
    pub(super) position_mm: Vec3,
    pub(super) yaw_rad: f32,
    pub(super) variant: usize,
    pub(super) uv_variant: usize,
    pub(super) elapsed_sec: f32,
    pub(super) size_scale: f32,
    pub(super) coagulation: f32,
}

impl ExpandingPool {
    pub(super) fn current_half_extents(&self) -> [f32; 2] {
        let (s_t, _) = evaluate_three_stage_envelope(self.elapsed_sec, &BLOOD_02_MOTION_INFO);
        let end = BLOOD_02_SCALE_END_MM[self.variant % BLOOD_02_SCALE_END_MM.len()];
        [
            end[0] * s_t * self.size_scale,
            end[1] * s_t * self.size_scale,
        ]
    }

    fn contains_xz(&self, pos_mm: Vec3) -> bool {
        let [hx, hz] = self.current_half_extents();
        let min_r = hx.min(hz);
        if min_r <= 1.0 {
            return false;
        }
        let dx = pos_mm.x - self.position_mm.x;
        let dz = pos_mm.z - self.position_mm.z;
        (dx * dx + dz * dz) < (min_r * min_r) * BLOOD_02_COLLISION_SCALE_SQ
    }
}

#[derive(Debug, Clone)]
pub(super) struct BloodFootmark {
    pub(super) position_mm: Vec3,
    pub(super) yaw_rad: f32,
    pub(super) side: u8,
    pub(super) variant: u8,
    pub(super) elapsed_sec: f32,
    pub(super) alpha_scale: f32,
    pub(super) coagulation: f32,
}

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
    pub(super) particles: Vec<BloodParticle>,
    pub(super) stains: Vec<FloorStain>,
    pub(super) pools: Vec<ExpandingPool>,
    pub(super) footmarks: Vec<BloodFootmark>,
    bloody_steps_remaining: u8,
    stick_timer_sec: f32,
    next_foot_side: u8,
    room_archives: HashMap<String, RoomBloodArchive>,
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
            velocity_mm_s: dir * 2000.0,
            yaw_rad: 0.0,
            mode: 0,
        });
    }

    /// `HH_Effect_Object_Impact_Post` (`0x0026a1e0`).
    pub fn post_impact(&mut self, desc: ImpactDescriptor) {
        match desc.kind {
            ImpactKind::BloodPoolEmitter => {
                let row =
                    (next_lcg_u32(&mut self.rng_state) as usize) % BLOOD_POOL_00_TIME_TABLE.len();
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
                let quadrant = (next_lcg_u32(&mut self.rng_state) & 3) as u8;
                let mode = desc.mode.min(1);
                self.particles.push(BloodParticle {
                    origin_mm: desc.position_mm,
                    vel_mm_s: Vec3::new(
                        desc.velocity_mm_s.x * BLOOD_01_VEL_XZ_SCALE,
                        desc.velocity_mm_s.y * BLOOD_01_VEL_Y_SCALE,
                        desc.velocity_mm_s.z * BLOOD_01_VEL_XZ_SCALE,
                    ),
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
                let variant =
                    (next_lcg_u32(&mut self.rng_state) as usize) % BLOOD_00_SCALE_END_MM.len();
                let yaw_rad = next_lcg_f32(&mut self.rng_state) * std::f32::consts::TAU
                    - std::f32::consts::PI;
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
                let variant =
                    (next_lcg_u32(&mut self.rng_state) as usize) % BLOOD_02_SCALE_END_MM.len();
                let uv_variant =
                    (next_lcg_u32(&mut self.rng_state) as usize) % BLOOD_02_UV_RECTS.len();
                let yaw_rad = next_lcg_f32(&mut self.rng_state) * std::f32::consts::TAU
                    - std::f32::consts::PI;
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
                let variant = (next_lcg_u32(&mut self.rng_state) & 1) as u8;
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

        let mut new_pools = Vec::new();
        for emitter in &mut self.pool_emitters {
            let times = BLOOD_POOL_00_TIME_TABLE[emitter.table_row];
            while emitter.spawned_count < 3 && emitter.elapsed_sec >= times[emitter.spawned_count] {
                let angle = next_lcg_f32(rng) * std::f32::consts::TAU;
                let radius =
                    BLOOD_POOL_00_MIN_OFFSET_MM + next_lcg_f32(rng) * BLOOD_POOL_00_OFFSET_RANGE_MM;
                new_pools.push(ImpactDescriptor {
                    kind: ImpactKind::ExpandingPool,
                    position_mm: emitter.origin_mm
                        + Vec3::new(angle.sin() * radius, 0.0, angle.cos() * radius),
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

        for p in &mut self.particles {
            if !p.active {
                continue;
            }
            let idx = usize::from(p.mode);
            let t_norm = (p.elapsed_sec / BLOOD_01_LIFETIME_SEC[idx]).clamp(0.0, 1.0);
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
            } else if p.elapsed_sec >= BLOOD_01_LIFETIME_SEC[idx] {
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

        for s in &mut self.stains {
            s.elapsed_sec += dt_sec;
            if !config.persistent_room_decals && s.elapsed_sec > BLOOD_00_HOLD_DURATION_SEC {
                s.fade_alpha = (s.fade_alpha - dt_sec * 0.5).max(0.0);
            }
        }
        if !config.persistent_room_decals {
            self.stains.retain(|s| s.fade_alpha > 0.0);
        }

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

        if footstep_triggered && self.bloody_steps_remaining > 0 && !stepped_in_pool {
            self.bloody_steps_remaining -= 1;
            let side = self.next_foot_side & 1;
            self.next_foot_side = side ^ 1;
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
}
