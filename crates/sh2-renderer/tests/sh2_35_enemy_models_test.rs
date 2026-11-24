//! Headless GPU visual & weapon-socket verification for all 12 `Enemy_List` models
//! (`scu`, `mkn`, `tyu`, `nse`, `xoo`, `arm`, `red`, `oni`, `ike`, `pap`, `bos`, `edi`)
//! under strict Vanilla PS2 rendering mode (`SH2-35`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv, evaluate_pose, parse_anm,
};
use sh2_assets::formats::mdls::parse_mdl;
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::skeletal::{
    dilate_transparent_border_rgb, set_enemy_weapon_visible, EnemyModelKey, ENEMY_MODEL_SPECS,
};
use sh2_renderer::{RenderContext, RendererConfig};

fn game_root() -> Option<PathBuf> {
    let candidates = [
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
        "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.join("data/chr/scu/scu.mdl").exists())
}

fn write_ppm(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
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
        label: Some("SH2-35 Readback Staging"),
        size: (padded_bpr * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-35 Readback Encoder"),
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
    ctx.queue.submit(Some(enc.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();

    let mapped = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let start = row * padded_bpr as usize;
        out.extend_from_slice(&mapped[start..start + unpadded_bpr as usize]);
    }
    drop(mapped);
    staging.unmap();
    out
}

#[test]
fn sh2_35_headless_render_all_12_enemy_list_models_vanilla_ps2() {
    let Some(root) = game_root() else {
        println!("Skipping SH2-35 headless render test: retail assets not present");
        return;
    };
    let dump_dir = std::env::var("SH2_35_DUMP_DIR").ok().map(PathBuf::from);

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 320,
        height: 240,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        println!("Skipping SH2-35 headless render test: no wgpu adapter available");
        return;
    };

    ctx.update_fog(&FogUniforms {
        fog_color: [0.12, 0.13, 0.15, 1.0],
        ..Default::default()
    });

    for spec in &ENEMY_MODEL_SPECS {
        let model = parse_mdl(&std::fs::read(root.join(spec.mdl_rel_path)).expect("read mdl"))
            .expect("parse mdl");
        let anm = parse_anm(
            &std::fs::read(root.join(spec.primary_anm_rel_path)).expect("read anm"),
            model.bone_count,
            &model.parents,
        )
        .expect("parse anm");

        let mut gpu_mesh = ctx.compile_skeletal_mesh(&model);
        let mut unique_tex_ids: Vec<u16> =
            model.primitives.iter().map(|pr| pr.texture_index).collect();
        unique_tex_ids.sort_unstable();
        unique_tex_ids.dedup();

        let mut van_bgs: Vec<Arc<wgpu::BindGroup>> = Vec::with_capacity(unique_tex_ids.len());
        for &raw_tid in &unique_tex_ids {
            let tidx = raw_tid as u32;
            if let Some(tex) = model.textures.get(&tidx) {
                let mut rgba = tex.to_rgba().expect("decode tex");
                dilate_transparent_border_rgb(tex.width as u32, tex.height as u32, &mut rgba, 2);
                van_bgs.push(ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    None,
                ));
            } else {
                van_bgs.push(ctx.create_texture_bind_group(1, 1, &[255, 255, 255, 255], None));
            }
        }
        for prim in &mut gpu_mesh.primitives {
            if let Ok(slot) = unique_tex_ids.binary_search(&prim.texture_index) {
                prim.texture_index = slot as u16;
            }
        }

        // Verify weapon pose toggle on `red.mdl` (`shCharacterSetWeaponRED` `0x0011caf0`)
        if spec.key == EnemyModelKey::Red {
            assert!(gpu_mesh
                .primitives
                .iter()
                .any(|p| p.pose_index == 3 && p.is_active));
            set_enemy_weapon_visible(&mut gpu_mesh, EnemyModelKey::Red, false);
            assert!(gpu_mesh
                .primitives
                .iter()
                .filter(|p| p.pose_index == 3)
                .all(|p| !p.is_active));
            set_enemy_weapon_visible(&mut gpu_mesh, EnemyModelKey::Red, true);
            assert!(gpu_mesh
                .primitives
                .iter()
                .any(|p| p.pose_index == 3 && p.is_active));
        }

        let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
        let pose = evaluate_pose(&model.parents, &bind_locals, &anm.frames[0]);
        let skin = compute_skin_matrices_with_inv(&pose, &model.inv_initial_matrices);

        let (min_y, max_y) = model
            .vertices
            .positions
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                (lo.min(p[1]), hi.max(p[1]))
            });
        let center_y = 0.5 * (min_y + max_y);
        let span_y = (max_y - min_y).max(400.0);
        let dist = span_y * 1.45;
        let eye = glam::Vec3::new(dist * 0.35, center_y - span_y * 0.15, dist);
        let target = glam::Vec3::new(0.0, center_y, 0.0);
        let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::new(0.0, -1.0, 0.0));
        let proj = glam::Mat4::perspective_rh(42.0_f32.to_radians(), 320.0 / 240.0, 10.0, 50_000.0);
        ctx.update_camera_with_mode(proj * view, proj.inverse(), eye, 3.0);
        ctx.update_spotlight(&SpotlightUniforms {
            position: [eye.x, eye.y, eye.z, 1.0],
            direction_inner: [
                (target.x - eye.x) / dist,
                (target.y - eye.y) / dist,
                (target.z - eye.z) / dist,
                0.85,
            ],
            color_outer: [1.6, 1.55, 1.5, 0.65],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            ..Default::default()
        });

        let draws = [(&gpu_mesh, skin.as_slice(), Some(van_bgs.as_slice()))];
        ctx.render_scene(None, None, &draws);
        let rgba = readback_ldr_rgba(&ctx);
        assert_eq!(rgba.len(), 320 * 240 * 4);

        // Verify non-uniform rendered pixels (creature silhouette is visible)
        let first_px = [rgba[0], rgba[1], rgba[2]];
        let lit_pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| {
                px[0].abs_diff(first_px[0]) > 8
                    || px[1].abs_diff(first_px[1]) > 8
                    || px[2].abs_diff(first_px[2]) > 8
            })
            .count();
        assert!(
            lit_pixels > 50,
            "{}: expected visible creature silhouette, got only {lit_pixels} non-bg pixels",
            spec.code
        );

        if let Some(ref dir) = dump_dir {
            write_ppm(
                &dir.join(format!("{}_vanilla.ppm", spec.code)),
                320,
                240,
                &rgba,
            );
        }
    }
}
