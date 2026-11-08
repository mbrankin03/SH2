use bytemuck::{Pod, Zeroable};

use super::collision::ClVhitResult;
use super::matrix::{Matrix4x4, Sh2Ptr32, Vector4};

/// Character Skeleton Bone Node (`shSkelton`, 240 bytes / `0xF0`, DIE `@0xe0ec`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShSkelton {
    pub next: Sh2Ptr32,   // +0x00 (0)   Next sibling/chain bone pointer (`shSkelton*`)
    pub parent: Sh2Ptr32, // +0x04 (4)   Parent bone pointer (`shSkelton*`)
    pub _pad0: [u8; 8],   // +0x08 (8)   16-byte alignment padding
    pub src_m: Matrix4x4, // +0x10 (16)  Source rotation/scale matrix (64B)
    pub src_t: Vector4,   // +0x50 (80)  Source translation vector (16B)
    pub des_m: Matrix4x4, // +0x60 (96)  Destination matrix (64B)
    pub des_t: Vector4,   // +0xA0 (160) Destination translation vector (16B)
    pub axis: Vector4,    // +0xB0 (176) Rotation axis vector (16B)
    pub theta: f32,       // +0xC0 (192) Rotation angle
    pub xx: f32,          // +0xC4 (196)
    pub yy: f32,          // +0xC8 (200)
    pub zz: f32,          // +0xCC (204)
    pub xy: f32,          // +0xD0 (208)
    pub yz: f32,          // +0xD4 (212)
    pub zx: f32,          // +0xD8 (216)
    pub c_count: u16,     // +0xDC (220) Interpolation counter
    pub c_speed: u16,     // +0xDE (222) Interpolation speed
    pub change: i8,       // +0xE0 (224) State change flag
    pub reserved: i8,     // +0xE1 (225)
    pub is_key: i8,       // +0xE2 (226) Keyframe flag
    pub pad: i8,          // +0xE3 (227)
    pub untouchable: Sh2Ptr32, // +0xE4 (228) `void*`
    pub _pad1: [u8; 8],   // +0xE8 (232) PS2 quadword tail padding to 0xF0
}

#[allow(non_camel_case_types)]
pub type shSkelton = ShSkelton;

/// Character Sensory Detection Cone / Radius (`shBattleArea`, 8 bytes, DIE `@0xf2d8`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShBattleArea {
    pub center: f32, // +0x00 (0) Detection center distance
    pub radius: f32, // +0x04 (4) Detection radius
}

#[allow(non_camel_case_types)]
pub type shBattleArea = ShBattleArea;

/// Character Combat & Health State (`shBattleInfo`, 128 bytes / `0x80`, DIE `@0xf3d3`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShBattleInfo {
    pub pos: Vector4,          // +0x00 (0)   Attack origin / hit position
    pub vec: Vector4,          // +0x10 (16)  Attack direction vector
    pub id: u16,               // +0x20 (32)  Weapon / attack ID
    pub kind: u16,             // +0x22 (34)  Attack kind
    pub dead_timer: f32,       // +0x24 (36)  Death state timer
    pub damage: f32,           // +0x28 (40)  Last damage amount
    pub shock: f32,            // +0x2C (44)  Stagger / shock amount
    pub atk_result: i32,       // +0x30 (48)  Attack hit result code
    pub _pad0: [u8; 12],       // +0x34 (52)  16-byte alignment padding
    pub prev_atk_pos: Vector4, // +0x40 (64)  Previous frame attack position
    pub target: Sh2Ptr32,      // +0x50 (80)  Combat target (`SubCharacter*`)
    pub hp: f32,               // +0x54 (84)  Current hit points
    pub hp_max: f32,           // +0x58 (88)  Maximum hit points
    pub hp_rate: f32,          // +0x5C (92)  Normalized HP ratio
    pub status: u32,           // +0x60 (96)  Combat status flags
    pub look: ShBattleArea,    // +0x64 (100) Visual awareness area
    pub feel: ShBattleArea,    // +0x6C (108) Proximity awareness area
    pub se: u8,                // +0x74 (116) Sound effect trigger ID
    pub _pad1: [u8; 11],       // +0x75 (117) PS2 quadword tail padding to 0x80
}

#[allow(non_camel_case_types)]
pub type shBattleInfo = ShBattleInfo;

/// Runtime Character / Actor Entity (`SubCharacter`, 496 bytes / `0x1F0`, DIE `@0xd4e2`)
/// Pointed to by `PlayerState.player`, `target`, `look_tgt`, and `enemy_liedown`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SubCharacter {
    pub index: i32,                  // +0x000 (0)   Actor slot index
    pub status: u32,                 // +0x004 (4)   Primary actor status
    pub sub_status: u32,             // +0x008 (8)   Secondary actor status
    pub sub_st: u32,                 // +0x00C (12)  Sub-state flags
    pub kind: i16,                   // +0x010 (16)  Character kind / archetype ID
    pub id: i16,                     // +0x012 (18)  Instance ID
    pub step: u32,                   // +0x014 (20)  State machine step counter
    pub model_type: u32,             // +0x018 (24)  3D model type identifier
    pub _pad0: [u8; 4],              // +0x01C (28)  16-byte alignment padding
    pub pos: Vector4,                // +0x020 (32)  World position
    pub rot: Vector4,                // +0x030 (48)  Euler rotation angles
    pub pos_spd: Vector4,            // +0x040 (64)  Linear velocity vector
    pub rot_spd: Vector4,            // +0x050 (80)  Angular velocity vector
    pub mat: Matrix4x4,              // +0x060 (96)  World transformation matrix (64B)
    pub sk_top: Sh2Ptr32,            // +0x0A0 (160) Root skeleton bone pointer (`shSkelton*`)
    pub eye_y: f32,                  // +0x0A4 (164) Eye height offset
    pub center_y: f32,               // +0x0A8 (168) Center-of-mass height offset
    pub spd: f32,                    // +0x0AC (172) Current scalar horizontal speed
    pub spd_org: f32,                // +0x0B0 (176) Base scalar horizontal speed
    pub spd_y: f32,                  // +0x0B4 (180) Vertical velocity
    pub spd_roty: f32,               // +0x0B8 (184) Yaw rotation speed
    pub _pad1: [u8; 4],              // +0x0BC (188) 16-byte alignment padding
    pub grnd_normal: Vector4,        // +0x0C0 (192) Ground surface normal vector
    pub grnd_height: f32,            // +0x0D0 (208) Ground elevation Y
    pub _pad2: [u8; 12],             // +0x0D4 (212) 16-byte alignment padding
    pub b_pos: Vector4,              // +0x0E0 (224) Backup / previous position
    pub b_rot: Vector4,              // +0x0F0 (240) Backup / previous rotation
    pub en_first_status: i32,        // +0x100 (256) Initial enemy spawn status
    pub colis_fall_timer: i32,       // +0x104 (260) Collision fall timer
    pub _pad3: [u8; 8],              // +0x108 (264) 16-byte alignment padding
    pub battle: ShBattleInfo,        // +0x110 (272) Combat & health state (128B)
    pub eye: ClVhitResult,           // +0x190 (400) Line-of-sight raycast result (64B)
    pub function: Sh2Ptr32,          // +0x1D0 (464) Main update callback pointer
    pub effecter_function: Sh2Ptr32, // +0x1D4 (468) Effector callback pointer
    pub pre: Sh2Ptr32, // +0x1D8 (472) Previous linked-list character (`SubCharacter*`)
    pub next: Sh2Ptr32, // +0x1DC (476) Next linked-list character (`SubCharacter*`)
    pub work: [u8; 4], // +0x1E0 (480) Scratchpad work bytes
    pub enemy_p: Sh2Ptr32, // +0x1E4 (484) Enemy-specific data pointer (`void*`)
    pub _pad4: [u8; 8], // +0x1E8 (488) PS2 quadword tail padding to 0x1F0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_chara_sizes_and_offsets() {
        assert_eq!(size_of::<ShSkelton>(), 240);
        assert_eq!(offset_of!(ShSkelton, src_m), 0x10);
        assert_eq!(offset_of!(ShSkelton, des_m), 0x60);
        assert_eq!(offset_of!(ShSkelton, untouchable), 0xE4);

        assert_eq!(size_of::<ShBattleArea>(), 8);
        assert_eq!(size_of::<ShBattleInfo>(), 128);
        assert_eq!(offset_of!(ShBattleInfo, prev_atk_pos), 0x40);
        assert_eq!(offset_of!(ShBattleInfo, target), 0x50);
        assert_eq!(offset_of!(ShBattleInfo, look), 0x64);
        assert_eq!(offset_of!(ShBattleInfo, se), 0x74);

        assert_eq!(size_of::<SubCharacter>(), 496);
        assert_eq!(offset_of!(SubCharacter, pos), 0x020);
        assert_eq!(offset_of!(SubCharacter, mat), 0x060);
        assert_eq!(offset_of!(SubCharacter, sk_top), 0x0A0);
        assert_eq!(offset_of!(SubCharacter, grnd_normal), 0x0C0);
        assert_eq!(offset_of!(SubCharacter, b_pos), 0x0E0);
        assert_eq!(offset_of!(SubCharacter, battle), 0x110);
        assert_eq!(offset_of!(SubCharacter, eye), 0x190);
        assert_eq!(offset_of!(SubCharacter, function), 0x1D0);
        assert_eq!(offset_of!(SubCharacter, enemy_p), 0x1E4);
    }
}
