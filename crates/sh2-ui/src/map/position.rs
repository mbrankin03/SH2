//! 1:1 PS2 / PC `ChizuSelect` (`SLUS_202.28` `0x001c7040` / `sh2pc.exe` `0x0049ac70`) and
//! `ChizuCurrentPositionCheck` (`SLUS_202.28` `0x001c9550` / `sh2pc.exe` `0x0049b9a0`).
//!
//! # Ground Truth Citations
//! - `chz_crt_block`: `SLUS_202.28` `0x0033bef0` (`0xba` = `186` rooms) / `sh2pc.exe` `0x0088cb18`
//!   (`0xd6` = `214` rooms including *Born From a Wish* rooms `0xbe..=0xd5`).
//! - `ChizuSelect`: `SLUS_202.28` `0x001c7040` / `sh2pc.exe` `0x0049ac70`.
//! - `ChizuCurrentPositionCheck`: `SLUS_202.28` `0x001c9550` / `sh2pc.exe` `0x0049b9a0`.
//! - `ChizuCurrentPositionDraw` yaw offsets & Map 1 visibility guard:
//!   `SLUS_202.28` `0x001c9000` / `sh2pc.exe` `0x0049d750`.

use std::f32::consts::{FRAC_PI_2, PI};

use sh2_core::events::EventFlags;

pub use crate::map::position_data::{ChzCrtBlockEntry, CHZ_CRT_BLOCK, CHZ_CRT_BLOCK_COUNT};

/// Exact 1:1 port of `ChizuSelect` (`SLUS_202.28` `0x001c7040` / `sh2pc.exe` `0x0049ac70`).
///
/// Resolves the active 1-based map sheet ID (`1..=36`, or `0` if none) from the player's PS2
/// `RoomName` ID (`0..214`), world position `[x, y, z]` in PS2 mm, and [`EventFlags`]:
/// - Applies all multi-floor stairwell / elevator overrides (`0x08`, `0x0e`, `0x0f`, `0x10`, `0x11`,
///   `0x20`, `0x21`, `0x29`, `0x2a`, `0x46`, `0x47`, `0x48`, `0x91`, `0x92`, `0xab`, `0xb8`).
/// - Upgrades Wood Side / Blue Creek maps (`4..=6 -> +5`, `7..=8 -> +2`) when both Apartment maps
///   are owned (`flags.get(25) && flags.get(26)`).
/// - Upgrades Lakeview Hotel Guest maps (`20..=27 -> +8`) when the Employee map is owned
///   (`flags.get(31)`).
/// - Collapses Toluca Prison Courtyard (`17`) into Toluca Prison (`16`).
#[must_use]
pub fn chizu_select(room_id: usize, player_pos: [f32; 3], flags: &EventFlags) -> u8 {
    let [px, py, pz] = player_pos;
    let mut map_id: i32 = CHZ_CRT_BLOCK
        .get(room_id)
        .map_or(0, |entry| entry.map_id as i32);

    match room_id {
        // Retail sh2pc.exe 0x0049ac70: Town West/East transition bridge (room 0x08)
        0x08 => {
            if (40298.0..79561.0).contains(&px) && (-60828.848..-59016.0).contains(&pz) {
                map_id = 2;
            } else {
                map_id = 3;
            }
        }
        // Retail sh2pc.exe 0x0049ac70: Toluca Lake boat / pier (room 0x0e)
        0x0e => {
            // DAT_01f7aa0c = flags word 11 (bits 352..383): bit 29 = 381, bit 27 = 379
            if flags.get(381) {
                map_id = 1;
            } else {
                map_id = if flags.get(379) { 1 } else { 3 };
            }
        }
        // Wood Side Apartments East/West/Central stairwells (rooms 0x0f, 0x10, 0x11)
        0x0f => {
            if py > -750.0 || (px < -60000.0 && py > -1000.0) {
                map_id = 4;
            } else if py > -2350.0 || (px < -60000.0 && py > -2600.0) {
                map_id = 5;
            } else {
                map_id = 6;
            }
        }
        0x10 => {
            if py > -900.0 || (pz > -19300.0 && py > -1150.0) {
                map_id = 4;
            } else if py > -2700.0 || (pz > -19300.0 && py > -3450.0) {
                map_id = 5;
            } else {
                map_id = 6;
            }
        }
        0x11 => {
            if py > -900.0 || (pz > -59300.0 && py > -1150.0) {
                map_id = 4;
            } else if py > -2700.0 || (pz > -59300.0 && py > -3450.0) {
                map_id = 5;
            } else {
                map_id = 6;
            }
        }
        // Blue Creek Apartments stairwells (rooms 0x20, 0x21)
        0x20 => {
            if pz < -19200.0 && py > -1050.0 {
                map_id = 7;
            } else {
                map_id = 8;
            }
        }
        0x21 => {
            if py < -855.0 || (pz > -99431.0 && py < -854.0) {
                map_id = 8;
            } else {
                map_id = 7;
            }
        }
        // Brookhaven Hospital Normal stairwell & elevator (rooms 0x29, 0x2a)
        0x29 => {
            if py > -950.0 || (pz > -100000.0 && py > -1200.0) {
                map_id = 12;
            } else {
                map_id = 13;
            }
        }
        0x2a => {
            if flags.get(17) {
                map_id = 12;
            }
            if flags.get(18) || flags.get(19) {
                map_id = 13;
            }
        }
        // Brookhaven Hospital Otherworld elevator & stairwells (rooms 0x46, 0x47, 0x48)
        0x46 => {
            if flags.get(17) {
                map_id = 14;
            }
            if flags.get(18) || flags.get(19) {
                map_id = 15;
            }
        }
        0x47 => {
            if py >= -1000.0 || (pz > -219978.0 && py > -1001.0) {
                map_id = 14;
            } else {
                map_id = 15;
            }
        }
        0x48 => {
            // Retail sh2pc.exe 0x0049b030 preserves chz_crt_block[0x48] when py >= 645.0
            if py < 645.0 {
                if py > -1000.0 || (py < -1001.0 && pz > -99992.0 && py >= -1000.0) {
                    map_id = 12;
                } else {
                    map_id = 15;
                }
            }
        }
        // Lakeview Hotel Normal & Otherworld employee/guest stairwells (rooms 0x91, 0x92, 0xab)
        0x91 => {
            if py < -920.0 || (pz < -20020.0 && py < -874.0) {
                map_id = 29;
            } else {
                map_id = 28;
            }
        }
        0x92 => {
            if py > -1400.0 {
                map_id = 21;
            } else {
                map_id = 22;
            }
        }
        0xab => {
            // Retail sh2pc.exe 0x0049b0b0 fixes SLUS prototype 0x1c/0x1d -> 0x20/0x21 (Otherworld Hotel)
            if py < -920.0 || (pz < -20020.0 && py < -874.0) {
                map_id = 33;
            } else {
                map_id = 32;
            }
        }
        // Retail sh2pc.exe 0x0049b110: Hotel Otherworld elevator / hallway (room 0xb8)
        0xb8 => {
            // DAT_01f7aa18 = flags word 14 (bits 448..479): bit 31 = 479
            map_id = if flags.get(479) { 24 } else { 26 };
        }
        _ => {}
    }

    // Combined Apartment map upgrade (flags 25 & 26: Wood Side + Blue Creek)
    if flags.get(25) && flags.get(26) {
        if (4..=6).contains(&map_id) {
            map_id += 5;
        } else if (7..=8).contains(&map_id) {
            map_id += 2;
        }
    }

    // Lakeview Hotel Employee map upgrade (flag 31, sign bit of word 0)
    if flags.get(31) && (20..=27).contains(&map_id) {
        map_id += 8;
    }

    // Toluca Prison Courtyard (17) collapses to Toluca Prison (16)
    if map_id == 17 {
        map_id = 16;
    }

    map_id.clamp(0, 36) as u8
}

/// Exact 1:1 port of `ChizuCurrentPositionCheck` (`SLUS_202.28` `0x001c9550` / `sh2pc.exe` `0x0049b9a0`).
///
/// Converts world coordinates `[x, y, z]` (PS2 mm) for `room_id` (`0..214`) into PS2 centered
/// `[-256..+256]` virtual map coordinates `(px, py)`.
#[must_use]
pub fn chizu_current_position_check(
    room_id: usize,
    current_map_id: u8,
    player_pos: [f32; 3],
    flags: &EventFlags,
) -> (f32, f32) {
    let [px, py, pz] = player_pos;
    let entry = CHZ_CRT_BLOCK
        .get(room_id)
        .copied()
        .unwrap_or(ChzCrtBlockEntry {
            map_id: 0,
            offset_x: 0.0,
            offset_y: 0.0,
        });
    let ox = entry.offset_x;
    let oy = entry.offset_y;

    match room_id {
        // Static pin rooms (no linear world offset)
        1 | 2 | 5 | 6 | 9 | 10 | 0x0b | 0x0c | 0x0d | 0x5c | 0xbe | 0xd5 => (ox, oy),
        // Observation Deck / Forest Trail (room 3)
        3 => (px * 0.00053 + ox, oy - pz * 0.00079),
        // South Vale East (room 4)
        4 => (px * 0.00142 + ox, oy - pz * 0.00196),
        // Outdoor courtyard / connector rooms
        7 | 0x12 | 0x15 | 0x1f => (px * 0.00801 + ox, oy - pz * 0.01037),
        // South Vale West & transition bridge (room 8)
        8 => {
            if (30453.578..35961.0).contains(&px) && (-60828.848..-59016.0).contains(&pz) {
                return (212.4619, 183.76071);
            }
            if (40298.0..79561.0).contains(&px) && (-60828.848..-59016.0).contains(&pz) {
                return (px * 0.00071 - 217.0, 67.0 - pz * 0.00196);
            }
            (px * 0.00146 + ox, oy - pz * 0.00196)
        }
        // Toluca Lake boat / pier (room 0x0e)
        0x0e => {
            if flags.get(381) {
                (-87.21007, -134.06479)
            } else if !flags.get(379) {
                (-78.40884, -178.77686)
            } else {
                (ox, oy)
            }
        }
        // Wood Side Apartments interior (0x0f..=0x1e)
        0x0f..=0x11 | 0x13 | 0x14 | 0x16..=0x1e => (px * 0.0087 + ox, oy - pz * 0.0096),
        // Blue Creek Apartments interior (0x20..=0x24, 0x27, 0x28)
        0x20..=0x24 | 0x27 | 0x28 => {
            let mut x = px * 0.0087 + ox;
            let y = oy - pz * 0.0096;
            if !flags.get(25) && flags.get(26) {
                if current_map_id == 7 {
                    x += 24.965286;
                } else if current_map_id == 8 {
                    x += 164.96529;
                }
            }
            (x, y)
        }
        // Blue Creek Apartments courtyard/stair (0x25, 0x26)
        0x25 | 0x26 => {
            let mut x = px * 0.00801 + ox;
            let y = oy - pz * 0.01037;
            if !flags.get(25) && flags.get(26) {
                if current_map_id == 7 {
                    x += 16.965286;
                } else if current_map_id == 8 {
                    x += 158.96529;
                }
            }
            (x, y)
        }
        // Brookhaven Hospital Normal stairwell (0x29)
        0x29 => {
            if py > -950.0 || (pz > -100000.0 && py > -1200.0) {
                (px * 0.013396 + 879.80005, -1988.8 - pz * 0.019375)
            } else if py <= -2950.0 && (pz <= -100000.0 || py <= -3200.0) {
                if py <= -4950.0 && (pz <= -100000.0 || py <= -5200.0) {
                    (px * 0.013396 + 631.7003, -1956.7996 - pz * 0.019375)
                } else {
                    (px * 0.013396 + 916.40015, -2086.5996 - pz * 0.019375)
                }
            } else {
                (px * 0.013396 + 916.2002, -1847.7001 - pz * 0.019375)
            }
        }
        // Brookhaven Hospital Normal elevator (0x2a)
        0x2a => {
            if flags.get(19) {
                (px * 0.01477 + 1421.101, pz * -0.019658 - 531.00037)
            } else if flags.get(18) {
                (px * 0.01477 + 1421.101, pz * -0.019658 - 289.90015)
            } else if flags.get(17) {
                (px * 0.01477 + 1385.2999, pz * -0.019658 - 426.99988)
            } else {
                (ox, oy)
            }
        }
        // Brookhaven Hospital Roof (0x3b)
        0x3b => (px * 0.014716 + ox, oy - pz * 0.019583),
        // Brookhaven Hospital Normal & Otherworld rooms (0x2b..=0x45, 0x49..=0x59)
        0x2b..=0x3a | 0x3c..=0x45 | 0x49..=0x59 => (px * 0.01477 + ox, oy - pz * 0.019658),
        // Brookhaven Hospital Otherworld elevator (0x46)
        0x46 => {
            if flags.get(19) {
                (px * 0.01477 + 240.9, pz * -0.019658 - 6029.8965)
            } else if flags.get(18) {
                (px * 0.01477 + 238.69986, pz * -0.019658 - 5789.196)
            } else if flags.get(17) {
                (px * 0.01477 + 203.69986, pz * -0.019658 - 5931.196)
            } else {
                (ox, oy)
            }
        }
        // Brookhaven Hospital Otherworld stairwell 1 (0x47)
        0x47 => {
            if py >= 1001.0 || (py >= 1000.0 && pz > -220067.0) {
                (px * 0.013396 + 878.0, (-31.899994 - pz * 0.019375) - 4096.0)
            } else if py >= -1000.0 || (pz > -219978.0 && py > -1001.0) {
                (px * 0.013396 + 878.80005, (-211.9 - pz * 0.019375) - 4096.0)
            } else if py >= -2998.0 || (pz > -220003.0 && py > -3000.0) {
                (px * 0.013396 + 915.0, (-73.899994 - pz * 0.019375) - 4096.0)
            } else if py <= -5000.0 && (pz <= -220011.0 || py < -5000.0) {
                (px * 0.013396 + 630.0, (-183.9 - pz * 0.019375) - 4096.0)
            } else {
                (px * 0.013396 + 915.0, (-313.9 - pz * 0.019375) - 4096.0)
            }
        }
        // Brookhaven Hospital Otherworld stairwell 2 (0x48)
        0x48 => {
            if py >= 1700.0 {
                (0.0, -1000.0)
            } else if py <= -1000.0 && (-1001.0 <= py || pz <= -99992.0 || py < -1000.0) {
                if py <= -3000.0 && (-3001.0 <= py || pz <= -100009.0 || py < -3000.0) {
                    (px * 0.013396 + 1303.0, (366.19952 - pz * 0.016455) - 4096.0)
                } else {
                    (px * 0.013396 + 1303.0, (607.1995 - pz * 0.016455) - 4096.0)
                }
            } else {
                (px * 0.013396 + 1267.3004, (pz * -0.019375 - 177.0) - 4096.0)
            }
        }
        // Toluca Prison (0x66..=0x74, rotated -90°: (y, -x))
        0x66..=0x71 | 0x73 | 0x74 => {
            let rx = px * 0.01377 + ox;
            let ry = oy - pz * 0.010408;
            (ry, -rx)
        }
        0x72 => {
            let rx = px * 0.01377 + ox;
            let ry = oy - pz * 0.009988;
            (ry, -rx)
        }
        // Labyrinth (0x78..=0x8d, rotated +90°: (-y, x))
        0x78..=0x8d => {
            let rx = px * 0.01007 + ox;
            let ry = oy - pz * 0.0089;
            (-ry, rx)
        }
        // Lakeview Hotel (0x91..=0xb9)
        0x91 | 0x93..=0x98 | 0x9b..=0x9e | 0xa5..=0xa7 | 0xab | 0xb0 | 0xb6 | 0xb7 => {
            (px * 0.0164 + ox, oy - pz * 0.01875)
        }
        0x92 | 0xa2 | 0xac => (px * 0.0164 + ox, oy - pz * 0.0209),
        0x99 => (px * 0.01597 + ox, oy - pz * 0.0207),
        0x9a | 0x9f..=0xa1 | 0xa3 | 0xa8 | 0xa9 | 0xad..=0xaf | 0xb1..=0xb5 | 0xb8 | 0xb9 => {
            (px * 0.01607 + ox, oy - pz * 0.0207)
        }
        // Default + Born From a Wish Baldwin Mansion multi-floor offsets (0xc0, 0xcb, 0xcd)
        _ => {
            let mut x = px * 0.01477 + ox;
            let y = oy - pz * 0.019658;
            if room_id == 0xc0 && py < -1900.0 {
                x += 191.0;
            } else if room_id == 0xcb && py < -1200.0 {
                x += 192.0;
            } else if room_id == 0xcd && py >= -400.0 {
                x -= 189.8;
            }
            (x, y)
        }
    }
}

/// Returns whether `ChizuCurrentPositionDraw` (`SLUS_202.28` `0x001c9000` / `sh2pc.exe` `0x0049d750`)
/// should render the player heading arrow on `current_map_id`.
#[must_use]
pub fn chizu_should_draw_player_arrow(
    current_map_id: u8,
    selected_map_id: u8,
    room_id: usize,
    flags: &EventFlags,
) -> bool {
    if current_map_id == 0 || current_map_id != selected_map_id {
        return false;
    }
    if current_map_id == 1 {
        // Map 1 (Town Overall) only displays the arrow in rooms 1, 2, 3, or room 0x0e with bit 381.
        return matches!(room_id, 1..=3) || (room_id == 0x0e && flags.get(381));
    }
    true
}

/// Returns the map-specific yaw rotation offset in radians (`ChizuCurrentPositionDraw`
/// `SLUS_202.28` `0x001c9078..0x001c9120`, `sh2pc.exe` `0x0049d7d0..0x0049d89b`).
#[must_use]
pub const fn chizu_arrow_yaw_offset(current_map_id: u8, room_id: usize) -> f32 {
    if matches!(current_map_id, 16 | 17) || matches!(room_id, 1 | 5) {
        -FRAC_PI_2
    } else if matches!(current_map_id, 18 | 19) {
        FRAC_PI_2
    } else if matches!(room_id, 9 | 10 | 11 | 0x5c) {
        -PI
    } else {
        0.0
    }
}

/// Maps `sh2-core`'s 1-based `ScenarioState::current_stage` (`1..=58`) to a canonical PS2
/// `RoomName` ID (`Event/stg_name.c` `0x001af0f0`) for `ChizuSelect` and
/// `ChizuCurrentPositionCheck` when fine-grained grid lookup is not yet active.
#[must_use]
pub const fn stage_to_default_room_id(stage_id: u16, is_maria: bool) -> usize {
    if is_maria || stage_id >= 53 {
        return match stage_id {
            53..=55 => 0x08, // Maria Heaven's Night / Town West
            _ => 0xc0,       // Baldwin Mansion
        };
    }
    match stage_id {
        1 => 0x04,       // TGS Trial (Town East)
        2 => 0x01,       // Restroom (er01)
        3 => 0x02,       // Observation Deck (ob01)
        4 => 0x03,       // Forest Trail (rr)
        5 => 0x04,       // South Vale East
        6 => 0x13,       // Wood Side Apartments 1F
        7 => 0x16,       // Wood Side Apartments 2F
        8 | 9 => 0x1b,   // Wood Side Apartments 3F
        10 => 0x22,      // Blue Creek Apartments 1F
        11 => 0x23,      // Blue Creek Apartments 2F
        12 => 0x0f,      // Apartments Stairwell (multi-floor override)
        13 => 0x12,      // Apartments Courtyard
        14 => 0x08,      // South Vale West & Rosewater Park
        15 => 0x09,      // Pete's Bowl-O-Rama
        16 => 0x0a,      // Back Alley to Heaven's Night
        17 => 0x0b,      // Heaven's Night
        18 => 0x2b,      // Brookhaven Hospital 1F Normal
        19 => 0x30,      // Brookhaven Hospital 2F Normal
        20 => 0x35,      // Brookhaven Hospital 3F Normal
        21 => 0x3b,      // Brookhaven Hospital Roof
        22 => 0x29,      // Brookhaven Hospital Stairwell (multi-floor override)
        23 => 0x49,      // Brookhaven Hospital 1F Otherworld
        24 => 0x4d,      // Brookhaven Hospital 2F Otherworld
        25 => 0x52,      // Brookhaven Hospital 3F Otherworld
        26 => 0x47,      // Brookhaven Hospital Otherworld Stairwell
        27 => 0x08,      // Town West Night
        28..=30 => 0x5c, // Historical Society
        31..=33 => 0x66, // Toluca Prison
        34..=37 => 0x78, // Labyrinth
        38 => 0x0e,      // Toluca Lake Boat
        39 => 0x91,      // Lakeview Hotel BF / Employee Stairwell
        40 => 0x92,      // Lakeview Hotel 1F
        41 => 0x99,      // Lakeview Hotel 2F
        42 => 0x9f,      // Lakeview Hotel 3F
        43 => 0xab,      // Lakeview Hotel Otherworld Stairwell
        44..=47 => 0xb0, // Lakeview Hotel Otherworld
        _ => 0x01,
    }
}
