//! Retail `Enemy_List` spawn mapping and [`AiSubsystem`] spawn helpers.
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c` (`enInitData`), `connectRoomInit` (`0x0020be10`)
//! - PC executable `sh2pc.exe`: `0x4aa5e0` (`enInitData`)

use crate::ai::active_enemy::ActiveEnemy;
use crate::ai::creatures::abstract_daddy::AbstractDaddyController;
use crate::ai::creatures::creeper::CreeperController;
use crate::ai::creatures::eddie::EddieController;
use crate::ai::creatures::flesh_lips::FleshLipsController;
use crate::ai::creatures::insects::RoomInsectController;
use crate::ai::creatures::lying_figure::LyingFigureController;
use crate::ai::creatures::mandarin::MandarinController;
use crate::ai::creatures::mannequin::MannequinController;
use crate::ai::creatures::mary::MaryBossController;
use crate::ai::creatures::nurse::NurseController;
use crate::ai::creatures::pyramid_head::PyramidHeadController;
use crate::ai::creatures::twin_pyramid_head::TwinPyramidHeadController;
use crate::ai::creatures::EnemySpecies;
use crate::ai::enemy_actor::EnemyActor;
use crate::ai::sensory::DifficultyLevel;
use crate::ai::AiSubsystem;
use glam::Vec3;

/// Spawn variant flags derived from retail `Enemy_List` `kind` & `status`/`sub_type`
/// (`enNSEInitData` `0x001577b0`, `enREDInitData` `0x0015d410`, `enPAPInitData` `0x0015a060`,
/// `enEDBInitData` `0x001494c0`, `enBOSInitData` `0x0013fa10`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnemySpawnVariant {
    /// `0x20b` Town Tough Nurse (`enNSEInitData` `0x001577b0`: `1.2x` HP/endurance).
    pub is_dark_nurse: bool,
    /// Retail `sub_type` (`*(i32*)(chara + 0x100)`):
    /// - `6` for Pyramid Head (Blue Creek stairwell boss), Abstract Daddy (Hotel boss), Eddie (Room 2)
    /// - `0xd` (`13`) for Mary Boss (Phase 2 floor)
    pub sub_type: i16,
}

impl EnemySpawnVariant {
    /// Map retail `Enemy_List` `(kind, status)` to `(EnemySpecies, EnemySpawnVariant)`.
    #[must_use]
    pub const fn from_kind_and_status(kind: i16, status: i16) -> Option<(EnemySpecies, Self)> {
        let (species, is_dark_nurse) = match kind {
            0x200 => (EnemySpecies::LyingFigure, false),
            0x201 => (EnemySpecies::Mannequin, false),
            0x202 => (EnemySpecies::Creeper, false),
            0x203 => (EnemySpecies::FleshLips, false),
            0x204 => (EnemySpecies::AbstractDaddy, false),
            0x205 => (EnemySpecies::Eddie, false),
            0x206 => (EnemySpecies::MaryBoss, false),
            0x207 => (EnemySpecies::Nurse, false),
            0x208 => (EnemySpecies::PyramidHead, false),
            0x209 => (EnemySpecies::TwinPyramidHead, false),
            0x20a => (EnemySpecies::Mandarin, false),
            0x20b => (EnemySpecies::Nurse, true),
            0x20f => (EnemySpecies::RoomInsect, false),
            _ => return None,
        };
        Some((
            species,
            Self {
                is_dark_nurse,
                sub_type: status,
            },
        ))
    }
}

impl AiSubsystem {
    /// Clear all enemies and reset when entering a new room (`enInitData`).
    pub fn clear_enemies(&mut self) {
        self.enemies.clear();
        self.tribe_bus.clear();
    }

    /// Spawn an enemy into the active world with authentic `enSetHP` (`0x001420c0`) stage scaling.
    pub fn spawn_enemy(&mut self, species: EnemySpecies, id: u32, position: Vec3, yaw: f32) {
        let default_variant = match species {
            EnemySpecies::PyramidHead => EnemySpawnVariant {
                sub_type: 6,
                ..Default::default()
            },
            _ => EnemySpawnVariant::default(),
        };
        self.spawn_enemy_with_variant(species, id, position, yaw, default_variant);
    }

    /// Spawn an enemy with explicit retail `Enemy_List` variant flags (`0x20b` dark nurse,
    /// `sub_type == 6` boss/room2, `sub_type == 0xd` Mary Phase 2).
    pub fn spawn_enemy_with_variant(
        &mut self,
        species: EnemySpecies,
        id: u32,
        position: Vec3,
        yaw: f32,
        variant: EnemySpawnVariant,
    ) {
        let twin_count = self
            .enemies
            .iter()
            .filter(|e| matches!(e.controller, ActiveEnemy::TwinPyramidHead(_)))
            .count();
        let insect_count = self
            .enemies
            .iter()
            .filter(|e| matches!(e.controller, ActiveEnemy::RoomInsect(_)))
            .count();

        let enemy = match species {
            EnemySpecies::LyingFigure => {
                ActiveEnemy::LyingFigure(LyingFigureController::new_for_stage(
                    id,
                    position,
                    yaw,
                    self.difficulty,
                    self.stage_place,
                ))
            }
            EnemySpecies::Mannequin => {
                ActiveEnemy::Mannequin(MannequinController::new_with_subtype(
                    id,
                    position,
                    yaw,
                    self.difficulty,
                    variant.sub_type.max(0) as u32,
                    self.stage_place,
                ))
            }
            EnemySpecies::Nurse => ActiveEnemy::Nurse(NurseController::new_for_stage(
                id,
                position,
                yaw,
                self.difficulty,
                variant.is_dark_nurse,
                self.stage_place,
            )),
            EnemySpecies::Creeper => ActiveEnemy::Creeper(CreeperController::new_with_difficulty(
                id,
                position,
                yaw,
                self.difficulty,
            )),
            EnemySpecies::PyramidHead => {
                ActiveEnemy::PyramidHead(PyramidHeadController::new_with_difficulty(
                    id,
                    position,
                    yaw,
                    self.difficulty,
                    variant.sub_type == 6,
                ))
            }
            EnemySpecies::AbstractDaddy => {
                ActiveEnemy::AbstractDaddy(AbstractDaddyController::new(
                    id,
                    position,
                    yaw,
                    variant.sub_type == 6,
                    self.difficulty,
                ))
            }
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
                variant.sub_type == 6,
                self.difficulty,
            )),
            EnemySpecies::TwinPyramidHead => ActiveEnemy::TwinPyramidHead(
                TwinPyramidHeadController::new(id, position, yaw, twin_count, self.difficulty),
            ),
            EnemySpecies::MaryBoss => ActiveEnemy::MaryBoss(MaryBossController::new(
                id,
                position,
                yaw,
                variant.sub_type == 0xd,
                self.difficulty,
            )),
            EnemySpecies::RoomInsect => ActiveEnemy::RoomInsect(RoomInsectController::new(
                id,
                position,
                yaw,
                insect_count == 0,
            )),
        };
        self.enemies.push(EnemyActor::new(enemy));
    }

    /// Populate enemies from a [`crate::campaign::RoomLoadManifest`] `enemy_spawns` list
    /// (`connectRoomInit` `0x0020be10` / `enInitData`).
    pub fn spawn_from_manifest(
        &mut self,
        spawns: &[crate::campaign::CampaignEnemySpawn],
        difficulty: DifficultyLevel,
    ) {
        self.clear_enemies();
        self.difficulty = difficulty;
        for spawn in spawns {
            if let Some((species, variant)) =
                EnemySpawnVariant::from_kind_and_status(spawn.kind, spawn.status)
            {
                let yaw = (f32::from(spawn.rot_y_raw) / 4096.0) * std::f32::consts::TAU;
                let pos = Vec3::from_array(spawn.position_mm);
                let id = u32::try_from(spawn.id.max(0)).unwrap_or(0);
                self.spawn_enemy_with_variant(species, id, pos, yaw, variant);
            }
        }
    }
}
