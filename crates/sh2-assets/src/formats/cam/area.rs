//! Oriented 3D bounding volume (`_VC_ROAD_AREA`) geometry and distance queries.

/// Bounding region in 3D world space (`_VC_ROAD_AREA`, 32 bytes).
///
/// In PS2 world coordinates:
/// - `(x0, z0)`, `(x1, z1)`, `(x2, z2)` define the horizontal rectangular envelope.
/// - `min_hy`: Lower Y limit (numerically larger float because Y points down).
/// - `max_hy`: Upper Y limit (numerically smaller float because Y points down).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoadArea {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
    pub x2: f32,
    pub z2: f32,
    pub min_hy: f32,
    pub max_hy: f32,
}

impl RoadArea {
    /// Returns the oriented rectangle's Y-axis rotation angle in radians (`SLUS_202.28`
    /// `vcSetNearRoadAryByCharaPos` `0x00197310`: `shAtan2(z0 - z1, x0 - x1)` = `(x0 - x1).atan2(z0 - z1)`).
    #[inline]
    pub fn orientation_angle_y(&self) -> f32 {
        (self.x0 - self.x1).atan2(self.z0 - self.z1)
    }

    /// Transforms a world XZ point `(x, z)` (PS2 world mm) into this volume's local oriented XZ
    /// frame (`SLUS_202.28` `vcTransRotRoadArea` `0x001a6b40` + `vcSetNearRoadAryByCharaPos` `0x00197310`),
    /// anchored at corner `(x1, z1)` rotated by `-orientation_angle_y()`.
    #[inline]
    pub fn world_to_local_xz(&self, x: f32, z: f32) -> (f32, f32) {
        let ang = self.orientation_angle_y();
        let (s, c) = ang.sin_cos();
        let dx = x - self.x1;
        let dz = z - self.z1;
        (dx * c - dz * s, dx * s + dz * c)
    }

    /// Transforms a local oriented XZ point `(lx, lz)` (PS2 mm) back into world XZ coordinates
    /// (`SLUS_202.28` `vcRotTransRoadArea` `0x001a6b80` + `vcSetNearRoadAryByCharaPos` `0x00197310`),
    /// rotating by `+orientation_angle_y()` and translating by `(x1, z1)`.
    #[inline]
    pub fn local_to_world_xz(&self, lx: f32, lz: f32) -> (f32, f32) {
        let ang = self.orientation_angle_y();
        let (s, c) = ang.sin_cos();
        (lx * c + lz * s + self.x1, -lx * s + lz * c + self.z1)
    }

    /// Returns the axis-aligned bounds `((min_lx, max_lx), (min_lz, max_lz))` in this volume's
    /// local oriented XZ frame (`SLUS_202.28` `vcSetNearRoadAryByCharaPos` `0x00197310`).
    #[inline]
    pub fn local_xz_bounds(&self) -> ((f32, f32), (f32, f32)) {
        let (lx0, lz0) = self.world_to_local_xz(self.x0, self.z0);
        let (lx2, lz2) = self.world_to_local_xz(self.x2, self.z2);
        ((lx0.min(lx2), lx0.max(lx2)), (lz0.min(lz2), lz0.max(lz2)))
    }

    /// Computes Team Silent's signed/unsigned Manhattan-style boundary distance in the volume's
    /// oriented local XZ frame (`SLUS_202.28` `vcGetXZSumDistFromLimArea` `0x0019d980`).
    ///
    /// Returns `(sum_dist, ofs_x, ofs_z)` in PS2 world millimeters:
    /// - Outside the volume: `sum_dist > 0.0` (sum of positive X/Z overflows).
    /// - Inside the volume: `0.0` when `merge_road == false`, or negative distance to the nearest
    ///   edge when `merge_road == true` (`VC_RD_MARGE_ROAD_F` `0x0080`).
    pub fn xz_sum_dist_from_lim_area(
        &self,
        world_x: f32,
        world_z: f32,
        merge_road: bool,
    ) -> (f32, f32, f32) {
        let (lx, lz) = self.world_to_local_xz(world_x, world_z);
        let ((min_x, max_x), (min_z, max_z)) = self.local_xz_bounds();
        Self::xz_sum_dist_in_local_bounds(lx, lz, min_x, max_x, min_z, max_z, merge_road)
    }

    /// Exact port of `SLUS_202.28` `vcGetXZSumDistFromLimArea` (`0x0019d980`) on pre-transformed
    /// local coordinates `(lx, lz)` and local bounds `[min_x, max_x] x [min_z, max_z]`.
    pub fn xz_sum_dist_in_local_bounds(
        lx: f32,
        lz: f32,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
        merge_road: bool,
    ) -> (f32, f32, f32) {
        let (dx, ofs_x) = if max_x < lx {
            let ofs = max_x - lx;
            (-ofs, ofs)
        } else if lx < min_x {
            let d = min_x - lx;
            (d, d)
        } else {
            let d = if (max_x + min_x) * 0.5 <= lx {
                lx - max_x
            } else {
                min_x - lx
            };
            (d, 0.0)
        };

        let (dz, ofs_z) = if max_z < lz {
            let ofs = max_z - lz;
            (-ofs, ofs)
        } else if lz < min_z {
            let d = min_z - lz;
            (d, d)
        } else {
            let d = if (max_z + min_z) * 0.5 <= lz {
                lz - max_z
            } else {
                min_z - lz
            };
            (d, 0.0)
        };

        let mut sum = if dx >= 0.0 {
            if dz >= 0.0 {
                dx + dz
            } else {
                dx
            }
        } else if dz >= 0.0 {
            dz
        } else {
            dx.max(dz)
        };

        if !merge_road && sum < 0.0 {
            sum = 0.0;
        }
        (sum, ofs_x, ofs_z)
    }

    /// Returns the world-space AABB minimum and maximum X coordinate envelope of the 4 corners.
    pub fn x_bounds(&self) -> (f32, f32) {
        let x3 = self.x0 + self.x2 - self.x1;
        let min_x = self.x0.min(self.x1).min(self.x2).min(x3);
        let max_x = self.x0.max(self.x1).max(self.x2).max(x3);
        (min_x, max_x)
    }

    /// Returns the world-space AABB minimum and maximum Z coordinate envelope of the 4 corners.
    pub fn z_bounds(&self) -> (f32, f32) {
        let z3 = self.z0 + self.z2 - self.z1;
        let min_z = self.z0.min(self.z1).min(self.z2).min(z3);
        let max_z = self.z0.max(self.z1).max(self.z2).max(z3);
        (min_z, max_z)
    }

    /// Returns the vertical Y bounds `(min_y, max_y)`.
    /// Note: in PS2 coords, `max_hy` is numerically lower than `min_hy`.
    pub fn y_bounds(&self) -> (f32, f32) {
        let min_y = self.min_hy.min(self.max_hy);
        let max_y = self.min_hy.max(self.max_hy);
        (min_y, max_y)
    }

    /// Returns center point `(x, y, z)` in world units (exact center of the oriented rectangle).
    pub fn center(&self) -> [f32; 3] {
        let ((min_lx, max_lx), (min_lz, max_lz)) = self.local_xz_bounds();
        let (cx, cz) = self.local_to_world_xz((min_lx + max_lx) * 0.5, (min_lz + max_lz) * 0.5);
        let y = (self.min_hy + self.max_hy) * 0.5;
        [cx, y, cz]
    }

    /// Checks if a 3D point `[x, y, z]` lies inside this oriented bounding volume (`vcTransRotRoadArea` `0x001a6b40`).
    pub fn contains_point(&self, pos: [f32; 3], margin: f32) -> bool {
        let (lx, lz) = self.world_to_local_xz(pos[0], pos[2]);
        let ((min_lx, max_lx), (min_lz, max_lz)) = self.local_xz_bounds();
        let (min_y, max_y) = self.y_bounds();

        lx >= (min_lx - margin)
            && lx <= (max_lx + margin)
            && lz >= (min_lz - margin)
            && lz <= (max_lz + margin)
            && pos[1] >= (min_y - margin)
            && pos[1] <= (max_y + margin)
    }

    /// Computes the 2D Euclidean distance from `(x, z)` to this oriented horizontal rectangle
    /// (`vcTransRotRoadArea` `0x001a6b40`). Returns `0.0` if inside.
    pub fn distance_to_xz(&self, x: f32, z: f32) -> f32 {
        let (lx, lz) = self.world_to_local_xz(x, z);
        let ((min_lx, max_lx), (min_lz, max_lz)) = self.local_xz_bounds();

        let dx = if lx < min_lx {
            min_lx - lx
        } else if lx > max_lx {
            lx - max_lx
        } else {
            0.0
        };

        let dz = if lz < min_lz {
            min_lz - lz
        } else if lz > max_lz {
            lz - max_lz
        } else {
            0.0
        };

        (dx * dx + dz * dz).sqrt()
    }
}
