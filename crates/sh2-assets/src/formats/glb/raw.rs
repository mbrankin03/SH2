//! Raw serde deserialization structs for glTF 2.0 JSON chunks.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub(super) struct RawGltf {
    #[serde(default)]
    pub scene: Option<usize>,
    #[serde(default)]
    pub scenes: Vec<RawScene>,
    #[serde(default)]
    pub nodes: Vec<RawNode>,
    #[serde(default)]
    pub meshes: Vec<RawMesh>,
    #[serde(default)]
    pub materials: Vec<RawMaterial>,
    #[serde(default)]
    pub textures: Vec<RawTextureRef>,
    #[serde(default)]
    pub images: Vec<RawImage>,
    #[serde(default)]
    pub skins: Vec<RawSkin>,
    #[serde(default)]
    pub animations: Vec<RawAnimation>,
    #[serde(default)]
    pub accessors: Vec<RawAccessor>,
    #[serde(default, rename = "bufferViews")]
    pub buffer_views: Vec<RawBufferView>,
}

#[derive(Deserialize)]
pub(super) struct RawScene {
    #[serde(default)]
    pub nodes: Vec<usize>,
}

#[derive(Deserialize)]
pub(super) struct RawNode {
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
    pub matrix: Option<[f32; 16]>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
    #[serde(default)]
    pub children: Vec<usize>,
}

#[derive(Deserialize)]
pub(super) struct RawMesh {
    #[serde(default)]
    pub primitives: Vec<RawPrimitive>,
}

#[derive(Deserialize)]
pub(super) struct RawPrimitive {
    pub attributes: HashMap<String, usize>,
    pub indices: Option<usize>,
    pub material: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct RawMaterial {
    #[serde(rename = "alphaMode")]
    pub alpha_mode: Option<String>,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr: Option<RawPbr>,
}

#[derive(Deserialize)]
pub(super) struct RawPbr {
    #[serde(rename = "baseColorTexture")]
    pub base_color_texture: Option<RawTexInfo>,
}

#[derive(Deserialize)]
pub(super) struct RawTexInfo {
    pub index: usize,
}

#[derive(Deserialize)]
pub(super) struct RawTextureRef {
    pub source: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct RawImage {
    #[serde(rename = "bufferView")]
    pub buffer_view: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct RawSkin {
    #[serde(default)]
    pub joints: Vec<usize>,
    #[serde(rename = "inverseBindMatrices")]
    pub inverse_bind_matrices: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct RawAnimation {
    #[serde(default)]
    pub channels: Vec<RawAnimChannel>,
    #[serde(default)]
    pub samplers: Vec<RawAnimSampler>,
}

#[derive(Deserialize)]
pub(super) struct RawAnimChannel {
    pub sampler: usize,
    pub target: RawAnimTarget,
}

#[derive(Deserialize)]
pub(super) struct RawAnimTarget {
    pub node: Option<usize>,
    pub path: String,
}

#[derive(Deserialize)]
pub(super) struct RawAnimSampler {
    pub input: usize,
    pub output: usize,
    pub interpolation: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawAccessor {
    #[serde(rename = "bufferView")]
    pub buffer_view: Option<usize>,
    #[serde(default, rename = "byteOffset")]
    pub byte_offset: usize,
    #[serde(rename = "componentType")]
    pub component_type: u32,
    pub count: usize,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize)]
pub(super) struct RawBufferView {
    #[serde(default, rename = "byteOffset")]
    pub byte_offset: usize,
    #[serde(rename = "byteLength")]
    pub byte_length: usize,
    #[serde(rename = "byteStride")]
    pub byte_stride: Option<usize>,
}
