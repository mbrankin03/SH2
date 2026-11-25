//! Integration tests for `SH2-100` retail `.tbn2` decoding, fire/smoke, water/rain,
//! and combat shell/spark/glass FX rendering in both Vanilla PS2 and Native-Upgrade modes.

use std::path::PathBuf;

use glam::{Mat4, Vec3};
use sh2_assets::formats::texture::decode_texture_bytes;
use sh2_core::config::{AppConfig, EnhancedPreset};
use sh2_core::effects::{
    FirePhenomenon, FxRng, GlassBreakSystem, ImpactSparkSystem, RainDropSystem, ShellCasingSystem,
    ShellWeaponKind, SmokeSystem, WaterPoolSystem, WaterSplashSystem,
};
use sh2_renderer::effects::GpuFxQuad;
use sh2_renderer::post::{PostUniforms, POST_FLAG_FIRE_HEAT_HAZE, POST_FLAG_PS2_LCG_NOISE};
use sh2_renderer::{RenderContext, RendererConfig};

fn retail_effect_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect")
}

fn readback_ldr_rgba(ctx: &RenderContext, width: u32, height: u32) -> Vec<u8> {
    let bytes_per_row = (width * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("FX Readback Buffer"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("FX Readback Encoder"),
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

#[test]
fn sh2_100_retail_tbn2_effect_textures_decode_cleanly() {
    let dir = retail_effect_dir();
    if !dir.exists() {
        return;
    }
    let expected = [
        ("fire00.tbn2", 512, 512),
        ("fire01.tbn2", 512, 512),
        ("flame003.tbn2", 512, 512),
        ("flame032.tbn2", 512, 512),
        ("water00.tbn2", 512, 512),
        ("water01.tbn2", 512, 512),
        ("water02.tbn2", 512, 512),
        ("water40.tbn2", 512, 512),
        ("sprash.tbn2", 256, 256),
        ("spark.tbn2", 512, 512),
        ("glas03.tbn2", 512, 512),
    ];
    for (name, w, h) in expected {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).expect("read retail .tbn2");
        let tex = decode_texture_bytes(&bytes, &path).expect("decode retail .tbn2");
        assert_eq!((tex.width, tex.height), (w, h), "{name} dimensions");
        assert_eq!(tex.rgba.len(), w * h * 4, "{name} rgba len");
    }
}

#[test]
fn sh2_100_headless_fx_pipeline_vanilla_ps2_vs_native_heat_haze() {
    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 320,
        height: 240,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        return;
    };

    let dir = retail_effect_dir();
    if dir.exists() {
        let fire_path = dir.join("fire00.tbn2");
        let bytes = std::fs::read(&fire_path).expect("read fire00.tbn2");
        let tex = decode_texture_bytes(&bytes, &fire_path).expect("decode fire00.tbn2");
        ctx.upload_billboard_fx_texture(&tex.rgba, tex.width as u32, tex.height as u32);
    }

    let cam_pos = Vec3::new(-100550.0, -1800.0, -72000.0);
    let target = Vec3::new(-100550.0, -1600.0, -75600.0);
    let view = Mat4::look_at_rh(cam_pos, target, Vec3::NEG_Y);
    let (proj, inv_proj) = ctx.perspective_projection(45f32.to_radians(), 25.0, 50000.0);
    ctx.update_camera_with_mode(proj * view, inv_proj, cam_pos, 0.0);

    let mut fire = FirePhenomenon::new_hotel_staircase(100);
    let mut rng = FxRng::new(100);
    let mut smoke = SmokeSystem::new(target, &mut rng);
    let mut pools = WaterPoolSystem::default();
    pools.spawn_ripple(target, &mut rng);
    let mut splashes = WaterSplashSystem::default();
    splashes.spawn_footstep_splash(target, Vec3::new(0.0, 800.0, 0.0), &mut rng);
    let mut rain = RainDropSystem::default();
    rain.spawn_around(cam_pos, 15, &mut rng);
    let mut shells = ShellCasingSystem::default();
    shells.spawn_shell(target, ShellWeaponKind::Handgun, &mut rng);
    let mut sparks = ImpactSparkSystem::default();
    sparks.spawn_impact(target, &mut rng);
    let mut glass = GlassBreakSystem::default();
    glass.trigger_break(target, Vec3::new(0.0, 0.0, 300.0));

    for _ in 0..5 {
        let dt = 1.0 / 30.0;
        fire.step(dt, true);
        smoke.step(dt);
        pools.step(dt);
        splashes.step(dt);
        rain.step(cam_pos, 0.0, dt, &mut rng);
        shells.step(dt, 0.0);
        sparks.step(dt);
        glass.step(dt, 0.0, &mut rng);
    }

    let mut add_quads = Vec::new();
    for em in &fire.emitters {
        for p in &em.particles {
            add_quads.push(GpuFxQuad::from_flame(em.origin, p));
        }
    }
    for e in &fire.embers {
        add_quads.push(GpuFxQuad::from_ember(e));
    }
    for s in &sparks.sparks {
        add_quads.push(GpuFxQuad::from_spark(s));
    }

    let mut alpha_quads = Vec::new();
    for p in &smoke.particles {
        if p.active {
            alpha_quads.push(GpuFxQuad::from_smoke(p));
        }
    }
    for r in &pools.ripples {
        alpha_quads.push(GpuFxQuad::from_water_ripple(r));
    }
    for d in &rain.drops {
        alpha_quads.push(GpuFxQuad::from_rain_drop(d));
    }
    for s in &shells.shells {
        alpha_quads.push(GpuFxQuad::from_shell(s));
    }
    for g in &glass.shards {
        alpha_quads.push(GpuFxQuad::from_glass_shard(g));
    }

    ctx.update_billboard_fx_quads(&add_quads, &alpha_quads);

    // 1. Strict Vanilla PS2 mode (`POST_FLAG_FIRE_HEAT_HAZE` cleared)
    let mut cfg = AppConfig::default();
    cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert!(!cfg.video.enable_fire_heat_haze);

    ctx.update_post(&PostUniforms {
        time: 0.5,
        flags: POST_FLAG_PS2_LCG_NOISE,
        ..PostUniforms::default()
    });
    ctx.render_scene(None, None, &[]);
    let vanilla_pixels = readback_ldr_rgba(&ctx, 320, 240);
    assert!(vanilla_pixels.iter().any(|&b| b > 0));

    // 2. Native-Upgrade mode (`POST_FLAG_FIRE_HEAT_HAZE` set)
    cfg.apply_enhanced_preset(EnhancedPreset::NativeEnhanced);
    assert!(cfg.video.enable_fire_heat_haze);

    ctx.update_post(&PostUniforms {
        time: 0.5,
        flags: POST_FLAG_PS2_LCG_NOISE | POST_FLAG_FIRE_HEAT_HAZE,
        ..PostUniforms::default()
    });
    ctx.render_scene(None, None, &[]);
    let native_pixels = readback_ldr_rgba(&ctx, 320, 240);
    assert!(native_pixels.iter().any(|&b| b > 0));
    assert_ne!(vanilla_pixels, native_pixels);
}
