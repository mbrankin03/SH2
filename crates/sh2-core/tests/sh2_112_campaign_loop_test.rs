//! Integration & state-machine parity tests for the Unified Campaign Game Loop (`SH2-112`,
//! `gamemain.c` / `PlayableMain` `0x0020b3c0` / `connectMain` `0x0020bf80`).

use sh2_core::campaign::{
    resolve_stage_manifest, room_area_dir, CampaignOrchestrator, CampaignPhase, CampaignTickInput,
    FmvTrigger, OverlayMenuKind, ScreenFadeState, MAIN_SCENARIO_START_STAGE,
    PS2_DEFAULT_FADE_SPEED, SE_GAME_OVER, SH2EE_DELAYED_FADE_IN_START, WISH_SCENARIO_START_STAGE,
};
use sh2_core::config::{ActionDifficulty, OutdoorStreamingMode, RiddleDifficulty};
use sh2_core::cutscene::{CS_INTRO_BATHROOM_DEMO_NO, CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO};
use sh2_core::events::StageTransitionCommand;
use sh2_core::scenario::{PlayableCharacter, ScenarioKind};
use sh2_core::simulation::SimulationCore;

/// Helper that drives `RoomLoading` / `DoorTransition` through all `connectMain` steps (`0..=7`),
/// simulating the thin `sh2-app` shell acknowledging `load_room`.
fn drive_connect_to_completion(
    orch: &mut CampaignOrchestrator,
    sim: &mut SimulationCore,
    dt_sec: f32,
) -> (bool, bool, Option<u16>) {
    let mut saw_loading_screen = false;
    let mut saw_load_room = false;
    let mut loaded_stage = None;
    let input = CampaignTickInput {
        prefer_sh2ee: true,
        ..CampaignTickInput::default()
    };

    for _ in 0..48 {
        let eff = orch.step(sim, &input, dt_sec);
        if eff.show_loading_screen.is_some() {
            saw_loading_screen = true;
        }
        if let Some(manifest) = eff.load_room {
            saw_load_room = true;
            loaded_stage = Some(manifest.stage_id);
            orch.notify_room_assets_loaded();
        }
        if !matches!(
            orch.phase,
            CampaignPhase::Prologue { .. }
                | CampaignPhase::RoomLoading { .. }
                | CampaignPhase::DoorTransition { .. }
        ) {
            break;
        }
    }
    (saw_loading_screen, saw_load_room, loaded_stage)
}

#[test]
fn main_scenario_prologue_loads_stage2_triggers_first_toilet_and_hands_off_without_smear() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let mut orch = CampaignOrchestrator::new_game(
        ScenarioKind::LetterFromSilentHeaven,
        ActionDifficulty::Normal,
        RiddleDifficulty::Normal,
        OutdoorStreamingMode::ClassicPs2LoadBg2x2,
        true,
    );

    assert!(matches!(orch.phase, CampaignPhase::Prologue { .. }));
    let (saw_loading, saw_load_room, loaded_stage) =
        drive_connect_to_completion(&mut orch, &mut sim, 1.0 / 60.0);

    assert!(saw_loading, "Prologue must trigger NowLoadingEnable");
    assert!(saw_load_room, "connectMain must emit RoomLoadManifest");
    assert_eq!(loaded_stage, Some(MAIN_SCENARIO_START_STAGE));
    assert_eq!(sim.scenario.current_stage, MAIN_SCENARIO_START_STAGE);
    assert_eq!(sim.scenario.current_room, "er01");

    // Stage 2 (`stg_toilet.c`) automatically triggers `first_toilet` (`demo_no == 1`).
    assert!(
        matches!(
            orch.phase,
            CampaignPhase::DramaDemo { demo_no, .. } if demo_no == u32::from(CS_INTRO_BATHROOM_DEMO_NO)
        ),
        "Expected automatic first_toilet DramaDemo, got {:?}",
        orch.phase
    );

    // Finish the cutscene and verify zero-smear post-cutscene `CharaSetPos` + camera snap.
    let finish_input = CampaignTickInput {
        drama_demo_finished: true,
        prefer_sh2ee: true,
        ..CampaignTickInput::default()
    };
    let eff = orch.step(&mut sim, &finish_input, 1.0 / 60.0);
    assert_eq!(orch.phase, CampaignPhase::ActiveGameplay);
    assert!(eff.allow_gameplay_sim);
    assert!(sim.scenario.flags.get(0x0020));
    assert_eq!(sim.prev_player_position, sim.scenario.player_position);
    assert_eq!(sim.camera.prev_snapshot, sim.camera.curr_snapshot);
}

#[test]
fn born_from_a_wish_starts_in_heavens_night_as_maria() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let mut orch = CampaignOrchestrator::new_game(
        ScenarioKind::BornFromAWish,
        ActionDifficulty::Hard,
        RiddleDifficulty::Extra,
        OutdoorStreamingMode::NativeWholeTown,
        false,
    );

    let (_, saw_load_room, loaded_stage) =
        drive_connect_to_completion(&mut orch, &mut sim, 1.0 / 60.0);
    assert!(saw_load_room);
    assert_eq!(loaded_stage, Some(WISH_SCENARIO_START_STAGE));
    assert_eq!(sim.scenario.character, PlayableCharacter::Maria);
    assert_eq!(sim.scenario.current_room, "th09");
    assert_eq!(orch.phase, CampaignPhase::ActiveGameplay);
}

#[test]
fn door_transition_resolves_connected_stage_pair_and_triggers_outdoor_observation_demo() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    let mut orch = CampaignOrchestrator::from_saved_stage(
        2,
        Some("er01".to_string()),
        [-19_606.6, 0.0, 20_429.55],
        -std::f32::consts::FRAC_PI_2,
        OutdoorStreamingMode::ClassicPs2LoadBg2x2,
        true,
    );
    // Mark opening restroom demo already seen so we land directly in ActiveGameplay.
    sim.scenario.flags.set(0x0020, true);
    let _ = drive_connect_to_completion(&mut orch, &mut sim, 1.0 / 60.0);
    assert_eq!(orch.phase, CampaignPhase::ActiveGameplay);

    // Transition Stage 2 (Restroom) -> Stage 3 (Observation Deck) via EventExecMove command.
    let cmd = StageTransitionCommand {
        dest_stage: 3,
        stage_change_mode: 3,
        is_connected_stage: false,
        observation_mode: false,
        dest_pos: [1500.0, 0.0, -2500.0],
        dest_rot_y: 0.0,
    };
    orch.request_transition(2, cmd, None);
    assert!(matches!(orch.phase, CampaignPhase::DoorTransition { .. }));

    // Step through fade-out (~1s) + connectMain.
    let (saw_loading, saw_load_room, loaded_stage) =
        drive_connect_to_completion(&mut orch, &mut sim, 0.1);
    assert!(saw_loading);
    assert!(saw_load_room);
    assert_eq!(loaded_stage, Some(3));
    assert_eq!(sim.scenario.current_room, "ob01");
    assert!(matches!(
        orch.phase,
        CampaignPhase::DramaDemo { demo_no, .. } if demo_no == u32::from(CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO)
    ));

    // Connected sub-stage pair check: Wood Side Apartments 1F (6) -> 2F (7).
    let apt_cmd = StageTransitionCommand {
        dest_stage: 7,
        stage_change_mode: 7,
        is_connected_stage: false,
        observation_mode: false,
        dest_pos: [0.0, 0.0, 0.0],
        dest_rot_y: 0.0,
    };
    orch.request_transition(6, apt_cmd, None);
    let _ = drive_connect_to_completion(&mut orch, &mut sim, 0.1);
    let manifest = orch.active_manifest.as_ref().expect("manifest populated");
    assert_eq!(manifest.stage_id, 7);
    assert!(
        manifest.is_connected_stage,
        "(6, 7) must be detected as a CONNECT_STAGE_PAIRS pair"
    );
    assert!(
        !manifest.enemy_spawns.is_empty() || !manifest.room_pickups.is_empty(),
        "Wood Side Apartments 2F manifest must populate retail entities"
    );
}

#[test]
fn delayed_fade_in_fix_holds_black_before_ramping_while_vanilla_ps2_ramps_immediately() {
    let mut enhanced = ScreenFadeState::new_black();
    enhanced.start_fade_in(PS2_DEFAULT_FADE_SPEED, true);
    assert_eq!(enhanced.black_alpha(), 1.0);
    // Advance less than |SH2EE_DELAYED_FADE_IN_START| / PS2_DEFAULT_FADE_SPEED (~0.111s):
    enhanced.tick(0.05);
    assert_eq!(
        enhanced.black_alpha(),
        1.0,
        "SH2:EE DelayedFadeIn ({SH2EE_DELAYED_FADE_IN_START}) must stay opaque black during pre-roll"
    );
    enhanced.tick(0.20);
    assert!(enhanced.black_alpha() < 1.0);

    let mut vanilla = ScreenFadeState::new_black();
    vanilla.start_fade_in(PS2_DEFAULT_FADE_SPEED, false);
    vanilla.tick(0.05);
    assert!(
        vanilla.black_alpha() < 1.0,
        "1:1 vanilla PS2 fade-in must ramp immediately on the first tick"
    );
}

#[test]
fn fmv_overlay_gameover_and_ending_credits_lifecycle() {
    let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.flags.set(0x0020, true);
    let mut orch = CampaignOrchestrator::from_saved_stage(
        2,
        Some("er01".to_string()),
        [0.0, 0.0, 0.0],
        0.0,
        OutdoorStreamingMode::ClassicPs2LoadBg2x2,
        false,
    );
    let _ = drive_connect_to_completion(&mut orch, &mut sim, 0.1);

    // 1. OverlayMenu: Map advances play_time_secs, Pause does not.
    let _ = orch.step(
        &mut sim,
        &CampaignTickInput {
            overlay_toggle: Some(OverlayMenuKind::Map),
            ..Default::default()
        },
        0.016,
    );
    assert_eq!(orch.phase, CampaignPhase::OverlayMenu(OverlayMenuKind::Map));
    let t_before_map = orch.play_time_secs;
    let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0);
    assert!(orch.play_time_secs > t_before_map);
    let _ = orch.step(
        &mut sim,
        &CampaignTickInput {
            overlay_toggle: Some(OverlayMenuKind::Map),
            ..Default::default()
        },
        0.016,
    );
    assert_eq!(orch.phase, CampaignPhase::ActiveGameplay);

    // 2. FMV Playback -> returns cleanly to ActiveGameplay.
    let mut dummy_eff = Default::default();
    orch.start_fmv(
        FmvTrigger {
            movie_id: "murder",
            post_stage: None,
            post_pos: None,
            post_yaw: None,
        },
        &mut dummy_eff,
    );
    assert!(matches!(
        orch.phase,
        CampaignPhase::FmvPlayback {
            movie_id: "murder",
            ..
        }
    ));
    let _ = orch.step(
        &mut sim,
        &CampaignTickInput {
            fmv_finished: true,
            ..Default::default()
        },
        0.016,
    );
    assert_eq!(orch.phase, CampaignPhase::ActiveGameplay);

    // 3. EndingCredits -> Results -> Return to Title.
    orch.trigger_ending(52);
    assert_eq!(
        orch.phase,
        CampaignPhase::EndingCredits {
            ending_stage: 52,
            showing_results: false
        }
    );
    let eff_res = orch.step(
        &mut sim,
        &CampaignTickInput {
            confirm_terminal_exit: true,
            ..Default::default()
        },
        0.016,
    );
    assert!(!eff_res.return_to_title);
    assert_eq!(
        orch.phase,
        CampaignPhase::EndingCredits {
            ending_stage: 52,
            showing_results: true
        }
    );
    let eff_exit = orch.step(
        &mut sim,
        &CampaignTickInput {
            confirm_terminal_exit: true,
            ..Default::default()
        },
        0.016,
    );
    assert!(eff_exit.return_to_title);

    // 4. GameOver when health <= 0.0 emits SE_GAME_OVER (0x3f4).
    orch.phase = CampaignPhase::ActiveGameplay;
    sim.scenario.health = 0.0;
    let eff_dead = orch.step(&mut sim, &CampaignTickInput::default(), 0.016);
    assert_eq!(orch.phase, CampaignPhase::GameOver);
    assert!(eff_dead.sound_effects.contains(&SE_GAME_OVER));
}

#[test]
fn delayed_fade_in_option_integrates_with_enhanced_presets() {
    let mut cfg = sh2_core::config::AppConfig::default();
    cfg.apply_enhanced_preset(sh2_core::config::EnhancedPreset::Ps2Authentic);
    assert!(!cfg.game.enable_delayed_fade_in);
    assert_eq!(
        cfg.detect_enhanced_preset(),
        sh2_core::config::EnhancedPreset::Ps2Authentic
    );

    cfg.game.enable_delayed_fade_in = true;
    assert_eq!(
        cfg.detect_enhanced_preset(),
        sh2_core::config::EnhancedPreset::Custom
    );

    cfg.apply_enhanced_preset(sh2_core::config::EnhancedPreset::Sh2ee);
    assert!(cfg.game.enable_delayed_fade_in);
    assert_eq!(
        cfg.detect_enhanced_preset(),
        sh2_core::config::EnhancedPreset::Sh2ee
    );
}

#[test]
fn all_58_retail_stages_resolve_valid_manifests() {
    assert_eq!(room_area_dir("ma01"), "bg2/ma");
    assert_eq!(room_area_dir("er01"), "bg/er");
    for stage_id in 1..=58 {
        let m = resolve_stage_manifest(stage_id, None, false, true);
        assert_eq!(m.stage_id, stage_id);
        assert!(m.map_rel_path.ends_with(".map"));
        assert!(m.cld_rel_path.ends_with(".cld"));
        assert!(m.cam_rel_path.ends_with(".cam"));
    }
}
