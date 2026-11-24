//! Regression & headless visual verification tests for SH2-122:
//! Solid `.map` level geometry culling & alpha-test parity (`ob01.map` sidewalk curb).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
use sh2_assets::formats::texture::TextureLoader;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::pipeline::{map_batch_render_state, GeometryPipeline, MAP_ALPHA_TEST_THRESHOLD};
use sh2_renderer::{GpuMap, RenderContext, RendererConfig};

fn game_root() -> Option<PathBuf> {
    let p = PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    p.exists().then_some(p)
}

#[test]
fn sh2_122_map_batch_render_state_matches_sh2pc_and_slus() {
    // 1. Verify exact `sh2pc.exe` `FUN_005b4cc0` (`0x005b4cc0`) / `0x005b1694` (`D3DRS_CULLMODE = 2`
    //    `D3DCULL_CW` -> `FrontFace::Ccw` + `Face::Back`) and `SLUS_202.28` `shGs_InitDefaultRegsEnv`
    //    (`0x001909e0`: `ATST=GREATER, AREF=0` -> `0.5 / 255.0`) across all three `BatchGroup`s.
    let opaque = map_batch_render_state(BatchGroup::Opaque);
    assert_eq!(opaque.front_face, wgpu::FrontFace::Ccw);
    assert_eq!(opaque.cull_mode, Some(wgpu::Face::Back));
    assert!(opaque.depth_write_enabled);
    assert!(!opaque.alpha_blending);
    assert!((opaque.alpha_test_threshold - MAP_ALPHA_TEST_THRESHOLD).abs() < 1e-7);

    let decal = map_batch_render_state(BatchGroup::Decal);
    assert_eq!(decal.front_face, wgpu::FrontFace::Ccw);
    assert_eq!(decal.cull_mode, Some(wgpu::Face::Back));
    assert!(!decal.depth_write_enabled);
    assert!(decal.alpha_blending);
    assert!((decal.alpha_test_threshold - MAP_ALPHA_TEST_THRESHOLD).abs() < 1e-7);

    let transparent = map_batch_render_state(BatchGroup::Transparent);
    assert_eq!(transparent.front_face, wgpu::FrontFace::Ccw);
    assert_eq!(transparent.cull_mode, Some(wgpu::Face::Back));
    assert!(!transparent.depth_write_enabled);
    assert!(transparent.alpha_blending);
    assert!((transparent.alpha_test_threshold - MAP_ALPHA_TEST_THRESHOLD).abs() < 1e-7);

    // 2. Guard against re-introducing the non-authentic interpolated vertex-normal backface discard
    //    (`dot(in.normal, ...) < -0.05`), the `0.05` alpha cutoff, or manual `!is_front` discard.
    let wgsl = GeometryPipeline::WGSL_SHADER;
    assert!(
        !wgsl.contains("-0.05"),
        "GeometryPipeline WGSL must not contain interpolated-normal -0.05 backface discard"
    );
    assert!(
        !wgsl.contains("< 0.05"),
        "GeometryPipeline WGSL must not contain non-authentic 0.05 alpha-test cutoff"
    );
    assert!(
        !wgsl.contains("is_front"),
        "GeometryPipeline WGSL must rely on hardware Face::Back culling, not fragment !is_front discard"
    );
    assert!(
        wgsl.contains("0.5 / 255.0"),
        "GeometryPipeline WGSL must enforce exact quantized 8-bit ATST=GREATER AREF=0 (0.5 / 255.0)"
    );
}

#[test]
fn sh2_122_grazing_normal_front_facing_quad_is_not_discarded_and_backface_is_culled() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        println!("Skipping headless GPU test: no suitable adapter");
        return;
    };

    // Distinct bright magenta clear color so any discarded pixel is unmistakable.
    ctx.set_room_clear_color(false, Some([255, 0, 255, 255]));

    let eye = glam::Vec3::new(0.0, 0.0, 500.0);
    let target = glam::Vec3::new(0.0, 0.0, 0.0);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::new(0.0, -1.0, 0.0));
    let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), 1.0, 10.0, 5000.0);
    ctx.update_camera_with_mode(proj * view, proj.inverse(), eye, 0.0);
    ctx.update_spotlight(&SpotlightUniforms {
        position: [eye.x, eye.y, eye.z, 1.0],
        direction_inner: [0.0, 0.0, -1.0, -1.0],
        color_outer: [1.0, 1.0, 1.0, -1.0],
        ambient_tint: [1.0, 1.0, 1.0, 1.0],
        ..Default::default()
    });

    // 1. Front-facing CCW quad (in Y-down camera basis) whose smoothed corner vertex normals
    //    intentionally tilt away from the camera (`dot(n, view_dir) = -0.35`, which the old
    //    `-0.05 * n_len` fragment hack discarded, punching holes in angled curb corners).
    let grazing_normal = glam::Vec3::new(0.0, -0.9367, -0.35).normalize().to_array();
    let ccw_map = MapFile {
        name: "synthetic_grazing_curb".to_string(),
        materials: vec![MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0xffb2b2b2,
            overlay: 0xff000000,
            specularity: 0.0,
        }],
        textures: Default::default(),
        batches: vec![MapBatch {
            group: BatchGroup::Opaque,
            material_index: 0,
            geo_id: 0,
            // In right-handed view with `eye = (0, 0, 500)`, `target = (0, 0, 0)`, `up = (0, -1, 0)`:
            // `right = fwd x up = (-1, 0, 0)`, so `+X` is screen-left and `-Y` is screen-up.
            // CCW front-facing in screen space:
            // (100, 100) [bottom-left] -> (-100, 100) [bottom-right]
            // -> (-100, -100) [top-right] -> (100, -100) [top-left].
            positions: vec![
                [100.0, 100.0, 0.0],
                [-100.0, 100.0, 0.0],
                [-100.0, -100.0, 0.0],
                [100.0, -100.0, 0.0],
            ],
            normals: vec![grazing_normal; 4],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            colors: vec![[200, 200, 200, 255]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    };
    let ccw_gpu = GpuMap::from_map(&ctx.device, &ccw_map);
    ctx.render_gpu_map(&ccw_gpu);
    let rgba_front = readback_ldr_rgba(&ctx);
    let corner_px = pixel_at(&rgba_front, 128, 2, 2);
    assert!(
        is_magenta(corner_px),
        "Sanity check: uncovered clear-color corner must be detected as magenta; corner={corner_px:?}"
    );
    let center_px = pixel_at(&rgba_front, 128, 64, 64);
    assert!(
        !is_magenta(center_px),
        "Front-facing CCW quad with grazing vertex normal must NOT be discarded; center={center_px:?}"
    );

    // 2. Identical quad with reversed (CW / back-facing) winding must be culled by hardware
    //    `cull_mode: Some(Face::Back)` (`sh2pc.exe` `D3DRS_CULLMODE = D3DCULL_CW`).
    let mut cw_map = ccw_map;
    cw_map.batches[0].indices = vec![0, 2, 1, 0, 3, 2];
    let cw_gpu = GpuMap::from_map(&ctx.device, &cw_map);
    ctx.render_gpu_map(&cw_gpu);
    let rgba_back = readback_ldr_rgba(&ctx);
    let back_center = pixel_at(&rgba_back, 128, 64, 64);
    assert!(
        is_magenta(back_center),
        "Back-facing CW quad must be culled by hardware Face::Back; center={back_center:?}"
    );
}

#[test]
fn sh2_122_ob01_sidewalk_curb_has_zero_see_through_holes() {
    let Some(root) = game_root() else {
        println!("Skipping retail ob01.map curb regression test: retail assets not found");
        return;
    };

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 512,
        height: 384,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        println!("Skipping retail ob01.map curb regression test: no suitable GPU adapter");
        return;
    };

    let data_dir = root.join("data");
    let sh2e_dir = root.join("sh2e");
    let vanilla_loader = TextureLoader::new(data_dir.clone(), None::<PathBuf>);
    let sh2e_loader = TextureLoader::new(data_dir, Some(sh2e_dir));

    // Check both Original PS2 (`data/bg/ob/ob01.map`) and SH2:EE (`sh2e/bg/ob/ob01.map`).
    let cases = [
        (
            "original_ps2",
            vanilla_loader
                .load_map_with_hd_overrides("bg/ob/ob01.map", "ob01")
                .expect("load vanilla ob01.map"),
        ),
        (
            "sh2ee",
            sh2e_loader
                .load_map_with_hd_overrides("bg/ob/ob01.map", "ob01")
                .expect("load sh2ee ob01.map"),
        ),
    ];

    // Camera looking down at the angled sidewalk curb corner around (7400, -50, -1880)
    // from near the ticket coordinates `X: 7898, Z: -1963` toward `X: 7017, Z: -1800`.
    let eye = glam::Vec3::new(7898.0, -520.0, -1963.0);
    let target = glam::Vec3::new(7017.0, -45.0, -1800.0);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::new(0.0, -1.0, 0.0));
    let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), 512.0 / 384.0, 10.0, 70_000.0);
    ctx.update_camera_with_mode(proj * view, proj.inverse(), eye, 0.0);
    ctx.update_spotlight(&SpotlightUniforms {
        position: [eye.x, eye.y, eye.z, 1.0],
        direction_inner: [0.0, 1.0, 0.0, -1.0],
        color_outer: [1.35, 1.35, 1.35, -1.0],
        ambient_tint: [1.0, 1.0, 1.0, 2.0],
        ..Default::default()
    });

    let snapshot_out = std::env::var_os("SH2_122_SNAPSHOT_OUT").map(PathBuf::from);

    for (label, map) in &cases {
        let gpu_map = GpuMap::from_map(&ctx.device, map);
        let bgs = build_map_texture_bind_groups(&ctx, map);

        // Pass 1: Bright magenta clear color to detect ANY see-through void pixel leaking
        // through the solid sidewalk curb & asphalt in the lower-center ROI.
        ctx.set_room_clear_color(false, Some([255, 0, 255, 255]));
        ctx.render_scene(Some(&gpu_map), Some(&bgs), &[]);
        let rgba_probe = readback_ldr_rgba(&ctx);

        // Lower-center ROI (`x in 128..384`, `y in 192..360`) is 100% covered by the solid
        // Observation Deck sidewalk curb and asphalt. Zero magenta void pixels may appear.
        let mut magenta_holes = 0usize;
        for y in 192..360 {
            for x in 128..384 {
                if is_magenta(pixel_at(&rgba_probe, 512, x, y)) {
                    magenta_holes += 1;
                }
            }
        }
        assert_eq!(
            magenta_holes, 0,
            "{label} ob01.map sidewalk curb ROI has {magenta_holes} see-through void pixels"
        );

        // Pass 2: Optional Vanilla PS2 FOGCOL (`0x46/255`) snapshot for visual verification.
        if *label == "original_ps2" {
            if let Some(ref out_path) = snapshot_out {
                ctx.set_room_clear_color(false, None);
                ctx.render_scene(Some(&gpu_map), Some(&bgs), &[]);
                let rgba_vis = readback_ldr_rgba(&ctx);
                save_png(out_path, 512, 384, &rgba_vis);
            }
        }
    }
}

fn build_map_texture_bind_groups(ctx: &RenderContext, map: &MapFile) -> Vec<Arc<wgpu::BindGroup>> {
    let mut bgs = Vec::with_capacity(map.materials.len());
    for mat in &map.materials {
        let tid = u32::from(mat.texture_id);
        let bg = if let Some(tex) = map.textures.get(&tid) {
            if let Ok(rgba) = tex.to_rgba() {
                ctx.create_texture_bind_group(tex.width as u32, tex.height as u32, &rgba, None)
            } else {
                Arc::clone(&ctx.default_texture_bind_group)
            }
        } else {
            Arc::clone(&ctx.default_texture_bind_group)
        };
        bgs.push(bg);
    }
    bgs
}

fn pixel_at(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn is_magenta(px: [u8; 4]) -> bool {
    px[0] > 140
        && px[2] > 140
        && px[0] > px[1].saturating_add(50)
        && px[2] > px[1].saturating_add(50)
}

fn save_png(path: &Path, width: u32, height: u32, rgba: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
    out.reserve((width * height * 3) as usize);
    for px in rgba.as_chunks::<4>().0 {
        out.push(px[0]);
        out.push(px[1]);
        out.push(px[2]);
    }
    std::fs::write(path, out).expect("write PPM snapshot");
}

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let unpadded_bpr = w * 4;
    let padded_bpr = unpadded_bpr.div_ceil(256) * 256;
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-122 Readback Staging"),
        size: u64::from(padded_bpr) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-122 Readback Encoder"),
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
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    rx.recv().expect("map_async channel").expect("map_async ok");
    let mapped = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded_bpr) as usize;
        out.extend_from_slice(&mapped[start..start + unpadded_bpr as usize]);
    }
    drop(mapped);
    staging.unmap();
    out
}
