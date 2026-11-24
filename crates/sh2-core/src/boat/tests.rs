//! Deterministic golden-vector tests for Toluca Lake rowboat physics, oar animation sync,
//! lake waves, water grid simulation, SH2:EE fixes, and `PlayingInfo` integration.

use glam::Vec3;
use std::f32::consts::PI;

use super::*;
use crate::endings::PlayingInfo;

#[test]
fn golden_lake_wave_init_and_sample() {
    let mut waves = LakeWaveSystem::new();
    assert_eq!(waves.rings.len(), LAKE_WAVE_COUNT);
    assert!((waves.rings[0].pos - 0.0).abs() < 1e-6);
    assert!((waves.rings[0].width - 3.0).abs() < 1e-6);
    assert!((waves.rings[0].power - 0.5).abs() < 1e-6);
    assert!((waves.rings[1].pos - 12.0).abs() < 1e-5);
    assert!((waves.rings[1].width - 3.12).abs() < 1e-5);
    assert!((waves.rings[1].power - 0.44).abs() < 1e-5);

    // Place boat right on ring 1's radius (`12.0` wave units from `(-20, -120)` -> `(-20, -108) * 500`)
    let boat_pos = Vec3::new(-20.0 * 500.0, 3690.0, (-120.0 + 12.0) * 500.0);
    let sample = waves.update(boat_pos, 0.0).expect("must hit ring 1");
    // At exact center of ring 1, cos(0) = 1.0 -> height = 2 * power = 0.88
    assert!(
        (sample.height - 0.88).abs() < 1e-4,
        "height={}",
        sample.height
    );
    assert!(sample.angle.abs() < 1e-4);
}

#[test]
fn golden_boat_control_damping_deadband_and_pc_clamp() {
    let mut boat = RowboatController::new(Vec3::ZERO, 0.0);
    // Drive forward at full power for 300 ticks (5 seconds)
    let input = RowboatInput {
        left_trigger: 1.0,
        right_trigger: 1.0,
        enable_analog_trigger_rowing: true,
        full_power_mode: true,
        current_boat_anim_id: 0x20,
        ..Default::default()
    };
    let mut info = PlayingInfo::default();
    for _ in 0..300 {
        boat.tick_full(&input, Some(&mut info), true);
    }
    // Retail PC max forward speed is 3.5 (SLUS prototype was 4.0)
    assert!(
        (boat.forward_speed - BOAT_MAX_FORWARD_SPEED_PC).abs() < 1e-4,
        "forward_speed={}",
        boat.forward_speed
    );
    assert!((boat.position.y - 3690.0).abs() < 50.0);
    assert!((info.boat_clear_time - 5.0).abs() < 1e-4);
    assert!((info.boat_max_speed - BOAT_MAX_FORWARD_SPEED_PC).abs() < 1e-4);

    // Release input and verify deadband snap-to-zero
    let idle = RowboatInput {
        current_boat_anim_id: 0x20,
        ..Default::default()
    };
    for _ in 0..2000 {
        boat.tick_full(&idle, None, true);
    }
    assert_eq!(boat.forward_speed, 0.0);
    assert_eq!(boat.angular_velocity, 0.0);
}

#[test]
fn golden_boat_3d_dual_stick_circular_rowing_and_anim_sync() {
    let mut boat = RowboatController::new(Vec3::ZERO, 0.0);
    let mut out = RowboatTickOutput::default();
    // Simulate circular stick rotation on both sticks (CCW left, CW right = classic PS2 forward rowing)
    for i in 0..120 {
        let angle = (i as f32) * 0.15;
        let input = RowboatInput {
            left_stick_x: angle.cos(),
            left_stick_y: angle.sin(),
            right_stick_x: (-angle).cos(),
            right_stick_y: (-angle).sin(),
            control_mode: RowboatControlMode::DualStick3D,
            full_power_mode: true,
            current_boat_anim_id: 0x20,
            ..Default::default()
        };
        out = boat.tick_full(&input, None, true);
    }
    assert!(boat.forward_speed > 0.5, "boat should move forward");
    assert_eq!(out.james_anim_id, 0x4e24);
    assert!(boat.oar_anim_frame <= RowboatController::MAX_OAR_STROKE_FRAME);
}

#[test]
fn golden_sh2ee_rowboat_spawn_and_anim_fix() {
    let default_pos = Vec3::new(0.0, 3690.0, 0.0);
    // Strict Vanilla PS2 mode keeps original spawn
    let (pos_ps2, yaw_ps2) = resolve_sh2ee_rowboat_spawn(true, false, default_pos, 0.0);
    assert_eq!(pos_ps2, default_pos);
    assert_eq!(yaw_ps2, 0.0);

    // SH2:EE mode spawns at Hotel dock when flag 0x17B is set
    let (pos_ee, yaw_ee) = resolve_sh2ee_rowboat_spawn(true, true, default_pos, 0.0);
    assert_eq!(pos_ee, ROWBOAT_HOTEL_DOCK_POS_MM);
    assert!((yaw_ee - PI).abs() < 1e-6);
}

#[test]
fn golden_water_grid_and_ps2_specular() {
    let mut grid = WaterGridSim::default();
    grid.add_impulse(24, 24, 1.0);
    grid.step();
    // Neighbors of (24, 24) should now have non-zero wave height
    assert!(grid.heights()[23 + 24 * WATER_GRID_SIZE] > 0.0);
    assert!(grid.heights()[25 + 24 * WATER_GRID_SIZE] > 0.0);

    // Specular_Calculator: normal = +Y, light = +Y, view = +Y -> cos = 1.0 -> specular = 1.0
    let s1 = ps2_water_specular(Vec3::Y, Vec3::Y, Vec3::Y);
    assert!((s1 - 1.0).abs() < 1e-5);
    // Beyond 20 degrees cutoff -> 0.0
    let angled = Vec3::new(0.5, 0.8660254, 0.0).normalize();
    assert_eq!(ps2_water_specular(angled, Vec3::Y, Vec3::Y), 0.0);
}

#[test]
fn retail_boat_mdl_anm_and_jms_boat_anm_parse_cleanly() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr");
    if !base.exists() {
        eprintln!("Skipping retail boat asset test: retail data not present");
        return;
    }

    let boat_mdl = std::fs::read(base.join("boat/boat.mdl")).expect("read boat.mdl");
    let model = sh2_assets::parse_mdl(&boat_mdl).expect("parse boat.mdl");
    assert!(model.bone_count > 0, "boat.mdl must have skeleton bones");

    let boat_anm = std::fs::read(base.join("boat/boat.anm")).expect("read boat.anm");
    let anm =
        sh2_assets::parse_anm(&boat_anm, model.bone_count, &model.parents).expect("parse boat.anm");
    assert!(
        !anm.frames.is_empty(),
        "boat.anm must have animation frames"
    );

    let jms_mdl = std::fs::read(base.join("jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let jms_model = sh2_assets::parse_mdl(&jms_mdl).expect("parse hhh_jms.mdl");
    let jms_boat_anm = std::fs::read(base.join("jms/jms_boat.anm")).expect("read jms_boat.anm");
    let jms_anm = sh2_assets::parse_anm(&jms_boat_anm, jms_model.bone_count, &jms_model.parents)
        .expect("parse jms_boat.anm");
    assert!(
        jms_anm.frames.len() >= 0x4C,
        "jms_boat.anm must cover 0x4C (76) frames for RowboatAnimationFix"
    );

    let boat_kg1 = std::fs::read(base.join("boat/boat.kg1")).expect("read boat.kg1");
    assert_eq!(
        boat_kg1.len(),
        0,
        "retail boat.kg1 is an empty 0-byte placeholder"
    );
}
