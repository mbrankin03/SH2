//! Player height sampling (`PlayerSetHeight`), vertical step smoothing (`clCollectCharaPosition`),
//! and environment ray probes (`PlayerCheckStraightLine`, `PlayerCheckSideLine`, `PlayerCheckFootLine`).
//!
//! # Ground Truth
//! - `SLUS_202.28` `Chacter/m3_play.c`:
//!   - `PlayerCheckSideLine` (`0x00107730`)
//!   - `PlayerCheckFootLine` (`0x001078f0`)
//!   - `PlayerSetHeight` (`0x00107aa0`)
//!   - `PlayerCheckStraightLine` (`0x00110890`)
//! - `SLUS_202.28` `Collision/cl_main.c`:
//!   - `clCollectCharaPosition` (`0x001321d0`)
//! - `sh2pc.exe` `sh2pc_00520000.c:6120` (`PlayerSetHeight` retail stage `0x0e` ±2100 mm override).

use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_6};

use super::character::CharacterId;
use super::controller::CollisionWorld;
use super::raycast::{EyeRayHit, EyeRayMode};
use super::solver::{regulate_angle, sh_atan2};

/// Normal-stage upward floor-ray probe offset (`-250.0` mm above feet).
///
/// Ground truth: `sh2pc.exe` `sh2pc_00520000.c:6125` (`PlayerSetHeight`).
pub const PLAYER_HEIGHT_RAY_UP_NORMAL: f32 = 250.0;

/// Normal-stage downward floor-ray probe offset (`+1500.0` mm below feet).
///
/// Ground truth: `sh2pc.exe` `sh2pc_00520000.c:6126` (`PlayerSetHeight`).
pub const PLAYER_HEIGHT_RAY_DOWN_NORMAL: f32 = 1500.0;

/// Stage `0x0e` (stairwell / deep vertical stage) symmetric floor-ray probe extent (`±2100.0` mm).
///
/// Ground truth: `sh2pc.exe` `sh2pc_00520000.c:6120-6122` (`if (stage == 0x0e) ±2100.0`).
pub const PLAYER_HEIGHT_RAY_STAGE_0E: f32 = 2100.0;

/// Stage ID with extended `±2100.0` mm floor probe range (`sh2pc_00520000.c:6120`).
pub const STAGE_ID_EXTENDED_HEIGHT_RAY: u8 = 0x0e;

/// Maximum drop distance (`100.0` mm) that snaps immediately to the floor without entering freefall.
///
/// Ground truth: `SLUS_202.28` `PlayerSetHeight` (`0x00107aa0`, `pos.y + 100.0 < floor_y`).
pub const PLAYER_STEP_SNAP_THRESHOLD_MM: f32 = 100.0;

/// Drop distance (`250.0` mm) beyond which the player triggers the `Fall` lower-body state (`0x18`).
///
/// Ground truth: `SLUS_202.28` `PlayerSetHeight` (`0x00107aa0`, `pos.y + 250.0 < floor_y`).
pub const PLAYER_FALL_STATE_THRESHOLD_MM: f32 = 250.0;

/// Freefall vertical acceleration scalar (`9.8`) used in `fall_vel += 9.8 * dt * dt` (`m/s` units).
///
/// Ground truth: `SLUS_202.28` `PlayerSetHeight` (`0x00107aa0`) & `sh2pc_00520000.c:6146`.
pub const PLAYER_FREEFALL_GRAVITY_ACCEL: f32 = 9.8;

/// Maximum unsmoothed vertical delta per second (`600.0 / fps` mm/frame) in `clCollectCharaPosition`.
///
/// Ground truth: `SLUS_202.28` `clCollectCharaPosition` (`0x001321d0`, `if (600.0 / fps < |dy|) dy /= 2.0`).
pub const CL_POSITION_Y_SMOOTH_SPEED_MM_PER_SEC: f32 = 600.0;

/// Forward probe ray length (`900.0` mm) in `PlayerCheckStraightLine` (`0x00110890`).
pub const PLAYER_STRAIGHT_LINE_LEN_MM: f32 = 900.0;

/// Sloped lower-ray starting Y offset (`-150.0` mm above feet) in `PlayerCheckStraightLine` (`0x00110890`).
pub const PLAYER_STRAIGHT_LINE_LOW_Y_OFFSET_MM: f32 = -150.0;

/// Sloped lower-ray elevation angle (`-PI/6` = `-30°`, `0xbf060a92`) in `PlayerCheckStraightLine` (`0x00110890`).
pub const PLAYER_STRAIGHT_LINE_SLOPE_RAD: f32 = -FRAC_PI_6;

/// Distance threshold (`250.0` mm) below which `PlayerCheckStraightLine` flags `wall_ahead` (`DAT_003c8405 = 1`).
///
/// Ground truth: `SLUS_202.28` `PlayerCheckStraightLine` (`0x00110890`).
pub const PLAYER_WALL_AHEAD_DIST_MM: f32 = 250.0;

/// Lateral side-probe origin Y offset (`-750.0` mm above feet) in `PlayerCheckSideLine` (`0x00107730`).
pub const PLAYER_SIDE_LINE_Y_OFFSET_MM: f32 = -750.0;

/// Lateral side-probe ray length (`600.0` mm) in `PlayerCheckSideLine` (`0x00107730`).
pub const PLAYER_SIDE_LINE_LEN_MM: f32 = 600.0;

/// Foot-probe upward ray offset (`-250.0` mm) in `PlayerCheckFootLine` (`0x001078f0`).
pub const PLAYER_FOOT_LINE_UP_MM: f32 = 250.0;

/// Foot-probe downward ray offset (`+750.0` mm) in `PlayerCheckFootLine` (`0x001078f0`).
pub const PLAYER_FOOT_LINE_DOWN_MM: f32 = 750.0;

/// Persistent vertical height and freefall state for `PlayerSetHeight` (`0x00107aa0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerHeightState {
    /// Last sampled walkable floor Y (`+0xd0` in `SLUS_202.28`, `+200` in `sh2pc.exe`).
    pub floor_y: f32,
    /// Floor surface normal (`+0xc0..+0xcc` in `SLUS_202.28`).
    pub floor_normal: Vec3,
    /// Accumulated downward velocity (`+0xb4` in `SLUS_202.28`, `+0xb0` in `sh2pc.exe`, m/s units).
    pub fall_velocity: f32,
    /// Whether `SCFreefallSwitch` (`chara+4 & 0x10000`) is currently active.
    pub is_freefall: bool,
}

impl Default for PlayerHeightState {
    fn default() -> Self {
        Self {
            floor_y: 0.0,
            floor_normal: Vec3::new(0.0, -1.0, 0.0),
            fall_velocity: 0.0,
            is_freefall: false,
        }
    }
}

/// Output of a single `PlayerSetHeight` (`0x00107aa0`) evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerHeightResult {
    /// Updated player Y coordinate in world mm (`+Y` downward).
    pub new_y: f32,
    /// Whether a floor triangle was hit by `clCheckHitEyesOnlyFloor`.
    pub floor_hit: bool,
    /// Whether `SCFreefallSwitch` (`0x10000`) is active this tick (`floor_y > pos.y + 100.0`).
    pub is_freefall: bool,
    /// Whether the drop exceeded `250.0` mm and triggered the `Fall` state flag (`0x1000000`).
    pub trigger_fall_state: bool,
    /// When `trigger_fall_state` is `true`, `false` (`0`) = falling forward, `true` (`1`) = falling backward.
    pub fall_backward: bool,
}

/// Exact 1:1 `PlayerSetHeight` (`SLUS_202.28` `0x00107aa0` / `sh2pc.exe` `sh2pc_00520000.c:6120`).
///
/// # Prototype vs. Retail Note
/// `sh2pc.exe` (`sh2pc_00520000.c:6120-6127`) explicitly extends the vertical floor probe from
/// `[-250.0, +1500.0]` to `[-2100.0, +2100.0]` on stage `0x0e`. Both binaries share identical
/// `100.0` mm step-snap, `9.8 * dt * dt` freefall velocity accumulation, and `250.0` mm `Fall`
/// state thresholds. Note that `sh2pc.exe` (`sh2pc_00520000.c:6154-6155`) contains a trailing
/// unconditional `pos.y = floor_y` snap after the `100.0` mm branch, whereas `SLUS_202.28`
/// (`0x00107aa0`) preserves `pos.y` during freefall (`pos.y + 100.0 < floor_y`); this
/// implementation follows `SLUS_202.28`'s freefall preservation so gravity/Fall states operate
/// properly without instant teleporting.
#[allow(clippy::too_many_arguments)]
pub fn player_set_height(
    world: &CollisionWorld,
    pos: Vec3,
    prev_pos: Vec3,
    yaw: f32,
    dt: f32,
    stage_id: u8,
    already_in_fall_state: bool,
    state: &mut PlayerHeightState,
) -> PlayerHeightResult {
    let (ray_up, ray_down) = if stage_id == STAGE_ID_EXTENDED_HEIGHT_RAY {
        (PLAYER_HEIGHT_RAY_STAGE_0E, PLAYER_HEIGHT_RAY_STAGE_0E)
    } else {
        (PLAYER_HEIGHT_RAY_UP_NORMAL, PLAYER_HEIGHT_RAY_DOWN_NORMAL)
    };

    let ray_start = Vec3::new(pos.x, pos.y - ray_up, pos.z);
    let ray_end = Vec3::new(pos.x, pos.y + ray_down, pos.z);

    let hit = world.check_hit_eyes_only_floor(ray_start, ray_end);
    let floor_hit = hit.is_some();
    if let Some(h) = hit {
        state.floor_y = h.point.y;
        state.floor_normal = h.normal;
    }

    let floor_y = state.floor_y;
    // Clear freefall switch at start of tick (`SCFreefallSwitch(param_1, 0)` in `0x00107aa0`)
    state.is_freefall = false;

    let mut new_y = pos.y;
    let mut trigger_fall_state = false;
    let mut fall_backward = false;

    if floor_y <= pos.y {
        // Floor is at or above feet (`+Y` downward): snap up and zero fall velocity
        state.fall_velocity = 0.0;
        new_y = floor_y;
    } else {
        // Floor is below feet: accumulate `9.8 * dt * dt` (`shGetDT() * 9.8 * shGetDT()`)
        state.fall_velocity += dt * PLAYER_FREEFALL_GRAVITY_ACCEL * dt;

        if pos.y + PLAYER_STEP_SNAP_THRESHOLD_MM < floor_y {
            // Drop > 100.0 mm: engage `SCFreefallSwitch(param_1, 1)`
            state.is_freefall = true;

            if pos.y + PLAYER_FALL_STATE_THRESHOLD_MM < floor_y && !already_in_fall_state {
                // Drop > 250.0 mm: signal `Fall` state (`player_flg_on(0x3c7f64, 0x1000000)`)
                trigger_fall_state = true;
                let move_dir = sh_atan2(pos.z - prev_pos.z, pos.x - prev_pos.x);
                let diff = regulate_angle(yaw - move_dir);
                fall_backward = diff.abs() >= FRAC_PI_2;
            }
        } else {
            // Drop <= 100.0 mm: step-snap directly to floor
            state.fall_velocity = 0.0;
            new_y = floor_y;
        }
    }

    PlayerHeightResult {
        new_y,
        floor_hit,
        is_freefall: state.is_freefall,
        trigger_fall_state,
        fall_backward,
    }
}

/// Exact 1:1 `clCollectCharaPosition` (`SLUS_202.28` `0x001321d0`) vertical step halving.
///
/// When a non-freefalling character's vertical delta `|prev_y - cur_y|` exceeds `600.0 / fps` mm
/// in a single frame, halves the vertical step (`cur_y += (prev_y - cur_y) * 0.5`) to smooth
/// sudden stair/curb height transitions.
#[inline]
pub fn cl_collect_chara_position_y_smooth(
    prev_y: f32,
    cur_y: f32,
    fps: f32,
    is_freefall: bool,
) -> f32 {
    if is_freefall || fps <= 0.0 || !fps.is_finite() {
        return cur_y;
    }
    let dy = prev_y - cur_y;
    if dy.abs() > CL_POSITION_Y_SMOOTH_SPEED_MM_PER_SEC / fps {
        cur_y + dy * 0.5
    } else {
        cur_y
    }
}

/// Result of `PlayerCheckStraightLine` (`SLUS_202.28` `0x00110890`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StraightLineProbeResult {
    /// Horizontal eye-height (`head_y`) 900 mm ray hit (`clCheckHitEyes` mode 0).
    pub head_hit: Option<EyeRayHit>,
    /// Sloped lower (`pos.y - 150.0` at `-30°` over 900 mm XZ) ray hit (`clCheckHitEyes` mode 0).
    pub low_hit: Option<EyeRayHit>,
    /// Dot product of normalized sloped ray direction with `low_hit` surface normal (`_DAT_003c7f34`).
    /// Used by `WallF` recoil trigger (`<= -0.85`).
    pub normal_dot: f32,
    /// Horizontal XZ distance from player to `low_hit` contact point (`_DAT_003c7f38`).
    pub hit_xz_dist: f32,
    /// `true` when `hit_xz_dist < 250.0` (`DAT_003c8405 = 1`, blocks sprinting into walls).
    pub wall_ahead: bool,
}

/// Exact 1:1 `PlayerCheckStraightLine` (`SLUS_202.28` `0x00110890`).
///
/// Casts two 900 mm `clCheckHitEyes(..., mode=0)` probes along `angle` (`yaw + angle_offset` in 3D
/// tank mode, or `angle_offset` directly in 2D mode):
/// 1. Horizontal probe at `head_y` (`chara + 0xa4`).
/// 2. Upward-sloped probe from `pos.y - 150.0` at `-30°` (`-PI/6`, `0xbf060a92`).
///
/// If either hits, computes `normal_dot` and `hit_xz_dist` from the sloped probe and sets
/// `wall_ahead = hit_xz_dist < 250.0`.
pub fn player_check_straight_line(
    world: &CollisionWorld,
    pos: Vec3,
    head_y: f32,
    yaw: f32,
    angle_offset: f32,
    is_2d_control: bool,
    exclude_character: Option<CharacterId>,
) -> StraightLineProbeResult {
    let angle = if is_2d_control {
        angle_offset
    } else {
        yaw + angle_offset
    };

    let (sin_a, cos_a) = angle.sin_cos();
    let end_x = pos.x + sin_a * PLAYER_STRAIGHT_LINE_LEN_MM;
    let end_z = pos.z + cos_a * PLAYER_STRAIGHT_LINE_LEN_MM;

    // 1. Horizontal head-height probe (`mode = 0` = NoThru)
    let head_start = Vec3::new(pos.x, head_y, pos.z);
    let head_end = Vec3::new(end_x, head_y, end_z);
    let head_hit = world.check_hit_eyes(
        head_start,
        head_end,
        EyeRayMode::NoThru,
        exclude_character,
        &[],
    );

    // 2. Sloped lower probe (`pos.y - 150.0` rising by `tan(-30°) * 900.0` along Y)
    let low_start_y = pos.y + PLAYER_STRAIGHT_LINE_LOW_Y_OFFSET_MM;
    let low_end_y = low_start_y
        + (PLAYER_STRAIGHT_LINE_SLOPE_RAD.sin() * PLAYER_STRAIGHT_LINE_LEN_MM)
            / PLAYER_STRAIGHT_LINE_SLOPE_RAD.cos();
    let low_start = Vec3::new(pos.x, low_start_y, pos.z);
    let low_end = Vec3::new(end_x, low_end_y, end_z);
    let low_hit = world.check_hit_eyes(
        low_start,
        low_end,
        EyeRayMode::NoThru,
        exclude_character,
        &[],
    );

    let mut normal_dot = 0.0f32;
    let mut hit_xz_dist = f32::INFINITY;
    let mut wall_ahead = false;

    if head_hit.is_some() || low_hit.is_some() {
        let ray_dir = (low_end - low_start).normalize_or_zero();
        if let Some(lh) = low_hit {
            let n = lh.normal.normalize_or_zero();
            normal_dot = ray_dir.dot(n);
            let dx = pos.x - lh.point.x;
            let dz = pos.z - lh.point.z;
            hit_xz_dist = (dx * dx + dz * dz).sqrt();
            if hit_xz_dist < PLAYER_WALL_AHEAD_DIST_MM {
                wall_ahead = true;
            }
        } else if let Some(hh) = head_hit {
            let n = hh.normal.normalize_or_zero();
            normal_dot = ray_dir.dot(n);
            let dx = pos.x - hh.point.x;
            let dz = pos.z - hh.point.z;
            hit_xz_dist = (dx * dx + dz * dz).sqrt();
            if hit_xz_dist < PLAYER_WALL_AHEAD_DIST_MM {
                wall_ahead = true;
            }
        }
    }

    StraightLineProbeResult {
        head_hit,
        low_hit,
        normal_dot,
        hit_xz_dist,
        wall_ahead,
    }
}

/// Result of `PlayerCheckSideLine` (`SLUS_202.28` `0x00107730`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SideLineProbeResult {
    /// Hit along `yaw + PI/2` (`600.0` mm at `pos.y - 750.0`).
    pub right_hit: Option<EyeRayHit>,
    /// Hit along `yaw - PI/2` (`600.0` mm at `pos.y - 750.0`).
    pub left_hit: Option<EyeRayHit>,
}

/// Exact 1:1 `PlayerCheckSideLine` (`SLUS_202.28` `0x00107730`).
///
/// Casts two horizontal `600.0` mm rays at `pos.y - 750.0` along `yaw + PI/2` and `yaw - PI/2`
/// using `clCheckHitEyes(..., mode=0)`.
pub fn player_check_side_line(
    world: &CollisionWorld,
    pos: Vec3,
    yaw: f32,
    exclude_character: Option<CharacterId>,
) -> SideLineProbeResult {
    let origin = Vec3::new(pos.x, pos.y + PLAYER_SIDE_LINE_Y_OFFSET_MM, pos.z);

    let (rsin, rcos) = (yaw + FRAC_PI_2).sin_cos();
    let right_end = Vec3::new(
        origin.x + rsin * PLAYER_SIDE_LINE_LEN_MM,
        origin.y,
        origin.z + rcos * PLAYER_SIDE_LINE_LEN_MM,
    );
    let right_hit = world.check_hit_eyes(
        origin,
        right_end,
        EyeRayMode::NoThru,
        exclude_character,
        &[],
    );

    let (lsin, lcos) = (yaw - FRAC_PI_2).sin_cos();
    let left_end = Vec3::new(
        origin.x + lsin * PLAYER_SIDE_LINE_LEN_MM,
        origin.y,
        origin.z + lcos * PLAYER_SIDE_LINE_LEN_MM,
    );
    let left_hit =
        world.check_hit_eyes(origin, left_end, EyeRayMode::NoThru, exclude_character, &[]);

    SideLineProbeResult {
        right_hit,
        left_hit,
    }
}

/// Result of `PlayerCheckFootLine` (`SLUS_202.28` `0x001078f0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootLineProbeResult {
    /// Left foot (bone `0x0e` = 14) floor hit with normalized surface normal (`_DAT_003c8120..003c814c`).
    pub left_foot_hit: Option<EyeRayHit>,
    /// Right foot (bone `0x0d` = 13) floor hit with normalized surface normal (`_DAT_003c8160..003c818c`).
    pub right_foot_hit: Option<EyeRayHit>,
}

/// Exact 1:1 `PlayerCheckFootLine` (`SLUS_202.28` `0x001078f0`).
///
/// Casts vertical floor-only rays (`clCheckHitEyesOnlyFloor`) from `foot.y - 250.0` to `foot.y + 750.0`
/// for both left (`bone 14`) and right (`bone 13`) foot world positions and normalizes the returned
/// floor plane normals.
pub fn player_check_foot_line(
    world: &CollisionWorld,
    left_foot_pos: Vec3,
    right_foot_pos: Vec3,
) -> FootLineProbeResult {
    let probe_foot = |foot: Vec3| -> Option<EyeRayHit> {
        let start = Vec3::new(foot.x, foot.y - PLAYER_FOOT_LINE_UP_MM, foot.z);
        let end = Vec3::new(foot.x, foot.y + PLAYER_FOOT_LINE_DOWN_MM, foot.z);
        world.check_hit_eyes_only_floor(start, end).map(|mut h| {
            h.normal = h.normal.normalize_or_zero();
            h
        })
    };

    FootLineProbeResult {
        left_foot_hit: probe_foot(left_foot_pos),
        right_foot_hit: probe_foot(right_foot_pos),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::character::{CharacterCylinder, CharacterId};
    use crate::collision::raycast::EyeHitKind;
    use sh2_assets::formats::cld::{parse_cld, FloorTriangle, HitPolyColumn, HitPolyPlane};

    fn make_floor_quad(y: f32, half: f32, material: u32) -> HitPolyPlane {
        HitPolyPlane {
            kind: 0,
            shape: 1,
            weight: 0,
            material,
            flg: 0,
            p: [
                [-half, y, -half, 1.0],
                [half, y, -half, 1.0],
                [half, y, half, 1.0],
                [-half, y, half, 1.0],
            ],
        }
    }

    fn make_wall_quad(
        z: f32,
        half_x: f32,
        y_top: f32,
        y_bottom: f32,
        material: u32,
    ) -> HitPolyPlane {
        HitPolyPlane {
            kind: 1,
            shape: 1,
            weight: 0,
            material,
            flg: 0,
            p: [
                [-half_x, y_top, z, 1.0],
                [half_x, y_top, z, 1.0],
                [half_x, y_bottom, z, 1.0],
                [-half_x, y_bottom, z, 1.0],
            ],
        }
    }

    #[test]
    fn test_player_set_height_normal_stage_0e_step_snap_freefall_and_fall() {
        let mut world = CollisionWorld::new();
        // Floor at Y = 0.0
        world.raw_floors.push(make_floor_quad(0.0, 1000.0, 0));
        let dt = 1.0 / 60.0;
        let mut state = PlayerHeightState::default();

        // 1. Step-snap within <= 100 mm drop (pos.y = -80.0, floor = 0.0 -> snap to 0.0, no freefall)
        let res = player_set_height(
            &world,
            Vec3::new(0.0, -80.0, 0.0),
            Vec3::new(0.0, -80.0, -10.0),
            0.0,
            dt,
            0x01,
            false,
            &mut state,
        );
        assert!(res.floor_hit);
        assert_eq!(res.new_y, 0.0);
        assert!(!res.is_freefall);
        assert!(!res.trigger_fall_state);
        assert_eq!(state.fall_velocity, 0.0);

        // 2. Drop > 100 mm and <= 250 mm (pos.y = -150.0, floor = 0.0 -> freefall 9.8*dt^2, no Fall state)
        let res_ff = player_set_height(
            &world,
            Vec3::new(0.0, -150.0, 0.0),
            Vec3::new(0.0, -150.0, -10.0),
            0.0,
            dt,
            0x01,
            false,
            &mut state,
        );
        assert!(res_ff.is_freefall);
        assert!(!res_ff.trigger_fall_state);
        assert!((state.fall_velocity - (9.8 * dt * dt)).abs() < 1e-6);

        // 3. Drop > 250 mm (pos.y = -300.0, floor = 0.0 -> freefall + Fall state triggered)
        let res_fall = player_set_height(
            &world,
            Vec3::new(0.0, -300.0, 0.0),
            Vec3::new(0.0, -300.0, -10.0),
            0.0,
            dt,
            0x01,
            false,
            &mut state,
        );
        assert!(res_fall.is_freefall);
        assert!(res_fall.trigger_fall_state);

        // 4. Stage 0x0e extended ±2100 mm ray reaches floor at +1800 mm below feet, while normal +1500 mm does not
        let mut deep_world = CollisionWorld::new();
        deep_world
            .raw_floors
            .push(make_floor_quad(1800.0, 1000.0, 0));
        let mut st_norm = PlayerHeightState::default();
        let res_norm = player_set_height(
            &deep_world,
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
            dt,
            0x01,
            false,
            &mut st_norm,
        );
        assert!(!res_norm.floor_hit);

        let mut st_0e = PlayerHeightState::default();
        let res_0e = player_set_height(
            &deep_world,
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
            dt,
            STAGE_ID_EXTENDED_HEIGHT_RAY,
            false,
            &mut st_0e,
        );
        assert!(res_0e.floor_hit);
        assert_eq!(st_0e.floor_y, 1800.0);
    }

    #[test]
    fn test_cl_collect_chara_position_y_halving() {
        // At 60 fps, threshold is 600.0 / 60.0 = 10.0 mm/frame
        // Delta = 4.0 mm (<= 10.0) -> no halving
        assert_eq!(
            cl_collect_chara_position_y_smooth(0.0, -4.0, 60.0, false),
            -4.0
        );
        // Delta = 20.0 mm (> 10.0) and NOT freefalling -> halved halfway back to prev_y (-10.0)
        assert_eq!(
            cl_collect_chara_position_y_smooth(0.0, -20.0, 60.0, false),
            -10.0
        );
        // Same delta while freefalling (0x10000 set) -> NO halving
        assert_eq!(
            cl_collect_chara_position_y_smooth(0.0, -20.0, 60.0, true),
            -20.0
        );
    }

    #[test]
    fn test_player_probes_and_cl_check_hit_eyes_modes() {
        let mut world = CollisionWorld::new();
        // Solid wall at Z = 200 (within 250 mm wall_ahead threshold)
        world
            .raw_walls
            .push(make_wall_quad(200.0, 1000.0, -2000.0, 100.0, 0));
        // Chainlink fence (material 9 = Thru) at Z = 100
        world
            .raw_walls
            .push(make_wall_quad(100.0, 1000.0, -2000.0, 100.0, 9));
        // Invisible boundary (material 12 = Thru) at Z = 50
        world
            .raw_walls
            .push(make_wall_quad(50.0, 1000.0, -2000.0, 100.0, 12));
        // Side wall (swd) at Z = 30
        world
            .raw_side_walls
            .push(make_wall_quad(30.0, 1000.0, -2000.0, 100.0, 0));
        // Floor at Y = 0
        world.raw_floors.push(make_floor_quad(0.0, 1000.0, 0));
        // Static BG column at X = 400, Z = 0, radius = 50
        world.columns.push(HitPolyColumn {
            kind: 0,
            shape: 0,
            weight: 0,
            material: 0,
            flg: 0,
            center_x: 400.0,
            y_bottom: 100.0,
            center_z: 0.0,
            y_top: -2000.0,
            radius: 50.0,
        });

        let origin = Vec3::new(0.0, -500.0, 0.0);
        let target = Vec3::new(0.0, -500.0, 500.0);

        // Mode Thru (1): skips material 9, 11, 12 and swd -> hits solid wall at Z = 200
        let h_thru = world
            .check_hit_eyes(origin, target, EyeRayMode::Thru, None, &[])
            .expect("Thru must hit solid wall");
        assert_eq!(h_thru.kind, EyeHitKind::Plane);
        assert!((h_thru.point.z - 200.0).abs() < 1e-2);

        // Mode NoThru (0): skips only material 12 and swd -> hits chainlink fence at Z = 100
        let h_nothru = world
            .check_hit_eyes(origin, target, EyeRayMode::NoThru, None, &[])
            .expect("NoThru must hit material 9 fence");
        assert!((h_nothru.point.z - 100.0).abs() < 1e-2);

        // Mode AllNoThru (2): hits swd at Z = 30
        let h_all = world
            .check_hit_eyes(origin, target, EyeRayMode::AllNoThru, None, &[])
            .expect("AllNoThru must hit swd");
        assert!((h_all.point.z - 30.0).abs() < 1e-2);

        // BG column raycast along +X (yaw + PI/2) via PlayerCheckSideLine
        let side = player_check_side_line(&world, Vec3::ZERO, 0.0, None);
        let rhit = side
            .right_hit
            .expect("Right side ray must hit BG column at X=400");
        assert_eq!(rhit.kind, EyeHitKind::BgColumn);
        assert!((rhit.point.x - 350.0).abs() < 1e-2);

        // PlayerCheckStraightLine along +Z: hits Z=100 (< 250 mm -> wall_ahead = true)
        let straight =
            player_check_straight_line(&world, Vec3::ZERO, -800.0, 0.0, 0.0, false, None);
        assert!(straight.head_hit.is_some());
        assert!(straight.wall_ahead);
        assert!((straight.hit_xz_dist - 100.0).abs() < 1.0);

        // PlayerCheckFootLine hits floor at Y=0 with normalized normal
        let feet = player_check_foot_line(
            &world,
            Vec3::new(-20.0, -5.0, 0.0),
            Vec3::new(20.0, -5.0, 0.0),
        );
        let lf = feet.left_foot_hit.expect("Left foot must hit floor");
        assert!((lf.point.y - 0.0).abs() < 1e-2);
        assert!((lf.normal.length() - 1.0).abs() < 1e-3);

        // Character cylinder hit test (excluding self)
        let enemy = CharacterCylinder::james(CharacterId(2), Vec3::new(-300.0, 0.0, 0.0));
        let h_char = world
            .check_hit_eyes(
                Vec3::new(0.0, -500.0, 0.0),
                Vec3::new(-600.0, -500.0, 0.0),
                EyeRayMode::Thru,
                Some(CharacterId(1)),
                &[enemy],
            )
            .expect("Ray must hit enemy character hit_col");
        assert_eq!(h_char.kind, EyeHitKind::Character);
        assert_eq!(h_char.character_id, Some(CharacterId(2)));
        let _ = FloorTriangle::new([[0.0; 3]; 3], 0);
    }

    #[test]
    fn test_retail_er01_cld_player_set_height_and_eye_rays() {
        let cld_path = std::path::Path::new(
            "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/er/er01.cld",
        );
        if !cld_path.exists() {
            return;
        }
        let bytes = std::fs::read(cld_path).expect("read er01.cld");
        let cld = parse_cld(&bytes).expect("parse er01.cld");
        let world = CollisionWorld::from_cld(&cld);
        let spawn = world
            .find_safe_spawn_point(0.0, 0.0)
            .expect("er01 spawn point");

        let mut hstate = PlayerHeightState::default();
        let res = player_set_height(
            &world,
            Vec3::new(spawn.x, spawn.y - 50.0, spawn.z),
            spawn,
            0.0,
            1.0 / 60.0,
            0x01,
            false,
            &mut hstate,
        );
        assert!(
            res.floor_hit,
            "PlayerSetHeight must hit retail er01.cld floor"
        );
        assert!(!res.is_freefall);
        assert!((res.new_y - spawn.y).abs() < 5.0);
    }
}
