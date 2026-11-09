//! Title screen and main menu state machine.
//! Ground truth references:
//! - PS2 July 13, 2001 Prototype: `title.c` (`titleInit`, `titleFadeIn`, `titleMainSelect`,
//!   `titleBattleSelect`, `titleRiddleSelect`, `titleFadeOutNewGame`, `titleFadeOut`, `titleExit`).
//! - Sound triggers: `0x2710` (cursor move), `0x2712` (confirm), `0x2713` (cancel), `15002` (game start transition).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::button_atlas::{AuthenticButtonSprite, ButtonSpriteSheet};
use crate::font::TextRasterizer;
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
    /// Test room / debug stage selection for volumetric fog and lighting calibration.
    TestRoomsSelect,
    /// Credits display screen.
    Credits,
    /// Fade-out transition into the game world / opening FMV (`rr21.map`).
    TransitionToGame,
}

/// Primary options on the main title menu (authentic 4-item retail list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainMenuOption {
    #[default]
    NewGame = 0,
    Continue = 1,
    Options = 2,
    Exit = 3,
}

impl MainMenuOption {
    pub const ALL: [MainMenuOption; 4] = [
        MainMenuOption::NewGame,
        MainMenuOption::Continue,
        MainMenuOption::Options,
        MainMenuOption::Exit,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::NewGame => "NEW GAME",
            Self::Continue => "CONTINUE",
            Self::Options => "OPTIONS",
            Self::Exit => "EXIT",
        }
    }
}

/// Identifiers for test rooms / sandbox stages selectable from the debug menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TestRoomKind {
    /// Authentic outdoor Toluca Lake Observation Deck (`ob01.map` + `ob01.cld`, R_OBSV_DECK = 0x02).
    #[default]
    ObservationDeck,
    /// Authentic opening story start restroom overlook (`er01.map` + `er01.cld`, R_BEGIN_BATHROOM = 0x01).
    RestroomStart,
    /// Authentic Lakeview Hotel Basement Boiler Room (`rr21.map` + `rr21.cld`, R_HTL_BOILER_RM = 0xA7).
    HotelBoilerRoom,
    /// Synthetic extended fog test runway with calibrated distance crossbars (5m, 10m, 25m, 50m, 75m, 100m).
    SyntheticFogRunway,
    /// Built-in enclosed room fallback sandbox with interior lighting, collision columns, and mesh skinning.
    FallbackSandbox,
    /// Interactive 3D Model, Animation, and Texture Viewer / Bestiary inspection stage (`SH2-36`).
    ModelAndTextureViewer,
    /// Interactive 1080p60 FMV Cutscene Player / Theater (`SH2-10`).
    FmvCutsceneViewer,
    /// Interactive Animated Footstep Loading Screen Test Chamber (`SH2-44`).
    LoadingScreenTest,
    /// Interactive Combat & Weapon Mechanics Arena (`SH2-13`).
    CombatArena,
    /// Interactive Creature Grapple Animation & Elevation Debug Room (`SH2-57`).
    GrappleAnimationRoom,
    /// Interactive BGM Streaming, Multi-Channel AIX Spatial Mixing & SFX Sound Studio (`SH2-60`).
    AudioDebugRoom,
    /// Interactive 8-Material Acoustic Surface Footstep & 3D SFX Attenuation Test Chamber (`SH2-61`).
    SurfaceSfxTestRoom,
    /// Interactive 85-Item Catalog, 3D `.mdl` / 2D `.tex` Inspect, Combinations & Healing Debug Room (`SH2-15`).
    InventoryAndItemsRoom,
    /// Interactive Puzzle State Machines, Stage Event (`ev_prog`) & Cutaway Sprite Overlay Debug Room (`SH2-15`).
    PuzzleAndCutawayRoom,
    /// Interactive Scripted Sequence System, `ev_pos` Trigger Zones & Environmental Horror Triggers Chamber (`SH2-47`).
    HorrorTriggersRoom,
}

impl TestRoomKind {
    pub const ALL: [TestRoomKind; 15] = [
        TestRoomKind::ObservationDeck,
        TestRoomKind::RestroomStart,
        TestRoomKind::HotelBoilerRoom,
        TestRoomKind::SyntheticFogRunway,
        TestRoomKind::FallbackSandbox,
        TestRoomKind::ModelAndTextureViewer,
        TestRoomKind::FmvCutsceneViewer,
        TestRoomKind::LoadingScreenTest,
        TestRoomKind::CombatArena,
        TestRoomKind::GrappleAnimationRoom,
        TestRoomKind::AudioDebugRoom,
        TestRoomKind::SurfaceSfxTestRoom,
        TestRoomKind::InventoryAndItemsRoom,
        TestRoomKind::PuzzleAndCutawayRoom,
        TestRoomKind::HorrorTriggersRoom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::ObservationDeck => "Observation Deck (ob01.map)",
            Self::RestroomStart => "Restroom Start (er01.map)",
            Self::HotelBoilerRoom => "Hotel Boiler Room (rr21.map)",
            Self::SyntheticFogRunway => "Synthetic Fog Runway",
            Self::FallbackSandbox => "Fallback Sandbox Room",
            Self::ModelAndTextureViewer => "Model & Texture Viewer",
            Self::FmvCutsceneViewer => "FMV Cutscene Player",
            Self::LoadingScreenTest => "Loading Screen Preview",
            Self::CombatArena => "Combat & Weapon Testing Arena",
            Self::GrappleAnimationRoom => "Grapple Animation Room",
            Self::AudioDebugRoom => "Audio & AIX Mixing Debug Room",
            Self::SurfaceSfxTestRoom => "Surface Footstep & 3D SFX Room",
            Self::InventoryAndItemsRoom => "Inventory & 3D/2D Item Inspect Room",
            Self::PuzzleAndCutawayRoom => "Puzzle Logic & Cutaway Overlays Room",
            Self::HorrorTriggersRoom => "Stage Events & Horror Triggers Room",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::ObservationDeck => "Toluca Lake Observation Deck with outdoor volumetric fog",
            Self::RestroomStart => {
                "Prologue restroom overlook with mirror reflections & ambient audio"
            }
            Self::HotelBoilerRoom => "Lakeview Hotel basement boiler room with complex geometry",
            Self::SyntheticFogRunway => {
                "Calibrated distance crossbars (5m, 10m, 25m, 50m, 75m, 100m)"
            }
            Self::FallbackSandbox => "Internal test chamber with lighting columns & mesh skinning",
            Self::ModelAndTextureViewer => "Inspect 3D character models, animations, and textures",
            Self::FmvCutsceneViewer => "Launch the 1080p60 cutscene theater player",
            Self::LoadingScreenTest => {
                "Test authentic animated PS2 footstep loading screen & transition"
            }
            Self::CombatArena => {
                "Real-time melee & firearm combat arena with Lying Figure & Mannequin targets"
            }
            Self::GrappleAnimationRoom => {
                "Pairwise creature grapple animation viewer with free orbit/flight camera"
            }
            Self::AudioDebugRoom => {
                "Test SH2:EE 48kHz vs PS2 BGM, 7-layer AIX room areas, situations & SFX"
            }
            Self::SurfaceSfxTestRoom => {
                "Walk across all 8 collision floor materials (Concrete/Wood/Metal/Carpet/Water/Grass/Gravel/Blood) & 3D SFX"
            }
            Self::InventoryAndItemsRoom => {
                "Browse all 85 items (3D .mdl + 2D .tex), test 999 cap, Bullet Adjust, healing & 11 combinations"
            }
            Self::PuzzleAndCutawayRoom => {
                "Interactive Clock, Safe, Coin Desk, Hospital Box, Hotel Drum & Baldwin Board cutaway overlays"
            }
            Self::HorrorTriggersRoom => {
                "Prison bathroom stall jumpscare (0x427D), Room 209 orbiting whisper (0x9C70), 3F footsteps & ev_pos zones"
            }
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
    /// Currently selected test room / sandbox stage.
    pub selected_test_room: TestRoomKind,
    pub test_room_index: usize,
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
            selected_test_room: TestRoomKind::ObservationDeck,
            test_room_index: 0,
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
        } else if !matches!(
            self.phase,
            TitlePhase::SplashIntro | TitlePhase::TransitionToGame
        ) {
            self.fade_alpha = 1.0;
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
                self.phase = TitlePhase::TitleLogo;
                self.fade_alpha = 1.0;
                self.is_fading_out = false;
                self.is_fading_in = false;
                sound = MenuSoundCue::Confirm;
            }
            TitlePhase::TitleLogo => {
                // Any button pressed on Title Screen proceeds to Main Menu
                self.phase = TitlePhase::MainMenu;
                sound = MenuSoundCue::Confirm;
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
                        self.start_fade_out();
                        sound = MenuSoundCue::GameStartTransition;
                    }
                    MainMenuOption::Options => {
                        self.phase = TitlePhase::OptionsMenu;
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
                    self.phase = TitlePhase::TransitionToGame;
                    self.start_fade_out();
                    sound = MenuSoundCue::GameStartTransition;
                }
                MenuNavAction::Cancel => {
                    self.phase = TitlePhase::MainMenu;
                    sound = MenuSoundCue::Cancel;
                }
                _ => {}
            },
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

    /// Transitions immediately to `phase` at full opacity (`fade_alpha = 1.0`), clearing any active fade flags.
    pub fn show_phase_immediately(&mut self, phase: TitlePhase) {
        self.phase = phase;
        self.fade_alpha = 1.0;
        self.is_fading_in = false;
        self.is_fading_out = false;
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
            _ => {
                self.fade_alpha = 1.0;
            }
        }
    }
}

// -----------------------------------------------------------------------------------------
// Authentic Proportional Font Rasterization Helpers (`AtlasFont`)
// -----------------------------------------------------------------------------------------

/// Draws the authentic title screen text elements onto `rast`.
/// Replaces legacy fixed 8x8 font rendering with authentic proportional glyphs,
/// two-tone gradient shading, and drop shadows matching PS2 prototype `title.c` (`0x002BAF48`).
pub fn draw_title_screen(
    sm: &TitleStateMachine,
    button_sprites: Option<&ButtonSpriteSheet>,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let center_x = rast.width / 2;

    match sm.phase {
        TitlePhase::TitleLogo => {
            if let Some(atlas) = button_sprites {
                let rect = AuthenticButtonSprite::StartPrompt.atlas_rect();
                let scale = 0.44f32;
                let w = (rect.width as f32 * scale) as i32;
                let x = center_x as i32 - (w / 2);
                let y = 610;

                let pulse = (sm.splash_timer * 3.5).sin() * 0.15 + 0.85;
                atlas.blit_sprite(
                    AuthenticButtonSprite::StartPrompt,
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    x,
                    y,
                    scale,
                    [1.0, 1.0, 1.0, pulse],
                );
            } else {
                font.draw_line_centered(
                    rast,
                    "PRESS ANY KEY OR ENTER TO START",
                    (center_x, 618),
                    22,
                    Sh2TextColor::GoldSelected,
                    true,
                );
            }
        }
        TitlePhase::MainMenu => {
            if let Some(atlas) = button_sprites {
                let center_xi = center_x as i32;
                // Calibrated PS2 prototype dimensions (SLUS_202.28 0x001c38c0 / titleDrawMainMenu)
                let start_y = 440;
                let step_y = 38;
                let btn_scale = 0.44f32;
                let glow_scale = 0.58f32;

                for (idx, opt) in MainMenuOption::ALL.iter().enumerate() {
                    let is_selected = idx == sm.main_menu_index;
                    let y = start_y + idx as i32 * step_y;

                    let sprite = match opt {
                        MainMenuOption::NewGame => AuthenticButtonSprite::NewGame,
                        MainMenuOption::Continue => AuthenticButtonSprite::Continue,
                        MainMenuOption::Options => AuthenticButtonSprite::Options,
                        MainMenuOption::Exit => AuthenticButtonSprite::Exit,
                    };

                    let rect = sprite.atlas_rect();
                    let scaled_w = (rect.width as f32 * btn_scale) as i32;
                    let btn_x = center_xi - (scaled_w / 2);

                    if is_selected {
                        // Render authentic 1:1 PS2 smooth procedural GlowCursor behind the active item
                        let glow_rect = AuthenticButtonSprite::GlowCursor.atlas_rect();
                        let glow_w = (glow_rect.width as f32 * glow_scale) as i32;
                        let glow_h = (glow_rect.height as f32 * glow_scale) as i32;
                        let btn_h = (rect.height as f32 * btn_scale) as i32;
                        let glow_center_y = y + (btn_h / 2);

                        ButtonSpriteSheet::draw_analytic_glow_cursor(
                            &mut rast.pixels,
                            rast.width,
                            rast.height,
                            center_xi,
                            glow_center_y,
                            glow_w,
                            glow_h,
                            [1.0, 1.0, 1.0, 0.95],
                        );
                    }

                    let tint = if is_selected {
                        // Bright white with full opacity
                        [1.0f32, 1.0, 1.0, 1.0]
                    } else {
                        // Authentic unselected silver/grey
                        [0.72f32, 0.72, 0.75, 0.75]
                    };

                    atlas.blit_sprite(
                        sprite,
                        &mut rast.pixels,
                        rast.width,
                        rast.height,
                        btn_x,
                        y,
                        btn_scale,
                        tint,
                    );
                }
            } else {
                let row_h = 36;
                let start_y = 350;
                for (idx, opt) in MainMenuOption::ALL.iter().enumerate() {
                    let is_selected = idx == sm.main_menu_index;
                    let y = start_y + idx * row_h;
                    let text = opt.label();
                    let style = if is_selected {
                        Sh2TextColor::GoldSelected
                    } else {
                        Sh2TextColor::SilverUnselected
                    };

                    let full_label = if is_selected {
                        format!("> {}", text)
                    } else {
                        format!("  {}", text)
                    };

                    font.draw_line_centered(rast, &full_label, (center_x, y), 26, style, true);
                }
            }
        }
        TitlePhase::Credits => {
            font.draw_line_centered(
                rast,
                "SILENT HILL 2 CREDITS",
                (center_x, 240),
                30,
                Sh2TextColor::AmberHeader,
                true,
            );
            font.draw_line_centered(
                rast,
                "DIRECTOR: KEIICHIRO TOYAMA",
                (center_x, 330),
                22,
                Sh2TextColor::White,
                true,
            );
            font.draw_line_centered(
                rast,
                "CGI & CHARACTER DESIGN: TAKAYOSHI SATO",
                (center_x, 380),
                22,
                Sh2TextColor::White,
                true,
            );
            font.draw_line_centered(
                rast,
                "MUSIC & SOUND DESIGN: AKIRA YAMAOKA",
                (center_x, 430),
                22,
                Sh2TextColor::White,
                true,
            );
            font.draw_line_centered(
                rast,
                "[Press Esc / Cancel to Return]",
                (center_x, 620),
                18,
                Sh2TextColor::MutedFooter,
                true,
            );
        }
        _ => {}
    }
}

/// Draws the authentic scenario selection screen onto `rast`.
pub fn draw_scenario_selection(
    sm: &TitleStateMachine,
    button_sprites: Option<&ButtonSpriteSheet>,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let center_x = rast.width / 2;

    if let Some(atlas) = button_sprites {
        let center_xi = center_x as i32;

        // Position NEW GAME context header at y = 410 (clear below title card drips)
        let hdr_rect = AuthenticButtonSprite::NewGame.atlas_rect();
        let hdr_scale = 0.44f32;
        let hdr_w = (hdr_rect.width as f32 * hdr_scale) as i32;
        let hdr_x = center_xi - (hdr_w / 2);
        let hdr_y = 410;

        atlas.blit_sprite(
            AuthenticButtonSprite::NewGame,
            &mut rast.pixels,
            rast.width,
            rast.height,
            hdr_x,
            hdr_y,
            hdr_scale,
            [0.72, 0.72, 0.75, 0.75],
        );

        let scenarios = [
            (0, AuthenticButtonSprite::MainScenario),
            (1, AuthenticButtonSprite::SubScenario),
        ];

        let scale = 0.44f32;
        let glow_scale = 0.62f32;
        let start_y = 480;
        let step_y = 55;

        for (idx, sprite) in &scenarios {
            let is_selected = *idx == sm.scenario_index;
            let y = start_y + *idx as i32 * step_y;
            let rect = sprite.atlas_rect();
            let w = (rect.width as f32 * scale) as i32;
            let x = center_xi - (w / 2);

            if is_selected {
                // Render authentic 1:1 PS2 smooth procedural GlowCursor behind the active item
                let glow_rect = AuthenticButtonSprite::GlowCursor.atlas_rect();
                // Proportional horizontal expansion for long scenario titles
                let glow_w = (glow_rect.width as f32 * glow_scale * 1.08) as i32;
                let glow_h = (glow_rect.height as f32 * glow_scale) as i32;
                let btn_h = (rect.height as f32 * scale) as i32;
                let glow_center_y = y + (btn_h / 2);

                ButtonSpriteSheet::draw_analytic_glow_cursor(
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    center_xi,
                    glow_center_y,
                    glow_w,
                    glow_h,
                    [1.0, 1.0, 1.0, 0.95],
                );
            }

            let tint = if is_selected {
                [1.0f32, 1.0, 1.0, 1.0]
            } else {
                [0.72f32, 0.72, 0.75, 0.75]
            };

            atlas.blit_sprite(
                *sprite,
                &mut rast.pixels,
                rast.width,
                rast.height,
                x,
                y,
                scale,
                tint,
            );
        }
    } else {
        font.draw_line_centered(
            rast,
            "SELECT SCENARIO",
            (center_x, 320),
            28,
            Sh2TextColor::AmberHeader,
            true,
        );
        let scenarios = [
            "LETTER FROM SILENT HEAVEN (JAMES)",
            "BORN FROM A WISH (MARIA)",
        ];
        for (idx, name) in scenarios.iter().enumerate() {
            let is_selected = idx == sm.scenario_index;
            let y = 410 + idx * 46;
            let style = if is_selected {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::SilverUnselected
            };
            let line = if is_selected {
                format!("> {}", name)
            } else {
                format!("  {}", name)
            };
            font.draw_line_centered(rast, &line, (center_x, y), 22, style, true);
        }
    }

    font.draw_line_centered(
        rast,
        "[Up/Down: Select | Enter: Confirm | Esc: Back]",
        (center_x, 658),
        16,
        Sh2TextColor::MutedFooter,
        true,
    );
}

/// Draws action / riddle difficulty selection onto `rast`.
pub fn draw_difficulty_selection(
    sm: &TitleStateMachine,
    button_sprites: Option<&ButtonSpriteSheet>,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let center_x = rast.width / 2;
    let is_action = sm.phase == TitlePhase::ActionDifficultySelect;

    if let Some(atlas) = button_sprites {
        let center_xi = center_x as i32;
        let hdr_sprite = if is_action {
            AuthenticButtonSprite::ActionLevel
        } else {
            AuthenticButtonSprite::RiddleLevel
        };
        let hdr_rect = hdr_sprite.atlas_rect();
        let hdr_scale = 0.55f32;
        let hdr_w = (hdr_rect.width as f32 * hdr_scale) as i32;
        atlas.blit_sprite(
            hdr_sprite,
            &mut rast.pixels,
            rast.width,
            rast.height,
            center_xi - (hdr_w / 2),
            260,
            hdr_scale,
            [1.0, 1.0, 1.0, 1.0],
        );

        if is_action {
            let choices = [
                (ActionDifficulty::Hard, AuthenticButtonSprite::Hard),
                (ActionDifficulty::Normal, AuthenticButtonSprite::Normal),
                (ActionDifficulty::Easy, AuthenticButtonSprite::Easy),
            ];
            for (idx, (diff, sprite)) in choices.iter().enumerate() {
                let is_selected = *diff == sm.action_difficulty;
                let y = 360 + idx as i32 * 65;
                let rect = sprite.atlas_rect();
                let scale = 0.52f32;
                let w = (rect.width as f32 * scale) as i32;
                let x = center_xi - (w / 2);

                if is_selected {
                    let glow_rect = AuthenticButtonSprite::GlowCursor.atlas_rect();
                    let glow_scale = 0.46f32;
                    let glow_w = (glow_rect.width as f32 * glow_scale) as i32;
                    let glow_h = (glow_rect.height as f32 * glow_scale) as i32;
                    let btn_h = (rect.height as f32 * scale) as i32;
                    let glow_center_y = y + (btn_h / 2);
                    ButtonSpriteSheet::draw_analytic_glow_cursor(
                        &mut rast.pixels,
                        rast.width,
                        rast.height,
                        center_xi,
                        glow_center_y,
                        glow_w,
                        glow_h,
                        [1.0, 1.0, 1.0, 0.90],
                    );
                }

                let tint = if is_selected {
                    [1.0f32, 0.95, 0.75, 1.0]
                } else {
                    [0.70f32, 0.70, 0.72, 0.75]
                };

                atlas.blit_sprite(
                    *sprite,
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    x,
                    y,
                    scale,
                    tint,
                );
            }
        } else {
            let choices = [
                (RiddleDifficulty::Hard, AuthenticButtonSprite::Hard),
                (RiddleDifficulty::Normal, AuthenticButtonSprite::Normal),
                (RiddleDifficulty::Easy, AuthenticButtonSprite::Easy),
                (RiddleDifficulty::Extra, AuthenticButtonSprite::Beginner),
            ];
            for (idx, (diff, sprite)) in choices.iter().enumerate() {
                let is_selected = *diff == sm.riddle_difficulty;
                let y = 350 + idx as i32 * 58;
                let rect = sprite.atlas_rect();
                let scale = 0.52f32;
                let w = (rect.width as f32 * scale) as i32;
                let x = center_xi - (w / 2);

                if is_selected {
                    let glow_rect = AuthenticButtonSprite::GlowCursor.atlas_rect();
                    let glow_scale = 0.46f32;
                    let glow_w = (glow_rect.width as f32 * glow_scale) as i32;
                    let glow_h = (glow_rect.height as f32 * glow_scale) as i32;
                    let btn_h = (rect.height as f32 * scale) as i32;
                    let glow_center_y = y + (btn_h / 2);
                    ButtonSpriteSheet::draw_analytic_glow_cursor(
                        &mut rast.pixels,
                        rast.width,
                        rast.height,
                        center_xi,
                        glow_center_y,
                        glow_w,
                        glow_h,
                        [1.0, 1.0, 1.0, 0.90],
                    );
                }

                let tint = if is_selected {
                    [1.0f32, 0.95, 0.75, 1.0]
                } else {
                    [0.70f32, 0.70, 0.72, 0.75]
                };

                atlas.blit_sprite(
                    *sprite,
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    x,
                    y,
                    scale,
                    tint,
                );
            }
        }
    } else {
        let title_text = if is_action {
            "ACTION DIFFICULTY"
        } else {
            "RIDDLE DIFFICULTY"
        };
        font.draw_line_centered(
            rast,
            title_text,
            (center_x, 320),
            28,
            Sh2TextColor::AmberHeader,
            true,
        );
        let diff_str = if is_action {
            format!("{:?}", sm.action_difficulty).to_uppercase()
        } else {
            format!("{:?}", sm.riddle_difficulty).to_uppercase()
        };
        font.draw_line_centered(
            rast,
            &format!("> {} <", diff_str),
            (center_x, 410),
            26,
            Sh2TextColor::GoldSelected,
            true,
        );
    }

    let footer_text = if is_action {
        "[Up/Down: Change | Enter: Next | Esc: Back]"
    } else {
        "[Up/Down: Change | Enter: Start Game | Esc: Back]"
    };
    font.draw_line_centered(
        rast,
        footer_text,
        (center_x, 658),
        16,
        Sh2TextColor::MutedFooter,
        true,
    );
}

/// Draws the test room selection stage list onto `rast` styled identically to the authentic PS2 Options Menu.
pub fn draw_test_rooms_screen(sm: &TitleStateMachine, font: &AtlasFont, rast: &mut TextRasterizer) {
    // Keep 4:3 authentic aspect ratio inside widescreen rasterizer
    let target_4_3_w = rast.height as f32 * (4.0 / 3.0);
    let (x_offset, base_w) = if target_4_3_w <= rast.width as f32 {
        ((rast.width as f32 - target_4_3_w) * 0.5, target_4_3_w)
    } else {
        (0.0, rast.width as f32)
    };

    let scale_x = base_w / 640.0;
    let scale_y = rast.height as f32 / 480.0;

    // Authentic PS2 octagon containers calibrated from retail PS2 SLUS-20228 / optmenu.c
    // Upper options panel: x in [36, 604], y in [64, 407] with 45-degree chamfers
    let panel_pts: Vec<(i32, i32)> = [
        (48.0, 64.0),
        (593.0, 64.0),
        (604.0, 76.0),
        (604.0, 407.0),
        (36.0, 407.0),
        (36.0, 76.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    // Lower contextual help box: x in [36, 604], y in [414, 461] with bottom chamfers
    let box_pts: Vec<(i32, i32)> = [
        (36.0, 414.0),
        (604.0, 414.0),
        (604.0, 436.0),
        (580.0, 461.0),
        (60.0, 461.0),
        (36.0, 436.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    // Outer double-border outline enclosing both panels: x in [30, 612], y in [58, 467]
    let outline_pts: Vec<(i32, i32)> = [
        (55.0, 58.0),
        (586.0, 58.0),
        (612.0, 83.0),
        (612.0, 440.0),
        (586.0, 467.0),
        (55.0, 467.0),
        (30.0, 440.0),
        (30.0, 83.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    // Calibrated PS2 colors (linear values for sRGB surface target)
    let fill_color = [12, 12, 13, 255];
    let border_color = [4, 4, 18, 255];

    crate::options::draw_polygon(rast, &panel_pts, Some(fill_color), Some(border_color));
    crate::options::draw_polygon(rast, &box_pts, Some(fill_color), Some(border_color));
    crate::options::draw_polygon(rast, &outline_pts, None, Some(border_color));

    let title_x = rast.width / 2;
    let title_y = (28.0 * scale_y).round() as usize;

    font.draw_line_centered(
        rast,
        "<   T E S T   R O O M S   >",
        (title_x, title_y),
        (22.0 * scale_y).round() as usize,
        Sh2TextColor::White,
        true,
    );

    let count = TestRoomKind::ALL.len();
    let start_y = 80.0;
    let end_y = 384.0;
    let step_y = if count > 1 {
        ((end_y - start_y) / (count - 1) as f32).min(38.0) * scale_y
    } else {
        38.0 * scale_y
    };
    let base_y = start_y * scale_y;
    let center_panel_x = (x_offset + 320.0 * scale_x).round() as usize;
    let font_size = if count > 12 {
        (14.0 * scale_y).round() as usize
    } else if count > 8 {
        (16.0 * scale_y).round() as usize
    } else {
        (18.0 * scale_y).round() as usize
    };

    for (idx, room) in TestRoomKind::ALL.iter().enumerate() {
        let y = (base_y + idx as f32 * step_y).round() as usize;
        let is_selected = idx == sm.test_room_index;

        let style = if is_selected {
            Sh2TextColor::White
        } else {
            Sh2TextColor::SilverUnselected
        };

        let label = room.label();

        if is_selected {
            // Authentic PS2 double-draw pop highlight at (-2, -2)
            font.draw_line_centered(
                rast,
                label,
                (center_panel_x.saturating_sub(2), y.saturating_sub(2)),
                font_size,
                Sh2TextColor::White,
                false,
            );
        }
        font.draw_line_centered(rast, label, (center_panel_x, y), font_size, style, true);
    }

    // Help description inside the lower box at x=90, y=438 in 640x480 space
    let help_x = (x_offset + 90.0 * scale_x).round() as usize;
    let help_y = (436.0 * scale_y).round() as usize;

    let selected_room = TestRoomKind::ALL
        .get(sm.test_room_index)
        .copied()
        .unwrap_or_default();
    let desc = selected_room.description();

    font.draw_line_styled(
        rast,
        desc,
        (help_x, help_y),
        font_size,
        Sh2TextColor::White,
        true,
    );
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

    #[test]
    fn test_test_rooms_selection_flow() {
        let mut sm = TitleStateMachine::new();
        sm.phase = TitlePhase::TestRoomsSelect;
        sm.test_room_index = 0;
        assert_eq!(
            TestRoomKind::ALL[sm.test_room_index],
            TestRoomKind::ObservationDeck
        );

        // Cycle to Hotel Boiler Room (index 2) and Synthetic Fog Runway (index 3)
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.test_room_index, 1);
        assert_eq!(
            TestRoomKind::ALL[sm.test_room_index],
            TestRoomKind::RestroomStart
        );
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.test_room_index, 2);
        assert_eq!(
            TestRoomKind::ALL[sm.test_room_index],
            TestRoomKind::HotelBoilerRoom
        );
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.test_room_index, 3);
        assert_eq!(
            TestRoomKind::ALL[sm.test_room_index],
            TestRoomKind::SyntheticFogRunway
        );

        // Confirm selection -> TransitionToGame
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::GameStartTransition);
        assert_eq!(sm.phase, TitlePhase::TransitionToGame);
        assert_eq!(sm.selected_test_room, TestRoomKind::SyntheticFogRunway);
    }

    #[test]
    fn test_menu_rendering_with_atlas_font() {
        let sm = TitleStateMachine::new();
        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);

        // Test Main Menu rasterization
        let mut main_sm = sm.clone();
        main_sm.phase = TitlePhase::MainMenu;
        draw_title_screen(&main_sm, None, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));

        // Test Main Menu rasterization with button sprite atlas if available
        let tex_path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/pic/etc/start01e.tex";
        if let Ok(mut f) = std::fs::File::open(tex_path) {
            use std::io::{Read, Seek, SeekFrom};
            let mut header = [0u8; 16];
            if f.read_exact(&mut header).is_ok() && f.seek(SeekFrom::Start(16)).is_ok() {
                let mut data = vec![0u8; 2048 * 2048 * 4];
                if f.read_exact(&mut data).is_ok() {
                    let sheet = crate::button_atlas::ButtonSpriteSheet::new(2048, 2048, data);
                    rast.clear();
                    draw_title_screen(&main_sm, Some(&sheet), &font, &mut rast);
                    let _ = image::save_buffer(
                        "/tmp/rendered_main_menu_test.png",
                        &rast.pixels,
                        640,
                        480,
                        image::ExtendedColorType::Rgba8,
                    );
                }
            }
        }

        // Test Test Rooms rasterization
        rast.clear();
        let mut tr_sm = sm.clone();
        tr_sm.phase = TitlePhase::TestRoomsSelect;
        draw_test_rooms_screen(&tr_sm, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));

        // Test Scenario Select without button atlas
        rast.clear();
        let mut scen_sm = sm.clone();
        scen_sm.phase = TitlePhase::ScenarioSelect;
        draw_scenario_selection(&scen_sm, None, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));

        // Test Difficulty Select without button atlas
        rast.clear();
        let mut diff_sm = sm.clone();
        diff_sm.phase = TitlePhase::ActionDifficultySelect;
        draw_difficulty_selection(&diff_sm, None, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));
    }
}
