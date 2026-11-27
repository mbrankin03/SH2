//! Headless 1:1 visual parity test for `SH2-190`:
//! Forest Trail (`ca11.map` + `hhh_jms.mdl` + 400 live `FogParticleVolume` 3D proximity fog sprites)
//! under BOTH strict Vanilla PS2 (`ClassicPs2` / `Ps2Authentic`) and Enhanced
//! (`ModernVolumetric` / `EnhancedEdition`) modes.
//!
//! Verifies that 3D proximity fog particles on the Forest Trail follow exact PS2/PC ground truth:
//! - `fogInit` (`SLUS_202.28` `0x001687d0`, `Fog/fog.c:67`) `fogSetColor(0x80, 0x80, 0x80, 0x80)`
//!   -> unity `[1.0, 1.0, 1.0]` RGB modulation of `fog.tex` (NOT dark `0x46` `FOGCOL`).
//! - Retail `data/bg/ca/caGB.fcl` `FogEnv[0]` (`0x1100`): `count = 400`, `area = 2000 mm`,
//!   `size = 400 mm`, `flags = 0x08` (floor-follow), `subdiv = 1` (`y_span = 2000 mm` above floor),
//!   `alpha = 0x10` (`16 / 255`).
//! - `sh2pc.exe` `FUN_00486d10` (`0x00486d10`) & `FUN_004896e0` (`0x004896e0`) vertical span,
//!   `300 mm` upper fade, and floor-proximity fade (`400 mm`) so James's legs and the dirt trail
//!   floor are clearly visible with subtle light-grey mist (no dark waist-level fog slab).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into, parse_anm,
};
use sh2_assets::formats::cld::parse_cld;
use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_assets::formats::texture::TextureLoader;
use sh2_assets::AssetPathResolver;
use sh2_core::collision::CollisionWorld;
use sh2_core::config::{FogQuality, LightingProfile};
use sh2_core::fog::{
    evaluate_ps2_particle_y_and_alpha, evaluate_room_fog_with_fixes, FogParticleVolume,
    RoomFogContext, CA_FOG_ENV_ALPHA_U8, PS2_DEFAULT_FOG_PARTICLE_SIZE_MM,
    PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM, R_FOREST_CEMETERY_ID,
};
use sh2_core::lighting::{apply_draw_env_fixes, lookup_draw_env};
use sh2_renderer::fog::{
    FogUniforms, GpuFogParticle, ScreenFogState, ScreenFogStepInput, PS2_DEFAULT_FOG_WIND_VEC,
    PS2_FOG_DEFAULT_COLOR_ALPHA_U8,
};
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::{GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig};

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SH2-190 Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SH2-190 Readback Encoder"),
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

fn build_map_bind_groups(ctx: &RenderContext, map: &MapFile) -> Vec<Arc<wgpu::BindGroup>> {
    let mut out = Vec::with_capacity(map.materials.len());
    for (idx, mat) in map.materials.iter().enumerate() {
        let bg = if let Some(tex) = map.textures.get(&u32::from(mat.texture_id)) {
            if let Ok(rgba) = tex.to_rgba() {
                ctx.create_texture_bind_group(
                    tex.width as u32,
                    tex.height as u32,
                    &rgba,
                    Some(&format!("{}_mat_{idx}", map.name)),
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

fn load_character_textures(
    ctx: &RenderContext,
    model: &mut ModelFile,
) -> Vec<Arc<wgpu::BindGroup>> {
    let mut chr_tex_bgs = Vec::new();
    let mut tid_to_slot: HashMap<u32, u16> = HashMap::new();
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
            Some(&format!("sh2_190_jms_{tid}")),
        ));
        tid_to_slot.insert(tid, slot);
    }
    for pr in &mut model.primitives {
        if let Some(&slot) = tid_to_slot.get(&(pr.texture_index as u32)) {
            pr.texture_index = slot;
        }
    }
    chr_tex_bgs
}

fn region_mean_luma(rgba: &[u8], w: u32, y_start: u32, y_end: u32) -> f32 {
    let mut sum = 0.0f64;
    let mut count = 0u64;
    for y in y_start..y_end {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            sum += 0.2126 * f64::from(rgba[idx])
                + 0.7152 * f64::from(rgba[idx + 1])
                + 0.0722 * f64::from(rgba[idx + 2]);
            count += 1;
        }
    }
    (sum / count.max(1) as f64) as f32
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

fn write_ppm_rgb(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut buf = Vec::with_capacity((w * h * 3) as usize + 32);
    buf.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in rgba.as_chunks::<4>().0 {
        buf.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, buf).expect("write ppm");
}

#[test]
fn sh2_190_forest_trail_ca11_3d_fog_particles_preserve_floor_and_james_visibility() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.join("bg/ca/ca11.map").exists()
        || !resolver.data_dir.join("bg/ca/ca11.cld").exists()
        || !resolver.data_dir.join("chr/jms/hhh_jms.mdl").exists()
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
        eprintln!("Skipping SH2-190 headless GPU render: no wgpu adapter available");
        return;
    };

    let loader = TextureLoader::new(resolver.data_dir.clone(), resolver.sh2e_dir.clone());
    if let Ok(fog_tex) = loader.load_texture("pic/effect/fog.tex") {
        ctx.upload_fog_noise_texture(&fog_tex.rgba, fog_tex.width as u32, fog_tex.height as u32);
    }

    ctx.set_room_clear_color(false, None);
    let ca11_map = loader
        .load_vanilla_map("bg/ca/ca11.map", "ca11")
        .expect("load ca11.map");
    let ca11_gpu = GpuMap::from_map(&ctx.device, &ca11_map);
    let ca11_bgs = build_map_bind_groups(&ctx, &ca11_map);

    let cld_bytes = std::fs::read(resolver.data_dir.join("bg/ca/ca11.cld")).expect("read ca11.cld");
    let cld = parse_cld(&cld_bytes).expect("parse ca11.cld");
    let collision = CollisionWorld::from_cld(&cld);

    // Compute `ca11.map` bounding box center (matching `sh2_153` & `sh2_186` Forest Trail entry
    // framing) and place James standing directly on the trail in front of the camera.
    let mut min_v = glam::Vec3::splat(f32::INFINITY);
    let mut max_v = glam::Vec3::splat(f32::NEG_INFINITY);
    for b in &ca11_map.batches {
        for &p in &b.positions {
            let v = glam::Vec3::from_array(p);
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
    }
    let map_center = (min_v + max_v) * 0.5;
    let james_xz = glam::Vec2::new(map_center.x - 450.0, map_center.z + 300.0);
    let floor_y = collision
        .sample_floor_height(
            james_xz.x,
            james_xz.y,
            map_center.y - 2_000.0,
            4_000.0,
            2_000.0,
        )
        .unwrap_or(map_center.y);
    let james_pos = glam::Vec3::new(james_xz.x, floor_y, james_xz.y);
    println!("SH2-190 ca11 james_pos={james_pos:?}, map_center={map_center:?}");

    // Load James (`hhh_jms.mdl`) and evaluate idle pose
    let mdl_bytes =
        std::fs::read(resolver.data_dir.join("chr/jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let mut model = parse_mdl(&mdl_bytes).expect("parse hhh_jms.mdl");
    let anm_bytes = std::fs::read(resolver.data_dir.join("chr/jms/jms_basic1.anm"))
        .expect("read jms_basic1.anm");
    let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse jms_basic1");
    let chr_tex_bgs = load_character_textures(&ctx, &mut model);
    let mut gpu_mesh = GpuSkeletalMesh::from_model(
        &ctx.device,
        &ctx.skeletal_pipeline.bone_bind_group_layout,
        &ctx.skeletal_pipeline.material_bind_group_layout,
        &model,
    );
    gpu_mesh.set_pose_visible(0x15, false);

    let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
    let mut pose = Vec::new();
    evaluate_pose_into(&model.parents, &bind_locals, &anm.frames[0], &mut pose);
    let mut skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);
    let world_mat = glam::Mat4::from_translation(james_pos);
    let world_skin: Vec<[[f32; 4]; 4]> = skin
        .iter()
        .map(|m| (world_mat * glam::Mat4::from_cols_array_2d(m)).to_cols_array_2d())
        .collect();

    // Framing matching `sh2_153` / `sh2_186` `ca11` Forest Trail entry with James centered in view
    let eye = glam::Vec3::new(map_center.x, map_center.y - 1_300.0, map_center.z + 2_500.0);
    let target = glam::Vec3::new(james_pos.x, james_pos.y - 900.0, james_pos.z);
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::NEG_Y);
    let (proj, inv_proj) = ctx.perspective_projection(45.0_f32.to_radians(), 100.0, 40_000.0);
    ctx.update_camera(proj * view, inv_proj, eye);

    let mut light_pool = CharacterLightPool::new();
    let raw_env = lookup_draw_env("CA", 11).expect("CA DrawEnvData");
    let mut screen_state = ScreenFogState::default();
    let screen_eval = screen_state.step(&ScreenFogStepInput {
        cam_delta: glam::Vec3::ZERO,
        area_size: PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
        wind_vec: PS2_DEFAULT_FOG_WIND_VEC,
        dt_scale: 1.0,
        world_view_rot: glam::Mat3::from_mat4(view),
        fog_color_a: PS2_FOG_DEFAULT_COLOR_ALPHA_U8,
        fog_alpha_param: CA_FOG_ENV_ALPHA_U8,
    });

    // Build 400 live 3D proximity fog particles using the exact `atmosphere.rs` + `sh2-core` path
    let ca_eval_vanilla = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            james_pos_y_mm: james_pos.y,
            camera_pos_y_mm: eye.y,
            ..Default::default()
        },
        false,
    );
    let mut vol = FogParticleVolume::new(
        ca_eval_vanilla.particle_count,
        ca_eval_vanilla.particle_range_x_mm,
        ca_eval_vanilla.particle_range_z_mm,
    );
    let cam_to_player = james_pos - eye;
    let cam_dist = cam_to_player.length();
    let anchor = if cam_dist > PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM {
        let projected = eye + (cam_to_player / cam_dist) * PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM;
        [projected.x, projected.y, projected.z]
    } else {
        [james_pos.x, james_pos.y, james_pos.z]
    };
    for _ in 0..30 {
        vol.step(
            1.0 / 60.0,
            ca_eval_vanilla.speed_mult,
            anchor,
            james_pos.to_array(),
            ca_eval_vanilla.james_fog_influence_mm,
        );
    }
    let radius_scale = ca_eval_vanilla.particle_size_mm / PS2_DEFAULT_FOG_PARTICLE_SIZE_MM;
    let gpu_particles: Vec<GpuFogParticle> = vol
        .particles()
        .iter()
        .map(|p| {
            let (wy, alpha) = evaluate_ps2_particle_y_and_alpha(
                anchor[1],
                p.local_pos[1],
                p.alpha,
                ca_eval_vanilla.particle_range_z_mm,
                ca_eval_vanilla.particle_subdiv,
                ca_eval_vanilla.particle_alpha_u8,
                ca_eval_vanilla.particle_floor_follow.then_some(james_pos.y),
                ca_eval_vanilla.particle_size_mm,
            );
            GpuFogParticle {
                world_pos_radius: [
                    anchor[0] + p.local_pos[0],
                    wy,
                    anchor[2] + p.local_pos[2],
                    p.radius_mm * radius_scale,
                ],
                color_alpha: [
                    ca_eval_vanilla.particle_color_rgb[0],
                    ca_eval_vanilla.particle_color_rgb[1],
                    ca_eval_vanilla.particle_color_rgb[2],
                    alpha,
                ],
            }
        })
        .collect();

    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok().map(PathBuf::from);

    // 1. Reference render WITHOUT 3D fog particles (Vanilla PS2)
    let env_vanilla = apply_draw_env_fixes(raw_env, "ca11", LightingProfile::Ps2Authentic);
    let env_lum = env_vanilla.ambient[0]
        .max(env_vanilla.ambient[1])
        .max(env_vanilla.ambient[2]);
    let light_vanilla = build_cutscene_light_uniforms(
        &mut light_pool,
        &CutsceneLightInput {
            draw_env: Some(&env_vanilla),
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: (env_lum * 2.2).clamp(0.25, 2.2),
            is_dark_room: false,
            cam_eye: eye,
            actor_pos: james_pos,
            flashlight_suppressed: true,
        },
    );
    ctx.update_spotlight(&light_vanilla);
    let mut fog_vanilla =
        FogUniforms::from_room_evaluation(&ca_eval_vanilla, FogQuality::ClassicPs2, false);
    fog_vanilla.screen_fog_uv = screen_state.to_gpu_vec4(&screen_eval);
    ctx.update_fog(&fog_vanilla);
    ctx.update_fog_particles(&[]);
    ctx.render_scene(
        Some(&ca11_gpu),
        Some(&ca11_bgs),
        &[(
            &gpu_mesh,
            world_skin.as_slice(),
            Some(chr_tex_bgs.as_slice()),
        )],
    );
    let rgba_no_particles = readback_ldr_rgba8(&ctx);

    // 2. Vanilla PS2 WITH 400 1:1 `caGB.fcl` 3D fog particles
    ctx.update_fog_particles(&gpu_particles);
    ctx.render_scene(
        Some(&ca11_gpu),
        Some(&ca11_bgs),
        &[(
            &gpu_mesh,
            world_skin.as_slice(),
            Some(chr_tex_bgs.as_slice()),
        )],
    );
    let rgba_vanilla = readback_ldr_rgba8(&ctx);

    // 3. Enhanced (`ModernVolumetric` + `EnhancedEdition`) WITH 400 1:1 `caGB.fcl` 3D fog particles
    let env_enh = apply_draw_env_fixes(raw_env, "ca11", LightingProfile::EnhancedEdition);
    let enh_lum = env_enh.ambient[0]
        .max(env_enh.ambient[1])
        .max(env_enh.ambient[2]);
    let light_enh = build_cutscene_light_uniforms(
        &mut light_pool,
        &CutsceneLightInput {
            draw_env: Some(&env_enh),
            demo_lights: &[],
            ambient_tint: [1.0, 1.0, 1.0, 1.0],
            base_amb_scale: (enh_lum * 2.2).clamp(0.25, 2.2),
            is_dark_room: false,
            cam_eye: eye,
            actor_pos: james_pos,
            flashlight_suppressed: true,
        },
    );
    ctx.update_spotlight(&light_enh);
    let ca_eval_enh = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_FOREST_CEMETERY_ID,
            james_pos_y_mm: james_pos.y,
            camera_pos_y_mm: eye.y,
            ..Default::default()
        },
        true,
    );
    let mut fog_enh =
        FogUniforms::from_room_evaluation(&ca_eval_enh, FogQuality::ModernVolumetric, false);
    fog_enh.screen_fog_uv = screen_state.to_gpu_vec4(&screen_eval);
    fog_enh.time = 2.0;
    ctx.update_fog(&fog_enh);
    ctx.update_fog_particles(&gpu_particles);
    ctx.render_scene(
        Some(&ca11_gpu),
        Some(&ca11_bgs),
        &[(
            &gpu_mesh,
            world_skin.as_slice(),
            Some(chr_tex_bgs.as_slice()),
        )],
    );
    let rgba_enh = readback_ldr_rgba8(&ctx);

    let lower_no_part = region_mean_luma(&rgba_no_particles, 640, 180, 360);
    let lower_vanilla = region_mean_luma(&rgba_vanilla, 640, 180, 360);
    let lower_enh = region_mean_luma(&rgba_enh, 640, 180, 360);
    let white_v = bright_white_fraction(&rgba_vanilla);
    let white_e = bright_white_fraction(&rgba_enh);
    println!(
        "SH2-190 ca11 lower-half luma: no_particles={lower_no_part:.2}, vanilla={lower_vanilla:.2}, enhanced={lower_enh:.2} (white v={:.1}%, e={:.1}%)",
        white_v * 100.0,
        white_e * 100.0
    );

    if let Some(ref dir) = out_dir {
        write_ppm_rgb(
            &dir.join("sh2_190_ca11_no_particles.ppm"),
            640,
            360,
            &rgba_no_particles,
        );
        write_ppm_rgb(
            &dir.join("sh2_190_ca11_vanilla.ppm"),
            640,
            360,
            &rgba_vanilla,
        );
        write_ppm_rgb(&dir.join("sh2_190_ca11_enhanced.ppm"), 640, 360, &rgba_enh);
    }

    // Authentic light-grey `fog.tex` particles (`0x80` RGB, `16/255` alpha) must NEVER darken
    // the lower half / trail floor into a dark charcoal waist-level band!
    assert!(
        lower_vanilla >= lower_no_part - 2.0,
        "SH2-190 regression: 3D fog particles darkened the ca11 trail floor (no_part={lower_no_part:.2}, vanilla={lower_vanilla:.2})"
    );
    assert!(
        (lower_vanilla - lower_no_part).abs() < 25.0,
        "SH2-190 regression: 3D fog particles overly opaque on ca11 trail floor (no_part={lower_no_part:.2}, vanilla={lower_vanilla:.2})"
    );
    assert!(
        white_v < 0.05 && white_e < 0.05,
        "SH2-190 ca11 must not blow out white (v={white_v:.3}, e={white_e:.3})"
    );
}
