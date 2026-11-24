//! Retail-asset & headless GPU regression tests for `SH2-121`:
//! Verify `ob01.map` material/geometry parity between `Original PS2 / PC` (`data/bg/ob/ob01.map`)
//! and `SH2:EE` (`sh2e/bg/ob/ob01.map`), plus untextured (`texture_id == 0`) `MapMaterial::color`
//! modulation (`sh2pc.exe` `FUN_005b2190` `0x005b2190`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
use sh2_assets::formats::TextureLoader;
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::{RenderContext, RendererConfig};

fn game_root() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    candidates.into_iter().find(|p| {
        p.join("data/bg/ob/ob01.map").is_file() && p.join("sh2e/bg/ob/ob01.map").is_file()
    })
}

fn build_test_map_bind_groups(ctx: &RenderContext, map: &MapFile) -> Vec<Arc<wgpu::BindGroup>> {
    let mut cache: std::collections::HashMap<u32, Arc<wgpu::BindGroup>> =
        std::collections::HashMap::new();
    for mat in &map.materials {
        let tid = mat.texture_id as u32;
        if tid != 0 && !cache.contains_key(&tid) {
            if let Some(tex) = map.textures.get(&tid) {
                if let Ok(rgba) = tex.to_rgba() {
                    let bg = ctx.create_texture_bind_group(
                        tex.width as u32,
                        tex.height as u32,
                        &rgba,
                        None,
                    );
                    cache.insert(tid, bg);
                }
            }
        }
    }

    map.materials
        .iter()
        .map(|mat| {
            cache
                .get(&(mat.texture_id as u32))
                .cloned()
                .unwrap_or_else(|| Arc::clone(&ctx.default_texture_bind_group))
        })
        .collect()
}

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let unpadded_bpr = w * 4;
    let padded_bpr = unpadded_bpr.div_ceil(256) * 256;
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-121 Readback Staging"),
        size: u64::from(padded_bpr) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-121 Readback Encoder"),
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

fn write_ppm(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.reserve((w * h * 3) as usize);
    for px in rgba.as_chunks::<4>().0 {
        out.push(px[0]);
        out.push(px[1]);
        out.push(px[2]);
    }
    std::fs::write(path, out).expect("write ppm");
}

/// Deterministic headless GPU test verifying that an untextured (`texture_id == 0`) `.map` batch
/// renders with its `MapMaterial::color` tint (`sh2pc.exe` `FUN_005b2190` `0x005b2190`) rather than
/// pure white.
#[test]
fn sh2_121_untextured_material_color_modulates_rendered_pixels() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-121 headless color test: no wgpu adapter available");
        return;
    };

    let make_quad_map = |color: u32| {
        let mut map = MapFile {
            name: "untextured_quad".to_string(),
            ..Default::default()
        };
        map.materials.push(MapMaterial {
            mode: 1,
            texture_id: 0,
            color,
            overlay: 0xff00_0000,
            specularity: 0.0,
        });
        map.batches.push(MapBatch {
            group: BatchGroup::Opaque,
            material_index: 0,
            geo_id: 0,
            positions: vec![
                [-2.0, -2.0, 0.0],
                [2.0, -2.0, 0.0],
                [2.0, 2.0, 0.0],
                [-2.0, 2.0, 0.0],
            ],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            colors: vec![[255, 255, 255, 255]; 4],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        });
        map
    };

    let eye = glam::Vec3::new(0.0, 0.0, 3.0);
    let view = glam::Mat4::look_at_rh(eye, glam::Vec3::ZERO, glam::Vec3::Y);
    let proj = glam::Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 0.1, 100.0);
    ctx.update_camera(proj * view, proj.inverse(), eye);
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, 0.0, 3.0, 1.0],
        direction_inner: [0.0, 0.0, -1.0, -1.0],
        color_outer: [1.0, 1.0, 1.0, -1.0],
        ambient_tint: [1.0, 1.0, 1.0, 1.0],
        ..Default::default()
    });

    let white_map = make_quad_map(0xffff_ffff);
    let grey_map = make_quad_map(0xff7f_7f7f); // retail `ob01.map` mat[22]
    let red_map = make_quad_map(0xffcc_2020);

    let white_gpu = GpuMap::from_map(&ctx.device, &white_map);
    ctx.render_gpu_map(&white_gpu);
    let white_px = readback_ldr_rgba(&ctx);

    let grey_gpu = GpuMap::from_map(&ctx.device, &grey_map);
    ctx.render_gpu_map(&grey_gpu);
    let grey_px = readback_ldr_rgba(&ctx);

    let red_gpu = GpuMap::from_map(&ctx.device, &red_map);
    ctx.render_gpu_map(&red_gpu);
    let red_px = readback_ldr_rgba(&ctx);

    let center_idx = (64 * 128 + 64) * 4;
    let w_r = white_px[center_idx];
    let g_r = grey_px[center_idx];
    let r_r = red_px[center_idx];
    let r_g = red_px[center_idx + 1];
    let r_b = red_px[center_idx + 2];

    assert!(
        g_r < w_r,
        "Untextured 0xff7f7f7f material must render darker than 0xffffffff (got grey={g_r}, white={w_r})"
    );
    assert!(
        r_r > r_g.saturating_add(20) && r_r > r_b.saturating_add(20),
        "Untextured 0xffcc2020 D3DCOLOR (0xAARRGGBB) must render red-dominant (got RGB=({r_r},{r_g},{r_b}))"
    );
}

/// Retail-asset integration test verifying `ob01.map` material bounds, texture resolution, and
/// headless rendering in both retail `Original PS2 / PC` (`data/`) and `SH2:EE` (`sh2e/`) tiers.
#[test]
fn sh2_121_retail_ob01_vanilla_vs_sh2e_material_bounds_and_render() {
    let Some(root) = game_root() else {
        eprintln!("Skipping SH2-121 retail ob01.map test: retail rip not present");
        return;
    };

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let vanilla_map = loader
        .load_vanilla_map("bg/ob/ob01.map", "ob01")
        .expect("parse retail data/bg/ob/ob01.map");
    let sh2e_map = loader
        .load_sh2e_map("bg/ob/ob01.map", "ob01")
        .expect("parse sh2e/bg/ob/ob01.map");

    // 1. Verify exact material table divergence between retail `data/` (27) and `sh2e/` (31).
    assert_eq!(vanilla_map.materials.len(), 27);
    assert_eq!(sh2e_map.materials.len(), 31);

    // Retail `data/bg/ob/ob01.map` untextured materials (`tid == 0`) with authentic tint colors.
    assert!(vanilla_map.materials[2].is_untextured());
    assert_eq!(vanilla_map.materials[2].color, 0xffb2_b2b2);
    assert!(vanilla_map.materials[11].is_untextured());
    assert_eq!(vanilla_map.materials[11].color, 0xffb2_b2b2);
    assert!(vanilla_map.materials[22].is_untextured());
    assert_eq!(vanilla_map.materials[22].color, 0xff7f_7f7f);

    // SH2:EE extra car-interior materials `27..=30` (`tid = 47735..=47738`).
    for (idx, expected_tid) in [(27, 47735u16), (28, 47736), (29, 47737), (30, 47738)] {
        assert_eq!(sh2e_map.materials[idx].texture_id, expected_tid);
        assert!(
            sh2e_map.textures.contains_key(&(expected_tid as u32)),
            "sh2e ob01.map must embed texture {expected_tid} for mat[{idx}]"
        );
    }

    // 2. Verify every batch in each `.map` stays strictly within its OWN material table, while
    //    cross-indexing `sh2e` batches against `vanilla`'s 27-entry material table fails (the bug!).
    assert!(
        vanilla_map
            .batches
            .iter()
            .all(|b| b.material_index < vanilla_map.materials.len()),
        "Every retail data/bg/ob/ob01.map batch must index within 0..27"
    );
    assert!(
        sh2e_map
            .batches
            .iter()
            .all(|b| b.material_index < sh2e_map.materials.len()),
        "Every sh2e/bg/ob/ob01.map batch must index within 0..31"
    );
    assert!(
        sh2e_map
            .batches
            .iter()
            .any(|b| b.material_index >= vanilla_map.materials.len()),
        "sh2e/bg/ob/ob01.map must contain batches with material_index >= 27 (car interior)"
    );

    // 3. Verify all textured materials in both maps resolve to an embedded texture (no missing IDs).
    for (tier, map) in [("vanilla", &vanilla_map), ("sh2e", &sh2e_map)] {
        for (i, mat) in map.materials.iter().enumerate() {
            if !mat.is_untextured() {
                assert!(
                    map.textures.contains_key(&(mat.texture_id as u32)),
                    "{tier} ob01.map mat[{i}] references missing texture_id {}",
                    mat.texture_id
                );
            }
        }
    }

    // 4. Headless GPU verification + optional visual snapshot of James's car in `ob01.map`.
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 480,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-121 headless GPU render: no wgpu adapter available");
        return;
    };

    let vanilla_gpu = GpuMap::from_map(&ctx.device, &vanilla_map);
    let sh2e_gpu = GpuMap::from_map(&ctx.device, &sh2e_map);

    let vanilla_bgs = build_test_map_bind_groups(&ctx, &vanilla_map);
    let sh2e_bgs = build_test_map_bind_groups(&ctx, &sh2e_map);

    assert!(vanilla_gpu.all_batches_within_material_table(vanilla_bgs.len()));
    assert!(sh2e_gpu.all_batches_within_material_table(sh2e_bgs.len()));
    assert!(
        !sh2e_gpu.all_batches_within_material_table(vanilla_bgs.len()),
        "Mismatched sh2e GpuMap + vanilla bind groups must be detected as out-of-bounds"
    );

    // Compute bounding box of SH2:EE car-interior batch (`material_index == 28`, James's car cabin)
    // so the camera directly frames James's car interior in both Vanilla PS2 and SH2:EE renders.
    let mut min_p = glam::Vec3::splat(f32::INFINITY);
    let mut max_p = glam::Vec3::splat(f32::NEG_INFINITY);
    for b in sh2e_map.batches.iter().filter(|b| b.material_index == 28) {
        for &p in &b.positions {
            let v = glam::Vec3::from_array(p);
            min_p = min_p.min(v);
            max_p = max_p.max(v);
        }
    }
    let car_center = (min_p + max_p) * 0.5;
    let cam_eye = car_center + glam::Vec3::new(-1600.0, -700.0, 1800.0);
    let view = glam::Mat4::look_at_rh(cam_eye, car_center, glam::Vec3::NEG_Y);
    let proj = glam::Mat4::perspective_rh(50.0f32.to_radians(), 640.0 / 480.0, 50.0, 200_000.0);
    ctx.update_camera(proj * view, proj.inverse(), cam_eye);
    ctx.update_spotlight(&SpotlightUniforms {
        position: [cam_eye.x, cam_eye.y, cam_eye.z, 1.0],
        direction_inner: [0.3, 0.85, 0.4, -1.0],
        color_outer: [1.1, 1.1, 1.05, -1.0],
        ambient_tint: [0.95, 0.96, 1.0, 1.6],
        ..Default::default()
    });

    // Render Vanilla PS2 paired GpuMap + bind groups
    ctx.render_scene(Some(&vanilla_gpu), Some(&vanilla_bgs), &[]);
    let van_img = readback_ldr_rgba(&ctx);

    // Render SH2:EE paired GpuMap + bind groups
    ctx.render_scene(Some(&sh2e_gpu), Some(&sh2e_bgs), &[]);
    let sh2e_img = readback_ldr_rgba(&ctx);

    let non_black_van = van_img
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 5 || px[1] > 5 || px[2] > 5)
        .count();
    let non_black_sh2e = sh2e_img
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 5 || px[1] > 5 || px[2] > 5)
        .count();
    assert!(
        non_black_van > 1000,
        "Vanilla ob01.map render produced too few lit pixels ({non_black_van})"
    );
    assert!(
        non_black_sh2e > 1000,
        "SH2:EE ob01.map render produced too few lit pixels ({non_black_sh2e})"
    );

    if let Ok(dump_dir) = std::env::var("SH2_121_DUMP_DIR") {
        let dir = PathBuf::from(dump_dir);
        std::fs::create_dir_all(&dir).ok();
        write_ppm(&dir.join("ob01_car_vanilla_ps2.ppm"), 640, 480, &van_img);
        write_ppm(&dir.join("ob01_car_sh2e.ppm"), 640, 480, &sh2e_img);
    }
}
