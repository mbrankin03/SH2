//! SH2-173 Parity & Integration Test Suite:
//! - `connectStageInit` (`0x0020bd60`) & `connectRoomInit` (`0x0020be10`) hooks
//! - `run_prologue` NG+ `ClearData`, `effective_riddle` propagation, RNG seed, & `BornFromAWish` flags 5/24
//! - Live `EndingPointTracker` (`EvProgEndingJudge` `0x00576490`) campaign wiring
//! - `MultiFloorElevatorPanel` & `PushableObjectKind` runtime wiring
//! - Full 58-stage `ev_prog` VA parity & coroutine execution across all 8 modular act/region files.

use sh2_core::campaign::room_name::{room_name, room_name_for_stage_pos};
use sh2_core::campaign::stage_hooks::{eval_room_init_hook, eval_stage_init_hook};
use sh2_core::campaign::{
    CampaignOrchestrator, CampaignPhase, CampaignTickInput, ConnectStep, RoomLoadTarget,
};
use sh2_core::config::{ActionDifficulty, OutdoorStreamingMode, RiddleDifficulty};
use sh2_core::cutaway::CutawayOverlayEngine;
use sh2_core::endings::{
    EndingPointTracker, MainEndingKind, PlayingInfo, FLAG_ENDING_IN_WATER, FLAG_ENDING_LEAVE,
    FLAG_ENDING_MARIA, FLAG_ENDING_REBIRTH, FLAG_HOSPITAL_ROOF_DIARY,
    FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK, FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE,
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

    // Verify canonical `RoomName` (`Event/stg_name.c` `0x001af0f0`) resolves exact numeric room IDs
    // from world coordinates across all key `connectRoomInit` stages:
    // - Stage 2 Restroom (`glb_crd = 6`, cell `0x1d0` -> `x=-40000, z=0` -> `0x01`)
    assert_eq!(room_name_for_stage_pos(2, [-40_000.0, 0.0, 0.0]), 0x01);
    // - Stage 15 Bowling (`glb_crd = 7`, cell `0x20e` -> `0x09`, cell `0x210` -> `0x0b`)
    assert_eq!(room_name_for_stage_pos(15, [0.0, 0.0, -40_000.0]), 0x09);
    assert_eq!(room_name_for_stage_pos(15, [0.0, 0.0, 0.0]), 0x0b);
    // - Stage 20 Hospital 3F (`glb_crd = 10`, cell `0x292` -> `0x41`)
    assert_eq!(room_name(80_000.0, 40_000.0, 10), 0x41);
    // - Stage 34 Labyrinth W (`glb_crd = 11`, cell `0x210` -> `0x89`)
    assert_eq!(room_name_for_stage_pos(34, [0.0, 0.0, 0.0]), 0x89);
    // - Stage 44 Hotel 1F B (`glb_crd = 13`, cell `0x250` -> `0xba`, `0x150` -> `0xae`, `0x190` -> `0xad`)
    assert_eq!(room_name_for_stage_pos(44, [40_000.0, 0.0, 0.0]), 0xba);
    assert_eq!(room_name_for_stage_pos(44, [-120_000.0, 0.0, 0.0]), 0xae);
    assert_eq!(room_name_for_stage_pos(44, [-80_000.0, 0.0, 0.0]), 0xad);
    // - Stage 56 Wish 2F (`glb_crd = 16`, cell `0x212` -> `0xcd`)
    assert_eq!(room_name_for_stage_pos(56, [0.0, 0.0, 40_000.0]), 0xcd);

    // Live `connectRoomInit` (`ConnectStep::RoomInit`) numeric room-ID resolution through
    // `CampaignOrchestrator::step`:
    // 1. Stage 15 Bowling at Room `0x09` (`[0, 0, -40_000]`) clears flag 15 (`connectStageInit`)
    //    and does NOT set flag 530 (`room_id == 0x09` guard in `0x00591c00`).
    let run_room_loading = |stage: u16, pos: [f32; 3], pre_flags: &[usize]| -> EventFlags {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        for &f in pre_flags {
            sim.scenario.flags.set(f, true);
        }
        let mut orch = CampaignOrchestrator::new_game(
            ScenarioKind::LetterFromSilentHeaven,
            ActionDifficulty::Normal,
            RiddleDifficulty::Normal,
            OutdoorStreamingMode::default(),
            false,
        );
        orch.phase = CampaignPhase::RoomLoading {
            target: RoomLoadTarget {
                stage_id: stage,
                room_override: None,
                spawn_pos: pos,
                spawn_yaw: 0.0,
                is_connected_stage: false,
                show_loading_screen: false,
            },
            connect_step: ConnectStep::StageOverlay,
            assets_ready: true,
        };
        let input = CampaignTickInput::default();
        for _ in 0..8 {
            let _ = orch.step(&mut sim, &input, 1.0 / 60.0);
        }
        sim.scenario.flags
    };
    let bowling_09 = run_room_loading(15, [0.0, 0.0, -40_000.0], &[15]);
    assert!(!bowling_09.get(15));
    assert!(!bowling_09.get(530));
    // 2. Stage 15 Bowling at Room `0x0b` (`[0, 0, 0]`) sets flag 530.
    let bowling_0b = run_room_loading(15, [0.0, 0.0, 0.0], &[15]);
    assert!(!bowling_0b.get(15));
    assert!(bowling_0b.get(530));
    // 3. Stage 34 Labyrinth W at Room `0x89` (`[0, 0, 0]`) with flag 327 sets flag 328.
    let lab_89 = run_room_loading(34, [0.0, 0.0, 0.0], &[327]);
    assert!(lab_89.get(328));
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
    sim.scenario.current_stage = 44; // Hotel 1F Otherworld (`glb_crd = 13`)
    sim.scenario.player_position = glam::Vec3::new(40_000.0, 0.0, 0.0); // Room `0xba` (`RoomNameHotelBack`)
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
    // `EvProgEndingJudge` (`0x00576660..0x0057672a`) sets `FLAG_ENDING_REBIRTH` (`621`)
    // and does NOT immediately trigger the ending (final boss & ending stage follow).
    assert!(flags.get(FLAG_ENDING_REBIRTH));
    assert_eq!(runner.take_triggered_ending(), None);

    // Verify all 3 non-Rebirth `EvProgEndingJudge` flag outcomes (`618` Leave, `619` Maria, `620` In Water):
    let run_ending_judge =
        |pre_flags: &[usize], info_override: PlayingInfo, tracker_override: EndingPointTracker| {
            let mut r = StageProgramRunner::new();
            let mut f = EventFlags::new();
            for &bit in pre_flags {
                f.set(bit, true);
            }
            let mut p = PuzzleState::new();
            let mut i = InventoryManager::new();
            let mut o = CutawayOverlayEngine::new();
            r.start(44, 6, 0);
            let running = r.tick_with_campaign(
                0,
                0,
                1.0 / 60.0,
                &mut f,
                &mut p,
                &mut i,
                &mut o,
                RiddleDifficulty::Normal,
                Some(&info_override),
                Some(&tracker_override),
                None,
            );
            assert!(!running);
            assert_eq!(r.take_triggered_ending(), None);
            (r.evaluated_main_ending(), f)
        };

    // Leave (`FLAG_ENDING_LEAVE` = `618`)
    let (leave_kind, leave_flags) = run_ending_judge(
        &[FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE],
        PlayingInfo::default(),
        EndingPointTracker {
            mary_item_examine_count: 10,
            ..EndingPointTracker::default()
        },
    );
    assert_eq!(leave_kind, Some(MainEndingKind::Leave));
    assert!(leave_flags.get(FLAG_ENDING_LEAVE));
    assert!(!leave_flags.get(FLAG_ENDING_MARIA));
    assert!(!leave_flags.get(FLAG_ENDING_IN_WATER));
    assert!(!leave_flags.get(FLAG_ENDING_REBIRTH));

    // Maria (`FLAG_ENDING_MARIA` = `619`)
    let (maria_kind, maria_flags) = run_ending_judge(
        &[FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK],
        PlayingInfo::default(),
        EndingPointTracker {
            maria_close_secs: 2000.0,
            ..EndingPointTracker::default()
        },
    );
    assert_eq!(maria_kind, Some(MainEndingKind::Maria));
    assert!(maria_flags.get(FLAG_ENDING_MARIA));
    assert!(!maria_flags.get(FLAG_ENDING_LEAVE));

    // In Water (`FLAG_ENDING_IN_WATER` = `620`)
    let (water_kind, water_flags) = run_ending_judge(
        &[FLAG_HOSPITAL_ROOF_DIARY],
        PlayingInfo::default(),
        EndingPointTracker {
            angelas_knife_examined: true,
            low_hp_weighted_secs: 600.0,
            ..EndingPointTracker::default()
        },
    );
    assert_eq!(water_kind, Some(MainEndingKind::InWater));
    assert!(water_flags.get(FLAG_ENDING_IN_WATER));
    assert!(!water_flags.get(FLAG_ENDING_LEAVE));
}

#[test]
fn pushables_and_multi_floor_elevator_panel_work_through_stage_runner() {
    let mut runner = StageProgramRunner::new();
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut overlay = CutawayOverlayEngine::new();
    let mut elevator = None;

    // 1. Stage 7 #7 Grandfather Clock push (`0x0059a840`) requires flag 81 -> sets flag 82 & SE `0x4a51`
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
    assert_eq!(runner.sound_effects(), &[0x4a51]);

    // 2. Stage 26 #2 Hospital B1 Storeroom Shelf push (`0x00588220` / `0x00588370`) sets flag 223,
    //    while Stage 24 #2 (`0x0058aa50`) does NOT set flag 223.
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
    assert!(!flags.get(FLAG_HOSPITAL_B1_SHELF_PUSHED));

    runner.start(26, 2, 0);
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

    // 3. Stage 20 #9 Brookhaven Hospital Normal Elevator panel (`0x0058ea25` -> `FUN_004a2a50`)
    runner.start(20, 9, 0);
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

    // 4. All verified `sh2pc.exe` `FUN_004a2a50` MultiFloorElevatorPanel `(stage, prog_no)` entries
    for &(stage, prog_no, expected_kind) in &[
        (20u8, 9u8, ElevatorKind::HospitalNormal),
        (20, 11, ElevatorKind::HospitalNormal),
        (21, 1, ElevatorKind::HospitalNormal),
        (25, 3, ElevatorKind::HospitalOtherworld),
        (26, 5, ElevatorKind::HospitalOtherworld),
        (41, 4, ElevatorKind::HotelEmployee),
        (41, 10, ElevatorKind::HotelEmployee),
        (43, 1, ElevatorKind::HotelEmployee),
    ] {
        elevator = None;
        runner.start(stage, prog_no, 0);
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
        assert!(
            running,
            "Stage {stage} #{prog_no} should keep running while elevator panel is open"
        );
        assert_eq!(
            elevator.as_ref().map(|e| e.kind),
            Some(expected_kind),
            "Stage {stage} #{prog_no} expected {expected_kind:?}"
        );
    }

    // Non-elevator programs (previously mis-wired) must NOT open `MultiFloorElevatorPanel`
    for &(stage, prog_no) in &[(18u8, 2u8), (22, 2), (40, 3)] {
        elevator = None;
        runner.start(stage, prog_no, 0);
        let _ = runner.tick_with_campaign(
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
        assert!(
            elevator.is_none(),
            "Stage {stage} #{prog_no} must NOT open MultiFloorElevatorPanel"
        );
    }
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

    // Verify Stage 48 #1 plays pre-boss cutscene demo 88 WITHOUT prematurely triggering ending
    let mut runner = StageProgramRunner::new();
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut overlay = CutawayOverlayEngine::new();
    runner.start(48, 1, 0);
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
    assert_eq!(runner.take_requested_demo(), Some(88));
    assert_eq!(runner.take_triggered_ending(), None);

    // Verify Stage 50 #4 (`0x0043ed20` / `0x0043f244`) emits demo 95, SE `0xeab2`, and bit `0x04`
    runner.start(50, 4, 0);
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
    assert_eq!(runner.take_requested_demo(), Some(95));
    assert_eq!(runner.sound_effects(), &[0xeab2]);
    assert_eq!(runner.take_triggered_ending(), Some(0x04));

    // Verify Stage 52 #1 Dog Ending lever coroutine (`0x00574430`) emits demo 98 (`0x62`) + bit `0x10`
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
