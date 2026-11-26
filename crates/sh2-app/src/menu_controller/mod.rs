//! Interactive Main Menu overlay and options presenter.
//! Integrates `TitleStateMachine` and `OptionsMenu` into the interactive render loop.

mod input;
mod overlay;
#[cfg(test)]
mod tests;

use sh2_core::config::{AppConfig, AudioPackDetectionStatus};
use sh2_ui::menu::{LaunchTarget, TitlePhase, TitleStateMachine};
use sh2_ui::options::OptionsMenu;
use sh2_ui::pause::PauseMenu;
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
}
