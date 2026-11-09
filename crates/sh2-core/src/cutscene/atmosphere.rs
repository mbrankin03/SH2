//! PS2 Room & Cutscene Atmospheric Grading, `TimeOfDay` Interior Presets (`0x009428E2`),
//! Lakeview Hotel Room 312 Sunset Tint (`Ambient_Yu` at `0x00942C50`), and Leave/Maria Ending
//! Graveyard Tree Lighting (`0x00476830` / `Asahi_col` at `0x00942C20`) (`SH2-105`).
//!
//! # Ground Truth (`SLUS_202.28`, `sh2pc.exe`, & SH2:EE)
//! 1. **Wood Side Rm 307 Closet Cutscene (`CS_APT_RPT_CLOSET = 0x0E`, `demo_no = 14`, `ClosetCutscene.cpp:228..247`)**:
//!    - While `demo_no == 14` (`sankaku1` / `wp_closet`), overrides `TimeOfDay` (`0x009428E2`, `ap18_ap21_HighLight`)
//!      from the bright daytime apartment preset `11` to `15` (darkened interior closet slats), and restores `11`
//!      immediately when the cutscene ends.
//!    - Suppresses `FlashLightRender` (`0x01F7DC04 = 0`) and zeroes spotlight RGB when `CameraX == -22256.61133f` (`ClosetCutscene.cpp:164`).
//! 2. **Lakeview Hotel Room 312 Sunset Grading (`R_HTL_RM_312 = 0xA2`, `TreeLighting.cpp:193..261`)**:
//!    - Before James watches the VHS tape (`event_index != 0` and `transition_state != FADE_BLACK_SRC`),
//!      restores the warm PS2 golden sunset world ambient/vertex color `RGB(255, 255, 227)` (`[1.0, 1.0, 227.0 / 255.0]`)
//!      at `NowDrawEnvData + 0x30` (`0x00942C50` / `Ambient_Yu`) and zeroes the indoor fog volume intensity
//!      (`RunHotelRoom312FogVolumeFix`) so fog clouds do not wash out the sunlit window vista.
//! 3. **Leave & Maria Ending Graveyard Tree Lighting (`CS_END_LEAVE_LETTER = 0x5A`, `CS_END_MARIA_LETTER = 0x5D`, `TreeLighting.cpp:101..191`)**:
//!    - In `DdsPlay` (`0x004A13C5`), calls `sh_Set_DrawEnvLightData` (`0x00476830`) with mode `2` instead of `1`
//!      during `demo_no == 90` (`0x5A`) and `demo_no == 93` (`0x5D`) so graveyard tree foliage receives proper
//!      two-sided PS2 vertex lighting.
//!    - During `CS_END_MARIA_LETTER` (`demo_no == 93`, `0x5D`), sets horizon fog color (`NowDrawEnvData + 0x00` / `0x00942C20` `Asahi_col`)
//!      to `RGB(125, 135, 150)` (`[125.0 / 255.0, 135.0 / 255.0, 150.0 / 255.0]`).

use super::stage_fixes::{
    ClosetCutsceneFixState, FinalBossRoomFixState, CS_APT_RPT_CLOSET_DEMO_NO, R_FINAL_BOSS_RM_ID,
};

/// `demo_no == 19` (`0x13`, `CS_APT_RPT_FIGHT`, Wood Side Room 307 Pyramid Head encounter).
pub const CS_APT_RPT_FIGHT_DEMO_NO: u16 = 19;

/// `demo_no == 90` (`0x5A`, `CS_END_LEAVE_LETTER`, Leave Ending graveyard letter cutscene).
pub const CS_END_LEAVE_LETTER_DEMO_NO: u16 = 90;

/// `demo_no == 93` (`0x5D`, `CS_END_MARIA_LETTER`, Maria Ending graveyard letter cutscene).
pub const CS_END_MARIA_LETTER_DEMO_NO: u16 = 93;

/// Room ID for Labyrinth Bottom D (`R_LAB_BOTTOM_D = 0x89` in `Patches.h`, Abstract Daddy Piston Room).
pub const R_LAB_BOTTOM_D_ID: u32 = 0x89;

/// Room ID for Labyrinth Top H (`R_LAB_TOP_H = 0x8A` in `Patches.h`, Abstract Daddy Piston Room).
pub const R_LAB_TOP_H_ID: u32 = 0x8A;

/// Room ID for Lakeview Hotel Room 312 (`R_HTL_RM_312 = 0xA2` in `Patches.h`).
pub const R_HTL_RM_312_ID: u32 = 0xA2;

/// Resolves the canonical `room_id` (`R_APT_E_RM_307`, `R_LAB_TOP_H`, `R_HTL_RM_312`, `R_FINAL_BOSS_RM`)
/// from a `(demo_no, stage_name)` pair when no explicit gameplay `room_id` is supplied.
#[inline]
#[must_use]
pub fn resolve_cutscene_room_id(demo_no: u16, stage_name: &str) -> u32 {
    match demo_no {
        CS_APT_RPT_CLOSET_DEMO_NO | CS_APT_RPT_FIGHT_DEMO_NO => {
            super::stage_fixes::R_APT_E_RM_307_ID
        }
        67 | 68 => R_LAB_TOP_H_ID,
        _ => {
            if stage_name.eq_ignore_ascii_case("ap18") || stage_name.eq_ignore_ascii_case("ap21") {
                super::stage_fixes::R_APT_E_RM_307_ID
            } else if stage_name.eq_ignore_ascii_case("rr91")
                || stage_name.eq_ignore_ascii_case("hp99")
            {
                R_HTL_RM_312_ID
            } else if stage_name.eq_ignore_ascii_case("ru73") {
                R_FINAL_BOSS_RM_ID
            } else if stage_name.eq_ignore_ascii_case("ug05") {
                R_LAB_TOP_H_ID
            } else {
                0
            }
        }
    }
}

/// Default daytime `TimeOfDay` preset (`11` at `0x009428E2` in `ClosetCutscene.cpp:240`).
pub const TIME_OF_DAY_APARTMENT_DAY: u8 = 11;

/// Darkened interior closet `TimeOfDay` preset (`15` at `0x009428E2` in `ClosetCutscene.cpp:236`)
/// applied during `CS_APT_RPT_CLOSET` (`demo_no = 14`) so foreground closet bars are properly darkened.
pub const TIME_OF_DAY_CLOSET_DARKENED: u8 = 15;

/// Ambient intensity multiplier (`0.55`) corresponding to `TimeOfDay = 15` inside the Room 307 closet.
pub const CLOSET_DARKENED_AMBIENT_SCALE: f32 = 0.55;

/// Warm PS2 sunset world ambient/vertex color `RGB(255, 255, 227)` (`Ambient_Yu` at `0x00942C50`
/// in `TreeLighting.cpp:209..214`) for Lakeview Hotel Room 312 before watching the VHS tape.
pub const HOTEL_ROOM_312_SUNSET_TINT_RGB: [f32; 3] = [1.0, 1.0, 227.0 / 255.0];

/// Graveyard horizon fog color `RGB(125, 135, 150)` (`Asahi_col` at `0x00942C20` in `TreeLighting.cpp:200..205`)
/// for `CS_END_MARIA_LETTER` (`demo_no = 93`, `0x5D`).
pub const MARIA_ENDING_HORIZON_FOG_RGB: [f32; 3] = [125.0 / 255.0, 135.0 / 255.0, 150.0 / 255.0];

/// Screen transition fade state (`TransitionState` in `TreeLighting.cpp:208`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenTransitionState {
    /// Normal gameplay or cutscene rendering (`!= FADE_BLACK_SRC`).
    #[default]
    Normal,
    /// Fading from a black source screen (`FADE_BLACK_SRC = 0x02`), during which Room 312 sunset override is paused.
    FadeBlackSrc,
}

/// Tree lighting mode passed to `sh_Set_DrawEnvLightData` (`0x00476830` at `0x004A13CB` in `DdsPlay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TreeLightingMode {
    /// Standard single-directional stage lighting (`mode = 1`).
    #[default]
    Standard = 1,
    /// Graveyard foliage lighting (`mode = 2`) used in `CS_END_LEAVE_LETTER` (`0x5A`) and `CS_END_MARIA_LETTER` (`0x5D`).
    EndingGraveyard = 2,
}

/// Input context for evaluating room & cutscene atmospheric grading (`SH2-105`).
#[derive(Debug, Clone, Copy)]
pub struct AtmosphereEvalInput {
    /// Active stage room ID (e.g. `R_APT_E_RM_307 = 0x13`, `R_HTL_RM_312 = 0xA2`, `R_FINAL_BOSS_RM = 0xBB`).
    pub room_id: u32,
    /// Active `.dds` cutscene number (`0` when no cutscene is playing).
    pub demo_no: u16,
    /// Current 30 FPS cutscene frame (`0.0` when no cutscene is playing).
    pub scene_frame: f32,
    /// Current world-space camera X coordinate.
    pub camera_pos_x: f32,
    /// Current world-space James Y elevation (negative-up in PS2 coordinates).
    pub james_pos_y: f32,
    /// Active viewport aspect ratio (`width / height`).
    pub viewport_aspect: f32,
    /// Room event progression index (`EventIndex` in `TreeLighting.cpp:208`; `!= 0` in Room 312 before watching the VHS tape).
    pub event_index: u32,
    /// Current screen fade transition state.
    pub transition_state: ScreenTransitionState,
}

impl Default for AtmosphereEvalInput {
    fn default() -> Self {
        Self {
            room_id: 0,
            demo_no: 0,
            scene_frame: 0.0,
            camera_pos_x: 0.0,
            james_pos_y: 0.0,
            viewport_aspect: 16.0 / 9.0,
            event_index: 1,
            transition_state: ScreenTransitionState::Normal,
        }
    }
}

/// Resolved atmospheric grading, lighting preset, and fog overrides for the current frame (`SH2-105`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereGradingEvaluation {
    /// Active `TimeOfDay` preset index (`15` during `CS_APT_RPT_CLOSET`, `11` default).
    pub time_of_day_preset: u8,
    /// Tree lighting mode (`EndingGraveyard = 2` during `0x5A` / `0x5D`, else `Standard = 1`).
    pub tree_lighting_mode: TreeLightingMode,
    /// Whether Lakeview Hotel Room 312 pre-VHS sunset grading (`Ambient_Yu`) is active.
    pub room_312_sunset_active: bool,
    /// Combined world/character ambient tint `(R, G, B, intensity_scale)` passed to `SpotlightUniforms::ambient_tint`.
    pub ambient_tint_rgba: [f32; 4],
    /// Optional horizon fog RGB override (`Some(RGB(125, 135, 150))` in `CS_END_MARIA_LETTER`).
    pub horizon_fog_rgb_override: Option<[f32; 3]>,
    /// Volumetric fog density multiplier (`0.0` in Room 312 pre-VHS sunset per `RunHotelRoom312FogVolumeFix`, `1.0` otherwise).
    pub fog_density_scale: f32,
    /// Whether James's flashlight must be extinguished (`true` when `CameraX == -22256.61133f` in `CS_APT_RPT_CLOSET`).
    pub flashlight_suppressed: bool,
    /// Optional camera far clip override (`Some(30000.0)` in `R_FINAL_BOSS_RM`).
    pub far_clip_override: Option<f32>,
}

impl Default for AtmosphereGradingEvaluation {
    fn default() -> Self {
        Self {
            time_of_day_preset: TIME_OF_DAY_APARTMENT_DAY,
            tree_lighting_mode: TreeLightingMode::Standard,
            room_312_sunset_active: false,
            ambient_tint_rgba: [1.0, 1.0, 1.0, 1.0],
            horizon_fog_rgb_override: None,
            fog_density_scale: 1.0,
            flashlight_suppressed: false,
            far_clip_override: None,
        }
    }
}

impl AtmosphereGradingEvaluation {
    /// Evaluates the complete PS2 atmospheric grading state from [`AtmosphereEvalInput`] with zero side effects.
    #[must_use]
    pub fn evaluate(input: AtmosphereEvalInput) -> Self {
        // 1. Wood Side Room 307 Closet Cutscene (`CS_APT_RPT_CLOSET = 0x0E`, `demo_no = 14`)
        let closet_fix = ClosetCutsceneFixState::evaluate(
            input.demo_no,
            input.scene_frame,
            input.camera_pos_x,
            input.viewport_aspect,
        );
        let time_of_day_preset = if closet_fix.active {
            TIME_OF_DAY_CLOSET_DARKENED
        } else {
            TIME_OF_DAY_APARTMENT_DAY
        };
        let ambient_scale = if closet_fix.active {
            CLOSET_DARKENED_AMBIENT_SCALE
        } else {
            1.0
        };

        // 2. Leave (`0x5A`) & Maria (`0x5D`) Ending Graveyard Tree Lighting & Horizon Fog (`TreeLighting.cpp`)
        let tree_lighting_mode = if matches!(
            input.demo_no,
            CS_END_LEAVE_LETTER_DEMO_NO | CS_END_MARIA_LETTER_DEMO_NO
        ) {
            TreeLightingMode::EndingGraveyard
        } else {
            TreeLightingMode::Standard
        };
        let horizon_fog_rgb_override = if input.demo_no == CS_END_MARIA_LETTER_DEMO_NO {
            Some(MARIA_ENDING_HORIZON_FOG_RGB)
        } else {
            None
        };

        // 3. Lakeview Hotel Room 312 Sunset Grading (`R_HTL_RM_312 = 0xA2`, pre-VHS tape)
        let room_312_sunset_active = input.room_id == R_HTL_RM_312_ID
            && input.transition_state != ScreenTransitionState::FadeBlackSrc
            && input.event_index != 0;

        let tint_rgb = if room_312_sunset_active {
            HOTEL_ROOM_312_SUNSET_TINT_RGB
        } else {
            [1.0, 1.0, 1.0]
        };
        let fog_density_scale = if room_312_sunset_active { 0.0 } else { 1.0 };

        // 4. Final Boss Room (`R_FINAL_BOSS_RM = 0xBB` / `ru73`) draw distance extension
        let final_boss_fix = FinalBossRoomFixState::evaluate(input.room_id, input.james_pos_y);
        let far_clip_override = if input.room_id == R_FINAL_BOSS_RM_ID {
            Some(final_boss_fix.draw_distance)
        } else {
            None
        };

        Self {
            time_of_day_preset,
            tree_lighting_mode,
            room_312_sunset_active,
            ambient_tint_rgba: [tint_rgb[0], tint_rgb[1], tint_rgb[2], ambient_scale],
            horizon_fog_rgb_override,
            fog_density_scale,
            flashlight_suppressed: closet_fix.flashlight_suppressed,
            far_clip_override,
        }
    }
}
