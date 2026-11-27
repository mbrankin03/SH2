//! Headless visual, lighting, foliage & multi-tile parity tests for `SH2-186`:
//! Forest Trail (`ca02..ca28`), Graveyard (`ca18..ca22`), Silent Hill Ranch / Town East gate
//! (`ca25..ca28`), and Town East (`cb54`) arrival in BOTH strict Vanilla PS2 and Enhanced modes.
//!
//! Ground-truth references:
//! - `SLUS_202.28` `CA_DrawEnvData` (`0x00361b10`) / `sh2pc.exe` (`0x007f41d0`)
//! - `SLUS_202.28` `CB_DrawEnvData` (`0x00361ef0`) / `sh2pc.exe` (`0x007f45b0`)
//! - `SLUS_202.28` `Set_DrawEnvData` (`0x00251060`) & `SetupDrawEnvData` (`0x00251370`)
//! - `sh2pc.exe` `FUN_0047c2d0` (`0x0047c2d0`, `CB`/`CC` Day `0xFFFE` vs Night `0xFFFF`)
//! - `SH2:EE` `Patches/LightingPatch.cpp:33-36` (`CemeteryLightingASM`, `0x0047C2DB`)
//! - `SH2-161` single-level 1x bilinear cutout sampling (`SLUS_202.28` `TEX1_1 = 0x60`)

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sh2_assets::formats::texture::TextureLoader;
use sh2_assets::AssetPathResolver;
use sh2_core::config::{FogQuality, LightingProfile, OutdoorStreamingMode, WorldTextureQuality};
use sh2_core::fog::{
    evaluate_room_fog_with_fixes, RoomFogContext, CA_FOG_ENV_ALPHA_U8, R_FOREST_CEMETERY_ID,
    R_TOWN_EAST_ID,
};
use sh2_core::lighting::{apply_draw_env_fixes, lookup_draw_env};
use sh2_core::world::{OutdoorArea, OutdoorTileStreamer};
use sh2_renderer::fog::{
    FogUniforms, ScreenFogState, ScreenFogStepInput, PS2_DEFAULT_FOG_WIND_VEC,
    PS2_FOG_DEFAULT_COLOR_ALPHA_U8,
};
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::{GpuOutdoorTilePool, RenderContext, RendererConfig};

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let unpadded = w * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;

    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-186 Readback Buffer"),
        size: u64::from(padded) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-186 Readback Encoder"),
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
                bytes_per_row: Some(padded),
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
    rx.recv().expect("map channel").expect("map ok");

    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        out.extend_from_slice(&data[start..start + unpadded as usize]);
    }
    drop(data);
    staging.unmap();
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

fn mean_luma(rgba: &[u8]) -> f32 {
    let mut sum = 0.0f64;
    let count = (rgba.len() / 4).max(1) as f64;
    for px in rgba.as_chunks::<4>().0 {
        sum += 0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2]);
    }
    (sum / count) as f32
}

/// Counts pixels with saturated neon-green chroma-key/atlas-bleed color (`SH2-161`).
fn count_neon_green_pixels(rgba: &[u8]) -> usize {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|px| {
            px[1] > px[0].saturating_add(28) && px[1] > px[2].saturating_add(28) && px[1] > 60
        })
        .count()
}

fn bright_white_fraction(rgba: &[u8]) -> f32 {
    let total = (rgba.len() / 4).max(1) as f32;
    let blown = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] >= 210 && px[1] >= 210 && px[2] >= 210)
        .count() as f32;
    blown / total
}

fn wait_until_resident(
    pool: &mut GpuOutdoorTilePool,
    streamer: &mut OutdoorTileStreamer,
    ctx: &RenderContext,
    expected_blocks: &[u16],
    timeout_secs: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        for block_no in pool.poll_uploads(ctx, 4) {
            streamer.mark_resident(block_no);
        }
        if expected_blocks.iter().all(|&b| pool.get(b).is_some()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct LandmarkSpec {
    slug: &'static str,
    stage_id: u16,
    stage_code: &'static str,
    room_code: &'static str,
    room_num: u16,
    fog_room_id: u32,
    anchor_xz: (f32, f32),
    primary_block: u16,
    /// Offset from primary tile center `(dx, dy_from_mid, dz)` for camera eye and look target.
    eye_offset: [f32; 3],
    target_offset: [f32; 3],
}

#[test]
fn sh2_186_forest_trail_graveyard_and_cb54_visual_lighting_foliage_parity() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.join("bg/ca/ca11.map").exists()
        || !resolver.data_dir.join("bg/ca/ca20.map").exists()
        || !resolver.data_dir.join("bg/cb/cb54.map").exists()
    {
        return;
    }

    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-186 headless GPU render: no wgpu adapter available");
        return;
    };

    let tex_loader = TextureLoader::new(resolver.data_dir.clone(), resolver.sh2e_dir.clone());
    if let Ok(fog_tex) = tex_loader.load_texture("pic/effect/fog.tex") {
        ctx.upload_fog_noise_texture(&fog_tex.rgba, fog_tex.width as u32, fog_tex.height as u32);
    }

    let landmarks = [
        // 1. `ca11` Forest Trail stairs / entry
        LandmarkSpec {
            slug: "01_ca11_forest_entry",
            stage_id: 4,
            stage_code: "CA",
            room_code: "ca11",
            room_num: 11,
            fog_room_id: R_FOREST_CEMETERY_ID,
            anchor_xz: (5_000.0, -15_000.0),
            primary_block: 11,
            eye_offset: [0.0, -1_300.0, 2_500.0],
            target_offset: [-2_000.0, -1_000.0, -4_000.0],
        },
        // 2. Mid-trail path & Well save point (`ca14`)
        LandmarkSpec {
            slug: "02_ca14_mid_trail_well",
            stage_id: 4,
            stage_code: "CA",
            room_code: "ca14",
            room_num: 14,
            fog_room_id: R_FOREST_CEMETERY_ID,
            anchor_xz: (-10_000.0, -10_000.0),
            primary_block: 14,
            eye_offset: [2_500.0, -1_400.0, 1_500.0],
            target_offset: [-4_000.0, -1_100.0, -1_500.0],
        },
        // 3. Graveyard entrance & Angela encounter area (`ca18..ca22`, centered on `ca20`)
        LandmarkSpec {
            slug: "03_ca20_graveyard_angela",
            stage_id: 4,
            stage_code: "CA",
            room_code: "ca20",
            room_num: 20,
            fog_room_id: R_FOREST_CEMETERY_ID,
            anchor_xz: (-70_000.0, -10_000.0),
            primary_block: 20,
            eye_offset: [2_500.0, -1_400.0, 1_000.0],
            target_offset: [-4_500.0, -1_100.0, -1_000.0],
        },
        // 4a. Silent Hill Ranch / Town East gate (`ca25..ca28`, centered on `ca26`)
        LandmarkSpec {
            slug: "04a_ca26_ranch_gate",
            stage_id: 4,
            stage_code: "CA",
            room_code: "ca26",
            room_num: 26,
            fog_room_id: R_FOREST_CEMETERY_ID,
            anchor_xz: (-130_000.0, -15_000.0),
            primary_block: 26,
            eye_offset: [2_000.0, -1_400.0, 1_500.0],
            target_offset: [-4_000.0, -1_100.0, -3_000.0],
        },
        // 4b. Silent Hill Town East `cb54` arrival
        LandmarkSpec {
            slug: "04b_cb54_town_east_arrival",
            stage_id: 5,
            stage_code: "CB",
            room_code: "cb54",
            room_num: 54,
            fog_room_id: R_TOWN_EAST_ID,
            anchor_xz: (48_000.0, 48_000.0),
            primary_block: 54,
            eye_offset: [0.0, -1_600.0, 3_000.0],
            target_offset: [-2_000.0, -1_300.0, -4_500.0],
        },
    ];

    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok().map(PathBuf::from);
    if let Some(ref dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let modes = [
        (
            "vanilla",
            WorldTextureQuality::OriginalPs2,
            FogQuality::ClassicPs2,
            LightingProfile::Ps2Authentic,
            false,
        ),
        (
            "enhanced",
            WorldTextureQuality::Sh2eeHd,
            FogQuality::ModernVolumetric,
            LightingProfile::EnhancedEdition,
            true,
        ),
    ];

    let mut light_pool = CharacterLightPool::new();

    for (mode_name, tex_quality, fog_quality, lighting_profile, sh2ee_fog_fixes) in modes {
        let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
        let mut pool = GpuOutdoorTilePool::with_roots(
            resolver.data_dir.clone(),
            resolver.sh2e_dir.clone(),
            tex_quality,
            ctx.supports_bc_compression(),
        );

        for lm in &landmarks {
            let plan = streamer.plan(lm.stage_id, lm.anchor_xz.0, lm.anchor_xz.1);
            assert!(
                plan.desired_blocks.contains(&lm.primary_block),
                "{}: streamer plan must contain primary block {}",
                lm.slug,
                lm.primary_block
            );
            pool.apply_plan(&plan);
            for &ev in &plan.evict_blocks {
                streamer.mark_evicted(ev);
            }
            wait_until_resident(&mut pool, &mut streamer, &ctx, &[lm.primary_block], 20);
            // Also drain any remaining queued neighbor tiles for full seamless horizon coverage
            wait_until_resident(&mut pool, &mut streamer, &ctx, &plan.desired_blocks, 20);

            let primary = pool
                .get_tile(lm.primary_block)
                .unwrap_or_else(|| panic!("{}: primary tile not resident", lm.slug));
            assert!(
                !primary.gpu_map.batches.is_empty(),
                "{}: primary tile has no batches",
                lm.slug
            );
            assert_eq!(
                primary.texture_bind_groups.len(),
                primary.gpu_map.material_count,
                "{}: 1:1 texture bind group mismatch",
                lm.slug
            );

            let min_b = glam::Vec3::from_array(primary.gpu_map.bounds_min);
            let max_b = glam::Vec3::from_array(primary.gpu_map.bounds_max);
            let center = (min_b + max_b) * 0.5;
            let eye = center + glam::Vec3::from_array(lm.eye_offset);
            let target = center + glam::Vec3::from_array(lm.target_offset);

            let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::NEG_Y);
            let (proj, inv_proj) =
                ctx.perspective_projection(45.0_f32.to_radians(), 100.0, 45_000.0);
            ctx.update_camera(proj * view, inv_proj, eye);

            let raw_env = lookup_draw_env(lm.stage_code, lm.room_num)
                .unwrap_or_else(|| panic!("{}: missing DrawEnvData", lm.slug));
            let env = apply_draw_env_fixes(raw_env, lm.room_code, lighting_profile);
            let env_lum = env.ambient[0].max(env.ambient[1]).max(env.ambient[2]);
            let is_dark_room = env_lum < 0.45;
            assert!(
                !is_dark_room,
                "{} ({mode_name}): daytime outdoor landmark must NOT resolve as dark room (ambient={:?})",
                lm.slug,
                env.ambient
            );

            ctx.set_room_clear_color(is_dark_room, None);
            let light = build_cutscene_light_uniforms(
                &mut light_pool,
                &CutsceneLightInput {
                    draw_env: Some(&env),
                    demo_lights: &[],
                    ambient_tint: [1.0, 1.0, 1.0, 1.0],
                    base_amb_scale: (env_lum * 2.2).clamp(0.25, 2.2),
                    is_dark_room,
                    cam_eye: eye,
                    actor_pos: target,
                    flashlight_suppressed: true,
                },
            );
            ctx.update_spotlight(&light);

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

            let fog_eval = evaluate_room_fog_with_fixes(
                &RoomFogContext {
                    room_id: lm.fog_room_id,
                    ..Default::default()
                },
                sh2ee_fog_fixes,
            );
            let mut fog_u = FogUniforms::from_room_evaluation(&fog_eval, fog_quality, false);
            fog_u.screen_fog_uv = screen_state.to_gpu_vec4(&screen_eval);
            if matches!(fog_quality, FogQuality::ModernVolumetric) {
                fog_u.time = 2.0;
            }
            ctx.update_fog(&fog_u);

            let mut draw_items = Vec::new();
            pool.collect_draw_items(&mut draw_items);
            assert!(
                !draw_items.is_empty(),
                "{}: expected resident outdoor draw items",
                lm.slug
            );
            ctx.render_scene_multi_map(&draw_items, &[]);
            let rgba = readback_ldr_rgba8(&ctx);

            let luma = mean_luma(&rgba);
            let neon = count_neon_green_pixels(&rgba);
            let white_frac = bright_white_fraction(&rgba);
            println!(
                "SH2-186 {} [{mode_name}]: tiles={} luma={luma:.2} neon_green={neon} white={:.1}%",
                lm.slug,
                draw_items.len(),
                white_frac * 100.0
            );

            if let Some(ref dir) = out_dir {
                write_ppm_rgb(
                    &dir.join(format!("{}_{mode_name}.ppm", lm.slug)),
                    640,
                    360,
                    &rgba,
                );
            }

            assert_eq!(
                neon, 0,
                "SH2-186 {} [{mode_name}]: SH2-161 foliage regression — found {neon} neon-green pixels",
                lm.slug
            );
            assert!(
                luma > 15.0 && luma < 155.0,
                "SH2-186 {} [{mode_name}]: expected moody daytime PS2 mist luma in (15, 155), got {luma:.2}",
                lm.slug
            );
            assert!(
                white_frac < 0.05,
                "SH2-186 {} [{mode_name}]: fog must not blow out white ({:.1}%)",
                lm.slug,
                white_frac * 100.0
            );
        }
    }

    // Confirm `OutdoorArea::Ca` & `OutdoorArea::Cb` coverage for sanity
    assert_eq!(OutdoorArea::from_stage_id(4), Some(OutdoorArea::Ca));
    assert_eq!(OutdoorArea::from_stage_id(5), Some(OutdoorArea::Cb));
}
