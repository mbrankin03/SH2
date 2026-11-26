//! Golden-vector unit tests, retail-asset integration tests, and headless visual
//! verification for Stenciled Planar Mirror Rendering (`SH2-101`).

use std::path::PathBuf;
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into, parse_anm,
};
use sh2_assets::formats::map::{parse_map, MapFile};
use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::formats::texture::{decode_texture_bytes, TextureLoader};
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::mirror::{
    oblique_near_plane_projection, reflect_point, reflected_camera_uniforms, reflection_matrix,
    PlanarMirrorAxis, PlanarMirrorSpec, MIRROR_WALL_AP84_Z_MM, MIRROR_WALL_BFW_D5_Z_MM,
    MIRROR_WALL_ER01_X_MM,
};
use sh2_renderer::pipeline::CameraUniforms;
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
        label: Some("Mirror Test Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Mirror Readback Encoder"),
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

fn write_ppm_rgb(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let mut buf = Vec::with_capacity((w * h * 3) as usize + 32);
    buf.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in rgba.as_chunks::<4>().0 {
        buf.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, buf).expect("write ppm");
}

fn build_map_texture_bgs(
    ctx: &RenderContext,
    map: &MapFile,
    label_prefix: &str,
) -> Vec<Arc<wgpu::BindGroup>> {
    let mut out = Vec::with_capacity(map.materials.len());
    for (idx, mat) in map.materials.iter().enumerate() {
        let bg = if let Some(tex) = map.textures.get(&u32::from(mat.texture_id)) {
            if let Ok(rgba) = tex.to_rgba() {
                let label = format!("{label_prefix}_mat_{idx}");
                ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&label),
                )
            } else {
                Arc::clone(&ctx.default_texture_bind_group)
            }
        } else {
            Arc::clone(&ctx.default_texture_bind_group)
        };
        out.push(bg);
    }
    out
}

#[test]
fn golden_reflection_matrix_x_and_z_planes() {
    // 1. Observation Deck Restroom (`er01`, room_id = 0x01): X = -20000.0
    let rx = reflection_matrix(
        Vec3::new(MIRROR_WALL_ER01_X_MM, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
    );
    assert!((rx.determinant() + 1.0).abs() < 1e-5);
    assert!((rx * rx).abs_diff_eq(Mat4::IDENTITY, 1e-4));
    let p_in = Vec3::new(-19_500.0, -800.0, 20_450.0);
    let p_out = rx.transform_point3(p_in);
    assert!(p_out.abs_diff_eq(Vec3::new(-20_500.0, -800.0, 20_450.0), 1e-3));

    // 2. Blue Creek Rm 109 (`ap84`, room_id = 0x24): Z = -99995.0
    let rz = reflection_matrix(
        Vec3::new(0.0, 0.0, MIRROR_WALL_AP84_Z_MM),
        Vec3::new(0.0, 0.0, 1.0),
    );
    assert!((rz.determinant() + 1.0).abs() < 1e-5);
    let p_ap_in = Vec3::new(59_800.0, -900.0, -99_500.0);
    let p_ap_out = rz.transform_point3(p_ap_in);
    assert!(p_ap_out.abs_diff_eq(Vec3::new(59_800.0, -900.0, -100_490.0), 1e-3));

    // 3. Born From a Wish Mansion (`0xd5`): Z = 20750.0
    let spec_d5 = PlanarMirrorSpec::for_room(0xd5).expect("room 0xd5 mirror");
    assert_eq!(spec_d5.axis, PlanarMirrorAxis::AxisZ);
    assert!((spec_d5.wall_pos() - MIRROR_WALL_BFW_D5_Z_MM).abs() < 1e-4);
    let p_d5 = reflect_point(
        Vec3::new(0.0, -500.0, 20_000.0),
        Vec3::from_array(spec_d5.plane_point),
        Vec3::from_array(spec_d5.plane_normal),
    );
    assert!(p_d5.abs_diff_eq(Vec3::new(0.0, -500.0, 21_500.0), 1e-3));
}

#[test]
fn golden_reflected_camera_uniforms_and_clip_plane() {
    let spec = PlanarMirrorSpec::for_room(0x01).expect("er01 mirror");
    let view = Mat4::look_at_rh(
        Vec3::new(-19_000.0, -1200.0, 20_450.0),
        Vec3::new(-20_000.0, -800.0, 20_450.0),
        Vec3::new(0.0, -1.0, 0.0),
    );
    let proj = Mat4::perspective_rh(45.0_f32.to_radians(), 16.0 / 9.0, 50.0, 50_000.0);
    let vp = proj * view;
    let main_cam = CameraUniforms {
        view_proj: vp.to_cols_array_2d(),
        inv_proj: proj.inverse().to_cols_array_2d(),
        cam_pos: [-19_000.0, -1200.0, 20_450.0, 0.0],
        inv_view_proj: vp.inverse().to_cols_array_2d(),
        clip_plane: [0.0; 4],
    };

    let refl = reflected_camera_uniforms(&main_cam, &spec);
    assert!((refl.cam_pos[0] - (-21_000.0)).abs() < 1e-3);
    assert!((refl.cam_pos[1] - (-1200.0)).abs() < 1e-3);
    assert!((refl.cam_pos[2] - 20_450.0).abs() < 1e-3);

    let cp = Vec4::from_array(refl.clip_plane);
    // Point in real room (X = -19500) is kept (>= 0)
    assert!(cp.dot(Vec4::new(-19_500.0, 0.0, 0.0, 1.0)) >= 0.0);
    // Point behind mirror (X = -20500) is clipped (< 0)
    assert!(cp.dot(Vec4::new(-20_500.0, 0.0, 0.0, 1.0)) < 0.0);
}

#[test]
fn golden_oblique_near_plane_projection_clips_mirror_plane() {
    let proj = Mat4::perspective_rh(45.0_f32.to_radians(), 16.0 / 9.0, 50.0, 50_000.0);
    let clip_view = Vec4::new(0.0, 0.0, -1.0, -500.0);
    let ob = oblique_near_plane_projection(proj, clip_view);
    let p_on_plane = ob * Vec4::new(0.0, 0.0, -500.0, 1.0);
    let z_ndc = p_on_plane.z / p_on_plane.w;
    assert!(
        z_ndc.abs() < 1e-4,
        "z_ndc on mirror plane should be ~0, got {z_ndc}"
    );
}

#[test]
fn retail_er01_and_ap84_mirror_decal_batch_extraction() {
    let Some(root) = game_root() else {
        return;
    };

    let er01_bytes = std::fs::read(root.join("data/bg/er/er01.map")).expect("read er01.map");
    let er01 = parse_map(&er01_bytes, "er01").expect("parse er01.map");
    let er01_spec = PlanarMirrorSpec::for_room_and_map(0x01, &er01).expect("er01 spec");
    let er01_idx = er01_spec
        .decal_batch_index
        .expect("er01.map must match its authentic framed mirror decal batch (mat=10)");
    assert_eq!(er01.batches[er01_idx].material_index, 10);
    assert!((er01_spec.wall_pos() - (-20_000.0)).abs() < 1e-3);

    let sh2e_er01_path = root.join("sh2e/bg/er/er01.map");
    if sh2e_er01_path.exists() {
        let sh2e_er01_bytes = std::fs::read(&sh2e_er01_path).expect("read sh2e er01.map");
        let sh2e_er01 = parse_map(&sh2e_er01_bytes, "er01").expect("parse sh2e er01.map");
        let sh2e_spec =
            PlanarMirrorSpec::for_room_and_map(0x01, &sh2e_er01).expect("sh2e er01 spec");
        let sh2e_idx = sh2e_spec
            .decal_batch_index
            .expect("sh2e/bg/er/er01.map must match its authentic framed mirror decal batch");
        assert_eq!(sh2e_er01.batches[sh2e_idx].material_index, 10);
    }

    let ap84_bytes = std::fs::read(root.join("data/bg/ap/ap84.map")).expect("read ap84.map");
    let ap84 = parse_map(&ap84_bytes, "ap84").expect("parse ap84.map");
    let ap84_spec = PlanarMirrorSpec::for_room_and_map(0x24, &ap84).expect("ap84 spec");
    let ap84_idx = ap84_spec
        .decal_batch_index
        .expect("ap84.map must match its authentic mirror decal batch (geo_id=0x0302)");
    assert_eq!(ap84.batches[ap84_idx].geo_id, 0x0302);
    assert!((ap84_spec.wall_pos() - (-99_995.0)).abs() < 1e-3);

    let glas_path = root.join("data/pic/effect/glas03.tbn2");
    let glas_bytes = std::fs::read(&glas_path).expect("read glas03.tbn2");
    let glas = decode_texture_bytes(&glas_bytes, &glas_path).expect("decode glas03.tbn2");
    assert_eq!((glas.width, glas.height), (512, 512));
}

#[test]
fn headless_render_er01_restroom_mirror_with_james_and_flashlight() {
    let Some(root) = game_root() else {
        return;
    };
    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };
    ctx.pcss_enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let map = loader
        .load_vanilla_map("bg/er/er01.map", "er01")
        .expect("load er01.map");
    let gpu_map = GpuMap::from_map(&ctx.device, &map);
    let map_tex_bgs = build_map_texture_bgs(&ctx, &map, "er01");

    let glas_path = root.join("data/pic/effect/glas03.tbn2");
    let glas_bytes = std::fs::read(&glas_path).expect("read glas03.tbn2");
    let glas = decode_texture_bytes(&glas_bytes, &glas_path).expect("decode glas03.tbn2");
    ctx.upload_mirror_glass_texture(glas.width as u32, glas.height as u32, &glas.rgba);

    let mirror_spec = PlanarMirrorSpec::for_room_and_map(0x01, &map).expect("er01 mirror");
    ctx.set_planar_mirror(Some(mirror_spec));

    // Load James (`hhh_jms.mdl` + `jms_stg_toilet.anm`) standing at the sink facing West (-X)
    let mdl_bytes = std::fs::read(root.join("data/chr/jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let mut model = parse_mdl(&mdl_bytes).expect("parse hhh_jms.mdl");
    let anm_bytes = std::fs::read(root.join("data/chr/jms/jms_stg_toilet.anm"))
        .expect("read jms_stg_toilet.anm");
    let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse anm");

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

    let gpu_mesh = GpuSkeletalMesh::from_model(
        &ctx.device,
        &ctx.skeletal_pipeline.bone_bind_group_layout,
        &ctx.skeletal_pipeline.material_bind_group_layout,
        &model,
    );
    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let mut pose = Vec::new();
    evaluate_pose_into(&model.parents, &bind_locals, &anm.frames[0], &mut pose);
    let mut skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);

    // Place James at X = -19606.6, Y = 0.0, Z = 20429.55 facing -X (yaw = -PI/2)
    let yaw = -std::f32::consts::FRAC_PI_2;
    let (sy, cy) = yaw.sin_cos();
    let world_mat = [
        [cy, 0.0, sy, -19_606.6],
        [0.0, 1.0, 0.0, 0.0],
        [-sy, 0.0, cy, 20_429.55],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for m in &mut skin {
        let orig = *m;
        for r in 0..4 {
            for c in 0..4 {
                m[r][c] = world_mat[r][0] * orig[0][c]
                    + world_mat[r][1] * orig[1][c]
                    + world_mat[r][2] * orig[2][c]
                    + world_mat[r][3] * orig[3][c];
            }
        }
    }
    gpu_mesh.update_bone_matrices(&ctx.queue, &skin);

    // Camera over James's right shoulder looking into the mirror at X = -20000.0
    let eye = Vec3::new(-18_950.0, -1350.0, 20_650.0);
    let target = Vec3::new(-20_000.0, -1100.0, 20_450.0);
    let view = Mat4::look_at_rh(eye, target, Vec3::new(0.0, -1.0, 0.0));
    let proj = Mat4::perspective_rh(45.0_f32.to_radians(), 16.0 / 9.0, 20.0, 50_000.0);
    ctx.update_camera(proj * view, proj.inverse(), eye);

    // Live flashlight on James's chest pointing West (-X) toward the mirror + room fill!
    let mut spot = SpotlightUniforms {
        position: [-19_620.0, -1150.0, 20_430.0, 1.0],
        direction_inner: [-1.0, 0.05, 0.0, 0.90],
        color_outer: [2.2, 2.1, 1.9, 0.65],
        ambient_tint: [1.0, 1.0, 1.0, 2.5],
        fill_and_range: [0.35, 0.0, 0.0, 0.0],
        ps2_falloff_params: [-1.0 / 6000.0, 1.2, 0.0, 0.0],
        ..Default::default()
    };
    spot.point_positions[0] = [-19_000.0, -1300.0, 20_400.0, 500.0];
    spot.point_colors[0] = [0.85, 0.85, 0.80, 6000.0];
    ctx.update_spotlight(&spot);

    ctx.render_scene(
        Some(&gpu_map),
        Some(&map_tex_bgs),
        &[(&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice()))],
    );

    let rgba = readback_ldr_rgba8(&ctx);
    if let Ok(out_path) = std::env::var("SH2_MIRROR_ER01_PPM") {
        write_ppm_rgb(&out_path, 640, 360, &rgba);
    }
    let avg_luma: f32 = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .sum::<f32>()
        / (640.0 * 360.0);
    assert!(
        avg_luma > 2.0,
        "er01 mirror render should be non-black (avg_luma={avg_luma:.2})"
    );
}

#[test]
fn headless_render_ap84_blue_creek_109_mirror() {
    let Some(root) = game_root() else {
        return;
    };
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };
    ctx.pcss_enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let map = loader
        .load_vanilla_map("bg/ap/ap84.map", "ap84")
        .expect("load ap84.map");
    let gpu_map = GpuMap::from_map(&ctx.device, &map);
    let map_tex_bgs = build_map_texture_bgs(&ctx, &map, "ap84");

    let mirror_spec = PlanarMirrorSpec::for_room_and_map(0x24, &map).expect("ap84 mirror");
    ctx.set_planar_mirror(Some(mirror_spec));

    // Camera inside Blue Creek Rm 109 looking toward the wall-to-wall mirror at Z = -99995.0
    let eye = Vec3::new(60_000.0, -900.0, -98_800.0);
    let target = Vec3::new(59_900.0, -650.0, -99_995.0);
    let view = Mat4::look_at_rh(eye, target, Vec3::new(0.0, -1.0, 0.0));
    let proj = Mat4::perspective_rh(45.0_f32.to_radians(), 16.0 / 9.0, 20.0, 50_000.0);
    ctx.update_camera(proj * view, proj.inverse(), eye);

    let mut spot = SpotlightUniforms {
        position: [60_000.0, -900.0, -99_000.0, 1.0],
        direction_inner: [0.0, 0.1, -1.0, 0.88],
        color_outer: [2.0, 1.9, 1.7, 0.60],
        ambient_tint: [1.0, 1.0, 1.0, 3.0],
        fill_and_range: [0.35, 0.0, 0.0, 0.0],
        ps2_falloff_params: [-1.0 / 6000.0, 1.2, 0.0, 0.0],
        ..Default::default()
    };
    spot.point_positions[0] = [59_900.0, -900.0, -99_000.0, 500.0];
    spot.point_colors[0] = [0.9, 0.85, 0.8, 6000.0];
    ctx.update_spotlight(&spot);

    ctx.render_scene(Some(&gpu_map), Some(&map_tex_bgs), &[]);

    let rgba = readback_ldr_rgba8(&ctx);
    if let Ok(out_path) = std::env::var("SH2_MIRROR_AP84_PPM") {
        write_ppm_rgb(&out_path, 640, 360, &rgba);
    }
    let avg_luma: f32 = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .sum::<f32>()
        / (640.0 * 360.0);
    assert!(
        avg_luma > 2.0,
        "ap84 mirror render should be non-black (avg_luma={avg_luma:.2})"
    );
}

#[test]
fn headless_render_er01_sh2ee_camera_has_intact_concrete_wall_and_zero_clear_seam() {
    let Some(root) = game_root() else {
        return;
    };
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 960,
        height: 540,
        enable_fog: false,
        enable_post: true,
        ..Default::default()
    }) else {
        return;
    };
    ctx.pcss_enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    // Set bright cyan clear color (`[0, 255, 255, 255]`) as an adversarial seam detector:
    // if the recessed concrete upper wall (`X = -20064.1`) or tile ledge back were clipped,
    // cyan pixels would bleed through along the wall/tile junction.
    ctx.set_room_clear_color(false, Some([0, 255, 255, 255]));

    let loader = TextureLoader::new(root.join("data"), Some(root.join("sh2e")));
    let map = loader
        .load_map_with_hd_overrides("bg/er/er01.map", "er01")
        .expect("load sh2e er01.map");
    let gpu_map = GpuMap::from_map(&ctx.device, &map);
    let map_tex_bgs = build_map_texture_bgs(&ctx, &map, "er01");

    let mirror_spec = PlanarMirrorSpec::for_room_and_gpu_map(0x01, &gpu_map)
        .expect("er01 mirror spec from GpuMap");
    assert!(
        mirror_spec.decal_batch_index.is_some(),
        "er01 GpuMap must resolve authentic mat=10 mirror decal batch"
    );
    ctx.set_planar_mirror(Some(mirror_spec));

    // Load James (`hhh_jms.mdl` + `jms_stg_toilet.anm`) at authentic er01 spawn
    let mdl_bytes = std::fs::read(root.join("data/chr/jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let mut model = parse_mdl(&mdl_bytes).expect("parse hhh_jms.mdl");
    let anm_bytes = std::fs::read(root.join("data/chr/jms/jms_stg_toilet.anm"))
        .expect("read jms_stg_toilet.anm");
    let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse anm");

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
    let gpu_mesh = GpuSkeletalMesh::from_model(
        &ctx.device,
        &ctx.skeletal_pipeline.bone_bind_group_layout,
        &ctx.skeletal_pipeline.material_bind_group_layout,
        &model,
    );
    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let mut pose = Vec::new();
    evaluate_pose_into(&model.parents, &bind_locals, &anm.frames[0], &mut pose);
    let mut skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);

    let yaw = -std::f32::consts::FRAC_PI_2;
    let (sy, cy) = yaw.sin_cos();
    let world_mat = [
        [cy, 0.0, sy, -19_475.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sy, 0.0, cy, 20_429.6],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for m in &mut skin {
        let orig = *m;
        for r in 0..4 {
            for c in 0..4 {
                m[r][c] = world_mat[r][0] * orig[0][c]
                    + world_mat[r][1] * orig[1][c]
                    + world_mat[r][2] * orig[2][c]
                    + world_mat[r][3] * orig[3][c];
            }
        }
    }
    gpu_mesh.update_bone_matrices(&ctx.queue, &skin);

    // Exact `er01.cam` Record #2 camera matching `er_04.png` / opening restroom view
    let eye = Vec3::new(-18_684.0, -1003.0, 20_361.0);
    let target = Vec3::new(-20_036.3, -342.7, 20_513.5);
    let view = Mat4::look_at_rh(eye, target, Vec3::new(0.0, -1.0, 0.0));
    let proj = Mat4::perspective_rh(33.0_f32.to_radians(), 16.0 / 9.0, 25.0, 50_000.0);
    ctx.update_camera(proj * view, proj.inverse(), eye);

    // Authentic `er01` `DrawEnvData` lighting (`SLUS_202.28` `ER_DrawEnvData`)
    let er01_env = sh2_core::lighting::lookup_draw_env("ER", 1).expect("ER room 1 DrawEnv");
    let mut pool = sh2_renderer::light::CharacterLightPool::new();
    let spot = sh2_renderer::light::build_cutscene_light_uniforms(
        &mut pool,
        &sh2_renderer::light::CutsceneLightInput {
            draw_env: Some(er01_env),
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: (er01_env.ambient[0]
                .max(er01_env.ambient[1])
                .max(er01_env.ambient[2])
                * 2.2)
                .clamp(0.25, 2.2),
            is_dark_room: false,
            cam_eye: eye,
            actor_pos: Vec3::new(-19_475.0, 0.0, 20_429.6),
            flashlight_suppressed: false,
        },
    );
    ctx.update_spotlight(&spot);

    ctx.render_scene(
        Some(&gpu_map),
        Some(&map_tex_bgs),
        &[(&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice()))],
    );

    let rgba = readback_ldr_rgba8(&ctx);
    if let Ok(out_path) = std::env::var("SH2_MIRROR_ER01_SH2EE_PPM") {
        write_ppm_rgb(&out_path, 960, 540, &rgba);
    }

    // Count cyan clear-color leak pixels (`G > 200 && B > 200 && R < 40`)
    let cyan_leak_pixels = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] < 40 && p[1] > 200 && p[2] > 200)
        .count();
    assert_eq!(
        cyan_leak_pixels, 0,
        "er01 restroom wall/tile junction must have zero clear-color leak pixels"
    );
}
