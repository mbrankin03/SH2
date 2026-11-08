//! In-Game Options Menu replicating SH2:EE (Enhanced Edition) advanced options architecture.
//! References: `Patches/CustomAdvancedOptions.cpp`, `Patches/OptionsMenuTweaks.cpp`.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
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

    /// Number of items in the currently active tab.
    pub fn item_count(&self) -> usize {
        match self.active_tab {
            OptionsTab::Video => 10,
            OptionsTab::Audio => {
                if self.audio_pack_status.both_detected() {
                    9
                } else {
                    8
                }
            }
            OptionsTab::Controls => 5,
            OptionsTab::Game => 7,
            OptionsTab::Debug => 6,
        }
    }

    /// Returns true if the currently selected row is the interactive FMV Cutscene Viewer launcher.
    pub fn is_fmv_viewer_selected(&self) -> bool {
        self.active_tab == OptionsTab::Video && self.selected_index == 9
    }

    /// Returns true if the currently selected row is the Test Rooms launcher.
    pub fn is_test_rooms_selected(&self) -> bool {
        (self.active_tab == OptionsTab::Game && self.selected_index == 6)
            || (self.active_tab == OptionsTab::Debug && self.selected_index == 4)
    }

    /// Returns true if the currently selected row is the Loading Screen Preview launcher.
    pub fn is_loading_test_selected(&self) -> bool {
        self.active_tab == OptionsTab::Debug && self.selected_index == 3
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
                    "Output Device",
                    self.draft_config
                        .audio
                        .selected_output_device
                        .clone()
                        .unwrap_or_else(|| "Default (System)".into()),
                )),
                5 => Some((
                    "Speaker Config",
                    self.draft_config.audio.speaker_config.label().to_string(),
                )),
                6 => Some((
                    "Mute in Background",
                    if self.draft_config.audio.mute_in_background {
                        "YES".into()
                    } else {
                        "NO".into()
                    },
                )),
                7 => Some((
                    "Restored Cut Audio",
                    if self.draft_config.audio.enable_restored_cut_audio {
                        "ON (Restored)".into()
                    } else {
                        "OFF (Vanilla)".into()
                    },
                )),
                8 if self.audio_pack_status.both_detected() => Some((
                    "Audio Pack",
                    self.draft_config
                        .audio
                        .audio_pack_source
                        .label()
                        .to_string(),
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
                5 => Some((
                    "Save Anywhere",
                    if self.draft_config.game.save_anywhere {
                        "ENABLED (PC)".into()
                    } else {
                        "DISABLED (PS2)".into()
                    },
                )),
                6 => Some(("Test Rooms", "[Press Enter to Select]".into())),
                _ => None,
            },
            OptionsTab::Debug => match index {
                0 => Some((
                    "Instant Room Cuts",
                    if self.draft_config.debug.instant_room_transitions {
                        "ON (Fast / Skip)".into()
                    } else {
                        "OFF (Authentic)".into()
                    },
                )),
                1 => Some((
                    "Min Loading Display",
                    format!(
                        "{} seconds",
                        self.draft_config.debug.min_loading_screen_seconds
                    ),
                )),
                2 => Some((
                    "Prompt To Continue",
                    if self.draft_config.debug.require_press_to_continue {
                        "ENABLED (Press Key)".into()
                    } else {
                        "AUTO (Fade In)".into()
                    },
                )),
                3 => Some(("Loading Screen Preview", "[Press Enter to Test]".into())),
                4 => Some(("Test Rooms Launcher", "[Press Enter to Open]".into())),
                5 => Some((
                    "Collision Wireframe",
                    if self.draft_config.debug.show_collision_wireframe {
                        "VISIBLE".into()
                    } else {
                        "HIDDEN".into()
                    },
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
                    // Output device selection
                    if !self.audio_devices.is_empty() {
                        let cur_name = self
                            .draft_config
                            .audio
                            .selected_output_device
                            .as_deref()
                            .unwrap_or("Default (System)");
                        let cur_idx = self
                            .audio_devices
                            .iter()
                            .position(|d| d == cur_name)
                            .unwrap_or(0);
                        let next_idx = if forward {
                            (cur_idx + 1) % self.audio_devices.len()
                        } else if cur_idx == 0 {
                            self.audio_devices.len() - 1
                        } else {
                            cur_idx - 1
                        };
                        let chosen = &self.audio_devices[next_idx];
                        if chosen == "Default (System)" {
                            self.draft_config.audio.selected_output_device = None;
                        } else {
                            self.draft_config.audio.selected_output_device = Some(chosen.clone());
                        }
                    }
                }
                5 => {
                    self.draft_config.audio.speaker_config = if forward {
                        self.draft_config.audio.speaker_config.next()
                    } else {
                        self.draft_config.audio.speaker_config.prev()
                    };
                }
                6 => {
                    self.draft_config.audio.mute_in_background =
                        !self.draft_config.audio.mute_in_background;
                }
                7 => {
                    self.draft_config.audio.enable_restored_cut_audio =
                        !self.draft_config.audio.enable_restored_cut_audio;
                }
                8 if self.audio_pack_status.both_detected() => {
                    self.draft_config.audio.audio_pack_source = if forward {
                        self.draft_config.audio.audio_pack_source.next()
                    } else {
                        self.draft_config.audio.audio_pack_source.prev()
                    };
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
                5 => {
                    self.draft_config.game.save_anywhere = !self.draft_config.game.save_anywhere;
                }
                6 => return OptionsSoundCue::LaunchTestRooms,
                _ => return OptionsSoundCue::None,
            },
            OptionsTab::Debug => match self.selected_index {
                0 => {
                    self.draft_config.debug.instant_room_transitions =
                        !self.draft_config.debug.instant_room_transitions;
                }
                1 => {
                    let cur = self.draft_config.debug.min_loading_screen_seconds as i32;
                    let step = if forward { 1 } else { -1 };
                    self.draft_config.debug.min_loading_screen_seconds =
                        (cur + step).clamp(0, 10) as u32;
                }
                2 => {
                    self.draft_config.debug.require_press_to_continue =
                        !self.draft_config.debug.require_press_to_continue;
                }
                3 => return OptionsSoundCue::LaunchLoadingTest,
                4 => return OptionsSoundCue::LaunchTestRooms,
                5 => {
                    self.draft_config.debug.show_collision_wireframe =
                        !self.draft_config.debug.show_collision_wireframe;
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

// -----------------------------------------------------------------------------------------
// Authentic Proportional Options Screen Rasterization (`AtlasFont`)
// -----------------------------------------------------------------------------------------

/// Draws a convex polygon with optional fill and outline colors directly onto `rast`.
pub fn draw_polygon(
    rast: &mut TextRasterizer,
    points: &[(i32, i32)],
    fill: Option<[u8; 4]>,
    outline: Option<[u8; 4]>,
) {
    if points.len() < 3 {
        return;
    }

    let min_y = points.iter().map(|p| p.1).min().unwrap_or(0).max(0) as usize;
    let max_y = points
        .iter()
        .map(|p| p.1)
        .max()
        .unwrap_or(0)
        .min(rast.height as i32 - 1) as usize;

    if let Some(fill_color) = fill {
        let n = points.len();
        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let mut nodes = Vec::new();
            for i in 0..n {
                let j = (i + 1) % n;
                let (x0, y0) = (points[i].0 as f32, points[i].1 as f32);
                let (x1, y1) = (points[j].0 as f32, points[j].1 as f32);

                if (y0 < py && y1 >= py) || (y1 < py && y0 >= py) {
                    let intersect_x = x0 + (py - y0) / (y1 - y0) * (x1 - x0);
                    nodes.push(intersect_x);
                }
            }
            nodes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for chunk in nodes.chunks(2) {
                if chunk.len() == 2 {
                    let start_x = chunk[0].round().max(0.0) as usize;
                    let end_x = (chunk[1].round() as usize).min(rast.width - 1);
                    for x in start_x..=end_x {
                        let idx = (y * rast.width + x) * 4;
                        // Alpha composite
                        let s_a = fill_color[3] as f32 / 255.0;
                        let d_a = rast.pixels[idx + 3] as f32 / 255.0;
                        let out_a = s_a + d_a * (1.0 - s_a);
                        if out_a > 0.0 {
                            let r = (fill_color[0] as f32 * s_a
                                + rast.pixels[idx] as f32 * d_a * (1.0 - s_a))
                                / out_a;
                            let g = (fill_color[1] as f32 * s_a
                                + rast.pixels[idx + 1] as f32 * d_a * (1.0 - s_a))
                                / out_a;
                            let b = (fill_color[2] as f32 * s_a
                                + rast.pixels[idx + 2] as f32 * d_a * (1.0 - s_a))
                                / out_a;
                            rast.pixels[idx] = r.clamp(0.0, 255.0) as u8;
                            rast.pixels[idx + 1] = g.clamp(0.0, 255.0) as u8;
                            rast.pixels[idx + 2] = b.clamp(0.0, 255.0) as u8;
                            rast.pixels[idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }

    if let Some(outline_color) = outline {
        let n = points.len();
        for i in 0..n {
            let j = (i + 1) % n;
            draw_thick_line(
                rast,
                points[i].0,
                points[i].1,
                points[j].0,
                points[j].1,
                2,
                outline_color,
            );
        }
    }
}

/// Bresenham's line algorithm with thickness.
fn draw_thick_line(
    rast: &mut TextRasterizer,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    thickness: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut cx = x0;
    let mut cy = y0;
    let radius = thickness / 2;

    loop {
        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let px = cx + ox;
                let py = cy + oy;
                if px >= 0 && px < rast.width as i32 && py >= 0 && py < rast.height as i32 {
                    let idx = (py as usize * rast.width + px as usize) * 4;
                    let s_a = color[3] as f32 / 255.0;
                    let d_a = rast.pixels[idx + 3] as f32 / 255.0;
                    let out_a = s_a + d_a * (1.0 - s_a);
                    if out_a > 0.0 {
                        let r = (color[0] as f32 * s_a
                            + rast.pixels[idx] as f32 * d_a * (1.0 - s_a))
                            / out_a;
                        let g = (color[1] as f32 * s_a
                            + rast.pixels[idx + 1] as f32 * d_a * (1.0 - s_a))
                            / out_a;
                        let b = (color[2] as f32 * s_a
                            + rast.pixels[idx + 2] as f32 * d_a * (1.0 - s_a))
                            / out_a;
                        rast.pixels[idx] = r.clamp(0.0, 255.0) as u8;
                        rast.pixels[idx + 1] = g.clamp(0.0, 255.0) as u8;
                        rast.pixels[idx + 2] = b.clamp(0.0, 255.0) as u8;
                        rast.pixels[idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            cx += sx;
        }
        if e2 < dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Draws an authentic 3D beveled volume block (15 total blocks matching `0x45bf10`).
fn draw_volume_block(rast: &mut TextRasterizer, x: i32, y: i32, lit: bool) {
    let w = 8;
    let h = 23;

    let (top_fill, bottom_fill, border_light, border_dark) = if lit {
        (
            [176u8, 176, 176, 255],
            [137u8, 137, 137, 255],
            [220u8, 220, 220, 255],
            [80u8, 80, 80, 255],
        )
    } else {
        (
            [80u8, 80, 80, 255],
            [60u8, 60, 60, 255],
            [100u8, 100, 100, 255],
            [40u8, 40, 40, 255],
        )
    };

    for dy in 0..h {
        let py = y + dy;
        if py < 0 || py >= rast.height as i32 {
            continue;
        }
        for dx in 0..w {
            let px = x + dx;
            if px < 0 || px >= rast.width as i32 {
                continue;
            }
            let idx = (py as usize * rast.width + px as usize) * 4;

            let c = if dx == 0 || dy == 0 {
                border_light
            } else if dx == w - 1 || dy == h - 1 {
                border_dark
            } else if dy < h / 2 {
                top_fill
            } else {
                bottom_fill
            };

            rast.pixels[idx] = c[0];
            rast.pixels[idx + 1] = c[1];
            rast.pixels[idx + 2] = c[2];
            rast.pixels[idx + 3] = c[3];
        }
    }
}

/// Contextual description for an option row based on active tab and item name.
fn option_help_text(tab: OptionsTab, name: &str) -> &'static str {
    match (tab, name) {
        (OptionsTab::Video, "Resolution") => "Internal rendering resolution for 3D world",
        (OptionsTab::Video, "Display Mode") => {
            "Display window mode (Windowed, Borderless, Fullscreen)"
        }
        (OptionsTab::Video, "V-Sync") => "Synchronize frame rate with display refresh rate",
        (OptionsTab::Video, "Frame Rate Cap") => "Maximum frame rate limit for game simulation",
        (OptionsTab::Video, "Fog Quality") => "Volumetric 3D fog raymarching and density quality",
        (OptionsTab::Video, "Film Grain / Noise") => "Authentic film grain noise filter intensity",
        (OptionsTab::Video, "Volumetric Lighting") => {
            "Dynamic volumetric light scattering from streetlamps and flashlight"
        }
        (OptionsTab::Video, "Lens Flare") => "Camera lens flare effects on bright light sources",
        (OptionsTab::Video, "Depth of Field") => {
            "Cinematic camera focal blur and distance depth of field"
        }
        (OptionsTab::Video, "FMV Cutscene Viewer") => {
            "Launch the interactive 1080p60 cutscene theater"
        }

        (OptionsTab::Audio, "Master Volume") => "Overall master sound and music volume level",
        (OptionsTab::Audio, "BGM Volume") => "Background music volume level",
        (OptionsTab::Audio, "SFX Volume") => "Sound effects and monster audio volume level",
        (OptionsTab::Audio, "Dialogue Volume") => {
            "Spoken character dialogue and voice volume level"
        }
        (OptionsTab::Audio, "Output Device") => {
            "Select active audio hardware output device (Speakers, Headphones, Interface)"
        }
        (OptionsTab::Audio, "Speaker Config") => {
            "Audio channel configuration (Stereo, Surround, Headphones)"
        }
        (OptionsTab::Audio, "Mute in Background") => "Silence audio when game window loses focus",
        (OptionsTab::Audio, "Restored Cut Audio") => {
            "Restore cut audio cues (Mary's Laura letter voiceover, closet & street bonus SFX)"
        }
        (OptionsTab::Audio, "Audio Pack") => {
            "Select BGM & SFX audio source (Auto, SH2:EE 48 kHz Lossless, or Original PS2/PC)"
        }

        (OptionsTab::Controls, "Movement Style") => {
            "Character control scheme (3D Tank or 2D Directional)"
        }
        (OptionsTab::Controls, "Button Prompt Glyphs") => {
            "Gamepad button prompt style (PlayStation, Xbox, Keyboard)"
        }
        (OptionsTab::Controls, "Stick Deadzone") => "Analog stick center deadzone threshold",
        (OptionsTab::Controls, "Stick Sensitivity") => {
            "Analog stick camera and rotation sensitivity"
        }
        (OptionsTab::Controls, "Vibration Intensity") => {
            "Gamepad haptic vibration and rumble intensity"
        }

        (OptionsTab::Game, "Action Difficulty") => {
            "Combat challenge and monster durability difficulty"
        }
        (OptionsTab::Game, "Riddle Difficulty") => {
            "Puzzle complexity and riddle challenge difficulty"
        }
        (OptionsTab::Game, "Subtitles") => "Display spoken dialogue subtitles on screen",
        (OptionsTab::Game, "Language") => "Select in-game text and subtitle language",
        (OptionsTab::Game, "Bullet Multiplier") => {
            "Multiplier for ammunition acquired from pickups"
        }
        (OptionsTab::Game, "Save Anywhere") => {
            "Allow saving anytime via pause menu (PC) or restrict to Red Squares (PS2)"
        }
        (OptionsTab::Game, "Test Rooms") => {
            "Select debug sandbox stage, fog calibration, or model viewer"
        }

        (OptionsTab::Debug, "Instant Room Cuts") => {
            "Speedrunner mode: cut instantly between rooms without loading screen"
        }
        (OptionsTab::Debug, "Min Loading Display") => {
            "Minimum duration to hold animated footstep loading screen (seconds)"
        }
        (OptionsTab::Debug, "Prompt To Continue") => {
            "Require pressing Enter / A to continue once loading completes"
        }
        (OptionsTab::Debug, "Loading Screen Preview") => {
            "Preview and test authentic PS2 animated footstep loading screen"
        }
        (OptionsTab::Debug, "Test Rooms Launcher") => {
            "Select debug sandbox stage, fog calibration, or model viewer"
        }
        (OptionsTab::Debug, "Collision Wireframe") => {
            "Render authentic CLD collision polygons and wall segments in world"
        }

        _ => "Adjust option settings",
    }
}

/// Draws the authentic options screen matching retail PC / PS2 `optmenu.c` (`0x45e750`, `0x461600`)
/// hosting our modern options (Resolution, Fog, Noise, Display Mode, Audio, Controls, Game).
#[allow(clippy::excessive_precision)]
pub fn draw_options_screen(menu: &OptionsMenu, font: &AtlasFont, rast: &mut TextRasterizer) {
    // Keep 4:3 authentic aspect ratio inside widescreen rasterizer
    let target_4_3_w = rast.height as f32 * (4.0 / 3.0);
    let (x_offset, base_w) = if target_4_3_w <= rast.width as f32 {
        ((rast.width as f32 - target_4_3_w) * 0.5, target_4_3_w)
    } else {
        (0.0, rast.width as f32)
    };

    let scale_x = base_w / 640.0;
    let scale_y = rast.height as f32 / 480.0;

    // Authentic PS2 octagon containers calibrated from retail PS2 SLUS-20228 / optmenu.c 0x45e750
    // Upper options panel: x in [36, 604], y in [64, 407] with 45-degree chamfers
    let panel_pts: Vec<(i32, i32)> = [
        (48.0, 64.0),
        (593.0, 64.0),
        (604.0, 76.0),
        (604.0, 407.0),
        (36.0, 407.0),
        (36.0, 76.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    // Lower contextual help box: x in [36, 604], y in [414, 461] with bottom chamfers
    let box_pts: Vec<(i32, i32)> = [
        (36.0, 414.0),
        (604.0, 414.0),
        (604.0, 436.0),
        (580.0, 461.0),
        (60.0, 461.0),
        (36.0, 436.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    // Outer double-border outline enclosing both panels: x in [30, 612], y in [58, 467]
    let outline_pts: Vec<(i32, i32)> = [
        (55.0, 58.0),
        (586.0, 58.0),
        (612.0, 83.0),
        (612.0, 440.0),
        (586.0, 467.0),
        (55.0, 467.0),
        (30.0, 440.0),
        (30.0, 83.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    // Authentic PS2 colors (calibrated for linear Rgba8Unorm rasterization blit to sRGB surface):
    // Fill: Dark charcoal grey matching PS2 optmenu.c ([55, 55, 55] sRGB output -> [12, 12, 13, 255] linear)
    let fill_color = [12, 12, 13, 255];
    // Border line: Deep navy/blue ([32, 32, 75] sRGB output -> [4, 4, 18, 255] linear)
    let border_color = [4, 4, 18, 255];

    draw_polygon(rast, &panel_pts, Some(fill_color), Some(border_color));
    draw_polygon(rast, &box_pts, Some(fill_color), Some(border_color));
    draw_polygon(rast, &outline_pts, None, Some(border_color));

    let title_x = rast.width / 2;
    let title_y = (28.0 * scale_y).round() as usize;

    // Authentic PS2 Title Header with category indicator (matching optmenu.c line 547):
    // e.g. "<   V I D E O   O P T I O N S   >"
    let category_header = match menu.active_tab {
        OptionsTab::Video => "<   V I D E O   O P T I O N S   >",
        OptionsTab::Audio => "<   A U D I O   O P T I O N S   >",
        OptionsTab::Controls => "<   C O N T R O L   O P T I O N S   >",
        OptionsTab::Game => "<   G A M E   O P T I O N S   >",
        OptionsTab::Debug => "<   D E B U G   T O O L S   >",
    };

    font.draw_line_centered(
        rast,
        category_header,
        (title_x, title_y),
        (22.0 * scale_y).round() as usize,
        Sh2TextColor::White,
        true,
    );

    // Option rows positioning matching PS2 optmenu.c:
    // Base y = 84px in 480p, step y = 30px
    let base_y = 84.0 * scale_y;
    let step_y = 29.5 * scale_y;

    let center_panel_x = (x_offset + 320.0 * scale_x).round() as usize;
    let label_right_x = (x_offset + 300.0 * scale_x).round() as usize;
    let val_left_x = (x_offset + 336.0 * scale_x).round() as usize;
    let font_size = (18.0 * scale_y).round() as usize;

    let item_count = menu.item_count();

    for idx in 0..item_count {
        if let Some((label, val)) = menu.item_display(idx) {
            let y = (base_y + idx as f32 * step_y).round() as usize;
            let is_selected = idx == menu.selected_index;

            let style = if is_selected {
                Sh2TextColor::White
            } else {
                Sh2TextColor::SilverUnselected
            };

            // Check if this item is a volume slider
            let is_slider = menu.active_tab == OptionsTab::Audio
                && (label == "BGM Volume"
                    || label == "SFX Volume"
                    || label == "Master Volume"
                    || label == "Dialogue Volume");

            if label == "FMV Cutscene Viewer"
                || label == "Test Rooms"
                || label == "Loading Screen Preview"
                || label == "Test Rooms Launcher"
            {
                // Centered special action item
                if is_selected {
                    font.draw_line_centered(
                        rast,
                        label,
                        (center_panel_x.saturating_sub(2), y.saturating_sub(2)),
                        font_size,
                        Sh2TextColor::White,
                        false,
                    );
                }
                font.draw_line_centered(rast, label, (center_panel_x, y), font_size, style, true);
            } else {
                // Label right-aligned to center divider line
                let label_w = font.text_width(label, font_size);
                let lx = label_right_x.saturating_sub(label_w);

                if is_selected {
                    // Double-draw at (-2, -2) for authentic PS2 selected pop highlight (0x461600)
                    font.draw_line_styled(
                        rast,
                        label,
                        (lx.saturating_sub(2), y.saturating_sub(2)),
                        font_size,
                        Sh2TextColor::White,
                        false,
                    );
                }
                font.draw_line_styled(rast, label, (lx, y), font_size, style, true);

                if is_slider {
                    // Render 15-segment volume blocks
                    let vol_pct = match label {
                        "Master Volume" => menu.draft_config.audio.master_volume,
                        "BGM Volume" => menu.draft_config.audio.bgm_volume,
                        "SFX Volume" => menu.draft_config.audio.sfx_volume,
                        "Dialogue Volume" => menu.draft_config.audio.dialogue_volume,
                        _ => 1.0,
                    };
                    let active_blocks = (vol_pct * 15.0).round() as usize;
                    let block_x_start = val_left_x as i32;
                    let block_y = (y as f32 + 1.0 * scale_y).round() as i32;

                    for b in 0..15 {
                        let bx = block_x_start + (b as f32 * 14.0 * scale_x).round() as i32;
                        let lit = b < active_blocks;
                        draw_volume_block(rast, bx, block_y, lit);
                    }
                } else {
                    // Render string value with selection highlight
                    if is_selected {
                        font.draw_line_styled(
                            rast,
                            &val,
                            (val_left_x.saturating_sub(2), y.saturating_sub(2)),
                            font_size,
                            Sh2TextColor::White,
                            false,
                        );
                    }
                    font.draw_line_styled(rast, &val, (val_left_x, y), font_size, style, true);
                }
            }
        }
    }

    // Help description inside the lower box at x=90, y=438 in 640x480 space
    let help_x = (x_offset + 90.0 * scale_x).round() as usize;
    let help_y = (436.0 * scale_y).round() as usize;

    let selected_label = menu
        .item_display(menu.selected_index)
        .map(|(l, _)| l)
        .unwrap_or("");

    let help_text = option_help_text(menu.active_tab, selected_label);

    font.draw_line_styled(
        rast,
        help_text,
        (help_x, help_y),
        font_size,
        Sh2TextColor::White,
        true,
    );
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
