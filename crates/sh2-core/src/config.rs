//! Engine and game configuration settings with persistent JSON serialization.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Internal rendering resolution option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InternalResolution {
    /// Native window / screen resolution (1x).
    #[default]
    Native,
    /// 1080p (1920x1080).
    Res1080p,
    /// 1440p (2560x1440).
    Res1440p,
    /// 4K UHD (3840x2160).
    Res4k,
}

impl InternalResolution {
    /// Return the width and height pair for the resolution.
    pub fn dimensions(&self, native_w: u32, native_h: u32) -> (u32, u32) {
        match self {
            Self::Native => (native_w, native_h),
            Self::Res1080p => (1920, 1080),
            Self::Res1440p => (2560, 1440),
            Self::Res4k => (3840, 2160),
        }
    }

    /// User-facing label string.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::Res1080p => "1080p (1920x1080)",
            Self::Res1440p => "1440p (2560x1440)",
            Self::Res4k => "4K UHD (3840x2160)",
        }
    }

    /// Next cyclical resolution setting.
    pub fn next(&self) -> Self {
        match self {
            Self::Native => Self::Res1080p,
            Self::Res1080p => Self::Res1440p,
            Self::Res1440p => Self::Res4k,
            Self::Res4k => Self::Native,
        }
    }

    /// Previous cyclical resolution setting.
    pub fn prev(&self) -> Self {
        match self {
            Self::Native => Self::Res4k,
            Self::Res1080p => Self::Native,
            Self::Res1440p => Self::Res1080p,
            Self::Res4k => Self::Res1440p,
        }
    }
}

/// Window display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WindowDisplayMode {
    /// Borderless window covering the current monitor.
    #[default]
    BorderlessFullscreen,
    /// Standard window with system titlebar.
    Windowed,
    /// Exclusive full screen mode.
    ExclusiveFullscreen,
}

impl WindowDisplayMode {
    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BorderlessFullscreen => "Borderless Fullscreen",
            Self::Windowed => "Windowed",
            Self::ExclusiveFullscreen => "Exclusive Fullscreen",
        }
    }

    /// Cycle to next display mode.
    pub fn next(&self) -> Self {
        match self {
            Self::BorderlessFullscreen => Self::Windowed,
            Self::Windowed => Self::ExclusiveFullscreen,
            Self::ExclusiveFullscreen => Self::BorderlessFullscreen,
        }
    }

    /// Cycle to previous display mode.
    pub fn prev(&self) -> Self {
        match self {
            Self::BorderlessFullscreen => Self::ExclusiveFullscreen,
            Self::Windowed => Self::BorderlessFullscreen,
            Self::ExclusiveFullscreen => Self::Windowed,
        }
    }
}

/// Atmospheric fog quality mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FogQuality {
    /// Classic fixed-function PS2 atmospheric emulation.
    ClassicPs2,
    /// Modern 3D volumetric compute fog.
    #[default]
    ModernVolumetric,
}

impl FogQuality {
    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClassicPs2 => "Classic PS2 (Emulated)",
            Self::ModernVolumetric => "Modern Volumetric (Compute)",
        }
    }

    /// Cycle fog quality.
    pub fn toggle(&self) -> Self {
        match self {
            Self::ClassicPs2 => Self::ModernVolumetric,
            Self::ModernVolumetric => Self::ClassicPs2,
        }
    }
}

/// Speaker output configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpeakerConfiguration {
    #[default]
    Stereo,
    Headphones,
    Surround51,
}

impl SpeakerConfiguration {
    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stereo => "Stereo 2.0",
            Self::Headphones => "Headphones (Binaural)",
            Self::Surround51 => "Surround 5.1",
        }
    }

    /// Next output config.
    pub fn next(&self) -> Self {
        match self {
            Self::Stereo => Self::Headphones,
            Self::Headphones => Self::Surround51,
            Self::Surround51 => Self::Stereo,
        }
    }

    /// Previous output config.
    pub fn prev(&self) -> Self {
        match self {
            Self::Stereo => Self::Surround51,
            Self::Headphones => Self::Stereo,
            Self::Surround51 => Self::Headphones,
        }
    }
}

/// Controller button prompt glyph style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ButtonGlyphStyle {
    /// PlayStation DualShock / DualSense glyphs (Cross, Circle, Square, Triangle).
    #[default]
    PlayStation,
    /// Xbox glyphs (A, B, X, Y).
    Xbox,
    /// Handheld style (AYN Thor / Steam Deck).
    HandheldThor,
}

impl ButtonGlyphStyle {
    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::PlayStation => "PlayStation (DualSense)",
            Self::Xbox => "Xbox / PC",
            Self::HandheldThor => "AYN Thor / Steam Deck",
        }
    }

    /// Next glyph style.
    pub fn next(&self) -> Self {
        match self {
            Self::PlayStation => Self::Xbox,
            Self::Xbox => Self::HandheldThor,
            Self::HandheldThor => Self::PlayStation,
        }
    }

    /// Previous glyph style.
    pub fn prev(&self) -> Self {
        match self {
            Self::PlayStation => Self::HandheldThor,
            Self::Xbox => Self::PlayStation,
            Self::HandheldThor => Self::Xbox,
        }
    }
}

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

/// Frame rate cap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FrameRateCap {
    #[default]
    Fps60,
    Fps30,
    Uncapped,
}

impl FrameRateCap {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fps60 => "60 FPS (Authentic)",
            Self::Fps30 => "30 FPS (Half-Rate)",
            Self::Uncapped => "Uncapped",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Fps60 => Self::Fps30,
            Self::Fps30 => Self::Uncapped,
            Self::Uncapped => Self::Fps60,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Fps60 => Self::Uncapped,
            Self::Fps30 => Self::Fps60,
            Self::Uncapped => Self::Fps30,
        }
    }
}

/// Video and graphics options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoOptions {
    pub resolution: InternalResolution,
    pub display_mode: WindowDisplayMode,
    pub vsync: bool,
    pub framerate_cap: FrameRateCap,
    pub fog_quality: FogQuality,
    /// Procedural film grain noise intensity in percentage (0 to 100).
    pub grain_intensity: u32,
    pub volumetric_light: bool,
    pub lens_flare: bool,
    pub depth_of_field: bool,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            resolution: InternalResolution::default(),
            display_mode: WindowDisplayMode::default(),
            vsync: true,
            framerate_cap: FrameRateCap::default(),
            fog_quality: FogQuality::default(),
            grain_intensity: 65,
            volumetric_light: true,
            lens_flare: true,
            depth_of_field: true,
        }
    }
}

/// Audio volume and routing options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioOptions {
    /// Master volume (0.0 to 1.0).
    pub master_volume: f32,
    /// Background music volume (0.0 to 1.0).
    pub bgm_volume: f32,
    /// Sound effects volume (0.0 to 1.0).
    pub sfx_volume: f32,
    /// Dialogue / voice volume (0.0 to 1.0).
    pub dialogue_volume: f32,
    /// Output speaker configuration.
    pub speaker_config: SpeakerConfiguration,
    /// Mute game audio when the window loses focus.
    pub mute_in_background: bool,
}

impl Default for AudioOptions {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            bgm_volume: 0.85,
            sfx_volume: 0.9,
            dialogue_volume: 1.0,
            speaker_config: SpeakerConfiguration::default(),
            mute_in_background: true,
        }
    }
}

/// Control options matching SH2 / SH2:EE controls config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlOptions {
    /// 3D Tank controls (authentic PS2) vs 2D camera-relative directional controls.
    pub tank_controls: bool,
    /// Button prompt visual theme.
    pub glyph_style: ButtonGlyphStyle,
    /// Analog stick deadzone ratio (0.0 to 0.5).
    pub stick_deadzone: f32,
    /// Stick sensitivity multiplier (0.5 to 2.0).
    pub stick_sensitivity: f32,
    /// Vibration / haptic feedback intensity (0.0 to 1.0).
    pub vibration_intensity: f32,
}

impl Default for ControlOptions {
    fn default() -> Self {
        Self {
            tank_controls: true,
            glyph_style: ButtonGlyphStyle::default(),
            stick_deadzone: 0.15,
            stick_sensitivity: 1.0,
            vibration_intensity: 1.0,
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
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            action_difficulty: ActionDifficulty::default(),
            riddle_difficulty: RiddleDifficulty::default(),
            subtitles: true,
            language: GameLanguage::default(),
            bullet_multiplier: BulletMultiplier::default(),
        }
    }
}

/// Complete game configuration structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub video: VideoOptions,
    pub audio: AudioOptions,
    pub controls: ControlOptions,
    pub game: GameOptions,
}

impl AppConfig {
    /// Serialize and write configuration to disk.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let serialized = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serialized)
    }

    /// Load configuration from disk, falling back to default if file is missing or invalid.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Self {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_and_cycling() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.video.resolution, InternalResolution::Native);
        assert_eq!(cfg.video.resolution.next(), InternalResolution::Res1080p);
        assert_eq!(cfg.video.resolution.prev(), InternalResolution::Res4k);

        assert_eq!(cfg.video.display_mode.next(), WindowDisplayMode::Windowed);
        assert_eq!(cfg.video.fog_quality.toggle(), FogQuality::ClassicPs2);

        assert_eq!(
            cfg.audio.speaker_config.next(),
            SpeakerConfiguration::Headphones
        );
        assert_eq!(cfg.controls.glyph_style.next(), ButtonGlyphStyle::Xbox);
        assert_eq!(cfg.game.action_difficulty.next(), ActionDifficulty::Hard);
        assert_eq!(cfg.game.riddle_difficulty.next(), RiddleDifficulty::Hard);
        assert_eq!(cfg.game.language.next(), GameLanguage::Japanese);
        assert_eq!(cfg.game.language.code(), 'e');
        assert_eq!(cfg.game.bullet_multiplier.next(), BulletMultiplier::Two);

        cfg.video.grain_intensity = 80;
        cfg.audio.sfx_volume = 0.75;
        assert_eq!(cfg.video.grain_intensity, 80);
        assert_eq!(cfg.audio.sfx_volume, 0.75);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut original = AppConfig::default();
        original.video.resolution = InternalResolution::Res1440p;
        original.video.grain_intensity = 45;
        original.audio.master_volume = 0.8;
        original.controls.tank_controls = false;
        original.game.language = GameLanguage::Spanish;

        let json = serde_json::to_string(&original).expect("serialize config");
        let decoded: AppConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(original, decoded);
    }
}
