//! Lighting & Spotlight configurations matching PS2 ground-truth `dbg_spotL_ctl`.

/// Spotlight uniform structure matching flashlight parameters, room atmospheric grading (`SH2-105`),
/// and authentic 1:1 room darkness modulation / camera fill (`SH2-33`).
///
/// Layout is aligned to 16 bytes for WGSL uniform buffer compliance (total size: 80 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpotlightUniforms {
    /// Light position in world space.
    pub position: [f32; 4],
    /// Light direction normalized (xyz) and inner cone angle cosine (w).
    pub direction_inner: [f32; 4],
    /// Light color RGB (xyz) and outer cone angle cosine (w).
    pub color_outer: [f32; 4],
    /// Room/cutscene world ambient tint RGB (`xyz`, e.g. `RGB(255, 255, 227)` in Hotel Room 312 Sunset)
    /// and combined ambient intensity scale (`w`, incorporating `0.5 + 0.25 * s_flash_level` from
    /// `sh2pc.exe` `0x47a090..0x47a18b` and `TimeOfDay` scaling).
    pub ambient_tint: [f32; 4],
    /// Camera view-space directional fill light & flashlight reach calibration (`SH2-33`):
    /// - `x`: `camera_fill_k = 0.3 * s_flash_level * 0.25` (`sh2pc.exe` `0x50b036`, `0x89ffd8`)
    /// - `y`: calibrated flashlight reach in PS2 mm (`6000.0` PS2 / `8500.0` SH2:EE, `EnhancedFlashlight.cpp:162-164`)
    /// - `z`: active `s_flash_level` (`0.0`, `0.3`, `0.75`, `1.0`, `sh2pc.exe` `0x89ffc0`)
    /// - `w`: reserved (`0.0`)
    pub fill_and_range: [f32; 4],
}

impl Default for SpotlightUniforms {
    fn default() -> Self {
        Self {
            position: [0.0, 1.5, 0.0, 1.0],
            direction_inner: [0.0, 0.0, -1.0, 0.95], // ~18 deg inner
            color_outer: [1.0, 0.95, 0.8, 0.85],     // ~31 deg outer
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            fill_and_range: [0.0, 8500.0, 0.0, 0.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotlight_uniforms_layout() {
        assert_eq!(std::mem::size_of::<SpotlightUniforms>(), 80);
        assert_eq!(std::mem::align_of::<SpotlightUniforms>(), 4);
    }
}
