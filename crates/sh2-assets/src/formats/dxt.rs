//! DDS and DXT (Block Compression BC1, BC2, BC3) software decoders.
//!
//! # Format Architecture & Compression Algorithms
//! DirectDraw Surface (`.dds`) and embedded `.map` / `.mdl` textures utilize fixed 4x4 pixel block compression:
//!
//! ### BC1 (DXT1) - 8 bytes per 4x4 block
//! - `color0` (16-bit RGB565) and `color1` (16-bit RGB565).
//! - 4-color palette generated:
//!   - If $c_0 > c_1$: 4 opaque colors ($c_0$, $c_1$, $\frac{2}{3} c_0 + \frac{1}{3} c_1$, $\frac{1}{3} c_0 + \frac{2}{3} c_1$).
//!   - If $c_0 \le c_1$: 3 opaque colors plus 1 transparent black pixel.
//! - 32-bit index table: 2 bits per pixel choosing from the 4 palette colors.
//!
//! ### BC2 (DXT3) - 16 bytes per 4x4 block
//! - 8 bytes explicit 4-bit alpha values per pixel (16 pixels $\times$ 4 bits = 64 bits).
//! - 8 bytes color block (identical to BC1 in 4-color mode).
//!
//! ### BC3 (DXT5) - 16 bytes per 4x4 block
//! - 8 bytes interpolated alpha:
//!   - 2 reference bytes $\alpha_0$, $\alpha_1$.
//!   - If $\alpha_0 > \alpha_1$: 6 interpolated alpha levels (8-level ramp).
//!   - If $\alpha_0 \le \alpha_1$: 4 interpolated alpha levels + 0 and 255 extremes.
//!   - 48-bit index table (3 bits per pixel).
//! - 8 bytes color block (identical to BC1 in 4-color mode).

use crate::error::AssetError;

/// Decodes raw DXT compressed pixel blocks into 32-bit RGBA8 pixels.
///
/// # Arguments
/// - `data`: Compressed block byte stream.
/// - `width`: Target width in pixels.
/// - `height`: Target height in pixels.
/// - `kind`: Compression kind:
///   - `1`: DXT1 / BC1 (8 bytes per 4x4 block)
///   - `3`: DXT3 / BC2 (16 bytes per 4x4 block, explicit 4-bit alpha)
///   - `5`: DXT5 / BC3 (16 bytes per 4x4 block, interpolated 8-step alpha)
///
/// # Returns
/// Uncompressed vector of length `width * height * 4` in RGBA byte order.
pub fn decode_dxt(
    data: &[u8],
    width: usize,
    height: usize,
    kind: u8,
) -> Result<Vec<u8>, AssetError> {
    if width == 0 || height == 0 {
        return Ok(Vec::new());
    }

    let bw = width.div_ceil(4);
    let bh = height.div_ceil(4);
    let block_size = if kind == 1 { 8 } else { 16 };
    let required_len = bw * bh * block_size;

    if data.len() < required_len {
        return Err(AssetError::BufferUnderflow {
            required: required_len,
            available: data.len(),
        });
    }

    let mut rgba = vec![0u8; width * height * 4];

    for by in 0..bh {
        for bx in 0..bw {
            let block_idx = (by * bw + bx) * block_size;
            let block = &data[block_idx..block_idx + block_size];

            let mut block_pixels = [[0u8; 4]; 16];

            if kind == 1 {
                // BC1 / DXT1: 8 bytes
                decode_bc1_block(block, &mut block_pixels);
            } else if kind == 3 {
                // BC2 / DXT3: 8 bytes alpha, 8 bytes color
                decode_bc1_block(&block[8..16], &mut block_pixels);
                // 4 bits per pixel alpha (scaled by 17: 0..15 -> 0..255)
                for i in 0..8 {
                    let a_byte = block[i];
                    let a0 = (a_byte & 0x0F) * 17;
                    let a1 = ((a_byte >> 4) & 0x0F) * 17;
                    block_pixels[i * 2][3] = a0;
                    block_pixels[i * 2 + 1][3] = a1;
                }
            } else if kind == 5 {
                // BC3 / DXT5: 8 bytes alpha, 8 bytes color
                decode_bc1_block(&block[8..16], &mut block_pixels);
                decode_bc3_alpha(block, &mut block_pixels);
            } else {
                return Err(AssetError::UnsupportedFormat(format!(
                    "Unsupported DXT kind: {kind}"
                )));
            }

            // Scatter 4x4 block into target row-major RGBA buffer
            for py in 0..4 {
                let y = by * 4 + py;
                if y >= height {
                    continue;
                }
                for px in 0..4 {
                    let x = bx * 4 + px;
                    if x >= width {
                        continue;
                    }
                    let src_px = block_pixels[py * 4 + px];
                    let dst_idx = (y * width + x) * 4;
                    rgba[dst_idx..dst_idx + 4].copy_from_slice(&src_px);
                }
            }
        }
    }

    Ok(rgba)
}

/// Converts a 16-bit RGB565 word into 24-bit RGB8 components.
fn rgb565_to_rgb8(c: u16) -> [u8; 3] {
    let r = (((c >> 11) & 31) * 255 + 15) / 31;
    let g = (((c >> 5) & 63) * 255 + 31) / 63;
    let b = ((c & 31) * 255 + 15) / 31;
    [r as u8, g as u8, b as u8]
}

/// Decodes an 8-byte BC1 color block into sixteen RGBA pixels.
fn decode_bc1_block(block: &[u8], out: &mut [[u8; 4]; 16]) {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let p0 = rgb565_to_rgb8(c0);
    let p1 = rgb565_to_rgb8(c1);

    let mut pal = [[0u8; 4]; 4];
    pal[0] = [p0[0], p0[1], p0[2], 255];
    pal[1] = [p1[0], p1[1], p1[2], 255];

    if c0 > c1 {
        // 4-color opaque palette
        pal[2] = [
            ((2 * p0[0] as u32 + p1[0] as u32) / 3) as u8,
            ((2 * p0[1] as u32 + p1[1] as u32) / 3) as u8,
            ((2 * p0[2] as u32 + p1[2] as u32) / 3) as u8,
            255,
        ];
        pal[3] = [
            ((p0[0] as u32 + 2 * p1[0] as u32) / 3) as u8,
            ((p0[1] as u32 + 2 * p1[1] as u32) / 3) as u8,
            ((p0[2] as u32 + 2 * p1[2] as u32) / 3) as u8,
            255,
        ];
    } else {
        // 3-color palette + 1 transparent pixel
        pal[2] = [
            ((p0[0] as u32 + p1[0] as u32) / 2) as u8,
            ((p0[1] as u32 + p1[1] as u32) / 2) as u8,
            ((p0[2] as u32 + p1[2] as u32) / 2) as u8,
            255,
        ];
        pal[3] = [0, 0, 0, 0];
    }

    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    for (i, pixel) in out.iter_mut().enumerate() {
        let sel = ((indices >> (2 * i)) & 3) as usize;
        *pixel = pal[sel];
    }
}

/// Decodes an 8-byte BC3 interpolated alpha block into the alpha channel of sixteen pixels.
fn decode_bc3_alpha(block: &[u8], out: &mut [[u8; 4]; 16]) {
    let a0 = block[0] as u32;
    let a1 = block[1] as u32;

    let mut alpha_pal = [0u8; 8];
    alpha_pal[0] = a0 as u8;
    alpha_pal[1] = a1 as u8;

    if a0 > a1 {
        // 8-step linear ramp
        for i in 1..7 {
            alpha_pal[i + 1] = (((7 - i as u32) * a0 + i as u32 * a1) / 7) as u8;
        }
    } else {
        // 6-step ramp with 0 and 255 clamping
        for i in 1..5 {
            alpha_pal[i + 1] = (((5 - i as u32) * a0 + i as u32 * a1) / 5) as u8;
        }
        alpha_pal[6] = 0;
        alpha_pal[7] = 255;
    }

    let mut bits = 0u64;
    for i in 0..6 {
        bits |= (block[2 + i] as u64) << (8 * i);
    }

    for (i, pixel) in out.iter_mut().enumerate() {
        let sel = ((bits >> (3 * i)) & 7) as usize;
        pixel[3] = alpha_pal[sel];
    }
}

/// Decodes standard DirectDraw Surface (`.dds`) file bytes into `(width, height, rgba)`.
///
/// Supports FourCC tags `DXT1`, `DXT3`, and `DXT5`.
pub fn decode_dds_file(data: &[u8]) -> Result<(usize, usize, Vec<u8>), AssetError> {
    if data.len() < 128 {
        return Err(AssetError::BufferUnderflow {
            required: 128,
            available: data.len(),
        });
    }

    if &data[0..4] != b"DDS " {
        return Err(AssetError::InvalidMagic {
            expected: b"DDS ",
            found: data[0..4].to_vec(),
        });
    }

    let height = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
    let width = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
    let fourcc = &data[84..88];

    let kind = match fourcc {
        b"DXT1" => 1,
        b"DXT3" => 3,
        b"DXT5" => 5,
        _ => {
            return Err(AssetError::UnsupportedFormat(format!(
                "Unsupported DDS FourCC: {:?}",
                String::from_utf8_lossy(fourcc)
            )))
        }
    };

    let rgba = decode_dxt(&data[128..], width, height, kind)?;
    Ok((width, height, rgba))
}
