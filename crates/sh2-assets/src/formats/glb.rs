//! Minimal, safe glTF 2.0 Binary (`.glb`) parser for optional SH2:EE 3D replacement models
//! (`sh2e/model/ty3.glb` for `CockroachesReplacement.cpp`, `sh2e/model/mon.glb` for
//! `DogRoomEnhanced.cpp`, and `sh2e/model/dmr.glb` for `MariaCutsceneReplacement.cpp`).
//!
//! Mirrors the CPU-side data model of SH2:EE's `Common/ModelGLTF.h` / `Common/ModelGLTF.cpp`:
//! - Triangle-list primitives with `POSITION`, `NORMAL`, `TEXCOORD_0`, optional `JOINTS_0`/`WEIGHTS_0`
//! - Scene node hierarchy with TRS/matrix transforms
//! - Skinning (`joints` + `inverseBindMatrices`)
//! - Keyframe animation tracks (`translation`, `rotation`, `scale`) with looping playback
//! - Embedded PNG textures decoded to RGBA8.

use super::binary::{read_f32, read_u16, read_u32};
use crate::error::AssetError;
use serde::Deserialize;
use std::collections::HashMap;

/// glTF 2.0 binary magic (`"glTF"` = `0x46546C67` little-endian).
pub const GLB_MAGIC: [u8; 4] = *b"glTF";
/// glTF 2.0 binary version.
pub const GLB_VERSION: u32 = 2;
/// GLB JSON chunk type (`"JSON"` = `0x4E4F534A`).
pub const GLB_CHUNK_JSON: u32 = 0x4E4F_534A;
/// GLB BIN chunk type (`"BIN\0"` = `0x004E4942`).
pub const GLB_CHUNK_BIN: u32 = 0x004E_4942;

/// Single vertex of a parsed `.glb` primitive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbVertex {
    /// Local-space position $(x, y, z)$.
    pub position: [f32; 3],
    /// Local-space unit normal $(n_x, n_y, n_z)$.
    pub normal: [f32; 3],
    /// Texture coordinates $(u, v)$.
    pub uv: [f32; 2],
    /// Up to 4 joint indices into the primitive's skin.
    pub joints: [u8; 4],
    /// Normalized joint blend weights.
    pub weights: [f32; 4],
}

/// Drawable primitive section within a [`GlbMesh`].
#[derive(Debug, Clone)]
pub struct GlbPrimitive {
    /// Interleaved vertices.
    pub vertices: Vec<GlbVertex>,
    /// Triangle-list indices (`u16`).
    pub indices: Vec<u16>,
    /// Optional index into [`GlbModel::textures`].
    pub texture_index: Option<usize>,
    /// True when the material uses `alphaMode = "BLEND"`.
    pub alpha_blend: bool,
}

/// Mesh containing one or more [`GlbPrimitive`] sections.
#[derive(Debug, Clone)]
pub struct GlbMesh {
    /// Primitive draw sections.
    pub primitives: Vec<GlbPrimitive>,
}

/// Scene node in the `.glb` hierarchy.
#[derive(Debug, Clone)]
pub struct GlbNode {
    /// Optional index into [`GlbModel::meshes`].
    pub mesh_index: Option<usize>,
    /// Optional index into [`GlbModel::skins`].
    pub skin_index: Option<usize>,
    /// Local translation $(x, y, z)$.
    pub translation: [f32; 3],
    /// Local quaternion rotation $(x, y, z, w)$.
    pub rotation: [f32; 4],
    /// Local scale $(s_x, s_y, s_z)$.
    pub scale: [f32; 3],
    /// Child node indices.
    pub children: Vec<usize>,
}

/// Skeletal skinning binding.
#[derive(Debug, Clone)]
pub struct GlbSkin {
    /// Node indices for each joint.
    pub joints: Vec<usize>,
    /// Column-major 4x4 inverse bind matrices per joint.
    pub inverse_bind_matrices: Vec<[f32; 16]>,
}

/// Keyframe channel targeting a single scene node.
#[derive(Debug, Clone)]
pub struct GlbAnimTrack {
    /// Target node index in [`GlbModel::nodes`].
    pub target_node: usize,
    /// Translation keyframes `(time_sec, [x, y, z])`.
    pub translations: Vec<(f32, [f32; 3])>,
    /// Rotation keyframes `(time_sec, [x, y, z, w])`.
    pub rotations: Vec<(f32, [f32; 4])>,
    /// Scale keyframes `(time_sec, [sx, sy, sz])`.
    pub scales: Vec<(f32, [f32; 3])>,
}

/// Decoded embedded texture from a `.glb` file.
#[derive(Debug, Clone)]
pub struct GlbTexture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA8 pixel payload (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

/// Parsed glTF 2.0 binary model (`ty3.glb`, `mon.glb`, `dmr.glb`).
#[derive(Debug, Clone)]
pub struct GlbModel {
    /// Parsed meshes.
    pub meshes: Vec<GlbMesh>,
    /// Scene nodes.
    pub nodes: Vec<GlbNode>,
    /// Root node indices of the default scene.
    pub root_nodes: Vec<usize>,
    /// Skins.
    pub skins: Vec<GlbSkin>,
    /// Animation tracks from the first animation clip.
    pub anim_tracks: Vec<GlbAnimTrack>,
    /// Total animation duration in seconds.
    pub anim_duration_sec: f32,
    /// Embedded RGBA textures.
    pub textures: Vec<GlbTexture>,
}

impl GlbModel {
    /// Evaluates local TRS transforms for every node at `time_sec` (wrapping if `loop_anim`).
    #[must_use]
    pub fn evaluate_node_locals(
        &self,
        time_sec: f32,
        loop_anim: bool,
    ) -> Vec<([f32; 3], [f32; 4], [f32; 3])> {
        let t = if self.anim_duration_sec > 0.0 {
            if loop_anim {
                time_sec.rem_euclid(self.anim_duration_sec)
            } else {
                time_sec.clamp(0.0, self.anim_duration_sec)
            }
        } else {
            0.0
        };

        let mut locals: Vec<([f32; 3], [f32; 4], [f32; 3])> = self
            .nodes
            .iter()
            .map(|n| (n.translation, n.rotation, n.scale))
            .collect();

        for track in &self.anim_tracks {
            if let Some(entry) = locals.get_mut(track.target_node) {
                if !track.translations.is_empty() {
                    entry.0 = sample_vec3(&track.translations, t);
                }
                if !track.rotations.is_empty() {
                    entry.1 = sample_quat(&track.rotations, t);
                }
                if !track.scales.is_empty() {
                    entry.2 = sample_vec3(&track.scales, t);
                }
            }
        }
        locals
    }
}

#[derive(Deserialize)]
struct RawGltf {
    #[serde(default)]
    scene: Option<usize>,
    #[serde(default)]
    scenes: Vec<RawScene>,
    #[serde(default)]
    nodes: Vec<RawNode>,
    #[serde(default)]
    meshes: Vec<RawMesh>,
    #[serde(default)]
    materials: Vec<RawMaterial>,
    #[serde(default)]
    textures: Vec<RawTextureRef>,
    #[serde(default)]
    images: Vec<RawImage>,
    #[serde(default)]
    skins: Vec<RawSkin>,
    #[serde(default)]
    animations: Vec<RawAnimation>,
    #[serde(default)]
    accessors: Vec<RawAccessor>,
    #[serde(default, rename = "bufferViews")]
    buffer_views: Vec<RawBufferView>,
}

#[derive(Deserialize)]
struct RawScene {
    #[serde(default)]
    nodes: Vec<usize>,
}

#[derive(Deserialize)]
struct RawNode {
    mesh: Option<usize>,
    skin: Option<usize>,
    translation: Option<[f32; 3]>,
    rotation: Option<[f32; 4]>,
    scale: Option<[f32; 3]>,
    #[serde(default)]
    children: Vec<usize>,
}

#[derive(Deserialize)]
struct RawMesh {
    #[serde(default)]
    primitives: Vec<RawPrimitive>,
}

#[derive(Deserialize)]
struct RawPrimitive {
    attributes: HashMap<String, usize>,
    indices: Option<usize>,
    material: Option<usize>,
}

#[derive(Deserialize)]
struct RawMaterial {
    #[serde(rename = "alphaMode")]
    alpha_mode: Option<String>,
    #[serde(rename = "pbrMetallicRoughness")]
    pbr: Option<RawPbr>,
}

#[derive(Deserialize)]
struct RawPbr {
    #[serde(rename = "baseColorTexture")]
    base_color_texture: Option<RawTexInfo>,
}

#[derive(Deserialize)]
struct RawTexInfo {
    index: usize,
}

#[derive(Deserialize)]
struct RawTextureRef {
    source: Option<usize>,
}

#[derive(Deserialize)]
struct RawImage {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
}

#[derive(Deserialize)]
struct RawSkin {
    #[serde(default)]
    joints: Vec<usize>,
    #[serde(rename = "inverseBindMatrices")]
    inverse_bind_matrices: Option<usize>,
}

#[derive(Deserialize)]
struct RawAnimation {
    #[serde(default)]
    channels: Vec<RawAnimChannel>,
    #[serde(default)]
    samplers: Vec<RawAnimSampler>,
}

#[derive(Deserialize)]
struct RawAnimChannel {
    sampler: usize,
    target: RawAnimTarget,
}

#[derive(Deserialize)]
struct RawAnimTarget {
    node: Option<usize>,
    path: String,
}

#[derive(Deserialize)]
struct RawAnimSampler {
    input: usize,
    output: usize,
}

#[derive(Deserialize)]
struct RawAccessor {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct RawBufferView {
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    #[serde(rename = "byteStride")]
    byte_stride: Option<usize>,
}

/// Parses a glTF 2.0 binary (`.glb`) buffer into a [`GlbModel`].
pub fn parse_glb(data: &[u8]) -> Result<GlbModel, AssetError> {
    if data.len() < 12 {
        return Err(AssetError::BufferUnderflow {
            required: 12,
            available: data.len(),
        });
    }
    if data[0..4] != GLB_MAGIC {
        return Err(AssetError::InvalidMagic {
            expected: &GLB_MAGIC,
            found: data[0..4].to_vec(),
        });
    }
    let version = read_u32(data, 4)?;
    if version != GLB_VERSION {
        return Err(AssetError::UnsupportedFormat(format!(
            "Unsupported GLB version {version}"
        )));
    }
    let total_len = read_u32(data, 8)? as usize;
    if total_len > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: total_len,
            available: data.len(),
        });
    }

    let mut offset = 12usize;
    let mut json_bytes: Option<&[u8]> = None;
    let mut bin_bytes: &[u8] = &[];

    while offset + 8 <= total_len {
        let chunk_len = read_u32(data, offset)? as usize;
        let chunk_type = read_u32(data, offset + 4)?;
        let payload_start = offset + 8;
        let payload_end = payload_start
            .checked_add(chunk_len)
            .filter(|&end| end <= total_len)
            .ok_or(AssetError::BufferUnderflow {
                required: payload_start.saturating_add(chunk_len),
                available: total_len,
            })?;
        match chunk_type {
            GLB_CHUNK_JSON if json_bytes.is_none() => {
                json_bytes = Some(&data[payload_start..payload_end]);
            }
            GLB_CHUNK_BIN if bin_bytes.is_empty() => {
                bin_bytes = &data[payload_start..payload_end];
            }
            _ => {}
        }
        offset = payload_end;
    }

    let json_slice = json_bytes.ok_or(AssetError::MalformedHeader("Missing GLB JSON chunk"))?;
    let raw: RawGltf = serde_json::from_slice(json_slice)
        .map_err(|e| AssetError::InvalidData(format!("Invalid GLB JSON: {e}")))?;

    let textures = parse_textures(&raw, bin_bytes)?;
    let meshes = parse_meshes(&raw, bin_bytes)?;
    let nodes = raw
        .nodes
        .iter()
        .map(|n| GlbNode {
            mesh_index: n.mesh,
            skin_index: n.skin,
            translation: n.translation.unwrap_or([0.0, 0.0, 0.0]),
            rotation: n.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]),
            scale: n.scale.unwrap_or([1.0, 1.0, 1.0]),
            children: n.children.clone(),
        })
        .collect();
    let root_nodes = raw
        .scene
        .and_then(|idx| raw.scenes.get(idx))
        .or_else(|| raw.scenes.first())
        .map(|s| s.nodes.clone())
        .unwrap_or_default();
    let skins = parse_skins(&raw, bin_bytes)?;
    let (anim_tracks, anim_duration_sec) = parse_animations(&raw, bin_bytes)?;

    Ok(GlbModel {
        meshes,
        nodes,
        root_nodes,
        skins,
        anim_tracks,
        anim_duration_sec,
        textures,
    })
}

fn accessor_slice<'r, 'b>(
    raw: &'r RawGltf,
    bin: &'b [u8],
    acc_idx: usize,
) -> Result<(&'b [u8], usize, &'r RawAccessor), AssetError> {
    let acc = raw
        .accessors
        .get(acc_idx)
        .ok_or_else(|| AssetError::InvalidData(format!("Missing accessor {acc_idx}")))?;
    let bv_idx = acc
        .buffer_view
        .ok_or_else(|| AssetError::InvalidData(format!("Accessor {acc_idx} has no bufferView")))?;
    let bv = raw
        .buffer_views
        .get(bv_idx)
        .ok_or_else(|| AssetError::InvalidData(format!("Missing bufferView {bv_idx}")))?;
    let start = bv
        .byte_offset
        .checked_add(acc.byte_offset)
        .ok_or(AssetError::MalformedHeader("BufferView offset overflow"))?;
    let end = bv
        .byte_offset
        .checked_add(bv.byte_length)
        .filter(|&e| e <= bin.len() && start <= e)
        .ok_or(AssetError::BufferUnderflow {
            required: bv.byte_offset.saturating_add(bv.byte_length),
            available: bin.len(),
        })?;
    let elem_size = component_byte_size(acc.component_type)? * type_component_count(&acc.kind)?;
    let stride = bv.byte_stride.unwrap_or(elem_size);
    if stride < elem_size {
        return Err(AssetError::MalformedHeader("Invalid bufferView stride"));
    }
    Ok((&bin[start..end], stride, acc))
}

fn component_byte_size(component_type: u32) -> Result<usize, AssetError> {
    match component_type {
        5120 | 5121 => Ok(1), // BYTE / UNSIGNED_BYTE
        5122 | 5123 => Ok(2), // SHORT / UNSIGNED_SHORT
        5125 | 5126 => Ok(4), // UNSIGNED_INT / FLOAT
        other => Err(AssetError::UnsupportedFormat(format!(
            "Unsupported glTF componentType {other}"
        ))),
    }
}

fn type_component_count(kind: &str) -> Result<usize, AssetError> {
    match kind {
        "SCALAR" => Ok(1),
        "VEC2" => Ok(2),
        "VEC3" => Ok(3),
        "VEC4" => Ok(4),
        "MAT4" => Ok(16),
        other => Err(AssetError::UnsupportedFormat(format!(
            "Unsupported glTF accessor type {other}"
        ))),
    }
}

fn read_f32_checked(slice: &[u8], offset: usize) -> Result<f32, AssetError> {
    if offset + 4 > slice.len() {
        return Err(AssetError::BufferUnderflow {
            required: offset + 4,
            available: slice.len(),
        });
    }
    read_f32(slice, offset)
}

fn parse_vec3_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 3]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "VEC3" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT VEC3 accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        out.push([
            read_f32_checked(slice, base)?,
            read_f32_checked(slice, base + 4)?,
            read_f32_checked(slice, base + 8)?,
        ]);
    }
    Ok(out)
}

fn parse_vec2_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 2]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "VEC2" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT VEC2 accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        out.push([
            read_f32_checked(slice, base)?,
            read_f32_checked(slice, base + 4)?,
        ]);
    }
    Ok(out)
}

fn parse_vec4_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 4]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "VEC4" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT VEC4 accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        out.push([
            read_f32_checked(slice, base)?,
            read_f32_checked(slice, base + 4)?,
            read_f32_checked(slice, base + 8)?,
            read_f32_checked(slice, base + 12)?,
        ]);
    }
    Ok(out)
}

fn parse_joints_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[u8; 4]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        match acc.component_type {
            5121 => {
                if base + 4 > slice.len() {
                    return Err(AssetError::BufferUnderflow {
                        required: base + 4,
                        available: slice.len(),
                    });
                }
                out.push([
                    slice[base],
                    slice[base + 1],
                    slice[base + 2],
                    slice[base + 3],
                ]);
            }
            5123 => {
                if base + 8 > slice.len() {
                    return Err(AssetError::BufferUnderflow {
                        required: base + 8,
                        available: slice.len(),
                    });
                }
                out.push([
                    read_u16(slice, base)? as u8,
                    read_u16(slice, base + 2)? as u8,
                    read_u16(slice, base + 4)? as u8,
                    read_u16(slice, base + 6)? as u8,
                ]);
            }
            other => {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported JOINTS_0 componentType {other}"
                )));
            }
        }
    }
    Ok(out)
}

fn parse_indices_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<u16>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        match acc.component_type {
            5121 => {
                if base >= slice.len() {
                    return Err(AssetError::BufferUnderflow {
                        required: base + 1,
                        available: slice.len(),
                    });
                }
                out.push(u16::from(slice[base]));
            }
            5123 => out.push(read_u16(slice, base)?),
            5125 => {
                let v = read_u32(slice, base)?;
                let idx = u16::try_from(v).map_err(|_| {
                    AssetError::UnsupportedFormat(format!("Index {v} exceeds u16::MAX"))
                })?;
                out.push(idx);
            }
            other => {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported index componentType {other}"
                )));
            }
        }
    }
    Ok(out)
}

fn parse_textures(raw: &RawGltf, bin: &[u8]) -> Result<Vec<GlbTexture>, AssetError> {
    let mut out = Vec::with_capacity(raw.images.len());
    for img in &raw.images {
        let Some(bv_idx) = img.buffer_view else {
            continue;
        };
        let bv = raw
            .buffer_views
            .get(bv_idx)
            .ok_or_else(|| AssetError::InvalidData(format!("Missing image bufferView {bv_idx}")))?;
        let end = bv
            .byte_offset
            .checked_add(bv.byte_length)
            .filter(|&e| e <= bin.len())
            .ok_or(AssetError::BufferUnderflow {
                required: bv.byte_offset.saturating_add(bv.byte_length),
                available: bin.len(),
            })?;
        let decoded = image::load_from_memory(&bin[bv.byte_offset..end])
            .map_err(|e| AssetError::ImageError(e.to_string()))?;
        let rgba8 = decoded.to_rgba8();
        out.push(GlbTexture {
            width: rgba8.width(),
            height: rgba8.height(),
            rgba: rgba8.into_raw(),
        });
    }
    Ok(out)
}

fn parse_meshes(raw: &RawGltf, bin: &[u8]) -> Result<Vec<GlbMesh>, AssetError> {
    let mut meshes = Vec::with_capacity(raw.meshes.len());
    for m in &raw.meshes {
        let mut primitives = Vec::with_capacity(m.primitives.len());
        for p in &m.primitives {
            let pos_acc =
                p.attributes.get("POSITION").copied().ok_or_else(|| {
                    AssetError::InvalidData("GLB primitive missing POSITION".into())
                })?;
            let positions = parse_vec3_accessor(raw, bin, pos_acc)?;
            let normals = match p.attributes.get("NORMAL").copied() {
                Some(idx) => parse_vec3_accessor(raw, bin, idx)?,
                None => vec![[0.0, 1.0, 0.0]; positions.len()],
            };
            let uvs = match p.attributes.get("TEXCOORD_0").copied() {
                Some(idx) => parse_vec2_accessor(raw, bin, idx)?,
                None => vec![[0.0, 0.0]; positions.len()],
            };
            let joints = match p.attributes.get("JOINTS_0").copied() {
                Some(idx) => parse_joints_accessor(raw, bin, idx)?,
                None => vec![[0, 0, 0, 0]; positions.len()],
            };
            let weights = match p.attributes.get("WEIGHTS_0").copied() {
                Some(idx) => parse_vec4_accessor(raw, bin, idx)?,
                None => vec![[1.0, 0.0, 0.0, 0.0]; positions.len()],
            };

            let mut vertices = Vec::with_capacity(positions.len());
            for (i, &position) in positions.iter().enumerate() {
                vertices.push(GlbVertex {
                    position,
                    normal: *normals.get(i).unwrap_or(&[0.0, 1.0, 0.0]),
                    uv: *uvs.get(i).unwrap_or(&[0.0, 0.0]),
                    joints: *joints.get(i).unwrap_or(&[0, 0, 0, 0]),
                    weights: *weights.get(i).unwrap_or(&[1.0, 0.0, 0.0, 0.0]),
                });
            }

            let indices = match p.indices {
                Some(idx) => parse_indices_accessor(raw, bin, idx)?,
                None => (0..vertices.len() as u16).collect(),
            };

            let (texture_index, alpha_blend) =
                match p.material.and_then(|idx| raw.materials.get(idx)) {
                    Some(mat) => {
                        let tex_idx = mat
                            .pbr
                            .as_ref()
                            .and_then(|pbr| pbr.base_color_texture.as_ref())
                            .and_then(|info| raw.textures.get(info.index))
                            .and_then(|t| t.source);
                        let blend = mat.alpha_mode.as_deref() == Some("BLEND");
                        (tex_idx, blend)
                    }
                    None => (None, false),
                };

            primitives.push(GlbPrimitive {
                vertices,
                indices,
                texture_index,
                alpha_blend,
            });
        }
        meshes.push(GlbMesh { primitives });
    }
    Ok(meshes)
}

fn parse_skins(raw: &RawGltf, bin: &[u8]) -> Result<Vec<GlbSkin>, AssetError> {
    let mut skins = Vec::with_capacity(raw.skins.len());
    for s in &raw.skins {
        let inverse_bind_matrices = match s.inverse_bind_matrices {
            Some(acc_idx) => {
                let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
                if acc.component_type != 5126 || acc.kind != "MAT4" {
                    return Err(AssetError::UnsupportedFormat(
                        "Expected FLOAT MAT4 inverseBindMatrices".into(),
                    ));
                }
                let mut mats = Vec::with_capacity(acc.count);
                for i in 0..acc.count {
                    let base = i * stride;
                    let mut m = [0.0f32; 16];
                    for (j, slot) in m.iter_mut().enumerate() {
                        *slot = read_f32_checked(slice, base + j * 4)?;
                    }
                    mats.push(m);
                }
                mats
            }
            None => Vec::new(),
        };
        skins.push(GlbSkin {
            joints: s.joints.clone(),
            inverse_bind_matrices,
        });
    }
    Ok(skins)
}

fn parse_animations(raw: &RawGltf, bin: &[u8]) -> Result<(Vec<GlbAnimTrack>, f32), AssetError> {
    let Some(anim) = raw.animations.first() else {
        return Ok((Vec::new(), 0.0));
    };
    let mut by_node: HashMap<usize, GlbAnimTrack> = HashMap::new();
    let mut max_time = 0.0f32;

    for ch in &anim.channels {
        let Some(node_idx) = ch.target.node else {
            continue;
        };
        let sampler = anim
            .samplers
            .get(ch.sampler)
            .ok_or_else(|| AssetError::InvalidData("Missing animation sampler".into()))?;
        let (t_slice, t_stride, t_acc) = accessor_slice(raw, bin, sampler.input)?;
        if t_acc.component_type != 5126 || t_acc.kind != "SCALAR" {
            continue;
        }
        let mut times = Vec::with_capacity(t_acc.count);
        for i in 0..t_acc.count {
            let t = read_f32_checked(t_slice, i * t_stride)?;
            max_time = max_time.max(t);
            times.push(t);
        }

        let track = by_node.entry(node_idx).or_insert_with(|| GlbAnimTrack {
            target_node: node_idx,
            translations: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
        });

        match ch.target.path.as_str() {
            "translation" => {
                let vals = parse_vec3_accessor(raw, bin, sampler.output)?;
                track.translations = times.into_iter().zip(vals).collect();
            }
            "rotation" => {
                let vals = parse_vec4_accessor(raw, bin, sampler.output)?;
                track.rotations = times.into_iter().zip(vals).collect();
            }
            "scale" => {
                let vals = parse_vec3_accessor(raw, bin, sampler.output)?;
                track.scales = times.into_iter().zip(vals).collect();
            }
            _ => {}
        }
    }

    let mut tracks: Vec<GlbAnimTrack> = by_node.into_values().collect();
    tracks.sort_by_key(|t| t.target_node);
    Ok((tracks, max_time))
}

fn sample_vec3(frames: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    if frames.len() == 1 || t <= frames[0].0 {
        return frames[0].1;
    }
    for w in frames.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t <= t1 {
            let span = (t1 - t0).max(f32::EPSILON);
            let a = ((t - t0) / span).clamp(0.0, 1.0);
            return [
                v0[0] + (v1[0] - v0[0]) * a,
                v0[1] + (v1[1] - v0[1]) * a,
                v0[2] + (v1[2] - v0[2]) * a,
            ];
        }
    }
    frames.last().map(|f| f.1).unwrap_or([0.0, 0.0, 0.0])
}

fn sample_quat(frames: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
    if frames.len() == 1 || t <= frames[0].0 {
        return frames[0].1;
    }
    for w in frames.windows(2) {
        let (t0, q0) = w[0];
        let (t1, mut q1) = w[1];
        if t <= t1 {
            let span = (t1 - t0).max(f32::EPSILON);
            let a = ((t - t0) / span).clamp(0.0, 1.0);
            let mut dot = q0[0] * q1[0] + q0[1] * q1[1] + q0[2] * q1[2] + q0[3] * q1[3];
            if dot < 0.0 {
                q1 = [-q1[0], -q1[1], -q1[2], -q1[3]];
                dot = -dot;
            }
            let out = if dot > 0.9995 {
                [
                    q0[0] + (q1[0] - q0[0]) * a,
                    q0[1] + (q1[1] - q0[1]) * a,
                    q0[2] + (q1[2] - q0[2]) * a,
                    q0[3] + (q1[3] - q0[3]) * a,
                ]
            } else {
                let theta0 = dot.clamp(-1.0, 1.0).acos();
                let theta = theta0 * a;
                let s0 = (theta0 - theta).sin() / theta0.sin();
                let s1 = theta.sin() / theta0.sin();
                [
                    q0[0] * s0 + q1[0] * s1,
                    q0[1] * s0 + q1[1] * s1,
                    q0[2] * s0 + q1[2] * s1,
                    q0[3] * s0 + q1[3] * s1,
                ]
            };
            let len =
                (out[0] * out[0] + out[1] * out[1] + out[2] * out[2] + out[3] * out[3]).sqrt();
            if len > f32::EPSILON {
                return [out[0] / len, out[1] / len, out[2] / len, out[3] / len];
            }
            return [0.0, 0.0, 0.0, 1.0];
        }
    }
    frames.last().map(|f| f.1).unwrap_or([0.0, 0.0, 0.0, 1.0])
}
