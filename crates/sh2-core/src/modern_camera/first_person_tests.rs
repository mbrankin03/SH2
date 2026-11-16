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

/// Signed slack (SH2 units) between the eye's forward offset from the head joint and the
/// envelope required for the bottom frustum edge to clear the neck opening (`>= 0` is clear).
fn clearance_slack(
    spec: &CharacterHeadRigSpec,
    eye: Vec3,
    head_joint: Vec3,
    look_yaw: f32,
    pitch: f32,
    fov_y: f32,
) -> f32 {
    let fwd = Vec3::new(look_yaw.sin(), 0.0, look_yaw.cos());
    let drop = spec.neck_lip_world_y(head_joint) - eye.y;
    (eye - head_joint).dot(fwd) - spec.min_clear_forward(drop, pitch + 0.5 * fov_y)
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
            // One-tick body-yaw lag at 0.05 rad/tick bounds the error to a few units.
            assert!(slack > -4.0, "tick {tick}: chest clearance slack {slack}");
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
