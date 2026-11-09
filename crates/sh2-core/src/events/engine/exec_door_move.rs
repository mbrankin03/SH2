//! Door (`EventExecDoor`), room/stage transition (`EventExecMove`), and flag (`EventExecFlag`)
//! execution handlers (`SLUS_202.28` `src/Event/event.c`).

use crate::types::{
    EventListEntry, EventResultKind, EventShapeKind, CONNECT_STAGE_PAIRS, DOOR_SE_TABLE,
};

use super::super::triggers::{
    event_exec_sub_flag_set, event_position_set, event_result_move_position,
};
use super::super::EventFlags;
use super::{
    EventEngineEffects, EventExecMode, EventPositionalSound, EventStepState, StageTransitionCommand,
};

/// Executes `EventExecFlag` (`SLUS_202.28` `0x001a9bf0`): sets (`rslt == 1`) or clears (`rslt == 2`)
/// `entry.result_flag()` and applies `EventExecSubFlagSet`.
pub fn event_exec_flag(entry: &EventListEntry, flags: &mut EventFlags) -> bool {
    let set_value = entry.result_kind() == Some(EventResultKind::FlagSet);
    let target_flag = entry.result_flag() as usize;
    flags.set(target_flag, set_value);
    event_exec_sub_flag_set(entry, flags);
    true
}

/// Executes one frame of `EventExecDoor` (`SLUS_202.28` `0x001a9f20`).
///
/// Handles locked (`rslt == 7`), broken (`rslt == 8`), and unlocked (`rslt == 9`) doors,
/// playing the 3D positional door sound at `ev_pos` (`pos.y -= 500.0`) and showing the
/// appropriate common or stage message.
pub fn event_exec_door(
    entry: &EventListEntry,
    ev_pos: &[u8],
    state: &mut EventStepState,
    flags: &mut EventFlags,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_e_step == 0 {
        let door_idx = (entry.dest_stage_or_door_se() as usize).min(DOOR_SE_TABLE.len() - 1);
        let door_se = DOOR_SE_TABLE[door_idx];
        let mut pos = entry
            .shape_kind()
            .and_then(|s| event_position_set(ev_pos, entry.ev_pos_offset(), s))
            .unwrap_or([0.0, 0.0, 0.0]);
        pos[1] -= 500.0;

        let msg_id = entry.message_or_save_id();
        let se_id = match entry.result_kind() {
            Some(EventResultKind::DoorLocked) => {
                if msg_id == 0xff {
                    effects.common_message = Some(5); // "It's locked."
                } else {
                    effects.stage_message = Some(msg_id as u16);
                }
                door_se.lock
            }
            Some(EventResultKind::DoorUnlocked) => {
                if msg_id == 0xff {
                    effects.common_message = Some(6); // "The lock is unlocked."
                } else if msg_id == 0xfe {
                    effects.common_message = Some(7); // "The door is unlocked."
                } else {
                    effects.stage_message = Some(msg_id as u16);
                }
                door_se.unlock
            }
            _ => {
                effects.common_message = Some(4); // "The lock is broken. I can't open this door."
                door_se.broken
            }
        };

        if se_id != 0 {
            effects.positional_sounds.push(EventPositionalSound {
                se_id,
                volume: 1.0,
                pos,
                channel: 0,
            });
        }

        let result_flag = entry.result_flag() as usize;
        if result_flag != 0 {
            flags.set(result_flag, true);
        }
        event_exec_sub_flag_set(entry, flags);
        effects.player_lock_change = Some(true);
        state.set_e_step(2);
    }

    if message_dismissed || state.ev_cancel {
        effects.clear_message = true;
        effects.player_lock_change = Some(false);
        true
    } else {
        false
    }
}

/// Checks if `(stage_a, stage_b)` is a connected sub-stage pair in [`CONNECT_STAGE_PAIRS`]
/// (`EventExecMove` at `0x001aa8c8..0x001aa944`).
pub fn is_connected_stage_pair(stage_a: u8, stage_b: u8) -> bool {
    let a = stage_a as i16;
    let b = stage_b as i16;
    CONNECT_STAGE_PAIRS
        .iter()
        .take_while(|&&(x, _)| x != 0)
        .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
}

/// Executes one frame of `EventExecMove` (`SLUS_202.28` `0x001aa670`).
pub fn event_exec_move(
    entry: &EventListEntry,
    ev_pos: &[u8],
    current_stage: u8,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_e_step {
        0 => {
            event_exec_sub_flag_set(entry, flags);
            let result_flag = entry.result_flag() as usize;
            if result_flag != 0 && state.ev_m_step != EventExecMode::MoveThenProgram {
                flags.set(result_flag, true);
            }

            if let Some(dest_shape) = entry.dest_shape_kind() {
                if let Some((pos, rot_y)) =
                    event_result_move_position(ev_pos, entry.dest_ev_pos_offset(), dest_shape)
                {
                    state.result_move_pos = pos;
                    state.result_move_rot_y = rot_y;
                }
            }

            // Trigger-side door open sound
            let mut open_pos = entry
                .shape_kind()
                .and_then(|s| event_position_set(ev_pos, entry.ev_pos_offset(), s))
                .unwrap_or([0.0, 0.0, 0.0]);
            open_pos[1] -= 500.0;

            let door_idx = (entry.dest_stage_or_door_se() as usize).min(DOOR_SE_TABLE.len() - 1);
            let door_se = DOOR_SE_TABLE[door_idx];
            if door_se.open != 0 {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id: door_se.open,
                    volume: 1.0,
                    pos: open_pos,
                    channel: 0,
                });
            }
            state.ev_close_se = door_se.close;

            // Destination-side door close sound position
            let dest_shape_as_ev = entry
                .dest_shape_kind()
                .and_then(|ds| EventShapeKind::from_raw(ds as u8));
            let mut close_pos = dest_shape_as_ev
                .and_then(|s| event_position_set(ev_pos, entry.dest_ev_pos_offset(), s))
                .unwrap_or(state.result_move_pos);
            close_pos[1] -= 500.0;
            state.se_pos = close_pos;

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
                dest_pos: state.result_move_pos,
                dest_rot_y: state.result_move_rot_y,
            });

            effects.player_lock_change = Some(true);
            state.set_e_step(3);
            effects.fade_command = Some(1);
            false
        }
        3 => {
            if state.ev_close_se != 0 {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id: state.ev_close_se,
                    volume: 1.0,
                    pos: state.se_pos,
                    channel: 0,
                });
            }
            effects.player_lock_change = Some(false);
            effects.fade_command = Some(4);

            if state.ev_m_step == EventExecMode::MoveThenProgram {
                state.ev_m_step = EventExecMode::Program;
                state.set_e_step(0);
                effects.active_prog_index = Some(entry.prog_index());
                false
            } else {
                true
            }
        }
        _ => false,
    }
}
