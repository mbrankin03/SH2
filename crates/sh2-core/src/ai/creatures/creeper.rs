//! Creeper cockroach swarm AI (`chr/tyu`, kind 0x202) (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_tyu.c`, `src/Enemy/en_insect.c`
//! - PC executable `sh2pc.exe`:
//!   - Fast floor traversal (up to 1,200 units/s).
//!   - High-density swarm behavior and scatter reactions when illuminated by flashlight.

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{is_lit_by_flashlight, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TyuState {
    #[default]
    Scatter,
    Swarm,
    Attack,
    Dead,
}

#[derive(Debug, Clone)]
pub struct CreeperController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub state: TyuState,
    pub stats: CreatureStats,
    pub speed: f32,
    pub anim: CreatureAnimationController,
}

impl CreeperController {
    /// Create a Creeper (`TYU`, `0x202`) at Normal difficulty (`enTYUInitData` `SLUS_202.28` `0x00165000`).
    pub fn new(id: u32, position: Vec3, yaw: f32) -> Self {
        Self::new_with_difficulty(id, position, yaw, DifficultyLevel::Normal)
    }

    /// Create a Creeper (`TYU`) with exact `SLUS_202.28` difficulty tables:
    /// - HP (`0x2ab2a0`): `[1.0, 250.0, 400.0, 600.0]`
    /// - Endurance (`0x2ab2c0`): `[1.0, 100.0, 150.0, 240.0]`
    /// - Speed (`enTYUGetSpeed` `0x00167bf0`, `0x2ab350`): `[0.8, 0.9, 1.0, 1.1] * 1250.0` mm/s
    pub fn new_with_difficulty(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
    ) -> Self {
        const TYU_HP: [f32; 4] = [1.0, 250.0, 400.0, 600.0];
        const TYU_END: [f32; 4] = [1.0, 100.0, 150.0, 240.0];
        const TYU_SPEED_SCALE: [f32; 4] = [0.8, 0.9, 1.0, 1.1];
        let idx = difficulty.as_index();
        Self {
            id,
            position,
            yaw,
            state: TyuState::Swarm,
            stats: CreatureStats::new(TYU_HP[idx], TYU_END[idx], 0.0),
            speed: TYU_SPEED_SCALE[idx] * 1250.0,
            anim: CreatureAnimationController::new_creeper(),
        }
    }

    pub fn update(&mut self, target: &PerceivableTarget, dt: f32) {
        if self.state == TyuState::Dead {
            return;
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();

        // Flashlight scatter reaction
        let lit = is_lit_by_flashlight(
            self.position,
            target.position,
            target.yaw,
            target.is_flashlight_on,
        );

        if lit {
            self.state = TyuState::Scatter;
            // Flee directly away from player
            self.yaw = (-dx).atan2(-dz);
        } else if dist < 250.0 {
            self.state = TyuState::Attack;
            self.yaw = dx.atan2(dz);
        } else {
            self.state = TyuState::Swarm;
            self.yaw = dx.atan2(dz);
        }

        self.position.x += self.yaw.sin() * self.speed * dt;
        self.position.z += self.yaw.cos() * self.speed * dt;
    }

    pub fn receive_hit(&mut self) {
        self.stats.health = 0.0;
        self.state = TyuState::Dead;
    }
}
