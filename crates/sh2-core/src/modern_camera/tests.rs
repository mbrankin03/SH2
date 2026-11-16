//! Unit and retail `.cld` / `.mdl` integration tests for `modern_camera` (`SH2-93`).

use glam::Vec3;
use sh2_assets::formats::cld::{parse_cld, CldFile, WallSegment};
use sh2_assets::formats::demo::parse_demo_dds;
use sh2_assets::formats::mdls::parse_mdl;

use super::*;
use crate::camera::CameraSnapshot;
use crate::collision::CollisionWorld;
use crate::cutaway::{CutawayOverlayEngine, OverlayKind};
use crate::cutscene::{find_drama_demo_by_id, DramaDemoPlayer};
use crate::events::EventFlags;
use crate::scenario::PlayableCharacter;
use crate::timing::TimingConstants;

fn default_tick_input(pos: Vec3, yaw: f32) -> ModernCameraTickInput {
    ModernCameraTickInput {
        character: PlayableCharacter::James,
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

fn retail_root() -> &'static str {
    "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"
}

#[test]
fn test_r3_mode_cycling_presets_and_smooth_crossfade() {
    let mut rig = ModernCameraRig::new();
    let rail = CameraSnapshot::default();
    let pos = Vec3::ZERO;
    let collision = CollisionWorld::create_synthetic_room(8000.0, 8000.0, 2000.0, 0.0, 0.0);
    let timing = TimingConstants::default();

    assert_eq!(rig.active_mode, GameplayCameraMode::ClassicPs2Rail);

    // 1. Default AllThree cycle: ClassicPs2Rail -> OverTheShoulder -> FirstPerson -> ClassicPs2Rail
    assert_eq!(
        rig.cycle_mode_r3(&rail, PlayableCharacter::James, pos, 0.0),
        GameplayCameraMode::OverTheShoulder
    );
    assert!(matches!(
        rig.transition,
        ModeTransitionState::Crossfading { .. }
    ));

    let inp = default_tick_input(pos, 0.0);
    let f_mid = rig.tick(&inp, &rail, &collision, &timing);
    assert!(f_mid.snapshot.eye.distance(rail.eye) > 1.0);
    for _ in 0..20 {
        rig.tick(&inp, &rail, &collision, &timing);
    }
    assert_eq!(rig.transition, ModeTransitionState::Settled);

    assert_eq!(
        rig.cycle_mode_r3(&rail, PlayableCharacter::James, pos, 0.0),
        GameplayCameraMode::FirstPerson
    );
    assert_eq!(
        rig.cycle_mode_r3(&rail, PlayableCharacter::James, pos, 0.0),
        GameplayCameraMode::ClassicPs2Rail
    );

    // 2. ClassicAndOts subset skips FirstPerson
    rig.cycle_preset = CameraCyclePreset::ClassicAndOts;
    assert_eq!(
        rig.cycle_mode_r3(&rail, PlayableCharacter::James, pos, 0.0),
        GameplayCameraMode::OverTheShoulder
    );
    assert_eq!(
        rig.cycle_mode_r3(&rail, PlayableCharacter::James, pos, 0.0),
        GameplayCameraMode::ClassicPs2Rail
    );
}

#[test]
fn test_ots_orbit_shoulder_swap_weapon_aim_and_flashlight_convergence() {
    let mut rig = ModernCameraRig::new();
    let rail = CameraSnapshot::default();
    let mut room = CldFile::create_synthetic_room(6000.0, 6000.0, 1800.0, 0.0, 0.0);
    // North target wall at Z = +1500 facing player at origin facing +Z (yaw = 0)
    room.walls.push(WallSegment::new(
        [-2000.0, 1500.0],
        [2000.0, 1500.0],
        -1800.0,
        100.0,
        1,
        0,
    ));
    let collision = CollisionWorld::from_cld(&room);
    let timing = TimingConstants::default();

    rig.active_mode = GameplayCameraMode::OverTheShoulder;
    rig.reset_to_player(PlayableCharacter::James, Vec3::ZERO, 0.0, &rail);

    let mut inp = default_tick_input(Vec3::ZERO, 0.0);
    let base = rig.tick(&inp, &rail, &collision, &timing);
    // Right shoulder at yaw = 0 places camera at +X > 0 and behind player (Z < 0)
    assert!(base.snapshot.eye.x > 80.0);
    assert!(base.snapshot.eye.z < -400.0);
    assert_eq!(base.reticle, OtsReticleState::Hidden);

    // Toggle to Left Shoulder via L3 and hold Ready Weapon + pitch/yaw mouse/gyro input
    inp.shoulder_swap_pressed = true;
    inp.ready_weapon = true;
    inp.extra_yaw_delta_rad = 0.12;
    inp.extra_pitch_delta_rad = -0.08;
    rig.tick(&inp, &rail, &collision, &timing);
    inp.shoulder_swap_pressed = false;
    inp.extra_yaw_delta_rad = 0.0;
    inp.extra_pitch_delta_rad = 0.0;

    let mut aimed = base;
    for _ in 0..25 {
        aimed = rig.tick(&inp, &rail, &collision, &timing);
    }
    assert_eq!(rig.ots.shoulder_side, OtsShoulderSide::Left);
    assert!(
        aimed.snapshot.eye.x < -50.0,
        "Left shoulder shifts eye to -X"
    );
    assert!(
        aimed.snapshot.fov_y < base.snapshot.fov_y,
        "Aiming narrows FOV"
    );
    assert!(matches!(aimed.reticle, OtsReticleState::SubtleDot { .. }));
    // Flashlight & ranged aim ray follow the OTS pitched/yawed look direction (+X > 0, -Y < 0)
    assert!(aimed.aim_dir.x > 0.05);
    assert!(aimed.aim_dir.y < -0.02);
    assert!((aimed.flashlight_dir - aimed.aim_dir).length() < 1e-4);
}

#[test]
fn test_spring_arm_against_retail_woodside_and_brookhaven_cld_corridors() {
    let root = retail_root();
    let ap06_path = format!("{root}/data/bg/ap/ap06.cld");
    if !std::path::Path::new(&ap06_path).exists() {
        return;
    }

    // 1. Retail Woodside Apartments 2F Corridor (`ap06.cld`) & Lobby (`ap01.cld`)
    let ap06 = parse_cld(&std::fs::read(&ap06_path).expect("read ap06.cld")).expect("parse ap06");
    let col_ap06 = CollisionWorld::from_cld(&ap06);
    assert!(!col_ap06.walls.is_empty());

    // Pick an actual corridor wall from `ap06.cld` and place James 220 units (~44 cm) inside it,
    // backing the 640-unit OTS boom directly toward the wall plane!
    let wall = col_ap06
        .walls
        .iter()
        .find(|w| w.group == 1 && w.y_top <= -1200.0 && w.y_bottom >= -50.0)
        .expect("ap06 corridor wall");
    let mid_x = 0.5 * (wall.start[0] + wall.end[0]);
    let mid_z = 0.5 * (wall.start[1] + wall.end[1]);
    let dx = wall.end[0] - wall.start[0];
    let dz = wall.end[1] - wall.start[1];
    let len = (dx * dx + dz * dz).sqrt().max(1.0);
    let nx = -dz / len;
    let nz = dx / len;
    // Place James 220 units along +normal from the wall midpoint, facing +normal so boom (-forward) shoots straight through the wall!
    let james_pos = Vec3::new(mid_x + nx * 220.0, -10.0, mid_z + nz * 220.0);
    let facing_yaw = nx.atan2(nz);

    let mut ots = OtsCameraRig::default();
    ots.reset_to_player(james_pos, facing_yaw);
    let cfg = SpringArmConfig::default();
    let (snap, _, _, _) = ots.tick(
        james_pos, facing_yaw, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, false, false, false, &col_ap06, &cfg,
    );
    assert!(
        ots.spring_arm.last_hit_fraction < 0.5,
        "ap06 corridor wall must occlude 640-unit OTS boom (got {})",
        ots.spring_arm.last_hit_fraction
    );
    assert!(
        ots.spring_arm.current_distance < 220.0,
        "Spring arm must pull in inside ap06 corridor wall without clipping outside"
    );
    assert!(snap.eye.is_finite());

    // 2. Retail `ap01.cld` (ceiling at Y = -1200.0) & Brookhaven `hp05.cld` (ceiling at Y = -1450.0):
    // Looking sharply downward (pitch > 0 -> boom swings upward toward -Y ceiling) must clamp below ceiling!
    for (rel_cld, expected_ceiling_y) in [
        ("data/bg/ap/ap01.cld", -1200.0_f32),
        ("data/bg/hp/hp05.cld", -1450.0_f32),
    ] {
        let bytes = std::fs::read(format!("{root}/{rel_cld}")).expect("read retail cld");
        let cld = parse_cld(&bytes).expect("parse retail cld");
        let col = CollisionWorld::from_cld(&cld);
        let tri = &col.floors[0];
        let cx = (tri.v[0][0] + tri.v[1][0] + tri.v[2][0]) / 3.0;
        let cy = (tri.v[0][1] + tri.v[1][1] + tri.v[2][1]) / 3.0;
        let cz = (tri.v[0][2] + tri.v[1][2] + tri.v[2][2]) / 3.0;

        let mut rig_ceil = OtsCameraRig::default();
        rig_ceil.config.hip_boom_distance = 1100.0;
        rig_ceil.reset_to_player(Vec3::new(cx, cy, cz), 0.0);
        rig_ceil.orbit_pitch = 1.00; // Looking steep down -> boom points steep up (-Y) into ceiling
        let (csnap, _, _, _) = rig_ceil.tick(
            Vec3::new(cx, cy, cz),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            false,
            false,
            false,
            &col,
            &cfg,
        );
        assert!(
            csnap.eye.y >= expected_ceiling_y + cfg.ceiling_standoff_units - 1.0,
            "{rel_cld}: eye.y ({}) must not clip above ceiling ({expected_ceiling_y})",
            csnap.eye.y
        );
    }
}

#[test]
fn test_first_person_retail_head_bone_anchoring_visibility_and_sway() {
    let root = retail_root();
    let jms_path = format!("{root}/data/chr/jms/hll_jms.mdl");
    if std::path::Path::new(&jms_path).exists() {
        for (rel, ch, expected_y) in [
            (
                "data/chr/jms/hll_jms.mdl",
                PlayableCharacter::James,
                -844.6_f32,
            ),
            (
                "data/chr/jms/hhh_jms.mdl",
                PlayableCharacter::James,
                -844.6_f32,
            ),
            (
                "data/chr/mar/lll_mar.mdl",
                PlayableCharacter::Maria,
                -825.6_f32,
            ),
            (
                "data/chr/mar/hhh_mar.mdl",
                PlayableCharacter::Maria,
                -825.6_f32,
            ),
        ] {
            let mdl = parse_mdl(&std::fs::read(format!("{root}/{rel}")).expect("read mdl"))
                .expect("parse mdl");
            let spec = CharacterHeadRigSpec::for_character(ch);
            let eye = spec.resolve_world_eye_anchor(
                Vec3::new(100.0, 0.0, 200.0),
                0.0,
                Some(&mdl.initial_matrices),
            );
            assert!(
                (eye.y - expected_y).abs() < 2.0,
                "{rel}: expected head-rig eye Y ~ {expected_y}, got {}",
                eye.y
            );
        }
    }

    let mut rig = ModernCameraRig::new();
    let rail = CameraSnapshot::default();
    let collision = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 1600.0, 0.0, 0.0);
    let timing = TimingConstants::default();

    rig.active_mode = GameplayCameraMode::FirstPerson;
    rig.first_person.config.fov_y_degrees = 85.0;
    rig.reset_to_player(PlayableCharacter::James, Vec3::ZERO, 0.0, &rail);

    let mut inp = default_tick_input(Vec3::ZERO, 0.0);
    inp.linear_speed = 1200.0;
    inp.extra_pitch_delta_rad = -5.0; // Extreme upward flick -> must clamp to min_pitch_radians
    let f1 = rig.tick(&inp, &rail, &collision, &timing);
    assert_eq!(
        f1.player_visibility,
        PlayerMeshVisibility::HideHeadAndUpperBody
    );
    assert!((f1.near_clip - FIRST_PERSON_NEAR_CLIP_UNITS).abs() < 1e-4);
    assert!((f1.snapshot.fov_y - 85.0_f32.to_radians()).abs() < 1e-4);
    assert!((rig.first_person.look_pitch - rig.first_person.config.min_pitch_radians).abs() < 1e-4);

    // Disable head-bob -> bob_phase decays and sway offset is zero
    rig.first_person.head_bob_mode = HeadBobMode::Disabled;
    inp.extra_pitch_delta_rad = 0.0;
    let f2 = rig.tick(&inp, &rail, &collision, &timing);
    assert!((f2.aim_dir - f2.flashlight_dir).length() < 1e-5);
}

#[test]
fn test_cutscene_and_cutaway_handoffs_preserve_and_restore_modern_mode() {
    let mut rig = ModernCameraRig::new();
    let rail = CameraSnapshot::default();
    let collision = CollisionWorld::create_synthetic_room(6000.0, 6000.0, 2000.0, 0.0, 0.0);
    let timing = TimingConstants::default();

    rig.active_mode = GameplayCameraMode::FirstPerson;
    rig.reset_to_player(PlayableCharacter::James, Vec3::ZERO, 0.0, &rail);
    let inp = default_tick_input(Vec3::ZERO, 0.0);
    let fps_frame = rig.tick(&inp, &rail, &collision, &timing);
    assert_eq!(
        fps_frame.player_visibility,
        PlayerMeshVisibility::HideHeadAndUpperBody
    );

    // 1. 2D Puzzle Cutaway (`apt_clock_open`) locks camera & suppresses R3, then blends out cleanly
    let mut cutaway = CutawayOverlayEngine::new();
    let flags = EventFlags::default();
    cutaway.set_picture(Some("apt_clock_open"), &flags);
    assert_eq!(cutaway.kind(), OverlayKind::ClockOpen);
    rig.sync_cutaway(&cutaway);
    assert_eq!(
        rig.cycle_mode_r3(&rail, PlayableCharacter::James, Vec3::ZERO, 0.0),
        GameplayCameraMode::FirstPerson,
        "R3 must not swap modes while in a 2D puzzle cutaway"
    );
    cutaway.set_picture(None, &flags);
    rig.sync_cutaway(&cutaway);
    assert!(matches!(
        rig.handoff,
        ExternalCameraHandoff::BlendingOutOfExternal { .. }
    ));
    for _ in 0..25 {
        rig.tick(&inp, &rail, &collision, &timing);
    }
    assert!(matches!(rig.handoff, ExternalCameraHandoff::GameplayActive));

    // 2. Retail `.dds` Cutscene (`tenbou.dds` SH2-63) forces FullBody during cutscene, then restores FirstPerson!
    let tenbou_path = format!("{}/data/demo/tenbou/tenbou.dds", retail_root());
    if std::path::Path::new(&tenbou_path).exists() {
        let info = find_drama_demo_by_id("tenbou").expect("tenbou info");
        let dds = parse_demo_dds(&std::fs::read(&tenbou_path).expect("read tenbou")).expect("dds");
        let mut player = DramaDemoPlayer::new(*info, dds, false);
        player.camera.is_hard_cut = false;
        rig.sync_cutscene(Some(&player));
        assert!(matches!(
            rig.handoff,
            ExternalCameraHandoff::BlendingIntoCutscene { .. }
        ));
        let cs_frame = rig.tick(&inp, &rail, &collision, &timing);
        assert_eq!(
            cs_frame.player_visibility,
            PlayerMeshVisibility::FullBody,
            "James's head/body must be restored to FullBody during real-time .dds cutscenes"
        );

        player.skip();
        rig.sync_cutscene(Some(&player));
        assert!(matches!(
            rig.handoff,
            ExternalCameraHandoff::BlendingOutOfExternal { .. }
        ));
        for _ in 0..25 {
            rig.tick(&inp, &rail, &collision, &timing);
        }
        let restored = rig.tick(&inp, &rail, &collision, &timing);
        assert_eq!(restored.active_mode, GameplayCameraMode::FirstPerson);
        assert_eq!(
            restored.player_visibility,
            PlayerMeshVisibility::HideHeadAndUpperBody
        );
    }
}
