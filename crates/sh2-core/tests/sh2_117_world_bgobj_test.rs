//! Integration & retail-asset verification tests for SH2-117:
//! - 3D room item pickups (`SLUS_202.28` `0x002b97e0` / `sh2pc.exe` `0x0088a0b8`)
//! - `bg_obj_stay.c` / `m3_bgobj.c` / `bg_chara.c` / `chara_bound.c`
//! - Pushable stage objects & `FlashlightClockPush.cpp`
//! - Multi-floor elevator panels, `ElevatorCursorColorFix.cpp` & `HospitalChase.cpp`
//! - Under-car Lying Figure spawns & `CreatureVehicleSpawn.cpp`

use std::path::Path;

use glam::Vec3;
use sh2_core::ai::creatures::lying_figure::{LyingFigureController, ScuState};
use sh2_core::ai::sensory::{DifficultyLevel, PerceivableTarget, RoomLighting};
use sh2_core::config::{ActionDifficulty, AppConfig, EnhancedPreset};
use sh2_core::events::EventFlags;
use sh2_core::inventory::InventoryManager;
use sh2_core::types::event::{pickup_model_rel_path, ITEM_KIND_TABLE};
use sh2_core::{
    event_main_sys_flag_mask, hospital_chase_elevator_stab_param,
    hotel_employee_elevator_cursor_color, resolve_active_room_pickups, resolve_stage_manifest,
    ElevatorCursorColor, ElevatorFloor, ElevatorKind, MirrorPlane, MultiFloorElevatorPanel,
    PushableObjectKind, StayObjectState, FLAG_APART_CLOCK_PUSHED, FLAG_APART_CLOCK_SOLVED,
    FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM, ROOM_APT_E_RM_208, ROOM_HSP_ALT_RPT_HALLWAY,
    SYS_FLAG_MASK_CLOCK_PUSH_FIX, SYS_FLAG_MASK_VANILLA,
};

#[test]
fn retail_pickup_and_bgobj_mdl_assets_exist_on_disk() {
    let data_root = Path::new("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data");
    if !data_root.exists() {
        return;
    }

    for entry in ITEM_KIND_TABLE.iter().skip(1) {
        let rel = pickup_model_rel_path(entry.chr_id).expect("valid pickup chr_id");
        let full = data_root.join(rel);
        assert!(
            full.exists(),
            "Retail 3D pickup model missing on disk: {}",
            full.display()
        );
    }

    for prop_rel in [
        PushableObjectKind::Apart208GrandfatherClock
            .asset_rel_path()
            .expect("clock mdl"),
        "chr/item/b_ell.mdl",
    ] {
        let full = data_root.join(prop_rel);
        assert!(
            full.exists(),
            "Retail stage bgobj model missing on disk: {}",
            full.display()
        );
    }
}

#[test]
fn stage_manifest_resolves_active_3d_pickups_with_valid_models() {
    // Stage 7 (Wood Side Apartments East 2F) has retail Item_List entries.
    let manifest = resolve_stage_manifest(7, None, false, false);
    assert!(
        !manifest.room_pickups.is_empty(),
        "Stage 7 must have retail Item_List pickups"
    );
    let flags = EventFlags::new();
    let active = resolve_active_room_pickups(
        &manifest.room_pickups,
        &flags,
        ActionDifficulty::Normal,
        manifest.is_outdoor,
        [0.0, 0.0, 0.0],
    );
    assert!(!active.is_empty());
    for pickup in &active {
        assert!(pickup_model_rel_path(i32::from(pickup.chr_id)).is_some());
    }
}

#[test]
fn under_car_lying_figure_spawn_respects_sh2ee_fix_and_2000mm_wake() {
    let mut lf_vanilla = LyingFigureController::new(
        1,
        Vec3::new(1000.0, 0.0, 1000.0),
        0.0,
        DifficultyLevel::Normal,
    );
    lf_vanilla.spawn_under_car(false);
    assert_eq!(lf_vanilla.state, ScuState::WaitCar);
    assert!(lf_vanilla.under_vehicle_collision_bit);

    let mut lf_fixed = LyingFigureController::new(
        2,
        Vec3::new(1000.0, 0.0, 1000.0),
        0.0,
        DifficultyLevel::Normal,
    );
    lf_fixed.spawn_under_car(true);
    assert_eq!(lf_fixed.state, ScuState::WaitCar);
    assert!(!lf_fixed.under_vehicle_collision_bit);

    // Player outside 2000.0 mm -> stays WaitCar
    let far_target = PerceivableTarget {
        position: Vec3::new(3500.0, 0.0, 1000.0),
        yaw: 0.0,
        is_flashlight_on: true,
        is_running: false,
        is_alive: true,
    };
    lf_fixed.update(
        &far_target,
        RoomLighting::Day,
        DifficultyLevel::Normal,
        1.0 / 60.0,
    );
    assert_eq!(lf_fixed.state, ScuState::WaitCar);

    // Player inside 2000.0 mm -> transitions to Crawl and clears collision bit
    let near_target = PerceivableTarget {
        position: Vec3::new(2500.0, 0.0, 1000.0),
        yaw: 0.0,
        is_flashlight_on: true,
        is_running: false,
        is_alive: true,
    };
    lf_vanilla.update(
        &near_target,
        RoomLighting::Day,
        DifficultyLevel::Normal,
        1.0 / 60.0,
    );
    assert_eq!(lf_vanilla.state, ScuState::Crawl);
    assert!(!lf_vanilla.under_vehicle_collision_bit);
}

#[test]
fn presets_gate_sh2ee_world_fixes_cleanly() {
    let mut cfg = AppConfig::default();
    cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert!(!cfg.game.enable_sh2ee_world_fixes);
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::Ps2Authentic);

    assert_eq!(
        event_main_sys_flag_mask(
            ROOM_APT_E_RM_208,
            FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM,
            cfg.game.enable_sh2ee_world_fixes
        ),
        SYS_FLAG_MASK_VANILLA
    );
    assert_eq!(
        hotel_employee_elevator_cursor_color(cfg.game.enable_sh2ee_world_fixes),
        ElevatorCursorColor::Gray
    );
    assert_eq!(
        hospital_chase_elevator_stab_param(
            ROOM_HSP_ALT_RPT_HALLWAY,
            34_000.0,
            cfg.game.enable_sh2ee_world_fixes
        ),
        None
    );

    cfg.apply_enhanced_preset(EnhancedPreset::Sh2ee);
    assert!(cfg.game.enable_sh2ee_world_fixes);
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::Sh2ee);
    assert_eq!(
        event_main_sys_flag_mask(
            ROOM_APT_E_RM_208,
            FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM,
            cfg.game.enable_sh2ee_world_fixes
        ),
        SYS_FLAG_MASK_CLOCK_PUSH_FIX
    );
    assert_eq!(
        hotel_employee_elevator_cursor_color(cfg.game.enable_sh2ee_world_fixes),
        ElevatorCursorColor::Green
    );
    assert_eq!(
        hospital_chase_elevator_stab_param(
            ROOM_HSP_ALT_RPT_HALLWAY,
            34_000.0,
            cfg.game.enable_sh2ee_world_fixes
        ),
        Some(5)
    );

    // Pushable clock + stay object + mirror + elevator panel sanity
    let mut stay = StayObjectState::default();
    stay.step(0.0);
    assert!((stay.shadow_y_mm - (-150.0)).abs() < 1e-4);

    let mirror = MirrorPlane::for_room(0x01).expect("room 0x01 mirror");
    assert!((mirror.wall_pos_mm - (-20_000.0)).abs() < 1e-4);

    let mut flags = EventFlags::new();
    flags.set(FLAG_APART_CLOCK_SOLVED, true);
    assert!(PushableObjectKind::Apart208GrandfatherClock.trigger_push(&mut flags));
    assert!(flags.get(FLAG_APART_CLOCK_PUSHED));

    let mut panel =
        MultiFloorElevatorPanel::new(ElevatorKind::HospitalNormal, ElevatorFloor::Floor1, true);
    let inv = InventoryManager::new();
    assert_eq!(
        panel.press_selected_floor(&mut flags, &inv),
        Ok(ElevatorFloor::Roof)
    );
}
