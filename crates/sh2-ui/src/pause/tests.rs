use super::*;
use crate::atlas_font::AtlasFont;
use crate::font::TextRasterizer;

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

#[test]
fn test_pause_menu_mouse_move_click_and_letterbox_parity() {
    fn pause_virtual_to_win(vx: f32, vy: f32, win_w: f32, win_h: f32) -> (f32, f32) {
        let (ox, oy, cw, ch) = crate::inventory::ps2_ui_canvas_rect(win_w, win_h);
        (
            ox + vx * cw / PAUSE_VIRTUAL_W,
            oy + vy * ch / PAUSE_VIRTUAL_H,
        )
    }

    let mut pause = PauseMenu::new();
    // Closed menu ignores mouse input
    assert!(!pause.handle_mouse_move(640.0, 360.0, 1280.0, 720.0));
    assert_eq!(
        pause.handle_mouse_click(640.0, 360.0, 1280.0, 720.0),
        (MenuSoundCue::None, PauseAction::None)
    );

    // Open with `can_save = false` on a widescreen (1920x1080) window
    pause.open(false, "Restroom", 15.0);
    assert!(!pause.handle_mouse_move(100.0, 100.0, 0.0, 1080.0));

    // Row 1 is Save (disabled when `can_save == false`): hover & click must be ignored
    let save_vy = PAUSE_ITEM_START_Y + 1.0 * PAUSE_ITEM_PITCH_Y + 10.0;
    let (sx, sy) = pause_virtual_to_win(256.0, save_vy, 1920.0, 1080.0);
    assert!(!pause.handle_mouse_move(sx, sy, 1920.0, 1080.0));
    assert_eq!(pause.selected_index, 0);
    assert_eq!(
        pause.handle_mouse_click(sx, sy, 1920.0, 1080.0),
        (MenuSoundCue::None, PauseAction::None)
    );

    // Row 2 is Load: hover selects index 2, click triggers Load
    let load_vy = PAUSE_ITEM_START_Y + 2.0 * PAUSE_ITEM_PITCH_Y + 10.0;
    let (lx, ly) = pause_virtual_to_win(256.0, load_vy, 1920.0, 1080.0);
    assert!(pause.handle_mouse_move(lx, ly, 1920.0, 1080.0));
    assert_eq!(pause.selected_index, 2);
    assert_eq!(
        pause.handle_mouse_click(lx, ly, 1920.0, 1080.0),
        (MenuSoundCue::Confirm, PauseAction::Load)
    );

    // Portrait (< 4:3) window (600x1000) vertical letterboxing parity:
    // Row 4 is Quit -> opens QuitConfirm, then hover/click "Yes" confirms QuitToTitle.
    pause.open(true, "Restroom", 15.0);
    let quit_vy = PAUSE_ITEM_START_Y + 4.0 * PAUSE_ITEM_PITCH_Y + 10.0;
    let (qx, qy) = pause_virtual_to_win(256.0, quit_vy, 600.0, 1000.0);
    assert!(pause.handle_mouse_move(qx, qy, 600.0, 1000.0));
    assert_eq!(pause.selected_index, 4);
    assert_eq!(
        pause.handle_mouse_click(qx, qy, 600.0, 1000.0),
        (MenuSoundCue::Confirm, PauseAction::None)
    );
    assert_eq!(pause.sub_phase, PauseSubPhase::QuitConfirm);
    assert_eq!(pause.quit_confirm_index, 1);

    // Hover and click "Yes" in portrait window
    let (yx, yy) = pause_virtual_to_win(
        PAUSE_QUIT_YES_XY.0,
        PAUSE_QUIT_YES_XY.1 + 8.0,
        600.0,
        1000.0,
    );
    assert!(pause.handle_mouse_move(yx, yy, 600.0, 1000.0));
    assert_eq!(pause.quit_confirm_index, 0);
    assert_eq!(
        pause.handle_mouse_click(yx, yy, 600.0, 1000.0),
        (MenuSoundCue::Confirm, PauseAction::QuitToTitle)
    );
    assert!(!pause.is_open);
}
