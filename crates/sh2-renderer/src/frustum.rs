//! Gribb-Hartmann view-frustum plane extraction and AABB intersection testing for `wgpu`
//! clip space (`x in [-w, w]`, `y in [-w, w]`, `z in [0, w]`).
//!
//! Used to cull offscreen [`crate::gpu_map::GpuMapBatch`]es across main scene, shadow depth,
//! planar mirror reflection, and Toluca Lake water reflection passes using each pass's active
//! `view_proj` matrix.

use glam::{Mat4, Vec3, Vec4};

/// Six inward-facing clip planes `(nx, ny, nz, d)` defining a view frustum in world space.
///
/// A world point `p` is inside plane `i` iff `nx * p.x + ny * p.y + nz * p.z + d >= 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    /// Planes ordered `[left, right, bottom, top, near, far]`.
    pub planes: [Vec4; 6],
}

impl Frustum {
    /// Extracts the six frustum planes from a column-major `view_proj` matrix targeting `wgpu`
    /// NDC (`x in [-1, 1]`, `y in [-1, 1]`, `z in [0, 1]`) via the Gribb-Hartmann method.
    ///
    /// Works identically for standard right-handed perspective, orthographic, oblique near-plane,
    /// and negative-determinant reflected (`main_vp * reflection_matrix`) projections because clip
    /// inequalities are linear in the rows of `view_proj`.
    ///
    /// Returns `None` if `view_proj` is zero or contains any non-finite (`NaN`/`Inf`) entries.
    #[must_use]
    pub fn from_view_proj(view_proj: Mat4) -> Option<Self> {
        let cols = view_proj.to_cols_array();
        if !cols.iter().all(|v| v.is_finite()) || cols.iter().all(|&v| v == 0.0) {
            return None;
        }

        let r0 = view_proj.row(0);
        let r1 = view_proj.row(1);
        let r2 = view_proj.row(2);
        let r3 = view_proj.row(3);

        let raw = [
            r3 + r0, // Left:   x >= -w
            r3 - r0, // Right:  x <=  w
            r3 + r1, // Bottom: y >= -w
            r3 - r1, // Top:    y <=  w
            r2,      // Near:   z >=  0  (wgpu NDC z in [0, 1])
            r3 - r2, // Far:    z <=  w
        ];

        let mut planes = [Vec4::ZERO; 6];
        for (dst, plane) in planes.iter_mut().zip(raw) {
            let normal_len = Vec3::new(plane.x, plane.y, plane.z).length();
            let normalized = if normal_len > 1e-12 {
                plane / normal_len
            } else {
                plane
            };
            if !normalized.is_finite() {
                return None;
            }
            *dst = normalized;
        }

        Some(Self { planes })
    }

    /// Returns `true` if the world-space axis-aligned bounding box `[bounds_min, bounds_max]`
    /// intersects or lies inside this frustum.
    ///
    /// Conservative guard: if any bound coordinate is non-finite or inverted (`min > max`), this
    /// returns `true` so batches with empty or uninitialized bounds are never falsely culled.
    #[inline]
    #[must_use]
    pub fn intersects_aabb(&self, bounds_min: [f32; 3], bounds_max: [f32; 3]) -> bool {
        for axis in 0..3 {
            let lo = bounds_min[axis];
            let hi = bounds_max[axis];
            if !lo.is_finite() || !hi.is_finite() || lo > hi {
                return true;
            }
        }

        for plane in &self.planes {
            // Positive vertex (p-vertex): corner of the AABB furthest along the inward plane normal.
            let px = if plane.x >= 0.0 {
                bounds_max[0]
            } else {
                bounds_min[0]
            };
            let py = if plane.y >= 0.0 {
                bounds_max[1]
            } else {
                bounds_min[1]
            };
            let pz = if plane.z >= 0.0 {
                bounds_max[2]
            } else {
                bounds_min[2]
            };
            if plane.x * px + plane.y * py + plane.z * pz + plane.w < 0.0 {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::{oblique_near_plane_projection, PlanarMirrorSpec};
    use crate::water::mirrored_reflection_view_proj;

    fn test_perspective_vp() -> Mat4 {
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 10.0, 100.0);
        // Camera at origin looking down -Z (standard RH), Y-up.
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        proj * view
    }

    #[test]
    fn inside_aabb_passes_all_six_planes() {
        let f = Frustum::from_view_proj(test_perspective_vp()).expect("valid frustum");
        // Center of frustum at z = -50
        assert!(f.intersects_aabb([-5.0, -5.0, -55.0], [5.0, 5.0, -45.0]));
        // Just inside near plane (z = -10.5)
        assert!(f.intersects_aabb([-1.0, -1.0, -11.0], [1.0, 1.0, -10.5]));
        // Just inside far plane (z = -99.5)
        assert!(f.intersects_aabb([-1.0, -1.0, -99.5], [1.0, 1.0, -99.0]));
    }

    #[test]
    fn outside_each_of_the_six_planes_is_culled() {
        let f = Frustum::from_view_proj(test_perspective_vp()).expect("valid frustum");
        // At z = -50 with 90 deg FOV, aspect 1.0, visible X and Y range is [-50, 50].
        // Left: x < -50
        assert!(!f.intersects_aabb([-70.0, -5.0, -55.0], [-60.0, 5.0, -45.0]));
        // Right: x > 50
        assert!(!f.intersects_aabb([60.0, -5.0, -55.0], [70.0, 5.0, -45.0]));
        // Bottom: y < -50
        assert!(!f.intersects_aabb([-5.0, -70.0, -55.0], [5.0, -60.0, -45.0]));
        // Top: y > 50
        assert!(!f.intersects_aabb([-5.0, 60.0, -55.0], [5.0, 70.0, -45.0]));
        // Near (wgpu z in [0, 1]): camera looks down -Z with near = 10, so z in [-9, -1] is in front of near!
        assert!(!f.intersects_aabb([-2.0, -2.0, -9.0], [2.0, 2.0, -1.0]));
        // Behind camera (z > 0)
        assert!(!f.intersects_aabb([-2.0, -2.0, 5.0], [2.0, 2.0, 15.0]));
        // Far: z < -100
        assert!(!f.intersects_aabb([-5.0, -5.0, -120.0], [5.0, 5.0, -101.0]));
    }

    #[test]
    fn straddling_and_enclosing_aabbs_pass() {
        let f = Frustum::from_view_proj(test_perspective_vp()).expect("valid frustum");
        // Straddling left plane at z = -50 (x = -50)
        assert!(f.intersects_aabb([-60.0, -5.0, -55.0], [-45.0, 5.0, -45.0]));
        // Straddling near plane (z = -10)
        assert!(f.intersects_aabb([-2.0, -2.0, -12.0], [2.0, 2.0, -5.0]));
        // Straddling far plane (z = -100)
        assert!(f.intersects_aabb([-5.0, -5.0, -110.0], [5.0, 5.0, -95.0]));
        // Enclosing the entire frustum
        assert!(f.intersects_aabb([-500.0, -500.0, -500.0], [500.0, 500.0, 500.0]));
    }

    #[test]
    fn invalid_or_empty_bounds_are_never_culled() {
        let f = Frustum::from_view_proj(test_perspective_vp()).expect("valid frustum");
        assert!(f.intersects_aabb([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]));
        assert!(f.intersects_aabb([10.0, 0.0, -50.0], [5.0, 1.0, -45.0]));
        assert!(f.intersects_aabb([f32::NAN, 0.0, -50.0], [1.0, 1.0, -45.0]));
    }

    #[test]
    fn zero_or_nan_matrix_returns_none() {
        assert!(Frustum::from_view_proj(Mat4::ZERO).is_none());
        let mut nan_mat = Mat4::IDENTITY;
        nan_mat.x_axis.x = f32::NAN;
        assert!(Frustum::from_view_proj(nan_mat).is_none());
    }

    #[test]
    fn mirrored_er01_reflection_view_proj_culls_correctly() {
        // Camera in er01 restroom looking toward the mirror at X = -20000.0
        let cam_pos = Vec3::new(-18500.0, -1200.0, 20450.0);
        let look_at = Vec3::new(-20000.0, -1200.0, 20450.0);
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 4.0 / 3.0, 25.0, 150_000.0);
        let view = Mat4::look_at_rh(cam_pos, look_at, Vec3::NEG_Y);
        let main_vp = proj * view;

        let spec = PlanarMirrorSpec::for_room(0x01).expect("er01 mirror");
        let refl_vp = main_vp * spec.reflection_matrix();
        let refl_frustum = Frustum::from_view_proj(refl_vp).expect("reflected frustum");

        // Real-room geometry behind the camera (X = -17000) IS visible in the mirror reflection!
        assert!(
            refl_frustum.intersects_aabb([-17200.0, -1500.0, 20200.0], [-16800.0, -900.0, 20700.0]),
            "real-room wall behind camera must be visible in reflected frustum"
        );
        // Geometry far behind the mirror wall (X = -25000) is behind the reflected camera and culled!
        assert!(
            !refl_frustum
                .intersects_aabb([-25500.0, -1500.0, 20200.0], [-24500.0, -900.0, 20700.0]),
            "geometry behind mirror plane and behind reflected camera must be culled"
        );
    }

    #[test]
    fn water_reflection_view_proj_culls_correctly() {
        let plane_y = 500.0; // PS2 Y-down water surface
        let cam_pos = Vec3::new(0.0, -500.0, 0.0); // 1000 mm above water
        let look_at = Vec3::new(0.0, 0.0, 2000.0);
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 16.0 / 9.0, 25.0, 100_000.0);
        let view = Mat4::look_at_rh(cam_pos, look_at, Vec3::NEG_Y);
        let main_vp = proj * view;

        let refl_vp = mirrored_reflection_view_proj(main_vp, plane_y);
        let refl_frustum = Frustum::from_view_proj(refl_vp).expect("water reflected frustum");

        // Object above the water surface (y = -800) in front of camera should be visible in reflection
        assert!(refl_frustum.intersects_aabb([-200.0, -1000.0, 1800.0], [200.0, -200.0, 2200.0]));
        // Object behind the camera (z = -5000) must be culled
        assert!(!refl_frustum.intersects_aabb([-200.0, -1000.0, -6000.0], [200.0, -200.0, -4000.0]));
    }

    #[test]
    fn oblique_near_plane_frustum_culls_behind_clip_plane() {
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 10.0, 200.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        // Clip plane at z = -40 in view space, normal pointing toward -Z (kept side z <= -40).
        let oblique_proj = oblique_near_plane_projection(proj, Vec4::new(0.0, 0.0, -1.0, -40.0));
        let f = Frustum::from_view_proj(oblique_proj * view).expect("oblique frustum");

        // Box at z in [-30, -20] is in front of the oblique near plane (z > -40) -> culled!
        assert!(!f.intersects_aabb([-2.0, -2.0, -30.0], [2.0, 2.0, -20.0]));
        // Box at z in [-80, -60] is beyond the oblique near plane -> visible!
        assert!(f.intersects_aabb([-2.0, -2.0, -80.0], [2.0, 2.0, -60.0]));
    }
}
