#![forbid(unsafe_code)]
//! Parser for Silent Hill 2 `.cam` binary camera volume definition files.
//!
//! Authentic layout:
//! Each `.cam` file contains an array of 128-byte road records (`_VC_ROAD_DATA`)
//! terminated by a record with flag `VC_RD_END_DATA_F` (`0x0001`).
//!
//! Ground truth references:
//! - PS2 beta `view/vc_main.c`, `sh2cam.h`, `sh2cam.c` (`vcConvertCamFile`)
//! - Sentinel values (e.g. `ofs_watch_hy == 10000.0` -> `-300.0`, `projection == 0.0` -> `448.0`)

mod area;
mod parser;
mod path;
mod types;

#[cfg(test)]
mod tests;

pub use area::RoadArea;
pub use parser::parse_cam;
pub use path::{
    global_area_cam_rel_path, normalize_room_cam_rel_path, CamSourceKind, ResolvedCamAsset,
};
pub use types::{
    AreaSizeType, CamFile, CamMovementParams, CamMovementType, ChaseCamParam, FixAngCamParam,
    LocusCircleCamParam, RoadRecord, RoadType, SettleCamParam, ThroughDoorCamParam,
    VC_RD_CAM_LIKE_SETTLE_F, VC_RD_END_DATA_F, VC_RD_INVALID_SV_F, VC_RD_LIM_UP_FAR_VIEW_F,
    VC_RD_MARGE_ROAD_F, VC_RD_NOT_WARP_F, VC_RD_NO_EXTRA_AREA_F, VC_RD_NO_FRONT_FLIP_F,
    VC_RD_USE_NEAR_ENEMY_F, VC_RD_USE_NO_ENEMY_F, VC_RD_WARP_IN_F, VC_RD_WARP_IN_OUT_FS,
    VC_RD_WARP_OUT_F,
};
