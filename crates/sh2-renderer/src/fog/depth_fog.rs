//! 1:1 port of `sh2gfw_Set_DepthFog` (`0x00175df0`) and `sh2gfw_Set_DepthFogFAFB` (`0x00175e70`)
//! from `GFW/sh2_set_drawenv.c` (`SLUS_202.28`).
//!
//! Computes exact perspective (`FA / z + FB`) and linear (`lin_fa * z + lin_fb`) depth-fog
//! coefficients bounded by `[fog_min, fog_max]`.

/// Default PS2 fog near distance in mm (`2000.0` mm = 2 m, fixing the previous `2.0` meter bug).
pub const PS2_DEFAULT_FOG_NEAR_MM: f32 = 2000.0;

/// Default PS2 fog far distance in mm (`25000.0` mm = 25 m, fixing the previous `25.0` meter bug).
pub const PS2_DEFAULT_FOG_FAR_MM: f32 = 25_000.0;

/// Default normalized minimum fog blend factor (`FogMin` = `0.0`, clear near camera).
pub const PS2_DEFAULT_FOG_MIN: f32 = 0.0;

/// Default normalized maximum fog blend factor (`FogMax` = `1.0`, full fog at far plane).
pub const PS2_DEFAULT_FOG_MAX: f32 = 1.0;

/// Exact coefficients produced by `sh2gfw_Set_DepthFog` (`SLUS_202.28` `0x00175df0`).
///
/// In `SLUS_202.28` `0x00175df0`:
/// - `param_1 = fog_far`, `param_2 = fog_near`, `param_3 = fog_max`, `param_4 = fog_min`
/// - `_DAT_00e4934c` (`persp_fa`) = `(fog_far * fog_near * (fog_min - fog_max)) / (fog_far - fog_near)`
/// - `_DAT_00e49348` (`persp_fb`) = `(fog_far * fog_max - fog_near * fog_min) / (fog_far - fog_near)`
/// - `_DAT_00e49340` (`lin_fa`)   = `(fog_min - fog_max) / (fog_near - fog_far)`
/// - `_DAT_00e49344` (`lin_fb`)   = `fog_min - fog_near * lin_fa`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthFogCoeffs {
    /// Perspective hyperbolic slope `FA` (`_DAT_00e4934c`): `fog(z) = clamp(persp_fa / z + persp_fb, min, max)`.
    pub persp_fa: f32,
    /// Perspective hyperbolic bias `FB` (`_DAT_00e49348`).
    pub persp_fb: f32,
    /// Linear slope (`_DAT_00e49340`): `fog(z) = clamp(lin_fa * z + lin_fb, min, max)`.
    pub lin_fa: f32,
    /// Linear intercept (`_DAT_00e49344`).
    pub lin_fb: f32,
    /// Lower fog bound (`sh2gfw_Set_FogMin` `0x00175de0`).
    pub fog_min: f32,
    /// Upper fog bound (`sh2gfw_Set_FogMax` `0x00175dd0`).
    pub fog_max: f32,
}

impl DepthFogCoeffs {
    /// Exact port of `sh2gfw_Set_DepthFog(fog_far, fog_near, fog_max, fog_min)` (`0x00175df0`).
    #[must_use]
    pub fn from_ps2_params(fog_far: f32, fog_near: f32, fog_max: f32, fog_min: f32) -> Self {
        let span = fog_far - fog_near;
        if span.abs() < 1e-4 {
            return Self {
                persp_fa: 0.0,
                persp_fb: fog_min,
                lin_fa: 0.0,
                lin_fb: fog_min,
                fog_min,
                fog_max,
            };
        }
        let persp_fa = (fog_far * fog_near * (fog_min - fog_max)) / span;
        let persp_fb = (fog_far * fog_max - fog_near * fog_min) / span;
        let lin_fa = (fog_min - fog_max) / (fog_near - fog_far);
        let lin_fb = fog_min - fog_near * lin_fa;
        Self {
            persp_fa,
            persp_fb,
            lin_fa,
            lin_fb,
            fog_min,
            fog_max,
        }
    }

    /// Evaluates the exact PS2 hyperbolic depth-fog equation `clamp(FA / z + FB, min, max)`.
    #[must_use]
    #[inline]
    pub fn eval_hyperbolic(&self, z_mm: f32) -> f32 {
        let lo = self.fog_min.min(self.fog_max);
        let hi = self.fog_min.max(self.fog_max);
        if z_mm <= 1e-4 {
            return self.fog_min.clamp(lo, hi);
        }
        (self.persp_fa / z_mm + self.persp_fb).clamp(lo, hi)
    }

    /// Evaluates the exact PS2 linear depth-fog equation `clamp(lin_fa * z + lin_fb, min, max)`.
    #[must_use]
    #[inline]
    pub fn eval_linear(&self, z_mm: f32) -> f32 {
        let lo = self.fog_min.min(self.fog_max);
        let hi = self.fog_min.max(self.fog_max);
        (self.lin_fa * z_mm + self.lin_fb).clamp(lo, hi)
    }

    /// Packs `[persp_fa, persp_fb, fog_min, fog_max]` for GPU uniform upload.
    #[must_use]
    #[inline]
    pub fn to_gpu_vec4(&self) -> [f32; 4] {
        [self.persp_fa, self.persp_fb, self.fog_min, self.fog_max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sh2gfw_set_depth_fog_exact_boundaries() {
        // Test normalized 0..1 fog_factor convention (fog_min = 0.0, fog_max = 1.0)
        let c = DepthFogCoeffs::from_ps2_params(25_000.0, 2000.0, 1.0, 0.0);
        assert!((c.eval_hyperbolic(2000.0) - 0.0).abs() < 1e-5);
        assert!((c.eval_hyperbolic(25_000.0) - 1.0).abs() < 1e-5);
        assert!((c.eval_linear(2000.0) - 0.0).abs() < 1e-5);
        assert!((c.eval_linear(25_000.0) - 1.0).abs() < 1e-5);
        // Hyperbolic fog rises faster than linear at intermediate depth (e.g. z = 5000 mm)
        let hyp_mid = c.eval_hyperbolic(5000.0);
        let lin_mid = c.eval_linear(5000.0);
        assert!(hyp_mid > lin_mid && hyp_mid < 1.0);

        // Test PS2 GS inverted F convention (fog_min = 255.0 at near, fog_max = 0.0 at far)
        let gs = DepthFogCoeffs::from_ps2_params(25_000.0, 2000.0, 0.0, 255.0);
        assert!((gs.eval_hyperbolic(2000.0) - 255.0).abs() < 1e-3);
        assert!((gs.eval_hyperbolic(25_000.0) - 0.0).abs() < 1e-3);
    }
}
