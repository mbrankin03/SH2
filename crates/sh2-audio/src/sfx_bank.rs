//! Authentic Silent Hill 2 Sound Effect (`sddata.bin` & `voice.afs`) lookup tables,
//! 3D distance attenuation (`SeCallPosDistanceF`), and surface material footstep selection (`SH2-61`).
//!
//! # Ground Truth & Binary References (`sh2pc.exe` & SH2:EE)
//! - **`0x8A1ED8` (`0x29A` = 666 pairs)**: Maps game `se_id` (`u32`, e.g., `10000` / `0x2710` = `CursorMove`,
//!   `11048` / `0x2B28` = `HandgunShot`) to internal driver ID `5000..=5698`, which indexes `0x8A67E0` (`SFXAddrMap`)
//!   to resolve one of the **417** RIFF WAVE entries in `sddata.bin` (`0..=416`).
//! - **`0x8A1E58` (32 pairs)**: Maps streamed SFX `se_id` (`u16`, e.g., `10051` / `0x2743` = `SavePoint`,
//!   `15002` / `0x3A9A` = `GameStartTransition`) to `voice.afs` entry indices (`raw_id - 0xEA60`, `13..=44`).
//! - **`0x5155E0` (`SeCallPosDistanceF`) & `0x517D10`**:
//!   - Inner full-volume radius: $d_{\min} = 1500.0$ world units (`0x6B4038`).
//!   - Outer cutoff radius: $d_{\max} = 11500.0$ world units ($d_{\max} - d_{\min} = 10000.0$, `0x6C5D70` = $10^{-4}$).
//!   - Normalized PS2 sound driver attenuation gain curve (`SH2-61`):
//!     $$\text{gain}(v) = \min\left(1.0,\ \frac{25.5}{255.0 \cdot (1.0 - v)}\right)$$
//! - **SH2:EE Patches**:
//!   - `Patches/PlayWavSound.cpp`: Standalone `.wav` cues in `sh2e/sound/extra/` (`g_start.wav`, `save_sound.wav`,
//!     `flashlight_on.wav`, `flashlight_off.wav`).
//!   - `Patches/ChainsawSoundFix.cpp`: Chainsaw idle loop (`0x1642` -> `sddata[352]`) skips initial rev transient on loop.

use glam::Vec3;
use sh2_core::SurfaceMaterial;

/// Total number of RIFF WAVE entries inside retail `data/sound/sddata.bin` and SH2:EE `sh2e/sound/sddata.bin`.
pub const SDDATA_ENTRY_COUNT: usize = 417;

/// Inner 3D distance radius (in world units, mm) within which positional sound effects play at unattenuated full volume (`0x5155E0` / `0x6B4038`).
pub const SFX_MIN_DISTANCE: f32 = 1500.0;

/// Outer 3D distance cutoff radius (in world units, mm) beyond which positional sound effects attenuate to silence (`0x5155E0`).
pub const SFX_MAX_DISTANCE: f32 = 11500.0;

/// Distance attenuation span ($d_{\max} - d_{\min} = 10000.0$ world units, reciprocal `0.0001` at `0x6C5D70`).
pub const SFX_DISTANCE_SPAN: f32 = SFX_MAX_DISTANCE - SFX_MIN_DISTANCE;

/// Evaluates the authentic PS2/PC sound driver attenuation gain formula (`SH2-61`):
///
/// $$\text{gain}(v) = \min\left(1.0,\ \frac{25.5}{255.0 \cdot (1.0 - v)}\right)$$
///
/// where $v \in [0.0, 1.0]$ is the normalized proximity/volume parameter ($v = 0.0$ at maximum distance cutoff,
/// $v \ge 0.9$ within the inner full-volume zone where $\text{gain} = 1.0$).
///
/// Note: At $v = 0.0$, the raw hyperbola evaluates to $25.5 / 255.0 = 0.10$. For callers that require
/// complete silence at the outer boundary $v \le 0.0$, see [`compute_se_call_pos_distance_f`].
#[inline]
pub fn compute_ps2_sfx_attenuation_gain(v: f32) -> f32 {
    let v_clamped = v.clamp(0.0, 1.0);
    let denom = 255.0 * (1.0 - v_clamped);
    if denom <= 25.5 {
        1.0
    } else {
        (25.5 / denom).clamp(0.0, 1.0)
    }
}

/// Computes the normalized proximity parameter $v \in [0.0, 1.0]$ from 3D world-space distance $d$
/// using `SeCallPosDistanceF` (`0x5155E0`) inner/outer radii ($d_{\min} = 1500.0$, $d_{\max} = 11500.0$):
///
/// $$v(d) = 1.0 - \text{clamp}\left(\frac{d - d_{\min}}{d_{\max} - d_{\min}},\ 0.0,\ 1.0\right)$$
#[inline]
pub fn compute_sfx_distance_param_v(distance: f32, min_dist: f32, max_dist: f32) -> f32 {
    let span = (max_dist - min_dist).max(1.0);
    let t = ((distance - min_dist) / span).clamp(0.0, 1.0);
    1.0 - t
}

/// Complete 3D positional SFX evaluation result (`SeCallPosDistanceF` at `0x5155E0` & `0x517D10`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SfxSpatialEval {
    /// 3D Euclidean distance between listener and emitter in world units (mm).
    pub distance: f32,
    /// Normalized proximity parameter $v \in [0.0, 1.0]$ ($1.0$ at $\le 1500$, $0.0$ at $\ge 11500$).
    pub proximity_v: f32,
    /// Normalized PS2 hyperbolic attenuation curve value `compute_ps2_sfx_attenuation_gain(v)`.
    pub ps2_curve_gain: f32,
    /// Final effective linear amplitude gain (`0.0..=1.0`, reaching `0.0` outside `SFX_MAX_DISTANCE`).
    pub effective_gain: f32,
    /// Integer byte attenuation `0..=255` passed to `shSdCall` (`0` = loudest, `255` = silent).
    pub driver_attenuation_byte: u8,
    /// Stereo pan in `[-1.0, 1.0]` (`-1.0` = full left, `0.0` = center, `+1.0` = full right).
    pub stereo_pan: f32,
}

/// Evaluates `SeCallPosDistanceF` (`0x5155E0`) and stereo listener panning (`0x517D10`) for a 3D sound emitter.
///
/// - `listener_pos`: Player / camera position in world space.
/// - `listener_yaw`: Listener facing yaw in radians (`0.0` = $+Z$, matching `SimulationCore`).
/// - `source_pos`: 3D world position of the sound effect emitter.
/// - `base_volume`: Caller volume multiplier in `[0.0, 1.0]`.
pub fn compute_se_call_pos_distance_f(
    listener_pos: Vec3,
    listener_yaw: f32,
    source_pos: Vec3,
    base_volume: f32,
) -> SfxSpatialEval {
    let delta = source_pos - listener_pos;
    let distance = delta.length();
    let v_dist = compute_sfx_distance_param_v(distance, SFX_MIN_DISTANCE, SFX_MAX_DISTANCE);
    let v = (v_dist * base_volume.clamp(0.0, 1.0)).clamp(0.0, 1.0);

    // Raw `0x5155E0` byte attenuation: `att = 255.0 * (1.0 - v)`
    let att_f = (255.0 * (1.0 - v)).clamp(0.0, 255.0);
    let driver_attenuation_byte = att_f.round() as u8;

    let ps2_curve_gain = compute_ps2_sfx_attenuation_gain(v);
    let effective_gain = if distance >= SFX_MAX_DISTANCE || base_volume <= 0.0 {
        0.0
    } else {
        (ps2_curve_gain * v).clamp(0.0, 1.0)
    };

    // Compute horizontal azimuth angle relative to listener's right ear vector
    let horiz_len_sq = delta.x * delta.x + delta.z * delta.z;
    let stereo_pan = if horiz_len_sq > 1.0 {
        let inv_len = horiz_len_sq.sqrt().recip();
        let dir_x = delta.x * inv_len;
        let dir_z = delta.z * inv_len;
        // Listener forward is (sin(yaw), 0, cos(yaw)); right ear in RH Y-down is (-cos(yaw), 0, sin(yaw))
        let (sin_y, cos_y) = listener_yaw.sin_cos();
        let right_x = -cos_y;
        let right_z = sin_y;
        (dir_x * right_x + dir_z * right_z).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    SfxSpatialEval {
        distance,
        proximity_v: v,
        ps2_curve_gain,
        effective_gain,
        driver_attenuation_byte,
        stereo_pan,
    }
}

/// Returns the pair of `(left_step_sddata_idx, right_step_sddata_idx)` in `sddata.bin` (`0..417`)
/// for a given character (`is_maria`), [`SurfaceMaterial`], and gait (`is_running`).
///
/// Ground-truth `sh2pc.exe` binary mapping verified against `.cld` room floor geometry across
/// `ap` (Apartments), `bw` (Pete's Bowl-O-Rama), `ca..cd` (Town / Forest / Rosewater Park),
/// `hp` (Brookhaven), `ps` (Prison), `rr` (Lakeview Hotel), and `ma` (Baldwin Mansion):
/// - **James (`0x54A9A0` / `0x54AFC1`)**:
///   - `Concrete` (`.cld` `0, 12`): `se_id = 11001` (`0x2AF9`) -> `sddata[147]` & `se_id = 18803` (`0x4973`) -> `sddata[161]`
///   - `Wood` (`.cld` `7` — Baldwin Mansion `ma`, Pete's Bowling Lanes `bw01`, Rosewater Pier `cd01/cd05`):
///     `se_id = 11005` (`0x2AFD`) -> `sddata[151]` & `se_id = 19907` (`0x4DC3`) -> `sddata[95]` (`Hotel Stairway Footstep`)
///   - `Metal` (`.cld` `5, 9, 11`): `se_id = 11000` (`0x2AF8`) -> `sddata[146]` & `se_id = 11006` (`0x2AFE`) -> `sddata[143]`
///   - `Carpet` (`.cld` `8` — Lakeview Hotel `rr` & Wood Side Apartments `ap` carpeted corridors):
///     `se_id = 11009` (`0x2B01`) -> `sddata[144]` & `se_id = 11054` (`0x2B2E`) -> `sddata[145]`
///   - `Water` (`.cld` `6`): `se_id = 11003` (`0x2AFB`) -> `sddata[148]` (walk) and `sddata[83, 84]` (`0x4978, 0x4977` water splash)
///   - `Grass` (`.cld` `2`): `se_id = 11002` (`0x2AFA`) -> `sddata[142]` (`James footstep (grass)`)
///   - `Gravel` (`.cld` `1, 3, 4`): `se_id = 11004` (`0x2AFC`) -> `sddata[150]` & `se_id = 11007` (`0x2AFF`) -> `sddata[149]` (dirt/gravel/sand crunch)
///   - `Blood` / Wet / Glass (`.cld` `10`): `se_id = 11008` (`0x2B00`) -> `sddata[141]` & `se_id = 11003` (`0x2AFB`) -> `sddata[148]`
/// - **Maria (`0x5495C0` / `0x549618`)**:
///   - `se_id = 19031..=19038` (`0x4A57..=0x4A5E`) -> `sddata[177..=184]`
#[inline]
pub const fn footstep_sddata_indices(
    is_maria: bool,
    material: SurfaceMaterial,
    is_running: bool,
) -> (usize, usize) {
    if is_maria {
        match material {
            SurfaceMaterial::Concrete => (177, 177), // 0x4A57 (Maria default concrete heel)
            SurfaceMaterial::Wood => (183, 183),     // 0x4A5D (Maria wood floorboard heel, .cld 7)
            SurfaceMaterial::Metal => {
                if is_running {
                    (182, 184) // 0x4A5B, 0x4A5C (Maria metal grating / solid metal heel)
                } else {
                    (182, 182) // 0x4A5B (Maria metal heel)
                }
            }
            SurfaceMaterial::Carpet => (180, 180), // 0x4A5E (Maria carpet / soft step, .cld 8)
            SurfaceMaterial::Water => {
                if is_running {
                    (83, 84) // 0x4978, 0x4977 (Water splash L/R)
                } else {
                    (181, 181) // 0x4A59 (Maria water step)
                }
            }
            SurfaceMaterial::Grass => (179, 179), // 0x4A58 (Maria grass step)
            SurfaceMaterial::Gravel => (178, 178), // 0x4A5A (Maria dirt/gravel step)
            SurfaceMaterial::Blood => (181, 184), // 0x4A59, 0x4A5C (Maria wet / glass step)
        }
    } else {
        match material {
            SurfaceMaterial::Concrete => {
                if is_running {
                    (147, 161) // 11001 (Concrete) & 18803 (Stone / tile hard floor)
                } else {
                    (147, 147) // 11001 (0x2AF9: James default concrete/stone step)
                }
            }
            SurfaceMaterial::Wood => {
                if is_running {
                    (151, 95) // 11005 (0x2AFD: Wood floorboards) & 19907 (0x4DC3: Hotel Stairway wood step)
                } else {
                    (95, 151) // 19907 (0x4DC3: Hollow wood step) & 11005 (0x2AFD: Wood floorboards)
                }
            }
            SurfaceMaterial::Metal => {
                if is_running {
                    (146, 143) // 11000 (Metal grating) & 11006 (Solid metal)
                } else {
                    (146, 146) // 11000 (0x2AF8: James metal grating step)
                }
            }
            SurfaceMaterial::Carpet => {
                if is_running {
                    (144, 145) // 11009 (0x2B01: Carpeted corridor) & 11054 (0x2B2E: Soft rug step)
                } else {
                    (144, 144) // 11009 (0x2B01: James carpet / rug step)
                }
            }
            SurfaceMaterial::Water => {
                if is_running {
                    (83, 84) // 0x4978, 0x4977 (Footstep water splash L/R)
                } else {
                    (148, 83) // 11003 (0x2AFB: Water puddle step) & 0x4978 (Footstep water)
                }
            }
            SurfaceMaterial::Grass => (142, 142), // 11002 (0x2AFA: James footstep grass)
            SurfaceMaterial::Gravel => {
                if is_running {
                    (150, 149) // 11004 (0x2AFC: Dirt/gravel) & 11007 (0x2AFF: Coarse gravel/sand)
                } else {
                    (150, 150) // 11004 (0x2AFC: James dirt / gravel crunch)
                }
            }
            SurfaceMaterial::Blood => {
                if is_running {
                    (141, 148) // 11008 (0x2B00: Broken glass / sharp wet step) & 11003 (Wet step)
                } else {
                    (148, 141) // 11003 (Wet step) & 11008 (Glass / visceral step)
                }
            }
        }
    }
}

/// Resolves a game `se_id` (`u32`) to its 0-based `sddata.bin` waveform index (`0..=416`),
/// matching `sh2pc.exe` `0x5147E0` / `0x5148B0` lookup across `0x8A1ED8` (666 entries) and `0x8A67E0` (`SFXAddrMap`).
pub fn resolve_se_id_to_sddata_index(se_id: u32) -> Option<usize> {
    // Also support direct internal IDs `5000..=5698` (`0x1388..=0x1642`, e.g. chainsaw idle loop `0x1642` -> 352)
    if se_id == 0x1642 {
        return Some(352);
    }
    SE_ID_TO_SDDATA_INDEX
        .iter()
        .find(|&&(id, _)| id as u32 == se_id)
        .map(|&(_, idx)| idx as usize)
}

/// Resolves a streamed AFS sound effect `se_id` (`u32`, e.g., `10051` / `0x2743` = `SavePoint`,
/// `15002` / `0x3A9A` = `GameStartTransition`) to its 0-based entry index in `sound/adx/voice/voice.afs` (`13..=44`),
/// matching `sh2pc.exe` `0x5148D3` (`0x8A1E58` table, `raw_id - 0xEA60`).
pub fn resolve_se_id_to_voice_afs_index(se_id: u32) -> Option<usize> {
    SE_ID_TO_VOICE_AFS_INDEX
        .iter()
        .find(|&&(id, _)| id as u32 == se_id)
        .map(|&(_, idx)| idx as usize)
}

/// Authentic 32-entry `voice.afs` streamed sound effect table extracted from `sh2pc.exe` at `0x8A1E58` (`raw - 0xEA60`).
pub const SE_ID_TO_VOICE_AFS_INDEX: [(u16, u8); 32] = [
    (0x4DB8, 13),
    (0x44C6, 14),
    (0x44CB, 15),
    (0x2B25, 16),
    (0x2B24, 17),
    (0x4268, 18),
    (0x3E83, 19),
    (0x3E85, 20),
    (0x4A43, 21),
    (0x465A, 22),
    (0x2742, 23),
    (0x3A9A, 24), // 15002: GameStartTransition (`g_start.wav`)
    (0x3E90, 25),
    (0x4278, 26),
    (0x4682, 27),
    (0x4984, 28),
    (0x4654, 29),
    (0x3CF0, 30),
    (0x49B2, 31),
    (0x3E95, 32),
    (0x4269, 33),
    (0x2B22, 34),
    (0x2743, 35), // 10051: SavePoint chime (`save_sound.wav`)
    (0x2716, 36), // 10006: Menu subscreen transition
    (0x2B27, 37),
    (0x427B, 38),
    (0x3E8C, 39),
    (0x4981, 40),
    (0x42CA, 41),
    (0x4982, 42),
    (0x427C, 43),
    (0x3E80, 44),
];

/// Authentic 666-entry `(se_id, sddata_index)` lookup table extracted 1:1 from `sh2pc.exe` (`0x8A1ED8` -> `0x8A67E0`).
pub const SE_ID_TO_SDDATA_INDEX: [(u16, u16); 666] = [
    (0x465F, 0),
    (0x4651, 1),
    (0x4650, 2),
    (0x4075, 3),
    (0x4077, 4),
    (0x4076, 5),
    (0x4074, 6),
    (0x3E8F, 7),
    (0x3E8E, 8),
    (0x3E8D, 9),
    (0x2FDC, 10),
    (0x2FDD, 11),
    (0x2FDE, 12),
    (0x2FDF, 13),
    (0x2FE0, 14),
    (0x2FE1, 15),
    (0x2FDA, 16),
    (0x2FDB, 17),
    (0x2FE2, 18),
    (0x2FE3, 19),
    (0x2FE4, 20),
    (0x2FE5, 21),
    (0x445C, 22),
    (0x4A55, 23),
    (0x4079, 24),
    (0x4078, 25),
    (0x2B30, 26),
    (0x2B26, 27),
    (0x426B, 28),
    (0x3E8B, 29),
    (0x3E81, 30),
    (0x3E82, 31),
    (0x3E84, 32),
    (0x4971, 33),
    (0x4970, 34),
    (0x4972, 35),
    (0x44C5, 36),
    (0x4A4A, 37),
    (0x4DC5, 38),
    (0x4DC2, 39),
    (0x4270, 40),
    (0x2B33, 41),
    (0x2B32, 42),
    (0x2B35, 43),
    (0x2B34, 44),
    (0x2B31, 45),
    (0x4A41, 46),
    (0x4A40, 47),
    (0x4A3F, 48),
    (0x4A3E, 49),
    (0x4A3D, 50),
    (0x4A52, 51),
    (0x4A51, 52),
    (0x4A53, 53),
    (0x4A50, 54),
    (0x4A4F, 55),
    (0x2B38, 56),
    (0x2B37, 57),
    (0x2B3A, 58),
    (0x2B39, 59),
    (0x2B36, 60),
    (0x3E86, 61),
    (0x4A3A, 62),
    (0x4A3B, 63),
    (0x4A3C, 64),
    (0x4A39, 65),
    (0x4A38, 66),
    (0x471F, 67),
    (0x471B, 68),
    (0x471C, 69),
    (0x471D, 70),
    (0x471E, 71),
    (0x4718, 72),
    (0x4719, 73),
    (0x471A, 74),
    (0x4A54, 75),
    (0x4DB9, 76),
    (0x4A42, 77),
    (0x4DBC, 78),
    (0x4A44, 79),
    (0x2B1E, 80),
    (0x465C, 81),
    (0x44C4, 82),
    (0x4978, 83),
    (0x4977, 84),
    (0x2B19, 85),
    (0x2B13, 86),
    (0x2B1F, 87),
    (0x4A47, 88),
    (0x4DBB, 89),
    (0x4A48, 90),
    (0x465B, 91),
    (0x4983, 92),
    (0x3A99, 93),
    (0x3A98, 94),
    (0x4DC3, 95),
    (0x497F, 96),
    (0x484C, 97),
    (0x484D, 98),
    (0x484E, 99),
    (0x484F, 100),
    (0x4845, 101),
    (0x4846, 102),
    (0x4847, 103),
    (0x4844, 104),
    (0x4850, 105),
    (0x4851, 106),
    (0x4852, 107),
    (0x4853, 108),
    (0x4854, 109),
    (0x4855, 110),
    (0x4848, 111),
    (0x4849, 112),
    (0x484A, 113),
    (0x484B, 114),
    (0x4657, 115),
    (0x4653, 116),
    (0x4655, 117),
    (0x4656, 118),
    (0x44C7, 119),
    (0x2B21, 120),
    (0x4980, 121),
    (0x2B3D, 122),
    (0x2B09, 123),
    (0x2B3E, 124),
    (0x2B3B, 125),
    (0x2B02, 126),
    (0x2B03, 127),
    (0x2B04, 128),
    (0x2B05, 129),
    (0x2B06, 130),
    (0x2B07, 131),
    (0x2B08, 132),
    (0x2B0A, 133),
    (0x2B0B, 134),
    (0x2B0C, 135),
    (0x2B0D, 136),
    (0x2B0E, 137),
    (0x2B0F, 138),
    (0x2B10, 139),
    (0x2B11, 140),
    (0x2B00, 141),
    (0x2AFA, 142),
    (0x2AFE, 143),
    (0x2B01, 144),
    (0x2B2E, 145),
    (0x2AF8, 146),
    (0x2AF9, 147),
    (0x2AFB, 148),
    (0x2AFF, 149),
    (0x2AFC, 150),
    (0x2AFD, 151),
    (0x4DC4, 152),
    (0x426C, 153),
    (0x4A4B, 154),
    (0x3E87, 155),
    (0x3E88, 156),
    (0x3E89, 157),
    (0x3E8A, 158),
    (0x3CF2, 159),
    (0x3CF1, 160),
    (0x4973, 161),
    (0x3CF3, 162),
    (0x426E, 163),
    (0x466E, 164),
    (0x466D, 165),
    (0x30D4, 166),
    (0x30D5, 167),
    (0x30D6, 168),
    (0x30D7, 169),
    (0x30D8, 170),
    (0x30D9, 171),
    (0x30DA, 172),
    (0x30DB, 173),
    (0x30DC, 174),
    (0x2B17, 175),
    (0x4277, 176),
    (0x4A57, 177),
    (0x4A5A, 178),
    (0x4A58, 179),
    (0x4A5E, 180),
    (0x4A59, 181),
    (0x4A5B, 182),
    (0x4A5D, 183),
    (0x4A5C, 184),
    (0x4A5F, 185),
    (0x4A60, 186),
    (0x4A61, 187),
    (0x49AA, 188),
    (0x49AB, 189),
    (0x49AC, 190),
    (0x49AD, 191),
    (0x49A2, 192),
    (0x49A3, 193),
    (0x49AE, 194),
    (0x49AF, 195),
    (0x49B0, 196),
    (0x49B1, 197),
    (0x49A8, 198),
    (0x49A9, 199),
    (0x49A4, 200),
    (0x49A5, 201),
    (0x49A6, 202),
    (0x49A7, 203),
    (0x426D, 204),
    (0x4274, 205),
    (0x4272, 206),
    (0x4271, 207),
    (0x4273, 208),
    (0x2745, 209),
    (0x2744, 210),
    (0x2EF3, 211),
    (0x2EEF, 212),
    (0x2EF0, 213),
    (0x2EF1, 214),
    (0x2EF2, 215),
    (0x2EE9, 216),
    (0x2EEA, 217),
    (0x2EEB, 218),
    (0x2EEC, 219),
    (0x2EED, 220),
    (0x2EEE, 221),
    (0x2EE1, 222),
    (0x2EE2, 223),
    (0x2EE3, 224),
    (0x2EE4, 225),
    (0x2EE5, 226),
    (0x2EE6, 227),
    (0x2EE7, 228),
    (0x2EE8, 229),
    (0x2EE0, 230),
    (0x4A4D, 231),
    (0x4720, 232),
    (0x4652, 233),
    (0x4A56, 234),
    (0x4A4C, 235),
    (0x2F51, 236),
    (0x2F52, 237),
    (0x2F53, 238),
    (0x2F54, 239),
    (0x2F44, 240),
    (0x2F46, 241),
    (0x2F47, 242),
    (0x2F48, 243),
    (0x2F49, 244),
    (0x2F4A, 245),
    (0x2F45, 246),
    (0x2F4D, 247),
    (0x2F4E, 248),
    (0x2F4F, 249),
    (0x2F50, 250),
    (0x2F4B, 251),
    (0x2F4C, 252),
    (0x4DBA, 253),
    (0x477D, 254),
    (0x477E, 255),
    (0x477F, 256),
    (0x4780, 257),
    (0x477C, 258),
    (0x4781, 259),
    (0x4782, 260),
    (0x4783, 261),
    (0x4784, 262),
    (0x4785, 263),
    (0x4786, 264),
    (0x4787, 265),
    (0x4788, 266),
    (0x426A, 267),
    (0x2B14, 268),
    (0x2B15, 269),
    (0x2B16, 270),
    (0x426F, 271),
    (0x4DB7, 272),
    (0x4DB6, 273),
    (0x2B28, 274),
    (0x2B29, 275),
    (0x2B2A, 276),
    (0x427A, 277),
    (0x4659, 278),
    (0x2B2B, 279),
    (0x2B2C, 280),
    (0x2B2D, 281),
    (0x2B3C, 282),
    (0x44C2, 283),
    (0x4A46, 284),
    (0x4A45, 285),
    (0x4DC1, 286),
    (0x2B2F, 287),
    (0x44C0, 288),
    (0x44C1, 289),
    (0x4A49, 290),
    (0x2713, 291),
    (0x2719, 292),
    (0x2710, 293),
    (0x2711, 294),
    (0x2715, 295),
    (0x2714, 296),
    (0x2712, 297),
    (0x2FB0, 298),
    (0x2FB1, 299),
    (0x2FB2, 300),
    (0x2FB3, 301),
    (0x2FB4, 302),
    (0x2FB5, 303),
    (0x2FA8, 304),
    (0x2FA9, 305),
    (0x2FAA, 306),
    (0x2FAB, 307),
    (0x2FAC, 308),
    (0x2FAD, 309),
    (0x2FAE, 310),
    (0x2FAF, 311),
    (0x4A4E, 312),
    (0x2B1C, 313),
    (0x2B1A, 314),
    (0x2B1B, 315),
    (0x427D, 316),
    (0x44C3, 317),
    (0x3EF2, 318),
    (0x3EF3, 319),
    (0x3EF4, 320),
    (0x3EF5, 321),
    (0x3EE4, 322),
    (0x3EE5, 323),
    (0x3EE6, 324),
    (0x3EE7, 325),
    (0x3EE8, 326),
    (0x3EE9, 327),
    (0x3EF1, 328),
    (0x3EEE, 329),
    (0x3EF8, 330),
    (0x3EF9, 331),
    (0x3EFA, 332),
    (0x3EFB, 333),
    (0x3EF0, 334),
    (0x3EF6, 335),
    (0x3EF7, 336),
    (0x3EEA, 337),
    (0x3EEB, 338),
    (0x3E94, 339),
    (0x3EEF, 340),
    (0x3EED, 341),
    (0x3EEC, 342),
    (0x44C9, 343),
    (0x44CA, 344),
    (0x4658, 345),
    (0x2B18, 346),
    (0x2B1D, 347),
    (0x2B20, 348),
    (0x2B23, 349),
    (0x44C6, 350),
    (0x2B25, 351),
    (0x2B24, 352),
    (0x4268, 353),
    (0x4A43, 354),
    (0x465A, 355),
    (0x4654, 356),
    (0x49B2, 357),
    (0x3E95, 358),
    (0x4269, 359),
    (0x2B27, 365),
    (0x3E80, 366),
    (0x428F, 394),
    (0x4293, 395),
    (0x428D, 396),
    (0x427E, 397),
    (0x427F, 398),
    (0x4280, 399),
    (0x4281, 400),
    (0x4282, 401),
    (0x428C, 402),
    (0x428A, 403),
    (0x428B, 404),
    (0x4283, 405),
    (0x4284, 406),
    (0x4288, 407),
    (0x4286, 408),
    (0x4287, 409),
    (0x4285, 410),
    (0x4289, 411),
    (0x428E, 412),
    (0x4291, 413),
    (0x4290, 414),
    (0x4292, 415),
    (0x9C51, 367),
    (0x9D4F, 16),
    (0x9D19, 16),
    (0x9CE1, 16),
    (0x9D50, 17),
    (0x9D1A, 17),
    (0x9CE2, 17),
    (0x9CB3, 377),
    (0x9CC7, 368),
    (0x9CEF, 369),
    (0x9CF6, 370),
    (0x9C86, 371),
    (0x9C6C, 372),
    (0x9C47, 373),
    (0x9C48, 374),
    (0x9C80, 72),
    (0x9CFE, 375),
    (0x9C8A, 376),
    (0x9C49, 378),
    (0x9D04, 97),
    (0x9D05, 104),
    (0x9D06, 105),
    (0x9D10, 379),
    (0x9D21, 380),
    (0x9D3A, 380),
    (0x9D45, 380),
    (0x9C4A, 380),
    (0x9C52, 380),
    (0x9C62, 380),
    (0x9C6D, 380),
    (0x9C59, 380),
    (0x9C94, 380),
    (0x9CA9, 380),
    (0x9CBD, 380),
    (0x9CD7, 380),
    (0x9CFF, 380),
    (0x9CE7, 380),
    (0x9CF0, 380),
    (0x9CE3, 380),
    (0x9CEC, 380),
    (0x9CF9, 380),
    (0x9D22, 381),
    (0x9D3B, 381),
    (0x9D46, 381),
    (0x9CF8, 382),
    (0x9C81, 188),
    (0x9C82, 189),
    (0x9C83, 192),
    (0x9C84, 193),
    (0x9C85, 194),
    (0x9CF7, 383),
    (0x9C78, 384),
    (0x9D23, 385),
    (0x9D34, 385),
    (0x9D47, 385),
    (0x9C41, 385),
    (0x9C4B, 385),
    (0x9C53, 385),
    (0x9C6E, 385),
    (0x9C5A, 385),
    (0x9C71, 385),
    (0x9C79, 385),
    (0x9D29, 385),
    (0x9D1B, 385),
    (0x9D07, 385),
    (0x9D11, 385),
    (0x9D00, 385),
    (0x9CE8, 385),
    (0x9CA7, 385),
    (0x9CFA, 385),
    (0x9D24, 386),
    (0x9D35, 386),
    (0x9D48, 386),
    (0x9C42, 386),
    (0x9C4C, 386),
    (0x9C54, 386),
    (0x9C6F, 386),
    (0x9C5B, 386),
    (0x9C72, 386),
    (0x9C7A, 386),
    (0x9D2A, 386),
    (0x9D1C, 386),
    (0x9D08, 386),
    (0x9D12, 386),
    (0x9D01, 386),
    (0x9CE9, 386),
    (0x9CA8, 386),
    (0x9CFB, 386),
    (0x9D3C, 240),
    (0x9D51, 240),
    (0x9C95, 240),
    (0x9CAA, 240),
    (0x9CBE, 240),
    (0x9CD8, 240),
    (0x9C8B, 240),
    (0x9C9E, 240),
    (0x9CB4, 240),
    (0x9CC8, 240),
    (0x9D3D, 242),
    (0x9D52, 242),
    (0x9C96, 242),
    (0x9CAB, 242),
    (0x9CBF, 242),
    (0x9CD9, 242),
    (0x9C8C, 242),
    (0x9C9F, 242),
    (0x9CB5, 242),
    (0x9CC9, 242),
    (0x9D3E, 251),
    (0x9D53, 251),
    (0x9C97, 251),
    (0x9CAC, 251),
    (0x9CC0, 251),
    (0x9CDA, 251),
    (0x9C8D, 251),
    (0x9CA0, 251),
    (0x9CB6, 251),
    (0x9CCA, 251),
    (0x9D09, 258),
    (0x9D02, 258),
    (0x9CEA, 258),
    (0x9CF1, 258),
    (0x9CED, 258),
    (0x9CFC, 258),
    (0x9D03, 261),
    (0x9CEB, 261),
    (0x9CF2, 261),
    (0x9CEE, 261),
    (0x9CFD, 261),
    (0x9C70, 387),
    (0x9C73, 388),
    (0x9C7B, 388),
    (0x9C87, 389),
    (0x9C88, 390),
    (0x9D25, 298),
    (0x9D36, 298),
    (0x9C43, 298),
    (0x9C4D, 298),
    (0x9C55, 298),
    (0x9C63, 298),
    (0x9C74, 298),
    (0x9C7C, 298),
    (0x9D2B, 298),
    (0x9D1D, 298),
    (0x9D26, 299),
    (0x9D37, 299),
    (0x9C44, 299),
    (0x9C4E, 299),
    (0x9C56, 299),
    (0x9C64, 299),
    (0x9C75, 299),
    (0x9C7D, 299),
    (0x9D2C, 299),
    (0x9D1E, 299),
    (0x9D27, 304),
    (0x9D38, 304),
    (0x9C45, 304),
    (0x9C4F, 304),
    (0x9C57, 304),
    (0x9C65, 304),
    (0x9C76, 304),
    (0x9C7E, 304),
    (0x9D2D, 304),
    (0x9D1F, 304),
    (0x9D28, 305),
    (0x9D39, 305),
    (0x9C46, 305),
    (0x9C50, 305),
    (0x9C58, 305),
    (0x9C66, 305),
    (0x9C77, 305),
    (0x9C7F, 305),
    (0x9D2E, 305),
    (0x9D20, 305),
    (0x9C89, 391),
    (0x9D3F, 322),
    (0x9D49, 322),
    (0x9D54, 322),
    (0x9C67, 322),
    (0x9C5C, 322),
    (0x9C98, 322),
    (0x9CAD, 322),
    (0x9CC1, 322),
    (0x9CDB, 322),
    (0x9C8E, 322),
    (0x9CA1, 322),
    (0x9CB7, 322),
    (0x9CCB, 322),
    (0x9CD1, 322),
    (0x9D2F, 322),
    (0x9D0A, 322),
    (0x9D13, 322),
    (0x9D40, 324),
    (0x9D4A, 324),
    (0x9D55, 324),
    (0x9C68, 324),
    (0x9C5D, 324),
    (0x9C99, 324),
    (0x9CAE, 324),
    (0x9CC2, 324),
    (0x9CDC, 324),
    (0x9C8F, 324),
    (0x9CA2, 324),
    (0x9CB8, 324),
    (0x9CCC, 324),
    (0x9CD2, 324),
    (0x9D30, 324),
    (0x9D0B, 324),
    (0x9D14, 324),
    (0x9D41, 326),
    (0x9D4B, 326),
    (0x9D56, 326),
    (0x9C69, 326),
    (0x9C5E, 326),
    (0x9C9A, 326),
    (0x9CAF, 326),
    (0x9CC3, 326),
    (0x9CDD, 326),
    (0x9C90, 326),
    (0x9CA3, 326),
    (0x9CB9, 326),
    (0x9CCD, 326),
    (0x9CD3, 326),
    (0x9D31, 326),
    (0x9D0C, 326),
    (0x9D15, 326),
    (0x9D42, 328),
    (0x9D4C, 328),
    (0x9D57, 328),
    (0x9C6A, 328),
    (0x9C5F, 328),
    (0x9C9B, 328),
    (0x9CB0, 328),
    (0x9CC4, 328),
    (0x9CDE, 328),
    (0x9C91, 328),
    (0x9CA4, 328),
    (0x9CBA, 328),
    (0x9CCE, 328),
    (0x9CD4, 328),
    (0x9D32, 328),
    (0x9D0D, 328),
    (0x9D16, 328),
    (0x9CF3, 328),
    (0x9CE4, 328),
    (0x9D43, 392),
    (0x9D4D, 392),
    (0x9D58, 392),
    (0x9C6B, 392),
    (0x9C60, 392),
    (0x9C9C, 392),
    (0x9CB1, 392),
    (0x9CC5, 392),
    (0x9CDF, 392),
    (0x9C92, 392),
    (0x9CA5, 392),
    (0x9CBB, 392),
    (0x9CCF, 392),
    (0x9CD5, 392),
    (0x9D33, 392),
    (0x9D0E, 392),
    (0x9D17, 392),
    (0x9CF4, 392),
    (0x9CE5, 392),
    (0x9D44, 393),
    (0x9D4E, 393),
    (0x9D59, 393),
    (0x9C61, 393),
    (0x9C9D, 393),
    (0x9CB2, 393),
    (0x9CC6, 393),
    (0x9CE0, 393),
    (0x9C93, 393),
    (0x9CA6, 393),
    (0x9CBC, 393),
    (0x9CD0, 393),
    (0x9CD6, 393),
    (0x9D0F, 393),
    (0x9D18, 393),
    (0x9CF5, 393),
    (0x9CE6, 393),
    (0x42CB, 416),
];
