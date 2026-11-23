//! Eddie Dombrowski Boss (`en_edb.c`) parameter tables & Hanging Meat pendulum (`enNIKCtrlMain`).
//!
//! # Ground Truth:
//! - `SLUS_202.28`: `0x002a9ce0` (`door_pos`), `0x002a9cf0` (`room_corner[6]`), `0x002aa070..0x002aa248`, `0x0014ddd0` (`enNIKCtrlMain`).
//! - `sh2pc.exe`: `0x004b1050..0x004b1360` (retail HP/speed/punch-limit retuning), `0x004b34a0`/`0x004b4220` (`+6` attack ID shift).

use glam::Vec3;

/// Retail PC Eddie Room 1 HP table (`sh2pc 0x004b1360`; SLUS `0x002aa070` had `500.0` for Beginner).
pub const EDB_HP_ROOM1_PC: [f32; 5] = [100.0, 1800.0, 2500.0, 4000.0, 5000.0];
/// Retail PC Eddie Room 2 HP table (`sh2pc 0x004b1360`; SLUS `0x002aa090` had `[2000, 4000, 5000, 7000, 10000]`).
pub const EDB_HP_ROOM2_PC: [f32; 5] = [1000.0, 4000.0, 5000.0, 10_000.0, 12_000.0];
/// Eddie endurance table (`SLUS 0x002aa0b0`, unchanged on PC).
pub const EDB_ENDURANCE: [f32; 5] = [200.0, 300.0, 350.0, 500.0, 600.0];
/// Eddie endurance recovery per second (`SLUS 0x002aa1b0` `enEDBAutoRecovery` `0x0014d950`).
pub const EDB_AUTO_RECOVERY: [f32; 5] = [10.0, 50.0, 70.0, 100.0, 150.0];
/// Retail PC walk speed multipliers (`sh2pc 0x004b1050`, base `1000.0` mm/s).
pub const EDB_WALK_SPEED_MULT_PC: [f32; 5] = [0.7, 0.8, 1.0, 1.2, 1.5];
/// Retail PC run speed multipliers (`sh2pc 0x004b10b0`, base `1400.0` mm/s).
pub const EDB_RUN_SPEED_MULT_PC: [f32; 5] = [0.7, 0.8, 1.0, 1.4, 1.5];
/// Retail PC aiming rotation multipliers (`sh2pc 0x004b11a0`, × `0.05235988` rad/tick).
pub const EDB_AIM_ROT_MULT_PC: [f32; 5] = [0.5, 0.6, 0.85, 1.2, 1.5];
/// Default hits-before-retreat counter (`SLUS 0x002aa238` `enEDBGetDefAfford` `0x0014dc50`).
pub const EDB_DEF_AFFORD: [i8; 5] = [7, 10, 15, 20, 30];
/// Retail PC consecutive punch limit (`sh2pc 0x004b1230`; SLUS `0x002aa248` had `[2, 3, 3, 4, 5]`).
pub const EDB_PUNCH_LIMIT_PC: [u8; 5] = [2, 3, 3, 5, 10];

/// Base turn step (`3°` = `0.05235988` rad/tick, `SLUS 0x001497e0`).
pub const EDB_TURN_STEP_RAD: f32 = 0.052_359_88;
/// Retail PC attack IDs (`+6` over SLUS `0x34`/`0x35`).
pub const EDB_ATTACK_ID_SHOOT_PC: u16 = 0x3a;
pub const EDB_ATTACK_ID_PUNCH_PC: u16 = 0x3b;
/// Room 1 exit door position (`door_pos` `SLUS 0x002a9ce0`).
pub const EDB_ROOM1_DOOR_POS: Vec3 = Vec3::new(100_000.0, 0.0, -99_100.0);
/// Room 2 meat-locker 6 corner waypoints (`room_corner[6]` `SLUS 0x002a9cf0`).
pub const EDB_ROOM2_CORNERS: [Vec3; 6] = [
    Vec3::new(142_500.0, 0.0, -103_750.0),
    Vec3::new(142_500.0, 0.0, -96_250.0),
    Vec3::new(138_250.0, 0.0, -103_750.0),
    Vec3::new(138_250.0, 0.0, -97_500.0),
    Vec3::new(139_500.0, 0.0, -96_250.0),
    Vec3::new(142_500.0, 0.0, -97_000.0),
];

/// Hanging meat slab pendulum physics (`enNIKCtrlMain` `SLUS 0x0014ddd0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HangingMeatPendulum {
    pub rest_pos: Vec3,
    pub disp_xz: (f32, f32),
    pub vel_xz: (f32, f32),
    pub pitch: f32,
    pub roll: f32,
}

impl HangingMeatPendulum {
    pub fn new(rest_pos: Vec3) -> Self {
        Self {
            rest_pos,
            disp_xz: (0.0, 0.0),
            vel_xz: (0.0, 0.0),
            pitch: 0.0,
            roll: 0.0,
        }
    }

    /// Update spring-damper pendulum (`vel -= pos * dt * 2.0`, `vel *= 1.0 - dt * 3.0`, `atan2(2000.0, ±disp)`).
    pub fn update(&mut self, dt: f32) {
        let damp = (1.0 - dt * 3.0).max(0.0);
        self.vel_xz.0 = (self.vel_xz.0 - self.disp_xz.0 * dt * 2.0) * damp;
        self.vel_xz.1 = (self.vel_xz.1 - self.disp_xz.1 * dt * 2.0) * damp;
        self.disp_xz.0 += self.vel_xz.0 * dt;
        self.disp_xz.1 += self.vel_xz.1 * dt;
        self.roll = self.disp_xz.0.atan2(2000.0);
        self.pitch = (-self.disp_xz.1).atan2(2000.0);
    }
}
