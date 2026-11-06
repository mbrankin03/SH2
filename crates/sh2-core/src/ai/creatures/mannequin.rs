//! Mannequin (`chr/mkn`, kind 0x201) state machine and AI behaviors (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_mkn.c` (`enMKNCtrlMannequin`, `enMKNCtrlWander`, `enMKNCtrlChase`, `enMKNCtrlAround`, `enMKNCtrlAttack`)
//! - PC executable `sh2pc.exe`:
//!   - `0x4c0d70`..`0x4c38dc`: Mannequin control logic.
//!   - `0x4c18f0`: `MKN_STAY` - motionless pose. Wakes when flashlight beam enters 2,250 units and 40° angle, or player approaches within 900 units (`0x4c0eb0`). Alerts nearby mannequins within 1,000 units.
//!   - `0x4c2420`: `MKN_WANDER` - wander targets 1,500 units away, stays within 5,000 units of spawn.
//!   - `0x4c2950`: `MKN_CHASE` - advances at player.
//!   - `0x4c2a90`: `MKN_CIRCLE` - when line of sight is broken, flanks to a point 1,000 units away, 120° off player's facing.
//!   - `0x4c2c70`: `MKN_ATTACK` - alternates arm swing (attack 44) and kick (attack 45).
//!
//! # Speeds:
//! - Walk root motion: 19.2 units per frame (`0x893070` anime 1, 30 fps = 576 units/s).
//! - Normal health: 490 HP (70% of Lying Figure).

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{
    evaluate_sensory, is_lit_by_flashlight, normalize_angle, DifficultyLevel, PerceivableTarget,
    RoomLighting, SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Authentic Mannequin state enum (`0x4c3790`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MknState {
    #[default]
    Stay = 0,
    Event = 1,
    Fall = 2,
    Fall2 = 3,
    Wander = 4,
    Precaution = 5,
    Chase = 6,
    Circle = 7,
    Attack = 8,
    Damage = 9,
    Confuse = 10,
    Down = 11,
    Dying = 12,
}

/// Mannequin controller and state machine.
#[derive(Debug, Clone)]
pub struct MannequinController {
    pub id: u32,
    pub position: Vec3,
    pub origin_pos: Vec3,
    pub yaw: f32,
    pub state: MknState,
    pub stats: CreatureStats,
    pub alert_timer: f32,
    pub state_timer: f32,
    pub is_kick_next: bool,
    pub sensory: SensoryProfile,
}

impl MannequinController {
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        let (hp, end, end_recovery) = match difficulty {
            DifficultyLevel::Beginner => (280.0, 7.0, 0.0),
            DifficultyLevel::Easy => (280.0, 133.0, 7.0),
            DifficultyLevel::Normal => (490.0, 200.0, 21.0),
            DifficultyLevel::Hard => (1050.0, 266.0, 42.0),
        };

        Self {
            id,
            position,
            origin_pos: position,
            yaw,
            state: MknState::Stay, // Default dormant/freeze in dark
            stats: CreatureStats::new(hp, end, end_recovery),
            alert_timer: 0.0,
            state_timer: 0.0,
            is_kick_next: false,
            sensory: SensoryProfile::mannequin(),
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        if self.state == MknState::Dying {
            return;
        }

        self.stats.update(dt);
        self.state_timer += dt;

        if self.stats.health <= 0.0 {
            self.state = MknState::Dying;
            return;
        }

        if self.state == MknState::Down {
            if self.stats.down_timer <= 0.0 {
                self.state = MknState::Chase;
                self.state_timer = 0.0;
            }
            return;
        }

        if self.stats.flinch_timer > 0.0 {
            self.state = MknState::Damage;
            return;
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        // Flashlight beam detection for waking up from STAY (`0x4c0eb0`)
        let lit_by_player = is_lit_by_flashlight(
            self.position,
            target.position,
            target.yaw,
            target.is_flashlight_on,
        );

        match self.state {
            MknState::Stay => {
                // Mannequin stands completely still like a prop until:
                // 1) Flashlight shines directly on it within 2,250 units
                // 2) Player approaches within 900 units
                if (lit_by_player && dist < 2250.0) || dist < 900.0 {
                    if dist <= 1000.0 {
                        // Instant strike if player is immediately adjacent
                        self.state = MknState::Attack;
                    } else {
                        self.state = MknState::Precaution;
                    }
                    self.state_timer = 0.0;
                }
            }

            MknState::Wander => {
                let odx = self.position.x - self.origin_pos.x;
                let odz = self.position.z - self.origin_pos.z;
                if odx * odx + odz * odz > 5000.0 * 5000.0 {
                    self.yaw = (-odx).atan2(-odz);
                }

                self.position.x += self.yaw.sin() * 576.0 * dt;
                self.position.z += self.yaw.cos() * 576.0 * dt;

                let (notice, alert) = evaluate_sensory(
                    &self.sensory,
                    self.position,
                    self.yaw,
                    self.alert_timer,
                    target,
                    lighting,
                    difficulty,
                    dt,
                );
                self.alert_timer = alert;

                if notice != SensoryNotice::None {
                    self.state = MknState::Precaution;
                    self.state_timer = 0.0;
                }
            }

            MknState::Precaution => {
                self.turn_towards(target_dir, 3.5 * dt);
                if self.state_timer > 1.0 {
                    self.state = MknState::Chase;
                    self.state_timer = 0.0;
                }
            }

            MknState::Chase => {
                self.turn_towards(target_dir, 3.5 * dt);

                if dist <= self.sensory.attack_range {
                    let diff = (normalize_angle(target_dir - self.yaw)).abs();
                    if diff <= self.sensory.attack_half_angle {
                        self.state = MknState::Attack;
                        self.state_timer = 0.0;
                        return;
                    }
                }

                // Advance at 576 units/s
                self.position.x += self.yaw.sin() * 576.0 * dt;
                self.position.z += self.yaw.cos() * 576.0 * dt;

                // If player breaks line of sight or turns away, enter Circle flank mode (`0x4c2a90`)
                if self.state_timer > 4.0 && dist > 1500.0 {
                    self.state = MknState::Circle;
                    self.state_timer = 0.0;
                }
            }

            MknState::Circle => {
                // Circle flank: move to point 1000 units away, 120° offset from player's facing
                let flank_angle = target.yaw + 120.0 * PI / 180.0;
                let flank_goal = Vec3::new(
                    target.position.x + flank_angle.sin() * 1000.0,
                    target.position.y,
                    target.position.z + flank_angle.cos() * 1000.0,
                );
                let fdx = flank_goal.x - self.position.x;
                let fdz = flank_goal.z - self.position.z;
                self.turn_towards(fdx.atan2(fdz), 4.0 * dt);

                self.position.x += self.yaw.sin() * 576.0 * dt;
                self.position.z += self.yaw.cos() * 576.0 * dt;

                if self.state_timer > 3.0 || dist <= self.sensory.attack_range {
                    self.state = MknState::Chase;
                    self.state_timer = 0.0;
                }
            }

            MknState::Attack => {
                // Attack animation duration ~1.0s
                if self.state_timer >= 1.0 {
                    self.is_kick_next = !self.is_kick_next; // Alternate arm swing and kick
                    self.state = MknState::Chase;
                    self.state_timer = 0.0;
                }
            }

            MknState::Damage if self.stats.flinch_timer <= 0.0 => {
                self.state = MknState::Chase;
                self.state_timer = 0.0;
            }

            _ => {}
        }
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }

    pub fn receive_hit(&mut self, ap: f32, sp: f32, roll: f32) {
        let knocked_down = self.stats.take_damage(ap, sp, roll);
        if self.stats.health <= 0.0 {
            self.state = MknState::Dying;
        } else if knocked_down {
            self.state = MknState::Down;
        } else {
            self.state = MknState::Damage;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mannequin_dormant_stay_and_wake() {
        let mut mkn =
            MannequinController::new(1, Vec3::new(0.0, 0.0, 0.0), 0.0, DifficultyLevel::Normal);
        assert_eq!(mkn.state, MknState::Stay);

        // Player is at 2000 units with flashlight OFF: Mannequin stays dormant
        let target_dark = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2000.0), PI, false, false);
        mkn.update(
            &target_dark,
            RoomLighting::DarkLightOff,
            DifficultyLevel::Normal,
            0.1,
        );
        assert_eq!(mkn.state, MknState::Stay);

        // Player shines flashlight towards mannequin from 2000 units (facing PI = -Z towards 0,0,0)
        let target_lit = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2000.0), PI, true, false);
        mkn.update(
            &target_lit,
            RoomLighting::DarkLightOn,
            DifficultyLevel::Normal,
            0.1,
        );
        assert_eq!(mkn.state, MknState::Precaution);
    }
}
