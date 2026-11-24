//! 1:1 PS2 cutscene (`DramaDemoMain`) + room `DrawEnvData` multi-light uniform builder.
//!
//! Combines `DrawEnvData` room lights with active `.dds` cutscene dynamic lights using the
//! exact 12-slot [`CharacterLightPool`] layering from `SLUS_202.28`:
//! - `Event/demoview.c` `DramaDemoMain` (`0x001b0f00` lines 444–551):
//!   - Point lights -> `sh2gfw_Set_DemoPointLight` (`0x0017d600`, up to 3 slots)
//!   - Spot lights -> `sh2gfw_Set_SpotLight` (`0x0017d750`)
//!   - Infinite/directional lights -> `sh2gfw_Set_PallarelLight` (`0x0017d3c0`, slots `1..=2`, `dir = -normalize(pos)`)
//! - `Chacter_Draw/sh2gfw_model_light.c` `sh2gfw_set_CommonCharacterLight` (`0x00130d20` lines 982–1125):
//!   - Populates room `DrawEnvData` directional (`0..3`) and point (`3..7`) lights first, then overlays
//!     `.dds` parallel lights on slots `1..=2` and appends up to 3 `.dds` demo point lights.
//! - `GFW/sh2gfw_LightSet.c` `sh2gfw_LightSet_ForBG` (`0x0017daf0` lines 493–534):
//!   - Lit rooms (`sh2gfw_Get_NightOrDay() == 0`) use `Parallel3` (`sh2gfw_setparal3LGT_blk` `0x0017d7f0`).
//!   - Dark rooms (`sh2gfw_Get_NightOrDay() != 0`) use `Spot1Parallel2` (`sh2gfw_setsp1para2LGT_blk`
//!     `0x0017ddd0`) when a `.dds` spotlight is active, otherwise `Parallel3`.

use glam::Vec3;
use sh2_assets::formats::demo::{DemoLightKind, DemoLightState};
use sh2_core::lighting::DrawEnvData;

use super::character_pool::{CharacterLightPool, CharacterLightType, MODEL3_LIGHT_ENV_RADIUS_MM};
use super::jms_spot::{BASE_REFLECTION_HIGHLIGHT_RGB, DAYTIME_REFLECTION_HIGHLIGHT_SCALE};
use super::uniforms::{
    compute_ps2_falloff_params, DemoPointLightParams, ParallelLightParams, SpotlightUniforms,
    PS2_DEFAULT_FLASHLIGHT_NEAR_MM,
};
use crate::skeletal::ENVIRON_PACKET_RGB_U8;

/// Linear RGB scale converting `DrawEnvData` / `CharacterLightPool` intensities into shader linear
/// diffuse units, matching gameplay `camera_wiring.rs` (`0.07 * 2.2 = 0.154`).
pub const CHARACTER_POOL_LINEAR_SCALE: f32 = 0.07 * 2.2;

/// Maximum number of `.dds` demo point lights (`sh2gfw_Set_DemoPointLight` `0x0017d600`, 3 slots).
const MAX_DEMO_POINT_LIGHTS: usize = 3;

/// Maximum number of `.dds` demo directional lights overriding parallel slots `1..=2`
/// (`demoview.c` `0x001b0f00` line 548, `sh2gfw_Set_PallarelLight` `0x0017d3c0`).
const MAX_DEMO_PARALLEL_LIGHTS: usize = 2;

/// Inputs for evaluating 1:1 PS2 cutscene lighting (`DramaDemoMain` + `sh2gfw_set_CommonCharacterLight`).
#[derive(Debug, Clone, Copy)]
pub struct CutsceneLightInput<'a> {
    /// Active room `DrawEnvData` preset (`lookup_draw_env` + `apply_draw_env_fixes`), if resolved.
    pub draw_env: Option<&'a DrawEnvData>,
    /// All currently visible `.dds` dynamic lights (`DramaDemoPlayer::active_lights`).
    pub demo_lights: &'a [DemoLightState],
    /// Atmospheric grading ambient tint `(R, G, B, scale)` from `DramaDemoPlayer::atmosphere`.
    pub ambient_tint: [f32; 4],
    /// Base room ambient scale derived from `DrawEnvData::ambient` (`(env_lum * 2.2).clamp(0.25, 2.2)`).
    pub base_amb_scale: f32,
    /// Whether the room is dark (`env_lum < 0.45`, matching `sh2gfw_Get_NightOrDay() != 0`).
    pub is_dark_room: bool,
    /// Active cutscene camera eye position in PS2 world mm.
    pub cam_eye: Vec3,
    /// Primary cutscene actor (or player) world position in PS2 world mm.
    pub actor_pos: Vec3,
    /// True during `CS_APT_RPT_CLOSET` (`demo_no == 14`, `ClosetCutscene.cpp:164`) when spotlight
    /// emission is explicitly suppressed.
    pub flashlight_suppressed: bool,
}

/// Builds 1:1 PS2 [`SpotlightUniforms`] for an active in-engine `.dds` cutscene by populating
/// `pool` with room `DrawEnvData` lights first and layering active `.dds` lights on top, matching
/// `sh2gfw_set_CommonCharacterLight` (`0x00130d20`) and `sh2gfw_LightSet_ForBG` (`0x0017daf0`).
pub fn build_cutscene_light_uniforms(
    pool: &mut CharacterLightPool,
    input: &CutsceneLightInput<'_>,
) -> SpotlightUniforms {
    let scaled_ambient_tint = [
        input.ambient_tint[0],
        input.ambient_tint[1],
        input.ambient_tint[2],
        input.ambient_tint[3] * input.base_amb_scale,
    ];

    if input.flashlight_suppressed {
        return SpotlightUniforms {
            position: [input.cam_eye.x, input.cam_eye.y, input.cam_eye.z, 0.0],
            direction_inner: [0.0, 0.0, 1.0, 0.90],
            color_outer: [0.0, 0.0, 0.0, 0.0],
            ambient_tint: scaled_ambient_tint,
            ..Default::default()
        };
    }

    // 1. Seed `CharacterLightPool` from room `DrawEnvData` (`sh2gfw_set_CommonCharacterLight` `0x00130d20`)
    pool.delete_all();
    let refl_scale = if input.is_dark_room {
        1.0
    } else {
        DAYTIME_REFLECTION_HIGHLIGHT_SCALE
    };
    let refl_color = Vec3::from_array(BASE_REFLECTION_HIGHLIGHT_RGB) * refl_scale;
    let view_dir = (input.cam_eye - input.actor_pos).normalize_or(Vec3::Z);

    let mut next_point_slot = 3usize;
    if let Some(env) = input.draw_env {
        for (i, dl) in env.directional_lights.iter().enumerate() {
            let col = Vec3::from_array(dl.color);
            if col.max_element() > 1e-4 {
                pool.set_parallel(i, Vec3::from_array(dl.direction), col);
            }
        }
        for pt in env.point_lights.iter().take(4) {
            let col = Vec3::from_array(pt.color);
            if col.max_element() > 1e-4 && pt.far_dist > pt.near_dist {
                pool.set_point(
                    next_point_slot,
                    Vec3::from_array(pt.position),
                    col,
                    pt.near_dist,
                    pt.far_dist,
                );
                next_point_slot += 1;
            }
        }
        pool.set_reflection(
            7,
            Vec3::from_array(env.directional_lights[0].direction),
            refl_color,
        );
    } else {
        pool.set_reflection(7, view_dir, refl_color);
    }

    // 2. Layer active `.dds` cutscene lights (`DramaDemoMain` `0x001b0f00` lines 444–551):
    //    - Directional -> overrides parallel slots `1..=2` (`sh2gfw_Set_PallarelLight`, `slot + 1`)
    //    - Point -> appended into slots `8..11` (`sh2gfw_Set_DemoPointLight`, up to 3 slots)
    //    - Spot -> captured for `Spot1Parallel2` in dark rooms (`sh2gfw_Set_SpotLight`)
    let mut demo_parallel_idx = 0usize;
    let mut demo_point_idx = 0usize;
    let mut active_demo_spot: Option<DemoLightState> = None;

    for &light in input.demo_lights {
        if !light.visible {
            continue;
        }
        let col = Vec3::from_array(light.color);
        if col.max_element() <= 1e-4 {
            continue;
        }
        match light.kind {
            DemoLightKind::Directional => {
                if demo_parallel_idx < MAX_DEMO_PARALLEL_LIGHTS {
                    let dir = (-Vec3::from_array(light.position)).normalize_or(Vec3::Y);
                    pool.set_parallel(1 + demo_parallel_idx, dir, col);
                    demo_parallel_idx += 1;
                }
            }
            DemoLightKind::Point => {
                if demo_point_idx < MAX_DEMO_POINT_LIGHTS {
                    let (near_mm, far_mm) = if light.decay[1] > light.decay[0] + 1.0 {
                        (light.decay[0].max(0.0), light.decay[1])
                    } else {
                        sh2_core::demo_light_decay_bounds(light.decay[1])
                    };
                    pool.set_point(
                        8 + demo_point_idx,
                        Vec3::from_array(light.position),
                        col,
                        near_mm,
                        far_mm,
                    );
                    demo_point_idx += 1;
                }
            }
            DemoLightKind::Spot => {
                if active_demo_spot.is_none() {
                    active_demo_spot = Some(light);
                }
            }
        }
    }

    // 3. Evaluate `CharacterLightPool` matrices & top-3 extra point lights around the cutscene actor
    pool.update_parallels();
    pool.update_reflections(view_dir, Vec3::NEG_Y);
    pool.update_extras(MODEL3_LIGHT_ENV_RADIUS_MM, input.actor_pos);

    let mut pool_parallels = [ParallelLightParams::default(); 3];
    if let (Some(nlm), Some(lcm)) = (pool.nth_parallel_nlm(0), pool.nth_parallel_lcm(0)) {
        let nlm_cols = nlm.to_cols_array_2d();
        let lcm_cols = lcm.to_cols_array_2d();
        let count = pool.valid_parallel_count().min(3);
        for slot in 0..count {
            pool_parallels[slot] = ParallelLightParams {
                direction: [nlm_cols[0][slot], nlm_cols[1][slot], nlm_cols[2][slot]],
                color: [
                    lcm_cols[slot][0] * CHARACTER_POOL_LINEAR_SCALE,
                    lcm_cols[slot][1] * CHARACTER_POOL_LINEAR_SCALE,
                    lcm_cols[slot][2] * CHARACTER_POOL_LINEAR_SCALE,
                ],
                enabled: true,
            };
        }
    }
    let aux_parallels = [
        ParallelLightParams::default(),
        pool_parallels[1],
        pool_parallels[2],
    ];

    let mut extra_points = [DemoPointLightParams::default(); 3];
    let mut pt_out = 0usize;
    for i in 0..pool.valid_extra_count() {
        if pt_out >= extra_points.len() {
            break;
        }
        if let Some(entry) = pool.nth_valid_extra(i) {
            if entry.kind == CharacterLightType::Point {
                extra_points[pt_out] = DemoPointLightParams {
                    position: entry.world_pos.to_array(),
                    color: (entry.color * CHARACTER_POOL_LINEAR_SCALE).to_array(),
                    near_mm: entry.falloff_near_mm,
                    far_mm: entry.falloff_far_mm,
                    enabled: true,
                };
                pt_out += 1;
            }
        }
    }

    let nhm = pool.nhm();
    let nhm_half = Vec3::new(nhm.x_axis.z, nhm.y_axis.z, nhm.z_axis.z);
    let refl_dir = if nhm_half.length_squared() > 1e-4 {
        nhm_half.normalize()
    } else {
        view_dir
    };
    let env_alpha_u8 = (pool.reflection_brightness() * f32::from(ENVIRON_PACKET_RGB_U8))
        .round()
        .clamp(0.0, f32::from(ENVIRON_PACKET_RGB_U8)) as u8;

    // 4. Assemble final `SpotlightUniforms` matching `sh2gfw_LightSet_ForBG` (`0x0017daf0`)
    let mut u = if input.is_dark_room {
        if let Some(spot) = active_demo_spot {
            let l_pos = Vec3::from_array(spot.position);
            let l_tgt = Vec3::from_array(spot.interest);
            let dir = (l_tgt - l_pos).normalize_or(Vec3::Z);
            let inner_cos = spot.cone[0].cos().clamp(0.1, 0.99);
            let outer_cos = spot.cone[1].cos().clamp(0.05, 0.95);
            let (near_mm, far_mm) = if spot.decay[1] > spot.decay[0] + 1.0 {
                (spot.decay[0].max(0.0), spot.decay[1])
            } else {
                sh2_core::demo_light_decay_bounds(spot.decay[1])
            };
            let effective_far = far_mm.max(4500.0);
            SpotlightUniforms {
                position: [l_pos.x, l_pos.y, l_pos.z, 1.0],
                direction_inner: [dir.x, dir.y, dir.z, inner_cos],
                color_outer: [
                    spot.color[0] * CHARACTER_POOL_LINEAR_SCALE,
                    spot.color[1] * CHARACTER_POOL_LINEAR_SCALE,
                    spot.color[2] * CHARACTER_POOL_LINEAR_SCALE,
                    outer_cos,
                ],
                ambient_tint: scaled_ambient_tint,
                fill_and_range: [0.0, effective_far, 0.0, 0.0],
                ps2_falloff_params: compute_ps2_falloff_params(
                    near_mm.max(PS2_DEFAULT_FLASHLIGHT_NEAR_MM * 0.25),
                    effective_far,
                ),
                ..Default::default()
            }
        } else {
            build_parallel3_primary_uniforms(
                pool_parallels[0],
                input.actor_pos,
                scaled_ambient_tint,
            )
        }
    } else {
        build_parallel3_primary_uniforms(pool_parallels[0], input.actor_pos, scaled_ambient_tint)
    };

    u.set_parallel_lights(&aux_parallels);
    u.set_demo_point_lights(&extra_points);
    u.set_gun_and_dark_trim(0.0, 0.0, input.is_dark_room);
    u.set_reflection(
        refl_dir.to_array(),
        pool.reflection_brightness(),
        pool.reflection_color().to_array(),
        env_alpha_u8,
    );
    u
}

fn build_parallel3_primary_uniforms(
    primary: ParallelLightParams,
    anchor_pos: Vec3,
    ambient_tint: [f32; 4],
) -> SpotlightUniforms {
    let (dir, color) = if primary.enabled {
        let d = Vec3::from_array(primary.direction).normalize_or(Vec3::Y);
        (
            d,
            [primary.color[0], primary.color[1], primary.color[2], -1.0],
        )
    } else {
        (Vec3::Y, [0.0, 0.0, 0.0, -1.0])
    };
    let far_pos = anchor_pos - dir * 50_000.0;
    SpotlightUniforms {
        position: [far_pos.x, far_pos.y, far_pos.z, 0.0],
        direction_inner: [dir.x, dir.y, dir.z, -1.0],
        color_outer: color,
        ambient_tint,
        fill_and_range: [0.0, 1_000_000.0, 0.0, 0.0],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_toilet_er01_with_zero_dds_lights_preserves_room_draw_env_lights() {
        let er01_env = sh2_core::lighting::lookup_draw_env("er", 1)
            .expect("ER_DRAW_ENV_DATA room 1 must exist");
        let mut pool = CharacterLightPool::new();
        let u = build_cutscene_light_uniforms(
            &mut pool,
            &CutsceneLightInput {
                draw_env: Some(er01_env),
                demo_lights: &[],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
                base_amb_scale: 1.1,
                is_dark_room: false,
                cam_eye: Vec3::new(-19650.0, -1100.0, 18750.0),
                actor_pos: Vec3::new(-19500.0, -900.0, 18700.0),
                flashlight_suppressed: false,
            },
        );

        // Primary parallel light (`ER_DRAW_ENV_DATA[2].directional_lights[0]` = `[0.5, 0.5, 0.5]`)
        assert!(
            (u.color_outer[0] - 0.5 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4,
            "er01 primary directional light must be preserved in first_toilet: {:?}",
            u.color_outer
        );
        assert_eq!(u.direction_inner[3], -1.0);
        assert_eq!(u.color_outer[3], -1.0);

        // Overhead restroom ceiling point light (`[1.8, 2.0, 2.0]` at `[-19250, -1450, 19750]`)
        assert!(
            u.point_colors[0][3] > u.point_positions[0][3],
            "er01 overhead point light must be active in first_toilet"
        );
        assert!((u.point_positions[0][0] - (-19250.0)).abs() < 1e-2);
        assert!(
            (u.point_colors[0][1] - 2.0 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4,
            "er01 point light green channel must match gameplay scale"
        );
        assert!(
            u.reflection_color[3] > 0.0,
            "daytime reflection highlight must be active in er01 cutscene"
        );
    }

    #[test]
    fn dds_directional_light_overrides_parallel_slot_1_while_keeping_room_slot_0() {
        let er01_env = sh2_core::lighting::lookup_draw_env("er", 1)
            .expect("ER_DRAW_ENV_DATA room 1 must exist");
        let demo_dir = DemoLightState {
            kind: DemoLightKind::Directional,
            visible: true,
            position: [0.0, -1000.0, 0.0],
            color: [1.0, 0.8, 0.6],
            ..Default::default()
        };
        let mut pool = CharacterLightPool::new();
        let u = build_cutscene_light_uniforms(
            &mut pool,
            &CutsceneLightInput {
                draw_env: Some(er01_env),
                demo_lights: &[demo_dir],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
                base_amb_scale: 1.1,
                is_dark_room: false,
                cam_eye: Vec3::new(-19650.0, -1100.0, 18750.0),
                actor_pos: Vec3::new(-19500.0, -900.0, 18700.0),
                flashlight_suppressed: false,
            },
        );

        // Slot 0 remains `er01` DrawEnvData `[0.5, 0.5, 0.5]`
        assert!((u.color_outer[0] - 0.5 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4);
        // Slot 1 receives `.dds` directional light `[1.0, 0.8, 0.6]` (`demoview.c:548`)
        assert!(u.parallel_dirs[1][3] > 0.5);
        assert!((u.parallel_colors[1][0] - 1.0 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4);
        assert!((u.parallel_colors[1][1] - 0.8 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4);
    }

    #[test]
    fn dds_point_and_spot_lights_apply_in_dark_room_cutscene() {
        let demo_spot = DemoLightState {
            kind: DemoLightKind::Spot,
            visible: true,
            position: [0.0, -1500.0, 1000.0],
            interest: [0.0, -1000.0, 0.0],
            color: [1.2, 1.0, 0.8],
            decay: [500.0, 6000.0],
            cone: [0.4, 0.7],
        };
        let demo_pt = DemoLightState {
            kind: DemoLightKind::Point,
            visible: true,
            position: [500.0, -1200.0, 0.0],
            color: [0.9, 0.9, 1.0],
            decay: [300.0, 4000.0],
            ..Default::default()
        };
        let mut pool = CharacterLightPool::new();
        let u = build_cutscene_light_uniforms(
            &mut pool,
            &CutsceneLightInput {
                draw_env: None,
                demo_lights: &[demo_pt, demo_spot],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
                base_amb_scale: 0.4,
                is_dark_room: true,
                cam_eye: Vec3::new(0.0, -1500.0, 2000.0),
                actor_pos: Vec3::new(0.0, -900.0, 0.0),
                flashlight_suppressed: false,
            },
        );

        assert_eq!(
            u.position[3], 1.0,
            "dark room with .dds spot uses Spot1Parallel2"
        );
        assert!(u.color_outer[3] > 0.0, "spot outer cosine must be positive");
        assert!((u.color_outer[0] - 1.2 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4);
        assert!(
            u.point_colors[0][3] > u.point_positions[0][3],
            ".dds point light must populate extra_points[0]"
        );
        assert!((u.point_colors[0][2] - 1.0 * CHARACTER_POOL_LINEAR_SCALE).abs() < 1e-4);
    }

    #[test]
    fn closet_flashlight_suppression_zeroes_direct_light_and_preserves_ambient() {
        let mut pool = CharacterLightPool::new();
        let u = build_cutscene_light_uniforms(
            &mut pool,
            &CutsceneLightInput {
                draw_env: sh2_core::lighting::lookup_draw_env("er", 1),
                demo_lights: &[],
                ambient_tint: [1.0, 1.0, 1.0, 0.55],
                base_amb_scale: 1.0,
                is_dark_room: true,
                cam_eye: Vec3::ZERO,
                actor_pos: Vec3::ZERO,
                flashlight_suppressed: true,
            },
        );
        assert_eq!(u.color_outer, [0.0, 0.0, 0.0, 0.0]);
        assert!((u.ambient_tint[3] - 0.55).abs() < 1e-6);
    }
}
