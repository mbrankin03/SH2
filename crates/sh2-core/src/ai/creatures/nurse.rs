//! Bubble Head Nurse (`chr/nse`, kind 0x207) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_nse.c` (`enNSECtrlWander`, `enNSECtrlPrecaution`, `enNSECtrlChase`, `enNSECtrlAttack`, `enNSECtrlDamage`, `enNSECtrlDown`)
//! - PC executable `sh2pc.exe`:
//!   - `0x4bb390`: Nurse state table (`NSE_WANDER`, `NSE_PRECAUTION`, `NSE_CHASE`, `NSE_ATTACK`, `NSE_DAMAGE`, `NSE_CONFUSE`, `NSE_DOWN`, `NSE_DYING`).
//!   - `0x892e18`: Sight table (4000/4000/3000/1000 by light, 80° view angle, attack 1500 within 5° or 900 within 30°).
//!   - Pipe swing attacks: reach 400–600 units, AP 12–18.
//!   - Erratic twitching locomotion: staggered burst steps alternating with sudden pauses.

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{
    evaluate_sensory, normalize_angle, DifficultyLevel, PerceivableTarget, RoomLighting,
    SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Authentic Nurse state enum (`0x4bb390`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NseState {
    #[default]
    Wander = 0,
    Precaution = 1,
    Chase = 2,
    Attack = 3,
    Damage = 4,
    Confuse = 5,
    Down = 6,
    Dying = 7,
}

/// Bubble Head Nurse controller and state machine.
#[derive(Debug, Clone)]
pub struct NurseController {
    pub id: u32,
    pub position: Vec3,
    pub origin_pos: Vec3,
    pub yaw: f32,
    pub state: NseState,
    pub stats: CreatureStats,
    pub alert_timer: f32,
    pub state_timer: f32,
    pub twitch_timer: f32,
    pub sensory: SensoryProfile,
}

impl NurseController {
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        let (hp, end, end_recovery) = match difficulty {
            DifficultyLevel::Beginner => (300.0, 10.0, 0.0),
            DifficultyLevel::Easy => (400.0, 180.0, 10.0),
            DifficultyLevel::Normal => (600.0, 260.0, 25.0),
            DifficultyLevel::Hard => (1200.0, 350.0, 50.0),
        };

        Self {
            id,
            position,
            origin_pos: position,
            yaw,
            state: NseState::Wander,
            stats: CreatureStats::new(hp, end, end_recovery),
            alert_timer: 0.0,
            state_timer: 0.0,
            twitch_timer: 0.0,
            sensory: SensoryProfile::nurse(),
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        if self.state == NseState::Dying {
            return;
        }

        self.stats.update(dt);
        self.state_timer += dt;
        self.twitch_timer += dt;

        if self.stats.health <= 0.0 {
            self.state = NseState::Dying;
            return;
        }

        if self.state == NseState::Down {
            if self.stats.down_timer <= 0.0 {
                self.state = NseState::Chase;
                self.state_timer = 0.0;
            }
            return;
        }

        if self.stats.flinch_timer > 0.0 {
            self.state = NseState::Damage;
            return;
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

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

        match self.state {
            NseState::Wander => {
                let odx = self.position.x - self.origin_pos.x;
                let odz = self.position.z - self.origin_pos.z;
                if odx * odx + odz * odz > 4000.0 * 4000.0 {
                    self.yaw = (-odx).atan2(-odz);
                }

                // Staggered twitch advance
                let twitch_speed = if (self.twitch_timer % 1.5) < 0.9 {
                    480.0
                } else {
                    60.0 // Twitch hesitation
                };
                self.position.x += self.yaw.sin() * twitch_speed * dt;
                self.position.z += self.yaw.cos() * twitch_speed * dt;

                if notice != SensoryNotice::None {
                    self.state = NseState::Precaution;
                    self.state_timer = 0.0;
                }
            }

            NseState::Precaution => {
                self.turn_towards(target_dir, 3.2 * dt);
                if self.state_timer > 1.2 || notice == SensoryNotice::InAttackRange {
                    self.state = NseState::Chase;
                    self.state_timer = 0.0;
                }
            }

            NseState::Chase => {
                self.turn_towards(target_dir, 3.2 * dt);

                if dist <= self.sensory.attack_range {
                    self.state = NseState::Attack;
                    self.state_timer = 0.0;
                    return;
                }

                // Twitch advance at player
                let twitch_speed = if (self.twitch_timer % 1.2) < 0.8 {
                    650.0 // Stagger sprint
                } else {
                    120.0
                };
                self.position.x += self.yaw.sin() * twitch_speed * dt;
                self.position.z += self.yaw.cos() * twitch_speed * dt;

                if notice == SensoryNotice::None && self.state_timer > 5.0 {
                    self.state = NseState::Wander;
                    self.state_timer = 0.0;
                }
            }

            NseState::Attack => {
                // Pipe swing attack duration ~1.1s
                if self.state_timer >= 1.1 {
                    self.state = NseState::Chase;
                    self.state_timer = 0.0;
                }
            }

            NseState::Damage if self.stats.flinch_timer <= 0.0 => {
                self.state = NseState::Chase;
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
            self.state = NseState::Dying;
        } else if knocked_down {
            self.state = NseState::Down;
        } else {
            self.state = NseState::Damage;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nurse_lifecycle() {
        let mut nurse = NurseController::new(1, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        assert_eq!(nurse.state, NseState::Wander);

        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1000.0), 0.0, false, false);
        nurse.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(nurse.state, NseState::Precaution);
    }
}
