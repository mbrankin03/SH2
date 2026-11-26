//! Interactive Main Menu overlay and options presenter.
//! Integrates `TitleStateMachine` and `OptionsMenu` into the interactive render loop.

use sh2_core::config::{AppConfig, AudioPackDetectionStatus};
use sh2_ui::menu::{
    LaunchTarget, MainMenuOption, MenuNavAction, MenuSoundCue, TitlePhase, TitleStateMachine,
};
use sh2_ui::options::{OptionsMenu, OptionsSoundCue};
use sh2_ui::pause::{PauseAction, PauseMenu, PauseSubPhase};
use sh2_ui::LoadingScreen;
use std::path::{Path, PathBuf};

/// Outcome of advancing a single frame of the title-to-game [`LoadingScreen`] transition (`SH2-75`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadingTransitionStep {
    /// `true` once the loading screen has rendered at least one frame and the caller should
    /// preload target room assets (`.map`, `.cld`, `.cam`, GPU bind groups) before calling
    /// [`LoadingScreen::mark_completed`].
    pub needs_room_preload: bool,
    /// `Some(is_right)` when a new footstep print was stamped on this tick (`FUN_00406be0`).
    pub footstep_spawned: Option<bool>,
    /// `true` on the frame that transitions cleanly into `in_game_active = true`.
    pub entered_game: bool,
}

/// App-level menu manager coordinating title sequence, scenario selection, pause menu, and settings persistence.
#[derive(Debug)]
pub struct AppMenuController {
    pub title_sm: TitleStateMachine,
    pub options_menu: OptionsMenu,
    pub pause_menu: PauseMenu,
    pub config_path: PathBuf,
    /// Resolved save directory (`~/Documents/Silent Hill 2/saves`, Android app files dir, or override).
    pub save_dir: PathBuf,
    /// Has the user completed title sequence and transitioned to in-game world?
    pub in_game_active: bool,
    /// Flag indicating that configuration was committed and needs to be applied by the engine loop.
    pub config_applied_pending: Option<AppConfig>,
    /// Revision counter bumped whenever interactive menu selections or phases change.
    pub revision: u64,
    /// Detected filesystem status for SH2:EE Lossless (`sh2e/sound/`) and Original PS2/PC (`data/sound/`) audio packs (`SH2-59`).
    pub audio_pack_status: AudioPackDetectionStatus,
    /// Layered Virtual File System (`mods/ -> sh2e/ -> data/`) built from `config.mods` (`SH2-29`).
    pub mod_vfs: std::sync::Arc<sh2_assets::ModVfs>,
}

impl AppMenuController {
    fn build_mod_vfs(
        resolver: &sh2_assets::AssetPathResolver,
        cfg: &AppConfig,
    ) -> (std::sync::Arc<sh2_assets::ModVfs>, Vec<sh2_ui::ModUiEntry>) {
        let discovered = sh2_assets::discover_mods(
            &resolver.resolve_mods_dir(),
            &cfg.mods.as_discovery_tuples(),
        );
        let mod_entries: Vec<sh2_ui::ModUiEntry> = discovered
            .iter()
            .map(|m| sh2_ui::ModUiEntry {
                id: m.id.clone(),
                name: m.metadata.name.clone(),
                author: m.metadata.author.clone(),
                version: m.metadata.version.clone(),
                description: m.metadata.description.clone(),
                override_count: m.overrides.len(),
                enabled: m.enabled,
                priority: m.priority,
                is_zip: m.source.is_zip(),
            })
            .collect();
        let vfs = std::sync::Arc::new(sh2_assets::ModVfs::from_packages(
            &discovered,
            resolver.sh2e_dir.clone(),
            resolver.data_dir.clone(),
        ));
        (vfs, mod_entries)
    }

    /// Rebuilds [`Self::mod_vfs`] from the active `mods/` directory and `cfg.mods` state.
    pub fn rebuild_mod_vfs(&mut self, cfg: &AppConfig) {
        let resolver = sh2_assets::AssetPathResolver::resolve();
        let (vfs, _) = Self::build_mod_vfs(&resolver, cfg);
        self.mod_vfs = vfs;
    }
    /// Initialize with a configuration file path (e.g. `config.json`), scanning the active game root for audio packs.
    pub fn new<P: AsRef<Path>>(config_path: P) -> Self {
        let resolver = sh2_assets::AssetPathResolver::resolve();
        let status = AudioPackDetectionStatus::detect_from_root(&resolver.game_root);
        // In test environments where no retail/sh2e assets exist on disk at `resolver.game_root`,
        // default to both packs present unless explicitly overridden via `new_with_audio_packs`.
        let effective_status = if !status.sh2ee_lossless_present && !status.ps2_original_present {
            AudioPackDetectionStatus::default()
        } else {
            status
        };
        Self::new_with_audio_packs(config_path, effective_status)
    }

    /// Initialize with a configuration file path and an explicit [`AudioPackDetectionStatus`].
    pub fn new_with_audio_packs<P: AsRef<Path>>(
        config_path: P,
        audio_pack_status: AudioPackDetectionStatus,
    ) -> Self {
        let path = config_path.as_ref().to_path_buf();
        let loaded_config = AppConfig::load_from_file(&path);
        let mut options_menu =
            OptionsMenu::new_with_audio_packs(loaded_config.clone(), audio_pack_status);
        let resolver = sh2_assets::AssetPathResolver::resolve();
        let (mod_vfs, mod_entries) = Self::build_mod_vfs(&resolver, &loaded_config);
        options_menu.set_mods(mod_entries);
        let mut title_sm = TitleStateMachine::new();
        let save_dir = crate::save_paths::resolve_save_dir(&path, &loaded_config);
        let repo = sh2_core::campaign::SaveSlotRepository::new(&save_dir);
        let has_saves = !repo
            .scan_slots(
                loaded_config.game.enable_unlimited_save_slots,
                loaded_config.game.enable_quicksave_autosave,
            )
            .is_empty();
        if has_saves {
            title_sm.set_save_availability(sh2_ui::menu::SaveAvailability::LoadAndContinue);
        }
        let pause_menu = PauseMenu::new();

        Self {
            title_sm,
            options_menu,
            pause_menu,
            config_path: path,
            save_dir,
            in_game_active: false,
            config_applied_pending: None,
            revision: 0,
            audio_pack_status,
            mod_vfs,
        }
    }

    /// Re-resolves [`Self::save_dir`] from `cfg` and `self.config_path`.
    pub fn refresh_save_dir(&mut self, cfg: &AppConfig) {
        self.save_dir = crate::save_paths::resolve_save_dir(&self.config_path, cfg);
    }

    /// Returns the path of the primary campaign save slot file inside [`Self::save_dir`].
    #[must_use]
    pub fn save_slot_path(&self) -> PathBuf {
        sh2_core::campaign::SaveSlotRepository::new(&self.save_dir).slot_path(0, 0)
    }

    /// Updates the detected audio pack status on both the controller and the nested `OptionsMenu`.
    pub fn set_audio_pack_status(&mut self, status: AudioPackDetectionStatus) {
        self.audio_pack_status = status;
        self.options_menu.set_audio_pack_status(status);
    }

    /// Resolves the active concrete [`sh2_core::AudioPackPreference`] (`Sh2eeLossless` or `Ps2Original`)
    /// based on the user's configured preference (`Auto`, `Sh2eeLossless`, `Ps2Original`) and detected packs.
    pub fn resolved_audio_pack_source(&self) -> sh2_core::AudioPackPreference {
        self.audio_pack_status
            .resolve_source(self.options_menu.original_config.audio.audio_pack_source)
    }

    /// Process per-frame animations and fade transitions.
    pub fn tick(&mut self, dt: f32) {
        if !self.in_game_active {
            let prev_phase = self.title_sm.phase;
            self.title_sm.tick(dt);
            if self.title_sm.phase != prev_phase {
                self.revision = self.revision.wrapping_add(1);
            }
            if self.title_sm.phase == TitlePhase::TransitionToGame
                && (self.title_sm.fade_alpha <= 0.05 || !self.title_sm.is_fading_out)
            {
                self.complete_game_transition();
            }
        }
    }

    /// Immediately completes an active [`TitlePhase::TransitionToGame`] handoff into gameplay
    /// once [`sh2_ui::LoadingScreen::transition_ready`] is `true` (or when instant transitions are
    /// enabled), clearing title fade state so the loading screen cannot re-instantiate in a loop.
    pub fn complete_game_transition(&mut self) {
        if !self.in_game_active && self.title_sm.phase == TitlePhase::TransitionToGame {
            self.in_game_active = true;
            self.title_sm.fade_alpha = 0.0;
            self.title_sm.is_fading_out = false;
            self.title_sm.is_fading_in = false;
            self.revision = self.revision.wrapping_add(1);
            tracing::info!(
                "Title sequence complete. Launching scenario: {:?}, Action: {:?}, Riddle: {:?}",
                self.title_sm.selected_scenario,
                self.title_sm.action_difficulty,
                self.title_sm.riddle_difficulty
            );
        }
    }

    /// Advances the title menu and [`LoadingScreen`] transition lifecycle cleanly without looping
    /// (`SH2-75`).
    ///
    /// - When `self.in_game_active` is already `true`, this is a no-op so in-game campaign loading
    ///   screens are never overwritten or re-instantiated.
    /// - On Frame 1 of [`TitlePhase::TransitionToGame`], instantiates [`LoadingScreen`] with
    ///   `load_completed = false` and ticks once so the first footstep renders immediately.
    /// - On Frame 2 (while `!ls.load_completed`), signals `needs_room_preload = true` so the host
    ///   preloads `.map`/`.cld`/`.cam` + GPU bind groups before calling `ls.mark_completed()`.
    /// - Once `ls.transition_ready` becomes `true`, clears `*loading_screen` and calls
    ///   [`Self::complete_game_transition`] immediately so the screen never loops.
    pub fn tick_with_loading(
        &mut self,
        loading_screen: &mut Option<LoadingScreen>,
        dt: f32,
    ) -> LoadingTransitionStep {
        if self.in_game_active {
            return LoadingTransitionStep::default();
        }

        if self.title_sm.phase != TitlePhase::TransitionToGame {
            *loading_screen = None;
            self.tick(dt);
            return LoadingTransitionStep::default();
        }

        let debug = &self.options_menu.draft_config.debug;
        let min_dur = debug.min_loading_screen_seconds as f32;
        let require_press = debug.require_press_to_continue;
        let instant = debug.instant_room_transitions;

        let is_new_game_campaign = matches!(
            self.title_sm.launch_target,
            LaunchTarget::Campaign {
                is_continue: false,
                ..
            }
        );

        // Retail SH2 / SH2:EE goes straight from New Game -> black screen -> opening FMV
        // without showing the `NOW LOADING...` footstep screen (`SH2-150`).
        if instant || is_new_game_campaign {
            *loading_screen = None;
            self.complete_game_transition();
            return LoadingTransitionStep {
                needs_room_preload: false,
                footstep_spawned: None,
                entered_game: self.in_game_active,
            };
        }

        let just_created = if loading_screen.is_none() {
            let label = match self.title_sm.launch_target {
                LaunchTarget::Campaign { scenario, .. } => match scenario {
                    sh2_core::ScenarioKind::LetterFromSilentHeaven => "Letter from Silent Heaven",
                    sh2_core::ScenarioKind::BornFromAWish => "Born from a Wish",
                },
                LaunchTarget::TestRoom(room) => room.label(),
            };
            *loading_screen = Some(LoadingScreen::new(label, min_dur, require_press));
            true
        } else {
            false
        };

        let mut step = LoadingTransitionStep::default();
        if let Some(ref mut ls) = loading_screen {
            // Signal preload on Frame 2 (after Frame 1 has presented the loading screen visibly).
            if !just_created && !ls.load_completed {
                step.needs_room_preload = true;
            }
            ls.tick(dt);
            step.footstep_spawned = ls.spawned_step_this_tick();
            if ls.transition_ready {
                *loading_screen = None;
                self.complete_game_transition();
                step.entered_game = self.in_game_active;
            }
        }
        step
    }

    /// Cleanly exits an active in-game session or debug room and returns to `target_phase`
    /// with full menu opacity (`fade_alpha = 1.0`) and cleared fade state.
    pub fn return_to_menu(&mut self, target_phase: TitlePhase) {
        self.in_game_active = false;
        self.pause_menu.close();
        self.title_sm.show_phase_immediately(target_phase);
        if let Some(idx) = sh2_ui::menu::TestRoomKind::ALL
            .iter()
            .position(|&r| r == self.title_sm.selected_test_room)
        {
            self.title_sm.test_room_index = idx;
        }
        self.revision = self.revision.wrapping_add(1);
    }

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
            if self.pause_menu.is_open
                && self.pause_menu.sub_phase == PauseSubPhase::InOptionsMenu
                && self.options_menu.handle_mouse_move(ps2_x, ps2_y)
            {
                self.revision = self.revision.wrapping_add(1);
                return MenuSoundCue::CursorMove;
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

        // Navigate to Options (SaveAvailability::None -> [NewGame=0, Options=1, Movies=2, Exit=3])
        controller.handle_nav(MenuNavAction::Down); // Options
        controller.handle_nav(MenuNavAction::Confirm);
        assert_eq!(controller.title_sm.phase, TitlePhase::OptionsMenu);

        // Enter Advanced Options (row 1 on 10-row Root), change resolution, and commit
        controller.handle_nav(MenuNavAction::Down); // Advanced Options
        controller.handle_nav(MenuNavAction::Confirm);
        controller.handle_nav(MenuNavAction::Right);
        assert!(controller.options_menu.is_dirty);
        controller.handle_nav(MenuNavAction::Confirm);
        assert!(!controller.options_menu.is_dirty);
        assert!(cfg_path.exists());

        // Return to Root Options, then to main menu
        controller.handle_nav(MenuNavAction::Cancel);
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

    #[test]
    fn test_loading_screen_transition_no_38x_reset_loop_and_preload() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_sh2_loading_lifecycle_config.json");
        let _ = std::fs::remove_file(&cfg_path);

        let mut controller = AppMenuController::new(&cfg_path);
        controller
            .options_menu
            .draft_config
            .debug
            .min_loading_screen_seconds = 2;
        controller
            .options_menu
            .draft_config
            .debug
            .require_press_to_continue = false;
        controller
            .options_menu
            .draft_config
            .debug
            .instant_room_transitions = false;

        // Enter TransitionToGame with active fade_out (fade_alpha = 1.0, is_fading_out = true)
        controller.title_sm.phase = TitlePhase::TransitionToGame;
        controller.title_sm.fade_alpha = 1.0;
        controller.title_sm.start_fade_out();
        controller.title_sm.launch_target =
            LaunchTarget::TestRoom(sh2_ui::menu::TestRoomKind::ObservationDeck);

        let mut loading_screen: Option<LoadingScreen> = None;
        let mut instantiations = 0usize;
        let mut preload_requests = 0usize;
        let mut footsteps_triggered = 0usize;
        let mut entered_game_ticks = 0usize;

        // Simulate 200 frames at 60 Hz (~3.33s)
        for _ in 0..200 {
            let was_none = loading_screen.is_none();
            let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
            if was_none && loading_screen.is_some() {
                instantiations += 1;
            }
            if step.footstep_spawned.is_some() {
                footsteps_triggered += 1;
            }
            if step.needs_room_preload {
                preload_requests += 1;
                if let Some(ref mut ls) = loading_screen {
                    ls.mark_completed();
                }
            }
            if step.entered_game {
                entered_game_ticks += 1;
            }
        }

        // LoadingScreen must be instantiated EXACTLY ONCE (never 38x reset loop)
        assert_eq!(instantiations, 1);
        assert_eq!(preload_requests, 1);
        assert!(footsteps_triggered >= 2);
        assert_eq!(entered_game_ticks, 1);
        assert!(controller.in_game_active);
        assert!(loading_screen.is_none());
        assert_eq!(controller.title_sm.fade_alpha, 0.0);
        assert!(!controller.title_sm.is_fading_out);

        let _ = std::fs::remove_file(&cfg_path);
    }

    #[test]
    fn test_loading_screen_instant_transition_completes_immediately() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_sh2_loading_instant_config.json");
        let _ = std::fs::remove_file(&cfg_path);

        let mut controller = AppMenuController::new(&cfg_path);
        controller
            .options_menu
            .draft_config
            .debug
            .instant_room_transitions = true;
        controller.title_sm.phase = TitlePhase::TransitionToGame;
        controller.title_sm.fade_alpha = 1.0;
        controller.title_sm.start_fade_out();
        controller.title_sm.launch_target =
            LaunchTarget::TestRoom(sh2_ui::menu::TestRoomKind::ObservationDeck);

        let mut loading_screen: Option<LoadingScreen> = None;
        let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
        assert!(step.entered_game);
        assert!(controller.in_game_active);
        assert!(loading_screen.is_none());

        let _ = std::fs::remove_file(&cfg_path);
    }

    #[test]
    fn test_loading_screen_require_press_to_continue_confirm_transitions_once() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_sh2_loading_press_confirm_config.json");
        let _ = std::fs::remove_file(&cfg_path);

        let mut controller = AppMenuController::new(&cfg_path);
        controller
            .options_menu
            .draft_config
            .debug
            .min_loading_screen_seconds = 1;
        controller
            .options_menu
            .draft_config
            .debug
            .require_press_to_continue = true;
        controller
            .options_menu
            .draft_config
            .debug
            .instant_room_transitions = false;
        controller.title_sm.phase = TitlePhase::TransitionToGame;
        controller.title_sm.fade_alpha = 1.0;
        controller.title_sm.start_fade_out();
        controller.title_sm.launch_target =
            LaunchTarget::TestRoom(sh2_ui::menu::TestRoomKind::ObservationDeck);

        let mut loading_screen: Option<LoadingScreen> = None;

        // Tick 90 frames at 60 Hz (1.5s > min_duration 1.0s), preloading on Frame 2
        for _ in 0..90 {
            let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
            if step.needs_room_preload {
                if let Some(ref mut ls) = loading_screen {
                    ls.mark_completed();
                }
            }
            assert!(!step.entered_game);
        }
        // Still waiting for user confirm
        assert!(!controller.in_game_active);
        assert!(loading_screen.is_some());

        // User presses Confirm -> `handle_confirm()` marks `transition_ready = true`
        assert!(loading_screen.as_mut().unwrap().handle_confirm());

        // Next tick completes transition cleanly into `in_game_active = true` without looping
        let final_step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
        assert!(final_step.entered_game);
        assert!(controller.in_game_active);
        assert!(loading_screen.is_none());

        // Subsequent ticks never re-instantiate `LoadingScreen`
        for _ in 0..30 {
            let post_step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
            assert!(!post_step.entered_game);
            assert!(loading_screen.is_none());
        }

        let _ = std::fs::remove_file(&cfg_path);
    }

    #[test]
    fn test_new_game_skips_loading_screen_straight_to_black_and_fmv() {
        let temp_dir = std::env::temp_dir();
        let cfg_path = temp_dir.join("test_sh2_new_game_no_loading_config.json");
        let _ = std::fs::remove_file(&cfg_path);

        let mut controller = AppMenuController::new(&cfg_path);
        controller
            .options_menu
            .draft_config
            .debug
            .instant_room_transitions = false;
        controller
            .options_menu
            .draft_config
            .debug
            .min_loading_screen_seconds = 3;
        controller.title_sm.phase = TitlePhase::TransitionToGame;
        controller.title_sm.launch_target = LaunchTarget::Campaign {
            scenario: sh2_core::ScenarioKind::LetterFromSilentHeaven,
            is_continue: false,
        };

        let mut loading_screen: Option<LoadingScreen> = None;
        let step = controller.tick_with_loading(&mut loading_screen, 1.0 / 60.0);
        assert!(step.entered_game);
        assert!(controller.in_game_active);
        assert!(
            loading_screen.is_none(),
            "New Game must go straight to black screen + opening FMV with no loading screen"
        );

        let _ = std::fs::remove_file(&cfg_path);
    }
}
