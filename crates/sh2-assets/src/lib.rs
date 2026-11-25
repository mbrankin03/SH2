#![forbid(unsafe_code)]
//! sh2-assets: Zero-copy safe binary parsers for Silent Hill 2 formats (.map, .mdls, .anm, .sdm, DDS).

pub mod error;
pub mod formats;
pub mod resolver;
pub mod vfs;

pub use error::AssetError;
pub use formats::*;
pub use resolver::AssetPathResolver;
pub use vfs::{discover_mods, ModMetadata, ModPackage, ModSource, ModVfs};
