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
pub mod npcs;
pub mod pathfinding;
pub mod radio;
pub mod sensory;

use crate::ai::creatures::abstract_daddy::AbstractDaddyController;
use crate::ai::creatures::creeper::CreeperController;
use crate::ai::creatures::eddie::EddieController;
use crate::ai::creatures::flesh_lips::{FleshLipsController, IkeState};
use crate::ai::creatures::insects::{InsectState, RoomInsectController};
use crate::ai::creatures::lying_figure::LyingFigureController;
use crate::ai::creatures::mandarin::MandarinController;
use crate::ai::creatures::mannequin::MannequinController;
use crate::ai::creatures::mary::MaryBossController;
use crate::ai::creatures::nurse::NurseController;
use crate::ai::creatures::pyramid_head::PyramidHeadController;
use crate::ai::creatures::twin_pyramid_head::{TwinPartnerSnapshot, TwinPyramidHeadController};
use crate::ai::creatures::EnemySpecies;
use crate::ai::maria::MariaCompanionController;
use crate::ai::radio::{RadioNoiseEvaluator, RadioStaticOutput};
use crate::ai::sensory::{DifficultyLevel, PerceivableTarget, RoomLighting};
use glam::Vec3;

/// Shared cross-entity context passed into [`ActiveEnemy::update_with_context`].
#[derive(Debug, Clone, Copy)]
pub struct EnemyUpdateContext {
    pub player_radius_mm: f32,
    pub player_ammo_empty: bool,
    pub player_spraying: bool,
    pub twin_partner: Option<TwinPartnerSnapshot>,
    pub other_flesh_lips_alive: usize,
    pub other_flesh_lips_attacking: bool,
    pub mandarin_near_other_turn: Option<f32>,
    pub mandarin_over_grating: bool,
    pub insect_leader: Option<(InsectState, Vec3)>,
}

impl Default for EnemyUpdateContext {
    fn default() -> Self {
        Self {
            player_radius_mm: 200.0,
            player_ammo_empty: false,
            player_spraying: false,
            twin_partner: None,
            other_flesh_lips_alive: 0,
            other_flesh_lips_attacking: false,
            mandarin_near_other_turn: None,
            mandarin_over_grating: true,
            insect_leader: None,
        }
    }
}
#[derive(Debug, Clone)]
pub enum ActiveEnemy {
    LyingFigure(LyingFigureController),
    Mannequin(MannequinController),
    Nurse(NurseController),
    Creeper(CreeperController),
    PyramidHead(PyramidHeadController),
    AbstractDaddy(AbstractDaddyController),
    FleshLips(FleshLipsController),
    Mandarin(MandarinController),
    Eddie(EddieController),
    TwinPyramidHead(TwinPyramidHeadController),
    MaryBoss(MaryBossController),
    RoomInsect(RoomInsectController),
}

impl ActiveEnemy {
    pub fn id(&self) -> u32 {
        match self {
            Self::LyingFigure(e) => e.id,
            Self::Mannequin(e) => e.id,
            Self::Nurse(e) => e.id,
            Self::Creeper(e) => e.id,
            Self::PyramidHead(e) => e.id,
            Self::AbstractDaddy(e) => e.id,
            Self::FleshLips(e) => e.id,
            Self::Mandarin(e) => e.id,
            Self::Eddie(e) => e.id,
            Self::TwinPyramidHead(e) => e.id,
            Self::MaryBoss(e) => e.id,
            Self::RoomInsect(e) => e.id,
        }
    }

    pub fn position(&self) -> Vec3 {
        match self {
            Self::LyingFigure(e) => e.position,
            Self::Mannequin(e) => e.position,
            Self::Nurse(e) => e.position,
            Self::Creeper(e) => e.position,
            Self::PyramidHead(e) => e.position,
            Self::AbstractDaddy(e) => e.position,
            Self::FleshLips(e) => e.position,
            Self::Mandarin(e) => e.position,
            Self::Eddie(e) => e.position,
            Self::TwinPyramidHead(e) => e.position,
            Self::MaryBoss(e) => e.position,
            Self::RoomInsect(e) => e.position,
        }
    }

    pub fn yaw(&self) -> f32 {
        match self {
            Self::LyingFigure(e) => e.yaw,
            Self::Mannequin(e) => e.yaw,
            Self::Nurse(e) => e.yaw,
            Self::Creeper(e) => e.yaw,
            Self::PyramidHead(e) => e.yaw,
            Self::AbstractDaddy(e) => e.yaw,
            Self::FleshLips(e) => e.yaw,
            Self::Mandarin(e) => e.yaw,
            Self::Eddie(e) => e.yaw,
            Self::TwinPyramidHead(e) => e.yaw,
            Self::MaryBoss(e) => e.yaw,
            Self::RoomInsect(e) => e.yaw,
        }
    }

    pub fn is_dead(&self) -> bool {
        match self {
            Self::LyingFigure(e) => e.stats.health <= 0.0,
            Self::Mannequin(e) => e.stats.health <= 0.0,
            Self::Nurse(e) => e.stats.health <= 0.0,
            Self::Creeper(e) => e.stats.health <= 0.0,
            Self::PyramidHead(e) => e.stats.health <= 0.0,
            Self::AbstractDaddy(e) => e.stats.health <= 0.0,
            Self::FleshLips(e) => e.stats.health <= 0.0,
            Self::Mandarin(e) => e.stats.health <= 0.0,
            Self::Eddie(e) => e.stats.health <= 0.0,
            Self::TwinPyramidHead(e) => e.stats.health <= 0.0,
            Self::MaryBoss(e) => e.stats.health <= 0.0,
            Self::RoomInsect(e) => e.stats.health <= 0.0,
        }
    }

    pub fn is_downed(&self) -> bool {
        match self {
            Self::LyingFigure(e) => e.stats.down_timer > 0.0,
            Self::Mannequin(e) => e.stats.down_timer > 0.0,
            Self::Nurse(e) => e.stats.down_timer > 0.0,
            Self::AbstractDaddy(e) => {
                e.state == crate::ai::creatures::abstract_daddy::PapState::Down
            }
            Self::Creeper(_)
            | Self::PyramidHead(_)
            | Self::FleshLips(_)
            | Self::Mandarin(_)
            | Self::Eddie(_)
            | Self::TwinPyramidHead(_)
            | Self::MaryBoss(_)
            | Self::RoomInsect(_) => false,
        }
    }

    /// Access the creature's skeletal animation controller if supported.
    pub fn anim(&self) -> Option<&crate::ai::creatures::anim::CreatureAnimationController> {
        match self {
            Self::LyingFigure(e) => Some(&e.anim),
            Self::Mannequin(e) => Some(&e.anim),
            Self::Nurse(e) => Some(&e.anim),
            _ => None,
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        self.update_with_context(
            target,
            lighting,
            difficulty,
            EnemyUpdateContext::default(),
            dt,
        );
    }

    /// Update with full cross-entity room context (Twin PH partner, Flesh Lips counter, Mandarin siblings, Insect leader, ammo/spray).
    pub fn update_with_context(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        ctx: EnemyUpdateContext,
        dt: f32,
    ) {
        match self {
            Self::LyingFigure(e) => e.update(target, lighting, difficulty, dt),
            Self::Mannequin(e) => e.update(target, lighting, difficulty, dt),
            Self::Nurse(e) => e.update(target, lighting, difficulty, dt),
            Self::Creeper(e) => e.update(target, dt),
            Self::PyramidHead(e) => e.update(target, lighting, difficulty, dt),
            Self::AbstractDaddy(e) => e.update(target, ctx.player_radius_mm, difficulty, dt),
            Self::FleshLips(e) => e.update(
                target,
                ctx.other_flesh_lips_alive,
                ctx.other_flesh_lips_attacking,
                difficulty,
                dt,
            ),
            Self::Mandarin(e) => e.update(
                target,
                ctx.mandarin_over_grating,
                ctx.mandarin_near_other_turn,
                difficulty,
                dt,
            ),
            Self::Eddie(e) => e.update(target, difficulty, dt),
            Self::TwinPyramidHead(e) => e.update(target, ctx.twin_partner, difficulty, dt),
            Self::MaryBoss(e) => e.update(target, ctx.player_ammo_empty, 0.0, difficulty, dt),
            Self::RoomInsect(e) => e.update(target, ctx.insect_leader, ctx.player_spraying, dt),
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
    /// True when the player has 0 firearm ammo (`enCheckPlayerBulletEmpty` for Mary Boss pity timer).
    pub player_ammo_empty: bool,
    /// True while the player is actively firing Hyper Spray (`enCheckPlayerSprayNow` for Moth Swarm escape).
    pub player_spraying: bool,
    /// Player collision radius in mm (`enGetPlayerSize()`, `200.0` mm for James).
    pub player_radius_mm: f32,
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
            player_ammo_empty: false,
            player_spraying: false,
            player_radius_mm: 200.0,
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
        let twin_count = self
            .enemies
            .iter()
            .filter(|e| matches!(e, ActiveEnemy::TwinPyramidHead(_)))
            .count();
        let insect_count = self
            .enemies
            .iter()
            .filter(|e| matches!(e, ActiveEnemy::RoomInsect(_)))
            .count();

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
            EnemySpecies::AbstractDaddy => ActiveEnemy::AbstractDaddy(
                AbstractDaddyController::new(id, position, yaw, false, self.difficulty),
            ),
            EnemySpecies::FleshLips => {
                ActiveEnemy::FleshLips(FleshLipsController::new(id, position, yaw, self.difficulty))
            }
            EnemySpecies::Mandarin => {
                ActiveEnemy::Mandarin(MandarinController::new(id, position, yaw, self.difficulty))
            }
            EnemySpecies::Eddie => ActiveEnemy::Eddie(EddieController::new(
                id,
                position,
                yaw,
                false,
                self.difficulty,
            )),
            EnemySpecies::TwinPyramidHead => ActiveEnemy::TwinPyramidHead(
                TwinPyramidHeadController::new(id, position, yaw, twin_count, self.difficulty),
            ),
            EnemySpecies::MaryBoss => ActiveEnemy::MaryBoss(MaryBossController::new(
                id,
                position,
                yaw,
                false,
                self.difficulty,
            )),
            EnemySpecies::RoomInsect => ActiveEnemy::RoomInsect(RoomInsectController::new(
                id,
                position,
                yaw,
                insect_count == 0,
            )),
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

        // Pre-capture cross-entity snapshots for paired bosses, multi-enemy Flesh Lips, Mandarins, and Moth Swarms.
        let twin_snapshots: Vec<(u32, TwinPartnerSnapshot)> = self
            .enemies
            .iter()
            .filter_map(|e| match e {
                ActiveEnemy::TwinPyramidHead(t) => Some((t.id, t.snapshot(player_pos))),
                _ => None,
            })
            .collect();
        let flesh_lips_info: Vec<(u32, bool, bool)> = self
            .enemies
            .iter()
            .filter_map(|e| match e {
                ActiveEnemy::FleshLips(f) => Some((
                    f.id,
                    f.stats.health > 0.0,
                    matches!(f.state, IkeState::Attack | IkeState::Attack2),
                )),
                _ => None,
            })
            .collect();
        let mandarin_positions: Vec<(u32, Vec3)> = self
            .enemies
            .iter()
            .filter_map(|e| match e {
                ActiveEnemy::Mandarin(m) if m.stats.health > 0.0 => Some((m.id, m.position)),
                _ => None,
            })
            .collect();
        let insect_leader: Option<(u32, InsectState, Vec3)> =
            self.enemies.iter().find_map(|e| match e {
                ActiveEnemy::RoomInsect(i)
                    if matches!(
                        i.state,
                        InsectState::Approach | InsectState::Attack | InsectState::Escape
                    ) =>
                {
                    Some((i.id, i.state, i.position))
                }
                _ => None,
            });

        for enemy in &mut self.enemies {
            let target = if self
                .maria
                .should_enemy_target_maria(enemy.position(), player_pos)
            {
                &maria_target
            } else {
                &player_target
            };

            let mut ctx = EnemyUpdateContext {
                player_radius_mm: self.player_radius_mm,
                player_ammo_empty: self.player_ammo_empty,
                player_spraying: self.player_spraying,
                ..EnemyUpdateContext::default()
            };

            match enemy {
                ActiveEnemy::TwinPyramidHead(t) => {
                    ctx.twin_partner = twin_snapshots
                        .iter()
                        .find(|(id, _)| *id != t.id)
                        .map(|(_, snap)| *snap);
                }
                ActiveEnemy::FleshLips(f) => {
                    ctx.other_flesh_lips_alive = flesh_lips_info
                        .iter()
                        .filter(|(id, alive, _)| *id != f.id && *alive)
                        .count();
                    ctx.other_flesh_lips_attacking = flesh_lips_info
                        .iter()
                        .any(|(id, _, attacking)| *id != f.id && *attacking);
                }
                ActiveEnemy::Mandarin(m) => {
                    ctx.mandarin_near_other_turn = mandarin_positions
                        .iter()
                        .filter(|(id, _)| *id != m.id)
                        .find_map(|(_, pos)| m.check_near_other(*pos));
                }
                ActiveEnemy::RoomInsect(i) => {
                    ctx.insect_leader = insect_leader
                        .filter(|(id, _, _)| *id != i.id)
                        .map(|(_, st, pos)| (st, pos));
                }
                _ => {}
            }

            enemy.update_with_context(target, self.lighting, self.difficulty, ctx, dt);
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
