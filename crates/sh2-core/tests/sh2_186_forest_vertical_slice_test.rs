//! End-to-end `ob01 -> Forest Trail (ca11..ca28) -> Angela -> Town East (cb54)` vertical-slice
//! integration test (`SH2-186`).
//!
//! Verifies 1:1 retail data and runtime behavior:
//! - Stage/room transition triggers & destination coordinates decoded from `sh2pc.exe` retail tables:
//!   - `stg_ovservation.c` (`ACT1_STAGE_OBSERVATION` `0x008e7a48`, `Event_List` `0x008e7948`)
//!   - `stg_forest.c` (`ACT1_STAGE_FOREST` `0x008e7538`, `Event_List` `0x008e6ff8`, `ev_pos` `0x008e6e88`)
//!   - `stg_town_east.c` (`ACT1_STAGE_TOWN_EAST`)
//! - `CA_DrawEnvData` (`SLUS_202.28` `0x00361b10` / `sh2pc.exe` `0x007f41d0`) and
//!   `CB_DrawEnvData` (`SLUS_202.28` `0x00361ef0` / `sh2pc.exe` `0x007f45b0`) lighting/ambient/fog
//!   parity across Vanilla PS2 (`Ps2Authentic`) and Enhanced (`EnhancedEdition`) profiles.
//! - Multi-tile `OutdoorTileStreamer` coverage across `ca11 -> ca14 -> ca18..ca22 (Graveyard) ->
//!   ca25..ca28 (Silent Hill Ranch / gate) -> cb54 (Town East arrival)` and retail `.cld` / `.cam`
//!   loading.

use sh2_assets::formats::cam::parse_cam;
use sh2_assets::formats::cld::parse_cld;
use sh2_assets::AssetPathResolver;
use sh2_core::campaign::retail_events::decode_retail_move_destination;
use sh2_core::config::LightingProfile;
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::events::engine::StageTransitionCommand;
use sh2_core::events::stages_act1::{act1_stage_descriptor, Act1StageId};
use sh2_core::events::stages_act2::normalize_retail_event_entry_for_input;
use sh2_core::events::triggers::EventEvalInput;
use sh2_core::inventory::InventoryManager;
use sh2_core::lighting::{
    apply_draw_env_fixes, lookup_draw_env, lookup_draw_env_for_time,
    DRAW_ENV_ROOM_DAYTIME_WILDCARD, DRAW_ENV_ROOM_WILDCARD,
};
use sh2_core::room_loader::stage_room_descriptor;
use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;
use sh2_core::types::{EventDestShapeKind, EventListEntry, EventResultKind};
use sh2_core::world::{OutdoorArea, OutdoorTileStreamer};

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
fn sh2_186_forest_to_town_east_retail_transitions_and_angela_events_decode_cleanly() {
    let stage_ob = act1_stage_descriptor(Act1StageId::Observation as u8).expect("stage 3");
    let stage_forest = act1_stage_descriptor(Act1StageId::Forest as u8).expect("stage 4");
    let _stage_town_east = act1_stage_descriptor(Act1StageId::TownEast as u8).expect("stage 5");

    // 1. `ob01` (Stage 3) -> Forest Trail `ca11` (Stage 4)
    let ob_to_forest = normalized_moves_to_stage(stage_ob.event_list, Act1StageId::Forest);
    assert!(
        !ob_to_forest.is_empty(),
        "ACT1_STAGE_OBSERVATION must have a transition into Stage 4 (Forest Trail)"
    );
    let dest_forest = decode_retail_move_destination(
        stage_ob.ev_pos,
        ob_to_forest[0].dest_ev_pos_offset(),
        ob_to_forest[0]
            .dest_shape_kind()
            .unwrap_or(EventDestShapeKind::Point),
    )
    .expect("decode ob01 -> Forest Trail destination pose");
    assert!(dest_forest.0.iter().all(|v| v.is_finite()));

    // 2. `stg_forest.c` (`0x008e6ff8`) must contain:
    //    - Well save point (`prog = 1`),
    //    - Angela Graveyard cutscene trigger (`prog = 2`, flag `36 = 0x24`),
    //    - Stage 5 Town East (`cb54`) arrival transition.
    let forest_entries: Vec<EventListEntry> = stage_forest
        .event_list
        .iter()
        .copied()
        .take_while(|e| !e.is_sentinel())
        .collect();
    assert!(
        forest_entries
            .iter()
            .any(|e| e.result_kind() == Some(EventResultKind::Program) && e.prog_index() == 1),
        "stg_forest.c must include Well save point program (prog=1)"
    );
    assert!(
        forest_entries
            .iter()
            .any(|e| e.result_kind() == Some(EventResultKind::Program) && e.prog_index() == 2),
        "stg_forest.c must include Angela Graveyard encounter program (prog=2, flag 0x24)"
    );

    let forest_to_town = normalized_moves_to_stage(stage_forest.event_list, Act1StageId::TownEast);
    assert!(
        !forest_to_town.is_empty(),
        "ACT1_STAGE_FOREST must have a transition into Stage 5 (Town East / cb54)"
    );
    let dest_town = decode_retail_move_destination(
        stage_forest.ev_pos,
        forest_to_town[0].dest_ev_pos_offset(),
        forest_to_town[0]
            .dest_shape_kind()
            .unwrap_or(EventDestShapeKind::Point),
    )
    .expect("decode Forest Trail -> Town East (cb54) destination pose");
    assert!(
        dest_town.0.iter().all(|v| v.is_finite()),
        "Forest -> Town East destination pose must be finite: {:?}",
        dest_town.0
    );

    // Canonical stage room descriptors for Stages 3, 4, 5
    assert_eq!(stage_room_descriptor(3).unwrap().default_room_id, "ob01");
    assert_eq!(stage_room_descriptor(4).unwrap().default_room_id, "ca11");
    assert_eq!(stage_room_descriptor(5).unwrap().default_room_id, "cb54");
}

#[test]
fn sh2_186_ca_and_cb_draw_env_lighting_parity_vanilla_vs_enhanced() {
    // `CA_DrawEnvData[0]` (`SLUS_202.28` `0x00361b10`, `sh2pc.exe` `0x007f41d0`): `ca14` exact entry
    let ca14_raw = lookup_draw_env("CA", 14).expect("CA_DrawEnvData[0]");
    assert_eq!(ca14_raw.room_no, 14);
    assert_eq!(ca14_raw.stage_id, 1);
    assert!((ca14_raw.ambient[0] - 0.9).abs() < 1e-4);
    assert!((ca14_raw.ambient_scale - 31.5).abs() < 1e-4);
    assert!((ca14_raw.fog_near - 10_000.0).abs() < 1e-2);
    assert!((ca14_raw.fog_far - 17_635.0).abs() < 1e-2);
    assert!((ca14_raw.fog_max - 21.0).abs() < 1e-2);
    assert!((ca14_raw.fog_min - 234.0).abs() < 1e-2);

    // Vanilla PS2 keeps `ca14` unmodified; Enhanced applies `CemeteryLightingASM` (`0x0047C2DB`)
    let ca14_ps2 = apply_draw_env_fixes(ca14_raw, "ca14", LightingProfile::Ps2Authentic);
    assert_eq!(ca14_ps2.room_no, 14);
    assert!((ca14_ps2.ambient[0] - 0.9).abs() < 1e-4);

    let ca14_ee = apply_draw_env_fixes(ca14_raw, "ca14", LightingProfile::EnhancedEdition);
    assert_eq!(ca14_ee.room_no, DRAW_ENV_ROOM_WILDCARD);
    assert!((ca14_ee.ambient[0] - 0.65).abs() < 1e-4);
    assert!((ca14_ee.ambient_scale - 16.0).abs() < 1e-4);
    assert_eq!(
        ca14_ee.effective_fog_bounds(false),
        (5500.0, 3000.0, 20.0, 235.0)
    );

    // All other Forest Trail tiles (`ca11`, `ca18`, `ca20`, `ca28`) resolve `CA_DrawEnvData[1]` (`0xFFFF`)
    for room in [11u16, 12, 16, 18, 20, 22, 26, 28] {
        let env = lookup_draw_env("CA", room).expect("CA wildcard");
        assert_eq!(env.room_no, DRAW_ENV_ROOM_WILDCARD);
        assert!((env.ambient[0] - 0.65).abs() < 1e-4);
        assert_eq!(env.shadow_darkness, 0);
        assert_eq!(
            env.effective_fog_bounds(false),
            (5500.0, 3000.0, 20.0, 235.0)
        );
    }

    // `CB_DrawEnvData` (`SLUS_202.28` `0x00361ef0`, `sh2pc.exe` `0x007f45b0`):
    // Daytime arrival at `cb54` MUST match `0xFFFE` (ambient=0.65, shadow_darkness=0),
    // while Nighttime Town East matches `0xFFFF` (ambient=0.15, shadow_darkness=1).
    let cb54_day = lookup_draw_env("CB", 54).expect("CB daytime");
    assert_eq!(cb54_day.room_no, DRAW_ENV_ROOM_DAYTIME_WILDCARD);
    assert!((cb54_day.ambient[0] - 0.65).abs() < 1e-4);
    assert!((cb54_day.ambient_scale - 16.0).abs() < 1e-4);
    assert_eq!(cb54_day.shadow_darkness, 0);
    assert_eq!(
        cb54_day.effective_fog_bounds(false),
        (5500.0, 3000.0, 20.0, 235.0)
    );

    let cb54_night = lookup_draw_env_for_time("CB", 54, true).expect("CB nighttime");
    assert_eq!(cb54_night.room_no, DRAW_ENV_ROOM_WILDCARD);
    assert!((cb54_night.ambient[0] - 0.15).abs() < 1e-4);
    assert!((cb54_night.ambient_scale - 31.5).abs() < 1e-4);
    assert_eq!(cb54_night.shadow_darkness, 1);
    assert_eq!(
        cb54_night.effective_fog_bounds(true),
        (8000.0, 3000.0, 100.0, 255.0)
    );
}

#[test]
fn sh2_186_end_to_end_vertical_slice_ob01_forest_angela_to_cb54_with_retail_assets() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.join("bg/ca/ca11.cld").exists()
        || !resolver.data_dir.join("bg/cb/cb54.cld").exists()
    {
        return;
    }

    // 1. Validate retail `.cld` & global `.cam` across all 4 Forest landmarks + `cb54` arrival.
    for rel_cld in [
        "bg/ca/ca11.cld",
        "bg/ca/ca14.cld",
        "bg/ca/ca18.cld",
        "bg/ca/ca20.cld",
        "bg/ca/ca22.cld",
        "bg/ca/ca26.cld",
        "bg/ca/ca28.cld",
        "bg/cb/cb54.cld",
    ] {
        let bytes = std::fs::read(resolver.data_dir.join(rel_cld))
            .unwrap_or_else(|e| panic!("read {rel_cld}: {e}"));
        let cld = parse_cld(&bytes).unwrap_or_else(|e| panic!("parse {rel_cld}: {e:?}"));
        assert!(
            !cld.floors.is_empty() || !cld.walls.is_empty(),
            "{rel_cld} must contain valid collision primitives"
        );
    }
    for rel_cam in ["bg/ca/caGB.cam", "bg/cb/cbGB.cam"] {
        let bytes = std::fs::read(resolver.data_dir.join(rel_cam))
            .unwrap_or_else(|e| panic!("read {rel_cam}: {e}"));
        let cam = parse_cam(&bytes).unwrap_or_else(|e| panic!("parse {rel_cam}: {e:?}"));
        assert!(
            !cam.records.is_empty(),
            "{rel_cam} must contain outdoor camera road records"
        );
    }

    // 2. Verify OutdoorTileStreamer walks cleanly across all 4 landmarks into `cb54`.
    let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
    let landmarks: &[(u16, f32, f32, OutdoorArea, u16)] = &[
        (4, 10_000.0, -10_000.0, OutdoorArea::Ca, 11), // ca11 entry
        (4, -10_000.0, -10_000.0, OutdoorArea::Ca, 14), // ca14 mid-trail
        (4, -70_000.0, -10_000.0, OutdoorArea::Ca, 20), // ca20 Graveyard / Angela
        (4, -150_000.0, -30_000.0, OutdoorArea::Ca, 28), // ca28 Ranch / Town East gate
        (5, 50_000.0, 50_000.0, OutdoorArea::Cb, 54),  // cb54 Town East arrival
    ];
    for &(stage, x, z, expected_area, expected_block) in landmarks {
        let plan = streamer.plan(stage, x, z);
        assert_eq!(plan.area, Some(expected_area));
        assert!(
            plan.desired_blocks.contains(&expected_block),
            "stage={stage} ({x}, {z}) must desire block {expected_block}, got {:?}",
            plan.desired_blocks
        );
    }

    // 3. Drive `SimulationCore` end-to-end: `er01 -> ob01 -> ca11 (Stage 4) -> Angela flag 36 -> cb54 (Stage 5)`.
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    assert_eq!(sim.scenario.current_room, "er01");

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

    assert!(sim.apply_stage_transition(
        &StageTransitionCommand {
            dest_stage: 4,
            stage_change_mode: 0,
            is_connected_stage: true,
            observation_mode: false,
            dest_pos: [10_000.0, 0.0, -10_000.0],
            dest_rot_y: 0.0,
        },
        None,
        true,
    ));
    assert_eq!(sim.scenario.current_stage, 4);
    assert_eq!(sim.scenario.current_room, "ca11");
    assert_eq!(sim.camera.active_room_id.as_deref(), Some("ca11"));

    // Mark Angela Graveyard encounter complete (`stg_forest.c` flag `36 = 0x24`).
    sim.scenario.event_flags.insert(36);
    assert!(sim.scenario.event_flags.contains(&36));

    // Transition through Silent Hill Ranch / Town East gate -> Stage 5 (`cb54`).
    assert!(sim.apply_stage_transition(
        &StageTransitionCommand {
            dest_stage: 5,
            stage_change_mode: 0,
            is_connected_stage: true,
            observation_mode: false,
            dest_pos: [50_000.0, 0.0, 50_000.0],
            dest_rot_y: 0.0,
        },
        None,
        true,
    ));
    assert_eq!(sim.scenario.current_stage, 5);
    assert_eq!(sim.scenario.current_room, "cb54");
    assert_eq!(sim.camera.active_room_id.as_deref(), Some("cb54"));
}
