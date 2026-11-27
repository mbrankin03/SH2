//! Full 3D `clCheckHitEyes` (`SLUS_202.28` `0x00138ea0`) and specialized floor/wall/ceiling
//! raycast methods on [`CollisionWorld`].
//!
//! Extracted from `controller.rs` to keep `CollisionWorld` modular (`AGENTS.md` Rule 2).

use glam::Vec3;
use sh2_assets::formats::cld::{FloorTriangle, HitPolyColumn, HitPolyPlane, WallSegment};

use super::character::{CharacterCylinder, CharacterId};
use super::controller::CollisionWorld;
use super::grid::SpatialGrid;
use super::raycast::{
    cl_check_sub_line_to_column_plus, cl_check_sub_line_to_plane, intersect_ray_triangle_3d,
    intersect_segment_bg_column, intersect_segment_xz_t, is_cld_material_thru,
    should_skip_material_for_mode, EyeHitKind, EyeRayHit, EyeRayMode, RaycastFilter,
};

/// Borrowed view of an active collision sector (or the unsegmented world fallback) along with
/// its [`SpatialGrid`] and ray-traversed cell list.
struct ActiveRaySector<'a> {
    grid: &'a SpatialGrid,
    active_cells: Option<&'a [usize]>,
    walls: &'a [WallSegment],
    floors: &'a [FloorTriangle],
    raw_walls: &'a [HitPolyPlane],
    raw_side_walls: &'a [HitPolyPlane],
    raw_floors: &'a [HitPolyPlane],
    raw_ceilings: &'a [HitPolyPlane],
    columns: &'a [HitPolyColumn],
}

impl CollisionWorld {
    /// Internal helper: queries `grid` along XZ segment `(x0, z0) -> (x1, z1)` when indexed
    /// geometry is present, invoking `f(Some(cells))` only if at least one cell is intersected,
    /// or `f(None)` for unindexed fallback worlds.
    #[inline]
    fn visit_ray_grid<F>(grid: &SpatialGrid, x0: f32, z0: f32, x1: f32, z1: f32, f: F)
    where
        F: FnOnce(Option<&[usize]>),
    {
        if grid.has_indexed_geometry {
            let (cells, count) = grid.query_ray_cells_array(x0, z0, x1, z1);
            if count > 0 {
                f(Some(&cells[..count]));
            }
        } else {
            f(None);
        }
    }

    /// Internal helper: visits each active collision sector (or the unsegmented world fallback
    /// when `self.sectors` is empty) intersecting XZ segment `(x0, z0) -> (x1, z1)`.
    ///
    /// Respects per-sector `sector.disable` (or `self.disable`) and per-sector [`SpatialGrid`]
    /// cell traversal (`clGetHitSectListVECHIT` `0x00138170`).
    #[inline]
    fn for_each_active_ray_sector<F>(&self, x0: f32, z0: f32, x1: f32, z1: f32, mut f: F)
    where
        F: FnMut(ActiveRaySector<'_>),
    {
        if !self.sectors.is_empty() {
            for sector in &self.sectors {
                if sector.disable != 0 {
                    continue;
                }
                Self::visit_ray_grid(&sector.grid, x0, z0, x1, z1, |active_cells| {
                    f(ActiveRaySector {
                        grid: &sector.grid,
                        active_cells,
                        walls: &self.walls
                            [sector.wall_offset..(sector.wall_offset + sector.wall_len)],
                        floors: &self.floors
                            [sector.floor_offset..(sector.floor_offset + sector.floor_len)],
                        raw_walls: &self.raw_walls[sector.raw_wall_offset
                            ..(sector.raw_wall_offset + sector.raw_wall_len)],
                        raw_side_walls: &self.raw_side_walls[sector.raw_side_wall_offset
                            ..(sector.raw_side_wall_offset + sector.raw_side_wall_len)],
                        raw_floors: &self.raw_floors[sector.raw_floor_offset
                            ..(sector.raw_floor_offset + sector.raw_floor_len)],
                        raw_ceilings: &self.raw_ceilings[sector.raw_ceiling_offset
                            ..(sector.raw_ceiling_offset + sector.raw_ceiling_len)],
                        columns: &self.columns
                            [sector.column_offset..(sector.column_offset + sector.column_len)],
                    });
                });
            }
        } else if self.disable == 0 {
            Self::visit_ray_grid(&self.grid, x0, z0, x1, z1, |active_cells| {
                f(ActiveRaySector {
                    grid: &self.grid,
                    active_cells,
                    walls: &self.walls,
                    floors: &self.floors,
                    raw_walls: &self.raw_walls,
                    raw_side_walls: &self.raw_side_walls,
                    raw_floors: &self.raw_floors,
                    raw_ceilings: &self.raw_ceilings,
                    columns: &self.columns,
                });
            });
        }
    }

    /// Internal helper: visits static `.cld` [`WallSegment`]s along XZ segment `(x0, z0) -> (x1, z1)`
    /// using per-sector [`SpatialGrid`] cell traversal (`clGetHitSectListVECHIT` `0x00138170`),
    /// respecting per-sector `sector.disable` flags and falling back to full slices for manually
    /// populated worlds without a `.cld` grid.
    pub(crate) fn for_each_ray_wall_segment<F>(&self, x0: f32, z0: f32, x1: f32, z1: f32, mut f: F)
    where
        F: FnMut(&WallSegment),
    {
        self.for_each_active_ray_sector(x0, z0, x1, z1, |sector| {
            if let Some(cells) = sector.active_cells {
                for &cell in cells {
                    for &idx in &sector.grid.wall_segment_cells[cell] {
                        if let Some(w) = sector.walls.get(idx) {
                            f(w);
                        }
                    }
                }
            } else {
                for w in sector.walls {
                    f(w);
                }
            }
        });
    }

    /// Internal helper: visits static `.cld` [`FloorTriangle`]s along XZ segment `(x0, z0) -> (x1, z1)`
    /// using per-sector [`SpatialGrid`] cell traversal (`clGetHitSectListVECHIT` `0x00138170`).
    pub(crate) fn for_each_ray_floor_triangle<F>(
        &self,
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        mut f: F,
    ) where
        F: FnMut(&FloorTriangle),
    {
        self.for_each_active_ray_sector(x0, z0, x1, z1, |sector| {
            if let Some(cells) = sector.active_cells {
                for &cell in cells {
                    for &idx in &sector.grid.floor_cells[cell] {
                        if let Some(tri) = sector.floors.get(idx) {
                            f(tri);
                        }
                    }
                }
            } else {
                for tri in sector.floors {
                    f(tri);
                }
            }
        });
    }

    /// Internal helper: test a single `_CL_HITPOLY_PLANE` against segment `origin -> target`.
    #[inline]
    fn test_eye_plane(
        origin: Vec3,
        target: Vec3,
        plane: &HitPolyPlane,
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        if should_skip_material_for_mode(plane.material, mode) {
            return;
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
            Self::test_eye_plane(origin, target, plane, mode, best_dist_sq, best_hit);
        }
    }

    /// Internal helper: test indexed `_CL_HITPOLY_PLANE`s across active `cells`.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn test_eye_planes_cells(
        origin: Vec3,
        target: Vec3,
        planes: &[HitPolyPlane],
        cell_lists: &[Vec<usize>; 16],
        active_cells: &[usize],
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        for &cell in active_cells {
            for &idx in &cell_lists[cell] {
                if let Some(plane) = planes.get(idx) {
                    Self::test_eye_plane(origin, target, plane, mode, best_dist_sq, best_hit);
                }
            }
        }
    }

    /// Internal helper: test a single `FloorTriangle` against segment `origin -> target`.
    #[inline]
    fn test_eye_floor_tri(
        origin: Vec3,
        target: Vec3,
        tri: &FloorTriangle,
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        if should_skip_material_for_mode(tri.material, mode) {
            return;
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

    /// Internal helper: test a single static BG `HitPolyColumn` against segment `origin -> target`.
    #[inline]
    fn test_eye_bg_column(
        origin: Vec3,
        target: Vec3,
        col: &HitPolyColumn,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
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

    /// Internal helper: test a sector's static floors (preferring `raw_floors`, falling back to `floors`).
    #[allow(clippy::too_many_arguments)]
    fn test_sector_static_floors(
        origin: Vec3,
        target: Vec3,
        grid: &SpatialGrid,
        active_cells: Option<&[usize]>,
        raw_floors: &[HitPolyPlane],
        floors: &[FloorTriangle],
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        if !raw_floors.is_empty() {
            if let Some(cells) = active_cells {
                Self::test_eye_planes_cells(
                    origin,
                    target,
                    raw_floors,
                    &grid.raw_floor_cells,
                    cells,
                    mode,
                    best_dist_sq,
                    best_hit,
                );
            } else {
                Self::test_eye_planes(origin, target, raw_floors, mode, best_dist_sq, best_hit);
            }
        } else if let Some(cells) = active_cells {
            for &cell in cells {
                for &idx in &grid.floor_cells[cell] {
                    if let Some(tri) = floors.get(idx) {
                        Self::test_eye_floor_tri(origin, target, tri, mode, best_dist_sq, best_hit);
                    }
                }
            }
        } else {
            for tri in floors {
                Self::test_eye_floor_tri(origin, target, tri, mode, best_dist_sq, best_hit);
            }
        }
    }

    /// Internal helper: test a sector's static walls, optional side walls, ceilings, and BG columns.
    #[allow(clippy::too_many_arguments)]
    fn test_sector_static_walls_and_columns(
        origin: Vec3,
        target: Vec3,
        grid: &SpatialGrid,
        active_cells: Option<&[usize]>,
        raw_walls: &[HitPolyPlane],
        raw_side_walls: &[HitPolyPlane],
        raw_ceilings: &[HitPolyPlane],
        columns: &[HitPolyColumn],
        walls: &[WallSegment],
        wall_mode: EyeRayMode,
        ceiling_mode: EyeRayMode,
        include_side_walls: bool,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        if !raw_walls.is_empty() || !raw_ceilings.is_empty() || !raw_side_walls.is_empty() {
            if let Some(cells) = active_cells {
                Self::test_eye_planes_cells(
                    origin,
                    target,
                    raw_walls,
                    &grid.wall_cells,
                    cells,
                    wall_mode,
                    best_dist_sq,
                    best_hit,
                );
                if include_side_walls {
                    Self::test_eye_planes_cells(
                        origin,
                        target,
                        raw_side_walls,
                        &grid.side_wall_cells,
                        cells,
                        wall_mode,
                        best_dist_sq,
                        best_hit,
                    );
                }
                Self::test_eye_planes_cells(
                    origin,
                    target,
                    raw_ceilings,
                    &grid.ceiling_cells,
                    cells,
                    ceiling_mode,
                    best_dist_sq,
                    best_hit,
                );
            } else {
                Self::test_eye_planes(origin, target, raw_walls, wall_mode, best_dist_sq, best_hit);
                if include_side_walls {
                    Self::test_eye_planes(
                        origin,
                        target,
                        raw_side_walls,
                        wall_mode,
                        best_dist_sq,
                        best_hit,
                    );
                }
                Self::test_eye_planes(
                    origin,
                    target,
                    raw_ceilings,
                    ceiling_mode,
                    best_dist_sq,
                    best_hit,
                );
            }
        } else {
            // Fallback for worlds constructed directly via `world.walls`
            let mut test_wall_seg = |w: &WallSegment| {
                if w.group == 3 && !include_side_walls {
                    return;
                }
                if should_skip_material_for_mode(w.material, wall_mode) {
                    return;
                }
                let plane = HitPolyPlane::from_wall_segment(w);
                if let Some((t, point, normal)) = cl_check_sub_line_to_plane(origin, target, &plane)
                {
                    let dist_sq = origin.distance_squared(point);
                    if dist_sq < *best_dist_sq {
                        *best_dist_sq = dist_sq;
                        *best_hit = Some(EyeRayHit {
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
            };
            if let Some(cells) = active_cells {
                for &cell in cells {
                    for &idx in &grid.wall_segment_cells[cell] {
                        if let Some(w) = walls.get(idx) {
                            test_wall_seg(w);
                        }
                    }
                }
            } else {
                for w in walls {
                    test_wall_seg(w);
                }
            }
        }

        if let Some(cells) = active_cells {
            for &cell in cells {
                for &idx in &grid.column_cells[cell] {
                    if let Some(col) = columns.get(idx) {
                        Self::test_eye_bg_column(origin, target, col, best_dist_sq, best_hit);
                    }
                }
            }
        } else {
            for col in columns {
                Self::test_eye_bg_column(origin, target, col, best_dist_sq, best_hit);
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
            if !df.active {
                continue;
            }
            Self::test_eye_floor_tri(origin, target, &df.triangle, mode, best_dist_sq, best_hit);
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

    /// Internal helper: test static `.cld` floors across all active sectors via `SpatialGrid`.
    fn test_eye_static_floors(
        &self,
        origin: Vec3,
        target: Vec3,
        mode: EyeRayMode,
        best_dist_sq: &mut f32,
        best_hit: &mut Option<EyeRayHit>,
    ) {
        self.for_each_active_ray_sector(origin.x, origin.z, target.x, target.z, |sector| {
            Self::test_sector_static_floors(
                origin,
                target,
                sector.grid,
                sector.active_cells,
                sector.raw_floors,
                sector.floors,
                mode,
                best_dist_sq,
                best_hit,
            );
        });
    }

    /// Exact 1:1 3D `clCheckHitEyes` (`SLUS_202.28` `0x00138ea0`) with per-sector `SpatialGrid`
    /// acceleration (`clGetHitSectListVECHIT` `0x00138170`):
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

        // 2. Static geometry (`fld`, `wld`, optional `swd`, `ced`, `cld` BG columns) per sector
        let include_side_walls = mode == EyeRayMode::AllNoThru;
        self.for_each_active_ray_sector(origin.x, origin.z, target.x, target.z, |sector| {
            Self::test_sector_static_floors(
                origin,
                target,
                sector.grid,
                sector.active_cells,
                sector.raw_floors,
                sector.floors,
                mode,
                &mut best_dist_sq,
                &mut best_hit,
            );
            Self::test_sector_static_walls_and_columns(
                origin,
                target,
                sector.grid,
                sector.active_cells,
                sector.raw_walls,
                sector.raw_side_walls,
                sector.raw_ceilings,
                sector.columns,
                sector.walls,
                mode,
                mode,
                include_side_walls,
                &mut best_dist_sq,
                &mut best_hit,
            );
        });

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
        self.for_each_active_ray_sector(origin.x, origin.z, target.x, target.z, |sector| {
            Self::test_sector_static_walls_and_columns(
                origin,
                target,
                sector.grid,
                sector.active_cells,
                sector.raw_walls,
                &[],
                sector.raw_ceilings,
                sector.columns,
                &[],
                EyeRayMode::NoThru,
                EyeRayMode::Thru,
                false,
                &mut best_dist_sq,
                &mut best_hit,
            );
        });
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
        self.for_each_active_ray_sector(origin.x, origin.z, target.x, target.z, |sector| {
            if let Some(cells) = sector.active_cells {
                Self::test_eye_planes_cells(
                    origin,
                    target,
                    sector.raw_ceilings,
                    &sector.grid.ceiling_cells,
                    cells,
                    EyeRayMode::Thru,
                    &mut best_dist_sq,
                    &mut best_hit,
                );
            } else {
                Self::test_eye_planes(
                    origin,
                    target,
                    sector.raw_ceilings,
                    EyeRayMode::Thru,
                    &mut best_dist_sq,
                    &mut best_hit,
                );
            }
        });
        best_hit
    }

    /// `SpatialGrid`-accelerated AI line-of-sight check matching `enCheckHitEyes` (`SLUS_202.28`
    /// `0x00145540`) / `enCheckHitEyes2` (`0x00145770`) and [`crate::ai::sensory::check_hit_eyes`].
    ///
    /// - Returns **negative Euclidean distance** (`-dist`) when line-of-sight is **clear** (`< 0.0`).
    /// - Returns **positive hit distance** (`+hit_dist >= 0.0`) when occluded by an active `.cld` wall.
    pub fn check_ai_los(&self, eye_from: Vec3, eye_to: Vec3) -> f32 {
        let total_dist = (eye_to - eye_from).length();
        if total_dist <= 1e-4 {
            return -total_dist;
        }

        let ray_y = (eye_from.y + eye_to.y) * 0.5;
        let mut best_t: Option<f32> = None;

        let mut test_wall = |w: &WallSegment| {
            if w.group == 2 || !(w.y_top <= ray_y && ray_y <= w.y_bottom) {
                return;
            }
            if let Some(t) = intersect_segment_xz_t(eye_from.x, eye_from.z, eye_to.x, eye_to.z, w) {
                if t > 0.0 && t < 1.0 && best_t.is_none_or(|b| t < b) {
                    best_t = Some(t);
                }
            }
        };

        self.for_each_ray_wall_segment(eye_from.x, eye_from.z, eye_to.x, eye_to.z, &mut test_wall);
        for w in &self.extra_walls {
            test_wall(w);
        }
        for dw in &self.dynamic_walls {
            if dw.active {
                test_wall(&dw.segment);
            }
        }

        match best_t {
            Some(t) => total_dist * t,
            None => -total_dist,
        }
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

        let mut best_t = 1.0f32;
        let dy = camera_pos.y - target.y;
        let mut test_wall = |w: &WallSegment| {
            if !filter.include_ceilings && w.group == 2 {
                return;
            }
            if is_cld_material_thru(w.material) {
                return;
            }
            if let Some(t) =
                intersect_segment_xz_t(target.x, target.z, camera_pos.x, camera_pos.z, w)
            {
                let y_at_hit = target.y + dy * t;
                if w.y_top <= y_at_hit && w.y_bottom >= y_at_hit && t >= 0.0 && t < best_t {
                    best_t = t;
                }
            }
        };
        self.for_each_ray_wall_segment(
            target.x,
            target.z,
            camera_pos.x,
            camera_pos.z,
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
}
