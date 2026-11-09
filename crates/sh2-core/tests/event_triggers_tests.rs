//! Comprehensive integration and unit tests for `SH2-47`:
//! - PS2 `src/Event/event.c` spatial trigger volumes (`ev_pos` shapes `0..=13`) and `EventCheck`
//! - `EventMainStandard` / `EventExec*` sequence state machine
//! - Scripted environmental horror triggers (`StageEventDispatcher`: Toluca Prison North/South/BF,
//!   Historical Society, Blue Creek Apartments, Wood Side Apartments).

use std::f32::consts::{FRAC_PI_2, PI};
use std::mem::size_of;

use sh2_core::types::{
    char_to_float2, char_to_float4, wrap_angle, EnemyListEntry, EventCamSetData,
    EventDestShapeKind, EventDoorSound, EventListEntry, EventResultKind, EventShapeKind,
    EventTriggerKind, ItemKindEntry, ItemListEntry, ModelListEntry, StageDataHeader, DOOR_SE_TABLE,
    ITEM_KIND_TABLE,
};
use sh2_core::{
    decode_ev_pos_shape, eval_monkey_kick_206, event_check, event_check_in, event_check_look,
    event_check_look_line, event_check_look_point, event_position_set, event_progress_check,
    event_result_move_position, is_connected_stage_pair, item_check_look_point,
    light_spot_on_off_check, player_height_matches_event_y, ActionDifficulty,
    AnyoneCryProgramState, CameraOverrideCommand, CollisionWallCommand, CutawayOverlayEngine,
    EvPosShape, EventEngine, EventEvalInput, EventExecMode, EventFlags, EventTriggerState,
    InventoryManager, PuzzleState, RiddleDifficulty, Sh2Rng, SpatialSoundCommand,
    StageEventDispatcher, StageHookContext, StageId, StageProgramRunner, APART_E3FE_ROOM13_CAMERA,
    EVENT_ITEM_INDEX_BASE, FLAG_APART_E3FE_FOOTSTEPS_DONE, FLAG_APART_ROOM209_WHISPER_DONE,
    FLAG_APART_W2F_CORRIDOR_DONE, FLAG_PRISON_BATHROOM_JUMPSCARE_DONE,
    FLAG_PRISON_BATHROOM_KNOCKED, FLAG_PRISON_BF_ROOM77_OPEN, FLAG_PRISON_N_ROOM66_WALL,
    FLAG_PRISON_S_BARS_ACTIVE, FLAG_PRISON_S_BARS_BOOST, FLAG_PRISON_S_BARS_COUNTING,
    FLAG_SOCIETY_ROOM5C_TRIGGERED, PROG_KEY_A, SE_APART_ANYONE_CRY, SE_APART_E3FE_FOOTSTEPS,
    SE_APART_MONKEY_KICK_206, SE_APART_ROOM205_TV_STATIC, SE_APART_ROOM209_WHISPER,
    SE_APART_W2F_CORRIDOR, SE_PRISON_BATHROOM_STALL_BANG, SE_PRISON_INVISIBLE_PRISONER,
    SE_PRISON_ROOM68_MECHANISM, SE_SOCIETY_ROOM_5C_HORROR, SE_SOCIETY_STAIRCASE_AMBIENT,
};

fn push_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

#[test]
fn ps2_event_struct_sizes_and_bitfields_match_rosetta_stone() {
    assert_eq!(size_of::<StageDataHeader>(), 52);
    assert_eq!(size_of::<EventListEntry>(), 16);
    assert_eq!(size_of::<ItemListEntry>(), 16);
    assert_eq!(size_of::<ItemKindEntry>(), 8);
    assert_eq!(size_of::<EventDoorSound>(), 12);
    assert_eq!(size_of::<ModelListEntry>(), 48);
    assert_eq!(size_of::<EnemyListEntry>(), 20);
    assert_eq!(size_of::<EventCamSetData>(), 64);

    assert_eq!(char_to_float2((-500i16).to_le_bytes()), -500.0);
    assert_eq!(char_to_float4(123.5f32.to_le_bytes()), 123.5);
    assert!(
        (wrap_angle(3.0 * PI) - (-PI)).abs() < 1e-5 || (wrap_angle(3.0 * PI) - PI).abs() < 1e-5
    );

    // Pack an EventListEntry and verify all 22 EventListElement selectors
    let entry = EventListEntry {
        // flag_0_inv=1, flag_0_toggle=1, flag_0=283, flag_1_inv=0, flag_1_toggle=1, flag_1=648
        flag: (1 << 31) | (1 << 30) | (283 << 16) | (1 << 14) | 648,
        // trigger=2 (ActionLook), offset=0x120, shape=3 (LinePosZ), item=28, consume=1, demo_dis=1
        cond: (2 << 28) | (0x120 << 16) | (3 << 12) | (28 << 4) | (1 << 3) | (1 << 2),
        // rslt=6 (MovePlusProgram), dest_offset=0x240, dest_shape=4, fade=7, obs=1
        rslt0: (6 << 28) | (0x240 << 16) | (4 << 12) | (7 << 5) | (1 << 4),
        // door/stage=11, prog/msg=5, result_flag=649
        rslt1: (11 << 22) | (5 << 14) | 649,
    };

    assert_eq!(entry.element(1), 1);
    assert_eq!(entry.element(2), 1);
    assert_eq!(entry.element(3), 283);
    assert_eq!(entry.element(4), 0);
    assert_eq!(entry.element(5), 1);
    assert_eq!(entry.element(6), 648);
    assert_eq!(entry.element(7), 2);
    assert_eq!(entry.element(8), 0x120);
    assert_eq!(entry.element(9), 3);
    assert_eq!(entry.element(10), 28);
    assert_eq!(entry.element(11), 1);
    assert_eq!(entry.element(12), 1);
    assert_eq!(entry.element(13), 6);
    assert_eq!(entry.element(14), 0x240);
    assert_eq!(entry.element(15), 4);
    assert_eq!(entry.element(16), 7);
    assert_eq!(entry.element(17), 1);
    assert_eq!(entry.element(18), 11);
    assert_eq!(entry.element(19), 5);
    assert_eq!(entry.element(20), 5);
    assert_eq!(entry.element(21), 5);
    assert_eq!(entry.element(22), 649);

    assert_eq!(entry.trigger_kind(), Some(EventTriggerKind::ActionLook));
    assert_eq!(entry.shape_kind(), Some(EventShapeKind::LinePosZ));
    assert_eq!(entry.result_kind(), Some(EventResultKind::MovePlusProgram));
    assert_eq!(
        entry.dest_shape_kind(),
        Some(EventDestShapeKind::DoorLineNegX)
    );

    // Verify PS2 tables
    assert_eq!(DOOR_SE_TABLE[1].open, 11000);
    assert_eq!(DOOR_SE_TABLE[1].lock, 11002);
    assert_eq!(ITEM_KIND_TABLE[0].item_id, 1);
    assert_eq!(ITEM_KIND_TABLE[0].chr_id, 0x0401);
    assert!(is_connected_stage_pair(6, 7));
    assert!(is_connected_stage_pair(11, 10));
    assert!(!is_connected_stage_pair(6, 11));
}

#[test]
fn spatial_trigger_geometry_look_point_line_and_polygons() {
    // 1. Point look test (0x001a8a30)
    // Player at (0, 0, 0) looking along +Z (rot_y = 0.0), point at (0, 0, 500) with radius 1000
    let w_center = event_check_look_point(0.0, 0.0, 0.0, 0.0, 500.0, 1000.0);
    assert!((w_center - 0.5).abs() < 1e-4);

    // Looking 90 degrees away (rot_y = PI/2) -> outside ±45° cone -> 0.0
    assert_eq!(
        event_check_look_point(0.0, 0.0, FRAC_PI_2, 0.0, 500.0, 1000.0),
        0.0
    );

    // 2. Line segment look test (0x001a8b10)
    // Segment from (-500, 1000) to (500, 1000): dx = 1000, dz = 0 -> normal = atan2(1000, 0) - PI/2 = 0.0
    let w_line = event_check_look_line(0.0, 500.0, 0.0, -500.0, 1000.0, 500.0, 1000.0, 1000.0);
    assert!((w_line - 0.5).abs() < 1e-4);
    // Approaching from the back side facing -Z (rot_y = PI) -> rejected by normal check!
    let w_back = event_check_look_line(0.0, 1500.0, PI, -500.0, 1000.0, 500.0, 1000.0, 1000.0);
    assert_eq!(w_back, 0.0);

    // 3. Height window test (py - 500 <= y <= py + 100, or y >= 45000)
    assert!(player_height_matches_event_y(0.0, -500.0));
    assert!(player_height_matches_event_y(0.0, 100.0));
    assert!(!player_height_matches_event_y(0.0, -501.0));
    assert!(!player_height_matches_event_y(0.0, 101.0));
    assert!(player_height_matches_event_y(0.0, 45_000.0));

    // 4. Decode and test Rectangle (11), Triangle (12), and Quad (13) in ev_pos
    let mut ev_pos = Vec::new();
    // Offset 0: Rectangle at (100, 0, 200) with w=300, d=400 (10 bytes)
    push_i16(&mut ev_pos, 100);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 200);
    push_i16(&mut ev_pos, 300);
    push_i16(&mut ev_pos, 400);

    // Offset 10: Triangle (0, 0)-(1000, 0)-(0, 1000) at y = 0 (14 bytes)
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);

    // Offset 24: Quad (0, 0)-(1000, 0)-(1000, 1000)-(0, 1000) at y = 0 (18 bytes)
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);

    assert!(event_check_in(
        &ev_pos,
        0,
        EventShapeKind::Rectangle,
        [250.0, 0.0, 400.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        0,
        EventShapeKind::Rectangle,
        [50.0, 0.0, 400.0]
    ));

    assert!(event_check_in(
        &ev_pos,
        10,
        EventShapeKind::Triangle,
        [200.0, 0.0, 200.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        10,
        EventShapeKind::Triangle,
        [800.0, 0.0, 800.0]
    ));

    assert!(event_check_in(
        &ev_pos,
        24,
        EventShapeKind::Quad,
        [800.0, 0.0, 800.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        24,
        EventShapeKind::Quad,
        [1200.0, 0.0, 800.0]
    ));

    // 5. Polyline decoding (shape 6 = 3-vertex polyline, 16 bytes)
    let mut poly_pos = Vec::new();
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 600);
    push_i16(&mut poly_pos, 1000);
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 1000);
    push_i16(&mut poly_pos, 1000);
    let decoded = decode_ev_pos_shape(&poly_pos, 0, EventShapeKind::Polyline3).unwrap();
    assert_eq!(
        decoded,
        EvPosShape::Polyline {
            y: 0.0,
            radius: 600.0,
            vertices: vec![[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0]],
        }
    );
    let w_poly = event_check_look(
        &poly_pos,
        0,
        EventShapeKind::Polyline3,
        [500.0, 0.0, -300.0],
        0.0,
        true,
    );
    assert!((w_poly - 0.5).abs() < 1e-4);

    // 6. EventPositionSet and EventResultMovePosition
    let mut move_pos = Vec::new();
    // DoorLinePosZ at (1000, 0, 2000), radius=500, len=800
    push_i16(&mut move_pos, 1000);
    push_i16(&mut move_pos, 0);
    push_i16(&mut move_pos, 2000);
    push_i16(&mut move_pos, 500);
    push_i16(&mut move_pos, 800);
    assert_eq!(
        event_position_set(&move_pos, 0, EventShapeKind::LinePosZ),
        Some([1000.0, 0.0, 2400.0])
    );
    let (dest_p, dest_rot) =
        event_result_move_position(&move_pos, 0, EventDestShapeKind::DoorLinePosZ).unwrap();
    assert_eq!(dest_p, [1250.0, 0.0, 2400.0]);
    assert!((dest_rot - FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn event_engine_state_machine_executes_flag_message_door_move_item_and_save() {
    let mut ev_pos = Vec::new();
    // Offset 0: Point at (0, 0, 500) radius 1000 (8 bytes)
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 500);
    push_i16(&mut ev_pos, 1000);
    // Offset 8: Dest PointWithAngle at (2000, 0, 3000), angle 90 deg (8 bytes)
    push_i16(&mut ev_pos, 2000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 3000);
    push_i16(&mut ev_pos, 90);

    let ev_list = [
        // 0: TouchEdge (cond=1) at offset 0 (Point) -> FlagSet(flag 100), requires !flag(100)
        EventListEntry {
            flag: (1 << 31) | (100 << 16),
            cond: (1 << 28),
            rslt0: 1 << 28,
            rslt1: 100,
        },
        // 1: ActionLook (cond=2) at offset 0 (Point) -> DoorLocked (rslt=7), door_se=1, msg=0xff ("It's locked."), sets flag 648
        EventListEntry {
            flag: (1 << 31) | (648 << 16),
            cond: (2 << 28),
            rslt0: 7 << 28,
            rslt1: (1 << 22) | (0xff << 14) | 648,
        },
        // 2: Sentinel
        EventListEntry {
            flag: 0x3fff,
            cond: 0,
            rslt0: 0,
            rslt1: 0,
        },
    ];

    let item_list = [
        // Sentinel
        ItemListEntry {
            flag: 0x3fff << 16,
            ..Default::default()
        },
    ];

    let mut engine = EventEngine::new();
    let mut flags = EventFlags::new();
    let mut inventory = InventoryManager::new();

    // Frame 1: Player walks into Point at (0, 0, 200) -> triggers entry 0 (TouchEdge -> FlagSet(100))
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

    // Frame 2: Player presses Action looking at the locked door (entry 1)
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
    assert_eq!(door_effects.common_message, Some(5)); // "It's locked."
    assert_eq!(door_effects.positional_sounds.len(), 1);
    assert_eq!(door_effects.positional_sounds[0].se_id, 11002);
    assert_eq!(door_effects.positional_sounds[0].pos, [0.0, -500.0, 500.0]);
    assert!(flags.get(648));

    // Frame 3: Player dismisses the locked door message -> unlocks player and returns to Idle
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
        // Health drink (kind=0) at (0, -50, 300), base flag 500 (sets flag 501 on pickup)
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

    // Step 0 -> 4 (crouch pickup anim 0x4e21 since player_y (0) - item_y (-50) = 50 < 100)
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

    // Step 4 -> 1 (anim_success_frame = true)
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

    // Step 1 -> 2 (awards item 1 = Health drink, sets flag 501, shows common message 10)
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

    // Step 2 -> 5 (message dismissed -> plays SE 0x2b21, deletes model 0x0401)
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

    // Step 5 -> 6 (anim_ended = true)
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

    // Step 6 -> Idle (unlocks player)
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

    // Verify LightSpotOnOffCheck and EventProgressCheck
    assert!(light_spot_on_off_check(true, true, &flags));
    flags.set(272, true);
    assert!(!light_spot_on_off_check(true, true, &flags));

    assert_eq!(event_progress_check(6, 0, [0.0, 0.0, 0.0], &flags), 1);
    flags.set(69, true);
    assert_eq!(event_progress_check(0, 0x17, [0.0, 0.0, 0.0], &flags), 4);
    flags.set(70, true);
    assert_eq!(event_progress_check(0, 0x18, [0.0, 0.0, 0.0], &flags), 5);
}

#[test]
fn prison_north_bathroom_stall_knock_and_jumpscare_sequence() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(42);

    // Pre-set knocked (648) & jumpscare done (649) and verify EvRoomInit can reset them (70% chance)
    let mut reset_observed = false;
    for _ in 0..10 {
        flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
        flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, true);
        dispatcher.on_room_init(StageId::PrisonN, 0x73, &mut flags, &mut rng);
        assert!(
            dispatcher.prison_n_bathroom_z_threshold >= -20_000.0
                && dispatcher.prison_n_bathroom_z_threshold <= -19_200.0
        );
        if !flags.get(FLAG_PRISON_BATHROOM_KNOCKED)
            && !flags.get(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE)
        {
            reset_observed = true;
            break;
        }
    }
    assert!(
        reset_observed,
        "EvRoomInit should clear flags 648 & 649 when frand() > 0.3"
    );

    // Player knocks on locked stall door -> sets flag(648)
    flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
    flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, false);

    // While player is still near the stalls (z = -20500.0 < threshold), nothing triggers
    let ctx_near_stall = StageHookContext {
        stage: StageId::PrisonN,
        room_id: 0x73,
        dt: 1.0 / 30.0,
        player_pos: [-138_700.0, 0.0, -20_500.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };
    let out_near = dispatcher.on_all_time(&ctx_near_stall, &mut flags, &mut rng);
    assert!(out_near.sounds.is_empty());

    // Player walks toward bathroom exit door (z = -19000.0 > threshold):
    // 60% chance (frand() > 0.4) to fire SE 0x427d at [-138700.0, -50.0, -21000.0] and set flag(649)
    let ctx_at_exit = StageHookContext {
        stage: StageId::PrisonN,
        room_id: 0x73,
        dt: 1.0 / 30.0,
        player_pos: [-138_700.0, 0.0, -19_000.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };

    let mut bang_triggered = false;
    for _ in 0..20 {
        flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
        flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, false);
        let out = dispatcher.on_all_time(&ctx_at_exit, &mut flags, &mut rng);
        if !out.sounds.is_empty() {
            assert_eq!(
                out.sounds[0],
                SpatialSoundCommand::CallPos {
                    se_id: SE_PRISON_BATHROOM_STALL_BANG,
                    volume: 1.0,
                    pos: [-138_700.0, -50.0, -21_000.0],
                    channel: 0,
                }
            );
            assert!(flags.get(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE));
            bang_triggered = true;
            break;
        } else {
            // 40% branch: clears flag(648) so knocking again can trigger it later
            assert!(!flags.get(FLAG_PRISON_BATHROOM_KNOCKED));
        }
    }
    assert!(
        bang_triggered,
        "Bathroom stall jumpscare SE 0x427d must trigger"
    );

    // Also verify PrisonN Room 0x66 and Room 0x67
    flags.set(FLAG_PRISON_N_ROOM66_WALL, true);
    let out_66 = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::PrisonN,
            room_id: 0x66,
            dt: 1.0 / 30.0,
            player_pos: [0.0; 3],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_66.collision_commands,
        vec![CollisionWallCommand::PrisonNPs45Wall]
    );
    assert_eq!(out_66.model_states[0].model_id, 0x2d);
    assert_eq!(out_66.model_states[0].state, 0);
}

#[test]
fn prison_south_invisible_prisoner_ritual_emitter_and_cell_bars_timer() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(777);

    // Initialize Room 0x72 (southern cellblock)
    dispatcher.on_room_init(StageId::PrisonS, 0x72, &mut flags, &mut rng);
    assert!(
        dispatcher.prison_s_prisoner_timer >= 10.0 && dispatcher.prison_s_prisoner_timer <= 15.0
    );

    // Tick enough time for the invisible prisoner timer to expire and emit SE 0x4682
    let ctx = StageHookContext {
        stage: StageId::PrisonS,
        room_id: 0x72,
        dt: 16.0,
        player_pos: [-140_000.0, 0.0, 20_000.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };
    let out = dispatcher.on_all_time(&ctx, &mut flags, &mut rng);
    assert_eq!(out.sounds.len(), 2); // PosSet + CallPos
    match out.sounds[1] {
        SpatialSoundCommand::CallPos {
            se_id,
            volume,
            pos,
            channel,
        } => {
            assert_eq!(se_id, SE_PRISON_INVISIBLE_PRISONER);
            assert_eq!(volume, 1.0);
            assert_eq!(channel, 0);
            assert!(pos[0] >= -146_000.0 && pos[0] <= -134_000.0);
            assert_eq!(pos[1], -500.0);
            assert!(pos[2] >= 10_400.0 && pos[2] <= 32_000.0);
        }
        other => panic!("Expected CallPos for SE 0x4682, got {other:?}"),
    }

    // Verify Oppressor cell bars timer (flags 288, 289, 290)
    flags.set(FLAG_PRISON_S_BARS_ACTIVE, true);
    flags.set(FLAG_PRISON_S_BARS_COUNTING, true);
    flags.set(FLAG_PRISON_S_BARS_BOOST, true);
    let ctx_bars = StageHookContext {
        stage: StageId::PrisonS,
        room_id: 0x68,
        dt: 11.0, // + 10.0 boost = 21.0 > 20.0 -> clears flag 288
        player_pos: [0.0; 3],
        player_rot_y: 0.0,
        active_sounds: [SE_PRISON_ROOM68_MECHANISM, 0, 0, 0],
    };
    let out_bars = dispatcher.on_all_time(&ctx_bars, &mut flags, &mut rng);
    assert!(!flags.get(FLAG_PRISON_S_BARS_BOOST));
    assert!(!flags.get(FLAG_PRISON_S_BARS_ACTIVE));
    assert_eq!(out_bars.model_states[0].model_id, 0x1d);
    assert_eq!(out_bars.model_states[0].state, -1);

    // Verify Prison Basement Room 0x77
    flags.set(FLAG_PRISON_BF_ROOM77_OPEN, false);
    let out_bf = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::PrisonBf,
            room_id: 0x77,
            dt: 0.1,
            player_pos: [0.0; 3],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_bf.collision_commands,
        vec![CollisionWallCommand::PrisonBfPs57Wall]
    );
    assert_eq!(out_bf.model_states[0].model_id, 0x39);
    assert_eq!(out_bf.model_states[0].state, 0);
}

#[test]
fn blue_creek_room209_orbiting_whisper_and_w2f_corridor_sound() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(1234);

    // Enter Room 0x28 (Room 209)
    dispatcher.on_room_init(StageId::ApartW2f, 0x28, &mut flags, &mut rng);
    assert!(
        dispatcher.apart_w2f_room209_x_threshold >= 20_000.0
            && dispatcher.apart_w2f_room209_x_threshold <= 21_200.0
    );

    // Step 1: Player crosses X threshold into Room 209 -> starts SE 0x9c70, sets flag(547),
    // and immediately positions the orbiting whisper at radius 2500.0 around the player!
    let ctx_start = StageHookContext {
        stage: StageId::ApartW2f,
        room_id: 0x28,
        dt: 0.5,
        player_pos: [19_500.0, -500.0, -10_000.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };
    let out1 = dispatcher.on_all_time(&ctx_start, &mut flags, &mut rng);
    assert!(flags.get(FLAG_APART_ROOM209_WHISPER_DONE));
    assert_eq!(out1.sounds.len(), 2); // CallPos + initial PosMove
    assert_eq!(
        out1.sounds[0],
        SpatialSoundCommand::CallPos {
            se_id: SE_APART_ROOM209_WHISPER,
            volume: 1.0,
            pos: [0.0, 0.0, 0.0],
            channel: 6,
        }
    );
    let pos1 = match out1.sounds[1] {
        SpatialSoundCommand::PosMove { se_id, pos, .. } => {
            assert_eq!(se_id, SE_APART_ROOM209_WHISPER);
            let dx = pos[0] - ctx_start.player_pos[0];
            let dz = pos[2] - ctx_start.player_pos[2];
            let dist = (dx * dx + dz * dz).sqrt();
            assert!((dist - 2500.0).abs() < 1.0);
            pos
        }
        other => panic!("Expected PosMove, got {other:?}"),
    };

    // Step 2: Next frame while SE 0x9c70 is still playing -> emitter orbits by dt * (PI / 8.0)
    let ctx_playing = StageHookContext {
        stage: StageId::ApartW2f,
        room_id: 0x28,
        dt: 1.0,
        player_pos: [19_500.0, -500.0, -10_000.0],
        player_rot_y: 0.0,
        active_sounds: [SE_APART_ROOM209_WHISPER, 0, 0, 0],
    };
    let out2 = dispatcher.on_all_time(&ctx_playing, &mut flags, &mut rng);
    assert_eq!(out2.sounds.len(), 1);
    match out2.sounds[0] {
        SpatialSoundCommand::PosMove { pos: pos2, .. } => {
            assert!((pos2[0] - pos1[0]).abs() > 50.0 || (pos2[2] - pos1[2]).abs() > 50.0);
        }
        other => panic!("Expected PosMove, got {other:?}"),
    }

    // Verify Blue Creek W2F Corridor (Room 0x26): arms when z > -6500.0, fires SE 0x9c6c when z < -6800.0
    dispatcher.on_room_init(StageId::ApartW2f, 0x26, &mut flags, &mut rng);
    let out_arm = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartW2f,
            room_id: 0x26,
            dt: 0.1,
            player_pos: [100_800.0, -500.0, -6400.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(out_arm.sounds.is_empty());
    assert!(dispatcher.apart_w2f_corridor_armed);

    let out_fire = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartW2f,
            room_id: 0x26,
            dt: 0.1,
            player_pos: [100_800.0, -500.0, -6900.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(flags.get(FLAG_APART_W2F_CORRIDOR_DONE));
    assert_eq!(out_fire.sounds.len(), 1);
    match out_fire.sounds[0] {
        SpatialSoundCommand::CallPos { se_id, channel, .. } => {
            assert_eq!(se_id, SE_APART_W2F_CORRIDOR);
            assert_eq!(channel, 2);
        }
        other => panic!("Expected CallPos for SE 0x9c6c, got {other:?}"),
    }
}

#[test]
fn wood_side_apartments_footsteps_anyone_cry_monkey_kick_and_society_staircase() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(99);

    // 1. Wood Side E 3F East Room 0x1f walking footsteps (SE 0x9c51 moving along -X at 700 units/s)
    dispatcher.on_stage_init(StageId::ApartE3fe, &mut flags);
    let out_start = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartE3fe,
            room_id: 0x1f,
            dt: 0.1,
            player_pos: [60_000.0, 0.0, 16_500.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(flags.get(FLAG_APART_E3FE_FOOTSTEPS_DONE));
    assert_eq!(
        out_start.sounds,
        vec![SpatialSoundCommand::CallPos {
            se_id: SE_APART_E3FE_FOOTSTEPS,
            volume: 1.0,
            pos: [60_000.0, 0.0, 11_350.0],
            channel: 2,
        }]
    );

    let out_move = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartE3fe,
            room_id: 0x1f,
            dt: 2.0,
            player_pos: [60_000.0, 0.0, 16_500.0],
            player_rot_y: 0.0,
            active_sounds: [SE_APART_E3FE_FOOTSTEPS, 0, 0, 0],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_move.sounds,
        vec![SpatialSoundCommand::PosMove {
            se_id: SE_APART_E3FE_FOOTSTEPS,
            volume: 1.0,
            pos: [60_000.0 - 1400.0, 0.0, 11_350.0],
        }]
    );

    // Room 0x13 key camera lock & release
    flags.set(91, true);
    let out_cam_lock = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartE3fe,
            room_id: 0x13,
            dt: 0.1,
            player_pos: [-22_000.0, 0.0, -101_500.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_cam_lock.camera_override,
        Some(CameraOverrideCommand::Lock(APART_E3FE_ROOM13_CAMERA))
    );

    // 2. Wood Side E 2F EvProgAnyoneCry & EvProgMonkeyKick206 & Room 0x18 TV static
    let mut cry = AnyoneCryProgramState::new();
    let (done0, snd0) = cry.tick(0.0, false, false);
    assert!(!done0);
    assert!(cry.player_locked);
    assert_eq!(cry.stage_anim, Some(0x4e22));
    assert_eq!(
        snd0,
        Some(SpatialSoundCommand::CallPos {
            se_id: SE_APART_ANYONE_CRY,
            volume: 1.0,
            pos: [-99_400.0, -500.0, 26_400.0],
            channel: 6,
        })
    );
    let (done1, _) = cry.tick(2.1, true, true);
    assert!(!done1);
    assert_eq!(cry.s_step, 0x0d);
    let (done2, _) = cry.tick(0.0, true, true);
    assert!(done2);
    assert!(!cry.player_locked);

    // Also test via StageProgramRunner (7, 1) and (7, 17)
    let mut runner = StageProgramRunner::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut overlay = CutawayOverlayEngine::new();
    runner.start(7, 1, 66);
    assert!(runner.tick(
        0,
        0,
        0.1,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert_eq!(runner.sound_effects(), &[SE_APART_ANYONE_CRY]);
    assert!(runner.tick(
        0,
        0,
        2.0,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert_eq!(runner.active_message_id(), Some(0));
    assert!(runner.tick(
        PROG_KEY_A,
        0,
        0.1,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert!(!runner.tick(
        0,
        0,
        0.1,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert!(flags.get(66));

    // EvProgMonkeyKick206 gated by !flag(69)
    assert_eq!(
        eval_monkey_kick_206(&flags),
        Some(SpatialSoundCommand::CallPos {
            se_id: SE_APART_MONKEY_KICK_206,
            volume: 1.0,
            pos: [-99_000.0, -500.0, 13_600.0],
            channel: 2,
        })
    );
    flags.set(69, true);
    assert_eq!(eval_monkey_kick_206(&flags), None);

    // Room 0x18 TV static after flag(69)
    let tv_load = dispatcher.on_sound_call_after_load(StageId::ApartE2f, 0x18, &mut flags);
    assert_eq!(tv_load.sounds.len(), 1);
    assert_eq!(
        tv_load.sounds[0],
        SpatialSoundCommand::CallPos {
            se_id: SE_APART_ROOM205_TV_STATIC,
            volume: 0.3,
            pos: [-58_082.375, -356.5, 19_011.39],
            channel: 12,
        }
    );
    assert!(flags.get(70));

    // 3. Historical Society Room 0x5d staircase ambient & Room 0x5c trigger
    dispatcher.on_room_init(StageId::Society, 0x5d, &mut flags, &mut rng);
    let out_soc_stair = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::Society,
            room_id: 0x5d,
            dt: 5.0,
            player_pos: [-11_520.0, 17_500.0, 40_000.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(out_soc_stair.sounds.len(), 1);
    match out_soc_stair.sounds[0] {
        SpatialSoundCommand::CallPos {
            se_id,
            volume,
            pos,
            channel,
        } => {
            assert_eq!(se_id, SE_SOCIETY_STAIRCASE_AMBIENT);
            assert!((volume - 0.25).abs() < 1e-4); // vol = 0.5 * 0.5 = 0.25 at y = 17500
            assert_eq!(pos[1], 42_670.0);
            assert_eq!(channel, 6);
        }
        other => panic!("Expected CallPos for SE 0x9c73, got {other:?}"),
    }

    flags.set(FLAG_SOCIETY_ROOM5C_TRIGGERED, false);
    let out_soc_5c = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::Society,
            room_id: 0x5c,
            dt: 0.1,
            player_pos: [-19_000.0, 0.0, -18_900.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(flags.get(FLAG_SOCIETY_ROOM5C_TRIGGERED));
    assert_eq!(
        out_soc_5c.sounds,
        vec![SpatialSoundCommand::CallPos {
            se_id: SE_SOCIETY_ROOM_5C_HORROR,
            volume: 1.0,
            pos: [-22_000.0, -500.0, -18_900.0],
            channel: 8,
        }]
    );
}
