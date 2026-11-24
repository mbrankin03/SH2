//! 1:1 PS2-accurate room fog parameters, `FogSpeedFix` timing constants, and localized
//! 3D proximity fog particle simulation (`SH2-32`).
//!
//! # Ground Truth Citations (`SLUS_202.28`, `sh2pc.exe`, & `Silent-Hill-2-Enhancements`)
//! - **Depth Fog Curve (`sh2gfw_Set_DepthFog` @ `0x00175df0`, `sh2gfw_Set_DepthFogFAFB` @ `0x00175e70`)**:
//!   Computes linear and perspective depth-fog coefficients `(fa, fb)` from `(fog_near, fog_far, fog_min, fog_max)`.
//! - **Screen-Space Multi-Layer Noise Planes (`fog_view_screen_fog` @ `0x0016c1c0`, `fog_asm_data2` @ `0x002ab470`,
//!   `sh2pc.exe` @ `0x0048734C` / `0x0048893D`, `FogFix.cpp` & `Settings.h:183-219`)**:
//!   Dual-layer animated UV-scrolling noise planes with `fog_layer1_x1/x2 = 0.250`, `fog_layer1_y1/y2 = 0.125`,
//!   `fog_layer2_complexity = 0.055`, `fog_layer2_density_add = 100.0`, `fog_layer2_density_mult = 1.4`,
//!   `fog_transparency_layer1 = 128`, `fog_transparency_layer2 = 112`.
//! - **Localized 3D Proximity Particles (`fog_defset_data` @ `0x002ab530`, `fog_asm_data1` @ `0x002ab400`,
//!   `fogInitParticle` @ `0x00169d90`, `fogMoveParticle` @ `0x0016a540`, `fog_part_clamp` @ `0x0016ba50`,
//!   `fog_part_alp` @ `0x0016b9a0`, `fog_part_obj` @ `0x0016b780`, `fogChangeWind` @ `0x00169380`,
//!   `shSway1f_asm` @ `0x0016f0a0`)**:
//!   Default `500` particles (expanded up to `2000` on Observation Deck per `ObservationDeckFogFix.cpp`),
//!   volume half-extent `2000.0` mm (`24500.0` mm X on Observation Deck), border fade `300.0` mm,
//!   and radial parting around James (`JamesFogInfluence = 200.0` mm, reduced to `10.0` mm on the Forest trail
//!   when `1125.0 <= james_y <= 1575.0` per `FogAdjustments.cpp:317-334`).
//! - **Context-Sensitive Room Switching (`FogAdjustments.cpp:34-125, 272-334`)**:
//!   Per-room `FogSpeedFix` drift multipliers (`0.25` outdoor/final boss, `0.50` courtyard/Eddie boss),
//!   Blue Creek Room 209 flashlight near-plane shift (`3000.0 -> 4200.0`), and Final Boss Arena
//!   upper-scaffolding camera elevation switch (`cam_pos_y < -15750.0`).

pub mod particles;

pub use particles::{FogParticle, FogParticleVolume};

use crate::cutscene::{
    OBSERVATION_DECK_FOG_PARTICLE_MAX, OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X, R_FINAL_BOSS_RM_ID,
    R_HTL_RM_312_ID,
};

/// Room ID for Toluca Lake Observation Deck (`R_OBSV_DECK = 0x02` in `Patches.h`).
pub const R_OBSV_DECK_ID: u32 = 0x02;
/// Exact PS2 fixed Y altitude (`+10,000.0` mm, PS2 Y-down = 10 m below the parking lot cliff)
/// for Observation Deck (`ob01`) 3D fog particles from retail `data/bg/ob/obGB.fcl`
/// (`FogEnv +0x08` at file offset `0x8c`, `subdiv (+0x12) = 4 != 0`), applied to
/// `fog_work.local_pos.y` (`0x00581484`) in `fogSetLocalPosV` (`SLUS_202.28` `0x0016c7c8..0x0016c7e4`).
pub const OB_FOG_ENV_FIXED_Y_MM: f32 = 10_000.0;
/// Exact PS2 daytime default GS `FOGCOL` RGB byte (`0x46` = `70` / `255.0` ≈ `0.2745`) written by
/// `SetupDrawEnvData` (`SLUS_202.28` `0x00251804..0x00251838`) when `sh2gfw_Get_NightOrDay() == 0`
/// and `DrawEnvData +0x9c == 0.0f` (as in `OB_DrawEnvData` `0x00362e70` / `sh2pc.exe` `0x3f5530`).
pub const PS2_DAYTIME_DEFAULT_FOG_COLOR_U8: u8 = 0x46;
/// Normalized `[0.0..=1.0]` channel value of [`PS2_DAYTIME_DEFAULT_FOG_COLOR_U8`] (`70.0 / 255.0`).
pub const PS2_DAYTIME_DEFAULT_FOG_COLOR_F32: f32 =
    (PS2_DAYTIME_DEFAULT_FOG_COLOR_U8 as f32) / 255.0;
/// Room ID for Forest Trail & Cemetery (`R_FOREST_CEMETERY = 0x03` in `Patches.h`).
pub const R_FOREST_CEMETERY_ID: u32 = 0x03;
/// Room ID for Silent Hill Town East (`R_TOWN_EAST = 0x04` in `Patches.h`).
pub const R_TOWN_EAST_ID: u32 = 0x04;
/// Room ID for Wood Side Apartments Courtyard (`R_APT_E_COURTYARD = 0x07` in `Patches.h`).
pub const R_APT_E_COURTYARD_ID: u32 = 0x07;
/// Room ID for Silent Hill Town West (`R_TOWN_WEST = 0x08` in `Patches.h`).
pub const R_TOWN_WEST_ID: u32 = 0x08;
/// Room ID for Blue Creek Apartments Rooms 208/209 (`R_APT_W_RM_208_209 = 0x28` in `Patches.h`).
pub const R_APT_W_RM_208_209_ID: u32 = 0x28;
/// Room ID for Meat Locker Corridor (`ps185`, `fogSetBlowPoint` `SLUS_202.28` `0x00274cb0` `param_1 == 0x8e`).
pub const R_EDI_BOSS_CORRIDOR_ID: u32 = 0x8E;
/// Room ID for Eddie Boss Room 1 (`R_EDI_BOSS_RM_1 = 0x8F` in `Patches.h`; `ps189` in `fogSetBlowPoint` `0x00274cb0`).
pub const R_EDI_BOSS_RM_1_ID: u32 = 0x8F;
/// Room ID for Eddie Boss Room 2 (`R_EDI_BOSS_RM_2 = 0x90` in `Patches.h`; `ps193` in `fogSetBlowPoint` `0x00274cb0`).
pub const R_EDI_BOSS_RM_2_ID: u32 = 0x90;

/// Default PS2 fog particle count (`fog_defset_data + 0x00` @ `0x002ab530` in `SLUS_202.28`).
pub const PS2_DEFAULT_FOG_PARTICLE_COUNT: usize = 500;
/// Default PS2 fog particle volume half-extent in mm (`fog_defset_data + 0x02` @ `0x002ab530`,
/// `fog_asm_data1 + 0x40` @ `0x002ab440`).
pub const PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM: f32 = 2000.0;
/// Default PS2 fog particle height band half-extent in mm (`fog_defset_data + 0x06` @ `0x002ab530`,
/// `fog_asm_data1 + 0x10` @ `0x002ab410`).
pub const PS2_DEFAULT_FOG_HEIGHT_BAND_MM: f32 = 100.0;
/// Default PS2 radial parting influence of James on nearby fog particles in mm
/// (`JamesFogInfluence` in `FogAdjustments.cpp:331`).
pub const PS2_DEFAULT_JAMES_FOG_INFLUENCE_MM: f32 = 200.0;
/// Reduced radial parting influence on the Forest trail (`1125.0 <= james_y <= 1575.0`)
/// preventing fog sprites from sticking to James (`FogAdjustments.cpp:317-323`).
pub const PS2_FOREST_TRAIL_JAMES_FOG_INFLUENCE_MM: f32 = 10.0;
/// Lower Y-elevation bound (PS2 mm) of the Forest trail sticky-fog zone (`FogAdjustments.cpp:317`).
pub const FOREST_TRAIL_MIN_Y_MM: f32 = 1125.0;
/// Upper Y-elevation bound (PS2 mm) of the Forest trail sticky-fog zone (`FogAdjustments.cpp:317`).
pub const FOREST_TRAIL_MAX_Y_MM: f32 = 1575.0;

/// Blue Creek Apt Room 209 original front fog distance in mm (`FogAdjustments.cpp:39`).
pub const BLUE_CREEK_ORIGINAL_FOG_NEAR_MM: f32 = 3000.0;
/// Blue Creek Apt Room 209 front fog distance in mm when flashlight is active (`FogAdjustments.cpp:38`).
pub const BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM: f32 = 4200.0;
/// Final Boss Arena camera Y threshold (PS2 mm) triggering upper-scaffolding fog switch (`FogAdjustments.cpp:40`).
pub const FINAL_BOSS_CAMERA_Y_THRESHOLD_MM: f32 = -15750.0;

/// `FogSpeedFix` drift speed multiplier (`0.25`) for outdoor streets, forest, and final boss (`FogAdjustments.cpp:288`).
pub const FOG_SPEED_OUTDOOR_SLOW: f32 = 0.25;
/// `FogSpeedFix` drift speed multiplier (`0.50`) for apartment courtyard and Eddie boss rooms (`FogAdjustments.cpp:299`).
pub const FOG_SPEED_COURTYARD_MEDIUM: f32 = 0.50;

/// Authentic PS2 / SH2:EE dual-layer UV-scrolling fog noise plane parameters.
///
/// Verified against `fog_view_screen_fog` (`0x0016c1c0`), `fog_asm_data2` (`0x002ab470`),
/// `sh2pc.exe` (`0x0048734C`, `0x0048893D`), `FogFix.cpp`, and `Settings.h:183-219`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ps2FogLayerParams {
    /// Layer 1 horizontal UV scale/extent X1 (`Settings.h:213`, `fog_asm_data2+0x38`).
    pub layer1_x1: f32,
    /// Layer 1 horizontal UV scale/extent X2 (`Settings.h:214`, `fog_asm_data2+0x64`).
    pub layer1_x2: f32,
    /// Layer 1 vertical UV scale/extent Y1 (`Settings.h:215`, `fog_view_screen_fog` `0x0016c330`).
    pub layer1_y1: f32,
    /// Layer 1 vertical UV scale/extent Y2 (`Settings.h:216`, `fog_asm_data2+0x54`).
    pub layer1_y2: f32,
    /// Layer 2 noise complexity / frequency (`Settings.h:217`, `FogFix.cpp:89`).
    pub layer2_complexity: f32,
    /// Layer 2 additive density bias (`Settings.h:218`, `FogFix.cpp:37`).
    pub layer2_density_add: f32,
    /// Layer 2 multiplicative density gain (`Settings.h:219`, `FogFix.cpp:36`).
    pub layer2_density_mult: f32,
    /// Layer 1 transparency in PS2 GS 0..255 alpha space (`Settings.h:183`, `fog_asm_data2+0x3c = 128.0`).
    pub transparency_layer1: u8,
    /// Layer 2 transparency in PS2 GS 0..255 alpha space (`Settings.h:184`, `fog_asm_data2+0x50 = 112.0`).
    pub transparency_layer2: u8,
}

impl Default for Ps2FogLayerParams {
    fn default() -> Self {
        Self {
            layer1_x1: 0.250,
            layer1_x2: 0.250,
            layer1_y1: 0.125,
            layer1_y2: 0.125,
            layer2_complexity: 0.055,
            layer2_density_add: 100.0,
            layer2_density_mult: 1.4,
            transparency_layer1: 128,
            transparency_layer2: 112,
        }
    }
}

/// Depth-fog falloff curve type matching PS2 `dbg_fogparm_ctl` / `dbg_ambifog_ctl` and `sh2gfw_Set_DepthFog` (`0x00175df0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum FogFalloffCurve {
    /// Linear ramp between `fog_near` and `fog_far` (PS2 GS `FOG` linear mode).
    #[default]
    Linear = 0,
    /// Exponential falloff `1 - exp(-k * t)`.
    Exponential = 1,
    /// Exponential-squared falloff `1 - exp(-k * t^2)` for dense atmospheric vistas.
    ExponentialSquared = 2,
}

impl FogFalloffCurve {
    /// Converts a raw `u32` uniform curve ID into [`FogFalloffCurve`].
    #[must_use]
    pub const fn from_u32(val: u32) -> Self {
        match val {
            1 => Self::Exponential,
            2 => Self::ExponentialSquared,
            _ => Self::Linear,
        }
    }
}

/// Evaluates the PS2 `sh2gfw_Set_DepthFog` (`0x00175df0`) linear depth-fog coefficients `(fa_linear, fb_linear)`
/// such that `fog_val = clamp(depth * fa_linear + fb_linear, fog_min, fog_max)`.
#[inline]
#[must_use]
pub fn ps2_depth_fog_linear_coeffs(
    fog_near: f32,
    fog_far: f32,
    fog_min: f32,
    fog_max: f32,
) -> (f32, f32) {
    let denom = fog_near - fog_far;
    if denom.abs() < 1e-4 {
        return (0.0, fog_max);
    }
    let fa = (fog_min - fog_max) / denom;
    let fb = fog_min - fog_near * fa;
    (fa, fb)
}

/// Room ID for Opening Restroom (`er01.map`, `RoomNameJms() == 1` in `PlayerSetReverseMode` `0x0024c190` line 198).
pub const R_RESTROOM_START_ID: u32 = 0x01;

/// Maps a lowercase stage/map code (e.g. `"ob01"`, `"ca11"`, `"ru73"`) to its canonical PS2/PC `room_id`.
#[must_use]
pub fn room_id_from_stage_code(stage_code: &str) -> u32 {
    let lower = stage_code.to_ascii_lowercase();
    match lower.as_str() {
        "er01" => R_RESTROOM_START_ID,
        "ob01" => R_OBSV_DECK_ID,
        "ca11" | "ca12" | "ca13" => R_FOREST_CEMETERY_ID,
        "tw01" | "tw02" => R_TOWN_EAST_ID,
        "tw11" | "tw12" => R_TOWN_WEST_ID,
        "ap07" => R_APT_E_COURTYARD_ID,
        "ap28" | "bc209" => R_APT_W_RM_208_209_ID,
        "ps185" => R_EDI_BOSS_CORRIDOR_ID,
        "ed01" | "ps189" => R_EDI_BOSS_RM_1_ID,
        "ed02" | "ps193" => R_EDI_BOSS_RM_2_ID,
        "rr91" | "hp99" => R_HTL_RM_312_ID,
        "ru73" => R_FINAL_BOSS_RM_ID,
        _ => 0,
    }
}

/// Runtime context for evaluating context-sensitive room fog parameters.
#[derive(Debug, Clone, Copy)]
pub struct RoomFogContext {
    /// Active stage room ID (`Patches.h` `R_*` enum).
    pub room_id: u32,
    /// Player (James) world-space Y coordinate in PS2 mm.
    pub james_pos_y_mm: f32,
    /// Active camera world-space Y coordinate in PS2 mm.
    pub camera_pos_y_mm: f32,
    /// Whether the pocket flashlight is currently switched ON.
    pub flashlight_on: bool,
    /// Whether ultrawide / Observation Deck extended particle range is enabled (`ObservationDeckFogFix.cpp`).
    pub extended_observation_deck_range: bool,
}

impl Default for RoomFogContext {
    fn default() -> Self {
        Self {
            room_id: 0,
            james_pos_y_mm: 0.0,
            camera_pos_y_mm: 0.0,
            flashlight_on: false,
            extended_observation_deck_range: true,
        }
    }
}

/// Evaluated room-specific atmospheric fog configuration derived from PS2 / SH2:EE ground truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomFogEvaluation {
    /// Fog ambient color RGBA in linear/display 0..1 space.
    pub fog_color: [f32; 4],
    /// Fog near start distance in PS2 mm.
    pub fog_near_mm: f32,
    /// Fog far saturation distance in PS2 mm.
    pub fog_far_mm: f32,
    /// Overall fog density multiplier (0.0 disables fog).
    pub density: f32,
    /// Falloff curve type (`dbg_fogparm_ctl`).
    pub curve: FogFalloffCurve,
    /// `FogSpeedFix` drift speed multiplier (`FogAdjustments.cpp:272-306`).
    pub speed_mult: f32,
    /// Dual-layer noise plane parameters (`FogFix.cpp`).
    pub layers: Ps2FogLayerParams,
    /// Number of active localized 3D proximity fog particles (`0` for indoor rooms).
    pub particle_count: usize,
    /// X half-extent of the localized proximity particle volume in PS2 mm.
    pub particle_range_x_mm: f32,
    /// Z half-extent of the localized proximity particle volume in PS2 mm.
    pub particle_range_z_mm: f32,
    /// Optional fixed world Y altitude (PS2 mm, Y-down) overriding player Y when `.fcl`
    /// `FogEnv.subdiv (+0x12) != 0` (`fogSetLocalPosV` `SLUS_202.28` `0x0016c7c8..0x0016c7e4`),
    /// e.g. `Some(10_000.0)` from `data/bg/ob/obGB.fcl` `+0x8c` placing Observation Deck mist
    /// 10 m below the parking lot cliff over Toluca Lake.
    pub particle_fixed_y_mm: Option<f32>,
    /// James radial parting influence in PS2 mm (`FogAdjustments.cpp:317-334`).
    pub james_fog_influence_mm: f32,
    /// Whether outdoor dual-layer sky/mist planes are active (`true` outdoors, `false` dark interiors).
    pub outdoor_layers_enabled: bool,
    /// In `R_FINAL_BOSS_RM`, `true` when `camera_pos_y < -15750.0` (disables dark indoor fog override).
    pub final_boss_upper_override: bool,
}

/// Evaluates 1:1 PS2-accurate room fog parameters for the given [`RoomFogContext`].
///
/// Covers Toluca Lake Observation Deck (`R_OBSV_DECK`), Forest Trail (`R_FOREST_CEMETERY`),
/// Town East/West (`R_TOWN_EAST`, `R_TOWN_WEST`), Wood Side Courtyard (`R_APT_E_COURTYARD`),
/// Blue Creek Room 209 flashlight toggle (`R_APT_W_RM_208_209`), Eddie Boss Arenas,
/// Hotel Room 312, and Final Boss Arena (`R_FINAL_BOSS_RM`).
#[must_use]
pub fn evaluate_room_fog(ctx: &RoomFogContext) -> RoomFogEvaluation {
    let base_layers = Ps2FogLayerParams::default();

    let speed_mult = match ctx.room_id {
        R_FOREST_CEMETERY_ID | R_TOWN_EAST_ID | R_TOWN_WEST_ID | R_FINAL_BOSS_RM_ID => {
            FOG_SPEED_OUTDOOR_SLOW
        }
        R_APT_E_COURTYARD_ID | R_EDI_BOSS_RM_1_ID | R_EDI_BOSS_RM_2_ID => {
            FOG_SPEED_COURTYARD_MEDIUM
        }
        _ => 1.0,
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
                    12000.0,
                    OBSERVATION_DECK_FOG_PARTICLE_MAX,
                )
            } else {
                (
                    PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                    PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                    PS2_DEFAULT_FOG_PARTICLE_COUNT,
                )
            };
            // Exact PS2 `OB_DrawEnvData` (`SLUS_202.28` `0x00362e70`, `sh2pc.exe` `0x3f5530`):
            // `fog_near = 19000.0` mm, `fog_far = 33000.0` mm, linear depth-fog ramp
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
                density: 1.0,
                curve: FogFalloffCurve::Linear,
                speed_mult: FOG_SPEED_OUTDOOR_SLOW,
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
        R_FOREST_CEMETERY_ID | R_TOWN_EAST_ID | R_TOWN_WEST_ID => RoomFogEvaluation {
            fog_color: [0.62, 0.66, 0.70, 1.0],
            fog_near_mm: 1800.0,
            fog_far_mm: 26000.0,
            density: 1.0,
            curve: FogFalloffCurve::Exponential,
            speed_mult,
            layers: base_layers,
            particle_count: PS2_DEFAULT_FOG_PARTICLE_COUNT,
            particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
            particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
            particle_fixed_y_mm: None,
            james_fog_influence_mm,
            outdoor_layers_enabled: true,
            final_boss_upper_override: false,
        },
        R_APT_E_COURTYARD_ID => RoomFogEvaluation {
            fog_color: [0.35, 0.37, 0.40, 1.0],
            fog_near_mm: 1600.0,
            fog_far_mm: 18000.0,
            density: 0.9,
            curve: FogFalloffCurve::Exponential,
            speed_mult,
            layers: base_layers,
            particle_count: PS2_DEFAULT_FOG_PARTICLE_COUNT / 2,
            particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
            particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
            particle_fixed_y_mm: None,
            james_fog_influence_mm,
            outdoor_layers_enabled: true,
            final_boss_upper_override: false,
        },
        R_APT_W_RM_208_209_ID => {
            let fog_near_mm = if ctx.flashlight_on {
                BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM
            } else {
                BLUE_CREEK_ORIGINAL_FOG_NEAR_MM
            };
            RoomFogEvaluation {
                fog_color: [0.09, 0.10, 0.12, 1.0],
                fog_near_mm,
                fog_far_mm: 11000.0,
                density: 0.9,
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
            let upper = ctx.camera_pos_y_mm < FINAL_BOSS_CAMERA_Y_THRESHOLD_MM;
            RoomFogEvaluation {
                fog_color: if upper {
                    [0.34, 0.36, 0.40, 1.0]
                } else {
                    [0.24, 0.26, 0.30, 1.0]
                },
                fog_near_mm: 2500.0,
                fog_far_mm: if upper { 30000.0 } else { 28000.0 },
                density: if upper { 0.85 } else { 0.95 },
                curve: FogFalloffCurve::Exponential,
                speed_mult,
                layers: base_layers,
                particle_count: PS2_DEFAULT_FOG_PARTICLE_COUNT,
                particle_range_x_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_range_z_mm: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
                particle_fixed_y_mm: None,
                james_fog_influence_mm,
                outdoor_layers_enabled: true,
                final_boss_upper_override: upper,
            }
        }
        R_RESTROOM_START_ID | R_HTL_RM_312_ID => RoomFogEvaluation {
            fog_color: [0.22, 0.21, 0.18, 1.0],
            fog_near_mm: 2000.0,
            fog_far_mm: 14000.0,
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
        },
        _ => RoomFogEvaluation {
            fog_color: [0.10, 0.11, 0.13, 1.0],
            fog_near_mm: 1200.0,
            fog_far_mm: 9000.0,
            density: 0.85,
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
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn final_boss_arena_switches_fog_above_camera_y_threshold() {
        let lower = evaluate_room_fog(&RoomFogContext {
            room_id: R_FINAL_BOSS_RM_ID,
            camera_pos_y_mm: -12000.0,
            ..Default::default()
        });
        let upper = evaluate_room_fog(&RoomFogContext {
            room_id: R_FINAL_BOSS_RM_ID,
            camera_pos_y_mm: -16000.0,
            ..Default::default()
        });
        assert!(!lower.final_boss_upper_override);
        assert!(upper.final_boss_upper_override);
        assert_eq!(upper.speed_mult, FOG_SPEED_OUTDOOR_SLOW);
        assert!(upper.fog_far_mm > lower.fog_far_mm);
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
}
