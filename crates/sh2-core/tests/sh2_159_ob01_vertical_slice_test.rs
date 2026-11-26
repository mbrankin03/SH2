//! End-to-end `er01 ↔ ob01 → Forest Trail (ca11 / Stage 4)` vertical-slice integration test (`SH2-159`).
//!
//! Verifies 1:1 retail data and runtime behavior:
//! - Stage/room transition trigger & destination coordinates decoded from `sh2pc.exe` retail tables:
//!   - `stg_toilet.c` (`ACT1_STAGE_TOILET` `0x008e7e80`, `Event_List` `0x008e7ce8`, `ev_pos` `0x008e7c20`)
//!   - `stg_ovservation.c` (`ACT1_STAGE_OBSERVATION` `0x008e7a48`, `Event_List` `0x008e7948`, `ev_pos` `0x008e78c0`)
//!   - `stg_forest.c` (`ACT1_STAGE_FOREST` `0x008e7538`, `Event_List` `0x008e6ff8`, `ev_pos` `0x008e6e88`)
//! - `ob01.cam` & `ob01.cld` retail asset loading via [`SimulationCore`].
//! - `stg_ovservation.c` `EvRoomInit` (`0x005a1350`) `.map` group `0x03` override (`flag(24)` gating)
//!   and `DisplayControlList` (`EvDispControlModelEntry` `0x004a2f10`) -> `MapPartVisibilityFilter`.

use sh2_assets::formats::cld::parse_cld;
use sh2_assets::AssetPathResolver;
use sh2_core::campaign::retail_events::decode_retail_move_destination;
use sh2_core::events::engine::StageTransitionCommand;
use sh2_core::events::stageparts::DisplayControlList;
use sh2_core::events::stages_act1::{
    act1_eval_alltime_and_room, act1_stage_descriptor, Act1StageId, Act1VanillaFixesConfig,
};
use sh2_core::events::stages_act2::normalize_retail_event_entry_for_input;
use sh2_core::events::triggers::EventEvalInput;
use sh2_core::events::{EventFlags, PuzzleState};
use sh2_core::inventory::InventoryManager;
use sh2_core::room_loader::stage_room_descriptor;
use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;
use sh2_core::types::{EventDestShapeKind, EventListEntry, EventResultKind};

fn normalized_moves_to_stage(
    entries: &[EventListEntry],
    dest_stage: Act1StageId,
) -> Vec<EventListEntry> {
    let input = EventEvalInput::default();
    let inv = InventoryManager::new();
    entries
        .iter()
        .copied()
        .take_while(|e| !e.is_sentinel())
        .map(|e| normalize_retail_event_entry_for_input(e, &input, &inv))
        .filter(|e| {
            matches!(
                e.result_kind(),
                Some(EventResultKind::Move | EventResultKind::MovePlusProgram)
            ) && e.stage_change_mode() == dest_stage as u8
        })
        .collect()
}

#[test]
fn er01_ob01_forest_retail_stage_transitions_decode_valid_coordinates() {
    let stage_toilet = act1_stage_descriptor(Act1StageId::Toilet as u8).expect("stage 2");
    let stage_observation = act1_stage_descriptor(Act1StageId::Observation as u8).expect("stage 3");
    let stage_forest = act1_stage_descriptor(Act1StageId::Forest as u8).expect("stage 4");

    // 1. Stage 2 (`er01` Toilet) -> Stage 3 (`ob01` Observation Deck) door event (`0x008e7ce8`)
    let toilet_to_ob = normalized_moves_to_stage(stage_toilet.event_list, Act1StageId::Observation);
    assert!(
        !toilet_to_ob.is_empty(),
        "ACT1_STAGE_TOILET must have a stage-change transition into Stage 3 (Observation)"
    );
    let dest_ob = decode_retail_move_destination(
        stage_toilet.ev_pos,
        toilet_to_ob[0].dest_ev_pos_offset(),
        toilet_to_ob[0]
            .dest_shape_kind()
            .unwrap_or(EventDestShapeKind::Point),
    )
    .expect("decode er01 -> ob01 destination pose");
    assert!(
        dest_ob.0.iter().all(|v| v.is_finite()),
        "er01 -> ob01 destination must be finite: {:?}",
        dest_ob.0
    );

    // 2. Stage 3 (`ob01` Observation Deck) -> Stage 2 (`er01` Toilet) AND -> Stage 4 (`ca11` Forest Trail)
    let ob_to_er = normalized_moves_to_stage(stage_observation.event_list, Act1StageId::Toilet);
    assert!(
        !ob_to_er.is_empty(),
        "ACT1_STAGE_OBSERVATION must have a return transition to Stage 2 (Toilet / er01)"
    );
    let dest_er = decode_retail_move_destination(
        stage_observation.ev_pos,
        ob_to_er[0].dest_ev_pos_offset(),
        ob_to_er[0]
            .dest_shape_kind()
            .unwrap_or(EventDestShapeKind::Point),
    )
    .expect("decode ob01 -> er01 destination pose");
    assert!(dest_er.0.iter().all(|v| v.is_finite()));

    let ob_to_forest = normalized_moves_to_stage(stage_observation.event_list, Act1StageId::Forest);
    assert!(
        !ob_to_forest.is_empty(),
        "ACT1_STAGE_OBSERVATION must have a transition to Stage 4 (Forest Trail)"
    );
    let dest_forest = decode_retail_move_destination(
        stage_observation.ev_pos,
        ob_to_forest[0].dest_ev_pos_offset(),
        ob_to_forest[0]
            .dest_shape_kind()
            .unwrap_or(EventDestShapeKind::Point),
    )
    .expect("decode ob01 -> Forest Trail destination pose");
    assert!(dest_forest.0.iter().all(|v| v.is_finite()));

    // 3. Stage 4 (`stg_forest.c`) return transition back to Stage 3 (`ob01`)
    let forest_to_ob = normalized_moves_to_stage(stage_forest.event_list, Act1StageId::Observation);
    assert!(
        !forest_to_ob.is_empty(),
        "ACT1_STAGE_FOREST must have a return transition to Stage 3 (Observation)"
    );

    // Verify canonical stage room descriptors for Stages 2, 3, 4.
    assert_eq!(stage_room_descriptor(2).unwrap().default_room_id, "er01");
    assert_eq!(stage_room_descriptor(3).unwrap().default_room_id, "ob01");
    assert_eq!(stage_room_descriptor(4).unwrap().default_room_id, "ca11");
}

#[test]
fn ob01_ev_room_init_group_3_override_matches_flag_24() {
    let mut flags = EventFlags::new();
    let puzzle = PuzzleState::new();

    // Before picking up Map of Silent Hill (`flag(24) = false`): `(0x03, 0)` -> visible.
    let snap_visible = act1_eval_alltime_and_room(
        Act1StageId::Observation,
        1,
        [15_600.0, 0.0, -4_140.0],
        0,
        false,
        false,
        Act1VanillaFixesConfig::default(),
        &mut flags,
        &puzzle,
    );
    assert_eq!(snap_visible.door_overrides, vec![(0x03, 0)]);

    let mut list_visible = DisplayControlList::new();
    for &(group, state) in &snap_visible.door_overrides {
        list_visible.entry(
            Act1StageId::Observation as u16,
            u16::from(group),
            i32::from(state),
        );
    }
    let filter_visible = list_visible.exec();
    assert!(filter_visible.is_batch_visible(1, 2, 1));
    assert!(filter_visible.is_batch_visible(1, 0, 0));

    // After picking up Map of Silent Hill (`flag(24) = true`): `(0x03, -1)` -> hidden.
    flags.set(24, true);
    let snap_hidden = act1_eval_alltime_and_room(
        Act1StageId::Observation,
        1,
        [15_600.0, 0.0, -4_140.0],
        0,
        false,
        false,
        Act1VanillaFixesConfig::default(),
        &mut flags,
        &puzzle,
    );
    assert_eq!(snap_hidden.door_overrides, vec![(0x03, -1)]);

    let mut list_hidden = DisplayControlList::new();
    for &(group, state) in &snap_hidden.door_overrides {
        list_hidden.entry(
            Act1StageId::Observation as u16,
            u16::from(group),
            i32::from(state),
        );
    }
    let filter_hidden = list_hidden.exec();
    assert!(!filter_hidden.is_batch_visible(1, 2, 1));
    assert!(filter_hidden.is_batch_visible(1, 0, 0));
}

#[test]
fn simulation_core_transitions_er01_ob01_forest_and_loads_ob01_cam_and_cld() {
    let resolver = AssetPathResolver::resolve();
    let ob01_cam = resolver.data_dir.join("bg/ob/ob01.cam");
    let ob01_cld = resolver.data_dir.join("bg/ob/ob01.cld");
    if !ob01_cam.exists() || !ob01_cld.exists() {
        return;
    }

    // Parse retail `ob01.cld` and confirm walkable floor + wall geometry exists.
    let cld_bytes = std::fs::read(&ob01_cld).expect("read ob01.cld");
    let cld = parse_cld(&cld_bytes).expect("parse ob01.cld");
    assert!(!cld.floors.is_empty(), "ob01.cld must contain floor quads");
    assert!(!cld.walls.is_empty(), "ob01.cld must contain wall segments");

    // Start in Opening Restroom (`er01`) -> transition to Stage 3 (`ob01`) -> back to Stage 2 (`er01`) -> `ob01` -> Stage 4 (`ca11`).
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    assert_eq!(sim.scenario.current_room, "er01");

    // `er01 -> ob01`
    assert!(sim.apply_stage_transition(
        &StageTransitionCommand {
            dest_stage: 3,
            stage_change_mode: 0,
            is_connected_stage: true,
            observation_mode: false,
            dest_pos: [15_600.0, 0.0, -4_140.0],
            dest_rot_y: 0.0,
        },
        None,
        true,
    ));
    assert_eq!(sim.scenario.current_stage, 3);
    assert_eq!(sim.scenario.current_room, "ob01");
    assert_eq!(sim.camera.active_room_id.as_deref(), Some("ob01"));
    assert!(
        sim.camera
            .cam_file
            .as_ref()
            .is_some_and(|c| !c.records.is_empty()),
        "ob01.cam must load non-empty camera records"
    );

    // `ob01 -> er01`
    assert!(sim.apply_stage_transition(
        &StageTransitionCommand {
            dest_stage: 2,
            stage_change_mode: 0,
            is_connected_stage: true,
            observation_mode: false,
            dest_pos: [-18_600.0, 0.0, 19_400.0],
            dest_rot_y: 0.0,
        },
        None,
        true,
    ));
    assert_eq!(sim.scenario.current_stage, 2);
    assert_eq!(sim.scenario.current_room, "er01");
    assert_eq!(sim.camera.active_room_id.as_deref(), Some("er01"));

    // `er01 -> ob01 -> Stage 4 (Forest Trail / ca11)`
    assert!(sim.apply_stage_transition(
        &StageTransitionCommand {
            dest_stage: 3,
            stage_change_mode: 0,
            is_connected_stage: true,
            observation_mode: false,
            dest_pos: [15_600.0, 0.0, -4_140.0],
            dest_rot_y: 0.0,
        },
        None,
        true,
    ));
    assert_eq!(sim.scenario.current_room, "ob01");

    assert!(sim.apply_stage_transition(
        &StageTransitionCommand {
            dest_stage: 4,
            stage_change_mode: 0,
            is_connected_stage: true,
            observation_mode: false,
            dest_pos: [0.0, 0.0, 0.0],
            dest_rot_y: 0.0,
        },
        None,
        true,
    ));
    assert_eq!(sim.scenario.current_stage, 4);
    assert_eq!(sim.scenario.current_room, "ca11");
    assert_eq!(sim.camera.active_room_id.as_deref(), Some("ca11"));
}
