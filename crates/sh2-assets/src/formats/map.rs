//! Safe binary parser for Silent Hill 2 Map format (`.map`).
//!
//! # Format Architecture & Memory Layout
//! Silent Hill 2 `.map` files store level geometry, mesh groupings, materials, and embedded textures.
//!
//! ### Binary File Header (16 bytes)
//! | Offset | Type | Description |
//! |--------|------|-------------|
//! | `0x00` | `u32` | Magic number (`0x20010510` in little-endian) |
//! | `0x04` | `u32` | Total file size in bytes |
//! | `0x08` | `u32` | Number of sub-files (`sub_count`, typically 2: geometry and textures) |
//! | `0x0C` | `u32` | Reserved / padding (always 0) |
//!
//! ### Sub-Files (Type 1: Geometry & Materials, Type 2: Embedded Textures)
//! - **Sub-file 1 (Geometry & Materials)**:
//!   - Header: `u32 magic`, `u32 geo_count`, `u32 geo_size`, `u32 material_count`.
//!   - Geometry records: `geo_id` (variant + part group), byte size, and offsets to:
//!     - Opaque mesh group
//!     - Transparent mesh group
//!     - Decals mesh group
//!   - Materials table: array of 16-byte structs containing blend mode, texture index, color tint,
//!     overlay factors, and specular power.
//! - **Mesh Groups & Batches**:
//!   - Meshes index a shared vertex buffer divided into sections (e.g. stride 0x14 for pos+uv,
//!     0x18 for pos+color+uv, 0x20 for pos+normal+uv, 0x24 for pos+normal+color+uv).
//!   - Part groups specify material IDs, section references, and primitive strips/fans/lists.
//!   - `parse_map` de-duplicates and unrolls triangle strips into indexed [`MapBatch`] draw calls.
//! - **Sub-file 2 (Textures)**:
//!   - Sequential embedded DXT compressed texture blocks tagged by texture IDs.

use super::binary::{read_f32, read_u16, read_u32};
use crate::error::AssetError;
use crate::formats::dxt::decode_dxt;
use std::collections::HashMap;

/// Magic identifier present at byte 0 of all valid PC .map files (`0x20010510`).
pub const MAP_MAGIC: u32 = 0x20010510;

/// Byte representation of `MAP_MAGIC` in little-endian order.
pub const MAP_MAGIC_BYTES: [u8; 4] = [0x10, 0x05, 0x01, 0x20];

/// Surface material definition applied to geometry batches.
///
/// Corresponds to the 16-byte material records at the end of geometry subfiles.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMaterial {
    /// Rendering mode / shader flag (e.g., standard shading, vertex color modulation, specular).
    pub mode: u16,
    /// Index into the map's texture table.
    pub texture_id: u16,
    /// RGBA tint / diffuse color stored in packed 32-bit format.
    pub color: u32,
    /// Overlay color / blend factor.
    pub overlay: u32,
    /// Specular reflection power exponent.
    pub specularity: f32,
}

/// An embedded texture stored inside a `.map` sub-file.
#[derive(Debug, Clone)]
pub struct MapTexture {
    /// Unique texture identifier referenced by material records.
    pub id: u32,
    /// Texture width in pixels.
    pub width: usize,
    /// Texture height in pixels.
    pub height: usize,
    /// Compression kind: 1 = DXT1 (BC1), 3 = DXT3 (BC2), 5 = DXT5 (BC3).
    pub kind: u8,
    /// Compressed texture payload data.
    pub data: Vec<u8>,
}

impl MapTexture {
    /// Decodes the compressed texture payload into an uncompressed RGBA8 byte buffer.
    pub fn to_rgba(&self) -> Result<Vec<u8>, AssetError> {
        decode_dxt(&self.data, self.width, self.height, self.kind)
    }
}

/// Category of geometry determining render queue, depth writing, and sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchGroup {
    /// Standard opaque geometry drawn with full depth testing and writing.
    Opaque,
    /// Alpha-blended geometry sorted back-to-front.
    Transparent,
    /// Coplanar decal surfaces (blood splatters, posters, floor markings).
    Decal,
}

/// A parsed section of vertex attributes before batching and draw call assembly.
#[derive(Debug, Clone, Default)]
pub struct VertexSection {
    /// Model-space vertex positions (X, Y, Z).
    pub positions: Vec<[f32; 3]>,
    /// Texture mapping coordinates (U, V).
    pub uvs: Vec<[f32; 2]>,
    /// Per-vertex RGBA colors (BGRA byte order remapped to RGBA).
    pub colors: Vec<[u8; 4]>,
    /// Surface normal vectors (X, Y, Z).
    pub normals: Vec<[f32; 3]>,
}

/// A pre-assembled GPU draw call containing vertices and triangle list indices.
///
/// All triangle strips, fans, and vertex sections have been converted into indexed
/// triangle lists ready for direct binding into a GPU pipeline (`wgpu::RenderPass`).
#[derive(Debug, Clone)]
pub struct MapBatch {
    /// Draw queue type (Opaque, Transparent, or Decal).
    pub group: BatchGroup,
    /// Index into [`MapFile::materials`].
    pub material_index: usize,
    /// Sub-geometry identifier (part group in bits 8..15, variant in bits 0..7).
    pub geo_id: u16,
    /// Vertex positions (X, Y, Z).
    pub positions: Vec<[f32; 3]>,
    /// Texture coordinates (U, V).
    pub uvs: Vec<[f32; 2]>,
    /// Vertex colors in RGBA8 order.
    pub colors: Vec<[u8; 4]>,
    /// Vertex surface normals.
    pub normals: Vec<[f32; 3]>,
    /// Triangle list index buffer indexing into `positions`, `uvs`, `colors`, and `normals`.
    pub indices: Vec<u32>,
}

/// Interleaved vertex format matching modern GPU vertex layout (`MapVertex`).
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MapVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl MapBatch {
    /// Converts parallel vertex attributes (`positions`, `normals`, `uvs`, `colors`) into interleaved [`MapVertex`] structs for direct GPU consumption.
    pub fn interleaved_vertices(&self) -> Vec<MapVertex> {
        let count = self.positions.len();
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let pos = self.positions[i];
            let norm = self.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            let uv = self.uvs.get(i).copied().unwrap_or([0.0, 0.0]);
            let c = self.colors.get(i).copied().unwrap_or([255, 255, 255, 255]);
            let col = [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                c[3] as f32 / 255.0,
            ];
            out.push(MapVertex {
                position: pos,
                normal: norm,
                uv,
                color: col,
            });
        }
        out
    }
}

/// Ingested representation of a complete `.map` file.
#[derive(Debug, Clone, Default)]
pub struct MapFile {
    /// Identifying file or stage name (e.g., "rr01" or "ap01").
    pub name: String,
    /// Material definitions indexed by batches.
    pub materials: Vec<MapMaterial>,
    /// Embedded textures keyed by their unique texture ID.
    pub textures: HashMap<u32, MapTexture>,
    /// Render batches prepared for GPU rasterization.
    pub batches: Vec<MapBatch>,
}

impl MapFile {
    /// Convenience alias to parse a map file without specifying a name.
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        parse_map(bytes, "map")
    }
}

/// Parses a Silent Hill 2 level geometry map file (`.map`) from raw bytes.
///
/// # Errors
/// Returns [`AssetError::InvalidMagic`] if the data does not begin with `0x20010510`,
/// or [`AssetError::BufferUnderflow`] if byte slices exceed the buffer bounds.
pub fn parse_map(data: &[u8], name: &str) -> Result<MapFile, AssetError> {
    if data.len() < 16 {
        return Err(AssetError::BufferUnderflow {
            required: 16,
            available: data.len(),
        });
    }

    let magic = read_u32(data, 0)?;
    if magic != MAP_MAGIC {
        return Err(AssetError::InvalidMagic {
            expected: &MAP_MAGIC_BYTES,
            found: data[0..4].to_vec(),
        });
    }

    let sub_count = read_u32(data, 8)? as usize;
    let mut map = MapFile {
        name: name.to_string(),
        materials: Vec::new(),
        textures: HashMap::new(),
        batches: Vec::new(),
    };

    let mut p = 16usize;
    for _ in 0..sub_count {
        if p + 16 > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: p + 16,
                available: data.len(),
            });
        }

        let sub_type = read_u32(data, p)?;
        let sub_len = read_u32(data, p + 4)? as usize;
        let body = p + 16;

        if body + sub_len > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: body + sub_len,
                available: data.len(),
            });
        }

        if sub_type == 1 {
            // Geometry and Materials sub-file
            parse_geometry_subfile(data, body, &mut map)?;
        } else if sub_type == 2 {
            // Textures sub-file
            parse_textures_subfile(data, body, &mut map)?;
        }

        p = body + sub_len;
    }

    Ok(map)
}

/// Parses sub-file type 1: geometry headers, mesh groups, vertex sections, and material records.
fn parse_geometry_subfile(data: &[u8], body: usize, map: &mut MapFile) -> Result<(), AssetError> {
    if body + 16 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: body + 16,
            available: data.len(),
        });
    }

    let geo_count = read_u32(data, body + 4)? as usize;
    let geo_size = read_u32(data, body + 8)? as usize;
    let mat_count = read_u32(data, body + 12)? as usize;

    let mut g = body + 16;
    for _ in 0..geo_count {
        if g + 20 > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: g + 20,
                available: data.len(),
            });
        }

        let geo_id = read_u32(data, g)? as u16;
        let size = read_u32(data, g + 4)? as usize;
        let o_ofs = read_u32(data, g + 8)? as usize;
        let t_ofs = read_u32(data, g + 12)? as usize;
        let d_ofs = read_u32(data, g + 16)? as usize;

        let first_batch_idx = map.batches.len();

        if o_ofs != 0 {
            parse_mesh_group(data, g + o_ofs, BatchGroup::Opaque, map)?;
        }
        if t_ofs != 0 {
            parse_mesh_group(data, g + t_ofs, BatchGroup::Transparent, map)?;
        }
        if d_ofs != 0 {
            parse_decals(data, g + d_ofs, map)?;
        }

        // Tag all batches generated from this geometry node with its geo_id
        for batch in &mut map.batches[first_batch_idx..] {
            batch.geo_id = geo_id;
        }

        g += size;
    }

    let mp = body + geo_size;
    if mp + mat_count * 16 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: mp + mat_count * 16,
            available: data.len(),
        });
    }

    for i in 0..mat_count {
        let ofs = mp + 16 * i;
        let mode = read_u16(data, ofs)?;
        let texture_id = read_u16(data, ofs + 2)?;
        let color = read_u32(data, ofs + 4)?;
        let overlay = read_u32(data, ofs + 8)?;
        let specularity = read_f32(data, ofs + 12)?;

        map.materials.push(MapMaterial {
            mode,
            texture_id,
            color,
            overlay,
            specularity,
        });
    }

    Ok(())
}

/// Parses mesh chunks (opaque or transparent) and converts triangle strip primitives to triangle lists.
fn parse_mesh_group(
    data: &[u8],
    base: usize,
    group: BatchGroup,
    map: &mut MapFile,
) -> Result<(), AssetError> {
    if base + 4 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: base + 4,
            available: data.len(),
        });
    }

    let count = read_u32(data, base)? as usize;
    for i in 0..count {
        let ofs = read_u32(data, base + 4 + i * 4)? as usize;
        let m = base + ofs;
        if m + 52 > data.len() {
            continue;
        }

        let vtx_ofs = read_u32(data, m + 32)? as usize;
        let idx_ofs = read_u32(data, m + 36)? as usize;
        let idx_len = read_u32(data, m + 40)? as usize;
        let pg_count = read_u32(data, m + 48)? as usize;

        let num_indices = idx_len / 2;
        let idx_start = m + idx_ofs;
        if idx_start + idx_len > data.len() {
            continue;
        }

        let mut indices = Vec::with_capacity(num_indices);
        for k in 0..num_indices {
            indices.push(read_u16(data, idx_start + k * 2)? as u32);
        }

        let sections = parse_vertex_sections(data, m + vtx_ofs)?;

        let mut p = m + 52;
        let mut cursor = 0usize;

        let mut acc: HashMap<(usize, usize), Vec<u32>> = HashMap::new();

        for _ in 0..pg_count {
            if p + 12 > data.len() {
                break;
            }
            let material = read_u32(data, p)? as usize;
            let section = read_u32(data, p + 4)? as usize;
            let part_count = read_u32(data, p + 8)? as usize;
            p += 12;

            for _ in 0..part_count {
                if p + 8 > data.len() {
                    break;
                }
                let strip_len = read_u16(data, p)? as usize;
                let prim = data[p + 3];
                p += 8;

                let n = if prim == 3 { strip_len * 3 } else { strip_len };

                if cursor + n <= indices.len() {
                    let sub_indices = &indices[cursor..cursor + n];
                    let tris = strip_to_list(sub_indices, prim);
                    acc.entry((material, section))
                        .or_default()
                        .extend_from_slice(&tris);
                }
                cursor += n;
            }
        }

        for ((material, section), tris) in acc {
            if !tris.is_empty() && section < sections.len() {
                collect_batch(map, group, &sections[section], material, &tris);
            }
        }
    }

    Ok(())
}

/// Parses decal geometry sections (billboards, splatters, and signs).
fn parse_decals(data: &[u8], base: usize, map: &mut MapFile) -> Result<(), AssetError> {
    if base + 4 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: base + 4,
            available: data.len(),
        });
    }

    let count = read_u32(data, base)? as usize;
    for i in 0..count {
        let ofs = read_u32(data, base + 4 + i * 4)? as usize;
        let g = base + ofs;
        if g + 48 > data.len() {
            continue;
        }

        let vtx_ofs = read_u32(data, g + 32)? as usize;
        let idx_ofs = read_u32(data, g + 36)? as usize;
        let idx_len = read_u32(data, g + 40)? as usize;
        let decal_count = read_u32(data, g + 44)? as usize;

        let num_indices = idx_len / 2;
        let idx_start = g + idx_ofs;
        if idx_start + idx_len > data.len() {
            continue;
        }

        let mut indices = Vec::with_capacity(num_indices);
        for k in 0..num_indices {
            indices.push(read_u16(data, idx_start + k * 2)? as u32);
        }

        let sections = parse_vertex_sections(data, g + vtx_ofs)?;

        let mut p = g + 48;
        let mut cursor = 0usize;

        for _ in 0..decal_count {
            if p + 16 > data.len() {
                break;
            }
            let material = read_u32(data, p)? as usize;
            let section = read_u32(data, p + 4)? as usize;
            let strip_len = read_u32(data, p + 8)? as usize;
            let strip_count = read_u32(data, p + 12)? as usize;
            p += 16;

            let mut tris = Vec::new();
            for _ in 0..strip_count {
                if cursor + strip_len <= indices.len() {
                    let sub_indices = &indices[cursor..cursor + strip_len];
                    tris.extend_from_slice(&strip_to_list(sub_indices, 1));
                    cursor += strip_len;
                }
            }

            if !tris.is_empty() && section < sections.len() {
                collect_batch(map, BatchGroup::Decal, &sections[section], material, &tris);
            }
        }
    }

    Ok(())
}

/// Parses vertex sections by inspecting stride indicators (0x14, 0x18, 0x20, 0x24).
fn parse_vertex_sections(data: &[u8], base: usize) -> Result<Vec<VertexSection>, AssetError> {
    if base + 8 > data.len() {
        return Ok(Vec::new());
    }

    let count = read_u32(data, base + 4)? as usize;
    let mut info = Vec::with_capacity(count);
    for j in 0..count {
        let entry_ofs = base + 8 + 12 * j;
        if entry_ofs + 12 > data.len() {
            break;
        }
        let ofs = read_u32(data, entry_ofs)? as usize;
        let stride = read_u32(data, entry_ofs + 4)? as usize;
        let length = read_u32(data, entry_ofs + 8)? as usize;
        info.push((ofs, stride, length));
    }

    let start = base + 8 + 12 * count;
    let mut sections = Vec::with_capacity(count);

    for (ofs, stride, length) in info {
        let sec_start = start + ofs;
        if stride == 0 || sec_start + length > data.len() {
            sections.push(VertexSection::default());
            continue;
        }

        let n = length / stride;
        let mut pos = Vec::with_capacity(n);
        let mut uv = Vec::with_capacity(n);
        let mut col = Vec::with_capacity(n);
        let mut nrm = Vec::with_capacity(n);

        for i in 0..n {
            let vo = sec_start + i * stride;
            let p_x = read_f32(data, vo)?;
            let p_y = read_f32(data, vo + 4)?;
            let p_z = read_f32(data, vo + 8)?;
            pos.push([p_x, p_y, p_z]);

            let mut normal = [0.0f32; 3];
            let mut tex_coord = [0.0f32; 2];
            let mut color = [255u8; 4];

            if stride == 0x14 {
                // Pos (0x0..0xC), UV (0xC..0x14)
                tex_coord = [read_f32(data, vo + 12)?, read_f32(data, vo + 16)?];
            } else if stride == 0x18 {
                // Pos (0x0..0xC), BGRA Color (0xC..0x10), UV (0x10..0x18)
                color = [data[vo + 14], data[vo + 13], data[vo + 12], data[vo + 15]];
                tex_coord = [read_f32(data, vo + 16)?, read_f32(data, vo + 20)?];
            } else if stride == 0x20 {
                // Pos (0x0..0xC), Normal (0xC..0x18), UV (0x18..0x20)
                normal = [
                    read_f32(data, vo + 12)?,
                    read_f32(data, vo + 16)?,
                    read_f32(data, vo + 20)?,
                ];
                tex_coord = [read_f32(data, vo + 24)?, read_f32(data, vo + 28)?];
            } else if stride >= 0x24 {
                // Pos (0x0..0xC), Normal (0xC..0x18), BGRA Color (0x18..0x1C), UV (0x1C..0x24)
                normal = [
                    read_f32(data, vo + 12)?,
                    read_f32(data, vo + 16)?,
                    read_f32(data, vo + 20)?,
                ];
                color = [data[vo + 26], data[vo + 25], data[vo + 24], data[vo + 27]];
                tex_coord = [read_f32(data, vo + 28)?, read_f32(data, vo + 32)?];
            }

            nrm.push(normal);
            uv.push(tex_coord);
            col.push(color);
        }

        sections.push(VertexSection {
            positions: pos,
            uvs: uv,
            colors: col,
            normals: nrm,
        });
    }

    Ok(sections)
}

/// Unrolls triangle strip, triangle fan, or raw triangle lists into standard indexed triangle lists.
///
/// - `prim == 1`: Triangle Strip (alternating winding order handled to avoid flipped normals).
/// - `prim == 2`: Triangle Fan (anchored at first vertex index).
/// - `prim == 3`: Triangle List (direct triplets).
fn strip_to_list(idx: &[u32], prim: u8) -> Vec<u32> {
    if prim == 3 {
        let len = (idx.len() / 3) * 3;
        return idx[..len].to_vec();
    }

    let mut out = Vec::new();
    if prim == 1 {
        // Triangle strip
        for i in 0..idx.len().saturating_sub(2) {
            let a = idx[i];
            let b = idx[i + 1];
            let c = idx[i + 2];
            // Discard degenerate triangle separators
            if a == b || b == c || a == c {
                continue;
            }
            if i % 2 == 0 {
                out.push(a);
                out.push(b);
                out.push(c);
            } else {
                out.push(b);
                out.push(a);
                out.push(c);
            }
        }
    } else if prim == 2 {
        // Triangle fan
        for i in 1..idx.len().saturating_sub(1) {
            out.push(idx[0]);
            out.push(idx[i]);
            out.push(idx[i + 1]);
        }
    }

    out
}

/// Deduplicates vertex references and builds a [`MapBatch`] draw call for a material group.
fn collect_batch(
    map: &mut MapFile,
    group: BatchGroup,
    sec: &VertexSection,
    material: usize,
    tris: &[u32],
) {
    let mut used_map = HashMap::new();
    let mut batch_pos = Vec::new();
    let mut batch_uv = Vec::new();
    let mut batch_col = Vec::new();
    let mut batch_nrm = Vec::new();
    let mut batch_indices = Vec::with_capacity(tris.len());

    for &idx in tris {
        let u_idx = idx as usize;
        if u_idx >= sec.positions.len() {
            continue;
        }

        let new_idx = *used_map.entry(u_idx).or_insert_with(|| {
            let cur = batch_pos.len() as u32;
            batch_pos.push(sec.positions[u_idx]);
            batch_uv.push(sec.uvs[u_idx]);
            batch_col.push(sec.colors[u_idx]);
            batch_nrm.push(sec.normals[u_idx]);
            cur
        });

        batch_indices.push(new_idx);
    }

    if !batch_indices.is_empty() {
        map.batches.push(MapBatch {
            group,
            material_index: material,
            geo_id: 0,
            positions: batch_pos,
            uvs: batch_uv,
            colors: batch_col,
            normals: batch_nrm,
            indices: batch_indices,
        });
    }
}

/// Parses sub-file type 2: embedded textures, headers, sprite declarations, and DXT byte streams.
fn parse_textures_subfile(data: &[u8], mut p: usize, map: &mut MapFile) -> Result<(), AssetError> {
    p += 16;
    while p + 32 <= data.len() {
        let tex_id = read_u32(data, p)?;
        if tex_id == 0 {
            break;
        }

        let sprite_count = read_u32(data, p + 12)? as usize;
        p += 32;

        for _ in 0..sprite_count {
            if p + 32 > data.len() {
                return Ok(());
            }

            let w = read_u16(data, p + 8)? as usize;
            let h = read_u16(data, p + 10)? as usize;
            let fmt = read_u16(data, p + 12)?;
            let length = read_u32(data, p + 16)? as usize;
            p += 32;

            if length != 0 && p + length <= data.len() {
                let kind = match fmt {
                    0x100 => 1,         // BC1 / DXT1
                    0x101 | 0x102 => 3, // BC2 / DXT3
                    0x103 | 0x104 => 5, // BC3 / DXT5
                    _ => 3,
                };

                let tex_data = data[p..p + length].to_vec();
                map.textures.insert(
                    tex_id,
                    MapTexture {
                        id: tex_id,
                        width: w,
                        height: h,
                        kind,
                        data: tex_data,
                    },
                );
            }
            p += length;
        }
    }

    Ok(())
}
