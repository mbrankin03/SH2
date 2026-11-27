//! Authentic PS2 `vcExecCamera` (`0x00195f90`, `vc_main.c:415-441`) road-record evaluation pipeline.

use glam::{Mat4, Vec3};
use sh2_assets::formats::cam::{RoadRecord, VC_RD_LIM_UP_FAR_VIEW_F, VC_RD_NO_FRONT_FLIP_F};

use super::{
    chase, exclusion, fixed_angle, kinematics, locus_door, noise, self_view, CameraMovementType,
    Ps2CameraController, CHARA_FALLBACK_HEAD_HEIGHT_MM, NEAREST_ENEMY_MAX_DIST_MM,
    PS2_DEFAULT_PROJECTION_Z,
};

/// `SelfView` (Mode 3) per-tick horizontal target-move cap in PS2 world mm
/// (`SLUS_202.28` `vcAutoRenewalCamTgtPos` `0x00199f80`, `vc_main.c:2411`: `0x43fa0000` = `500.0`).
const SELF_VIEW_MAX_TGT_MV_XZ_LEN_MM: f32 = 500.0;

impl Ps2CameraController {
    /// Evaluates one step of the authentic PS2 `vcExecCamera` (`0x00195f90`, `vc_main.c:415-441`)
    /// pipeline for `rec`, returning `(eye, look_target, up, fov_y, active_mv_type, warp_cut)`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn step_road_record(
        &mut self,
        rec: &RoadRecord,
        player_pos: Vec3,
        player_yaw: f32,
        neck_ang_x: f32,
        neck_ang_y: f32,
        override_mv_type: Option<CameraMovementType>,
        through_door_ideal_eye: Option<Vec3>,
        road_changed: bool,
        mut warp: bool,
        dt: f32,
    ) -> (Vec3, Vec3, Vec3, f32, CameraMovementType, bool) {
        let active_mv_type = override_mv_type.unwrap_or(rec.cam_mv_type);
        let (chara_bottom_y, mut chara_top_y, chara_center_y) =
            chase::compute_chara_y_bounds(player_pos.y, self.crouch_rate);
        let is_outdoor = crate::widescreen_rail::is_outdoor_room_or_record(
            self.active_room_id.as_deref(),
            rec.area_size_type,
        );
        // `SLUS_202.28` `vcPreSetDataInVC_WORK` (`0x00196be0`, `vc_main.c:771`):
        // `(BgIsOut() == 0) && (*(int *)(_stage + 0x24) != 5)` is indoor.
        let ideal_cam_pos_h =
            chase::compute_ideal_cam_pos_h(chara_top_y, is_outdoor || self.stage_id == 5);

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
        let head_pos = Vec3::new(
            player_pos.x,
            player_pos.y - CHARA_FALLBACK_HEAD_HEIGHT_MM,
            player_pos.z,
        );
        self.raw_state.chara_pos = [player_pos.x, player_pos.y, player_pos.z, 1.0];
        self.raw_state.chara_bottom_y = chara_bottom_y;
        self.raw_state.chara_top_y = chara_top_y;
        self.raw_state.chara_center_y = chara_center_y;
        self.raw_state.chara_grnd_y = player_pos.y;
        self.raw_state.chara_head_pos = [head_pos.x, head_pos.y, head_pos.z, 1.0];
        self.raw_state.chara_mv_spd = chara_mv_spd;
        self.raw_state.chara_ang_spd_y = chara_ang_spd_y;
        self.raw_state.chara_eye_ang_y = player_yaw;
        self.raw_state.ideal_cam_pos_h = ideal_cam_pos_h;

        let cur_cam_pos = Vec3::new(
            self.raw_state.cam_pos[0],
            self.raw_state.cam_pos[1],
            self.raw_state.cam_pos[2],
        );
        let nearest_enemy_dist = self
            .nearest_enemy
            .map(|(d, _)| d)
            .unwrap_or(NEAREST_ENEMY_MAX_DIST_MM);

        // 1. Evaluate `vcRetFarWatchRate` (`0x00196700`, `vc_main.c:416`) &
        //    `vcRetSelfViewEffectRate` (`0x001968f0`, `vc_main.c:417`) FIRST.
        let far_watch = self_view::ret_far_watch_rate(
            self.search_button_pressed,
            active_mv_type,
            self.raw_state.flags,
            rec.flags,
            is_outdoor,
            self.through_door_session.rail_sta_to_chara_dist,
            Vec3::from_array(self.through_door_session.param.rail_sta_pos),
            player_pos,
            player_yaw,
        );
        if far_watch.trigger_fix_ang_warp
            && fixed_angle::vc_warp_for_fix_ang_cam(
                rec,
                player_yaw,
                &mut self.raw_state.fix_cam_mng,
            )
        {
            // `vcWarpForFixAngCam` (`0x00252980`) raises `0x1c` (`WARP_CAM | WARP_WATCH | WARP_CAM_TGT`).
            warp = true;
        }
        let sv_eff_rate = self_view::ret_self_view_effect_rate(
            active_mv_type,
            cur_cam_pos,
            head_pos,
            nearest_enemy_dist,
        );

        // 2. `vcSetFlagsByCamMvType` (`0x00196aa0`, `vc_main.c:418-423`).
        let chara_in_screen_check = far_watch.rate == 0.0
            && matches!(
                active_mv_type,
                CameraMovementType::Chase
                    | CameraMovementType::Settle
                    | CameraMovementType::LocusCircle
            );
        let warp_cam = warp || active_mv_type == CameraMovementType::SelfView;
        let warp_tgt = warp_cam || far_watch.rate != 0.0;
        let warp_watch = warp;

        // 3. `vcGetUseWatchAndCamMvParam` (`0x0019c4b0`, `vc_main.c:424`).
        let (watch_prm, cam_prm) = kinematics::vc_get_use_watch_and_cam_mv_param(
            sv_eff_rate,
            far_watch.rate,
            is_outdoor,
            self.stage_id,
            self.on_boat,
            chara_mv_spd,
            None,
            None,
        );

        // 4. Ideal camera position (`vcAutoRenewalCamTgtPos` `0x00199f80`, `vc_main.c:425-427`).
        let mut locus_proj_override: Option<f32> = None;
        let ideal_pos = match active_mv_type {
            CameraMovementType::LocusCircle => {
                let (locus_eye, locus_proj) =
                    locus_door::evaluate_locus_circle_record(rec, player_pos);
                if (locus_proj - rec.projection).abs() > f32::EPSILON {
                    locus_proj_override = Some(locus_proj);
                }
                locus_eye
            }
            CameraMovementType::SelfView => self_view::make_ideal_cam_pos_by_head_pos(
                if warp_watch {
                    self_view::VC_FLAG_WARP_WATCH
                } else {
                    0
                },
                player_pos,
                chara_center_y,
                head_pos,
                player_yaw,
            ),
            CameraMovementType::ThroughDoor => through_door_ideal_eye
                .unwrap_or_else(|| locus_door::evaluate_through_door_record(rec, player_yaw)),
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

        // 5. Camera target renewal (`vcAutoRenewalCamTgtPos` `0x00199f80`).
        let mut cam_tgt_pos = if warp_tgt {
            self.raw_state.old_cam_excl_area_r = -1.0;
            ideal_pos
        } else {
            Vec3::new(
                self.raw_state.cam_tgt_pos[0],
                self.raw_state.cam_tgt_pos[1],
                self.raw_state.cam_tgt_pos[2],
            )
        };

        let mv_vec = if active_mv_type == CameraMovementType::SelfView {
            chase::vc_make_basic_cam_tgt_mv_vec(
                SELF_VIEW_MAX_TGT_MV_XZ_LEN_MM,
                ideal_pos,
                cam_tgt_pos,
                dt,
                warp_tgt,
            )
        } else {
            let max_xz = chase::vc_ret_max_tgt_mv_xz_len(
                chara_mv_spd,
                chara_ang_spd_y,
                cam_prm.max_spd_xz,
                is_outdoor || self.stage_id == 5,
                self.on_boat,
                dt,
            );
            let mut v =
                chase::vc_make_basic_cam_tgt_mv_vec(max_xz, ideal_pos, cam_tgt_pos, dt, warp_tgt);
            if matches!(
                active_mv_type,
                CameraMovementType::Chase | CameraMovementType::LocusCircle
            ) && (rec.flags & VC_RD_NO_FRONT_FLIP_F) == 0
            {
                exclusion::vc_cam_tgt_mv_vec_is_fliped_from_chara_front(
                    max_xz,
                    &mut v,
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
            if active_mv_type != CameraMovementType::ThroughDoor {
                chase::vc_adj_tgt_mv_vec_y_by_cur_near_road(
                    &mut v,
                    rec,
                    cam_tgt_pos,
                    player_pos,
                    chara_top_y,
                    ideal_cam_pos_h,
                    is_outdoor || self.stage_id == 5,
                );
            }
            v
        };
        self.raw_state.cam_tgt_mv_ang_y = mv_vec.x.atan2(mv_vec.z);
        if dt <= 0.0 && !warp_tgt {
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

        // 6. Velocity-bounded eye integration (`vcRenewalCamData` `0x0019c840`, `vc_main.c:428`).
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
            warp_cam,
            dt,
        );
        self.raw_state.cam_pos = [cam_pos.x, cam_pos.y, cam_pos.z, 1.0];
        self.raw_state.cam_velo = [cam_velo.x, cam_velo.y, cam_velo.z, 0.0];
        self.raw_state.cam_mv_ang[0] = cam_mv_ang_y;

        // 7. Projection focal-distance ramp (`vcChangeProjectionValue` `0x0019dc50`, `vc_main.c:429-431`).
        let effective_projection = if let Some(locus_proj) = locus_proj_override {
            self.projection_ramp.current_scr_z = locus_proj;
            self.projection_ramp.mode = 0;
            locus_proj
        } else {
            self.projection_ramp.step()
        };
        let (scr_half_wx, scr_half_wy) =
            kinematics::compute_screen_half_angles(effective_projection);
        self.raw_state.scr_half_ang_wx = scr_half_wx;
        self.raw_state.scr_half_ang_wy = scr_half_wy;

        // 8. Watch target & roll renewal (`vcAutoRenewalWatchTgtPosAndAngZ` `0x00198ba0`, `vc_main.c:432-437`)
        //    BEFORE `vcRenewalCamMatAng`.
        let nearest_item_pair = self
            .nearest_item
            .center_y
            .map(|cy| (self.nearest_item.dist_mm, cy));
        let mut watch_tgt = if active_mv_type == CameraMovementType::SelfView {
            self.raw_state.watch_tgt_ang_z = 0.0;
            self_view::make_far_watch_tgt_pos(&self_view::FarWatchInput {
                stage_id: self.stage_id,
                area_size_type: rec.area_size_type,
                on_boat: self.on_boat,
                cam_pos,
                chara_pos: player_pos,
                chara_yaw: player_yaw,
                chara_center_y,
                neck_ang_x,
                neck_ang_y,
                nearest_enemy: self.nearest_enemy,
                nearest_item: nearest_item_pair,
            })
        } else {
            // `vcMakeNormalWatchTgtPos` (`0x00198d80`, `vc_main.c:1939`) resets `watch_tgt_ang_z = 0.0`.
            self.raw_state.watch_tgt_ang_z = 0.0;
            let mut normal_tgt = if active_mv_type == CameraMovementType::FixedAngle {
                fixed_angle::vc_make_fix_ang_watch_tgt_pos(
                    rec,
                    cam_pos,
                    &self.raw_state.fix_cam_mng,
                )
            } else {
                chase::vc_make_normal_watch_tgt_pos(
                    rec,
                    cam_pos,
                    player_pos,
                    chara_bottom_y,
                    player_yaw,
                    self.stage_id,
                    self.on_boat,
                )
            };
            if far_watch.rate != 0.0 {
                let far_tgt = self_view::make_far_watch_tgt_pos(&self_view::FarWatchInput {
                    stage_id: self.stage_id,
                    area_size_type: rec.area_size_type,
                    on_boat: self.on_boat,
                    cam_pos,
                    chara_pos: player_pos,
                    chara_yaw: player_yaw,
                    chara_center_y,
                    neck_ang_x,
                    neck_ang_y,
                    nearest_enemy: self.nearest_enemy,
                    nearest_item: nearest_item_pair,
                });
                normal_tgt = normal_tgt.lerp(far_tgt, far_watch.rate);
            }
            normal_tgt
        };

        if sv_eff_rate > 0.0 {
            let (mixed_tgt, mixed_ang_z) = self_view::mix_self_view_effect_to_watch_tgt_pos(
                sv_eff_rate,
                watch_tgt,
                self.raw_state.watch_tgt_ang_z,
                cam_pos,
                player_yaw,
                [neck_ang_x, player_yaw + neck_ang_y, 0.0],
                0,
                self.nearest_enemy.is_some(),
            );
            watch_tgt = mixed_tgt;
            self.raw_state.watch_tgt_ang_z = mixed_ang_z;
        }

        if watch_tgt.y > self.raw_state.watch_tgt_max_y {
            watch_tgt.y = self.raw_state.watch_tgt_max_y;
        }
        if (rec.flags & VC_RD_LIM_UP_FAR_VIEW_F) != 0
            && (rec.cam_mv_type == CameraMovementType::Chase
                || active_mv_type == CameraMovementType::SelfView)
        {
            watch_tgt = self_view::adjust_watch_y_limit_high_when_far_view(
                watch_tgt,
                cam_pos,
                effective_projection,
                PS2_DEFAULT_PROJECTION_Z,
                self.stage_id,
            );
        }
        self.raw_state.watch_tgt_pos = [watch_tgt.x, watch_tgt.y, watch_tgt.z, 1.0];

        // 9. Base & offset angular renewal (`vcRenewalCamMatAng` `0x0019c9d0`, `vc_main.c:438`).
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
        let new_base_ang = if active_mv_type == CameraMovementType::Settle {
            chase::vc_make_new_base_cam_ang(
                rec,
                cam_pos,
                watch_tgt,
                base_ang,
                chara_mv_spd,
                false,
                warp_watch,
                dt,
            )
        } else {
            Vec3::ZERO
        };
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

        // Character-in-screen framing (`vcSetFlagsByCamMvType` `0x00196aa0`: enabled only when
        // `far_watch_rate == 0.0` and `active_mv_type` is `Chase | Settle | LocusCircle`).
        if chara_in_screen_check {
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
        if warp_watch {
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
        if active_mv_type == rec.cam_mv_type {
            kinematics::vc_correct_cam_mat_ang_forcibly(&mut ofs_ang, rec);
        }
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

        // 10. `vcSetDataToVwSystem` (`0x0019d650`, `vc_main.c:439`, `vc_main.c:3962-3999`):
        //     preserve `watch_tgt_ang_z` roll from `cam_mat` and multiply the 6-element
        //     Mode 3 (`SelfView`) handheld noise matrix onto `cam_mat`.
        let mut vw_mat = Mat4::from_mat3(cam_rot_mat);
        if active_mv_type == CameraMovementType::SelfView {
            self.self_view_timer_sec += dt;
            vw_mat *= noise::build_self_view_noise_matrix(self.self_view_timer_sec);
        }
        let fwd = vw_mat.z_axis.truncate().normalize_or(Vec3::Z);
        let up = (-vw_mat.y_axis.truncate()).normalize_or(Vec3::new(0.0, -1.0, 0.0));
        let watch_dist = (watch_tgt - cam_pos).length().max(500.0);
        let look_target = cam_pos + fwd * watch_dist;
        let fov_y = kinematics::fov_y_from_ps2_projection_unclamped(effective_projection);

        (cam_pos, look_target, up, fov_y, active_mv_type, warp)
    }
}
