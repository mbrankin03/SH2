//! Exact `SLUS_202.28` & `sh2pc.exe` static data tables for the `Effect2` blood system.
//!
//! Every constant in this module is extracted verbatim from:
//! - `SLUS_202.28` (`Effect2/hh_class_blood_00.c` .. `hh_class_blood_stick_00.c`,
//!   `Effect2/hh_class_object_execute.c`).
//! - `sh2pc.exe` (`0x004cd290`, `0x004cdc20`, `0x00892340`, `0x008924e8`, `0x0089e808`).
//! - `Silent-Hill-2-Enhancements/Patches/BloodSize.cpp` (`SetBloodSize` @ `0x004ce246`).

use crate::config::BloodColor;

/// Fixed 30 Hz step used by floor decals (`HH_Class_Blood_00` `0x002541a0`,
/// `HH_Class_Blood_02` `0x002551f0`, `HH_Class_Blood_FootMark_00` `0x00255f40`,
/// `HH_Class_Blood_Pool_Phenomenon_00` `0x002560a0`).
pub const BLOOD_DT_30HZ: f32 = 0.033_333_335;

/// Fixed 60 Hz step used by airborne sprays & drops (`HH_Class_Blood_01` `0x002548e0`,
/// `HH_Class_Blood_05` `0x00295380`, `HH_Class_Blood_Splash_Phenomenon_00` `0x00256ba0`).
pub const BLOOD_DT_60HZ: f32 = 0.016_666_668;

/// SH2:EE `IncreaseBlood` scale factor (`BloodSize.cpp` `SetBloodSize` `0x004ce246` -> `1.0f`).
pub const SH2EE_BLOOD_SIZE_SCALE: f32 = 1.0;

/// Maximum bloody footsteps stored by `HH_Class_Blood_Stick_00` (`0x00257420`: `< '\n'`).
pub const MAX_BLOODY_STEPS: u8 = 10;

/// Maximum lifetime of `HH_Class_Blood_Stick_00` in seconds (`0x00257750`: `120.0`).
pub const BLOOD_STICK_MAX_LIFETIME_SEC: f32 = 120.0;

/// Subtractive RGB values in PS2 0..255 GS space (`sh2pc.exe` `DAT_008924e8` @ `0x004cdc20`,
/// `SLUS_202.28` `_83` @ `0x0036de60` = `[40, 255, 230]`, `_84` @ `0x0036e470`, `_511` @ `0x0037d4f0`).
///
/// Used with PS2 GS `ALPHA_1 = 0x8000000042` (`Cd - Cs * As`):
/// - `Normal` Red: `[40, 255, 230]` (subtracts green & blue, leaving crimson red)
/// - `Green`: `[180, 4, 150]` (subtracts red & blue, leaving green)
/// - `Violet`: `[24, 230, 23]` (subtracts green, leaving violet)
/// - `Black`: `[194, 210, 180]` (subtracts all channels nearly equally, leaving black)
pub const BLOOD_COLOR_SUBTRACTIVE_RGB_GS: [[f32; 3]; 4] = [
    [40.0, 255.0, 230.0],
    [180.0, 4.0, 150.0],
    [24.0, 230.0, 23.0],
    [194.0, 210.0, 180.0],
];

/// Subtractive RGB values for `HH_Class_Blood_04` (`sh2pc.exe` `DAT_00892340` @ `0x004cd290`,
/// `SLUS_202.28` `Base_Rgba_84` @ `0x0037bb00` = `[60, 255, 230]`).
pub const BLOOD_04_COLOR_SUBTRACTIVE_RGB_GS: [[f32; 3]; 4] = [
    [60.0, 255.0, 230.0],
    [180.0, 4.0, 150.0],
    [24.0, 230.0, 23.0],
    [194.0, 210.0, 180.0],
];

/// Non-subtractive fallback RGBA values (`sh2pc.exe` `DAT_0089e808` when `DAT_00a3339c == 0`).
pub const BLOOD_COLOR_NON_SUBTRACTIVE_RGBA_GS: [[f32; 4]; 4] = [
    [100.0, 22.0, 17.0, 255.0],
    [80.0, 100.0, 40.0, 255.0],
    [90.0, 32.0, 90.0, 255.0],
    [0.0, 0.0, 0.0, 255.0],
];

/// Returns normalized `[r, g, b]` in `[0.0, 1.0]` for subtractive (`Cd - Cs * As`) blending.
#[must_use]
pub const fn blood_color_subtractive_rgb_norm(color: BloodColor) -> [f32; 3] {
    let gs = BLOOD_COLOR_SUBTRACTIVE_RGB_GS[color as usize];
    [gs[0] / 255.0, gs[1] / 255.0, gs[2] / 255.0]
}

/// Returns normalized `[r, g, b, a]` in `[0.0, 1.0]` for non-subtractive fallback blending.
#[must_use]
pub const fn blood_color_non_subtractive_rgba_norm(color: BloodColor) -> [f32; 4] {
    let gs = BLOOD_COLOR_NON_SUBTRACTIVE_RGBA_GS[color as usize];
    [gs[0] / 255.0, gs[1] / 255.0, gs[2] / 255.0, gs[3] / 255.0]
}

// ============================================================================
// Class 5 / 7: Airborne Hit Spray & Falling Drops (hh_class_blood_01.c)
// ============================================================================

/// Horizontal XZ velocity scale in `hh_class_blood_01.c` `Object_Initialize` (`0x002542e0`).
pub const BLOOD_01_VEL_XZ_SCALE: f32 = 0.549_450_5;

/// Vertical Y velocity scale in `hh_class_blood_01.c` `Object_Initialize` (`0x002542e0`).
pub const BLOOD_01_VEL_Y_SCALE: f32 = 1.098_901;

/// Gravity suppression coefficient `_suppress_coff` (`SLUS_202.28` `0x0036db40` = `2.0`).
pub const BLOOD_01_SUPPRESS_COFF: f32 = 2.0;

/// Parabolic gravity in PS2 Y-down mm/s² (`2.0 * 2450.0 * 1.098901 = 5384.615`).
pub const BLOOD_01_GRAVITY_MM_S2: f32 = BLOOD_01_SUPPRESS_COFF * 2450.0 * BLOOD_01_VEL_Y_SCALE;

/// Base billboard half-extent in PS2 mm (`_vertex_info_list` @ `0x0036db50`).
pub const BLOOD_01_HALF_EXTENT_MM: f32 = 25.0;

/// Mode 0 (mist) & Mode 1 (falling drop) start/end scale (`_scale_start_list` `0x0036de10`,
/// `_scale_end_list` `0x0036de18`).
pub const BLOOD_01_SCALE_START: [f32; 2] = [0.8, 0.8];
pub const BLOOD_01_SCALE_END: [f32; 2] = [7.0, 2.0];

/// Mode 0 (mist) & Mode 1 (falling drop) start/end GS alpha (`_rgba_start_list` `0x0036de00`,
/// `_rgba_end_list` `0x0036de08`).
pub const BLOOD_01_ALPHA_START_GS: [f32; 2] = [128.0, 128.0];
pub const BLOOD_01_ALPHA_END_GS: [f32; 2] = [0.0, 128.0];

/// Mode 0 (mist = `0.6`s @ `0x0036de20`) & Mode 1 (drop = `1.0`s @ `0x0036de40`) lifetimes.
pub const BLOOD_01_LIFETIME_SEC: [f32; 2] = [0.6, 1.0];

/// UV rects `[u_min, v_min, u_max, v_max]` in `bloodtex0.tbn2` for airborne mist/drops
/// (bottom-right puffs `[0.5..1.0, 0.5..1.0]` & mid-left drops `[0.0..0.5, 0.5..0.75]`).
pub const BLOOD_01_MIST_UV_RECTS: [[f32; 4]; 4] = [
    [0.5, 0.5, 0.75, 0.75],
    [0.75, 0.5, 1.0, 0.75],
    [0.5, 0.75, 0.75, 1.0],
    [0.75, 0.75, 1.0, 1.0],
];

pub const BLOOD_01_DROP_UV_RECTS: [[f32; 4]; 4] = [
    [0.0, 0.5, 0.25, 0.75],
    [0.25, 0.5, 0.5, 0.75],
    [0.0, 0.5, 0.25, 0.75],
    [0.25, 0.5, 0.5, 0.75],
];

// ============================================================================
// Class 11: Hit Spray Emitter (hh_class_blood_splash_phenomenon_00.c)
// ============================================================================

/// Active duration of `HH_Class_Blood_Splash_Phenomenon_00` (`0x00256ba0` line 240: `0.5 < t`).
pub const BLOOD_SPLASH_00_DURATION_SEC: f32 = 0.5;

/// Max particles per channel (`_discharge_full_number` @ `0x0036e6c0`: `[5, 3]`).
pub const BLOOD_SPLASH_00_FULL_NUMBER: [u16; 2] = [5, 3];

/// Emission interval per channel in seconds (`_discharge_time` @ `0x0036e6c8`: `[0.01, 0.13333334]`).
pub const BLOOD_SPLASH_00_DISCHARGE_TIME: [f32; 2] = [0.01, 0.133_333_34];

// ============================================================================
// Class 10: Floor Blood Stain from Falling Drop (hh_class_blood_00.c)
// ============================================================================

/// Hold duration in seconds before fade-out (`HH_Class_Blood_00` `0x002541a0`: `30.0`).
pub const BLOOD_00_HOLD_DURATION_SEC: f32 = 30.0;

/// Scale end list in mm (`_scale_end_list` @ `0x0036da90`, first 5 variants).
pub const BLOOD_00_SCALE_END_MM: [[f32; 2]; 5] = [
    [40.0, 40.0],
    [30.0, 35.0],
    [35.0, 30.0],
    [35.0, 32.0],
    [30.0, 30.0],
];

/// Motion envelope keyframes `[dt_sec, dscale_t, drgba_t]` (`_motion_info` @ `0x0036db10`).
/// Note: `Object_Motion_00` (`0x00253c80`) accumulates `keyframes[0]` (`0.70` initial scale)
/// before interpolating segments `1..=3` (`1.0s`, `2.0s`, `4.0s`).
pub const BLOOD_00_MOTION_INFO: [[f32; 3]; 4] = [
    [0.0, 0.70, 0.0],
    [1.0, 0.15, 0.0],
    [2.0, 0.10, 0.35],
    [4.0, 0.05, 0.65],
];

/// UV rect in `bloodtex0.tbn2` for floor drop stains (top-left splatter sheet).
pub const BLOOD_00_UV_RECT: [f32; 4] = [0.0, 0.0, 0.5, 0.5];

// ============================================================================
// Class 3 & Class 12: 3-Stage Expanding Death Pool (hh_class_blood_02.c & pool_00.c)
// ============================================================================

/// `enEfctBloodPool` (`0x00146bb0`): spawns pool 3.0 mm above enemy origin (`fStack_c - 3.0`).
pub const EN_EFCT_BLOOD_POOL_Y_OFFSET_MM: f32 = -3.0;

/// Staggered spawn times in seconds (`_Time_Table` @ `0x0036e650`, 5 variants × 3 pools).
pub const BLOOD_POOL_00_TIME_TABLE: [[f32; 3]; 5] = [
    [0.0, 1.0, 2.0],
    [0.0, 2.0, 4.0],
    [1.0, 2.0, 4.0],
    [2.0, 3.0, 4.0],
    [1.6, 3.0, 5.0],
];

/// Minimum radial offset in mm (`HH_Class_Blood_Pool_Phenomenon_00` `0x002560a0`: `50.0 + rand % 150`).
pub const BLOOD_POOL_00_MIN_OFFSET_MM: f32 = 50.0;
pub const BLOOD_POOL_00_OFFSET_RANGE_MM: f32 = 150.0;

/// 5 pool scale-end variants in mm (`_scale_end_list` @ `0x0036e280`).
pub const BLOOD_02_SCALE_END_MM: [[f32; 2]; 5] = [
    [400.0, 400.0],
    [350.0, 400.0],
    [400.0, 350.0],
    [300.0, 400.0],
    [400.0, 300.0],
];

/// 3-stage expansion keyframes `[dt_sec, dscale_t, drgba_t]` (`_motion_info` @ `0x0036e2d0`).
/// Total expansion duration = `2.0 + 6.0 + 8.0 = 16.0` seconds.
pub const BLOOD_02_MOTION_INFO: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [2.0, 0.3, 0.0],
    [6.0, 0.5, 0.35],
    [8.0, 0.2, 0.65],
];

/// Collision threshold multiplier (`HH_Class_Blood_02_DesignateLocation_CollisionCheck` `0x002552c0`:
/// `dist_sq < min_radius_sq * 0.95`).
pub const BLOOD_02_COLLISION_SCALE_SQ: f32 = 0.95;

/// UV rects for `Blood02` pool variants in `bloodtex0.tbn2` (top-left & top-right).
pub const BLOOD_02_UV_RECTS: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.5, 0.5],
    [0.5, 0.0, 1.0, 0.5],
    [0.0, 0.0, 0.5, 0.5],
    [0.5, 0.0, 1.0, 0.5],
];

// ============================================================================
// Class 9: Bloody Footprints (hh_class_blood_footmark_00.c)
// ============================================================================

/// Footprint quad half-width (X) and half-length (Z) in PS2 mm (`0x0036e480`: `110.0, 120.0`).
pub const BLOOD_FOOTMARK_HALF_WIDTH_MM: f32 = 110.0;
pub const BLOOD_FOOTMARK_HALF_LENGTH_MM: f32 = 120.0;

/// Lateral stance offset from James's center to left/right foot in PS2 mm (`±110.0`).
pub const BLOOD_FOOTMARK_STANCE_OFFSET_MM: f32 = 110.0;

/// Hold time in seconds before fade (`HH_Class_Blood_FootMark_00` `0x00255f40`: `10.0`).
pub const BLOOD_FOOTMARK_HOLD_SEC: f32 = 10.0;

/// Alpha fade decrement per 30 Hz tick (`HH_Class_Blood_FootMark_00` `0x00255f40`: `0.01`).
pub const BLOOD_FOOTMARK_FADE_PER_30HZ: f32 = 0.01;

/// Base GS alpha (`_square_00_rgba` @ `0x0036e5e0`: `128.0`).
pub const BLOOD_FOOTMARK_BASE_ALPHA_GS: f32 = 128.0;

/// UV rects `[u_min, v_min, u_max, v_max]` for left/right footprints (`_square_0x_stq_list` @ `0x0036e4c0..0x0036e5b0`).
/// Index `[side (0=right, 1=left)][variant (0..2)]`.
pub const BLOOD_FOOTMARK_UV_RECTS: [[[f32; 4]; 2]; 2] = [
    [[0.125, 0.75, 0.25, 0.875], [0.125, 0.875, 0.25, 1.0]],
    [[0.0, 0.75, 0.125, 0.875], [0.0, 0.875, 0.125, 1.0]],
];
