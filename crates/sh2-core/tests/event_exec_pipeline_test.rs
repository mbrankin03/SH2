//! Integration tests for `EventEngine` (`EventMainStandard`), `EventExecFlag`, `EventExecDoor`,
//! `EventExecItem` (crouch vs stand pickup), `LightSpotOnOffCheck`, and `EventProgressCheck`.

use sh2_core::types::{EventListEntry, ItemListEntry};
use sh2_core::{
    event_check, event_progress_check, item_check_look_point, light_spot_on_off_check,
    ActionDifficulty, EventEngine, EventEvalInput, EventExecMode, EventFlags, EventTriggerState,
    InventoryManager, EVENT_ITEM_INDEX_BASE,
};

fn push_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

#[test]
fn event_engine_state_machine_executes_flag_message_door_move_item_and_save() {
    let mut ev_pos = Vec::new();
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 500);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 2000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 3000);
    push_i16(&mut ev_pos, 90);

    let ev_list = [
        EventListEntry {
            flag: (1 << 31) | (100 << 16),
            cond: (1 << 28),
            rslt0: 1 << 28,
            rslt1: 100,
        },
        EventListEntry {
            flag: (1 << 31) | (648 << 16),
            cond: (2 << 28),
            rslt0: 7 << 28,
            rslt1: (1 << 22) | (0xff << 14) | 648,
        },
        EventListEntry {
            flag: 0x3fff,
            cond: 0,
            rslt0: 0,
            rslt1: 0,
        },
    ];

    let item_list = [ItemListEntry {
        flag: 0x3fff << 16,
        ..Default::default()
    }];

    let mut engine = EventEngine::new();
    let mut flags = EventFlags::new();
    let mut inventory = InventoryManager::new();

    let input = EventEvalInput {
        player_pos: [0.0, 0.0, 200.0],
        player_rot_y: 0.0,
        ..Default::default()
    };
    let effects = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        31,
        &input,
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert!(flags.get(100));
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::Idle);
    assert!(effects.common_message.is_none());

    let action_input = EventEvalInput {
        player_pos: [0.0, 0.0, 200.0],
        player_rot_y: 0.0,
        action_pressed: true,
        ..Default::default()
    };
    let door_effects = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        31,
        &action_input,
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::Door);
    assert_eq!(door_effects.player_lock_change, Some(true));
    assert_eq!(door_effects.common_message, Some(5));
    assert_eq!(door_effects.positional_sounds.len(), 1);
    assert_eq!(door_effects.positional_sounds[0].se_id, 11002);
    assert_eq!(door_effects.positional_sounds[0].pos, [0.0, -500.0, 500.0]);
    assert!(flags.get(648));

    let dismiss_effects = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        31,
        &input,
        true,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::Idle);
    assert!(dismiss_effects.clear_message);
    assert_eq!(dismiss_effects.player_lock_change, Some(false));
}

#[test]
fn item_pickup_crouch_and_stand_and_progress_check() {
    let ev_pos = [];
    let ev_list = [EventListEntry {
        flag: 0x3fff,
        ..Default::default()
    }];
    let item_list = [
        ItemListEntry {
            flag: 500 << 16,
            pos_x: 0,
            pos_y: -50,
            pos_z: 300,
            ..Default::default()
        },
        ItemListEntry {
            flag: 0x3fff << 16,
            ..Default::default()
        },
    ];

    let mut flags = EventFlags::new();
    let mut trigger_state = EventTriggerState::new();
    let input = EventEvalInput {
        player_pos: [0.0, 0.0, 0.0],
        player_rot_y: 0.0,
        action_pressed: true,
        action_difficulty: ActionDifficulty::Normal,
        ..Default::default()
    };

    let w = item_check_look_point(&item_list[0], input.player_pos, input.player_rot_y);
    assert!(w > 0.0);

    let chosen = event_check(
        &ev_list,
        &item_list,
        &ev_pos,
        &flags,
        &input,
        &mut trigger_state,
    );
    assert_eq!(chosen, Some(EVENT_ITEM_INDEX_BASE));

    let mut engine = EventEngine::new();
    let mut inventory = InventoryManager::new();

    let fx0 = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        7,
        &input,
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(fx0.player_pickup_anim, Some(0x4e21));
    assert_eq!(engine.step_state.ev_e_step, 4);

    engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        7,
        &input,
        false,
        true,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(engine.step_state.ev_e_step, 1);

    let fx1 = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        7,
        &input,
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert!(flags.get(501));
    assert!(inventory.has_item(1));
    assert_eq!(fx1.common_message, Some(10));
    assert_eq!(engine.step_state.ev_e_step, 2);

    let fx2 = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        7,
        &input,
        true,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(fx2.sound_effects, vec![0x2b21]);
    assert_eq!(fx2.deleted_item_model, Some((0x0401, 500)));
    assert_eq!(engine.step_state.ev_e_step, 5);

    engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        7,
        &input,
        false,
        false,
        true,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(engine.step_state.ev_e_step, 6);

    let fx6 = engine.tick_standard(
        &ev_list,
        &item_list,
        &ev_pos,
        7,
        &input,
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inventory,
    );
    assert_eq!(fx6.player_lock_change, Some(false));
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::Idle);

    assert!(light_spot_on_off_check(true, true, &flags));
    flags.set(272, true);
    assert!(!light_spot_on_off_check(true, true, &flags));

    assert_eq!(event_progress_check(6, 0, [0.0, 0.0, 0.0], &flags), 1);
    flags.set(69, true);
    assert_eq!(event_progress_check(0, 0x17, [0.0, 0.0, 0.0], &flags), 4);
    flags.set(70, true);
    assert_eq!(event_progress_check(0, 0x18, [0.0, 0.0, 0.0], &flags), 5);
}
