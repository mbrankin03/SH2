//! Golden-vector 1:1 binary parity tests for `SH2-127`:
//! - `Event/event_sub.c` (`0x001abf50..0x001ad85b`, `sh2pc.exe` `0x004a22d0..0x004a30aa`)
//! - `Event/picture.c` (`0x001ae360..0x001af097`)
//! - `Event/event.c` `EventExecItem` (`0x001aa260`) crouch pause/restart signals
//! - `Event/item.c` helpers (`0x001ae180`, `0x001ae240`, `0x001ae330`)

use sh2_core::events::ElevatorCupboardState;
use sh2_core::types::ItemListEntry;
use sh2_core::{
    decode_cursor_axis, ev_disp_control_model_entry, ev_disp_control_model_exec,
    ev_sub_file_load_and_fade_out, ev_sub_item_get, ev_sub_item_get_and_anim, ev_sub_item_use0,
    ev_sub_map_get, ev_sub_message, ev_sub_message_retail_pc, ev_sub_movie_end, ev_sub_movie_ready,
    ev_sub_movie_start, ev_sub_picture_cursor, ev_sub_picture_display,
    ev_sub_picture_display_and_fade_in, ev_sub_picture_display_and_fade_out, ev_sub_picture_end,
    ev_sub_picture_filter, ev_sub_picture_layer, ev_sub_question, event_exec_item,
    item_combination_use_check, item_use_se_timing, picture_draw, picture_load_image, CursorBounds,
    CursorInput, CursorTintMode, DispControlModelTable, DispOnOffCommand, EvSubItemGetAndAnimInput,
    EvSubItemUseParams, EvSubPictureDisplayInput, EvSubPictureState, EventEngineEffects,
    EventFlags, EventStepState, InvalidFileLoadArgs, InventoryManager, MovieCommand,
    MovieReadyInput, MovieStartInput, MovieState, MovieTimingProfile, PictureAlphaBlend,
    PictureDrawParams, PictureFlags, PicturePrimitiveKind, PictureTestConfig, PictureUv,
    PlayerAnimSignal, ANIM_ID_PICKUP_CROUCH, ANIM_ID_PICKUP_STAND, DEMO_STATUS_MOVIE_READY_BIT,
    MOVIE_MAIN_MODE_PC, MOVIE_MAIN_MODE_PS2, PICTURE_DEFAULT_CBP, SE_ID_ITEM_GET,
    SYS_FLAG_DEMO_ACTIVE_BIT,
};

fn make_item_entry(item_kind_idx: u32, flag_id: u32) -> ItemListEntry {
    // `ItemListEntry` layout (`SLUS_202.28` `0x001aa260`): pos_x, pos_z, pos_y (f16, 0 = 0.0mm),
    // rot_y (f16), flag (`(kind << 29) | (flag_id & 0x3fff)`).
    ItemListEntry {
        pos_x: 0.0,
        pos_z: 0.0,
        pos_y: 0,
        rot_y: 0,
        flag: ((item_kind_idx & 0x7) << 29) | (flag_id & 0x3fff),
    }
}

#[test]
fn event_exec_item_crouch_emits_pause_and_restart_signals() {
    let item = make_item_entry(1, 100); // Health drink on floor (y = 0)
    let mut state = EventStepState::new();
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();

    // Step 0 -> crouch animation 0x4e21, step 4
    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 0.0, false, false, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_lock_change, Some(true));
    assert_eq!(fx.player_pickup_anim, Some(ANIM_ID_PICKUP_CROUCH));
    assert_eq!(fx.player_anim_signal, None);
    assert_eq!(state.ev_e_step, 4);

    // Step 4 -> anim_success_frame emits PlayerAnimSignal::Pause and enters step 1
    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 0.0, true, false, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_anim_signal, Some(PlayerAnimSignal::Pause));
    assert_eq!(state.ev_e_step, 1);

    // Step 1 -> grants item and displays message, enters step 2
    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 0.0, false, false, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert!(flags.get(101));
    assert!(fx.common_message.is_some());
    assert_eq!(state.ev_e_step, 2);

    // Step 2 -> message_dismissed emits PlayerAnimSignal::Restart and enters step 5
    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 0.0, false, false, true, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert!(fx.clear_message);
    assert_eq!(fx.sound_effects, vec![SE_ID_ITEM_GET]);
    assert_eq!(fx.player_anim_signal, Some(PlayerAnimSignal::Restart));
    assert_eq!(state.ev_e_step, 5);

    // Step 5 -> anim_ended enters step 6
    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 0.0, false, true, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(state.ev_e_step, 6);

    // Step 6 -> unlocks player and finishes
    let mut fx = EventEngineEffects::default();
    assert!(event_exec_item(
        &item, 0.0, false, false, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_lock_change, Some(false));
}

#[test]
fn event_exec_item_standing_skips_pause_and_restart_signals() {
    // Player at y = 200mm, item at y = 0mm -> player_y - item_y == 200.0 >= 100.0 (standing)
    let item = make_item_entry(1, 200);
    let mut state = EventStepState::new();
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();

    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 200.0, false, false, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_pickup_anim, Some(ANIM_ID_PICKUP_STAND));
    assert_eq!(state.ev_e_step, 1);

    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 200.0, false, false, false, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(state.ev_e_step, 2);

    let mut fx = EventEngineEffects::default();
    assert!(!event_exec_item(
        &item, 200.0, false, false, true, &mut state, &mut flags, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_anim_signal, None);
    assert_eq!(state.ev_e_step, 6);
}

#[test]
fn ev_sub_message_and_question_cancel_semantics() {
    let mut state = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_message(42, false, &mut state, &mut fx));
    assert_eq!(fx.stage_message, Some(42));
    assert_eq!(state.ev_s_step, 1);

    // Cancelling EvSubMessage clears ev_prog_flag_set and advances to step 2
    state.cancel();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_message(42, false, &mut state, &mut fx));
    assert!(!state.ev_prog_flag_set);
    assert!(fx.clear_message);
    assert_eq!(state.ev_s_step, 2);
    assert!(ev_sub_message(42, false, &mut state, &mut fx));

    // Retail PC EvSubMessage (0x004a22d0) does NOT clear ev_prog_flag_set on ev_cancel
    let mut pc_state = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_message_retail_pc(42, false, &mut pc_state, &mut fx));
    pc_state.cancel();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_message_retail_pc(42, false, &mut pc_state, &mut fx));
    assert!(pc_state.ev_prog_flag_set);
    assert!(fx.clear_message);
    assert_eq!(pc_state.ev_s_step, 2);
    assert!(ev_sub_message_retail_pc(42, false, &mut pc_state, &mut fx));

    // EvSubQuestion ignores ev_cancel
    let mut q_state = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_question(7, false, &mut q_state, &mut fx));
    q_state.cancel();
    assert!(!ev_sub_question(7, false, &mut q_state, &mut fx));
    assert_eq!(q_state.ev_s_step, 1);
    assert!(q_state.ev_prog_flag_set);
    assert!(!ev_sub_question(7, true, &mut q_state, &mut fx));
    assert_eq!(q_state.ev_s_step, 2);
    assert!(ev_sub_question(7, true, &mut q_state, &mut fx));
}

#[test]
fn ev_sub_item_use0_and_se_timing_tablets() {
    // Prototype Prison Tablets (0x38..=0x3a) vs retail (0x3b..=0x3d)
    assert!(!item_use_se_timing(0x38, false, false));
    assert!(item_use_se_timing(0x38, true, false));
    assert!(!item_use_se_timing(0x3b, false, true));
    assert!(item_use_se_timing(0x3b, true, true));
    // Normal key plays immediately on step 0
    assert!(item_use_se_timing(35, false, true));
    assert!(!item_use_se_timing(35, true, true));

    let mut inv = InventoryManager::new();
    inv.item_get_now(0x3b);
    let mut state = EventStepState::new();
    let params = EvSubItemUseParams {
        item_id: 0x3b,
        msg_id: 12,
        se_id: 0x2b20,
        se_pos: Some([100.0, 0.0, 200.0]),
        lock_player: true,
        retail_ids: true,
    };

    // Step 0: locks player, consumes item, shows message, does NOT play SE yet
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_use0(
        &params, false, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_lock_change, Some(true));
    assert!(!inv.has_item(0x3b));
    assert_eq!(fx.stage_message, Some(12));
    assert!(fx.positional_sounds.is_empty());

    // Step 1: message dismissed -> plays 3D SE at volume 1.0
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_use0(
        &params, true, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(fx.positional_sounds.len(), 1);
    assert_eq!(fx.positional_sounds[0].se_id, 0x2b20);
    assert!((fx.positional_sounds[0].volume - 1.0).abs() < 1e-6);

    // Step 2: unlocks player and finishes
    let mut fx = EventEngineEffects::default();
    assert!(ev_sub_item_use0(
        &params, true, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_lock_change, Some(false));

    // Retail PC 0x004a33b0: ev_cancel dismisses step 1 when lock_player is true, but not when false
    let mut cancel_state = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_use0(
        &params,
        false,
        &mut cancel_state,
        &mut inv,
        &mut fx
    ));
    cancel_state.cancel();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_use0(
        &params,
        false,
        &mut cancel_state,
        &mut inv,
        &mut fx
    ));
    assert_eq!(cancel_state.ev_s_step, 2);
    assert!(fx.clear_message);

    // Without lock_player, retail PC does NOT allow ev_cancel on step 1
    let mut unlocked = params;
    unlocked.lock_player = false;
    let mut unlocked_state = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_use0(
        &unlocked,
        false,
        &mut unlocked_state,
        &mut inv,
        &mut fx
    ));
    unlocked_state.cancel();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_use0(
        &unlocked,
        false,
        &mut unlocked_state,
        &mut inv,
        &mut fx
    ));
    assert_eq!(unlocked_state.ev_s_step, 1);
}

#[test]
fn ev_sub_item_get_and_anim_crouch_and_cancel() {
    let mut inv = InventoryManager::new();
    let mut state = EventStepState::new();
    let base_in = EvSubItemGetAndAnimInput {
        item_id: 25,
        msg_id: 88,
        item_chara_id: 0x041f,
        player_y: 50.0,
        item_y: Some(0.0), // 50 - 0 < 100 -> crouch
        anim_success_frame: false,
        anim_ended: false,
        message_dismissed: false,
    };

    // Step 0 -> crouch 0x4e21, step 9
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_get_and_anim(
        &base_in, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(state.ev_s_step, 9);
    assert_eq!(fx.player_pickup_anim, Some(ANIM_ID_PICKUP_CROUCH));

    // Step 9 -> Pause -> step 10
    let mut fx = EventEngineEffects::default();
    let mut step9_in = base_in;
    step9_in.anim_success_frame = true;
    assert!(!ev_sub_item_get_and_anim(
        &step9_in, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_anim_signal, Some(PlayerAnimSignal::Pause));
    assert_eq!(state.ev_s_step, 10);

    // Step 10 -> stage message + SE 0x2b21 -> step 3
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_get_and_anim(
        &base_in, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(fx.stage_message, Some(88));
    assert_eq!(fx.sound_effects, vec![SE_ID_ITEM_GET]);
    assert_eq!(state.ev_s_step, 3);

    // Step 3 -> message dismissed -> ItemGet + Restart + model delete -> step 8
    let mut fx = EventEngineEffects::default();
    let mut step3_in = base_in;
    step3_in.message_dismissed = true;
    assert!(!ev_sub_item_get_and_anim(
        &step3_in, &mut state, &mut inv, &mut fx
    ));
    assert!(inv.has_item(25));
    assert_eq!(fx.player_anim_signal, Some(PlayerAnimSignal::Restart));
    assert_eq!(fx.deleted_item_model, Some((0x041f, 0)));
    assert_eq!(state.ev_s_step, 8);

    // Step 8 -> anim_ended -> step 11 -> unlocks and returns true
    let mut fx = EventEngineEffects::default();
    let mut step8_in = base_in;
    step8_in.anim_ended = true;
    assert!(!ev_sub_item_get_and_anim(
        &step8_in, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(state.ev_s_step, 0xb);
    let mut fx = EventEngineEffects::default();
    assert!(ev_sub_item_get_and_anim(
        &base_in, &mut state, &mut inv, &mut fx
    ));
    assert_eq!(fx.player_lock_change, Some(false));

    // Also verify EvSubItemGet
    let mut state2 = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_get(
        26,
        0x072c,
        9,
        false,
        &mut state2,
        &mut inv,
        &mut fx
    ));
    assert_eq!(fx.sound_effects, vec![SE_ID_ITEM_GET]);
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_item_get(
        26,
        0x072c,
        9,
        true,
        &mut state2,
        &mut inv,
        &mut fx
    ));
    assert!(inv.has_item(26));
    assert_eq!(fx.deleted_item_model, Some((0x072c, 0)));
    assert!(ev_sub_item_get(
        26,
        0x072c,
        9,
        true,
        &mut state2,
        &mut inv,
        &mut fx
    ));
}

#[test]
fn ev_sub_file_load_and_fade_out_alignment_and_error() {
    let mut state = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    assert_eq!(
        ev_sub_file_load_and_fade_out(None, 0, Some(2), false, false, &mut state, &mut fx),
        Err(InvalidFileLoadArgs)
    );

    let (done, plan) =
        ev_sub_file_load_and_fade_out(Some(10), 0x801, Some(11), false, false, &mut state, &mut fx)
            .expect("valid load");
    assert!(!done);
    assert_eq!(plan.unwrap().layer_offset_bytes, 0x1000);
    assert_eq!(fx.fade_command, Some(1));
    assert_eq!(state.ev_s_step, 3);

    let (done, plan) =
        ev_sub_file_load_and_fade_out(Some(10), 0x801, Some(11), true, true, &mut state, &mut fx)
            .expect("valid sync");
    assert!(done);
    assert!(plan.is_none());
}

#[test]
fn picture_draw_and_load_image_defaults_and_gs_packing() {
    // Untextured solid rect with defaults
    let solid = picture_draw(PictureDrawParams {
        priority: 8,
        flags: PictureFlags(0),
        ..PictureDrawParams::default()
    });
    assert_eq!(solid.primitive, PicturePrimitiveKind::SolidSprite);
    assert_eq!(solid.rect_q4, [-4096, -4096, 4096, 4096]);
    assert_eq!(solid.rgba, [0x80, 0x80, 0x80, 0x80]);
    assert_eq!(solid.uv, None);
    assert_eq!(solid.sort_key, 0x100 - 8);

    // Textured 4-vertex quad strip with default STQ UVs
    let quad = picture_draw(PictureDrawParams {
        tex_slot: 1,
        priority: 3,
        flags: PictureFlags(PictureFlags::TEXTURED | PictureFlags::QUAD_STRIP),
        ..PictureDrawParams::default()
    });
    assert_eq!(quad.primitive, PicturePrimitiveKind::TexturedQuadStrip);
    assert_eq!(quad.uv, Some(PictureUv::FloatStq([0.0, 0.0, 1.0, 1.0])));

    // GS ALPHA_1 and TEST_1 bit-packing
    let alpha = PictureAlphaBlend {
        a: 2,
        b: 1,
        c: 2,
        d: 1,
        fix: 64,
    };
    assert_eq!(
        alpha.pack_gs_alpha1(),
        2 | (1 << 2) | (2 << 4) | (1 << 6) | (64u64 << 32)
    );
    let test = PictureTestConfig {
        ate: 1,
        atst: 5,
        aref: 0x80,
        afail: 0,
        date: 1,
        datm: 0,
        zte: 1,
        ztst: 2,
    };
    assert_eq!(
        test.pack_gs_test1(),
        1 | (5 << 1) | (0x80 << 4) | (1 << 14) | (1 << 16) | (2 << 17)
    );

    // PictureLoadImage defaults
    let load = picture_load_image(2, 2, -1, -1, 0x2000);
    assert_eq!(load.dma_priority, 0x12);
    assert_eq!(load.tbp0, 0x2000);
    assert_eq!(load.cbp, PICTURE_DEFAULT_CBP);
}

#[test]
fn ev_sub_picture_display_layer_and_filter_sequence() {
    let mut state = EventStepState::new();
    let mut pic = EvSubPictureState::default();
    let mut input = EvSubPictureDisplayInput {
        file_id: 55,
        msg_id: 10,
        dt: 0.25, // 4.0/s * 0.25s = 1.0 full filter in one step
        fs_sync_ready: false,
        fade_complete: false,
        confirm_or_cancel_trigger: false,
        message_dismissed: false,
    };

    // Step 0 -> queues file 55, fade 1, locks player -> step 4
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_map_get(&input, &mut state, &mut pic, &mut fx));
    assert_eq!(pic.queued_file_id, Some(55));
    assert_eq!(fx.fade_command, Some(1));
    assert_eq!(fx.player_lock_change, Some(true));
    assert_eq!(state.ev_s_step, 4);

    // Step 4 -> fs_sync + fade_complete -> fade 4 -> step 7
    input.fs_sync_ready = true;
    input.fade_complete = true;
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_picture_display(
        &input, &mut state, &mut pic, &mut fx
    ));
    assert_eq!(fx.fade_command, Some(4));
    assert_eq!(state.ev_s_step, 7);

    // Step 7 -> fade_complete -> step 2
    pic.clear_frame_commands();
    assert!(!ev_sub_picture_display(
        &input, &mut state, &mut pic, &mut fx
    ));
    assert_eq!(state.ev_s_step, 2);
    assert_eq!(pic.draw_commands.len(), 1);

    // Step 2 -> confirm triggers stage message -> step 3
    input.confirm_or_cancel_trigger = true;
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_picture_display(
        &input, &mut state, &mut pic, &mut fx
    ));
    assert_eq!(fx.stage_message, Some(10));
    assert_eq!(state.ev_s_step, 3);

    // Step 3 -> activates EvSubPictureFilter, ramps filter_alpha to 1.0 -> black quad FIX=64
    pic.clear_frame_commands();
    input.message_dismissed = true;
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_picture_display(
        &input, &mut state, &mut pic, &mut fx
    ));
    assert!((pic.filter_alpha - 1.0).abs() < 1e-6);
    assert_eq!(pic.draw_commands.len(), 2);
    let filter_quad = pic.draw_commands[1];
    assert_eq!(filter_quad.priority, 8);
    assert_eq!(filter_quad.alpha_blend.unwrap().fix, 64);
    assert_eq!(fx.fade_command, Some(2));
    assert_eq!(state.ev_s_step, 6);

    // Step 6 -> fade_complete -> step 11 -> unlocks and fades in (4)
    let mut fx = EventEngineEffects::default();
    assert!(!ev_sub_picture_display(
        &input, &mut state, &mut pic, &mut fx
    ));
    assert_eq!(state.ev_s_step, 0xb);
    let mut fx = EventEngineEffects::default();
    assert!(ev_sub_picture_display(
        &input, &mut state, &mut pic, &mut fx
    ));
    assert_eq!(fx.player_lock_change, Some(false));
    assert_eq!(fx.fade_command, Some(4));

    // Verify EvSubPictureLayer + FadeIn/FadeOut helpers
    pic.clear_frame_commands();
    ev_sub_picture_layer([-160, -160, 160, 160], 0x40, &mut pic);
    assert_eq!(pic.load_commands[0].tex_slot, 2);
    assert_eq!(pic.draw_commands[0].priority, 3);
    assert_eq!(pic.draw_commands[0].rgba[3], 0x40);

    let mut s_fade = EventStepState::new();
    let mut fx = EventEngineEffects::default();
    ev_sub_picture_filter(&mut pic);
    ev_sub_picture_end(0.125, &mut pic);
    assert!(ev_sub_picture_display_and_fade_in(
        0.0,
        true,
        &mut s_fade,
        &mut pic,
        &mut fx
    ));
    assert_eq!(fx.fade_command, Some(4));
    s_fade.ev_s_step = 0;
    assert!(ev_sub_picture_display_and_fade_out(
        0.0,
        true,
        &mut s_fade,
        &mut pic,
        &mut fx
    ));
    assert_eq!(fx.fade_command, Some(1));
}

#[test]
fn ev_sub_picture_cursor_deadzone_clamping_and_tints() {
    assert_eq!(decode_cursor_axis(0x65), 0.0);
    assert_eq!(decode_cursor_axis(0x80), 0.0);
    assert_eq!(decode_cursor_axis(0x9a), 0.0);
    assert!((decode_cursor_axis(0x00) - (-1.0)).abs() < 1e-6);
    assert!((decode_cursor_axis(0xff) - 1.0).abs() < 1e-6);

    let mut pic = EvSubPictureState::default();
    let dpad_down_right = CursorInput {
        dpad_right: true,
        dpad_down: true,
        ..CursorInput::default()
    };
    ev_sub_picture_cursor(
        CursorTintMode::Blue,
        dpad_down_right,
        1.0,
        CursorBounds::PS2,
        &mut pic,
    );
    assert!((pic.cursor_x - 153.6).abs() < 1e-4);
    assert!((pic.cursor_y - 192.0).abs() < 1e-4);

    let cmd = pic.draw_commands[0];
    assert_eq!(cmd.priority, 9);
    assert_eq!(cmd.rgba, [0x60, 0x60, 0x80, 0x80]);
    assert_eq!(
        cmd.uv,
        Some(PictureUv::IntegerQ4([0x600, 0x400, 0x8f0, 0x7f0]))
    );
    assert_eq!(cmd.rect_q4[2] - cmd.rect_q4[0], 0x180);
    assert_eq!(cmd.rect_q4[3] - cmd.rect_q4[1], 0x200);

    // Retail PC Y clamping extends to 240.0
    pic.clear_frame_commands();
    ev_sub_picture_cursor(
        CursorTintMode::Green,
        dpad_down_right,
        1.0,
        CursorBounds::PC,
        &mut pic,
    );
    assert!((pic.cursor_x - 256.0).abs() < 1e-4);
    assert!((pic.cursor_y - 240.0).abs() < 1e-4);
    assert_eq!(pic.draw_commands[0].rgba, [0x60, 0x80, 0x60, 0x80]);
}

#[test]
fn disp_control_model_and_movie_coroutines() {
    let mut table = DispControlModelTable::new();
    ev_disp_control_model_entry(&mut table, 0x12, 0x0005, 3);
    ev_disp_control_model_entry(&mut table, 0x12, 0x0005, 7);
    ev_disp_control_model_entry(&mut table, 0x12, 0x0006, -1);
    assert_eq!(table.entries().len(), 2);
    assert_eq!(table.entries()[0].key, 0x0012_0005);
    assert_eq!(table.entries()[0].mask, (1 << 3) | (1 << 7));
    assert_eq!(table.entries()[1].mask, 0);

    let normal_cmds = ev_disp_control_model_exec(&table, false);
    assert_eq!(
        normal_cmds,
        vec![
            DispOnOffCommand::Set {
                key: 0x0012_0005,
                mask: (1 << 3) | (1 << 7)
            },
            DispOnOffCommand::Set {
                key: 0x0012_0006,
                mask: 0
            },
        ]
    );
    let fast_cmds = ev_disp_control_model_exec(&table, true);
    assert_eq!(fast_cmds[0], DispOnOffCommand::Init);
    assert_eq!(
        fast_cmds[1],
        DispOnOffCommand::FastSet {
            key: 0x0012_0005,
            mask: (1 << 3) | (1 << 7)
        }
    );

    // Movie Ready / Start / End — PS2 vs Retail PC drift
    let mut m_ps2 = MovieState::default();
    let ready_ps2 = ev_sub_movie_ready(
        &MovieReadyInput {
            movie_id: 4,
            subtitle_start: 10,
            subtitle_end: 20,
            sys_flags: SYS_FLAG_DEMO_ACTIVE_BIT,
            demo_frame: 70.0,
            total_demo_frame: 100.0, // 100 - 30 == 70 -> ready on PS2
            sd_stat: 0x50,
            movie_is_sleeping: true,
            profile: MovieTimingProfile::Ps2,
        },
        &mut m_ps2,
    );
    assert_eq!(
        ready_ps2,
        vec![
            MovieCommand::PreSet { movie_id: 4 },
            MovieCommand::SetSubtitleData {
                start_id: 10,
                end_id: 20
            },
        ]
    );
    assert_ne!(m_ps2.demo_status & DEMO_STATUS_MOVIE_READY_BIT, 0);

    // Same frame 70/100 is NOT ready on Retail PC (needs >= 95.0)
    let mut m_pc = MovieState::default();
    let not_ready_pc = ev_sub_movie_ready(
        &MovieReadyInput {
            movie_id: 4,
            subtitle_start: 10,
            subtitle_end: 20,
            sys_flags: SYS_FLAG_DEMO_ACTIVE_BIT,
            demo_frame: 70.0,
            total_demo_frame: 100.0,
            sd_stat: 0x50,
            movie_is_sleeping: true,
            profile: MovieTimingProfile::RetailPc,
        },
        &mut m_pc,
    );
    assert!(not_ready_pc.is_empty());

    // Start requires frame_counter % 3 == 1
    let (started_bad, _) = ev_sub_movie_start(
        &MovieStartInput {
            movie_is_sleeping: false,
            movie_ready: true,
            trigger: true,
            frame_counter: 2,
            profile: MovieTimingProfile::Ps2,
        },
        &mut m_ps2,
    );
    assert!(!started_bad);

    let (started_ps2, start_cmds) = ev_sub_movie_start(
        &MovieStartInput {
            movie_is_sleeping: false,
            movie_ready: true,
            trigger: true,
            frame_counter: 4,
            profile: MovieTimingProfile::Ps2,
        },
        &mut m_ps2,
    );
    assert!(started_ps2);
    assert_eq!(m_ps2.main_mode, MOVIE_MAIN_MODE_PS2);
    assert_eq!(start_cmds, vec![MovieCommand::SetPauseRetain]);
    assert_eq!(
        ev_sub_movie_end(MovieTimingProfile::Ps2, &mut m_ps2),
        Some(MovieCommand::ResetFilterCommand)
    );
    assert_eq!(m_ps2.movie_fade_guard, 1);

    let (started_pc, _) = ev_sub_movie_start(
        &MovieStartInput {
            movie_is_sleeping: false,
            movie_ready: true,
            trigger: true,
            frame_counter: 7,
            profile: MovieTimingProfile::RetailPc,
        },
        &mut m_pc,
    );
    assert!(started_pc);
    assert_eq!(m_pc.main_mode, MOVIE_MAIN_MODE_PC);
}

#[test]
fn item_c_helpers_combination_ampoule_efficacy_and_shelf() {
    // ItemCombinationUseCheck: single item passes through directly
    assert_eq!(item_combination_use_check(0, 0, 35), 35);
    assert_eq!(item_combination_use_check(35, 0, 0), 35);
    assert_eq!(item_combination_use_check(0, 0, 0), 0);
    // Unsorted 2-item and 3-item retail combinations
    assert_eq!(item_combination_use_check(36, 0, 35), 86);
    assert_eq!(item_combination_use_check(61, 59, 60), 92);
    assert_eq!(item_combination_use_check(64, 62, 63), 96);
    // Invalid combination -> 0
    assert_eq!(item_combination_use_check(1, 2, 0), 0);

    // ItemAmpolueEfficacy: 600s -> 1.0, >300s -> 1.0, 150s -> 0.5, 0s -> 0.0
    let mut inv = InventoryManager::new();
    assert_eq!(inv.ampoule_efficacy(), 0.0);
    inv.item_get_now(3);
    inv.use_healing_item(3, 50.0, 100.0).unwrap();
    assert!((inv.ampoule_efficacy() - 1.0).abs() < 1e-6);
    inv.tick(299.0);
    assert!((inv.ampoule_efficacy() - 1.0).abs() < 1e-6);
    inv.tick(151.0); // 150s remaining
    assert!((inv.ampoule_efficacy() - 0.5).abs() < 1e-5);
    inv.tick(150.0); // 0s remaining
    assert_eq!(inv.ampoule_efficacy(), 0.0);

    // ItemPutForShelf dispatch to stage cupboard callback
    let mut flags = EventFlags::new();
    let mut cupboard = ElevatorCupboardState::new();
    inv.item_get_now(1);
    let moved = inv.item_put_for_shelf(1, |inventory, _item_id| {
        cupboard.deposit_all(&mut flags, inventory)
    });
    assert_eq!(moved, 1);
    assert!(!inv.has_item(1));
}
