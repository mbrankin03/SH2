//! Interactive Main Menu overlay and options presenter.
//! Integrates `TitleStateMachine` and `OptionsMenu` into the interactive render loop.

use sh2_core::config::AppConfig;
use sh2_ui::menu::{MainMenuOption, MenuNavAction, MenuSoundCue, TitlePhase, TitleStateMachine};
use sh2_ui::options::{OptionsMenu, OptionsSoundCue};
use sh2_ui::pause::{PauseAction, PauseMenu, PauseSubPhase};
use std::path::{Path, PathBuf};

/// App-level menu manager coordinating title sequence, scenario selection, pause menu, and settings persistence.
#[derive(Debug)]
pub struct AppMenuController {
    pub title_sm: TitleStateMachine,
    pub options_menu: OptionsMenu,
    pub pause_menu: PauseMenu,
    pub config_path: PathBuf,
    /// Has the user completed title sequence and transitioned to in-game world?
    pub in_game_active: bool,
    /// Flag indicating that configuration was committed and needs to be applied by the engine loop.
    pub config_applied_pending: Option<AppConfig>,
}

impl AppMenuController {
    /// Initialize with a configuration file path (e.g. `config.json`).
    pub fn new<P: AsRef<Path>>(config_path: P) -> Self {
        let path = config_path.as_ref().to_path_buf();
        let loaded_config = AppConfig::load_from_file(&path);
        let options_menu = OptionsMenu::new(loaded_config);
        let title_sm = TitleStateMachine::new();
        let pause_menu = PauseMenu::new();

        Self {
            title_sm,
            options_menu,
            pause_menu,
            config_path: path,
            in_game_active: false,
            config_applied_pending: None,
        }
    }

    /// Process per-frame animations and fade transitions.
    pub fn tick(&mut self, dt: f32) {
        if !self.in_game_active {
            self.title_sm.tick(dt);
            if self.title_sm.phase == TitlePhase::TransitionToGame
                && (self.title_sm.fade_alpha <= 0.05 || !self.title_sm.is_fading_out)
            {
                self.in_game_active = true;
                tracing::info!(
                    "Title sequence complete. Launching scenario: {:?}, Action: {:?}, Riddle: {:?}",
                    self.title_sm.selected_scenario,
                    self.title_sm.action_difficulty,
                    self.title_sm.riddle_difficulty
                );
            }
        }
    }

    /// Handle unified navigation input across Title and Options screens.
    pub fn handle_nav(&mut self, action: MenuNavAction) -> MenuSoundCue {
        if self.title_sm.phase == TitlePhase::OptionsMenu {
            match action {
                MenuNavAction::Up => {
                    self.options_menu.navigate_vertical(-1);
                    MenuSoundCue::CursorMove
                }
                MenuNavAction::Down => {
                    self.options_menu.navigate_vertical(1);
                    MenuSoundCue::CursorMove
                }
                MenuNavAction::Left => {
                    let cue = self.options_menu.adjust_value(-1);
                    if cue == OptionsSoundCue::LaunchFmvViewer {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer;
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        MenuSoundCue::GameStartTransition
                    } else if cue == OptionsSoundCue::LaunchTestRooms {
                        self.title_sm.phase = TitlePhase::TestRoomsSelect;
                        self.title_sm.test_room_index = 0;
                        MenuSoundCue::Confirm
                    } else {
                        MenuSoundCue::CursorMove
                    }
                }
                MenuNavAction::Right => {
                    let cue = self.options_menu.adjust_value(1);
                    if cue == OptionsSoundCue::LaunchFmvViewer {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer;
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        MenuSoundCue::GameStartTransition
                    } else if cue == OptionsSoundCue::LaunchTestRooms {
                        self.title_sm.phase = TitlePhase::TestRoomsSelect;
                        self.title_sm.test_room_index = 0;
                        MenuSoundCue::Confirm
                    } else {
                        MenuSoundCue::CursorMove
                    }
                }
                MenuNavAction::Confirm => {
                    if self.options_menu.is_fmv_viewer_selected() {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer;
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        return MenuSoundCue::GameStartTransition;
                    }
                    if self.options_menu.is_test_rooms_selected() {
                        self.title_sm.phase = TitlePhase::TestRoomsSelect;
                        self.title_sm.test_room_index = 0;
                        return MenuSoundCue::Confirm;
                    }
                    let (saved_cfg, cue) = self.options_menu.commit_changes();
                    self.config_applied_pending = Some(saved_cfg.clone());
                    if let Err(e) = saved_cfg.save_to_file(&self.config_path) {
                        tracing::warn!("Failed to persist config to {:?}: {}", self.config_path, e);
                    } else {
                        tracing::info!("Saved configuration to {:?}", self.config_path);
                    }
                    if cue == OptionsSoundCue::ConfirmSave {
                        MenuSoundCue::Confirm
                    } else {
                        MenuSoundCue::None
                    }
                }
                MenuNavAction::Cancel => {
                    // Discard and return to Main Menu
                    self.options_menu.discard_changes();
                    self.title_sm.phase = TitlePhase::MainMenu;
                    MenuSoundCue::Cancel
                }
            }
        } else {
            let cue = self.title_sm.handle_input(action);
            if self.title_sm.phase == TitlePhase::OptionsMenu {
                // Refresh live audio output devices upon opening options menu so newly plugged devices appear immediately
                let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
                self.options_menu = OptionsMenu::new(self.options_menu.original_config.clone());
                self.options_menu.set_audio_devices(fresh_devices);
            }
            cue
        }
    }

    /// Cycle tabs in Options Menu using shoulder triggers / Q & E keys.
    pub fn switch_options_tab(&mut self, forward: bool) -> MenuSoundCue {
        if self.title_sm.phase == TitlePhase::OptionsMenu
            || (self.pause_menu.is_open
                && self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu)
        {
            // If switching into Audio tab, merge live discovered devices into options menu
            if (!forward && self.options_menu.active_tab == sh2_ui::options::OptionsTab::Controls)
                || (forward && self.options_menu.active_tab == sh2_ui::options::OptionsTab::Video)
            {
                let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
                for dev in fresh_devices {
                    if !self.options_menu.audio_devices.contains(&dev) {
                        self.options_menu.audio_devices.push(dev);
                    }
                }
            }
            self.options_menu.switch_tab(forward);
            MenuSoundCue::CursorMove
        } else {
            MenuSoundCue::None
        }
    }

    /// Handle pause menu navigation inputs.
    pub fn handle_pause_nav(&mut self, action: MenuNavAction) -> (MenuSoundCue, PauseAction) {
        if !self.pause_menu.is_open {
            return (MenuSoundCue::None, PauseAction::None);
        }

        if self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu {
            match action {
                MenuNavAction::Up => {
                    self.options_menu.navigate_vertical(-1);
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Down => {
                    self.options_menu.navigate_vertical(1);
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Left => {
                    self.options_menu.adjust_value(-1);
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Right => {
                    self.options_menu.adjust_value(1);
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Confirm => {
                    let (saved_cfg, cue) = self.options_menu.commit_changes();
                    self.config_applied_pending = Some(saved_cfg.clone());
                    if let Err(e) = saved_cfg.save_to_file(&self.config_path) {
                        tracing::warn!("Failed to persist config to {:?}: {}", self.config_path, e);
                    } else {
                        tracing::info!("Saved in-game configuration to {:?}", self.config_path);
                    }
                    let sound = if cue == OptionsSoundCue::ConfirmSave {
                        MenuSoundCue::Confirm
                    } else {
                        MenuSoundCue::None
                    };
                    (sound, PauseAction::None)
                }
                MenuNavAction::Cancel => {
                    self.options_menu.discard_changes();
                    self.pause_menu.sub_phase = PauseSubPhase::Main;
                    (MenuSoundCue::Cancel, PauseAction::None)
                }
            }
        } else {
            let (cue, action) = self.pause_menu.handle_nav(action);
            if action == PauseAction::OpenOptions {
                // Refresh live audio output devices upon opening options menu so newly plugged devices appear immediately
                let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
                self.options_menu = OptionsMenu::new(self.options_menu.original_config.clone());
                self.options_menu.set_audio_devices(fresh_devices);
            } else if action == PauseAction::QuitToTitle {
                // Reset in_game_active flag and return to Main Menu
                self.in_game_active = false;
                self.title_sm.phase = TitlePhase::MainMenu;
                self.title_sm.fade_alpha = 1.0;
            }
            (cue, action)
        }
    }

    /// Formatted status summary for display / debug overlays.
    pub fn overlay_text(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.in_game_active {
            return lines;
        }

        lines.push(format!(
            "--- SILENT HILL 2 --- (Phase: {:?})",
            self.title_sm.phase
        ));
        lines.push(format!("Fade Alpha: {:.2}", self.title_sm.fade_alpha));

        match self.title_sm.phase {
            TitlePhase::SplashIntro => {
                lines.push("KONAMI / TEAM SILENT".into());
                lines.push("[Press Enter / Cross / A to Skip]".into());
            }
            TitlePhase::TitleLogo => {
                lines.push("SILENT HILL 2".into());
                lines.push("[Press Enter / Cross / A to Start]".into());
            }
            TitlePhase::MainMenu => {
                lines.push("=== MAIN MENU ===".into());
                for (idx, opt) in MainMenuOption::ALL.iter().enumerate() {
                    let prefix = if idx == self.title_sm.main_menu_index {
                        "> "
                    } else {
                        "  "
                    };
                    lines.push(format!("{}{}", prefix, opt.label()));
                }
            }
            TitlePhase::ScenarioSelect => {
                lines.push("=== SELECT SCENARIO ===".into());
                let s0 = if self.title_sm.scenario_index == 0 {
                    "> "
                } else {
                    "  "
                };
                let s1 = if self.title_sm.scenario_index == 1 {
                    "> "
                } else {
                    "  "
                };
                lines.push(format!("{}Letter from Silent Heaven (James)", s0));
                lines.push(format!("{}Born from a Wish (Maria)", s1));
            }
            TitlePhase::ActionDifficultySelect => {
                lines.push("=== ACTION DIFFICULTY ===".into());
                lines.push(format!("Selected: {:?}", self.title_sm.action_difficulty));
                lines.push("[Up/Down: Change, Confirm: Next, Cancel: Back]".into());
            }
            TitlePhase::RiddleDifficultySelect => {
                lines.push("=== RIDDLE DIFFICULTY ===".into());
                lines.push(format!("Selected: {:?}", self.title_sm.riddle_difficulty));
                lines.push("[Up/Down: Change, Confirm: Start Game, Cancel: Back]".into());
            }
            TitlePhase::OptionsMenu => {
                lines.push(format!(
                    "=== OPTIONS: {} ===",
                    self.options_menu.active_tab.label()
                ));
                lines.push("[Q / E or L1 / R1: Switch Category]".into());
                for i in 0..self.options_menu.item_count() {
                    if let Some((label, val)) = self.options_menu.item_display(i) {
                        let prefix = if i == self.options_menu.selected_index {
                            "> "
                        } else {
                            "  "
                        };
                        lines.push(format!("{}{:<22} : {}", prefix, label, val));
                    }
                }
                if self.options_menu.is_dirty {
                    lines.push("* Unsaved changes! [Confirm to Save, Cancel to Discard]".into());
                } else {
                    lines.push("[Cancel to Return to Main Menu]".into());
                }
            }
            TitlePhase::TestRoomsSelect => {
                lines.push("=== TEST ROOMS / DEBUG STAGES ===".into());
                for (idx, room) in sh2_ui::menu::TestRoomKind::ALL.iter().enumerate() {
                    let prefix = if idx == self.title_sm.test_room_index {
                        "> "
                    } else {
                        "  "
                    };
                    lines.push(format!("{}{}", prefix, room.label()));
                }
                lines.push("[Up/Down: Select Room, Confirm: Boot Stage, Cancel: Back]".into());
            }
            TitlePhase::Credits => {
                lines.push("=== CREDITS ===".into());
                lines.push("Director: Keiichiro Toyama".into());
                lines.push("CGI & Character Design: Takayoshi Sato".into());
                lines.push("Music & Sound Design: Akira Yamaoka".into());
                lines.push("[Press Cancel / Esc to Return]".into());
            }
            TitlePhase::TransitionToGame => {
                lines.push(format!(
                    "Loading Stage: {}...",
                    self.title_sm.selected_test_room.label()
                ));
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Navigate to Options
        controller.handle_nav(MenuNavAction::Down); // Continue
        controller.handle_nav(MenuNavAction::Down); // Options
        controller.handle_nav(MenuNavAction::Confirm);
        assert_eq!(controller.title_sm.phase, TitlePhase::OptionsMenu);

        // Change resolution and commit
        controller.handle_nav(MenuNavAction::Right);
        assert!(controller.options_menu.is_dirty);
        controller.handle_nav(MenuNavAction::Confirm);
        assert!(!controller.options_menu.is_dirty);
        assert!(cfg_path.exists());

        // Return to main menu
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
}
