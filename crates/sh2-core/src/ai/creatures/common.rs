//! Shared `en_common.c` enemy runtime primitives (1:1 PS2/PC parity).
//!
//! # Ground Truth & Binary References
//! - **`SLUS_202.28` (`docs/decomp/slus/Enemy/en_common.c`):**
//!   - `enGetPlace` (`0x00141d20`): stage index -> place tier `0..=5`.
//!   - `enSetHP` (`0x001420c0`) / `sh2pc.exe` (`0x004a5fe0`): stage HP/Endurance multipliers.
//!   - `enReduceHP` (`0x00142200`) / `sh2pc.exe` (`0x004a6160`): AP reduces BOTH HP and Endurance.
//!   - `enAddHP` (`0x00142290`) / `sh2pc.exe` (`0x004a61d0`): heal + clamp endurance to HP.
//!   - `enAddEnduranceDT` (`0x00142310`) / `sh2pc.exe` (`0x004a6270`): `min(max_endurance, health)` cap.
//!   - `enCheckDamage` (`0x001423a0`): multi-hit division `damage / hit_count`, stomp/spray/no-damage gates.
//!   - `enSetHitBack` (`0x00142630`) + `enMoveExec` (`0x00143aa0`): `sp * 2.0` impulse, `* 0.5` decay.
//!   - `enSetSize` (`0x001426e0`) + `enSetHitColumn` (`0x001440c0`) + `EnBattleCollisionRate` (`0x002a9bf0`).
//!   - `enCheckSleepIn` (`0x00142950`, `15000.0` mm) & `enCheckSleepOut` (`0x001429f0`, `7500.0` mm).
//!   - `enSetCommunication` (`0x00146d80`) & `enCommunicateTribe` (`0x00146e40`): 8-slot tribe alert bus.

use super::EnemySpecies;
use crate::combat::attack_list::HyperSprayColor;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Outdoor sleep-in culling distance in PS2 world mm (`enCheckSleepIn`, `SLUS_202.28` `0x00142950`).
pub const EN_SLEEP_IN_DIST_MM: f32 = 15_000.0;

/// Outdoor sleep-out wake distance in PS2 world mm (`enCheckSleepOut`, `SLUS_202.28` `0x001429f0`).
pub const EN_SLEEP_OUT_DIST_MM: f32 = 7_500.0;

/// Maximum number of concurrent tribe alert messages (`enSetCommunication` `0x00146d80`, `0x005522a0`).
pub const MAX_TRIBE_COMM_SLOTS: usize = 8;

/// Stage progression tier returned by `enGetPlace` (`SLUS_202.28` `0x00141d20`).
///
/// Maps `stage + 0x24` (`9..=13`) to tiers `1..=5`; all other stages return `0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum StagePlace {
    /// Default / early stages (`enGetPlace() == 0`).
    #[default]
    Default = 0,
    /// Stage 9 (`enGetPlace() == 1`).
    Stage9 = 1,
    /// Stage 10 (`enGetPlace() == 2`): SCU/MKN/NSE HP × 1.5.
    Stage10 = 2,
    /// Stage 11 (`enGetPlace() == 3`): SCU/MKN/NSE HP × 1.5, Endurance × 1.2.
    Stage11 = 3,
    /// Stage 12 (`enGetPlace() == 4`): SCU/MKN/NSE HP × 2.0, Endurance × 1.5.
    Stage12 = 4,
    /// Stage 13 (`enGetPlace() == 5`): SCU/MKN/NSE HP × 2.0, Endurance × 1.5.
    Stage13 = 5,
}

impl StagePlace {
    /// Convert raw `stage + 0x24` stage ID to `StagePlace` (`enGetPlace` `0x00141d20`).
    pub const fn from_stage_id(stage_id: i32) -> Self {
        match stage_id {
            9 => Self::Stage9,
            10 => Self::Stage10,
            11 => Self::Stage11,
            12 => Self::Stage12,
            13 => Self::Stage13,
            _ => Self::Default,
        }
    }
}

/// Apply `enSetHP` (`SLUS_202.28` `0x001420c0` / `sh2pc.exe` `0x004a5fe0`) stage multipliers.
///
/// Only Lying Figure (`SCU`, type 1), Mannequin (`MKN`, type 2), and Bubble Head Nurse (`NSE`, type 6)
/// receive stage scaling:
/// - `Stage10` (`place == 2`): `(hp * 1.5, end)`
/// - `Stage11` (`place == 3`): `(hp * 1.5, end * 1.2)`
/// - `Stage12` / `Stage13` (`place == 4 | 5`): `(hp * 2.0, end * 1.5)`
///
/// Note: Retail PC `sh2pc.exe` (`0x004a5fe0`) additionally applies a small random variance to
/// SCU/MKN/NSE/ARM spawns via `thunk_FUN_00448770`; deterministic base values match both binaries.
pub fn apply_stage_hp_multipliers(
    species: EnemySpecies,
    base_hp: f32,
    base_endurance: f32,
    place: StagePlace,
) -> (f32, f32) {
    if !matches!(
        species,
        EnemySpecies::LyingFigure | EnemySpecies::Mannequin | EnemySpecies::Nurse
    ) {
        return (base_hp, base_endurance);
    }

    match place {
        StagePlace::Stage10 => (base_hp * 1.5, base_endurance),
        StagePlace::Stage11 => (base_hp * 1.5, base_endurance * 1.2),
        StagePlace::Stage12 | StagePlace::Stage13 => (base_hp * 2.0, base_endurance * 1.5),
        StagePlace::Default | StagePlace::Stage9 => (base_hp, base_endurance),
    }
}

/// Authentic collision/eye cylinder dimensions from `enSetSize` (`SLUS_202.28` `0x001426e0`).
///
/// All units are PS2 world millimeters (mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnemySize {
    /// Body collision radius (`work + 0x6c` / `0x70`).
    pub radius: f32,
    /// Cylinder height (`work + 0x74` / `0x78`).
    pub height: f32,
    /// Center Y offset from feet (`work + 0x7c` / `0x80`, written to `chara + 0xa8`).
    pub center_y: f32,
    /// Eye Y offset from feet (`work + 0x84` / `0x88`, written to `chara + 0xa4`).
    pub eye_y: f32,
}

impl EnemySize {
    /// Construct an `EnemySize` matching `enSetSize(radius, height, center_y, eye_y)`.
    pub const fn new(radius: f32, height: f32, center_y: f32, eye_y: f32) -> Self {
        Self {
            radius,
            height,
            center_y,
            eye_y,
        }
    }

    /// Standing / default `enSetSize` table extracted from `SLUS_202.28` `en*InitData`.
    pub const fn standing_for(species: EnemySpecies) -> Self {
        match species {
            // `enSCUInitData` 0x001603b0: 0x43160000, 0x44480000, 0x443b8000, 0x443b8000
            EnemySpecies::LyingFigure => Self::new(150.0, 800.0, 750.0, 750.0),
            // `enMKNInitData` 0x00153410: 0x43160000, 0x44480000, 0x443b8000, 0x443b8000
            EnemySpecies::Mannequin => Self::new(150.0, 800.0, 750.0, 750.0),
            // `enNSEInitData` 0x001577b0: 0x43160000, 0x44480000, 0x443b8000, 0x443b8000
            EnemySpecies::Nurse => Self::new(150.0, 800.0, 750.0, 750.0),
            // `enTYUInitData` 0x00165000: 0x42c80000, 0x43160000, 0x42960000, 0x42c80000
            EnemySpecies::Creeper => Self::new(100.0, 150.0, 75.0, 100.0),
            // `enREDInitData` 0x0015d410: 0x43160000, 0x44610000, 0x44480000, 0x44548000
            EnemySpecies::PyramidHead => Self::new(150.0, 900.0, 800.0, 850.0),
            // `enONIInitData` 0x00228b00: 0x437a0000, 0x44610000, 0x44480000, 0x44548000
            EnemySpecies::TwinPyramidHead => Self::new(250.0, 900.0, 800.0, 850.0),
            // `enPAPInitData` 0x0015a060: 0x43c80000, 0x44228000, 0x44160000, 0x44228000
            EnemySpecies::AbstractDaddy => Self::new(400.0, 650.0, 600.0, 650.0),
            // `enIKEInitData` 0x00150b50: 0x43160000, 0x44228000, 0x43fa0000, 0x44160000
            EnemySpecies::FleshLips => Self::new(150.0, 650.0, 500.0, 600.0),
            // `enARMInitData` 0x002396b0: 0x43960000, 0x43fa0000, 0x44160000, 0x44160000
            EnemySpecies::Mandarin => Self::new(300.0, 500.0, 600.0, 600.0),
            // `enEDBInitData` 0x001494c0: 0x43480000, 0x44548000, 0x443b8000, 0x443b8000
            EnemySpecies::Eddie => Self::new(200.0, 850.0, 750.0, 750.0),
            // `enBOSInitData` 0x0013fa10: 0x437a0000, 0x44480000, 0x42c80000, 0xc3160000
            EnemySpecies::MaryBoss => Self::new(250.0, 800.0, 100.0, -150.0),
            // `enINSInitData`: small moth/insect collision cylinder
            EnemySpecies::RoomInsect => Self::new(100.0, 150.0, 75.0, 100.0),
        }
    }

    /// Downed / prone `enSetNewSize` table extracted from `SLUS_202.28`.
    pub const fn downed_for(species: EnemySpecies) -> Self {
        match species {
            // `en_scu.c` 0x001603b0: 0x437a0000, 0x43480000, 0x42c80000, 0x43480000
            EnemySpecies::LyingFigure => Self::new(250.0, 200.0, 100.0, 200.0),
            // `en_mkn.c` 0x00153410: 0x437a0000, 0x43480000, 0x42c80000, 0x43480000
            EnemySpecies::Mannequin => Self::new(250.0, 200.0, 100.0, 200.0),
            // `en_nse.c` 0x001577b0: 0x43480000, 0x43480000, 0x42c80000, 0x43480000
            EnemySpecies::Nurse => Self::new(200.0, 200.0, 100.0, 200.0),
            // `en_pap.c` 0x0015a060: 0x43960000, 0x437a0000, 0x42fa0000, 0x42fa0000
            EnemySpecies::AbstractDaddy => Self::new(300.0, 250.0, 125.0, 125.0),
            // `en_bos.c` 0x0013fa10: 0x43af0000, 0x437a0000, 0x42fa0000, 0x437a0000
            EnemySpecies::MaryBoss => Self::new(350.0, 250.0, 125.0, 250.0),
            _ => Self::standing_for(species),
        }
    }

    /// `EnBattleCollisionRate` (`SLUS_202.28` `0x002a9bf0`) multiplier applied to `radius`
    /// when registering weapon hit columns (`enSetHitColumn` `0x001440c0` line 2313).
    pub const fn battle_collision_rate(species: EnemySpecies) -> f32 {
        match species {
            EnemySpecies::Creeper => 2.0,
            EnemySpecies::Mandarin => 1.0,
            EnemySpecies::RoomInsect => 1.0,
            _ => 0.8,
        }
    }

    /// Effective weapon-hit cylinder radius (`enSetHitColumn` `0x001440c0` lines 2313–2316).
    ///
    /// Equals `radius * EnBattleCollisionRate[species] * (1.5 if downed else 1.0)`.
    pub fn battle_radius(self, species: EnemySpecies, is_downed: bool) -> f32 {
        let mut r = self.radius * Self::battle_collision_rate(species);
        if is_downed {
            r *= 1.5;
        }
        r
    }
}

/// Evaluate `enCheckSleepIn` (`SLUS_202.28` `0x00142950` / `sh2pc.exe` `0x004a6510`).
///
/// Returns `true` when a living enemy in an outdoor stage exceeds `15000.0` mm from the player
/// (or is not marked active in the room).
pub fn en_check_sleep_in(
    dist_to_player_mm: f32,
    is_outdoor: bool,
    is_dead: bool,
    is_active_in_room: bool,
) -> bool {
    if is_dead {
        return false;
    }
    if is_active_in_room && (!is_outdoor || dist_to_player_mm <= EN_SLEEP_IN_DIST_MM) {
        return false;
    }
    true
}

/// Evaluate `enCheckSleepOut` (`SLUS_202.28` `0x001429f0`).
///
/// Returns `true` when a sleeping enemy should wake up: active in room and either indoor
/// or within `7500.0` mm (`< 7500.0`) of the player.
pub fn en_check_sleep_out(
    dist_to_player_mm: f32,
    is_outdoor: bool,
    is_active_in_room: bool,
) -> bool {
    if !is_active_in_room {
        return false;
    }
    if is_outdoor && dist_to_player_mm >= EN_SLEEP_OUT_DIST_MM {
        return false;
    }
    true
}

/// Dynamic combat and health statistics for an enemy creature (`en_common.c`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureStats {
    /// Current HP (`chara + 0x164` / `sh2pc +0x13c`).
    pub health: f32,
    /// Maximum HP (`chara + 0x168` / `sh2pc +0x140`).
    pub max_health: f32,
    /// Cached HP percentage `0.0..=100.0` (`chara + 0x16c` / `sh2pc +0x144`), used for walk speed scaling.
    pub hp_rate: f32,
    /// Current stagger endurance (`work + 0x40` / `sh2pc +0x44`).
    pub endurance: f32,
    /// Maximum stagger endurance (`work + 0x44` / `sh2pc +0x48`).
    pub max_endurance: f32,
    /// Rate at which endurance recovers per second (`enAddEnduranceDT` `0x00142310`).
    pub endurance_recovery_rate: f32,
    /// Downed duration counter (seconds remaining).
    pub down_timer: f32,
    /// Flinch stagger timer (seconds remaining).
    pub flinch_timer: f32,
    /// Consecutive-frame hit counter for multi-hit damage division (`work + 0x08` in `enCheckDamage` `0x001423a0`).
    pub multi_hit_count: u8,
    /// Last attack ID that damaged this enemy (`work + 0x09` in `enCheckDamage` `0x001423a0`).
    pub last_damage_attack_id: u16,
    /// Invulnerability / no-damage flag (`chara + 0x170 & 0x40`, `enFlagSetNoDamage` `0x00142fb0`).
    pub no_damage: bool,
    /// Normalized XZ hitback impulse direction (`work + 0x48`, `work + 0x4c` in `enSetHitBack` `0x00142630`).
    pub hitback_dir: [f32; 2],
    /// Remaining hitback impulse speed in mm (`work + 0x50` in `enSetHitBack` `0x00142630`, `sp * 2.0`).
    pub hitback_speed: f32,
}

impl CreatureStats {
    /// Create a new `CreatureStats` (`enSetHP` `0x001420c0`).
    pub fn new(health: f32, endurance: f32, endurance_recovery_rate: f32) -> Self {
        Self {
            health,
            max_health: health,
            hp_rate: if health > 0.0 { 100.0 } else { 0.0 },
            endurance,
            max_endurance: endurance,
            endurance_recovery_rate,
            down_timer: 0.0,
            flinch_timer: 0.0,
            multi_hit_count: 0,
            last_damage_attack_id: 0,
            no_damage: false,
            hitback_dir: [0.0, 0.0],
            hitback_speed: 0.0,
        }
    }

    /// Create a new `CreatureStats` applying `enSetHP` (`0x001420c0`) stage multipliers for `species`.
    pub fn new_for_stage(
        species: EnemySpecies,
        base_health: f32,
        base_endurance: f32,
        endurance_recovery_rate: f32,
        place: StagePlace,
    ) -> Self {
        let (hp, end) = apply_stage_hp_multipliers(species, base_health, base_endurance, place);
        Self::new(hp, end, endurance_recovery_rate)
    }

    /// Recompute `hp_rate = (health / max_health) * 100.0` (`enReduceHP` `0x00142200`).
    #[inline]
    fn refresh_hp_rate(&mut self) {
        self.hp_rate = if self.max_health > 0.0 {
            (self.health / self.max_health) * 100.0
        } else {
            0.0
        };
    }

    /// Recover endurance by `rate * dt`, clamped to `min(max_endurance, health)`
    /// (`enAddEnduranceDT`, `SLUS_202.28` `0x00142310` / `sh2pc.exe` `0x004a6270`).
    pub fn add_endurance_dt(&mut self, rate: f32, dt: f32) {
        let next = (self.endurance + rate * dt)
            .min(self.max_endurance)
            .min(self.health);
        self.endurance = next.max(0.0);
    }

    /// Heal enemy HP and reset endurance to `min(max_endurance, health)`
    /// (`enAddHP`, `SLUS_202.28` `0x00142290` / `sh2pc.exe` `0x004a61d0`).
    pub fn add_hp(&mut self, delta: f32) -> f32 {
        self.health = (self.health + delta).min(self.max_health).max(0.0);
        self.endurance = self.max_endurance.min(self.health);
        self.refresh_hp_rate();
        self.health
    }

    /// Update recovery and stagger timers (`enAddEnduranceDT` `0x00142310`).
    pub fn update(&mut self, dt: f32) {
        if self.health <= 0.0 {
            return;
        }

        if self.flinch_timer > 0.0 {
            self.flinch_timer = (self.flinch_timer - dt).max(0.0);
        }

        if self.down_timer > 0.0 {
            self.down_timer = (self.down_timer - dt).max(0.0);
        } else if self.endurance_recovery_rate > 0.0 {
            self.add_endurance_dt(self.endurance_recovery_rate, dt);
        }
    }

    /// Reset consecutive-frame multi-hit counter when a frame has no incoming damage
    /// (`enCheckDamage` `SLUS_202.28` `0x001423a0` line 806).
    #[inline]
    pub fn reset_multi_hit_if_idle(&mut self) {
        self.multi_hit_count = 0;
    }

    /// Validate and scale incoming attack damage per `enCheckDamage` (`SLUS_202.28` `0x001423a0`).
    ///
    /// - Returns `None` if `raw_ap <= 0.0`, `no_damage` is set, a stomp/kick (`31..=40` retail)
    ///   targets a standing enemy, or White/Yellow Hyper Spray (`8 | 9`) hits a boss.
    /// - Otherwise increments `multi_hit_count` (`work + 0x08`) and returns
    ///   `Some(raw_ap / multi_hit_count as f32)` so multi-frame weapons (Chainsaw, Great Knife)
    ///   diminish per consecutive hit frame exactly as on PS2/PC.
    pub fn check_damage(
        &mut self,
        species: EnemySpecies,
        attack_id: u16,
        raw_ap: f32,
        is_downed: bool,
        spray_color: Option<HyperSprayColor>,
    ) -> Option<f32> {
        if raw_ap <= 0.0 {
            self.multi_hit_count = 0;
            return None;
        }
        if self.no_damage {
            return None;
        }

        // Retail PC `sh2_attack_list` finish kicks/stomps are IDs 31..=40 (`0x1f..=0x28`,
        // shifted +6 from prototype `0x19..=0x22`, `sh2pc.exe` `0x004a6330`).
        let is_finish_attack = (31..=40).contains(&attack_id);
        if is_finish_attack && !is_downed {
            return None;
        }

        // Friendly-fire species immunities (`enCheckDamage` `SLUS_202.28` `0x001423a0` lines 812–813 /
        // `sh2pc.exe` `0x004a92ee` lines 6915–6916):
        // - Lying Figure (`type 1`, SCU) is immune to acid spray (`0x24` proto / `0x2a` = `42` retail).
        // - Creeper (`type 3`, TYU) is immune to creeper bite (`0x33` proto / `0x39` = `57` retail).
        if (species == EnemySpecies::LyingFigure && attack_id == 42)
            || (species == EnemySpecies::Creeper && attack_id == 57)
        {
            return None;
        }

        // Hyper Spray (`8 | 9`) vs bosses: White (0) and Yellow (1) have no effect
        // (`enCheckDamage` `0x001423a0` lines 814–822).
        if matches!(attack_id, 8 | 9) {
            let is_regular = matches!(
                species,
                EnemySpecies::LyingFigure
                    | EnemySpecies::Mannequin
                    | EnemySpecies::Nurse
                    | EnemySpecies::Creeper
                    | EnemySpecies::RoomInsect
                    | EnemySpecies::AbstractDaddy
            );
            if !is_regular {
                if let Some(color) = spray_color {
                    if !color.affects_bosses() {
                        return None;
                    }
                }
            }
        }

        self.multi_hit_count = self.multi_hit_count.saturating_add(1).max(1);
        self.last_damage_attack_id = attack_id;
        Some(raw_ap / f32::from(self.multi_hit_count))
    }

    /// Apply validated damage to BOTH HP and Endurance (`enReduceHP` `SLUS_202.28` `0x00142200` /
    /// `sh2pc.exe` `0x004a6160`).
    ///
    /// Returns remaining endurance.
    pub fn reduce_hp(&mut self, damage: f32) -> f32 {
        self.health = (self.health - damage).max(0.0);
        self.endurance = (self.endurance - damage).max(0.0);
        self.refresh_hp_rate();
        self.endurance
    }

    /// Initialize knockback impulse from attack `sp` (`sh2_attack_list +0x08`) and attack vector
    /// (`enSetHitBack`, `SLUS_202.28` `0x00142630`).
    ///
    /// Normalizes `(attack_dir.x, attack_dir.z)` and sets `hitback_speed = sp * 2.0`.
    pub fn set_hit_back(&mut self, attack_dir_xz: [f32; 2], sp: f32) {
        let len =
            (attack_dir_xz[0] * attack_dir_xz[0] + attack_dir_xz[1] * attack_dir_xz[1]).sqrt();
        if len > 1e-4 {
            self.hitback_dir = [attack_dir_xz[0] / len, attack_dir_xz[1] / len];
        } else {
            self.hitback_dir = [0.0, 0.0];
        }
        self.hitback_speed = (sp * 2.0).max(0.0);
    }

    /// Advance hitback impulse by one frame and return XZ world displacement in mm
    /// (`enMoveExec`, `SLUS_202.28` `0x00143aa0` lines 2113–2140).
    ///
    /// Each tick: `hitback_speed *= 0.5`, clamped to `(body_radius - 50.0).max(0.0)`,
    /// and zeroed once `hitback_speed < 1.0`.
    pub fn step_hit_back(&mut self, body_radius_mm: f32) -> Vec3 {
        if self.hitback_speed <= 0.0 {
            return Vec3::ZERO;
        }
        self.hitback_speed *= 0.5;
        let max_step = (body_radius_mm - 50.0).max(0.0);
        let step = self.hitback_speed.min(max_step);
        let disp = Vec3::new(self.hitback_dir[0] * step, 0.0, self.hitback_dir[1] * step);
        if self.hitback_speed < 1.0 {
            self.hitback_speed = 0.0;
        }
        disp
    }

    /// Process incoming attack (`ap` reduces BOTH HP & Endurance per `enReduceHP` `0x00142200`;
    /// `sp` drives hitback `sp * 2.0` per `enSetHitBack` `0x00142630`).
    ///
    /// Returns `true` if the blow knocks the creature down or kills it.
    pub fn take_damage(&mut self, ap: f32, sp: f32, random_roll: f32) -> bool {
        self.take_damage_directed(ap, sp, [0.0, 0.0], random_roll)
    }

    /// Directed variant of [`Self::take_damage`] recording knockback direction for `enSetHitBack`.
    pub fn take_damage_directed(
        &mut self,
        ap: f32,
        sp: f32,
        attack_dir_xz: [f32; 2],
        random_roll: f32,
    ) -> bool {
        self.reduce_hp(ap);
        if sp > 0.0 {
            self.set_hit_back(attack_dir_xz, sp);
        }

        if self.health <= 0.0 {
            return true;
        }

        // PS2 knockdown check: endurance == 0 or roll exceeds endurance * 0.002 + 0.8
        let knockdown_threshold = self.endurance * 0.002 + 0.8;
        if self.endurance <= 0.0 || random_roll > knockdown_threshold {
            self.down_timer = 8.0;
            true
        } else {
            self.flinch_timer = 0.5;
            false
        }
    }
}

/// Single tribe alert slot (`0x005522a0`, 32 bytes per slot in `enSetCommunication` `0x00146d80`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TribeAlert {
    /// World position of the alert source (`+0x00..+0x0c`).
    pub position: Vec3,
    /// Tribe / species group identifier (`+0x10`, non-zero = active).
    pub tribe_id: u32,
    /// Alert message kind (`+0x14`).
    pub message_type: u32,
    /// Remaining lifetime in ticks (`+0x18`, decremented by 1 each `enExecTask`).
    pub ttl_ticks: i32,
    /// Alert broadcast radius in mm (`+0x1c`).
    pub radius_mm: f32,
}

/// 8-slot tribe alert communication bus (`enSetCommunication` `0x00146d80` &
/// `enCommunicateTribe` `0x00146e40` in `en_common.c`).
#[derive(Debug, Clone, Default)]
pub struct TribeCommunicationBus {
    slots: [Option<TribeAlert>; MAX_TRIBE_COMM_SLOTS],
}

impl TribeCommunicationBus {
    /// Post an alert to the first free slot (`enSetCommunication` `0x00146d80`).
    ///
    /// Returns `true` if a slot was allocated, or `false` if all 8 slots are occupied.
    pub fn post(
        &mut self,
        radius_mm: f32,
        tribe_id: u32,
        message_type: u32,
        position: Vec3,
        ttl_ticks: i32,
    ) -> bool {
        if tribe_id == 0 {
            return false;
        }
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(TribeAlert {
                    position,
                    tribe_id,
                    message_type,
                    ttl_ticks,
                    radius_mm,
                });
                return true;
            }
        }
        false
    }

    /// Find the closest active alert matching `tribe_id` within its `radius_mm`
    /// (`enCommunicateTribe` `0x00146e40`).
    pub fn query_nearest(&self, tribe_id: u32, listener_pos: Vec3) -> Option<TribeAlert> {
        let mut best: Option<(f32, TribeAlert)> = None;
        for alert in self.slots.iter().flatten() {
            if alert.tribe_id != tribe_id {
                continue;
            }
            let dist = (listener_pos - alert.position).length();
            if dist < alert.radius_mm && best.is_none_or(|(best_d, _)| dist < best_d) {
                best = Some((dist, *alert));
            }
        }
        best.map(|(_, alert)| alert)
    }

    /// Decrement active slot TTLs by 1 tick and expire slots `<= 0`
    /// (`enExecTask` `0x001413f0` lines 272–282).
    pub fn tick(&mut self) {
        for slot in &mut self.slots {
            if let Some(alert) = slot {
                if alert.ttl_ticks > 0 {
                    alert.ttl_ticks -= 1;
                    if alert.ttl_ticks < 1 {
                        *slot = None;
                    }
                }
            }
        }
    }

    /// Clear all tribe alert slots.
    pub fn clear(&mut self) {
        self.slots = [None; MAX_TRIBE_COMM_SLOTS];
    }

    /// Number of currently active tribe alert slots.
    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}
