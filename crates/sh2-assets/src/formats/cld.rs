//! Parser and data structures for Silent Hill 2 `.cld` collision files.
//!
//! # Ground Truth & Format Specification
//! Layout confirmed from PS2 beta symbols (`_CL_CLDHEADER`, `_CL_HITPOLY_PLANE`,
//! `_CL_HITPOLY_COLUMN` in `Collision/cl_main.c`) and PC retail `.cld` binaries:
//!
//! - **Header:**
//!   - `sx`, `sz` (`f32`): World bounds / origin coordinates.
//!   - `b0size`, `b1size`, `b2size`, `b3size`, `csize`, `disable` (`i32`): Byte lengths for polygon groups and columns.
//!   - `b0ofs[16]`, `b1ofs[16]`, `b2ofs[16]`, `b3ofs[16]`, `clofs[16]` (`i32`): Spatial 4x4 grid cell index lists (-1 terminated).
//!   - `fldofs`, `wldofs`, `cedofs`, `swdofs`, `cldofs` (`u32`): Offsets to polygon/column arrays.
//! - **Hitpoly Plane (80 bytes):**
//!   - `kind` (`u8`), `shape` (`u8`), `pad` (`u16`), `weight` (`u32`), `material` (`u32`), `flg` (`i32`)
//!   - `p[4][4]` (`f32`): 4 vertices $(X, Y, Z, W)$ representing a 3D floor quad/triangle or wall polygon.
//!     World coordinates use $Y$ pointing downwards.
//! - **Hitpoly Column (48 bytes):**
//!   - `kind` (`u8`), `shape` (`u8`), `pad` (`u16`), `weight` (`u32`), `material` (`u32`), `flg` (`i32`)
//!   - `p[4][2]` (`f32`): 4 points $(X, Z)$ defining column footprint.

use crate::error::AssetError;

/// A 3D floor collision triangle extracted from `.cld` group 0 (`fld`).
/// Coordinates are in Silent Hill 2 world units with $+Y$ pointing downward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorTriangle {
    /// 3 triangle vertices: `[[x0, y0, z0], [x1, y1, z1], [x2, y2, z2]]`.
    pub v: [[f32; 3]; 3],
    /// Floor material index (footstep sound, wetness, blood splash).
    pub material: u32,
}

impl FloorTriangle {
    /// Create a new floor triangle.
    pub const fn new(v: [[f32; 3]; 3], material: u32) -> Self {
        Self { v, material }
    }

    /// Perform a 2D barycentric projection test in $XZ$ plane.
    /// If $(x, z)$ lies inside the triangle, returns `Some(y)` interpolated height.
    pub fn sample_height(&self, x: f32, z: f32) -> Option<f32> {
        let ax = self.v[0][0];
        let az = self.v[0][2];
        let bx = self.v[1][0];
        let bz = self.v[1][2];
        let cx = self.v[2][0];
        let cz = self.v[2][2];

        let d = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
        if d.abs() < 1e-6 {
            return None;
        }

        let w0 = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / d;
        let w1 = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / d;
        let w2 = 1.0 - w0 - w1;

        const EPSILON: f32 = -1e-4;
        if w0 < EPSILON || w1 < EPSILON || w2 < EPSILON {
            return None;
        }

        let y = w0 * self.v[0][1] + w1 * self.v[1][1] + w2 * self.v[2][1];
        Some(y)
    }
}

/// A 2D wall segment in the $XZ$ plane with vertical height span $[y_{\text{top}}, y_{\text{bottom}}]$.
/// Because $+Y$ points downward in PS2/PC coordinates, $y_{\text{top}} \le y_{\text{bottom}}$.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallSegment {
    /// Segment start point in $XZ$ coordinates $(x_0, z_0)$.
    pub start: [f32; 2],
    /// Segment end point in $XZ$ coordinates $(x_1, z_1)$.
    pub end: [f32; 2],
    /// Top height boundary (lower numeric value because $+Y$ is down).
    pub y_top: f32,
    /// Bottom height boundary (higher numeric value because $+Y$ is down).
    pub y_bottom: f32,
    /// Collision group (0 = floor, 1 = wall, 2 = ceiling, 3 = side wall).
    pub group: u32,
    /// Surface material.
    pub material: u32,
}

impl WallSegment {
    /// Constructs a new wall segment.
    pub const fn new(
        start: [f32; 2],
        end: [f32; 2],
        y_top: f32,
        y_bottom: f32,
        group: u32,
        material: u32,
    ) -> Self {
        Self {
            start,
            end,
            y_top,
            y_bottom,
            group,
            material,
        }
    }

    /// Check if a vertical cylinder of height `h` with feet at `y` overlaps this wall vertically.
    /// Character body spans $y - h$ (head) to $y$ (feet).
    pub fn overlaps_vertical(&self, y: f32, h: f32) -> bool {
        self.y_top < (y - 10.0) && self.y_bottom > (y - h)
    }
}

/// Parsed `.cld` collision file.
#[derive(Debug, Clone, Default)]
pub struct CldFile {
    /// Room origin coordinates $(X, Z)$.
    pub origin: [f32; 2],
    /// Walkable floor triangles.
    pub floors: Vec<FloorTriangle>,
    /// Obstacle wall segments.
    pub walls: Vec<WallSegment>,
}

impl CldFile {
    /// Creates synthetic collision geometry matching the procedural fallback room
    /// (e.g. boiler room perimeter walls and center pillar quad).
    ///
    /// - Dimensions: width `w`, length `l`, ceiling height `h` (downwards $Y \in [-h, 0]$).
    /// - Center pillar: width `pw`, length `pl`.
    pub fn create_synthetic_room(w: f32, l: f32, h: f32, pw: f32, pl: f32) -> Self {
        let half_w = w * 0.5;
        let half_l = l * 0.5;
        let half_pw = pw * 0.5;
        let half_pl = pl * 0.5;

        // Floor quad at Y = 0.0, split into 2 triangles
        let floors = vec![
            FloorTriangle::new(
                [
                    [-half_w, 0.0, -half_l],
                    [half_w, 0.0, -half_l],
                    [half_w, 0.0, half_l],
                ],
                0,
            ),
            FloorTriangle::new(
                [
                    [-half_w, 0.0, -half_l],
                    [half_w, 0.0, half_l],
                    [-half_w, 0.0, half_l],
                ],
                0,
            ),
        ];

        let mut walls = Vec::new();
        let y_top = -h;
        let y_bottom = 100.0; // Extend slightly below floor for solid containment

        // 4 Outer perimeter walls
        // North wall
        walls.push(WallSegment::new(
            [-half_w, -half_l],
            [half_w, -half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));
        // East wall
        walls.push(WallSegment::new(
            [half_w, -half_l],
            [half_w, half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));
        // South wall
        walls.push(WallSegment::new(
            [half_w, half_l],
            [-half_w, half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));
        // West wall
        walls.push(WallSegment::new(
            [-half_w, half_l],
            [-half_w, -half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));

        // Center pillar walls (if dimensions > 0)
        if pw > 0.0 && pl > 0.0 {
            walls.push(WallSegment::new(
                [-half_pw, -half_pl],
                [half_pw, -half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
            walls.push(WallSegment::new(
                [half_pw, -half_pl],
                [half_pw, half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
            walls.push(WallSegment::new(
                [half_pw, half_pl],
                [-half_pw, half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
            walls.push(WallSegment::new(
                [-half_pw, half_pl],
                [-half_pw, -half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
        }

        Self {
            origin: [0.0, 0.0],
            floors,
            walls,
        }
    }
}

/// Parse a `.cld` binary buffer into structured `CldFile` collision geometry.
pub fn parse_cld(data: &[u8]) -> Result<CldFile, AssetError> {
    const MIN_HEADER_SIZE: usize = 32 + 64 * 5 + 20; // 372 bytes
    if data.len() < MIN_HEADER_SIZE {
        return Err(AssetError::BufferUnderflow {
            required: MIN_HEADER_SIZE,
            available: data.len(),
        });
    }

    let sx = f32::from_le_bytes(data[0..4].try_into().unwrap());
    let sz = f32::from_le_bytes(data[4..8].try_into().unwrap());

    let b0size = i32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let b1size = i32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let b2size = i32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
    let b3size = i32::from_le_bytes(data[20..24].try_into().unwrap()) as usize;
    let _csize = i32::from_le_bytes(data[24..28].try_into().unwrap()) as usize;
    let _disable = i32::from_le_bytes(data[28..32].try_into().unwrap());

    // Polygon array offsets at byte 352 (32 + 64 * 5)
    let ofs_base = 32 + 64 * 5;
    let fldofs = u32::from_le_bytes(data[ofs_base..ofs_base + 4].try_into().unwrap()) as usize;
    let wldofs = u32::from_le_bytes(data[ofs_base + 4..ofs_base + 8].try_into().unwrap()) as usize;
    let cedofs = u32::from_le_bytes(data[ofs_base + 8..ofs_base + 12].try_into().unwrap()) as usize;
    let swdofs =
        u32::from_le_bytes(data[ofs_base + 12..ofs_base + 16].try_into().unwrap()) as usize;
    let _cldofs =
        u32::from_le_bytes(data[ofs_base + 16..ofs_base + 20].try_into().unwrap()) as usize;

    let group_info = [
        (0u32, fldofs, b0size),
        (1u32, wldofs, b1size),
        (2u32, cedofs, b2size),
        (3u32, swdofs, b3size),
    ];

    let mut floors = Vec::new();
    let mut walls = Vec::new();

    for &(group, ofs, size) in &group_info {
        if size == 0 {
            continue;
        }
        if ofs + size > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: ofs + size,
                available: data.len(),
            });
        }

        let num_polys = size / 80;
        for i in 0..num_polys {
            let p_ofs = ofs + i * 80;
            let kind = data[p_ofs];
            let shape = data[p_ofs + 1];
            if kind == 0 {
                continue;
            }

            let material = u32::from_le_bytes(data[p_ofs + 8..p_ofs + 12].try_into().unwrap());

            // Read 4 vertices: [f32; 4] each, only take (x, y, z)
            let mut pts = [[0.0f32; 3]; 4];
            for (v_idx, pt) in pts.iter_mut().enumerate() {
                let v_ofs = p_ofs + 16 + v_idx * 16;
                let x = f32::from_le_bytes(data[v_ofs..v_ofs + 4].try_into().unwrap());
                let y = f32::from_le_bytes(data[v_ofs + 4..v_ofs + 8].try_into().unwrap());
                let z = f32::from_le_bytes(data[v_ofs + 8..v_ofs + 12].try_into().unwrap());
                *pt = [x, y, z];
            }

            let vertex_count = if shape == 0 { 3 } else { 4 };

            if group == 0 {
                // Floor polygons (fld): triangulate quads into 2 triangles
                floors.push(FloorTriangle::new([pts[0], pts[1], pts[2]], material));
                if vertex_count == 4 {
                    floors.push(FloorTriangle::new([pts[0], pts[2], pts[3]], material));
                }
            } else {
                // Wall / barrier polygon candidate (groups 1, 2, 3)
                // Compute plane normal to filter out near-horizontal planes:
                let v0 = pts[0];
                let v1 = pts[1];
                let v2 = pts[2];

                let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

                let nx = e1[1] * e2[2] - e1[2] * e2[1];
                let ny = e1[2] * e2[0] - e1[0] * e2[2];
                let nz = e1[0] * e2[1] - e1[1] * e2[0];

                let norm_len = (nx * nx + ny * ny + nz * nz).sqrt();
                if norm_len < 1e-6 || (ny.abs() / norm_len) > 0.5 {
                    // Flat/slanted floor-like polygon in wall group: skip
                    continue;
                }

                // Project points to XZ and find maximum spanning edge
                let mut best_dist_sq = -1.0f32;
                let mut best_pair = (0, 1);
                for a in 0..vertex_count {
                    for b in (a + 1)..vertex_count {
                        let dx = pts[a][0] - pts[b][0];
                        let dz = pts[a][2] - pts[b][2];
                        let d2 = dx * dx + dz * dz;
                        if d2 > best_dist_sq {
                            best_dist_sq = d2;
                            best_pair = (a, b);
                        }
                    }
                }

                if best_dist_sq < 1.0 {
                    continue;
                }

                let mut y_min = pts[0][1];
                let mut y_max = pts[0][1];
                for pt in &pts[1..vertex_count] {
                    if pt[1] < y_min {
                        y_min = pt[1];
                    }
                    if pt[1] > y_max {
                        y_max = pt[1];
                    }
                }

                let p_a = pts[best_pair.0];
                let p_b = pts[best_pair.1];

                walls.push(WallSegment::new(
                    [p_a[0], p_a[2]],
                    [p_b[0], p_b[2]],
                    y_min,
                    y_max,
                    group,
                    material,
                ));
            }
        }
    }

    Ok(CldFile {
        origin: [sx, sz],
        floors,
        walls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_room_collision() {
        let room = CldFile::create_synthetic_room(8000.0, 12000.0, 3000.0, 600.0, 600.0);
        assert_eq!(room.floors.len(), 2);
        // 4 outer walls + 4 pillar walls = 8 walls
        assert_eq!(room.walls.len(), 8);

        // Test floor height sampling at origin
        let y0 = room.floors[0].sample_height(100.0, -100.0);
        assert!(y0.is_some());
        assert!((y0.unwrap() - 0.0).abs() < 1e-4);

        // Outside room should be None
        let y_outside = room.floors[0].sample_height(10000.0, 10000.0);
        assert!(y_outside.is_none());
    }

    #[test]
    fn test_floor_triangle_height_interpolation() {
        let tri = FloorTriangle::new([[0.0, 0.0, 0.0], [100.0, 10.0, 0.0], [0.0, 20.0, 100.0]], 0);

        // Test at vertex 0
        let y0 = tri.sample_height(0.0, 0.0).expect("vertex 0 inside");
        assert!((y0 - 0.0).abs() < 1e-3);

        // Test at centroid
        let y_mid = tri.sample_height(33.333, 33.333).expect("midpoint inside");
        assert!((y_mid - 10.0).abs() < 0.1);
    }
}
