//! First-Person body-awareness tests (`SH2-93`): stationary mouse-turn body lock, natural neck
//! tilt, `63 deg` pitch clamp, neck-opening clearance envelope, and `.cld` wall pushback pitch cap.

use glam::Vec3;
use sh2_assets::formats::cld::{CldFile, WallSegment};

use super::*;
use crate::camera::CameraSnapshot;
use crate::collision::CollisionWorld;
use crate::scenario::{PlayableCharacter, ScenarioKind};
use crate::simulation::SimulationCore;

fn fp_input(character: PlayableCharacter, pos: Vec3, yaw: f32) -> ModernCameraTickInput {
    ModernCameraTickInput {
        character,
        player_pos: pos,
        player_yaw: yaw,
        linear_speed: 0.0,
        right_stick_x: 0.0,
        right_stick_y: 0.0,
        extra_yaw_delta_rad: 0.0,
        extra_pitch_delta_rad: 0.0,
        sensitivity: 1.0,
        ready_weapon: false,
        quick_turn_pressed: false,
        shoulder_swap_pressed: false,
        in_bug_room: false,
    }
}

/// Signed slack (SH2 units) between the steepest bottom-edge ray (`pitch + fov_y/2`) and the
/// neck-opening front lip plus margin, measured independently of the rig's own envelope
/// (`>= 0` means the view cannot dip inside the torso).
fn clearance_slack(
    spec: &CharacterHeadRigSpec,
    eye: Vec3,
    head_joint: Vec3,
    look_yaw: f32,
    pitch: f32,
    fov_y: f32,
) -> f32 {
    let fwd = Vec3::new(look_yaw.sin(), 0.0, look_yaw.cos());
    let drop = (spec.neck_lip_world_y(head_joint) - eye.y).max(0.0);
    let depression = pitch + 0.5 * fov_y;
    let ray_fwd_at_lip = drop * depression.cos() / depression.sin();
    (eye - head_joint).dot(fwd) + ray_fwd_at_lip
        - (spec.neck_opening_radius + NECK_OPENING_CLEAR_MARGIN_UNITS)
}

fn wrap_angle(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[test]
fn test_fp_max_pitch_clamps_to_natural_63_degrees_and_clears_neck_opening() {
    let collision = CollisionWorld::create_synthetic_room(8000.0, 8000.0, 2000.0, 0.0, 0.0);
    let cfg = SpringArmConfig::default();
    for (ch, pos, yaw) in [
        (PlayableCharacter::James, Vec3::ZERO, 0.0_f32),
        (PlayableCharacter::James, Vec3::new(120.0, 0.0, -80.0), -2.0),
        (PlayableCharacter::Maria, Vec3::new(-50.0, 0.0, 60.0), 2.4),
    ] {
        for fov_deg in [55.0_f32, 75.0, 100.0] {
            let mut rig = FirstPersonCameraRig::default();
            rig.config.fov_y_degrees = fov_deg;
            rig.reset_to_player(ch, pos, yaw);
            let mut inp = fp_input(ch, pos, yaw);
            inp.extra_pitch_delta_rad = 5.0;
            let (snap, _, dir, _) = rig.tick(&inp, &collision, &cfg);
            assert!((rig.look_pitch - 1.10).abs() < 1e-5, "max pitch clamp");
            assert!(
                dir.y > 0.85 && dir.y < 0.90,
                "~63 deg downward view {dir:?}"
            );

            let spec = CharacterHeadRigSpec::for_character(ch);
            let head = spec.resolve_world_head_joint(pos, yaw, None);
            let slack = clearance_slack(&spec, snap.eye, head, yaw, rig.look_pitch, snap.fov_y);
            assert!(slack >= -1e-3, "{ch:?} fov {fov_deg}: slack {slack}");

            // Natural nod: eye sits forward of and lower than the level-gaze eye.
            let level = spec.resolve_world_eye_anchor(pos, yaw, None);
            let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            assert!((snap.eye - level).dot(fwd) > 8.0);
            assert!(snap.eye.y > level.y + 15.0);
        }
    }
}

#[test]
fn test_fp_stationary_mouse_turn_keeps_body_locked_and_chest_clear() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.collision = CollisionWorld::create_synthetic_room(8000.0, 8000.0, 2000.0, 0.0, 0.0);
    sim.snap_player_transform(Vec3::ZERO, 0.0);
    sim.modern_camera.active_mode = GameplayCameraMode::FirstPerson;
    sim.modern_camera.reset_to_player(
        PlayableCharacter::James,
        Vec3::ZERO,
        0.0,
        &CameraSnapshot::default(),
    );
    let spec = CharacterHeadRigSpec::JAMES;

    // No WASD / left stick at all: only mouse yaw + downward pitch, for > 1 full revolution.
    for tick in 0..160 {
        sim.input.accumulate_mouse_look(0.05, 0.02);
        sim.tick();
        let fp = &sim.modern_camera.first_person;
        let yaw_err = wrap_angle(sim.scenario.player_yaw - fp.look_yaw);
        assert!(
            yaw_err.abs() < 1e-5,
            "tick {tick}: player_yaw must track look_yaw when standing still (err {yaw_err})"
        );
        assert!(sim.scenario.player_position.length() < 1e-3, "no drift");

        let frame = sim.modern_camera.curr_frame;
        if fp.look_pitch > 0.0 {
            let head = spec.resolve_world_head_joint(Vec3::ZERO, fp.look_yaw, None);
            let slack = clearance_slack(
                &spec,
                frame.snapshot.eye,
                head,
                fp.look_yaw,
                fp.look_pitch,
                1.309,
            );
            assert!(slack >= -1e-3, "tick {tick}: chest clearance slack {slack}");
        }
    }
    assert!((sim.modern_camera.first_person.look_pitch - 1.10).abs() < 1e-4);
    assert_eq!(
        sim.player_mesh_visibility(),
        PlayerMeshVisibility::HideHeadOnly
    );
}

#[test]
fn test_fp_wall_pushback_caps_pitch_instead_of_seeing_inside_torso() {
    // Thin full-height wall 30 units in front of James's root: `.cld` containment drags the eye
    // back behind the collar line, so the rig must cap downward pitch rather than look inside.
    let mut room = CldFile::create_synthetic_room(6000.0, 6000.0, 1800.0, 0.0, 0.0);
    room.walls.push(WallSegment::new(
        [-1000.0, 30.0],
        [1000.0, 30.0],
        -1800.0,
        100.0,
        1,
        0,
    ));
    let collision = CollisionWorld::from_cld(&room);
    let cfg = SpringArmConfig::default();
    let mut rig = FirstPersonCameraRig::default();
    rig.reset_to_player(PlayableCharacter::James, Vec3::ZERO, 0.0);
    let mut inp = fp_input(PlayableCharacter::James, Vec3::ZERO, 0.0);
    inp.extra_pitch_delta_rad = 5.0;

    let (snap, _, _, _) = rig.tick(&inp, &collision, &cfg);
    assert!(
        snap.eye.z < 30.0,
        "eye stays inside the room: {:?}",
        snap.eye
    );
    assert!(rig.look_pitch < 1.10, "pitch capped: {}", rig.look_pitch);
    assert!(rig.look_pitch >= 0.0, "never forced to look up");

    let spec = CharacterHeadRigSpec::JAMES;
    let head = spec.resolve_world_head_joint(Vec3::ZERO, 0.0, None);
    let slack = clearance_slack(&spec, snap.eye, head, 0.0, rig.look_pitch, snap.fov_y);
    assert!(
        slack >= -1e-2,
        "capped view still clears neck opening: {slack}"
    );
}

/// Synthetic 41-bone James pose: identity rotations with the head joint at its retail bind position.
fn james_bind_head_pose() -> Vec<[[f32; 4]; 4]> {
    let id = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut pose = vec![id; 41];
    let head = CharacterHeadRigSpec::JAMES.fallback_model_head;
    pose[15][0][3] = head.x;
    pose[15][1][3] = head.y;
    pose[15][2][3] = head.z;
    pose
}

fn first_person_sim() -> SimulationCore {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.collision = CollisionWorld::create_synthetic_room(20000.0, 20000.0, 2400.0, 0.0, 0.0);
    sim.snap_player_transform(Vec3::ZERO, 0.0);
    sim.modern_camera.active_mode = GameplayCameraMode::FirstPerson;
    sim.modern_camera.reset_to_player(
        PlayableCharacter::James,
        Vec3::ZERO,
        0.0,
        &CameraSnapshot::default(),
    );
    sim
}

/// Asserts the body faces the view and the eye clears the neck stump of the body rendered at the
/// *end-of-tick* `(player_position, player_yaw)` (zero positional / yaw lag).
fn assert_eye_locked_to_rendered_body(sim: &SimulationCore, tag: &str) {
    let fp = &sim.modern_camera.first_person;
    let yaw = sim.scenario.player_yaw;
    let yaw_err = wrap_angle(yaw - fp.look_yaw);
    assert!(
        yaw_err.abs() < 1e-5,
        "{tag}: player_yaw vs look_yaw err {yaw_err}"
    );

    let spec = CharacterHeadRigSpec::JAMES;
    let pos = sim.scenario.player_position;
    let snap = sim.modern_camera.curr_frame.snapshot;
    let head = spec.resolve_world_head_joint(pos, yaw, None);
    let slack = clearance_slack(&spec, snap.eye, head, yaw, fp.look_pitch, snap.fov_y);
    assert!(
        slack >= -1e-2,
        "{tag}: eye lags the rendered neck stump (slack {slack:.2})"
    );
    let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let ahead = (snap.eye - head).dot(fwd);
    assert!(
        ahead >= spec.neck_opening_radius + NECK_OPENING_CLEAR_MARGIN_UNITS - 1e-2,
        "{tag}: eye only {ahead:.1} units ahead of the rendered head joint"
    );
}

#[test]
fn test_fp_running_strafing_flicks_and_quick_turn_have_zero_eye_lag() {
    for start_pitch in [0.0_f32, 5.0] {
        let mut sim = first_person_sim();
        let pose = james_bind_head_pose();
        sim.input.accumulate_mouse_look(0.0, start_pitch);

        // 1. `W + Shift` Run3 sprint for 60 ticks, feeding the pose BEFORE each tick like `render.rs`.
        sim.input
            .set_host_keyboard_state(0.0, 1.0, 0.0, 0.0, true, false);
        for tick in 0..60 {
            sim.set_player_skeleton_pose(Some(&pose));
            sim.tick();
            assert_eye_locked_to_rendered_body(&sim, &format!("run tick {tick} p{start_pitch}"));
        }
        assert!(
            sim.scenario.player_position.length() > 800.0,
            "sprint must actually move James: {:?}",
            sim.scenario.player_position
        );

        // 2. `W + D` diagonal strafe (body pre-aligned to look_yaw + 45 deg during locomotion).
        sim.input
            .set_host_keyboard_state(1.0, 1.0, 0.0, 0.0, false, false);
        for tick in 0..40 {
            sim.set_player_skeleton_pose(Some(&pose));
            sim.tick();
            assert_eye_locked_to_rendered_body(&sim, &format!("W+D tick {tick} p{start_pitch}"));
        }

        // 3. Single-frame 90 deg mouse flick while still strafing.
        sim.input
            .accumulate_mouse_look(std::f32::consts::FRAC_PI_2, 0.0);
        sim.set_player_skeleton_pose(Some(&pose));
        sim.tick();
        assert_eye_locked_to_rendered_body(&sim, &format!("90 deg flick p{start_pitch}"));

        // 4. `Q` quick-turn from a standstill: body follows the camera through the whole 180 deg.
        sim.input
            .set_host_keyboard_state(0.0, 0.0, 0.0, 0.0, false, false);
        let yaw_before = sim.modern_camera.first_person.look_yaw;
        sim.input.press_quick_turn();
        for tick in 0..45 {
            sim.set_player_skeleton_pose(Some(&pose));
            sim.tick();
            let tag = format!("quick-turn tick {tick} p{start_pitch}");
            assert_eye_locked_to_rendered_body(&sim, &tag);
        }
        let turned = wrap_angle(sim.modern_camera.first_person.look_yaw - yaw_before);
        assert!(
            (turned.abs() - std::f32::consts::PI).abs() < 1e-2,
            "turned {turned}"
        );
    }
}
