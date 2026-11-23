//! GPU uniform layout and 1:1 PS2 linear/hyperbolic attenuation math for multi-light
//! world and character rendering (`GFW/sh2gfw_LightSet.c`, `Chacter_Draw/light_n.c`).

use super::jms_spot::DARK_HOSEI;

/// Default PS2 flashlight height offset above floor in PS2 mm (Y-down: `-1500.0` mm = 1.5 m up).
///
/// Derived from James's chest-pocket flashlight mount height (~1.5 m in PS2 millimeter world units,
/// replacing the previous meter-unit `1.5` bug).
pub const PS2_DEFAULT_FLASHLIGHT_POS_Y_MM: f32 = -1500.0;

/// Default PS2 flashlight linear falloff near knee in PS2 mm (`sh2gfw_ALLInit_SpotMan` `0x0028e0b0`,
/// `DAT_0037b540 = 0x44bb8000 = 1500.0f` mm).
pub const PS2_DEFAULT_FLASHLIGHT_NEAR_MM: f32 = 1500.0;

/// Default PS2 flashlight linear falloff far cutoff in PS2 mm (`EnhancedFlashlight.cpp:162`,
/// PS2 retail spotlight range `6000.0` mm).
pub const PS2_DEFAULT_FLASHLIGHT_FAR_MM: f32 = 6000.0;

/// Active PS2 VU1 background lighting microcode mode (`sh2gfw_LightSet_ForBG` @ `0x0017daf0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum WorldLightMode {
    /// Daytime / lit-room 3-parallel directional lights (`sh2gfw_setparal3LGT_blk` @ `0x0017d7f0`).
    Parallel3 = 0,
    /// Night / dark-room 1-spotlight + 2-parallel directional lights (`sh2gfw_setsp1para2LGT_blk` @ `0x0017ddd0`).
    Spot1Parallel2 = 1,
}

/// CPU-side parallel directional light descriptor for `sh2gfw_setparal3LGT_blk` (`0x0017d7f0`)
/// and `sh2gfw_setsp1para2LGT_blk` (`0x0017ddd0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParallelLightParams {
    /// Unit light direction vector in PS2 world space.
    pub direction: [f32; 3],
    /// Linear RGB color.
    pub color: [f32; 3],
    /// Whether this parallel slot is active.
    pub enabled: bool,
}

impl Default for ParallelLightParams {
    fn default() -> Self {
        Self {
            // Default light direction from `SLUS_202.28` `0x002a94d0` (`[0.0, 0.0, 1.0, 0.0]`)
            direction: [0.0, 0.0, 1.0],
            color: [0.0, 0.0, 0.0],
            enabled: false,
        }
    }
}

/// CPU-side cutscene/demo point light descriptor (`sh2gfw_Set_DemoPointLight` @ `0x0017d600`,
/// 3 slots stored at `0x00e1cb80`, stride `0x30`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoPointLightParams {
    /// World-space position in PS2 mm (`param_4` -> `0x00e1cb90 + slot * 0x30`).
    pub position: [f32; 3],
    /// Linear RGB color (`param_5` -> `0x00e1cb80 + slot * 0x30`).
    pub color: [f32; 3],
    /// Near attenuation distance in PS2 mm (`param_1` -> `0x00e1cba0 + slot * 0x30`).
    pub near_mm: f32,
    /// Far attenuation cutoff in PS2 mm (`param_2` -> `0x00e1cba4 + slot * 0x30`).
    pub far_mm: f32,
    /// Active flag (`DAT_00e1ca8c + slot * 2 != 0`).
    pub enabled: bool,
}

impl Default for DemoPointLightParams {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0],
            near_mm: 0.0,
            far_mm: 0.0,
            enabled: false,
        }
    }
}

/// Computes exact PS2 linear and hyperbolic distance-attenuation coefficients from `(near_mm, far_mm)`.
///
/// Exact equations from `Chacter_Draw/light_n.c` `UpdateFParam` (`SLUS_202.28` `0x00126170`) and
/// `GFW/sh2gfw_LightSet.c` `sh2gfw_setsp1para2LGT_blk` (`SLUS_202.28` `0x0017ddd0` offsets `+0xc00`, `+0xbf0`):
/// - `linear_fa = 1.0 / (near - far)` (`+0x80` / `+0xc00`)
/// - `linear_fb = -linear_fa * far` (`+0x84` / `+0xbf0`)
/// - `hyper_fb  = near / (near - far)` (`+0x94`)
/// - `hyper_fa  = -hyper_fb * far` (`+0x90`)
///
/// Returns `[linear_fa, linear_fb, hyper_fa, hyper_fb]`.
/// When `|near - far| < 1e-4`, returns `[0.0, 1.0, 0.0, 1.0]` (no distance attenuation).
#[must_use]
pub fn compute_ps2_falloff_params(near_mm: f32, far_mm: f32) -> [f32; 4] {
    let denom = near_mm - far_mm;
    if denom.abs() < 1e-4 {
        return [0.0, 1.0, 0.0, 1.0];
    }
    let linear_fa = 1.0 / denom;
    let linear_fb = -(linear_fa * far_mm);
    let hyper_fb = near_mm / denom;
    let hyper_fa = -hyper_fb * far_mm;
    [linear_fa, linear_fb, hyper_fa, hyper_fb]
}

/// Evaluates exact PS2 linear distance attenuation (`sh2gfw_setsp1para2LGT_blk` `0x0017ddd0` /
/// `CalcInfluence` `0x001269b0`): `clamp(linear_fa * dist_mm + linear_fb, 0.0, 1.0)`.
///
/// Returns `1.0` for `dist_mm <= near_mm` and `0.0` for `dist_mm >= far_mm`.
#[must_use]
#[inline]
pub fn eval_linear_falloff(dist_mm: f32, near_mm: f32, far_mm: f32) -> f32 {
    let [fa, fb, _, _] = compute_ps2_falloff_params(near_mm, far_mm);
    (fa * dist_mm + fb).clamp(0.0, 1.0)
}

/// Evaluates exact PS2 hyperbolic distance attenuation (`UpdateFParam` `0x00126170` `+0x90`, `+0x94`):
/// `clamp(hyper_fa / dist_mm + hyper_fb, 0.0, 1.0)`.
///
/// Returns `1.0` for `dist_mm <= near_mm` and `0.0` for `dist_mm >= far_mm`.
#[must_use]
#[inline]
pub fn eval_hyperbolic_falloff(dist_mm: f32, near_mm: f32, far_mm: f32) -> f32 {
    if dist_mm <= near_mm {
        return 1.0;
    }
    let [_, _, h_fa, h_fb] = compute_ps2_falloff_params(near_mm, far_mm);
    (h_fa / dist_mm.max(1e-4) + h_fb).clamp(0.0, 1.0)
}

/// Multi-light world and character uniform block matching `GFW/sh2gfw_LightSet.c`
/// (`sh2gfw_setparal3LGT_blk` `0x0017d7f0`, `sh2gfw_setsp1para2LGT_blk` `0x0017ddd0`,
/// `sh2gfw_Set_DemoPointLight` `0x0017d600`) and `Chacter_Draw/light_n.c` (`UpdateReflections` `0x00126f90`).
///
/// Strictly 16-byte aligned for WGSL uniform buffer compliance (21 × `vec4<f32>` = **336 bytes**).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpotlightUniforms {
    /// Primary spotlight position in PS2 world mm (`xyz`) and mode flag (`w`: `0.0` = `Parallel3`, `1.0` = `Spot1Parallel2`).
    pub position: [f32; 4],
    /// Primary spotlight direction normalized (`xyz`) and inner cone cosine (`w`, or `-1.0` for parallel).
    pub direction_inner: [f32; 4],
    /// Primary spotlight RGB (`xyz`) and outer cone cosine (`w`, matching `sh2gfw_setsp1para2LGT_blk` `+0xbf4`).
    pub color_outer: [f32; 4],
    /// Room/cutscene world ambient tint RGB (`xyz`) and combined ambient intensity scale (`w`).
    pub ambient_tint: [f32; 4],
    /// Camera view-space directional fill & range calibration:
    /// - `x`: `camera_fill_k = 0.3 * s_flash_level * 0.25` (`sh2pc.exe` `0x50b036`, `0x89ffd8`)
    /// - `y`: flashlight far cutoff in PS2 mm (`6000.0` PS2 / `8500.0` SH2:EE)
    /// - `z`: active `s_flash_level` (`0.0`, `0.3`, `0.75`, `1.0`, `sh2pc.exe` `0x89ffc0`)
    /// - `w`: hyperbolic falloff blend (`0.0` = linear `0x0017ddd0`, `1.0` = hyperbolic `UpdateFParam` `0x00126170`)
    pub fill_and_range: [f32; 4],
    /// 3 parallel directional light directions (`xyz`, unit world-space) and enabled flag (`w`: `1.0` = active).
    ///
    /// In `Parallel3` mode (`0x0017d7f0`), all 3 slots (`0..3`) are evaluated.
    /// In `Spot1Parallel2` mode (`0x0017ddd0`), slots `1` and `2` provide the 2 auxiliary parallel lights.
    pub parallel_dirs: [[f32; 4]; 3],
    /// 3 parallel directional light RGB colors (`xyz`, `w` reserved `0.0`).
    pub parallel_colors: [[f32; 4]; 3],
    /// 3 demo/room point light positions (`xyz` in PS2 mm) and `near_mm` (`w`) (`sh2gfw_Set_DemoPointLight` `0x0017d600`).
    pub point_positions: [[f32; 4]; 3],
    /// 3 demo/room point light RGB colors (`xyz`) and `far_mm` (`w`; `<= 0.0` disables slot).
    pub point_colors: [[f32; 4]; 3],
    /// Exact PS2 distance-falloff coefficients `[linear_fa, linear_fb, hyper_fa, hyper_fb]`
    /// from `sh2gfw_setsp1para2LGT_blk` (`0x0017ddd0` `+0xc00`/`+0xbf0`) and `UpdateFParam` (`0x00126170`).
    pub ps2_falloff_params: [f32; 4],
    /// Gun muzzle flash ambient boost & `DARK_Hosei` trim:
    /// - `x`: `gun_light_boost = (sh2gfw_Get_GunLight() * 2.0) / 4.0 = gun_light * 0.5` in 1.0-normalized
    ///   ambient space (factoring out the `4.0` VU1 ambient scale in `0x0017d7f0` / `0x0017ddd0`)
    /// - `y`: `dark_trim_factor` (`sh2gfw_Get_DarkTrimFactor` `0x0028e350`)
    /// - `z`: `DARK_Hosei` (`0.5` @ `0x002ab710`)
    /// - `w`: `night_or_day` (`0.0` = Day `ALPHA_1=0x54`, `1.0` = Night `ALPHA_1=0x58`, `0x00181be0`)
    pub gun_and_dark_trim: [f32; 4],
    /// Strongest reflection light direction (`xyz`) & `reflection_brightness` (`w`) (`UpdateReflections` `0x00126f90`).
    pub reflection_dir: [f32; 4],
    /// Reflection specular color RGB (`xyz`, `LightGetReflectionColor` `0x00127780`) and
    /// `MakeEnvironPacket` (`0x0012cf20` `+0x45`) environment alpha (`w` in `0.0..=1.0`).
    pub reflection_color: [f32; 4],
}

impl Default for SpotlightUniforms {
    fn default() -> Self {
        let falloff = compute_ps2_falloff_params(
            PS2_DEFAULT_FLASHLIGHT_NEAR_MM,
            PS2_DEFAULT_FLASHLIGHT_FAR_MM,
        );
        Self {
            position: [0.0, PS2_DEFAULT_FLASHLIGHT_POS_Y_MM, 0.0, 1.0],
            direction_inner: [0.0, 0.0, -1.0, 0.95],
            color_outer: [1.0, 0.95, 0.8, 0.85],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            fill_and_range: [0.0, PS2_DEFAULT_FLASHLIGHT_FAR_MM, 0.0, 0.0],
            parallel_dirs: [[0.0, 0.0, 1.0, 0.0]; 3],
            parallel_colors: [[0.0; 4]; 3],
            point_positions: [[0.0; 4]; 3],
            point_colors: [[0.0; 4]; 3],
            ps2_falloff_params: falloff,
            gun_and_dark_trim: [0.0, 0.0, DARK_HOSEI, 1.0],
            reflection_dir: [0.0, -0.707_106_77, 0.707_106_77, 1.0],
            reflection_color: [1.0, 1.0, 1.0, 0.0],
        }
    }
}

impl SpotlightUniforms {
    /// Populates the 3 parallel directional light slots (`sh2gfw_setparal3LGT_blk` `0x0017d7f0`).
    pub fn set_parallel_lights(&mut self, lights: &[ParallelLightParams; 3]) {
        for (i, l) in lights.iter().enumerate() {
            self.parallel_dirs[i] = [
                l.direction[0],
                l.direction[1],
                l.direction[2],
                if l.enabled { 1.0 } else { 0.0 },
            ];
            self.parallel_colors[i] = [l.color[0], l.color[1], l.color[2], 0.0];
        }
    }

    /// Populates the 3 demo/room point light slots (`sh2gfw_Set_DemoPointLight` `0x0017d600`).
    pub fn set_demo_point_lights(&mut self, lights: &[DemoPointLightParams; 3]) {
        for (i, l) in lights.iter().enumerate() {
            if l.enabled && l.far_mm > l.near_mm {
                self.point_positions[i] = [l.position[0], l.position[1], l.position[2], l.near_mm];
                self.point_colors[i] = [l.color[0], l.color[1], l.color[2], l.far_mm];
            } else {
                self.point_positions[i] = [0.0; 4];
                self.point_colors[i] = [0.0; 4];
            }
        }
    }

    /// Updates the exact PS2 linear/hyperbolic distance falloff coefficients (`0x0017ddd0` / `0x00126170`).
    pub fn set_ps2_falloff(&mut self, near_mm: f32, far_mm: f32, use_hyperbolic: bool) {
        self.ps2_falloff_params = compute_ps2_falloff_params(near_mm, far_mm);
        self.fill_and_range[1] = far_mm;
        self.fill_and_range[3] = if use_hyperbolic { 1.0 } else { 0.0 };
    }

    /// Sets gun muzzle flash ambient boost (`(sh2gfw_Get_GunLight() * 2.0) / 4.0 = gun_light * 0.5`
    /// in 1.0-normalized ambient space, factoring out the `4.0` VU1 ambient scale in `0x0017d7f0` /
    /// `0x0017ddd0`), `DARK_Hosei` trim (`sh2gfw_Get_DarkTrimFactor` `0x0028e350`), and day/night flag
    /// (`0x00181be0`).
    pub fn set_gun_and_dark_trim(&mut self, gun_light: f32, dark_trim_factor: f32, is_night: bool) {
        self.gun_and_dark_trim = [
            gun_light * super::jms_spot::GUN_LIGHT_NORMALIZED_AMBIENT_MULTIPLIER,
            dark_trim_factor,
            DARK_HOSEI,
            if is_night { 1.0 } else { 0.0 },
        ];
    }

    /// Sets the reflection light direction/color (`UpdateReflections` `0x00126f90`) and
    /// `MakeEnvironPacket` (`0x0012cf20`) environment alpha (`env_alpha_u8 / 128.0`).
    pub fn set_reflection(
        &mut self,
        dir: [f32; 3],
        brightness: f32,
        color: [f32; 3],
        env_alpha_u8: u8,
    ) {
        self.reflection_dir = [dir[0], dir[1], dir[2], brightness];
        self.reflection_color = [
            color[0],
            color[1],
            color[2],
            f32::from(env_alpha_u8) / 128.0,
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotlight_uniforms_layout() {
        assert_eq!(std::mem::size_of::<SpotlightUniforms>(), 336);
        assert_eq!(std::mem::align_of::<SpotlightUniforms>(), 4);
    }

    #[test]
    fn test_default_uses_ps2_mm_units() {
        let u = SpotlightUniforms::default();
        assert_eq!(u.position[1], -1500.0);
        assert_eq!(u.fill_and_range[1], 6000.0);
    }

    #[test]
    fn test_linear_and_hyperbolic_falloff_knees() {
        let near = 1500.0_f32;
        let far = 6000.0_f32;
        // At or before near knee: 1.0
        assert!((eval_linear_falloff(0.0, near, far) - 1.0).abs() < 1e-6);
        assert!((eval_linear_falloff(near, near, far) - 1.0).abs() < 1e-6);
        assert!((eval_hyperbolic_falloff(0.0, near, far) - 1.0).abs() < 1e-6);
        assert!((eval_hyperbolic_falloff(near, near, far) - 1.0).abs() < 1e-6);
        // At midpoint 3750 mm: linear = 0.5, hyperbolic = (1500*6000/4500)/3750 - 1500/4500 = 0.2
        let mid = 0.5 * (near + far);
        assert!((eval_linear_falloff(mid, near, far) - 0.5).abs() < 1e-6);
        let expected_hyper_mid = (near * far / (far - near)) / mid - near / (far - near);
        assert!((eval_hyperbolic_falloff(mid, near, far) - expected_hyper_mid).abs() < 1e-6);
        // At or beyond far cutoff: 0.0
        assert!(eval_linear_falloff(far, near, far).abs() < 1e-6);
        assert!(eval_linear_falloff(9000.0, near, far).abs() < 1e-6);
        assert!(eval_hyperbolic_falloff(far, near, far).abs() < 1e-6);
        assert!(eval_hyperbolic_falloff(9000.0, near, far).abs() < 1e-6);
    }
}
