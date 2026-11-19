//! Thin camera, skeletal head-bone anchor, look-coupled flashlight, and submesh visibility
//! wiring between `AppState`, `SimulationCore`, and `RenderContext` (`SH2-93`).

use crate::state::AppState;
use sh2_core::modern_camera::{
    ExternalCameraHandoff, PlayerMeshVisibility, STANDARD_NEAR_CLIP_UNITS,
};
use sh2_core::scenario::PlayableCharacter;
use sh2_core::CutsceneCameraSnapshot;
use sh2_renderer::light::SpotlightUniforms;
use sh2_ui::menu::TestRoomKind;

/// Synchronizes held desktop keyboard/mouse axes (`WASD`, `IJKL`, `Shift`, `R`/Right-Click) and
/// `.dds` cutscene / 2D cutaway handoffs into `state.sim_core` prior to 60 Hz stepping (`SH2-93`).
pub(crate) fn sync_host_input_and_handoffs(state: &mut AppState) {
    let cutscene_locked = state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input());
    let combat_locked = state.active_test_room == TestRoomKind::CombatArena
        && state.combat_arena.player.mode != sh2_core::combat::CombatMode::Free;

    let (move_x, move_y) = if cutscene_locked || combat_locked {
        (0.0, 0.0)
    } else {
        (
            (state.key_right as i8 - state.key_left as i8) as f32,
            (state.key_forward as i8 - state.key_backward as i8) as f32,
        )
    };
    let (look_x, look_y) = if cutscene_locked {
        (0.0, 0.0)
    } else {
        (
            (state.key_look_right as i8 - state.key_look_left as i8) as f32,
            (state.key_look_up as i8 - state.key_look_down as i8) as f32,
        )
    };

    state.sim_core.scenario.character = if state.active_maria {
        PlayableCharacter::Maria
    } else {
        PlayableCharacter::James
    };
    state.sim_core.input.set_host_keyboard_state(
        move_x,
        move_y,
        look_x,
        look_y,
        state.key_run_held && !cutscene_locked,
        state.key_ready_held && !cutscene_locked,
    );
    state
        .sim_core
        .sync_cutscene_handoff(state.drama_demo_session.as_ref().map(|s| &s.player));
    let cutaway_clone = state.sim_core.scenario.cutaway_overlay.clone();
    state.sim_core.sync_cutaway_handoff(&cutaway_clone);
}

/// Feeds evaluated model-space skeletal bone matrices (`James` / `Maria`) into `SimulationCore`
/// for First-Person head-bone eye anchoring and updates foreground `PlayerMeshVisibility` (`SH2-93`).
pub(crate) fn sync_player_pose_and_visibility(state: &mut AppState) {
    let visibility = state.sim_core.player_mesh_visibility();
    if let Some(ref mut jms) = state.character {
        let vis = if state.active_maria && state.maria.is_some() {
            PlayerMeshVisibility::FullBody
        } else {
            visibility
        };
        jms.gpu_mesh.set_player_visibility(vis);
    }
    if let Some(ref mut mar) = state.maria {
        let vis = if state.active_maria {
            visibility
        } else {
            PlayerMeshVisibility::FullBody
        };
        mar.gpu_mesh.set_player_visibility(vis);
    }

    let active_char = if state.active_maria {
        state.maria.as_ref().or(state.character.as_ref())
    } else {
        state.character.as_ref().or(state.maria.as_ref())
    };
    let pose_slice = active_char.map(|c| {
        if !c.pose_scratch.is_empty() {
            c.pose_scratch.as_slice()
        } else {
            c.model.initial_matrices.as_slice()
        }
    });
    state.sim_core.set_player_skeleton_pose(pose_slice);
}

/// Uploads the active sub-frame view/projection matrices, FOV, eye position, and perspective
/// near-clip (`8.0` FPS vs `25.0` Rail/OTS) to `RenderContext` (`SH2-93`).
pub(crate) fn update_camera_uniforms(
    state: &mut AppState,
    alpha: f32,
    cutscene_camera_override: Option<CutsceneCameraSnapshot>,
) {
    let (view, fov, cam_pos, near_clip) = match cutscene_camera_override {
        Some(cs_cam)
            if matches!(
                state.sim_core.modern_camera.handoff,
                ExternalCameraHandoff::CutsceneLocked { .. }
            ) =>
        {
            (
                cs_cam.view_matrix(),
                cs_cam.fov_y_radians,
                cs_cam.eye,
                STANDARD_NEAR_CLIP_UNITS,
            )
        }
        _ => (
            state.sim_core.active_view_matrix(alpha),
            state.sim_core.active_fov_y(alpha),
            state.sim_core.active_camera_eye(alpha),
            state.sim_core.active_near_clip(alpha),
        ),
    };
    let (proj, inv_proj) =
        state
            .render_ctx
            .perspective_projection(fov, near_clip, crate::rooms::DEFAULT_CAMERA_Z_FAR);
    state
        .render_ctx
        .update_camera(proj * view, inv_proj, cam_pos);
}

/// Uploads the active look-coupled pocket/eye flashlight ray (or `.dds` cutscene spotlight override)
/// into `RenderContext` (`SH2-93`).
pub(crate) fn update_spotlight_uniforms(
    state: &mut AppState,
    alpha: f32,
    cutscene_spotlight_override: Option<SpotlightUniforms>,
) {
    let (spot_pos, light_dir) = state.sim_core.interpolated_flashlight_ray(alpha);
    let active_demo = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input());
    let ambient_tint = active_demo
        .map(|s| s.player.atmosphere.ambient_tint_rgba)
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);

    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let effective_room_code = active_demo
        .map(|s| s.player.info.stage_name)
        .unwrap_or(state.sim_core.scenario.current_room.as_str());
    let (stage_code, room_num) = parse_ps2_stage_and_room(effective_room_code);
    let draw_env = sh2_core::lighting::lookup_draw_env(stage_code, room_num).map(|raw| {
        sh2_core::lighting::apply_draw_env_fixes(raw, effective_room_code, video.lighting_profile)
    });
    let (env_amb_rgb, outer_cos, packed_map_id, world_color_rgb) = if let Some(env) = draw_env {
        (
            env.ambient,
            env.flashlight_outer_rad.cos().clamp(0.45, 0.92),
            sh2_core::lighting::packed_map_id_from_draw_env(&env),
            Some(env.clear_color_rgb),
        )
    } else {
        (
            [0.15, 0.15, 0.15],
            std::f32::consts::FRAC_PI_4.cos(),
            None,
            None,
        )
    };
    let env_lum = env_amb_rgb[0].max(env_amb_rgb[1]).max(env_amb_rgb[2]);
    let is_dark_room = env_lum < 0.45;
    let base_amb_scale = (env_lum * 2.2).clamp(0.25, 2.2);

    let spot_uniforms = if active_demo.is_some_and(|s| s.player.closet_fix.flashlight_suppressed) {
        SpotlightUniforms {
            position: [spot_pos.x, spot_pos.y, spot_pos.z, 0.0],
            direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
            color_outer: [0.0, 0.0, 0.0, 0.0],
            ambient_tint: [
                ambient_tint[0],
                ambient_tint[1],
                ambient_tint[2],
                ambient_tint[3] * base_amb_scale,
            ],
            ..Default::default()
        }
    } else if let Some(mut cs_spot) = cutscene_spotlight_override {
        cs_spot.ambient_tint[3] *= base_amb_scale;
        cs_spot
    } else if let Some(session) = active_demo {
        let key_pos = session.player.camera.eye + glam::Vec3::new(0.0, -600.0, 0.0);
        let safe_dir = (session.player.camera.interest - key_pos)
            .normalize_or(glam::Vec3::new(0.0, 0.5, 0.86));
        SpotlightUniforms {
            position: [key_pos.x, key_pos.y, key_pos.z, 1.0],
            direction_inner: [safe_dir.x, safe_dir.y, safe_dir.z, 0.25],
            color_outer: [0.45, 0.44, 0.42, -0.50],
            ambient_tint: [
                ambient_tint[0],
                ambient_tint[1],
                ambient_tint[2],
                ambient_tint[3] * base_amb_scale,
            ],
            ..Default::default()
        }
    } else {
        let room_code = state.sim_core.scenario.current_room.as_str();
        let chapter = match state.sim_core.scenario.scenario {
            sh2_core::scenario::ScenarioKind::LetterFromSilentHeaven => {
                sh2_core::lighting::ShadowChapter::MainScenario
            }
            sh2_core::scenario::ScenarioKind::BornFromAWish => {
                sh2_core::lighting::ShadowChapter::BornFromAWish
            }
        };
        let flashlight_active = state.sim_core.scenario.is_flashlight_active();
        let darkness =
            sh2_core::lighting::evaluate_room_darkness(&sh2_core::lighting::RoomDarknessInput {
                packed_map_id,
                room_id: sh2_core::lighting::darkness_room_id_from_code(room_code),
                chapter,
                is_dark_room,
                has_flashlight: state.sim_core.scenario.has_flashlight(),
                flashlight_on: flashlight_active,
                flag_625_set: state.sim_core.scenario.has_flag(u32::from(
                    sh2_core::lighting::FLAG_WOODSIDE_FLASHLIGHT_PICKUP,
                )),
                world_color_rgb,
                lighting_profile: video.lighting_profile,
            });

        // Scale PS2 DrawEnvData.ambient (0.15..0.90) by authentic `0.5 + 0.25 * s_flash_level`
        // (`sh2pc.exe` `0x47a090..0x47a18b`), keeping sunlit rooms naturally lit.
        let modulated_ambient = if is_dark_room {
            base_amb_scale * darkness.ambient_scale
        } else {
            base_amb_scale
        };

        if flashlight_active {
            // Flashlight ON: chest-pocket forward spotlight (`Calculate_Jms_Spot` @ 0x0028e940).
            // `fl_mul = 1.2` (`EnhancedFlashlight.cpp:49`) is unit-normalized relative to the
            // 4.0 PC wall scale (`PS2_FLASHLIGHT_COLOR_SCALE = 1.2 / 4.0`), with reach `6000` / `8500` mm.
            let fl_mul =
                sh2_core::lighting::flashlight_wall_brightness_multiplier(video.lighting_profile);
            SpotlightUniforms {
                position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
                direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
                color_outer: [1.18 * fl_mul, 1.12 * fl_mul, 0.96 * fl_mul, outer_cos],
                ambient_tint: [
                    ambient_tint[0],
                    ambient_tint[1],
                    ambient_tint[2],
                    modulated_ambient,
                ],
                fill_and_range: [
                    darkness.camera_fill_k,
                    darkness.flashlight_reach_mm,
                    darkness.flash_level,
                    0.0,
                ],
            }
        } else {
            // Flashlight OFF: spotlight is completely OFF (`[0, 0, 0]`), while dark rooms receive
            // authentic `s_flash_level` ambient modulation (`0.75` pre-pickup -> `0.575` post-pickup)
            // and camera view-space directional fill (`0.075` pre-pickup -> `0.0225` post-pickup).
            SpotlightUniforms {
                position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
                direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
                color_outer: [0.0, 0.0, 0.0, outer_cos],
                ambient_tint: [
                    ambient_tint[0],
                    ambient_tint[1],
                    ambient_tint[2],
                    modulated_ambient,
                ],
                fill_and_range: [
                    darkness.camera_fill_k,
                    darkness.flashlight_reach_mm,
                    darkness.flash_level,
                    0.0,
                ],
            }
        }
    };
    state.render_ctx.update_spotlight(&spot_uniforms);
    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let specular_scale =
        sh2_core::lighting::specular_intensity_scale("", 0, video.lighting_profile);
    let opacity = f32::from(sh2_core::lighting::SH2EE_DEFAULT_SHADOW_OPACITY_U8) / 255.0;
    // Only cast directional PCSS spotlight shadows when an actual forward spotlight cone is active
    // (`outer_cos > 0` and non-zero intensity), not on wide omnidirectional indoor ambient fill lights.
    let has_active_spotlight_cone = spot_uniforms.color_outer[3] > 0.0
        && (spot_uniforms.color_outer[0]
            + spot_uniforms.color_outer[1]
            + spot_uniforms.color_outer[2])
            > 0.01;
    let effective_shadow_mode = if has_active_spotlight_cone {
        video.shadow_quality
    } else {
        sh2_core::config::ShadowQualityMode::ClassicPs2Stencil
    };
    state
        .render_ctx
        .update_shadow_config(effective_shadow_mode, opacity, specular_scale);
}

/// Splits a PS2 room code like `"rr21"` or `"ap108"` into `("rr", 21)` for `DrawEnvData` lookup.
fn parse_ps2_stage_and_room(room_code: &str) -> (&str, u16) {
    let split_idx = room_code
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(room_code.len());
    let stage = &room_code[..split_idx];
    let num = room_code[split_idx..].parse::<u16>().unwrap_or(0);
    (stage, num)
}
