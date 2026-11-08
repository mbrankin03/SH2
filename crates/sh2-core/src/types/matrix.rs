use bytemuck::{Pod, Zeroable};

/// 32-bit PS2 EE / x86 virtual memory address (`sh2_ptr32_t`).
/// Ensures 64-bit host builds retain exact 32-bit struct sizes and offsets.
pub type Sh2Ptr32 = u32;

/// 3D Cartesian Vector (12 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    #[inline]
    pub const fn from_array(v: [f32; 3]) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
}

impl From<[f32; 3]> for Vector3 {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        Self::from_array(v)
    }
}

impl From<Vector3> for [f32; 3] {
    #[inline]
    fn from(v: Vector3) -> Self {
        v.to_array()
    }
}

/// 4D Homogeneous / PS2 VU0 Quadword Vector (16 bytes, DIE `@0xe3e9`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    #[inline]
    pub const fn from_array(v: [f32; 4]) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
            w: v[3],
        }
    }
}

impl From<[f32; 4]> for Vector4 {
    #[inline]
    fn from(v: [f32; 4]) -> Self {
        Self::from_array(v)
    }
}

impl From<Vector4> for [f32; 4] {
    #[inline]
    fn from(v: Vector4) -> Self {
        v.to_array()
    }
}

/// 4x4 Floating-Point Matrix (64 bytes, DIE `@0xe988`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Matrix4x4 {
    pub m: [[f32; 4]; 4],
}

/// World-View-Screen Matrix Bundle (`_VbWVSMATRIX`, 192 bytes, DIE `@0x2cf22`)
/// Maps to `VbWvsMatrix` at `0x01C6E440` in `SLUS_202.28`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct WorldViewScreenMatrices {
    pub wvm: Matrix4x4, // +0x00 (0)   World-to-View Matrix
    pub vsm: Matrix4x4, // +0x40 (64)  View-to-Screen Matrix
    pub wsm: Matrix4x4, // +0x80 (128) World-to-Screen Matrix
}

#[allow(non_camel_case_types)]
pub type _VbWVSMATRIX = WorldViewScreenMatrices;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_math_sizes_and_offsets() {
        assert_eq!(size_of::<Sh2Ptr32>(), 4);
        assert_eq!(size_of::<Vector3>(), 12);
        assert_eq!(size_of::<Vector4>(), 16);
        assert_eq!(size_of::<Matrix4x4>(), 64);
        assert_eq!(size_of::<WorldViewScreenMatrices>(), 192);
        assert_eq!(offset_of!(WorldViewScreenMatrices, wvm), 0x00);
        assert_eq!(offset_of!(WorldViewScreenMatrices, vsm), 0x40);
        assert_eq!(offset_of!(WorldViewScreenMatrices, wsm), 0x80);
    }
}
