//! Authentic 4x4 spatial grid acceleration partitioning (`clGetHitSectListMOVE`).
//!
//! Ground truth from `SLUS_202.28` (`Collision/cl_main.c`):
//! - `clGetHitSectListMOVE` (`0x00138b60`): dispatches to `InDoor` or `OutDoor`.
//! - `clGetHitSectListMOVEInDoor` (`0x00138df0`): checks `header.disable == 0` (`+0x1c`),
//!   returns cell `0` only.
//! - `clGetHitSectListMOVEOutDoor` (`0x00138bc0` / `sh2pc.exe` `0x005249d0`): checks
//!   `header.disable == 0`, then block distance `(x - sx)^2 + y^2 + (z - sz)^2 <= 900_000_000.0`
//!   (`30_000.0^2`), then for each `(row, col)` in `0..4 x 0..4` with cell corner
//!   `(sx + col * 5000.0, sz + row * 5000.0)` checks `(x - cx)^2 + (z - cz)^2 < 64_000_000.0`
//!   (`8_000.0^2`), emitting cell `row * 4 + col`.

use sh2_assets::formats::cld::CldFile;

/// Outdoor `.cld` block cell pitch in world mm (`clGetHitSectListMOVEOutDoor` `0x00138bc0`).
pub const OUTDOOR_CELL_SIZE: f32 = 5000.0;

/// Outdoor `.cld` block cull radius squared (`30_000.0^2 = 9e8`, `0x00138bc0`).
pub const OUTDOOR_BLOCK_CULL_DIST_SQ: f32 = 900_000_000.0;

/// Outdoor `.cld` cell cull radius squared (`8_000.0^2 = 6.4e7`, `0x00138bc0`).
pub const OUTDOOR_CELL_CULL_DIST_SQ: f32 = 64_000_000.0;

/// 4x4 Spatial acceleration grid matching PS2 `_CL_SELECT_MAP` and `.cld` cell tables.
#[derive(Debug, Clone, Default)]
pub struct SpatialGrid {
    /// Grid origin `(sx, sz)` in world units (mm).
    pub origin_x: f32,
    pub origin_z: f32,
    /// Header `disable` flag (`+0x1c`). When non-zero, `clGetHitSectListMOVE` emits no cells.
    pub disable: i32,
    /// Whether this grid uses `clGetHitSectListMOVEOutDoor` (`true`) or `InDoor` cell 0 (`false`).
    pub is_outdoor: bool,
    /// Cell dimensions in world units (mm) for `point_to_cell` queries.
    pub cell_width: f32,
    pub cell_depth: f32,
    /// Floor triangle indices per cell `[row * 4 + col]` (mapped from `b0_cells` to `floors`).
    pub floor_cells: [Vec<usize>; 16],
    /// Raw wall (`wld`, group 1) polygon indices per cell (`b1ofs[16]`).
    pub wall_cells: [Vec<usize>; 16],
    /// Raw ceiling (`ced`, group 2) polygon indices per cell (`b2ofs[16]`).
    pub ceiling_cells: [Vec<usize>; 16],
    /// Raw side wall (`swd`, group 3) polygon indices per cell (`b3ofs[16]`).
    pub side_wall_cells: [Vec<usize>; 16],
    /// Raw static BG column (`cld`) indices per cell (`clofs[16]`).
    pub column_cells: [Vec<usize>; 16],
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

        if has_cld_cells {
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

            Self {
                origin_x,
                origin_z,
                disable: cld.disable,
                is_outdoor,
                cell_width,
                cell_depth,
                floor_cells,
                wall_cells: cld.b1_cells.clone(),
                ceiling_cells: cld.b2_cells.clone(),
                side_wall_cells: cld.b3_cells.clone(),
                column_cells: cld.cl_cells.clone(),
            }
        } else {
            // Fallback for synthetic worlds without explicit cell tables: place all in cell 0 (InDoor)
            let mut floor_cells: [Vec<usize>; 16] = Default::default();
            let mut wall_cells: [Vec<usize>; 16] = Default::default();
            let mut side_wall_cells: [Vec<usize>; 16] = Default::default();
            let mut column_cells: [Vec<usize>; 16] = Default::default();

            for i in 0..cld.floors.len() {
                floor_cells[0].push(i);
            }
            for i in 0..cld.raw_walls.len() {
                wall_cells[0].push(i);
            }
            for i in 0..cld.raw_side_walls.len() {
                side_wall_cells[0].push(i);
            }
            for i in 0..cld.columns.len() {
                column_cells[0].push(i);
            }

            Self {
                origin_x: min_x,
                origin_z: min_z,
                disable: cld.disable,
                is_outdoor: false,
                cell_width,
                cell_depth,
                floor_cells,
                wall_cells,
                ceiling_cells: Default::default(),
                side_wall_cells,
                column_cells,
            }
        }
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
    /// `0x00138df0` / `sh2pc.exe` `0x005249d0`, `0x00524b10`).
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
