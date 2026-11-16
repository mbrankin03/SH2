//! Enemy encounter `.cam` road selection coordinator (`VC_RD_USE_NEAR_ENEMY_F`,
//! `VC_RD_USE_NO_ENEMY_F`) and warp-safe encounter switching (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` references:
//! - `vcSetNearestEnemyDataInVC_WORK` (`0x00196f20`)
//! - `vcSetNearRoadAryByCharaPos` (`0x00197310`, `0x19738c..0x1973b8`)
//! - `vcSetCurNearRoadInVC_WORK` (`0x00197ba4`): `VC_RD_WARP_IN_F` / `VC_RD_WARP_OUT_F` cuts

pub use super::enemy_road_eval::{
    effective_road_priority, evaluate_nearest_enemy, is_road_eligible_for_enemy,
    old_road_advantage_mm, select_best_road_index, EnemyCameraAlertState, EnemyCameraTarget,
    EnemyLifeCameraState, EnemyProximityState, PlayerCameraSuppression, SelfViewGate,
    StageEventRoadGate, VC_DEAD_ENEMY_GRACE_SECS, VC_DEAD_ENEMY_TIMER_MAX_SECS,
    VC_EVENT_EFFECT_ADVANTAGE_MM, VC_EXTRA_BOUNDARY_WIDTH_MM, VC_MAX_CAMERA_TARGETS,
    VC_NEAR_ENEMY_MAX_DIST_MM,
};

use crate::camera::Ps2CameraController;
use glam::Vec3;
use sh2_assets::formats::cam::{CamFile, RoadRecord, VC_RD_WARP_IN_F, VC_RD_WARP_OUT_F};

/// Zero-allocation runtime coordinator for enemy-encounter `.cam` volume framing (`vc_main.c`).
#[derive(Debug, Clone, Default)]
pub struct EnemyEncounterFraming {
    /// Most recent enemy proximity evaluation (`vcWork.nearest_enemy_p` / `nearest_enemy_xz_dist`).
    pub proximity: EnemyProximityState,
    /// Stage-event road priority gate (`vcRetPrioStgEvnt`).
    pub event_gate: StageEventRoadGate,
    /// Search self-view road priority gate (`VC_PRS_F_VIEW_F`).
    pub self_view: SelfViewGate,
    /// Reusable scratch list of eligible road records to avoid per-tick allocations.
    scratch_records: Vec<RoadRecord>,
    /// Reusable mapping from `scratch_records` index back to canonical `CamFile::records` index.
    scratch_indices: Vec<usize>,
}

impl EnemyEncounterFraming {
    /// Creates a new enemy-encounter framing tracker in `Clear` proximity state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets `camera` to `player_pos` / `player_yaw` (in PS2 mm and radians) while filtering
    /// `.cam` records by `VC_RD_USE_NEAR_ENEMY_F` (`0x0040`) and `VC_RD_USE_NO_ENEMY_F` (`0x0020`).
    pub fn reset_camera_with_enemies(
        &mut self,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        player_suppression: PlayerCameraSuppression,
        enemies: &[EnemyCameraTarget],
    ) {
        self.proximity = evaluate_nearest_enemy(player_pos, player_suppression, enemies);
        self.with_filtered_cam_file(camera, |cam, _| {
            cam.reset_to_player(player_pos, player_yaw);
        });
    }

    /// Steps `camera` by one 60 Hz tick while enforcing `VC_RD_USE_NEAR_ENEMY_F` (`0x0040`),
    /// `VC_RD_USE_NO_ENEMY_F` (`0x0020`), and warp-cut safety across enemy encounter transitions.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_camera_with_enemies(
        &mut self,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        right_stick_x: f32,
        right_stick_y: f32,
        player_suppression: PlayerCameraSuppression,
        enemies: &[EnemyCameraTarget],
    ) {
        self.proximity = evaluate_nearest_enemy(player_pos, player_suppression, enemies);
        let prev_canonical = camera.active_road_index;
        let prev_warp_out = prev_canonical
            .and_then(|i| camera.cam_file.as_ref()?.records.get(i))
            .is_some_and(|r| (r.flags & VC_RD_WARP_OUT_F) != 0);

        self.with_filtered_cam_file(camera, |cam, prev_became_filtered_out| {
            let pre_tick_prev = cam.curr_snapshot;
            cam.tick(player_pos, player_yaw, right_stick_x, right_stick_y);

            // When the previously active road was just disqualified by enemy appearance/death,
            // enforce `VC_RD_WARP_OUT_F` / `VC_RD_WARP_IN_F` (`vcSetCurNearRoadInVC_WORK` `0x00197ba4`).
            if prev_became_filtered_out {
                let new_warp_in = cam
                    .active_road_index
                    .and_then(|fi| cam.cam_file.as_ref()?.records.get(fi))
                    .is_some_and(|r| (r.flags & VC_RD_WARP_IN_F) != 0);
                if prev_warp_out || new_warp_in {
                    cam.reset_to_player(player_pos, player_yaw);
                } else {
                    cam.prev_snapshot = pre_tick_prev;
                }
            }
        });
    }

    fn with_filtered_cam_file<F>(&mut self, camera: &mut Ps2CameraController, f: F)
    where
        F: FnOnce(&mut Ps2CameraController, bool),
    {
        let Some(orig_cam) = camera.cam_file.take() else {
            f(camera, false);
            return;
        };

        let proximity = self.proximity;
        let event_gate = self.event_gate;
        let self_view = self.self_view;
        let needs_filtering = orig_cam.records.iter().any(|r| {
            !is_road_eligible_for_enemy(r.flags, proximity)
                || effective_road_priority(r, event_gate, self_view) != r.rd_type.priority()
        });

        if !needs_filtering {
            camera.cam_file = Some(orig_cam);
            f(camera, false);
            return;
        }

        self.scratch_records.clear();
        self.scratch_indices.clear();
        for (idx, rec) in orig_cam.records.iter().enumerate() {
            let prio = effective_road_priority(rec, event_gate, self_view);
            if is_road_eligible_for_enemy(rec.flags, proximity) && prio > 0 {
                self.scratch_records.push(rec.clone());
                self.scratch_indices.push(idx);
            }
        }

        let old_canonical = camera.active_road_index;
        let mapped_old =
            old_canonical.and_then(|orig| self.scratch_indices.iter().position(|&i| i == orig));
        let prev_became_filtered_out = old_canonical.is_some() && mapped_old.is_none();

        let filtered_cam = CamFile {
            records: std::mem::take(&mut self.scratch_records),
        };
        camera.cam_file = Some(filtered_cam);
        camera.active_road_index = mapped_old;

        f(camera, prev_became_filtered_out);

        let new_canonical = camera
            .active_road_index
            .and_then(|fi| self.scratch_indices.get(fi).copied());
        if let Some(mut temp_cam) = camera.cam_file.take() {
            temp_cam.records.clear();
            self.scratch_records = temp_cam.records;
        }
        camera.cam_file = Some(orig_cam);
        camera.active_road_index = new_canonical;
    }
}
