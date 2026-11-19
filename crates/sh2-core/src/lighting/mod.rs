//! PS2 / PC lighting, per-room `DrawEnvData` tables, SH2:EE lighting/shadow parity fixes,
//! 7-sprite lens flares, and Hyper Spray mist particle simulation (`SH2-81`).

pub mod character_light;
pub mod draw_env;
pub mod draw_env_tables;
pub mod fixes;
pub mod hyper_spray;
pub mod lens_flare;

pub use character_light::{
    evaluate_character_lighting, CharacterLightCandidate, CharacterLightKind,
    CharacterLightingResult, SelectedCharacterLight, CHAR_LIGHT_AMBIENT_WEIGHT,
    CHAR_LIGHT_DIRECT_WEIGHT, CHAR_LIGHT_FLASHLIGHT_AMBIENT_SCALE, CHAR_LIGHT_ROOM_AMBIENT_SCALE,
    MAX_CHARACTER_ACTIVE_LIGHTS,
};
pub use draw_env::{
    env_flash_off_level, lookup_draw_env, DrawEnvData, DrawEnvDirectionalLight, DrawEnvPointLight,
    ENV_FLASH_OFF_POST_PICKUP, ENV_FLASH_OFF_PRE_PICKUP, FLAG_WOODSIDE_FLASHLIGHT_PICKUP,
};
pub use draw_env_tables::{StageDrawEnvTable, ALL_STAGE_DRAW_ENV_TABLES};
pub use fixes::{
    apply_draw_env_fixes, bfaw_attic_shadow_cutoff_y, evaluate_enemy_reveal_lighting,
    flashlight_wall_brightness_multiplier, is_halogen_fullbright_room,
    sh2ee_room_shadow_opacity_u8, should_disable_cutscene_shadows,
    should_suppress_room312_balcony_shadow, specular_intensity_scale, ShadowChapter,
    ShadowFadeController, ShadowFadePhase, DEMO6_TUNNEL_REVEAL_HOLD_FRAME, ENEMY_REVEAL_MAX_LEVEL,
    ENEMY_REVEAL_MIN_LEVEL, HALOGEN_FULLBRIGHT_ROOMS, PC_ENVIRONMENT_BRIGHTNESS_MUL,
    PS2_FLASHLIGHT_RANGE_MM, SH2EE_DEFAULT_SHADOW_OPACITY_U8, SH2EE_FLASHLIGHT_BRIGHTNESS_MUL,
    SH2EE_FLASHLIGHT_RANGE_MM, SH2EE_SHADOW_FADE_IN_FRAMES_30HZ, SH2EE_SHADOW_FADE_OUT_FRAMES_30HZ,
};
pub use hyper_spray::{
    HyperSprayColorTier, HyperSprayEmitter, SprayParticle, SprayParticlePhase,
    HYPER_SPRAY_LIFETIME_30HZ, HYPER_SPRAY_SPEED_FACTOR_0_1, HYPER_SPRAY_SPEED_FACTOR_2,
    LYING_FIGURE_SPEED_FACTOR_0, LYING_FIGURE_SPEED_FACTOR_1, LYING_FIGURE_SPEED_FACTOR_2,
    MAX_SPRAY_PARTICLES, SPRAY_Y_ADJUST_60HZ,
};
pub use lens_flare::{
    evaluate_lens_flares, LensFlareQuad, LensFlareSource, LensFlareSpriteSpec, LENS_FLARE_SPRITES,
    LENS_FLARE_SPRITE_COUNT,
};
