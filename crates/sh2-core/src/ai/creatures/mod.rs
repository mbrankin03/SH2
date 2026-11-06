//! Shared definitions, traits, and damage response logic for SH2 creature state machines.
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c`, `src/Chacter/sh_character_battle.c`, `src/Chacter/sh_character_status.c`
//! - PC executable `sh2pc.exe`:
//!   - `0x4a88e0`: `enExecTask` general task execution loop.
//!   - `0x4aa5e0`: `enInitData` enemy initialization and parameter setup.
//!   - `0x4a96c0`: `enDying` state handler (marks row killed in bitfield `0x1f7d9e0`).
//!   - `0x4c65d0`: Common damage calculation: AP against HP, SP against endurance, knockdown check.
//!   - `0x4c3e30`: Endurance recovery rates per second.

pub mod creeper;
pub mod lying_figure;
pub mod mannequin;
pub mod nurse;
pub mod pyramid_head;

use serde::{Deserialize, Serialize};

/// Species identifier matching table `0x88d408` and stage `Enemy_List`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum EnemySpecies {
    /// 0x200: Lying Figure (`chr/scu`).
    LyingFigure = 0x200,
    /// 0x201: Mannequin (`chr/mkn`).
    Mannequin = 0x201,
    /// 0x202: Creeper (`chr/tyu`).
    Creeper = 0x202,
    /// 0x207: Bubble Head Nurse (`chr/nse`).
    Nurse = 0x207,
    /// 0x208: Pyramid Head (`chr/red`).
    PyramidHead = 0x208,
}

/// Generic high-level behavior state for Silent Hill 2 creatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CreatureState {
    /// Idle, stationary or dormant pose.
    #[default]
    Idle,
    /// Wandering patrol around start origin.
    Wander,
    /// Alert / caution: facing threat, analyzing noise/light.
    Precaution,
    /// Actively pursuing player/companion.
    Chase,
    /// Performing an attack strike or projectile spray.
    Attack,
    /// Flinching from non-knockdown strike.
    Damage,
    /// Knocked down on the floor (prone or supine).
    Down,
    /// Dying / corpse state.
    Dying,
}

/// Dynamic combat and health statistics for an enemy creature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureStats {
    pub health: f32,
    pub max_health: f32,
    pub endurance: f32,
    pub max_endurance: f32,
    /// Rate at which endurance recovers per second (`0x4c3e30`).
    pub endurance_recovery_rate: f32,
    /// Downed duration counter (seconds remaining).
    pub down_timer: f32,
    /// Flinch stagger timer (seconds remaining).
    pub flinch_timer: f32,
}

impl CreatureStats {
    pub fn new(health: f32, endurance: f32, endurance_recovery_rate: f32) -> Self {
        Self {
            health,
            max_health: health,
            endurance,
            max_endurance: endurance,
            endurance_recovery_rate,
            down_timer: 0.0,
            flinch_timer: 0.0,
        }
    }

    /// Update recovery and stagger timers (`0x4c3e30`).
    pub fn update(&mut self, dt: f32) {
        if self.health <= 0.0 {
            return;
        }

        if self.flinch_timer > 0.0 {
            self.flinch_timer = (self.flinch_timer - dt).max(0.0);
        }

        if self.down_timer > 0.0 {
            self.down_timer = (self.down_timer - dt).max(0.0);
        } else {
            // Recover endurance over time when not downed
            if self.endurance < self.max_endurance {
                self.endurance =
                    (self.endurance + self.endurance_recovery_rate * dt).min(self.max_endurance);
            }
        }
    }

    /// Process incoming damage (AP) and stagger (SP) (`0x4c65d0`).
    /// Returns true if the blow knocks the creature down.
    pub fn take_damage(&mut self, ap: f32, sp: f32, random_roll: f32) -> bool {
        self.health = (self.health - ap).max(0.0);
        self.endurance = (self.endurance - sp).max(0.0);

        if self.health <= 0.0 {
            return true; // Knockdown / death
        }

        // PS2 knockdown formula: rand <= endurance * 0.002 + 0.8
        // If endurance reaches 0 or roll fails threshold, creature falls down
        let knockdown_threshold = self.endurance * 0.002 + 0.8;
        if self.endurance <= 0.0 || random_roll > knockdown_threshold {
            self.down_timer = 8.0; // Knocked down
            true
        } else {
            self.flinch_timer = 0.5; // Flinch
            false
        }
    }
}
