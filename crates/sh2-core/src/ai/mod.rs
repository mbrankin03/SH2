//! High-level Enemy AI subsystem orchestrating all creature instances, sensory perception,
//! radio noise modulation, and the Maria companion system (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c`, `src/Chacter/m3_maria_sub.c`
//! - PC executable `sh2pc.exe`:
//!   - `0x4a88e0`: `enExecTask` - global enemy update loop across active slots.
//!   - `0x4aa5e0`: `enInitData` - spawn management.
//!   - `0x542ad0`: Companion targeting arbitration.

pub mod creatures;
pub mod maria;
pub mod radio;
pub mod sensory;

use crate::ai::creatures::creeper::CreeperController;
use crate::ai::creatures::lying_figure::LyingFigureController;
use crate::ai::creatures::mannequin::MannequinController;
use crate::ai::creatures::nurse::NurseController;
use crate::ai::creatures::pyramid_head::PyramidHeadController;
use crate::ai::creatures::EnemySpecies;
use crate::ai::maria::MariaCompanionController;
use crate::ai::radio::{RadioNoiseEvaluator, RadioStaticOutput};
use crate::ai::sensory::{DifficultyLevel, PerceivableTarget, RoomLighting};
use glam::Vec3;

/// Unified enemy instance wrapper supporting all core archetype state machines.
#[derive(Debug, Clone)]
pub enum ActiveEnemy {
    LyingFigure(LyingFigureController),
    Mannequin(MannequinController),
    Nurse(NurseController),
    Creeper(CreeperController),
    PyramidHead(PyramidHeadController),
}

impl ActiveEnemy {
    pub fn id(&self) -> u32 {
        match self {
            Self::LyingFigure(e) => e.id,
            Self::Mannequin(e) => e.id,
            Self::Nurse(e) => e.id,
            Self::Creeper(e) => e.id,
            Self::PyramidHead(e) => e.id,
        }
    }

    pub fn position(&self) -> Vec3 {
        match self {
            Self::LyingFigure(e) => e.position,
            Self::Mannequin(e) => e.position,
            Self::Nurse(e) => e.position,
            Self::Creeper(e) => e.position,
            Self::PyramidHead(e) => e.position,
        }
    }

    pub fn yaw(&self) -> f32 {
        match self {
            Self::LyingFigure(e) => e.yaw,
            Self::Mannequin(e) => e.yaw,
            Self::Nurse(e) => e.yaw,
            Self::Creeper(e) => e.yaw,
            Self::PyramidHead(e) => e.yaw,
        }
    }

    pub fn is_dead(&self) -> bool {
        match self {
            Self::LyingFigure(e) => e.stats.health <= 0.0,
            Self::Mannequin(e) => e.stats.health <= 0.0,
            Self::Nurse(e) => e.stats.health <= 0.0,
            Self::Creeper(e) => e.stats.health <= 0.0,
            Self::PyramidHead(e) => e.stats.health <= 0.0,
        }
    }

    pub fn is_downed(&self) -> bool {
        match self {
            Self::LyingFigure(e) => e.stats.down_timer > 0.0,
            Self::Mannequin(e) => e.stats.down_timer > 0.0,
            Self::Nurse(e) => e.stats.down_timer > 0.0,
            Self::Creeper(_) => false,
            Self::PyramidHead(_) => false,
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        match self {
            Self::LyingFigure(e) => e.update(target, lighting, difficulty, dt),
            Self::Mannequin(e) => e.update(target, lighting, difficulty, dt),
            Self::Nurse(e) => e.update(target, lighting, difficulty, dt),
            Self::Creeper(e) => e.update(target, dt),
            Self::PyramidHead(e) => e.update(target, lighting, difficulty, dt),
        }
    }
}

/// Global AI and Companion subsystem (`en_common.c` / `m3_maria_sub.c`).
#[derive(Debug, Clone)]
pub struct AiSubsystem {
    /// Active enemies currently spawned in the stage/room (max 24-32 slots).
    pub enemies: Vec<ActiveEnemy>,
    /// Maria companion controller.
    pub maria: MariaCompanionController,
    /// Radio static evaluator.
    pub radio_evaluator: RadioNoiseEvaluator,
    /// Latest radio output state.
    pub radio_output: RadioStaticOutput,
    /// Current room lighting condition.
    pub lighting: RoomLighting,
    /// Game difficulty setting.
    pub difficulty: DifficultyLevel,
}

impl Default for AiSubsystem {
    fn default() -> Self {
        Self {
            enemies: Vec::with_capacity(32),
            maria: MariaCompanionController::default(),
            radio_evaluator: RadioNoiseEvaluator::default(),
            radio_output: RadioStaticOutput::default(),
            lighting: RoomLighting::Lit,
            difficulty: DifficultyLevel::Normal,
        }
    }
}

impl AiSubsystem {
    /// Clear all enemies and reset when entering a new room (`enInitData`).
    pub fn clear_enemies(&mut self) {
        self.enemies.clear();
    }

    /// Spawn an enemy into the active world.
    pub fn spawn_enemy(&mut self, species: EnemySpecies, id: u32, position: Vec3, yaw: f32) {
        let enemy = match species {
            EnemySpecies::LyingFigure => ActiveEnemy::LyingFigure(LyingFigureController::new(
                id,
                position,
                yaw,
                self.difficulty,
            )),
            EnemySpecies::Mannequin => {
                ActiveEnemy::Mannequin(MannequinController::new(id, position, yaw, self.difficulty))
            }
            EnemySpecies::Nurse => {
                ActiveEnemy::Nurse(NurseController::new(id, position, yaw, self.difficulty))
            }
            EnemySpecies::Creeper => {
                ActiveEnemy::Creeper(CreeperController::new(id, position, yaw))
            }
            EnemySpecies::PyramidHead => {
                ActiveEnemy::PyramidHead(PyramidHeadController::new(id, position, yaw))
            }
        };
        self.enemies.push(enemy);
    }

    /// Primary 60 Hz AI simulation update loop (`enExecTask`).
    pub fn update(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        dt: f32,
    ) {
        // Collect active enemy positions for radio and companion awareness
        let mut active_hostile_positions = Vec::with_capacity(self.enemies.len());
        for e in &self.enemies {
            if !e.is_dead() {
                active_hostile_positions.push(e.position());
            }
        }

        // 1. Update Maria companion
        self.maria
            .update(player_pos, player_yaw, &active_hostile_positions, dt);

        // 2. Target arbitration and enemy updates (`0x542ad0`)
        let player_target =
            PerceivableTarget::new(player_pos, player_yaw, is_flashlight_on, is_running);

        let maria_target =
            PerceivableTarget::new(self.maria.position, self.maria.yaw, false, false);

        for enemy in &mut self.enemies {
            let target = if self
                .maria
                .should_enemy_target_maria(enemy.position(), player_pos)
            {
                &maria_target
            } else {
                &player_target
            };

            enemy.update(target, self.lighting, self.difficulty, dt);
        }

        // 3. Update Pocket Radio proximity static
        self.radio_output =
            self.radio_evaluator
                .evaluate(radio_enabled, player_pos, &active_hostile_positions, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_subsystem_lifecycle_and_radio() {
        let mut ai = AiSubsystem::default();
        let player_pos = Vec3::new(0.0, 0.0, 0.0);

        // Spawn Lying Figure 1000 units away
        ai.spawn_enemy(
            EnemySpecies::LyingFigure,
            1,
            Vec3::new(0.0, 0.0, 1000.0),
            std::f32::consts::PI,
        );
        assert_eq!(ai.enemies.len(), 1);

        // Tick AI
        ai.update(player_pos, 0.0, true, false, true, 0.016);

        // Radio static should be active and audible at 1000 units
        assert!(ai.radio_output.volume > 0.8);
        assert!((ai.radio_output.nearest_enemy_dist - 1000.0).abs() < 50.0);
    }
}
