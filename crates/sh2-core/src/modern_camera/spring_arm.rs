//! `.cld` spring-arm boom raycast and First-Person eye bounds solver (`SH2-93`).
//!
//! # Coordinate & Scale Conventions
//! - **World Scale:** `500.0` Silent Hill 2 world units = `1.0` meter (`1 unit = 2 mm`,
//!   matching `SH2_WORLD_UNIT_SCALE` in `move.c` / `0x54ef10`).
//! - **Vertical Axis (`+Y` Downward):** Floor rests near `Y = 0.0`; overhead ceilings sit at
//!   negative `Y` (e.g., `ap01.cld` `Y = -1200.0`, `hp05.cld` `Y = -1450.0`, `ap06.cld` `Y = -1600.0`).
//!   Staying *below* a ceiling therefore means `eye.y >= ceiling_y + ceiling_standoff_units`.

use glam::{Vec2, Vec3};
use sh2_assets::formats::cld::WallSegment;

use crate::collision::raycast::intersect_segment_xz;
use crate::collision::CollisionWorld;

/// Tuning parameters for `.cld` spring-arm collision and room bounds containment (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringArmConfig {
    /// Minimum standoff distance from `.cld` wall planes in SH2 world units (`45.0` = 9.0 cm).
    pub wall_standoff_units: f32,
    /// Lateral frustum feeler offset perpendicular to the boom ray in SH2 world units (`34.0` = 6.8 cm).
    pub probe_radius_units: f32,
    /// Minimum vertical clearance below overhead `.cld` ceilings in SH2 world units (`45.0` = 9.0 cm).
    pub ceiling_standoff_units: f32,
    /// Minimum vertical clearance above walkable `.cld` floor triangles in SH2 world units (`80.0` = 16.0 cm).
    pub floor_standoff_units: f32,
    /// Minimum standoff distance from walls for the First-Person eye anchor in SH2 world units (`32.0` = 6.4 cm).
    pub first_person_eye_standoff_units: f32,
    /// Exponential smoothing factor per 60 Hz tick when pulling inward on occlusion (`0.0..=1.0`).
    pub pull_in_rate: f32,
    /// Exponential smoothing factor per 60 Hz tick when extending back out after clearing walls (`0.0..=1.0`).
    pub extend_out_rate: f32,
    /// Horizontal radius in SH2 world units used to infer local room ceiling height from `.cld` walls.
    pub ceiling_search_radius_units: f32,
}

impl Default for SpringArmConfig {
    fn default() -> Self {
        Self {
            wall_standoff_units: 45.0,
            probe_radius_units: 34.0,
            ceiling_standoff_units: 45.0,
            floor_standoff_units: 80.0,
            first_person_eye_standoff_units: 32.0,
            pull_in_rate: 0.72,
            extend_out_rate: 0.14,
            ceiling_search_radius_units: 3500.0,
        }
    }
}

/// Persistent 60 Hz spring-arm boom state tracking smoothed arm length and occlusion telemetry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringArmState {
    /// Current smoothed boom length in SH2 world units (`2 mm` per unit).
    pub current_distance: f32,
    /// Fractional hit distance `0.0..=1.0` along the desired boom on the latest tick (`< 1.0` if occluded).
    pub last_hit_fraction: f32,
    /// Minimum ceiling `Y` (in `+Y`-down world units) active above the camera on the latest tick, if any.
    pub active_ceiling_y: Option<f32>,
}

impl SpringArmState {
    /// Creates a spring-arm state initialized to `initial_distance` SH2 world units.
    pub const fn new(initial_distance: f32) -> Self {
        Self {
            current_distance: initial_distance,
            last_hit_fraction: 1.0,
            active_ceiling_y: None,
        }
    }

    /// Immediately snaps the boom arm distance (used on room cuts and camera mode resets).
    pub fn reset(&mut self, distance: f32) {
        self.current_distance = distance.max(0.0);
        self.last_hit_fraction = 1.0;
        self.active_ceiling_y = None;
    }
}

impl Default for SpringArmState {
    fn default() -> Self {
        Self::new(640.0)
    }
}

/// Shortest Euclidean distance in the `XZ` plane from point `p` to 2D wall segment `wall`.
#[inline]
pub fn point_to_wall_distance_xz(px: f32, pz: f32, wall: &WallSegment) -> f32 {
    let a = Vec2::new(wall.start[0], wall.start[1]);
    let b = Vec2::new(wall.end[0], wall.end[1]);
    let p = Vec2::new(px, pz);
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Infers the enclosing room ceiling `Y` coordinate (world `+Y` downward) near `(x, z)` from `.cld`
/// wall/ceiling barriers (`group == 1, 2, 3` and active dynamic walls).
///
/// Ignores waist-high fixtures (such as bathroom sinks with `y_top > floor_y - 550.0`) unless
/// `(x, z)` is directly underneath an overhead lintel (`y_bottom < floor_y - 300.0`).
pub fn sample_local_ceiling_y(
    collision: &CollisionWorld,
    x: f32,
    z: f32,
    floor_y: f32,
    search_radius: f32,
    probe_radius: f32,
) -> Option<f32> {
    let mut room_ceiling_y: Option<f32> = None;
    let mut direct_lintel_y: Option<f32> = None;

    for w in collision.all_walls() {
        let dist = point_to_wall_distance_xz(x, z, w);
        if dist > search_radius {
            continue;
        }

        // Direct overhanging lintel / soffit above head level (e.g. doorway header or low duct)
        if dist <= probe_radius && w.y_bottom < floor_y - 550.0 {
            direct_lintel_y = Some(match direct_lintel_y {
                Some(prev) => prev.max(w.y_bottom),
                None => w.y_bottom,
            });
        }

        // Full-height room/corridor enclosure walls extend from near floor up to ceiling (`y_top`).
        let reaches_near_floor = w.y_bottom >= floor_y - 250.0;
        let tall_enough_for_ceiling = w.y_top <= floor_y - 950.0;
        if reaches_near_floor && tall_enough_for_ceiling {
            // In +Y down, if a room mixes outer walls (-1600) and corridor walls (-1200),
            // track the highest enclosing room ceiling (minimum numeric Y) within the local sector,
            // while preferring closer walls within 1800 units (~3.6 m).
            if dist <= 1800.0 {
                room_ceiling_y = Some(match room_ceiling_y {
                    Some(prev) => prev.min(w.y_top),
                    None => w.y_top,
                });
            } else if room_ceiling_y.is_none() {
                room_ceiling_y = Some(w.y_top);
            }
        }
    }

    match (room_ceiling_y, direct_lintel_y) {
        (Some(rc), Some(dl)) => Some(rc.max(dl)),
        (Some(rc), None) => Some(rc),
        (None, Some(dl)) => Some(dl),
        (None, None) => None,
    }
}

/// Sweeps a 3D ray `origin -> target` with parallel lateral `XZ` feelers (`±probe_radius`) against
/// all active `.cld` walls and ceiling segments, returning `(clamped_point, hit_fraction)`.
pub fn sweep_ray_against_cld(
    origin: Vec3,
    target: Vec3,
    collision: &CollisionWorld,
    standoff_units: f32,
    probe_radius: f32,
) -> (Vec3, f32) {
    let delta = target - origin;
    let horiz_len = Vec2::new(delta.x, delta.z).length();
    let total_len = delta.length();
    if total_len <= 1e-4 {
        return (target, 1.0);
    }

    let lateral = if horiz_len > 1e-4 && probe_radius > 0.0 {
        Vec2::new(-delta.z / horiz_len, delta.x / horiz_len) * probe_radius
    } else {
        Vec2::ZERO
    };

    let feeler_offsets = if lateral.length_squared() > 1e-6 {
        [Vec2::ZERO, lateral, -lateral]
    } else {
        [Vec2::ZERO, Vec2::ZERO, Vec2::ZERO]
    };
    let feeler_count = if lateral.length_squared() > 1e-6 {
        3
    } else {
        1
    };

    let mut best_t = 1.0f32;

    for w in collision.all_walls() {
        for off in feeler_offsets.iter().take(feeler_count) {
            let px = origin.x + off.x;
            let pz = origin.z + off.y;
            let qx = target.x + off.x;
            let qz = target.z + off.y;

            if let Some((t, _normal)) = intersect_segment_xz(px, pz, qx, qz, w) {
                let y_at_hit = origin.y + delta.y * t;
                let y_pad = probe_radius * 0.5;
                if y_at_hit >= w.y_top - y_pad && y_at_hit <= w.y_bottom + y_pad && t < best_t {
                    best_t = t.max(0.0);
                }
            }
        }
    }

    if best_t < 1.0 {
        let hit_dist = total_len * best_t;
        let safe_dist = (hit_dist - standoff_units).max(0.0);
        let safe_t = (safe_dist / total_len).clamp(0.0, 1.0);
        (origin + delta * safe_t, safe_t)
    } else {
        (target, 1.0)
    }
}

/// Geometric parameters for a single 60 Hz Over-the-Shoulder spring-arm boom query (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtsBoomQuery {
    /// Character spine centerline anchor in SH2 world units (`+Y` down).
    pub spine_anchor: Vec3,
    /// Desired lateral right/left shoulder pivot in SH2 world units.
    pub desired_shoulder_pivot: Vec3,
    /// Unit vector pointing from shoulder pivot backward toward desired camera eye.
    pub boom_dir_unit: Vec3,
    /// Unoccluded target boom distance in SH2 world units.
    pub desired_boom_distance: f32,
    /// Player floor reference `Y` in SH2 world units (`+Y` down).
    pub player_floor_y: f32,
}

/// Evaluates the `.cld` spring-arm boom from `query.spine_anchor` -> `query.desired_shoulder_pivot` -> `desired_eye`,
/// smoothly pulling in inside narrow corridors and clamping against room ceilings and floors.
pub fn solve_ots_spring_arm(
    query: &OtsBoomQuery,
    collision: &CollisionWorld,
    config: &SpringArmConfig,
    state: &mut SpringArmState,
) -> (Vec3, Vec3) {
    // 1. Constrain lateral shoulder pivot so hugging a wall never places the boom root outside the room
    let (safe_pivot, _) = sweep_ray_against_cld(
        query.spine_anchor,
        query.desired_shoulder_pivot,
        collision,
        config.wall_standoff_units,
        config.probe_radius_units * 0.5,
    );

    // 2. Sweep boom arm from safe shoulder pivot along boom_dir_unit
    let desired_dist = query.desired_boom_distance.max(1.0);
    let raw_desired_eye = safe_pivot + query.boom_dir_unit * desired_dist;
    let (_clamped_raw_eye, hit_fraction) = sweep_ray_against_cld(
        safe_pivot,
        raw_desired_eye,
        collision,
        config.wall_standoff_units,
        config.probe_radius_units,
    );
    state.last_hit_fraction = hit_fraction;

    let max_unoccluded_dist = desired_dist * hit_fraction;

    // 3. Asymmetric spring-arm smoothing: pull inward rapidly, never exceed hard wall clamp, ease outward smoothly
    if max_unoccluded_dist < state.current_distance {
        let lerped = state.current_distance
            + (max_unoccluded_dist - state.current_distance) * config.pull_in_rate.clamp(0.05, 1.0);
        state.current_distance = lerped.min(max_unoccluded_dist);
    } else {
        state.current_distance += (max_unoccluded_dist - state.current_distance)
            * config.extend_out_rate.clamp(0.01, 1.0);
        state.current_distance = state.current_distance.min(max_unoccluded_dist);
    }

    let mut eye = safe_pivot + query.boom_dir_unit * state.current_distance;

    // 4. Constrain vertical Y between local .cld ceiling (+Y down -> eye.y >= ceiling_y + standoff) and floor
    let ceiling_y = sample_local_ceiling_y(
        collision,
        eye.x,
        eye.z,
        query.player_floor_y,
        config.ceiling_search_radius_units,
        config.probe_radius_units,
    );
    state.active_ceiling_y = ceiling_y;
    if let Some(cy) = ceiling_y {
        let min_eye_y = cy + config.ceiling_standoff_units;
        if eye.y < min_eye_y {
            eye.y = min_eye_y;
        }
    }

    let floor_y = collision
        .sample_floor_surface(eye.x, eye.z, query.player_floor_y, 400.0, 600.0)
        .map(|s| s.height_y)
        .unwrap_or(query.player_floor_y);
    let max_eye_y = floor_y - config.floor_standoff_units;
    if eye.y > max_eye_y {
        eye.y = max_eye_y;
    }

    (safe_pivot, eye)
}

/// Constrains a First-Person eye position inside the active room's `.cld` walls, ceilings, and floors.
pub fn constrain_eye_in_room(
    player_position: Vec3,
    desired_eye: Vec3,
    collision: &CollisionWorld,
    config: &SpringArmConfig,
) -> Vec3 {
    let core_anchor = Vec3::new(player_position.x, desired_eye.y, player_position.z);
    let (mut eye, _) = sweep_ray_against_cld(
        core_anchor,
        desired_eye,
        collision,
        config.first_person_eye_standoff_units,
        config.probe_radius_units * 0.6,
    );

    if let Some(cy) = sample_local_ceiling_y(
        collision,
        eye.x,
        eye.z,
        player_position.y,
        config.ceiling_search_radius_units,
        config.probe_radius_units,
    ) {
        let min_y = cy + config.ceiling_standoff_units;
        if eye.y < min_y {
            eye.y = min_y;
        }
    }

    let floor_y = collision
        .sample_floor_surface(eye.x, eye.z, player_position.y, 350.0, 500.0)
        .map(|s| s.height_y)
        .unwrap_or(player_position.y);
    let max_y = floor_y - config.floor_standoff_units;
    if eye.y > max_y {
        eye.y = max_y;
    }

    eye
}
