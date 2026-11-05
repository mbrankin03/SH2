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
                // BC1 / DXT1: 8 bytes (uses c0 <= c1 3-color + transparent black mode)
                decode_bc1_block(block, &mut block_pixels, true);
            } else if kind == 3 {
                // BC2 / DXT3: 8 bytes alpha, 8 bytes color (always 4-color interpolation)
                decode_bc1_block(&block[8..16], &mut block_pixels, false);
                // 4 bits per pixel alpha (scaled by 17: 0..15 -> 0..255)
                for i in 0..8 {
                    let a_byte = block[i];
                    let a0 = (a_byte & 0x0F) * 17;
                    let a1 = ((a_byte >> 4) & 0x0F) * 17;
                    block_pixels[i * 2][3] = a0;
                    block_pixels[i * 2 + 1][3] = a1;
                }
            } else if kind == 5 {
                // BC3 / DXT5: 8 bytes alpha, 8 bytes color (always 4-color interpolation)
                decode_bc1_block(&block[8..16], &mut block_pixels, false);
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

    // Dilate RGB colors from non-transparent pixels into adjacent alpha == 0 cutout pixels
    // so bilinear texture filtering and 2x upscaling do not bleed black (0, 0, 0, 0) fringes
    // onto hair strand tips and cutout alpha boundaries.
    bleed_transparent_cutout_rgb(&mut rgba, width, height);

    Ok(rgba)
}

/// Propagates RGB values from visible (`alpha > 0`) pixels into neighboring transparent cutout
/// (`alpha == 0` with `RGB == (0, 0, 0)`) pixels while keeping their alpha channel at `0`.
///
/// Many PS2/PC Silent Hill 2 DXT textures store `(0, 0, 0, 0)` in masked cutout regions
/// (such as Maria's dyed pink hair tips and strand cards). Without RGB dilation, hardware
/// `FilterMode::Linear` sampling and bilinear upscaling interpolate between hair color and
/// `(0, 0, 0)`, creating dark/black outlines around cutout hair strands.
fn bleed_transparent_cutout_rgb(rgba: &mut [u8], width: usize, height: usize) {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return;
    }

    // Quick check: only run if the texture actually contains both alpha == 0 and alpha > 0 pixels
    let total_px = width * height;
    let mut has_zero_alpha = false;
    let mut has_nonzero_alpha = false;
    for i in 0..total_px {
        if rgba[i * 4 + 3] == 0 {
            has_zero_alpha = true;
        } else {
            has_nonzero_alpha = true;
        }
        if has_zero_alpha && has_nonzero_alpha {
            break;
        }
    }
    if !has_zero_alpha || !has_nonzero_alpha {
        return;
    }

    // Perform 4 passes of 8-connected RGB dilation into alpha == 0 pixels
    let mut filled = vec![false; total_px];
    for (i, slot) in filled.iter_mut().enumerate() {
        *slot = rgba[i * 4 + 3] > 0;
    }

    let mut next_rgba = rgba.to_vec();
    let mut next_filled = filled.clone();

    for _pass in 0..4 {
        let mut any_changed = false;
        for y in 0..height {
            let y_min = y.saturating_sub(1);
            let y_max = (y + 1).min(height - 1);
            for x in 0..width {
                let idx = y * width + x;
                if filled[idx] {
                    continue;
                }

                let x_min = x.saturating_sub(1);
                let x_max = (x + 1).min(width - 1);

                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;

                for ny in y_min..=y_max {
                    for nx in x_min..=x_max {
                        let n_idx = ny * width + nx;
                        if filled[n_idx] {
                            let n_ofs = n_idx * 4;
                            sum_r += rgba[n_ofs] as u32;
                            sum_g += rgba[n_ofs + 1] as u32;
                            sum_b += rgba[n_ofs + 2] as u32;
                            count += 1;
                        }
                    }
                }

                if let (Some(r), Some(g), Some(b)) = (
                    sum_r.checked_div(count),
                    sum_g.checked_div(count),
                    sum_b.checked_div(count),
                ) {
                    let ofs = idx * 4;
                    next_rgba[ofs] = r as u8;
                    next_rgba[ofs + 1] = g as u8;
                    next_rgba[ofs + 2] = b as u8;
                    // Keep alpha == 0 so cutout discard still treats this texel as transparent
                    next_filled[idx] = true;
                    any_changed = true;
                }
            }
        }
        if !any_changed {
            break;
        }
        rgba.copy_from_slice(&next_rgba);
        filled.copy_from_slice(&next_filled);
    }
}

/// Converts a 16-bit RGB565 word into 24-bit RGB8 components.
fn rgb565_to_rgb8(c: u16) -> [u8; 3] {
    let r = (((c >> 11) & 31) * 255 + 15) / 31;
    let g = (((c >> 5) & 63) * 255 + 31) / 63;
    let b = ((c & 31) * 255 + 15) / 31;
    [r as u8, g as u8, b as u8]
}

/// Decodes an 8-byte BC1 color block into sixteen RGBA pixels.
///
/// When `allow_punchthrough` is `true` (BC1 / DXT1), blocks with `c0 <= c1` use the 3-color
/// palette with transparent black at index 3. When `false` (BC2 / DXT3 and BC3 / DXT5), all
/// blocks use the 4-color interpolated palette because alpha is encoded in the preceding 8 bytes.
fn decode_bc1_block(block: &[u8], out: &mut [[u8; 4]; 16], allow_punchthrough: bool) {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let p0 = rgb565_to_rgb8(c0);
    let p1 = rgb565_to_rgb8(c1);

    let mut pal = [[0u8; 4]; 4];
    pal[0] = [p0[0], p0[1], p0[2], 255];
    pal[1] = [p1[0], p1[1], p1[2], 255];

    if c0 > c1 || !allow_punchthrough {
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
        // 3-color palette + 1 transparent pixel (BC1/DXT1 only)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bc3_dxt5_c0_le_c1_uses_four_color_interpolation_and_dilates_cutout_rgb() {
        // Construct a 4x4 DXT5 block where:
        // - Alpha block has a0 = 0, a1 = 255 (c0 <= c1 ramp with code 6 = 0, code 7 = 255)
        //   First 8 pixels use alpha code 1 (255), next 8 pixels use alpha code 6 (0).
        // - Color block has c0 <= c1 (e.g. c0 = 0x7BEF mid-gray/pinkish, c1 = 0xFFFF white)
        //   and pixel 0 uses color code 3 (`(p0 + 2*p1)/3`), while pixel 8 (alpha=0) has black RGB.
        let mut block = [0u8; 16];
        block[0] = 0; // a0
        block[1] = 255; // a1
                        // 16 pixels x 3 bits = 48 bits:
                        // pixels 0..8 -> code 1 (alpha = 255)
                        // pixels 8..16 -> code 6 (alpha = 0)
        let mut abits = 0u64;
        for i in 0..8 {
            abits |= 1u64 << (3 * i);
        }
        for i in 8..16 {
            abits |= 6u64 << (3 * i);
        }
        for i in 0..6 {
            block[2 + i] = ((abits >> (8 * i)) & 0xFF) as u8;
        }

        // Color block: c0 = 0xF810 (pink/magenta-ish), c1 = 0xFFFF (white), so c0 <= c1!
        let c0: u16 = 0xF810;
        let c1: u16 = 0xFFFF;
        block[8..10].copy_from_slice(&c0.to_le_bytes());
        block[10..12].copy_from_slice(&c1.to_le_bytes());
        // All 16 pixels select palette index 3 (0b11 per pixel = 0xFFFFFFFF)
        block[12..16].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());

        let rgba = decode_dxt(&block, 4, 4, 5).expect("DXT5 decode should succeed");
        // In DXT5, even when c0 <= c1, palette index 3 MUST be (p0 + 2*p1)/3 (bright pink/white), NOT black (0,0,0)!
        assert!(
            rgba[0] > 200 && rgba[1] > 150 && rgba[2] > 180 && rgba[3] == 255,
            "DXT5 c0<=c1 code 3 pixel should be 4-color interpolated, got {:?}",
            &rgba[0..4]
        );
        // Cutout pixel 8 should preserve alpha == 0 while having dilated non-black RGB
        let px8 = &rgba[8 * 4..9 * 4];
        assert_eq!(px8[3], 0);
        assert!(
            px8[0] > 200 && px8[1] > 150 && px8[2] > 180,
            "Cutout alpha==0 pixel should have dilated RGB from neighbor, got {:?}",
            px8
        );
    }
}
