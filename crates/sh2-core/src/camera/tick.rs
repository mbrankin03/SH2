//! 60 Hz camera simulation tick (`vcExecCamera` `0x00195f90`), road selection, and search/far-watch blending.

use glam::Vec3;
use sh2_assets::formats::cam::{AreaSizeType, VC_RD_LIM_UP_FAR_VIEW_F};

use super::{
    kinematics, noise, road_priority, self_view, CameraMovementType, CameraSnapshot,
    Ps2CameraController, CHARA_CENTER_HEIGHT_MM, CHARA_FALLBACK_HEAD_HEIGHT_MM,
    DEFAULT_WATCH_TARGET_Y_OFFSET_MM, NEAREST_ENEMY_MAX_DIST_MM, PS2_DEFAULT_PROJECTION_Z,
    SIM_DT_SECS,
};

impl Ps2CameraController {
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
}
