//! 3D Model, Animation & Texture Viewer room controller (`SH2-36`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::formats::anm::{
    compute_skin_matrices_with_inv_into, evaluate_pose_into, interpolate_frames_into,
};
use sh2_debug::{
    build_skeleton_debug_lines, build_wireframe_lines, ModelCategory, StageLightingRig,
    TextureSourceMode, ViewerRenderMode, BESTIARY_CATALOG,
};
use sh2_ui::menu::TitlePhase;

pub(crate) struct ModelViewerRoomController;

impl RoomController for ModelViewerRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, _repeat: bool) {
        match key_code {
            KeyCode::Escape | KeyCode::Backspace if pressed => {
                state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                tracing::info!("Returned from Model & Texture Viewer to Test Rooms menu.");
            }
            KeyCode::KeyW | KeyCode::ArrowUp => {
                state.key_forward = pressed;
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                state.key_backward = pressed;
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                state.key_left = pressed;
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
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
            KeyCode::KeyN | KeyCode::PageDown | KeyCode::Period if pressed => {
                state.model_viewer.next_model();
                tracing::info!(
                    "[Model Viewer] Selected model [{}/{}]: {}",
                    state.model_viewer.selected_model_index + 1,
                    BESTIARY_CATALOG.len(),
                    state.model_viewer.current_entry().display_name
                );
            }
            KeyCode::KeyP | KeyCode::PageUp | KeyCode::Comma if pressed => {
                state.model_viewer.prev_model();
                tracing::info!(
                    "[Model Viewer] Selected model [{}/{}]: {}",
                    state.model_viewer.selected_model_index + 1,
                    BESTIARY_CATALOG.len(),
                    state.model_viewer.current_entry().display_name
                );
            }
            KeyCode::KeyM | KeyCode::Tab if pressed => {
                state.model_viewer.next_animation_clip();
                tracing::info!(
                    "[Model Viewer] Switched animation clip index to {}",
                    state.model_viewer.selected_anm_index
                );
            }
            KeyCode::KeyC if pressed => {
                state.model_viewer.next_subclip();
                if let Some(sc) = state.model_viewer.active_subclip() {
                    tracing::info!(
                        "[Model Viewer] Selected PS2 Sub-Clip: {} (frames {}..={} @ {:.2} FPS)",
                        sc.label,
                        sc.clip.start_frame,
                        sc.clip.end_frame,
                        sc.clip.fps
                    );
                } else {
                    tracing::info!(
                        "[Model Viewer] Selected PS2 Sub-Clip: ALL FRAMES (0..{} @ 30.0 FPS)",
                        state.model_viewer.total_frames
                    );
                }
            }
            KeyCode::Space if pressed => {
                state.model_viewer.toggle_pause();
                tracing::info!(
                    "[Model Viewer] Playback {}",
                    if state.model_viewer.is_paused {
                        "PAUSED"
                    } else {
                        "PLAYING"
                    }
                );
            }
            KeyCode::BracketLeft if pressed => {
                state.model_viewer.step_frame(-1);
                tracing::info!(
                    "[Model Viewer] Step backward -> Frame {:.0}/{}",
                    state.model_viewer.current_frame,
                    state.model_viewer.total_frames
                );
            }
            KeyCode::BracketRight if pressed => {
                state.model_viewer.step_frame(1);
                tracing::info!(
                    "[Model Viewer] Step forward -> Frame {:.0}/{}",
                    state.model_viewer.current_frame,
                    state.model_viewer.total_frames
                );
            }
            KeyCode::Minus if pressed => {
                state.model_viewer.adjust_playback_speed(-0.1);
                tracing::info!(
                    "[Model Viewer] Playback speed: {:.1}x",
                    state.model_viewer.playback_speed
                );
            }
            KeyCode::Equal if pressed => {
                state.model_viewer.adjust_playback_speed(0.1);
                tracing::info!(
                    "[Model Viewer] Playback speed: {:.1}x",
                    state.model_viewer.playback_speed
                );
            }
            KeyCode::KeyR if pressed => {
                state.model_viewer.reverse_playback();
                tracing::info!(
                    "[Model Viewer] Reversed playback speed: {:.1}x",
                    state.model_viewer.playback_speed
                );
            }
            KeyCode::KeyT if pressed => {
                state.model_viewer.turntable_enabled = !state.model_viewer.turntable_enabled;
                tracing::info!(
                    "[Model Viewer] Turntable auto-spin: {}",
                    state.model_viewer.turntable_enabled
                );
            }
            KeyCode::KeyV if pressed => {
                state.model_viewer.render_mode = state.model_viewer.render_mode.next();
                tracing::info!(
                    "[Model Viewer] Render mode: {}",
                    state.model_viewer.render_mode.label()
                );
            }
            KeyCode::KeyB if pressed => {
                state.model_viewer.skeleton_overlay = !state.model_viewer.skeleton_overlay;
                tracing::info!(
                    "[Model Viewer] Bone skeleton & joint axes overlay: {}",
                    state.model_viewer.skeleton_overlay
                );
            }
            KeyCode::KeyG if pressed => {
                state.model_viewer.wireframe_overlay = !state.model_viewer.wireframe_overlay;
                tracing::info!(
                    "[Model Viewer] Wireframe mesh overlay: {}",
                    state.model_viewer.wireframe_overlay
                );
            }
            KeyCode::KeyH if pressed => {
                state.model_viewer.texture_source = state.model_viewer.texture_source.toggle();
                tracing::info!(
                    "[Model Viewer] Texture source: {}",
                    state.model_viewer.texture_source.label()
                );
            }
            KeyCode::KeyL if pressed => {
                state.model_viewer.lighting_rig = state.model_viewer.lighting_rig.next();
                tracing::info!(
                    "[Model Viewer] Stage lighting rig: {}",
                    state.model_viewer.lighting_rig.label()
                );
            }
            KeyCode::KeyO if pressed => {
                state
                    .model_viewer
                    .camera
                    .reset_for_entry(state.model_viewer.current_entry());
                state.model_viewer.turntable_yaw = 0.0;
                tracing::info!("[Model Viewer] Reset orbit camera framing.");
            }
            KeyCode::KeyK if pressed => {
                state.debug_collision_overlay = !state.debug_collision_overlay;
                tracing::info!(
                    "[Debug] Collision wireframe overlay (clDbgDrawBGColiPoly): {}",
                    state.debug_collision_overlay
                );
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
                .model_viewer
                .camera
                .orbit(orbit_yaw_delta, orbit_pitch_delta);
            state.model_viewer.camera.zoom(zoom_delta);
            state.model_viewer.camera.pan(0.0, pan_up_delta);
            state.model_viewer.tick(dt_sec);
        }

        let view = state.model_viewer.camera.view_matrix();
        let proj = glam::Mat4::perspective_rh(
            state.model_viewer.camera.fov_y,
            aspect,
            super::DEFAULT_CAMERA_Z_NEAR,
            super::DEFAULT_CAMERA_Z_FAR,
        );
        let view_proj = proj * view;
        let inv_proj = proj.inverse();
        let eye_pos = state.model_viewer.camera.eye_position();

        let shade_mode = match state.model_viewer.render_mode {
            ViewerRenderMode::UntexturedClay => 1.0,
            ViewerRenderMode::SurfaceNormals => 2.0,
            ViewerRenderMode::Textured
            | ViewerRenderMode::Wireframe
            | ViewerRenderMode::SkeletonJoints => {
                if state.model_viewer.lighting_rig == StageLightingRig::FlashlightCone {
                    0.0
                } else {
                    3.0
                }
            }
        };
        state
            .render_ctx
            .update_camera_with_mode(view_proj, inv_proj, eye_pos, shade_mode);

        // Stage spotlight configuration
        let target = state.model_viewer.camera.target;
        let to_target = (target - eye_pos).normalize_or_zero();
        let spot_uniforms = match state.model_viewer.lighting_rig {
            StageLightingRig::StudioNeutral => sh2_renderer::light::SpotlightUniforms {
                position: [eye_pos.x, eye_pos.y - 300.0, eye_pos.z, 1.0],
                direction_inner: [to_target.x, to_target.y, to_target.z, 0.70],
                color_outer: [1.3, 1.28, 1.22, 0.35],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
            },
            StageLightingRig::HighContrastRim => sh2_renderer::light::SpotlightUniforms {
                position: [eye_pos.x, eye_pos.y - 500.0, eye_pos.z, 1.0],
                direction_inner: [to_target.x, to_target.y, to_target.z, 0.85],
                color_outer: [2.8, 2.5, 2.1, 0.60],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
            },
            StageLightingRig::FlashlightCone => sh2_renderer::light::SpotlightUniforms {
                position: [eye_pos.x, eye_pos.y, eye_pos.z, 1.0],
                direction_inner: [to_target.x, to_target.y, to_target.z, 0.92],
                color_outer: [2.6, 2.35, 1.85, 0.75],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
            },
            StageLightingRig::FullBright => sh2_renderer::light::SpotlightUniforms {
                position: [eye_pos.x, eye_pos.y, eye_pos.z, 1.0],
                direction_inner: [to_target.x, to_target.y, to_target.z, -1.0],
                color_outer: [2.0, 2.0, 2.0, -1.0],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
            },
        };
        state.render_ctx.update_spotlight(&spot_uniforms);

        state.skin_matrices_scratch.clear();
        let mut debug_lines = Vec::new();

        if let Some(ref mut c) = state.viewer_asset {
            let (f0, f1, alpha_frame) = state.model_viewer.sample_cursor();
            let frame_count = c.anm.frames.len();
            if frame_count > 0 {
                interpolate_frames_into(
                    &c.anm.frames[f0 % frame_count],
                    &c.anm.frames[f1 % frame_count],
                    alpha_frame,
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

                // Evaluate companion `.cls` facial morph cluster weights if available (`SH2-63`)
                if let Some(ref cls) = c.cls {
                    if !c.model.clusters.is_empty() {
                        cls.evaluate_global_frame_into(
                            state.model_viewer.current_frame,
                            true,
                            &mut c.cluster_weights_scratch,
                        );
                        c.model.apply_cluster_weights_into(
                            &c.cluster_weights_scratch,
                            &mut c.morphed_positions_scratch,
                        );
                        c.gpu_mesh.update_morphed_positions(
                            &state.render_ctx.queue,
                            &c.model,
                            &c.morphed_positions_scratch,
                            &mut c.gpu_vertex_scratch,
                        );
                    }
                } else {
                    c.cluster_weights_scratch.clear();
                }

                let entry = state.model_viewer.current_entry();
                let (s, c_yaw) = state.model_viewer.turntable_yaw.sin_cos();
                let (tx, ty, tz) = if entry.category == ModelCategory::WeaponOrItem
                    && !c.model.vertices.positions.is_empty()
                {
                    let mut min_p = [f32::INFINITY; 3];
                    let mut max_p = [f32::NEG_INFINITY; 3];
                    for p in &c.model.vertices.positions {
                        for k in 0..3 {
                            min_p[k] = min_p[k].min(p[k]);
                            max_p[k] = max_p[k].max(p[k]);
                        }
                    }
                    let cx = 0.5 * (min_p[0] + max_p[0]);
                    let cy = 0.5 * (min_p[1] + max_p[1]);
                    let cz = 0.5 * (min_p[2] + max_p[2]);
                    (
                        -(c_yaw * cx + s * cz),
                        entry.default_target_y - cy,
                        -(-s * cx + c_yaw * cz),
                    )
                } else {
                    (0.0, 0.0, 0.0)
                };
                let world_mat = [
                    [c_yaw, 0.0, s, tx],
                    [0.0, 1.0, 0.0, ty],
                    [-s, 0.0, c_yaw, tz],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                state.skin_matrices_scratch.reserve(c.skin_scratch.len());
                for m_skin in &c.skin_scratch {
                    let mut combined = [[0.0; 4]; 4];
                    for r in 0..4 {
                        for col in 0..4 {
                            combined[r][col] = world_mat[r][0] * m_skin[0][col]
                                + world_mat[r][1] * m_skin[1][col]
                                + world_mat[r][2] * m_skin[2][col]
                                + world_mat[r][3] * m_skin[3][col];
                        }
                    }
                    state.skin_matrices_scratch.push(combined);
                }

                if state.model_viewer.render_mode == ViewerRenderMode::Wireframe
                    || state.model_viewer.wireframe_overlay
                {
                    let has_variant_poses = c.model.primitives.iter().any(|pr| pr.pose_index > 0);
                    let has_pose_14 = c.model.primitives.iter().any(|pr| pr.pose_index == 14);
                    let has_pose_6 = c.model.primitives.iter().any(|pr| pr.pose_index == 6);
                    let mut all_tris = Vec::new();
                    for (k, pr) in c.model.primitives.iter().enumerate() {
                        let is_active = if has_variant_poses {
                            pr.pose_index == 0
                                || pr.pose_index == 3
                                || (has_pose_14 && pr.pose_index == 14)
                                || (!has_pose_14 && has_pose_6 && pr.pose_index == 6)
                        } else {
                            true
                        };
                        if is_active {
                            all_tris.extend(c.model.primitive_triangles(k));
                        }
                    }
                    let wire_positions = if c.cls.is_some()
                        && c.morphed_positions_scratch.len() == c.model.vertices.positions.len()
                    {
                        c.morphed_positions_scratch.as_slice()
                    } else {
                        c.model.vertices.positions.as_slice()
                    };
                    debug_lines.extend(build_wireframe_lines(
                        wire_positions,
                        &c.model.vertices.bone_indices,
                        &c.model.vertices.weights,
                        &state.skin_matrices_scratch,
                        &all_tris,
                        [0.15, 0.92, 0.65, 1.0],
                    ));
                }

                if state.model_viewer.render_mode == ViewerRenderMode::SkeletonJoints
                    || state.model_viewer.skeleton_overlay
                {
                    debug_lines.extend(build_skeleton_debug_lines(
                        &c.model.parents,
                        &c.pose_scratch,
                        &world_mat,
                        45.0,
                    ));
                }
            }
        }

        let skeletal_draw = if let Some(ref c) = state.viewer_asset {
            if !state.skin_matrices_scratch.is_empty() {
                let tex_slice = match state.model_viewer.texture_source {
                    TextureSourceMode::EnhancedHd => c.hd_texture_bind_groups.as_slice(),
                    TextureSourceMode::VanillaPs2 => c.vanilla_texture_bind_groups.as_slice(),
                };
                vec![(
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(tex_slice),
                )]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        state.render_ctx.render_scene_with_debug_lines(
            Some(&state.gpu_map),
            Some(&state.map_texture_bind_groups),
            &skeletal_draw,
            &debug_lines,
        );
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

        let entry = state.model_viewer.current_entry();
        let cat_str = entry.category.label();
        let total_models = BESTIARY_CATALOG.len();
        let cur_idx = state.model_viewer.selected_model_index + 1;

        state.menu_rasterizer.draw_text(
            "3D MODEL, ANIMATION & TEXTURE VIEWER [BESTIARY DEBUG]",
            24,
            18,
            2,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "MODEL [{}/{}] ({}) : {}",
                cur_idx, total_models, cat_str, entry.display_name
            ),
            24,
            46,
            2,
            white,
            true,
        );

        if let Some(ref c) = state.viewer_asset {
            let tri_count = (c.gpu_mesh.total_indices / 3) as usize;
            state.menu_rasterizer.draw_text(
                &format!(
                    "MESH STATS : {} BONES | {} VERTICES | {} TRIS | {} SUBMESHES | {} TEXTURES",
                    c.model.bone_count,
                    c.model.vertices.positions.len(),
                    tri_count,
                    c.model.primitives.len(),
                    c.model.textures.len()
                ),
                24,
                72,
                1,
                cyan,
                true,
            );
        }

        let anm_name = if entry.anm_paths.is_empty() {
            "STATIC BIND POSE"
        } else {
            entry.anm_paths[state.model_viewer.selected_anm_index % entry.anm_paths.len()]
        };
        let play_status = if state.model_viewer.is_paused {
            "PAUSED"
        } else {
            "PLAYING"
        };
        let subclips = state.model_viewer.available_subclips();
        let subclip_info = if let Some(sc) = state.model_viewer.active_subclip() {
            let idx = state.model_viewer.selected_subclip_index.unwrap_or(0) + 1;
            format!(
                "SUB-CLIP [{}/{}] {} ({}..={} @ {:.2} FPS)",
                idx,
                subclips.len(),
                sc.label,
                sc.clip.start_frame,
                sc.clip.end_frame,
                sc.clip.fps
            )
        } else if !subclips.is_empty() {
            format!(
                "SUB-CLIP [ALL/{}] FULL TRACK (PRESS [C] FOR PS2 SUB-CLIPS)",
                subclips.len()
            )
        } else {
            "SUB-CLIP [FULL TRACK]".to_string()
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "ANIM CLIP [{}/{}] : {} | {} | FRAME {:03.0}/{:03} | SPEED {:.1}X",
                state.model_viewer.selected_anm_index + 1,
                entry.anm_paths.len().max(1),
                anm_name,
                play_status,
                state.model_viewer.current_frame.floor(),
                state.model_viewer.total_frames,
                state.model_viewer.playback_speed
            ),
            24,
            92,
            1,
            green,
            true,
        );
        state
            .menu_rasterizer
            .draw_text(&subclip_info, 24, 110, 1, yellow, true);

        state.menu_rasterizer.draw_text(
            &format!(
                "SHADING : {} | OVERLAYS: BONES={} WIRE={}",
                state.model_viewer.render_mode.label(),
                if state.model_viewer.skeleton_overlay {
                    "ON"
                } else {
                    "OFF"
                },
                if state.model_viewer.wireframe_overlay {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            24,
            128,
            1,
            white,
            true,
        );

        state.menu_rasterizer.draw_text(
            &format!(
                "TEXTURES : {} | STAGE LIGHT : {} | TURNTABLE : {}",
                state.model_viewer.texture_source.label(),
                state.model_viewer.lighting_rig.label(),
                if state.model_viewer.turntable_enabled {
                    "AUTO-SPIN ON"
                } else {
                    "OFF"
                }
            ),
            24,
            146,
            1,
            cyan,
            true,
        );

        if let Some(ref c) = state.viewer_asset {
            if !c.model.clusters.is_empty() {
                let cls_summary = if let Some(ref cls) = c.cls {
                    let active_count = c
                        .cluster_weights_scratch
                        .iter()
                        .filter(|&&w| w.abs() > 0.01)
                        .count();
                    let max_w = c
                        .cluster_weights_scratch
                        .iter()
                        .fold(0.0f32, |acc, &w| acc.max(w.abs()));
                    format!(
                        "FACIAL CLUSTERS (.CLS) : {} TARGETS | {} BLOCKS ({} FRAMES) | ACTIVE: {}/{} (PEAK WEIGHT: {:.2})",
                        c.model.clusters.len(),
                        cls.blocks.len(),
                        cls.total_frames(),
                        active_count,
                        c.cluster_weights_scratch.len(),
                        max_w
                    )
                } else {
                    format!(
                        "FACIAL CLUSTERS (.MDL) : {} MORPH TARGETS READY (SELECT DEMO .ANM WITH [M/Tab] FOR LIVE .CLS FACIAL SYNC)",
                        c.model.clusters.len()
                    )
                };
                state
                    .menu_rasterizer
                    .draw_text(&cls_summary, 24, 164, 1, yellow, true);
            }
        }

        // Bottom hotkey legend
        state.menu_rasterizer.draw_text(
            "[N/P or PgDn/PgUp] Next/Prev Model  [M/Tab] Next .ANM File  [C] Next PS2 Sub-Clip  [Space] Play/Pause  [[ / ]] Step  [-/+] Speed  [R] Rev",
            24,
            666,
            1,
            muted,
            true,
        );
        state.menu_rasterizer.draw_text(
            "[V] Render Mode  [B] Bone Skeleton  [G] Wireframe  [H] Vanilla/HD Textures  [L] Stage Light  [T] Turntable  [WASD/Mouse] Orbit  [Q/E] Zoom  [Esc] Exit",
            24,
            688,
            1,
            muted,
            true,
        );

        state.finish_hud_overlay(encoder, surface_view);
    }
}
