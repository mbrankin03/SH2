//! Opaque cutout hair/fringe card detection and `alpha == 0` border RGB dilation (`SH2-37`).
//!
//! Low-LOD gameplay models (`lll_jms.mdl` `prim[23]`, `lll_mar.mdl` `prim[19..22, 24..25]`, and
//! `lxx_mar.mdl` `prim[24..27, 29..30]`) store cutout hair cards in the opaque `prim` stream
//! (`material_type == 2`, `is_transparent == false`) with `(0, 0, 0, 0)` punch-through texels,
//! whereas Maria's fingernails (`tid = 1245`, `prim[14..15]` / `prim[11..12]`), Lying Figure
//! (`tid = 1282`), and Abstract Daddy (`tid = 1293`) store non-zero PS2 specular gloss values in
//! `alpha` (`a0 == 0` across their UV islands, matching `sh2pc.exe` `0x00500008` `ALPHATESTENABLE = 0`).

use std::collections::HashMap;

use sh2_assets::formats::mdls::ModelFile;

/// Cached `(width, height, rgba8_bytes)` decoded embedded textures keyed by `texture_id`.
pub type DecodedModelTextureMap = HashMap<u32, (usize, usize, Vec<u8>)>;

/// Decodes all embedded textures in `model` once for fast UV-island cutout classification.
pub fn decode_model_textures_for_inspection(model: &ModelFile) -> DecodedModelTextureMap {
    let mut map = HashMap::with_capacity(model.textures.len());
    for (&tid, tex) in &model.textures {
        if tex.width > 0 && tex.height > 0 {
            if let Ok(rgba) = tex.to_rgba() {
                if rgba.len() == tex.width * tex.height * 4 {
                    map.insert(tid, (tex.width, tex.height, rgba));
                }
            }
        }
    }
    map
}

/// Returns `true` when an opaque stream primitive (`!prim.is_transparent`) uses genuine `alpha == 0`
/// silhouette cutouts on its UV island (e.g., `lll_jms` `prim[23]` hair card and `lll_mar` / `lxx_mar`
/// `tid = 1246` hair cards) and should be promoted to the two-pass `TransparentCore` +
/// `TransparentFringe` pipeline.
///
/// Returns `false` for:
/// - `prim.is_transparent` (`tprim` primitives already use two-pass transparency),
/// - `prim.material_type >= 3` (PS2/PC specular & env gloss-mask submeshes such as `red.mdl` `1303`),
/// - `prim.material_type <= 2` gloss-mask or solid submeshes with `< 5%` `alpha == 0` coverage on
///   their UV triangles (protecting Maria's `1245` fingernails, `scu.mdl` `1282`, `pap.mdl` `1293`,
///   and minor `< 2%` UV-seam grazes on solid clothing such as `hhh_mar` `prim[21]`).
pub fn is_opaque_cutout_primitive(
    model: &ModelFile,
    prim_idx: usize,
    decoded_textures: &DecodedModelTextureMap,
) -> bool {
    let Some(prim) = model.primitives.get(prim_idx) else {
        return false;
    };
    if prim.is_transparent || prim.material_type >= 3 {
        return false;
    }

    let tid = prim.texture_index as u32;
    if let Some((w, h, rgba)) = decoded_textures.get(&tid) {
        let tris = model.primitive_triangles(prim_idx);
        let mut total_samples = 0usize;
        let mut zero_alpha_samples = 0usize;
        let mut solid_alpha_samples = 0usize;

        for &[i0, i1, i2] in &tris {
            let (Some(&uv0), Some(&uv1), Some(&uv2)) = (
                model.vertices.uvs.get(i0 as usize),
                model.vertices.uvs.get(i1 as usize),
                model.vertices.uvs.get(i2 as usize),
            ) else {
                continue;
            };
            let centroid = [
                (uv0[0] + uv1[0] + uv2[0]) * (1.0 / 3.0),
                (uv0[1] + uv1[1] + uv2[1]) * (1.0 / 3.0),
            ];

            for uv in [uv0, uv1, uv2, centroid] {
                let u_wrapped = uv[0].rem_euclid(1.0);
                let v_wrapped = uv[1].rem_euclid(1.0);
                let px = ((u_wrapped * (*w as f32)) as usize).min(w.saturating_sub(1));
                let py = ((v_wrapped * (*h as f32)) as usize).min(h.saturating_sub(1));
                let alpha = rgba[(py * (*w) + px) * 4 + 3];
                total_samples += 1;
                if alpha == 0 {
                    zero_alpha_samples += 1;
                } else if alpha >= 240 {
                    solid_alpha_samples += 1;
                }
            }
        }

        return total_samples > 0
            && solid_alpha_samples > 0
            && zero_alpha_samples * 20 >= total_samples;
    }

    // Fallback for unhydrated `no_tex = 1` models when embedded textures were not pre-attached:
    // double-sided (`backface_culling == 0`) non-specular primitives that do not share a texture ID
    // with any `material_type >= 3` gloss-mask submesh (e.g., `rlll_mar` / `rlxx_mar` `tid = 1246`).
    if model.textures.is_empty() && prim.backface_culling == 0 {
        let shares_gloss_tex = model
            .primitives
            .iter()
            .any(|other| other.texture_index == prim.texture_index && other.material_type >= 3);
        return !shares_gloss_tex;
    }

    false
}

/// Dilates RGB colors from non-zero-alpha (`A > 0`) pixels outward into adjacent fully transparent
/// (`A == 0`) pixels across `passes` 8-connected iterations while keeping `A == 0` unchanged.
///
/// Prevents hardware bilinear texture filtering (`wgpu::FilterMode::Linear`) from bleeding black
/// `(0, 0, 0, 0)` background texels into hair/eyelash fringes (e.g., `lll_mar` / `lxx_mar` `1246`,
/// `lll_jms` `1241`, `4K 1252.dds` Eddie hair, and `4K 1258.dds` Laura hair). Leaves all `A > 0`
/// texels (including specular gloss-mask textures) 100% bit-identical.
pub fn dilate_transparent_border_rgb(width: u32, height: u32, rgba: &mut [u8], passes: usize) {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || passes == 0 || rgba.len() != w * h * 4 {
        return;
    }
    if !rgba.as_chunks::<4>().0.iter().any(|px| px[3] == 0) {
        return;
    }

    let mut visited_non_zero = vec![false; w * h];
    for (idx, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        visited_non_zero[idx] = px[3] > 0;
    }

    let mut updates: Vec<(usize, [u8; 3])> = Vec::new();
    const OFFSETS: [(isize, isize); 8] = [
        (-1, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
    ];

    for _ in 0..passes {
        updates.clear();
        for y in 0..h {
            let row_base = y * w;
            for x in 0..w {
                let idx = row_base + x;
                if visited_non_zero[idx] {
                    continue;
                }
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut weight_sum = 0u32;

                for &(dx, dy) in &OFFSETS {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let n_idx = (ny as usize) * w + (nx as usize);
                    if visited_non_zero[n_idx] {
                        let base = n_idx * 4;
                        let w_alpha = (rgba[base + 3] as u32).max(1);
                        sum_r += (rgba[base] as u32) * w_alpha;
                        sum_g += (rgba[base + 1] as u32) * w_alpha;
                        sum_b += (rgba[base + 2] as u32) * w_alpha;
                        weight_sum += w_alpha;
                    }
                }

                let half = weight_sum / 2;
                if let (Some(r), Some(g), Some(b)) = (
                    (sum_r + half).checked_div(weight_sum),
                    (sum_g + half).checked_div(weight_sum),
                    (sum_b + half).checked_div(weight_sum),
                ) {
                    updates.push((idx, [r as u8, g as u8, b as u8]));
                }
            }
        }

        if updates.is_empty() {
            break;
        }
        for &(idx, rgb) in &updates {
            let base = idx * 4;
            rgba[base] = rgb[0];
            rgba[base + 1] = rgb[1];
            rgba[base + 2] = rgb[2];
            // Keep `rgba[base + 3] == 0` strictly unchanged; mark `visited_non_zero` so subsequent
            // dilation rings propagate the hair color another texel outward.
            visited_non_zero[idx] = true;
        }
    }
}

/// Computes the UV-space-equivalent border dilation pass count for a texture of dimensions
/// `(dst_w, dst_h)` relative to its vanilla `(van_w, van_h)` base dimensions and `base_passes`.
///
/// Scales `base_passes` linearly by `ceil(max(dst_w / van_w, dst_h / van_h))` (e.g., `2` passes at
/// 1x vanilla -> `8` passes at 4x 4K) so hardware bilinear filtering, 4x4 BC3 compression blocks,
/// and mip levels 1–2 never bleed dark `(0, 0, 0, 0)` gutter texels into hair/fringe edges.
#[must_use]
pub fn scaled_border_dilation_passes(
    van_w: u32,
    van_h: u32,
    dst_w: u32,
    dst_h: u32,
    base_passes: usize,
) -> usize {
    if base_passes == 0 || van_w == 0 || van_h == 0 {
        return base_passes;
    }
    let scale_x = dst_w.div_ceil(van_w).max(1) as usize;
    let scale_y = dst_h.div_ceil(van_h).max(1) as usize;
    base_passes.saturating_mul(scale_x.max(scale_y))
}

/// Transfers SH2:EE RGB color corrections (such as Maria's restored pink hair tips on `tid = 1246`
/// in `sh2e/chr/mar/lll_mar.mdl` and `sh2e/chr2/mar/lxx_mar.mdl`) onto a 4K AI Master override
/// texture while preserving 4K strand detail, reconstructs clean binary cutout alpha from the
/// vanilla silhouette to eliminate offline-upscaler / DXT5 stippled hair-tip shredding, clamps
/// stray upscaler alpha specks outside smooth-alpha silhouettes (e.g., `hhh_mar` `tid = 1247`),
/// and strips `(255, 255, 255, 0)` / `(0, 0, 0, 0)` gutter bleed along cutout borders.
///
/// Only invoked when the 4K Textures Options toggle is active; strict 1:1 Vanilla PS2 textures are
/// never modified by this function.
pub fn refine_4k_chr_texture_from_sources(
    van_w: u32,
    van_h: u32,
    van_raw_rgba: &[u8],
    sh2e_raw: Option<(u32, u32, &[u8])>,
    dst_w: u32,
    dst_h: u32,
    dst_4k_rgba: &mut [u8],
) {
    let vw = van_w as usize;
    let vh = van_h as usize;
    let dw = dst_w as usize;
    let dh = dst_h as usize;
    if vw == 0
        || vh == 0
        || dw == 0
        || dh == 0
        || van_raw_rgba.len() != vw * vh * 4
        || dst_4k_rgba.len() != dw * dh * 4
    {
        return;
    }

    let van_pixels = van_raw_rgba.as_chunks::<4>().0;
    let has_zero_alpha = van_pixels.iter().any(|px| px[3] == 0);
    let mid_alpha_count = van_pixels
        .iter()
        .filter(|px| px[3] > 0 && px[3] < 240)
        .count();
    let is_binary_cutout_source = has_zero_alpha && mid_alpha_count * 100 < van_pixels.len();

    let sh2e_edit = sh2e_raw.and_then(|(sw, sh, s_rgba)| {
        let sw_u = sw as usize;
        let sh_u = sh as usize;
        if sw_u == 0 || sh_u == 0 || s_rgba.len() != sw_u * sh_u * 4 {
            return None;
        }
        let differs_rgb = if sw == van_w && sh == van_h {
            van_pixels
                .iter()
                .zip(s_rgba.as_chunks::<4>().0.iter())
                .any(|(v, s)| v[0] != s[0] || v[1] != s[1] || v[2] != s[2])
        } else {
            // Compare SH2:EE RGB at each vanilla texel center when SH2:EE has different dimensions
            // (e.g., 2x HD SH2:EE character texture vs 1x vanilla `.mdl`).
            let sx_step = (sw_u as f32) / (vw as f32);
            let sy_step = (sh_u as f32) / (vh as f32);
            let mut found = false;
            'outer: for vy in 0..vh {
                let sy = (((vy as f32 + 0.5) * sy_step - 0.5)
                    .round()
                    .clamp(0.0, (sh_u - 1) as f32)) as usize;
                for vx in 0..vw {
                    let sx = (((vx as f32 + 0.5) * sx_step - 0.5)
                        .round()
                        .clamp(0.0, (sw_u - 1) as f32)) as usize;
                    let vi = (vy * vw + vx) * 4;
                    let si = (sy * sw_u + sx) * 4;
                    if van_raw_rgba[vi] != s_rgba[si]
                        || van_raw_rgba[vi + 1] != s_rgba[si + 1]
                        || van_raw_rgba[vi + 2] != s_rgba[si + 2]
                    {
                        found = true;
                        break 'outer;
                    }
                }
            }
            found
        };
        differs_rgb.then_some((sw, sh, s_rgba))
    });

    if !has_zero_alpha && sh2e_edit.is_none() {
        return;
    }

    let mut van_dil = van_raw_rgba.to_vec();
    if is_binary_cutout_source || sh2e_edit.is_some() {
        dilate_transparent_border_rgb(van_w, van_h, &mut van_dil, 6);
    }

    let sh2e_dil = sh2e_edit.map(|(sw, sh, s_rgba)| {
        let mut buf = s_rgba.to_vec();
        let sh2e_passes = scaled_border_dilation_passes(van_w, van_h, sw, sh, 6).min(24);
        dilate_transparent_border_rgb(sw, sh, &mut buf, sh2e_passes);
        (sw as usize, sh as usize, buf)
    });

    // Exact 8-bit UNORM midpoints:
    // - `BINARY_ALPHA_ISOCONTOUR = 127.5` (`255.0 * 0.5`): 0.5 level-set of the bilinearly
    //   reconstructed vanilla `{0, 255}` binary silhouette mask, producing smooth unbroken 4K hair
    //   strands that match vanilla's exact `{0, 255}` alpha distribution (passing `TransparentCore`
    //   `>= PS2_ALPHA_OPAQUE_THRESHOLD` without DXT5 stipple shredding).
    // - `TRANSPARENT_OUTSIDE_EPS = 0.5` (`PS2_ALPHA_TEST_THRESHOLD * 255.0`): strictly `0` across
    //   all four surrounding vanilla texels (`SLUS_202.28` `TEST_1 = 0x5000d` `ATST=GREATER, AREF=0`).
    const BINARY_ALPHA_ISOCONTOUR: f32 = 127.5;
    const TRANSPARENT_OUTSIDE_EPS: f32 = 0.5;

    let scale_x = (vw as f32) / (dw as f32);
    let scale_y = (vh as f32) / (dh as f32);

    for y4 in 0..dh {
        let sy = ((y4 as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (vh - 1) as f32);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(vh - 1);
        let fy = sy - (y0 as f32);

        let sh2e_y = sh2e_dil.as_ref().map(|(_, sh_u, _)| {
            let s_scale_y = (*sh_u as f32) / (dh as f32);
            let ssy = ((y4 as f32 + 0.5) * s_scale_y - 0.5).clamp(0.0, (*sh_u - 1) as f32);
            let sy0 = ssy.floor() as usize;
            let sy1 = (sy0 + 1).min(*sh_u - 1);
            (sy0, sy1, ssy - (sy0 as f32))
        });

        for x4 in 0..dw {
            let sx = ((x4 as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (vw - 1) as f32);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(vw - 1);
            let fx = sx - (x0 as f32);

            let dst_base = (y4 * dw + x4) * 4;
            let mut rgb = [
                dst_4k_rgba[dst_base] as f32,
                dst_4k_rgba[dst_base + 1] as f32,
                dst_4k_rgba[dst_base + 2] as f32,
            ];

            let src_a = if has_zero_alpha {
                bilerp_alpha(van_raw_rgba, vw, x0, y0, x1, y1, fx, fy)
            } else {
                255.0
            };

            let van_rgb = if is_binary_cutout_source || sh2e_dil.is_some() {
                bilerp_rgb(&van_dil, vw, x0, y0, x1, y1, fx, fy)
            } else {
                rgb
            };

            if is_binary_cutout_source {
                // Reconstruct clean binary cutout alpha from the bilinearly upscaled vanilla mask,
                // eliminating offline-upscaler / DXT5 stippled dots and noisy `1..=254` halos.
                let clean_a = if src_a >= BINARY_ALPHA_ISOCONTOUR {
                    255u8
                } else {
                    0u8
                };
                dst_4k_rgba[dst_base + 3] = clean_a;

                if clean_a == 0 {
                    // Fill transparent gutter with clean dilated vanilla RGB (then SH2:EE tinted below)
                    // so bilinear filtering, 4x4 BC3 blocks, and mipmaps never bleed black `(0,0,0)`.
                    rgb = van_rgb;
                } else if src_a < 254.0 {
                    // Decontaminate offline-upscaler dark/white gutter bleed along cutout borders.
                    let w_clean = ((254.0 - src_a) * (1.0 / 50.0)).clamp(0.0, 1.0);
                    for c in 0..3 {
                        rgb[c] = rgb[c] * (1.0 - w_clean) + van_rgb[c] * w_clean;
                    }
                }
            } else if has_zero_alpha && src_a < TRANSPARENT_OUTSIDE_EPS {
                // Smooth-alpha cutout texture (e.g., `hhh_mar` `tid = 1247`): clamp hallucinated
                // 4K upscaler background specks outside the vanilla silhouette back to `alpha == 0`.
                dst_4k_rgba[dst_base + 3] = 0;
            }

            if let (Some((sw_u, _, ref s_dil)), Some((sy0, sy1, sfy))) = (&sh2e_dil, sh2e_y) {
                let s_scale_x = (*sw_u as f32) / (dw as f32);
                let ssx = ((x4 as f32 + 0.5) * s_scale_x - 0.5).clamp(0.0, (*sw_u - 1) as f32);
                let sx0 = ssx.floor() as usize;
                let sx1 = (sx0 + 1).min(*sw_u - 1);
                let sfx = ssx - (sx0 as f32);

                let sh2e_rgb = bilerp_rgb(s_dil, *sw_u, sx0, sy0, sx1, sy1, sfx, sfy);
                for c in 0..3 {
                    let ratio = (sh2e_rgb[c] + 12.0) / (van_rgb[c] + 12.0);
                    rgb[c] = (rgb[c] * ratio).clamp(0.0, 255.0);
                }
            }

            dst_4k_rgba[dst_base] = rgb[0].round().clamp(0.0, 255.0) as u8;
            dst_4k_rgba[dst_base + 1] = rgb[1].round().clamp(0.0, 255.0) as u8;
            dst_4k_rgba[dst_base + 2] = rgb[2].round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn bilerp_rgb(
    rgba: &[u8],
    width: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    fx: f32,
    fy: f32,
) -> [f32; 3] {
    let i00 = (y0 * width + x0) * 4;
    let i10 = (y0 * width + x1) * 4;
    let i01 = (y1 * width + x0) * 4;
    let i11 = (y1 * width + x1) * 4;
    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;
    [
        rgba[i00] as f32 * w00
            + rgba[i10] as f32 * w10
            + rgba[i01] as f32 * w01
            + rgba[i11] as f32 * w11,
        rgba[i00 + 1] as f32 * w00
            + rgba[i10 + 1] as f32 * w10
            + rgba[i01 + 1] as f32 * w01
            + rgba[i11 + 1] as f32 * w11,
        rgba[i00 + 2] as f32 * w00
            + rgba[i10 + 2] as f32 * w10
            + rgba[i01 + 2] as f32 * w01
            + rgba[i11 + 2] as f32 * w11,
    ]
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn bilerp_alpha(
    rgba: &[u8],
    width: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    fx: f32,
    fy: f32,
) -> f32 {
    let a00 = rgba[(y0 * width + x0) * 4 + 3] as f32;
    let a10 = rgba[(y0 * width + x1) * 4 + 3] as f32;
    let a01 = rgba[(y1 * width + x0) * 4 + 3] as f32;
    let a11 = rgba[(y1 * width + x1) * 4 + 3] as f32;
    (a00 * (1.0 - fx) + a10 * fx) * (1.0 - fy) + (a01 * (1.0 - fx) + a11 * fx) * fy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dilate_transparent_border_rgb_fills_black_zero_alpha_gutter_without_altering_alpha() {
        // 3x1 strip: [blonde hair (120, 96, 64, 255), black cutout (0, 0, 0, 0), black cutout (0, 0, 0, 0)]
        let mut rgba = vec![
            120, 96, 64, 255, // x = 0 (hair)
            0, 0, 0, 0, // x = 1 (1-texel gutter)
            0, 0, 0, 0, // x = 2 (2-texel gutter)
        ];
        dilate_transparent_border_rgb(3, 1, &mut rgba, 2);

        // Pixel 0 unchanged
        assert_eq!(&rgba[0..4], &[120, 96, 64, 255]);
        // Pixels 1 & 2 inherit hair RGB while keeping alpha == 0
        assert_eq!(&rgba[4..8], &[120, 96, 64, 0]);
        assert_eq!(&rgba[8..12], &[120, 96, 64, 0]);
    }

    #[test]
    fn refine_4k_chr_texture_transfers_sh2e_pink_tips_and_cleans_cutout_gutter() {
        // 2x1 vanilla: [blonde tip (168, 148, 110, 255), black cutout gutter (0, 0, 0, 0)]
        let van = vec![168, 148, 110, 255, 0, 0, 0, 0];
        // 2x1 SH2:EE: [pink-dyed tip (168, 108, 94, 255), black cutout gutter (0, 0, 0, 0)]
        let sh2e = vec![168, 108, 94, 255, 0, 0, 0, 0];
        // 4x1 4K upscaled from vanilla with dark gutter bleed on pixel 1 (`alpha = 140`, darkened RGB)
        let mut dst_4k = vec![
            170, 150, 112, 255, // interior strand
            60, 50, 35, 140, // upscaler gutter-bled fringe
            0, 0, 0, 0, // cutout
            0, 0, 0, 0, // cutout
        ];

        refine_4k_chr_texture_from_sources(
            2,
            1,
            &van,
            Some((2, 1, sh2e.as_slice())),
            4,
            1,
            &mut dst_4k,
        );

        // Interior pixel gains SH2:EE's rose-pink ratio (`R` stays ~170, `G` drops ~110)
        assert!(dst_4k[0] >= 165);
        assert!(dst_4k[1] <= 115);
        assert!(
            dst_4k[0] as i32 - dst_4k[1] as i32 >= 50,
            "Expected rose-pink R-G spread >= 50, got {:?}",
            &dst_4k[0..4]
        );
        // Fringe pixel (`sx = 0.25`, `src_a = 191.25 >= 127.5`) is decontaminated from `(60, 50, 35)`
        // up to bright pink-blonde AND its noisy upscaler alpha (`140`) is reconstructed cleanly to `255`!
        assert!(
            dst_4k[4] >= 150,
            "Expected gutter-decontaminated red >= 150, got {:?}",
            &dst_4k[4..8]
        );
        assert_eq!(dst_4k[7], 255);
        // Cutout pixels (`sx = 0.75`, `src_a = 63.75 < 127.5`) remain strictly `alpha == 0` with
        // clean dilated hair RGB instead of black `(0, 0, 0)`.
        assert_eq!(dst_4k[11], 0);
        assert!(dst_4k[8] >= 150);
    }

    #[test]
    fn scaled_border_dilation_passes_scales_proportionally_for_hd_and_4k() {
        assert_eq!(scaled_border_dilation_passes(1024, 512, 1024, 512, 2), 2);
        assert_eq!(scaled_border_dilation_passes(1024, 512, 2048, 1024, 2), 4);
        assert_eq!(scaled_border_dilation_passes(1024, 512, 4096, 2048, 2), 8);
        assert_eq!(scaled_border_dilation_passes(256, 256, 1024, 1024, 2), 8);
        assert_eq!(scaled_border_dilation_passes(0, 0, 1024, 1024, 2), 2);
        assert_eq!(scaled_border_dilation_passes(256, 256, 1024, 1024, 0), 0);
    }

    #[test]
    fn refine_4k_chr_texture_transfers_sh2e_colors_when_sh2e_resolution_differs_from_vanilla() {
        // 2x1 vanilla: [blonde (168, 148, 110, 255), cutout (0, 0, 0, 0)]
        let van = vec![168, 148, 110, 255, 0, 0, 0, 0];
        // 4x1 SH2:EE (2x wider than vanilla): pink-dyed tips across pixels 0..2, cutout at 2..4
        let sh2e_2x = vec![
            168, 108, 94, 255, // x = 0
            168, 108, 94, 255, // x = 1
            0, 0, 0, 0, // x = 2
            0, 0, 0, 0, // x = 3
        ];
        // 8x1 4K (4x wider than vanilla)
        let mut dst_4k = vec![
            170, 150, 112, 255, // 0
            170, 150, 112, 255, // 1
            170, 150, 112, 240, // 2 (noisy upscaler alpha)
            60, 50, 35, 90, // 3 (darkened upscaler fringe)
            0, 0, 0, 40, // 4 (stray upscaler alpha dot in cutout)
            0, 0, 0, 0, // 5
            0, 0, 0, 0, // 6
            0, 0, 0, 0, // 7
        ];

        refine_4k_chr_texture_from_sources(
            2,
            1,
            &van,
            Some((4, 1, sh2e_2x.as_slice())),
            8,
            1,
            &mut dst_4k,
        );

        // SH2:EE rose-pink tint transferred despite `4x1 != 2x1` resolution mismatch
        assert!(dst_4k[0] >= 165);
        assert!(dst_4k[1] <= 115);
        assert!(dst_4k[0] as i32 - dst_4k[1] as i32 >= 50);
        // Binary silhouette reconstructed cleanly: pixels 0..4 are solid 255, pixels 4..8 are 0
        for i in 0..4 {
            assert_eq!(dst_4k[i * 4 + 3], 255, "pixel {i} should be opaque 255");
        }
        for i in 4..8 {
            assert_eq!(dst_4k[i * 4 + 3], 0, "pixel {i} should be cutout 0");
        }
    }

    #[test]
    fn refine_4k_chr_texture_clamps_stray_upscaler_specks_on_smooth_alpha_textures() {
        // 4x1 smooth-alpha vanilla (`tid = 1247` style): 50% intermediate alpha -> not binary cutout
        let van = vec![
            160, 130, 100, 200, // x = 0 (smooth hair)
            160, 130, 100, 120, // x = 1 (soft fringe)
            0, 0, 0, 0, // x = 2 (transparent background)
            0, 0, 0, 0, // x = 3 (transparent background)
        ];
        // 8x1 4K with intact smooth hair at 0..4 and a hallucinated white speck at pixel 6 (`a = 180`)
        let mut dst_4k = vec![
            160, 130, 100, 205, // 0
            160, 130, 100, 195, // 1
            160, 130, 100, 140, // 2
            160, 130, 100, 95, // 3
            0, 0, 0, 0, // 4
            0, 0, 0, 0, // 5
            255, 255, 255, 180, // 6: hallucinated speck strictly outside vanilla silhouette
            0, 0, 0, 0, // 7
        ];

        refine_4k_chr_texture_from_sources(4, 1, &van, None, 8, 1, &mut dst_4k);

        // Interior smooth alpha preserved bit-identically
        assert_eq!(dst_4k[3], 205);
        assert_eq!(dst_4k[11], 140);
        // Hallucinated background speck strictly outside vanilla silhouette clamped to 0
        assert_eq!(dst_4k[6 * 4 + 3], 0);
    }
}
