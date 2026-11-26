//! Golden-vector 1:1 parity tests for `SH2-165`:
//! - `EventTriggerKind` (`0..=10`), first-match-wins, and `consume_item` (`5..=10`)
//!   (`SLUS_202.28` `EventCheck` `0x001a7ed0`, `sh2pc.exe` `0x0048e070`)
//! - `EventResultKind` (`3=Program, 4=Move, 5=MovePlusProgram, 6=Message`)
//!   (`SLUS_202.28` `EventMainStandard` `0x001a7b50`, `sh2pc.exe` `0x0048e610`)
//! - `EventListEntry::is_sentinel()` (`cond == 0 && rslt0 == 0` at `0x001a8020` / `0x0048e1a2`)
//! - `(cond >> 2) & 1` dark-room / `LightSpotOnOffCheck() == 0` gating (`0x001a80ec`)
//! - 3D `Item_List` (`gi_list`) `CharToFloat2` f16 (`pos_y`, `rot_y`) & bitfields
//!   (`ItemListElement` `0x001a9320` / `0x0048b480`, `EventItemConditionCheck` `0x001ab1a0`)
//! - `ClearData` ending bitmask (`sh2pc.exe` `0x0048d330` & `ClearEndKind`)
//! - Room 208 Grandfather Clock solved flag (`81`) & Campaign `PROG_KEY_LEFT/RIGHT` forwarding
//! - `EventMainStandard` 16-iteration same-frame chaining loop, `active_prog_completion_flag`,
//!   `MovePlusProgram` chaining, `Message` result flags, and authentic door open/close 3D SFX.

use sh2_core::campaign::retail_events::evaluate_retail_stage_events;
use sh2_core::campaign::{
    resolve_stage_manifest, CampaignOrchestrator, CampaignPhase, CampaignPickupSpawn,
    CampaignTickInput, ScreenFadeState,
};
use sh2_core::config::{ActionDifficulty, OutdoorStreamingMode};
use sh2_core::endings::ClearEndKind;
use sh2_core::events::stages_act2::RetailItemSpawn;
use sh2_core::events::{
    evaluate_event_entry, event_check, ClearData, EventEngine, EventEvalInput, EventExecMode,
    EventFlags, StageProgramRunner, MAX_EVENT_CHAIN_ITERATIONS, PROG_KEY_A, PROG_KEY_LEFT,
    PROG_KEY_RIGHT, PZ_CLOCK,
};
use sh2_core::inventory::InventoryManager;
use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;
use sh2_core::types::{
    char_to_float2, float_to_char2, EventDestShapeKind, EventListEntry, EventResultKind,
    EventShapeKind,
};
use sh2_core::world::{
    resolve_active_room_pickups, PushableObjectKind, FLAG_APART_CLOCK_PUSHED,
    FLAG_APART_CLOCK_SOLVED,
};

fn push_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_f16(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&float_to_char2(v));
}

#[test]
fn event_check_triggers_0_through_10_first_match_wins_and_consume_item() {
    // ev_pos:
    //   offset 0:  Point at (0, 0, 200) (10 bytes)
    //   offset 10: Rectangle at (-100, 0, -100), w=200, d=200 (14 bytes)
    let mut ev_pos = Vec::new();
    push_f32(&mut ev_pos, 0.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 200.0);
    push_f32(&mut ev_pos, -100.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, -100.0);
    push_f16(&mut ev_pos, 200.0);
    push_f16(&mut ev_pos, 200.0);

    let make_entry = |trg: u32, off: u32, shape: u32, item: u32, consume: bool, flag_out: u32| {
        EventListEntry {
            flag: 0,
            cond: (trg << 28)
                | (off << 16)
                | (shape << 12)
                | (item << 4)
                | (u32::from(consume) << 3),
            rslt0: 1 << 28, // FlagSet
            rslt1: flag_out,
        }
    };

    let flags = EventFlags::new();
    let mut inv = InventoryManager::new();
    let key_id: u8 = 44;

    let idle = EventEvalInput {
        player_pos: [0.0, 0.0, 0.0],
        player_rot_y: 0.0,
        action_pressed: false,
        used_item_id: None,
        is_dark_room: false,
        flashlight_on: true,
        action_difficulty: ActionDifficulty::Normal,
        player_can_search: true,
    };
    let action = EventEvalInput {
        action_pressed: true,
        ..idle
    };
    let use_key = EventEvalInput {
        used_item_id: Some(key_id),
        ..idle
    };

    // 0 & 1: Unconditional
    assert!(evaluate_event_entry(
        &make_entry(0, 0, 0, 0, false, 1),
        &ev_pos,
        &flags,
        &inv,
        &idle
    ));
    assert!(evaluate_event_entry(
        &make_entry(1, 0, 0, 0, false, 1),
        &ev_pos,
        &flags,
        &inv,
        &idle
    ));

    // 2: Action + Look (Point) and Action + In (Rectangle)
    let e2_look = make_entry(2, 0, EventShapeKind::Point as u32, 0, false, 1);
    let e2_in = make_entry(2, 10, EventShapeKind::Rectangle as u32, 0, false, 1);
    assert!(!evaluate_event_entry(
        &e2_look, &ev_pos, &flags, &inv, &idle
    ));
    assert!(evaluate_event_entry(
        &e2_look, &ev_pos, &flags, &inv, &action
    ));
    assert!(evaluate_event_entry(&e2_in, &ev_pos, &flags, &inv, &action));

    // 3: PassiveLook (Point), 4: PassiveIn (Rectangle)
    assert!(evaluate_event_entry(
        &make_entry(3, 0, EventShapeKind::Point as u32, 0, false, 1),
        &ev_pos,
        &flags,
        &inv,
        &idle
    ));
    assert!(evaluate_event_entry(
        &make_entry(4, 10, EventShapeKind::Rectangle as u32, 0, false, 1),
        &ev_pos,
        &flags,
        &inv,
        &idle
    ));

    // 5: UseItemLook, 6: UseItemIn
    let e5 = make_entry(5, 0, EventShapeKind::Point as u32, key_id as u32, true, 10);
    let e6 = make_entry(
        6,
        10,
        EventShapeKind::Rectangle as u32,
        key_id as u32,
        true,
        11,
    );
    assert!(!evaluate_event_entry(&e5, &ev_pos, &flags, &inv, &action));
    assert!(evaluate_event_entry(&e5, &ev_pos, &flags, &inv, &use_key));
    assert!(!evaluate_event_entry(&e6, &ev_pos, &flags, &inv, &idle));
    assert!(evaluate_event_entry(&e6, &ev_pos, &flags, &inv, &use_key));

    // 7..=10: carried-item triggers
    let e7 = make_entry(7, 0, EventShapeKind::Point as u32, key_id as u32, true, 12);
    let e8 = make_entry(8, 0, EventShapeKind::Point as u32, key_id as u32, true, 13);
    let e9 = make_entry(
        9,
        10,
        EventShapeKind::Rectangle as u32,
        key_id as u32,
        true,
        14,
    );
    let e10 = make_entry(10, 0, EventShapeKind::Point as u32, key_id as u32, true, 15);
    // Without carrying key, 7/8/9/10 fail on idle/action (10 still succeeds on explicit use_key)
    assert!(!evaluate_event_entry(&e7, &ev_pos, &flags, &inv, &action));
    assert!(!evaluate_event_entry(&e8, &ev_pos, &flags, &inv, &idle));
    assert!(!evaluate_event_entry(&e9, &ev_pos, &flags, &inv, &idle));
    assert!(!evaluate_event_entry(&e10, &ev_pos, &flags, &inv, &action));
    assert!(evaluate_event_entry(&e10, &ev_pos, &flags, &inv, &use_key));

    // With key in inventory, 7/8/9/10 match
    inv.give_raw(key_id, 1);
    assert!(evaluate_event_entry(&e7, &ev_pos, &flags, &inv, &action));
    assert!(evaluate_event_entry(&e8, &ev_pos, &flags, &inv, &idle));
    assert!(evaluate_event_entry(&e9, &ev_pos, &flags, &inv, &idle));
    assert!(evaluate_event_entry(&e10, &ev_pos, &flags, &inv, &action));

    // First-match-wins: entry 0 (PassiveLook) precedes entry 1 (Unconditional0) -> returns Some(0)
    let list = [
        make_entry(3, 0, EventShapeKind::Point as u32, 0, false, 20),
        make_entry(0, 0, 0, 0, false, 21),
        EventListEntry::default(),
    ];
    assert_eq!(
        event_check(&list, &[], &ev_pos, &flags, &inv, &idle),
        Some(0)
    );

    // consume_item applies to triggers 5..=10 in EventEngine (`0x001a8330` / `0x0048e2d8`)
    for trg in 5u32..=10 {
        let mut f = EventFlags::new();
        let mut i = InventoryManager::new();
        i.give_raw(key_id, 1);
        let shape = if matches!(trg, 6 | 9) {
            EventShapeKind::Rectangle as u32
        } else {
            EventShapeKind::Point as u32
        };
        let off = if matches!(trg, 6 | 9) { 10 } else { 0 };
        let entry = make_entry(trg, off, shape, key_id as u32, true, 50);
        let inp = match trg {
            5 | 6 => use_key,
            7 | 10 => action,
            _ => idle,
        };
        let _ = evaluate_retail_stage_events(
            &[entry, EventListEntry::default()],
            &ev_pos,
            41,
            &inp,
            &mut f,
            &mut i,
        );
        assert!(f.get(50), "trg {trg} should execute FlagSet");
        assert!(
            !i.has_item(key_id),
            "trg {trg} with consume_item=1 must consume key {key_id}"
        );
    }
}

#[test]
fn event_result_kind_3_to_6_sentinel_and_dark_room_gating_match_binaries() {
    // EventResultKind 3..=6 (`SLUS_202.28` `0x001a7b50`, `sh2pc.exe` `0x0048e610`)
    assert_eq!(EventResultKind::from_raw(3), Some(EventResultKind::Program));
    assert_eq!(EventResultKind::from_raw(4), Some(EventResultKind::Move));
    assert_eq!(
        EventResultKind::from_raw(5),
        Some(EventResultKind::MovePlusProgram)
    );
    assert_eq!(EventResultKind::from_raw(6), Some(EventResultKind::Message));

    // EventListEntry::is_sentinel() (`cond == 0 && rslt0 == 0` at `0x001a8020` / `0x0048e1a2`)
    let zero_sentinel = EventListEntry {
        flag: 0,
        cond: 0,
        rslt0: 0,
        rslt1: 0,
    };
    assert!(zero_sentinel.is_sentinel());
    let non_sentinel_with_3fff_flag = EventListEntry {
        flag: 0x3fff,
        cond: 0,
        rslt0: 1 << 28,
        rslt1: 5,
    };
    assert!(!non_sentinel_with_3fff_flag.is_sentinel());

    // Dark-room `(cond >> 2) & 1` gating (`SLUS_202.28` `0x001a80ec`)
    let light_gated = EventListEntry {
        flag: 0,
        cond: 1 << 2, // Unconditional0 + requires_light
        rslt0: 1 << 28,
        rslt1: 77,
    };
    assert!(light_gated.requires_light());
    let flags = EventFlags::new();
    let inv = InventoryManager::new();
    let dark_no_flash = EventEvalInput {
        is_dark_room: true,
        flashlight_on: false,
        ..EventEvalInput::default()
    };
    let dark_with_flash = EventEvalInput {
        is_dark_room: true,
        flashlight_on: true,
        ..EventEvalInput::default()
    };
    assert!(!evaluate_event_entry(
        &light_gated,
        &[],
        &flags,
        &inv,
        &dark_no_flash
    ));
    assert!(evaluate_event_entry(
        &light_gated,
        &[],
        &flags,
        &inv,
        &dark_with_flash
    ));
}

#[test]
fn item_list_f16_bitfields_and_room_pickups_extra_condition() {
    let y_f16 = u16::from_le_bytes(float_to_char2(-500.0));
    let rot_f16 = u16::from_le_bytes(float_to_char2(1.5));
    // kind=4 (Health drink), diff=2, extra_cond=2 (requires flag 108), flag_id=300
    let packed = (4u32 << 29) | (2u32 << 26) | (2u32 << 20) | 300u32;
    let spawn = RetailItemSpawn {
        pos_x_bits: 1200.0f32.to_bits(),
        pos_z_bits: (-3400.0f32).to_bits(),
        pos_y_hi16: y_f16,
        rot_y_hi16: rot_f16,
        flags_packed: packed,
    };
    assert!((spawn.pos_y() - char_to_float2(y_f16.to_le_bytes())).abs() < 1e-4);
    assert!((spawn.pos_y() - (-500.0)).abs() < 1.0);
    assert!((spawn.rot_y() - 1.5).abs() < 0.01);
    assert_eq!(spawn.item_kind_index(), 4);
    assert_eq!(spawn.difficulty_condition(), 2);
    assert_eq!(spawn.extra_condition(), 2);
    assert_eq!(spawn.flag_id(), 300);

    // Verify `resolve_active_room_pickups` enforces `extra_condition` (`EventItemConditionCheck` `0x001ab1a0`)
    let pickup = CampaignPickupSpawn {
        position_mm: [100.0, -500.0, 100.0],
        rot_y_raw: rot_f16 as i16,
        item_kind_index: 4,
        difficulty_condition: 0,
        extra_condition: 2, // requires flag(108)
        flag_id: 300,
    };
    let mut flags = EventFlags::new();
    assert!(resolve_active_room_pickups(
        &[pickup],
        &flags,
        ActionDifficulty::Normal,
        false,
        [0.0; 3]
    )
    .is_empty());
    flags.set(108, true);
    let active =
        resolve_active_room_pickups(&[pickup], &flags, ActionDifficulty::Normal, false, [0.0; 3]);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].flag_id, 300);

    // Stage manifest (Act 1 & Act 2) decodes Y via f16, not raw integer or bfloat16
    let m_act2 = resolve_stage_manifest(31, None, false, false);
    for p in &m_act2.room_pickups {
        assert!(p.position_mm[1].is_finite() && p.position_mm[1].abs() < 50_000.0);
    }
}

#[test]
fn clear_data_ending_bitmask_matches_clear_end_kind_and_sh2pc_0048d330() {
    assert_eq!(ClearData::ENDING_LEAVE, ClearEndKind::LEAVE);
    assert_eq!(ClearData::ENDING_MARIA, ClearEndKind::MARIA);
    assert_eq!(ClearData::ENDING_MARIA, 0x02);
    assert_eq!(ClearData::ENDING_IN_WATER, ClearEndKind::IN_WATER);
    assert_eq!(ClearData::ENDING_IN_WATER, 0x04);
    assert_eq!(ClearData::ENDING_REBIRTH, ClearEndKind::REBIRTH);
    assert_eq!(ClearData::ENDING_DOG, ClearEndKind::DOG);
    assert_eq!(ClearData::ENDING_DOG, 0x10);
    assert_eq!(ClearData::ENDING_UFO, ClearEndKind::UFO);
    assert_eq!(ClearData::ENDING_UFO, 0x20);
    assert_eq!(
        ClearData::ENDING_BORN_FROM_A_WISH,
        ClearEndKind::BORN_FROM_A_WISH
    );
}

#[test]
fn event_main_standard_16_iter_chain_message_flag_and_move_plus_program() {
    // 1. Same-frame chaining: 3 FlagSet events chain in a single tick_standard call
    let chain = [
        EventListEntry {
            flag: 10 << 16, // requires !flag(10)
            cond: 0,
            rslt0: 1 << 28, // FlagSet
            rslt1: 10,
        },
        EventListEntry {
            flag: (1 << 31) | (10 << 16) | 11, // requires flag(10) && !flag(11)
            cond: 0,
            rslt0: 1 << 28,
            rslt1: 11,
        },
        EventListEntry {
            flag: (1 << 31) | (11 << 16) | 12, // requires flag(11) && !flag(12)
            cond: 0,
            rslt0: 1 << 28,
            rslt1: 12,
        },
        EventListEntry::default(),
    ];
    let mut engine = EventEngine::new();
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();
    let _ = engine.tick_standard(
        &chain,
        &[],
        &[],
        3,
        &EventEvalInput::default(),
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inv,
    );
    assert!(flags.get(10) && flags.get(11) && flags.get(12));

    // 2. Cap at MAX_EVENT_CHAIN_ITERATIONS (16) prevents infinite loop on cyclic FlagSet/FlagClear
    let cycle = [
        EventListEntry {
            flag: 20 << 16,
            cond: 0,
            rslt0: 1 << 28, // FlagSet 20
            rslt1: 20,
        },
        EventListEntry {
            flag: (1 << 31) | (20 << 16),
            cond: 0,
            rslt0: 2 << 28, // FlagClear 20
            rslt1: 20,
        },
        EventListEntry::default(),
    ];
    assert_eq!(MAX_EVENT_CHAIN_ITERATIONS, 16);
    let _ = engine.tick_standard(
        &cycle,
        &[],
        &[],
        3,
        &EventEvalInput::default(),
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inv,
    );
    // Even number of iterations (16): sets then clears 8 times -> flag 20 ends clear without hanging
    assert!(!flags.get(20));

    // 3. Message (`rslt = 6`) sets `result_flag` (`EventExecMessage` `0x001a9cc0`)
    let msg_ev = [
        EventListEntry {
            flag: 0,
            cond: 0,
            rslt0: 6 << 28,
            rslt1: (12 << 14) | 99,
        },
        EventListEntry::default(),
    ];
    let msg_fx = evaluate_retail_stage_events(
        &msg_ev,
        &[],
        3,
        &EventEvalInput::default(),
        &mut flags,
        &mut inv,
    );
    assert_eq!(msg_fx.stage_message, Some(12));
    assert!(flags.get(99));

    // 4. MovePlusProgram (`rslt = 5`) emits trigger-side open SFX on step 0 and destination-side
    //    close SFX + chained `active_prog_index` & `active_prog_completion_flag` on step 3
    let mut ev_pos = Vec::new();
    // Trigger Point at offset 0: (100, 0, 200)
    push_f32(&mut ev_pos, 100.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 200.0);
    // Destination Point at offset 10: (500, 0, 600)
    push_f32(&mut ev_pos, 500.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 600.0);

    let move_prog = [
        EventListEntry {
            flag: 0,
            cond: (EventShapeKind::Point as u32) << 12,
            rslt0: (5 << 28) | (10 << 16) | ((EventDestShapeKind::Point as u32) << 12),
            rslt1: (1 << 22) | (4 << 14) | 250, // door_se[1], prog #4, completion_flag 250
        },
        EventListEntry::default(),
    ];
    let mut move_eng = EventEngine::new();
    let fx0 = move_eng.tick_standard(
        &move_prog,
        &[],
        &ev_pos,
        7,
        &EventEvalInput::default(),
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inv,
    );
    assert!(fx0.stage_transition.is_some());
    assert_eq!(fx0.positional_sounds.len(), 1);
    assert!((fx0.positional_sounds[0].pos[0] - 100.0).abs() < 1e-3);
    assert!((fx0.positional_sounds[0].pos[1] - (-500.0)).abs() < 1e-3);
    assert!((fx0.positional_sounds[0].pos[2] - 200.0).abs() < 1e-3);
    assert_eq!(
        move_eng.step_state.ev_m_step,
        EventExecMode::MoveThenProgram
    );

    let fx3 = move_eng.tick_standard(
        &move_prog,
        &[],
        &ev_pos,
        7,
        &EventEvalInput::default(),
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inv,
    );
    assert_eq!(fx3.positional_sounds.len(), 1);
    assert!((fx3.positional_sounds[0].pos[0] - 500.0).abs() < 1e-3);
    assert!((fx3.positional_sounds[0].pos[1] - (-500.0)).abs() < 1e-3);
    assert!((fx3.positional_sounds[0].pos[2] - 600.0).abs() < 1e-3);
    assert_eq!(fx3.active_prog_index, Some(4));
    assert_eq!(fx3.active_prog_completion_flag, 250);
    assert_eq!(move_eng.step_state.ev_m_step, EventExecMode::Program);
}

#[test]
fn room208_clock_flag_aligned_to_81_and_campaign_forwards_prog_dir_keys() {
    // 1. Solving Stage 7 prog #6 sets FLAG_APART_CLOCK_SOLVED (81), unlocking PushableObjectKind
    let mut flags = EventFlags::new();
    let mut puzzle = sh2_core::events::PuzzleState::new();
    puzzle.write_u16(PZ_CLOCK, 0x8980); // 9:10 = solved
    let mut inv = InventoryManager::new();
    let mut overlay = sh2_core::cutaway::CutawayOverlayEngine::new();
    let mut runner = StageProgramRunner::new();
    runner.start(7, 6, 0);
    assert!(runner.tick(
        0,
        0,
        1.0 / 60.0,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        sh2_core::config::RiddleDifficulty::Normal,
    ));
    assert!(!runner.tick(
        PROG_KEY_A,
        PROG_KEY_A,
        1.0 / 60.0,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        sh2_core::config::RiddleDifficulty::Normal,
    ));
    assert!(flags.get(FLAG_APART_CLOCK_SOLVED));
    assert!(PushableObjectKind::Apart208GrandfatherClock.can_push(&flags));
    assert!(PushableObjectKind::Apart208GrandfatherClock.trigger_push(&mut flags));
    assert!(flags.get(FLAG_APART_CLOCK_PUSHED));

    // 2. Campaign gameplay forwards PROG_KEY_LEFT / PROG_KEY_RIGHT to StageProgramRunner
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 7;
    sim.scenario.puzzle_state.write_u16(PZ_CLOCK, 0x4000);
    let mut orch = CampaignOrchestrator::from_saved_stage(
        7,
        Some("ap01".to_string()),
        [0.0, 0.0, 0.0],
        0.0,
        OutdoorStreamingMode::default(),
        false,
    );
    orch.phase = CampaignPhase::ActiveGameplay;
    orch.fade = ScreenFadeState::default();
    orch.player_locked = false;

    sim.scenario.prog_runner.start(7, 6, 0);
    // Tick 0 enters step 1 (sets picture -> starts fade-out-to-cutaway; fast-forward fade)
    for _ in 0..130 {
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
    }
    let before_clock = sim.scenario.puzzle_state.read_u16(PZ_CLOCK);
    let right_input = CampaignTickInput {
        prog_dir_down: PROG_KEY_RIGHT,
        prog_dir_held: PROG_KEY_RIGHT,
        ..CampaignTickInput::default()
    };
    for _ in 0..10 {
        let _ = orch.step(&mut sim, &right_input, 1.0 / 60.0);
    }
    let after_right = sim.scenario.puzzle_state.read_u16(PZ_CLOCK);
    assert_ne!(
        after_right, before_clock,
        "PROG_KEY_RIGHT must advance Room 208 clock hands in CampaignOrchestrator"
    );

    let left_input = CampaignTickInput {
        prog_dir_down: PROG_KEY_LEFT,
        prog_dir_held: PROG_KEY_LEFT,
        ..CampaignTickInput::default()
    };
    for _ in 0..10 {
        let _ = orch.step(&mut sim, &left_input, 1.0 / 60.0);
    }
    assert_ne!(
        sim.scenario.puzzle_state.read_u16(PZ_CLOCK),
        after_right,
        "PROG_KEY_LEFT must turn Room 208 clock hands back in CampaignOrchestrator"
    );
}

#[test]
fn cross_stage_move_and_move_plus_program_succeed_with_empty_destination_ev_list() {
    // Regression test for PR #142 Tech Lead finding #2:
    // After a cross-stage Move/MovePlusProgram transition, `ev_work_no` from the old stage
    // may be out of bounds in the destination stage (or `&[]`), yet step 3 must still emit
    // the destination-side close SFX and chain into `EventExecMode::Program` using latched
    // `pending_move_prog_index` / `pending_move_prog_flag`.
    let mut ev_pos = Vec::new();
    push_f32(&mut ev_pos, 100.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 200.0);
    push_f32(&mut ev_pos, 500.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 600.0);

    let old_stage_list = [
        // Dummy non-matching entry at index 0 so the MovePlusProgram sits at index 1 (> empty len 0)
        EventListEntry {
            flag: (1 << 31) | (999 << 16), // requires flag(999) which is clear
            cond: 0,
            rslt0: 1 << 28,
            rslt1: 1,
        },
        EventListEntry {
            flag: 0,
            cond: (EventShapeKind::Point as u32) << 12,
            rslt0: (5 << 28) | (10 << 16) | ((EventDestShapeKind::Point as u32) << 12) | (7 << 5),
            rslt1: (1 << 22) | (4 << 14) | 333, // door_se[1], prog #4, completion_flag 333
        },
        EventListEntry::default(),
    ];

    let mut engine = EventEngine::new();
    let mut flags = EventFlags::new();
    let mut inv = InventoryManager::new();

    // Frame 1 (old stage): step 0 emits transition + trigger-side open SFX.
    let fx0 = engine.tick_standard(
        &old_stage_list,
        &[],
        &ev_pos,
        6,
        &EventEvalInput::default(),
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inv,
    );
    assert!(fx0.stage_transition.is_some());
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::MoveThenProgram);
    assert_eq!(engine.step_state.ev_e_step, 3);
    assert_eq!(engine.step_state.ev_work_no, Some(1));

    // Frame 2 (new stage with EMPTY ev_list `&[]`): step 3 emits close SFX and enters Program.
    let fx3 = engine.tick_standard(
        &[],
        &[],
        &[],
        7,
        &EventEvalInput::default(),
        false,
        false,
        false,
        false,
        &mut flags,
        &mut inv,
    );
    assert_eq!(fx3.positional_sounds.len(), 1);
    assert_eq!(fx3.positional_sounds[0].se_id, 11001); // door_se[1].close
    assert!((fx3.positional_sounds[0].pos[0] - 500.0).abs() < 1e-3);
    assert_eq!(fx3.active_prog_index, Some(4));
    assert_eq!(fx3.active_prog_completion_flag, 333);
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::Program);
    assert_eq!(engine.step_state.ev_work_no, None);

    // Frame 3 (still empty destination ev_list, prog_finished = true): sets completion flag 333!
    let _ = engine.tick_standard(
        &[],
        &[],
        &[],
        7,
        &EventEvalInput::default(),
        false,
        false,
        false,
        true,
        &mut flags,
        &mut inv,
    );
    assert!(flags.get(333));
    assert_eq!(engine.step_state.ev_m_step, EventExecMode::Idle);
}

#[test]
fn campaign_door_transition_preserves_event_engine_close_sfx_and_move_plus_program() {
    // Regression test for PR #142 Tech Lead finding #1:
    // `ConnectStep::StageInit` must NOT reset `EventEngine` during a `DoorTransition`, so upon
    // re-entering `ActiveGameplay` step 3 plays the destination-side close SFX and starts the
    // chained `StageProgramRunner` program.
    use sh2_core::events::engine::StageTransitionCommand;

    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 7;
    let mut orch = CampaignOrchestrator::from_saved_stage(
        7,
        Some("ap01".to_string()),
        [0.0, 0.0, 0.0],
        0.0,
        OutdoorStreamingMode::default(),
        false,
    );
    // Simulate EventEngine having just executed step 0 of a MovePlusProgram door transition.
    orch.event_engine.step_state.ev_m_step = EventExecMode::MoveThenProgram;
    orch.event_engine.step_state.ev_e_step = 3;
    orch.event_engine.step_state.ev_close_se = 11001;
    orch.event_engine.step_state.se_pos = [500.0, -500.0, 600.0];
    orch.event_engine.step_state.pending_move_prog_index = 4;
    orch.event_engine.step_state.pending_move_prog_flag = 333;

    let cmd = StageTransitionCommand {
        dest_stage: 0,
        stage_change_mode: 0,
        is_connected_stage: false,
        observation_mode: false,
        dest_pos: [500.0, 0.0, 600.0],
        dest_rot_y: 0.0,
    };
    orch.phase = CampaignPhase::DoorTransition {
        command: cmd,
        resolved_room: "ap01".to_string(),
        connect_step: sh2_core::campaign::ConnectStep::StageOverlay,
        assets_ready: false,
        fade_out_complete: true,
        loading_screen_active: false,
    };
    orch.fade.set_black_immediate();

    // Advance connectMain through StageOverlay -> StageInit -> ... -> ActiveGameplay
    let mut saw_close_sfx = false;
    for _ in 0..20 {
        let eff = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        if eff.load_room.is_some() {
            orch.notify_room_assets_loaded();
        }
        if eff
            .positional_sounds
            .iter()
            .any(|s| s.se_id == 11001 && (s.pos[0] - 500.0).abs() < 1e-3)
        {
            saw_close_sfx = true;
        }
        if sim.scenario.prog_runner.is_running() {
            break;
        }
    }
    assert!(
        saw_close_sfx,
        "DoorTransition must preserve EventEngine so step 3 emits destination-side close SFX"
    );
    assert!(
        sim.scenario.prog_runner.is_running(),
        "DoorTransition must chain MovePlusProgram into StageProgramRunner"
    );
    let ctx = sim.scenario.prog_runner.context().expect("active prog");
    assert_eq!(ctx.prog_no, 4);
    assert_eq!(ctx.completion_flag, 333);
}

#[test]
fn campaign_dark_stage_gates_light_required_events_until_flashlight_is_on() {
    // Regression test for PR #142 Tech Lead finding #3:
    // In a dark stage (Stage 7 = Wood Side Apartments 2F), `gameplay.rs` must wire
    // `sim.scenario.is_current_stage_dark()` and `sim.scenario.is_flashlight_active()` into
    // `EventEvalInput` so light-gated (`(cond >> 2) & 1`) events are blocked without flashlight
    // and trigger once the flashlight is acquired and turned on.
    use sh2_core::events::stages_act1::act1_stage_descriptor;
    use sh2_core::events::triggers::decode_ev_pos_shape;

    let stage7 = act1_stage_descriptor(7).expect("Stage 7 descriptor");
    // Find a light-gated (`requires_light()`) Action Message (`rslt == 6`, `trg == 2`) entry in Stage 7.
    let target = stage7
        .event_list
        .iter()
        .find(|e| {
            !e.is_sentinel()
                && e.requires_light()
                && e.trigger_kind() == Some(sh2_core::types::EventTriggerKind::Action)
                && e.result_kind() == Some(EventResultKind::Message)
                && e.flag_0() == 0
                && e.flag_1() == 0
                && e.shape_kind() == Some(EventShapeKind::Rectangle)
        })
        .expect("Stage 7 has a light-gated Rectangle Action Message event");
    let shape = decode_ev_pos_shape(
        stage7.ev_pos,
        target.ev_pos_offset(),
        EventShapeKind::Rectangle,
    )
    .expect("valid rectangle shape");
    let (cx, cy, cz) = match shape {
        sh2_core::events::EvPosShape::Rectangle { x, y, z, w, d } => (x + w * 0.5, y, z + d * 0.5),
        _ => unreachable!(),
    };

    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 7;
    assert!(sim.scenario.is_current_stage_dark());
    sim.snap_player_transform(glam::Vec3::new(cx, cy, cz), 0.0);

    let mut orch = CampaignOrchestrator::from_saved_stage(
        7,
        Some("ap01".to_string()),
        [cx, cy, cz],
        0.0,
        OutdoorStreamingMode::default(),
        false,
    );
    orch.phase = CampaignPhase::ActiveGameplay;
    orch.fade = ScreenFadeState::default();
    orch.player_locked = false;

    let action = CampaignTickInput {
        action_pressed: true,
        ..CampaignTickInput::default()
    };

    // 1. Without flashlight in dark Stage 7: light-gated event MUST NOT fire.
    assert!(!sim.scenario.is_flashlight_active());
    let eff_dark = orch.step(&mut sim, &action, 1.0 / 60.0);
    assert!(
        eff_dark.stage_message.is_none(),
        "Light-gated event must not fire in dark Stage 7 without flashlight"
    );

    // 2. Grant James the Flashlight (`item 0x12`, `acquire_flashlight(true)`): event MUST fire.
    sim.scenario.acquire_flashlight(true);
    assert!(sim.scenario.is_flashlight_active());
    let eff_lit = orch.step(&mut sim, &action, 1.0 / 60.0);
    assert_eq!(
        eff_lit.stage_message,
        Some(u16::from(target.message_or_save_id())),
        "Light-gated event must fire in dark Stage 7 once flashlight is active"
    );
}
