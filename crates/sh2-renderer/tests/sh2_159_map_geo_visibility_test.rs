//! Headless `.map` `geo_id` visibility filtering & `ob01.map` car-seat Map of Silent Hill
//! before/after parity test (`SH2-159`).
//!
//! Verified 1:1 against:
//! - `SLUS_202.28`: `EvDispControlModelEntry` (`0x001ad560`), `EvDispControlModelExec` (`0x001ad5e0`)
//! - `sh2pc.exe`: `FUN_004a2f10` (`0x004a2f10`), `FUN_004a2f60` (`0x004a2f60`),
//!   `FUN_0047d120` (`0x0047d120`), `SetDispOnOffMask` (`0x005b4760` / `0x005b47a0`),
//!   and `stg_ovservation.c` `EvRoomInit` (`0x005a1350`: room `0x03`, `flag(24) ? -1 : 0`).

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::texture::TextureLoader;
use sh2_core::events::stageparts::{DisplayControlList, MapPartVisibilityFilter};
use sh2_core::events::stages_act1::{
    act1_eval_alltime_and_room, Act1StageId, Act1VanillaFixesConfig,
};
use sh2_core::events::{EventFlags, PuzzleState};
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
        label: Some("SH2-159 Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-159 Readback Encoder"),
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

fn write_ppm_rgb(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let mut buf = Vec::with_capacity((w * h * 3) as usize + 32);
    buf.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in rgba.as_chunks::<4>().0 {
        buf.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, buf).expect("write ppm");
}

#[test]
fn parse_base_room_id_from_map_name_matches_retail_naming() {
    assert_eq!(GpuMap::parse_base_room_id_from_map_name("ob01"), 1);
    assert_eq!(
        GpuMap::parse_base_room_id_from_map_name("bg/ob/ob01.map"),
        1
    );
    assert_eq!(GpuMap::parse_base_room_id_from_map_name("er01"), 1);
    assert_eq!(GpuMap::parse_base_room_id_from_map_name("cb29"), 0x1d);
    assert_eq!(GpuMap::parse_base_room_id_from_map_name("cb46"), 0x2e);
    assert_eq!(GpuMap::parse_base_room_id_from_map_name("ap104"), 104);
    assert_eq!(GpuMap::parse_base_room_id_from_map_name("synthetic"), 0);
}

#[test]
fn geo_id_visibility_semantics_match_0x4a2f10_and_0x5b47a0() {
    // 1. `variant_no == 0` -> bit 0 set (`mask = 0x0001`), shows `variant_index == 1` in `base_room_id + part_group`.
    let mut show_list = DisplayControlList::new();
    show_list.entry(Act1StageId::Observation as u16, 0x03, 0);
    let show_filter: MapPartVisibilityFilter = show_list.exec();
    // `ob01.map`: `base_room_id = 1`, `geo_id = 0x0201` -> `part_group = 2`, `variant_index = 1` -> `1 + 2 = 0x03`.
    assert!(show_filter.is_batch_visible(1, 2, 1));
    assert!(!show_filter.is_batch_visible(1, 2, 2));
    // `variant_index == 0` is ALWAYS visible regardless of group or mask (`0x005b47ad: test cx, cx; je visible`).
    assert!(show_filter.is_batch_visible(1, 0, 0));
    assert!(show_filter.is_batch_visible(1, 2, 0));

    // 2. `variant_no == -1` -> bitmask `0x0000`, hides all switchable variants (`variant_index > 0`) in group `0x03`.
    let mut hide_list = DisplayControlList::new();
    hide_list.entry(Act1StageId::Observation as u16, 0x03, -1);
    let hide_filter = hide_list.exec();
    assert!(!hide_filter.is_batch_visible(1, 2, 1));
    assert!(!hide_filter.is_batch_visible(1, 2, 2));
    assert!(hide_filter.is_batch_visible(1, 0, 0));
    assert!(hide_filter.is_batch_visible(1, 2, 0));

    // 3. Multiple entries OR-merge bits into the same `(stage_id, room_id)` (`0x004a2f5a: or [eax+4], edx`).
    let mut multi = DisplayControlList::new();
    multi.entry(Act1StageId::ApartOut as u16, 0x2e, 2);
    multi.entry(Act1StageId::ApartOut as u16, 0x2e, 1);
    let multi_filter = multi.exec();
    assert!(!multi_filter.is_batch_visible(0x2e, 0, 1)); // bit 0 unset
    assert!(multi_filter.is_batch_visible(0x2e, 0, 2)); // bit 1 set
    assert!(multi_filter.is_batch_visible(0x2e, 0, 3)); // bit 2 set
}

#[test]
fn ob01_car_seat_map_geo_id_0x0201_toggles_with_flag_24_and_renders_before_after() {
    let Some(root) = game_root() else {
        return;
    };

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    // Verify both vanilla and SH2:EE `ob01.map` contain `geo_id = 0x0201`.
    let vanilla_map = loader
        .load_vanilla_map("bg/ob/ob01.map", "ob01")
        .expect("load vanilla ob01.map");
    assert!(
        vanilla_map.batches.iter().any(|b| b.geo_id == 0x0201),
        "data/bg/ob/ob01.map must contain geo_id=0x0201 car-seat map batch"
    );

    let sh2e_map = loader
        .load_map_with_hd_overrides("bg/ob/ob01.map", "ob01")
        .expect("load sh2e ob01.map");
    let map_prop_batches: Vec<_> = sh2e_map
        .batches
        .iter()
        .filter(|b| b.geo_id == 0x0201)
        .collect();
    assert!(
        !map_prop_batches.is_empty(),
        "sh2e/bg/ob/ob01.map must contain geo_id=0x0201 car-seat map batch(es)"
    );

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };
    // Exact `SetupDrawEnvData` (`0x00251370:358-361`) daytime clear color `[0x46, 0x46, 0x46, 0x40]`.
    ctx.set_room_clear_color(false, None);

    let gpu_map = GpuMap::from_map(&ctx.device, &sh2e_map);
    assert_eq!(gpu_map.base_room_id, 1, "ob01.map base_room_id must be 1");

    let mut map_tex_bgs: Vec<Arc<wgpu::BindGroup>> = Vec::with_capacity(sh2e_map.materials.len());
    for mat in &sh2e_map.materials {
        let bg = if let Some(tex) = sh2e_map.textures.get(&u32::from(mat.texture_id)) {
            if let Ok(rgba) = tex.to_rgba() {
                ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&format!("ob01_mat_{}", map_tex_bgs.len())),
                )
            } else {
                Arc::clone(&ctx.default_texture_bind_group)
            }
        } else {
            Arc::clone(&ctx.default_texture_bind_group)
        };
        map_tex_bgs.push(bg);
    }

    // Compute exact bounding center of the `geo_id = 0x0201` Map of Silent Hill on James's car seat.
    let mut bmin = [f32::INFINITY; 3];
    let mut bmax = [f32::NEG_INFINITY; 3];
    let mut prop_batch_count = 0usize;
    for b in &gpu_map.batches {
        if b.geo_id == 0x0201 {
            prop_batch_count += 1;
            assert_eq!(b.part_group, 2);
            assert_eq!(b.variant_index, 1);
            for axis in 0..3 {
                bmin[axis] = bmin[axis].min(b.bounds_min[axis]);
                bmax[axis] = bmax[axis].max(b.bounds_max[axis]);
            }
        }
    }
    assert!(prop_batch_count > 0);
    let target = glam::Vec3::new(
        0.5 * (bmin[0] + bmax[0]),
        0.5 * (bmin[1] + bmax[1]),
        0.5 * (bmin[2] + bmax[2]),
    );

    // Position camera looking closely at the white folded Map of Silent Hill on James's car seat
    // through the open driver-side door (`-Y` is up in PS2 world space).
    let cam_eye = target + glam::Vec3::new(-420.0, -480.0, 520.0);
    let view = glam::Mat4::look_at_rh(cam_eye, target, glam::Vec3::NEG_Y);
    let proj = glam::Mat4::perspective_rh(42.0_f32.to_radians(), 640.0 / 360.0, 25.0, 50_000.0);
    ctx.update_camera(proj * view, proj.inverse(), cam_eye);

    let ob01_env = sh2_core::lighting::lookup_draw_env("OB", 1);
    let mut pool = CharacterLightPool::new();
    let spot = build_cutscene_light_uniforms(
        &mut pool,
        &CutsceneLightInput {
            draw_env: ob01_env,
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: 1.0,
            is_dark_room: false,
            cam_eye,
            actor_pos: target,
            flashlight_suppressed: false,
        },
    );
    ctx.update_spotlight(&spot);

    // --- BEFORE: `flag(24) = false` -> `EvRoomInit` (`0x005a1350`) emits `(0x03, 0)` (map visible) ---
    let mut flags = EventFlags::new();
    let puzzle = PuzzleState::new();
    let snap_before = act1_eval_alltime_and_room(
        Act1StageId::Observation,
        1,
        [target.x, target.y, target.z],
        0,
        false,
        false,
        Act1VanillaFixesConfig::default(),
        &mut flags,
        &puzzle,
    );
    assert_eq!(snap_before.door_overrides, vec![(0x03, 0)]);
    ctx.set_map_geo_overrides(Act1StageId::Observation as u16, &snap_before.door_overrides);
    let filter_before = ctx.map_visibility_filter();
    assert_eq!(
        gpu_map.visible_batch_count_with(filter_before.as_ref()),
        gpu_map.batches.len(),
        "All ob01 batches including geo_id=0x0201 must be visible when flag(24)=false"
    );

    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &[]);
    let rgba_before = readback_ldr_rgba8(&ctx);

    // --- AFTER: `flag(24) = true` -> `EvRoomInit` (`0x005a1350`) emits `(0x03, -1)` (map hidden) ---
    flags.set(24, true);
    let snap_after = act1_eval_alltime_and_room(
        Act1StageId::Observation,
        1,
        [target.x, target.y, target.z],
        0,
        false,
        false,
        Act1VanillaFixesConfig::default(),
        &mut flags,
        &puzzle,
    );
    assert_eq!(snap_after.door_overrides, vec![(0x03, -1)]);
    ctx.set_map_geo_overrides(Act1StageId::Observation as u16, &snap_after.door_overrides);
    let filter_after = ctx.map_visibility_filter();
    assert_eq!(
        gpu_map.visible_batch_count_with(filter_after.as_ref()),
        gpu_map.batches.len() - prop_batch_count,
        "geo_id=0x0201 batches must be hidden when flag(24)=true"
    );

    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &[]);
    let rgba_after = readback_ldr_rgba8(&ctx);

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm_rgb(
            &dir.join("sh2_159_ob01_car_map_visible.ppm"),
            640,
            360,
            &rgba_before,
        );
        write_ppm_rgb(
            &dir.join("sh2_159_ob01_car_map_hidden.ppm"),
            640,
            360,
            &rgba_after,
        );
    }

    // Verify visually significant pixel difference where the white Map of Silent Hill was hidden.
    let mut changed_pixels = 0usize;
    let mut luma_before_sum = 0.0f64;
    let mut luma_after_sum = 0.0f64;
    for (p0, p1) in rgba_before
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rgba_after.as_chunks::<4>().0.iter())
    {
        let diff = (i32::from(p0[0]) - i32::from(p1[0])).abs()
            + (i32::from(p0[1]) - i32::from(p1[1])).abs()
            + (i32::from(p0[2]) - i32::from(p1[2])).abs();
        if diff > 24 {
            changed_pixels += 1;
            luma_before_sum +=
                0.299 * f64::from(p0[0]) + 0.587 * f64::from(p0[1]) + 0.114 * f64::from(p0[2]);
            luma_after_sum +=
                0.299 * f64::from(p1[0]) + 0.587 * f64::from(p1[1]) + 0.114 * f64::from(p1[2]);
        }
    }
    assert!(
        changed_pixels > 500,
        "Hiding geo_id=0x0201 must noticeably change the car-seat region (changed_pixels={changed_pixels})"
    );
    let mean_before = luma_before_sum / (changed_pixels as f64);
    let mean_after = luma_after_sum / (changed_pixels as f64);
    assert!(
        mean_before > mean_after,
        "Removing the white Map of Silent Hill must lower mean luminance in changed pixels (before={mean_before:.1}, after={mean_after:.1})"
    );
}
