//! End-to-end headless GPU & retail-asset integration tests for 1:1 PS2-accurate
//! multi-layered UV-scrolling and 3D raymarched volumetric fog (`SH2-32`).

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile};
use sh2_assets::formats::texture::decode_texture_bytes;
use sh2_core::config::FogQuality;
use sh2_core::fog::{evaluate_room_fog, FogParticleVolume, RoomFogContext, R_OBSV_DECK_ID};
use sh2_renderer::fog::{FogUniforms, GpuFogParticle};
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::{RenderContext, RendererConfig};

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-32 Fog Test Readback Buffer"),
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-32 Readback Encoder"),
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

fn build_synthetic_corridor_map() -> MapFile {
    // Long corridor floor + distant back wall at Z = -12000 mm
    let positions = vec![
        [-4000.0, 0.0, 0.0],
        [4000.0, 0.0, 0.0],
        [4000.0, 0.0, -14000.0],
        [-4000.0, 0.0, -14000.0],
        [-4000.0, 0.0, -12000.0],
        [4000.0, 0.0, -12000.0],
        [4000.0, 4000.0, -12000.0],
        [-4000.0, 4000.0, -12000.0],
    ];
    let normals = vec![
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let uvs = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 4.0],
        [0.0, 4.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];
    let colors = vec![
        [128, 128, 128, 255],
        [128, 128, 128, 255],
        [128, 128, 128, 255],
        [128, 128, 128, 255],
        [102, 102, 102, 255],
        [102, 102, 102, 255],
        [102, 102, 102, 255],
        [102, 102, 102, 255],
    ];
    let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];

    MapFile {
        name: "synthetic_fog_corridor".to_string(),
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
fn test_sh2_32_volumetric_fog_drift_curves_god_rays_and_particles() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        eprintln!("Skipping headless GPU fog test: no wgpu adapter available");
        return;
    };

    let map = build_synthetic_corridor_map();
    let gpu_map = GpuMap::from_map(&ctx.device, &map);

    let eye = glam::Vec3::new(0.0, 1200.0, -500.0);
    let target = glam::Vec3::new(0.0, 1200.0, -8000.0);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
    let (proj, inv_proj) = ctx.perspective_projection(60.0_f32.to_radians(), 25.0, 40000.0);
    ctx.update_camera(proj * view, inv_proj, eye);

    // 1. Verify dual-layer UV-scrolling drift changes rendered pixels over time
    let eval = evaluate_room_fog(&RoomFogContext {
        room_id: R_OBSV_DECK_ID,
        ..Default::default()
    });
    let mut fog_t0 = FogUniforms::from_room_evaluation(&eval, FogQuality::ModernVolumetric, false);
    fog_t0.fog_far = 14000.0;
    fog_t0.time = 0.0;
    ctx.update_spotlight(&SpotlightUniforms {
        color_outer: [0.0, 0.0, 0.0, 0.85],
        ..Default::default()
    });
    ctx.update_fog(&fog_t0);
    ctx.render_gpu_map(&gpu_map);
    let frame_t0 = readback_ldr_rgba(&ctx);

    let mut fog_t1 = fog_t0;
    fog_t1.time = 12.0;
    ctx.update_fog(&fog_t1);
    ctx.render_gpu_map(&gpu_map);
    let frame_t1 = readback_ldr_rgba(&ctx);
    assert_ne!(
        frame_t0, frame_t1,
        "dual-layer UV-scrolling fog must animate over time"
    );

    // 2. Verify ClassicPs2 vs ModernVolumetric & all 3 curve types execute cleanly
    for curve in 0..=2u32 {
        let mut classic = FogUniforms::from_room_evaluation(&eval, FogQuality::ClassicPs2, false);
        classic.curve_type = curve;
        classic.fog_far = 14000.0;
        ctx.update_fog(&classic);
        ctx.render_gpu_map(&gpu_map);
        let px = readback_ldr_rgba(&ctx);
        assert!(
            mean_luma(&px) > 10.0,
            "classic curve {curve} should render fog"
        );
    }

    // 3. Verify flashlight volumetric inscattering (god rays) adds measurable luminance
    let spot = SpotlightUniforms {
        position: [0.0, 1200.0, -500.0, 1.0],
        direction_inner: [0.0, 0.0, -1.0, 0.96],
        color_outer: [3.5, 3.2, 2.6, 0.82],
        ambient_tint: [1.0, 1.0, 1.0, 1.0],
    };
    ctx.update_spotlight(&spot);

    let mut dark_fog_no_vol = FogUniforms {
        fog_color: [0.08, 0.09, 0.11, 1.0],
        fog_near: 400.0,
        fog_far: 12000.0,
        density: 1.0,
        step_count: 24,
        curve_type: 1,
        ..Default::default()
    };
    dark_fog_no_vol.apply_video_options(FogQuality::ModernVolumetric, false);
    ctx.update_fog(&dark_fog_no_vol);
    ctx.render_gpu_map(&gpu_map);
    let luma_no_godrays = mean_luma(&readback_ldr_rgba(&ctx));

    let mut dark_fog_vol = dark_fog_no_vol;
    dark_fog_vol.apply_video_options(FogQuality::ModernVolumetric, true);
    ctx.update_fog(&dark_fog_vol);
    ctx.render_gpu_map(&gpu_map);
    let luma_with_godrays = mean_luma(&readback_ldr_rgba(&ctx));
    assert!(
        luma_with_godrays > luma_no_godrays + 1.0,
        "flashlight volumetric inscattering should brighten mist beam: with={luma_with_godrays:.2}, without={luma_no_godrays:.2}"
    );

    // 4. Verify localized 3D proximity fog particle billboards render cleanly
    let mut vol = FogParticleVolume::new(128, 2000.0, 2000.0);
    vol.step(
        1.0 / 60.0,
        0.25,
        [0.0, 1000.0, -3000.0],
        [0.0, 0.0, -500.0],
        200.0,
    );
    let gpu_particles: Vec<GpuFogParticle> = vol
        .particles()
        .iter()
        .map(|p| GpuFogParticle {
            world_pos_radius: [
                p.local_pos[0],
                1000.0 + p.local_pos[1],
                -3000.0 + p.local_pos[2],
                p.radius_mm,
            ],
            color_alpha: [0.65, 0.68, 0.72, p.alpha],
        })
        .collect();
    ctx.update_fog_particles(&gpu_particles);
    ctx.update_fog(&fog_t0);
    ctx.render_gpu_map(&gpu_map);
    let px_particles = readback_ldr_rgba(&ctx);
    assert!(mean_luma(&px_particles) > 10.0);
}

#[test]
fn test_sh2_32_retail_fog_tex_upload_when_present() {
    let fog_tex_path = std::path::Path::new(
        "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/fog.tex",
    );
    if !fog_tex_path.exists() {
        eprintln!("Skipping retail fog.tex test: retail asset not present");
        return;
    }
    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 64,
        height: 64,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        return;
    };

    let raw = std::fs::read(fog_tex_path).expect("read retail fog.tex");
    let decoded = decode_texture_bytes(&raw, fog_tex_path).expect("decode retail fog.tex");
    assert_eq!(decoded.width, 256);
    assert_eq!(decoded.height, 256);
    ctx.upload_fog_noise_texture(&decoded.rgba, decoded.width as u32, decoded.height as u32);
    let map = build_synthetic_corridor_map();
    ctx.render_map(&map);
    let px = readback_ldr_rgba(&ctx);
    assert_eq!(px.len(), 64 * 64 * 4);
}
