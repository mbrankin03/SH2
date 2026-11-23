use bytemuck::{Pod, Zeroable};

use super::matrix::{Matrix4x4, Sh2Ptr32, Vector4};

/// `_VC_CAM_MV_PARAM` (16 bytes; `SLUS_202.28` `cam_mv_prm_user` `0x00390560`).
///
/// Units: PS2 world millimeters per second² (`accel_*`) and millimeters per second (`max_spd_*`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CameraMoveParam {
    /// Horizontal XZ linear acceleration in mm/s².
    pub accel_xz: f32,
    /// Vertical Y linear acceleration in mm/s².
    pub accel_y: f32,
    /// Horizontal XZ maximum speed in mm/s.
    pub max_spd_xz: f32,
    /// Vertical Y maximum speed in mm/s.
    pub max_spd_y: f32,
}

#[allow(non_camel_case_types)]
pub type _VC_CAM_MV_PARAM = CameraMoveParam;

/// `_VC_WATCH_MV_PARAM` (16 bytes; `SLUS_202.28` `watch_mv_prm_user` `0x00390510`,
/// consumed by `vcAdjCamOfsAngByOfsAngSpd` `0x0019d440`).
///
/// Units: radians per second² (`ang_accel_*`) and radians per second (`max_ang_spd_*`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CameraWatchParam {
    /// Pitch (X-axis) angular acceleration in rad/s² (`+0x00`).
    pub ang_accel_x: f32,
    /// Yaw (Y-axis) angular acceleration in rad/s² (`+0x04`).
    pub ang_accel_y: f32,
    /// Pitch (X-axis) maximum angular speed in rad/s (`+0x08`).
    pub max_ang_spd_x: f32,
    /// Yaw (Y-axis) maximum angular speed in rad/s (`+0x0C`).
    pub max_ang_spd_y: f32,
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

/// Main Camera Workspace prefix (`_VC_WORK` / `vcWork` at `SLUS_202.28` `0x010e5ad0`,
/// 544 bytes / `0x220` up to `near_road_ary` at `+0x220`).
///
/// All VU0 128-bit vector fields (`_lqc2`/`_sqc2`) are 16-byte aligned to match
/// `SLUS_202.28` `vc_main.c` (`vcSetFirstCamWork` `0x00195ad0`, `vcSetSubjChara` `0x00195f00`,
/// `vcRenewalCamData` `0x0019c840`, `vcRenewalCamMatAng` `0x0019c9d0`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CameraState {
    /// Non-zero when the virtual camera system is active (`+0x000`, `0x010e5ad0`).
    pub view_cam_active_f: i32,
    /// 32-bit pointer to active `_VC_ROAD_DATA` array list (`+0x004`, `0x010e5ad4`).
    pub vc_road_ary_list: Sh2Ptr32,
    /// Runtime `_VC_FLAGS` bitfield (`+0x008`, `0x010e5ad8`).
    pub flags: u32,
    /// Through-door activation initialization flag (`+0x00C`, `0x010e5adc`).
    pub through_door_activate_init_f: i32,
    /// Active through-door camera rail state (`+0x010`, 48 bytes).
    pub through_door: ThroughDoorCamParam,
    /// Vertical screen half-angle in radians (`atan2(scr_h, scr_z) / 2.0`, `+0x040`, `0x010e5b10`).
    pub scr_half_ang_wy: f32,
    /// Horizontal screen half-angle in radians (`atan2(scr_w, scr_z) / 2.0`, `+0x044`, `0x010e5b14`).
    pub scr_half_ang_wx: f32,
    /// User/scripted camera translation limits (`+0x048`, 16 bytes, `0x010e5b18`).
    pub user_cam_mv_prm: CameraMoveParam,
    /// VU0 16-byte alignment padding (`+0x058`, 8 bytes).
    pub _pad0: [u8; 8],
    /// Target camera position in PS2 world mm (`+0x060`, `0x010e5b30`).
    pub cam_tgt_pos: [f32; 4],
    /// Current camera position in PS2 world mm (`+0x070`, `0x010e5b40`).
    pub cam_pos: [f32; 4],
    /// Camera movement direction angle (`cam_mv_ang_y` at `+0x080`, `0x010e5b50`, 16-byte slot).
    pub cam_mv_ang: [f32; 4],
    /// Current camera velocity in PS2 mm/s (`+0x090`, `0x010e5b60`).
    pub cam_velo: [f32; 4],
    /// Previous frame exclusion radius in mm (`-1.0` when uninitialized, `+0x0A0`, `0x010e5b70`).
    pub old_cam_excl_area_r: f32,
    /// Ideal camera height in PS2 world mm (`chara_top_y - 200.0` indoor / `- 450.0` outdoor, `+0x0A4`, `0x010e5b74`).
    pub ideal_cam_pos_h: f32,
    /// User/scripted watch angular limits (`+0x0A8`, 16 bytes, `0x010e5b78`).
    pub user_watch_mv_prm: CameraWatchParam,
    /// VU0 16-byte alignment padding (`+0x0B8`, 8 bytes).
    pub _pad1: [u8; 8],
    /// Watch look-at target position in PS2 world mm (`+0x0C0`, `0x010e5b90`).
    pub watch_tgt_pos: [f32; 4],
    /// Maximum allowed watch target Y in PS2 world mm (`50000.0` default, `+0x0D0`, `0x010e5ba0`).
    pub watch_tgt_max_y: f32,
    /// Target camera roll angle in radians (`+0x0D4`, `0x010e5ba4`).
    pub watch_tgt_ang_z: f32,
    /// VU0 16-byte alignment padding (`+0x0D8`, 8 bytes).
    pub _pad2: [u8; 8],
    /// Final camera matrix Euler angles `[pitch_x, yaw_y, roll_z, 0]` in radians (`+0x0E0`, `0x010e5bb0`).
    pub cam_mat_ang: [f32; 4],
    /// Final camera world transform matrix (`+0x0F0`, 64 bytes, `0x010e5bc0`).
    pub cam_mat: Matrix4x4,
    /// Offset camera Euler angles relative to `base_cam_ang` in radians (`+0x130`, `0x010e5c00`).
    pub ofs_cam_ang: [f32; 4],
    /// Offset camera angular velocity in rad/s (`+0x140`, `0x010e5c10`).
    pub ofs_cam_ang_spd: [f32; 4],
    /// Base camera Euler angles in radians (`+0x150`, `0x010e5c20`).
    pub base_cam_ang: [f32; 4],
    /// Previous frame base camera Euler angles in radians (`+0x160`, `0x010e5c30`).
    pub old_base_cam_ang: [f32; 4],
    /// Character-to-ideal-camera chase yaw angle in radians (`+0x170`, `0x010e5c40`).
    pub cam_chara2ideal_ang_y: f32,
    /// VU0 16-byte alignment padding (`+0x174`, 12 bytes).
    pub _pad3: [u8; 12],
    /// Camera target velocity in PS2 mm/s (`+0x180`, `0x010e5c50`).
    pub cam_tgt_velo: [f32; 4],
    /// Camera target horizontal movement angle in radians (`+0x190`, `0x010e5c60`).
    pub cam_tgt_mv_ang_y: f32,
    /// Camera target horizontal speed in mm/s (`+0x194`, `0x010e5c64`).
    pub cam_tgt_spd: f32,
    /// VU0 16-byte alignment padding (`+0x198`, 8 bytes).
    pub _pad4: [u8; 8],
    /// Subject character world position in PS2 mm (`+0x1A0`, `0x010e5c70`).
    pub chara_pos: [f32; 4],
    /// Character bottom Y in PS2 mm (`pos.y` standing, `pos.y - 250.0` crouching, `+0x1B0`, `0x010e5c80`).
    pub chara_bottom_y: f32,
    /// Character top Y in PS2 mm (`pos.y - 925.0`, `+0x1B4`, `0x010e5c84`).
    pub chara_top_y: f32,
    /// Character center Y in PS2 mm (`(bottom + top) / 2.0`, `+0x1B8`, `0x010e5c88`).
    pub chara_center_y: f32,
    /// Character ground Y in PS2 mm (`+0x1BC`, `0x010e5c8c`).
    pub chara_grnd_y: f32,
    /// Character head world position in PS2 mm (`+0x1C0`, `0x010e5c90`).
    pub chara_head_pos: [f32; 4],
    /// Character horizontal movement speed in mm/s (`+0x1D0`, `0x010e5ca0`).
    pub chara_mv_spd: f32,
    /// Character movement heading angle in radians (`+0x1D4`, `0x010e5ca4`).
    pub chara_mv_ang_y: f32,
    /// Character yaw angular speed in rad/s (`+0x1D8`, `0x010e5ca8`).
    pub chara_ang_spd_y: f32,
    /// Character facing / eye yaw angle in radians (`+0x1DC`, `0x010e5cac`).
    pub chara_eye_ang_y: f32,
    /// Character eye angle width in radians (`0x40060a92` ≈ `2.0943952` = `120°`, `+0x1E0`, `0x010e5cb0`).
    pub chara_eye_ang_wy: f32,
    /// Character watch Y parameter (`+0x1E4`, `0x010e5cb4`).
    pub chara_watch_y: f32,
    /// Fixed-angle camera flip/warp state (`+0x1E8`, 16 bytes, `0x010e5cb8`).
    pub fix_cam_mng: FixCamManager,
    /// VU0 16-byte alignment padding (`+0x1F8`, 8 bytes).
    pub _pad5: [u8; 8],
    /// Locus-circle camera orbit state (`+0x200`, 32 bytes, `0x010e5cd0`).
    pub cir_cam_mng: CirCamManager,
}

pub type Camera = CameraState;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_camera_sizes_and_offsets() {
        assert_eq!(size_of::<CameraMoveParam>(), 16);
        assert_eq!(size_of::<CameraWatchParam>(), 16);
        assert_eq!(size_of::<ThroughDoorCamParam>(), 48);
        assert_eq!(size_of::<FixCamManager>(), 16);
        assert_eq!(size_of::<CirCamManager>(), 32);
        assert_eq!(size_of::<CameraState>(), 0x220);

        // Exact VU0-aligned offsets verified against SLUS_202.28 vcWork (0x010e5ad0).
        assert_eq!(offset_of!(CameraState, vc_road_ary_list), 0x004);
        assert_eq!(offset_of!(CameraState, flags), 0x008);
        assert_eq!(offset_of!(CameraState, through_door), 0x010);
        assert_eq!(offset_of!(CameraState, scr_half_ang_wy), 0x040);
        assert_eq!(offset_of!(CameraState, scr_half_ang_wx), 0x044);
        assert_eq!(offset_of!(CameraState, user_cam_mv_prm), 0x048);
        assert_eq!(offset_of!(CameraState, cam_tgt_pos), 0x060);
        assert_eq!(offset_of!(CameraState, cam_pos), 0x070);
        assert_eq!(offset_of!(CameraState, cam_mv_ang), 0x080);
        assert_eq!(offset_of!(CameraState, cam_velo), 0x090);
        assert_eq!(offset_of!(CameraState, old_cam_excl_area_r), 0x0A0);
        assert_eq!(offset_of!(CameraState, ideal_cam_pos_h), 0x0A4);
        assert_eq!(offset_of!(CameraState, user_watch_mv_prm), 0x0A8);
        assert_eq!(offset_of!(CameraState, watch_tgt_pos), 0x0C0);
        assert_eq!(offset_of!(CameraState, watch_tgt_max_y), 0x0D0);
        assert_eq!(offset_of!(CameraState, watch_tgt_ang_z), 0x0D4);
        assert_eq!(offset_of!(CameraState, cam_mat_ang), 0x0E0);
        assert_eq!(offset_of!(CameraState, cam_mat), 0x0F0);
        assert_eq!(offset_of!(CameraState, ofs_cam_ang), 0x130);
        assert_eq!(offset_of!(CameraState, ofs_cam_ang_spd), 0x140);
        assert_eq!(offset_of!(CameraState, base_cam_ang), 0x150);
        assert_eq!(offset_of!(CameraState, old_base_cam_ang), 0x160);
        assert_eq!(offset_of!(CameraState, cam_chara2ideal_ang_y), 0x170);
        assert_eq!(offset_of!(CameraState, cam_tgt_velo), 0x180);
        assert_eq!(offset_of!(CameraState, cam_tgt_mv_ang_y), 0x190);
        assert_eq!(offset_of!(CameraState, cam_tgt_spd), 0x194);
        assert_eq!(offset_of!(CameraState, chara_pos), 0x1A0);
        assert_eq!(offset_of!(CameraState, chara_bottom_y), 0x1B0);
        assert_eq!(offset_of!(CameraState, chara_top_y), 0x1B4);
        assert_eq!(offset_of!(CameraState, chara_center_y), 0x1B8);
        assert_eq!(offset_of!(CameraState, chara_grnd_y), 0x1BC);
        assert_eq!(offset_of!(CameraState, chara_head_pos), 0x1C0);
        assert_eq!(offset_of!(CameraState, chara_mv_spd), 0x1D0);
        assert_eq!(offset_of!(CameraState, chara_mv_ang_y), 0x1D4);
        assert_eq!(offset_of!(CameraState, chara_ang_spd_y), 0x1D8);
        assert_eq!(offset_of!(CameraState, chara_eye_ang_y), 0x1DC);
        assert_eq!(offset_of!(CameraState, chara_eye_ang_wy), 0x1E0);
        assert_eq!(offset_of!(CameraState, chara_watch_y), 0x1E4);
        assert_eq!(offset_of!(CameraState, fix_cam_mng), 0x1E8);
        assert_eq!(offset_of!(CameraState, cir_cam_mng), 0x200);
    }
}
