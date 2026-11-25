//! Angela's Blazing Staircase Fire & Rising Smoke (`SH2-100`).
//!
//! Ported 1:1 from:
//! - `Event/stage/stg_hotel_fire.c` (`SLPM_123.45` `EvProgAngelaInFire` `0x0022a960`)
//! - `Effect2/hh_class_fire_phenomenon_00.c` (`SLUS_202.28` `0x00280c00..0x00280c80`)
//! - `Effect2/hh_class_flame_00..03.c` (`SLUS_202.28` `0x00280ff0..0x00293a30`)
//! - `Effect/ef_smoke.c` (`SLUS_202.28` `0x0022d700..0x0022e5b0`)

use glam::Vec3;
use std::f32::consts::PI;

use super::FxRng;

/// PS2 30 Hz effect step (`0.033333335f32`, `hh_class_flame_00.c` `0x002813b0`).
pub const PS2_EFFECT_DT_SEC: f32 = 1.0 / 30.0;

/// Upward buoyancy acceleration scale in PS2 Y-down mm/s^2 (`-2450.0`, `hh_class_flame_00.c` `0x00281110`).
pub const FLAME_GRAVITY_ACCEL_MM_S2: f32 = -2450.0;

/// 4x4 sub-atlas UV rects (`[u_min, v_min, u_max, v_max]`) from `_square_stq_list` (`0x00377a50`)
/// for `fire00.tbn2`, `fire01.tbn2`, `flame003.tbn2`, `flame032.tbn2` (`512x512`).
pub const FLAME_UV_QUADRANTS: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.24804688, 0.24804688],
    [0.0, 0.25, 0.24804688, 0.49804688],
    [0.0, 0.5, 0.24804688, 0.748_046_9],
    [0.0, 0.75, 0.24804688, 0.998_046_9],
];

/// Parameter row from `_flame_init_parm_table` (`0x60` bytes per entry, `SLUS_202.28` `0x00377b50`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlameInitParams {
    pub particle_count: u32,
    pub axis_flags: [u32; 3],
    pub spread_init: f32,
    pub spread_delta: f32,
    pub buoyancy_scale: f32,
    pub alpha_peak: f32,
    pub base_scale: f32,
    pub life_base: f32,
    pub life_center_bonus: f32,
    pub vel_span: [i32; 3],
    pub pos_span: [i32; 3],
    pub base_rgba: [f32; 4],
}

/// Exact 8-entry `_flame_init_parm_table` (`SLUS_202.28` `0x00377b50`, shared by `flame_00`, `flame_01`, `flame_03`).
pub const FLAME_00_PARM_TABLE: [FlameInitParams; 8] = [
    FlameInitParams {
        particle_count: 80,
        axis_flags: [0, 0, 1],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.18,
        alpha_peak: 80.0,
        base_scale: 190.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [700, 100, 700],
        pos_span: [10, 250, 500],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 100,
        axis_flags: [0, 0, 1],
        spread_init: 1.0,
        spread_delta: -0.035,
        buoyancy_scale: 0.30,
        alpha_peak: 90.0,
        base_scale: 210.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [100, 100, 700],
        pos_span: [10, 350, 700],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 80,
        axis_flags: [1, 0, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.18,
        alpha_peak: 80.0,
        base_scale: 190.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [700, 100, 700],
        pos_span: [500, 250, 10],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 80,
        axis_flags: [0, 1, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.20,
        alpha_peak: 100.0,
        base_scale: 170.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [700, 100, 700],
        pos_span: [10, 600, 100],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 100,
        axis_flags: [0, 1, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.20,
        alpha_peak: 80.0,
        base_scale: 180.0,
        life_base: 0.4,
        life_center_bonus: 2.0,
        vel_span: [700, 100, 700],
        pos_span: [10, 700, 100],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 100,
        axis_flags: [0, 1, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.25,
        alpha_peak: 90.0,
        base_scale: 190.0,
        life_base: 0.4,
        life_center_bonus: 2.0,
        vel_span: [100, 100, 700],
        pos_span: [10, 600, 200],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 70,
        axis_flags: [0, 0, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.15,
        alpha_peak: 128.0,
        base_scale: 120.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [700, 100, 700],
        pos_span: [150, 200, 150],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
    FlameInitParams {
        particle_count: 100,
        axis_flags: [0, 0, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.30,
        alpha_peak: 100.0,
        base_scale: 200.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [900, 300, 900],
        pos_span: [400, 300, 400],
        base_rgba: [255.0, 255.0, 255.0, 0.0],
    },
];

/// Exact 4-entry `_flame_init_parm_table` for `hh_class_flame_02.c` (`SLUS_202.28` `0x003797e0`).
pub const FLAME_02_PARM_TABLE: [FlameInitParams; 4] = [
    FlameInitParams {
        particle_count: 12,
        axis_flags: [0, 0, 1],
        spread_init: 1.0,
        spread_delta: -0.01,
        buoyancy_scale: 0.09,
        alpha_peak: 50.0,
        base_scale: 600.0,
        life_base: 1.5,
        life_center_bonus: 1.8,
        vel_span: [100, 100, 800],
        pos_span: [10, 250, 500],
        base_rgba: [30.0, 30.0, 30.0, 0.0],
    },
    FlameInitParams {
        particle_count: 12,
        axis_flags: [1, 0, 0],
        spread_init: 1.0,
        spread_delta: -0.01,
        buoyancy_scale: 0.09,
        alpha_peak: 50.0,
        base_scale: 600.0,
        life_base: 1.5,
        life_center_bonus: 1.8,
        vel_span: [800, 100, 100],
        pos_span: [500, 250, 10],
        base_rgba: [30.0, 30.0, 30.0, 0.0],
    },
    FlameInitParams {
        particle_count: 8,
        axis_flags: [0, 1, 0],
        spread_init: 1.0,
        spread_delta: -0.03,
        buoyancy_scale: 0.10,
        alpha_peak: 40.0,
        base_scale: 600.0,
        life_base: 0.4,
        life_center_bonus: 1.8,
        vel_span: [1000, 100, 1000],
        pos_span: [100, 150, 100],
        base_rgba: [40.0, 40.0, 40.0, 0.0],
    },
    FlameInitParams {
        particle_count: 12,
        axis_flags: [0, 1, 0],
        spread_init: 1.2,
        spread_delta: -0.005,
        buoyancy_scale: 0.05,
        alpha_peak: 40.0,
        base_scale: 900.0,
        life_base: 2.5,
        life_center_bonus: 1.5,
        vel_span: [500, 100, 500],
        pos_span: [100, 250, 100],
        base_rgba: [40.0, 40.0, 40.0, 0.0],
    },
];

/// Effect2 flame class IDs from `hh_class_fire_phenomenon_00.c` (`SLUS_202.28` `0x00280c80`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FlameClassKind {
    None = 0x00,
    Flame00 = 0x2b,
    Flame01 = 0x2c,
    Flame02 = 0x2d,
    Flame03 = 0x2f,
}

impl FlameClassKind {
    #[must_use]
    pub const fn from_raw(id: u32) -> Self {
        match id {
            0x2b => Self::Flame00,
            0x2c => Self::Flame01,
            0x2d => Self::Flame02,
            0x2f => Self::Flame03,
            _ => Self::None,
        }
    }
}

/// Placement descriptor from `_post_info_normal` (`0x00376510`) / `_post_info_demo` (`0x00375de0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotelFirePostInfo {
    pub pos: [f32; 3],
    pub offset: [f32; 3],
    pub cls_0: u32,
    pub cls_1: u32,
    pub start_frame: f32,
    pub end_frame: f32,
    pub prm_0: f32,
    pub prm_1: f32,
    pub parm_idx_0: usize,
    pub parm_idx_1: usize,
}

/// `(world_pos, offset, cls_kind, start_frame, end_frame)` entry from `_post_info_demo` (`0x00375de0`).
pub type HotelFireDemoEntry = HotelFirePostInfo;

/// Full 23-entry `_post_info_demo` table from `SLUS_202.28` (`0x00375de0`).
pub const HOTEL_FIRE_POST_INFO_DEMO: [HotelFirePostInfo; 23] = [
    HotelFirePostInfo {
        pos: [-100550.0, -3600.0, -74800.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 5,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100550.0, -3700.0, -75600.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 1,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100400.0, 0.0, -79400.0],
        offset: [0.0, -700.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 7,
        parm_idx_1: 3,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -400.0, -78700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 1666.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -400.0, -78700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2d,
        cls_1: 0x2f,
        start_frame: 1665.0,
        end_frame: 2556.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -400.0, -78700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 2555.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -750.0, -78450.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 1666.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -750.0, -78450.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2d,
        cls_1: 0x2f,
        start_frame: 1665.0,
        end_frame: 2556.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -750.0, -78450.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 2555.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -1200.0, -78100.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 1666.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 4,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -1200.0, -77950.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2d,
        cls_1: 0x2f,
        start_frame: 1665.0,
        end_frame: 2556.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 4,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -1200.0, -78100.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 2555.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 4,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -2500.0, -76800.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100550.0, -2000.0, -76400.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 4,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99500.0, -2800.0, -76800.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 1,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99500.0, -2200.0, -76100.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 5,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99500.0, -4000.0, -74900.0],
        offset: [0.0, -700.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 7,
        parm_idx_1: 3,
    },
    HotelFirePostInfo {
        pos: [-99800.0, -2500.0, -75700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 6,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100200.0, -4500.0, -73700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 2,
        parm_idx_1: 1,
    },
    HotelFirePostInfo {
        pos: [-99750.0, -5600.0, -72600.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 2,
        parm_idx_1: 1,
    },
    HotelFirePostInfo {
        pos: [-99500.0, 0.0, -78400.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2c,
        cls_1: 0x00,
        start_frame: 0.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 6,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100300.0, -910.0, -77300.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2c,
        cls_1: 0x00,
        start_frame: 5000.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 3.5,
        parm_idx_0: 6,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99850.0, -900.0, -77300.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2c,
        cls_1: 0x00,
        start_frame: 5000.0,
        end_frame: 10000.0,
        prm_0: 0.0,
        prm_1: 4.0,
        parm_idx_0: 2,
        parm_idx_1: 2,
    },
];

/// Full 16-entry `_post_info_normal` table from `SLUS_202.28` (`0x00376510`).
pub const HOTEL_FIRE_POST_INFO_NORMAL: [HotelFirePostInfo; 16] = [
    HotelFirePostInfo {
        pos: [-100550.0, -3600.0, -74800.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 5,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100550.0, -3700.0, -75600.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 1,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -400.0, -78700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -750.0, -78450.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -1200.0, -78100.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 4,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100500.0, -2500.0, -76800.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 0,
        parm_idx_1: 0,
    },
    HotelFirePostInfo {
        pos: [-100550.0, -2000.0, -76400.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 4,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99500.0, -2800.0, -76800.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 1,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99500.0, -2200.0, -76100.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 5,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99500.0, -4000.0, -74900.0],
        offset: [0.0, -700.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 7,
        parm_idx_1: 3,
    },
    HotelFirePostInfo {
        pos: [-99800.0, -2500.0, -75700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 6,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100200.0, -4500.0, -73700.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 2,
        parm_idx_1: 1,
    },
    HotelFirePostInfo {
        pos: [-99750.0, -5600.0, -72600.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 2,
        parm_idx_1: 1,
    },
    HotelFirePostInfo {
        pos: [-99500.0, 0.0, -78400.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 6,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-100300.0, -910.0, -77300.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 6,
        parm_idx_1: 2,
    },
    HotelFirePostInfo {
        pos: [-99850.0, -900.0, -77300.0],
        offset: [0.0, -500.0, 0.0],
        cls_0: 0x2b,
        cls_1: 0x2f,
        start_frame: 0.0,
        end_frame: 0.0,
        prm_0: 0.0,
        prm_1: 0.0,
        parm_idx_0: 2,
        parm_idx_1: 1,
    },
];

/// Single flame particle (`hh_class_flame_00.c` `0x50`-byte record).
#[derive(Debug, Clone, Copy)]
pub struct FlameParticle {
    pub local_pos: Vec3,
    pub vel: Vec3,
    pub buoyancy_y: f32,
    pub rgba: [f32; 4],
    pub age: f32,
    pub scale: f32,
    pub spread: f32,
    pub life: f32,
    pub quadrant: u8,
    pub world_offset: Vec3,
}

impl FlameParticle {
    /// Initializes a particle from `params` (`Particle_Initialize` `0x00281110`).
    pub fn init(params: &FlameInitParams, rng: &mut FxRng) -> Self {
        let mut radius_ref = (params.pos_span[0] + params.pos_span[2]) as f32 * 0.5;
        if params.axis_flags[0] != 0 {
            radius_ref = params.pos_span[0] as f32;
        }
        if params.axis_flags[1] != 0 {
            radius_ref = params.pos_span[1] as f32;
        }
        if params.axis_flags[2] != 0 {
            radius_ref = params.pos_span[2] as f32;
        }
        let local_pos = Vec3::new(
            rng.centered_i32(params.pos_span[0]),
            rng.centered_i32(params.pos_span[1]),
            rng.centered_i32(params.pos_span[2]),
        );
        let vel = Vec3::new(
            rng.centered_i32(params.vel_span[0]),
            rng.centered_i32(params.vel_span[1]),
            rng.centered_i32(params.vel_span[2]),
        );
        let norm_r2 = (local_pos.length_squared() / (radius_ref * radius_ref).max(1.0)).min(1.0);
        let life = params.life_base + params.life_center_bonus * (1.0 - norm_r2);
        let quadrant = (rng.next_u32() & 3) as u8;
        Self {
            local_pos,
            vel,
            buoyancy_y: params.buoyancy_scale * FLAME_GRAVITY_ACCEL_MM_S2,
            rgba: params.base_rgba,
            age: 0.0,
            scale: params.base_scale,
            spread: params.spread_init,
            life,
            quadrant,
            world_offset: Vec3::ZERO,
        }
    }

    /// Evaluates `Particle_Motion` + `Particle_Calculator` (`0x002813b0`, `0x00281510`).
    pub fn step(&mut self, params: &FlameInitParams, dt: f32, rng: &mut FxRng) {
        if self.age > self.life {
            *self = Self::init(params, rng);
        }
        let quarter_life = self.life * 0.25;
        let inv_life = 1.0 / self.life.max(1e-4);
        let env = if self.age < quarter_life {
            self.age / quarter_life.max(1e-4)
        } else {
            1.0 - inv_life * (self.age - quarter_life) * 1.333_333_4
        };
        // `Particle_Calculator`: `x = vx * age * spread`, `y = vy * age + buoyancy * age^2`, `z = vz * age * spread`
        let t = self.age;
        self.world_offset = Vec3::new(
            self.vel.x * t * self.spread,
            self.vel.y * t + self.buoyancy_y * t * t,
            self.vel.z * t * self.spread,
        );
        if self.spread > 0.0 {
            self.spread += params.spread_delta * (dt / PS2_EFFECT_DT_SEC);
        }
        self.age += dt;
        self.rgba[3] = (params.alpha_peak * env).max(0.0);
        self.scale = params.base_scale * (1.0 - t * inv_life).max(0.0);
    }
}

/// Multi-layered flame emitter (`hh_class_flame_00..03.c`).
#[derive(Debug, Clone)]
pub struct FlameEmitter {
    pub class_kind: FlameClassKind,
    pub origin: Vec3,
    pub params: FlameInitParams,
    pub particles: Vec<FlameParticle>,
    pub elapsed: f32,
}

impl FlameEmitter {
    /// Creates a new flame emitter at `origin` with randomized initial particle phases.
    #[must_use]
    pub fn new(
        class_kind: FlameClassKind,
        origin: Vec3,
        params: FlameInitParams,
        rng: &mut FxRng,
    ) -> Self {
        let mut particles = Vec::with_capacity(params.particle_count as usize);
        for _ in 0..params.particle_count {
            let mut p = FlameParticle::init(&params, rng);
            p.age = p.life * ((rng.next_u32() % 100) as f32) * 0.01;
            particles.push(p);
        }
        Self {
            class_kind,
            origin,
            params,
            particles,
            elapsed: 0.0,
        }
    }

    /// Advances all particles by `dt` seconds.
    pub fn step(&mut self, dt: f32, rng: &mut FxRng) {
        self.elapsed += dt;
        for p in &mut self.particles {
            p.step(&self.params, dt, rng);
        }
    }
}

/// Native-upgrade glowing ember particle (`enable_fire_heat_haze = true`).
#[derive(Debug, Clone, Copy)]
pub struct FireEmber {
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub life: f32,
    pub size_mm: f32,
}

/// Native-upgrade flickering orange point light bounce (`enable_fire_heat_haze = true`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirePointLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub near_mm: f32,
    pub far_mm: f32,
}

/// Angela's staircase fire controller (`hh_class_fire_phenomenon_00.c` `0x00280c80`).
#[derive(Debug, Clone)]
pub struct FirePhenomenon {
    pub emitters: Vec<FlameEmitter>,
    pub embers: Vec<FireEmber>,
    pub time: f32,
    rng: FxRng,
}

fn params_for_class(cls: FlameClassKind, idx: usize) -> FlameInitParams {
    match cls {
        FlameClassKind::Flame02 => FLAME_02_PARM_TABLE[idx & 3],
        _ => FLAME_00_PARM_TABLE[idx & 7],
    }
}

impl FirePhenomenon {
    /// Creates the Lakeview Hotel staircase fire from all 16 entries of `_post_info_normal` (`0x00376510`),
    /// dispatching both `cls_0` at `pos` and `cls_1` at `pos + offset` (`hh_class_fire_phenomenon_00.c`).
    #[must_use]
    pub fn new_hotel_staircase(seed: u32) -> Self {
        let mut rng = FxRng::new(seed);
        let mut emitters = Vec::with_capacity(HOTEL_FIRE_POST_INFO_NORMAL.len() * 2);
        for entry in &HOTEL_FIRE_POST_INFO_NORMAL {
            let base = Vec3::from_array(entry.pos);
            let c0 = FlameClassKind::from_raw(entry.cls_0);
            if c0 != FlameClassKind::None {
                emitters.push(FlameEmitter::new(
                    c0,
                    base,
                    params_for_class(c0, entry.parm_idx_0),
                    &mut rng,
                ));
            }
            let c1 = FlameClassKind::from_raw(entry.cls_1);
            if c1 != FlameClassKind::None {
                emitters.push(FlameEmitter::new(
                    c1,
                    base + Vec3::from_array(entry.offset),
                    params_for_class(c1, entry.parm_idx_1),
                    &mut rng,
                ));
            }
        }
        Self {
            emitters,
            embers: Vec::new(),
            time: 0.0,
            rng,
        }
    }

    /// Steps all flame emitters and (when `native_upgrade` is true) glowing embers.
    pub fn step(&mut self, dt: f32, native_upgrade: bool) {
        self.time += dt;
        for emitter in &mut self.emitters {
            emitter.step(dt, &mut self.rng);
        }
        if native_upgrade {
            for e in &mut self.embers {
                e.pos += e.vel * dt;
                e.vel.x += (self.time * 5.0 + e.pos.y * 0.01).sin() * 120.0 * dt;
                e.age += dt;
            }
            self.embers.retain(|e| e.age < e.life);
            if self.embers.len() < 48 {
                if let Some(em) = self.emitters.first() {
                    self.embers.push(FireEmber {
                        pos: em.origin
                            + Vec3::new(
                                self.rng.sway(-350.0, 350.0),
                                self.rng.sway(-100.0, 50.0),
                                self.rng.sway(-350.0, 350.0),
                            ),
                        vel: Vec3::new(
                            self.rng.sway(-80.0, 80.0),
                            -self.rng.sway(450.0, 950.0),
                            self.rng.sway(-80.0, 80.0),
                        ),
                        age: 0.0,
                        life: self.rng.sway(0.8, 1.8),
                        size_mm: self.rng.sway(14.0, 28.0),
                    });
                }
            }
        } else {
            self.embers.clear();
        }
    }

    /// Returns the flickering orange dynamic point-light bounce when `native_upgrade` is enabled.
    #[must_use]
    pub fn flickering_point_light(&self, native_upgrade: bool) -> Option<FirePointLight> {
        if !native_upgrade {
            return None;
        }
        let origin = self.emitters.first()?.origin;
        let flicker = 0.82 + 0.12 * (self.time * 13.7).sin() + 0.06 * (self.time * 29.3).cos();
        Some(FirePointLight {
            position: [origin.x, origin.y - 600.0, origin.z],
            color: [1.0 * flicker, 0.42 * flicker, 0.08 * flicker],
            near_mm: 1200.0,
            far_mm: 6500.0,
        })
    }
}

/// 3 UV tiles for `ef_smoke.c` `SetSmokeSTValue` (`0x0022dc20`, 8x8 grid `0.125` step).
pub const SMOKE_UV_TILES: [[f32; 4]; 3] = [
    [2.0 * 0.125, 7.0 * 0.125, 3.0 * 0.125, 8.0 * 0.125],
    [3.0 * 0.125, 6.0 * 0.125, 4.0 * 0.125, 7.0 * 0.125],
    [3.0 * 0.125, 7.0 * 0.125, 4.0 * 0.125, 8.0 * 0.125],
];

/// Rising smoke particle (`Effect/ef_smoke.c` `0x0022d700..0x0022e5b0`).
#[derive(Debug, Clone, Copy)]
pub struct SmokeParticle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub life: f32,
    pub size_x: f32,
    pub size_y: f32,
    pub rgba: [f32; 4],
    pub uv_rect: [f32; 4],
    pub active: bool,
}

/// Smoke emitter (`Effect/ef_smoke.c`, 16 particles per emitter).
#[derive(Debug, Clone)]
pub struct SmokeSystem {
    pub origin: Vec3,
    pub particles: Vec<SmokeParticle>,
}

impl SmokeSystem {
    /// Creates a 16-particle smoke burst at `origin` (`CreateSmokeObject` `0x0022d7f0`).
    #[must_use]
    pub fn new(origin: Vec3, rng: &mut FxRng) -> Self {
        let mut particles = Vec::with_capacity(16);
        for _ in 0..16 {
            let life = rng.sway(1.5, 1.5) + 5.95;
            let size_x = (rng.next_f32() * 1.05 + 1.0) * 300.0;
            let size_y = (rng.next_f32() * 1.0 + 1.05) * 300.0;
            let vel = Vec3::new(
                rng.sway(-110.0, 110.0),
                rng.next_f32() * 100.0 + 40.0,
                rng.sway(-110.0, 110.0),
            );
            let tile = (rng.next_u32() % 3) as usize;
            particles.push(SmokeParticle {
                pos: origin
                    + Vec3::new(
                        rng.sway(-300.0, 300.0),
                        rng.sway(-80.0, 80.0),
                        rng.sway(-300.0, 300.0),
                    ),
                vel,
                age: 0.0,
                life,
                size_x,
                size_y,
                rgba: [112.0, 104.0, 104.0, 0.0],
                uv_rect: SMOKE_UV_TILES[tile],
                active: true,
            });
        }
        Self { origin, particles }
    }

    /// Evaluates `RenewSmokeRGBA`, `DiffuseSmoke`, `RenewSmokePos`, and `CountSmokeDiffusionTimer`.
    pub fn step(&mut self, dt: f32) {
        for p in &mut self.particles {
            if !p.active {
                continue;
            }
            p.age += dt;
            if p.age > p.life {
                p.age = 0.0;
                p.active = false;
                continue;
            }
            let ratio = p.age / p.life.max(1e-4);
            let rgb_s = ((ratio + 1.0) * 0.5 * PI).sin();
            let alpha_s = (ratio * PI).sin();
            p.rgba = [rgb_s * 112.0, rgb_s * 104.0, rgb_s * 104.0, alpha_s * 20.0];
            p.pos = self.origin + Vec3::new(p.age * p.vel.x, -p.age * p.vel.y, p.age * p.vel.z);
        }
    }

    /// Returns `true` when all 16 smoke particles have finished (`IsFinishSmoke` `0x0022e5b0`).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.particles.iter().all(|p| !p.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flame_particle_matches_slus_002813b0_envelope() {
        let mut rng = FxRng::new(42);
        let params = &FLAME_00_PARM_TABLE[0];
        let mut p = FlameParticle::init(params, &mut rng);
        p.life = 1.0;
        p.age = 0.25;
        p.step(params, PS2_EFFECT_DT_SEC, &mut rng);
        assert!((p.rgba[3] - params.alpha_peak).abs() < 1e-3);
        assert!((p.scale - params.base_scale * 0.75).abs() < 1e-3);
    }

    #[test]
    fn fire_phenomenon_gates_native_upgrade_embers_and_light() {
        let mut fire = FirePhenomenon::new_hotel_staircase(1);
        fire.step(PS2_EFFECT_DT_SEC, false);
        assert!(fire.embers.is_empty());
        assert!(fire.flickering_point_light(false).is_none());

        fire.step(PS2_EFFECT_DT_SEC, true);
        assert!(!fire.embers.is_empty());
        assert!(fire.flickering_point_light(true).is_some());
    }

    #[test]
    fn smoke_system_rises_upward_in_ps2_y_down_and_finishes() {
        let mut rng = FxRng::new(7);
        let mut smoke = SmokeSystem::new(Vec3::ZERO, &mut rng);
        smoke.step(0.5);
        assert!(smoke.particles[0].pos.y < 0.0);
        assert!(smoke.particles[0].rgba[3] > 0.0);
        smoke.step(10.0);
        assert!(smoke.is_finished());
    }
}
