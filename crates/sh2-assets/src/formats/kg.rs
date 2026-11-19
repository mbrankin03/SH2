//! PS2 / PC Shadow Model binary parsers for `.kg1` character shadow meshes (`data/chr/*/*.kg1`)
//! and `.kg2` room shadow meshes (`data/bg/*/*.kg2`).
//!
//! ## Ground-Truth Binary Layout
//! Verified against `SLUS_202.28` (`sh2shd_init_char_man` @ `0x0018c060`,
//! `sh2shd_make_reftag_pool_char` @ `0x0018c130`, `sh2shd_init_outdoor_man2` @ `0x001a0920`,
//! `loadBgKG2_Replace` @ `0x00272630`) and `sh2pc.exe` (`0x004449f0` / `0x00444b20`):
//!
//! - **File Header (16 bytes, `0x00..0x10`)**:
//!   - `+0x00` (`u16`): Header ID / flags
//!   - `+0x02` (`u16`): Reserved / type
//!   - `+0x04` (`u32`): Number of shadow parts (`part_count`)
//!   - `+0x08..0x10`: Padding
//! - **Part Header (96 bytes = `0x60`, repeated `part_count` times)**:
//!   - `+0x04` (`u16`): Bone index (`.kg1`) or room object ID (`.kg2`)
//!   - `+0x06` (`u16`): Number of strips (`strip_count`)
//!   - `+0x18..0x20` (`[i16; 4]`): Local bounding sphere `(cx, cy, cz, radius)` in PS2 mm
//!   - `+0x20..0x60` (`[[f32; 4]; 4]`): Column-major 4x4 world transform matrix (identity/zeroed in `.kg1`,
//!     populated with room world transform in `.kg2` and copied via `sceVu0CopyMatrix` @ `0x001a0a70`)
//! - **Strip Header (16 bytes, 1 qword)** followed by `qword_count - 1` payload qwords:
//!   - `+0x00` (`u16`): `vertex_count`
//!   - `+0x02` (`u16`): `strip_type` (`5, 6` in `.kg1`; `1..=10` in `.kg2`)
//!     - Types `5, 6, 8, 9`: Triangle strips with interleaved per-face `Q15` normals (`q1 = (v0, v1)`,
//!       `q_{k+2} = (face_normal_k, v_{k+2})` where normals have `w == 0`, magnitude `32768`, and
//!       vertices have `w == 1` in PS2 mm).
//!     - Types `1, 2, 3, 4, 7, 10`: Planar shadow polygons (`q1 = (face_normal, v0)`, followed by
//!       packed `i16x4` vertices `v1..v_{N-1}`). Note: in `sh2shd_init_outdoor_man2` (`0x001a0a24`),
//!       strip types `3, 4, 10` terminate the part's strip list immediately after one strip.
//!   - `+0x04` (`u16`): Packed `i16x4` count
//!   - `+0x06` (`u16`): Total `qword_count` (16-byte units, inclusive of this 16-byte strip header)
//!   - `+0x08..0x10` (`[i16; 4]`): Strip local bounding sphere `(cx, cy, cz, radius)` in PS2 mm
//!     (read by `sh2pc.exe` `0x00444bc9..0x00444c14` for spotlight cone culling).

use crate::error::AssetError;
use crate::formats::binary::{read_f32, read_i16, read_u16, read_u32};

/// Scale factor converting PS2 Q15 signed-16-bit shadow face normals (`±32768`) to unit floats.
pub const KG_NORMAL_Q15_SCALE: f32 = 1.0 / 32768.0;

/// Bounding sphere in local PS2 millimeter coordinates `(cx, cy, cz, radius)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KgBoundingSphere {
    /// Center X in PS2 mm.
    pub cx: i16,
    /// Center Y in PS2 mm (Y-down).
    pub cy: i16,
    /// Center Z in PS2 mm.
    pub cz: i16,
    /// Bounding radius in PS2 mm.
    pub radius: i16,
}

impl KgBoundingSphere {
    /// Converts the bounding sphere to `(center_mm, radius_mm)` in `f32`.
    #[inline]
    #[must_use]
    pub fn to_f32(self) -> ([f32; 3], f32) {
        (
            [f32::from(self.cx), f32::from(self.cy), f32::from(self.cz)],
            f32::from(self.radius).max(0.0),
        )
    }
}

/// Decoded shadow triangle in local part space (PS2 mm) with unit face normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KgTriangle {
    /// Triangle vertices `[v0, v1, v2]` in local part space (PS2 mm).
    pub vertices: [[f32; 3]; 3],
    /// Outward-pointing unit face normal in local part space.
    pub normal: [f32; 3],
}

/// Decoded geometry payload of a single `.kg1` / `.kg2` shadow strip.
#[derive(Debug, Clone, PartialEq)]
pub enum KgStripGeometry {
    /// Triangle strip with per-face Q15 normals (`strip_type` in `{5, 6, 8, 9}`).
    TriangleStrip {
        /// Strip vertices in PS2 mm (`v0, v1, v2, ...`).
        vertices: Vec<[i16; 3]>,
        /// Normalized per-face normals (`len == vertices.len().saturating_sub(2)`).
        face_normals: Vec<[f32; 3]>,
    },
    /// Planar shadow polygon (`strip_type` in `{1, 2, 3, 4, 7, 10}`).
    PlanarPolygon {
        /// Shared normalized face normal of the polygon.
        normal: [f32; 3],
        /// Ordered polygon vertices in PS2 mm.
        vertices: Vec<[i16; 3]>,
    },
}

/// Single shadow strip inside a [`KgPart`].
#[derive(Debug, Clone, PartialEq)]
pub struct KgStrip {
    /// Declared vertex count from `+0x00` of the strip header.
    pub vertex_count: u16,
    /// Team Silent strip type (`5, 6` in `.kg1`; `1..=10` in `.kg2`).
    pub strip_type: u16,
    /// Local strip bounding sphere for spotlight cone culling (`sh2pc.exe` `0x00444bc9`).
    pub bounding_sphere: KgBoundingSphere,
    /// Decoded vertices and face normals.
    pub geometry: KgStripGeometry,
}

impl KgStrip {
    /// Expands this strip into individual [`KgTriangle`]s with consistent winding and unit normals.
    #[must_use]
    pub fn to_triangles(&self) -> Vec<KgTriangle> {
        match &self.geometry {
            KgStripGeometry::TriangleStrip {
                vertices,
                face_normals,
            } => {
                if vertices.len() < 3 {
                    return Vec::new();
                }
                let tri_count = vertices.len() - 2;
                let mut tris = Vec::with_capacity(tri_count);
                for i in 0..tri_count {
                    let v0 = i16_to_f32_vec3(vertices[i]);
                    let v1 = i16_to_f32_vec3(vertices[i + 1]);
                    let v2 = i16_to_f32_vec3(vertices[i + 2]);
                    let (a, b, c) = if i % 2 == 0 {
                        (v0, v1, v2)
                    } else {
                        (v1, v0, v2)
                    };
                    let normal = face_normals
                        .get(i)
                        .copied()
                        .unwrap_or_else(|| compute_triangle_normal(a, b, c));
                    tris.push(KgTriangle {
                        vertices: [a, b, c],
                        normal,
                    });
                }
                tris
            }
            KgStripGeometry::PlanarPolygon { normal, vertices } => {
                if vertices.len() < 3 {
                    return Vec::new();
                }
                let v0 = i16_to_f32_vec3(vertices[0]);
                let mut tris = Vec::with_capacity(vertices.len() - 2);
                for i in 1..(vertices.len() - 1) {
                    let v1 = i16_to_f32_vec3(vertices[i]);
                    let v2 = i16_to_f32_vec3(vertices[i + 1]);
                    let n = if normal_len_sq(*normal) > 1e-6 {
                        *normal
                    } else {
                        compute_triangle_normal(v0, v1, v2)
                    };
                    tris.push(KgTriangle {
                        vertices: [v0, v1, v2],
                        normal: n,
                    });
                }
                tris
            }
        }
    }
}

/// One articulated bone part (`.kg1`) or static room shadow object (`.kg2`).
#[derive(Debug, Clone, PartialEq)]
pub struct KgPart {
    /// Character skeleton bone index (`.kg1`) or room object ID (`.kg2`).
    pub bone_or_object_id: u16,
    /// Part-level bounding sphere in local PS2 mm (`+0x18..0x20`).
    pub bounding_sphere: KgBoundingSphere,
    /// Column-major 4x4 transform matrix (`+0x20..0x60`; populated for `.kg2` room objects).
    pub world_matrix: [[f32; 4]; 4],
    /// Shadow strips belonging to this part.
    pub strips: Vec<KgStrip>,
}

impl KgPart {
    /// Returns `true` when the embedded 4x4 `world_matrix` has a non-zero diagonal/translation
    /// (always true for `.kg2` world-placed shadow objects; false for `.kg1` bone-driven parts).
    #[must_use]
    pub fn has_embedded_matrix(&self) -> bool {
        self.world_matrix[3][3].abs() > 0.5
    }
}

/// Parsed `.kg1` (character) or `.kg2` (room) shadow model file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KgFile {
    /// Header flags / identifier from `+0x00`.
    pub header_flags: u16,
    /// Header secondary field from `+0x02`.
    pub header_type: u16,
    /// Shadow parts contained in this file.
    pub parts: Vec<KgPart>,
}

impl KgFile {
    /// Total number of strips across all parts.
    #[must_use]
    pub fn total_strip_count(&self) -> usize {
        self.parts.iter().map(|p| p.strips.len()).sum()
    }

    /// Total number of expanded shadow triangles across all parts.
    #[must_use]
    pub fn total_triangle_count(&self) -> usize {
        self.parts
            .iter()
            .flat_map(|p| p.strips.iter())
            .map(|s| s.to_triangles().len())
            .sum()
    }
}

/// Parses a Silent Hill 2 `.kg1` (character shadow model) or `.kg2` (room shadow model) buffer.
///
/// Empty 0-byte `.kg1` placeholder files (e.g. `boat.kg1`, `bos.kg1`) are valid in retail data
/// and return an empty [`KgFile`].
pub fn parse_kg(data: &[u8]) -> Result<KgFile, AssetError> {
    if data.is_empty() {
        return Ok(KgFile::default());
    }
    if data.len() < 16 {
        return Err(AssetError::BufferUnderflow {
            required: 16,
            available: data.len(),
        });
    }

    let header_flags = read_u16(data, 0)?;
    let header_type = read_u16(data, 2)?;
    let part_count = read_u32(data, 4)? as usize;

    // Guard against corrupted headers declaring absurd part counts before allocating
    let max_possible_parts = (data.len() - 16) / 0x60;
    if part_count > max_possible_parts {
        return Err(AssetError::InvalidData(format!(
            "kg part_count {part_count} exceeds maximum possible parts {max_possible_parts} for {} bytes",
            data.len()
        )));
    }

    let mut parts = Vec::with_capacity(part_count);
    let mut ofs = 16usize;

    for _ in 0..part_count {
        if ofs + 0x60 > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: ofs + 0x60,
                available: data.len(),
            });
        }

        let bone_or_object_id = read_u16(data, ofs + 0x04)?;
        let strip_count = read_u16(data, ofs + 0x06)? as usize;
        let bounding_sphere = read_bounding_sphere(data, ofs + 0x18)?;

        let mut world_matrix = [[0.0f32; 4]; 4];
        for (col, mat_col) in world_matrix.iter_mut().enumerate() {
            for (row, cell) in mat_col.iter_mut().enumerate() {
                *cell = read_f32(data, ofs + 0x20 + (col * 4 + row) * 4)?;
            }
        }
        ofs += 0x60;

        let mut strips = Vec::with_capacity(strip_count);
        for _ in 0..strip_count {
            if ofs + 16 > data.len() {
                return Err(AssetError::BufferUnderflow {
                    required: ofs + 16,
                    available: data.len(),
                });
            }

            let vertex_count = read_u16(data, ofs)?;
            let strip_type = read_u16(data, ofs + 2)?;
            let qword_count = read_u16(data, ofs + 6)? as usize;
            let strip_sphere = read_bounding_sphere(data, ofs + 8)?;

            if qword_count == 0 {
                return Err(AssetError::InvalidData(format!(
                    "kg strip at offset 0x{ofs:x} has zero qword_count"
                )));
            }
            let strip_bytes = qword_count.saturating_mul(16);
            if ofs + strip_bytes > data.len() {
                return Err(AssetError::BufferUnderflow {
                    required: ofs + strip_bytes,
                    available: data.len(),
                });
            }

            let geometry = parse_strip_payload(
                &data[(ofs + 16)..(ofs + strip_bytes)],
                strip_type,
                vertex_count as usize,
            )?;

            strips.push(KgStrip {
                vertex_count,
                strip_type,
                bounding_sphere: strip_sphere,
                geometry,
            });

            ofs += strip_bytes;

            // `sh2shd_init_outdoor_man2` (`0x001a0a24..0x001a0a84`):
            // Strip types 3, 4, and 10 terminate the part's strip list immediately after one strip.
            if matches!(strip_type, 3 | 4 | 10) {
                break;
            }
        }

        parts.push(KgPart {
            bone_or_object_id,
            bounding_sphere,
            world_matrix,
            strips,
        });
    }

    Ok(KgFile {
        header_flags,
        header_type,
        parts,
    })
}

/// Alias for [`parse_kg`] specifically for character `.kg1` shadow files.
#[inline]
pub fn parse_kg1(data: &[u8]) -> Result<KgFile, AssetError> {
    parse_kg(data)
}

/// Alias for [`parse_kg`] specifically for room `.kg2` shadow files.
#[inline]
pub fn parse_kg2(data: &[u8]) -> Result<KgFile, AssetError> {
    parse_kg(data)
}

fn read_bounding_sphere(data: &[u8], ofs: usize) -> Result<KgBoundingSphere, AssetError> {
    Ok(KgBoundingSphere {
        cx: read_i16(data, ofs)?,
        cy: read_i16(data, ofs + 2)?,
        cz: read_i16(data, ofs + 4)?,
        radius: read_i16(data, ofs + 6)?,
    })
}

fn parse_strip_payload(
    payload: &[u8],
    strip_type: u16,
    declared_verts: usize,
) -> Result<KgStripGeometry, AssetError> {
    let entry_count = payload.len() / 8;
    let is_triangle_strip = matches!(strip_type, 5 | 6 | 8 | 9);

    if is_triangle_strip {
        // q1 = (v0, v1), q2.. = (normal_k, v_{k+2})
        let mut vertices = Vec::with_capacity(declared_verts);
        let mut face_normals = Vec::with_capacity(declared_verts.saturating_sub(2));

        if entry_count >= 2 {
            vertices.push(read_i16x3(payload, 0)?);
            vertices.push(read_i16x3(payload, 8)?);
            let mut e = 2usize;
            while e + 1 < entry_count && vertices.len() < declared_verts {
                let raw_n = read_i16x3(payload, e * 8)?;
                face_normals.push(decode_q15_normal(raw_n));
                vertices.push(read_i16x3(payload, (e + 1) * 8)?);
                e += 2;
            }
        }

        Ok(KgStripGeometry::TriangleStrip {
            vertices,
            face_normals,
        })
    } else {
        // Planar polygon: entry 0 = face normal (w == 0), entries 1.. = vertices (w == 1)
        let normal = if entry_count >= 1 {
            decode_q15_normal(read_i16x3(payload, 0)?)
        } else {
            [0.0, -1.0, 0.0]
        };
        let mut vertices = Vec::with_capacity(declared_verts);
        for e in 1..entry_count {
            if vertices.len() >= declared_verts {
                break;
            }
            vertices.push(read_i16x3(payload, e * 8)?);
        }
        Ok(KgStripGeometry::PlanarPolygon { normal, vertices })
    }
}

#[inline]
fn read_i16x3(data: &[u8], ofs: usize) -> Result<[i16; 3], AssetError> {
    Ok([
        read_i16(data, ofs)?,
        read_i16(data, ofs + 2)?,
        read_i16(data, ofs + 4)?,
    ])
}

#[inline]
fn decode_q15_normal(raw: [i16; 3]) -> [f32; 3] {
    let x = f32::from(raw[0]) * KG_NORMAL_Q15_SCALE;
    let y = f32::from(raw[1]) * KG_NORMAL_Q15_SCALE;
    let z = f32::from(raw[2]) * KG_NORMAL_Q15_SCALE;
    let len = (x * x + y * y + z * z).sqrt();
    if len > 1e-5 {
        [x / len, y / len, z / len]
    } else {
        [0.0, 0.0, 0.0]
    }
}

#[inline]
fn i16_to_f32_vec3(v: [i16; 3]) -> [f32; 3] {
    [f32::from(v[0]), f32::from(v[1]), f32::from(v[2])]
}

#[inline]
fn normal_len_sq(n: [f32; 3]) -> f32 {
    n[0] * n[0] + n[1] * n[1] + n[2] * n[2]
}

fn compute_triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let nx = e1[1] * e2[2] - e1[2] * e2[1];
    let ny = e1[2] * e2[0] - e1[0] * e2[2];
    let nz = e1[0] * e2[1] - e1[1] * e2[0];
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 1e-6 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, -1.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_kg1_placeholder_parses_cleanly() {
        let kg = parse_kg1(&[]).expect("empty .kg1 must parse cleanly");
        assert!(kg.parts.is_empty());
        assert_eq!(kg.total_strip_count(), 0);
        assert_eq!(kg.total_triangle_count(), 0);
    }

    #[test]
    fn truncated_kg_returns_buffer_underflow() {
        let err = parse_kg(&[0u8; 8]).unwrap_err();
        assert!(matches!(err, AssetError::BufferUnderflow { .. }));
    }

    #[test]
    fn synthetic_kg1_triangle_strip_roundtrips() {
        // 16B header + 96B part + 16B strip header + 2 payload qwords (3 vertices, 1 normal)
        let mut buf = vec![0u8; 16 + 0x60 + 48];
        buf[4..8].copy_from_slice(&1u32.to_le_bytes()); // part_count = 1
        let p = 16;
        buf[p + 4..p + 6].copy_from_slice(&3u16.to_le_bytes()); // bone_id = 3
        buf[p + 6..p + 8].copy_from_slice(&1u16.to_le_bytes()); // strip_count = 1
        let s = p + 0x60;
        buf[s..s + 2].copy_from_slice(&3u16.to_le_bytes()); // vertex_count = 3
        buf[s + 2..s + 4].copy_from_slice(&6u16.to_le_bytes()); // strip_type = 6
        buf[s + 4..s + 6].copy_from_slice(&5u16.to_le_bytes());
        buf[s + 6..s + 8].copy_from_slice(&3u16.to_le_bytes()); // qword_count = 3
                                                                // q1: v0 = (0,0,0,1), v1 = (100,0,0,1)
        let q1 = s + 16;
        buf[q1 + 6..q1 + 8].copy_from_slice(&1i16.to_le_bytes());
        buf[q1 + 8..q1 + 10].copy_from_slice(&100i16.to_le_bytes());
        buf[q1 + 14..q1 + 16].copy_from_slice(&1i16.to_le_bytes());
        // q2: n0 = (0, 32767, 0, 0), v2 = (0, 0, 100, 1)
        let q2 = s + 32;
        buf[q2 + 2..q2 + 4].copy_from_slice(&32767i16.to_le_bytes());
        buf[q2 + 12..q2 + 14].copy_from_slice(&100i16.to_le_bytes());
        buf[q2 + 14..q2 + 16].copy_from_slice(&1i16.to_le_bytes());

        let kg = parse_kg1(&buf).expect("synthetic kg1 must parse");
        assert_eq!(kg.parts.len(), 1);
        assert_eq!(kg.parts[0].bone_or_object_id, 3);
        assert!(!kg.parts[0].has_embedded_matrix());
        let tris = kg.parts[0].strips[0].to_triangles();
        assert_eq!(tris.len(), 1);
        assert!((tris[0].normal[1] - 1.0).abs() < 1e-3);
    }
}
