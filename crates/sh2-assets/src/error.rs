use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AssetError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid magic identifier: expected {expected:?}, found {found:?}")]
    InvalidMagic {
        expected: &'static [u8],
        found: Vec<u8>,
    },

    #[error(
        "Buffer too small for structure: required {required} bytes, available {available} bytes"
    )]
    BufferUnderflow { required: usize, available: usize },

    #[error("Malformed header: {0}")]
    MalformedHeader(&'static str),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Image decode error: {0}")]
    ImageError(String),
}
