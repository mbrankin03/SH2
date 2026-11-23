//! 1:1 port of `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`, `Fog/fog.c`):
//! camera-coupled + wind-coupled screen-space fog UV scrolling and GS alpha scaling.

use glam::{Mat3, Vec3};

/// Camera displacement scale factor (`0.1 / area_size` in `fog_view_screen_fog` `0x0016c1c0`).
pub const SCREEN_FOG_CAM_SCALE: f32 = 0.1;

/// Wind velocity scale factor (`dt_scale * 0.005` in `fog_view_screen_fog` `0x0016c1c0`).
pub const SCREEN_FOG_WIND_SCALE: f32 = 0.005;

/// View-space UV scroll multiplier (`* 0.25` in `fog_view_screen_fog` `0x0016c1c0`).
pub const SCREEN_FOG_UV_SCROLL_SCALE: f32 = 0.25;

/// Half-span of the fullscreen fog quad UV window (`±0.125` in `fog_view_screen_fog` `0x0016c1c0`).
pub const SCREEN_FOG_UV_HALF_SPAN: f32 = 0.125;

/// Default PS2 fog wind speed magnitude (`0x3ecccccd` = `0.4f`) in `fogChangeWind`
/// (`SLUS_202.28` `0x00169380`, `uVar8 = 0x3ecccccd` for default `wind_type = 0` from
/// `fog_defset_data +0x10` at `0x002ab530`).
pub const PS2_FOG_WIND_DEFAULT_SPEED: f32 = 0.4;

/// Default PS2 horizontal wind vector at [`PS2_FOG_WIND_DEFAULT_SPEED`] (`fogChangeWind` `0x00169380`).
pub const PS2_DEFAULT_FOG_WIND_VEC: Vec3 = Vec3::new(PS2_FOG_WIND_DEFAULT_SPEED, 0.0, 0.0);

/// Default PS2 `fogSetColor` alpha byte (`0x80` = `128`) set by `fogInit` (`SLUS_202.28` `0x001687d0`).
pub const PS2_FOG_DEFAULT_COLOR_ALPHA_U8: u8 = 0x80;

/// 1:1 screen-space fog UV accumulator (`_DAT_00581500`, `_DAT_00581504` in `Fog/fog.c`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenFogState {
    /// Wrapped horizontal UV center in `[0.0, 1.0)` (`_DAT_00581500`).
    pub u: f32,
    /// Wrapped vertical UV center in `[0.0, 1.0)` (`_DAT_00581504`).
    pub v: f32,
}

impl Default for ScreenFogState {
    fn default() -> Self {
        Self { u: 0.5, v: 0.5 }
    }
}

/// Output quad UV rectangle and GS alpha from `fog_view_screen_fog` (`0x0016c1c0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenFogEvaluation {
    /// Top-left UV `(u - 0.125, v - 0.125)`.
    pub uv_min: [f32; 2],
    /// Bottom-right UV `(u + 0.125, v + 0.125)`.
    pub uv_max: [f32; 2],
    /// PS2 GS alpha byte `max(1, (fog_color_a * fog_alpha_param) >> 9)` (`0x0016c1c0` line 2699).
    pub gs_alpha_u8: u8,
}

/// Per-frame inputs to [`ScreenFogState::step`] (`fog_view_screen_fog` `0x0016c1c0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenFogStepInput {
    /// Camera displacement `cam_pos - prev_cam_pos` in PS2 mm (`_DAT_00581460 - _DAT_00581450`).
    pub cam_delta: Vec3,
    /// Active fog area radius in PS2 mm (`_DAT_00581514`, e.g. `2000.0`).
    pub area_size: f32,
    /// Current wind vector (`_DAT_005814b0`).
    pub wind_vec: Vec3,
    /// Frame delta scale (`_DAT_005814ec`).
    pub dt_scale: f32,
    /// 3×3 world-to-view rotation matrix (`_DAT_00581410..00581430`).
    pub world_view_rot: Mat3,
    /// `fogSetColor` alpha byte (`_DAT_0058153c >> 24`, default `0x80`).
    pub fog_color_a: u8,
    /// Environment alpha byte (`_DAT_0058152e`).
    pub fog_alpha_param: u8,
}

impl ScreenFogState {
    /// Steps one frame of `fog_view_screen_fog` (`0x0016c1c0`).
    pub fn step(&mut self, input: &ScreenFogStepInput) -> ScreenFogEvaluation {
        let cam_term = input.cam_delta * (SCREEN_FOG_CAM_SCALE / input.area_size.max(1.0));
        let wind_term = input.wind_vec * (input.dt_scale * SCREEN_FOG_WIND_SCALE);
        let view_delta = input.world_view_rot * (cam_term + wind_term);

        self.u -= view_delta.x * SCREEN_FOG_UV_SCROLL_SCALE;
        self.v -= view_delta.y * SCREEN_FOG_UV_SCROLL_SCALE;
        // Wrap to [0.0, 1.0) matching `x - floor(x)` in `0x0016c1c0` lines 2693-2694
        self.u = self.u.rem_euclid(1.0);
        self.v = self.v.rem_euclid(1.0);

        let raw_alpha =
            ((u32::from(input.fog_color_a) * u32::from(input.fog_alpha_param)) >> 9).max(1);
        let gs_alpha_u8 = raw_alpha.min(255) as u8;

        ScreenFogEvaluation {
            uv_min: [
                self.u - SCREEN_FOG_UV_HALF_SPAN,
                self.v - SCREEN_FOG_UV_HALF_SPAN,
            ],
            uv_max: [
                self.u + SCREEN_FOG_UV_HALF_SPAN,
                self.v + SCREEN_FOG_UV_HALF_SPAN,
            ],
            gs_alpha_u8,
        }
    }

    /// Packs `[u, v, half_span (0.125), normalized_alpha]` for GPU uniform upload.
    #[must_use]
    pub fn to_gpu_vec4(&self, eval: &ScreenFogEvaluation) -> [f32; 4] {
        [
            self.u,
            self.v,
            SCREEN_FOG_UV_HALF_SPAN,
            f32::from(eval.gs_alpha_u8) / 128.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fog_view_screen_fog_uv_wrap_and_alpha() {
        let mut st = ScreenFogState { u: 0.5, v: 0.5 };
        let eval = st.step(&ScreenFogStepInput {
            cam_delta: Vec3::new(2000.0, 0.0, 0.0),
            area_size: 2000.0,
            wind_vec: Vec3::ZERO,
            dt_scale: 1.0,
            world_view_rot: Mat3::IDENTITY,
            fog_color_a: 0x80,
            fog_alpha_param: 0x80,
        });
        // cam_term.x = 2000 * (0.1 / 2000) = 0.1; u = 0.5 - 0.1 * 0.25 = 0.475
        assert!((st.u - 0.475).abs() < 1e-6);
        assert!((eval.uv_min[0] - 0.35).abs() < 1e-6);
        assert!((eval.uv_max[0] - 0.60).abs() < 1e-6);
        // (128 * 128) >> 9 = 16384 >> 9 = 32
        assert_eq!(eval.gs_alpha_u8, 32);
    }
}
