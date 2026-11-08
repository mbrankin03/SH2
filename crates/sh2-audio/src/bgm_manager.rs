//! 1:1 Silent Hill 2 BGM Streaming & Multi-Channel AIX Spatial Mixing Engine (`SH2-60`).
//!
//! Reproduces Team Silent's BGM subsystem from the PS2 Debug Prototype (`SLUS_202.28`:
//! `sound/sh_sound.c` and `sound/snd_select.c`) and Retail PC Binary (`sh2pc.exe`):
//! - **`SeBgmChange`** (`0x515C20`, PS2 `0x193c20`): Resolves the active BGM page (`0..=47`)
//!   from the current stage (`0..=58`), room ID (`bgm_room_id` / `0x494DB0`), and game event flags.
//! - **`BgmPageSet`** (`0x515690`, PS2 `0x194b80`): Evaluates the dynamic situation page:
//!   - Stage `EvBgmControl` override (`4..=15`) when active.
//!   - `3` (`EnemyNear`) when an active enemy (`0x200` type, alive, `0x400` active flag) is within
//!     `6000.0` world units (`dist_xz^2 < 36,000,000.0` at `0x6C5F5C`).
//!   - `2` (`LowHealth`) when player health ratio `< 0.20` (`0x63345C`).
//!   - Stage `EvBgmControl` low-priority return (`0..=3`), or `0` (`Calm`).
//! - **`sndGetSoundAryByCharaPos`** (`0x5187E0`, PS2 `0x221540`): Evaluates up to 8 overlapping
//!   `_SOUND_DATA` spatial volume bounding boxes (with `HALF_W = 50.0` X/Z margin and `[min_y, max_y]`
//!   vertical bounds, falling back to record `0` default when no area box matches), or situation
//!   preset table `0x8A9EB8` when `situation_page >= 6`.
//! - **`SeBgmManager`** (`0x516080`, PS2 `0x194300`): Drives the 7-track fade state machine
//!   (`TR_OFF = 0`, `TR_IN = 1`, `TR_ON = 2`, `TR_OUT = 3`) with priority-based fade-in/fade-out
//!   ramps (`FADE_PRIO = [0, 2, 3, 1, 4]`, `FADE_TIME = [0.0, 3.0, 1.5, 6.0, 0.0]` seconds),
//!   3.0-second page transition crossfades (`0x5155E0`), and per-file master volume scaling (`0x8A73B4`).

use glam::Vec3;
use sh2_assets::{decode_adx, decode_aix, DecodedAdx};
use sh2_core::AudioPackSource;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::bgm_data::{
    bgm_page_entry, bgm_room_id, find_bgm_file_by_load_id, stage_glb_crd, SoundDataRecord,
    STAGE_SOUND_DATA,
};
use crate::cutscene_bgm::{master_volume_for_start_id, resolve_bgm_stream_path_for_source};
use crate::engine::{AudioEngine, AudioError};

/// Number of stereo tracks managed in lockstep by `SeBgmManager` (`0x1F837D8`, 7 tracks).
pub const BGM_MAX_TRACKS: usize = 7;

/// Maximum number of overlapping `_SOUND_DATA` bounding boxes accumulated per frame (`w_p->get_num <= 8`).
pub const MAX_MATCHED_SOUND_AREAS: usize = 8;

/// Horizontal player half-width margin (`w_p->rad = 50.0`, `0x42480000`) used in `sndGetSoundAryByCharaPos`.
pub const SOUND_AREA_HALF_WIDTH: f32 = 50.0;

/// Squared XZ distance threshold (`6000.0^2 = 36,000,000.0` at `0x6C5F5C`) for enemy proximity situation `3`.
pub const ENEMY_NEAR_DIST_SQ: f32 = 36_000_000.0;

/// Player health ratio threshold (`0.20` at `0x63345C`) for low-health situation `2`.
pub const LOW_HEALTH_RATIO_THRESHOLD: f32 = 0.20;

/// Fade priority table (`fade_prio[5]` at `0x8A7750` in `sh2pc.exe`):
/// index = fade type (`0` = off, `1` = 3.0s, `2` = 1.5s, `3` = 6.0s, `4` = instant),
/// value = priority (`0` lowest .. `4` highest).
pub const FADE_PRIO: [i32; 5] = [0, 2, 3, 1, 4];

/// Fade duration table in seconds (`track_fade_time[5]` at `0x8A7764` in `sh2pc.exe`):
/// - Type `0`: `0.0s` (inactive)
/// - Type `1`: `3.0s` (normal fade)
/// - Type `2`: `1.5s` (fast fade)
/// - Type `3`: `6.0s` (slow fade)
/// - Type `4`: `0.0s` (instant step)
pub const FADE_TIME_SECS: [f32; 5] = [0.0, 3.0, 1.5, 6.0, 0.0];

/// Situation preset table for `situation_page` `6..=15` (`0x8A9EB8` in `sh2pc.exe`).
/// Each entry specifies `(fade_in[7], fade_out[7])`.
pub const SITUATION_PRESETS_6_TO_15: [([u8; 7], [u8; 7]); 10] = [
    ([2, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0]), // Page  6: Track 0
    ([2, 2, 0, 0, 0, 0, 0], [1, 1, 0, 0, 0, 0, 0]), // Page  7: Tracks 0, 1
    ([2, 0, 2, 0, 0, 0, 0], [1, 0, 1, 0, 0, 0, 0]), // Page  8: Tracks 0, 2
    ([2, 2, 2, 0, 0, 0, 0], [1, 1, 1, 0, 0, 0, 0]), // Page  9: Tracks 0, 1, 2
    ([2, 0, 0, 2, 0, 0, 0], [1, 0, 0, 1, 0, 0, 0]), // Page 10: Tracks 0, 3
    ([2, 2, 0, 2, 0, 0, 0], [1, 1, 0, 1, 0, 0, 0]), // Page 11: Tracks 0, 1, 3
    ([2, 0, 2, 2, 0, 0, 0], [1, 0, 1, 1, 0, 0, 0]), // Page 12: Tracks 0, 2, 3
    ([2, 2, 2, 2, 0, 0, 0], [1, 1, 1, 1, 0, 0, 0]), // Page 13: Tracks 0, 1, 2, 3
    ([0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0, 0]), // Page 14: All tracks fade out
    ([1, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0]), // Page 15: Track 0 (3s fade-in)
];

/// Track fade state machine (`Track.stat` at `0x1F837D8 + i*8` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BgmTrackStatus {
    /// Track is silent (`vol == 0.0`).
    #[default]
    Off = 0,
    /// Track is ramping up toward `1.0` at `FADE_TIME_SECS[fade_in]`.
    FadingIn = 1,
    /// Track is at full volume (`vol == 1.0`).
    On = 2,
    /// Track is ramping down toward `0.0` at `FADE_TIME_SECS[fade_out]`.
    FadingOut = 3,
}

/// State of a single stereo BGM layer (`8` bytes in `sh2pc.exe` at `0x1F837D8 + i*8`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BgmTrackState {
    /// Current state (`Off`, `FadingIn`, `On`, `FadingOut`).
    pub status: BgmTrackStatus,
    /// Fade-in speed type (`1..=4`).
    pub fade_in: u8,
    /// Fade-out speed type (`1..=4`).
    pub fade_out: u8,
    /// Normalized track volume (`0.0..=1.0`).
    pub volume: f32,
}

impl Default for BgmTrackState {
    fn default() -> Self {
        Self {
            status: BgmTrackStatus::Off,
            fade_in: 0,
            fade_out: 0,
            volume: 0.0,
        }
    }
}

/// Result of evaluating `_SOUND_DATA` spatial volume areas via [`evaluate_sound_areas`] (`sndGetSoundAryByCharaPos`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundAreaEvaluation {
    /// Number of matched `_SOUND_DATA` boxes (`1..=8`), or `1` when using default record `0` or situation preset.
    pub matched_box_count: usize,
    /// Indices of matched `_SOUND_DATA` records within the stage table (empty if situation preset `>= 6`).
    pub matched_record_indices: Vec<usize>,
    /// Whether no area bounding box (`1..N`) matched and record `0` (stage default) was used.
    pub used_default_record: bool,
    /// Resolved highest-priority `(fade_in, fade_out)` command per track (`0` = track should be off).
    pub track_fades: [(u8, u8); BGM_MAX_TRACKS],
}

/// Event/story flags consulted by `SeBgmChange` (`0x515C20`) and stage `EvBgmControl` functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BgmEventFlags {
    /// `0x1F7D5E4` byte flags (stage 2/3/4 prologue & forest progression).
    pub flag_5e4: u8,
    /// `0x1F7D5E8` dword flags (apartments clock/courtyard/stairwell progression).
    pub flag_5e8: u32,
    /// `0x1F7D5EC` dword flags (Rosewater Park / Maria meeting progression).
    pub flag_5ec: u32,
    /// `0x1F7D5F0` dword flags (Town West / Heaven's Night / Bowling progression).
    pub flag_5f0: u32,
    /// `0x1F7D608` dword flags (Labyrinth / Catacombs progression).
    pub flag_608: u32,
    /// `0x1F7D60C` dword flags (Eddie boss progression).
    pub flag_60c: u32,
    /// `0x1F7D60E` byte flags (Labyrinth Angela/Abstract Daddy progression).
    pub flag_60e: u8,
    /// `0x1F7D618` dword flags (Hotel 1F music box Cinderella/Snow White/Little Mermaid).
    pub flag_618: u32,
    /// `0x1F7D61B` byte flags (Hotel 1F music box completed / staircase).
    pub flag_61b: u8,
    /// `0x1F7D61C` dword flags (Hotel Basement / 1F Dark / Final Boss progression).
    pub flag_61c: u32,
    /// `0x1F7D620` dword flags (Hotel Room 312 tape watched `0x8000` / radio / general triggers).
    pub flag_620: u32,
    /// `0x1F7D622` byte flags (Hotel Room 1F corridor trigger).
    pub flag_622: u8,
    /// `0x1F7D62C` dword flags (global BGM mute/cutscene inhibit flags `0xC00`, `0x800`).
    pub flag_62c: u32,
    /// Whether the Hotel 1F lobby music box is currently open/active (`0x1D85FA4`).
    pub hotel_music_box_active: bool,
    /// Whether an active cutscene/event flag (`0x934D94 & 0x40`) is asserted.
    pub event_mode_active: bool,
    /// Optional explicit override for `EvBgmControl` return value.
    pub ev_bgm_override: Option<u32>,
}

/// Resolves the active BGM page (`0..=47`) for the given `stage_id`, `room_id`, and `flags`.
///
/// Faithfully implements `SeBgmChange` (`0x515C20` in `sh2pc.exe`, `0x193c20` in `SLUS_202.28`).
pub fn pick_bgm_page(stage_id: u32, room_id: u32, flags: &BgmEventFlags) -> u32 {
    match stage_id {
        // Stage 2: Opening Restroom (`Stg_toilet`)
        2 => {
            if (flags.flag_5e4 & 0x80) != 0 {
                0
            } else if (flags.flag_5e4 & 0x02) == 0 {
                46 // Page 46 -> forest/bgm_114_a.adx
            } else {
                47 // Page 47 -> forest/bgm_114_b.aix
            }
        }
        // Stage 3 (`Stg_observation`) & Stage 4 (`Stg_forest`)
        3 | 4 => {
            if (flags.flag_5e4 & 0x80) != 0 || (flags.flag_62c & 0x0C00) != 0 {
                0
            } else if (flags.flag_5e4 & 0x10) == 0 {
                47 // Page 47 -> forest/bgm_114_b.aix
            } else {
                3 // Page 3 -> forest/bgm_115.aix
            }
        }
        // Stage 5: Town East (`Stg_town_east`)
        5 => 3, // Page 3 -> forest/bgm_115.aix
        // Stage 6: Wood Side Apartments 1F (`Stg_apart_e1f`)
        6 => {
            if (flags.flag_5e8 & 0x0020_0000) == 0 && room_id == 0x1E {
                7 // Page 7 -> apart/bgm_001.adx
            } else if (flags.flag_5e8 & 0x0800_0000) != 0 {
                9 // Page 9 -> apart/bgm_101.aix
            } else {
                4 // Page 4 -> apart/bgm_003.adx
            }
        }
        // Stage 7 (`Stg_apart_e2f`) & Stage 8 (`Stg_apart_e3fw`)
        7 | 8 => {
            if (flags.flag_5e8 & 0x0800_0000) != 0 {
                9 // Page 9 -> apart/bgm_101.aix
            } else {
                5 // Page 5 -> apart/bgm_118.aix
            }
        }
        // Stage 9: Wood Side Apartments 3F East (`Stg_apart_e3fe`)
        9 => {
            if (flags.flag_5e8 & 0x0800_0000) != 0 {
                9 // Page 9 -> apart/bgm_101.aix
            } else {
                6 // Page 6 -> apart/bgm_103.aix
            }
        }
        // Stage 10: Blue Creek Apartments 1F (`Stg_apart_w1f`)
        10 => {
            if (flags.flag_5ec & 0x2000) == 0 && room_id == 0x24 {
                8 // Page 8 -> apart/bgm_014.adx
            } else {
                10 // Page 10 -> apart/bgm_102.aix
            }
        }
        // Stage 11: Blue Creek Apartments 2F (`Stg_apart_w2f`)
        11 => 10, // Page 10 -> apart/bgm_102.aix
        // Stage 12: Apartment Stairwell (`Stg_apart_stair`) — Pyramid Head first boss
        12 => 12, // Page 12 -> apart/bgm_125.aix
        // Stage 13: Apartment Outro (`Stg_apart_out`)
        13 => 0,
        // Stage 14: Town West / Rosewater Park (`Stg_town_west`)
        14 => {
            if (flags.flag_62c & 0x0800) != 0 {
                0
            } else if (flags.flag_5f0 & 0x0080_0000) != 0 && (flags.flag_5f0 & 0x0100_0000) == 0 {
                13 // Page 13 -> town/bgm_005.adx (Laura on wall)
            } else {
                11 // Page 11 -> town/bgm_117.aix
            }
        }
        // Stage 15: Pete's Bowl-O-Rama (`Stg_bowling`)
        15 => 14, // Page 14 -> town/bgm_018.adx
        // Stage 16: Heaven's Night Exterior (`Stg_to_heaven`)
        16 => 11, // Page 11 -> town/bgm_117.aix
        // Stage 17: Heaven's Night Interior (`Stg_heaven_night`)
        17 => 15, // Page 15 -> town/bgm_004.adx
        // Stage 18 (`Stg_hospital_1f_f`), 19 (`2f_f`), 21 (`rf_f`)
        18 | 19 | 21 => 16, // Page 16 -> hospital/bgm_119.aix
        // Stage 20: Brookhaven Hospital 3F Front (`Stg_hospital_3f_f`)
        20 => 17, // Page 17 -> hospital/bgm_111.aix
        // Stage 22: Brookhaven Hospital 1F Nightmare (`Stg_hospital_1fw_b`)
        22 => {
            if room_id == 0x34 {
                18 // Page 18 -> hospital/bgm_123.aix
            } else if room_id == 0x30 {
                19 // Page 19 -> hospital/bgm_016.adx
            } else {
                20 // Page 20 -> hospital/bgm_105.aix
            }
        }
        // Stage 23: Brookhaven Hospital 2F Nightmare (`Stg_hospital_2f_b`)
        23 => 23, // Page 23 -> hospital/bgm_021.adx
        // Stage 24 (`3f_b`), 26 (`1fe_b`)
        24 | 26 => 20, // Page 20 -> hospital/bgm_105.aix
        // Stage 25: Brookhaven Hospital Basement Nightmare (`Stg_hospital_bf_b`)
        25 => 21, // Page 21 -> hospital/bgm_100.aix
        // Stage 27: Hospital Pass (`Stg_hospital_pass`)
        27 => 22, // Page 22 -> hospital/bgm_106.aix
        // Stage 28: Hospital Tunnel Chase (`Stg_delusion_1`)
        28 => 0,
        // Stage 29 (`Stg_delusion_2`), 30 (`Stg_society`)
        29 | 30 => 24, // Page 24 -> prison/bgm_107.aix
        // Stage 31 (`Stg_delusion_3`), 32 (`Stg_prison_n`)
        31 | 32 => 25, // Page 25 -> prison/bgm_110.aix
        // Stage 33: Toluca Prison South (`Stg_prison_s`)
        33 => 26, // Page 26 -> prison/bgm_109.aix
        // Stage 34: Labyrinth West (`Stg_labyrinth_w`)
        34 => 27, // Page 27 -> prison/bgm_116.aix
        // Stage 35: Labyrinth East (`Stg_labyrinth_e`)
        35 => {
            if room_id == 0x8A && (flags.flag_608 & 0x8000) == 0 {
                28 // Page 28 -> prison/bgm_015.adx (Maria cell)
            } else {
                27 // Page 27 -> prison/bgm_116.aix
            }
        }
        // Stage 36: Labyrinth North (`Stg_labyrinth_n`)
        36 => {
            if room_id == 0x89 || (flags.flag_60e & 0x01) != 0 {
                30 // Page 30 -> prison/bgm_020.adx
            } else {
                27 // Page 27 -> prison/bgm_116.aix
            }
        }
        // Stage 37: Eddie Boss (`Stg_eddie_boss`)
        37 => 31, // Page 31 -> prison/bgm_128.aix
        // Stage 38: Toluca Lake Rowboat (`Stg_lake`)
        38 => 32, // Page 32 -> hotel/bgm_126.adx
        // Stage 39: Lakeview Hotel Exterior / Garden (`Stg_hotel_f`)
        39 => 35, // Page 35 -> hotel/bgm_112.aix
        // Stage 40: Lakeview Hotel 1F (`Stg_hotel_1f_f`)
        40 => {
            if (flags.flag_61b & 0x01) == 0 {
                35 // Page 35 -> hotel/bgm_112.aix
            } else {
                34 // Page 34 -> hotel/bgm_017.adx
            }
        }
        // Stage 41: Lakeview Hotel 2F (`Stg_hotel_2f_f`)
        41 => 36, // Page 36 -> hotel/bgm_009.adx
        // Stage 42: Lakeview Hotel 3F (`Stg_hotel_3f_f`)
        42 => {
            if room_id == 0xA2 {
                if (flags.flag_620 & 0x8000) != 0 {
                    37 // Page 37 -> hotel/bgm_121.aix
                } else {
                    0
                }
            } else {
                36 // Page 36 -> hotel/bgm_009.adx
            }
        }
        // Stage 43 (`Stg_hotel_bf_b`) & Stage 44 (`Stg_hotel_1f_b`)
        43 | 44 => {
            if room_id == 0xAC {
                39 // Page 39 -> hotel/bgm_124.aix (Two Pyramid Heads boss)
            } else if (flags.flag_61c & 0x04) != 0 {
                44 // Page 44 -> hotel/bgm_120.aix
            } else {
                38 // Page 38 -> hotel/bgm_108.aix
            }
        }
        // Stage 45 (`Stg_hotel_2f_b`) & Stage 46 (`Stg_hotel_3f_b`)
        45 | 46 => 40, // Page 40 -> hotel/bgm_122.aix
        // Stage 47: Final Boss (`Stg_hotel_fire`)
        47 => 41, // Page 41 -> hotel/bgm_113.aix
        // Stages 48..=51: Endings (`recovery`, `maria`, `suicide`, `rebirth`)
        48..=51 => 42, // Page 42 -> end/bgm_002.adx
        // Stage 52: Dog Ending (`Stg_end_dog`)
        52 => 0,
        // Stage 53: Born From A Wish — Heaven's Night (`Stg_maria_heaven`)
        53 => 15, // Page 15 -> town/bgm_004.adx
        // Stage 54: Born From A Wish — Town (`Stg_maria_town`)
        54 => 11, // Page 11 -> town/bgm_117.aix
        // Stages 55..=57: Born From A Wish — Baldwin Mansion (`1f`, `2f`, `bf`)
        55..=57 => 45, // Page 45 -> mansion/bgm_104.aix
        // Stage 58: Born From A Wish — Blue Creek (`Stg_maria_apart`)
        58 => 10, // Page 10 -> apart/bgm_102.aix
        _ => 0,
    }
}

/// Evaluates the stage-specific `EvBgmControl` situation page override (`0..=15`),
/// reproducing the 25 stage `EvBgmControl` routines in `sh2pc.exe` (`0x5A1D00..0x574D70`).
pub fn eval_stage_ev_bgm_control(stage_id: u32, room_id: u32, flags: &BgmEventFlags) -> u32 {
    if let Some(explicit) = flags.ev_bgm_override {
        return explicit;
    }
    match stage_id {
        2 => {
            if (flags.flag_5e4 & 0x10) != 0 {
                1
            } else {
                0
            }
        }
        3 => {
            if flags.event_mode_active {
                if (flags.flag_620 & 0x20) != 0 {
                    14
                } else {
                    4
                }
            } else {
                0
            }
        }
        6 => (flags.flag_5e8 >> 21) & 1,
        7 | 8 => {
            if (flags.flag_620 & 0x1000_0000) != 0 {
                14
            } else {
                0
            }
        }
        12 => {
            if (flags.flag_620 & 0x0002_0000) != 0 && (flags.flag_620 & 0x4000) == 0 {
                1
            } else {
                0
            }
        }
        14 => {
            if (flags.flag_620 & 0x2000) != 0 {
                1
            } else if (flags.flag_620 & 0x80) != 0 {
                14
            } else {
                0
            }
        }
        15 => {
            if (flags.flag_620 & 0x0004_0000) != 0 {
                0
            } else {
                14
            }
        }
        27 => (flags.flag_620 >> 8) & 4,
        29 => {
            if flags.event_mode_active {
                14
            } else {
                0
            }
        }
        35 => {
            if (flags.flag_608 & 0x8000) != 0 {
                0
            } else if (flags.flag_608 & 0x4000) != 0 {
                14
            } else {
                4
            }
        }
        36 => (flags.flag_620 >> 10) & 4,
        37 => {
            if flags.event_mode_active {
                14
            } else if (flags.flag_60c & 0x0080_0000) != 0 && (flags.flag_60c & 0x0100_0000) == 0 {
                4
            } else if (flags.flag_60c & 0x0200_0000) != 0 && (flags.flag_60c & 0x0400_0000) == 0 {
                5
            } else {
                14
            }
        }
        40 => {
            if (flags.flag_61b & 0x01) != 0 {
                4
            } else if !flags.hotel_music_box_active {
                0
            } else {
                let mut page = if (flags.flag_618 & 0x0022_2000) != 0 {
                    7
                } else {
                    6
                };
                if (flags.flag_618 & 0x0044_4000) != 0 {
                    page += 2;
                }
                if (flags.flag_618 & 0x0088_8000) != 0 {
                    page += 4;
                }
                page
            }
        }
        42 => (flags.flag_620 >> 13) & 4,
        43 => flags.flag_61c & 4,
        44 => {
            if room_id == 0xAC {
                if (flags.flag_61c & 0x08) != 0 {
                    1
                } else {
                    3
                }
            } else {
                0
            }
        }
        48..=51 => {
            if (flags.flag_620 & 0x0008_0000) != 0 && (flags.flag_61c & 0x1000_0000) == 0 {
                4
            } else {
                14
            }
        }
        _ => 0,
    }
}

/// Evaluates the active BGM situation page (`0..=15`) for the current frame.
///
/// Faithfully implements `BgmPageSet` (`0x515690` in `sh2pc.exe`, `0x194b80` in `SLUS_202.28`):
/// 1. Calls stage `EvBgmControl`. If the result is `>= 4` (`4..=5` or `6..=15`), returns it immediately.
/// 2. Scans active enemies: if any alive enemy is within `6000.0` units in the XZ plane
///    (`dx*dx + dz*dz < 36,000,000.0`), returns `3` (Enemy Near).
/// 3. Checks player health ratio: if `player_hp_ratio < 0.20`, returns `2` (Low Health).
/// 4. Otherwise returns the stage `EvBgmControl` value (`0..=3`, where `0` is Calm).
pub fn evaluate_situation_page(
    stage_id: u32,
    room_id: u32,
    player_hp_ratio: f32,
    min_enemy_dist_xz: Option<f32>,
    flags: &BgmEventFlags,
) -> u32 {
    let ev_page = eval_stage_ev_bgm_control(stage_id, room_id, flags);
    if ev_page > 3 {
        return ev_page;
    }

    if let Some(dist_xz) = min_enemy_dist_xz {
        if dist_xz >= 0.0 && (dist_xz * dist_xz) < ENEMY_NEAR_DIST_SQ {
            return 3;
        }
    }

    if player_hp_ratio < LOW_HEALTH_RATIO_THRESHOLD {
        return 2;
    }

    ev_page
}

/// Extracts `(fade_in, fade_out)` commands for each of the 7 AIX tracks from a single `_SOUND_DATA` record
/// at the given `situation_page` (`0..=5`).
///
/// Matches the inner loop of `sndGetSoundAryByCharaPos` (`0x518945..0x518A58` in `sh2pc.exe`):
/// - If `(record.flags & (1 << situation_page)) != 0`, shifts `chanstat[track]` right by `situation_page * 5`.
/// - Otherwise uses the base calm page (`situation_page = 0`, low 5 bits of `chanstat[track]`).
/// - Within the 5-bit field:
///   - Bit `0` (`& 1`): track active (`0` -> `(0, 0)`, `1` -> active).
///   - Bits `1..=2` (`(bits >> 1) & 3`): `fade_in = val + 1` (`1..=4`).
///   - Bits `3..=4` (`(bits >> 3) & 3`): `fade_out = val + 1` (`1..=4`).
#[inline]
pub fn extract_record_track_fades(
    record: &SoundDataRecord,
    situation_page: u32,
) -> [(u8, u8); BGM_MAX_TRACKS] {
    let shift = if situation_page < 31 && (record.flags & (1i32 << situation_page)) != 0 {
        situation_page * 5
    } else {
        0
    };

    let mut out = [(0u8, 0u8); BGM_MAX_TRACKS];
    for (track_idx, &word) in record.chanstat.iter().enumerate() {
        let bits = word >> shift;
        if (bits & 1) != 0 {
            let fade_in = (((bits >> 1) & 0x3) as u8) + 1;
            let fade_out = (((bits >> 3) & 0x3) as u8) + 1;
            out[track_idx] = (fade_in, fade_out);
        }
    }
    out
}

/// Evaluates `_SOUND_DATA` spatial volume bounding boxes for `stage_id` at `player_pos` and `situation_page`.
///
/// Faithfully implements `sndGetSoundAryByCharaPos` (`0x5187E0` in `sh2pc.exe`, `0x221540` in `SLUS_202.28`)
/// and the priority reduction loop in `SeBgmManager` (`0x516372..0x5163FC`):
/// - If `situation_page >= 6`: uses [`SITUATION_PRESETS_6_TO_15`] (`0x8A9EB8`) directly (`get_num = 1`).
/// - Otherwise (`situation_page < 6`):
///   - Tests records `1..N` in `STAGE_SOUND_DATA[stage_id]` against `player_pos`:
///     - `x + 50.0 >= rec.x0` and `x - 50.0 <= rec.x2`
///     - `z + 50.0 >= rec.z2` and `z - 50.0 <= rec.z0`
///     - `y >= rec.max_y` and `y <= rec.min_y` (note: in SH2 Y-down coordinates, `max_y` is the more negative
///       ceiling/upper bound and `min_y` is the floor/lower bound; we support both `rec.max_y <= y <= rec.min_y`
///       and `rec.min_y <= y <= rec.max_y` so standard Y-up test arenas also match cleanly).
///   - Accumulates up to `MAX_MATCHED_SOUND_AREAS` (`8`) matching boxes.
///   - If `0` boxes matched, falls back to record `0` (`STAGE_SOUND_DATA[stage_id][0]`, stage default).
///   - Across all matched records, selects the `fade_in` and `fade_out` types with the highest [`FADE_PRIO`]
///     for each of the 7 tracks.
pub fn evaluate_sound_areas(
    stage_id: u32,
    player_pos: Vec3,
    situation_page: u32,
) -> SoundAreaEvaluation {
    if (6..=15).contains(&situation_page) {
        let preset_idx = (situation_page - 6) as usize;
        let (ins, outs) = SITUATION_PRESETS_6_TO_15[preset_idx];
        let mut track_fades = [(0u8, 0u8); BGM_MAX_TRACKS];
        for i in 0..BGM_MAX_TRACKS {
            track_fades[i] = (ins[i], outs[i]);
        }
        return SoundAreaEvaluation {
            matched_box_count: 1,
            matched_record_indices: Vec::new(),
            used_default_record: false,
            track_fades,
        };
    }

    let records: &[SoundDataRecord] = STAGE_SOUND_DATA
        .get(stage_id as usize)
        .copied()
        .unwrap_or(&[]);

    if records.is_empty() {
        return SoundAreaEvaluation {
            matched_box_count: 0,
            matched_record_indices: Vec::new(),
            used_default_record: true,
            track_fades: [(0, 0); BGM_MAX_TRACKS],
        };
    }

    let px = player_pos.x;
    let py = player_pos.y;
    let pz = player_pos.z;

    let mut matched_indices = Vec::new();
    for (idx, rec) in records.iter().enumerate().skip(1) {
        if rec.flags < 0 {
            break;
        }
        let x_ok = (px + SOUND_AREA_HALF_WIDTH) >= rec.x0 && (px - SOUND_AREA_HALF_WIDTH) <= rec.x2;
        let z_ok = (pz + SOUND_AREA_HALF_WIDTH) >= rec.z2 && (pz - SOUND_AREA_HALF_WIDTH) <= rec.z0;
        let y_low = rec.max_y.min(rec.min_y);
        let y_high = rec.max_y.max(rec.min_y);
        let y_ok = py >= y_low && py <= y_high;

        if x_ok && z_ok && y_ok {
            matched_indices.push(idx);
            if matched_indices.len() >= MAX_MATCHED_SOUND_AREAS {
                break;
            }
        }
    }

    let used_default_record = matched_indices.is_empty();
    let eval_indices: &[usize] = if used_default_record {
        &[0]
    } else {
        &matched_indices
    };

    let mut resolved = [(0u8, 0u8); BGM_MAX_TRACKS];
    for &rec_idx in eval_indices {
        let rec_fades = extract_record_track_fades(&records[rec_idx], situation_page);
        for track_idx in 0..BGM_MAX_TRACKS {
            let (cand_in, cand_out) = rec_fades[track_idx];
            let (cur_in, cur_out) = resolved[track_idx];

            let cur_in_prio = FADE_PRIO.get(cur_in as usize).copied().unwrap_or(0);
            let cand_in_prio = FADE_PRIO.get(cand_in as usize).copied().unwrap_or(0);
            let best_in = if cand_in_prio > cur_in_prio {
                cand_in
            } else {
                cur_in
            };

            let cur_out_prio = FADE_PRIO.get(cur_out as usize).copied().unwrap_or(0);
            let cand_out_prio = FADE_PRIO.get(cand_out as usize).copied().unwrap_or(0);
            let best_out = if cand_out_prio > cur_out_prio {
                cand_out
            } else {
                cur_out
            };

            resolved[track_idx] = (best_in, best_out);
        }
    }

    SoundAreaEvaluation {
        matched_box_count: eval_indices.len(),
        matched_record_indices: matched_indices,
        used_default_record,
        track_fades: resolved,
    }
}

/// Runtime state machine for Silent Hill 2's BGM streaming & multi-channel AIX mixing (`SeBgmManager`).
#[derive(Debug, Clone)]
pub struct BgmManager {
    /// Game root directory containing `sh2e/sound/adx/` and `data/sound/adx/`.
    pub game_root: PathBuf,
    /// Selected audio source pack (`EnhancedLossless` 48 kHz vs `OriginalPs2` 24/44.1 kHz).
    pub audio_pack_source: AudioPackSource,
    /// Current stage ID (`0..=58`, `bgm.room` at `0x1F837CC`).
    pub stage_id: u32,
    /// Current room ID resolved from `bgm_room_id(glb_crd, x, z)`.
    pub room_id: u32,
    /// Current active BGM page (`0..=47`).
    pub current_page: u32,
    /// Target BGM page requested by `request_page` / `SeBgmChange`.
    pub target_page: u32,
    /// Currently playing `start_id` (`50000..=50130`, `bgm.play` at `0x1F837C0`).
    pub current_play_id: i32,
    /// Next queued `start_id` (`bgm.next` at `0x1F837C4`).
    pub next_play_id: i32,
    /// Next queued `load_id` (`5030..=5083`, `bgm.load` at `0x1F837C8`).
    pub pending_load_id: i32,
    /// Loaded `load_id` currently resident in the audio engine (`5030..=5083`, or `0`).
    pub loaded_file_id: i32,
    /// Relative path of the currently loaded `.adx` or `.aix` file.
    pub loaded_rel_path: Option<&'static str>,
    /// Resolved filesystem path of the currently playing `.adx` or `.aix` file.
    pub loaded_full_path: Option<PathBuf>,
    /// Whether the currently loaded stream is a single-track `.adx` (`true`) or multi-layer `.aix` (`false`).
    pub loaded_is_adx: bool,
    /// Sample rate (Hz) of the currently loaded stream (e.g., `48000` for SH2:EE, `24000`/`44100` for PS2/PC).
    pub loaded_sample_rate: u32,
    /// Number of decoded stereo layers in the active stream (`1` for ADX, `2..=7` for AIX).
    pub loaded_layer_count: usize,
    /// Per-file master volume scalar (`0x8A73B4`, `0.25..=0.90`, default `0.40`).
    pub file_master_volume: f32,
    /// Global BGM ducking/fade scalar (`bgm.vol` at `0x1F837D4`, `0.0..=1.0`, default `1.0`).
    pub global_bgm_fade: f32,
    /// Current situation page (`0` = Calm, `2` = Low Health, `3` = Enemy Near, `4..=15` = Event).
    pub situation_page: u32,
    /// Last `_SOUND_DATA` spatial evaluation snapshot.
    pub last_area_eval: SoundAreaEvaluation,
    /// Per-track state machine for the 7 AIX layers (`0x1F837D8`).
    pub tracks: [BgmTrackState; BGM_MAX_TRACKS],
    /// Event/progression flags affecting BGM page and situation selection.
    pub event_flags: BgmEventFlags,
}

impl BgmManager {
    /// Creates a new `BgmManager` rooted at `game_root` with the specified [`AudioPackSource`].
    pub fn new(game_root: impl Into<PathBuf>, audio_pack_source: AudioPackSource) -> Self {
        Self {
            game_root: game_root.into(),
            audio_pack_source,
            stage_id: 0,
            room_id: 0,
            current_page: 0,
            target_page: 0,
            current_play_id: 0,
            next_play_id: 0,
            pending_load_id: 0,
            loaded_file_id: 0,
            loaded_rel_path: None,
            loaded_full_path: None,
            loaded_is_adx: false,
            loaded_sample_rate: 0,
            loaded_layer_count: 0,
            file_master_volume: 0.40,
            global_bgm_fade: 1.0,
            situation_page: 0,
            last_area_eval: SoundAreaEvaluation {
                matched_box_count: 0,
                matched_record_indices: Vec::new(),
                used_default_record: true,
                track_fades: [(0, 0); BGM_MAX_TRACKS],
            },
            tracks: [BgmTrackState::default(); BGM_MAX_TRACKS],
            event_flags: BgmEventFlags::default(),
        }
    }

    /// Switches between `AudioPackSource::EnhancedLossless` (`sh2e/sound/adx/`) and
    /// `AudioPackSource::OriginalPs2` (`data/sound/adx/`). If a stream is currently playing,
    /// reloads the active stream from the new pack while preserving all 7 track volumes and states.
    pub fn set_audio_pack_source(
        &mut self,
        source: AudioPackSource,
        engine: Option<&mut AudioEngine>,
    ) -> Result<(), AudioError> {
        if self.audio_pack_source == source {
            return Ok(());
        }
        self.audio_pack_source = source;
        if self.loaded_file_id > 0 {
            let load_id = self.loaded_file_id;
            self.load_and_start_stream(load_id, engine)?;
        }
        Ok(())
    }

    /// Requests a transition to BGM `page` (`0..=47`), reproducing `FUN_005155e0` (`0x5155E0` in `sh2pc.exe`):
    /// - Looks up `(load_id, start_id)` in `BGM_PAGES[page]`.
    /// - If `start_id != current_play_id`:
    ///   - Sets `next_play_id = start_id` and `pending_load_id = load_id`.
    ///   - Transitions all active tracks (`status != Off`) to `FadingOut` (`TR_OUT = 3`) with
    ///     `fade_out = 1` (`3.0` seconds), unless already fading out with type `3` (`6.0s`).
    pub fn request_page(&mut self, page: u32) {
        let clamped_page = page.min(47);
        self.target_page = clamped_page;
        let entry = bgm_page_entry(clamped_page);

        if entry.start_id != self.current_play_id {
            self.next_play_id = entry.start_id;
            self.pending_load_id = entry.load_id;

            for track in &mut self.tracks {
                if track.status != BgmTrackStatus::Off {
                    if track.status == BgmTrackStatus::FadingOut && track.fade_out == 3 {
                        continue;
                    }
                    track.status = BgmTrackStatus::FadingOut;
                    track.fade_out = 1; // 3.0s fade-out before loading next page
                }
            }
        }
    }

    /// Evaluates `SeBgmChange` (`0x515C20`) for `stage_id` and `player_pos`, and requests the resolved BGM page.
    pub fn update_stage_and_position(&mut self, stage_id: u32, player_pos: Vec3) -> u32 {
        self.stage_id = stage_id;
        let glb = stage_glb_crd(stage_id);
        self.room_id = bgm_room_id(glb, player_pos.x, player_pos.z);
        let page = pick_bgm_page(stage_id, self.room_id, &self.event_flags);
        self.request_page(page);
        page
    }

    /// Immediately stops all BGM tracks and resets state (`0x515640` in `sh2pc.exe`).
    pub fn stop_all(&mut self, engine: Option<&mut AudioEngine>) {
        self.current_page = 0;
        self.target_page = 0;
        self.current_play_id = 0;
        self.next_play_id = 0;
        self.pending_load_id = 0;
        self.loaded_file_id = 0;
        self.loaded_rel_path = None;
        self.loaded_full_path = None;
        self.loaded_layer_count = 0;
        self.loaded_sample_rate = 0;
        self.tracks = [BgmTrackState::default(); BGM_MAX_TRACKS];
        if let Some(eng) = engine {
            eng.stop_bgm(Duration::from_millis(100));
        }
    }

    /// Steps the `SeBgmManager` (`0x516080`) state machine by `dt_secs`:
    /// 1. Resolves room ID and `SeBgmChange` target BGM page for `(stage_id, player_pos)`.
    /// 2. Handles page transition fade-out completion (`all tracks == Off`) and loads `pending_load_id`.
    /// 3. Starts `next_play_id` and sets `file_master_volume` (`0x8A73B4`) and initial track states:
    ///    - For single-stream `.adx`: Track 0 starts immediately at `On` (`vol = 1.0`), matching `SeBgmCall` (`0x194220`).
    ///    - For multi-layer `.aix`: All 7 tracks start at `Off` (`vol = 0.0`) and are driven by `_SOUND_DATA` area mixing.
    /// 4. Evaluates `BgmPageSet` situation page and `sndGetSoundAryByCharaPos` `_SOUND_DATA` area fades.
    /// 5. Advances per-track `FadingIn` / `FadingOut` volume ramps and updates `AudioEngine` layer gains.
    pub fn tick(
        &mut self,
        stage_id: u32,
        player_pos: Vec3,
        player_hp_ratio: f32,
        min_enemy_dist_xz: Option<f32>,
        dt_secs: f32,
        mut engine: Option<&mut AudioEngine>,
    ) -> Result<(), AudioError> {
        self.update_stage_and_position(stage_id, player_pos);

        // 1. Check if a page transition is waiting for active tracks to finish fading out (`0x516089..0x5160F3`)
        let mut waiting_for_fadeout = false;
        if self.current_play_id != self.next_play_id && self.next_play_id != 0 {
            let all_off = self.tracks.iter().all(|t| t.status == BgmTrackStatus::Off);
            if all_off {
                if let Some(eng) = engine.as_deref_mut() {
                    eng.stop_bgm(Duration::from_millis(20));
                }
                if self.pending_load_id > 0 {
                    self.load_and_start_stream(self.pending_load_id, engine.as_deref_mut())?;
                } else {
                    self.loaded_file_id = 0;
                    self.loaded_rel_path = None;
                    self.loaded_full_path = None;
                    self.loaded_layer_count = 0;
                    self.loaded_sample_rate = 0;
                }
                self.current_play_id = self.next_play_id;
                self.current_page = self.target_page;
                self.pending_load_id = 0;

                if self.current_play_id > 0 {
                    self.file_master_volume =
                        master_volume_for_start_id(self.current_play_id as u32);
                    if self.loaded_is_adx {
                        self.tracks[0] = BgmTrackState {
                            status: BgmTrackStatus::On,
                            fade_in: 4,
                            fade_out: 1,
                            volume: 1.0,
                        };
                    }
                } else {
                    // Page 0 (-1, -1) silence reached
                    self.current_play_id = 0;
                    self.next_play_id = 0;
                }
            } else {
                waiting_for_fadeout = true;
            }
        } else if self.pending_load_id != 0 {
            // Immediate first load when `current_play_id == 0` (`0x5160F4..0x516299`)
            let load_id = self.pending_load_id;
            self.pending_load_id = 0;
            self.current_play_id = self.next_play_id;
            self.current_page = self.target_page;

            if self.current_play_id > 0 && load_id > 0 {
                self.file_master_volume = master_volume_for_start_id(self.current_play_id as u32);
                self.tracks = [BgmTrackState::default(); BGM_MAX_TRACKS];
                self.load_and_start_stream(load_id, engine.as_deref_mut())?;
                if self.loaded_is_adx {
                    self.tracks[0] = BgmTrackState {
                        status: BgmTrackStatus::On,
                        fade_in: 4,
                        fade_out: 1,
                        volume: 1.0,
                    };
                }
            } else {
                self.stop_all(engine.as_deref_mut());
            }
        }

        // 2. Evaluate `_SOUND_DATA` spatial volume areas when an AIX stream is active and not mid-page-transition (`0x51629A..0x516401`)
        if self.current_play_id > 0 && self.pending_load_id == 0 && !waiting_for_fadeout {
            self.situation_page = evaluate_situation_page(
                self.stage_id,
                self.room_id,
                player_hp_ratio,
                min_enemy_dist_xz,
                &self.event_flags,
            );
            let eval = evaluate_sound_areas(self.stage_id, player_pos, self.situation_page);

            if !self.loaded_is_adx {
                for (i, &(in_type, out_type)) in eval.track_fades.iter().enumerate() {
                    if in_type != 0 {
                        if self.tracks[i].status == BgmTrackStatus::Off
                            || self.tracks[i].status == BgmTrackStatus::FadingOut
                        {
                            self.tracks[i].status = BgmTrackStatus::FadingIn;
                            self.tracks[i].fade_in = in_type;
                            self.tracks[i].fade_out = out_type.max(1);
                        }
                    } else if self.tracks[i].status != BgmTrackStatus::Off {
                        self.tracks[i].status = BgmTrackStatus::FadingOut;
                        if self.tracks[i].fade_out == 0 {
                            self.tracks[i].fade_out = 1;
                        }
                    }
                }
            }
            self.last_area_eval = eval;
        }

        // 3. Step per-track volume ramps (`0x516402..0x51650B`)
        let dt = dt_secs.max(0.0);
        for (i, track) in self.tracks.iter_mut().enumerate() {
            match track.status {
                BgmTrackStatus::FadingIn => {
                    if track.fade_in == 4 {
                        track.volume = 1.0;
                        track.status = BgmTrackStatus::On;
                    } else {
                        let dur = FADE_TIME_SECS
                            .get(track.fade_in as usize)
                            .copied()
                            .unwrap_or(3.0);
                        if dur <= 0.0 {
                            track.volume = 1.0;
                            track.status = BgmTrackStatus::On;
                        } else {
                            track.volume += dt / dur;
                            if track.volume >= 1.0 {
                                track.volume = 1.0;
                                track.status = BgmTrackStatus::On;
                            }
                        }
                    }
                }
                BgmTrackStatus::FadingOut => {
                    if track.fade_out == 4 {
                        track.volume = 0.0;
                        track.status = BgmTrackStatus::Off;
                    } else {
                        let dur = FADE_TIME_SECS
                            .get(track.fade_out as usize)
                            .copied()
                            .unwrap_or(3.0);
                        if dur <= 0.0 {
                            track.volume = 0.0;
                            track.status = BgmTrackStatus::Off;
                        } else {
                            track.volume -= dt / dur;
                            if track.volume <= 0.0 {
                                track.volume = 0.0;
                                track.status = BgmTrackStatus::Off;
                            }
                        }
                    }
                }
                BgmTrackStatus::On => {
                    track.volume = 1.0;
                }
                BgmTrackStatus::Off => {
                    track.volume = 0.0;
                }
            }

            if let Some(eng) = engine.as_deref_mut() {
                let effective_gain = track.volume * self.file_master_volume * self.global_bgm_fade;
                eng.set_bgm_layer_gain(i, effective_gain, Duration::from_millis(16));
            }
        }

        Ok(())
    }

    /// Resolves, decodes, and starts playback of the `.adx` or `.aix` file corresponding to `load_id`.
    fn load_and_start_stream(
        &mut self,
        load_id: i32,
        engine: Option<&mut AudioEngine>,
    ) -> Result<(), AudioError> {
        let Some(file_entry) = find_bgm_file_by_load_id(load_id) else {
            return Err(AudioError::AssetError(format!(
                "Unknown BGM load_id: {load_id}"
            )));
        };

        self.loaded_file_id = load_id;
        self.loaded_rel_path = Some(file_entry.path);
        self.loaded_is_adx = file_entry.is_adx;

        let Some(full_path) = resolve_bgm_stream_path_for_source(
            &self.game_root,
            file_entry.path,
            self.audio_pack_source,
        ) else {
            // In headless unit tests without retail assets on disk, still record metadata state cleanly
            self.loaded_full_path = None;
            self.loaded_layer_count = if file_entry.is_adx { 1 } else { BGM_MAX_TRACKS };
            self.loaded_sample_rate = match self.audio_pack_source {
                AudioPackSource::Auto | AudioPackSource::Sh2eeLossless => 48000,
                AudioPackSource::Ps2Original => 24000,
            };
            return Ok(());
        };

        let bytes = std::fs::read(&full_path).map_err(|e| {
            AudioError::AssetError(format!(
                "Failed to read BGM file {}: {e}",
                full_path.display()
            ))
        })?;

        let layers: Vec<DecodedAdx> = if file_entry.is_adx {
            let decoded = decode_adx(&bytes).map_err(|e| {
                AudioError::AssetError(format!("Failed to decode ADX {}: {e}", full_path.display()))
            })?;
            vec![decoded]
        } else {
            decode_aix(&bytes).map_err(|e| {
                AudioError::AssetError(format!("Failed to decode AIX {}: {e}", full_path.display()))
            })?
        };

        self.loaded_full_path = Some(full_path);
        self.loaded_layer_count = layers.len();
        self.loaded_sample_rate = layers.first().map(|l| l.header.sample_rate).unwrap_or(0);

        if let Some(eng) = engine {
            let mut initial_gains = [0.0f32; BGM_MAX_TRACKS];
            for (i, gain) in initial_gains.iter_mut().enumerate() {
                let track_vol = if file_entry.is_adx && i == 0 {
                    1.0
                } else {
                    self.tracks[i].volume
                };
                *gain = track_vol * self.file_master_volume * self.global_bgm_fade;
            }
            // AIX streams in Silent Hill 2 loop seamlessly from end-of-stream back to sample 0
            let loop_whole_stream = !file_entry.is_adx;
            eng.play_bgm_layers(&layers, &initial_gains[..layers.len()], loop_whole_stream)?;
        }

        Ok(())
    }

    /// Formats a concise telemetry summary line for the in-game debug HUD.
    pub fn hud_summary(&self) -> String {
        let file_label = self.loaded_rel_path.unwrap_or("None (Silence)");
        let pack_label = match self.audio_pack_source {
            AudioPackSource::Auto => "Auto (SH2:EE 48kHz)",
            AudioPackSource::Sh2eeLossless => "SH2:EE 48kHz",
            AudioPackSource::Ps2Original => "PS2/PC Orig",
        };
        let sit_label = match self.situation_page {
            0 => "0:Calm",
            1 => "1:Event1",
            2 => "2:LowHP",
            3 => "3:EnemyNear",
            4 => "4:Event4",
            5 => "5:Event5",
            6..=13 => "6..13:MusicBox",
            14 => "14:Mute",
            15 => "15:Event15",
            _ => "Other",
        };
        let vols: Vec<String> = self
            .tracks
            .iter()
            .take(self.loaded_layer_count.clamp(1, BGM_MAX_TRACKS))
            .enumerate()
            .map(|(i, t)| format!("T{}:{:.0}%", i, t.volume * 100.0))
            .collect();
        format!(
            "BGM [{} | {}Hz] Page:{} (Room:0x{:02X}) Sit:{} | {} | [{}]",
            pack_label,
            self.loaded_sample_rate,
            self.current_page,
            self.room_id,
            sit_label,
            file_label,
            vols.join(" ")
        )
    }
}

/// Decodes a BGM `.adx` or `.aix` file from `game_root` using `source` into its constituent stereo layers.
pub fn decode_bgm_layers_from_disk(
    game_root: impl AsRef<Path>,
    rel_bgm_path: &str,
    source: AudioPackSource,
) -> Result<(PathBuf, Vec<DecodedAdx>), AudioError> {
    let path =
        resolve_bgm_stream_path_for_source(game_root, rel_bgm_path, source).ok_or_else(|| {
            AudioError::AssetError(format!("Could not resolve BGM path: {rel_bgm_path}"))
        })?;
    let bytes = std::fs::read(&path)
        .map_err(|e| AudioError::AssetError(format!("Failed to read {}: {e}", path.display())))?;
    let is_aix = rel_bgm_path.to_ascii_lowercase().ends_with(".aix")
        || (bytes.len() >= 4 && &bytes[0..4] == b"AIXF");
    let layers = if is_aix {
        decode_aix(&bytes).map_err(|e| AudioError::AssetError(e.to_string()))?
    } else {
        vec![decode_adx(&bytes).map_err(|e| AudioError::AssetError(e.to_string()))?]
    };
    Ok((path, layers))
}
