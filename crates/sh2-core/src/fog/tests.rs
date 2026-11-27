use super::*;
use crate::cutscene::{
    OBSERVATION_DECK_FOG_PARTICLE_MAX, OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X, R_FINAL_BOSS_RM_ID,
};

#[test]
fn ps2_depth_fog_linear_coeffs_match_endpoints() {
    let (fa, fb) = ps2_depth_fog_linear_coeffs(2000.0, 32000.0, 0.0, 1.0);
    assert!((2000.0 * fa + fb - 0.0).abs() < 1e-5);
    assert!((32000.0 * fa + fb - 1.0).abs() < 1e-5);
}

#[test]
fn blue_creek_room_209_shifts_near_fog_with_flashlight() {
    let off = evaluate_room_fog(&RoomFogContext {
        room_id: R_APT_W_RM_208_209_ID,
        flashlight_on: false,
        ..Default::default()
    });
    let on = evaluate_room_fog(&RoomFogContext {
        room_id: R_APT_W_RM_208_209_ID,
        flashlight_on: true,
        ..Default::default()
    });
    assert_eq!(off.fog_near_mm, BLUE_CREEK_ORIGINAL_FOG_NEAR_MM);
    assert_eq!(on.fog_near_mm, BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM);
    assert!(!off.outdoor_layers_enabled);
    assert_eq!(off.particle_count, 0);
}

#[test]
fn forest_trail_reduces_james_fog_influence_in_elevation_band() {
    let outside_band = evaluate_room_fog(&RoomFogContext {
        room_id: R_FOREST_CEMETERY_ID,
        james_pos_y_mm: 500.0,
        ..Default::default()
    });
    let inside_band = evaluate_room_fog(&RoomFogContext {
        room_id: R_FOREST_CEMETERY_ID,
        james_pos_y_mm: 1350.0,
        ..Default::default()
    });
    assert_eq!(outside_band.speed_mult, FOG_SPEED_OUTDOOR_SLOW);
    assert_eq!(
        outside_band.james_fog_influence_mm,
        PS2_DEFAULT_JAMES_FOG_INFLUENCE_MM
    );
    assert_eq!(
        inside_band.james_fog_influence_mm,
        PS2_FOREST_TRAIL_JAMES_FOG_INFLUENCE_MM
    );
    // Exact `CA_DrawEnvData[1]` (`0x00361d00`) + `SetupDrawEnvData` Daytime defaults (`0x00251370:344-362`)
    assert_eq!(
        outside_band.fog_color,
        [
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            1.0,
        ]
    );
    assert_eq!(outside_band.fog_near_mm, PS2_DAYTIME_DEFAULT_FOG_NEAR_MM);
    assert_eq!(outside_band.fog_far_mm, PS2_DAYTIME_DEFAULT_FOG_FAR_MM);
    assert!((outside_band.fog_min - PS2_DAYTIME_DEFAULT_FOG_MIN_NORM).abs() < 1e-6);
    assert!((outside_band.fog_max - PS2_DAYTIME_DEFAULT_FOG_MAX_NORM).abs() < 1e-6);
    assert_eq!(outside_band.curve, FogFalloffCurve::Linear);
    assert_eq!(outside_band.particle_count, CA_FOG_ENV_PARTICLE_COUNT);
    assert_eq!(outside_band.particle_size_mm, CA_FOG_ENV_PARTICLE_SIZE_MM);
    assert_eq!(outside_band.particle_subdiv, CA_FOG_ENV_SUBDIV);
    assert_eq!(outside_band.particle_alpha_u8, CA_FOG_ENV_ALPHA_U8);
    assert!(outside_band.particle_floor_follow);
    assert_eq!(outside_band.particle_color_rgb, PS2_FOG_PARTICLE_MIST_RGB);
    assert_eq!(room_id_from_stage_code("wp01"), R_FOREST_CEMETERY_ID);
    assert_eq!(room_id_from_stage_code("ca11"), R_FOREST_CEMETERY_ID);
    assert_eq!(room_id_from_stage_code("ca14"), R_FOREST_CEMETERY_ID);
}

#[test]
fn forest_trail_ca11_particle_y_sign_and_alpha_match_cagb_fcl_and_fun_004896e0() {
    let floor_y = 0.0;
    let area = PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM; // 2000.0 mm
    let size = CA_FOG_ENV_PARTICLE_SIZE_MM; // 400.0 mm
    let alpha_scale = f32::from(CA_FOG_ENV_ALPHA_U8) / 255.0; // 16 / 255

    // At floor (`local_y_sym = -100` -> `norm_y = 0.0` -> `world_y = floor_y`):
    // `height_above_floor = 0.0` -> culled (`alpha = 0.0`) per `FUN_004896e0` (`sh2pc_00480000.c:5678`).
    let (wy_floor, a_floor) = evaluate_ps2_particle_y_and_alpha(
        floor_y,
        -100.0,
        1.0,
        area,
        CA_FOG_ENV_SUBDIV,
        CA_FOG_ENV_ALPHA_U8,
        Some(floor_y),
        size,
    );
    assert!((wy_floor - floor_y).abs() < 1e-4);
    assert!(a_floor.abs() < 1e-6);

    // 200 mm above floor in PS2 Y-down (`norm_y = 0.1` -> `local_y_sym = -80` -> `world_y = -200.0`):
    // Halfway through `400 mm` floor fade (`200 / 400 = 0.5`) * `16 / 255`.
    let (wy_low, a_low) = evaluate_ps2_particle_y_and_alpha(
        floor_y,
        -80.0,
        1.0,
        area,
        CA_FOG_ENV_SUBDIV,
        CA_FOG_ENV_ALPHA_U8,
        Some(floor_y),
        size,
    );
    assert!((wy_low - (-200.0)).abs() < 1e-4);
    assert!((a_low - 0.5 * alpha_scale).abs() < 1e-6);

    // Mid-volume (`local_y_sym = 0.0` -> `norm_y = 0.5` -> `world_y = -1000.0` = 1 m above trail):
    // Above floor fade (`1000 > 400`) and below upper fade (`-1000 > -1700`) -> full `16 / 255`.
    let (wy_mid, a_mid) = evaluate_ps2_particle_y_and_alpha(
        floor_y,
        0.0,
        1.0,
        area,
        CA_FOG_ENV_SUBDIV,
        CA_FOG_ENV_ALPHA_U8,
        Some(floor_y),
        size,
    );
    assert!((wy_mid - (-1000.0)).abs() < 1e-4);
    assert!((a_mid - alpha_scale).abs() < 1e-6);

    // Top of `subdiv = 1` band (`local_y_sym = +100` -> `world_y = -2000.0` = 2 m above trail):
    // Faded to `0.0` by `300 mm` upper border fade (`FUN_00486d10` `sh2pc_00480000.c:4237-4239`).
    let (wy_top, a_top) = evaluate_ps2_particle_y_and_alpha(
        floor_y,
        100.0,
        1.0,
        area,
        CA_FOG_ENV_SUBDIV,
        CA_FOG_ENV_ALPHA_U8,
        Some(floor_y),
        size,
    );
    assert!((wy_top - (-2000.0)).abs() < 1e-4);
    assert!(a_top.abs() < 1e-6);

    // Particle below floor (`anchor_y > floor_y`, e.g. `world_y = +100.0`) must be culled (`0.0`).
    let (_wy_below, a_below) = evaluate_ps2_particle_y_and_alpha(
        100.0,
        -100.0,
        1.0,
        area,
        CA_FOG_ENV_SUBDIV,
        CA_FOG_ENV_ALPHA_U8,
        Some(floor_y),
        size,
    );
    assert!(a_below.abs() < 1e-6);
}

#[test]
fn town_east_and_west_match_cbgb_and_ccgb_fcl_env0_and_yfix() {
    for (stage_code, room_id) in [("cb54", R_TOWN_EAST_ID), ("cc01", R_TOWN_WEST_ID)] {
        assert_eq!(room_id_from_stage_code(stage_code), room_id);
        let eval = evaluate_room_fog(&RoomFogContext {
            room_id,
            ..Default::default()
        });
        assert_eq!(eval.particle_count, CB_CC_FOG_ENV_PARTICLE_COUNT);
        assert_eq!(eval.particle_fixed_y_mm, Some(CB_CC_FOG_ENV_FIXED_Y_MM));
        assert_eq!(eval.particle_size_mm, PS2_DEFAULT_FOG_PARTICLE_SIZE_MM);
        assert_eq!(eval.particle_subdiv, PS2_DEFAULT_FOG_ENV_SUBDIV);
        assert_eq!(eval.particle_alpha_u8, PS2_DEFAULT_FOG_ENV_ALPHA_U8);
        assert!(!eval.particle_floor_follow);
        assert_eq!(eval.particle_color_rgb, PS2_FOG_PARTICLE_MIST_RGB);

        // Verify `yfix = -100.0` mm (`cbGB.fcl` @ `0x29ec`, `ccGB.fcl` @ `0x298c`):
        // mid-volume (`local_y_sym = 0.0`) -> `world_y = -1100.0` mm, full `16/255` alpha;
        // at `yfix` (`local_y_sym = -100.0`) -> `world_y = -100.0` mm, culled (`0.0`).
        let (wy_mid, a_mid) = evaluate_ps2_particle_y_and_alpha(
            CB_CC_FOG_ENV_FIXED_Y_MM,
            0.0,
            1.0,
            eval.particle_range_z_mm,
            eval.particle_subdiv,
            eval.particle_alpha_u8,
            eval.particle_fixed_y_mm,
            eval.particle_size_mm,
        );
        assert!((wy_mid - (-1100.0)).abs() < 1e-4);
        assert!((a_mid - (f32::from(PS2_DEFAULT_FOG_ENV_ALPHA_U8) / 255.0)).abs() < 1e-6);

        let (wy_bot, a_bot) = evaluate_ps2_particle_y_and_alpha(
            CB_CC_FOG_ENV_FIXED_Y_MM,
            -100.0,
            1.0,
            eval.particle_range_z_mm,
            eval.particle_subdiv,
            eval.particle_alpha_u8,
            eval.particle_fixed_y_mm,
            eval.particle_size_mm,
        );
        assert!((wy_bot - CB_CC_FOG_ENV_FIXED_Y_MM).abs() < 1e-4);
        assert!(a_bot.abs() < 1e-6);
    }
}

#[test]
fn restroom_and_default_indoor_rooms_match_setup_draw_env_data() {
    let er = evaluate_room_fog(&RoomFogContext {
        room_id: R_RESTROOM_START_ID,
        ..Default::default()
    });
    assert_eq!(er.density, 0.0);
    assert!(!er.outdoor_layers_enabled);
    assert_eq!(er.fog_near_mm, PS2_DAYTIME_DEFAULT_FOG_NEAR_MM);
    assert_eq!(er.fog_far_mm, PS2_DAYTIME_DEFAULT_FOG_FAR_MM);
    assert_eq!(
        er.fog_color,
        [
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            1.0,
        ]
    );

    let indoor_night = evaluate_room_fog(&RoomFogContext {
        room_id: 0,
        ..Default::default()
    });
    assert_eq!(indoor_night.fog_color, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(indoor_night.fog_near_mm, PS2_NIGHTTIME_DEFAULT_FOG_NEAR_MM);
    assert_eq!(indoor_night.fog_far_mm, PS2_NIGHTTIME_DEFAULT_FOG_FAR_MM);
    assert!((indoor_night.fog_min - PS2_NIGHTTIME_DEFAULT_FOG_MIN_NORM).abs() < 1e-6);
    assert!((indoor_night.fog_max - PS2_NIGHTTIME_DEFAULT_FOG_MAX_NORM).abs() < 1e-6);
    assert!(!indoor_night.outdoor_layers_enabled);
}

#[test]
fn final_boss_arena_switches_fog_above_camera_y_threshold_and_gates_sh2ee_override() {
    let lower_sh2ee = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FINAL_BOSS_RM_ID,
            camera_pos_y_mm: -12000.0,
            ..Default::default()
        },
        true,
    );
    let upper_sh2ee = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FINAL_BOSS_RM_ID,
            camera_pos_y_mm: -16000.0,
            ..Default::default()
        },
        true,
    );
    let upper_vanilla = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FINAL_BOSS_RM_ID,
            camera_pos_y_mm: -16000.0,
            ..Default::default()
        },
        false,
    );

    assert!(!lower_sh2ee.final_boss_upper_override);
    assert_eq!(lower_sh2ee.density, 1.0);
    assert!(upper_sh2ee.final_boss_upper_override);
    assert_eq!(upper_sh2ee.density, 0.0);
    assert_eq!(upper_sh2ee.speed_mult, FOG_SPEED_OUTDOOR_SLOW);
    assert!(upper_sh2ee.fog_far_mm > lower_sh2ee.fog_far_mm);

    // Strict 1:1 Vanilla PS2 mode preserves `RU_DrawEnvData[10]` (`12000..18000`) with `density = 1.0`.
    assert!(!upper_vanilla.final_boss_upper_override);
    assert_eq!(upper_vanilla.density, 1.0);
    assert_eq!(upper_vanilla.fog_near_mm, RU73_FOG_NEAR_MM);
    assert_eq!(upper_vanilla.fog_far_mm, RU73_FOG_FAR_MM);
    assert_eq!(upper_vanilla.speed_mult, 1.0);
}

#[test]
fn observation_deck_particle_y_and_alpha_match_fun_00486d10() {
    // Bottom of `subdiv = 4` band (`local_y_sym = -100` -> `norm_y = 0.0` -> `world_y = 1000.0`, full `0x30/128` alpha)
    let (wy_bot, a_bot) = evaluate_ob01_particle_y_and_alpha(OB_FOG_ANCHOR_Y_MM, -100.0, 1.0);
    assert!((wy_bot - 1000.0).abs() < 1e-4);
    assert!((a_bot - OB_FOG_ALPHA_NORM).abs() < 1e-6);

    // Top of `subdiv = 4` band (`local_y_sym = +100` -> `norm_y = 1.0` -> `world_y = 125.0`, faded to `0.0`)
    let (wy_top, a_top) = evaluate_ob01_particle_y_and_alpha(OB_FOG_ANCHOR_Y_MM, 100.0, 1.0);
    assert!((wy_top - 125.0).abs() < 1e-4);
    assert!(a_top.abs() < 1e-6);
}

#[test]
fn observation_deck_uses_extended_particle_range() {
    let eval = evaluate_room_fog(&RoomFogContext {
        room_id: R_OBSV_DECK_ID,
        extended_observation_deck_range: true,
        ..Default::default()
    });
    assert_eq!(
        eval.particle_range_x_mm,
        OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X
    );
    assert_eq!(eval.particle_count, OBSERVATION_DECK_FOG_PARTICLE_MAX);
    assert_eq!(eval.particle_fixed_y_mm, Some(OB_FOG_ENV_FIXED_Y_MM));
    assert!((eval.fog_min - OB_FOG_MIN_NORM).abs() < 1e-6);
    assert!((eval.fog_max - OB_FOG_MAX_NORM).abs() < 1e-6);
    assert_eq!(
        eval.fog_color,
        [
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
            1.0,
        ]
    );
    assert!(eval.outdoor_layers_enabled);
}

#[test]
fn eddie_meat_locker_stage_codes_map_to_fog_blow_rooms() {
    assert_eq!(room_id_from_stage_code("ps185"), R_EDI_BOSS_CORRIDOR_ID);
    assert_eq!(room_id_from_stage_code("PS185"), 0x8E);
    assert_eq!(room_id_from_stage_code("ps189"), R_EDI_BOSS_RM_1_ID);
    assert_eq!(room_id_from_stage_code("ed01"), R_EDI_BOSS_RM_1_ID);
    assert_eq!(room_id_from_stage_code("ps193"), R_EDI_BOSS_RM_2_ID);
    assert_eq!(room_id_from_stage_code("ed02"), R_EDI_BOSS_RM_2_ID);
}

#[test]
fn sh2ee_draw_distance_and_baldwin_mansion_fog_speed_respect_toggle() {
    // DrawDistance.cpp: vanilla = 1.0x (7500mm), SH2:EE = 11000mm * scale
    assert_eq!(
        dynamic_draw_distance_scale(R_FOREST_CEMETERY_ID, false),
        1.0
    );
    assert_eq!(
        dynamic_draw_distance_scale(R_FOREST_CEMETERY_ID, true),
        44.0
    );
    assert_eq!(dynamic_draw_distance_scale(R_EDI_BOSS_RM_2_ID, true), 2.0);
    assert_eq!(
        dynamic_draw_distance_scale(R_MAN_BLUE_CREEK_ENTRANCE_ID, true),
        2.0
    );
    assert_eq!(room_draw_distance_mm(R_FOREST_CEMETERY_ID, false), 7500.0);
    assert_eq!(
        room_draw_distance_mm(R_MAN_BLUE_CREEK_ENTRANCE_ID, true),
        22_000.0
    );

    // FogAdjustments.cpp: Baldwin Mansion FogSpeed = 0.25f and BlueCreekNewFog = 4200.0f
    let mansion = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_MAN_BLUE_CREEK_ENTRANCE_ID,
            ..Default::default()
        },
        true,
    );
    assert_eq!(mansion.speed_mult, FOG_SPEED_OUTDOOR_SLOW);

    let bc_vanilla = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_APT_W_RM_208_209_ID,
            flashlight_on: true,
            ..Default::default()
        },
        false,
    );
    assert_eq!(bc_vanilla.fog_near_mm, BLUE_CREEK_ORIGINAL_FOG_NEAR_MM);
}

#[test]
fn screen_fog_layer_params_distinguish_classic_ps2_and_enhanced() {
    let classic_params = Ps2FogLayerParams::classic_ps2();
    assert_eq!(classic_params.layer1_x1, 0.125);
    assert_eq!(classic_params.layer1_x2, 0.125);
    assert_eq!(classic_params.layer1_y1, 0.125);
    assert_eq!(classic_params.layer1_y2, 0.125);
    assert_eq!(Ps2FogLayerParams::default(), classic_params);

    let enhanced_params = Ps2FogLayerParams::sh2ee_enhanced();
    assert_eq!(enhanced_params.layer1_x1, 0.250);
    assert_eq!(enhanced_params.layer1_x2, 0.250);
    assert_eq!(enhanced_params.layer1_y1, 0.125);
    assert_eq!(enhanced_params.layer1_y2, 0.125);

    let ctx = RoomFogContext {
        room_id: R_OBSV_DECK_ID,
        ..Default::default()
    };
    let vanilla = evaluate_room_fog_with_fixes(&ctx, false);
    let enhanced = evaluate_room_fog_with_fixes(&ctx, true);
    assert_eq!(vanilla.layers, classic_params);
    assert_eq!(enhanced.layers, enhanced_params);
}
