//! Typed glTF 2.0 binary accessor decoders with explicit bounds validation.

use super::super::binary::{read_f32, read_u16, read_u32};
use super::raw::{RawAccessor, RawGltf};
use crate::error::AssetError;

pub(super) fn accessor_slice<'r, 'b>(
    raw: &'r RawGltf,
    bin: &'b [u8],
    acc_idx: usize,
) -> Result<(&'b [u8], usize, &'r RawAccessor), AssetError> {
    let acc = raw
        .accessors
        .get(acc_idx)
        .ok_or_else(|| AssetError::InvalidData(format!("Missing accessor {acc_idx}")))?;
    let bv_idx = acc
        .buffer_view
        .ok_or_else(|| AssetError::InvalidData(format!("Accessor {acc_idx} has no bufferView")))?;
    let bv = raw
        .buffer_views
        .get(bv_idx)
        .ok_or_else(|| AssetError::InvalidData(format!("Missing bufferView {bv_idx}")))?;
    let start = bv
        .byte_offset
        .checked_add(acc.byte_offset)
        .ok_or(AssetError::MalformedHeader("BufferView offset overflow"))?;
    let end = bv
        .byte_offset
        .checked_add(bv.byte_length)
        .filter(|&e| e <= bin.len() && start <= e)
        .ok_or(AssetError::BufferUnderflow {
            required: bv.byte_offset.saturating_add(bv.byte_length),
            available: bin.len(),
        })?;
    let elem_size = component_byte_size(acc.component_type)? * type_component_count(&acc.kind)?;
    let stride = bv.byte_stride.unwrap_or(elem_size);
    if stride < elem_size {
        return Err(AssetError::MalformedHeader("Invalid bufferView stride"));
    }
    Ok((&bin[start..end], stride, acc))
}

fn component_byte_size(component_type: u32) -> Result<usize, AssetError> {
    match component_type {
        5120 | 5121 => Ok(1),
        5122 | 5123 => Ok(2),
        5125 | 5126 => Ok(4),
        other => Err(AssetError::UnsupportedFormat(format!(
            "Unsupported glTF componentType {other}"
        ))),
    }
}

fn type_component_count(kind: &str) -> Result<usize, AssetError> {
    match kind {
        "SCALAR" => Ok(1),
        "VEC2" => Ok(2),
        "VEC3" => Ok(3),
        "VEC4" => Ok(4),
        "MAT4" => Ok(16),
        other => Err(AssetError::UnsupportedFormat(format!(
            "Unsupported glTF accessor type {other}"
        ))),
    }
}

pub(super) fn read_f32_checked(slice: &[u8], offset: usize) -> Result<f32, AssetError> {
    if offset + 4 > slice.len() {
        return Err(AssetError::BufferUnderflow {
            required: offset + 4,
            available: slice.len(),
        });
    }
    read_f32(slice, offset)
}

pub(super) fn parse_scalar_f32_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<f32>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "SCALAR" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT SCALAR accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        out.push(read_f32_checked(slice, i * stride)?);
    }
    Ok(out)
}

pub(super) fn parse_vec2_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 2]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "VEC2" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT VEC2 accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        out.push([
            read_f32_checked(slice, base)?,
            read_f32_checked(slice, base + 4)?,
        ]);
    }
    Ok(out)
}

pub(super) fn parse_vec3_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 3]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "VEC3" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT VEC3 accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        out.push([
            read_f32_checked(slice, base)?,
            read_f32_checked(slice, base + 4)?,
            read_f32_checked(slice, base + 8)?,
        ]);
    }
    Ok(out)
}

pub(super) fn parse_vec4_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 4]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "VEC4" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT VEC4 accessor".into(),
        ));
    }
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        out.push([
            read_f32_checked(slice, base)?,
            read_f32_checked(slice, base + 4)?,
            read_f32_checked(slice, base + 8)?,
            read_f32_checked(slice, base + 12)?,
        ]);
    }
    Ok(out)
}

pub(super) fn parse_mat4_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[f32; 16]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    if acc.component_type != 5126 || acc.kind != "MAT4" {
        return Err(AssetError::UnsupportedFormat(
            "Expected FLOAT MAT4 accessor".into(),
        ));
    }
    let mut mats = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        let mut m = [0.0f32; 16];
        for (j, slot) in m.iter_mut().enumerate() {
            *slot = read_f32_checked(slice, base + j * 4)?;
        }
        mats.push(m);
    }
    Ok(mats)
}

pub(super) fn parse_joints_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<[u8; 4]>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        match acc.component_type {
            5121 => {
                if base + 4 > slice.len() {
                    return Err(AssetError::BufferUnderflow {
                        required: base + 4,
                        available: slice.len(),
                    });
                }
                out.push([
                    slice[base],
                    slice[base + 1],
                    slice[base + 2],
                    slice[base + 3],
                ]);
            }
            5123 => {
                if base + 8 > slice.len() {
                    return Err(AssetError::BufferUnderflow {
                        required: base + 8,
                        available: slice.len(),
                    });
                }
                out.push([
                    read_u16(slice, base)? as u8,
                    read_u16(slice, base + 2)? as u8,
                    read_u16(slice, base + 4)? as u8,
                    read_u16(slice, base + 6)? as u8,
                ]);
            }
            other => {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported JOINTS_0 componentType {other}"
                )));
            }
        }
    }
    Ok(out)
}

pub(super) fn parse_indices_accessor(
    raw: &RawGltf,
    bin: &[u8],
    acc_idx: usize,
) -> Result<Vec<u16>, AssetError> {
    let (slice, stride, acc) = accessor_slice(raw, bin, acc_idx)?;
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let base = i * stride;
        match acc.component_type {
            5121 => {
                if base >= slice.len() {
                    return Err(AssetError::BufferUnderflow {
                        required: base + 1,
                        available: slice.len(),
                    });
                }
                out.push(u16::from(slice[base]));
            }
            5123 => out.push(read_u16(slice, base)?),
            5125 => {
                let v = read_u32(slice, base)?;
                let idx = u16::try_from(v).map_err(|_| {
                    AssetError::UnsupportedFormat(format!("Index {v} exceeds u16::MAX"))
                })?;
                out.push(idx);
            }
            other => {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported index componentType {other}"
                )));
            }
        }
    }
    Ok(out)
}
