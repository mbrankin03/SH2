//! Parser and data structures for Silent Hill 2 `.cld` collision files.
//!
//! # Ground Truth & Format Specification
//! Layout confirmed from PS2 `SLUS_202.28` (`Collision/cl_main.c`: `clCheckBg2Chara` `0x00133920`,
//! `clGetHitSectListMOVEOutDoor` `0x00138bc0`, `clGetHitSectListMOVEInDoor` `0x00138df0`,
//! `clCheckColumn2ColumnHit` `0x00134f60`) and retail PC `sh2pc.exe` `.cld` binaries:
//!
//! - **Header (`_CL_CLDHEADER`, 372 bytes):**
//!   - `+0x00` `sx`, `sz` (`f32`): World block origin coordinates (mm).
//!   - `+0x08` `b0size`, `b1size`, `b2size`, `b3size`, `csize` (`i32`): Byte lengths for
//!     floor (`fld`), wall (`wld`), ceiling (`ced`), side wall (`swd`), and static BG column (`cld`) arrays.
//!   - `+0x1c` `disable` (`i32`): When non-zero, `clGetHitSectListMOVE*` and `clGetHitSectListVECHIT*`
//!     skip this `.cld` block completely.
//!   - `+0x20` `b0ofs[16]` (`i32`): 4x4 grid cell index lists for floors (`fld`, `-1` terminated).
//!   - `+0x60` `b1ofs[16]` (`i32`): 4x4 grid cell index lists for walls (`wld`, `-1` terminated).
//!   - `+0xa0` `b2ofs[16]` (`i32`): 4x4 grid cell index lists for ceilings (`ced`, `-1` terminated).
//!   - `+0xe0` `b3ofs[16]` (`i32`): 4x4 grid cell index lists for side walls (`swd`, `-1` terminated).
//!   - `+0x120` `clofs[16]` (`i32`): 4x4 grid cell index lists for static BG columns (`cld`, `-1` terminated).
//!   - `+0x160` `fldofs`, `wldofs`, `cedofs`, `swdofs`, `cldofs` (`u32`): Byte offsets to arrays.
//! - **Hitpoly Plane (`_CL_HITPOLY_PLANE`, 80 bytes):**
//!   - `+0x00` `kind` (`u8`), `+0x01` `shape` (`u8`: `0` = tri, `1` = quad), `+0x02` `pad` (`u16`),
//!     `+0x04` `weight` (`u32`), `+0x08` `material` (`u32`), `+0x0c` `flg` (`i32`).
//!   - `+0x10..+0x50` `p[4][4]` (`f32`): 4 vertices `(X, Y, Z, W)` in world mm (`+Y` down).
//! - **Hitpoly Column (`_CL_HITPOLY_COLUMN`, 48 bytes):**
//!   - `+0x00` `kind` (`u8`), `+0x01` `shape` (`u8`: `3` = cylinder), `+0x02` `pad` (`u16`),
//!     `+0x04` `weight` (`u32`), `+0x08` `material` (`u32`), `+0x0c` `flg` (`i32`).
//!   - `+0x10` `p[0]` (`[f32; 4]`): `(center_x, y_bottom, center_z, 1.0)`.
//!   - `+0x20` `p[1]` (`[f32; 4]`): `(0.0, y_top, 0.0, radius)`.

use crate::error::AssetError;

/// Exact 80-byte `_CL_HITPOLY_PLANE` record from a `.cld` file (`Collision/cl_main.c`).
///
/// Coordinates are in Silent Hill 2 world mm with `+Y` pointing downward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitPolyPlane {
    /// Entry kind (`0` = empty/sentinel, `1` = active).
    pub kind: u8,
    /// Polygon shape (`0` = triangle using `p[0..3]`, `1` = quad using `p[0..4]`).
    pub shape: u8,
    /// Collision weight (`+0x04`).
    pub weight: u32,
    /// Surface / thru material ID (`+0x08`).
    pub material: u32,
    /// Collision flags (`+0x0c`).
    pub flg: i32,
    /// Four 4-component vertices `(x, y, z, w)` in world mm (`+0x10..+0x50`).
    pub p: [[f32; 4]; 4],
}

impl HitPolyPlane {
    /// Byte size of a single `_CL_HITPOLY_PLANE` record (`0x50` = 80 bytes).
    pub const BYTE_SIZE: usize = 80;

    /// Computes the unnormalized plane equation `(nx, ny, nz, d)` matching
    /// `clCalcPlaneEquation` (`SLUS_202.28` `0x00131590` / `sh2pc.exe` `0x00529ec0`):
    /// - `e1 = p[1] - p[0]`
    /// - `e2 = p[2] - p[1]`
    /// - `n = e2 x e1`
    /// - `d = -(n . p[0])`
    #[inline]
    pub fn plane_equation(&self) -> [f32; 4] {
        let e1x = self.p[1][0] - self.p[0][0];
        let e1y = self.p[1][1] - self.p[0][1];
        let e1z = self.p[1][2] - self.p[0][2];

        let e2x = self.p[2][0] - self.p[1][0];
        let e2y = self.p[2][1] - self.p[1][1];
        let e2z = self.p[2][2] - self.p[1][2];

        // FUN_004d1150(out, e2, e1) = e2 x e1
        let nx = e2y * e1z - e2z * e1y;
        let ny = e2z * e1x - e2x * e1z;
        let nz = e2x * e1y - e2y * e1x;
        let d = -(nx * self.p[0][0] + ny * self.p[0][1] + nz * self.p[0][2]);
        [nx, ny, nz, d]
    }

    /// Converts a 2D [`WallSegment`] into a vertical quad [`HitPolyPlane`] whose
    /// `clCalcPlaneEquation` normal points to the left of `start -> end` in XZ
    /// (`(-dz, 0, dx)` in `+Y`-down space), matching retail `.cld` wall winding.
    pub const fn from_wall_segment(seg: &WallSegment) -> Self {
        Self {
            kind: 1,
            shape: 1,
            weight: 0,
            material: seg.material,
            flg: 0,
            p: [
                [seg.start[0], seg.y_top, seg.start[1], 1.0],
                [seg.start[0], seg.y_bottom, seg.start[1], 1.0],
                [seg.end[0], seg.y_bottom, seg.end[1], 1.0],
                [seg.end[0], seg.y_top, seg.end[1], 1.0],
            ],
        }
    }
}

/// Exact 48-byte `_CL_HITPOLY_COLUMN` static BG column record (`cldofs` / `csize`,
/// `SLUS_202.28` `clCheckColumn2ColumnHit` `0x00134f60` / `sh2pc.exe` `0x00523190`).
///
/// Coordinates are in world mm with `+Y` pointing downward (`y_top <= y_bottom`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitPolyColumn {
    /// Entry kind (`0` = empty/sentinel, `1` = active).
    pub kind: u8,
    /// Shape ID (`3` = vertical cylinder).
    pub shape: u8,
    /// Collision weight (`+0x04`).
    pub weight: u32,
    /// Surface material ID (`+0x08`).
    pub material: u32,
    /// Collision flags (`+0x0c`).
    pub flg: i32,
    /// Cylinder center X in world mm (`+0x10`).
    pub center_x: f32,
    /// Cylinder bottom Y in world mm (`+0x14`, numerically larger in `+Y`-down).
    pub y_bottom: f32,
    /// Cylinder center Z in world mm (`+0x18`).
    pub center_z: f32,
    /// Cylinder top Y in world mm (`+0x24`, numerically smaller in `+Y`-down).
    pub y_top: f32,
    /// Cylinder radius in world mm (`+0x2c`).
    pub radius: f32,
}

impl HitPolyColumn {
    /// Byte size of a single `_CL_HITPOLY_COLUMN` record (`0x30` = 48 bytes).
    pub const BYTE_SIZE: usize = 48;

    /// Returns true if this column record is active (`kind != 0` and `radius > 0.0`).
    #[inline]
    pub fn is_active(&self) -> bool {
        self.kind != 0 && self.radius > 0.0
    }
}

/// A 3D floor collision triangle extracted from `.cld` group 0 (`fld`).
/// Coordinates are in Silent Hill 2 world units (mm) with `+Y` pointing downward.
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

    /// Perform a 2D barycentric projection test in XZ plane.
    /// If `(x, z)` lies inside the triangle, returns `Some(y)` interpolated height.
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

/// A 2D wall segment in the XZ plane with vertical height span `[y_top, y_bottom]`.
/// Because `+Y` points downward in PS2/PC coordinates, `y_top <= y_bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallSegment {
    /// Segment start point in XZ coordinates `(x0, z0)`.
    pub start: [f32; 2],
    /// Segment end point in XZ coordinates `(x1, z1)`.
    pub end: [f32; 2],
    /// Top height boundary (lower numeric value because `+Y` is down).
    pub y_top: f32,
    /// Bottom height boundary (higher numeric value because `+Y` is down).
    pub y_bottom: f32,
    /// Collision group (`0` = floor, `1` = wall, `2` = ceiling, `3` = side wall).
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
    /// Character body spans `y - h` (head) to `y` (feet).
    pub fn overlaps_vertical(&self, y: f32, h: f32) -> bool {
        self.y_top < (y - 10.0) && self.y_bottom > (y - h)
    }
}

/// Parsed `.cld` collision file matching `_CL_CLDHEADER` (`Collision/cl_main.c`).
#[derive(Debug, Clone, Default)]
pub struct CldFile {
    /// Room/block origin coordinates `(sx, sz)` in world mm (`+0x00`).
    pub origin: [f32; 2],
    /// Header disable flag (`+0x1c`). When non-zero, `clGetHitSectListMOVE*` skips this `.cld`.
    pub disable: i32,
    /// Triangulated walkable floor triangles (derived from `raw_floors`).
    pub floors: Vec<FloorTriangle>,
    /// Derived 2D obstacle wall segments (for legacy raycast/audio consumers).
    pub walls: Vec<WallSegment>,
    /// Raw `_CL_HITPOLY_PLANE` floor array (`fldofs`/`b0size`), indexed 1:1 by `b0_cells`.
    pub raw_floors: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_PLANE` wall array (`wldofs`/`b1size`), indexed 1:1 by `b1_cells`.
    pub raw_walls: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_PLANE` ceiling array (`cedofs`/`b2size`), indexed 1:1 by `b2_cells`.
    pub raw_ceilings: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_PLANE` side-wall array (`swdofs`/`b3size`), indexed 1:1 by `b3_cells`.
    pub raw_side_walls: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_COLUMN` static BG column array (`cldofs`/`csize`), indexed 1:1 by `cl_cells`.
    pub columns: Vec<HitPolyColumn>,
    /// Authentic 4x4 spatial grid polygon indices for floors (`b0ofs[16]` at `+0x20`).
    pub b0_cells: [Vec<usize>; 16],
    /// Authentic 4x4 spatial grid polygon indices for walls (`b1ofs[16]` at `+0x60`).
    pub b1_cells: [Vec<usize>; 16],
    /// Authentic 4x4 spatial grid polygon indices for ceilings (`b2ofs[16]` at `+0xa0`).
    pub b2_cells: [Vec<usize>; 16],
    /// Authentic 4x4 spatial grid polygon indices for side walls (`b3ofs[16]` at `+0xe0`).
    pub b3_cells: [Vec<usize>; 16],
    /// Authentic 4x4 spatial grid column indices for static BG columns (`clofs[16]` at `+0x120`).
    pub cl_cells: [Vec<usize>; 16],
}

impl CldFile {
    /// Returns `true` if this `.cld` file is disabled (`disable != 0`, header `+0x1c`).
    #[inline]
    pub const fn is_disabled(&self) -> bool {
        self.disable != 0
    }

    /// Returns `true` if any cell `1..16` across `b0..cl` contains indices,
    /// indicating an outdoor 4x4 sector block (`clGetHitSectListMOVEOutDoor`)
    /// rather than an indoor room (`clGetHitSectListMOVEInDoor`, cell 0 only).
    pub fn has_outdoor_grid(&self) -> bool {
        (1..16).any(|c| {
            !self.b0_cells[c].is_empty()
                || !self.b1_cells[c].is_empty()
                || !self.b2_cells[c].is_empty()
                || !self.b3_cells[c].is_empty()
                || !self.cl_cells[c].is_empty()
        })
    }

    /// Creates synthetic collision geometry matching the procedural fallback room
    /// (e.g. boiler room perimeter walls and center pillar quad).
    ///
    /// - Dimensions: width `w`, length `l`, ceiling height `h` (downwards `Y in [-h, 0]`).
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

        // 4 Outer perimeter walls (normals point inward into the room)
        // North wall (normal +Z)
        walls.push(WallSegment::new(
            [-half_w, -half_l],
            [half_w, -half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));
        // East wall (normal -X)
        walls.push(WallSegment::new(
            [half_w, -half_l],
            [half_w, half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));
        // South wall (normal -Z)
        walls.push(WallSegment::new(
            [half_w, half_l],
            [-half_w, half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));
        // West wall (normal +X)
        walls.push(WallSegment::new(
            [-half_w, half_l],
            [-half_w, -half_l],
            y_top,
            y_bottom,
            1,
            0,
        ));

        // Center pillar walls (normals point outward into the room)
        if pw > 0.0 && pl > 0.0 {
            // North face of pillar (normal -Z)
            walls.push(WallSegment::new(
                [half_pw, -half_pl],
                [-half_pw, -half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
            // East face of pillar (normal +X)
            walls.push(WallSegment::new(
                [half_pw, half_pl],
                [half_pw, -half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
            // South face of pillar (normal +Z)
            walls.push(WallSegment::new(
                [-half_pw, half_pl],
                [half_pw, half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
            // West face of pillar (normal -X)
            walls.push(WallSegment::new(
                [-half_pw, -half_pl],
                [-half_pw, half_pl],
                y_top,
                y_bottom,
                1,
                0,
            ));
        }

        let raw_walls: Vec<HitPolyPlane> =
            walls.iter().map(HitPolyPlane::from_wall_segment).collect();

        let mut b0_cells: [Vec<usize>; 16] = Default::default();
        let mut b1_cells: [Vec<usize>; 16] = Default::default();
        for i in 0..floors.len() {
            b0_cells[0].push(i);
        }
        for i in 0..raw_walls.len() {
            b1_cells[0].push(i);
        }

        Self {
            origin: [0.0, 0.0],
            disable: 0,
            floors,
            walls,
            raw_floors: Vec::new(),
            raw_walls,
            raw_ceilings: Vec::new(),
            raw_side_walls: Vec::new(),
            columns: Vec::new(),
            b0_cells,
            b1_cells,
            b2_cells: Default::default(),
            b3_cells: Default::default(),
            cl_cells: Default::default(),
        }
    }
}

#[inline]
fn read_f32_le(data: &[u8], offset: usize) -> Result<f32, AssetError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(AssetError::BufferUnderflow {
            required: offset + 4,
            available: data.len(),
        })?
        .try_into()
        .map_err(|_| AssetError::BufferUnderflow {
            required: offset + 4,
            available: data.len(),
        })?;
    Ok(f32::from_le_bytes(bytes))
}

#[inline]
fn read_i32_le(data: &[u8], offset: usize) -> Result<i32, AssetError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(AssetError::BufferUnderflow {
            required: offset + 4,
            available: data.len(),
        })?
        .try_into()
        .map_err(|_| AssetError::BufferUnderflow {
            required: offset + 4,
            available: data.len(),
        })?;
    Ok(i32::from_le_bytes(bytes))
}

#[inline]
fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, AssetError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(AssetError::BufferUnderflow {
            required: offset + 4,
            available: data.len(),
        })?
        .try_into()
        .map_err(|_| AssetError::BufferUnderflow {
            required: offset + 4,
            available: data.len(),
        })?;
    Ok(u32::from_le_bytes(bytes))
}

/// Parses a 16-cell `-1`-terminated index table (`b0ofs`/`b1ofs`/`b2ofs`/`b3ofs`/`clofs`).
fn parse_cell_table(
    data: &[u8],
    header_table_offset: usize,
) -> Result<[Vec<usize>; 16], AssetError> {
    let mut cells: [Vec<usize>; 16] = Default::default();
    for (cell, cell_indices) in cells.iter_mut().enumerate() {
        let ofs_pos = header_table_offset + cell * 4;
        let table_ofs_raw = read_i32_le(data, ofs_pos)?;
        if table_ofs_raw <= 0 {
            continue;
        }
        let mut cur = table_ofs_raw as usize;
        while cur + 4 <= data.len() {
            let idx = read_i32_le(data, cur)?;
            cur += 4;
            if idx == -1 {
                break;
            }
            if idx >= 0 {
                cell_indices.push(idx as usize);
            }
        }
    }
    Ok(cells)
}

/// Parses an array of 80-byte `_CL_HITPOLY_PLANE` records preserving 1:1 raw indices.
fn parse_plane_array(
    data: &[u8],
    ofs: usize,
    size: usize,
) -> Result<Vec<HitPolyPlane>, AssetError> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let end = ofs.checked_add(size).ok_or(AssetError::BufferUnderflow {
        required: usize::MAX,
        available: data.len(),
    })?;
    if end > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: end,
            available: data.len(),
        });
    }

    let num_polys = size / HitPolyPlane::BYTE_SIZE;
    let mut planes = Vec::with_capacity(num_polys);
    for i in 0..num_polys {
        let p_ofs = ofs + i * HitPolyPlane::BYTE_SIZE;
        let kind = data[p_ofs];
        let shape = data[p_ofs + 1];
        let weight = read_u32_le(data, p_ofs + 4)?;
        let material = read_u32_le(data, p_ofs + 8)?;
        let flg = read_i32_le(data, p_ofs + 12)?;
        let mut p = [[0.0f32; 4]; 4];
        for (v_idx, pt) in p.iter_mut().enumerate() {
            let v_ofs = p_ofs + 16 + v_idx * 16;
            pt[0] = read_f32_le(data, v_ofs)?;
            pt[1] = read_f32_le(data, v_ofs + 4)?;
            pt[2] = read_f32_le(data, v_ofs + 8)?;
            pt[3] = read_f32_le(data, v_ofs + 12)?;
        }
        planes.push(HitPolyPlane {
            kind,
            shape,
            weight,
            material,
            flg,
            p,
        });
    }
    Ok(planes)
}

/// Parses an array of 48-byte `_CL_HITPOLY_COLUMN` records (`cldofs`/`csize`) preserving 1:1 raw indices.
fn parse_column_array(
    data: &[u8],
    ofs: usize,
    size: usize,
) -> Result<Vec<HitPolyColumn>, AssetError> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let end = ofs.checked_add(size).ok_or(AssetError::BufferUnderflow {
        required: usize::MAX,
        available: data.len(),
    })?;
    if end > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: end,
            available: data.len(),
        });
    }

    let num_cols = size / HitPolyColumn::BYTE_SIZE;
    let mut columns = Vec::with_capacity(num_cols);
    for i in 0..num_cols {
        let c_ofs = ofs + i * HitPolyColumn::BYTE_SIZE;
        let kind = data[c_ofs];
        let shape = data[c_ofs + 1];
        let weight = read_u32_le(data, c_ofs + 4)?;
        let material = read_u32_le(data, c_ofs + 8)?;
        let flg = read_i32_le(data, c_ofs + 12)?;
        let center_x = read_f32_le(data, c_ofs + 16)?;
        let y_bottom = read_f32_le(data, c_ofs + 20)?;
        let center_z = read_f32_le(data, c_ofs + 24)?;
        let y_top = read_f32_le(data, c_ofs + 36)?;
        let radius = read_f32_le(data, c_ofs + 44)?;
        columns.push(HitPolyColumn {
            kind,
            shape,
            weight,
            material,
            flg,
            center_x,
            y_bottom,
            center_z,
            y_top,
            radius,
        });
    }
    Ok(columns)
}

/// Extracts a 2D [`WallSegment`] from an active [`HitPolyPlane`] for legacy raycast/audio consumers.
fn plane_to_wall_segment(plane: &HitPolyPlane, group: u32) -> Option<WallSegment> {
    if plane.kind == 0 {
        return None;
    }
    let vertex_count = if plane.shape == 0 { 3 } else { 4 };
    let pts = [
        [plane.p[0][0], plane.p[0][1], plane.p[0][2]],
        [plane.p[1][0], plane.p[1][1], plane.p[1][2]],
        [plane.p[2][0], plane.p[2][1], plane.p[2][2]],
        [plane.p[3][0], plane.p[3][1], plane.p[3][2]],
    ];

    let eq = plane.plane_equation();
    let norm_len = (eq[0] * eq[0] + eq[1] * eq[1] + eq[2] * eq[2]).sqrt();
    if norm_len < 1e-6 || (eq[1].abs() / norm_len) > 0.5 {
        return None;
    }

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
        return None;
    }

    let mut y_min = pts[0][1];
    let mut y_max = pts[0][1];
    for pt in &pts[1..vertex_count] {
        y_min = y_min.min(pt[1]);
        y_max = y_max.max(pt[1]);
    }

    let p_a = pts[best_pair.0];
    let p_b = pts[best_pair.1];
    Some(WallSegment::new(
        [p_a[0], p_a[2]],
        [p_b[0], p_b[2]],
        y_min,
        y_max,
        group,
        plane.material,
    ))
}

/// Parse a `.cld` binary buffer into structured [`CldFile`] collision geometry.
pub fn parse_cld(data: &[u8]) -> Result<CldFile, AssetError> {
    const MIN_HEADER_SIZE: usize = 32 + 64 * 5 + 20; // 372 bytes (0x174)
    if data.len() < MIN_HEADER_SIZE {
        return Err(AssetError::BufferUnderflow {
            required: MIN_HEADER_SIZE,
            available: data.len(),
        });
    }

    let sx = read_f32_le(data, 0)?;
    let sz = read_f32_le(data, 4)?;

    let b0size = read_i32_le(data, 8)?.max(0) as usize;
    let b1size = read_i32_le(data, 12)?.max(0) as usize;
    let b2size = read_i32_le(data, 16)?.max(0) as usize;
    let b3size = read_i32_le(data, 20)?.max(0) as usize;
    let csize = read_i32_le(data, 24)?.max(0) as usize;
    let disable = read_i32_le(data, 28)?;

    // Polygon & column array offsets at byte 0x160 (352 = 32 + 64 * 5)
    let fldofs = read_u32_le(data, 0x160)? as usize;
    let wldofs = read_u32_le(data, 0x164)? as usize;
    let cedofs = read_u32_le(data, 0x168)? as usize;
    let swdofs = read_u32_le(data, 0x16c)? as usize;
    let cldofs = read_u32_le(data, 0x170)? as usize;

    let raw_floors = parse_plane_array(data, fldofs, b0size)?;
    let raw_walls = parse_plane_array(data, wldofs, b1size)?;
    let raw_ceilings = parse_plane_array(data, cedofs, b2size)?;
    let raw_side_walls = parse_plane_array(data, swdofs, b3size)?;
    let columns = parse_column_array(data, cldofs, csize)?;

    // Derive FloorTriangle list from raw_floors
    let mut floors = Vec::with_capacity(raw_floors.len() * 2);
    for plane in &raw_floors {
        if plane.kind == 0 {
            continue;
        }
        let p0 = [plane.p[0][0], plane.p[0][1], plane.p[0][2]];
        let p1 = [plane.p[1][0], plane.p[1][1], plane.p[1][2]];
        let p2 = [plane.p[2][0], plane.p[2][1], plane.p[2][2]];
        floors.push(FloorTriangle::new([p0, p1, p2], plane.material));
        if plane.shape != 0 {
            let p3 = [plane.p[3][0], plane.p[3][1], plane.p[3][2]];
            floors.push(FloorTriangle::new([p0, p2, p3], plane.material));
        }
    }

    // Derive legacy WallSegment list from groups 1, 2, 3
    let mut walls = Vec::with_capacity(raw_walls.len() + raw_side_walls.len());
    for (group, planes) in [
        (1u32, &raw_walls),
        (2u32, &raw_ceilings),
        (3u32, &raw_side_walls),
    ] {
        for plane in planes {
            if let Some(seg) = plane_to_wall_segment(plane, group) {
                walls.push(seg);
            }
        }
    }

    // Parse all five 4x4 spatial grid cell index tables (-1 terminated)
    let b0_cells = parse_cell_table(data, 0x20)?;
    let b1_cells = parse_cell_table(data, 0x60)?;
    let b2_cells = parse_cell_table(data, 0xa0)?;
    let b3_cells = parse_cell_table(data, 0xe0)?;
    let cl_cells = parse_cell_table(data, 0x120)?;

    Ok(CldFile {
        origin: [sx, sz],
        disable,
        floors,
        walls,
        raw_floors,
        raw_walls,
        raw_ceilings,
        raw_side_walls,
        columns,
        b0_cells,
        b1_cells,
        b2_cells,
        b3_cells,
        cl_cells,
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
        assert_eq!(room.raw_walls.len(), 8);
        assert!(!room.is_disabled());

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

    #[test]
    fn test_plane_equation_inward_normal() {
        let room = CldFile::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        // North wall at Z = -1000 should have normal pointing +Z (into room)
        let n_north = room.raw_walls[0].plane_equation();
        assert!(
            n_north[2] > 0.0,
            "North wall normal must point +Z: {:?}",
            n_north
        );
        // East wall at X = +1000 should have normal pointing -X (into room)
        let n_east = room.raw_walls[1].plane_equation();
        assert!(
            n_east[0] < 0.0,
            "East wall normal must point -X: {:?}",
            n_east
        );
    }
}
