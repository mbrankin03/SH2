#![forbid(unsafe_code)]
//! sh2-assets: Zero-copy safe binary parsers for Silent Hill 2 formats (.map, .mdls, .anm, .sdm, DDS).

pub mod error;
pub mod formats;
pub mod resolver;

pub use error::AssetError;
pub use formats::*;
pub use resolver::AssetPathResolver;
