//! Safe binary parser for Silent Hill 2 Skeletal Animations (`.anm`).
//!
//! # Format Architecture & Memory Layout
//! Silent Hill 2 `.anm` files contain keyframed joint transformations (rotation and translation)
//! compressed into sequential 8-bone blocks.
//!
//! ### Animation Block Structure
//! The animation stream consists of repeated chunks corresponding to groups of 8 bones:
//! - **Block Header (4 bytes)**: A 32-bit bitfield containing 8 nibbles (4 bits per bone).
//!   Each 4-bit track code indicates the transform data stored for that bone:
//!   - `1`: Fixed-point Euler rotation (3 $\times$ `i16`, scaled by $1 / 4096.0$ radians).
//!   - `2`: Translation followed by Euler rotation.
//!     If root bone: translation is 3 x `f32` (12 bytes).
//!     If non-root bone: translation is 3 x `f16` half-precision floats (6 bytes).
//!   - `3`: Extended Euler rotation (`3h` + 10-byte padding).
//!   - `4` / `5`: Extended translation and rotation tracks.
//!   - `6`: Dual translation and rotation tracks (used for root trajectories and complex joints).
//! - **Frame Boundaries**:
//!   When `(block * 8)` wraps across the aligned `per_frame = (bone_count + 7) & !7` boundary,
//!   a complete [`AnimationFrame`] has been decoded.

use crate::error::AssetError;

/// Transform track encoding types declared in the 4-bit nibbles of block headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationTrackType {
    /// 3 x `i16` rotation angles (Euler radians / 4096).
    RotationOnly,
    /// Vector translation (3xf32 or 3xf16) and 3 x `i16` rotation.
    TranslationAndRotation,
    /// Rotation track with trailing metadata padding.
    ExtendedRotation,
    /// Extended translation (single-precision root or half-precision joint) and rotation.
    ExtendedTranslationAndRotation1,
    /// Alternate extended translation layout with padding.
    ExtendedTranslationAndRotation2,
    /// Dual translation vectors and rotation (used for complex root motion).
    DualTranslationAndRotation,
    /// Unrecognized track code.
    Unknown(u8),
}

impl From<u8> for AnimationTrackType {
    fn from(val: u8) -> Self {
        match val {
            1 => AnimationTrackType::RotationOnly,
            2 => AnimationTrackType::TranslationAndRotation,
            3 => AnimationTrackType::ExtendedRotation,
            4 => AnimationTrackType::ExtendedTranslationAndRotation1,
            5 => AnimationTrackType::ExtendedTranslationAndRotation2,
            6 => AnimationTrackType::DualTranslationAndRotation,
            other => AnimationTrackType::Unknown(other),
        }
    }
}

/// Rotation and translation for a specific bone in an animation frame.
///
/// Rotations are stored in Euler radians (scaled from original 4096 units/radian).
#[derive(Debug, Clone, PartialEq)]
pub struct BoneTransform {
    /// Euler rotation angles $(R_x, R_y, R_z)$ in radians, if present.
    pub rotation: Option<[f32; 3]>,
    /// Translation offset $(T_x, T_y, T_z)$ in world/parent space, if present.
    pub translation: Option<[f32; 3]>,
    /// Encoding descriptor from the frame block header.
    pub track_type: AnimationTrackType,
}

/// A complete animation frame containing transform entries for each bone.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationFrame {
    /// Array of bone transforms indexed by skeleton bone ID.
    pub bones: Vec<Option<BoneTransform>>,
}

/// Parsed skeletal animation sequence.
#[derive(Debug, Clone)]
pub struct AnimationFile {
    /// Total number of bones animated by this sequence.
    pub bone_count: usize,
    /// Ordered sequence of decoded animation frames.
    pub frames: Vec<AnimationFrame>,
}

/// Parses raw `.anm` bytes into an [`AnimationFile`] given the target model's bone hierarchy.
///
/// # Arguments
/// - `data`: Raw byte slice of the `.anm` file.
/// - `bone_count`: Skeleton bone count from the matching [`.mdl` / `.mdls`](crate::formats::mdls::ModelFile).
/// - `parents`: Skeleton parent array used to identify root bones (`parents[i] == 255`).
pub fn parse_anm(
    data: &[u8],
    bone_count: usize,
    parents: &[u8],
) -> Result<AnimationFile, AssetError> {
    if bone_count == 0 {
        return Ok(AnimationFile {
            bone_count: 0,
            frames: Vec::new(),
        });
    }

    let per_frame = (bone_count + 7) & !7;
    let mut frames = Vec::new();
    let mut cur_frame: Vec<Option<BoneTransform>> = vec![None; bone_count];

    let mut p = 0usize;
    let mut block = 0usize;

    while p + 4 <= data.len() {
        let header = read_u32(data, p)?;
        p += 4;

        for j in 0..8 {
            let t = ((header >> (4 * j)) & 7) as u8;
            let bone_idx = ((block * 8) % per_frame) + j;
            let is_root = if bone_idx < parents.len() {
                parents[bone_idx] == 255
            } else {
                false
            };

            let mut rot: Option<[f32; 3]> = None;
            let mut trans: Option<[f32; 3]> = None;

            match t {
                1 => {
                    // Type 1: 3x i16 rotations
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6;
                    }
                }
                2 => {
                    // Type 2: Root translation (3x f32) or joint translation (3x f16), then 3x i16 rot
                    if is_root {
                        if p + 12 <= data.len() {
                            let tx = read_f32(data, p)?;
                            let ty = read_f32(data, p + 4)?;
                            let tz = read_f32(data, p + 8)?;
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    } else {
                        if p + 6 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 2)?);
                            let tz = f16_to_f32(read_u16(data, p + 4)?);
                            trans = Some([tx, ty, tz]);
                            p += 6;
                        }
                    }
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6;
                    }
                }
                3 => {
                    // Type 3: 3x i16 rotations + 10 bytes padding
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10;
                    }
                }
                4 | 5 => {
                    // Types 4 & 5: Extended translation tracks with 10 bytes padding
                    if t == 4 && is_root {
                        if p + 12 <= data.len() {
                            let tx = read_f32(data, p)?;
                            let ty = read_f32(data, p + 4)?;
                            let tz = read_f32(data, p + 8)?;
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    } else if t == 5 && is_root {
                        if p + 12 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 4)?);
                            let tz = f16_to_f32(read_u16(data, p + 8)?);
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    } else {
                        if p + 6 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 2)?);
                            let tz = f16_to_f32(read_u16(data, p + 4)?);
                            trans = Some([tx, ty, tz]);
                            p += 6;
                        }
                    }
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10;
                    }
                }
                6 => {
                    // Type 6: Dual translation vectors (PC 0x54FC67)
                    if is_root {
                        if p + 12 <= data.len() {
                            let tx = read_f32(data, p)?;
                            let ty = read_f32(data, p + 4)?;
                            let tz = read_f32(data, p + 8)?;
                            trans = Some([tx, ty, tz]);
                            p += 24;
                        }
                    } else {
                        if p + 6 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 2)?);
                            let tz = f16_to_f32(read_u16(data, p + 4)?);
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    }
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10;
                    }
                }
                _ => {}
            }

            if bone_idx < bone_count && t != 0 {
                cur_frame[bone_idx] = Some(BoneTransform {
                    rotation: rot,
                    translation: trans,
                    track_type: AnimationTrackType::from(t),
                });
            }
        }

        block += 1;
        if (block * 8).is_multiple_of(per_frame) {
            frames.push(AnimationFrame {
                bones: cur_frame.clone(),
            });
            cur_frame = vec![None; bone_count];
        }
    }

    Ok(AnimationFile { bone_count, frames })
}

/// Converts an IEEE 754 half-precision float (`u16`) to standard single-precision `f32`.
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign << 31)
        } else {
            // Denormalized half float
            let mut m = mant;
            let mut e = 113;
            while (m & 0x400) == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x3FF;
            f32::from_bits((sign << 31) | (e << 23) | (m << 13))
        }
    } else if exp == 31 {
        // Infinity or NaN
        f32::from_bits((sign << 31) | (0xFF << 23) | (mant << 13))
    } else {
        // Normalized half float
        f32::from_bits((sign << 31) | ((exp + 112) << 23) | (mant << 13))
    }
}

#[inline(always)]
fn read_u16(data: &[u8], ofs: usize) -> Result<u16, AssetError> {
    if ofs + 2 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        })
    } else {
        Ok(u16::from_le_bytes([data[ofs], data[ofs + 1]]))
    }
}

#[inline(always)]
fn read_i16(data: &[u8], ofs: usize) -> Result<i16, AssetError> {
    if ofs + 2 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 2,
            available: data.len(),
        })
    } else {
        Ok(i16::from_le_bytes([data[ofs], data[ofs + 1]]))
    }
}

#[inline(always)]
fn read_u32(data: &[u8], ofs: usize) -> Result<u32, AssetError> {
    if ofs + 4 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        })
    } else {
        Ok(u32::from_le_bytes([
            data[ofs],
            data[ofs + 1],
            data[ofs + 2],
            data[ofs + 3],
        ]))
    }
}

#[inline(always)]
fn read_f32(data: &[u8], ofs: usize) -> Result<f32, AssetError> {
    if ofs + 4 > data.len() {
        Err(AssetError::BufferUnderflow {
            required: ofs + 4,
            available: data.len(),
        })
    } else {
        Ok(f32::from_le_bytes([
            data[ofs],
            data[ofs + 1],
            data[ofs + 2],
            data[ofs + 3],
        ]))
    }
}
