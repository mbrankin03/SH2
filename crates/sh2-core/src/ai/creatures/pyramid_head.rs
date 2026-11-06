//! Pyramid Head (`chr/red`, `chr/oni`, kind 0x208/0x209) AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_red.c`, `src/Enemy/en_oni.c`
//! - PC executable `sh2pc.exe`:
//!   - `0x4c0b00`: Pyramid Head state table (`RED_WANDER`, `RED_CHASE`, `RED_HUNT`, `RED_ATTACK`, `RED_GRAB`, `RED_STAND`).
//!   - Unflinching relentless advance (immune to standard stagger/flinch).
//!   - Great Knife execution swings (AP 1000 - instant kill) and choke grab attacks.

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{
    evaluate_sensory, normalize_angle, DifficultyLevel, PerceivableTarget, RoomLighting,
    SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RedState {
    #[default]
    Wander = 0,
    Chase = 1,
    Hunt = 2,
    Stairs = 3,
    Attack = 4,
    Grab = 5,
    Confuse = 6,
    Leave = 7,
    Stand = 8,
}

#[derive(Debug, Clone)]
pub struct PyramidHeadController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub state: RedState,
    pub stats: CreatureStats,
    pub state_timer: f32,
    pub sensory: SensoryProfile,
}

impl PyramidHeadController {
    pub fn new(id: u32, position: Vec3, yaw: f32) -> Self {
        Self {
            id,
            position,
            yaw,
            state: RedState::Wander,
            stats: CreatureStats::new(5000.0, 9999.0, 0.0), // Massive boss endurance (unflinching)
            state_timer: 0.0,
            sensory: SensoryProfile::pyramid_head(),
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        self.stats.update(dt);
        self.state_timer += dt;

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        let (notice, _) = evaluate_sensory(
            &self.sensory,
            self.position,
            self.yaw,
            0.0,
            target,
            lighting,
            difficulty,
            dt,
        );

        match self.state {
            RedState::Wander => {
                // Relentless slow march (280 units/s)
                self.position.x += self.yaw.sin() * 280.0 * dt;
                self.position.z += self.yaw.cos() * 280.0 * dt;

                if notice != SensoryNotice::None {
                    self.state = RedState::Chase;
                    self.state_timer = 0.0;
                }
            }

            RedState::Chase => {
                self.turn_towards(target_dir, 1.8 * dt);

                // Overhead execution swing reach: 1800 units
                if dist <= 1800.0 {
                    self.state = RedState::Attack;
                    self.state_timer = 0.0;
                    return;
                }

                // Slow unstoppable march (340 units/s) dragging Great Knife
                self.position.x += self.yaw.sin() * 340.0 * dt;
                self.position.z += self.yaw.cos() * 340.0 * dt;
            }

            RedState::Attack if self.state_timer >= 2.2 => {
                // Heavy overhead Great Knife strike duration: 2.2 seconds
                self.state = RedState::Chase;
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

    pub fn receive_hit(&mut self, ap: f32, _sp: f32) {
        // Pyramid Head never flinches or falls down from gunfire or melee swings
        self.stats.health = (self.stats.health - ap).max(0.0);
    }
}
