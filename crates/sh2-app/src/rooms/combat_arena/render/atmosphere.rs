//! Atmospheric fog uniforms, UV-scrolling screen fog (`fog_view_screen_fog`), 3D proximity fog
//! sprites, and steam-vent (`fog_blow`) particle updates.

use crate::state::AppState;
use sh2_ui::menu::TestRoomKind;

pub(crate) fn update_atmosphere_fog_uniforms(state: &mut AppState, alpha: f32) {
    let det_fog_time = state.sim_core.deterministic_fog_time(alpha);
    let prev_sim_tick = (state.last_fog_time.max(0.0) * 60.0).floor() as u64;
    let curr_sim_tick = (det_fog_time.max(0.0) * 60.0).floor() as u64;
    let blow_frame_steps = curr_sim_tick.saturating_sub(prev_sim_tick).min(6) as i32;
    let fog_dt = (det_fog_time - state.last_fog_time).clamp(0.0, 0.1);
    state.last_fog_time = det_fog_time;
    state.fog_uniforms.time = det_fog_time;

    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let mut effective_fog = state.fog_uniforms;
    effective_fog.apply_video_options(video.fog_quality, video.volumetric_light);
    if let Some(ref session) = state.drama_demo_session {
        if session.player.locks_gameplay_input() {
            if let Some(rgb) = session.player.atmosphere.horizon_fog_rgb_override {
                effective_fog.fog_color[0] = rgb[0];
                effective_fog.fog_color[1] = rgb[1];
                effective_fog.fog_color[2] = rgb[2];
            }
            effective_fog.fog_color[3] *= session.player.atmosphere.fog_density_scale;
        }
    }
    // Step 1:1 `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`) UV-scrolling screen fog overlay
    let (cam_eye, view_mat) = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
        .map(|s| (s.player.camera.eye, s.player.camera.view_matrix()))
        .unwrap_or_else(|| {
            (
                state.sim_core.active_camera_eye(alpha),
                state.sim_core.active_view_matrix(alpha),
            )
        });
    let cam_delta = cam_eye - state.last_screen_fog_cam;
    state.last_screen_fog_cam = cam_eye;
    if effective_fog.layer2_params[3] > 0.5 && effective_fog.density > 0.0 {
        let fog_alpha_param = (effective_fog.layer_transparencies[0] * 128.0)
            .round()
            .clamp(0.0, 128.0) as u8;
        let fog_color_a = (effective_fog.fog_color[3]
            * f32::from(sh2_renderer::fog::PS2_FOG_DEFAULT_COLOR_ALPHA_U8))
        .round()
        .clamp(0.0, 255.0) as u8;
        let screen_eval = state
            .screen_fog_state
            .step(&sh2_renderer::fog::ScreenFogStepInput {
                cam_delta,
                area_size: sh2_core::fog::PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                wind_vec: sh2_renderer::fog::PS2_DEFAULT_FOG_WIND_VEC,
                dt_scale: fog_dt * 60.0 * effective_fog.speed_mult,
                world_view_rot: glam::Mat3::from_mat4(view_mat),
                fog_color_a,
                fog_alpha_param,
            });
        effective_fog.screen_fog_uv = state.screen_fog_state.to_gpu_vec4(&screen_eval);
    } else {
        effective_fog.screen_fog_uv[3] = 0.0;
    }
    state.render_ctx.update_fog(&effective_fog);

    // Step and upload localized 3D proximity fog sprites decoupled from display refresh rate (`SH2-32`)
    if effective_fog.layer2_params[3] > 0.5 && effective_fog.density > 0.0 {
        let player_pos = state.sim_core.scenario.player_position;
        let ps2_room_id = {
            let from_room =
                sh2_core::fog::room_id_from_stage_code(&state.sim_core.scenario.current_room);
            if from_room != 0 {
                from_room
            } else if matches!(
                state.active_test_room,
                TestRoomKind::ObservationDeck | TestRoomKind::SyntheticFogRunway
            ) {
                sh2_core::fog::R_OBSV_DECK_ID
            } else {
                0
            }
        };
        let room_eval = sh2_core::fog::evaluate_room_fog(&sh2_core::fog::RoomFogContext {
            room_id: ps2_room_id,
            james_pos_y_mm: player_pos.y,
            camera_pos_y_mm: state.sim_core.active_camera_eye(alpha).y,
            flashlight_on: state.sim_core.scenario.is_flashlight_active(),
            extended_observation_deck_range: true,
        });
        let target_count = if room_eval.particle_count > 0 {
            room_eval.particle_count
        } else {
            sh2_core::fog::PS2_DEFAULT_FOG_PARTICLE_COUNT
        };
        state.fog_particles.reconfigure(
            target_count,
            room_eval.particle_range_x_mm,
            room_eval.particle_range_z_mm,
        );
        // PS2 Y-down coordinate system: when `.fcl` specifies a fixed Y altitude (`fogSetLocalPosV`
        // `SLUS_202.28` `0x0016c7c8..0x0016c7e4`, e.g. `obGB.fcl` `+10,000 mm` below the cliff),
        // anchor particles at that exact altitude; otherwise center mist puffs 520 mm above floor.
        let anchor_y = room_eval
            .particle_fixed_y_mm
            .unwrap_or(player_pos.y - 520.0);
        let anchor = [player_pos.x, anchor_y, player_pos.z];
        let player_arr = [player_pos.x, player_pos.y, player_pos.z];
        state.fog_particles.step(
            fog_dt,
            effective_fog.speed_mult,
            anchor,
            player_arr,
            room_eval.james_fog_influence_mm,
        );
        let fog_rgb = [
            effective_fog.fog_color[0],
            effective_fog.fog_color[1],
            effective_fog.fog_color[2],
        ];
        state.fog_particle_scratch.clear();
        state
            .fog_particle_scratch
            .extend(state.fog_particles.particles().iter().map(|p| {
                sh2_renderer::fog::GpuFogParticle {
                    world_pos_radius: [
                        anchor[0] + p.local_pos[0],
                        anchor[1] + p.local_pos[1],
                        anchor[2] + p.local_pos[2],
                        p.radius_mm,
                    ],
                    color_alpha: [fog_rgb[0], fog_rgb[1], fog_rgb[2], p.alpha],
                }
            }));
        // Step 1:1 `Fog/fog_blow.c` (`0x00274ca0..0x00275480`) steam-vent particles for rooms 0x8e/0x8f/0x90
        if state.fog_blow_system.active_room_id != ps2_room_id {
            state.fog_blow_system.set_room(ps2_room_id);
        }
        state
            .fog_blow_system
            .tick(fog_dt, blow_frame_steps, player_pos);
        state
            .fog_blow_system
            .write_gpu_instances(fog_rgb, &mut state.fog_particle_scratch);
        state
            .render_ctx
            .update_fog_particles(&state.fog_particle_scratch);
    } else {
        state.render_ctx.update_fog_particles(&[]);
    }
}
