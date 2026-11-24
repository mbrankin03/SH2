//! Unified runtime enemy actor wrapping [`ActiveEnemy`] for both AI simulation and combat
//! hit/grapple/animation resolution (eliminates the legacy `CombatTargetEnemy` split-brain).
//!
//! # Ground Truth & Binary References
//! - `en_common.c`: `enCheckDamage` (`0x001423a0`), `enReduceHP` (`0x00142200`),
//!   `enSetHitBack` (`0x00142630`), `enSetSize` (`0x001426e0`), `enMoveExec` (`0x00143aa0`).

use crate::ai::creatures::abstract_daddy::PapState;
use crate::ai::creatures::anim::{CreatureAnimStateKind, CreatureAnimationController};
use crate::ai::creatures::creeper::TyuState;
use crate::ai::creatures::eddie::EdbState;
use crate::ai::creatures::flesh_lips::IkeState;
use crate::ai::creatures::insects::InsectState;
use crate::ai::creatures::lying_figure::ScuState;
use crate::ai::creatures::mandarin::ArmState;
use crate::ai::creatures::mannequin::MknState;
use crate::ai::creatures::mary::BosState;
use crate::ai::creatures::nurse::NseState;
use crate::ai::creatures::pyramid_head::RedState;
use crate::ai::creatures::twin_pyramid_head::OniState;
use crate::ai::creatures::{CreatureStats, EnemySize, EnemySpecies};
use crate::ai::ActiveEnemy;
use crate::combat::attack_list::{AttackSpec, HyperSprayColor};
use crate::combat::enemy_attack::EnemyAttackWork;
use crate::combat::hitboxes::CombatHitbox;
use glam::Vec3;

/// Single unified enemy slot owned by [`crate::ai::AiSubsystem`].
///
/// Combines the authentic per-species AI controller (`ActiveEnemy`) with sub-frame interpolation
/// pose history, grapple binding state, sleep culling state, attack-window state, and skeletal
/// animation playback.
#[derive(Debug, Clone)]
pub struct EnemyActor {
    /// Authentic per-species AI controller state machine.
    pub controller: ActiveEnemy,
    /// Previous tick world position in mm (for sub-frame render interpolation).
    pub prev_position: Vec3,
    /// Previous tick yaw in radians (for sub-frame render interpolation).
    pub prev_yaw: f32,
    /// True while this enemy is actively holding James in a [`crate::combat::GrappleSession`].
    pub is_grappling_player: bool,
    /// True while culled by `enCheckSleepIn` (`15000.0` mm outdoor, `0x00142950`).
    pub is_sleeping: bool,
    /// Per-attack hit-check state (`shBattleAttackHitCheckInit` `0x00123130`).
    pub attack_work: EnemyAttackWork,
    /// Animation controller for species that do not store one directly on their inner controller.
    fallback_anim: CreatureAnimationController,
}

impl EnemyActor {
    /// Wrap an [`ActiveEnemy`] in a unified [`EnemyActor`].
    pub fn new(controller: ActiveEnemy) -> Self {
        let pos = controller.position();
        let yaw = controller.yaw();
        let fallback_anim = match &controller {
            ActiveEnemy::LyingFigure(_) => CreatureAnimationController::new_lying_figure(),
            ActiveEnemy::Mannequin(_) => CreatureAnimationController::new_mannequin(),
            ActiveEnemy::Nurse(_) => CreatureAnimationController::new_nurse(),
            ActiveEnemy::Creeper(_) => CreatureAnimationController::new_creeper(),
            ActiveEnemy::PyramidHead(_) | ActiveEnemy::TwinPyramidHead(_) => {
                CreatureAnimationController::new_pyramid_head()
            }
            ActiveEnemy::AbstractDaddy(_) => CreatureAnimationController::new_abstract_daddy(),
            ActiveEnemy::FleshLips(_) => CreatureAnimationController::new_flesh_lips(),
            ActiveEnemy::Mandarin(_) => CreatureAnimationController::new_mandarin(),
            ActiveEnemy::MaryBoss(_) => CreatureAnimationController::new_mary_boss(),
            ActiveEnemy::Eddie(_) | ActiveEnemy::RoomInsect(_) => {
                CreatureAnimationController::new_lying_figure()
            }
        };
        Self {
            controller,
            prev_position: pos,
            prev_yaw: yaw,
            is_grappling_player: false,
            is_sleeping: false,
            attack_work: EnemyAttackWork::default(),
            fallback_anim,
        }
    }

    /// Unique enemy instance / bestiary ID.
    #[inline]
    pub fn id(&self) -> u32 {
        self.controller.id()
    }

    /// Species enum (`0x200..=0x20f`).
    #[inline]
    pub fn species(&self) -> EnemySpecies {
        match &self.controller {
            ActiveEnemy::LyingFigure(_) => EnemySpecies::LyingFigure,
            ActiveEnemy::Mannequin(_) => EnemySpecies::Mannequin,
            ActiveEnemy::Nurse(_) => EnemySpecies::Nurse,
            ActiveEnemy::Creeper(_) => EnemySpecies::Creeper,
            ActiveEnemy::PyramidHead(_) => EnemySpecies::PyramidHead,
            ActiveEnemy::AbstractDaddy(_) => EnemySpecies::AbstractDaddy,
            ActiveEnemy::FleshLips(_) => EnemySpecies::FleshLips,
            ActiveEnemy::Mandarin(_) => EnemySpecies::Mandarin,
            ActiveEnemy::Eddie(_) => EnemySpecies::Eddie,
            ActiveEnemy::TwinPyramidHead(_) => EnemySpecies::TwinPyramidHead,
            ActiveEnemy::MaryBoss(_) => EnemySpecies::MaryBoss,
            ActiveEnemy::RoomInsect(_) => EnemySpecies::RoomInsect,
        }
    }

    /// Human-readable species label for HUD/debug overlays.
    pub fn name(&self) -> &'static str {
        match &self.controller {
            ActiveEnemy::LyingFigure(_) => "Lying Figure (scu)",
            ActiveEnemy::Mannequin(_) => "Mannequin (mkn)",
            ActiveEnemy::Nurse(_) => "Bubble Head Nurse (nse)",
            ActiveEnemy::Creeper(_) => "Creeper (tyu)",
            ActiveEnemy::PyramidHead(_) => "Pyramid Head (red)",
            ActiveEnemy::AbstractDaddy(_) => "Abstract Daddy (pap)",
            ActiveEnemy::FleshLips(_) => "Flesh Lips (ike)",
            ActiveEnemy::Mandarin(_) => "Mandarin (arm)",
            ActiveEnemy::Eddie(_) => "Eddie Boss (edb)",
            ActiveEnemy::TwinPyramidHead(_) => "Twin Pyramid Head (oni)",
            ActiveEnemy::MaryBoss(_) => "Mary Final Boss (bos)",
            ActiveEnemy::RoomInsect(_) => "Room Insect (ins)",
        }
    }

    /// Current world position in PS2 mm.
    #[inline]
    pub fn position(&self) -> Vec3 {
        self.controller.position()
    }

    /// Overwrite current world position in PS2 mm.
    pub fn set_position(&mut self, pos: Vec3) {
        match &mut self.controller {
            ActiveEnemy::LyingFigure(e) => e.position = pos,
            ActiveEnemy::Mannequin(e) => e.position = pos,
            ActiveEnemy::Nurse(e) => e.position = pos,
            ActiveEnemy::Creeper(e) => e.position = pos,
            ActiveEnemy::PyramidHead(e) => e.position = pos,
            ActiveEnemy::AbstractDaddy(e) => e.position = pos,
            ActiveEnemy::FleshLips(e) => e.position = pos,
            ActiveEnemy::Mandarin(e) => e.position = pos,
            ActiveEnemy::Eddie(e) => e.position = pos,
            ActiveEnemy::TwinPyramidHead(e) => e.position = pos,
            ActiveEnemy::MaryBoss(e) => e.position = pos,
            ActiveEnemy::RoomInsect(e) => e.position = pos,
        }
    }

    /// Current facing yaw in radians.
    #[inline]
    pub fn yaw(&self) -> f32 {
        self.controller.yaw()
    }

    /// Overwrite current facing yaw in radians.
    pub fn set_yaw(&mut self, yaw: f32) {
        match &mut self.controller {
            ActiveEnemy::LyingFigure(e) => e.yaw = yaw,
            ActiveEnemy::Mannequin(e) => e.yaw = yaw,
            ActiveEnemy::Nurse(e) => e.yaw = yaw,
            ActiveEnemy::Creeper(e) => e.yaw = yaw,
            ActiveEnemy::PyramidHead(e) => e.yaw = yaw,
            ActiveEnemy::AbstractDaddy(e) => e.yaw = yaw,
            ActiveEnemy::FleshLips(e) => e.yaw = yaw,
            ActiveEnemy::Mandarin(e) => e.yaw = yaw,
            ActiveEnemy::Eddie(e) => e.yaw = yaw,
            ActiveEnemy::TwinPyramidHead(e) => e.yaw = yaw,
            ActiveEnemy::MaryBoss(e) => e.yaw = yaw,
            ActiveEnemy::RoomInsect(e) => e.yaw = yaw,
        }
    }

    /// Capture current `(position, yaw)` as the previous-tick interpolation anchor.
    #[inline]
    pub fn record_prev_pose(&mut self) {
        self.prev_position = self.position();
        self.prev_yaw = self.yaw();
    }

    /// Snap both previous and current transforms to `(position, yaw)`.
    #[inline]
    pub fn snap_pose(&mut self, position: Vec3, yaw: f32) {
        self.set_position(position);
        self.set_yaw(yaw);
        self.prev_position = position;
        self.prev_yaw = yaw;
    }

    /// Evaluate sub-frame interpolated `(position, yaw)` for `alpha` in `[0.0, 1.0]`.
    #[inline]
    pub fn interpolated_pose(&self, alpha: f32) -> (Vec3, f32) {
        let t = alpha.clamp(0.0, 1.0);
        let pos = self.prev_position.lerp(self.position(), t);
        let yaw = crate::timing::lerp_angle_shortest(self.prev_yaw, self.yaw(), t);
        (pos, yaw)
    }

    /// Borrow shared `en_common.c` combat stats.
    pub fn stats(&self) -> &CreatureStats {
        match &self.controller {
            ActiveEnemy::LyingFigure(e) => &e.stats,
            ActiveEnemy::Mannequin(e) => &e.stats,
            ActiveEnemy::Nurse(e) => &e.stats,
            ActiveEnemy::Creeper(e) => &e.stats,
            ActiveEnemy::PyramidHead(e) => &e.stats,
            ActiveEnemy::AbstractDaddy(e) => &e.stats,
            ActiveEnemy::FleshLips(e) => &e.stats,
            ActiveEnemy::Mandarin(e) => &e.stats,
            ActiveEnemy::Eddie(e) => &e.stats,
            ActiveEnemy::TwinPyramidHead(e) => &e.stats,
            ActiveEnemy::MaryBoss(e) => &e.stats,
            ActiveEnemy::RoomInsect(e) => &e.stats,
        }
    }

    /// Mutably borrow shared `en_common.c` combat stats.
    pub fn stats_mut(&mut self) -> &mut CreatureStats {
        match &mut self.controller {
            ActiveEnemy::LyingFigure(e) => &mut e.stats,
            ActiveEnemy::Mannequin(e) => &mut e.stats,
            ActiveEnemy::Nurse(e) => &mut e.stats,
            ActiveEnemy::Creeper(e) => &mut e.stats,
            ActiveEnemy::PyramidHead(e) => &mut e.stats,
            ActiveEnemy::AbstractDaddy(e) => &mut e.stats,
            ActiveEnemy::FleshLips(e) => &mut e.stats,
            ActiveEnemy::Mandarin(e) => &mut e.stats,
            ActiveEnemy::Eddie(e) => &mut e.stats,
            ActiveEnemy::TwinPyramidHead(e) => &mut e.stats,
            ActiveEnemy::MaryBoss(e) => &mut e.stats,
            ActiveEnemy::RoomInsect(e) => &mut e.stats,
        }
    }

    /// Authentic `enSetSize` (`0x001426e0`) dimensions for this enemy's current posture.
    #[inline]
    pub fn size(&self) -> EnemySize {
        if self.is_downed() {
            EnemySize::downed_for(self.species())
        } else {
            EnemySize::standing_for(self.species())
        }
    }

    /// Active weapon-hit cylinder (`enSetHitColumn` `0x001440c0` + `EnBattleCollisionRate` `0x002a9bf0`).
    ///
    /// Note: For gun/melee raycasts against standing upright humanoids, we extend cylinder height
    /// to `2.0 * size.height` (matching James's chest/head ray height `-1000..-1100` mm when `enSetSize`
    /// stores half-height `800..900` mm).
    pub fn hitbox(&self) -> CombatHitbox {
        let sz = self.size();
        let radius = sz.battle_radius(self.species(), self.is_downed());
        if self.is_downed() {
            CombatHitbox::new_downed(self.position(), radius, sz.height)
        } else {
            CombatHitbox::new_standing(self.position(), radius, sz.height * 2.0)
        }
    }

    #[inline]
    pub fn is_dead(&self) -> bool {
        self.controller.is_dead()
    }

    #[inline]
    pub fn is_downed(&self) -> bool {
        self.controller.is_downed()
    }

    /// True when this enemy is currently in an attacking/grappling FSM state.
    pub fn is_attacking(&self) -> bool {
        if self.is_grappling_player {
            return true;
        }
        match &self.controller {
            ActiveEnemy::LyingFigure(e) => e.state == ScuState::Attack,
            ActiveEnemy::Mannequin(e) => e.state == MknState::Attack,
            ActiveEnemy::Nurse(e) => e.state == NseState::Attack,
            ActiveEnemy::Creeper(e) => e.state == TyuState::Attack,
            ActiveEnemy::PyramidHead(e) => matches!(e.state, RedState::Attack | RedState::Grab),
            ActiveEnemy::AbstractDaddy(e) => e.state == PapState::Attack,
            ActiveEnemy::FleshLips(e) => matches!(
                e.state,
                IkeState::Kick | IkeState::Swing | IkeState::Attack | IkeState::Attack2
            ),
            ActiveEnemy::Mandarin(e) => e.active_hand.is_some(),
            ActiveEnemy::Eddie(e) => matches!(e.state, EdbState::Shoot | EdbState::Punch),
            ActiveEnemy::TwinPyramidHead(e) => {
                matches!(
                    e.state,
                    OniState::Attack | OniState::Attack3 | OniState::BothAttack
                )
            }
            ActiveEnemy::MaryBoss(e) => matches!(e.state, BosState::Attack | BosState::Attack2),
            ActiveEnemy::RoomInsect(e) => e.state == InsectState::Attack,
        }
    }

    /// True when this enemy is actively pursuing or fighting the player.
    pub fn is_aggroed(&self) -> bool {
        if self.is_attacking() {
            return true;
        }
        match &self.controller {
            ActiveEnemy::LyingFigure(e) => matches!(
                e.state,
                ScuState::Precaution | ScuState::Chase | ScuState::Backward | ScuState::Crawl
            ),
            ActiveEnemy::Mannequin(e) => {
                matches!(
                    e.state,
                    MknState::Precaution | MknState::Chase | MknState::Circle
                )
            }
            ActiveEnemy::Nurse(e) => matches!(e.state, NseState::Precaution | NseState::Chase),
            ActiveEnemy::Creeper(e) => e.state == TyuState::Swarm,
            ActiveEnemy::PyramidHead(e) => matches!(e.state, RedState::Chase | RedState::Stairs),
            ActiveEnemy::AbstractDaddy(e) => matches!(e.state, PapState::Chase | PapState::Rush),
            ActiveEnemy::FleshLips(e) => matches!(e.state, IkeState::Chase | IkeState::Move),
            ActiveEnemy::Mandarin(e) => e.state == ArmState::Wander,
            ActiveEnemy::Eddie(e) => matches!(e.state, EdbState::Approach | EdbState::Search),
            ActiveEnemy::TwinPyramidHead(e) => {
                matches!(e.state, OniState::Chase | OniState::Berserk)
            }
            ActiveEnemy::MaryBoss(e) => matches!(e.state, BosState::Approach | BosState::Distance),
            ActiveEnemy::RoomInsect(e) => e.state == InsectState::Approach,
        }
    }

    /// Retail `sh2_attack_list` ID for this enemy's active attack (or `None` when not attacking).
    pub fn active_attack_id(&self) -> Option<u32> {
        if !self.is_attacking() || self.is_dead() {
            return None;
        }
        match &self.controller {
            ActiveEnemy::LyingFigure(_) => Some(42), // SCU acid fog (proto 0x24 + 6)
            ActiveEnemy::Mannequin(_) => Some(43),   // MKN strike (proto 0x25 + 6)
            ActiveEnemy::Nurse(_) => Some(46),       // NSE pipe strike (proto 0x28 + 6)
            ActiveEnemy::Creeper(_) => Some(44),     // TYU bite
            ActiveEnemy::PyramidHead(e) => e.active_attack_id.map(u32::from).or(Some(50)), // RED slash / seize
            ActiveEnemy::AbstractDaddy(_) => Some(63), // PAP grab/hug
            ActiveEnemy::FleshLips(_) => Some(60),     // IKE strike/hug
            ActiveEnemy::Mandarin(_) => Some(70),      // ARM needle (proto 0x40 + 6)
            ActiveEnemy::Eddie(e) => match e.state {
                EdbState::Shoot => Some(58), // EDB revolver shot (proto 0x34 + 6, 5000mm)
                _ => Some(59),               // EDB punch
            },
            ActiveEnemy::TwinPyramidHead(_) => Some(55), // ONI spear strike
            ActiveEnemy::MaryBoss(_) => Some(65),        // BOS tentacle/hug
            ActiveEnemy::RoomInsect(_) => Some(45),
        }
    }

    /// Borrow this enemy's skeletal animation controller.
    pub fn anim(&self) -> &CreatureAnimationController {
        self.controller.anim().unwrap_or(&self.fallback_anim)
    }

    /// Mutably borrow this enemy's skeletal animation controller.
    pub fn anim_mut(&mut self) -> &mut CreatureAnimationController {
        match &mut self.controller {
            ActiveEnemy::LyingFigure(e) => &mut e.anim,
            ActiveEnemy::Mannequin(e) => &mut e.anim,
            ActiveEnemy::Nurse(e) => &mut e.anim,
            ActiveEnemy::Creeper(e) => &mut e.anim,
            _ => &mut self.fallback_anim,
        }
    }

    /// Advance fallback skeletal animation for controllers without an embedded `anim` field.
    pub fn tick_fallback_anim(&mut self, dt: f32) {
        if self.controller.anim().is_none() {
            let yaw = self.yaw();
            self.fallback_anim.update(dt, yaw);
        }
    }

    /// Apply hitback impulse displacement from `enMoveExec` (`0x00143aa0` lines 2113–2140).
    pub fn step_hitback(&mut self) {
        let radius = self.size().radius;
        let disp = self.stats_mut().step_hit_back(radius);
        if disp != Vec3::ZERO {
            let next = self.position() + disp;
            self.set_position(next);
        }
    }

    /// Apply a player weapon hit using authentic `enCheckDamage` (`0x001423a0`) +
    /// `enReduceHP` (`0x00142200`) + `enSetHitBack` (`0x00142630`).
    pub fn receive_hit(&mut self, atk: &AttackSpec, attack_dir_xz: [f32; 2]) {
        let _ = self.receive_hit_with_overflow(atk, attack_dir_xz);
    }

    /// Apply a player weapon hit and return `(twin_id, overflow_damage)` when a
    /// [`ActiveEnemy::TwinPyramidHead`] overflows HP below `0.0` (`enONISetDamage` `0x0022b370` /
    /// `sh2pc 0x004bc8b0`: `(-next_hp) * 0.75` transferred to partner).
    pub fn receive_hit_with_overflow(
        &mut self,
        atk: &AttackSpec,
        attack_dir_xz: [f32; 2],
    ) -> Option<(u32, f32)> {
        if self.is_dead() {
            return None;
        }
        let species = self.species();
        let was_downed = self.is_downed();
        let scaled_ap =
            self.stats_mut()
                .check_damage(species, atk.id as u16, atk.ap, was_downed, None)?;

        let twin_overflow = match &self.controller {
            ActiveEnemy::TwinPyramidHead(t) => {
                let overflow = (scaled_ap - t.stats.health).max(0.0)
                    * crate::ai::creatures::twin_pyramid_head::ONI_SHARED_OVERFLOW_PC;
                (overflow > 0.0).then_some((t.id, overflow))
            }
            _ => None,
        };

        self.attack_work.init();
        self.stats_mut().reduce_hp(scaled_ap);
        if atk.sp > 0.0 {
            self.stats_mut().set_hit_back(attack_dir_xz, atk.sp);
        }

        // `enCheckInstantDeath` (`sh2pc.exe` `0x004a6330`): retail finish kicks/stomps `31..=40`
        // (`0x1f..=0x28`, shifted +6 from proto `0x19..=0x22`) finish downed enemies immediately.
        let is_instant_death_finish =
            was_downed && ((31..=40).contains(&atk.id) || atk.ap >= 9999.0);
        let dead = self.stats().health <= 0.0 || is_instant_death_finish;
        if dead {
            self.stats_mut().health = 0.0;
            self.stats_mut().endurance = 0.0;
            self.apply_post_damage_state(true, true);
        } else if was_downed {
            self.apply_post_damage_state(false, true);
        } else if self.stats().endurance <= 0.0 {
            self.stats_mut().down_timer = 10.0;
            self.apply_post_damage_state(false, true);
        } else {
            self.stats_mut().flinch_timer = 0.5;
            self.apply_post_damage_state(false, false);
        }
        twin_overflow
    }

    /// Apply Hyper Spray aerosol hit (`enCheckDamage` `0x001423a0` + `enCheckSpray` `0x001425a0`).
    pub fn receive_hyper_spray_hit(&mut self, atk: &AttackSpec, color: HyperSprayColor) -> bool {
        if self.is_dead() {
            return false;
        }
        let species = self.species();
        let was_downed = self.is_downed();
        let Some(scaled_ap) =
            self.stats_mut()
                .check_damage(species, atk.id as u16, atk.ap, was_downed, Some(color))
        else {
            return false;
        };

        let is_boss = matches!(
            species,
            EnemySpecies::PyramidHead
                | EnemySpecies::TwinPyramidHead
                | EnemySpecies::AbstractDaddy
                | EnemySpecies::FleshLips
                | EnemySpecies::Eddie
                | EnemySpecies::MaryBoss
        );

        self.attack_work.init();
        if !is_boss && color.is_lethal_to_regular_enemies() {
            self.stats_mut().health = 0.0;
            self.stats_mut().endurance = 0.0;
            self.apply_post_damage_state(true, true);
        } else {
            self.stats_mut().reduce_hp(scaled_ap);
            if self.stats().health <= 0.0 {
                self.apply_post_damage_state(true, true);
            } else if self.stats().endurance <= 0.0 {
                self.stats_mut().down_timer = 10.0;
                self.apply_post_damage_state(false, true);
            } else {
                self.stats_mut().flinch_timer = 0.5;
                self.apply_post_damage_state(false, false);
            }
        }
        true
    }

    /// Synchronize inner controller FSM state after taking damage/knockdown/death.
    fn apply_post_damage_state(&mut self, dead: bool, downed: bool) {
        match &mut self.controller {
            ActiveEnemy::LyingFigure(e) => {
                e.state = if dead {
                    ScuState::Dying
                } else if downed {
                    ScuState::Down
                } else {
                    ScuState::Damage
                };
                e.anim.sync_lying_figure(e.state);
            }
            ActiveEnemy::Mannequin(e) => {
                e.state = if dead {
                    MknState::Dying
                } else if downed {
                    MknState::Down
                } else {
                    MknState::Damage
                };
                e.anim.sync_mannequin(e.state, e.is_kick_next);
            }
            ActiveEnemy::Nurse(e) => {
                e.state = if dead {
                    NseState::Dying
                } else if downed {
                    NseState::Down
                } else {
                    NseState::Damage
                };
                e.anim.sync_nurse(e.state);
            }
            ActiveEnemy::Creeper(e) => {
                if dead {
                    e.state = TyuState::Dead;
                }
            }
            ActiveEnemy::AbstractDaddy(e) => {
                e.state = if dead {
                    PapState::Dying
                } else if downed {
                    PapState::Down
                } else {
                    PapState::Damage
                };
            }
            ActiveEnemy::FleshLips(e) => {
                if dead {
                    e.state = IkeState::Dead;
                }
            }
            ActiveEnemy::Mandarin(e) => {
                e.state = if dead {
                    ArmState::Dead
                } else {
                    ArmState::Damage
                };
            }
            ActiveEnemy::Eddie(e) => {
                e.state = if dead {
                    EdbState::Dead
                } else {
                    EdbState::Damage
                };
            }
            ActiveEnemy::MaryBoss(e) => {
                if dead {
                    e.state = BosState::Dead;
                }
            }
            ActiveEnemy::TwinPyramidHead(e) => {
                e.state = if dead {
                    OniState::Dead
                } else {
                    OniState::Damage
                };
            }
            ActiveEnemy::PyramidHead(e) => {
                if dead && e.is_boss_encounter && e.state != RedState::Leave {
                    e.state = RedState::Leave;
                    e.sub_state = 0;
                    e.active_attack_id = None;
                }
            }
            ActiveEnemy::RoomInsect(_) => {}
        }

        if self.controller.anim().is_none() && (dead || downed) {
            let clip = self.fallback_anim.current_clip();
            self.fallback_anim.transition_to(
                CreatureAnimStateKind::Knockdown,
                clip,
                0.0,
                None,
                None,
            );
        }
    }

    /// Force an enemy into attacking or non-attacking state (used by grapple sessions & tests).
    pub fn set_attacking(&mut self, attacking: bool) {
        if !attacking {
            self.is_grappling_player = false;
            self.attack_work.init();
        }
        match &mut self.controller {
            ActiveEnemy::LyingFigure(e) => {
                e.state = if attacking {
                    ScuState::Attack
                } else {
                    ScuState::Chase
                };
            }
            ActiveEnemy::Mannequin(e) => {
                e.state = if attacking {
                    MknState::Attack
                } else {
                    MknState::Chase
                };
            }
            ActiveEnemy::Nurse(e) => {
                e.state = if attacking {
                    NseState::Attack
                } else {
                    NseState::Chase
                };
            }
            ActiveEnemy::Creeper(e) => {
                e.state = if attacking {
                    TyuState::Attack
                } else {
                    TyuState::Swarm
                };
            }
            ActiveEnemy::PyramidHead(e) => {
                e.state = if attacking {
                    RedState::Grab
                } else {
                    RedState::Chase
                };
            }
            ActiveEnemy::AbstractDaddy(e) => {
                e.state = if attacking {
                    PapState::Attack
                } else {
                    PapState::Chase
                };
            }
            ActiveEnemy::FleshLips(e) => {
                e.state = if attacking {
                    IkeState::Attack
                } else {
                    IkeState::Chase
                };
            }
            ActiveEnemy::Mandarin(e) => {
                e.active_hand = if attacking { Some(0) } else { None };
                e.state = ArmState::Wander;
            }
            ActiveEnemy::Eddie(e) => {
                e.state = if attacking {
                    EdbState::Punch
                } else {
                    EdbState::Approach
                };
            }
            ActiveEnemy::TwinPyramidHead(e) => {
                e.state = if attacking {
                    OniState::Attack
                } else {
                    OniState::Chase
                };
            }
            ActiveEnemy::MaryBoss(e) => {
                e.state = if attacking {
                    BosState::Attack
                } else {
                    BosState::Approach
                };
            }
            ActiveEnemy::RoomInsect(e) => {
                e.state = if attacking {
                    InsectState::Attack
                } else {
                    InsectState::Approach
                };
            }
        }
    }

    /// Force an enemy into its active aggro/chase state (used by CombatArena TAB target cycling).
    pub fn force_aggro(&mut self) {
        self.stats_mut().flinch_timer = 0.0;
        self.set_attacking(false);
    }

    /// Force an enemy into downed state for stomp/kick testing.
    pub fn force_downed(&mut self, down_secs: f32) {
        self.stats_mut().down_timer = down_secs;
        self.apply_post_damage_state(false, true);
    }
}
