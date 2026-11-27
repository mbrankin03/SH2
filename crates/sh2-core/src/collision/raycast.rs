//! Camera ray occlusion clamping and 3D line-of-sight raycasts (`clCheckHitEyes`).
//!
//! # Ground Truth
//! - `SLUS_202.28` `Collision/cl_main.c`:
//!   - `clCheckHitEyes` (`0x00138ea0`): dispatches `mode == 0` (`NoThru`), `mode == 1` (`Thru`), `mode == 2` (`AllNoThru`).
//!   - `clCheckHitEyesOnlyFloor` (`0x00138f10`), `clCheckHitEyesOnlyFloorThru` (`0x00139010`),
//!     `clCheckHitEyesOnlyWall` (`0x00139110`), `clCheckHitEyesOnlyFloorCeil` (`0x001392c0`).
//!   - `clCheckHitEyeVectorWall` (`0x00139b90`: skips `Thru` materials `9, 11, 12`),
//!     `clCheckHitSwordVectorWall` (`0x001367f0`: skips material `12`),
//!     `clCheckHitNoThruVectorWall` (`0x00136990`: skips nothing).
//!   - `clCheckHitEyeVectorBGColumn` (`0x00139d40`), `clCheckHitEyeVectorCharacter` (`0x0013a590`).
//! - `SLUS_202.28` `Collision/cl_calc2.c`:
//!   - `clCheckSubLineToPlane` (`0x001315e0`), `clCheckSubLineToPlane3` (`0x00131830`),
//!     `clCheckSubLineToColumnPlus` (`0x00131a10`).

use glam::Vec3;
use sh2_assets::formats::cld::{FloorTriangle, HitPolyColumn, HitPolyPlane, WallSegment};

use super::character::CharacterId;
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

/// Raycast mode for `clCheckHitEyes` (`SLUS_202.28` `0x00138ea0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EyeRayMode {
    /// `mode == 0` (`clCheckHitEyeVectorNoThru` `0x00139680`): skips only invisible walk boundary `material == 12`.
    /// Tests dynamic walls/floors, static `fld`/`wld`/`ced`, BG columns, and characters.
    NoThru = 0,
    /// `mode == 1` (`clCheckHitEyeVector` `0x00139420`): skips see-through `Thru` materials `9, 11, 12`.
    /// Tests dynamic walls/floors, static `fld`/`wld`/`ced`, BG columns, and characters.
    Thru = 1,
    /// `mode == 2` (`clCheckHitEyeVectorAllNoThru` `0x001398e0`): skips NO materials and also tests `swd` (side walls).
    /// Tests dynamic walls/floors, static `fld`/`wld`/`swd`/`ced`, BG columns, and characters.
    AllNoThru = 2,
}

/// Geometry category hit by `clCheckHitEyes` (`SLUS_202.28` `cl_main.c` `*param_1` values `1, 2, 3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EyeHitKind {
    /// Static or dynamic `_CL_HITPOLY_PLANE` (floor, wall, ceiling, or side wall; `*param_1 = 1`).
    Plane = 1,
    /// Static `_CL_HITPOLY_COLUMN` background pillar (`*param_1 = 2`).
    BgColumn = 2,
    /// Active character hit cylinder (`*param_1 = 3`).
    Character = 3,
}

/// Full 3D raycast result matching `_CL_VHIT_RESULT` in `SLUS_202.28` `Collision/cl_main.c`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeRayHit {
    /// Geometry category hit (`Plane = 1`, `BgColumn = 2`, `Character = 3`).
    pub kind: EyeHitKind,
    /// Exact 3D contact point (`+0x10..+0x1c`).
    pub point: Vec3,
    /// Surface normal (`+0x20..+0x2c`, plane equation normal for planes, `Vec3::ZERO` for columns/characters).
    pub normal: Vec3,
    /// Normalized ray parameter `t` in `[0.0, 1.0]` along `origin -> target`.
    pub t: f32,
    /// Squared 3D distance from `origin` to `point` (`fStack_4` in `cl_main.c`).
    pub dist_sq: f32,
    /// Raw `.cld` plane material (`0` for columns/characters).
    pub material: u32,
    /// Character ID when `kind == EyeHitKind::Character`.
    pub character_id: Option<CharacterId>,
}

/// Returns `true` when `clCheckHitEyes` skips a plane with `raw_material` under `mode`.
///
/// Ground truth:
/// - `Thru` (`clCheckHitEyeVectorWall` `0x00139b90`): skips `9, 11, 12`.
/// - `NoThru` (`clCheckHitSwordVectorWall` `0x001367f0`): skips `12` (`0xc`).
/// - `AllNoThru` (`clCheckHitNoThruVectorWall` `0x00136990`): skips nothing.
#[inline]
pub const fn should_skip_material_for_mode(raw_material: u32, mode: EyeRayMode) -> bool {
    match mode {
        EyeRayMode::Thru => is_cld_material_thru(raw_material),
        EyeRayMode::NoThru => (raw_material & 0x80) == 0 && raw_material == 12,
        EyeRayMode::AllNoThru => false,
    }
}

/// Exact 3D segment-vs-`_CL_HITPOLY_PLANE` intersection matching `clCheckSubLineToPlane3` (`0x00131830`,
/// triangle) and `clCheckSubLineToPlane` (`0x001315e0`, quad) in `SLUS_202.28` `Collision/cl_calc2.c`.
///
/// Returns `(t, hit_point, normal)` if the 3D segment `origin -> target` pierces `plane`.
pub fn cl_check_sub_line_to_plane(
    origin: Vec3,
    target: Vec3,
    plane: &HitPolyPlane,
) -> Option<(f32, Vec3, Vec3)> {
    let v0 = Vec3::new(plane.p[0][0], plane.p[0][1], plane.p[0][2]);
    let v1 = Vec3::new(plane.p[1][0], plane.p[1][1], plane.p[1][2]);
    let v2 = Vec3::new(plane.p[2][0], plane.p[2][1], plane.p[2][2]);
    let is_quad = plane.shape != 0;

    // AABB early-out matching VU0 `_vmax`/`_vmini` reject in `0x001315e0` / `0x00131830`
    let mut pmin = v0.min(v1).min(v2);
    let mut pmax = v0.max(v1).max(v2);
    if is_quad {
        let v3 = Vec3::new(plane.p[3][0], plane.p[3][1], plane.p[3][2]);
        pmin = pmin.min(v3);
        pmax = pmax.max(v3);
    }
    let rmin = origin.min(target);
    let rmax = origin.max(target);
    if pmax.x < rmin.x
        || pmax.y < rmin.y
        || pmax.z < rmin.z
        || rmax.x < pmin.x
        || rmax.y < pmin.y
        || rmax.z < pmin.z
    {
        return None;
    }

    // Plane normal `n = (v2 - v0) × (v1 - v0)` (`clCalcPlaneEquation`)
    let e01 = v1 - v0;
    let e02 = v2 - v0;
    let normal = e02.cross(e01);
    let normal_len_sq = normal.length_squared();
    if normal_len_sq < 1e-10 {
        return None;
    }

    let dir = target - origin;
    let denom = normal.dot(dir);
    if denom.abs() < 1e-8 {
        return None;
    }

    // Plane equation `n · x + d = 0`, where `d = -(n · v0)`
    let d0 = normal.dot(origin - v0);
    let d1 = normal.dot(target - v0);
    // Segment must straddle or touch the plane (`d0` and `d1` opposite signs or zero)
    if (d0 > 0.0 && d1 > 0.0) || (d0 < 0.0 && d1 < 0.0) {
        return None;
    }

    let t = (-d0 / denom).clamp(0.0, 1.0);
    let hit_pt = origin + dir * t;

    // Test point-in-triangle (or 2-triangle quad `v0,v1,v2` + `v0,v2,v3`) using barycentric / edge cross signs
    let point_in_tri = |a: Vec3, b: Vec3, c: Vec3| -> bool {
        let ab = b - a;
        let bc = c - b;
        let ca = a - c;
        let ap = hit_pt - a;
        let bp = hit_pt - b;
        let cp = hit_pt - c;
        let c0 = ab.cross(ap).dot(normal);
        let c1 = bc.cross(bp).dot(normal);
        let c2 = ca.cross(cp).dot(normal);
        const EPS: f32 = 1e-3;
        let all_non_neg = c0 >= -EPS && c1 >= -EPS && c2 >= -EPS;
        let all_non_pos = c0 <= EPS && c1 <= EPS && c2 <= EPS;
        all_non_neg || all_non_pos
    };

    let inside = if is_quad {
        let v3 = Vec3::new(plane.p[3][0], plane.p[3][1], plane.p[3][2]);
        point_in_tri(v0, v1, v2) || point_in_tri(v0, v2, v3)
    } else {
        point_in_tri(v0, v1, v2)
    };

    if inside {
        Some((t, hit_pt, normal.normalize_or_zero()))
    } else {
        None
    }
}

/// Exact 3D segment-vs-vertical cylinder intersection matching `clCheckSubLineToColumnPlus`
/// (`SLUS_202.28` `0x00131a10`).
///
/// Cylinder is centered at `(cx, cz)` with radius `r`, spanning `[y_top .. y_bottom]` (`+Y` downward,
/// so `y_top <= y_bottom`). Returns `(t, hit_point)` for the closest contact in `[0.0, 1.0]`.
pub fn cl_check_sub_line_to_column_plus(
    origin: Vec3,
    target: Vec3,
    cx: f32,
    cz: f32,
    y_top: f32,
    y_bottom: f32,
    radius: f32,
) -> Option<(f32, Vec3)> {
    if radius <= 0.0 {
        return None;
    }

    // Vertical AABB quick reject (`0x00131a10`)
    let rmin_y = origin.y.min(target.y);
    let rmax_y = origin.y.max(target.y);
    if rmax_y < y_top || rmin_y > y_bottom {
        return None;
    }

    let ox = origin.x - cx;
    let oz = origin.z - cz;
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let dz = target.z - origin.z;

    let a = dx * dx + dz * dz;
    let r_sq = radius * radius;
    let c = ox * ox + oz * oz - r_sq;

    // Origin already inside cylinder in XZ: check Y overlap
    if c <= 0.0 {
        if origin.y >= y_top && origin.y <= y_bottom {
            return Some((0.0, origin));
        }
        // Check cap intersection if moving vertically into [y_top, y_bottom]
        if dy.abs() > 1e-8 {
            let cap_y = if origin.y < y_top { y_top } else { y_bottom };
            let t = (cap_y - origin.y) / dy;
            if (0.0..=1.0).contains(&t) {
                let hx = ox + dx * t;
                let hz = oz + dz * t;
                if hx * hx + hz * hz <= r_sq {
                    return Some((t, origin + Vec3::new(dx, dy, dz) * t));
                }
            }
        }
        return None;
    }

    if a < 1e-10 {
        return None;
    }

    let b = 2.0 * (ox * dx + oz * dz);
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }

    let sqrt_disc = disc.sqrt();
    let t0 = (-b - sqrt_disc) / (2.0 * a);
    let t1 = (-b + sqrt_disc) / (2.0 * a);

    // Check closest cylinder wall hit first
    for t in [t0, t1] {
        if (0.0..=1.0).contains(&t) {
            let hy = origin.y + dy * t;
            if hy >= y_top && hy <= y_bottom {
                return Some((t, origin + Vec3::new(dx, dy, dz) * t));
            }
        }
    }

    // Check top/bottom cap intersections (`0x00131cc0..0x00131d40`)
    if dy.abs() > 1e-8 {
        let mut best_cap: Option<(f32, Vec3)> = None;
        for cap_y in [y_top, y_bottom] {
            let t = (cap_y - origin.y) / dy;
            if (0.0..=1.0).contains(&t) {
                let hx = ox + dx * t;
                let hz = oz + dz * t;
                if hx * hx + hz * hz <= r_sq && best_cap.is_none_or(|(bt, _)| t < bt) {
                    best_cap = Some((t, origin + Vec3::new(dx, dy, dz) * t));
                }
            }
        }
        if best_cap.is_some() {
            return best_cap;
        }
    }

    None
}

/// Convenience wrapper for `cl_check_sub_line_to_column_plus` on a static `.cld` `HitPolyColumn`.
#[inline]
pub fn intersect_segment_bg_column(
    origin: Vec3,
    target: Vec3,
    col: &HitPolyColumn,
) -> Option<(f32, Vec3)> {
    cl_check_sub_line_to_column_plus(
        origin,
        target,
        col.center_x,
        col.center_z,
        col.y_top,
        col.y_bottom,
        col.radius,
    )
}

/// Filter options for raycast queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastFilter {
    /// Include ceiling / high overhead geometry (group 2). Default is `true` (matching `clCheckHitEyeVector`).
    pub include_ceilings: bool,
    /// Include dynamic stage obstacles. Default is `true`.
    pub include_dynamic: bool,
    /// Optional pullback margin along the ray. Default is `0.0` for 1:1 PS2 `clCheckHitEyes` (`0x00138ea0`) parity.
    pub safety_margin: f32,
}

impl Default for RaycastFilter {
    fn default() -> Self {
        Self {
            include_ceilings: true,
            include_dynamic: true,
            safety_margin: 0.0,
        }
    }
}

/// 2D line segment intersection in XZ plane without `sqrt` or normal computation.
///
/// Returns parameter `t` in `[0.0, 1.0]` along `P -> Q`, or `None`. Preferred for LOS,
/// camera clamping, and acoustic occlusion callers that only need the hit parameter `t`.
#[inline]
pub fn intersect_segment_xz_t(
    px: f32,
    pz: f32,
    qx: f32,
    qz: f32,
    wall: &WallSegment,
) -> Option<f32> {
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
        Some(t)
    } else {
        None
    }
}

/// 2D Line segment intersection in $XZ$ plane.
/// Returns parameter $t \in [0, 1]$ along line $P \to Q$ and unit 2D wall normal, or `None`.
pub fn intersect_segment_xz(
    px: f32,
    pz: f32,
    qx: f32,
    qz: f32,
    wall: &WallSegment,
) -> Option<(f32, [f32; 2])> {
    let t = intersect_segment_xz_t(px, pz, qx, qz, wall)?;
    let sx = wall.end[0] - wall.start[0];
    let sz = wall.end[1] - wall.start[1];
    // Wall normal: perpendicular to wall segment in XZ
    let wall_len = (sx * sx + sz * sz).sqrt().max(1e-4);
    let nx = -sz / wall_len;
    let nz = sx / wall_len;
    Some((t, [nx, nz]))
}

/// Clamp camera position along a 3D ray from subject `target` to `camera_pos` (`clCheckHitEyes` / `0x00138ea0`).
///
/// Skips `Thru` materials `9, 11, 12` (`clCheckHitEyeVectorWall` `0x00139b90`) and returns the exact
/// 3D contact point (or pulled back by `filter.safety_margin`, which defaults to `0.0` for 1:1 PS2 parity).
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
    let dy = camera_pos.y - target.y;

    for w in walls {
        if !filter.include_ceilings && w.group == 2 {
            continue;
        }
        if is_cld_material_thru(w.material) {
            continue;
        }

        if let Some(t) = intersect_segment_xz_t(target.x, target.z, camera_pos.x, camera_pos.z, w) {
            let y_at_hit = target.y + dy * t;
            if !(w.y_top <= y_at_hit && w.y_bottom >= y_at_hit) {
                continue;
            }
            if t >= 0.0 && t < best_t {
                best_t = t;
            }
        }
    }

    if best_t < 1.0 {
        let clamped_t = if best_t > filter.safety_margin {
            best_t - filter.safety_margin
        } else {
            0.0
        };

        Vec3::new(
            target.x + (camera_pos.x - target.x) * clamped_t,
            target.y + dy * clamped_t,
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

/// Zero-allocation incremental accumulator shared by brute-force [`raycast_acoustic_occlusion`]
/// and `SpatialGrid`-accelerated [`super::CollisionWorld::raycast_acoustic_occlusion`].
pub(crate) struct AcousticOcclusionAccumulator {
    emitter_pos: Vec3,
    listener_head_pos: Vec3,
    delta: Vec3,
    ray_len: f32,
    padded_origin: Vec3,
    padded_target: Vec3,
    padded_len: f32,
    upper_y: f32,
    lower_y: f32,
    check_slabs: bool,
    barrier_dists: [f32; 8],
    barrier_weights: [f32; 8],
    barrier_is_slab: [bool; 8],
    barrier_count: usize,
    closest_t: f32,
    primary_material: u32,
}

impl AcousticOcclusionAccumulator {
    const MAX_TRACKED_BARRIERS: usize = 8;
    const WALL_Y_EPSILON: f32 = 20.0;
    const ENDPOINT_T_MARGIN: f32 = 0.015;
    const SLAB_RAY_PAD: f32 = 20.0;

    #[inline]
    pub(crate) fn new(emitter_pos: Vec3, listener_head_pos: Vec3) -> Option<Self> {
        if !emitter_pos.is_finite() || !listener_head_pos.is_finite() {
            return None;
        }
        let delta = listener_head_pos - emitter_pos;
        let ray_len = delta.length();
        if !ray_len.is_finite() || ray_len <= 10.0 {
            return None;
        }
        let check_slabs = delta.y.abs() > 450.0;
        let (padded_origin, padded_target, padded_len) = if check_slabs {
            let ray_dir_unit = delta / ray_len;
            (
                emitter_pos - ray_dir_unit * Self::SLAB_RAY_PAD,
                listener_head_pos + ray_dir_unit * Self::SLAB_RAY_PAD,
                ray_len + 2.0 * Self::SLAB_RAY_PAD,
            )
        } else {
            (emitter_pos, listener_head_pos, ray_len)
        };
        Some(Self {
            emitter_pos,
            listener_head_pos,
            delta,
            ray_len,
            padded_origin,
            padded_target,
            padded_len,
            upper_y: emitter_pos.y.min(listener_head_pos.y),
            lower_y: emitter_pos.y.max(listener_head_pos.y),
            check_slabs,
            barrier_dists: [0.0; Self::MAX_TRACKED_BARRIERS],
            barrier_weights: [0.0; Self::MAX_TRACKED_BARRIERS],
            barrier_is_slab: [false; Self::MAX_TRACKED_BARRIERS],
            barrier_count: 0,
            closest_t: 1.0,
            primary_material: 0,
        })
    }

    #[inline]
    pub(crate) fn checks_slabs(&self) -> bool {
        self.check_slabs
    }

    #[inline]
    pub(crate) fn padded_endpoints(&self) -> (Vec3, Vec3) {
        (self.padded_origin, self.padded_target)
    }

    #[inline]
    pub(crate) fn test_wall(&mut self, w: &WallSegment) {
        if w.group == 3 || is_cld_material_thru(w.material) {
            return;
        }
        if let Some(t) = intersect_segment_xz_t(
            self.emitter_pos.x,
            self.emitter_pos.z,
            self.listener_head_pos.x,
            self.listener_head_pos.z,
            w,
        ) {
            if !t.is_finite()
                || !(Self::ENDPOINT_T_MARGIN..=(1.0 - Self::ENDPOINT_T_MARGIN)).contains(&t)
            {
                return;
            }
            let y_at_hit = self.emitter_pos.y + self.delta.y * t;
            if y_at_hit < w.y_top - Self::WALL_Y_EPSILON
                || y_at_hit > w.y_bottom + Self::WALL_Y_EPSILON
            {
                return;
            }

            let hit_dist = t * self.ray_len;
            let weight = cld_material_barrier_weight(w.material);
            if t < self.closest_t {
                self.closest_t = t;
                self.primary_material = w.material;
            }

            if let Some(existing_idx) = self.barrier_dists[..self.barrier_count]
                .iter()
                .position(|&d| (d - hit_dist).abs() < MIN_DISTINCT_BARRIER_SEPARATION_UNITS)
            {
                if weight > self.barrier_weights[existing_idx] {
                    self.barrier_weights[existing_idx] = weight;
                }
            } else if self.barrier_count < Self::MAX_TRACKED_BARRIERS {
                self.barrier_dists[self.barrier_count] = hit_dist;
                self.barrier_weights[self.barrier_count] = weight;
                self.barrier_is_slab[self.barrier_count] = false;
                self.barrier_count += 1;
            }
        }
    }

    #[inline]
    pub(crate) fn test_floor(&mut self, tri: &FloorTriangle) {
        if !self.check_slabs || is_cld_material_thru(tri.material) {
            return;
        }
        if let Some(t_padded) =
            intersect_ray_triangle_3d(self.padded_origin, self.padded_target, tri)
        {
            if !t_padded.is_finite() {
                return;
            }
            let hit_pt = self.padded_origin + (self.padded_target - self.padded_origin) * t_padded;
            let y_at_hit = hit_pt.y;
            if self.upper_y > y_at_hit + 25.0 || self.lower_y < y_at_hit + 350.0 {
                return;
            }

            let unpadded_dist =
                (t_padded * self.padded_len - Self::SLAB_RAY_PAD).clamp(0.0, self.ray_len);
            let t = (unpadded_dist / self.ray_len).clamp(0.0, 1.0);
            let weight = cld_material_barrier_weight(tri.material);
            if t < self.closest_t {
                self.closest_t = t;
                self.primary_material = tri.material;
            }

            if let Some(existing_idx) = self.barrier_dists[..self.barrier_count]
                .iter()
                .position(|&d| (d - unpadded_dist).abs() < MIN_DISTINCT_BARRIER_SEPARATION_UNITS)
            {
                if weight > self.barrier_weights[existing_idx] {
                    self.barrier_weights[existing_idx] = weight;
                }
            } else if self.barrier_count < Self::MAX_TRACKED_BARRIERS {
                self.barrier_dists[self.barrier_count] = unpadded_dist;
                self.barrier_weights[self.barrier_count] = weight;
                self.barrier_is_slab[self.barrier_count] = true;
                self.barrier_count += 1;
            }
        }
    }

    #[inline]
    pub(crate) fn finish(self) -> AcousticOcclusionHit {
        let mut wall_hit_count = 0u32;
        let mut slab_hit_count = 0u32;
        let mut barrier_density = 0.0f32;
        for i in 0..self.barrier_count {
            if self.barrier_is_slab[i] {
                slab_hit_count += 1;
            } else {
                wall_hit_count += 1;
            }
            barrier_density += self.barrier_weights[i];
        }
        AcousticOcclusionHit {
            wall_hit_count,
            slab_hit_count,
            closest_t: self.closest_t,
            primary_material: self.primary_material,
            barrier_density,
        }
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
    let Some(mut acc) = AcousticOcclusionAccumulator::new(emitter_pos, listener_head_pos) else {
        return AcousticOcclusionHit::clear();
    };
    for w in walls {
        acc.test_wall(w);
    }
    if acc.checks_slabs() {
        for tri in floors {
            acc.test_floor(tri);
        }
    }
    acc.finish()
}
