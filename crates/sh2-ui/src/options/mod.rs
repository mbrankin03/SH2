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

/// Lightweight UI representation of a discovered mod package in `mods/` (`SH2-29`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModUiEntry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub override_count: usize,
    pub enabled: bool,
    pub priority: i32,
    pub is_zip: bool,
}

/// High-level Options category tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptionsTab {
    /// Authentic 10-row SH2/SH2:EE root `O P T I O N S` screen (`Game/Advanced/Control Options` + core settings).
    #[default]
    Root = 7,
    Video = 0,
    Audio = 1,
    Controls = 2,
    Game = 3,
    /// Dedicated collated tab for all enhanced graphics, audio, and camera toggles + 1-click Version Preset.
    Enhanced = 4,
    /// Drag-and-drop mod manager (`SH2-29`).
    Mods = 5,
    Debug = 6,
}

impl OptionsTab {
    pub const ALL: [OptionsTab; 7] = [
        OptionsTab::Video,
        OptionsTab::Audio,
        OptionsTab::Controls,
        OptionsTab::Game,
        OptionsTab::Enhanced,
        OptionsTab::Mods,
        OptionsTab::Debug,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Root => "OPTIONS",
            Self::Video => "VIDEO & GRAPHICS",
            Self::Audio => "AUDIO & SOUND",
            Self::Controls => "CONTROLS & INPUT",
            Self::Game => "GAMEPLAY & PUZZLES",
            Self::Enhanced => "ENHANCED MODE",
            Self::Mods => "MOD MANAGER",
            Self::Debug => "DEBUG & TESTING",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Video => Self::Audio,
            Self::Audio => Self::Controls,
            Self::Controls => Self::Game,
            Self::Game => Self::Enhanced,
            Self::Enhanced => Self::Mods,
            Self::Mods => Self::Debug,
            Self::Debug => Self::Video,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Video => Self::Debug,
            Self::Audio => Self::Video,
            Self::Controls => Self::Audio,
            Self::Game => Self::Controls,
            Self::Enhanced => Self::Game,
            Self::Mods => Self::Enhanced,
            Self::Debug => Self::Mods,
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
    /// First visible row index when a tab has more than `10` items (`option_font` `0x0021a320`).
    pub scroll_offset: usize,
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
    /// Discovered mods in `mods/` displayed in the `MOD MANAGER` tab (`SH2-29`).
    pub mods: Vec<ModUiEntry>,
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
            active_tab: OptionsTab::Root,
            selected_index: 0,
            scroll_offset: 0,
            draft_config: config.clone(),
            original_config: config,
            is_dirty: false,
            audio_devices: vec!["Default (System)".into()],
            audio_pack_status,
            sub_screen: OptionsSubScreen::None,
            config_row: 0,
            config_preset_col: 1,
            anim_frame: 0,
            mods: Vec::new(),
        }
    }

    /// Populates the discovered mod list for the `MOD MANAGER` tab (`SH2-29`).
    pub fn set_mods(&mut self, mods: Vec<ModUiEntry>) {
        self.mods = mods;
        if self.active_tab == OptionsTab::Mods {
            let count = self.item_count();
            if self.selected_index >= count {
                self.selected_index = count.saturating_sub(1);
            }
        }
    }

    /// Synchronizes current `self.mods` enabled states and load-order priorities into `draft_config.mods`.
    pub(crate) fn sync_mods_to_draft_config(&mut self) {
        let len = self.mods.len() as i32;
        self.draft_config.mods.entries.clear();
        for (idx, m) in self.mods.iter_mut().enumerate() {
            // Higher priority = earlier in list (top = highest)
            m.priority = len - idx as i32;
            self.draft_config
                .mods
                .set_mod_state(m.id.clone(), m.enabled, m.priority);
        }
        self.is_dirty = self.draft_config != self.original_config;
    }

    /// Moves the currently selected mod up (`delta < 0`) or down (`delta > 0`) in load order.
    pub fn move_selected_mod(&mut self, delta: i32) -> OptionsSoundCue {
        if self.active_tab != OptionsTab::Mods || self.mods.len() < 2 {
            return OptionsSoundCue::None;
        }
        let idx = self.selected_index;
        if idx >= self.mods.len() {
            return OptionsSoundCue::None;
        }
        if delta < 0 && idx > 0 {
            self.mods.swap(idx, idx - 1);
            self.selected_index = idx - 1;
            self.sync_mods_to_draft_config();
            return OptionsSoundCue::ValueChange;
        }
        if delta > 0 && idx + 1 < self.mods.len() {
            self.mods.swap(idx, idx + 1);
            self.selected_index = idx + 1;
            self.sync_mods_to_draft_config();
            return OptionsSoundCue::ValueChange;
        }
        OptionsSoundCue::None
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

    /// Updates `selected_index` when hovering over visible rows in `512×448` options space (`[0..512] × [0..448]`).
    /// Returns `true` if `selected_index` changed.
    pub fn handle_mouse_move(&mut self, ps2_x: f32, ps2_y: f32) -> bool {
        if self.sub_screen != OptionsSubScreen::None || !(28.0..=484.0).contains(&ps2_x) {
            return false;
        }
        let item_count = self.item_count();
        let (max_visible_rows, base_y_v, step_y_v) = if self.active_tab == OptionsTab::Root {
            (11usize, 74.0f32, 25.0f32)
        } else {
            (10usize, 81.0f32, 27.0f32)
        };
        let visible_start = self.scroll_offset.min(item_count.saturating_sub(1));
        let visible_end = (visible_start + max_visible_rows).min(item_count);
        for (vis_row, idx) in (visible_start..visible_end).enumerate() {
            let row_top = base_y_v + vis_row as f32 * step_y_v - 4.0;
            if (row_top..=row_top + step_y_v).contains(&ps2_y) {
                if self.selected_index != idx {
                    self.selected_index = idx;
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Scrolls the active options list vertically via mouse wheel (`steps > 0` = scroll down,
    /// `steps < 0` = scroll up) without wrapping around the top/bottom ends, and re-evaluates
    /// `hover_ps2` so the row under the mouse cursor stays highlighted as the list scrolls.
    pub fn handle_mouse_wheel(
        &mut self,
        steps: i32,
        hover_ps2: Option<(f32, f32)>,
    ) -> OptionsSoundCue {
        if steps == 0 || self.sub_screen != OptionsSubScreen::None {
            return OptionsSoundCue::None;
        }
        let item_count = self.item_count();
        if item_count == 0 {
            return OptionsSoundCue::None;
        }
        let max_vis = if self.active_tab == OptionsTab::Root {
            11
        } else {
            10
        };
        let max_scroll = item_count.saturating_sub(max_vis);
        let prev_scroll = self.scroll_offset;
        let prev_sel = self.selected_index;

        if max_scroll > 0 {
            let next_scroll =
                (self.scroll_offset as i32 + steps).clamp(0, max_scroll as i32) as usize;
            self.scroll_offset = next_scroll;
            if let Some((px, py)) = hover_ps2 {
                self.handle_mouse_move(px, py);
            }
            if self.selected_index < self.scroll_offset {
                self.selected_index = self.scroll_offset;
            } else if self.selected_index >= self.scroll_offset + max_vis {
                self.selected_index = (self.scroll_offset + max_vis - 1).min(item_count - 1);
            }
        } else {
            let next_sel =
                (self.selected_index as i32 + steps).clamp(0, item_count as i32 - 1) as usize;
            self.selected_index = next_sel;
        }

        if self.scroll_offset != prev_scroll || self.selected_index != prev_sel {
            OptionsSoundCue::CursorMove
        } else {
            OptionsSoundCue::None
        }
    }

    /// Handles a left mouse click at `(ps2_x, ps2_y)` in `512×448` options space:
    /// - On the right-edge scrollbar track (`x ∈ [454.0..=482.0]`, `y ∈ [76.0..=348.0]`) when `item_count > max_vis`, jumps `scroll_offset` to the clicked track position.
    /// - On submenu rows (`Game Options`, `Advanced Options`, `Audio Options`, `Control Options`, `Enhanced Options`), enters the submenu.
    /// - On 15-bar volume rows, sets volume directly to the clicked bar (`0..=15`) when clicking in the bar region (`x >= 265`).
    /// - On any other setting row, cycles/toggles its value forward (or backward if clicking the left label side `x < 256`).
    pub fn handle_mouse_click(&mut self, ps2_x: f32, ps2_y: f32) -> Option<OptionsSoundCue> {
        let item_count = self.item_count();
        let max_vis = if self.active_tab == OptionsTab::Root {
            11
        } else {
            10
        };
        if self.sub_screen == OptionsSubScreen::None
            && item_count > max_vis
            && (454.0..=482.0).contains(&ps2_x)
            && (76.0..=348.0).contains(&ps2_y)
        {
            let max_scroll = item_count - max_vis;
            let t = ((ps2_y - 80.0) / (344.0 - 80.0)).clamp(0.0, 1.0);
            let next_scroll = (t * max_scroll as f32).round() as usize;
            self.scroll_offset = next_scroll.min(max_scroll);
            self.selected_index = self
                .selected_index
                .clamp(self.scroll_offset, self.scroll_offset + max_vis - 1);
            return Some(OptionsSoundCue::CursorMove);
        }
        self.handle_mouse_move(ps2_x, ps2_y);
        if self.enter_submenu() {
            return Some(OptionsSoundCue::CursorMove);
        }
        if let Some((label, _)) = self.item_display(self.selected_index) {
            let is_slider = (self.active_tab == OptionsTab::Audio
                || self.active_tab == OptionsTab::Root)
                && (label == "BGM Volume"
                    || label == "SFX Volume"
                    || label == "Master Volume"
                    || label == "Dialogue Volume");
            if is_slider && ps2_x >= 265.0 {
                let bar = (((ps2_x - 271.0) / 11.0).floor() as i32 + 1).clamp(0, 15) as u8;
                let gain = sh2_core::config::ps2_level_to_volume_gain(bar);
                match label {
                    "Master Volume" => self.draft_config.audio.master_volume = gain,
                    "BGM Volume" => self.draft_config.audio.bgm_volume = gain,
                    "SFX Volume" => self.draft_config.audio.sfx_volume = gain,
                    "Dialogue Volume" => self.draft_config.audio.dialogue_volume = gain,
                    _ => {}
                }
                self.is_dirty = self.draft_config != self.original_config;
                return Some(OptionsSoundCue::ValueChange);
            }
        }
        if self.active_tab == OptionsTab::Mods && !self.mods.is_empty() {
            if (400.0..432.0).contains(&ps2_x) {
                return Some(self.move_selected_mod(-1));
            }
            if ps2_x >= 432.0 {
                return Some(self.move_selected_mod(1));
            }
        }
        let dir = if ps2_x < 245.0 { -1 } else { 1 };
        Some(self.adjust_value(dir))
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

    /// Enters a submenu from `OptionsTab::Root` (`Game Options`, `Advanced Options`, `Control Options`)
    /// or `Enhanced Options` inside `Advanced Options` (`OptionsTab::Video`).
    pub fn enter_submenu(&mut self) -> bool {
        if self.sub_screen != OptionsSubScreen::None {
            return false;
        }
        let target = match (self.active_tab, self.selected_index) {
            (OptionsTab::Root, 0) => Some(OptionsTab::Game),
            (OptionsTab::Root, 1) => Some(OptionsTab::Video),
            (OptionsTab::Root, 2) => Some(OptionsTab::Audio),
            (OptionsTab::Root, 3) => Some(OptionsTab::Controls),
            (OptionsTab::Video, 15) => Some(OptionsTab::Enhanced),
            (OptionsTab::Video, 17) => Some(OptionsTab::Mods),
            _ => None,
        };
        if let Some(tab) = target {
            self.active_tab = tab;
            self.selected_index = 0;
            self.scroll_offset = 0;
            true
        } else {
            false
        }
    }

    /// Steps back one submenu level (`Enhanced -> Video`, `Game/Video/Audio/Controls -> Root`).
    pub fn exit_to_root(&mut self) -> bool {
        if self.active_tab == OptionsTab::Root {
            return false;
        }
        let (parent_tab, parent_idx) = match self.active_tab {
            OptionsTab::Enhanced => (OptionsTab::Video, 15),
            OptionsTab::Mods => (OptionsTab::Video, 17),
            OptionsTab::Game => (OptionsTab::Root, 0),
            OptionsTab::Video | OptionsTab::Debug => (OptionsTab::Root, 1),
            OptionsTab::Audio => (OptionsTab::Root, 2),
            OptionsTab::Controls => (OptionsTab::Root, 3),
            OptionsTab::Root => (OptionsTab::Root, 0),
        };
        self.active_tab = parent_tab;
        self.selected_index = parent_idx;
        let max_vis = if parent_tab == OptionsTab::Root {
            11
        } else {
            10
        };
        self.scroll_offset = parent_idx.saturating_sub(max_vis - 1);
        true
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
        let max_vis = if self.active_tab == OptionsTab::Root {
            11
        } else {
            10
        };
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + max_vis {
            self.scroll_offset = self.selected_index + 1 - max_vis;
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
        self.scroll_offset = 0;
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

        assert_eq!(menu.active_tab, OptionsTab::Root);
        assert_eq!(menu.item_count(), 11);
        menu.navigate_vertical(1); // Advanced Options
        assert!(menu.enter_submenu());
        assert_eq!(menu.active_tab, OptionsTab::Video);
        assert_eq!(menu.item_count(), 21);

        // Verify Cutaway & Puzzle Quality 3-tier selector (index 18) and Aspect Scaling (index 19)
        menu.selected_index = 18;
        assert_eq!(
            menu.item_display(18),
            Some((
                "Cutaway & Puzzle Quality",
                "4K AI Master (4096x4096)".to_string()
            ))
        );
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.video.cutaway_quality,
            sh2_core::CutawayQuality::Sh2eeHd
        );
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.video.cutaway_quality,
            sh2_core::CutawayQuality::OriginalPs2
        );
        menu.selected_index = 19;
        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.video.cutaway_aspect_mode,
            sh2_core::CutawayAspectMode::Fill16x9WidescreenCrop
        );
        menu.discard_changes();
        menu.selected_index = 0;

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
        assert_eq!(menu.item_count(), 20);
        assert!(!menu.draft_config.game.enable_secondary_physics);
        assert!(!menu.draft_config.game.enable_foot_ik);
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
        assert!(menu.draft_config.game.enable_foot_ik);
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

    #[test]
    fn test_sh2_29_mods_tab_toggle_and_priority_reorder() {
        let mut menu = OptionsMenu::new(AppConfig::default());
        menu.set_mods(vec![
            ModUiEntry {
                id: "mod_a".into(),
                name: "Mod A".into(),
                author: "Author A".into(),
                version: "1.0".into(),
                description: "First mod".into(),
                override_count: 5,
                enabled: true,
                priority: 2,
                is_zip: false,
            },
            ModUiEntry {
                id: "mod_b".into(),
                name: "Mod B".into(),
                author: "Author B".into(),
                version: "2.0".into(),
                description: "Second mod".into(),
                override_count: 3,
                enabled: true,
                priority: 1,
                is_zip: true,
            },
        ]);

        menu.active_tab = OptionsTab::Mods;
        menu.selected_index = 0;
        assert_eq!(menu.item_count(), 2);
        let (label, val) = menu.item_display_cow(0).unwrap();
        assert_eq!(label, "Mod A");
        assert!(val.contains("ENABLED") && val.contains("#1"));

        // Toggle Mod A off
        assert_eq!(menu.adjust_value(1), OptionsSoundCue::ValueChange);
        assert!(!menu.mods[0].enabled);
        assert!(!menu.draft_config.mods.entries[0].enabled);

        // Move Mod A down in priority
        assert_eq!(menu.move_selected_mod(1), OptionsSoundCue::ValueChange);
        assert_eq!(menu.selected_index, 1);
        assert_eq!(menu.mods[0].id, "mod_b");
        assert_eq!(menu.mods[1].id, "mod_a");
        assert!(menu.mods[0].priority > menu.mods[1].priority);
    }

    #[test]
    fn test_sh2_90_video_fmv_source_option_and_ps2_preset() {
        let mut menu = OptionsMenu::new(AppConfig::default());
        menu.active_tab = OptionsTab::Video;
        menu.selected_index = 20;
        let (label, val) = menu.item_display(20).unwrap();
        assert_eq!(label, "FMV Cutscene Source");
        assert_eq!(val, "Auto (4K VSR if detected)");

        menu.adjust_value(1);
        assert_eq!(
            menu.draft_config.video.fmv_source,
            sh2_core::config::FmvCutsceneSource::Upscaled4kMp4
        );

        let mut ps2_cfg = AppConfig::default();
        ps2_cfg.apply_enhanced_preset(sh2_core::config::EnhancedPreset::Ps2Authentic);
        assert_eq!(
            ps2_cfg.video.fmv_source,
            sh2_core::config::FmvCutsceneSource::VanillaPc
        );
    }
}
