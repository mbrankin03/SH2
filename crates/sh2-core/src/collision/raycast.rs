//! Camera ray occlusion clamping and line-of-sight raycasts (`clCheckHitEyes`).
//!
//! In Silent Hill 2, the 3rd-person chase camera uses raycast intersection against the static
//! and dynamic geometry to guarantee the camera does not pass through walls or clip into
//! out-of-bounds void:
//! - Casts from character target $(tx, tz)$ to camera target $(cx, cz)$ at height $y$.
//! - Tests against active wall segments in $XZ$.
//! - When an intersection $t \in [0, 1]$ is hit, the camera is clamped to stop slightly in front
//!   of the wall (`best - 0.08` margin).

use glam::Vec3;
use sh2_assets::formats::cld::WallSegment;

/// Result of a collision raycast hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// Distance parameter $t \in [0, 1]$ along ray origin -> target.
    pub t: f32,
    /// Exact 3D point of contact.
    pub point: Vec3,
    /// 2D surface normal of the impacted obstacle.
    pub normal: [f32; 2],
    /// Material index of the impacted surface.
    pub material: u32,
}

/// Filter options for raycast queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastFilter {
    /// Include ceiling / high overhead geometry (group 2). Default is false.
    pub include_ceilings: bool,
    /// Include dynamic stage obstacles. Default is true.
    pub include_dynamic: bool,
    /// Stop margin offset (e.g. 0.08 for authentic PS2 camera clamping).
    pub safety_margin: f32,
}

impl Default for RaycastFilter {
    fn default() -> Self {
        Self {
            include_ceilings: false,
            include_dynamic: true,
            safety_margin: 0.08,
        }
    }
}

/// 2D Line segment intersection in $XZ$ plane.
/// Returns parameter $t \in [0, 1]$ along line $P \to Q$, or `None`.
pub fn intersect_segment_xz(
    px: f32,
    pz: f32,
    qx: f32,
    qz: f32,
    wall: &WallSegment,
) -> Option<(f32, [f32; 2])> {
    let rx = qx - px;
    let rz = qz - pz;
    let sx = wall.end[0] - wall.start[0];
    let sz = wall.end[1] - wall.start[1];

    let den = rx * sz - rz * sx;
    if den.abs() < 1e-6 {
        return None;
    }

    let t = ((wall.start[0] - px) * sz - (wall.start[1] - pz) * sx) / den;
    let u = ((wall.start[0] - px) * rz - (wall.start[1] - pz) * rx) / den;

    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        // Wall normal: perpendicular to wall segment in XZ
        let wall_len = (sx * sx + sz * sz).sqrt().max(1e-4);
        let nx = -sz / wall_len;
        let nz = sx / wall_len;
        Some((t, [nx, nz]))
    } else {
        None
    }
}

/// Clamp camera position along a ray from subject `target` to `camera_pos` (`clCheckHitEyes` / `col_clamp_ray`).
///
/// Stops short of the closest obstacle by `filter.safety_margin`.
pub fn clamp_camera_ray<'a, I>(
    target: Vec3,
    camera_pos: Vec3,
    walls: I,
    filter: &RaycastFilter,
) -> Vec3
where
    I: Iterator<Item = &'a WallSegment>,
{
    let mut best_t = 1.0f32;
    let y = camera_pos.y;

    for w in walls {
        if !filter.include_ceilings && w.group == 2 {
            continue;
        }

        // Check if wall covers the ray height Y
        if !(w.y_top <= y && w.y_bottom >= y) {
            continue;
        }

        if let Some((t, _norm)) = intersect_segment_xz(
            target.x, target.z,
            camera_pos.x, camera_pos.z,
            w,
        ) {
            if t >= 0.0 && t < best_t {
                best_t = t;
            }
        }
    }

    if best_t < 1.0 {
        // Apply authentic PS2 safety margin
        let clamped_t = if best_t > filter.safety_margin {
            best_t - filter.safety_margin
        } else {
            0.0
        };

        Vec3::new(
            target.x + (camera_pos.x - target.x) * clamped_t,
            camera_pos.y,
            target.z + (camera_pos.z - target.z) * clamped_t,
        )
    } else {
        camera_pos
    }
}
