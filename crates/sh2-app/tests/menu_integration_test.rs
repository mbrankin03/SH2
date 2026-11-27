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
    controller
        .title_sm
        .set_save_availability(sh2_ui::menu::SaveAvailability::None);

    // 1. Initial State: Splash Screen Intro (start00.tex)
    assert_eq!(controller.title_sm.phase, TitlePhase::SplashIntro);
    assert!(!controller.in_game_active);
    assert!(controller.title_sm.fade_alpha >= 0.0);

    // Tick advances splash timer
    controller.tick(1.0);
    assert!(controller.title_sm.splash_timer >= 1.0);

    // Default boot profile is RetailPc (`sh2pc.exe` `0x00405e80`); `tick(1.0)` finished logo 0
    // FadeIn -> Hold, so 5 Confirm presses finish both logos and reach `TitleLogo`.
    assert_eq!(
        controller.title_sm.boot_sequence.profile,
        sh2_ui::menu::BootProfile::RetailPc
    );
    let mut cue = MenuSoundCue::None;
    for _ in 0..5 {
        cue = controller.handle_nav(MenuNavAction::Confirm);
    }
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::TitleLogo);

    // Press Confirm on Title Logo (start01.tex) to reach Main Menu
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);
    assert_eq!(controller.title_sm.main_menu_index, 0);

    // 2. Main Menu Navigation (SaveAvailability::None -> [NewGame=0, Options=1, Movies=2, Exit=3])
    let cue = controller.handle_nav(MenuNavAction::Down);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        controller.title_sm.selected_main_menu_option(),
        MainMenuOption::Options
    );

    // 3. Open Options Menu (SH2:EE Options Architecture)
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::OptionsMenu);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Root);
    controller.handle_nav(MenuNavAction::Down); // Advanced Options
    controller.handle_nav(MenuNavAction::Confirm);
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

    // Toggle Fog Quality (Classic PS2 -> Modern Volumetric)
    controller.handle_nav(MenuNavAction::Down); // Index 3: Framerate
    controller.handle_nav(MenuNavAction::Down); // Index 4: Fog Quality
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller.options_menu.draft_config.video.fog_quality,
        FogQuality::ModernVolumetric
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

    // Adjust Master Volume (16 PS2 levels: 15 -> 12 = 12/15 = 0.80)
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
    assert_eq!(reloaded.video.fog_quality, FogQuality::ModernVolumetric);
    assert_eq!(reloaded.video.grain_intensity, 75);
    assert!((reloaded.audio.master_volume - 0.80).abs() < 0.001);
    assert!(!reloaded.controls.tank_controls);
    assert_eq!(reloaded.controls.glyph_style, ButtonGlyphStyle::Xbox);
    assert_eq!(reloaded.game.language, GameLanguage::Japanese);

    // Return from Game submenu -> Root Options -> Main Menu
    let cue = controller.handle_nav(MenuNavAction::Cancel);
    assert_eq!(cue, MenuSoundCue::Cancel);
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Root);
    let cue = controller.handle_nav(MenuNavAction::Cancel);
    assert_eq!(cue, MenuSoundCue::Cancel);
    assert_eq!(controller.title_sm.phase, TitlePhase::MainMenu);

    // 4. Start New Game: Scenario & Difficulty Selection
    controller.title_sm.main_menu_index = 0; // New Game
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(controller.title_sm.phase, TitlePhase::ScenarioSelect);

    // Select "Letter From Silent Heaven" (Main Scenario -> Action + Riddle difficulties)
    controller.title_sm.riddle_clear_mask = 0x07; // Cleared Easy + Normal + Hard -> Extra unlock
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(
        controller.title_sm.selected_scenario,
        ScenarioKind::LetterFromSilentHeaven
    );
    assert_eq!(
        controller.title_sm.phase,
        TitlePhase::ActionDifficultySelect
    );

    // Select Hard Action Difficulty (Up from Normal in Hard/Normal/Easy/Beginner column)
    controller.handle_nav(MenuNavAction::Up);
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

    // Select Hard Riddle Difficulty (Up from Normal); with 0x07 clear mask upgrades to Extra (`0x0048aa20`)
    controller.handle_nav(MenuNavAction::Up);
    assert_eq!(
        controller.title_sm.riddle_difficulty,
        RiddleDifficulty::Hard
    );

    // Confirm game launch: Emits authentic audio transition cue 15002 & upgrades Hard -> Extra
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(
        controller.title_sm.riddle_difficulty,
        RiddleDifficulty::Extra
    );
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
    debug_controller.options_menu.selected_index = 7; // Test Rooms (row index 7 now that Delayed Fade-In is row 6)

    let cue = debug_controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::Confirm);
    assert_eq!(debug_controller.title_sm.phase, TitlePhase::TestRoomsSelect);

    // Select Toluca Lake Observation Deck
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::ObservationDeck
    );

    // Navigate up to wrap to the last entry ("Story Stages, Riddles & Endings (SH2-65)"),
    // then up through "STAGE EVENTS & HORROR TRIGGERS ROOM" (SH2-47),
    // "PUZZLE LOGIC & CUTAWAY OVERLAYS ROOM" (SH2-15),
    // "INVENTORY & 3D/2D ITEM INSPECT ROOM" (SH2-15),
    // "SURFACE FOOTSTEP & 3D SFX ROOM" (SH2-61), "AUDIO & AIX MIXING DEBUG ROOM" (SH2-60),
    // "GRAPPLE ANIMATION ROOM" (SH2-57), "COMBAT & WEAPON TESTING ARENA" (SH2-13),
    // and "LOADING SCREEN PREVIEW" (SH2-44)
    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::StoryStagesAndEndingsRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Story Stages, Riddles & Endings (SH2-65)"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::HorrorTriggersRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Stage Events & Horror Triggers Room"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::PuzzleAndCutawayRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Puzzle Logic & Cutaway Overlays Room"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::InventoryAndItemsRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Inventory & 3D/2D Item Inspect Room"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::SurfaceSfxTestRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Surface Footstep & 3D SFX Room"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::AudioDebugRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Audio & AIX Mixing Debug Room"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::GrappleAnimationRoom
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Grapple Animation Room"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::CombatArena
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Combat & Weapon Testing Arena"
    );

    let cue = debug_controller.handle_nav(MenuNavAction::Up);
    assert_eq!(cue, MenuSoundCue::CursorMove);
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index],
        sh2_ui::menu::TestRoomKind::LoadingScreenTest
    );
    assert_eq!(
        sh2_ui::menu::TestRoomKind::ALL[debug_controller.title_sm.test_room_index].label(),
        "Loading Screen Preview"
    );

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
    fmv_opt_controller.options_menu.active_tab = OptionsTab::Video;
    // Video tab is index 0; World & Character Textures is row 9 (SH2-92); FMV Cutscene Viewer is row index 10
    fmv_opt_controller.options_menu.selected_index = 10;
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

#[test]
fn test_options_debug_tab_and_loading_screen_selection() {
    let test_dir = std::env::temp_dir().join("sh2_test_options_debug_tab");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let config_path = test_dir.join("config.json");

    let mut controller = AppMenuController::new(&config_path);
    controller
        .title_sm
        .set_save_availability(sh2_ui::menu::SaveAvailability::None);
    controller.title_sm.phase = TitlePhase::OptionsMenu;

    // Navigate to Debug tab (Tab 4)
    controller.options_menu.active_tab = OptionsTab::Debug;
    controller.options_menu.selected_index = 0;
    assert_eq!(controller.options_menu.active_tab, OptionsTab::Debug);
    assert_eq!(controller.options_menu.item_count(), 6);

    // Toggle Instant Room Transitions
    let (label, val) = controller.options_menu.item_display(0).unwrap();
    assert_eq!(label, "Instant Room Cuts");
    assert_eq!(val, "OFF (Authentic)");
    controller.handle_nav(MenuNavAction::Right);
    let (_, val_after) = controller.options_menu.item_display(0).unwrap();
    assert_eq!(val_after, "ON (Fast / Skip)");
    assert!(
        controller
            .options_menu
            .draft_config
            .debug
            .instant_room_transitions
    );

    // Row 1: Min Loading Display
    controller.handle_nav(MenuNavAction::Down);
    assert_eq!(controller.options_menu.selected_index, 1);
    controller.handle_nav(MenuNavAction::Right);
    assert_eq!(
        controller
            .options_menu
            .draft_config
            .debug
            .min_loading_screen_seconds,
        3
    );

    // Row 2: Prompt To Continue
    controller.handle_nav(MenuNavAction::Down);
    assert_eq!(controller.options_menu.selected_index, 2);
    controller.handle_nav(MenuNavAction::Right);
    assert!(
        controller
            .options_menu
            .draft_config
            .debug
            .require_press_to_continue
    );

    // Row 3: Loading Screen Preview launcher
    controller.handle_nav(MenuNavAction::Down);
    assert_eq!(controller.options_menu.selected_index, 3);
    assert!(controller.options_menu.is_loading_test_selected());
    let cue = controller.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(controller.title_sm.phase, TitlePhase::TransitionToGame);
    assert_eq!(
        controller.title_sm.selected_test_room,
        sh2_ui::menu::TestRoomKind::LoadingScreenTest
    );

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_sh2_61_surface_sfx_test_room_8_material_pads_and_collision() {
    use sh2_app::synthetic_geometry::{
        create_surface_sfx_test_room_cld, create_surface_sfx_test_room_geometry,
        surface_sfx_test_pad_zones,
    };
    use sh2_core::collision::CollisionWorld;
    use sh2_core::SurfaceMaterial;

    let map = create_surface_sfx_test_room_geometry();
    assert_eq!(map.batches.len(), 1);
    assert!(!map.batches[0].positions.is_empty());

    let cld = create_surface_sfx_test_room_cld();
    let world = CollisionWorld::from_cld(&cld);

    let pads = surface_sfx_test_pad_zones();
    assert_eq!(pads.len(), 8);

    // Verify that standing on the center of each of the 8 pads resolves its exact `SurfaceMaterial` (0..=7)
    for (idx, pad) in pads.iter().enumerate() {
        let cx = (pad.min_x + pad.max_x) * 0.5;
        let cz = (pad.min_z + pad.max_z) * 0.5;
        let sample = world
            .sample_floor_surface(cx, cz, 0.0, 250.0, 350.0)
            .expect("Pad must have walkable floor triangle");
        assert_eq!(sample.raw_material, pad.material_id);
        assert_eq!(sample.surface, SurfaceMaterial::from_index(idx as u8));
        assert!((sample.height_y - 0.0).abs() < 1e-3);
    }
}

#[test]
fn test_sh2_15_inventory_and_puzzle_cutaway_debug_rooms_and_rasterizer() {
    use sh2_core::{CutawayOverlayEngine, EventFlags, PuzzleState};
    use sh2_ui::menu::{TestRoomKind, TitlePhase};
    use sh2_ui::TextRasterizer;

    let test_dir = std::env::temp_dir().join("sh2_15_debug_rooms_test");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let config_path = test_dir.join("test_config.json");

    // 1. Verify launching InventoryAndItemsRoom & PuzzleAndCutawayRoom via AppMenuController
    let mut ctrl = AppMenuController::new(&config_path);
    ctrl.title_sm.phase = TitlePhase::TestRoomsSelect;
    ctrl.title_sm.test_room_index = TestRoomKind::ALL
        .iter()
        .position(|&r| r == TestRoomKind::InventoryAndItemsRoom)
        .unwrap();
    let cue = ctrl.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(
        ctrl.title_sm.selected_test_room,
        TestRoomKind::InventoryAndItemsRoom
    );

    ctrl.title_sm.phase = TitlePhase::TestRoomsSelect;
    ctrl.title_sm.test_room_index = TestRoomKind::ALL
        .iter()
        .position(|&r| r == TestRoomKind::PuzzleAndCutawayRoom)
        .unwrap();
    let cue = ctrl.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(
        ctrl.title_sm.selected_test_room,
        TestRoomKind::PuzzleAndCutawayRoom
    );

    // 2. Verify TextRasterizer 2D cutaway blit & OverlaySpriteQuad triangle rasterization
    let mut rast = TextRasterizer::new(640, 480);
    let dummy_rgba: Vec<u8> = [200u8, 180, 160, 255].repeat(64 * 64);
    rast.blit_rgba_scaled(20, 20, 128, 128, &dummy_rgba, 64, 64, 1.0);
    let center_ofs = (80 * 640 + 80) * 4;
    assert_eq!(rast.pixels[center_ofs], 200);
    assert_eq!(rast.pixels[center_ofs + 1], 180);
    assert_eq!(rast.pixels[center_ofs + 2], 160);
    assert_eq!(rast.pixels[center_ofs + 3], 255);

    let flags = EventFlags::new();
    let puzzle = PuzzleState::new();
    let mut overlay = CutawayOverlayEngine::new();
    overlay.set_picture(Some("out_p_incar"), &flags);
    let sprites = overlay.evaluate_frame(&flags, &puzzle, 0, 0).to_vec();
    assert_eq!(sprites.len(), 1);
    let tris = sprites[0].to_viewport_triangles(0.0, 0.0, 512.0, 512.0);
    let map_rgba = vec![255u8; 32 * 32 * 4];
    rast.blit_overlay_triangles(&tris, &map_rgba, 32, 32, 1.0);
    // Sprite covers x: -48.375..207.625 -> screen x: 207.625..463.625, y: -31.625..224.375 -> screen y: 224.375..480.375
    let sprite_ofs = (350 * 640 + 335) * 4;
    assert_eq!(rast.pixels[sprite_ofs], 255);
    assert_eq!(rast.pixels[sprite_ofs + 3], 255);

    // 3. Regression test: Entering a debug room and pressing Escape (`return_to_menu`)
    // must clear `is_fading_out` so `fade_alpha` remains 1.0 across subsequent menu ticks.
    ctrl.title_sm
        .show_phase_immediately(TitlePhase::TestRoomsSelect);
    ctrl.title_sm.test_room_index = TestRoomKind::ALL
        .iter()
        .position(|&r| r == TestRoomKind::PuzzleAndCutawayRoom)
        .unwrap();
    ctrl.handle_nav(MenuNavAction::Confirm);
    while !ctrl.in_game_active {
        ctrl.tick(0.016);
    }
    assert!(ctrl.in_game_active);
    assert!(!ctrl.title_sm.is_fading_out);
    assert!(!ctrl.title_sm.is_fading_in);

    // Simulate pressing Escape inside the debug room
    ctrl.return_to_menu(TitlePhase::TestRoomsSelect);
    assert!(!ctrl.in_game_active);
    assert_eq!(ctrl.title_sm.phase, TitlePhase::TestRoomsSelect);
    assert!((ctrl.title_sm.fade_alpha - 1.0).abs() < 1e-5);
    assert!(!ctrl.title_sm.is_fading_out);
    assert!(!ctrl.title_sm.is_fading_in);

    // Tick 120 frames (2 seconds) on TestRoomsSelect and ensure fade_alpha never decays to 0.0
    for _ in 0..120 {
        ctrl.tick(0.016);
    }
    assert_eq!(ctrl.title_sm.phase, TitlePhase::TestRoomsSelect);
    assert!((ctrl.title_sm.fade_alpha - 1.0).abs() < 1e-5);

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_sh2_65_cli_flags_and_story_stages_and_endings_room_menu_selection() {
    use sh2_app::CliArgs;
    use sh2_ui::menu::{TestRoomKind, TitlePhase};

    for flag in [
        "--story",
        "--story-demo",
        "--stages",
        "--endings",
        "--sh2-65",
        "--room=story",
        "--room=endings",
        "--room=stages",
        "--test-room=sh2-65",
    ] {
        let parsed = CliArgs::parse_from_slice(&["sh2-app", flag]);
        assert_eq!(
            parsed.initial_test_room,
            Some(TestRoomKind::StoryStagesAndEndingsRoom),
            "flag {flag} failed"
        );
    }

    for alias in ["story", "story-demo", "stages", "endings", "sh2-65"] {
        let parsed = CliArgs::parse_from_slice(&["sh2-app", "--room", alias]);
        assert_eq!(
            parsed.initial_test_room,
            Some(TestRoomKind::StoryStagesAndEndingsRoom),
            "--room {alias} failed"
        );
    }

    let test_dir = std::env::temp_dir().join("sh2_65_menu_story_room_test");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    let config_path = test_dir.join("test_config.json");

    let mut ctrl = AppMenuController::new(&config_path);
    ctrl.title_sm.phase = TitlePhase::TestRoomsSelect;
    ctrl.title_sm.test_room_index = TestRoomKind::ALL
        .iter()
        .position(|&r| r == TestRoomKind::StoryStagesAndEndingsRoom)
        .unwrap();
    assert_eq!(
        TestRoomKind::StoryStagesAndEndingsRoom.label(),
        "Story Stages, Riddles & Endings (SH2-65)"
    );
    let cue = ctrl.handle_nav(MenuNavAction::Confirm);
    assert_eq!(cue, MenuSoundCue::GameStartTransition);
    assert_eq!(
        ctrl.title_sm.selected_test_room,
        TestRoomKind::StoryStagesAndEndingsRoom
    );

    // Verify NEW GAME (`MainMenuOption::NewGame` -> `RiddleDifficultySelect` -> `Confirm`)
    // automatically sets `selected_test_room` to `TestRoomKind::RestroomStart` (`er01.map`, Stage 2 `stg_toilet`)
    let mut ng_ctrl = AppMenuController::new(&config_path);
    ng_ctrl
        .title_sm
        .set_save_availability(sh2_ui::menu::SaveAvailability::None);
    ng_ctrl
        .title_sm
        .show_phase_immediately(TitlePhase::TitleLogo);
    ng_ctrl.handle_nav(MenuNavAction::Confirm); // TitleLogo -> MainMenu
    ng_ctrl.handle_nav(MenuNavAction::Confirm); // NewGame -> ScenarioSelect
    ng_ctrl.handle_nav(MenuNavAction::Confirm); // ScenarioSelect -> ActionDifficultySelect
    ng_ctrl.handle_nav(MenuNavAction::Confirm); // ActionDifficultySelect -> RiddleDifficultySelect
    let ng_cue = ng_ctrl.handle_nav(MenuNavAction::Confirm); // RiddleDifficultySelect -> TransitionToGame
    assert_eq!(ng_cue, MenuSoundCue::GameStartTransition);
    assert_eq!(ng_ctrl.title_sm.phase, TitlePhase::TransitionToGame);
    assert_eq!(
        ng_ctrl.title_sm.selected_test_room,
        TestRoomKind::RestroomStart
    );

    let _ = fs::remove_dir_all(&test_dir);
}
