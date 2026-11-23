//! Silent Hill 2 authentic PS2 camera system and subframe interpolation.
//!
//! Ground truth references:
//! - PS2 beta `view/vc_main.c`, `vc_calc.c`, `vc_newest.c`, `vw_calc.c`, `vb_main.c`
//! - `crates/sh2-core/src/types/camera.rs` (`CameraState` / `_VC_WORK`)
//! - Strict compliance: Exactly mirrors PS2 camera pathing, fixed cinematic angles,
//!   chase rails, room `.cam` boundaries, and right-stick head/search panning.
//!   Zero over-the-shoulder modifications.

use crate::types::camera::{CameraState, FixCamManager};
use glam::{Mat4, Vec3};
use sh2_assets::formats::cam::{
    AreaSizeType, CamFile, CamSourceKind, RoadRecord, VC_RD_LIM_UP_FAR_VIEW_F,
    VC_RD_NO_FRONT_FLIP_F,
};
use sh2_assets::AssetPathResolver;
use std::f32::consts::PI;

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
/// Authentic PS2 road priority gates (`vcRetPrioStgEvnt`), nearest-item search, and hysteresis (`SH2-134`).
pub mod road_priority;
/// Authentic PS2 Mode 3 (`SELF_VIEW`) and `FarWatch` search-view algorithms (`SH2-134`).
pub mod self_view;
#[cfg(test)]
mod tests_parity;

/// Fixed 60 Hz simulation timestep in seconds (`1.0 / 60.0`).
const SIM_DT_SECS: f32 = 1.0 / 60.0;

/// Snapshot of camera transforms for sub-frame alpha interpolation between 60 Hz ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSnapshot {
    /// World position (Y pointing down as in original PS2 engine).
    pub eye: Vec3,
    /// Look-at target position.
    pub target: Vec3,
    /// Camera up vector.
    pub up: Vec3,
    /// Field of view in radians.
    pub fov_y: f32,
}

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, -1700.0, -3000.0),
            target: Vec3::new(0.0, -1700.0, 0.0),
            up: Vec3::new(0.0, -1.0, 0.0),
            fov_y: kinematics::fov_y_from_ps2_projection_unclamped(
                kinematics::PS2_DEFAULT_PROJECTION_SCR_Z,
            ),
        }
    }
}

/// Camera controller maintaining the authentic PS2 camera state and interpolation.
pub struct Ps2CameraController {
    /// Internal low-level camera state matching runtime `_VC_WORK`.
    pub raw_state: CameraState,
    /// Active movement type.
    pub mv_type: CameraMovementType,
    /// Currently active road record index in `cam_file`, if any.
    pub active_road_index: Option<usize>,
    /// Loaded `.cam` trigger volume definition file for the active room.
    pub cam_file: Option<CamFile>,
    /// Canonical lowercase room identifier of the currently loaded `.cam` file (e.g. `"er01"`, `"ob01"`, `"rr21"`).
    pub active_room_id: Option<String>,
    /// Source pack (`Sh2eeOverride` vs `VanillaRetail`) of the currently loaded `.cam` file (`SH2-50`).
    pub active_cam_source: Option<CamSourceKind>,
    /// Previous tick snapshot for interpolation.
    pub prev_snapshot: CameraSnapshot,
    /// Current tick snapshot.
    pub curr_snapshot: CameraSnapshot,
    /// Search camera neck yaw angle in radians (`PlayerGetNeckAngleY`, `_DAT_012027f4`).
    pub search_yaw_offset: f32,
    /// Search camera neck pitch angle in radians (`PlayerGetNeckAngleX`, `_DAT_012027f0`).
    pub search_pitch_offset: f32,
    /// Target camera eye position (synced with `raw_state.cam_pos`).
    pub ideal_eye: Vec3,
    /// Target camera look-at position (synced with watch target along `cam_mat_ang`).
    pub ideal_target: Vec3,
    /// Dynamic viewport aspect-ratio rail clearance and intra-room warp transition configuration (`SH2-53`).
    pub aspect_rail_config: crate::widescreen_rail::AspectRailConfig,
    /// Active doorway transit session (`_VC_THROUGH_DOOR_CAM_PARAM`, `SLUS_202.28` `0x00196e00`, `sh2pc.exe` `0x0051b0c0`).
    pub through_door_session: locus_door::ThroughDoorSession,
    /// Nearest world item within `5000.0` mm (`vcSetNearestItemDataInVC_WORK`, `0x001971a0`).
    pub nearest_item: road_priority::NearestItemData,
    /// Optional nearest enemy `(xz_dist_mm, center_y_mm)` for `vcRetSelfViewEffectRate` & `vcMakeFarWatchTgtPos`.
    pub nearest_enemy: Option<(f32, f32)>,
    /// Elapsed Mode 3 (`SELF_VIEW`) breathing noise timer in seconds (`_vcSelfViewTimer`, `0x0019d8e0`).
    pub self_view_timer_sec: f32,
    /// Active stage ID (`_stage + 0x24`) for `vcMakeFarWatchTgtPos`, `vcAdjustWatchYLimitHighWhenFarView`, and `vcRetPrioStgEvnt`.
    pub stage_id: i32,
    /// `true` when James is rowing the Toluca Lake boat (`_sh2jms->flags & 0x20000`).
    pub on_boat: bool,
    /// `true` while the L2 / Search View button is held (`PlayerSearchVIewButtonCheck`, `VC_WORK.flags & 0x200`).
    pub search_button_pressed: bool,
    /// Whether the most recent camera tick moved smoothly (`vcRetSmoothCamMvF` `0x00196360`).
    pub last_move_smooth: bool,
}

impl Default for Ps2CameraController {
    fn default() -> Self {
        Self::new()
    }
}

impl Ps2CameraController {
    /// Create a new camera controller initialized to PS2 default framing.
    pub fn new() -> Self {
        let initial = CameraSnapshot::default();
        let raw_state = CameraState {
            old_cam_excl_area_r: -1.0,
            watch_tgt_max_y: 50_000.0,
            ..Default::default()
        };
        Self {
            raw_state,
            mv_type: CameraMovementType::Chase,
            active_road_index: None,
            cam_file: None,
            active_room_id: None,
            active_cam_source: None,
            prev_snapshot: initial,
            curr_snapshot: initial,
            search_yaw_offset: 0.0,
            search_pitch_offset: 0.0,
            ideal_eye: initial.eye,
            ideal_target: initial.target,
            aspect_rail_config: crate::widescreen_rail::AspectRailConfig::default(),
            through_door_session: locus_door::ThroughDoorSession::default(),
            nearest_item: road_priority::NearestItemData::default(),
            nearest_enemy: None,
            self_view_timer_sec: 0.0,
            stage_id: 0,
            on_boat: false,
            search_button_pressed: false,
            last_move_smooth: true,
        }
    }

    /// Starts a Mode 5 (`THROUGH_DOOR`) doorway transition session (`vcSetTHROUGH_DOOR_CAM_PARAM_in_VC_WORK`, `0x00196e00`).
    pub fn activate_through_door(&mut self, player_pos: Vec3, player_yaw: f32, is_outdoor: bool) {
        self.through_door_session
            .activate(player_pos, player_yaw, is_outdoor);
    }

    /// Ends an active Mode 5 (`THROUGH_DOOR`) doorway transition session (`0x00196e6c`).
    pub fn deactivate_through_door(&mut self) {
        self.through_door_session.deactivate();
    }

    /// Scans up to 20 nearby world items within `5000.0` mm (`vcSetNearestItemDataInVC_WORK`, `0x001971a0`).
    pub fn update_nearest_item(
        &mut self,
        player_pos: Vec3,
        player_status_flags: u32,
        candidates: &[road_priority::CameraItemCandidate],
    ) {
        self.nearest_item =
            road_priority::find_nearest_item_data(player_pos, player_status_flags, candidates);
    }

    /// Configure dynamic viewport aspect-ratio rail clearance (PS2 world mm) and intra-room
    /// `VC_RD_WARP_OUT_F` / `VC_RD_WARP_IN_F` smoothing policy (`SH2-53`).
    pub fn set_aspect_rail_config(&mut self, config: crate::widescreen_rail::AspectRailConfig) {
        self.aspect_rail_config = config;
    }

    /// Enable dynamic widescreen rail wall clearance for `viewport_aspect` (`W / H`, dimensionless),
    /// scaling `vcAdjustXzInLimAreaUsingMIN_IN_ROAD_DIST` (`0x0019b700`) margins when `aspect > 4.0 / 3.0` (`SH2-53`).
    pub fn enable_dynamic_widescreen_clearance(&mut self, viewport_aspect: f32) {
        let policy = self.aspect_rail_config.transition_policy;
        self.aspect_rail_config =
            crate::widescreen_rail::AspectRailConfig::dynamic_widescreen(viewport_aspect)
                .with_transition_policy(policy);
    }

    /// Set the intra-room `.cam` volume warp transition policy (`SH2-53`, preserving instant cuts on `reset_to_player`).
    pub fn set_intra_room_transition_policy(
        &mut self,
        policy: crate::widescreen_rail::IntraRoomTransitionPolicy,
    ) {
        self.aspect_rail_config.transition_policy = policy;
    }

    /// Load or swap the active room's `.cam` trigger volume definitions.
    pub fn set_cam_file(&mut self, cam_file: Option<CamFile>) {
        if cam_file.is_none() {
            self.active_room_id = None;
            self.active_cam_source = None;
        }
        self.cam_file = cam_file;
        self.active_road_index = None;
    }

    /// Load or swap the active room's `.cam` trigger volume definitions along with its room ID and source pack (`SH2-50`).
    pub fn set_cam_file_with_metadata(
        &mut self,
        cam_file: Option<CamFile>,
        room_id: Option<String>,
        source: Option<CamSourceKind>,
    ) {
        self.cam_file = cam_file;
        self.active_room_id = room_id;
        self.active_cam_source = source;
        self.active_road_index = None;
    }

    /// Automatically resolves and ingests the `.cam` trigger volume file for `room_or_rel_path`
    /// (`"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`, or `"bg/er/er01.cam"`), prioritizing
    /// SH2:EE widescreen/high-framerate overrides (`sh2e/bg/<area>/<room>.cam` and
    /// `sh2e/bg2/<area>/<room>.cam`) when `prefer_sh2ee` is `true` before falling back to
    /// vanilla retail `data/bg/...` (`SH2-50`).
    ///
    /// Returns `true` if a valid `.cam` file was found and loaded.
    pub fn load_room_cam_with_resolver(
        &mut self,
        resolver: &AssetPathResolver,
        room_or_rel_path: &str,
        prefer_sh2ee: bool,
    ) -> bool {
        match resolver.load_cam_for_room(room_or_rel_path, prefer_sh2ee) {
            Ok(Some(asset)) => {
                self.set_cam_file_with_metadata(
                    Some(asset.cam_file),
                    Some(asset.room_id),
                    Some(asset.source),
                );
                true
            }
            _ => {
                self.set_cam_file_with_metadata(None, None, None);
                false
            }
        }
    }

    /// Resolves and ingests the `.cam` trigger volume file for `room_or_rel_path` using the
    /// default [`AssetPathResolver`] and `prefer_sh2ee` preference (`SH2-50`).
    pub fn load_room_cam(&mut self, room_or_rel_path: &str, prefer_sh2ee: bool) -> bool {
        let resolver = AssetPathResolver::resolve();
        self.load_room_cam_with_resolver(&resolver, room_or_rel_path, prefer_sh2ee)
    }

    /// Clamps `pos` (PS2 world mm, Y-down) within a `_VC_ROAD_AREA` rail using PS2
    /// `vcAdjustXzInLimAreaUsingMIN_IN_ROAD_DIST` (`0x0019b700`) and `vcGetMinInRoadDist` (`0x001a6bd0`)
    /// in authentic 4:3 CRT mode (`400.0` mm indoor, `500.0` mm outdoor).
    pub fn clamp_to_road_rail(
        rail: &sh2_assets::formats::cam::RoadArea,
        pos: Vec3,
        is_outdoor: bool,
    ) -> Vec3 {
        crate::widescreen_rail::clamp_to_road_rail_with_config(
            rail,
            pos,
            is_outdoor,
            &crate::widescreen_rail::AspectRailConfig::authentic_ps2_4x3(),
        )
    }

    /// Clamps `pos` (PS2 world mm, Y-down) within a `_VC_ROAD_AREA` rail using dynamic viewport
    /// aspect-ratio wall clearance (`SH2-53`, `vcAdjustXzInLimAreaUsingMIN_IN_ROAD_DIST` `0x0019b700`).
    pub fn clamp_to_road_rail_with_config(
        rail: &sh2_assets::formats::cam::RoadArea,
        pos: Vec3,
        is_outdoor: bool,
        config: &crate::widescreen_rail::AspectRailConfig,
    ) -> Vec3 {
        crate::widescreen_rail::clamp_to_road_rail_with_config(rail, pos, is_outdoor, config)
    }

    /// Triggers `vcWarpForFixAngCam` (`0x00252980`) when the active road is `FixedAngle` (Mode 2)
    /// and the character faces ≥ 135° away from the fixed angle (`kind_id == 1`).
    ///
    /// Returns `true` if the camera flipped 180° and requires a warp snap on the next tick.
    pub fn trigger_fixed_angle_search_warp(&mut self, player_yaw: f32) -> bool {
        let Some(rec) = self
            .active_road_index
            .and_then(|idx| self.cam_file.as_ref()?.records.get(idx))
        else {
            return false;
        };
        if rec.cam_mv_type != CameraMovementType::FixedAngle {
            return false;
        }
        fixed_angle::vc_warp_for_fix_ang_cam(rec, player_yaw, &mut self.raw_state.fix_cam_mng)
    }

    /// Evaluates one step of the authentic PS2 `vcExecCamera` (`0x00195f90`) pipeline for `rec`.
    fn step_road_record(
        &mut self,
        rec: &RoadRecord,
        player_pos: Vec3,
        player_yaw: f32,
        road_changed: bool,
        warp: bool,
        dt: f32,
    ) -> (Vec3, Vec3, f32, CameraMovementType) {
        let (chara_bottom_y, mut chara_top_y, chara_center_y) =
            chase::compute_chara_y_bounds(player_pos.y, 0.0);
        let is_outdoor = crate::widescreen_rail::is_outdoor_room_or_record(
            self.active_room_id.as_deref(),
            rec.area_size_type,
        );
        let ideal_cam_pos_h = chase::compute_ideal_cam_pos_h(chara_top_y, is_outdoor);

        // Update character telemetry in `_VC_WORK` (`vcSetSubjChara` `0x00195f00`).
        let prev_chara_pos = Vec3::new(
            self.raw_state.chara_pos[0],
            self.raw_state.chara_pos[1],
            self.raw_state.chara_pos[2],
        );
        let prev_chara_yaw = self.raw_state.chara_eye_ang_y;
        let (chara_mv_spd, chara_ang_spd_y) = if warp || dt <= 0.0 {
            (0.0, 0.0)
        } else {
            let dpos = player_pos - prev_chara_pos;
            let spd = dpos.x.hypot(dpos.z) / dt;
            let ang_spd = kinematics::sh_angle_regulate(player_yaw - prev_chara_yaw) / dt;
            (spd, ang_spd)
        };
        self.raw_state.chara_pos = [player_pos.x, player_pos.y, player_pos.z, 1.0];
        self.raw_state.chara_bottom_y = chara_bottom_y;
        self.raw_state.chara_top_y = chara_top_y;
        self.raw_state.chara_center_y = chara_center_y;
        self.raw_state.chara_grnd_y = player_pos.y;
        self.raw_state.chara_mv_spd = chara_mv_spd;
        self.raw_state.chara_ang_spd_y = chara_ang_spd_y;
        self.raw_state.chara_eye_ang_y = player_yaw;
        self.raw_state.ideal_cam_pos_h = ideal_cam_pos_h;

        let (watch_prm, cam_prm) = kinematics::vc_get_use_watch_and_cam_mv_param(
            0.0,
            0.0,
            is_outdoor,
            false,
            chara_mv_spd,
            None,
            None,
        );

        let cur_cam_pos = Vec3::new(
            self.raw_state.cam_pos[0],
            self.raw_state.cam_pos[1],
            self.raw_state.cam_pos[2],
        );
        let mut effective_projection = rec.projection;

        // 1. Ideal camera position (`vcAutoRenewalCamTgtPos` `0x00199f80`).
        let ideal_pos = match rec.cam_mv_type {
            CameraMovementType::LocusCircle => {
                let (locus_eye, locus_proj) =
                    locus_door::evaluate_locus_circle_record(rec, player_pos);
                effective_projection = locus_proj;
                locus_eye
            }
            CameraMovementType::SelfView => {
                let head_pos = Vec3::new(
                    player_pos.x,
                    player_pos.y - CHARA_FALLBACK_HEAD_HEIGHT_MM,
                    player_pos.z,
                );
                self_view::make_ideal_cam_pos_by_head_pos(
                    0,
                    player_pos,
                    player_pos.y - CHARA_CENTER_HEIGHT_MM,
                    head_pos,
                    player_yaw,
                )
            }
            CameraMovementType::ThroughDoor => {
                locus_door::evaluate_through_door_record(rec, player_yaw)
            }
            CameraMovementType::FixedAngle => {
                let pos = fixed_angle::vc_make_ideal_cam_pos_for_fix_ang_cam(
                    rec,
                    player_pos,
                    player_yaw,
                    cur_cam_pos,
                    &mut chara_top_y,
                    &mut self.raw_state.fix_cam_mng,
                    road_changed,
                    is_outdoor,
                    &self.aspect_rail_config,
                );
                self.raw_state.chara_top_y = chara_top_y;
                pos
            }
            CameraMovementType::Chase | CameraMovementType::Settle => {
                chase::vc_make_ideal_cam_pos_use_vc_road_data(
                    rec,
                    player_pos,
                    player_yaw,
                    chara_ang_spd_y,
                    cur_cam_pos,
                    ideal_cam_pos_h,
                    &mut self.raw_state.cam_chara2ideal_ang_y,
                    is_outdoor,
                    &self.aspect_rail_config,
                )
            }
        };

        // 2. Camera target renewal (`vcAutoRenewalCamTgtPos` `0x00199f80` +
        //    `vcFlipFromCamExclusionArea` + `vcAdjTgtMvVecYByCurNearRoad`).
        //    On warp (`_DAT_010e5ad8 & 0x10`), `SLUS_202.28` (`vc_main.c:2387-2446`) copies
        //    `ideal_pos` into `cam_tgt_pos` and still runs `vcMakeBasicCamTgtMvVec`,
        //    `vcCamTgtMvVecIsFlipedFromCharaFront`, and `vcAdjTgtMvVecYByCurNearRoad` before
        //    applying `cam_tgt_pos += mv_vec`.
        let mut cam_tgt_pos = if warp {
            self.raw_state.old_cam_excl_area_r = -1.0;
            ideal_pos
        } else {
            Vec3::new(
                self.raw_state.cam_tgt_pos[0],
                self.raw_state.cam_tgt_pos[1],
                self.raw_state.cam_tgt_pos[2],
            )
        };

        let max_xz = chase::vc_ret_max_tgt_mv_xz_len(
            chara_mv_spd,
            chara_ang_spd_y,
            cam_prm.max_spd_xz,
            is_outdoor,
            dt,
        );
        let mut mv_vec =
            chase::vc_make_basic_cam_tgt_mv_vec(max_xz, ideal_pos, cam_tgt_pos, dt, warp);
        if matches!(
            rec.cam_mv_type,
            CameraMovementType::Chase | CameraMovementType::LocusCircle
        ) && (rec.flags & VC_RD_NO_FRONT_FLIP_F) == 0
        {
            exclusion::vc_cam_tgt_mv_vec_is_fliped_from_chara_front(
                max_xz,
                &mut mv_vec,
                cam_tgt_pos,
                player_pos,
                player_yaw,
                &mut self.raw_state.old_cam_excl_area_r,
                &rec.lim_rd,
                rec.area_size_type,
                is_outdoor,
                &self.aspect_rail_config,
                dt,
            );
        }
        if rec.cam_mv_type != CameraMovementType::ThroughDoor {
            chase::vc_adj_tgt_mv_vec_y_by_cur_near_road(
                &mut mv_vec,
                rec,
                cam_tgt_pos,
                player_pos,
                chara_top_y,
                ideal_cam_pos_h,
                is_outdoor,
            );
        }
        self.raw_state.cam_tgt_mv_ang_y = mv_vec.x.atan2(mv_vec.z);
        if dt <= 0.0 && !warp {
            self.raw_state.cam_tgt_velo[0] = 0.0;
            self.raw_state.cam_tgt_velo[2] = 0.0;
            self.raw_state.cam_tgt_spd = 0.0;
        } else {
            cam_tgt_pos += mv_vec;
            if dt > 0.0 {
                let tgt_vel = mv_vec / dt;
                self.raw_state.cam_tgt_velo = [tgt_vel.x, tgt_vel.y, tgt_vel.z, 0.0];
                self.raw_state.cam_tgt_spd = tgt_vel.x.hypot(tgt_vel.z);
            } else {
                self.raw_state.cam_tgt_velo = [0.0; 4];
                self.raw_state.cam_tgt_spd = 0.0;
            }
        }
        self.raw_state.cam_tgt_pos = [cam_tgt_pos.x, cam_tgt_pos.y, cam_tgt_pos.z, 1.0];

        // 3. Velocity-bounded eye integration (`vcRenewalCamData` `0x0019c840`).
        let mut cam_pos = cur_cam_pos;
        let mut cam_velo = Vec3::new(
            self.raw_state.cam_velo[0],
            self.raw_state.cam_velo[1],
            self.raw_state.cam_velo[2],
        );
        let mut cam_mv_ang_y = self.raw_state.cam_mv_ang[0];
        kinematics::vc_renewal_cam_data(
            &mut cam_pos,
            cam_tgt_pos,
            &mut cam_velo,
            &mut cam_mv_ang_y,
            &cam_prm,
            warp,
            dt,
        );
        self.raw_state.cam_pos = [cam_pos.x, cam_pos.y, cam_pos.z, 1.0];
        self.raw_state.cam_velo = [cam_velo.x, cam_velo.y, cam_velo.z, 0.0];
        self.raw_state.cam_mv_ang[0] = cam_mv_ang_y;

        // 4. Watch target (`vcMakeNormalWatchTgtPos` `0x00198d80`).
        let watch_tgt = if rec.cam_mv_type == CameraMovementType::FixedAngle {
            fixed_angle::vc_make_fix_ang_watch_tgt_pos(rec, cam_pos, &self.raw_state.fix_cam_mng)
        } else {
            chase::vc_make_normal_watch_tgt_pos(
                rec,
                cam_pos,
                player_pos,
                chara_bottom_y,
                player_yaw,
            )
        };
        self.raw_state.watch_tgt_pos = [watch_tgt.x, watch_tgt.y, watch_tgt.z, 1.0];

        // 5. Screen half-angles (`vcPreSetDataInVC_WORK` `0x00196be0`).
        let (scr_half_wx, scr_half_wy) =
            kinematics::compute_screen_half_angles(effective_projection);
        self.raw_state.scr_half_ang_wx = scr_half_wx;
        self.raw_state.scr_half_ang_wy = scr_half_wy;

        // 6. Base & offset angular renewal (`vcRenewalCamMatAng` `0x0019c9d0`).
        let mut base_ang = Vec3::new(
            self.raw_state.base_cam_ang[0],
            self.raw_state.base_cam_ang[1],
            self.raw_state.base_cam_ang[2],
        );
        let mut ofs_ang = Vec3::new(
            self.raw_state.ofs_cam_ang[0],
            self.raw_state.ofs_cam_ang[1],
            self.raw_state.ofs_cam_ang[2],
        );
        let new_base_ang = chase::vc_make_new_base_cam_ang(
            rec,
            cam_pos,
            watch_tgt,
            base_ang,
            chara_mv_spd,
            false,
            warp,
            dt,
        );
        self.raw_state.old_base_cam_ang = self.raw_state.base_cam_ang;
        if new_base_ang != base_ang {
            kinematics::vc_renewal_base_cam_ang_and_adjust_ofs_cam_ang(
                &mut base_ang,
                &mut ofs_ang,
                new_base_ang,
            );
        }
        self.raw_state.base_cam_ang = [base_ang.x, base_ang.y, base_ang.z, 0.0];

        let base_mat_t = kinematics::vw_rot_matrix_yxz(base_ang).transpose();
        let mut tgt_ofs_ang = kinematics::vc_make_ofs_cam_tgt_ang(
            base_mat_t,
            watch_tgt,
            cam_pos,
            self.raw_state.watch_tgt_ang_z,
        );

        // Character-in-screen framing (`vcSetFlagsByCamMvType` `0x00196aa0`: enabled for Chase/Settle/LocusCircle).
        if matches!(
            rec.cam_mv_type,
            CameraMovementType::Chase
                | CameraMovementType::Settle
                | CameraMovementType::LocusCircle
        ) {
            let (btm_ofs, top_ofs) =
                kinematics::vc_make_ofs_cam_2_chara_bottom_and_top_ang_by_base_mat_t(
                    chara_bottom_y + rec.trace_btm_hy,
                    chara_top_y,
                    base_mat_t,
                    cam_pos,
                    player_pos,
                );
            kinematics::vc_adj_cam_ofs_ang_by_chara_in_screen(
                &mut tgt_ofs_ang,
                btm_ofs,
                top_ofs,
                scr_half_wx,
                scr_half_wy,
            );
        }

        let mut ofs_ang_spd = Vec3::new(
            self.raw_state.ofs_cam_ang_spd[0],
            self.raw_state.ofs_cam_ang_spd[1],
            self.raw_state.ofs_cam_ang_spd[2],
        );
        if warp {
            ofs_ang = tgt_ofs_ang;
            ofs_ang_spd = Vec3::ZERO;
        } else {
            kinematics::vc_adj_cam_ofs_ang_by_ofs_ang_spd(
                &mut ofs_ang,
                &mut ofs_ang_spd,
                tgt_ofs_ang,
                &watch_prm,
                dt,
            );
        }
        kinematics::vc_correct_cam_mat_ang_forcibly(&mut ofs_ang, rec);
        let (cam_rot_mat, final_ang) =
            kinematics::vc_make_cam_mat_and_cam_ang_by_base_ang_and_ofs_ang(base_ang, ofs_ang);
        ofs_ang = Vec3::new(
            kinematics::sh_angle_regulate(ofs_ang.x),
            kinematics::sh_angle_regulate(ofs_ang.y),
            kinematics::sh_angle_regulate(ofs_ang.z),
        );
        self.raw_state.ofs_cam_ang = [ofs_ang.x, ofs_ang.y, ofs_ang.z, 0.0];
        self.raw_state.ofs_cam_ang_spd = [ofs_ang_spd.x, ofs_ang_spd.y, ofs_ang_spd.z, 0.0];
        self.raw_state.cam_mat_ang = [final_ang.x, final_ang.y, final_ang.z, 0.0];
        self.raw_state.cam_mat.m = [
            [
                cam_rot_mat.x_axis.x,
                cam_rot_mat.x_axis.y,
                cam_rot_mat.x_axis.z,
                0.0,
            ],
            [
                cam_rot_mat.y_axis.x,
                cam_rot_mat.y_axis.y,
                cam_rot_mat.y_axis.z,
                0.0,
            ],
            [
                cam_rot_mat.z_axis.x,
                cam_rot_mat.z_axis.y,
                cam_rot_mat.z_axis.z,
                0.0,
            ],
            [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
        ];

        let watch_dist = (watch_tgt - cam_pos).length().max(500.0);
        let look_target = cam_pos + kinematics::vw_angle_to_vector(watch_dist, final_ang);
        let fov_y = kinematics::fov_y_from_ps2_projection_unclamped(effective_projection);

        (cam_pos, look_target, fov_y, rec.cam_mv_type)
    }

    /// Reset camera position instantly behind the player without smoothing (`vcSetFirstCamWork` `0x00195ad0`).
    pub fn reset_to_player(&mut self, player_pos: Vec3, player_yaw: f32) {
        self.raw_state.cam_chara2ideal_ang_y = kinematics::sh_angle_regulate(player_yaw + PI);
        self.raw_state.old_cam_excl_area_r = -1.0;
        self.raw_state.cam_velo = [0.0; 4];
        self.raw_state.ofs_cam_ang_spd = [0.0; 4];
        self.raw_state.base_cam_ang = [0.0; 4];
        self.raw_state.fix_cam_mng = FixCamManager::default();
        self.raw_state.chara_pos = [player_pos.x, player_pos.y, player_pos.z, 1.0];
        self.raw_state.chara_eye_ang_y = player_yaw;

        let active_rec = self.cam_file.as_ref().and_then(|cam_file| {
            let player_arr = [player_pos.x, player_pos.y, player_pos.z];
            let rec = cam_file.find_active_record(player_arr, 0.0)?;
            let idx = cam_file.records.iter().position(|r| std::ptr::eq(r, rec));
            Some((idx, rec.clone()))
        });

        let (eye, target, fov_y, mv_type) = if let Some((idx, rec)) = active_rec {
            self.active_road_index = idx;
            self.step_road_record(&rec, player_pos, player_yaw, true, true, 0.0)
        } else {
            self.active_road_index = None;
            let (fb_eye, fb_tgt, fb_fov, fb_mv) = Self::fallback_placement(player_pos, player_yaw);
            self.raw_state.cam_pos = [fb_eye.x, fb_eye.y, fb_eye.z, 1.0];
            self.raw_state.cam_tgt_pos = [fb_eye.x, fb_eye.y, fb_eye.z, 1.0];
            let ang = kinematics::vw_vector_to_angle(fb_tgt - fb_eye);
            self.raw_state.ofs_cam_ang = [ang.x, ang.y, ang.z, 0.0];
            self.raw_state.cam_mat_ang = [ang.x, ang.y, ang.z, 0.0];
            (fb_eye, fb_tgt, fb_fov, fb_mv)
        };

        let snap = CameraSnapshot {
            eye,
            target,
            up: Vec3::new(0.0, -1.0, 0.0),
            fov_y,
        };

        self.mv_type = mv_type;
        self.ideal_eye = eye;
        self.ideal_target = target;
        self.prev_snapshot = snap;
        self.curr_snapshot = snap;
        self.search_yaw_offset = 0.0;
        self.search_pitch_offset = 0.0;
        self.last_move_smooth = false;
    }

    /// Neutral fallback camera placement when no `.cam` file is loaded (`SLUS_202.28`
    /// `mv_nml_chr2cam_r[Wide]` = `1000.0` mm, `ideal_cam_pos_h` = `chara_top_y - 200.0`,
    /// `ofs_watch_hy` default = `-300.0`, `projection` default = `448.0`).
    fn fallback_placement(
        player_pos: Vec3,
        player_yaw: f32,
    ) -> (Vec3, Vec3, f32, CameraMovementType) {
        let forward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
        let (bottom_y, top_y, _) = chase::compute_chara_y_bounds(player_pos.y, 0.0);
        let eye_y = chase::compute_ideal_cam_pos_h(top_y, false);
        let radius = chase::MV_NML_CHR2CAM_R[sh2_assets::formats::cam::AreaSizeType::Wide as usize];
        let eye = Vec3::new(
            player_pos.x - forward.x * radius,
            eye_y,
            player_pos.z - forward.z * radius,
        );
        let target = Vec3::new(player_pos.x, bottom_y - 300.0, player_pos.z);
        let fov_y = kinematics::fov_y_from_ps2_projection_unclamped(
            kinematics::PS2_DEFAULT_PROJECTION_SCR_Z,
        );
        (eye, target, fov_y, CameraMovementType::Chase)
    }

    /// Step the camera simulation by one 60 Hz tick (`vcExecCamera` `0x00195f90`).
    ///
    /// Evaluates active `ThroughDoorSession`, `.cam` volume triggers with `vcSetCurNearRoadInVC_WORK`
    /// hysteresis, velocity-bounded `vw_calc.c` kinematics, `vcRetSmoothCamMvF` (`0x00196360`),
    /// and authentic PS2 `SelfView` / `FarWatch` / `vcCamMatNoise` framing.
    /// `neck_ang_y` (`right_stick_x`) and `neck_ang_x` (`right_stick_y`) drive `vcMakeFarWatchTgtPos`
    /// (`SLUS_202.28` `0x00199760`) without synthetic offset multipliers.
    pub fn tick(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        right_stick_x: f32,
        right_stick_y: f32,
    ) {
        self.tick_with_context(player_pos, player_yaw, right_stick_y, right_stick_x, None);
    }

    /// Step the camera simulation by one 60 Hz tick with optional stage-event priority context
    /// (`vcRetPrioStgEvnt`, `SLUS_202.28` `0x00294ab0` / `sh2pc.exe` `0x005227c0`).
    pub fn tick_with_context(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        neck_ang_x: f32,
        neck_ang_y: f32,
        stg_evnt_ctx: Option<&road_priority::StageEventPriorityContext<'_>>,
    ) {
        self.prev_snapshot = self.curr_snapshot;
        let prev_pos = Vec3::new(
            self.raw_state.cam_pos[0],
            self.raw_state.cam_pos[1],
            self.raw_state.cam_pos[2],
        );
        let prev_ang = Vec3::new(
            self.raw_state.cam_mat_ang[0],
            self.raw_state.cam_mat_ang[1],
            self.raw_state.cam_mat_ang[2],
        );
        self.search_yaw_offset = neck_ang_y;
        self.search_pitch_offset = neck_ang_x;

        let mut cur_road_flags: u16 = 0;
        let mut cur_area_size = AreaSizeType::Wide;
        let mut cur_projection: f32 = PS2_DEFAULT_PROJECTION_Z;
        let mut warp_cut = false;
        let mut road_changed = false;

        let nearest_enemy_dist = self
            .nearest_enemy
            .map(|(d, _)| d)
            .unwrap_or(NEAREST_ENEMY_MAX_DIST_MM);

        let (eye, mut look, target_fov, target_mv_type) = if let Some(td_eye) =
            self.through_door_session.step(
                SIM_DT_SECS,
                player_pos,
                player_yaw,
                self.nearest_enemy.map(|(d, _)| d),
            ) {
            let td_look = player_pos + Vec3::new(0.0, DEFAULT_WATCH_TARGET_Y_OFFSET_MM, 0.0);
            let td_ang = kinematics::vw_vector_to_angle(td_look - td_eye);
            self.raw_state.cam_pos = [td_eye.x, td_eye.y, td_eye.z, 1.0];
            self.raw_state.cam_tgt_pos = [td_eye.x, td_eye.y, td_eye.z, 1.0];
            self.raw_state.cam_mat_ang = [td_ang.x, td_ang.y, td_ang.z, 0.0];
            let td_fov = kinematics::fov_y_from_ps2_projection_unclamped(
                kinematics::PS2_DEFAULT_PROJECTION_SCR_Z,
            );
            (td_eye, td_look, td_fov, CameraMovementType::ThroughDoor)
        } else {
            // Select best candidate road using `vcRetRoadUsePriority` (`vcRetPrioStgEvnt`)
            // and `vcSetCurNearRoadInVC_WORK` hysteresis (`0x00197af0`).
            let selected_road = self.cam_file.as_ref().and_then(|cam_file| {
                let mut best: Option<road_priority::RoadSelectionCandidate<'_>> = None;
                let mut old_cand: Option<road_priority::RoadSelectionCandidate<'_>> = None;

                for (idx, rec) in cam_file.records.iter().enumerate() {
                    let (min_y, max_y) = rec.lim_sw.y_bounds();
                    if player_pos.y < min_y || player_pos.y > max_y {
                        continue;
                    }
                    let prio = road_priority::ret_road_use_priority(
                        rec.rd_type,
                        self.search_button_pressed,
                        stg_evnt_ctx,
                    );
                    let signed_dist = road_priority::signed_xz_dist_from_lim_sw(rec, player_pos);
                    let cand = road_priority::RoadSelectionCandidate {
                        index: idx,
                        record: rec,
                        priority: prio,
                        signed_dist_mm: signed_dist,
                    };
                    if Some(idx) == self.active_road_index {
                        old_cand = Some(cand);
                    }
                    if prio == 0 {
                        continue;
                    }
                    let better = match best {
                        None => true,
                        Some(b) => {
                            if prio > b.priority {
                                signed_dist <= 0.0 || signed_dist < b.signed_dist_mm
                            } else if prio < b.priority {
                                b.signed_dist_mm > 0.0 && signed_dist < b.signed_dist_mm
                            } else {
                                signed_dist < b.signed_dist_mm
                            }
                        }
                    };
                    if better {
                        best = Some(cand);
                    }
                }

                let best_cand = best?;
                let decision = road_priority::resolve_cur_near_road_hysteresis(
                    old_cand, best_cand, player_yaw,
                );
                let old_flags = self
                    .active_road_index
                    .and_then(|old_idx| cam_file.records.get(old_idx))
                    .map(|r| r.flags);
                let rec = cam_file.records[decision.selected_index].clone();
                Some((decision, old_flags, rec))
            });

            if let Some((decision, old_flags, rec)) = selected_road {
                if decision.switched {
                    road_changed = true;
                    if decision.warp_cut {
                        if let Some(old_f) = old_flags {
                            match crate::widescreen_rail::evaluate_intra_room_transition(
                                old_f,
                                rec.flags,
                                self.aspect_rail_config.transition_policy,
                            ) {
                                crate::widescreen_rail::IntraRoomTransitionStep::InstantWarpCut => {
                                    warp_cut = true;
                                }
                                crate::widescreen_rail::IntraRoomTransitionStep::SmoothBlend(_) => {
                                    // Velocity-bounded kinematics handle smooth transition naturally.
                                }
                            }
                        } else {
                            warp_cut = true;
                        }
                    }
                    self.active_road_index = Some(decision.selected_index);
                }

                cur_road_flags = rec.flags;
                cur_area_size = rec.area_size_type;
                cur_projection = if rec.projection > 0.0 {
                    rec.projection
                } else {
                    PS2_DEFAULT_PROJECTION_Z
                };

                self.step_road_record(
                    &rec,
                    player_pos,
                    player_yaw,
                    road_changed,
                    warp_cut,
                    SIM_DT_SECS,
                )
            } else {
                let (fb_eye, fb_look, fb_fov, fb_mv) =
                    Self::fallback_placement(player_pos, player_yaw);
                let (watch_prm, cam_prm) = kinematics::vc_get_use_watch_and_cam_mv_param(
                    0.0, 0.0, false, false, 0.0, None, None,
                );
                let mut pos = prev_pos;
                let mut vel = Vec3::new(
                    self.raw_state.cam_velo[0],
                    self.raw_state.cam_velo[1],
                    self.raw_state.cam_velo[2],
                );
                let mut mv_ang_y = self.raw_state.cam_mv_ang[0];
                kinematics::vc_renewal_cam_data(
                    &mut pos,
                    fb_eye,
                    &mut vel,
                    &mut mv_ang_y,
                    &cam_prm,
                    false,
                    SIM_DT_SECS,
                );
                self.raw_state.cam_pos = [pos.x, pos.y, pos.z, 1.0];
                self.raw_state.cam_tgt_pos = [fb_eye.x, fb_eye.y, fb_eye.z, 1.0];
                self.raw_state.cam_velo = [vel.x, vel.y, vel.z, 0.0];
                self.raw_state.cam_mv_ang[0] = mv_ang_y;

                let tgt_ang = kinematics::vw_vector_to_angle(fb_look - pos);
                let mut ofs_ang = Vec3::new(
                    self.raw_state.ofs_cam_ang[0],
                    self.raw_state.ofs_cam_ang[1],
                    self.raw_state.ofs_cam_ang[2],
                );
                let mut ofs_ang_spd = Vec3::new(
                    self.raw_state.ofs_cam_ang_spd[0],
                    self.raw_state.ofs_cam_ang_spd[1],
                    self.raw_state.ofs_cam_ang_spd[2],
                );
                kinematics::vc_adj_cam_ofs_ang_by_ofs_ang_spd(
                    &mut ofs_ang,
                    &mut ofs_ang_spd,
                    tgt_ang,
                    &watch_prm,
                    SIM_DT_SECS,
                );
                self.raw_state.ofs_cam_ang = [ofs_ang.x, ofs_ang.y, ofs_ang.z, 0.0];
                self.raw_state.ofs_cam_ang_spd = [ofs_ang_spd.x, ofs_ang_spd.y, ofs_ang_spd.z, 0.0];
                self.raw_state.cam_mat_ang = [ofs_ang.x, ofs_ang.y, ofs_ang.z, 0.0];
                let dist = (fb_look - pos).length().max(500.0);
                (
                    pos,
                    pos + kinematics::vw_angle_to_vector(dist, ofs_ang),
                    fb_fov,
                    fb_mv,
                )
            }
        };

        let cur_ang = Vec3::new(
            self.raw_state.cam_mat_ang[0],
            self.raw_state.cam_mat_ang[1],
            self.raw_state.cam_mat_ang[2],
        );
        self.last_move_smooth =
            kinematics::vc_ret_smooth_cam_mv_f(prev_pos, eye, prev_ang, cur_ang, 1.0);

        // Authentic PS2 `vcRetFarWatchRate` (`0x00196700`), `vcMakeFarWatchTgtPos` (`0x00199760`),
        // `vcRetSelfViewEffectRate` (`0x001968f0`), `vcMixSelfViewEffectToWatchTgtPos` (`0x00199080`),
        // and `vcAdjustWatchYLimitHighWhenFarView` (`0x00199dd0`).
        let is_outdoor = cur_area_size == AreaSizeType::Outdoor;
        let far_watch = self_view::ret_far_watch_rate(
            self.search_button_pressed,
            target_mv_type,
            self.raw_state.flags,
            cur_road_flags,
            is_outdoor,
            self.through_door_session.rail_sta_to_chara_dist,
            Vec3::from_array(self.through_door_session.param.rail_sta_pos),
            player_pos,
            player_yaw,
        );

        let head_pos = Vec3::new(
            player_pos.x,
            player_pos.y - CHARA_FALLBACK_HEAD_HEIGHT_MM,
            player_pos.z,
        );
        let chara_center_y = player_pos.y - CHARA_CENTER_HEIGHT_MM;
        let nearest_item_pair = self
            .nearest_item
            .center_y
            .map(|cy| (self.nearest_item.dist_mm, cy));

        if target_mv_type == CameraMovementType::SelfView || far_watch.rate > 0.0 {
            let far_tgt = self_view::make_far_watch_tgt_pos(&self_view::FarWatchInput {
                stage_id: self.stage_id,
                area_size_type: cur_area_size,
                on_boat: self.on_boat,
                cam_pos: eye,
                chara_pos: player_pos,
                chara_yaw: player_yaw,
                chara_center_y,
                neck_ang_x,
                neck_ang_y,
                nearest_enemy: self.nearest_enemy,
                nearest_item: nearest_item_pair,
            });
            look = if target_mv_type == CameraMovementType::SelfView {
                far_tgt
            } else {
                look.lerp(far_tgt, far_watch.rate)
            };
        }

        let sv_eff_rate =
            self_view::ret_self_view_effect_rate(target_mv_type, eye, head_pos, nearest_enemy_dist);
        if sv_eff_rate > 0.0 {
            let (mixed_tgt, mixed_ang_z) = self_view::mix_self_view_effect_to_watch_tgt_pos(
                sv_eff_rate,
                look,
                self.raw_state.watch_tgt_ang_z,
                eye,
                player_yaw,
                [neck_ang_x, player_yaw + neck_ang_y, 0.0],
                0,
                self.nearest_enemy.is_some(),
            );
            look = mixed_tgt;
            self.raw_state.watch_tgt_ang_z = mixed_ang_z;
        }

        if (cur_road_flags & VC_RD_LIM_UP_FAR_VIEW_F) != 0
            && (target_mv_type == CameraMovementType::Chase
                || target_mv_type == CameraMovementType::SelfView)
        {
            look = self_view::adjust_watch_y_limit_high_when_far_view(
                look,
                eye,
                cur_projection,
                PS2_DEFAULT_PROJECTION_Z,
                self.stage_id,
            );
        }

        self.mv_type = target_mv_type;
        self.ideal_eye = eye;
        self.ideal_target = look;

        let mut final_target = self.ideal_target;

        // Authentic PS2 Mode 3 (`SELF_VIEW`) camera matrix breathing noise (`vcCamMatNoise` `0x0019d8e0`).
        if self.mv_type == CameraMovementType::SelfView {
            self.self_view_timer_sec += SIM_DT_SECS;
            final_target = noise::apply_self_view_noise_to_target(
                self.ideal_eye,
                final_target,
                self.self_view_timer_sec,
            );
        }

        self.curr_snapshot = CameraSnapshot {
            eye: self.ideal_eye,
            target: final_target,
            up: Vec3::new(0.0, -1.0, 0.0),
            fov_y: target_fov,
        };

        // On an instant camera cut (`VC_RD_WARP_IN_F` / `VC_RD_WARP_OUT_F`), snap `prev_snapshot`
        // to `curr_snapshot` as well so 120–240 Hz sub-frame interpolation (`interpolated_view_matrix`)
        // does not smear between the old camera volume and the new cut angle (`SH2-50`).
        if warp_cut {
            self.prev_snapshot = self.curr_snapshot;
        }
    }

    /// Compute sub-frame interpolated view matrix between previous and current simulation ticks.
    ///
    /// `alpha` is the clock accumulator progress between ticks (0.0 .. 1.0).
    pub fn interpolated_view_matrix(&self, alpha: f32) -> Mat4 {
        let eye = self.prev_snapshot.eye.lerp(self.curr_snapshot.eye, alpha);
        let target = self
            .prev_snapshot
            .target
            .lerp(self.curr_snapshot.target, alpha);
        let up = self.prev_snapshot.up;

        Mat4::look_at_rh(eye, target, up)
    }

    /// Interpolated field-of-view in radians.
    pub fn interpolated_fov(&self, alpha: f32) -> f32 {
        self.prev_snapshot.fov_y + (self.curr_snapshot.fov_y - self.prev_snapshot.fov_y) * alpha
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ps2_camera_reset_and_tick() {
        let mut cam = Ps2CameraController::new();
        let p_pos = Vec3::new(1000.0, 0.0, 2000.0);
        cam.reset_to_player(p_pos, 0.0);

        assert_eq!(cam.mv_type, CameraMovementType::Chase);
        assert!((cam.curr_snapshot.target.x - 1000.0).abs() < f32::EPSILON);

        cam.tick(p_pos + Vec3::new(10.0, 0.0, 10.0), 0.1, 0.5, 0.0);
        let view_mat = cam.interpolated_view_matrix(0.5);
        assert!(view_mat.is_finite());
    }

    #[test]
    fn test_camera_er01_ps2_parity_framing() {
        let mut controller = Ps2CameraController::new();
        if !controller.load_room_cam("er01", false) {
            return;
        }
        assert_eq!(controller.active_room_id.as_deref(), Some("er01"));
        assert_eq!(
            controller.active_cam_source,
            Some(CamSourceKind::VanillaRetail)
        );

        // Exact player coordinates from screenshot at opening restroom:
        // PLAYER POS: X: -19390.1, Y: 0.0, Z: 20317.8, YAW: -2.23 rad (-127.6 deg)
        let player_pos = Vec3::new(-19390.1, 0.0, 20317.8);
        controller.reset_to_player(player_pos, -2.23);

        assert_eq!(controller.active_road_index, Some(2));
        assert_eq!(controller.mv_type, CameraMovementType::Chase);

        // Ground-truth PS2 camera eye: (-18684.0, -1003.0, 20361.0)
        assert!((controller.curr_snapshot.eye.x - (-18684.0)).abs() < 1.0);
        assert!((controller.curr_snapshot.eye.y - (-1003.0)).abs() < 1.0);
        assert!((controller.curr_snapshot.eye.z - 20361.0).abs() < 1.0);

        // Ground truth look target: points towards sink/mirror and downward into chest/reflection
        // (-Y is up, so target.y must be below the eye: target.y > eye.y (-1003.0))
        assert!(controller.curr_snapshot.target.y > controller.curr_snapshot.eye.y);
        assert!(controller.curr_snapshot.target.x < controller.curr_snapshot.eye.x);
        assert!(controller.curr_snapshot.fov_y < 0.6);
    }

    #[test]
    fn test_sh2_50_warp_in_out_flags_snap_prev_and_curr_snapshots() {
        use sh2_assets::formats::cam::{
            AreaSizeType, CamMovementParams, CamMovementType as AssetCamMovementType,
            ChaseCamParam, RoadArea, RoadRecord, RoadType, VC_RD_END_DATA_F, VC_RD_WARP_IN_F,
            VC_RD_WARP_OUT_F,
        };

        let make_area = |min_x: f32, max_x: f32| RoadArea {
            x0: min_x,
            z0: -1000.0,
            x1: min_x,
            z1: 1000.0,
            x2: max_x,
            z2: 1000.0,
            min_hy: -2000.0,
            max_hy: 2000.0,
        };

        let make_fixed_record = |min_x: f32, max_x: f32, eye: Vec3, flags: u16| RoadRecord {
            lim_sw: make_area(min_x, max_x),
            lim_rd: RoadArea {
                x0: eye.x,
                z0: eye.z,
                x1: eye.x,
                z1: eye.z,
                x2: eye.x,
                z2: eye.z,
                min_hy: eye.y,
                max_hy: eye.y,
            },
            kind_id: 0,
            flags,
            area_size_type: AreaSizeType::Wide,
            rd_type: RoadType::Event,
            mv_y_type: 3,
            ofs_watch_hy: -300.0,
            trace_btm_hy: 0.0,
            rd_dir_type: 0,
            projection: 448.0,
            proj_volume: 0.0,
            proj_sec: 1.0,
            cam_mv_type: AssetCamMovementType::Settle,
            params: CamMovementParams::Chase(ChaseCamParam::default()),
        };

        let cam_file = CamFile {
            records: vec![
                make_fixed_record(
                    -1000.0,
                    0.0,
                    Vec3::new(-500.0, -1400.0, -800.0),
                    VC_RD_WARP_OUT_F,
                ),
                make_fixed_record(
                    0.0,
                    1000.0,
                    Vec3::new(500.0, -2200.0, 800.0),
                    VC_RD_WARP_IN_F | VC_RD_END_DATA_F,
                ),
            ],
        };

        let mut controller = Ps2CameraController::new();
        controller.set_cam_file(Some(cam_file));

        // Spawn inside Road #0
        controller.reset_to_player(Vec3::new(-250.0, 0.0, 0.0), 0.0);
        assert_eq!(controller.active_road_index, Some(0));
        assert!((controller.curr_snapshot.eye.x - (-500.0)).abs() < 1e-3);

        // Cross boundary into Road #1 (`VC_RD_WARP_IN_F` / `VC_RD_WARP_OUT_F` active):
        // Both `curr_snapshot` and `prev_snapshot` must snap immediately with zero lerp smear.
        controller.tick(Vec3::new(250.0, 0.0, 0.0), 0.0, 0.0, 0.0);
        assert_eq!(controller.active_road_index, Some(1));
        assert!((controller.curr_snapshot.eye.x - 500.0).abs() < 1e-3);
        assert!((controller.prev_snapshot.eye.x - 500.0).abs() < 1e-3);
        assert!((controller.curr_snapshot.eye.y - (-2200.0)).abs() < 1e-3);
        assert!((controller.prev_snapshot.eye.y - (-2200.0)).abs() < 1e-3);
    }

    #[test]
    fn test_vc_set_watch_tgt_xz_pos_aligns_with_camera_to_player_ray() {
        // Regression test for `SLUS_202.28` `vcSetWatchTgtXzPos` (`0x00199ca0`, `shAtan2(dz, dx)`
        // = `dx.atan2(dz)`): when James faces `+Z` (`yaw = 0`) and the chase camera sits directly
        // behind him in `-Z`, the watch target must project ahead along `+Z`, not 90° sideways
        // along `+X`.
        let mut controller = Ps2CameraController::new();
        if !controller.load_room_cam("ca04", true) {
            return; // Skip gracefully in CI without retail assets
        }

        let player_pos = Vec3::new(26_547.9, -13_319.5, -95_962.2);
        let player_yaw = -0.03f32;
        controller.reset_to_player(player_pos, player_yaw);

        let eye = controller.curr_snapshot.eye;
        let tgt = controller.curr_snapshot.target;
        let to_player = Vec3::new(player_pos.x - eye.x, 0.0, player_pos.z - eye.z).normalize();
        let to_tgt = Vec3::new(tgt.x - eye.x, 0.0, tgt.z - eye.z).normalize();
        let alignment = to_player.dot(to_tgt);
        assert!(
            alignment > 0.95,
            "camera target must align with camera->player ray (dot={alignment:.3}, eye={eye:?}, tgt={tgt:?})"
        );
    }
}
