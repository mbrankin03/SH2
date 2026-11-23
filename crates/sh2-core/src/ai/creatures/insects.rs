//! Room Insects / Mary Moth Swarm (`en_insect.c`, type `0x0f`) & Ambient Fly (`en_fly.c`) (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_insect.c`):
//!   - `0x0027d0b0` `enINSInitData`, `0x0027d210` `enSetInsect`, `0x0027d540` `enINSCtrlMain`.
//!   - `0x0027d640` `enINSCtrlApproach`, `0x0027d800` `enINSCtrlAttack`, `0x0027d8a0` `enINSCtrlEscape`,
//!     `0x0027db00` `enINSCtrlChase`, `0x0027dc70` `enINSCtrlStay`, `0x0027de00` `enINSMove`.
//!   - `0x00375b60` `escape_point[3]` — Mary rooftop escape targets.
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_fly.c`) & Retail PC (`sh2pc 0x005b6eb0`):
//!   - `0x001db2a0` `flyInit`, `0x001db3c0` `flyMove` (0% drift on retail PC).
//! - Retail PC (`sh2pc.exe`) drift (`en_insect.c`):
//!   - `enINSMove` (`0x004abc10`): wall check only in `Stay` (`state == 4`).
//!   - Sound `0x49b2` volume `0.6 -> 1.0` (`0x004abf3e`).

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{normalize_angle, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Mary boss rooftop moth escape points (`escape_point[3]` `SLUS 0x00375b60`).
pub const INSECT_ESCAPE_POINTS: [Vec3; 3] = [
    Vec3::new(58_600.0, -16_500.0, 63_600.0),
    Vec3::new(61_400.0, -16_500.0, 63_600.0),
    Vec3::new(58_600.0, -16_500.0, 56_400.0),
];

/// Room insect state enum matching `0x00375b90` (`enINSCtrlMain` `SLUS 0x0027d540`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum InsectState {
    #[default]
    Approach = 0,
    Attack = 1,
    Escape = 2,
    Chase = 3,
    Stay = 4,
}

/// Room Insect / Mary Moth Swarm controller (`en_insect.c`, type `0x0f`).
#[derive(Debug, Clone)]
pub struct RoomInsectController {
    pub id: u32,
    pub position: Vec3,
    pub home_pos: Vec3,
    pub yaw: f32,
    pub state: InsectState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub speed_mm_s: f32,
    pub accel_speed: f32,
    pub speed_acc: f32,
    pub vel_y: f32,
    pub ang_vel: f32,
    pub timer_ticks: f32,
    pub escape_idx: usize,
    pub wing_amp: f32,
    pub wing_vel: f32,
    /// True when the swarm has finished escaping above `Y = -15500.0` (`enINSCtrlEscape` `0x0027d8a0`).
    pub despawn_all: bool,
}

impl RoomInsectController {
    /// Spawn a swarm leader (`is_leader = true` -> `Approach`) or follower (`Chase`) (`enINSInitData` `0x0027d0b0`).
    pub fn new(id: u32, position: Vec3, yaw: f32, is_leader: bool) -> Self {
        Self {
            id,
            position,
            home_pos: position,
            yaw: normalize_angle(yaw),
            state: if is_leader {
                InsectState::Approach
            } else {
                InsectState::Chase
            },
            sub_state: 0,
            stats: CreatureStats::new(1.0, 1.0, 0.0),
            speed_mm_s: if is_leader { 500.0 } else { 1500.0 },
            accel_speed: 500.0,
            speed_acc: 0.0,
            vel_y: 0.0,
            ang_vel: 0.0,
            timer_ticks: 15.0,
            escape_idx: 0,
            wing_amp: 25.0,
            wing_vel: 0.0,
            despawn_all: false,
        }
    }

    /// Spawn an ambient room moth in `Stay` mode (`enSetInsect` `SLUS 0x0027d210`).
    pub fn new_ambient_stay(id: u32, position: Vec3, yaw: f32) -> Self {
        let mut s = Self::new(id, position, yaw, false);
        s.state = InsectState::Stay;
        s.speed_mm_s = 500.0;
        s
    }

    /// Advance insect AI by `dt` seconds (`enINSCtrlMain` `SLUS 0x0027d540`).
    ///
    /// - `leader_state_pos`: for `Chase` followers, pass `(leader.state, leader.position)`.
    /// - `player_spraying`: whether James is actively using Hyper Spray (`enCheckPlayerSprayNow`).
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        leader_state_pos: Option<(InsectState, Vec3)>,
        player_spraying: bool,
        dt: f32,
    ) {
        let ticks = dt * 60.0;

        match self.state {
            InsectState::Approach => {
                if self.sub_state == 0 {
                    self.speed_mm_s = 500.0;
                    self.accel_speed = 500.0;
                    self.timer_ticks = 15.0;
                    self.sub_state = 1;
                }
                self.accel_speed += dt * 250.0;
                let goal = Vec3::new(target.position.x, -16_800.0, target.position.z);
                let dist_xz = xz_dist(self.position, target.position);
                let target_spd = self.accel_speed * (dist_xz / 500.0).max(0.0).sqrt();
                self.move_spring(0.8, 0.087_266_46, target_spd, goal, dt);

                if dist_xz < 250.0 {
                    if player_spraying {
                        self.state = InsectState::Escape;
                        self.sub_state = 0;
                    } else {
                        self.timer_ticks -= ticks;
                        if self.timer_ticks <= 0.0 {
                            self.state = InsectState::Attack;
                            self.sub_state = 0;
                        }
                    }
                }
            }

            InsectState::Attack => {
                self.move_spring(0.9, 0.087_266_46, 1500.0, target.position, dt);
            }

            InsectState::Escape => match self.sub_state {
                0 => {
                    self.escape_idx = INSECT_ESCAPE_POINTS
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| {
                            a.distance(self.position)
                                .total_cmp(&b.distance(self.position))
                        })
                        .map_or(0, |(i, _)| i);
                    self.sub_state = 1;
                }
                1 => {
                    let pt = INSECT_ESCAPE_POINTS[self.escape_idx];
                    self.move_spring(0.8, 0.087_266_46, 2500.0, pt, dt);
                    if self.position.distance(pt) < 1500.0 {
                        self.sub_state = 2;
                    }
                }
                _ => {
                    let mut pt = INSECT_ESCAPE_POINTS[self.escape_idx];
                    pt.y = -15_000.0;
                    self.move_spring(0.8, 0.087_266_46, 2500.0, pt, dt);
                    if self.position.y > -15_500.0 {
                        self.despawn_all = true;
                    }
                }
            },

            InsectState::Chase => {
                if let Some((l_state, l_pos)) = leader_state_pos {
                    if l_state == InsectState::Attack {
                        self.move_spring(0.8, 0.087_266_46, 1500.0, target.position, dt);
                    } else {
                        let target_spd = (self.position.distance(l_pos) / 500.0) * 1500.0;
                        self.move_spring(0.8, 0.174_532_92, target_spd, l_pos, dt);
                    }
                } else {
                    self.move_spring(0.8, 0.087_266_46, 1500.0, target.position, dt);
                }
            }

            InsectState::Stay => {
                if target.is_flashlight_on {
                    let goal = Vec3::new(
                        target.position.x + target.yaw.sin() * 600.0,
                        self.home_pos.y,
                        target.position.z + target.yaw.cos() * 600.0,
                    );
                    self.move_spring(0.9, 0.174_532_92, 1500.0, goal, dt);
                } else {
                    self.move_spring(0.9, 0.087_266_46, 500.0, self.home_pos, dt);
                }
                // `enINSCtrlStay` `0x0027dc70`: triples vertical velocity (`vel.y *= 3.0`).
                self.position.y += self.vel_y * 2.0 * dt;
            }
        }
    }

    /// Spring-damper flight step (`enINSMove` `SLUS 0x0027de00`).
    fn move_spring(
        &mut self,
        damp: f32,
        turn_speed: f32,
        target_speed: f32,
        target_pos: Vec3,
        dt: f32,
    ) {
        let ticks = dt * 60.0;
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dir = dx.atan2(dz);

        self.vel_y = (self.vel_y - (self.position.y - target_pos.y) * 0.05) * 0.95;
        self.ang_vel = normalize_angle(dir - self.yaw) * 0.1 + self.ang_vel * damp;
        let desired = normalize_angle(self.yaw + self.ang_vel);
        let step =
            normalize_angle(desired - self.yaw).clamp(-turn_speed * ticks, turn_speed * ticks);
        self.yaw = normalize_angle(self.yaw + step);

        self.speed_acc = (self.speed_acc - (self.speed_mm_s - target_speed) * 0.1) * 0.98;
        let min_s = target_speed * 0.5;
        let max_s = target_speed * 2.0;
        let next_s = self.speed_mm_s + self.speed_acc;
        if next_s < min_s || next_s > max_s {
            self.speed_mm_s = next_s.clamp(min_s, max_s);
            self.speed_acc = 0.0;
        } else {
            self.speed_mm_s = next_s;
        }

        self.wing_vel = (self.wing_vel - (self.wing_amp - 25.0) * 0.05) * 0.95;
        self.wing_amp += self.wing_vel;

        self.position.x += self.yaw.sin() * self.speed_mm_s * dt;
        self.position.y += self.vel_y * dt;
        self.position.z += self.yaw.cos() * self.speed_mm_s * dt;
    }
}

/// Ambient Fly / Moth controller (`en_fly.c`: `flyInit` `0x001db2a0`, `flyMove` `0x001db3c0` / `sh2pc 0x005b6eb0`).
#[derive(Debug, Clone)]
pub struct AmbientFlyController {
    pub position: Vec3,
    pub base_y: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub yaw: f32,
    pub vel_y: f32,
    pub yaw_acc: f32,
    pub yaw_off: f32,
}

impl AmbientFlyController {
    /// Initialize ambient fly (`flyInit` `SLUS 0x001db2a0`).
    pub fn new(position: Vec3, initial_yaw: f32) -> Self {
        Self {
            position,
            base_y: position.y,
            min_y: position.y - 500.0,
            max_y: position.y + 500.0,
            yaw: normalize_angle(initial_yaw),
            vel_y: 0.0,
            yaw_acc: 0.0,
            yaw_off: 0.0,
        }
    }

    /// Execute one tick of `flyMove` (`SLUS 0x001db3c0` / `sh2pc 0x005b6eb0`, 0% drift).
    ///
    /// - `sway_y`: sample in `[-25.0, 25.0]` matching `shSway1f(-25.0, 0.1)`.
    /// - `sway_yaw`: sample in `[-0.034906585, 0.034906585]` (`±2°`) matching `shSway1f(-0.034906585, 0.1)`.
    pub fn update(&mut self, sway_y: f32, sway_yaw: f32, dt: f32) {
        self.vel_y = ((self.vel_y + sway_y) - (self.position.y - self.base_y) * 0.01) * 0.95;
        self.position.y = (self.position.y + self.vel_y).clamp(self.min_y, self.max_y);

        self.yaw_acc = ((self.yaw_acc + sway_yaw) - self.yaw_off * 0.01) * 0.95;
        self.yaw_off = (self.yaw_off + self.yaw_acc) * 0.95;

        let step = self.yaw_off.clamp(-0.261_799_4, 0.261_799_4);
        self.yaw = normalize_angle(self.yaw + step);

        // Moves at 1500.0 mm/s (`flyMove` `0x001db3c0` / `0x005b6eb0`).
        self.position.x += self.yaw.sin() * (dt * 1500.0);
        self.position.z += self.yaw.cos() * (dt * 1500.0);
    }
}

fn xz_dist(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_insect_approach_to_escape_and_ambient_fly() {
        let mut leader = RoomInsectController::new(1, Vec3::new(0.0, -16_800.0, 0.0), 0.0, true);
        let player = PerceivableTarget::new(Vec3::new(0.0, -16_800.0, 100.0), 0.0, false, false);
        leader.update(&player, None, true, 1.0 / 60.0);
        assert_eq!(leader.state, InsectState::Escape);

        let mut fly = AmbientFlyController::new(Vec3::new(0.0, -1000.0, 0.0), 0.0);
        fly.update(10.0, 0.01, 1.0 / 60.0);
        assert!(fly.position.y >= fly.min_y && fly.position.y <= fly.max_y);
        assert!(fly.position.z > 0.0);
    }
}
