//! Enemy pathfinding (`enInitPath` / `enSetPath`), forbidden areas, sleep culling, and tribe communication (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_common.c`):
//!   - `0x00144aa0` `enInitPath` — initialize steering angles and path state.
//!   - `0x00144ae0` `enSetPath` — obstacle-avoidance raycast steering & ±15°..±75° fan search.
//!   - `0x00147000` `enRoomForbiddenArea` / `0x00147420` `enCheckForbiddenAreaSub` (in [`forbidden`]).
//!   - `0x00142950` `enCheckSleepIn` / `0x001429f0` `enCheckSleepOut` / `0x00146d80` `enSetCommunication` (in [`comm`]).
//! - Retail PC (`sh2pc.exe`):
//!   - `0x004aadf0` `enSetPath` — fixes `SLUS_202.28` `0x00144e24` MIPS `c.olt.s $f8,$f8` sign bug in wall-follow steer.

pub mod comm;
pub mod forbidden;

pub use comm::{
    en_check_sleep_in, en_check_sleep_out, TribeCommPool, TribeCommSlot, ENEMY_SLEEP_IN_DIST_MM,
    ENEMY_SLEEP_OUT_DIST_MM,
};
pub use forbidden::{
    en_check_forbidden_area_sub, en_room_forbidden_area, ForbiddenAreaFlags, ForbiddenRect,
    RoomForbiddenAreas,
};

use crate::ai::sensory::normalize_angle;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// 15 degrees in radians (`0x3e860a92` in `SLUS_202.28` `enSetPath` `0x00144ae0`).
pub const PATH_STEP_ANGLE_RAD: f32 = 0.261_799_4;

/// 1 degree fallback angle when `goal_ang == cur_angle` (`0x3c8efa35` in `enSetPath` `0x00144ae0`).
pub const PATH_ZERO_DIFF_FALLBACK_RAD: f32 = 0.017_453_292;

/// -0.1 degree nudge when initial ±15° probes fail (`enSetPath` `0x00144ae0`).
pub const PATH_STUCK_NUDGE_RAD: f32 = -0.001_745_329_2;

/// Extra distance added to blocked ray distance when probing alternate angles (`0x42480000` = `50.0` mm in `enSetPath` `0x00144ae0`).
pub const PATH_PROBE_EXTRA_MM: f32 = 50.0;

/// Maximum fan search step `k` (`2..=5`, covering ±30°, ±45°, ±60°, ±75° in `enSetPath` `0x00144ae0`).
pub const PATH_FAN_MAX_STEP: u8 = 5;

/// Per-enemy-type path lock duration table `_2091` (`SLUS 0x002a9cc8`, 12 entries indexed by internal type `0..=11`).
///
/// Order: `0=None, 1=SCU(30), 2=MKN(20), 3=TYU(0), 4=RED(20), 5=ONI(20), 6=NSE(15), 7=IKE(12), 8=PAP(30), 9=EDB(15), 10=ARM(12), 11=BOS(12)`.
pub const PATH_LOCK_DURATION_TABLE: [i16; 12] = [0, 30, 20, 0, 20, 20, 15, 12, 30, 15, 12, 12];

/// Internal enemy path-type index into [`PATH_LOCK_DURATION_TABLE`] (`*work` in `SLUS 0x00144ae0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(usize)]
pub enum PathEnemyType {
    None = 0,
    LyingFigure = 1,
    Mannequin = 2,
    Creeper = 3,
    PyramidHead = 4,
    TwinPyramidHead = 5,
    Nurse = 6,
    FleshLips = 7,
    AbstractDaddy = 8,
    Eddie = 9,
    Mandarin = 10,
    MaryBoss = 11,
}

impl PathEnemyType {
    /// Returns the base lock-timer duration in ticks from `_2091` (`SLUS 0x002a9cc8`).
    pub const fn lock_duration(self) -> i16 {
        PATH_LOCK_DURATION_TABLE[self as usize]
    }
}

/// Steering mode inside `EnLocalWork +0x2c` (`enSetPath` `SLUS 0x00144ae0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PathSteerState {
    /// `path_state == -1`: 180° turnaround recovery until within 15° of `target_angle`.
    Turnaround,
    /// `path_state == 0`: clear line-of-sight / initial obstacle probe.
    #[default]
    Clear,
    /// `path_state == 1`: wall-following steer (±15° candidates).
    WallFollow,
    /// `path_state == 2..=5`: progressive fan search at `±(k * 15°)`.
    FanSearch(u8),
}

/// Per-enemy pathfinding state matching `EnLocalWork +0x20..+0x2f` (`SLUS 0x00144aa0` / `0x00144ae0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct EnemyPathState {
    /// Current heading angle in radians (`+0x20`).
    pub cur_angle: f32,
    /// Desired steering angle in radians (`+0x24`).
    pub target_angle: f32,
    /// Distance of last raycast probe in mm (`+0x28`).
    pub last_hit_dist: f32,
    /// Pathfinding state machine (`+0x2c`).
    pub state: PathSteerState,
    /// Consecutive blocked-both-sides counter (`+0x2d`).
    pub stuck_cnt: u8,
    /// Steering lock timer in 60 Hz ticks (`+0x2e`).
    pub lock_timer: i16,
}

impl EnemyPathState {
    /// Initialize path state to face `angle` (`enInitPath` `SLUS 0x00144aa0`).
    pub fn init(&mut self, angle: f32) {
        let reg = normalize_angle(angle);
        self.cur_angle = reg;
        self.target_angle = reg;
        self.state = PathSteerState::Clear;
        self.lock_timer = 0;
    }

    /// Construct a new initialized path state (`enInitPath` `SLUS 0x00144aa0`).
    pub fn new(angle: f32) -> Self {
        let mut s = Self::default();
        s.init(angle);
        s
    }

    /// Execute one tick of `enSetPath` (`SLUS 0x00144ae0` / `sh2pc 0x004aadf0`).
    ///
    /// - `check_ray(from, to) -> f32` follows `enCheckPath` (`0x00145350`) convention:
    ///   - `< 0.0`: path is **clear**, magnitude is `-distance_mm`.
    ///   - `>= 0.0`: path is **blocked**, value is `+hit_distance_mm`.
    /// - Uses the retail `sh2pc.exe` (`0x004ab110`) signed wall-follow fix.
    pub fn set_path<F>(
        &mut self,
        enemy_type: PathEnemyType,
        self_pos: Vec3,
        target_pos: Vec3,
        sway_sample: f32,
        frame_delta_ticks: i16,
        check_ray: F,
    ) -> bool
    where
        F: Fn(Vec3, Vec3) -> f32,
    {
        let dx = target_pos.x - self_pos.x;
        let dz = target_pos.z - self_pos.z;
        let goal_ang = dx.atan2(dz);
        let dist = check_ray(self_pos, target_pos);

        if dist < 0.0 {
            self.target_angle = normalize_angle(goal_ang);
            self.last_hit_dist = -dist;
            self.state = PathSteerState::Clear;
            self.stuck_cnt = 0;
            self.lock_timer = 0;
            return false;
        }

        if self.lock_timer > 0 {
            self.lock_timer = self.lock_timer.saturating_sub(frame_delta_ticks.max(1));
            return true;
        }

        self.last_hit_dist = dist;
        let probe_dist = dist + PATH_PROBE_EXTRA_MM;
        let probe_at = |ang: f32| -> f32 {
            let p = Vec3::new(
                self_pos.x + ang.sin() * probe_dist,
                self_pos.y,
                self_pos.z + ang.cos() * probe_dist,
            );
            check_ray(self_pos, p)
        };
        let base_lock = enemy_type.lock_duration();

        let out_ang = match self.state {
            PathSteerState::Turnaround => {
                let diff = normalize_angle(self.target_angle - self.cur_angle).abs();
                if diff < PATH_STEP_ANGLE_RAD {
                    self.stuck_cnt = 0;
                    self.state = PathSteerState::WallFollow;
                }
                self.target_angle
            }

            PathSteerState::Clear => {
                self.state = PathSteerState::WallFollow;
                let a_plus = normalize_angle(goal_ang + PATH_STEP_ANGLE_RAD);
                let a_minus = normalize_angle(goal_ang - PATH_STEP_ANGLE_RAD);
                let d_plus = probe_at(a_plus);
                let d_minus = probe_at(a_minus);

                if d_plus < 0.0 && d_minus >= 0.0 {
                    a_plus
                } else if d_minus < 0.0 && d_plus >= 0.0 {
                    a_minus
                } else if d_plus >= 0.0 && d_minus >= 0.0 {
                    self.stuck_cnt = self.stuck_cnt.saturating_add(1);
                    if self.stuck_cnt >= 3 {
                        self.state = PathSteerState::Turnaround;
                        normalize_angle(goal_ang + PI)
                    } else {
                        self.state = PathSteerState::FanSearch(2);
                        normalize_angle(goal_ang + PATH_STUCK_NUDGE_RAD)
                    }
                } else {
                    self.state = PathSteerState::FanSearch(2);
                    normalize_angle(goal_ang + PATH_STUCK_NUDGE_RAD)
                }
            }

            PathSteerState::WallFollow => {
                let mut diff = normalize_angle(goal_ang - self.cur_angle);
                if diff == 0.0 {
                    diff = PATH_ZERO_DIFF_FALLBACK_RAD;
                }
                let sign = if diff >= 0.0 { 1.0 } else { -1.0 };
                let cand1 = if diff.abs() < PATH_STEP_ANGLE_RAD {
                    goal_ang
                } else {
                    normalize_angle(self.cur_angle + sign * PATH_STEP_ANGLE_RAD)
                };

                let d1 = probe_at(cand1);
                if d1 < 0.0 {
                    let scale = (sway_sample.clamp(-0.5, 0.5) + 1.0).max(0.0);
                    self.lock_timer = (f32::from(base_lock) * scale) as i16;
                    cand1
                } else {
                    let d_goal = probe_at(goal_ang);
                    if d_goal < 0.0 {
                        self.lock_timer = base_lock;
                        goal_ang
                    } else {
                        let (best_d, best_ang) = if d_goal > d1 {
                            (d_goal, goal_ang)
                        } else {
                            (d1, cand1)
                        };
                        let cand3 = normalize_angle(self.cur_angle - sign * PATH_STEP_ANGLE_RAD);
                        let d3 = probe_at(cand3);
                        if d3 < 0.0 {
                            self.lock_timer = base_lock;
                            cand3
                        } else if best_d < 0.0 {
                            self.lock_timer = base_lock;
                            best_ang
                        } else {
                            self.state = PathSteerState::FanSearch(2);
                            best_ang
                        }
                    }
                }
            }

            PathSteerState::FanSearch(k) => {
                let step = f32::from(k) * PATH_STEP_ANGLE_RAD;
                let a_plus = normalize_angle(goal_ang + step);
                let a_minus = normalize_angle(goal_ang - step);
                let d_plus = probe_at(a_plus);
                let d_minus = probe_at(a_minus);

                if d_plus < 0.0 && d_minus >= 0.0 {
                    self.state = PathSteerState::WallFollow;
                    a_plus
                } else if d_minus < 0.0 && d_plus >= 0.0 {
                    self.state = PathSteerState::WallFollow;
                    a_minus
                } else if d_plus < 0.0 && d_minus < 0.0 {
                    let next_k = k.saturating_add(1);
                    if next_k > PATH_FAN_MAX_STEP {
                        self.state = PathSteerState::WallFollow;
                        a_plus
                    } else {
                        self.state = PathSteerState::FanSearch(next_k);
                        goal_ang
                    }
                } else {
                    let best = if d_plus > d_minus { a_plus } else { a_minus };
                    let next_k = k.saturating_add(1);
                    if next_k > PATH_FAN_MAX_STEP {
                        self.state = PathSteerState::Turnaround;
                        normalize_angle(best + PI)
                    } else {
                        self.state = PathSteerState::FanSearch(next_k);
                        best
                    }
                }
            }
        };

        self.target_angle = normalize_angle(out_ang);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_en_set_path_clear_and_fan_avoidance() {
        let mut path = EnemyPathState::new(0.0);
        let self_pos = Vec3::ZERO;
        let target = Vec3::new(0.0, 0.0, 1000.0);

        let blocked = path.set_path(
            PathEnemyType::LyingFigure,
            self_pos,
            target,
            0.0,
            1,
            |a, b| -a.distance(b),
        );
        assert!(!blocked);
        assert_eq!(path.state, PathSteerState::Clear);
        assert!(path.target_angle.abs() < 1e-4);

        let blocked2 = path.set_path(
            PathEnemyType::LyingFigure,
            self_pos,
            target,
            0.0,
            1,
            |a, b| {
                let ang = (b.x - a.x).atan2(b.z - a.z);
                if ang > 0.1 {
                    -a.distance(b)
                } else {
                    400.0
                }
            },
        );
        assert!(blocked2);
        assert_eq!(path.state, PathSteerState::WallFollow);
        assert!((path.target_angle - PATH_STEP_ANGLE_RAD).abs() < 1e-4);
    }
}
