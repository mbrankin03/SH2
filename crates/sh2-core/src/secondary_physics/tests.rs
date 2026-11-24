use super::*;
use std::path::Path;

#[test]
fn test_secondary_physics_presets_and_step() {
    let mut sim_james = SecondaryBonePhysics::for_james();
    assert!(sim_james.configs.contains_key(&11));
    assert!(sim_james.configs.contains_key(&12));
    assert!(sim_james.configs.contains_key(&16));
    assert!(sim_james.configs.contains_key(&17));
    assert!(sim_james.configs.contains_key(&23));
    assert!(sim_james.configs.contains_key(&24));

    let sim_maria = SecondaryBonePhysics::for_maria();
    assert!(sim_maria.configs.contains_key(&12));
    assert!(sim_maria.configs.contains_key(&17));
    assert!(sim_maria.configs.contains_key(&13));
    assert!(sim_maria.configs.contains_key(&14));

    let dt = 1.0 / 60.0;
    let p0 = Vec3::new(0.0, 0.0, 0.0);
    let p1 = Vec3::new(0.0, 0.0, 25.0);

    sim_james.step(dt, p0, 0.0);
    sim_james.step(dt, p1, 0.0);

    let st11 = sim_james.states.get(&11).unwrap();
    assert!(st11.deflection.x != 0.0 || st11.angular_velocity.x != 0.0);

    let mut locals = vec![[[0.0; 4]; 4]; 40];
    for loc in &mut locals {
        loc[0][0] = 1.0;
        loc[1][1] = 1.0;
        loc[2][2] = 1.0;
        loc[3][3] = 1.0;
    }

    sim_james.apply_to_locals(&mut locals);
    assert_ne!(locals[11][0][0], 0.0);
}

#[test]
fn test_secondary_physics_native_upgrade_toggle_gate() {
    use crate::config::{AppConfig, EnhancedPreset, GameOptions};

    assert!(!GameOptions::default().enable_secondary_physics);
    let mut cfg = AppConfig::default();
    cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert!(!cfg.game.enable_secondary_physics);
    cfg.apply_enhanced_preset(EnhancedPreset::NativeEnhanced);
    assert!(cfg.game.enable_secondary_physics);

    let mut sim = SecondaryBonePhysics::for_james();
    let dt = 1.0 / 60.0;
    sim.step_if_enabled(false, dt, Vec3::ZERO, 0.0);
    sim.step_if_enabled(false, dt, Vec3::new(0.0, 0.0, 50.0), 0.5);

    let mut locals = vec![
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        40
    ];
    let orig = locals.clone();
    sim.apply_to_locals_if_enabled(false, &mut locals, 1.0);
    assert_eq!(locals, orig);
}

#[test]
fn test_thigh_capsule_collision_james_and_maria() {
    let mut pose = vec![
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        36
    ];
    let parents = vec![255u8; 36];

    pose[4][0][3] = -52.0;
    pose[4][1][3] = -472.0;
    pose[4][2][3] = -30.0;
    pose[6][0][3] = -44.0;
    pose[6][1][3] = -258.0;
    pose[6][2][3] = -30.0;
    pose[23][0][3] = -52.0;
    pose[23][1][3] = -360.0;
    pose[23][2][3] = -20.0;

    let sim_james = SecondaryBonePhysics::for_james();
    sim_james.resolve_thigh_collisions_if_enabled(true, &mut pose, &parents);
    let pushed_pos = Vec3::new(pose[23][0][3], pose[23][1][3], pose[23][2][3]);
    let closest = closest_point_on_segment(
        pushed_pos,
        Vec3::new(-52.0, -472.0, -30.0),
        Vec3::new(-44.0, -258.0, -30.0),
    );
    assert!(pushed_pos.distance(closest) >= ThighCapsuleSpec::JAMES.radius_mm - 1e-3);

    pose[3][0][3] = -39.0;
    pose[3][1][3] = -461.0;
    pose[3][2][3] = -10.0;
    pose[7][0][3] = -36.0;
    pose[7][1][3] = -286.0;
    pose[7][2][3] = -2.0;
    pose[13][0][3] = -39.0;
    pose[13][1][3] = -363.0;
    pose[13][2][3] = -1.0;

    let sim_maria = SecondaryBonePhysics::for_maria();
    let before_maria = pose[13];
    sim_maria.resolve_thigh_collisions_if_enabled(false, &mut pose, &parents);
    assert_eq!(pose[13], before_maria);

    sim_maria.resolve_thigh_collisions_if_enabled(true, &mut pose, &parents);
    let pushed_maria = Vec3::new(pose[13][0][3], pose[13][1][3], pose[13][2][3]);
    let closest_m = closest_point_on_segment(
        pushed_maria,
        Vec3::new(-39.0, -461.0, -10.0),
        Vec3::new(-36.0, -286.0, -2.0),
    );
    assert!(pushed_maria.distance(closest_m) >= ThighCapsuleSpec::MARIA.radius_mm - 1e-3);
}

#[test]
fn test_retail_mdl_bind_pose_thigh_capsule_clearance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    assert!(
        root.exists(),
        "Retail assets directory must exist at {}",
        root.display()
    );

    let jms_bytes = std::fs::read(root.join("data/chr/jms/hll_jms.mdl")).unwrap();
    let jms = sh2_assets::formats::mdls::parse_mdl(&jms_bytes).unwrap();
    let mut jms_pose = jms.initial_matrices.clone();
    let sim_james = SecondaryBonePhysics::for_james();
    sim_james.resolve_thigh_collisions_if_enabled(true, &mut jms_pose, &jms.parents);
    assert_eq!(
        jms_pose, jms.initial_matrices,
        "Retail James bind pose must have clean thigh-capsule clearance at rest"
    );

    let mar_bytes = std::fs::read(root.join("data/chr/mar/lll_mar.mdl")).unwrap();
    let mar = sh2_assets::formats::mdls::parse_mdl(&mar_bytes).unwrap();
    let mut mar_pose = mar.initial_matrices.clone();
    let sim_maria = SecondaryBonePhysics::for_maria();
    sim_maria.resolve_thigh_collisions_if_enabled(true, &mut mar_pose, &mar.parents);
    assert_eq!(
        mar_pose, mar.initial_matrices,
        "Retail Maria bind pose must have clean thigh-capsule clearance at rest"
    );
}
