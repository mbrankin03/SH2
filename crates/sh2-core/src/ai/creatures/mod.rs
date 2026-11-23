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

pub mod abstract_daddy;
pub mod anim;
pub mod common;
pub mod creeper;
pub mod eddie;
pub mod flesh_lips;
pub mod insects;
pub mod lying_figure;
pub mod mandarin;
pub mod mannequin;
pub mod mary;
pub mod nurse;
pub mod pyramid_head;
pub mod twin_pyramid_head;

pub use common::{
    apply_stage_hp_multipliers, en_check_sleep_in, en_check_sleep_out, CreatureStats, EnemySize,
    StagePlace, TribeAlert, TribeCommunicationBus, EN_SLEEP_IN_DIST_MM, EN_SLEEP_OUT_DIST_MM,
    MAX_TRIBE_COMM_SLOTS,
};

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Horizontal XZ Euclidean distance in PS2 world mm.
#[inline]
pub fn xz_dist(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

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
    /// 0x203: Flesh Lips (`chr/ike`).
    FleshLips = 0x203,
    /// 0x204: Abstract Daddy (`chr/pap`).
    AbstractDaddy = 0x204,
    /// 0x205: Eddie Boss (`chr/edb`).
    Eddie = 0x205,
    /// 0x206: Mary / Maria Final Boss (`chr/bos`).
    MaryBoss = 0x206,
    /// 0x207: Bubble Head Nurse (`chr/nse`).
    Nurse = 0x207,
    /// 0x208: Pyramid Head (`chr/red`).
    PyramidHead = 0x208,
    /// 0x209: Twin Pyramid Heads (`chr/oni`).
    TwinPyramidHead = 0x209,
    /// 0x20a: Mandarin (`chr/arm`).
    Mandarin = 0x20a,
    /// 0x20f: Room Insects / Mary Moth Swarm (`en_insect.c`, type `0x0f`).
    RoomInsect = 0x20f,
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
