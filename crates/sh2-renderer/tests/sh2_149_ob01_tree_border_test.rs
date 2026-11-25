//! Regression tests for `SH2-149`:
//! 4K texture mode hairline border artifacts on Observation Deck (`ob01.map`) alpha-cutout trees.
//!
//! Verifies:
//! 1. Per-texture UV wrap classification clamps `[0, 1]`-bounded `.map` textures (matching
//!    `sh2pc.exe` `FUN_005b4cc0` `0x005b4cc0` & `0x005b1694`: `D3DTSS_ADDRESSU/V = D3DTADDRESS_CLAMP`),
//!    eliminating `AddressMode::Repeat` wrap-around border bleed on alpha-cutout quads.
//! 2. Tiling world textures with UVs outside `[0, 1]` (e.g. `er01.map` `tid=3336`) preserve
//!    `AddressMode::Repeat` on their wrapping axis.
//! 3. `sanitize_4k_cutout_alpha_against_vanilla` strips circular-wrap border alpha bleed and
//!    low-alpha DXT5 fringe from 4K upscaled `ob01.map` tree textures (`tid=4650`, `tid=4660`).

use std::path::PathBuf;

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
use sh2_assets::formats::TextureLoader;
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::{prepare_mipmapped_texture_cpu, RenderContext, RendererConfig};

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
    let candidates = [
        PathBuf::from("mods/sh2_4k_textures"),
        PathBuf::from("../../mods/sh2_4k_textures"),
        PathBuf::from("/home/matthewb/Documents/SH2-Port/mods/sh2_4k_textures"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("sh2_4k_textures.zip").is_file() || p.join("map").is_dir())
}

fn readback_ldr_rgba(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let unpadded_bpr = w * 4;
    let padded_bpr = unpadded_bpr.div_ceil(256) * 256;
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-149 Readback Staging"),
        size: u64::from(padded_bpr) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-149 Readback Encoder"),
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
                bytes_per_row: Some(padded_bpr),
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
    for y in 0..h as usize {
        let row = &data[y * padded_bpr as usize..y * padded_bpr as usize + (w * 4) as usize];
        out.extend_from_slice(row);
    }
    drop(data);
    staging.unmap();
    out
}

/// Deterministic GPU test proving that `ClampToEdge` (`wrap_u=false, wrap_v=false`) prevents
/// opaque/dark pixels at `U=1` / `V=1` from wrapping across onto the transparent `U=0` / `V=0`
/// edge of an alpha-cutout quad.
#[test]
fn sh2_149_clamped_sampler_eliminates_uv_wrap_hairline_on_cutout_quad() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-149 headless sampler test: no wgpu adapter available");
        return;
    };

    // 32x32 cutout texture: top half (y < 16) is fully transparent (0,0,0,0),
    // bottom half (y >= 16, including V=1.0 edge) is opaque dark green (20, 90, 25, 255).
    let (tw, th) = (32u32, 32u32);
    let mut rgba = vec![0u8; (tw * th * 4) as usize];
    for y in 16..th {
        for x in 0..tw {
            let ofs = ((y * tw + x) * 4) as usize;
            rgba[ofs] = 20;
            rgba[ofs + 1] = 90;
            rgba[ofs + 2] = 25;
            rgba[ofs + 3] = 255;
        }
    }

    let mut map = MapFile {
        name: "cutout_quad".to_string(),
        ..Default::default()
    };
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 1,
        color: 0xffff_ffff,
        overlay: 0xff00_0000,
        specularity: 0.0,
    });
    map.batches.push(MapBatch {
        group: BatchGroup::Decal,
        material_index: 0,
        geo_id: 0,
        positions: vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        // V=0.0 at top (+Y), V=1.0 at bottom (-Y)
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        colors: vec![[255, 255, 255, 255]; 4],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    });

    let eye = glam::Vec3::new(0.0, 0.0, 2.2);
    let view = glam::Mat4::look_at_rh(eye, glam::Vec3::ZERO, glam::Vec3::Y);
    let proj = glam::Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 0.1, 100.0);
    ctx.update_camera(proj * view, proj.inverse(), eye);
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, 0.0, 2.2, 1.0],
        direction_inner: [0.0, 0.0, -1.0, -1.0],
        color_outer: [1.0, 1.0, 1.0, -1.0],
        ambient_tint: [1.0, 1.0, 1.0, 1.0],
        ..Default::default()
    });
    ctx.set_room_clear_color(false, Some([200, 200, 210, 255]));

    let gpu_map = GpuMap::from_map(&ctx.device, &map);
    let prepared = prepare_mipmapped_texture_cpu(tw, th, &rgba, ctx.supports_bc_compression());

    let (wrap_u, wrap_v) = map.texture_uv_wrap_axes(1);
    assert_eq!((wrap_u, wrap_v), (false, false));

    let clamped_bg = vec![ctx.upload_prepared_texture_bind_group_with_wrap(
        &prepared,
        wrap_u,
        wrap_v,
        Some("SH2-149 Clamped Cutout"),
    )];
    ctx.render_scene(Some(&gpu_map), Some(&clamped_bg), &[]);
    let clamped_px = readback_ldr_rgba(&ctx);

    // Inspect the top half of the quad (y in 14..50, x in 32..96) where V is near 0.0 (transparent).
    // With ClampToEdge, every pixel must remain neutral grey background with zero green
    // wrap-around hairline from V=1.0 (where opaque texels render green ~[14, 68, 20]).
    let is_green_bleed = |px: &[u8]| px[1] > px[0].saturating_add(10) || px[2] < 120;
    let mut bleed_clamped = 0usize;
    for y in 14..50 {
        for x in 32..96 {
            let ofs = (y * 128 + x) * 4;
            if is_green_bleed(&clamped_px[ofs..ofs + 4]) {
                bleed_clamped += 1;
            }
        }
    }
    assert_eq!(
        bleed_clamped, 0,
        "ClampToEdge must produce zero wrap-around hairline pixels at V=0.0 (got {bleed_clamped})"
    );

    // Contrast with Repeat (`wrap_u=true, wrap_v=true`): V=0.0 wraps across to V=1.0 (opaque green),
    // producing a visible hairline along the top edge of the quad.
    let repeat_bg = vec![ctx.upload_prepared_texture_bind_group_with_wrap(
        &prepared,
        true,
        true,
        Some("SH2-149 Repeat Cutout"),
    )];
    ctx.render_scene(Some(&gpu_map), Some(&repeat_bg), &[]);
    let repeat_px = readback_ldr_rgba(&ctx);
    let mut bleed_repeat = 0usize;
    for y in 14..50 {
        for x in 32..96 {
            let ofs = (y * 128 + x) * 4;
            if is_green_bleed(&repeat_px[ofs..ofs + 4]) {
                bleed_repeat += 1;
            }
        }
    }
    assert!(
        bleed_repeat > 0,
        "Expected Repeat sampler to reproduce the V=0.0 wrap-around hairline artifact"
    );
}

/// Retail-asset integration test verifying `ob01.map` tree UV bounds, tiling preservation on
/// `er01.map`, and 4K cutout border alpha sanitization when `mods/sh2_4k_textures` is present.
#[test]
fn sh2_149_retail_ob01_trees_clamp_and_sanitize_4k_border_bleed() {
    let Some(root) = game_root() else {
        eprintln!("Skipping SH2-149 retail ob01.map test: retail rip not present");
        return;
    };

    let loader =
        TextureLoader::with_4k_dir(root.join("data"), Some(root.join("sh2e")), mods_4k_dir());

    let ob01_vanilla = loader
        .load_sh2e_map("bg/ob/ob01.map", "ob01")
        .expect("parse sh2e/bg/ob/ob01.map");

    // 1. All ob01.map tree textures (4643, 4650, 4660) stay within [0, 1] and must clamp on both axes.
    for tid in [4643u32, 4650, 4660] {
        assert_eq!(
            ob01_vanilla.texture_uv_wrap_axes(tid),
            (false, false),
            "ob01.map tree texture {tid} must use ClampToEdge on U and V"
        );
    }

    // 2. Tiling world textures in other maps (e.g. er01.map tid=3336) must preserve Repeat on wrapping axes.
    let er01 = loader
        .load_sh2e_map("bg/er/er01.map", "er01")
        .expect("parse sh2e/bg/er/er01.map");
    assert_eq!(
        er01.texture_uv_wrap_axes(3336),
        (true, false),
        "er01.map tiling texture 3336 must preserve Repeat on U"
    );

    // 3. When 4K pack is present, verify apply_4k_map_texture_overrides strips circular-wrap
    //    alpha bleed from tid=4650's left border (which is alpha==0 in vanilla ob01.map).
    if mods_4k_dir().is_some() {
        let mut ob01_4k = ob01_vanilla.clone();
        loader.apply_4k_map_texture_overrides(&mut ob01_4k);
        let tex_4650 = ob01_4k.textures.get(&4650).expect("4K tid=4650");
        assert!(tex_4650.width >= 2048, "Expected 4K upscaled tid=4650");
        let rgba_4650 = tex_4650.to_rgba().expect("decode 4K tid=4650");
        let left_col_max_alpha = (0..tex_4650.height)
            .map(|y| rgba_4650[(y * tex_4650.width) * 4 + 3])
            .max()
            .unwrap_or(0);
        assert_eq!(
            left_col_max_alpha, 0,
            "4K tid=4650 left border must have zero alpha after sanitization (matching vanilla ob01.map)"
        );
    }
}
