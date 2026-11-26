//! Debug HUD text formatting for [`AppMenuController`].

use super::AppMenuController;
use sh2_ui::menu::{MainMenuOption, TitlePhase};

impl AppMenuController {
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
                    if let Some((label, val)) = self.options_menu.item_display_cow(i) {
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
                lines.push("Director: Masashi Tsuboyama".into());
                lines.push("CGI & Character Design: Takayoshi Sato".into());
                lines.push("Music & Sound Design: Akira Yamaoka".into());
                lines.push("[Press Cancel / Esc to Return]".into());
            }
            TitlePhase::ExitConfirm => {
                lines.push("Exit the game?".into());
                let choice = if self.title_sm.exit_confirm_index == 0 {
                    "> Yes <   No"
                } else {
                    "  Yes   > No <"
                };
                lines.push(choice.into());
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
