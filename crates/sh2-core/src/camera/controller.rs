//! Camera controller state, configuration, room `.cam` loading, and subframe interpolation.

use crate::types::camera::CameraState;
use glam::{Mat4, Vec3};
use sh2_assets::formats::cam::{CamFile, CamSourceKind};
use sh2_assets::AssetPathResolver;

use super::{fixed_angle, kinematics, locus_door, road_priority, CameraMovementType};

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

    /// Compute sub-frame interpolated PS2 `VbWvsMatrix` (`+X` right, `+Y` down, `+Z` forward;
    /// `SLUS_202.28` `view/vb_main.c` `0x010e59d0` / `sh2pc.exe` `0x01f80e00`) for
    /// `SeCallPosDirection` (`0x00192990` / `0x00515e60`) and `RadioNoise` (`0x001ab300`).
    pub fn interpolated_ps2_wvs_matrix(&self, alpha: f32) -> Mat4 {
        let eye = self.prev_snapshot.eye.lerp(self.curr_snapshot.eye, alpha);
        let target = self
            .prev_snapshot
            .target
            .lerp(self.curr_snapshot.target, alpha);
        let up = self.prev_snapshot.up;

        ps2_wvs_matrix_from_look_at(eye, target, up)
    }

    /// Interpolated field-of-view in radians.
    pub fn interpolated_fov(&self, alpha: f32) -> f32 {
        self.prev_snapshot.fov_y + (self.curr_snapshot.fov_y - self.prev_snapshot.fov_y) * alpha
    }
}

/// Builds a PS2 `VbWvsMatrix` (`+X` right, `+Y` down, `+Z` forward; `SLUS_202.28` `view/vb_main.c`
/// `0x010e59d0` / `sh2pc.exe` `0x01f80e00`) from an SH2 `+Y`-down `(eye, target, up)` camera pose.
///
/// Unlike [`Mat4::look_at_rh`] (which places the forward axis on `-Z`), `SeCallPosDirection`
/// (`0x00192990` / `0x00515e60`) expects `+Z` forward so sounds straight ahead yield `nz = +1.0`
/// and pan to `0°` rather than `±140°`.
pub fn ps2_wvs_matrix_from_look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let forward = target - eye;
    if !forward.is_finite() || forward.length_squared() <= 1e-8 {
        return Mat4::from_translation(-eye);
    }
    let f = forward.normalize();
    let r = f.cross(up).normalize_or(Vec3::X);
    let d = f.cross(r);
    Mat4::from_cols(
        glam::Vec4::new(r.x, d.x, f.x, 0.0),
        glam::Vec4::new(r.y, d.y, f.y, 0.0),
        glam::Vec4::new(r.z, d.z, f.z, 0.0),
        glam::Vec4::new(-r.dot(eye), -d.dot(eye), -f.dot(eye), 1.0),
    )
}
