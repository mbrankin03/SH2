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
/// Corresponds to the 16-byte material records at the end of PC `.map` (`0x20010510`) geometry
/// subfiles, parsed by `sh2pc.exe` `FUN_005b3e50` (`0x005b3e50`) into 20-byte runtime material
/// records.
///
/// ### PS2 Prototype vs PC Retail Format Note
/// On PS2 (`SLUS_202.28` `sh2gfw_parse_HEAD` `0x00180800` / `sh2gfw_Geom_MakePacket` `0x0017f980`),
/// `.map` files store pre-compiled GS/VU1 GIF packets where untextured primitives set `PRIM.TME = 0`
/// and carry flat/vertex RGBA directly. For the PC port (`sh2pc.exe`), Creature Labs converted this
/// into an explicit 16-byte `MapMaterial` table where `texture_id == 0` binds `NULL` (`SetTexture(0, 0)`
/// in `FUN_005b4940` `0x005b4940`) and `color` (`D3DCOLOR` `0xAARRGGBB`) is uploaded via `FUN_005b2190`
/// (`0x005b2190`) into vertex shader constant `c26`.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMaterial {
    /// Rendering mode / shader flag (`u8` low byte indexes `DAT_008f0338` via `sh2pc.exe` `FUN_005b2d40` `0x005b2d40`).
    pub mode: u16,
    /// Texture ID looked up via `sh2pc.exe` `FUN_0045a2f0`; `0` denotes an untextured material (`IDirect3DTexture8* == NULL`).
    pub texture_id: u16,
    /// Diffuse tint stored in `D3DCOLOR` `0xAARRGGBB` layout (`sh2pc.exe` `FUN_005b2190` `0x005b2190`).
    pub color: u32,
    /// Specular/overlay tint stored in `D3DCOLOR` `0xAARRGGBB` layout (`sh2pc.exe` `FUN_005b2240` `0x005b2240`).
    pub overlay: u32,
    /// Specular reflection power exponent (`sh2pc.exe` `FUN_005b6300` LUT when non-zero).
    pub specularity: f32,
}

impl MapMaterial {
    /// Returns `true` when this material has no diffuse texture (`texture_id == 0`).
    ///
    /// Verified 1:1 against `sh2pc.exe` `FUN_005b3e50` (`0x005b3e50`) + `FUN_005b4940` (`0x005b4940`),
    /// where `texture_id == 0` yields `puVar1[2] = NULL` (`SetTexture(0, NULL)`).
    #[inline]
    #[must_use]
    pub const fn is_untextured(&self) -> bool {
        self.texture_id == 0
    }

    /// Returns `true` when this material's shader mode multiplies lit diffuse color by vertex
    /// shader constant `c26` ([`Self::color`]).
    ///
    /// Verified 1:1 against `sh2pc.exe` `FUN_005b2d40` (`0x005b2d40`) and the mode-to-shader table
    /// `DAT_008f0338` → `DAT_008f0280`:
    /// - `mode & 0xff` in `{1, 2, 6}` → `VS[5]` (`0x008ef230`) / `VS[7]` (`0x008efa10`):
    ///   `mul r3.xyz, r3.xyzz, c26.xyzz` + `add oD0.xyz, r3.xyzz, r3.xyzz` (**uses `c26`**).
    /// - `mode & 0xff == 0` → `VS[1]` (`0x008ef700`) / `VS[3]` (`0x008ef880`): `mov oD0, c14.wwww`
    ///   (`1.0`, **ignores `c26`**).
    /// - `mode & 0xff == 3` → `VS[2]` (`0x008ef7c0`) / `VS[4]` (`0x008ef940`): `mov oD0, v5`
    ///   (**ignores `c26`**).
    /// - `mode & 0xff` in `{4, 5, 7}` → `VS[6]` (`0x008ef490`) / `VS[8]` (`0x008efb68`):
    ///   `mul r4.xyz, v5.xyzz, c22.xyzz` (**ignores `c26`**, uses per-vertex color `v5`).
    #[inline]
    #[must_use]
    pub const fn uses_diffuse_color_constant(&self) -> bool {
        matches!(self.mode & 0xff, 1 | 2 | 6)
    }

    /// Unpacks [`Self::color`] (`D3DCOLOR` `0xAARRGGBB`) into `[R, G, B, 255]` bytes.
    ///
    /// Verified 1:1 against `sh2pc.exe` `FUN_005b2190` (`0x005b2190`), which extracts
    /// `R = (color >> 16) & 0xff`, `G = (color >> 8) & 0xff`, `B = color & 0xff` and forces `A = 1.0`.
    #[inline]
    #[must_use]
    pub const fn diffuse_rgba_u8(&self) -> [u8; 4] {
        [
            ((self.color >> 16) & 0xff) as u8,
            ((self.color >> 8) & 0xff) as u8,
            (self.color & 0xff) as u8,
            0xff,
        ]
    }

    /// Unpacks [`Self::color`] (`D3DCOLOR` `0xAARRGGBB`) into normalized `[R, G, B, 1.0]` floats.
    ///
    /// Exact 1:1 parity with `sh2pc.exe` `FUN_005b2190` (`0x005b2190`), which populates vertex
    /// shader constant `c26` (`_DAT_01db8880..0x01db888c`) using `* (1.0 / 255.0)` (`0.003921569`)
    /// for RGB and `1.0` (`0x3f800000`) for alpha.
    #[inline]
    #[must_use]
    pub fn diffuse_rgba_f32(&self) -> [f32; 4] {
        let [r, g, b, _] = self.diffuse_rgba_u8();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
    }
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
    /// Compression kind: 0 = Uncompressed 32-bit BGRA8, 1 = DXT1 (BC1), 3 = DXT3 (BC2), 5 = DXT5 (BC3).
    pub kind: u8,
    /// Raw or compressed texture payload data.
    pub data: Vec<u8>,
}

impl MapTexture {
    /// Decodes the texture payload into an uncompressed RGBA8 byte buffer.
    pub fn to_rgba(&self) -> Result<Vec<u8>, AssetError> {
        if self.kind == 0 {
            let exp_len = self.width * self.height * 4;
            if self.data.len() < exp_len {
                return Err(AssetError::BufferUnderflow {
                    required: exp_len,
                    available: self.data.len(),
                });
            }
            let mut rgba = vec![0u8; exp_len];
            let (src_chunks, _) = self.data[..exp_len].as_chunks::<4>();
            let (dst_chunks, _) = rgba.as_chunks_mut::<4>();
            for (src, dst) in src_chunks.iter().zip(dst_chunks.iter_mut()) {
                dst[0] = src[2]; // R
                dst[1] = src[1]; // G
                dst[2] = src[0]; // B
                dst[3] = src[3]; // A
            }
            return Ok(rgba);
        }
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

/// Interleaved vertex format matching modern GPU vertex layout (`MapVertex`, 52 bytes).
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MapVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    /// Low byte of [`MapMaterial::mode`] (`mode & 0xff`, `sh2pc.exe` `FUN_005b2d40` `0x005b2d40` /
    /// `DAT_008f0338`) selecting the exact `VS[1..8]` (`0x008ef230..0x008efb68`) lighting equation:
    /// - `0`: unlit fullbright (`VS[1]`/`VS[3]`: `oD0 = 1.0`)
    /// - `3`: unlit vertex color (`VS[2]`/`VS[4]`: `oD0 = v5`)
    /// - `1, 2, 6`: lit diffuse + `c19` room ambient (`VS[5]`/`VS[7]`)
    /// - `4, 5, 7`: lit diffuse + `c21`/`c20` + additive baked vertex radiance `v5 * c22` (`VS[6]`/`VS[8]`)
    pub shader_mode: u32,
}

impl MapBatch {
    /// Returns the sub-room / part-group offset encoded in the upper byte of `geo_id` (`geo_id >> 8`).
    ///
    /// Verified 1:1 against `sh2pc.exe` `0x5b47a0` (`movzx ecx, byte ptr [eax + 0x2]`), where
    /// multi-room indoor `.map` containers (e.g. `ap42.map`, `rr91.map`) store `0..=3` to match
    /// `target_room_id - loaded_map_base_room_id`.
    #[inline]
    #[must_use]
    pub const fn part_group(&self) -> u8 {
        (self.geo_id >> 8) as u8
    }

    /// Returns the 1-based switchable variant index encoded in the low byte of `geo_id` (`geo_id & 0xff`).
    ///
    /// - `0`: Base static room geometry (always visible regardless of display control bitmask).
    /// - `1..=16`: Switchable variant controlled by bit `1u16 << (variant - 1)` from
    ///   `EvDispControlModelEntry` (`SLUS_202.28` `0x001ad560` / `sh2pc.exe` `0x4a2f10`).
    #[inline]
    #[must_use]
    pub const fn variant_index(&self) -> u8 {
        (self.geo_id & 0xff) as u8
    }

    /// Evaluates whether this batch is visible under the given 16-bit display control `bitmask`
    /// for its [`Self::part_group`].
    ///
    /// Exact 1:1 parity with `sh2pc.exe` `0x5b47a8..0x5b47c8`:
    /// - `variant_index == 0` is always visible.
    /// - `variant_index in 1..=16` is visible iff `(bitmask & (1u16 << (variant_index - 1))) != 0`.
    #[inline]
    #[must_use]
    pub const fn is_visible_for_mask(&self, bitmask: u16) -> bool {
        let variant = self.variant_index();
        if variant == 0 {
            return true;
        }
        let bit = variant - 1;
        if bit >= 16 {
            return false;
        }
        (bitmask & (1u16 << bit)) != 0
    }

    /// Converts parallel vertex attributes (`positions`, `normals`, `uvs`, `colors`) into interleaved [`MapVertex`] structs for direct GPU consumption.
    pub fn interleaved_vertices(&self) -> Vec<MapVertex> {
        self.interleaved_vertices_for_material(None)
    }

    /// Converts parallel vertex attributes into interleaved [`MapVertex`] structs, modulating vertex
    /// RGB by [`MapMaterial::diffuse_rgba_f32`] when `material` is untextured (`texture_id == 0`)
    /// **and** its shader mode consumes constant `c26` ([`MapMaterial::uses_diffuse_color_constant`],
    /// `mode & 0xff` in `{1, 2, 6}`).
    ///
    /// Verified 1:1 against `sh2pc.exe` `FUN_005b4940` (`0x005b4940`), `FUN_005b2190` (`0x005b2190`),
    /// `FUN_005b2d40` (`0x005b2d40`), and `VS[1..8]` (`0x008ef700..0x008efb68`), where untextured
    /// `mode 1/2/6` batches bind `NULL` (sampling white `1.0`) and modulate diffuse RGB by `c26.xyz`
    /// (`material.color` RGB / 255.0) while preserving vertex alpha (`c26.w = 1.0`). Untextured
    /// `mode 0/3/4/5/7` shaders ignore `c26` (many retail `.map` files store `color = 0xff000000`
    /// on `mode 0` fullbright and `mode 4` vertex-colored batches).
    ///
    /// Note: Textured `mode 1/2/6` materials intentionally leave `c26` unapplied here because
    /// `VS[5]` (`0x008ef230`) / `VS[7]` (`0x008efa10`) pair `mul r3.xyz, r3.xyzz, c26.xyzz` with
    /// `add oD0.xyz, r3.xyzz, r3.xyzz` (2x PS2 GS overbright), whereas `sh2-renderer`'s WGSL
    /// geometry shader does not yet implement the 2x scale; applying `0xffb2b2b2` alone would
    /// darken 2,448+ textured retail materials by ~30%.
    pub fn interleaved_vertices_for_material(
        &self,
        material: Option<&MapMaterial>,
    ) -> Vec<MapVertex> {
        let mat_rgb = material
            .filter(|m| m.is_untextured() && m.uses_diffuse_color_constant())
            .map(|m| {
                let [r, g, b, _] = m.diffuse_rgba_f32();
                [r, g, b]
            })
            .unwrap_or([1.0, 1.0, 1.0]);

        let shader_mode = material.map_or(1u32, |m| u32::from(m.mode & 0xff));
        let count = self.positions.len();
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let pos = self.positions[i];
            let norm = self.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            let uv = self.uvs.get(i).copied().unwrap_or([0.0, 0.0]);
            let c = self.colors.get(i).copied().unwrap_or([255, 255, 255, 255]);
            let col = [
                (c[0] as f32 / 255.0) * mat_rgb[0],
                (c[1] as f32 / 255.0) * mat_rgb[1],
                (c[2] as f32 / 255.0) * mat_rgb[2],
                c[3] as f32 / 255.0,
            ];
            out.push(MapVertex {
                position: pos,
                normal: norm,
                uv,
                color: col,
                shader_mode,
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

/// Tolerance around `[0.0, 1.0]` when classifying whether a `.map` texture's UVs wrap/tile.
///
/// Retail `.map` geometry (e.g. `ob01.map` car interior `mat[27..=28]`) exhibits ~`0.0004` float
/// rounding outside `[0.0, 1.0]`, whereas genuinely tiling surfaces (e.g. `er01.map` `mat[4]`
/// `U=[-0.6156, 1.4953]`, `ap01.map` `mat[9]` `U=[-0.0215, 1.0000]`) exceed `0.005`.
pub const MAP_UV_CLAMP_EPSILON: f32 = 0.005;

impl MapFile {
    /// Convenience alias to parse a map file without specifying a name.
    pub fn parse(bytes: &[u8]) -> Result<Self, AssetError> {
        parse_map(bytes, "map")
    }

    /// Returns `(wrap_u, wrap_v)` indicating whether any batch referencing `texture_id` has UVs
    /// outside `[0.0 - MAP_UV_CLAMP_EPSILON, 1.0 + MAP_UV_CLAMP_EPSILON]`.
    ///
    /// ### Parity Ground Truth (`SLUS_202.28` & `sh2pc.exe`)
    /// - `sh2pc.exe` `FUN_005b4cc0` (`0x005b4cc0`, `sh2pc_005b0000.c:2330-2332`) and `0x005b1694`
    ///   (`sh2pc_005a0000.c:7579-7581`) set `D3DTSS_ADDRESSU (0xd) = 3 (D3DTADDRESS_CLAMP)` and
    ///   `D3DTSS_ADDRESSV (0xe) = 3 (D3DTADDRESS_CLAMP)` for `.map` geometry, preventing border
    ///   wrap-around bleed on `[0, 1]`-mapped alpha-cutout atlases (such as `ob01.map` pine trees
    ///   `tid=4650` and `tid=4660`).
    /// - `SLUS_202.28` `shGs_InitDefaultRegsEnv` (`0x001909e0`, `shGs/sh2gfw_GsInit.c:658-659`)
    ///   sets `CLAMP_1 (0x08) = 0` (`WMS = 0 [REPEAT]`, `WMT = 0 [REPEAT]`), which tiling
    ///   surfaces with UVs outside `[0, 1]` rely on.
    #[must_use]
    pub fn texture_uv_wrap_axes(&self, texture_id: u32) -> (bool, bool) {
        if texture_id == 0 {
            return (false, false);
        }
        let lo = -MAP_UV_CLAMP_EPSILON;
        let hi = 1.0 + MAP_UV_CLAMP_EPSILON;
        let mut wrap_u = false;
        let mut wrap_v = false;

        for batch in &self.batches {
            let Some(mat) = self.materials.get(batch.material_index) else {
                continue;
            };
            if u32::from(mat.texture_id) != texture_id {
                continue;
            }
            for &[u, v] in &batch.uvs {
                if u < lo || u > hi {
                    wrap_u = true;
                }
                if v < lo || v > hi {
                    wrap_v = true;
                }
                if wrap_u && wrap_v {
                    return (true, true);
                }
            }
        }

        (wrap_u, wrap_v)
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
                // Exact `sh2pc.exe` `FUN_005b4940` (`0x005b4940`) 8-byte part entry layout:
                // - `+0..+2`: `u16` base count (`*puVar8`)
                // - `+2`: `u8` primitive-type index into `DAT_008f0378`
                //   (`0 => D3DPT_TRIANGLESTRIP [5]`, `1 => D3DPT_TRIANGLELIST [4]`)
                // - `+3`: `u8` index multiplier (`1` for strip, `3` for list)
                let strip_len = read_u16(data, p)? as usize;
                let prim_type_idx = data[p + 2];
                let index_mult = data[p + 3] as usize;
                p += 8;

                let (n, prim) = match prim_type_idx {
                    1 => (strip_len * index_mult.max(3), 3u8),
                    _ => (strip_len * index_mult.max(1), 1u8),
                };

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
    while data.len().saturating_sub(p) >= 16 {
        let tex_id = read_u32(data, p)?;
        if tex_id == 0 {
            p += 16;
            continue;
        }

        if data.len().saturating_sub(p) < 32 {
            break;
        }

        let sprite_count = read_u32(data, p + 12)? as usize;
        p += 32;

        for _ in 0..sprite_count {
            if data.len().saturating_sub(p) < 32 {
                return Ok(());
            }

            let w = read_u16(data, p + 8)? as usize;
            let h = read_u16(data, p + 10)? as usize;
            let fmt = data[p + 12];
            let comp = data[p + 13];
            let length = read_u32(data, p + 16)? as usize;
            p += 32;

            if w > 0 && h > 0 && length != 0 && p + length <= data.len() {
                let kind = if comp == 0 {
                    0 // Uncompressed 32-bit BGRA8
                } else {
                    match fmt {
                        0 => 1,     // BC1 / DXT1
                        1 | 2 => 3, // BC2 / DXT3
                        3 | 4 => 5, // BC3 / DXT5
                        _ => 3,
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_material_d3dcolor_unpacking_matches_sh2pc_005b2190() {
        // Retail `data/bg/ob/ob01.map` mat[2] / mat[11]: untextured, color = 0xffb2b2b2
        let mat_light_grey = MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0xffb2_b2b2,
            overlay: 0xff00_0000,
            specularity: 0.0,
        };
        assert!(mat_light_grey.is_untextured());
        assert_eq!(mat_light_grey.diffuse_rgba_u8(), [0xb2, 0xb2, 0xb2, 0xff]);
        let f = mat_light_grey.diffuse_rgba_f32();
        assert!((f[0] - (178.0 / 255.0)).abs() < 1e-6);
        assert!((f[1] - (178.0 / 255.0)).abs() < 1e-6);
        assert!((f[2] - (178.0 / 255.0)).abs() < 1e-6);
        assert_eq!(f[3], 1.0);

        // Retail `data/bg/ob/ob01.map` mat[22]: untextured, color = 0xff7f7f7f
        let mat_mid_grey = MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0xff7f_7f7f,
            overlay: 0xff00_0000,
            specularity: 0.0,
        };
        assert!(mat_mid_grey.is_untextured());
        assert_eq!(mat_mid_grey.diffuse_rgba_u8(), [0x7f, 0x7f, 0x7f, 0xff]);

        // Asymmetric channel order check (`0xAARRGGBB` -> R=0xaa, G=0x55, B=0x11, A forced to 1.0
        // per `sh2pc.exe` `FUN_005b2190` `0x005b2190`: `_DAT_01db888c = 0x3f800000`)
        let mat_asym = MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0x33aa_5511,
            overlay: 0,
            specularity: 0.0,
        };
        assert_eq!(mat_asym.diffuse_rgba_u8(), [0xaa, 0x55, 0x11, 0xff]);
        let fa = mat_asym.diffuse_rgba_f32();
        assert!((fa[0] - (0xaa as f32 / 255.0)).abs() < 1e-6);
        assert!((fa[1] - (0x55 as f32 / 255.0)).abs() < 1e-6);
        assert!((fa[2] - (0x11 as f32 / 255.0)).abs() < 1e-6);
        assert_eq!(fa[3], 1.0);
    }

    #[test]
    fn test_interleaved_vertices_modulates_untextured_material_color_only() {
        let batch = MapBatch {
            group: BatchGroup::Opaque,
            material_index: 0,
            geo_id: 0,
            positions: vec![[0.0, 0.0, 0.0]],
            uvs: vec![[0.5, 0.5]],
            colors: vec![[255, 255, 255, 128]],
            normals: vec![[0.0, 1.0, 0.0]],
            indices: vec![0],
        };

        let untextured = MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0xff80_4020,
            overlay: 0,
            specularity: 0.0,
        };
        let verts_untex = batch.interleaved_vertices_for_material(Some(&untextured));
        assert!((verts_untex[0].color[0] - (0x80 as f32 / 255.0)).abs() < 1e-6);
        assert!((verts_untex[0].color[1] - (0x40 as f32 / 255.0)).abs() < 1e-6);
        assert!((verts_untex[0].color[2] - (0x20 as f32 / 255.0)).abs() < 1e-6);
        // Vertex alpha must be preserved (`c26.w = 1.0`).
        assert!((verts_untex[0].color[3] - (128.0 / 255.0)).abs() < 1e-6);

        let textured = MapMaterial {
            mode: 1,
            texture_id: 4654,
            color: 0xff80_4020,
            overlay: 0,
            specularity: 0.0,
        };
        let verts_tex = batch.interleaved_vertices_for_material(Some(&textured));
        assert_eq!(verts_tex[0].color[0], 1.0);
        assert_eq!(verts_tex[0].color[1], 1.0);
        assert_eq!(verts_tex[0].color[2], 1.0);
        assert!((verts_tex[0].color[3] - (128.0 / 255.0)).abs() < 1e-6);
    }

    #[test]
    fn test_interleaved_vertices_respects_shader_mode_c26_gating() {
        let batch = MapBatch {
            group: BatchGroup::Opaque,
            material_index: 0,
            geo_id: 0,
            positions: vec![[0.0, 0.0, 0.0]],
            uvs: vec![[0.5, 0.5]],
            colors: vec![[200, 150, 100, 128]],
            normals: vec![[0.0, 1.0, 0.0]],
            indices: vec![0],
        };

        // Modes 0, 3, 4, 5, 7 ignore `c26` in `sh2pc.exe` (`VS[1..4, 6, 8]`).
        // Even with `color == 0xff000000` (seen on 19 retail `data/bg/` untextured batches),
        // vertex RGB and alpha must remain untouched.
        for mode in [0u16, 3, 4, 5, 7] {
            let mat = MapMaterial {
                mode,
                texture_id: 0,
                color: 0xff00_0000,
                overlay: 0,
                specularity: 0.0,
            };
            assert!(
                !mat.uses_diffuse_color_constant(),
                "mode {mode} must not consume c26"
            );
            let verts = batch.interleaved_vertices_for_material(Some(&mat));
            assert!((verts[0].color[0] - (200.0 / 255.0)).abs() < 1e-6);
            assert!((verts[0].color[1] - (150.0 / 255.0)).abs() < 1e-6);
            assert!((verts[0].color[2] - (100.0 / 255.0)).abs() < 1e-6);
            assert!((verts[0].color[3] - (128.0 / 255.0)).abs() < 1e-6);
        }

        // Modes 1, 2, 6 consume `c26` (`VS[5]` `0x008ef230` / `VS[7]` `0x008efa10`), modulating
        // vertex RGB by `diffuse_rgba_f32()` while preserving vertex alpha.
        for mode in [1u16, 2, 6] {
            let mat = MapMaterial {
                mode,
                texture_id: 0,
                color: 0xff80_4020,
                overlay: 0,
                specularity: 0.0,
            };
            assert!(
                mat.uses_diffuse_color_constant(),
                "mode {mode} must consume c26"
            );
            let verts = batch.interleaved_vertices_for_material(Some(&mat));
            assert!((verts[0].color[0] - (200.0 / 255.0) * (0x80 as f32 / 255.0)).abs() < 1e-6);
            assert!((verts[0].color[1] - (150.0 / 255.0) * (0x40 as f32 / 255.0)).abs() < 1e-6);
            assert!((verts[0].color[2] - (100.0 / 255.0) * (0x20 as f32 / 255.0)).abs() < 1e-6);
            assert!((verts[0].color[3] - (128.0 / 255.0)).abs() < 1e-6);
        }
    }

    #[test]
    fn test_texture_uv_wrap_axes_clamps_within_bounds_and_wraps_tiling_axes() {
        let mut map = MapFile::default();
        for tid in [4650u16, 4660, 3336, 1864] {
            map.materials.push(MapMaterial {
                mode: 1,
                texture_id: tid,
                color: 0xffff_ffff,
                overlay: 0,
                specularity: 0.0,
            });
        }
        // mat[0] (tid=4650): strictly within [0, 1] + tiny float rounding (-0.0004..=1.0004)
        map.batches.push(MapBatch {
            group: BatchGroup::Decal,
            material_index: 0,
            geo_id: 0,
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[-0.0004, 0.0003], [1.0004, 0.0], [0.5, 0.999]],
            colors: vec![[255; 4]; 3],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        });
        // mat[2] (tid=3336): wraps horizontally (U=[-0.6156, 1.4953]), clamped vertically
        map.batches.push(MapBatch {
            group: BatchGroup::Opaque,
            material_index: 2,
            geo_id: 0,
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[-0.6156, 0.0], [1.4953, 1.0], [0.5, 0.5]],
            colors: vec![[255; 4]; 3],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        });
        // mat[3] (tid=1864): wraps vertically only
        map.batches.push(MapBatch {
            group: BatchGroup::Opaque,
            material_index: 3,
            geo_id: 0,
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 2.0], [0.5, 0.5]],
            colors: vec![[255; 4]; 3],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        });

        assert_eq!(map.texture_uv_wrap_axes(0), (false, false));
        assert_eq!(map.texture_uv_wrap_axes(4650), (false, false));
        assert_eq!(map.texture_uv_wrap_axes(4660), (false, false));
        assert_eq!(map.texture_uv_wrap_axes(3336), (true, false));
        assert_eq!(map.texture_uv_wrap_axes(1864), (false, true));
    }
}
