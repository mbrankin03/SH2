//! Retail `sh2pc.exe` 10-byte `ev_pos` spatial trigger & `EventExecMove` evaluator for all 58
//! campaign stages (`stages_act1` Stages 1–27 & `stages_act2` Stages 28–58).
//!
//! Verified 1:1 against `sh2pc.exe` (`CharToFloat2` `0x0048b400`, `EventCheck` `0x0048e070`,
//! `EventCheckIn` `0x0048da90`, `EventCheckLook` `0x0048d4d0`, `RETAIL_LOOK_RADIUS_MM = 400.0` at
//! `0x006a91e8`) and `SLUS_202.28` (`EventResultMovePosition` `0x001a9730`, `EventExecMove`
//! `0x001aa670`, `EventExecDoor` `0x001a9f20`).

use std::f32::consts::{FRAC_PI_2, PI};

use crate::events::engine::{
    is_connected_stage_pair, EventEngineEffects, EventPositionalSound, StageTransitionCommand,
};
use crate::events::stages_act2::{
    char_to_float2_retail, decode_retail_ev_pos_shape, normalize_retail_event_entry_for_input,
};
use crate::events::triggers::{
    check_event_flag_conditions, event_check_look_line, event_check_look_point,
    event_exec_sub_flag_set, player_height_matches_event_y, EvPosShape, EventEvalInput,
};
use crate::events::EventFlags;
use crate::inventory::InventoryManager;
use crate::types::{
    char_to_float4, wrap_angle, EventDestShapeKind, EventListEntry, EventResultKind,
    EventTriggerKind, DOOR_SE_TABLE,
};

/// Decodes a retail 10-byte `ev_pos` destination pose (`EventResultMovePosition` `0x001a9730`
/// using `sh2pc.exe` `f32`/`f16` layout: `x0: f32@0`, `y: f16@4`, `z0: f32@6`, `param: f16@10`).
#[must_use]
pub fn decode_retail_move_destination(
    ev_pos: &[u8],
    dest_offset: usize,
    dest_shape: EventDestShapeKind,
) -> Option<([f32; 3], f32)> {
    let min_len = match dest_shape {
        EventDestShapeKind::DoorSegment => 14,
        _ => 12,
    };
    let s = ev_pos.get(dest_offset..dest_offset.checked_add(min_len)?)?;
    let x0 = char_to_float4([s[0], s[1], s[2], s[3]]);
    let raw_y = char_to_float2_retail([s[4], s[5]]);
    let y = if raw_y == 0.0 { 0.0 } else { raw_y };
    let z0 = char_to_float4([s[6], s[7], s[8], s[9]]);
    let p0 = char_to_float2_retail([s[10], s[11]]);

    // Exact 1:1 parity with `EventResultMovePosition` (`SLUS_202.28` `0x001a9730`, jump table
    // `0x00391060`; `sh2pc.exe` `0x0048b7e0`, jump table `0x0048ba04`).
    let (pos, angle) = match dest_shape {
        EventDestShapeKind::PointWithAngle => ([x0, y, z0], p0 * PI / 180.0),
        // Case 1 (`0x001a97f0` / `0x0048b855`): x = x0 + p0/2, z = z0 + 250, rot_y = 0.0
        EventDestShapeKind::DoorLineNegZ => ([x0 + p0 * 0.5, y, z0 + 250.0], 0.0),
        // Case 2 (`0x001a983c` / `0x0048b888`): x = x0 - p0/2, z = z0 - 250, rot_y = PI
        EventDestShapeKind::DoorLinePosX => ([x0 - p0 * 0.5, y, z0 - 250.0], PI),
        // Case 3 (`0x001a9890` / `0x0048b8bb`): x = x0 + 250, z = z0 - p0/2, rot_y = +PI/2
        EventDestShapeKind::DoorLinePosZ => ([x0 + 250.0, y, z0 - p0 * 0.5], FRAC_PI_2),
        // Case 4 (`0x001a98e4` / `0x0048b8ee`): x = x0 - 250, z = z0 + p0/2, rot_y = -PI/2
        EventDestShapeKind::DoorLineNegX => ([x0 - 250.0, y, z0 + p0 * 0.5], -FRAC_PI_2),
        // Case 5 (`0x001a9938` / `0x0048b921`): midpoint + 250 along normal (atan2(dz, p0) - PI/2)
        EventDestShapeKind::DoorSegment => {
            let dz = char_to_float2_retail([s[12], s[13]]);
            let mid_x = x0 + p0 * 0.5;
            let mid_z = z0 + dz * 0.5;
            let angle = dz.atan2(p0) - FRAC_PI_2;
            (
                [mid_x + angle.sin() * 250.0, y, mid_z + angle.cos() * 250.0],
                angle,
            )
        }
    };

    Some((pos, wrap_angle(angle)))
}

/// Evaluates one frame of a retail stage's `Event_List` + 10-byte `ev_pos` buffer
/// (`sh2pc.exe` `EventCheck` `0x0048e070` + `EventMainStandard` `0x0048e600`).
pub fn evaluate_retail_stage_events(
    raw_event_list: &[EventListEntry],
    ev_pos: &[u8],
    current_stage: u8,
    input: &EventEvalInput,
    flags: &mut EventFlags,
    inventory: &InventoryManager,
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

        apply_retail_event_result(&entry, ev_pos, current_stage, flags, &mut effects);
        if effects.stage_transition.is_some()
            || effects.common_message.is_some()
            || effects.stage_message.is_some()
        {
            break;
        }
    }

    effects
}

fn apply_retail_event_result(
    entry: &EventListEntry,
    ev_pos: &[u8],
    current_stage: u8,
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
            let (dest_pos, dest_rot_y) = entry
                .dest_shape_kind()
                .and_then(|ds| {
                    decode_retail_move_destination(ev_pos, entry.dest_ev_pos_offset(), ds)
                })
                .unwrap_or(([0.0, 0.0, 0.0], 0.0));

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
        _ => {}
    }
}

fn point_in_retail_shape(player_pos: [f32; 3], shape: &EvPosShape) -> bool {
    let [px, py, pz] = player_pos;
    match *shape {
        EvPosShape::Point { x, y, z, radius } => {
            player_height_matches_event_y(py, y) && (px - x).hypot(pz - z) <= radius
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
        EvPosShape::Polyline {
            y,
            radius,
            ref vertices,
        } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            vertices
                .windows(2)
                .any(|seg| dist_to_segment(px, pz, seg[0], seg[1]) <= radius)
        }
    }
}

fn player_can_interact_with_shape(input: &EventEvalInput, shape: &EvPosShape) -> bool {
    let [px, py, pz] = input.player_pos;
    // Exact 1:1 PS2 `EventCheckLookPoint` (`SLUS_202.28` `0x001a8a30`) and
    // `EventCheckLookLine` (`SLUS_202.28` `0x001a8b10`) using the decoded shape radius
    // (`RETAIL_LOOK_RADIUS_MM = 400.0` mm) and facing-cone check.
    match *shape {
        EvPosShape::Point { x, y, z, radius } => {
            player_height_matches_event_y(py, y)
                && event_check_look_point(px, pz, input.player_rot_y, x, z, radius) > 0.0
        }
        EvPosShape::Polyline {
            y,
            radius,
            ref vertices,
        } => {
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
                        radius,
                    ) > 0.0
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
