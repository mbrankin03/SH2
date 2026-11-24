//! Enemy & boss character model catalog, skeletal rig metadata, and weapon/attack socket
//! specifications (`Enemy_List`: `scu`, `mkn`, `tyu`, `nse`, `xoo`, `arm`, `red`, `oni`, `ike`,
//! `pap`, `bos`, `edi`) (`SH2-35`).
//!
//! # Ground Truth
//! - PS2 Prototype `SLUS_202.28`:
//!   - `Chacter/m3_scu.c`: `EnemySCUFunction` (`0x0011f8d0`, `0x200`), `shEnemySCU_EffectFoot` (`0x0011ff30`, bones `20`/`21`)
//!   - `Chacter/m3_mkn.c`: `EnemyMKNFunction` (`0x001058d0`, `0x201`), `shGetEnemyMKNAttackPos` (`0x00105bb0`, bones `5`/`7`, `[500,0,0]` at `0x002a8a30`)
//!   - `Chacter/m3_tyu.c`: `EnemyTYUFunction` (`0x00220710`, `0x202`), `shGetEnemyTYUAttackPos` (`0x00220920`, bone `0`, `[-500,200,0]` at `0x0034c280`)
//!   - `Chacter/m3_ike.c`: `EnemyIKEFunction` (`0x00105150`, `0x203`), `shGetEnemyIKEAttackPos` (`0x00105430`, bone `5`, `[93,237,0]` at `0x002a88a0`)
//!   - `Chacter/m3_pap.c`: `EnemyPAPFunction` (`0x00105e80`, `0x204`), `shGetEnemyPAPAttackPos` (`0x00106170`, bone `14`, `[±293,226,0]` at `0x002a8a60`)
//!   - `Chacter/m3_edb.c`: `EnemyEDBFunction` (`0x00209bc0`, `0x205`), `shGetEnemyEDBAttackPos` (`0x00209e90`, bone `31` `[208,-26,-68]`->`[511,-64,-88]`, bone `27` `[500,0,0]` at `0x00347ff0`)
//!   - `Chacter/m3_bos.c`: `EnemyBOSFunction` (`0x00220b20`, `0x206`), `shGetEnemyBOSAttackPos` (`0x00220df0`, bone `44`, `[500,0,0]` at `0x0034c2f0`)
//!   - `Chacter/m3_nse.c`: `EnemyNSEFunction` (`0x0020dad0`, `0x207`/`0x20b`), `shGetEnemyNSEAttackPos` (`0x0020dd10`, bone `24`, `[500,0,0]` at `0x00348000`)
//!   - `Chacter/m3_red.c`: `EnemyREDFunction` (`0x0011c990`, `0x208`), `shCharacterSetWeaponRED` (`0x0011caf0`, `pose_index = 3`), `shGetEnemyREDAttackPos` (`0x0011ccd0`, bones `25`/`36`, `[500,0,0]` at `0x002a93c0`)
//!   - `Chacter/m3_oni.c`: `EnemyONIFunction` (`0x00221060`, `0x209`), `shGetEnemyONIAttackPos` (`0x00221340`, bone `36`, `[500,0,0]` at `0x0034c330`)
//!   - `Chacter/m3_arm.c`: `EnemyARMFunction` (`0x00238ee0`, `0x20a`), `shGetEnemyARMAttackPos` (`0x002390f0`, bones `13`/`14`, `[500,0,0]` at `0x0034e900`)
//!   - `Chacter/m3_wep.c`: `shUpdateWeaponMatrixAfterAnime` (`0x00120120`, James right-hand weapon socket bone `35`)
//! - Retail PC `sh2pc.exe` (`STRINGS.tsv`): `data/chr/{scu,mkn,tyu,nse,xoo,arm,red,oni,ike,pap,bos,edi}/...`

use super::anm::mat4_mul;

/// Canonical `Enemy_List` character model identifiers (`0x200..=0x20b` in `SLUS_202.28` / `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum EnemyModelKey {
    /// `0x200`: Lying Figure (`chr/scu/scu.mdl`, 30 bones).
    Scu = 0x200,
    /// `0x201`: Mannequin (`chr/mkn/mkn.mdl`, 17 bones).
    Mkn = 0x201,
    /// `0x202`: Creeper (`chr/tyu/tyu.mdl`, 22 bones).
    Tyu = 0x202,
    /// `0x203`: Flesh Lips (`chr/ike/ike.mdl`, 16 bones).
    Ike = 0x203,
    /// `0x204`: Abstract Daddy (`chr/pap/pap.mdl`, 29 bones).
    Pap = 0x204,
    /// `0x205`: Eddie Boss (`chr/edi/lll_edi.mdl`, 35 bones).
    Edi = 0x205,
    /// `0x206`: Mary / Maria Final Boss (`chr/bos/bos.mdl`, 45 bones).
    Bos = 0x206,
    /// `0x207`: Bubble Head Nurse (`chr/nse/nse.mdl`, 30 bones).
    Nse = 0x207,
    /// `0x208`: Pyramid Head - Great Knife (`chr/red/red.mdl`, 45 bones).
    Red = 0x208,
    /// `0x209`: Pyramid Head - Great Spear (`chr/oni/oni.mdl`, 41 bones).
    Oni = 0x209,
    /// `0x20a`: Mandarin (`chr/arm/arm.mdl`, 15 bones).
    Arm = 0x20a,
    /// `0x20b`: Bubble Head Nurse Variant (`chr/xoo/xoo.mdl`, 30 bones).
    Xoo = 0x20b,
}

impl EnemyModelKey {
    /// All 12 `Enemy_List` character models in canonical ID order (`0x200..=0x20b`).
    pub const ALL: [Self; 12] = [
        Self::Scu,
        Self::Mkn,
        Self::Tyu,
        Self::Ike,
        Self::Pap,
        Self::Edi,
        Self::Bos,
        Self::Nse,
        Self::Red,
        Self::Oni,
        Self::Arm,
        Self::Xoo,
    ];

    /// Looks up an [`EnemyModelKey`] from its 16-bit `SLUS_202.28` character model ID (`0x200..=0x20b`).
    #[must_use]
    pub const fn from_char_id(char_id: u16) -> Option<Self> {
        match char_id {
            0x200 => Some(Self::Scu),
            0x201 => Some(Self::Mkn),
            0x202 => Some(Self::Tyu),
            0x203 => Some(Self::Ike),
            0x204 => Some(Self::Pap),
            0x205 => Some(Self::Edi),
            0x206 => Some(Self::Bos),
            0x207 => Some(Self::Nse),
            0x208 => Some(Self::Red),
            0x209 => Some(Self::Oni),
            0x20a => Some(Self::Arm),
            0x20b => Some(Self::Xoo),
            _ => None,
        }
    }

    /// Returns the static retail specification for this enemy/boss model.
    #[must_use]
    pub const fn spec(self) -> &'static EnemyModelSpec {
        match self {
            Self::Scu => &ENEMY_MODEL_SPECS[0],
            Self::Mkn => &ENEMY_MODEL_SPECS[1],
            Self::Tyu => &ENEMY_MODEL_SPECS[2],
            Self::Ike => &ENEMY_MODEL_SPECS[3],
            Self::Pap => &ENEMY_MODEL_SPECS[4],
            Self::Edi => &ENEMY_MODEL_SPECS[5],
            Self::Bos => &ENEMY_MODEL_SPECS[6],
            Self::Nse => &ENEMY_MODEL_SPECS[7],
            Self::Red => &ENEMY_MODEL_SPECS[8],
            Self::Oni => &ENEMY_MODEL_SPECS[9],
            Self::Arm => &ENEMY_MODEL_SPECS[10],
            Self::Xoo => &ENEMY_MODEL_SPECS[11],
        }
    }
}

/// Static retail asset & skeletal rig descriptor for an `Enemy_List` character (`0x200..=0x20b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyModelSpec {
    /// Canonical enemy model key.
    pub key: EnemyModelKey,
    /// 16-bit character ID stored in `.mdl` header `+0x04` and `Enemy_List`.
    pub char_id: u16,
    /// Short folder/code name (`scu`, `mkn`, `tyu`, `ike`, `pap`, `edi`, `bos`, `nse`, `red`, `oni`, `arm`, `xoo`).
    pub code: &'static str,
    /// Primary vanilla retail `.mdl` path relative to game root.
    pub mdl_rel_path: &'static str,
    /// Optional SH2:EE HD override `.mdl` path relative to game root (`sh2e/chr/...`).
    pub sh2e_mdl_rel_path: Option<&'static str>,
    /// Primary gameplay `.anm` path relative to game root.
    pub primary_anm_rel_path: &'static str,
    /// Additional retail `.anm` files for this species (`mkn_311.anm`, `red_wlk.anm`, `p_edi.anm`).
    pub secondary_anm_rel_paths: &'static [&'static str],
    /// Expected bone count in the retail `.mdl` skeleton.
    pub expected_bone_count: usize,
    /// Optional equipment `pose_index` toggled by weapon visibility (`red.mdl` `pose_index = 3`).
    pub weapon_pose_index: Option<u8>,
}

/// Complete specification table for all 12 `Enemy_List` models (`0x200..=0x20b`).
pub const ENEMY_MODEL_SPECS: [EnemyModelSpec; 12] = [
    EnemyModelSpec {
        key: EnemyModelKey::Scu,
        char_id: 0x200,
        code: "scu",
        mdl_rel_path: "data/chr/scu/scu.mdl",
        sh2e_mdl_rel_path: Some("sh2e/chr/scu/scu.mdl"),
        primary_anm_rel_path: "data/chr/scu/scu.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 30,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Mkn,
        char_id: 0x201,
        code: "mkn",
        mdl_rel_path: "data/chr/mkn/mkn.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/mkn/mkn.anm",
        secondary_anm_rel_paths: &["data/chr/mkn/mkn_311.anm"],
        expected_bone_count: 17,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Tyu,
        char_id: 0x202,
        code: "tyu",
        mdl_rel_path: "data/chr/tyu/tyu.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/tyu/tyu.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 22,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Ike,
        char_id: 0x203,
        code: "ike",
        mdl_rel_path: "data/chr/ike/ike.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/ike/ike.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 16,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Pap,
        char_id: 0x204,
        code: "pap",
        mdl_rel_path: "data/chr/pap/pap.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/pap/pap.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 29,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Edi,
        char_id: 0x205,
        code: "edi",
        mdl_rel_path: "data/chr/edi/lll_edi.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/edi/lll_edi.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 35,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Bos,
        char_id: 0x206,
        code: "bos",
        mdl_rel_path: "data/chr/bos/bos.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/bos/bos.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 45,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Nse,
        char_id: 0x207,
        code: "nse",
        mdl_rel_path: "data/chr/nse/nse.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/nse/nse.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 30,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Red,
        char_id: 0x208,
        code: "red",
        mdl_rel_path: "data/chr/red/red.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/red/red.anm",
        secondary_anm_rel_paths: &["data/chr/red/red_wlk.anm"],
        expected_bone_count: 45,
        weapon_pose_index: Some(3),
    },
    EnemyModelSpec {
        key: EnemyModelKey::Oni,
        char_id: 0x209,
        code: "oni",
        mdl_rel_path: "data/chr/oni/oni.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/oni/oni.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 41,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Arm,
        char_id: 0x20a,
        code: "arm",
        mdl_rel_path: "data/chr/arm/arm.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/arm/arm.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 15,
        weapon_pose_index: None,
    },
    EnemyModelSpec {
        key: EnemyModelKey::Xoo,
        char_id: 0x20b,
        code: "xoo",
        mdl_rel_path: "data/chr/xoo/xoo.mdl",
        sh2e_mdl_rel_path: None,
        primary_anm_rel_path: "data/chr/xoo/xoo.anm",
        secondary_anm_rel_paths: &[],
        expected_bone_count: 30,
        weapon_pose_index: None,
    },
];

/// Named bone-attached weapon, attack, or foot-effect socket from `SLUS_202.28` `Chacter/m3_*.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkeletalSocketKind {
    /// James Sunderland right-hand weapon socket (`m3_wep.c` `shUpdateWeaponMatrixAfterAnime` `0x00120120`: bone `0x23` = `35`).
    JamesRightHand,
    /// Lying Figure left foot ground-impact socket (`m3_scu.c` `shEnemySCU_EffectFoot` `0x0011ff30`: bone `0x15` = `21`, `+50.0` Y).
    ScuLeftFoot,
    /// Lying Figure right foot ground-impact socket (`m3_scu.c` `shEnemySCU_EffectFoot` `0x0011ff30`: bone `0x14` = `20`, `+50.0` Y).
    ScuRightFoot,
    /// Mannequin arm swing attack socket (`m3_mkn.c` `shGetEnemyMKNAttackPos` `0x00105bb0`: bone `5`, `[500, 0, 0]`).
    MknArmSwing,
    /// Mannequin kick attack socket (`m3_mkn.c` `shGetEnemyMKNAttackPos` `0x00105bb0`: bone `7`, `[500, 0, 0]`).
    MknKick,
    /// Creeper mandible bite socket (`m3_tyu.c` `shGetEnemyTYUAttackPos` `0x00220920`: bone `0`, `[-500, 200, 0]`).
    TyuMandibles,
    /// Bubble Head Nurse (`nse` / `xoo`) steel pipe socket (`m3_nse.c` `shGetEnemyNSEAttackPos` `0x0020dd10`: bone `0x18` = `24`, `[500, 0, 0]`).
    NsePipe,
    /// Mandarin left tentacle socket (`m3_arm.c` `shGetEnemyARMAttackPos` `0x002390f0`: bone `0x0d` = `13`, `[500, 0, 0]`).
    ArmLeftTentacle,
    /// Mandarin right tentacle socket (`m3_arm.c` `shGetEnemyARMAttackPos` `0x002390f0`: bone `0x0e` = `14`, `[500, 0, 0]`).
    ArmRightTentacle,
    /// Pyramid Head Great Knife blade socket (`m3_red.c` `shGetEnemyREDAttackPos` `0x0011ccd0`: bone `0x19` = `25`, `[500, 0, 0]`).
    RedGreatKnife,
    /// Pyramid Head right-hand grab socket (`m3_red.c` `shGetEnemyREDAttackPos` `0x0011ccd0`: bone `0x24` = `36`, `[500, 0, 0]`).
    RedHandGrab,
    /// Pyramid Head Great Spear socket (`m3_oni.c` `shGetEnemyONIAttackPos` `0x00221340`: bone `0x24` = `36`, `[500, 0, 0]`).
    OniGreatSpear,
    /// Flesh Lips body/attack socket (`m3_ike.c` `shGetEnemyIKEAttackPos` `0x00105430`: bone `5`, `[93, 237, 0]`).
    IkeBody,
    /// Abstract Daddy arm socket (`param_4 == 0x39`, `m3_pap.c` `shGetEnemyPAPAttackPos` `0x00106170`: bone `0x0e` = `14`, `[293, 226, 0]`).
    PapArmPositive,
    /// Abstract Daddy arm socket (`param_4 != 0x39`, `m3_pap.c` `shGetEnemyPAPAttackPos` `0x00106170`: bone `0x0e` = `14`, `[-293, 226, 0]`).
    PapArmNegative,
    /// Mary Final Boss tendril socket (`m3_bos.c` `shGetEnemyBOSAttackPos` `0x00220df0`: bone `0x2c` = `44`, `[500, 0, 0]`).
    BosTendril,
    /// Eddie Boss revolver muzzle socket (`param_4 == 0x34`, `m3_edb.c` `shGetEnemyEDBAttackPos` `0x00209e90`: bone `0x1f` = `31`, `[208,-26,-68]` -> `[511,-64,-88]`).
    EdbRevolverMuzzle,
    /// Eddie Boss melee socket (`param_4 == 0x35`, `m3_edb.c` `shGetEnemyEDBAttackPos` `0x00209e90`: bone `0x1b` = `27`, `[500, 0, 0]`).
    EdbMelee,
}

/// Bone-local socket definition in PS2 world millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponSocketSpec {
    /// Socket identifier.
    pub kind: SkeletalSocketKind,
    /// Zero-based bone index in the target `.mdl` skeleton.
    pub bone_index: usize,
    /// Bone-local origin offset in PS2 mm (default `[0.0, 0.0, 0.0]` = bone pivot).
    pub local_origin: [f32; 3],
    /// Bone-local tip/forward offset in PS2 mm used to compute world attack/aim direction.
    pub local_tip: [f32; 3],
}

impl SkeletalSocketKind {
    /// Returns the exact `SLUS_202.28` bone index and local offsets for this socket.
    #[must_use]
    pub const fn spec(self) -> WeaponSocketSpec {
        match self {
            Self::JamesRightHand => WeaponSocketSpec {
                kind: self,
                bone_index: 0x23,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [0.0, 0.0, 100.0],
            },
            Self::ScuLeftFoot => WeaponSocketSpec {
                kind: self,
                bone_index: 0x15,
                local_origin: [0.0, 50.0, 0.0],
                local_tip: [0.0, 50.0, 100.0],
            },
            Self::ScuRightFoot => WeaponSocketSpec {
                kind: self,
                bone_index: 0x14,
                local_origin: [0.0, 50.0, 0.0],
                local_tip: [0.0, 50.0, 100.0],
            },
            Self::MknArmSwing => WeaponSocketSpec {
                kind: self,
                bone_index: 5,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::MknKick => WeaponSocketSpec {
                kind: self,
                bone_index: 7,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::TyuMandibles => WeaponSocketSpec {
                kind: self,
                bone_index: 0,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [-500.0, 200.0, 0.0],
            },
            Self::NsePipe => WeaponSocketSpec {
                kind: self,
                bone_index: 0x18,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::ArmLeftTentacle => WeaponSocketSpec {
                kind: self,
                bone_index: 0x0d,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::ArmRightTentacle => WeaponSocketSpec {
                kind: self,
                bone_index: 0x0e,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::RedGreatKnife => WeaponSocketSpec {
                kind: self,
                bone_index: 0x19,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::RedHandGrab => WeaponSocketSpec {
                kind: self,
                bone_index: 0x24,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::OniGreatSpear => WeaponSocketSpec {
                kind: self,
                bone_index: 0x24,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::IkeBody => WeaponSocketSpec {
                kind: self,
                bone_index: 5,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [93.0, 237.0, 0.0],
            },
            Self::PapArmPositive => WeaponSocketSpec {
                kind: self,
                bone_index: 0x0e,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [293.0, 226.0, 0.0],
            },
            Self::PapArmNegative => WeaponSocketSpec {
                kind: self,
                bone_index: 0x0e,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [-293.0, 226.0, 0.0],
            },
            Self::BosTendril => WeaponSocketSpec {
                kind: self,
                bone_index: 0x2c,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
            Self::EdbRevolverMuzzle => WeaponSocketSpec {
                kind: self,
                bone_index: 0x1f,
                local_origin: [208.0, -26.0, -68.0],
                local_tip: [511.0, -64.0, -88.0],
            },
            Self::EdbMelee => WeaponSocketSpec {
                kind: self,
                bone_index: 0x1b,
                local_origin: [0.0, 0.0, 0.0],
                local_tip: [500.0, 0.0, 0.0],
            },
        }
    }
}

/// Returns all bone-attached weapon/attack sockets belonging to `key`.
#[must_use]
pub const fn enemy_sockets_for(key: EnemyModelKey) -> &'static [SkeletalSocketKind] {
    match key {
        EnemyModelKey::Scu => &[
            SkeletalSocketKind::ScuLeftFoot,
            SkeletalSocketKind::ScuRightFoot,
        ],
        EnemyModelKey::Mkn => &[SkeletalSocketKind::MknArmSwing, SkeletalSocketKind::MknKick],
        EnemyModelKey::Tyu => &[SkeletalSocketKind::TyuMandibles],
        EnemyModelKey::Nse | EnemyModelKey::Xoo => &[SkeletalSocketKind::NsePipe],
        EnemyModelKey::Arm => &[
            SkeletalSocketKind::ArmLeftTentacle,
            SkeletalSocketKind::ArmRightTentacle,
        ],
        EnemyModelKey::Red => &[
            SkeletalSocketKind::RedGreatKnife,
            SkeletalSocketKind::RedHandGrab,
        ],
        EnemyModelKey::Oni => &[SkeletalSocketKind::OniGreatSpear],
        EnemyModelKey::Ike => &[SkeletalSocketKind::IkeBody],
        EnemyModelKey::Pap => &[
            SkeletalSocketKind::PapArmPositive,
            SkeletalSocketKind::PapArmNegative,
        ],
        EnemyModelKey::Bos => &[SkeletalSocketKind::BosTendril],
        EnemyModelKey::Edi => &[
            SkeletalSocketKind::EdbRevolverMuzzle,
            SkeletalSocketKind::EdbMelee,
        ],
    }
}

/// World-space evaluated bone socket transform (`world_matrix`, `origin`, `tip`, `direction`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluatedSocket {
    /// Combined row-major 4x4 world matrix of the socket's parent bone (`entity_world * bone_pose`).
    pub bone_world_matrix: [[f32; 4]; 4],
    /// World-space socket origin in PS2 mm.
    pub origin: [f32; 3],
    /// World-space socket tip in PS2 mm.
    pub tip: [f32; 3],
    /// Unit direction vector `(tip - origin).normalize()`.
    pub direction: [f32; 3],
}

#[inline]
fn transform_point_row_major(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

/// Evaluates a [`WeaponSocketSpec`] in world space given `entity_world` and model-space
/// `bone_poses` (output of [`super::anm::evaluate_pose_into`]).
///
/// Matches `SLUS_202.28` `shGetEnemy*AttackPos` (`sceVu0ApplyMatrix` on bone local matrix + pivot,
/// then `entity_world`, followed by `vrsqrt` normalization of `tip - origin`).
#[must_use]
pub fn evaluate_socket_world(
    entity_world: [[f32; 4]; 4],
    bone_poses: &[[[f32; 4]; 4]],
    socket: &WeaponSocketSpec,
) -> Option<EvaluatedSocket> {
    let bone_pose = *bone_poses.get(socket.bone_index)?;
    let bone_world_matrix = mat4_mul(entity_world, bone_pose);
    let origin = transform_point_row_major(&bone_world_matrix, socket.local_origin);
    let tip = transform_point_row_major(&bone_world_matrix, socket.local_tip);
    let dx = tip[0] - origin[0];
    let dy = tip[1] - origin[1];
    let dz = tip[2] - origin[2];
    let len_sq = dx * dx + dy * dy + dz * dz;
    let direction = if len_sq > 1e-12 {
        let inv = len_sq.sqrt().recip();
        [dx * inv, dy * inv, dz * inv]
    } else {
        [0.0, 0.0, 1.0]
    };
    Some(EvaluatedSocket {
        bone_world_matrix,
        origin,
        tip,
        direction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    #[test]
    fn enemy_list_covers_all_12_species_and_sockets_fit_bone_counts() {
        assert_eq!(EnemyModelKey::ALL.len(), 12);
        for (i, &key) in EnemyModelKey::ALL.iter().enumerate() {
            assert_eq!(key as u16, 0x200 + i as u16);
            assert_eq!(EnemyModelKey::from_char_id(0x200 + i as u16), Some(key));
            let spec = key.spec();
            assert_eq!(spec.key, key);
            assert_eq!(spec.char_id, 0x200 + i as u16);
            for &sock in enemy_sockets_for(key) {
                assert!(
                    sock.spec().bone_index < spec.expected_bone_count,
                    "Socket {:?} bone {} out of range for {} ({})",
                    sock,
                    sock.spec().bone_index,
                    spec.code,
                    spec.expected_bone_count
                );
            }
        }
    }

    #[test]
    fn evaluate_socket_world_matches_eddie_revolver_muzzle_math() {
        let mut bone_poses = vec![IDENTITY; 35];
        // Translate bone 31 pivot to (10, 20, 30) in model space
        bone_poses[31][0][3] = 10.0;
        bone_poses[31][1][3] = 20.0;
        bone_poses[31][2][3] = 30.0;

        let mut entity_world = IDENTITY;
        entity_world[0][3] = 1000.0;

        let eval = evaluate_socket_world(
            entity_world,
            &bone_poses,
            &SkeletalSocketKind::EdbRevolverMuzzle.spec(),
        )
        .expect("valid socket");

        assert!((eval.origin[0] - 1218.0).abs() < 1e-4);
        assert!((eval.origin[1] - (-6.0)).abs() < 1e-4);
        assert!((eval.origin[2] - (-38.0)).abs() < 1e-4);

        let dir_len =
            (eval.direction[0].powi(2) + eval.direction[1].powi(2) + eval.direction[2].powi(2))
                .sqrt();
        assert!((dir_len - 1.0).abs() < 1e-5);
        assert!(eval.direction[0] > 0.9);
    }
}
