//! Gameplay difficulty, language, debug overlays, and master enhanced preset options.

use serde::{Deserialize, Serialize};

/// Game action difficulty level (`sh2pc.exe` `DAT_01dbbff4` / `titleGetBattleLevelFromCursor`
/// `0x00495b70`: `0 = Beginner`, `1 = Easy`, `2 = Normal`, `3 = Hard`).
///
/// Prototype drift: Jul 13, 2001 `SLUS_202.28` (`0x001c2a40`) only exposes 3 levels
/// (`Easy = 1`, `Normal = 2`, `Hard = 3`); retail PS2 & PC add `Beginner = 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActionDifficulty {
    /// Retail code `0` (`sh2pc.exe` `0x00495b70`, cursor row 4).
    Beginner,
    /// Retail code `1` (`SLUS_202.28` `0x001c2a40` / `sh2pc.exe` `0x00495b70`, cursor row 3).
    Easy,
    /// Retail code `2` (default in `PlayingInfoColdInit` `0x0020a680`, cursor row 2).
    #[default]
    Normal,
    /// Retail code `3` (`SLUS_202.28` `0x001c2a40` / `sh2pc.exe` `0x00495b70`, cursor row 1).
    Hard,
}

impl ActionDifficulty {
    /// Retail numeric code (`DAT_01dbbff4`: `0 = Beginner`, `1 = Easy`, `2 = Normal`, `3 = Hard`).
    #[must_use]
    pub const fn to_retail_code(self) -> u8 {
        match self {
            Self::Beginner => 0,
            Self::Easy => 1,
            Self::Normal => 2,
            Self::Hard => 3,
        }
    }

    /// Decodes a retail numeric code (`DAT_01dbbff4`) into [`ActionDifficulty`].
    #[must_use]
    pub const fn from_retail_code(code: u8) -> Self {
        match code {
            0 => Self::Beginner,
            1 => Self::Easy,
            3 => Self::Hard,
            _ => Self::Normal,
        }
    }

    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Beginner => "Beginner",
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }

    /// Cycle to next difficulty (`Beginner -> Easy -> Normal -> Hard -> Beginner`).
    pub fn next(&self) -> Self {
        match self {
            Self::Beginner => Self::Easy,
            Self::Easy => Self::Normal,
            Self::Normal => Self::Hard,
            Self::Hard => Self::Beginner,
        }
    }

    /// Cycle to previous difficulty (`Beginner -> Hard -> Normal -> Easy -> Beginner`).
    pub fn prev(&self) -> Self {
        match self {
            Self::Beginner => Self::Hard,
            Self::Easy => Self::Beginner,
            Self::Normal => Self::Easy,
            Self::Hard => Self::Normal,
        }
    }
}

/// Game riddle difficulty level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RiddleDifficulty {
    Easy,
    #[default]
    Normal,
    Hard,
    Extra,
}

impl RiddleDifficulty {
    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
            Self::Extra => "Extra",
        }
    }

    /// Cycle to next riddle difficulty.
    pub fn next(&self) -> Self {
        match self {
            Self::Easy => Self::Normal,
            Self::Normal => Self::Hard,
            Self::Hard => Self::Extra,
            Self::Extra => Self::Easy,
        }
    }

    /// Cycle to previous riddle difficulty.
    pub fn prev(&self) -> Self {
        match self {
            Self::Easy => Self::Extra,
            Self::Normal => Self::Easy,
            Self::Hard => Self::Normal,
            Self::Extra => Self::Hard,
        }
    }
}

/// Subtitle language options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameLanguage {
    #[default]
    English,
    Japanese,
    French,
    German,
    Italian,
    Spanish,
}

impl GameLanguage {
    /// Return single-letter code used in SH2 file naming (e.g. `start01e.tex`).
    pub fn code(&self) -> char {
        match self {
            Self::English => 'e',
            Self::Japanese => 'j',
            Self::French => 'f',
            Self::German => 'g',
            Self::Italian => 'i',
            Self::Spanish => 's',
        }
    }

    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Japanese => "Japanese",
            Self::French => "French",
            Self::German => "German",
            Self::Italian => "Italian",
            Self::Spanish => "Spanish",
        }
    }

    /// Cycle to next language.
    pub fn next(&self) -> Self {
        match self {
            Self::English => Self::Japanese,
            Self::Japanese => Self::French,
            Self::French => Self::German,
            Self::German => Self::Italian,
            Self::Italian => Self::Spanish,
            Self::Spanish => Self::English,
        }
    }

    /// Cycle to previous language.
    pub fn prev(&self) -> Self {
        match self {
            Self::English => Self::Spanish,
            Self::Japanese => Self::English,
            Self::French => Self::Japanese,
            Self::German => Self::French,
            Self::Italian => Self::German,
            Self::Spanish => Self::Italian,
        }
    }
}

/// Bullet multiplier settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum BulletMultiplier {
    Half,
    #[default]
    One,
    Two,
    Three,
}

impl BulletMultiplier {
    /// Numerical scale factor.
    pub fn value(&self) -> f32 {
        match self {
            Self::Half => 0.5,
            Self::One => 1.0,
            Self::Two => 2.0,
            Self::Three => 3.0,
        }
    }

    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Half => "0.5x",
            Self::One => "1.0x (Normal)",
            Self::Two => "2.0x",
            Self::Three => "3.0x",
        }
    }

    /// Next multiplier.
    pub fn next(&self) -> Self {
        match self {
            Self::Half => Self::One,
            Self::One => Self::Two,
            Self::Two => Self::Three,
            Self::Three => Self::Half,
        }
    }

    /// Previous multiplier.
    pub fn prev(&self) -> Self {
        match self {
            Self::Half => Self::Three,
            Self::One => Self::Half,
            Self::Two => Self::One,
            Self::Three => Self::Two,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_brightness_level() -> u8 {
    PS2_DEFAULT_BRIGHTNESS_LEVEL
}

/// Default PS2 brightness level (`PlayingInfoColdInit` `0x0020a680`, `DAT_0122b386 = 3`).
pub const PS2_DEFAULT_BRIGHTNESS_LEVEL: u8 = 3;

/// Maximum PS2 brightness level (`bright_main` `0x00212ec0`, range `0..=7`).
pub const PS2_MAX_BRIGHTNESS_LEVEL: u8 = 7;

/// Retail PC brightness multiplier LUT for `level = 0..=7` (`sh2pc.exe` `FUN_0047b770` `0x0047b770`).
pub const PS2_BRIGHTNESS_MULTIPLIERS: [f32; 8] = [0.5, 0.6, 0.7, 1.0, 1.2, 1.4, 1.7, 2.0];

/// PS2 Extra Options Blood Color (`Item/otn_option.c` `0x00210d50`, `DAT_0122b38f`, msgs `0x30/0x46/0x32/0x33/0x34`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BloodColor {
    /// `0` = Normal red (`msg 0x46`, default in `PlayingInfoColdInit` `0x0020a680`).
    #[default]
    Normal = 0,
    /// `1` = Green (`msg 0x32`).
    Green = 1,
    /// `2` = Violet (`msg 0x33`).
    Violet = 2,
    /// `3` = Black (`msg 0x34`).
    Black = 3,
}

impl BloodColor {
    /// User-facing label matching `option_msg_e.mes`.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Green => "Green",
            Self::Violet => "Violet",
            Self::Black => "Black",
        }
    }

    /// Cycle forward (`0 -> 1 -> 2 -> 3 -> 0`).
    pub const fn next(self) -> Self {
        match self {
            Self::Normal => Self::Green,
            Self::Green => Self::Violet,
            Self::Violet => Self::Black,
            Self::Black => Self::Normal,
        }
    }

    /// Cycle backward (`0 -> 3 -> 2 -> 1 -> 0`).
    pub const fn prev(self) -> Self {
        match self {
            Self::Normal => Self::Black,
            Self::Green => Self::Normal,
            Self::Violet => Self::Green,
            Self::Black => Self::Violet,
        }
    }
}

/// PS2 Extra Options View Control (`Item/otn_option.c` `0x00210d50`, `DAT_0122b390`, msgs `0x35/0x46/0x47`).
///
/// Retail PC drift: `sh2pc.exe` (`0x00462e50`, `DAT_01dbc000`) labels mode `1` as `"Switch"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ViewControlMode {
    /// `0` = Normal (default).
    #[default]
    Normal = 0,
    /// `1` = Reverse (PS2 `msg 0x47`) / Switch (PC `msg 0x2f`).
    Reverse = 1,
}

impl ViewControlMode {
    /// User-facing label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Reverse => "Reverse",
        }
    }

    /// Toggle between `Normal` and `Reverse`.
    pub const fn toggle(self) -> Self {
        match self {
            Self::Normal => Self::Reverse,
            Self::Reverse => Self::Normal,
        }
    }
}

/// PS2 Extra Options Retreat Turn (`Item/otn_option.c` `0x00210d50`, `DAT_0122b391`, msgs `0x37/0x46/0x47`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RetreatTurnMode {
    /// `0` = Normal (default).
    #[default]
    Normal = 0,
    /// `1` = Reverse (`msg 0x47`).
    Reverse = 1,
}

impl RetreatTurnMode {
    /// User-facing label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Reverse => "Reverse",
        }
    }

    /// Toggle between `Normal` and `Reverse`.
    pub const fn toggle(self) -> Self {
        match self {
            Self::Normal => Self::Reverse,
            Self::Reverse => Self::Normal,
        }
    }
}

/// PS2/PC Extra Options Walk/Run Control (`Item/otn_option.c` `0x00210d50`, `DAT_0122b392`,
/// retail PC `sh2pc.exe` `0x00462e50` `DAT_01dbc002`).
///
/// Prototype drift: `SLUS_202.28` has 2 states (`Normal = 0`, `Reverse = 1`); retail PC expands
/// to 3 states (`Walk = 0`, `Run = 1`, `Analog = 2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WalkRunControlMode {
    /// `0` = Normal (Walk default, hold Run).
    #[default]
    Normal = 0,
    /// `1` = Reverse (Run default, hold Walk).
    Reverse = 1,
    /// `2` = Analog (Retail PC `sh2pc.exe` `0x01dbc002 == 2`).
    Analog = 2,
}

impl WalkRunControlMode {
    /// User-facing label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal (Walk)",
            Self::Reverse => "Reverse (Run)",
            Self::Analog => "Analog",
        }
    }

    /// Cycle forward (`Normal -> Reverse -> Analog -> Normal`).
    pub const fn next(self) -> Self {
        match self {
            Self::Normal => Self::Reverse,
            Self::Reverse => Self::Analog,
            Self::Analog => Self::Normal,
        }
    }

    /// Cycle backward (`Normal -> Analog -> Reverse -> Normal`).
    pub const fn prev(self) -> Self {
        match self {
            Self::Normal => Self::Analog,
            Self::Reverse => Self::Normal,
            Self::Analog => Self::Reverse,
        }
    }
}

/// Gameplay and puzzle difficulty preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameOptions {
    pub action_difficulty: ActionDifficulty,
    pub riddle_difficulty: RiddleDifficulty,
    pub subtitles: bool,
    pub language: GameLanguage,
    pub bullet_multiplier: BulletMultiplier,
    /// Allow saving anywhere from the pause menu (Retail PC behavior) vs Red Squares only (PS2 original behavior).
    pub save_anywhere: bool,
    /// SH2:EE `DelayedFadeIn.cpp` (`-0.133334f` black pre-roll on room fade-in) vs 1:1 vanilla PS2 fade-in (`0.0`).
    #[serde(default = "default_true")]
    pub enable_delayed_fade_in: bool,
    /// PS2 Main Options Auto Load (`SLUS_202.28` `0x0122b38a`, default `false`).
    #[serde(default)]
    pub auto_load: bool,
    /// PS2 Extra Options Blood Color (`SLUS_202.28` `0x0122b38f`, default `Normal`).
    #[serde(default)]
    pub blood_color: BloodColor,
    /// PS2 Extra Options View Control (`SLUS_202.28` `0x0122b390`, default `Normal`).
    #[serde(default)]
    pub view_control: ViewControlMode,
    /// PS2 Extra Options Retreat Turn (`SLUS_202.28` `0x0122b391`, default `Normal`).
    #[serde(default)]
    pub retreat_turn: RetreatTurnMode,
    /// PS2/PC Extra Options Walk/Run Control (`SLUS_202.28` `0x0122b392` / `sh2pc` `0x01dbc002`, default `Normal`).
    #[serde(default)]
    pub walk_run_control: WalkRunControlMode,
    /// PS2 Brightness calibration level `0..=7` (`bright_main` `0x00212ec0`, `DAT_0122b386`, default `3`).
    #[serde(default = "default_brightness_level")]
    pub brightness_level: u8,
    /// Native-upgrade procedural secondary bone spring-damper cloth/jacket/skirt physics
    /// (`secondary_physics.rs`). Default `false` for strict 1:1 original PS2 parity (AGENTS.md Rule 3.4).
    #[serde(default)]
    pub enable_secondary_physics: bool,
    /// SH2:EE world & event bugfixes (`FlashlightClockPush`, `ElevatorCursorColorFix`,
    /// `HospitalChase`, `CreatureVehicleSpawn`). `false` in strict 1:1 Vanilla PS2 mode.
    #[serde(default = "default_true")]
    pub enable_sh2ee_world_fixes: bool,
    /// Native-upgrade unlimited save slots + JSON/bincode export (`false` = 1:1 PS2 2 cards × 15 slots).
    #[serde(default)]
    pub enable_unlimited_save_slots: bool,
    /// Native-upgrade quicksave / autosave slots (`false` = 1:1 PS2 manual save only).
    #[serde(default)]
    pub enable_quicksave_autosave: bool,
    /// Native-upgrade live 3D save thumbnails & Ending Tracker snapshot (`false` = 1:1 PS2 `savebg`).
    #[serde(default)]
    pub enable_save_thumbnails: bool,
    /// Native-upgrade free 3D item orbit & zoom in inventory Examine (`false` = 1:1 PS2 examine).
    #[serde(default)]
    pub enable_free_item_orbit: bool,
    /// SH2:EE `CutsceneUnequip` (`Patches/RemoveWeaponFromCutscene.cpp`): auto-hide James's equipped
    /// right-hand weapon mesh during `.dds` cutscenes (`false` in strict 1:1 Vanilla PS2 mode).
    #[serde(default = "default_true")]
    pub enable_cutscene_unequip: bool,
    /// Native-upgrade analytical 2-bone Foot IK on stairs and slopes (`animation::foot_ik`).
    /// Default `false` for strict 1:1 original PS2 parity (AGENTS.md Rule 3.4).
    #[serde(default)]
    pub enable_foot_ik: bool,
    /// Native-upgrade Persistent Room Decals & Coagulation (`effects::blood`): preserves floor
    /// blood pools, splatters, and bloody footprints across room transitions with gradual
    /// coagulation darkening, vs strict 1:1 PS2 `blood_clear()` on room exit (`false`).
    #[serde(default)]
    pub enable_persistent_blood_decals: bool,
}

/// Debug and development options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugOptions {
    /// Instant room transitions (skip footstep loading screen).
    pub instant_room_transitions: bool,
    /// Minimum time in seconds to display loading screen footsteps (default: 2s).
    pub min_loading_screen_seconds: u32,
    /// Require user button press (Press to Continue) when room loading finishes.
    pub require_press_to_continue: bool,
    /// Render collision wireframes in 3D world.
    pub show_collision_wireframe: bool,
    /// Render camera raycast and occlusion debug overlay.
    pub show_camera_debug: bool,
}

impl Default for DebugOptions {
    fn default() -> Self {
        Self {
            instant_room_transitions: false,
            min_loading_screen_seconds: 2,
            require_press_to_continue: false,
            show_collision_wireframe: false,
            show_camera_debug: false,
        }
    }
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            action_difficulty: ActionDifficulty::default(),
            riddle_difficulty: RiddleDifficulty::default(),
            subtitles: true,
            language: GameLanguage::default(),
            bullet_multiplier: BulletMultiplier::default(),
            save_anywhere: true,
            enable_delayed_fade_in: true,
            auto_load: false,
            blood_color: BloodColor::default(),
            view_control: ViewControlMode::default(),
            retreat_turn: RetreatTurnMode::default(),
            walk_run_control: WalkRunControlMode::default(),
            brightness_level: PS2_DEFAULT_BRIGHTNESS_LEVEL,
            enable_secondary_physics: false,
            enable_sh2ee_world_fixes: true,
            enable_unlimited_save_slots: false,
            enable_quicksave_autosave: false,
            enable_save_thumbnails: false,
            enable_free_item_orbit: false,
            enable_cutscene_unequip: true,
            enable_foot_ik: false,
            enable_persistent_blood_decals: false,
        }
    }
}

/// Master version preset collating all enhanced graphics, audio, and camera toggles so players
/// can instantly swap between PS2 Authentic, SH2:EE, and Native Enhanced profiles to compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EnhancedPreset {
    /// Strict 1:1 PlayStation 2 original visuals, audio, and camera behavior.
    Ps2Authentic,
    /// Silent Hill 2: Enhanced Edition (HD textures, soft stencil shadows, lighting fixes, lossless audio).
    Sh2ee,
    /// Full native Rust engine upgrades (4K AI textures, volumetric fog/lighting, PCSS self-shadows, HRTF, wall occlusion, OTS camera).
    #[default]
    NativeEnhanced,
    /// Mixed user-customized combination of individual enhanced toggles.
    Custom,
}

impl EnhancedPreset {
    /// User-facing label for the Enhanced Mode options tab.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ps2Authentic => "PS2 Authentic (1:1 Original)",
            Self::Sh2ee => "SH2:EE (Enhanced Edition)",
            Self::NativeEnhanced => "Native Enhanced (All Upgrades)",
            Self::Custom => "Custom (Mixed)",
        }
    }

    /// Next selectable preset (skips `Custom`, snapping to the next full version profile).
    pub const fn next_selectable(self) -> Self {
        match self {
            Self::Ps2Authentic => Self::Sh2ee,
            Self::Sh2ee => Self::NativeEnhanced,
            Self::NativeEnhanced | Self::Custom => Self::Ps2Authentic,
        }
    }

    /// Previous selectable preset (skips `Custom`, snapping to the previous full version profile).
    pub const fn prev_selectable(self) -> Self {
        match self {
            Self::Ps2Authentic | Self::Custom => Self::NativeEnhanced,
            Self::Sh2ee => Self::Ps2Authentic,
            Self::NativeEnhanced => Self::Sh2ee,
        }
    }
}
