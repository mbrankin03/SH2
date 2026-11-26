//! Authentic In-Game Pause Menu state machine and rendering.
//! Ground truth references:
//! - Retail PC / PS2 game event 0x10 (`EVENT_PAUSE_MENU` at 0x407310 / 0x407620).
//! - SH2:EE `InputTweaks.cpp` (Pause hitboxes: 5 items at top 216, left 281, height 30, width 118).
//! - 5 items: CONTINUE (Resume), SAVE (Save Game), LOAD (Load Game), OPTIONS, QUIT.
//! - Sub-menu on QUIT: "Exit to Main Menu?" with YES / NO (No default).
//! - Pure black semi-transparent backdrop overlay (50% alpha dim) over paused game simulation.
//! - Proportional font rendering matching retail presentation.

mod render;
pub use render::*;

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

    /// Retail PC `sh2pc.exe` (`FUN_00407300` `0x00407300`, `0x0062f098..0x0062f0c4`) labels.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Continue => "Resume",
            Self::Save => "Save Game",
            Self::Load => "Load Game",
            Self::Options => "Options",
            Self::Quit => "Quit",
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
    /// Transient feedback banner (e.g. `"GAME SAVED"`, `"NO SAVE DATA FOUND"`).
    pub status_message: Option<String>,
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
            status_message: None,
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
        self.status_message = None;
    }

    /// Sets a transient status message displayed below the pause menu items.
    pub fn set_status_message(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    /// Close the pause menu and return to gameplay.
    pub fn close(&mut self) {
        self.is_open = false;
        self.sub_phase = PauseSubPhase::Main;
        self.status_message = None;
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

    /// Maps window pixel coordinates `(mouse_x, mouse_y)` into the 4:3 pillarboxed/letterboxed
    /// `512 × 480` virtual coordinate space used by [`draw_pause_screen_with_scrim`], matching
    /// `crate::inventory::ps2_ui_canvas_rect` (`state.ps2_ui_viewport()`) 1:1 on all aspect ratios.
    fn window_to_pause_virtual(
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> Option<(f32, f32)> {
        let (ox, oy, cw, ch) = crate::inventory::ps2_ui_canvas_rect(win_w, win_h);
        if cw <= 0.0 || ch <= 0.0 {
            return None;
        }
        let vx = (mouse_x - ox) * PAUSE_VIRTUAL_W / cw;
        let vy = (mouse_y - oy) * PAUSE_VIRTUAL_H / ch;
        Some((vx, vy))
    }

    /// Hit-tests `Main` pause menu rows (`0..5`, skipping disabled `Save`) in `512 × 480` space.
    fn hit_test_main_row(&self, vx: f32, vy: f32) -> Option<usize> {
        if !(148.0..=364.0).contains(&vx) {
            return None;
        }
        for (i, &opt) in PauseMenuOption::ALL.iter().enumerate() {
            let y0 = PAUSE_ITEM_START_Y + (i as f32) * PAUSE_ITEM_PITCH_Y;
            let y1 = y0 + PAUSE_ITEM_PITCH_Y;
            if vy >= y0 && vy < y1 {
                if opt == PauseMenuOption::Save && !self.can_save {
                    return None;
                }
                return Some(i);
            }
        }
        None
    }

    /// Hit-tests `QuitConfirm` `"Yes"` (`0`) and `"No"` (`1`) labels in `512 × 480` space.
    fn hit_test_quit_confirm(vx: f32, vy: f32) -> Option<usize> {
        let y0 = PAUSE_QUIT_YES_XY.1 - 6.0;
        let y1 = PAUSE_QUIT_YES_XY.1 + PAUSE_ITEM_PITCH_Y + 6.0;
        if !(y0..=y1).contains(&vy) {
            return None;
        }
        if (PAUSE_QUIT_YES_XY.0 - 30.0..=PAUSE_QUIT_YES_XY.0 + 30.0).contains(&vx) {
            Some(0)
        } else if (PAUSE_QUIT_NO_XY.0 - 30.0..=PAUSE_QUIT_NO_XY.0 + 30.0).contains(&vx) {
            Some(1)
        } else {
            None
        }
    }

    /// Updates hover selection for `Main` and `QuitConfirm` pause sub-phases. Returns `true`
    /// when the highlighted item changed.
    pub fn handle_mouse_move(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> bool {
        if !self.is_open {
            return false;
        }
        let Some((vx, vy)) = Self::window_to_pause_virtual(mouse_x, mouse_y, win_w, win_h) else {
            return false;
        };
        match self.sub_phase {
            PauseSubPhase::Main => {
                if let Some(idx) = self.hit_test_main_row(vx, vy) {
                    if idx != self.selected_index {
                        self.selected_index = idx;
                        return true;
                    }
                }
                false
            }
            PauseSubPhase::QuitConfirm => {
                if let Some(idx) = Self::hit_test_quit_confirm(vx, vy) {
                    if idx != self.quit_confirm_index {
                        self.quit_confirm_index = idx;
                        return true;
                    }
                }
                false
            }
            PauseSubPhase::InOptionsMenu => false,
        }
    }

    /// Handles left-click confirmation on `Main` or `QuitConfirm` pause items.
    pub fn handle_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> (MenuSoundCue, PauseAction) {
        if !self.is_open {
            return (MenuSoundCue::None, PauseAction::None);
        }
        let Some((vx, vy)) = Self::window_to_pause_virtual(mouse_x, mouse_y, win_w, win_h) else {
            return (MenuSoundCue::None, PauseAction::None);
        };
        match self.sub_phase {
            PauseSubPhase::Main => {
                if let Some(idx) = self.hit_test_main_row(vx, vy) {
                    self.selected_index = idx;
                    return self.handle_nav(MenuNavAction::Confirm);
                }
                (MenuSoundCue::None, PauseAction::None)
            }
            PauseSubPhase::QuitConfirm => {
                if let Some(idx) = Self::hit_test_quit_confirm(vx, vy) {
                    self.quit_confirm_index = idx;
                    return self.handle_nav(MenuNavAction::Confirm);
                }
                (MenuSoundCue::None, PauseAction::None)
            }
            PauseSubPhase::InOptionsMenu => (MenuSoundCue::None, PauseAction::None),
        }
    }
}

#[cfg(test)]
mod tests;
