//! Volumetric compute fog parameters matching PS2 ground-truth registers.
//!
//! # Volumetric Fog Architecture
//! In original Silent Hill 2 hardware on the PlayStation 2, fog density was calculated
//! per-pixel using specialized VU1 microcode and GS fog registers (`FOGCOL`, `GS_FOG`).
//!
//! This module mirrors the uniform buffer required by modern GPU shaders (`wgpu` / WGSL)
//! to render distance-based exponential and linear fog falloff.

/// Uniform buffer for volumetric fog shader.
///
/// Layout is aligned to 16 bytes for WGSL uniform buffer compliance (total size: 32 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FogUniforms {
    /// Fog ambient color (RGBA). (Offset 0, size 16)
    pub fog_color: [f32; 4],
    /// Near distance cutoff where fog starts taking effect. (Offset 16, size 4)
    pub fog_near: f32,
    /// Far distance cutoff where fog density reaches maximum. (Offset 20, size 4)
    pub fog_far: f32,
    /// Fog density multiplier. (Offset 24, size 4)
    pub density: f32,
    /// Explicit padding to satisfy 16-byte WGSL alignment. (Offset 28, size 4)
    pub _padding: f32,
}

impl Default for FogUniforms {
    fn default() -> Self {
        Self {
            fog_color: [0.7, 0.7, 0.75, 1.0],
            fog_near: 2.0,
            fog_far: 25.0,
            density: 1.0,
            _padding: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fog_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<FogUniforms>(), 32);
        assert_eq!(std::mem::align_of::<FogUniforms>(), 4);
    }
}
