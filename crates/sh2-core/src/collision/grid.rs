//! Authentic 4x4 spatial grid acceleration partitioning (`clGetHitSectListMOVE`).
//!
//! Ground truth from `SLUS_202.28` (`Collision/cl_main.c`):
//! - `clGetHitSectListMOVE` (`0x00138b60`): dispatches to `InDoor` or `OutDoor`.
//! - `clGetHitSectListMOVEInDoor` (`0x00138df0`): checks `header.disable == 0` (`+0x1c`),
//!   returns cell `0` only.
//! - `clGetHitSectListMOVEOutDoor` (`0x00138bc0` / `sh2pc.exe` `0x00524940`): checks
//!   `header.disable == 0`, then block distance `(x - sx)^2 + y^2 + (z - sz)^2 <= 900_000_000.0`
//!   (`30_000.0^2`), then for each `(row, col)` in `0..4 x 0..4` with cell corner
//!   `(sx + col * 5000.0, sz + row * 5000.0)` checks `(x - cx)^2 + (z - cz)^2 < 64_000_000.0`
//!   (`8_000.0^2`), emitting cell `row * 4 + col`.

use sh2_assets::formats::cld::{CldFile, FloorTriangle, HitPolyColumn, HitPolyPlane, WallSegment};

/// Outdoor `.cld` block cell pitch in world mm (`clGetHitSectListMOVEOutDoor` `0x00138bc0`).
pub const OUTDOOR_CELL_SIZE: f32 = 5000.0;

/// Outdoor `.cld` block cull radius squared (`30_000.0^2 = 9e8`, `0x00138bc0`).
pub const OUTDOOR_BLOCK_CULL_DIST_SQ: f32 = 900_000_000.0;

/// Outdoor `.cld` cell cull radius squared (`8_000.0^2 = 6.4e7`, `0x00138bc0`).
pub const OUTDOOR_CELL_CULL_DIST_SQ: f32 = 64_000_000.0;

/// Empty sentinel AABB `[min_x, min_z, max_x, max_z]` with `min > max`.
const EMPTY_AABB_XZ: [f32; 4] = [
    f32::INFINITY,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NEG_INFINITY,
];

#[inline]
fn expand_aabb_xz(aabb: &mut [f32; 4], x: f32, z: f32) {
    aabb[0] = aabb[0].min(x);
    aabb[1] = aabb[1].min(z);
    aabb[2] = aabb[2].max(x);
    aabb[3] = aabb[3].max(z);
}

#[inline]
fn union_aabb_xz(dst: &mut [f32; 4], src: [f32; 4]) {
    if src[0] <= src[2] && src[1] <= src[3] {
        dst[0] = dst[0].min(src[0]);
        dst[1] = dst[1].min(src[1]);
        dst[2] = dst[2].max(src[2]);
        dst[3] = dst[3].max(src[3]);
    }
}

#[inline]
fn plane_aabb_xz(plane: &HitPolyPlane) -> [f32; 4] {
    let n = if plane.shape != 0 { 4 } else { 3 };
    let mut aabb = EMPTY_AABB_XZ;
    for pt in &plane.p[..n] {
        expand_aabb_xz(&mut aabb, pt[0], pt[2]);
    }
    aabb
}

#[inline]
fn floor_tri_aabb_xz(tri: &FloorTriangle) -> [f32; 4] {
    let mut aabb = EMPTY_AABB_XZ;
    for pt in &tri.v {
        expand_aabb_xz(&mut aabb, pt[0], pt[2]);
    }
    aabb
}

#[inline]
fn column_aabb_xz(col: &HitPolyColumn) -> [f32; 4] {
    let r = col.radius.max(0.0);
    [
        col.center_x - r,
        col.center_z - r,
        col.center_x + r,
        col.center_z + r,
    ]
}

#[inline]
fn wall_seg_aabb_xz(w: &WallSegment) -> [f32; 4] {
    [
        w.start[0].min(w.end[0]),
        w.start[1].min(w.end[1]),
        w.start[0].max(w.end[0]),
        w.start[1].max(w.end[1]),
    ]
}

/// Exact 2D segment-vs-AABB intersection test in XZ (`clCheckCrossLine2BoxXZ` / slab method).
///
/// Uses inclusive bounds (`<=`) with a tiny floating-point pad so boundary, grazing, and
/// zero-length rays never produce false negatives.
#[inline]
pub fn segment_intersects_aabb_xz(x0: f32, z0: f32, x1: f32, z1: f32, aabb: [f32; 4]) -> bool {
    let [min_x, min_z, max_x, max_z] = aabb;
    if min_x > max_x || min_z > max_z {
        return false;
    }
    // Small epsilon pad prevents grazing rays on exact cell boundaries from missing due to rounding.
    const PAD: f32 = 1e-2;
    let min_x = min_x - PAD;
    let min_z = min_z - PAD;
    let max_x = max_x + PAD;
    let max_z = max_z + PAD;

    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;

    let dx = x1 - x0;
    if dx.abs() < 1e-8 {
        if x0 < min_x || x0 > max_x {
            return false;
        }
    } else {
        let inv_dx = 1.0 / dx;
        let mut t1 = (min_x - x0) * inv_dx;
        let mut t2 = (max_x - x0) * inv_dx;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return false;
        }
    }

    let dz = z1 - z0;
    if dz.abs() < 1e-8 {
        if z0 < min_z || z0 > max_z {
            return false;
        }
    } else {
        let inv_dz = 1.0 / dz;
        let mut t1 = (min_z - z0) * inv_dz;
        let mut t2 = (max_z - z0) * inv_dz;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return false;
        }
    }

    true
}

/// 4x4 Spatial acceleration grid matching PS2 `_CL_SELECT_MAP` and `.cld` cell tables.
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    /// Grid origin `(sx, sz)` in world units (mm).
    pub origin_x: f32,
    pub origin_z: f32,
    /// Header `disable` flag (`+0x1c`). When non-zero, `clGetHitSectListMOVE` emits no cells.
    pub disable: i32,
    /// Whether this grid uses `clGetHitSectListMOVEOutDoor` (`true`) or `InDoor` cell 0 (`false`).
    pub is_outdoor: bool,
    /// `true` when constructed from a `.cld` file via [`Self::from_cld`] so raycasts can trust
    /// cell tables instead of scanning raw slices. `false` for `SpatialGrid::default()`.
    pub has_indexed_geometry: bool,
    /// Cell dimensions in world units (mm) for `point_to_cell` queries.
    pub cell_width: f32,
    pub cell_depth: f32,
    /// Tight XZ bounding box `[min_x, min_z, max_x, max_z]` enclosing all geometry in this block.
    pub block_aabb_xz: [f32; 4],
    /// Tight XZ bounding box `[min_x, min_z, max_x, max_z]` enclosing all geometry in each cell.
    pub cell_aabb_xz: [[f32; 4]; 16],
    /// Floor triangle indices per cell `[row * 4 + col]` (mapped from `b0_cells` to `floors`).
    pub floor_cells: [Vec<usize>; 16],
    /// Raw floor (`fld`, group 0) polygon indices per cell (`b0ofs[16]`).
    pub raw_floor_cells: [Vec<usize>; 16],
    /// Raw wall (`wld`, group 1) polygon indices per cell (`b1ofs[16]`).
    pub wall_cells: [Vec<usize>; 16],
    /// Raw ceiling (`ced`, group 2) polygon indices per cell (`b2ofs[16]`).
    pub ceiling_cells: [Vec<usize>; 16],
    /// Raw side wall (`swd`, group 3) polygon indices per cell (`b3ofs[16]`).
    pub side_wall_cells: [Vec<usize>; 16],
    /// Raw static BG column (`cld`) indices per cell (`clofs[16]`).
    pub column_cells: [Vec<usize>; 16],
    /// Derived 2D [`WallSegment`] indices per cell (`walls`) for AI LOS and acoustic raycasts.
    pub wall_segment_cells: [Vec<usize>; 16],
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self {
            origin_x: 0.0,
            origin_z: 0.0,
            disable: 0,
            is_outdoor: false,
            has_indexed_geometry: false,
            cell_width: 0.0,
            cell_depth: 0.0,
            block_aabb_xz: EMPTY_AABB_XZ,
            cell_aabb_xz: [EMPTY_AABB_XZ; 16],
            floor_cells: Default::default(),
            raw_floor_cells: Default::default(),
            wall_cells: Default::default(),
            ceiling_cells: Default::default(),
            side_wall_cells: Default::default(),
            column_cells: Default::default(),
            wall_segment_cells: Default::default(),
        }
    }
}

#[derive(Clone, Copy)]
struct GridLayout {
    is_outdoor: bool,
    origin_x: f32,
    origin_z: f32,
    cell_width: f32,
    cell_depth: f32,
}

/// Maps an XZ AABB `[min_x, min_z, max_x, max_z]` to inclusive `(col_start..=col_end, row_start..=row_end)`
/// in `0..=3` for outdoor grids, or `(0..=0, 0..=0)` for indoor grids.
#[inline]
fn aabb_to_cell_range(layout: GridLayout, aabb: [f32; 4]) -> (usize, usize, usize, usize) {
    if !layout.is_outdoor || layout.cell_width <= 1e-4 || layout.cell_depth <= 1e-4 {
        return (0, 0, 0, 0);
    }
    let c0 = ((aabb[0] - layout.origin_x) / layout.cell_width).floor() as isize;
    let c1 = ((aabb[2] - layout.origin_x) / layout.cell_width).floor() as isize;
    let r0 = ((aabb[1] - layout.origin_z) / layout.cell_depth).floor() as isize;
    let r1 = ((aabb[3] - layout.origin_z) / layout.cell_depth).floor() as isize;
    (
        c0.clamp(0, 3) as usize,
        c1.clamp(0, 3) as usize,
        r0.clamp(0, 3) as usize,
        r1.clamp(0, 3) as usize,
    )
}

/// Ensures every element `0..len` appears in at least one cell of `cells`, inserting any
/// unindexed element into the cell(s) overlapped by its XZ AABB.
fn ensure_all_indexed<F>(cells: &mut [Vec<usize>; 16], len: usize, layout: GridLayout, aabb_of: F)
where
    F: Fn(usize) -> [f32; 4],
{
    if len == 0 {
        return;
    }
    let mut seen = vec![false; len];
    for cell in cells.iter() {
        for &idx in cell {
            if idx < len {
                seen[idx] = true;
            }
        }
    }
    for (idx, was_seen) in seen.into_iter().enumerate() {
        if !was_seen {
            let aabb = aabb_of(idx);
            let (c0, c1, r0, r1) = aabb_to_cell_range(layout, aabb);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    cells[r * 4 + c].push(idx);
                }
            }
        }
    }
}

impl SpatialGrid {
    /// Build spatial grid from an authentic parsed `.cld` file.
    pub fn from_cld(cld: &CldFile) -> Self {
        let (origin_x, origin_z) = (cld.origin[0], cld.origin[1]);
        let is_outdoor = cld.has_outdoor_grid();

        let mut min_x = origin_x;
        let mut max_x = origin_x + OUTDOOR_CELL_SIZE * 4.0;
        let mut min_z = origin_z;
        let mut max_z = origin_z + OUTDOOR_CELL_SIZE * 4.0;

        if !is_outdoor && !cld.floors.is_empty() {
            let mut f_min_x = f32::INFINITY;
            let mut f_max_x = f32::NEG_INFINITY;
            let mut f_min_z = f32::INFINITY;
            let mut f_max_z = f32::NEG_INFINITY;

            for f in &cld.floors {
                for pt in &f.v {
                    f_min_x = f_min_x.min(pt[0]);
                    f_max_x = f_max_x.max(pt[0]);
                    f_min_z = f_min_z.min(pt[2]);
                    f_max_z = f_max_z.max(pt[2]);
                }
            }

            if f_max_x > f_min_x && f_max_z > f_min_z {
                min_x = min_x.min(f_min_x);
                max_x = max_x.max(f_max_x);
                min_z = min_z.min(f_min_z);
                max_z = max_z.max(f_max_z);
            }
        }

        let cell_width = if is_outdoor {
            OUTDOOR_CELL_SIZE
        } else {
            (max_x - min_x).max(1000.0) / 4.0
        };
        let cell_depth = if is_outdoor {
            OUTDOOR_CELL_SIZE
        } else {
            (max_z - min_z).max(1000.0) / 4.0
        };

        let has_cld_cells = cld.b0_cells.iter().any(|c| !c.is_empty())
            || cld.b1_cells.iter().any(|c| !c.is_empty())
            || cld.b2_cells.iter().any(|c| !c.is_empty())
            || cld.b3_cells.iter().any(|c| !c.is_empty())
            || cld.cl_cells.iter().any(|c| !c.is_empty());

        let (
            eff_origin_x,
            eff_origin_z,
            eff_is_outdoor,
            mut floor_cells,
            mut raw_floor_cells,
            mut wall_cells,
            mut ceiling_cells,
            mut side_wall_cells,
            mut column_cells,
        ) = if has_cld_cells {
            // Build exact raw_floor_idx -> FloorTriangle index span map
            let mut floor_cells: [Vec<usize>; 16] = Default::default();
            if !cld.raw_floors.is_empty() {
                let mut raw_to_tri: Vec<(usize, usize)> = Vec::with_capacity(cld.raw_floors.len());
                let mut tri_cursor = 0usize;
                for plane in &cld.raw_floors {
                    if plane.kind == 0 {
                        raw_to_tri.push((tri_cursor, 0));
                    } else {
                        let count = if plane.shape != 0 { 2 } else { 1 };
                        raw_to_tri.push((tri_cursor, count));
                        tri_cursor += count;
                    }
                }
                for (cell_idx, indices) in cld.b0_cells.iter().enumerate() {
                    for &raw_idx in indices {
                        if let Some(&(start, count)) = raw_to_tri.get(raw_idx) {
                            for tri_idx in start..(start + count) {
                                if tri_idx < cld.floors.len() {
                                    floor_cells[cell_idx].push(tri_idx);
                                }
                            }
                        }
                    }
                }
            } else {
                for (cell_idx, indices) in cld.b0_cells.iter().enumerate() {
                    for &idx in indices {
                        if idx < cld.floors.len() {
                            floor_cells[cell_idx].push(idx);
                        }
                    }
                }
            }

            (
                origin_x,
                origin_z,
                is_outdoor,
                floor_cells,
                cld.b0_cells.clone(),
                cld.b1_cells.clone(),
                cld.b2_cells.clone(),
                cld.b3_cells.clone(),
                cld.cl_cells.clone(),
            )
        } else {
            // Fallback for synthetic worlds without explicit cell tables: place all in cell 0 (InDoor)
            let mut floor_cells: [Vec<usize>; 16] = Default::default();
            let mut raw_floor_cells: [Vec<usize>; 16] = Default::default();
            let mut wall_cells: [Vec<usize>; 16] = Default::default();
            let mut ceiling_cells: [Vec<usize>; 16] = Default::default();
            let mut side_wall_cells: [Vec<usize>; 16] = Default::default();
            let mut column_cells: [Vec<usize>; 16] = Default::default();

            floor_cells[0].extend(0..cld.floors.len());
            raw_floor_cells[0].extend(0..cld.raw_floors.len());
            wall_cells[0].extend(0..cld.raw_walls.len());
            ceiling_cells[0].extend(0..cld.raw_ceilings.len());
            side_wall_cells[0].extend(0..cld.raw_side_walls.len());
            column_cells[0].extend(0..cld.columns.len());

            (
                min_x,
                min_z,
                false,
                floor_cells,
                raw_floor_cells,
                wall_cells,
                ceiling_cells,
                side_wall_cells,
                column_cells,
            )
        };

        let layout = GridLayout {
            is_outdoor: eff_is_outdoor,
            origin_x: eff_origin_x,
            origin_z: eff_origin_z,
            cell_width,
            cell_depth,
        };

        // Guarantee 100% coverage of all primitives (even if a `.cld` omits an index from `b*ofs`).
        ensure_all_indexed(&mut floor_cells, cld.floors.len(), layout, |i| {
            floor_tri_aabb_xz(&cld.floors[i])
        });
        ensure_all_indexed(&mut raw_floor_cells, cld.raw_floors.len(), layout, |i| {
            plane_aabb_xz(&cld.raw_floors[i])
        });
        ensure_all_indexed(&mut wall_cells, cld.raw_walls.len(), layout, |i| {
            plane_aabb_xz(&cld.raw_walls[i])
        });
        ensure_all_indexed(&mut ceiling_cells, cld.raw_ceilings.len(), layout, |i| {
            plane_aabb_xz(&cld.raw_ceilings[i])
        });
        ensure_all_indexed(
            &mut side_wall_cells,
            cld.raw_side_walls.len(),
            layout,
            |i| plane_aabb_xz(&cld.raw_side_walls[i]),
        );
        ensure_all_indexed(&mut column_cells, cld.columns.len(), layout, |i| {
            column_aabb_xz(&cld.columns[i])
        });

        // Index derived 2D `WallSegment`s (`cld.walls`) into `wall_segment_cells`.
        let mut wall_segment_cells: [Vec<usize>; 16] = Default::default();
        for (idx, w) in cld.walls.iter().enumerate() {
            let aabb = wall_seg_aabb_xz(w);
            let (c0, c1, r0, r1) = aabb_to_cell_range(layout, aabb);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    wall_segment_cells[r * 4 + c].push(idx);
                }
            }
        }

        // Compute tight per-cell and block XZ AABBs enclosing every primitive indexed in each cell.
        let mut cell_aabb_xz = [EMPTY_AABB_XZ; 16];
        for cell in 0..16 {
            let aabb = &mut cell_aabb_xz[cell];
            for &i in &floor_cells[cell] {
                if let Some(tri) = cld.floors.get(i) {
                    union_aabb_xz(aabb, floor_tri_aabb_xz(tri));
                }
            }
            for &i in &raw_floor_cells[cell] {
                if let Some(p) = cld.raw_floors.get(i) {
                    union_aabb_xz(aabb, plane_aabb_xz(p));
                }
            }
            for &i in &wall_cells[cell] {
                if let Some(p) = cld.raw_walls.get(i) {
                    union_aabb_xz(aabb, plane_aabb_xz(p));
                }
            }
            for &i in &ceiling_cells[cell] {
                if let Some(p) = cld.raw_ceilings.get(i) {
                    union_aabb_xz(aabb, plane_aabb_xz(p));
                }
            }
            for &i in &side_wall_cells[cell] {
                if let Some(p) = cld.raw_side_walls.get(i) {
                    union_aabb_xz(aabb, plane_aabb_xz(p));
                }
            }
            for &i in &column_cells[cell] {
                if let Some(col) = cld.columns.get(i) {
                    union_aabb_xz(aabb, column_aabb_xz(col));
                }
            }
            for &i in &wall_segment_cells[cell] {
                if let Some(w) = cld.walls.get(i) {
                    union_aabb_xz(aabb, wall_seg_aabb_xz(w));
                }
            }
        }

        let mut block_aabb_xz = EMPTY_AABB_XZ;
        for &c_aabb in &cell_aabb_xz {
            union_aabb_xz(&mut block_aabb_xz, c_aabb);
        }

        Self {
            origin_x: eff_origin_x,
            origin_z: eff_origin_z,
            disable: cld.disable,
            is_outdoor: eff_is_outdoor,
            has_indexed_geometry: true,
            cell_width,
            cell_depth,
            block_aabb_xz,
            cell_aabb_xz,
            floor_cells,
            raw_floor_cells,
            wall_cells,
            ceiling_cells,
            side_wall_cells,
            column_cells,
            wall_segment_cells,
        }
    }

    /// Zero-allocation stack variant of `clGetHitSectListVECHIT` (`SLUS_202.28` `0x00138170`,
    /// `0x001381e0` OutDoor, `0x001384b0` InDoor) for ray/segment `(x0, z0) -> (x1, z1)`.
    ///
    /// Returns `(cells, count)` with active cell indices in `cells[0..count]`:
    /// - Returns `0` cells if `self.disable != 0` or the segment misses `self.block_aabb_xz`.
    /// - `InDoor` (`!self.is_outdoor`): emits cell `0` iff the segment intersects `block_aabb_xz`.
    /// - `OutDoor` (`self.is_outdoor`): tests each of the 16 cells' tight enclosing XZ AABBs
    ///   (guaranteed to contain all primitives spanning beyond nominal 5000 mm cell edges).
    #[inline]
    pub fn query_ray_cells_array(
        &self,
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
    ) -> ([usize; 16], usize) {
        let mut cells = [0usize; 16];
        if self.disable != 0 {
            return (cells, 0);
        }
        if !segment_intersects_aabb_xz(x0, z0, x1, z1, self.block_aabb_xz) {
            return (cells, 0);
        }
        if !self.is_outdoor {
            cells[0] = 0;
            return (cells, 1);
        }
        let mut count = 0usize;
        for (cell, &aabb) in self.cell_aabb_xz.iter().enumerate() {
            if segment_intersects_aabb_xz(x0, z0, x1, z1, aabb) {
                cells[count] = cell;
                count += 1;
            }
        }
        (cells, count)
    }

    /// Zero-allocation stack variant of `clGetHitSectListMOVE` (`0x00138b60`).
    ///
    /// Returns `(cells, count)` with active cell indices in `cells[0..count]`.
    pub fn query_move_cells_array(&self, x: f32, y: f32, z: f32) -> ([usize; 16], usize) {
        let mut cells = [0usize; 16];
        let mut count = 0usize;
        if self.disable != 0 {
            return (cells, 0);
        }
        if !self.is_outdoor {
            cells[0] = 0;
            return (cells, 1);
        }

        let dx_block = x - self.origin_x;
        let dz_block = z - self.origin_z;
        let block_dist_sq = dx_block * dx_block + y * y + dz_block * dz_block;
        if block_dist_sq > OUTDOOR_BLOCK_CULL_DIST_SQ {
            return (cells, 0);
        }

        for row in 0..4usize {
            let cell_z = self.origin_z + (row as f32) * OUTDOOR_CELL_SIZE;
            let dz = z - cell_z;
            for col in 0..4usize {
                let cell_x = self.origin_x + (col as f32) * OUTDOOR_CELL_SIZE;
                let dx = x - cell_x;
                if dx * dx + dz * dz < OUTDOOR_CELL_CULL_DIST_SQ {
                    cells[count] = row * 4 + col;
                    count += 1;
                }
            }
        }
        (cells, count)
    }

    /// Authentic PS2/PC `clGetHitSectListMOVE` (`SLUS_202.28` `0x00138b60`, `0x00138bc0`,
    /// `0x00138df0` / `sh2pc.exe` `0x00524940`, `0x00524b10`).
    ///
    /// Populates `out_cells` with active sector indices (`0..16`) for movement collision at `(x, y, z)`:
    /// - Returns empty if `self.disable != 0` (`.cld` header `+0x1c`).
    /// - `InDoor` (`!self.is_outdoor`): emits cell `0`.
    /// - `OutDoor` (`self.is_outdoor`): culls block at `(x - sx)^2 + y^2 + (z - sz)^2 <= 30_000^2`,
    ///   then emits every cell `(row, col)` whose corner `(sx + col * 5000, sz + row * 5000)`
    ///   satisfies `(x - cx)^2 + (z - cz)^2 < 8_000^2`.
    pub fn query_move_cells(&self, x: f32, y: f32, z: f32, out_cells: &mut Vec<usize>) {
        out_cells.clear();
        let (cells, count) = self.query_move_cells_array(x, y, z);
        out_cells.extend_from_slice(&cells[..count]);
    }

    /// Map a 2D world coordinate `(x, z)` to a grid cell index `[0..15]`.
    #[inline]
    pub fn point_to_cell(&self, x: f32, z: f32) -> usize {
        if !self.is_outdoor {
            return 0;
        }
        let col = if self.cell_width > 1e-4 {
            ((x - self.origin_x) / self.cell_width).floor() as isize
        } else {
            0
        };
        let row = if self.cell_depth > 1e-4 {
            ((z - self.origin_z) / self.cell_depth).floor() as isize
        } else {
            0
        };

        let clamped_col = col.clamp(0, 3) as usize;
        let clamped_row = row.clamp(0, 3) as usize;
        clamped_row * 4 + clamped_col
    }

    /// Query cell and immediate 8-neighborhood for cylinder radius queries.
    pub fn query_neighborhood_cells(
        &self,
        x: f32,
        z: f32,
        radius: f32,
        out_cells: &mut Vec<usize>,
    ) {
        out_cells.clear();
        if self.disable != 0 {
            return;
        }
        if !self.is_outdoor {
            out_cells.push(0);
            return;
        }
        let min_cell = self.point_to_cell(x - radius, z - radius);
        let max_cell = self.point_to_cell(x + radius, z + radius);

        let min_col = min_cell % 4;
        let max_col = max_cell % 4;
        let min_row = min_cell / 4;
        let max_row = max_cell / 4;

        let (c_start, c_end) = if min_col <= max_col {
            (min_col, max_col)
        } else {
            (max_col, min_col)
        };
        let (r_start, r_end) = if min_row <= max_row {
            (min_row, max_row)
        } else {
            (max_row, min_row)
        };

        for r in r_start..=r_end {
            for c in c_start..=c_end {
                let cell = r * 4 + c;
                if !out_cells.contains(&cell) {
                    out_cells.push(cell);
                }
            }
        }
    }
}
