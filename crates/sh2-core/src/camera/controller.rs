//! Camera controller state, configuration, room `.cam` loading, and subframe interpolation.

use crate::types::camera::CameraState;
use glam::{Mat4, Vec3};
use sh2_assets::formats::cam::{CamFile, CamSourceKind};
use sh2_assets::AssetPathResolver;

use super::{
    fixed_angle, kinematics, locus_door, road_bank, road_priority, CameraMovementType,
    PS2_DEFAULT_PROJECTION_Z,
};

/// Gameplay projection focal-distance ramp state mirroring `_vcProjectionParam`
/// (`SLUS_202.28` `0x010e5a90`, `vcSetProjectionValue` `0x0019db80` &
/// `vcChangeProjectionValue` `0x0019dc50`, `vc_main.c:4101-4160`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectionRampState {
    /// Active screen projection distance `scr_z` (`_VbScreenInfo`, default `448.0`).
    pub current_scr_z: f32,
    /// Starting `scr_z` when the current ramp was scheduled (`_DAT_010e5a94`).
    pub start_scr_z: f32,
    /// Target `scr_z` (`_vcProjectionParam`).
    pub target_scr_z: f32,
    /// Per-tick delta (`_DAT_010e5a98 = (target - start) / frames`).
    pub step_per_tick: f32,
    /// Ramp mode (`_DAT_010e5a9c`: `0` = idle, `1` = linear ramp, `2` = instant snap).
    pub mode: u8,
}

impl Default for ProjectionRampState {
    fn default() -> Self {
        Self {
            current_scr_z: PS2_DEFAULT_PROJECTION_Z,
            start_scr_z: PS2_DEFAULT_PROJECTION_Z,
            target_scr_z: PS2_DEFAULT_PROJECTION_Z,
            step_per_tick: 0.0,
            mode: 0,
        }
    }
}

impl ProjectionRampState {
    /// Schedules a projection change (`SLUS_202.28` `vcSetProjectionValue` `0x0019db80`).
    pub fn schedule(&mut self, scr_z: f32, frames: u16) {
        let target = if scr_z == 0.0 {
            PS2_DEFAULT_PROJECTION_Z
        } else {
            scr_z
        };
        self.target_scr_z = target;
        self.start_scr_z = self.current_scr_z;
        if frames < 2 {
            self.step_per_tick = 0.0;
            self.mode = 2;
        } else {
            self.step_per_tick = (target - self.current_scr_z) / f32::from(frames);
            self.mode = 1;
        }
    }

    /// Steps the active projection ramp by one tick (`SLUS_202.28` `vcChangeProjectionValue` `0x0019dc50`).
    pub fn step(&mut self) -> f32 {
        match self.mode {
            2 => {
                self.mode = 0;
                self.current_scr_z = self.target_scr_z;
            }
            1 => {
                self.current_scr_z += self.step_per_tick;
                if self.start_scr_z < self.target_scr_z {
                    if self.current_scr_z > self.target_scr_z {
                        self.mode = 0;
                        self.current_scr_z = self.target_scr_z;
                    }
                } else if self.current_scr_z < self.target_scr_z {
                    self.mode = 0;
                    self.current_scr_z = self.target_scr_z;
                }
            }
            _ => {}
        }
        self.current_scr_z
    }
}

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
    /// Loaded `.cam` trigger volume definition file for the active room (or merged outdoor global + tile bank).
    pub cam_file: Option<CamFile>,
    /// Multi-slot resident outdoor `.cam` bank keeping `<area>GB.cam` alongside active tile `.cam`s (`SH2-184`).
    pub road_bank: road_bank::OutdoorCamBank,
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
    /// Player crouch blend rate `[0.0, 1.0]` (`_DAT_01f8db68` in `sh2pc.exe` `FUN_005193a0` `0x005193a0`).
    pub crouch_rate: f32,
    /// Active gameplay projection focal-distance ramp (`vcSetProjectionValue` `0x0019db80` / `vcChangeProjectionValue` `0x0019dc50`).
    pub projection_ramp: ProjectionRampState,
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
            road_bank: road_bank::OutdoorCamBank::default(),
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
            crouch_rate: 0.0,
            projection_ramp: ProjectionRampState::default(),
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
        self.road_bank.clear();
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
        self.road_bank.clear();
        self.cam_file = cam_file;
        self.active_room_id = room_id;
        self.active_cam_source = source;
        self.active_road_index = None;
    }

    /// Returns the resident slot origin `(CamSlotKey, local_record_idx)` of the currently active
    /// road when managed by [`OutdoorCamBank`](road_bank::OutdoorCamBank) (`SH2-184`).
    #[inline]
    #[must_use]
    pub fn active_road_origin(&self) -> Option<&(road_bank::CamSlotKey, usize)> {
        self.active_road_index
            .and_then(|idx| self.road_bank.origin_of(idx))
    }

    /// Ensures the outdoor area's global `<area>GB.cam` is resident in Slot 0 of `self.road_bank`
    /// (`SLUS_202.28` `loadBgCAM_ReplaceG` `0x00271d20` / `sh2pc.exe` `FUN_005132c0` `0x005132c0`),
    /// reusing the already-loaded buffer when `(gb_id, prefer_sh2ee)` has not changed.
    fn ensure_global_cam_resident(
        &mut self,
        resolver: &AssetPathResolver,
        gb_rel_path: &str,
        prefer_sh2ee: bool,
    ) {
        let Some((gb_id, _)) = sh2_assets::formats::cam::normalize_room_cam_rel_path(gb_rel_path)
        else {
            return;
        };
        if self.road_bank.has_global(&gb_id, prefer_sh2ee) {
            return;
        }
        match resolver.load_single_cam_file(gb_rel_path, prefer_sh2ee) {
            Ok(Some(asset)) => {
                self.road_bank
                    .set_global_slot(Some(road_bank::LoadedCamSlot {
                        key: road_bank::CamSlotKey::Global(asset.room_id),
                        source: asset.source,
                        prefer_sh2ee,
                        cam_file: asset.cam_file,
                    }));
            }
            _ => {
                self.road_bank.set_global_slot(None);
            }
        }
    }

    /// Automatically resolves and ingests the `.cam` trigger volume file for `room_or_rel_path`
    /// (`"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`, or `"bg/er/er01.cam"`), prioritizing
    /// SH2:EE widescreen/high-framerate overrides (`sh2e/bg/<area>/<room>.cam` and
    /// `sh2e/bg2/<area>/<room>.cam`) when `prefer_sh2ee` is `true` before falling back to
    /// vanilla retail `data/bg/...` (`SH2-50`).
    ///
    /// For outdoor areas (`ca*`, `cb*`, `cc*`, `cd*`), populates `road_bank` with Slot 0
    /// `<area>GB.cam` (`loadBgCAM_ReplaceG` `0x00271d20`) alongside the tile's `.cam`
    /// (`loadBgCAM_Replace` `0x00271e10`, `SH2-184`).
    ///
    /// Returns `true` if a valid `.cam` file was found and loaded.
    pub fn load_room_cam_with_resolver(
        &mut self,
        resolver: &AssetPathResolver,
        room_or_rel_path: &str,
        prefer_sh2ee: bool,
    ) -> bool {
        self.road_bank.clear();
        self.active_road_index = None;

        if let Some(gb_rel) = sh2_assets::formats::cam::global_area_cam_rel_path(room_or_rel_path) {
            self.ensure_global_cam_resident(resolver, &gb_rel, prefer_sh2ee);
            let mut tile_id: Option<String> = None;
            let mut source: Option<CamSourceKind> = None;
            if let Ok(Some(asset)) = resolver.load_single_cam_file(room_or_rel_path, prefer_sh2ee) {
                tile_id = Some(asset.room_id.clone());
                source = Some(asset.source);
                self.road_bank.upsert_tile_slot(road_bank::LoadedCamSlot {
                    key: road_bank::CamSlotKey::Tile(asset.room_id),
                    source: asset.source,
                    prefer_sh2ee,
                    cam_file: asset.cam_file,
                });
            }
            let merged = self.road_bank.build_merged_cam(tile_id.as_deref());
            let has_cam = merged.is_some();
            self.cam_file = merged;
            self.active_room_id = tile_id.or_else(|| {
                self.road_bank
                    .global_cam_id()
                    .map(std::string::ToString::to_string)
            });
            self.active_cam_source =
                source.or_else(|| self.road_bank.global_slot().map(|s| s.source));
            return has_cam;
        }

        match resolver.load_single_cam_file(room_or_rel_path, prefer_sh2ee) {
            Ok(Some(asset)) => {
                self.cam_file = Some(asset.cam_file);
                self.active_room_id = Some(asset.room_id);
                self.active_cam_source = Some(asset.source);
                true
            }
            _ => {
                self.cam_file = None;
                self.active_room_id = None;
                self.active_cam_source = None;
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

    /// Synchronizes `road_bank` with an outdoor [`TileStreamingPlan`](crate::world::TileStreamingPlan),
    /// keeping `plan.priority_gb_cam` (`caGB.cam` / `cbGB.cam`) resident in Slot 0 alongside every
    /// desired tile's `.cam` (`SLUS_202.28` `loadBgCAM_ReplaceG` `0x00271d20` +
    /// `loadBg2x2_ActivateRequestOutdoor` `0x00270590` + `loadBgCAM_vcReset` `0x00271f90`,
    /// `sh2pc.exe` `FUN_00513260` `0x00513260`, `SH2-184`).
    ///
    /// Preserves camera position, velocity, snapshots, and remaps `active_road_index` by
    /// `(CamSlotKey, local_record_idx)` so `vcSetCurNearRoadInVC_WORK` (`0x00197af0`) hysteresis
    /// operates seamlessly across outdoor tile boundaries with zero snap cuts.
    pub fn sync_outdoor_cam_plan_with_resolver(
        &mut self,
        resolver: &AssetPathResolver,
        plan: &crate::world::TileStreamingPlan,
        active_room_id: &str,
        prefer_sh2ee: bool,
    ) -> bool {
        let Some(area) = plan.area else {
            return false;
        };

        let prev_origin = self.active_road_origin().cloned();

        // Slot 0: keep `<area>GB.cam` resident without reloading when unchanged.
        if let Some(gb_rel) = plan.priority_gb_cam {
            self.ensure_global_cam_resident(resolver, gb_rel, prefer_sh2ee);
        } else {
            self.road_bank.set_global_slot(None);
        }

        // Slots 1..N: load any newly desired tile `.cam`s and evict tiles outside `desired_blocks`.
        let prefix = area.prefix();
        let mut keep_ids = Vec::with_capacity(plan.desired_blocks.len());
        for &block_no in &plan.desired_blocks {
            let tile_id = format!("{prefix}{block_no:02}");
            if !self.road_bank.has_tile(&tile_id, prefer_sh2ee) {
                if let Ok(Some(asset)) = resolver.load_single_cam_file(&tile_id, prefer_sh2ee) {
                    self.road_bank.upsert_tile_slot(road_bank::LoadedCamSlot {
                        key: road_bank::CamSlotKey::Tile(asset.room_id),
                        source: asset.source,
                        prefer_sh2ee,
                        cam_file: asset.cam_file,
                    });
                }
            }
            keep_ids.push(tile_id);
        }
        self.road_bank.retain_tiles(&keep_ids);

        let canonical_active = active_room_id.to_ascii_lowercase();
        let merged = self.road_bank.build_merged_cam(Some(&canonical_active));
        let has_cam = merged.is_some();
        self.cam_file = merged;
        self.active_room_id = Some(canonical_active.clone());
        self.active_cam_source = self
            .road_bank
            .tile_slots()
            .iter()
            .find(|s| s.key.id().eq_ignore_ascii_case(&canonical_active))
            .map(|s| s.source)
            .or_else(|| self.road_bank.global_slot().map(|s| s.source));
        // Preserve active road identity across tile swaps (`vcSetCurNearRoadInVC_WORK` `0x00197af0`).
        self.active_road_index = self.road_bank.remap_active_road_index(prev_origin.as_ref());
        has_cam
    }

    /// Synchronizes `road_bank` with `plan` using the default [`AssetPathResolver`] (`SH2-184`).
    pub fn sync_outdoor_cam_plan(
        &mut self,
        plan: &crate::world::TileStreamingPlan,
        active_room_id: &str,
        prefer_sh2ee: bool,
    ) -> bool {
        let resolver = AssetPathResolver::resolve();
        self.sync_outdoor_cam_plan_with_resolver(&resolver, plan, active_room_id, prefer_sh2ee)
    }

    /// Seamlessly activates `tile_room_id` (e.g. `"ca12"`, `"cb55"`) while keeping `<area>GB.cam`
    /// resident in Slot 0 and retaining up to `PS2_LOADBG_2X2_OUTDOOR_CAM_SLOTS` (4) recent
    /// same-area tile `.cam` slots so the previous tile stays resident across the boundary
    /// (`loadBgCAM_Replace` `0x00271e10` + `loadBgCAM_vcReset` `0x00271f90`, `SH2-184`).
    ///
    /// Never resets camera position, velocity, snapshots, or `active_road_index` when the active
    /// road's slot remains resident.
    pub fn switch_outdoor_tile_cam_with_resolver(
        &mut self,
        resolver: &AssetPathResolver,
        tile_room_id: &str,
        prefer_sh2ee: bool,
    ) -> bool {
        let Some((canonical_tile, _)) =
            sh2_assets::formats::cam::normalize_room_cam_rel_path(tile_room_id)
        else {
            return false;
        };

        let prev_origin = self.active_road_origin().cloned();

        if let Some(gb_rel) = sh2_assets::formats::cam::global_area_cam_rel_path(&canonical_tile) {
            self.ensure_global_cam_resident(resolver, &gb_rel, prefer_sh2ee);
        }

        if !self.road_bank.has_tile(&canonical_tile, prefer_sh2ee) {
            if let Ok(Some(asset)) = resolver.load_single_cam_file(&canonical_tile, prefer_sh2ee) {
                self.road_bank.upsert_tile_slot(road_bank::LoadedCamSlot {
                    key: road_bank::CamSlotKey::Tile(asset.room_id),
                    source: asset.source,
                    prefer_sh2ee,
                    cam_file: asset.cam_file,
                });
            }
        }
        if let Some(area_prefix) = canonical_tile.get(0..2) {
            self.road_bank
                .retain_area_and_cap(area_prefix, road_bank::PS2_LOADBG_2X2_OUTDOOR_CAM_SLOTS);
        }

        let merged = self.road_bank.build_merged_cam(Some(&canonical_tile));
        let has_cam = merged.is_some();
        self.cam_file = merged;
        self.active_room_id = Some(canonical_tile.clone());
        self.active_cam_source = self
            .road_bank
            .tile_slots()
            .iter()
            .find(|s| s.key.id().eq_ignore_ascii_case(&canonical_tile))
            .map(|s| s.source)
            .or_else(|| self.road_bank.global_slot().map(|s| s.source));
        self.active_road_index = self.road_bank.remap_active_road_index(prev_origin.as_ref());
        has_cam
    }

    /// Seamlessly activates `tile_room_id` using the default [`AssetPathResolver`] (`SH2-184`).
    pub fn switch_outdoor_tile_cam(&mut self, tile_room_id: &str, prefer_sh2ee: bool) -> bool {
        let resolver = AssetPathResolver::resolve();
        self.switch_outdoor_tile_cam_with_resolver(&resolver, tile_room_id, prefer_sh2ee)
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
        let up = self
            .prev_snapshot
            .up
            .lerp(self.curr_snapshot.up, alpha)
            .normalize_or(Vec3::new(0.0, -1.0, 0.0));

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
        let up = self
            .prev_snapshot
            .up
            .lerp(self.curr_snapshot.up, alpha)
            .normalize_or(Vec3::new(0.0, -1.0, 0.0));

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
