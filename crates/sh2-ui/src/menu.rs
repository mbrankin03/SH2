//! Title screen and main menu state machine.
//! Ground truth references:
//! - PS2 July 13, 2001 Prototype: `title.c` (`titleInit`, `titleFadeIn`, `titleMainSelect`,
//!   `titleBattleSelect`, `titleRiddleSelect`, `titleFadeOutNewGame`, `titleFadeOut`, `titleExit`).
//! - Sound triggers: `0x2710` (cursor move), `0x2712` (confirm), `0x2713` (cancel), `15002` (game start transition).

use serde::{Deserialize, Serialize};
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::scenario::ScenarioKind;

/// Authentic title sequence phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TitlePhase {
    /// Initial Konami / Team Silent splash intro (`data/pic/etc/start00.tex`).
    #[default]
    SplashIntro,
    /// Silent Hill 2 main title logo screen (`start01.tex`).
    TitleLogo,
    /// Main interactive selection menu.
    MainMenu,
    /// Scenario selection (Main Scenario: James vs Sub Scenario: Maria).
    ScenarioSelect,
    /// Action combat difficulty selection (Easy, Normal, Hard).
    ActionDifficultySelect,
    /// Riddle puzzle difficulty selection (Easy, Normal, Hard, Extra).
    RiddleDifficultySelect,
    /// In-game options configuration menu (Video, Audio, Controls, Game).
    OptionsMenu,
    /// Credits display screen.
    Credits,
    /// Fade-out transition into the game world / opening FMV (`rr21.map`).
    TransitionToGame,
}

/// Primary options on the main title menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainMenuOption {
    #[default]
    NewGame = 0,
    Continue = 1,
    Options = 2,
    Credits = 3,
    Exit = 4,
}

impl MainMenuOption {
    pub const ALL: [MainMenuOption; 5] = [
        MainMenuOption::NewGame,
        MainMenuOption::Continue,
        MainMenuOption::Options,
        MainMenuOption::Credits,
        MainMenuOption::Exit,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::NewGame => "NEW GAME",
            Self::Continue => "CONTINUE",
            Self::Options => "OPTIONS",
            Self::Credits => "CREDITS",
            Self::Exit => "EXIT",
        }
    }
}

/// Audio cues emitted by the title menu state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSoundCue {
    None,
    /// Cursor move sound (authentic ID `0x2710` / 10000).
    CursorMove,
    /// Confirm selection sound (authentic ID `0x2712` / 10002).
    Confirm,
    /// Cancel / back sound (authentic ID `0x2713` / 10003).
    Cancel,
    /// Game start audio fadeout cue (authentic ID `15002` / `0x3a9a`).
    GameStartTransition,
}

/// Navigation directional action from gamepad or keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuNavAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
}

/// Title state machine coordinator.
#[derive(Debug, Clone)]
pub struct TitleStateMachine {
    pub phase: TitlePhase,
    pub main_menu_index: usize,
    pub scenario_index: usize,
    pub action_difficulty: ActionDifficulty,
    pub riddle_difficulty: RiddleDifficulty,
    pub selected_scenario: ScenarioKind,
    /// Visual fade opacity: 0.0 (fully transparent / black) to 1.0 (fully visible).
    pub fade_alpha: f32,
    pub fade_speed: f32,
    pub is_fading_in: bool,
    pub is_fading_out: bool,
    /// Timer for splash screen duration in seconds.
    pub splash_timer: f32,
    /// Last audio cue emitted by state change.
    pub pending_audio_cue: MenuSoundCue,
}

impl Default for TitleStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl TitleStateMachine {
    /// Initialize title screen matching PS2 `titleInit`.
    pub fn new() -> Self {
        Self {
            phase: TitlePhase::SplashIntro,
            main_menu_index: 0,
            scenario_index: 0,
            action_difficulty: ActionDifficulty::Normal,
            riddle_difficulty: RiddleDifficulty::Normal,
            selected_scenario: ScenarioKind::LetterFromSilentHeaven,
            fade_alpha: 0.0,
            fade_speed: 1.5,
            is_fading_in: true,
            is_fading_out: false,
            splash_timer: 0.0,
            pending_audio_cue: MenuSoundCue::None,
        }
    }

    /// Advance menu timers and smooth alpha fades per frame tick (delta time `dt`).
    pub fn tick(&mut self, dt: f32) {
        self.pending_audio_cue = MenuSoundCue::None;

        if self.is_fading_in {
            self.fade_alpha = (self.fade_alpha + self.fade_speed * dt).min(1.0);
            if self.fade_alpha >= 1.0 {
                self.is_fading_in = false;
            }
        } else if self.is_fading_out {
            self.fade_alpha = (self.fade_alpha - self.fade_speed * dt).max(0.0);
            if self.fade_alpha <= 0.0 {
                self.is_fading_out = false;
                self.on_fade_out_complete();
            }
        }

        // Auto progression from splash screen
        if self.phase == TitlePhase::SplashIntro {
            self.splash_timer += dt;
            if self.splash_timer >= 2.5 && !self.is_fading_out {
                self.start_fade_out();
            }
        }
    }

    /// Handle player navigation input (D-pad, sticks, Enter, Esc).
    pub fn handle_input(&mut self, action: MenuNavAction) -> MenuSoundCue {
        let mut sound = MenuSoundCue::None;

        match self.phase {
            TitlePhase::SplashIntro => {
                // Any button skips splash intro immediately
                if matches!(action, MenuNavAction::Confirm | MenuNavAction::Cancel) {
                    self.phase = TitlePhase::TitleLogo;
                    self.fade_alpha = 1.0;
                    self.is_fading_out = false;
                    self.is_fading_in = false;
                    sound = MenuSoundCue::Confirm;
                }
            }
            TitlePhase::TitleLogo => {
                if matches!(action, MenuNavAction::Confirm) {
                    self.phase = TitlePhase::MainMenu;
                    sound = MenuSoundCue::Confirm;
                }
            }
            TitlePhase::MainMenu => match action {
                MenuNavAction::Up => {
                    if self.main_menu_index == 0 {
                        self.main_menu_index = MainMenuOption::ALL.len() - 1;
                    } else {
                        self.main_menu_index -= 1;
                    }
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Down => {
                    self.main_menu_index = (self.main_menu_index + 1) % MainMenuOption::ALL.len();
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => match MainMenuOption::ALL[self.main_menu_index] {
                    MainMenuOption::NewGame => {
                        self.phase = TitlePhase::ScenarioSelect;
                        self.scenario_index = 0;
                        sound = MenuSoundCue::Confirm;
                    }
                    MainMenuOption::Continue => {
                        // In prototype / vanilla SH2, launches into last saved state
                        self.phase = TitlePhase::TransitionToGame;
                        sound = MenuSoundCue::GameStartTransition;
                    }
                    MainMenuOption::Options => {
                        self.phase = TitlePhase::OptionsMenu;
                        sound = MenuSoundCue::Confirm;
                    }
                    MainMenuOption::Credits => {
                        self.phase = TitlePhase::Credits;
                        sound = MenuSoundCue::Confirm;
                    }
                    MainMenuOption::Exit => {
                        sound = MenuSoundCue::Cancel;
                    }
                },
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::TitleLogo;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::ScenarioSelect => match action {
                MenuNavAction::Up | MenuNavAction::Down => {
                    self.scenario_index = if self.scenario_index == 0 { 1 } else { 0 };
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    self.selected_scenario = if self.scenario_index == 0 {
                        ScenarioKind::LetterFromSilentHeaven
                    } else {
                        ScenarioKind::BornFromAWish
                    };
                    self.phase = TitlePhase::ActionDifficultySelect;
                    sound = MenuSoundCue::Confirm;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::MainMenu;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::ActionDifficultySelect => match action {
                MenuNavAction::Up => {
                    self.action_difficulty = self.action_difficulty.prev();
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Down => {
                    self.action_difficulty = self.action_difficulty.next();
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    self.phase = TitlePhase::RiddleDifficultySelect;
                    sound = MenuSoundCue::Confirm;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::ScenarioSelect;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::RiddleDifficultySelect => match action {
                MenuNavAction::Up => {
                    self.riddle_difficulty = self.riddle_difficulty.prev();
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Down => {
                    self.riddle_difficulty = self.riddle_difficulty.next();
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    // Match PS2 `titleFadeOutNewGame`
                    self.phase = TitlePhase::TransitionToGame;
                    self.start_fade_out();
                    sound = MenuSoundCue::GameStartTransition;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::ActionDifficultySelect;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::Credits => {
                if matches!(action, MenuNavAction::Confirm | MenuNavAction::Cancel) {
                    self.phase = TitlePhase::MainMenu;
                    sound = MenuSoundCue::Cancel;
                }
            }
            TitlePhase::OptionsMenu | TitlePhase::TransitionToGame => {}
        }

        self.pending_audio_cue = sound;
        sound
    }

    /// Trigger smooth fade out.
    pub fn start_fade_out(&mut self) {
        self.is_fading_out = true;
        self.is_fading_in = false;
    }

    /// Trigger smooth fade in.
    pub fn start_fade_in(&mut self) {
        self.is_fading_in = true;
        self.is_fading_out = false;
        self.fade_alpha = 0.0;
    }

    fn on_fade_out_complete(&mut self) {
        match self.phase {
            TitlePhase::SplashIntro => {
                self.phase = TitlePhase::TitleLogo;
                self.start_fade_in();
            }
            TitlePhase::TransitionToGame => {
                // Keep faded out at 0.0 for seamless game world start
                self.fade_alpha = 0.0;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_state_machine_flow() {
        let mut sm = TitleStateMachine::new();
        assert_eq!(sm.phase, TitlePhase::SplashIntro);

        // Skip splash with confirm
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(sm.phase, TitlePhase::TitleLogo);

        // Advance to Main Menu
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(sm.phase, TitlePhase::MainMenu);

        // Navigate down to OPTIONS
        let cue = sm.handle_input(MenuNavAction::Down);
        assert_eq!(cue, MenuSoundCue::CursorMove);
        let cue = sm.handle_input(MenuNavAction::Down);
        assert_eq!(cue, MenuSoundCue::CursorMove);
        assert_eq!(
            MainMenuOption::ALL[sm.main_menu_index],
            MainMenuOption::Options
        );

        // Open Options Menu
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(sm.phase, TitlePhase::OptionsMenu);
    }

    #[test]
    fn test_scenario_and_difficulty_selection_flow() {
        let mut sm = TitleStateMachine::new();
        sm.phase = TitlePhase::MainMenu;
        sm.main_menu_index = 0; // New Game

        // Select New Game -> Scenario Select
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(sm.phase, TitlePhase::ScenarioSelect);

        // Choose "Born from a Wish" (SubScenario)
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.scenario_index, 1);
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(sm.selected_scenario, ScenarioKind::BornFromAWish);
        assert_eq!(sm.phase, TitlePhase::ActionDifficultySelect);

        // Set Hard Action Difficulty
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.action_difficulty, ActionDifficulty::Hard);
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(sm.phase, TitlePhase::RiddleDifficultySelect);

        // Set Extra Riddle Difficulty
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.riddle_difficulty, RiddleDifficulty::Hard);
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.riddle_difficulty, RiddleDifficulty::Extra);

        // Confirm New Game start -> TransitionToGame + audio cue 15002
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::GameStartTransition);
        assert_eq!(sm.phase, TitlePhase::TransitionToGame);
    }
}
