//! In-Game Options Menu replicating SH2:EE (Enhanced Edition) advanced options architecture.
//! References: `Patches/CustomAdvancedOptions.cpp`, `Patches/OptionsMenuTweaks.cpp`.
//!
//! Modularized into:
//! - [`tabs`]: Per-tab item display formatting and value adjustment logic (`Video`, `Audio`, `Controls`, `Game`, `Debug`).
//! - [`render`]: Authentic PS2 `optmenu.c` (`0x45e750`, `0x461600`, `0x45bf10`) screen rasterization and polygon drawing.

mod render;
pub mod subscreens;
mod tabs;

pub use render::{draw_options_screen, draw_polygon};
pub use subscreens::{
    arrow_pulse_alpha, draw_brightness_subscreen, draw_controller_config_subscreen,
    draw_option_arrow, format_ps2_button_mask, ArrowDir, OptionsSubScreen,
};

use sh2_core::config::{
    AppConfig, AudioPackDetectionStatus, ControllerPreset, PS2_MAX_BRIGHTNESS_LEVEL,
};

/// High-level Options category tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptionsTab {
    #[default]
    Video = 0,
    Audio = 1,
    Controls = 2,
    Game = 3,
    /// Dedicated collated tab for all enhanced graphics, audio, and camera toggles + 1-click Version Preset.
    Enhanced = 4,
    Debug = 5,
}

impl OptionsTab {
    pub const ALL: [OptionsTab; 6] = [
        OptionsTab::Video,
        OptionsTab::Audio,
        OptionsTab::Controls,
        OptionsTab::Game,
        OptionsTab::Enhanced,
        OptionsTab::Debug,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "VIDEO & GRAPHICS",
            Self::Audio => "AUDIO & SOUND",
            Self::Controls => "CONTROLS & INPUT",
            Self::Game => "GAMEPLAY & PUZZLES",
            Self::Enhanced => "ENHANCED MODE",
            Self::Debug => "DEBUG & TESTING",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Video => Self::Audio,
            Self::Audio => Self::Controls,
            Self::Controls => Self::Game,
            Self::Game => Self::Enhanced,
            Self::Enhanced => Self::Debug,
            Self::Debug => Self::Video,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Video => Self::Debug,
            Self::Audio => Self::Video,
            Self::Controls => Self::Audio,
            Self::Game => Self::Controls,
            Self::Enhanced => Self::Game,
            Self::Debug => Self::Enhanced,
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
    /// Active PS2 Options subscreen (`bright_main` / `config_main` / `position_main`).
    pub sub_screen: OptionsSubScreen,
    /// Active row inside `config_main` (`0` = top preset bar, `1..=13` = 13 actions).
    pub config_row: usize,
    /// Active column on top preset bar of `config_main` (`0=EXIT, 1=Type 1, 2=Type 2, 3=Type 3, 4=RESET`).
    pub config_preset_col: usize,
    /// Frame counter driving pulsing `allow_u/d/l/r` arrows (`frame % 40`).
    pub anim_frame: u32,
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
            sub_screen: OptionsSubScreen::None,
            config_row: 0,
            config_preset_col: 1,
            anim_frame: 0,
        }
    }

    /// Advances the options animation frame counter (for pulsing `allow_u/d/l/r` arrows).
    pub fn tick_frame(&mut self) {
        self.anim_frame = self.anim_frame.wrapping_add(1);
    }

    /// Opens a PS2 Options subscreen (`bright_main`, `config_main`, or `position_main`).
    pub fn open_sub_screen(&mut self, sub: OptionsSubScreen) {
        self.sub_screen = sub;
        if sub == OptionsSubScreen::ControllerConfig {
            self.config_row = 0;
            self.config_preset_col = match self.draft_config.controls.ps2_bindings.preset {
                ControllerPreset::Type1 => 1,
                ControllerPreset::Type2 => 2,
                ControllerPreset::Type3 => 3,
            };
        }
    }

    /// Closes any active PS2 Options subscreen back to the main tabbed options view.
    pub fn close_sub_screen(&mut self) -> bool {
        if self.sub_screen != OptionsSubScreen::None {
            self.sub_screen = OptionsSubScreen::None;
            true
        } else {
            false
        }
    }

    /// Handles Confirm inside an active PS2 subscreen. Returns `Some(cue)` if consumed.
    pub fn handle_subscreen_confirm(&mut self) -> Option<OptionsSoundCue> {
        match self.sub_screen {
            OptionsSubScreen::None => None,
            OptionsSubScreen::BrightnessCalibration => {
                self.sub_screen = OptionsSubScreen::None;
                Some(OptionsSoundCue::ConfirmSave)
            }
            OptionsSubScreen::ControllerConfig => {
                if self.config_row == 0 {
                    match self.config_preset_col {
                        0 => {
                            self.sub_screen = OptionsSubScreen::None;
                            Some(OptionsSoundCue::ConfirmSave)
                        }
                        1 => {
                            self.draft_config
                                .controls
                                .ps2_bindings
                                .apply_preset(ControllerPreset::Type1);
                            self.is_dirty = self.draft_config != self.original_config;
                            Some(OptionsSoundCue::ValueChange)
                        }
                        2 => {
                            self.draft_config
                                .controls
                                .ps2_bindings
                                .apply_preset(ControllerPreset::Type2);
                            self.is_dirty = self.draft_config != self.original_config;
                            Some(OptionsSoundCue::ValueChange)
                        }
                        3 => {
                            self.draft_config
                                .controls
                                .ps2_bindings
                                .apply_preset(ControllerPreset::Type3);
                            self.is_dirty = self.draft_config != self.original_config;
                            Some(OptionsSoundCue::ValueChange)
                        }
                        4 => {
                            self.draft_config
                                .controls
                                .ps2_bindings
                                .apply_preset(ControllerPreset::Type1);
                            self.is_dirty = self.draft_config != self.original_config;
                            Some(OptionsSoundCue::ValueChange)
                        }
                        _ => Some(OptionsSoundCue::None),
                    }
                } else {
                    Some(OptionsSoundCue::None)
                }
            }
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

    /// Navigate up / down within active tab or active PS2 subscreen.
    pub fn navigate_vertical(&mut self, delta: i32) -> OptionsSoundCue {
        match self.sub_screen {
            OptionsSubScreen::BrightnessCalibration => return OptionsSoundCue::None,
            OptionsSubScreen::ControllerConfig => {
                // `config_main` (`0x00213a50`): `key_cur ∈ 0..=13` wrapping.
                if delta < 0 {
                    self.config_row = if self.config_row == 0 {
                        13
                    } else {
                        self.config_row - 1
                    };
                } else if delta > 0 {
                    self.config_row = (self.config_row + 1) % 14;
                }
                return OptionsSoundCue::CursorMove;
            }
            OptionsSubScreen::None => {}
        }

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

    /// Adjusts active PS2 subscreen horizontal parameter (`bright_main`, `position_main`,
    /// `config_main`). Returns `Some(cue)` if consumed by a subscreen.
    pub(crate) fn adjust_subscreen_horizontal(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        match self.sub_screen {
            OptionsSubScreen::None => None,
            OptionsSubScreen::BrightnessCalibration => {
                let cur = self.draft_config.game.brightness_level as i16;
                let next = (cur + if forward { 1 } else { -1 })
                    .clamp(0, PS2_MAX_BRIGHTNESS_LEVEL as i16) as u8;
                self.draft_config.game.brightness_level = next;
                self.is_dirty = self.draft_config != self.original_config;
                Some(OptionsSoundCue::ValueChange)
            }
            OptionsSubScreen::ControllerConfig => {
                if self.config_row == 0 {
                    if forward {
                        self.config_preset_col = (self.config_preset_col + 1) % 5;
                    } else {
                        self.config_preset_col = if self.config_preset_col == 0 {
                            4
                        } else {
                            self.config_preset_col - 1
                        };
                    }
                    Some(OptionsSoundCue::CursorMove)
                } else {
                    Some(OptionsSoundCue::None)
                }
            }
        }
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
        assert_eq!(menu.item_count(), 15);

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

        // Adjust Master Volume: 16 discrete PS2 levels (15 -> 14 = 14/15)
        menu.adjust_value(-1);
        assert!((menu.draft_config.audio.master_volume - (14.0 / 15.0)).abs() < 0.001);

        // Discard changes
        menu.discard_changes();
        assert!(!menu.is_dirty);
        assert_eq!(
            menu.draft_config.video.resolution,
            InternalResolution::Native
        );
    }

    #[test]
    fn test_ps2_options_subscreens_and_extra_options() {
        use sh2_core::config::{BloodColor, RetreatTurnMode, ViewControlMode, WalkRunControlMode};

        let mut menu = OptionsMenu::new(AppConfig::default());
        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);

        // 1. Brightness Calibration (`bright_main` 0x00212ec0, 0..=7, default 3)
        menu.open_sub_screen(OptionsSubScreen::BrightnessCalibration);
        assert_eq!(menu.draft_config.game.brightness_level, 3);
        menu.adjust_value(1);
        assert_eq!(menu.draft_config.game.brightness_level, 4);
        draw_options_screen(&menu, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));
        menu.handle_subscreen_confirm();
        assert_eq!(menu.sub_screen, OptionsSubScreen::None);

        // 2. Controller Config (`config_main` 0x00213a50, Type 1/2/3 + conflict check)
        menu.open_sub_screen(OptionsSubScreen::ControllerConfig);
        menu.adjust_value(1); // Type 2
        menu.handle_subscreen_confirm();
        assert_eq!(
            menu.draft_config.controls.ps2_bindings.preset,
            ControllerPreset::Type2
        );
        rast.clear();
        draw_options_screen(&menu, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));
        assert!(menu.close_sub_screen());

        // 3. PS2 Extra Options in Game tab (Auto Load, Blood Color, View Control, Retreat Turn, Walk/Run)
        menu.active_tab = OptionsTab::Game;
        menu.selected_index = 8;
        menu.adjust_value(1);
        assert!(menu.draft_config.game.auto_load);
        menu.selected_index = 9;
        menu.adjust_value(1);
        assert_eq!(menu.draft_config.game.blood_color, BloodColor::Green);
        menu.selected_index = 10;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.game.view_control,
            ViewControlMode::Reverse
        );
        menu.selected_index = 11;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.game.retreat_turn,
            RetreatTurnMode::Reverse
        );
        menu.selected_index = 12;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.game.walk_run_control,
            WalkRunControlMode::Reverse
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

    #[test]
    fn test_sh2_93_controls_tab_camera_mode_fov_and_sway_options() {
        use sh2_core::config::{CameraCyclePreset, GameplayCameraMode, OtsShoulderSide};

        let mut menu = OptionsMenu::new(AppConfig::default());
        menu.active_tab = OptionsTab::Controls;
        assert_eq!(menu.item_count(), 15);

        // Camera Mode: ClassicPs2Rail -> OverTheShoulder -> FirstPerson
        menu.selected_index = 5;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.controls.default_camera_mode,
            GameplayCameraMode::OverTheShoulder
        );
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.controls.default_camera_mode,
            GameplayCameraMode::FirstPerson
        );

        // R3 Cycle Preset & OTS Shoulder
        menu.selected_index = 6;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.controls.camera_cycle_preset,
            CameraCyclePreset::ClassicAndOts
        );
        menu.selected_index = 7;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.controls.ots_shoulder,
            OtsShoulderSide::Left
        );

        // First-Person FOV (75 -> 80) & Sway toggle
        menu.selected_index = 9;
        menu.adjust_value(1);
        assert_eq!(menu.draft_config.controls.first_person_fov_deg, 80);
        menu.selected_index = 10;
        menu.adjust_value(1);
        assert!(!menu.draft_config.controls.first_person_head_bob);
    }

    #[test]
    fn test_enhanced_tab_version_presets_and_custom_detection() {
        use sh2_core::config::{
            EnhancedPreset, FogQuality, LightingProfile, ShadowQualityMode, SpatialAudioMode,
            WorldTextureQuality,
        };

        let mut menu = OptionsMenu::new(AppConfig::default());
        menu.active_tab = OptionsTab::Enhanced;
        assert_eq!(menu.item_count(), 19);
        assert!(!menu.draft_config.game.enable_secondary_physics);
        assert!(!menu.draft_config.game.enable_unlimited_save_slots);
        assert!(!menu.draft_config.game.enable_quicksave_autosave);
        assert!(!menu.draft_config.game.enable_save_thumbnails);
        assert!(!menu.draft_config.game.enable_free_item_orbit);
        assert!(!menu.draft_config.audio.enable_dynamic_room_acoustics);

        // Row 0: Version Preset snaps to PS2 Authentic (1:1 Original)
        menu.selected_index = 0;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::Ps2Authentic
        );
        assert!(!menu.draft_config.video.aces_tonemapping);
        assert!(!menu.draft_config.game.enable_secondary_physics);
        assert_eq!(
            menu.draft_config.video.texture_quality,
            WorldTextureQuality::OriginalPs2
        );
        assert_eq!(menu.draft_config.video.fog_quality, FogQuality::ClassicPs2);
        assert_eq!(
            menu.draft_config.video.shadow_quality,
            ShadowQualityMode::ClassicPs2Stencil
        );
        assert_eq!(
            menu.draft_config.video.lighting_profile,
            LightingProfile::Ps2Authentic
        );
        assert_eq!(
            menu.draft_config.audio.spatial_audio_mode,
            SpatialAudioMode::ClassicPs2Stereo
        );
        assert!(!menu.draft_config.audio.enable_acoustic_occlusion);
        assert!(!menu.draft_config.audio.enable_dynamic_room_acoustics);

        // Next preset: SH2:EE
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::Sh2ee
        );
        assert!(!menu.draft_config.video.aces_tonemapping);
        assert_eq!(
            menu.draft_config.video.shadow_quality,
            ShadowQualityMode::EnhancedEditionSoft
        );

        // Next preset: Native Enhanced
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::NativeEnhanced
        );
        assert!(menu.draft_config.video.aces_tonemapping);
        assert_eq!(
            menu.draft_config.video.shadow_quality,
            ShadowQualityMode::NativePcss
        );
        assert_eq!(
            menu.draft_config.audio.spatial_audio_mode,
            SpatialAudioMode::BinauralHrtf
        );
        assert!(menu.draft_config.game.enable_secondary_physics);
        assert!(menu.draft_config.audio.enable_dynamic_room_acoustics);

        // Toggling ACES Tonemapping (row 11) switches Version Preset to Custom (Mixed)
        menu.selected_index = 11;
        assert_eq!(menu.item_display(11).unwrap().0, "ACES Tonemapping");
        menu.adjust_value(1);
        assert!(!menu.draft_config.video.aces_tonemapping);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::Custom
        );
        menu.adjust_value(1);
        assert!(menu.draft_config.video.aces_tonemapping);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::NativeEnhanced
        );

        // Toggling Dynamic Room Acoustics (row 12) switches Version Preset to Custom (Mixed)
        menu.selected_index = 12;
        assert_eq!(menu.item_display(12).unwrap().0, "Dynamic Room Acoustics");
        menu.adjust_value(1);
        assert!(!menu.draft_config.audio.enable_dynamic_room_acoustics);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::Custom
        );
        menu.adjust_value(1);
        assert!(menu.draft_config.audio.enable_dynamic_room_acoustics);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::NativeEnhanced
        );

        // Toggling Secondary Cloth Physics (row 10) switches Version Preset to Custom (Mixed)
        menu.selected_index = 10;
        menu.adjust_value(1);
        assert!(!menu.draft_config.game.enable_secondary_physics);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::Custom
        );

        // Toggling an individual setting switches Version Preset display to Custom (Mixed)
        menu.selected_index = 4; // Shadow Mode
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.detect_enhanced_preset(),
            EnhancedPreset::Custom
        );
    }
}
