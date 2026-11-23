//! Deterministic golden-vector and retail-asset integration tests for **SH2-135**:
//! 1:1 multi-light world/character pipeline, linear/hyperbolic flashlight attenuation & lifecycle,
//! 64×64 sphere-map specular, and PS2 depth/screen/blow fog.

use glam::{Mat3, Vec3};
use sh2_core::config::FogQuality;
use sh2_renderer::fog::{
    DepthFogCoeffs, FogBlowSystem, FogUniforms, ScreenFogState, ScreenFogStepInput,
    BLOW_POINT_PS185, BLOW_POINT_PS189, BLOW_POINT_PS193, PS2_DEFAULT_FOG_FAR_MM,
    PS2_DEFAULT_FOG_NEAR_MM, ROOM_BLOW_PS185, ROOM_BLOW_PS189, ROOM_BLOW_PS193,
};
use sh2_renderer::light::{
    compute_ps2_falloff_params, eval_hyperbolic_falloff, eval_linear_falloff, CharacterLightPool,
    DemoPointLightParams, JmsSpotManager, ParallelLightParams, SpotlightUniforms,
    CHARACTER_EXTRA_LIGHT_LIMIT, CHARACTER_LIGHT_SLOT_COUNT, DARK_HOSEI, PITCH_BLACK_MAP_IDS,
    PS2_DEFAULT_FLASHLIGHT_FAR_MM, PS2_DEFAULT_FLASHLIGHT_NEAR_MM, PS2_DEFAULT_FLASHLIGHT_POS_Y_MM,
    PS2_NHM_UV_BIAS_ROW, SPOT_LCG_INCREMENT, SPOT_LCG_MULTIPLIER,
};
use sh2_renderer::skeletal::{
    composite_gs_specular, compute_sphere_map_uv, environ_packet_alpha,
    generate_specular_sphere_map_rgba, GS_ALPHA_1_SPECULAR_DAY, GS_ALPHA_1_SPECULAR_NIGHT,
    SPECULAR_SPHERE_MAP_ALPHA_U8, SPECULAR_SPHERE_MAP_DIM,
};
use sh2_renderer::{RenderContext, RendererConfig};

#[test]
fn golden_ps2_falloff_and_multi_light_uniforms() {
    let mut u = SpotlightUniforms::default();
    assert_eq!(std::mem::size_of::<SpotlightUniforms>(), 336);
    assert_eq!(u.position[1], PS2_DEFAULT_FLASHLIGHT_POS_Y_MM);
    assert_eq!(u.fill_and_range[1], PS2_DEFAULT_FLASHLIGHT_FAR_MM);

    // Exact linear & hyperbolic knees (`0x0017ddd0` / `UpdateFParam` `0x00126170`)
    let near = PS2_DEFAULT_FLASHLIGHT_NEAR_MM;
    let far = PS2_DEFAULT_FLASHLIGHT_FAR_MM;
    let [l_fa, l_fb, h_fa, h_fb] = compute_ps2_falloff_params(near, far);
    assert!((l_fa * near + l_fb - 1.0).abs() < 1e-6);
    assert!((l_fa * far + l_fb - 0.0).abs() < 1e-6);
    assert!((h_fa / near + h_fb - 1.0).abs() < 1e-6);
    assert!((h_fa / far + h_fb - 0.0).abs() < 1e-6);
    assert!((eval_linear_falloff(3750.0, near, far) - 0.5).abs() < 1e-6);
    assert!((eval_hyperbolic_falloff(3750.0, near, far) - 0.2).abs() < 1e-6);

    // 3-parallel (`0x0017d7f0`), 3 demo point lights (`0x0017d600`), gun flash & DARK_Hosei (`0x002ab710`)
    u.set_parallel_lights(&[
        ParallelLightParams {
            direction: [0.0, 1.0, 0.0],
            color: [0.5, 0.5, 0.5],
            enabled: true,
        },
        ParallelLightParams {
            direction: [0.0, 0.0, 1.0],
            color: [0.1, 0.1, 0.1],
            enabled: true,
        },
        ParallelLightParams {
            direction: [-0.87, 0.35, -0.345523],
            color: [0.05, 0.05, 0.05],
            enabled: true,
        },
    ]);
    u.set_demo_point_lights(&[
        DemoPointLightParams {
            position: [1000.0, -500.0, 2000.0],
            color: [0.9, 0.6, 0.3],
            near_mm: 500.0,
            far_mm: 4000.0,
            enabled: true,
        },
        DemoPointLightParams::default(),
        DemoPointLightParams::default(),
    ]);
    u.set_gun_and_dark_trim(0.75, 0.3, true);
    assert_eq!(u.gun_and_dark_trim[0], 1.5); // sh2gfw_Get_GunLight() * 2.0
    assert_eq!(u.gun_and_dark_trim[1], 0.3);
    assert_eq!(u.gun_and_dark_trim[2], DARK_HOSEI);
    assert_eq!(u.gun_and_dark_trim[3], 1.0);
}

#[test]
fn golden_jms_spot_manager_ramp_flicker_and_gun_light() {
    let mut spot = JmsSpotManager::new();
    assert!(!spot.spot_on);
    assert_eq!(spot.ramp_max, 60);
    assert_eq!(spot.spot_dist_mm, 1500.0);

    // Gun muzzle flash: 1-frame trigger delay, then 0.75 -> 0.60 -> 0.45 -> 0.30 -> 0.15 -> 0.0
    spot.trigger_gun_light();
    assert_eq!(spot.gun_light(), 0.0);
    assert!((spot.jms_gun_light() - 0.075).abs() < 1e-6);
    spot.propagate_gun_light();
    assert!((spot.gun_light() - 0.75).abs() < 1e-6);
    spot.propagate_gun_light();
    assert!((spot.gun_light() - 0.60).abs() < 1e-6);
    for _ in 0..4 {
        spot.propagate_gun_light();
    }
    assert_eq!(spot.gun_light(), 0.0);

    // Turn-on ramp: +2/frame from 0 to 60 => 30 frames
    spot.turn_on();
    assert!(spot.spot_on);
    for _ in 0..30 {
        spot.step_ramp();
    }
    assert_eq!(spot.ramp_count, 60);
    assert_eq!(spot.ramp_step, 0);
    assert!((spot.spot_intensity - 1.0).abs() < 1e-6);

    // Turn-off ramp: -10/frame from 60 down to 1 => 6 frames -> TrueOff
    spot.turn_off();
    for _ in 0..6 {
        spot.step_ramp();
    }
    assert!(!spot.spot_on);
    assert_eq!(spot.ramp_count, 1);

    // Deterministic LCG flicker (`Flicker_Jms_Spot` `0x0028e7e0`)
    let mut flicker = JmsSpotManager::new();
    flicker.start_flicker(10, 3, true);
    let expected_seed = 1u32
        .wrapping_mul(SPOT_LCG_MULTIPLIER)
        .wrapping_add(SPOT_LCG_INCREMENT);
    let expected_scale = (expected_seed as f32) / 4_294_967_296.0_f32;
    flicker.step_flicker();
    assert_eq!(flicker.lcg_seed, expected_seed);
    assert!((flicker.shadow_scale_factor - expected_scale).abs() < 1e-6);
    assert!((flicker.spot_intensity - (7.0 - 3.0 * expected_scale)).abs() < 1e-5);

    // Pitch-black map IDs (`sh2gfw_Get_MaxDarkScaleFactor` `0x0028e360`)
    for &map_id in &PITCH_BLACK_MAP_IDS {
        assert_eq!(JmsSpotManager::max_dark_scale_factor(true, map_id), 0.0);
    }
    assert_eq!(
        JmsSpotManager::max_dark_scale_factor(false, PITCH_BLACK_MAP_IDS[0]),
        1.0
    );
    assert_eq!(
        JmsSpotManager::max_dark_scale_factor(true, 0x0001_0001),
        0.3
    );
}

#[test]
fn golden_character_12_slot_light_pool_selection() {
    let mut pool = CharacterLightPool::new();
    assert_eq!(CHARACTER_LIGHT_SLOT_COUNT, 12);
    assert_eq!(CHARACTER_EXTRA_LIGHT_LIMIT, 6);

    // Populate 8 point lights at varying distances + 2 reflection highlights + 2 parallel lights
    for i in 0..8 {
        let dist = 500.0 + (i as f32) * 500.0;
        pool.set_point(
            i,
            Vec3::new(dist, 0.0, 0.0),
            Vec3::splat(1.0),
            500.0,
            6000.0,
        );
    }
    pool.set_reflection(8, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.2, 0.2, 0.2));
    pool.set_reflection(9, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.8, 0.8, 0.8));
    pool.set_parallel(10, Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.5, 0.5, 0.5));
    pool.set_parallel(11, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.2, 0.2, 0.2));

    pool.update_extras(175.0, Vec3::ZERO);
    assert_eq!(pool.valid_extra_count(), CHARACTER_EXTRA_LIGHT_LIMIT);
    // Verify strictly descending influence order (`CompareByInfluence` `0x00126c00`)
    for i in 0..5 {
        let a = pool.nth_valid_extra(i).unwrap().influence;
        let b = pool.nth_valid_extra(i + 1).unwrap().influence;
        assert!(a >= b, "extra lights must be sorted descending: {a} >= {b}");
    }

    pool.update_reflections(Vec3::Z, Vec3::Y);
    assert!((pool.reflection_brightness() - 0.8).abs() < 1e-5);
    assert_eq!(pool.nhm().row(3).to_array(), PS2_NHM_UV_BIAS_ROW);
}

#[test]
fn golden_64x64_specular_sphere_map_and_gs_blend() {
    assert_eq!(SPECULAR_SPHERE_MAP_DIM, 64);
    assert_eq!(GS_ALPHA_1_SPECULAR_NIGHT, 0x58);
    assert_eq!(GS_ALPHA_1_SPECULAR_DAY, 0x54);
    assert!((environ_packet_alpha(0x80) - 1.0).abs() < 1e-6);

    let rgba = generate_specular_sphere_map_rgba();
    assert_eq!(rgba.len(), 64 * 64 * 4);
    // Center (32, 32) = 255, alpha = 0x80
    let c = (32 * 64 + 32) * 4;
    assert_eq!(
        &rgba[c..c + 4],
        &[255, 255, 255, SPECULAR_SPHERE_MAP_ALPHA_U8]
    );
    // Half-radius (48, 32): fx = 0.5 => (1 - 0.25)^4 * 255 = 80.6835 -> 80
    let m = (32 * 64 + 48) * 4;
    assert_eq!(&rgba[m..m + 4], &[80, 80, 80, SPECULAR_SPHERE_MAP_ALPHA_U8]);
    // Outside unit circle (0, 0) = 0
    assert_eq!(&rgba[0..4], &[0, 0, 0, SPECULAR_SPHERE_MAP_ALPHA_U8]);

    let uv = compute_sphere_map_uv(Vec3::Z, Vec3::Z, Vec3::Z);
    assert!((uv[0] - 0.5).abs() < 1e-5 && (uv[1] - 0.5).abs() < 1e-5);

    let night = composite_gs_specular(Vec3::splat(0.25), Vec3::splat(0.5), 0.5, true);
    assert!((night - Vec3::splat(0.5)).length() < 1e-5);
}

#[test]
fn golden_ps2_depth_screen_and_blow_fog() {
    let fog = FogUniforms::default();
    assert_eq!(std::mem::size_of::<FogUniforms>(), 192);
    assert_eq!(fog.fog_near, PS2_DEFAULT_FOG_NEAR_MM);
    assert_eq!(fog.fog_far, PS2_DEFAULT_FOG_FAR_MM);

    // `sh2gfw_Set_DepthFog` (`0x00175df0`) exact FA/z + FB bounds
    let c = DepthFogCoeffs::from_ps2_params(25_000.0, 2000.0, 0.9, 0.1);
    assert!((c.eval_hyperbolic(2000.0) - 0.1).abs() < 1e-5);
    assert!((c.eval_hyperbolic(25_000.0) - 0.9).abs() < 1e-5);
    assert!((c.eval_linear(2000.0) - 0.1).abs() < 1e-5);
    assert!((c.eval_linear(25_000.0) - 0.9).abs() < 1e-5);

    // `fog_view_screen_fog` (`0x0016c1c0`)
    let mut screen = ScreenFogState::default();
    let eval = screen.step(&ScreenFogStepInput {
        cam_delta: Vec3::new(1000.0, 0.0, 0.0),
        area_size: 2000.0,
        wind_vec: Vec3::ZERO,
        dt_scale: 1.0,
        world_view_rot: Mat3::IDENTITY,
        fog_color_a: 0x80,
        fog_alpha_param: 0x80,
    });
    assert_eq!(eval.gs_alpha_u8, 32);

    // `fog_blow.c` retail tables
    assert_eq!(BLOW_POINT_PS185.len(), 14);
    assert_eq!(BLOW_POINT_PS189.len(), 10);
    assert_eq!(BLOW_POINT_PS193.len(), 16);
    let mut blow = FogBlowSystem::new();
    blow.set_room(ROOM_BLOW_PS185);
    assert_eq!(blow.spawn_interval_frames, 20);
    blow.set_room(ROOM_BLOW_PS189);
    assert_eq!(blow.spawn_interval_frames, 30);
    blow.set_room(ROOM_BLOW_PS193);
    assert_eq!(blow.spawn_interval_frames, 60);
}

#[test]
fn retail_headless_vanilla_ps2_pipeline_smoke_test() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        // No GPU / software adapter available in CI
        return;
    };

    // Configure strict 1:1 Vanilla PS2 fog & multi-light uniforms
    let mut fog = FogUniforms::default();
    fog.apply_video_options(FogQuality::ClassicPs2, false);
    fog.set_depth_fog_bounds(2000.0, 25_000.0, 0.0, 1.0);
    ctx.update_fog(&fog);

    let mut spot = SpotlightUniforms::default();
    spot.set_ps2_falloff(1500.0, 6000.0, false);
    spot.set_gun_and_dark_trim(0.0, 0.3, true);
    ctx.update_spotlight(&spot);

    // Verify retail Restroom map (`er01.map`) loads and renders cleanly if retail assets are present
    let map_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/data/bg/er/er01.map");
    if map_path.exists() {
        let bytes = std::fs::read(&map_path).expect("retail er01.map should read cleanly");
        let map = sh2_assets::formats::map::MapFile::parse(&bytes)
            .expect("retail er01.map should parse cleanly");
        let gpu_map = sh2_renderer::GpuMap::from_map(&ctx.device, &map);
        ctx.render_gpu_map(&gpu_map);
    }
}
