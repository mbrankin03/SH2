//! Camera reset (`vcSetFirstCamWork` `0x00195ad0`) and neutral fallback placement.

use crate::types::camera::FixCamManager;
use glam::Vec3;
use std::f32::consts::PI;

use super::{chase, kinematics, CameraMovementType, CameraSnapshot, Ps2CameraController};

impl Ps2CameraController {
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

        let (eye, target, up, fov_y, mv_type) = if let Some((idx, rec)) = active_rec {
            self.active_road_index = idx;
            self.projection_ramp.schedule(rec.projection, 0);
            let (e, t, u, f, mv, _) = self.step_road_record(
                &rec, player_pos, player_yaw, 0.0, 0.0, None, None, true, true, 0.0,
            );
            (e, t, u, f, mv)
        } else {
            self.active_road_index = None;
            self.projection_ramp
                .schedule(kinematics::PS2_DEFAULT_PROJECTION_SCR_Z, 0);
            let (fb_eye, fb_tgt, fb_fov, fb_mv) = Self::fallback_placement(player_pos, player_yaw);
            self.raw_state.cam_pos = [fb_eye.x, fb_eye.y, fb_eye.z, 1.0];
            self.raw_state.cam_tgt_pos = [fb_eye.x, fb_eye.y, fb_eye.z, 1.0];
            let ang = kinematics::vw_vector_to_angle(fb_tgt - fb_eye);
            self.raw_state.ofs_cam_ang = [ang.x, ang.y, ang.z, 0.0];
            self.raw_state.cam_mat_ang = [ang.x, ang.y, ang.z, 0.0];
            (fb_eye, fb_tgt, Vec3::new(0.0, -1.0, 0.0), fb_fov, fb_mv)
        };

        let snap = CameraSnapshot {
            eye,
            target,
            up,
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
    pub(super) fn fallback_placement(
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
}
