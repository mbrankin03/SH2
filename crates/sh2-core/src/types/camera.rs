//! Camera subsystem types extracted from `_VC_WORK`.

use super::matrix::Matrix4x4;
use bytemuck::{Pod, Zeroable};

/// Camera movement physics parameters (`_VC_CAM_MV_PARAM`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct CameraMoveParam {
    pub accel_xz: f32,
    pub accel_y: f32,
    pub max_spd_xz: f32,
    pub max_spd_y: f32,
}

/// Camera look/watch tracking parameters (`_VC_WATCH_MV_PARAM`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct CameraWatchParam {
    pub ang_accel_x: f32,
    pub ang_accel_y: f32,
    pub max_ang_spd_x: f32,
    pub max_ang_spd_y: f32,
}

/// Through-door camera transit parameter (`_VC_THROUGH_DOOR_CAM_PARAM`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct ThroughDoorCamParam {
    pub active_f: i32,
    pub timer: f32,
    pub rail_ang_y: f32,
    pub rail_sta_pos: [f32; 4],
    pub rail_sta_to_chara_dist: f32,
}

/// Primary camera state matching runtime `_VC_WORK` structure (416 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct CameraState {
    pub view_cam_active_f: i32,
    pub vc_road_ary_list: u32,
    pub flags: u32,
    pub through_door_activate_init_f: i32,
    pub through_door: ThroughDoorCamParam,
    pub cam_noise: i32,
    pub scr_half_ang_wy: f32,
    pub scr_half_ang_wx: f32,
    pub user_cam_mv_prm: CameraMoveParam,
    pub cam_tgt_pos: [f32; 4],
    pub cam_pos: [f32; 4],
    pub cam_mv_ang_y: f32,
    pub cam_velo: [f32; 4],
    pub old_cam_excl_area_r: f32,
    pub ideal_cam_pos_h: f32,
    pub user_watch_mv_prm: CameraWatchParam,
    pub watch_tgt_pos: [f32; 4],
    pub watch_tgt_max_y: f32,
    pub watch_tgt_ang_z: f32,
    pub cam_mat_ang: [f32; 4],
    pub cam_mat: Matrix4x4,
    pub ofs_cam_ang: [f32; 4],
    pub ofs_cam_ang_spd: [f32; 4],
    pub base_cam_ang: [f32; 4],
    pub old_base_cam_ang: [f32; 4],
    pub cam_chara2ideal_ang_y: f32,
    pub cam_tgt_velo: [f32; 4],
    pub cam_tgt_mv_ang_y: f32,
    pub cam_tgt_spd: f32,
    pub chara_pos: [f32; 4],
    pub chara_bottom_y: f32,
    pub chara_top_y: f32,
    pub chara_center_y: f32,
    pub chara_grnd_y: f32,
    pub chara_head_pos: [f32; 4],
    pub chara_mv_spd: f32,
    pub chara_mv_ang_y: f32,
    pub chara_ang_spd_y: f32,
    pub chara_eye_ang_y: f32,
    pub chara_eye_ang_wy: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_camera_size() {
        assert_eq!(size_of::<CameraState>(), 416);
    }
}
