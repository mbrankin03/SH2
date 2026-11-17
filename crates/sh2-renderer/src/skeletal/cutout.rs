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

/// Transfers SH2:EE RGB color corrections (such as Maria's restored pink hair tips on `tid = 1246`
/// in `sh2e/chr/mar/lll_mar.mdl` and `sh2e/chr2/mar/lxx_mar.mdl`) onto a 4K AI Master override
/// texture while preserving 4K strand detail, and strips offline-upscaler `(255,255,255,0)` /
/// `(0,0,0,0)` gutter bleed along binary cutout borders.
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

    let sh2e_edit_slice = sh2e_raw.and_then(|(sw, sh, s_rgba)| {
        if sw == van_w && sh == van_h && s_rgba.len() == van_raw_rgba.len() {
            let differs_rgb = van_pixels
                .iter()
                .zip(s_rgba.as_chunks::<4>().0.iter())
                .any(|(v, s)| v[0] != s[0] || v[1] != s[1] || v[2] != s[2]);
            differs_rgb.then_some(s_rgba)
        } else {
            None
        }
    });

    if !is_binary_cutout_source && sh2e_edit_slice.is_none() {
        return;
    }

    let mut van_dil = van_raw_rgba.to_vec();
    dilate_transparent_border_rgb(van_w, van_h, &mut van_dil, 6);

    let sh2e_dil = sh2e_edit_slice.map(|s_rgba| {
        let mut buf = s_rgba.to_vec();
        dilate_transparent_border_rgb(van_w, van_h, &mut buf, 6);
        buf
    });

    let scale_x = (vw as f32) / (dw as f32);
    let scale_y = (vh as f32) / (dh as f32);

    for y4 in 0..dh {
        let sy = ((y4 as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (vh - 1) as f32);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(vh - 1);
        let fy = sy - (y0 as f32);

        for x4 in 0..dw {
            let sx = ((x4 as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (vw - 1) as f32);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(vw - 1);
            let fx = sx - (x0 as f32);

            let van_rgb = bilerp_rgb(&van_dil, vw, x0, y0, x1, y1, fx, fy);
            let dst_base = (y4 * dw + x4) * 4;
            let mut rgb = [
                dst_4k_rgba[dst_base] as f32,
                dst_4k_rgba[dst_base + 1] as f32,
                dst_4k_rgba[dst_base + 2] as f32,
            ];

            if is_binary_cutout_source {
                let src_a = bilerp_alpha(van_raw_rgba, vw, x0, y0, x1, y1, fx, fy);
                if src_a < 254.0 {
                    let w_clean = ((254.0 - src_a) * (1.0 / 50.0)).clamp(0.0, 1.0);
                    for c in 0..3 {
                        rgb[c] = rgb[c] * (1.0 - w_clean) + van_rgb[c] * w_clean;
                    }
                }
            }

            if let Some(ref s_dil) = sh2e_dil {
                let sh2e_rgb = bilerp_rgb(s_dil, vw, x0, y0, x1, y1, fx, fy);
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
        // Fringe pixel (`alpha = 140`) is decontaminated from `(60, 50, 35)` up to bright pink-blonde!
        assert!(
            dst_4k[4] >= 150,
            "Expected gutter-decontaminated red >= 150, got {:?}",
            &dst_4k[4..8]
        );
        assert_eq!(dst_4k[7], 140);
    }
}
