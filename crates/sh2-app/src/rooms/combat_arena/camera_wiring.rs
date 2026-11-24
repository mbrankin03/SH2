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
    let gamepad_snapshot = state.gamepad.poll();
    state
        .sim_core
        .input
        .set_host_gamepad_state(gamepad_snapshot);
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
    let is_dds_cutscene_active = state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input());
    let enable_cutscene_unequip = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .enable_cutscene_unequip;
    let right_hand_pose = sh2_core::effective_right_hand_pose(
        state.combat_arena.player.weapon,
        enable_cutscene_unequip,
        is_dds_cutscene_active,
    );

    if let Some(ref mut jms) = state.character {
        let vis = if state.active_maria && state.maria.is_some() {
            PlayerMeshVisibility::FullBody
        } else {
            visibility
        };
        jms.gpu_mesh.set_player_visibility(vis);
        // `PlayerCheckModelParts` (`SLUS_202.28` `0x00109958`): pocket flashlight (`pose_index == 0x15` = 21)
        // is only shown when James possesses the flashlight item (`item 0x12`, Wood Side Apt Rm 205).
        jms.gpu_mesh
            .set_pose_visible(0x15, state.sim_core.scenario.has_flashlight());
        // `PlayerCheckModelParts` (`0x001098d0`) + `CutsceneUnequip` (`RemoveWeaponFromCutscene.cpp`).
        jms.gpu_mesh.set_right_hand_pose(right_hand_pose);
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
pub(crate) fn update_spotlight_uniforms(state: &mut AppState, alpha: f32) {
    let (spot_pos, light_dir) = state.sim_core.interpolated_flashlight_ray(alpha);
    let active_demo_snapshot = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
        .map(|s| {
            let demo_lights: Vec<sh2_assets::formats::demo::DemoLightState> =
                s.player.dds_lights().to_vec();
            (
                s.player.info.stage_name,
                s.player.atmosphere.ambient_tint_rgba,
                s.player.camera.eye,
                s.player
                    .player_handoff_transform()
                    .map(|(pos, _)| pos)
                    .unwrap_or(state.sim_core.scenario.player_position),
                s.player.closet_fix.flashlight_suppressed,
                demo_lights,
            )
        });
    let ambient_tint = active_demo_snapshot
        .as_ref()
        .map(|(_, tint, _, _, _, _)| *tint)
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);

    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let effective_room_code = active_demo_snapshot
        .as_ref()
        .map(|(stage, _, _, _, _, _)| *stage)
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

    let is_demo_active = active_demo_snapshot.is_some();
    let spot_uniforms =
        if let Some((_, _, cam_eye, actor_pos, flashlight_suppressed, demo_lights)) =
            active_demo_snapshot
        {
            sh2_renderer::light::build_cutscene_light_uniforms(
                &mut state.char_light_pool,
                &sh2_renderer::light::CutsceneLightInput {
                    draw_env: draw_env.as_ref(),
                    demo_lights: &demo_lights,
                    ambient_tint,
                    base_amb_scale,
                    is_dark_room,
                    cam_eye,
                    actor_pos,
                    flashlight_suppressed,
                },
            )
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
            let has_flashlight = state.sim_core.scenario.has_flashlight();
            // Sync & tick 1:1 `sh2_JmsSpot_Man.c` lifecycle (`0x0028e410..0x0028eaf0`)
            if flashlight_active && !state.jms_spot_man.spot_on {
                state.jms_spot_man.turn_on();
            } else if !flashlight_active
                && state.jms_spot_man.spot_on
                && state.jms_spot_man.ramp_step >= 0
            {
                state.jms_spot_man.turn_off();
            }
            // Step `Programed_Light_Set` (`SLUS_202.28` `0x0028eaf0`, called from 30 Hz `draw_main`
            // `0x0019feb0:169`) at exact 30 Hz (`tick_counter / 2` from the 60 Hz sim clock) for 1:1
            // flashlight turn-on (1.0 s), turn-off (0.2 s), flicker, and 15 s dark-trim adaptation.
            let ps2_draw_tick_30hz = state.sim_core.tick_counter / 2;
            state.jms_spot_man.tick_at_sim_tick(
                ps2_draw_tick_30hz,
                is_dark_room,
                has_flashlight,
                packed_map_id.unwrap_or(0),
            );
            let gun_light = state.jms_spot_man.gun_light();
            let dark_trim = state.jms_spot_man.dark_trim_factor();
            let spot_active = state.jms_spot_man.spot_on;
            let spot_gain = state.jms_spot_man.normalized_spot_intensity();

            // Populate & evaluate 12-slot `CharacterLightPool` (`Chacter_Draw/light_n.c` `0x001260b0..0x00127360`)
            state.char_light_pool.delete_all();
            // Exact `Programed_Light_Set` (`SLUS_202.28` `0x0028eaf0` lines 456-463):
            // Daytime (`sh2gfw_Get_NightOrDay() == 0`) scales `0x0037b570` (`[1, 1, 1]`) by `0.7` (`0x3f333333`);
            // nighttime uses `[1, 1, 1]` directly.
            let refl_scale = if is_dark_room {
                1.0
            } else {
                sh2_renderer::light::DAYTIME_REFLECTION_HIGHLIGHT_SCALE
            };
            let refl_color =
                glam::Vec3::from_array(sh2_renderer::light::BASE_REFLECTION_HIGHLIGHT_RGB)
                    * refl_scale;
            if let Some(env) = draw_env {
                for (i, dl) in env.directional_lights.iter().enumerate() {
                    let col = glam::Vec3::from_array(dl.color);
                    if col.max_element() > 1e-4 {
                        state.char_light_pool.set_parallel(
                            i,
                            glam::Vec3::from_array(dl.direction),
                            col,
                        );
                    }
                }
                for (i, pt) in env.point_lights.iter().take(4).enumerate() {
                    let col = glam::Vec3::from_array(pt.color);
                    if col.max_element() > 1e-4 && pt.far_dist > pt.near_dist {
                        state.char_light_pool.set_point(
                            3 + i,
                            glam::Vec3::from_array(pt.position),
                            col,
                            pt.near_dist,
                            pt.far_dist,
                        );
                    }
                }
                state.char_light_pool.set_reflection(
                    7,
                    glam::Vec3::from_array(env.directional_lights[0].direction),
                    refl_color,
                );
            } else {
                state
                    .char_light_pool
                    .set_reflection(7, light_dir, refl_color);
            }
            if spot_active {
                state.char_light_pool.set_spot(
                    8,
                    spot_pos,
                    light_dir,
                    glam::Vec3::new(1.18, 1.12, 0.96) * spot_gain,
                    outer_cos,
                    (
                        sh2_renderer::light::PS2_DEFAULT_FLASHLIGHT_NEAR_MM,
                        sh2_renderer::light::PS2_DEFAULT_FLASHLIGHT_FAR_MM,
                    ),
                );
            }
            let cam_eye = state.sim_core.active_camera_eye(alpha);
            let char_pos = state.sim_core.scenario.player_position;
            let view_dir = (cam_eye - char_pos).normalize_or(glam::Vec3::Z);
            state.char_light_pool.update_parallels();
            state
                .char_light_pool
                .update_reflections(view_dir, glam::Vec3::NEG_Y);
            state
                .char_light_pool
                .update_extras(sh2_renderer::light::MODEL3_LIGHT_ENV_RADIUS_MM, char_pos);

            let darkness = sh2_core::lighting::evaluate_room_darkness(
                &sh2_core::lighting::RoomDarknessInput {
                    packed_map_id,
                    room_id: sh2_core::lighting::darkness_room_id_from_code(room_code),
                    chapter,
                    is_dark_room,
                    has_flashlight,
                    flashlight_on: spot_active,
                    flag_625_set: state.sim_core.scenario.has_flag(u32::from(
                        sh2_core::lighting::FLAG_WOODSIDE_FLASHLIGHT_PICKUP,
                    )),
                    world_color_rgb,
                    lighting_profile: video.lighting_profile,
                },
            );

            // Scale PS2 DrawEnvData.ambient (0.15..0.90) by authentic `0.5 + 0.25 * s_flash_level`
            // (`sh2pc.exe` `0x47a090..0x47a18b`), keeping sunlit rooms naturally lit.
            let modulated_ambient = if is_dark_room {
                base_amb_scale * darkness.ambient_scale
            } else {
                base_amb_scale
            };

            const PARALLEL_LIGHT_LINEAR_SCALE: f32 = 0.07 * 2.2;
            // Extract packed parallel directional lights from `CharacterLightPool::update_parallels`
            // (`UpdateParallels` `0x00126d30`: `parallel_nlm` + `parallel_lcm` matrix 0).
            let mut pool_parallels = [sh2_renderer::light::ParallelLightParams::default(); 3];
            if let (Some(nlm), Some(lcm)) = (
                state.char_light_pool.nth_parallel_nlm(0),
                state.char_light_pool.nth_parallel_lcm(0),
            ) {
                let nlm_cols = nlm.to_cols_array_2d();
                let lcm_cols = lcm.to_cols_array_2d();
                let count = state.char_light_pool.valid_parallel_count().min(3);
                for slot in 0..count {
                    pool_parallels[slot] = sh2_renderer::light::ParallelLightParams {
                        direction: [nlm_cols[0][slot], nlm_cols[1][slot], nlm_cols[2][slot]],
                        color: [
                            lcm_cols[slot][0] * PARALLEL_LIGHT_LINEAR_SCALE,
                            lcm_cols[slot][1] * PARALLEL_LIGHT_LINEAR_SCALE,
                            lcm_cols[slot][2] * PARALLEL_LIGHT_LINEAR_SCALE,
                        ],
                        enabled: true,
                    };
                }
            }
            let aux_parallels = [
                sh2_renderer::light::ParallelLightParams::default(),
                pool_parallels[1],
                pool_parallels[2],
            ];

            // Extract top-3 sorted point lights from `CharacterLightPool::update_extras` (`0x00127360`)
            let mut extra_points = [sh2_renderer::light::DemoPointLightParams::default(); 3];
            let mut pt_out = 0usize;
            for i in 0..state.char_light_pool.valid_extra_count() {
                if pt_out >= extra_points.len() {
                    break;
                }
                if let Some(entry) = state.char_light_pool.nth_valid_extra(i) {
                    if entry.kind == sh2_renderer::light::CharacterLightType::Point {
                        extra_points[pt_out] = sh2_renderer::light::DemoPointLightParams {
                            position: entry.world_pos.to_array(),
                            color: (entry.color * PARALLEL_LIGHT_LINEAR_SCALE).to_array(),
                            near_mm: entry.falloff_near_mm,
                            far_mm: entry.falloff_far_mm,
                            enabled: true,
                        };
                        pt_out += 1;
                    }
                }
            }

            // Extract NHM halfway vector (`UpdateReflections` `0x00126f90`) and `MakeEnvironPacket`
            // (`0x0012cf20`) environment reflection alpha byte (`0..=0x80`).
            let nhm = state.char_light_pool.nhm();
            let nhm_half = glam::Vec3::new(nhm.x_axis.z, nhm.y_axis.z, nhm.z_axis.z);
            let refl_dir = if nhm_half.length_squared() > 1e-4 {
                nhm_half.normalize()
            } else {
                light_dir
            };
            let env_alpha_u8 = (state.char_light_pool.reflection_brightness()
                * f32::from(sh2_renderer::skeletal::ENVIRON_PACKET_RGB_U8))
            .round()
            .clamp(
                0.0,
                f32::from(sh2_renderer::skeletal::ENVIRON_PACKET_RGB_U8),
            ) as u8;

            if spot_active {
                // Flashlight ON (including 30-frame turn-on ramp, 6-frame turn-off ramp, and LCG flicker):
                // 1-spot + 2-parallel (`sh2gfw_setsp1para2LGT_blk` @ 0x0017ddd0, `Calculate_Jms_Spot` @ 0x0028e940).
                let fl_mul = sh2_core::lighting::flashlight_wall_brightness_multiplier(
                    video.lighting_profile,
                ) * spot_gain;
                let mut u = SpotlightUniforms {
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
                    ..Default::default()
                };
                u.set_parallel_lights(&aux_parallels);
                u.set_demo_point_lights(&extra_points);
                u.set_ps2_falloff(
                    sh2_renderer::light::PS2_DEFAULT_FLASHLIGHT_NEAR_MM,
                    darkness.flashlight_reach_mm,
                    false,
                );
                u.set_gun_and_dark_trim(gun_light, dark_trim, is_dark_room);
                u.set_reflection(
                    refl_dir.to_array(),
                    state.char_light_pool.reflection_brightness(),
                    state.char_light_pool.reflection_color().to_array(),
                    env_alpha_u8,
                );
                u
            } else if !is_dark_room {
                // Daytime / lit room (`sh2gfw_Get_NightOrDay() == 0`): 3-parallel directional lights
                // (`sh2gfw_setparal3LGT_blk` @ `SLUS_202.28` `0x0017d7f0`), sourced from `parallel_nlm` / `parallel_lcm`.
                let (dir, color) = if pool_parallels[0].enabled {
                    let d = glam::Vec3::from_array(pool_parallels[0].direction)
                        .normalize_or(glam::Vec3::new(0.0, 1.0, 0.0));
                    (
                        d,
                        [
                            pool_parallels[0].color[0],
                            pool_parallels[0].color[1],
                            pool_parallels[0].color[2],
                            -1.0,
                        ],
                    )
                } else {
                    (glam::Vec3::new(0.0, 1.0, 0.0), [0.0, 0.0, 0.0, -1.0])
                };
                let far_pos = spot_pos - dir * 50_000.0;
                let mut u = SpotlightUniforms {
                    position: [far_pos.x, far_pos.y, far_pos.z, 0.0],
                    direction_inner: [dir.x, dir.y, dir.z, -1.0],
                    color_outer: color,
                    ambient_tint: [
                        ambient_tint[0],
                        ambient_tint[1],
                        ambient_tint[2],
                        modulated_ambient,
                    ],
                    fill_and_range: [0.0, 1_000_000.0, darkness.flash_level, 0.0],
                    ..Default::default()
                };
                u.set_parallel_lights(&aux_parallels);
                u.set_demo_point_lights(&extra_points);
                u.set_gun_and_dark_trim(gun_light, dark_trim, false);
                u.set_reflection(
                    refl_dir.to_array(),
                    state.char_light_pool.reflection_brightness(),
                    state.char_light_pool.reflection_color().to_array(),
                    env_alpha_u8,
                );
                u
            } else {
                // Dark room, Flashlight OFF: spotlight is OFF (`[0, 0, 0]`), while dark rooms receive
                // authentic `s_flash_level` ambient modulation, `DARK_Hosei` trim, and aux parallels.
                let mut u = SpotlightUniforms {
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
                    ..Default::default()
                };
                u.set_parallel_lights(&aux_parallels);
                u.set_demo_point_lights(&extra_points);
                u.set_gun_and_dark_trim(gun_light, dark_trim, true);
                u.set_reflection(
                    refl_dir.to_array(),
                    state.char_light_pool.reflection_brightness(),
                    state.char_light_pool.reflection_color().to_array(),
                    env_alpha_u8,
                );
                u
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
    state.dust_spotlight = sh2_core::lighting::DustSpotlightParams {
        enabled: has_active_spotlight_cone,
        position_mm: glam::Vec3::new(
            spot_uniforms.position[0],
            spot_uniforms.position[1],
            spot_uniforms.position[2],
        ),
        direction: glam::Vec3::new(
            spot_uniforms.direction_inner[0],
            spot_uniforms.direction_inner[1],
            spot_uniforms.direction_inner[2],
        )
        .normalize_or(glam::Vec3::NEG_Z),
        outer_cos: spot_uniforms.color_outer[3],
        range_mm: spot_uniforms.fill_and_range[1],
    };
    let effective_shadow_mode = if has_active_spotlight_cone {
        video.shadow_quality
    } else {
        sh2_core::config::ShadowQualityMode::ClassicPs2Stencil
    };
    state
        .render_ctx
        .update_shadow_config(effective_shadow_mode, opacity, specular_scale);

    // Sync SH2-136 PS2 room clear color (`0x00175930`), soft stencil shadow blur (`0x001767e0`),
    // dual GS composite mode (`0x00175f90`), and `Check_DrawOrder` (`0x001a08a0`).
    let room_clear_rgba = world_color_rgb.map(|rgb| {
        [
            rgb[0],
            rgb[1],
            rgb[2],
            if is_dark_room { 0x7c } else { 0x40 },
        ]
    });
    state
        .render_ctx
        .set_room_clear_color(is_dark_room, room_clear_rgba);
    state.render_ctx.set_soft_shadow_blur_enabled(
        effective_shadow_mode != sh2_core::config::ShadowQualityMode::NativePcss,
    );
    state.render_ctx.set_shadow_composite_mode(if is_dark_room {
        sh2_renderer::shadow::ShadowCompositeMode::Multiplicative
    } else {
        sh2_renderer::shadow::ShadowCompositeMode::Subtractive
    });
    state.render_ctx.set_shadow_blur_opacity(opacity);
    let demo_num = if is_demo_active { 1 } else { 0 };
    state
        .render_ctx
        .set_shadow_draw_order(sh2_renderer::shadow::check_draw_order(
            packed_map_id.unwrap_or(0),
            demo_num,
        ));
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
