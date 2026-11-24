//! Headless visual & lighting parity regression test for `first_toilet` (`er01.map` Opening
//! Restroom Mirror cutscene) at Frame 102 (`SLUS_202.28` `sh2gfw_set_CommonCharacterLight`
//! `0x00130d20` & `DramaDemoMain` `0x001b0f00`).

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into, parse_anm,
};
use sh2_assets::formats::cls::parse_cls;
use sh2_assets::formats::demo::parse_demo_dds;
use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::formats::texture::TextureLoader;
use sh2_core::{find_drama_demo_by_id, DramaDemoPlayer};
use sh2_renderer::light::{
    build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput, SpotlightUniforms,
};
use sh2_renderer::{GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig};

const ER01_MIRROR_WALL_X: f32 = -20000.0;

fn game_root() -> Option<PathBuf> {
    let p = PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    p.exists().then_some(p)
}

fn reflect_skin_matrix_across_x(m: [[f32; 4]; 4], wall_x: f32) -> [[f32; 4]; 4] {
    let mut refl = m;
    refl[0][0] = -m[0][0];
    refl[0][1] = -m[0][1];
    refl[0][2] = -m[0][2];
    refl[0][3] = 2.0 * wall_x - m[0][3];
    refl
}

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Cutscene Test Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Cutscene Readback Encoder"),
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
fn first_toilet_frame_102_james_receives_er01_draw_env_lighting() {
    let Some(root) = game_root() else {
        return;
    };

    let dds_bytes = std::fs::read(root.join("data/demo/first_toilet/first_toilet.dds"))
        .expect("read first_toilet.dds");
    let dds = parse_demo_dds(&dds_bytes).expect("parse first_toilet.dds");
    assert_eq!(
        dds.header.total_lights(),
        0,
        "first_toilet.dds has 0 .dds lights and relies on er01 DrawEnvData"
    );

    let info = find_drama_demo_by_id("first_toilet").expect("first_toilet in catalog");
    let mut player = DramaDemoPlayer::new(*info, dds, false);
    player.set_viewport_dimensions(640.0, 360.0, player.widescreen_mode);
    player.seek_to_frame(102.0, false);

    let mdl_bytes = std::fs::read(root.join("data/chr/jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let mut model = parse_mdl(&mdl_bytes).expect("parse hhh_jms.mdl");
    let anm_bytes =
        std::fs::read(root.join("data/demo/first_toilet/hhh_jms.anm")).expect("read hhh_jms.anm");
    let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse hhh_jms.anm");
    let cls_bytes =
        std::fs::read(root.join("data/demo/first_toilet/hhh_jms.cls")).expect("read hhh_jms.cls");
    let cls = parse_cls(&cls_bytes).expect("parse hhh_jms.cls");

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let map = loader
        .load_vanilla_map("bg/er/er01.map", "er01")
        .expect("load er01.map");

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };

    // Build map GPU buffers and texture bind groups
    let gpu_map = GpuMap::from_map(&ctx.device, &map);
    let mut map_tex_bgs: Vec<Arc<wgpu::BindGroup>> = Vec::with_capacity(map.materials.len());
    for mat in &map.materials {
        let bg = if let Some(tex) = map.textures.get(&u32::from(mat.texture_id)) {
            if let Ok(rgba) = tex.to_rgba() {
                ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&format!("er01_mat_{}", map_tex_bgs.len())),
                )
            } else {
                Arc::clone(&ctx.default_texture_bind_group)
            }
        } else {
            Arc::clone(&ctx.default_texture_bind_group)
        };
        map_tex_bgs.push(bg);
    }

    // Build character texture bind groups and remap primitive texture_index
    let mut chr_tex_bgs: Vec<Arc<wgpu::BindGroup>> = Vec::new();
    let mut tid_to_slot = std::collections::HashMap::new();
    let mut tids: Vec<u32> = model.textures.keys().copied().collect();
    tids.sort_unstable();
    for tid in tids {
        let tex = &model.textures[&tid];
        let rgba = tex.to_rgba().expect("decode hhh_jms tex");
        let slot = chr_tex_bgs.len() as u16;
        chr_tex_bgs.push(ctx.create_texture_bind_group(
            tex.width as u32,
            tex.height as u32,
            &rgba,
            Some(&format!("hhh_jms_{tid}")),
        ));
        tid_to_slot.insert(tid, slot);
    }
    for pr in &mut model.primitives {
        if let Some(&slot) = tid_to_slot.get(&(pr.texture_index as u32)) {
            pr.texture_index = slot;
        }
    }

    let mut gpu_mesh = GpuSkeletalMesh::from_model(
        &ctx.device,
        &ctx.skeletal_pipeline.bone_bind_group_layout,
        &ctx.skeletal_pipeline.material_bind_group_layout,
        &model,
    );
    gpu_mesh.set_pose_visible(0x15, false);

    // Evaluate `.cls` facial morph and `.anm` skinning at frame 102
    let actor = &player.actors[0];
    let mut weights = Vec::new();
    cls.evaluate_global_frame_into(actor.global_cls_frame(102.0), false, &mut weights);
    let mut morphed = Vec::new();
    model.apply_cluster_weights_into(&weights, &mut morphed);
    let mut vtx_scratch = Vec::new();
    gpu_mesh.update_morphed_positions(&ctx.queue, &model, &morphed, &mut vtx_scratch);

    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let (f0, f1, frac) = anm.sample_span_at(actor.global_anm_frame(102.0));
    let mut frame_a = anm.frames[0].clone();
    interpolate_frames_into(&anm.frames[f0], &anm.frames[f1], frac, &mut frame_a);
    let mut pose = Vec::new();
    evaluate_pose_into(&model.parents, &bind_locals, &frame_a, &mut pose);
    let mut skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);

    let world_mat = actor.world_matrix_row_major();
    let mut mirror_skin = Vec::with_capacity(skin.len());
    for m_skin in &mut skin {
        let orig = *m_skin;
        for r in 0..4 {
            for col in 0..4 {
                m_skin[r][col] = world_mat[r][0] * orig[0][col]
                    + world_mat[r][1] * orig[1][col]
                    + world_mat[r][2] * orig[2][col]
                    + world_mat[r][3] * orig[3][col];
            }
        }
        mirror_skin.push(reflect_skin_matrix_across_x(*m_skin, ER01_MIRROR_WALL_X));
    }

    let proj =
        glam::Mat4::perspective_rh(player.camera.fov_y_radians, 640.0 / 360.0, 20.0, 50_000.0);
    let view = player.camera.view_matrix();
    ctx.update_camera(proj * view, proj.inverse(), player.camera.eye);

    let skeletal_draw = [
        (&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice())),
        (
            &gpu_mesh,
            mirror_skin.as_slice(),
            Some(chr_tex_bgs.as_slice()),
        ),
    ];

    // 1. Render with OLD broken fallback (zero direct lights, ambient only)
    let broken_spot = SpotlightUniforms {
        position: [0.0, 0.0, 0.0, 1.0],
        direction_inner: [0.0, 0.5, 0.86, -1.0],
        color_outer: [0.0, 0.0, 0.0, -1.0],
        ambient_tint: [1.0, 1.0, 1.0, 1.1],
        fill_and_range: [0.0, 1_000_000.0, 0.0, 0.0],
        ..Default::default()
    };
    ctx.update_spotlight(&broken_spot);
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
    let before_rgba = readback_ldr_rgba8(&ctx);

    // 2. Render with FIXED 1:1 `build_cutscene_light_uniforms` (`er01` DrawEnvData lights)
    let er01_env = sh2_core::lighting::lookup_draw_env("er", 1).expect("er01 draw env");
    let mut pool = CharacterLightPool::new();
    let fixed_spot = build_cutscene_light_uniforms(
        &mut pool,
        &CutsceneLightInput {
            draw_env: Some(er01_env),
            demo_lights: &[],
            ambient_tint: player.atmosphere.ambient_tint_rgba,
            base_amb_scale: 1.1,
            is_dark_room: false,
            cam_eye: player.camera.eye,
            actor_pos: actor.position,
            flashlight_suppressed: false,
        },
    );
    ctx.update_spotlight(&fixed_spot);
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
    let after_rgba = readback_ldr_rgba8(&ctx);

    // Also render James-only (without map) to measure exact character luma delta
    ctx.update_spotlight(&broken_spot);
    ctx.render_scene(None, None, &skeletal_draw);
    let before_chr = readback_ldr_rgba8(&ctx);

    ctx.update_spotlight(&fixed_spot);
    ctx.render_scene(None, None, &skeletal_draw);
    let after_chr = readback_ldr_rgba8(&ctx);

    let chr_luma_stats = |rgba: &[u8]| -> (f32, f32) {
        let mut sum = 0.0f64;
        let mut max_l = 0.0f64;
        let mut cnt = 0u64;
        for px in rgba.as_chunks::<4>().0 {
            let l = 0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]);
            if l > 2.0 {
                sum += l;
                if l > max_l {
                    max_l = l;
                }
                cnt += 1;
            }
        }
        let mean = if cnt == 0 {
            0.0
        } else {
            (sum / cnt as f64) as f32
        };
        (mean, max_l as f32)
    };

    let (luma_before, max_before) = chr_luma_stats(&before_chr);
    let (luma_after, max_after) = chr_luma_stats(&after_chr);
    println!(
        "first_toilet frame 102 James character luma: before=(mean={luma_before:.2}, max={max_before:.2}), after=(mean={luma_after:.2}, max={max_after:.2})"
    );

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm_rgb(
            &dir.join("first_toilet_102_before.ppm"),
            640,
            360,
            &before_rgba,
        );
        write_ppm_rgb(
            &dir.join("first_toilet_102_after.ppm"),
            640,
            360,
            &after_rgba,
        );
        write_ppm_rgb(
            &dir.join("first_toilet_102_chr_after.ppm"),
            640,
            360,
            &after_chr,
        );
    }

    assert!(
        luma_after > luma_before * 2.0 && max_after > max_before * 2.0,
        "Fixed cutscene lighting must lift James out of dark ambient-only silhouette (before=({luma_before:.2}, max={max_before:.2}), after=({luma_after:.2}, max={max_after:.2}))"
    );
}
