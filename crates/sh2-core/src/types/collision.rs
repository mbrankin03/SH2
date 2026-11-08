use bytemuck::{Pod, Zeroable};

use super::matrix::{Sh2Ptr32, Vector4};

/// Wall Raycast Hit Payload (`_CL_VHIT_WALL`, 48 bytes, DIE `@0x10abd`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClVhitWall {
    pub cp: Vector4,    // +0x00 (0)  Contact point
    pub nl: Vector4,    // +0x10 (16) Surface normal
    pub pd: Sh2Ptr32,   // +0x20 (32) Pointer to `_CL_HITPOLY_HEAD`
    pub _pad: [u8; 12], // +0x24 (36) PS2 quadword tail padding
}

#[allow(non_camel_case_types)]
pub type _CL_VHIT_WALL = ClVhitWall;

/// Character Raycast Hit Payload (`_CL_VHIT_CHARA`, 32 bytes, DIE `@0xdbbf`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClVhitChara {
    pub sc: Sh2Ptr32,   // +0x00 (0)  Pointer to `SubCharacter`
    pub _pad: [u8; 12], // +0x04 (4)  16-byte alignment padding for `cp`
    pub cp: Vector4,    // +0x10 (16) Contact point
}

#[allow(non_camel_case_types)]
pub type _CL_VHIT_CHARA = ClVhitChara;

/// Union Storage of Wall and Character Hit Objects (48 bytes, DIE `@0xdd47`)
/// Represented as `ClVhitWall` (the 48-byte union carrier) with zero-copy
/// `bytemuck` accessors for both `_CL_VHIT_WALL` (48B) and `_CL_VHIT_CHARA` (32B).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClVhitHobj {
    pub wall: ClVhitWall,
}

impl ClVhitHobj {
    #[inline]
    pub fn as_wall(&self) -> &ClVhitWall {
        &self.wall
    }

    #[inline]
    pub fn as_wall_mut(&mut self) -> &mut ClVhitWall {
        &mut self.wall
    }

    #[inline]
    pub fn as_chara(&self) -> &ClVhitChara {
        let bytes = &bytemuck::bytes_of(&self.wall)[..32];
        bytemuck::from_bytes(bytes)
    }

    #[inline]
    pub fn as_chara_mut(&mut self) -> &mut ClVhitChara {
        let bytes = &mut bytemuck::bytes_of_mut(&mut self.wall)[..32];
        bytemuck::from_bytes_mut(bytes)
    }
}

/// Raycast / Sensor Hit Result (`_CL_VHIT_RESULT`, 64 bytes, DIE `@0xdcdb`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClVhitResult {
    pub kind: i32,        // +0x00 (0)  Hit object classification
    pub _pad: [u8; 12],   // +0x04 (4)  16-byte alignment padding for `hobj`
    pub hobj: ClVhitHobj, // +0x10 (16) Hit object payload (48B union of wall / chara)
}

impl ClVhitResult {
    #[inline]
    pub fn as_wall(&self) -> &ClVhitWall {
        self.hobj.as_wall()
    }

    #[inline]
    pub fn as_wall_mut(&mut self) -> &mut ClVhitWall {
        self.hobj.as_wall_mut()
    }

    #[inline]
    pub fn as_chara(&self) -> &ClVhitChara {
        self.hobj.as_chara()
    }

    #[inline]
    pub fn as_chara_mut(&mut self) -> &mut ClVhitChara {
        self.hobj.as_chara_mut()
    }
}

#[allow(non_camel_case_types)]
pub type _CL_VHIT_RESULT = ClVhitResult;

/// Cylindrical / Column Collision Volume (`_CL_HITPOLY_COLUMN`, 48 bytes, DIE `@0xe53b`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClHitpolyColumn {
    pub kind: u8,         // +0x00 (0)  Collision kind
    pub shape: u8,        // +0x01 (1)  Collision shape
    pub pad: u16,         // +0x02 (2)  Alignment padding
    pub weight: u32,      // +0x04 (4)  Collision weight
    pub material: u32,    // +0x08 (8)  Surface material ID
    pub flg: i32,         // +0x0C (12) Collision flags
    pub p: [[f32; 4]; 2], // +0x10 (16) Top/bottom cylinder endpoints (32B)
}

#[allow(non_camel_case_types)]
pub type _CL_HITPOLY_COLUMN = ClHitpolyColumn;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_collision_sizes_and_offsets() {
        assert_eq!(size_of::<ClVhitWall>(), 48);
        assert_eq!(offset_of!(ClVhitWall, cp), 0x00);
        assert_eq!(offset_of!(ClVhitWall, nl), 0x10);
        assert_eq!(offset_of!(ClVhitWall, pd), 0x20);

        assert_eq!(size_of::<ClVhitChara>(), 32);
        assert_eq!(offset_of!(ClVhitChara, sc), 0x00);
        assert_eq!(offset_of!(ClVhitChara, cp), 0x10);

        assert_eq!(size_of::<ClVhitResult>(), 64);
        assert_eq!(offset_of!(ClVhitResult, kind), 0x00);
        assert_eq!(offset_of!(ClVhitResult, hobj), 0x10);

        assert_eq!(size_of::<ClHitpolyColumn>(), 48);
        assert_eq!(offset_of!(ClHitpolyColumn, kind), 0x00);
        assert_eq!(offset_of!(ClHitpolyColumn, shape), 0x01);
        assert_eq!(offset_of!(ClHitpolyColumn, pad), 0x02);
        assert_eq!(offset_of!(ClHitpolyColumn, weight), 0x04);
        assert_eq!(offset_of!(ClHitpolyColumn, material), 0x08);
        assert_eq!(offset_of!(ClHitpolyColumn, flg), 0x0C);
        assert_eq!(offset_of!(ClHitpolyColumn, p), 0x10);
    }

    #[test]
    fn test_vhit_result_union_accessors() {
        let mut res = ClVhitResult {
            kind: 2,
            ..Default::default()
        };
        res.as_chara_mut().sc = 0x01C0_1234;
        res.as_chara_mut().cp = Vector4::new(1.0, 2.0, 3.0, 1.0);
        assert_eq!(res.as_chara().sc, 0x01C0_1234);
        assert_eq!(res.as_wall().nl, Vector4::new(1.0, 2.0, 3.0, 1.0));
    }
}
