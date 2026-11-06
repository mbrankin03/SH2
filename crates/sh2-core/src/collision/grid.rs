//! Authentic 4x4 spatial grid acceleration partitioning (`clGetHitSectListMOVE`).
//!
//! On the original PS2, both indoor and outdoor stages were spatially partitioned into a
//! 4x4 sector grid (16 cells) defined in the `.cld` header:
//! - Origin: `(sx, sz)`
//! - Sector width/height: Outdoor blocks standard is 20,000 mm (20 meters) per block,
//!   divided into a 4x4 grid (5,000 mm per cell). Indoor rooms define their bounds from
//!   the `.cld` geometry extent.
//! - Cell index tables: `b0ofs[16]` (floors) and `b1ofs[16]` (walls) contain -1 terminated
//!   lists of polygon indices.

use sh2_assets::formats::cld::CldFile;

/// 4x4 Spatial acceleration grid matching PS2 `_CL_SELECT_MAP` and `.cld` structure.
#[derive(Debug, Clone, Default)]
pub struct SpatialGrid {
    /// Grid origin $(X, Z)$ in world units (mm).
    pub origin_x: f32,
    pub origin_z: f32,
    /// Cell dimensions in world units (mm).
    pub cell_width: f32,
    pub cell_depth: f32,
    /// Floor triangle indices per cell [row * 4 + col].
    pub floor_cells: [Vec<usize>; 16],
    /// Wall segment indices per cell [row * 4 + col].
    pub wall_cells: [Vec<usize>; 16],
}

impl SpatialGrid {
    /// Build spatial grid from authentic parsed `.cld` file.
    pub fn from_cld(cld: &CldFile) -> Self {
        let (origin_x, origin_z) = (cld.origin[0], cld.origin[1]);

        // Standard outdoor block size is 20,000 mm; 4x4 grid yields 5,000 mm per cell.
        // For indoor rooms, calculate bounding box extent if available, or fallback to 5,000 mm.
        let mut min_x = origin_x;
        let mut max_x = origin_x + 20000.0;
        let mut min_z = origin_z;
        let mut max_z = origin_z + 20000.0;

        if !cld.floors.is_empty() {
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

        let extent_x = (max_x - min_x).max(1000.0);
        let extent_z = (max_z - min_z).max(1000.0);
        let cell_width = extent_x / 4.0;
        let cell_depth = extent_z / 4.0;

        let has_cld_cells = cld.b0_cells.iter().any(|c| !c.is_empty())
            || cld.b1_cells.iter().any(|c| !c.is_empty());

        if has_cld_cells {
            // Remap floor indices if triangles were split from quads
            let mut floor_cells: [Vec<usize>; 16] = Default::default();
            for (cell_idx, indices) in cld.b0_cells.iter().enumerate() {
                for &idx in indices {
                    // In parser_cld, quads produce up to 2 floor triangles per poly.
                    // If index is within bounds, include it.
                    if idx < cld.floors.len() {
                        floor_cells[cell_idx].push(idx);
                    }
                    if idx * 2 < cld.floors.len() && !floor_cells[cell_idx].contains(&(idx * 2)) {
                        floor_cells[cell_idx].push(idx * 2);
                    }
                    if idx * 2 + 1 < cld.floors.len()
                        && !floor_cells[cell_idx].contains(&(idx * 2 + 1))
                    {
                        floor_cells[cell_idx].push(idx * 2 + 1);
                    }
                }
            }

            let mut wall_cells: [Vec<usize>; 16] = Default::default();
            for (cell_idx, indices) in cld.b1_cells.iter().enumerate() {
                for &idx in indices {
                    if idx < cld.walls.len() {
                        wall_cells[cell_idx].push(idx);
                    }
                }
            }

            Self {
                origin_x,
                origin_z,
                cell_width,
                cell_depth,
                floor_cells,
                wall_cells,
            }
        } else {
            // Fallback: bin static floors and walls into 4x4 spatial cells automatically
            let mut grid = Self {
                origin_x: min_x,
                origin_z: min_z,
                cell_width,
                cell_depth,
                floor_cells: Default::default(),
                wall_cells: Default::default(),
            };

            for (i, floor) in cld.floors.iter().enumerate() {
                let cx = (floor.v[0][0] + floor.v[1][0] + floor.v[2][0]) / 3.0;
                let cz = (floor.v[0][2] + floor.v[1][2] + floor.v[2][2]) / 3.0;
                let cell = grid.point_to_cell(cx, cz);
                grid.floor_cells[cell].push(i);
            }

            for (i, wall) in cld.walls.iter().enumerate() {
                let mx = (wall.start[0] + wall.end[0]) * 0.5;
                let mz = (wall.start[1] + wall.end[1]) * 0.5;
                let cell = grid.point_to_cell(mx, mz);
                grid.wall_cells[cell].push(i);
            }

            grid
        }
    }

    /// Map a 2D world coordinate $(x, z)$ to a grid cell index $[0..15]$.
    #[inline]
    pub fn point_to_cell(&self, x: f32, z: f32) -> usize {
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

    /// Query cell and immediate 8-neighborhood for cylinder radius queries (`clGetHitSectListMOVE`).
    pub fn query_neighborhood_cells(
        &self,
        x: f32,
        z: f32,
        radius: f32,
        out_cells: &mut Vec<usize>,
    ) {
        out_cells.clear();
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
