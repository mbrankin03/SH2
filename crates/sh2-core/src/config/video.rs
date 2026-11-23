//! Video, graphics, shadow, lighting, and outdoor streaming configuration options.

use serde::{Deserialize, Serialize};

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

/// 3-tier World (`.map`) & Character (`.mdl`) texture quality setting (`SH2-92`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WorldTextureQuality {
    /// 4K AI Super-Resolution Master Pack (`realesr-general-wdn-x4v3` + PBR Detail Maps),
    /// falling back to `Sh2eeHd` -> `OriginalPs2`.
    #[default]
    Ai4kMaster,
    /// Silent Hill 2: Enhanced Edition HD texture pack (`sh2e/bg/` and `sh2e/chr/`).
    Sh2eeHd,
    /// Original PS2 / Retail PC textures (`data/bg/` and `data/chr/`).
    OriginalPs2,
}

impl WorldTextureQuality {
    /// User-facing label string.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ai4kMaster => "4K AI Master + Detail Maps",
            Self::Sh2eeHd => "SH2:EE HD",
            Self::OriginalPs2 => "Original PS2 / PC",
        }
    }

    /// Cycle to next texture quality tier.
    pub fn next(&self) -> Self {
        match self {
            Self::Ai4kMaster => Self::Sh2eeHd,
            Self::Sh2eeHd => Self::OriginalPs2,
            Self::OriginalPs2 => Self::Ai4kMaster,
        }
    }

    /// Cycle to previous texture quality tier.
    pub fn prev(&self) -> Self {
        match self {
            Self::Ai4kMaster => Self::OriginalPs2,
            Self::Sh2eeHd => Self::Ai4kMaster,
            Self::OriginalPs2 => Self::Sh2eeHd,
        }
    }
}

/// Shadow rendering backend mode (`SH2-81`).
///
/// Allows purist players to select authentic 1:1 PS2 `.kg1`/`.kg2` stencil & drop shadows,
/// SH2:EE soft-opacity stencil shadows, or Native Enhanced GPU PCSS contact-hardening soft
/// self-shadows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShadowQualityMode {
    /// Authentic PS2 `.kg1`/`.kg2` two-pass stencil shadow volumes + lit-room drop shadows (`sh2shd_shadow_model.c`).
    ClassicPs2Stencil,
    /// SH2:EE per-room opacity (`170/255` default) + `~33`-frame fade-in soft stencil shadows (`EnableSoftShadows`).
    EnhancedEditionSoft,
    /// Native `wgpu` Percentage-Closer Soft Shadows (PCSS) with contact-hardening penumbras & true self-shadowing.
    #[default]
    NativePcss,
}

impl ShadowQualityMode {
    /// User-facing label for the Video Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClassicPs2Stencil => "Classic PS2 (.kg1/.kg2 Stencil)",
            Self::EnhancedEditionSoft => "SH2:EE Soft Stencil",
            Self::NativePcss => "Native PCSS (Soft Self-Shadow)",
        }
    }

    /// Cycles to the next shadow mode.
    pub const fn next(self) -> Self {
        match self {
            Self::ClassicPs2Stencil => Self::EnhancedEditionSoft,
            Self::EnhancedEditionSoft => Self::NativePcss,
            Self::NativePcss => Self::ClassicPs2Stencil,
        }
    }

    /// Cycles to the previous shadow mode.
    pub const fn prev(self) -> Self {
        match self {
            Self::ClassicPs2Stencil => Self::NativePcss,
            Self::EnhancedEditionSoft => Self::ClassicPs2Stencil,
            Self::NativePcss => Self::EnhancedEditionSoft,
        }
    }
}

/// Room & character lighting parity profile (`SH2-81`).
///
/// Controls whether SH2:EE lighting patches (`PS2FlashlightBrightness`, `HalogenLightFix`,
/// `CatacombsMeatRoomFix`, `EnemyRevealLighting`, `SpecularFix`) are applied over raw PS2
/// `DrawEnvData` tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LightingProfile {
    /// Strict 1:1 original PS2 `DrawEnvData` lighting without SH2:EE overrides.
    Ps2Authentic,
    /// PS2 `DrawEnvData` plus SH2:EE lighting parity fixes (`EnhancedFlashlight`, `CatacombsMeatRoom`, `SpecularFix`).
    EnhancedEdition,
    /// SH2:EE lighting parity fixes combined with Native Enhanced PCSS self-shadowing.
    #[default]
    NativeEnhanced,
}

impl LightingProfile {
    /// User-facing label for the Video Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ps2Authentic => "PS2 Authentic (1:1 Original)",
            Self::EnhancedEdition => "SH2:EE (Lighting Fixes)",
            Self::NativeEnhanced => "Native Enhanced (Fixes + PCSS)",
        }
    }

    /// Cycles to the next lighting profile.
    pub const fn next(self) -> Self {
        match self {
            Self::Ps2Authentic => Self::EnhancedEdition,
            Self::EnhancedEdition => Self::NativeEnhanced,
            Self::NativeEnhanced => Self::Ps2Authentic,
        }
    }

    /// Cycles to the previous lighting profile.
    pub const fn prev(self) -> Self {
        match self {
            Self::Ps2Authentic => Self::NativeEnhanced,
            Self::EnhancedEdition => Self::Ps2Authentic,
            Self::NativeEnhanced => Self::EnhancedEdition,
        }
    }
}

/// Outdoor `.map` tile residency & streaming mode (`SH2-78`).
///
/// Allows purist players to select strict 1:1 PS2 `loadbg_2x2` (`0x26f910` / `0x270ad0`)
/// `2x2` quadrant streaming with `7,500..17,500 mm` bands, or native `wgpu` `5x5` /
/// Whole-Town GPU residency (`< 45 MB` VRAM) eliminating tile-boundary stutters and pop-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutdoorStreamingMode {
    /// Authentic 1:1 PS2 `loadbg_2x2` 2x2 quadrant streaming (`SLUS_202.28` `0x270dd0`).
    ClassicPs2LoadBg2x2,
    /// Native `5x5` neighborhood GPU residency (25 tiles around the player's current cell).
    Native5x5Grid,
    /// Native Whole-Town GPU residency (pre-streams all tiles in the outdoor area onto GPU buffers).
    #[default]
    NativeWholeTown,
}

impl OutdoorStreamingMode {
    /// User-facing label for the Video Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClassicPs2LoadBg2x2 => "Classic PS2 (2x2 LoadBg)",
            Self::Native5x5Grid => "Native 5x5 Tile Grid",
            Self::NativeWholeTown => "Native Whole-Town Residency",
        }
    }

    /// Cycles to the next outdoor streaming mode.
    pub const fn next(self) -> Self {
        match self {
            Self::ClassicPs2LoadBg2x2 => Self::Native5x5Grid,
            Self::Native5x5Grid => Self::NativeWholeTown,
            Self::NativeWholeTown => Self::ClassicPs2LoadBg2x2,
        }
    }

    /// Cycles to the previous outdoor streaming mode.
    pub const fn prev(self) -> Self {
        match self {
            Self::ClassicPs2LoadBg2x2 => Self::NativeWholeTown,
            Self::Native5x5Grid => Self::ClassicPs2LoadBg2x2,
            Self::NativeWholeTown => Self::Native5x5Grid,
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
    /// 3-tier World (`.map`) & Character (`.mdl`) texture quality (`SH2-92`).
    #[serde(default)]
    pub texture_quality: WorldTextureQuality,
    /// Shadow rendering mode: Classic PS2 `.kg1`/`.kg2` stencil, SH2:EE soft stencil, or Native PCSS (`SH2-81`).
    #[serde(default)]
    pub shadow_quality: ShadowQualityMode,
    /// Room & character lighting parity profile (`SH2-81`).
    #[serde(default)]
    pub lighting_profile: LightingProfile,
    /// Outdoor `.map` tile residency mode: Classic PS2 `2x2`, Native `5x5`, or Whole-Town (`SH2-78`).
    #[serde(default)]
    pub outdoor_streaming_mode: OutdoorStreamingMode,
    /// Modern ACES filmic tonemapping (`native-upgrade`, `SH2-136`). Disabled in strict 1:1 Vanilla PS2 mode.
    #[serde(default)]
    pub aces_tonemapping: bool,
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
            texture_quality: WorldTextureQuality::default(),
            shadow_quality: ShadowQualityMode::default(),
            lighting_profile: LightingProfile::default(),
            outdoor_streaming_mode: OutdoorStreamingMode::default(),
            aces_tonemapping: true,
        }
    }
}
