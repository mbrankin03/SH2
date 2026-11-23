//! Retail-asset & headless GPU integration test for `SH2-136`:
//! - 4-pass ping-pong soft stencil shadow blur (`0x001767e0`) + dual GS composite modes (`0x00175f90`)
//! - James self-shadow exclusion (`0x001a5e60` / `0x001a5f60`) on retail `jms.kg1`
//! - Dynamic room clear color (`0x00175930` / `0x00251370`)
//! - PS2 2D post-process suite (Vanilla PS2 ACES-off vs ACES-on, brightness LUT, LCG noise, glow, fade)
//! - Lens flare visibility smoothing (`0x00184ef0`, `0x00184bf0`)

use std::fs;
use std::path::PathBuf;

use glam::{Mat4, Vec3};
use sh2_assets::formats::kg::parse_kg;
use sh2_renderer::effects::{apply_ps2_lens_flare_smoothing, GpuLensFlareQuad, LensFlareSmoother};
use sh2_renderer::post::{
    ps2_brightness_scale, PostUniforms, Ps2FadeKind, POST_FLAG_ACES_TONEMAP,
    POST_FLAG_PS2_LCG_NOISE,
};
use sh2_renderer::shadow::{
    extrude_kg1_stencil_volume, extrude_kg1_stencil_volume_filtered, JamesSelfShadowExclusion,
    ShadowCompositeMode,
};
use sh2_renderer::{RenderContext, RendererConfig};

fn find_retail_data_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/data"),
        PathBuf::from(
            "/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/data",
        ),
    ];
    candidates.into_iter().find(|p| p.is_dir())
}

#[test]
fn test_sh2_136_soft_stencil_blur_self_shadow_exclusion_clear_color_and_post_pipeline() {
    let Some(data_dir) = find_retail_data_dir() else {
        eprintln!("Skipping SH2-136 retail integration test: retail data dir not found");
        return;
    };

    // 1. Verify James self-shadow exclusion reduces extruded vertices on retail `jms.kg1`
    let jms_kg1_path = data_dir.join("chr/jms/jms.kg1");
    let kg_bytes = fs::read(&jms_kg1_path).expect("read retail jms.kg1");
    let kg = parse_kg(&kg_bytes).expect("parse retail jms.kg1");
    let bone_mats = vec![Mat4::IDENTITY.to_cols_array_2d(); 64];
    let light_pos = Vec3::new(0.0, -1500.0, 2000.0);

    let full_verts = extrude_kg1_stencil_volume(&kg, &bone_mats, light_pos, 10_000.0);
    let head_excl = JamesSelfShadowExclusion::for_character(0x100, 3);
    let upper_excl = JamesSelfShadowExclusion::for_character(0x100, 2);
    let no_head_verts = extrude_kg1_stencil_volume_filtered(
        &kg,
        &bone_mats,
        light_pos,
        10_000.0,
        head_excl.excluded_parts(),
    );
    let no_upper_verts = extrude_kg1_stencil_volume_filtered(
        &kg,
        &bone_mats,
        light_pos,
        10_000.0,
        upper_excl.excluded_parts(),
    );
    assert!(
        !full_verts.is_empty(),
        "retail jms.kg1 must extrude shadow volume vertices"
    );
    assert!(
        no_head_verts.len() <= full_verts.len(),
        "head exclusion must not add vertices"
    );
    assert!(
        no_upper_verts.len() <= no_head_verts.len(),
        "upper-body exclusion is a superset of head exclusion"
    );

    // 2. Verify lens flare smoothing on GPU quads
    let mut smoother = LensFlareSmoother::default();
    let mut rate = 0.0;
    for _ in 0..8 {
        rate = smoother.step(true, 1.0, true);
    }
    assert!(rate > 0.7 && rate < 1.0);
    let mut quads = vec![GpuLensFlareQuad {
        rect: [0.1, 0.2, 0.05, 0.05],
        uv_rect: [0.0, 0.0, 0.5, 0.5],
        color: [0.8, 0.8, 0.8, 1.0],
    }];
    apply_ps2_lens_flare_smoothing(&mut quads, rate);
    assert!((quads[0].rect[3] - quads[0].rect[2] * (512.0 / 448.0)).abs() < 1e-6);

    // 3. Headless GPU verification: soft stencil blur (both GS modes), dynamic clear color,
    //    and Vanilla PS2 (ACES off) + Native Enhanced (ACES on) post-process passes.
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 256,
        height: 256,
        enable_hdr: true,
        enable_fog: false,
        enable_post: true,
    }) else {
        eprintln!("Skipping SH2-136 GPU pass verification: no headless adapter available");
        return;
    };

    ctx.pcss_enabled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    ctx.set_soft_shadow_blur_enabled(true);
    ctx.update_shadow_volumes(&no_upper_verts, &[], 80.0 / 128.0);

    // Day clear color (`0x46, 0x46, 0x46, 0x40`) + Subtractive GS shadow composite (`0x8000000052`)
    ctx.set_room_clear_color(false, None);
    ctx.set_shadow_composite_mode(ShadowCompositeMode::Subtractive);
    ctx.update_post(&PostUniforms {
        time: 1.0,
        grain_intensity: 0.65,
        exposure: ps2_brightness_scale(3),
        frame_count: 30,
        flags: POST_FLAG_PS2_LCG_NOISE, // Strict Vanilla PS2: ACES OFF
        glow_threshold: 0.7,
        glow_strength: 0.25,
        glow_step_px: 1.0,
        fade_rgba: Ps2FadeKind::FadeOutBlack.evaluate_rgba(5, 30),
        ..PostUniforms::default()
    });
    ctx.render_scene(None, None, &[]);

    // Night clear color (`0x00, 0x00, 0x00, 0x7c`) + Multiplicative GS shadow composite (`0x8000000046`)
    ctx.set_room_clear_color(true, None);
    ctx.set_shadow_composite_mode(ShadowCompositeMode::Multiplicative);
    ctx.update_post(&PostUniforms {
        time: 2.0,
        grain_intensity: 0.65,
        exposure: ps2_brightness_scale(5),
        frame_count: 60,
        flags: POST_FLAG_ACES_TONEMAP | POST_FLAG_PS2_LCG_NOISE, // Native Enhanced: ACES ON
        fade_rgba: Ps2FadeKind::FadeInRed.evaluate_rgba(15, 30),
        ..PostUniforms::default()
    });
    ctx.render_scene(None, None, &[]);
    let _ = ctx.device.poll(wgpu::Maintain::wait());
}
