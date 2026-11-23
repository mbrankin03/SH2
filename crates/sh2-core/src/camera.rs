//! Silent Hill 2 authentic PS2 camera system and subframe interpolation.
//!
//! Ground truth references:
//! - PS2 beta `view/vc_main.c`, `vc_calc.c`, `vc_newest.c`, `vw_calc.c`, `vb_main.c`
//! - `crates/sh2-core/src/types/camera.rs` (`CameraState` / `_VC_WORK`)
//! - Strict compliance: Exactly mirrors PS2 camera pathing, fixed cinematic angles,
//!   chase rails, room `.cam` boundaries, and right-stick head/search panning.
//!   Zero over-the-shoulder modifications.

use crate::types::camera::CameraState;
use glam::{Mat4, Quat, Vec3};
use sh2_assets::formats::cam::{
    AreaSizeType, CamFile, CamMovementParams, CamSourceKind, RoadRecord, VC_RD_CAM_LIKE_SETTLE_F,
    VC_RD_LIM_UP_FAR_VIEW_F,
};
use sh2_assets::AssetPathResolver;
use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

/// Re-export authentic camera movement types from assets for engine consumers.
pub use sh2_assets::formats::cam::CamMovementType as CameraMovementType;
pub use sh2_assets::formats::cam::CamSourceKind as CameraSourceKind;

/// Authentic PS2 `LOCUS_CIRCLE` (Mode 4) and `THROUGH_DOOR` (Mode 5) camera motion algorithms (`SH2-51`, `SH2-134`).
pub mod locus_door;
/// Authentic PS2 Mode 3 (`SELF_VIEW`) camera matrix breathing/handheld noise (`SH2-134`).
pub mod noise;
/// Authentic PS2 road priority gates (`vcRetPrioStgEvnt`), nearest-item search, and hysteresis (`SH2-134`).
pub mod road_priority;
/// Authentic PS2 Mode 3 (`SELF_VIEW`) and `FarWatch` search-view algorithms (`SH2-134`).
pub mod self_view;

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
            fov_y: FRAC_PI_4, // ~45 degrees in radians
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
    /// Target camera eye position (smoothing accumulator).
    pub ideal_eye: Vec3,
    /// Target camera look-at position (smoothing accumulator).
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
        Self {
            raw_state: CameraState::default(),
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

    /// Compute camera eye, target, fov, and movement type for a given road record and player pose.
    ///
    /// Implements authentic PS2 camera positioning (`vcMakeIdealCamPosUseVC_ROAD_DATA`),
    /// dynamic lookahead watch targeting (`vcSetWatchTgtXzPos`), screen pitch framing (`vcAdjCamOfsAngByCharaInScreen`),
    /// and yaw limit clamping (`vcCorrectCamMatAngForcibly`).
    fn evaluate_road_record(
        rec: &RoadRecord,
        player_pos: Vec3,
        player_yaw: f32,
    ) -> (Vec3, Vec3, f32, CameraMovementType) {
        // PS2 projection focal conversion: 448.0 -> ~53 deg (~0.927 rad), 757.0 -> ~33 deg (~0.575 rad)
        let fov_y = if rec.projection > 0.0 {
            (2.0 * ((0.5 * 448.0) / rec.projection).atan()).clamp(0.50, 1.15)
        } else {
            FRAC_PI_4
        };

        // Determine camera eye position based on road movement type and rail bounds
        let (rail_min_x, rail_max_x) = rec.lim_rd.x_bounds();
        let (rail_min_z, rail_max_z) = rec.lim_rd.z_bounds();
        let eye_y = (rec.lim_rd.min_hy + rec.lim_rd.max_hy) * 0.5;
        let is_rail_constrained =
            (rail_max_x - rail_min_x).abs() > 0.01 || (rail_max_z - rail_min_z).abs() > 0.01;

        let mut effective_projection = rec.projection;
        let eye = match rec.cam_mv_type {
            CameraMovementType::FixedAngle | CameraMovementType::Settle => Vec3::new(
                (rail_min_x + rail_max_x) * 0.5,
                eye_y,
                (rail_min_z + rail_max_z) * 0.5,
            ),
            CameraMovementType::LocusCircle => {
                let (locus_eye, locus_proj) =
                    locus_door::evaluate_locus_circle_record(rec, player_pos);
                effective_projection = locus_proj;
                locus_eye
            }
            CameraMovementType::SelfView => {
                let head_pos = Vec3::new(player_pos.x, player_pos.y - 937.5, player_pos.z);
                self_view::make_ideal_cam_pos_by_head_pos(
                    0,
                    player_pos,
                    player_pos.y - 468.75,
                    head_pos,
                    player_yaw,
                )
            }
            CameraMovementType::ThroughDoor => {
                locus_door::evaluate_through_door_record(rec, player_yaw)
            }
            _ => {
                if is_rail_constrained {
                    let forward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
                    let unconstrained_eye =
                        player_pos - forward * 795.0 + Vec3::new(0.0, -900.0, 0.0);
                    Self::clamp_to_road_rail(&rec.lim_rd, unconstrained_eye, false)
                } else {
                    let forward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
                    player_pos - forward * 2000.0 + Vec3::new(0.0, -1400.0, 0.0)
                }
            }
        };

        if rec.cam_mv_type == CameraMovementType::FixedAngle {
            if let CamMovementParams::FixedAngle(ref fix) = rec.params {
                let rot = Quat::from_euler(glam::EulerRot::YXZ, fix.ang_y, fix.ang_x, 0.0);
                let forward = rot * Vec3::new(0.0, 0.0, 1.0);
                let look_target = eye + forward * fix.cam2wth_dist.max(500.0);
                return (eye, look_target, fov_y, CameraMovementType::FixedAngle);
            }
        }

        let fov_y = if effective_projection > 0.0 {
            (2.0 * ((0.5 * 448.0) / effective_projection).atan()).clamp(0.50, 1.15)
        } else {
            fov_y
        };

        // 1. Dynamic Lookahead Watch Target (vcSetWatchTgtXzPos)
        let (tgt_watch_cir_r_base, min_cam2watch_dist) = match rec.area_size_type {
            AreaSizeType::Tiny => (400.0f32, 1400.0f32),
            AreaSizeType::Small => (600.0f32, 1800.0f32),
            AreaSizeType::Wide => (1000.0f32, 2800.0f32),
            AreaSizeType::Outdoor => (1250.0f32, 3200.0f32),
        };

        let mut tgt_watch_cir_r = tgt_watch_cir_r_base;
        let mut tgt_chara2watch_dist = 0.0f32;

        let dx = player_pos.x - eye.x;
        let dz = player_pos.z - eye.z;
        let cam2chr_dist = (dx * dx + dz * dz).sqrt();

        if rec.cam_mv_type == CameraMovementType::Settle {
            tgt_watch_cir_r *= 0.5;
        } else if cam2chr_dist < min_cam2watch_dist + tgt_watch_cir_r {
            tgt_chara2watch_dist = (min_cam2watch_dist + tgt_watch_cir_r) - cam2chr_dist;
        }

        // `SLUS_202.28` `vcSetWatchTgtXzPos` (`0x00199ca0`, `0x00199cdc..0x00199cf8`) calls
        // `shAtan2(dz, dx)` (`0x0016e9e0`), where Team Silent's `shAtan2(z, x)` returns the yaw
        // measured from `+Z` toward `+X` (`sin = dx/r`, `cos = dz/r`), i.e. Rust `dx.atan2(dz)`.
        let cam2chr_ang = dx.atan2(dz);
        let chr2watch_x =
            player_yaw.sin() * tgt_watch_cir_r + cam2chr_ang.sin() * tgt_chara2watch_dist;
        let chr2watch_z =
            player_yaw.cos() * tgt_watch_cir_r + cam2chr_ang.cos() * tgt_chara2watch_dist;

        let tgt_x = chr2watch_x + player_pos.x;
        let tgt_z = chr2watch_z + player_pos.z;
        let tgt_y = rec.ofs_watch_hy + (player_pos.y - 250.0);

        // 2. Initial angle calculation (vwVectorToAngle)
        let vec_to_tgt = Vec3::new(tgt_x - eye.x, tgt_y - eye.y, tgt_z - eye.z);
        let r_xz = (vec_to_tgt.x * vec_to_tgt.x + vec_to_tgt.z * vec_to_tgt.z).sqrt();
        let raw_ang_x = (-vec_to_tgt.y).atan2(r_xz);
        let raw_ang_y = vec_to_tgt.x.atan2(vec_to_tgt.z);

        // 3. Screen framing adjustment (vcAdjCamOfsAngByCharaInScreen)
        let scr_z = if effective_projection > 0.0 {
            effective_projection
        } else {
            448.0
        };
        let scr_sy = 448.0f32;
        let scr_half_wy = (scr_sy).atan2(scr_z) * 0.5;

        let chara_btm_y = rec.trace_btm_hy + (player_pos.y - 250.0);
        let chara_top_y = player_pos.y - 937.5;

        let vec_btm = Vec3::new(
            player_pos.x - eye.x,
            chara_btm_y - eye.y,
            player_pos.z - eye.z,
        );
        let r_btm = (vec_btm.x * vec_btm.x + vec_btm.z * vec_btm.z).sqrt();
        let btm_ang_x = (-vec_btm.y).atan2(r_btm);

        let vec_top = Vec3::new(
            player_pos.x - eye.x,
            chara_top_y - eye.y,
            player_pos.z - eye.z,
        );
        let r_top = (vec_top.x * vec_top.x + vec_top.z * vec_top.z).sqrt();
        let top_ang_x = (-vec_top.y).atan2(r_top);

        let w2c_btm = Self::normalize_angle(btm_ang_x - raw_ang_x);
        let w2c_top = Self::normalize_angle(top_ang_x - raw_ang_x);

        let mut adj_x = 0.0f32;
        if -scr_half_wy > w2c_btm {
            adj_x = w2c_btm + scr_half_wy;
        }
        let v = w2c_top - adj_x;
        if v > scr_half_wy {
            adj_x += v - scr_half_wy;
        }
        adj_x = adj_x.clamp(-FRAC_PI_6, FRAC_PI_6);

        let final_pitch = raw_ang_x + adj_x;
        let mut final_yaw = raw_ang_y;

        // 4. Yaw limit clamp (vcCorrectCamMatAngForcibly)
        let (lr_lim, rr_lim) = match rec.params {
            CamMovementParams::Chase(ref chs) => (chs.lr_lim_ang_y, chs.rr_lim_ang_y),
            CamMovementParams::Settle(ref stl) => (stl.lr_lim_ang_y, stl.rr_lim_ang_y),
            _ => (-17.451548, 17.451548),
        };

        let settle_or_like_settle = rec.cam_mv_type == CameraMovementType::Settle
            || (rec.cam_mv_type == CameraMovementType::Chase
                && (rec.flags & VC_RD_CAM_LIKE_SETTLE_F) != 0);

        if settle_or_like_settle && (lr_lim != -17.451548 || rr_lim != 17.451548) {
            let ofs_lr = Self::normalize_angle(final_yaw - lr_lim);
            let ofs_rr = Self::normalize_angle(final_yaw - rr_lim);
            if !(ofs_lr >= 0.0 && ofs_rr <= 0.0) {
                if ofs_lr.abs() < ofs_rr.abs() {
                    final_yaw = Self::normalize_angle(lr_lim);
                } else {
                    final_yaw = Self::normalize_angle(rr_lim);
                }
            }
        }

        // 5. Compute look target along forward vector
        let fwd_x = final_pitch.cos() * final_yaw.sin();
        let fwd_y = -final_pitch.sin();
        let fwd_z = final_pitch.cos() * final_yaw.cos();

        let look_target = Vec3::new(
            eye.x + fwd_x * 2000.0,
            eye.y + fwd_y * 2000.0,
            eye.z + fwd_z * 2000.0,
        );

        let mv_type = match rec.cam_mv_type {
            CameraMovementType::Settle => CameraMovementType::Settle,
            CameraMovementType::SelfView => CameraMovementType::SelfView,
            CameraMovementType::LocusCircle => CameraMovementType::LocusCircle,
            CameraMovementType::ThroughDoor => CameraMovementType::ThroughDoor,
            _ => CameraMovementType::Chase,
        };

        (eye, look_target, fov_y, mv_type)
    }

    /// Normalizes an angle in radians to `[-PI, PI)`.
    #[inline]
    fn normalize_angle(mut a: f32) -> f32 {
        while a < -PI {
            a += 2.0 * PI;
        }
        while a >= PI {
            a -= 2.0 * PI;
        }
        a
    }

    /// Reset camera position instantly behind the player without smoothing (e.g. on new game, door warps).
    pub fn reset_to_player(&mut self, player_pos: Vec3, player_yaw: f32) {
        let (eye, target, fov_y, mv_type) = if let Some(ref cam_file) = self.cam_file {
            let player_arr = [player_pos.x, player_pos.y, player_pos.z];
            if let Some(rec) = cam_file.find_active_record(player_arr, 100.0) {
                if let Some(idx) = cam_file.records.iter().position(|r| std::ptr::eq(r, rec)) {
                    self.active_road_index = Some(idx);
                }
                let (eval_eye, eval_look, eval_fov, eval_mv) =
                    Self::evaluate_road_record(rec, player_pos, player_yaw);
                let (eye, target) = crate::widescreen_rail::apply_chase_rail_clearance(
                    rec,
                    self.active_room_id.as_deref(),
                    eval_eye,
                    eval_look,
                    player_pos,
                    player_yaw,
                    &self.aspect_rail_config,
                );
                (eye, target, eval_fov, eval_mv)
            } else {
                Self::fallback_placement(player_pos, player_yaw)
            }
        } else {
            Self::fallback_placement(player_pos, player_yaw)
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
    }

    /// Fallback camera placement when no `.cam` record covers the coordinates.
    fn fallback_placement(
        player_pos: Vec3,
        player_yaw: f32,
    ) -> (Vec3, Vec3, f32, CameraMovementType) {
        let forward = Vec3::new(player_yaw.sin(), 0.0, player_yaw.cos());
        let eye = if player_pos.x > -23000.0
            && player_pos.x < -17000.0
            && player_pos.z > 18000.0
            && player_pos.z < 23000.0
        {
            // Stage 2 er01 opening restroom fallback (Record 2 sink placement)
            player_pos - forward * 795.0 + Vec3::new(0.0, -900.0, 0.0)
        } else if player_pos.x < -50000.0 {
            // Stage rr21 hotel boiler room entrance fallback
            Vec3::new(-99700.0, -1450.0, -59600.0)
        } else {
            player_pos - forward * 2000.0 + Vec3::new(0.0, -1400.0, 0.0)
        };

        let target = if player_pos.x > -23000.0
            && player_pos.x < -17000.0
            && player_pos.z > 18000.0
            && player_pos.z < 23000.0
        {
            player_pos + Vec3::new(0.0, -1100.0, 0.0)
        } else if player_pos.x < -50000.0 {
            player_pos + Vec3::new(0.0, -850.0, 0.0)
        } else {
            player_pos + Vec3::new(0.0, -1100.0, 0.0)
        };

        (eye, target, FRAC_PI_4, CameraMovementType::Chase)
    }

    /// Step the camera simulation by one 60 Hz tick.
    ///
    /// Evaluates active `ThroughDoorSession`, `.cam` volume triggers with `vcSetCurNearRoadInVC_WORK`
    /// hysteresis, and authentic PS2 `SelfView` / `FarWatch` / `vcCamMatNoise` framing.
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
        const DT_60HZ: f32 = 1.0 / 60.0;
        self.prev_snapshot = self.curr_snapshot;
        self.search_yaw_offset = neck_ang_y;
        self.search_pitch_offset = neck_ang_x;

        let target_eye: Vec3;
        let mut target_look: Vec3;
        let target_fov: f32;
        let target_mv_type: CameraMovementType;
        let mut cur_road_flags: u16 = 0;
        let mut cur_area_size = AreaSizeType::Wide;
        let mut cur_projection: f32 = 448.0;
        let mut warp_cut = false;
        let mut eye_lerp = crate::widescreen_rail::DEFAULT_SMOOTH_WARP_EYE_LERP;
        let mut target_lerp = crate::widescreen_rail::DEFAULT_SMOOTH_WARP_TARGET_LERP;

        let nearest_enemy_dist = self.nearest_enemy.map(|(d, _)| d).unwrap_or(7500.0);

        if let Some(td_eye) = self.through_door_session.step(
            DT_60HZ,
            player_pos,
            player_yaw,
            self.nearest_enemy.map(|(d, _)| d),
        ) {
            target_eye = td_eye;
            target_look = player_pos + Vec3::new(0.0, -450.0, 0.0);
            target_fov = FRAC_PI_4;
            target_mv_type = CameraMovementType::ThroughDoor;
        } else if let Some(ref cam_file) = self.cam_file {
            // Select best candidate road using `vcRetRoadUsePriority` (`vcRetPrioStgEvnt`)
            // and `vcSetCurNearRoadInVC_WORK` hysteresis (`0x00197af0`).
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

            if let Some(best_cand) = best {
                let decision = road_priority::resolve_cur_near_road_hysteresis(
                    old_cand, best_cand, player_yaw,
                );
                let rec = &cam_file.records[decision.selected_index];
                if decision.switched {
                    if let Some(old_idx) = self.active_road_index {
                        let old_rec = &cam_file.records[old_idx];
                        if decision.warp_cut {
                            match crate::widescreen_rail::evaluate_intra_room_transition(
                                old_rec.flags,
                                rec.flags,
                                self.aspect_rail_config.transition_policy,
                            ) {
                                crate::widescreen_rail::IntraRoomTransitionStep::InstantWarpCut => {
                                    warp_cut = true;
                                }
                                crate::widescreen_rail::IntraRoomTransitionStep::SmoothBlend(
                                    factors,
                                ) => {
                                    eye_lerp = factors.eye_lerp;
                                    target_lerp = factors.target_lerp;
                                }
                            }
                        }
                    }
                    self.active_road_index = Some(decision.selected_index);
                }

                cur_road_flags = rec.flags;
                cur_area_size = rec.area_size_type;
                cur_projection = if rec.projection > 0.0 {
                    rec.projection
                } else {
                    448.0
                };

                let (eval_eye, eval_look, eval_fov, eval_mv) =
                    Self::evaluate_road_record(rec, player_pos, player_yaw);
                (target_eye, target_look) = crate::widescreen_rail::apply_chase_rail_clearance(
                    rec,
                    self.active_room_id.as_deref(),
                    eval_eye,
                    eval_look,
                    player_pos,
                    player_yaw,
                    &self.aspect_rail_config,
                );
                target_fov = eval_fov;
                target_mv_type = eval_mv;
            } else {
                let (fb_eye, fb_look, fb_fov, fb_mv) =
                    Self::fallback_placement(player_pos, player_yaw);
                target_eye = fb_eye;
                target_look = fb_look;
                target_fov = fb_fov;
                target_mv_type = fb_mv;
            }
        } else {
            let (fb_eye, fb_look, fb_fov, fb_mv) = Self::fallback_placement(player_pos, player_yaw);
            target_eye = fb_eye;
            target_look = fb_look;
            target_fov = fb_fov;
            target_mv_type = fb_mv;
        }

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

        let head_pos = Vec3::new(player_pos.x, player_pos.y - 937.5, player_pos.z);
        let ideal_cam_pos_h = head_pos.y + if is_outdoor { -450.0 } else { -200.0 };
        let nearest_item_pair = self
            .nearest_item
            .center_y
            .map(|cy| (self.nearest_item.dist_mm, cy));

        if target_mv_type == CameraMovementType::SelfView || far_watch.rate > 0.0 {
            let far_tgt = self_view::make_far_watch_tgt_pos(&self_view::FarWatchInput {
                stage_id: self.stage_id,
                area_size_type: cur_area_size,
                on_boat: self.on_boat,
                cam_pos: target_eye,
                chara_pos: player_pos,
                chara_yaw: player_yaw,
                ideal_cam_pos_h,
                neck_ang_x,
                neck_ang_y,
                nearest_enemy: self.nearest_enemy,
                nearest_item: nearest_item_pair,
            });
            target_look = if target_mv_type == CameraMovementType::SelfView {
                far_tgt
            } else {
                target_look.lerp(far_tgt, far_watch.rate)
            };
        }

        let sv_eff_rate = self_view::ret_self_view_effect_rate(
            target_mv_type,
            target_eye,
            head_pos,
            nearest_enemy_dist,
        );
        if sv_eff_rate > 0.0 {
            let (mixed_tgt, mixed_ang_z) = self_view::mix_self_view_effect_to_watch_tgt_pos(
                sv_eff_rate,
                target_look,
                self.raw_state.watch_tgt_ang_z,
                target_eye,
                player_yaw,
                [neck_ang_x, player_yaw + neck_ang_y, 0.0],
                0,
                self.nearest_enemy.is_some(),
            );
            target_look = mixed_tgt;
            self.raw_state.watch_tgt_ang_z = mixed_ang_z;
        }

        if (cur_road_flags & VC_RD_LIM_UP_FAR_VIEW_F) != 0
            && (target_mv_type == CameraMovementType::Chase
                || target_mv_type == CameraMovementType::SelfView)
        {
            target_look = self_view::adjust_watch_y_limit_high_when_far_view(
                target_look,
                target_eye,
                cur_projection,
                448.0,
                self.stage_id,
            );
        }

        self.mv_type = target_mv_type;

        // If warp cut is signaled (instant cut across cinematic cameras), snap instantly
        if warp_cut {
            self.ideal_eye = target_eye;
            self.ideal_target = target_look;
        } else {
            // Authentic 60 Hz exponential smoothing across volume boundaries
            self.ideal_eye = self.ideal_eye.lerp(target_eye, eye_lerp);
            self.ideal_target = self.ideal_target.lerp(target_look, target_lerp);
        }

        let mut final_target = self.ideal_target;

        // Authentic PS2 Mode 3 (`SELF_VIEW`) camera matrix breathing noise (`vcCamMatNoise` `0x0019d8e0`).
        if self.mv_type == CameraMovementType::SelfView {
            self.self_view_timer_sec += DT_60HZ;
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
            x1: max_x,
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
            mv_y_type: 0,
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
