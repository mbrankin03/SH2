//! Deterministic 1:1 parity tests for `SH2-189`: progression-blocking turn-around and walk-back
//! via `PlayerEventMove` (`SLUS_202.28` `0x00283d10`, `sh2pc.exe` `0x0053b3c0`).
//!
//! Verifies all four retail progression-blocking boundary programs in `sh2pc.exe`:
//! - Stage 3 (`stg_ovservation.c`) `ev_prog[4]` (`0x005a0cb0`): forest stairs without map (`-500 mm` Z)
//! - Stage 3 (`stg_ovservation.c`) `ev_prog[5]` (`0x005a0de0`): blocked road/tunnel (`-500 mm` X)
//! - Stage 5 (`stg_town_east.c`) `ev_prog[4]` (`0x0059d720`): Town East boundary (`+500 mm` X)
//! - Stage 14 (`stg_town_west.c`) `ev_prog[4]` (`0x00592550`): Town West boundary (`+500 mm` Z)

use std::f32::consts::{FRAC_PI_2, PI};

use glam::Vec3;
use sh2_core::animation::AnimationStateKind;
use sh2_core::campaign::{CampaignOrchestrator, CampaignPhase, CampaignTickInput, ScreenFadeState};
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::events::{STG3_FLAG_STAIRS_WARNED, STG3_MSG_BLOCKED_ROAD, STG3_MSG_STAIRS_NO_MAP};
use sh2_core::locomotion::{wrap_angle, LocomotionState, BOUNDARY_WALKBACK_DISTANCE_MM};
use sh2_core::scenario::ScenarioKind;
use sh2_core::simulation::SimulationCore;

fn setup_active_gameplay(
    stage_id: u16,
    room: &str,
    pos: [f32; 3],
    yaw: f32,
) -> (SimulationCore, CampaignOrchestrator) {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = stage_id;
    sim.scenario.current_room = room.to_string();
    sim.snap_player_transform(Vec3::from_array(pos), yaw);

    let mut orch = CampaignOrchestrator::from_saved_stage(
        stage_id,
        Some(room.to_string()),
        pos,
        yaw,
        OutdoorStreamingMode::default(),
        false,
    );
    orch.phase = CampaignPhase::ActiveGameplay;
    orch.fade = ScreenFadeState::default();
    orch.player_locked = false;
    (sim, orch)
}

#[test]
fn sh2_189_ob01_pre_map_stairs_block_smoothly_turns_around_without_jutting_forward() {
    let start_pos = [19_350.0_f32, 0.0, -500.0];
    let start_yaw = 0.0_f32; // Facing +Z toward the blocked stairs
    let (mut sim, mut orch) = setup_active_gameplay(3, "ob01", start_pos, start_yaw);

    // Simulate James sprinting full speed (`4.0 m/s`) into the stairs trigger:
    // `start_event_move` must zero prior sprint momentum so he never juts forward.
    sim.locomotion.speed_mps = 4.0;
    sim.locomotion.state = LocomotionState::Run3;

    sim.scenario
        .prog_runner
        .start(3, 4, STG3_FLAG_STAIRS_WARNED as u16);

    let idle = CampaignTickInput::default();
    let confirm = CampaignTickInput {
        action_pressed: true,
        ..CampaignTickInput::default()
    };

    // Frame 0: shows stage message 2 and locks player.
    let eff0 = orch.step(&mut sim, &idle, 1.0 / 60.0);
    assert_eq!(eff0.stage_message, Some(STG3_MSG_STAIRS_NO_MAP as u16));
    assert!(!eff0.allow_gameplay_sim);

    // Frame 1: player dismisses message -> starts `PlayerEventMove` (`-500 mm` along Z),
    // remains locked, does NOT teleport in 1 frame.
    let eff1 = orch.step(&mut sim, &confirm, 1.0 / 60.0);
    assert!(!eff1.allow_gameplay_sim);
    assert!(sim.event_move.is_active());
    assert!(
        (sim.scenario.player_position.z - start_pos[2]).abs() < 20.0,
        "must not teleport on dismiss frame; z = {}",
        sim.scenario.player_position.z
    );

    let mut max_forward_z = 0.0_f32;
    let mut saw_walk_anim = false;
    let mut saw_intermediate_yaw = false;
    let mut frames = 0_u32;

    for _ in 0..240 {
        let eff = orch.step(&mut sim, &idle, 1.0 / 60.0);
        frames += 1;
        max_forward_z = max_forward_z.max(sim.scenario.player_position.z - start_pos[2]);
        if sim.animation.state() == AnimationStateKind::Walk {
            saw_walk_anim = true;
        }
        let yaw_mag = sim.scenario.player_yaw.abs();
        if yaw_mag > 0.3 && yaw_mag < PI - 0.3 {
            saw_intermediate_yaw = true;
        }
        if eff.allow_gameplay_sim {
            break;
        }
    }

    assert!(!sim.event_move.is_active());
    assert!(!sim.scenario.prog_runner.is_running());
    assert!(sim.scenario.flags.get(STG3_FLAG_STAIRS_WARNED));
    assert!(saw_walk_anim, "James must play Walk animation (clip 0x68)");
    assert!(
        saw_intermediate_yaw,
        "James's yaw must smoothly interpolate rather than snapping"
    );
    assert!(
        (40..=140).contains(&frames),
        "turn-around should complete smoothly in ~1.15s (~69 frames), took {frames}"
    );
    assert!(
        max_forward_z < 15.0,
        "James must never jut forward into the stairs; max_forward_z = {max_forward_z} mm"
    );
    let dz = sim.scenario.player_position.z - start_pos[2];
    assert!(
        dz < -400.0 && dz > -(BOUNDARY_WALKBACK_DISTANCE_MM + 80.0),
        "James should step ~500 mm back along -Z, got dz = {dz} mm"
    );
    assert!(
        sim.scenario.player_yaw.abs() > PI * 0.75,
        "James should finish facing away from +Z boundary, got {} deg",
        sim.scenario.player_yaw.to_degrees()
    );
}

#[test]
fn sh2_189_all_generic_progression_block_events_smoothly_turn_and_step_back() {
    struct Case {
        name: &'static str,
        stage: u16,
        room: &'static str,
        prog_no: u8,
        expected_msg: u16,
        start_yaw: f32,
        return_dir: Vec3,
    }

    let cases = [
        Case {
            name: "ob01 blocked road (3, 5) -500mm X",
            stage: 3,
            room: "ob01",
            prog_no: 5,
            expected_msg: STG3_MSG_BLOCKED_ROAD as u16,
            start_yaw: FRAC_PI_2, // Facing +X toward blocked tunnel
            return_dir: Vec3::new(-1.0, 0.0, 0.0),
        },
        Case {
            name: "town east boundary (5, 4) +500mm X",
            stage: 5,
            room: "ca01",
            prog_no: 4,
            expected_msg: 30,
            start_yaw: -FRAC_PI_2, // Facing -X toward blocked street
            return_dir: Vec3::new(1.0, 0.0, 0.0),
        },
        Case {
            name: "town west boundary (14, 4) +500mm Z",
            stage: 14,
            room: "cb01",
            prog_no: 4,
            expected_msg: 90,
            start_yaw: PI, // Facing -Z toward blocked street
            return_dir: Vec3::new(0.0, 0.0, 1.0),
        },
    ];

    let idle = CampaignTickInput::default();
    let confirm = CampaignTickInput {
        action_pressed: true,
        ..CampaignTickInput::default()
    };

    for case in cases {
        let start_pos = [1000.0_f32, 0.0, 1000.0];
        let (mut sim, mut orch) =
            setup_active_gameplay(case.stage, case.room, start_pos, case.start_yaw);
        sim.scenario
            .prog_runner
            .start(case.stage as u8, case.prog_no, 0);

        let eff0 = orch.step(&mut sim, &idle, 1.0 / 60.0);
        assert_eq!(eff0.stage_message, Some(case.expected_msg), "{}", case.name);
        assert!(!eff0.allow_gameplay_sim, "{}", case.name);

        let eff1 = orch.step(&mut sim, &confirm, 1.0 / 60.0);
        assert!(!eff1.allow_gameplay_sim, "{}", case.name);
        assert!(sim.event_move.is_active(), "{}", case.name);

        let mut max_wrong_way = 0.0_f32;
        let mut unlocked = false;
        for _ in 0..240 {
            let eff = orch.step(&mut sim, &idle, 1.0 / 60.0);
            let disp = sim.scenario.player_position - Vec3::from_array(start_pos);
            // Projection opposite to `return_dir` measures any forward jutting into the block.
            let wrong_way = -disp.dot(case.return_dir);
            max_wrong_way = max_wrong_way.max(wrong_way);
            if eff.allow_gameplay_sim {
                unlocked = true;
                break;
            }
        }

        assert!(unlocked, "{} must unlock after PlayerEventMove", case.name);
        assert!(
            max_wrong_way < 15.0,
            "{} must not jut forward into blocked boundary; max_wrong_way = {max_wrong_way} mm",
            case.name
        );

        let disp = sim.scenario.player_position - Vec3::from_array(start_pos);
        let back_dist = disp.dot(case.return_dir);
        assert!(
            back_dist > 400.0 && back_dist < BOUNDARY_WALKBACK_DISTANCE_MM + 80.0,
            "{} expected ~500 mm walkback along return_dir, got {back_dist} mm",
            case.name
        );

        let target_yaw = case.return_dir.x.atan2(case.return_dir.z);
        let yaw_err = wrap_angle(sim.scenario.player_yaw - target_yaw).abs();
        assert!(
            yaw_err < PI * 0.25,
            "{} expected final yaw near {} deg, got {} deg (err = {} deg)",
            case.name,
            target_yaw.to_degrees(),
            sim.scenario.player_yaw.to_degrees(),
            yaw_err.to_degrees()
        );
    }
}
