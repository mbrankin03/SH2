//! Headless 1:1 visual & lighting parity test for the Opening Restroom (`er01.map`) gameplay
//! mirror scene (`PlanarMirrorSpec` + `ER_DrawEnvData` `room_no == 1` + `hhh_jms.mdl`) (`SH2-155`).

use std::path::PathBuf;
use std::sync::Arc;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into, parse_anm,
};
use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::formats::texture::TextureLoader;
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::mirror::PlanarMirrorSpec;
use sh2_renderer::{GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig};

fn game_root() -> Option<PathBuf> {
    let p = PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
    p.exists().then_some(p)
}

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ER01 Parity Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ER01 Parity Readback Encoder"),
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
fn er01_gameplay_mirror_and_room_lighting_match_sh2pc_draw_env() {
    let Some(root) = game_root() else {
        return;
    };

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let map = loader
        .load_vanilla_map("bg/er/er01.map", "er01")
        .expect("load er01.map");

    let mdl_bytes = std::fs::read(root.join("data/chr/jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let mut model = parse_mdl(&mdl_bytes).expect("parse hhh_jms.mdl");
    let anm_bytes =
        std::fs::read(root.join("data/chr/jms/jms_basic1.anm")).expect("read jms_basic1.anm");
    let anm =
        parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse jms_basic1.anm");

    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };
    // Exact `SetupDrawEnvData` (`0x00251370:358-361`) daytime clear color `[0x46, 0x46, 0x46, 0x40]`
    ctx.set_room_clear_color(false, None);

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

    // Pose James at authentic post-prologue `er01` mirror spawn (`-19350, 0, 19800`) facing the mirror (-X)
    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let mut pose = Vec::new();
    evaluate_pose_into(&model.parents, &bind_locals, &anm.frames[0], &mut pose);
    let mut skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);

    let james_pos = glam::Vec3::new(-19350.0, 0.0, 19800.0);
    let world = glam::Mat4::from_translation(james_pos)
        * glam::Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2);
    for m in &mut skin {
        let m4 = world * glam::Mat4::from_cols_array_2d(m).transpose();
        *m = m4.transpose().to_cols_array_2d();
    }

    let er01_env = sh2_core::lighting::lookup_draw_env("ER", 1).expect("er01 draw env");
    let mut pool = CharacterLightPool::new();

    // Activate authentic stenciled planar mirror for `er01` (`room_id == 0x01`)
    let mirror_spec = PlanarMirrorSpec::for_room_and_gpu_map(0x01, &gpu_map);
    assert!(
        mirror_spec.is_some(),
        "er01.map must resolve PlanarMirrorSpec"
    );
    ctx.set_planar_mirror(mirror_spec);

    // Shot 1: Gameplay mirror camera (over James's shoulder looking into the mirror & sinks)
    let cam_eye = glam::Vec3::new(-18100.0, -1450.0, 18900.0);
    let cam_target = glam::Vec3::new(-20000.0, -1250.0, 19850.0);
    let view = glam::Mat4::look_at_rh(cam_eye, cam_target, glam::Vec3::NEG_Y);
    let proj = glam::Mat4::perspective_rh(46.0_f32.to_radians(), 640.0 / 360.0, 25.0, 50_000.0);
    ctx.update_camera(proj * view, proj.inverse(), cam_eye);

    let spot = build_cutscene_light_uniforms(
        &mut pool,
        &CutsceneLightInput {
            draw_env: Some(er01_env),
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: 1.1,
            is_dark_room: false,
            cam_eye,
            actor_pos: james_pos,
            flashlight_suppressed: false,
        },
    );
    ctx.update_spotlight(&spot);
    let skeletal_draw = [(&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice()))];
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
    let mirror_rgba = readback_ldr_rgba8(&ctx);

    // Shot 2: Restroom room overview (inside north wall looking toward stalls & door)
    let ov_eye = glam::Vec3::new(-19400.0, -1450.0, 20300.0);
    let ov_target = glam::Vec3::new(-18200.0, -1100.0, 18200.0);
    let ov_view = glam::Mat4::look_at_rh(ov_eye, ov_target, glam::Vec3::NEG_Y);
    ctx.update_camera(proj * ov_view, proj.inverse(), ov_eye);
    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &skeletal_draw);
    let overview_rgba = readback_ldr_rgba8(&ctx);

    let mean_luma = |rgba: &[u8]| -> f32 {
        let sum: f64 = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| {
                0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2])
            })
            .sum();
        (sum / (rgba.len() / 4) as f64) as f32
    };
    let mirror_luma = mean_luma(&mirror_rgba);
    let overview_luma = mean_luma(&overview_rgba);
    println!("er01 gameplay mean luma: mirror={mirror_luma:.2}, overview={overview_luma:.2}");

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm_rgb(
            &dir.join("er01_gameplay_mirror.ppm"),
            640,
            360,
            &mirror_rgba,
        );
        write_ppm_rgb(
            &dir.join("er01_gameplay_overview.ppm"),
            640,
            360,
            &overview_rgba,
        );
    }

    assert!(
        mirror_luma > 25.0 && mirror_luma < 180.0,
        "er01 mirror scene must be well-lit without blowing out (mean={mirror_luma:.2})"
    );
    assert!(
        overview_luma > 25.0 && overview_luma < 180.0,
        "er01 overview scene must be well-lit without blowing out (mean={overview_luma:.2})"
    );
}
