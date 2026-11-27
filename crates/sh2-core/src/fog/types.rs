//! Core fog parameter types and depth-fog curve definitions (`SH2-32`, `SH2-153`).

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

impl Ps2FogLayerParams {
    /// Strict 1:1 PS2 `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`, `Fog/fog.c:2695-2698`)
    /// square `±0.125` UV half-span parameters (without SH2:EE widescreen `0.250` stretch).
    #[must_use]
    pub const fn classic_ps2() -> Self {
        Self {
            layer1_x1: 0.125,
            layer1_x2: 0.125,
            layer1_y1: 0.125,
            layer1_y2: 0.125,
            layer2_complexity: 0.055,
            layer2_density_add: 100.0,
            layer2_density_mult: 1.4,
            transparency_layer1: 128,
            transparency_layer2: 112,
        }
    }

    /// SH2:EE `FogFix.cpp` / `Settings.h:183-219` widescreen-adjusted `[0.250, 0.125]` layer parameters.
    #[must_use]
    pub const fn sh2ee_enhanced() -> Self {
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

impl Default for Ps2FogLayerParams {
    fn default() -> Self {
        Self::classic_ps2()
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
    /// Normalized minimum depth-fog blend factor at `fog_near_mm` (`1.0 - DrawEnvData.fog_min / 255.0`,
    /// `sh2gfw_Set_DepthFog` `0x00175df0`, `sh2pc.exe` `FUN_00458c20` `sh2pc_00470000.c:7921`).
    pub fog_min: f32,
    /// Normalized maximum depth-fog blend factor at `fog_far_mm` (`1.0 - DrawEnvData.fog_max / 255.0`,
    /// `sh2gfw_Set_DepthFog` `0x00175df0`, `sh2pc.exe` `FUN_00458c30` `sh2pc_00470000.c:7922`).
    pub fog_max: f32,
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
    /// In `R_FINAL_BOSS_RM`, `true` when `camera_pos_y < -15750.0` AND `enable_sh2ee_world_fixes` is enabled
    /// (SH2:EE `FinalAreaBoss2ASM` `FogAdjustments.cpp:100-125` disables depth fog on upper scaffolding).
    pub final_boss_upper_override: bool,
}
