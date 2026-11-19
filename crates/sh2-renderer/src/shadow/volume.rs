//! Pure CPU `.kg1` / `.kg2` shadow volume extrusion (`sh2shd_shadow_model.c`, `sh2pc.exe`
//! `0x444b20` / `0x4449f0`, up to 4 casters @ `0x5aad70`) and lit-room drop shadow floor
//! flattening (`sh2pc.exe` `0x5a7c90`, `SLUS_202.28` `sh2shd_make_drop_shadow_plane` @ `0x001a5660`).

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use sh2_assets::formats::kg::KgFile;
use std::collections::HashMap;

/// Maximum simultaneous character `.kg1` shadow casters per frame (`sh2pc.exe` `0x5aad70`).
pub const MAX_KG1_SHADOW_CASTERS: usize = 4;

/// Default spotlight shadow volume extrusion distance in PS2 world mm (`12.0 m`).
pub const DEFAULT_SHADOW_EXTRUDE_DIST_MM: f32 = 12_000.0;

/// Small Y-upward offset (in PS2 Y-down space: `-2.0` mm) to prevent drop-shadow Z-fighting on floors.
pub const DROP_SHADOW_FLOOR_BIAS_MM: f32 = 2.0;

/// Compact 12-byte vertex for shadow volume and drop-shadow GPU buffers.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct ShadowVolumeVertex {
    /// World-space position in PS2 mm (Y-down).
    pub position: [f32; 3],
}

/// Quantized vertex key (0.5 mm precision) for silhouette edge adjacency matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct QuantizedPos([i32; 3]);

impl QuantizedPos {
    #[inline]
    fn from_vec3(v: Vec3) -> Self {
        Self([
            (v.x * 2.0).round() as i32,
            (v.y * 2.0).round() as i32,
            (v.z * 2.0).round() as i32,
        ])
    }
}

/// Extrudes a closed Z-fail stencil shadow volume from a character `.kg1` shadow model
/// (`sh2pc.exe` `0x444b20` / `0x4449f0`) away from `light_pos` (PS2 world mm).
///
/// Each `.kg1` part is transformed by `bone_matrices[part.bone_or_object_id]`.
#[must_use]
pub fn extrude_kg1_stencil_volume(
    kg: &KgFile,
    bone_matrices: &[[[f32; 4]; 4]],
    light_pos: Vec3,
    extrude_dist_mm: f32,
) -> Vec<ShadowVolumeVertex> {
    let mut out = Vec::new();
    let dist = extrude_dist_mm.max(500.0);

    for part in &kg.parts {
        let bone_idx = part.bone_or_object_id as usize;
        let mat = bone_matrices
            .get(bone_idx)
            .map(Mat4::from_cols_array_2d)
            .unwrap_or(Mat4::IDENTITY);
        extrude_part_triangles(part, mat, light_pos, dist, &mut out);
    }

    out
}

/// Extrudes a closed Z-fail stencil shadow volume from a static room `.kg2` shadow model
/// (`LoadBg/loadbg_kg2.c`, `SLUS_202.28` `sh2shd_init_outdoor_man2` @ `0x001a0920`) away from
/// `light_pos` (PS2 world mm), using each part's embedded 4x4 `world_matrix`.
#[must_use]
pub fn extrude_kg2_stencil_volume(
    kg: &KgFile,
    light_pos: Vec3,
    extrude_dist_mm: f32,
) -> Vec<ShadowVolumeVertex> {
    let mut out = Vec::new();
    let dist = extrude_dist_mm.max(500.0);

    for part in &kg.parts {
        let mat = if part.has_embedded_matrix() {
            Mat4::from_cols_array_2d(&part.world_matrix)
        } else {
            Mat4::IDENTITY
        };
        extrude_part_triangles(part, mat, light_pos, dist, &mut out);
    }

    out
}

/// Flattens a character `.kg1` shadow model onto a horizontal floor plane `y = floor_y`
/// (`sh2pc.exe` `0x5a7c90`, `SLUS_202.28` `sh2shd_make_drop_shadow_plane` @ `0x001a5660`)
/// for lit rooms or dark rooms with `shadow_darkness == 0`.
///
/// `light_dir` points from the overhead light toward the floor (in PS2 Y-down space, `light_dir.y > 0.0`).
#[must_use]
pub fn flatten_kg1_drop_shadow(
    kg: &KgFile,
    bone_matrices: &[[[f32; 4]; 4]],
    floor_y: f32,
    light_dir: Vec3,
) -> Vec<ShadowVolumeVertex> {
    let mut out = Vec::new();
    let dir = light_dir.normalize_or(Vec3::Y);
    // Avoid grazing-angle infinity by clamping vertical slope
    let dy = if dir.y.abs() < 0.25 {
        0.25f32.copysign(if dir.y == 0.0 { 1.0 } else { dir.y })
    } else {
        dir.y
    };
    let target_y = floor_y - DROP_SHADOW_FLOOR_BIAS_MM;

    for part in &kg.parts {
        let bone_idx = part.bone_or_object_id as usize;
        let mat = bone_matrices
            .get(bone_idx)
            .map(Mat4::from_cols_array_2d)
            .unwrap_or(Mat4::IDENTITY);

        for strip in &part.strips {
            for tri in strip.to_triangles() {
                for v in tri.vertices {
                    let wp = mat.transform_point3(Vec3::from_array(v));
                    // Project along light_dir onto y = target_y
                    let t = ((floor_y - wp.y) / dy).clamp(-1500.0, 2500.0);
                    let px = wp.x + dir.x * t;
                    let pz = wp.z + dir.z * t;
                    out.push(ShadowVolumeVertex {
                        position: [px, target_y, pz],
                    });
                }
            }
        }
    }

    out
}

fn extrude_part_triangles(
    part: &sh2_assets::formats::kg::KgPart,
    mat: Mat4,
    light_pos: Vec3,
    extrude_dist: f32,
    out: &mut Vec<ShadowVolumeVertex>,
) {
    // Collect light-facing triangles and count directed silhouette edges.
    // An edge (a -> b) cancelled by a neighbor's (b -> a) is interior; uncancelled edges form the silhouette!
    let mut edge_counts: HashMap<(QuantizedPos, QuantizedPos), (Vec3, Vec3, i32)> = HashMap::new();

    for strip in &part.strips {
        for tri in strip.to_triangles() {
            let v0 = mat.transform_point3(Vec3::from_array(tri.vertices[0]));
            let v1 = mat.transform_point3(Vec3::from_array(tri.vertices[1]));
            let v2 = mat.transform_point3(Vec3::from_array(tri.vertices[2]));

            let face_n = (v1 - v0).cross(v2 - v0);
            let centroid = (v0 + v1 + v2) * (1.0 / 3.0);
            let to_light = light_pos - centroid;

            // Only triangles facing the light cast the shadow volume caps and silhouette
            if face_n.dot(to_light) <= 0.0 {
                continue;
            }

            // Front cap (original triangle, slightly pushed away from light to avoid self-shadowing)
            let e0 = extrude_point(v0, light_pos, extrude_dist);
            let e1 = extrude_point(v1, light_pos, extrude_dist);
            let e2 = extrude_point(v2, light_pos, extrude_dist);

            push_tri(out, v0, v1, v2);
            // Far extruded back cap (reversed winding to close the volume)
            push_tri(out, e0, e2, e1);

            for (a, b) in [(v0, v1), (v1, v2), (v2, v0)] {
                let qa = QuantizedPos::from_vec3(a);
                let qb = QuantizedPos::from_vec3(b);
                if let Some(entry) = edge_counts.get_mut(&(qb, qa)) {
                    entry.2 -= 1;
                } else {
                    edge_counts
                        .entry((qa, qb))
                        .and_modify(|e| e.2 += 1)
                        .or_insert((a, b, 1));
                }
            }
        }
    }

    // Extrude every remaining silhouette edge into a quad (2 triangles)
    for (_, (a, b, count)) in edge_counts {
        if count == 0 {
            continue;
        }
        let (p0, p1) = if count > 0 { (a, b) } else { (b, a) };
        let e0 = extrude_point(p0, light_pos, extrude_dist);
        let e1 = extrude_point(p1, light_pos, extrude_dist);
        push_tri(out, p0, e0, p1);
        push_tri(out, p1, e0, e1);
    }
}

#[inline]
fn extrude_point(p: Vec3, light_pos: Vec3, dist: f32) -> Vec3 {
    let dir = (p - light_pos).normalize_or(Vec3::Y);
    p + dir * dist
}

#[inline]
fn push_tri(out: &mut Vec<ShadowVolumeVertex>, a: Vec3, b: Vec3, c: Vec3) {
    out.push(ShadowVolumeVertex {
        position: a.to_array(),
    });
    out.push(ShadowVolumeVertex {
        position: b.to_array(),
    });
    out.push(ShadowVolumeVertex {
        position: c.to_array(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::kg::{KgBoundingSphere, KgPart, KgStrip, KgStripGeometry};

    fn single_quad_kg() -> KgFile {
        KgFile {
            header_flags: 0,
            header_type: 0,
            parts: vec![KgPart {
                bone_or_object_id: 0,
                bounding_sphere: KgBoundingSphere::default(),
                world_matrix: Mat4::IDENTITY.to_cols_array_2d(),
                strips: vec![KgStrip {
                    vertex_count: 4,
                    strip_type: 4,
                    bounding_sphere: KgBoundingSphere::default(),
                    geometry: KgStripGeometry::PlanarPolygon {
                        normal: [0.0, 0.0, 1.0],
                        vertices: vec![
                            [-100, -100, 0],
                            [100, -100, 0],
                            [100, 100, 0],
                            [-100, 100, 0],
                        ],
                    },
                }],
            }],
        }
    }

    #[test]
    fn extrude_kg1_produces_closed_volume_with_caps_and_sides() {
        let kg = single_quad_kg();
        let mats = [Mat4::IDENTITY.to_cols_array_2d()];
        let verts = extrude_kg1_stencil_volume(&kg, &mats, Vec3::new(0.0, 0.0, 1000.0), 5000.0);
        // 2 front-cap tris + 2 back-cap tris + 4 silhouette edges * 2 tris = 12 tris = 36 verts
        assert_eq!(verts.len(), 36);
        assert!(verts.iter().all(|v| v.position[2] <= 1.0));
    }

    #[test]
    fn flatten_kg1_drop_shadow_projects_onto_floor_plane() {
        let kg = single_quad_kg();
        let mats = [Mat4::IDENTITY.to_cols_array_2d()];
        let verts = flatten_kg1_drop_shadow(&kg, &mats, 500.0, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(verts.len(), 6);
        for v in &verts {
            assert!((v.position[1] - (500.0 - DROP_SHADOW_FLOOR_BIAS_MM)).abs() < 1e-3);
        }
    }
}
