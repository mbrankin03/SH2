//! Typed campaign lifecycle state machine (`PlayableMain` `0x0020b3c0`, `connectMain` `0x0020bf80`,
//! `ScreenEffectFadeStart` `0x0020dfc0`, and SH2:EE `DelayedFadeIn.cpp`).
//!
//! Pure state definitions with zero `wgpu`, `winit`, `kira`, or UI rendering dependencies
//! (`AGENTS.md` §1).

use super::cutscene_handoff::DramaDemoTrigger;
use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::event_camera::PostSequenceRestorePolicy;
use crate::events::{ClearData, StageTransitionCommand};
use crate::scenario::ScenarioKind;

/// Default screen fade rate in alpha-per-second when `ScreenEffectFadeStart` (`SLUS_202.28`
/// `0x0020dfe0..0x0020dfe8`) receives `speed == 0.0` (`0x3f99999a` = `1.2f` / sec).
pub const PS2_DEFAULT_FADE_SPEED: f32 = 1.2;

/// SH2:EE `DelayedFadeIn.cpp` (`FadeInStartValue = -0.133334f`): negative pre-roll applied to
/// room fade-in transitions so ~4 frames of black hold hide post-connect animation/camera settle
/// artifacts before ramping opacity to visible.
pub const SH2EE_DELAYED_FADE_IN_START: f32 = -0.133_334;

/// Game Over sound effect ID dispatched by `EventMain` (`SLUS_202.28` `0x001a7b20`:
/// `shSdCall(0x3f4)` = `1012`) when `PlayerEventDeadAnimeFinish` completes.
pub const SE_GAME_OVER: u32 = 0x03f4;

/// Retail starting stage ID for *Letter from Silent Heaven* (`titleFadeOutNewGame` `0x001c2ec0`,
/// `stg_toilet.c` = Stage `2`, `er01.map`, `R_BEGIN_BATHROOM = 0x01`).
pub const MAIN_SCENARIO_START_STAGE: u16 = 2;

/// Retail starting stage ID for *Born from a Wish* (`stage_wish_heaven` = Stage `53`,
/// `th09.map`, `prog_wish.c`).
pub const WISH_SCENARIO_START_STAGE: u16 = 53;

/// Overlay menus that suspend active gameplay inside `PlayableMain` (`SLUS_202.28` `0x00396290`
/// jump table steps `5..=9` and `15`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayMenuKind {
    /// `PlayableMain` step `5` (`0x0020b924`): `ChizuMain` (Area Map screen).
    Map,
    /// `PlayableMain` step `6` (`0x0020b94c`): `itemmain` (Inventory & Equipment screen).
    Inventory,
    /// `PlayableMain` step `7` (`0x0020b988`): `option_main` (In-game Options menu).
    Options,
    /// `PlayableMain` step `8` (`0x0020b9c4`): `MemoMain` (Collected Memos screen).
    Memo,
    /// `PlayableMain` step `9` (`0x0020ba00`): `mcSaveMenu` (Red Square Save screen).
    Save,
    /// `PlayableMain` step `15` (`0x0020bc74`): `PauseDisp` (Pause overlay).
    Pause,
}

impl OverlayMenuKind {
    /// Returns `true` if `GameTimerCountUp` (`0x002902d0`) continues ticking during this overlay
    /// (`PlayableMain` steps `5..=8` call `GameTimerCountUp`; steps `9` and `15` do not).
    #[must_use]
    pub const fn advances_game_timer(self) -> bool {
        matches!(
            self,
            Self::Map | Self::Inventory | Self::Options | Self::Memo
        )
    }
}

/// Sub-steps of `connectMain` (`SLUS_202.28` `0x0020bf80`, jump table `0x003962d0`, steps `0..=7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConnectStep {
    /// Step `0` (`0x0020bfb8`): `StgOverlay` — resolve destination stage descriptor & overlay.
    StageOverlay = 0,
    /// Step `1` (`0x0020bffc`): `lisPutCmd0(connectStageInit)` (`0x0020bd60`) — stage init & chara banks.
    StageInit = 1,
    /// Step `2` (`0x0020c050`): `lisSync(1, -1)` — await stage init completion.
    WaitStageSync = 2,
    /// Step `3` (`0x0020c0a4`): `shCharacterSetPosAfterDemo` (`0x0011f770`) — place player at target pose.
    PlacePlayer = 3,
    /// Step `4` (`0x0020c100`): `lisPutCmd0(connectRoomInit)` (`0x0020be10`) — load `.map`/`.cld`/`.cam`,
    /// `PlayerInitOnConnect`, `MariaInitOnConnect`, `vcInitCamera`, `loadBgCAM_vcReset`.
    RoomInit = 4,
    /// Step `5` (`0x0020c154`): `lisSync(1, -1)` — await async room asset load (`sh2-app` callback).
    WaitRoomSync = 5,
    /// Step `6` (`0x0020c1a8`): `lisPutCmd0(connectPlayableInit)` (`0x0020bf30`) — weapons, flashlight, messages.
    PlayableInit = 6,
    /// Step `7` (`0x0020c1fc`): `lisSync` + `fsSync` + `SeSoundLoad` + `ScreenEffectFadeStart(4)`.
    Complete = 7,
}

impl ConnectStep {
    /// Returns the next sequential [`ConnectStep`], or `None` once [`ConnectStep::Complete`] is reached.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::StageOverlay => Some(Self::StageInit),
            Self::StageInit => Some(Self::WaitStageSync),
            Self::WaitStageSync => Some(Self::PlacePlayer),
            Self::PlacePlayer => Some(Self::RoomInit),
            Self::RoomInit => Some(Self::WaitRoomSync),
            Self::WaitRoomSync => Some(Self::PlayableInit),
            Self::PlayableInit => Some(Self::Complete),
            Self::Complete => None,
        }
    }
}

/// Parameters for loading a target stage and room inside [`CampaignPhase::RoomLoading`].
#[derive(Debug, Clone, PartialEq)]
pub struct RoomLoadTarget {
    /// Target 1-based retail stage ID (`1..=58`).
    pub stage_id: u16,
    /// Optional room identifier override (e.g. `"er01"`, `"ob01"`, `"ap34"`).
    pub room_override: Option<String>,
    /// Destination player world position in PS2 mm.
    pub spawn_pos: [f32; 3],
    /// Destination player facing yaw in radians.
    pub spawn_yaw: f32,
    /// `true` if transitioning between a connected pair in `CONNECT_STAGE_PAIRS` (`bit 0x08`).
    pub is_connected_stage: bool,
    /// `true` to show the PS2 footstep loading screen (`NowLoadingEnable` `0x00298fa0`).
    pub show_loading_screen: bool,
}

/// Sub-state of [`CampaignPhase::FmvPlayback`] distinguishing active playback from a smooth
/// skip fade-out (`ScreenEffectFadeStart(1.2, 1)` `SLUS_202.28` `0x0020dfc0`, `SH2-150`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FmvExitState {
    /// FMV is actively playing (`pssMain` `0x0028ae30` / `sh2pc.exe` `FUN_0043dd10` `0x0043dd10`).
    #[default]
    Playing,
    /// Player requested skip (`pssCheckMovieCancel` `0x0028bbd0`); fading to opaque black at
    /// [`PS2_DEFAULT_FADE_SPEED`] (`1.2 / sec`) before handing off to gameplay, stage, or ending.
    FadingOutSkip,
}

/// Active lifecycle state of the unified start-to-credits campaign game loop (`gamemain.c` /
/// `PlayableMain` `0x0020b3c0` / `connect.c` `0x0020bf80`).
#[derive(Debug, Clone, PartialEq)]
pub enum CampaignPhase {
    /// `PlayableMain` step `0` (`0x0020b414`): New-game initialization (`FlagInit`, `ItemDataInit`,
    /// `ScreenEffectFadeStart(2)`), preparing the opening stage and prologue cutscene.
    Prologue {
        /// Selected scenario (`LetterFromSilentHeaven` or `BornFromAWish`).
        scenario: ScenarioKind,
        /// Selected combat difficulty.
        action_difficulty: ActionDifficulty,
        /// Selected puzzle difficulty.
        riddle_difficulty: RiddleDifficulty,
        /// Persistent NG+ clear data (`0x01dbbf80..0x01dbbf85`) used by `FlagInit` (`0x0048aa4c`).
        clear_data: ClearData,
        /// Non-hardcoded RNG seed for `FlagInit` puzzle randomization (`0x0048abcd`).
        rng_seed: u32,
    },
    /// `PlayableMain` steps `1..=3` (`0x0020b498..0x0020b73b`): Initial or save-load `connectMain`
    /// stage/room load sequence with optional PS2 footstep loading screen (`now_loading.c`).
    RoomLoading {
        /// Target stage, room, and spawn pose being loaded.
        target: RoomLoadTarget,
        /// Current step inside `connectMain` (`0x0020bf80`).
        connect_step: ConnectStep,
        /// Set to `true` once the thin app shell reports `.map`/`.cld`/audio assets are resident.
        assets_ready: bool,
    },
    /// `PlayableMain` step `4` (`0x0020b73c`): Live 60 Hz gameplay (`EventMain` `0x001a7810`,
    /// player locomotion, AI, collision, camera rails, and outdoor `LoadBgSync`).
    ActiveGameplay,
    /// `EventExecMove` (`0x001aa670`) door/room/stage transition: fade out -> `NowLoadingEnable`
    /// -> `connectMain` -> `DelayedFadeIn` -> resume `ActiveGameplay`.
    DoorTransition {
        /// Transition command emitted by `EventExecMove`.
        command: StageTransitionCommand,
        /// Resolved destination room ID (e.g. `"ob01"`, `"ap01"`).
        resolved_room: String,
        /// Current step inside `connectMain` (`0x0020bf80`).
        connect_step: ConnectStep,
        /// `true` once the pre-transition screen fade-out has reached full black (`alpha >= 1.0`).
        fade_out_complete: bool,
        /// `true` once the thin app shell reports destination room assets are resident.
        assets_ready: bool,
        /// `true` if `NowLoadingEnable` (`0x00298fa0`) was activated for this transition.
        loading_screen_active: bool,
    },
    /// Real-time 3D `.dds` cutscene playback (`DramaDemoMain` `0x001b0f00`), locking gameplay
    /// input until completion or skip, then applying `shCharacterSetPosAfterDemo` (`0x0011f770`)
    /// and camera rail warp reset (`VC_RD_WARP_IN_F`).
    DramaDemo {
        /// Retail drama demo number (`1..` in `DRAMA_DEMO_CATALOG`).
        demo_no: u32,
        /// Optional post-cutscene player position (`CharaSetPos` / `shCharacterSetPosAfterDemo`).
        post_pos: Option<[f32; 3]>,
        /// Optional post-cutscene player yaw in radians.
        post_yaw: Option<f32>,
        /// Camera restore policy upon demo exit.
        restore_policy: PostSequenceRestorePolicy,
        /// Current 0-based cursor inside `DramaDemoPlayInfo::sequence_chain`.
        chain_cursor: usize,
    },
    /// Pre-rendered `.bik` / `.pss` FMV playback (`PlayableMain` steps `13..=14`
    /// `0x0020bb28..0x0020bc73`: `MovieWaitReady` -> `MoviePlayFromReady` -> `MovieMain`).
    FmvPlayback {
        /// Canonical movie identifier (e.g. `"toilet"`, `"open"`, `"deai"`, `"murder"`, `"end"`).
        movie_id: &'static str,
        /// Optional destination stage to load immediately after the FMV finishes.
        post_stage: Option<u16>,
        /// Optional destination position after the FMV finishes.
        post_pos: Option<[f32; 3]>,
        /// Optional destination yaw after the FMV finishes.
        post_yaw: Option<f32>,
        /// Optional `.dds` `DramaDemo` to start upon natural FMV completion.
        post_demo: Option<Box<DramaDemoTrigger>>,
        /// Optional event flag to set on completion or skip.
        completion_flag: Option<usize>,
        /// Next step cursor inside `DramaDemoPlayInfo::sequence_chain`.
        chain_cursor: usize,
        /// Active playback vs. smooth skip fade-out sub-state (`SH2-150`).
        exit_state: FmvExitState,
    },
    /// `PlayableMain` steps `5..=9` & `15`: In-game overlay screen (Map, Inventory, Options,
    /// Memo, Save, or Pause).
    OverlayMenu(OverlayMenuKind),
    /// `PlayableMain` step `12` (`0x0020bab4`): `GameoverMain` (`0x001c4c70`).
    GameOver,
    /// `PlayableMain` steps `10..=11` (`0x0020ba20..0x0020bab3`): Ending credits (`GameendMain`
    /// `0x001c4830`) followed by 10-star ranking results (`ResultMain` `0x00293e40`).
    EndingCredits {
        /// Ending stage ID (`48..=52` for Main Scenario, `54` for Born from a Wish).
        ending_stage: u16,
        /// `false` during credits roll (`GameendMain`), `true` on the final results screen (`ResultMain`).
        showing_results: bool,
    },
}

/// Direction of the campaign screen fade overlay (`ScreenEffectFadeStart` `0x0020dfc0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FadeDirection {
    /// No active fade; alpha stays clamped at its current value.
    #[default]
    Idle,
    /// `ScreenEffectFadeStart(1)`: Fading out toward opaque black (`black_alpha -> 1.0`).
    FadeOutToBlack,
    /// `ScreenEffectFadeStart(4)`: Fading in from black toward clear gameplay (`black_alpha -> 0.0`).
    FadeInFromBlack,
}

/// Deterministic screen fade state (`ScreenEffectManager` `0x0020df30` + SH2:EE `DelayedFadeIn.cpp`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenFadeState {
    /// Current active fade direction.
    pub direction: FadeDirection,
    /// Internal progress value where `0.0` = black and `1.0` = fully visible.
    /// May start at [`SH2EE_DELAYED_FADE_IN_START`] (`-0.133334`) when SH2:EE delayed fade-in is enabled.
    visibility: f32,
    /// Fade velocity in units per second (`PS2_DEFAULT_FADE_SPEED` = `1.2`).
    speed: f32,
}

impl Default for ScreenFadeState {
    fn default() -> Self {
        Self::new_clear()
    }
}

impl ScreenFadeState {
    /// Creates a fully visible (transparent black overlay) fade state.
    #[must_use]
    pub const fn new_clear() -> Self {
        Self {
            direction: FadeDirection::Idle,
            visibility: 1.0,
            speed: PS2_DEFAULT_FADE_SPEED,
        }
    }

    /// Creates an immediately black fade state (`ScreenEffectFadeStart(2)`).
    #[must_use]
    pub const fn new_black() -> Self {
        Self {
            direction: FadeDirection::Idle,
            visibility: 0.0,
            speed: PS2_DEFAULT_FADE_SPEED,
        }
    }

    /// Immediately sets screen to opaque black (`ScreenEffectFadeStart(2)`).
    pub fn set_black_immediate(&mut self) {
        self.direction = FadeDirection::Idle;
        self.visibility = 0.0;
    }

    /// Begins fading out to opaque black (`ScreenEffectFadeStart(1)`).
    pub fn start_fade_out(&mut self, speed: f32) {
        self.direction = FadeDirection::FadeOutToBlack;
        self.speed = if speed <= 0.0 {
            PS2_DEFAULT_FADE_SPEED
        } else {
            speed
        };
    }

    /// Begins fading in from black to visible gameplay (`ScreenEffectFadeStart(4)`).
    ///
    /// When `delayed_fade_in_fix` is `true`, seeds `visibility` at [`SH2EE_DELAYED_FADE_IN_START`]
    /// (`-0.133334f` from SH2:EE `DelayedFadeIn.cpp`) so black is held for ~4 frames before ramping.
    pub fn start_fade_in(&mut self, speed: f32, delayed_fade_in_fix: bool) {
        self.direction = FadeDirection::FadeInFromBlack;
        self.visibility = if delayed_fade_in_fix {
            SH2EE_DELAYED_FADE_IN_START
        } else {
            0.0
        };
        self.speed = if speed <= 0.0 {
            PS2_DEFAULT_FADE_SPEED
        } else {
            speed
        };
    }

    /// Advances the fade ramp by `dt_sec` seconds (`ScreenEffectManager` `0x0020df30`).
    pub fn tick(&mut self, dt_sec: f32) {
        if dt_sec <= 0.0 {
            return;
        }
        match self.direction {
            FadeDirection::Idle => {}
            FadeDirection::FadeOutToBlack => {
                self.visibility = (self.visibility - self.speed * dt_sec).max(0.0);
                if self.visibility <= 0.0 {
                    self.direction = FadeDirection::Idle;
                }
            }
            FadeDirection::FadeInFromBlack => {
                self.visibility = (self.visibility + self.speed * dt_sec).min(1.0);
                if self.visibility >= 1.0 {
                    self.direction = FadeDirection::Idle;
                }
            }
        }
    }

    /// Returns black overlay opacity in `[0.0, 1.0]` (`0.0` = clear, `1.0` = opaque black),
    /// matching `ClampFadeASM` in SH2:EE `DelayedFadeIn.cpp` (negative visibility clamps to `0.0`,
    /// yielding `black_alpha = 1.0`).
    #[must_use]
    pub fn black_alpha(&self) -> f32 {
        1.0 - self.visibility.clamp(0.0, 1.0)
    }

    /// Returns `true` when the screen is completely black (`black_alpha >= 1.0`).
    #[must_use]
    pub fn is_fully_black(&self) -> bool {
        self.visibility <= 0.0
    }

    /// Returns `true` when the screen is completely clear (`black_alpha <= 0.0`).
    #[must_use]
    pub fn is_fully_clear(&self) -> bool {
        self.visibility >= 1.0 && self.direction == FadeDirection::Idle
    }
}

/// Typed fade-out / fade-in transition wrapping cutaway still entry and exit
/// (`SLUS_202.28` `EvSubPictureDisplay` `0x001aca00`, `EvSubPictureDisplayAndFadeIn`
/// `0x001ac8b0`, `EvSubPictureDisplayAndFadeOut` `0x001ac9a0`).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CutawayFadeState {
    /// No cutaway fade transition is currently active.
    #[default]
    Idle,
    /// Fading 3D gameplay out to black (`ScreenEffectFadeStart(0, 1)`) before revealing
    /// the cutaway still (`EvSubPictureDisplay` step `0 -> 4`).
    FadingOutToCutaway {
        /// Cutaway picture identifier to display once the screen reaches full black.
        picture: String,
        /// Preserved `out_p_incar` map overlay alpha (`0.0..=1.0`) restored on reveal.
        incar_alpha: f32,
    },
    /// Fading the cutaway still out to black (`ScreenEffectFadeStart(0, 2)`) before
    /// returning to 3D gameplay (`EvSubPictureDisplay` step `6 -> 0xb`).
    FadingOutToGameplay {
        /// Cutaway picture identifier held on screen while fading out to black.
        picture: String,
        /// Preserved `out_p_incar` map overlay alpha (`0.0..=1.0`) held during fade-out.
        incar_alpha: f32,
    },
}
