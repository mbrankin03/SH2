//! 8-Material Acoustic Surface Footstep & 3D SFX Attenuation Test Chamber (`SH2-61`).
//!
//! Provides an interactive 3D and 2D studio environment purpose-built for verifying `SH2-61`:
//! 1. **8 Walkable Collision Floor Material Pads** (`FloorTriangle::material` `0..=7`):
//!    - `0: Concrete / Stone`, `1: Wood / Planks`, `2: Metal Grating`, `3: Carpet / Rug`,
//!      `4: Water / Puddle`, `5: Grass / Forest Dirt`, `6: Gravel / Rubble`, `7: Blood / Organic`.
//!    - Walking or running across the pads with James or Maria (`[C]` to switch character) automatically
//!      samples `CollisionWorld::sample_floor_surface`, resolves `footstep_sddata_indices(is_maria, material, is_running)`,
//!      and enforces the `DoubleFootstepFix` (`MIN_FOOTSTEP_INTERVAL_SECS = 0.20`).
//! 2. **Interactive 3D Sound Emitter & Attenuation Runway (`SeCallPosDistanceF` `0x5155E0`)**:
//!    - Places a live 3D sound emitter beacon in the hall with wireframe inner ($d_{\min} = 1500.0$) and
//!      outer ($d_{\max} = 11500.0$) distance rings (`[H]` to trigger 3D spatialized SFX at emitter,
//!      `[G]` to move the emitter across calibrated distance markers, `[T]` to cycle emitter SFX cue).
//! 3. **Full 417-Entry `sddata.bin` & 32-Entry `voice.afs` Jukebox Browser**:
//!    - `[PageUp]` / `[PageDown]` to step through all `0..=416` `sddata.bin` entries and `[Enter]` to trigger.
//!    - `[J]` for Save Point chime (`10051` / `0x2743` in `voice.afs` entry `35` or `sh2e/sound/extra/save_sound.wav`).
//!    - `[L]` for Game Start transition (`15002` / `0x3A9A` in `voice.afs` entry `24` or `sh2e/sound/extra/g_start.wav`).
//!    - `[F]` for Flashlight On/Off toggle (`sh2e/sound/extra/flashlight_on.wav` / `flashlight_off.wav`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_app::synthetic_geometry::surface_sfx_test_pad_zones;
use sh2_audio::{
    compute_se_call_pos_distance_f, footstep_sddata_indices, play_menu_sound, SoundCueKind,
    SDDATA_ENTRY_COUNT, SFX_MAX_DISTANCE, SFX_MIN_DISTANCE,
};
use sh2_core::SurfaceMaterial;
use sh2_debug::DebugLineVertex;
use sh2_ui::menu::TitlePhase;

/// Curated 3D spatial SFX cues (`se_id`, label) from the 666-entry `0x8A1ED8` table.
pub(crate) const SPATIAL_SFX_PRESETS: &[(u32, &str)] = &[
    (11048, "11048 (0x2B28): Handgun Fire (sddata[274])"),
    (11051, "11051 (0x2B2B): Shotgun Blast (sddata[279])"),
    (11056, "11056 (0x2B30): Hunting Rifle Shot (sddata[26])"),
    (11032, "11032 (0x2B18): Wooden Plank Hit (sddata[346])"),
    (11040, "11040 (0x2B20): Steel Pipe Hit (sddata[348])"),
    (11005, "11005 (0x2AFD): James Wood Footstep (sddata[151])"),
    (5698, "5698 (0x1642): Chainsaw Loop (sddata[352], Looped)"),
    (11001, "11001 (0x2AF9): James Concrete Step (sddata[147])"),
];

/// Interactive runtime state for [`TestRoomKind::SurfaceSfxTestRoom`] (`SH2-61`).
#[derive(Debug, Clone)]
pub(crate) struct SurfaceSfxRoomState {
    /// World-space 3D position of the spatial SFX emitter beacon.
    pub emitter_pos: glam::Vec3,
    /// Index into [`SPATIAL_SFX_PRESETS`].
    pub spatial_preset_idx: usize,
    /// Selected `sddata.bin` entry index (`0..417`) for the raw sample jukebox browser.
    pub browse_sddata_idx: usize,
    /// Last `sddata.bin` index triggered by a footstep (`0..112`).
    pub last_footstep_sddata_idx: usize,
    /// Total footsteps triggered since entering the room (verifying `DoubleFootstepFix`).
    pub footstep_counter: u32,
    /// Last triggered SFX description for the HUD banner.
    pub last_triggered_desc: String,
    /// Whether to draw 3D wireframe pad boundaries and `SeCallPosDistanceF` attenuation rings.
    pub show_3d_guides: bool,
}

impl Default for SurfaceSfxRoomState {
    fn default() -> Self {
        Self {
            emitter_pos: glam::Vec3::new(0.0, 0.0, 4000.0),
            spatial_preset_idx: 0,
            browse_sddata_idx: 147, // James concrete footstep (`11001`)
            last_footstep_sddata_idx: 147,
            footstep_counter: 0,
            last_triggered_desc: "Walk across the 8 colored floor pads or press [1-8] to teleport"
                .to_string(),
            show_3d_guides: true,
        }
    }
}

impl SurfaceSfxRoomState {
    pub fn current_spatial_preset(&self) -> (u32, &'static str) {
        SPATIAL_SFX_PRESETS
            .get(self.spatial_preset_idx % SPATIAL_SFX_PRESETS.len())
            .copied()
            .unwrap_or(SPATIAL_SFX_PRESETS[0])
    }
}

/// Builds 3D wireframe outlines for the 8 material pads, the 3D sound emitter beacon,
/// and the `SeCallPosDistanceF` inner ($1500$) / outer ($11500$) distance rings.
pub(crate) fn build_surface_sfx_debug_lines(
    active_material: SurfaceMaterial,
    emitter_pos: glam::Vec3,
) -> Vec<DebugLineVertex> {
    let mut lines = Vec::with_capacity(512);

    // 1. Outline the 8 material pads; highlight the currently occupied pad in bright gold/green
    for pad in surface_sfx_test_pad_zones() {
        let is_active = pad.pad_index == (active_material as u8);
        let y = if is_active { -8.0 } else { -3.0 };
        let color = if is_active {
            [0.20, 1.0, 0.35, 1.0]
        } else {
            [
                pad.color_rgba[0] as f32 / 255.0,
                pad.color_rgba[1] as f32 / 255.0,
                pad.color_rgba[2] as f32 / 255.0,
                0.85,
            ]
        };

        let corners = [
            [pad.min_x, y, pad.min_z],
            [pad.max_x, y, pad.min_z],
            [pad.max_x, y, pad.max_z],
            [pad.min_x, y, pad.max_z],
        ];
        for i in 0..4 {
            let next = (i + 1) % 4;
            lines.push(DebugLineVertex::new(corners[i], color));
            lines.push(DebugLineVertex::new(corners[next], color));
        }
    }

    // 2. Draw 3D Sound Emitter Beacon (`emitter_pos`) + Inner (`1500u`) & Outer (`11500u`) `SeCallPosDistanceF` rings
    let beacon_lines = sh2_debug::build_chara_collision_debug_lines(
        [emitter_pos.x, emitter_pos.y, emitter_pos.z],
        180.0,
        1400.0,
        16,
        [1.0, 0.55, 0.15, 1.0], // Orange emitter beacon cylinder
    );
    lines.extend(beacon_lines);

    let segs = 36usize;
    for &(radius, col) in &[
        (SFX_MIN_DISTANCE, [0.2, 0.95, 0.4, 0.95]), // d_min = 1500 (full volume)
        (SFX_MAX_DISTANCE, [0.95, 0.25, 0.25, 0.85]), // d_max = 11500 (cutoff)
    ] {
        for i in 0..segs {
            let a0 = (i as f32) * std::f32::consts::TAU / (segs as f32);
            let a1 = ((i + 1) as f32) * std::f32::consts::TAU / (segs as f32);
            let p0 = [
                emitter_pos.x + a0.cos() * radius,
                -4.0,
                emitter_pos.z + a0.sin() * radius,
            ];
            let p1 = [
                emitter_pos.x + a1.cos() * radius,
                -4.0,
                emitter_pos.z + a1.sin() * radius,
            ];
            lines.push(DebugLineVertex::new(p0, col));
            lines.push(DebugLineVertex::new(p1, col));
        }
    }

    lines
}

pub(crate) struct SurfaceSfxRoomController;

impl RoomController for SurfaceSfxRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        if pressed && !repeat {
            match key_code {
                KeyCode::Escape | KeyCode::Backspace => {
                    state.menu_ctrl.in_game_active = false;
                    state.menu_ctrl.title_sm.phase = TitlePhase::TestRoomsSelect;
                    state.menu_ctrl.title_sm.fade_alpha = 1.0;
                    tracing::info!(
                        "Returned from Surface Footstep & 3D SFX Test Room to Test Rooms menu."
                    );
                    return;
                }
                // [Digit1]..=[Digit8]: Teleport directly onto Pad 0..=7 and trigger a footstep for that surface material
                KeyCode::Digit1
                | KeyCode::Digit2
                | KeyCode::Digit3
                | KeyCode::Digit4
                | KeyCode::Digit5
                | KeyCode::Digit6
                | KeyCode::Digit7
                | KeyCode::Digit8 => {
                    let pad_idx = match key_code {
                        KeyCode::Digit1 => 0,
                        KeyCode::Digit2 => 1,
                        KeyCode::Digit3 => 2,
                        KeyCode::Digit4 => 3,
                        KeyCode::Digit5 => 4,
                        KeyCode::Digit6 => 5,
                        KeyCode::Digit7 => 6,
                        _ => 7,
                    };
                    let pads = surface_sfx_test_pad_zones();
                    let pad = pads[pad_idx];
                    let cx = (pad.min_x + pad.max_x) * 0.5;
                    let cz = (pad.min_z + pad.max_z) * 0.5;
                    let target_pos = glam::Vec3::new(cx, 0.0, cz);
                    state
                        .sim_core
                        .snap_player_transform(target_pos, state.sim_core.scenario.player_yaw);
                    state
                        .sim_core
                        .camera
                        .reset_to_player(target_pos, state.sim_core.scenario.player_yaw);

                    let mat = SurfaceMaterial::from_cld_material(pad.material_id);
                    state.sim_core.active_floor_material = mat;

                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        if let Ok((sd_idx, _)) = bank.play_footstep(
                            eng,
                            state.active_maria,
                            mat,
                            state.sim_core.input.state.is_running,
                            0.95,
                        ) {
                            state.surface_sfx_room.last_footstep_sddata_idx = sd_idx;
                            state.surface_sfx_room.footstep_counter += 1;
                            state.surface_sfx_room.last_triggered_desc = format!(
                                "Teleported to Pad #{}: {} -> played sddata[{}]",
                                pad_idx + 1,
                                mat.label(),
                                sd_idx
                            );
                        }
                    }
                    return;
                }
                // [H]: Fire active 3D spatial SFX preset from `emitter_pos` using `SeCallPosDistanceF` (`0x5155E0`)
                KeyCode::KeyH => {
                    let (se_id, label) = state.surface_sfx_room.current_spatial_preset();
                    let p_pos = state.sim_core.scenario.player_position;
                    let p_yaw = state.sim_core.scenario.player_yaw;
                    let fwd = glam::Vec3::new(p_yaw.sin(), 0.0, p_yaw.cos());
                    let emit_pos = state.surface_sfx_room.emitter_pos;

                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        eng.set_listener_transform(p_pos, fwd);
                        if let Ok((eval, _)) =
                            bank.play_se_id_3d(eng, se_id, p_pos, p_yaw, emit_pos, 1.0)
                        {
                            state.surface_sfx_room.last_triggered_desc = format!(
                                "3D SFX {} @ dist={:.0}u -> v={:.2}, gain={:.0}%, att_byte={}, pan={:+.2}",
                                label,
                                eval.distance,
                                eval.proximity_v,
                                eval.effective_gain * 100.0,
                                eval.driver_attenuation_byte,
                                eval.stereo_pan
                            );
                        }
                    }
                    return;
                }
                // [G]: Move 3D emitter beacon across calibrated `SeCallPosDistanceF` distance steps (`1500`, `4000`, `6500`, `9000`, `11500`)
                KeyCode::KeyG => {
                    const Z_STEPS: &[f32] = &[1500.0, 4000.0, 6500.0, 9000.0, 11500.0];
                    let cur_z = state.surface_sfx_room.emitter_pos.z;
                    let next_z = Z_STEPS
                        .iter()
                        .copied()
                        .find(|&z| z > cur_z + 50.0)
                        .unwrap_or(Z_STEPS[0]);
                    state.surface_sfx_room.emitter_pos = glam::Vec3::new(0.0, 0.0, next_z);
                    state.surface_sfx_room.last_triggered_desc = format!(
                        "Moved 3D SFX Emitter Beacon to Z = {:.0} (Press [H] to fire 3D SFX)",
                        next_z
                    );
                    return;
                }
                // [T]: Cycle 3D spatial SFX preset (`Handgun`, `Shotgun`, `Rifle`, `Plank`, `Pipe`, `Great Knife`, `Chainsaw`)
                KeyCode::KeyT => {
                    state.surface_sfx_room.spatial_preset_idx =
                        (state.surface_sfx_room.spatial_preset_idx + 1) % SPATIAL_SFX_PRESETS.len();
                    let (_, label) = state.surface_sfx_room.current_spatial_preset();
                    state.surface_sfx_room.last_triggered_desc =
                        format!("Selected 3D Emitter Preset: {}", label);
                    return;
                }
                // [PageUp] / [PageDown] / [Enter]: Browse and play any of the 417 `sddata.bin` entries directly
                KeyCode::PageUp => {
                    state.surface_sfx_room.browse_sddata_idx =
                        (state.surface_sfx_room.browse_sddata_idx + 1) % SDDATA_ENTRY_COUNT;
                    let idx = state.surface_sfx_room.browse_sddata_idx;
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        let _ = bank.play_sddata_index(eng, idx, 0.95);
                    }
                    state.surface_sfx_room.last_triggered_desc =
                        format!("Played sddata.bin[{}] (417-entry bank browser)", idx);
                    return;
                }
                KeyCode::PageDown => {
                    state.surface_sfx_room.browse_sddata_idx =
                        if state.surface_sfx_room.browse_sddata_idx == 0 {
                            SDDATA_ENTRY_COUNT - 1
                        } else {
                            state.surface_sfx_room.browse_sddata_idx - 1
                        };
                    let idx = state.surface_sfx_room.browse_sddata_idx;
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        let _ = bank.play_sddata_index(eng, idx, 0.95);
                    }
                    state.surface_sfx_room.last_triggered_desc =
                        format!("Played sddata.bin[{}] (417-entry bank browser)", idx);
                    return;
                }
                KeyCode::Enter => {
                    let idx = state.surface_sfx_room.browse_sddata_idx;
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        let _ = bank.play_sddata_index(eng, idx, 0.95);
                    }
                    state.surface_sfx_room.last_triggered_desc =
                        format!("Replayed sddata.bin[{}]", idx);
                    return;
                }
                // [J]: Save Point Chime (`10051` / `0x2743` in `voice.afs` entry `35` or `sh2e/sound/extra/save_sound.wav`)
                KeyCode::KeyJ => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ = play_menu_sound(
                            eng,
                            state.audio_bank.as_ref(),
                            SoundCueKind::SavePoint,
                        );
                    }
                    state.surface_sfx_room.last_triggered_desc =
                        "Played 10051 (0x2743): Save Point Chime (voice.afs[35] / save_sound.wav)"
                            .to_string();
                    return;
                }
                // [L]: Game Start Transition (`15002` / `0x3A9A` in `voice.afs` entry `24` or `sh2e/sound/extra/g_start.wav`)
                KeyCode::KeyL => {
                    if let Some(ref mut eng) = state.audio_engine {
                        let _ = play_menu_sound(
                            eng,
                            state.audio_bank.as_ref(),
                            SoundCueKind::GameStartTransition,
                        );
                    }
                    state.surface_sfx_room.last_triggered_desc =
                        "Played 15002 (0x3A9A): Game Start Chime (voice.afs[24] / g_start.wav)"
                            .to_string();
                    return;
                }
                // [V]: Toggle 3D wireframe pad outlines & attenuation rings
                KeyCode::KeyV => {
                    state.surface_sfx_room.show_3d_guides = !state.surface_sfx_room.show_3d_guides;
                    return;
                }
                _ => {}
            }
        }

        // Delegate movement (`WASD`, `Shift`), character switch (`C`), flashlight (`F`), and pack swap (`B`)
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
                    let mat = state.sim_core.active_floor_material;
                    let is_running = state.sim_core.last_locomotion.state.is_running();
                    let vol = state.sim_core.last_locomotion.footstep_volume;
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_mut())
                    {
                        if let Ok((sd_idx, _)) =
                            bank.play_footstep(eng, state.active_maria, mat, is_running, vol)
                        {
                            state.surface_sfx_room.last_footstep_sddata_idx = sd_idx;
                            state.surface_sfx_room.footstep_counter += 1;
                            state.surface_sfx_room.last_triggered_desc = format!(
                                "Footstep #{}: {} ({}) -> sddata[{}] @ vol {:.0}%",
                                state.surface_sfx_room.footstep_counter,
                                mat.label(),
                                if is_running { "Run" } else { "Walk" },
                                sd_idx,
                                vol * 100.0
                            );
                        }
                    }
                }
            }
        }

        let alpha = state.sim_core.interpolation_alpha();
        let view = state.sim_core.camera.interpolated_view_matrix(alpha);
        let fov_y = state.sim_core.camera.interpolated_fov(alpha);
        let proj = glam::Mat4::perspective_rh(
            fov_y,
            aspect,
            super::DEFAULT_CAMERA_Z_NEAR,
            super::DEFAULT_CAMERA_Z_FAR,
        );
        let view_proj = proj * view;
        let inv_proj = proj.inverse();
        let eye_pos = state.sim_core.camera.curr_snapshot.eye;
        state.render_ctx.update_camera(view_proj, inv_proj, eye_pos);

        let (player_pos, player_yaw) = state.sim_core.interpolated_player_pose(alpha);

        // Evaluate active character (James or Maria) skeletal pose
        state.skin_matrices_scratch.clear();
        let active_char_mut = if state.active_maria {
            state.maria.as_mut().or(state.character.as_mut())
        } else {
            state.character.as_mut().or(state.maria.as_mut())
        };
        if let Some(c) = active_char_mut {
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
        let active_char_ref = if state.active_maria {
            state.maria.as_ref().or(state.character.as_ref())
        } else {
            state.character.as_ref().or(state.maria.as_ref())
        };
        if let Some(c) = active_char_ref {
            if !state.skin_matrices_scratch.is_empty() {
                skeletal_draw.push((
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(c.texture_bind_groups.as_slice()),
                ));
            }
        }

        if is_in_game {
            if state.surface_sfx_room.show_3d_guides {
                let mut debug_lines = build_surface_sfx_debug_lines(
                    state.sim_core.active_floor_material,
                    state.surface_sfx_room.emitter_pos,
                );
                // Draw yellow player cylinder
                let chara_lines = sh2_debug::build_chara_collision_debug_lines(
                    player_pos.into(),
                    state.sim_core.collision_config.radius,
                    state.sim_core.collision_config.height,
                    16,
                    [1.0, 0.9, 0.1, 1.0],
                );
                debug_lines.extend(chara_lines);

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

        let p_pos = state.sim_core.scenario.player_position;
        let p_yaw = state.sim_core.scenario.player_yaw;
        let active_mat = state.sim_core.active_floor_material;
        let is_running = state.sim_core.last_locomotion.state.is_running();

        let bank_count = state
            .audio_bank
            .as_ref()
            .map(|b| b.sddata_entry_count())
            .unwrap_or(0);
        let bank_rate = state
            .audio_bank
            .as_ref()
            .map(|b| b.primary_sample_rate())
            .unwrap_or(0);
        let pack_label = if bank_rate >= 48000 {
            "SH2:EE LOSSLESS 48 kHz (sh2e/sound/sddata.bin)"
        } else {
            "ORIGINAL PS2/PC 16-22.05 kHz (data/sound/sddata.bin)"
        };

        state.menu_rasterizer.draw_text(
            "SILENT HILL 2 - 8-MATERIAL SURFACE FOOTSTEP & 3D SFX ATTENUATION CHAMBER [SH2-61]",
            20,
            16,
            2,
            yellow,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "ACTIVE SFX BANK [B]: {} | Loaded WAVs: {} / 417 ({} Hz)",
                pack_label, bank_count, bank_rate
            ),
            20,
            44,
            1,
            green,
            true,
        );

        let (step_l, step_r) = footstep_sddata_indices(state.active_maria, active_mat, is_running);
        state.menu_rasterizer.draw_text(
            &format!(
                "CHARACTER [C]: {} | FLOOR SURFACE: {:?} ({}) | GAIT: {} | Footstep Pair: sddata[{}, {}] | Total Steps: {}",
                if state.active_maria { "Maria (Born From A Wish)" } else { "James Sunderland" },
                active_mat,
                active_mat.label(),
                if is_running { "RUN (Shift)" } else { "WALK" },
                step_l,
                step_r,
                state.surface_sfx_room.footstep_counter
            ),
            20,
            64,
            1,
            cyan,
            true,
        );

        // 8-Material Pad Table
        state.menu_rasterizer.draw_text(
            "--- 8 COLLISION FLOOR MATERIAL PADS (.cld FloorTriangle::material 0..=7 | Press [1-8] to Teleport) ---",
            20,
            90,
            1,
            yellow,
            true,
        );

        let all_mats = [
            SurfaceMaterial::Concrete,
            SurfaceMaterial::Wood,
            SurfaceMaterial::Metal,
            SurfaceMaterial::Carpet,
            SurfaceMaterial::Water,
            SurfaceMaterial::Grass,
            SurfaceMaterial::Gravel,
            SurfaceMaterial::Blood,
        ];
        for (i, &mat) in all_mats.iter().enumerate() {
            let y = 110 + i * 20;
            let (wl, wr) = footstep_sddata_indices(state.active_maria, mat, false);
            let (rl, rr) = footstep_sddata_indices(state.active_maria, mat, true);
            let is_cur = mat == active_mat;
            let row_col = if is_cur { green } else { white };
            let marker = if is_cur { ">> ACTIVE PAD <<" } else { "" };

            state.menu_rasterizer.draw_text(
                &format!(
                    "[Key {}] Mat #{}: {:<22} | Walk L/R: sddata[{:>3}, {:>3}] | Run L/R: sddata[{:>3}, {:>3}] {}",
                    i + 1,
                    i,
                    mat.label(),
                    wl,
                    wr,
                    rl,
                    rr,
                    marker
                ),
                20,
                y,
                1,
                row_col,
                true,
            );
        }

        // 3D Positional Attenuation Telemetry (`SeCallPosDistanceF` `0x5155E0`)
        let emit_pos = state.surface_sfx_room.emitter_pos;
        let eval = compute_se_call_pos_distance_f(p_pos, p_yaw, emit_pos, 1.0);
        let (_, preset_label) = state.surface_sfx_room.current_spatial_preset();

        state.menu_rasterizer.draw_text(
            "--- 3D POSITIONAL ATTENUATION & AZIMUTH PANNING (SeCallPosDistanceF 0x5155E0: d_min=1500, d_max=11500) ---",
            20,
            282,
            1,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "EMITTER BEACON [G]: (X:{:.0}, Z:{:.0}) | PLAYER: (X:{:.0}, Z:{:.0}) | 3D DISTANCE: {:.0}u | PRESET [T]: {}",
                emit_pos.x, emit_pos.z, p_pos.x, p_pos.z, eval.distance, preset_label
            ),
            20,
            302,
            1,
            cyan,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "PROXIMITY v: {:.3} | PS2 CURVE gain(v)=25.5/(255*(1-v)): {:.1}% | EFFECTIVE GAIN: {:.1}% | shSdCall BYTE: {} | STEREO PAN: {:+.2}",
                eval.proximity_v,
                eval.ps2_curve_gain * 100.0,
                eval.effective_gain * 100.0,
                eval.driver_attenuation_byte,
                eval.stereo_pan
            ),
            20,
            322,
            1,
            orange,
            true,
        );

        // Raw 417-entry `sddata.bin` Jukebox & Last Triggered Event
        let b_idx = state.surface_sfx_room.browse_sddata_idx;
        let wav_meta = state
            .audio_bank
            .as_ref()
            .and_then(|b| b.get_sddata_entry(b_idx))
            .map(|w| {
                format!(
                    "{} Hz, {} ch, {} bytes, loops={} (loop_start={})",
                    w.sample_rate,
                    w.channels,
                    w.data.len(),
                    w.loops,
                    w.loop_start
                )
            })
            .unwrap_or_else(|| "Unloaded".to_string());

        state.menu_rasterizer.draw_text(
            &format!(
                "SDDATA JUKEBOX [PgUp/PgDn/Enter]: Entry #{} / 416 ({}) | LAST EVENT: {}",
                b_idx, wav_meta, state.surface_sfx_room.last_triggered_desc
            ),
            20,
            348,
            1,
            green,
            true,
        );

        // Footer Controls
        state.menu_rasterizer.draw_text(
            "[WASD + Shift] Walk/Run Across 8 Material Pads  |  [1-8] Teleport to Material Pad  |  [C] Switch James / Maria",
            20,
            648,
            1,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[H] Fire 3D Emitter SFX  |  [G] Move Emitter Distance (1500..11500u)  |  [T] Cycle 3D SFX Preset  |  [B] Swap 48kHz / PS2 Pack",
            20,
            668,
            1,
            white,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[PgUp/PgDn/Enter] Browse All 417 sddata.bin WAVs  |  [J] Save Chime (10051)  |  [L] Start Chime (15002)  |  [F] Flashlight  |  [V] 3D Guides",
            20,
            688,
            1,
            muted,
            true,
        );

        state.finish_hud_overlay(encoder, surface_view);
    }
}
