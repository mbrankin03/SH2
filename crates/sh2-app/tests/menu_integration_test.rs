//! Integration test for SH2-18 Main Menu, Scenario Select, and Options Menu.

use sh2_app::AppMenuController;
use sh2_core::config::{
    ActionDifficulty, AppConfig, ButtonGlyphStyle, FogQuality, GameLanguage, InternalResolution,
    RiddleDifficulty, WindowDisplayMode,
};
use sh2_core::scenario::ScenarioKind;
use sh2_ui::menu::{MainMenuOption, MenuNavAction, MenuSoundCue, TitlePhase};
use sh2_ui::options::OptionsTab;
use std::fs;

#[test]
fn test_sh2_18_full_menu_and_options_flow() {
    let test_dir = std::env::temp_dir().join("sh2_18_integration_test");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let config_path = test_dir.join("test_config.json");

    let mut controller = AppMenuController::new(&config_path);

    // 1. Initial State: Splash Screen Intro (start00.tex)
    assert_eq!(controller.title_sm.phase, TitlePhase::SplashIntro);
    assert!(!controller.in_game_active);
    assert!(controller.title_sm.fade_alpha >= 0.0);

    // Tick advances splash timer
    controller.tick(1.0);
    assert!(controller.title_sm.splash_timer >= 1.0);

    // Skip splash intro via Confirm (Enter / Cross / A)
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::TitleLogo);

    // Press Confirm on Title Logo (start01.tex) to reach Main Menu
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);
    assert_eq!(controller.title_sm.main_menu_index, 0);

    // 2. Main Menu Navigation: Navigate down through options
    let cue = controller.handle_nav(MenuNavAction::Down);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        MainMenuOption::ALL[controller.title_sm.main_menu_index],
        MainMenuOption::Continue
    );

    let cue = controller.handle_nav(MenuNavAction::Down);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        MainMenuOption::ALL[controller.title_sm.main_menu_index],
        MainMenuOption::Options
    );

    // 3. Open Options Menu (SH2:EE Options Architecture)
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::OptionsMenu);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Video);

    // Video Options: Modify Resolution (Native -> 1080p)
    assert_eq!(controller.options_menu.selected_index, 0);
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.video.resolution,
        InternalResolution::Res1080p
    );
    assert!(controller.options_menu.is_dirty);

    // Cycle through Display Mode (Borderless -> Windowed)
    controller.handle_nav(MenuNavAction::Down); // Index 1: Display Mode
    assert_eq!(controller.options_menu.selected_index, 1);
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.video.display_mode,
        WindowDisplayMode::Windowed
    );

    // Toggle V-Sync
    controller.handle_nav(MenuNavAction::Down); // Index 2: V-Sync
    controller.handle_nav(MenuNavAction::Right);
    assert!(!controller.options_menu.draft_config.video.vsync);

    // Toggle Fog Quality (Modern Volumetric -> Classic PS2)
    controller.handle_nav(MenuNavAction::Down); // Index 3: Framerate
    controller.handle_nav(MenuNavAction::Down); // Index 4: Fog Quality
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.video.fog_quality,
        FogQuality::ClassicPs2
    );

    // Adjust Film Grain Slider (65% -> 75%)
    controller.handle_nav(MenuNavAction::Down); // Index 5: Film Grain
    controller.handle_nav(MenuNavAction::Right);
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.video.grain_intensity,
        75
    );

    // Switch Category Tabs: Video -> Audio
    controller.switch_options_tab(true);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Audio);
    assert_eq!(controller.options_menu.selected_index, 0);

    // Adjust Master Volume (100% -> 80%)
    controller.handle_nav(MenuNavAction::Left);
    controller.handle_nav(MenuNavAction::Left);
    controller.handle_nav(MenuNavAction::Left);
    controller.handle_nav(MenuNavAction::Left);
    assert!((controller.options_menu.draft_config.audio.master_volume - 0.80).abs() < 0.001);

    // Switch Category Tabs: Audio -> Controls
    controller.switch_options_tab(true);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Controls);
    assert_eq!(controller.options_menu.selected_index, 0);

    // Toggle 3D Tank Controls to 2D Directional Controls
    controller.handle_nav(MenuNavAction::Right);
    assert!(!controller.options_menu.draft_config.controls.tank_controls);

    // Change Button Glyphs (PlayStation -> Xbox)
    controller.handle_nav(MenuNavAction::Down);
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.controls.glyph_style,
        ButtonGlyphStyle::Xbox
    );

    // Switch Category Tabs: Controls -> Game
    controller.switch_options_tab(true);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Game);
    assert_eq!(controller.options_menu.selected_index, 0); // Action Diff

    // Change Language (English -> Japanese): Index 3
    controller.handle_nav(MenuNavAction::Down); // Index 1: Riddle Diff
    controller.handle_nav(MenuNavAction::Down); // Index 2: Subtitles
    controller.handle_nav(MenuNavAction::Down); // Index 3: Language
    assert_eq!(controller.options_menu.selected_index, 3);
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.game.language,
        GameLanguage::Japanese
    );
    assert_eq!(
        controller.options_menu.draft_config.game.language.code(),
        'j'
    );

    // Commit and persist settings to file
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert!(!controller.options_menu.is_dirty);
    assert!(config_path.exists());

    // Verify persisted config on disk matches applied changes
    let reloaded = AppConfig::load_from_file(&config_path);
    assert_eq!(reloaded.video.resolution, InternalResolution::Res1080p);
    assert_eq!(reloaded.video.display_mode, WindowDisplayMode::Windowed);
    assert!(!reloaded.video.vsync);
    assert_eq!(reloaded.video.fog_quality, FogQuality::ClassicPs2);
    assert_eq!(reloaded.video.grain_intensity, 75);
    assert!((reloaded.audio.master_volume - 0.80).abs() < 0.001);
    assert!(!reloaded.controls.tank_controls);
    assert_eq!(reloaded.controls.glyph_style, ButtonGlyphStyle::Xbox);
    assert_eq!(reloaded.game.language, GameLanguage::Japanese);

    // Return from Options to Main Menu
    let cue = controller.handle_nav(MenuNavAction::Cancel);
    assert_eq!(cue, MenuSoundCue::Cancel);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);

    // 4. Start New Game: Scenario & Difficulty Selection
    controller.title_sm.main_menu_index = 0; // New Game
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::ScenarioSelect);

    // Select "Born from a Wish" (Maria Sub-Scenario)
    controller.handle_nav(MenuNavAction::Down);
    assert_eq!(controller.title_sm.scenario_index, 1);
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(
        controller.title_sm.selected_scenario,
        ScenarioKind::BornFromAWish
    );
    assert_eq!(
        controller.title_sm.phase,
        TitlePhase::ActionDifficultySelect
    );

    // Select Hard Action Difficulty
    controller.handle_nav(MenuNavAction::Down);
    assert_eq!(
        controller.title_sm.action_difficulty,
        ActionDifficulty::Hard
    );
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(
        controller.title_sm.phase,
        TitlePhase::RiddleDifficultySelect
    );

    // Select Extra Riddle Difficulty
    controller.handle_nav(MenuNavAction::Down);
    controller.handle_nav(MenuNavAction::Down);
    assert_eq!(
        controller.title_sm.riddle_difficulty,
        RiddleDifficulty::Extra
    );

    // Confirm game launch: Emits authentic audio transition cue 15002
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(controller.title_sm.phase, TitlePhase::TransitionToGame);

    // Simulate transition fadeout completion
    controller.title_sm.fade_alpha = 0.0;
    controller.title_sm.is_fading_out = false;
    controller.tick(0.016);
    assert!(controller.in_game_active);

    // 5. Test Rooms / Debug Stage Selection flow via Options Menu
    let mut debug_controller = AppMenuController::new(&config_path);
    debug_controller.title_sm.phase = TitlePhase::OptionsMenu;
    debug_controller.options_menu.active_tab = sh2_ui::OptionsTab::Game;
    debug_controller.options_menu.selected_index = 5; // Test Rooms

    let cue = debug_controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(debug_controller.title_sm.phase, TitlePhase::TestRoomsSelect);

    // Select Toluca Lake Observation Deck
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::ObservationDeck
    );

    // Navigate up to wrap to the last entry ("FMV CUTSCENE PLAYER (1080P60)", SH2-10),
    // and up once more to "MODEL & TEXTURE VIEWER" (SH2-36)
    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::FmvCutsceneViewer
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "FMV Cutscene Player"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::ModelAndTextureViewer
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Model & Texture Viewer"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(
        debug_controller.title_sm.phase,
        TitlePhase::TransitionToGame
    );
    assert_eq!(
        debug_controller.title_sm.selected_test_room,
        sh2_ui::menu::TestRoomKind::ModelAndTextureViewer
    );

    // 6. Verify launching the FMV Cutscene Viewer directly from Options -> Video tab (SH2-10)
    let mut fmv_opt_controller = AppMenuController::new(&config_path);
    fmv_opt_controller.title_sm.phase = TitlePhase::OptionsMenu;
    // Video tab is index 0; FMV Cutscene Viewer is row index 9
    fmv_opt_controller.options_menu.selected_index = 9;
    let cue = fmv_opt_controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(
        fmv_opt_controller.title_sm.phase,
        TitlePhase::TransitionToGame
    );
    assert_eq!(
        fmv_opt_controller.title_sm.selected_test_room,
        sh2_ui::menu::TestRoomKind::FmvCutsceneViewer
    );

    // 7. Verify In-Game Pause Menu Integration (SH2-45)
    let mut in_game_controller = AppMenuController::new(&config_path);
    in_game_controller.in_game_active = true;
    assert!(!in_game_controller.pause_menu.is_open);

    // ESC / Pause opens the pause menu with location and play time
    in_game_controller
        .pause_menu
        .open(false, "Outdoor Observation Deck", 450.0);
    assert!(in_game_controller.pause_menu.is_open);
    assert_eq!(in_game_controller.pause_menu.selected_index, 0); // CONTINUE

    // Vertical navigation: Down skips disabled SAVE option
    let (cue, action) = in_game_controller.handle_pause_nav(MenuNavAction::Down);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(action, sh2_ui::pause::PauseAction::None);
    assert_eq!(in_game_controller.pause_menu.selected_index, 2); // LOAD

    // Navigate to OPTIONS and confirm
    in_game_controller.handle_pause_nav(MenuNavAction::Down);
    assert_eq!(in_game_controller.pause_menu.selected_index, 3); // OPTIONS
    let (cue, action) = in_game_controller.handle_pause_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(action, sh2_ui::pause::PauseAction::OpenOptions);
    assert_eq!(
        in_game_controller.pause_menu.sub_phase,
        sh2_ui::pause::PauseSubPhase::InOptionsMenu
    );

    // Cancel from Options returns directly to Pause Menu Main sub-phase
    let (cue, action) = in_game_controller.handle_pause_nav(MenuNavAction::Cancel);
    assert_eq!(cue, MenuSoundCue::Cancel);
    assert_eq!(action, sh2_ui::pause::PauseAction::None);
    assert_eq!(
        in_game_controller.pause_menu.sub_phase,
        sh2_ui::pause::PauseSubPhase::Main
    );

    // Confirm CONTINUE resumes gameplay
    in_game_controller.pause_menu.selected_index = 0;
    let (cue, action) = in_game_controller.handle_pause_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(action, sh2_ui::pause::PauseAction::Resume);
    assert!(!in_game_controller.pause_menu.is_open);
    assert!(in_game_controller.in_game_active);

    // Clean up temporary test files
    let _ = fs::remove_dir_all(&test_dir);
}
