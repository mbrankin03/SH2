//! Planar mirror plane definitions, reflection matrices, and oblique near-plane clipping math.
//!
//! Ground-truth mirror plane coordinates and axes are derived 1:1 from:
//! - `SLUS_202.28` `PlayerSetReverseMode` (`0x0024c190`, `Chacter/m3_rpjames.c`)
//! - `SLUS_202.28` `RObjectInit` (`0x00104250`, `Chacter/m3_bgobj.c`)
//! - `sh2pc.exe` `FUN_00546900` (`0x00546900`)
//!
//! Prototype-vs-retail drift:
//! - `sh2pc.exe` `FUN_00546900` adds `room_id == 0xd5` (Born From a Wish Mansion, `AxisZ`,
//!   `wall_pos = 20750.0` / `0x46a21c00`) and disables `0xab, 0xb6, 0xb7, 0xb9` floor reflections
//!   that were enabled in `SLUS_202.28`.

use glam::{Mat4, Vec3, Vec4};
use sh2_assets::formats::map::{BatchGroup, MapFile};

use crate::pipeline::CameraUniforms;

/// Mirror wall X coordinate in `er01.map` (`room_id == 0x01`, `SLUS_202.28` `0x0024c190`: `0xc69c4000`).
pub const MIRROR_WALL_ER01_X_MM: f32 = -20_000.0;

/// Mirror wall Z coordinate in `ap84.map` / Blue Creek Rm 109 (`room_id == 0x24`, `SLUS_202.28` `0x0024c190`: `0xc7c34d80`).
pub const MIRROR_WALL_AP84_Z_MM: f32 = -99_995.0;

/// Mirror wall Z coordinate in Born From a Wish Mansion (`room_id == 0xd5`, `sh2pc.exe` `0x00546900`: `0x46a21c00`).
pub const MIRROR_WALL_BFW_D5_Z_MM: f32 = 20_750.0;

/// Full restroom mirror quad for `er01.map` (`X = -20000.0`, spanning above the sink between the wall pipes).
/// Note: `er01.map` `batch[17]` (`Decal`) only covers a small stain patch in front of James's chest,
/// so the stencil mask uses the full mirror opening quad while `batch[17]` still blends on top.
pub const ER01_FALLBACK_QUAD: [[f32; 3]; 4] = [
    [MIRROR_WALL_ER01_X_MM, -1560.0, 18480.0],
    [MIRROR_WALL_ER01_X_MM, -650.0, 18480.0],
    [MIRROR_WALL_ER01_X_MM, -650.0, 21510.0],
    [MIRROR_WALL_ER01_X_MM, -1560.0, 21510.0],
];

/// Default fallback mirror quad for `ap84.map` (Blue Creek Rm 109) extracted from retail `batch[1]` (`Decal`).
pub const AP84_FALLBACK_QUAD: [[f32; 3]; 4] = [
    [59154.992, -1057.4408, MIRROR_WALL_AP84_Z_MM],
    [60669.996, -1057.4408, MIRROR_WALL_AP84_Z_MM],
    [60669.996, -57.92508, MIRROR_WALL_AP84_Z_MM],
    [59154.992, -57.92508, MIRROR_WALL_AP84_Z_MM],
];

/// Reflection axis matching `SLUS_202.28` `_mirror_mode` (`0x0024c190`) and `sh2pc.exe` `DAT_01d7f880` (`0x00546900`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanarMirrorAxis {
    /// `_mirror_mode == 0`: reflection across `X = wall_pos`.
    AxisX = 0,
    /// `_mirror_mode == 1`: reflection across `Y = wall_pos`.
    AxisY = 1,
    /// `_mirror_mode == 2`: reflection across `Z = wall_pos`.
    AxisZ = 2,
}

/// Complete specification of an active planar mirror surface for stenciled reflection rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanarMirrorSpec {
    /// Reflection axis (`0` = X, `1` = Y, `2` = Z).
    pub axis: PlanarMirrorAxis,
    /// Point lying on the mirror plane in PS2 world mm.
    pub plane_point: [f32; 3],
    /// Unit normal pointing into the real room (half-space `dot(n, p - p0) >= 0`).
    pub plane_normal: [f32; 3],
    /// World-space corners of the mirror quad (used when no `.map` decal batch is matched).
    pub quad_vertices: [[f32; 3]; 4],
    /// Optional index into `GpuMap::batches` for the authentic `.map` mirror decal stencil mask.
    pub decal_batch_index: Option<usize>,
    /// Tolerance in mm behind the mirror plane allowed in the main pass before clipping pre-baked PS2
    /// duplicate room geometry (`dot(n, p - p0) < -main_pass_clip_bias_mm` is discarded).
    pub main_pass_clip_bias_mm: f32,
}

impl PlanarMirrorSpec {
    /// Returns the planar mirror specification for a retail `room_id` if it has a vertical wall mirror
    /// (`SLUS_202.28` `0x0024c190` / `sh2pc.exe` `0x00546900`), without inspecting `.map` batches.
    #[must_use]
    pub const fn for_room(room_id: u32) -> Option<Self> {
        match room_id {
            0x01 => Some(Self {
                axis: PlanarMirrorAxis::AxisX,
                plane_point: [MIRROR_WALL_ER01_X_MM, 0.0, 0.0],
                plane_normal: [1.0, 0.0, 0.0],
                quad_vertices: ER01_FALLBACK_QUAD,
                decal_batch_index: None,
                main_pass_clip_bias_mm: 1.0,
            }),
            0x24 => Some(Self {
                axis: PlanarMirrorAxis::AxisZ,
                plane_point: [0.0, 0.0, MIRROR_WALL_AP84_Z_MM],
                plane_normal: [0.0, 0.0, 1.0],
                quad_vertices: AP84_FALLBACK_QUAD,
                decal_batch_index: None,
                main_pass_clip_bias_mm: 1.0,
            }),
            0xd5 => Some(Self {
                axis: PlanarMirrorAxis::AxisZ,
                plane_point: [0.0, 0.0, MIRROR_WALL_BFW_D5_Z_MM],
                plane_normal: [0.0, 0.0, -1.0],
                quad_vertices: [
                    [-1000.0, -2500.0, MIRROR_WALL_BFW_D5_Z_MM],
                    [1000.0, -2500.0, MIRROR_WALL_BFW_D5_Z_MM],
                    [1000.0, 0.0, MIRROR_WALL_BFW_D5_Z_MM],
                    [-1000.0, 0.0, MIRROR_WALL_BFW_D5_Z_MM],
                ],
                decal_batch_index: None,
                main_pass_clip_bias_mm: 1.0,
            }),
            _ => None,
        }
    }

    /// Resolves a [`PlanarMirrorSpec`] for `room_id` and locates the matching mirror [`BatchGroup::Decal`]
    /// batch inside `map` (e.g. `er01.map` `batch[17]` at `X = -20000.0`, `ap84.map` `batch[1]` at `Z = -99995.0`).
    #[must_use]
    pub fn for_room_and_map(room_id: u32, map: &MapFile) -> Option<Self> {
        let mut spec = Self::for_room(room_id)?;
        // `er01.map` `batch[17]` is only a small stain decal; use the full mirror quad for `0x01`.
        if room_id != 0x01 {
            spec.decal_batch_index = find_mirror_decal_batch(map, spec.axis, spec.wall_pos());
        }
        Some(spec)
    }

    /// Resolves a [`PlanarMirrorSpec`] by `.map` name (`"er01"`, `"ap84"`, `"ap88"`, etc.) and inspects `map`.
    #[must_use]
    pub fn for_map_name(map_name: &str, map: &MapFile) -> Option<Self> {
        let room_id = if map_name.eq_ignore_ascii_case("er01") {
            0x01
        } else if map_name.eq_ignore_ascii_case("ap84") || map_name.eq_ignore_ascii_case("ap88") {
            0x24
        } else {
            return None;
        };
        Self::for_room_and_map(room_id, map)
    }

    /// Scalar wall coordinate along [`Self::axis`] in PS2 mm.
    #[inline]
    #[must_use]
    pub const fn wall_pos(&self) -> f32 {
        match self.axis {
            PlanarMirrorAxis::AxisX => self.plane_point[0],
            PlanarMirrorAxis::AxisY => self.plane_point[1],
            PlanarMirrorAxis::AxisZ => self.plane_point[2],
        }
    }

    /// Builds the 4x4 planar reflection matrix across this mirror plane.
    #[inline]
    #[must_use]
    pub fn reflection_matrix(&self) -> Mat4 {
        reflection_matrix(
            Vec3::from_array(self.plane_point),
            Vec3::from_array(self.plane_normal),
        )
    }

    /// Builds the world-space clip plane `(nx, ny, nz, d)` where `dot(n, p) + d >= 0` is kept.
    #[inline]
    #[must_use]
    pub fn clip_plane(&self, bias_mm: f32) -> [f32; 4] {
        let p = Vec3::from_array(self.plane_point);
        let n = Vec3::from_array(self.plane_normal).normalize_or_zero();
        let d = -n.dot(p) + bias_mm;
        [n.x, n.y, n.z, d]
    }
}

/// Searches `map` for a [`BatchGroup::Decal`] batch whose vertices lie flat on `axis == wall_pos`.
#[must_use]
pub fn find_mirror_decal_batch(
    map: &MapFile,
    axis: PlanarMirrorAxis,
    wall_pos: f32,
) -> Option<usize> {
    let comp = match axis {
        PlanarMirrorAxis::AxisX => 0,
        PlanarMirrorAxis::AxisY => 1,
        PlanarMirrorAxis::AxisZ => 2,
    };
    const EPS_MM: f32 = 1.0;
    map.batches.iter().enumerate().find_map(|(idx, batch)| {
        if batch.group != BatchGroup::Decal || batch.positions.is_empty() {
            return None;
        }
        let all_on_plane = batch
            .positions
            .iter()
            .all(|p| (p[comp] - wall_pos).abs() <= EPS_MM);
        all_on_plane.then_some(idx)
    })
}

/// Computes the 4x4 planar reflection matrix `R = I - 2 * n * n^T` around `point` with unit `normal`.
#[must_use]
pub fn reflection_matrix(point: Vec3, normal: Vec3) -> Mat4 {
    let n = normal.normalize_or_zero();
    let d = -n.dot(point);
    Mat4::from_cols(
        Vec4::new(
            1.0 - 2.0 * n.x * n.x,
            -2.0 * n.x * n.y,
            -2.0 * n.x * n.z,
            0.0,
        ),
        Vec4::new(
            -2.0 * n.y * n.x,
            1.0 - 2.0 * n.y * n.y,
            -2.0 * n.y * n.z,
            0.0,
        ),
        Vec4::new(
            -2.0 * n.z * n.x,
            -2.0 * n.z * n.y,
            1.0 - 2.0 * n.z * n.z,
            0.0,
        ),
        Vec4::new(-2.0 * d * n.x, -2.0 * d * n.y, -2.0 * d * n.z, 1.0),
    )
}

/// Reflects a 3D point across the plane `(point, normal)`.
#[inline]
#[must_use]
pub fn reflect_point(p: Vec3, plane_point: Vec3, plane_normal: Vec3) -> Vec3 {
    let n = plane_normal.normalize_or_zero();
    p - 2.0 * (p - plane_point).dot(n) * n
}

/// Computes [`CameraUniforms`] for the reflected mirror pass from `main_cam` and `spec`.
#[must_use]
pub fn reflected_camera_uniforms(
    main_cam: &CameraUniforms,
    spec: &PlanarMirrorSpec,
) -> CameraUniforms {
    let refl = spec.reflection_matrix();
    let main_vp = Mat4::from_cols_array_2d(&main_cam.view_proj);
    let refl_vp = main_vp * refl;
    let cam_pos = Vec3::new(
        main_cam.cam_pos[0],
        main_cam.cam_pos[1],
        main_cam.cam_pos[2],
    );
    let refl_cam_pos = reflect_point(
        cam_pos,
        Vec3::from_array(spec.plane_point),
        Vec3::from_array(spec.plane_normal),
    );
    let inv_view_proj = if refl_vp.determinant().abs() > 1e-12 {
        refl_vp.inverse().to_cols_array_2d()
    } else {
        main_cam.inv_view_proj
    };
    CameraUniforms {
        view_proj: refl_vp.to_cols_array_2d(),
        inv_proj: main_cam.inv_proj,
        cam_pos: [
            refl_cam_pos.x,
            refl_cam_pos.y,
            refl_cam_pos.z,
            main_cam.cam_pos[3],
        ],
        inv_view_proj,
        clip_plane: spec.clip_plane(0.0),
    }
}

/// Modifies a `[0, 1]` NDC depth projection matrix (`wgpu` / Vulkan / DX12) so its near clip plane
/// coincides with `clip_plane_view` (Lengyel's oblique near-plane clipping method).
///
/// `clip_plane_view` must be in view space with the normal pointing toward the camera/kept side.
#[must_use]
pub fn oblique_near_plane_projection(proj: Mat4, clip_plane_view: Vec4) -> Mat4 {
    let mut m = proj;
    let q = m.inverse()
        * Vec4::new(
            clip_plane_view.x.signum(),
            clip_plane_view.y.signum(),
            1.0,
            1.0,
        );
    let denom = clip_plane_view.dot(q);
    if denom.abs() < 1e-6 {
        return proj;
    }
    // For wgpu [0, 1] depth, row 2 (z_ndc = 0 at near plane) is scaled to match the clip plane.
    let c = clip_plane_view * (1.0 / denom);
    m.x_axis.z = c.x;
    m.y_axis.z = c.y;
    m.z_axis.z = c.z;
    m.w_axis.z = c.w;
    m
}
