//! In-Game Options Menu replicating SH2:EE (Enhanced Edition) advanced options architecture.
//! References: `Patches/CustomAdvancedOptions.cpp`, `Patches/OptionsMenuTweaks.cpp`.
//!
//! Modularized into:
//! - [`tabs`]: Per-tab item display formatting and value adjustment logic (`Video`, `Audio`, `Controls`, `Game`, `Debug`).
//! - [`render`]: Authentic PS2 `optmenu.c` (`0x45e750`, `0x461600`, `0x45bf10`) screen rasterization and polygon drawing.

mod render;
mod tabs;

pub use render::{draw_options_screen, draw_polygon};

use sh2_core::config::{AppConfig, AudioPackDetectionStatus};

/// High-level Options category tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptionsTab {
    #[default]
    Video = 0,
    Audio = 1,
    Controls = 2,
    Game = 3,
    Debug = 4,
}

impl OptionsTab {
    pub const ALL: [OptionsTab; 5] = [
        OptionsTab::Video,
        OptionsTab::Audio,
        OptionsTab::Controls,
        OptionsTab::Game,
        OptionsTab::Debug,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "VIDEO & GRAPHICS",
            Self::Audio => "AUDIO & SOUND",
            Self::Controls => "CONTROLS & INPUT",
            Self::Game => "GAMEPLAY & PUZZLES",
            Self::Debug => "DEBUG & TESTING",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Video => Self::Audio,
            Self::Audio => Self::Controls,
            Self::Controls => Self::Game,
            Self::Game => Self::Debug,
            Self::Debug => Self::Video,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Video => Self::Debug,
            Self::Audio => Self::Video,
            Self::Controls => Self::Audio,
            Self::Game => Self::Controls,
            Self::Debug => Self::Game,
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
    /// Triggered when the user selects the Test Rooms launcher from the Options menu.
    LaunchTestRooms,
    /// Triggered when the user selects the Loading Screen Preview from the Options menu.
    LaunchLoadingTest,
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
    /// Discovered host audio output devices (e.g. "Default", "Speakers", "Headphones").
    pub audio_devices: Vec<String>,
    /// Filesystem detection status for SH2:EE Lossless (`sh2e/sound/`) and Original PS2/PC (`data/sound/`) audio packs (`SH2-59`).
    pub audio_pack_status: AudioPackDetectionStatus,
}

impl OptionsMenu {
    pub fn new(config: AppConfig) -> Self {
        Self::new_with_audio_packs(config, AudioPackDetectionStatus::default())
    }

    pub fn new_with_audio_packs(
        config: AppConfig,
        audio_pack_status: AudioPackDetectionStatus,
    ) -> Self {
        Self {
            active_tab: OptionsTab::Video,
            selected_index: 0,
            draft_config: config.clone(),
            original_config: config,
            is_dirty: false,
            audio_devices: vec!["Default (System)".into()],
            audio_pack_status,
        }
    }

    /// Updates the detected filesystem audio pack status (`SH2-59`).
    pub fn set_audio_pack_status(&mut self, status: AudioPackDetectionStatus) {
        self.audio_pack_status = status;
    }

    /// Sets the list of available host audio devices.
    pub fn set_audio_devices(&mut self, mut devices: Vec<String>) {
        if !devices.iter().any(|d| d == "Default (System)") {
            devices.insert(0, "Default (System)".into());
        }
        self.audio_devices = devices;
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

    /// Save draft modifications and commit as new baseline.
    pub fn commit_changes(&mut self) -> (AppConfig, OptionsSoundCue) {
        self.original_config = self.draft_config.clone();
        self.is_dirty = false;
        (self.original_config.clone(), OptionsSoundCue::ConfirmSave)
    }

    /// Reset draft configuration to default factory settings.
    pub fn reset_to_defaults(&mut self) -> OptionsSoundCue {
        self.draft_config = AppConfig::default();
        self.is_dirty = true;
        OptionsSoundCue::ValueChange
    }

    /// Discard modifications and revert to baseline.
    pub fn discard_changes(&mut self) -> OptionsSoundCue {
        self.draft_config = self.original_config.clone();
        self.is_dirty = false;
        OptionsSoundCue::CancelDiscard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_font::AtlasFont;
    use crate::font::TextRasterizer;
    use sh2_core::config::InternalResolution;

    #[test]
    fn test_options_tab_switching_and_editing() {
        let original = AppConfig::default();
        let mut menu = OptionsMenu::new(original);

        assert_eq!(menu.active_tab, OptionsTab::Video);
        assert_eq!(menu.item_count(), 11);

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
        let mut rast = TextRasterizer::new(640, 480);

        draw_options_screen(&menu, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));

        let _ = image::save_buffer(
            "/tmp/rendered_options_test.png",
            &rast.pixels,
            640,
            480,
            image::ExtendedColorType::Rgba8,
        );
    }
}
