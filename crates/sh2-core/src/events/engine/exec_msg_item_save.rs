//! Message (`EventExecMessage`), item pickup (`EventExecItem`), save point (`EventExecSave`),
//! and common fail message (`EventExecChizuFail` / `EventExecItemUseFail`) handlers
//! (`SLUS_202.28` `src/Event/event.c`).

use crate::inventory::InventoryManager;
use crate::types::{EventListEntry, ItemListEntry, ITEM_KIND_TABLE};

use super::super::triggers::event_exec_sub_flag_set;
use super::super::EventFlags;
use super::{EventEngineEffects, EventStepState};

/// Executes one frame of `EventExecMessage` (`SLUS_202.28` `0x001a9cc0`).
///
/// Returns `true` when the message has been dismissed (`message_dismissed` or `state.ev_cancel`).
pub fn event_exec_message(
    entry: &EventListEntry,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_e_step == 0 {
        effects.stage_message = Some(entry.message_or_save_id() as u16);
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

/// Executes one frame of `EventExecItem` (`SLUS_202.28` `0x001aa260`) for a stage `Item_List` pickup.
#[allow(clippy::too_many_arguments)]
pub fn event_exec_item(
    item: &ItemListEntry,
    player_y: f32,
    anim_success_frame: bool,
    anim_ended: bool,
    message_dismissed: bool,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    effects: &mut EventEngineEffects,
) -> bool {
    let item_y = item.pos_y as f32;
    let is_crouch_pickup = (player_y - item_y) < 100.0;

    match state.ev_e_step {
        0 => {
            effects.player_lock_change = Some(true);
            if is_crouch_pickup {
                effects.player_pickup_anim = Some(0x4e21);
                state.set_e_step(4);
            } else {
                effects.player_pickup_anim = Some(0x0065);
                state.set_e_step(1);
            }
            false
        }
        4 => {
            if state.ev_cancel {
                state.set_e_step(6);
            } else if anim_success_frame {
                state.set_e_step(1);
            }
            false
        }
        1 => {
            let kind_idx = (item.item_kind_index() as usize).min(ITEM_KIND_TABLE.len() - 1);
            let kind = ITEM_KIND_TABLE[kind_idx];
            let base_flag = item.flag_id() as usize;
            flags.set(base_flag + 1, true);
            if kind.item_id > 0 {
                inventory.item_get_now(kind.item_id as u8);
            }
            effects.common_message = Some(kind.msg_id);
            state.set_e_step(2);
            false
        }
        2 => {
            if message_dismissed || state.ev_cancel {
                effects.clear_message = true;
                effects.sound_effects.push(0x2b21);
                let kind_idx = (item.item_kind_index() as usize).min(ITEM_KIND_TABLE.len() - 1);
                let kind = ITEM_KIND_TABLE[kind_idx];
                effects.deleted_item_model = Some((kind.chr_id, item.flag_id()));
                if is_crouch_pickup {
                    state.set_e_step(5);
                } else {
                    state.set_e_step(6);
                }
            }
            false
        }
        5 => {
            if anim_ended || state.ev_cancel {
                state.set_e_step(6);
            }
            false
        }
        6 => {
            effects.player_lock_change = Some(false);
            true
        }
        _ => true,
    }
}

/// Executes one frame of `EventExecSave` (`SLUS_202.28` `0x001aab10`).
///
/// Checks `flag(5)` (`0x01126430` bit 5): on the player's first save interaction, displays
/// common message `8` before setting `flag(5)` and transitioning to the save screen (`step 7`).
pub fn event_exec_save(
    entry: &EventListEntry,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_e_step {
        0 => {
            effects.player_lock_change = Some(true);
            if flags.get(5) {
                state.set_e_step(7);
            } else {
                effects.common_message = Some(8);
                state.set_e_step(1);
            }
            false
        }
        1 => {
            if message_dismissed || state.ev_cancel {
                effects.clear_message = true;
                if state.ev_cancel {
                    state.set_e_step(6);
                } else {
                    flags.set(5, true);
                    state.set_e_step(7);
                }
            }
            false
        }
        7 => {
            effects.fade_command = Some(11);
            effects.save_point_name_id = Some(entry.message_or_save_id());
            effects.sound_effects.push(0x2743);
            state.set_e_step(6);
            false
        }
        6 => {
            effects.fade_command = Some(4);
            effects.player_lock_change = Some(false);
            true
        }
        _ => false,
    }
}

/// Executes one frame of `EventExecChizuFail` (`SLUS_202.28` `0x001aae40`) or `EventExecItemUseFail`.
pub fn event_exec_common_fail_message(
    common_msg_id: u16,
    state: &mut EventStepState,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_e_step == 0 {
        effects.common_message = Some(common_msg_id);
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
