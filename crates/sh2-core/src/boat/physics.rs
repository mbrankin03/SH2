//! Rowboat physics, buoyancy/roll energy, oar animation sync, splash SFX/ripples,
//! and SH2:EE fixes (`Chacter/m3_boat.c`, `m3_boat_jms.c`, `m3_play.c`, `player_result.c`).
//!
//! Verified 1:1 against:
//! - PS2 Prototype ELF (`SLUS_202.28`):
//!   - `BoatSetHeight` (`0x0026ccc0`: `(wv_height_y + 7.18) * 500.0 = 3590.0` mm)
//!   - `BoatCheckControl` (`0x0026d040`: max speed `4.0`)
//!   - `BoatCheckAnimeSpeed` (`0x0026da20`)
//!   - `BoatCheckAnime` (`0x0026dd30`)
//!   - `BoatCheckOarStatus` (`0x0026def0`: `3500.0` mm threshold)
//!   - `BoatCheckEffect` (`0x0026e040`)
//!   - `BoatCheckRollEnergy` (`0x0026e150`)
//!   - `BoatPlayerCheckAnime` (`0x0010ec40`, `Chacter/m3_play.c`)
//!   - `GameBoatTimerCountUp` (`0x00290450`) & `GameBoatMaxSpeedCheck` (`0x00290490`)
//! - Retail PC Binary (`sh2pc.exe`):
//!   - `FUN_0053efa0` (`BoatSetHeight`: `(wv_height_y + 7.38) * 500.0 = 3690.0` mm)
//!   - `FUN_0053f1d0` (`BoatCheckControl`: max speed `3.5`)
//!   - `FUN_0053f770` (`BoatCheckAnimeSpeed`)
//!   - `FUN_0053f960` (`BoatCheckAnime`)
//!   - `FUN_0053fa60` (`BoatCheckOarStatus`)
//!   - `FUN_0053fb40` (`BoatCheckEffect`: SFX `0x3cf1`/`0x3cf2`, `5000.0` mm lateral offset)
//!   - `FUN_0053fd60` (`BoatCheckRollEnergy`)
//! - SH2:EE Fixes:
//!   - `Patches/RowboatAnimation.cpp` (wrap/reset at frame `0x4C` = `76`)
//!   - `Patches/RowboatSpawn.cpp` (Hotel dock spawn when flag `0x17B` = `379` is set)

use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI};

use super::input::regulate_angle;
use super::wave::LakeWaveSample;

/// Retail PC rowboat base water offset (`7.38` * `500.0` = `3690.0` mm at `0x0053efa0`).
///
/// Prototype drift: `SLUS_202.28` (`0x0026ccc0`) uses `7.18` (`3590.0` mm).
pub const BOAT_WATER_BASE_OFFSET_PC: f32 = 7.38;

/// PS2 prototype rowboat base water offset (`7.18` * `500.0` = `3590.0` mm at `0x0026ccc0`).
pub const BOAT_WATER_BASE_OFFSET_PS2: f32 = 7.18;

/// World scale factor (`500.0` mm per unit at `0x0026ccc0` / `0x0026d040`).
pub const BOAT_WORLD_SCALE_MM: f32 = 500.0;

/// Linear forward velocity damping coefficient (`0.15` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_LINEAR_DAMPING: f32 = 0.15;

/// Angular yaw velocity damping coefficient (`0.2` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_ANGULAR_DAMPING: f32 = 0.2;

/// Linear velocity deadband snap-to-zero (`0.05` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_LINEAR_DEADBAND: f32 = 0.05;

/// Angular velocity deadband snap-to-zero (`0.025` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_ANGULAR_DEADBAND: f32 = 0.025;

/// Linear acceleration scale (`pos_aspd = straight_power * 1.5` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_LINEAR_ACCEL_SCALE: f32 = 1.5;

/// Reverse speed limit (`-3.0` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_MIN_FORWARD_SPEED: f32 = -3.0;

/// Retail PC maximum forward speed (`3.5` at `0x0053f1d0`).
///
/// Prototype drift: `SLUS_202.28` (`0x0026d040`) clamps at `4.0`.
pub const BOAT_MAX_FORWARD_SPEED_PC: f32 = 3.5;

/// Angular acceleration numerator (`rot_aspd = round_power * 0.25 / (|v| + 1.0)` at `0x0026d040`).
pub const BOAT_ANGULAR_ACCEL_SCALE: f32 = 0.25;

/// Angular velocity clamp (`±1.5` rad/s at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_MAX_ANGULAR_SPEED: f32 = 1.5;

/// Wave roll/pitch amplitude divisor (`1480.4408` = `2.0 / 0.0013509489` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_WAVE_ENERGY_DIVISOR: f32 = 1480.4408;

/// Wave energy decay per tick for heave `Y` (`0.01` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_HEAVE_DECAY: f32 = 0.01;

/// Wave energy decay per tick for roll `X` & pitch `Z` (`0.02` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_ROLL_DECAY: f32 = 0.02;

/// Wave energy cutoff threshold (`0.0005` at `0x0026d040` / `0x0053f1d0`).
pub const BOAT_WAVE_ENERGY_EPSILON: f32 = 0.0005;

/// Oar submersion height threshold in world mm (`3500.0` at `0x0026def0` / `0x0053fa60`).
pub const BOAT_OAR_WATER_THRESHOLD_MM: f32 = 3500.0;

/// Lateral splash offset from boat center (`5000.0` mm at `0x0053fb40`).
pub const BOAT_SPLASH_LATERAL_OFFSET_MM: f32 = 5000.0;

/// Retail PC splash SFX ID for right oar (`0x3cf1` at `0x0053fb40`).
pub const SFX_BOAT_OAR_RIGHT: u32 = 0x3cf1;

/// Retail PC splash SFX ID for left oar (`0x3cf2` at `0x0053fb40`).
pub const SFX_BOAT_OAR_LEFT: u32 = 0x3cf2;

/// Retail PC splash speed-to-volume scale (`9.765625e-05` = `1 / 10240` at `0x0053fb40`).
pub const BOAT_SPLASH_SPEED_SCALE: f32 = 9.765_625e-5;

/// Maximum Q12 oar animation speed (`0x800` = `2048` at `0x0026da20` / `0x0053f770`).
pub const BOAT_MAX_ANIME_SPEED_Q12: i16 = 0x800;

/// Q12 animation speed acceleration step (`0x40` at `0x0026da20` / `0x0053f770`).
pub const BOAT_ANIME_ACCEL_STEP: i16 = 0x40;

/// Q12 animation speed deceleration step (`0x10` at `0x0026da20` / `0x0053f770`).
pub const BOAT_ANIME_DECEL_STEP: i16 = 0x10;

/// SH2:EE `RowboatAnimationFix` frame wrap limit (`0x4C` = `76.0` in `Patches/RowboatAnimation.cpp`).
pub const ROWBOAT_ANIM_FIX_MAX_FRAME: f32 = 76.0;

/// SH2:EE `RowboatSpawnFix` game flag (`kEnteredRowboatGameFlag = 0x17B` = `379`).
pub const FLAG_ENTERED_ROWBOAT: usize = 0x17B;

/// SH2:EE `RowboatSpawnFix` Hotel dock world position in mm (`Patches/RowboatSpawn.cpp`).
pub const ROWBOAT_HOTEL_DOCK_POS_MM: Vec3 = Vec3::new(-8915.9, 3690.0, -19412.7);

/// SH2:EE `RowboatSpawnFix` Hotel dock yaw in radians (`PI` in `Patches/RowboatSpawn.cpp`).
pub const ROWBOAT_HOTEL_DOCK_YAW_RAD: f32 = PI;

/// `close_to_value` (`0x0026c100`): steps `val` toward `target` by `step`.
#[inline]
pub fn close_to_value(val: i16, target: i16, step: i16) -> i16 {
    if val == target {
        return val;
    }
    if target < val {
        (val - step).max(target)
    } else {
        (val + step).min(target)
    }
}

/// Oar water-contact state (`BoatCheckOarStatus` `0x0026def0` / `0x0053fa60`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RowboatOarState {
    pub left_in_water: bool,
    pub right_in_water: bool,
    pub prev_left_in_water: bool,
    pub prev_right_in_water: bool,
}

/// Splash / ripple event emitted when an oar enters or exits the water (`BoatCheckEffect`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowboatOarEvent {
    /// `true` for left oar, `false` for right oar.
    pub is_left: bool,
    /// `true` when entering water (`0 -> 1`), `false` when exiting (`1 -> 0`).
    pub entered_water: bool,
    /// World-space splash position in mm.
    pub world_pos: Vec3,
    /// Retail PC SFX ID (`0x3cf2` left, `0x3cf1` right).
    pub sfx_id: u32,
    /// Normalized splash intensity (`|anim_speed| * 9.765625e-5`).
    pub intensity: f32,
}

/// Wave-induced boat heave, roll, and pitch state (`BoatCheckRollEnergy` `0x0026e150` / `0x0053fd60`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RowboatWaveState {
    pub energy_x: f32,
    pub energy_y: f32,
    pub energy_z: f32,
    pub timer_x: f32,
    pub timer_y: f32,
    pub timer_z: f32,
    pub amplitude_x: f32,
    pub amplitude_z: f32,
    pub height_x: f32,
    pub height_y: f32,
    pub height_z: f32,
    pub status_x: i32,
    pub status_z: i32,
    pub type_x: u8,
    pub type_z: u8,
}

impl RowboatWaveState {
    /// Integrates wave heave/roll/pitch (`BoatCheckControl` `0x0026d040` / `0x0053f1d0`).
    pub fn integrate_control(&mut self, dt: f32) -> (f32, f32) {
        let mut roll = 0.0;
        let mut pitch = 0.0;

        if self.energy_y != 0.0 {
            self.timer_y += dt;
            if self.timer_y > 2.0 {
                self.timer_y -= 2.0;
            }
            self.height_y = -self.energy_y * (self.timer_y * PI).sin();
            self.energy_y -= self.energy_y * BOAT_HEAVE_DECAY;
            if self.energy_y.abs() < BOAT_WAVE_ENERGY_EPSILON {
                self.energy_y = 0.0;
                self.timer_y = 0.0;
                self.height_y = 0.0;
            }
        }

        if self.energy_x != 0.0 {
            self.timer_x += dt;
            if self.timer_x > 2.0 {
                self.timer_x -= 2.0;
            }
            self.status_x = if self.timer_x < 0.5 || self.timer_x >= 1.5 {
                1
            } else {
                -1
            };
            self.amplitude_x = ((self.energy_x * 2.0) / BOAT_WAVE_ENERGY_DIVISOR).sqrt();
            self.height_x = self.amplitude_x * (self.timer_x * PI).sin();
            self.energy_x -= self.energy_x * BOAT_ROLL_DECAY;
            if self.energy_x.abs() < BOAT_WAVE_ENERGY_EPSILON {
                self.energy_x = 0.0;
                self.timer_x = 0.0;
                self.height_x = 0.0;
            }
            roll = self.height_x.atan2(0.5);
        }

        if self.energy_z != 0.0 {
            self.timer_z += dt;
            if self.timer_z > 2.0 {
                self.timer_z -= 2.0;
            }
            self.status_z = if self.timer_z < 0.5 || self.timer_z >= 1.5 {
                1
            } else {
                -1
            };
            self.amplitude_z = ((self.energy_z * 2.0) / BOAT_WAVE_ENERGY_DIVISOR).sqrt();
            self.height_z = self.amplitude_z * (self.timer_z * PI).sin();
            self.energy_z -= self.energy_z * BOAT_ROLL_DECAY;
            if self.energy_z.abs() < BOAT_WAVE_ENERGY_EPSILON {
                self.energy_z = 0.0;
                self.timer_z = 0.0;
                self.height_z = 0.0;
            }
            pitch = self.height_z.atan2(2.0);
        }

        (roll, pitch)
    }

    /// Updates 4-quadrant roll/pitch energy (`BoatCheckRollEnergy` `0x0026e150` / `0x0053fd60`).
    pub fn apply_roll_energy(
        &mut self,
        round_power: f32,
        straight_power: f32,
        boat_yaw: f32,
        wave_sample: Option<LakeWaveSample>,
        dt: f32,
    ) {
        let mut init_x = self.energy_x == 0.0 && round_power != 0.0;
        let mut init_z = self.energy_z == 0.0 && straight_power != 0.0;
        let mut init_y = false;

        let mut fx = round_power * 0.05;
        let mut fz = straight_power * 0.05;
        let mut fy = 0.0;

        if let Some(wave) = wave_sample {
            let rel_angle = regulate_angle(wave.angle - boat_yaw);
            fx += dt * wave.power * rel_angle.sin() * 2.0;
            fz += dt * wave.power * rel_angle.cos() * 4.0;
            fy += wave.power * 0.05;
            init_x = self.energy_x == 0.0;
            init_z = self.energy_z == 0.0;
            init_y = self.energy_y == 0.0;
        }

        if init_y {
            self.timer_y = 0.0;
            self.energy_y += fy;
        }

        if init_x {
            if fx > 0.0 {
                self.timer_x = 0.0;
                self.status_x = 1;
                self.type_x = u8::from(self.height_x < 0.0);
            } else {
                self.timer_x = 1.0;
                self.status_x = -1;
                self.type_x = if self.height_x > 0.0 { 2 } else { 3 };
            }
        }

        if init_z {
            if fz > 0.0 {
                self.timer_z = 0.0;
                self.status_z = 1;
                self.type_z = u8::from(self.height_z < 0.0);
            } else {
                self.timer_z = 1.0;
                self.status_z = -1;
                self.type_z = if self.height_z > 0.0 { 2 } else { 3 };
            }
        }

        step_quadrant(&mut self.energy_x, &mut self.timer_x, &mut self.type_x, fx);
        step_quadrant(&mut self.energy_z, &mut self.timer_z, &mut self.type_z, fz);
    }
}

#[inline]
fn step_quadrant(energy: &mut f32, timer: &mut f32, qtype: &mut u8, force: f32) {
    match *qtype {
        0 => {
            *energy += force;
            if *energy < 0.0 {
                *energy = -*energy;
                *timer = 0.5;
                *qtype = 2;
            }
        }
        1 => {
            *energy += force;
            if *energy < 0.0 {
                *energy = -*energy;
                *timer = 1.0;
                *qtype = 3;
            }
        }
        2 => {
            *energy -= force;
            if *energy < 0.0 {
                *energy = -*energy;
                *timer = 0.0;
                *qtype = 0;
            }
        }
        _ => {
            *energy -= force;
            if *energy < 0.0 {
                *energy = -*energy;
                *timer = 1.5;
                *qtype = 1;
            }
        }
    }
}

/// Evaluates `BoatCheckEffect` (`0x0026e040` SLUS / `0x0053fb40` PC) for oar splash events.
pub fn collect_oar_events(
    oar_state: &RowboatOarState,
    boat_pos: Vec3,
    boat_yaw: f32,
    anim_speed_q12: i16,
) -> Vec<RowboatOarEvent> {
    let mut events = Vec::new();
    let intensity = (anim_speed_q12 as f32).abs() * BOAT_SPLASH_SPEED_SCALE;

    let check_side = |curr: bool, prev: bool, is_left: bool, events: &mut Vec<RowboatOarEvent>| {
        if curr == prev {
            return;
        }
        let angle = if is_left {
            boat_yaw - FRAC_PI_2
        } else {
            boat_yaw + FRAC_PI_2
        };
        let world_pos = Vec3::new(
            boat_pos.x + angle.sin() * BOAT_SPLASH_LATERAL_OFFSET_MM,
            boat_pos.y,
            boat_pos.z + angle.cos() * BOAT_SPLASH_LATERAL_OFFSET_MM,
        );
        events.push(RowboatOarEvent {
            is_left,
            entered_water: curr,
            world_pos,
            sfx_id: if is_left {
                SFX_BOAT_OAR_LEFT
            } else {
                SFX_BOAT_OAR_RIGHT
            },
            intensity,
        });
    };

    check_side(
        oar_state.right_in_water,
        oar_state.prev_right_in_water,
        false,
        &mut events,
    );
    check_side(
        oar_state.left_in_water,
        oar_state.prev_left_in_water,
        true,
        &mut events,
    );
    events
}

/// Returns the SH2:EE `RowboatSpawnFix` Hotel dock `(position, yaw)` if `kEnteredRowboatGameFlag` (`0x17B`) is set.
#[inline]
pub fn resolve_sh2ee_rowboat_spawn(
    entered_rowboat_flag: bool,
    enable_sh2ee_world_fixes: bool,
    default_pos: Vec3,
    default_yaw: f32,
) -> (Vec3, f32) {
    if enable_sh2ee_world_fixes && entered_rowboat_flag {
        (ROWBOAT_HOTEL_DOCK_POS_MM, ROWBOAT_HOTEL_DOCK_YAW_RAD)
    } else {
        (default_pos, default_yaw)
    }
}
