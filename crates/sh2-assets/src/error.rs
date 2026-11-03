use thiserror::Error;

#[derive(Error, Debug)]
pub enum AssetError {
    #[error("Invalid magic identifier: expected {expected:?}, found {found:?}")]
    InvalidMagic {
        expected: &'static [u8],
        found: Vec<u8>,
    },
    #[error("Buffer too small for structure: required {required} bytes, available {available} bytes")]
    BufferUnderflow { required: usize, available: usize },
    #[error("Malformed header: {0}")]
    MalformedHeader(&'static str),
}
