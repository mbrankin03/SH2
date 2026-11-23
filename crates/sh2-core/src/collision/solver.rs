//! Exact 1:1 PS2/PC collision math and multi-character solver (`Collision/cl_main.c` & `cl_calc2.c`).
//!
//! Ground-truth symbols implemented verbatim from `SLUS_202.28` and cross-checked against `sh2pc.exe`:
//! - `clCheckSubWallToColumn` (`0x00131d70` / `sh2pc 0x00529830`)
//! - `clCheckSubColumnToColumn` (`0x00131f90` / `sh2pc 0x00529c50`)
//! - `clCheckColumn2WallHit` (`0x00134cc0` / `sh2pc 0x00522f00`)
//! - `clCheckColumn2ColumnHit` (`0x00134f60` / `sh2pc 0x00523190`)
//! - `clAddWallCollectVector` (`0x00134b20` / `sh2pc 0x00522de0`)
//! - `clAddCollectVector` (`0x001337b0` / `sh2pc 0x00522cf0`)
//! - `clMakeWallHitCollectVector` (`0x001341a0` / `sh2pc 0x005254f0`)
//! - `clCollectCharaALL` (`0x00132e20` / `sh2pc 0x00525f50`)

use std::f32::consts::PI;

use glam::Vec3;
use sh2_assets::formats::cld::{HitPolyColumn, HitPolyPlane};

use super::character::CollisionColumn;

/// Maximum wall/column hits collected per character per BG check (`clWallHitData[32]`, `0x00133e60`).
pub const CL_MAX_WALL_HITS: usize = 32;

/// Number of interleaved relaxation iterations in `clCollectCharaALL` (`0x00132e20`: `if (4 < iVar7) return;`).
pub const CL_COLLECT_CHARA_ITERATIONS: usize = 5;

/// Discrete weight-difference pushback table (`SLUS_202.28` `0x002a9960` / `sh2pc.exe` `0x00525f50`).
///
/// Indexed by `0` when `weight_a < weight_b`, else `(weight_a - weight_b + 1).min(4)`.
/// Recoil applied to the other actor is `-(1.0 - CL_WEIGHT_DIFF_TABLE[idx]) * push`.
pub const CL_WEIGHT_DIFF_TABLE: [f32; 5] = [1.0, 0.98, 0.94, 0.90, 0.86];

/// Corner-vs-face suppression angle threshold (`89.0 deg` = `1.553343` rad, `clMakeWallHitCollectVector` `0x001341a0`).
pub const CL_CORNER_SUPPRESS_RAD: f32 = 1.553_343;

/// Trailing-normal suppression angle threshold (`75.0 deg` = `1.3089969` rad, `clMakeWallHitCollectVector` `0x001341a0`).
pub const CL_TRAILING_NORMAL_SUPPRESS_RAD: f32 = 1.308_996_9;

/// Parallel corner-vs-face suppression angle threshold (`2.0 deg` = `0.034906585` rad, `clMakeWallHitCollectVector` `0x001341a0`).
pub const CL_PARALLEL_CORNER_SUPPRESS_RAD: f32 = 0.034_906_585;

/// Post-flag velocity clamp normal angle threshold (`90.0 deg` = `1.5707964` rad, `clMakeWallHitCollectVector` `0x001341a0`).
pub const CL_POST_FLAG_CLAMP_RAD: f32 = 1.570_796_4;

/// Post-flag pushback speed cap numerator (`500.0 / fps` mm/frame, `clMakeWallHitCollectVector` `0x001341a0`).
pub const CL_POST_FLAG_CLAMP_SPEED: f32 = 500.0;

/// Opposing-wall pinch suppression angle threshold (`120.0 deg` = `2.0943952` rad, `clMakeWallHitCollectVector` `0x001341a0`).
pub const CL_PINCH_SUPPRESS_RAD: f32 = 2.094_395_2;

/// Sentinel movement angle when `(dx, dz) == (0.0, 0.0)` (`0x7f7fffff` = `f32::MAX`, `clCheckBg2Chara` `0x00133920`).
pub const CL_NO_MOVE_ANGLE: f32 = f32::MAX;

/// Hit classification stored in `clWallHitData[i].kind` (`0x00134cc0` / `0x00134f60`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WallHitKind {
    /// Face contact against a wall plane (`kind = 1`).
    Face = 1,
    /// Endpoint/corner contact against a wall plane (`kind = 2`).
    Corner = 2,
    /// Static BG cylinder column contact (`kind = 3`, `clCheckColumn2ColumnHit`).
    BgColumn = 3,
    /// Suppressed by corner/trailing/parallel rules (`kind = -1`).
    Suppressed = -1,
    /// Suppressed by opposing-wall 120-degree pinch rule (`kind = -2`).
    PinchSuppressed = -2,
}

/// Single collected wall or static BG column hit entry (`clWallHitData[i]`, `cl_main.c`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallHitRecord {
    /// Classification (`Face = 1`, `Corner = 2`, `BgColumn = 3`, or suppressed `< 0`).
    pub kind: WallHitKind,
    /// Pushback correction vector in XZ (`y = 0.0`, world mm).
    pub collect_vec: Vec3,
    /// Plane equation `(nx, ny, nz, d)` for wall hits (`None` for `BgColumn`).
    pub plane_eq: Option<[f32; 4]>,
    /// Normalized plane XZ azimuth angle (`shAtan2(nx, nz)`), populated by `clMakeWallHitCollectVector`.
    pub normal_angle: f32,
}

impl WallHitRecord {
    /// Zero-initialized placeholder for stack-allocated `[WallHitRecord; CL_MAX_WALL_HITS]` buffers.
    pub const EMPTY: Self = Self {
        kind: WallHitKind::Suppressed,
        collect_vec: Vec3::ZERO,
        plane_eq: None,
        normal_angle: 0.0,
    };
}

/// Exact `shAngleRegulate` helper: normalizes an angle in radians to `[-PI, +PI]`.
#[inline]
pub fn regulate_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

/// Exact `shAtan2(x, z)` matching PS2/PC SH2 yaw convention (`0` = `+Z`, `+PI/2` = `+X`).
#[inline]
pub fn sh_atan2(x: f32, z: f32) -> f32 {
    x.atan2(z)
}

/// Exact `1/16` fixed-point quantization applied at the end of `clCheckBg2Chara`
/// (`SLUS_202.28` `0x00133920`: `_vftoi4` -> `_vitof4`).
#[inline]
pub fn quantize_collect_vector_1_16(v: Vec3) -> Vec3 {
    Vec3::new(
        (v.x * 16.0).trunc() / 16.0,
        (v.y * 16.0).trunc() / 16.0,
        (v.z * 16.0).trunc() / 16.0,
    )
}

/// Exact 1:1 `clCheckSubWallToColumn` (`SLUS_202.28` `0x00131d70` / `sh2pc.exe` `0x00529830`).
///
/// Tests vertical overlap and 2D XZ cylinder-to-segment intersection between the wall
/// endpoints `p0` (`plane.p[0]`) and `p2` (`plane.p[2]`) and `col`.
/// Returns `Some(push_xz)` on hit.
pub fn cl_check_sub_wall_to_column(
    p0: [f32; 4],
    p2: [f32; 4],
    col: &CollisionColumn,
) -> Option<Vec3> {
    let x_max = p0[0].max(p2[0]);
    let y_max = p0[1].max(p2[1]);
    let z_max = p0[2].max(p2[2]);
    let x_min = p0[0].min(p2[0]);
    let y_min = p0[1].min(p2[1]);
    let z_min = p0[2].min(p2[2]);

    let cx = col.center_x;
    let cz = col.center_z;
    let r = col.radius;

    // 3D AABB overlap test (`sh2pc.exe` `0x00529830` lines 5493-5495)
    if z_max < cz - r
        || y_max < col.y_top
        || x_max < cx - r
        || (cz + r) < z_min
        || (cx + r) < x_min
        || col.y_bottom < y_min
    {
        return None;
    }

    let dx = p2[0] - p0[0];
    let dz = p2[2] - p0[2];
    let len_sq = dx * dx + dz * dz;
    if len_sq == 0.0 {
        return None;
    }

    let rel0_x = cx - p0[0];
    let rel0_z = cz - p0[2];
    let r_sq = r * r;

    let t = (rel0_x * dx + rel0_z * dz) / len_sq;
    let ox = cx - (p0[0] + dx * t);
    let oz = cz - (p0[2] + dz * t);
    let perp_dist_sq = ox * ox + oz * oz;

    if perp_dist_sq > r_sq {
        return None;
    }

    let d0_sq = rel0_x * rel0_x + rel0_z * rel0_z;
    let d2_sq = (cx - p2[0]) * (cx - p2[0]) + (cz - p2[2]) * (cz - p2[2]);
    let within_endpoints = d0_sq <= r_sq || d2_sq <= r_sq || (0.0..=1.0).contains(&t);
    if !within_endpoints {
        return None;
    }

    if perp_dist_sq != 0.0 {
        let dist = perp_dist_sq.sqrt();
        let scale = (r - dist) / dist;
        Some(Vec3::new(ox * scale, 0.0, oz * scale))
    } else {
        Some(Vec3::ZERO)
    }
}

/// Exact 1:1 `clCheckSubColumnToColumn` (`SLUS_202.28` `0x00131f90` / `sh2pc.exe` `0x00529c50`).
///
/// Computes the pushout vector applied to `target` (`param_3`) to separate it from `source` (`param_2`).
/// Includes the retail `+1.0` mm separation margin (`((r1 + r2 - dist) + 1.0) / dist`).
pub fn cl_check_sub_column_to_column(
    source: &CollisionColumn,
    target: &CollisionColumn,
) -> Option<Vec3> {
    if !source.active || !target.active {
        return None;
    }

    let r1 = source.radius;
    let r2 = target.radius;

    // 3D AABB overlap test (`sh2pc.exe` `0x00529c50` lines 5554-5556)
    if target.y_bottom < source.y_top
        || source.y_bottom < target.y_top
        || (r2 + source.center_z) < (target.center_z - r1)
        || (r2 + source.center_x) < (target.center_x - r1)
        || (r1 + source.center_x) < (target.center_x - r2)
        || (r1 + source.center_z) < (target.center_z - r2)
    {
        return None;
    }

    let dx = target.center_x - source.center_x;
    let dz = target.center_z - source.center_z;
    let dist_sq = dx * dx + dz * dz;
    let dist = dist_sq.sqrt();
    let pen = (r1 + r2) - dist;

    if pen < 0.0 {
        return None;
    }

    if dist > 1e-6 {
        let scale = (pen + 1.0) / dist;
        Some(Vec3::new(dx * scale, 0.0, dz * scale))
    } else {
        // Guard against exact concentric NaN division
        Some(Vec3::new(pen + 1.0, 0.0, 0.0))
    }
}

/// Exact 1:1 `clCheckColumn2WallHit` (`SLUS_202.28` `0x00134cc0` / `sh2pc.exe` `0x00522f00`).
///
/// Performs:
/// 1. One-sided plane backface culling (`dot(col_pos - p[2], normal) >= 0.0`).
/// 2. Cylinder-to-wall intersection via `clCheckSubWallToColumn`.
/// 3. Face (`kind = 1`) vs corner (`kind = 2`) classification; corner hits recompute pushback
///    from the closer endpoint `(p[0]` or `p[2])` with `+1.0` mm margin.
pub fn cl_check_column_to_wall_hit(
    plane: &HitPolyPlane,
    col: &CollisionColumn,
) -> Option<WallHitRecord> {
    if plane.kind == 0 || !col.active {
        return None;
    }

    let p0 = plane.p[0];
    let p2 = plane.p[2];
    let rel2_x = col.center_x - p2[0];
    let rel2_y = col.y_bottom - p2[1];
    let rel2_z = col.center_z - p2[2];

    let eq = plane.plane_equation();
    let side = rel2_x * eq[0] + rel2_y * eq[1] + rel2_z * eq[2];
    if side < 0.0 {
        return None;
    }

    let sub_push = cl_check_sub_wall_to_column(p0, p2, col)?;

    // Face vs corner classification (`0x00134cc0` lines 1072-1092)
    let f1 = rel2_x * (p0[0] - p2[0]) + rel2_z * (p0[2] - p2[2]);
    let f2 = (col.center_x - p0[0]) * (p2[0] - p0[0]) + (col.center_z - p0[2]) * (p2[2] - p0[2]);

    if (f1 < 0.0 && f2 >= 0.0) || (f1 >= 0.0 && f2 < 0.0) {
        let d0_x = col.center_x - p0[0];
        let d0_z = col.center_z - p0[2];
        let d0_sq = d0_x * d0_x + d0_z * d0_z;

        let d2_x = col.center_x - p2[0];
        let d2_z = col.center_z - p2[2];
        let d2_sq = d2_x * d2_x + d2_z * d2_z;

        let (vx, vz, dist_sq) = if d2_sq <= d0_sq {
            (d2_x, d2_z, d2_sq)
        } else {
            (d0_x, d0_z, d0_sq)
        };

        let push = if dist_sq > 1e-6 {
            let dist = dist_sq.sqrt();
            let scale = ((col.radius - dist) + 1.0) / dist;
            Vec3::new(vx * scale, 0.0, vz * scale)
        } else {
            sub_push
        };

        Some(WallHitRecord {
            kind: WallHitKind::Corner,
            collect_vec: push,
            plane_eq: Some(eq),
            normal_angle: 0.0,
        })
    } else {
        Some(WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: sub_push,
            plane_eq: Some(eq),
            normal_angle: 0.0,
        })
    }
}

/// Exact 1:1 `clCheckColumn2ColumnHit` (`SLUS_202.28` `0x00134f60` / `sh2pc.exe` `0x00523190`)
/// against a static BG column (`_CL_HITPOLY_COLUMN` from `.cld`).
pub fn cl_check_column_to_bg_column_hit(
    bg_col: &HitPolyColumn,
    chara_col: &CollisionColumn,
) -> Option<WallHitRecord> {
    if !bg_col.is_active() || !chara_col.active {
        return None;
    }
    let static_col = CollisionColumn {
        center_x: bg_col.center_x,
        y_bottom: bg_col.y_bottom,
        center_z: bg_col.center_z,
        y_top: bg_col.y_top,
        radius: bg_col.radius,
        weight: bg_col.weight as i32,
        active: true,
    };
    let push = cl_check_sub_column_to_column(&static_col, chara_col)?;
    Some(WallHitRecord {
        kind: WallHitKind::BgColumn,
        collect_vec: Vec3::new(push.x, 0.0, push.z),
        plane_eq: None,
        normal_angle: 0.0,
    })
}

/// Exact 1:1 `clAddCollectVector` (`SLUS_202.28` `0x001337b0` / `sh2pc.exe` `0x00522cf0`).
///
/// Accumulates `add` into `acc` by clamping each XZ axis of `acc + add` to
/// `[min(acc, add), max(acc, add)]`.
#[inline]
pub fn cl_add_collect_vector(acc: &mut Vec3, add: Vec3) {
    let sum_x = acc.x + add.x;
    let clamped_x = sum_x.clamp(acc.x.min(add.x), acc.x.max(add.x));

    let sum_z = acc.z + add.z;
    let clamped_z = sum_z.clamp(acc.z.min(add.z), acc.z.max(add.z));

    acc.x = clamped_x;
    acc.y += add.y;
    acc.z = clamped_z;
}

/// Exact 1:1 `clAddWallCollectVector` (`SLUS_202.28` `0x00134b20` / `sh2pc.exe` `0x00522de0`).
///
/// First hit copies directly into `acc`; subsequent hits accumulate via the clamped
/// `[min(acc, add), max(acc, add)]` rule on X and Z.
#[inline]
pub fn cl_add_wall_collect_vector(acc: &mut Vec3, add: Vec3, hit_count: &mut usize) {
    if *hit_count == 0 {
        *acc = add;
    } else {
        cl_add_collect_vector(acc, add);
    }
    *hit_count += 1;
}

/// Context passed to `clMakeWallHitCollectVector` (`0x001341a0`).
#[derive(Debug, Clone, Copy)]
pub struct WallCollectContext {
    /// Movement direction angle (`shAtan2(dx, dz)` or `CL_NO_MOVE_ANGLE` when stationary).
    pub move_angle: f32,
    /// Character current position (`chara + 0x20`).
    pub cur_pos: Vec3,
    /// Character previous position (`chara + 0xe0`).
    pub prev_pos: Vec3,
    /// Character type (`chara + 0x170`; `1` or `2` enables 120-deg pinch suppression).
    pub chara_type: i32,
    /// Post-flag counter (`chara + 0x104` / `sh2pc +0xf0`) for `500.0 / fps` push clamp.
    pub post_flag_timer: i32,
    /// Current simulation FPS (30.0 or 60.0) for `500.0 / fps` clamp.
    pub fps: f32,
}

impl Default for WallCollectContext {
    fn default() -> Self {
        Self {
            move_angle: CL_NO_MOVE_ANGLE,
            cur_pos: Vec3::ZERO,
            prev_pos: Vec3::ZERO,
            chara_type: 1,
            post_flag_timer: 0,
            fps: 60.0,
        }
    }
}

/// Exact 1:1 `clMakeWallHitCollectVector` (`SLUS_202.28` `0x001341a0` / `sh2pc.exe` `0x005254f0`).
///
/// Processes collected `hits` (walls + static BG columns) and returns `(collect_vec, applied_count)`:
/// - Single hit: passes straight through `clAddWallCollectVector`.
/// - Multi-hit:
///   1. Normalizes wall plane normals and computes `normal_angle = shAtan2(nx, nz)`.
///   2. Suppresses corner hits (`kind == 2`) within `89 deg` (`1.553343` rad) of any face hit's
///      adjusted motion direction.
///   3. Suppresses non-BG-column hits whose normal is within `75 deg` (`1.3089969` rad) of `move_angle`.
///   4. Suppresses corner hits (`kind == 2`) within `2 deg` (`0.034906585` rad) of a parallel face hit.
///   5. If `post_flag_timer != 0`, clamps pushback magnitude to `500.0 / fps` for normals within `90 deg`.
///   6. For `chara_type == 1 || 2`, suppresses pairs of opposing push vectors separated by `> 120 deg`
///      (`2.0943952` rad) (`kind = PinchSuppressed`).
///   7. Accumulates surviving hits via `clAddWallCollectVector`.
pub fn cl_make_wall_hit_collect_vector(
    hits: &mut [WallHitRecord],
    ctx: &mut WallCollectContext,
) -> (Vec3, usize) {
    let mut collect_vec = Vec3::ZERO;
    let mut applied_count = 0usize;
    let count = hits.len();

    if count == 0 {
        return (collect_vec, 0);
    }

    if count == 1 {
        cl_add_wall_collect_vector(&mut collect_vec, hits[0].collect_vec, &mut applied_count);
        return (collect_vec, applied_count);
    }

    // Pass 1: normalize plane normals, compute normal_angle, and apply 89-deg / 75-deg suppression
    for i in 0..count {
        if hits[i].kind != WallHitKind::BgColumn {
            if let Some(eq) = hits[i].plane_eq {
                let len = (eq[0] * eq[0] + eq[1] * eq[1] + eq[2] * eq[2]).sqrt();
                let inv = if len > 0.0 && (1.0 / len) <= f32::MAX {
                    1.0 / len
                } else {
                    1.0
                };
                let nx = eq[0] * inv;
                let nz = eq[2] * inv;
                hits[i].normal_angle = sh_atan2(nx, nz);
            }

            if ctx.move_angle != CL_NO_MOVE_ANGLE {
                if hits[i].kind == WallHitKind::Corner {
                    let norm_ang = hits[i].normal_angle;
                    for j in 0..count {
                        if i != j && hits[j].kind == WallHitKind::Face {
                            let adj = (hits[j].collect_vec + ctx.cur_pos) - ctx.prev_pos;
                            let adj_ang = sh_atan2(adj.x, adj.z);
                            let diff = regulate_angle(norm_ang - adj_ang);
                            if diff.abs() < CL_CORNER_SUPPRESS_RAD {
                                hits[i].kind = WallHitKind::Suppressed;
                                break;
                            }
                        }
                    }
                }

                if hits[i].kind != WallHitKind::Suppressed {
                    let diff = regulate_angle(hits[i].normal_angle - ctx.move_angle);
                    if diff.abs() < CL_TRAILING_NORMAL_SUPPRESS_RAD {
                        hits[i].kind = WallHitKind::Suppressed;
                    }
                }
            }
        }
    }

    // Pass 2: suppress corner hits parallel (within 2 deg) to any face hit
    for i in 0..count {
        if hits[i].kind == WallHitKind::Corner {
            let ang_i = hits[i].normal_angle;
            for j in 0..count {
                if i != j && hits[j].kind == WallHitKind::Face {
                    let diff = regulate_angle(hits[j].normal_angle - ang_i);
                    if diff.abs() < CL_PARALLEL_CORNER_SUPPRESS_RAD {
                        hits[i].kind = WallHitKind::Suppressed;
                        break;
                    }
                }
            }
        }
    }

    // Pass 3: post-flag 500.0/fps pushback speed clamp within 90 deg of move_angle
    if ctx.post_flag_timer != 0 {
        let max_push = CL_POST_FLAG_CLAMP_SPEED / ctx.fps.max(1.0);
        for hit in hits.iter_mut() {
            if hit.kind != WallHitKind::Suppressed {
                let diff = regulate_angle(hit.normal_angle - ctx.move_angle);
                if diff.abs() < CL_POST_FLAG_CLAMP_RAD {
                    let mag = (hit.collect_vec.x * hit.collect_vec.x
                        + hit.collect_vec.z * hit.collect_vec.z)
                        .sqrt();
                    if mag > max_push && mag > 0.0 {
                        let scale = max_push / mag;
                        hit.collect_vec *= scale;
                    }
                }
            }
        }
        ctx.post_flag_timer -= 1;
    }

    // Pass 4: 120-degree opposing-wall pinch suppression for chara_type 1 or 2
    if ctx.chara_type == 1 || ctx.chara_type == 2 {
        for i in 0..count.saturating_sub(1) {
            if hits[i].kind == WallHitKind::Suppressed {
                continue;
            }
            for j in (i + 1)..count {
                if hits[j].kind == WallHitKind::Suppressed {
                    continue;
                }
                let vi = hits[i].collect_vec;
                let vj = hits[j].collect_vec;
                if (vi.x != 0.0 || vi.z != 0.0) && (vj.x != 0.0 || vj.z != 0.0) {
                    let ang_i = sh_atan2(vi.x, vi.z);
                    let ang_j = sh_atan2(vj.x, vj.z);
                    let diff = regulate_angle(ang_i - ang_j);
                    if diff.abs() > CL_PINCH_SUPPRESS_RAD {
                        hits[i].kind = WallHitKind::PinchSuppressed;
                        hits[j].kind = WallHitKind::PinchSuppressed;
                    }
                }
            }
        }
    }

    // Pass 5: accumulate surviving hits (`kind >= 0`)
    for hit in hits.iter() {
        if (hit.kind as i32) >= 0 {
            cl_add_wall_collect_vector(&mut collect_vec, hit.collect_vec, &mut applied_count);
        }
    }

    (collect_vec, applied_count)
}

/// Entry for a single character participating in `clCollectCharaALL` (`0x00132e20`).
#[derive(Debug, Clone, Copy)]
pub struct CharaCollectEntry {
    /// Active movement cylinder (`move_col`).
    pub col: CollisionColumn,
    /// Pending frame displacement `(cur_pos - prev_pos)` in XZ.
    pub move_vec: Vec3,
    /// Accumulated recoil pushback from other characters (`0x48e070`).
    pub push_vec: Vec3,
    /// Previous position before this frame (`chara + 0xe0`).
    pub prev_pos: Vec3,
    /// Character type (`1` = player, `2` = enemy/NPC).
    pub chara_type: i32,
}

impl CharaCollectEntry {
    /// Zero-initialized placeholder for stack-allocated `clCharaList[32]` buffers.
    pub const EMPTY: Self = Self {
        col: CollisionColumn {
            center_x: 0.0,
            y_bottom: 0.0,
            center_z: 0.0,
            y_top: 0.0,
            radius: 0.0,
            weight: 0,
            active: false,
        },
        move_vec: Vec3::ZERO,
        push_vec: Vec3::ZERO,
        prev_pos: Vec3::ZERO,
        chara_type: 1,
    };
}

/// Computes the discrete weight-diff recoil factor `(1.0 - CL_WEIGHT_DIFF_TABLE[idx])`
/// from `clCollectCharaALL` (`SLUS_202.28` `0x00132e20` lines 488-499).
#[inline]
pub fn weight_diff_recoil_factor(weight_a: i32, weight_b: i32) -> f32 {
    let diff = weight_a - weight_b;
    let idx = if diff < 0 {
        0
    } else {
        ((diff + 1) as usize).min(CL_WEIGHT_DIFF_TABLE.len() - 1)
    };
    1.0 - CL_WEIGHT_DIFF_TABLE[idx]
}

/// Selects the per-axis displacement between `move_vec` and `push_vec` matching
/// `clCollectCharaALL` (`0x00132e20` lines 384-419).
#[inline]
fn select_axis_step(move_axis: f32, push_axis: f32) -> f32 {
    if push_axis == 0.0 {
        move_axis
    } else if push_axis * move_axis < 0.0 || move_axis.abs() < push_axis.abs() {
        push_axis
    } else {
        move_axis
    }
}

/// Exact 1:1 `clCollectCharaALL` (`SLUS_202.28` `0x00132e20` / `sh2pc.exe` `0x00525f50`).
///
/// Runs up to **5 interleaved iterations** (`CL_COLLECT_CHARA_ITERATIONS = 5`), breaking early
/// when an iteration produces zero character-to-character overlaps.
///
/// `bg_check` is invoked twice per active character per iteration (before and after character-to-character
/// separation) and returns the quantized BG wall/column collect vector (`clCheckBg2Chara`).
pub fn cl_collect_chara_all<F>(entries: &mut [CharaCollectEntry], mut bg_check: F) -> usize
where
    F: FnMut(&CharaCollectEntry) -> Vec3,
{
    let n = entries.len();
    if n == 0 {
        return 0;
    }

    let mut iterations_run = 0usize;

    for _iter in 0..CL_COLLECT_CHARA_ITERATIONS {
        iterations_run += 1;
        let mut chara_overlap_count = 0usize;

        for i in 0..n {
            // 1. Apply pending move_vec / push_vec along X and Z
            let step_x = select_axis_step(entries[i].move_vec.x, entries[i].push_vec.x);
            let step_z = select_axis_step(entries[i].move_vec.z, entries[i].push_vec.z);
            entries[i].col.center_x += step_x;
            entries[i].col.center_z += step_z;

            // Clamp move_vec magnitude to <= col.radius (0x00132e20 lines 427-436)
            let dx = entries[i].col.center_x - entries[i].prev_pos.x;
            let dz = entries[i].col.center_z - entries[i].prev_pos.z;
            let move_len = (dx * dx + dz * dz).sqrt();
            entries[i].move_vec = Vec3::new(dx, 0.0, dz);
            if move_len > 0.0 {
                let ratio = entries[i].col.radius / move_len;
                if ratio < 1.0 {
                    entries[i].move_vec *= ratio;
                }
            }

            // 2. First BG check (walls + side walls + static BG columns + dynamic walls)
            if entries[i].col.active {
                let bg_push = bg_check(&entries[i]);
                entries[i].col.center_x += bg_push.x;
                entries[i].col.center_z += bg_push.z;
            }

            // Clear i's incoming push_vec before char-char pass
            entries[i].push_vec = Vec3::ZERO;

            // 3. Character-to-character column separation (`clCheckSubColumnToColumn`)
            if entries[i].col.active {
                for j in 0..n {
                    if i == j || !entries[j].col.active {
                        continue;
                    }
                    if let Some(mut push) =
                        cl_check_sub_column_to_column(&entries[j].col, &entries[i].col)
                    {
                        push.y = 0.0;
                        // Retail PC (`sh2pc.exe` 0x00525f50 lines 3709-3716) clamps push to <= radius_i
                        let push_len = (push.x * push.x + push.z * push.z).sqrt();
                        if push_len > entries[i].col.radius && push_len > 0.0 {
                            let s = entries[i].col.radius / push_len;
                            push.x *= s;
                            push.z *= s;
                        }

                        entries[i].col.center_x += push.x;
                        entries[i].col.center_z += push.z;

                        // Discrete weight-diff recoil applied to j's push_vec
                        let recoil_factor =
                            weight_diff_recoil_factor(entries[i].col.weight, entries[j].col.weight);
                        let mut recoil =
                            Vec3::new(-push.x * recoil_factor, 0.0, -push.z * recoil_factor);
                        let recoil_len = (recoil.x * recoil.x + recoil.z * recoil.z).sqrt();
                        if recoil_len > entries[i].col.radius && recoil_len > 0.0 {
                            let s = entries[i].col.radius / recoil_len;
                            recoil.x *= s;
                            recoil.z *= s;
                        }
                        cl_add_collect_vector(&mut entries[j].push_vec, recoil);
                        chara_overlap_count += 1;
                    }
                }
            }

            // Clear i's move_vec after char-char pass (0x00132e20 lines 525-528)
            entries[i].move_vec = Vec3::ZERO;

            // 4. Second BG check so char-char pushes cannot shove i through a wall
            if entries[i].col.active {
                let bg_push = bg_check(&entries[i]);
                entries[i].col.center_x += bg_push.x;
                entries[i].col.center_z += bg_push.z;
            }
        }

        if chara_overlap_count == 0 {
            break;
        }
    }

    iterations_run
}
