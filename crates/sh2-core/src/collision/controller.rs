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
use super::probes::{PLAYER_HEIGHT_RAY_DOWN_NORMAL, PLAYER_HEIGHT_RAY_UP_NORMAL};
use super::raycast::{
    intersect_segment_xz_t, AcousticOcclusionAccumulator, AcousticOcclusionHit, EyeRayHit,
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
    /// Maximum vertical climb/step-up threshold (`250.0` mm, `PlayerSetHeight` `0x00107aa0`).
    pub max_step_up: f32,
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
            max_step_up: PLAYER_HEIGHT_RAY_UP_NORMAL,
            max_iterations: CL_COLLECT_CHARA_ITERATIONS,
        }
    }
}

/// Individual `.cld` block preserved inside a merged [`CollisionWorld`] so
/// `clGetHitSectListMOVEOutDoor` (`0x00138bc0`) and `clGetHitSectListMOVEInDoor` (`0x00138df0`)
/// evaluate each block's own `header.disable` (`+0x1c`), `(sx, sz)` origin, and 4x4 `SpatialGrid`.
#[derive(Debug, Clone)]
pub struct CollisionSector {
    /// Block origin coordinates `(sx, sz)` in world mm.
    pub origin: [f32; 2],
    /// Header `disable` flag (`+0x1c`). When non-zero, this `.cld` block is skipped.
    pub disable: i32,
    /// Preserved 4x4 spatial acceleration broadphase grid for this `.cld` block.
    pub grid: SpatialGrid,
    /// Start offset into `CollisionWorld::floors`.
    pub floor_offset: usize,
    /// Number of triangles belonging to this sector in `CollisionWorld::floors`.
    pub floor_len: usize,
    /// Start offset into `CollisionWorld::raw_floors`.
    pub raw_floor_offset: usize,
    /// Number of raw floor planes belonging to this sector in `CollisionWorld::raw_floors`.
    pub raw_floor_len: usize,
    /// Start offset into `CollisionWorld::raw_walls`.
    pub raw_wall_offset: usize,
    /// Number of wall planes belonging to this sector in `CollisionWorld::raw_walls`.
    pub raw_wall_len: usize,
    /// Start offset into `CollisionWorld::raw_ceilings`.
    pub raw_ceiling_offset: usize,
    /// Number of ceiling planes belonging to this sector in `CollisionWorld::raw_ceilings`.
    pub raw_ceiling_len: usize,
    /// Start offset into `CollisionWorld::raw_side_walls`.
    pub raw_side_wall_offset: usize,
    /// Number of side-wall planes belonging to this sector in `CollisionWorld::raw_side_walls`.
    pub raw_side_wall_len: usize,
    /// Start offset into `CollisionWorld::columns`.
    pub column_offset: usize,
    /// Number of static BG columns belonging to this sector in `CollisionWorld::columns`.
    pub column_len: usize,
    /// Start offset into `CollisionWorld::walls`.
    pub wall_offset: usize,
    /// Number of derived 2D wall segments belonging to this sector in `CollisionWorld::walls`.
    pub wall_len: usize,
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
    /// Per-`.cld` blocks preserved by [`CollisionWorld::merge`] (`loadBgCLD_GetLoadedDataAddrList`
    /// in `clGetHitSectListMOVEOutDoor` `0x00138bc0`). Empty for single-`.cld` worlds.
    pub sectors: Vec<CollisionSector>,
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
            sectors: Vec::new(),
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

    /// Merge multiple collision worlds into a single combined environment (`col_merge`),
    /// preserving each `.cld` block's independent `origin`, `disable` (`+0x1c`), and 4x4
    /// `SpatialGrid` in [`CollisionWorld::sectors`] (`clGetHitSectListMOVEOutDoor` `0x00138bc0`).
    pub fn merge(parts: &[&CollisionWorld]) -> Self {
        let mut merged = Self::new();
        for p in parts {
            let floor_base = merged.floors.len();
            let raw_floor_base = merged.raw_floors.len();
            let raw_wall_base = merged.raw_walls.len();
            let raw_ceiling_base = merged.raw_ceilings.len();
            let raw_side_wall_base = merged.raw_side_walls.len();
            let column_base = merged.columns.len();
            let wall_base = merged.walls.len();

            if p.sectors.is_empty() {
                merged.sectors.push(CollisionSector {
                    origin: p.origin,
                    disable: p.disable,
                    grid: p.grid.clone(),
                    floor_offset: floor_base,
                    floor_len: p.floors.len(),
                    raw_floor_offset: raw_floor_base,
                    raw_floor_len: p.raw_floors.len(),
                    raw_wall_offset: raw_wall_base,
                    raw_wall_len: p.raw_walls.len(),
                    raw_ceiling_offset: raw_ceiling_base,
                    raw_ceiling_len: p.raw_ceilings.len(),
                    raw_side_wall_offset: raw_side_wall_base,
                    raw_side_wall_len: p.raw_side_walls.len(),
                    column_offset: column_base,
                    column_len: p.columns.len(),
                    wall_offset: wall_base,
                    wall_len: p.walls.len(),
                });
            } else {
                for s in &p.sectors {
                    merged.sectors.push(CollisionSector {
                        origin: s.origin,
                        disable: s.disable,
                        grid: s.grid.clone(),
                        floor_offset: floor_base + s.floor_offset,
                        floor_len: s.floor_len,
                        raw_floor_offset: raw_floor_base + s.raw_floor_offset,
                        raw_floor_len: s.raw_floor_len,
                        raw_wall_offset: raw_wall_base + s.raw_wall_offset,
                        raw_wall_len: s.raw_wall_len,
                        raw_ceiling_offset: raw_ceiling_base + s.raw_ceiling_offset,
                        raw_ceiling_len: s.raw_ceiling_len,
                        raw_side_wall_offset: raw_side_wall_base + s.raw_side_wall_offset,
                        raw_side_wall_len: s.raw_side_wall_len,
                        column_offset: column_base + s.column_offset,
                        column_len: s.column_len,
                        wall_offset: wall_base + s.wall_offset,
                        wall_len: s.wall_len,
                    });
                }
            }

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

        if let Some(first) = merged.sectors.first() {
            merged.origin = first.origin;
            merged.grid = first.grid.clone();
        }
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

        // 1. Broadphase: query floor triangles per `.cld` sector (`clGetHitSectListMOVEOutDoor` `0x00138bc0`)
        if !self.sectors.is_empty() {
            for sector in &self.sectors {
                if sector.disable != 0 {
                    continue;
                }
                let sector_floors =
                    &self.floors[sector.floor_offset..(sector.floor_offset + sector.floor_len)];
                let (active_cells, cell_count) = sector.grid.query_move_cells_array(x, y_ref, z);
                let mut queried_any = false;
                for &cell in &active_cells[..cell_count] {
                    let cell_tri_indices = &sector.grid.floor_cells[cell];
                    if !cell_tri_indices.is_empty() {
                        queried_any = true;
                        for &idx in cell_tri_indices {
                            if let Some(tri) = sector_floors.get(idx) {
                                if let Some(y) = tri.sample_height(x, z) {
                                    if y >= y_ref - up
                                        && y <= y_ref + down
                                        && best.is_none_or(|prev| y < prev.height_y)
                                    {
                                        best = Some(FloorSample {
                                            height_y: y,
                                            raw_material: tri.material,
                                            surface: SurfaceMaterial::from_cld_material(
                                                tri.material,
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                if !queried_any && !sector.grid.is_outdoor {
                    for tri in sector_floors {
                        if let Some(y) = tri.sample_height(x, z) {
                            if y >= y_ref - up
                                && y <= y_ref + down
                                && best.is_none_or(|prev| y < prev.height_y)
                            {
                                best = Some(FloorSample {
                                    height_y: y,
                                    raw_material: tri.material,
                                    surface: SurfaceMaterial::from_cld_material(tri.material),
                                });
                            }
                        }
                    }
                }
            }
        } else if self.disable == 0 {
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

        // 2. SpatialGrid broadphase (`clGetHitSectListMOVE` 0x00138b60 / `OutDoor` 0x00138bc0)
        if !self.sectors.is_empty() {
            for sector in &self.sectors {
                if sector.disable != 0 || hit_count >= CL_MAX_WALL_HITS {
                    continue;
                }
                let (active_cells, cell_count) =
                    sector
                        .grid
                        .query_move_cells_array(col.center_x, col.y_bottom, col.center_z);
                let active_slice = &active_cells[..cell_count];
                let sector_walls = &self.raw_walls
                    [sector.raw_wall_offset..(sector.raw_wall_offset + sector.raw_wall_len)];
                let sector_side_walls = &self.raw_side_walls[sector.raw_side_wall_offset
                    ..(sector.raw_side_wall_offset + sector.raw_side_wall_len)];
                let sector_cols =
                    &self.columns[sector.column_offset..(sector.column_offset + sector.column_len)];

                for &cell in active_slice {
                    for &idx in &sector.grid.wall_cells[cell] {
                        if hit_count >= CL_MAX_WALL_HITS {
                            break;
                        }
                        if let Some(plane) = sector_walls.get(idx) {
                            if let Some(hit) = cl_check_column_to_wall_hit(plane, col) {
                                hits[hit_count] = hit;
                                hit_count += 1;
                            }
                        }
                    }
                }
                for &cell in active_slice {
                    for &idx in &sector.grid.side_wall_cells[cell] {
                        if hit_count >= CL_MAX_WALL_HITS {
                            break;
                        }
                        if let Some(plane) = sector_side_walls.get(idx) {
                            if let Some(hit) = cl_check_column_to_wall_hit(plane, col) {
                                hits[hit_count] = hit;
                                hit_count += 1;
                            }
                        }
                    }
                }
                for &cell in active_slice {
                    for &idx in &sector.grid.column_cells[cell] {
                        if hit_count >= CL_MAX_WALL_HITS {
                            break;
                        }
                        if let Some(bg_col) = sector_cols.get(idx) {
                            if let Some(hit) = cl_check_column_to_bg_column_hit(bg_col, col) {
                                hits[hit_count] = hit;
                                hit_count += 1;
                            }
                        }
                    }
                }
            }
        } else if self.disable == 0 {
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
            let mut test_wall = |w: &WallSegment| {
                if w.group == 2 || w.y_top >= y_bottom || w.y_bottom <= y_top {
                    return;
                }
                if let Some(t) = intersect_segment_xz_t(
                    current_pos.x,
                    current_pos.z,
                    desired_pos.x,
                    desired_pos.z,
                    w,
                ) {
                    if earliest_hit.is_none_or(|(prev_t, _, _)| t < prev_t) {
                        earliest_hit =
                            Some((t, current_pos.x + move_dx * t, current_pos.z + move_dz * t));
                    }
                }
            };
            self.for_each_ray_wall_segment(
                current_pos.x,
                current_pos.z,
                desired_pos.x,
                desired_pos.z,
                &mut test_wall,
            );
            for w in &self.extra_walls {
                test_wall(w);
            }
            for dw in &self.dynamic_walls {
                if dw.active {
                    test_wall(&dw.segment);
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

        // 3. Sample floor height (`PlayerSetHeight` `0x00107aa0`: `[-250.0, +1500.0]` mm probe)
        if let Some(floor_y) = self.sample_floor_height(
            pos.x,
            pos.z,
            current_pos.y,
            config.max_step_up,
            PLAYER_HEIGHT_RAY_DOWN_NORMAL,
        ) {
            pos.y = floor_y;
        }

        pos
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
    /// `listener_head_pos` against active `.cld` walls, doors, and multi-story floor slabs (`SH2-84`),
    /// accelerated by per-sector [`SpatialGrid`] traversal (`clGetHitSectListVECHIT` `0x00138170`).
    pub fn raycast_acoustic_occlusion(
        &self,
        emitter_pos: Vec3,
        listener_head_pos: Vec3,
    ) -> AcousticOcclusionHit {
        let Some(mut acc) = AcousticOcclusionAccumulator::new(emitter_pos, listener_head_pos)
        else {
            return AcousticOcclusionHit::clear();
        };

        // 1. Static `.cld` walls via per-sector `SpatialGrid` + extra/dynamic walls
        self.for_each_ray_wall_segment(
            emitter_pos.x,
            emitter_pos.z,
            listener_head_pos.x,
            listener_head_pos.z,
            |w| acc.test_wall(w),
        );
        for w in &self.extra_walls {
            acc.test_wall(w);
        }
        for dw in &self.dynamic_walls {
            if dw.active {
                acc.test_wall(&dw.segment);
            }
        }

        // 2. Multi-story `.cld` floor slabs via per-sector `SpatialGrid` (using padded ray endpoints)
        if acc.checks_slabs() {
            let (p0, p1) = acc.padded_endpoints();
            self.for_each_ray_floor_triangle(p0.x, p0.z, p1.x, p1.z, |tri| acc.test_floor(tri));
            for df in &self.dynamic_floors {
                if df.active {
                    acc.test_floor(&df.triangle);
                }
            }
        }

        acc.finish()
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

    /// Per-frame lifecycle initialization (`clFrameInitCollisionData` `0x00132120`, `cl_main.c:96-107`):
    /// resets all per-frame dynamic wall, dynamic floor, extra wall, and registered character lists.
    pub fn frame_init(&mut self) {
        self.world.extra_walls.clear();
        self.world.dynamic_walls.clear();
        self.world.dynamic_floors.clear();
        self.characters.clear();
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

    /// Exact 1:1 `clCheckHitEyesOnlyFloor` (`SLUS_202.28` `0x00138f10`):
    /// casts a segment against dynamic floors and static `.cld` floors, skipping `material == 12`.
    pub fn check_hit_eyes_only_floor(&self, origin: Vec3, target: Vec3) -> Option<EyeRayHit> {
        self.world.check_hit_eyes_only_floor(origin, target)
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

        // Exact PS2 `clCharaList[32]` fixed-capacity stack buffer (strictly zero-allocation).
        // Per `clSetCharaHitColumn` (`0x001325e0`, `cl_main.c:226-260`), each entry starts at
        // `prev_position` with `move_vec = position - prev_position`, per-actor `chara_type`, and
        // `post_flag_timer` so `clCollectCharaALL` (`0x00132e20`) integrates `move_vec` inside
        // the 5-iteration interleaved BG + actor separation loop.
        const MAX_CL_CHARA: usize = 32;
        let entry_count = self.characters.len().min(MAX_CL_CHARA);
        let mut entries = [CharaCollectEntry::EMPTY; MAX_CL_CHARA];
        for (slot, c) in entries[..entry_count]
            .iter_mut()
            .zip(self.characters.iter())
        {
            let move_vec = Vec3::new(
                c.position.x - c.prev_position.x,
                0.0,
                c.position.z - c.prev_position.z,
            );
            let mut start_col = c.move_column();
            start_col.center_x = c.prev_position.x;
            start_col.center_z = c.prev_position.z;
            *slot = CharaCollectEntry {
                col: start_col,
                move_vec,
                push_vec: Vec3::ZERO,
                prev_pos: c.prev_position,
                chara_type: c.chara_type,
                post_flag_timer: c.post_flag_timer,
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
                post_flag_timer: entry.post_flag_timer,
                fps: 60.0,
            };
            let push = world.check_bg_to_chara(&entry.col, &mut ctx);
            entry.post_flag_timer = ctx.post_flag_timer;
            push
        });

        for (chara, entry) in self
            .characters
            .iter_mut()
            .zip(entries[..entry_count].iter())
        {
            chara.position.x = entry.col.center_x;
            chara.position.z = entry.col.center_z;
            chara.prev_position = chara.position;
            chara.post_flag_timer = entry.post_flag_timer;
        }

        iterations
    }

    /// Update a character's position through unified `clCollectCharaALL` (`0x00132e20`)
    /// 5-iteration interleaved `move_vec` + BG + actor separation, followed by floor height sampling.
    pub fn update_character_kinematics(&mut self, id: CharacterId, desired_pos: Vec3) -> Vec3 {
        if let Some(c) = self.characters.iter_mut().find(|c| c.id == id) {
            let current_pos = c.position;
            c.prev_position = current_pos;
            c.position = desired_pos;

            // Run unified 5-iteration `clCollectCharaALL` integrating `move_vec = desired_pos - current_pos`
            // together with BG wall/column pushback and character-to-character separation.
            self.resolve_character_separations();

            if let Some(c) = self.characters.iter_mut().find(|c| c.id == id) {
                if let Some(floor_y) = self.world.sample_floor_height(
                    c.position.x,
                    c.position.z,
                    current_pos.y,
                    self.config.max_step_up,
                    PLAYER_HEIGHT_RAY_DOWN_NORMAL,
                ) {
                    c.position.y = floor_y;
                }
                c.prev_position = c.position;
                return c.position;
            }
        }

        self.world
            .resolve_character_movement(desired_pos, desired_pos, &self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::RaycastFilter;

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
