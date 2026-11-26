//! Unified Campaign `EventEngine` bridge for all 58 retail stages (`stages_act1` Stages 1–27 &
//! `stages_act2` Stages 28–58).
//!
//! Consolidated directly onto 1:1 [`EventEngine`] (`SLUS_202.28` `EventMainStandard` `0x001a7b50`,
//! `EventCheck` `0x001a7ed0`, `EventExecMessage` `0x001a9cc0`, `EventExecMove` `0x001aa670`,
//! `EventExecDoor` `0x001a9f20`, `EventExecItem` `0x001aa260`; `sh2pc.exe` `0x0048e070` / `0x0048e610`)
//! so campaign gameplay and unit tests share a single authentic event execution path.

use crate::events::engine::{EventEngine, EventEngineEffects, EventExecMode};
use crate::events::triggers::{event_result_move_position, EventEvalInput};
use crate::events::EventFlags;
use crate::inventory::InventoryManager;
use crate::types::{EventDestShapeKind, EventListEntry, ItemListEntry};

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

/// Evaluates one frame of a retail stage's `Event_List`, `Item_List`, and `ev_pos` buffer
/// using the persistent campaign [`EventEngine`] (`EventMainStandard` `0x001a7b50` / `0x0048e610`).
#[allow(clippy::too_many_arguments)]
pub fn tick_retail_stage_events(
    event_engine: &mut EventEngine,
    raw_event_list: &[EventListEntry],
    item_list: &[ItemListEntry],
    ev_pos: &[u8],
    current_stage: u8,
    input: &EventEvalInput,
    message_dismissed: bool,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> EventEngineEffects {
    event_engine.tick_standard(
        raw_event_list,
        item_list,
        ev_pos,
        current_stage,
        input,
        message_dismissed,
        true,
        true,
        false,
        flags,
        inventory,
    )
}

/// Stateless one-frame convenience wrapper over [`EventEngine::tick_standard`] for tests and
/// single-shot stage event queries (`EventCheck` `0x001a7ed0` + `EventMainStandard` `0x001a7b50`).
pub fn evaluate_retail_stage_events(
    raw_event_list: &[EventListEntry],
    ev_pos: &[u8],
    current_stage: u8,
    input: &EventEvalInput,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
) -> EventEngineEffects {
    let mut engine = EventEngine::new();
    engine.tick_standard(
        raw_event_list,
        &[],
        ev_pos,
        current_stage,
        input,
        false,
        true,
        true,
        false,
        flags,
        inventory,
    )
}

/// Evaluates 3D `Item_List` (`gi_list`) pickups through [`EventEngine::tick_standard`]
/// (`EventCheck` `0x001a7ed0` and `EventExecItem` `0x001aa260`), granting the item and setting
/// `flag(base_flag + 1)` when the player presses Action while facing an available pickup.
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
    let mut engine = EventEngine::new();
    let mut picked_up = false;
    let mut step_input = *input;
    for step in 0..6 {
        if step > 0 {
            step_input.action_pressed = false;
        }
        let tick_eff = engine.tick_standard(
            &[],
            items,
            &[],
            0,
            &step_input,
            true,
            true,
            true,
            false,
            flags,
            inventory,
        );
        if tick_eff.common_message.is_some() {
            effects.common_message = tick_eff.common_message;
            picked_up = true;
        }
        effects.sound_effects.extend(tick_eff.sound_effects);
        if tick_eff.deleted_item_model.is_some() {
            effects.deleted_item_model = tick_eff.deleted_item_model;
        }
        if engine.step_state.ev_m_step != EventExecMode::Item {
            break;
        }
    }
    picked_up
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::StageProgramRunner;

    #[test]
    fn retail_program_and_save_events_emit_effects() {
        let mut flags = EventFlags::new();
        let mut inv = InventoryManager::new();
        let input = EventEvalInput::default();

        // Program result (rslt = 3 per SLUS 0x001a7b50 / sh2pc 0x0048e610, prog_no = 3, result_flag = 42)
        let prog_entry = EventListEntry {
            flag: 0,
            cond: 0,
            rslt0: 3 << 28,
            rslt1: (3 << 14) | 42,
        };
        let effects =
            evaluate_retail_stage_events(&[prog_entry], &[], 2, &input, &mut flags, &mut inv);
        assert_eq!(effects.active_prog_index, Some(3));
        assert_eq!(effects.active_prog_completion_flag, 42);

        // First SavePoint result (rslt = 10, save_id = 7) via EventEngine multi-step flow
        let save_entry = EventListEntry {
            flag: 0,
            cond: 0,
            rslt0: 10 << 28,
            rslt1: 7 << 14,
        };
        let mut engine = EventEngine::new();
        let e0 = engine.tick_standard(
            &[save_entry],
            &[],
            &[],
            2,
            &input,
            false,
            true,
            true,
            false,
            &mut flags,
            &mut inv,
        );
        assert_eq!(e0.common_message, Some(8));
        // Dismiss first-save explanation -> sets flag(5) and advances to step 7
        let _ = engine.tick_standard(
            &[save_entry],
            &[],
            &[],
            2,
            &input,
            true,
            true,
            true,
            false,
            &mut flags,
            &mut inv,
        );
        assert!(flags.get(5));
        // Step 7 emits save_point_name_id and SE 0x2743
        let e2 = engine.tick_standard(
            &[save_entry],
            &[],
            &[],
            2,
            &input,
            false,
            true,
            true,
            false,
            &mut flags,
            &mut inv,
        );
        assert_eq!(e2.save_point_name_id, Some(7));
        assert_eq!(e2.sound_effects, vec![0x2743]);
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
