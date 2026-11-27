//! BGM page & situation page selection (`SeBgmChange`, stage `EvBgmControl`, and `BgmPageSet`).

use super::types::{BgmEventFlags, ENEMY_NEAR_DIST_SQ, LOW_HEALTH_RATIO_THRESHOLD};

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
                35 // Page 35 -> hotel/bgm_112_NG.aix
            } else {
                34 // Page 34 -> hotel/bgm_112.aix (7-layer music box AIX in sh2e/)
            }
        }
        // Stage 41: Lakeview Hotel 2F (`Stg_hotel_2f_f`)
        // When `enable_restored_cut_audio` is true and the music box puzzle is solved (`flag_61b & 1`),
        // `MusicBoxVolume.cpp` calls `shBgmCall(0x22)` (`Page 34` -> `hotel/bgm_112.aix`) so the 7th layer
        // (`Track 6`, quieter/muffled Cinderella music box) plays in the 2F hallways connected to the lobby.
        41 => {
            if flags.enable_restored_cut_audio && (flags.flag_61b & 0x01) != 0 {
                34 // Page 34 -> hotel/bgm_112.aix (MusicBoxVolume.cpp 7th-layer restoration)
            } else {
                36 // Page 36 -> hotel/bgm_121.aix
            }
        }
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
        // Stage 3 (`stg_ovservation.c` `EvBgmControl`, `sh2pc.exe` `0x005a1390..0x005a13b8`):
        // - When event/demo mode (`0x932194 & 0x40`) is active: returns `14` (`0x0e`, Mute) once
        //   `flag(517)` (`0x01f7aa20 & 0x20`) is set after `tenbou` frame `1400.0` (`0x005a0936`),
        //   or `4` (`0x04`, Track 1 only) before frame `1400.0`.
        // - Outside event/demo mode (`0x005a13ab..0x005a13b8`): returns `(flag_5e4 >> 4) & 1`.
        3 => {
            if flags.event_mode_active {
                if (flags.flag_620 & 0x20) != 0 {
                    14
                } else {
                    4
                }
            } else {
                u32::from((flags.flag_5e4 >> 4) & 1)
            }
        }
        // Stage 4 (`stg_forest.c` `EvBgmControl`, `sh2pc.exe` `0x005a0260..0x005a027d`):
        // - When `DramaDemoGetNo() == 3` (`haka_agl` Angela Graveyard cutscene): returns `4`.
        // - Otherwise: returns `(flag_5e4 >> 4) & 1` (`1` once `flag(36)` is set, `0` before).
        4 => {
            if flags.active_demo_no == Some(3) {
                4
            } else {
                u32::from((flags.flag_5e4 >> 4) & 1)
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
        // Stage 41 (`Stg_hotel_2f_f`): `MusicBoxVolume.cpp` `GetStageBgmBank()` returns situation `4`
        // once the Hotel 1F music box puzzle is solved (`flag_61b & 0x01`).
        41 => {
            if flags.enable_restored_cut_audio && (flags.flag_61b & 0x01) != 0 {
                4
            } else {
                0
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
    // Retail `sh2pc.exe` `FUN_00516430` (`0x00516430:4757`): `game_mode == 0x0b` forces situation `15`.
    if flags.game_mode == 0x0b {
        return 15;
    }
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
