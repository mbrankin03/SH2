//! Arena target enemy state for combat simulation and testing.

use super::attack_list::{AttackSpec, HyperSprayColor};
use super::hitboxes::CombatHitbox;
use crate::ai::creatures::anim::CreatureAnimationController;
use glam::Vec3;
use std::f32::consts::PI;

/// Simulated monster/enemy in combat room for testing attack interactions.
#[derive(Debug, Clone)]
pub struct CombatTargetEnemy {
    pub id: u32,
    pub name: &'static str,
    pub prev_position: Vec3,
    pub position: Vec3,
    pub prev_yaw: f32,
    pub yaw: f32,
    pub spawn_origin: Vec3,
    pub aggro_radius: f32,
    pub is_aggroed: bool,
    pub health: f32,
    pub max_health: f32,
    pub endurance: f32,
    pub max_endurance: f32,
    pub is_downed: bool,
    pub is_dead: bool,
    pub flinch_timer: f32,
    pub down_timer: f32,
    pub attack_cooldown: f32,
    pub attack_timer: f32,
    pub is_attacking: bool,
    pub is_kick_next: bool,
    pub is_grappling_player: bool,
    pub hitbox_radius: f32,
    pub hitbox_height: f32,
    pub anim: CreatureAnimationController,
}

impl CombatTargetEnemy {
    /// Create a Lying Figure (scu) test target with authentic Normal difficulty values.
    /// Health: 700, Endurance: 285 (`0x4c46c0`).
    pub fn new_lying_figure(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Lying Figure (scu)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 700.0,
            max_health: 700.0,
            endurance: 285.0,
            max_endurance: 285.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.5,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 350.0,
            hitbox_height: 1500.0,
            anim: CreatureAnimationController::new_lying_figure(),
        }
    }

    /// Create a Mannequin (mkn) test target with authentic Normal difficulty values.
    /// Health: 490 (700 * 0.7), Endurance: 200 (`0x4c15c0`).
    pub fn new_mannequin(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Mannequin (mkn)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 490.0,
            max_health: 490.0,
            endurance: 200.0,
            max_endurance: 200.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.8,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 300.0,
            hitbox_height: 1600.0,
            anim: CreatureAnimationController::new_mannequin(),
        }
    }

    /// Create a Bubble Head Nurse (nse) test target with authentic Normal difficulty values.
    /// Health: 600, Endurance: 260.
    pub fn new_nurse(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Bubble Head Nurse (nse)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 600.0,
            max_health: 600.0,
            endurance: 260.0,
            max_endurance: 260.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.6,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 320.0,
            hitbox_height: 1600.0,
            anim: CreatureAnimationController::new_nurse(),
        }
    }

    /// Create a Pyramid Head (red) boss target.
    /// Health: 2000, Endurance: 1000.
    pub fn new_pyramid_head(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Pyramid Head (red)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2000.0,
            is_aggroed: false,
            health: 2000.0,
            max_health: 2000.0,
            endurance: 1000.0,
            max_endurance: 1000.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 1.0,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 450.0,
            hitbox_height: 2200.0,
            anim: CreatureAnimationController::new_pyramid_head(),
        }
    }

    /// Create an Abstract Daddy (pap) boss target.
    /// Health: 1800, Endurance: 800.
    pub fn new_abstract_daddy(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Abstract Daddy (pap)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2000.0,
            is_aggroed: false,
            health: 1800.0,
            max_health: 1800.0,
            endurance: 800.0,
            max_endurance: 800.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.8,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 500.0,
            hitbox_height: 1800.0,
            anim: CreatureAnimationController::new_abstract_daddy(),
        }
    }

    /// Create a Flesh Lips (ike) boss target.
    /// Health: 1200, Endurance: 600.
    pub fn new_flesh_lips(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Flesh Lips (ike)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2000.0,
            is_aggroed: false,
            health: 1200.0,
            max_health: 1200.0,
            endurance: 600.0,
            max_endurance: 600.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 1.0,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 400.0,
            hitbox_height: 1800.0,
            anim: CreatureAnimationController::new_flesh_lips(),
        }
    }

    /// Create a Mandarin (arm) target placed above ground for animation visibility.
    /// Health: 500, Endurance: 250.
    pub fn new_mandarin(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Mandarin (arm)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 500.0,
            max_health: 500.0,
            endurance: 250.0,
            max_endurance: 250.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.7,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 350.0,
            hitbox_height: 1400.0,
            anim: CreatureAnimationController::new_mandarin(),
        }
    }

    /// Create a Creeper (tyu) insect swarm target.
    /// Health: 50, Endurance: 20 (dies in 1 stomp or plank hit).
    pub fn new_creeper(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Creeper (tyu)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1600.0,
            is_aggroed: false,
            health: 50.0,
            max_health: 50.0,
            endurance: 20.0,
            max_endurance: 20.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.4,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 150.0,
            hitbox_height: 200.0,
            anim: CreatureAnimationController::new_creeper(),
        }
    }

    /// Create a Mary / Maria Final Boss (bos) target.
    /// Health: 2500, Endurance: 1200.
    pub fn new_mary_boss(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Mary Final Boss (bos)",
            prev_position: position,
            position,
            prev_yaw: 0.0,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2200.0,
            is_aggroed: false,
            health: 2500.0,
            max_health: 2500.0,
            endurance: 1200.0,
            max_endurance: 1200.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 1.2,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 500.0,
            hitbox_height: 2400.0,
            anim: CreatureAnimationController::new_mary_boss(),
        }
    }

    /// Snaps the enemy's previous and current tick transforms to `(position, yaw)`.
    #[inline]
    pub fn snap_pose(&mut self, position: Vec3, yaw: f32) {
        self.prev_position = position;
        self.position = position;
        self.prev_yaw = yaw;
        self.yaw = yaw;
    }

    /// Evaluates sub-frame interpolated enemy `(position, yaw)` for `alpha` in `[0.0, 1.0]`.
    #[inline]
    pub fn interpolated_pose(&self, alpha: f32) -> (Vec3, f32) {
        let t = alpha.clamp(0.0, 1.0);
        let pos = self.prev_position.lerp(self.position, t);
        let yaw = crate::timing::lerp_angle_shortest(self.prev_yaw, self.yaw, t);
        (pos, yaw)
    }

    /// Get current active collision hitbox.
    pub fn hitbox(&self) -> CombatHitbox {
        if self.is_downed {
            CombatHitbox::new_downed(self.position, self.hitbox_radius, self.hitbox_height)
        } else {
            CombatHitbox::new_standing(self.position, self.hitbox_radius, self.hitbox_height)
        }
    }

    /// Apply weapon damage (AP) and stagger (SP) (`0x4c65d0`).
    /// Apply attack damage (`enReduceHP`, `SLUS_202.28` `0x00142200`).
    ///
    /// In Team Silent's engine, `ap` (`sh2_attack_list +0x04`) reduces **both** HP and Endurance,
    /// while `sp` (`+0x08`) drives hitback impulse (`sp * 2.0` in `enSetHitBack` `0x00142630`).
    pub fn receive_hit(&mut self, atk: &AttackSpec) {
        if self.is_dead {
            return;
        }

        self.health = (self.health - atk.ap).max(0.0);
        self.endurance = (self.endurance - atk.ap).max(0.0);
        self.is_attacking = false;
        self.attack_timer = 0.0;
        self.is_aggroed = true; // Retaliate immediately if struck

        if self.is_downed {
            // Attacks against downed enemies finish them off or trigger death twitch
            if self.health <= 0.0 || atk.ap >= 9999.0 {
                self.is_dead = true;
            }
            self.sync_animation();
            return;
        }

        if self.health <= 0.0 || self.endurance <= 0.0 {
            self.is_downed = true;
            self.down_timer = 10.0; // Stays down for 10 seconds (300 ticks)
        } else {
            self.flinch_timer = 0.5; // Flinch stagger for 0.5s
        }

        // Sync creature animation state for all species (1..=9)
        self.sync_animation();
    }

    /// Apply Hyper Spray aerosol hit (`enCheckDamage` `SLUS_202.28` `0x001424dc` lines 814–822
    /// & `enCheckSpray` `0x001425a0`).
    ///
    /// - Bosses (`Pyramid Head = 4`, `Abstract Daddy = 5`, `Flesh Lips = 6`, `Mary = 9`) are
    ///   unaffected by White (`0`) or Yellow (`1`) spray (`0 <= enGetSprayPower() < 2`).
    /// - Green spray (`2`) kills regular non-boss enemies and stuns bosses.
    /// - Purple (`-1`), White (`0`), and Yellow (`1`) stun regular enemies and apply `atk.ap`.
    pub fn receive_hyper_spray_hit(&mut self, atk: &AttackSpec, color: HyperSprayColor) -> bool {
        if self.is_dead {
            return false;
        }

        let is_boss = matches!(self.id, 4 | 5 | 6 | 9);
        if is_boss && !color.affects_bosses() {
            return false;
        }

        self.is_attacking = false;
        self.attack_timer = 0.0;
        self.is_aggroed = true;

        if !is_boss && color.is_lethal_to_regular_enemies() {
            self.health = 0.0;
            self.endurance = 0.0;
            self.is_downed = true;
            self.is_dead = true;
        } else {
            self.health = (self.health - atk.ap).max(0.0);
            self.endurance = (self.endurance - atk.ap).max(0.0);
            if self.health <= 0.0 || self.endurance <= 0.0 {
                self.is_downed = true;
                self.down_timer = 10.0;
            } else {
                self.flinch_timer = 0.5;
            }
        }

        self.sync_animation();
        true
    }

    /// Synchronize the skeletal `CreatureAnimationController` with the creature's current combat state.
    pub fn sync_animation(&mut self) {
        match self.id {
            1 => {
                let scu_state = if self.is_dead || self.is_downed {
                    crate::ai::creatures::lying_figure::ScuState::Down
                } else if self.flinch_timer > 0.0 {
                    crate::ai::creatures::lying_figure::ScuState::Damage
                } else if self.is_attacking {
                    crate::ai::creatures::lying_figure::ScuState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::lying_figure::ScuState::Chase
                } else {
                    crate::ai::creatures::lying_figure::ScuState::Wander
                };
                self.anim.sync_lying_figure(scu_state);
            }
            2 => {
                let mkn_state = if self.is_dead || self.is_downed {
                    crate::ai::creatures::mannequin::MknState::Down
                } else if self.flinch_timer > 0.0 {
                    crate::ai::creatures::mannequin::MknState::Damage
                } else if self.is_attacking {
                    crate::ai::creatures::mannequin::MknState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::mannequin::MknState::Chase
                } else {
                    crate::ai::creatures::mannequin::MknState::Event
                };
                self.anim.sync_mannequin(mkn_state, self.is_kick_next);
            }
            3 => {
                let nse_state = if self.is_dead || self.is_downed {
                    crate::ai::creatures::nurse::NseState::Down
                } else if self.flinch_timer > 0.0 {
                    crate::ai::creatures::nurse::NseState::Damage
                } else if self.is_attacking {
                    crate::ai::creatures::nurse::NseState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::nurse::NseState::Chase
                } else {
                    crate::ai::creatures::nurse::NseState::Confuse
                };
                self.anim.sync_nurse(nse_state);
            }
            4 => {
                let red_state = if self.is_grappling_player {
                    crate::ai::creatures::pyramid_head::RedState::Grab
                } else if self.is_attacking {
                    crate::ai::creatures::pyramid_head::RedState::Attack
                } else if self.is_aggroed && !self.is_dead && !self.is_downed {
                    crate::ai::creatures::pyramid_head::RedState::Chase
                } else {
                    crate::ai::creatures::pyramid_head::RedState::Stand
                };
                self.anim.sync_pyramid_head(red_state);
            }
            5 => {
                // Abstract Daddy (pap)
                use crate::ai::creatures::anim::{AbstractDaddyAnimClips, CreatureAnimStateKind};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        AbstractDaddyAnimClips::KNOCKDOWN,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        AbstractDaddyAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_grappling_player {
                    // Authentic PS2 PAP_ANIM[3] looping strangle hold while smothering James
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        AbstractDaddyAnimClips::STRUGGLE_HOLD,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        AbstractDaddyAnimClips::LUNGE_GRAB,
                        0.0,
                        Some((
                            AbstractDaddyAnimClips::ATTACK_HIT_START,
                            AbstractDaddyAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        AbstractDaddyAnimClips::WALK,
                        AbstractDaddyAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        AbstractDaddyAnimClips::STAND,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            6 => {
                // Flesh Lips (ike)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, FleshLipsAnimClips};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        FleshLipsAnimClips::FALL_DEATH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        FleshLipsAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_grappling_player {
                    // Authentic PS2 IKE_ANIM[4] looping choke hold while straddling and strangling James
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        FleshLipsAnimClips::CHOKE_HOLD,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        FleshLipsAnimClips::GRAB_DESCEND,
                        0.0,
                        Some((
                            FleshLipsAnimClips::ATTACK_HIT_START,
                            FleshLipsAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        FleshLipsAnimClips::GLIDE_ADVANCE,
                        FleshLipsAnimClips::SWING_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        FleshLipsAnimClips::HANG_IDLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            7 => {
                // Mandarin (arm)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, MandarinAnimClips};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        MandarinAnimClips::KNOCKDOWN,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        MandarinAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        MandarinAnimClips::TENTACLE_STRIKE,
                        0.0,
                        Some((
                            MandarinAnimClips::ATTACK_HIT_START,
                            MandarinAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        MandarinAnimClips::CRAWL,
                        MandarinAnimClips::CRAWL_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        MandarinAnimClips::HANG_IDLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            8 => {
                // Creeper (tyu)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, CreeperAnimClips};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        CreeperAnimClips::SCUTTLE,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking || self.is_grappling_player {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        CreeperAnimClips::BITE,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        CreeperAnimClips::SCUTTLE,
                        CreeperAnimClips::SCUTTLE_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        CreeperAnimClips::SCUTTLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            9 => {
                // Mary Final Boss (bos)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, MaryBossAnimClips};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        MaryBossAnimClips::DEATH_FALL,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        MaryBossAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_grappling_player {
                    // Authentic PS2 enBOSCtrlAttack (0x00140450):
                    // When Mary latches onto James, she transitions through LIFT_CARRY (74..=88) into CHOKE_HOLD (89..=112)
                    let choke_clip = if self.anim.current_clip() == MaryBossAnimClips::CHOKE_HOLD
                        || (self.anim.current_clip() == MaryBossAnimClips::LIFT_CARRY
                            && self.anim.is_clip_finished())
                    {
                        MaryBossAnimClips::CHOKE_HOLD
                    } else {
                        MaryBossAnimClips::LIFT_CARRY
                    };
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        choke_clip,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        MaryBossAnimClips::LUNGE_CATCH,
                        0.0,
                        Some((
                            MaryBossAnimClips::ATTACK_HIT_START,
                            MaryBossAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        MaryBossAnimClips::HOVER_IDLE,
                        MaryBossAnimClips::HOVER_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        MaryBossAnimClips::HOVER_IDLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            _ => {}
        }
    }

    /// Update enemy timers, attack logic, and navigation towards player.
    pub fn update(&mut self, player_pos: Vec3, dt: f32) {
        self.prev_position = self.position;
        self.prev_yaw = self.yaw;

        if self.is_dead {
            self.sync_animation();
            self.anim.update(dt, self.yaw);
            return;
        }
        if self.flinch_timer > 0.0 {
            self.flinch_timer = (self.flinch_timer - dt).max(0.0);
        }
        if self.down_timer > 0.0 {
            self.down_timer -= dt;
            if self.down_timer <= 0.0 && self.health > 0.0 {
                // Recover from floor
                self.is_downed = false;
                self.endurance = self.max_endurance * 0.5;
            }
        }
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }

        if !self.is_downed && self.flinch_timer <= 0.0 {
            let dx = player_pos.x - self.position.x;
            let dz = player_pos.z - self.position.z;
            let dist = (dx * dx + dz * dz).sqrt();

            // Aggro threshold checking:
            // Monster activates if player enters its section territory (aggro_radius)
            // or if it was struck by an attack.
            if !self.is_aggroed && dist <= self.aggro_radius {
                self.is_aggroed = true;
            }

            if self.is_aggroed {
                let desired_yaw = dx.atan2(dz);

                // Smoothly turn towards player
                let mut diff = desired_yaw - self.yaw;
                while diff > PI {
                    diff -= 2.0 * PI;
                }
                while diff < -PI {
                    diff += 2.0 * PI;
                }
                if self.is_grappling_player {
                    // Holding player in grapple lock (0x4b7160 / 0x4c00a0):
                    // Attack timer does not expire, walk root motion is suspended,
                    // and GrappleSession owns creature yaw/position coupling.
                    self.is_attacking = true;
                    self.attack_timer = 0.0;
                } else {
                    // Smoothly turn towards player when not locked in grapple
                    self.yaw += diff.clamp(-3.0 * dt, 3.0 * dt);
                }

                if self.is_grappling_player {
                    // Already handled above
                } else if self.is_attacking {
                    self.attack_timer += dt;

                    // Forward lunge root motion during initial attack swing/lunge frames:
                    // - Abstract Daddy (5): authentic PS2 pap_attack (enemy.c:3275-3312) advances towards James to close the gap
                    // - Pyramid Head (4): Great Knife forward momentum
                    // - Mary Boss (9): authentic PS2 enBOSCtrlAttack (0x0014055c) swoops forward 600mm to catch James
                    // - Mannequin (2) / Nurse (3): slight attack step
                    let lunge_speed = match self.id {
                        5 => 500.0,
                        4 => 250.0,
                        9 => 600.0,
                        2 | 3 => 150.0,
                        _ => 0.0,
                    };
                    if lunge_speed > 0.0 && dist > 150.0 && self.attack_timer < 0.45 {
                        self.position.x += self.yaw.sin() * lunge_speed * dt;
                        self.position.z += self.yaw.cos() * lunge_speed * dt;
                    }

                    // Authentic animation-driven attack duration:
                    // Rather than an arbitrary 1.0s hard cutoff, wait until the non-looping
                    // attack clip finishes, or fall back to authentic clip duration per creature
                    let clip_dur = self.anim.current_clip().duration_seconds();
                    let min_atk_time = if clip_dur > 0.1 && !self.anim.current_clip().looping {
                        clip_dur
                    } else {
                        match self.id {
                            1 => 0.80, // Lying Figure acid spray / flail
                            2 => 1.17, // Mannequin swing / kick
                            3 => 1.23, // Nurse pipe swing
                            4 => 2.25, // Pyramid Head knife swing
                            5 => 0.67, // Abstract Daddy lunge
                            6 => 1.07, // Flesh Lips ceiling grab descent
                            7 => 0.57, // Mandarin tentacle strike
                            8 => 0.27, // Creeper bite
                            9 => 0.50, // Mary Boss lunge catch
                            _ => 1.0,
                        }
                    };

                    if self.anim.is_clip_finished() || self.attack_timer >= min_atk_time {
                        self.is_attacking = false;
                        self.attack_timer = 0.0;
                        self.attack_cooldown = match self.id {
                            4 => 2.5, // Pyramid Head recovery
                            9 => 1.2, // Mary Boss recovery
                            _ => 1.5,
                        };
                        if self.id != 1 {
                            self.is_kick_next = !self.is_kick_next;
                        }
                    }
                } else {
                    let (attack_range, max_angle) = match self.id {
                        1 => (1200.0, 0.6), // Lying figure acid spray
                        2 => (650.0, 0.6),  // Mannequin
                        3 => (800.0, 0.6),  // Nurse pipe swing
                        4 => (900.0, 0.6),  // Pyramid Head
                        // Authentic PS2 pap_sense (enemy.c:3166): e->p_dist < 130.0 + 405.0 = 535.0, angle < 0.1745 (10 deg)
                        5 => (535.0, 0.35), // Abstract Daddy lunge grab
                        6 => (650.0, 0.6),  // Flesh Lips choke descent
                        7 => (600.0, 0.6),  // Mandarin tentacle strike
                        8 => (200.0, 0.6),  // Creeper bite
                        // Authentic PS2 enBOSCanAttackPlayer (0x00140b24): dist <= 600.0, angle <= 0.0872 rad (~5 deg)
                        9 => (600.0, 0.35), // Mary final boss tentacle grapple lunge
                        _ => (700.0, 0.6),
                    };
                    if dist <= attack_range && self.attack_cooldown <= 0.0 && diff.abs() < max_angle
                    {
                        // Trigger creature attack
                        self.is_attacking = true;
                        self.attack_timer = 0.0;
                    } else if dist > 150.0 {
                        // Move towards player using authentic walk root motion
                        let walk_speed = match self.id {
                            1 => 402.0,  // scu
                            2 => 576.0,  // mkn
                            3 => 480.0,  // nse
                            4 => 330.0,  // red
                            5 => 540.0,  // pap
                            6 => 360.0,  // ike
                            7 => 450.0,  // arm
                            8 => 1200.0, // tyu
                            9 => 420.0,  // bos
                            _ => 400.0,
                        };
                        self.position.x += self.yaw.sin() * walk_speed * dt;
                        self.position.z += self.yaw.cos() * walk_speed * dt;
                    }
                }
            } else {
                // Return smoothly towards home anchor if drifted, or idle in place
                let home_dx = self.spawn_origin.x - self.position.x;
                let home_dz = self.spawn_origin.z - self.position.z;
                let home_dist = (home_dx * home_dx + home_dz * home_dz).sqrt();
                if home_dist > 50.0 {
                    self.yaw = home_dx.atan2(home_dz);
                    self.position.x += self.yaw.sin() * 200.0 * dt;
                    self.position.z += self.yaw.cos() * 200.0 * dt;
                }
            }
        }

        // Sync and advance creature animation
        self.sync_animation();
        self.anim.update(dt, self.yaw);
    }
}
