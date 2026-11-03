use crate::error::AssetError;
use crate::formats::dxt::decode_dxt;
use std::collections::HashMap;

pub const MDL_MAGIC: [u8; 4] = [0x03, 0x00, 0xFF, 0xFF];

#[derive(Debug, Clone)]
pub struct ModelPrimitive {
    pub bone_indices: Vec<u16>,
    pub pair_indices: Vec<u16>,
    pub texture_index: u16,
    pub start: usize,
    pub length: usize,
    pub pose_index: u8,
    pub material_type: u8,
    pub backface_culling: u32,
    pub diffuse: [f32; 3],
    pub ambient: [f32; 3],
    pub specular: [f32; 3],
    pub is_transparent: bool,
}

#[derive(Debug, Clone)]
pub struct ModelTexture {
    pub id: u32,
    pub width: usize,
    pub height: usize,
    pub kind: u8,
    pub data: Vec<u8>,
}

impl ModelTexture {
    pub fn to_rgba(&self) -> Result<Vec<u8>, AssetError> {
        decode_dxt(&self.data, self.width, self.height, self.kind)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelVertices {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
    pub bone_indices: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

#[derive(Debug, Clone)]
pub struct ModelFile {
    pub bone_count: usize,
    pub parents: Vec<u8>,
    pub initial_matrices: Vec<[[f32; 4]; 4]>,
    pub pairs: Vec<(u8, u8)>,
    pub primitives: Vec<ModelPrimitive>,
    pub strip_indices: Vec<u16>,
    pub vertices: ModelVertices,
    pub textures: HashMap<u32, ModelTexture>,
}

pub fn parse_mdl(data: &[u8]) -> Result<ModelFile, AssetError> {
    if data.len() < 68 {
        return Err(AssetError::BufferUnderflow {
            required: 68,
            available: data.len(),
        });
    }

    let no_tex = data[0];
    let tex_count = read_u32(data, 4)? as usize;
    let tex_ofs = read_u32(data, 8)? as usize;
    let mdl_ofs = read_u32(data, 16)? as usize;

    let base = if mdl_ofs != 0 { mdl_ofs } else { 64 };
    if base + 56 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: base + 56,
            available: data.len(),
        });
    }

    if data[base..base + 4] != MDL_MAGIC {
        return Err(AssetError::InvalidMagic {
            expected: &MDL_MAGIC,
            found: data[base..base + 4].to_vec(),
        });
    }

    let bone_count = read_u32(data, base + 12)? as usize;
    let pairs_count = read_u32(data, base + 20)? as usize;
    let pairs_ofs = read_u32(data, base + 24)? as usize;
    let prim_count = read_u32(data, base + 32)? as usize;
    let prim_ofs = read_u32(data, base + 36)? as usize;
    let vertex_count = read_u32(data, base + 128)? as usize;
    let vertex_ofs = read_u32(data, base + 132)? as usize;
    let tri_ofs = read_u32(data, base + 144)? as usize;

    // Bone hierarchy and bind matrices
    let skel_ofs = base + 64;
    let mut parents = Vec::with_capacity(bone_count);
    if skel_ofs + bone_count <= data.len() {
        parents.extend_from_slice(&data[skel_ofs..skel_ofs + bone_count]);
    } else {
        parents.resize(bone_count, 255);
    }

    let mat_ofs = base + 64 + ((bone_count + 15) & !15);
    let mut initial_matrices = Vec::with_capacity(bone_count);
    for b in 0..bone_count {
        let mo = mat_ofs + b * 64;
        if mo + 64 <= data.len() {
            let mut m = [[0.0f32; 4]; 4];
            #[allow(clippy::needless_range_loop)]
            for col in 0..4 {
                for row in 0..4 {
                    m[row][col] = read_f32(data, mo + (col * 4 + row) * 4)?;
                }
            }
            initial_matrices.push(m);
        } else {
            // Identity matrix fallback
            let mut id = [[0.0f32; 4]; 4];
            id[0][0] = 1.0;
            id[1][1] = 1.0;
            id[2][2] = 1.0;
            id[3][3] = 1.0;
            initial_matrices.push(id);
        }
    }

    // Bone pairs
    let mut pairs = Vec::with_capacity(pairs_count);
    let po = base + pairs_ofs;
    for i in 0..pairs_count {
        let entry = po + i * 4;
        if entry + 2 <= data.len() {
            pairs.push((data[entry], data[entry + 1]));
        }
    }

    // Primitives
    let mut primitives = Vec::with_capacity(prim_count);
    let mut p = base + prim_ofs;
    for _ in 0..prim_count {
        if p + 4 > data.len() {
            break;
        }
        let size = read_u32(data, p)? as usize;
        let h = p + 4;
        if h + 124 > data.len() {
            break;
        }

        let bcount = read_u32(data, h + 4)? as usize;
        let pcount = read_u32(data, h + 12)? as usize;
        let pofs = read_u32(data, h + 16)? as usize;
        let tcount = read_u32(data, h + 20)? as usize;
        let tofs = read_u32(data, h + 24)? as usize;

        let mtype = data[h + 32];
        let pose = data[h + 34];
        let cull = read_u32(data, h + 36)?;

        let diffuse = [
            read_f32(data, h + 64)?,
            read_f32(data, h + 68)?,
            read_f32(data, h + 72)?,
        ];
        let ambient = [
            read_f32(data, h + 80)?,
            read_f32(data, h + 84)?,
            read_f32(data, h + 88)?,
        ];
        let specular = [
            read_f32(data, h + 96)?,
            read_f32(data, h + 100)?,
            read_f32(data, h + 104)?,
        ];
        let start = read_u32(data, h + 112)? as usize;
        let length = read_u32(data, h + 116)? as usize;

        let mut bones = Vec::with_capacity(bcount);
        for b in 0..bcount {
            bones.push(read_u16(data, h + 124 + b * 2)?);
        }

        let mut pidx = Vec::with_capacity(pcount);
        if pcount > 0 && p + pofs + pcount * 2 <= data.len() {
            for k in 0..pcount {
                pidx.push(read_u16(data, p + pofs + k * 2)?);
            }
        }

        let tidx = if tcount > 0 && p + tofs + 2 <= data.len() {
            read_u16(data, p + tofs)?
        } else {
            0
        };

        primitives.push(ModelPrimitive {
            bone_indices: bones,
            pair_indices: pidx,
            texture_index: tidx,
            start,
            length,
            pose_index: pose,
            material_type: mtype,
            backface_culling: cull,
            diffuse,
            ambient,
            specular,
            is_transparent: false,
        });

        p += size;
    }

    // Strip indices
    let end_idx = if vertex_ofs > tri_ofs {
        vertex_ofs
    } else {
        data.len() - base
    };
    let strip_len_bytes = end_idx.saturating_sub(tri_ofs);
    let num_strips = strip_len_bytes / 2;
    let mut strip_indices = Vec::with_capacity(num_strips);
    let s_start = base + tri_ofs;
    for i in 0..num_strips {
        if s_start + (i + 1) * 2 <= data.len() {
            strip_indices.push(read_u16(data, s_start + i * 2)?);
        }
    }

    // Vertices (0x30 bytes per vertex)
    let mut vertices = ModelVertices::default();
    let v_start = base + vertex_ofs;
    let stride = 0x30;
    if v_start + vertex_count * stride <= data.len() {
        vertices.positions.reserve(vertex_count);
        vertices.uvs.reserve(vertex_count);
        vertices.normals.reserve(vertex_count);
        vertices.bone_indices.reserve(vertex_count);
        vertices.weights.reserve(vertex_count);

        for i in 0..vertex_count {
            let vo = v_start + i * stride;
            let px = read_f32(data, vo)?;
            let py = read_f32(data, vo + 4)?;
            let pz = read_f32(data, vo + 8)?;
            vertices.positions.push([px, py, pz]);

            let w0 = read_f32(data, vo + 12)?;
            let w1 = read_f32(data, vo + 16)?;
            let w2 = read_f32(data, vo + 20)?;
            let w3 = read_f32(data, vo + 24)?;
            vertices.weights.push([w0, w1, w2, w3]);

            let nx = (read_i16(data, vo + 28)? as f32) / -32768.0;
            let ny = (read_i16(data, vo + 30)? as f32) / -32768.0;
            let nz = (read_i16(data, vo + 32)? as f32) / -32768.0;
            vertices.normals.push([nx, ny, nz]);

            let u = read_f32(data, vo + 36)?;
            let v = read_f32(data, vo + 40)?;
            vertices.uvs.push([u, v]);

            let b0 = data[vo + 44] as u16;
            let b1 = data[vo + 45] as u16;
            let b2 = data[vo + 46] as u16;
            let b3 = data[vo + 47] as u16;
            vertices.bone_indices.push([b0, b1, b2, b3]);
        }
    }

    // Embedded textures
    let mut textures = HashMap::new();
    if tex_count > 0 && no_tex == 0 && tex_ofs != 0 && tex_ofs + 16 <= data.len() {
        let mut t = tex_ofs + 16;
        for _ in 0..tex_count {
            if t + 32 > data.len() {
                break;
            }
            let tid = read_u32(data, t)?;
            let w = read_u16(data, t + 4)? as usize;
            let h = read_u16(data, t + 6)? as usize;
            let sprites = read_u16(data, t + 12)? as usize;
            t += 4 + 8 + 2 + 18;

            let mut fmt = 0u8;
            for _ in 0..sprites {
                if t + 32 <= data.len() {
                    fmt = data[t + 12];
                    t += 32;
                }
            }

            let kind = match fmt {
                0 => 1,
                1 | 2 => 3,
                3 | 4 => 5,
                _ => 3,
            };

            let size = if kind == 1 { w * h / 2 } else { w * h };
            if t + size <= data.len() {
                textures.insert(
                    tid,
                    ModelTexture {
                        id: tid,
                        width: w,
                        height: h,
                        kind,
                        data: data[t..t + size].to_vec(),
                    },
                );
                t += size;
            }
        }
    }

    Ok(ModelFile {
        bone_count,
        parents,
        initial_matrices,
        pairs,
        primitives,
        strip_indices,
        vertices,
        textures,
    })
}

#[inline(always)]
fn read_u16(data: &[u8], ofs: usize) -> Result<u16, AssetError> {
    if ofs + 2 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        })
    } else {
        Ok(u16::from_le_bytes([data[ofs], data[ofs + 1]]))
    }
}

#[inline(always)]
fn read_i16(data: &[u8], ofs: usize) -> Result<i16, AssetError> {
    if ofs + 2 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        })
    } else {
        Ok(i16::from_le_bytes([data[ofs], data[ofs + 1]]))
    }
}

#[inline(always)]
fn read_u32(data: &[u8], ofs: usize) -> Result<u32, AssetError> {
    if ofs + 4 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        })
    } else {
        Ok(u32::from_le_bytes([
            data[ofs],
            data[ofs + 1],
            data[ofs + 2],
            data[ofs + 3],
        ]))
    }
}

#[inline(always)]
fn read_f32(data: &[u8], ofs: usize) -> Result<f32, AssetError> {
    if ofs + 4 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        })
    } else {
        Ok(f32::from_le_bytes([
            data[ofs],
            data[ofs + 1],
            data[ofs + 2],
            data[ofs + 3],
        ]))
    }
}
