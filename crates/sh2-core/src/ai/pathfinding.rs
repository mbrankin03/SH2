//! Enemy pathfinding, room forbidden areas, sleep culling, and tribe communication (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_common.c`):
//!   - `0x00144aa0` `enInitPath` — initialize steering angles and path state.
//!   - `0x00144ae0` `enSetPath` — obstacle-avoidance raycast steering & ±15°..±75° fan search.
//!   - `0x00147000` `enRoomForbiddenArea` — stage/room exclusion rectangles (`min_x, min_z, max_x, max_z`).
//!   - `0x00147420` `enCheckForbiddenAreaSub` — inflated AABB ray-segment clipping.
//!   - `0x00142950` `enCheckSleepIn` / `0x001429f0` `enCheckSleepOut` — outdoor `15000.0` / `7500.0` mm sleep culling.
//!   - `0x00146d80` `enSetCommunication` / `0x00146e40` `enCommunicateTribe` — 8-slot tribe alert pool.
//! - Retail PC (`sh2pc.exe`):
//!   - `0x004aadf0` `enSetPath` — fixes `SLUS_202.28` `0x00144e24` MIPS `c.olt.s $f8,$f8` sign bug in wall-follow steer.
//!   - `0x004a84a0` `enRoomForbiddenArea` — adds retail stage 3 / 0x10 (BFAW) and rooms `0x15, 0x1f, 0x83, 0x84, 0xa8(2nd), 0xb8, 0xc8, 0xcc`.

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

/// Outdoor sleep-in distance threshold (`enCheckSleepIn` `SLUS 0x00142950` / `sh2pc 0x004a63f0`).
pub const ENEMY_SLEEP_IN_DIST_MM: f32 = 15_000.0;

/// Outdoor wake-up distance threshold (`enCheckSleepOut` `SLUS 0x001429f0` / `sh2pc 0x004a6400`).
pub const ENEMY_SLEEP_OUT_DIST_MM: f32 = 7_500.0;

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
    /// - `sway_sample`: deterministic sample in `[-0.5, 0.5]` matching `shSway1f(-0.5, 0.5)`.
    /// - `frame_delta_ticks`: tick decrement matching `shGetDF()` (typically `1` at 60 Hz).
    /// - Returns `false` (`0`) when LOS to `target_pos` is clear, `true` (`1`) when steering around an obstacle.
    ///
    /// Uses the retail `sh2pc.exe` (`0x004ab110`) signed wall-follow fix rather than the `SLUS_202.28`
    /// `0x00144e24` MIPS `c.olt.s $f8,$f8` bug.
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

/// Axis-aligned XZ rectangle in PS2 world mm (`enRoomForbiddenArea` `SLUS 0x00147000` / `sh2pc 0x004a84a0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ForbiddenRect {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

impl ForbiddenRect {
    /// Create a forbidden XZ rectangle.
    pub const fn new(min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Self {
        Self {
            min_x,
            min_z,
            max_x,
            max_z,
        }
    }

    /// Returns `true` if `pos.(x, z)` lies inside this rectangle (`enExecTask` `SLUS 0x001413f0`).
    pub fn contains_xz(&self, pos: Vec3) -> bool {
        pos.x >= self.min_x && pos.x <= self.max_x && pos.z >= self.min_z && pos.z <= self.max_z
    }
}

/// Room-level AI flags computed by `enRoomForbiddenArea` (`_DAT_0055244c` in `SLUS 0x00147000` / `_DAT_01f7cdec` in `sh2pc 0x004a84a0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ForbiddenAreaFlags {
    /// Bit 0 (`0x01`): player is currently standing inside an active forbidden area.
    pub player_in_forbidden: bool,
    /// Bit 1 (`0x02`): special boss/encounter rooms (`0x30, 0x65, 0xbb`, plus `0xb8` on retail PC).
    pub special_room: bool,
    /// Bit 2 (`0x04`): tight-corridor down-radius flag (`sh2pc 0x004a84a0`: `150.0` vs `225.0` mm in `enSCUCtrlDown`).
    pub narrow_down_radius: bool,
    /// Bit 4 (`0x10`): room `0x27` flag (`sh2pc 0x004a84a0`).
    pub room_0x27_flag: bool,
}

/// Active forbidden rectangles and flags for a `(stage_id, room_id)` pair (`SLUS 0x00147000` / `sh2pc 0x004a84a0`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomForbiddenAreas {
    pub rects: Vec<ForbiddenRect>,
    pub flags: ForbiddenAreaFlags,
}

/// Resolve the active forbidden areas and room flags for `(stage_id, room_id)`.
///
/// - When `retail_pc` is `true`, uses the final retail `sh2pc.exe` (`0x004a84a0`) table
///   (including Born From a Wish `stage == 0x10` and added rooms `0x15, 0x1f, 0x83, 0x84, 0xa8(2nd), 0xb8`).
/// - When `retail_pc` is `false`, uses the exact `SLUS_202.28` (`0x00147000`) prototype table.
pub fn en_room_forbidden_area(stage_id: u32, room_id: u32, retail_pc: bool) -> RoomForbiddenAreas {
    let mut out = RoomForbiddenAreas::default();

    if stage_id == 3 {
        if retail_pc {
            out.rects
                .push(ForbiddenRect::new(-20_500.0, 86_000.0, -15_000.0, 92_500.0));
        }
        out.rects
            .push(ForbiddenRect::new(-20_000.0, 40_000.0, -4_000.0, 57_000.0));
        out.rects.push(ForbiddenRect::new(
            -100_000.0, 60_000.0, -80_000.0, 100_000.0,
        ));
    } else if retail_pc && stage_id == 0x10 {
        out.flags.narrow_down_radius = true;
        match room_id {
            0xc8 => out.rects.push(ForbiddenRect::new(
                -63_500.0, 103_300.0, -61_500.0, 104_700.0,
            )),
            0xcc => out
                .rects
                .push(ForbiddenRect::new(58_000.0, -59_800.0, 58_800.0, -56_000.0)),
            _ => {}
        }
    } else {
        match room_id {
            0x07 => {
                if retail_pc {
                    out.rects
                        .push(ForbiddenRect::new(82_000.0, -71_300.0, 85_000.0, -68_000.0));
                } else {
                    out.rects
                        .push(ForbiddenRect::new(83_500.0, -71_300.0, 85_000.0, -69_600.0));
                }
            }
            0x15 if retail_pc => {
                out.rects.push(ForbiddenRect::new(
                    -103_400.0, 8_700.0, -102_200.0, 10_800.0,
                ));
            }
            0x1f if retail_pc => {
                out.rects
                    .push(ForbiddenRect::new(55_500.0, 8_000.0, 57_200.0, 12_000.0));
            }
            0x83 if retail_pc => {
                out.rects.push(ForbiddenRect::new(
                    56_400.0, -101_000.0, 57_400.0, -99_000.0,
                ));
                out.rects.push(ForbiddenRect::new(
                    62_600.0, -101_000.0, 63_600.0, -99_000.0,
                ));
            }
            0x84 if retail_pc => {
                out.rects
                    .push(ForbiddenRect::new(60_000.0, 99_000.0, 65_000.0, 100_400.0));
            }
            0x92 => {
                out.rects
                    .push(ForbiddenRect::new(-20_800.0, 18_400.0, -19_200.0, 22_800.0));
            }
            0x99 => {
                out.rects
                    .push(ForbiddenRect::new(-64_000.0, 20_800.0, -63_200.0, 23_500.0));
                out.rects
                    .push(ForbiddenRect::new(-60_750.0, 20_750.0, -59_250.0, 25_600.0));
            }
            0xa0 | 0xb2 => {
                out.rects
                    .push(ForbiddenRect::new(60_000.0, -19_400.0, 60_750.0, -16_800.0));
                out.rects
                    .push(ForbiddenRect::new(61_200.0, -19_400.0, 62_000.0, -16_800.0));
            }
            0xa3 | 0xb5 => {
                out.rects
                    .push(ForbiddenRect::new(16_000.0, 20_000.0, 16_800.0, 22_600.0));
            }
            0xa8 => {
                out.rects.push(ForbiddenRect::new(
                    -20_800.0, -60_000.0, -19_200.0, -55_600.0,
                ));
                if retail_pc {
                    out.rects.push(ForbiddenRect::new(
                        -16_000.0, -59_200.0, -14_700.0, -58_000.0,
                    ));
                }
            }
            0x30 | 0x65 | 0xbb => {
                out.flags.special_room = true;
            }
            0xb8 if retail_pc => {
                out.flags.special_room = true;
            }
            _ => {}
        }

        if retail_pc {
            if matches!(
                room_id,
                0x17 | 0x18 | 0x1a | 0x39 | 0x3a | 0x4b | 0x4c | 0x51 | 0x68
            ) {
                out.flags.narrow_down_radius = true;
            }
            if room_id == 0x27 {
                out.flags.room_0x27_flag = true;
            }
        }
    }

    out
}

/// Clip segment `from -> to` against an inflated forbidden rectangle (`enCheckForbiddenAreaSub` `SLUS 0x00147420`).
///
/// Inflates `rect` by `radius` on all 4 sides. If clipped, updates `to.(x, z)` to the entry intersection and returns `true`.
pub fn en_check_forbidden_area_sub(
    radius: f32,
    rect: ForbiddenRect,
    from: Vec3,
    to: &mut Vec3,
) -> bool {
    let min_x = rect.min_x - radius;
    let max_x = rect.max_x + radius;
    let min_z = rect.min_z - radius;
    let max_z = rect.max_z + radius;

    // Trivial reject when both endpoints lie outside any side.
    if (from.x < min_x && to.x < min_x)
        || (from.x > max_x && to.x > max_x)
        || (from.z < min_z && to.z < min_z)
        || (from.z > max_z && to.z > max_z)
    {
        return false;
    }

    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let mut t_enter = 0.0_f32;
    let mut t_exit = 1.0_f32;

    if dx.abs() < 1e-6 {
        if from.x < min_x || from.x > max_x {
            return false;
        }
    } else {
        let tx1 = (min_x - from.x) / dx;
        let tx2 = (max_x - from.x) / dx;
        let (tmin, tmax) = if tx1 < tx2 { (tx1, tx2) } else { (tx2, tx1) };
        t_enter = t_enter.max(tmin);
        t_exit = t_exit.min(tmax);
    }

    if dz.abs() < 1e-6 {
        if from.z < min_z || from.z > max_z {
            return false;
        }
    } else {
        let tz1 = (min_z - from.z) / dz;
        let tz2 = (max_z - from.z) / dz;
        let (tmin, tmax) = if tz1 < tz2 { (tz1, tz2) } else { (tz2, tz1) };
        t_enter = t_enter.max(tmin);
        t_exit = t_exit.min(tmax);
    }

    if t_enter <= t_exit && (0.0..=1.0).contains(&t_enter) {
        to.x = from.x + dx * t_enter;
        to.z = from.z + dz * t_enter;
        true
    } else {
        false
    }
}

/// Evaluate whether an active enemy should enter sleep culling (`enCheckSleepIn` `SLUS 0x00142950` / `sh2pc 0x004a63f0`).
///
/// Returns `true` when the enemy should sleep (outdoor distance `> 15000.0` mm or not in active room and not forced-awake).
pub fn en_check_sleep_in(
    in_active_room: bool,
    force_awake: bool,
    is_outdoor: bool,
    player_dist_mm: f32,
) -> bool {
    !((in_active_room && (!is_outdoor || player_dist_mm <= ENEMY_SLEEP_IN_DIST_MM)) || force_awake)
}

/// Evaluate whether a sleeping enemy should wake up (`enCheckSleepOut` `SLUS 0x001429f0` / `sh2pc 0x004a6400`).
///
/// Returns `true` when the enemy should wake (in active room and, if outdoor, within `< 7500.0` mm of player).
pub fn en_check_sleep_out(in_active_room: bool, is_outdoor: bool, player_dist_mm: f32) -> bool {
    in_active_room && (!is_outdoor || player_dist_mm < ENEMY_SLEEP_OUT_DIST_MM)
}

/// Single alert slot in the 8-slot tribe communication pool (`0x005522a0`, `enSetCommunication` `0x00146d80`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TribeCommSlot {
    pub radius_mm: f32,
    pub tribe_id: u8,
    pub msg_type: u8,
    pub pos: Vec3,
    pub timer_ticks: i16,
}

/// 8-slot tribe alert broadcast pool (`enSetCommunication` `0x00146d80` / `enCommunicateTribe` `0x00146e40`).
///
/// - SCU (`tribe_id = 1`): `radius = 4000.0` mm, `duration = 120` ticks (`enSCUCtrlChase` `0x00161190`).
/// - MKN (`tribe_id = 2`): `radius = 1000.0` mm, `duration = 2` ticks (`enMKNCtrlMannequin` `0x001539e0`).
#[derive(Debug, Clone, Default)]
pub struct TribeCommPool {
    slots: [Option<TribeCommSlot>; 8],
}

impl TribeCommPool {
    /// Post a tribe alert into the first free slot (`enSetCommunication` `SLUS 0x00146d80`).
    pub fn post(&mut self, radius_mm: f32, tribe_id: u8, msg_type: u8, pos: Vec3, duration: i16) {
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(TribeCommSlot {
                    radius_mm,
                    tribe_id,
                    msg_type,
                    pos,
                    timer_ticks: duration,
                });
                return;
            }
        }
    }

    /// Decrement all active alert timers by `ticks` (`enExecTask` `SLUS 0x001413f0`).
    pub fn tick(&mut self, ticks: i16) {
        for slot in &mut self.slots {
            if let Some(s) = slot {
                s.timer_ticks -= ticks;
                if s.timer_ticks <= 0 {
                    *slot = None;
                }
            }
        }
    }

    /// Query the closest active alert for `tribe_id` within its broadcast radius (`enCommunicateTribe` `SLUS 0x00146e40`).
    pub fn query_nearest(&self, tribe_id: u8, pos: Vec3) -> Option<TribeCommSlot> {
        let mut best: Option<(f32, TribeCommSlot)> = None;
        for s in self.slots.iter().flatten() {
            if s.tribe_id != tribe_id {
                continue;
            }
            let d = pos.distance(s.pos);
            if d < s.radius_mm && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *s));
            }
        }
        best.map(|(_, s)| s)
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

        // Clear LOS returns false and aims directly at target (+Z => yaw 0).
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

        // Obstacle dead ahead (angle ~0), clear at +15° (+0.2618 rad), blocked at -15°.
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

    #[test]
    fn test_en_room_forbidden_area_slus_vs_pc() {
        let slus_stage3 = en_room_forbidden_area(3, 0, false);
        let pc_stage3 = en_room_forbidden_area(3, 0, true);
        assert_eq!(slus_stage3.rects.len(), 2);
        assert_eq!(pc_stage3.rects.len(), 3);

        // Clip segment entering room 0x07 forbidden rect.
        let r = en_room_forbidden_area(1, 0x07, false).rects[0];
        let from = Vec3::new(82_000.0, 0.0, -70_500.0);
        let mut to = Vec3::new(84_000.0, 0.0, -70_500.0);
        assert!(en_check_forbidden_area_sub(150.0, r, from, &mut to));
        assert!((to.x - (83_500.0 - 150.0)).abs() < 1e-3);
    }

    #[test]
    fn test_sleep_culling_thresholds_and_tribe_comm() {
        assert!(!en_check_sleep_in(true, false, true, 15_000.0));
        assert!(en_check_sleep_in(true, false, true, 15_001.0));
        assert!(!en_check_sleep_out(true, true, 7_500.0));
        assert!(en_check_sleep_out(true, true, 7_499.0));

        let mut pool = TribeCommPool::default();
        pool.post(4000.0, 1, 0, Vec3::new(1000.0, 0.0, 0.0), 2);
        assert!(pool.query_nearest(1, Vec3::ZERO).is_some());
        assert!(pool.query_nearest(2, Vec3::ZERO).is_none());
        pool.tick(2);
        assert!(pool.query_nearest(1, Vec3::ZERO).is_none());
    }
}
