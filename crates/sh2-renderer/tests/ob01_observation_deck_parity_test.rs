//! Headless 1:1 visual, lighting, and fog parity test for the Toluca Lake Observation Deck
//! (`ob01.map`) gameplay shots and `tenbou.dds` opening monologue cutscene (`SH2-156`).
//!
//! Ground truth:
//! - `OB_DrawEnvData`: `SLUS_202.28` `0x00362e70` / `sh2pc.exe` `0x3f5530`
//!   (`fog_near = 19,000.0` mm, `fog_far = 33,000.0` mm, `ambient = [0.9, 0.9, 0.9]`,
//!   `shadow_darkness = 0` drop-shadow mode, 3 daytime parallel lights).
//! - `SetupDrawEnvData`: `SLUS_202.28` `GFW/sh2_DrawEnvData.c` `0x00251370:358-361`
//!   (daytime GS `clearcolor` and `FOGCOL` = `[0x46, 0x46, 0x46, 0x40]`).
//! - `obGB.fcl` (`data/bg/ob/obGB.fcl` & `sh2e/bg/ob/obGB.fcl`):
//!   `FogEnv.y (+0x08)` at file offset `0x8c` = `+10,000` mm (`0x2710`) valley mist altitude
//!   (`fogSetLocalPosV` `SLUS_202.28` `0x0016c7c8..0x0016c7e4`).
//! - `data/demo/tenbou/tenbou.dds` + `hhh_jms.anm` + `hhh_jms.cls`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into,
    interpolate_frames_into, parse_anm,
};
use sh2_assets::formats::cld::parse_cld;
use sh2_assets::formats::cls::parse_cls;
use sh2_assets::formats::demo::parse_demo_dds;
use sh2_assets::formats::kg::parse_kg1;
use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::formats::texture::TextureLoader;
use sh2_core::collision::CollisionWorld;
use sh2_core::config::FogQuality;
use sh2_core::fog::{
    evaluate_room_fog_with_fixes, FogParticleVolume, RoomFogContext, OB_FOG_ENV_FIXED_Y_MM,
    OB_FOG_MAX_NORM, OB_FOG_MIN_NORM, PS2_DAYTIME_DEFAULT_FOG_COLOR_F32, R_OBSV_DECK_ID,
};
use sh2_core::lighting::lookup_draw_env;
use sh2_core::{find_drama_demo_by_id, DramaDemoPlayer};
use sh2_renderer::fog::{FogUniforms, GpuFogParticle};
use sh2_renderer::light::{build_cutscene_light_uniforms, CharacterLightPool, CutsceneLightInput};
use sh2_renderer::shadow::flatten_kg1_drop_shadow;
use sh2_renderer::{
    prepare_map_texture_cpu, GpuMap, GpuSkeletalMesh, RenderContext, RendererConfig,
};

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
    let mut candidates: Vec<PathBuf> = std::env::var_os("SH2_4K_TEXTURES_DIR")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    candidates.extend([
        PathBuf::from("mods/sh2_4k_textures"),
        PathBuf::from("../../mods/sh2_4k_textures"),
        PathBuf::from("../../../../Documents/SH2-Port/mods/sh2_4k_textures"),
    ]);
    candidates
        .into_iter()
        .find(|p| p.join("sh2_4k_textures.zip").is_file() || p.join("map").is_dir())
}

fn readback_ldr_rgba8(ctx: &RenderContext) -> Vec<u8> {
    let w = ctx.config.width;
    let h = ctx.config.height;
    let bytes_per_row = (w * 4).div_ceil(256) * 256;
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("OB01 Parity Readback"),
        size: u64::from(bytes_per_row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("OB01 Parity Readback Encoder"),
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

fn write_ppm_rgb(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let mut buf = Vec::with_capacity((w * h * 3) as usize + 32);
    buf.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for px in rgba.as_chunks::<4>().0 {
        buf.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, buf).expect("write ppm");
}

fn mean_luma(rgba: &[u8]) -> f32 {
    let sum: f64 = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| 0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
        .sum();
    (sum / (rgba.len() / 4) as f64) as f32
}

fn build_map_bind_groups(ctx: &RenderContext, map: &MapFile) -> Vec<Arc<wgpu::BindGroup>> {
    let mut cache: HashMap<u32, Arc<wgpu::BindGroup>> = HashMap::new();
    for mat in &map.materials {
        let tid = u32::from(mat.texture_id);
        if tid != 0 && !cache.contains_key(&tid) {
            if let Some(tex) = map.textures.get(&tid) {
                if let Ok(rgba) = tex.to_rgba() {
                    let (wrap_u, wrap_v) = map.texture_uv_wrap_axes(tid);
                    let prepared = prepare_map_texture_cpu(
                        tex.width as u32,
                        tex.height as u32,
                        &rgba,
                        ctx.supports_bc_compression(),
                    );
                    let bg = ctx.upload_prepared_texture_bind_group_with_wrap(
                        &prepared,
                        wrap_u,
                        wrap_v,
                        Some(&format!("ob01_tid_{tid}")),
                    );
                    cache.insert(tid, bg);
                }
            }
        }
    }

    map.materials
        .iter()
        .map(|mat| {
            cache
                .get(&u32::from(mat.texture_id))
                .cloned()
                .unwrap_or_else(|| Arc::clone(&ctx.default_texture_bind_group))
        })
        .collect()
}

fn load_character_textures(
    ctx: &RenderContext,
    model: &mut sh2_assets::formats::mdls::ModelFile,
) -> Vec<Arc<wgpu::BindGroup>> {
    let mut chr_tex_bgs = Vec::new();
    let mut tid_to_slot = HashMap::new();
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
    chr_tex_bgs
}

fn configure_ob01_fog_and_valley_mist(
    ctx: &RenderContext,
    player_pos: glam::Vec3,
    cam_eye: glam::Vec3,
    enable_sh2ee_world_fixes: bool,
) -> FogUniforms {
    let eval = evaluate_room_fog_with_fixes(
        &RoomFogContext {
            room_id: R_OBSV_DECK_ID,
            james_pos_y_mm: player_pos.y,
            camera_pos_y_mm: cam_eye.y,
            flashlight_on: false,
            extended_observation_deck_range: enable_sh2ee_world_fixes,
        },
        enable_sh2ee_world_fixes,
    );
    assert_eq!(eval.fog_near_mm, 19_000.0);
    assert_eq!(eval.fog_far_mm, 33_000.0);
    assert!((eval.fog_min - OB_FOG_MIN_NORM).abs() < 1e-6);
    assert!((eval.fog_max - OB_FOG_MAX_NORM).abs() < 1e-6);
    assert_eq!(eval.particle_fixed_y_mm, Some(OB_FOG_ENV_FIXED_Y_MM));
    assert_eq!(eval.fog_color[0], PS2_DAYTIME_DEFAULT_FOG_COLOR_F32);

    let mut fog_u = FogUniforms::from_room_evaluation(&eval, FogQuality::ClassicPs2, false);
    assert!((fog_u.depth_fog_params[2] - OB_FOG_MIN_NORM).abs() < 1e-6);
    assert!((fog_u.depth_fog_params[3] - OB_FOG_MAX_NORM).abs() < 1e-6);
    fog_u.time = 1.5;
    // Active `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`) drifting mist overlay over lake/hills
    fog_u.screen_fog_uv = [0.5, 0.5, 0.125, 32.0 / 128.0];
    ctx.update_fog(&fog_u);

    // Exact 1:1 `SLUS_202.28` `Fog/fogdata.c:166-171` (`0x0016cb40`, `DAT_00581534 == 0x05`)
    // and `sh2pc.exe` `FUN_00486d10` (`0x00486d10`, `subdiv = 4`, `area = 7000 mm`).
    let mut vol = FogParticleVolume::new(
        eval.particle_count,
        eval.particle_range_x_mm,
        eval.particle_range_z_mm,
    );
    let anchor = [
        player_pos.x,
        sh2_core::fog::OB_FOG_ANCHOR_Y_MM,
        sh2_core::fog::OB_FOG_ANCHOR_Z_MM,
    ];
    vol.step(
        1.0 / 30.0,
        eval.speed_mult,
        anchor,
        player_pos.to_array(),
        eval.james_fog_influence_mm,
    );
    let fog_rgb = sh2_core::fog::PS2_FOG_PARTICLE_MIST_RGB;
    let radius_scale = sh2_core::fog::OB_FOG_ENV_PARTICLE_SIZE_MM / 300.0;
    let gpu_particles: Vec<GpuFogParticle> = vol
        .particles()
        .iter()
        .map(|p| {
            let (wy, alpha) = sh2_core::fog::evaluate_ob01_particle_y_and_alpha(
                anchor[1],
                p.local_pos[1],
                p.alpha,
            );
            GpuFogParticle {
                world_pos_radius: [
                    anchor[0] + p.local_pos[0],
                    wy,
                    anchor[2] + p.local_pos[2],
                    p.radius_mm * radius_scale,
                ],
                color_alpha: [fog_rgb[0], fog_rgb[1], fog_rgb[2], alpha],
            }
        })
        .collect();
    ctx.update_fog_particles(&gpu_particles);
    fog_u
}

struct GameplayShot {
    name: &'static str,
    cam_eye: glam::Vec3,
    cam_target: glam::Vec3,
    james_pos: glam::Vec3,
    james_yaw: f32,
}

#[test]
fn ob01_gameplay_and_tenbou_cutscene_visual_lighting_fog_parity() {
    let Some(root) = game_root() else {
        eprintln!("Skipping SH2-156 ob01 parity test: retail rip not present");
        return;
    };

    // Verify `OB_DrawEnvData` ground truth (`SLUS_202.28` `0x00362e70` / `sh2pc.exe` `0x3f5530`)
    let ob_env = lookup_draw_env("OB", 0).expect("OB_DrawEnvData");
    assert_eq!(ob_env.fog_near, 19_000.0);
    assert_eq!(ob_env.fog_far, 33_000.0);
    assert!((ob_env.ambient[0] - 0.9).abs() < 1e-4);
    assert!(
        ob_env.uses_drop_shadow_mode(),
        "OB_DrawEnvData shadow_darkness (+0x09) == 0 uses floor drop-shadow mode"
    );

    // Verify `obGB.fcl` `+10,000 mm` (`0x2710`) valley mist altitude in both retail `data/` and `sh2e/`
    for rel in ["data/bg/ob/obGB.fcl", "sh2e/bg/ob/obGB.fcl"] {
        let fcl = std::fs::read(root.join(rel)).expect("read obGB.fcl");
        assert_eq!(fcl.len(), 156);
        let y_fixed = i32::from_le_bytes(fcl[0x8c..0x90].try_into().unwrap());
        assert_eq!(y_fixed, 10_000, "{rel} +0x8c must be +10,000 mm");
    }

    let loader =
        TextureLoader::with_4k_dir(root.join("data"), Some(root.join("sh2e")), mods_4k_dir());
    let vanilla_map = loader
        .load_vanilla_map("bg/ob/ob01.map", "ob01")
        .expect("load data/bg/ob/ob01.map");
    let mut enhanced_map = loader
        .load_sh2e_map("bg/ob/ob01.map", "ob01")
        .expect("load sh2e/bg/ob/ob01.map");
    let has_4k = mods_4k_dir().is_some();
    if has_4k {
        loader.apply_4k_map_texture_overrides(&mut enhanced_map);
    }

    let mdl_bytes = std::fs::read(root.join("data/chr/jms/hhh_jms.mdl")).expect("read hhh_jms.mdl");
    let mut model = parse_mdl(&mdl_bytes).expect("parse hhh_jms.mdl");
    let cld_bytes = std::fs::read(root.join("data/bg/ob/ob01.cld")).expect("read ob01.cld");
    let cld = parse_cld(&cld_bytes).expect("parse ob01.cld");
    let collision = CollisionWorld::from_cld(&cld);
    let sample_floor = |x: f32, z: f32| -> f32 {
        collision
            .sample_floor_height(x, z, -500.0, 2000.0, 2000.0)
            .unwrap_or(0.0)
    };
    let basic_anm_bytes =
        std::fs::read(root.join("data/chr/jms/jms_basic1.anm")).expect("read jms_basic1.anm");
    let basic_anm =
        parse_anm(&basic_anm_bytes, model.bone_count, &model.parents).expect("parse jms_basic1");
    let kg1_bytes = std::fs::read(root.join("data/chr/jms/hhh_jms.kg1")).ok();
    let jms_kg1 = kg1_bytes.as_deref().and_then(|b| parse_kg1(b).ok());

    let tenbou_dds_bytes =
        std::fs::read(root.join("data/demo/tenbou/tenbou.dds")).expect("read tenbou.dds");
    let tenbou_dds = parse_demo_dds(&tenbou_dds_bytes).expect("parse tenbou.dds");
    let tenbou_anm_bytes =
        std::fs::read(root.join("data/demo/tenbou/hhh_jms.anm")).expect("read tenbou hhh_jms.anm");
    let tenbou_anm = parse_anm(&tenbou_anm_bytes, model.bone_count, &model.parents)
        .expect("parse tenbou hhh_jms.anm");
    let tenbou_cls_bytes =
        std::fs::read(root.join("data/demo/tenbou/hhh_jms.cls")).expect("read tenbou hhh_jms.cls");
    let tenbou_cls = parse_cls(&tenbou_cls_bytes).expect("parse tenbou hhh_jms.cls");

    let Some(mut ctx) = RenderContext::new_headless(RendererConfig {
        width: 640,
        height: 360,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-156 headless GPU render: no wgpu adapter available");
        return;
    };

    // Upload retail fog noise texture if available
    if let Ok(fog_tex) = loader.load_texture("pic/effect/fog.tex") {
        ctx.upload_fog_noise_texture(&fog_tex.rgba, fog_tex.width as u32, fog_tex.height as u32);
    }

    // Exact `SetupDrawEnvData` (`0x00251370:358-361`) daytime clear color `[0x46, 0x46, 0x46, 0x40]`
    ctx.set_room_clear_color(false, None);

    let vanilla_gpu = GpuMap::from_map(&ctx.device, &vanilla_map);
    let vanilla_bgs = build_map_bind_groups(&ctx, &vanilla_map);
    let enhanced_gpu = GpuMap::from_map(&ctx.device, &enhanced_map);
    let enhanced_bgs = build_map_bind_groups(&ctx, &enhanced_map);

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
    evaluate_pose_into(
        &model.parents,
        &bind_locals,
        &basic_anm.frames[0],
        &mut pose,
    );
    let mut base_skin = Vec::new();
    compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut base_skin);

    // Find James's car center from SH2:EE car-interior batch (`material_index == 28`)
    let mut car_min = glam::Vec3::splat(f32::INFINITY);
    let mut car_max = glam::Vec3::splat(f32::NEG_INFINITY);
    for b in enhanced_map
        .batches
        .iter()
        .filter(|b| b.material_index == 28)
    {
        for &p in &b.positions {
            let v = glam::Vec3::from_array(p);
            car_min = car_min.min(v);
            car_max = car_max.max(v);
        }
    }
    let car_center = (car_min + car_max) * 0.5;
    println!(
        "ob01 car_center={car_center:?}, tenbou frames={}, dds_lights={}",
        tenbou_dds.header.total_frames,
        tenbou_dds.header.total_lights()
    );

    let gameplay_shots = [
        // 1. Toluca Lake Overlook: behind James at the stone parapet looking out over the lake, hills, & +10,000 mm mist
        GameplayShot {
            name: "overlook",
            cam_eye: glam::Vec3::new(17800.0, -1800.0, -4200.0),
            cam_target: glam::Vec3::new(19000.0, -200.0, 6000.0),
            james_pos: glam::Vec3::new(18900.0, sample_floor(18900.0, -1350.0), -1350.0),
            james_yaw: 0.0,
        },
        // 2. Parking lot & James's blue sedan framed against Toluca Lake & hills
        GameplayShot {
            name: "parking_car",
            cam_eye: car_center + glam::Vec3::new(2200.0, -1500.0, -4400.0),
            cam_target: car_center + glam::Vec3::new(-400.0, -200.0, 800.0),
            james_pos: glam::Vec3::new(
                car_center.x + 1200.0,
                sample_floor(car_center.x + 1200.0, car_center.z - 900.0),
                car_center.z - 900.0,
            ),
            james_yaw: -0.7,
        },
        // 3. Restroom exterior & observation deck walkway (`tenbou` exit area)
        GameplayShot {
            name: "restroom_ext",
            cam_eye: glam::Vec3::new(17600.0, -1650.0, -7800.0),
            cam_target: glam::Vec3::new(20600.0, -1100.0, -3800.0),
            james_pos: glam::Vec3::new(19700.0, sample_floor(19700.0, -5200.0), -5200.0),
            james_yaw: -2.4,
        },
        // 4. Forest stairs next to the white van & green dumpster (west end of deck)
        GameplayShot {
            name: "forest_stairs",
            cam_eye: glam::Vec3::new(1500.0, -1650.0, -5200.0),
            cam_target: glam::Vec3::new(1600.0, -400.0, 2000.0),
            james_pos: glam::Vec3::new(2200.0, sample_floor(2200.0, -2200.0), -2200.0),
            james_yaw: 0.0,
        },
        // 5. Classic parking-lot overlook behind the utility pole & power line looking toward Toluca Lake
        GameplayShot {
            name: "pole_overlook",
            cam_eye: glam::Vec3::new(18600.0, -2800.0, -12500.0),
            cam_target: glam::Vec3::new(19500.0, -1500.0, -2500.0),
            james_pos: glam::Vec3::new(18900.0, sample_floor(18900.0, -1350.0), -1350.0),
            james_yaw: 0.0,
        },
    ];

    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok().map(PathBuf::from);
    if let Some(ref dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let proj = glam::Mat4::perspective_rh(46.0_f32.to_radians(), 640.0 / 360.0, 50.0, 120_000.0);
    let mut pool = CharacterLightPool::new();
    let tiers: [(&str, &GpuMap, &[Arc<wgpu::BindGroup>], bool); 2] = [
        ("vanilla", &vanilla_gpu, &vanilla_bgs, false),
        (
            if has_4k { "sh2e_4k" } else { "sh2e" },
            &enhanced_gpu,
            &enhanced_bgs,
            true,
        ),
    ];

    // 1. Render all 4 gameplay shots in both Vanilla PS2 and SH2:EE / 4K tiers
    for shot in &gameplay_shots {
        let world = glam::Mat4::from_translation(shot.james_pos)
            * glam::Mat4::from_rotation_y(shot.james_yaw);
        let mut skin = base_skin.clone();
        for m in &mut skin {
            let m4 = world * glam::Mat4::from_cols_array_2d(m).transpose();
            *m = m4.transpose().to_cols_array_2d();
        }
        // Authentic `OB_DrawEnvData` floor drop shadow (`shadow_darkness == 0`):
        // transpose row-major shader skin matrices into column-major `Mat4` for `flatten_kg1_drop_shadow`.
        if let Some(ref kg1) = jms_kg1 {
            let col_major_skin: Vec<[[f32; 4]; 4]> = skin
                .iter()
                .map(|m| {
                    glam::Mat4::from_cols_array_2d(m)
                        .transpose()
                        .to_cols_array_2d()
                })
                .collect();
            let drop_verts = flatten_kg1_drop_shadow(
                kg1,
                &col_major_skin,
                shot.james_pos.y,
                glam::Vec3::from_array(ob_env.directional_lights[0].direction),
            );
            ctx.update_shadow_volumes(&[], &drop_verts, 0x50 as f32 / 255.0);
        }

        let view = glam::Mat4::look_at_rh(shot.cam_eye, shot.cam_target, glam::Vec3::NEG_Y);
        ctx.update_camera(proj * view, proj.inverse(), shot.cam_eye);

        let spot = build_cutscene_light_uniforms(
            &mut pool,
            &CutsceneLightInput {
                draw_env: Some(ob_env),
                demo_lights: &[],
                ambient_tint: [1.0, 1.0, 1.0, 1.0],
                base_amb_scale: (0.9_f32 * 2.2).clamp(0.25, 2.2),
                is_dark_room: false,
                cam_eye: shot.cam_eye,
                actor_pos: shot.james_pos,
                flashlight_suppressed: false,
            },
        );
        ctx.update_spotlight(&spot);
        let skeletal_draw = [(&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice()))];

        for &(tier_name, gpu_map, map_bgs, enable_fixes) in &tiers {
            configure_ob01_fog_and_valley_mist(&ctx, shot.james_pos, shot.cam_eye, enable_fixes);
            ctx.render_scene(Some(gpu_map), Some(map_bgs), &skeletal_draw);
            let rgba = readback_ldr_rgba8(&ctx);
            let luma = mean_luma(&rgba);
            println!(
                "ob01 gameplay [{tier_name}] {}: mean_luma={luma:.2}",
                shot.name
            );
            assert!(
                luma > 20.0 && luma < 220.0,
                "ob01 {tier_name} {} mean luma out of range: {luma:.2}",
                shot.name
            );
            if let Some(ref dir) = out_dir {
                write_ppm_rgb(
                    &dir.join(format!("ob01_{}_{tier_name}.ppm", shot.name)),
                    640,
                    360,
                    &rgba,
                );
            }
        }
    }

    // 2. Render `tenbou.dds` cutscene keyframes in both Vanilla PS2 and SH2:EE / 4K tiers
    let info = find_drama_demo_by_id("tenbou").expect("tenbou in catalog");
    let mut player = DramaDemoPlayer::new(*info, tenbou_dds, false);
    player.set_viewport_dimensions(640.0, 360.0, player.widescreen_mode);

    let keyframes: [(&str, f32); 3] = [("f060", 60.0), ("f500", 500.0), ("f1200", 1200.0)];
    let mut weights = Vec::new();
    let mut morphed = Vec::new();
    let mut vtx_scratch = Vec::new();
    let mut frame_interp = tenbou_anm.frames[0].clone();

    for &(label, frame) in &keyframes {
        player.seek_to_frame(frame, false);
        let actor = &player.actors[0];
        tenbou_cls.evaluate_global_frame_into(actor.global_cls_frame(frame), false, &mut weights);
        model.apply_cluster_weights_into(&weights, &mut morphed);
        gpu_mesh.update_morphed_positions(&ctx.queue, &model, &morphed, &mut vtx_scratch);

        let (f0, f1, frac) = tenbou_anm.sample_span_at(actor.global_anm_frame(frame));
        interpolate_frames_into(
            &tenbou_anm.frames[f0],
            &tenbou_anm.frames[f1],
            frac,
            &mut frame_interp,
        );
        evaluate_pose_into(&model.parents, &bind_locals, &frame_interp, &mut pose);
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
        if let Some(ref kg1) = jms_kg1 {
            let col_major_skin: Vec<[[f32; 4]; 4]> = skin
                .iter()
                .map(|m| {
                    glam::Mat4::from_cols_array_2d(m)
                        .transpose()
                        .to_cols_array_2d()
                })
                .collect();
            let floor_y = sample_floor(actor.position.x, actor.position.z);
            let drop_verts = flatten_kg1_drop_shadow(
                kg1,
                &col_major_skin,
                floor_y,
                glam::Vec3::from_array(ob_env.directional_lights[0].direction),
            );
            ctx.update_shadow_volumes(&[], &drop_verts, 0x50 as f32 / 255.0);
        }

        let cut_proj =
            glam::Mat4::perspective_rh(player.camera.fov_y_radians, 640.0 / 360.0, 50.0, 120_000.0);
        let cut_view = player.camera.view_matrix();
        ctx.update_camera(cut_proj * cut_view, cut_proj.inverse(), player.camera.eye);

        let spot = build_cutscene_light_uniforms(
            &mut pool,
            &CutsceneLightInput {
                draw_env: Some(ob_env),
                demo_lights: player.dds_lights(),
                ambient_tint: player.atmosphere.ambient_tint_rgba,
                base_amb_scale: (0.9_f32 * 2.2).clamp(0.25, 2.2),
                is_dark_room: false,
                cam_eye: player.camera.eye,
                actor_pos: actor.position,
                flashlight_suppressed: false,
            },
        );
        ctx.update_spotlight(&spot);
        let skeletal_draw = [(&gpu_mesh, skin.as_slice(), Some(chr_tex_bgs.as_slice()))];

        for &(tier_name, gpu_map, map_bgs, enable_fixes) in &tiers {
            let mut fog_u = configure_ob01_fog_and_valley_mist(
                &ctx,
                actor.position,
                player.camera.eye,
                enable_fixes,
            );
            if let Some(rgb) = player.atmosphere.horizon_fog_rgb_override {
                fog_u.fog_color[0] = rgb[0];
                fog_u.fog_color[1] = rgb[1];
                fog_u.fog_color[2] = rgb[2];
                fog_u.fog_color[3] *= player.atmosphere.fog_density_scale;
                ctx.update_fog(&fog_u);
            }
            ctx.render_scene(Some(gpu_map), Some(map_bgs), &skeletal_draw);
            let rgba = readback_ldr_rgba8(&ctx);
            let luma = mean_luma(&rgba);
            println!(
                "tenbou {label} [{tier_name}]: cam={:?}, actor={:?}, mean_luma={luma:.2}",
                player.camera.eye, actor.position
            );
            assert!(
                luma > 20.0 && luma < 220.0,
                "tenbou {label} {tier_name} mean luma out of range: {luma:.2}"
            );
            if let Some(ref dir) = out_dir {
                write_ppm_rgb(
                    &dir.join(format!("tenbou_{label}_{tier_name}.ppm")),
                    640,
                    360,
                    &rgba,
                );
            }
        }
    }
}
