//! Retail-asset integration & visual parity tests for `SH2-183`:
//! - Multi-tile outdoor `.map` + `.cld` streaming for Stage 4 Forest Trail & Graveyard
//!   (`ca03..ca28`, `CA_OUTDOOR_BLOCKS`) and Stage 5 Town East (`cb54`, `CB_OUTDOOR_BLOCKS`).
//! - `TextureLoader`-backed `GpuOutdoorTilePool` (merging shared `<area>.map`, `sh2e/`, and 4K
//!   tiers with per-tile texture bind groups) and `CollisionWorld::merge()` across tile seams.
//! - Unified single-pass `RenderContext::render_scene_multi_map` rendering with frustum culling.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sh2_assets::AssetPathResolver;
use sh2_core::config::{FogQuality, OutdoorStreamingMode, WorldTextureQuality};
use sh2_core::fog::{evaluate_room_fog, room_id_from_stage_code, RoomFogContext};
use sh2_core::world::{OutdoorArea, OutdoorTileStreamer};
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::{GpuOutdoorTilePool, MapDrawItem, RenderContext, RendererConfig};

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bpp = 4u32;
    let unpadded = w * bpp;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;

    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-183 Readback Buffer"),
        size: u64::from(padded) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-183 Readback Encoder"),
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
    rx.recv().unwrap().unwrap();

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
    use std::io::Write;
    let mut f = std::fs::File::create(path).expect("create ppm preview");
    write!(f, "P6\n{w} {h}\n255\n").expect("write ppm header");
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for px in rgba.as_chunks::<4>().0 {
        rgb.extend_from_slice(&px[..3]);
    }
    f.write_all(&rgb).expect("write ppm body");
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

#[test]
fn sh2_183_forest_trail_and_town_east_stream_textures_and_merged_cld() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.join("bg/ca/ca11.map").exists()
        || !resolver.data_dir.join("bg/cb/cb54.map").exists()
    {
        return;
    }

    let Some(ctx) = RenderContext::new_headless(RendererConfig::default()) else {
        return;
    };

    let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
    let mut pool = GpuOutdoorTilePool::with_roots(
        resolver.data_dir.clone(),
        resolver.sh2e_dir.clone(),
        WorldTextureQuality::Sh2eeHd,
        ctx.supports_bc_compression(),
    );

    // 1. Forest Trail (`Stage 4`, `ca11` south-west quadrant -> streams `ca11`, `ca12`, `ca14`).
    let plan_ca = streamer.plan(4, 5_000.0, -15_000.0);
    assert_eq!(plan_ca.area, Some(OutdoorArea::Ca));
    assert_eq!(plan_ca.priority_gb_cam, Some("bg/ca/caGB.cam"));
    assert!(plan_ca.desired_blocks.contains(&11));
    assert!(plan_ca.desired_blocks.contains(&12));
    assert!(plan_ca.desired_blocks.contains(&14));

    let queued = pool.apply_plan(&plan_ca);
    assert!(queued >= 3, "expected >= 3 Forest Trail tiles queued");
    wait_until_resident(&mut pool, &mut streamer, &ctx, &[11, 12, 14], 15);

    for &block_no in &[11u16, 12, 14] {
        let tile = pool
            .get_tile(block_no)
            .unwrap_or_else(|| panic!("ca{block_no:02} must be resident in pool"));
        assert!(
            !tile.gpu_map.batches.is_empty(),
            "ca{block_no:02} must have compiled GPU batches"
        );
        assert_eq!(
            tile.texture_bind_groups.len(),
            tile.gpu_map.material_count,
            "ca{block_no:02} must have 1:1 per-material texture bind groups"
        );
        assert!(
            tile.collision.is_some(),
            "ca{block_no:02} must have parsed .cld collision sector"
        );
    }

    let merged_cld = pool
        .take_merged_collision_if_dirty()
        .expect("merged .cld collision world must be dirty after tile uploads");
    assert!(
        merged_cld.sectors.len() >= 3,
        "merged Forest Trail collision must contain >= 3 independent .cld sectors, got {}",
        merged_cld.sectors.len()
    );
    assert!(
        pool.take_merged_collision_if_dirty().is_none(),
        "collision dirty flag must clear after take"
    );

    // 2. Walk down the Forest Trail into the Graveyard (`ca18..ca22`) and Town East gate (`ca25..ca28`).
    let plan_graveyard = streamer.plan(4, -75_000.0, -15_000.0); // cell (-4, -1) = ca20
    assert!(plan_graveyard.desired_blocks.contains(&20));
    assert!(!plan_graveyard.evict_blocks.is_empty());
    pool.apply_plan(&plan_graveyard);
    for &ev in &plan_graveyard.evict_blocks {
        streamer.mark_evicted(ev);
    }
    wait_until_resident(&mut pool, &mut streamer, &ctx, &[20], 15);
    assert!(pool.get(20).is_some(), "graveyard tile ca20 must stream in");
    assert!(
        pool.get(11).is_none(),
        "distant tile ca11 must be evicted in 2x2 mode"
    );

    // 3. Transition to Stage 5 Town East (`cb54` arrival at cell (2, 2): center (50000, 50000)).
    let plan_cb = streamer.plan(5, 45_000.0, 45_000.0);
    assert_eq!(plan_cb.area, Some(OutdoorArea::Cb));
    assert_eq!(plan_cb.priority_gb_cam, Some("bg/cb/cbGB.cam"));
    assert!(plan_cb.desired_blocks.contains(&54));
    assert!(plan_cb.desired_blocks.contains(&55));
    pool.apply_plan(&plan_cb);
    wait_until_resident(&mut pool, &mut streamer, &ctx, &[54, 55], 15);
    let cb54 = pool.get_tile(54).expect("cb54 must be resident");
    assert_eq!(cb54.texture_bind_groups.len(), cb54.gpu_map.material_count);
    let merged_cb = pool
        .take_merged_collision_if_dirty()
        .expect("merged cb .cld must rebuild on area transition");
    assert!(merged_cb.sectors.len() >= 2);
}

#[test]
fn sh2_183_multi_map_render_eliminates_void_gap_across_ca11_ca12_seam() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.join("bg/ca/ca11.map").exists()
        || !resolver.data_dir.join("bg/ca/ca12.map").exists()
    {
        return;
    }

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: true,
        ..RendererConfig::default()
    }) else {
        return;
    };

    let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
    let mut pool = GpuOutdoorTilePool::with_roots(
        resolver.data_dir.clone(),
        resolver.sh2e_dir.clone(),
        WorldTextureQuality::Sh2eeHd,
        ctx.supports_bc_compression(),
    );

    // Stream `ca11`, `ca12`, `ca14` around the south-west corner of `ca11`.
    let plan = streamer.plan(4, 5_000.0, -15_000.0);
    pool.apply_plan(&plan);
    wait_until_resident(&mut pool, &mut streamer, &ctx, &[11, 12, 14], 15);

    let ca11 = pool.get_tile(11).expect("ca11 resident");
    let ca12 = pool.get_tile(12).expect("ca12 resident");

    // Pick a camera near the southern edge of `ca11` looking straight across the seam into `ca12`.
    let seam_y = 0.5 * (ca11.gpu_map.bounds_min[1] + ca11.gpu_map.bounds_max[1]);
    let cam_eye = glam::Vec3::new(10_000.0, seam_y - 1_600.0, -16_500.0);
    let cam_target = glam::Vec3::new(10_000.0, seam_y - 1_200.0, -28_000.0);
    let aspect = 640.0 / 360.0;
    let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 50.0, 120_000.0);
    let view = glam::Mat4::look_at_rh(cam_eye, cam_target, glam::Vec3::NEG_Y);
    ctx.update_camera(proj * view, proj.inverse(), cam_eye);

    let fog_eval = evaluate_room_fog(&RoomFogContext {
        room_id: room_id_from_stage_code("ca11"),
        ..Default::default()
    });
    let fog = FogUniforms::from_room_evaluation(&fog_eval, FogQuality::ClassicPs2, false);
    ctx.update_fog(&fog);

    // Render single-tile (`ca11` only) vs unified multi-tile (`ca11` + `ca12` + `ca14`).
    let single = [MapDrawItem::new(
        &ca11.gpu_map,
        Some(ca11.texture_bind_groups.as_slice()),
    )];
    ctx.render_scene_multi_map(&single, &[]);
    let single_rgba = readback_ldr_rgba8(&ctx);

    let mut multi = Vec::new();
    pool.collect_draw_items(&mut multi);
    assert!(multi.len() >= 3);
    ctx.render_scene_multi_map(&multi, &[]);
    let multi_rgba = readback_ldr_rgba8(&ctx);

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm_rgb(
            &dir.join("sh2_183_forest_ca11_single.ppm"),
            640,
            360,
            &single_rgba,
        );
        write_ppm_rgb(
            &dir.join("sh2_183_forest_ca11_ca12_multi.ppm"),
            640,
            360,
            &multi_rgba,
        );
    }

    // Multi-tile rendering must visibly populate pixels across the `ca11 -> ca12` horizon seam
    // where single-tile rendering only shows fog/clear void.
    let diff_pixels = single_rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(multi_rgba.as_chunks::<4>().0.iter())
        .filter(|(a, b)| {
            a[0].abs_diff(b[0]) > 4 || a[1].abs_diff(b[1]) > 4 || a[2].abs_diff(b[2]) > 4
        })
        .count();
    assert!(
        diff_pixels > 1_000,
        "Expected multi-tile render (ca11+ca12+ca14, ca12 batches={}) to draw visible geometry ahead of single-tile ca11 (diff_pixels={diff_pixels})",
        ca12.gpu_map.batches.len()
    );
}
