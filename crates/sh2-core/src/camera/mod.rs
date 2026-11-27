//! Silent Hill 2 authentic PS2 camera system and subframe interpolation.
//!
//! Ground truth references:
//! - PS2 beta `view/vc_main.c`, `vc_calc.c`, `vc_newest.c`, `vw_calc.c`, `vb_main.c`
//! - `crates/sh2-core/src/types/camera.rs` (`CameraState` / `_VC_WORK`)
//! - Strict compliance: Exactly mirrors PS2 camera pathing, fixed cinematic angles,
//!   chase rails, room `.cam` boundaries, and right-stick head/search panning.
//!   Zero over-the-shoulder modifications.

/// Re-export authentic camera movement types from assets for engine consumers.
pub use sh2_assets::formats::cam::CamMovementType as CameraMovementType;
pub use sh2_assets::formats::cam::CamSourceKind as CameraSourceKind;

/// Default PS2 projection plane distance (`448.0`) used when a `.cam` record specifies `projection == 0.0`
/// and as the PS2 vertical screen height (`scr_sy = 448.0`) in screen-framing math
/// (`SLUS_202.28` `vcSetProjectionValue` `0x0019db80` / `sh2pc.exe` `FUN_0051d2b0` `0x0051d2b0`).
pub const PS2_DEFAULT_PROJECTION_Z: f32 = 448.0;

/// Non-bone fallback player head height offset in PS2 world mm (`-950.0` along Y-down)
/// (`SLUS_202.28` `vcMoveAndSetCamera` `0x0019df50` `view/vc_util.c:219` /
/// `sh2pc.exe` `FUN_005193a0` `0x005193a0` `sh2pc_00510000.c:7090`).
pub const CHARA_FALLBACK_HEAD_HEIGHT_MM: f32 = 950.0;

/// Retail standing player top height (`chara_top_y = player_pos.y - 937.5`) in PS2 world mm
/// (`sh2pc.exe` `FUN_005193a0` `0x005193a0` `sh2pc_00510000.c:7097`; prototype `SLUS_202.28`
/// `vcMoveAndSetCamera` `0x0019df50` `view/vc_util.c:230` used `925.0`).
pub const CHARA_TOP_HEIGHT_MM: f32 = 937.5;

/// Standing player vertical center offset (`(0.0 + 937.5) * 0.5 = 468.75` mm) derived from
/// `vcSetSubjChara` (`SLUS_202.28` `0x00195f00` `view/vc_main.c:355` /
/// `sh2pc.exe` `FUN_00519b90` `0x00519b90` `sh2pc_00510000.c:7306`).
pub const CHARA_CENTER_HEIGHT_MM: f32 = CHARA_TOP_HEIGHT_MM * 0.5;

/// Maximum XZ scan distance (`7500.0` mm) used as the sentinel when no enemy is in range
/// (`SLUS_202.28` `vcSetNearestEnemyDataInVC_WORK` `0x00196f20` `view/vc_main.c:844-845,856` /
/// `sh2pc.exe` `FUN_0051a270` `0x0051a270` `sh2pc_00510000.c:7587-7588`).
pub const NEAREST_ENEMY_MAX_DIST_MM: f32 = 7500.0;

/// Default vertical watch-target offset (`-450.0` mm along Y-down) used for doorway/far-watch
/// framing when no enemy or item target is active (`SLUS_202.28` `vcPreSetDataInVC_WORK`
/// `0x00196be0` `view/vc_main.c:775` and `vcMakeFarWatchTgtPos` `0x00199760` `view/vc_main.c:2193`).
pub const DEFAULT_WATCH_TARGET_Y_OFFSET_MM: f32 = -450.0;

/// Authentic PS2 Chase, Settle, vertical Y-tracking, and base-angle camera algorithms (`SH2-133`).
pub mod chase;
/// Authentic PS2 character-front camera exclusion zone pushout (`SH2-133`).
pub mod exclusion;
/// Authentic PS2 FixedAngle camera positioning and 180° search-button warp (`SH2-133`).
pub mod fixed_angle;
/// Authentic PS2 velocity-bounded kinematics (`vw_calc.c` / `vc_main.c`, `SH2-133`).
pub mod kinematics;
/// Authentic PS2 `LOCUS_CIRCLE` (Mode 4) and `THROUGH_DOOR` (Mode 5) camera motion algorithms (`SH2-51`, `SH2-134`).
pub mod locus_door;
/// Authentic PS2 Mode 3 (`SELF_VIEW`) camera matrix breathing/handheld noise (`SH2-134`).
pub mod noise;
/// Multi-slot resident outdoor `.cam` road bank (`<area>GB.cam` + per-tile `.cam`, `SH2-184`).
pub mod road_bank;
/// Authentic PS2 road priority gates (`vcRetPrioStgEvnt`), nearest-item search, and hysteresis (`SH2-134`).
pub mod road_priority;
/// Authentic PS2 Mode 3 (`SELF_VIEW`) and `FarWatch` search-view algorithms (`SH2-134`).
pub mod self_view;

mod controller;
mod fallback;
mod step;
mod tick;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_outdoor_cam;
#[cfg(test)]
mod tests_parity;

pub use controller::{ps2_wvs_matrix_from_look_at, CameraSnapshot, Ps2CameraController};

/// Fixed 60 Hz simulation timestep in seconds (`1.0 / 60.0`).
pub(super) const SIM_DT_SECS: f32 = 1.0 / 60.0;
