//! Alpha-coverage-preserving mipmap generation (Castano's method) for RGBA8 textures (`SH2-92`).

/// Computes the number of mipmap levels for a 2D texture of dimensions `(width, height)`.
pub fn mip_level_count_for_size(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    32 - max_dim.leading_zeros()
}

/// Alpha-test cutout threshold (`128` out of `255` = `0.5` normalized) used when preserving
/// cutout alpha coverage across mipmap levels (`SH2-92`).
pub const ALPHA_CUTOUT_MIP_REF_THRESHOLD: u8 = 128;

/// Computes the fraction `[0.0, 1.0]` of texels in an RGBA8 buffer whose alpha channel is `>= threshold`.
pub fn compute_alpha_coverage_rgba(rgba: &[u8], threshold: u8) -> f32 {
    let total_px = rgba.len() / 4;
    if total_px == 0 {
        return 1.0;
    }
    let mut passing = 0usize;
    for px in rgba.as_chunks::<4>().0 {
        if px[3] >= threshold {
            passing += 1;
        }
    }
    (passing as f32) / (total_px as f32)
}

/// Rescales the alpha channel of `rgba` via binary search so that its cutout coverage at `threshold`
/// matches `target_coverage` (Castano's alpha-to-coverage mipmap preservation method).
///
/// Prevents distant chain-link fences, grate floors, tree foliage, and character hair fringes from
/// thinning out or vanishing at higher mip levels under 16x anisotropic filtering.
pub fn preserve_alpha_coverage_in_place(rgba: &mut [u8], target_coverage: f32, threshold: u8) {
    let total_px = rgba.len() / 4;
    if total_px == 0 || target_coverage <= 0.0 || target_coverage >= 1.0 {
        return;
    }

    let current_cov = compute_alpha_coverage_rgba(rgba, threshold);
    if (current_cov - target_coverage).abs() <= (1.0 / (total_px as f32).max(1.0)) {
        return;
    }

    // Binary search for a multiplier `scale` in `[0.25, 8.0]` that brings coverage closest to `target_coverage`.
    // If all texels fell below `threshold` after uniform box averaging (so `current_cov == 0.0`),
    // ensure the lower bound starts at the exact scale needed to lift the peak alpha in this mip level
    // back to `threshold`.
    let max_a = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| px[3])
        .max()
        .unwrap_or(0);
    if max_a == 0 {
        return;
    }

    let min_lift = (threshold as f32) / (max_a as f32);
    let mut lo = 0.25f32;
    let mut hi = (4.0f32).max(min_lift * 1.5);
    let mut best_scale = min_lift.max(1.0);
    let mut best_diff = (current_cov - target_coverage).abs();

    for _ in 0..12 {
        let mid = 0.5 * (lo + hi);
        let mut passing = 0usize;
        for px in rgba.as_chunks::<4>().0 {
            let scaled_a = (px[3] as f32 * mid).round().clamp(0.0, 255.0) as u8;
            if scaled_a >= threshold {
                passing += 1;
            }
        }
        let cov = (passing as f32) / (total_px as f32);
        let diff = (cov - target_coverage).abs();
        if diff < best_diff || (cov > 0.0 && best_diff == target_coverage) {
            best_diff = diff;
            best_scale = mid;
        }
        if cov < target_coverage {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    if (best_scale - 1.0).abs() > 1e-3 {
        for px in rgba.as_chunks_mut::<4>().0 {
            px[3] = (px[3] as f32 * best_scale).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Generates a full box-filtered RGBA8 mipmap chain from level-0 `rgba` pixels.
///
/// Preserves cutout alpha coverage (`ALPHA_CUTOUT_MIP_REF_THRESHOLD`) and weights RGB downsampling
/// by alpha so transparent cutout regions neither thin out at distance nor bleed dark borders into
/// higher mip levels when sampled with 16x anisotropic filtering.
pub fn generate_mipmap_chain_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Vec<(u32, u32, Vec<u8>)> {
    let w0 = width.max(1);
    let h0 = height.max(1);
    let levels = mip_level_count_for_size(w0, h0) as usize;
    let mut chain = Vec::with_capacity(levels);

    let expected0 = (w0 as usize) * (h0 as usize) * 4;
    let mip0 = if rgba.len() >= expected0 {
        rgba[..expected0].to_vec()
    } else {
        vec![255u8; expected0]
    };
    let mip0_coverage = compute_alpha_coverage_rgba(&mip0, ALPHA_CUTOUT_MIP_REF_THRESHOLD);
    // Only apply Castano cutout coverage scaling when the texture actually contains transparent
    // cutout texels (`alpha < 4`). Non-cutout DXT5 textures that store specular/gloss intensity
    // in the alpha channel (where `min_alpha > 0` across the entire surface, such as Pyramid Head's
    // helmet `1303`/`1307`, Great Knife `1305`, or Lying Figure `1282`) must not have their alpha
    // rescaled toward `ALPHA_CUTOUT_MIP_REF_THRESHOLD` across mip levels.
    let has_transparent_cutout_texels = mip0.as_chunks::<4>().0.iter().any(|px| px[3] < 4);
    let has_cutout_alpha =
        has_transparent_cutout_texels && mip0_coverage > 0.0 && mip0_coverage < 1.0;
    chain.push((w0, h0, mip0));

    for lvl in 1..levels {
        let (prev_w, prev_h, ref prev_data) = chain[lvl - 1];
        let next_w = (prev_w / 2).max(1);
        let next_h = (prev_h / 2).max(1);
        let mut next_data = vec![0u8; (next_w as usize) * (next_h as usize) * 4];

        let pw = prev_w as usize;
        let ph = prev_h as usize;
        let nw = next_w as usize;
        let nh = next_h as usize;

        for y in 0..nh {
            let sy0 = (y * 2).min(ph - 1);
            let sy1 = (y * 2 + 1).min(ph - 1);
            for x in 0..nw {
                let sx0 = (x * 2).min(pw - 1);
                let sx1 = (x * 2 + 1).min(pw - 1);

                let idx00 = (sy0 * pw + sx0) * 4;
                let idx10 = (sy0 * pw + sx1) * 4;
                let idx01 = (sy1 * pw + sx0) * 4;
                let idx11 = (sy1 * pw + sx1) * 4;

                let samples = [idx00, idx10, idx01, idx11];
                let mut sum_a = 0u32;
                let mut sum_r = 0f32;
                let mut sum_g = 0f32;
                let mut sum_b = 0f32;
                let mut weight_sum = 0f32;

                for &s_ofs in &samples {
                    let a = prev_data[s_ofs + 3];
                    sum_a += a as u32;
                    let w = (a as f32).max(1.0);
                    sum_r += prev_data[s_ofs] as f32 * w;
                    sum_g += prev_data[s_ofs + 1] as f32 * w;
                    sum_b += prev_data[s_ofs + 2] as f32 * w;
                    weight_sum += w;
                }

                let dst_ofs = (y * nw + x) * 4;
                next_data[dst_ofs] = (sum_r / weight_sum).round().clamp(0.0, 255.0) as u8;
                next_data[dst_ofs + 1] = (sum_g / weight_sum).round().clamp(0.0, 255.0) as u8;
                next_data[dst_ofs + 2] = (sum_b / weight_sum).round().clamp(0.0, 255.0) as u8;
                next_data[dst_ofs + 3] = ((sum_a + 2) / 4) as u8;
            }
        }

        if has_cutout_alpha {
            preserve_alpha_coverage_in_place(
                &mut next_data,
                mip0_coverage,
                ALPHA_CUTOUT_MIP_REF_THRESHOLD,
            );
        }

        chain.push((next_w, next_h, next_data));
    }

    chain
}
