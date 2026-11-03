//! Lighting & Spotlight configurations matching PS2 ground-truth `dbg_spotL_ctl`.

/// Spotlight uniform structure matching flashlight parameters.
///
/// Layout is aligned to 16 bytes for WGSL uniform buffer compliance (total size: 48 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpotlightUniforms {
    /// Light position in world space.
    pub position: [f32; 4],
    /// Light direction normalized (xyz) and inner cone angle cosine (w).
    pub direction_inner: [f32; 4],
    /// Light color RGB (xyz) and outer cone angle cosine (w).
    pub color_outer: [f32; 4],
}

impl Default for SpotlightUniforms {
    fn default() -> Self {
        Self {
            position: [0.0, 1.5, 0.0, 1.0],
            direction_inner: [0.0, 0.0, -1.0, 0.95], // ~18 deg inner
            color_outer: [1.0, 0.95, 0.8, 0.85],     // ~31 deg outer
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotlight_uniforms_layout() {
        assert_eq!(std::mem::size_of::<SpotlightUniforms>(), 48);
        assert_eq!(std::mem::align_of::<SpotlightUniforms>(), 4);
    }
}
