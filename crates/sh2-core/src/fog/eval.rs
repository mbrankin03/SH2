//! Room-specific fog evaluation, stage-code lookup, and draw-distance scaling (`SH2-32`, `SH2-153`).

use super::constants::*;
use super::types::{FogFalloffCurve, Ps2FogLayerParams, RoomFogContext, RoomFogEvaluation};
use crate::cutscene::{
    OBSERVATION_DECK_FOG_PARTICLE_MAX, OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X, R_FINAL_BOSS_RM_ID,
    R_HTL_RM_312_ID,
};

/// Maps a lowercase stage/map code (e.g. `"ob01"`, `"ca11"`, `"wp01"`, `"ru73"`) to its canonical PS2/PC `room_id`.
#[must_use]
pub fn room_id_from_stage_code(stage_code: &str) -> u32 {
    let lower = stage_code.to_ascii_lowercase();
    match lower.as_str() {
        "er01" => R_RESTROOM_START_ID,
        "ob01" => R_OBSV_DECK_ID,
        "wp01" => R_FOREST_CEMETERY_ID,
        s if s.starts_with("ca") => R_FOREST_CEMETERY_ID,
        "tw01" | "tw02" => R_TOWN_EAST_ID,
        s if s.starts_with("cb") => R_TOWN_EAST_ID,
        "tw11" | "tw12" => R_TOWN_WEST_ID,
        s if s.starts_with("cc") => R_TOWN_WEST_ID,
        "ap07" => R_APT_E_COURTYARD_ID,
        "ap18" => crate::world::ROOM_APT_E_RM_208 as u32,
        "ap28" | "bc209" => R_APT_W_RM_208_209_ID,
        "ps137" => crate::world::R_BUG_RM_ID,
        "ps185" => R_EDI_BOSS_CORRIDOR_ID,
        "ed01" | "ps189" => R_EDI_BOSS_RM_1_ID,
        "ed02" | "ps193" => R_EDI_BOSS_RM_2_ID,
        "rr91" | "hp99" => R_HTL_RM_312_ID,
        "ru69" => crate::endings::ROOM_HOTEL_CORRIDOR_MONOLOGUE as u32,
        "ru73" => R_FINAL_BOSS_RM_ID,
        _ => 0,
    }
}

/// Evaluates 1:1 PS2-accurate room fog parameters for the given [`RoomFogContext`].
///
/// Covers Toluca Lake Observation Deck (`R_OBSV_DECK`), Forest Trail (`R_FOREST_CEMETERY`),
/// Town East/West (`R_TOWN_EAST`, `R_TOWN_WEST`), Wood Side Courtyard (`R_APT_E_COURTYARD`),
/// Blue Creek Room 209 flashlight toggle (`R_APT_W_RM_208_209`), Eddie Boss Arenas,
/// Baldwin Mansion rooms, Hotel Room 312, and Final Boss Arena (`R_FINAL_BOSS_RM`).
#[must_use]
pub fn evaluate_room_fog(ctx: &RoomFogContext) -> RoomFogEvaluation {
    evaluate_room_fog_with_fixes(ctx, true)
}

/// Evaluates room fog parameters with explicit `enable_sh2ee_world_fixes` gating for
/// `Patches/FogAdjustments.cpp` (`BlueCreekNewFog` `4200.0` vs `3000.0`, `FinalAreaBoss2ASM`
/// upper-scaffolding fog disable, and `RunFogSpeed` across Forest/Town/Baldwin Mansion/Eddie arenas).
#[must_use]
pub fn evaluate_room_fog_with_fixes(
    ctx: &RoomFogContext,
    enable_sh2ee_world_fixes: bool,
) -> RoomFogEvaluation {
    let base_layers = if enable_sh2ee_world_fixes {
        Ps2FogLayerParams::sh2ee_enhanced()
    } else {
        Ps2FogLayerParams::classic_ps2()
    };

    let speed_mult = if enable_sh2ee_world_fixes {
        match ctx.room_id {
            R_FOREST_CEMETERY_ID
            | R_TOWN_EAST_ID
            | R_TOWN_WEST_ID
            | R_EDI_BOSS_CORRIDOR_ID
            | R_HTL_ALT_READING_RM_ID
            | R_FINAL_BOSS_RM_ID
            | R_END_DOG_RM_ID
            | R_MAN_GRAND_ENTRANCE_ID
            | R_MAN_LOUNGE_2F_ID
            | R_MAN_LONG_HALLWAY_ID
            | R_MAN_SERV_RM_ID
            | R_MAN_OUTSIDE_ENTRANCE_ID
            | R_MAN_BLUE_CREEK_ENTRANCE_ID => FOG_SPEED_OUTDOOR_SLOW,
            R_APT_E_COURTYARD_ID | R_EDI_BOSS_RM_1_ID | R_EDI_BOSS_RM_2_ID => {
                FOG_SPEED_COURTYARD_MEDIUM
            }
            _ => 1.0,
        }
    } else {
        1.0
    };

    let james_fog_influence_mm = if ctx.room_id == R_FOREST_CEMETERY_ID
        && ctx.james_pos_y_mm >= FOREST_TRAIL_MIN_Y_MM
        && ctx.james_pos_y_mm <= FOREST_TRAIL_MAX_Y_MM
    {
        PS2_FOREST_TRAIL_JAMES_FOG_INFLUENCE_MM
    } else {
        PS2_DEFAULT_JAMES_FOG_INFLUENCE_MM
    };

    match ctx.room_id {
        R_OBSV_DECK_ID => {
            let (range_x, range_z, count) = if ctx.extended_observation_deck_range {
                (
                    OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X,
                    OB_FOG_ENV_AREA_SIZE_MM,
                    OBSERVATION_DECK_FOG_PARTICLE_MAX,
                )
            } else {
                (
                    OB_FOG_ENV_AREA_SIZE_MM,
                    OB_FOG_ENV_AREA_SIZE_MM,
                    OB_FOG_ENV_PARTICLE_COUNT,
                )
            };
            // Exact PS2 `OB_DrawEnvData` (`SLUS_202.28` `0x00362e70`, `sh2pc.exe` `0x3f5530`):
            // `fog_near = 19000.0` mm, `fog_far = 33000.0` mm, `fog_max = 40.0` (`215/255`),
            // `fog_min = 230.0` (`25/255`), linear/hyperbolic depth-fog ramp
            // (`sh2gfw_Set_DepthFog` `0x00175df0`), and daytime GS `FOGCOL = (0x46, 0x46, 0x46)`
            // (`SetupDrawEnvData` `0x00251804..0x00251838` when `+0x9c == 0.0f`).
            RoomFogEvaluation {
                fog_color: [
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    1.0,
                ],
                fog_near_mm: 19_000.0,
                fog_far_mm: 33_000.0,
                fog_min: OB_FOG_MIN_NORM,
                fog_max: OB_FOG_MAX_NORM,
                density: 1.0,
                curve: FogFalloffCurve::Linear,
                // `FogAdjustments.cpp:272-306`: `R_OBSV_DECK` is not in the `0.25f` FogSpeedFix list (`1.0`).
                speed_mult,
                layers: base_layers,
                particle_count: count,
                particle_range_x_mm: range_x,
                particle_range_z_mm: range_z,
                particle_fixed_y_mm: Some(OB_FOG_ENV_FIXED_Y_MM),
                james_fog_influence_mm,
                outdoor_layers_enabled: true,
                final_boss_upper_override: false,
            }
        }
        R_FOREST_CEMETERY_ID | R_TOWN_EAST_ID | R_TOWN_WEST_ID => {
            // Exact PS2/PC `SetupDrawEnvData` Daytime defaults (`SLUS_202.28` `0x00251370:344-362`,
            // `sh2pc.exe` `sh2pc_00470000.c:9133-9154`) for `CA_DrawEnvData[1]` (`0x00361d00` /
            // `sh2pc.exe` `0x3f43c0`, Forest Trail `ca11`), `CB_DrawEnvData[0]` (`0x00361ef0`), and
            // `CC_DrawEnvData[3]` (`0x003628a0`):
            // `FOGCOL = (0x46, 0x46, 0x46)`, `fog_near = 3000.0`, `fog_far = 5500.0`,
            // `fog_min = (255 - 235) / 255`, `fog_max = (255 - 20) / 255`, hyperbolic `FA/z + FB` (`curve = 0`).
            // Forest Trail particle count (`400`) matches retail `data/bg/ca/caGB.fcl` `FogEnv[0]` (`0x1100`).
            let particle_count = if ctx.room_id == R_FOREST_CEMETERY_ID {
                CA_FOG_ENV_PARTICLE_COUNT
            } else {
                PS2_DEFAULT_FOG_PARTICLE_COUNT
            };
            RoomFogEvaluation {
                fog_color: [
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    1.0,
                ],
                fog_near_mm: PS2_DAYTIME_DEFAULT_FOG_NEAR_MM,
                fog_far_mm: PS2_DAYTIME_DEFAULT_FOG_FAR_MM,
                fog_min: PS2_DAYTIME_DEFAULT_FOG_MIN_NORM,
                fog_max: PS2_DAYTIME_DEFAULT_FOG_MAX_NORM,
                density: 1.0,
                curve: FogFalloffCurve::Linear,
                speed_mult,
                layers: base_layers,
                particle_count,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: true,
                final_boss_upper_override: false,
            }
        }
        R_APT_E_COURTYARD_ID => {
            // Exact `AP_DrawEnvData` (`SLUS_202.28` `0x003641d0`: `dark = 1`, `fog_far = 0.0`, `flag9c = 0.0`)
            // Nighttime `SetupDrawEnvData` defaults (`0x00251370:373-390`).
            RoomFogEvaluation {
                fog_color: [0.0, 0.0, 0.0, 1.0],
                fog_near_mm: PS2_NIGHTTIME_DEFAULT_FOG_NEAR_MM,
                fog_far_mm: PS2_NIGHTTIME_DEFAULT_FOG_FAR_MM,
                fog_min: PS2_NIGHTTIME_DEFAULT_FOG_MIN_NORM,
                fog_max: PS2_NIGHTTIME_DEFAULT_FOG_MAX_NORM,
                density: 1.0,
                curve: FogFalloffCurve::Linear,
                speed_mult,
                layers: base_layers,
                particle_count: PS2_DEFAULT_FOG_PARTICLE_COUNT,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: true,
                final_boss_upper_override: false,
            }
        }
        R_APT_W_RM_208_209_ID => {
            // `AP_DrawEnvData` Nighttime defaults (`0x00251370:373-390`) + SH2:EE `FogAdjustments.cpp:38-39`
            // flashlight near-plane shift (`3000.0` -> `4200.0`).
            let fog_near_mm = if enable_sh2ee_world_fixes && ctx.flashlight_on {
                BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM
            } else {
                BLUE_CREEK_ORIGINAL_FOG_NEAR_MM
            };
            RoomFogEvaluation {
                fog_color: [0.0, 0.0, 0.0, 1.0],
                fog_near_mm,
                fog_far_mm: PS2_NIGHTTIME_DEFAULT_FOG_FAR_MM,
                fog_min: PS2_NIGHTTIME_DEFAULT_FOG_MIN_NORM,
                fog_max: PS2_NIGHTTIME_DEFAULT_FOG_MAX_NORM,
                density: 1.0,
                curve: FogFalloffCurve::Linear,
                speed_mult,
                layers: base_layers,
                particle_count: 0,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: false,
                final_boss_upper_override: false,
            }
        }
        R_FINAL_BOSS_RM_ID => {
            // `RU_DrawEnvData[10]` (`0x0036d130`, `room_no = 73`, `12000..18000`) for upper scaffolding
            // (`camera_pos_y < -15750.0`, `FogAdjustments.cpp:40,76-125`) vs `RU_DrawEnvData[0]` (`0x0036bdd0`,
            // `8000..12000`) for lower arena.
            // SH2:EE `FinalAreaBoss2ASM` (`FogAdjustments.cpp:100-125`) disables depth fog (`D3DRS_FOGENABLE = 0`,
            // `density = 0.0`) on upper scaffolding; strict 1:1 Vanilla PS2 retains `density = 1.0`.
            let upper = ctx.camera_pos_y_mm < FINAL_BOSS_CAMERA_Y_THRESHOLD_MM;
            let sh2ee_upper_override = upper && enable_sh2ee_world_fixes;
            RoomFogEvaluation {
                fog_color: if upper {
                    [
                        PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                        PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                        PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                        1.0,
                    ]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                },
                fog_near_mm: if upper {
                    RU73_FOG_NEAR_MM
                } else {
                    RU_LOWER_FOG_NEAR_MM
                },
                fog_far_mm: if upper {
                    RU73_FOG_FAR_MM
                } else {
                    RU_LOWER_FOG_FAR_MM
                },
                fog_min: if upper {
                    RU73_FOG_MIN_NORM
                } else {
                    RU_LOWER_FOG_MIN_NORM
                },
                fog_max: if upper {
                    RU73_FOG_MAX_NORM
                } else {
                    RU_LOWER_FOG_MAX_NORM
                },
                density: if sh2ee_upper_override { 0.0 } else { 1.0 },
                curve: FogFalloffCurve::Linear,
                speed_mult,
                layers: base_layers,
                particle_count: PS2_DEFAULT_FOG_PARTICLE_COUNT,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: true,
                final_boss_upper_override: sh2ee_upper_override,
            }
        }
        R_RESTROOM_START_ID | R_HTL_RM_312_ID => {
            // `ER_DrawEnvData[2]` (`SLUS_202.28` `0x00363440`, `room_no = 1`, `dark = 0`, `fog_far = 0.0`)
            // Daytime `SetupDrawEnvData` defaults with indoor non-fogged `density = 0.0`.
            RoomFogEvaluation {
                fog_color: [
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    PS2_DAYTIME_DEFAULT_FOG_COLOR_F32,
                    1.0,
                ],
                fog_near_mm: PS2_DAYTIME_DEFAULT_FOG_NEAR_MM,
                fog_far_mm: PS2_DAYTIME_DEFAULT_FOG_FAR_MM,
                fog_min: PS2_DAYTIME_DEFAULT_FOG_MIN_NORM,
                fog_max: PS2_DAYTIME_DEFAULT_FOG_MAX_NORM,
                density: 0.0,
                curve: FogFalloffCurve::Linear,
                speed_mult: 1.0,
                layers: base_layers,
                particle_count: 0,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: false,
                final_boss_upper_override: false,
            }
        }
        _ => {
            // Default indoor nighttime rooms (`RR_DrawEnvData`, `AP_DrawEnvData`, `HP_DrawEnvData`,
            // `PS_DrawEnvData`): exact `SetupDrawEnvData` Nighttime defaults (`SLUS_202.28` `0x00251370:373-390`).
            RoomFogEvaluation {
                fog_color: [0.0, 0.0, 0.0, 1.0],
                fog_near_mm: PS2_NIGHTTIME_DEFAULT_FOG_NEAR_MM,
                fog_far_mm: PS2_NIGHTTIME_DEFAULT_FOG_FAR_MM,
                fog_min: PS2_NIGHTTIME_DEFAULT_FOG_MIN_NORM,
                fog_max: PS2_NIGHTTIME_DEFAULT_FOG_MAX_NORM,
                density: 1.0,
                curve: FogFalloffCurve::Linear,
                speed_mult,
                layers: base_layers,
                particle_count: 0,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: false,
                final_boss_upper_override: false,
            }
        }
    }
}

/// Evaluates `RunDynamicDrawDistance` (`Patches/DrawDistance.cpp:221-244`, `sh2pc.exe` `0x0047D824`):
///
/// - `enable_sh2ee_world_fixes == false`: `1.0` (1:1 vanilla PS2/PC).
/// - `R_FOREST_CEMETERY` (`0x03`): `44.0`
/// - `R_EDI_BOSS_RM_2` (`0x90`) or `R_MAN_BLUE_CREEK_ENTRANCE` (`0xD4`): `2.0`
/// - Other rooms: `1.0`
#[must_use]
pub const fn dynamic_draw_distance_scale(room_id: u32, enable_sh2ee_world_fixes: bool) -> f32 {
    if !enable_sh2ee_world_fixes {
        return 1.0;
    }
    match room_id {
        R_FOREST_CEMETERY_ID => 44.0,
        R_EDI_BOSS_RM_2_ID | R_MAN_BLUE_CREEK_ENTRANCE_ID => 2.0,
        _ => 1.0,
    }
}

/// Computes the effective far-clip / draw distance in PS2 mm for `room_id`, combining
/// `PatchDrawDistance` (`7500.0` mm vanilla `0x0047c199` vs `11000.0` mm SH2:EE `DrawDistance.cpp:90`)
/// with [`dynamic_draw_distance_scale`].
#[must_use]
pub fn room_draw_distance_mm(room_id: u32, enable_sh2ee_world_fixes: bool) -> f32 {
    let base = if enable_sh2ee_world_fixes {
        SH2EE_EXTENDED_DRAW_DISTANCE_MM
    } else {
        DEFAULT_DRAW_DISTANCE_MM
    };
    base * dynamic_draw_distance_scale(room_id, enable_sh2ee_world_fixes)
}
