//! 1:1 PS2 multi-light world/character lighting pipeline, linear/hyperbolic flashlight
//! attenuation, `sh2_JmsSpot_Man.c` lifecycle, and 12-slot character `LightPointer` pool
//! (`SH2-135`).
//!
//! # Ground-Truth Citations (`SLUS_202.28` / `sh2pc.exe`)
//! - **World Multi-Light Blocks (`GFW/sh2gfw_LightSet.c`)**:
//!   - `sh2gfw_setparal3LGT_blk` (`0x0017d7f0`): 3-parallel daytime lighting + gun muzzle flash + `DARK_Hosei` trim.
//!   - `sh2gfw_setsp1para2LGT_blk` (`0x0017ddd0`): 1-spot + 2-parallel night/interior lighting + linear falloff.
//!   - `sh2gfw_Set_DemoPointLight` (`0x0017d600`): 3 cutscene/demo point lights.
//!   - `DARK_Hosei` (`0x002ab710` = `0.5f`).
//! - **Flashlight Lifecycle & Gun Muzzle Flash (`Chacter_Draw/sh2_JmsSpot_Man.c`)**:
//!   - `sh2gfw_ALLInit_SpotMan` (`0x0028e0b0`), `sh2gfw_Propagate_JmsGunLight` (`0x0028e1b0`),
//!     `sh2gfw_On_JmsSPOT` (`0x0028e410`), `sh2gfw_TrueOff_JmsSPOT` (`0x0028e500`),
//!     `Flicker_Jms_Spot` (`0x0028e7e0`), `Calculate_Jms_Spot` (`0x0028e940`).
//! - **12-Slot Character Light Pool (`Chacter_Draw/light_n.c`)**:
//!   - `LightPointer` (`0x001260b0`), `UpdateIntensity` (`0x00126120`), `UpdateFParam` (`0x00126170`),
//!     `UpdateSParam` (`0x001261d0`), `CalcInfluence` (`0x001269b0`), `UpdateParallels` (`0x00126d30`),
//!     `UpdateReflections` (`0x00126f90`), `UpdateExtras` (`0x00127360`).

pub mod character_pool;
pub mod cutscene;
pub mod jms_spot;
pub mod uniforms;

pub use character_pool::{
    CharacterLightEntry, CharacterLightPool, CharacterLightType, FakeLightKind,
    CHARACTER_EXTRA_LIGHT_LIMIT, CHARACTER_LIGHT_SLOT_COUNT, CHARACTER_PARALLEL_MATRIX_LIMIT,
    FAKE_LIGHT_MIN_ATTENUATION, MODEL3_LIGHT_ENV_RADIUS_MM, PS2_LUMA_WEIGHTS, PS2_NHM_UV_BIAS_COL,
    UPDATE_EXTRAS_NIGHT_REFLECTION_SCALE,
};
pub use cutscene::{
    build_cutscene_light_uniforms, CutsceneLightInput, CHARACTER_POOL_LINEAR_SCALE,
};
pub use jms_spot::{
    linear_trim, JmsSpotManager, BASE_REFLECTION_HIGHLIGHT_RGB, DARK_HOSEI,
    DARK_ROOM_MAX_SCALE_DEFAULT, DARK_ROOM_MAX_SCALE_NO_FLASHLIGHT,
    DAYTIME_REFLECTION_HIGHLIGHT_SCALE, GUN_LIGHT_AMBIENT_MULTIPLIER, GUN_LIGHT_DECAY_PER_TICK,
    GUN_LIGHT_INITIAL_INTENSITY, GUN_LIGHT_JMS_BASE_OFFSET,
    GUN_LIGHT_NORMALIZED_AMBIENT_MULTIPLIER, PITCH_BLACK_MAP_IDS, PS2_VU1_WORLD_LIGHT_SCALE,
    SPOT_LCG_INCREMENT, SPOT_LCG_MULTIPLIER, SPOT_MIN_COLOR_OFF, SPOT_OFF_THRESHOLD,
};
pub use uniforms::{
    compute_ps2_falloff_params, eval_hyperbolic_falloff, eval_linear_falloff, DemoPointLightParams,
    ParallelLightParams, SpotlightUniforms, WorldLightMode, PS2_DEFAULT_FLASHLIGHT_FAR_MM,
    PS2_DEFAULT_FLASHLIGHT_NEAR_MM, PS2_DEFAULT_FLASHLIGHT_POS_Y_MM,
};
