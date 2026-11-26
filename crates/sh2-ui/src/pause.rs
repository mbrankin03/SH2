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

    /// Maps window pixel coordinates `(mouse_x, mouse_y)` into the 4:3 pillarboxed `512 × 480`
    /// virtual coordinate space used by [`draw_pause_screen_with_scrim`].
    fn window_to_pause_virtual(
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> Option<(f32, f32)> {
        if win_w <= 0.0 || win_h <= 0.0 {
            return None;
        }
        let target_4_3_w = win_h * (4.0 / 3.0);
        let (x_offset, base_w) = if target_4_3_w <= win_w {
            ((win_w - target_4_3_w) * 0.5, target_4_3_w)
        } else {
            (0.0, win_w)
        };
        let vx = (mouse_x - x_offset) * PAUSE_VIRTUAL_W / base_w;
        let vy = mouse_y * PAUSE_VIRTUAL_H / win_h;
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

/// Retail PC `sh2pc.exe` Pause Menu 512×480 virtual coordinates (`FUN_00407300` `0x00407300`
/// and `FUN_00407610` `0x00407610`).
pub const PAUSE_VIRTUAL_W: f32 = 512.0;
pub const PAUSE_VIRTUAL_H: f32 = 480.0;
/// `"PAUSED"` header position `(0x100, 0xa4)` in `FUN_00407300` (`0x00407300`).
pub const PAUSE_HEADER_XY: (f32, f32) = (256.0, 164.0);
/// First pause item Y (`0xe6` = `230`) and row pitch (`0x1e` = `30`) in `FUN_00407300` (`0x00407300`).
pub const PAUSE_ITEM_START_Y: f32 = 230.0;
pub const PAUSE_ITEM_PITCH_Y: f32 = 30.0;
/// Quit-confirm prompt `(0x100, 0xe8)` and Yes/No `(0xe2/0x11e, 0x10a)` in `FUN_00407610` (`0x00407610`).
pub const PAUSE_QUIT_PROMPT_XY: (f32, f32) = (256.0, 232.0);
pub const PAUSE_QUIT_YES_XY: (f32, f32) = (226.0, 266.0);
pub const PAUSE_QUIT_NO_XY: (f32, f32) = (286.0, 266.0);

/// Renders the authentic Pause Menu overlay into `rasterizer`, scaled cleanly into a 4:3
/// pillarboxed viewport using exact retail `sh2pc.exe` 512×448 virtual coordinates
/// (`FUN_00407300` `0x00407300`, `FUN_00407610` `0x00407610`).
pub fn draw_pause_screen(
    pause_menu: &PauseMenu,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    draw_pause_screen_with_scrim(pause_menu, font, rasterizer, true);
}

/// Same as [`draw_pause_screen`], allowing the caller (`sh2-app`) to disable the inner 4:3 dim
/// scrim when a full-screen widescreen backdrop scrim has already been rendered across the
/// entire swapchain.
pub fn draw_pause_screen_with_scrim(
    pause_menu: &PauseMenu,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    draw_scrim: bool,
) {
    if !pause_menu.is_open {
        return;
    }

    let w = rasterizer.width;
    let h = rasterizer.height;

    // 4:3 pillarboxed projection from 512×448 retail PC virtual coordinates
    let target_4_3_w = h as f32 * (4.0 / 3.0);
    let (x_offset, base_w) = if target_4_3_w <= w as f32 {
        ((w as f32 - target_4_3_w) * 0.5, target_4_3_w)
    } else {
        (0.0, w as f32)
    };
    let scale_x = base_w / PAUSE_VIRTUAL_W;
    let scale_y = h as f32 / PAUSE_VIRTUAL_H;
    let to_px = |vx: f32, vy: f32| -> (usize, usize) {
        (
            (x_offset + vx * scale_x).round().max(0.0) as usize,
            (vy * scale_y).round().max(0.0) as usize,
        )
    };

    // 1. Optional semi-transparent dark backdrop scrim over paused frame (`sh2gfw_Set_PauseRetain`)
    if draw_scrim {
        let dim_color = [0u8, 0, 0, 96];
        for y in 0..h {
            for x in 0..w {
                rasterizer.blend_pixel(x, y, dim_color);
            }
        }
    }

    // `FUN_00407300` (`0x00407300`) uses `FontSize[0]` (`20x30` in `512x480`, `x_scale = scale_x / scale_y`).
    let font_px = (30.0 * scale_y).round().max(14.0) as usize;
    let sub_size = (20.0 * scale_y).round().max(10.0) as usize;
    let x_scale = scale_x / scale_y;
    let pop_dx = (2.0 * scale_x).round().max(1.0) as usize;
    let pop_dy = (2.0 * scale_y).round().max(1.0) as usize;

    match pause_menu.sub_phase {
        PauseSubPhase::Main => {
            // `"PAUSED"` at `(0x100, 0xa4)` (`256, 164`) — `FUN_00407300` `0x00407300:4750-4752`
            font.draw_line_centered_aspect(
                rasterizer,
                "PAUSED",
                to_px(PAUSE_HEADER_XY.0, PAUSE_HEADER_XY.1),
                font_px,
                x_scale,
                Sh2TextColor::White,
                true,
            );

            // Exact `sh2pc.exe` `FUN_00407300` (`0x00407300:4753, 4758`) `fontSetColorDirect` PS2 `0..0x80`
            // colors converted to 8-bit `0..255`:
            // - enabled: `(0x7f, 0x7f, 0x7f, 0x60)` -> `[254, 254, 254, 192]`
            // - disabled Save: `(0x55, 0x55, 0x55, 0x60)` -> `[170, 170, 170, 192]`
            let unselected_color = Sh2TextColor::Custom {
                top: [254, 254, 254, 192],
                bottom: [254, 254, 254, 192],
            };
            let disabled_color = Sh2TextColor::Custom {
                top: [170, 170, 170, 192],
                bottom: [170, 170, 170, 192],
            };

            // 5 items at `(256, 230 + idx * 30)` (`0xe6, 0x104, 0x122, 0x140, 0x15e`)
            for (idx, opt) in PauseMenuOption::ALL.iter().enumerate() {
                let vy = PAUSE_ITEM_START_Y + idx as f32 * PAUSE_ITEM_PITCH_Y;
                let pos = to_px(256.0, vy);
                let is_selected = idx == pause_menu.selected_index;
                let is_disabled = *opt == PauseMenuOption::Save && !pause_menu.can_save;

                let base_color = if is_disabled {
                    disabled_color
                } else {
                    unselected_color
                };

                // Unselected base pass (`alpha = 0x60/0x80 = 192`)
                font.draw_line_centered_aspect(
                    rasterizer,
                    opt.label(),
                    pos,
                    font_px,
                    x_scale,
                    base_color,
                    true,
                );

                // Selected double-draw pop pass at `(-2, -2)` (`0xfe, vy - 2`, `alpha = 0xff`)
                if is_selected && !is_disabled {
                    font.draw_line_centered_aspect(
                        rasterizer,
                        opt.label(),
                        (pos.0.saturating_sub(pop_dx), pos.1.saturating_sub(pop_dy)),
                        font_px,
                        x_scale,
                        Sh2TextColor::White,
                        false,
                    );
                }
            }

            if let Some(ref msg) = pause_menu.status_message {
                font.draw_line_centered_aspect(
                    rasterizer,
                    msg,
                    to_px(256.0, 392.0),
                    sub_size,
                    x_scale,
                    Sh2TextColor::GoldSelected,
                    true,
                );
            }
        }
        PauseSubPhase::QuitConfirm => {
            // `"Exit to main menu?"` at `(0x100, 0xe8)` (`256, 232`) — `FUN_00407610` `0x00407610`
            font.draw_line_centered_aspect(
                rasterizer,
                "Exit to main menu?",
                to_px(PAUSE_QUIT_PROMPT_XY.0, PAUSE_QUIT_PROMPT_XY.1),
                font_px,
                x_scale,
                Sh2TextColor::White,
                true,
            );

            let yes_pos = to_px(PAUSE_QUIT_YES_XY.0, PAUSE_QUIT_YES_XY.1);
            let no_pos = to_px(PAUSE_QUIT_NO_XY.0, PAUSE_QUIT_NO_XY.1);

            // Base unselected pass (`FUN_00407610` `0x00407610`: PS2 `0..0x80` `(0x7f, 0x7f, 0x7f, 0x60)`)
            let unselected_color = Sh2TextColor::Custom {
                top: [254, 254, 254, 192],
                bottom: [254, 254, 254, 192],
            };
            font.draw_line_centered_aspect(
                rasterizer,
                "Yes",
                yes_pos,
                font_px,
                x_scale,
                unselected_color,
                true,
            );
            font.draw_line_centered_aspect(
                rasterizer,
                "No",
                no_pos,
                font_px,
                x_scale,
                unselected_color,
                true,
            );

            // Selected pop pass at `(-2, -2)` (`0xe0, 0x108` or `0x11c, 0x108`, `alpha = 0xff`)
            let (sel_label, sel_pos) = if pause_menu.quit_confirm_index == 0 {
                ("Yes", yes_pos)
            } else {
                ("No", no_pos)
            };
            font.draw_line_centered_aspect(
                rasterizer,
                sel_label,
                (
                    sel_pos.0.saturating_sub(pop_dx),
                    sel_pos.1.saturating_sub(pop_dy),
                ),
                font_px,
                x_scale,
                Sh2TextColor::White,
                false,
            );
        }
        PauseSubPhase::InOptionsMenu => {}
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
