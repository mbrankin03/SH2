//! Mouse, keyboard, and gamepad navigation handlers for [`AppMenuController`].

use super::AppMenuController;
use sh2_ui::menu::{MenuNavAction, MenuSoundCue, TitlePhase};
use sh2_ui::options::{OptionsMenu, OptionsSoundCue};
use sh2_ui::pause::{PauseAction, PauseSubPhase};

impl AppMenuController {
    /// Routes mouse cursor hover `(mouse_x, mouse_y)` in window pixels across Title, Options, and Pause menus.
    pub fn handle_mouse_move(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> MenuSoundCue {
        let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(win_w, win_h);
        if cw <= 0.0 || ch <= 0.0 {
            return MenuSoundCue::None;
        }
        let ps2_x = (mouse_x - ox) * 512.0 / cw;
        let ps2_y = (mouse_y - oy) * 448.0 / ch;

        if self.in_game_active {
            if self.pause_menu.is_open {
                let moved = if self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu {
                    self.options_menu.handle_mouse_move(ps2_x, ps2_y)
                } else {
                    self.pause_menu
                        .handle_mouse_move(mouse_x, mouse_y, win_w, win_h)
                };
                if moved {
                    self.revision = self.revision.wrapping_add(1);
                    return MenuSoundCue::CursorMove;
                }
            }
            return MenuSoundCue::None;
        }

        if self.title_sm.phase == TitlePhase::OptionsMenu {
            if self.options_menu.handle_mouse_move(ps2_x, ps2_y) {
                self.revision = self.revision.wrapping_add(1);
                return MenuSoundCue::CursorMove;
            }
            MenuSoundCue::None
        } else if self
            .title_sm
            .handle_mouse_move(ps2_x - 256.0, ps2_y - 224.0)
        {
            self.revision = self.revision.wrapping_add(1);
            MenuSoundCue::CursorMove
        } else {
            MenuSoundCue::None
        }
    }

    /// Routes left mouse click `(mouse_x, mouse_y)` in window pixels across Title and Options screens.
    pub fn handle_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> MenuSoundCue {
        let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(win_w, win_h);
        if cw <= 0.0 || ch <= 0.0 {
            return MenuSoundCue::None;
        }
        let ps2_x = (mouse_x - ox) * 512.0 / cw;
        let ps2_y = (mouse_y - oy) * 448.0 / ch;

        if self.title_sm.phase == TitlePhase::OptionsMenu {
            self.revision = self.revision.wrapping_add(1);
            if self.options_menu.is_fmv_viewer_selected()
                || self.options_menu.is_loading_test_selected()
                || self.options_menu.is_test_rooms_selected()
                || self.options_menu.sub_screen != sh2_ui::options::OptionsSubScreen::None
            {
                return self.handle_nav(MenuNavAction::Confirm);
            }
            if let Some(cue) = self.options_menu.handle_mouse_click(ps2_x, ps2_y) {
                let (saved_cfg, _) = self.options_menu.commit_changes();
                self.config_applied_pending = Some(saved_cfg.clone());
                let _ = saved_cfg.save_to_file(&self.config_path);
                self.refresh_save_dir(&saved_cfg);
                return match cue {
                    OptionsSoundCue::CursorMove => MenuSoundCue::Confirm,
                    OptionsSoundCue::ValueChange => MenuSoundCue::CursorMove,
                    _ => MenuSoundCue::Confirm,
                };
            }
            MenuSoundCue::None
        } else {
            self.title_sm
                .handle_mouse_move(ps2_x - 256.0, ps2_y - 224.0);
            self.handle_nav(MenuNavAction::Confirm)
        }
    }

    /// Routes left mouse click `(mouse_x, mouse_y)` in window pixels when the in-game Pause Menu
    /// (Main, QuitConfirm, or Pause -> Options) is open.
    pub fn handle_pause_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> (MenuSoundCue, PauseAction) {
        if !self.in_game_active || !self.pause_menu.is_open {
            return (MenuSoundCue::None, PauseAction::None);
        }
        if self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu {
            let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(win_w, win_h);
            if cw <= 0.0 || ch <= 0.0 {
                return (MenuSoundCue::None, PauseAction::None);
            }
            let ps2_x = (mouse_x - ox) * 512.0 / cw;
            let ps2_y = (mouse_y - oy) * 448.0 / ch;
            if let Some(cue) = self.options_menu.handle_mouse_click(ps2_x, ps2_y) {
                self.revision = self.revision.wrapping_add(1);
                let (saved_cfg, _) = self.options_menu.commit_changes();
                self.config_applied_pending = Some(saved_cfg.clone());
                let _ = saved_cfg.save_to_file(&self.config_path);
                self.refresh_save_dir(&saved_cfg);
                let sound = match cue {
                    OptionsSoundCue::ValueChange => MenuSoundCue::CursorMove,
                    _ => MenuSoundCue::Confirm,
                };
                return (sound, PauseAction::None);
            }
            return (MenuSoundCue::None, PauseAction::None);
        }
        let (sound, action) = self
            .pause_menu
            .handle_mouse_click(mouse_x, mouse_y, win_w, win_h);
        if sound != MenuSoundCue::None || action != PauseAction::None {
            self.revision = self.revision.wrapping_add(1);
        }
        if action == PauseAction::OpenOptions {
            let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
            self.options_menu = OptionsMenu::new_with_audio_packs(
                self.options_menu.original_config.clone(),
                self.audio_pack_status,
            );
            self.options_menu.set_audio_devices(fresh_devices);
        } else if action == PauseAction::QuitToTitle {
            self.in_game_active = false;
            self.title_sm.phase = TitlePhase::MainMenu;
            self.title_sm.fade_alpha = 1.0;
        }
        (sound, action)
    }

    /// Routes mouse scroll wheel (`steps > 0` = scroll down, `steps < 0` = scroll up) inside the Options menu.
    pub fn handle_mouse_wheel(
        &mut self,
        steps: i32,
        cursor_pos: Option<(f64, f64)>,
        win_w: f32,
        win_h: f32,
    ) -> MenuSoundCue {
        let in_options = (!self.in_game_active && self.title_sm.phase == TitlePhase::OptionsMenu)
            || (self.in_game_active
                && self.pause_menu.is_open
                && self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu);
        if !in_options || steps == 0 {
            return MenuSoundCue::None;
        }
        let hover_ps2 = cursor_pos.and_then(|(mx, my)| {
            let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(win_w, win_h);
            if cw > 0.0 && ch > 0.0 {
                Some(((mx as f32 - ox) * 512.0 / cw, (my as f32 - oy) * 448.0 / ch))
            } else {
                None
            }
        });
        let cue = self.options_menu.handle_mouse_wheel(steps, hover_ps2);
        if cue != OptionsSoundCue::None {
            self.revision = self.revision.wrapping_add(1);
            MenuSoundCue::CursorMove
        } else {
            MenuSoundCue::None
        }
    }

    /// Handle unified navigation input across Title and Options screens.
    pub fn handle_nav(&mut self, action: MenuNavAction) -> MenuSoundCue {
        self.revision = self.revision.wrapping_add(1);
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
                    if self.options_menu.active_tab == sh2_ui::options::OptionsTab::Audio {
                        self.config_applied_pending = Some(self.options_menu.draft_config.clone());
                    }
                    if cue == OptionsSoundCue::LaunchFmvViewer {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer;
                        self.title_sm.launch_target = sh2_ui::menu::LaunchTarget::TestRoom(
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer,
                        );
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        MenuSoundCue::GameStartTransition
                    } else if cue == OptionsSoundCue::LaunchTestRooms {
                        self.title_sm.phase = TitlePhase::TestRoomsSelect;
                        self.title_sm.test_room_index = 0;
                        MenuSoundCue::Confirm
                    } else if cue == OptionsSoundCue::LaunchLoadingTest {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::LoadingScreenTest;
                        self.title_sm.launch_target = sh2_ui::menu::LaunchTarget::TestRoom(
                            sh2_ui::menu::TestRoomKind::LoadingScreenTest,
                        );
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        MenuSoundCue::GameStartTransition
                    } else {
                        MenuSoundCue::CursorMove
                    }
                }
                MenuNavAction::Right => {
                    let cue = self.options_menu.adjust_value(1);
                    if self.options_menu.active_tab == sh2_ui::options::OptionsTab::Audio {
                        self.config_applied_pending = Some(self.options_menu.draft_config.clone());
                    }
                    if cue == OptionsSoundCue::LaunchFmvViewer {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer;
                        self.title_sm.launch_target = sh2_ui::menu::LaunchTarget::TestRoom(
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer,
                        );
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        MenuSoundCue::GameStartTransition
                    } else if cue == OptionsSoundCue::LaunchTestRooms {
                        self.title_sm.phase = TitlePhase::TestRoomsSelect;
                        self.title_sm.test_room_index = 0;
                        MenuSoundCue::Confirm
                    } else if cue == OptionsSoundCue::LaunchLoadingTest {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::LoadingScreenTest;
                        self.title_sm.launch_target = sh2_ui::menu::LaunchTarget::TestRoom(
                            sh2_ui::menu::TestRoomKind::LoadingScreenTest,
                        );
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        MenuSoundCue::GameStartTransition
                    } else {
                        MenuSoundCue::CursorMove
                    }
                }
                MenuNavAction::Confirm => {
                    if let Some(sub_cue) = self.options_menu.handle_subscreen_confirm() {
                        return if sub_cue == OptionsSoundCue::None {
                            MenuSoundCue::None
                        } else {
                            MenuSoundCue::Confirm
                        };
                    }
                    if self.options_menu.enter_submenu() {
                        return MenuSoundCue::Confirm;
                    }
                    if self.options_menu.is_fmv_viewer_selected() {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer;
                        self.title_sm.launch_target = sh2_ui::menu::LaunchTarget::TestRoom(
                            sh2_ui::menu::TestRoomKind::FmvCutsceneViewer,
                        );
                        self.title_sm.phase = TitlePhase::TransitionToGame;
                        self.title_sm.start_fade_out();
                        return MenuSoundCue::GameStartTransition;
                    }
                    if self.options_menu.is_loading_test_selected() {
                        self.title_sm.selected_test_room =
                            sh2_ui::menu::TestRoomKind::LoadingScreenTest;
                        self.title_sm.launch_target = sh2_ui::menu::LaunchTarget::TestRoom(
                            sh2_ui::menu::TestRoomKind::LoadingScreenTest,
                        );
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
                    self.rebuild_mod_vfs(&saved_cfg);
                    self.config_applied_pending = Some(saved_cfg.clone());
                    if let Err(e) = saved_cfg.save_to_file(&self.config_path) {
                        tracing::warn!("Failed to persist config to {:?}: {}", self.config_path, e);
                    } else {
                        tracing::info!("Saved configuration to {:?}", self.config_path);
                    }
                    self.refresh_save_dir(&saved_cfg);
                    if cue == OptionsSoundCue::ConfirmSave {
                        MenuSoundCue::Confirm
                    } else {
                        MenuSoundCue::None
                    }
                }
                MenuNavAction::Cancel => {
                    if self.options_menu.close_sub_screen() {
                        return MenuSoundCue::Cancel;
                    }
                    if self.options_menu.exit_to_root() {
                        return MenuSoundCue::Cancel;
                    }
                    // Discard and return to Main Menu, restoring original audio config if live-previewed
                    self.options_menu.discard_changes();
                    self.config_applied_pending = Some(self.options_menu.original_config.clone());
                    self.title_sm.phase = TitlePhase::MainMenu;
                    self.title_sm.start_fade_in_duration(0.5);
                    MenuSoundCue::Cancel
                }
            }
        } else {
            let cue = self.title_sm.handle_input(action);
            if self.title_sm.phase == TitlePhase::OptionsMenu {
                // Refresh live audio output devices upon opening options menu so newly plugged devices appear immediately
                let existing_mods = self.options_menu.mods.clone();
                let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
                self.options_menu = OptionsMenu::new_with_audio_packs(
                    self.options_menu.original_config.clone(),
                    self.audio_pack_status,
                );
                self.options_menu.set_audio_devices(fresh_devices);
                self.options_menu.set_mods(existing_mods);
            }
            cue
        }
    }

    /// Cycle tabs in Options Menu using shoulder triggers / Q & E keys.
    pub fn switch_options_tab(&mut self, forward: bool) -> MenuSoundCue {
        self.revision = self.revision.wrapping_add(1);
        if self.title_sm.phase == TitlePhase::OptionsMenu
            || (self.pause_menu.is_open
                && self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu)
        {
            // If switching into Audio tab and no audio devices have been populated yet, merge live discovered devices
            if ((!forward && self.options_menu.active_tab == sh2_ui::options::OptionsTab::Controls)
                || (forward && self.options_menu.active_tab == sh2_ui::options::OptionsTab::Video))
                && self.options_menu.audio_devices.len() <= 1
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
        self.revision = self.revision.wrapping_add(1);
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
                    if self.options_menu.active_tab == sh2_ui::options::OptionsTab::Audio {
                        self.config_applied_pending = Some(self.options_menu.draft_config.clone());
                    }
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Right => {
                    self.options_menu.adjust_value(1);
                    if self.options_menu.active_tab == sh2_ui::options::OptionsTab::Audio {
                        self.config_applied_pending = Some(self.options_menu.draft_config.clone());
                    }
                    (MenuSoundCue::CursorMove, PauseAction::None)
                }
                MenuNavAction::Confirm => {
                    if let Some(sub_cue) = self.options_menu.handle_subscreen_confirm() {
                        let s = if sub_cue == OptionsSoundCue::None {
                            MenuSoundCue::None
                        } else {
                            MenuSoundCue::Confirm
                        };
                        return (s, PauseAction::None);
                    }
                    if self.options_menu.enter_submenu() {
                        return (MenuSoundCue::Confirm, PauseAction::None);
                    }
                    let (saved_cfg, cue) = self.options_menu.commit_changes();
                    self.rebuild_mod_vfs(&saved_cfg);
                    self.config_applied_pending = Some(saved_cfg.clone());
                    if let Err(e) = saved_cfg.save_to_file(&self.config_path) {
                        tracing::warn!("Failed to persist config to {:?}: {}", self.config_path, e);
                    } else {
                        tracing::info!("Saved in-game configuration to {:?}", self.config_path);
                    }
                    self.refresh_save_dir(&saved_cfg);
                    let sound = if cue == OptionsSoundCue::ConfirmSave {
                        MenuSoundCue::Confirm
                    } else {
                        MenuSoundCue::None
                    };
                    (sound, PauseAction::None)
                }
                MenuNavAction::Cancel => {
                    if self.options_menu.close_sub_screen() {
                        return (MenuSoundCue::Cancel, PauseAction::None);
                    }
                    if self.options_menu.exit_to_root() {
                        return (MenuSoundCue::Cancel, PauseAction::None);
                    }
                    self.options_menu.discard_changes();
                    self.config_applied_pending = Some(self.options_menu.original_config.clone());
                    self.pause_menu.sub_phase = PauseSubPhase::Main;
                    (MenuSoundCue::Cancel, PauseAction::None)
                }
            }
        } else {
            let (cue, action) = self.pause_menu.handle_nav(action);
            if action == PauseAction::OpenOptions {
                // Refresh live audio output devices upon opening options menu so newly plugged devices appear immediately
                let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
                self.options_menu = OptionsMenu::new_with_audio_packs(
                    self.options_menu.original_config.clone(),
                    self.audio_pack_status,
                );
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
}
