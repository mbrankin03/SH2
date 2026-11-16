//! Enemy encounter `.cam` road selection coordinator (`VC_RD_USE_NEAR_ENEMY_F`,
//! `VC_RD_USE_NO_ENEMY_F`), `SelfViewGate`, `old_road_advantage_mm` hysteresis, and warp-safe
//! encounter switching (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` references:
//! - `vcSetNearestEnemyDataInVC_WORK` (`0x00196f20`)
//! - `vcSetNearRoadAryByCharaPos` (`0x00197310`, `0x19738c..0x1973b8`)
//! - `vcRetRoadUsePriority` (`0x00197a10`) & `vcAdvantageDistOfOldCurRoad` (`0x00198ae0`)
//! - `vcGetBestNewCurNearRoad` (`0x00198260`) & `vcSetCurNearRoadInVC_WORK` (`0x00197ba4`)

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

/// Switch-volume containment margin in PS2 millimeters during `reset_camera_with_enemies`
/// (matches `Ps2CameraController::reset_to_player`).
pub const VC_RESET_ROAD_MARGIN_MM: f32 = 100.0;

/// Switch-volume containment margin in PS2 millimeters during `tick_camera_with_enemies`
/// (matches `Ps2CameraController::tick`).
pub const VC_TICK_ROAD_MARGIN_MM: f32 = 150.0;

/// Zero-allocation runtime coordinator for enemy-encounter `.cam` volume framing (`vc_main.c`).
#[derive(Debug, Clone, Default)]
pub struct EnemyEncounterFraming {
    /// Most recent enemy proximity evaluation (`vcWork.nearest_enemy_p` / `nearest_enemy_xz_dist`).
    pub proximity: EnemyProximityState,
    /// Stage-event road priority gate (`vcRetPrioStgEvnt`).
    pub event_gate: StageEventRoadGate,
    /// Search self-view road priority gate (`VC_PRS_F_VIEW_F`).
    pub self_view: SelfViewGate,
    /// Reusable 1-element scratch buffer feeding the winning road into `Ps2CameraController`.
    scratch_records: Vec<RoadRecord>,
}

impl EnemyEncounterFraming {
    /// Creates a new enemy-encounter framing tracker in `Clear` proximity state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets `camera` to `player_pos` / `player_yaw` (in PS2 mm and radians) using
    /// [`select_best_road_index`] (`0x0020`/`0x0040`, `SelfViewGate`, `StageEventRoadGate`, and
    /// `old_road_advantage_mm` hysteresis).
    pub fn reset_camera_with_enemies(
        &mut self,
        camera: &mut Ps2CameraController,
        player_pos: Vec3,
        player_yaw: f32,
        player_suppression: PlayerCameraSuppression,
        enemies: &[EnemyCameraTarget],
    ) {
        self.proximity = evaluate_nearest_enemy(player_pos, player_suppression, enemies);
        let Some(orig_cam) = camera.cam_file.take() else {
            camera.reset_to_player(player_pos, player_yaw);
            return;
        };

        let chosen_canonical = select_best_road_index(
            &orig_cam,
            player_pos,
            VC_RESET_ROAD_MARGIN_MM,
            camera.active_road_index,
            self.proximity,
            self.event_gate,
            self.self_view,
        );
        self.run_on_selected_road(camera, &orig_cam, chosen_canonical, |cam| {
            cam.reset_to_player(player_pos, player_yaw);
        });
        camera.cam_file = Some(orig_cam);
        camera.active_road_index = chosen_canonical;
    }

    /// Steps `camera` by one 60 Hz tick using [`select_best_road_index`] (`0x0020`/`0x0040`,
    /// `SelfViewGate`, `StageEventRoadGate`, `old_road_advantage_mm` hysteresis) and warp-cut
    /// safety (`VC_RD_WARP_OUT_F` / `VC_RD_WARP_IN_F`).
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
        let Some(orig_cam) = camera.cam_file.take() else {
            camera.tick(player_pos, player_yaw, right_stick_x, right_stick_y);
            return;
        };

        let prev_canonical = camera.active_road_index;
        let chosen_canonical = select_best_road_index(
            &orig_cam,
            player_pos,
            VC_TICK_ROAD_MARGIN_MM,
            prev_canonical,
            self.proximity,
            self.event_gate,
            self.self_view,
        );

        let warp_cut = prev_canonical != chosen_canonical
            && (prev_canonical
                .and_then(|i| orig_cam.records.get(i))
                .is_some_and(|r| (r.flags & VC_RD_WARP_OUT_F) != 0)
                || chosen_canonical
                    .and_then(|i| orig_cam.records.get(i))
                    .is_some_and(|r| (r.flags & VC_RD_WARP_IN_F) != 0));

        self.run_on_selected_road(camera, &orig_cam, chosen_canonical, |cam| {
            if warp_cut {
                cam.reset_to_player(player_pos, player_yaw);
                if right_stick_x.abs() > f32::EPSILON || right_stick_y.abs() > f32::EPSILON {
                    cam.tick(player_pos, player_yaw, right_stick_x, right_stick_y);
                    cam.prev_snapshot = cam.curr_snapshot;
                }
            } else {
                cam.tick(player_pos, player_yaw, right_stick_x, right_stick_y);
            }
        });
        camera.cam_file = Some(orig_cam);
        camera.active_road_index = chosen_canonical;
    }

    fn run_on_selected_road<F>(
        &mut self,
        camera: &mut Ps2CameraController,
        orig_cam: &CamFile,
        chosen_canonical: Option<usize>,
        f: F,
    ) where
        F: FnOnce(&mut Ps2CameraController),
    {
        self.scratch_records.clear();
        if let Some(rec) = chosen_canonical.and_then(|idx| orig_cam.records.get(idx)) {
            self.scratch_records.push(rec.clone());
        }
        let single_cam = CamFile {
            records: std::mem::take(&mut self.scratch_records),
        };
        let has_rec = !single_cam.records.is_empty();
        camera.cam_file = Some(single_cam);
        camera.active_road_index = has_rec.then_some(0);

        f(camera);

        if let Some(mut temp_cam) = camera.cam_file.take() {
            temp_cam.records.clear();
            self.scratch_records = temp_cam.records;
        }
    }
}
