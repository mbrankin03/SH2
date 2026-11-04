//! Format parsers for SH2 binary files.

pub mod adx;
pub mod anm;
pub mod dxt;
pub mod map;
pub mod mdls;
pub mod mes;
pub mod sound;
pub mod texture;

pub use adx::{decode_adx, parse_adx_header, AdxHeader, DecodedAdx};
pub use anm::{
    compute_bind_locals, compute_skin_matrices, euler_to_matrix, evaluate_pose, interpolate_frames,
    mat4_inverse, mat4_mul, parse_anm, AnimationFile, AnimationFrame, BoneTransform,
};
pub use dxt::{decode_dds_file, decode_dxt};
pub use map::{parse_map, BatchGroup, MapBatch, MapFile, MapMaterial, MapTexture};
pub use mdls::{parse_mdl, ModelFile, ModelPrimitive, ModelTexture, ModelVertices};
pub use mes::{parse_mes, DialogueMessage, MessageTable};
pub use sound::{
    parse_sddata, parse_sdm_headers, parse_sdm_sdb, SdmEntryHeader, SoundBank, WavEntry,
};
pub use texture::{decode_texture_bytes, DecodedTexture, TextureLoader};
