//! Pure PS2 GS register encodings, dual shadow composite blend math, and blur uniform types
//! (`SLUS_202.28` `GFW/sh2_shadow_filters.c`).

use bytemuck::{Pod, Zeroable};

/// PS2 GS `ALPHA_1` register value for Day / subtractive shadow compositing
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 == 0`).
///
/// Bit decode (`(A - B) * C >> 7 + D`): `A = 2 (0)`, `B = 0 (Cs)`, `C = 1 (Ad)`, `D = 1 (Cd)`,
/// `FIX = 0x80` -> `Cd - Cs * Ad`.
pub const GS_SHADOW_ALPHA_SUBTRACTIVE: u64 = 0x0000_0080_0000_0052;

/// PS2 GS `TEST_1` register value paired with [`GS_SHADOW_ALPHA_SUBTRACTIVE`]
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 == 0`).
pub const GS_SHADOW_TEST_SUBTRACTIVE: u64 = 0x0003_0000;

/// PS2 GS `ALPHA_1` register value for Night / multiplicative shadow compositing
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 != 0`).
///
/// Bit decode (`(A - B) * C >> 7 + D`): `A = 2 (0)`, `B = 1 (Cd)`, `C = 0 (As)`, `D = 1 (Cd)`,
/// `FIX = 0x80` -> `Cd * (1 - As)`.
pub const GS_SHADOW_ALPHA_MULTIPLICATIVE: u64 = 0x0000_0080_0000_0046;

/// PS2 GS `TEST_1` register value paired with [`GS_SHADOW_ALPHA_MULTIPLICATIVE`]
/// (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`, `param_1 != 0`).
pub const GS_SHADOW_TEST_MULTIPLICATIVE: u64 = 0x0003_0002;

/// Number of bilinear filter passes executed by `shdw_shadow_filter_main` (`0x001767e0`):
/// 1× `shdw_StencilBuf2Work` (`0x00176550`) + 3× `shdw_Work2Work` (`0x001766a0`).
pub const PS2_SHADOW_BLUR_PASS_COUNT: usize = 4;

/// PS2 shadow filter work buffer dimension in pixels (`0x8800 - 0x7800 = 0x1000` in 12.4 fixed-point = `256` px;
/// `SLUS_202.28` `shdw_StencilBuf2Work` `0x00176550`, `shdw_Work2Work` `0x001766a0`).
pub const PS2_SHADOW_WORK_SIZE: u32 = 256;

/// Default PS2 room shadow density (`0x50 = 80`) returned by `Get_NowRoomShadowColor` (`0x00250ee0`)
/// when the room's `DrawEnvData` field at `+0x0a` is `-1`.
pub const PS2_DEFAULT_ROOM_SHADOW_COLOR: u8 = 0x50;

/// PS2 GS neutral modulation scale (`0x80 = 128` represents `1.0` in GS RGBAQ modulation;
/// `SLUS_202.28` `shdw_clear` `0x00176440`, `shdw_Work2Work` `0x001766a0`).
pub const PS2_GS_NEUTRAL_RGB: f32 = 128.0;

/// Dual PS2 GS stencil shadow composite blend modes (`SLUS_202.28` `sh2gfw_Set_FilterData` `0x00175f90`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowCompositeMode {
    /// `param_1 == 0`: `ALPHA_1 = 0x8000000052`, `TEST_1 = 0x30000` (`Cd - Cs * Ad`).
    Subtractive,
    /// `param_1 != 0`: `ALPHA_1 = 0x8000000046`, `TEST_1 = 0x30002` (`Cd * (1 - As)`).
    Multiplicative,
}

impl ShadowCompositeMode {
    /// Selects the GS shadow composite mode from `sh2gfw_Set_FilterData(param_1)` (`0x00175f90`).
    #[must_use]
    pub const fn from_filter_param(param_1: i32) -> Self {
        if param_1 == 0 {
            Self::Subtractive
        } else {
            Self::Multiplicative
        }
    }

    /// Returns the exact 64-bit PS2 GS `ALPHA_1` (`0x42`) register value (`0x00175f90`).
    #[must_use]
    pub const fn gs_alpha_1(self) -> u64 {
        match self {
            Self::Subtractive => GS_SHADOW_ALPHA_SUBTRACTIVE,
            Self::Multiplicative => GS_SHADOW_ALPHA_MULTIPLICATIVE,
        }
    }

    /// Returns the exact 64-bit PS2 GS `TEST_1` (`0x47`) register value (`0x00175f90`).
    #[must_use]
    pub const fn gs_test_1(self) -> u64 {
        match self {
            Self::Subtractive => GS_SHADOW_TEST_SUBTRACTIVE,
            Self::Multiplicative => GS_SHADOW_TEST_MULTIPLICATIVE,
        }
    }

    /// Evaluates the exact PS2 GS composite equation for a single RGB pixel (`dst_rgb`),
    /// blurred shadow mask coverage (`mask` in `0.0..=1.0`), and room shadow color (`shadow_rgb`,
    /// `shadow_alpha` in `0.0..=1.0` where `0x80 / 128.0 = 1.0`).
    #[must_use]
    pub fn composite_rgb(
        self,
        dst_rgb: [f32; 3],
        dst_alpha: f32,
        shadow_rgb: [f32; 3],
        shadow_alpha: f32,
        mask: f32,
    ) -> [f32; 3] {
        let m = mask.clamp(0.0, 1.0);
        match self {
            // ALPHA_1 = 0x8000000052: (0 - Cs) * Ad + Cd = Cd - Cs * Ad
            Self::Subtractive => {
                let ad = dst_alpha.clamp(0.0, 1.0);
                [
                    (dst_rgb[0] - shadow_rgb[0] * m * ad).max(0.0),
                    (dst_rgb[1] - shadow_rgb[1] * m * ad).max(0.0),
                    (dst_rgb[2] - shadow_rgb[2] * m * ad).max(0.0),
                ]
            }
            // ALPHA_1 = 0x8000000046: (0 - Cd) * As + Cd = Cd * (1 - As)
            Self::Multiplicative => {
                let factor = (1.0 - (shadow_alpha * m).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                [
                    dst_rgb[0] * factor,
                    dst_rgb[1] * factor,
                    dst_rgb[2] * factor,
                ]
            }
        }
    }

    /// Returns the `wgpu::BlendState` implementing this GS `ALPHA_1` composite mode.
    #[must_use]
    pub const fn wgpu_blend_state(self) -> wgpu::BlendState {
        match self {
            // ALPHA_1 = 0x8000000052: (0 - Cs) * Ad + Cd = Cd - Cs * Ad
            Self::Subtractive => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::DstAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::ReverseSubtract,
                },
                alpha: wgpu::BlendComponent::OVER,
            },
            // Cd * (1 - As)
            Self::Multiplicative => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            },
        }
    }
}

/// Evaluates `Get_ShadowColor` (`0x00176040`) with `sh2gfw_Set_ShadowColorScaleFactor` (`0x00176130`).
///
/// On PS2 (`0x00176040`), the shadow RGB channels all take `(u32)(base_r * scale)` and the alpha
/// channel takes `(u32)(base_a * scale)`.
#[must_use]
pub fn evaluate_ps2_shadow_color(base_r: u32, base_a: u32, scale_factor: f32) -> [u32; 4] {
    let rgb = ((base_r as f32) * scale_factor).max(0.0) as u32;
    let a = ((base_a as f32) * scale_factor).max(0.0) as u32;
    [rgb, rgb, rgb, a]
}

/// Uniform block for a single `ShadowBlurPipeline` pass (`16` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct ShadowBlurUniforms {
    /// `(texel_offset_x, texel_offset_y, shadow_opacity, uv_flip_sign)`.
    ///
    /// `uv_flip_sign` alternates per pass (`+1.0` / `-1.0`) matching `shdw_Work2Work` (`0x001766a0`)
    /// so the bilinear half-texel offset blurs symmetrically without net shadow translation.
    pub params: [f32; 4],
}

impl Default for ShadowBlurUniforms {
    fn default() -> Self {
        Self {
            params: [
                0.5 / (PS2_SHADOW_WORK_SIZE as f32),
                0.5 / (PS2_SHADOW_WORK_SIZE as f32),
                (PS2_DEFAULT_ROOM_SHADOW_COLOR as f32) / PS2_GS_NEUTRAL_RGB,
                1.0,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gs_composite_modes_match_slus_00175f90_registers() {
        assert_eq!(
            ShadowCompositeMode::from_filter_param(0),
            ShadowCompositeMode::Subtractive
        );
        assert_eq!(
            ShadowCompositeMode::Subtractive.gs_alpha_1(),
            0x0000_0080_0000_0052
        );
        assert_eq!(ShadowCompositeMode::Subtractive.gs_test_1(), 0x0003_0000);

        assert_eq!(
            ShadowCompositeMode::from_filter_param(1),
            ShadowCompositeMode::Multiplicative
        );
        assert_eq!(
            ShadowCompositeMode::Multiplicative.gs_alpha_1(),
            0x0000_0080_0000_0046
        );
        assert_eq!(ShadowCompositeMode::Multiplicative.gs_test_1(), 0x0003_0002);
    }

    #[test]
    fn golden_vector_dual_composite_equations() {
        let dst = [0.8, 0.6, 0.4];
        let shadow_rgb = [0.25, 0.25, 0.25];
        let shadow_alpha = 80.0 / 128.0; // 0x50 / 0x80 = 0.625

        // Mode 0 (`0x8000000052`): Cd - Cs * Ad
        let sub =
            ShadowCompositeMode::Subtractive.composite_rgb(dst, 1.0, shadow_rgb, shadow_alpha, 0.5);
        assert!((sub[0] - (0.8 - 0.125)).abs() < 1e-6);
        assert!((sub[1] - (0.6 - 0.125)).abs() < 1e-6);
        assert!((sub[2] - (0.4 - 0.125)).abs() < 1e-6);

        // Mode 1 (`0x8000000046`): Cd * (1 - As)
        let mul = ShadowCompositeMode::Multiplicative.composite_rgb(
            dst,
            1.0,
            shadow_rgb,
            shadow_alpha,
            0.5,
        );
        let expected_factor = 1.0 - 0.625 * 0.5;
        assert!((mul[0] - 0.8 * expected_factor).abs() < 1e-6);
        assert!((mul[1] - 0.6 * expected_factor).abs() < 1e-6);
        assert!((mul[2] - 0.4 * expected_factor).abs() < 1e-6);
    }

    #[test]
    fn get_shadow_color_matches_slus_00176040() {
        assert_eq!(evaluate_ps2_shadow_color(0x50, 0x50, 1.0), [80, 80, 80, 80]);
        assert_eq!(evaluate_ps2_shadow_color(0x80, 0x40, 0.5), [64, 64, 64, 32]);
    }

    #[test]
    fn blur_uniforms_alignment() {
        assert_eq!(std::mem::size_of::<ShadowBlurUniforms>(), 16);
        assert_eq!(PS2_SHADOW_BLUR_PASS_COUNT, 4);
        assert_eq!(PS2_SHADOW_WORK_SIZE, 256);
    }
}
