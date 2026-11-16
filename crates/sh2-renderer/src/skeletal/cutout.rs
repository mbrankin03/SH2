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
}
