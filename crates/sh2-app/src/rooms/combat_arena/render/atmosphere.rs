//! Atmospheric fog uniforms, UV-scrolling screen fog (`fog_view_screen_fog`), 3D proximity fog
//! sprites, and steam-vent (`fog_blow`) particle updates.

use crate::state::AppState;
use sh2_core::config::VideoOptions;
use sh2_core::fog::{
    evaluate_room_fog_with_fixes, room_id_from_stage_code, RoomFogContext, RoomFogEvaluation,
    R_OBSV_DECK_ID,
};
use sh2_renderer::fog::{
    FogUniforms, GpuFogParticle, PS2_DEFAULT_FOG_WIND_VEC, PS2_FOG_DEFAULT_COLOR_ALPHA_U8,
    ROOM_BLOW_PS185, ROOM_BLOW_PS189, ROOM_BLOW_PS193,
};
use sh2_ui::menu::TestRoomKind;

/// Resolves the canonical PS2/PC `room_id` from `ScenarioState::current_room`, falling back to
/// `R_OBSV_DECK_ID` for outdoor observation-deck test rooms when `current_room` is empty/unmapped.
pub(crate) fn resolve_ps2_room_id(current_room: &str, active_test_room: TestRoomKind) -> u32 {
    let from_room = room_id_from_stage_code(current_room);
    if from_room != 0 {
        from_room
    } else if matches!(
        active_test_room,
        TestRoomKind::ObservationDeck | TestRoomKind::SyntheticFogRunway
    ) {
        R_OBSV_DECK_ID
    } else {
        0
    }
}

/// Evaluates per-frame room fog (`evaluate_room_fog_with_fixes`) and applies it to `base_fog`
/// when `ps2_room_id != 0`, followed by `VideoOptions` (`fog_quality`, `volumetric_light`).
pub(crate) fn evaluate_effective_room_fog(
    base_fog: &FogUniforms,
    ps2_room_id: u32,
    player_pos_y_mm: f32,
    camera_pos_y_mm: f32,
    flashlight_on: bool,
    video: &VideoOptions,
    enable_sh2ee_world_fixes: bool,
) -> (FogUniforms, RoomFogEvaluation) {
    let room_eval = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: ps2_room_id,
            james_pos_y_mm: player_pos_y_mm,
            camera_pos_y_mm,
            flashlight_on,
            extended_observation_deck_range: enable_sh2ee_world_fixes,
        },
        enable_sh2ee_world_fixes,
    );
    let mut effective_fog = *base_fog;
    if ps2_room_id != 0 {
        effective_fog.apply_room_evaluation(&room_eval);
    }
    effective_fog.apply_video_options(video.fog_quality, video.volumetric_light);
    (effective_fog, room_eval)
}

pub(crate) fn update_atmosphere_fog_uniforms(state: &mut AppState, alpha: f32) {
    let det_fog_time = state.sim_core.deterministic_fog_time(alpha);
    let curr_sim_tick = state.sim_core.tick_counter;
    let blow_frame_steps = curr_sim_tick
        .saturating_sub(state.last_fog_blow_tick)
        .min(6) as i32;
    state.last_fog_blow_tick = curr_sim_tick;
    let fog_dt = (det_fog_time - state.last_fog_time).clamp(0.0, 0.1);
    state.last_fog_time = det_fog_time;
    state.fog_uniforms.time = det_fog_time;

    // Step 1:1 `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`) camera/view lookup
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
    let player_pos = state.sim_core.scenario.player_position;
    let ps2_room_id = resolve_ps2_room_id(
        &state.sim_core.scenario.current_room,
        state.active_test_room,
    );

    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let enable_sh2ee_world_fixes = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .enable_sh2ee_world_fixes;
    let (mut effective_fog, room_eval) = evaluate_effective_room_fog(
        &state.fog_uniforms,
        ps2_room_id,
        player_pos.y,
        cam_eye.y,
        state.sim_core.scenario.is_flashlight_active(),
        video,
        enable_sh2ee_world_fixes,
    );

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

    let cam_delta = cam_eye - state.last_screen_fog_cam;
    state.last_screen_fog_cam = cam_eye;
    if effective_fog.layer2_params[3] > 0.5 && effective_fog.density > 0.0 {
        let fog_alpha_param = (effective_fog.layer_transparencies[0] * 128.0)
            .round()
            .clamp(0.0, 128.0) as u8;
        let fog_color_a = (effective_fog.fog_color[3] * f32::from(PS2_FOG_DEFAULT_COLOR_ALPHA_U8))
            .round()
            .clamp(0.0, 255.0) as u8;
        let screen_eval = state
            .screen_fog_state
            .step(&sh2_renderer::fog::ScreenFogStepInput {
                cam_delta,
                area_size: sh2_core::fog::PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                wind_vec: PS2_DEFAULT_FOG_WIND_VEC,
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

    let fog_rgb = [
        effective_fog.fog_color[0],
        effective_fog.fog_color[1],
        effective_fog.fog_color[2],
    ];
    state.fog_particle_scratch.clear();

    // Step and upload localized 3D proximity fog sprites decoupled from display refresh rate (`SH2-32`)
    if effective_fog.layer2_params[3] > 0.5 && effective_fog.density > 0.0 {
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
        // Exact `fogSetLocalPosV` (`SLUS_202.28` `0x0016c6d0`, `Fog/fog.c:2965-2983`) +
        // `obGB.fcl` floor cutoff (`param_1[5] = 0`, `param_1[6] = 700`, `bVar2 = 0x21` bit 0):
        // On `ob01`, keep mist below the parking lot deck (`player_pos.y + 700.0 mm` in PS2 Y-down)
        // so depth testing keeps the concrete & car crisp while billows rise behind the stone wall.
        let cam_to_player = player_pos - cam_eye;
        let cam_dist = cam_to_player.length();
        let anchor = if ps2_room_id == R_OBSV_DECK_ID {
            let horiz = if cam_dist > 2_000.0 {
                cam_eye + (cam_to_player / cam_dist) * 2_000.0
            } else {
                player_pos
            };
            [
                horiz.x,
                player_pos.y + sh2_core::fog::OB_FOG_ENV_PARTICLE_SIZE_MM,
                horiz.z,
            ]
        } else if cam_dist > 2_000.0 {
            let projected = cam_eye + (cam_to_player / cam_dist) * 2_000.0;
            [
                projected.x,
                projected.y.max(player_pos.y - 400.0),
                projected.z,
            ]
        } else {
            let anchor_y = room_eval
                .particle_fixed_y_mm
                .unwrap_or(player_pos.y - 520.0);
            [player_pos.x, anchor_y, player_pos.z]
        };
        let player_arr = [player_pos.x, player_pos.y, player_pos.z];
        state.fog_particles.step(
            fog_dt,
            effective_fog.speed_mult,
            anchor,
            player_arr,
            room_eval.james_fog_influence_mm,
        );
        // `fogInit` (`SLUS_202.28` `0x001687d0`, `Fog/fog.c:67`) uses `fogSetColor(0x80, 0x80, 0x80, 0x80)`
        // (unity modulation of `fog.tex` bright mist) rather than dark `0x46` `FOGCOL`.
        let particle_rgb = if ps2_room_id == R_OBSV_DECK_ID {
            sh2_core::fog::PS2_FOG_PARTICLE_MIST_RGB
        } else {
            fog_rgb
        };
        let radius_scale = if ps2_room_id == R_OBSV_DECK_ID {
            sh2_core::fog::OB_FOG_ENV_PARTICLE_SIZE_MM / 300.0
        } else {
            1.0
        };
        state
            .fog_particle_scratch
            .extend(
                state
                    .fog_particles
                    .particles()
                    .iter()
                    .map(|p| GpuFogParticle {
                        world_pos_radius: [
                            anchor[0] + p.local_pos[0],
                            anchor[1] + p.local_pos[1],
                            anchor[2] + p.local_pos[2],
                            p.radius_mm * radius_scale,
                        ],
                        color_alpha: [particle_rgb[0], particle_rgb[1], particle_rgb[2], p.alpha],
                    }),
            );
    }

    // Step 1:1 `Fog/fog_blow.c` (`0x00274ca0..0x00275480`) steam-vent particles for indoor
    // meat-locker / Eddie boss rooms (`0x8e`, `0x8f`, `0x90`) on fixed 60 Hz ticks.
    if state.fog_blow_system.active_room_id != ps2_room_id {
        state.fog_blow_system.set_room(ps2_room_id);
    }
    if effective_fog.density > 0.0
        && matches!(
            ps2_room_id,
            ROOM_BLOW_PS185 | ROOM_BLOW_PS189 | ROOM_BLOW_PS193
        )
    {
        for _ in 0..blow_frame_steps {
            state.fog_blow_system.tick(1.0 / 60.0, 1, player_pos);
        }
        state
            .fog_blow_system
            .write_gpu_instances(fog_rgb, &mut state.fog_particle_scratch);
    }

    state
        .render_ctx
        .update_fog_particles(&state.fog_particle_scratch);

    // Step `Effect2/hh_class_dust_00.c` (`0x00257f30`) airborne dust motes on fixed 60 Hz ticks
    // and evaluate `HH_ClassWrapper_SpotLight_ColorRatio_Calculator` (`0x00269840`).
    let dust_steps = curr_sim_tick.saturating_sub(state.last_dust_tick).min(6) as u32;
    state.last_dust_tick = curr_sim_tick;
    for _ in 0..dust_steps {
        state.dust_system.tick(sh2_core::lighting::DUST_DT_60HZ);
    }
    state.dust_particle_scratch.clear();
    for mote in &state.dust_system.motes {
        state
            .dust_particle_scratch
            .push(sh2_renderer::effects::GpuDustParticle::from_mote(
                mote,
                &state.dust_spotlight,
            ));
    }
    state
        .render_ctx
        .update_dust_particles(&state.dust_particle_scratch);

    // Step `Effect2/hh_class_blood_*.c` Impact Queue Blood System (`SH2-79`)
    let blood_steps = curr_sim_tick.saturating_sub(state.last_blood_tick).min(6) as u32;
    state.last_blood_tick = curr_sim_tick;
    let blood_cfg = sh2_core::effects::BloodSystemConfig {
        blood_color: state.menu_ctrl.options_menu.draft_config.game.blood_color,
        blood_size_scale: sh2_core::effects::SH2EE_BLOOD_SIZE_SCALE,
        persistent_room_decals: state
            .menu_ctrl
            .options_menu
            .draft_config
            .game
            .enable_persistent_blood_decals,
        non_subtractive_fallback: false,
    };
    let footstep_now = state.sim_core.blood_footstep_triggered;
    state.sim_core.blood_footstep_triggered = false;
    let player_pos = state.sim_core.scenario.player_position;
    let player_yaw = state.sim_core.scenario.player_yaw;
    let collision = &state.sim_core.collision;
    for step_idx in 0..blood_steps {
        state.blood_system.step(
            sh2_core::effects::blood::BLOOD_DT_60HZ,
            |pos| {
                collision
                    .sample_floor_surface(pos.x, pos.z, pos.y - 300.0, 2000.0, 1.0)
                    .map_or(player_pos.y, |s| s.height_y)
            },
            player_pos,
            player_yaw,
            footstep_now && step_idx == 0,
            blood_cfg,
        );
    }
    state
        .blood_system
        .build_render_items(blood_cfg, &mut state.blood_item_scratch);
    state.blood_quad_scratch.clear();
    for item in &state.blood_item_scratch {
        state
            .blood_quad_scratch
            .push(sh2_renderer::effects::GpuBloodQuad::from_render_item(item));
    }
    state.render_ctx.update_blood_quads(
        &state.blood_quad_scratch,
        !blood_cfg.non_subtractive_fallback,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_core::fog::{
        BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM, BLUE_CREEK_ORIGINAL_FOG_NEAR_MM, R_APT_W_RM_208_209_ID,
        R_EDI_BOSS_CORRIDOR_ID, R_EDI_BOSS_RM_1_ID, R_EDI_BOSS_RM_2_ID,
    };

    #[test]
    fn blue_creek_209_updates_fog_near_and_depth_coeffs_per_frame() {
        let room_id = resolve_ps2_room_id("ap28", TestRoomKind::ObservationDeck);
        assert_eq!(room_id, R_APT_W_RM_208_209_ID);

        let base = FogUniforms::default();
        let video = VideoOptions::default();

        let (off_fog, off_eval) =
            evaluate_effective_room_fog(&base, room_id, 0.0, -500.0, false, &video, true);
        let (on_fog, _on_eval) =
            evaluate_effective_room_fog(&base, room_id, 0.0, -500.0, true, &video, true);
        let (vanilla_on_fog, _vanilla_eval) =
            evaluate_effective_room_fog(&base, room_id, 0.0, -500.0, true, &video, false);

        assert!(!off_eval.outdoor_layers_enabled);
        assert_eq!(off_fog.layer2_params[3], 0.0);
        assert_eq!(off_fog.fog_near, BLUE_CREEK_ORIGINAL_FOG_NEAR_MM);
        assert_eq!(on_fog.fog_near, BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM);
        assert_eq!(vanilla_on_fog.fog_near, BLUE_CREEK_ORIGINAL_FOG_NEAR_MM);
        assert_ne!(off_fog.depth_fog_params, on_fog.depth_fog_params);
    }

    #[test]
    fn final_boss_arena_updates_fog_uniforms_above_camera_y_threshold() {
        let room_id = resolve_ps2_room_id("ru73", TestRoomKind::ObservationDeck);
        let base = FogUniforms::default();
        let video = VideoOptions::default();

        let (lower_fog, lower_eval) =
            evaluate_effective_room_fog(&base, room_id, 0.0, -12000.0, true, &video, true);
        let (upper_fog, upper_eval) =
            evaluate_effective_room_fog(&base, room_id, 0.0, -16000.0, true, &video, true);

        assert!(!lower_eval.final_boss_upper_override);
        assert!(upper_eval.final_boss_upper_override);
        assert_ne!(lower_fog.fog_color, upper_fog.fog_color);
        assert!(upper_fog.fog_far > lower_fog.fog_far);
        assert_ne!(lower_fog.density, upper_fog.density);
        assert_ne!(lower_fog.depth_fog_params, upper_fog.depth_fog_params);
    }

    #[test]
    fn eddie_meat_locker_rooms_resolve_and_reach_fog_blow_system() {
        let mut blow = sh2_renderer::fog::FogBlowSystem::new();
        for (stage_code, expected_id) in [
            ("ps185", R_EDI_BOSS_CORRIDOR_ID),
            ("ps189", R_EDI_BOSS_RM_1_ID),
            ("ps193", R_EDI_BOSS_RM_2_ID),
        ] {
            let room_id = resolve_ps2_room_id(stage_code, TestRoomKind::ObservationDeck);
            assert_eq!(room_id, expected_id);
            let (fog, eval) = evaluate_effective_room_fog(
                &FogUniforms::default(),
                room_id,
                0.0,
                -500.0,
                true,
                &VideoOptions::default(),
                true,
            );
            assert!(!eval.outdoor_layers_enabled);
            assert!(fog.density > 0.0);
            blow.set_room(room_id);
            assert_eq!(blow.active_room_id, expected_id);
        }
    }
}
