//! Regression and visual parity tests for `SH2-161`:
//! Toluca Lake (`ob01.map` / `ob.map`) pine tree neon-green streaks and dark straight rectangular
//! edge hairlines across Vanilla PS2, SH2:EE, and 4K AI Master texture tiers.
//!
//! Ground truth:
//! - `SLUS_202.28` `shGs_InitDefaultRegsEnv` (`0x001909e0`, `shGs/sh2gfw_GsInit.c:664-665`):
//!   `TEX1_1 (0x14) = 0x60` -> `MXL = 0` (single mip level 0, no mipmaps), `MMAG = 1 (LINEAR)`,
//!   `MMIN = 1 (LINEAR)`.
//! - `sh2pc.exe` `FUN_00457200` (`0x00457200`) + `FUN_004590c0` (`0x004590c0`) + `FUN_0045aaa0`
//!   (`0x0045aaa0`): `.map` embedded texture header byte `+0x15` (`mip_flag`) is `0` for `ob.map`
//!   and `ob01.map`, passing `Levels = 1` to `IDirect3DDevice8::CreateTexture` (no mipmaps).
//! - `sh2pc.exe` `FUN_005b4cc0` (`0x005b4cc0`, `sh2pc_005b0000.c:2330-2335`):
//!   `D3DTSS_ADDRESSU/V = D3DTADDRESS_CLAMP (3)`, `D3DTSS_MAGFILTER/MINFILTER = D3DTEXF_LINEAR (2)`,
//!   1x bilinear filtering (no anisotropic filtering).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
use sh2_assets::formats::texture::TextureLoader;
use sh2_core::config::FogQuality;
use sh2_core::fog::{evaluate_room_fog_with_fixes, RoomFogContext, R_OBSV_DECK_ID};
use sh2_core::lighting::lookup_draw_env;
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::light::{
    build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput, SpotlightUniforms,
};
use sh2_renderer::{
    prepare_map_texture_cpu, prepare_mipmapped_texture_cpu, GpuMap, RenderContext, RendererConfig,
};

fn game_root() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    candidates.into_iter().find(|p| {
        p.join("data/bg/ob/ob01.map").is_file() && p.join("sh2e/bg/ob/ob01.map").is_file()
    })
}

fn mods_4k_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("SH2_4K_TEXTURES_DIR")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    candidates.extend([
        PathBuf::from("mods/sh2_4k_textures"),
        PathBuf::from("../../mods/sh2_4k_textures"),
        PathBuf::from("../../../../Documents/SH2-Port/mods/sh2_4k_textures"),
    ]);
    candidates
        .into_iter()
        .find(|p| p.join("sh2_4k_textures.zip").is_file() || p.join("map").is_dir())
}

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-161 Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-161 Readback Encoder"),
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

fn write_ppm_rgb(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let mut buf = Vec::with_capacity((w * h * 3) as usize + 32);
    buf.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in rgba.as_chunks::<4>().0 {
        buf.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, buf).expect("write ppm");
}

fn build_map_bind_groups(ctx: &RenderContext, map: &MapFile) -> Vec<Arc<wgpu::BindGroup>> {
    let mut cache: HashMap<u32, Arc<wgpu::BindGroup>> = HashMap::new();
    for mat in &map.materials {
        let tid = u32::from(mat.texture_id);
        if tid != 0 && !cache.contains_key(&tid) {
            if let Some(tex) = map.textures.get(&tid) {
                if let Ok(rgba) = tex.to_rgba() {
                    let (wrap_u, wrap_v) = map.texture_uv_wrap_axes(tid);
                    let prepared = prepare_map_texture_cpu(
                        tex.width as u32,
                        tex.height as u32,
                        &rgba,
                        ctx.supports_bc_compression(),
                    );
                    let bg = ctx.upload_prepared_texture_bind_group_with_wrap(
                        &prepared,
                        wrap_u,
                        wrap_v,
                        Some(&format!("sh2_161_tid_{tid}")),
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
                .get(&u32::from(mat.texture_id))
                .cloned()
                .unwrap_or_else(|| Arc::clone(&ctx.default_texture_bind_group))
        })
        .collect()
}

/// Counts pixels with saturated neon-green chroma-key/atlas-bleed color (`G > R + 28 && G > B + 28 && G > 60`).
fn count_neon_green_pixels(rgba: &[u8]) -> usize {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|px| {
            px[1] > px[0].saturating_add(28) && px[1] > px[2].saturating_add(28) && px[1] > 60
        })
        .count()
}

#[test]
fn sh2_161_ob01_toluca_lake_pine_tree_zero_neon_green_and_hairline_artifacts() {
    let Some(root) = game_root() else {
        eprintln!("Skipping SH2-161 pine tree test: retail rip not present");
        return;
    };

    let loader =
        TextureLoader::with_4k_dir(root.join("data"), Some(root.join("sh2e")), mods_4k_dir());
    let vanilla_map = loader
        .load_vanilla_map("bg/ob/ob01.map", "ob01")
        .expect("load data/bg/ob/ob01.map");
    let sh2e_map = loader
        .load_sh2e_map("bg/ob/ob01.map", "ob01")
        .expect("load sh2e/bg/ob/ob01.map");
    let mut map_4k = sh2e_map.clone();
    let has_4k = mods_4k_dir().is_some();
    if has_4k {
        loader.apply_4k_map_texture_overrides(&mut map_4k);
    }

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 640,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-161 headless GPU test: no wgpu adapter available");
        return;
    };

    ctx.set_room_clear_color(false, None);

    // Aim directly at the Toluca Lake pine tree right behind the stone parapet wall
    // (same tree framed in the user's playtest screenshots).
    let cam_eye = glam::Vec3::new(19600.0, -1550.0, -3200.0);
    let cam_target = glam::Vec3::new(20150.0, -1200.0, 1200.0);
    let view = glam::Mat4::look_at_rh(cam_eye, cam_target, glam::Vec3::NEG_Y);
    let proj = glam::Mat4::perspective_rh(42.0_f32.to_radians(), 1.0, 50.0, 120_000.0);
    ctx.update_camera(proj * view, proj.inverse(), cam_eye);

    let ob_env = lookup_draw_env("OB", 0).expect("OB_DrawEnvData");
    let mut pool = CharacterLightPool::new();
    let spot = build_cutscene_light_uniforms(
        &mut pool,
        &CutsceneLightInput {
            draw_env: Some(ob_env),
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: (0.9_f32 * 2.2).clamp(0.25, 2.2),
            is_dark_room: false,
            cam_eye,
            actor_pos: glam::Vec3::new(18900.0, -100.0, -1350.0),
            flashlight_suppressed: false,
        },
    );
    ctx.update_spotlight(&spot);

    let eval = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_OBSV_DECK_ID,
            james_pos_y_mm: -100.0,
            camera_pos_y_mm: cam_eye.y,
            flashlight_on: false,
            extended_observation_deck_range: true,
        },
        true,
    );
    let fog_u = FogUniforms::from_room_evaluation(&eval, FogQuality::ClassicPs2, false);
    ctx.update_fog(&fog_u);

    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok().map(PathBuf::from);
    if let Some(ref dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let mut tiers: Vec<(&str, &MapFile)> = vec![("vanilla", &vanilla_map), ("sh2e", &sh2e_map)];
    if has_4k {
        tiers.push(("4k", &map_4k));
    }

    for (tier_name, map) in tiers {
        let gpu_map = GpuMap::from_map(&ctx.device, map);
        let bgs = build_map_bind_groups(&ctx, map);
        ctx.render_scene(Some(&gpu_map), Some(&bgs), &[]);
        let px = readback_ldr_rgba8(&ctx);
        let neon_count = count_neon_green_pixels(&px);
        println!("SH2-161 tier={tier_name}: neon_green_pixels={neon_count}");
        if let Some(ref dir) = out_dir {
            write_ppm_rgb(&dir.join(format!("pine_{tier_name}.ppm")), 640, 640, &px);
        }
        assert_eq!(
            neon_count, 0,
            "SH2-161 tier={tier_name} must have zero neon-green atlas-bleed pixels (got {neon_count})"
        );
    }

    if has_4k {
        let tex_4656 = map_4k.textures.get(&4656).expect("4K tid=4656");
        assert!(tex_4656.width >= 2048, "Expected 4K upscaled tid=4656");
        let rgba_4656 = tex_4656.to_rgba().expect("decode 4K tid=4656");
        // Vanilla row y=351 maps to 4K rows y=1404..1408; columns van_x=64..192 map to x=256..768.
        let mut max_seam_alpha = 0u8;
        for y in 1404..1408 {
            for x in 256..768 {
                let a = rgba_4656[(y * tex_4656.width + x) * 4 + 3];
                max_seam_alpha = max_seam_alpha.max(a);
            }
        }
        assert_eq!(
            max_seam_alpha, 0,
            "4K tid=4656 rows y=1404..1408 (vanilla y=351) must have zero alpha after sanitization"
        );
    }
}

/// Deterministic GPU regression test proving that `prepare_map_texture_cpu` (single mip level +
/// 1x bilinear filtering, matching `SLUS_202.28` `TEX1_1 = 0x60` & `sh2pc.exe` `FUN_004590c0`
/// `Levels = 1`) eliminates cross-atlas-seam neon-green bleed on a distant half-texel-inset
/// cutout quad, whereas full mipmaps + 16x anisotropy reproduce the artifact.
#[test]
fn sh2_161_single_level_bilinear_eliminates_zero_gutter_atlas_seam_bleed() {
    // 1. Unit check: cutout textures produce 1 mip level; opaque textures keep full mipmaps.
    let cutout_rgba = vec![0u8; 32 * 32 * 4];
    let opaque_rgba = vec![255u8; 32 * 32 * 4];
    let prep_cutout = prepare_map_texture_cpu(32, 32, &cutout_rgba, false);
    let prep_opaque = prepare_map_texture_cpu(32, 32, &opaque_rgba, false);
    assert_eq!(prep_cutout.mip_levels.len(), 1);
    assert!(prep_opaque.mip_levels.len() > 1);

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-161 synthetic GPU test: no wgpu adapter available");
        return;
    };

    // 512x512 packed atlas with a zero-gutter seam at y=350:
    // rows 0..350 fully transparent (alpha=0), rows 350..512 opaque neon-green (0,255,0,255).
    let (tw, th) = (512u32, 512u32);
    let mut atlas = vec![0u8; (tw * th * 4) as usize];
    for y in 350..th {
        for x in 0..tw {
            let ofs = ((y * tw + x) * 4) as usize;
            atlas[ofs] = 0;
            atlas[ofs + 1] = 255;
            atlas[ofs + 2] = 0;
            atlas[ofs + 3] = 255;
        }
    }

    // Quad mapped strictly to transparent rows 0..349 with exact PS2/PC 0.5-texel inset:
    // V in [0.5/512, 349.5/512], right next to y=350 (#00FF00).
    let v_lo = 0.5 / th as f32;
    let v_hi = 349.5 / th as f32;
    let mut map = MapFile {
        name: "atlas_seam".to_string(),
        ..Default::default()
    };
    map.materials.push(MapMaterial {
        mode: 0,
        texture_id: 1,
        color: 0xffff_ffff,
        overlay: 0xff00_0000,
        specularity: 0.0,
    });
    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 0,
        positions: vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        uvs: vec![[0.0, v_hi], [1.0, v_hi], [1.0, v_lo], [0.0, v_lo]],
        colors: vec![[255, 255, 255, 255]; 4],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    });

    // Place camera at Z=5.5 so the 351-texel region spans ~40px (dv/dy ~ 8.8 -> mip ~3.1).
    let eye = glam::Vec3::new(0.0, 0.0, 5.5);
    let view = glam::Mat4::look_at_rh(eye, glam::Vec3::ZERO, glam::Vec3::Y);
    let proj = glam::Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 0.1, 100.0);
    ctx.update_camera(proj * view, proj.inverse(), eye);
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, 0.0, 5.5, 1.0],
        direction_inner: [0.0, 0.0, -1.0, -1.0],
        color_outer: [1.0, 1.0, 1.0, -1.0],
        ambient_tint: [1.0, 1.0, 1.0, 1.0],
        ..Default::default()
    });
    ctx.set_room_clear_color(false, Some([120, 120, 128, 255]));

    let gpu_map = GpuMap::from_map(&ctx.device, &map);

    // 1. Single-level + 1x bilinear (`prepare_map_texture_cpu`): ZERO neon-green bleed.
    let fixed_prep = prepare_map_texture_cpu(tw, th, &atlas, ctx.supports_bc_compression());
    let fixed_bg = vec![ctx.upload_prepared_texture_bind_group_with_wrap(
        &fixed_prep,
        false,
        false,
        Some("SH2-161 Single-Level Atlas"),
    )];
    ctx.render_scene(Some(&gpu_map), Some(&fixed_bg), &[]);
    let fixed_px = readback_ldr_rgba8(&ctx);
    assert_eq!(
        count_neon_green_pixels(&fixed_px),
        0,
        "Single-level 1x bilinear cutout atlas must have zero cross-seam neon-green bleed"
    );

    // 2. Contrast with full mipmap chain + 16x anisotropy: reproduces cross-seam neon-green bleed.
    let mip_prep = prepare_mipmapped_texture_cpu(tw, th, &atlas, ctx.supports_bc_compression());
    let mip_bg = vec![ctx.upload_prepared_texture_bind_group_with_wrap(
        &mip_prep,
        false,
        false,
        Some("SH2-161 Mipmapped Atlas"),
    )];
    ctx.render_scene(Some(&gpu_map), Some(&mip_bg), &[]);
    let mip_px = readback_ldr_rgba8(&ctx);
    assert!(
        count_neon_green_pixels(&mip_px) > 0,
        "Expected full mipmap chain + 16x anisotropy to reproduce cross-seam neon-green bleed"
    );
}
