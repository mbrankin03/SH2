//! Gameplay difficulty, language, debug overlays, and master enhanced preset options.

use serde::{Deserialize, Serialize};

/// Game action difficulty level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActionDifficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl ActionDifficulty {
    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }

    /// Cycle to next difficulty.
    pub fn next(&self) -> Self {
        match self {
            Self::Easy => Self::Normal,
            Self::Normal => Self::Hard,
            Self::Hard => Self::Easy,
        }
    }

    /// Cycle to previous difficulty.
    pub fn prev(&self) -> Self {
        match self {
            Self::Easy => Self::Hard,
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
