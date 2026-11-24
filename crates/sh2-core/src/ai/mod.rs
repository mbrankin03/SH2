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
pub mod enemy_actor;
pub mod maria;
pub mod npcs;
pub mod pathfinding;
pub mod radio;
pub mod sensory;

pub use enemy_actor::EnemyActor;

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
use crate::ai::creatures::{
    en_check_sleep_in, en_check_sleep_out, EnemySpecies, StagePlace, TribeCommunicationBus,
};
use crate::ai::maria::MariaCompanionController;
use crate::ai::radio::{
    en_kind_from_species, RadioEnemySample, RadioNoiseEvaluator, RadioStaticOutput,
};
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

/// Boss completion signals emitted by active boss controllers for consumption by stage events
/// and inventory (`sh2-core::events`, `sh2-core::campaign`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiBossEvents {
    /// Pyramid Head 1st encounter retreated down flooded stairwell (`enREDCtrlBattleEnd` `0x0015eeb0`).
    pub ph_battle_end_complete: bool,
    /// Last Flesh Lips defeated (`enIKECtrlDead` `0x001522b0`).
    pub flesh_lips_battle_cleared: bool,
    /// Abstract Daddy boss defeated (`enPAPCtrlDown` `0x0015baf0`).
    pub abstract_daddy_defeated: bool,
    /// Eddie Room 1 escaped (`enEDBCtrlEscape2` `0x0014c2b0`).
    pub eddie_room1_escaped: bool,
    /// Eddie Room 2 defeated (`enEDBCtrlDead` `0x0014c750`).
    pub eddie_room2_dead: bool,
    /// Twin Pyramid Heads reached suicide pose (`enONICtrlSuicide` `0x0022ad20`).
    pub twin_ph_suicide_ready: bool,
    /// Mary Boss Phase 1 cleared (`enBOSCtrlDead` `0x00140820`).
    pub mary_phase1_cleared: bool,
    /// Mary Boss Phase 2 defeated / ending triggered (`enBOSCtrlDead` `0x00140820`).
    pub mary_ending_triggered: bool,
}

/// Global AI and Companion subsystem (`en_common.c` / `m3_maria_sub.c`).
#[derive(Debug, Clone)]
pub struct AiSubsystem {
    /// Active enemies currently spawned in the stage/room (max 24-32 slots).
    pub enemies: Vec<EnemyActor>,
    /// Maria companion controller.
    pub maria: MariaCompanionController,
    /// Radio static evaluator.
    pub radio_evaluator: RadioNoiseEvaluator,
    /// Latest radio output state.
    pub radio_output: RadioStaticOutput,
    /// Current room lighting condition (`enGetWorldCondition` `0x00141ca0`).
    pub lighting: RoomLighting,
    /// Game difficulty setting (`enGetMode()` `0..=3`).
    pub difficulty: DifficultyLevel,
    /// Stage progression tier for `enSetHP` (`0x001420c0`) multipliers.
    pub stage_place: StagePlace,
    /// True when the active stage is outdoor (`BgIsOut()`, controls sleep culling & acid fog reach).
    pub is_outdoor: bool,
    /// 8-slot tribe alert communication bus (`enSetCommunication` / `enCommunicateTribe`).
    pub tribe_bus: TribeCommunicationBus,
    /// True when the player has 0 firearm ammo (`enCheckPlayerBulletEmpty` for Mary Boss pity timer).
    pub player_ammo_empty: bool,
    /// True while the player is actively firing Hyper Spray (`enCheckPlayerSprayNow` for Moth Swarm escape).
    pub player_spraying: bool,
    /// Player collision radius in mm (`enGetPlayerSize()`, `200.0` mm for James).
    pub player_radius_mm: f32,
    /// Deterministic LCG state (`shRandI` equivalent for `TwinPyramidHead` spin-loop rolls).
    rng_state: u32,
    /// Accumulated phase for `MaryBoss` hover sway (`shSway1f(-250.0, 0.1)` `enBOSMoveExec` `0x00140df0`).
    sway_phase: f32,
    /// Next synthetic ID assigned to dynamically spawned Mary moth swarm insects.
    next_dynamic_id: u32,
    /// Reusable scratch buffers to keep `update_filtered` 100% zero-allocation per tick.
    scratch_hostile_positions: Vec<Vec3>,
    scratch_radio_samples: Vec<RadioEnemySample>,
    scratch_twin_snapshots: Vec<(u32, TwinPartnerSnapshot)>,
    scratch_flesh_lips_info: Vec<(u32, bool, bool)>,
    scratch_mandarin_positions: Vec<(u32, Vec3)>,
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
            stage_place: StagePlace::Default,
            is_outdoor: false,
            tribe_bus: TribeCommunicationBus::default(),
            player_ammo_empty: false,
            player_spraying: false,
            player_radius_mm: 200.0,
            rng_state: 0x1999_0901,
            sway_phase: 0.0,
            next_dynamic_id: 10_000,
            scratch_hostile_positions: Vec::with_capacity(32),
            scratch_radio_samples: Vec::with_capacity(32),
            scratch_twin_snapshots: Vec::with_capacity(4),
            scratch_flesh_lips_info: Vec::with_capacity(8),
            scratch_mandarin_positions: Vec::with_capacity(8),
        }
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

    /// Apply a player weapon hit to `enemies[enemy_idx]` and transfer any `TwinPyramidHead`
    /// HP overflow (`(-next_hp) * 0.75`) to its partner (`enONISetDamage` `0x0022b370` / `sh2pc 0x004bc8b0`).
    pub fn apply_hit_to_enemy(
        &mut self,
        enemy_idx: usize,
        atk: &crate::combat::attack_list::AttackSpec,
        attack_dir_xz: [f32; 2],
    ) {
        let Some(enemy) = self.enemies.get_mut(enemy_idx) else {
            return;
        };
        if let Some((hit_id, overflow)) = enemy.receive_hit_with_overflow(atk, attack_dir_xz) {
            for other in &mut self.enemies {
                if let ActiveEnemy::TwinPyramidHead(partner) = &mut other.controller {
                    if partner.id != hit_id {
                        partner.stats.health = (partner.stats.health - overflow).max(0.0);
                        break;
                    }
                }
            }
        }
    }

    /// Apply a Hyper Spray aerosol hit to `enemies[enemy_idx]`.
    pub fn apply_hyper_spray_hit_to_enemy(
        &mut self,
        enemy_idx: usize,
        atk: &crate::combat::attack_list::AttackSpec,
        color: crate::combat::attack_list::HyperSprayColor,
    ) -> bool {
        let Some(enemy) = self.enemies.get_mut(enemy_idx) else {
            return false;
        };
        enemy.receive_hyper_spray_hit(atk, color)
    }

    /// Poll and clear one-shot boss completion events emitted by active boss controllers.
    pub fn consume_boss_events(&mut self) -> AiBossEvents {
        let mut events = AiBossEvents::default();
        for enemy in &mut self.enemies {
            match &mut enemy.controller {
                ActiveEnemy::PyramidHead(e) if e.battle_end_complete => {
                    events.ph_battle_end_complete = true;
                }
                ActiveEnemy::FleshLips(e) if e.battle_cleared_event => {
                    events.flesh_lips_battle_cleared = true;
                    e.battle_cleared_event = false;
                }
                ActiveEnemy::AbstractDaddy(e) if e.boss_defeated_event => {
                    events.abstract_daddy_defeated = true;
                    e.boss_defeated_event = false;
                }
                ActiveEnemy::Eddie(e) => {
                    if e.room1_escaped_event {
                        events.eddie_room1_escaped = true;
                        e.room1_escaped_event = false;
                    }
                    if e.room2_dead_event {
                        events.eddie_room2_dead = true;
                        e.room2_dead_event = false;
                    }
                }
                ActiveEnemy::TwinPyramidHead(e) if e.suicide_cutscene_ready => {
                    events.twin_ph_suicide_ready = true;
                    e.suicide_cutscene_ready = false;
                }
                ActiveEnemy::MaryBoss(e) => {
                    if e.phase1_cleared_event {
                        events.mary_phase1_cleared = true;
                        e.phase1_cleared_event = false;
                    }
                    if e.ending_triggered_event {
                        events.mary_ending_triggered = true;
                        e.ending_triggered_event = false;
                    }
                }
                _ => {}
            }
        }
        events
    }

    /// Primary 60 Hz AI simulation update loop (`enExecTask` `SLUS_202.28` `0x001413f0`).
    pub fn update(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        dt: f32,
    ) {
        self.update_filtered(
            player_pos,
            player_yaw,
            is_flashlight_on,
            is_running,
            radio_enabled,
            None,
            dt,
        );
    }

    /// Update AI simulation, optionally isolating execution to a single enemy index
    /// (used by the Combat Arena single-target test mode).
    #[allow(clippy::too_many_arguments)]
    pub fn update_filtered(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        only_enemy_idx: Option<usize>,
        dt: f32,
    ) {
        self.update_filtered_with_collision(
            player_pos,
            player_yaw,
            is_flashlight_on,
            is_running,
            radio_enabled,
            only_enemy_idx,
            None,
            dt,
        );
    }

    /// Update AI simulation with optional `.cld` [`crate::collision::CollisionWorld`] floor probing
    /// (for Mandarin `enCheckFloor` grating checks) and cross-entity loopbacks.
    #[allow(clippy::too_many_arguments)]
    pub fn update_filtered_with_collision(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        only_enemy_idx: Option<usize>,
        collision: Option<&crate::collision::CollisionWorld>,
        dt: f32,
    ) {
        // Collect active enemy positions for radio and companion awareness (zero-alloc scratch)
        self.scratch_hostile_positions.clear();
        for e in &self.enemies {
            if !e.is_dead() {
                self.scratch_hostile_positions.push(e.position());
            }
        }

        // 1. Update Maria companion
        self.maria
            .update(player_pos, player_yaw, &self.scratch_hostile_positions, dt);

        // 2. Target arbitration and enemy updates (`0x542ad0`)
        let player_target =
            PerceivableTarget::new(player_pos, player_yaw, is_flashlight_on, is_running);

        let maria_target =
            PerceivableTarget::new(self.maria.position, self.maria.yaw, false, false);

        // Pre-capture cross-entity snapshots into reusable scratch buffers (zero heap allocation).
        self.scratch_twin_snapshots.clear();
        self.scratch_flesh_lips_info.clear();
        self.scratch_mandarin_positions.clear();
        let mut insect_leader: Option<(u32, InsectState, Vec3)> = None;
        let mut has_alive_insects = false;

        for e in &self.enemies {
            match &e.controller {
                ActiveEnemy::TwinPyramidHead(t) => {
                    self.scratch_twin_snapshots
                        .push((t.id, t.snapshot(player_pos)));
                }
                ActiveEnemy::FleshLips(f) => {
                    self.scratch_flesh_lips_info.push((
                        f.id,
                        f.stats.health > 0.0,
                        matches!(f.state, IkeState::Attack | IkeState::Attack2),
                    ));
                }
                ActiveEnemy::Mandarin(m) if m.stats.health > 0.0 => {
                    self.scratch_mandarin_positions.push((m.id, m.position));
                }
                ActiveEnemy::RoomInsect(i) => {
                    if i.stats.health > 0.0 {
                        has_alive_insects = true;
                    }
                    if insect_leader.is_none()
                        && matches!(
                            i.state,
                            InsectState::Approach | InsectState::Attack | InsectState::Escape
                        )
                    {
                        insect_leader = Some((i.id, i.state, i.position));
                    }
                }
                _ => {}
            }
        }

        // Advance Mary hover sway (`shSway1f(-250.0, 0.1)` `enBOSMoveExec` `0x00140df0`).
        self.sway_phase += dt * 60.0 * 0.1;
        let mary_sway_y_sample = self.sway_phase.sin() * 250.0;
        let has_collision_floors = collision.is_some_and(|c| c.all_floors().next().is_some());

        let mut mary_swarm_spawn: Option<(Vec3, f32)> = None;
        let mut insects_despawn_all = false;

        let twin_snapshots = &self.scratch_twin_snapshots;
        let flesh_lips_info = &self.scratch_flesh_lips_info;
        let mandarin_positions = &self.scratch_mandarin_positions;

        for idx in 0..self.enemies.len() {
            if only_enemy_idx.is_some_and(|only| only != idx) {
                continue;
            }

            self.rng_state = self
                .rng_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let rng_bits = self.rng_state;
            let enemy = &mut self.enemies[idx];
            enemy.record_prev_pose();

            // Outdoor sleep culling (`enCheckSleepIn` 15000mm / `enCheckSleepOut` 7500mm)
            let dist_to_player = (enemy.position() - player_pos).length();
            if enemy.is_sleeping {
                if en_check_sleep_out(dist_to_player, self.is_outdoor, true) {
                    enemy.is_sleeping = false;
                } else {
                    continue;
                }
            } else if en_check_sleep_in(dist_to_player, self.is_outdoor, enemy.is_dead(), true) {
                enemy.is_sleeping = true;
                continue;
            }

            // Drive loopback fields from EnemyActor animation & grapple state (`*(i32*)(work+0x14) == -1`, `enCheckHuggedPlayer`).
            let clip_finished = enemy.anim().is_clip_finished();
            let is_grappling = enemy.is_grappling_player;
            match &mut enemy.controller {
                ActiveEnemy::Mannequin(e) => {
                    e.anim_finished |= clip_finished;
                }
                ActiveEnemy::PyramidHead(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                ActiveEnemy::AbstractDaddy(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                ActiveEnemy::FleshLips(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                ActiveEnemy::Mandarin(e) => {
                    e.anim_finished |= clip_finished;
                }
                ActiveEnemy::Eddie(e) => {
                    e.anim_finished |= clip_finished;
                }
                ActiveEnemy::TwinPyramidHead(e) => {
                    e.anim_finished |= clip_finished;
                    e.spin_loop_rand_bits = rng_bits;
                }
                ActiveEnemy::MaryBoss(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                _ => {}
            }

            // While grappling James, `GrappleSession` owns enemy anchoring & pose.
            if enemy.is_grappling_player {
                enemy.tick_fallback_anim(dt);
                continue;
            }

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
                mary_sway_y_sample,
                ..EnemyUpdateContext::default()
            };

            match &enemy.controller {
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
                    if has_collision_floors {
                        if let Some(world) = collision {
                            let probe = m.forward_grating_probe_pos();
                            let hit = world
                                .sample_floor_surface(probe.x, probe.y, probe.z, 1000.0, 2000.0);
                            // `enCheckFloor` (`0x00145a80`): returns `2` for material `9` or `0xb`, `0` for none, `1` for solid.
                            ctx.mandarin_over_grating =
                                hit.is_none_or(|h| matches!(h.raw_material & 0x0F, 9 | 11));
                        }
                    }
                }
                ActiveEnemy::RoomInsect(i) => {
                    ctx.insect_leader = insect_leader
                        .filter(|(id, _, _)| *id != i.id)
                        .map(|(_, st, pos)| (st, pos));
                }
                _ => {}
            }

            enemy
                .controller
                .update_with_context(target, self.lighting, self.difficulty, ctx, dt);
            enemy.step_hitback();
            enemy.tick_fallback_anim(dt);

            match &enemy.controller {
                ActiveEnemy::MaryBoss(m)
                    if m.moth_swarm_active && !has_alive_insects && mary_swarm_spawn.is_none() =>
                {
                    let to_player = player_pos - m.position;
                    let yaw = to_player.x.atan2(to_player.z);
                    mary_swarm_spawn = Some((m.position, yaw));
                }
                ActiveEnemy::RoomInsect(i) if i.despawn_all => {
                    insects_despawn_all = true;
                }
                _ => {}
            }
        }

        if insects_despawn_all {
            self.enemies
                .retain(|e| !matches!(e.controller, ActiveEnemy::RoomInsect(_)));
            for e in &mut self.enemies {
                if let ActiveEnemy::MaryBoss(m) = &mut e.controller {
                    m.moth_swarm_active = false;
                }
            }
        } else if let Some((spawn_pos, spawn_yaw)) = mary_swarm_spawn {
            // `enBOSCtrlGenerate` (`0x00140090`): spawn RoomInsect swarm (leader + followers up to 32 slots).
            let available_slots = 32usize.saturating_sub(self.enemies.len());
            let spawn_count = available_slots.min(16);
            for _ in 0..spawn_count {
                let id = self.next_dynamic_id;
                self.next_dynamic_id = self.next_dynamic_id.wrapping_add(1);
                self.spawn_enemy(EnemySpecies::RoomInsect, id, spawn_pos, spawn_yaw);
            }
        }

        // Decay tribe alert bus TTLs (`enExecTask` `0x001413f0` lines 272–282)
        self.tribe_bus.tick();

        // 3. Update Pocket Radio proximity static (`RadioNoise` `0x001ab300` & `enSetRadioVolume` `0x00143890`)
        self.scratch_radio_samples.clear();
        for e in &self.enemies {
            self.scratch_radio_samples.push(RadioEnemySample {
                kind: en_kind_from_species(e.species()),
                sub_type: 0,
                position: e.position(),
                is_spawned_active: !e.is_sleeping,
                is_alive: !e.is_dead(),
                is_downed: e.controller.is_downed(),
            });
        }
        self.radio_output = self.radio_evaluator.evaluate_enemies(
            radio_enabled,
            player_pos,
            player_pos,
            glam::Mat4::IDENTITY,
            &self.scratch_radio_samples,
            dt,
        );
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

        // Tick AI enough frames for the 2.5s `RadioNoise` (`0x001ab300`) volume ramp to settle
        for _ in 0..200 {
            ai.update(player_pos, 0.0, true, false, true, 0.016);
        }

        // Radio static should be active and audible at 1000 units (`cos(1000/12500 * pi/2) ≈ 0.992`)
        assert!(ai.radio_output.volume > 0.8);
        assert!(ai.radio_output.nearest_enemy_dist < 12_500.0);
    }
}
