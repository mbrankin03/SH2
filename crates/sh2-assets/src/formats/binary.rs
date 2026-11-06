//! Centralized binary reading helpers for little-endian Silent Hill 2 asset files.

use crate::error::AssetError;

/// Reads an unsigned 16-bit little-endian integer from a byte slice.
#[inline(always)]
pub fn read_u16(data: &[u8], ofs: usize) -> Result<u16, AssetError> {
    if ofs + 2 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        });
    }
    Ok(u16::from_le_bytes([data[ofs], data[ofs + 1]]))
}

/// Reads a signed 16-bit little-endian integer from a byte slice.
#[inline(always)]
pub fn read_i16(data: &[u8], ofs: usize) -> Result<i16, AssetError> {
    if ofs + 2 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        });
    }
    Ok(i16::from_le_bytes([data[ofs], data[ofs + 1]]))
}

/// Reads an unsigned 32-bit little-endian integer from a byte slice.
#[inline(always)]
pub fn read_u32(data: &[u8], ofs: usize) -> Result<u32, AssetError> {
    if ofs + 4 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        });
    }
    Ok(u32::from_le_bytes([
        data[ofs],
        data[ofs + 1],
        data[ofs + 2],
        data[ofs + 3],
    ]))
}

/// Reads an IEEE 754 32-bit little-endian float from a byte slice.
#[inline(always)]
pub fn read_f32(data: &[u8], ofs: usize) -> Result<f32, AssetError> {
    if ofs + 4 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        });
    }
    Ok(f32::from_le_bytes([
        data[ofs],
        data[ofs + 1],
        data[ofs + 2],
        data[ofs + 3],
    ]))
}
