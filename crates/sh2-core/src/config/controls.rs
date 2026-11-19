//! Controller, tank/2D movement, and modern 3D camera configuration options.

use serde::{Deserialize, Serialize};

pub use crate::modern_camera::{
    CameraCyclePreset, GameplayCameraMode, OtsShoulderSide, ReticleMode,
};

fn default_first_person_fov_deg() -> u32 {
    75
}

fn default_true() -> bool {
    true
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

/// Control options matching SH2 / SH2:EE controls config plus modern 3D camera settings (`SH2-93`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlOptions {
    /// 3D Tank controls (authentic PS2) vs 2D camera-relative directional controls.
    pub tank_controls: bool,
    /// Button prompt visual theme.
    pub glyph_style: ButtonGlyphStyle,
    /// Analog stick deadzone ratio (0.0 to 0.5).
    pub stick_deadzone: f32,
    /// Stick sensitivity multiplier (0.5 to 3.0).
    pub stick_sensitivity: f32,
    /// Vibration / haptic feedback intensity (0.0 to 1.0).
    pub vibration_intensity: f32,
    /// Default gameplay camera mode (`ClassicPs2Rail`, `OverTheShoulder`, or `FirstPerson`) (`SH2-93`).
    #[serde(default)]
    pub default_camera_mode: GameplayCameraMode,
    /// Subset of camera modes cycled live via `R3` (`AllThree`, `ClassicAndOts`, etc.) (`SH2-93`).
    #[serde(default)]
    pub camera_cycle_preset: CameraCyclePreset,
    /// Preferred shoulder alignment in Over-the-Shoulder 3D mode (`Right` or `Left`) (`SH2-93`).
    #[serde(default)]
    pub ots_shoulder: OtsShoulderSide,
    /// Diegetic aiming reticle display mode (`AimOnly`, `AlwaysSubtle`, or `Off`) (`SH2-93`).
    #[serde(default)]
    pub ots_reticle: ReticleMode,
    /// Vertical field-of-view in degrees for First-Person mode (`55..=100`, default `75`) (`SH2-93`).
    #[serde(default = "default_first_person_fov_deg")]
    pub first_person_fov_deg: u32,
    /// Enable subtle stride head-bob and footstep sway in First-Person mode (`SH2-93`).
    #[serde(default = "default_true")]
    pub first_person_head_bob: bool,
}

impl Default for ControlOptions {
    fn default() -> Self {
        Self {
            tank_controls: true,
            glyph_style: ButtonGlyphStyle::default(),
            stick_deadzone: 0.15,
            stick_sensitivity: 1.0,
            vibration_intensity: 1.0,
            default_camera_mode: GameplayCameraMode::default(),
            camera_cycle_preset: CameraCyclePreset::default(),
            ots_shoulder: OtsShoulderSide::default(),
            ots_reticle: ReticleMode::default(),
            first_person_fov_deg: default_first_person_fov_deg(),
            first_person_head_bob: true,
        }
    }
}
