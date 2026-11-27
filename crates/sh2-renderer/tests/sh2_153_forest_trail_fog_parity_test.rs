//! Headless 1:1 visual & optical-depth parity test for Forest Trail (`ca11` / `wp01`) and
//! Opening Restroom (`er01`) under both Vanilla (`ClassicPs2`) and Enhanced (`ModernVolumetric`)
//! fog modes (`SH2-153`).
//!
//! Ground-truth references:
//! - `SLUS_202.28` `CA_DrawEnvData[1]` (`0x00361d00`) / `sh2pc.exe` (`0x3f43c0`)
//! - `SLUS_202.28` `SetupDrawEnvData` (`0x00251370:344-362`) / `sh2pc.exe` (`sh2pc_00470000.c:9133-9154`)
//! - `SLUS_202.28` `ER_DrawEnvData[2]` (`0x00363440`, `room_no = 1`)
//! - Retail `data/bg/ca/caGB.fcl` (`FogEnv[0]` @ `0x1100`, `alpha = 0x10`)

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::texture::TextureLoader;
use sh2_core::config::FogQuality;
use sh2_core::fog::{
    evaluate_room_fog_with_fixes, RoomFogContext, CA_FOG_ENV_ALPHA_U8, R_FOREST_CEMETERY_ID,
    R_RESTROOM_START_ID,
};
use sh2_renderer::fog::{
    FogUniforms, ScreenFogState, ScreenFogStepInput, PS2_DEFAULT_FOG_WIND_VEC,
    PS2_FOG_DEFAULT_COLOR_ALPHA_U8,
};
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::{GpuMap, RenderContext, RendererConfig};

fn game_root() -> Option<PathBuf> {
    let p = PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    p.exists().then_some(p)
}

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-153 Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-153 Readback Encoder"),
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
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let start = row * bytes_per_row as usize;
        out.extend_from_slice(&mapped[start..start + (w as usize) * 4]);
    }
    drop(mapped);
    readback.unmap();
    out
}

fn build_map_bind_groups(ctx: &RenderContext, map: &MapFile) -> Vec<Arc<wgpu::BindGroup>> {
    let mut out = Vec::with_capacity(map.materials.len());
    for (idx, mat) in map.materials.iter().enumerate() {
        let bg = if let Some(tex) = map.textures.get(&u32::from(mat.texture_id)) {
            if let Ok(rgba) = tex.to_rgba() {
                ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&format!("{}_mat_{idx}", map.name)),
                )
            } else {
                Arc::clone(&ctx.default_texture_bind_group)
            }
        } else {
            Arc::clone(&ctx.default_texture_bind_group)
        };
        out.push(bg);
    }
    out
}

fn mean_luma(rgba: &[u8]) -> f32 {
    let mut sum = 0.0f64;
    let count = (rgba.len() / 4).max(1) as f64;
    for px in rgba.as_chunks::<4>().0 {
        sum += 0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2]);
    }
    (sum / count) as f32
}

fn bright_white_fraction(rgba: &[u8]) -> f32 {
    let total = (rgba.len() / 4).max(1) as f32;
    let blown = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] >= 200 && px[1] >= 200 && px[2] >= 200)
        .count() as f32;
    blown / total
}

fn write_ppm_rgb(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let mut buf = Vec::with_capacity((w * h * 3) as usize + 32);
    buf.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in rgba.as_chunks::<4>().0 {
        buf.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, buf).expect("write ppm");
}

#[test]
fn sh2_153_forest_trail_ca11_and_restroom_er01_fog_parity_vanilla_and_enhanced() {
    let Some(root) = game_root() else {
        return;
    };
    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-153 headless GPU render: no wgpu adapter available");
        return;
    };

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    if let Ok(fog_tex) = loader.load_texture("pic/effect/fog.tex") {
        ctx.upload_fog_noise_texture(&fog_tex.rgba, fog_tex.width as u32, fog_tex.height as u32);
    }

    // 1. Forest Trail (`ca11.map`, `R_FOREST_CEMETERY_ID`):
    // Exact `SetupDrawEnvData` daytime clear color `[0x46, 0x46, 0x46, 0x40]`.
    ctx.set_room_clear_color(false, None);
    let ca11_map = loader
        .load_vanilla_map("bg/ca/ca11.map", "ca11")
        .expect("load ca11.map");
    let ca11_gpu = GpuMap::from_map(&ctx.device, &ca11_map);
    let ca11_bgs = build_map_bind_groups(&ctx, &ca11_map);

    // Compute grounded midpoint camera inside `ca11.map` geometry looking down the forest trail
    let mut min_v = glam::Vec3::splat(f32::INFINITY);
    let mut max_v = glam::Vec3::splat(f32::NEG_INFINITY);
    for b in &ca11_map.batches {
        for &p in &b.positions {
            let v = glam::Vec3::from_array(p);
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
    }
    let center = (min_v + max_v) * 0.5;
    // PS2 Y-down: camera ~1.5 m above median ground looking along -Z trail axis
    let eye = glam::Vec3::new(center.x, center.y - 1200.0, center.z + 2500.0);
    let target = glam::Vec3::new(center.x, center.y - 900.0, center.z - 2500.0);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::NEG_Y);
    let (proj, inv_proj) = ctx.perspective_projection(45.0_f32.to_radians(), 100.0, 40_000.0);
    ctx.update_camera(proj * view, inv_proj, eye);

    let mut light_pool = CharacterLightPool::new();
    let ca_env = sh2_core::lighting::lookup_draw_env("CA", 11);
    let ca_light = build_cutscene_light_uniforms(
        &mut light_pool,
        &CutsceneLightInput {
            draw_env: ca_env,
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: 1.0,
            is_dark_room: false,
            cam_eye: eye,
            actor_pos: target,
            flashlight_suppressed: true,
        },
    );
    ctx.update_spotlight(&ca_light);

    let mut screen_state = ScreenFogState::default();
    let screen_eval = screen_state.step(&ScreenFogStepInput {
        cam_delta: glam::Vec3::ZERO,
        area_size: sh2_core::fog::PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
        wind_vec: PS2_DEFAULT_FOG_WIND_VEC,
        dt_scale: 1.0,
        world_view_rot: glam::Mat3::from_mat4(view),
        fog_color_a: PS2_FOG_DEFAULT_COLOR_ALPHA_U8,
        fog_alpha_param: CA_FOG_ENV_ALPHA_U8,
    });

    let ca_eval_vanilla = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            ..Default::default()
        },
        false,
    );
    let mut fog_ca_vanilla =
        FogUniforms::from_room_evaluation(&ca_eval_vanilla, FogQuality::ClassicPs2, false);
    fog_ca_vanilla.screen_fog_uv = screen_state.to_gpu_vec4(&screen_eval);
    ctx.update_fog(&fog_ca_vanilla);
    ctx.render_scene(Some(&ca11_gpu), Some(&ca11_bgs), &[]);
    let ca_vanilla_rgba = readback_ldr_rgba8(&ctx);

    let ca_eval_enh = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            ..Default::default()
        },
        true,
    );
    let mut fog_ca_enh =
        FogUniforms::from_room_evaluation(&ca_eval_enh, FogQuality::ModernVolumetric, false);
    fog_ca_enh.screen_fog_uv = screen_state.to_gpu_vec4(&screen_eval);
    fog_ca_enh.time = 2.0;
    ctx.update_fog(&fog_ca_enh);
    ctx.render_scene(Some(&ca11_gpu), Some(&ca11_bgs), &[]);
    let ca_enh_rgba = readback_ldr_rgba8(&ctx);

    let ca_v_luma = mean_luma(&ca_vanilla_rgba);
    let ca_e_luma = mean_luma(&ca_enh_rgba);
    let ca_v_white = bright_white_fraction(&ca_vanilla_rgba);
    let ca_e_white = bright_white_fraction(&ca_enh_rgba);
    println!(
        "SH2-153 ca11: vanilla_luma={ca_v_luma:.2} (white={:.1}%), enhanced_luma={ca_e_luma:.2} (white={:.1}%)",
        ca_v_white * 100.0,
        ca_e_white * 100.0
    );
    assert!(
        ca_v_luma > 15.0 && ca_v_luma < 140.0,
        "ca11 Vanilla fog should be moody PS2 0x46 mist, not black or white wall: {ca_v_luma:.2}"
    );
    assert!(
        ca_e_luma > 15.0 && ca_e_luma < 140.0,
        "ca11 Enhanced ModernVolumetric fog must not blow out white: {ca_e_luma:.2}"
    );
    assert!(
        ca_e_white < 0.05 && ca_v_white < 0.05,
        "ca11 must have <5% blown-out white pixels (v={ca_v_white:.3}, e={ca_e_white:.3})"
    );
    assert!(
        (ca_e_luma - ca_v_luma).abs() < 12.0,
        "ModernVolumetric optical depth must track ClassicPs2 closely: v={ca_v_luma:.2}, e={ca_e_luma:.2}"
    );

    // 2. Opening Restroom (`er01.map`, `R_RESTROOM_START_ID`):
    let er01_map = loader
        .load_vanilla_map("bg/er/er01.map", "er01")
        .expect("load er01.map");
    let er01_gpu = GpuMap::from_map(&ctx.device, &er01_map);
    let er01_bgs = build_map_bind_groups(&ctx, &er01_map);

    let er_eye = glam::Vec3::new(-18_400.0, -1_500.0, 21_400.0);
    let er_target = glam::Vec3::new(-20_000.0, -1_300.0, 20_430.0);
    let er_view = glam::Mat4::look_at_rh(er_eye, er_target, glam::Vec3::NEG_Y);
    let (er_proj, er_inv_proj) = ctx.perspective_projection(45.0_f32.to_radians(), 100.0, 20_000.0);
    ctx.update_camera(er_proj * er_view, er_inv_proj, er_eye);

    let er_env = sh2_core::lighting::lookup_draw_env("ER", 1);
    let er_light = build_cutscene_light_uniforms(
        &mut light_pool,
        &CutsceneLightInput {
            draw_env: er_env,
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: 1.0,
            is_dark_room: false,
            cam_eye: er_eye,
            actor_pos: er_target,
            flashlight_suppressed: true,
        },
    );
    ctx.update_spotlight(&er_light);

    let er_eval = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_RESTROOM_START_ID,
            ..Default::default()
        },
        false,
    );
    let fog_er_vanilla = FogUniforms::from_room_evaluation(&er_eval, FogQuality::ClassicPs2, false);
    ctx.update_fog(&fog_er_vanilla);
    ctx.render_scene(Some(&er01_gpu), Some(&er01_bgs), &[]);
    let er_vanilla_rgba = readback_ldr_rgba8(&ctx);

    let fog_er_enh =
        FogUniforms::from_room_evaluation(&er_eval, FogQuality::ModernVolumetric, false);
    ctx.update_fog(&fog_er_enh);
    ctx.render_scene(Some(&er01_gpu), Some(&er01_bgs), &[]);
    let er_enh_rgba = readback_ldr_rgba8(&ctx);

    let er_v_luma = mean_luma(&er_vanilla_rgba);
    let er_e_luma = mean_luma(&er_enh_rgba);
    println!("SH2-153 er01: vanilla_luma={er_v_luma:.2}, enhanced_luma={er_e_luma:.2}");
    assert!(
        (er_v_luma - er_e_luma).abs() < 1.0,
        "er01 density=0.0 must match identically between Vanilla and Enhanced: v={er_v_luma:.2}, e={er_e_luma:.2}"
    );

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm_rgb(
            &dir.join("sh2_153_ca11_vanilla.ppm"),
            640,
            360,
            &ca_vanilla_rgba,
        );
        write_ppm_rgb(
            &dir.join("sh2_153_ca11_enhanced.ppm"),
            640,
            360,
            &ca_enh_rgba,
        );
        write_ppm_rgb(
            &dir.join("sh2_153_er01_vanilla.ppm"),
            640,
            360,
            &er_vanilla_rgba,
        );
        write_ppm_rgb(
            &dir.join("sh2_153_er01_enhanced.ppm"),
            640,
            360,
            &er_enh_rgba,
        );
    }
}
