//! Audio & Multi-Channel AIX Spatial Mixing Debug Room controller (`SH2-60`).
//!
//! Provides an interactive 3D and 2D studio environment for testing and inspecting:
//! - Dual-source audio pack streaming (`EnhancedLossless` 48 kHz `sh2e/sound/adx/` vs `OriginalPs2` `data/sound/adx/`).
//! - Real-time 7-layer `.aix` spatial volume mixing driven by stage `_SOUND_DATA` bounding boxes (`0x8BC8C0`).
//! - 3D wireframe visualization of all `_SOUND_DATA` bounding boxes for the active stage (highlighted in green when
//!   James is inside a box, cyan when inactive).
//! - Situation page overrides (`0: Calm`, `2: LowHP (<20%)`, `3: EnemyNear (<6000u)`, `4..=5: Event`, `6..=13: Hotel Music Box`, `14: Mute`).
//! - Stage preset cycling and direct BGM Page (`0..=47`) jukebox browsing across all 47 retail ADX/AIX streams.
//! - Menu / UI SFX cue testing (`CursorMove`, `Confirm`, `Cancel`, `GameStartTransition`) and Environmental Reverb presets.

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_audio::{
    bgm_page_entry, compute_se_call_pos_distance_f, find_bgm_file_by_load_id, play_menu_sound,
    CutAudioRestorer, EnvironmentalPreset, SoundCueKind, SoundDataRecord, BGM_MAX_TRACKS,
    BGM_PAGES, CUT_AND_RARE_AUDIO_CATALOG, E3_PROTOTYPE_SDB_CATALOG,
    E3_TGS_TRIAL_SDB_SAMPLE_RECORDS, FADE_TIME_SECS, LAURA_LETTER_VOICEOVER_SLICES,
    STAGE_SOUND_DATA,
};
use sh2_core::SurfaceMaterial;
use sh2_debug::DebugLineVertex;
use sh2_ui::menu::TitlePhase;

/// Stage presets curated for interactive multi-channel AIX and single-stream ADX testing.
pub(crate) const AUDIO_DEBUG_STAGE_PRESETS: &[(u32, &str)] = &[
    (
        3,
        "Stage 3: Observation Deck (bgm_114_b.aix, 6 layers, 5 boxes)",
    ),
    (2, "Stage 2: Opening Restroom (bgm_114_b.aix, 6 layers)"),
    (5, "Stage 5: Town East (bgm_115.aix, 5 layers, 32 boxes)"),
    (
        7,
        "Stage 7: Wood Side Apt 2F (bgm_118.aix, 5 layers, 49 boxes)",
    ),
    (
        10,
        "Stage 10: Blue Creek Apt 1F (bgm_102.aix, 5 layers, 29 boxes)",
    ),
    (12, "Stage 12: Apt Stairwell Boss (bgm_125.aix, 3 layers)"),
    (
        14,
        "Stage 14: Town West / Rosewater (bgm_117.aix, 4 layers)",
    ),
    (
        17,
        "Stage 17: Heaven's Night (bgm_004.adx, Promise Reprise)",
    ),
    (
        18,
        "Stage 18: Brookhaven 1F (bgm_119.aix, 5 layers, 46 boxes)",
    ),
    (
        22,
        "Stage 22: Brookhaven 1F Nightmare (bgm_105.aix, 5 layers)",
    ),
    (
        24,
        "Stage 24: Brookhaven 3F Nightmare (bgm_105.aix, 5 layers)",
    ),
    (
        32,
        "Stage 32: Toluca Prison North (bgm_110.aix, 6 layers, 55 boxes)",
    ),
    (
        35,
        "Stage 35: Labyrinth East (bgm_116.aix, 6 layers, 51 boxes)",
    ),
    (
        40,
        "Stage 40: Lakeview Hotel 1F (bgm_112.aix, 7-Layer Music Box)",
    ),
    (
        41,
        "Stage 41: Lakeview Hotel 2F (bgm_112.aix 7th Layer / bgm_121.aix)",
    ),
    (
        43,
        "Stage 43: Hotel Basement Boiler (bgm_108.aix, 5 layers)",
    ),
    (45, "Stage 45: Hotel 2F Otherworld (bgm_122.aix, 5 layers)"),
    (47, "Stage 47: Final Boss (bgm_113.aix, 4 layers)"),
    (48, "Stage 48: Leave Ending (end/bgm_002.adx, ADX stream)"),
    (
        55,
        "Stage 55: Born From A Wish Mansion (bgm_104.aix, 5 layers)",
    ),
];

/// Interactive state for the Audio & AIX Spatial Mixing Debug Room.
#[derive(Debug, Clone)]
pub(crate) struct AudioDebugState {
    /// Index into [`AUDIO_DEBUG_STAGE_PRESETS`].
    pub preset_index: usize,
    /// Optional explicit BGM page jukebox override (`0..=47`). `None` = driven automatically by `SeBgmChange`.
    pub manual_page_override: Option<u32>,
    /// Simulated situation mode (`0` = Auto/Calm, `1` = LowHP `<20%`, `2` = EnemyNear `<6000u`, `3` = Event `4`, `4` = MusicBox `9`, `5` = Mute `14`).
    pub situation_sim_mode: u8,
    /// Soloed AIX layer (`Some(0..=6)`), or `None` for normal `_SOUND_DATA` spatial mixing.
    pub solo_track: Option<usize>,
    /// Whether to draw 3D wireframe bounding boxes for the active stage's `_SOUND_DATA` areas.
    pub show_sound_boxes_3d: bool,
    /// Active environmental DSP reverb preset index (`0..=5`).
    pub reverb_preset_idx: usize,
    /// Last triggered SFX cue label for HUD display.
    pub last_sfx_triggered: &'static str,
    /// Active surface material override for testing 8-material footsteps (`SH2-61`).
    pub test_surface_material: SurfaceMaterial,
    /// Simulated 3D SFX emitter distance (`1500.0..=11500.0`) for testing `SeCallPosDistanceF` (`0x5155E0`).
    pub test_emitter_distance: f32,
    /// Selected index into [`CUT_AND_RARE_AUDIO_CATALOG`] (`SH2-62`).
    pub cut_catalog_index: usize,
    /// Selected slice index (`0..=10`) into [`LAURA_LETTER_VOICEOVER_SLICES`] (`voice.afs[133]` / `SCENE_45`) (`SH2-62`).
    pub laura_slice_index: usize,
    /// Selected index into [`E3_PROTOTYPE_SDB_CATALOG`] (`SH2-62`).
    pub proto_sdb_index: usize,
    /// Whether to overlay E3 2001 `tgs_trial.sdb` (`E3_TGS_TRIAL_SDB_SAMPLE_RECORDS`) 3D bounding boxes (`SH2-62`).
    pub show_proto_tgs_sdb_boxes: bool,
    /// Cutscene audio restorer state machine (`PlayUnusedAudio.cpp` & `ClosetCutsceneBonusAudio`) (`SH2-62`).
    pub cut_audio_restorer: CutAudioRestorer,
}

impl Default for AudioDebugState {
    fn default() -> Self {
        Self {
            preset_index: 0,
            manual_page_override: None,
            situation_sim_mode: 0,
            solo_track: None,
            show_sound_boxes_3d: true,
            reverb_preset_idx: 0,
            last_sfx_triggered: "None",
            test_surface_material: SurfaceMaterial::Concrete,
            test_emitter_distance: 3500.0,
            cut_catalog_index: 0,
            laura_slice_index: 0,
            proto_sdb_index: 0,
            show_proto_tgs_sdb_boxes: false,
            cut_audio_restorer: CutAudioRestorer::new(),
        }
    }
}

impl AudioDebugState {
    pub fn current_stage_preset(&self) -> (u32, &'static str) {
        AUDIO_DEBUG_STAGE_PRESETS
            .get(self.preset_index % AUDIO_DEBUG_STAGE_PRESETS.len())
            .copied()
            .unwrap_or((3, "Stage 3: Observation Deck"))
    }

    pub fn situation_mode_label(&self) -> &'static str {
        match self.situation_sim_mode {
            0 => "AUTO (0: Calm / Normal)",
            1 => "SIMULATE LOW HP (<20% -> Sit 2)",
            2 => "SIMULATE ENEMY NEAR (1500u -> Sit 3)",
            3 => "FORCE EVENT OVERRIDE (Sit 4)",
            4 => "HOTEL MUSIC BOX PRESET (Sit 9)",
            5 => "FORCE MUTE OVERRIDE (Sit 14)",
            _ => "AUTO",
        }
    }

    pub fn reverb_preset(&self) -> EnvironmentalPreset {
        match self.reverb_preset_idx % 5 {
            0 => EnvironmentalPreset::Off,
            1 => EnvironmentalPreset::FoggyStreets,
            2 => EnvironmentalPreset::InteriorRoom,
            3 => EnvironmentalPreset::Corridor,
            _ => EnvironmentalPreset::CavernousEcho,
        }
    }
}

/// Builds 3D wireframe bounding boxes (`DebugLineVertex` pairs) for all `_SOUND_DATA` records in `stage_id`.
pub(crate) fn build_sound_data_debug_lines(
    stage_id: u32,
    matched_indices: &[usize],
    player_y: f32,
) -> Vec<DebugLineVertex> {
    let records = STAGE_SOUND_DATA
        .get(stage_id as usize)
        .copied()
        .unwrap_or(&[]);
    build_sound_records_wireframe(
        records,
        matched_indices,
        player_y,
        [0.15, 1.0, 0.35, 1.0],
        [0.20, 0.70, 0.95, 0.85],
    )
}

/// Builds 3D wireframe bounding boxes (`DebugLineVertex` pairs) for an arbitrary slice of [`SoundDataRecord`]s
/// (used for both retail `STAGE_SOUND_DATA` and prototype `.sdb` overlays like E3 `tgs_trial.sdb`) (`SH2-62`).
pub(crate) fn build_sound_records_wireframe(
    records: &[SoundDataRecord],
    matched_indices: &[usize],
    player_y: f32,
    active_color: [f32; 4],
    inactive_color: [f32; 4],
) -> Vec<DebugLineVertex> {
    if records.len() <= 1 {
        return Vec::new();
    }

    let mut lines = Vec::with_capacity((records.len() - 1) * 24);
    for (idx, rec) in records.iter().enumerate().skip(1) {
        let is_matched = matched_indices.contains(&idx);
        let color = if is_matched {
            active_color
        } else {
            inactive_color
        };

        let min_x = rec.x0.min(rec.x2);
        let max_x = rec.x0.max(rec.x2);
        let min_z = rec.z2.min(rec.z0);
        let max_z = rec.z2.max(rec.z0);

        // Clamp extreme sentinel Y values (`-50000..50000`) around the player's floor height so boxes are easy to see in 3D
        let y_bot = rec.max_y.clamp(player_y - 2500.0, player_y + 400.0);
        let y_top = rec.min_y.clamp(player_y - 2800.0, y_bot - 250.0);

        let corners_bot = [
            [min_x, y_bot, min_z],
            [max_x, y_bot, min_z],
            [max_x, y_bot, max_z],
            [min_x, y_bot, max_z],
        ];
        let corners_top = [
            [min_x, y_top, min_z],
            [max_x, y_top, min_z],
            [max_x, y_top, max_z],
            [min_x, y_top, max_z],
        ];

        for i in 0..4 {
            let next = (i + 1) % 4;
            // Bottom rectangle
            lines.push(DebugLineVertex::new(corners_bot[i], color));
            lines.push(DebugLineVertex::new(corners_bot[next], color));
            // Top rectangle
            lines.push(DebugLineVertex::new(corners_top[i], color));
            lines.push(DebugLineVertex::new(corners_top[next], color));
            // Vertical pillars
            lines.push(DebugLineVertex::new(corners_bot[i], color));
            lines.push(DebugLineVertex::new(corners_top[i], color));
        }
    }

    lines
}

pub(crate) struct AudioDebugRoomController;

impl RoomController for AudioDebugRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        if pressed && !repeat {
            match key_code {
                KeyCode::Escape | KeyCode::Backspace => {
                    state.menu_ctrl.in_game_active = false;
                    state.menu_ctrl.title_sm.phase = TitlePhase::TestRoomsSelect;
                    state.menu_ctrl.title_sm.fade_alpha = 1.0;
                    tracing::info!("Returned from Audio Debug Room to Test Rooms menu.");
                    return;
                }
                // [B]: Toggle SH2:EE 48kHz Lossless (`sh2e/sound/adx/`) vs Original PS2 (`data/sound/adx/`)
                KeyCode::KeyB => {
                    let next_source = state
                        .bgm_manager
                        .audio_pack_source
                        .toggle_concrete(&state.menu_ctrl.audio_pack_status);
                    if let Err(e) = state
                        .bgm_manager
                        .set_audio_pack_source(next_source, state.audio_engine.as_mut())
                    {
                        tracing::warn!("[Audio Debug] Failed to switch audio pack source: {:?}", e);
                    } else {
                        let resolver = sh2_assets::AssetPathResolver::resolve();
                        state.audio_bank =
                            Some(sh2_audio::MenuAudioBank::load_from_dirs_for_source(
                                &resolver.data_dir,
                                resolver.sh2e_dir.as_deref(),
                                next_source,
                            ));
                        tracing::info!(
                            "[Audio Debug] Switched AudioPackSource -> {:?} ({})",
                            next_source,
                            state.bgm_manager.hud_summary()
                        );
                    }
                    return;
                }
                // [N] / [P]: Cycle curated Stage BGM + `_SOUND_DATA` presets
                KeyCode::KeyN => {
                    state.audio_debug.manual_page_override = None;
                    state.audio_debug.preset_index =
                        (state.audio_debug.preset_index + 1) % AUDIO_DEBUG_STAGE_PRESETS.len();
                    let (stg, desc) = state.audio_debug.current_stage_preset();
                    state.bgm_stage_override = Some(stg);
                    Self::teleport_to_stage_sound_box(state, stg);
                    tracing::info!("[Audio Debug] Selected {}", desc);
                    return;
                }
                KeyCode::KeyP => {
                    state.audio_debug.manual_page_override = None;
                    if state.audio_debug.preset_index == 0 {
                        state.audio_debug.preset_index = AUDIO_DEBUG_STAGE_PRESETS.len() - 1;
                    } else {
                        state.audio_debug.preset_index -= 1;
                    }
                    let (stg, desc) = state.audio_debug.current_stage_preset();
                    state.bgm_stage_override = Some(stg);
                    Self::teleport_to_stage_sound_box(state, stg);
                    tracing::info!("[Audio Debug] Selected {}", desc);
                    return;
                }
                // [PageUp] / [PageDown]: Directly step through all 48 retail BGM Pages (`0..=47`)
                KeyCode::PageUp => {
                    let cur = state
                        .audio_debug
                        .manual_page_override
                        .unwrap_or(state.bgm_manager.target_page);
                    let next = (cur + 1) % (BGM_PAGES.len() as u32);
                    state.audio_debug.manual_page_override = Some(next);
                    state.bgm_manager.request_page(next);
                    tracing::info!("[Audio Debug] Manual BGM Page override -> Page {}", next);
                    return;
                }
                KeyCode::PageDown => {
                    let cur = state
                        .audio_debug
                        .manual_page_override
                        .unwrap_or(state.bgm_manager.target_page);
                    let next = if cur == 0 {
                        (BGM_PAGES.len() - 1) as u32
                    } else {
                        cur - 1
                    };
                    state.audio_debug.manual_page_override = Some(next);
                    state.bgm_manager.request_page(next);
                    tracing::info!("[Audio Debug] Manual BGM Page override -> Page {}", next);
                    return;
                }
                // [T]: Cycle Situation Page simulation (`0: Calm`, `2: LowHP`, `3: EnemyNear`, `4: Event`, `9: MusicBox`, `14: Mute`)
                KeyCode::KeyT => {
                    state.audio_debug.situation_sim_mode =
                        (state.audio_debug.situation_sim_mode + 1) % 6;
                    tracing::info!(
                        "[Audio Debug] Situation simulation -> {}",
                        state.audio_debug.situation_mode_label()
                    );
                    return;
                }
                // [Digit1]..=[Digit7]: Toggle Solo on AIX Layer 0..=6 (`[Digit0]` = Clear Solo / Auto Mix)
                KeyCode::Digit1
                | KeyCode::Digit2
                | KeyCode::Digit3
                | KeyCode::Digit4
                | KeyCode::Digit5
                | KeyCode::Digit6
                | KeyCode::Digit7 => {
                    let idx = match key_code {
                        KeyCode::Digit1 => 0,
                        KeyCode::Digit2 => 1,
                        KeyCode::Digit3 => 2,
                        KeyCode::Digit4 => 3,
                        KeyCode::Digit5 => 4,
                        KeyCode::Digit6 => 5,
                        _ => 6,
                    };
                    if state.audio_debug.solo_track == Some(idx) {
                        state.audio_debug.solo_track = None;
                        tracing::info!(
                            "[Audio Debug] Cleared layer solo (restored spatial `_SOUND_DATA` mix)"
                        );
                    } else {
                        state.audio_debug.solo_track = Some(idx);
                        tracing::info!("[Audio Debug] Soloed AIX Layer T{}", idx);
                    }
                    return;
                }
                KeyCode::Digit0 => {
                    state.audio_debug.solo_track = None;
                    tracing::info!(
                        "[Audio Debug] Cleared layer solo (restored spatial `_SOUND_DATA` mix)"
                    );
                    return;
                }
                // [V]: Toggle 3D `_SOUND_DATA` bounding box wireframe visualization
                KeyCode::KeyV => {
                    state.audio_debug.show_sound_boxes_3d = !state.audio_debug.show_sound_boxes_3d;
                    tracing::info!(
                        "[Audio Debug] 3D `_SOUND_DATA` volume wireframes: {}",
                        state.audio_debug.show_sound_boxes_3d
                    );
                    return;
                }
                // [E]: Cycle Environmental DSP Reverb preset (`Off`, `FoggyStreets`, `InteriorRoom`, `Corridor`, `CavernousEcho`)
                KeyCode::KeyE => {
                    state.audio_debug.reverb_preset_idx =
                        (state.audio_debug.reverb_preset_idx + 1) % 5;
                    let preset = state.audio_debug.reverb_preset();
                    if let Some(ref mut eng) = state.audio_engine {
                        eng.set_environmental_preset(preset);
                    }
                    tracing::info!("[Audio Debug] Environmental Reverb Preset -> {:?}", preset);
                    return;
                }
                // [U] / [I] / [O] / [L]: Trigger authentic Menu & Game Start SFX cues (`sddata.bin`)
                KeyCode::KeyU => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ = play_menu_sound(
                            eng,
                            state.audio_bank.as_ref(),
                            SoundCueKind::CursorMove,
                        );
                    }
                    state.audio_debug.last_sfx_triggered = "10000: CursorMove (0x2710)";
                    return;
                }
                KeyCode::KeyI => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ =
                            play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
                    }
                    state.audio_debug.last_sfx_triggered = "10002: Confirm (0x2712)";
                    return;
                }
                KeyCode::KeyO => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ =
                            play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Cancel);
                    }
                    state.audio_debug.last_sfx_triggered = "10003: Cancel (0x2713)";
                    return;
                }
                KeyCode::KeyL => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ = play_menu_sound(
                            eng,
                            state.audio_bank.as_ref(),
                            SoundCueKind::GameStartTransition,
                        );
                    }
                    state.audio_debug.last_sfx_triggered = "15002: GameStartTransition (0x3A9A)";
                    return;
                }
                // [J]: Trigger Save Point Chime (`10051` / `0x2743` in `voice.afs` entry `35` or `sh2e/sound/extra/save_sound.wav`) (`SH2-61`)
                KeyCode::KeyJ => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ = play_menu_sound(
                            eng,
                            state.audio_bank.as_ref(),
                            SoundCueKind::SavePoint,
                        );
                    }
                    state.audio_debug.last_sfx_triggered = "10051: SavePoint Chime (0x2743)";
                    return;
                }
                // [M]: Cycle Surface Material (`Concrete -> Wood -> Metal -> Carpet -> Water -> Grass -> Gravel -> Blood`) & play footstep (`SH2-61`)
                KeyCode::KeyM => {
                    state.audio_debug.test_surface_material =
                        state.audio_debug.test_surface_material.next();
                    state.sim_core.active_floor_material = state.audio_debug.test_surface_material;
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        let _ = bank.play_footstep(
                            eng,
                            state.active_maria,
                            state.audio_debug.test_surface_material,
                            state.sim_core.input.state.is_running,
                            0.95,
                        );
                    }
                    state.audio_debug.last_sfx_triggered =
                        state.audio_debug.test_surface_material.label();
                    tracing::info!(
                        "[Audio Debug] Cycled footstep surface material -> {}",
                        state.audio_debug.test_surface_material.label()
                    );
                    return;
                }
                // [H]: Trigger 3D Spatialized Handgun Fire (`11048` / `0x2B28`) at `test_emitter_distance` (`SeCallPosDistanceF` `0x5155E0`) (`SH2-61`)
                KeyCode::KeyH => {
                    const DIST_STEPS: &[f32] = &[1500.0, 4000.0, 6500.0, 9000.0, 11500.0];
                    let next_dist = DIST_STEPS
                        .iter()
                        .copied()
                        .find(|&d| d > state.audio_debug.test_emitter_distance + 10.0)
                        .unwrap_or(DIST_STEPS[0]);
                    state.audio_debug.test_emitter_distance = next_dist;

                    let p_pos = state.sim_core.scenario.player_position;
                    let p_yaw = state.sim_core.scenario.player_yaw;
                    let fwd = glam::Vec3::new(p_yaw.sin(), 0.0, p_yaw.cos());
                    let right = glam::Vec3::new(p_yaw.cos(), 0.0, -p_yaw.sin());
                    let emitter_pos = p_pos + (fwd * 0.7 + right * 0.7).normalize() * next_dist;

                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        eng.set_listener_transform(p_pos, fwd);
                        let _ = bank.play_se_id_3d(eng, 11048, p_pos, p_yaw, emitter_pos, 1.0);
                    }
                    state.audio_debug.last_sfx_triggered = "11048: 3D Handgun Shot (0x2B28)";
                    return;
                }
                // [BracketLeft] / [BracketRight]: Browse Cut, Unreferenced & Rare Audio Catalog (`SH2-62`) and audition
                KeyCode::BracketRight => {
                    state.audio_debug.cut_catalog_index = (state.audio_debug.cut_catalog_index + 1)
                        % CUT_AND_RARE_AUDIO_CATALOG.len();
                    let entry = CUT_AND_RARE_AUDIO_CATALOG[state.audio_debug.cut_catalog_index];
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        let _ = bank.play_cut_catalog_entry(eng, &entry, 0.95);
                    }
                    state.audio_debug.last_sfx_triggered = entry.title;
                    tracing::info!(
                        "[Audio Debug SH2-62] Auditioning cut/rare cue [{}/{}]: {}",
                        state.audio_debug.cut_catalog_index + 1,
                        CUT_AND_RARE_AUDIO_CATALOG.len(),
                        entry.title
                    );
                    return;
                }
                KeyCode::BracketLeft => {
                    if state.audio_debug.cut_catalog_index == 0 {
                        state.audio_debug.cut_catalog_index = CUT_AND_RARE_AUDIO_CATALOG.len() - 1;
                    } else {
                        state.audio_debug.cut_catalog_index -= 1;
                    }
                    let entry = CUT_AND_RARE_AUDIO_CATALOG[state.audio_debug.cut_catalog_index];
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        let _ = bank.play_cut_catalog_entry(eng, &entry, 0.95);
                    }
                    state.audio_debug.last_sfx_triggered = entry.title;
                    tracing::info!(
                        "[Audio Debug SH2-62] Auditioning cut/rare cue [{}/{}]: {}",
                        state.audio_debug.cut_catalog_index + 1,
                        CUT_AND_RARE_AUDIO_CATALOG.len(),
                        entry.title
                    );
                    return;
                }
                // [Enter]: Replay currently selected Cut/Rare catalog entry (`SH2-62`)
                KeyCode::Enter => {
                    let entry = CUT_AND_RARE_AUDIO_CATALOG[state.audio_debug.cut_catalog_index];
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        let _ = bank.play_cut_catalog_entry(eng, &entry, 0.95);
                    }
                    state.audio_debug.last_sfx_triggered = entry.title;
                    return;
                }
                // [KeyK]: Step through and play Mary's 11 restored letter slices (`voice.afs[133]` / `SCENE_45`, `PlayUnusedAudio.cpp`) (`SH2-62`)
                KeyCode::KeyK => {
                    let slice_idx = state.audio_debug.laura_slice_index;
                    let slice = LAURA_LETTER_VOICEOVER_SLICES[slice_idx];
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        let _ = bank.play_laura_letter_slice(eng, slice_idx, 1.0);
                    }
                    state.audio_debug.last_sfx_triggered = slice.subtitle_excerpt;
                    tracing::info!(
                        "[Audio Debug SH2-62] Playing SCENE_45 Mary Letter Slice #{}/11 ({:.2}s..{:.2}s): \"{}\"",
                        slice_idx + 1,
                        slice.start_sec,
                        slice.end_sec,
                        slice.subtitle_excerpt
                    );
                    state.audio_debug.laura_slice_index =
                        (slice_idx + 1) % LAURA_LETTER_VOICEOVER_SLICES.len();
                    return;
                }
                // [KeyR]: Toggle `AudioOptions::enable_restored_cut_audio` (`SH2-62`)
                KeyCode::KeyR => {
                    let next_val = !state
                        .menu_ctrl
                        .options_menu
                        .original_config
                        .audio
                        .enable_restored_cut_audio;
                    state
                        .menu_ctrl
                        .options_menu
                        .original_config
                        .audio
                        .enable_restored_cut_audio = next_val;
                    state
                        .menu_ctrl
                        .options_menu
                        .draft_config
                        .audio
                        .enable_restored_cut_audio = next_val;
                    if !next_val {
                        if let Some(ref mut bank) = state.audio_bank {
                            bank.stop_active_cut_audio();
                        }
                    }
                    tracing::info!(
                        "[Audio Debug SH2-62] Restored Cut Audio toggle -> {}",
                        next_val
                    );
                    return;
                }
                // [KeyG]: Toggle E3 2001 `tgs_trial.sdb` 3D spatial bounding-box wireframe overlay & cycle E3 `.sdb` catalog (`SH2-62`)
                KeyCode::KeyG => {
                    if !state.audio_debug.show_proto_tgs_sdb_boxes {
                        state.audio_debug.show_proto_tgs_sdb_boxes = true;
                    } else {
                        state.audio_debug.proto_sdb_index = (state.audio_debug.proto_sdb_index + 1)
                            % E3_PROTOTYPE_SDB_CATALOG.len();
                        if state.audio_debug.proto_sdb_index == 0 {
                            state.audio_debug.show_proto_tgs_sdb_boxes = false;
                        }
                    }
                    let sdb_meta = E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index];
                    tracing::info!(
                        "[Audio Debug SH2-62] E3 Prototype .sdb overlay: {} ({} — {} records)",
                        state.audio_debug.show_proto_tgs_sdb_boxes,
                        sdb_meta.filename,
                        sdb_meta.record_count
                    );
                    return;
                }
                _ => {}
            }
        }

        // Delegate movement (`WASD`, `Shift`, camera/fog tweaks) to standard 3D room controls
        CombatArenaRoomController::handle_key_input(state, key_code, pressed, repeat);
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        _dt_sec: f32,
        aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        if !is_paused {
            let ticks = state.sim_core.update(dt);
            if ticks > 0 {
                let p_pos = state.sim_core.scenario.player_position;
                let p_yaw = state.sim_core.scenario.player_yaw;
                let fwd = glam::Vec3::new(p_yaw.sin(), 0.0, p_yaw.cos());
                if let Some(ref mut eng) = state.audio_engine {
                    eng.set_listener_transform(p_pos, fwd);
                }
                if state.sim_core.footstep_triggered_since_last_poll {
                    state.sim_core.footstep_triggered_since_last_poll = false;
                    let mat = state.audio_debug.test_surface_material;
                    state.sim_core.active_floor_material = mat;
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        let _ = bank.play_footstep(
                            eng,
                            state.active_maria,
                            mat,
                            state.sim_core.last_locomotion.state.is_running(),
                            state.sim_core.last_locomotion.footstep_volume,
                        );
                    }
                }
            }

            if is_in_game {
                let (stg, _) = state.audio_debug.current_stage_preset();
                state.bgm_stage_override = Some(stg);
                let (_, mut flags) = state.active_bgm_stage_and_flags();

                let (hp_ratio, min_enemy_dist_xz) = match state.audio_debug.situation_sim_mode {
                    1 => (0.10, None),        // LowHP (< 20%) -> Situation 2
                    2 => (1.0, Some(1500.0)), // EnemyNear (< 6000u) -> Situation 3
                    3 => {
                        flags.flag_61b |= 0x01;
                        flags.ev_bgm_override = Some(4);
                        (1.0, None)
                    }
                    4 => {
                        flags.ev_bgm_override = Some(9);
                        (1.0, None)
                    }
                    5 => {
                        flags.ev_bgm_override = Some(14);
                        (1.0, None)
                    }
                    _ => (1.0, None),
                };

                state.bgm_manager.event_flags = flags;
                let player_pos = state.sim_core.scenario.player_position;

                if let Some(manual_page) = state.audio_debug.manual_page_override {
                    state.bgm_manager.stage_id = stg;
                    state.bgm_manager.request_page(manual_page);
                } else {
                    state.bgm_manager.update_stage_and_position(stg, player_pos);
                }

                // Advance `SeBgmManager` (`0x516080`)
                if let Err(e) = state.bgm_manager.tick(
                    stg,
                    player_pos,
                    hp_ratio,
                    min_enemy_dist_xz,
                    dt.as_secs_f32(),
                    state.audio_engine.as_mut(),
                ) {
                    tracing::warn!("[Audio Debug] SeBgmManager tick warning: {:?}", e);
                }

                // Advance `CutAudioRestorer` (`PlayUnusedAudio.cpp` / `PlayWavSound.cpp`) (`SH2-62`)
                let restored_enabled = state
                    .menu_ctrl
                    .options_menu
                    .original_config
                    .audio
                    .enable_restored_cut_audio;
                let _ = state.audio_debug.cut_audio_restorer.tick(
                    restored_enabled,
                    0,
                    state.bgm_manager.room_id,
                    0.0,
                    3,
                    0,
                    dt.as_secs_f32(),
                );

                // Apply solo track override if active
                if let Some(solo_idx) = state.audio_debug.solo_track {
                    for i in 0..BGM_MAX_TRACKS {
                        let v = if i == solo_idx { 1.0 } else { 0.0 };
                        state.bgm_manager.tracks[i].volume = v;
                        if let Some(ref mut eng) = state.audio_engine {
                            let gain = v
                                * state.bgm_manager.file_master_volume
                                * state.bgm_manager.global_bgm_fade;
                            eng.set_bgm_layer_gain(i, gain, Duration::from_millis(15));
                        }
                    }
                }
            }
        }

        // Render 3D scene + `_SOUND_DATA` spatial bounding box wireframes
        let alpha = state.sim_core.interpolation_alpha();
        let view = state.sim_core.camera.interpolated_view_matrix(alpha);
        let fov_y = state.sim_core.camera.interpolated_fov(alpha);
        let proj = glam::Mat4::perspective_rh(fov_y, aspect, 10.0, 60000.0);
        let view_proj = proj * view;
        let inv_proj = proj.inverse();
        let eye_pos = state.sim_core.camera.curr_snapshot.eye;
        state.render_ctx.update_camera(view_proj, inv_proj, eye_pos);

        let (player_pos, player_yaw) = state.sim_core.interpolated_player_pose(alpha);

        // Update James skeletal pose
        state.skin_matrices_scratch.clear();
        if let Some(ref mut c) = state.character {
            let frame_count = c.anm.frames.len();
            if frame_count > 0 {
                let (_, cur_cursor, _) = state
                    .sim_core
                    .animation
                    .evaluate_blended_cursor_at_alpha(alpha);
                let f0 = (cur_cursor.0 as usize) % frame_count;
                let f1 = (cur_cursor.1 as usize) % frame_count;
                sh2_assets::formats::anm::interpolate_frames_into(
                    &c.anm.frames[f0],
                    &c.anm.frames[f1],
                    cur_cursor.2,
                    &mut c.frame_scratch_a,
                );
                sh2_assets::formats::anm::evaluate_pose_into(
                    &c.model.parents,
                    &c.bind_locals,
                    &c.frame_scratch_a,
                    &mut c.pose_scratch,
                );
                sh2_assets::formats::anm::compute_skin_matrices_with_inv_into(
                    &c.pose_scratch,
                    &c.model.inv_initial_matrices,
                    &mut c.skin_scratch,
                );
                let (s, c_yaw) = player_yaw.sin_cos();
                let world_mat = [
                    [c_yaw, 0.0, s, player_pos.x],
                    [0.0, 1.0, 0.0, player_pos.y],
                    [-s, 0.0, c_yaw, player_pos.z],
                    [0.0, 0.0, 0.0, 1.0],
                ];
                for m_skin in &c.skin_scratch {
                    let mut m = *m_skin;
                    let orig = m;
                    for r in 0..4 {
                        for col in 0..4 {
                            m[r][col] = world_mat[r][0] * orig[0][col]
                                + world_mat[r][1] * orig[1][col]
                                + world_mat[r][2] * orig[2][col]
                                + world_mat[r][3] * orig[3][col];
                        }
                    }
                    state.skin_matrices_scratch.push(m);
                }
            }
        }

        let mut skeletal_draw = Vec::new();
        if let Some(ref c) = state.character {
            if !state.skin_matrices_scratch.is_empty() {
                skeletal_draw.push((
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(c.texture_bind_groups.as_slice()),
                ));
            }
        }

        if is_in_game {
            if state.audio_debug.show_sound_boxes_3d || state.audio_debug.show_proto_tgs_sdb_boxes {
                let mut debug_lines = if state.audio_debug.show_sound_boxes_3d {
                    build_sound_data_debug_lines(
                        state.bgm_manager.stage_id,
                        &state.bgm_manager.last_area_eval.matched_record_indices,
                        player_pos.y,
                    )
                } else {
                    Vec::new()
                };

                // Overlay E3 2001 `tgs_trial.sdb` (`SLPM_123.45`) 3D spatial BGM bounding boxes in magenta/gold (`SH2-62`)
                if state.audio_debug.show_proto_tgs_sdb_boxes {
                    let proto_lines = build_sound_records_wireframe(
                        &E3_TGS_TRIAL_SDB_SAMPLE_RECORDS,
                        &[],
                        player_pos.y - 120.0,
                        [1.0, 0.75, 0.15, 1.0],
                        [0.95, 0.30, 0.85, 0.90],
                    );
                    debug_lines.extend(proto_lines);
                }

                // Also draw a yellow ring around James representing his `SOUND_AREA_HALF_WIDTH = 50.0` probe
                let probe_lines = sh2_debug::build_chara_collision_debug_lines(
                    player_pos.into(),
                    100.0,
                    1600.0,
                    16,
                    [1.0, 0.9, 0.1, 1.0],
                );
                debug_lines.extend(probe_lines);

                state.render_ctx.render_scene_with_debug_lines(
                    Some(&state.gpu_map),
                    Some(&state.map_texture_bind_groups),
                    &skeletal_draw,
                    &debug_lines,
                );
            } else {
                state.render_ctx.render_scene(
                    Some(&state.gpu_map),
                    Some(&state.map_texture_bind_groups),
                    &skeletal_draw,
                );
            }
        }
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        state.begin_hud_overlay();

        let yellow = [245, 210, 65, 255];
        let cyan = [105, 215, 235, 255];
        let green = [120, 230, 150, 255];
        let white = [235, 235, 235, 255];
        let orange = [255, 165, 75, 255];
        let muted = [165, 165, 175, 255];

        let (stg, preset_desc) = state.audio_debug.current_stage_preset();
        let p_pos = state.sim_core.scenario.player_position;
        let mgr = &state.bgm_manager;

        let pack_str = match mgr.audio_pack_source {
            sh2_core::AudioPackPreference::Auto => {
                "AUTO -> SH2:EE LOSSLESS 48 kHz (sh2e/sound/adx/)"
            }
            sh2_core::AudioPackPreference::Sh2eeLossless => {
                "SH2:EE LOSSLESS 48 kHz (sh2e/sound/adx/)"
            }
            sh2_core::AudioPackPreference::Ps2Original => {
                "ORIGINAL PS2 / PC 24-44.1 kHz (data/sound/adx/)"
            }
        };

        state.menu_rasterizer.draw_text(
            "SILENT HILL 2 - AUDIO & MULTI-CHANNEL AIX SPATIAL MIXING STUDIO [SH2-60]",
            20,
            16,
            2,
            yellow,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "SOURCE PACK [B] : {} | SAMPLE RATE: {} Hz",
                pack_str, mgr.loaded_sample_rate
            ),
            20,
            44,
            1,
            green,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!("STAGE PRESET [N/P]: {}", preset_desc),
            20,
            62,
            1,
            white,
            true,
        );

        let page_entry = bgm_page_entry(mgr.current_page);
        let file_info = find_bgm_file_by_load_id(page_entry.load_id)
            .map(|f| {
                format!(
                    "{} ({})",
                    f.path,
                    if f.is_adx {
                        "Single-Stream ADX"
                    } else {
                        "Multi-Layer AIX"
                    }
                )
            })
            .unwrap_or_else(|| "Silence (-1, -1)".to_string());

        state.menu_rasterizer.draw_text(
            &format!(
                "ACTIVE BGM PAGE [PgUp/PgDn]: Page {} (Target: {}) | load_id: {} | start_id: {} | Master Vol: {:.0}%",
                mgr.current_page,
                mgr.target_page,
                page_entry.load_id,
                page_entry.start_id,
                mgr.file_master_volume * 100.0
            ),
            20,
            80,
            1,
            cyan,
            true,
        );

        let disk_path_str = mgr
            .loaded_full_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "None".to_string());

        state.menu_rasterizer.draw_text(
            &format!("STREAM FILE : {} -> {}", file_info, disk_path_str),
            20,
            98,
            1,
            white,
            true,
        );

        let total_boxes = STAGE_SOUND_DATA
            .get(stg as usize)
            .map(|s| s.len().saturating_sub(1))
            .unwrap_or(0);
        let matched_str = if mgr.last_area_eval.used_default_record {
            "Fallback Record #0 (Outside Boxes)".to_string()
        } else {
            format!(
                "Matched Boxes: {:?}",
                mgr.last_area_eval.matched_record_indices
            )
        };

        state.menu_rasterizer.draw_text(
            &format!(
                "PLAYER POS (WASD): X: {:.0}  Y: {:.0}  Z: {:.0} | Room ID: 0x{:02X} | Stage Boxes: {} ({})",
                p_pos.x, p_pos.y, p_pos.z, mgr.room_id, total_boxes, matched_str
            ),
            20,
            116,
            1,
            green,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "SITUATION [T]: {} -> Resolved BgmPageSet: {} | Solo Layer [1-7, 0=Auto]: {}",
                state.audio_debug.situation_mode_label(),
                mgr.situation_page,
                state
                    .audio_debug
                    .solo_track
                    .map(|i| format!("SOLO T{}", i))
                    .unwrap_or_else(|| "OFF (Spatial _SOUND_DATA Mix)".to_string())
            ),
            20,
            134,
            1,
            orange,
            true,
        );

        // Draw 7-Track AIX / ADX Real-Time VU Meter Table
        state.menu_rasterizer.draw_text(
            "--- 7-CHANNEL AIX / ADX LAYER MIXER (SeBgmManager 0x516080 & sndGetSoundAryByCharaPos 0x5187E0) ---",
            20,
            162,
            1,
            yellow,
            true,
        );

        for i in 0..BGM_MAX_TRACKS {
            let y = 182 + i * 22;
            let trk = &mgr.tracks[i];
            let (cmd_in, cmd_out) = mgr.last_area_eval.track_fades[i];
            let is_present = i < mgr.loaded_layer_count;

            let bar_blocks = ((trk.volume.clamp(0.0, 1.0) * 20.0).round() as usize).min(20);
            let mut bar = String::with_capacity(22);
            bar.push('[');
            for b in 0..20 {
                if b < bar_blocks {
                    bar.push('#');
                } else {
                    bar.push('.');
                }
            }
            bar.push(']');

            let fade_in_sec = FADE_TIME_SECS
                .get(trk.fade_in as usize)
                .copied()
                .unwrap_or(0.0);
            let fade_out_sec = FADE_TIME_SECS
                .get(trk.fade_out as usize)
                .copied()
                .unwrap_or(0.0);
            let eff_pct = trk.volume * mgr.file_master_volume * 100.0;

            let row_color = if !is_present {
                muted
            } else if state.audio_debug.solo_track == Some(i) {
                yellow
            } else if trk.volume > 0.01 {
                green
            } else {
                cyan
            };

            let status_tag = if !is_present {
                "N/A (Not in container)".to_string()
            } else {
                format!(
                    "{:?} | AreaCmd:(in={}, out={}) | Ramp:(in={:.1}s, out={:.1}s)",
                    trk.status, cmd_in, cmd_out, fade_in_sec, fade_out_sec
                )
            };

            state.menu_rasterizer.draw_text(
                &format!(
                    "LAYER T{} [Key {}]: {} {:>5.1}% (Eff: {:>5.1}%) | {}",
                    i,
                    i + 1,
                    bar,
                    trk.volume * 100.0,
                    eff_pct,
                    status_tag
                ),
                20,
                y,
                1,
                row_color,
                true,
            );
        }

        // Environmental Reverb & SFX Test Panel
        let eng_status = if let Some(ref eng) = state.audio_engine {
            format!(
                "ACTIVE (Kira Playing Layers: {})",
                eng.active_bgm_layer_count()
            )
        } else {
            "OFFLINE (No Audio Output Device)".to_string()
        };

        state.menu_rasterizer.draw_text(
            &format!(
                "AUDIO ENGINE : {} | DSP REVERB [E]: {:?} | LAST SFX [U/I/O/L/J/H]: {}",
                eng_status,
                state.audio_debug.reverb_preset(),
                state.audio_debug.last_sfx_triggered
            ),
            20,
            352,
            1,
            cyan,
            true,
        );

        let bank_samples = state
            .audio_bank
            .as_ref()
            .map(|b| b.sddata_entry_count())
            .unwrap_or(0);
        let bank_rate = state
            .audio_bank
            .as_ref()
            .map(|b| b.primary_sample_rate())
            .unwrap_or(0);
        let spatial_eval = compute_se_call_pos_distance_f(
            p_pos,
            state.sim_core.scenario.player_yaw,
            p_pos
                + glam::Vec3::new(1.0, 0.0, 1.0).normalize()
                    * state.audio_debug.test_emitter_distance,
            1.0,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "SFX BANK (SH2-61): {} / 417 WAVs ({} Hz) | Footstep Surface [M]: {} | 3D Shot [H]: dist={:.0}u (v={:.2}, gain={:.0}%, pan={:+.2})",
                bank_samples,
                bank_rate,
                state.audio_debug.test_surface_material.label(),
                state.audio_debug.test_emitter_distance,
                spatial_eval.proximity_v,
                spatial_eval.effective_gain * 100.0,
                spatial_eval.stereo_pan
            ),
            20,
            372,
            1,
            green,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "3D SOUND AREA WIREFRAMES [V]: {} | E3 PROTOTYPE .SDB OVERLAY [G]: {} ({} - {} boxes)",
                if state.audio_debug.show_sound_boxes_3d { "ON" } else { "OFF" },
                if state.audio_debug.show_proto_tgs_sdb_boxes { "ON (Magenta)" } else { "OFF" },
                E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index].filename,
                E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index].record_count
            ),
            20,
            392,
            1,
            white,
            true,
        );

        // --- SH2-62: Cut / Beta Audio, Unreferenced sddata.bin & Restored Voiceover Panel ---
        let cut_entry = CUT_AND_RARE_AUDIO_CATALOG[state.audio_debug.cut_catalog_index];
        let laura_slice = LAURA_LETTER_VOICEOVER_SLICES[state.audio_debug.laura_slice_index];
        let proto_sdb = E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index];

        state.menu_rasterizer.draw_text(
            "--- CUT / UNUSED RETAIL AUDIO, RESTORED VOICEOVER & PS2 PROTOTYPE SOUNDBANKS [SH2-62] ---",
            20,
            418,
            1,
            yellow,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "RESTORED CUT AUDIO [R]: {} | CUT CATALOG [[ / ], Enter] ({}/{} - {}): {}",
                if state
                    .menu_ctrl
                    .options_menu
                    .original_config
                    .audio
                    .enable_restored_cut_audio
                {
                    "ENABLED"
                } else {
                    "DISABLED (Vanilla)"
                },
                state.audio_debug.cut_catalog_index + 1,
                CUT_AND_RARE_AUDIO_CATALOG.len(),
                cut_entry.category.label(),
                cut_entry.title
            ),
            20,
            438,
            1,
            orange,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "LAURA LETTER VOICEOVER [K] (voice.afs[133] / SCENE_45): Page {}/11 ({:.2}s..{:.2}s) \"{}\"",
                laura_slice.page_index + 1,
                laura_slice.start_sec,
                laura_slice.end_sec,
                laura_slice.subtitle_excerpt
            ),
            20,
            458,
            1,
            cyan,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "PS2 PROTOTYPE .SDB [G]: {} ({} B, {} recs) - {} | July 13 SOUND.DAT: 79 VAG banks (1,422 samples) + 33 .sfc",
                proto_sdb.filename,
                proto_sdb.byte_length,
                proto_sdb.record_count,
                proto_sdb.notes
            ),
            20,
            478,
            1,
            green,
            true,
        );

        // Controls cheat-sheet footer
        state.menu_rasterizer.draw_text(
            "[B] Swap SH2:EE 48kHz / Original PS2  |  [N/P] Prev/Next Stage Preset  |  [PgUp/PgDn] Browse All 48 BGM Pages",
            20,
            648,
            1,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[[ / ] + Enter] Audition Cut/Rare SFX (sddata360..364 / 40000+)  |  [K] Play Mary Letter Slice  |  [R] Restored Toggle  |  [G] E3 .sdb",
            20,
            668,
            1,
            white,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[WASD] Walk  |  [M] Footstep Surface  |  [T] Situation  |  [1-7, 0] Solo Layer  |  [U/I/O/L/J/H] SFX & 3D Shot  |  [E] Reverb  |  [V] Boxes",
            20,
            688,
            1,
            muted,
            true,
        );

        state.finish_hud_overlay(encoder, surface_view);
    }
}

impl AudioDebugRoomController {
    /// Snaps James into the first spatial `_SOUND_DATA` bounding box of `stage_id` (if any)
    /// so switching stages immediately places him inside an active multi-layer mixing zone.
    pub(crate) fn teleport_to_stage_sound_box(state: &mut AppState, stage_id: u32) {
        let records = STAGE_SOUND_DATA
            .get(stage_id as usize)
            .copied()
            .unwrap_or(&[]);
        let target_pos = if stage_id == 3 {
            glam::Vec3::new(0.0, -100.0, -5000.0)
        } else if let Some(rec) = records.get(1) {
            let cx = (rec.x0 + rec.x2) * 0.5;
            let cz = (rec.z0 + rec.z2) * 0.5;
            glam::Vec3::new(cx, 0.0, cz)
        } else {
            glam::Vec3::new(0.0, 0.0, 0.0)
        };
        state.sim_core.snap_player_transform(target_pos, 0.0);
        state.sim_core.camera.reset_to_player(target_pos, 0.0);
    }
}
