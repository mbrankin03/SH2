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

/// Frame rate cap mode supporting 30 Hz, 60 Hz, high-refresh OLED (120/144/240 Hz), and Uncapped VRR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FrameRateCap {
    Fps30,
    #[default]
    Fps60,
    Fps120,
    Fps144,
    Fps240,
    Uncapped,
}

impl FrameRateCap {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fps30 => "30 FPS (PS2 Cutscene Rate)",
            Self::Fps60 => "60 FPS (Authentic)",
            Self::Fps120 => "120 FPS (High Refresh)",
            Self::Fps144 => "144 FPS (High Refresh)",
            Self::Fps240 => "240 FPS (OLED Ultra)",
            Self::Uncapped => "Uncapped (VRR)",
        }
    }

    /// Returns the numeric target frames per second, or `None` when uncapped.
    pub fn target_fps(&self) -> Option<u32> {
        match self {
            Self::Fps30 => Some(30),
            Self::Fps60 => Some(60),
            Self::Fps120 => Some(120),
            Self::Fps144 => Some(144),
            Self::Fps240 => Some(240),
            Self::Uncapped => None,
        }
    }

    /// Returns the exact target frame interval duration, or `None` when uncapped.
    pub fn target_frame_duration(&self) -> Option<std::time::Duration> {
        self.target_fps().map(|fps| {
            std::time::Duration::from_nanos((1_000_000_000u64 + (fps as u64 / 2)) / fps as u64)
        })
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Fps30 => Self::Fps60,
            Self::Fps60 => Self::Fps120,
            Self::Fps120 => Self::Fps144,
            Self::Fps144 => Self::Fps240,
            Self::Fps240 => Self::Uncapped,
            Self::Uncapped => Self::Fps30,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Fps30 => Self::Uncapped,
            Self::Fps60 => Self::Fps30,
            Self::Fps120 => Self::Fps60,
            Self::Fps144 => Self::Fps120,
            Self::Fps240 => Self::Fps144,
            Self::Uncapped => Self::Fps240,
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

/// Audio pack preference for background music (`.adx` / `.aix`) and sound effects (`sddata.bin` / `extra/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AudioPackPreference {
    /// Automatically select SH2:EE 48 kHz Lossless if present, otherwise Original PS2/PC.
    #[default]
    Auto,
    /// Silent Hill 2: Enhanced Edition 48 kHz Lossless Audio Pack (`sh2e/sound/`), falling back to retail (`data/sound/`).
    #[serde(alias = "EnhancedLossless")]
    Sh2eeLossless,
    /// Original PS2 / Retail PC 22.05 / 24 / 44.1 kHz Audio Pack (`data/sound/`).
    #[serde(alias = "OriginalPs2")]
    Ps2Original,
}

impl AudioPackPreference {
    /// Backwards-compatible constant alias for `Self::Sh2eeLossless`.
    #[allow(non_upper_case_globals)]
    pub const EnhancedLossless: Self = Self::Sh2eeLossless;

    /// Backwards-compatible constant alias for `Self::Ps2Original`.
    #[allow(non_upper_case_globals)]
    pub const OriginalPs2: Self = Self::Ps2Original;

    /// User-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto (Prefer SH2:EE Lossless)",
            Self::Sh2eeLossless => "SH2:EE Lossless (48 kHz)",
            Self::Ps2Original => "Original PS2 / PC (24/44.1 kHz)",
        }
    }

    /// Cycle to next audio pack preference (`Auto -> Sh2eeLossless -> Ps2Original -> Auto`).
    pub fn next(&self) -> Self {
        match self {
            Self::Auto => Self::Sh2eeLossless,
            Self::Sh2eeLossless => Self::Ps2Original,
            Self::Ps2Original => Self::Auto,
        }
    }

    /// Cycle to previous audio pack preference (`Auto -> Ps2Original -> Sh2eeLossless -> Auto`).
    pub fn prev(&self) -> Self {
        match self {
            Self::Auto => Self::Ps2Original,
            Self::Sh2eeLossless => Self::Auto,
            Self::Ps2Original => Self::Sh2eeLossless,
        }
    }

    /// Toggles directly between `Sh2eeLossless` and `Ps2Original` (resolving `Auto` against `status` first).
    pub fn toggle_concrete(&self, status: &AudioPackDetectionStatus) -> Self {
        match status.resolve_source(*self) {
            Self::Sh2eeLossless => Self::Ps2Original,
            _ => Self::Sh2eeLossless,
        }
    }
}

/// Type alias preserving compatibility with existing `AudioPackSource` consumers (`SH2-60`).
pub type AudioPackSource = AudioPackPreference;

/// Filesystem detection summary for SH2:EE Lossless (`sh2e/sound/`) and Original PS2/PC (`data/sound/`) audio packs (`SH2-59`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPackDetectionStatus {
    /// `true` when `sh2e/sound/adx/` or `sh2e/sound/sddata.bin` is detected on disk.
    pub sh2ee_lossless_present: bool,
    /// `true` when `data/sound/adx/` or `data/sound/sddata.bin` is detected on disk.
    pub ps2_original_present: bool,
    /// `true` when `sh2e/sound/extra/` is detected on disk.
    pub sh2ee_extra_present: bool,
}

impl Default for AudioPackDetectionStatus {
    fn default() -> Self {
        Self {
            sh2ee_lossless_present: true,
            ps2_original_present: true,
            sh2ee_extra_present: true,
        }
    }
}

impl AudioPackDetectionStatus {
    /// Scans the given `game_root` directory (containing `sh2e/` and/or `data/`) for installed audio packs.
    pub fn detect_from_root<P: AsRef<Path>>(game_root: P) -> Self {
        let root = game_root.as_ref();
        let sh2e_sound = root.join("sh2e").join("sound");
        let data_sound = root.join("data").join("sound");

        let sh2ee_lossless_present =
            sh2e_sound.join("adx").is_dir() || sh2e_sound.join("sddata.bin").is_file();
        let ps2_original_present =
            data_sound.join("adx").is_dir() || data_sound.join("sddata.bin").is_file();
        let sh2ee_extra_present = sh2e_sound.join("extra").is_dir();

        Self {
            sh2ee_lossless_present,
            ps2_original_present,
            sh2ee_extra_present,
        }
    }

    /// Returns `true` when both SH2:EE Lossless and Original PS2/PC audio packs are installed.
    pub fn both_detected(&self) -> bool {
        self.sh2ee_lossless_present && self.ps2_original_present
    }

    /// Resolves a user [`AudioPackPreference`] (`Auto`, `Sh2eeLossless`, `Ps2Original`) into a concrete
    /// active source (`Sh2eeLossless` or `Ps2Original`) based on detected filesystem assets:
    /// - If only SH2:EE is present: returns `Sh2eeLossless`.
    /// - If only PS2/PC is present: returns `Ps2Original`.
    /// - If both (or neither) are present:
    ///   - `Auto` or `Sh2eeLossless` -> `Sh2eeLossless`
    ///   - `Ps2Original` -> `Ps2Original`
    pub fn resolve_source(&self, preference: AudioPackPreference) -> AudioPackPreference {
        match (self.sh2ee_lossless_present, self.ps2_original_present) {
            (true, false) => AudioPackPreference::Sh2eeLossless,
            (false, true) => AudioPackPreference::Ps2Original,
            _ => match preference {
                AudioPackPreference::Auto | AudioPackPreference::Sh2eeLossless => {
                    AudioPackPreference::Sh2eeLossless
                }
                AudioPackPreference::Ps2Original => AudioPackPreference::Ps2Original,
            },
        }
    }

    /// User-facing status description for the Options Menu and HUD.
    pub fn status_label(&self) -> &'static str {
        match (self.sh2ee_lossless_present, self.ps2_original_present) {
            (true, true) => "Dual Packs Detected (SH2:EE + PS2)",
            (true, false) => "SH2:EE Lossless Only",
            (false, true) => "Original PS2 / PC Only",
            (false, false) => "No Audio Packs Detected",
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
    /// Selected audio output device name (None = System Default).
    pub selected_output_device: Option<String>,
    /// Mute game audio when the window loses focus.
    pub mute_in_background: bool,
    /// Preferred BGM & SFX audio asset source pack (`Auto`, `Sh2eeLossless`, `Ps2Original`).
    #[serde(default, alias = "audio_pack_preference")]
    pub audio_pack_source: AudioPackPreference,
    /// Restore cut / unused audio sequences (`SH2-62`), such as Mary's voiceover reading her letter
    /// to Laura in the Lakeview Hotel restaurant (`SCENE_45` / `PlayUnusedAudio.cpp`), Apartment Closet
    /// bonus audio (`sankaku_toujyou_2.wav`), and Lying Figure street footsteps (`machi.wav`).
    /// Defaults to `false` for strict vanilla parity.
    #[serde(default)]
    pub enable_restored_cut_audio: bool,
}

impl Default for AudioOptions {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            bgm_volume: 0.85,
            sfx_volume: 0.9,
            dialogue_volume: 1.0,
            speaker_config: SpeakerConfiguration::default(),
            selected_output_device: None,
            mute_in_background: true,
            audio_pack_source: AudioPackSource::default(),
            enable_restored_cut_audio: false,
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
    /// Allow saving anywhere from the pause menu (Retail PC behavior) vs Red Squares only (PS2 original behavior).
    pub save_anywhere: bool,
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
    #[serde(default)]
    pub debug: DebugOptions,
}

impl AppConfig {
    /// Serialize and write configuration to disk atomically.
    ///
    /// Writes to a temporary `.tmp` sibling file, flushes to disk, and renames over the target path.
    /// This prevents configuration corruption or zero-byte file writes if the application terminates
    /// or crashes during saving.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let serialized = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let target = path.as_ref();
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = target.with_extension(format!(
            "tmp.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(&tmp_path, serialized)?;
        fs::rename(&tmp_path, target)?;
        Ok(())
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

        // Verify SH2-56 FrameRateCap cycle: 60 -> 120 -> 144 -> 240 -> Uncapped -> 30 -> 60
        let cap0 = FrameRateCap::default();
        assert_eq!(cap0, FrameRateCap::Fps60);
        assert_eq!(cap0.target_fps(), Some(60));
        let cap1 = cap0.next();
        assert_eq!(cap1, FrameRateCap::Fps120);
        assert_eq!(cap1.target_fps(), Some(120));
        let cap2 = cap1.next();
        assert_eq!(cap2, FrameRateCap::Fps144);
        assert_eq!(cap2.target_fps(), Some(144));
        let cap3 = cap2.next();
        assert_eq!(cap3, FrameRateCap::Fps240);
        assert_eq!(cap3.target_fps(), Some(240));
        let cap4 = cap3.next();
        assert_eq!(cap4, FrameRateCap::Uncapped);
        assert_eq!(cap4.target_fps(), None);
        let cap5 = cap4.next();
        assert_eq!(cap5, FrameRateCap::Fps30);
        assert_eq!(cap5.target_fps(), Some(30));
        assert_eq!(cap5.next(), FrameRateCap::Fps60);

        // Verify SH2-59 AudioPackPreference cycle: Auto -> Sh2eeLossless -> Ps2Original -> Auto
        let ap0 = AudioPackPreference::default();
        assert_eq!(ap0, AudioPackPreference::Auto);
        let ap1 = ap0.next();
        assert_eq!(ap1, AudioPackPreference::Sh2eeLossless);
        let ap2 = ap1.next();
        assert_eq!(ap2, AudioPackPreference::Ps2Original);
        assert_eq!(ap2.next(), AudioPackPreference::Auto);
        assert_eq!(ap0.prev(), AudioPackPreference::Ps2Original);

        // Verify AudioPackDetectionStatus resolution logic
        let both = AudioPackDetectionStatus {
            sh2ee_lossless_present: true,
            ps2_original_present: true,
            sh2ee_extra_present: true,
        };
        assert!(both.both_detected());
        assert_eq!(
            both.resolve_source(AudioPackPreference::Auto),
            AudioPackPreference::Sh2eeLossless
        );
        assert_eq!(
            both.resolve_source(AudioPackPreference::Ps2Original),
            AudioPackPreference::Ps2Original
        );

        let only_ps2 = AudioPackDetectionStatus {
            sh2ee_lossless_present: false,
            ps2_original_present: true,
            sh2ee_extra_present: false,
        };
        assert!(!only_ps2.both_detected());
        assert_eq!(
            only_ps2.resolve_source(AudioPackPreference::Auto),
            AudioPackPreference::Ps2Original
        );
        assert_eq!(
            only_ps2.resolve_source(AudioPackPreference::Sh2eeLossless),
            AudioPackPreference::Ps2Original
        );

        let only_ee = AudioPackDetectionStatus {
            sh2ee_lossless_present: true,
            ps2_original_present: false,
            sh2ee_extra_present: true,
        };
        assert!(!only_ee.both_detected());
        assert_eq!(
            only_ee.resolve_source(AudioPackPreference::Auto),
            AudioPackPreference::Sh2eeLossless
        );
        assert_eq!(
            only_ee.resolve_source(AudioPackPreference::Ps2Original),
            AudioPackPreference::Sh2eeLossless
        );
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
