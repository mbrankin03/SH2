//! Active enemy controller wrapper and shared cross-entity update context.
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c` (`enExecTask` `0x001413f0`)

use crate::ai::creatures::abstract_daddy::AbstractDaddyController;
use crate::ai::creatures::creeper::CreeperController;
use crate::ai::creatures::eddie::EddieController;
use crate::ai::creatures::flesh_lips::FleshLipsController;
use crate::ai::creatures::insects::{InsectState, RoomInsectController};
use crate::ai::creatures::lying_figure::LyingFigureController;
use crate::ai::creatures::mandarin::MandarinController;
use crate::ai::creatures::mannequin::MannequinController;
use crate::ai::creatures::mary::MaryBossController;
use crate::ai::creatures::nurse::NurseController;
use crate::ai::creatures::pyramid_head::PyramidHeadController;
use crate::ai::creatures::twin_pyramid_head::{TwinPartnerSnapshot, TwinPyramidHeadController};
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
    /// Deterministic hover sway sample in `[-250.0, 250.0]` (`shSway1f(-250.0, 0.1)` `enBOSMoveExec` `0x00140df0`).
    pub mary_sway_y_sample: f32,
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
            mary_sway_y_sample: 0.0,
        }
    }
}

/// Polymorphic wrapper around the active creature/boss controller inside [`super::EnemyActor`].
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
    /// Return the entity's slot/spawn ID.
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

    /// Return the entity's current world position in PS2 mm.
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

    /// Return the entity's current yaw in radians.
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

    /// Return true when the entity is dead (or Pyramid Head 1st encounter retreated).
    pub fn is_dead(&self) -> bool {
        match self {
            Self::LyingFigure(e) => e.stats.health <= 0.0,
            Self::Mannequin(e) => e.stats.health <= 0.0,
            Self::Nurse(e) => e.stats.health <= 0.0,
            Self::Creeper(e) => e.stats.health <= 0.0,
            Self::PyramidHead(e) => e.battle_end_complete,
            Self::AbstractDaddy(e) => e.stats.health <= 0.0,
            Self::FleshLips(e) => e.stats.health <= 0.0,
            Self::Mandarin(e) => e.stats.health <= 0.0,
            Self::Eddie(e) => e.stats.health <= 0.0,
            Self::TwinPyramidHead(e) => e.stats.health <= 0.0,
            Self::MaryBoss(e) => e.stats.health <= 0.0,
            Self::RoomInsect(e) => e.stats.health <= 0.0,
        }
    }

    /// Return true when the entity is currently knocked down.
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
            Self::Creeper(e) => Some(&e.anim),
            _ => None,
        }
    }

    /// Step the controller for one frame using [`EnemyUpdateContext::default`].
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
            Self::MaryBoss(e) => e.update(
                target,
                ctx.player_ammo_empty,
                ctx.mary_sway_y_sample,
                difficulty,
                dt,
            ),
            Self::RoomInsect(e) => e.update(target, ctx.insect_leader, ctx.player_spraying, dt),
        }
    }
}
