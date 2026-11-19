//! Headless GPU integration test for `SH2-33`: authentic 1:1 room darkness modulation,
//! flashlight acquisition state (`s_flash_level = 1.0` pre-pickup vs `0.3` post-pickup),
//! camera view-space directional fill light, and calibrated flashlight reach.

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile};
use sh2_core::config::LightingProfile;
use sh2_core::lighting::{evaluate_room_darkness, RoomDarknessInput, ShadowChapter};
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::{RenderContext, RendererConfig};

fn build_wall_map() -> MapFile {
    // Wall quad at z = -2000 mm facing +Z toward the camera at (0, 0, 0)
    let positions = vec![
        [-2000.0, -2000.0, -2000.0],
        [2000.0, -2000.0, -2000.0],
        [2000.0, 2000.0, -2000.0],
        [-2000.0, 2000.0, -2000.0],
    ];
    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let colors = vec![
        [255, 255, 255, 255],
        [255, 255, 255, 255],
        [255, 255, 255, 255],
        [255, 255, 255, 255],
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];

    MapFile {
        name: "sh2_33_dark_wall".to_string(),
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

fn readback_mean_luma(ctx: &RenderContext) -> f32 {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let row_bytes = (w * 4).div_ceil(256) * 256;
    let buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-33 Readback"),
        size: u64::from(row_bytes * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-33 Readback Enc"),
        });
    enc.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &ctx.ldr_output_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buf,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
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
    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    rx.recv().expect("map channel").expect("map ok");
    let mapped = slice.get_mapped_range();
    let mut sum = 0.0_f64;
    let mut count = 0_u64;
    for y in 0..h {
        let start = (y * row_bytes) as usize;
        for x in 0..w {
            let i = start + (x * 4) as usize;
            let luma = 0.2126 * f64::from(mapped[i])
                + 0.7152 * f64::from(mapped[i + 1])
                + 0.0722 * f64::from(mapped[i + 2]);
            sum += luma;
            count += 1;
        }
    }
    drop(mapped);
    buf.unmap();
    (sum / count.max(1) as f64) as f32
}

#[test]
fn sh2_33_pre_pickup_dark_room_renders_brighter_than_post_pickup_off() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 128,
        height: 128,
        ..Default::default()
    }) else {
        return;
    };
    let map = build_wall_map();
    let gpu_map = GpuMap::from_map(&ctx.device, &map);

    let cam_pos = glam::Vec3::new(0.0, 0.0, 0.0);
    let view = glam::Mat4::look_at_rh(cam_pos, glam::Vec3::new(0.0, 0.0, -2000.0), glam::Vec3::Y);
    let (proj, inv_proj) = ctx.perspective_projection(50.0_f32.to_radians(), 10.0, 20_000.0);
    ctx.update_camera(proj * view, inv_proj, cam_pos);

    let no_fog = FogUniforms {
        density: 0.0,
        ..Default::default()
    };
    ctx.update_fog(&no_fog);

    // 1. Pre-pickup dark room (`!has_flashlight`, flashlight OFF): s_flash_level = 1.0
    let pre = evaluate_room_darkness(&RoomDarknessInput {
        packed_map_id: None,
        room_id: None,
        chapter: ShadowChapter::MainScenario,
        is_dark_room: true,
        has_flashlight: false,
        flashlight_on: false,
        flag_625_set: false,
        world_color_rgb: None,
        lighting_profile: LightingProfile::EnhancedEdition,
    });
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, 0.0, 0.0, 1.0],
        direction_inner: [0.0, 0.0, -1.0, 0.90],
        color_outer: [0.0, 0.0, 0.0, 0.70],
        ambient_tint: [1.0, 1.0, 1.0, pre.ambient_scale],
        fill_and_range: [
            pre.camera_fill_k,
            pre.flashlight_reach_mm,
            pre.flash_level,
            0.0,
        ],
    });
    ctx.render_gpu_map(&gpu_map);
    let luma_pre = readback_mean_luma(&ctx);

    // 2. Post-pickup dark room (`has_flashlight`, flashlight OFF): s_flash_level = 0.3
    let post = evaluate_room_darkness(&RoomDarknessInput {
        packed_map_id: None,
        room_id: None,
        chapter: ShadowChapter::MainScenario,
        is_dark_room: true,
        has_flashlight: true,
        flashlight_on: false,
        flag_625_set: true,
        world_color_rgb: None,
        lighting_profile: LightingProfile::EnhancedEdition,
    });
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, 0.0, 0.0, 1.0],
        direction_inner: [0.0, 0.0, -1.0, 0.90],
        color_outer: [0.0, 0.0, 0.0, 0.70],
        ambient_tint: [1.0, 1.0, 1.0, post.ambient_scale],
        fill_and_range: [
            post.camera_fill_k,
            post.flashlight_reach_mm,
            post.flash_level,
            0.0,
        ],
    });
    ctx.render_gpu_map(&gpu_map);
    let luma_post_off = readback_mean_luma(&ctx);

    assert!(
        luma_pre > luma_post_off + 2.0,
        "Pre-pickup dark room (s_flash_level=1.0, luma={luma_pre:.2}) must render measurably brighter than post-pickup flashlight OFF (s_flash_level=0.3, luma={luma_post_off:.2})"
    );

    // 3. Post-pickup dark room with flashlight ON (`s_flash_level = 0.0`, beam ON)
    let on = evaluate_room_darkness(&RoomDarknessInput {
        packed_map_id: None,
        room_id: None,
        chapter: ShadowChapter::MainScenario,
        is_dark_room: true,
        has_flashlight: true,
        flashlight_on: true,
        flag_625_set: true,
        world_color_rgb: None,
        lighting_profile: LightingProfile::EnhancedEdition,
    });
    ctx.update_spotlight(&SpotlightUniforms {
        position: [0.0, 0.0, 0.0, 1.0],
        direction_inner: [0.0, 0.0, -1.0, 0.90],
        color_outer: [1.4, 1.35, 1.15, 0.70],
        ambient_tint: [1.0, 1.0, 1.0, on.ambient_scale],
        fill_and_range: [
            on.camera_fill_k,
            on.flashlight_reach_mm,
            on.flash_level,
            0.0,
        ],
    });
    ctx.render_gpu_map(&gpu_map);
    let luma_on = readback_mean_luma(&ctx);

    assert!(
        luma_on > luma_post_off + 5.0,
        "Flashlight ON (luma={luma_on:.2}) must visibly illuminate the wall compared to flashlight OFF (luma={luma_post_off:.2})"
    );
}
