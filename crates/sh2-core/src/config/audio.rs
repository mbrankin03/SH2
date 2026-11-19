//! Audio volume, speaker layout, HRTF spatialization, and audio pack configuration options.

use serde::{Deserialize, Serialize};
use std::path::Path;

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

/// 3D point-source SFX spatialization mode (`SH2-84`).
///
/// Selects between authentic PlayStation 2 horizontal stereo panning (`SeCallPosDistanceF`
/// `0x5155E0` / `0x517D10`) and real-time Binaural HRTF headphone processing (Woodworth
/// spherical-head ITD, head-shadow ILD, and pinna elevation/front-back cues).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpatialAudioMode {
    /// Classic PS2 horizontal azimuth stereo panning (`0x517D10`).
    #[default]
    ClassicPs2Stereo,
    /// Binaural 3D HRTF spatialization for headphones and IEMs (`SH2-84`).
    BinauralHrtf,
}

impl SpatialAudioMode {
    /// User-facing label for the Audio Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClassicPs2Stereo => "Classic PS2 Stereo",
            Self::BinauralHrtf => "Binaural HRTF (3D Headphones)",
        }
    }

    /// Returns `true` when Binaural HRTF 3D spatialization is enabled.
    #[inline]
    pub const fn is_binaural_hrtf(self) -> bool {
        matches!(self, Self::BinauralHrtf)
    }

    /// Toggles between `ClassicPs2Stereo` and `BinauralHrtf`.
    #[inline]
    pub const fn toggle(self) -> Self {
        match self {
            Self::ClassicPs2Stereo => Self::BinauralHrtf,
            Self::BinauralHrtf => Self::ClassicPs2Stereo,
        }
    }

    /// Cycles to the next spatialization mode.
    #[inline]
    pub const fn next(self) -> Self {
        self.toggle()
    }

    /// Cycles to the previous spatialization mode.
    #[inline]
    pub const fn prev(self) -> Self {
        self.toggle()
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
    /// 3D SFX headphone spatialization mode (`ClassicPs2Stereo` vs `BinauralHrtf`) (`SH2-84`).
    #[serde(default)]
    pub spatial_audio_mode: SpatialAudioMode,
    /// Real-time `.cld` 3D line-of-sight wall, door, and floor-slab acoustic occlusion (`SH2-84`).
    /// Defaults to `false` (`OFF (Original PS2)`) so players receive strict 1:1 PlayStation 2
    /// `SeCallPosDistanceF` (`0x00192bc0` / `0x5155E0`) unoccluded behavior unless enabled.
    #[serde(default)]
    pub enable_acoustic_occlusion: bool,
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
            spatial_audio_mode: SpatialAudioMode::default(),
            enable_acoustic_occlusion: false,
            selected_output_device: None,
            mute_in_background: true,
            audio_pack_source: AudioPackSource::default(),
            enable_restored_cut_audio: false,
        }
    }
}
