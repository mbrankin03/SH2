//! 1:1 PS2 / Director's Cut Paper Map tables (`Event/chizu.c` in `SLUS_202.28` / `sh2pc.exe`).
//!
//! # Ground Truth Citations (`SLUS_202.28` & `sh2pc.exe`)
//! - `ChizuMain` (`0x001c6b90` / `0x0049e2e0`): `SeCall(1.0, 0x2b17)` paper rustle SFX.
//! - `ChizuFileLoad` (`0x001c7ac0` / `0x0049b3b0`): base map texture table `0x0033db10` / `0x0088ea88`,
//!   mark texture table `0x0033dba0` / `0x0088eb20`.
//! - `ChizuPossessionCheck` (`0x001c7890` / `0x0049b240`): exact `EventFlags` bit checks for maps
//!   `1..=35`, plus retail Map `36` (Baldwin Mansion: `DAT_01f7aa30 & 0x40000000` = flag `670`).
//! - `ChizuConnectCheck` (`0x001c8ce0` / `0x0049b7c0`): connectivity table `0x0033c7e0` / `0x0088d520`
//!   (`[up, down, right, left]`) with same-building guards and ownership fallbacks.
//! - `ChizuConnectArrowDraw` (`0x001c88f0` / `0x0049d2d0`): `def_pos$1438` (`0x0033dc40`),
//!   `tex$1437` (`0x0033dc30`), `data_pic_etc_carsol_tex` (`0x003a1c20`).
//! - `ChizuCurrentPositionDraw` (`0x001c9000` / `0x0049d750`): `uv$1582` (`0x0033dc60`).
//! - `EventExecChizuFail` (`0x001aae40`): "It's too dark to read the map here."

use sh2_core::events::EventFlags;

/// Total number of maps across James (`1..=35`) and Maria (`36` = Baldwin Mansion).
pub const MAP_COUNT: usize = 36;

/// Paper map rustle SFX ID (`SeCall(1.0, 0x2b17)` in `ChizuMain` `0x001c6e8c` / `sh2pc.exe` `0x0049e560`).
pub const CHIZU_PAPER_SE_ID: u32 = 0x2b17;

/// Shared map cursor/arrow sprite sheet (`data_pic_etc_carsol_tex` `0x003a1c20`, loaded into
/// `cursor_adr` `0x01126940` in `sh2_init.c:40`; `256x128` RGBA).
pub const CHIZU_CURSOR_TEX_PATH: &str = "pic/etc/carsol.tex";

/// Default world-X to PS2-map-px scale (`ChizuCurrentPositionCheck` @ `0x001c95c0`, `0x3aba1f4b`).
pub const CHIZU_WORLD_SCALE_X: f32 = f32::from_bits(0x3aba_1f4b);
/// Default world-Z to PS2-map-px scale (`ChizuCurrentPositionCheck` @ `0x001c9608`, `0x3b007358`).
pub const CHIZU_WORLD_SCALE_Z: f32 = f32::from_bits(0x3b00_7358);

/// Default screen positions `[x, y]` in PS2 centered `[-256..+256]` virtual space for the 4
/// directional connect arrows (`def_pos$1438` at `SLUS_202.28` `0x0033dc40`, order `[Up, Down, Right, Left]`).
pub const CHIZU_ARROW_DEF_POS: [[f32; 2]; 4] = [
    [-36.0, -224.0], // dir 0: Up
    [-36.0, 176.0],  // dir 1: Down
    [212.0, 0.0],    // dir 2: Right
    [-244.0, 0.0],   // dir 3: Left
];

/// Source pixel UV rectangles `[u0, v0, u1, v1]` inside `256x128` `pic/etc/carsol.tex` for the 4
/// directional connect arrows (`tex$1437` at `SLUS_202.28` `0x0033dc30`, `sh2pc.exe` `0x0088ebb4`).
pub const CHIZU_ARROW_TEX_UVS: [[u8; 4]; 4] = [
    [96, 0, 159, 63],  // dir 0: Up
    [160, 0, 223, 63], // dir 1: Down
    [48, 0, 95, 95],   // dir 2: Right
    [0, 0, 47, 95],    // dir 3: Left
];

/// Source Q4 fixed-point UV rectangles `[u0, v0, u1, v1]` (`>> 4` = px in `256x128` `carsol.tex`)
/// for the 2-layer player heading arrow (`uv$1582` at `SLUS_202.28` `0x0033dc60`, `sh2pc.exe` `0x0088ebe4`).
/// - Layer `0`: green/white foreground heading arrow (`[144, 64, 175, 127]` px)
/// - Layer `1`: drop-shadow arrow offset `(+2.0, +0.5)` (`[176, 64, 207, 127]` px)
pub const CHIZU_PLAYER_ARROW_UVS: [[i16; 4]; 2] = [
    [2304, 1024, 2800, 2032], // layer 0: foreground
    [2816, 1024, 3312, 2032], // layer 1: shadow
];

/// Projects world `(x_mm, z_mm)` into normalized `4:3` sheet coordinates `(0.0..=1.0, 0.0..=1.0)`
/// using the fallback `ChizuCurrentPositionCheck` (`0x001c95c0..0x001c9620`) PS2 scale factors.
///
/// Prefer [`crate::map::chizu_current_position_check`] whenever the player's PS2 `room_id` is known.
pub fn chizu_world_to_sheet_norm(world_x_mm: f32, world_z_mm: f32) -> (f32, f32) {
    let px = world_x_mm * CHIZU_WORLD_SCALE_X;
    let py = -world_z_mm * CHIZU_WORLD_SCALE_Z;
    let nx = ((px + 256.0) / 512.0).clamp(0.0, 1.0);
    let ny = ((py + 256.0) / 512.0).clamp(0.0, 1.0);
    (nx, ny)
}

/// Directional floor/wing navigation on the paper map (`ChizuConnectCheck` @ `0x001c8ce0`,
/// `ChizuControl` @ `0x001c8360`, `def_pos$1438` @ `0x0033dc40`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapDirection {
    /// Up floor / next sheet (`param_2 == 0`, pad `0x400`, arrow at `(-36, -224)`).
    Up = 0,
    /// Down floor / previous sheet (`param_2 == 1`, pad `0x800`, arrow at `(-36, +176)`).
    Down = 1,
    /// Next building/wing (`param_2 == 2`, pad `0x100` Right, arrow at `(+212, 0)`).
    Right = 2,
    /// Previous building/wing (`param_2 == 3`, pad `0x200` Left, arrow at `(-244, 0)`).
    Left = 3,
}

/// Static specification for a single paper map sheet (`1..=36`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSheetSpec {
    /// 1-based map identifier (`1..=36`).
    pub map_id: u8,
    /// Human-readable sheet title & floor label (for diagnostics/tests; not drawn on the PS2 sheet).
    pub title: &'static str,
    /// Relative retail base map texture path (`ChizuFileLoad` `0x0033db10` / `0x0088ea88`).
    pub base_tex_path: &'static str,
    /// Relative retail red-pen marker overlay atlas (`ChizuFileLoad` `0x0033dba0` / `0x0088eb20`).
    pub mark_tex_path: &'static str,
    /// First `EventFlags` sentinel ID in retail `sh2pc.exe` `chizu_marker_list` (`0x0088e838`).
    pub marker_flag_start: u16,
    /// Last `EventFlags` sentinel ID (exclusive) in retail `sh2pc.exe` `chizu_marker_list` (`0x0088e838`).
    pub marker_flag_end: u16,
}

/// All 36 paper map sheets extracted 1:1 from `sh2pc.exe` `0x0088ea88` / `0x0088eb20` / `0x0088e838`
/// (cross-checked against `SLUS_202.28` `0x0033db10` / `0x0033dba0` / `0x0033d8d0`).
#[rustfmt::skip]
pub const MAP_SPECS: [MapSheetSpec; MAP_COUNT] = [
    MapSheetSpec { map_id: 1,  title: "Silent Hill (Overall)",           base_tex_path: "pic/map/outmap.tex",        mark_tex_path: "pic/map/outmapmark.tex",      marker_flag_start: 0x0610, marker_flag_end: 0x0614 },
    MapSheetSpec { map_id: 2,  title: "South Vale (East)",               base_tex_path: "pic/map/outmape.tex",       mark_tex_path: "pic/map/outmapmark.tex",      marker_flag_start: 0x05cd, marker_flag_end: 0x05ed },
    MapSheetSpec { map_id: 3,  title: "South Vale (West)",               base_tex_path: "pic/map/outmapw.tex",       mark_tex_path: "pic/map/outmapmark.tex",      marker_flag_start: 0x05ef, marker_flag_end: 0x060a },
    MapSheetSpec { map_id: 4,  title: "Wood Side Apartments 1F",         base_tex_path: "pic/map/apartmape1f.tex",   mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x061a, marker_flag_end: 0x0631 },
    MapSheetSpec { map_id: 5,  title: "Wood Side Apartments 2F",         base_tex_path: "pic/map/apartmape2f.tex",   mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0638, marker_flag_end: 0x065c },
    MapSheetSpec { map_id: 6,  title: "Wood Side Apartments 3F",         base_tex_path: "pic/map/apartmape3f.tex",   mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0662, marker_flag_end: 0x0679 },
    MapSheetSpec { map_id: 7,  title: "Blue Creek Apartments 1F",        base_tex_path: "pic/map/apartmapw.tex",     mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x067f, marker_flag_end: 0x0693 },
    MapSheetSpec { map_id: 8,  title: "Blue Creek Apartments 2F",        base_tex_path: "pic/map/apartmapw.tex",     mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0698, marker_flag_end: 0x06ad },
    MapSheetSpec { map_id: 9,  title: "Apartments (East & West) 1F",     base_tex_path: "pic/map/apartmapew1f.tex",  mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x067f, marker_flag_end: 0x0693 },
    MapSheetSpec { map_id: 10, title: "Apartments (East & West) 2F",     base_tex_path: "pic/map/apartmapew2f.tex",  mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0698, marker_flag_end: 0x06ad },
    MapSheetSpec { map_id: 11, title: "Apartments (East & West) 3F",     base_tex_path: "pic/map/apartmapew3f.tex",  mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0662, marker_flag_end: 0x0679 },
    MapSheetSpec { map_id: 12, title: "Brookhaven Hospital 1F / BF",     base_tex_path: "pic/map/hospitalmap01.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x06b3, marker_flag_end: 0x06e1 },
    MapSheetSpec { map_id: 13, title: "Brookhaven Hospital 2F / 3F / RF",base_tex_path: "pic/map/hospitalmap02.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x06e9, marker_flag_end: 0x0731 },
    MapSheetSpec { map_id: 14, title: "Brookhaven Otherworld 1F / BF",   base_tex_path: "pic/map/hospitalmap01.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x073a, marker_flag_end: 0x0769 },
    MapSheetSpec { map_id: 15, title: "Brookhaven Otherworld 2F / 3F",   base_tex_path: "pic/map/hospitalmap02.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x0771, marker_flag_end: 0x07a6 },
    MapSheetSpec { map_id: 16, title: "Toluca Prison",                   base_tex_path: "pic/map/prisonmap03.tex",   mark_tex_path: "pic/map/prisonmap03mark.tex", marker_flag_start: 0x07af, marker_flag_end: 0x07fc },
    MapSheetSpec { map_id: 17, title: "Toluca Prison (Courtyard)",       base_tex_path: "pic/map/prisonmap03.tex",   mark_tex_path: "pic/map/prisonmap03mark.tex", marker_flag_start: 0x0804, marker_flag_end: 0x080c },
    MapSheetSpec { map_id: 18, title: "Labyrinth (Upper)",               base_tex_path: "pic/map/prisonmap.tex",     mark_tex_path: "pic/map/prisonmap01mark.tex", marker_flag_start: 0x091f, marker_flag_end: 0x0938 },
    MapSheetSpec { map_id: 19, title: "Labyrinth (Lower)",               base_tex_path: "pic/map/prisonmap.tex",     mark_tex_path: "pic/map/prisonmap02mark.tex", marker_flag_start: 0x0940, marker_flag_end: 0x0959 },
    MapSheetSpec { map_id: 20, title: "Lakeview Hotel BF (Guest)",       base_tex_path: "pic/map/hotelmapbf01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0811, marker_flag_end: 0x0817 },
    MapSheetSpec { map_id: 21, title: "Lakeview Hotel 1F (Guest)",       base_tex_path: "pic/map/hotelmap1f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x082d, marker_flag_end: 0x0842 },
    MapSheetSpec { map_id: 22, title: "Lakeview Hotel 2F (Guest)",       base_tex_path: "pic/map/hotelmap2f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x086d, marker_flag_end: 0x0890 },
    MapSheetSpec { map_id: 23, title: "Lakeview Hotel 3F (Guest)",       base_tex_path: "pic/map/hotelmap3f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0897, marker_flag_end: 0x08a2 },
    MapSheetSpec { map_id: 24, title: "Lakeview Otherworld BF",          base_tex_path: "pic/map/hotelmapbf01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x08a8, marker_flag_end: 0x08ac },
    MapSheetSpec { map_id: 25, title: "Lakeview Otherworld 1F",          base_tex_path: "pic/map/hotelmap1f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x08c0, marker_flag_end: 0x08ca },
    MapSheetSpec { map_id: 26, title: "Lakeview Otherworld 2F",          base_tex_path: "pic/map/hotelmap2f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x08e7, marker_flag_end: 0x090b },
    MapSheetSpec { map_id: 27, title: "Lakeview Otherworld 3F",          base_tex_path: "pic/map/hotelmap3f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0912, marker_flag_end: 0x091a },
    MapSheetSpec { map_id: 28, title: "Lakeview Hotel BF (Employee)",    base_tex_path: "pic/map/hotelmapbf02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x081b, marker_flag_end: 0x0827 },
    MapSheetSpec { map_id: 29, title: "Lakeview Hotel 1F (Employee)",    base_tex_path: "pic/map/hotelmap1f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0849, marker_flag_end: 0x0865 },
    MapSheetSpec { map_id: 30, title: "Lakeview Hotel 2F (Employee)",    base_tex_path: "pic/map/hotelmap2f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x086d, marker_flag_end: 0x0890 },
    MapSheetSpec { map_id: 31, title: "Lakeview Hotel 3F (Employee)",    base_tex_path: "pic/map/hotelmap3f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0897, marker_flag_end: 0x08a2 },
    MapSheetSpec { map_id: 32, title: "Lakeview Otherworld BF (Emp)",    base_tex_path: "pic/map/hotelmapbf02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x08b0, marker_flag_end: 0x08ba },
    MapSheetSpec { map_id: 33, title: "Lakeview Otherworld 1F (Emp)",    base_tex_path: "pic/map/hotelmap1f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x08d0, marker_flag_end: 0x08e1 },
    MapSheetSpec { map_id: 34, title: "Lakeview Otherworld 2F (Emp)",    base_tex_path: "pic/map/hotelmap2f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x08e7, marker_flag_end: 0x090b },
    MapSheetSpec { map_id: 35, title: "Lakeview Otherworld 3F (Emp)",    base_tex_path: "pic/map/hotelmap3f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0912, marker_flag_end: 0x091a },
    MapSheetSpec { map_id: 36, title: "Baldwin Mansion (Maria)",         base_tex_path: "pic/add/mapboldwin.tex",    mark_tex_path: "pic/add/mapmark.tex",         marker_flag_start: 0x095e, marker_flag_end: 0x0993 },
];

/// Exact 1:1 floor/wing connectivity table from retail `sh2pc.exe` `0x0088d520`
/// (cross-checked against `SLUS_202.28` `0x0033c7e0`; retail PC links Maps `1..=3` & `32..=35`
/// through Map `36` Baldwin Mansion). Column order: `[Up, Down, Right, Left]`.
#[rustfmt::skip]
pub const CHIZU_CONNECT: [[u8; 4]; MAP_COUNT + 1] = [
    [0, 0, 0, 0],     [2, 3, 9, 36],    [3, 1, 9, 36],    [1, 2, 9, 36],
    [5, 0, 12, 1],    [6, 4, 12, 1],    [0, 5, 12, 1],    [0, 0, 12, 1],
    [0, 0, 12, 1],    [10, 0, 12, 1],   [11, 9, 12, 1],   [0, 10, 12, 1],
    [13, 0, 16, 9],   [14, 12, 16, 9],  [15, 13, 16, 9],  [0, 14, 16, 9],
    [0, 18, 29, 12],  [16, 18, 29, 12], [16, 19, 29, 12], [18, 0, 29, 12],
    [21, 0, 24, 16],  [22, 20, 25, 16], [23, 21, 26, 16], [0, 22, 27, 16],
    [25, 0, 1, 20],   [26, 24, 1, 21],  [27, 25, 1, 22],  [0, 26, 1, 23],
    [29, 0, 32, 16],  [30, 28, 33, 16], [31, 29, 34, 16], [0, 30, 35, 16],
    [33, 0, 36, 28],  [34, 32, 36, 29], [35, 33, 36, 30], [0, 34, 36, 31],
    [0, 0, 1, 33],
];

/// Looks up the static [`MapSheetSpec`] for a 1-based `map_id` (`1..=36`).
#[must_use]
pub fn map_spec(map_id: u8) -> Option<&'static MapSheetSpec> {
    if (1..=(MAP_COUNT as u8)).contains(&map_id) {
        Some(&MAP_SPECS[(map_id - 1) as usize])
    } else {
        None
    }
}

/// Exact 1:1 port of `ChizuPossessionCheck` (`SLUS_202.28` `0x001c7890`, `sh2pc.exe` `0x0049b240`).
#[must_use]
pub fn chizu_possession_check(map_id: u8, flags: &EventFlags) -> bool {
    match map_id {
        1..=3 => flags.get(24),
        4..=6 => flags.get(25),
        7..=8 => flags.get(26),
        9..=11 => flags.get(26) && flags.get(25),
        12..=13 => flags.get(27),
        14..=15 => flags.get(27) && flags.get(215),
        16..=17 => flags.get(28),
        18..=19 => flags.get(29),
        20..=23 => flags.get(30),
        24..=27 => flags.get(476) && flags.get(30),
        28..=31 => flags.get(31),
        32..=35 => flags.get(476) && flags.get(31),
        // Retail sh2pc.exe 0x0049b368: DAT_01f7aa30 & 0x40000000 -> flag 670
        36 => flags.get(670),
        _ => false,
    }
}

/// Returns `true` if `candidate` belongs to the same building group as `origin`
/// (`ChizuConnectCheck` self-loop guard at `SLUS_202.28` `0x001c8d40..0x001c8e2c`,
/// `sh2pc.exe` `0x0049b830..0x0049b8e0`).
const fn is_same_building_group(origin: u8, candidate: u8) -> bool {
    match candidate {
        1 => matches!(origin, 1..=3),
        9 => matches!(origin, 4..=11),
        12 => matches!(origin, 12..=15),
        16 => matches!(origin, 16..=19),
        21 => matches!(origin, 20..=23),
        25 => matches!(origin, 24..=27),
        29 => matches!(origin, 28..=31),
        33 => matches!(origin, 32..=35),
        36 => origin == 36,
        _ => false,
    }
}

/// Exact 1:1 port of `ChizuConnectCheck` (`SLUS_202.28` `0x001c8ce0` / `sh2pc.exe` `0x0049b7c0`).
///
/// Resolves the next owned neighboring map sheet from `current_map_id` in `direction`:
/// - `Up` / `Down` (`0`/`1`): single-hop floor change, requiring ownership of the immediate neighbor.
/// - `Right` / `Left` (`2`/`3`): cross-building walk with exact ownership fallbacks:
///   - Unowned `9` (Combined Apts) falls back to `4` (Wood Side 1F) if flag `25` is set, else
///     `7` or `8` (Blue Creek 1F/2F, matching `selected_map_id == 8`) if flag `26` is set.
///   - Unowned `16` (Prison) falls back to `18` (Labyrinth) if flag `29` is set.
///   - Unowned `29` (Hotel Employee) falls back to `21` (Hotel Guest 1F) if flag `30` or `31` is set.
/// - In Maria's *Born From a Wish* scenario (`is_maria_scenario == true`), `Up`/`Down` to maps
///   `1`/`2` is blocked and `1` redirects to `3` (`sh2pc.exe` `0x0049b805`, `0x0049cef8`).
#[must_use]
pub fn chizu_connect_check(
    current_map_id: u8,
    direction: MapDirection,
    flags: &EventFlags,
    selected_map_id: u8,
    is_maria_scenario: bool,
) -> Option<u8> {
    let dir_idx = direction as usize;
    let mut cur = current_map_id;

    for _ in 0..MAP_COUNT {
        let next = CHIZU_CONNECT.get(cur as usize)?[dir_idx];
        if next == 0 {
            return None;
        }
        if is_maria_scenario
            && matches!(direction, MapDirection::Up | MapDirection::Down)
            && matches!(next, 1 | 2)
        {
            return None;
        }

        if matches!(direction, MapDirection::Up | MapDirection::Down) {
            if chizu_possession_check(next, flags) {
                return Some(if is_maria_scenario && next == 1 {
                    3
                } else {
                    next
                });
            }
            return None;
        }

        // Right / Left cross-building traversal
        if is_same_building_group(current_map_id, next) {
            return None;
        }
        if chizu_possession_check(next, flags) {
            return Some(if is_maria_scenario && next == 1 {
                3
            } else {
                next
            });
        }

        // Exact ownership fallbacks (`SLUS_202.28` `0x001c8e40..0x001c8f60`, `sh2pc.exe` `0x0049b8f8..0x0049b990`)
        match next {
            9 => {
                if flags.get(25) {
                    return Some(4);
                }
                if flags.get(26) {
                    return Some(if selected_map_id == 8 { 8 } else { 7 });
                }
            }
            16 if flags.get(29) => return Some(18),
            29 if flags.get(30) || flags.get(31) => return Some(21),
            _ => {}
        }

        cur = next;
    }
    None
}

/// Convenience wrapper for [`chizu_connect_check`] using default James scenario context.
#[must_use]
pub fn chizu_connect_neighbor(
    current_map_id: u8,
    direction: MapDirection,
    flags: &EventFlags,
) -> Option<u8> {
    chizu_connect_check(current_map_id, direction, flags, current_map_id, false)
}

/// Result of attempting to open the paper map during gameplay (`EventExecChizuFail` @ `0x001aae40`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapReadCheck {
    /// Player owns the map and has enough light to read it.
    Allowed,
    /// Player does not own the map for the current area.
    NoMapOwned,
    /// Room is dark and the pocket flashlight is OFF (`EventExecChizuFail` `0x001aae40`).
    TooDark,
}

/// Evaluates whether James/Maria can open and read `map_id` (`ChizuPossessionCheck` + `EventExecChizuFail`).
#[must_use]
pub fn chizu_can_read_map(
    map_id: u8,
    flags: &EventFlags,
    is_dark_room: bool,
    flashlight_active: bool,
) -> MapReadCheck {
    if !chizu_possession_check(map_id, flags) {
        return MapReadCheck::NoMapOwned;
    }
    if is_dark_room && !flashlight_active {
        return MapReadCheck::TooDark;
    }
    MapReadCheck::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chizu_possession_and_connectivity_fallbacks() {
        let mut flags = EventFlags::new();
        assert!(!chizu_possession_check(1, &flags));
        assert!(!chizu_possession_check(36, &flags));
        assert_eq!(
            chizu_can_read_map(1, &flags, false, false),
            MapReadCheck::NoMapOwned
        );

        flags.set(24, true);
        assert!(chizu_possession_check(1, &flags));
        assert_eq!(
            chizu_can_read_map(1, &flags, true, false),
            MapReadCheck::TooDark
        );
        assert_eq!(
            chizu_can_read_map(1, &flags, true, true),
            MapReadCheck::Allowed
        );
        assert_eq!(chizu_connect_neighbor(1, MapDirection::Up, &flags), Some(2));
        assert_eq!(chizu_connect_neighbor(2, MapDirection::Up, &flags), Some(3));

        // Ownership fallback: from Map 1 Right -> target 9 (Combined Apts), owns only Wood Side (flag 25) -> falls back to 4
        flags.set(25, true);
        assert_eq!(
            chizu_connect_neighbor(1, MapDirection::Right, &flags),
            Some(4)
        );

        // Owns only Blue Creek (flag 26) -> falls back to 7 (or 8 when selected_map_id == 8)
        flags.set(25, false);
        flags.set(26, true);
        assert_eq!(
            chizu_connect_check(1, MapDirection::Right, &flags, 1, false),
            Some(7)
        );
        assert_eq!(
            chizu_connect_check(1, MapDirection::Right, &flags, 8, false),
            Some(8)
        );

        // Owns both Wood Side + Blue Creek -> navigates directly to combined Map 9
        flags.set(25, true);
        assert_eq!(
            chizu_connect_neighbor(1, MapDirection::Right, &flags),
            Some(9)
        );

        // Ownership fallback: unowned Prison (16) falls back to Labyrinth (18) when flag 29 is set
        flags.set(29, true);
        assert_eq!(
            chizu_connect_neighbor(12, MapDirection::Right, &flags),
            Some(18)
        );

        // Ownership fallback: unowned Hotel Employee (29) falls back to Hotel Guest 1F (21) when flag 30 is set
        flags.set(30, true);
        assert_eq!(
            chizu_connect_neighbor(16, MapDirection::Right, &flags),
            Some(21)
        );

        // Map 36 Baldwin Mansion uses retail flag 670
        flags.set(670, true);
        assert!(chizu_possession_check(36, &flags));
    }
}
