use super::*;
use crate::collision::CollisionConfig;
use sh2_assets::formats::cld::FloorTriangle;
use std::path::Path;

fn identity_pose_from_positions(positions: &[Vec3]) -> Vec<[[f32; 4]; 4]> {
    positions
        .iter()
        .map(|p| {
            [
                [1.0, 0.0, 0.0, p.x],
                [0.0, 1.0, 0.0, p.y],
                [0.0, 0.0, 1.0, p.z],
                [0.0, 0.0, 0.0, 1.0],
            ]
        })
        .collect()
}

fn james_test_skeleton() -> (Vec<u8>, Vec<[[f32; 4]; 4]>) {
    let mut parents = vec![255u8; 31];
    parents[4] = 1;
    parents[6] = 4;
    parents[13] = 6;
    parents[25] = 13;
    parents[29] = 25;
    parents[5] = 1;
    parents[7] = 5;
    parents[14] = 7;
    parents[26] = 14;
    parents[30] = 26;

    let mut positions = vec![Vec3::ZERO; 31];
    positions[0] = Vec3::new(0.0, -562.01, -21.35);
    positions[1] = Vec3::new(0.0, -562.01, -21.35);
    positions[4] = Vec3::new(-52.11, -472.44, -31.48);
    positions[6] = Vec3::new(-44.38, -257.55, -23.96);
    positions[13] = Vec3::new(-43.55, -235.95, -26.88);
    positions[25] = Vec3::new(-36.38, -52.81, -50.17);
    positions[29] = Vec3::new(-40.47, 0.31, -0.11);
    positions[5] = Vec3::new(52.11, -472.44, -31.48);
    positions[7] = Vec3::new(44.40, -257.60, -22.53);
    positions[14] = Vec3::new(43.57, -236.02, -25.52);
    positions[26] = Vec3::new(36.38, -52.81, -50.17);
    positions[30] = Vec3::new(40.47, 0.31, -0.11);
    (parents, identity_pose_from_positions(&positions))
}

#[test]
fn test_two_bone_law_of_cosines_golden_vector() {
    let (hip_a, knee_b) = solve_two_bone_ik_angles(300.0, 400.0, 500.0);
    assert!((knee_b - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    assert!((hip_a - (400.0_f32 / 500.0).asin()).abs() < 1e-4);
}

#[test]
fn test_foot_ik_on_stair_step_and_slope() {
    let (parents, mut pose) = james_test_skeleton();
    let orig_left_ankle_y = bone_translation(&pose[25]).y;
    let orig_right_ankle_y = bone_translation(&pose[26]).y;

    let mut world = CollisionWorld::default();
    world.floors.push(FloorTriangle::new(
        [
            [-300.0, -120.0, -300.0],
            [-1.0, -120.0, -300.0],
            [-1.0, -120.0, 300.0],
        ],
        0,
    ));
    world.floors.push(FloorTriangle::new(
        [
            [-300.0, -120.0, -300.0],
            [-1.0, -120.0, 300.0],
            [-300.0, -120.0, 300.0],
        ],
        0,
    ));
    world.floors.push(FloorTriangle::new(
        [
            [1.0, 80.0, -300.0],
            [300.0, 80.0, -300.0],
            [300.0, 80.0, 300.0],
        ],
        0,
    ));
    world.floors.push(FloorTriangle::new(
        [
            [1.0, 80.0, -300.0],
            [300.0, 80.0, 300.0],
            [1.0, 80.0, 300.0],
        ],
        0,
    ));
    let ctrl = CollisionController::new(world, CollisionConfig::default());

    let mut solver = FootIkSolver::for_james();
    // dt = 1.0 converges in one step (alpha = clamp(1.0 * 20.0, 0, 1) = 1.0)
    solver.solve_into(&ctrl, Vec3::ZERO, 0.0, 1.0, &parents, &mut pose);

    assert!((solver.smoothed_pelvis_y - 80.0).abs() < 1e-3);
    let new_left_ankle_y = bone_translation(&pose[25]).y;
    let new_right_ankle_y = bone_translation(&pose[26]).y;
    assert!((new_left_ankle_y - (orig_left_ankle_y - 120.0)).abs() < 2.0);
    assert!((new_right_ankle_y - (orig_right_ankle_y + 80.0)).abs() < 2.0);
}

#[test]
fn test_foot_ik_pause_dt_zero_holds_smoothed_pose() {
    let (parents, mut pose) = james_test_skeleton();
    let world = CollisionWorld::default();
    let mut solver = FootIkSolver::for_james();
    solver.smoothed_pelvis_y = 45.0;
    solver.smoothed_left_y = -25.0;
    solver.smoothed_right_y = 45.0;

    // With dt = 0.0 (paused), smoothed state must be preserved (no snap to 0.0)
    solver.solve_if_enabled(true, &world, Vec3::ZERO, 0.0, 0.0, &parents, &mut pose);
    assert_eq!(solver.smoothed_pelvis_y, 45.0);
    assert_eq!(solver.smoothed_left_y, -25.0);
    assert_eq!(solver.smoothed_right_y, 45.0);
}

#[test]
fn test_foot_ik_toggle_off_restores_vanilla_ps2() {
    let (parents, orig_pose) = james_test_skeleton();
    let mut pose = orig_pose.clone();
    let world = CollisionWorld::default();
    let mut solver = FootIkSolver::for_james();
    solver.smoothed_pelvis_y = 42.0;

    solver.solve_if_enabled(
        false,
        &world,
        Vec3::ZERO,
        0.0,
        1.0 / 60.0,
        &parents,
        &mut pose,
    );
    assert_eq!(pose, orig_pose);
    assert_eq!(solver.smoothed_pelvis_y, 0.0);
}

#[test]
fn test_retail_james_and_maria_mdl_foot_ik_chains() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    assert!(
        root.exists(),
        "Retail assets directory must exist at {}",
        root.display()
    );

    let jms_bytes = std::fs::read(root.join("data/chr/jms/hll_jms.mdl")).unwrap();
    let jms = sh2_assets::formats::mdls::parse_mdl(&jms_bytes).unwrap();
    assert!(FootIkRigSpec::JAMES.matches_parents(&jms.parents));

    let mar_bytes = std::fs::read(root.join("data/chr/mar/lll_mar.mdl")).unwrap();
    let mar = sh2_assets::formats::mdls::parse_mdl(&mar_bytes).unwrap();
    assert!(FootIkRigSpec::MARIA.matches_parents(&mar.parents));

    let cld_bytes = std::fs::read(root.join("data/bg/er/er01.cld")).unwrap();
    let cld = sh2_assets::formats::cld::parse_cld(&cld_bytes).unwrap();
    let world = CollisionWorld::from_cld(&cld);
    let mut pose = jms.initial_matrices.clone();
    let mut solver = FootIkSolver::for_james();
    solver.solve_into_world(&world, Vec3::ZERO, 0.0, 1.0, &jms.parents, &mut pose);
    assert!(pose[25][1][3].is_finite());
    assert!(pose[26][1][3].is_finite());
}
