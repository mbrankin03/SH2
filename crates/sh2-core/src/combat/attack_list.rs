//! Complete 72-entry Retail PC `sh2_attack_list`, weapon grips, Hyper Spray tiers, and SFX IDs.
//!
//! # Ground Truth & Binary References
//! - **`sh2pc.exe` `0x00629000` / `0x006ec380` (`sh2_attack_list[72]`, 36 bytes / `0x24` per entry):**
//!   Exact retail PC Director's Cut attack table (`id`, `kind`, `ap`, `sp`, `min_range`, `max_range`,
//!   `first_hit_frame`, `last_hit_frame`).
//! - **Prototype (`SLUS_202.28`) vs. Retail PC (`sh2pc.exe`) Drift:**
//!   - Retail PC inserts Born From a Wish Maria weapons at indices `25..=30` (Maria Revolver `25..=26`,
//!     reload `27`, Maria Chinese Cleaver `28..=30`), shifting Kicks (`25..=29 -> 31..=35`), Stomps
//!     (`30..=34 -> 36..=40`), and all enemy attack IDs (`36..=65 -> 42..=71`) by **+6**.
//!   - `SLUS_202.28` `shBattleAttackByHumanGunshotTypeA` (`0x00120cf0`) plays `0x2b14` (`11028`) for
//!     Handgun (`IDs 1..=2`) and `0x2b28` (`11048`) for Hunting Rifle (`ID 6`); `sh2pc.exe` (`0x00543b27`)
//!     also plays `0x2b14` (`11028`) for Maria's Revolver (`IDs 25..=26`). Note: `sh2pc.exe`'s jump table
//!     at `0x00543b12` mapped Handgun `1..=2` to `0x2b28` (a known PC port regression); we restore the
//!     authentic PS2 Handgun SFX `0x2b14` (`11028`) per `SLUS_202.28` `0x00120cf0`.
//! - **`PlayerRequestAttack` (`SLUS_202.28` `0x00110530` / `sh2pc.exe` `0x005343a0`):**
//!   Per-gun dry-fire SFX IDs: Handgun / Maria Revolver `0x2b16` (`11030`), Shotgun `0x2b2c` (`11052`),
//!   Hunting Rifle `0x2b29` (`11049`).
//! - **`GameSaveSprayPower` (`SLUS_202.28` `0x00290c20`) & `shBattleAttackByHumanFog` (`0x00121a40`):**
//!   Hyper Spray color tiers (`DAT_0122b319`) and ABGR particle colors.

use serde::{Deserialize, Serialize};

/// Handgun & Maria Revolver gunshot SFX (`0x2b14` = `11028`, `SLUS_202.28` `0x00120cf0` / `sh2pc.exe` `0x00543b27`).
pub const SE_HANDGUN_SHOT: u32 = 11028;
/// Handgun & Maria Revolver dry-fire click SFX (`0x2b16` = `11030`, `PlayerRequestAttack` `0x00110530` / `0x005343a0`).
pub const SE_HANDGUN_DRY_FIRE: u32 = 11030;
/// Shotgun gunshot SFX (`0x2b2b` = `11051`, `shBattleAttackByHumanGunshotTypeB` `0x00120f30` / `0x00543bc0`).
pub const SE_SHOTGUN_SHOT: u32 = 11051;
/// Shotgun dry-fire click SFX (`0x2b2c` = `11052`, `PlayerRequestAttack` `0x00110530` / `0x005343a0`).
pub const SE_SHOTGUN_DRY_FIRE: u32 = 11052;
/// Hunting Rifle gunshot SFX (`0x2b28` = `11048`, `shBattleAttackByHumanGunshotTypeA` `0x00120cf0` / `0x00543b20`).
pub const SE_RIFLE_SHOT: u32 = 11048;
/// Hunting Rifle dry-fire click SFX (`0x2b29` = `11049`, `PlayerRequestAttack` `0x00110530` / `0x005343a0`).
pub const SE_RIFLE_DRY_FIRE: u32 = 11049;
/// Hyper Spray hiss SFX (`0x2b27` = `11047`, `shBattleAttackByHumanFog` `0x00121a40` / `sh2pc.exe` `0x0054590e`).
pub const SE_HYPER_SPRAY_SHOT: u32 = 11047;
/// Chainsaw attack SFX (`0x2b26` = `11046`).
pub const SE_CHAINSAW_ATTACK: u32 = 11046;
/// Wooden Plank impact/swing SFX (`11032`).
pub const SE_PLANK_SWING: u32 = 11032;
/// Steel Pipe / Kick / Stomp SFX (`11040`).
pub const SE_PIPE_OR_FINISH: u32 = 11040;
/// Melee air swish SFX (`11027`).
pub const SE_MELEE_SWISH: u32 = 11027;
/// Firearm reload click SFX (`11029`).
pub const SE_FIREARM_RELOAD: u32 = 11029;

/// Weapon grip classification matching PS2/PC `gWeaponHandGrip` (`sh2pc.exe` `0x0088d2f0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum WeaponGrip {
    #[default]
    None = 0,
    Handgun = 1,
    Shotgun = 2,
    HuntingRifle = 3,
    HyperSpray = 4,
    WoodenPlank = 5,
    SteelPipe = 6,
    Chainsaw = 7,
    GreatKnife = 8,
    MariaRevolver = 9,
    MariaChineseCleaver = 10,
}

impl WeaponGrip {
    /// Return the corresponding authentic item ID (`sh2pc.exe` `0x005345f0` / `PlayerNowItemName`).
    pub const fn item_id(&self) -> Option<u32> {
        match self {
            Self::None => None,
            Self::Handgun => Some(4),
            Self::Shotgun => Some(6),
            Self::HuntingRifle => Some(8),
            Self::HyperSpray => Some(12),
            Self::WoodenPlank => Some(13),
            Self::SteelPipe => Some(14),
            Self::GreatKnife => Some(15),
            Self::Chainsaw => Some(16),
            Self::MariaRevolver => Some(10),
            Self::MariaChineseCleaver => Some(17),
        }
    }

    /// Return the corresponding authentic ammo item ID if firearm.
    pub const fn ammo_item_id(&self) -> Option<u32> {
        match self {
            Self::Handgun => Some(5),        // Handgun bullets
            Self::Shotgun => Some(7),        // Shotgun shells
            Self::HuntingRifle => Some(9),   // Rifle shells
            Self::MariaRevolver => Some(11), // Revolver bullets
            _ => None,
        }
    }

    /// Magazine / cylinder capacity.
    pub const fn magazine_capacity(&self) -> u32 {
        match self {
            Self::Handgun => 10,
            Self::Shotgun => 6,
            Self::HuntingRifle => 4,
            Self::MariaRevolver => 10,
            _ => 0,
        }
    }

    /// Per-gun dry-fire SFX ID (`PlayerRequestAttack`, `SLUS_202.28` `0x00110530` / `sh2pc.exe` `0x005343a0`).
    pub const fn dry_fire_sfx(&self) -> Option<u32> {
        match self {
            Self::Handgun | Self::MariaRevolver => Some(SE_HANDGUN_DRY_FIRE),
            Self::Shotgun => Some(SE_SHOTGUN_DRY_FIRE),
            Self::HuntingRifle => Some(SE_RIFLE_DRY_FIRE),
            _ => None,
        }
    }

    /// Whether this weapon is a firearm.
    pub const fn is_firearm(&self) -> bool {
        matches!(
            self,
            Self::Handgun | Self::Shotgun | Self::HuntingRifle | Self::MariaRevolver
        )
    }

    /// Whether this weapon is a melee weapon.
    pub const fn is_melee(&self) -> bool {
        matches!(
            self,
            Self::WoodenPlank
                | Self::SteelPipe
                | Self::Chainsaw
                | Self::GreatKnife
                | Self::MariaChineseCleaver
        )
    }

    /// Default kick/stomp variation index (`0x53b190`).
    /// 0: bare hands / handgun / spray / maria weapons
    /// 1: long guns (shotgun, rifle)
    /// 2: plank and pipe
    /// 3: great knife
    /// 4: chainsaw
    pub const fn kick_type(&self) -> usize {
        match self {
            Self::None
            | Self::Handgun
            | Self::HyperSpray
            | Self::MariaRevolver
            | Self::MariaChineseCleaver => 0,
            Self::Shotgun | Self::HuntingRifle => 1,
            Self::WoodenPlank | Self::SteelPipe => 2,
            Self::GreatKnife => 3,
            Self::Chainsaw => 4,
        }
    }

    /// Right-hand skeletal pose variation index (`3..=13`) from `PlayerCheckModelParts`
    /// (`SLUS_202.28` `0x001098d0`, `_DAT_003c83bc` passed to `sh2gfw_Set_JMSequip`).
    ///
    /// - `3`: Empty open right hand (`None`)
    /// - `7`: Handgun / Maria Revolver
    /// - `8`: Shotgun / Hunting Rifle
    /// - `9`: Steel Pipe
    /// - `10`: Wooden Plank / Maria Chinese Cleaver
    /// - `11`: Hyper Spray
    /// - `12`: Great Knife
    /// - `13`: Chainsaw
    #[must_use]
    pub const fn right_hand_pose_index(&self) -> u8 {
        match self {
            Self::None => 3,
            Self::Handgun | Self::MariaRevolver => 7,
            Self::Shotgun | Self::HuntingRifle => 8,
            Self::SteelPipe => 9,
            Self::WoodenPlank | Self::MariaChineseCleaver => 10,
            Self::HyperSpray => 11,
            Self::GreatKnife => 12,
            Self::Chainsaw => 13,
        }
    }
}

/// Hyper Spray color/power tier (`DAT_0122b319` in `GameSaveSprayPower` `SLUS_202.28` `0x00290c20`).
///
/// Controls both particle tint (`shBattleAttackByHumanFog` `0x00121a40`) and enemy stun/kill
/// eligibility (`enCheckDamage` `SLUS_202.28` `0x001424dc` lines 814–822).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i8)]
pub enum HyperSprayColor {
    /// Rank `< 6` (`-1` / `0xff`): Purple spray (`0xff400040`), lowest stun power.
    Purple = -1,
    /// Rank `6..80` (`0`): White spray (`0xffc0c0c0`), standard stun.
    #[default]
    White = 0,
    /// Rank `80..100` (`1`): Yellow spray (`0xff40c0c0`), extended stun.
    Yellow = 1,
    /// Rank `>= 100` (`2`): Green spray (`0xff40ff40`), lethal to regular enemies and stuns bosses.
    Green = 2,
}

impl HyperSprayColor {
    /// Derive Hyper Spray tier from total ranking score (`GameSaveSprayPower`, `SLUS_202.28` `0x00290c20`).
    pub const fn from_rank_score(rank_total: u32) -> Self {
        if rank_total < 6 {
            Self::Purple
        } else if rank_total < 80 {
            Self::White
        } else if rank_total < 100 {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    /// Authentic PS2/PC ABGR particle color (`shBattleAttackByHumanFog`, `SLUS_202.28` `0x00121a40`).
    pub const fn particle_abgr(&self) -> u32 {
        match self {
            Self::Purple => 0xff40_0040,
            Self::White => 0xffc0_c0c0,
            Self::Yellow => 0xff40_c0c0,
            Self::Green => 0xff40_ff40,
        }
    }

    /// Whether this spray color bypasses the boss/special-enemy filter in `enCheckDamage`
    /// (`SLUS_202.28` `0x001424dc` line 818: `enGetSprayPower() >= 2` or `< 0`).
    pub const fn affects_bosses(&self) -> bool {
        matches!(self, Self::Green | Self::Purple)
    }

    /// Whether this spray color is lethal to regular non-boss enemies (`Green` = `2`).
    pub const fn is_lethal_to_regular_enemies(&self) -> bool {
        matches!(self, Self::Green)
    }

    /// Numeric power value matching `DAT_0122b319` (`-1`, `0`, `1`, `2`).
    pub const fn raw_power(&self) -> i8 {
        *self as i8
    }
}

/// Dispatch category for player attacks (`shBattleExecAttackQueue`, `SLUS_202.28` `0x00123760`
/// / `sh2pc.exe` `0x00544f90`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackDispatchKind {
    /// Unused/reload sentinel slot (`ap == 0.0`).
    None,
    /// Single-ray firearm (`shBattleAttackByHumanGunshotTypeA` `0x00120cf0`): Handgun `1..=2`, Rifle `6`, Maria Revolver `25..=26`.
    GunshotTypeA,
    /// 10-pellet 3D cone firearm (`shBattleAttackByHumanGunshotTypeB` `0x00120f30`): Shotgun `4`.
    GunshotTypeB,
    /// 5-ray aerosol cone (`shBattleAttackByHumanFog` `0x00121a40`): Hyper Spray `8..=9`.
    HumanFog,
    /// Swept melee blade quad (`shBattleAttackByHumanFightType` `0x00121370` / `sh2pc` `0x00543ec0`):
    /// Plank `12..=14`, Pipe `15..=18`, Great Knife `19..=22`, Chainsaw `23..=24`, Maria Cleaver `28..=30`.
    HumanFight,
    /// Downed enemy finisher (`shBattleAttackByHumanFinish` `0x00121f40`): Kicks `31..=35`, Stomps `36..=40`.
    HumanFinish,
    /// Enemy attack (`IDs 42..=71`).
    Enemy,
}

/// Attack definition from Retail PC `sh2_attack_list` (`sh2pc.exe` `0x00629000` / `0x006ec380`, 36 bytes per entry).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackSpec {
    /// Entry index / attack ID (`+0x00`, `0..=71`).
    pub id: u32,
    /// Collision/attack kind (`+0x02` in `sh2_attack_list`).
    pub kind: u8,
    /// Attack Power (`+0x04`): reduces both enemy HP and Endurance (`enReduceHP` `0x00142200`).
    pub ap: f32,
    /// Hitback / Stagger Power (`+0x08`): hitback impulse (`sp * 2.0` in `enSetHitBack` `0x00142630`).
    pub sp: f32,
    /// Minimum ray/blade offset along weapon axis in PS2 world mm (`+0x0c`).
    pub min_range: f32,
    /// Maximum ray/blade reach along weapon axis in PS2 world mm (`+0x10`).
    pub max_range: f32,
    /// First active 30 Hz animation hit frame (`+0x20`).
    pub first_hit_frame: u8,
    /// Last active 30 Hz animation hit frame inclusive (`+0x21`).
    pub last_hit_frame: u8,
    /// Associated sound effect ID (`SLUS_202.28` `0x00120cf0`, `0x00120f30`, `0x00121a40`, `0x00120858`).
    pub sfx_id: u32,
}

impl AttackSpec {
    /// Classify how `shBattleExecAttackQueue` (`SLUS_202.28` `0x00123760` / `sh2pc.exe` `0x00544f90`)
    /// dispatches hit detection for this attack ID.
    pub const fn dispatch_kind(&self) -> AttackDispatchKind {
        match self.id {
            1 | 2 | 6 | 25 | 26 => AttackDispatchKind::GunshotTypeA,
            4 => AttackDispatchKind::GunshotTypeB,
            8 | 9 => AttackDispatchKind::HumanFog,
            12..=24 | 28..=30 => AttackDispatchKind::HumanFight,
            31..=40 => AttackDispatchKind::HumanFinish,
            42..=71 => AttackDispatchKind::Enemy,
            _ => AttackDispatchKind::None,
        }
    }

    /// Returns `true` if `shBattleAttackByHumanFightType` (`SLUS_202.28` `0x00121370` / `sh2pc.exe` `0x00543ec0`)
    /// evaluates hits on **every** active frame without locking out after the first hit.
    ///
    /// Verified in `sh2pc.exe` `0x00543ec0` lines 2822–2834:
    /// - Single-hit melee (`12..=18`): Wooden Plank & Steel Pipe (gated by `*(int *)(chara + 0x124) == 0`).
    /// - Multi-hit melee (`19..=24`, `28..=30`): Great Knife, Chainsaw, and Maria Chinese Cleaver.
    /// - Hyper Spray (`8..=9`): Continuous 5-ray cone every tick while spraying (`0x00121a40`).
    pub const fn is_multi_hit(&self) -> bool {
        matches!(self.id, 8 | 9 | 19..=24 | 28..=30)
    }
}

#[allow(clippy::too_many_arguments)]
const fn entry(
    id: u32,
    kind: u8,
    ap: f32,
    sp: f32,
    min_range: f32,
    max_range: f32,
    first_hit_frame: u8,
    last_hit_frame: u8,
    sfx_id: u32,
) -> AttackSpec {
    AttackSpec {
        id,
        kind,
        ap,
        sp,
        min_range,
        max_range,
        first_hit_frame,
        last_hit_frame,
        sfx_id,
    }
}

/// Complete 72-entry Retail PC `sh2_attack_list` extracted 1:1 from `sh2pc.exe` `0x00629000` / `0x006ec380`.
///
/// Every entry matches the exact 36-byte (`0x24`) binary record in `sh2pc.exe`:
/// - `0`: Empty sentinel
/// - `1..=2`: Handgun standing/moving shot (`ap=120, sp=60, range=-100..7500`)
/// - `3`: Handgun reload sentinel
/// - `4`: Shotgun 10-pellet 3D cone (`ap=100, sp=120, range=-100..5000`)
/// - `5`: Shotgun reload sentinel
/// - `6`: Hunting Rifle (`ap=500, sp=150, range=-350..9000`)
/// - `7`: Hunting Rifle reload sentinel
/// - `8..=9`: Hyper Spray (`ap=1, sp=0, range=-125..1000`)
/// - `10..=11`: Hyper Spray shake/reload sentinels
/// - `12..=14`: Wooden Plank light/combo/heavy (`ap=100/80/140, sp=80, range=-150..350`)
/// - `15..=18`: Steel Pipe swing/combo/overhead/lunge (`ap=120/96/160/130, sp=100/100/100/50, range=-150..540`)
/// - `19..=22`: Great Knife swings & overhead chops (`ap=600/600/1000/1000, range=0..725`, multi-hit)
/// - `23..=24`: Chainsaw light & forward thrust (`ap=250/300, range=0..525`, multi-hit)
/// - `25..=26`: Maria Revolver (`ap=150, sp=0, range=0..7500`, Born From a Wish)
/// - `27`: Maria Revolver reload sentinel
/// - `28..=30`: Maria Chinese Cleaver light/combo/heavy (`ap=60/35/60, range=-100..250`, multi-hit)
/// - `31..=35`: James/Maria Kicks (5 weapon grip variants, `ap=9999.9, sp=0.1, range=0..350`)
/// - `36..=40`: James/Maria Stomps (5 weapon grip variants, `ap=9999.9, sp=0.1, range=0..350`)
/// - `41`: Empty separator (`id=0` in `sh2pc.exe`)
/// - `42..=71`: Enemy attacks (shifted +6 from `SLUS_202.28` `36..=65`), including retail grapple
///   strike/hug pairs: Pyramid Head `53/54`, Flesh Lips `60/61`, Abstract Daddy `63/64`,
///   Mary Tentacle `65/66`, Mary Moth Swarm `68/69`.
pub const SH2_ATTACK_LIST: [AttackSpec; 72] = [
    entry(0, 0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0),
    entry(1, 3, 120.0, 60.0, -100.0, 7500.0, 15, 25, SE_HANDGUN_SHOT),
    entry(2, 3, 120.0, 60.0, -100.0, 7500.0, 0, 10, SE_HANDGUN_SHOT),
    entry(3, 0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0),
    entry(4, 3, 100.0, 120.0, -100.0, 5000.0, 0, 10, SE_SHOTGUN_SHOT),
    entry(5, 0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0),
    entry(6, 7, 500.0, 150.0, -350.0, 9000.0, 0, 10, SE_RIFLE_SHOT),
    entry(7, 0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0),
    entry(8, 7, 1.0, 0.0, -125.0, 1000.0, 8, 25, SE_HYPER_SPRAY_SHOT),
    entry(9, 7, 1.0, 0.0, -125.0, 1000.0, 2, 19, SE_HYPER_SPRAY_SHOT),
    entry(10, 0, 0.0, 0.0, 0.0, 0.0, 0, 30, 0),
    entry(11, 0, 0.0, 0.0, 0.0, 0.0, 0, 24, 0),
    entry(12, 2, 100.0, 80.0, -150.0, 350.0, 12, 15, SE_PLANK_SWING),
    entry(13, 2, 80.0, 80.0, -150.0, 350.0, 8, 11, SE_PLANK_SWING),
    entry(14, 2, 140.0, 80.0, -150.0, 350.0, 8, 10, SE_PLANK_SWING),
    entry(
        15,
        2,
        120.0,
        100.0,
        -150.0,
        540.0,
        10,
        12,
        SE_PIPE_OR_FINISH,
    ),
    entry(16, 2, 96.0, 100.0, -150.0, 540.0, 4, 6, SE_PIPE_OR_FINISH),
    entry(
        17,
        2,
        160.0,
        100.0,
        -150.0,
        540.0,
        15,
        17,
        SE_PIPE_OR_FINISH,
    ),
    entry(18, 3, 130.0, 50.0, -150.0, 540.0, 8, 10, SE_PIPE_OR_FINISH),
    entry(19, 1, 600.0, 0.0, 0.0, 725.0, 10, 14, SE_MELEE_SWISH),
    entry(20, 1, 600.0, 0.0, 0.0, 725.0, 13, 17, SE_MELEE_SWISH),
    entry(21, 1, 1000.0, 0.0, 0.0, 725.0, 17, 18, SE_MELEE_SWISH),
    entry(22, 1, 1000.0, 0.0, 0.0, 725.0, 18, 19, SE_MELEE_SWISH),
    entry(23, 1, 250.0, 0.0, 0.0, 525.0, 12, 27, SE_CHAINSAW_ATTACK),
    entry(24, 3, 300.0, 0.0, 0.0, 525.0, 6, 13, SE_CHAINSAW_ATTACK),
    entry(25, 3, 150.0, 0.0, 0.0, 7500.0, 14, 25, SE_HANDGUN_SHOT),
    entry(26, 3, 150.0, 0.0, 0.0, 7500.0, 0, 10, SE_HANDGUN_SHOT),
    entry(27, 0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0),
    entry(28, 1, 60.0, 0.0, -100.0, 250.0, 12, 15, SE_MELEE_SWISH),
    entry(29, 1, 35.0, 0.0, -100.0, 250.0, 8, 11, SE_MELEE_SWISH),
    entry(30, 1, 60.0, 0.0, -100.0, 250.0, 8, 10, SE_MELEE_SWISH),
    entry(31, 1, 9999.9, 0.1, 0.0, 350.0, 7, 10, SE_PIPE_OR_FINISH),
    entry(32, 1, 9999.9, 0.1, 0.0, 350.0, 7, 10, SE_PIPE_OR_FINISH),
    entry(33, 1, 9999.9, 0.1, 0.0, 350.0, 7, 10, SE_PIPE_OR_FINISH),
    entry(34, 1, 9999.9, 0.1, 0.0, 350.0, 9, 9, SE_PIPE_OR_FINISH),
    entry(35, 1, 9999.9, 0.1, 0.0, 350.0, 7, 11, SE_PIPE_OR_FINISH),
    entry(36, 1, 9999.9, 0.1, 0.0, 350.0, 3, 9, SE_PIPE_OR_FINISH),
    entry(37, 1, 9999.9, 0.1, 0.0, 350.0, 3, 9, SE_PIPE_OR_FINISH),
    entry(38, 1, 9999.9, 0.1, 0.0, 350.0, 3, 9, SE_PIPE_OR_FINISH),
    entry(39, 1, 9999.9, 0.1, 0.0, 350.0, 3, 9, SE_PIPE_OR_FINISH),
    entry(40, 1, 9999.9, 0.1, 0.0, 350.0, 3, 9, SE_PIPE_OR_FINISH),
    entry(0, 0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0),
    entry(42, 4, 7.0, 5.0, 0.0, 250.0, 9, 22, 0),
    entry(43, 3, 5.0, 5.0, 0.0, 250.0, 0, 16, 0),
    entry(44, 2, 18.0, 10.0, 0.0, 400.0, 19, 22, 0),
    entry(45, 2, 18.0, 10.0, 0.0, 400.0, 19, 22, 0),
    entry(46, 2, 20.0, 10.0, 0.0, 250.0, 15, 18, 0),
    entry(47, 3, 15.0, 12.5, 0.0, 250.0, 13, 16, 0),
    entry(48, 2, 20.0, 12.5, 0.0, 250.0, 15, 18, 0),
    entry(49, 3, 15.0, 15.0, 0.0, 250.0, 13, 16, 0),
    entry(50, 1, 40.0, 0.5, 0.0, 600.0, 21, 27, 0),
    entry(51, 1, 1000.0, 0.0, 0.0, 600.0, 67, 71, 0),
    entry(52, 2, 25.0, 12.5, 0.0, 250.0, 7, 10, 0),
    entry(53, 2, 10.0, 0.0, 0.0, 250.0, 8, 10, 0),
    entry(54, 6, 0.2, 0.0, 0.0, 250.0, 0, 23, 0),
    entry(55, 3, 30.0, 15.0, 0.0, 700.0, 17, 23, 0),
    entry(56, 2, 50.0, 17.5, 0.0, 700.0, 0, 28, 0),
    entry(57, 3, 5.0, 0.0, 0.0, 300.0, 0, 7, 0),
    entry(58, 3, 24.0, 0.0, 0.0, 5000.0, 0, 1, 0),
    entry(59, 2, 15.0, 10.0, 0.0, 250.0, 9, 11, 0),
    entry(60, 2, 7.0, 0.0, 0.0, 250.0, 28, 31, 0),
    entry(61, 6, 0.2, 0.0, 0.0, 250.0, 0, 23, 0),
    entry(62, 2, 17.0, 7.5, 0.0, 250.0, 11, 14, 0),
    entry(63, 2, 10.0, 0.0, 0.0, 400.0, 0, 19, 0),
    entry(64, 6, 0.2, 0.0, 0.0, 400.0, 0, 23, 0),
    entry(65, 2, 10.0, 0.0, 0.0, 300.0, 11, 14, 0),
    entry(66, 6, 0.2, 0.0, 0.0, 300.0, 0, 23, 0),
    entry(67, 2, 20.0, 10.0, 0.0, 300.0, 7, 9, 0),
    entry(68, 2, 1.0, 0.0, 0.0, 300.0, 0, 99, 0),
    entry(69, 6, 0.05, 0.0, 0.0, 300.0, 0, 99, 0),
    entry(70, 3, 10.0, 0.0, 0.0, 450.0, 0, 16, 0),
    entry(71, 3, 10.0, 0.0, 0.0, 450.0, 0, 16, 0),
];

/// Look up an entry in the 72-entry Retail PC `sh2_attack_list` (`0x00629000` / `0x006ec380`).
#[inline]
pub const fn get_attack_spec(id: usize) -> Option<&'static AttackSpec> {
    if id < SH2_ATTACK_LIST.len() {
        Some(&SH2_ATTACK_LIST[id])
    } else {
        None
    }
}

/// Authentic Retail PC Kick attack specs (`sh2_attack_list[31..=35]`, `0x00629000`).
pub const ATK_KICKS: [AttackSpec; 5] = [
    SH2_ATTACK_LIST[31],
    SH2_ATTACK_LIST[32],
    SH2_ATTACK_LIST[33],
    SH2_ATTACK_LIST[34],
    SH2_ATTACK_LIST[35],
];

/// Authentic Retail PC Stomp attack specs (`sh2_attack_list[36..=40]`, `0x00629000`).
pub const ATK_STOMPS: [AttackSpec; 5] = [
    SH2_ATTACK_LIST[36],
    SH2_ATTACK_LIST[37],
    SH2_ATTACK_LIST[38],
    SH2_ATTACK_LIST[39],
    SH2_ATTACK_LIST[40],
];

/// Handgun standing shot (`sh2_attack_list[1]`, `0x00629000`).
pub const ATK_HANDGUN_STANDING: AttackSpec = SH2_ATTACK_LIST[1];
/// Handgun ready/moving shot (`sh2_attack_list[2]`, `0x00629000`).
pub const ATK_HANDGUN: AttackSpec = SH2_ATTACK_LIST[2];
/// Shotgun 10-pellet 3D cone shot (`sh2_attack_list[4]`, `0x00629000`).
pub const ATK_SHOTGUN: AttackSpec = SH2_ATTACK_LIST[4];
/// Hunting Rifle shot (`sh2_attack_list[6]`, `0x00629000`).
pub const ATK_HUNTING_RIFLE: AttackSpec = SH2_ATTACK_LIST[6];
/// Hyper Spray initial burst (`sh2_attack_list[8]`, `0x00629000`).
pub const ATK_HYPER_SPRAY_1: AttackSpec = SH2_ATTACK_LIST[8];
/// Hyper Spray sustained spray (`sh2_attack_list[9]`, `0x00629000`).
pub const ATK_HYPER_SPRAY_2: AttackSpec = SH2_ATTACK_LIST[9];

/// Wooden Plank light swing (`sh2_attack_list[12]`, `0x00629000`).
pub const ATK_PLANK_LIGHT: AttackSpec = SH2_ATTACK_LIST[12];
/// Wooden Plank combo swing (`sh2_attack_list[13]`, `0x00629000`).
pub const ATK_PLANK_COMBO: AttackSpec = SH2_ATTACK_LIST[13];
/// Wooden Plank downward heavy swing (`sh2_attack_list[14]`, `0x00629000`).
pub const ATK_PLANK_HEAVY: AttackSpec = SH2_ATTACK_LIST[14];

/// Steel Pipe swing (`sh2_attack_list[15]`, `0x00629000`).
pub const ATK_PIPE_SWING: AttackSpec = SH2_ATTACK_LIST[15];
/// Steel Pipe combo swing (`sh2_attack_list[16]`, `0x00629000`).
pub const ATK_PIPE_COMBO: AttackSpec = SH2_ATTACK_LIST[16];
/// Steel Pipe overhead heavy swing (`sh2_attack_list[17]`, `0x00629000`).
pub const ATK_PIPE_OVERHEAD: AttackSpec = SH2_ATTACK_LIST[17];
/// Steel Pipe forward lunge thrust (`sh2_attack_list[18]`, `0x00629000`).
pub const ATK_PIPE_LUNGE: AttackSpec = SH2_ATTACK_LIST[18];

/// Great Knife horizontal swing 1 (`sh2_attack_list[19]`, `0x00629000`, multi-hit).
pub const ATK_GREAT_KNIFE_SWING_1: AttackSpec = SH2_ATTACK_LIST[19];
/// Great Knife horizontal swing 2 (`sh2_attack_list[20]`, `0x00629000`, multi-hit).
pub const ATK_GREAT_KNIFE_SWING_2: AttackSpec = SH2_ATTACK_LIST[20];
/// Great Knife overhead chop 1 (`sh2_attack_list[21]`, `0x00629000`, multi-hit).
pub const ATK_GREAT_KNIFE_OVERHEAD_1: AttackSpec = SH2_ATTACK_LIST[21];
/// Great Knife overhead chop 2 (`sh2_attack_list[22]`, `0x00629000`, multi-hit).
pub const ATK_GREAT_KNIFE_OVERHEAD_2: AttackSpec = SH2_ATTACK_LIST[22];

/// Chainsaw light swing (`sh2_attack_list[23]`, `0x00629000`, multi-hit).
pub const ATK_CHAINSAW_LIGHT: AttackSpec = SH2_ATTACK_LIST[23];
/// Chainsaw forward thrust (`sh2_attack_list[24]`, `0x00629000`, multi-hit).
pub const ATK_CHAINSAW_THRUST: AttackSpec = SH2_ATTACK_LIST[24];

/// Maria Revolver standing shot (`sh2_attack_list[25]`, `0x00629000`, Born From a Wish).
pub const ATK_MARIA_REVOLVER_1: AttackSpec = SH2_ATTACK_LIST[25];
/// Maria Revolver ready/moving shot (`sh2_attack_list[26]`, `0x00629000`, Born From a Wish).
pub const ATK_MARIA_REVOLVER_2: AttackSpec = SH2_ATTACK_LIST[26];

/// Maria Chinese Cleaver light swing (`sh2_attack_list[28]`, `0x00629000`, multi-hit).
pub const ATK_MARIA_CLEAVER_LIGHT: AttackSpec = SH2_ATTACK_LIST[28];
/// Maria Chinese Cleaver combo swing (`sh2_attack_list[29]`, `0x00629000`, multi-hit).
pub const ATK_MARIA_CLEAVER_COMBO: AttackSpec = SH2_ATTACK_LIST[29];
/// Maria Chinese Cleaver heavy swing (`sh2_attack_list[30]`, `0x00629000`, multi-hit).
pub const ATK_MARIA_CLEAVER_HEAVY: AttackSpec = SH2_ATTACK_LIST[30];
