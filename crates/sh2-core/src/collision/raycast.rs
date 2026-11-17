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
use sh2_assets::formats::cld::{FloorTriangle, WallSegment};

use crate::modern_camera::CharacterHeadRigSpec;

/// Vertical offset from James's feet (`+Y` downward, `500.0` SH2 units = `1.0 m`) to ear/eye anchor height
/// (`-844.6` SH2 units = `1.6892 m`), extracted 1:1 from retail `hhh_jms.mdl` bones `15/20/22`
/// (`CharacterHeadRigSpec::JAMES.fallback_model_eye.y`) and `SLUS_202.28` `vcMakeHeroHeadPos` (`0x0019e320`).
pub const JAMES_HEAD_OFFSET_Y: f32 = CharacterHeadRigSpec::JAMES.fallback_model_eye.y;

/// Vertical offset from Maria's feet (`+Y` downward, `500.0` SH2 units = `1.0 m`) to ear/eye anchor height
/// (`-825.6` SH2 units = `1.6512 m`), extracted 1:1 from retail `hhh_mar.mdl` bones `9/23/25`
/// (`CharacterHeadRigSpec::MARIA.fallback_model_eye.y`) and `SLUS_202.28` `vcMakeHeroHeadPos` (`0x0019e320`).
pub const MARIA_HEAD_OFFSET_Y: f32 = CharacterHeadRigSpec::MARIA.fallback_model_eye.y;

/// Minimum separation along a 3D acoustic ray (`220.0` SH2 units = `44 cm`) required to count two
/// intersected `.cld` segments as distinct physical walls rather than opposite faces of one room partition.
pub const MIN_DISTINCT_BARRIER_SEPARATION_UNITS: f32 = 220.0;

/// Computes James's or Maria's 3D world-space ear/head position (`+Y` downward SH2 units) from feet `player_pos`.
#[inline]
pub fn listener_head_position(player_pos: Vec3, is_maria: bool) -> Vec3 {
    let dy = if is_maria {
        MARIA_HEAD_OFFSET_Y
    } else {
        JAMES_HEAD_OFFSET_Y
    };
    Vec3::new(player_pos.x, player_pos.y + dy, player_pos.z)
}

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

        if let Some((t, _norm)) =
            intersect_segment_xz(target.x, target.z, camera_pos.x, camera_pos.z, w)
        {
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

/// Zero-allocation summary of a 3D line-of-sight acoustic raycast from a 3D SFX emitter to
/// James's/Maria's head against `.cld` wall/door planes and multi-story floor slabs (`SH2-84`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcousticOcclusionHit {
    /// Number of distinct solid `.cld` wall or door barriers intersected (`wldofs` `group == 1` or `cedofs` `group == 2`).
    pub wall_hit_count: u32,
    /// Number of distinct solid `.cld` floor/ceiling slabs intersected vertically between stories.
    pub slab_hit_count: u32,
    /// Closest hit parameter $t \in [0.0, 1.0]$ along `emitter_pos -> listener_head_pos` (`1.0` if clear).
    pub closest_t: f32,
    /// Raw `.cld` surface material attribute (`0` = concrete, `7` = wood door, `5/11` = metal, etc.) of the closest barrier.
    pub primary_material: u32,
    /// Accumulated material-weighted transmission density (`0.0` = unoccluded, `~0.85` = 1 wood door, `1.0` = 1 wall, `>= 2.0` = multi-wall).
    pub barrier_density: f32,
}

impl Default for AcousticOcclusionHit {
    fn default() -> Self {
        Self::clear()
    }
}

impl AcousticOcclusionHit {
    /// Unoccluded clear line-of-sight result.
    #[inline]
    pub const fn clear() -> Self {
        Self {
            wall_hit_count: 0,
            slab_hit_count: 0,
            closest_t: 1.0,
            primary_material: 0,
            barrier_density: 0.0,
        }
    }

    /// Returns `true` if at least one solid wall, door, or floor slab occludes the direct path.
    #[inline]
    pub const fn is_occluded(&self) -> bool {
        self.wall_hit_count > 0 || self.slab_hit_count > 0
    }

    /// Total number of distinct physical barriers crossed between emitter and listener head.
    #[inline]
    pub const fn total_hits(&self) -> u32 {
        self.wall_hit_count + self.slab_hit_count
    }
}

/// Returns `true` when a `.cld` `_CL_HITPOLY_PLANE::material` (`+0x08`) is acoustically and
/// visually see-through (`Thru`), matching `SLUS_202.28` `clCheckHitEyeVectorWall` (`0x00139b90` at
/// `0x00139be4..0x00139c04`) and `clCheckHitEyesOnlyFloorThru` (`0x00139010`):
/// - `9`: Chainlink fence / wire mesh (`Thru` — sight and sound pass unoccluded)
/// - `11`: Toluca Prison iron cell bars & Labyrinth open metal floor grates (`Thru`)
/// - `12`: Invisible player walk-containment boundary (`er01.cld`, `rr21.cld`, `hp01.cld`, `ob01.cld`) (`Thru`)
#[inline]
pub const fn is_cld_material_thru(raw_material: u32) -> bool {
    // Synthetic surface-SFX pad markers have bit 0x80 set; only inspect raw `.cld` plane materials.
    if (raw_material & 0x80) != 0 {
        return false;
    }
    matches!(raw_material, 9 | 11 | 12)
}

/// Relative acoustic transmission weight per solid `.cld` `_CL_HITPOLY_PLANE::material` (`+0x08`).
///
/// Verified across all 337 retail `.cld` files (`{0, 1, 2, 3, 5, 7, 8, 9, 10, 11, 12, 99}`) and
/// `SLUS_202.28` `clCheckHitEyeVectorWall` (`0x00139b90`):
/// - `9, 11, 12` return `0.0` (`Thru` — chainlink fence, iron prison bars / grates, invisible walk boundary).
/// - `7, 8` return `0.85` (wooden doors / partitions in `ap01.cld` & `ps01.cld` and carpeted dividers).
/// - `5` returns `1.15` (solid boiler / bulkhead steel plate in `rr21.cld` & `ap01.cld`).
/// - `0, 1, 2, 3, 4, 6, 10, 99` (and any other solid `.cld` material) return `1.0` (standard concrete, plaster, brick, stone).
#[inline]
pub const fn cld_material_barrier_weight(raw_material: u32) -> f32 {
    if is_cld_material_thru(raw_material) {
        return 0.0;
    }
    let material = if (raw_material & 0x80) != 0 {
        raw_material & !0x80
    } else {
        raw_material
    };
    match material {
        7 | 8 => 0.85, // Wood door / carpeted partition
        5 => 1.15,     // Solid metal plate / bulkhead
        _ => 1.0,      // Standard concrete / plaster / stone wall (including retail material 99)
    }
}

/// 3D Möller–Trumbore ray-triangle intersection along segment `origin -> target`.
/// Returns parameter $t \in [0.0, 1.0]$ if the segment pierces `tri`.
#[inline]
pub fn intersect_ray_triangle_3d(origin: Vec3, target: Vec3, tri: &FloorTriangle) -> Option<f32> {
    let dir = target - origin;
    let v0 = Vec3::from_array(tri.v[0]);
    let v1 = Vec3::from_array(tri.v[1]);
    let v2 = Vec3::from_array(tri.v[2]);

    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-6 {
        return None;
    }

    let inv_det = 1.0 / det;
    let s = origin - v0;
    let u = inv_det * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(e1);
    let v = inv_det * dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = inv_det * e2.dot(q);
    if (0.0..=1.0).contains(&t) {
        Some(t)
    } else {
        None
    }
}

/// Casts a zero-allocation 3D line-of-sight ray from `emitter_pos` to `listener_head_pos`
/// (in SH2 world units, `+Y` downward) against active `.cld` walls (`wldofs` `group == 1` &
/// `cedofs` `group == 2`, matching `SLUS_202.28` `clCheckHitEyeVector` `0x00139420`) and
/// multi-story floor/ceiling slabs (`fldofs` `group == 0`, `clCheckHitEyesOnlyFloorThru` `0x00139010`) (`SH2-84`).
pub fn raycast_acoustic_occlusion<'w, 'f, W, F>(
    emitter_pos: Vec3,
    listener_head_pos: Vec3,
    walls: W,
    floors: F,
) -> AcousticOcclusionHit
where
    W: Iterator<Item = &'w WallSegment>,
    F: Iterator<Item = &'f FloorTriangle>,
{
    if !emitter_pos.is_finite() || !listener_head_pos.is_finite() {
        return AcousticOcclusionHit::clear();
    }
    let delta = listener_head_pos - emitter_pos;
    let ray_len = delta.length();
    if !ray_len.is_finite() || ray_len <= 10.0 {
        return AcousticOcclusionHit::clear();
    }

    const MAX_TRACKED_BARRIERS: usize = 8;
    const WALL_Y_EPSILON: f32 = 20.0;
    const ENDPOINT_T_MARGIN: f32 = 0.015;
    const SLAB_RAY_PAD: f32 = 20.0;

    let mut barrier_dists = [0.0f32; MAX_TRACKED_BARRIERS];
    let mut barrier_weights = [0.0f32; MAX_TRACKED_BARRIERS];
    let mut barrier_is_slab = [false; MAX_TRACKED_BARRIERS];
    let mut barrier_count = 0usize;

    let mut closest_t = 1.0f32;
    let mut primary_material = 0u32;

    // 1. Intersect vertical `.cld` wall, door (`wldofs` `group == 1`, e.g. `material == 7`), and
    //    upper-wall/lintel/arch (`cedofs` `group == 2`) planes.
    //    `SLUS_202.28` `clCheckHitEyeVector` (`0x00139420` at `0x001394dc..0x0013958c`),
    //    `clCheckHitEyesOnlyWall` (`0x00139110`), and `clCheckHitThrustGunVector` (`0x00137c70`)
    //    test `fldofs` (`0`), `wldofs` (`1`), and `cedofs` (`2`) while skipping `swdofs` (`group == 3`,
    //    waist-high character-walk pushback curbs/furniture tested only in `clCheckBg2Chara` `0x00133a9c`).
    for w in walls {
        if w.group == 3 || is_cld_material_thru(w.material) {
            continue;
        }
        if let Some((t, _normal)) = intersect_segment_xz(
            emitter_pos.x,
            emitter_pos.z,
            listener_head_pos.x,
            listener_head_pos.z,
            w,
        ) {
            if !t.is_finite() || !(ENDPOINT_T_MARGIN..=(1.0 - ENDPOINT_T_MARGIN)).contains(&t) {
                continue;
            }
            let y_at_hit = emitter_pos.y + delta.y * t;
            if y_at_hit < w.y_top - WALL_Y_EPSILON || y_at_hit > w.y_bottom + WALL_Y_EPSILON {
                continue;
            }

            let hit_dist = t * ray_len;
            let weight = cld_material_barrier_weight(w.material);
            if t < closest_t {
                closest_t = t;
                primary_material = w.material;
            }

            if let Some(existing_idx) = barrier_dists[..barrier_count]
                .iter()
                .position(|&d| (d - hit_dist).abs() < MIN_DISTINCT_BARRIER_SEPARATION_UNITS)
            {
                if weight > barrier_weights[existing_idx] {
                    barrier_weights[existing_idx] = weight;
                }
            } else if barrier_count < MAX_TRACKED_BARRIERS {
                barrier_dists[barrier_count] = hit_dist;
                barrier_weights[barrier_count] = weight;
                barrier_is_slab[barrier_count] = false;
                barrier_count += 1;
            }
        }
    }

    // 2. Intersect multi-story `.cld` floor/ceiling slabs when vertical separation spans stories (`|dy| > 450`).
    //    Extend the ray segment by `SLAB_RAY_PAD` (`20.0` SH2 units = `4 cm`) so emitters authored directly
    //    on an upstairs floor slab (`emitter_pos.y == -1600.0`, `t ≈ 0.0`) still intersect the 2F deck cleanly,
    //    while rejecting the lower story's own walkable floor beneath the lower endpoint's feet.
    if delta.y.abs() > 450.0 {
        let ray_dir_unit = delta / ray_len;
        let padded_origin = emitter_pos - ray_dir_unit * SLAB_RAY_PAD;
        let padded_target = listener_head_pos + ray_dir_unit * SLAB_RAY_PAD;
        let padded_len = ray_len + 2.0 * SLAB_RAY_PAD;
        let upper_y = emitter_pos.y.min(listener_head_pos.y);
        let lower_y = emitter_pos.y.max(listener_head_pos.y);

        for tri in floors {
            // `SLUS_202.28` `clCheckHitEyesOnlyFloorThru` (`0x00139010` -> `0x00139b90`):
            // open metal grates / wire mesh floors (`material == 9, 11, 12`, e.g. Mandarin under Labyrinth grates)
            // are acoustically see-through (`Thru`) and must not muffle vertical sources!
            if is_cld_material_thru(tri.material) {
                continue;
            }
            if let Some(t_padded) = intersect_ray_triangle_3d(padded_origin, padded_target, tri) {
                if !t_padded.is_finite() {
                    continue;
                }
                let hit_pt = padded_origin + (padded_target - padded_origin) * t_padded;
                let y_at_hit = hit_pt.y;
                // Accept iff one endpoint is at or above the slab (`+Y` down: `upper_y <= y_at_hit + 25.0`)
                // and the other endpoint lies in a story below the slab (`lower_y >= y_at_hit + 350.0`).
                if upper_y > y_at_hit + 25.0 || lower_y < y_at_hit + 350.0 {
                    continue;
                }

                let unpadded_dist = (t_padded * padded_len - SLAB_RAY_PAD).clamp(0.0, ray_len);
                let t = (unpadded_dist / ray_len).clamp(0.0, 1.0);
                let weight = cld_material_barrier_weight(tri.material);
                if t < closest_t {
                    closest_t = t;
                    primary_material = tri.material;
                }

                if let Some(existing_idx) = barrier_dists[..barrier_count].iter().position(|&d| {
                    (d - unpadded_dist).abs() < MIN_DISTINCT_BARRIER_SEPARATION_UNITS
                }) {
                    if weight > barrier_weights[existing_idx] {
                        barrier_weights[existing_idx] = weight;
                    }
                } else if barrier_count < MAX_TRACKED_BARRIERS {
                    barrier_dists[barrier_count] = unpadded_dist;
                    barrier_weights[barrier_count] = weight;
                    barrier_is_slab[barrier_count] = true;
                    barrier_count += 1;
                }
            }
        }
    }

    let mut wall_hit_count = 0u32;
    let mut slab_hit_count = 0u32;
    let mut barrier_density = 0.0f32;
    for i in 0..barrier_count {
        if barrier_is_slab[i] {
            slab_hit_count += 1;
        } else {
            wall_hit_count += 1;
        }
        barrier_density += barrier_weights[i];
    }

    AcousticOcclusionHit {
        wall_hit_count,
        slab_hit_count,
        closest_t,
        primary_material,
        barrier_density,
    }
}
