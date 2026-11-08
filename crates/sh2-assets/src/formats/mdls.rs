//! Safe binary parser for Silent Hill 2 Character Models (`.mdls` / `.mdl`).
//!
//! # Format Architecture & Memory Layout
//! Character models in Silent Hill 2 store skeletal hierarchies, bind pose matrices,
//! bone pairs for dual-bone vertex blending, triangle primitives, and vertex buffers.
//!
//! ### Binary File Header (64 bytes)
//! | Offset | Type | Description |
//! |--------|------|-------------|
//! | `0x00` | `u8`  | `no_tex` flag (non-zero when textures are externalized) |
//! | `0x04` | `u32` | Number of embedded textures (`tex_count`) |
//! | `0x08` | `u32` | Byte offset to texture table (`tex_ofs`) |
//! | `0x10` | `u32` | Byte offset to main model header (`mdl_ofs`, typically `0x40`) |
//!
//! ### Model Header (Magic `0x03 0x00 0xFF 0xFF` at base offset `0x40`)
//! Offsets inside the model header are relative to the model base offset (`0x40`).
//! | Relative Offset | Field | Description |
//! |-----------------|-------|-------------|
//! | `+0x00` | Magic | `0x03, 0x00, 0xFF, 0xFF` identifier |
//! | `+0x08` | `init_ofs` | Bind pose transformation matrices (4x4 floats per bone) |
//! | `+0x0C` | `bone_count` | Number of bones in the skeleton hierarchy |
//! | `+0x10` | `skel_ofs` | Skeleton parent bone array (parent index, 255 for root) |
//! | `+0x14` | `pairs_count` | Number of bone blending pairs |
//! | `+0x18` | `pairs_ofs` | Offset to bone pair indices `(parent, child)` |
//! | `+0x20` | `prim_count` | Number of primitive mesh draw commands |
//! | `+0x24` | `prim_ofs` | Offset to primitive headers |
//! | `+0x80` | `vertex_count` | Total opaque vertices in buffer (at byte 128) |
//! | `+0x84` | `vertex_ofs` | Byte offset to vertex array (at byte 132) |
//! | `+0x90` | `tri_ofs` | Byte offset to shared `u16` triangle strip list (at byte 144) |
//!
//! ### Vertex Layout (0x30 bytes per vertex)
//! Vertices are initially stored in the local coordinate space of their primary bone.
//! | Offset | Type | Field | Description |
//! |--------|------|-------|-------------|
//! | `+0x00` | `[f32; 3]` | `positions` | Local coordinate position $(X, Y, Z)$ |
//! | `+0x0C` | `[f32; 4]` | `weights` | Blend weights ($w_0, w_1, w_2, w_3$) |
//! | `+0x1C` | `[i16; 3]` | `normals` | Compressed fixed-point normal vectors |
//! | `+0x22` | `u16` | `pad` | Alignment padding |
//! | `+0x24` | `[f32; 2]` | `uvs` | Texture mapping coordinates $(U, V)$ |
//! | `+0x2C` | `[u8; 4]`  | `bones` | Bone indices ($b_0$ primary bone, $b_{1..3}$ pair indices) |

use super::binary::{read_f32, read_i16, read_u16, read_u32};
use crate::error::AssetError;
use crate::formats::dxt::decode_dxt;
use std::collections::HashMap;

/// Model header magic identifier (`0x03, 0x00, 0xFF, 0xFF`).
pub const MDL_MAGIC: [u8; 4] = [0x03, 0x00, 0xFF, 0xFF];

/// Mesh primitive partition within a character model.
///
/// Primitives define the material properties, active bones, backface culling,
/// and the sub-range of indices in the model's shared triangle strip buffer.
#[derive(Debug, Clone)]
pub struct ModelPrimitive {
    /// Bone indices from the skeleton hierarchy active for this primitive.
    pub bone_indices: Vec<u16>,
    /// Bone pair indices into [`ModelFile::pairs`] used for multi-bone weighting.
    pub pair_indices: Vec<u16>,
    /// Index into the model's texture list.
    pub texture_index: u16,
    /// Starting offset into the shared strip index buffer.
    pub start: usize,
    /// Number of strip indices spanned by this primitive.
    pub length: usize,
    /// Target pose identifier.
    pub pose_index: u8,
    /// Material shader configuration code.
    pub material_type: u8,
    /// Backface culling flags (0 = double sided, 1 = cull backfaces).
    pub backface_culling: u32,
    /// Diffuse surface color reflection $(R, G, B)$.
    pub diffuse: [f32; 3],
    /// Ambient lighting color $(R, G, B)$.
    pub ambient: [f32; 3],
    /// Specular reflection color $(R, G, B)$.
    pub specular: [f32; 3],
    /// Indicates whether primitive is rendered in the alpha transparency pass.
    pub is_transparent: bool,
}

/// Embedded character model texture.
#[derive(Debug, Clone)]
pub struct ModelTexture {
    /// Texture identification number.
    pub id: u32,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Compression kind: 1 = DXT1 (BC1), 3 = DXT3 (BC2), 5 = DXT5 (BC3).
    pub kind: u8,
    /// Compressed texture payload data.
    pub data: Vec<u8>,
}

impl ModelTexture {
    /// Decodes the compressed texture payload into an uncompressed RGBA8 byte buffer.
    pub fn to_rgba(&self) -> Result<Vec<u8>, AssetError> {
        decode_dxt(&self.data, self.width, self.height, self.kind)
    }
}

impl ModelFile {
    /// Converts a primitive's triangle strip index range into a list of triangle index triples `(i0, i1, i2)`.
    ///
    /// Strips are separated by degenerate triangles (`i0 == i1` or `i1 == i2` or `i0 == i2`).
    /// Triangle winding alternates every triangle to maintain consistent front-facing orientation.
    pub fn primitive_triangles(&self, prim_index: usize) -> Vec<[u16; 3]> {
        if prim_index >= self.primitives.len() {
            return Vec::new();
        }
        let pr = &self.primitives[prim_index];
        let start = pr.start;
        let end = (pr.start + pr.length).min(self.strip_indices.len());
        if start + 2 >= end {
            return Vec::new();
        }

        let slice = &self.strip_indices[start..end];
        let mut tris = Vec::new();

        for i in 0..slice.len().saturating_sub(2) {
            let x = slice[i];
            let y = slice[i + 1];
            let z = slice[i + 2];

            // Ignore degenerate restart triangles
            if x == y || y == z || x == z {
                continue;
            }

            if i % 2 == 0 {
                tris.push([x, y, z]);
            } else {
                tris.push([y, x, z]);
            }
        }

        tris
    }
}

/// Vertex attributes for all vertices in the character model.
///
/// Follows the native 0x30-byte vertex stride layout unpacked into contiguous vectors.
#[derive(Debug, Clone, Default)]
pub struct ModelVertices {
    /// Local vertex positions $(X, Y, Z)$ before bone transform skinning.
    pub positions: Vec<[f32; 3]>,
    /// Texture coordinates $(U, V)$.
    pub uvs: Vec<[f32; 2]>,
    /// Normalized surface normal vectors $(X, Y, Z)$.
    pub normals: Vec<[f32; 3]>,
    /// Bone indices assigned to each vertex (up to 4 bones).
    pub bone_indices: Vec<[u16; 4]>,
    /// Blending weights corresponding to the bone indices (sum to 1.0).
    pub weights: Vec<[f32; 4]>,
}

/// Ingested representation of a complete character model (`.mdls` / `.mdl`).
#[derive(Debug, Clone)]
pub struct ModelFile {
    /// Total number of bones in the skeleton.
    pub bone_count: usize,
    /// Parent bone index array (`parents[bone_idx] == 255` for root bones).
    pub parents: Vec<u8>,
    /// Bind pose model-space transform matrices per bone (row-major $4 \times 4$).
    pub initial_matrices: Vec<[[f32; 4]; 4]>,
    /// Pre-computed inverted bind pose matrices per bone ($M_{initial}^{-1}$) for zero-allocation skinning.
    pub inv_initial_matrices: Vec<[[f32; 4]; 4]>,
    /// Bone pairing table `(parent, child)` referenced by primitive blend weights.
    pub pairs: Vec<(u8, u8)>,
    /// Sub-mesh primitives defining draw ranges and material settings.
    pub primitives: Vec<ModelPrimitive>,
    /// Shared triangle strip index list indexing into `vertices`.
    pub strip_indices: Vec<u16>,
    /// Contiguous vertex buffers.
    pub vertices: ModelVertices,
    /// Embedded textures keyed by texture ID.
    pub textures: HashMap<u32, ModelTexture>,
}

/// Returns the canonical companion `.mdl` path that provides the embedded texture container
/// for `no_tex = 1` mirror/reflection or untextured character model variants.
///
/// On the PS2 (`SLUS_202.28`), reflection models (`ragl.mdl`, `rhhh_edi.mdl`, `rlau.mdl`, `rmry.mdl`)
/// and untextured cutscene variants (`hhh_edi_notex.mdl`) set `no_tex = 1` in their 64-byte file header
/// and bind the already-resident GS VRAM textures uploaded by their primary character `.mdl`.
pub fn companion_texture_mdl_path(mdl_rel_path: &str) -> Option<&'static str> {
    let normalized = mdl_rel_path.replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(mdl_rel_path);
    match file_name {
        "ragl.mdl" => Some("data/chr/agl/agl.mdl"),
        "rhhh_edi.mdl" | "hhh_edi_notex.mdl" => Some("data/chr/edi/hhh_edi.mdl"),
        "rlau.mdl" => Some("data/chr/lau/lau.mdl"),
        "rmry.mdl" => Some("data/chr/mry/mry.mdl"),
        "rmxx.mdl" => Some("data/chr/mxx/mxx.mdl"),
        "rhhh_jms.mdl" | "hhh_jms_notex.mdl" => Some("data/chr/jms/hhh_jms.mdl"),
        "rhhl_jms.mdl" | "hhl_jms_notex.mdl" => Some("data/chr/jms/hhl_jms.mdl"),
        "rhll_jms.mdl" | "hll_jms_notex.mdl" => Some("data/chr/jms/hll_jms.mdl"),
        "rlll_jms.mdl" | "lll_jms_notex.mdl" => Some("data/chr/jms/lll_jms.mdl"),
        "rhhh_mar.mdl" | "hhh_mar_notex.mdl" | "rhhh_mar_notex.mdl" => {
            Some("data/chr/mar/hhh_mar.mdl")
        }
        "rlll_mar.mdl" => Some("data/chr/mar/lll_mar.mdl"),
        _ => None,
    }
}

/// Parses a Silent Hill 2 character model file (`.mdl` / `.mdls`) from raw bytes.
///
/// # Errors
/// Returns [`AssetError::InvalidMagic`] if model header magic `0x03 0x00 0xFF 0xFF` is not found,
/// or [`AssetError::BufferUnderflow`] if byte slices exceed the buffer length.
pub fn parse_mdl(data: &[u8]) -> Result<ModelFile, AssetError> {
    if data.len() < 68 {
        return Err(AssetError::BufferUnderflow {
            required: 68,
            available: data.len(),
        });
    }

    let no_tex = data[0];
    let tex_ofs_w3 = read_u32(data, 12)? as usize;
    let tex_ofs_w2 = read_u32(data, 8)? as usize;
    let tex_ofs = if tex_ofs_w3 + 4 <= data.len()
        && data[tex_ofs_w3..tex_ofs_w3 + 4] == crate::formats::texture::TEX_MAGIC_BYTES
    {
        tex_ofs_w3
    } else if tex_ofs_w2 + 4 <= data.len()
        && data[tex_ofs_w2..tex_ofs_w2 + 4] == crate::formats::texture::TEX_MAGIC_BYTES
    {
        tex_ofs_w2
    } else if tex_ofs_w3 > 0 && tex_ofs_w3 < data.len() {
        tex_ofs_w3
    } else {
        tex_ofs_w2
    };
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

    // Read header offsets
    let bone_count = read_u32(data, base + 12)? as usize;
    let pairs_count = read_u32(data, base + 20)? as usize;
    let pairs_ofs = read_u32(data, base + 24)? as usize;
    let prim_count = read_u32(data, base + 32)? as usize;
    let prim_ofs = read_u32(data, base + 36)? as usize;
    let vertex_count = read_u32(data, base + 128)? as usize;
    let vertex_ofs = read_u32(data, base + 132)? as usize;
    let tri_ofs = read_u32(data, base + 144)? as usize;

    // 1. Bone hierarchy (parents)
    let skel_ofs = read_u32(data, base + 16)? as usize;
    let mut parents = Vec::with_capacity(bone_count);
    let s_start = base + skel_ofs;
    if s_start + bone_count <= data.len() {
        parents.extend_from_slice(&data[s_start..s_start + bone_count]);
    } else {
        parents.resize(bone_count, 255);
    }

    // 2. Initial bind matrices (stored column-major on disk, transposed to row-major in memory)
    let init_ofs = read_u32(data, base + 8)? as usize;
    let mat_ofs = base + init_ofs;
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
            // Identity fallback if truncated
            let mut id = [[0.0f32; 4]; 4];
            id[0][0] = 1.0;
            id[1][1] = 1.0;
            id[2][2] = 1.0;
            id[3][3] = 1.0;
            initial_matrices.push(id);
        }
    }

    // 3. Bone pairs for blending
    let mut pairs = Vec::with_capacity(pairs_count);
    let po = base + pairs_ofs;
    for i in 0..pairs_count {
        let entry = po + i * 2;
        if entry + 2 <= data.len() {
            pairs.push((data[entry], data[entry + 1]));
        }
    }

    // 3b. Texture mapping metadata (main_texture_ids and texture_pairs)
    let pcms_ofs = read_u32(data, base + 28)? as usize;
    let tex_blocks_count = read_u32(data, base + 48)? as usize;
    let tex_id_count = read_u32(data, base + 56)? as usize;

    let mut meta = base + pcms_ofs + 64 * pairs_count;
    let mut main_texture_ids = Vec::with_capacity(tex_blocks_count);
    if meta + 4 * tex_blocks_count <= data.len() {
        for _ in 0..tex_blocks_count {
            main_texture_ids.push(read_u32(data, meta)?);
            meta += 4;
        }
    }
    if !tex_blocks_count.is_multiple_of(4) {
        meta += 16 - (4 * tex_blocks_count) % 16;
    }
    let mut texture_pairs: Vec<(u32, u32)> = Vec::with_capacity(tex_id_count);
    if meta + 8 * tex_id_count <= data.len() {
        for _ in 0..tex_id_count {
            let block_idx = read_u32(data, meta)?;
            let sprite_id = read_u32(data, meta + 4)?;
            texture_pairs.push((block_idx, sprite_id));
            meta += 8;
        }
    }

    // 4. Primitive descriptors
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
            let raw_idx = read_u16(data, p + tofs)? as usize;
            if raw_idx < texture_pairs.len() {
                let (block_idx, _sid) = texture_pairs[raw_idx];
                if (block_idx as usize) < main_texture_ids.len() {
                    main_texture_ids[block_idx as usize] as u16
                } else {
                    raw_idx as u16
                }
            } else {
                raw_idx as u16
            }
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

    // 5. Shared triangle strip index list
    let tprim_count = read_u32(data, base + 40)? as usize;
    let tprim_ofs = read_u32(data, base + 44)? as usize;
    let mut end_idx = if vertex_ofs > tri_ofs {
        vertex_ofs
    } else {
        data.len() - base
    };
    if tprim_count > 0 && tprim_ofs > tri_ofs && tprim_ofs < end_idx {
        end_idx = tprim_ofs;
    }
    let strip_len_bytes = end_idx.saturating_sub(tri_ofs);
    let num_strips = strip_len_bytes / 2;
    let mut strip_indices = Vec::with_capacity(num_strips);
    let s_start = base + tri_ofs;
    for i in 0..num_strips {
        if s_start + (i + 1) * 2 <= data.len() {
            strip_indices.push(read_u16(data, s_start + i * 2)?);
        }
    }

    // 6. Opaque vertices (0x30 bytes per vertex)
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

    // 7. Map vertices to owning primitives using non-degenerate triangle indices
    // to resolve global skeleton bone indices and model-space bind positions
    if !strip_indices.is_empty() && !primitives.is_empty() && !vertices.positions.is_empty() {
        let mut owner = vec![usize::MAX; vertex_count];

        for (k, pr) in primitives.iter().enumerate() {
            let start = pr.start;
            let end = (pr.start + pr.length).min(strip_indices.len());
            if start + 2 < end {
                let slice = &strip_indices[start..end];
                for i in 0..slice.len().saturating_sub(2) {
                    let x = slice[i] as usize;
                    let y = slice[i + 1] as usize;
                    let z = slice[i + 2] as usize;
                    if x == y || y == z || x == z {
                        continue;
                    }
                    for v in [x, y, z] {
                        if v < vertex_count {
                            owner[v] = k;
                        }
                    }
                }
            }
        }

        // Fallback for any vertex referenced only by degenerate strip connectors
        for (k, pr) in primitives.iter().enumerate() {
            let start = pr.start;
            let end = (pr.start + pr.length).min(strip_indices.len());
            if start < end {
                for &idx in &strip_indices[start..end] {
                    let v = idx as usize;
                    if v < vertex_count && owner[v] == usize::MAX {
                        owner[v] = k;
                    }
                }
            }
        }

        // Resolve local bone indices to global skeleton bone indices and normalize weights
        for (i, &k) in owner.iter().enumerate().take(vertex_count) {
            if k == usize::MAX {
                continue;
            }
            let pr = &primitives[k];
            let raw_b = vertices.bone_indices[i];
            let mut resolved_b = [0u16; 4];

            // Primary bone
            let b0_local = raw_b[0] as usize;
            let b0 = if b0_local < pr.bone_indices.len() {
                pr.bone_indices[b0_local]
            } else {
                0
            };
            resolved_b[0] = b0;

            // Secondary bones from bone pairs
            for j in 1..4 {
                let x = raw_b[j] as usize;
                let mut valid_pair = false;
                if x != 0 && x != 255 && vertices.weights[i][j] > 0.0 && x >= pr.bone_indices.len()
                {
                    let pi = x - pr.bone_indices.len();
                    if pi < pr.pair_indices.len() {
                        let pair_idx = pr.pair_indices[pi] as usize;
                        if pair_idx < pairs.len() {
                            resolved_b[j] = pairs[pair_idx].1 as u16;
                            valid_pair = true;
                        }
                    }
                }
                if !valid_pair {
                    vertices.weights[i][j] = 0.0;
                    resolved_b[j] = b0;
                }
            }

            vertices.bone_indices[i] = resolved_b;

            // Transform raw vertex position and normal from bone-local space into model bind space (M @ p)
            if (b0 as usize) < initial_matrices.len() {
                let m = &initial_matrices[b0 as usize];
                let p = vertices.positions[i];
                let mx = m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3];
                let my = m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3];
                let mz = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3];
                vertices.positions[i] = [mx, my, mz];

                let n = vertices.normals[i];
                let nx = m[0][0] * n[0] + m[0][1] * n[1] + m[0][2] * n[2];
                let ny = m[1][0] * n[0] + m[1][1] * n[1] + m[1][2] * n[2];
                let nz = m[2][0] * n[0] + m[2][1] * n[1] + m[2][2] * n[2];
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                if len > 0.0001 {
                    vertices.normals[i] = [nx / len, ny / len, nz / len];
                }
            }

            // Normalize weights
            let w = vertices.weights[i];
            let sum = w[0] + w[1] + w[2] + w[3];
            if sum > 0.0 {
                let inv = 1.0 / sum;
                vertices.weights[i] = [w[0] * inv, w[1] * inv, w[2] * inv, w[3] * inv];
            } else {
                vertices.weights[i] = [1.0, 0.0, 0.0, 0.0];
            }
        }
    }

    // 7b. Transparent primitives (`tprim_count`) and transparent vertices (`tvertex_count`, stride 0x48)
    // High-LOD models (such as `hhh_mar`, `hhh_jms`, `hhh_edi`, `agl`, `lau`, `mry`, `bos`) store
    // semi-transparent hair strands, dyed hair tips, eyelashes, and clothing fringes in a second
    // primitive/strip/vertex stream immediately following the opaque vertices.
    let tvertex_count = read_u32(data, base + 136)? as usize;
    let tvertex_ofs = read_u32(data, base + 140)? as usize;
    let ttri_ofs = read_u32(data, base + 148)? as usize;
    if tprim_count > 0
        && tvertex_count > 0
        && tprim_ofs > 0
        && ttri_ofs > 0
        && tvertex_ofs > ttri_ofs
        && base + tvertex_ofs + tvertex_count * 0x48 <= data.len()
        && vertex_count + tvertex_count <= (u16::MAX as usize)
    {
        let base_tvertex = vertices.positions.len() as u16;
        let base_tstrip = strip_indices.len();

        // Append transparent triangle strip indices (shifted by `base_tvertex`)
        let tstrip_bytes = tvertex_ofs.saturating_sub(ttri_ofs);
        let num_tstrips = tstrip_bytes / 2;
        let ts_start = base + ttri_ofs;
        strip_indices.reserve(num_tstrips);
        for i in 0..num_tstrips {
            if ts_start + (i + 1) * 2 <= data.len() {
                let raw_v = read_u16(data, ts_start + i * 2)?;
                strip_indices.push(raw_v.saturating_add(base_tvertex));
            }
        }

        // Parse transparent primitive headers (144 bytes each at `base + tprim_ofs`)
        let mut tp = base + tprim_ofs;
        for _ in 0..tprim_count {
            if tp + 144 > data.len() {
                break;
            }
            let size = read_u32(data, tp)? as usize;
            if size < 116 || tp + size > data.len() {
                break;
            }

            let tcount = read_u32(data, tp + 8)? as usize;
            let tofs = read_u32(data, tp + 12)? as usize;
            let mtype = data[tp + 20];
            let cull = read_u32(data, tp + 24)?;

            let diffuse = [
                read_f32(data, tp + 52)?,
                read_f32(data, tp + 56)?,
                read_f32(data, tp + 60)?,
            ];
            let ambient = [
                read_f32(data, tp + 68)?,
                read_f32(data, tp + 72)?,
                read_f32(data, tp + 76)?,
            ];
            let specular = [0.0f32, 0.0, 0.0];

            let start = base_tstrip + (read_u32(data, tp + 100)? as usize);
            let length = read_u32(data, tp + 104)? as usize;

            let tidx = if tcount > 0 && tp + tofs + 2 <= data.len() {
                let raw_idx = read_u16(data, tp + tofs)? as usize;
                if raw_idx < texture_pairs.len() {
                    let (block_idx, _sid) = texture_pairs[raw_idx];
                    if (block_idx as usize) < main_texture_ids.len() {
                        main_texture_ids[block_idx as usize] as u16
                    } else {
                        raw_idx as u16
                    }
                } else {
                    raw_idx as u16
                }
            } else {
                0
            };

            primitives.push(ModelPrimitive {
                bone_indices: Vec::new(),
                pair_indices: Vec::new(),
                texture_index: tidx,
                start,
                length,
                pose_index: 0,
                material_type: mtype,
                backface_culling: cull,
                diffuse,
                ambient,
                specular,
                is_transparent: true,
            });

            tp += size;
        }

        // Parse transparent vertices (0x48 = 72 bytes per vertex at `base + tvertex_ofs`)
        let tv_start = base + tvertex_ofs;
        let tstride = 0x48;
        vertices.positions.reserve(tvertex_count);
        vertices.uvs.reserve(tvertex_count);
        vertices.normals.reserve(tvertex_count);
        vertices.bone_indices.reserve(tvertex_count);
        vertices.weights.reserve(tvertex_count);

        for i in 0..tvertex_count {
            let vo = tv_start + i * tstride;
            let px = read_f32(data, vo)?;
            let py = read_f32(data, vo + 4)?;
            let pz = read_f32(data, vo + 8)?;

            let mut w = [
                read_f32(data, vo + 16)?,
                read_f32(data, vo + 20)?,
                read_f32(data, vo + 24)?,
                read_f32(data, vo + 28)?,
            ];

            let nx = read_f32(data, vo + 32)?;
            let ny = read_f32(data, vo + 36)?;
            let nz = read_f32(data, vo + 40)?;

            let u = read_f32(data, vo + 48)?;
            let v = read_f32(data, vo + 52)?;

            // Transparent vertices store 4 x u16 at offset +64..72:
            // - slot 0 (`vo + 64`): direct global skeleton bone index `b0`
            // - slots 1..3 (`vo + 66, 68, 70`): direct global `pairs` table indices (or 0xFFFF if unused)
            let b0_raw = read_u16(data, vo + 64)?;
            let b0 = if (b0_raw as usize) < bone_count {
                b0_raw
            } else {
                0
            };
            let mut resolved_b = [b0; 4];

            for j in 1..4 {
                let pair_idx = read_u16(data, vo + 64 + j * 2)? as usize;
                if pair_idx != 0xFFFF && w[j] > 0.0 && pair_idx < pairs.len() {
                    resolved_b[j] = pairs[pair_idx].1 as u16;
                } else {
                    w[j] = 0.0;
                    resolved_b[j] = b0;
                }
            }

            // Transform position and normal from primary bone `b0` space into model bind space
            let (mut pos, mut nrm) = ([px, py, pz], [nx, ny, nz]);
            if (b0 as usize) < initial_matrices.len() {
                let m = &initial_matrices[b0 as usize];
                pos = [
                    m[0][0] * px + m[0][1] * py + m[0][2] * pz + m[0][3],
                    m[1][0] * px + m[1][1] * py + m[1][2] * pz + m[1][3],
                    m[2][0] * px + m[2][1] * py + m[2][2] * pz + m[2][3],
                ];
                let tnx = m[0][0] * nx + m[0][1] * ny + m[0][2] * nz;
                let tny = m[1][0] * nx + m[1][1] * ny + m[1][2] * nz;
                let tnz = m[2][0] * nx + m[2][1] * ny + m[2][2] * nz;
                let len = (tnx * tnx + tny * tny + tnz * tnz).sqrt();
                if len > 0.0001 {
                    nrm = [tnx / len, tny / len, tnz / len];
                }
            }

            let sum = w[0] + w[1] + w[2] + w[3];
            let norm_w = if sum > 0.0 {
                let inv = 1.0 / sum;
                [w[0] * inv, w[1] * inv, w[2] * inv, w[3] * inv]
            } else {
                [1.0, 0.0, 0.0, 0.0]
            };

            vertices.positions.push(pos);
            vertices.normals.push(nrm);
            vertices.uvs.push([u, v]);
            vertices.bone_indices.push(resolved_b);
            vertices.weights.push(norm_w);
        }
    }

    // 8. Embedded textures (parse 0x19990901 container if present at tex_ofs)
    let mut textures = HashMap::new();
    if no_tex == 0 && tex_ofs != 0 && tex_ofs + 16 <= data.len() {
        if let Ok(container_texs) = crate::formats::texture::parse_texture_container(data, tex_ofs)
        {
            for ctex in container_texs {
                let kind = match ctex.format {
                    0 => 1,
                    1 | 2 => 3,
                    3 | 4 => 5,
                    _ => 3,
                };
                textures.insert(
                    ctex.id,
                    ModelTexture {
                        id: ctex.id,
                        width: ctex.width,
                        height: ctex.height,
                        kind,
                        data: ctex.data,
                    },
                );
            }
        }
    }

    let inv_initial_matrices = initial_matrices
        .iter()
        .map(|m| crate::formats::anm::mat4_inverse(*m))
        .collect();

    Ok(ModelFile {
        bone_count,
        parents,
        initial_matrices,
        inv_initial_matrices,
        pairs,
        primitives,
        strip_indices,
        vertices,
        textures,
    })
}
