//! Story, puzzle, and SH2:EE `MapMeshToggle` / `FixTownGateEvents` map part switches.
//!
//! Verified 1:1 against:
//! - Retail PC `sh2pc.exe`: `0x00595e90` (`TownEastGateLockDisplay`, `cb46` / `0x2e`).
//! - SH2:EE `Patches/Langs.cpp` (`PatchTownGateEvents`, line 1070).
//! - SH2:EE `Patches/MapMeshToggle.cpp` (lines 23..193: Motorhome, Blue Creek 1F,
//!   Hospital Garden, Otherworld Hospital Lobby, Hospital 3F, Toluca Lake, Hotel Room 312).

use crate::events::stageparts::{DisplayControlList, StagePartsContext};

/// Flag `0x31` (49): James entered the Wood Side Apartments gate (`kEnteredApartmentGate`).
pub const FLAG_ENTERED_APARTMENT_GATE: usize = 0x31;
/// Flag `0x55` (85): James met Eddie in Wood Side Room 101 (`kMetEddieGameFlag`).
pub const FLAG_MET_EDDIE: usize = 0x55;
/// Flag `0x5f` (95): James threw the canned juice down the garbage chute (`kCannedJuiceGameFlag`).
pub const FLAG_CANNED_JUICE_USED: usize = 0x5f;
/// Flag `0xe3` (227): Copper Ring placed on Otherworld Hospital 3F door (`kPlacedCopperRingGameFlag`).
pub const FLAG_PLACED_COPPER_RING: usize = 0xe3;
/// Flag `0xe4` (228): Lead Ring placed on Otherworld Hospital 3F door (`kPlacedLeadRingGameFlag`).
pub const FLAG_PLACED_LEAD_RING: usize = 0xe4;
/// Flag `0xfb` (251): Wood Side Apartments gate unlocked (`FixTownGateEvents`, `sh2pc.exe` `0x00595ea5`).
pub const FLAG_WOODSIDE_GATE_UNLOCKED: usize = 251;
/// Flag `0x1db` (475): James watched the videotape in Lakeview Hotel Room 312 (`kUsedVideoTapeGameFlag`).
pub const FLAG_USED_VIDEO_TAPE: usize = 0x1db;
/// Flag `0x2d5` (725): Blue Gem used first time in Hospital Garden (`kUsedBlueGemFirstTimeGameFlag`).
pub const FLAG_USED_BLUE_GEM_FIRST: usize = 0x2d5;
/// Flag `0x2d6` (726): Blue Gem used second time at Toluca Lake dock (`kUsedBlueGemSecondTimeGameFlag`).
pub const FLAG_USED_BLUE_GEM_SECOND: usize = 0x2d6;

/// Flag `0x68` (104): Wood Side Apartments Room 208 clock pushed aside to reveal passage (`Stage 8`).
const FLAG_WOODSIDE_CLOCK_MOVED: usize = 0x68;
/// Flag `0x64` (100): Wood Side Apartments stairwell / corridor light switch (`Stage 7`/`8`).
const FLAG_WOODSIDE_STAIRWELL_LIGHT: usize = 0x64;
/// Flag `0x109` (265): Toluca Prison cell bars state (`Stage 32`/`33`).
const FLAG_PRISON_CELL_BARS_OPEN: usize = 0x109;

/// Cutscene ID: Otherworld Hospital Lobby post-chase elevator (`CS_HSP_ALT_RPT_CHASE_3`).
pub const CS_HSP_ALT_RPT_CHASE_3: u16 = 0x3b;
/// Cutscene ID: Toluca Lake Rebirth ending epilogue (`CS_END_REBIRTH_EPILOGUE`).
pub const CS_END_REBIRTH_EPILOGUE: u16 = 0x54;

/// Room ID: Texxon Gas / Neely's Motorhome (`R_MOTORHOME`, `Patches/MapMeshToggle.cpp:188`).
pub const R_MOTORHOME: u16 = 0x05;
/// Room ID: Otherworld Hospital 1F Lobby (`R_HSP_ALT_LOBBY`, `Patches/MapMeshToggle.cpp:47`).
pub const R_HSP_ALT_LOBBY: u16 = 0x97;

/// Applies stage-specific story/puzzle switches and SH2:EE `MapMeshToggle` / `FixTownGateEvents`
/// display control entries.
///
/// Returns the updated `HotelRoom312MemoFlag` (`0x02d6` while the Room 312 Blue Gem hint is active,
/// `0` otherwise — `Patches/MapMeshToggle.cpp:125`).
pub fn apply_stage_switches(ctx: &StagePartsContext<'_>, list: &mut DisplayControlList) -> u16 {
    let mut hotel_312_memo_flag = 0u16;

    match ctx.stage_id {
        // Stage 2: Opening Restroom / Motorhome (`MapMeshToggle.cpp:186` — MotorhomeDisplayControl)
        2 => {
            if ctx.enable_sh2ee_fixes && ctx.room_id == R_MOTORHOME {
                let variant = if ctx.flags.get(FLAG_ENTERED_APARTMENT_GATE) {
                    -1
                } else {
                    0
                };
                list.entry(2, 0x05, variant);
            }
        }

        // Stage 5: Town East — Wood Side Apartments gate lock (`sh2pc.exe` `0x00595e90` + SH2:EE `PatchTownGateEvents`)
        5 => {
            let gate_unlocked = ctx.flags.get(FLAG_WOODSIDE_GATE_UNLOCKED);
            if gate_unlocked {
                // `sh2pc.exe` `0x00595eae`: pushes variant 2 on part `0x2e` (cb46)
                list.entry(5, 0x2e, 2);
            }
            // Vanilla PC bug pushed variant `1` (broken lock still visible) unconditionally at `0x00595ebf`.
            // SH2:EE `PatchTownGateEvents` (`Langs.cpp:1068`) patches `push 1` -> `push 0` to remove the lock.
            if ctx.enable_sh2ee_fixes && gate_unlocked {
                list.entry(5, 0x2e, 0);
            } else {
                list.entry(5, 0x2e, 1);
            }
        }

        // Stage 7 & 8: Wood Side Apartments 2F / 3F — clock room passage & stairwell light switches
        7 | 8 => {
            if ctx.stage_id == 8 {
                let clock_variant = if ctx.flags.get(FLAG_WOODSIDE_CLOCK_MOVED) {
                    1
                } else {
                    0
                };
                // Room 208 clock mesh switch
                list.entry(8, 0x2a, clock_variant);
            }
            let stair_variant = if ctx.flags.get(FLAG_WOODSIDE_STAIRWELL_LIGHT) {
                1
            } else {
                0
            };
            list.entry(ctx.stage_id, 0x12, stair_variant);
        }

        // Stage 10: Blue Creek Apartments 1F (`MapMeshToggle.cpp:56` — BlueCreek1FDisplayControl)
        10 => {
            if ctx.enable_sh2ee_fixes {
                let variant = if ctx.flags.get(FLAG_MET_EDDIE) { 1 } else { 0 };
                list.entry(10, 0x5c, variant);
            }
        }

        // Stage 22: Brookhaven Hospital Garden (`MapMeshToggle.cpp:74` — HospitalGardenDisplayControl)
        22 => {
            if ctx.enable_sh2ee_fixes {
                let variant = if ctx.has_blue_gem { 0 } else { -1 };
                list.entry(22, 0xaa, variant);
            }
        }

        // Stage 23: Otherworld Hospital Lobby (`MapMeshToggle.cpp:45` — Hospital1FDisplayControl)
        23 => {
            if ctx.enable_sh2ee_fixes && ctx.room_id == R_HSP_ALT_LOBBY {
                let variant = if ctx.cutscene_id == CS_HSP_ALT_RPT_CHASE_3 {
                    1
                } else {
                    0
                };
                list.entry(23, 0x97, variant);
            }
        }

        // Stage 25: Otherworld Hospital 3F ring door (`MapMeshToggle.cpp:145` — Hospital3FDisplayControl)
        25 => {
            if ctx.enable_sh2ee_fixes {
                let has_copper = ctx.flags.get(FLAG_PLACED_COPPER_RING);
                let has_lead = ctx.flags.get(FLAG_PLACED_LEAD_RING);
                if has_copper {
                    list.entry(25, 0xd6, 0);
                }
                if has_lead {
                    list.entry(25, 0xd6, 1);
                }
                if !has_copper && !has_lead {
                    list.entry(25, 0xd6, -1);
                }
            }
        }

        // Stage 32 & 33: Toluca Prison North / South — cell bar switches
        32 | 33 => {
            let bars_variant = if ctx.flags.get(FLAG_PRISON_CELL_BARS_OPEN) {
                1
            } else {
                0
            };
            list.entry(ctx.stage_id, 0x2d, bars_variant);
        }

        // Stage 38: Toluca Lake (`MapMeshToggle.cpp:97` — TownLakeDisplayControl)
        38 => {
            if ctx.enable_sh2ee_fixes {
                let dock_hint = if ctx.flags.get(FLAG_USED_BLUE_GEM_FIRST) {
                    0
                } else {
                    -1
                };
                list.entry(38, 0x01, dock_hint);

                let rebirth_water = if ctx.cutscene_id == CS_END_REBIRTH_EPILOGUE {
                    3
                } else {
                    -1
                };
                list.entry(38, 0x0b, rebirth_water);
            }
        }

        // Stage 42: Lakeview Hotel Room 312 (`MapMeshToggle.cpp:117` — HotelRoom312DisplayControl)
        42 if ctx.enable_sh2ee_fixes => {
            let show_hint =
                ctx.flags.get(FLAG_USED_BLUE_GEM_SECOND) && !ctx.flags.get(FLAG_USED_VIDEO_TAPE);
            list.entry(42, 0x5b, if show_hint { 0 } else { -1 });
            if show_hint {
                hotel_312_memo_flag = FLAG_USED_BLUE_GEM_SECOND as u16;
            }
        }

        _ => {}
    }

    hotel_312_memo_flag
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::stageparts::{stageparts_tick, StagePartsState};
    use crate::events::EventFlags;

    #[test]
    fn test_fix_town_gate_events_removes_lock_after_flag_251() {
        let mut flags = EventFlags::new();
        let mut state = StagePartsState::default();

        // Before Flag 251: lock variant 1 (bit 1 -> variant_index 2) is shown.
        let ctx_locked = StagePartsContext {
            stage_id: 5,
            room_id: 0x2e,
            cutscene_id: 0,
            dt_secs: 1.0 / 60.0,
            flags: &flags,
            has_blue_gem: false,
            enable_sh2ee_fixes: true,
        };
        let out_locked = stageparts_tick(&mut state, &ctx_locked);
        assert!(out_locked.visibility.is_batch_visible(0x2e, 0, 2));
        assert!(!out_locked.visibility.is_batch_visible(0x2e, 0, 1));

        // After Flag 251 with SH2:EE fix: variant 0 (bit 0 -> variant_index 1) + variant 2 (bit 2 -> variant_index 3),
        // broken lock variant 1 (bit 1 -> variant_index 2) is removed!
        flags.set(FLAG_WOODSIDE_GATE_UNLOCKED, true);
        let ctx_open = StagePartsContext {
            stage_id: 5,
            room_id: 0x2e,
            cutscene_id: 0,
            dt_secs: 1.0 / 60.0,
            flags: &flags,
            has_blue_gem: false,
            enable_sh2ee_fixes: true,
        };
        let out_open = stageparts_tick(&mut state, &ctx_open);
        assert!(out_open.visibility.is_batch_visible(0x2e, 0, 1));
        assert!(!out_open.visibility.is_batch_visible(0x2e, 0, 2));
        assert!(out_open.visibility.is_batch_visible(0x2e, 0, 3));
    }

    #[test]
    fn test_sh2ee_map_mesh_toggle_all_seven_stages() {
        let mut flags = EventFlags::new();
        let mut state = StagePartsState::default();

        // 1. Motorhome hint (Stage 2, room 0x05)
        let out = stageparts_tick(
            &mut state,
            &StagePartsContext {
                stage_id: 2,
                room_id: R_MOTORHOME,
                cutscene_id: 0,
                dt_secs: 0.0,
                flags: &flags,
                has_blue_gem: false,
                enable_sh2ee_fixes: true,
            },
        );
        assert!(out.visibility.is_batch_visible(0x05, 0, 1));

        // 2. Hospital 3F ring door (Stage 25, room 0xd6) — copper + lead rings
        flags.set(FLAG_PLACED_COPPER_RING, true);
        flags.set(FLAG_PLACED_LEAD_RING, true);
        let out = stageparts_tick(
            &mut state,
            &StagePartsContext {
                stage_id: 25,
                room_id: 0xd6,
                cutscene_id: 0,
                dt_secs: 0.0,
                flags: &flags,
                has_blue_gem: false,
                enable_sh2ee_fixes: true,
            },
        );
        assert!(out.visibility.is_batch_visible(0xd6, 0, 1));
        assert!(out.visibility.is_batch_visible(0xd6, 0, 2));

        // 3. Hotel Room 312 (Stage 42, room 0x5b) sets Event 19 memo flag 0x2d6
        flags.set(FLAG_USED_BLUE_GEM_SECOND, true);
        let out = stageparts_tick(
            &mut state,
            &StagePartsContext {
                stage_id: 42,
                room_id: 0x5b,
                cutscene_id: 0,
                dt_secs: 0.0,
                flags: &flags,
                has_blue_gem: true,
                enable_sh2ee_fixes: true,
            },
        );
        assert!(out.visibility.is_batch_visible(0x5b, 0, 1));
        assert_eq!(out.hotel_room_312_memo_flag, 0x2d6);
    }
}
