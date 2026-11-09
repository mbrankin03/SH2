//! Lighting & Spotlight configurations matching PS2 ground-truth `dbg_spotL_ctl`.

/// Spotlight uniform structure matching flashlight parameters and room atmospheric grading (`SH2-105`).
///
/// Layout is aligned to 16 bytes for WGSL uniform buffer compliance (total size: 64 bytes).
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
    /// and `TimeOfDay` interior ambient scale (`w`, e.g. `0.55` for `TimeOfDay = 15` in Wood Side Rm 307 Closet).
    pub ambient_tint: [f32; 4],
}

impl Default for SpotlightUniforms {
    fn default() -> Self {
        Self {
            position: [0.0, 1.5, 0.0, 1.0],
            direction_inner: [0.0, 0.0, -1.0, 0.95], // ~18 deg inner
            color_outer: [1.0, 0.95, 0.8, 0.85],     // ~31 deg outer
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotlight_uniforms_layout() {
        assert_eq!(std::mem::size_of::<SpotlightUniforms>(), 64);
        assert_eq!(std::mem::align_of::<SpotlightUniforms>(), 4);
    }
}
