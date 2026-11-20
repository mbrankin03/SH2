//! Title screen and main menu state machine (`SLUS_202.28` `Event/title.c` `0x001c0b00..0x001c4740`
//! and retail PC `sh2pc.exe` `0x00495520..0x00498d80`).

use super::boot::{BootProfile, BootSequenceState};
use super::types::{
    resolve_effective_riddle, LaunchTarget, MainMenuOption, MenuNavAction, MenuSoundCue,
    SaveAvailability, TestRoomKind, TitlePhase,
};
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::scenario::ScenarioKind;

/// Attract-mode idle timeout in seconds (`titleInit` `0x001c1260` / `sh2pc` `0x00496f62`,
/// `0x42700000` = `60.0f`).
pub const TITLE_ATTRACT_TIMEOUT_SECS: f32 = 60.0;

/// Title screen fade-in and fade-out duration in seconds (`titleFadeIn` `0x001c1880`,
/// `titleFadeOut` `0x001c30d0`, `titleFadeOutNewGame` `0x001c2ec0`, `0x40000000` = `2.0f`).
pub const TITLE_FADE_DURATION_SECS: f32 = 2.0;

/// Idle selection pulse phase increment per 30 Hz frame (`Event/title.c` `_DAT_0116d964`,
/// `sh2pc` `0x006ad7b0` = `0.06981317f` = `4°` = `π/45` rad/frame).
pub const TITLE_IDLE_PULSE_RAD_PER_30HZ: f32 = 0.069_813_17;

/// Confirm-blink phase increment per 30 Hz frame during `titleFadeOut` (`0x001c30d0` /
/// `sh2pc` `0x00498bd8`, `0x006a14e4` = `1.5707964f` = `π/2` rad/frame).
pub const TITLE_CONFIRM_BLINK_RAD_PER_30HZ: f32 = std::f32::consts::FRAC_PI_2;

/// Ordered Action difficulty rows matching `sh2pc.exe` `titleGetBattleLevelFromCursor`
/// (`0x00495b70`: cursor 1 = Hard, 2 = Normal, 3 = Easy, 4 = Beginner).
pub const ACTION_DIFFICULTY_ROWS: [ActionDifficulty; 4] = [
    ActionDifficulty::Hard,
    ActionDifficulty::Normal,
    ActionDifficulty::Easy,
    ActionDifficulty::Beginner,
];

/// Ordered visible Riddle difficulty rows matching `SLUS_202.28` `titleGetRiddleLevelFromCursor`
/// (`0x001c2e30`) & `sh2pc.exe` (`0x00495bf0`: cursor 1 = Hard, 2 = Normal, 3 = Easy).
/// Extra Riddle is unlocked silently on New Game start via [`resolve_effective_riddle`] (`0x0048aa20`).
pub const RIDDLE_DIFFICULTY_ROWS: [RiddleDifficulty; 3] = [
    RiddleDifficulty::Hard,
    RiddleDifficulty::Normal,
    RiddleDifficulty::Easy,
];

/// Title state machine coordinator (`Event/title.c` + `logo.c` + `sh2pc.exe` `0x00498d80`).
#[derive(Debug, Clone)]
pub struct TitleStateMachine {
    pub phase: TitlePhase,
    pub main_menu_index: usize,
    pub scenario_index: usize,
    pub action_difficulty: ActionDifficulty,
    pub riddle_difficulty: RiddleDifficulty,
    pub selected_scenario: ScenarioKind,
    /// Currently selected test room / sandbox stage.
    pub selected_test_room: TestRoomKind,
    /// Target session mode (`Campaign` vs `TestRoom`) resolved when entering `TransitionToGame` (`SH2-112`).
    pub launch_target: LaunchTarget,
    pub test_room_index: usize,
    /// Visual fade opacity: `0.0` (fully black) to `1.0` (fully visible).
    pub fade_alpha: f32,
    /// Fade rate in alpha/second (`1.0 / TITLE_FADE_DURATION_SECS` = `0.5` for 2.0s fade).
    pub fade_speed: f32,
    pub is_fading_in: bool,
    pub is_fading_out: bool,
    /// Boot logo sequence state (`logo.c` `0x0020c910..0x0020d220` / `sh2pc` `0x00405e80`).
    pub boot_sequence: BootSequenceState,
    /// Legacy timer for splash screen duration in seconds.
    pub splash_timer: f32,
    /// Continuous elapsed time in seconds across all title phases (drives `StartPrompt` pulse).
    pub elapsed_timer: f32,
    /// Attract-mode idle countdown in seconds (`_DAT_0116d95c`, initialized to `60.0f`).
    pub attract_timer: f32,
    /// Set to `true` when `attract_timer` reaches `0.0` in `MainMenu` (`titleExit` -> OP movie).
    pub attract_movie_requested: bool,
    /// Selection pulse / confirm-blink angle in radians (`_DAT_0116d964` / `DAT_0094d554`).
    pub pulse_phase_rad: f32,
    /// Save-data availability governing `LOAD` / `CONTINUE` slot visibility (`titleCreateMainMenu`).
    pub save_availability: SaveAvailability,
    /// Whether to append retail PC `MOVIES` (`0x16`) and `EXIT` (`0x15`) main-menu slots.
    pub include_pc_menu_slots: bool,
    /// Riddle clear bitmask (`bit 0 = Easy, bit 1 = Normal, bit 2 = Hard, bit 3 = Extra`) for
    /// `sh2pc.exe` `0x0048aa20` Extra Riddle unlock.
    pub riddle_clear_mask: u8,
    /// Alternate Extra Riddle toggle bit (`DAT_01dbbf85 & 1` at `0x0048aa20`).
    pub extra_riddle_toggle_bit: bool,
    /// Last audio cue emitted by state change.
    pub pending_audio_cue: MenuSoundCue,
}

impl Default for TitleStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl TitleStateMachine {
    /// Initialize title screen matching PS2 `logo.c` + `titleInit` (`0x001c1260`).
    pub fn new() -> Self {
        Self {
            phase: TitlePhase::SplashIntro,
            main_menu_index: 0,
            scenario_index: 0,
            action_difficulty: ActionDifficulty::Normal,
            riddle_difficulty: RiddleDifficulty::Normal,
            selected_scenario: ScenarioKind::LetterFromSilentHeaven,
            selected_test_room: TestRoomKind::ObservationDeck,
            launch_target: LaunchTarget::TestRoom(TestRoomKind::ObservationDeck),
            test_room_index: 0,
            fade_alpha: 0.0,
            fade_speed: 1.0 / TITLE_FADE_DURATION_SECS,
            is_fading_in: true,
            is_fading_out: false,
            boot_sequence: BootSequenceState::new(BootProfile::Ps2Original),
            splash_timer: 0.0,
            elapsed_timer: 0.0,
            attract_timer: TITLE_ATTRACT_TIMEOUT_SECS,
            attract_movie_requested: false,
            pulse_phase_rad: 0.0,
            save_availability: SaveAvailability::None,
            include_pc_menu_slots: true,
            riddle_clear_mask: 0,
            extra_riddle_toggle_bit: false,
            pending_audio_cue: MenuSoundCue::None,
        }
    }

    /// Updates save-data availability (`mcStartCheck2()` -> `titleCreateMainMenu` `0x001c1a90`)
    /// and clamps `main_menu_index` to the valid slot range.
    pub fn set_save_availability(&mut self, availability: SaveAvailability) {
        self.save_availability = availability;
        let slots = self.visible_main_menu_options();
        if self.main_menu_index >= slots.len() {
            self.main_menu_index = availability
                .default_cursor_index()
                .min(slots.len().saturating_sub(1));
        }
    }

    /// Returns the ordered main-menu options currently visible for [`Self::save_availability`].
    #[must_use]
    pub fn visible_main_menu_options(&self) -> Vec<MainMenuOption> {
        self.save_availability
            .visible_slots(self.include_pc_menu_slots)
    }

    /// Returns the currently highlighted [`MainMenuOption`], defaulting to `NewGame`.
    #[must_use]
    pub fn selected_main_menu_option(&self) -> MainMenuOption {
        let slots = self.visible_main_menu_options();
        slots
            .get(self.main_menu_index)
            .copied()
            .unwrap_or(MainMenuOption::NewGame)
    }

    /// Computes the authentic PS2/PC cosine pulse intensity in `[128, 254]` (`_DAT_0116d960` =
    /// `cos(phase) * 63.0f + 191.0f`, `Event/title.c` `0x001c1e40` / `0x001c30d0`).
    #[must_use]
    pub fn selection_pulse_u8(&self) -> u8 {
        (self.pulse_phase_rad.cos() * 63.0 + 191.0).clamp(128.0, 254.0) as u8
    }

    /// Computes normalized selection pulse alpha in `[0.502, 1.0]`.
    #[must_use]
    pub fn selection_pulse_alpha(&self) -> f32 {
        self.selection_pulse_u8() as f32 / 255.0
    }

    /// Advance menu timers, boot logos, attract countdown, pulse/confirm-blink, and fades.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        self.pending_audio_cue = MenuSoundCue::None;
        self.elapsed_timer += dt;

        // Advance selection pulse (idle 4°/frame @ 30 Hz) or confirm blink (90°/frame @ 30 Hz).
        let frames_30hz = dt * 30.0;
        let phase_step = if self.phase == TitlePhase::TransitionToGame && self.is_fading_out {
            TITLE_CONFIRM_BLINK_RAD_PER_30HZ
        } else {
            TITLE_IDLE_PULSE_RAD_PER_30HZ
        };
        self.pulse_phase_rad =
            (self.pulse_phase_rad + phase_step * frames_30hz).rem_euclid(std::f32::consts::TAU);

        // Boot logo sequence (`logo.c` 0x0020c910..0x0020d220 / `sh2pc` 0x00405e80)
        if self.phase == TitlePhase::SplashIntro {
            self.splash_timer += dt;
            let done = self.boot_sequence.tick(dt);
            self.fade_alpha = self.boot_sequence.alpha;
            if done {
                self.phase = TitlePhase::TitleLogo;
                self.start_fade_in();
            }
            return;
        }

        // Standard 2.0s fade-in / fade-out (`titleFadeIn` 0x001c1880, `titleFadeOut` 0x001c30d0)
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
        } else if self.phase != TitlePhase::TransitionToGame {
            self.fade_alpha = 1.0;
        }

        // 60.0s attract-mode timeout (`_DAT_0116d95c` in `titleMainSelect`/`Scenario`/`Battle`/`Riddle`)
        match self.phase {
            TitlePhase::MainMenu
            | TitlePhase::ScenarioSelect
            | TitlePhase::ActionDifficultySelect
            | TitlePhase::RiddleDifficultySelect
                if !self.is_fading_in && !self.is_fading_out =>
            {
                self.attract_timer -= dt;
                if self.attract_timer <= 0.0 {
                    if self.phase == TitlePhase::MainMenu {
                        self.attract_timer = 0.0;
                        self.attract_movie_requested = true;
                        self.start_fade_out();
                    } else {
                        // `titleBacktoMainMenuFromLevelMenu` (0x001c29a0)
                        self.phase = TitlePhase::MainMenu;
                        self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                        self.pending_audio_cue = MenuSoundCue::Cancel;
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle player navigation input (D-pad, sticks, Enter, Esc).
    pub fn handle_input(&mut self, action: MenuNavAction) -> MenuSoundCue {
        let mut sound = MenuSoundCue::None;

        match self.phase {
            TitlePhase::SplashIntro => {
                // Skip straight to TitleLogo on confirm/cancel (preserves responsive boot skip).
                self.phase = TitlePhase::TitleLogo;
                self.fade_alpha = 1.0;
                self.is_fading_out = false;
                self.is_fading_in = false;
                self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                sound = MenuSoundCue::Confirm;
            }
            TitlePhase::TitleLogo => {
                // Any confirm proceeds to Main Menu with save-aware default cursor.
                self.phase = TitlePhase::MainMenu;
                self.main_menu_index = self.save_availability.default_cursor_index();
                self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                sound = MenuSoundCue::Confirm;
            }
            TitlePhase::MainMenu => {
                let slots = self.visible_main_menu_options();
                let count = slots.len().max(1);
                if self.main_menu_index >= count {
                    self.main_menu_index = 0;
                }
                match action {
                    MenuNavAction::Up => {
                        self.main_menu_index = if self.main_menu_index == 0 {
                            count - 1
                        } else {
                            self.main_menu_index - 1
                        };
                        self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                        sound = MenuSoundCue::CursorMove;
                    }
                    MenuNavAction::Down => {
                        self.main_menu_index = (self.main_menu_index + 1) % count;
                        self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                        sound = MenuSoundCue::CursorMove;
                    }
                    MenuNavAction::Confirm => {
                        self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                        match slots[self.main_menu_index] {
                            MainMenuOption::NewGame => {
                                self.phase = TitlePhase::ScenarioSelect;
                                self.scenario_index = 0;
                                sound = MenuSoundCue::Confirm;
                            }
                            MainMenuOption::Continue | MainMenuOption::Load => {
                                self.launch_target = LaunchTarget::Campaign {
                                    scenario: self.selected_scenario,
                                    is_continue: true,
                                };
                                self.phase = TitlePhase::TransitionToGame;
                                self.start_fade_out();
                                sound = MenuSoundCue::GameStartTransition;
                            }
                            MainMenuOption::Options => {
                                self.phase = TitlePhase::OptionsMenu;
                                sound = MenuSoundCue::Confirm;
                            }
                            MainMenuOption::Movies => {
                                self.selected_test_room = TestRoomKind::FmvCutsceneViewer;
                                self.launch_target =
                                    LaunchTarget::TestRoom(TestRoomKind::FmvCutsceneViewer);
                                self.phase = TitlePhase::TransitionToGame;
                                self.start_fade_out();
                                sound = MenuSoundCue::Confirm;
                            }
                            MainMenuOption::Exit => {
                                sound = MenuSoundCue::Cancel;
                            }
                        }
                    }
                    MenuNavAction::Cancel => {
                        self.phase = TitlePhase::TitleLogo;
                        self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                        sound = MenuSoundCue::Cancel;
                    }
                    _ => {}
                }
            }
            TitlePhase::ScenarioSelect => match action {
                MenuNavAction::Up | MenuNavAction::Down => {
                    self.scenario_index ^= 1;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    self.selected_scenario = if self.scenario_index == 0 {
                        ScenarioKind::LetterFromSilentHeaven
                    } else {
                        ScenarioKind::BornFromAWish
                    };
                    self.phase = TitlePhase::ActionDifficultySelect;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::Confirm;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::MainMenu;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::ActionDifficultySelect => match action {
                MenuNavAction::Up => {
                    let idx = ACTION_DIFFICULTY_ROWS
                        .iter()
                        .position(|&d| d == self.action_difficulty)
                        .unwrap_or(1);
                    let next = if idx == 0 {
                        ACTION_DIFFICULTY_ROWS.len() - 1
                    } else {
                        idx - 1
                    };
                    self.action_difficulty = ACTION_DIFFICULTY_ROWS[next];
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Down => {
                    let idx = ACTION_DIFFICULTY_ROWS
                        .iter()
                        .position(|&d| d == self.action_difficulty)
                        .unwrap_or(1);
                    let next = (idx + 1) % ACTION_DIFFICULTY_ROWS.len();
                    self.action_difficulty = ACTION_DIFFICULTY_ROWS[next];
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    // Retail PC `sh2pc.exe` `0x004984c7`: Born From a Wish skips Riddle difficulty.
                    if self.selected_scenario == ScenarioKind::BornFromAWish {
                        self.selected_test_room = TestRoomKind::RestroomStart;
                        self.test_room_index = 1;
                        self.launch_target = LaunchTarget::Campaign {
                            scenario: self.selected_scenario,
                            is_continue: false,
                        };
                        self.phase = TitlePhase::TransitionToGame;
                        self.start_fade_out();
                        sound = MenuSoundCue::GameStartTransition;
                    } else {
                        self.phase = TitlePhase::RiddleDifficultySelect;
                        sound = MenuSoundCue::Confirm;
                    }
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::ScenarioSelect;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::RiddleDifficultySelect => match action {
                MenuNavAction::Up => {
                    let idx = RIDDLE_DIFFICULTY_ROWS
                        .iter()
                        .position(|&d| d == self.riddle_difficulty)
                        .unwrap_or(1);
                    let next = if idx == 0 {
                        RIDDLE_DIFFICULTY_ROWS.len() - 1
                    } else {
                        idx - 1
                    };
                    self.riddle_difficulty = RIDDLE_DIFFICULTY_ROWS[next];
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Down => {
                    let idx = RIDDLE_DIFFICULTY_ROWS
                        .iter()
                        .position(|&d| d == self.riddle_difficulty)
                        .unwrap_or(1);
                    let next = (idx + 1) % RIDDLE_DIFFICULTY_ROWS.len();
                    self.riddle_difficulty = RIDDLE_DIFFICULTY_ROWS[next];
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    // Apply silent Extra Riddle upgrade (`sh2pc.exe` `0x0048aa20`) if unlocked.
                    self.riddle_difficulty = resolve_effective_riddle(
                        self.selected_scenario,
                        self.riddle_difficulty,
                        self.riddle_clear_mask,
                        self.extra_riddle_toggle_bit,
                    );
                    self.selected_test_room = TestRoomKind::RestroomStart;
                    self.test_room_index = 1;
                    self.launch_target = LaunchTarget::Campaign {
                        scenario: self.selected_scenario,
                        is_continue: false,
                    };
                    self.phase = TitlePhase::TransitionToGame;
                    self.start_fade_out();
                    sound = MenuSoundCue::GameStartTransition;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::ActionDifficultySelect;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::Credits => {
                if matches!(action, MenuNavAction::Confirm | MenuNavAction::Cancel) {
                    self.phase = TitlePhase::MainMenu;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::Cancel;
                }
            }
            TitlePhase::TestRoomsSelect => match action {
                MenuNavAction::Up => {
                    if self.test_room_index == 0 {
                        self.test_room_index = TestRoomKind::ALL.len() - 1;
                    } else {
                        self.test_room_index -= 1;
                    }
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Down => {
                    self.test_room_index = (self.test_room_index + 1) % TestRoomKind::ALL.len();
                    sound = MenuSoundCue::CursorMove;
                }
                MenuNavAction::Confirm => {
                    self.selected_test_room = TestRoomKind::ALL[self.test_room_index];
                    self.launch_target = LaunchTarget::TestRoom(self.selected_test_room);
                    self.phase = TitlePhase::TransitionToGame;
                    self.start_fade_out();
                    sound = MenuSoundCue::GameStartTransition;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::MainMenu;
                    self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
            TitlePhase::OptionsMenu | TitlePhase::TransitionToGame => {}
        }

        self.pending_audio_cue = sound;
        sound
    }

    /// Trigger smooth 2.0s fade out (`titleFadeOut` `0x001c30d0`).
    pub fn start_fade_out(&mut self) {
        self.is_fading_out = true;
        self.is_fading_in = false;
        self.fade_speed = 1.0 / TITLE_FADE_DURATION_SECS;
    }

    /// Trigger smooth 2.0s fade in (`titleFadeIn` `0x001c1880`).
    pub fn start_fade_in(&mut self) {
        self.is_fading_in = true;
        self.is_fading_out = false;
        self.fade_alpha = 0.0;
        self.fade_speed = 1.0 / TITLE_FADE_DURATION_SECS;
    }

    /// Transitions immediately to `phase` at full opacity (`fade_alpha = 1.0`), clearing any active fade flags.
    pub fn show_phase_immediately(&mut self, phase: TitlePhase) {
        self.phase = phase;
        self.fade_alpha = 1.0;
        self.is_fading_in = false;
        self.is_fading_out = false;
        self.attract_timer = TITLE_ATTRACT_TIMEOUT_SECS;
        self.attract_movie_requested = false;
    }

    fn on_fade_out_complete(&mut self) {
        match self.phase {
            TitlePhase::SplashIntro => {
                self.phase = TitlePhase::TitleLogo;
                self.start_fade_in();
            }
            TitlePhase::TransitionToGame => {
                self.fade_alpha = 0.0;
            }
            _ => {
                self.fade_alpha = 1.0;
            }
        }
    }
}
