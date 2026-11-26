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

/// Exact `sh2pc.exe` `.map` directional/spot light color scale (`4.0 * DAT_007f4120[0xc]` =
/// `4.0 * 0.0625 = 0.25`, `FUN_0047a420` `0x0047a420` lines 8121-8125 & `DAT_009428e7 = 0xc`).
pub const PS2_MAP_LIGHT_COLOR_SCALE: f32 = 4.0 * 0.0625;

/// Exact `sh2pc.exe` `.map` ambient constant `c19` scale (`DAT_007f4120[0xb] = 0.125`,
/// `FUN_00479da0` `0x00479da0` + `DAT_009428e2 = 0xb`, applied to `ambient * 4.0 + gun_light * 2.0
/// + dark_trim * 2.0 * ambient` from `FUN_0047a090` `0x0047a090`).
pub const PS2_MAP_AMBIENT_C19_SCALE: f32 = 0.125;

/// Exact `sh2pc.exe` `.map` base-color constant `c20` scale (`DAT_007f4120[0xc] = 0.0625`,
/// `FUN_00479da0` `0x00479da0` line 7925 + `DAT_009428e3 = 0xc`; `22.4 * 0.0625 = 1.4`).
pub const PS2_MAP_BASE_COLOR_C20_SCALE: f32 = 0.0625;

/// Exact `sh2pc.exe` `.map` base-ambient constant `c21` scale (`DAT_007f4120[0xc] = 0.0625`,
/// `FUN_00479da0` `0x00479da0` line 7930 + `DAT_009428e5 = 0xc`; `0.045 * 0.0625 = 0.0028125`).
pub const PS2_MAP_BASE_AMBIENT_C21_SCALE: f32 = 0.0625;

/// Exact `sh2pc.exe` `.map` vertex-ambient constant `c22` scale (`DAT_007f4120[10] = 0.25`,
/// `FUN_00479da0` `0x00479da0` line 7936 + `DAT_009428e4 = 10`; `4.0 * 0.25 = 1.0`).
pub const PS2_MAP_VERTEX_AMBIENT_C22_SCALE: f32 = 0.25;

/// Exact `SLUS_202.28` outdoor daytime character ambient multiplier (`DAT_002ad9e0 = 0x3fa66666`
/// = `1.30`, `Set_DrawEnvData` `0x00251060` line 262 for `stage_id < 5 && night_or_day == 0`).
pub const PS2_CHAR_AMBIENT_OUTDOOR_DAY_MULT: f32 = 1.3;

/// Exact `SLUS_202.28` / `sh2pc.exe` indoor & nighttime character ambient multiplier
/// (`DAT_002ad9e0 = 0x3f866666 = 1.05`, `Set_DrawEnvData` `0x00251060` lines 265/269,
/// `sh2pc.exe` `_DAT_00942b50` at `0x00470000.c:9343`).
pub const PS2_CHAR_AMBIENT_INDOOR_MULT: f32 = 1.05;

/// Exact `sh2pc.exe` `FUN_0050dd00` (`0x0050dd00` lines 8933-8935, `DAT_008a0074 = 0.3f`)
/// dark-trim additive boost to character global ambient.
pub const PS2_CHAR_DARK_TRIM_ADD: f32 = 0.3;

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
/// Strictly 16-byte aligned for WGSL uniform buffer compliance (26 × `vec4<f32>` = **416 bytes**).
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
    /// 3 parallel directional light RGB colors (`xyz`, raw `DrawEnvData` 1.0x units; `.map` multiplies by `0.25`, `w` reserved `0.0`).
    pub parallel_colors: [[f32; 4]; 3],
    /// 3 demo/room point light positions (`xyz` in PS2 mm) and `near_mm` (`w`) (`sh2gfw_Set_DemoPointLight` `0x0017d600`).
    pub point_positions: [[f32; 4]; 3],
    /// 3 demo/room point light RGB colors (`xyz`, raw `DrawEnvData` 1.0x units) and `far_mm` (`w`; `<= 0.0` disables slot).
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
    /// `sh2pc.exe` `.map` vertex shader constant `c19` (`FUN_00479da0` `0x00479da0` + `FUN_0047a090`
    /// `0x0047a090`): `0.125 * (DrawEnvData.ambient * 4.0 + gun_light * 2.0 + dark_trim * 2.0 * ambient)`.
    /// `w = 1.0` enables exact `VS[1..8]` `DrawEnvData` lighting in `pipeline.rs`.
    pub map_ambient_c19: [f32; 4],
    /// `sh2pc.exe` `.map` vertex shader constant `c20` (`FUN_00479da0` `0x00479da0:7925`):
    /// `DrawEnvData.base_color * 0.0625` (`1.4` default).
    pub map_base_color_c20: [f32; 4],
    /// `sh2pc.exe` `.map` vertex shader constant `c21` (`FUN_00479da0` `0x00479da0:7930`):
    /// `(DrawEnvData.base_ambient * (1.0 + 2.0 * dark_trim) + gun_light * 2.0) * 0.0625`.
    pub map_base_ambient_c21: [f32; 4],
    /// `sh2pc.exe` `.map` vertex shader constant `c22` (`FUN_00479da0` `0x00479da0:7936`):
    /// `DrawEnvData.vertex_ambient * 0.25` (`1.0` default).
    pub map_vertex_ambient_c22: [f32; 4],
    /// `SLUS_202.28` `Model3UpdateGlobalAmbient` (`0x00128e70`) / `sh2pc.exe` `FUN_0050dd00`
    /// (`0x0050dd00:8924-8936`) character global ambient:
    /// `DrawEnvData.ambient * 2.0 * DAT_002ad9e0 + 0.3 * min(dark_trim, 1.0)`.
    /// `w = 1.0` enables exact `Model3` character ambient in `skeletal/shader.rs`.
    pub char_global_ambient: [f32; 4],
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
            map_ambient_c19: [0.0; 4],
            map_base_color_c20: [1.4, 1.4, 1.4, 0.0],
            map_base_ambient_c21: [0.045 * 0.0625, 0.045 * 0.0625, 0.045 * 0.0625, 0.0],
            map_vertex_ambient_c22: [1.0, 1.0, 1.0, 0.0],
            char_global_ambient: [0.0; 4],
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

    /// Populates the exact `sh2pc.exe` `.map` vertex-shader constants `c19..c22` (`FUN_00479da0`
    /// `0x00479da0` & `FUN_0047a090` `0x0047a090`) and `SLUS_202.28` / `sh2pc.exe` character global
    /// ambient (`Model3UpdateGlobalAmbient` `0x00128e70` & `FUN_0050dd00` `0x0050dd00`) from `env`.
    ///
    /// Per `sh2pc.exe` `FUN_0047a090` (`0x0047a090:7972-8009`) and `FUN_0050dd00`
    /// (`0x0050dd00:8919-8937`), `map_ambient_c19` already incorporates `(0.5 + 0.25 * dark_trim)`
    /// via `(env.ambient * 4.0 + gun_light * 2.0 + dark_trim * 2.0 * env.ambient) * 0.125`, while
    /// `map_vertex_ambient_c22` and `char_global_ambient` are never multiplied by `dark_scale`.
    pub fn set_draw_env_constants(
        &mut self,
        env: &sh2_core::lighting::DrawEnvData,
        gun_light: f32,
        dark_trim: f32,
        is_dark_room: bool,
    ) {
        let gun2 = gun_light * 2.0;
        let trim2 = dark_trim * 2.0;
        // `FUN_0047a090` (`0x0047a090:7978-8008`) + `FUN_00479da0` (`0x00479da0:7925-7940`):
        // - c19 = (ambient * 4.0 + gun2 + trim2 * ambient) * 0.125
        // - c20 = base_color * 0.0625
        // - c21 = (base_ambient + gun2 + trim2 * base_ambient) * 0.0625
        // - c22 = vertex_ambient * 0.25
        self.map_ambient_c19 = [
            (env.ambient[0] * 4.0 + gun2 + trim2 * env.ambient[0]) * PS2_MAP_AMBIENT_C19_SCALE,
            (env.ambient[1] * 4.0 + gun2 + trim2 * env.ambient[1]) * PS2_MAP_AMBIENT_C19_SCALE,
            (env.ambient[2] * 4.0 + gun2 + trim2 * env.ambient[2]) * PS2_MAP_AMBIENT_C19_SCALE,
            1.0,
        ];
        self.map_base_color_c20 = [
            env.base_color[0] * PS2_MAP_BASE_COLOR_C20_SCALE,
            env.base_color[1] * PS2_MAP_BASE_COLOR_C20_SCALE,
            env.base_color[2] * PS2_MAP_BASE_COLOR_C20_SCALE,
            0.0,
        ];
        self.map_base_ambient_c21 = [
            (env.base_ambient[0] + gun2 + trim2 * env.base_ambient[0])
                * PS2_MAP_BASE_AMBIENT_C21_SCALE,
            (env.base_ambient[1] + gun2 + trim2 * env.base_ambient[1])
                * PS2_MAP_BASE_AMBIENT_C21_SCALE,
            (env.base_ambient[2] + gun2 + trim2 * env.base_ambient[2])
                * PS2_MAP_BASE_AMBIENT_C21_SCALE,
            0.0,
        ];
        self.map_vertex_ambient_c22 = [
            env.vertex_ambient[0] * PS2_MAP_VERTEX_AMBIENT_C22_SCALE,
            env.vertex_ambient[1] * PS2_MAP_VERTEX_AMBIENT_C22_SCALE,
            env.vertex_ambient[2] * PS2_MAP_VERTEX_AMBIENT_C22_SCALE,
            0.0,
        ];

        // `SLUS_202.28` `Set_DrawEnvData` (`0x00251060:259-270`) + `Model3UpdateGlobalAmbient`
        // (`0x00128e70:219-221`) & `sh2pc.exe` `FUN_0050dd00` (`0x0050dd00:8924-8935`):
        // `char_amb = (env.ambient * 2.0) * DAT_002ad9e0 + 0.3 * min(dark_trim, 1.0)`.
        let stage_mult = if env.stage_id < 5 && !is_dark_room {
            PS2_CHAR_AMBIENT_OUTDOOR_DAY_MULT
        } else {
            PS2_CHAR_AMBIENT_INDOOR_MULT
        };
        let trim_add = PS2_CHAR_DARK_TRIM_ADD * dark_trim.min(1.0);
        self.char_global_ambient = [
            env.ambient[0] * 2.0 * stage_mult + trim_add,
            env.ambient[1] * 2.0 * stage_mult + trim_add,
            env.ambient[2] * 2.0 * stage_mult + trim_add,
            1.0,
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotlight_uniforms_layout() {
        assert_eq!(std::mem::size_of::<SpotlightUniforms>(), 416);
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

    #[test]
    fn test_draw_env_constants_match_sh2pc_00479da0_and_slus_00128e70() {
        let er01 = sh2_core::lighting::lookup_draw_env("ER", 1).expect("ER room 1 DrawEnvData");
        let mut u = SpotlightUniforms::default();
        u.set_draw_env_constants(er01, 0.0, 0.0, false);

        // `FUN_00479da0` (`0x00479da0`) + `FUN_0047a090` (`0x0047a090`):
        // - c19 = (0.5 * 4.0) * 0.125 = 0.25
        // - c20 = 22.4 * 0.0625 = 1.4
        // - c21 = 0.045 * 0.0625 = 0.0028125
        // - c22 = 4.0 * 0.25 = 1.0
        assert!((u.map_ambient_c19[0] - 0.25).abs() < 1e-5);
        assert_eq!(u.map_ambient_c19[3], 1.0);
        assert!((u.map_base_color_c20[0] - 1.4).abs() < 1e-5);
        assert!((u.map_base_ambient_c21[0] - 0.002_812_5).abs() < 1e-6);
        assert!((u.map_vertex_ambient_c22[0] - 1.0).abs() < 1e-5);

        // `SLUS_202.28` `Model3UpdateGlobalAmbient` (`0x00128e70`) + `Set_DrawEnvData` (`0x00251060`):
        // `er01` (`stage_id == 6 >= 5`) uses `DAT_002ad9e0 = 1.05`: `0.5 * 2.0 * 1.05 = 1.05`.
        assert!((u.char_global_ambient[0] - 1.05).abs() < 1e-5);
        assert_eq!(u.char_global_ambient[3], 1.0);

        // Dark room (`AP` room 18, `ambient = 0.15`, `base_ambient = 0.045`, `vertex_ambient = 4.0`)
        // with `dark_trim = 0.5` and `gun_light = 0.0` (`SH2-168`):
        // - `FUN_0047a090` (`0x0047a090:7978-8008`):
        //   c19 = (0.15 * 4.0 + 0.5 * 2.0 * 0.15) * 0.125 = 0.15 * (0.5 + 0.25 * 0.5) = 0.09375 (NO double dark_scale)
        //   c21 = (0.045 + 0.5 * 2.0 * 0.045) * 0.0625 = 0.09 * 0.0625 = 0.005625
        //   c22 = 4.0 * 0.25 = 1.0 (NEVER multiplied by dark_scale)
        // - `FUN_0050dd00` (`0x0050dd00:8924-8935`):
        //   char_global_ambient = (0.15 * 2.0 * 1.05) + 0.3 * 0.5 = 0.315 + 0.15 = 0.465 (NEVER multiplied by dark_scale)
        let ap18 = sh2_core::lighting::lookup_draw_env("AP", 18).expect("AP room 18 DrawEnvData");
        u.set_draw_env_constants(ap18, 0.0, 0.5, true);
        let expected_c19 = (0.15 * 4.0 + 2.0 * 0.5 * 0.15) * 0.125;
        let expected_c21 = (0.045 + 2.0 * 0.5 * 0.045) * 0.0625;
        let expected_char_amb = 0.15 * 2.0 * 1.05 + 0.3 * 0.5;
        assert!((u.map_ambient_c19[0] - expected_c19).abs() < 1e-6);
        assert!((u.map_base_ambient_c21[0] - expected_c21).abs() < 1e-6);
        assert!((u.map_vertex_ambient_c22[0] - 1.0).abs() < 1e-6);
        assert!((u.char_global_ambient[0] - expected_char_amb).abs() < 1e-5);
    }
}
