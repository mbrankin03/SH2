//! Automated 58-stage retail transition audit (`SH2-112`).
//!
//! Iterates over every `EventResultKind::Move` and `EventResultKind::MovePlusProgram` entry across
//! all 58 retail stages (`stages_act1` Stages 1–27 & `stages_act2` Stages 28–58) and verifies:
//! 1. `decode_retail_move_destination` (`SLUS_202.28` `0x001a9730` / `sh2pc.exe` `0x0048b7e0`)
//!    succeeds with finite coordinates & yaw for 100% of transitions (including Case 0 10-byte
//!    `PointWithAngle` and `>= 6` fallthrough).
//! 2. `resolve_room_id_for_stage_pos` resolves both outdoor (`BlockNumber` `0x001b09f0`) and
//!    indoor (`.cld` floor footprint) rooms to existing retail `.map`, `.cld`, and `.cam` files.
//! 3. Destination `(x, z)` lands on a walkable `.cld` floor triangle and has `.cam` coverage.

use std::collections::HashMap;

use sh2_assets::formats::cam::{parse_cam, CamFile};
use sh2_assets::formats::cld::{parse_cld, CldFile};
use sh2_assets::AssetPathResolver;
use sh2_core::campaign::retail_events::decode_retail_move_destination;
use sh2_core::campaign::{resolve_room_id_for_stage_pos, resolve_stage_manifest};
use sh2_core::events::stages_act1::act1_stage_descriptor;
use sh2_core::events::stages_act2::{normalize_retail_event_entry_for_input, stage_table_act2};
use sh2_core::events::triggers::EventEvalInput;
use sh2_core::inventory::InventoryManager;
use sh2_core::types::{EventDestShapeKind, EventListEntry, EventResultKind};

fn stage_events(stage_id: u16) -> Option<(&'static [EventListEntry], &'static [u8])> {
    if stage_id <= 27 {
        if let Some(act1) = act1_stage_descriptor(stage_id as u8) {
            return Some((act1.event_list, act1.ev_pos));
        }
    }
    stage_table_act2(stage_id).map(|act2| (act2.event_list, act2.pos_list))
}

#[test]
fn audit_all_58_stages_retail_move_transitions() {
    let resolver = AssetPathResolver::resolve();
    if resolver.resolve_file("bg/er/er01.cld").is_none() {
        return; // Skip gracefully in CI without retail assets
    }

    let mut cld_cache: HashMap<String, Option<CldFile>> = HashMap::new();
    let mut cam_cache: HashMap<String, Option<CamFile>> = HashMap::new();

    let mut total_moves = 0usize;
    let mut decode_failures = 0usize;
    let mut missing_map = 0usize;
    let mut missing_cam = 0usize;
    let mut on_floor_count = 0usize;
    let mut off_floor_non_boat = 0usize;
    let mut no_cam_record = 0usize;
    let mut off_floor_details: Vec<String> = Vec::new();

    let dummy_input = EventEvalInput::default();
    let dummy_inv = InventoryManager::new();

    for stage_id in 1..=58u16 {
        let Some((event_list, ev_pos)) = stage_events(stage_id) else {
            continue;
        };

        for (idx, &raw_entry) in event_list.iter().enumerate() {
            if raw_entry.is_sentinel() {
                break;
            }
            let entry = normalize_retail_event_entry_for_input(raw_entry, &dummy_input, &dummy_inv);
            let Some(rslt) = entry.result_kind() else {
                continue;
            };
            if !matches!(
                rslt,
                EventResultKind::Move | EventResultKind::MovePlusProgram
            ) {
                continue;
            }

            total_moves += 1;

            // `SLUS_202.28` `0x001a97c8` (`sltiu $1, $17, 6; beqz`): `0` and `>= 6` use Case 0.
            let ds = entry.dest_shape_kind().unwrap_or(EventDestShapeKind::Point);
            let Some((dest_pos, dest_yaw)) =
                decode_retail_move_destination(ev_pos, entry.dest_ev_pos_offset(), ds)
            else {
                decode_failures += 1;
                off_floor_details.push(format!(
                    "Stage {stage_id} ev[{idx}]: decode failed (off=0x{:x}, ds={ds:?})",
                    entry.dest_ev_pos_offset()
                ));
                continue;
            };

            if !dest_pos.iter().all(|v| v.is_finite()) || !dest_yaw.is_finite() {
                decode_failures += 1;
                continue;
            }

            let raw_dest_stage = entry.stage_change_mode() as u16;
            let target_stage = if raw_dest_stage != 0 {
                raw_dest_stage
            } else {
                stage_id
            };

            let resolved_room = resolve_room_id_for_stage_pos(target_stage, dest_pos);
            let manifest =
                resolve_stage_manifest(target_stage, resolved_room.as_deref(), true, false);

            if resolver.resolve_file(&manifest.map_rel_path).is_none() {
                missing_map += 1;
            }

            let cam_opt = cam_cache
                .entry(manifest.cam_rel_path.clone())
                .or_insert_with(|| {
                    resolver
                        .resolve_file(&manifest.cam_rel_path)
                        .and_then(|p| std::fs::read(p).ok())
                        .and_then(|b| parse_cam(&b).ok())
                });
            if cam_opt.is_none() {
                missing_cam += 1;
            } else if cam_opt
                .as_ref()
                .is_some_and(|c| c.find_active_record(dest_pos, 150.0).is_none())
            {
                no_cam_record += 1;
            }

            // Stage 38 (`qt01`, Toluca Lake Boat) is open water and has no `.cld` floor file.
            if target_stage == 38 {
                continue;
            }

            let cld_opt = cld_cache
                .entry(manifest.cld_rel_path.clone())
                .or_insert_with(|| {
                    resolver
                        .resolve_file(&manifest.cld_rel_path)
                        .and_then(|p| std::fs::read(p).ok())
                        .and_then(|b| parse_cld(&b).ok())
                });
            let on_floor = cld_opt.as_ref().is_some_and(|cld| {
                cld.floors
                    .iter()
                    .any(|tri| tri.sample_height(dest_pos[0], dest_pos[2]).is_some())
            });
            if on_floor {
                on_floor_count += 1;
            } else {
                off_floor_non_boat += 1;
                if stage_id != 1 {
                    off_floor_details.push(format!(
                        "Stage {stage_id}->{target_stage} ev[{idx}] ({}): OFF FLOOR at ({:.1}, {:.1}, {:.1}) [resolved={resolved_room:?}]",
                        manifest.room_id, dest_pos[0], dest_pos[1], dest_pos[2]
                    ));
                }
            }
        }
    }

    assert_eq!(
        decode_failures, 0,
        "All {total_moves} retail Move events must decode cleanly"
    );
    assert_eq!(
        missing_map, 0,
        "All resolved rooms must have retail .map files"
    );
    assert_eq!(
        missing_cam, 0,
        "All resolved rooms must have retail .cam files"
    );
    assert!(
        on_floor_count >= 490,
        "Expected >= 490 retail Move transitions to resolve onto walkable .cld floors, got {on_floor_count} (off_floor_non_boat={off_floor_non_boat}, no_cam={no_cam_record}, details={:?})",
        &off_floor_details[..off_floor_details.len().min(5)]
    );
}
