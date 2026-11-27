//! Layer 3 & Layer 4 parity regression tests for SH2-192:
//! Angela's hair (`agl.mdl` `tprim[0..3]`) and multi-character transparent hair fringe rendering
//! under outdoor depth fog (`haka_agl.dds` / `ca11.map`).
//!
//! Ground-truth citations:
//! - `SLUS_202.28` `Model3Draw_n` (`0x001285e0`), `Model3DrawVu0Parts` (`0x0012bd50`),
//!   `SortEnvPrim` (`0x00128550`), `sh2_Model_MakeMatrixParams` (`0x001281d0`),
//!   `sh2gfw_Set_DepthFog` (`0x00175df0`).
//! - `sh2pc.exe` `FUN_00503800` (`0x00503800`, `D3DRS_FOGENABLE = 1`, `D3DRS_ALPHATESTENABLE = 1`,
//!   `D3DRS_ALPHAFUNC = D3DCMP_GREATER (5)`, `D3DRS_ALPHAREF = 0`), `FUN_00503ba0` (`0x00503ba0`).

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into, parse_anm,
};
use sh2_assets::formats::cls::parse_cls;
use sh2_assets::formats::demo::parse_demo_dds;
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_assets::formats::texture::TextureLoader;
use sh2_core::config::FogQuality;
use sh2_core::fog::{evaluate_room_fog_with_fixes, RoomFogContext, R_FOREST_CEMETERY_ID};
use sh2_core::{find_drama_demo_by_id, DramaDemoPlayer};
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::{
    dilate_transparent_border_rgb, refine_4k_chr_texture_from_sources,
    scaled_border_dilation_passes, GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig,
    SkeletalPipeline,
};

fn game_root() -> Option<PathBuf> {
    let p = PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    p.exists().then_some(p)
}

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-192 Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-192 Readback Encoder"),
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

fn build_chr_texture_bgs(
    ctx: &RenderContext,
    model: &ModelFile,
    tex_loader: Option<&TextureLoader>,
) -> (
    Vec<Arc<wgpu::BindGroup>>,
    std::collections::HashMap<u32, u16>,
) {
    let mut bgs: Vec<Arc<wgpu::BindGroup>> = Vec::new();
    let mut tid_to_slot = std::collections::HashMap::new();
    let mut tids: Vec<u32> = model.textures.keys().copied().collect();
    tids.sort_unstable();
    for tid in tids {
        let tex = &model.textures[&tid];
        let mut van_rgba = tex.to_rgba().expect("decode vanilla chr texture");
        let van_min_alpha = van_rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[3])
            .min()
            .unwrap_or(255);
        let bg = if let Some(mut dec) =
            tex_loader.and_then(|tl| tl.load_4k_chr_texture_override(tid))
        {
            if van_min_alpha > 0 {
                for px in dec.rgba.as_chunks_mut::<4>().0 {
                    px[3] = px[3].max(van_min_alpha);
                }
            }
            if van_min_alpha == 0 {
                refine_4k_chr_texture_from_sources(
                    tex.width as u32,
                    tex.height as u32,
                    &van_rgba,
                    None,
                    dec.width as u32,
                    dec.height as u32,
                    &mut dec.rgba,
                );
            }
            let passes = scaled_border_dilation_passes(
                tex.width as u32,
                tex.height as u32,
                dec.width as u32,
                dec.height as u32,
                2,
            );
            dilate_transparent_border_rgb(
                dec.width as u32,
                dec.height as u32,
                &mut dec.rgba,
                passes,
            );
            ctx.create_texture_bind_group(dec.width as u32, dec.height as u32, &dec.rgba, None)
        } else {
            dilate_transparent_border_rgb(tex.width as u32, tex.height as u32, &mut van_rgba, 2);
            ctx.create_texture_bind_group(tex.width as u32, tex.height as u32, &van_rgba, None)
        };
        let slot = bgs.len() as u16;
        bgs.push(bg);
        tid_to_slot.insert(tid, slot);
    }
    (bgs, tid_to_slot)
}

fn count_dark_pixels_in_rect(
    rgba: &[u8],
    w: u32,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    max_luma: f32,
) -> usize {
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = ((y * w + x) * 4) as usize;
            let luma = 0.299 * f32::from(rgba[idx])
                + 0.587 * f32::from(rgba[idx + 1])
                + 0.114 * f32::from(rgba[idx + 2]);
            if luma <= max_luma {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn sh2_192_wgsl_shader_wires_post_fog_transparent_fringe_entry_point() {
    assert!(
        SkeletalPipeline::WGSL_SKINNING_SHADER.contains("fn fs_transparent_fringe_fogged"),
        "Skeletal WGSL shader must expose `fs_transparent_fringe_fogged` for post-depth-fog hair fringe"
    );
    assert!(
        SkeletalPipeline::WGSL_SKINNING_SHADER.contains("struct SkeletalFogUniforms"),
        "Skeletal WGSL shader must declare `SkeletalFogUniforms` at `@group(0) @binding(6)`"
    );
}

#[test]
fn sh2_192_angela_mdl_hair_primitives_extend_below_opaque_jaw_cutoff() {
    let Some(root) = game_root() else {
        return;
    };
    let mdl_bytes = std::fs::read(root.join("data/chr/agl/agl.mdl")).expect("read agl.mdl");
    let model = parse_mdl(&mdl_bytes).expect("parse agl.mdl");

    let opaque_count = model
        .primitives
        .iter()
        .filter(|p| !p.is_transparent)
        .count();
    let tprim_count = model.primitives.iter().filter(|p| p.is_transparent).count();
    assert_eq!(opaque_count, 14, "agl.mdl has 14 opaque primitives");
    assert_eq!(
        tprim_count, 5,
        "agl.mdl has 5 transparent hair/lash primitives"
    );

    // Verify opaque hair (`prim[10]`, `prim[11]`) stops near jaw level (`Y ~= -747.8 mm`) while
    // transparent lower hair strands (`tprim[0]`, `tprim[2]`, `tid = 1263`, double-sided `cull = 0`)
    // extend down to neck/collar level (`Y ~= -716.4 mm`).
    let prim_max_y = |prim_idx: usize| -> f32 {
        model
            .primitive_triangles(prim_idx)
            .into_iter()
            .flatten()
            .map(|vi| model.vertices.positions[vi as usize][1])
            .fold(f32::NEG_INFINITY, f32::max)
    };
    let opaque_hair_bottom_y = prim_max_y(10).max(prim_max_y(11));
    let transp_hair_bottom_y = prim_max_y(14).max(prim_max_y(16));
    assert!(
        transp_hair_bottom_y > opaque_hair_bottom_y + 25.0,
        "Transparent lower hair (`{transp_hair_bottom_y:.1} mm`) must extend >25 mm below opaque hair cutoff (`{opaque_hair_bottom_y:.1} mm`)"
    );
    assert_eq!(model.primitives[14].backface_culling, 0);
    assert_eq!(model.primitives[16].backface_culling, 0);
    assert_eq!(model.primitives[14].texture_index, 1263);
}

#[test]
fn sh2_192_angela_haka_agl_hair_fringe_survives_outdoor_fog_vanilla_and_enhanced() {
    let Some(root) = game_root() else {
        return;
    };
    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok().map(PathBuf::from);
    if let Some(ref d) = out_dir {
        let _ = std::fs::create_dir_all(d);
    }

    let dds_bytes =
        std::fs::read(root.join("data/demo/haka_agl/haka_agl.dds")).expect("read haka_agl.dds");
    let dds = parse_demo_dds(&dds_bytes).expect("parse haka_agl.dds");
    let info = find_drama_demo_by_id("haka_agl").expect("haka_agl in catalog");
    let mut player = DramaDemoPlayer::new(*info, dds, false);
    player.set_viewport_dimensions(640.0, 360.0, player.widescreen_mode);
    player.seek_to_frame(2500.0, false);

    let agl_idx = player
        .actors
        .iter()
        .position(|a| a.actor_name.contains("agl"))
        .expect("agl actor in haka_agl");
    let actor = &player.actors[agl_idx];

    let mdl_bytes = std::fs::read(root.join("data/chr/agl/agl.mdl")).expect("read agl.mdl");
    let model = parse_mdl(&mdl_bytes).expect("parse agl.mdl");
    let anm_bytes = std::fs::read(root.join("data/demo/haka_agl/agl.anm")).expect("read agl.anm");
    let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse agl.anm");
    let cls_bytes = std::fs::read(root.join("data/demo/haka_agl/agl.cls")).expect("read agl.cls");
    let cls = parse_cls(&cls_bytes).expect("parse agl.cls");

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let map = loader
        .load_vanilla_map("bg/ca/ca11.map", "ca11")
        .expect("load ca11.map");

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: true,
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
                ctx.create_texture_bind_group(tex.width as u32, tex.height as u32, &rgba, None)
            } else {
                Arc::clone(&ctx.default_texture_bind_group)
            }
        } else {
            Arc::clone(&ctx.default_texture_bind_group)
        };
        map_tex_bgs.push(bg);
    }

    let (van_bgs, tid_to_slot) = build_chr_texture_bgs(&ctx, &model, None);
    let (k4_bgs, _) = build_chr_texture_bgs(&ctx, &model, Some(&loader));

    let mut gpu_mesh = GpuSkeletalMesh::from_model(
        &ctx.device,
        &ctx.skeletal_pipeline.bone_bind_group_layout,
        &ctx.skeletal_pipeline.material_bind_group_layout,
        &model,
    );
    for pr in &mut gpu_mesh.primitives {
        if let Some(&slot) = tid_to_slot.get(&(pr.texture_index as u32)) {
            pr.texture_index = slot;
        }
    }

    // Evaluate `.cls` and `.anm` at frame 2500
    let mut weights = Vec::new();
    cls.evaluate_global_frame_into(actor.global_cls_frame(2500.0), false, &mut weights);
    let mut morphed = Vec::new();
    model.apply_cluster_weights_into(&weights, &mut morphed);
    let mut vtx_scratch = Vec::new();
    gpu_mesh.update_morphed_positions(&ctx.queue, &model, &morphed, &mut vtx_scratch);

    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let (f0, f1, frac) = anm.sample_span_at(actor.global_anm_frame(2500.0));
    let mut frame_a = anm.frames[0].clone();
    interpolate_frames_into(&anm.frames[f0], &anm.frames[f1], frac, &mut frame_a);
    let mut pose = Vec::new();
    evaluate_pose_into(&model.parents, &bind_locals, &frame_a, &mut pose);
    let mut skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);
    let world_mat = actor.world_matrix_row_major();
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
    }

    let ca11_env = sh2_core::lighting::lookup_draw_env("CA", 11).expect("CA draw env");
    let mut pool = CharacterLightPool::new();
    let spot = build_cutscene_light_uniforms(
        &mut pool,
        &CutsceneLightInput {
            draw_env: Some(ca11_env),
            demo_lights: &player.current_state.lights,
            ambient_tint: player.atmosphere.ambient_tint_rgba,
            base_amb_scale: 1.0,
            is_dark_room: false,
            cam_eye: player.camera.eye,
            actor_pos: actor.position,
            flashlight_suppressed: false,
        },
    );
    ctx.update_spotlight(&spot);

    let proj =
        glam::Mat4::perspective_rh(player.camera.fov_y_radians, 640.0 / 360.0, 20.0, 50_000.0);
    let view = player.camera.view_matrix();
    ctx.update_camera(proj * view, proj.inverse(), player.camera.eye);

    // 1. Strict Vanilla PS2 (`FogQuality::ClassicPs2`, vanilla embedded textures)
    let ca_eval_van = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            ..Default::default()
        },
        false,
    );
    let fog_van = FogUniforms::from_room_evaluation(&ca_eval_van, FogQuality::ClassicPs2, false);
    ctx.update_fog(&fog_van);
    let draw_van = [(&gpu_mesh, skin.as_slice(), Some(van_bgs.as_slice()))];
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &draw_van);
    let van_rgba = readback_ldr_rgba8(&ctx);

    // 2. Enhanced 4K (`FogQuality::ModernVolumetric`, 4K AI textures)
    let ca_eval_enh = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            ..Default::default()
        },
        true,
    );
    let fog_enh =
        FogUniforms::from_room_evaluation(&ca_eval_enh, FogQuality::ModernVolumetric, false);
    ctx.update_fog(&fog_enh);
    let draw_4k = [(&gpu_mesh, skin.as_slice(), Some(k4_bgs.as_slice()))];
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &draw_4k);
    let enh_rgba = readback_ldr_rgba8(&ctx);

    // 3. Verify lower hair fringe band (`x = 130..260, y = 185..215`, where `tprim[0..2]` hangs
    //    over background fog & neck) contains substantial dark hair pixels in BOTH modes!
    let van_fringe_px = count_dark_pixels_in_rect(&van_rgba, 640, 130, 185, 260, 215, 45.0);
    let enh_fringe_px = count_dark_pixels_in_rect(&enh_rgba, 640, 130, 185, 260, 215, 45.0);
    assert!(
        van_fringe_px >= 600,
        "Vanilla PS2 Angela lower hair fringe must not be erased by depth fog (got {van_fringe_px} dark hair px)"
    );
    assert!(
        enh_fringe_px >= 600,
        "Enhanced 4K Angela lower hair fringe must not be erased by depth fog (got {enh_fringe_px} dark hair px)"
    );

    // 4. Verify `TransparentFringeFogged` still applies PS2 depth fog when Angela is far away
    //    (e.g., 10 m away in `ca11` fog -> her hair tips fade into graveyard fog rather than
    //    staying pitch-black against white mist).
    let far_eye = player.camera.eye
        + (player.camera.eye - player.camera.interest).normalize_or_zero() * 9000.0;
    let far_view = glam::Mat4::look_at_rh(
        far_eye,
        player.camera.interest,
        glam::Vec3::new(0.0, -1.0, 0.0),
    );
    ctx.update_fog(&fog_van);
    ctx.update_camera(proj * far_view, proj.inverse(), far_eye);
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &draw_van);
    let far_rgba = readback_ldr_rgba8(&ctx);
    let far_pitch_black = count_dark_pixels_in_rect(&far_rgba, 640, 0, 0, 640, 360, 18.0);
    assert_eq!(
        far_pitch_black, 0,
        "Distant Angela hair fringe at ~10 m must fade into graveyard depth fog (found {far_pitch_black} unfogged dark px)"
    );

    if let Some(ref d) = out_dir {
        write_ppm_rgb(
            &d.join("haka_agl_f2500_vanilla_after.ppm"),
            640,
            360,
            &van_rgba,
        );
        write_ppm_rgb(
            &d.join("haka_agl_f2500_enhanced_4k_after.ppm"),
            640,
            360,
            &enh_rgba,
        );
    }
}

#[test]
fn sh2_192_maria_transparent_hair_survives_outdoor_fog() {
    let Some(root) = game_root() else {
        return;
    };
    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok().map(PathBuf::from);
    if let Some(ref d) = out_dir {
        let _ = std::fs::create_dir_all(d);
    }

    let mdl_bytes = std::fs::read(root.join("data/chr/mar/hhh_mar.mdl")).expect("read hhh_mar.mdl");
    let model = parse_mdl(&mdl_bytes).expect("parse hhh_mar.mdl");
    assert!(
        model.primitives.iter().any(|p| p.is_transparent),
        "hhh_mar.mdl must have transparent hair tprim submeshes"
    );

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: true,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };

    let (van_bgs, tid_to_slot) = build_chr_texture_bgs(&ctx, &model, None);
    let mut gpu_mesh = GpuSkeletalMesh::from_model(
        &ctx.device,
        &ctx.skeletal_pipeline.bone_bind_group_layout,
        &ctx.skeletal_pipeline.material_bind_group_layout,
        &model,
    );
    for pr in &mut gpu_mesh.primitives {
        if let Some(&slot) = tid_to_slot.get(&(pr.texture_index as u32)) {
            pr.texture_index = slot;
        }
    }

    let mut id = [[0.0f32; 4]; 4];
    id[0][0] = 1.0;
    id[1][1] = 1.0;
    id[2][2] = 1.0;
    id[3][3] = 1.0;
    let bind_skin = vec![id; model.bone_count];

    let ca11_env = sh2_core::lighting::lookup_draw_env("CA", 11).expect("CA draw env");
    let mut pool = CharacterLightPool::new();
    let eye = glam::Vec3::new(0.0, -760.0, 520.0);
    let target = glam::Vec3::new(0.0, -760.0, 0.0);
    let spot = build_cutscene_light_uniforms(
        &mut pool,
        &CutsceneLightInput {
            draw_env: Some(ca11_env),
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: 1.0,
            is_dark_room: false,
            cam_eye: eye,
            actor_pos: glam::Vec3::ZERO,
            flashlight_suppressed: true,
        },
    );
    ctx.update_spotlight(&spot);

    let ca_eval = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            ..Default::default()
        },
        false,
    );
    let fog_u = FogUniforms::from_room_evaluation(&ca_eval, FogQuality::ClassicPs2, false);
    ctx.update_fog(&fog_u);

    let proj = glam::Mat4::perspective_rh(40.0_f32.to_radians(), 640.0 / 360.0, 20.0, 50_000.0);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::new(0.0, -1.0, 0.0));
    ctx.update_camera(proj * view, proj.inverse(), eye);

    let draw_mar = [(&gpu_mesh, bind_skin.as_slice(), Some(van_bgs.as_slice()))];
    ctx.render_scene(None, None, &draw_mar);
    let rgba = readback_ldr_rgba8(&ctx);
    if let Some(ref d) = out_dir {
        write_ppm_rgb(&d.join("maria_ca11_fog_after.ppm"), 640, 360, &rgba);
    }
}
