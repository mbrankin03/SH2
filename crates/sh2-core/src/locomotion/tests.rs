//! Unit and golden-vector parity tests for `sh2-core` locomotion.

use super::*;

#[test]
fn test_ps2_movement_speeds_and_parity_constants() {
    let mut controller = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;

    // 1. Walk test at full stick tilt (1.0)
    let mut input = LocomotionInput {
        stick_y: 1.0,
        ..Default::default()
    };
    let dt = 1.0 / 60.0;

    let out = controller.update(&input, &mut yaw, dt, true);
    assert_eq!(out.state, LocomotionState::Walk);
    // Speed should be exactly 1.5 m/s * 500 = 750 units/s
    assert!((controller.speed_mps - 1.5).abs() < 1e-3);
    assert!((out.linear_speed - 750.0).abs() < 1e-3);
    // Per-tick displacement at 60 Hz = 750 / 60 = 12.5 units
    assert!((out.delta_position.z - 12.5).abs() < 1e-2);

    // 2. Backward walk test
    controller.reset();
    input.stick_y = -1.0;
    let out_back = controller.update(&input, &mut yaw, dt, true);
    assert_eq!(out_back.state, LocomotionState::Back);
    // Speed should be 1.2 m/s * 500 = 600 units/s (~10.0 units/tick)
    assert!((controller.speed_mps - 1.2).abs() < 1e-3);
    assert!((out_back.linear_speed - 600.0).abs() < 1e-3);
    assert!((out_back.delta_position.z - (-10.0)).abs() < 1e-2);

    // 3. Playable Maria vs Follower Maria speed test
    controller.reset();
    input.stick_y = 1.0;
    input.is_maria_follower = false;
    let out_maria_playable = controller.update(&input, &mut yaw, dt, true);
    assert_eq!(out_maria_playable.state, LocomotionState::Walk);
    // Playable Maria has full 1.0x player speed (750 units/s)
    assert!((out_maria_playable.linear_speed - 750.0).abs() < 1e-3);

    // Follower Maria has 0.8x companion multiplier (600 units/s)
    controller.reset();
    input.is_maria_follower = true;
    let out_maria_follower = controller.update(&input, &mut yaw, dt, true);
    assert_eq!(out_maria_follower.state, LocomotionState::Walk);
    assert!((out_maria_follower.linear_speed - 600.0).abs() < 1e-3);
}

#[test]
fn test_ps2_turning_speeds_and_quick_turn() {
    let mut controller = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    // Spot turn test (stick_x = 1.0) -> 2.4 rad/s
    let input_turn = LocomotionInput {
        stick_x: 1.0,
        ..Default::default()
    };
    let out_turn = controller.update(&input_turn, &mut yaw, dt, true);
    assert_eq!(out_turn.state, LocomotionState::TurnRight);
    // 2.4 rad/s * (1/60s) = 0.04 rad/tick
    assert!((yaw - 0.04).abs() < 1e-3);

    // Spot turn with run held -> 2.4 * 1.5 = 3.6 rad/s
    yaw = 0.0;
    let input_turn_run = LocomotionInput {
        stick_x: 1.0,
        run_held: true,
        ..Default::default()
    };
    controller.update(&input_turn_run, &mut yaw, dt, true);
    assert!((yaw - 0.06).abs() < 1e-3); // 3.6 / 60 = 0.06 rad/tick

    // Quick turn test (180° = PI rad at 5.4977875 rad/s)
    controller.reset();
    yaw = 0.0;
    let input_quick = LocomotionInput {
        quick_turn_pressed: true,
        ..Default::default()
    };
    let out_qt = controller.update(&input_quick, &mut yaw, dt, true);
    assert!(out_qt.state.is_quick_turn());

    // Step until complete (PI / 5.4977875 s = ~0.5714 s -> ~35 ticks)
    let mut ticks = 0;
    while controller.state.is_quick_turn() {
        controller.update(&LocomotionInput::default(), &mut yaw, dt, true);
        ticks += 1;
    }
    assert!((34..=36).contains(&ticks));
    assert!((yaw.abs() - PI).abs() < 1e-2);
}

#[test]
fn test_ps2_running_acceleration_and_deceleration() {
    let mut controller = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    let input_run = LocomotionInput {
        stick_y: 1.0,
        run_held: true,
        ..Default::default()
    };

    // First tick enters Run1 and clamps speed to the 2.0 m/s PS2 floor (0x54cfd0)
    let out = controller.update(&input_run, &mut yaw, dt, true);
    assert_eq!(out.state, LocomotionState::Run1);
    assert!((controller.speed_mps - 2.0).abs() < 1e-3);
    assert!((out.linear_speed - 1000.0).abs() < 1e-3);

    // Advance 1 second (60 ticks): acceleration at 3.0 m/s^2 progresses through Run1 (3.5 m/s) and Run2 (4.0 m/s) to Run3
    for _ in 0..60 {
        controller.update(&input_run, &mut yaw, dt, true);
    }
    assert!(controller.speed_mps >= 3.5);
    assert!(controller.state.is_running());

    // Let go of stick -> deceleration at 20 m/s^2 post-run
    let top_speed = controller.speed_mps;
    let out_stop = controller.update(&LocomotionInput::default(), &mut yaw, dt, true);
    assert_eq!(out_stop.state, LocomotionState::Stand);
    let speed_after_1_tick = controller.speed_mps;
    assert!(speed_after_1_tick < top_speed);
    // After 20 ticks (~0.33s), speed should have dropped by 20.0 * 0.33 = 6.6 m/s -> fully stopped (0.0)
    for _ in 0..20 {
        controller.update(&LocomotionInput::default(), &mut yaw, dt, true);
    }
    assert_eq!(controller.speed_mps, 0.0);
}

#[test]
fn test_sh2_61_double_footstep_glitch_mitigation() {
    let mut controller = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    // Simulate rapid toggling between Walk and Run1 while moving forward
    let mut last_step_tick: Option<u32> = None;
    let mut step_count = 0u32;

    for tick in 0..180u32 {
        let input = LocomotionInput {
            stick_y: 1.0,
            run_held: (tick / 3) % 2 == 0,
            ..Default::default()
        };
        let out = controller.update(&input, &mut yaw, dt, true);
        if out.footstep_triggered {
            if let Some(prev) = last_step_tick {
                let elapsed_secs = (tick - prev) as f32 * dt;
                assert!(
                    elapsed_secs + 1e-4 >= MIN_FOOTSTEP_INTERVAL_SECS,
                    "Double-footstep glitch detected! Two footsteps triggered {}s apart (< {}s)",
                    elapsed_secs,
                    MIN_FOOTSTEP_INTERVAL_SECS
                );
            }
            last_step_tick = Some(tick);
            step_count += 1;
        }
    }
    assert!(
        step_count >= 5,
        "Expected footsteps during 3s walk/run movement"
    );
}

#[test]
fn test_sh2_61_authentic_walk_and_run_footstep_cadence() {
    let mut controller = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    // 1. Full-speed forward Walk (1.5 m/s -> 20.625 FPS on 24-frame cycle = 1.71875 steps/sec, ~0.582s interval).
    // Over 600 ticks (10.0 seconds), we expect ~17 footsteps (specifically 17 steps: 10.0 * 20.625 / 12.0 = 17.1875).
    let walk_input = LocomotionInput {
        stick_y: 1.0,
        run_held: false,
        ..Default::default()
    };
    let mut walk_steps = Vec::new();
    for tick in 0..600u32 {
        let out = controller.update(&walk_input, &mut yaw, dt, true);
        if out.footstep_triggered {
            walk_steps.push(tick as f32 * dt);
        }
    }
    assert!(
        (16..=18).contains(&walk_steps.len()),
        "Full walk over 10s should trigger ~17 footsteps (1.72 steps/s), got {}",
        walk_steps.len()
    );
    for pair in walk_steps.windows(2) {
        let interval = pair[1] - pair[0];
        assert!(
            (interval - (12.0 / 20.625)).abs() < 0.03,
            "Walk step interval should be ~0.582s (12 frames @ 20.625 FPS), got {interval:.3}s"
        );
    }

    // 2. Half-speed forward Walk (stick_y = 0.5 -> slower cadence than full walk)
    controller.reset();
    let slow_walk_input = LocomotionInput {
        stick_y: 0.5,
        run_held: false,
        ..Default::default()
    };
    let mut slow_walk_steps = 0usize;
    for _ in 0..600u32 {
        let out = controller.update(&slow_walk_input, &mut yaw, dt, true);
        if out.footstep_triggered {
            slow_walk_steps += 1;
        }
    }
    assert!(
            slow_walk_steps < walk_steps.len(),
            "Half-tilt walk ({slow_walk_steps} steps) must have slower cadence than full walk ({} steps)",
            walk_steps.len()
        );
}

#[test]
fn test_sh2_132_direction_offset_easing_run1_stamina_and_downgrades() {
    let mut ctrl = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    // 1. Run1 has ZERO stamina drain (`lower_run1_3d` 0x00117ec0)
    let run_in = LocomotionInput {
        stick_y: 1.0,
        run_held: true,
        ..Default::default()
    };
    for _ in 0..25 {
        ctrl.update(&run_in, &mut yaw, dt, true);
    }
    assert_eq!(ctrl.state, LocomotionState::Run1);
    assert_eq!(
        ctrl.stamina_frames, 0.0,
        "Run1 must NOT drain stamina (SLUS 0x00117ec0)"
    );

    // 2. Continue accelerating into Run2 -> drains stamina unless Ampoule is active
    let ampoule_run = LocomotionInput {
        ampoule_active: true,
        ..run_in
    };
    for _ in 0..40 {
        ctrl.update(&ampoule_run, &mut yaw, dt, true);
    }
    assert!(matches!(
        ctrl.state,
        LocomotionState::Run2 | LocomotionState::Run3
    ));
    assert_eq!(
        ctrl.stamina_frames, 0.0,
        "Ampoule must prevent stamina drain in Run2/Run3"
    );

    // 3. Analog speed-drop downgrade Run3 -> Run2 -> Run1
    let half_run = LocomotionInput {
        stick_y: 0.5,
        run_held: true,
        ..Default::default()
    };
    for _ in 0..60 {
        ctrl.update(&half_run, &mut yaw, dt, true);
    }
    assert_eq!(
        ctrl.state,
        LocomotionState::Run1,
        "Analog speed drop below 3.5 m/s must downgrade Run3 -> Run2 -> Run1"
    );

    // 4. Direction-offset easing: Walk eases at 2.0 rad/s (vs 4.0 rad/s in Run1)
    ctrl.reset();
    ctrl.update(
        &LocomotionInput {
            strafe_right: true,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert!((ctrl.direction_offset - PI * 0.5).abs() < 1e-4);
    ctrl.update(
        &LocomotionInput {
            stick_y: 1.0,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    let expected_walk_offset = PI * 0.5 - 2.0 * dt;
    assert!(
        (ctrl.direction_offset - expected_walk_offset).abs() < 1e-4,
        "Walk must ease direction_offset at 2.0 rad/s; got {}",
        ctrl.direction_offset
    );
}

#[test]
fn test_sh2_132_strafe_reversal_digital_back_run_and_aim_run() {
    let mut ctrl = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    // 1. Back + Run boosts output speed to 1.8 m/s (900 units/s)
    let back_run = LocomotionInput {
        stick_y: -1.0,
        run_held: true,
        ..Default::default()
    };
    let out_br = ctrl.update(&back_run, &mut yaw, dt, true);
    assert_eq!(out_br.state, LocomotionState::Back);
    assert!((out_br.linear_speed - 900.0).abs() < 1e-2);

    // 2. Aim + Run in Walk boosts output speed to 2.25 m/s (1125 units/s)
    ctrl.reset();
    let aim_walk_run = LocomotionInput {
        stick_y: 1.0,
        run_held: true,
        is_aiming: true,
        ..Default::default()
    };
    let out_awr = ctrl.update(&aim_walk_run, &mut yaw, dt, true);
    assert_eq!(out_awr.state, LocomotionState::Walk);
    assert!((out_awr.linear_speed - 1125.0).abs() < 1e-2);

    // 3. StrafeWalk reversal from Back decelerates at 5.1 m/s^2 (`0x00117320`)
    ctrl.reset();
    ctrl.state = LocomotionState::StrafeLeftWalk;
    ctrl.prev_state = LocomotionState::Back;
    ctrl.speed_mps = 1.0;
    ctrl.update(
        &LocomotionInput {
            strafe_left: true,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert!((ctrl.speed_mps - (1.0 - 5.100_000_4 * dt)).abs() < 1e-4);

    // 4. Digital Walk ramps at +2.5 m/s^2 from 0.6 entry speed instead of instant 1.5
    ctrl.reset();
    ctrl.update(
        &LocomotionInput {
            stick_y: 1.0,
            is_digital_input: true,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert!((ctrl.speed_mps - (0.6 + 2.5 * dt)).abs() < 1e-4);
}

#[test]
fn test_sh2_132_water_slope_great_knife_and_wall_f_recoil() {
    let mut ctrl = PlayerLocomotionController::new();
    let mut yaw = 0.0f32;
    let dt = 1.0 / 60.0;

    // 1. Water wading: 0.65x speed (`PlayerUpdatePosition3D`) and 1.3x turn rate (`lower_walk_3d`)
    let water_walk = LocomotionInput {
        stick_y: 1.0,
        in_water: true,
        ..Default::default()
    };
    let out_w = ctrl.update(&water_walk, &mut yaw, dt, true);
    assert!((out_w.linear_speed - (750.0 * 0.65)).abs() < 1e-2);

    let water_turn = LocomotionInput {
        stick_y: 1.0,
        stick_x: 1.0,
        in_water: true,
        is_digital_input: true,
        ..Default::default()
    };
    let out_wt = ctrl.update(&water_turn, &mut yaw, dt, true);
    assert!((out_wt.turn_speed_rad_per_sec - (1.9 * 1.3)).abs() < 1e-2);

    // 2. Slope cos^2 scaling (`PlayerUpdatePosition3D` 0x0011c340)
    ctrl.reset();
    yaw = 0.0;
    let slope_angle = 0.5f32;
    let out_slope = ctrl.update(
        &LocomotionInput {
            stick_y: 1.0,
            floor_normal: Vec3::new(0.0, -slope_angle.cos(), slope_angle),
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    let expected_dz = 750.0 * dt * slope_angle.cos().powi(2);
    assert!((out_slope.delta_position.z - expected_dz).abs() < 1e-2);

    // 3. Great Knife (`*_nata`) uses 24-frame speed table, blocks Run, and uses 4.712389 rad/s quick turn
    ctrl.reset();
    yaw = 0.0;
    let nata_walk = LocomotionInput {
        stick_y: 1.0,
        run_held: true,
        is_great_knife: true,
        anim_frame: 3, // GREAT_KNIFE_WALK_SPEEDS[3] == 0.045 m/s
        ..Default::default()
    };
    let out_nata = ctrl.update(&nata_walk, &mut yaw, dt, true);
    assert_eq!(out_nata.state, LocomotionState::Walk);
    assert!((ctrl.speed_mps - 0.045).abs() < 1e-5);

    // 4. WallF recoil triggers when sprinting (>= 3.0 m/s) head-on (dot <= -0.85) into a wall
    ctrl.reset();
    ctrl.state = LocomotionState::Run2;
    ctrl.speed_mps = 3.8;
    let out_wf = ctrl.update(
        &LocomotionInput {
            stick_y: 1.0,
            run_held: true,
            wall_head_hit: true,
            wall_normal_dot: -0.95,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert_eq!(out_wf.state, LocomotionState::WallF);
    // WallF remains active until `lower_anim_finished` (`_DAT_003c7f70 & 1`) is signaled
    let out_wf_exit = ctrl.update(
        &LocomotionInput {
            lower_anim_finished: true,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert_eq!(out_wf_exit.state, LocomotionState::Stand);

    // 5. Fall state (`lower_fall` 0x0021f5c0): ramps to 0.9 for frames < 21, decelerates for >= 21,
    //    and exits only on `lower_anim_finished` (`_DAT_003c7f70 & 1`).
    ctrl.reset();
    ctrl.update(
        &LocomotionInput {
            trigger_fall: true,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert_eq!(ctrl.state, LocomotionState::Fall);
    // Step 42 ticks (21 frames at 30 fps) -> reaches 0.9 m/s, then decelerates via PlayerSpeedDownToStand
    for _ in 0..42 {
        ctrl.update(&LocomotionInput::default(), &mut yaw, dt, true);
    }
    assert_eq!(ctrl.state, LocomotionState::Fall);
    let speed_at_21 = ctrl.speed_mps;
    ctrl.update(&LocomotionInput::default(), &mut yaw, dt, true);
    assert!(
        ctrl.speed_mps < speed_at_21,
        "Fall frames >= 21 must decelerate via PlayerSpeedDownToStand"
    );
    let out_fall_exit = ctrl.update(
        &LocomotionInput {
            lower_anim_finished: true,
            ..Default::default()
        },
        &mut yaw,
        dt,
        true,
    );
    assert_eq!(out_fall_exit.state, LocomotionState::Stand);
}
