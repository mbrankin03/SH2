//! Pocket Radio constants, `enemy_track$2443` table, and `enSetRadioVolume` (`SH2-137`).
//!
//! Extracted from `SLUS_202.28` (`Event/event.c` `0x001ab300`, `Enemy/en_common.c` `enTransID`
//! `0x00141b30` & `enSetRadioVolume` `0x00143890`, `enemy_track$2443` `0x002b9850`).

use std::f32::consts::FRAC_PI_2;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::ai::creatures::EnemySpecies;

/// Maximum Pocket Radio detection radius in world mm (`RadioNoise` `SLUS_202.28` `0x001ab300`).
pub const RADIO_MAX_DISTANCE_MM: f32 = 12_500.0;

/// Number of simultaneous static tracks driven by `shSdRadio` (`0x001ab300`, tracks `1..=4`).
pub const RADIO_TRACK_COUNT: usize = 4;

/// Maximum number of nearest enemies mixed in `RadioNoise` Mode 0 (`0x001ab300`, 3 slots).
pub const RADIO_MAX_NEAREST_ENEMIES: usize = 3;

/// Per-track volume ramp divisor in seconds (`dt / 2.5`, `RadioNoise` `0x001ab300`).
pub const RADIO_VOLUME_RAMP_DIVISOR: f32 = 2.5;

/// Per-track stereo pan ramp divisor in seconds (`dt / 2.0`, `RadioNoise` `0x001ab300`).
pub const RADIO_PAN_RAMP_DIVISOR: f32 = 2.0;

/// Contributor weights for the 3 nearest enemies (`1.0`, `1/3`, `1/6`, `RadioNoise` `0x001ab300`).
pub const RADIO_CONTRIB_WEIGHTS: [f32; RADIO_MAX_NEAREST_ENEMIES] = [1.0, 1.0 / 3.0, 1.0 / 6.0];

/// Base volume argument (`0x3f4ccccd` = `0.8`) passed to `SeCallPosDistance` in Mode 1 (`0x001ab300`).
pub const RADIO_MODE1_BASE_VOLUME: f32 = 0.8;

/// Maximum discrete inventory radio volume setting (`DAT_01126900`, `0..=15`, `0x001ab300`).
pub const RADIO_MAX_VOLUME_LEVEL: u8 = 15;

/// Exact 16×4 species-to-track weight matrix dumped from `SLUS_202.28` `enemy_track$2443` (`0x002b9850`).
///
/// Indexed by `enLocalWork.kind` (`0..=15`, `enTransID` `0x00141b30` in `Enemy/en_common.c:452`):
/// - `1`: Lying Figure (`0x200`) `[1.0, 0.0, 1.0, 0.0]`
/// - `2`: Mannequin (`0x201`) `[0.0, 1.0, 1.0, 0.0]`
/// - `3`: Creeper (`0x202`) `[0.6, 0.0, 0.0, 0.0]`
/// - `4`: Pyramid Head (`0x208`) `[0.4, 0.4, 0.4, 1.0]`
/// - `5`: Twin Pyramid Head (`0x209`) `[0.4, 0.4, 0.4, 1.0]`
/// - `6`: Bubble Head Nurse (`0x207`/`0x20b`) `[1.0, 1.0, 0.0, 0.0]`
/// - `7`: Flesh Lips (`0x203`) `[0.0, 0.0, 0.0, 0.0]`
/// - `8`: Abstract Daddy (`0x204`) `[0.0, 1.0, 0.8, 0.4]`
/// - `9`: Eddie (`0x205`) `[0.0, 0.0, 0.0, 0.0]`
/// - `10`: Mandarin (`0x20a`) `[1.0, 0.8, 0.0, 0.2]`
/// - `11`: Mary Boss (`0x206`) `[0.0, 0.0, 0.0, 0.0]`
/// - `13`/`14`: `ty2`/`fly` (`0x20c`/`0x20d`) `[0.6, 0.0, 0.0, 0.0]`
/// - `15`: unused / `0` (`enTransID` `0x00141b30` / `sh2pc.exe` `FUN_004a5ba0` `0x004a5ba0` return `0` for unmapped IDs including `0x20f`)
pub const ENEMY_RADIO_TRACK_WEIGHTS: [[f32; RADIO_TRACK_COUNT]; 16] = [
    [0.0, 0.0, 0.0, 0.0], // 0: none
    [1.0, 0.0, 1.0, 0.0], // 1: Lying Figure (scu)
    [0.0, 1.0, 1.0, 0.0], // 2: Mannequin (mkn)
    [0.6, 0.0, 0.0, 0.0], // 3: Creeper (tyu)
    [0.4, 0.4, 0.4, 1.0], // 4: Pyramid Head (red)
    [0.4, 0.4, 0.4, 1.0], // 5: Twin Pyramid Head (oni)
    [1.0, 1.0, 0.0, 0.0], // 6: Bubble Head Nurse (nse)
    [0.0, 0.0, 0.0, 0.0], // 7: Flesh Lips (ike)
    [0.0, 1.0, 0.8, 0.4], // 8: Abstract Daddy (pap)
    [0.0, 0.0, 0.0, 0.0], // 9: Eddie (edb)
    [1.0, 0.8, 0.0, 0.2], // 10: Mandarin (arm)
    [0.0, 0.0, 0.0, 0.0], // 11: Mary Boss (bos)
    [0.0, 0.0, 0.0, 0.0], // 12: nik (0x421)
    [0.6, 0.0, 0.0, 0.0], // 13: ty2 (0x20c)
    [0.6, 0.0, 0.0, 0.0], // 14: fly (0x20d)
    [0.0, 0.0, 0.0, 0.0], // 15: unused / 0
];

/// Retail `sh2pc.exe` `FUN_0048c710` (`0x0048c710`, `sh2pc_00480000.c:7841`, `DAT_0088a0f8`)
/// per-species 4-track weight table (16 entries × 4 floats at `+0x00..+0x0f` of each 32-byte row).
///
/// Prototype (`SLUS_202.28` `0x002b9850`) vs Retail PC (`sh2pc.exe` `0x0088a0f8`) drift:
/// retail PC rebalanced per-species weights across all 4 static tracks and paired each track
/// with its own per-species detection radius in [`SH2PC_RADIO_TRACK_RADII_MM`].
pub const SH2PC_RADIO_TRACK_WEIGHTS: [[f32; RADIO_TRACK_COUNT]; 16] = [
    [0.0, 0.0, 0.0, 0.0], // 0: none
    [1.0, 1.0, 0.6, 0.8], // 1: Lying Figure (scu)
    [0.9, 0.8, 1.0, 0.5], // 2: Mannequin (mkn)
    [0.5, 0.2, 0.5, 0.0], // 3: Creeper (tyu)
    [0.5, 0.5, 0.5, 1.0], // 4: Pyramid Head (red)
    [0.3, 0.3, 0.3, 1.0], // 5: Twin Pyramid Head (oni)
    [1.0, 0.8, 1.0, 0.0], // 6: Bubble Head Nurse (nse)
    [0.0, 0.0, 0.0, 0.0], // 7: Flesh Lips (ike)
    [0.8, 1.0, 0.8, 0.8], // 8: Abstract Daddy (pap)
    [0.0, 0.0, 0.0, 0.0], // 9: Eddie (edb)
    [0.4, 0.8, 1.0, 0.5], // 10: Mandarin (arm)
    [0.0, 0.0, 0.0, 0.0], // 11: Mary Boss (bos)
    [0.0, 0.0, 0.0, 0.0], // 12: nik
    [0.5, 0.2, 0.5, 0.0], // 13: ty2
    [0.5, 0.2, 0.5, 0.0], // 14: fly / ty3
    [0.0, 0.0, 0.0, 0.0], // 15: unused
];

/// Retail `sh2pc.exe` `FUN_0048c710` (`0x0048c710`, `sh2pc_00480000.c:7787`, `DAT_0088a108`)
/// per-species 4-track detection radius table in world mm (16 entries × 4 floats at `+0x10..+0x1f`
/// of each 32-byte row starting at `0x0088a0f8`).
pub const SH2PC_RADIO_TRACK_RADII_MM: [[f32; RADIO_TRACK_COUNT]; 16] = [
    [0.0, 0.0, 0.0, 0.0],                     // 0: none
    [12_500.0, 5_000.0, 1_250.0, 7_500.0],    // 1: Lying Figure (scu)
    [14_000.0, 6_000.0, 1_500.0, 5_000.0],    // 2: Mannequin (mkn)
    [7_500.0, 1_500.0, 2_500.0, 0.0],         // 3: Creeper (tyu)
    [10_000.0, 9_000.0, 8_000.0, 10_000.0],   // 4: Pyramid Head (red)
    [10_000.0, 10_000.0, 10_000.0, 10_000.0], // 5: Twin Pyramid Head (oni)
    [8_000.0, 6_000.0, 4_000.0, 0.0],         // 6: Bubble Head Nurse (nse)
    [0.0, 0.0, 0.0, 0.0],                     // 7: Flesh Lips (ike)
    [15_000.0, 7_500.0, 2_500.0, 7_500.0],    // 8: Abstract Daddy (pap)
    [0.0, 0.0, 0.0, 0.0],                     // 9: Eddie (edb)
    [5_000.0, 3_500.0, 1_500.0, 5_000.0],     // 10: Mandarin (arm)
    [0.0, 0.0, 0.0, 0.0],                     // 11: Mary Boss (bos)
    [0.0, 0.0, 0.0, 0.0],                     // 12: nik
    [7_500.0, 1_500.0, 2_500.0, 0.0],         // 13: ty2
    [7_500.0, 1_500.0, 2_500.0, 0.0],         // 14: fly / ty3
    [0.0, 0.0, 0.0, 0.0],                     // 15: unused
];

/// Retail `sh2pc.exe` `FUN_0048c710` (`sh2pc_00480000.c:7831`) inner full-volume deadzone in mm.
pub const SH2PC_RADIO_INNER_DEADZONE_MM: f32 = 1.5;

/// Retail `sh2pc.exe` `FUN_0048c710` (`sh2pc_00480000.c:7826`) threat score threshold gating track 3.
pub const SH2PC_RADIO_TRACK3_THREAT_THRESHOLD: i32 = 9;

/// Retail `sh2pc.exe` `FUN_0048c710` (`sh2pc_00480000.c:7946`) per-rank geometric decay factor (`1/3`).
pub const SH2PC_RADIO_GEOMETRIC_DECAY: f32 = 1.0 / 3.0;

/// Retail `sh2pc.exe` `FUN_0048c710` (`sh2pc_00480000.c:7953`) early-out threshold for decayed contributors.
pub const SH2PC_RADIO_DECAY_CUTOFF: f32 = 0.1;

/// Evaluates the retail `sh2pc.exe` `FUN_0048c710` (`sh2pc_00480000.c:7794-7812`) threat score
/// contribution when an enemy is strictly within `SH2PC_RADIO_TRACK_RADII_MM[kind][3]`.
#[inline]
pub const fn sh2pc_radio_threat_score(kind: u8) -> i32 {
    match kind {
        1 | 2 | 6 => 3,
        3 | 13 | 14 => 1,
        4 | 5 => 9,
        8 | 10 => 6,
        _ => 0,
    }
}

/// Evaluates the retail `sh2pc.exe` `FUN_0048c710` (`sh2pc_00480000.c:7831-7838`) cosine proximity
/// curve with `1.5` mm inner deadzone and per-species/per-track `max_radius_mm`.
#[inline]
pub fn sh2pc_radio_cosine_attenuation(dist_mm: f32, max_radius_mm: f32) -> f32 {
    if !dist_mm.is_finite()
        || !max_radius_mm.is_finite()
        || dist_mm < 0.0
        || dist_mm > max_radius_mm
    {
        return 0.0;
    }
    if dist_mm < SH2PC_RADIO_INNER_DEADZONE_MM {
        return 1.0;
    }
    let denom = max_radius_mm - SH2PC_RADIO_INNER_DEADZONE_MM;
    if denom <= 0.0 {
        return 0.0;
    }
    (((dist_mm - SH2PC_RADIO_INNER_DEADZONE_MM) / denom) * FRAC_PI_2)
        .cos()
        .max(0.0)
}

/// Converts an [`EnemySpecies`] to its `SLUS_202.28` `enLocalWork.kind` index (`0..=14`,
/// `enTransID` `0x00141b30` in `Enemy/en_common.c:452`, retail `sh2pc.exe` `FUN_004a5ba0`
/// `0x004a5ba0` in `sh2pc_004a0000.c:3644`). Unmapped IDs including `RoomInsect` (`0x20f`)
/// return `0`.
#[inline]
pub const fn en_kind_from_species(species: EnemySpecies) -> u8 {
    match species {
        EnemySpecies::LyingFigure => 1,
        EnemySpecies::Mannequin => 2,
        EnemySpecies::Creeper => 3,
        EnemySpecies::PyramidHead => 4,
        EnemySpecies::TwinPyramidHead => 5,
        EnemySpecies::Nurse => 6,
        EnemySpecies::FleshLips => 7,
        EnemySpecies::AbstractDaddy => 8,
        EnemySpecies::Eddie => 9,
        EnemySpecies::Mandarin => 10,
        EnemySpecies::MaryBoss => 11,
        EnemySpecies::RoomInsect => 0,
    }
}

/// Evaluates `enSetRadioVolume` (`SLUS_202.28` `Enemy/en_common.c` `0x00143890`), returning the
/// enemy's base radio emission weight (`0.0`, `0.5`, `0.8`, or `1.0`).
#[inline]
pub const fn en_set_radio_volume(
    kind: u8,
    sub_type: u8,
    is_spawned_active: bool,
    is_alive: bool,
    is_downed: bool,
) -> f32 {
    if kind == 4 && sub_type == 0 {
        1.0
    } else if matches!(kind, 4 | 5 | 7 | 9 | 11) || !is_spawned_active || !is_alive {
        0.0
    } else if kind == 3 {
        0.5
    } else if !is_downed {
        1.0
    } else {
        0.8
    }
}

/// Evaluates the `RadioNoise` (`0x001ab300`) cosine proximity curve:
/// `cos((dist_mm / 12500.0) * PI / 2.0)` for `dist_mm <= 12500.0`, and `0.0` beyond.
#[inline]
pub fn radio_cosine_attenuation(dist_mm: f32) -> f32 {
    if !dist_mm.is_finite() || dist_mm < 0.0 || dist_mm > RADIO_MAX_DISTANCE_MM {
        return 0.0;
    }
    ((dist_mm / RADIO_MAX_DISTANCE_MM) * FRAC_PI_2)
        .cos()
        .max(0.0)
}

/// Pocket Radio operating mode (`_DAT_011263e4` in `RadioNoise` `0x001ab300`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum RadioMode {
    /// Mode 0: Normal carried pocket radio with 3-nearest-enemy 4-track mixing.
    #[default]
    Normal,
    /// Mode 1: World-positioned radio emitting directional static before pickup (`0x001ab300` lines 1871–1892).
    WorldSource { position: [f32; 3] },
    /// Mode 2: Forced static driven by the first active enemy's `radio_vol` (or `1.0` if none, lines 1893–1923).
    ForcedFirstEnemy,
}

/// Menu/map ducking state for `RadioNoise` (`_DAT_01126408`, `0x001ab300` lines 2024–2046).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RadioDuckingState {
    /// Normal gameplay: `1.0`.
    #[default]
    Normal,
    /// Pause or Inventory (`game_mode == 5 || game_mode == 6`): `0.5`.
    PauseOrInventory,
    /// Map or Memo (`game_mode == 8`): `0.25`.
    MapOrMemo,
}

impl RadioDuckingState {
    /// Target `_DAT_01126408` ducking scalar (`0x001ab300` lines 2024–2032).
    #[inline]
    pub const fn target_gain(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::PauseOrInventory => 0.5,
            Self::MapOrMemo => 0.25,
        }
    }
}

/// Per-enemy snapshot fed into [`super::RadioNoiseEvaluator::evaluate_enemies`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioEnemySample {
    /// `enLocalWork.kind` (`0..=14`, see [`en_kind_from_species`]).
    pub kind: u8,
    /// Enemy subtype (`enLocalWork[5]`, used by Pyramid Head `kind == 4`).
    pub sub_type: u8,
    /// World-space position in mm (`chara + 0x20`).
    pub position: Vec3,
    /// `*(u32*)(chara + 0x04) & 0x10 != 0` (active in scene).
    pub is_spawned_active: bool,
    /// `*(u32*)(chara + 0x170) & 0x01 != 0` (alive).
    pub is_alive: bool,
    /// `*(u32*)(chara + 0x170) & 0x04 != 0` (downed / lying).
    pub is_downed: bool,
}

impl RadioEnemySample {
    /// Computes this enemy's `enSetRadioVolume` (`0x00143890`) emission weight.
    #[inline]
    pub const fn radio_volume(&self) -> f32 {
        en_set_radio_volume(
            self.kind,
            self.sub_type,
            self.is_spawned_active,
            self.is_alive,
            self.is_downed,
        )
    }
}

/// Complete 4-track Pocket Radio output computed every tick (`RadioNoise` `0x001ab300`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RadioStaticOutput {
    /// Whether the pocket radio is powered on and active (`_radio != 0`).
    pub is_active: bool,
    /// Combined scalar loudness summary (`max(track_volumes) * global_duck * (vol_level / 15.0)`) in `[0.0, 1.0]`.
    pub volume: f32,
    /// 3D Euclidean distance to the nearest emitting enemy in world mm (`f32::MAX` if none).
    pub nearest_enemy_dist: f32,
    /// Ramped per-track linear volumes (`0x011263e8`, `[0.0, 1.0]`).
    pub track_volumes: [f32; RADIO_TRACK_COUNT],
    /// Ramped per-track normalized pans (`0x011263f8`, `[-1.0, 1.0]`).
    pub track_pans: [f32; RADIO_TRACK_COUNT],
    /// Integer `shSdRadio` track volumes (`0..=127`).
    pub driver_volumes: [u8; RADIO_TRACK_COUNT],
    /// Integer `shSdRadio` track pans (`0..=127`, `64` = center).
    pub driver_pans: [u8; RADIO_TRACK_COUNT],
    /// Ramped menu/map ducking scalar (`_DAT_01126408`, `[0.0, 1.0]`).
    pub global_duck: f32,
}

impl Default for RadioStaticOutput {
    fn default() -> Self {
        Self {
            is_active: false,
            volume: 0.0,
            nearest_enemy_dist: f32::MAX,
            track_volumes: [0.0; RADIO_TRACK_COUNT],
            track_pans: [0.0; RADIO_TRACK_COUNT],
            driver_volumes: [0; RADIO_TRACK_COUNT],
            driver_pans: [64; RADIO_TRACK_COUNT],
            global_duck: 0.0,
        }
    }
}
