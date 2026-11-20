//! 1:1 PS2 / Director's Cut Paper Map tables (`Event/chizu.c` in `SLUS_202.28`).
//!
//! # Ground Truth Citations (`SLUS_202.28`)
//! - `ChizuFileLoad` (`0x001c7ac0`): base map texture table `0x0033db10`, mark texture table `0x0033dba0`.
//! - `ChizuPossessionCheck` (`0x001c7890`, jump table `0x003927d0`): exact `EventFlags` bit checks
//!   for maps `1..=35`, plus Map `36` (`data/pic/add/mapboldwin.tex` for Maria's *Born from a Wish*).
//! - `ChizuConnectCheck` (`0x001c8ce0`, connectivity table `0x0033c7e0`): `[up, down, left, right]`
//!   multi-floor & cross-wing transitions.
//! - `ChizuCurrentPositionCheck` (`0x001c95c0..0x001c9620`): default world-to-map scale constants
//!   `0x3aba1f4b` (`0.00142` px/mm X) and `0x3b007358` (`0.00196` px/mm Z).
//! - `EventExecChizuFail` (`0x001aae40`): "It's too dark to read the map here."

use sh2_core::events::EventFlags;

/// Total number of maps across James (`1..=35`) and Maria (`36` = Baldwin Mansion).
pub const MAP_COUNT: usize = 36;

/// Default world-X to PS2-map-px scale (`ChizuCurrentPositionCheck` @ `0x001c95c0`, `0x3aba1f4b`).
pub const CHIZU_WORLD_SCALE_X: f32 = f32::from_bits(0x3aba_1f4b);
/// Default world-Z to PS2-map-px scale (`ChizuCurrentPositionCheck` @ `0x001c9608`, `0x3b007358`).
pub const CHIZU_WORLD_SCALE_Z: f32 = f32::from_bits(0x3b00_7358);

/// Projects world `(x_mm, z_mm)` into normalized `4:3` sheet coordinates `(0.0..=1.0, 0.0..=1.0)`
/// using the exact `ChizuCurrentPositionCheck` (`0x001c95c0..0x001c9620`) PS2 scale factors.
pub fn chizu_world_to_sheet_norm(world_x_mm: f32, world_z_mm: f32) -> (f32, f32) {
    let px = world_x_mm * CHIZU_WORLD_SCALE_X;
    let py = -world_z_mm * CHIZU_WORLD_SCALE_Z;
    let nx = (0.5 + px / 640.0).clamp(0.05, 0.95);
    let ny = (0.5 + py / 480.0).clamp(0.05, 0.95);
    (nx, ny)
}

/// Directional floor/wing navigation on the paper map (`ChizuConnectCheck` @ `0x001c8ce0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapDirection {
    /// Up floor / next sheet (`$5 == 0`, pad `0x400`).
    Up = 0,
    /// Down floor / previous sheet (`$5 == 1`, pad `0x800`).
    Down = 1,
    /// Previous building/wing (`$5 == 2`, pad `0x100`).
    Left = 2,
    /// Next building/wing (`$5 == 3`, pad `0x200`).
    Right = 3,
}

/// Static specification for a single paper map sheet (`1..=36`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSheetSpec {
    /// 1-based map identifier (`1..=36`).
    pub map_id: u8,
    /// Human-readable sheet title & floor label.
    pub title: &'static str,
    /// Relative retail base map texture path (`ChizuFileLoad` `0x0033db10`).
    pub base_tex_path: &'static str,
    /// Relative retail red-pen marker overlay atlas (`ChizuFileLoad` `0x0033dba0`).
    pub mark_tex_path: &'static str,
    /// First `EventFlags` ID in `chizu_marker_list` (`0x0033d8d0`) for this sheet.
    pub marker_flag_start: u16,
    /// Last `EventFlags` ID (inclusive) in `chizu_marker_list` (`0x0033d8d0`) for this sheet.
    pub marker_flag_end: u16,
}

/// All 36 paper map sheets extracted 1:1 from `SLUS_202.28` `0x0033db10` / `0x0033dba0` / `0x0033d8d0`,
/// plus Director's Cut Map `36` (`data/pic/add/mapboldwin.tex`, Maria's Baldwin Mansion).
#[rustfmt::skip]
pub const MAP_SPECS: [MapSheetSpec; MAP_COUNT] = [
    MapSheetSpec { map_id: 1,  title: "Silent Hill (Overall)",           base_tex_path: "pic/map/outmap.tex",        mark_tex_path: "pic/map/outmapmark.tex",      marker_flag_start: 0x0529, marker_flag_end: 0x052d },
    MapSheetSpec { map_id: 2,  title: "South Vale (East)",               base_tex_path: "pic/map/outmape.tex",       mark_tex_path: "pic/map/outmapmark.tex",      marker_flag_start: 0x04e9, marker_flag_end: 0x0502 },
    MapSheetSpec { map_id: 3,  title: "South Vale (West)",               base_tex_path: "pic/map/outmapw.tex",       mark_tex_path: "pic/map/outmapmark.tex",      marker_flag_start: 0x0508, marker_flag_end: 0x051e },
    MapSheetSpec { map_id: 4,  title: "Wood Side Apartments 1F",         base_tex_path: "pic/map/apartmape1f.tex",   mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0533, marker_flag_end: 0x054a },
    MapSheetSpec { map_id: 5,  title: "Wood Side Apartments 2F",         base_tex_path: "pic/map/apartmape2f.tex",   mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0550, marker_flag_end: 0x0574 },
    MapSheetSpec { map_id: 6,  title: "Wood Side Apartments 3F",         base_tex_path: "pic/map/apartmape3f.tex",   mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0576, marker_flag_end: 0x058b },
    MapSheetSpec { map_id: 7,  title: "Blue Creek Apartments 1F",        base_tex_path: "pic/map/apartmapw.tex",     mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0591, marker_flag_end: 0x05a3 },
    MapSheetSpec { map_id: 8,  title: "Blue Creek Apartments 2F",        base_tex_path: "pic/map/apartmapw.tex",     mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x05a7, marker_flag_end: 0x05bc },
    MapSheetSpec { map_id: 9,  title: "Apartments (East & West) 1F",     base_tex_path: "pic/map/apartmapew1f.tex",  mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0591, marker_flag_end: 0x05a3 },
    MapSheetSpec { map_id: 10, title: "Apartments (East & West) 2F",     base_tex_path: "pic/map/apartmapew2f.tex",  mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x05a7, marker_flag_end: 0x05bc },
    MapSheetSpec { map_id: 11, title: "Apartments (East & West) 3F",     base_tex_path: "pic/map/apartmapew3f.tex",  mark_tex_path: "pic/map/apartmapmark.tex",    marker_flag_start: 0x0576, marker_flag_end: 0x058b },
    MapSheetSpec { map_id: 12, title: "Brookhaven Hospital 1F / BF",     base_tex_path: "pic/map/hospitalmap01.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x05c2, marker_flag_end: 0x05f0 },
    MapSheetSpec { map_id: 13, title: "Brookhaven Hospital 2F / 3F / RF",base_tex_path: "pic/map/hospitalmap02.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x05f6, marker_flag_end: 0x063e },
    MapSheetSpec { map_id: 14, title: "Brookhaven Otherworld 1F / BF",   base_tex_path: "pic/map/hospitalmap01.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x0644, marker_flag_end: 0x0673 },
    MapSheetSpec { map_id: 15, title: "Brookhaven Otherworld 2F / 3F",   base_tex_path: "pic/map/hospitalmap02.tex", mark_tex_path: "pic/map/hospitalmapmk.tex",   marker_flag_start: 0x0677, marker_flag_end: 0x06aa },
    MapSheetSpec { map_id: 16, title: "Toluca Prison",                   base_tex_path: "pic/map/prisonmap03.tex",   mark_tex_path: "pic/map/prisonmap03mark.tex", marker_flag_start: 0x06b0, marker_flag_end: 0x06f9 },
    MapSheetSpec { map_id: 17, title: "Toluca Prison (Courtyard)",       base_tex_path: "pic/map/prisonmap03.tex",   mark_tex_path: "pic/map/prisonmap03mark.tex", marker_flag_start: 0x06fb, marker_flag_end: 0x0703 },
    MapSheetSpec { map_id: 18, title: "Labyrinth (Upper)",               base_tex_path: "pic/map/prisonmap.tex",     mark_tex_path: "pic/map/prisonmap01mark.tex", marker_flag_start: 0x07fb, marker_flag_end: 0x0814 },
    MapSheetSpec { map_id: 19, title: "Labyrinth (Lower)",               base_tex_path: "pic/map/prisonmap.tex",     mark_tex_path: "pic/map/prisonmap02mark.tex", marker_flag_start: 0x0815, marker_flag_end: 0x082d },
    MapSheetSpec { map_id: 20, title: "Lakeview Hotel BF (Guest)",       base_tex_path: "pic/map/hotelmapbf01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0709, marker_flag_end: 0x070e },
    MapSheetSpec { map_id: 21, title: "Lakeview Hotel 1F (Guest)",       base_tex_path: "pic/map/hotelmap1f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0721, marker_flag_end: 0x0736 },
    MapSheetSpec { map_id: 22, title: "Lakeview Hotel 2F (Guest)",       base_tex_path: "pic/map/hotelmap2f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x075d, marker_flag_end: 0x077d },
    MapSheetSpec { map_id: 23, title: "Lakeview Hotel 3F (Guest)",       base_tex_path: "pic/map/hotelmap3f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0781, marker_flag_end: 0x078c },
    MapSheetSpec { map_id: 24, title: "Lakeview Otherworld BF",          base_tex_path: "pic/map/hotelmapbf01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0792, marker_flag_end: 0x0795 },
    MapSheetSpec { map_id: 25, title: "Lakeview Otherworld 1F",          base_tex_path: "pic/map/hotelmap1f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x07a7, marker_flag_end: 0x07b1 },
    MapSheetSpec { map_id: 26, title: "Lakeview Otherworld 2F",          base_tex_path: "pic/map/hotelmap2f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x07ca, marker_flag_end: 0x07eb },
    MapSheetSpec { map_id: 27, title: "Lakeview Otherworld 3F",          base_tex_path: "pic/map/hotelmap3f01.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x07ef, marker_flag_end: 0x07f7 },
    MapSheetSpec { map_id: 28, title: "Lakeview Hotel BF (Employee)",    base_tex_path: "pic/map/hotelmapbf02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0712, marker_flag_end: 0x071e },
    MapSheetSpec { map_id: 29, title: "Lakeview Hotel 1F (Employee)",    base_tex_path: "pic/map/hotelmap1f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x073b, marker_flag_end: 0x0757 },
    MapSheetSpec { map_id: 30, title: "Lakeview Hotel 2F (Employee)",    base_tex_path: "pic/map/hotelmap2f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x075d, marker_flag_end: 0x077d },
    MapSheetSpec { map_id: 31, title: "Lakeview Hotel 3F (Employee)",    base_tex_path: "pic/map/hotelmap3f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0781, marker_flag_end: 0x078c },
    MapSheetSpec { map_id: 32, title: "Lakeview Otherworld BF (Emp)",    base_tex_path: "pic/map/hotelmapbf02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x0799, marker_flag_end: 0x07a3 },
    MapSheetSpec { map_id: 33, title: "Lakeview Otherworld 1F (Emp)",    base_tex_path: "pic/map/hotelmap1f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x07b5, marker_flag_end: 0x07c6 },
    MapSheetSpec { map_id: 34, title: "Lakeview Otherworld 2F (Emp)",    base_tex_path: "pic/map/hotelmap2f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x07ca, marker_flag_end: 0x07eb },
    MapSheetSpec { map_id: 35, title: "Lakeview Otherworld 3F (Emp)",    base_tex_path: "pic/map/hotelmap3f02.tex",  mark_tex_path: "pic/map/hotelmapmark.tex",    marker_flag_start: 0x07ef, marker_flag_end: 0x07f7 },
    MapSheetSpec { map_id: 36, title: "Baldwin Mansion (Maria)",         base_tex_path: "pic/add/mapboldwin.tex",    mark_tex_path: "pic/add/mapmark.tex",         marker_flag_start: 0x0830, marker_flag_end: 0x0840 },
];

/// Exact 1:1 floor/wing connectivity table from `SLUS_202.28` `0x0033c7e0` (`[up, down, left, right]`).
#[rustfmt::skip]
pub const CHIZU_CONNECT: [[u8; 4]; MAP_COUNT + 1] = [
    [0, 0, 0, 0],     [2, 3, 9, 29],    [3, 1, 9, 29],    [1, 2, 9, 29],
    [5, 0, 12, 1],    [6, 4, 12, 1],    [0, 5, 12, 1],    [0, 0, 12, 1],
    [0, 0, 12, 1],    [10, 0, 12, 1],   [11, 9, 12, 1],   [0, 10, 12, 1],
    [13, 0, 16, 9],   [14, 12, 16, 9],  [15, 13, 16, 9],  [0, 14, 16, 9],
    [0, 18, 29, 12],  [16, 18, 29, 12], [16, 19, 29, 12], [18, 0, 29, 12],
    [21, 0, 24, 16],  [22, 20, 25, 16], [23, 21, 26, 16], [0, 22, 27, 16],
    [25, 0, 1, 20],   [26, 24, 1, 21],  [27, 25, 1, 22],  [0, 26, 1, 23],
    [29, 0, 32, 16],  [30, 28, 33, 16], [31, 29, 34, 16], [0, 30, 35, 16],
    [33, 0, 1, 28],   [34, 32, 1, 29],  [35, 33, 1, 30],  [0, 34, 1, 31],
    [0, 0, 0, 0],
];

/// Looks up the static [`MapSheetSpec`] for a 1-based `map_id` (`1..=36`).
pub fn map_spec(map_id: u8) -> Option<&'static MapSheetSpec> {
    if (1..=(MAP_COUNT as u8)).contains(&map_id) {
        Some(&MAP_SPECS[(map_id - 1) as usize])
    } else {
        None
    }
}

/// Exact 1:1 port of `ChizuPossessionCheck` (`SLUS_202.28` `0x001c7890`, jump table `0x003927d0`).
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
        36 => flags.get(32) || flags.get(24),
        _ => false,
    }
}

/// Resolves the next owned neighboring map in `direction` (`ChizuConnectCheck` @ `0x001c8ce0`).
pub fn chizu_connect_neighbor(
    current_map_id: u8,
    direction: MapDirection,
    flags: &EventFlags,
) -> Option<u8> {
    let mut cur = current_map_id as usize;
    let dir_idx = direction as usize;
    for _ in 0..MAP_COUNT {
        if cur == 0 || cur > MAP_COUNT {
            return None;
        }
        let next = CHIZU_CONNECT[cur][dir_idx];
        if next == 0 || next == current_map_id {
            return None;
        }
        if chizu_possession_check(next, flags) {
            return Some(next);
        }
        if matches!(direction, MapDirection::Left | MapDirection::Right) {
            cur = next as usize;
        } else {
            return None;
        }
    }
    None
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
    fn test_chizu_possession_and_connectivity_parity() {
        let mut flags = EventFlags::new();
        assert!(!chizu_possession_check(1, &flags));
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

        flags.set(25, true);
        assert!(chizu_possession_check(4, &flags));
        assert!(!chizu_possession_check(9, &flags));
        flags.set(26, true);
        assert!(chizu_possession_check(9, &flags));
        assert_eq!(
            chizu_connect_neighbor(1, MapDirection::Left, &flags),
            Some(9)
        );
    }
}
