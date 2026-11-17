//! Secret NG+ Weapons: Chainsaw & Hyper Spray (`FlagInit`, `EvProgGetChainsaw`, `enCheckSpray`, `SprayEffect.cpp`).
//!
//! Verified 1:1 against `SLUS_202.28` (`EvProgGetChainsaw` `0x01f02c30`, `sh2_attack_list` `0x0038cb90`,
//! `enCheckSpray` `0x001425a0`, `enCheckDamage` `0x001424dc`, `enEDBCtrlDamage` `0x0014c010`,
//! `shGetHumanAttackSprayPos` `0x00121630`), `sh2pc.exe` (`FlagInit` `0x0048ab52..0x0048abc3`,
//! `sh2_attack_list` `0x00629000`, `EvProgGetChainsaw` `0x0059fb00`, spray timer/taper/ABGR/self-damage
//! `0x00533f51` & `0x00545694..0x00545a00`), and SH2:EE (`PreventChainsawSpawn.cpp`, `SprayEffect.cpp`).

use crate::combat::AttackSpec;

use super::playing_info::ClearEndKind;

/// Item ID: `Hyper Spray` (`12` in `ITEM_CATALOG`, `WeaponGrip::HyperSpray`).
pub const ITEM_HYPER_SPRAY: u8 = 12;
/// Item ID: `Chainsaw` (`16` in `ITEM_CATALOG`, `WeaponGrip::Chainsaw`).
pub const ITEM_CHAINSAW: u8 = 16;

/// Event flag: Unlocks `Dog key` spawn (`Flag 11` = `0x0800` in `0x01f7a9e0` at `0x0048ab96`).
pub const FLAG_SPAWN_DOG_KEY: usize = 11;
/// Event flag: Unlocks `Chainsaw` log spawn on Forest Trail (`Flag 12` = `0x1000` in `0x01f7a9e0` at `0x0048ab61`).
pub const FLAG_SPAWN_CHAINSAW: usize = 12;
/// Event flag: Set once `Chainsaw` is picked up from the Forest Trail log (`Stage 4 ev[20]` @ `0x008e7138`).
pub const FLAG_CHAINSAW_PICKED_UP: usize = 38;
/// Event flag: Unlocks `Hyper Spray` spawn in Saul Street Motorhome (`Flag 631` = `0x800000` in `0x01f7aa2c` at `0x0048abc3`).
pub const FLAG_SPAWN_HYPER_SPRAY: usize = 631;
/// Event flag: Set once `Hyper Spray` is picked up in the Motorhome (`Stage 2 ev[8]` @ `0x008e7d68`).
pub const FLAG_HYPER_SPRAY_PICKED_UP: usize = 632;
/// Event flag: Unlocks `Blue Gem` spawn in Observation Deck restroom (`Flag 723` = `0x80000` in `0x01f7aa38` at `0x0048ab89`).
pub const FLAG_SPAWN_BLUE_GEM: usize = 723;

/// Stage 4 room ID for the Forest Trail logs across from Silent Hill Ranch (`0x0156`, `Stage 4 ev[20]` @ `0x008e7138`).
pub const ROOM_FOREST_TRAIL_CHAINSAW: u16 = 0x0156;
/// Stage 2 room ID for the Saul Street Motorhome (`0x0072`, `Stage 2 ev[8]` @ `0x008e7d68`).
pub const ROOM_SAUL_STREET_MOTORHOME: u16 = 0x0072;

/// Sound effect ID played by `EvProgGetChainsaw` when picking up the Chainsaw (`0x2744` = `10052` at `0x0059fb11`).
pub const SE_CHAINSAW_PICKUP_CRANK: u32 = 0x2744;
/// Sound effect ID played when spraying the Hyper Spray (`0x2B27` = `11047` at `0x0054590e` & `0x0054a9ff`).
pub const SE_HYPER_SPRAY_HISS: u32 = 0x2B27;

/// Initial Hyper Spray pressurization timer (`20.0` s = `0x41a00000` written to `0x01fb80f0` at `0x00533f51`).
pub const HYPER_SPRAY_FULL_DURATION_SECS: f32 = 20.0;
/// Remaining Hyper Spray time below which cone range and hiss volume taper linearly (`10.0` s at `0x00638854`).
pub const HYPER_SPRAY_TAPER_THRESHOLD_SECS: f32 = 10.0;
/// Base minimum cone range of Hyper Spray (`-125.0` mm in `sh2_attack_list[8..=9]` at `SLUS_202.28 @ 0x0038ccbc` / `sh2pc.exe @ 0x0062912c`).
pub const HYPER_SPRAY_BASE_MIN_RANGE_MM: f32 = -125.0;
/// Base maximum cone range of Hyper Spray (`1000.0` mm in `sh2_attack_list[8..=9]` at `SLUS_202.28 @ 0x0038ccc0` / `sh2pc.exe @ 0x00629130`).
pub const HYPER_SPRAY_BASE_MAX_RANGE_MM: f32 = 1000.0;
/// Ray origin forward offset in `shGetHumanAttackSprayPos` (`900.0` mm = `0x44610000` at `0x001216ec`).
pub const HYPER_SPRAY_RAY_STEP_MM: f32 = 900.0;
/// Cone spread half-angle in `shGetHumanAttackSprayPos` (`~0.418879` rad = `24 deg` = `0x3ed67750` at `0x001217b0`).
pub const HYPER_SPRAY_CONE_HALF_ANGLE_RAD: f32 = 0.418_879_03;
/// Initial burst SFX volume for `SE_HYPER_SPRAY_HISS` (`0.7` = `0x3f333333` at `0x00545909`).
pub const HYPER_SPRAY_INITIAL_SFX_VOLUME: f32 = 0.7;
/// Sustained loop SFX volume above the `10.0` s taper threshold (`0.5` = `0x3f000000` at `0x0054a9d4`).
pub const HYPER_SPRAY_SUSTAIN_SFX_VOLUME: f32 = 0.5;

/// Chainsaw horizontal swing 1 (`sh2_attack_list[19]` at `SLUS_202.28 @ 0x0038ce3c` / `sh2pc.exe @ 0x006292ac`).
pub const ATK_CHAINSAW_SWING_1: AttackSpec = AttackSpec {
    id: 19,
    kind: 1,
    ap: 600.0,
    sp: 0.0,
    min_range: 0.0,
    max_range: 725.0,
    first_hit_frame: 10,
    last_hit_frame: 14,
    sfx_id: 0x2B26,
};

/// Chainsaw horizontal swing 2 (`sh2_attack_list[20]` at `SLUS_202.28 @ 0x0038ce60` / `sh2pc.exe @ 0x006292d0`).
pub const ATK_CHAINSAW_SWING_2: AttackSpec = AttackSpec {
    id: 20,
    kind: 1,
    ap: 600.0,
    sp: 0.0,
    min_range: 0.0,
    max_range: 725.0,
    first_hit_frame: 13,
    last_hit_frame: 17,
    sfx_id: 0x2B26,
};

/// Chainsaw revved overhead chop 1 (`sh2_attack_list[21]` at `SLUS_202.28 @ 0x0038ce84` / `sh2pc.exe @ 0x006292f4`).
pub const ATK_CHAINSAW_OVERHEAD_1: AttackSpec = AttackSpec {
    id: 21,
    kind: 1,
    ap: 1000.0,
    sp: 0.0,
    min_range: 0.0,
    max_range: 725.0,
    first_hit_frame: 17,
    last_hit_frame: 18,
    sfx_id: 0x2B26,
};

/// Chainsaw revved overhead chop 2 (`sh2_attack_list[22]` at `SLUS_202.28 @ 0x0038cea8` / `sh2pc.exe @ 0x00629318`).
pub const ATK_CHAINSAW_OVERHEAD_2: AttackSpec = AttackSpec {
    id: 22,
    kind: 1,
    ap: 1000.0,
    sp: 0.0,
    min_range: 0.0,
    max_range: 725.0,
    first_hit_frame: 18,
    last_hit_frame: 19,
    sfx_id: 0x2B26,
};

/// Chainsaw forward thrust (`sh2_attack_list[23]` at `SLUS_202.28 @ 0x0038cecc` / `sh2pc.exe @ 0x0062933c`).
pub const ATK_CHAINSAW_THRUST: AttackSpec = AttackSpec {
    id: 23,
    kind: 1,
    ap: 250.0,
    sp: 0.0,
    min_range: 0.0,
    max_range: 525.0,
    first_hit_frame: 12,
    last_hit_frame: 27,
    sfx_id: 0x2B26,
};

/// Complete 5-entry Chainsaw slice of `sh2_attack_list[19..=23]` (`SLUS_202.28 @ 0x0038cb90` / `sh2pc.exe @ 0x00629000`).
pub const CHAINSAW_ATTACK_SPECS: [AttackSpec; 5] = [
    ATK_CHAINSAW_SWING_1,
    ATK_CHAINSAW_SWING_2,
    ATK_CHAINSAW_OVERHEAD_1,
    ATK_CHAINSAW_OVERHEAD_2,
    ATK_CHAINSAW_THRUST,
];

/// Standing Hyper Spray attack spec (`sh2_attack_list[8]` at `SLUS_202.28 @ 0x0038ccb0` / `sh2pc.exe @ 0x00629120`).
pub const ATK_HYPER_SPRAY_STAND: AttackSpec = AttackSpec {
    id: 8,
    kind: 7,
    ap: 1.0,
    sp: 0.0,
    min_range: HYPER_SPRAY_BASE_MIN_RANGE_MM,
    max_range: HYPER_SPRAY_BASE_MAX_RANGE_MM,
    first_hit_frame: 8,
    last_hit_frame: 25,
    sfx_id: SE_HYPER_SPRAY_HISS,
};

/// Walking Hyper Spray attack spec (`sh2_attack_list[9]` at `SLUS_202.28 @ 0x0038ccd4` / `sh2pc.exe @ 0x00629144`).
pub const ATK_HYPER_SPRAY_WALK: AttackSpec = AttackSpec {
    id: 9,
    kind: 7,
    ap: 1.0,
    sp: 0.0,
    min_range: HYPER_SPRAY_BASE_MIN_RANGE_MM,
    max_range: HYPER_SPRAY_BASE_MAX_RANGE_MM,
    first_hit_frame: 2,
    last_hit_frame: 19,
    sfx_id: SE_HYPER_SPRAY_HISS,
};

/// Unlock evaluation output for NG+ bonus items in `FlagInit` (`0x0048ab52..0x0048abc3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NgPlusSecretSpawns {
    /// `Flag 12`: Chainsaw spawns embedded in Forest Trail log.
    pub spawn_chainsaw: bool,
    /// `Flag 631`: Hyper Spray spawns in Saul Street Motorhome.
    pub spawn_hyper_spray: bool,
    /// `Flag 11`: Dog Key spawns in doghouse lot near Rosewater Park / Jacks Inn.
    pub spawn_dog_key: bool,
    /// `Flag 723`: Blue Gem spawns in Observation Deck restroom.
    pub spawn_blue_gem: bool,
    /// `Flag 10`: Rebirth ritual items & NG+ memos spawn.
    pub spawn_ngplus_bonus_items: bool,
}

/// Evaluates NG+ bonus item spawn flags (`FlagInit` at `0x0048ab52..0x0048abc3` in `sh2pc.exe`).
///
/// - `battle_level`: `0` Beginner, `1` Easy, `2` Normal, `3` Hard (`0x01dbbff4`).
/// - `cleared_difficulty_mask`: `0x40` Action Normal cleared, `0x80` Action Hard cleared (`0x01dbbf82`).
/// - `prevent_first_playthrough_chainsaw`: `true` applies `PreventChainsawSpawn.cpp` (`0x0048ab6e`, strict PS2
///   NG+-only behavior); `false` preserves the vanilla PC Director's Cut bug (`0x0048ab68..0x0048ab76`) that
///   spawns the Chainsaw on a first playthrough when `battle_level < 2` (Beginner/Easy).
pub const fn evaluate_ngplus_secret_spawns(
    clear_end_kind: ClearEndKind,
    clear_count: u8,
    battle_level: u8,
    cleared_difficulty_mask: u8,
    prevent_first_playthrough_chainsaw: bool,
) -> NgPlusSecretSpawns {
    let base_count = clear_end_kind.base_endings_count();
    let is_ngplus = base_count > 0;

    let spawn_chainsaw = is_ngplus || (!prevent_first_playthrough_chainsaw && battle_level < 2);

    let spawn_blue_gem = is_ngplus && clear_end_kind.contains(ClearEndKind::BORN_FROM_A_WISH);
    let spawn_dog_key = base_count == 3 || clear_end_kind.contains(ClearEndKind::REBIRTH);
    let spawn_hyper_spray = (cleared_difficulty_mask & 0x80) != 0
        || ((cleared_difficulty_mask & 0x40) != 0 && clear_count >= 2)
        || clear_count >= 3;

    NgPlusSecretSpawns {
        spawn_chainsaw,
        spawn_hyper_spray,
        spawn_dog_key,
        spawn_blue_gem,
        spawn_ngplus_bonus_items: is_ngplus,
    }
}

/// Evaluates whether interacting with the Forest Trail log picks up the Chainsaw (`EvProgGetChainsaw` `0x0059fb00`).
pub const fn try_pickup_chainsaw(
    current_stage: u8,
    current_room: u16,
    flag_spawn_chainsaw: bool,
    flag_chainsaw_picked_up: bool,
) -> bool {
    current_stage == 4
        && current_room == ROOM_FOREST_TRAIL_CHAINSAW
        && flag_spawn_chainsaw
        && !flag_chainsaw_picked_up
}

/// Hyper Spray mist color tier (`0x00545896..0x005458c4` jump table `0x00545a14` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyperSprayColor {
    /// `spray_pow == -1` (`<= 2.0` stars on PC): Purple (`0xFF400040` ABGR = `[0x40, 0x00, 0x40, 0xFF]` RGBA).
    Purple,
    /// `spray_pow == 0` (`2.1..=7.9` stars): White (`0xFFC0C0C0` ABGR = `[0xC0, 0xC0, 0xC0, 0xFF]` RGBA).
    White,
    /// `spray_pow == 1` (`8.0..=9.9` stars): Yellow (`0xFF40C0C0` ABGR = `[0xC0, 0xC0, 0x40, 0xFF]` RGBA).
    Yellow,
    /// `spray_pow == 2` (`10.0` stars): Green (`0xFF40FF40` ABGR = `[0x40, 0xFF, 0x40, 0xFF]` RGBA).
    Green,
}

impl HyperSprayColor {
    /// Resolves mist color from `playing.spray_pow` (`-1..=2`, `0x00545896`).
    pub const fn from_spray_pow(spray_pow: i8) -> Self {
        match spray_pow {
            i8::MIN..=-1 => Self::Purple,
            0 => Self::White,
            1 => Self::Yellow,
            2..=i8::MAX => Self::Green,
        }
    }

    /// Packed 32-bit `0xAABBGGRR` color word written to `enEfctSetSpray` (`0x005458aa..0x005458c4`).
    pub const fn packed_abgr(self) -> u32 {
        match self {
            Self::Purple => 0xFF40_0040,
            Self::White => 0xFFC0_C0C0,
            Self::Yellow => 0xFF40_C0C0,
            Self::Green => 0xFF40_FF40,
        }
    }

    /// Unpacked `[R, G, B, A]` color bytes.
    pub const fn rgba(self) -> [u8; 4] {
        let v = self.packed_abgr();
        [
            (v & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            ((v >> 16) & 0xFF) as u8,
            ((v >> 24) & 0xFF) as u8,
        ]
    }
}

/// Enemy reaction to being hit by Hyper Spray (`enCheckDamage` `0x001424dc`, `enEDBCtrlDamage` `0x0014c010`,
/// `enPAPCtrlWander` `0x0015a4d4`, `enMKNCtrlConfuse` `0x00155cfc` in `SLUS_202.28`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SprayEnemyReaction {
    /// `Purple` (`spray_pow == -1`): Brief flinch only (`bltz` at `0x001424e4` skips full paralysis).
    BriefFlinch,
    /// `White` (`spray_pow == 0`): Zeroes immediate attack damage (`0x00142500`) and freezes/stuns enemy.
    FreezeStun,
    /// `Yellow` (`spray_pow == 1`): Freezes/confuses enemy and enables inter-enemy confusion (`0x00155cfc`).
    ConfuseAndFreeze,
    /// `Green` (`spray_pow >= 2`): Instant kill on regular creatures (`state = 8` death at `0x0014c034`)
    /// and heavy stagger/damage on bosses.
    InstantKillOrBossStagger {
        /// `true` if the target is killed outright (`!is_boss`), `false` for boss stagger.
        instant_kill: bool,
    },
}

/// Evaluates how an enemy reacts to Hyper Spray (`0x001424dc` & `0x0014c010` in `SLUS_202.28`).
pub const fn evaluate_spray_enemy_reaction(spray_pow: i8, is_boss: bool) -> SprayEnemyReaction {
    match HyperSprayColor::from_spray_pow(spray_pow) {
        HyperSprayColor::Purple => SprayEnemyReaction::BriefFlinch,
        HyperSprayColor::White => SprayEnemyReaction::FreezeStun,
        HyperSprayColor::Yellow => SprayEnemyReaction::ConfuseAndFreeze,
        HyperSprayColor::Green => SprayEnemyReaction::InstantKillOrBossStagger {
            instant_kill: !is_boss,
        },
    }
}

/// Per-tick output of [`HyperSprayCanState::tick_spray`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HyperSprayTickResult {
    /// Mist color tier emitted this tick.
    pub color: HyperSprayColor,
    /// Effective minimum cone range (`mm`, scaled when `remaining_secs < 10.0` at `0x005456ff`).
    pub effective_min_range_mm: f32,
    /// Effective maximum cone range (`mm`, scaled when `remaining_secs < 10.0` at `0x005456c6`).
    pub effective_max_range_mm: f32,
    /// Hiss SFX (`0x2B27`) volume (`0.7` on initial burst, `0.5` above `10.0` s, `0.05 * remaining_secs` below `10.0` s).
    pub sfx_volume: f32,
    /// HP self-damage inflicted on James (`dt` HP/s when `battle_level >= 2` at `0x00545937..0x005459b8`).
    pub james_self_damage: f32,
    /// `true` when the can empties (`remaining_secs <= 0.0` at `0x005459be`), interrupting spray into James coughing.
    pub triggered_james_cough: bool,
}

/// Stateful Hyper Spray canister & self-cough tracker (`0x01fb80f0` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HyperSprayCanState {
    /// Remaining pressurized spray duration in seconds (`20.0` initial at `0x00533f51`).
    pub remaining_secs: f32,
    /// Whether the current spray stream has already played its initial `0.7` burst (`0x15c(%ebp)` at `0x005458e5`).
    pub stream_started: bool,
}

impl Default for HyperSprayCanState {
    fn default() -> Self {
        Self::new()
    }
}

impl HyperSprayCanState {
    /// Creates a full `20.0` s Hyper Spray canister (`0x00533f51`).
    pub const fn new() -> Self {
        Self {
            remaining_secs: HYPER_SPRAY_FULL_DURATION_SECS,
            stream_started: false,
        }
    }

    /// Resets stream burst state when the player releases the attack button (`0x00545a01`).
    pub fn release_trigger(&mut self) {
        self.stream_started = false;
    }

    /// Refills canister pressure after completing the coughing / shake recovery (`0x00559210`).
    pub fn refill(&mut self) {
        self.remaining_secs = HYPER_SPRAY_FULL_DURATION_SECS;
        self.stream_started = false;
    }

    /// Advances an active Hyper Spray emission by `dt_secs` (`0x00545694..0x00545a00` & `0x0054a9c1`).
    ///
    /// Note: At `0x00545937`, `cmpb $0x2, 0x01dbbff4; jb skip` drains James's HP on Normal (`2`) and Hard (`3`)
    /// action levels (`battle_level >= 2`), clamping HP to `[0.0, hp_max]` and adding `dt_secs` to `jms_damage_total`.
    pub fn tick_spray(
        &mut self,
        dt_secs: f32,
        spray_pow: i8,
        battle_level: u8,
        james_hp: &mut f32,
        james_hp_max: f32,
    ) -> HyperSprayTickResult {
        let dt = dt_secs.max(0.0);
        let taper = if self.remaining_secs < HYPER_SPRAY_TAPER_THRESHOLD_SECS {
            (self.remaining_secs * 0.1).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let effective_min_range_mm = HYPER_SPRAY_BASE_MIN_RANGE_MM * taper;
        let effective_max_range_mm = HYPER_SPRAY_BASE_MAX_RANGE_MM * taper;

        let sfx_volume = if !self.stream_started {
            self.stream_started = true;
            HYPER_SPRAY_INITIAL_SFX_VOLUME
        } else if self.remaining_secs < HYPER_SPRAY_TAPER_THRESHOLD_SECS {
            (self.remaining_secs * 0.05).max(0.0)
        } else {
            HYPER_SPRAY_SUSTAIN_SFX_VOLUME
        };

        self.remaining_secs -= dt;
        let mut james_self_damage = 0.0;
        if battle_level >= 2 && dt > 0.0 {
            *james_hp = (*james_hp - dt).clamp(0.0, james_hp_max.max(0.0));
            james_self_damage = dt;
        }

        let triggered_james_cough = if self.remaining_secs <= 0.0 {
            self.remaining_secs = 0.0;
            self.stream_started = false;
            true
        } else {
            false
        };

        HyperSprayTickResult {
            color: HyperSprayColor::from_spray_pow(spray_pow),
            effective_min_range_mm,
            effective_max_range_mm,
            sfx_volume,
            james_self_damage,
            triggered_james_cough,
        }
    }
}

/// Particle velocity and spawn-cadence parameters from `SprayEffect.cpp` (`0x004a442c..0x004a509d`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SprayParticleTimingConfig {
    /// Speed multiplier for Hyper Spray particle states `0` and `1` (`0.3` at 30 FPS vs `0.15` at 60 FPS).
    pub speed_factor_state_0_1: f32,
    /// Speed multiplier for Hyper Spray particle state `2` (`0.5` at 30 FPS vs `0.25` at 60 FPS).
    pub speed_factor_state_2: f32,
    /// Vertical drift adjustment per frame (`100.0` at 30 FPS vs `50.0` at 60 FPS).
    pub y_adjust: f32,
    /// Frame stride between particle allocations (`1` = every frame vanilla 30 FPS; `2` = every other frame at 60 FPS).
    pub spawn_frame_stride: u32,
}

impl SprayParticleTimingConfig {
    /// Resolves particle parameters (`SprayEffect.cpp`), preserving 1:1 vanilla 30 FPS values when
    /// `enable_60fps_spray_fix` is `false`.
    pub const fn resolve(enable_60fps_spray_fix: bool) -> Self {
        if enable_60fps_spray_fix {
            Self {
                speed_factor_state_0_1: 0.15,
                speed_factor_state_2: 0.25,
                y_adjust: 50.0,
                spawn_frame_stride: 2,
            }
        } else {
            Self {
                speed_factor_state_0_1: 0.30,
                speed_factor_state_2: 0.50,
                y_adjust: 100.0,
                spawn_frame_stride: 1,
            }
        }
    }

    /// Returns `true` if a new Hyper Spray particle should spawn on `frame_index` (`IsSprayAllocActiveASM`).
    pub const fn should_spawn_particle(
        self,
        frame_index: u64,
        active_particle_count: usize,
    ) -> bool {
        if active_particle_count == 0 || self.spawn_frame_stride <= 1 {
            true
        } else {
            frame_index.is_multiple_of(self.spawn_frame_stride as u64)
        }
    }
}
