//! Grapple Animation & Elevation Inspection Room controller (`SH2-57`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::formats::anm::{
    compute_skin_matrices_with_inv_into, evaluate_pose_into, interpolate_frames_into,
};
use sh2_debug::{build_skeleton_debug_lines, JamesAnmSource, GRAPPLE_CATALOG};
use sh2_ui::menu::TitlePhase;

pub(crate) struct GrappleRoomController;

impl RoomController for GrappleRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, _repeat: bool) {
        match key_code {
            KeyCode::Escape | KeyCode::Backspace if pressed => {
                state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                tracing::info!("Returned from Grapple Animation Room to Test Rooms menu.");
            }
            KeyCode::Tab | KeyCode::KeyN if pressed => {
                state.grapple_viewer.next_entry();
                tracing::info!(
                    "[Grapple Viewer] Switched to encounter: {}",
                    state.grapple_viewer.current_entry().name
                );
            }
            KeyCode::KeyP if pressed => {
                state.grapple_viewer.prev_entry();
                tracing::info!(
                    "[Grapple Viewer] Switched to encounter: {}",
                    state.grapple_viewer.current_entry().name
                );
            }
            KeyCode::Digit1 if pressed => {
                state.grapple_viewer.cycle_enemy_clip();
                tracing::info!(
                    "[Grapple Viewer] Creature clip: {}",
                    state.grapple_viewer.active_enemy_clip_name()
                );
            }
            KeyCode::Digit2 if pressed => {
                if state.sim_core.input.state.is_running {
                    state.grapple_viewer.prev_james_clip();
                } else {
                    state.grapple_viewer.next_james_clip();
                }
                tracing::info!(
                    "[Grapple Viewer] James clip: {}",
                    state.grapple_viewer.active_james_clip_name()
                );
            }
            KeyCode::Digit4 if pressed => {
                state.grapple_viewer.prev_james_clip();
                tracing::info!(
                    "[Grapple Viewer] James clip: {}",
                    state.grapple_viewer.active_james_clip_name()
                );
            }
            KeyCode::Digit3 if pressed => {
                let forward = !state.sim_core.input.state.is_running;
                state.grapple_viewer.cycle_anm_source(forward);
                tracing::info!(
                    "[Grapple Viewer] James ANM source: {:?}",
                    state.grapple_viewer.active_james_anm_source()
                );
            }
            KeyCode::KeyM if pressed => {
                state.grapple_viewer.toggle_scrub_mode();
                tracing::info!(
                    "[Grapple Viewer] Free timeline scrub mode: {}",
                    state.grapple_viewer.is_free_scrubbing
                );
            }
            KeyCode::PageDown if pressed => {
                if state.grapple_viewer.is_free_scrubbing {
                    state.grapple_viewer.scrub_shift(15);
                    tracing::info!(
                        "[Grapple Viewer] Scrub window: {}..={}",
                        state.grapple_viewer.scrub_start_frame,
                        state.grapple_viewer.scrub_start_frame
                            + state.grapple_viewer.scrub_window_len.saturating_sub(1)
                    );
                } else {
                    state.grapple_viewer.next_entry();
                    tracing::info!(
                        "[Grapple Viewer] Switched to encounter: {}",
                        state.grapple_viewer.current_entry().name
                    );
                }
            }
            KeyCode::PageUp if pressed => {
                if state.grapple_viewer.is_free_scrubbing {
                    state.grapple_viewer.scrub_shift(-15);
                    tracing::info!(
                        "[Grapple Viewer] Scrub window: {}..={}",
                        state.grapple_viewer.scrub_start_frame,
                        state.grapple_viewer.scrub_start_frame
                            + state.grapple_viewer.scrub_window_len.saturating_sub(1)
                    );
                } else {
                    state.grapple_viewer.prev_entry();
                    tracing::info!(
                        "[Grapple Viewer] Switched to encounter: {}",
                        state.grapple_viewer.current_entry().name
                    );
                }
            }
            KeyCode::Home if pressed => {
                state.grapple_viewer.scrub_jump_start();
            }
            KeyCode::End if pressed => {
                let max_f = state
                    .grapple_viewer
                    .active_james_anm_source()
                    .total_frames();
                state.grapple_viewer.scrub_jump_end(max_f);
            }
            KeyCode::Comma if pressed => {
                state.grapple_viewer.scrub_adjust_window(-5);
                tracing::info!(
                    "[Grapple Viewer] Scrub window len: {}",
                    state.grapple_viewer.scrub_window_len
                );
            }
            KeyCode::Period if pressed => {
                state.grapple_viewer.scrub_adjust_window(5);
                tracing::info!(
                    "[Grapple Viewer] Scrub window len: {}",
                    state.grapple_viewer.scrub_window_len
                );
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                state.sim_core.input.state.is_running = pressed;
            }
            KeyCode::Space if pressed => {
                state.grapple_viewer.toggle_pause();
                tracing::info!(
                    "[Grapple Viewer] Playback paused: {}",
                    state.grapple_viewer.is_paused
                );
            }
            KeyCode::BracketLeft if pressed => {
                state.grapple_viewer.step_frame(false);
            }
            KeyCode::BracketRight if pressed => {
                state.grapple_viewer.step_frame(true);
            }
            KeyCode::Minus | KeyCode::NumpadSubtract if pressed => {
                state.grapple_viewer.adjust_speed(false);
                tracing::info!(
                    "[Grapple Viewer] Playback speed: {:.2}x",
                    state.grapple_viewer.playback_speed
                );
            }
            KeyCode::Equal | KeyCode::NumpadAdd if pressed => {
                state.grapple_viewer.adjust_speed(true);
                tracing::info!(
                    "[Grapple Viewer] Playback speed: {:.2}x",
                    state.grapple_viewer.playback_speed
                );
            }
            KeyCode::KeyI | KeyCode::ArrowUp if pressed => {
                // In PS2 world space, UP is -Y
                state.grapple_viewer.adjust_james_elevation(-25.0);
                tracing::info!(
                    "[Grapple Viewer] James Y elevation: {:.1} mm",
                    state.grapple_viewer.james_y_offset
                );
            }
            KeyCode::KeyK | KeyCode::ArrowDown if pressed => {
                state.grapple_viewer.adjust_james_elevation(25.0);
                tracing::info!(
                    "[Grapple Viewer] James Y elevation: {:.1} mm",
                    state.grapple_viewer.james_y_offset
                );
            }
            KeyCode::KeyJ | KeyCode::ArrowLeft if pressed => {
                state.grapple_viewer.adjust_relative_distance(-25.0);
                tracing::info!(
                    "[Grapple Viewer] Creature Z distance: {:.1} mm",
                    state.grapple_viewer.relative_z_offset
                );
            }
            KeyCode::KeyL | KeyCode::ArrowRight if pressed => {
                state.grapple_viewer.adjust_relative_distance(25.0);
                tracing::info!(
                    "[Grapple Viewer] Creature Z distance: {:.1} mm",
                    state.grapple_viewer.relative_z_offset
                );
            }
            KeyCode::KeyR if pressed => {
                state.grapple_viewer.reset_for_current_entry();
                tracing::info!("[Grapple Viewer] Reset framing & offsets to default.");
            }
            KeyCode::KeyB if pressed => {
                state.grapple_viewer.skeleton_overlay = !state.grapple_viewer.skeleton_overlay;
            }
            KeyCode::KeyW => {
                state.key_forward = pressed;
            }
            KeyCode::KeyS => {
                state.key_backward = pressed;
            }
            KeyCode::KeyA => {
                state.key_left = pressed;
            }
            KeyCode::KeyD => {
                state.key_right = pressed;
            }
            KeyCode::KeyQ => {
                state.key_zoom_in = pressed;
            }
            KeyCode::KeyE => {
                state.key_zoom_out = pressed;
            }
            KeyCode::KeyZ => {
                state.key_pan_up = pressed;
            }
            KeyCode::KeyX => {
                state.key_pan_down = pressed;
            }
            _ => {}
        }
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        _dt: Duration,
        dt_sec: f32,
        aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        _is_in_game: bool,
    ) {
        let mut orbit_yaw_delta = 0.0f32;
        let mut orbit_pitch_delta = 0.0f32;
        let mut zoom_delta = 0.0f32;
        let mut pan_up_delta = 0.0f32;

        if !is_paused {
            if state.key_left {
                orbit_yaw_delta -= 1.8 * dt_sec;
            }
            if state.key_right {
                orbit_yaw_delta += 1.8 * dt_sec;
            }
            if state.key_forward {
                orbit_pitch_delta += 1.2 * dt_sec;
            }
            if state.key_backward {
                orbit_pitch_delta -= 1.2 * dt_sec;
            }
            if state.key_zoom_in {
                zoom_delta -= 1400.0 * dt_sec;
            }
            if state.key_zoom_out {
                zoom_delta += 1400.0 * dt_sec;
            }
            if state.key_pan_up {
                pan_up_delta += 800.0 * dt_sec;
            }
            if state.key_pan_down {
                pan_up_delta -= 800.0 * dt_sec;
            }

            state
                .grapple_viewer
                .camera
                .orbit(orbit_yaw_delta, orbit_pitch_delta);
            state.grapple_viewer.camera.zoom(zoom_delta);
            state.grapple_viewer.camera.pan(0.0, pan_up_delta);
            state.grapple_viewer.tick(dt_sec);
        }

        let view = state.grapple_viewer.camera.view_matrix();
        let proj = glam::Mat4::perspective_rh(
            state.grapple_viewer.camera.fov_y,
            aspect,
            super::DEFAULT_CAMERA_Z_NEAR,
            super::DEFAULT_CAMERA_Z_FAR,
        );
        let view_proj = proj * view;
        let inv_proj = proj.inverse();
        let eye_pos = state.grapple_viewer.camera.eye_position();

        // Neutral clean 3D inspection shading mode (3.0 = Full textured shading)
        state
            .render_ctx
            .update_camera_with_mode(view_proj, inv_proj, eye_pos, 3.0);

        // Stage spotlight configuration pointing at grapple focal center
        let target = state.grapple_viewer.camera.target;
        let to_target = (target - eye_pos).normalize_or_zero();
        let spot_uniforms = sh2_renderer::light::SpotlightUniforms {
            position: [eye_pos.x, eye_pos.y - 300.0, eye_pos.z, 1.0],
            direction_inner: [to_target.x, to_target.y, to_target.z, 0.70],
            color_outer: [1.3, 1.28, 1.22, 0.35],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
        };
        state.render_ctx.update_spotlight(&spot_uniforms);

        state.skin_matrices_scratch.clear();
        let mut debug_lines = Vec::new();
        let mut skeletal_draw = Vec::new();
        let entry = state.grapple_viewer.current_entry();

        // 1. Evaluate and render James Sunderland (zero per-frame heap allocation via c.skin_scratch)
        if let Some(ref mut c) = state.character {
            let active_src = state.grapple_viewer.active_james_anm_source();
            let active_james_anm = match active_src {
                JamesAnmSource::Basic => &c.anm,
                src => state.james_anm_cache.get(&src).unwrap_or(&c.anm),
            };

            let frame_count = active_james_anm.frames.len();
            if frame_count > 0 {
                let (f0, f1, alpha) = state.grapple_viewer.sample_james_cursor(frame_count);
                interpolate_frames_into(
                    &active_james_anm.frames[f0],
                    &active_james_anm.frames[f1],
                    alpha,
                    &mut c.frame_scratch_a,
                );
                evaluate_pose_into(
                    &c.model.parents,
                    &c.bind_locals,
                    &c.frame_scratch_a,
                    &mut c.pose_scratch,
                );
                compute_skin_matrices_with_inv_into(
                    &c.pose_scratch,
                    &c.model.inv_initial_matrices,
                    &mut c.skin_scratch,
                );

                let (s, c_yaw) = entry.james_yaw.sin_cos();
                let world_mat = [
                    [c_yaw, 0.0, s, 0.0],
                    [0.0, 1.0, 0.0, state.grapple_viewer.james_y_offset],
                    [-s, 0.0, c_yaw, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                for m_skin in &mut c.skin_scratch {
                    let orig = *m_skin;
                    for r in 0..4 {
                        for col in 0..4 {
                            m_skin[r][col] = world_mat[r][0] * orig[0][col]
                                + world_mat[r][1] * orig[1][col]
                                + world_mat[r][2] * orig[2][col]
                                + world_mat[r][3] * orig[3][col];
                        }
                    }
                }

                if state.grapple_viewer.skeleton_overlay {
                    debug_lines.extend(build_skeleton_debug_lines(
                        &c.model.parents,
                        &c.pose_scratch,
                        &world_mat,
                        35.0,
                    ));
                }
            }
        }

        // 2. Evaluate and render Creature (zero per-frame heap allocation via ea.skin_scratch)
        let skeleton_overlay = state.grapple_viewer.skeleton_overlay;
        let enemy_y_offset = state.grapple_viewer.enemy_y_offset;
        let relative_z_offset = state.grapple_viewer.relative_z_offset;
        let enemy_frame_sample = state
            .enemy_asset(entry.creature_id)
            .map(|ea| ea.anm.frames.len())
            .filter(|&fc| fc > 0)
            .map(|fc| state.grapple_viewer.sample_enemy_cursor(fc));

        if let (Some((f0, f1, alpha)), Some(ea)) =
            (enemy_frame_sample, state.enemy_asset_mut(entry.creature_id))
        {
            interpolate_frames_into(
                &ea.anm.frames[f0],
                &ea.anm.frames[f1],
                alpha,
                &mut ea.frame_scratch_a,
            );
            evaluate_pose_into(
                &ea.model.parents,
                &ea.bind_locals,
                &ea.frame_scratch_a,
                &mut ea.pose_scratch,
            );
            compute_skin_matrices_with_inv_into(
                &ea.pose_scratch,
                &ea.model.inv_initial_matrices,
                &mut ea.skin_scratch,
            );

            let (s, c_yaw) = entry.enemy_yaw.sin_cos();
            let ex = entry.default_anchor_offset.x;
            let ey = enemy_y_offset;
            let ez = entry.default_anchor_offset.z + relative_z_offset;
            let world_mat = [
                [c_yaw, 0.0, s, ex],
                [0.0, 1.0, 0.0, ey],
                [-s, 0.0, c_yaw, ez],
                [0.0, 0.0, 0.0, 1.0],
            ];

            for m_skin in &mut ea.skin_scratch {
                let orig = *m_skin;
                for r in 0..4 {
                    for col in 0..4 {
                        m_skin[r][col] = world_mat[r][0] * orig[0][col]
                            + world_mat[r][1] * orig[1][col]
                            + world_mat[r][2] * orig[2][col]
                            + world_mat[r][3] * orig[3][col];
                    }
                }
            }

            if skeleton_overlay {
                debug_lines.extend(build_skeleton_debug_lines(
                    &ea.model.parents,
                    &ea.pose_scratch,
                    &world_mat,
                    35.0,
                ));
            }
        }

        if let Some(ref c) = state.character {
            if !c.skin_scratch.is_empty() {
                skeletal_draw.push((
                    &c.gpu_mesh,
                    c.skin_scratch.as_slice(),
                    Some(c.texture_bind_groups.as_slice()),
                ));
            }
        }

        if let Some(ea) = state.enemy_asset(entry.creature_id) {
            if !ea.skin_scratch.is_empty() {
                skeletal_draw.push((
                    &ea.gpu_mesh,
                    ea.skin_scratch.as_slice(),
                    Some(ea.texture_bind_groups.as_slice()),
                ));
            }
        }

        if !debug_lines.is_empty() {
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

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        state.begin_hud_overlay();

        let white = [225, 225, 225, 255];
        let yellow = [245, 210, 65, 255];
        let cyan = [105, 215, 235, 255];
        let green = [120, 230, 150, 255];
        let muted = [165, 165, 170, 255];
        let magenta = [240, 110, 200, 255];

        let entry = state.grapple_viewer.current_entry();
        let cur_idx = state.grapple_viewer.selected_index + 1;
        let total = GRAPPLE_CATALOG.len();

        // Top Title & Encounter Selector
        state.menu_rasterizer.draw_text(
            &format!(
                "[GRAPPLE ANIMATION ROOM] ENCOUNTER [{}/{}]: {}",
                cur_idx, total, entry.name
            ),
            20,
            20,
            1,
            yellow,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!("SCENARIO: {}", entry.description),
            20,
            42,
            1,
            white,
            true,
        );

        // Creature & James Animation Playhead telemetry
        let e_clip = state.grapple_viewer.active_enemy_clip();
        let j_clip = state.grapple_viewer.active_james_clip();
        let e_time = state.grapple_viewer.enemy_time;
        let j_time = state.grapple_viewer.james_time;
        let e_dur = e_clip.duration_seconds();
        let j_dur = j_clip.duration_seconds();
        let e_f = state.grapple_viewer.enemy_current_frame();
        let j_f = state.grapple_viewer.james_current_frame();

        state.menu_rasterizer.draw_text(
            &format!(
                "ENEMY ANIMATION [1 to cycle]: {} | Frame: {:.1} (Range: {}..={}) | Rate: {:.0} fps | Time: {:.2}s / {:.2}s ({})",
                state.grapple_viewer.active_enemy_clip_name(),
                e_f,
                e_clip.start_frame,
                e_clip.end_frame,
                e_clip.fps,
                if e_dur > 0.0 { e_time % e_dur } else { 0.0 },
                e_dur,
                if e_clip.looping { "LOOP" } else { "ONCE" }
            ),
            20,
            66,
            1,
            magenta,
            true,
        );

        let j_source = state.grapple_viewer.active_james_anm_source();
        let all_sources = JamesAnmSource::all();
        let src_idx = all_sources.iter().position(|&s| s == j_source).unwrap_or(0) + 1;
        state.menu_rasterizer.draw_text(
            &format!(
                "ACTIVE ANM FILE [3 / Shift+3]: [{}/{}] {} ({} frames)",
                src_idx,
                all_sources.len(),
                j_source.label(),
                j_source.total_frames()
            ),
            20,
            88,
            1,
            cyan,
            true,
        );

        let j_header = if state.grapple_viewer.is_free_scrubbing {
            format!(
                "JAMES CLIP [FREE SCRUB] [M to exit]: {} | Frame: {:.1} (Range: {}..={}) | Rate: {:.0} fps | Time: {:.2}s / {:.2}s",
                state.grapple_viewer.active_james_clip_name(),
                j_f,
                j_clip.start_frame,
                j_clip.end_frame,
                j_clip.fps,
                if j_dur > 0.0 { j_time % j_dur } else { 0.0 },
                j_dur
            )
        } else {
            format!(
                "JAMES CLIP [2 to fwd, 4 / Shift+2 to rev]: {} | Frame: {:.1} (Range: {}..={}) | Rate: {:.0} fps | Time: {:.2}s / {:.2}s ({})",
                state.grapple_viewer.active_james_clip_name(),
                j_f,
                j_clip.start_frame,
                j_clip.end_frame,
                j_clip.fps,
                if j_dur > 0.0 { j_time % j_dur } else { 0.0 },
                j_dur,
                if j_clip.looping { "LOOP" } else { "ONCE" }
            )
        };
        state
            .menu_rasterizer
            .draw_text(&j_header, 20, 110, 1, cyan, true);

        // Vertical Elevation & Spatial Alignment readout
        let cur_elevation = state.grapple_viewer.james_y_offset;
        let def_elevation = entry.default_james_y_offset;
        let lift_delta = cur_elevation - def_elevation;
        let cur_z = entry.default_anchor_offset.z + state.grapple_viewer.relative_z_offset;
        let def_z = entry.default_anchor_offset.z;
        let z_delta = state.grapple_viewer.relative_z_offset;

        state.menu_rasterizer.draw_text(
            &format!(
                "JAMES ELEVATION (Y): {:+7.1} mm [Default: {:+7.1} mm, Delta: {:+6.1} mm] [UP/DOWN or I/K (±25mm)]",
                cur_elevation, def_elevation, lift_delta
            ),
            20,
            134,
            1,
            green,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "CREATURE DIST   (Z): {:+7.1} mm [Default: {:+7.1} mm, Delta: {:+6.1} mm] [LEFT/RIGHT or J/L (±25mm)]",
                cur_z, def_z, z_delta
            ),
            20,
            156,
            1,
            green,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!("KONAMI PS2 DATA: {}", entry.notes),
            20,
            178,
            1,
            yellow,
            true,
        );

        // Playback state & Camera telemetry
        let cam = &state.grapple_viewer.camera;
        let status_str = if state.grapple_viewer.is_paused {
            "PAUSED [Step: '[' or ']']"
        } else {
            "PLAYING [Space to Pause]"
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "STATUS: {} | SPEED: {:.2}x [-/+] | CAM DIST: {:.0} mm | PITCH: {:.1}° | YAW: {:.1}° | SKELETON: {} [B]",
                status_str,
                state.grapple_viewer.playback_speed,
                cam.distance,
                cam.pitch.to_degrees(),
                cam.yaw.to_degrees(),
                if state.grapple_viewer.skeleton_overlay {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            20,
            202,
            1,
            white,
            true,
        );

        // Bottom hotkey legend
        if state.grapple_viewer.is_free_scrubbing {
            state.menu_rasterizer.draw_text(
                "[PgUp/PgDn] Shift Window (±15)  [, / .] Window Size (±5)  [Home/End] Jump Start/End  [3/Shift+3] ANM File  [M] Exit Scrub",
                20,
                664,
                1,
                yellow,
                true,
            );
        } else {
            state.menu_rasterizer.draw_text(
                "[TAB/N/P] Encounter  [1] Enemy Clip  [2/4] James Clip  [3/Shift+3] ANM File (15 Files)  [M] Free Scrub",
                20,
                664,
                1,
                white,
                true,
            );
        }
        state.menu_rasterizer.draw_text(
            "[Up/Down or I/K] James Height (Y)  [Left/Right or J/L] Dist (Z)  [Space] Pause  [[ / ]] Step  [-/+] Speed  [R] Reset  [B] Skeleton  [Esc] Exit",
            20,
            686,
            1,
            muted,
            true,
        );

        state.finish_hud_overlay(encoder, surface_view);
    }
}
