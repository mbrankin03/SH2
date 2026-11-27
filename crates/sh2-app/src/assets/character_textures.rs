//! Single-tier character/enemy GPU texture decoding, SH2:EE / 4K refinement, and upscaling (`SH2-181`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use sh2_assets::formats::mdls::{companion_texture_mdl_path, parse_mdl, ModelTexture};
use sh2_assets::formats::TextureLoader;
use sh2_core::config::WorldTextureQuality;
use sh2_renderer::RenderContext;

/// Generates a high-resolution procedural HD inspection variation from an RGBA8 texture
/// when an external SH2:EE `.dds`/`.png` replacement texture is not found on disk,
/// allowing side-by-side filtering & detail comparison in the Model Viewer.
pub(crate) fn upscale_hd_preview_rgba(
    rgba: &[u8],
    width: usize,
    height: usize,
) -> (u32, u32, Vec<u8>) {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return (1, 1, vec![255, 255, 255, 255]);
    }
    let out_w = width * 2;
    let out_h = height * 2;
    let mut out = vec![0u8; out_w * out_h * 4];

    for y in 0..out_h {
        let sy = (y / 2).min(height - 1);
        let sy_next = (sy + 1).min(height - 1);
        let fy = if y % 2 == 1 { 0.5f32 } else { 0.0f32 };

        for x in 0..out_w {
            let sx = (x / 2).min(width - 1);
            let sx_next = (sx + 1).min(width - 1);
            let fx = if x % 2 == 1 { 0.5f32 } else { 0.0f32 };

            let p00 = (sy * width + sx) * 4;
            let p10 = (sy * width + sx_next) * 4;
            let p01 = (sy_next * width + sx) * 4;
            let p11 = (sy_next * width + sx_next) * 4;

            let dst = (y * out_w + x) * 4;
            let a00 = rgba[p00 + 3] as f32;
            let a10 = rgba[p10 + 3] as f32;
            let a01 = rgba[p01 + 3] as f32;
            let a11 = rgba[p11 + 3] as f32;

            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;

            let out_a = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
            out[dst + 3] = out_a.clamp(0.0, 255.0) as u8;

            // Weight RGB interpolation by alpha when mixing cutout (a == 0) and visible texels
            // so transparent cutout texels never darken or fringe adjacent hair/alpha edges.
            let aw00 = w00 * a00.max(1.0);
            let aw10 = w10 * a10.max(1.0);
            let aw01 = w01 * a01.max(1.0);
            let aw11 = w11 * a11.max(1.0);
            let aw_sum = (aw00 + aw10 + aw01 + aw11).max(1e-5);

            for c in 0..3 {
                let v00 = rgba[p00 + c] as f32;
                let v10 = rgba[p10 + c] as f32;
                let v01 = rgba[p01 + c] as f32;
                let v11 = rgba[p11 + c] as f32;
                let mut val = (v00 * aw00 + v10 * aw10 + v01 * aw01 + v11 * aw11) / aw_sum;
                // Subtle micro-contrast sharpening on RGB channels only when all 4 neighbors are non-transparent
                if a00 > 0.0 && a10 > 0.0 && a01 > 0.0 && a11 > 0.0 {
                    let lap = v00 * 4.0 - (v10 + v01 + v11 + v00);
                    val = (val + lap * 0.15).clamp(0.0, 255.0);
                }
                out[dst + c] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }

    (out_w as u32, out_h as u32, out)
}

/// Reloads embedded vanilla textures from disk (plus companion `.mdl` textures for `no_tex = 1`
/// variants) for on-demand character texture tier switching (`SH2-181`).
pub(super) fn load_vanilla_model_textures(
    base_root: &Path,
    mdl_rel: &str,
) -> HashMap<u32, ModelTexture> {
    let van_rel = mdl_rel
        .strip_prefix("sh2e/")
        .map(|s| format!("data/{s}"))
        .unwrap_or_else(|| mdl_rel.to_string());
    let primary_path = base_root.join(&van_rel);
    let bytes = std::fs::read(&primary_path)
        .or_else(|_| std::fs::read(base_root.join(mdl_rel)))
        .ok();
    let Some(mut model) = bytes.and_then(|b| parse_mdl(&b).ok()) else {
        return HashMap::new();
    };
    if model.textures.is_empty() {
        if let Some(comp_rel) = companion_texture_mdl_path(mdl_rel) {
            if let Ok(comp_bytes) = std::fs::read(base_root.join(comp_rel)) {
                if let Ok(comp_mdl) = parse_mdl(&comp_bytes) {
                    model.textures = comp_mdl.textures;
                }
            }
        }
    }
    model.textures
}

/// Loads optional `sh2e/` override textures for `mdl_rel` (or its companion `.mdl`), returning
/// `None` when `quality == WorldTextureQuality::OriginalPs2` or no `sh2e/` override exists.
fn load_sh2e_model_textures(base_root: &Path, mdl_rel: &str) -> Option<HashMap<u32, ModelTexture>> {
    let sh2e_cand = if let Some(stripped) = mdl_rel.strip_prefix("data/") {
        Some(base_root.join("sh2e").join(stripped))
    } else if mdl_rel.starts_with("sh2e/") {
        Some(base_root.join(mdl_rel))
    } else {
        None
    };

    if let Some(p) = sh2e_cand {
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(mdl) = parse_mdl(&bytes) {
                if !mdl.textures.is_empty() {
                    return Some(mdl.textures);
                }
            }
        }
    }

    if let Some(comp_rel) = companion_texture_mdl_path(mdl_rel) {
        if let Some(stripped) = comp_rel.strip_prefix("data/") {
            let sh2e_comp = base_root.join("sh2e").join(stripped);
            if let Ok(bytes) = std::fs::read(&sh2e_comp) {
                if let Ok(mdl) = parse_mdl(&bytes) {
                    if !mdl.textures.is_empty() {
                        return Some(mdl.textures);
                    }
                }
            }
        }
    }

    None
}

/// Builds ONLY the single active [`WorldTextureQuality`] tier of GPU diffuse texture bind groups
/// for a character/enemy model (`SH2-181`).
pub(super) fn build_character_texture_bind_groups_for_quality(
    ctx: &RenderContext,
    character_name: &'static str,
    base_root: &Path,
    mdl_rel: &str,
    unique_tex_ids: &[u16],
    van_textures: &HashMap<u32, ModelTexture>,
    quality: WorldTextureQuality,
) -> Vec<Arc<wgpu::BindGroup>> {
    let sh2e_textures = if quality != WorldTextureQuality::OriginalPs2 {
        load_sh2e_model_textures(base_root, mdl_rel)
    } else {
        None
    };
    let tex_loader = if quality == WorldTextureQuality::Ai4kMaster {
        Some(TextureLoader::new(
            base_root.join("data"),
            Some(base_root.join("sh2e")),
        ))
    } else {
        None
    };

    let mut bind_groups = Vec::with_capacity(unique_tex_ids.len());

    for &raw_tid in unique_tex_ids {
        let tidx = raw_tid as u32;
        if let Some(tex) = van_textures.get(&tidx) {
            if let Ok(mut rgba) = tex.to_rgba() {
                // Non-cutout character textures (such as Pyramid Head's helmet `1303`/`1307`, Great Knife `1305`,
                // Lying Figure `1282`, and Abstract Daddy `1293`) store PS2 specular/gloss intensity in the DXT5
                // alpha channel with `van_min_alpha > 0` across 100% of texels. Preserve that non-zero alpha floor
                // on any HD / 4K override so bicubic ringing or compression artifacts never punch cutout holes.
                let van_min_alpha = rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|px| px[3])
                    .min()
                    .unwrap_or(255);

                match quality {
                    WorldTextureQuality::OriginalPs2 => {
                        sh2_renderer::dilate_transparent_border_rgb(
                            tex.width as u32,
                            tex.height as u32,
                            &mut rgba,
                            2,
                        );
                        bind_groups.push(ctx.create_texture_bind_group(
                            tex.width as u32,
                            tex.height as u32,
                            &rgba,
                            Some(&format!("{character_name} Vanilla Texture {tidx}")),
                        ));
                    }
                    WorldTextureQuality::Sh2eeHd => {
                        let bg = if let Some(hd_tex) =
                            sh2e_textures.as_ref().and_then(|m| m.get(&tidx))
                        {
                            if let Ok(mut hd_rgba) = hd_tex.to_rgba() {
                                if van_min_alpha > 0 {
                                    for px in hd_rgba.as_chunks_mut::<4>().0 {
                                        px[3] = px[3].max(van_min_alpha);
                                    }
                                }
                                let sh2e_passes = sh2_renderer::scaled_border_dilation_passes(
                                    tex.width as u32,
                                    tex.height as u32,
                                    hd_tex.width as u32,
                                    hd_tex.height as u32,
                                    2,
                                );
                                sh2_renderer::dilate_transparent_border_rgb(
                                    hd_tex.width as u32,
                                    hd_tex.height as u32,
                                    &mut hd_rgba,
                                    sh2e_passes,
                                );
                                ctx.create_texture_bind_group(
                                    hd_tex.width as u32,
                                    hd_tex.height as u32,
                                    &hd_rgba,
                                    Some(&format!("{character_name} SH2:EE HD Texture {tidx}")),
                                )
                            } else {
                                sh2_renderer::dilate_transparent_border_rgb(
                                    tex.width as u32,
                                    tex.height as u32,
                                    &mut rgba,
                                    2,
                                );
                                ctx.create_texture_bind_group(
                                    tex.width as u32,
                                    tex.height as u32,
                                    &rgba,
                                    Some(&format!("{character_name} Vanilla Texture {tidx}")),
                                )
                            }
                        } else {
                            sh2_renderer::dilate_transparent_border_rgb(
                                tex.width as u32,
                                tex.height as u32,
                                &mut rgba,
                                2,
                            );
                            let (hw, hh, mut hd_rgba) =
                                upscale_hd_preview_rgba(&rgba, tex.width, tex.height);
                            sh2_renderer::dilate_transparent_border_rgb(hw, hh, &mut hd_rgba, 2);
                            ctx.create_texture_bind_group(
                                hw,
                                hh,
                                &hd_rgba,
                                Some(&format!("{character_name} Enhanced Texture {tidx}")),
                            )
                        };
                        bind_groups.push(bg);
                    }
                    WorldTextureQuality::Ai4kMaster => {
                        let sh2e_raw: Option<(u32, u32, Vec<u8>)> = sh2e_textures
                            .as_ref()
                            .and_then(|m| m.get(&tidx))
                            .and_then(|hd_tex| {
                                hd_tex.to_rgba().ok().map(|mut hd_rgba| {
                                    if van_min_alpha > 0 {
                                        for px in hd_rgba.as_chunks_mut::<4>().0 {
                                            px[3] = px[3].max(van_min_alpha);
                                        }
                                    }
                                    (hd_tex.width as u32, hd_tex.height as u32, hd_rgba)
                                })
                            });

                        if let Some(mut dec) = tex_loader
                            .as_ref()
                            .and_then(|tl| tl.load_4k_chr_texture_override(tidx))
                        {
                            if van_min_alpha > 0 {
                                for px in dec.rgba.as_chunks_mut::<4>().0 {
                                    px[3] = px[3].max(van_min_alpha);
                                }
                            }
                            if van_min_alpha == 0 || sh2e_raw.is_some() {
                                sh2_renderer::refine_4k_chr_texture_from_sources(
                                    tex.width as u32,
                                    tex.height as u32,
                                    &rgba,
                                    sh2e_raw.as_ref().map(|(w, h, b)| (*w, *h, b.as_slice())),
                                    dec.width as u32,
                                    dec.height as u32,
                                    &mut dec.rgba,
                                );
                            }
                            let hd_passes = sh2_renderer::scaled_border_dilation_passes(
                                tex.width as u32,
                                tex.height as u32,
                                dec.width as u32,
                                dec.height as u32,
                                2,
                            );
                            sh2_renderer::dilate_transparent_border_rgb(
                                dec.width as u32,
                                dec.height as u32,
                                &mut dec.rgba,
                                hd_passes,
                            );
                            tracing::info!(
                                "Loaded 4K AI Master character texture override for '{}' tex_id={} ({}x{})",
                                character_name,
                                tidx,
                                dec.width,
                                dec.height
                            );
                            bind_groups.push(ctx.create_texture_bind_group(
                                dec.width as u32,
                                dec.height as u32,
                                &dec.rgba,
                                Some(&format!("{character_name} 4K AI Texture {tidx}")),
                            ));
                        } else if let Some((sw, sh, mut hd_rgba)) = sh2e_raw {
                            let sh2e_passes = sh2_renderer::scaled_border_dilation_passes(
                                tex.width as u32,
                                tex.height as u32,
                                sw,
                                sh,
                                2,
                            );
                            sh2_renderer::dilate_transparent_border_rgb(
                                sw,
                                sh,
                                &mut hd_rgba,
                                sh2e_passes,
                            );
                            bind_groups.push(ctx.create_texture_bind_group(
                                sw,
                                sh,
                                &hd_rgba,
                                Some(&format!("{character_name} SH2:EE HD Texture {tidx}")),
                            ));
                        } else {
                            sh2_renderer::dilate_transparent_border_rgb(
                                tex.width as u32,
                                tex.height as u32,
                                &mut rgba,
                                2,
                            );
                            let (hw, hh, mut hd_rgba) =
                                upscale_hd_preview_rgba(&rgba, tex.width, tex.height);
                            sh2_renderer::dilate_transparent_border_rgb(hw, hh, &mut hd_rgba, 2);
                            bind_groups.push(ctx.create_texture_bind_group(
                                hw,
                                hh,
                                &hd_rgba,
                                Some(&format!("{character_name} Enhanced Texture {tidx}")),
                            ));
                        }
                    }
                }
                continue;
            }
        }
        bind_groups.push(Arc::clone(&ctx.default_texture_bind_group));
    }

    bind_groups
}
