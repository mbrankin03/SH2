//! Category tab definitions, display formatting, and value adjustment logic for [`super::OptionsMenu`].

use std::borrow::Cow;

use super::{OptionsMenu, OptionsSoundCue, OptionsSubScreen, OptionsTab};
use sh2_core::config::{ps2_level_to_volume_gain, volume_gain_to_ps2_level, PS2_VOLUME_MAX_LEVEL};

impl OptionsMenu {
    /// Number of items in the currently active tab.
    pub fn item_count(&self) -> usize {
        match self.active_tab {
            OptionsTab::Root => 11,
            OptionsTab::Video => 21,
            OptionsTab::Audio => {
                if self.audio_pack_status.both_detected() {
                    12
                } else {
                    11
                }
            }
            OptionsTab::Controls => 17,
            OptionsTab::Game => 13,
            OptionsTab::Enhanced => {
                if self.audio_pack_status.both_detected() {
                    21
                } else {
                    20
                }
            }
            OptionsTab::Mods => self.mods.len().max(1),
            OptionsTab::Debug => 6,
        }
    }

    /// Returns true if the currently selected row is the interactive FMV Cutscene Viewer launcher.
    pub fn is_fmv_viewer_selected(&self) -> bool {
        self.active_tab == OptionsTab::Video && self.selected_index == 10
    }

    /// Returns true if the currently selected row is the Test Rooms launcher.
    pub fn is_test_rooms_selected(&self) -> bool {
        (self.active_tab == OptionsTab::Game && self.selected_index == 7)
            || (self.active_tab == OptionsTab::Debug && self.selected_index == 4)
    }

    /// Returns true if the currently selected row is the Loading Screen Preview launcher.
    pub fn is_loading_test_selected(&self) -> bool {
        self.active_tab == OptionsTab::Debug && self.selected_index == 3
    }

    /// Retrieve the title and string value of an item in the active tab.
    pub fn item_display(&self, index: usize) -> Option<(&'static str, String)> {
        match self.active_tab {
            OptionsTab::Root => self.root_item_display(index),
            OptionsTab::Video => self.video_item_display(index),
            OptionsTab::Audio => self.audio_item_display(index),
            OptionsTab::Controls => self.controls_item_display(index),
            OptionsTab::Game => self.game_item_display(index),
            OptionsTab::Enhanced => self.enhanced_item_display(index),
            OptionsTab::Mods => {
                if self.mods.is_empty() && index == 0 {
                    Some(("Installed Mods", "None (Drop into mods/)".into()))
                } else {
                    None
                }
            }
            OptionsTab::Debug => self.debug_item_display(index),
        }
    }

    /// Retrieve the title (supporting dynamic mod names) and string value of an item in the active tab.
    pub fn item_display_cow(&self, index: usize) -> Option<(Cow<'static, str>, String)> {
        if self.active_tab == OptionsTab::Mods && !self.mods.is_empty() {
            return self.mods.get(index).map(|m| {
                let state = if m.enabled { "ENABLED" } else { "DISABLED" };
                (
                    Cow::Owned(m.name.clone()),
                    format!("{state}  Load #{}", index + 1),
                )
            });
        }
        self.item_display(index)
            .map(|(label, val)| (Cow::Borrowed(label), val))
    }

    /// Formats the bottom description box text for the `MOD MANAGER` tab (`SH2-29`).
    #[must_use]
    pub fn mod_help_text(&self, index: usize) -> String {
        let Some(m) = self.mods.get(index) else {
            return "Drop mod folders or .zip archives into mods/ to install.".to_string();
        };
        if m.description.is_empty() {
            format!(
                "v{} by {}  [^/v or PgUp/PgDn: Load Order]",
                m.version, m.author
            )
        } else {
            let max_desc = 28usize;
            let desc: String = if m.description.chars().count() > max_desc {
                let clipped: String = m.description.chars().take(max_desc - 3).collect();
                format!("{clipped}...")
            } else {
                m.description.clone()
            };
            format!("v{} by {} - {}  [^/v: Order]", m.version, m.author, desc)
        }
    }

    /// 1:1 10-row SH2/SH2:EE root `O P T I O N S` layout (`Item/otn_option.c`).
    fn root_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let g = &self.draft_config.game;
        let c = &self.draft_config.controls;
        let a = &self.draft_config.audio;
        match index {
            0 => Some(("Game Options", String::new())),
            1 => Some(("Advanced Options", String::new())),
            2 => Some(("Audio Options", String::new())),
            3 => Some(("Control Options", String::new())),
            4 => Some((
                "Control Type",
                if c.tank_controls {
                    "Rotational (3D)".into()
                } else {
                    "Directional (2D)".into()
                },
            )),
            5 => Some((
                "Vibration",
                if c.vibration_intensity >= 0.85 {
                    "Hard".into()
                } else if c.vibration_intensity >= 0.45 {
                    "Normal".into()
                } else if c.vibration_intensity > 0.05 {
                    "Soft".into()
                } else {
                    "Off".into()
                },
            )),
            6 => Some(("Language", g.language.label().to_string())),
            7 => Some((
                "Subtitles",
                if g.subtitles {
                    "On".into()
                } else {
                    "Off".into()
                },
            )),
            8 => Some((
                "Master Volume",
                format!("{}%", (a.master_volume * 100.0).round() as i32),
            )),
            9 => Some((
                "BGM Volume",
                format!("{}%", (a.bgm_volume * 100.0).round() as i32),
            )),
            10 => Some((
                "SFX Volume",
                format!("{}%", (a.sfx_volume * 100.0).round() as i32),
            )),
            _ => None,
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
            11 => Some((
                "Brightness Calibration",
                format!("Level {} (0..7)", self.draft_config.game.brightness_level),
            )),
            12 => Some((
                "ACES Tonemapping",
                if v.aces_tonemapping {
                    "ON (Filmic HDR)".into()
                } else {
                    "OFF (PS2 Linear)".into()
                },
            )),
            13 => Some((
                "Water Reflections",
                if v.enable_water_reflections {
                    "ON (Planar)".into()
                } else {
                    "OFF (PS2 1:1)".into()
                },
            )),
            14 => Some((
                "Oar Water Ripples",
                if v.enable_oar_ripples {
                    "ON (Native)".into()
                } else {
                    "OFF (PS2 1:1)".into()
                },
            )),
            15 => Some(("Enhanced Options", String::new())),
            16 => Some((
                "Title Screen Style",
                if v.enable_enhanced_title_screen {
                    "Enhanced Edition".into()
                } else {
                    "Original (SILENT HILL 2)".into()
                },
            )),
            17 => Some(("Mod Manager", String::new())),
            18 => Some((
                "Cutaway & Puzzle Quality",
                v.cutaway_quality.label().to_string(),
            )),
            19 => Some((
                "Cutaway Aspect Scaling",
                v.cutaway_aspect_mode.label().to_string(),
            )),
            20 => Some(("FMV Cutscene Source", v.fmv_source.label().to_string())),
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
            6 => Some(("3D Spatial Audio", a.spatial_audio_mode.label().to_string())),
            7 => Some((
                "Wall Occlusion",
                if a.enable_acoustic_occlusion {
                    "ON (.cld Raycast)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            8 => Some((
                "Mute in Background",
                if a.mute_in_background {
                    "YES".into()
                } else {
                    "NO".into()
                },
            )),
            9 => Some((
                "Restored Cut Audio",
                if a.enable_restored_cut_audio {
                    "ON (Restored)".into()
                } else {
                    "OFF (Vanilla)".into()
                },
            )),
            10 => Some((
                "Dynamic Room Acoustics",
                if a.enable_dynamic_room_acoustics {
                    "ON (.cam Reverb)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            11 if self.audio_pack_status.both_detected() => {
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
            5 => Some(("Camera Mode", c.default_camera_mode.label().to_string())),
            6 => Some(("R3 Camera Cycle", c.camera_cycle_preset.label().to_string())),
            7 => Some(("OTS Shoulder", c.ots_shoulder.label().to_string())),
            8 => Some(("Aiming Reticle", c.ots_reticle.label().to_string())),
            9 => Some((
                "First-Person FOV",
                format!("{} deg", c.first_person_fov_deg),
            )),
            10 => Some((
                "First-Person Sway",
                if c.first_person_head_bob {
                    "Subtle Sway".into()
                } else {
                    "Disabled".into()
                },
            )),
            11 => Some((
                "Controller Config (PS2)",
                format!("{} [Right/Enter]", c.ps2_bindings.preset.label()),
            )),
            12 => Some(("Combat Input Mode", c.combat_input_mode.label().to_string())),
            13 => Some((
                "R2 Heavy Threshold",
                format!("{:.0}%", c.r2_heavy_pressure_threshold * 100.0),
            )),
            14 => Some((
                "Analog Trigger Rowing",
                if c.enable_analog_trigger_rowing {
                    "ON (L2/R2)".into()
                } else {
                    "OFF (PS2 Sticks)".into()
                },
            )),
            15 => Some((
                "Widescreen Rail Clearance",
                c.widescreen_rail_clearance.label().to_string(),
            )),
            16 => Some((
                "Intra-Room Camera Cuts",
                c.intra_room_warp_policy.label().to_string(),
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
            6 => Some((
                "Delayed Fade-In (SH2:EE)",
                if g.enable_delayed_fade_in {
                    "ON (SH2:EE)".into()
                } else {
                    "OFF (PS2 1:1)".into()
                },
            )),
            7 => Some(("Test Rooms", "[Press Enter to Select]".into())),
            8 => Some((
                "Auto Load",
                if g.auto_load {
                    "ON".into()
                } else {
                    "OFF".into()
                },
            )),
            9 => Some(("Blood Color", g.blood_color.label().to_string())),
            10 => Some(("View Control", g.view_control.label().to_string())),
            11 => Some(("Retreat Turn", g.retreat_turn.label().to_string())),
            12 => Some(("Walk/Run Control", g.walk_run_control.label().to_string())),
            _ => None,
        }
    }

    fn enhanced_item_display(&self, index: usize) -> Option<(&'static str, String)> {
        let v = &self.draft_config.video;
        let a = &self.draft_config.audio;
        let c = &self.draft_config.controls;
        match index {
            0 => Some((
                "Version Preset",
                self.draft_config
                    .detect_enhanced_preset()
                    .label()
                    .to_string(),
            )),
            1 => Some(("Textures", v.texture_quality.label().to_string())),
            2 => Some(("Fog Engine", v.fog_quality.label().to_string())),
            3 => Some((
                "Volumetric Lighting",
                if v.volumetric_light {
                    "ON".into()
                } else {
                    "OFF".into()
                },
            )),
            4 => Some(("Shadow Mode", v.shadow_quality.label().to_string())),
            5 => Some(("Lighting Profile", v.lighting_profile.label().to_string())),
            6 => Some(("3D Spatial Audio", a.spatial_audio_mode.label().to_string())),
            7 => Some((
                "Wall Occlusion",
                if a.enable_acoustic_occlusion {
                    "ON (.cld Raycast)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            8 => Some((
                "Restored Cut Audio",
                if a.enable_restored_cut_audio {
                    "ON (Restored)".into()
                } else {
                    "OFF (Vanilla)".into()
                },
            )),
            9 => Some(("Camera Mode", c.default_camera_mode.label().to_string())),
            10 => Some((
                "Secondary Cloth Physics",
                if self.draft_config.game.enable_secondary_physics {
                    "ON (Native)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            11 => Some((
                "ACES Tonemapping",
                if v.aces_tonemapping {
                    "ON (Filmic HDR)".into()
                } else {
                    "OFF (PS2 Linear)".into()
                },
            )),
            12 => Some((
                "Dynamic Room Acoustics",
                if a.enable_dynamic_room_acoustics {
                    "ON (.cam Reverb)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            13 => Some((
                "Unlimited Save Slots",
                if self.draft_config.game.enable_unlimited_save_slots {
                    "ON (Native + Export)".into()
                } else {
                    "OFF (PS2 2x15)".into()
                },
            )),
            14 => Some((
                "Quicksave & Autosave",
                if self.draft_config.game.enable_quicksave_autosave {
                    "ON (Native)".into()
                } else {
                    "OFF (Manual Only)".into()
                },
            )),
            15 => Some((
                "Save Thumbnails",
                if self.draft_config.game.enable_save_thumbnails {
                    "ON (3D + Tracker)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            16 => Some((
                "Free 3D Item Orbit",
                if self.draft_config.game.enable_free_item_orbit {
                    "ON (Orbit + Zoom)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            17 => Some((
                "Cutscene Unequip",
                if self.draft_config.game.enable_cutscene_unequip {
                    "ON (Hide Weapon)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            18 => Some((
                "Foot IK (Stairs)",
                if self.draft_config.game.enable_foot_ik {
                    "ON (Native)".into()
                } else {
                    "OFF (Original PS2)".into()
                },
            )),
            19 => Some((
                "Persistent Blood Decals",
                if self.draft_config.game.enable_persistent_blood_decals {
                    "ON (Coagulate)".into()
                } else {
                    "OFF (PS2 Clear)".into()
                },
            )),
            20 if self.audio_pack_status.both_detected() => {
                Some(("Audio Pack", a.audio_pack_source.label().to_string()))
            }
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

        if let Some(sub_cue) = self.adjust_subscreen_horizontal(forward) {
            return sub_cue;
        }

        let special_cue = match self.active_tab {
            OptionsTab::Root => self.adjust_root_value(forward),
            OptionsTab::Video => self.adjust_video_value(forward),
            OptionsTab::Audio => self.adjust_audio_value(forward),
            OptionsTab::Controls => self.adjust_controls_value(forward),
            OptionsTab::Game => self.adjust_game_value(forward),
            OptionsTab::Enhanced => self.adjust_enhanced_value(forward),
            OptionsTab::Mods => {
                if let Some(m) = self.mods.get_mut(self.selected_index) {
                    m.enabled = !m.enabled;
                    self.sync_mods_to_draft_config();
                    Some(OptionsSoundCue::ValueChange)
                } else {
                    Some(OptionsSoundCue::None)
                }
            }
            OptionsTab::Debug => self.adjust_debug_value(forward),
        };

        if let Some(cue) = special_cue {
            return cue;
        }

        self.is_dirty = self.draft_config != self.original_config;
        OptionsSoundCue::ValueChange
    }

    fn adjust_root_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        match self.selected_index {
            0..=3 => return Some(OptionsSoundCue::None),
            4 => {
                let c = &mut self.draft_config.controls;
                c.tank_controls = !c.tank_controls;
            }
            5 => {
                let c = &mut self.draft_config.controls;
                const LEVELS: [f32; 4] = [0.0, 0.33, 0.66, 1.0];
                let cur_idx = if c.vibration_intensity >= 0.85 {
                    3
                } else if c.vibration_intensity >= 0.45 {
                    2
                } else if c.vibration_intensity > 0.05 {
                    1
                } else {
                    0
                };
                let next_idx = if forward {
                    (cur_idx + 1) % LEVELS.len()
                } else {
                    (cur_idx + LEVELS.len() - 1) % LEVELS.len()
                };
                c.vibration_intensity = LEVELS[next_idx];
            }
            6 => {
                let g = &mut self.draft_config.game;
                g.language = if forward {
                    g.language.next()
                } else {
                    g.language.prev()
                };
            }
            7 => {
                self.draft_config.game.subtitles = !self.draft_config.game.subtitles;
            }
            8 => {
                let cur = volume_gain_to_ps2_level(self.draft_config.audio.master_volume) as i32;
                let next = (cur + if forward { 1 } else { -1 })
                    .clamp(0, PS2_VOLUME_MAX_LEVEL as i32) as u8;
                self.draft_config.audio.master_volume = ps2_level_to_volume_gain(next);
            }
            9 => {
                let cur = volume_gain_to_ps2_level(self.draft_config.audio.bgm_volume) as i32;
                let next = (cur + if forward { 1 } else { -1 })
                    .clamp(0, PS2_VOLUME_MAX_LEVEL as i32) as u8;
                self.draft_config.audio.bgm_volume = ps2_level_to_volume_gain(next);
            }
            10 => {
                let cur = volume_gain_to_ps2_level(self.draft_config.audio.sfx_volume) as i32;
                let next = (cur + if forward { 1 } else { -1 })
                    .clamp(0, PS2_VOLUME_MAX_LEVEL as i32) as u8;
                self.draft_config.audio.sfx_volume = ps2_level_to_volume_gain(next);
            }
            _ => {}
        }
        None
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
            11 => {
                self.open_sub_screen(OptionsSubScreen::BrightnessCalibration);
                return Some(OptionsSoundCue::CursorMove);
            }
            12 => v.aces_tonemapping = !v.aces_tonemapping,
            13 => v.enable_water_reflections = !v.enable_water_reflections,
            14 => v.enable_oar_ripples = !v.enable_oar_ripples,
            16 => v.enable_enhanced_title_screen = !v.enable_enhanced_title_screen,
            18 => {
                v.cutaway_quality = if forward {
                    v.cutaway_quality.next()
                } else {
                    v.cutaway_quality.prev()
                };
            }
            19 => {
                v.cutaway_aspect_mode = v.cutaway_aspect_mode.toggle();
            }
            20 => {
                v.fmv_source = if forward {
                    v.fmv_source.next()
                } else {
                    v.fmv_source.prev()
                };
            }
            _ => return Some(OptionsSoundCue::None),
        }
        None
    }

    fn adjust_enhanced_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        match self.selected_index {
            0 => {
                let current = self.draft_config.detect_enhanced_preset();
                let next = if forward {
                    current.next_selectable()
                } else {
                    current.prev_selectable()
                };
                self.draft_config.apply_enhanced_preset(next);
            }
            1 => {
                let v = &mut self.draft_config.video;
                v.texture_quality = if forward {
                    v.texture_quality.next()
                } else {
                    v.texture_quality.prev()
                };
            }
            2 => {
                self.draft_config.video.fog_quality = self.draft_config.video.fog_quality.toggle();
            }
            3 => {
                self.draft_config.video.volumetric_light =
                    !self.draft_config.video.volumetric_light;
            }
            4 => {
                let v = &mut self.draft_config.video;
                v.shadow_quality = if forward {
                    v.shadow_quality.next()
                } else {
                    v.shadow_quality.prev()
                };
            }
            5 => {
                let v = &mut self.draft_config.video;
                v.lighting_profile = if forward {
                    v.lighting_profile.next()
                } else {
                    v.lighting_profile.prev()
                };
            }
            6 => {
                self.draft_config.audio.spatial_audio_mode =
                    self.draft_config.audio.spatial_audio_mode.toggle();
            }
            7 => {
                self.draft_config.audio.enable_acoustic_occlusion =
                    !self.draft_config.audio.enable_acoustic_occlusion;
            }
            8 => {
                self.draft_config.audio.enable_restored_cut_audio =
                    !self.draft_config.audio.enable_restored_cut_audio;
            }
            9 => {
                let c = &mut self.draft_config.controls;
                c.default_camera_mode = if forward {
                    c.default_camera_mode.next()
                } else {
                    c.default_camera_mode.prev()
                };
            }
            10 => {
                self.draft_config.game.enable_secondary_physics =
                    !self.draft_config.game.enable_secondary_physics;
            }
            11 => {
                self.draft_config.video.aces_tonemapping =
                    !self.draft_config.video.aces_tonemapping;
            }
            12 => {
                self.draft_config.audio.enable_dynamic_room_acoustics =
                    !self.draft_config.audio.enable_dynamic_room_acoustics;
            }
            13 => {
                self.draft_config.game.enable_unlimited_save_slots =
                    !self.draft_config.game.enable_unlimited_save_slots;
            }
            14 => {
                self.draft_config.game.enable_quicksave_autosave =
                    !self.draft_config.game.enable_quicksave_autosave;
            }
            15 => {
                self.draft_config.game.enable_save_thumbnails =
                    !self.draft_config.game.enable_save_thumbnails;
            }
            16 => {
                self.draft_config.game.enable_free_item_orbit =
                    !self.draft_config.game.enable_free_item_orbit;
            }
            17 => {
                self.draft_config.game.enable_cutscene_unequip =
                    !self.draft_config.game.enable_cutscene_unequip;
            }
            18 => {
                self.draft_config.game.enable_foot_ik = !self.draft_config.game.enable_foot_ik;
            }
            19 => {
                self.draft_config.game.enable_persistent_blood_decals =
                    !self.draft_config.game.enable_persistent_blood_decals;
            }
            20 if self.audio_pack_status.both_detected() => {
                let a = &mut self.draft_config.audio;
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

    fn adjust_audio_value(&mut self, forward: bool) -> Option<OptionsSoundCue> {
        let step_ps2_vol = |current: f32| -> f32 {
            let level = volume_gain_to_ps2_level(current) as i16;
            let next =
                (level + if forward { 1 } else { -1 }).clamp(0, PS2_VOLUME_MAX_LEVEL as i16) as u8;
            ps2_level_to_volume_gain(next)
        };
        let a = &mut self.draft_config.audio;
        match self.selected_index {
            0 => a.master_volume = step_ps2_vol(a.master_volume),
            1 => a.bgm_volume = step_ps2_vol(a.bgm_volume),
            2 => a.sfx_volume = step_ps2_vol(a.sfx_volume),
            3 => a.dialogue_volume = step_ps2_vol(a.dialogue_volume),
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
            6 => {
                a.spatial_audio_mode = if forward {
                    a.spatial_audio_mode.next()
                } else {
                    a.spatial_audio_mode.prev()
                };
            }
            7 => a.enable_acoustic_occlusion = !a.enable_acoustic_occlusion,
            8 => a.mute_in_background = !a.mute_in_background,
            9 => a.enable_restored_cut_audio = !a.enable_restored_cut_audio,
            10 => a.enable_dynamic_room_acoustics = !a.enable_dynamic_room_acoustics,
            11 if self.audio_pack_status.both_detected() => {
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
            5 => {
                c.default_camera_mode = if forward {
                    c.default_camera_mode.next()
                } else {
                    c.default_camera_mode.prev()
                };
            }
            6 => {
                c.camera_cycle_preset = if forward {
                    c.camera_cycle_preset.next()
                } else {
                    c.camera_cycle_preset.prev()
                };
            }
            7 => {
                c.ots_shoulder = if forward {
                    c.ots_shoulder.next()
                } else {
                    c.ots_shoulder.prev()
                };
            }
            8 => {
                c.ots_reticle = if forward {
                    c.ots_reticle.next()
                } else {
                    c.ots_reticle.prev()
                };
            }
            9 => {
                let step: i32 = if forward { 5 } else { -5 };
                c.first_person_fov_deg =
                    (c.first_person_fov_deg as i32 + step).clamp(55, 100) as u32;
            }
            10 => {
                c.first_person_head_bob = !c.first_person_head_bob;
            }
            11 => {
                self.open_sub_screen(OptionsSubScreen::ControllerConfig);
                return Some(OptionsSoundCue::CursorMove);
            }
            12 => {
                c.combat_input_mode = if forward {
                    c.combat_input_mode.next()
                } else {
                    c.combat_input_mode.prev()
                };
            }
            13 => {
                let step = if forward { 0.05 } else { -0.05 };
                c.r2_heavy_pressure_threshold =
                    (c.r2_heavy_pressure_threshold + step).clamp(0.25, 0.95);
            }
            14 => c.enable_analog_trigger_rowing = !c.enable_analog_trigger_rowing,
            15 => c.widescreen_rail_clearance = c.widescreen_rail_clearance.toggle(),
            16 => {
                c.intra_room_warp_policy = if forward {
                    c.intra_room_warp_policy.next()
                } else {
                    c.intra_room_warp_policy.prev()
                };
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
            6 => g.enable_delayed_fade_in = !g.enable_delayed_fade_in,
            7 => return Some(OptionsSoundCue::LaunchTestRooms),
            8 => g.auto_load = !g.auto_load,
            9 => {
                g.blood_color = if forward {
                    g.blood_color.next()
                } else {
                    g.blood_color.prev()
                };
            }
            10 => g.view_control = g.view_control.toggle(),
            11 => g.retreat_turn = g.retreat_turn.toggle(),
            12 => {
                g.walk_run_control = if forward {
                    g.walk_run_control.next()
                } else {
                    g.walk_run_control.prev()
                };
            }
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
