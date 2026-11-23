//! Centralized CollisionController and CollisionWorld subsystem (`cl_main.c` / `cl_calc2.c`).
//!
//! Provides the primary simulation interface for:
//! - Static `.cld` background geometry (`raw_walls`, `raw_side_walls`, `columns`) and
//!   authentic `clGetHitSectListMOVE` broadphase (`SpatialGrid`).
//! - Dynamic extra walls/floors (`clAddDynamicWall`, `clAddDynamicFloor`).
//! - Character cylinder management and 5-iteration interleaved separation (`clCollectCharaALL`).
//! - 1:1 PS2/PC wall pushback relaxation (`clCheckBg2Chara` -> `clCheckColumn2WallHit` ->
//!   `clCheckColumn2ColumnHit` -> `clMakeWallHitCollectVector`).
//! - Floor height sampling and stair climbing (`clCollectCharaHeightNormal`).
//! - Camera ray clamping against room barriers (`clCheckHitEyes`).
//! - Safe spawn snapping (`col_spawn`) and room/block collision merging (`col_merge`).

use glam::Vec3;
use sh2_assets::formats::cld::{CldFile, FloorTriangle, HitPolyColumn, HitPolyPlane, WallSegment};

use super::character::{
    CharacterCylinder, CharacterId, CollisionColumn, JAMES_HIT_RADIUS, JAMES_MOVE_RADIUS,
    PLAYER_BASE_FORWARD_OFFSET, PLAYER_HIT_COL_HEIGHT, PLAYER_MOVE_COL_HEIGHT,
    PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
};
use super::dynamic::{DynamicFloor, DynamicWall};
use super::grid::SpatialGrid;
use super::raycast::{
    cl_check_sub_line_to_column_plus, cl_check_sub_line_to_plane, clamp_camera_ray,
    intersect_ray_triangle_3d, intersect_segment_bg_column, raycast_acoustic_occlusion,
    should_skip_material_for_mode, AcousticOcclusionHit, EyeHitKind, EyeRayHit, EyeRayMode,
    RaycastFilter,
};
use super::solver::{
    cl_check_column_to_bg_column_hit, cl_check_column_to_wall_hit, cl_collect_chara_all,
    cl_make_wall_hit_collect_vector, quantize_collect_vector_1_16, sh_atan2, CharaCollectEntry,
    WallCollectContext, WallHitRecord, CL_COLLECT_CHARA_ITERATIONS, CL_MAX_WALL_HITS,
    CL_NO_MOVE_ANGLE,
};

/// Surface material category extracted from `.cld` / `.sfm` floor collision triangles (`FloorTriangle::material`),
/// driving authentic material-dependent footstep sound selection (`0x54A9A0` / `0x5495C0` -> `0x5147E0` / `sddata.bin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SurfaceMaterial {
    /// Asphalt, concrete, and stone surfaces (`.cld` material `0, 12` -> `11001` -> `sddata[147]`).
    #[default]
    Concrete = 0,
    /// Wooden planks, bowling lanes, and floorboards (`.cld` material `7` -> `11005` -> `sddata[151, 95]`).
    Wood = 1,
    /// Metal grating, iron stairs, and boiler plates (`.cld` material `5, 9, 11` -> `11000` / `11006` -> `sddata[146, 143]`).
    Metal = 2,
    /// Carpet, rugs, and padded hotel/apartment hallways (`.cld` material `8` -> `11009` / `11054` -> `sddata[144, 145]`).
    Carpet = 3,
    /// Shallow water puddles, flooded basement, and sewers (`.cld` material `6` -> `11003` -> `sddata[148, 83, 84]`).
    Water = 4,
    /// Forest grass, brush, and foliage trails (`.cld` material `2` -> `11002` -> `sddata[142]`).
    Grass = 5,
    /// Dirt, gravel, loose rubble, and forest/cemetery paths (`.cld` material `1, 3, 4` -> `11004` / `11007` -> `sddata[150, 149]`).
    Gravel = 6,
    /// Viscous blood / broken glass / wet organic surfaces (`.cld` material `10` -> `11008` / `11003` -> `sddata[148, 141]`).
    Blood = 7,
}

impl SurfaceMaterial {
    /// Total number of distinct surface material categories.
    pub const COUNT: usize = 8;

    /// Ordered list of all surface material categories.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Concrete,
        Self::Wood,
        Self::Metal,
        Self::Carpet,
        Self::Water,
        Self::Grass,
        Self::Gravel,
        Self::Blood,
    ];

    /// Maps a [`SurfaceMaterial`] directly from its 0-based index (`0..=7`), used by debug UI and synthetic test pads.
    #[inline]
    pub const fn from_index(idx: u8) -> Self {
        match idx & 0x07 {
            0 => Self::Concrete,
            1 => Self::Wood,
            2 => Self::Metal,
            3 => Self::Carpet,
            4 => Self::Water,
            5 => Self::Grass,
            6 => Self::Gravel,
            _ => Self::Blood,
        }
    }

    /// Maps a raw `.cld` `FloorTriangle::material` attribute (`u32`) to a [`SurfaceMaterial`].
    ///
    /// Matches `sh2pc.exe` `0x54A9A0` / `0x54AFC1` (James) and `0x5495C0` (Maria) switch table (`0..=12`).
    #[inline]
    pub const fn from_cld_material(raw: u32) -> Self {
        if (raw & 0x80) != 0 {
            return Self::from_index((raw & 0x07) as u8);
        }
        match raw & 0x0F {
            0 | 12 => Self::Concrete,
            1 | 3 | 4 => Self::Gravel,
            2 => Self::Grass,
            5 | 9 | 11 => Self::Metal,
            6 => Self::Water,
            7 => Self::Wood,
            8 => Self::Carpet,
            10 => Self::Blood,
            _ => Self::from_index((raw & 0x07) as u8),
        }
    }

    /// Cycles to the next surface material (useful for interactive audio debug testing).
    #[inline]
    pub const fn next(self) -> Self {
        Self::from_index((self as u8).wrapping_add(1))
    }

    /// Human-readable label for debug HUD display.
    #[inline]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Concrete => "Concrete / Stone",
            Self::Wood => "Wood / Planks",
            Self::Metal => "Metal Grating",
            Self::Carpet => "Carpet / Rug",
            Self::Water => "Water / Puddle",
            Self::Grass => "Grass / Foliage",
            Self::Gravel => "Dirt / Gravel",
            Self::Blood => "Wet / Glass / Blood",
        }
    }
}

/// Result of sampling the walkable floor beneath a character at `(X, Z)`, including both height and surface material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorSample {
    /// Walkable floor height `Y` in world units (`+Y` points downward).
    pub height_y: f32,
    /// Raw `.cld` floor triangle material attribute (`FloorTriangle::material`).
    pub raw_material: u32,
    /// Classified acoustic surface material for footstep SFX dispatch.
    pub surface: SurfaceMaterial,
}

/// Configuration constants for character collision and kinematics.
///
/// Defaults match `PlayerSetColumn_SetTarget` (`SLUS_202.28` `0x00107d40` / `sh2pc.exe` `0x0052f110`)
/// and `clCollectCharaALL` (`0x00132e20`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionConfig {
    /// Movement collision cylinder radius in world mm (`175.0` James, `150.0` playable Maria).
    pub radius: f32,
    /// Movement collision cylinder height in world mm (`800.0`: `[-50.0 .. -850.0]`).
    pub height: f32,
    /// Movement cylinder bottom Y offset relative to feet (`-50.0` mm).
    pub y_bottom_offset: f32,
    /// Combat hitbox cylinder radius in world mm (`150.0` James, `125.00001` playable Maria).
    pub hit_radius: f32,
    /// Combat hitbox cylinder height in world mm (`900.0`: `[0.0 .. -900.0]`).
    pub hit_height: f32,
    /// Forward cylinder offset along character yaw in world mm (`+25.0` mm).
    pub forward_offset: f32,
    /// Maximum vertical climb/step-up threshold (`250.0` mm).
    pub max_step_up: f32,
    /// Maximum vertical drop/step-down threshold (`350.0` mm).
    pub max_step_down: f32,
    /// Number of interleaved relaxation iterations (`5`, `clCollectCharaALL` `0x00132e20`).
    pub max_iterations: usize,
}

impl Default for CollisionConfig {
    fn default() -> Self {
        Self {
            radius: JAMES_MOVE_RADIUS,
            height: PLAYER_MOVE_COL_HEIGHT,
            y_bottom_offset: PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
            hit_radius: JAMES_HIT_RADIUS,
            hit_height: PLAYER_HIT_COL_HEIGHT,
            forward_offset: PLAYER_BASE_FORWARD_OFFSET,
            max_step_up: 250.0,
            max_step_down: 350.0,
            max_iterations: CL_COLLECT_CHARA_ITERATIONS,
        }
    }
}

/// Static and dynamic collision state representing an active map room or combined streaming sectors.
#[derive(Debug, Clone, Default)]
pub struct CollisionWorld {
    /// Room/block origin coordinates `(sx, sz)` in world mm.
    pub origin: [f32; 2],
    /// Header `disable` flag (`+0x1c`). When non-zero, `clGetHitSectListMOVE` skips this `.cld`.
    pub disable: i32,
    /// Floor triangles for height sampling and stepping.
    pub floors: Vec<FloorTriangle>,
    /// Derived 2D wall segments (for legacy raycast/audio consumers).
    pub walls: Vec<WallSegment>,
    /// Raw `_CL_HITPOLY_PLANE` floors (`fld`, group 0), indexed 1:1 by `grid.floor_cells`.
    pub raw_floors: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_PLANE` walls (`wld`, group 1), indexed 1:1 by `grid.wall_cells`.
    pub raw_walls: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_PLANE` ceilings (`ced`, group 2), indexed 1:1 by `grid.ceiling_cells`.
    pub raw_ceilings: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_PLANE` side walls (`swd`, group 3), indexed 1:1 by `grid.side_wall_cells`.
    pub raw_side_walls: Vec<HitPolyPlane>,
    /// Raw `_CL_HITPOLY_COLUMN` static BG columns (`cld`), indexed 1:1 by `grid.column_cells`.
    pub columns: Vec<HitPolyColumn>,
    /// Extra transient or stage-added wall segments (`s_extra`).
    pub extra_walls: Vec<WallSegment>,
    /// Dynamic stage wall obstacles (`clAddDynamicWall`).
    pub dynamic_walls: Vec<DynamicWall>,
    /// Dynamic stage floor obstacles (`clAddDynamicFloor`).
    pub dynamic_floors: Vec<DynamicFloor>,
    /// Authentic 4x4 spatial acceleration broadphase grid (`clGetHitSectListMOVE`).
    pub grid: SpatialGrid,
}

impl CollisionWorld {
    /// Create an empty collision world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load collision geometry from a parsed `.cld` file.
    pub fn from_cld(cld: &CldFile) -> Self {
        let grid = SpatialGrid::from_cld(cld);
        Self {
            origin: cld.origin,
            disable: cld.disable,
            floors: cld.floors.clone(),
            walls: cld.walls.clone(),
            raw_floors: cld.raw_floors.clone(),
            raw_walls: cld.raw_walls.clone(),
            raw_ceilings: cld.raw_ceilings.clone(),
            raw_side_walls: cld.raw_side_walls.clone(),
            columns: cld.columns.clone(),
            extra_walls: Vec::new(),
            dynamic_walls: Vec::new(),
            dynamic_floors: Vec::new(),
            grid,
        }
    }

    /// Construct a procedural room collision environment matching the fallback boiler room.
    pub fn create_synthetic_room(w: f32, l: f32, h: f32, pw: f32, pl: f32) -> Self {
        let cld = CldFile::create_synthetic_room(w, l, h, pw, pl);
        Self::from_cld(&cld)
    }

    /// Clear extra dynamic wall segments (`col_extra_clear`).
    pub fn clear_extra_walls(&mut self) {
        self.extra_walls.clear();
        self.dynamic_walls.clear();
        self.dynamic_floors.clear();
    }

    /// Add an extra transient wall segment (`col_extra_wall`).
    pub fn add_extra_wall(&mut self, wall: WallSegment) {
        self.extra_walls.push(wall);
    }

    /// Add a dynamic stage wall obstacle (`clAddDynamicWall`).
    pub fn add_dynamic_wall(&mut self, wall: DynamicWall) {
        self.dynamic_walls.push(wall);
    }

    /// Add a dynamic stage floor obstacle (`clAddDynamicFloor`).
    pub fn add_dynamic_floor(&mut self, floor: DynamicFloor) {
        self.dynamic_floors.push(floor);
    }

    /// Iterate over all active walls (static + extra + dynamic).
    pub fn all_walls(&self) -> impl Iterator<Item = &WallSegment> {
        self.walls.iter().chain(self.extra_walls.iter()).chain(
            self.dynamic_walls
                .iter()
                .filter(|dw| dw.active)
                .map(|dw| &dw.segment),
        )
    }

    /// Merge multiple collision worlds into a single combined environment (`col_merge`).
    pub fn merge(parts: &[&CollisionWorld]) -> Self {
        let mut merged = Self::new();
        for p in parts {
            merged.floors.extend_from_slice(&p.floors);
            merged.walls.extend_from_slice(&p.walls);
            merged.raw_floors.extend_from_slice(&p.raw_floors);
            merged.raw_walls.extend_from_slice(&p.raw_walls);
            merged.raw_ceilings.extend_from_slice(&p.raw_ceilings);
            merged.raw_side_walls.extend_from_slice(&p.raw_side_walls);
            merged.columns.extend_from_slice(&p.columns);
            merged.extra_walls.extend_from_slice(&p.extra_walls);
            merged.dynamic_walls.extend_from_slice(&p.dynamic_walls);
            merged.dynamic_floors.extend_from_slice(&p.dynamic_floors);
        }

        // Rebuild spatial grid for merged geometry
        let cld = CldFile {
            origin: [0.0, 0.0],
            floors: merged.floors.clone(),
            walls: merged.walls.clone(),
            raw_floors: merged.raw_floors.clone(),
            raw_walls: merged.raw_walls.clone(),
            raw_ceilings: merged.raw_ceilings.clone(),
            raw_side_walls: merged.raw_side_walls.clone(),
            columns: merged.columns.clone(),
            ..Default::default()
        };
        merged.grid = SpatialGrid::from_cld(&cld);
        merged
    }

    /// Sample the walkable floor height and surface material at `(x, z)` closest to `y_ref` within step boundaries.
    ///
    /// Authentic PS2 `clCollectCharaHeightNormal`:
    /// Because world `+Y` points downward:
    /// - `up` is above feet: `y >= y_ref - up`
    /// - `down` is below feet: `y <= y_ref + down`
    ///
    /// Returns the highest floor (minimum numeric `Y`) within range along with its `FloorTriangle::material`.
    pub fn sample_floor_surface(
        &self,
        x: f32,
        z: f32,
        y_ref: f32,
        up: f32,
        down: f32,
    ) -> Option<FloorSample> {
        let mut best: Option<FloorSample> = None;

        // 1. Broadphase: query floor triangles in the current grid cell (gated on `disable == 0`)
        if self.disable == 0 {
            let cell = self.grid.point_to_cell(x, z);
            let cell_tri_indices = &self.grid.floor_cells[cell];

            if !cell_tri_indices.is_empty() {
                for &idx in cell_tri_indices {
                    if let Some(tri) = self.floors.get(idx) {
                        if let Some(y) = tri.sample_height(x, z) {
                            if y >= y_ref - up && y <= y_ref + down {
                                match best {
                                    None => {
                                        best = Some(FloorSample {
                                            height_y: y,
                                            raw_material: tri.material,
                                            surface: SurfaceMaterial::from_cld_material(
                                                tri.material,
                                            ),
                                        });
                                    }
                                    Some(prev) if y < prev.height_y => {
                                        best = Some(FloorSample {
                                            height_y: y,
                                            raw_material: tri.material,
                                            surface: SurfaceMaterial::from_cld_material(
                                                tri.material,
                                            ),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            } else {
                // Broadphase cell empty or unindexed: fallback to all static floors
                for tri in &self.floors {
                    if let Some(y) = tri.sample_height(x, z) {
                        if y >= y_ref - up && y <= y_ref + down {
                            match best {
                                None => {
                                    best = Some(FloorSample {
                                        height_y: y,
                                        raw_material: tri.material,
                                        surface: SurfaceMaterial::from_cld_material(tri.material),
                                    });
                                }
                                Some(prev) if y < prev.height_y => {
                                    best = Some(FloorSample {
                                        height_y: y,
                                        raw_material: tri.material,
                                        surface: SurfaceMaterial::from_cld_material(tri.material),
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // 2. Check dynamic floor obstacles
        for df in &self.dynamic_floors {
            if !df.active {
                continue;
            }
            if let Some(y) = df.triangle.sample_height(x, z) {
                if y >= y_ref - up && y <= y_ref + down {
                    match best {
                        None => {
                            best = Some(FloorSample {
                                height_y: y,
                                raw_material: df.triangle.material,
                                surface: SurfaceMaterial::from_cld_material(df.triangle.material),
                            });
                        }
                        Some(prev) if y < prev.height_y => {
                            best = Some(FloorSample {
                                height_y: y,
                                raw_material: df.triangle.material,
                                surface: SurfaceMaterial::from_cld_material(df.triangle.material),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        best
    }

    /// Sample the walkable floor height at `(x, z)` closest to `y_ref` within step boundaries.
    pub fn sample_floor_height(
        &self,
        x: f32,
        z: f32,
        y_ref: f32,
        up: f32,
        down: f32,
    ) -> Option<f32> {
        self.sample_floor_surface(x, z, y_ref, up, down)
            .map(|s| s.height_y)
    }

    /// Exact 1:1 `clCheckBg2Chara` (`SLUS_202.28` `0x00133920` / `sh2pc.exe` `0x00525c90`):
    ///
    /// 1. Checks dynamic & extra walls via `clCheckColumn2WallHit`.
    /// 2. Queries active cells via `SpatialGrid::query_move_cells` (`clGetHitSectListMOVE`).
    /// 3. Checks `raw_walls` (`b1ofs`), `raw_side_walls` (`b3ofs`), and static BG `columns` (`clofs`).
    /// 4. Combines hits via `clMakeWallHitCollectVector` and quantizes to `1/16` mm (`_vftoi4`/`_vitof4`).
    pub fn check_bg_to_chara(&self, col: &CollisionColumn, ctx: &mut WallCollectContext) -> Vec3 {
        if !col.active {
            return Vec3::ZERO;
        }

        let mut hits = [WallHitRecord::EMPTY; CL_MAX_WALL_HITS];
        let mut hit_count = 0usize;

        // 1. Dynamic walls (`clCheckHitDynamicWallCollision` 0x00133fb0) & extra walls
        for dw in &self.dynamic_walls {
            if !dw.active || hit_count >= CL_MAX_WALL_HITS {
                continue;
            }
            let plane = HitPolyPlane::from_wall_segment(&dw.segment);
            if let Some(hit) = cl_check_column_to_wall_hit(&plane, col) {
                hits[hit_count] = hit;
                hit_count += 1;
            }
        }
        for ew in &self.extra_walls {
            if hit_count >= CL_MAX_WALL_HITS {
                break;
            }
            let plane = HitPolyPlane::from_wall_segment(ew);
            if let Some(hit) = cl_check_column_to_wall_hit(&plane, col) {
                hits[hit_count] = hit;
                hit_count += 1;
            }
        }

        // 2. SpatialGrid broadphase (`clGetHitSectListMOVE` 0x00138b60)
        if self.disable == 0 {
            let (active_cells, cell_count) =
                self.grid
                    .query_move_cells_array(col.center_x, col.y_bottom, col.center_z);
            let active_slice = &active_cells[..cell_count];

            if !self.raw_walls.is_empty()
                || !self.raw_side_walls.is_empty()
                || !self.columns.is_empty()
            {
                // Pass A: group 1 walls (`wldofs` + `b1ofs`)
                for &cell in active_slice {
                    for &idx in &self.grid.wall_cells[cell] {
                        if hit_count >= CL_MAX_WALL_HITS {
                            break;
                        }
                        if let Some(plane) = self.raw_walls.get(idx) {
                            if let Some(hit) = cl_check_column_to_wall_hit(plane, col) {
                                hits[hit_count] = hit;
                                hit_count += 1;
                            }
                        }
                    }
                }
                // Pass B: group 3 side walls (`swdofs` + `b3ofs`)
                for &cell in active_slice {
                    for &idx in &self.grid.side_wall_cells[cell] {
                        if hit_count >= CL_MAX_WALL_HITS {
                            break;
                        }
                        if let Some(plane) = self.raw_side_walls.get(idx) {
                            if let Some(hit) = cl_check_column_to_wall_hit(plane, col) {
                                hits[hit_count] = hit;
                                hit_count += 1;
                            }
                        }
                    }
                }
                // Pass C: static BG columns (`cldofs` + `clofs`, `clCheckColumn2ColumnHit` 0x00134f60)
                for &cell in active_slice {
                    for &idx in &self.grid.column_cells[cell] {
                        if hit_count >= CL_MAX_WALL_HITS {
                            break;
                        }
                        if let Some(bg_col) = self.columns.get(idx) {
                            if let Some(hit) = cl_check_column_to_bg_column_hit(bg_col, col) {
                                hits[hit_count] = hit;
                                hit_count += 1;
                            }
                        }
                    }
                }
            } else if !self.walls.is_empty() {
                // Legacy fallback for worlds constructed manually via `world.walls.push(...)`
                for w in &self.walls {
                    if w.group == 2 || hit_count >= CL_MAX_WALL_HITS {
                        continue;
                    }
                    let plane = HitPolyPlane::from_wall_segment(w);
                    if let Some(hit) = cl_check_column_to_wall_hit(&plane, col) {
                        hits[hit_count] = hit;
                        hit_count += 1;
                    }
                }
            }
        }

        let (collect_vec, applied) = cl_make_wall_hit_collect_vector(&mut hits[..hit_count], ctx);
        if applied == 0 {
            Vec3::ZERO
        } else {
            quantize_collect_vector_1_16(collect_vec)
        }
    }

    /// Resolve lateral wall & static BG column collisions for a vertical cylinder using
    /// `clCheckBg2Chara` (`SpatialGrid` + `clCheckColumn2WallHit` + `clCheckColumn2ColumnHit` +
    /// `clMakeWallHitCollectVector`).
    pub fn resolve_wall_pushback(&self, pos: &mut Vec3, config: &CollisionConfig) -> bool {
        let mut pushed = false;
        let prev_pos = *pos;

        for _iter in 0..config.max_iterations {
            let y_bottom = pos.y + config.y_bottom_offset;
            let y_top = y_bottom - config.height;
            let col = CollisionColumn::new(pos.x, y_bottom, pos.z, y_top, config.radius, 1);

            let mut ctx = WallCollectContext {
                move_angle: CL_NO_MOVE_ANGLE,
                cur_pos: *pos,
                prev_pos,
                chara_type: 1,
                post_flag_timer: 0,
                fps: 60.0,
            };

            let push = self.check_bg_to_chara(&col, &mut ctx);
            if push.x == 0.0 && push.z == 0.0 {
                break;
            }
            pos.x += push.x;
            pos.z += push.z;
            pushed = true;
        }

        pushed
    }

    /// Full kinematic collision update for character position:
    /// 1. Continuous anti-tunneling sweep against walls in active `SpatialGrid` cells.
    /// 2. 1:1 `clCheckBg2Chara` lateral wall & BG column pushback and sliding.
    /// 3. Vertical floor height snapping and step traversal.
    pub fn resolve_character_movement(
        &self,
        current_pos: Vec3,
        desired_pos: Vec3,
        config: &CollisionConfig,
    ) -> Vec3 {
        let mut pos = desired_pos;

        let move_dx = desired_pos.x - current_pos.x;
        let move_dz = desired_pos.z - current_pos.z;
        let move_dist_sq = move_dx * move_dx + move_dz * move_dz;
        let move_angle = if move_dx == 0.0 && move_dz == 0.0 {
            CL_NO_MOVE_ANGLE
        } else {
            sh_atan2(move_dx, move_dz)
        };

        // 1. Anti-tunneling segment check for large single-step displacements (> radius)
        if move_dist_sq > config.radius * config.radius {
            let y_bottom = current_pos.y + config.y_bottom_offset;
            let y_top = y_bottom - config.height;
            let mut earliest_hit: Option<(f32, f32, f32)> = None;

            for w in self.all_walls() {
                if w.group == 2 || w.y_top >= y_bottom || w.y_bottom <= y_top {
                    continue;
                }

                let rx = move_dx;
                let rz = move_dz;
                let sx = w.end[0] - w.start[0];
                let sz = w.end[1] - w.start[1];

                let den = rx * sz - rz * sx;
                if den.abs() < 1e-6 {
                    continue;
                }

                let t =
                    ((w.start[0] - current_pos.x) * sz - (w.start[1] - current_pos.z) * sx) / den;
                let u =
                    ((w.start[0] - current_pos.x) * rz - (w.start[1] - current_pos.z) * rx) / den;

                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    let hit_x = current_pos.x + rx * t;
                    let hit_z = current_pos.z + rz * t;
                    match earliest_hit {
                        None => earliest_hit = Some((t, hit_x, hit_z)),
                        Some((prev_t, _, _)) if t < prev_t => {
                            earliest_hit = Some((t, hit_x, hit_z));
                        }
                        _ => {}
                    }
                }
            }

            if let Some((_hit_t, hit_x, hit_z)) = earliest_hit {
                let move_len = move_dist_sq.sqrt();
                let dir_x = move_dx / move_len;
                let dir_z = move_dz / move_len;
                pos.x = hit_x - dir_x * config.radius;
                pos.z = hit_z - dir_z * config.radius;
            }
        }

        // 2. 1:1 `clCheckBg2Chara` lateral wall & static BG column pushback
        for _iter in 0..config.max_iterations {
            let y_bottom = pos.y + config.y_bottom_offset;
            let y_top = y_bottom - config.height;
            let col = CollisionColumn::new(pos.x, y_bottom, pos.z, y_top, config.radius, 1);

            let mut ctx = WallCollectContext {
                move_angle,
                cur_pos: pos,
                prev_pos: current_pos,
                chara_type: 1,
                post_flag_timer: 0,
                fps: 60.0,
            };

            let push = self.check_bg_to_chara(&col, &mut ctx);
            if push.x == 0.0 && push.z == 0.0 {
                break;
            }
            pos.x += push.x;
            pos.z += push.z;
        }

        // 3. Sample floor height
        if let Some(floor_y) = self.sample_floor_height(
            pos.x,
            pos.z,
            current_pos.y,
            config.max_step_up,
            config.max_step_down,
        ) {
            pos.y = floor_y;
        }

        pos
    }

    /// Internal helper: test a slice of `_CL_HITPOLY_PLANE` against segment `origin -> target`.
    fn test_eye_planes(
        origin: Vec3,
        target: Vec3,
        planes: &[HitPolyPlane],
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        for plane in planes {
            if should_skip_material_for_mode(plane.material, mode) {
                continue;
            }
            if let Some((t, point, normal)) = cl_check_sub_line_to_plane(origin, target, plane) {
                let dist_sq = origin.distance_squared(point);
                if dist_sq < *best_dist_sq {
                    *best_dist_sq = dist_sq;
                    *best_hit = Some(EyeRayHit {
                        kind: EyeHitKind::Plane,
                        point,
                        normal,
                        t,
                        dist_sq,
                        material: plane.material,
                        character_id: None,
                    });
                }
            }
        }
    }

    /// Internal helper: test dynamic floors (`clCheckHitEyeVectorDynamicFloor` `0x0013a200`).
    fn test_eye_dynamic_floors(
        &self,
        origin: Vec3,
        target: Vec3,
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        for df in &self.dynamic_floors {
            if !df.active || should_skip_material_for_mode(df.triangle.material, mode) {
                continue;
            }
            if let Some(t) = intersect_ray_triangle_3d(origin, target, &df.triangle) {
                let point = origin + (target - origin) * t;
                let dist_sq = origin.distance_squared(point);
                if dist_sq < *best_dist_sq {
                    let v0 = Vec3::from_array(df.triangle.v[0]);
                    let v1 = Vec3::from_array(df.triangle.v[1]);
                    let v2 = Vec3::from_array(df.triangle.v[2]);
                    let normal = (v2 - v0).cross(v1 - v0).normalize_or_zero();
                    *best_dist_sq = dist_sq;
                    *best_hit = Some(EyeRayHit {
                        kind: EyeHitKind::Plane,
                        point,
                        normal,
                        t,
                        dist_sq,
                        material: df.triangle.material,
                        character_id: None,
                    });
                }
            }
        }
    }

    /// Internal helper: test dynamic & extra walls (`clCheckHitEyeVectorDynamicWall` `0x00139e70`).
    fn test_eye_dynamic_walls(
        &self,
        origin: Vec3,
        target: Vec3,
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        for dw in &self.dynamic_walls {
            if !dw.active || should_skip_material_for_mode(dw.segment.material, mode) {
                continue;
            }
            let plane = HitPolyPlane::from_wall_segment(&dw.segment);
            if let Some((t, point, normal)) = cl_check_sub_line_to_plane(origin, target, &plane) {
                let dist_sq = origin.distance_squared(point);
                if dist_sq < *best_dist_sq {
                    *best_dist_sq = dist_sq;
                    *best_hit = Some(EyeRayHit {
                        kind: EyeHitKind::Plane,
                        point,
                        normal,
                        t,
                        dist_sq,
                        material: dw.segment.material,
                        character_id: None,
                    });
                }
            }
        }
        for ew in &self.extra_walls {
            if should_skip_material_for_mode(ew.material, mode) {
                continue;
            }
            let plane = HitPolyPlane::from_wall_segment(ew);
            if let Some((t, point, normal)) = cl_check_sub_line_to_plane(origin, target, &plane) {
                let dist_sq = origin.distance_squared(point);
                if dist_sq < *best_dist_sq {
                    *best_dist_sq = dist_sq;
                    *best_hit = Some(EyeRayHit {
                        kind: EyeHitKind::Plane,
                        point,
                        normal,
                        t,
                        dist_sq,
                        material: ew.material,
                        character_id: None,
                    });
                }
            }
        }
    }

    /// Internal helper: test static `.cld` floors (preferring `raw_floors`, falling back to `floors`).
    fn test_eye_static_floors(
        &self,
        origin: Vec3,
        target: Vec3,
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        if self.disable != 0 {
            return;
        }
        if !self.raw_floors.is_empty() {
            Self::test_eye_planes(
                origin,
                target,
                &self.raw_floors,
                mode,
                best_dist_sq,
                best_hit,
            );
        } else {
            for tri in &self.floors {
                if should_skip_material_for_mode(tri.material, mode) {
                    continue;
                }
                if let Some(t) = intersect_ray_triangle_3d(origin, target, tri) {
                    let point = origin + (target - origin) * t;
                    let dist_sq = origin.distance_squared(point);
                    if dist_sq < *best_dist_sq {
                        let v0 = Vec3::from_array(tri.v[0]);
                        let v1 = Vec3::from_array(tri.v[1]);
                        let v2 = Vec3::from_array(tri.v[2]);
                        let normal = (v2 - v0).cross(v1 - v0).normalize_or_zero();
                        *best_dist_sq = dist_sq;
                        *best_hit = Some(EyeRayHit {
                            kind: EyeHitKind::Plane,
                            point,
                            normal,
                            t,
                            dist_sq,
                            material: tri.material,
                            character_id: None,
                        });
                    }
                }
            }
        }
    }

    /// Internal helper: test static BG columns (`clCheckHitEyeVectorBGColumn` `0x00139d40`).
    fn test_eye_bg_columns(
        &self,
        origin: Vec3,
        target: Vec3,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        if self.disable != 0 {
            return;
        }
        for col in &self.columns {
            if let Some((t, point)) = intersect_segment_bg_column(origin, target, col) {
                let dist_sq = origin.distance_squared(point);
                if dist_sq < *best_dist_sq {
                    *best_dist_sq = dist_sq;
                    *best_hit = Some(EyeRayHit {
                        kind: EyeHitKind::BgColumn,
                        point,
                        normal: Vec3::ZERO,
                        t,
                        dist_sq,
                        material: 0,
                        character_id: None,
                    });
                }
            }
        }
    }

    /// Exact 1:1 3D `clCheckHitEyes` (`SLUS_202.28` `0x00138ea0`):
    ///
    /// Dispatches:
    /// - `EyeRayMode::NoThru` (`0` -> `clCheckHitEyeVectorNoThru` `0x00139680`): skips `material == 12`,
    ///   tests dynamic walls/floors, static `fld`/`wld`/`ced`, BG columns, and characters.
    /// - `EyeRayMode::Thru` (`1` -> `clCheckHitEyeVector` `0x00139420`): skips `Thru` materials `9, 11, 12`,
    ///   tests dynamic walls/floors, static `fld`/`wld`/`ced`, BG columns, and characters.
    /// - `EyeRayMode::AllNoThru` (`2` -> `clCheckHitEyeVectorAllNoThru` `0x001398e0`): skips nothing,
    ///   tests dynamic walls/floors, static `fld`/`wld`/`swd`/`ced`, BG columns, and characters.
    pub fn check_hit_eyes(
        &self,
        origin: Vec3,
        target: Vec3,
        mode: EyeRayMode,
        exclude_character: Option<CharacterId>,
        characters: &[CharacterCylinder],
    ) -> Option<EyeRayHit> {
        let mut best_dist_sq = origin.distance_squared(target);
        let mut best_hit: Option<EyeRayHit> = None;

        // 1. Dynamic walls & floors
        self.test_eye_dynamic_walls(origin, target, mode, &mut best_dist_sq, &mut best_hit);
        self.test_eye_dynamic_floors(origin, target, mode, &mut best_dist_sq, &mut best_hit);

        // 2. Static geometry (`fld`, `wld`, optional `swd`, `ced`, `cld` BG columns)
        if self.disable == 0 {
            self.test_eye_static_floors(origin, target, mode, &mut best_dist_sq, &mut best_hit);

            if !self.raw_walls.is_empty()
                || !self.raw_ceilings.is_empty()
                || !self.raw_side_walls.is_empty()
            {
                Self::test_eye_planes(
                    origin,
                    target,
                    &self.raw_walls,
                    mode,
                    &mut best_dist_sq,
                    &mut best_hit,
                );
                if mode == EyeRayMode::AllNoThru {
                    Self::test_eye_planes(
                        origin,
                        target,
                        &self.raw_side_walls,
                        mode,
                        &mut best_dist_sq,
                        &mut best_hit,
                    );
                }
                Self::test_eye_planes(
                    origin,
                    target,
                    &self.raw_ceilings,
                    mode,
                    &mut best_dist_sq,
                    &mut best_hit,
                );
            } else {
                // Fallback for worlds constructed directly via `world.walls`
                for w in &self.walls {
                    if w.group == 3 && mode != EyeRayMode::AllNoThru {
                        continue;
                    }
                    if should_skip_material_for_mode(w.material, mode) {
                        continue;
                    }
                    let plane = HitPolyPlane::from_wall_segment(w);
                    if let Some((t, point, normal)) =
                        cl_check_sub_line_to_plane(origin, target, &plane)
                    {
                        let dist_sq = origin.distance_squared(point);
                        if dist_sq < best_dist_sq {
                            best_dist_sq = dist_sq;
                            best_hit = Some(EyeRayHit {
                                kind: EyeHitKind::Plane,
                                point,
                                normal,
                                t,
                                dist_sq,
                                material: w.material,
                                character_id: None,
                            });
                        }
                    }
                }
            }

            self.test_eye_bg_columns(origin, target, &mut best_dist_sq, &mut best_hit);
        }

        // 3. Active character hit cylinders (`clCheckHitEyeVectorCharacter` `0x0013a590`)
        for c in characters {
            if exclude_character == Some(c.id) || !c.active {
                continue;
            }
            let hc = c.hit_column();
            if !hc.active {
                continue;
            }
            if let Some((t, point)) = cl_check_sub_line_to_column_plus(
                origin,
                target,
                hc.center_x,
                hc.center_z,
                hc.y_top,
                hc.y_bottom,
                hc.radius,
            ) {
                let dist_sq = origin.distance_squared(point);
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_hit = Some(EyeRayHit {
                        kind: EyeHitKind::Character,
                        point,
                        normal: Vec3::ZERO,
                        t,
                        dist_sq,
                        material: 0,
                        character_id: Some(c.id),
                    });
                }
            }
        }

        best_hit
    }

    /// Exact 1:1 `clCheckHitEyesOnlyFloor` (`SLUS_202.28` `0x00138f10`):
    /// dynamic floors + static `fld` floors, skipping only `material == 12` (`clCheckHitSwordVectorWall`).
    pub fn check_hit_eyes_only_floor(&self, origin: Vec3, target: Vec3) -> Option<EyeRayHit> {
        let mut best_dist_sq = origin.distance_squared(target);
        let mut best_hit: Option<EyeRayHit> = None;
        self.test_eye_dynamic_floors(
            origin,
            target,
            EyeRayMode::NoThru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        self.test_eye_static_floors(
            origin,
            target,
            EyeRayMode::NoThru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        best_hit
    }

    /// Exact 1:1 `clCheckHitEyesOnlyFloorThru` (`SLUS_202.28` `0x00139010`):
    /// dynamic floors + static `fld` floors, skipping `Thru` materials `9, 11, 12`.
    pub fn check_hit_eyes_only_floor_thru(&self, origin: Vec3, target: Vec3) -> Option<EyeRayHit> {
        let mut best_dist_sq = origin.distance_squared(target);
        let mut best_hit: Option<EyeRayHit> = None;
        self.test_eye_dynamic_floors(
            origin,
            target,
            EyeRayMode::Thru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        self.test_eye_static_floors(
            origin,
            target,
            EyeRayMode::Thru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        best_hit
    }

    /// Exact 1:1 `clCheckHitEyesOnlyWall` (`SLUS_202.28` `0x00139110`):
    /// dynamic walls + static `wld` (skip `12`) + `ced` (skip `9/11/12`) + static BG columns.
    pub fn check_hit_eyes_only_wall(&self, origin: Vec3, target: Vec3) -> Option<EyeRayHit> {
        let mut best_dist_sq = origin.distance_squared(target);
        let mut best_hit: Option<EyeRayHit> = None;
        self.test_eye_dynamic_walls(
            origin,
            target,
            EyeRayMode::NoThru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        if self.disable == 0 {
            Self::test_eye_planes(
                origin,
                target,
                &self.raw_walls,
                EyeRayMode::NoThru,
                &mut best_dist_sq,
                &mut best_hit,
            );
            Self::test_eye_planes(
                origin,
                target,
                &self.raw_ceilings,
                EyeRayMode::Thru,
                &mut best_dist_sq,
                &mut best_hit,
            );
            self.test_eye_bg_columns(origin, target, &mut best_dist_sq, &mut best_hit);
        }
        best_hit
    }

    /// Exact 1:1 `clCheckHitEyesOnlyFloorCeil` (`SLUS_202.28` `0x001392c0`):
    /// dynamic floors + static `fld` & `ced`, skipping `Thru` materials `9, 11, 12`.
    pub fn check_hit_eyes_only_floor_ceil(&self, origin: Vec3, target: Vec3) -> Option<EyeRayHit> {
        let mut best_dist_sq = origin.distance_squared(target);
        let mut best_hit: Option<EyeRayHit> = None;
        self.test_eye_dynamic_floors(
            origin,
            target,
            EyeRayMode::Thru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        self.test_eye_static_floors(
            origin,
            target,
            EyeRayMode::Thru,
            &mut best_dist_sq,
            &mut best_hit,
        );
        if self.disable == 0 {
            Self::test_eye_planes(
                origin,
                target,
                &self.raw_ceilings,
                EyeRayMode::Thru,
                &mut best_dist_sq,
                &mut best_hit,
            );
        }
        best_hit
    }

    /// Clamp camera position against room walls, floors, ceilings, and BG columns (`clCheckHitEyes` `0x00138ea0`).
    ///
    /// Uses full 3D `check_hit_eyes(..., EyeRayMode::Thru, ...)` when raw `.cld` geometry is loaded,
    /// returning the exact contact point (with optional `filter.safety_margin`, defaulting to `0.0`).
    pub fn clamp_camera(&self, target: Vec3, camera_pos: Vec3, filter: &RaycastFilter) -> Vec3 {
        if !self.raw_walls.is_empty()
            || !self.raw_floors.is_empty()
            || !self.raw_ceilings.is_empty()
            || !self.columns.is_empty()
        {
            if let Some(hit) = self.check_hit_eyes(target, camera_pos, EyeRayMode::Thru, None, &[])
            {
                if filter.safety_margin > 0.0 {
                    let clamped_t = (hit.t - filter.safety_margin).max(0.0);
                    return target + (camera_pos - target) * clamped_t;
                }
                return hit.point;
            }
            return camera_pos;
        }
        clamp_camera_ray(target, camera_pos, self.all_walls(), filter)
    }

    /// Iterate over all active walkable/structural floor triangles (static + active dynamic floors).
    pub fn all_floors(&self) -> impl Iterator<Item = &FloorTriangle> {
        self.floors.iter().chain(
            self.dynamic_floors
                .iter()
                .filter(|df| df.active)
                .map(|df| &df.triangle),
        )
    }

    /// Casts a zero-allocation 3D acoustic line-of-sight ray from `emitter_pos` to
    /// `listener_head_pos` against active `.cld` walls, doors, and multi-story floor slabs (`SH2-84`).
    pub fn raycast_acoustic_occlusion(
        &self,
        emitter_pos: Vec3,
        listener_head_pos: Vec3,
    ) -> AcousticOcclusionHit {
        raycast_acoustic_occlusion(
            emitter_pos,
            listener_head_pos,
            self.all_walls(),
            self.all_floors(),
        )
    }

    /// Find safe walkable spawn point closest to `(x, z)` snapped to nearest floor centroid (`col_spawn`).
    pub fn find_safe_spawn_point(&self, x: f32, z: f32) -> Option<Vec3> {
        let mut best_dist_sq = f32::INFINITY;
        let mut best_spawn: Option<Vec3> = None;

        for f in &self.floors {
            let cx = (f.v[0][0] + f.v[1][0] + f.v[2][0]) / 3.0;
            let cy = (f.v[0][1] + f.v[1][1] + f.v[2][1]) / 3.0;
            let cz = (f.v[0][2] + f.v[1][2] + f.v[2][2]) / 3.0;

            let dx = cx - x;
            let dz = cz - z;
            let dist_sq = dx * dx + dz * dz;

            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_spawn = Some(Vec3::new(cx, cy, cz));
            }
        }

        best_spawn
    }
}

/// Unified CollisionController subsystem managing frame lifecycle and multi-actor separation (`cl_main.c`).
#[derive(Debug, Clone, Default)]
pub struct CollisionController {
    /// Active collision environment.
    pub world: CollisionWorld,
    /// Registered character collision cylinders (James, Maria, enemies).
    pub characters: Vec<CharacterCylinder>,
    /// Global collision config.
    pub config: CollisionConfig,
}

impl CollisionController {
    /// Initialize collision controller (`clAllInitCollisionData`).
    pub fn new(world: CollisionWorld, config: CollisionConfig) -> Self {
        Self {
            world,
            characters: Vec::new(),
            config,
        }
    }

    /// Per-frame lifecycle initialization (`clFrameInitCollisionData`).
    pub fn frame_init(&mut self) {
        self.world.extra_walls.clear();
    }

    /// Register or update a character cylinder (`clSetCharaHitColumn`).
    pub fn register_character(&mut self, chara: CharacterCylinder) {
        if let Some(existing) = self.characters.iter_mut().find(|c| c.id == chara.id) {
            *existing = chara;
        } else {
            self.characters.push(chara);
        }
    }

    /// Remove a character cylinder.
    pub fn unregister_character(&mut self, id: CharacterId) {
        self.characters.retain(|c| c.id != id);
    }

    /// Casts a zero-allocation 3D acoustic line-of-sight ray from `emitter_pos` to
    /// `listener_head_pos` against the controller's active `.cld` collision world (`SH2-84`).
    pub fn raycast_acoustic_occlusion(
        &self,
        emitter_pos: Vec3,
        listener_head_pos: Vec3,
    ) -> AcousticOcclusionHit {
        self.world
            .raycast_acoustic_occlusion(emitter_pos, listener_head_pos)
    }

    /// Exact 1:1 `clCollectCharaALL` (`SLUS_202.28` `0x00132e20` / `sh2pc.exe` `0x00526280`):
    /// runs up to 5 interleaved BG + character-to-character relaxation iterations using the
    /// discrete weight-diff table `[1.0, 0.98, 0.94, 0.90, 0.86]`.
    ///
    /// Returns the number of iterations executed (`1..=5`).
    pub fn resolve_character_separations(&mut self) -> usize {
        if self.characters.is_empty() {
            return 0;
        }

        // Exact PS2 `clCharaList[32]` fixed-capacity stack buffer (strictly zero-allocation)
        const MAX_CL_CHARA: usize = 32;
        let entry_count = self.characters.len().min(MAX_CL_CHARA);
        let mut entries = [CharaCollectEntry::EMPTY; MAX_CL_CHARA];
        for (slot, c) in entries[..entry_count]
            .iter_mut()
            .zip(self.characters.iter())
        {
            *slot = CharaCollectEntry {
                col: c.move_column(),
                move_vec: Vec3::ZERO,
                push_vec: Vec3::ZERO,
                prev_pos: c.position,
                chara_type: 1,
            };
        }

        let world = &self.world;
        let iterations = cl_collect_chara_all(&mut entries[..entry_count], |entry| {
            let move_angle = if entry.move_vec.x == 0.0 && entry.move_vec.z == 0.0 {
                CL_NO_MOVE_ANGLE
            } else {
                sh_atan2(entry.move_vec.x, entry.move_vec.z)
            };
            let mut ctx = WallCollectContext {
                move_angle,
                cur_pos: Vec3::new(entry.col.center_x, entry.prev_pos.y, entry.col.center_z),
                prev_pos: entry.prev_pos,
                chara_type: entry.chara_type,
                post_flag_timer: 0,
                fps: 60.0,
            };
            world.check_bg_to_chara(&entry.col, &mut ctx)
        });

        for (chara, entry) in self
            .characters
            .iter_mut()
            .zip(entries[..entry_count].iter())
        {
            chara.position.x = entry.col.center_x;
            chara.position.z = entry.col.center_z;
        }

        iterations
    }

    /// Update a character's position through full environment collision and 5-iteration separation.
    pub fn update_character_kinematics(&mut self, id: CharacterId, desired_pos: Vec3) -> Vec3 {
        let (current_pos, config) = if let Some(c) = self.characters.iter().find(|c| c.id == id) {
            let mut cfg = self.config;
            cfg.radius = c.radius;
            cfg.height = c.height;
            cfg.y_bottom_offset = c.y_bottom_offset;
            (c.position, cfg)
        } else {
            (desired_pos, self.config)
        };

        // 1. Resolve background world geometry collision
        let resolved_pos = self
            .world
            .resolve_character_movement(current_pos, desired_pos, &config);

        if let Some(c) = self.characters.iter_mut().find(|c| c.id == id) {
            c.position = resolved_pos;
        }

        // 2. Resolve 5-iteration interleaved BG + actor-to-actor separations (`clCollectCharaALL`)
        self.resolve_character_separations();

        self.characters
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.position)
            .unwrap_or(resolved_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wall_pushback_and_sliding() {
        // Enclosed square room 2000x2000 (X: -1000..1000, Z: -1000..1000)
        let world = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        let config = CollisionConfig::default(); // radius = 175.0 (`PlayerSetColumn_SetTarget` 0x00107d40)

        // Move player towards east wall (X = 1000.0) beyond the boundary
        let current_pos = Vec3::new(800.0, 0.0, 0.0);
        let desired_pos = Vec3::new(980.0, 0.0, 100.0); // should be pushed back to X <= 1000 - 175 = 825

        let final_pos = world.resolve_character_movement(current_pos, desired_pos, &config);

        assert!(
            final_pos.x <= 1000.0 - config.radius + 0.1,
            "Player X ({}) must be clamped by wall radius boundary {}",
            final_pos.x,
            1000.0 - config.radius
        );
        // Z motion should slide freely
        assert_eq!(final_pos.z, 100.0);
        assert_eq!(final_pos.y, 0.0);
    }

    #[test]
    fn test_center_pillar_collision() {
        // 4000x4000 room with center pillar 600x600 (-300..300, -300..300)
        let world = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 3000.0, 600.0, 600.0);
        let config = CollisionConfig::default(); // radius = 175.0

        // Try walking into the pillar from the west
        let current_pos = Vec3::new(-500.0, 0.0, 0.0);
        let desired_pos = Vec3::new(-200.0, 0.0, 0.0); // Inside the pillar's west wall (-300)

        let final_pos = world.resolve_character_movement(current_pos, desired_pos, &config);

        // West wall of pillar is at X = -300, so cylinder center should be pushed to <= -300 - 175 = -475
        assert!(
            final_pos.x <= -300.0 - config.radius + 0.1,
            "Player should be pushed outside pillar boundary: pos.x={}",
            final_pos.x
        );
    }

    #[test]
    fn test_stair_stepping_and_gravity() {
        let mut world = CollisionWorld::new();
        world.floors.push(FloorTriangle::new(
            [[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [1000.0, 0.0, 1000.0]],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [[0.0, 0.0, 0.0], [1000.0, 0.0, 1000.0], [0.0, 0.0, 1000.0]],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [
                [1000.0, -150.0, 0.0],
                [2000.0, -150.0, 0.0],
                [2000.0, -150.0, 1000.0],
            ],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [
                [1000.0, -150.0, 0.0],
                [2000.0, -150.0, 1000.0],
                [1000.0, -150.0, 1000.0],
            ],
            0,
        ));

        let config = CollisionConfig::default();

        let current_pos = Vec3::new(900.0, 0.0, 500.0);
        let desired_pos = Vec3::new(1100.0, 0.0, 500.0);

        let final_pos = world.resolve_character_movement(current_pos, desired_pos, &config);
        assert_eq!(
            final_pos.y, -150.0,
            "Player must step up onto platform at -150.0"
        );
    }

    #[test]
    fn test_camera_wall_clamping_cl_check_hit_eyes() {
        let world = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        let filter = RaycastFilter::default();

        let player_eye = Vec3::new(0.0, -1200.0, 0.0);
        let desired_camera = Vec3::new(0.0, -1200.0, -1500.0);

        let clamped = world.clamp_camera(player_eye, desired_camera, &filter);

        // Exact 1:1 `clCheckHitEyes` (`0x00138ea0`): stops directly at the wall plane `z = -1000.0`
        // with zero guessed pullback margin (`safety_margin = 0.0`).
        assert!(
            (clamped.z - (-1000.0)).abs() < 1e-3,
            "Camera ray must stop at wall plane z = -1000.0: clamped.z={}",
            clamped.z
        );
    }

    #[test]
    fn test_character_to_character_separation_cl_check_column2_column_hit() {
        let world = CollisionWorld::new();
        let config = CollisionConfig::default();
        let mut controller = CollisionController::new(world, config);

        // James at (0, 0, 0) with radius 175
        let james = CharacterCylinder::james(CharacterId(1), Vec3::new(0.0, 0.0, 0.0));
        // Nurse at (200, 0, 0) with radius 150. Combined radius = 325 > 200 (overlapping!)
        let nurse = CharacterCylinder::new(
            CharacterId(2),
            Vec3::new(200.0, 0.0, 0.0),
            150.0,
            800.0,
            1.0,
        );

        controller.register_character(james);
        controller.register_character(nurse);

        controller.resolve_character_separations();

        let new_james = controller
            .characters
            .iter()
            .find(|c| c.id == CharacterId(1))
            .unwrap();
        let new_nurse = controller
            .characters
            .iter()
            .find(|c| c.id == CharacterId(2))
            .unwrap();

        let dist = (new_nurse.position.x - new_james.position.x).abs();
        assert!(
            dist >= 324.9,
            "Cylinders must be pushed apart to at least combined radius 325.0; got {}",
            dist
        );
    }

    #[test]
    fn test_col_spawn_centroid_snapping() {
        let world = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 3000.0, 0.0, 0.0);
        let spawn = world
            .find_safe_spawn_point(500.0, 500.0)
            .expect("Must find floor centroid");

        assert_eq!(spawn.y, 0.0);
        assert!(spawn.x.abs() <= 2000.0);
        assert!(spawn.z.abs() <= 2000.0);
    }

    #[test]
    fn test_dynamic_wall_pushback() {
        let mut world = CollisionWorld::new();
        let config = CollisionConfig::default();

        // Add a closed dynamic door wall at X = 500 across Z: -500..500 (normal points -X)
        world.add_dynamic_wall(DynamicWall::new(
            500.0, -500.0, 500.0, 500.0, -3000.0, 100.0, 0,
        ));

        let current_pos = Vec3::new(300.0, 0.0, 0.0);
        let desired_pos = Vec3::new(400.0, 0.0, 0.0);

        let resolved = world.resolve_character_movement(current_pos, desired_pos, &config);
        assert!(
            resolved.x <= 500.0 - config.radius + 0.1,
            "Dynamic wall must push character back: pos.x={}",
            resolved.x
        );
    }

    #[test]
    fn test_col_merge_rooms() {
        let room_a = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
        let room_b = CollisionWorld::create_synthetic_room(4000.0, 4000.0, 3000.0, 0.0, 0.0);

        let merged = CollisionWorld::merge(&[&room_a, &room_b]);
        assert_eq!(
            merged.floors.len(),
            room_a.floors.len() + room_b.floors.len()
        );
        assert_eq!(merged.walls.len(), room_a.walls.len() + room_b.walls.len());
    }
}
