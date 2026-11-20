//! Shared title/menu enums, save-availability rules (`titleCreateMainMenu` `0x001c1a90` /
//! `sh2pc.exe` `0x00495520`), and Extra Riddle unlock logic (`sh2pc.exe` `0x0048aa20`).

use serde::{Deserialize, Serialize};
use sh2_core::config::RiddleDifficulty;
use sh2_core::scenario::ScenarioKind;

/// Authentic title sequence phases (`Event/title.c` `0x0033bea0` & `sh2pc.exe` `0x0088c350`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TitlePhase {
    /// Initial CESA / SCE / Konami / Splash boot sequence (`logo.c` `0x0020c910..0x0020d220`).
    #[default]
    SplashIntro,
    /// Silent Hill 2 main title logo screen (`start00.tex` + `start01.tex`).
    TitleLogo,
    /// Main interactive selection menu (`titleMainSelect` `0x001c1e40`).
    MainMenu,
    /// Scenario selection (`titleScenarioSelect` `sh2pc.exe` `0x00498080`: Main Scenario vs Born From a Wish).
    ScenarioSelect,
    /// Action combat difficulty selection (`titleBattleSelect` `0x001c2730` / `0x00498270`).
    ActionDifficultySelect,
    /// Riddle puzzle difficulty selection (`titleRiddleSelect` `0x001c2b60` / `0x00498660`).
    RiddleDifficultySelect,
    /// In-game options configuration menu (`Item/otn_option.c` `0x002108c0`).
    OptionsMenu,
    /// Test room / debug stage selection for volumetric fog and lighting calibration.
    TestRoomsSelect,
    /// Credits display screen.
    Credits,
    /// Fade-out transition into the game world / opening FMV (`titleFadeOutNewGame` `0x001c2ec0`).
    TransitionToGame,
}

/// Primary options on the main title menu (`titleCreateMainMenu` `0x001c1a90` / `sh2pc.exe` `0x00495520`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainMenuOption {
    /// Sprite ID `1` (`LOAD`) — shown when save slots exist (`SaveAvailability::LoadAndContinue | LoadOnly`).
    Load = 0,
    /// Sprite ID `2` (`CONTINUE`) — shown when a quick/recent save exists.
    Continue = 1,
    /// Sprite ID `3` (`NEW GAME`) — always visible.
    #[default]
    NewGame = 2,
    /// Sprite ID `4` (`OPTIONS`) — always visible.
    Options = 3,
    /// Sprite ID `22` (`0x16`, `MOVIES`) — Retail PC slot (`sh2pc.exe` `0x00495520`).
    Movies = 4,
    /// Sprite ID `21` (`0x15`, `EXIT`) — Retail PC slot (`sh2pc.exe` `0x00495520`).
    Exit = 5,
}

impl MainMenuOption {
    /// Legacy 4-item default list preserved for existing callers (`NewGame`, `Continue`, `Options`, `Exit`).
    pub const ALL: [MainMenuOption; 4] = [
        MainMenuOption::NewGame,
        MainMenuOption::Continue,
        MainMenuOption::Options,
        MainMenuOption::Exit,
    ];

    /// User-facing uppercase label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Load => "LOAD",
            Self::Continue => "CONTINUE",
            Self::NewGame => "NEW GAME",
            Self::Options => "OPTIONS",
            Self::Movies => "MOVIES",
            Self::Exit => "EXIT",
        }
    }
}

/// Save-data availability code returned by `mcStartCheck2()` (`titleCreateMainMenu` `0x001c1a90` /
/// `sh2pc.exe` `0x00495520`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SaveAvailability {
    /// `0` = No save data: hides `LOAD` and `CONTINUE`, defaults cursor to `NEW GAME`.
    #[default]
    None = 0,
    /// `1` = Both `LOAD` and `CONTINUE` available; defaults cursor to `CONTINUE`.
    LoadAndContinue = 1,
    /// `2` = `CONTINUE` only; defaults cursor to `CONTINUE`.
    ContinueOnly = 2,
    /// `3` = `LOAD` only; defaults cursor to `LOAD`.
    LoadOnly = 3,
}

impl SaveAvailability {
    /// Builds the exact ordered main-menu slot list matching `titleCreateMainMenu` (`0x001c1a90`)
    /// and retail PC `sh2pc.exe` (`0x00495520`).
    #[must_use]
    pub fn visible_slots(self, include_pc_slots: bool) -> Vec<MainMenuOption> {
        let mut slots = Vec::with_capacity(6);
        match self {
            Self::None => {}
            Self::LoadAndContinue => {
                slots.push(MainMenuOption::Load);
                slots.push(MainMenuOption::Continue);
            }
            Self::ContinueOnly => {
                slots.push(MainMenuOption::Continue);
            }
            Self::LoadOnly => {
                slots.push(MainMenuOption::Load);
            }
        }
        slots.push(MainMenuOption::NewGame);
        slots.push(MainMenuOption::Options);
        if include_pc_slots {
            slots.push(MainMenuOption::Movies);
            slots.push(MainMenuOption::Exit);
        }
        slots
    }

    /// Default cursor index inside [`Self::visible_slots`] matching `titleCreateMainMenu` (`0x001c1a90`).
    #[must_use]
    pub const fn default_cursor_index(self) -> usize {
        match self {
            Self::None => 0,            // NEW GAME
            Self::LoadAndContinue => 1, // CONTINUE
            Self::ContinueOnly => 0,    // CONTINUE
            Self::LoadOnly => 0,        // LOAD
        }
    }
}

/// Resolves the effective [`RiddleDifficulty`] on starting a Main Scenario New Game, matching
/// retail PC `sh2pc.exe` `0x0048aa20` (`sh2pc_00480000.c:6493-6497`).
///
/// Extra Riddle (`DAT_01dbbff5 == 3`) is not a 4th visible menu option; when the player selects
/// `Hard` (`2`) after clearing Easy (`bit 0`), Normal (`bit 1`), and Hard (`bit 2`) (`clear_mask & 7 == 7`),
/// the engine silently upgrades `Hard` to `Extra` unless Extra was already cleared (`bit 3`) with
/// the alternate toggle bit set.
#[must_use]
pub const fn resolve_effective_riddle(
    scenario: ScenarioKind,
    selected: RiddleDifficulty,
    riddle_clear_mask: u8,
    extra_toggle_bit: bool,
) -> RiddleDifficulty {
    if matches!(scenario, ScenarioKind::LetterFromSilentHeaven)
        && matches!(selected, RiddleDifficulty::Hard)
        && (riddle_clear_mask & 0x07) == 0x07
        && ((riddle_clear_mask & 0x08) == 0 || !extra_toggle_bit)
    {
        RiddleDifficulty::Extra
    } else {
        selected
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
    /// Interactive Act 1 & Act 2 Story Stage Tables (`1..=58`), Riddles, Live Ending Judge, 10-Star Ranking, UFO Comic & Mary's Letter Studio (`SH2-65`).
    StoryStagesAndEndingsRoom,
}

impl TestRoomKind {
    pub const ALL: [TestRoomKind; 16] = [
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
        TestRoomKind::StoryStagesAndEndingsRoom,
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
            Self::StoryStagesAndEndingsRoom => "Story Stages, Riddles & Endings (SH2-65)",
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
            Self::StoryStagesAndEndingsRoom => {
                "Stages 1-58 tables, Act 1/2 Riddles, Ending Judge, 10-Star Rank, Chainsaw/Spray, UFO Comic & Mary's Letter"
            }
        }
    }
}

/// Audio cues emitted by the title menu state machine (`Event/title.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSoundCue {
    None,
    /// Cursor move sound (`shSdCall(0x2710)` / `10000`).
    CursorMove,
    /// Confirm selection sound (`shSdCall(0x2712)` / `10002`).
    Confirm,
    /// Cancel / back sound (`shSdCall(0x2713)` / `10003`).
    Cancel,
    /// Game start audio fadeout cue (`SeCall(1.0f, 0x3a9a)` / `15002`).
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

/// Target runtime session launched when exiting [`TitlePhase::TransitionToGame`] (`SH2-112`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchTarget {
    /// Unified start-to-credits Campaign Game Loop (`gamemain.c` / `connect.c`).
    Campaign {
        /// Selected scenario (`LetterFromSilentHeaven` or `BornFromAWish`).
        scenario: ScenarioKind,
        /// `true` when launched via `Continue` / `Load` (save resume), `false` for `NewGame`.
        is_continue: bool,
    },
    /// Isolated per-subsystem debug / inspection test room.
    TestRoom(TestRoomKind),
}

impl Default for LaunchTarget {
    fn default() -> Self {
        Self::TestRoom(TestRoomKind::ObservationDeck)
    }
}
