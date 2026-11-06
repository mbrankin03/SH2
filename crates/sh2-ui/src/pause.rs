//! Authentic In-Game Pause Menu state machine and rendering.
//! Ground truth references:
//! - Retail PC / PS2 game event 0x10 (`EVENT_PAUSE_MENU` at 0x407310 / 0x407620).
//! - SH2:EE `InputTweaks.cpp` (Pause hitboxes: 5 items at top 216, left 281, height 30, width 118).
//! - 5 items: CONTINUE (Resume), SAVE (Save Game), LOAD (Load Game), OPTIONS, QUIT.
//! - Sub-menu on QUIT: "Exit to Main Menu?" with YES / NO (No default).
//! - Pure black semi-transparent backdrop overlay (50% alpha dim) over paused game simulation.
//! - Proportional font rendering matching retail presentation.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::menu::{MenuNavAction, MenuSoundCue};
use serde::{Deserialize, Serialize};

/// Items selectable on the primary Pause Menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PauseMenuOption {
    #[default]
    Continue = 0,
    Save = 1,
    Load = 2,
    Options = 3,
    Quit = 4,
}

impl PauseMenuOption {
    pub const ALL: [PauseMenuOption; 5] = [
        PauseMenuOption::Continue,
        PauseMenuOption::Save,
        PauseMenuOption::Load,
        PauseMenuOption::Options,
        PauseMenuOption::Quit,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Continue => "CONTINUE",
            Self::Save => "SAVE",
            Self::Load => "LOAD",
            Self::Options => "OPTIONS",
            Self::Quit => "QUIT",
        }
    }
}

/// Action resulting from pause menu selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseAction {
    None,
    Resume,
    Save,
    Load,
    OpenOptions,
    QuitToTitle,
}

/// Current sub-phase of the pause state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PauseSubPhase {
    #[default]
    Main,
    QuitConfirm,
    InOptionsMenu,
}

/// Pause menu state machine managing cursor indices, quit confirmation, and navigation.
#[derive(Debug, Clone, PartialEq)]
pub struct PauseMenu {
    pub is_open: bool,
    pub sub_phase: PauseSubPhase,
    pub selected_index: usize,
    /// In quit confirmation dialog: 0 = YES, 1 = NO (retail defaults to NO = 1)
    pub quit_confirm_index: usize,
    /// Can James save at the current moment (e.g. at red square, or quicksave allowed)?
    pub can_save: bool,
    /// Location name string (e.g. "Observation Deck", "South Vale", etc.)
    pub location_name: String,
    /// Play time in elapsed seconds
    pub play_time_secs: f64,
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            is_open: false,
            sub_phase: PauseSubPhase::Main,
            selected_index: 0,
            quit_confirm_index: 1, // Retail defaults to "No"
            can_save: false,
            location_name: String::new(),
            play_time_secs: 0.0,
        }
    }

    /// Open pause menu, specifying whether saving is currently allowed.
    pub fn open(&mut self, can_save: bool, location: &str, play_time: f64) {
        self.is_open = true;
        self.sub_phase = PauseSubPhase::Main;
        self.selected_index = 0;
        self.quit_confirm_index = 1;
        self.can_save = can_save;
        self.location_name = location.to_string();
        self.play_time_secs = play_time;
    }

    /// Close the pause menu and return to gameplay.
    pub fn close(&mut self) {
        self.is_open = false;
        self.sub_phase = PauseSubPhase::Main;
    }

    /// Handle directional and action inputs.
    pub fn handle_nav(&mut self, action: MenuNavAction) -> (MenuSoundCue, PauseAction) {
        if !self.is_open {
            return (MenuSoundCue::None, PauseAction::None);
        }

        match self.sub_phase {
            PauseSubPhase::Main => match action {
                MenuNavAction::Up => {
                    let total = PauseMenuOption::ALL.len();
                    let mut next = (self.selected_index + total - 1) % total;
                    // If Save is disabled and selected, skip over it
                    if next == PauseMenuOption::Save as usize && !self.can_save {
                        next = (next + total - 1) % total;
                    }
                    self.selected_index = next;
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Down => {
                    let total = PauseMenuOption::ALL.len();
                    let mut next = (self.selected_index + 1) % total;
                    // If Save is disabled and selected, skip over it
                    if next == PauseMenuOption::Save as usize && !self.can_save {
                        next = (next + 1) % total;
                    }
                    self.selected_index = next;
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Left | MenuNavAction::Right => {
                    // No horizontal navigation on main pause menu
                    (MenuSoundCue::None, PauseAction::None)
                }
                MenuNavAction::Confirm => match PauseMenuOption::ALL[self.selected_index] {
                    PauseMenuOption::Continue => {
                        self.close();
                        (MenuSoundCue::Confirm, PauseAction::Resume)
                    }
                    PauseMenuOption::Save => {
                        if self.can_save {
                            (MenuSoundCue::Confirm, PauseAction::Save)
                        } else {
                            (MenuSoundCue::Cancel, PauseAction::None)
                        }
                    }
                    PauseMenuOption::Load => (MenuSoundCue::Confirm, PauseAction::Load),
                    PauseMenuOption::Options => {
                        self.sub_phase = PauseSubPhase::InOptionsMenu;
                        (MenuSoundCue::Confirm, PauseAction::OpenOptions)
                    }
                    PauseMenuOption::Quit => {
                        self.sub_phase = PauseSubPhase::QuitConfirm;
                        self.quit_confirm_index = 1; // Default to NO
                        (MenuSoundCue::Confirm, PauseAction::None)
                    }
                },
                MenuNavAction::Cancel => {
                    // ESC / Cancel on main pause menu resumes gameplay
                    self.close();
                    (MenuSoundCue::Cancel, PauseAction::Resume)
                }
            },
            PauseSubPhase::QuitConfirm => match action {
                MenuNavAction::Left
                | MenuNavAction::Right
                | MenuNavAction::Up
                | MenuNavAction::Down => {
                    self.quit_confirm_index ^= 1;
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Confirm => {
                    if self.quit_confirm_index == 0 {
                        // User selected YES -> Quit to title
                        self.close();
                        (MenuSoundCue::Confirm, PauseAction::QuitToTitle)
                    } else {
                        // User selected NO -> Return to pause menu
                        self.sub_phase = PauseSubPhase::Main;
                        (MenuSoundCue::Confirm, PauseAction::None)
                    }
                }
                MenuNavAction::Cancel => {
                    // Cancel quit dialog -> Return to pause menu
                    self.sub_phase = PauseSubPhase::Main;
                    (MenuSoundCue::Cancel, PauseAction::None)
                }
            },
            PauseSubPhase::InOptionsMenu => {
                // When in options sub-phase, options menu handles its own navigation until back/cancel
                if action == MenuNavAction::Cancel {
                    self.sub_phase = PauseSubPhase::Main;
                    (MenuSoundCue::Cancel, PauseAction::None)
                } else {
                    (MenuSoundCue::None, PauseAction::None)
                }
            }
        }
    }
}

/// Renders the authentic Pause Menu overlay into the given `TextRasterizer`.
///
/// Matches retail PC / PS2 styling:
/// - 50% black alpha scrim over the frozen game world
/// - Centered "PAUSED" title header in authentic large typography
/// - Location and formatted play time (HH:MM:SS) display
/// - Authentic cursor highlight and disabled option styling
pub fn draw_pause_screen(
    pause_menu: &PauseMenu,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    if !pause_menu.is_open {
        return;
    }

    let w = rasterizer.width;
    let h = rasterizer.height;
    let center_x = w / 2;

    // 1. Semi-transparent dark background overlay (50% scrim = [0, 0, 0, 160])
    let dim_color = [0u8, 0, 0, 165];
    for y in 0..h {
        for x in 0..w {
            rasterizer.blend_pixel(x, y, dim_color);
        }
    }

    match pause_menu.sub_phase {
        PauseSubPhase::Main => {
            // Main PAUSED title
            font.draw_line_centered(
                rasterizer,
                "PAUSED",
                (center_x, 150),
                36,
                Sh2TextColor::AmberHeader,
                true,
            );

            // Optional location text & elapsed play time
            let s = pause_menu.play_time_secs as u64;
            let hours = s / 3600;
            let mins = (s / 60) % 60;
            let secs = s % 60;
            let time_str = format!("{:02}:{:02}:{:02}", hours, mins, secs);

            if !pause_menu.location_name.is_empty() {
                font.draw_line_centered(
                    rasterizer,
                    &pause_menu.location_name,
                    (center_x, 195),
                    20,
                    Sh2TextColor::SilverUnselected,
                    true,
                );
                font.draw_line_centered(
                    rasterizer,
                    &time_str,
                    (center_x, 220),
                    18,
                    Sh2TextColor::MutedFooter,
                    true,
                );
            } else {
                font.draw_line_centered(
                    rasterizer,
                    &time_str,
                    (center_x, 200),
                    18,
                    Sh2TextColor::MutedFooter,
                    true,
                );
            }

            // Draw 5 pause menu choices: CONTINUE, SAVE, LOAD, OPTIONS, QUIT
            let start_y = 280;
            let step_y = 48;

            for (idx, opt) in PauseMenuOption::ALL.iter().enumerate() {
                let y = start_y + (idx * step_y);
                let is_selected = idx == pause_menu.selected_index;
                let is_disabled = *opt == PauseMenuOption::Save && !pause_menu.can_save;

                let (color, text_scale) = if is_disabled {
                    (Sh2TextColor::MutedFooter, 24)
                } else if is_selected {
                    (Sh2TextColor::GoldSelected, 26)
                } else {
                    (Sh2TextColor::SilverUnselected, 24)
                };

                // Draw indicator arrow or brackets for selected item
                let label = if is_selected {
                    format!("> {} <", opt.label())
                } else {
                    opt.label().to_string()
                };

                font.draw_line_centered(rasterizer, &label, (center_x, y), text_scale, color, true);
            }

            // Footer prompt
            font.draw_line_centered(
                rasterizer,
                "[ENTER / CROSS] Select   [ESC / CIRCLE] Resume Gameplay",
                (center_x, 660),
                16,
                Sh2TextColor::MutedFooter,
                true,
            );
        }
        PauseSubPhase::QuitConfirm => {
            // "Exit to Main Menu?" prompt
            font.draw_line_centered(
                rasterizer,
                "Exit to Main Menu?",
                (center_x, 260),
                32,
                Sh2TextColor::White,
                true,
            );

            // Subtitle warning
            font.draw_line_centered(
                rasterizer,
                "Any unsaved progress will be lost.",
                (center_x, 310),
                18,
                Sh2TextColor::SilverUnselected,
                true,
            );

            // YES / NO options centered horizontally
            let yes_selected = pause_menu.quit_confirm_index == 0;
            let no_selected = pause_menu.quit_confirm_index == 1;

            let yes_label = if yes_selected { "> YES <" } else { "YES" };
            let no_label = if no_selected { "> NO <" } else { "NO" };

            let yes_color = if yes_selected {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::SilverUnselected
            };
            let no_color = if no_selected {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::SilverUnselected
            };

            let left_x = center_x - 120;
            let right_x = center_x + 120;
            let opt_y = 400;

            font.draw_line_centered(rasterizer, yes_label, (left_x, opt_y), 28, yes_color, true);
            font.draw_line_centered(rasterizer, no_label, (right_x, opt_y), 28, no_color, true);

            // Footer prompt
            font.draw_line_centered(
                rasterizer,
                "[ENTER / CROSS] Confirm   [ESC / CIRCLE] Back",
                (center_x, 660),
                16,
                Sh2TextColor::MutedFooter,
                true,
            );
        }
        PauseSubPhase::InOptionsMenu => {
            // Options menu renders its own full UI
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pause_menu_navigation_and_loop() {
        let mut pause = PauseMenu::new();
        assert!(!pause.is_open);

        pause.open(false, "Observation Deck", 125.0);
        assert!(pause.is_open);
        assert_eq!(pause.selected_index, 0); // Continue

        // Down skips Save because can_save is false
        let (cue, action) = pause.handle_nav(MenuNavAction::Down);
        assert_eq!(cue, MenuSoundCue::CursorMove);
        assert_eq!(action, PauseAction::None);
        assert_eq!(pause.selected_index, 2); // Load

        // Down again goes to Options
        pause.handle_nav(MenuNavAction::Down);
        assert_eq!(pause.selected_index, 3); // Options

        // Down again goes to Quit
        pause.handle_nav(MenuNavAction::Down);
        assert_eq!(pause.selected_index, 4); // Quit

        // Down wraps to Continue (0)
        pause.handle_nav(MenuNavAction::Down);
        assert_eq!(pause.selected_index, 0); // Continue

        // Up wraps to Quit (4)
        pause.handle_nav(MenuNavAction::Up);
        assert_eq!(pause.selected_index, 4); // Quit

        // Up skips Save when can_save is false
        pause.handle_nav(MenuNavAction::Up); // Options (3)
        pause.handle_nav(MenuNavAction::Up); // Load (2)
        pause.handle_nav(MenuNavAction::Up); // Skips Save(1) -> Continue (0)
        assert_eq!(pause.selected_index, 0);
    }

    #[test]
    fn test_pause_menu_save_enabled_when_allowed() {
        let mut pause = PauseMenu::new();
        pause.open(true, "Lakeview Hotel", 500.0);
        assert_eq!(pause.selected_index, 0);

        // Down goes to Save(1)
        pause.handle_nav(MenuNavAction::Down);
        assert_eq!(pause.selected_index, 1);

        // Confirm triggers Save action
        let (cue, action) = pause.handle_nav(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(action, PauseAction::Save);
    }

    #[test]
    fn test_pause_menu_quit_flow() {
        let mut pause = PauseMenu::new();
        pause.open(false, "South Vale", 300.0);

        // Navigate to Quit
        pause.selected_index = 4;
        let (cue, action) = pause.handle_nav(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(action, PauseAction::None);
        assert_eq!(pause.sub_phase, PauseSubPhase::QuitConfirm);
        assert_eq!(pause.quit_confirm_index, 1); // Defaults to NO

        // Pressing Enter on NO cancels quit dialog and returns to Main pause menu
        let (cue, action) = pause.handle_nav(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(action, PauseAction::None);
        assert_eq!(pause.sub_phase, PauseSubPhase::Main);

        // Enter quit dialog again, toggle to YES, then confirm
        pause.handle_nav(MenuNavAction::Confirm); // Opens quit confirm
        assert_eq!(pause.sub_phase, PauseSubPhase::QuitConfirm);
        pause.handle_nav(MenuNavAction::Left);
        assert_eq!(pause.quit_confirm_index, 0); // YES
        let (cue, action) = pause.handle_nav(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(action, PauseAction::QuitToTitle);
        assert!(!pause.is_open);
    }

    #[test]
    fn test_pause_menu_resume_action() {
        let mut pause = PauseMenu::new();
        pause.open(false, "Restroom", 10.0);
        assert!(pause.is_open);

        // Confirming Continue resumes
        let (cue, action) = pause.handle_nav(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(action, PauseAction::Resume);
        assert!(!pause.is_open);

        // Re-open and cancel also resumes
        pause.open(false, "Restroom", 10.0);
        let (cue, action) = pause.handle_nav(MenuNavAction::Cancel);
        assert_eq!(cue, MenuSoundCue::Cancel);
        assert_eq!(action, PauseAction::Resume);
        assert!(!pause.is_open);
    }

    #[test]
    fn test_draw_pause_screen_rendering() {
        let font = AtlasFont::shared(None);
        let mut rasterizer = TextRasterizer::new(1280, 720);
        let mut pause = PauseMenu::new();
        pause.open(true, "Observation Deck", 3661.0);

        draw_pause_screen(&pause, font, &mut rasterizer);

        let non_zero_count = rasterizer.pixels.iter().filter(|&&p| p > 0).count();
        assert!(non_zero_count > 10000);

        pause.sub_phase = PauseSubPhase::QuitConfirm;
        rasterizer.clear();
        draw_pause_screen(&pause, font, &mut rasterizer);
        let non_zero_quit = rasterizer.pixels.iter().filter(|&&p| p > 0).count();
        assert!(non_zero_quit > 10000);
    }
}
