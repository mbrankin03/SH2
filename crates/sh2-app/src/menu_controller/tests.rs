use super::*;
use sh2_ui::menu::{MenuNavAction, MenuSoundCue};
use sh2_ui::pause::{PauseAction, PauseSubPhase};

#[test]
fn test_menu_controller_lifecycle_and_persistence() {
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_sh2_config.json");
    let _ = std::fs::remove_file(&cfg_path);

    let mut controller = AppMenuController::new(&cfg_path);
    assert_eq!(controller.title_sm.phase, TitlePhase::SplashIntro);
    assert!(!controller.in_game_active);

    // Skip to title logo
    controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::TitleLogo);

    // Enter main menu
    controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);

    // Navigate to Options (SaveAvailability::None -> [NewGame=0, Options=1, Movies=2, Exit=3])
    controller.handle_nav(MenuNavAction::Down); // Options
    controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::OptionsMenu);

    // Enter Advanced Options (row 1 on 10-row Root), change resolution, and commit
    controller.handle_nav(MenuNavAction::Down); // Advanced Options
    controller.handle_nav(MenuNavAction::Confirm);
    controller.handle_nav(MenuNavAction::Right);
    assert!(controller.options_menu.is_dirty);
    controller.handle_nav(MenuNavAction::Confirm);
    assert!(!controller.options_menu.is_dirty);
    assert!(cfg_path.exists());

    // Return to Root Options, then to main menu
    controller.handle_nav(MenuNavAction::Cancel);
    controller.handle_nav(MenuNavAction::Cancel);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);

    // Start new game
    controller.title_sm.main_menu_index = 0;
    controller.handle_nav(MenuNavAction::Confirm); // Scenario
    controller.handle_nav(MenuNavAction::Confirm); // Action diff
    controller.handle_nav(MenuNavAction::Confirm); // Riddle diff
    controller.handle_nav(MenuNavAction::Confirm); // Transition to game

    assert_eq!(controller.title_sm.phase, TitlePhase::TransitionToGame);

    // Simulate fade out finish
    controller.title_sm.fade_alpha = 0.0;
    controller.title_sm.is_fading_out = false;
    controller.tick(0.016);
    assert!(controller.in_game_active);

    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn test_menu_controller_in_game_pause_flow() {
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_sh2_pause_config.json");
    let _ = std::fs::remove_file(&cfg_path);

    let mut controller = AppMenuController::new(&cfg_path);
    controller.in_game_active = true;
    assert!(!controller.pause_menu.is_open);

    // Open pause menu
    controller.pause_menu.open(false, "Toluca Lake", 120.0);
    assert!(controller.pause_menu.is_open);

    // Navigate Down (skips Save) -> Load (index 2)
    let (cue, action) = controller.handle_pause_nav(MenuNavAction::Down);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(action, PauseAction::None);
    assert_eq!(controller.pause_menu.selected_index, 2);

    // Down -> Options (index 3)
    controller.handle_pause_nav(MenuNavAction::Down);
    assert_eq!(controller.pause_menu.selected_index, 3);

    // Confirm -> Open Options sub-phase
    let (cue, action) = controller.handle_pause_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(action, PauseAction::OpenOptions);
    assert_eq!(
        controller.pause_menu.sub_phase,
        PauseSubPhase::InOptionsMenu
    );

    // In options, test tab switching
    let tab_cue = controller.switch_options_tab(true);
    assert_eq!(tab_cue, MenuSoundCue::CursorMove);

    // In options, cancel returns to pause menu Main sub-phase
    let (cue, action) = controller.handle_pause_nav(MenuNavAction::Cancel);
    assert_eq!(cue, MenuSoundCue::Cancel);
    assert_eq!(action, PauseAction::None);
    assert_eq!(controller.pause_menu.sub_phase, PauseSubPhase::Main);

    // Navigate to Quit (index 4)
    controller.handle_pause_nav(MenuNavAction::Down);
    assert_eq!(controller.pause_menu.selected_index, 4);

    // Confirm Quit -> QuitConfirm sub-phase
    controller.handle_pause_nav(MenuNavAction::Confirm);
    assert_eq!(controller.pause_menu.sub_phase, PauseSubPhase::QuitConfirm);

    // Toggle to YES and confirm -> QuitToTitle
    controller.handle_pause_nav(MenuNavAction::Left);
    let (cue, action) = controller.handle_pause_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(action, PauseAction::QuitToTitle);
    assert!(!controller.pause_menu.is_open);
    assert!(!controller.in_game_active);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);

    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn test_loading_screen_transition_no_38x_reset_loop_and_preload() {
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_sh2_loading_lifecycle_config.json");
    let _ = std::fs::remove_file(&cfg_path);

    let mut controller = AppMenuController::new(&cfg_path);
    controller
        .options_menu
        .draft_config
        .debug
        .min_loading_screen_seconds = 2;
    controller
        .options_menu
        .draft_config
        .debug
        .require_press_to_continue = false;
    controller
        .options_menu
        .draft_config
        .debug
        .instant_room_transitions = false;

    // Enter TransitionToGame with active fade_out (fade_alpha = 1.0, is_fading_out = true)
    controller.title_sm.phase = TitlePhase::TransitionToGame;
    controller.title_sm.fade_alpha = 1.0;
    controller.title_sm.start_fade_out();
    controller.title_sm.launch_target =
        LaunchTarget::TestRoom(sh2_ui::menu::TestRoomKind::ObservationDeck);

    let mut loading_screen: Option<LoadingScreen> = None;
    let mut instantiations = 0usize;
    let mut preload_requests = 0usize;
    let mut footsteps_triggered = 0usize;
    let mut entered_game_ticks = 0usize;

    // Simulate 200 frames at 60 Hz (~3.33s)
    for _ in 0..200 {
        let was_none = loading_screen.is_none();
        let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
        if was_none && loading_screen.is_some() {
            instantiations += 1;
        }
        if step.footstep_spawned.is_some() {
            footsteps_triggered += 1;
        }
        if step.needs_room_preload {
            preload_requests += 1;
            if let Some(ref mut ls) = loading_screen {
                ls.mark_completed();
            }
        }
        if step.entered_game {
            entered_game_ticks += 1;
        }
    }

    // LoadingScreen must be instantiated EXACTLY ONCE (never 38x reset loop)
    assert_eq!(instantiations, 1);
    assert_eq!(preload_requests, 1);
    assert!(footsteps_triggered >= 2);
    assert_eq!(entered_game_ticks, 1);
    assert!(controller.in_game_active);
    assert!(loading_screen.is_none());
    assert_eq!(controller.title_sm.fade_alpha, 0.0);
    assert!(!controller.title_sm.is_fading_out);

    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn test_loading_screen_instant_transition_completes_immediately() {
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_sh2_loading_instant_config.json");
    let _ = std::fs::remove_file(&cfg_path);

    let mut controller = AppMenuController::new(&cfg_path);
    controller
        .options_menu
        .draft_config
        .debug
        .instant_room_transitions = true;
    controller.title_sm.phase = TitlePhase::TransitionToGame;
    controller.title_sm.fade_alpha = 1.0;
    controller.title_sm.start_fade_out();
    controller.title_sm.launch_target =
        LaunchTarget::TestRoom(sh2_ui::menu::TestRoomKind::ObservationDeck);

    let mut loading_screen: Option<LoadingScreen> = None;
    let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
    assert!(step.entered_game);
    assert!(controller.in_game_active);
    assert!(loading_screen.is_none());

    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn test_loading_screen_require_press_to_continue_confirm_transitions_once() {
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_sh2_loading_press_confirm_config.json");
    let _ = std::fs::remove_file(&cfg_path);

    let mut controller = AppMenuController::new(&cfg_path);
    controller
        .options_menu
        .draft_config
        .debug
        .min_loading_screen_seconds = 1;
    controller
        .options_menu
        .draft_config
        .debug
        .require_press_to_continue = true;
    controller
        .options_menu
        .draft_config
        .debug
        .instant_room_transitions = false;
    controller.title_sm.phase = TitlePhase::TransitionToGame;
    controller.title_sm.fade_alpha = 1.0;
    controller.title_sm.start_fade_out();
    controller.title_sm.launch_target =
        LaunchTarget::TestRoom(sh2_ui::menu::TestRoomKind::ObservationDeck);

    let mut loading_screen: Option<LoadingScreen> = None;

    // Tick 90 frames at 60 Hz (1.5s > min_duration 1.0s), preloading on Frame 2
    for _ in 0..90 {
        let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
        if step.needs_room_preload {
            if let Some(ref mut ls) = loading_screen {
                ls.mark_completed();
            }
        }
        assert!(!step.entered_game);
    }
    // Still waiting for user confirm
    assert!(!controller.in_game_active);
    assert!(loading_screen.is_some());

    // User presses Confirm -> `handle_confirm()` marks `transition_ready = true`
    assert!(loading_screen.as_mut().unwrap().handle_confirm());

    // Next tick completes transition cleanly into `in_game_active = true` without looping
    let final_step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
    assert!(final_step.entered_game);
    assert!(controller.in_game_active);
    assert!(loading_screen.is_none());

    // Subsequent ticks never re-instantiate `LoadingScreen`
    for _ in 0..30 {
        let post_step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
        assert!(!post_step.entered_game);
        assert!(loading_screen.is_none());
    }

    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn test_new_game_skips_loading_screen_straight_to_black_and_fmv() {
    let temp_dir = std::env::temp_dir();
    let cfg_path = temp_dir.join("test_sh2_new_game_no_loading_config.json");
    let _ = std::fs::remove_file(&cfg_path);

    let mut controller = AppMenuController::new(&cfg_path);
    controller
        .options_menu
        .draft_config
        .debug
        .instant_room_transitions = false;
    controller
        .options_menu
        .draft_config
        .debug
        .min_loading_screen_seconds = 3;
    controller.title_sm.phase = TitlePhase::TransitionToGame;
    controller.title_sm.launch_target = LaunchTarget::Campaign {
        scenario: sh2_core::ScenarioKind::LetterFromSilentHeaven,
        is_continue: false,
    };

    let mut loading_screen: Option<LoadingScreen> = None;
    let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
    assert!(step.entered_game);
    assert!(controller.in_game_active);
    assert!(
        loading_screen.is_none(),
        "New Game must go straight to black screen + opening FMV with no loading screen"
    );

    let _ = std::fs::remove_file(&cfg_path);
}
