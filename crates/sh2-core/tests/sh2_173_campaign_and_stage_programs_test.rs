//! SH2-173 Parity & Integration Test Suite:
//! - `connectStageInit` (`0x0020bd60`) & `connectRoomInit` (`0x0020be10`) hooks
//! - `run_prologue` NG+ `ClearData`, `effective_riddle` propagation, RNG seed, & `BornFromAWish` flags 5/24
//! - Live `EndingPointTracker` (`EvProgEndingJudge` `0x00576490`) campaign wiring
//! - `MultiFloorElevatorPanel` & `PushableObjectKind` runtime wiring
//! - Full 58-stage `ev_prog` VA parity & coroutine execution across all 8 modular act/region files.

use sh2_core::campaign::stage_hooks::{eval_room_init_hook, eval_stage_init_hook};
use sh2_core::campaign::{CampaignOrchestrator, CampaignPhase, CampaignTickInput};
use sh2_core::config::{ActionDifficulty, OutdoorStreamingMode, RiddleDifficulty};
use sh2_core::cutaway::CutawayOverlayEngine;
use sh2_core::endings::{
    EndingPointTracker, MainEndingKind, PlayingInfo, FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE,
    REBIRTH_RITUAL_ITEM_IDS,
};
use sh2_core::events::horror_triggers::StageEventDispatcher;
use sh2_core::events::stage_programs::endings::DOG_ENDING_DEMO_NO;
use sh2_core::events::stage_programs::find_static_prog_spec;
use sh2_core::events::{
    ClearData, EventFlags, PuzzleState, Sh2Rng, StageProgramRunner, PROG_KEY_A, PROG_KEY_DOWN,
};
use sh2_core::inventory::InventoryManager;
use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;
use sh2_core::world::{
    ElevatorKind, FLAG_APART_CLOCK_PUSHED, FLAG_APART_CLOCK_SOLVED, FLAG_HOSPITAL_B1_SHELF_PUSHED,
};

#[test]
fn connect_stage_and_room_init_hooks_match_sh2pc_disassembly() {
    let mut flags = EventFlags::new();
    let mut horror = StageEventDispatcher::new();
    let mut rng = Sh2Rng::new(0x1234_5678);

    // Stage 5 TownEast (`0x0059dfc0`): 1534 -> 1497, 1533 -> 1496, 1494 -> 1526
    flags.set(1534, true);
    flags.set(1533, true);
    flags.set(1494, true);
    eval_stage_init_hook(5, &mut flags, &mut horror);
    assert!(flags.get(1497));
    assert!(flags.get(1496));
    assert!(flags.get(1526));

    // Stage 15 Bowling (`0x00591bd0` stage_init clears flag 15; `0x00591c00` room_init sets 530 outside 0x09)
    flags.set(15, true);
    eval_stage_init_hook(15, &mut flags, &mut horror);
    assert!(!flags.get(15));
    eval_room_init_hook(15, 0x0b, &mut flags, &mut horror, &mut rng);
    assert!(flags.get(530));

    // Stage 16 ToHeaven (`0x00591170`): flag 163 -> sets flag 164
    flags.set(163, true);
    eval_room_init_hook(16, 0x01, &mut flags, &mut horror, &mut rng);
    assert!(flags.get(164));

    // Stage 20 Hospital 3F Normal (`0x0058edc0`): Room 0x41 clears flag 15, sets 188, and 186 -> 629
    flags.set(15, true);
    flags.set(186, true);
    eval_room_init_hook(20, 0x41, &mut flags, &mut horror, &mut rng);
    assert!(!flags.get(15));
    assert!(flags.get(188));
    assert!(flags.get(629));

    // Stage 34 Labyrinth W (`0x00582a00`): Room 0x89 with flag 327 && !368 -> sets 328
    flags.set(327, true);
    eval_room_init_hook(34, 0x89, &mut flags, &mut horror, &mut rng);
    assert!(flags.get(328));

    // Stage 40 Hotel 2F (`0x0057a170`): sets flag 405
    eval_stage_init_hook(40, &mut flags, &mut horror);
    assert!(flags.get(405));

    // Stage 41 Hotel 3F (`0x00578730`): Room 0xa2 clears flag 527
    flags.set(527, true);
    eval_room_init_hook(41, 0xa2, &mut flags, &mut horror, &mut rng);
    assert!(!flags.get(527));

    // Stage 49 EndMaria (`0x00440ee0`): sets flags 45 & 168
    eval_stage_init_hook(49, &mut flags, &mut horror);
    assert!(flags.get(45));
    assert!(flags.get(168));

    // Stage 56 Wish 2F (`0x00571860`): flag 711 -> clears flag 710
    flags.set(710, true);
    flags.set(711, true);
    eval_room_init_hook(56, 0xcd, &mut flags, &mut horror, &mut rng);
    assert!(!flags.get(710));
}

#[test]
fn run_prologue_propagates_clear_data_extra_riddle_seed_and_bfw_flags() {
    // 1. NG+ Extra Riddle promotion & Chainsaw flag 12 (`0x0048aa78`, `0x0048ab61`)
    let clear = ClearData {
        endings_seen: ClearData::ENDING_LEAVE,
        cleared_levels: ClearData::ALL_BASE_RIDDLES_CLEARED | ClearData::ACTION_NORMAL_CLEARED,
        clear_count: 3,
    };
    let mut sim_a = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let mut orch_a = CampaignOrchestrator::new_game_with_clear_data(
        ScenarioKind::LetterFromSilentHeaven,
        ActionDifficulty::Normal,
        RiddleDifficulty::Hard,
        clear,
        0x1111_2222,
        OutdoorStreamingMode::default(),
        false,
    );
    let input = CampaignTickInput::default();
    let _ = orch_a.step(&mut sim_a, &input, 1.0 / 60.0);
    assert_eq!(sim_a.scenario.riddle_difficulty, RiddleDifficulty::Extra);
    assert!(sim_a.scenario.flags.get(10));
    assert!(sim_a.scenario.flags.get(12)); // Chainsaw unlocked on NG+

    // 2. Different RNG seed produces distinct randomized puzzle state
    let mut sim_b = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let mut orch_b = CampaignOrchestrator::new_game_with_clear_data(
        ScenarioKind::LetterFromSilentHeaven,
        ActionDifficulty::Normal,
        RiddleDifficulty::Normal,
        ClearData::default(),
        0x9999_8888,
        OutdoorStreamingMode::default(),
        false,
    );
    let _ = orch_b.step(&mut sim_b, &input, 1.0 / 60.0);
    assert_ne!(
        sim_a.scenario.puzzle_state.raw(),
        sim_b.scenario.puzzle_state.raw()
    );

    // 3. Born From a Wish initializes flags 5 & 24 (`sh2pc.exe` `0x0048aa3e`: `0x1000020`)
    let mut sim_wish = SimulationCore::new(ScenarioKind::BornFromAWish);
    let mut orch_wish = CampaignOrchestrator::new_game_with_clear_data(
        ScenarioKind::BornFromAWish,
        ActionDifficulty::Normal,
        RiddleDifficulty::Normal,
        ClearData::default(),
        0x4444_5555,
        OutdoorStreamingMode::default(),
        false,
    );
    let _ = orch_wish.step(&mut sim_wish, &input, 1.0 / 60.0);
    assert!(sim_wish.scenario.flags.get(5));
    assert!(sim_wish.scenario.flags.get(24));
    assert!(matches!(orch_wish.phase, CampaignPhase::RoomLoading { .. }));
}

#[test]
fn ending_point_tracker_and_stage44_ending_judge_wired_end_to_end() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let mut orch = CampaignOrchestrator::new_game(
        ScenarioKind::LetterFromSilentHeaven,
        ActionDifficulty::Normal,
        RiddleDifficulty::Normal,
        OutdoorStreamingMode::default(),
        false,
    );
    orch.phase = CampaignPhase::ActiveGameplay;
    sim.scenario.health = 20.0; // Low HP (< 50%)
    sim.scenario.flags.set(15, true); // Maria accompanying
    sim.scenario.current_room = "ru69".to_string(); // Room 0xba corridor monologue (`R_HTL_ALT_LONG_HALLWAY`)

    let input = CampaignTickInput {
        maria_distance_mm: Some(500.0), // < 1000 mm -> close to Maria
        maria_bumped: true,
        ..CampaignTickInput::default()
    };
    let _ = orch.step(&mut sim, &input, 1.0);
    assert!(sim.scenario.ending_tracker.low_hp_weighted_secs > 0.0);
    assert!(sim.scenario.ending_tracker.maria_close_secs >= 1.0);
    assert_eq!(sim.scenario.ending_tracker.maria_bump_count, 1);

    // Tick Mary's corridor monologue in Room 0xba to completion -> sets Flag 624
    for _ in 0..200 {
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0);
    }
    assert!(sim.scenario.flags.get(FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE));

    // Evaluate Stage 44 #6 (`EvProgEndingJudge`, `0x00576490`) with all 4 Rebirth items -> Rebirth (`0x08`)
    let mut inv = InventoryManager::new();
    for id in REBIRTH_RITUAL_ITEM_IDS {
        inv.item_get_now(id);
    }
    let mut runner = StageProgramRunner::new();
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut overlay = CutawayOverlayEngine::new();
    let info = PlayingInfo::default();
    let tracker = EndingPointTracker::default();

    runner.start(44, 6, 0);
    let still_running = runner.tick_with_campaign(
        0,
        0,
        1.0 / 60.0,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
        Some(&info),
        Some(&tracker),
        None,
    );
    assert!(!still_running);
    assert_eq!(
        runner.evaluated_main_ending(),
        Some(MainEndingKind::Rebirth)
    );
    assert_eq!(runner.take_triggered_ending(), Some(0x08));
}

#[test]
fn pushables_and_multi_floor_elevator_panel_work_through_stage_runner() {
    let mut runner = StageProgramRunner::new();
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut overlay = CutawayOverlayEngine::new();
    let mut elevator = None;

    // 1. Stage 7 #7 Grandfather Clock push requires flag 81 (`FLAG_APART_CLOCK_SOLVED`) -> sets flag 82
    runner.start(7, 7, 0);
    runner.tick(
        0,
        0,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
    );
    assert!(!flags.get(FLAG_APART_CLOCK_PUSHED));

    flags.set(FLAG_APART_CLOCK_SOLVED, true);
    runner.start(7, 7, 0);
    runner.tick(
        0,
        0,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
    );
    assert!(flags.get(FLAG_APART_CLOCK_PUSHED));

    // 2. Stage 24 #2 Hospital B1 Storeroom Shelf push sets flag 223 (`FLAG_HOSPITAL_B1_SHELF_PUSHED`)
    runner.start(24, 2, 0);
    runner.tick(
        0,
        0,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
    );
    assert!(flags.get(FLAG_HOSPITAL_B1_SHELF_PUSHED));

    // 3. Stage 18 #2 Brookhaven Hospital Normal Elevator panel opens, moves cursor, and closes on Action
    runner.start(18, 2, 0);
    let running = runner.tick_with_campaign(
        0,
        0,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
        None,
        None,
        Some(&mut elevator),
    );
    assert!(running);
    assert_eq!(
        elevator.as_ref().map(|e| e.kind),
        Some(ElevatorKind::HospitalNormal)
    );
    let running = runner.tick_with_campaign(
        PROG_KEY_DOWN,
        PROG_KEY_DOWN,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
        None,
        None,
        Some(&mut elevator),
    );
    assert!(running);
    assert_eq!(elevator.as_ref().map(|e| e.cursor_index), Some(1));
    let running = runner.tick_with_campaign(
        PROG_KEY_A,
        PROG_KEY_A,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
        None,
        None,
        Some(&mut elevator),
    );
    assert!(!running);
    assert!(elevator.is_none());
}

#[test]
fn all_58_stages_ev_prog_vas_have_1to1_ported_handlers_and_dog_ending_works() {
    let mut covered_count = 0usize;
    for stage in 1u8..=58 {
        for prog_no in 1u8..=30 {
            if let Some(expected_va) = StageProgramRunner::resolve_ev_prog_va(stage, prog_no) {
                if stage == 3 {
                    assert!((1..=6).contains(&prog_no));
                } else {
                    let spec = find_static_prog_spec(stage, prog_no).unwrap_or_else(|| {
                        panic!(
                            "Missing StaticProgSpec for Stage {stage} prog #{prog_no} (VA {expected_va:#010x})"
                        )
                    });
                    assert_eq!(
                        spec.va, expected_va,
                        "VA mismatch for Stage {stage} prog #{prog_no}"
                    );
                }
                covered_count += 1;
            }
        }
    }
    assert_eq!(
        covered_count, 347,
        "Expected all 347 unique non-null ev_prog VAs across 58 stages"
    );

    // Verify Stage 52 #1 Dog Ending lever coroutine (`0x00574430`) emits demo 98 (`0x62`) + bit `0x10`
    let mut runner = StageProgramRunner::new();
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut overlay = CutawayOverlayEngine::new();
    runner.start(52, 1, 0);
    let running = runner.tick(
        0,
        0,
        0.016,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal,
    );
    assert!(!running);
    assert_eq!(runner.take_requested_demo(), Some(DOG_ENDING_DEMO_NO));
    assert_eq!(runner.take_triggered_ending(), Some(0x10));
}
