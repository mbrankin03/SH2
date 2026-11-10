//! Category tab definitions, display formatting, and value adjustment logic for [`super::OptionsMenu`].

use super::{OptionsMenu, OptionsSoundCue, OptionsTab};

impl OptionsMenu {
    /// Number of items in the currently active tab.
    pub fn item_count(&self) -> usize {
        match self.active_tab {
            OptionsTab::Video => 11,
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
        self.active_tab == OptionsTab::Video && self.selected_index == 10
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
            OptionsTab::Video => self.video_item_display(index),
            OptionsTab::Audio => self.audio_item_display(index),
            OptionsTab::Controls => self.controls_item_display(index),
            OptionsTab::Game => self.game_item_display(index),
            OptionsTab::Debug => self.debug_item_display(index),
        }
    }

    fn video_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let v = &self.draft_config.video;
        match index {
            0 => Some(("Resolution", v.resolution.label().to_string())),
            1 => Some(("Display Mode", v.display_mode.label().to_string())),
            2 => Some((
                "V-Sync",
                if v.vsync {
                    "Enabled".into()
                } else {
                    "Disabled".into()
                },
            )),
            3 => Some(("Frame Rate Cap", v.framerate_cap.label().to_string())),
            4 => Some(("Fog Quality", v.fog_quality.label().to_string())),
            5 => Some(("Film Grain / Noise", format!("{}%", v.grain_intensity))),
            6 => Some((
                "Volumetric Lighting",
                if v.volumetric_light {
                    "ON".into()
                } else {
                    "OFF".into()
                },
            )),
            7 => Some((
                "Lens Flare",
                if v.lens_flare {
                    "ON".into()
                } else {
                    "OFF".into()
                },
            )),
            8 => Some((
                "Depth of Field",
                if v.depth_of_field {
                    "ON".into()
                } else {
                    "OFF".into()
                },
            )),
            9 => Some((
                "World & Character Textures",
                v.texture_quality.label().to_string(),
            )),
            10 => Some((
                "FMV Cutscene Viewer",
                "[Press Enter / Right to Launch]".into(),
            )),
            _ => None,
        }
    }

    fn audio_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let a = &self.draft_config.audio;
        match index {
            0 => Some(("Master Volume", format!("{:.0}%", a.master_volume * 100.0))),
            1 => Some(("BGM Volume", format!("{:.0}%", a.bgm_volume * 100.0))),
            2 => Some(("SFX Volume", format!("{:.0}%", a.sfx_volume * 100.0))),
            3 => Some((
                "Dialogue Volume",
                format!("{:.0}%", a.dialogue_volume * 100.0),
            )),
            4 => Some((
                "Output Device",
                a.selected_output_device
                    .clone()
                    .unwrap_or_else(|| "Default (System)".into()),
            )),
            5 => Some(("Speaker Config", a.speaker_config.label().to_string())),
            6 => Some((
                "Mute in Background",
                if a.mute_in_background {
                    "YES".into()
                } else {
                    "NO".into()
                },
            )),
            7 => Some((
                "Restored Cut Audio",
                if a.enable_restored_cut_audio {
                    "ON (Restored)".into()
                } else {
                    "OFF (Vanilla)".into()
                },
            )),
            8 if self.audio_pack_status.both_detected() => {
                Some(("Audio Pack", a.audio_pack_source.label().to_string()))
            }
            _ => None,
        }
    }

    fn controls_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let c = &self.draft_config.controls;
        match index {
            0 => Some((
                "Movement Style",
                if c.tank_controls {
                    "3D Tank (Authentic)".into()
                } else {
                    "2D Directional (Modern)".into()
                },
            )),
            1 => Some(("Button Prompt Glyphs", c.glyph_style.label().to_string())),
            2 => Some((
                "Stick Deadzone",
                format!("{:.0}%", c.stick_deadzone * 100.0),
            )),
            3 => Some(("Stick Sensitivity", format!("{:.1}x", c.stick_sensitivity))),
            4 => Some((
                "Vibration Intensity",
                format!("{:.0}%", c.vibration_intensity * 100.0),
            )),
            _ => None,
        }
    }

    fn game_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let g = &self.draft_config.game;
        match index {
            0 => Some(("Action Difficulty", g.action_difficulty.label().to_string())),
            1 => Some(("Riddle Difficulty", g.riddle_difficulty.label().to_string())),
            2 => Some((
                "Subtitles",
                if g.subtitles {
                    "ON".into()
                } else {
                    "OFF".into()
                },
            )),
            3 => Some(("Language", g.language.label().to_string())),
            4 => Some(("Bullet Multiplier", g.bullet_multiplier.label().to_string())),
            5 => Some((
                "Save Anywhere",
                if g.save_anywhere {
                    "ENABLED (PC)".into()
                } else {
                    "DISABLED (PS2)".into()
                },
            )),
            6 => Some(("Test Rooms", "[Press Enter to Select]".into())),
            _ => None,
        }
    }

    fn debug_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let d = &self.draft_config.debug;
        match index {
            0 => Some((
                "Instant Room Cuts",
                if d.instant_room_transitions {
                    "ON (Fast / Skip)".into()
                } else {
                    "OFF (Authentic)".into()
                },
            )),
            1 => Some((
                "Min Loading Display",
                format!("{} seconds", d.min_loading_screen_seconds),
            )),
            2 => Some((
                "Prompt To Continue",
                if d.require_press_to_continue {
                    "ENABLED (Press Key)".into()
                } else {
                    "AUTO (Fade In)".into()
                },
            )),
            3 => Some(("Loading Screen Preview", "[Press Enter to Test]".into())),
            4 => Some(("Test Rooms Launcher", "[Press Enter to Open]".into())),
            5 => Some((
                "Collision Wireframe",
                if d.show_collision_wireframe {
                    "VISIBLE".into()
                } else {
                    "HIDDEN".into()
                },
            )),
            _ => None,
        }
    }

    /// Adjust value of currently selected setting left (-1) or right (+1).
    pub fn adjust_value(&mut self, direction: i32) -> OptionsSoundCue {
        let forward = direction >= 0;

        let special_cue = match self.active_tab {
            OptionsTab::Video => self.adjust_video_value(forward),
            OptionsTab::Audio => self.adjust_audio_value(forward),
            OptionsTab::Controls => self.adjust_controls_value(forward),
            OptionsTab::Game => self.adjust_game_value(forward),
            OptionsTab::Debug => self.adjust_debug_value(forward),
        };

        if let Some(cue) = special_cue {
            return cue;
        }

        self.is_dirty = self.draft_config != self.original_config;
        OptionsSoundCue::ValueChange
    }

    fn adjust_video_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        let v = &mut self.draft_config.video;
        match self.selected_index {
            0 => {
                v.resolution = if forward {
                    v.resolution.next()
                } else {
                    v.resolution.prev()
                };
            }
            1 => {
                v.display_mode = if forward {
                    v.display_mode.next()
                } else {
                    v.display_mode.prev()
                };
            }
            2 => v.vsync = !v.vsync,
            3 => {
                v.framerate_cap = if forward {
                    v.framerate_cap.next()
                } else {
                    v.framerate_cap.prev()
                };
            }
            4 => v.fog_quality = v.fog_quality.toggle(),
            5 => {
                let step = if forward { 5 } else { -5 };
                v.grain_intensity = (v.grain_intensity as i32 + step).clamp(0, 100) as u32;
            }
            6 => v.volumetric_light = !v.volumetric_light,
            7 => v.lens_flare = !v.lens_flare,
            8 => v.depth_of_field = !v.depth_of_field,
            9 => {
                v.texture_quality = if forward {
                    v.texture_quality.next()
                } else {
                    v.texture_quality.prev()
                };
            }
            10 => return Some(OptionsSoundCue::LaunchFmvViewer),
            _ => return Some(OptionsSoundCue::None),
        }
        None
    }

    fn adjust_audio_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        let a = &mut self.draft_config.audio;
        match self.selected_index {
            0 => {
                let step = if forward { 0.05 } else { -0.05 };
                a.master_volume = (a.master_volume + step).clamp(0.0, 1.0);
            }
            1 => {
                let step = if forward { 0.05 } else { -0.05 };
                a.bgm_volume = (a.bgm_volume + step).clamp(0.0, 1.0);
            }
            2 => {
                let step = if forward { 0.05 } else { -0.05 };
                a.sfx_volume = (a.sfx_volume + step).clamp(0.0, 1.0);
            }
            3 => {
                let step = if forward { 0.05 } else { -0.05 };
                a.dialogue_volume = (a.dialogue_volume + step).clamp(0.0, 1.0);
            }
            4 => {
                if !self.audio_devices.is_empty() {
                    let cur_name = a
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
                        a.selected_output_device = None;
                    } else {
                        a.selected_output_device = Some(chosen.clone());
                    }
                }
            }
            5 => {
                a.speaker_config = if forward {
                    a.speaker_config.next()
                } else {
                    a.speaker_config.prev()
                };
            }
            6 => a.mute_in_background = !a.mute_in_background,
            7 => a.enable_restored_cut_audio = !a.enable_restored_cut_audio,
            8 if self.audio_pack_status.both_detected() => {
                a.audio_pack_source = if forward {
                    a.audio_pack_source.next()
                } else {
                    a.audio_pack_source.prev()
                };
            }
            _ => return Some(OptionsSoundCue::None),
        }
        None
    }

    fn adjust_controls_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        let c = &mut self.draft_config.controls;
        match self.selected_index {
            0 => c.tank_controls = !c.tank_controls,
            1 => {
                c.glyph_style = if forward {
                    c.glyph_style.next()
                } else {
                    c.glyph_style.prev()
                };
            }
            2 => {
                let step = if forward { 0.05 } else { -0.05 };
                c.stick_deadzone = (c.stick_deadzone + step).clamp(0.0, 0.5);
            }
            3 => {
                let step = if forward { 0.1 } else { -0.1 };
                c.stick_sensitivity = (c.stick_sensitivity + step).clamp(0.5, 3.0);
            }
            4 => {
                let step = if forward { 0.1 } else { -0.1 };
                c.vibration_intensity = (c.vibration_intensity + step).clamp(0.0, 1.0);
            }
            _ => return Some(OptionsSoundCue::None),
        }
        None
    }

    fn adjust_game_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        let g = &mut self.draft_config.game;
        match self.selected_index {
            0 => {
                g.action_difficulty = if forward {
                    g.action_difficulty.next()
                } else {
                    g.action_difficulty.prev()
                };
            }
            1 => {
                g.riddle_difficulty = if forward {
                    g.riddle_difficulty.next()
                } else {
                    g.riddle_difficulty.prev()
                };
            }
            2 => g.subtitles = !g.subtitles,
            3 => {
                g.language = if forward {
                    g.language.next()
                } else {
                    g.language.prev()
                };
            }
            4 => {
                g.bullet_multiplier = if forward {
                    g.bullet_multiplier.next()
                } else {
                    g.bullet_multiplier.prev()
                };
            }
            5 => g.save_anywhere = !g.save_anywhere,
            6 => return Some(OptionsSoundCue::LaunchTestRooms),
            _ => return Some(OptionsSoundCue::None),
        }
        None
    }

    fn adjust_debug_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        let d = &mut self.draft_config.debug;
        match self.selected_index {
            0 => d.instant_room_transitions = !d.instant_room_transitions,
            1 => {
                let cur = d.min_loading_screen_seconds as i32;
                let step = if forward { 1 } else { -1 };
                d.min_loading_screen_seconds = (cur + step).clamp(0, 10) as u32;
            }
            2 => d.require_press_to_continue = !d.require_press_to_continue,
            3 => return Some(OptionsSoundCue::LaunchLoadingTest),
            4 => return Some(OptionsSoundCue::LaunchTestRooms),
            5 => d.show_collision_wireframe = !d.show_collision_wireframe,
            _ => return Some(OptionsSoundCue::None),
        }
        None
    }
}
