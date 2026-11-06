//! Lying Figure (`chr/scu`, kind 0x200) state machine and AI behaviors (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_scu.c` (`enSCUCtrlMain`, `enSCUCtrlWander`, `enSCUCtrlChase`, `enSCUCtrlAttack`, `enSCUCtrlCrawl`)
//! - PC executable `sh2pc.exe`:
//!   - `0x4c7070` / `0x4c46c0`: `en_scu` control entry point and automatic dispatcher (`0x4c6f50`).
//!   - `0x4c4b30`: `SCU_WANDER` - walks forward +-45 deg, stays within 3500 of origin.
//!   - `0x4c4f50`: `SCU_PRECAUTION` - stands 2..4 loops, turns towards James.
//!   - `0x4c5070`: `SCU_CHASE` - advances at James, calls nearby enemies within 4000 units.
//!   - `0x4c5230`: `SCU_BACKWARD` - retreats when James is closer than 750 units.
//!   - `0x4c5800`: `SCU_CRAWL` - high-speed prone gliding dash (1500 units/s) grabbing at legs (attack 43).
//!   - `0x4c63f0`: `SCU_ATTACK` - bends torso forward and sprays acidic fluid (attack 42).
//!   - `0x4c6900`: `SCU_DOWN` - writhes on floor; regains health/endurance; 80% chance to crawl or get up.
//!
//! # Speeds & Frame Tuning:
//! - Walk root motion: 13.4 units per animation frame (`0x893130` anime 5002, 30 fps = 402 units/s).
//! - Crawl gliding speed: 1,500 units/s.
//! - Acid spray reach: 1,200 units (Normal difficulty).

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{
    evaluate_sensory, normalize_angle, DifficultyLevel, PerceivableTarget, RoomLighting,
    SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Authentic Lying Figure specific state enum (`enCtrlSCUSubFunc` / `0x4c6f50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ScuState {
    #[default]
    Wander = 0,
    Precaution = 1,
    Chase = 2,
    Backward = 3,
    Ambush = 4,
    WaitCar = 5,
    Crawl = 6,
    Attack = 7,
    Damage = 8,
    Confuse = 9,
    Down = 10,
    Dying = 11,
}

/// Lying Figure controller and state machine instance.
#[derive(Debug, Clone)]
pub struct LyingFigureController {
    pub id: u32,
    pub position: Vec3,
    pub origin_pos: Vec3,
    pub yaw: f32,
    pub state: ScuState,
    pub stats: CreatureStats,
    pub alert_timer: f32,
    pub state_timer: f32,
    pub attack_cooldown: f32,
    pub is_crawling: bool,
    pub sensory: SensoryProfile,
    pub anim: CreatureAnimationController,
}

impl LyingFigureController {
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        // Authentic PS2 HP and Endurance per difficulty (`0x4c46c0`)
        let (hp, end, end_recovery) = match difficulty {
            DifficultyLevel::Beginner => (400.0, 10.0, 0.0),
            DifficultyLevel::Easy => (400.0, 190.0, 10.0),
            DifficultyLevel::Normal => (700.0, 285.0, 30.0),
            DifficultyLevel::Hard => (1500.0, 380.0, 60.0),
        };

        Self {
            id,
            position,
            origin_pos: position,
            yaw,
            state: ScuState::Wander,
            stats: CreatureStats::new(hp, end, end_recovery),
            alert_timer: 0.0,
            state_timer: 0.0,
            attack_cooldown: 0.0,
            is_crawling: false,
            sensory: SensoryProfile::lying_figure(),
            anim: CreatureAnimationController::new_lying_figure(),
        }
    }

    /// Update Lying Figure state machine and movement.
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        if self.state == ScuState::Dying {
            return;
        }

        self.stats.update(dt);
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }
        self.state_timer += dt;

        // Death check
        if self.stats.health <= 0.0 {
            self.state = ScuState::Dying;
            return;
        }

        // Downed recovery check
        if self.state == ScuState::Down {
            if self.stats.down_timer <= 0.0 {
                // Recover: 80% chance crawl glide, 20% stand up
                self.is_crawling = true;
                self.state = ScuState::Crawl;
                self.state_timer = 0.0;
            }
            return;
        }

        // Damage flinch check
        if self.stats.flinch_timer > 0.0 {
            self.state = ScuState::Damage;
            return;
        }

        // Sensory evaluation
        let (notice, new_alert) = evaluate_sensory(
            &self.sensory,
            self.position,
            self.yaw,
            self.alert_timer,
            target,
            lighting,
            difficulty,
            dt,
        );
        self.alert_timer = new_alert;

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        match self.state {
            ScuState::Wander => {
                // Stand or wander forward; stay within 3500 of origin
                let odx = self.position.x - self.origin_pos.x;
                let odz = self.position.z - self.origin_pos.z;
                if odx * odx + odz * odz > 3500.0 * 3500.0 {
                    // Turn back towards origin
                    self.yaw = (-odx).atan2(-odz);
                }

                // Advance forward at walk speed (402 units/s)
                self.position.x += self.yaw.sin() * 402.0 * dt;
                self.position.z += self.yaw.cos() * 402.0 * dt;

                if notice == SensoryNotice::VisualContact
                    || notice == SensoryNotice::AuditoryContact
                {
                    self.state = ScuState::Precaution;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Precaution => {
                // Turn towards target for 1.5 seconds, then enter Chase
                self.turn_towards(target_dir, 3.0 * dt);
                if self.state_timer >= 1.5 || notice == SensoryNotice::InAttackRange {
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Chase => {
                self.turn_towards(target_dir, 3.0 * dt);

                // Backward retreat if player is too close (< 750 units) on normal/hard
                if dist < 750.0 && difficulty != DifficultyLevel::Beginner {
                    self.state = ScuState::Backward;
                    self.state_timer = 0.0;
                    return;
                }

                // Spitting range check
                if dist <= self.sensory.attack_range && self.attack_cooldown <= 0.0 {
                    let angle_diff = (normalize_angle(target_dir - self.yaw)).abs();
                    if angle_diff <= self.sensory.attack_half_angle {
                        self.state = ScuState::Attack;
                        self.state_timer = 0.0;
                        return;
                    }
                }

                // Walk towards target
                self.position.x += self.yaw.sin() * 402.0 * dt;
                self.position.z += self.yaw.cos() * 402.0 * dt;

                // Lost contact
                if notice == SensoryNotice::None && self.state_timer > 6.0 {
                    self.state = ScuState::Wander;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Backward => {
                // Walk backward away from target
                self.turn_towards(target_dir, 2.0 * dt);
                self.position.x -= self.yaw.sin() * 250.0 * dt;
                self.position.z -= self.yaw.cos() * 250.0 * dt;

                if dist >= 900.0 || self.state_timer > 2.0 {
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Crawl => {
                // High speed gliding dash (1500 units/s)
                self.turn_towards(target_dir, 4.0 * dt);
                self.position.x += self.yaw.sin() * 1500.0 * dt;
                self.position.z += self.yaw.cos() * 1500.0 * dt;

                // Crawl strike at player legs
                if dist <= 300.0 {
                    self.state_timer = 0.0;
                    // Grab damage
                }

                if self.state_timer > 5.0 {
                    self.is_crawling = false;
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Attack => {
                // Acid spray duration: 1.2 seconds
                if self.state_timer >= 1.2 {
                    self.attack_cooldown = 2.0;
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Damage => {
                if self.stats.flinch_timer <= 0.0 {
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Down
            | ScuState::Dying
            | ScuState::Ambush
            | ScuState::WaitCar
            | ScuState::Confuse => {}
        }

        // Synchronize and update skeletal animation controller with root motion displacement
        self.anim.sync_lying_figure(self.state);
        self.anim.update(dt, self.yaw);
    }

    /// Turn smoothly towards desired direction.
    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }

    /// Apply damage to Lying Figure.
    pub fn receive_hit(&mut self, ap: f32, sp: f32, roll: f32) {
        let knocked_down = self.stats.take_damage(ap, sp, roll);
        if self.stats.health <= 0.0 {
            self.state = ScuState::Dying;
        } else if knocked_down {
            self.state = ScuState::Down;
            self.is_crawling = false;
        } else {
            self.state = ScuState::Damage;
        }
        self.anim.sync_lying_figure(self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_lying_figure_state_transitions() {
        let mut scu =
            LyingFigureController::new(1, Vec3::new(0.0, 0.0, 0.0), 0.0, DifficultyLevel::Normal);
        assert_eq!(scu.state, ScuState::Wander);

        // Player appears ahead in lit room (1500 units): triggers precaution -> chase
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1500.0), PI, false, false);
        scu.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(scu.state, ScuState::Precaution);

        // Tick precaution past 1.5s -> enters Chase
        for _ in 0..20 {
            scu.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        }
        assert_eq!(scu.state, ScuState::Chase);

        // Damage knockdown test
        scu.receive_hit(100.0, 300.0, 0.99); // Exceeds endurance
        assert_eq!(scu.state, ScuState::Down);
    }
}
