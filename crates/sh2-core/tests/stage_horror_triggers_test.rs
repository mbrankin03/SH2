//! Integration tests for scripted environmental horror triggers (`StageEventDispatcher`:
//! Toluca Prison North/South/BF, Historical Society, Blue Creek Apartments, Wood Side Apartments).

use sh2_core::{
    eval_monkey_kick_206, AnyoneCryProgramState, CameraOverrideCommand, CollisionWallCommand,
    CutawayOverlayEngine, EventFlags, InventoryManager, PuzzleState, RiddleDifficulty, Sh2Rng,
    SpatialSoundCommand, StageEventDispatcher, StageHookContext, StageId, StageProgramRunner,
    APART_E3FE_ROOM13_CAMERA, FLAG_APART_E3FE_FOOTSTEPS_DONE, FLAG_APART_ROOM209_WHISPER_DONE,
    FLAG_APART_W2F_CORRIDOR_DONE, FLAG_PRISON_BATHROOM_JUMPSCARE_DONE,
    FLAG_PRISON_BATHROOM_KNOCKED, FLAG_PRISON_BF_ROOM77_OPEN, FLAG_PRISON_N_ROOM66_WALL,
    FLAG_PRISON_S_BARS_ACTIVE, FLAG_PRISON_S_BARS_BOOST, FLAG_PRISON_S_BARS_COUNTING,
    FLAG_SOCIETY_ROOM5C_TRIGGERED, PROG_KEY_A, SE_APART_ANYONE_CRY, SE_APART_E3FE_FOOTSTEPS,
    SE_APART_MONKEY_KICK_206, SE_APART_ROOM205_TV_STATIC, SE_APART_ROOM209_WHISPER,
    SE_APART_W2F_CORRIDOR, SE_PRISON_BATHROOM_STALL_BANG, SE_PRISON_INVISIBLE_PRISONER,
    SE_PRISON_ROOM68_MECHANISM, SE_SOCIETY_ROOM_5C_HORROR, SE_SOCIETY_STAIRCASE_AMBIENT,
};

#[test]
fn prison_north_bathroom_stall_knock_and_jumpscare_sequence() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(42);

    let mut reset_observed = false;
    for _ in 0..10 {
        flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
        flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, true);
        dispatcher.on_room_init(StageId::PrisonN, 0x73, &mut flags, &mut rng);
        assert!(
            dispatcher.prison_n_bathroom_z_threshold >= -20_000.0
                && dispatcher.prison_n_bathroom_z_threshold <= -19_200.0
        );
        if !flags.get(FLAG_PRISON_BATHROOM_KNOCKED)
            && !flags.get(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE)
        {
            reset_observed = true;
            break;
        }
    }
    assert!(
        reset_observed,
        "EvRoomInit should clear flags 648 & 649 when frand() > 0.3"
    );

    flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
    flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, false);

    let ctx_near_stall = StageHookContext {
        stage: StageId::PrisonN,
        room_id: 0x73,
        dt: 1.0 / 30.0,
        player_pos: [-138_700.0, 0.0, -20_500.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };
    let out_near = dispatcher.on_all_time(&ctx_near_stall, &mut flags, &mut rng);
    assert!(out_near.sounds.is_empty());

    let ctx_at_exit = StageHookContext {
        stage: StageId::PrisonN,
        room_id: 0x73,
        dt: 1.0 / 30.0,
        player_pos: [-138_700.0, 0.0, -19_000.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };

    let mut bang_triggered = false;
    for _ in 0..20 {
        flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
        flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, false);
        let out = dispatcher.on_all_time(&ctx_at_exit, &mut flags, &mut rng);
        if !out.sounds.is_empty() {
            assert_eq!(
                out.sounds[0],
                SpatialSoundCommand::CallPos {
                    se_id: SE_PRISON_BATHROOM_STALL_BANG,
                    volume: 1.0,
                    pos: [-138_700.0, -50.0, -21_000.0],
                    channel: 0,
                }
            );
            assert!(flags.get(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE));
            bang_triggered = true;
            break;
        } else {
            assert!(!flags.get(FLAG_PRISON_BATHROOM_KNOCKED));
        }
    }
    assert!(
        bang_triggered,
        "Bathroom stall jumpscare SE 0x427d must trigger"
    );

    flags.set(FLAG_PRISON_N_ROOM66_WALL, true);
    let out_66 = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::PrisonN,
            room_id: 0x66,
            dt: 1.0 / 30.0,
            player_pos: [0.0; 3],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_66.collision_commands,
        vec![CollisionWallCommand::PrisonNPs45Wall]
    );
    assert_eq!(out_66.model_states[0].model_id, 0x2d);
    assert_eq!(out_66.model_states[0].state, 0);
}

#[test]
fn prison_south_invisible_prisoner_ritual_emitter_and_cell_bars_timer() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(777);

    dispatcher.on_room_init(StageId::PrisonS, 0x72, &mut flags, &mut rng);
    assert!(
        dispatcher.prison_s_prisoner_timer >= 10.0 && dispatcher.prison_s_prisoner_timer <= 15.0
    );

    let ctx = StageHookContext {
        stage: StageId::PrisonS,
        room_id: 0x72,
        dt: 16.0,
        player_pos: [-140_000.0, 0.0, 20_000.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };
    let out = dispatcher.on_all_time(&ctx, &mut flags, &mut rng);
    assert_eq!(out.sounds.len(), 2);
    match out.sounds[1] {
        SpatialSoundCommand::CallPos {
            se_id,
            volume,
            pos,
            channel,
        } => {
            assert_eq!(se_id, SE_PRISON_INVISIBLE_PRISONER);
            assert_eq!(volume, 1.0);
            assert_eq!(channel, 0);
            assert!(pos[0] >= -146_000.0 && pos[0] <= -134_000.0);
            assert_eq!(pos[1], -500.0);
            assert!(pos[2] >= 10_400.0 && pos[2] <= 32_000.0);
        }
        other => panic!("Expected CallPos for SE 0x4682, got {other:?}"),
    }

    flags.set(FLAG_PRISON_S_BARS_ACTIVE, true);
    flags.set(FLAG_PRISON_S_BARS_COUNTING, true);
    flags.set(FLAG_PRISON_S_BARS_BOOST, true);
    let ctx_bars = StageHookContext {
        stage: StageId::PrisonS,
        room_id: 0x68,
        dt: 11.0,
        player_pos: [0.0; 3],
        player_rot_y: 0.0,
        active_sounds: [SE_PRISON_ROOM68_MECHANISM, 0, 0, 0],
    };
    let out_bars = dispatcher.on_all_time(&ctx_bars, &mut flags, &mut rng);
    assert!(!flags.get(FLAG_PRISON_S_BARS_BOOST));
    assert!(!flags.get(FLAG_PRISON_S_BARS_ACTIVE));
    assert_eq!(out_bars.model_states[0].model_id, 0x1d);
    assert_eq!(out_bars.model_states[0].state, -1);

    flags.set(FLAG_PRISON_BF_ROOM77_OPEN, false);
    let out_bf = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::PrisonBf,
            room_id: 0x77,
            dt: 0.1,
            player_pos: [0.0; 3],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_bf.collision_commands,
        vec![CollisionWallCommand::PrisonBfPs57Wall]
    );
    assert_eq!(out_bf.model_states[0].model_id, 0x39);
    assert_eq!(out_bf.model_states[0].state, 0);
}

#[test]
fn blue_creek_room209_orbiting_whisper_and_w2f_corridor_sound() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(1234);

    dispatcher.on_room_init(StageId::ApartW2f, 0x28, &mut flags, &mut rng);
    assert!(
        dispatcher.apart_w2f_room209_x_threshold >= 20_000.0
            && dispatcher.apart_w2f_room209_x_threshold <= 21_200.0
    );

    let ctx_start = StageHookContext {
        stage: StageId::ApartW2f,
        room_id: 0x28,
        dt: 0.5,
        player_pos: [19_500.0, -500.0, -10_000.0],
        player_rot_y: 0.0,
        active_sounds: [0; 4],
    };
    let out1 = dispatcher.on_all_time(&ctx_start, &mut flags, &mut rng);
    assert!(flags.get(FLAG_APART_ROOM209_WHISPER_DONE));
    assert_eq!(out1.sounds.len(), 2);
    assert_eq!(
        out1.sounds[0],
        SpatialSoundCommand::CallPos {
            se_id: SE_APART_ROOM209_WHISPER,
            volume: 1.0,
            pos: [0.0, 0.0, 0.0],
            channel: 6,
        }
    );
    let pos1 = match out1.sounds[1] {
        SpatialSoundCommand::PosMove { se_id, pos, .. } => {
            assert_eq!(se_id, SE_APART_ROOM209_WHISPER);
            let dx = pos[0] - ctx_start.player_pos[0];
            let dz = pos[2] - ctx_start.player_pos[2];
            let dist = (dx * dx + dz * dz).sqrt();
            assert!((dist - 2500.0).abs() < 1.0);
            pos
        }
        other => panic!("Expected PosMove, got {other:?}"),
    };

    let ctx_playing = StageHookContext {
        stage: StageId::ApartW2f,
        room_id: 0x28,
        dt: 1.0,
        player_pos: [19_500.0, -500.0, -10_000.0],
        player_rot_y: 0.0,
        active_sounds: [SE_APART_ROOM209_WHISPER, 0, 0, 0],
    };
    let out2 = dispatcher.on_all_time(&ctx_playing, &mut flags, &mut rng);
    assert_eq!(out2.sounds.len(), 1);
    match out2.sounds[0] {
        SpatialSoundCommand::PosMove { pos: pos2, .. } => {
            assert!((pos2[0] - pos1[0]).abs() > 50.0 || (pos2[2] - pos1[2]).abs() > 50.0);
        }
        other => panic!("Expected PosMove, got {other:?}"),
    }

    dispatcher.on_room_init(StageId::ApartW2f, 0x26, &mut flags, &mut rng);
    let out_arm = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartW2f,
            room_id: 0x26,
            dt: 0.1,
            player_pos: [100_800.0, -500.0, -6400.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(out_arm.sounds.is_empty());
    assert!(dispatcher.apart_w2f_corridor_armed);

    let out_fire = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartW2f,
            room_id: 0x26,
            dt: 0.1,
            player_pos: [100_800.0, -500.0, -6900.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(flags.get(FLAG_APART_W2F_CORRIDOR_DONE));
    assert_eq!(out_fire.sounds.len(), 1);
    match out_fire.sounds[0] {
        SpatialSoundCommand::CallPos { se_id, channel, .. } => {
            assert_eq!(se_id, SE_APART_W2F_CORRIDOR);
            assert_eq!(channel, 2);
        }
        other => panic!("Expected CallPos for SE 0x9c6c, got {other:?}"),
    }
}

#[test]
fn wood_side_apartments_footsteps_anyone_cry_monkey_kick_and_society_staircase() {
    let mut dispatcher = StageEventDispatcher::new();
    let mut flags = EventFlags::new();
    let mut rng = Sh2Rng::new(99);

    dispatcher.on_stage_init(StageId::ApartE3fe, &mut flags);
    let out_start = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartE3fe,
            room_id: 0x1f,
            dt: 0.1,
            player_pos: [60_000.0, 0.0, 16_500.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(flags.get(FLAG_APART_E3FE_FOOTSTEPS_DONE));
    assert_eq!(
        out_start.sounds,
        vec![SpatialSoundCommand::CallPos {
            se_id: SE_APART_E3FE_FOOTSTEPS,
            volume: 1.0,
            pos: [60_000.0, 0.0, 11_350.0],
            channel: 2,
        }]
    );

    let out_move = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartE3fe,
            room_id: 0x1f,
            dt: 2.0,
            player_pos: [60_000.0, 0.0, 16_500.0],
            player_rot_y: 0.0,
            active_sounds: [SE_APART_E3FE_FOOTSTEPS, 0, 0, 0],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_move.sounds,
        vec![SpatialSoundCommand::PosMove {
            se_id: SE_APART_E3FE_FOOTSTEPS,
            volume: 1.0,
            pos: [60_000.0 - 1400.0, 0.0, 11_350.0],
        }]
    );

    flags.set(91, true);
    let out_cam_lock = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::ApartE3fe,
            room_id: 0x13,
            dt: 0.1,
            player_pos: [-22_000.0, 0.0, -101_500.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(
        out_cam_lock.camera_override,
        Some(CameraOverrideCommand::Lock(APART_E3FE_ROOM13_CAMERA))
    );

    let mut cry = AnyoneCryProgramState::new();
    let (done0, snd0) = cry.tick(0.0, false, false);
    assert!(!done0);
    assert!(cry.player_locked);
    assert_eq!(cry.stage_anim, Some(0x4e22));
    assert_eq!(
        snd0,
        Some(SpatialSoundCommand::CallPos {
            se_id: SE_APART_ANYONE_CRY,
            volume: 1.0,
            pos: [-99_400.0, -500.0, 26_400.0],
            channel: 6,
        })
    );
    let (done1, _) = cry.tick(2.1, true, true);
    assert!(!done1);
    assert_eq!(cry.s_step, 0x0d);
    let (done2, _) = cry.tick(0.0, true, true);
    assert!(done2);
    assert!(!cry.player_locked);

    let mut runner = StageProgramRunner::new();
    let mut puzzle = PuzzleState::new();
    let mut inv = InventoryManager::new();
    let mut overlay = CutawayOverlayEngine::new();
    runner.start(7, 1, 66);
    assert!(runner.tick(
        0,
        0,
        0.1,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert_eq!(runner.sound_effects(), &[SE_APART_ANYONE_CRY]);
    assert!(runner.tick(
        0,
        0,
        2.0,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert_eq!(runner.active_message_id(), Some(0));
    assert!(runner.tick(
        PROG_KEY_A,
        0,
        0.1,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert!(!runner.tick(
        0,
        0,
        0.1,
        &mut flags,
        &mut puzzle,
        &mut inv,
        &mut overlay,
        RiddleDifficulty::Normal
    ));
    assert!(flags.get(66));

    assert_eq!(
        eval_monkey_kick_206(&flags),
        Some(SpatialSoundCommand::CallPos {
            se_id: SE_APART_MONKEY_KICK_206,
            volume: 1.0,
            pos: [-99_000.0, -500.0, 13_600.0],
            channel: 2,
        })
    );
    flags.set(69, true);
    assert_eq!(eval_monkey_kick_206(&flags), None);

    let tv_load = dispatcher.on_sound_call_after_load(StageId::ApartE2f, 0x18, &mut flags);
    assert_eq!(tv_load.sounds.len(), 1);
    assert_eq!(
        tv_load.sounds[0],
        SpatialSoundCommand::CallPos {
            se_id: SE_APART_ROOM205_TV_STATIC,
            volume: 0.3,
            pos: [-58_082.375, -356.5, 19_011.39],
            channel: 12,
        }
    );
    assert!(flags.get(70));

    dispatcher.on_room_init(StageId::Society, 0x5d, &mut flags, &mut rng);
    let out_soc_stair = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::Society,
            room_id: 0x5d,
            dt: 5.0,
            player_pos: [-11_520.0, 17_500.0, 40_000.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert_eq!(out_soc_stair.sounds.len(), 1);
    match out_soc_stair.sounds[0] {
        SpatialSoundCommand::CallPos {
            se_id,
            volume,
            pos,
            channel,
        } => {
            assert_eq!(se_id, SE_SOCIETY_STAIRCASE_AMBIENT);
            assert!((volume - 0.25).abs() < 1e-4);
            assert_eq!(pos[1], 42_670.0);
            assert_eq!(channel, 6);
        }
        other => panic!("Expected CallPos for SE 0x9c73, got {other:?}"),
    }

    flags.set(FLAG_SOCIETY_ROOM5C_TRIGGERED, false);
    let out_soc_5c = dispatcher.on_all_time(
        &StageHookContext {
            stage: StageId::Society,
            room_id: 0x5c,
            dt: 0.1,
            player_pos: [-19_000.0, 0.0, -18_900.0],
            player_rot_y: 0.0,
            active_sounds: [0; 4],
        },
        &mut flags,
        &mut rng,
    );
    assert!(flags.get(FLAG_SOCIETY_ROOM5C_TRIGGERED));
    assert_eq!(
        out_soc_5c.sounds,
        vec![SpatialSoundCommand::CallPos {
            se_id: SE_SOCIETY_ROOM_5C_HORROR,
            volume: 1.0,
            pos: [-22_000.0, -500.0, -18_900.0],
            channel: 8,
        }]
    );
}
