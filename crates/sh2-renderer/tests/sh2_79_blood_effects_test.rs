//! Offscreen GPU integration test for `SH2-79` (`Effect2/hh_class_blood_*.c`):
//! - Retail `data/pic/effect/bloodtex0.tbn2` upload
//! - Subtractive (`Cd - Cs * As`, PS2 GS `ALPHA_1 = 0x8000000042`) death pools, hit sprays,
//!   floor stains, and bloody footprints under strict Vanilla PS2 mode.

use std::path::PathBuf;

use glam::Vec3;
use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile};
use sh2_assets::formats::texture::decode_texture_bytes;
use sh2_core::config::BloodColor;
use sh2_core::effects::blood::BLOOD_DT_30HZ;
use sh2_core::effects::{BloodEffectSystem, BloodSystemConfig, ImpactDescriptor, ImpactKind};
use sh2_renderer::effects::GpuBloodQuad;
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::{RenderContext, RendererConfig};

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-79 Blood Test Readback Buffer"),
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-79 Readback Encoder"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &ctx.ldr_output_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &readback,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    ctx.queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    ctx.device.poll(wgpu::Maintain::Wait);
    let mapped = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let start = row * bytes_per_row as usize;
        out.extend_from_slice(&mapped[start..start + (w as usize) * 4]);
    }
    drop(mapped);
    readback.unmap();
    out
}

fn build_bright_floor_map() -> MapFile {
    let positions = vec![
        [-4000.0, 0.0, -4000.0],
        [4000.0, 0.0, -4000.0],
        [4000.0, 0.0, 4000.0],
        [-4000.0, 0.0, 4000.0],
    ];
    let normals = vec![[0.0, -1.0, 0.0]; 4];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let colors = vec![[220, 220, 220, 255]; 4];
    let indices = vec![0, 1, 2, 0, 2, 3];
    MapFile {
        name: "synthetic_blood_floor".to_string(),
        batches: vec![MapBatch {
            geo_id: 0,
            material_index: 0,
            group: BatchGroup::Opaque,
            positions,
            uvs,
            colors,
            normals,
            indices,
        }],
        materials: vec![],
        textures: std::collections::HashMap::new(),
    }
}

#[test]
fn offscreen_subtractive_blood_pools_and_footprints_render() {
    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 320,
        height: 240,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping headless GPU blood test: no wgpu adapter available");
        return;
    };

    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data");
    let blood_path = data_dir.join("pic/effect/bloodtex0.tbn2");
    if blood_path.is_file() {
        let bytes = std::fs::read(&blood_path).expect("read bloodtex0.tbn2");
        let decoded = decode_texture_bytes(&bytes, &blood_path).expect("decode bloodtex0.tbn2");
        ctx.upload_blood_texture(&decoded.rgba, decoded.width as u32, decoded.height as u32);
    }

    let map = build_bright_floor_map();
    let gpu_map = GpuMap::from_map(&ctx.device, &map);

    let eye = Vec3::new(0.0, -1400.0, -1600.0);
    let target = Vec3::new(0.0, 0.0, 300.0);
    let view = glam::Mat4::look_at_rh(eye, target, Vec3::NEG_Y);
    let (proj, inv_proj) = ctx.perspective_projection(45.0_f32.to_radians(), 25.0, 20000.0);
    ctx.update_camera(proj * view, inv_proj, eye);

    // Bright overhead spotlight so subtractive blood (`Cd - Cs * As`) clearly shows crimson red
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, -1500.0, 0.0, 1.0],
        direction_inner: [0.0, 1.0, 0.0, 0.99],
        color_outer: [1.0, 1.0, 1.0, 0.2],
        ..Default::default()
    });

    let mut sys = BloodEffectSystem::new();
    let blood_cfg = BloodSystemConfig {
        blood_color: BloodColor::Normal,
        ..Default::default()
    };

    sys.post_enemy_death_pool(Vec3::new(0.0, 0.0, 0.0));
    sys.post_hit_splash(Vec3::new(0.0, -700.0, 0.0), Vec3::new(0.2, -0.5, 0.8));
    for _ in 0..180 {
        sys.step(
            BLOOD_DT_30HZ,
            |_| 0.0,
            Vec3::new(5000.0, 0.0, 5000.0),
            0.0,
            false,
            blood_cfg,
        );
    }
    sys.step(BLOOD_DT_30HZ, |_| 0.0, Vec3::ZERO, 0.0, false, blood_cfg);
    for i in 1..=4 {
        sys.step(
            BLOOD_DT_30HZ,
            |_| 0.0,
            Vec3::new(0.0, 0.0, (i as f32) * 350.0),
            0.0,
            true,
            blood_cfg,
        );
    }
    sys.post_impact(ImpactDescriptor {
        kind: ImpactKind::FloorStain,
        position_mm: Vec3::new(250.0, 0.0, 250.0),
        velocity_mm_s: Vec3::ZERO,
        yaw_rad: 0.4,
        mode: 0,
    });

    let mut items = Vec::new();
    sys.build_render_items(blood_cfg, &mut items);
    assert!(!items.is_empty());

    let quads: Vec<GpuBloodQuad> = items.iter().map(GpuBloodQuad::from_render_item).collect();
    ctx.update_blood_quads(&quads, true);
    ctx.render_gpu_map(&gpu_map);

    let rgba = readback_ldr_rgba(&ctx);
    assert_eq!(rgba.len(), 320 * 240 * 4);

    // Subtractive Normal Red blood (`[40, 255, 230]`) leaves R >> G, B on bright floor pixels
    let red_dominant_pixels = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > px[1].saturating_add(15) && px[0] > px[2].saturating_add(15))
        .count();
    assert!(
        red_dominant_pixels > 50,
        "Expected subtractive crimson blood pixels, found {red_dominant_pixels}"
    );
}
