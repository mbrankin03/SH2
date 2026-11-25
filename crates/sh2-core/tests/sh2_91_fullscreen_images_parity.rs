//! SH2-91: Golden-vector 1:1 parity tests for SH2:EE `FullscreenImages.cpp` / `FullscreenImages.h`
//! aspect-ratio scaling (`4:3` pillarbox vs `16:9` widescreen crop), `IsScaled` rules,
//! pixel-locked `-256..256` overlay sprite alignment across `4:3`, `16:9`, `20:9`, `21:9`, `32:9`,
//! and 3-tier `CutawayQuality` config preset integration.

use sh2_core::{
    is_cutaway_widescreen_scalable, AppConfig, CutawayAspectMode, CutawayOverlayEngine,
    CutawayQuality, EnhancedPreset, EventFlags, FullscreenImageLayout, PuzzleState,
};

#[test]
fn test_fullscreen_images_is_scaled_rules_match_header() {
    // `IsScaled == 1` in `FullscreenImages.h`:
    assert!(is_cutaway_widescreen_scalable("out_p_incar"));
    assert!(is_cutaway_widescreen_scalable(
        "data/pic/out/p_incar_map.tex"
    ));
    assert!(is_cutaway_widescreen_scalable("apt_clock_open"));
    assert!(is_cutaway_widescreen_scalable("apt/clock_hari"));
    assert!(is_cutaway_widescreen_scalable("apt_p_safe_close"));
    assert!(is_cutaway_widescreen_scalable("hsp_p_box"));
    assert!(is_cutaway_widescreen_scalable("dls_p_manacles_on"));
    assert!(is_cutaway_widescreen_scalable("htl_p_trank"));
    assert!(is_cutaway_widescreen_scalable("add_p_teien"));
    assert!(is_cutaway_widescreen_scalable("ufo/hj01"));

    // `IsScaled == 0` in `FullscreenImages.h` (maps, items, letters, edge-critical panels):
    assert!(!is_cutaway_widescreen_scalable("map/apartmape1f"));
    assert!(!is_cutaway_widescreen_scalable(
        "data/pic/map/hospitalmapmk.tex"
    ));
    assert!(!is_cutaway_widescreen_scalable("add/mapboldwin"));
    assert!(!is_cutaway_widescreen_scalable("item/x_coinelder"));
    assert!(!is_cutaway_widescreen_scalable("htl/p_laura_letter"));
    assert!(!is_cutaway_widescreen_scalable("hsp/p_panel"));
    assert!(!is_cutaway_widescreen_scalable("htl/p_hanging2"));
    assert!(!is_cutaway_widescreen_scalable("etc/gameover1"));
    assert!(!is_cutaway_widescreen_scalable("etc/start00"));
}

#[test]
fn test_fullscreen_images_aspect_modes_across_4_3_16_9_20_9_21_9_32_9() {
    let viewports = [
        ("4:3", 1024.0, 768.0),
        ("16:9", 1920.0, 1080.0),
        ("20:9", 2400.0, 1080.0),
        ("21:9", 2560.0, 1080.0),
        ("32:9", 3840.0, 1080.0),
    ];

    for (label, vw, vh) in viewports {
        // 1. Fit4x3Pillarbox: never crops top/bottom, preserves 4:3 width = vh * 4/3
        let fit = FullscreenImageLayout::compute(
            vw,
            vh,
            "apt_clock_open",
            CutawayAspectMode::Fit4x3Pillarbox,
        );
        assert!(!fit.is_widescreen_cropped, "{label} fit should not crop");
        assert!((fit.draw_y - 0.0).abs() < 1e-3, "{label} fit draw_y");
        assert!((fit.draw_h - vh).abs() < 1e-3, "{label} fit draw_h");
        assert!(
            (fit.draw_w - vh * (4.0 / 3.0)).abs() < 1e-2,
            "{label} fit draw_w"
        );

        // 2. Fill16x9WidescreenCrop on scalable still (`apt_clock_open`):
        let fill = FullscreenImageLayout::compute(
            vw,
            vh,
            "apt_clock_open",
            CutawayAspectMode::Fill16x9WidescreenCrop,
        );
        if vw / vh > 4.0 / 3.0 + 1e-4 {
            assert!(fill.is_widescreen_cropped, "{label} fill should zoom/crop");
            // `SizeFullscreen` caps at `alg2 * 0.75 = 4/3` -> `draw_h = vh * 4/3`, `draw_w = vh * 16/9`
            assert!(
                (fill.draw_h - vh * (4.0 / 3.0)).abs() < 1e-2,
                "{label} fill draw_h"
            );
            assert!(
                (fill.draw_w - vh * (16.0 / 9.0)).abs() < 1e-2,
                "{label} fill draw_w"
            );
            if label == "16:9" {
                assert!(
                    fill.draw_x.abs() < 1e-2,
                    "16:9 should fill width edge-to-edge"
                );
                assert!(!fill.black_bars[0].is_non_empty());
            } else {
                // 20:9, 21:9, 32:9 have black pillarboxes outside 16:9
                assert!(fill.draw_x > 10.0, "{label} should pillarbox outside 16:9");
                assert!(fill.black_bars[0].is_non_empty());
                assert!(fill.black_bars[1].is_non_empty());
            }
        } else {
            assert!(!fill.is_widescreen_cropped, "4:3 viewport never crops");
        }

        // 3. Edge-critical `IsScaled == false` still (`hsp_p_panel`) always preserves 4:3 even in Fill mode!
        let panel_fill = FullscreenImageLayout::compute(
            vw,
            vh,
            "hsp_p_panel",
            CutawayAspectMode::Fill16x9WidescreenCrop,
        );
        assert!(
            !panel_fill.is_widescreen_cropped,
            "{label} hsp_p_panel must preserve 4:3"
        );
        assert!((panel_fill.draw_w - vh * (4.0 / 3.0)).abs() < 1e-2);
    }
}

#[test]
fn test_overlay_sprites_stay_pixel_locked_to_background_across_resolutions() {
    let mut engine = CutawayOverlayEngine::new();
    let flags = EventFlags::default();
    let puzzle = PuzzleState::default();
    engine.set_picture(Some("out_p_incar"), &flags);
    let quads = engine.evaluate_frame(&flags, &puzzle, 1, 0);
    assert_eq!(quads.len(), 1);
    let incar_quad = &quads[0];

    // Normalized position of `out/p_incar_map` inside `-256..256`:
    // x0 = -774/16 = -48.375 -> norm_x0 = (-48.375 + 256) / 512 = 0.40551758
    let norm_x0 = (incar_quad.vertices[0].0 + 256.0) / 512.0;
    let norm_y0 = (incar_quad.vertices[0].1 + 256.0) / 512.0;

    for (vw, vh) in [
        (1024.0, 768.0),
        (1920.0, 1080.0),
        (2400.0, 1080.0),
        (2560.0, 1080.0),
        (3840.0, 1080.0),
    ] {
        for mode in [
            CutawayAspectMode::Fit4x3Pillarbox,
            CutawayAspectMode::Fill16x9WidescreenCrop,
        ] {
            let layout = FullscreenImageLayout::compute(vw, vh, "out_p_incar", mode);
            let tris = layout
                .map_and_clip_overlay_quad(incar_quad)
                .expect("InCar map quad should be visible");
            let expected_x0 = layout.draw_x + norm_x0 * layout.draw_w;
            let expected_y0 = layout.draw_y + norm_y0 * layout.draw_h;
            assert!(
                (tris[0].0 - expected_x0).abs() < 1e-3,
                "x0 mismatch at {vw}x{vh} {mode:?}"
            );
            assert!(
                (tris[0].1 - expected_y0).abs() < 1e-3,
                "y0 mismatch at {vw}x{vh} {mode:?}"
            );
        }
    }
}

#[test]
fn test_cutaway_quality_presets_and_vanilla_ps2_gate() {
    let mut cfg = AppConfig::default();
    assert_eq!(cfg.video.cutaway_quality, CutawayQuality::Ai4kMaster);

    cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert_eq!(cfg.video.cutaway_quality, CutawayQuality::OriginalPs2);
    assert_eq!(
        cfg.video.cutaway_aspect_mode,
        CutawayAspectMode::Fit4x3Pillarbox
    );
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::Ps2Authentic);

    cfg.apply_enhanced_preset(EnhancedPreset::Sh2ee);
    assert_eq!(cfg.video.cutaway_quality, CutawayQuality::Sh2eeHd);
    assert_eq!(
        cfg.video.cutaway_aspect_mode,
        CutawayAspectMode::Fill16x9WidescreenCrop
    );
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::Sh2ee);

    cfg.apply_enhanced_preset(EnhancedPreset::NativeEnhanced);
    assert_eq!(cfg.video.cutaway_quality, CutawayQuality::Ai4kMaster);
    assert_eq!(
        cfg.video.cutaway_aspect_mode,
        CutawayAspectMode::Fill16x9WidescreenCrop
    );
    assert_eq!(cfg.detect_enhanced_preset(), EnhancedPreset::NativeEnhanced);
}
