//! SH2-170 1:1 parity tests for row-major bone matrices & post-FK evaluation order,
//! `Fall`/`WallF` clip mapping, freefall Y integration, multi-`.cld` outdoor grids & `disable`,
//! `frame_init` reset, `clCollectCharaALL` `move_vec`/`chara_type`/`post_flag_timer`, and
//! `LakeWaveSystem` ring traversal.

use glam::Vec3;
use sh2_assets::formats::anm::{evaluate_pose, AnimationFrame, AnimationTrackType, BoneTransform};
use sh2_assets::formats::cld::{CldFile, FloorTriangle, HitPolyPlane};
use sh2_core::animation::{
    AnimationStateKind, BlendMode, CharacterLocomotionClips, HeadLookTracker, JamesBasicClips,
    LocomotionAnimationSync,
};
use sh2_core::boat::LakeWaveSystem;
use sh2_core::collision::{
    CharacterCylinder, CharacterId, CollisionConfig, CollisionController, CollisionWorld,
    DynamicFloor, DynamicWall,
};
use sh2_core::locomotion::{
    LocomotionInput, LocomotionOutput, LocomotionState, PlayerLocomotionController,
    SH2_WORLD_UNIT_SCALE,
};
use sh2_core::secondary_physics::SecondaryBonePhysics;

#[test]
fn test_row_major_head_tracking_preserves_translation_and_applies_post_evaluate_pose() {
    // 3-bone chain: 0 (root) -> 1 (neck) -> 2 (head)
    let parents = [255u8, 0, 1];
    let bind_locals = vec![
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        [
            [1.0, 0.0, 0.0, 10.0],
            [0.0, 1.0, 0.0, -500.0],
            [0.0, 0.0, 1.0, 20.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, -150.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    ];

    let tracker = HeadLookTracker {
        pitch_rad: 0.25,
        yaw_rad: 0.5,
        prev_pitch_rad: 0.25,
        prev_yaw_rad: 0.5,
    };

    // 1. Verify `apply_to_locals` does NOT transpose or rotate local translation `m[0..3][3]`
    let mut locals = bind_locals.clone();
    tracker.apply_to_locals(&mut locals, 1, 2, 1.0);
    assert_eq!(locals[1][0][3], 10.0);
    assert_eq!(locals[1][1][3], -500.0);
    assert_eq!(locals[1][2][3], 20.0);
    assert_eq!(locals[1][3], [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(locals[2][0][3], 0.0);
    assert_eq!(locals[2][1][3], -150.0);
    assert_eq!(locals[2][2][3], 0.0);

    // 2. Verify `apply_to_pose` AFTER `evaluate_pose` modifies keyframed bones (which would have
    //    been overwritten if applied before `evaluate_pose`) and preserves neck world translation!
    let keyframed = AnimationFrame {
        bones: vec![
            None,
            Some(BoneTransform {
                rotation: Some([0.0, 0.0, 0.0]),
                translation: None,
                next_translation: None,
                axis_angle_delta: None,
                nibble_flag: false,
                track_type: AnimationTrackType::RotationOnly,
            }),
            Some(BoneTransform {
                rotation: Some([0.0, 0.0, 0.0]),
                translation: None,
                next_translation: None,
                axis_angle_delta: None,
                nibble_flag: false,
                track_type: AnimationTrackType::RotationOnly,
            }),
        ],
    };

    let base_pose = evaluate_pose(&parents, &bind_locals, &keyframed);
    let mut tracked_pose = base_pose.clone();
    tracker.apply_to_pose(&mut tracked_pose, &parents, 1, 2, 1.0);

    // Neck pivot translation must be preserved identically
    assert!((tracked_pose[1][0][3] - base_pose[1][0][3]).abs() < 1e-4);
    assert!((tracked_pose[1][1][3] - base_pose[1][1][3]).abs() < 1e-4);
    assert!((tracked_pose[1][2][3] - base_pose[1][2][3]).abs() < 1e-4);
    // Neck & head rotations must differ from un-tracked keyframed pose
    assert!(
        (tracked_pose[1][0][2] - base_pose[1][0][2]).abs() > 1e-3,
        "neck bone must be rotated post-evaluate_pose"
    );
    assert!(
        (tracked_pose[2][0][2] - base_pose[2][0][2]).abs() > 1e-2,
        "head bone must accumulate neck + head rotation post-evaluate_pose"
    );
    // Child head bone translation must pivot around neck (since neck pitched by 0.05 rad)
    assert!(
        (tracked_pose[2][2][3] - base_pose[2][2][3]).abs() > 1e-2,
        "child head bone must orbit neck pivot"
    );
}

#[test]
fn test_secondary_physics_row_major_post_pose_preserves_translation() {
    let parents = [255u8, 0];
    let bind_locals = vec![
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        [
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, -300.0],
            [0.0, 0.0, 1.0, -15.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    ];
    let mut sim = SecondaryBonePhysics::new();
    sim.register_bone(1, Default::default());
    sim.step(1.0 / 60.0, Vec3::new(0.0, 0.0, 500.0), 0.0);
    sim.step(1.0 / 60.0, Vec3::new(0.0, 0.0, 1000.0), 0.2);

    let mut pose = bind_locals.clone();
    sim.apply_to_pose_interpolated(&mut pose, &parents, 1.0);
    assert!((pose[1][0][3] - 5.0).abs() < 1e-4);
    assert!((pose[1][1][3] - (-300.0)).abs() < 1e-4);
    assert!((pose[1][2][3] - (-15.0)).abs() < 1e-4);
    assert_eq!(pose[1][3], [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn test_gait_sync_maps_fall_and_wall_f_to_non_looping_ps2_clips() {
    let clips = CharacterLocomotionClips::JAMES;

    let fall_out = LocomotionOutput {
        delta_position: Vec3::ZERO,
        turn_speed_rad_per_sec: 0.0,
        state: LocomotionState::Fall,
        linear_speed: 0.0,
        footstep_triggered: false,
        footstep_volume: 0.0,
    };
    let fall_cmd = LocomotionAnimationSync::resolve(AnimationStateKind::Run, &fall_out, &clips);
    assert_eq!(fall_cmd.state, AnimationStateKind::Fall);
    assert_eq!(fall_cmd.clip, JamesBasicClips::FALL);
    assert!(!fall_cmd.clip.looping);
    assert!(matches!(
        fall_cmd.blend_mode,
        BlendMode::PoseCrossfade { .. }
    ));

    let wall_f_out = LocomotionOutput {
        state: LocomotionState::WallF,
        ..fall_out
    };
    let wall_f_cmd =
        LocomotionAnimationSync::resolve(AnimationStateKind::Run3, &wall_f_out, &clips);
    assert_eq!(wall_f_cmd.state, AnimationStateKind::WallF);
    assert_eq!(wall_f_cmd.clip, JamesBasicClips::WALL_F);
    assert!(!wall_f_cmd.clip.looping);
    assert!(matches!(
        wall_f_cmd.blend_mode,
        BlendMode::PoseCrossfade { .. }
    ));
}

#[test]
fn test_locomotion_integrates_freefall_velocity_times_500() {
    let mut ctrl = PlayerLocomotionController::new();
    let mut yaw = 0.0;
    let input = LocomotionInput {
        fall_velocity: 0.04,
        ..Default::default()
    };
    let out = ctrl.update(&input, &mut yaw, 1.0 / 60.0, true);
    let expected_y = 0.04 * SH2_WORLD_UNIT_SCALE;
    assert!(
        (out.delta_position.y - expected_y).abs() < 1e-4,
        "expected delta_position.y = {expected_y}, got {}",
        out.delta_position.y
    );
}

#[test]
fn test_collision_world_merge_preserves_outdoor_grids_and_disable_flag() {
    // Sector A: outdoor block at origin (40000, 40000), enabled (`disable = 0`), with wall in cell 0
    let mut cld_a = CldFile {
        origin: [40000.0, 40000.0],
        disable: 0,
        ..Default::default()
    };
    let wall_a = HitPolyPlane::from_wall_segment(&sh2_assets::formats::cld::WallSegment {
        start: [40500.0, 40000.0],
        end: [40500.0, 42000.0],
        y_top: -3000.0,
        y_bottom: 100.0,
        material: 0,
        group: 1,
    });
    cld_a.raw_walls.push(wall_a);
    cld_a.b1_cells[0].push(0);
    // Mark as outdoor by populating a second cell
    cld_a.b1_cells[5].push(0);
    let world_a = CollisionWorld::from_cld(&cld_a);
    assert!(world_a.grid.is_outdoor);

    // Sector B: disabled block (`disable = 1`) at origin (40000, 40000) with a floor at Y = -100
    let mut cld_b = CldFile {
        origin: [40000.0, 40000.0],
        disable: 1,
        ..Default::default()
    };
    cld_b.floors.push(FloorTriangle::new(
        [
            [40000.0, -100.0, 40000.0],
            [42000.0, -100.0, 40000.0],
            [42000.0, -100.0, 42000.0],
        ],
        0,
    ));
    cld_b.b0_cells[0].push(0);
    let world_b = CollisionWorld::from_cld(&cld_b);

    let merged = CollisionWorld::merge(&[&world_a, &world_b]);
    assert_eq!(merged.sectors.len(), 2);
    assert!(merged.sectors[0].grid.is_outdoor);
    assert_eq!(merged.sectors[0].origin, [40000.0, 40000.0]);
    assert_eq!(merged.sectors[1].disable, 1);

    // Disabled sector B's floor must NOT be sampled
    let floor = merged.sample_floor_height(40300.0, 40100.0, 0.0, 250.0, 1500.0);
    assert!(floor.is_none(), "disabled .cld sector must be skipped");

    // Enabled outdoor sector A's wall at X = 40500 must push back a cylinder at X = 40400
    let cfg = CollisionConfig::default();
    let resolved = merged.resolve_character_movement(
        Vec3::new(40300.0, 0.0, 41000.0),
        Vec3::new(40400.0, 0.0, 41000.0),
        &cfg,
    );
    assert!(
        resolved.x <= 40500.0 - cfg.radius + 0.2,
        "outdoor sector A wall must push back player: resolved.x={}",
        resolved.x
    );
}

#[test]
fn test_collision_controller_frame_init_and_move_vec_integration() {
    let world = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
    let mut ctrl = CollisionController::new(world, CollisionConfig::default());

    // Add dynamic wall, dynamic floor, extra wall, and character
    ctrl.world
        .add_dynamic_wall(DynamicWall::new(0.0, -100.0, 0.0, 100.0, -1000.0, 0.0, 0));
    ctrl.world.add_dynamic_floor(DynamicFloor::new(
        [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 0.0, 100.0]],
        0,
    ));
    ctrl.register_character(
        CharacterCylinder::james(CharacterId(1), Vec3::new(800.0, 0.0, 0.0))
            .with_chara_type(1)
            .with_post_flag_timer(3),
    );

    // Verify `update_character_kinematics` integrates `move_vec` inside `clCollectCharaALL`
    let resolved = ctrl.update_character_kinematics(CharacterId(1), Vec3::new(980.0, 0.0, 0.0));
    assert!(
        resolved.x <= 1000.0 - ctrl.config.radius + 0.2,
        "clCollectCharaALL must clamp east wall via move_vec: x={}",
        resolved.x
    );

    // Verify `frame_init` (`0x00132120`) resets all per-frame lists
    ctrl.frame_init();
    assert!(ctrl.world.extra_walls.is_empty());
    assert!(ctrl.world.dynamic_walls.is_empty());
    assert!(ctrl.world.dynamic_floors.is_empty());
    assert!(ctrl.characters.is_empty());
}

#[test]
fn test_lake_wave_system_includes_newest_wave_ring() {
    let mut waves = LakeWaveSystem::new();
    // Place boat right at the wave origin (dist = 0.0):
    // Ring 0 (`newest_wave = 0`) has `pos = 0.0, width = 3.0` -> `pos - width = -3.0 < 0.0 <= 3.0`.
    // Previously `update` skipped `newest_wave` and returned `None`!
    let boat_at_origin = Vec3::new(-20.0 / 0.002, 0.0, -120.0 / 0.002);
    let sample = waves
        .update(boat_at_origin, 0.0)
        .expect("LakeWaveSystem::update must check newest_wave ring first (0x0026f050)");
    assert!(sample.height > 0.0);
    assert_eq!(waves.rand_seed, 1);
}
