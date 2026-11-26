//! Authentic PS2 camera road priority gates, nearest-item search, and hysteresis selection (`SH2-134`).
//!
//! Ground truth:
//! - PS2 `SLUS_202.28`:
//!   - `vcRetPrioStgEvnt` (`0x00294ab0`), `vcRetPrioCbEvnt` (`0x00294b20`),
//!     `vcRetPrioApEvnt` (`0x00294be0`), `vcRetPrioCcEvnt` (`0x00294d10`) in `view/vc_prio.c`
//!   - `vcRetRoadUsePriority` (`0x00197a10`), `vcSetCurNearRoadInVC_WORK` (`0x00197af0`),
//!     `vcAdvantageDistOfOldCurRoad` (`0x00198ae0`), `vcSetNearestItemDataInVC_WORK` (`0x001971a0`) in `view/vc_main.c`
//! - Retail PC `sh2pc.exe`:
//!   - `vcRetPrioStgEvnt` (`0x005227c0`, expanded from SLUS stages `2/3/9` to also gate stages `8/0xc/0xd/0x10`
//!     and additional blocks/rooms in `2/3/9`)
//!   - `vcRetRoadUsePriority` (`0x0051a500`)
//!   - `vcAdvantageDistOfOldCurRoad` (`0x0051a580`, `extra_boundary_width` at `0x006c83b0`)
//!   - `vcSetNearestItemDataInVC_WORK` (`0x0051a410`)

use crate::camera::locus_door::regulate_angle;
use glam::Vec3;
use sh2_assets::formats::cam::{
    RoadRecord, RoadType, VC_RD_MARGE_ROAD_F, VC_RD_NOT_WARP_F, VC_RD_NO_EXTRA_AREA_F,
    VC_RD_USE_NEAR_ENEMY_F, VC_RD_USE_NO_ENEMY_F, VC_RD_WARP_IN_F, VC_RD_WARP_OUT_F,
};
use std::f32::consts::{FRAC_PI_2, PI};

/// Maximum XZ scan radius in PS2 mm for `vcSetNearestItemDataInVC_WORK` (`SLUS_202.28` `0x001971a0`, `sh2pc.exe` `0x0051a410`).
pub const NEAREST_ITEM_MAX_DIST_MM: f32 = 5000.0;
/// Maximum candidate item slots scanned per tick (`0x14 = 20`) in `vcSetNearestItemDataInVC_WORK` (`0x001971a0`).
pub const NEAREST_ITEM_MAX_SLOTS: usize = 20;
/// Player status bit (`_sh2jms + 0x170` bit `0x20` in SLUS / `+0x148` in PC) inhibiting nearest-item camera targeting (`0x001971a0`).
pub const PLAYER_FLAG_INHIBIT_ITEM_CAM: u32 = 0x20;

/// Extra boundary hysteresis width in PS2 mm indexed by `AreaSizeType` (`extra_boundary_width`:
/// `SLUS_202.28` `0x00390600`, `sh2pc.exe` `0x006c83b0`).
pub const EXTRA_BOUNDARY_WIDTH_MM: [f32; 4] = [75.0, 125.0, 175.0, 250.0];
/// Extra boundary hysteresis width in PS2 mm for `RoadType::Effect` (1) and `RoadType::Event` (2) (`0x42480000` = `50.0`, `0x00198ae0`).
pub const EXTRA_BOUNDARY_EFFECT_EVENT_MM: f32 = 50.0;
/// Priority-drop advantage multiplier (`2.0 * advantage`) in `vcSetCurNearRoadInVC_WORK` (`SLUS_202.28` `0x00197af0`).
pub const OLD_ROAD_PRIORITY_ADVANTAGE_SCALE: f32 = 2.0;
/// Travel-angle guard threshold (`20 deg` in radians = `0.34906584`) in `vcSetCurNearRoadInVC_WORK` (`SLUS_202.28` `0x00197af0`).
pub const OLD_ROAD_TRAVEL_ANGLE_GUARD_RAD: f32 = 0.349_065_84;

/// Context for evaluating `vcRetPrioStgEvnt` (`SLUS_202.28` `0x00294ab0`, `sh2pc.exe` `0x005227c0`).
#[derive(Debug, Clone, Copy, Default)]
pub struct StageEventPriorityContext<'a> {
    /// Active stage ID (`_stage + 0x24`: `2` = cb, `3` = cc, `8` = hosp/ow, `9` = ap, `12` = prison/lab, `13` = hotel, `16` = hotel ow/bfaw).
    pub stage_id: i32,
    /// Current outdoor block number from `BlockNumber(pos.x, pos.z)` (used in stages `2` and `3`).
    pub block_number: u16,
    /// Current indoor room ID from `RoomNameJms()` (used in stages `8`, `9`, `12`, `13`, `16`).
    pub room_id: i32,
    /// Player world position in PS2 mm, Y-down (`_sh2jms->pos`, used for spatial splits in rooms `0x18`, `0x21`, `0xbb`).
    pub player_pos: Vec3,
    /// `game_flag` / `flag_work` 32-bit word array (`SLUS_202.28` `0x01126430`, `sh2pc.exe` `0x01f7a9e0`).
    pub flag_words: &'a [u32],
}

impl StageEventPriorityContext<'_> {
    #[inline]
    fn flag_bit(&self, word_index: usize, bit: u32) -> bool {
        self.flag_words
            .get(word_index)
            .is_some_and(|&w| ((w >> bit) & 1) != 0)
    }
}

/// Evaluates whether an `Effect` or `Event` camera road is currently suppressed (`true` = suppressed -> priority `0`)
/// by stage event flags.
///
/// Direct 1:1 port of `vcRetPrioStgEvnt` (`SLUS_202.28` `0x00294ab0` + full retail `sh2pc.exe` `0x005227c0`).
///
/// Prototype-vs-retail drift:
/// - `SLUS_202.28` (`view/vc_prio.c`) only checked stages `2` (`cb`: blocks `0x12`, `0x36`, `0x43`),
///   `3` (`cc`: block `0x29`), and `9` (`ap`: rooms `0x15`, `0x17`, `0x18`, `0x1b`, `0x1e`, `0x1f`, `0x21`).
/// - Retail `sh2pc.exe` (`0x005227c0`) retained all prototype gates, added spatial splits in `ap` rooms
///   `0x18` (`z < 21250.0`) and `0x21` (`y < -1700.0`), added `cb` blocks `0x13/0x14/0x15/0x34/0x2e` and `cc`
///   blocks `10/11/13/14/20/21/23/24`, and added stages `8`, `0xc`, `0xd`, and `0x10`.
pub fn ret_prio_stg_evnt(ctx: &StageEventPriorityContext<'_>) -> bool {
    match ctx.stage_id {
        // Stage 2: East/West South Vale (`cb`, `vcRetPrioCbEvnt` 0x00294b20 / sh2pc 0x005227c0)
        2 => match ctx.block_number {
            0x12 => ctx.flag_bit(1, 0x0f),                // game_flag[1] >> 15
            0x13..=0x15 | 0x34 => !ctx.flag_bit(7, 0x1b), // !(game_flag[7] >> 27) (retail)
            0x2e => ctx.flag_bit(4, 0x15),                // game_flag[4] >> 21 (retail)
            0x36 => ctx.flag_bit(1, 0x0b),                // game_flag[1] >> 11
            0x43 => ctx.flag_bit(3, 0x15),                // game_flag[3] >> 21
            _ => false,
        },
        // Stage 3: South Vale Night (`cc`, `vcRetPrioCcEvnt` 0x00294d10 / sh2pc 0x005227c0)
        3 => match ctx.block_number {
            10 | 11 | 13 | 14 | 0x14 | 0x15 | 0x17 | 0x18 => !ctx.flag_bit(20, 0x1c), // !(game_flag[20] >> 28) (retail)
            0x29 => ctx.flag_bit(5, 8), // game_flag[5] >> 8
            _ => false,
        },
        // Stage 8: Hospital (`sh2pc.exe` 0x005227c0, retail addition)
        8 => {
            if ctx.room_id != 0x0d {
                false
            } else if !ctx.flag_bit(20, 0x1c) {
                true
            } else {
                ctx.flag_bit(20, 0x1d)
            }
        }
        // Stage 9: Wood Side / Blue Creek Apartments (`ap`, `vcRetPrioApEvnt` 0x00294be0 / sh2pc 0x005227c0)
        9 => match ctx.room_id {
            0x15 => ctx.flag_bit(2, 0x1f), // game_flag[2] >> 31
            0x17 => ctx.flag_bit(1, 0x1e), // game_flag[1] >> 30
            0x18 => {
                if ctx.player_pos.z < 21_250.0 {
                    !(!ctx.flag_bit(2, 8) && ctx.flag_bit(2, 5))
                } else {
                    ctx.flag_bit(2, 0x12)
                }
            }
            0x1b => ctx.flag_bit(2, 2),    // game_flag[2] >> 2
            0x1e => ctx.flag_bit(2, 0x18), // game_flag[2] >> 24
            0x1f => ctx.flag_bit(2, 3),    // game_flag[2] >> 3
            0x21 => {
                if ctx.player_pos.y < -1700.0 {
                    ctx.flag_bit(4, 0x12) // game_flag[4] >> 18
                } else {
                    ctx.flag_bit(4, 0x11) // game_flag[4] >> 17 (retail)
                }
            }
            _ => false,
        },
        // Stage 12 (`0xc`): Historical Society / Prison / Labyrinth (`sh2pc.exe` 0x005227c0, retail addition)
        0x0c => match ctx.room_id {
            0x98 => ctx.flag_bit(12, 10),   // game_flag[12] >> 10
            0xa3 => ctx.flag_bit(14, 0x19), // game_flag[14] >> 25
            0xa5 => ctx.flag_bit(12, 2),    // game_flag[12] >> 2
            _ => false,
        },
        // Stage 13 (`0xd`): Lakeview Hotel (`sh2pc.exe` 0x005227c0, retail addition)
        0x0d => {
            if ctx.room_id != 0xbb {
                false
            } else if ctx.player_pos.x > 60_000.0 {
                ctx.flag_bit(15, 0x19) // game_flag[15] >> 25
            } else {
                !ctx.flag_bit(15, 0x1b) // !(game_flag[15] >> 27)
            }
        }
        // Stage 16 (`0x10`): Lakeview Hotel Otherworld (`sh2pc.exe` 0x005227c0, retail addition)
        0x10 => match ctx.room_id {
            0xc0 => ctx.flag_bit(21, 5),    // game_flag[21] >> 5
            0xc2 => ctx.flag_bit(22, 0x0b), // game_flag[22] >> 11
            0xc5 => ctx.flag_bit(22, 2),    // game_flag[22] >> 2
            0xc7 => ctx.flag_bit(22, 7),    // game_flag[22] >> 7
            _ => false,
        },
        _ => false,
    }
}

/// Evaluates road selection priority for `rd_type`.
///
/// Direct 1:1 port of `vcRetRoadUsePriority` (`SLUS_202.28` `0x00197a10`, `sh2pc.exe` `0x0051a500`).
pub fn ret_road_use_priority(
    rd_type: RoadType,
    search_button_pressed: bool,
    stg_evnt_ctx: Option<&StageEventPriorityContext<'_>>,
) -> u8 {
    match rd_type {
        RoadType::Road => 2,
        RoadType::Effect => {
            let suppressed = stg_evnt_ctx.is_some_and(ret_prio_stg_evnt);
            if suppressed {
                0
            } else {
                5
            }
        }
        RoadType::Event => {
            let suppressed = stg_evnt_ctx.is_some_and(ret_prio_stg_evnt);
            if suppressed {
                0
            } else {
                6
            }
        }
        RoadType::RoadPrioLow => 1,
        RoadType::RoadPrioHigh => 3,
        RoadType::SelfViewOnly => {
            if search_button_pressed {
                4
            } else {
                0
            }
        }
    }
}

/// Candidate world item for `vcSetNearestItemDataInVC_WORK` (`SLUS_202.28` `0x001971a0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraItemCandidate {
    /// Stable caller-supplied item identifier.
    pub id: u32,
    /// Item world position in PS2 mm, Y-down.
    pub pos: Vec3,
    /// Item vertical focus height in PS2 mm (`+0xa4` in `SubCharacter` / `item`).
    pub center_y: f32,
}

/// Result of `vcSetNearestItemDataInVC_WORK` (`SLUS_202.28` `0x001971a0`, `sh2pc.exe` `0x0051a410`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestItemData {
    /// Selected item ID, or `None` if no item lies within `5000.0` mm or targeting is inhibited.
    pub item_id: Option<u32>,
    /// Horizontal XZ distance in PS2 mm (defaults to `5000.0` mm when no item is selected).
    pub dist_mm: f32,
    /// Vertical focus height in PS2 mm, Y-down (if an item was selected).
    pub center_y: Option<f32>,
}

impl Default for NearestItemData {
    fn default() -> Self {
        Self {
            item_id: None,
            dist_mm: NEAREST_ITEM_MAX_DIST_MM,
            center_y: None,
        }
    }
}

/// Scans up to `20` (`0x14`) nearby world items within `5000.0` mm XZ of `player_pos`.
///
/// Direct 1:1 port of `vcSetNearestItemDataInVC_WORK` (`SLUS_202.28` `0x001971a0`, `sh2pc.exe` `0x0051a410`).
pub fn find_nearest_item_data(
    player_pos: Vec3,
    player_status_flags: u32,
    candidates: &[CameraItemCandidate],
) -> NearestItemData {
    if (player_status_flags & PLAYER_FLAG_INHIBIT_ITEM_CAM) != 0 {
        return NearestItemData::default();
    }

    let mut best_id: Option<u32> = None;
    let mut best_dist = NEAREST_ITEM_MAX_DIST_MM;
    let mut best_center_y: Option<f32> = None;

    for item in candidates.iter().take(NEAREST_ITEM_MAX_SLOTS) {
        let dx = item.pos.x - player_pos.x;
        let dz = item.pos.z - player_pos.z;
        if dx.abs() < NEAREST_ITEM_MAX_DIST_MM && dz.abs() < NEAREST_ITEM_MAX_DIST_MM {
            let dist = dx.hypot(dz);
            if dist < best_dist {
                best_dist = dist;
                best_id = Some(item.id);
                best_center_y = Some(item.center_y);
            }
        }
    }

    NearestItemData {
        item_id: best_id,
        dist_mm: best_dist,
        center_y: best_center_y,
    }
}

/// Computes the hysteresis advantage distance in PS2 mm for keeping the current camera road.
///
/// Direct 1:1 port of `vcAdvantageDistOfOldCurRoad` (`SLUS_202.28` `0x00198ae0`, `sh2pc.exe` `0x0051a580`).
pub fn advantage_dist_of_old_cur_road(rec: &RoadRecord) -> f32 {
    if (rec.flags & VC_RD_NO_EXTRA_AREA_F) != 0 {
        return 0.0;
    }
    match rec.rd_type {
        RoadType::Effect | RoadType::Event => EXTRA_BOUNDARY_EFFECT_EVENT_MM,
        RoadType::Road
        | RoadType::RoadPrioLow
        | RoadType::RoadPrioHigh
        | RoadType::SelfViewOnly => EXTRA_BOUNDARY_WIDTH_MM[rec.area_size_type as usize],
    }
}

/// Candidate road entry with its signed XZ distance from `lim_sw` (`<= 0` inside, `> 0` outside)
/// and evaluated priority (`vcRetRoadUsePriority`).
#[derive(Debug, Clone, Copy)]
pub struct RoadSelectionCandidate<'a> {
    /// Index of the record inside `CamFile::records`.
    pub index: usize,
    /// Borrowed `.cam` road record.
    pub record: &'a RoadRecord,
    /// Evaluated priority from `vcRetRoadUsePriority`.
    pub priority: u8,
    /// Signed XZ sum-distance from `lim_sw` in PS2 mm (`<= 0.0` inside, `> 0.0` outside).
    pub signed_dist_mm: f32,
}

/// Outcome of `vcSetCurNearRoadInVC_WORK` hysteresis arbitration (`SLUS_202.28` `0x00197af0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadSwitchDecision {
    /// Winning road record index in `CamFile::records`.
    pub selected_index: usize,
    /// `true` if the active road changed on this tick.
    pub switched: bool,
    /// `true` if `VC_RD_WARP_OUT_F` / `VC_RD_WARP_IN_F` triggered a hard cut (and neither road set `VC_RD_NOT_WARP_F`).
    pub warp_cut: bool,
}

/// Returns `true` when a `.cam` road record is excluded by enemy-presence flags
/// (`SLUS_202.28` `vcSetNearRoadAryByCharaPos` `0x00197310`, `vc_main.c:998-1004`).
///
/// - When no enemy is nearby (`!has_nearest_enemy`): records with `VC_RD_USE_NEAR_ENEMY_F` (`0x40`) are skipped.
/// - When an enemy is nearby (`has_nearest_enemy`): records with `VC_RD_USE_NO_ENEMY_F` (`0x20`) are skipped.
#[inline]
pub fn is_road_excluded_by_enemy_flags(flags: u16, has_nearest_enemy: bool) -> bool {
    if has_nearest_enemy {
        (flags & VC_RD_USE_NO_ENEMY_F) != 0
    } else {
        (flags & VC_RD_USE_NEAR_ENEMY_F) != 0
    }
}

/// Computes oriented local-frame distance from a road's `lim_sw` volume plus vertical Y-excess
/// (`SLUS_202.28` `vcSetNearRoadAryByCharaPos` `0x00197310`, `vc_main.c:1122-1138` &
/// `vcGetXZSumDistFromLimArea` `0x0019d980`, `vc_main.c:4026-4081`).
///
/// - Transforms `(pos.x, pos.z)` into `rec.lim_sw`'s oriented local frame (`world_to_local_xz`).
/// - Gates negative inside depth on `VC_RD_MARGE_ROAD_F` (`0x80`); otherwise clamps inside XZ distance to `0.0`.
/// - Adds vertical Y-excess outside `[min_y, max_y]` instead of hard-rejecting the road.
pub fn signed_xz_dist_from_lim_sw(rec: &RoadRecord, pos: Vec3) -> f32 {
    let merge_road = (rec.flags & VC_RD_MARGE_ROAD_F) != 0;
    let (xz_dist, _, _) = rec
        .lim_sw
        .xz_sum_dist_from_lim_area(pos.x, pos.z, merge_road);
    let (min_y, max_y) = rec.lim_sw.y_bounds();
    let y_excess = if pos.y > max_y {
        pos.y - max_y
    } else if pos.y < min_y {
        min_y - pos.y
    } else {
        0.0
    };
    xz_dist + y_excess
}

/// Evaluates whether the camera should keep `old_road` or switch to `best_new_road`.
///
/// Direct 1:1 port of `vcSetCurNearRoadInVC_WORK` (`SLUS_202.28` `0x00197af0`):
/// - Keeps `old_road` when `old_prio > new_prio` and `old_dist < 2.0 * advantage`.
/// - Switches immediately when `new_prio > old_prio` and `new_dist <= 0.0`.
/// - Switches immediately when `old_road` sets `VC_RD_NO_EXTRA_AREA_F` (`0x100`) and `mv_y_type` toggles `0 <-> 5`.
/// - Keeps `old_road` when `old_dist - advantage <= new_dist`.
/// - Keeps `old_road` when still inside (`old_dist < 0.0`) and travel angle relative to `rd_dir_type` is `< 20 deg` (`0.34906584` rad).
/// - Suppresses `warp_cut` when either `old_road` or `best_new_road` sets `VC_RD_NOT_WARP_F` (`0x400`).
pub fn resolve_cur_near_road_hysteresis(
    old_road: Option<RoadSelectionCandidate<'_>>,
    best_new: RoadSelectionCandidate<'_>,
    chara_mv_ang_y: f32,
) -> RoadSwitchDecision {
    let Some(old) = old_road else {
        return RoadSwitchDecision {
            selected_index: best_new.index,
            switched: true,
            warp_cut: (best_new.record.flags & VC_RD_WARP_IN_F) != 0
                && (best_new.record.flags & VC_RD_NOT_WARP_F) == 0,
        };
    };

    if old.index == best_new.index {
        return RoadSwitchDecision {
            selected_index: old.index,
            switched: false,
            warp_cut: false,
        };
    }

    let adv = advantage_dist_of_old_cur_road(old.record);
    let mut should_switch = false;

    if old.priority <= best_new.priority
        || old.signed_dist_mm >= adv * OLD_ROAD_PRIORITY_ADVANTAGE_SCALE
    {
        let prio_immediate = best_new.priority > old.priority && best_new.signed_dist_mm <= 0.0;
        let no_extra_mv_y_toggle = (old.record.flags & VC_RD_NO_EXTRA_AREA_F) != 0
            && ((old.record.mv_y_type == 5 && best_new.record.mv_y_type == 0)
                || (old.record.mv_y_type == 0 && best_new.record.mv_y_type == 5));
        if prio_immediate || no_extra_mv_y_toggle {
            should_switch = true;
        } else if old.signed_dist_mm - adv > best_new.signed_dist_mm {
            let dir_ang = if old.record.rd_dir_type == 1 {
                FRAC_PI_2
            } else {
                0.0
            };
            let mut travel_ang = regulate_angle(chara_mv_ang_y - dir_ang);
            if travel_ang < 0.0 {
                travel_ang += PI;
            }
            if travel_ang > FRAC_PI_2 {
                travel_ang = PI - travel_ang;
            }
            if old.signed_dist_mm >= 0.0 || travel_ang >= OLD_ROAD_TRAVEL_ANGLE_GUARD_RAD {
                should_switch = true;
            }
        }
    }

    if !should_switch {
        return RoadSwitchDecision {
            selected_index: old.index,
            switched: false,
            warp_cut: false,
        };
    }

    let mut warp_cut = (old.record.flags & VC_RD_WARP_OUT_F) != 0
        || (best_new.record.flags & VC_RD_WARP_IN_F) != 0;
    if (best_new.record.flags & VC_RD_NOT_WARP_F) != 0 || (old.record.flags & VC_RD_NOT_WARP_F) != 0
    {
        warp_cut = false;
    }

    RoadSwitchDecision {
        selected_index: best_new.index,
        switched: true,
        warp_cut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::cam::{
        AreaSizeType, CamMovementParams, CamMovementType, ChaseCamParam, RoadArea,
    };

    fn make_record(
        min_x: f32,
        max_x: f32,
        rd_type: RoadType,
        area_size_type: AreaSizeType,
        flags: u16,
    ) -> RoadRecord {
        RoadRecord {
            lim_sw: RoadArea {
                x0: min_x,
                z0: -1000.0,
                x1: min_x,
                z1: 1000.0,
                x2: max_x,
                z2: 1000.0,
                min_hy: -2000.0,
                max_hy: 2000.0,
            },
            lim_rd: RoadArea::default(),
            kind_id: 0,
            flags,
            area_size_type,
            rd_type,
            mv_y_type: 0,
            ofs_watch_hy: -300.0,
            trace_btm_hy: 0.0,
            rd_dir_type: 0,
            projection: 448.0,
            proj_volume: 0.0,
            proj_sec: 0.0,
            cam_mv_type: CamMovementType::Chase,
            params: CamMovementParams::Chase(ChaseCamParam::default()),
        }
    }

    #[test]
    fn test_ret_prio_stg_evnt_cb_ap_cc_and_retail_stages() {
        let mut flags = [0u32; 24];
        // cb block 0x12 checks game_flag[1] bit 15
        let ctx_cb = StageEventPriorityContext {
            stage_id: 2,
            block_number: 0x12,
            room_id: 0,
            player_pos: Vec3::ZERO,
            flag_words: &flags,
        };
        assert!(!ret_prio_stg_evnt(&ctx_cb));
        assert_eq!(
            ret_road_use_priority(RoadType::Event, false, Some(&ctx_cb)),
            6
        );

        flags[1] |= 1 << 0x0f;
        let ctx_cb_set = StageEventPriorityContext {
            stage_id: 2,
            block_number: 0x12,
            room_id: 0,
            player_pos: Vec3::ZERO,
            flag_words: &flags,
        };
        assert!(ret_prio_stg_evnt(&ctx_cb_set));
        assert_eq!(
            ret_road_use_priority(RoadType::Event, false, Some(&ctx_cb_set)),
            0
        );

        // cc block 0x29 checks game_flag[5] bit 8
        flags[5] |= 1 << 8;
        let ctx_cc = StageEventPriorityContext {
            stage_id: 3,
            block_number: 0x29,
            room_id: 0,
            player_pos: Vec3::ZERO,
            flag_words: &flags,
        };
        assert!(ret_prio_stg_evnt(&ctx_cc));

        // ap room 0x15 checks game_flag[2] bit 31
        flags[2] |= 1 << 31;
        let ctx_ap = StageEventPriorityContext {
            stage_id: 9,
            block_number: 0,
            room_id: 0x15,
            player_pos: Vec3::ZERO,
            flag_words: &flags,
        };
        assert!(ret_prio_stg_evnt(&ctx_ap));
    }

    #[test]
    fn test_find_nearest_item_data_radius_and_inhibit() {
        let items = [
            CameraItemCandidate {
                id: 1,
                pos: Vec3::new(5001.0, 0.0, 0.0),
                center_y: -500.0,
            },
            CameraItemCandidate {
                id: 2,
                pos: Vec3::new(1200.0, 0.0, 1600.0),
                center_y: -400.0,
            },
            CameraItemCandidate {
                id: 3,
                pos: Vec3::new(300.0, 0.0, 400.0),
                center_y: -300.0,
            },
        ];
        let found = find_nearest_item_data(Vec3::ZERO, 0, &items);
        assert_eq!(found.item_id, Some(3));
        assert!((found.dist_mm - 500.0).abs() < 1e-3);
        assert_eq!(found.center_y, Some(-300.0));

        let inhibited = find_nearest_item_data(Vec3::ZERO, PLAYER_FLAG_INHIBIT_ITEM_CAM, &items);
        assert_eq!(inhibited.item_id, None);
        assert_eq!(inhibited.dist_mm, NEAREST_ITEM_MAX_DIST_MM);
    }

    #[test]
    fn test_vc_set_cur_near_road_hysteresis_rules() {
        let old_ev = make_record(-1000.0, 0.0, RoadType::Event, AreaSizeType::Wide, 0);
        let new_rd = make_record(
            0.0,
            1000.0,
            RoadType::Road,
            AreaSizeType::Wide,
            VC_RD_WARP_IN_F,
        );

        // 1. Advantage for Event is 50 mm -> 2.0 * adv = 100 mm; at old_dist = 80 mm (<100), keep old Event!
        let hold_prio = resolve_cur_near_road_hysteresis(
            Some(RoadSelectionCandidate {
                index: 0,
                record: &old_ev,
                priority: 6,
                signed_dist_mm: 80.0,
            }),
            RoadSelectionCandidate {
                index: 1,
                record: &new_rd,
                priority: 2,
                signed_dist_mm: -50.0,
            },
            0.0,
        );
        assert!(!hold_prio.switched);

        // 2. Travel-angle guard: equal priority, still inside old (-20 mm) while new is deeper (-300 mm),
        //    traveling parallel to road (ang = 0.1 rad < 20 deg) -> keep old!
        let old_rd = make_record(-1000.0, 0.0, RoadType::Road, AreaSizeType::Tiny, 0);
        let hold_ang = resolve_cur_near_road_hysteresis(
            Some(RoadSelectionCandidate {
                index: 0,
                record: &old_rd,
                priority: 2,
                signed_dist_mm: -20.0,
            }),
            RoadSelectionCandidate {
                index: 1,
                record: &new_rd,
                priority: 2,
                signed_dist_mm: -300.0,
            },
            0.1,
        );
        assert!(!hold_ang.switched);

        // 3. Same geometry, but traveling >= 20 deg (0.5 rad) -> switches and triggers warp_cut!
        let switch_ang = resolve_cur_near_road_hysteresis(
            Some(RoadSelectionCandidate {
                index: 0,
                record: &old_rd,
                priority: 2,
                signed_dist_mm: -20.0,
            }),
            RoadSelectionCandidate {
                index: 1,
                record: &new_rd,
                priority: 2,
                signed_dist_mm: -300.0,
            },
            0.5,
        );
        assert!(switch_ang.switched);
        assert!(switch_ang.warp_cut);

        // 4. VC_RD_NO_EXTRA_AREA_F removes extra boundary advantage (adv == 0.0)
        let no_extra = make_record(
            -1000.0,
            0.0,
            RoadType::Road,
            AreaSizeType::Outdoor,
            VC_RD_NO_EXTRA_AREA_F,
        );
        assert_eq!(advantage_dist_of_old_cur_road(&no_extra), 0.0);

        // 5. `vcGetXZSumDistFromLimArea` (`0x0019d980`) returns L1 sum `dx + dz` outside corners
        // `new_rd` has `lim_sw` X=[0, 1000], Z=[-1000, 1000]: at (1300, 0, 1400), dx=300, dz=400 -> 700 (not hypot 500)
        let corner_dist = signed_xz_dist_from_lim_sw(&new_rd, Vec3::new(1300.0, 0.0, 1400.0));
        assert!((corner_dist - 700.0).abs() < 1e-4);
    }
}
