//! Retail `sh2pc.exe` 10-byte `ev_pos` spatial trigger & `EventExecMove` evaluator for all 58
//! campaign stages (`stages_act1` Stages 1–27 & `stages_act2` Stages 28–58).
//!
//! Verified 1:1 against `sh2pc.exe` (`CharToFloat2` `0x0048b400`, `EventCheck` `0x0048e070`,
//! `EventCheckIn` `0x0048da90`, `EventCheckLook` `0x0048d4d0`, `RETAIL_LOOK_RADIUS_MM = 400.0` at
//! `0x006a91e8`) and `SLUS_202.28` (`EventResultMovePosition` `0x001a9730`, `EventExecMove`
//! `0x001aa670`, `EventExecDoor` `0x001a9f20`).

use crate::events::engine::{
    is_connected_stage_pair, EventEngineEffects, EventPositionalSound, StageTransitionCommand,
};
use crate::events::stages_act2::{
    decode_retail_ev_pos_shape, normalize_retail_event_entry_for_input,
};
use crate::events::triggers::{
    check_event_flag_conditions, event_check_look_line, event_check_look_point,
    event_exec_sub_flag_set, event_item_condition_check, event_result_move_position,
    item_check_look_point, player_height_matches_event_y, EvPosShape, EventEvalInput,
    EVENT_LOOK_RADIUS_MM,
};
use crate::events::EventFlags;
use crate::inventory::InventoryManager;
use crate::types::{
    wrap_angle, EventDestShapeKind, EventListEntry, EventResultKind, EventTriggerKind,
    ItemListEntry, DOOR_SE_TABLE, ITEM_KIND_TABLE,
};

/// Decodes a retail `ev_pos` destination pose (`EventResultMovePosition` `SLUS_202.28` `0x001a9730`,
/// `sh2pc.exe` `0x0048b780`: `x0: f32@0`, `y: f16@4`, `z0: f32@6`, optional `p0: f16@10`, `dz: f16@12`).
///
/// Case 0 (`Point`, `0x001a9730` / `0x0048b780`) reads only 10 bytes (`x0, y, z0`) and
/// preserves the player's current yaw (`0.0` here; callers with a live player pose pass
/// `current_player_yaw` to [`event_result_move_position`]).
#[must_use]
pub fn decode_retail_move_destination(
    ev_pos: &[u8],
    dest_offset: usize,
    dest_shape: EventDestShapeKind,
) -> Option<([f32; 3], f32)> {
    event_result_move_position(ev_pos, dest_offset, dest_shape, 0.0)
}

/// Evaluates one frame of a retail stage's `Event_List` + 10-byte `ev_pos` buffer
/// (`sh2pc.exe` `EventCheck` `0x0048e070` + `EventMainStandard` `0x0048e600`).
pub fn evaluate_retail_stage_events(
    raw_event_list: &[EventListEntry],
    ev_pos: &[u8],
    current_stage: u8,
    input: &EventEvalInput,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> EventEngineEffects {
    let mut effects = EventEngineEffects::default();

    for &raw_entry in raw_event_list {
        if raw_entry.is_sentinel() {
            break;
        }
        let entry = normalize_retail_event_entry_for_input(raw_entry, input, inventory);
        if !check_event_flag_conditions(&entry, flags) {
            continue;
        }
        let Some(trg) = entry.trigger_kind() else {
            continue;
        };

        let triggered = match trg {
            EventTriggerKind::TouchContinuous | EventTriggerKind::TouchEdge => {
                entry.shape_kind().is_some_and(|shape| {
                    decode_retail_ev_pos_shape(ev_pos, entry.ev_pos_offset(), shape)
                        .is_some_and(|s| point_in_retail_shape(input.player_pos, &s))
                })
            }
            EventTriggerKind::ActionLook => {
                input.action_pressed
                    && entry.shape_kind().is_some_and(|shape| {
                        decode_retail_ev_pos_shape(ev_pos, entry.ev_pos_offset(), shape)
                            .is_some_and(|s| player_can_interact_with_shape(input, &s))
                    })
            }
            EventTriggerKind::UseItemLook => {
                input.used_item_id == Some(entry.required_item_id())
                    && entry.shape_kind().is_some_and(|shape| {
                        decode_retail_ev_pos_shape(ev_pos, entry.ev_pos_offset(), shape)
                            .is_some_and(|s| player_can_interact_with_shape(input, &s))
                    })
            }
            // `Always` / `StageInit` / `RoomInit` programs are handled by `AUTO_STAGE_DEMO_RULES`
            // and stage lifecycle hooks rather than blocking door/item evaluation here.
            _ => false,
        };

        if !triggered {
            continue;
        }

        if matches!(
            trg,
            EventTriggerKind::UseItemLook | EventTriggerKind::UseItemTouch
        ) && entry.consume_item()
        {
            inventory.item_lose(entry.required_item_id());
        }

        apply_retail_event_result(
            &entry,
            ev_pos,
            current_stage,
            input.player_rot_y,
            flags,
            &mut effects,
        );
        if effects.stage_transition.is_some()
            || effects.common_message.is_some()
            || effects.stage_message.is_some()
            || effects.active_prog_index.is_some()
            || effects.save_point_name_id.is_some()
        {
            break;
        }
    }

    effects
}

/// Evaluates 3D `Item_List` (`gi_list`) pickups for a single frame (`EventCheck` `0x001a7ed0`
/// and `EventExecItem` `0x001aa260`), granting the item and marking `flag(base_flag + 1)` when
/// the player presses Action while facing an available pickup.
pub fn evaluate_retail_item_pickups(
    items: &[ItemListEntry],
    input: &EventEvalInput,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    effects: &mut EventEngineEffects,
) -> bool {
    if !input.action_pressed || !input.player_can_search {
        return false;
    }
    for item in items {
        if item.is_sentinel() {
            break;
        }
        let base_flag = item.flag_id() as usize;
        // `SLUS_202.28` `0x001a7ed0` & `sh2pc.exe` `0x0048e3b0`: available only when both
        // `flag(base_flag) == 0` and `flag(base_flag + 1) == 0`.
        if flags.get(base_flag) || flags.get(base_flag + 1) {
            continue;
        }
        if !event_item_condition_check(item, input.action_difficulty, flags) {
            continue;
        }
        if !item_check_look_point(item, input.player_pos, input.player_rot_y) {
            continue;
        }
        let kind_idx = (item.item_kind_index() as usize).min(ITEM_KIND_TABLE.len() - 1);
        let kind = ITEM_KIND_TABLE[kind_idx];
        flags.set(base_flag + 1, true);
        if kind.item_id > 0 {
            inventory.item_get_now(kind.item_id as u8);
        }
        effects.common_message = Some(kind.msg_id);
        effects.sound_effects.push(0x2b21);
        effects.deleted_item_model = Some((kind.chr_id, item.flag_id()));
        return true;
    }
    false
}

fn apply_retail_event_result(
    entry: &EventListEntry,
    ev_pos: &[u8],
    current_stage: u8,
    current_player_yaw: f32,
    flags: &mut EventFlags,
    effects: &mut EventEngineEffects,
) {
    let Some(rslt) = entry.result_kind() else {
        return;
    };
    match rslt {
        EventResultKind::Move | EventResultKind::MovePlusProgram => {
            event_exec_sub_flag_set(entry, flags);
            let result_flag = entry.result_flag() as usize;
            if result_flag != 0 && rslt == EventResultKind::Move {
                flags.set(result_flag, true);
            }
            // `SLUS_202.28` `EventResultMovePosition` (`0x001a97c8`: `sltiu $1, $17, 6; beqz`)
            // and `sh2pc.exe` (`0x0048b841`: `cmpl $4, %eax; ja 0x48b9a6`) treat both `0` and
            // `>= 6` as Case 0: `[x0, y, z0]` preserving `player->rot.y`.
            let ds = entry.dest_shape_kind().unwrap_or(EventDestShapeKind::Point);
            let (dest_pos, dest_rot_y) = event_result_move_position(
                ev_pos,
                entry.dest_ev_pos_offset(),
                ds,
                current_player_yaw,
            )
            .unwrap_or(([0.0, 0.0, 0.0], wrap_angle(current_player_yaw)));

            let door_idx = (entry.dest_stage_or_door_se() as usize).min(DOOR_SE_TABLE.len() - 1);
            let door_se = DOOR_SE_TABLE[door_idx];
            if door_se.open != 0 {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id: door_se.open,
                    volume: 1.0,
                    pos: [dest_pos[0], dest_pos[1] - 500.0, dest_pos[2]],
                    channel: 0,
                });
            }

            let stage_change_mode = entry.stage_change_mode();
            let dest_stage = if stage_change_mode != 0 {
                stage_change_mode
            } else {
                current_stage
            };
            let is_connected =
                stage_change_mode != 0 && is_connected_stage_pair(current_stage, dest_stage);
            effects.stage_transition = Some(StageTransitionCommand {
                dest_stage: if stage_change_mode != 0 {
                    dest_stage
                } else {
                    0
                },
                stage_change_mode,
                is_connected_stage: is_connected,
                observation_mode: entry.observation_mode(),
                dest_pos,
                dest_rot_y,
            });
        }
        EventResultKind::DoorLocked
        | EventResultKind::DoorBroken
        | EventResultKind::DoorUnlocked => {
            let door_idx = (entry.dest_stage_or_door_se() as usize).min(DOOR_SE_TABLE.len() - 1);
            let door_se = DOOR_SE_TABLE[door_idx];
            let msg_id = entry.message_or_save_id();
            let se_id = match rslt {
                EventResultKind::DoorLocked => {
                    if msg_id == 0xff {
                        effects.common_message = Some(5);
                    } else {
                        effects.stage_message = Some(u16::from(msg_id));
                    }
                    door_se.lock
                }
                EventResultKind::DoorUnlocked => {
                    if msg_id == 0xff {
                        effects.common_message = Some(6);
                    } else if msg_id == 0xfe {
                        effects.common_message = Some(7);
                    } else {
                        effects.stage_message = Some(u16::from(msg_id));
                    }
                    door_se.unlock
                }
                _ => {
                    effects.common_message = Some(4);
                    door_se.broken
                }
            };
            if se_id != 0 {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id,
                    volume: 1.0,
                    pos: [0.0, -500.0, 0.0],
                    channel: 0,
                });
            }
            let result_flag = entry.result_flag() as usize;
            if result_flag != 0 {
                flags.set(result_flag, true);
            }
            event_exec_sub_flag_set(entry, flags);
        }
        EventResultKind::FlagSet | EventResultKind::FlagClear => {
            flags.set(
                entry.result_flag() as usize,
                rslt == EventResultKind::FlagSet,
            );
            event_exec_sub_flag_set(entry, flags);
        }
        EventResultKind::Message => {
            effects.stage_message = Some(u16::from(entry.message_or_save_id()));
            event_exec_sub_flag_set(entry, flags);
        }
        EventResultKind::Program => {
            // `EventExecProgram` (`SLUS_202.28` `0x001a9e00`): set sub-flags and launch `ev_prog`.
            event_exec_sub_flag_set(entry, flags);
            effects.active_prog_index = Some(entry.prog_index());
            effects.active_prog_completion_flag = entry.result_flag();
        }
        EventResultKind::SavePoint => {
            // `EventExecSave` (`SLUS_202.28` `0x001aab10`): first save shows common message `8`
            // and sets `flag(5)` (`0x01126430` bit 5), then transitions to Save screen (`0x2743`).
            if !flags.get(5) {
                flags.set(5, true);
                effects.common_message = Some(8);
            }
            effects.fade_command = Some(11);
            effects.save_point_name_id = Some(entry.message_or_save_id());
            effects.sound_effects.push(0x2743);
        }
    }
}

fn point_in_retail_shape(player_pos: [f32; 3], shape: &EvPosShape) -> bool {
    let [px, py, pz] = player_pos;
    match *shape {
        EvPosShape::Point { x, y, z } => {
            player_height_matches_event_y(py, y) && (px - x).hypot(pz - z) <= EVENT_LOOK_RADIUS_MM
        }
        EvPosShape::Rectangle { x, y, z, w, d } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            let (min_x, max_x) = if w >= 0.0 { (x, x + w) } else { (x + w, x) };
            let (min_z, max_z) = if d >= 0.0 { (z, z + d) } else { (z + d, z) };
            px >= min_x && px <= max_x && pz >= min_z && pz <= max_z
        }
        EvPosShape::Triangle { y, vertices } => {
            player_height_matches_event_y(py, y) && point_in_triangle(px, pz, vertices)
        }
        EvPosShape::Quad { y, vertices } => {
            player_height_matches_event_y(py, y)
                && (point_in_triangle(px, pz, [vertices[0], vertices[1], vertices[2]])
                    || point_in_triangle(px, pz, [vertices[0], vertices[2], vertices[3]]))
        }
        EvPosShape::Polyline { y, ref vertices } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            vertices
                .windows(2)
                .any(|seg| dist_to_segment(px, pz, seg[0], seg[1]) <= EVENT_LOOK_RADIUS_MM)
        }
    }
}

fn player_can_interact_with_shape(input: &EventEvalInput, shape: &EvPosShape) -> bool {
    let [px, py, pz] = input.player_pos;
    // Exact 1:1 PS2 `EventCheckLookPoint` (`SLUS_202.28` `0x001a8b00`) and
    // `EventCheckLookLine` (`SLUS_202.28` `0x001a8c80` / `sh2pc.exe` `0x0048b0c0`).
    match *shape {
        EvPosShape::Point { x, y, z } => {
            player_height_matches_event_y(py, y)
                && event_check_look_point(px, pz, input.player_rot_y, x, z)
        }
        EvPosShape::Polyline { y, ref vertices } => {
            player_height_matches_event_y(py, y)
                && vertices.windows(2).any(|seg| {
                    event_check_look_line(
                        px,
                        pz,
                        input.player_rot_y,
                        seg[0][0],
                        seg[0][1],
                        seg[1][0],
                        seg[1][1],
                    )
                })
        }
        _ => point_in_retail_shape(input.player_pos, shape),
    }
}

fn point_in_triangle(px: f32, pz: f32, v: [[f32; 2]; 3]) -> bool {
    let d1 = (px - v[1][0]) * (v[0][1] - v[1][1]) - (v[0][0] - v[1][0]) * (pz - v[1][1]);
    let d2 = (px - v[2][0]) * (v[1][1] - v[2][1]) - (v[1][0] - v[2][0]) * (pz - v[2][1]);
    let d3 = (px - v[0][0]) * (v[2][1] - v[0][1]) - (v[2][0] - v[0][0]) * (pz - v[0][1]);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn dist_to_segment(px: f32, pz: f32, a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = b[0] - a[0];
    let dz = b[1] - a[1];
    let len_sq = dx * dx + dz * dz;
    if len_sq <= 1e-6 {
        return (px - a[0]).hypot(pz - a[1]);
    }
    let t = (((px - a[0]) * dx + (pz - a[1]) * dz) / len_sq).clamp(0.0, 1.0);
    (px - (a[0] + t * dx)).hypot(pz - (a[1] + t * dz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::StageProgramRunner;

    #[test]
    fn retail_program_and_save_events_emit_effects() {
        let mut flags = EventFlags::new();
        let mut effects = EventEngineEffects::default();

        // Program result (rslt = 5, prog_no = 3, result_flag = 42)
        let prog_entry = EventListEntry {
            flag: 0,
            cond: 0,
            rslt0: 5 << 28,
            rslt1: (3 << 14) | 42,
        };
        apply_retail_event_result(&prog_entry, &[], 2, 0.0, &mut flags, &mut effects);
        assert_eq!(effects.active_prog_index, Some(3));
        assert_eq!(effects.active_prog_completion_flag, 42);

        // First SavePoint result (rslt = 10, save_id = 7)
        let mut save_effects = EventEngineEffects::default();
        let save_entry = EventListEntry {
            flag: 0,
            cond: 0,
            rslt0: 10 << 28,
            rslt1: 7 << 14,
        };
        apply_retail_event_result(&save_entry, &[], 2, 0.0, &mut flags, &mut save_effects);
        assert!(flags.get(5));
        assert_eq!(save_effects.common_message, Some(8));
        assert_eq!(save_effects.save_point_name_id, Some(7));
        assert_eq!(save_effects.sound_effects, vec![0x2743]);
    }

    #[test]
    fn retail_item_pickup_grants_item_and_sets_flag() {
        let mut flags = EventFlags::new();
        let mut inventory = InventoryManager::new();
        let mut effects = EventEngineEffects::default();
        // Item kind 1 = Handgun bullets (item_id = 5, msg_id = 9), flag_id = 120
        let item = ItemListEntry {
            pos_x: 0.0,
            pos_z: 100.0,
            pos_y: 0,
            rot_y: 0,
            flag: (1 << 29) | 120,
        };
        let input = EventEvalInput {
            player_pos: [0.0, 0.0, 0.0],
            player_rot_y: 0.0,
            action_pressed: true,
            player_can_search: true,
            ..EventEvalInput::default()
        };
        assert!(evaluate_retail_item_pickups(
            &[item],
            &input,
            &mut flags,
            &mut inventory,
            &mut effects,
        ));
        assert!(flags.get(121));
        assert!(inventory.count(5) >= 1);
        assert_eq!(effects.common_message, Some(9));
        assert_eq!(effects.sound_effects, vec![0x2b21]);
    }

    #[test]
    fn ev_prog_va_lookup_and_ending_progs_work() {
        assert_eq!(
            StageProgramRunner::resolve_ev_prog_va(2, 1),
            Some(0x005a_14a0)
        );
        assert_eq!(
            StageProgramRunner::resolve_ev_prog_va(48, 1),
            Some(0x0043_fc70)
        );
        assert_eq!(StageProgramRunner::resolve_ev_prog_va(2, 0), None);

        let mut flags = EventFlags::new();
        let mut puzzle = crate::events::PuzzleState::new();
        let mut inv = InventoryManager::new();
        let mut overlay = crate::cutaway::CutawayOverlayEngine::new();
        let mut runner = StageProgramRunner::new();
        runner.start(52, 1, 0);
        assert!(!runner.tick(
            0,
            0,
            1.0 / 60.0,
            &mut flags,
            &mut puzzle,
            &mut inv,
            &mut overlay,
            crate::config::RiddleDifficulty::Normal,
        ));
        assert_eq!(runner.take_triggered_ending(), Some(0x10));
    }

    #[test]
    fn event_flags_and_inventory_round_trip() {
        let mut flags = EventFlags::new();
        flags.set(5, true);
        flags.set(618, true);
        let words = flags.to_words();
        let mut restored = EventFlags::new();
        restored.from_words(&words);
        assert!(restored.get(5));
        assert!(restored.get(618));

        let mut inv = InventoryManager::new();
        inv.item_get_now(20);
        inv.item_get_now(21);
        let items = inv.owned_items();
        let mut inv2 = InventoryManager::new();
        inv2.restore_items(&items);
        assert_eq!(inv2.count(20), 1);
        assert_eq!(inv2.count(21), 1);
    }
}
