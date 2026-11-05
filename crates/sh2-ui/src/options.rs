//! In-Game Options Menu replicating SH2:EE (Enhanced Edition) advanced options architecture.
//! References: `Patches/CustomAdvancedOptions.cpp`, `Patches/OptionsMenuTweaks.cpp`.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use sh2_core::config::AppConfig;

/// High-level Options category tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptionsTab {
    #[default]
    Video = 0,
    Audio = 1,
    Controls = 2,
    Game = 3,
}

impl OptionsTab {
    pub const ALL: [OptionsTab; 4] = [
        OptionsTab::Video,
        OptionsTab::Audio,
        OptionsTab::Controls,
        OptionsTab::Game,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "VIDEO & GRAPHICS",
            Self::Audio => "AUDIO & SOUND",
            Self::Controls => "CONTROLS & INPUT",
            Self::Game => "GAMEPLAY & PUZZLES",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Video => Self::Audio,
            Self::Audio => Self::Controls,
            Self::Controls => Self::Game,
            Self::Game => Self::Video,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Video => Self::Game,
            Self::Audio => Self::Video,
            Self::Controls => Self::Audio,
            Self::Game => Self::Controls,
        }
    }
}

/// Sound cue produced during options manipulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsSoundCue {
    None,
    CursorMove,
    ValueChange,
    ConfirmSave,
    CancelDiscard,
    /// Triggered when the user selects the interactive FMV Cutscene Viewer from the Options menu.
    LaunchFmvViewer,
}

/// Interactive Options Menu controller.
#[derive(Debug, Clone)]
pub struct OptionsMenu {
    pub active_tab: OptionsTab,
    pub selected_index: usize,
    /// Working copy of configuration being edited.
    pub draft_config: AppConfig,
    /// Clean copy before current modifications.
    pub original_config: AppConfig,
    /// Has any setting been modified compared to original?
    pub is_dirty: bool,
}

impl OptionsMenu {
    pub fn new(config: AppConfig) -> Self {
        Self {
            active_tab: OptionsTab::Video,
            selected_index: 0,
            draft_config: config.clone(),
            original_config: config,
            is_dirty: false,
        }
    }

    /// Number of items in the currently active tab.
    pub fn item_count(&self) -> usize {
        match self.active_tab {
            OptionsTab::Video => 10,
            OptionsTab::Audio => 6,
            OptionsTab::Controls => 5,
            OptionsTab::Game => 5,
        }
    }

    /// Returns true if the currently selected row is the interactive FMV Cutscene Viewer launcher.
    pub fn is_fmv_viewer_selected(&self) -> bool {
        self.active_tab == OptionsTab::Video && self.selected_index == 9
    }

    /// Retrieve the title and string value of an item in the active tab.
    pub fn item_display(&self, index: usize) -> Option<(&'static str, String)> {
        match self.active_tab {
            OptionsTab::Video => match index {
                0 => Some((
                    "Resolution",
                    self.draft_config.video.resolution.label().to_string(),
                )),
                1 => Some((
                    "Display Mode",
                    self.draft_config.video.display_mode.label().to_string(),
                )),
                2 => Some((
                    "V-Sync",
                    if self.draft_config.video.vsync {
                        "Enabled".into()
                    } else {
                        "Disabled".into()
                    },
                )),
                3 => Some((
                    "Frame Rate Cap",
                    self.draft_config.video.framerate_cap.label().to_string(),
                )),
                4 => Some((
                    "Fog Quality",
                    self.draft_config.video.fog_quality.label().to_string(),
                )),
                5 => Some((
                    "Film Grain / Noise",
                    format!("{}%", self.draft_config.video.grain_intensity),
                )),
                6 => Some((
                    "Volumetric Lighting",
                    if self.draft_config.video.volumetric_light {
                        "ON".into()
                    } else {
                        "OFF".into()
                    },
                )),
                7 => Some((
                    "Lens Flare",
                    if self.draft_config.video.lens_flare {
                        "ON".into()
                    } else {
                        "OFF".into()
                    },
                )),
                8 => Some((
                    "Depth of Field",
                    if self.draft_config.video.depth_of_field {
                        "ON".into()
                    } else {
                        "OFF".into()
                    },
                )),
                9 => Some((
                    "FMV Cutscene Viewer",
                    "[Press Enter / Right to Launch]".into(),
                )),
                _ => None,
            },
            OptionsTab::Audio => match index {
                0 => Some((
                    "Master Volume",
                    format!("{:.0}%", self.draft_config.audio.master_volume * 100.0),
                )),
                1 => Some((
                    "BGM Volume",
                    format!("{:.0}%", self.draft_config.audio.bgm_volume * 100.0),
                )),
                2 => Some((
                    "SFX Volume",
                    format!("{:.0}%", self.draft_config.audio.sfx_volume * 100.0),
                )),
                3 => Some((
                    "Dialogue Volume",
                    format!("{:.0}%", self.draft_config.audio.dialogue_volume * 100.0),
                )),
                4 => Some((
                    "Speaker Config",
                    self.draft_config.audio.speaker_config.label().to_string(),
                )),
                5 => Some((
                    "Mute in Background",
                    if self.draft_config.audio.mute_in_background {
                        "YES".into()
                    } else {
                        "NO".into()
                    },
                )),
                _ => None,
            },
            OptionsTab::Controls => match index {
                0 => Some((
                    "Movement Style",
                    if self.draft_config.controls.tank_controls {
                        "3D Tank (Authentic)".into()
                    } else {
                        "2D Directional (Modern)".into()
                    },
                )),
                1 => Some((
                    "Button Prompt Glyphs",
                    self.draft_config.controls.glyph_style.label().to_string(),
                )),
                2 => Some((
                    "Stick Deadzone",
                    format!("{:.0}%", self.draft_config.controls.stick_deadzone * 100.0),
                )),
                3 => Some((
                    "Stick Sensitivity",
                    format!("{:.1}x", self.draft_config.controls.stick_sensitivity),
                )),
                4 => Some((
                    "Vibration Intensity",
                    format!(
                        "{:.0}%",
                        self.draft_config.controls.vibration_intensity * 100.0
                    ),
                )),
                _ => None,
            },
            OptionsTab::Game => match index {
                0 => Some((
                    "Action Difficulty",
                    self.draft_config.game.action_difficulty.label().to_string(),
                )),
                1 => Some((
                    "Riddle Difficulty",
                    self.draft_config.game.riddle_difficulty.label().to_string(),
                )),
                2 => Some((
                    "Subtitles",
                    if self.draft_config.game.subtitles {
                        "ON".into()
                    } else {
                        "OFF".into()
                    },
                )),
                3 => Some((
                    "Language",
                    self.draft_config.game.language.label().to_string(),
                )),
                4 => Some((
                    "Bullet Multiplier",
                    self.draft_config.game.bullet_multiplier.label().to_string(),
                )),
                _ => None,
            },
        }
    }

    /// Navigate up / down within active tab.
    pub fn navigate_vertical(&mut self, delta: i32) -> OptionsSoundCue {
        let count = self.item_count();
        if count == 0 {
            return OptionsSoundCue::None;
        }

        if delta < 0 {
            if self.selected_index == 0 {
                self.selected_index = count - 1;
            } else {
                self.selected_index -= 1;
            }
        } else if delta > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
        OptionsSoundCue::CursorMove
    }

    /// Switch active tab left or right.
    pub fn switch_tab(&mut self, next: bool) -> OptionsSoundCue {
        self.active_tab = if next {
            self.active_tab.next()
        } else {
            self.active_tab.prev()
        };
        self.selected_index = 0;
        OptionsSoundCue::CursorMove
    }

    /// Adjust value of currently selected setting left (-1) or right (+1).
    pub fn adjust_value(&mut self, direction: i32) -> OptionsSoundCue {
        let forward = direction >= 0;

        match self.active_tab {
            OptionsTab::Video => match self.selected_index {
                0 => {
                    self.draft_config.video.resolution = if forward {
                        self.draft_config.video.resolution.next()
                    } else {
                        self.draft_config.video.resolution.prev()
                    };
                }
                1 => {
                    self.draft_config.video.display_mode = if forward {
                        self.draft_config.video.display_mode.next()
                    } else {
                        self.draft_config.video.display_mode.prev()
                    };
                }
                2 => self.draft_config.video.vsync = !self.draft_config.video.vsync,
                3 => {
                    self.draft_config.video.framerate_cap = if forward {
                        self.draft_config.video.framerate_cap.next()
                    } else {
                        self.draft_config.video.framerate_cap.prev()
                    };
                }
                4 => {
                    self.draft_config.video.fog_quality =
                        self.draft_config.video.fog_quality.toggle();
                }
                5 => {
                    let step = if forward { 5 } else { -5 };
                    self.draft_config.video.grain_intensity =
                        (self.draft_config.video.grain_intensity as i32 + step).clamp(0, 100)
                            as u32;
                }
                6 => {
                    self.draft_config.video.volumetric_light =
                        !self.draft_config.video.volumetric_light;
                }
                7 => {
                    self.draft_config.video.lens_flare = !self.draft_config.video.lens_flare;
                }
                8 => {
                    self.draft_config.video.depth_of_field =
                        !self.draft_config.video.depth_of_field;
                }
                9 => return OptionsSoundCue::LaunchFmvViewer,
                _ => return OptionsSoundCue::None,
            },
            OptionsTab::Audio => match self.selected_index {
                0 => {
                    let step = if forward { 0.05 } else { -0.05 };
                    self.draft_config.audio.master_volume =
                        (self.draft_config.audio.master_volume + step).clamp(0.0, 1.0);
                }
                1 => {
                    let step = if forward { 0.05 } else { -0.05 };
                    self.draft_config.audio.bgm_volume =
                        (self.draft_config.audio.bgm_volume + step).clamp(0.0, 1.0);
                }
                2 => {
                    let step = if forward { 0.05 } else { -0.05 };
                    self.draft_config.audio.sfx_volume =
                        (self.draft_config.audio.sfx_volume + step).clamp(0.0, 1.0);
                }
                3 => {
                    let step = if forward { 0.05 } else { -0.05 };
                    self.draft_config.audio.dialogue_volume =
                        (self.draft_config.audio.dialogue_volume + step).clamp(0.0, 1.0);
                }
                4 => {
                    self.draft_config.audio.speaker_config = if forward {
                        self.draft_config.audio.speaker_config.next()
                    } else {
                        self.draft_config.audio.speaker_config.prev()
                    };
                }
                5 => {
                    self.draft_config.audio.mute_in_background =
                        !self.draft_config.audio.mute_in_background;
                }
                _ => return OptionsSoundCue::None,
            },
            OptionsTab::Controls => match self.selected_index {
                0 => {
                    self.draft_config.controls.tank_controls =
                        !self.draft_config.controls.tank_controls;
                }
                1 => {
                    self.draft_config.controls.glyph_style = if forward {
                        self.draft_config.controls.glyph_style.next()
                    } else {
                        self.draft_config.controls.glyph_style.prev()
                    };
                }
                2 => {
                    let step = if forward { 0.05 } else { -0.05 };
                    self.draft_config.controls.stick_deadzone =
                        (self.draft_config.controls.stick_deadzone + step).clamp(0.0, 0.5);
                }
                3 => {
                    let step = if forward { 0.1 } else { -0.1 };
                    self.draft_config.controls.stick_sensitivity =
                        (self.draft_config.controls.stick_sensitivity + step).clamp(0.5, 3.0);
                }
                4 => {
                    let step = if forward { 0.1 } else { -0.1 };
                    self.draft_config.controls.vibration_intensity =
                        (self.draft_config.controls.vibration_intensity + step).clamp(0.0, 1.0);
                }
                _ => return OptionsSoundCue::None,
            },
            OptionsTab::Game => match self.selected_index {
                0 => {
                    self.draft_config.game.action_difficulty = if forward {
                        self.draft_config.game.action_difficulty.next()
                    } else {
                        self.draft_config.game.action_difficulty.prev()
                    };
                }
                1 => {
                    self.draft_config.game.riddle_difficulty = if forward {
                        self.draft_config.game.riddle_difficulty.next()
                    } else {
                        self.draft_config.game.riddle_difficulty.prev()
                    };
                }
                2 => {
                    self.draft_config.game.subtitles = !self.draft_config.game.subtitles;
                }
                3 => {
                    self.draft_config.game.language = if forward {
                        self.draft_config.game.language.next()
                    } else {
                        self.draft_config.game.language.prev()
                    };
                }
                4 => {
                    self.draft_config.game.bullet_multiplier = if forward {
                        self.draft_config.game.bullet_multiplier.next()
                    } else {
                        self.draft_config.game.bullet_multiplier.prev()
                    };
                }
                _ => return OptionsSoundCue::None,
            },
        }

        self.is_dirty = self.draft_config != self.original_config;
        OptionsSoundCue::ValueChange
    }

    /// Save draft modifications and commit as new baseline.
    pub fn commit_changes(&mut self) -> (AppConfig, OptionsSoundCue) {
        self.original_config = self.draft_config.clone();
        self.is_dirty = false;
        (self.original_config.clone(), OptionsSoundCue::ConfirmSave)
    }

    /// Discard modifications and revert to baseline.
    pub fn discard_changes(&mut self) -> OptionsSoundCue {
        self.draft_config = self.original_config.clone();
        self.is_dirty = false;
        OptionsSoundCue::CancelDiscard
    }
}

// -----------------------------------------------------------------------------------------
// Authentic Proportional Options Screen Rasterization (`AtlasFont`)
// -----------------------------------------------------------------------------------------

/// Draws the authentic options screen onto `rast` using proportional fonts,
/// two-tone gradient shading, and authentic SH2 styling.
pub fn draw_options_screen(menu: &OptionsMenu, font: &AtlasFont, rast: &mut TextRasterizer) {
    let center_x = rast.width / 2;

    let tab_name = menu.active_tab.label();
    font.draw_line_centered(
        rast,
        &format!("OPTIONS: {}", tab_name),
        (center_x, 90),
        28,
        Sh2TextColor::AmberHeader,
        true,
    );

    font.draw_line_centered(
        rast,
        "[Q / E : Switch Category]",
        (center_x, 130),
        18,
        Sh2TextColor::Cyan,
        true,
    );

    let start_y = 180;
    let row_h = 38;
    let label_x = 240;
    let colon_x = 640;
    let val_x = 670;

    for i in 0..menu.item_count() {
        if let Some((label, val)) = menu.item_display(i) {
            let is_sel = i == menu.selected_index;
            let y = start_y + i * row_h;
            let style = if is_sel {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::SilverUnselected
            };

            let prefix = if is_sel { "> " } else { "  " };
            let full_label = format!("{}{}", prefix, label);

            font.draw_line_styled(rast, &full_label, (label_x, y), 22, style, true);
            font.draw_line_styled(rast, ":", (colon_x, y), 22, Sh2TextColor::MutedFooter, true);
            font.draw_line_styled(rast, &val, (val_x, y), 22, style, true);
        }
    }

    if menu.is_dirty {
        font.draw_line_centered(
            rast,
            "* UNSAVED CHANGES [Enter to Save | Esc to Discard]",
            (center_x, 658),
            16,
            Sh2TextColor::GoldSelected,
            true,
        );
    } else {
        font.draw_line_centered(
            rast,
            "[Esc / Backspace: Return to Main Menu]",
            (center_x, 658),
            16,
            Sh2TextColor::MutedFooter,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_core::config::InternalResolution;

    #[test]
    fn test_options_tab_switching_and_editing() {
        let original = AppConfig::default();
        let mut menu = OptionsMenu::new(original);

        assert_eq!(menu.active_tab, OptionsTab::Video);
        assert_eq!(menu.item_count(), 10);

        // Adjust Resolution
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.video.resolution,
            InternalResolution::Res1080p
        );
        assert!(menu.is_dirty);

        // Switch to Audio Tab
        menu.switch_tab(true);
        assert_eq!(menu.active_tab, OptionsTab::Audio);
        assert_eq!(menu.selected_index, 0);

        // Adjust Master Volume
        menu.adjust_value(-1);
        assert!((menu.draft_config.audio.master_volume - 0.95).abs() < 0.001);

        // Discard changes
        menu.discard_changes();
        assert!(!menu.is_dirty);
        assert_eq!(
            menu.draft_config.video.resolution,
            InternalResolution::Native
        );
    }

    #[test]
    fn test_options_rendering_with_atlas_font() {
        let original = AppConfig::default();
        let menu = OptionsMenu::new(original);
        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(800, 600);

        draw_options_screen(&menu, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));
    }
}
