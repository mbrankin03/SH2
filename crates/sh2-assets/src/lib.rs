#![forbid(unsafe_code)]
//! sh2-assets: Zero-copy safe binary parsers for Silent Hill 2 formats (.map, .mdls, .anm, .sdm, DDS).

pub mod error;
pub mod fmv_manifest;
pub mod formats;
pub mod resolver;
pub mod vfs;

pub use error::AssetError;
pub use fmv_manifest::{
    sha256_bytes, sha256_file, verify_fmv_pack, FmvEntryVerifyStatus, FmvManifestEntry,
    FmvPackManifest, FmvPackVerificationReport,
};
pub use formats::*;
pub use resolver::AssetPathResolver;
pub use vfs::{discover_mods, ModMetadata, ModPackage, ModSource, ModVfs};
