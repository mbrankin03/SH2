//! Format parsers for SH2 binary files.

pub mod anm;
pub mod dxt;
pub mod map;
pub mod mdls;
pub mod sound;
pub mod texture;

pub use anm::{parse_anm, AnimationFile, AnimationFrame, BoneTransform};
pub use dxt::{decode_dds_file, decode_dxt};
pub use map::{parse_map, BatchGroup, MapBatch, MapFile, MapMaterial, MapTexture};
pub use mdls::{parse_mdl, ModelFile, ModelPrimitive, ModelTexture, ModelVertices};
pub use sound::{parse_sddata, SoundBank, WavEntry};
pub use texture::{decode_texture_bytes, DecodedTexture, TextureLoader};
