//! End-to-end headless GPU & retail-asset integration tests for `SH2-98`:
//! - Retail `data/pic/effect/playable0.tbn2` upload & `Effect2/hh_class_dust_00.c` airborne dust motes
//! - `HH_Class_Raise_Dust_Phenomenon_00` burst spawning & `HH_ClassWrapper_SpotLight_ColorRatio_Calculator`
//! - Strict Vanilla PS2 mode (`FogQuality::ClassicPs2`, `volumetric_light = false`) headless PNG capture

use std::path::PathBuf;

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile};
use sh2_assets::formats::texture::decode_texture_bytes;
use sh2_core::config::FogQuality;
use sh2_core::lighting::{DustSpotlightParams, DustSystem, DUST_DT_60HZ};
use sh2_renderer::effects::GpuDustParticle;
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::{RenderContext, RendererConfig};

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-98 Dust Test Readback Buffer"),
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-98 Readback Encoder"),
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
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    rx.recv().expect("map channel").expect("map ok");

    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let start = row * bytes_per_row as usize;
        pixels.extend_from_slice(&mapped[start..start + (w * 4) as usize]);
    }
    drop(mapped);
    readback.unmap();
    pixels
}

fn build_dark_room_map() -> MapFile {
    let positions = vec![
        [-4000.0, 0.0, 0.0],
        [4000.0, 0.0, 0.0],
        [4000.0, 0.0, -8000.0],
        [-4000.0, 0.0, -8000.0],
    ];
    let normals = vec![[0.0, -1.0, 0.0]; 4];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let colors = vec![[40, 40, 40, 255]; 4];
    let indices = vec![0, 1, 2, 0, 2, 3];

    MapFile {
        name: "synthetic_dust_room".to_string(),
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

fn mean_luma(rgba: &[u8]) -> f32 {
    let mut sum = 0.0f64;
    let count = (rgba.len() / 4).max(1) as f64;
    for px in rgba.as_chunks::<4>().0 {
        sum += 0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2]);
    }
    (sum / count) as f32
}

#[test]
fn test_sh2_98_retail_playable0_dust_motes_and_vanilla_ps2_snapshot() {
    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 256,
        height: 256,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        eprintln!("Skipping headless GPU dust test: no wgpu adapter available");
        return;
    };

    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data");
    let dust_tex_path = data_dir.join("pic/effect/playable0.tbn2");
    if dust_tex_path.exists() {
        let bytes = std::fs::read(&dust_tex_path).expect("read playable0.tbn2");
        let decoded = decode_texture_bytes(&bytes, &dust_tex_path).expect("decode playable0.tbn2");
        ctx.upload_dust_texture(&decoded.rgba, decoded.width as u32, decoded.height as u32);
    }

    let map = build_dark_room_map();
    let gpu_map = GpuMap::from_map(&ctx.device, &map);

    let eye = glam::Vec3::new(0.0, -800.0, 0.0);
    let target = glam::Vec3::new(0.0, -600.0, -3000.0);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::NEG_Y);
    let (proj, inv_proj) = ctx.perspective_projection(60.0_f32.to_radians(), 25.0, 20000.0);
    ctx.update_camera(proj * view, inv_proj, eye);

    // Strict 1:1 Vanilla PS2 fog mode (no modern volumetric raymarching)
    let mut fog = FogUniforms::default();
    fog.apply_video_options(FogQuality::ClassicPs2, false);
    fog.fog_color = [0.05, 0.06, 0.07, 1.0];
    fog.density = 0.5;
    ctx.update_fog(&fog);

    let mut dust_sys = DustSystem::new();
    for _ in 0..8 {
        dust_sys.spawn_raise_dust(
            glam::Vec3::new(0.0, -400.0, -1800.0),
            glam::Vec3::new(0.0, -450.0, 150.0),
            0.0,
        );
    }
    for _ in 0..30 {
        dust_sys.tick(DUST_DT_60HZ);
    }
    assert!(!dust_sys.motes.is_empty());

    // 1. Render with flashlight OFF
    let spot_off = SpotlightUniforms {
        position: [0.0, -700.0, -200.0, 1.0],
        direction_inner: [0.0, 0.0, -1.0, 0.90],
        color_outer: [0.0, 0.0, 0.0, 0.75],
        ..Default::default()
    };
    ctx.update_spotlight(&spot_off);
    let dust_spot_off = DustSpotlightParams::default();
    let instances_off: Vec<GpuDustParticle> = dust_sys
        .motes
        .iter()
        .map(|m| GpuDustParticle::from_mote(m, &dust_spot_off))
        .collect();
    ctx.update_dust_particles(&instances_off);
    ctx.render_gpu_map(&gpu_map);
    let luma_off = mean_luma(&readback_ldr_rgba(&ctx));

    // 2. Render with flashlight ON illuminating the airborne dust motes
    let spot_on = SpotlightUniforms {
        position: [0.0, -700.0, -200.0, 1.0],
        direction_inner: [0.0, 0.1, -1.0, 0.90],
        color_outer: [1.0, 0.96, 0.88, 0.75],
        fill_and_range: [0.05, 7000.0, 1.0, 0.0],
        ..Default::default()
    };
    ctx.update_spotlight(&spot_on);
    let dust_spot_on = DustSpotlightParams {
        enabled: true,
        position_mm: glam::Vec3::new(0.0, -700.0, -200.0),
        direction: glam::Vec3::new(0.0, 0.1, -1.0).normalize(),
        outer_cos: 0.75,
        range_mm: 7000.0,
    };
    let instances_on: Vec<GpuDustParticle> = dust_sys
        .motes
        .iter()
        .map(|m| GpuDustParticle::from_mote(m, &dust_spot_on))
        .collect();
    ctx.update_dust_particles(&instances_on);
    ctx.render_gpu_map(&gpu_map);
    let rgba_on = readback_ldr_rgba(&ctx);
    let luma_on = mean_luma(&rgba_on);

    assert!(
        luma_on > luma_off,
        "flashlight cone should illuminate airborne dust motes: on={luma_on:.3}, off={luma_off:.3}"
    );

    if let Ok(out_path) = std::env::var("SH2_98_PREVIEW_OUT") {
        use std::io::Write;
        let mut file = std::fs::File::create(out_path).expect("create SH2-98 preview PPM");
        writeln!(file, "P6\n{} {}\n255", ctx.config.width, ctx.config.height)
            .expect("write PPM header");
        for px in rgba_on.as_chunks::<4>().0 {
            file.write_all(&px[..3]).expect("write PPM pixel");
        }
    }
}
