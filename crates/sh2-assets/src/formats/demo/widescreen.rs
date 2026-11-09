//! Aspect-aware cutscene field-of-view (FOV), widescreen framing modes (`CutsceneWidescreenMode`),
//! and 4:3 letterbox/pillarbox border policies (`CutsceneBorderMode`) (`SH2-77`).
//!
//! # Ground Truth & SH2:EE Reference
//! - **Native PS2/PC `DdsPlay` (`0x4a176b` in `sh2pc.exe`, `0x1b23c0` in `SLUS_202.28`)**:
//!   - Authored for a `4:3` (`640x448` PS2 / `640x480` PC) frame:
//!     $$\text{scr\_z} = \text{plane} \times 1.14702 \times \text{demo\_fov\_scale}$$
//!     $$\text{fov\_y} = 2 \arctan\left(\frac{224.0}{\text{scr\_z}}\right)$$
//! - **SH2:EE `ReduceCutsceneFOV` (`Patches/Resolution.cpp`, `Patches/Common.cpp`)**:
//!   - Multiplies `scr_z` by `1.125` at `16:9` to zoom in slightly and hide off-screen actors parked
//!     just outside the `4:3` edge, clamping the zoom factor at `16:9` so `20:9`, `21:9`, and `32:9`
//!     ultrawide displays never suffer vertical cropping.
//! - **Modern Native Hor+ with Smart 4:3 Frustum Culling (`UltrawideHorPlusSmartCull`)**:
//!   - Preserves the full authored vertical FOV (`scr_z` uncropped) across `16:9`, `21:9`, and `32:9`
//!     while computing the exact authored `4:3` horizontal frustum cone (`fov_x_4_3`) so stationary
//!     actors parked outside the `4:3` frame are automatically culled in the widescreen side margins.

use super::{DEMO_SCREEN_SY, DEMO_SCR_Z_SCALE};

/// Original authored 4:3 aspect ratio (`4.0 / 3.0 = 1.3333334`).
pub const AUTHORED_4X3_ASPECT: f32 = 4.0 / 3.0;

/// Standard 16:9 widescreen aspect ratio (`16.0 / 9.0 = 1.7777778`).
pub const STANDARD_16X9_ASPECT: f32 = 16.0 / 9.0;

/// Standard 21:9 ultrawide aspect ratio (`64.0 / 27.0 = 2.3703704`, e.g. `2560x1080` / `3440x1440`).
pub const ULTRAWIDE_21X9_ASPECT: f32 = 64.0 / 27.0;

/// Super-ultrawide 32:9 aspect ratio (`32.0 / 9.0 = 3.5555556`, e.g. `5120x1440`).
pub const SUPER_ULTRAWIDE_32X9_ASPECT: f32 = 32.0 / 9.0;

/// SH2:EE `ReduceCutsceneFOV` maximum focal length zoom multiplier at `>= 16:9` (`1.125`).
pub const SH2EE_16X9_SCR_Z_ZOOM: f32 = 1.125;

/// Widescreen framing and off-screen actor culling mode for in-engine 3D cutscenes (`SH2-77`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutsceneWidescreenMode {
    /// **Default**: Native Hor+ widescreen/ultrawide framing (`scr_z = plane * 1.14702 * demo_fov_scale`)
    /// preserving full vertical height across `16:9`, `21:9`, and `32:9`, paired with smart off-screen
    /// `4:3` parked actor frustum culling.
    #[default]
    UltrawideHorPlusSmartCull,
    /// Smoothly scales `scr_z` as a function of `viewport_aspect.clamp(4.0 / 3.0, 16.0 / 9.0)`
    /// (`1.0` at `4:3` up to `1.125` at `>= 16:9`, matching SH2:EE `ReduceCutsceneFOV` without
    /// over-cropping `20:9`, `21:9`, or `32:9` displays).
    Sh2eeZoom16x9,
    /// Renders cutscenes inside an authentic `4:3` pillarboxed sub-viewport with black side bars.
    Pillarbox4x3,
}

impl CutsceneWidescreenMode {
    /// Cycles to the next [`CutsceneWidescreenMode`] variant for interactive HUD/options toggling.
    #[inline]
    pub const fn next(self) -> Self {
        match self {
            Self::UltrawideHorPlusSmartCull => Self::Sh2eeZoom16x9,
            Self::Sh2eeZoom16x9 => Self::Pillarbox4x3,
            Self::Pillarbox4x3 => Self::UltrawideHorPlusSmartCull,
        }
    }

    /// Short human-readable label for debug HUDs and menus.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::UltrawideHorPlusSmartCull => "Hor+ + Smart 4:3 Cull",
            Self::Sh2eeZoom16x9 => "SH2:EE 16:9 Zoom (1.125x)",
            Self::Pillarbox4x3 => "Authentic 4:3 Pillarbox",
        }
    }

    /// Returns `true` if off-screen stationary `4:3` actor frustum culling is active in this mode.
    #[inline]
    pub const fn uses_smart_offscreen_culling(self) -> bool {
        matches!(self, Self::UltrawideHorPlusSmartCull | Self::Sh2eeZoom16x9)
    }
}

/// Policy for top/bottom cutscene letterbox bars (`DisableCutsceneBorders` in `WidescreenFixesPack.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutsceneBorderMode {
    /// **Default (`Auto`)**: Automatically hides top/bottom `4:3` letterbox bars when `viewport_aspect > 4.0 / 3.0`
    /// so widescreen/ultrawide viewports are not double-cropped vertically, while preserving borders on `4:3`.
    #[default]
    Auto,
    /// Always hide cutscene letterbox borders regardless of aspect ratio.
    Disabled,
    /// Always render original cutscene letterbox borders (`f0 = 0.395, f1 = 0.605`).
    Enabled,
}

impl CutsceneBorderMode {
    /// Returns `true` if top/bottom letterbox bars should be hidden for the given `viewport_aspect` and `widescreen_mode`.
    #[inline]
    pub fn should_hide_letterbox_bars(
        self,
        viewport_aspect: f32,
        widescreen_mode: CutsceneWidescreenMode,
    ) -> bool {
        match self {
            Self::Disabled => true,
            Self::Enabled => false,
            Self::Auto => {
                widescreen_mode != CutsceneWidescreenMode::Pillarbox4x3
                    && viewport_aspect > AUTHORED_4X3_ASPECT + 1e-3
            }
        }
    }
}

/// Computes the aspect-aware projection focal distance `scr_z` for `camera_plane`, `demo_fov_scale`,
/// `viewport_aspect`, and [`CutsceneWidescreenMode`].
#[inline]
pub fn compute_widescreen_scr_z(
    camera_plane: f32,
    demo_fov_scale: f32,
    viewport_aspect: f32,
    mode: CutsceneWidescreenMode,
) -> f32 {
    let scale = if demo_fov_scale > 0.0 {
        demo_fov_scale
    } else {
        1.0
    };
    let mut scr_z = camera_plane * DEMO_SCR_Z_SCALE * scale;
    if mode == CutsceneWidescreenMode::Sh2eeZoom16x9 {
        let clamped_aspect = viewport_aspect.clamp(AUTHORED_4X3_ASPECT, STANDARD_16X9_ASPECT);
        let t =
            (clamped_aspect - AUTHORED_4X3_ASPECT) / (STANDARD_16X9_ASPECT - AUTHORED_4X3_ASPECT);
        let zoom = 1.0 + (SH2EE_16X9_SCR_Z_ZOOM - 1.0) * t.clamp(0.0, 1.0);
        scr_z *= zoom;
    }
    if scr_z < 50.0 {
        scr_z = DEMO_SCREEN_SY;
    }
    scr_z
}

/// Computes the authored `4:3` vertical and horizontal field-of-view in radians `(fov_y_4x3, fov_x_4x3)`
/// for a given `camera_plane` and `demo_fov_scale`.
///
/// Used by smart off-screen actor frustum culling to determine whether an actor lies inside the
/// original `4:3` camera cone (`fov_x_4_3 = 2 * atan(tan(fov_y_4_3 / 2) * (4.0 / 3.0))`).
#[inline]
pub fn authored_4x3_fov_radians(camera_plane: f32, demo_fov_scale: f32) -> (f32, f32) {
    let scr_z = compute_widescreen_scr_z(
        camera_plane,
        demo_fov_scale,
        AUTHORED_4X3_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    let half_tan_y = DEMO_SCREEN_SY * 0.5 / scr_z;
    let fov_y = 2.0 * half_tan_y.atan();
    let fov_x = 2.0 * (half_tan_y * AUTHORED_4X3_ASPECT).atan();
    (fov_y, fov_x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widescreen_scr_z_and_fov_across_aspects() {
        let plane = 300.0;
        let scale = 1.0;

        // 1. UltrawideHorPlusSmartCull preserves vertical scr_z across 4:3, 16:9, 20:9, 21:9, 32:9
        let z_4_3 = compute_widescreen_scr_z(
            plane,
            scale,
            AUTHORED_4X3_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        let z_16_9 = compute_widescreen_scr_z(
            plane,
            scale,
            STANDARD_16X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        let z_21_9 = compute_widescreen_scr_z(
            plane,
            scale,
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        assert!((z_4_3 - z_16_9).abs() < 1e-5);
        assert!((z_4_3 - z_21_9).abs() < 1e-5);

        // 2. Sh2eeZoom16x9 scales smoothly from 1.0x at 4:3 to 1.125x at 16:9 and clamps at >= 16:9
        let z_zoom_4_3 = compute_widescreen_scr_z(
            plane,
            scale,
            AUTHORED_4X3_ASPECT,
            CutsceneWidescreenMode::Sh2eeZoom16x9,
        );
        let z_zoom_16_10 =
            compute_widescreen_scr_z(plane, scale, 1.6, CutsceneWidescreenMode::Sh2eeZoom16x9);
        let z_zoom_16_9 = compute_widescreen_scr_z(
            plane,
            scale,
            STANDARD_16X9_ASPECT,
            CutsceneWidescreenMode::Sh2eeZoom16x9,
        );
        let z_zoom_20_9 = compute_widescreen_scr_z(
            plane,
            scale,
            20.0 / 9.0,
            CutsceneWidescreenMode::Sh2eeZoom16x9,
        );
        let z_zoom_21_9 = compute_widescreen_scr_z(
            plane,
            scale,
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::Sh2eeZoom16x9,
        );

        assert!((z_zoom_4_3 - z_4_3).abs() < 1e-5);
        assert!(z_zoom_16_10 > z_zoom_4_3 && z_zoom_16_10 < z_zoom_16_9);
        assert!((z_zoom_16_9 - z_4_3 * 1.125).abs() < 1e-4);
        // 20:9 and 21:9 must clamp to the 16:9 zoom factor so ultrawide monitors aren't over-cropped
        assert!((z_zoom_20_9 - z_zoom_16_9).abs() < 1e-5);
        assert!((z_zoom_21_9 - z_zoom_16_9).abs() < 1e-5);

        // 3. Authored 4:3 horizontal FOV is wider than vertical FOV by atan(tan(fov_y/2) * 4/3)
        let (fov_y, fov_x) = authored_4x3_fov_radians(plane, scale);
        assert!(fov_x > fov_y);
        let expected_fov_x = 2.0 * ((fov_y * 0.5).tan() * (4.0 / 3.0)).atan();
        assert!((fov_x - expected_fov_x).abs() < 1e-6);
    }

    #[test]
    fn test_cutscene_border_mode_auto() {
        assert!(!CutsceneBorderMode::Auto.should_hide_letterbox_bars(
            AUTHORED_4X3_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull
        ));
        assert!(CutsceneBorderMode::Auto.should_hide_letterbox_bars(
            STANDARD_16X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull
        ));
        assert!(CutsceneBorderMode::Auto.should_hide_letterbox_bars(
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::Sh2eeZoom16x9
        ));
        assert!(!CutsceneBorderMode::Auto.should_hide_letterbox_bars(
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::Pillarbox4x3
        ));
    }
}
