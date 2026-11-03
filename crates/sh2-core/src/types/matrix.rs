//! Matrix and transformation types matching PS2 EE / PC layout.

use bytemuck::{Pod, Zeroable};

/// 4x4 Floating-point Matrix (64 bytes, 16-byte aligned).
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Matrix4x4 {
    pub m: [[f32; 4]; 4],
}

impl Default for Matrix4x4 {
    fn default() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

/// World-View-Screen Matrix Triple (`_VbWVSMATRIX`, 192 bytes from DWARF).
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct WorldViewScreenMatrices {
    /// World-to-View matrix (64 bytes)
    pub wvm: Matrix4x4,
    /// View-to-Screen matrix (64 bytes)
    pub vsm: Matrix4x4,
    /// World-to-Screen matrix (64 bytes)
    pub wsm: Matrix4x4,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_matrix_sizes() {
        assert_eq!(size_of::<Matrix4x4>(), 64);
        assert_eq!(size_of::<WorldViewScreenMatrices>(), 192);
    }
}
