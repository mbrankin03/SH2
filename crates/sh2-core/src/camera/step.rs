//! Authentic PS2 `vcExecCamera` (`0x00195f90`) road-record evaluation pipeline.

use glam::Vec3;
use sh2_assets::formats::cam::{RoadRecord, VC_RD_NO_FRONT_FLIP_F};

use super::{
    chase, exclusion, fixed_angle, kinematics, locus_door, self_view, CameraMovementType,
    Ps2CameraController, CHARA_CENTER_HEIGHT_MM, CHARA_FALLBACK_HEAD_HEIGHT_MM,
};

impl Ps2CameraController {
    /// Evaluates one step of the authentic PS2 `vcExecCamera` (`0x00195f90`) pipeline for `rec`.
    pub(super) fn step_road_record(
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
}
