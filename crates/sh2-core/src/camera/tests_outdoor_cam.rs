//! Unit and retail `.cam` integration tests for `SH2-184` dual `<area>GB.cam` + per-tile `.cam`
//! outdoor camera road streaming and tile-crossing continuity.

use glam::Vec3;
use sh2_assets::formats::cam::{
    AreaSizeType, CamFile, CamMovementParams, CamMovementType, CamSourceKind, ChaseCamParam,
    RoadArea, RoadRecord, RoadType, VC_RD_WARP_IN_F,
};
use sh2_assets::AssetPathResolver;

use crate::camera::road_bank::{CamSlotKey, LoadedCamSlot, OutdoorCamBank};
use crate::camera::Ps2CameraController;
use crate::campaign::{CampaignOrchestrator, CampaignPhase, CampaignTickInput};
use crate::config::OutdoorStreamingMode;
use crate::event_camera::{
    EventCameraAuthority, EventCameraSystem, PlayerCameraSuppression, ScriptTransitionKind,
    FOREST_CEMETERY_ANGELA_CAM,
};
use crate::scenario::ScenarioKind;
use crate::simulation::SimulationCore;
use crate::world::OutdoorTileStreamer;
use crate::ScreenFadeState;

fn make_road(
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    rd_type: RoadType,
    flags: u16,
    projection: f32,
) -> RoadRecord {
    RoadRecord {
        lim_sw: RoadArea {
            x0: min_x,
            z0: min_z,
            x1: min_x,
            z1: max_z,
            x2: max_x,
            z2: max_z,
            min_hy: -4000.0,
            max_hy: 1000.0,
        },
        lim_rd: RoadArea {
            x0: min_x - 2000.0,
            z0: min_z - 2000.0,
            x1: min_x - 2000.0,
            z1: max_z + 2000.0,
            x2: max_x + 2000.0,
            z2: max_z + 2000.0,
            min_hy: -4000.0,
            max_hy: 1000.0,
        },
        kind_id: 0,
        flags,
        area_size_type: AreaSizeType::Outdoor,
        rd_type,
        mv_y_type: 0,
        ofs_watch_hy: -300.0,
        trace_btm_hy: 0.0,
        rd_dir_type: 0,
        projection,
        proj_volume: 0.0,
        proj_sec: 0.0,
        cam_mv_type: CamMovementType::Chase,
        params: CamMovementParams::Chase(ChaseCamParam::default()),
    }
}

#[test]
fn test_outdoor_cam_bank_global_vs_tile_priority_and_tiebreak() {
    let mut ctrl = Ps2CameraController::new();
    let mut bank = OutdoorCamBank::default();

    // Global `caGB` has a Road (prio 2) covering X=[-5000, 5000], Z=[-5000, 5000]
    // and a RoadPrioHigh (prio 3) covering X=[1000, 5000], Z=[-5000, 5000].
    bank.set_global_slot(Some(LoadedCamSlot {
        key: CamSlotKey::Global("cagb".to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![
                make_road(-5000.0, 5000.0, -5000.0, 5000.0, RoadType::Road, 0, 448.0),
                make_road(
                    1000.0,
                    5000.0,
                    -5000.0,
                    5000.0,
                    RoadType::RoadPrioHigh,
                    0,
                    512.0,
                ),
            ],
        },
    }));

    // Tile `ca11` has a Road (prio 2) covering X=[-5000, 5000], Z=[-5000, 5000] (exact tie with caGB[0])
    // and a RoadPrioLow (prio 1) covering X=[-5000, -1000], Z=[-5000, 5000].
    bank.upsert_tile_slot(LoadedCamSlot {
        key: CamSlotKey::Tile("ca11".to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![
                make_road(-5000.0, 5000.0, -5000.0, 5000.0, RoadType::Road, 0, 400.0),
                make_road(
                    -5000.0,
                    -1000.0,
                    -5000.0,
                    5000.0,
                    RoadType::RoadPrioLow,
                    0,
                    360.0,
                ),
            ],
        },
    });

    ctrl.cam_file = bank.build_merged_cam(Some("ca11"));
    ctrl.road_bank = bank;

    // 1. At X = 0, Z = 0: both `ca11[0]` (Road, prio 2) and `caGB[0]` (Road, prio 2) have dist = 0.0.
    //    Per-tile `ca11[0]` MUST win the tie over global `caGB[0]` (`vcGetNearestNEAR_ROAD_DATA` 0x001988b0).
    ctrl.reset_to_player(Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Tile("ca11".to_string()), 0))
    );

    // 2. At X = 2500, Z = 0: `caGB[1]` is `RoadPrioHigh` (prio 3), beating tile `ca11[0]` (`Road`, prio 2).
    ctrl.tick(Vec3::new(2500.0, 0.0, 0.0), 0.0, 0.0, 0.0);
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Global("cagb".to_string()), 1))
    );
}

#[test]
fn test_outdoor_cam_bank_preserves_active_road_identity_and_hysteresis_across_tile_swap() {
    let mut ctrl = Ps2CameraController::new();
    let mut bank = OutdoorCamBank::default();

    let gb_slot = LoadedCamSlot {
        key: CamSlotKey::Global("cagb".to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![make_road(
                -140_000.0,
                -120_000.0,
                -25_000.0,
                -10_000.0,
                RoadType::Road,
                VC_RD_WARP_IN_F,
                448.0,
            )],
        },
    };
    let ca11_slot = LoadedCamSlot {
        key: CamSlotKey::Tile("ca11".to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![make_road(
                9_000.0,
                13_000.0,
                -20_000.0,
                -10_000.0,
                RoadType::Road,
                VC_RD_WARP_IN_F,
                448.0,
            )],
        },
    };
    let ca12_slot = LoadedCamSlot {
        key: CamSlotKey::Tile("ca12".to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![make_road(
                9_000.0,
                13_000.0,
                -30_000.0,
                -20_000.0,
                RoadType::Road,
                VC_RD_WARP_IN_F,
                448.0,
            )],
        },
    };

    bank.set_global_slot(Some(gb_slot));
    bank.upsert_tile_slot(ca11_slot);
    bank.upsert_tile_slot(ca12_slot);
    ctrl.cam_file = bank.build_merged_cam(Some("ca11"));
    ctrl.road_bank = bank;

    // Start inside `ca11[0]` near the south tile seam (`z = -19_900`).
    ctrl.reset_to_player(Vec3::new(11_000.0, 0.0, -19_900.0), 0.0);
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Tile("ca11".to_string()), 0))
    );
    let eye_before = ctrl.curr_snapshot.eye;

    // Simulate crossing the `z = -20_000` tile seam into `ca12` while `ca11` remains resident:
    // rebuild merged cam with `ca12` as primary and remap `active_road_index`.
    let prev_origin = ctrl.active_road_origin().cloned();
    ctrl.cam_file = ctrl.road_bank.build_merged_cam(Some("ca12"));
    ctrl.active_road_index = ctrl.road_bank.remap_active_road_index(prev_origin.as_ref());

    // `ca11[0]` moved from flat index 0 to flat index 1, and `active_road_index` tracked it losslessly!
    assert_eq!(ctrl.active_road_index, Some(1));
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Tile("ca11".to_string()), 0))
    );

    // Step 100 mm past `z = -20_000` (into `ca12` at `z = -20_100`).
    // Because `AreaSizeType::Outdoor` hysteresis is `250.0` mm (`extra_boundary_width[3]`),
    // `vcSetCurNearRoadInVC_WORK` MUST keep `ca11[0]` (100 - 250 = -150 <= 0) with NO warp cut!
    ctrl.tick(Vec3::new(11_000.0, 0.0, -20_100.0), 0.0, 0.0, 0.0);
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Tile("ca11".to_string()), 0)),
        "Outdoor 250mm hysteresis must keep ca11[0] 100mm past the seam without snapping"
    );
    assert!(
        (ctrl.curr_snapshot.eye - eye_before).length() < 250.0,
        "Camera must move smoothly across the tile seam, not snap"
    );

    // Now test staying on a `Global` (`caGB`) road while tile slots swap around it.
    ctrl.reset_to_player(Vec3::new(-130_000.0, 0.0, -20_000.0), 0.0);
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Global("cagb".to_string()), 0))
    );
    let prev_gb_origin = ctrl.active_road_origin().cloned();
    ctrl.road_bank.retain_tiles(&["ca12".to_string()]);
    ctrl.cam_file = ctrl.road_bank.build_merged_cam(Some("ca12"));
    ctrl.active_road_index = ctrl
        .road_bank
        .remap_active_road_index(prev_gb_origin.as_ref());
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Global("cagb".to_string()), 0)),
        "Global caGB road identity must survive tile eviction"
    );
}

#[test]
fn test_retail_forest_slice_ca_gb_plus_tiles_and_cb54_seamless_continuity() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    let mut ctrl = Ps2CameraController::new();
    let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);

    // 1. Start at `ob01` stairs, then transition into Forest Trail (`stage 4`, `ca11`).
    assert!(ctrl.load_room_cam_with_resolver(&resolver, "ob01", false));
    assert!(
        ctrl.road_bank.is_empty(),
        "Indoor ob01 must not populate outdoor road_bank"
    );

    assert!(ctrl.load_room_cam_with_resolver(&resolver, "ca11", false));
    assert_eq!(ctrl.road_bank.global_cam_id(), Some("cagb"));
    ctrl.reset_to_player(Vec3::new(10_500.0, 0.0, -15_000.0), 0.0);
    assert!(ctrl.active_road_index.is_some());

    // 2. Walk smoothly from `ca11` across `z = -20_000` into `ca12` (mid-trail).
    for step in 0..30 {
        let z = -19_000.0 - (step as f32) * 100.0; // -19,000 -> -21,900
        let pos = Vec3::new(11_000.0, 0.0, z);
        let active_tile = if z > -20_000.0 { "ca11" } else { "ca12" };
        if let Some(plan) = streamer.plan_if_changed(4, pos.x, pos.z) {
            assert_eq!(plan.priority_gb_cam, Some("bg/ca/caGB.cam"));
            assert!(ctrl.sync_outdoor_cam_plan_with_resolver(&resolver, &plan, active_tile, false));
            assert_eq!(ctrl.road_bank.global_cam_id(), Some("cagb"));
        }
        ctrl.tick(pos, 0.0, 0.0, 0.0);
        assert!(
            ctrl.active_road_index.is_some(),
            "Active road must remain valid at z={z}"
        );
    }

    // 3. Verify waypoints along the Forest Slice:
    //    Well (`ca04`), Graveyard (`ca18..ca22` + Angela scripted cemetery cam `0x0059f7ac`),
    //    Ranch (`ca26`/`ca27`/`ca28` + `caGB.cam` road resolution).
    let forest_waypoints: &[(&str, Vec3)] = &[
        ("ca04", Vec3::new(25_000.0, 0.0, -85_000.0)), // Well area
        ("ca18", Vec3::new(-57_000.0, -500.0, -3_000.0)), // Graveyard east
        ("ca19", Vec3::new(-72_000.0, -500.0, 2_000.0)), // Graveyard north-mid
        ("ca20", Vec3::new(-65_000.0, -500.0, -3_000.0)), // Graveyard south-mid
        ("ca21", Vec3::new(-90_000.0, -500.0, 3_000.0)), // Graveyard west-north
        ("ca22", Vec3::new(-88_000.0, -500.0, -1_000.0)), // Graveyard west-south
        ("ca26", Vec3::new(-130_000.0, -500.0, -21_000.0)), // Ranch (`caGB.cam` record [2]!)
        ("ca28", Vec3::new(-148_300.0, -500.0, -20_800.0)), // Ranch west exit (`ca28.cam`)
    ];

    let mut hit_cagb_road = false;
    for &(tile, pos) in forest_waypoints {
        let plan = streamer.plan(4, pos.x, pos.z);
        assert_eq!(plan.priority_gb_cam, Some("bg/ca/caGB.cam"));
        assert!(ctrl.sync_outdoor_cam_plan_with_resolver(&resolver, &plan, tile, false));
        assert_eq!(ctrl.road_bank.global_cam_id(), Some("cagb"));
        ctrl.tick(pos, 0.0, 0.0, 0.0);
        assert!(
            ctrl.active_road_index.is_some(),
            "Expected active road at {tile} ({pos:?})"
        );
        if matches!(ctrl.active_road_origin(), Some((CamSlotKey::Global(g), _)) if g == "cagb") {
            hit_cagb_road = true;
        }
    }
    assert!(
        hit_cagb_road,
        "Expected Ranch waypoint (-130000, -500, -21000) to resolve a road from resident caGB.cam"
    );

    // 4. Verify Angela scripted cemetery camera (`0x0059f7ac`) overrides and smoothly hands back
    //    to the resident graveyard `.cam` bank without losing `caGB.cam` residency.
    let graveyard_pos = Vec3::new(-72_000.0, -500.0, 2_000.0);
    let plan_gy = streamer.plan(4, graveyard_pos.x, graveyard_pos.z);
    assert!(ctrl.sync_outdoor_cam_plan_with_resolver(&resolver, &plan_gy, "ca19", false));
    let mut ev_cam = EventCameraSystem::new();
    ev_cam.script_camera.set_camera_script(
        FOREST_CEMETERY_ANGELA_CAM,
        ScriptTransitionKind::WarpCut,
        ctrl.curr_snapshot.fov_y,
    );
    assert_eq!(
        ev_cam.active_authority(),
        EventCameraAuthority::ScriptOverride
    );
    let _ = ev_cam.tick(
        &mut ctrl,
        graveyard_pos,
        0.0,
        0.0,
        0.0,
        1.0 / 60.0,
        PlayerCameraSuppression::Active,
        &[],
        None,
    );
    assert!(
        (ctrl.curr_snapshot.eye - FOREST_CEMETERY_ANGELA_CAM.eye_mm).length() < 1e-2,
        "Scripted Angela cemetery camera 0x0059f7ac must take control"
    );
    ev_cam.script_camera.return_pre_auto_cam_work(
        ScriptTransitionKind::SmoothBlend,
        &mut ctrl,
        graveyard_pos,
        0.0,
    );
    // SmoothBlend converges cleanly back to `GameplayRail` over ticks.
    for _ in 0..180 {
        if ev_cam.active_authority() == EventCameraAuthority::GameplayRail {
            break;
        }
        let _ = ev_cam.tick(
            &mut ctrl,
            graveyard_pos,
            0.0,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            None,
        );
    }
    assert_eq!(
        ev_cam.active_authority(),
        EventCameraAuthority::GameplayRail
    );
    ctrl.tick(graveyard_pos, 0.0, 0.0, 0.0);
    assert_eq!(ctrl.road_bank.global_cam_id(), Some("cagb"));
    assert!(ctrl.active_road_index.is_some());

    // 5. Transition from Forest (`stage 4`, `ca`) to Town East (`stage 5`, `cb54`):
    //    `caGB.cam` must be cleanly replaced by `cbGB.cam` alongside `cb54.cam`.
    let cb54_pos = Vec3::new(47_000.0, -500.0, 50_500.0);
    let plan_cb = streamer.plan(5, cb54_pos.x, cb54_pos.z);
    assert_eq!(plan_cb.priority_gb_cam, Some("bg/cb/cbGB.cam"));
    assert!(ctrl.sync_outdoor_cam_plan_with_resolver(&resolver, &plan_cb, "cb54", false));
    assert_eq!(ctrl.road_bank.global_cam_id(), Some("cbgb"));
    ctrl.reset_to_player(cb54_pos, 0.0);
    assert_eq!(
        ctrl.active_road_origin(),
        Some(&(CamSlotKey::Tile("cb54".to_string()), 0))
    );

    // Also verify a `cbGB.cam` main-street corridor resolves cleanly when walking into Nathan Ave.
    let nathan_ave_pos = Vec3::new(0.0, -500.0, 10_000.0);
    let plan_cb_street = streamer.plan(5, nathan_ave_pos.x, nathan_ave_pos.z);
    assert!(ctrl.sync_outdoor_cam_plan_with_resolver(&resolver, &plan_cb_street, "cb03", false));
    assert_eq!(ctrl.road_bank.global_cam_id(), Some("cbgb"));
    ctrl.tick(nathan_ave_pos, 0.0, 0.0, 0.0);
    assert!(ctrl.active_road_index.is_some());
}

#[test]
fn test_retail_campaign_orchestrator_outdoor_tile_crossing_merges_cagb_and_tiles() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 4;
    sim.scenario.current_room = "ca11".to_string();
    sim.snap_player_transform(Vec3::new(11_000.0, 0.0, -19_500.0), 0.0);
    assert!(sim.camera.load_room_cam("ca11", false));
    sim.camera
        .reset_to_player(sim.scenario.player_position, 0.0);

    let mut orch = CampaignOrchestrator::from_saved_stage(
        4,
        Some("ca11".to_string()),
        [11_000.0, 0.0, -19_500.0],
        0.0,
        OutdoorStreamingMode::ClassicPs2LoadBg2x2,
        false,
    );
    orch.phase = CampaignPhase::ActiveGameplay;
    orch.fade = ScreenFadeState::default();
    orch.player_locked = false;

    let idle = CampaignTickInput::default();
    let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
    assert_eq!(sim.camera.road_bank.global_cam_id(), Some("cagb"));
    assert!(sim.camera.active_road_index.is_some());

    // Cross into `ca12` (Z = -20_100 mm): `caGB.cam` must remain resident alongside both `ca11` & `ca12`,
    // and `active_road_index` must remain valid without resetting.
    sim.scenario.player_position = Vec3::new(11_000.0, 0.0, -20_100.0);
    let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
    assert_eq!(sim.scenario.current_room, "ca12");
    assert_eq!(sim.camera.road_bank.global_cam_id(), Some("cagb"));
    assert!(
        sim.camera.road_bank.has_tile("ca11", false)
            && sim.camera.road_bank.has_tile("ca12", false),
        "Both ca11 and ca12 must stay resident in the 2x2 neighborhood across the seam"
    );
    assert!(sim.camera.active_road_index.is_some());
}

fn make_tile_slot(id: &str) -> LoadedCamSlot {
    LoadedCamSlot {
        key: CamSlotKey::Tile(id.to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![make_road(
                -1000.0,
                1000.0,
                -1000.0,
                1000.0,
                RoadType::Road,
                0,
                448.0,
            )],
        },
    }
}

fn empty_test_resolver() -> AssetPathResolver {
    AssetPathResolver {
        game_root: std::path::PathBuf::from("/nonexistent/sh2"),
        data_dir: std::path::PathBuf::from("/nonexistent/sh2/data"),
        sh2e_dir: None,
    }
}

#[test]
fn test_outdoor_cam_active_room_id_normalization_and_empty_merged_is_none() {
    let empty_resolver = empty_test_resolver();

    let plan = crate::world::TileStreamingPlan {
        area: Some(crate::world::OutdoorArea::Ca),
        priority_gb_cam: Some("bg/ca/caGB.cam"),
        desired_blocks: vec![11],
        ..Default::default()
    };

    // 1. Pre-populated bank with `"ca11"` + `"cagb"`: `sync_outdoor_cam_plan_with_resolver`
    //    must normalize `"bg/ca/CA11.cam"` and `"CA11"` to canonical `"ca11"`.
    let mut ctrl = Ps2CameraController::new();
    ctrl.road_bank.set_global_slot(Some(LoadedCamSlot {
        key: CamSlotKey::Global("cagb".to_string()),
        source: CamSourceKind::VanillaRetail,
        prefer_sh2ee: false,
        cam_file: CamFile {
            records: vec![make_road(
                -5000.0,
                5000.0,
                -5000.0,
                5000.0,
                RoadType::Road,
                0,
                448.0,
            )],
        },
    }));
    ctrl.road_bank.upsert_tile_slot(make_tile_slot("ca11"));

    assert!(ctrl.sync_outdoor_cam_plan_with_resolver(
        &empty_resolver,
        &plan,
        "bg/ca/CA11.cam",
        false
    ));
    assert_eq!(ctrl.active_room_id.as_deref(), Some("ca11"));
    assert_eq!(ctrl.active_cam_source, Some(CamSourceKind::VanillaRetail));

    assert!(ctrl.sync_outdoor_cam_plan_with_resolver(&empty_resolver, &plan, "CA11", false));
    assert_eq!(ctrl.active_room_id.as_deref(), Some("ca11"));

    assert!(ctrl.switch_outdoor_tile_cam_with_resolver(&empty_resolver, "bg/ca/CA11.cam", false));
    assert_eq!(ctrl.active_room_id.as_deref(), Some("ca11"));

    // 2. When no `.cam` records are resident (`merged.is_none()`), all three loaders must clear
    //    `active_room_id` and `active_cam_source` to `None` and return `false`.
    let missing_plan = crate::world::TileStreamingPlan {
        area: Some(crate::world::OutdoorArea::Ca),
        priority_gb_cam: None,
        desired_blocks: vec![99],
        ..Default::default()
    };
    assert!(!ctrl.sync_outdoor_cam_plan_with_resolver(
        &empty_resolver,
        &missing_plan,
        "bg/ca/CA99.cam",
        false
    ));
    assert!(ctrl.cam_file.is_none());
    assert_eq!(ctrl.active_room_id, None);
    assert_eq!(ctrl.active_cam_source, None);

    ctrl.active_room_id = Some("stale".to_string());
    ctrl.active_cam_source = Some(CamSourceKind::VanillaRetail);
    assert!(!ctrl.switch_outdoor_tile_cam_with_resolver(&empty_resolver, "bg/ca/CA99.cam", false));
    assert!(ctrl.cam_file.is_none());
    assert_eq!(ctrl.active_room_id, None);
    assert_eq!(ctrl.active_cam_source, None);

    ctrl.active_room_id = Some("stale".to_string());
    ctrl.active_cam_source = Some(CamSourceKind::VanillaRetail);
    assert!(!ctrl.load_room_cam_with_resolver(&empty_resolver, "bg/ca/CA99.cam", false));
    assert!(ctrl.cam_file.is_none());
    assert_eq!(ctrl.active_room_id, None);
    assert_eq!(ctrl.active_cam_source, None);
}

#[test]
fn test_outdoor_cam_bank_lru_eviction_order() {
    let mut bank = OutdoorCamBank::default();
    for id in ["ca11", "ca12", "ca13", "ca14"] {
        bank.upsert_tile_slot(make_tile_slot(id));
    }

    // Re-access the oldest tile (`ca11`): LRU order becomes [ca12, ca13, ca14, ca11].
    assert!(bank.touch_tile("ca11"));
    assert!(!bank.touch_tile("ca99"));

    // Insert `ca15` and cap to 4: `ca12` (LRU) must be evicted, while `ca11` stays resident.
    bank.upsert_tile_slot(make_tile_slot("ca15"));
    bank.retain_area_and_cap("ca", 4);
    let ids: Vec<&str> = bank.tile_slots().iter().map(|s| s.key.id()).collect();
    assert_eq!(ids, vec!["ca13", "ca14", "ca11", "ca15"]);

    // Making `ca13` active via `build_merged_cam` promotes it to MRU: [ca14, ca11, ca15, ca13].
    assert!(bank.build_merged_cam(Some("ca13")).is_some());
    bank.upsert_tile_slot(make_tile_slot("ca16"));
    bank.retain_area_and_cap("ca", 4);
    let ids: Vec<&str> = bank.tile_slots().iter().map(|s| s.key.id()).collect();
    assert_eq!(ids, vec!["ca11", "ca15", "ca13", "ca16"]);

    // Upserting an already-resident tile (`ca11`) promotes it to MRU: [ca15, ca13, ca16, ca11].
    bank.upsert_tile_slot(make_tile_slot("ca11"));
    bank.upsert_tile_slot(make_tile_slot("ca17"));
    bank.retain_area_and_cap("ca", 4);
    let ids: Vec<&str> = bank.tile_slots().iter().map(|s| s.key.id()).collect();
    assert_eq!(ids, vec!["ca13", "ca16", "ca11", "ca17"]);

    // Verify `switch_outdoor_tile_cam_with_resolver` promotes a revisited tile so it survives
    // subsequent tile switches.
    let empty_resolver = empty_test_resolver();
    let mut ctrl = Ps2CameraController::new();
    ctrl.road_bank = bank;
    // Current LRU order: [ca13, ca16, ca11, ca17]. Revisit `ca13` -> [ca16, ca11, ca17, ca13].
    assert!(ctrl.switch_outdoor_tile_cam_with_resolver(&empty_resolver, "CA13", false));
    assert_eq!(ctrl.active_room_id.as_deref(), Some("ca13"));
    ctrl.road_bank.upsert_tile_slot(make_tile_slot("ca18"));
    ctrl.road_bank.retain_area_and_cap("ca", 4);
    let ids: Vec<&str> = ctrl
        .road_bank
        .tile_slots()
        .iter()
        .map(|s| s.key.id())
        .collect();
    assert_eq!(ids, vec!["ca11", "ca17", "ca13", "ca18"]);
}
