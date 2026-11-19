//! End-to-end GPU integration tests for `SH2-81`:
//! - Retail `.kg1` / `.kg2` stencil shadow volume extrusion & floor drop-shadow rendering
//! - Native Enhanced `2048x2048` GPU PCSS spotlight shadow mapping + character self-shadowing
//! - Retail `lens_flare.tbn2` 7-sprite additive halo flares
//! - Retail `poison.tbn2` Hyper Spray 4-color particle clouds

use glam::{Mat4, Vec3};
use sh2_assets::formats::kg::{parse_kg1, parse_kg2};
use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile};
use sh2_assets::formats::texture::decode_texture_bytes;
use sh2_core::config::{LightingProfile, ShadowQualityMode};
use sh2_core::lighting::{
    apply_draw_env_fixes, evaluate_character_lighting, evaluate_lens_flares, lookup_draw_env,
    HyperSprayColorTier, HyperSprayEmitter, LensFlareSource, LENS_FLARE_SPRITE_COUNT,
};
use sh2_renderer::effects::{GpuLensFlareQuad, GpuSprayParticle};
use sh2_renderer::gpu_map::GpuMap;
use sh2_renderer::light::SpotlightUniforms;
use sh2_renderer::shadow::{
    extrude_kg1_stencil_volume, extrude_kg2_stencil_volume, flatten_kg1_drop_shadow,
    DEFAULT_SHADOW_EXTRUDE_DIST_MM,
};
use sh2_renderer::{RenderContext, RendererConfig};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const RETAIL_DATA: &str = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data";

fn synthetic_floor_room() -> MapFile {
    MapFile {
        name: "sh2_81_floor_test".to_string(),
        batches: vec![MapBatch {
            material_index: 0,
            geo_id: 0,
            positions: vec![
                [-4000.0, 0.0, -4000.0],
                [4000.0, 0.0, -4000.0],
                [4000.0, 0.0, 4000.0],
                [-4000.0, 0.0, 4000.0],
            ],
            normals: vec![
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
            ],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            colors: vec![[255, 255, 255, 255]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            group: BatchGroup::Opaque,
        }],
        materials: Vec::new(),
        textures: HashMap::new(),
    }
}

#[test]
fn test_sh2_81_pcss_stencil_drop_shadows_lens_flares_and_hyper_spray_pipeline() {
    let config = RendererConfig {
        width: 320,
        height: 240,
        enable_hdr: true,
        enable_fog: true,
        enable_post: true,
    };
    let Some(mut ctx) = RenderContext::new_headless(config) else {
        return;
    };

    let map = synthetic_floor_room();
    let gpu_map = GpuMap::from_map(&ctx.device, &map);

    let (proj, inv_proj) = ctx.perspective_projection(60f32.to_radians(), 25.0, 20_000.0);
    let cam_pos = Vec3::new(0.0, -1600.0, 2500.0);
    let view = Mat4::look_at_rh(cam_pos, Vec3::new(0.0, -800.0, 0.0), Vec3::NEG_Y);
    let vp = proj * view;
    ctx.update_camera(vp, inv_proj, cam_pos);

    let spot = SpotlightUniforms {
        position: [0.0, -1200.0, 1500.0, 1.0],
        direction_inner: [0.0, 0.25, -0.968, 0.92],
        color_outer: [1.2, 1.15, 1.0, 0.72],
        ambient_tint: [1.0, 1.0, 1.0, 1.0],
        ..Default::default()
    };
    ctx.update_spotlight(&spot);

    // 1. Native Enhanced PCSS mode
    ctx.update_shadow_config(ShadowQualityMode::NativePcss, 170.0 / 255.0, 0.5);
    ctx.render_gpu_map(&gpu_map);

    // 2. Classic PS2 Stencil + Drop Shadows from retail .kg1 & .kg2 (when retail assets present)
    let data_dir = Path::new(RETAIL_DATA);
    if data_dir.exists() {
        let jms_kg1_bytes =
            fs::read(data_dir.join("chr/jms/hhh_jms.kg1")).expect("read hhh_jms.kg1");
        let jms_kg1 = parse_kg1(&jms_kg1_bytes).expect("parse hhh_jms.kg1");
        let identity_bones = vec![Mat4::IDENTITY.to_cols_array_2d(); 64];
        let stencil_verts = extrude_kg1_stencil_volume(
            &jms_kg1,
            &identity_bones,
            Vec3::new(0.0, -1200.0, 1500.0),
            DEFAULT_SHADOW_EXTRUDE_DIST_MM,
        );
        let drop_verts =
            flatten_kg1_drop_shadow(&jms_kg1, &identity_bones, 0.0, Vec3::new(0.0, 1.0, 0.0));
        assert!(!stencil_verts.is_empty());
        assert!(!drop_verts.is_empty());

        let ap06_kg2_bytes = fs::read(data_dir.join("bg/ap/ap06.kg2")).expect("read ap06.kg2");
        let ap06_kg2 = parse_kg2(&ap06_kg2_bytes).expect("parse ap06.kg2");
        let kg2_verts = extrude_kg2_stencil_volume(
            &ap06_kg2,
            Vec3::new(-49843.0, -1442.0, -53000.0),
            DEFAULT_SHADOW_EXTRUDE_DIST_MM,
        );
        assert!(!kg2_verts.is_empty());

        ctx.update_shadow_config(ShadowQualityMode::ClassicPs2Stencil, 170.0 / 255.0, 1.0);
        ctx.update_shadow_volumes(&stencil_verts, &drop_verts, 170.0 / 255.0);

        // 3. Retail lens_flare.tbn2 upload + 7-sprite halo flare render
        let flare_path = data_dir.join("pic/effect/lens_flare.tbn2");
        let flare_bytes = fs::read(&flare_path).expect("read lens_flare.tbn2");
        let flare_tex =
            decode_texture_bytes(&flare_bytes, &flare_path).expect("decode lens_flare.tbn2");
        ctx.upload_lens_flare_texture(
            &flare_tex.rgba,
            flare_tex.width as u32,
            flare_tex.height as u32,
        );

        let flares = evaluate_lens_flares(
            &[LensFlareSource {
                position: Vec3::new(0.0, -900.0, -500.0),
                color: Vec3::ONE,
                max_distance: 15_000.0,
                intensity: 1.0,
            }],
            vp,
            cam_pos,
            320.0 / 240.0,
        );
        assert_eq!(flares.len(), LENS_FLARE_SPRITE_COUNT);
        let gpu_flares: Vec<GpuLensFlareQuad> = flares.into_iter().map(Into::into).collect();
        ctx.update_lens_flares(&gpu_flares);

        // 4. Retail poison.tbn2 upload + Hyper Spray 4-color particle cloud render
        let spray_path = data_dir.join("pic/effect/poison.tbn2");
        let spray_bytes = fs::read(&spray_path).expect("read poison.tbn2");
        let spray_tex =
            decode_texture_bytes(&spray_bytes, &spray_path).expect("decode poison.tbn2");
        ctx.upload_spray_mist_texture(
            &spray_tex.rgba,
            spray_tex.width as u32,
            spray_tex.height as u32,
        );

        let mut emitter = HyperSprayEmitter::new();
        for tier in [
            HyperSprayColorTier::Purple,
            HyperSprayColorTier::White,
            HyperSprayColorTier::Yellow,
            HyperSprayColorTier::Green,
        ] {
            emitter.spawn_hyper_spray(
                Vec3::new(0.0, -1000.0, 200.0),
                Vec3::NEG_Z,
                tier,
                false,
                true,
                false,
            );
        }
        emitter.update(0.05);
        let gpu_spray: Vec<GpuSprayParticle> = emitter.particles().iter().map(Into::into).collect();
        ctx.update_spray_particles(&gpu_spray);

        ctx.render_gpu_map(&gpu_map);
    }

    // 5. Verify DrawEnvData + Character Lighting integration
    let ps = lookup_draw_env("PS", 0).expect("PS_DrawEnvData");
    let fixed = apply_draw_env_fixes(ps, "ps189", LightingProfile::EnhancedEdition);
    assert_eq!(fixed.vertex_ambient, [2.5, 2.0, 2.0]);
    let char_light = evaluate_character_lighting(Vec3::ZERO, &fixed, &[], 1.0, 1.0);
    assert!(char_light.combined_rgb.length() > 0.0);
}
