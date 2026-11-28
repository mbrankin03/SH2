//! 1:1 PS2/PC localized 3D proximity fog particle simulation (`SH2-32`).
//!
//! # Ground Truth Citations (`SLUS_202.28` & `sh2pc.exe`)
//! - `fogSetEnvironment` (`SLUS_202.28` `0x00168dd0`, `Fog/fog.c:213-327`) & `FUN_00488f40` (`sh2pc.exe` `0x00488f40`)
//! - `fogInitWind` (`0x00169330`) & `fogChangeWind` (`0x00169380` / `FUN_00487c40` `0x00487c40`)
//! - `fogInitParticle` (`0x00169d90`) & `fog_init_part_sub` (`0x00169e30` / `FUN_004858a0` `0x004858a0`)
//! - `fogMoveParticle` (`0x0016a540` / `FUN_00489100` `0x00489100`)
//! - `fog_part_grid` (`0x0016b230` / `FUN_00486260` `0x00486260`): 8×8×8 density-gradient repulsion
//!   scaled by `grid_repulsion / part_num` + `vel *= 0.9` damping
//! - `fog_part_obj` (`0x0016b780` / `FUN_004868c0` `0x004868c0`): radial parting around James
//! - `fog_part_alp` (`0x0016b9a0` / `FUN_00486c30` `0x00486c30`): spring-damped alpha with `shSway1f`
//! - `fog_part_clamp` (`0x0016ba50` / `FUN_00486d10` `0x00486d10`): toroidal wrap with X/Z flip & `300.0 mm` border fades
//! - SH2:EE `RunFogSpeed` (`Patches/FogAdjustments.cpp:238-306`, `sh2pc.exe` `0x0048683D`):
//!   replaces ONLY `FogEnv +0x14` (`grid_repulsion_f14`) inside `fog_part_grid`; NEVER scales `dt_scale` or wind.

use crate::cutscene::{OBSERVATION_DECK_FOG_BORDER_WIDTH, OBSERVATION_DECK_FOG_PARTICLE_MAX};

use super::constants::{
    PS2_DEFAULT_FOG_ENV_ALPHA_U8, PS2_DEFAULT_FOG_ENV_SUBDIV, PS2_DEFAULT_FOG_HEIGHT_BAND_MM,
    PS2_DEFAULT_FOG_PARTICLE_COUNT, PS2_DEFAULT_FOG_PARTICLE_SIZE_MM,
    PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM, PS2_FOG_BORDER_FADE_MM,
};
use super::wind::{FogWindState, Ps2FogRng, FOG_PARTICLE_VELOCITY_DAMPING};

/// Exact 8×8×8 density grid cell count (`0x200 = 512`, `fogInitWind` `0x00169330`, `fog_part_grid` `0x0016b230`).
const FOG_GRID_CELLS: usize = 512;

/// Inverse of `PS2_FOG_BORDER_FADE_MM` (`1.0 / 300.0 = 0.0033333334`, `FUN_00486d10` `0x00486d10`).
const INV_FOG_BORDER_FADE_MM: f32 = 1.0 / PS2_FOG_BORDER_FADE_MM;

/// Maximum PS2 particle raw alpha (`127.0`, `FUN_00486c30` `sh2pc_00480000.c:4123,4135-4136`).
const FOG_ALPHA_RAW_MAX: f32 = 127.0;

/// Spring stiffness in `fog_part_alp` (`0.01`, `FUN_00486c30` `sh2pc_00480000.c:4127`).
const FOG_ALPHA_SPRING_K: f32 = 0.01;

/// Alpha velocity damping in `fog_part_alp` (`0.9`, `FUN_00486c30` `sh2pc_00480000.c:4127`).
const FOG_ALPHA_VEL_DAMPING: f32 = 0.9;

/// `shSway1f` bias in `fog_part_alp` (`0x3ca3d70a = 0.02`, `FUN_00486c30` `sh2pc_00480000.c:4125`).
const FOG_ALPHA_SWAY_BIAS: f32 = 0.02;

/// `shSway1f` amplitude scale in `fog_part_alp` (`-1.0 / 32.0 = -0.03125`, `fogSetEnvironment`
/// `Fog/fog.c:307`, `FUN_00486c30` `sh2pc_00480000.c:4125`).
const FOG_ALPHA_SWAY_SCALE: f32 = -0.031_25;

/// Outgoing velocity damping factor in `fog_part_obj` (`0.1`, `FUN_004868c0` `sh2pc_00480000.c:4016`).
const FOG_OBJ_OUTGOING_DAMPING: f32 = 0.1;

/// Anchor teleport distance threshold triggering re-init (`30000.0 mm`, `fogMoveParticle` `Fog/fog.c:1081`).
const FOG_TELEPORT_REINIT_DIST_MM: f32 = 30_000.0;

/// Active `.fcl` `FogEnv` environment parameters (`fogSetEnvironment` `0x00168dd0`,
/// `fog_defset_data` @ `0x002ab530` in `SLUS_202.28`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogEnvParams {
    /// Particle count (`+0x00`, default `500` from `fog_defset_data`).
    pub part_num: usize,
    /// Horizontal X volume half-extent in PS2 mm (`+0x02`, or `24500.0` for SH2:EE Observation Deck).
    pub range_x_mm: f32,
    /// Horizontal Z volume half-extent / canonical `.fcl` `area` in PS2 mm (`+0x02`, default `2000.0`).
    pub range_z_mm: f32,
    /// Billboard half-size/radius in PS2 mm (`+0x04`, default `300.0`).
    pub size_mm: f32,
    /// Wall/object soft boundary thickness in PS2 mm (`+0x06`, default `100.0`).
    pub soft_thickness_mm: f32,
    /// Fixed floor/anchor Y in PS2 mm (`+0x08`, default `0.0`).
    pub y_fix_mm: f32,
    /// Altitude fade upper Y threshold in PS2 mm when `flags & 0x01 != 0` (`+0x0a`).
    pub alt_fade_y_mm: f32,
    /// Altitude fade span in PS2 mm when `flags & 0x01 != 0` (`+0x0c`).
    pub alt_fade_span_mm: f32,
    /// Y threshold above which target alpha boosts to `127.0` (`+0x0e`, default `10000.0`).
    pub alpha_boost_y_mm: f32,
    /// Wind preset type `0..=0x12` (`+0x10`, default `0`).
    pub wind_type: u8,
    /// Environment bitflags (`+0x11`, default `0`).
    pub flags: u8,
    /// Vertical subdivision shift (`+0x12`, default `1`).
    pub subdiv: u8,
    /// Base particle alpha byte (`+0x13`, default `0x10 = 16`).
    pub alpha_u8: u8,
    /// `fog_part_grid` density-gradient repulsion strength (`+0x14`, default `2.0`).
    pub grid_repulsion: f32,
}

impl Default for FogEnvParams {
    fn default() -> Self {
        Self {
            part_num: PS2_DEFAULT_FOG_PARTICLE_COUNT,
            range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
            range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
            size_mm: PS2_DEFAULT_FOG_PARTICLE_SIZE_MM,
            soft_thickness_mm: PS2_DEFAULT_FOG_HEIGHT_BAND_MM,
            y_fix_mm: 0.0,
            alt_fade_y_mm: 0.0,
            alt_fade_span_mm: 0.0,
            alpha_boost_y_mm: 10_000.0,
            wind_type: 0,
            flags: 0,
            subdiv: PS2_DEFAULT_FOG_ENV_SUBDIV,
            alpha_u8: PS2_DEFAULT_FOG_ENV_ALPHA_U8,
            grid_repulsion: 2.0,
        }
    }
}

/// Single localized 3D proximity fog particle (`fwork` 96-byte `0x60` entry in `SLUS_202.28`
/// `fogInitParticle` `0x00169d90`, `fogMoveParticle` `0x0016a540`, `sh2pc.exe` `FUN_004858a0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogParticle {
    /// Local offset relative to `anchor_world` in PS2 mm `(x, y, z)`.
    pub local_pos: [f32; 3],
    /// Particle velocity in PS2 mm per 60 Hz frame `(vx, vy, vz)` (`unaff_ESI[4..7]`).
    pub velocity: [f32; 3],
    /// Per-particle grid-gradient sample jitter in `[-0.5, +0.5)` (`unaff_ESI[8..11]`).
    pub grid_jitter: [f32; 3],
    /// Billboard quad half-size in PS2 mm (`FogEnv +0x04`).
    pub radius_mm: f32,
    /// Unclamped spring-damped alpha in `0.0..=127.0` (`unaff_ESI[0x14]`, `fog_part_alp`).
    pub alpha_raw: f32,
    /// Alpha spring velocity (`unaff_ESI[0x15]`, `fog_part_alp`).
    pub alpha_vel: f32,
    /// Evaluated normalized border-faded alpha in `0.0..=1.0` (`clamped_alpha / base_alpha`).
    pub alpha: f32,
    /// Collision hit counter (`unaff_ESI[0x16]`, `fog_part_obj`).
    pub hit_count: i32,
    /// Fade-out flag (`unaff_ESI[0x17]`, `fogMoveParticle`).
    pub fading_out: bool,
}

/// Wraps a coordinate into `[-half_extent, +half_extent]`, returning `(wrapped_value, odd_wrap_count)`
/// so X/Z wraps can flip the orthogonal axis (`fog_part_clamp` `0x0016ba50` / `FUN_00486d10` `0x00486d10`).
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

/// Wraps a vertical local Y coordinate according to `subdiv` (`FUN_00486d10` `sh2pc_00480000.c:4172-4205`).
/// Returns `(wrapped_y, reset_alpha)` where `reset_alpha` is `true` on upper-bound wrap (`> 0.0` or `> area`).
#[inline]
fn wrap_vertical_ps2(val: f32, area_mm: f32, subdiv: u8) -> (f32, bool) {
    if subdiv == 0 {
        if val < -area_mm {
            let span = 2.0 * area_mm;
            let shifted = val + area_mm;
            (shifted.rem_euclid(span) - area_mm, false)
        } else if val > area_mm {
            let span = 2.0 * area_mm;
            let shifted = val + area_mm;
            (shifted.rem_euclid(span) - area_mm, true)
        } else {
            (val, false)
        }
    } else {
        let y_span = (2.0 * area_mm) / ((1u32 << (subdiv & 0x1f)) as f32);
        if val < -y_span {
            let shifted = val + y_span;
            (shifted.rem_euclid(y_span) - y_span, false)
        } else if val > 0.0 {
            let shifted = val + y_span;
            (shifted.rem_euclid(y_span) - y_span, true)
        } else {
            (val, false)
        }
    }
}

/// Deterministic 1:1 PS2/PC localized 3D proximity fog particle volume.
///
/// Mirrors `fogInitParticle` (`0x00169d90`), `fogMoveParticle` (`0x0016a540`),
/// `fogChangeWind` (`0x00169380`), `fog_part_grid` (`0x0016b230` / `FUN_00486260`),
/// `fog_part_obj` (`0x0016b780` / `FUN_004868c0`), `fog_part_alp` (`0x0016b9a0` / `FUN_00486c30`),
/// and `fog_part_clamp` (`0x0016ba50` / `FUN_00486d10`).
#[derive(Debug, Clone)]
pub struct FogParticleVolume {
    particles: Vec<FogParticle>,
    anchor_world: [f32; 3],
    prev_player_world: Option<[f32; 3]>,
    env: FogEnvParams,
    wind: FogWindState,
    rng: Ps2FogRng,
    /// Previous frame 8×8×8 density grid (`DAT_01f60f20` / `0x0057bd50`, initialized to `1.0`).
    grid_prev: [f32; FOG_GRID_CELLS],
    /// Current frame 8×8×8 density grid (`DAT_01f61720` / `0x70000800`).
    grid_curr: [f32; FOG_GRID_CELLS],
    /// Smoothly integrated world-space UV drift `[drift_u, drift_v]` in `[0.0, 1.0)` for
    /// `ModernVolumetric` fog, accumulated per frame at `wind.xz * (dt_scale * 0.005 * 0.25)`
    /// (`fog_view_screen_fog` `0x0016c1c0`) so `fogChangeWind` rotations never snap UVs.
    volumetric_wind_drift: [f32; 2],
}

impl FogParticleVolume {
    /// Creates a deterministic particle volume populated with `count` sprites inside
    /// `[-range_x_mm, +range_x_mm] × y_band × [-range_z_mm, +range_z_mm]`.
    #[must_use]
    pub fn new(count: usize, range_x_mm: f32, range_z_mm: f32) -> Self {
        let clamped_count = count.min(OBSERVATION_DECK_FOG_PARTICLE_MAX);
        let rx = range_x_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        let rz = range_z_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);

        let env = FogEnvParams {
            part_num: clamped_count,
            range_x_mm: rx,
            range_z_mm: rz,
            ..FogEnvParams::default()
        };

        let mut rng = Ps2FogRng::new(0x51A2_F061);
        let mut wind = FogWindState::default();
        wind.init_wind(env.wind_type, &mut rng);

        let mut vol = Self {
            particles: Vec::with_capacity(clamped_count),
            anchor_world: [0.0, 0.0, 0.0],
            prev_player_world: None,
            env,
            wind,
            rng,
            grid_prev: [1.0; FOG_GRID_CELLS],
            grid_curr: [0.0; FOG_GRID_CELLS],
            volumetric_wind_drift: [0.0, 0.0],
        };
        vol.init_particles();
        vol
    }

    /// Initializes all `env.part_num` particles (`fog_init_part_sub` `0x00169e30` / `FUN_004858a0` `0x004858a0`).
    fn init_particles(&mut self) {
        self.particles.clear();
        let count = self.env.part_num.min(OBSERVATION_DECK_FOG_PARTICLE_MAX);
        let rx = self.env.range_x_mm;
        let rz = self.env.range_z_mm;
        let base_alpha = f32::from(self.env.alpha_u8);
        let y_span = if self.env.subdiv == 0 {
            rz
        } else {
            (2.0 * rz) / ((1u32 << (self.env.subdiv & 0x1f)) as f32)
        };

        for _ in 0..count {
            let pos_rand = self.rng.rand_v_scale(1.0);
            let x = pos_rand[0] * rx;
            let y = if self.env.subdiv == 0 {
                pos_rand[1] * rz
            } else {
                // `FUN_004858a0:3290-3291`: negative `|y|` in `[-y_span, 0.0]`
                -pos_rand[1].abs() * y_span
            };
            let z = pos_rand[2] * rz;
            let vel = self.rng.rand_v_scale(1.0);
            let jitter = self.rng.rand_v_scale(0.5);
            self.particles.push(FogParticle {
                local_pos: [x, y, z],
                velocity: vel,
                grid_jitter: jitter,
                radius_mm: self.env.size_mm,
                alpha_raw: base_alpha,
                alpha_vel: 0.0,
                alpha: if base_alpha > 0.0 { 1.0 } else { 0.0 },
                hit_count: 0,
                fading_out: false,
            });
        }
    }

    /// Reconfigures particle count and volume extents when switching rooms.
    pub fn reconfigure(&mut self, count: usize, range_x_mm: f32, range_z_mm: f32) {
        let clamped_count = count.min(OBSERVATION_DECK_FOG_PARTICLE_MAX);
        let rx = range_x_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        let rz = range_z_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        if self.particles.len() != clamped_count
            || (self.env.range_x_mm - rx).abs() > 1.0
            || (self.env.range_z_mm - rz).abs() > 1.0
        {
            self.env.part_num = clamped_count;
            self.env.range_x_mm = rx;
            self.env.range_z_mm = rz;
            self.init_particles();
        }
    }

    /// Applies an active `.fcl` `FogEnvParams` preset (`fogSetEnvironment` `0x00168dd0`,
    /// `FUN_00488f40` `0x00488f40`).
    ///
    /// Automatically triggers `fogChangeWind` when `wind_type` changes (or `fogInitWind` when
    /// `reset_wind_on_type_change` is `true`, e.g., Observation Deck stage 5), updates particle
    /// count/size/repulsion/alpha/subdiv, and re-initializes particles if `area` expands by > 2000 mm
    /// (`Fog/fog.c:240,322-324`).
    pub fn set_environment(&mut self, mut next: FogEnvParams, reset_wind_on_type_change: bool) {
        next.part_num = next.part_num.min(OBSERVATION_DECK_FOG_PARTICLE_MAX);
        next.range_x_mm = next.range_x_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);
        next.range_z_mm = next.range_z_mm.max(OBSERVATION_DECK_FOG_BORDER_WIDTH + 1.0);

        let area_expanded = next.range_z_mm > self.env.range_z_mm + 2000.0;
        let wind_changed = next.wind_type != self.env.wind_type;
        let count_or_extent_changed = next.part_num != self.particles.len()
            || (next.range_x_mm - self.env.range_x_mm).abs() > 1.0
            || (next.range_z_mm - self.env.range_z_mm).abs() > 1.0
            || next.subdiv != self.env.subdiv;

        self.env = next;

        if wind_changed {
            if reset_wind_on_type_change {
                self.wind.init_wind(next.wind_type, &mut self.rng);
                self.grid_prev.fill(1.0);
            } else {
                self.wind.change_wind(next.wind_type, &mut self.rng);
            }
            for p in &mut self.particles {
                p.grid_jitter = self.rng.rand_v_scale(0.5);
            }
        }

        if area_expanded || count_or_extent_changed {
            self.init_particles();
        } else {
            for p in &mut self.particles {
                p.radius_mm = next.size_mm;
            }
        }
    }

    /// Returns the active [`FogEnvParams`].
    #[must_use]
    pub const fn env(&self) -> &FogEnvParams {
        &self.env
    }

    /// Returns the live PS2 wind acceleration vector in mm/frame² (`_DAT_005814b0` / `_DAT_01f7a620`,
    /// unscaled by `FogSpeedFix`).
    #[must_use]
    pub const fn wind_vec(&self) -> [f32; 3] {
        self.wind.wind_vec
    }

    /// Returns the smoothly integrated `[drift_u, drift_v]` UV offset in `[0.0, 1.0)` for
    /// `ModernVolumetric` fog, accumulated per frame from the live unscaled wind vector.
    #[must_use]
    pub const fn volumetric_wind_drift(&self) -> [f32; 2] {
        self.volumetric_wind_drift
    }

    /// Returns the live [`FogWindState`] (for inspection and deterministic unit testing).
    #[must_use]
    pub const fn wind_state(&self) -> &FogWindState {
        &self.wind
    }

    /// Explicitly sets the wind acceleration vector in PS2 mm/frame² (used by deterministic tests).
    pub fn set_wind_accel_per_frame(&mut self, wind_mm_per_frame2: [f32; 3]) {
        self.wind.wind_vec = wind_mm_per_frame2;
    }

    /// Returns the immutable slice of localized particles.
    #[must_use]
    pub fn particles(&self) -> &[FogParticle] {
        &self.particles
    }

    /// Returns the current world-space anchor `(x, y, z)` in PS2 mm.
    #[must_use]
    pub const fn anchor_world(&self) -> [f32; 3] {
        self.anchor_world
    }

    /// Backwards-compatible step wrapper that maps a non-unit `speed_mult` (`0.25` / `0.50`) to
    /// `grid_repulsion_override` ONLY — never scaling `dt_scale` or wind velocity (`FogAdjustments.cpp:238-306`).
    pub fn step(
        &mut self,
        dt_secs: f32,
        speed_mult: f32,
        anchor_world: [f32; 3],
        player_world_pos: [f32; 3],
        james_fog_influence_mm: f32,
    ) {
        let grid_repulsion_override = if (speed_mult - 1.0).abs() > 1e-4 {
            Some(speed_mult)
        } else {
            None
        };
        self.step_ps2(
            dt_secs,
            grid_repulsion_override,
            anchor_world,
            player_world_pos,
            james_fog_influence_mm,
        );
    }

    /// Exact 1:1 PS2/PC `fogMoveParticle` (`SLUS_202.28` `0x0016a540`, `sh2pc.exe` `FUN_00489100` `0x00489100`).
    ///
    /// - `dt_secs`: frame delta time in seconds (`dt_scale = dt_secs * 60.0`, NEVER scaled by `FogSpeedFix`).
    /// - `grid_repulsion_override`: optional SH2:EE `FogSpeedFix` (`0.25` / `0.50`) replacing ONLY
    ///   `FogEnv +0x14` (`_DAT_01f7a68c`) inside `fog_part_grid` (`FUN_00486260` `0x0048683D`).
    /// - `anchor_world`: fog volume center in PS2 mm (`_DAT_00581480` / `DAT_01f7a5f0`).
    /// - `player_world_pos`: James world position in PS2 mm for `fog_part_obj` (`0x0016b780`).
    /// - `james_fog_influence_mm`: James radial parting radius in PS2 mm (`200.0` default, `10.0` on Forest trail band).
    pub fn step_ps2(
        &mut self,
        dt_secs: f32,
        grid_repulsion_override: Option<f32>,
        anchor_world: [f32; 3],
        player_world_pos: [f32; 3],
        james_fog_influence_mm: f32,
    ) {
        if self.particles.is_empty() {
            self.anchor_world = anchor_world;
            self.prev_player_world = Some(player_world_pos);
            return;
        }

        let dt_scale = (dt_secs.max(0.0) * 60.0).min(6.0);
        let delta_anchor = [
            anchor_world[0] - self.anchor_world[0],
            anchor_world[1] - self.anchor_world[1],
            anchor_world[2] - self.anchor_world[2],
        ];
        let anchor_jump_xz =
            (delta_anchor[0] * delta_anchor[0] + delta_anchor[2] * delta_anchor[2]).sqrt();
        self.anchor_world = anchor_world;

        // `fogMoveParticle` (`Fog/fog.c:1081-1085`): if anchor teleports > 30000 mm, re-init volume
        if anchor_jump_xz > FOG_TELEPORT_REINIT_DIST_MM {
            self.init_particles();
            self.grid_prev.fill(1.0);
        }

        // Countdown `fogSetEnvironment` wind timer (`Fog/fog.c:227-231`)
        if self.wind.tick(dt_scale, &mut self.rng) {
            // `FUN_00487c40:4891-4898`: refresh per-particle grid jitter on wind change
            for p in &mut self.particles {
                p.grid_jitter = self.rng.rand_v_scale(0.5);
            }
        }

        let player_vel = match self.prev_player_world {
            Some(prev) => [
                player_world_pos[0] - prev[0],
                player_world_pos[1] - prev[1],
                player_world_pos[2] - prev[2],
            ],
            None => [0.0, 0.0, 0.0],
        };
        self.prev_player_world = Some(player_world_pos);

        let part_num = self.particles.len() as f32;
        let area_mm = self.env.range_z_mm;
        let rx = self.env.range_x_mm;
        let rz = self.env.range_z_mm;
        let grid_scale = 4.0 / area_mm; // `DAT_002ab448 = 4.0 / area` (`FUN_00486260:3824`)
        let y_grid_scale = if self.env.subdiv == 0 {
            1.0
        } else {
            (1u32 << (self.env.subdiv & 0x1f)) as f32
        };
        // Exact `FogAdjustments.cpp:238-306` (`0x0048683D`): `FogSpeedFix` ONLY replaces `FogEnv +0x14`!
        let effective_grid_repulsion = grid_repulsion_override.unwrap_or(self.env.grid_repulsion);
        let repulsion_scale = effective_grid_repulsion / part_num;
        let wind_vec = self.wind.wind_vec;
        // Smoothly integrate `ModernVolumetric` UV drift (`fog_view_screen_fog` `0x0016c1c0`
        // `wind * (dt_scale * 0.005) * 0.25`) so `fogChangeWind` rotations curve smoothly without snap.
        let drift_scale = dt_scale * 0.005 * 0.25;
        self.volumetric_wind_drift[0] =
            (self.volumetric_wind_drift[0] + wind_vec[0] * drift_scale).rem_euclid(1.0);
        self.volumetric_wind_drift[1] =
            (self.volumetric_wind_drift[1] + wind_vec[2] * drift_scale).rem_euclid(1.0);
        let base_alpha = f32::from(self.env.alpha_u8);
        let alt_fade_enabled =
            (self.env.flags & 0x01) != 0 && self.env.alt_fade_span_mm.abs() > 1e-4;
        let alt_fade_top = self.env.alt_fade_y_mm;
        let alt_fade_bot = self.env.alt_fade_y_mm - self.env.alt_fade_span_mm;
        let alpha_boost_y = self.env.alpha_boost_y_mm;
        let obj_enabled = (self.env.flags & 0x80) == 0;
        let obj_r_inner = self.env.size_mm + james_fog_influence_mm.max(0.0);
        let obj_soft = self.env.soft_thickness_mm.max(1.0);
        let obj_r_outer = obj_r_inner + obj_soft;
        let obj_r_outer_sq = obj_r_outer * obj_r_outer;

        self.grid_curr.fill(0.0);

        // Pass 1: integrate position, add wind acceleration, run `fog_part_grid` & `fog_part_alp`
        // (`fogMoveParticle` `Fog/fog.c:1131-1235`, `FUN_00489100` `sh2pc_00480000.c:5249-5292`).
        for p in &mut self.particles {
            if p.fading_out {
                p.alpha_raw *= 0.75;
                if p.alpha_raw <= 1.0 {
                    let pr = self.rng.rand_v_scale(1.0);
                    let y_span = if self.env.subdiv == 0 {
                        rz
                    } else {
                        (2.0 * rz) / ((1u32 << (self.env.subdiv & 0x1f)) as f32)
                    };
                    p.local_pos = [
                        pr[0] * rx,
                        if self.env.subdiv == 0 {
                            pr[1] * rz
                        } else {
                            -pr[1].abs() * y_span
                        },
                        pr[2] * rz,
                    ];
                    p.velocity = self.rng.rand_v_scale(1.0);
                    p.grid_jitter = self.rng.rand_v_scale(0.5);
                    p.alpha_raw = 0.0;
                    p.alpha_vel = 0.0;
                    p.hit_count = 0;
                    p.fading_out = false;
                }
            } else if p.hit_count > 0 {
                p.fading_out = true;
            }

            // `pos += vel * dt_scale` (keep in anchor-relative local coordinates)
            if anchor_jump_xz <= FOG_TELEPORT_REINIT_DIST_MM {
                p.local_pos[0] += p.velocity[0] * dt_scale - delta_anchor[0];
                p.local_pos[1] += p.velocity[1] * dt_scale - delta_anchor[1];
                p.local_pos[2] += p.velocity[2] * dt_scale - delta_anchor[2];
            }
            p.hit_count = 0;

            // `vel += wind_vec` (`Fog/fog.c:1179-1182`, `FUN_00489100:5280-5284`)
            p.velocity[0] += wind_vec[0];
            p.velocity[1] += wind_vec[1];
            p.velocity[2] += wind_vec[2];

            // `fog_part_grid` (`0x0016b230`, `FUN_00486260` `sh2pc_00480000.c:3824-3962`)
            let wx = anchor_world[0] + p.local_pos[0];
            let wy = anchor_world[1] + p.local_pos[1];
            let wz = anchor_world[2] + p.local_pos[2];

            let gx = wx * grid_scale;
            let gy = wy * grid_scale * y_grid_scale;
            let gz = wz * grid_scale;

            let ix_f = gx.floor();
            let iy_f = gy.floor();
            let iz_f = gz.floor();
            let fx = gx - ix_f;
            let fy = gy - iy_f;
            let fz = gz - iz_f;
            let ifx = 1.0 - fx;
            let ify = 1.0 - fy;
            let ifz = 1.0 - fz;

            let ix0 = (ix_f as i32 & 7) as usize;
            let iy0 = (iy_f as i32 & 7) as usize;
            let iz0 = (iz_f as i32 & 7) as usize;
            let ix1 = (ix0 + 1) & 7;
            let iy1 = (iy0 + 1) & 7;
            let iz1 = (iz0 + 1) & 7;

            let z0_off = iz0 << 6;
            let z1_off = iz1 << 6;
            let y0_off = iy0 << 3;
            let y1_off = iy1 << 3;

            self.grid_curr[z0_off | y0_off | ix0] += ifx + ify + ifz;
            self.grid_curr[z0_off | y0_off | ix1] += fx + ify + ifz;
            self.grid_curr[z0_off | y1_off | ix0] += ifx + fy + ifz;
            self.grid_curr[z0_off | y1_off | ix1] += fx + fy + ifz;
            self.grid_curr[z1_off | y0_off | ix0] += ifx + ify + fz;
            self.grid_curr[z1_off | y0_off | ix1] += fx + ify + fz;
            self.grid_curr[z1_off | y1_off | ix0] += ifx + fy + fz;
            self.grid_curr[z1_off | y1_off | ix1] += fx + fy + fz;

            // Sample density gradient from `grid_prev` at `(gx, gy, gz) + grid_jitter`
            let gx2 = gx + p.grid_jitter[0];
            let gy2 = gy + p.grid_jitter[1];
            let gz2 = gz + p.grid_jitter[2];
            let jx_f = gx2.floor();
            let jy_f = gy2.floor();
            let jz_f = gz2.floor();
            let tfx = gx2 - jx_f;
            let tfy = gy2 - jy_f;
            let tfz = gz2 - jz_f;

            let jx0 = (jx_f as i32 & 7) as usize;
            let jy0 = (jy_f as i32 & 7) as usize;
            let jz0 = (jz_f as i32 & 7) as usize;
            let jx1 = (jx0 + 1) & 7;
            let jy1 = (jy0 + 1) & 7;
            let jz1 = (jz0 + 1) & 7;
            let jxm = (jx0.wrapping_sub(1)) & 7;
            let jxp = (jx1 + 1) & 7;
            let jym = (jy0.wrapping_sub(1)) & 7;
            let jyp = (jy1 + 1) & 7;
            let jzm = (jz0.wrapping_sub(1)) & 7;
            let jzp = (jz1 + 1) & 7;

            let g = &self.grid_prev;
            let cell = |cx: usize, cy: usize, cz: usize| -> f32 { g[(cz << 6) | (cy << 3) | cx] };

            let sx_p = cell(jxp, jy0, jz0)
                + cell(jxp, jy1, jz0)
                + cell(jxp, jy0, jz1)
                + cell(jxp, jy1, jz1);
            let sx_0 = cell(jx0, jy0, jz0)
                + cell(jx0, jy1, jz0)
                + cell(jx0, jy0, jz1)
                + cell(jx0, jy1, jz1);
            let sx_1 = cell(jx1, jy0, jz0)
                + cell(jx1, jy1, jz0)
                + cell(jx1, jy0, jz1)
                + cell(jx1, jy1, jz1);
            let sx_m = cell(jxm, jy0, jz0)
                + cell(jxm, jy1, jz0)
                + cell(jxm, jy0, jz1)
                + cell(jxm, jy1, jz1);
            let grad_x = (sx_p - sx_0) * (1.0 - tfx) + (sx_1 - sx_m) * tfx;

            let sy_p = cell(jx0, jyp, jz0)
                + cell(jx1, jyp, jz0)
                + cell(jx0, jyp, jz1)
                + cell(jx1, jyp, jz1);
            let sy_0 = cell(jx0, jy0, jz0)
                + cell(jx1, jy0, jz0)
                + cell(jx0, jy0, jz1)
                + cell(jx1, jy0, jz1);
            let sy_1 = cell(jx0, jy1, jz0)
                + cell(jx1, jy1, jz0)
                + cell(jx0, jy1, jz1)
                + cell(jx1, jy1, jz1);
            let sy_m = cell(jx0, jym, jz0)
                + cell(jx1, jym, jz0)
                + cell(jx0, jym, jz1)
                + cell(jx1, jym, jz1);
            let grad_y = (sy_p - sy_0) * (1.0 - tfy) + (sy_1 - sy_m) * tfy;

            let sz_p = cell(jx0, jy0, jzp)
                + cell(jx1, jy0, jzp)
                + cell(jx0, jy1, jzp)
                + cell(jx1, jy1, jzp);
            let sz_0 = cell(jx0, jy0, jz0)
                + cell(jx1, jy0, jz0)
                + cell(jx0, jy1, jz0)
                + cell(jx1, jy1, jz0);
            let sz_1 = cell(jx0, jy0, jz1)
                + cell(jx1, jy0, jz1)
                + cell(jx0, jy1, jz1)
                + cell(jx1, jy1, jz1);
            let sz_m = cell(jx0, jy0, jzm)
                + cell(jx1, jy0, jzm)
                + cell(jx0, jy1, jzm)
                + cell(jx1, jy1, jzm);
            let grad_z = (sz_p - sz_0) * (1.0 - tfz) + (sz_1 - sz_m) * tfz;

            p.velocity[0] =
                (p.velocity[0] - repulsion_scale * grad_x) * FOG_PARTICLE_VELOCITY_DAMPING;
            p.velocity[1] =
                (p.velocity[1] - repulsion_scale * grad_y) * FOG_PARTICLE_VELOCITY_DAMPING;
            p.velocity[2] =
                (p.velocity[2] - repulsion_scale * grad_z) * FOG_PARTICLE_VELOCITY_DAMPING;

            // `fog_part_alp` (`0x0016b9a0`, `FUN_00486c30` `sh2pc_00480000.c:4111-4138`)
            let mut target_alpha = base_alpha;
            if alt_fade_enabled && wy < alt_fade_top {
                if wy >= alt_fade_bot {
                    target_alpha =
                        ((wy - alt_fade_bot) / (alt_fade_top - alt_fade_bot)) * base_alpha;
                } else {
                    target_alpha = 0.0;
                }
            }
            if wy > alpha_boost_y {
                target_alpha = FOG_ALPHA_RAW_MAX;
            }
            let sway = self
                .rng
                .sway_1f(base_alpha * FOG_ALPHA_SWAY_SCALE, FOG_ALPHA_SWAY_BIAS);
            p.alpha_vel = (p.alpha_vel + sway - (p.alpha_raw - target_alpha) * FOG_ALPHA_SPRING_K)
                * FOG_ALPHA_VEL_DAMPING;
            p.alpha_raw = (p.alpha_raw + p.alpha_vel).clamp(0.0, FOG_ALPHA_RAW_MAX);

            // `fogMoveParticle` (`Fog/fog.c:1190-1192`): fast upward velocity triggers fade-out
            if p.velocity[1] < -2.0 {
                p.fading_out = true;
            }
        }

        // Swap `grid_curr` -> `grid_prev` (`FUN_00489100:5295-5301`)
        self.grid_prev = self.grid_curr;

        // Pass 2: `fog_part_obj` (James radial parting) + `fog_part_clamp` (toroidal wrap & border fade)
        // (`Fog/fog.c:1283-1373`, `FUN_004868c0` `0x004868c0`, `FUN_00486d10` `0x00486d10`).
        for p in &mut self.particles {
            if obj_enabled {
                let wx = anchor_world[0] + p.local_pos[0];
                let wy = anchor_world[1] + p.local_pos[1];
                let wz = anchor_world[2] + p.local_pos[2];
                let dx = wx - player_world_pos[0];
                let dy = wy - player_world_pos[1];
                let dz = wz - player_world_pos[2];
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq < obj_r_outer_sq && dist_sq > 1e-6 {
                    let dist = dist_sq.sqrt();
                    let inv_dist = 1.0 / dist;
                    let nx = dx * inv_dist;
                    let ny = dy * inv_dist;
                    let nz = dz * inv_dist;
                    let weight = if dist >= obj_r_inner {
                        (obj_r_outer - dist) / obj_soft
                    } else {
                        p.hit_count += 1;
                        1.0
                    };
                    let mut rel_dot = ((p.velocity[0] - player_vel[0]) * nx
                        + (p.velocity[1] - player_vel[1]) * ny
                        + (p.velocity[2] - player_vel[2]) * nz)
                        * weight;
                    if rel_dot >= 0.0 {
                        rel_dot *= FOG_OBJ_OUTGOING_DAMPING;
                    }
                    p.velocity[0] -= nx * rel_dot;
                    p.velocity[1] -= ny * rel_dot;
                    p.velocity[2] -= nz * rel_dot;
                }
            }

            // `fog_part_clamp` (`0x0016ba50`, `FUN_00486d10` `sh2pc_00480000.c:4156-4257`)
            let (wx_local, flip_z) = wrap_symmetric_with_parity(p.local_pos[0], rx);
            p.local_pos[0] = wx_local;
            if flip_z {
                p.local_pos[2] = -p.local_pos[2];
            }

            let (wy_local, reset_alpha) =
                wrap_vertical_ps2(p.local_pos[1], area_mm, self.env.subdiv);
            p.local_pos[1] = wy_local;
            if reset_alpha {
                p.alpha_raw = 0.0;
            }

            let (wz_local, flip_x) = wrap_symmetric_with_parity(p.local_pos[2], rz);
            p.local_pos[2] = wz_local;
            if flip_x {
                p.local_pos[0] = -p.local_pos[0];
            }

            // Border fade (`300.0 mm`, `FUN_00486d10:4217-4253`)
            let mut clamped_alpha = p.alpha_raw;
            if p.local_pos[0] < PS2_FOG_BORDER_FADE_MM - rx {
                clamped_alpha *= ((rx + p.local_pos[0]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
            } else if p.local_pos[0] > rx - PS2_FOG_BORDER_FADE_MM {
                clamped_alpha *= ((rx - p.local_pos[0]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
            }

            if self.env.subdiv == 0 {
                if p.local_pos[1] < PS2_FOG_BORDER_FADE_MM - area_mm {
                    clamped_alpha *=
                        ((area_mm + p.local_pos[1]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
                } else if p.local_pos[1] > area_mm - PS2_FOG_BORDER_FADE_MM {
                    clamped_alpha *=
                        ((area_mm - p.local_pos[1]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
                }
            } else {
                let y_span = (2.0 * area_mm) / ((1u32 << (self.env.subdiv & 0x1f)) as f32);
                if p.local_pos[1] < PS2_FOG_BORDER_FADE_MM - y_span {
                    clamped_alpha *=
                        ((y_span + p.local_pos[1]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
                }
            }

            if p.local_pos[2] < PS2_FOG_BORDER_FADE_MM - rz {
                clamped_alpha *= ((rz + p.local_pos[2]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
            } else if p.local_pos[2] > rz - PS2_FOG_BORDER_FADE_MM {
                clamped_alpha *= ((rz - p.local_pos[2]) * INV_FOG_BORDER_FADE_MM).clamp(0.0, 1.0);
            }

            p.alpha = if base_alpha > 0.0 {
                (clamped_alpha / base_alpha).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_change_wind_clamps_turn_to_60_deg_and_respects_timer_bounds() {
        let mut rng = Ps2FogRng::new(0x1234_5678);
        let mut wind = FogWindState::default();
        wind.init_wind(0, &mut rng);

        for _ in 0..40 {
            let prev = wind.wind_vec;
            let prev_ang = prev[0].atan2(prev[2]);
            wind.change_wind(0, &mut rng);
            let next = wind.wind_vec;
            let next_ang = next[0].atan2(next[2]);
            let diff = super::super::wind::angle_regulate(prev_ang - next_ang).abs();
            assert!(
                diff <= super::super::wind::FOG_WIND_MAX_TURN_RAD + 1e-4,
                "wind turn exceeded 60 deg: {diff} rad"
            );
            assert!(
                (300.0..=900.0).contains(&wind.timer_frames),
                "wind timer out of [300, 900]: {}",
                wind.timer_frames
            );
        }
    }

    #[test]
    fn stationary_particles_drift_at_exact_ps2_terminal_velocity_under_vanilla_and_sh2ee() {
        // Test both wind_type = 0 (0.4 mm/frame² -> 216 mm/s) and wind_type = 0x12 (0.7 mm/frame² -> 378 mm/s),
        // under both Vanilla (`grid_repulsion_override = None`) and SH2:EE (`Some(0.25)`).
        for (accel, expected_mm_per_sec) in [(0.4f32, 216.0f32), (0.7f32, 378.0f32)] {
            assert!(
                (FogWindState::terminal_speed_mm_per_sec(accel) - expected_mm_per_sec).abs() < 1e-4
            );
            for repulsion_override in [None, Some(0.25f32)] {
                let mut vol = FogParticleVolume::new(400, 2000.0, 2000.0);
                vol.env.flags = 0x80; // disable fog_part_obj so James does not obstruct free wind flow
                vol.wind.timer_frames = 10_000.0; // hold constant wind during measurement
                vol.set_wind_accel_per_frame([accel, 0.0, 0.0]);

                for _ in 0..180 {
                    vol.step_ps2(
                        1.0 / 60.0,
                        repulsion_override,
                        [0.0, 0.0, 0.0],
                        [0.0, 0.0, 0.0],
                        200.0,
                    );
                }

                // Mean cloud X velocity in mm/s (`velocity[0] * 60.0`) matches exact PS2 terminal speed
                let mean_vx = vol
                    .particles()
                    .iter()
                    .map(|p| p.velocity[0] * 60.0)
                    .sum::<f32>()
                    / (vol.particles().len() as f32);
                assert!(
                    (mean_vx - expected_mm_per_sec).abs() < 5.0,
                    "accel={accel}, override={repulsion_override:?}: expected ~{expected_mm_per_sec} mm/s, got {mean_vx:.2} mm/s"
                );
            }
        }
    }

    #[test]
    fn fog_speed_fix_only_affects_grid_repulsion_not_wind_drift() {
        let mut vanilla = FogParticleVolume::new(400, 2000.0, 2000.0);
        vanilla.env.flags = 0x80; // isolate wind + fog_part_grid
        let mut sh2ee = vanilla.clone();

        vanilla.wind.timer_frames = 10_000.0;
        sh2ee.wind.timer_frames = 10_000.0;
        vanilla.set_wind_accel_per_frame([0.4, 0.0, 0.0]);
        sh2ee.set_wind_accel_per_frame([0.4, 0.0, 0.0]);

        for _ in 0..120 {
            vanilla.step_ps2(1.0 / 60.0, None, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 200.0);
            sh2ee.step_ps2(
                1.0 / 60.0,
                Some(0.25),
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                200.0,
            );
        }

        // Mean X velocity across the cloud must match PS2 wind terminal velocity (~216 mm/s) in BOTH modes
        let mean_vx_vanilla: f32 = vanilla
            .particles()
            .iter()
            .map(|p| p.velocity[0] * 60.0)
            .sum::<f32>()
            / (vanilla.particles().len() as f32);
        let mean_vx_sh2ee: f32 = sh2ee
            .particles()
            .iter()
            .map(|p| p.velocity[0] * 60.0)
            .sum::<f32>()
            / (sh2ee.particles().len() as f32);

        assert!(
            (mean_vx_vanilla - 216.0).abs() < 5.0,
            "vanilla mean vx should be ~216 mm/s, got {mean_vx_vanilla:.2}"
        );
        assert!(
            (mean_vx_sh2ee - 216.0).abs() < 5.0,
            "sh2ee mean vx should ALSO be ~216 mm/s (not 0.25x = 54 mm/s!), got {mean_vx_sh2ee:.2}"
        );
        // Individual particle positions differ because grid repulsion was 2.0 vs 0.25
        let pos_diff: f32 = vanilla
            .particles()
            .iter()
            .zip(sh2ee.particles().iter())
            .map(|(a, b)| (a.local_pos[0] - b.local_pos[0]).abs())
            .sum();
        assert!(
            pos_diff > 1.0,
            "FogSpeedFix (0.25 vs 2.0) should alter fog_part_grid repulsion spread"
        );
    }

    #[test]
    fn particle_volume_wraps_and_parts_around_james() {
        let mut vol = FogParticleVolume::new(64, 2000.0, 2000.0);
        vol.particles[0].local_pos = [50.0, 0.0, 0.0];
        vol.particles[0].velocity = [-5.0, 0.0, 0.0]; // moving toward James at origin
        vol.step_ps2(1.0 / 60.0, None, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 200.0);
        assert!(
            vol.particles[0].velocity[0] > -5.0,
            "fog_part_obj should deflect inward particle velocity away from James"
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
        vol.step_ps2(
            1.0 / 60.0,
            Some(0.25),
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
            assert!(p.local_pos[2].abs() <= 2000.0 + 1e-3);
        }
    }

    #[test]
    fn volumetric_wind_drift_curves_smoothly_without_snap_when_wind_rotates() {
        let mut vol = FogParticleVolume::new(16, 2000.0, 2000.0);
        vol.wind.timer_frames = 10_000.0;
        vol.set_wind_accel_per_frame([0.4, 0.0, 0.0]);

        // Advance 600 frames (10 s) in +X
        for _ in 0..600 {
            vol.step_ps2(1.0 / 60.0, None, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 200.0);
        }
        let drift_before = vol.volumetric_wind_drift();

        // Rotate wind direction by 60° (simulating `fogChangeWind`) and step 1 frame
        let ang = super::super::wind::FOG_WIND_MAX_TURN_RAD;
        vol.set_wind_accel_per_frame([0.4 * ang.cos(), 0.0, 0.4 * ang.sin()]);
        vol.step_ps2(1.0 / 60.0, None, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 200.0);
        let drift_after = vol.volumetric_wind_drift();

        // Per-frame delta must be tiny (`<= 0.4 * 0.005 * 0.25 = 0.0005`), never snapping!
        let mut du = (drift_after[0] - drift_before[0]).abs();
        if du > 0.5 {
            du = 1.0 - du;
        }
        let mut dv = (drift_after[1] - drift_before[1]).abs();
        if dv > 0.5 {
            dv = 1.0 - dv;
        }
        let step_dist = (du * du + dv * dv).sqrt();
        assert!(
            step_dist <= 0.0005 + 1e-6,
            "volumetric wind drift snapped on wind rotation: step_dist={step_dist:.6}"
        );
        assert!(
            step_dist > 0.0004,
            "volumetric wind drift should continue advancing smoothly: step_dist={step_dist:.6}"
        );
    }
}
