use bytemuck::{Pod, Zeroable};

use super::matrix::{Matrix4x4, Sh2Ptr32, Vector4};

/// `_VC_CAM_MV_PARAM` (16 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CameraMoveParam {
    pub accel_xz: f32,
    pub accel_y: f32,
    pub max_spd_xz: f32,
    pub max_spd_y: f32,
}

#[allow(non_camel_case_types)]
pub type _VC_CAM_MV_PARAM = CameraMoveParam;

/// `_VC_WATCH_MV_PARAM` (12 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CameraWatchParam {
    pub ang_accel_x: f32,
    pub ang_accel_y: f32,
    pub max_ang_spd: f32,
}

#[allow(non_camel_case_types)]
pub type _VC_WATCH_MV_PARAM = CameraWatchParam;

/// `_VC_THROUGH_DOOR_CAM_PARAM` (48 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ThroughDoorCamParam {
    pub active_f: i32,               // +0x00 (0)
    pub timer: f32,                  // +0x04 (4)
    pub rail_ang_y: f32,             // +0x08 (8)
    pub _pad0: [u8; 4],              // +0x0C (12)
    pub rail_sta_pos: [f32; 4],      // +0x10 (16)
    pub rail_sta_to_chara_dist: f32, // +0x20 (32)
    pub _pad1: [u8; 12],             // +0x24 (36)
}

#[allow(non_camel_case_types)]
pub type _VC_THROUGH_DOOR_CAM_PARAM = ThroughDoorCamParam;

/// `_VC_FIX_CAM_MANAGER` (16 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FixCamManager {
    pub exception_f: i32,
    pub add_ang_y: f32,
    pub add_rd_x: f32,
    pub add_rd_z: f32,
}

#[allow(non_camel_case_types)]
pub type _VC_FIX_CAM_MANAGER = FixCamManager;

/// `_VC_CIR_CAM_MANAGER` (32 bytes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CirCamManager {
    pub origin: Vector4,
    pub sw_p: Vector4,
}

#[allow(non_camel_case_types)]
pub type _VC_CIR_CAM_MANAGER = CirCamManager;

/// Main Camera Workspace (`VC_WORK` / `vcWork`, 416 bytes / `0x1A0`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CameraState {
    pub view_cam_active_f: i32,              // +0x000 (0)
    pub vc_road_ary_list: Sh2Ptr32,          // +0x004 (4)  32-bit pointer
    pub flags: u32,                          // +0x008 (8)
    pub through_door_activate_init_f: i32,   // +0x00C (12)
    pub through_door: ThroughDoorCamParam,   // +0x010 (16)
    pub scr_half_ang_wy: f32,                // +0x040 (64)
    pub scr_half_ang_wx: f32,                // +0x044 (68)
    pub user_cam_mv_prm: CameraMoveParam,    // +0x048 (72)
    pub cam_tgt_pos: [f32; 4],               // +0x058 (88)
    pub cam_pos: [f32; 4],                   // +0x068 (104)
    pub cam_mv_ang: [f32; 4],                // +0x078 (120)
    pub cam_velo: [f32; 4],                  // +0x088 (136)
    pub old_cam_excl_area_r: f32,            // +0x098 (152)
    pub ideal_cam_pos_h: [f32; 4],           // +0x09C (156)
    pub user_watch_mv_prm: CameraWatchParam, // +0x0AC (172)
    pub watch_tgt_pos: [f32; 4],             // +0x0B8 (184)
    pub watch_tgt_max_y: f32,                // +0x0C8 (200)
    pub watch_tgt_ang_z: f32,                // +0x0CC (204)
    pub cam_mat_ang: [f32; 3],               // +0x0D0 (208)
    pub _pad0: [u8; 4],                      // +0x0DC (220)
    pub cam_mat: Matrix4x4,                  // +0x0E0 (224)
    pub ofs_cam_ang: [f32; 3],               // +0x120 (288)
    pub ofs_cam_ang_spd: [f32; 3],           // +0x12C (300)
    pub base_cam_ang: [f32; 3],              // +0x138 (312)
    pub old_base_cam_ang: [u8; 12],          // +0x144 (324)
    pub cam_chara2ideal_ang_y: f32,          // +0x150 (336)
    pub cam_tgt_velo: [f32; 4],              // +0x154 (340)
    pub cam_tgt_mv_ang_y: f32,               // +0x164 (356)
    pub cam_tgt_spd: f32,                    // +0x168 (360)
    pub chara_pos: [f32; 4],                 // +0x16C (364)
    pub chara_bottom_y: f32,                 // +0x17C (380)
    pub chara_top_y: f32,                    // +0x180 (384)
    pub chara_center_y: f32,                 // +0x184 (388)
    pub chara_grnd_y: f32,                   // +0x188 (392)
    pub chara_head_pos: [f32; 3],            // +0x18C (396)
    pub chara_mv_spd: f32,                   // +0x198 (408)
    pub chara_mv_ang_y: f32,                 // +0x19C (412)
}

pub type Camera = CameraState;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_camera_sizes_and_offsets() {
        assert_eq!(size_of::<CameraMoveParam>(), 16);
        assert_eq!(size_of::<CameraWatchParam>(), 12);
        assert_eq!(size_of::<ThroughDoorCamParam>(), 48);
        assert_eq!(size_of::<FixCamManager>(), 16);
        assert_eq!(size_of::<CirCamManager>(), 32);
        assert_eq!(size_of::<CameraState>(), 416);
        assert_eq!(offset_of!(CameraState, vc_road_ary_list), 0x004);
        assert_eq!(offset_of!(CameraState, through_door), 0x010);
        assert_eq!(offset_of!(CameraState, cam_pos), 0x068);
        assert_eq!(offset_of!(CameraState, watch_tgt_pos), 0x0B8);
        assert_eq!(offset_of!(CameraState, cam_mat), 0x0E0);
        assert_eq!(offset_of!(CameraState, chara_mv_ang_y), 0x19C);
    }
}
