//! Authentic PS2 `LOCUS_CIRCLE` (Mode 4) and `THROUGH_DOOR` (Mode 5) camera motion algorithms.
//!
//! Ground truth:
//! - PS2 `SLUS_202.28`: `vcMakeIdealCamPosForLocusCircleCam` (`0x0019a9c0`),
//!   `vcRetNearRatioSwitchAreaForCircleCam` (`0x001a69d0`), `vcAdjTgtMvVecYByCurNearRoad` (`0x0019b990`),
//!   `vcChangeProjByDist` (`0x00252cf0`), `vcMakeIdealCamPosForThroughDoorCam` (`0x0019a820`),
//!   `vcSetTHROUGH_DOOR_CAM_PARAM_in_VC_WORK` (`0x00196e00`), `vcRetThroughDoorCamEndF` (`0x001965b0`).
//! - Retail PC `sh2pc.exe`: `0x0051b1e0`, `0x0051b0c0`, `0x0051a1f0`, `0x00519dd0`.

use glam::Vec3;
use sh2_assets::formats::cam::{
    AreaSizeType, CamMovementParams, LocusCircleCamParam, RoadArea, RoadRecord, ThroughDoorCamParam,
};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// Initial doorway pullback offset along `rail_ang_y` in PS2 mm (`SLUS_202.28` `0x0019a868`; `sh2pc.exe` `0x0051b171` uses `-300.0`).
pub const THROUGH_DOOR_INIT_ALONG_OFS_MM: f32 = -650.0;
/// Base doorway longitudinal offset along `rail_ang_y` in PS2 mm (`SLUS_202.28` `0x0019a88c`).
pub const THROUGH_DOOR_ALONG_BASE_MM: f32 = 375.0;
/// Cosine amplitude along `rail_ang_y` in PS2 mm (`SLUS_202.28` `0x0019a898`; `sh2pc.exe` `0x0051b141` uses `475.0`).
pub const THROUGH_DOOR_ALONG_AMP_MM: f32 = 350.0;
/// Lateral sine amplitude perpendicular to `rail_ang_y` in PS2 mm (`SLUS_202.28` `0x0019a8a0`; `sh2pc.exe` `0x0051b162` uses `425.0`).
pub const THROUGH_DOOR_SIDE_AMP_MM: f32 = 300.0;
/// Elapsed seconds after which a nearby enemy aborts `ThroughDoor` (`SLUS_202.28` `0x001965e4`).
pub const THROUGH_DOOR_ENEMY_ABORT_SEC: f32 = 1.2;
/// Horizontal enemy distance in PS2 mm triggering early `ThroughDoor` exit (`SLUS_202.28` `0x00196610`).
pub const THROUGH_DOOR_ENEMY_ABORT_DIST_MM: f32 = 600.0;
/// Horizontal walkout distance in PS2 mm ending `ThroughDoor` (`SLUS_202.28` `0x00196638`).
pub const THROUGH_DOOR_MAX_CHARA_DIST_MM: f32 = 800.0;
/// Minimum walkout distance in PS2 mm before turn-back exit applies (`SLUS_202.28` `0x00196660`).
pub const THROUGH_DOOR_TURNBACK_MIN_DIST_MM: f32 = 250.0;
/// Maximum character-vs-door angle (70 deg in radians) before `ThroughDoor` exits (`SLUS_202.28` `0x001966b0`).
pub const THROUGH_DOOR_TURNBACK_MAX_ANG_RAD: f32 = 1.221_730_5;

/// Wraps an angle in radians into `[-PI, PI)` matching PS2 `shAngleRegulate` (`SLUS_202.28` `0x0016e970`).
#[inline]
pub fn regulate_angle(rad: f32) -> f32 {
    if !rad.is_finite() {
        0.0
    } else if (-PI..PI).contains(&rad) {
        rad
    } else {
        (rad + PI).rem_euclid(TAU) - PI
    }
}

/// Computes `_VC_CIR_CAM_MANAGER` quadrant corner `[origin_x, origin_z]` and `lim_sw` span `[sw_dx, sw_dz]` in PS2 mm (`0x0019aa30`).
#[inline]
pub fn compute_cir_cam_manager(lim_sw: &RoadArea, reg_ang_y: f32) -> ([f32; 2], [f32; 2]) {
    let ((min_x, max_x), (min_z, max_z)) = (lim_sw.x_bounds(), lim_sw.z_bounds());
    let corner = if (-PI..-FRAC_PI_2).contains(&reg_ang_y) {
        [max_x, max_z]
    } else if (-FRAC_PI_2..0.0).contains(&reg_ang_y) {
        [max_x, min_z]
    } else if (0.0..FRAC_PI_2).contains(&reg_ang_y) {
        [min_x, min_z]
    } else {
        [min_x, max_z]
    };
    (corner, [(max_x - min_x).abs(), (max_z - min_z).abs()])
}

#[inline]
fn locus_arc_xz(ox: f32, oz: f32, sw: [f32; 2], reg_ang_y: f32, d: f32, r: f32) -> (f32, f32) {
    let (sx, sz, x_leads) = if (-PI..-FRAC_PI_2).contains(&reg_ang_y) {
        (-1.0, -1.0, true)
    } else if (-FRAC_PI_2..0.0).contains(&reg_ang_y) {
        (-1.0, 1.0, false)
    } else if reg_ang_y > 0.0 && reg_ang_y < FRAC_PI_2 {
        (1.0, 1.0, true)
    } else {
        (1.0, -1.0, false)
    };
    if d == 0.0 {
        return (ox + sx * sw[0], oz + sz * sw[1]);
    }
    let (sin_abs, cos_d) = (if d > 0.0 { d.sin() } else { -d.sin() }, d.cos());
    if (d > 0.0) == x_leads {
        (ox + sx * (sw[0] + r * sin_abs), oz + sz * sw[1] * cos_d)
    } else {
        (ox + sx * sw[0] * cos_d, oz + sz * (sw[1] + r * sin_abs))
    }
}

/// Evaluates ideal camera eye position (PS2 mm, Y-down) and focal projection for `LOCUS_CIRCLE` (Mode 4).
///
/// Ported from `vcMakeIdealCamPosForLocusCircleCam` (`0x0019a9c0`), `vcRetNearRatioSwitchAreaForCircleCam`
/// (`0x001a69d0`), `vcAdjTgtMvVecYByCurNearRoad` (`0x0019b990`), and `vcChangeProjByDist` (`0x00252cf0`).
pub fn evaluate_locus_circle_eye(
    rec: &RoadRecord,
    cir: &LocusCircleCamParam,
    player_pos: Vec3,
) -> (Vec3, f32) {
    let reg_ang_y = regulate_angle(cir.ang_y);
    let (cm_origin, [sw_dx, sw_dz]) = compute_cir_cam_manager(&rec.lim_sw, reg_ang_y);
    let (ox, oz) = if cir.origin_x.abs() > f32::EPSILON && cir.origin_z.abs() > f32::EPSILON {
        (cir.origin_x, cir.origin_z)
    } else {
        (cm_origin[0], cm_origin[1])
    };

    let d = regulate_angle((player_pos.x - ox).atan2(player_pos.z - oz) - reg_ang_y) * 2.0;
    let (eye_x, eye_z) = locus_arc_xz(ox, oz, [sw_dx, sw_dz], reg_ang_y, d, cir.radius);

    let is_outdoor = rec.area_size_type == AreaSizeType::Outdoor;
    let ideal_cam_pos_h = (player_pos.y - 937.5) + if is_outdoor { -450.0 } else { -200.0 };
    let sw_ang = regulate_angle((player_pos.x - cm_origin[0]).atan2(player_pos.z - cm_origin[1]));
    let use_x = (-3.0 * FRAC_PI_4..-FRAC_PI_4).contains(&sw_ang)
        || (FRAC_PI_4..3.0 * FRAC_PI_4).contains(&sw_ang);
    let (span, dist) = if use_x {
        (sw_dx, (cm_origin[0] - player_pos.x).abs())
    } else {
        (sw_dz, (cm_origin[1] - player_pos.z).abs())
    };
    let raw = if span > f32::EPSILON {
        dist / span
    } else {
        0.0
    };
    let ratio = (if (-PI..PI).contains(&cir.ang_y) {
        raw
    } else {
        1.0 - raw
    })
    .clamp(0.0, 1.0);

    let extra_y = if is_outdoor { -250.0 } else { 0.0 };
    let eye_y = match rec.mv_y_type {
        1 => rec.lim_rd.min_hy * (1.0 - ratio) + rec.lim_rd.max_hy * ratio,
        2 => rec.lim_rd.max_hy * (1.0 - ratio) + rec.lim_rd.min_hy * ratio,
        4 => (rec.lim_rd.min_hy + player_pos.y + extra_y) * (1.0 - ratio) + ideal_cam_pos_h * ratio,
        5 => (rec.lim_rd.max_hy + player_pos.y + extra_y) * (1.0 - ratio) + ideal_cam_pos_h * ratio,
        _ => ideal_cam_pos_h.clamp(rec.lim_rd.y_bounds().0, rec.lim_rd.y_bounds().1),
    };
    let proj = if is_outdoor && (rec.mv_y_type == 4 || rec.mv_y_type == 5) {
        448.0 + 1.2 * (-1375.0 - (eye_y - player_pos.y)).abs()
    } else {
        rec.projection
    };
    (Vec3::new(eye_x, eye_y, eye_z), proj)
}

/// Evaluates a `RoadRecord` configured as `CamMovementType::LocusCircle`, returning `(eye, projection)` in PS2 mm (`0x0019a9c0`).
pub fn evaluate_locus_circle_record(rec: &RoadRecord, player_pos: Vec3) -> (Vec3, f32) {
    let CamMovementParams::LocusCircle(cir) = rec.params else {
        return evaluate_locus_circle_eye(rec, &LocusCircleCamParam::default(), player_pos);
    };
    evaluate_locus_circle_eye(rec, &cir, player_pos)
}

/// Evaluates ideal camera eye position (PS2 mm, Y-down) for `THROUGH_DOOR` (Mode 5, `SLUS_202.28` `0x0019a820`).
pub fn evaluate_through_door_eye(
    prm: &ThroughDoorCamParam,
    lim_rd: &RoadArea,
    player_yaw: f32,
    init_pullback: bool,
) -> Vec3 {
    let sta = if prm.rail_sta_pos == [0.0, 0.0, 0.0] {
        Vec3::from_array(lim_rd.center())
    } else {
        Vec3::from_array(prm.rail_sta_pos)
    };
    let (along_ofs, side_ofs) = if init_pullback {
        (THROUGH_DOOR_INIT_ALONG_OFS_MM, 0.0)
    } else {
        let delta_yaw = regulate_angle(player_yaw - prm.rail_ang_y);
        let phase = (PI * (delta_yaw.abs() - FRAC_PI_4).max(0.0)) / (PI - FRAC_PI_4);
        (
            THROUGH_DOOR_ALONG_BASE_MM - THROUGH_DOOR_ALONG_AMP_MM * phase.cos(),
            -THROUGH_DOOR_SIDE_AMP_MM * delta_yaw.sin(),
        )
    };
    let (sin_r, cos_r) = prm.rail_ang_y.sin_cos();
    Vec3::new(
        sta.x + along_ofs * sin_r + side_ofs * cos_r,
        sta.y,
        sta.z + along_ofs * cos_r - side_ofs * sin_r,
    )
}

/// Evaluates a `RoadRecord` configured as `CamMovementType::ThroughDoor`, returning ideal eye in PS2 mm (`0x0019a820`).
pub fn evaluate_through_door_record(rec: &RoadRecord, player_yaw: f32) -> Vec3 {
    let CamMovementParams::ThroughDoor(td) = rec.params else {
        return evaluate_through_door_eye(
            &ThroughDoorCamParam::default(),
            &rec.lim_rd,
            player_yaw,
            false,
        );
    };
    evaluate_through_door_eye(&td, &rec.lim_rd, player_yaw, false)
}

/// Lifecycle phase of `_VC_THROUGH_DOOR_CAM_PARAM` (`SLUS_202.28` `0x00196e00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThroughDoorPhase {
    /// Door transit camera inactive (`active_f == 0`).
    #[default]
    Inactive,
    /// First-tick doorway pullback (`through_door_activate_init_f != 0`).
    InitialPullback,
    /// Active doorway rail tracking (`active_f == 1`, `through_door_activate_init_f == 0`).
    ActiveTransit,
}

/// Stateful `_VC_THROUGH_DOOR_CAM_PARAM` runtime controller (`SLUS_202.28` `0x00196e00`, `0x001965b0`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ThroughDoorSession {
    /// Current state-machine phase.
    pub phase: ThroughDoorPhase,
    /// Elapsed transit duration in seconds (`timer`).
    pub timer_sec: f32,
    /// Doorway rail parameters (`rail_ang_y` in radians, `rail_sta_pos` in PS2 mm).
    pub param: ThroughDoorCamParam,
    /// Horizontal XZ distance from `rail_sta_pos` to player in PS2 mm (`rail_sta_to_chara_dist`).
    pub rail_sta_to_chara_dist: f32,
}

impl ThroughDoorSession {
    /// Starts a doorway camera transition at `player_pos` (PS2 mm, Y-down) and `player_yaw` (radians, `0x00196e3c`).
    pub fn activate(&mut self, player_pos: Vec3, player_yaw: f32, is_outdoor: bool) {
        let ideal_h = (player_pos.y - 937.5) + if is_outdoor { -450.0 } else { -200.0 };
        let param = ThroughDoorCamParam {
            rail_ang_y: player_yaw,
            rail_sta_pos: [player_pos.x, ideal_h, player_pos.z],
        };
        *self = Self {
            phase: ThroughDoorPhase::InitialPullback,
            timer_sec: 0.0,
            param,
            rail_sta_to_chara_dist: 0.0,
        };
    }

    /// Terminates an active doorway transition (`vcSetTHROUGH_DOOR_CAM_PARAM_in_VC_WORK` mode `1`, `0x00196e6c`).
    pub fn deactivate(&mut self) {
        self.phase = ThroughDoorPhase::Inactive;
        self.timer_sec = 0.0;
    }

    /// Checks whether `ThroughDoor` should hand off to room `.cam` rails (`vcRetThroughDoorCamEndF`, `0x001965b0`).
    pub fn is_ended(&self, player_pos: Vec3, player_yaw: f32, enemy_xz_dist: Option<f32>) -> bool {
        if self.phase == ThroughDoorPhase::Inactive {
            return true;
        }
        if self.timer_sec > THROUGH_DOOR_ENEMY_ABORT_SEC
            && enemy_xz_dist.is_some_and(|d| d < THROUGH_DOOR_ENEMY_ABORT_DIST_MM)
        {
            return true;
        }
        if self.rail_sta_to_chara_dist > THROUGH_DOOR_MAX_CHARA_DIST_MM {
            return true;
        }
        if self.rail_sta_to_chara_dist > THROUGH_DOOR_TURNBACK_MIN_DIST_MM {
            let sta = self.param.rail_sta_pos;
            let ang_to_chara = (player_pos.x - sta[0]).atan2(player_pos.z - sta[2]);
            return regulate_angle(player_yaw - ang_to_chara).abs()
                > THROUGH_DOOR_TURNBACK_MAX_ANG_RAD;
        }
        false
    }

    /// Advances timer/rail height (`dt_sec` seconds) and returns ideal eye (PS2 mm) while active (`0x00196e7c`).
    pub fn step(
        &mut self,
        dt_sec: f32,
        player_pos: Vec3,
        player_yaw: f32,
        enemy_xz_dist: Option<f32>,
    ) -> Option<Vec3> {
        let init_pullback = match self.phase {
            ThroughDoorPhase::Inactive => return None,
            ThroughDoorPhase::InitialPullback => true,
            ThroughDoorPhase::ActiveTransit => false,
        };
        let sta = self.param.rail_sta_pos;
        self.rail_sta_to_chara_dist = (player_pos.x - sta[0]).hypot(player_pos.z - sta[2]);
        self.timer_sec += dt_sec;
        if self.timer_sec <= 0.75 {
            self.param.rail_sta_pos[1] += -250.0 * dt_sec;
        }
        if self.is_ended(player_pos, player_yaw, enemy_xz_dist) {
            self.deactivate();
            return None;
        }
        let eye =
            evaluate_through_door_eye(&self.param, &RoadArea::default(), player_yaw, init_pullback);
        self.phase = ThroughDoorPhase::ActiveTransit;
        Some(eye)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraMovementType, Ps2CameraController};
    use sh2_assets::formats::cam::{parse_cam, CamFile, CamMovementType};

    fn cir(ox: f32, oz: f32, ang_y: f32, radius: f32) -> LocusCircleCamParam {
        let mut p = LocusCircleCamParam::default();
        (p.origin_x, p.origin_z, p.ang_y, p.radius) = (ox, oz, ang_y, radius);
        p
    }

    #[test]
    fn test_locus_circle_quadrants_continuity_and_zero_origin() {
        let mut buf = [0u8; 256];
        for (off, val) in [
            (0x00, 2250.0f32),
            (0x04, 57750.0),
            (0x08, 2250.0),
            (0x0C, 54500.0),
            (0x10, 5500.0),
            (0x14, 54500.0),
            (0x18, 500.0),
            (0x1C, -4000.0),
            (0x38, -865.0),
            (0x3C, -1385.0),
            (0x78, -FRAC_PI_4),
            (0x7C, 1250.0),
        ] {
            buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
        }
        buf[0x48..0x4C].copy_from_slice(&3i32.to_le_bytes());
        buf[0x50..0x54].copy_from_slice(&5i32.to_le_bytes());
        buf[0x6C..0x70].copy_from_slice(&4i32.to_le_bytes());
        buf[0x80 + 0x44] = 0x01;
        let mut rec = parse_cam(&buf)
            .expect("valid synthetic cam")
            .records
            .remove(0);
        let (eye0, proj0) = evaluate_locus_circle_record(&rec, Vec3::new(3875.0, 0.0, 56125.0));
        assert!((eye0.x - 2250.0).abs() < 1.0 && (eye0.z - 57750.0).abs() < 1.0 && proj0 > 448.0);

        rec.params = CamMovementParams::LocusCircle(cir(5500.0, 54500.0, -3.9269907, 1000.0));
        assert!(evaluate_locus_circle_record(&rec, Vec3::ZERO).0.is_finite());
        let q1 = cir(5500.0, 57750.0, -3.0 * FRAC_PI_4, 1000.0);
        let (e_edge, _) = evaluate_locus_circle_eye(&rec, &q1, Vec3::new(5500.0, 0.0, 54500.0));
        let (e_near, _) = evaluate_locus_circle_eye(&rec, &q1, Vec3::new(5499.9, 0.0, 54500.0));
        assert!((e_edge - e_near).length() < 5.0);

        for q in [-3.0 * FRAC_PI_4, -FRAC_PI_4, FRAC_PI_4, 3.0 * FRAC_PI_4] {
            let c = cir(1000.0, 2000.0, q, 1000.0);
            let pt = |a: f32| Vec3::new(1000.0 + a.sin() * 1500.0, 0.0, 2000.0 + a.cos() * 1500.0);
            let d = evaluate_locus_circle_eye(&rec, &c, pt(q + 1e-3)).0
                - evaluate_locus_circle_eye(&rec, &c, pt(q - 1e-3)).0;
            assert!(d.length() < 25.0);
        }
    }

    #[test]
    fn test_through_door_formula_and_session_lifecycle() {
        let prm = ThroughDoorCamParam {
            rail_ang_y: 0.0,
            rail_sta_pos: [1000.0, -1137.5, 2000.0],
        };
        let ez = |y: f32, i: bool| evaluate_through_door_eye(&prm, &RoadArea::default(), y, i).z;
        assert!((ez(0.0, true) - 1350.0).abs() < 1e-3 && (ez(0.0, false) - 2025.0).abs() < 1e-3);
        assert!((ez(PI - 1e-4, false) - 2725.0).abs() < 1.0);

        let mut s = ThroughDoorSession::default();
        s.activate(Vec3::ZERO, 0.0, false);
        assert!(s.step(1.0 / 60.0, Vec3::ZERO, 0.0, None).is_some());
        let p300 = Vec3::new(0.0, 0.0, 300.0);
        assert!(s.step(0.016, p300, PI, None).is_none());

        s.activate(Vec3::ZERO, 0.0, false);
        let p100 = Vec3::new(0.0, 0.0, 100.0);
        let _ = s.step(1.3, p100, 0.0, Some(500.0));
        assert!(s.step(0.016, p100, 0.0, Some(500.0)).is_none());
    }

    #[test]
    fn test_retail_cam_corpus_locus_circle_and_through_door_controller() {
        let mut buf = [0u8; 256];
        buf[0x54..0x58].copy_from_slice(&10000.0f32.to_le_bytes());
        buf[0x6C..0x70].copy_from_slice(&5i32.to_le_bytes());
        buf[0x70..0x74].copy_from_slice(&FRAC_PI_2.to_le_bytes());
        buf[0x78..0x7C].copy_from_slice(&(-1137.5f32).to_le_bytes());
        buf[0x80 + 0x44] = 0x01;
        let mut ctrl = Ps2CameraController::new();
        ctrl.set_cam_file(parse_cam(&buf).ok());
        ctrl.reset_to_player(Vec3::ZERO, FRAC_PI_2);
        assert_eq!(ctrl.mv_type, CameraMovementType::ThroughDoor);
        assert!((ctrl.curr_snapshot.eye.x - 25.0).abs() < 1e-2);

        let bg = std::path::Path::new(
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg",
        );
        if !bg.exists() {
            return;
        }
        let mut locus_total = 0usize;
        for a in ["cb", "cc"] {
            let Ok(rd) = std::fs::read_dir(bg.join(a)) else {
                continue;
            };
            for e in rd.flatten() {
                let Ok(bytes) = std::fs::read(e.path()) else {
                    continue;
                };
                let Ok(cam) = parse_cam(&bytes) else { continue };
                for rec in cam.records {
                    if rec.cam_mv_type != CamMovementType::LocusCircle {
                        continue;
                    }
                    locus_total += 1;
                    let c = rec.lim_sw.center();
                    ctrl.set_cam_file(Some(CamFile { records: vec![rec] }));
                    ctrl.reset_to_player(Vec3::new(c[0], 0.0, c[2]), 0.0);
                    ctrl.tick(Vec3::new(c[0] + 150.0, 0.0, c[2] - 150.0), 0.4, 0.0, 0.0);
                    assert_eq!(ctrl.mv_type, CameraMovementType::LocusCircle);
                    assert!(ctrl.interpolated_view_matrix(0.5).is_finite());
                }
            }
        }
        assert_eq!(locus_total, 60);
    }
}
