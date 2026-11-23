//! Full 3D `clCheckHitEyes` (`SLUS_202.28` `0x00138ea0`) and specialized floor/wall/ceiling
//! raycast methods on [`CollisionWorld`].
//!
//! Extracted from `controller.rs` to keep `CollisionWorld` modular (`AGENTS.md` Rule 2).

use glam::Vec3;
use sh2_assets::formats::cld::HitPolyPlane;

use super::character::{CharacterCylinder, CharacterId};
use super::controller::CollisionWorld;
use super::raycast::{
    cl_check_sub_line_to_column_plus, cl_check_sub_line_to_plane, clamp_camera_ray,
    intersect_ray_triangle_3d, intersect_segment_bg_column, should_skip_material_for_mode,
    EyeHitKind, EyeRayHit, EyeRayMode, RaycastFilter,
};

impl CollisionWorld {
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
}
