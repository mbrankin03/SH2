//! Authentic 1:1 Silent Hill 2 room darkness modulation, flashlight acquisition state,
//! ambient lighting scaling, camera view-space directional fill light, and PS2/SH2:EE
//! flashlight intensity & reach calibration (`SH2-33`).
//!
//! # Ground-Truth Binary References
//! - `sh2pc.exe` `0x50a3c0` (`env_flash_off_level`): per-room map ID lookup table & default
//!   fallback testing Event Flag `625` (`0x01f7aa2c & 0x20000`) and carried item `0x12`
//!   (`0x01f7a7e0 & 0x40000`), returning `0.0f` (`0x62eddc`), `0.3f` (`0x89ffc8` / `0x89ffcc`),
//!   `0.75f` (`0x89ffd0`), or `1.0f` (`0x89ffc4`).
//! - `sh2pc.exe` `0x50a3b0` (`0x89ffc0` / `s_flash_level`): getter for active flashlight level
//!   (`0.0f` when flashlight is ON at `0x50af9f`).
//! - `sh2pc.exe` `0x47a090..0x47a18b`: ambient register `c19` modulation
//!   `Ambient * (4.0 + 2.0 * s_flash_level) / 8.0 = Ambient * (0.5 + 0.25 * s_flash_level)`.
//! - `sh2pc.exe` `0x50b036` (`0x47a6d0`, `0x89ffd8 = [0.3, 0.3, 0.3, 0.0]`, `0x47b210`):
//!   camera view-space directional fill light `k = 0.3 * s_flash_level * 0.25`.
//! - `sh2pc.exe` `0x47a535` & `0x47b99a` (`Silent-Hill-2-Enhancements/Patches/EnhancedFlashlight.cpp`):
//!   PS2 flashlight wall brightness ratio `1.2 / 4.0` (`0.3`) and extended reach `8500.0` mm
//!   (vs `6000.0` mm on original PS2).
//! - `Silent-Hill-2-Enhancements/Patches/LightingPatch.cpp:91-148` (`RunRoomLighting`):
//!   per-room `s_flash_level` overrides for Main Scenario (`R_MOTORHOME`, `R_TOWN_EAST`,
//!   `R_APT_E_COURTYARD`, `R_TOWN_WEST`, `R_TOWN_LAKE`) and Born From a Wish
//!   (`R_TOWN_WEST`, `R_MAN_OUTSIDE_ENTRANCE`, `R_MAN_BLUE_CREEK_ENTRANCE`).
//! - `SLUS_202.28` `LightSpotOnOffCheck` (`0x001aad70`) & `sh2gfw_Set_JmsSpot_OnOrOff` (`0x0017e3c0`).

use crate::config::LightingProfile;
use crate::lighting::draw_env::{DrawEnvData, ENV_FLASH_OFF_POST_PICKUP, ENV_FLASH_OFF_PRE_PICKUP};
use crate::lighting::fixes::{
    ShadowChapter, PC_ENVIRONMENT_BRIGHTNESS_MUL, PS2_FLASHLIGHT_RANGE_MM,
    SH2EE_FLASHLIGHT_BRIGHTNESS_MUL, SH2EE_FLASHLIGHT_RANGE_MM,
};

/// Inventory catalog ID for the Flashlight (`item 0x12` = 18, `sh2pc.exe` `0x50a48e`:
/// `test dword ptr [0x01f7a7e0], 0x40000`).
pub const ITEM_ID_FLASHLIGHT: u8 = 0x12;

/// `s_flash_level` when the flashlight is switched ON (`sh2pc.exe` `0x50af9f` `0x89ffc0 = 0.0f`).
pub const FLASH_LEVEL_ON: f32 = 0.0;

/// `s_flash_level` in dark rooms when the flashlight is OFF prior to Room 205 pickup
/// (`sh2pc.exe` `0x50a487` `0x89ffc4 = 1.0f`).
pub const FLASH_LEVEL_OFF_PRE_PICKUP: f32 = ENV_FLASH_OFF_PRE_PICKUP;

/// `s_flash_level` in dark rooms when the flashlight is OFF after acquiring item `0x12`
/// (`sh2pc.exe` `0x50a41b` / `0x50a461`, `0x89ffc8` / `0x89ffcc` = `0.3f`).
pub const FLASH_LEVEL_OFF_POST_PICKUP: f32 = ENV_FLASH_OFF_POST_PICKUP;

/// Intermediate `s_flash_level` for specific rooms such as `0xa0001` and `0xd0019`
/// (`sh2pc.exe` `0x50a3fc` `0x89ffd0 = 0.75f`).
pub const FLASH_LEVEL_ROOM_MID: f32 = 0.75;

/// Base ambient scale factor (`4.0 / 8.0 = 0.5`) in `c19 = Ambient * (0.5 + 0.25 * s_flash_level)`
/// (`sh2pc.exe` `0x47a09a..0x47a0c1`, `0x6318d4 = 4.0f`).
pub const AMBIENT_SCALE_BASE: f32 = 0.5;

/// `s_flash_level` ambient scale coefficient (`2.0 / 8.0 = 0.25`) in
/// `c19 = Ambient * (0.5 + 0.25 * s_flash_level)` (`sh2pc.exe` `0x47a15d..0x47a18b`).
pub const AMBIENT_SCALE_FLASH_COEFF: f32 = 0.25;

/// Base camera view-space directional fill light RGB component (`0.3f`, `sh2pc.exe` `0x89ffd8`).
pub const CAMERA_FILL_BASE: f32 = 0.3;

/// Camera fill light normalization factor (`1.0 / 4.0 = 0.25`, `sh2pc.exe` `0x6318d4 = 4.0f`).
pub const CAMERA_FILL_NORM: f32 = 0.25;

/// Authentic PS2 flashlight color scale ratio (`1.2 / 4.0 = 0.3`, `EnhancedFlashlight.cpp:49,64`,
/// `sh2pc.exe` `0x47a535`), preventing washed-out PC walls.
pub const PS2_FLASHLIGHT_COLOR_SCALE: f32 =
    SH2EE_FLASHLIGHT_BRIGHTNESS_MUL / PC_ENVIRONMENT_BRIGHTNESS_MUL;

/// Retail SH2 Room IDs referenced by `LightingPatch.cpp:119-143` (`Patches/Patches.h:162-172,367-368`).
pub mod room_ids {
    /// East South Vale streets (`Patches/Patches.h:162`).
    pub const R_TOWN_EAST: u32 = 0x04;
    /// Saul Street / Texxon Motorhome (`Patches/Patches.h:163`).
    pub const R_MOTORHOME: u32 = 0x05;
    /// Wood Side Apartments East Courtyard (`Patches/Patches.h:165`).
    pub const R_APT_E_COURTYARD: u32 = 0x07;
    /// West South Vale streets (`Patches/Patches.h:166`).
    pub const R_TOWN_WEST: u32 = 0x08;
    /// Toluca Lake boat sequence (`Patches/Patches.h:172`).
    pub const R_TOWN_LAKE: u32 = 0x0E;
    /// Baldwin Mansion outside entrance (`Patches/Patches.h:367`, Born From a Wish).
    pub const R_MAN_OUTSIDE_ENTRANCE: u32 = 0xD3;
    /// Baldwin Mansion Blue Creek entrance (`Patches/Patches.h:368`, Born From a Wish).
    pub const R_MAN_BLUE_CREEK_ENTRANCE: u32 = 0xD4;
}

/// Inputs required to evaluate authentic 1:1 Silent Hill 2 room darkness modulation (`SH2-33`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomDarknessInput {
    /// Packed PS2/PC map ID `(stage_id << 16) | room_no` (`sh2pc.exe` `0x47b700`), if known.
    pub packed_map_id: Option<u32>,
    /// Retail SH2 room ID (`Patches/Patches.h`, e.g. `room_ids::R_MOTORHOME`), if known.
    pub room_id: Option<u32>,
    /// Active scenario chapter (`MainScenario` vs `BornFromAWish`).
    pub chapter: ShadowChapter,
    /// Whether the active room is a dark interior (`s_env.dark == true`).
    pub is_dark_room: bool,
    /// Whether the player possesses the Flashlight (`item 0x12`, `0x01f7a7e0 & 0x40000`).
    pub has_flashlight: bool,
    /// Whether the flashlight is currently emitting light (`LightSpotOnOffCheck` @ `0x001aad70`).
    pub flashlight_on: bool,
    /// Whether Event Flag `625` (`FLAG_WOODSIDE_FLASHLIGHT_PICKUP`, `0x01f7aa2c & 0x20000`) is set.
    pub flag_625_set: bool,
    /// Optional world color RGB bytes `(R, G, B)` for the `R_MOTORHOME` `(70, 70, 70)` guard
    /// (`LightingPatch.cpp:122`). When `None`, the motorhome override applies unconditionally.
    pub world_color_rgb: Option<[u8; 3]>,
    /// Active lighting profile (`Ps2Authentic`, `EnhancedEdition`, or `NativeEnhanced`).
    pub lighting_profile: LightingProfile,
}

/// Evaluated room darkness modulation and flashlight calibration parameters (`SH2-33`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomDarknessModulation {
    /// Computed `s_flash_level` (`sh2pc.exe` `0x89ffc0` / `0x8a2ba8`), in `0.0..=1.0`.
    pub flash_level: f32,
    /// Ambient scaling multiplier `0.5 + 0.25 * flash_level` (`sh2pc.exe` `0x47a090..0x47a18b`).
    pub ambient_scale: f32,
    /// Camera view-space directional fill intensity `0.3 * flash_level * 0.25`
    /// (`sh2pc.exe` `0x50b036`, `0x89ffd8`).
    pub camera_fill_k: f32,
    /// PS2-calibrated flashlight color scale (`1.2 / 4.0 = 0.3`, `EnhancedFlashlight.cpp:49,64`).
    pub flashlight_color_scale: f32,
    /// Calibrated flashlight reach in PS2 mm (`6000.0` for `Ps2Authentic`, `8500.0` for SH2:EE/Native,
    /// `EnhancedFlashlight.cpp:162-164`).
    pub flashlight_reach_mm: f32,
}

/// Evaluates `sh2pc.exe` `0x50a3c0` (`env_flash_off_level`) with full 1:1 per-room map ID overrides.
///
/// Disassembled table from `sh2pc.exe` `0x50a3c0..0x50a530`:
/// - `0.0f` (`0x62eddc`): `0x60005`, `0xa003d`, `0xa007d`, `0xa0091`, `0xa00e6`, `0xc005b`,
///   `0xd0015`, `0xd0049`
/// - `0.75f` (`0x89ffd0`): `0xa0001`, `0xa0045`, `0xa0085`, `0xa00ca`, `0xa00d6`, `0xd0019`
/// - `0.3f` (`0x89ffcc`): `0xc0015`, `0xd0025`
/// - `1.0f` (`0x89ffc4`): `0xd0029`, `0xd002d`, `0xd0031`, `0xd0039`
/// - Default (`0x50a40f` / `0x50a48e`): `0.3f` if Event Flag `625` (`0x1f7aa2c & 0x20000`) or
///   Flashlight item `0x12` (`0x1f7a7e0 & 0x40000`) is set, else `1.0f`.
#[inline]
#[must_use]
pub const fn env_flash_off_level_for_map(
    packed_map_id: Option<u32>,
    has_flashlight: bool,
    flag_625_set: bool,
) -> f32 {
    if let Some(map_id) = packed_map_id {
        match map_id {
            0x06_0005 | 0x0a_003d | 0x0a_007d | 0x0a_0091 | 0x0a_00e6 | 0x0c_005b | 0x0d_0015
            | 0x0d_0049 => return FLASH_LEVEL_ON,
            0x0a_0001 | 0x0a_0045 | 0x0a_0085 | 0x0a_00ca | 0x0a_00d6 | 0x0d_0019 => {
                return FLASH_LEVEL_ROOM_MID;
            }
            0x0c_0015 | 0x0d_0025 => return FLASH_LEVEL_OFF_POST_PICKUP,
            0x0d_0029 | 0x0d_002d | 0x0d_0031 | 0x0d_0039 => return FLASH_LEVEL_OFF_PRE_PICKUP,
            _ => {}
        }
    }

    if flag_625_set || has_flashlight {
        FLASH_LEVEL_OFF_POST_PICKUP
    } else {
        FLASH_LEVEL_OFF_PRE_PICKUP
    }
}

/// Evaluates SH2:EE per-room `s_flash_level` overrides (`LightingPatch.cpp:119-143`, `RunRoomLighting`).
///
/// Returns `Some(level)` when a room override applies for the given `chapter` and `room_id`:
/// - **Main Scenario:**
///   - `R_MOTORHOME` (`0x05`) when `world_color_rgb` is `None` or `Some([70, 70, 70])`: `1.0f`
///   - `R_TOWN_EAST` (`0x04`), `R_APT_E_COURTYARD` (`0x07`), `R_TOWN_WEST` (`0x08`),
///     `R_TOWN_LAKE` (`0x0E`): `0.0f`
/// - **Born From a Wish:**
///   - `R_TOWN_WEST` (`0x08`), `R_MAN_OUTSIDE_ENTRANCE` (`0xD3`): `1.0f`
///   - `R_MAN_BLUE_CREEK_ENTRANCE` (`0xD4`): `0.0f`
#[inline]
#[must_use]
pub const fn env_ee_room_level(
    chapter: ShadowChapter,
    room_id: u32,
    world_color_rgb: Option<[u8; 3]>,
) -> Option<f32> {
    match chapter {
        ShadowChapter::MainScenario => match room_id {
            room_ids::R_MOTORHOME => match world_color_rgb {
                None | Some([70, 70, 70]) => Some(FLASH_LEVEL_OFF_PRE_PICKUP),
                Some(_) => None,
            },
            room_ids::R_TOWN_EAST
            | room_ids::R_APT_E_COURTYARD
            | room_ids::R_TOWN_WEST
            | room_ids::R_TOWN_LAKE => Some(FLASH_LEVEL_ON),
            _ => None,
        },
        ShadowChapter::BornFromAWish => match room_id {
            room_ids::R_TOWN_WEST | room_ids::R_MAN_OUTSIDE_ENTRANCE => {
                Some(FLASH_LEVEL_OFF_PRE_PICKUP)
            }
            room_ids::R_MAN_BLUE_CREEK_ENTRANCE => Some(FLASH_LEVEL_ON),
            _ => None,
        },
    }
}

/// Computes the packed PS2/PC map ID `(stage_id << 16) | room_no` from a [`DrawEnvData`] entry
/// when neither `stage_id` nor `room_no` is the `0xFFFF` wildcard.
#[inline]
#[must_use]
pub const fn packed_map_id_from_draw_env(env: &DrawEnvData) -> Option<u32> {
    if env.stage_id == 0xFFFF || env.room_no == 0xFFFF {
        None
    } else {
        Some(((env.stage_id as u32) << 16) | (env.room_no as u32))
    }
}

/// Evaluates authentic 1:1 room darkness modulation, ambient scaling (`c19`), camera fill
/// intensity (`k`), and PS2/SH2:EE flashlight calibration (`SH2-33`).
#[must_use]
pub const fn evaluate_room_darkness(input: &RoomDarknessInput) -> RoomDarknessModulation {
    let flash_level = if input.flashlight_on {
        // Flashlight ON (`sh2pc.exe` `0x50af9f`): s_flash_level = 0.0f
        FLASH_LEVEL_ON
    } else {
        // Check SH2:EE per-room override when not in strict Ps2Authentic mode
        let ee_override = if !matches!(input.lighting_profile, LightingProfile::Ps2Authentic) {
            match input.room_id {
                Some(rid) => env_ee_room_level(input.chapter, rid, input.world_color_rgb),
                None => None,
            }
        } else {
            None
        };

        if let Some(level) = ee_override {
            level
        } else if input.is_dark_room {
            env_flash_off_level_for_map(
                input.packed_map_id,
                input.has_flashlight,
                input.flag_625_set,
            )
        } else {
            // Naturally lit / outdoor bright rooms do not apply indoor dark-room fill
            FLASH_LEVEL_ON
        }
    };

    let ambient_scale = AMBIENT_SCALE_BASE + AMBIENT_SCALE_FLASH_COEFF * flash_level;
    let camera_fill_k = CAMERA_FILL_BASE * flash_level * CAMERA_FILL_NORM;
    let flashlight_reach_mm = match input.lighting_profile {
        LightingProfile::Ps2Authentic => PS2_FLASHLIGHT_RANGE_MM,
        LightingProfile::EnhancedEdition | LightingProfile::NativeEnhanced => {
            SH2EE_FLASHLIGHT_RANGE_MM
        }
    };

    RoomDarknessModulation {
        flash_level,
        ambient_scale,
        camera_fill_k,
        flashlight_color_scale: PS2_FLASHLIGHT_COLOR_SCALE,
        flashlight_reach_mm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dark_room_input(has_flashlight: bool, flashlight_on: bool) -> RoomDarknessInput {
        RoomDarknessInput {
            packed_map_id: None,
            room_id: None,
            chapter: ShadowChapter::MainScenario,
            is_dark_room: true,
            has_flashlight,
            flashlight_on,
            flag_625_set: false,
            world_color_rgb: None,
            lighting_profile: LightingProfile::EnhancedEdition,
        }
    }

    #[test]
    fn flashlight_on_sets_zero_level_and_suppresses_camera_fill() {
        let m = evaluate_room_darkness(&dark_room_input(true, true));
        assert!((m.flash_level - 0.0).abs() < 1e-6);
        assert!((m.ambient_scale - 0.5).abs() < 1e-6);
        assert!((m.camera_fill_k - 0.0).abs() < 1e-6);
        assert!((m.flashlight_color_scale - 0.3).abs() < 1e-6);
        assert!((m.flashlight_reach_mm - 8500.0).abs() < 1e-3);
    }

    #[test]
    fn dark_room_before_flashlight_pickup_uses_level_1_0() {
        let m = evaluate_room_darkness(&dark_room_input(false, false));
        assert!((m.flash_level - 1.0).abs() < 1e-6);
        assert!((m.ambient_scale - 0.75).abs() < 1e-6);
        assert!((m.camera_fill_k - 0.075).abs() < 1e-6);
    }

    #[test]
    fn dark_room_after_flashlight_pickup_toggled_off_drops_to_0_3() {
        let m = evaluate_room_darkness(&dark_room_input(true, false));
        assert!((m.flash_level - 0.3).abs() < 1e-6);
        assert!((m.ambient_scale - 0.575).abs() < 1e-6);
        assert!((m.camera_fill_k - 0.0225).abs() < 1e-6);
        // Verify >3.3x dimmer fill than pre-pickup (0.075 / 0.0225 = 3.333...)
        let pre = evaluate_room_darkness(&dark_room_input(false, false));
        assert!(pre.camera_fill_k / m.camera_fill_k > 3.3);
    }

    #[test]
    fn sh2pc_0x50a3c0_per_map_overrides_match_disassembly() {
        // 0.75f rooms: 0xa0001, 0xd0019, etc.
        assert!((env_flash_off_level_for_map(Some(0x0a_0001), true, true) - 0.75).abs() < 1e-6);
        assert!((env_flash_off_level_for_map(Some(0x0d_0019), false, false) - 0.75).abs() < 1e-6);
        // 0.0f rooms
        assert!((env_flash_off_level_for_map(Some(0x06_0005), false, false) - 0.0).abs() < 1e-6);
        assert!((env_flash_off_level_for_map(Some(0x0d_0049), false, false) - 0.0).abs() < 1e-6);
        // 0.3f rooms
        assert!((env_flash_off_level_for_map(Some(0x0c_0015), false, false) - 0.3).abs() < 1e-6);
        assert!((env_flash_off_level_for_map(Some(0x0d_0025), false, false) - 0.3).abs() < 1e-6);
        // 1.0f rooms
        assert!((env_flash_off_level_for_map(Some(0x0d_0029), true, true) - 1.0).abs() < 1e-6);
        assert!((env_flash_off_level_for_map(Some(0x0d_0039), true, true) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sh2ee_lighting_patch_room_overrides_match_cpp() {
        // Outdoor Main Scenario rooms fixed at 0.0f
        for &rid in &[
            room_ids::R_TOWN_EAST,
            room_ids::R_APT_E_COURTYARD,
            room_ids::R_TOWN_WEST,
            room_ids::R_TOWN_LAKE,
        ] {
            let mut input = dark_room_input(false, false);
            input.room_id = Some(rid);
            let m = evaluate_room_darkness(&input);
            assert!((m.flash_level - 0.0).abs() < 1e-6, "room 0x{rid:x}");
        }

        // Motorhome fixed at 1.0f when world color is (70, 70, 70) or unspecified
        let mut mh = dark_room_input(true, false);
        mh.room_id = Some(room_ids::R_MOTORHOME);
        mh.world_color_rgb = Some([70, 70, 70]);
        assert!((evaluate_room_darkness(&mh).flash_level - 1.0).abs() < 1e-6);

        // Motorhome with non-matching world color falls back to post-pickup 0.3f
        mh.world_color_rgb = Some([0, 0, 0]);
        assert!((evaluate_room_darkness(&mh).flash_level - 0.3).abs() < 1e-6);

        // Born From a Wish overrides
        let mut bfaw = dark_room_input(true, false);
        bfaw.chapter = ShadowChapter::BornFromAWish;
        bfaw.room_id = Some(room_ids::R_MAN_OUTSIDE_ENTRANCE);
        assert!((evaluate_room_darkness(&bfaw).flash_level - 1.0).abs() < 1e-6);
        bfaw.room_id = Some(room_ids::R_MAN_BLUE_CREEK_ENTRANCE);
        assert!((evaluate_room_darkness(&bfaw).flash_level - 0.0).abs() < 1e-6);
    }

    #[test]
    fn ps2_authentic_profile_uses_6000mm_reach_and_skips_ee_overrides() {
        let mut input = dark_room_input(true, false);
        input.room_id = Some(room_ids::R_MOTORHOME);
        input.lighting_profile = LightingProfile::Ps2Authentic;
        let m = evaluate_room_darkness(&input);
        assert!((m.flashlight_reach_mm - 6000.0).abs() < 1e-3);
        // Without EE override, post-pickup dark room is 0.3f
        assert!((m.flash_level - 0.3).abs() < 1e-6);
    }
}
