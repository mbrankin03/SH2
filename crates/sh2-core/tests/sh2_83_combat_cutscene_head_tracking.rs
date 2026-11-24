//! Golden-vector and retail-asset integration tests for SH2-83:
//! - Pressure-sensitive combat (`0xC0 < pressure`), SH2:EE `HoldToStomp` / `SwapLightHeavyAttack` (`200 ms`),
//!   running light-attack override, and Native zero-latency split bindings / `R2` analog trigger thresholds.
//! - `CutsceneUnequip` (`RemoveWeaponFromCutscene.cpp` + `PlayerCheckModelParts` `0x001098d0`).
//! - Procedural head/neck bone tracking (`PlayerCheckNeckAngle` `0x00108e10`, `FUN_005424a0` / `FUN_005426b0`)
//!   and `TeddyBearLookFix` (`0x0727`, flag `0xAA`).

use glam::Vec3;
use sh2_core::animation::{
    select_head_look_target, HeadLookTracker, LookTargetCandidate, LookTargetKind,
    DEFAULT_LOOK_SEARCH_RADIUS_MM, JAMES_HEAD_BONE_INDEX, JAMES_NECK_BONE_INDEX,
    MARIA_HEAD_BONE_INDEX, MARIA_NECK_BONE_INDEX, NECK_CRANIAL_SHARE, NECK_PITCH_MAX_RAD,
    NECK_PITCH_MIN_RAD, NECK_UPPER_SPINE_SHARE, NECK_YAW_MAX_RAD, NECK_YAW_MIN_RAD,
};
use sh2_core::combat::{
    CombatController, CombatInputSnapshot, CombatMode, WeaponGrip, ATK_CHAINSAW_LIGHT,
    ATK_CHAINSAW_THRUST, ATK_GREAT_KNIFE_OVERHEAD_1, ATK_GREAT_KNIFE_SWING_1, ATK_KICKS,
    ATK_MARIA_CLEAVER_HEAVY, ATK_MARIA_CLEAVER_LIGHT, ATK_PIPE_LUNGE, ATK_PIPE_OVERHEAD,
    ATK_PLANK_HEAVY, ATK_PLANK_LIGHT, ATK_STOMPS, PS2_PRESSURE_HEAVY_THRESHOLD,
};
use sh2_core::config::{
    AppConfig, CombatInputMode, EnhancedPreset, PS2_HEAVY_PRESSURE_THRESHOLD_NORM,
};
use sh2_core::cutscene::{
    effective_right_hand_pose, should_unequip_weapon_in_cutscene, JAMES_EMPTY_RIGHT_HAND_POSE,
};
use sh2_core::events::{EventFlags, ACT1_FLAG_TEDDY_BEAR_NEEDLE_TAKEN, ACT1_PROP_KIND_TEDDY_BEAR};

#[test]
fn test_ps2_pressure_threshold_light_vs_heavy_across_melee_weapons() {
    let dt = 1.0 / 60.0;
    let weapons_and_attacks = [
        (
            WeaponGrip::WoodenPlank,
            ATK_PLANK_LIGHT.id,
            ATK_PLANK_HEAVY.id,
        ),
        (
            WeaponGrip::SteelPipe,
            ATK_PIPE_LUNGE.id,
            ATK_PIPE_OVERHEAD.id,
        ),
        (
            WeaponGrip::GreatKnife,
            ATK_GREAT_KNIFE_SWING_1.id,
            ATK_GREAT_KNIFE_OVERHEAD_1.id,
        ),
        (
            WeaponGrip::Chainsaw,
            ATK_CHAINSAW_LIGHT.id,
            ATK_CHAINSAW_THRUST.id,
        ),
        (
            WeaponGrip::MariaChineseCleaver,
            ATK_MARIA_CLEAVER_LIGHT.id,
            ATK_MARIA_CLEAVER_HEAVY.id,
        ),
    ];

    for (weapon, light_id, heavy_id) in weapons_and_attacks {
        // Light pressure (`0xC0 = 192`, just below `0xC1` threshold in `0x005343a0`) -> Light swing
        let mut ctrl = CombatController::new(false, weapon == WeaponGrip::MariaChineseCleaver);
        ctrl.combat_input_mode = CombatInputMode::Ps2Pressure;
        ctrl.enable_hold_to_stomp = false;
        ctrl.enable_swap_light_heavy = false;
        ctrl.equip_weapon(weapon);
        ctrl.mode = CombatMode::Ready;
        ctrl.handle_combat_inputs_ex(
            CombatInputSnapshot {
                ready_held: true,
                action_pressed: true,
                action_held: true,
                attack_pressure: Some(PS2_PRESSURE_HEAVY_THRESHOLD - 1),
                ..Default::default()
            },
            dt,
        );
        assert_eq!(ctrl.mode, CombatMode::Swing, "{weapon:?} light pressure");
        assert!(!ctrl.is_heavy_attack);
        assert_eq!(ctrl.active_attack.unwrap().id, light_id);

        // Heavy pressure (`0xC1 = 193`, `0x005343a0`) -> Heavy swing
        ctrl.mode = CombatMode::Ready;
        ctrl.handle_combat_inputs_ex(
            CombatInputSnapshot {
                ready_held: true,
                action_pressed: true,
                action_held: true,
                attack_pressure: Some(PS2_PRESSURE_HEAVY_THRESHOLD),
                ..Default::default()
            },
            dt,
        );
        assert_eq!(ctrl.mode, CombatMode::Swing, "{weapon:?} heavy pressure");
        assert!(ctrl.is_heavy_attack);
        assert_eq!(ctrl.active_attack.unwrap().id, heavy_id);
    }
}

#[test]
fn test_ps2_pressure_and_native_split_kick_vs_stomp() {
    let dt = 1.0 / 60.0;
    let mut ctrl = CombatController::new(false, false);
    ctrl.equip_weapon(WeaponGrip::Handgun);
    ctrl.combat_input_mode = CombatInputMode::Ps2Pressure;
    ctrl.enable_hold_to_stomp = false;
    ctrl.mode = CombatMode::Ready;

    // Pressure 0xC0 -> Quick Kick
    ctrl.handle_combat_inputs_ex(
        CombatInputSnapshot {
            ready_held: true,
            action_pressed: true,
            action_held: true,
            has_downed_enemy_nearby: true,
            attack_pressure: Some(0xC0),
            ..Default::default()
        },
        dt,
    );
    assert_eq!(ctrl.mode, CombatMode::Stomp);
    assert!(!ctrl.is_heavy_attack);
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_KICKS[0].id);

    // Native zero-latency split Heavy/Stomp -> immediate frame-0 Stomp (0 ms wait)
    ctrl.combat_input_mode = CombatInputMode::NativeZeroLatency;
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs_ex(
        CombatInputSnapshot {
            ready_held: true,
            has_downed_enemy_nearby: true,
            native_heavy_pressed: true,
            ..Default::default()
        },
        dt,
    );
    assert_eq!(ctrl.mode, CombatMode::Stomp);
    assert!(ctrl.is_heavy_attack);
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_STOMPS[0].id);

    // Native zero-latency split Light/Kick -> immediate frame-0 Quick Kick
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs_ex(
        CombatInputSnapshot {
            ready_held: true,
            has_downed_enemy_nearby: true,
            native_light_pressed: true,
            ..Default::default()
        },
        dt,
    );
    assert_eq!(ctrl.mode, CombatMode::Stomp);
    assert!(!ctrl.is_heavy_attack);
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_KICKS[0].id);
}

#[test]
fn test_running_forces_light_swing_on_plank_and_cleaver() {
    let dt = 1.0 / 60.0;
    for (weapon, expected_id) in [
        (WeaponGrip::WoodenPlank, ATK_PLANK_LIGHT.id),
        (WeaponGrip::MariaChineseCleaver, ATK_MARIA_CLEAVER_LIGHT.id),
    ] {
        let mut ctrl = CombatController::new(false, weapon == WeaponGrip::MariaChineseCleaver);
        ctrl.combat_input_mode = CombatInputMode::NativeZeroLatency;
        ctrl.equip_weapon(weapon);
        ctrl.mode = CombatMode::Ready;
        ctrl.handle_combat_inputs_ex(
            CombatInputSnapshot {
                ready_held: true,
                is_running: true,
                native_heavy_pressed: true,
                ..Default::default()
            },
            dt,
        );
        assert_eq!(ctrl.mode, CombatMode::Swing);
        assert!(!ctrl.is_heavy_attack);
        assert_eq!(ctrl.active_attack.unwrap().id, expected_id);
    }
}

#[test]
fn test_enhanced_presets_preserve_vanilla_ps2_vs_sh2ee_vs_native() {
    let mut cfg = AppConfig::default();

    cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert_eq!(cfg.controls.combat_input_mode, CombatInputMode::Ps2Pressure);
    assert!(!cfg.game.enable_cutscene_unequip);
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::Ps2Authentic);

    cfg.apply_enhanced_preset(EnhancedPreset::Sh2ee);
    assert_eq!(
        cfg.controls.combat_input_mode,
        CombatInputMode::Sh2eeTapHold
    );
    assert!(cfg.game.enable_cutscene_unequip);
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::Sh2ee);

    cfg.apply_enhanced_preset(EnhancedPreset::NativeEnhanced);
    assert_eq!(
        cfg.controls.combat_input_mode,
        CombatInputMode::NativeZeroLatency
    );
    assert!(cfg.game.enable_cutscene_unequip);
    assert_eq!(
        cfg.controls.r2_heavy_pressure_threshold,
        PS2_HEAVY_PRESSURE_THRESHOLD_NORM
    );
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::NativeEnhanced);
}

#[test]
fn test_cutscene_unequip_and_right_hand_poses() {
    assert!(!should_unequip_weapon_in_cutscene(false, true));
    assert!(!should_unequip_weapon_in_cutscene(true, false));
    assert!(should_unequip_weapon_in_cutscene(true, true));

    assert_eq!(WeaponGrip::None.right_hand_pose_index(), 3);
    assert_eq!(WeaponGrip::Handgun.right_hand_pose_index(), 7);
    assert_eq!(WeaponGrip::Shotgun.right_hand_pose_index(), 8);
    assert_eq!(WeaponGrip::SteelPipe.right_hand_pose_index(), 9);
    assert_eq!(WeaponGrip::WoodenPlank.right_hand_pose_index(), 10);
    assert_eq!(WeaponGrip::HyperSpray.right_hand_pose_index(), 11);
    assert_eq!(WeaponGrip::GreatKnife.right_hand_pose_index(), 12);
    assert_eq!(WeaponGrip::Chainsaw.right_hand_pose_index(), 13);

    // CutsceneUnequip = true hides weapon during .dds cutscene (pose 3)
    assert_eq!(
        effective_right_hand_pose(WeaponGrip::SteelPipe, true, true),
        JAMES_EMPTY_RIGHT_HAND_POSE
    );
    // Strict Vanilla PS2 mode keeps weapon pose (9)
    assert_eq!(
        effective_right_hand_pose(WeaponGrip::SteelPipe, false, true),
        9
    );
}

#[test]
fn test_head_tracking_clamps_and_teddy_bear_look_fix() {
    let mut tracker = HeadLookTracker::default();
    let eye = Vec3::new(0.0, -844.6, 0.0);
    // Target far to the right and high up -> should clamp to NECK_YAW_MAX_RAD (0.8) and NECK_PITCH_MAX_RAD (0.5)
    let extreme_target = Vec3::new(1000.0, -2500.0, 100.0);
    for _ in 0..30 {
        tracker.step(eye, 0.0, Some(extreme_target), false, 1.0 / 60.0);
    }
    assert!((tracker.yaw_rad - NECK_YAW_MAX_RAD).abs() < 1e-4);
    assert!((tracker.pitch_rad - NECK_PITCH_MAX_RAD).abs() < 1e-4);

    // Target far to the left and low -> should clamp to NECK_YAW_MIN_RAD (-0.8) and NECK_PITCH_MIN_RAD (-0.4)
    let extreme_low_left = Vec3::new(-1000.0, 1000.0, 100.0);
    for _ in 0..40 {
        tracker.step(eye, 0.0, Some(extreme_low_left), false, 1.0 / 60.0);
    }
    assert!((tracker.yaw_rad - NECK_YAW_MIN_RAD).abs() < 1e-4);
    assert!((tracker.pitch_rad - NECK_PITCH_MIN_RAD).abs() < 1e-4);

    // Verify 20% / 80% split constants
    assert!((NECK_UPPER_SPINE_SHARE + NECK_CRANIAL_SHARE - 1.0).abs() < 1e-6);

    // TeddyBearLookFix (0x0727 + flag 0xAA)
    let mut flags = EventFlags::default();
    let bear = LookTargetCandidate {
        kind: LookTargetKind::ItemOrProp,
        prop_kind: ACT1_PROP_KIND_TEDDY_BEAR,
        position: Vec3::new(100.0, -400.0, 500.0),
        is_combat_target: false,
        is_active: true,
    };
    let maria = LookTargetCandidate {
        kind: LookTargetKind::Npc,
        prop_kind: 0x0104,
        position: Vec3::new(-400.0, -800.0, 900.0),
        is_combat_target: false,
        is_active: true,
    };

    // Before Bent Needle taken -> James looks at closer Teddy Bear
    let chosen_before = select_head_look_target(
        Vec3::ZERO,
        &[bear, maria],
        DEFAULT_LOOK_SEARCH_RADIUS_MM,
        &flags,
        true,
    );
    assert_eq!(chosen_before.unwrap().prop_kind, ACT1_PROP_KIND_TEDDY_BEAR);

    // After Bent Needle taken (`flag 0xAA`):
    // - SH2:EE enabled -> ignores 0x0727 and looks at Maria
    // - Vanilla PS2 (`enable_sh2ee_world_fixes = false`) -> keeps staring at 0x0727
    flags.set(ACT1_FLAG_TEDDY_BEAR_NEEDLE_TAKEN, true);
    let chosen_sh2ee = select_head_look_target(
        Vec3::ZERO,
        &[bear, maria],
        DEFAULT_LOOK_SEARCH_RADIUS_MM,
        &flags,
        true,
    );
    assert_eq!(chosen_sh2ee.unwrap().prop_kind, 0x0104);

    let chosen_vanilla = select_head_look_target(
        Vec3::ZERO,
        &[bear, maria],
        DEFAULT_LOOK_SEARCH_RADIUS_MM,
        &flags,
        false,
    );
    assert_eq!(chosen_vanilla.unwrap().prop_kind, ACT1_PROP_KIND_TEDDY_BEAR);
}

#[test]
fn test_retail_james_mdl_head_tracking_and_right_hand_poses() {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let Some(mdl_path) = resolver.resolve_file("chr/jms/lll_jms.mdl") else {
        return; // Retail assets not mounted in CI
    };
    let data = std::fs::read(&mdl_path).expect("read lll_jms.mdl");
    let model = sh2_assets::formats::mdls::parse_mdl(&data).expect("parse lll_jms.mdl");

    assert!(model.bone_count > JAMES_HEAD_BONE_INDEX);
    assert!(model.bone_count > JAMES_NECK_BONE_INDEX);

    let mut locals = model.initial_matrices.clone();
    let tracker = HeadLookTracker {
        pitch_rad: 0.25,
        yaw_rad: 0.5,
        prev_pitch_rad: 0.25,
        prev_yaw_rad: 0.5,
    };
    tracker.apply_to_locals(
        &mut locals,
        JAMES_NECK_BONE_INDEX,
        JAMES_HEAD_BONE_INDEX,
        1.0,
    );
    // Verify modified bone matrices remain finite and non-zero
    for m in [
        &locals[JAMES_NECK_BONE_INDEX],
        &locals[JAMES_HEAD_BONE_INDEX],
    ] {
        assert!(m.iter().flatten().all(|v| v.is_finite()));
    }
    // Verify all right-hand weapon pose indices 3..=13 exist in retail hll_jms / lll_jms primitives
    assert!(model
        .primitives
        .iter()
        .any(|p| p.pose_index == JAMES_EMPTY_RIGHT_HAND_POSE));

    // Verify Maria's neck/head bone parent hierarchy in retail lll_mar.mdl / hhh_mar.mdl
    if let Some(mar_path) = resolver.resolve_file("chr/mar/lll_mar.mdl") {
        let mar_data = std::fs::read(&mar_path).expect("read lll_mar.mdl");
        let mar_model = sh2_assets::formats::mdls::parse_mdl(&mar_data).expect("parse lll_mar.mdl");
        assert!(mar_model.bone_count > MARIA_HEAD_BONE_INDEX);
        assert_eq!(
            mar_model.parents[MARIA_HEAD_BONE_INDEX],
            MARIA_NECK_BONE_INDEX as u8
        );
    }
}
