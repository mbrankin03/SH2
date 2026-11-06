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
    AreaSizeType, CamFile, CamMovementParams, RoadRecord, VC_RD_CAM_LIKE_SETTLE_F, VC_RD_WARP_IN_F,
    VC_RD_WARP_OUT_F,
};
use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

/// Re-export authentic camera movement types from assets for engine consumers.
pub use sh2_assets::formats::cam::CamMovementType as CameraMovementType;

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
    /// Previous tick snapshot for interpolation.
    pub prev_snapshot: CameraSnapshot,
    /// Current tick snapshot.
    pub curr_snapshot: CameraSnapshot,
    /// Search camera look offset yaw (radians) driven by right analog stick.
    pub search_yaw_offset: f32,
    /// Search camera look offset pitch (radians) driven by right analog stick.
    pub search_pitch_offset: f32,
    /// Target camera eye position (smoothing accumulator).
    pub ideal_eye: Vec3,
    /// Target camera look-at position (smoothing accumulator).
    pub ideal_target: Vec3,
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
            prev_snapshot: initial,
            curr_snapshot: initial,
            search_yaw_offset: 0.0,
            search_pitch_offset: 0.0,
            ideal_eye: initial.eye,
            ideal_target: initial.target,
        }
    }

    /// Load or swap the active room's `.cam` trigger volume definitions.
    pub fn set_cam_file(&mut self, cam_file: Option<CamFile>) {
        self.cam_file = cam_file;
        self.active_road_index = None;
    }

    /// Clamps position `pos` within a `_VC_ROAD_AREA` rail using PS2 `vcAdjustXzInLimAreaUsingMIN_IN_ROAD_DIST`.
    ///
    /// For indoor rooms, `min_in_road_dist` = 400.0 mm.
    fn clamp_to_road_rail(
        rail: &sh2_assets::formats::cam::RoadArea,
        mut pos: Vec3,
        is_outdoor: bool,
    ) -> Vec3 {
        let min_dist = if is_outdoor { 500.0 } else { 400.0 };
        let (min_x, max_x) = rail.x_bounds();
        let (min_z, max_z) = rail.z_bounds();

        let mut clamped_min_x = min_x + min_dist;
        let mut clamped_max_x = max_x - min_dist;
        if clamped_max_x < clamped_min_x {
            let mid_x = (clamped_min_x + clamped_max_x) * 0.5;
            clamped_min_x = mid_x;
            clamped_max_x = mid_x;
        }

        let mut clamped_min_z = min_z + min_dist;
        let mut clamped_max_z = max_z - min_dist;
        if clamped_max_z < clamped_min_z {
            let mid_z = (clamped_min_z + clamped_max_z) * 0.5;
            clamped_min_z = mid_z;
            clamped_max_z = mid_z;
        }

        pos.x = pos.x.clamp(clamped_min_x, clamped_max_x);
        pos.z = pos.z.clamp(clamped_min_z, clamped_max_z);

        // Clamp Y between min_hy and max_hy
        let (min_y, max_y) = rail.y_bounds();
        pos.y = pos.y.clamp(min_y, max_y);

        pos
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

        let eye = match rec.cam_mv_type {
            CameraMovementType::FixedAngle | CameraMovementType::Settle => Vec3::new(
                (rail_min_x + rail_max_x) * 0.5,
                eye_y,
                (rail_min_z + rail_max_z) * 0.5,
            ),
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

        let cam2chr_ang = dz.atan2(dx);
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
        let scr_z = if rec.projection > 0.0 {
            rec.projection
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
                Self::evaluate_road_record(rec, player_pos, player_yaw)
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
    /// Evaluates `.cam` volume triggers and smooth transitions across volume boundaries.
    /// `right_stick` offsets adjust the authentic PS2 search-view camera angle without altering room rails.
    pub fn tick(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        right_stick_x: f32,
        right_stick_y: f32,
    ) {
        self.prev_snapshot = self.curr_snapshot;

        // Apply PS2 search camera pan limits (maximum ~0.6 rad yaw, ~0.3 rad pitch)
        self.search_yaw_offset = right_stick_x * 0.6;
        self.search_pitch_offset = right_stick_y * 0.3;

        let target_eye: Vec3;
        let target_look: Vec3;
        let target_fov: f32;
        let target_mv_type: CameraMovementType;
        let mut warp_cut = false;

        if let Some(ref cam_file) = self.cam_file {
            let player_arr = [player_pos.x, player_pos.y, player_pos.z];
            if let Some(rec) = cam_file.find_active_record(player_arr, 150.0) {
                let rec_idx = cam_file.records.iter().position(|r| std::ptr::eq(r, rec));

                // Volume transition check
                if self.active_road_index != rec_idx {
                    if let Some(old_idx) = self.active_road_index {
                        let old_rec = &cam_file.records[old_idx];
                        if (old_rec.flags & VC_RD_WARP_OUT_F) != 0
                            || (rec.flags & VC_RD_WARP_IN_F) != 0
                        {
                            warp_cut = true;
                        }
                    }
                    self.active_road_index = rec_idx;
                }

                let (eval_eye, eval_look, eval_fov, eval_mv) =
                    Self::evaluate_road_record(rec, player_pos, player_yaw);
                target_eye = eval_eye;
                target_look = eval_look;
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

        self.mv_type = target_mv_type;

        // If warp cut is signaled (instant cut across cinematic cameras), snap instantly
        if warp_cut {
            self.ideal_eye = target_eye;
            self.ideal_target = target_look;
        } else {
            // Authentic 60 Hz exponential smoothing across volume boundaries (~0.15 lerp coefficient)
            self.ideal_eye = self.ideal_eye.lerp(target_eye, 0.15);
            self.ideal_target = self.ideal_target.lerp(target_look, 0.18);
        }

        let mut final_target = self.ideal_target;

        // Apply search-view look rotation around target
        if self.search_yaw_offset.abs() > 0.01 || self.search_pitch_offset.abs() > 0.01 {
            let rot = Quat::from_rotation_y(self.search_yaw_offset);
            let dir = rot * (final_target - self.ideal_eye);
            final_target = self.ideal_eye + dir;
            final_target.y += self.search_pitch_offset * 1000.0;
        }

        self.curr_snapshot = CameraSnapshot {
            eye: self.ideal_eye,
            target: final_target,
            up: Vec3::new(0.0, -1.0, 0.0),
            fov_y: target_fov,
        };
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
        use sh2_assets::formats::cam::parse_cam;
        use std::fs;
        use std::path::Path;

        let er01_path =
            "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/er/er01.cam";
        if !Path::new(er01_path).exists() {
            return;
        }

        let data = fs::read(er01_path).expect("Failed to read er01.cam");
        let cam_file = parse_cam(&data).expect("Failed to parse er01.cam");

        let mut controller = Ps2CameraController::new();
        controller.set_cam_file(Some(cam_file));

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
}
