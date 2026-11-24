//! SH2:EE Lighting & Shadow Parity Fixes (gated on [`LightingProfile`] / [`ShadowQualityMode`]
//! so purist players can always select 1:1 original PS2 behavior).
//!
//! Verified 1:1 against `Silent-Hill-2-Enhancements/`:
//! - `Patches/EnhancedFlashlight.cpp` (`PS2FlashlightBrightness`, range `6000` -> `8500`)
//! - `Patches/LightingPatch.cpp` (`RoomLightingFix`, `HalogenLightFix`)
//! - `Patches/CatacombsMeatRoom.cpp` (`CatacombsMeatRoomFix` for `ps189` / `ps193`)
//! - `Patches/Specular.cpp` (`SpecularFix` — prevents double specular intensity on character models/eyelids)
//! - `Wrappers/d3d8/IDirect3DDevice8.cpp:4305` (`EnableSoftShadows` per-room opacity + `~33`-frame fade-in)
//! - `Patches/Room312Shadow.cpp` (`Room312ShadowFix` for `R_HTL_RM_312` / `hp161`)
//! - `Patches/DisableShadowCutscene.cpp` (`DisableShadowCutscene`)
//! - `Patches/AtticShadowFixes.cpp` (`BFaWAtticFix` for `R_MAN_ATTIC`)

use crate::config::{LightingProfile, ShadowQualityMode};
use crate::lighting::draw_env::DrawEnvData;

/// SH2:EE flashlight wall multiplier (`EnhancedFlashlight.cpp:49`, `0x3F99999A = 1.2f`).
pub const SH2EE_FLASHLIGHT_BRIGHTNESS_MUL: f32 = 1.2;

/// Retail PC default wall/environment brightness multiplier (`EnhancedFlashlight.cpp:64`, `0x40800000 = 4.0f`).
pub const PC_ENVIRONMENT_BRIGHTNESS_MUL: f32 = 4.0;

/// Original PS2/PC flashlight reach in mm (`EnhancedFlashlight.cpp:162`, `0x45BB8000 = 6000.0f`).
pub const PS2_FLASHLIGHT_RANGE_MM: f32 = 6000.0;

/// SH2:EE extended flashlight reach in mm (`EnhancedFlashlight.cpp:164`, `0x4604D000 = 8500.0f`).
pub const SH2EE_FLASHLIGHT_RANGE_MM: f32 = 8500.0;

/// Default SH2:EE soft-shadow opacity (`170 / 255`, `IDirect3DDevice8.cpp:4366`).
pub const SH2EE_DEFAULT_SHADOW_OPACITY_U8: u8 = 170;

/// Approximate 33-frame fade-in step per 30 Hz frame (`~3` intensity percent/frame, `IDirect3DDevice8.cpp:4420`).
pub const SH2EE_SHADOW_FADE_IN_FRAMES_30HZ: f32 = 33.0;

/// Approximate fade-out step per 30 Hz frame (`~15` intensity percent/frame, `IDirect3DDevice8.cpp:4435`).
pub const SH2EE_SHADOW_FADE_OUT_FRAMES_30HZ: f32 = 7.0;

/// Enemy reveal lighting minimum brightness (`0.065`) and revealed target (`0.7`)
/// for Wood Side Room 205 mannequin reveal & Demo 6 tunnel Lying Figure.
pub const ENEMY_REVEAL_MIN_LEVEL: f32 = 0.065;
pub const ENEMY_REVEAL_MAX_LEVEL: f32 = 0.7;
/// Demo 6 (tunnel Lying Figure) camera frame threshold (`705.5`) until which reveal lighting is held.
pub const DEMO6_TUNNEL_REVEAL_HOLD_FRAME: f32 = 705.5;

/// Room codes with mode-3 fullbright halogen/fluorescent light fixtures (`HalogenLightFix`).
pub const HALOGEN_FULLBRIGHT_ROOMS: &[&str] = &[
    "ap26", "ap88", "ap108", "hp230", "ps53", "ps145", "qt01", "rr49",
];

/// Returns `true` if `room_code` has mode-3 fullbright light fixtures requiring `HalogenLightFix`.
#[must_use]
pub fn is_halogen_fullbright_room(room_code: &str) -> bool {
    HALOGEN_FULLBRIGHT_ROOMS
        .iter()
        .any(|r| r.eq_ignore_ascii_case(room_code))
}

/// Applies SH2:EE lighting fixes (`PS2FlashlightBrightness`, `CatacombsMeatRoomFix`) to a copy of
/// [`DrawEnvData`] when `profile` enables SH2:EE fixes (`EnhancedEdition` / `NativeEnhanced`).
/// In [`LightingProfile::Ps2Authentic`], returns the unmodified PS2 `DrawEnvData` 1:1.
#[must_use]
pub fn apply_draw_env_fixes(
    base: &DrawEnvData,
    room_code: &str,
    profile: LightingProfile,
) -> DrawEnvData {
    let mut env = *base;
    if profile == LightingProfile::Ps2Authentic {
        return env;
    }

    // `EnhancedFlashlight.cpp:162-164`: extend 6000.0 mm flashlight reach to 8500.0 mm
    if (env.flashlight_range - PS2_FLASHLIGHT_RANGE_MM).abs() < 1.0 {
        env.flashlight_range = SH2EE_FLASHLIGHT_RANGE_MM;
    }

    // `CatacombsMeatRoom.cpp:25-36`: ps189 (Small Room) & ps193 (Large Room)
    if room_code.eq_ignore_ascii_case("ps189") {
        env.vertex_ambient = [2.5, 2.0, 2.0];
    } else if room_code.eq_ignore_ascii_case("ps193") {
        env.vertex_ambient = [3.15, 2.8, 2.8];
        env.base_ambient = [0.0, -0.1, -0.1];
    }

    env
}

/// Returns the flashlight wall brightness multiplier (`1.2` for PS2/EE, `4.0` for unpatched PC).
#[inline]
#[must_use]
pub const fn flashlight_wall_brightness_multiplier(profile: LightingProfile) -> f32 {
    match profile {
        LightingProfile::Ps2Authentic
        | LightingProfile::EnhancedEdition
        | LightingProfile::NativeEnhanced => SH2EE_FLASHLIGHT_BRIGHTNESS_MUL,
    }
}

/// Evaluates `EnemyRevealLighting` for Wood Side Room 205 mannequin reveal (`0.065 -> 0.7`)
/// and Demo 6 tunnel Lying Figure (held at `0.065` until camera frame `705.5`, then `0.7`).
#[must_use]
pub fn evaluate_enemy_reveal_lighting(
    demo_no: Option<u32>,
    camera_frame: f32,
    room_code: &str,
    flashlight_on: bool,
    profile: LightingProfile,
) -> f32 {
    if profile == LightingProfile::Ps2Authentic {
        return 1.0;
    }
    // Demo 6 tunnel Lying Figure
    if demo_no == Some(6) {
        return if camera_frame < DEMO6_TUNNEL_REVEAL_HOLD_FRAME {
            ENEMY_REVEAL_MIN_LEVEL
        } else {
            ENEMY_REVEAL_MAX_LEVEL
        };
    }
    // Wood Side Room 205 mannequin reveal
    if room_code.eq_ignore_ascii_case("ap46") || room_code.eq_ignore_ascii_case("ap_rm205") {
        return if flashlight_on {
            ENEMY_REVEAL_MAX_LEVEL
        } else {
            ENEMY_REVEAL_MIN_LEVEL
        };
    }
    1.0
}

/// Evaluates `SpecularFix` (`Patches/Specular.cpp`): prevents double specular intensity on
/// character models and eyelids (keeping Maria's eyes material index `3` / `1` glossy).
#[must_use]
pub fn specular_intensity_scale(
    model_name: &str,
    material_index: usize,
    profile: LightingProfile,
) -> f32 {
    if profile == LightingProfile::Ps2Authentic {
        return 1.0;
    }
    let lower = model_name.to_ascii_lowercase();
    let is_maria = lower.contains("_mar") || lower.contains("dmr");
    if is_maria {
        let eye_mat = if lower.contains("dmr") { 1 } else { 3 };
        if material_index == eye_mat {
            return 1.0;
        }
        return 0.5;
    }
    // Prevent doubled PC specular on James & other character faces/eyelids
    0.5
}

/// Scenario chapter selector for `EnableSoftShadows` per-room opacity lookup (`IDirect3DDevice8.cpp:4305`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowChapter {
    /// Main Scenario ("Letter from Silent Heaven").
    #[default]
    MainScenario,
    /// Sub-scenario ("Born From a Wish").
    BornFromAWish,
}

/// Returns the SH2:EE per-room soft-shadow opacity in `0..=255` (`IDirect3DDevice8.cpp:4305-4367`).
///
/// Default is `170` (`170/255 ≈ 0.667`).
#[must_use]
pub fn sh2ee_room_shadow_opacity_u8(chapter: ShadowChapter, room_id: u32) -> u8 {
    match chapter {
        ShadowChapter::MainScenario => match room_id {
            0x27 => 40,         // R_APT_E_STAIRCASE_N
            0xA2 | 0xBF => 50,  // R_HTL_RM_312, R_END_DOG_RM
            0x7E => 70,         // R_LAB_BOTTOM_D
            0x19 | 0x3A => 110, // R_BOWL_MAIN, R_APT_W_STAIRCASE_N
            0xBD => 128,        // R_END_BEDROOM_LEAVE
            _ => SH2EE_DEFAULT_SHADOW_OPACITY_U8,
        },
        ShadowChapter::BornFromAWish => match room_id {
            0x36 => 30,  // R_APT_W_RM_203
            0xD5 => 60,  // R_MAN_LOUNGE_2F
            0xD8 => 128, // R_MAN_ATTIC
            // Heaven's Night / West Apts / Baldwin Mansion rooms
            0x1E | 0x1F | 0x39 | 0x3A | 0x33 | 0x34 | 0xC4..=0xDF => 90,
            _ => SH2EE_DEFAULT_SHADOW_OPACITY_U8,
        },
    }
}

/// Shadow fade-in / fade-out state machine (`IDirect3DDevice8.cpp:4413-4455`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowFadePhase {
    #[default]
    Steady,
    FadingIn,
    FadingOut,
    RefadingAmbient,
}

/// Stateful controller for SH2:EE's `~33`-frame shadow opacity fade-in when toggling the flashlight
/// (`IDirect3DDevice8.cpp:4376-4455`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowFadeController {
    /// Current fade phase.
    pub phase: ShadowFadePhase,
    /// Normalized fade intensity in `0.0..=1.0`.
    pub intensity: f32,
    /// Last observed flashlight switch state.
    pub last_flashlight_on: bool,
}

impl Default for ShadowFadeController {
    fn default() -> Self {
        Self {
            phase: ShadowFadePhase::Steady,
            intensity: 1.0,
            last_flashlight_on: false,
        }
    }
}

impl ShadowFadeController {
    /// Advances the shadow fade state by `dt_seconds` (framerate-independent, calibrated to
    /// `33` frames @ 30 Hz = `1.1s` fade-in and `~7` frames @ 30 Hz = `0.233s` fade-out).
    pub fn update(&mut self, flashlight_on: bool, dt_seconds: f32) {
        let dt = dt_seconds.max(0.0);
        if flashlight_on != self.last_flashlight_on {
            self.last_flashlight_on = flashlight_on;
            if flashlight_on {
                self.phase = ShadowFadePhase::FadingIn;
                self.intensity = 0.0;
            } else {
                self.phase = ShadowFadePhase::FadingOut;
            }
        }

        match self.phase {
            ShadowFadePhase::Steady => {
                self.intensity = 1.0;
            }
            ShadowFadePhase::FadingIn => {
                let fade_duration = SH2EE_SHADOW_FADE_IN_FRAMES_30HZ / 30.0;
                self.intensity = (self.intensity + dt / fade_duration).min(1.0);
                if self.intensity >= 1.0 - 1e-5 {
                    self.intensity = 1.0;
                    self.phase = ShadowFadePhase::Steady;
                }
            }
            ShadowFadePhase::FadingOut => {
                let fade_duration = SH2EE_SHADOW_FADE_OUT_FRAMES_30HZ / 30.0;
                self.intensity = (self.intensity - dt / fade_duration).max(0.0);
                if self.intensity <= 1e-5 {
                    self.intensity = 0.0;
                    self.phase = ShadowFadePhase::RefadingAmbient;
                }
            }
            ShadowFadePhase::RefadingAmbient => {
                // `IDirect3DDevice8.cpp:4446`: 1% every other 30 Hz frame (~6.67s full ambient recovery)
                self.intensity = (self.intensity + dt * 0.15).min(1.0);
                if self.intensity >= 1.0 - 1e-5 {
                    self.intensity = 1.0;
                    self.phase = ShadowFadePhase::Steady;
                }
            }
        }
    }

    /// Computes effective shadow opacity in `0.0..=1.0` combining per-room SH2:EE opacity and fade.
    #[must_use]
    pub fn effective_opacity(
        &self,
        shadow_mode: ShadowQualityMode,
        chapter: ShadowChapter,
        room_id: u32,
    ) -> f32 {
        match shadow_mode {
            ShadowQualityMode::ClassicPs2Stencil => {
                f32::from(SH2EE_DEFAULT_SHADOW_OPACITY_U8) / 255.0
            }
            ShadowQualityMode::EnhancedEditionSoft | ShadowQualityMode::NativePcss => {
                let base = f32::from(sh2ee_room_shadow_opacity_u8(chapter, room_id)) / 255.0;
                (base * self.intensity).clamp(0.0, 1.0)
            }
        }
    }
}

/// Evaluates `Room312ShadowFix` (`Patches/Room312Shadow.cpp:32-89`):
/// returns `true` when outdoor balcony shadow bleed onto inner hotel walls in `R_HTL_RM_312` (`hp161`)
/// should be suppressed.
#[inline]
#[must_use]
pub fn should_suppress_room312_balcony_shadow(room_code: &str, room_id: u32) -> bool {
    room_id == 0xA2 || room_code.eq_ignore_ascii_case("hp161")
}

/// Evaluates `DisableShadowCutscene` (`Patches/DisableShadowCutscene.cpp:44-46`):
/// returns `true` when shadows must be disabled for the active cutscene ID and camera X coordinate.
#[must_use]
pub fn should_disable_cutscene_shadows(cutscene_id: u32, camera_pos_x: f32) -> bool {
    const CS_ANGELA_CEMETERY: u32 = 0x05;
    const CS_APT_RPT_CLOSET: u32 = 0x10;
    const CS_BOWL_MARIA: u32 = 0x16;
    const CS_HSP_ALT_RADIO_QUIZ: u32 = 0x26;
    const CS_HSP_ALT_LAURA_LEAVES: u32 = 0x2B;
    const CS_HTL_ALT_RPT_BOSS_INTRO: u32 = 0x50;
    const CS_END_MARIA_EPILOGUE: u32 = 0x5D;
    const CS_END_REBIRTH_EPILOGUE: u32 = 0x5F;

    match cutscene_id {
        CS_ANGELA_CEMETERY
        | CS_HSP_ALT_RADIO_QUIZ
        | CS_BOWL_MARIA
        | CS_HSP_ALT_LAURA_LEAVES
        | CS_END_MARIA_EPILOGUE
        | CS_END_REBIRTH_EPILOGUE => true,
        CS_APT_RPT_CLOSET => (camera_pos_x - (-20_133.998)).abs() < 1.0,
        CS_HTL_ALT_RPT_BOSS_INTRO => (camera_pos_x - (-19_808.115)).abs() < 1.0,
        _ => false,
    }
}

/// Evaluates `BFaWAtticFix` (`Patches/AtticShadowFixes.cpp:73-83`):
/// returns `-10.0` in Baldwin Mansion attic (`R_MAN_ATTIC = 0xC7` in `Patches.h`, or `0xD8`)
/// when `camera_pos_y == -1450.0` to hide the dead shadow caster, and `10.0` otherwise.
#[must_use]
pub fn bfaw_attic_shadow_cutoff_y(room_id: u32, camera_pos_y: f32) -> f32 {
    const R_MAN_ATTIC_PATCHES_H: u32 = 0xC7;
    const R_MAN_ATTIC_LEGACY: u32 = 0xD8;
    if (room_id == R_MAN_ATTIC_PATCHES_H || room_id == R_MAN_ATTIC_LEGACY)
        && (camera_pos_y - (-1450.0)).abs() < 1.0
    {
        -10.0
    } else {
        10.0
    }
}

/// Evaluates `BFaWAtticFix` (`Patches/AtticShadowFixes.cpp:73-83`) gated on `enable_sh2ee_world_fixes`.
/// Returns `10.0` in strict 1:1 Vanilla PS2 mode (`enable_sh2ee_world_fixes == false`).
#[must_use]
pub fn bfaw_attic_shadow_cutoff_y_gated(
    room_id: u32,
    camera_pos_y: f32,
    enable_sh2ee_world_fixes: bool,
) -> f32 {
    if !enable_sh2ee_world_fixes {
        10.0
    } else {
        bfaw_attic_shadow_cutoff_y(room_id, camera_pos_y)
    }
}

/// Light slot mode written by `sh_Kari_LightSetSpot` (`SLUS_202.28` `0x001265f0`, `sh2pc.exe`
/// `0x0050d6c0` `(&DAT_00aa9cf8)[param_1 * 0x26] = 3`) vs `LightSetPoint` (`0x00126320`, `*__s = 2`),
/// patched by `Patches/LightingTransition.cpp:35-72`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpotLightTransitionMode {
    /// `2` = Point light (`LightSetPoint`, used for smooth lighting transitions in specific rooms).
    Point = 2,
    /// `3` = Spot light (`sh_Kari_LightSetSpot`, vanilla default).
    Spot = 3,
}

/// Evaluates `LightingTransitionASM` (`Patches/LightingTransition.cpp:35-72`, hooking
/// `sh2pc.exe` `0x0050d6f0` inside `FUN_0050d6c0`):
///
/// - In [`LightingProfile::Ps2Authentic`]: always returns [`SpotLightTransitionMode::Spot`] (`3`).
/// - In `EnhancedEdition` / `NativeEnhanced`:
///   - `R_MAN_S_HALL_2F` (`0xD0`): [`SpotLightTransitionMode::Point`] (`2`) for all objects.
///   - `R_APT_W_RM_203` (`0x27`), `R_HTL_W_HALL_2F` (`0xA0`), `R_HTL_ALT_EMPLOYEE_STAIRS` (`0xAB`),
///     `R_HTL_ALT_W_ROOM_HALL_2F` (`0xB1`), `R_HTL_ALT_W_HALL_2F` (`0xB2`),
///     `R_HTL_ALT_E_ROOM_HALL_2F` (`0xB4`): [`SpotLightTransitionMode::Point`] (`2`) for James,
///     [`SpotLightTransitionMode::Spot`] (`3`) for other objects.
#[must_use]
pub const fn lighting_transition_mode(
    room_id: u32,
    is_player_james: bool,
    profile: LightingProfile,
) -> SpotLightTransitionMode {
    if matches!(profile, LightingProfile::Ps2Authentic) {
        return SpotLightTransitionMode::Spot;
    }
    const R_APT_W_RM_203: u32 = 0x27;
    const R_HTL_W_HALL_2F: u32 = 0xA0;
    const R_HTL_ALT_EMPLOYEE_STAIRS: u32 = 0xAB;
    const R_HTL_ALT_W_ROOM_HALL_2F: u32 = 0xB1;
    const R_HTL_ALT_W_HALL_2F: u32 = 0xB2;
    const R_HTL_ALT_E_ROOM_HALL_2F: u32 = 0xB4;
    const R_MAN_S_HALL_2F: u32 = 0xD0;

    match room_id {
        R_MAN_S_HALL_2F => SpotLightTransitionMode::Point,
        R_APT_W_RM_203
        | R_HTL_W_HALL_2F
        | R_HTL_ALT_EMPLOYEE_STAIRS
        | R_HTL_ALT_W_ROOM_HALL_2F
        | R_HTL_ALT_W_HALL_2F
        | R_HTL_ALT_E_ROOM_HALL_2F => {
            if is_player_james {
                SpotLightTransitionMode::Point
            } else {
                SpotLightTransitionMode::Spot
            }
        }
        _ => SpotLightTransitionMode::Spot,
    }
}

/// Evaluates `RunLightingTransition` (`Patches/LightingTransition.cpp:186-200`):
/// disables flashlight rendering during `CS_END_MARIA_EPILOGUE` (`0x5D`) and
/// `CS_BOWL_LAURA_EDDIE` (`0x19`) when SH2:EE fixes are active.
#[must_use]
pub const fn should_disable_cutscene_flashlight(
    cutscene_id: u32,
    profile: LightingProfile,
) -> bool {
    if matches!(profile, LightingProfile::Ps2Authentic) {
        return false;
    }
    const CS_BOWL_LAURA_EDDIE: u32 = 0x19;
    const CS_END_MARIA_EPILOGUE: u32 = 0x5D;
    matches!(cutscene_id, CS_BOWL_LAURA_EDDIE | CS_END_MARIA_EPILOGUE)
}

/// Evaluates `CemeteryLightingASM` (`Patches/LightingPatch.cpp:33-36`, `sh2pc.exe` `0x0047C2DB`):
/// remaps `0x0001000E` to `0x0001000D` when SH2:EE lighting fixes are active.
#[must_use]
pub const fn cemetery_lighting_id(raw_id: u32, profile: LightingProfile) -> u32 {
    if !matches!(profile, LightingProfile::Ps2Authentic) && raw_id == 0x0001_000E {
        0x0001_000D
    } else {
        raw_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::draw_env::lookup_draw_env;

    #[test]
    fn ps2_authentic_leaves_draw_env_unmodified() {
        let ps = lookup_draw_env("PS", 0).expect("PS");
        let unchanged = apply_draw_env_fixes(ps, "ps189", LightingProfile::Ps2Authentic);
        assert_eq!(unchanged.vertex_ambient, ps.vertex_ambient);
        assert_eq!(unchanged.flashlight_range, ps.flashlight_range);
    }

    #[test]
    fn sh2ee_fixes_patch_catacombs_and_flashlight_range() {
        let ps = lookup_draw_env("PS", 0).expect("PS");
        let small = apply_draw_env_fixes(ps, "ps189", LightingProfile::EnhancedEdition);
        assert_eq!(small.vertex_ambient, [2.5, 2.0, 2.0]);
        assert_eq!(small.flashlight_range, 8500.0);

        let large = apply_draw_env_fixes(ps, "ps193", LightingProfile::EnhancedEdition);
        assert_eq!(large.vertex_ambient, [3.15, 2.8, 2.8]);
        assert_eq!(large.base_ambient, [0.0, -0.1, -0.1]);
    }

    #[test]
    fn shadow_fade_controller_ramps_over_33_frames_at_30hz() {
        let mut ctrl = ShadowFadeController::default();
        ctrl.update(true, 0.0);
        assert_eq!(ctrl.phase, ShadowFadePhase::FadingIn);
        assert_eq!(ctrl.intensity, 0.0);

        for _ in 0..33 {
            ctrl.update(true, 1.0 / 30.0);
        }
        assert_eq!(ctrl.phase, ShadowFadePhase::Steady);
        assert!((ctrl.intensity - 1.0).abs() < 1e-4);
    }

    #[test]
    fn enemy_reveal_and_halogen_room_checks() {
        assert!(is_halogen_fullbright_room("ap26"));
        assert!(is_halogen_fullbright_room("rr49"));
        assert!(!is_halogen_fullbright_room("er01"));

        assert_eq!(
            evaluate_enemy_reveal_lighting(
                Some(6),
                500.0,
                "cc01",
                true,
                LightingProfile::EnhancedEdition
            ),
            0.065
        );
        assert_eq!(
            evaluate_enemy_reveal_lighting(
                Some(6),
                706.0,
                "cc01",
                true,
                LightingProfile::EnhancedEdition
            ),
            0.7
        );
    }

    #[test]
    fn lighting_transition_cemetery_and_attic_fixes_respect_profile() {
        // LightingTransition.cpp: Point (2) for James in R_APT_W_RM_203, Spot (3) for others/vanilla
        assert_eq!(
            lighting_transition_mode(0x27, true, LightingProfile::Ps2Authentic),
            SpotLightTransitionMode::Spot
        );
        assert_eq!(
            lighting_transition_mode(0x27, true, LightingProfile::EnhancedEdition),
            SpotLightTransitionMode::Point
        );
        assert_eq!(
            lighting_transition_mode(0x27, false, LightingProfile::EnhancedEdition),
            SpotLightTransitionMode::Spot
        );
        assert_eq!(
            lighting_transition_mode(0xD0, false, LightingProfile::EnhancedEdition),
            SpotLightTransitionMode::Point
        );
        assert!(should_disable_cutscene_flashlight(
            0x19,
            LightingProfile::EnhancedEdition
        ));
        assert!(!should_disable_cutscene_flashlight(
            0x19,
            LightingProfile::Ps2Authentic
        ));

        // LightingPatch.cpp: CemeteryLightingASM
        assert_eq!(
            cemetery_lighting_id(0x0001_000E, LightingProfile::EnhancedEdition),
            0x0001_000D
        );
        assert_eq!(
            cemetery_lighting_id(0x0001_000E, LightingProfile::Ps2Authentic),
            0x0001_000E
        );

        // AtticShadowFixes.cpp: R_MAN_ATTIC = 0xC7
        assert_eq!(bfaw_attic_shadow_cutoff_y_gated(0xC7, -1450.0, true), -10.0);
        assert_eq!(bfaw_attic_shadow_cutoff_y_gated(0xC7, -1450.0, false), 10.0);
    }
}
