//! Map format (.map) parser for geometry, materials, and textures.

use std::collections::HashMap;
use bytemuck::{Pod, Zeroable};

pub const MAP_MAGIC: u32 = 0x20010510;

/// Map binary header format matching original PS2/PC map files.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MapHeader {
    pub magic: u32,
    pub size: u32,
    pub chunk_count: u32,
    pub unused: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapMaterial {
    pub mode: u16,
    pub texture_id: u16,
    pub color: u32,
    pub overlay: u32,
    pub specularity: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTexture {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub kind: u32, // 1: DXT1, 3: DXT3, 5: DXT5
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct MapVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchGroup {
    Opaque,
    Transparent,
    Decal,
}

#[derive(Debug, Clone)]
pub struct MapBatch {
    pub group: BatchGroup,
    pub material_id: u32,
    pub geo_id: u16,
    pub vertices: Vec<MapVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct MapFile {
    pub materials: Vec<MapMaterial>,
    pub textures: HashMap<u32, MapTexture>,
    pub batches: Vec<MapBatch>,
}

struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    fn u8(&self, ofs: usize) -> Option<u8> {
        self.data.get(ofs).copied()
    }

    fn u16(&self, ofs: usize) -> Option<u16> {
        let b = self.data.get(ofs..ofs + 2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&self, ofs: usize) -> Option<u32> {
        let b = self.data.get(ofs..ofs + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&self, ofs: usize) -> Option<f32> {
        let b = self.data.get(ofs..ofs + 4)?;
        Some(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

fn strip_to_list(indices: &[u32], prim: u8) -> Vec<u32> {
    if prim == 3 {
        let len = (indices.len() / 3) * 3;
        return indices[..len].to_vec();
    }
    let mut out = Vec::new();
    if prim == 1 {
        // Triangle strip
        for i in 0..indices.len().saturating_sub(2) {
            let a = indices[i];
            let b = indices[i + 1];
            let c = indices[i + 2];
            if a == b || b == c || a == c {
                continue;
            }
            if i % 2 == 0 {
                out.extend_from_slice(&[a, b, c]);
            } else {
                out.extend_from_slice(&[b, a, c]);
            }
        }
    } else if prim == 2 {
        // Triangle fan
        for i in 1..indices.len().saturating_sub(1) {
            out.extend_from_slice(&[indices[0], indices[i], indices[i + 1]]);
        }
    }
    out
}

struct VertexSection {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    normals: Vec<[f32; 3]>,
}

fn parse_vertex_sections(r: &Reader, base: usize) -> Option<Vec<VertexSection>> {
    let count = r.u32(base + 4)? as usize;
    let mut info = Vec::with_capacity(count);
    for j in 0..count {
        let entry = base + 8 + 12 * j;
        let ofs = r.u32(entry)? as usize;
        let stride = r.u32(entry + 4)? as usize;
        let length = r.u32(entry + 8)? as usize;
        info.push((ofs, stride, length));
    }
    let start = base + 8 + 12 * count;
    let mut sections = Vec::with_capacity(count);

    for (ofs, stride, length) in info {
        let vtx_data_start = start + ofs;
        if stride == 0 || vtx_data_start + length > r.data.len() {
            continue;
        }
        let num_vertices = length / stride;
        let mut positions = Vec::with_capacity(num_vertices);
        let mut uvs = Vec::with_capacity(num_vertices);
        let mut colors = Vec::with_capacity(num_vertices);
        let mut normals = Vec::with_capacity(num_vertices);

        for i in 0..num_vertices {
            let p = vtx_data_start + i * stride;
            let px = r.f32(p).unwrap_or(0.0);
            let py = r.f32(p + 4).unwrap_or(0.0);
            let pz = r.f32(p + 8).unwrap_or(0.0);
            positions.push([px, py, pz]);

            let mut norm = [0.0f32, 0.0, 0.0];
            let mut uv = [0.0f32, 0.0];
            let mut col = [1.0f32, 1.0, 1.0, 1.0];

            match stride {
                0x14 => {
                    uv = [r.f32(p + 12).unwrap_or(0.0), r.f32(p + 16).unwrap_or(0.0)];
                }
                0x18 => {
                    // BGRA u8 at 0x0c
                    let b = r.u8(p + 12).unwrap_or(255) as f32 / 255.0;
                    let g = r.u8(p + 13).unwrap_or(255) as f32 / 255.0;
                    let r_col = r.u8(p + 14).unwrap_or(255) as f32 / 255.0;
                    let a = r.u8(p + 15).unwrap_or(255) as f32 / 255.0;
                    col = [r_col, g, b, a];
                    uv = [r.f32(p + 16).unwrap_or(0.0), r.f32(p + 20).unwrap_or(0.0)];
                }
                0x20 => {
                    norm = [
                        r.f32(p + 12).unwrap_or(0.0),
                        r.f32(p + 16).unwrap_or(0.0),
                        r.f32(p + 20).unwrap_or(0.0),
                    ];
                    uv = [r.f32(p + 24).unwrap_or(0.0), r.f32(p + 28).unwrap_or(0.0)];
                }
                s if s >= 0x24 => {
                    norm = [
                        r.f32(p + 12).unwrap_or(0.0),
                        r.f32(p + 16).unwrap_or(0.0),
                        r.f32(p + 20).unwrap_or(0.0),
                    ];
                    let b = r.u8(p + 24).unwrap_or(255) as f32 / 255.0;
                    let g = r.u8(p + 25).unwrap_or(255) as f32 / 255.0;
                    let r_col = r.u8(p + 26).unwrap_or(255) as f32 / 255.0;
                    let a = r.u8(p + 27).unwrap_or(255) as f32 / 255.0;
                    col = [r_col, g, b, a];
                    uv = [r.f32(p + 28).unwrap_or(0.0), r.f32(p + 32).unwrap_or(0.0)];
                }
                _ => {}
            }

            uvs.push(uv);
            colors.push(col);
            normals.push(norm);
        }

        sections.push(VertexSection {
            positions,
            uvs,
            colors,
            normals,
        });
    }

    Some(sections)
}

fn collect_batch(
    batches: &mut Vec<MapBatch>,
    group: BatchGroup,
    sections: &[VertexSection],
    section_idx: usize,
    material_id: u32,
    geo_id: u16,
    tris: &[u32],
) {
    if tris.is_empty() || section_idx >= sections.len() {
        return;
    }
    let section = &sections[section_idx];
    let mut remap = HashMap::new();
    let mut out_vertices = Vec::new();
    let mut out_indices = Vec::with_capacity(tris.len());

    for &idx in tris {
        let vert_idx = idx as usize;
        if vert_idx >= section.positions.len() {
            continue;
        }
        let mapped = *remap.entry(vert_idx).or_insert_with(|| {
            let next = out_vertices.len() as u32;
            out_vertices.push(MapVertex {
                position: section.positions[vert_idx],
                normal: section.normals[vert_idx],
                uv: section.uvs[vert_idx],
                color: section.colors[vert_idx],
            });
            next
        });
        out_indices.push(mapped);
    }

    if !out_indices.is_empty() {
        batches.push(MapBatch {
            group,
            material_id,
            geo_id,
            vertices: out_vertices,
            indices: out_indices,
        });
    }
}

fn parse_mesh_group(
    r: &Reader,
    base: usize,
    group: BatchGroup,
    geo_id: u16,
    batches: &mut Vec<MapBatch>,
) -> Option<()> {
    let count = r.u32(base)? as usize;
    for i in 0..count {
        let ofs = r.u32(base + 4 + i * 4)? as usize;
        let m = base + ofs;
        let vtx_ofs = r.u32(m + 32)? as usize;
        let idx_ofs = r.u32(m + 36)? as usize;
        let idx_len = r.u32(m + 40)? as usize;
        let pg_count = r.u32(m + 48)? as usize;

        let num_indices = idx_len / 2;
        let mut indices = Vec::with_capacity(num_indices);
        for j in 0..num_indices {
            indices.push(r.u16(m + idx_ofs + j * 2)? as u32);
        }

        let sections = parse_vertex_sections(r, m + vtx_ofs)?;
        let mut p = m + 52;
        let mut cursor = 0;
        let mut acc: HashMap<(u32, usize), Vec<u32>> = HashMap::new();

        for _ in 0..pg_count {
            let material = r.u32(p)?;
            let section = r.u32(p + 4)? as usize;
            let part_count = r.u32(p + 8)? as usize;
            p += 12;

            for _ in 0..part_count {
                let strip_len = r.u16(p)? as usize;
                let prim = r.u8(p + 3)?;
                p += 8;

                let n = if prim == 3 { strip_len * 3 } else { strip_len };
                if cursor + n <= indices.len() {
                    let tris = strip_to_list(&indices[cursor..cursor + n], prim);
                    acc.entry((material, section)).or_default().extend(tris);
                }
                cursor += n;
            }
        }

        for ((material, section), tris) in acc {
            collect_batch(batches, group.clone(), &sections, section, material, geo_id, &tris);
        }
    }
    Some(())
}

fn parse_decals(
    r: &Reader,
    base: usize,
    geo_id: u16,
    batches: &mut Vec<MapBatch>,
) -> Option<()> {
    let count = r.u32(base)? as usize;
    for i in 0..count {
        let ofs = r.u32(base + 4 + i * 4)? as usize;
        let g = base + ofs;
        let vtx_ofs = r.u32(g + 32)? as usize;
        let idx_ofs = r.u32(g + 36)? as usize;
        let idx_len = r.u32(g + 40)? as usize;
        let decal_count = r.u32(g + 44)? as usize;

        let num_indices = idx_len / 2;
        let mut indices = Vec::with_capacity(num_indices);
        for j in 0..num_indices {
            indices.push(r.u16(g + idx_ofs + j * 2)? as u32);
        }

        let sections = parse_vertex_sections(r, g + vtx_ofs)?;
        let mut p = g + 48;
        let mut cursor = 0;

        for _ in 0..decal_count {
            let material = r.u32(p)?;
            let section = r.u32(p + 4)? as usize;
            let strip_len = r.u32(p + 8)? as usize;
            let strip_count = r.u32(p + 12)? as usize;
            p += 16;

            let mut tris = Vec::new();
            for _ in 0..strip_count {
                if cursor + strip_len <= indices.len() {
                    tris.extend(strip_to_list(&indices[cursor..cursor + strip_len], 1));
                }
                cursor += strip_len;
            }

            collect_batch(
                batches,
                BatchGroup::Decal,
                &sections,
                section,
                material,
                geo_id,
                &tris,
            );
        }
    }
    Some(())
}

fn map_tex_kind(fmt: u32) -> u32 {
    match fmt {
        0x100 => 1,
        0x101 | 0x102 => 3,
        0x103 | 0x104 => 5,
        _ => 3,
    }
}

fn parse_textures(r: &Reader, mut p: usize, textures: &mut HashMap<u32, MapTexture>) -> Option<()> {
    p += 16;
    loop {
        let tex_id = r.u32(p)?;
        if tex_id == 0 {
            break;
        }
        let sprite_count = r.u32(p + 12)? as usize;
        p += 32;

        for _ in 0..sprite_count {
            let w = r.u16(p + 6)? as u32;
            let h = r.u16(p + 8)? as u32;
            let fmt = r.u32(p + 10)?;
            let length = r.u32(p + 14)? as usize;
            p += 32;

            if length > 0 && p + length <= r.data.len() {
                textures.insert(
                    tex_id,
                    MapTexture {
                        id: tex_id,
                        width: w,
                        height: h,
                        kind: map_tex_kind(fmt),
                        data: r.data[p..p + length].to_vec(),
                    },
                );
                p += length;
            }
        }
    }
    Some(())
}

impl MapFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        let r = Reader::new(bytes);
        let magic = r.u32(0).ok_or("Buffer too short for header")?;
        if magic != MAP_MAGIC {
            return Err("Invalid MAP magic identifier");
        }
        let sub_count = r.u32(8).ok_or("Invalid subfile count")? as usize;
        let mut map_file = MapFile::default();
        let mut p = 16;

        for _ in 0..sub_count {
            let sub_type = r.u32(p).ok_or("Truncated subfile header")?;
            let sub_len = r.u32(p + 4).ok_or("Truncated subfile length")? as usize;
            let body = p + 16;

            if sub_type == 1 {
                let geo_count = r.u32(body + 4).ok_or("Truncated geo_count")? as usize;
                let geo_size = r.u32(body + 8).ok_or("Truncated geo_size")? as usize;
                let mat_count = r.u32(body + 12).ok_or("Truncated mat_count")? as usize;

                let mut g = body + 16;
                for _ in 0..geo_count {
                    let geo_id = (r.u32(g).unwrap_or(0) & 0xffff) as u16;
                    let size = r.u32(g + 4).unwrap_or(0) as usize;
                    let o_ofs = r.u32(g + 8).unwrap_or(0) as usize;
                    let t_ofs = r.u32(g + 12).unwrap_or(0) as usize;
                    let d_ofs = r.u32(g + 16).unwrap_or(0) as usize;

                    if o_ofs > 0 {
                        parse_mesh_group(&r, g + o_ofs, BatchGroup::Opaque, geo_id, &mut map_file.batches);
                    }
                    if t_ofs > 0 {
                        parse_mesh_group(&r, g + t_ofs, BatchGroup::Transparent, geo_id, &mut map_file.batches);
                    }
                    if d_ofs > 0 {
                        parse_decals(&r, g + d_ofs, geo_id, &mut map_file.batches);
                    }
                    g += size;
                }

                let mp = body + geo_size;
                for i in 0..mat_count {
                    let pos = mp + 16 * i;
                    if let (Some(mode), Some(texture_id), Some(color), Some(overlay), Some(specularity)) = (
                        r.u16(pos),
                        r.u16(pos + 2),
                        r.u32(pos + 4),
                        r.u32(pos + 8),
                        r.f32(pos + 12),
                    ) {
                        map_file.materials.push(MapMaterial {
                            mode,
                            texture_id,
                            color,
                            overlay,
                            specularity,
                        });
                    }
                }
            } else if sub_type == 2 {
                parse_textures(&r, body, &mut map_file.textures);
            }

            p = body + sub_len;
        }

        Ok(map_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_header_pod() {
        assert_eq!(std::mem::size_of::<MapHeader>(), 16);
    }

    #[test]
    fn test_map_vertex_layout() {
        assert_eq!(std::mem::size_of::<MapVertex>(), 48);
    }

    #[test]
    fn test_real_map_file() {
        let path = "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map";
        if let Ok(bytes) = std::fs::read(path) {
            let map = MapFile::parse(&bytes).expect("Failed to parse rr21.map");
            assert_eq!(map.materials.len(), 14);
            assert_eq!(map.batches.len(), 13);
            let total_tris: usize = map.batches.iter().map(|b| b.indices.len() / 3).sum();
            assert_eq!(total_tris, 21299);
        }
    }
}
