//! Active Grapple Session State Machine, Button Mashing, and Lifecycle Management.
//!
//! Ground truth decompiled from `prototype_extracted/SLUS_202.28`:
//! - `PlayerCheckHuggingAttack` (`0x00110070`): Initializes grab state machine and `hugging_gauge`
//! - `PlayerCheckHuggingButton` (`0x0021c360`): Escape gauge draining via button mashing and stick flicks
//! - `shBattleHuggedHuman` (`0x00124db0`): Validates hold status and triggers breakout recoil

use super::anchor::compute_creature_grapple_transform;
use super::tables::{get_grapple_spec, BestiaryId, GrappleSpec};
use crate::ai::creatures::anim::CreatureAnimStateKind;
use crate::animation::AnimationClip;
use crate::combat::{CombatController, CombatMode, CombatTargetEnemy};
use glam::Vec3;

/// Lifecycle phases of an active creature grapple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrapplePhase {
    /// Initial coupling handshake; positions and orientations snapping into place.
    Latching,
    /// Sustained hold loop: creature executes hold animation, James executes struggle animation,
    /// damage-over-time is applied, and button mashing drains the struggle meter.
    Holding,
    /// James has overpowered the hold (gauge <= 0) and executes breakout release push (`aid 130`).
    BreakingOut,
    /// Session has completed or was prematurely interrupted (e.g. enemy flinched/died); ready for cleanup.
    Terminated,
}

/// An active grapple session governing coupling, animations, damage, and escape mechanics.
#[derive(Debug, Clone, PartialEq)]
pub struct GrappleSession {
    /// Index into the combat arena's enemy array.
    pub enemy_idx: usize,
    /// Enemy bestiary identifier.
    pub species: BestiaryId,
    /// Static behavioral specification for this creature's grapple.
    pub spec: GrappleSpec,
    /// Current phase in the grapple lifecycle.
    pub phase: GrapplePhase,
    /// Remaining breakout struggle time (in seconds).
    pub struggle_gauge: f32,
    /// Initial breakout duration (used to calculate progress percentage: 0.0 to 1.0).
    pub initial_gauge: f32,
    /// Timer tracking playback of the breakout release push animation (`aid 130`).
    pub breakout_timer: f32,
    /// Lockstep animation counter synchronized between James and the enemy.
    pub anime_counter: f32,
    /// Total elapsed seconds since the grapple began.
    pub elapsed_time: f32,
}

impl GrappleSession {
    /// Attempt to initiate a grapple session between James and an attacking enemy.
    ///
    /// Validates distance, attack status, and species capability before binding.
    pub fn try_initiate(
        enemy_idx: usize,
        enemy: &mut CombatTargetEnemy,
        player: &mut CombatController,
        player_pos: Vec3,
        player_yaw: &mut f32,
        difficulty: usize,
    ) -> Option<Self> {
        let species = BestiaryId::from_raw(enemy.id)?;
        let spec = *get_grapple_spec(species)?;

        if !enemy.is_attacking || enemy.is_dead || enemy.is_downed || enemy.flinch_timer > 0.0 {
            return None;
        }

        let dx = enemy.position.x - player_pos.x;
        let dz = enemy.position.z - player_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();

        if !dist.is_finite() || dist > spec.grab_range {
            return None;
        }

        if player.mode == CombatMode::Held || player.invulnerability_timer > 0.0 {
            return None;
        }

        // Initialize struggle duration from authentic PS2 hurihodoki_timer
        let duration = spec.breakout_duration(difficulty);

        // Bind enemy and player into grapple state
        enemy.is_grappling_player = true;
        player.mode = CombatMode::Held;
        player.struggle_meter = duration;

        // Snap James to face the grabbing assailant (1:1 with authentic PS2 orientation)
        *player_yaw = dx.atan2(dz);

        Some(Self {
            enemy_idx,
            species,
            spec,
            phase: GrapplePhase::Latching,
            struggle_gauge: duration,
            initial_gauge: duration,
            breakout_timer: 0.0,
            anime_counter: 0.0,
            elapsed_time: 0.0,
        })
    }

    /// Register player struggle button presses or stick flicks to accelerate breakout (`PlayerCheckHuggingButton`).
    pub fn register_struggle_input(&mut self, button_presses: u32) {
        if self.phase == GrapplePhase::Holding || self.phase == GrapplePhase::Latching {
            // Authentic PS2 mechanic: each button mash event shaves 1 frame of hold meter (0.0333s at 30Hz)
            let shave_amount = (button_presses as f32) * 0.0333;
            self.struggle_gauge = (self.struggle_gauge - shave_amount).max(0.0);
        }
    }

    /// Advance the grapple session by `dt` seconds.
    ///
    /// Manages anchoring, damage over time, animation synchrony, and phase transitions.
    pub fn update(
        &mut self,
        dt: f32,
        player: &mut CombatController,
        enemies: &mut [CombatTargetEnemy],
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        if self.phase == GrapplePhase::Terminated {
            return;
        }

        // Validate enemy existence and status
        let enemy = match enemies.get_mut(self.enemy_idx) {
            Some(e) if e.id == (self.species as u32) => e,
            _ => {
                // Enemy array modified or enemy despawned: abort immediately to prevent ghost holds
                self.terminate(player, None);
                return;
            }
        };

        // If player broke free via external struggle input handler (e.g. player.mode != Held), trigger recoil
        if player.mode != CombatMode::Held
            && player.mode != CombatMode::Dying
            && player.mode != CombatMode::Dead
        {
            self.trigger_breakout_recoil(player, enemy, player_yaw);
            return;
        }

        // If enemy died, got knocked down, or was externally flinched from damage during the hold, release James immediately
        if enemy.is_dead
            || enemy.is_downed
            || (self.phase != GrapplePhase::BreakingOut && enemy.flinch_timer > 0.0)
        {
            self.terminate(player, Some(enemy));
            return;
        }

        self.elapsed_time += dt;
        self.anime_counter += dt * 30.0; // 30Hz anime counter

        // Maintain horizontal physical anchoring coupling (PlayerCheckHuggedPos) during latch and hold.
        // Keep enemy.position.y on the floor plane (player_pos.y) so 3D hitboxes stay floor-aligned and
        // visual rendering applies GrappleAnchor::vertical_offset exactly once without double-offsetting.
        if self.phase == GrapplePhase::Latching || self.phase == GrapplePhase::Holding {
            let (creature_pos, creature_yaw) =
                compute_creature_grapple_transform(self.species, player_pos, player_yaw);
            enemy.position = Vec3::new(creature_pos.x, player_pos.y, creature_pos.z);
            enemy.yaw = creature_yaw;
            enemy.is_grappling_player = true;
            enemy.is_attacking = true;
        }

        match self.phase {
            GrapplePhase::Latching => {
                self.phase = GrapplePhase::Holding;
            }
            GrapplePhase::Holding => {
                // Synchronize struggle gauge with player struggle meter if it was modified externally
                if player.struggle_meter < self.struggle_gauge {
                    self.struggle_gauge = player.struggle_meter;
                }

                // Authentic passive time decay: 0.20 * dt (so a 0.80s gauge lasts ~4.0s without mashing)
                self.struggle_gauge = (self.struggle_gauge - 0.20 * dt).max(0.0);
                player.struggle_meter = self.struggle_gauge;

                // Apply authentic PS2 gripping damage over time
                player.health = (player.health - self.spec.hold_dps * dt).max(0.0);

                if player.health <= 0.0 {
                    player.mode = CombatMode::Dying;
                    player.anim_frame = 0.0;
                    self.terminate(player, Some(enemy));
                    return;
                }

                // Check for breakout: transition to BreakingOut phase where James executes shove (aid 130)
                if self.struggle_gauge <= 0.0 {
                    self.phase = GrapplePhase::BreakingOut;
                    self.breakout_timer = 0.0;

                    // Release creature grab binding and push creature back with recoil flinch
                    enemy.is_grappling_player = false;
                    enemy.is_attacking = false;
                    enemy.attack_cooldown = 2.0;
                    enemy.flinch_timer = 0.5;

                    let backward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
                    enemy.position += backward * 250.0;

                    if let Some(recoil_clip) = self.spec.creature_breakout_clip {
                        enemy.anim.transition_to(
                            CreatureAnimStateKind::Flinch,
                            recoil_clip,
                            0.0,
                            None,
                            None,
                        );
                    }
                }
            }
            GrapplePhase::BreakingOut => {
                self.breakout_timer += dt;
                // James plays aid 130 (frames 652..=670 @ 15fps = ~1.2s duration)
                let breakout_duration = self.spec.james_breakout_clip.duration_seconds().min(1.2);
                if self.breakout_timer >= breakout_duration {
                    self.terminate(player, Some(enemy));
                }
            }
            GrapplePhase::Terminated => {}
        }
    }

    /// Trigger authentic PS2 recoil pushback on the creature and start breakout.
    pub fn trigger_breakout_recoil(
        &mut self,
        player: &mut CombatController,
        enemy: &mut CombatTargetEnemy,
        player_yaw: f32,
    ) {
        self.phase = GrapplePhase::Terminated;

        enemy.is_grappling_player = false;
        enemy.is_attacking = false;
        enemy.attack_cooldown = 2.0;
        enemy.flinch_timer = 0.5;

        // Authentic PS2 recoil: push creature backward away from James
        let backward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
        enemy.position += backward * 250.0;

        if let Some(recoil_clip) = self.spec.creature_breakout_clip {
            enemy
                .anim
                .transition_to(CreatureAnimStateKind::Flinch, recoil_clip, 0.0, None, None);
        }

        if player.mode == CombatMode::Held {
            player.mode = CombatMode::Hurt;
            player.struggle_meter = 0.0;
            player.invulnerability_timer = 0.5;
            player.anim_frame = 0.0;
            player.clip_length = 18.0;
        }
    }

    /// Terminate the grapple session and restore both characters cleanly with zero state leaks.
    pub fn terminate(
        &mut self,
        player: &mut CombatController,
        enemy: Option<&mut CombatTargetEnemy>,
    ) {
        self.phase = GrapplePhase::Terminated;

        if let Some(e) = enemy {
            e.is_grappling_player = false;
            e.is_attacking = false;
            e.attack_timer = 0.0;
        }

        if player.mode == CombatMode::Held {
            player.mode = CombatMode::Free;
            player.struggle_meter = 0.0;
            player.invulnerability_timer = 0.5; // Brief post-grapple invulnerability frames
        }
    }

    /// Retrieve the authentic animation clip James should play right now based on active phase.
    #[inline]
    pub fn active_player_clip(&self) -> AnimationClip {
        match self.phase {
            GrapplePhase::Latching | GrapplePhase::Holding => self.spec.james_struggle_clip,
            GrapplePhase::BreakingOut => self.spec.james_breakout_clip,
            GrapplePhase::Terminated => self.spec.james_breakout_clip,
        }
    }

    /// Returns true if the session is fully finished and ready to be dropped.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.phase == GrapplePhase::Terminated
    }
}
