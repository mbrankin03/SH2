//! Safe glTF 2.0 Binary (`.glb`) parser for optional SH2:EE 3D replacement models
//! (`sh2e/model/ty3.glb` for `CockroachesReplacement.cpp`, `sh2e/model/mon.glb` for
//! `DogRoomEnhanced.cpp`, and `sh2e/model/dmr.glb` for `MariaCutsceneReplacement.cpp`).

mod accessors;
mod anim;
mod raw;

use super::binary::read_u32;
use crate::error::AssetError;
use accessors::{
    parse_indices_accessor, parse_joints_accessor, parse_mat4_accessor, parse_vec2_accessor,
    parse_vec3_accessor, parse_vec4_accessor,
};
use anim::{parse_animations, sample_quat, sample_vec3};
use raw::RawGltf;

pub use anim::{GlbAnimTrack, GlbInterpolation};

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

/// Evaluated local transform for a [`GlbNode`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlbNodeLocalTransform {
    /// Explicit column-major 4x4 matrix from glTF `node.matrix`.
    Matrix([f32; 16]),
    /// Decomposed translation/rotation/scale transform.
    Trs {
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    },
}

/// Scene node in the `.glb` hierarchy.
#[derive(Debug, Clone)]
pub struct GlbNode {
    /// Optional index into [`GlbModel::meshes`].
    pub mesh_index: Option<usize>,
    /// Optional index into [`GlbModel::skins`].
    pub skin_index: Option<usize>,
    /// Optional explicit column-major 4x4 local matrix (`node.matrix`).
    pub matrix: Option<[f32; 16]>,
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
    /// Evaluates local transforms for every node at `time_sec` (wrapping if `loop_anim`).
    #[must_use]
    pub fn evaluate_node_locals(
        &self,
        time_sec: f32,
        loop_anim: bool,
    ) -> Vec<GlbNodeLocalTransform> {
        let t = if self.anim_duration_sec > 0.0 {
            if loop_anim {
                time_sec.rem_euclid(self.anim_duration_sec)
            } else {
                time_sec.clamp(0.0, self.anim_duration_sec)
            }
        } else {
            0.0
        };

        let mut locals: Vec<GlbNodeLocalTransform> = self
            .nodes
            .iter()
            .map(|n| match n.matrix {
                Some(m) => GlbNodeLocalTransform::Matrix(m),
                None => GlbNodeLocalTransform::Trs {
                    translation: n.translation,
                    rotation: n.rotation,
                    scale: n.scale,
                },
            })
            .collect();

        for track in &self.anim_tracks {
            let Some(node) = self.nodes.get(track.target_node) else {
                continue;
            };
            let Some(entry) = locals.get_mut(track.target_node) else {
                continue;
            };
            let (mut translation, mut rotation, mut scale) = match *entry {
                GlbNodeLocalTransform::Trs {
                    translation,
                    rotation,
                    scale,
                } => (translation, rotation, scale),
                GlbNodeLocalTransform::Matrix(_) => (node.translation, node.rotation, node.scale),
            };
            if !track.translations.is_empty() {
                translation = sample_vec3(&track.translations, t, track.translation_interp);
            }
            if !track.rotations.is_empty() {
                rotation = sample_quat(&track.rotations, t, track.rotation_interp);
            }
            if !track.scales.is_empty() {
                scale = sample_vec3(&track.scales, t, track.scale_interp);
            }
            *entry = GlbNodeLocalTransform::Trs {
                translation,
                rotation,
                scale,
            };
        }
        locals
    }
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

    let (textures, image_to_tex) = parse_textures(&raw, bin_bytes)?;
    let meshes = parse_meshes(&raw, bin_bytes, &image_to_tex)?;
    let nodes = raw
        .nodes
        .iter()
        .map(|n| GlbNode {
            mesh_index: n.mesh,
            skin_index: n.skin,
            matrix: n.matrix,
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

fn parse_textures(
    raw: &RawGltf,
    bin: &[u8],
) -> Result<(Vec<GlbTexture>, Vec<Option<usize>>), AssetError> {
    let mut textures = Vec::with_capacity(raw.images.len());
    let mut image_to_tex = Vec::with_capacity(raw.images.len());
    for img in &raw.images {
        let Some(bv_idx) = img.buffer_view else {
            image_to_tex.push(None);
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
        let tex_idx = textures.len();
        textures.push(GlbTexture {
            width: rgba8.width(),
            height: rgba8.height(),
            rgba: rgba8.into_raw(),
        });
        image_to_tex.push(Some(tex_idx));
    }
    Ok((textures, image_to_tex))
}

fn parse_meshes(
    raw: &RawGltf,
    bin: &[u8],
    image_to_tex: &[Option<usize>],
) -> Result<Vec<GlbMesh>, AssetError> {
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
                None => {
                    if vertices.len() > usize::from(u16::MAX) + 1 {
                        return Err(AssetError::UnsupportedFormat(format!(
                            "Unindexed primitive vertex count {} exceeds u16::MAX + 1",
                            vertices.len()
                        )));
                    }
                    (0..vertices.len()).map(|i| i as u16).collect()
                }
            };

            let (texture_index, alpha_blend) =
                match p.material.and_then(|idx| raw.materials.get(idx)) {
                    Some(mat) => {
                        let tex_idx = mat
                            .pbr
                            .as_ref()
                            .and_then(|pbr| pbr.base_color_texture.as_ref())
                            .and_then(|info| raw.textures.get(info.index))
                            .and_then(|t| t.source)
                            .and_then(|img_idx| image_to_tex.get(img_idx).copied().flatten());
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
            Some(acc_idx) => parse_mat4_accessor(raw, bin, acc_idx)?,
            None => Vec::new(),
        };
        skins.push(GlbSkin {
            joints: s.joints.clone(),
            inverse_bind_matrices,
        });
    }
    Ok(skins)
}
