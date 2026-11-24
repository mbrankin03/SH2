//! Regression & visual verification tests for SH2-120:
//! Maria's hair cutout & color artifacts when 4K AI Master textures are enabled.

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::anm::{compute_bind_locals, compute_skin_matrices, evaluate_pose};
use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::formats::texture::TextureLoader;
use sh2_renderer::{
    dilate_transparent_border_rgb, light::SpotlightUniforms, refine_4k_chr_texture_from_sources,
    scaled_border_dilation_passes, RenderContext, RendererConfig,
};

fn game_root() -> Option<PathBuf> {
    let p = PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    p.exists().then_some(p)
}

#[test]
fn sh2_120_maria_4k_hair_textures_have_clean_binary_alpha_pink_tips_and_no_dark_gutter() {
    let Some(root) = game_root() else {
        return;
    };
    let tl = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));

    for (van_rel, sh2e_rel, hair_tid, expect_pink_transfer) in [
        (
            "data/chr/mar/lll_mar.mdl",
            Some("sh2e/chr/mar/lll_mar.mdl"),
            1246u32,
            true,
        ),
        (
            "data/chr2/mar/lxx_mar.mdl",
            Some("sh2e/chr2/mar/lxx_mar.mdl"),
            1246u32,
            true,
        ),
        ("data/chr/mar/hhh_mar.mdl", None, 1246u32, false),
        ("data/chr2/mar/hhh_mar.mdl", None, 53843u32, false),
    ] {
        let van_mdl =
            parse_mdl(&std::fs::read(root.join(van_rel)).expect("read mdl")).expect("parse mdl");
        let sh2e_mdl = sh2e_rel.and_then(|r| {
            std::fs::read(root.join(r))
                .ok()
                .and_then(|b| parse_mdl(&b).ok())
        });

        let van_tex = &van_mdl.textures[&hair_tid];
        let van_rgba = van_tex.to_rgba().expect("decode van hair tex");

        let sh2e_raw = sh2e_mdl.as_ref().and_then(|sm| {
            sm.textures.get(&hair_tid).and_then(|st| {
                st.to_rgba()
                    .ok()
                    .map(|buf| (st.width as u32, st.height as u32, buf))
            })
        });

        let Some(mut dec) = tl.load_4k_chr_texture_override(hair_tid) else {
            continue;
        };

        refine_4k_chr_texture_from_sources(
            van_tex.width as u32,
            van_tex.height as u32,
            &van_rgba,
            sh2e_raw.as_ref().map(|(w, h, b)| (*w, *h, b.as_slice())),
            dec.width as u32,
            dec.height as u32,
            &mut dec.rgba,
        );
        let hd_passes = scaled_border_dilation_passes(
            van_tex.width as u32,
            van_tex.height as u32,
            dec.width as u32,
            dec.height as u32,
            2,
        );
        assert_eq!(hd_passes, 8, "4K 4x texture must use 8 dilation passes");
        dilate_transparent_border_rgb(
            dec.width as u32,
            dec.height as u32,
            &mut dec.rgba,
            hd_passes,
        );

        // 1. Binary cutout source (`1246` / `53843`) must have ZERO noisy mid-alpha (`1..=254`) pixels
        //    after refinement, eliminating DXT5 / upscaler stippled hair-tip shredding.
        let mid_alpha_count = dec
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] > 0 && px[3] < 255)
            .count();
        assert_eq!(
            mid_alpha_count, 0,
            "{van_rel} tid={hair_tid}: 4K binary cutout hair must have 0 noisy mid-alpha texels"
        );

        // 2. SH2:EE pink hair tips must transfer onto 4K `lll_mar` / `lxx_mar` `tid=1246`.
        if expect_pink_transfer {
            // Hair UV island occupies the left half (`x < width / 2`, `y < height * 5 / 8`).
            let dw = dec.width;
            let dh = dec.height;
            let mut pink_hair_tips = 0usize;
            for y in 0..(dh * 5 / 8) {
                for x in 0..(dw / 2) {
                    let i = (y * dw + x) * 4;
                    let [r, g, _b, a] = [
                        dec.rgba[i],
                        dec.rgba[i + 1],
                        dec.rgba[i + 2],
                        dec.rgba[i + 3],
                    ];
                    if a == 255 && r > g.saturating_add(25) && r > 100 {
                        pink_hair_tips += 1;
                    }
                }
            }
            assert!(
                pink_hair_tips > 50_000,
                "{van_rel} tid={hair_tid}: expected SH2:EE pink hair tips transferred onto 4K (>50k px), got {pink_hair_tips}"
            );
        }

        // Optional visual dump of the refined 4K `1246` hair-tip crop over bright fog
        if van_rel == "data/chr/mar/lll_mar.mdl" {
            if let Ok(dump_dir) = std::env::var("SH2_120_DUMP_DIR") {
                let out_dir = PathBuf::from(dump_dir);
                std::fs::create_dir_all(&out_dir).ok();
                let dw = dec.width;
                let crop_w = 1024usize;
                let crop_h = 550usize;
                let mut over_fog = Vec::with_capacity(crop_w * crop_h * 4);
                for y in 750..(750 + crop_h) {
                    for x in 0..crop_w {
                        let i = (y * dw + x) * 4;
                        let a = dec.rgba[i + 3] as f32 / 255.0;
                        let r = (dec.rgba[i] as f32 * a + 200.0 * (1.0 - a)).round() as u8;
                        let g = (dec.rgba[i + 1] as f32 * a + 205.0 * (1.0 - a)).round() as u8;
                        let b = (dec.rgba[i + 2] as f32 * a + 210.0 * (1.0 - a)).round() as u8;
                        over_fog.extend_from_slice(&[r, g, b, 255]);
                    }
                }
                write_ppm(
                    &out_dir.join("crop_1246_4k_after_over_fog.ppm"),
                    crop_w as u32,
                    crop_h as u32,
                    &over_fog,
                );
            }
        }
    }

    // 3. Smooth-alpha hair sheet (`hhh_mar` `tid=1247`) must keep smooth alpha gradients while
    //    clamping stray upscaler specks outside the vanilla silhouette.
    let hhh = parse_mdl(&std::fs::read(root.join("data/chr/mar/hhh_mar.mdl")).expect("read hhh"))
        .expect("parse hhh");
    if let (Some(van_1247), Some(mut dec_1247)) = (
        hhh.textures.get(&1247),
        tl.load_4k_chr_texture_override(1247),
    ) {
        let van_rgba = van_1247.to_rgba().expect("decode 1247");
        let before_zero = dec_1247
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] == 0)
            .count();
        refine_4k_chr_texture_from_sources(
            van_1247.width as u32,
            van_1247.height as u32,
            &van_rgba,
            None,
            dec_1247.width as u32,
            dec_1247.height as u32,
            &mut dec_1247.rgba,
        );
        let after_zero = dec_1247
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] == 0)
            .count();
        let after_mid = dec_1247
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] > 0 && px[3] < 255)
            .count();
        assert!(
            after_zero >= before_zero,
            "Smooth-alpha cleanup must clamp stray background specks to 0"
        );
        assert!(
            after_mid > 100_000,
            "Smooth-alpha hair sheet 1247 must preserve its smooth alpha gradient, got {after_mid}"
        );
    }
}

#[test]
fn sh2_120_render_maria_hair_vanilla_vs_4k_headless() {
    let Some(root) = game_root() else {
        return;
    };
    let dump_dir = std::env::var("SH2_120_DUMP_DIR").ok().map(PathBuf::from);
    if let Some(ref d) = dump_dir {
        std::fs::create_dir_all(d).ok();
    }

    let tl = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 512,
        height: 512,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        return;
    };

    ctx.update_fog(&sh2_renderer::fog::FogUniforms {
        fog_color: [0.78, 0.80, 0.82, 1.0],
        ..Default::default()
    });

    for (tag, van_rel, sh2e_rel) in [
        (
            "lll_mar",
            "data/chr/mar/lll_mar.mdl",
            Some("sh2e/chr/mar/lll_mar.mdl"),
        ),
        ("hhh_mar", "data/chr/mar/hhh_mar.mdl", None),
    ] {
        let model =
            parse_mdl(&std::fs::read(root.join(van_rel)).expect("read mdl")).expect("parse mdl");
        let sh2e_model = sh2e_rel.and_then(|r| {
            std::fs::read(root.join(r))
                .ok()
                .and_then(|b| parse_mdl(&b).ok())
        });

        let mut gpu_mesh = ctx.compile_skeletal_mesh(&model);
        let mut unique_tex_ids: Vec<u16> =
            model.primitives.iter().map(|pr| pr.texture_index).collect();
        unique_tex_ids.sort_unstable();
        unique_tex_ids.dedup();

        let mut van_bgs = Vec::new();
        let mut sh2e_bgs = Vec::new();
        let mut hd_bgs = Vec::new();

        for &raw_tid in &unique_tex_ids {
            let tidx = raw_tid as u32;
            let tex = &model.textures[&tidx];
            let mut rgba = tex.to_rgba().expect("decode tex");
            let van_min_alpha = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .min()
                .unwrap_or(255);
            let van_raw = rgba.clone();

            // Strict 1:1 Vanilla PS2 path (untouched)
            dilate_transparent_border_rgb(tex.width as u32, tex.height as u32, &mut rgba, 2);
            let van_bg =
                ctx.create_texture_bind_group(tex.width as u32, tex.height as u32, &rgba, None);

            let mut sh2e_raw = None;
            let sh2e_bg = if let Some(ref sm) = sh2e_model {
                if let Some(st) = sm.textures.get(&tidx) {
                    let mut s_rgba = st.to_rgba().expect("decode sh2e");
                    if van_min_alpha > 0 {
                        for px in s_rgba.as_chunks_mut::<4>().0 {
                            px[3] = px[3].max(van_min_alpha);
                        }
                    }
                    sh2e_raw = Some((st.width as u32, st.height as u32, s_rgba.clone()));
                    let sh2e_passes = scaled_border_dilation_passes(
                        tex.width as u32,
                        tex.height as u32,
                        st.width as u32,
                        st.height as u32,
                        2,
                    );
                    dilate_transparent_border_rgb(
                        st.width as u32,
                        st.height as u32,
                        &mut s_rgba,
                        sh2e_passes,
                    );
                    ctx.create_texture_bind_group(st.width as u32, st.height as u32, &s_rgba, None)
                } else {
                    Arc::clone(&van_bg)
                }
            } else {
                Arc::clone(&van_bg)
            };

            let hd_bg = tl
                .load_4k_chr_texture_override(tidx)
                .map(|mut dec| {
                    if van_min_alpha > 0 {
                        for px in dec.rgba.as_chunks_mut::<4>().0 {
                            px[3] = px[3].max(van_min_alpha);
                        }
                    }
                    refine_4k_chr_texture_from_sources(
                        tex.width as u32,
                        tex.height as u32,
                        &van_raw,
                        sh2e_raw.as_ref().map(|(w, h, b)| (*w, *h, b.as_slice())),
                        dec.width as u32,
                        dec.height as u32,
                        &mut dec.rgba,
                    );
                    let hd_passes = scaled_border_dilation_passes(
                        tex.width as u32,
                        tex.height as u32,
                        dec.width as u32,
                        dec.height as u32,
                        2,
                    );
                    dilate_transparent_border_rgb(
                        dec.width as u32,
                        dec.height as u32,
                        &mut dec.rgba,
                        hd_passes,
                    );
                    ctx.create_texture_bind_group(
                        dec.width as u32,
                        dec.height as u32,
                        &dec.rgba,
                        None,
                    )
                })
                .unwrap_or_else(|| Arc::clone(&sh2e_bg));

            van_bgs.push(van_bg);
            sh2e_bgs.push(sh2e_bg);
            hd_bgs.push(hd_bg);
        }

        for prim in &mut gpu_mesh.primitives {
            if let Ok(slot) = unique_tex_ids.binary_search(&prim.texture_index) {
                prim.texture_index = slot as u16;
            }
        }

        let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
        let empty_frame = sh2_assets::formats::anm::AnimationFrame {
            bones: vec![None; model.bone_count],
        };
        let pose = evaluate_pose(&model.parents, &bind_locals, &empty_frame);
        let skin = compute_skin_matrices(&pose, &model.initial_matrices);

        let min_y = model
            .vertices
            .positions
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        let head_y = min_y + 130.0;
        let eye = glam::Vec3::new(110.0, head_y, 380.0);
        let target = glam::Vec3::new(0.0, head_y, 0.0);
        let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::new(0.0, -1.0, 0.0));
        let proj = glam::Mat4::perspective_rh(38.0_f32.to_radians(), 1.0, 10.0, 50_000.0);
        ctx.update_camera_with_mode(proj * view, proj.inverse(), eye, 3.0);
        ctx.update_spotlight(&SpotlightUniforms {
            position: [eye.x, eye.y, eye.z, 1.0],
            direction_inner: [0.0, 0.0, -1.0, 0.90],
            color_outer: [1.5, 1.45, 1.4, 0.70],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            ..Default::default()
        });

        for (mode_name, bgs) in [
            ("vanilla_after", &van_bgs),
            ("sh2e_after", &sh2e_bgs),
            ("4k_after", &hd_bgs),
        ] {
            let draws = [(&gpu_mesh, skin.as_slice(), Some(bgs.as_slice()))];
            ctx.render_scene(None, None, &draws);
            let rgba = readback_ldr_rgba(&ctx);
            assert_eq!(rgba.len(), 512 * 512 * 4);
            if let Some(ref d) = dump_dir {
                write_ppm(&d.join(format!("{tag}_{mode_name}.ppm")), 512, 512, &rgba);
            }
        }
    }
}

fn write_ppm(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.reserve((w * h * 3) as usize);
    for px in rgba.as_chunks::<4>().0 {
        out.push(px[0]);
        out.push(px[1]);
        out.push(px[2]);
    }
    std::fs::write(path, out).expect("write ppm");
}

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let unpadded_bpr = w * 4;
    let padded_bpr = unpadded_bpr.div_ceil(256) * 256;
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-120 Readback Staging"),
        size: u64::from(padded_bpr) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-120 Readback Encoder"),
        });
    enc.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &ctx.ldr_output_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    ctx.queue.submit(std::iter::once(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        let row = &data[y * padded_bpr as usize..y * padded_bpr as usize + (w * 4) as usize];
        out.extend_from_slice(row);
    }
    drop(data);
    staging.unmap();
    out
}
