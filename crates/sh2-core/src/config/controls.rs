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

/// PS2 Controller Config preset (`Item/otn_option.c` `key_type1` `0x00213610`,
/// `key_type2` `0x002136b0`, `key_type3` `0x00213750`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ControllerPreset {
    /// Preset Type 1 (`key_type1` `0x00213610`).
    #[default]
    Type1 = 1,
    /// Preset Type 2 (`key_type2` `0x002136b0`).
    Type2 = 2,
    /// Preset Type 3 (`key_type3` `0x00213750`).
    Type3 = 3,
}

impl ControllerPreset {
    /// User-facing label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Type1 => "Type 1",
            Self::Type2 => "Type 2",
            Self::Type3 => "Type 3",
        }
    }

    /// Returns the 13 action bitmasks from `key_type1/2/3` (`SLUS_202.28` `0x00213610..0x00213750`):
    /// `[Enter, Cancel, Skip, StepRight, StepLeft, Action, RunGuard, Light, Item, SearchView, Aim, Map, Pause]`.
    #[must_use]
    pub const fn bindings(self) -> [u32; 13] {
        match self {
            Self::Type1 => [
                0x2000,  // Enter (Circle)
                0x4000,  // Cancel (Cross)
                0x0004,  // Skip (L1)
                0x20000, // Step right (L3)
                0x10000, // Step left (Select)
                0x2000,  // Action (Circle)
                0x4000,  // Run/Guard (Cross)
                0x8000,  // Light (Square)
                0x000c,  // Item (L1 + R1)
                0x40000, // Search View (R3)
                0x80000, // Aim (Start)
                0x1000,  // Map (Triangle)
                0x0000,  // Pause
            ],
            Self::Type2 => [
                0x4000,  // Enter (Cross)
                0x8000,  // Cancel (Square)
                0x0004,  // Skip (L1)
                0x20000, // Step right (L3)
                0x10000, // Step left (Select)
                0x4000,  // Action (Cross)
                0x8000,  // Run/Guard (Square)
                0x2000,  // Light (Circle)
                0x000c,  // Item (L1 + R1)
                0x40000, // Search View (R3)
                0x80000, // Aim (Start)
                0x1000,  // Map (Triangle)
                0x0000,  // Pause
            ],
            Self::Type3 => [
                0xa004,  // Enter (Square + Circle + L1)
                0x4000,  // Cancel (Cross)
                0x0004,  // Skip (L1)
                0x80000, // Step right (Start)
                0x40000, // Step left (R3)
                0xa000,  // Action (Square + Circle)
                0x4000,  // Run/Guard (Cross)
                0x1000,  // Light (Triangle)
                0x0004,  // Item (L1)
                0x10000, // Search View (Select)
                0x20000, // Aim (L3)
                0x0008,  // Map (R1)
                0x0000,  // Pause
            ],
        }
    }
}

/// 13-action PS2 remappable button assignment table (`Item/otn_option.c` `config_main` `0x00213a50`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ps2ControllerBindings {
    /// Active preset (`Type1`, `Type2`, `Type3`).
    pub preset: ControllerPreset,
    /// Per-action PS2 pad bitmasks matching `key_type1/2/3` (`0x00213610..0x00213750`).
    pub actions: [u32; 13],
}

impl Default for Ps2ControllerBindings {
    fn default() -> Self {
        Self::from_preset(ControllerPreset::Type1)
    }
}

impl Ps2ControllerBindings {
    /// Action row labels matching `config_main` (`0x00213a50`, msgs `0x0a..0x16`).
    pub const ACTION_LABELS: [&'static str; 13] = [
        "Enter",
        "Cancel",
        "Skip",
        "Step right",
        "Step left",
        "Action",
        "Run / Guard",
        "Light",
        "Item",
        "Search View",
        "Aim",
        "Map",
        "Pause",
    ];

    /// Whether an action row is mandatory (cannot be cleared to `0`, `key_conf` `param_3 == 1`).
    pub const REQUIRED: [bool; 13] = [
        true,  // Enter
        true,  // Cancel
        false, // Skip
        false, // Step right
        false, // Step left
        true,  // Action
        false, // Run/Guard
        false, // Light
        true,  // Item
        false, // Search View
        true,  // Aim
        false, // Map
        false, // Pause
    ];

    /// Constructs bindings initialized to a PS2 preset (`key_type1/2/3`).
    #[must_use]
    pub const fn from_preset(preset: ControllerPreset) -> Self {
        Self {
            preset,
            actions: preset.bindings(),
        }
    }

    /// Applies a preset (`Type1`, `Type2`, or `Type3`).
    pub fn apply_preset(&mut self, preset: ControllerPreset) {
        self.preset = preset;
        self.actions = preset.bindings();
    }

    /// Toggles `button_bit` on `action_idx` (`0..13`) with `key_conf` (`0x002137f0`) and
    /// `key_conf_check` (`0x00213840`) conflict resolution so required actions never become `0`.
    pub fn toggle_button(&mut self, action_idx: usize, button_bit: u32) -> bool {
        if action_idx >= 13 || button_bit == 0 {
            return false;
        }
        let prev = self.actions[action_idx];
        let toggled = prev ^ button_bit;
        if toggled == 0 && Self::REQUIRED[action_idx] {
            return false;
        }

        // Conflict group: rows 0..=1 (Enter vs Cancel) vs rows 3..=12 (10 gameplay actions in `key_conf_check`).
        let group_range = if action_idx <= 1 {
            0..=1
        } else if action_idx >= 3 {
            3..=12
        } else {
            2..=2
        };

        // Adding a bit strips it from conflicting slots unless that would empty a required slot.
        if (toggled & button_bit) != 0 {
            for other in group_range.clone() {
                if other == action_idx {
                    continue;
                }
                if (self.actions[other] & button_bit) != 0 {
                    let cleared = self.actions[other] & !button_bit;
                    if cleared == 0 && Self::REQUIRED[other] {
                        return false;
                    }
                }
            }
            for other in group_range {
                if other != action_idx {
                    self.actions[other] &= !button_bit;
                }
            }
        }

        self.actions[action_idx] = toggled;
        true
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
    /// PS2 13-action controller preset & remapping table (`Item/otn_option.c` `config_main` `0x00213a50`).
    #[serde(default)]
    pub ps2_bindings: Ps2ControllerBindings,
    /// Native-upgrade L2/R2 analog trigger rowing on Toluca Lake (`SH2-80`). Disabled in strict 1:1 Vanilla PS2 mode.
    #[serde(default)]
    pub enable_analog_trigger_rowing: bool,
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
            ps2_bindings: Ps2ControllerBindings::default(),
            enable_analog_trigger_rowing: false,
        }
    }
}
