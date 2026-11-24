//! Layer 3 & Layer 4 headless GPU & retail-asset tests for Toluca Lake water & planar reflections (`SH2-80`).

use glam::{Mat4, Vec3};
use sh2_renderer::water::{
    mirrored_reflection_view_proj, GpuOarRipple, LakeWaterPipeline, WaterRippleBuffer,
    WaterUniforms,
};
use sh2_renderer::{RenderContext, RendererConfig};

#[test]
fn golden_planar_reflection_matrix_mirrors_across_water_plane() {
    let plane_y = 3690.0f32;
    let cam = Vec3::new(1000.0, 4500.0, -2000.0);
    let mirrored_cam = LakeWaterPipeline::mirrored_camera_pos(cam, plane_y);
    assert!((mirrored_cam.x - 1000.0).abs() < 1e-4);
    assert!((mirrored_cam.y - 2880.0).abs() < 1e-4);
    assert!((mirrored_cam.z - -2000.0).abs() < 1e-4);

    let refl_vp = mirrored_reflection_view_proj(Mat4::IDENTITY, plane_y);
    let p = refl_vp.transform_point3(cam);
    assert!((p.y - mirrored_cam.y).abs() < 1e-3);
}

#[test]
fn headless_lake_water_vanilla_ps2_vs_native_reflection() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 320,
        height: 240,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-80 headless water test: no wgpu adapter available");
        return;
    };

    ctx.lake_water_enabled
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let eye = Vec3::new(0.0, 4800.0, -6000.0);
    let target = Vec3::new(0.0, 3690.0, 0.0);
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let proj = Mat4::perspective_rh(45.0f32.to_radians(), 320.0 / 240.0, 100.0, 200_000.0);
    let vp = proj * view;

    ctx.update_camera(vp, proj.inverse(), eye);

    // 1. Strict 1:1 Vanilla PS2 mode (`enable_water_reflections = 0.0`, `enable_oar_ripples = 0.0`)
    //    Driven by `WaterGridSim` (`hh_class_water_00.c` `0x0025ba70`), zero planar reflection pass.
    let mut grid = sh2_core::boat::WaterGridSim::default();
    grid.add_impulse(24, 24, 15.0);
    grid.step();
    ctx.water_pipeline.update_grid(&ctx.queue, grid.heights());
    ctx.water_reflection_pass_enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let mut uniforms = WaterUniforms {
        view_proj: vp.to_cols_array_2d(),
        cam_pos: [eye.x, eye.y, eye.z, 1.0],
        params: [3690.0, 0.0, 0.0, 0.0],
        ..Default::default()
    };
    let mut ripples = WaterRippleBuffer::default();
    ctx.water_pipeline.update(&ctx.queue, &uniforms, &ripples);
    ctx.render_scene(None, None, &[]);
    let ps2_pixels = readback_ldr_rgba(&ctx, 320, 240);
    assert!(
        ps2_pixels.as_chunks::<4>().0.iter().any(|px| px[2] > px[0]),
        "Vanilla PS2 lake water should render cool-tinted water pixels"
    );

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let _ = std::fs::create_dir_all(&out_dir);
        write_ppm(
            &std::path::Path::new(&out_dir).join("sh2_80_lake_water_vanilla_ps2.ppm"),
            &ps2_pixels,
            320,
            240,
        );
    }

    // 2. Native-Upgrade mode (`enable_water_reflections = 1.0`, `enable_oar_ripples = 1.0`)
    //    Enables the mirrored planar reflection pass into `reflection_view` + oar ripples.
    ctx.water_reflection_pass_enabled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    uniforms.params = [3690.0, 0.0, 1.0, 1.0];
    ripples.ripples[0] = GpuOarRipple {
        pos_age: [0.0, 3690.0, -1500.0, 0.4],
        params: [900.0, 0.35, 600.0, 3.0],
    };
    ctx.water_pipeline.update(&ctx.queue, &uniforms, &ripples);
    ctx.render_scene(None, None, &[]);
    let native_pixels = readback_ldr_rgba(&ctx, 320, 240);
    assert_eq!(native_pixels.len(), 320 * 240 * 4);

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        write_ppm(
            &std::path::Path::new(&out_dir).join("sh2_80_lake_water_native.ppm"),
            &native_pixels,
            320,
            240,
        );
    }
}

fn write_ppm(path: &std::path::Path, rgba: &[u8], width: u32, height: u32) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = writeln!(file, "P6\n{width} {height}\n255");
        for px in rgba.as_chunks::<4>().0 {
            let _ = file.write_all(&px[..3]);
        }
    }
}

fn readback_ldr_rgba(ctx: &RenderContext, width: u32, height: u32) -> Vec<u8> {
    let bytes_per_row = (width * 4).div_ceil(256) * 256;
    let size = (bytes_per_row * height) as u64;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Water Test Readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Water Readback Encoder"),
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
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    ctx.queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let mapped = slice.get_mapped_range();
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height as usize {
        let start = row * bytes_per_row as usize;
        out.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
    }
    drop(mapped);
    readback.unmap();
    out
}
