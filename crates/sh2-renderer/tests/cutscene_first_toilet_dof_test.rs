//! Headless visual & 1:1 parity regression test for Depth of Field (DoF) during the opening
//! bathroom in-game cutscene (`first_toilet.dds` in `er01.map`, `SH2-187`).
//!
//! Ground-truth references:
//! - `SLUS_202.28` `sh2gfw_Swap_Soft` (`0x001785e0`), `Make_Filter_Packet` `case 8` (`0x0017b280`),
//!   and `GFW/sh2_DrawEnvData.c:402,414` (`z_near = 100.0`, `z_far = 7500.0`).
//! - `sh2pc.exe` `Toilet_Dof_Filter` (`0x005a1f30..0x005a2014`), Z-plane alpha mask (`0x005a1b50`,
//!   `z_ndc = 0x3f49cd3e` -> `450.0` mm), `EffectMode = 8` dispatch (`0x004795b4..0x00479649`),
//!   and `er01` clip planes (`0x0047c139`).

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
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::post::{
    PostUniforms, POST_FLAG_ACES_TONEMAP, POST_FLAG_PS2_LCG_NOISE, POST_FLAG_SH2EE_NOISE,
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
        label: Some("DoF Test Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DoF Readback Encoder"),
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

fn roi_mean_abs_diff(a: &[u8], b: &[u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> f32 {
    let mut sum = 0u64;
    let mut count = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            sum += u64::from(a[i].abs_diff(b[i]))
                + u64::from(a[i + 1].abs_diff(b[i + 1]))
                + u64::from(a[i + 2].abs_diff(b[i + 2]));
            count += 3;
        }
    }
    if count == 0 {
        0.0
    } else {
        (sum as f32) / (count as f32)
    }
}

#[test]
fn first_toilet_dof_dual_plane_separation_and_exit_cleanup() {
    let Some(root) = game_root() else {
        return;
    };

    let dds_bytes = std::fs::read(root.join("data/demo/first_toilet/first_toilet.dds"))
        .expect("read first_toilet.dds");
    let dds = parse_demo_dds(&dds_bytes).expect("parse first_toilet.dds");
    let info = find_drama_demo_by_id("first_toilet").expect("first_toilet in catalog");
    let mut player = DramaDemoPlayer::new(*info, dds, false);
    let (w, h) = (960u32, 540u32);
    player.set_viewport_dimensions(w as f32, h as f32, player.widescreen_mode);

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
        width: w,
        height: h,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };

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

    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let er01_env = sh2_core::lighting::lookup_draw_env("er", 1).expect("er01 draw env");

    let frames = [1.0f32, 102.0, 300.0, 615.0];
    let mut weights = Vec::new();
    let mut morphed = Vec::new();
    let mut vtx_scratch = Vec::new();
    let mut pose = Vec::new();
    let mut skin = Vec::new();

    for &frame in &frames {
        player.seek_to_frame(frame, false);
        let actor = &player.actors[0];
        let view = player.camera.view_matrix();
        let fx = player.special_fx;

        assert!(fx.dof_active);
        assert!((fx.dof_focus_dist_mm - 450.0).abs() < 1e-2);
        assert_eq!(fx.dof_step_ps2_px, 2.0);

        cls.evaluate_global_frame_into(actor.global_cls_frame(frame), false, &mut weights);
        model.apply_cluster_weights_into(&weights, &mut morphed);
        gpu_mesh.update_morphed_positions(&ctx.queue, &model, &morphed, &mut vtx_scratch);

        let (f0, f1, frac) = anm.sample_span_at(actor.global_anm_frame(frame));
        let mut frame_a = anm.frames[0].clone();
        interpolate_frames_into(&anm.frames[f0], &anm.frames[f1], frac, &mut frame_a);
        evaluate_pose_into(&model.parents, &bind_locals, &frame_a, &mut pose);
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

        let proj = glam::Mat4::perspective_rh(
            player.camera.fov_y_radians,
            w as f32 / h as f32,
            25.0,
            50_000.0,
        );
        ctx.update_camera(proj * view, proj.inverse(), player.camera.eye);

        let mut pool = CharacterLightPool::new();
        let spot = build_cutscene_light_uniforms(
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
        ctx.update_spotlight(&spot);

        let skeletal_draw = [
            (&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice())),
            (
                &gpu_mesh,
                mirror_skin.as_slice(),
                Some(chr_tex_bgs.as_slice()),
            ),
        ];

        // 1. Vanilla PS2 with DoF enabled
        ctx.update_post(&PostUniforms {
            dof_tap_offset_px: fx.dof_tap_offset_px,
            dof_strength: fx.dof_strength,
            dof_focus_params: [
                fx.dof_focus_dist_mm,
                fx.dof_near_strength,
                fx.dof_far_strength,
                fx.dof_step_ps2_px,
            ],
            flags: POST_FLAG_PS2_LCG_NOISE,
            ..PostUniforms::default()
        });
        ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
        let vanilla_rgba = readback_ldr_rgba8(&ctx);

        // 2. Vanilla PS2 reference with DoF disabled (for exact plane-separation verification)
        if frame == 1.0 || frame == 615.0 {
            ctx.update_post(&PostUniforms {
                flags: POST_FLAG_PS2_LCG_NOISE,
                ..PostUniforms::default()
            });
            ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
            let nodof_rgba = readback_ldr_rgba8(&ctx);

            // Foreground 1st urinal ROI (`view_z < 450 mm`) vs James/back-wall ROI (`view_z >= 450 mm`)
            let fg_diff = roi_mean_abs_diff(&vanilla_rgba, &nodof_rgba, w, 60, 120, 320, 420);
            let bg_diff = roi_mean_abs_diff(&vanilla_rgba, &nodof_rgba, w, 580, 120, 880, 420);

            if frame == 1.0 {
                assert!(
                    bg_diff > fg_diff * 5.0 && bg_diff > 0.5,
                    "Frame 1 must blur far background more than near urinal (fg_diff={fg_diff:.3}, bg_diff={bg_diff:.3})"
                );
            } else {
                assert!(
                    fg_diff > 0.25,
                    "Frame 615 must blur foreground urinal (fg_diff={fg_diff:.3})"
                );
                assert_eq!(
                    bg_diff, 0.0,
                    "Frame 615 far_strength == 0.0 must leave James & background wall bit-exact sharp (bg_diff={bg_diff:.3})"
                );
            }
        }

        // 3. Enhanced mode (SH2:EE fine grain + ACES tonemap)
        ctx.update_post(&PostUniforms {
            dof_tap_offset_px: fx.dof_tap_offset_px,
            dof_strength: fx.dof_strength,
            dof_focus_params: [
                fx.dof_focus_dist_mm,
                fx.dof_near_strength,
                fx.dof_far_strength,
                fx.dof_step_ps2_px,
            ],
            flags: POST_FLAG_PS2_LCG_NOISE | POST_FLAG_ACES_TONEMAP | POST_FLAG_SH2EE_NOISE,
            ..PostUniforms::default()
        });
        ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
        let enhanced_rgba = readback_ldr_rgba8(&ctx);

        if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
            let dir = PathBuf::from(out_dir);
            let _ = std::fs::create_dir_all(&dir);
            write_ppm_rgb(
                &dir.join(format!("first_toilet_f{:03}_vanilla.ppm", frame as u32)),
                w,
                h,
                &vanilla_rgba,
            );
            write_ppm_rgb(
                &dir.join(format!("first_toilet_f{:03}_enhanced.ppm", frame as u32)),
                w,
                h,
                &enhanced_rgba,
            );
        }
    }

    // 4. Cutscene exit / skip cleanup (`sh2pc.exe` `0x005a200e -> FUN_00478260`)
    player.skip();
    assert!(!player.locks_gameplay_input());
    assert!(!player.special_fx.dof_active);
    assert_eq!(player.special_fx.dof_strength, 0.0);
    assert_eq!(player.special_fx.dof_focus_dist_mm, 0.0);
    assert_eq!(player.special_fx.dof_near_strength, 0.0);
    assert_eq!(player.special_fx.dof_far_strength, 0.0);
    assert_eq!(player.special_fx.dof_step_ps2_px, 0.0);
}
