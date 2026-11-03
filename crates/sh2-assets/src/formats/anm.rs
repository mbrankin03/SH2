use crate::error::AssetError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationTrackType {
    RotationOnly,
    TranslationAndRotation,
    ExtendedRotation,
    ExtendedTranslationAndRotation1,
    ExtendedTranslationAndRotation2,
    DualTranslationAndRotation,
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

/// Rotation and translation for a specific bone in a frame.
/// Rotations are stored in Euler radians (scaled from 4096 units/rad).
#[derive(Debug, Clone, PartialEq)]
pub struct BoneTransform {
    pub rotation: Option<[f32; 3]>,
    pub translation: Option<[f32; 3]>,
    pub track_type: AnimationTrackType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationFrame {
    pub bones: Vec<Option<BoneTransform>>,
}

#[derive(Debug, Clone)]
pub struct AnimationFile {
    pub bone_count: usize,
    pub frames: Vec<AnimationFrame>,
}

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
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6;
                    }
                }
                2 => {
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
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10; // 6 bytes rot + 8 bytes + 2 pad
                    }
                }
                4 | 5 => {
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

/// Convert IEEE 754 half precision float (u16) to single precision f32
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign << 31)
        } else {
            // Denormalized
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
        // Normalized
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
