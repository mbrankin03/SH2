//! 60 Hz camera simulation tick (`vcExecCamera` `0x00195f90`), road selection, and search/far-watch blending.

use glam::Vec3;
use sh2_assets::formats::cam::RoadRecord;

use super::{
    kinematics, road_priority, CameraMovementType, CameraSnapshot, Ps2CameraController, SIM_DT_SECS,
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

        let mut warp_cut = false;
        let mut road_changed = false;
        let has_nearest_enemy = self.nearest_enemy.is_some();

        // Advance active doorway transition (`vcSetTHROUGH_DOOR_CAM_PARAM_in_VC_WORK` `0x00196e00`,
        // `vcRetCurCamMvType` `0x00196540`): when active, `mv_type` becomes `ThroughDoor` (5)
        // and routes through `vcRenewalCamData` + `vcRenewalCamMatAng`.
        let td_ideal_eye = self.through_door_session.step(
            SIM_DT_SECS,
            player_pos,
            player_yaw,
            self.nearest_enemy.map(|(d, _)| d),
        );

        // Select best candidate road using `vcSetNearRoadAryByCharaPos` (`0x00197310`, oriented
        // `lim_sw`, `0x40`/`0x20` enemy flags, `0x80` merge flag, Y-excess), `vcRetRoadUsePriority`
        // (`0x00197a10`), and `vcSetCurNearRoadInVC_WORK` hysteresis (`0x00197af0`).
        let selected_road = self.cam_file.as_ref().and_then(|cam_file| {
            let mut best: Option<road_priority::RoadSelectionCandidate<'_>> = None;
            let mut old_cand: Option<road_priority::RoadSelectionCandidate<'_>> = None;

            for (idx, rec) in cam_file.records.iter().enumerate() {
                if road_priority::is_road_excluded_by_enemy_flags(rec.flags, has_nearest_enemy) {
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
            let decision =
                road_priority::resolve_cur_near_road_hysteresis(old_cand, best_cand, player_yaw);
            let old_flags = self
                .active_road_index
                .and_then(|old_idx| cam_file.records.get(old_idx))
                .map(|r| r.flags);
            let rec = cam_file.records[decision.selected_index].clone();
            Some((decision, old_flags, rec))
        });

        let (eye, look, up, target_fov, target_mv_type) = if let Some((decision, old_flags, rec)) =
            selected_road
        {
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
                // `SLUS_202.28` `vcSetCurNearRoadInVC_WORK` (`0x00197af0`, `vc_main.c:1373-1380`):
                // schedule projection focal-distance ramp on road switch.
                let proj_frames = if warp_cut || rec.proj_sec <= 0.0 {
                    0
                } else {
                    (rec.proj_sec * (1.0 / SIM_DT_SECS))
                        .trunc()
                        .clamp(0.0, u16::MAX as f32) as u16
                };
                self.projection_ramp.schedule(rec.projection, proj_frames);
                self.active_road_index = Some(decision.selected_index);
            }

            let override_mv = td_ideal_eye.map(|_| CameraMovementType::ThroughDoor);
            let (e, l, u, f, mv, did_warp) = self.step_road_record(
                &rec,
                player_pos,
                player_yaw,
                neck_ang_x,
                neck_ang_y,
                override_mv,
                td_ideal_eye,
                road_changed,
                warp_cut,
                SIM_DT_SECS,
            );
            warp_cut = did_warp;
            (e, l, u, f, mv)
        } else if let Some(td_eye) = td_ideal_eye {
            // Active `ThroughDoorSession` without a loaded `.cam` file: still route through
            // `vcRenewalCamData` + `vcRenewalCamMatAng` using a default `RoadRecord` (`mv_type = 5`).
            let default_rec = RoadRecord {
                cam_mv_type: CameraMovementType::ThroughDoor,
                ..RoadRecord::default()
            };
            let (e, l, u, f, mv, did_warp) = self.step_road_record(
                &default_rec,
                player_pos,
                player_yaw,
                neck_ang_x,
                neck_ang_y,
                Some(CameraMovementType::ThroughDoor),
                Some(td_eye),
                false,
                false,
                SIM_DT_SECS,
            );
            warp_cut = did_warp;
            (e, l, u, f, mv)
        } else {
            let (fb_eye, fb_look, fb_fov, fb_mv) = Self::fallback_placement(player_pos, player_yaw);
            let (watch_prm, cam_prm) = kinematics::vc_get_use_watch_and_cam_mv_param(
                0.0,
                0.0,
                false,
                self.stage_id,
                self.on_boat,
                0.0,
                None,
                None,
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
                Vec3::new(0.0, -1.0, 0.0),
                fb_fov,
                fb_mv,
            )
        };

        // `vcRetSmoothCamMvF` (`0x00196360`, `vc_main.c:441`) runs LAST after `vcSetDataToVwSystem`.
        let cur_ang = Vec3::new(
            self.raw_state.cam_mat_ang[0],
            self.raw_state.cam_mat_ang[1],
            self.raw_state.cam_mat_ang[2],
        );
        self.last_move_smooth =
            kinematics::vc_ret_smooth_cam_mv_f(prev_pos, eye, prev_ang, cur_ang, 1.0);

        self.mv_type = target_mv_type;
        self.ideal_eye = eye;
        self.ideal_target = look;

        self.curr_snapshot = CameraSnapshot {
            eye: self.ideal_eye,
            target: self.ideal_target,
            up,
            fov_y: target_fov,
        };

        // On an instant camera cut (`VC_RD_WARP_IN_F` / `VC_RD_WARP_OUT_F` / `vcWarpForFixAngCam`),
        // snap `prev_snapshot` to `curr_snapshot` so 120–240 Hz sub-frame interpolation does not
        // smear between the old camera volume and the new cut angle (`SH2-50`).
        if warp_cut {
            self.prev_snapshot = self.curr_snapshot;
        }
    }
}
