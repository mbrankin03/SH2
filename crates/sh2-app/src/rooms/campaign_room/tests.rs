//! Deterministic unit, retail-asset, and headless visual preview tests for unified Campaign
//! `.mes` event messages and 2D cutaway overlays (`SH2-157`).

use std::collections::HashMap;
use std::path::PathBuf;

use super::messages::{
    draw_campaign_message, load_mes_table, stage_mes_rel_path, CampaignMessageSource,
    CampaignMessageState, COMMON_MES_REL_PATH,
};
use crate::rooms::story_progression_room::hud_common::blit_cutaway_into_rasterizer;
use sh2_assets::formats::mes::{DialogueMessage, MessageTable};
use sh2_assets::{AssetPathResolver, CutawayTextureAsset};
use sh2_core::{CutawayAspectMode, CutawayOverlayEngine, CutawayQuality, EventFlags, PuzzleState};
use sh2_ui::TextRasterizer;

fn write_ppm(path: &std::path::Path, rast: &TextRasterizer) {
    let mut out = Vec::with_capacity(32 + rast.width * rast.height * 3);
    out.extend_from_slice(format!("P6\n{} {}\n255\n", rast.width, rast.height).as_bytes());
    for px in rast.pixels.as_chunks::<4>().0 {
        out.extend_from_slice(&px[0..3]);
    }
    std::fs::write(path, out).expect("write preview ppm");
}

#[test]
fn test_all_58_stages_resolve_to_existing_retail_mes_files() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    for stage_id in 1..=58u16 {
        let rel = stage_mes_rel_path(stage_id)
            .unwrap_or_else(|| panic!("Stage {stage_id} must map to a retail .mes path"));
        let table = load_mes_table(rel, false)
            .unwrap_or_else(|| panic!("Stage {stage_id} retail .mes failed to load from {rel}"));
        assert!(
            !table.messages.is_empty(),
            "Stage {stage_id} ({rel}) must contain at least one message"
        );
    }

    let common = load_mes_table(COMMON_MES_REL_PATH, false)
        .expect("common_msg_e.mes must load from retail assets");
    assert_eq!(common.get_text(4), Some("It's locked."));
    assert_eq!(common.get_text(5), Some("Now it's unlocked."));
    assert_eq!(common.get_text(9), Some("I got handgun bullets."));
    assert_eq!(common.get_text(17), Some("DUALSHOCK®2 not connected"));
    for msg in &common.messages {
        // Message 8 legitimately contains "What's this...?"
        if msg.id != 8 {
            assert!(
                !msg.text.contains('?'),
                "Unexpected '?' leak in common_msg_e.mes[{}]: {:?}",
                msg.id,
                msg.text
            );
        }
    }
}

#[test]
fn test_stage_observation_mes_clean_decoding_and_pagination_lifecycle() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    let obs = load_mes_table("etc/message/stage_observation_msg_e.mes", false)
        .expect("stage_observation_msg_e.mes must load");
    assert_eq!(
        obs.get_text(2),
        Some("There's a map in the car.\nIt might be a good idea to take it.")
    );
    assert_eq!(obs.get_text(3), Some("There's nothing else worth taking."));

    let msg5 = obs.get(5).expect("stage_observation_msg_e.mes[5]");
    assert_eq!(msg5.pages.len(), 2);
    assert_eq!(msg5.pages[0], "There's nothing on the\npassenger's seat.");
    assert_eq!(msg5.pages[1], "The map is on the driver's seat.");

    // Verify zero unexpected control-code '?' leaks across stage_observation_msg_e.mes.
    for msg in &obs.messages {
        for ch in msg.text.chars() {
            assert!(
                ch == '\n' || ch == '\x0C' || !ch.is_control(),
                "Unexpected control char {ch:?} in stage_observation_msg_e.mes[{}]",
                msg.id
            );
        }
    }

    // Standalone multi-page dismiss lifecycle (page 0 -> page 1 -> cleared).
    let mut hud = CampaignMessageState::default();
    hud.apply_effects(3, false, Some(5), None, false, false);
    assert_eq!(
        hud.active_page_text(),
        Some("There's nothing on the\npassenger's seat.")
    );
    assert!(hud.handle_dismiss_input());
    assert_eq!(
        hud.active_page_text(),
        Some("The map is on the driver's seat.")
    );
    assert!(hud.handle_dismiss_input());
    assert_eq!(hud.active_page_text(), None);
    assert!(!hud.handle_dismiss_input());
}

#[test]
fn test_program_driven_message_and_clear_lifecycle() {
    let stage_tbl = MessageTable {
        messages: vec![
            DialogueMessage {
                id: 0,
                pages: vec!["Stage msg 0".to_string()],
                text: "Stage msg 0".to_string(),
            },
            DialogueMessage {
                id: 1,
                pages: vec!["Page A".to_string(), "Page B".to_string()],
                text: "Page A\x0CPage B".to_string(),
            },
        ],
    };
    let common_tbl = MessageTable {
        messages: vec![DialogueMessage {
            id: 0,
            pages: vec!["It's locked.".to_string()],
            text: "It's locked.".to_string(),
        }],
    };

    let mut hud = CampaignMessageState::default();
    hud.set_tables_for_test(3, stage_tbl, common_tbl);

    // Program-driven message stays active until `clear_message` is emitted.
    hud.apply_effects(3, false, Some(0), None, false, true);
    assert_eq!(
        hud.active().map(|a| a.source),
        Some(CampaignMessageSource::Stage)
    );
    assert_eq!(hud.active_page_text(), Some("Stage msg 0"));
    assert!(!hud.handle_dismiss_input());
    assert_eq!(hud.active_page_text(), Some("Stage msg 0"));

    // Clear message resets active display.
    hud.apply_effects(3, false, None, None, true, false);
    assert_eq!(hud.active_page_text(), None);

    // Common message displays and dismisses on user confirm.
    hud.apply_effects(3, false, None, Some(0), false, false);
    assert_eq!(
        hud.active().map(|a| a.source),
        Some(CampaignMessageSource::Common)
    );
    assert_eq!(hud.active_page_text(), Some("It's locked."));
    assert!(hud.handle_dismiss_input());
    assert_eq!(hud.active_page_text(), None);
}

#[test]
fn test_out_p_incar_cutaway_and_message_overlay_render() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    let obs = load_mes_table("etc/message/stage_observation_msg_e.mes", false)
        .expect("stage_observation_msg_e.mes");
    let msg_with_map = obs.get_text(2).expect("msg 2");
    let msg_empty = obs.get_text(3).expect("msg 3");

    let mut flags = EventFlags::new();
    let puzzle = PuzzleState::default();
    let mut overlay = CutawayOverlayEngine::new();
    overlay.set_picture(Some("out_p_incar"), &flags);
    overlay.set_incar_alpha(1.0);
    let quads_with_map = overlay.evaluate_frame(&flags, &puzzle, 1, 0).to_vec();
    assert_eq!(quads_with_map.len(), 1);
    assert_eq!(quads_with_map[0].sheet_name, "out/p_incar_map");

    let mut cache: HashMap<(String, CutawayQuality), CutawayTextureAsset> = HashMap::new();
    let mut rast_with_map = TextRasterizer::new(1280, 720);
    rast_with_map.fill_rect(0, 0, 1280, 720, [0, 0, 0, 255]);
    blit_cutaway_into_rasterizer(
        &mut rast_with_map,
        &mut cache,
        0,
        0,
        1280,
        720,
        "out_p_incar",
        CutawayQuality::Sh2eeHd,
        CutawayAspectMode::Fit4x3Pillarbox,
        &quads_with_map,
    );
    draw_campaign_message(&mut rast_with_map, msg_with_map);

    // Second frame: map taken (alpha = 0.0, flag 28 set) + stage_observation_msg_e.mes[3]
    flags.set(28, true);
    overlay.set_incar_alpha(0.0);
    let quads_empty = overlay.evaluate_frame(&flags, &puzzle, 1, 0).to_vec();
    assert!(quads_empty.is_empty());

    let mut rast_empty = TextRasterizer::new(1280, 720);
    rast_empty.fill_rect(0, 0, 1280, 720, [0, 0, 0, 255]);
    blit_cutaway_into_rasterizer(
        &mut rast_empty,
        &mut cache,
        0,
        0,
        1280,
        720,
        "out_p_incar",
        CutawayQuality::Sh2eeHd,
        CutawayAspectMode::Fit4x3Pillarbox,
        &quads_empty,
    );
    draw_campaign_message(&mut rast_empty, msg_empty);

    // Assert car-seat cutaway rendered non-black pixels in center viewport
    let center_lit = rast_with_map
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 16 || px[1] > 16 || px[2] > 16)
        .count();
    assert!(
        center_lit > 100_000,
        "Expected fullscreen out_p_incar cutaway to cover viewport, got {center_lit} lit pixels"
    );

    // Assert the map sprite at (-48.375, -31.625) visibly alters the car seat pixels
    let diff_pixels = rast_with_map
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rast_empty.pixels.as_chunks::<4>().0.iter())
        .filter(|(a, b)| {
            a[0].abs_diff(b[0]) > 15 || a[1].abs_diff(b[1]) > 15 || a[2].abs_diff(b[2]) > 15
        })
        .count();
    assert!(
        diff_pixels > 5_000,
        "Expected p_incar_map sprite + message diff > 5000 px, got {diff_pixels}"
    );

    // Assert bottom subtitle band contains bright white glyph pixels
    let subtitle_band_start = (720 - 140) * 1280 * 4;
    let subtitle_white_px = rast_with_map.pixels[subtitle_band_start..]
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] > 200 && px[1] > 200 && px[2] > 200)
        .count();
    assert!(
        subtitle_white_px > 500,
        "Expected bright subtitle glyph pixels in bottom band, got {subtitle_white_px}"
    );

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm(
            &dir.join("sh2_157_out_p_incar_with_map.ppm"),
            &rast_with_map,
        );
        write_ppm(&dir.join("sh2_157_out_p_incar_empty.ppm"), &rast_empty);
    }
}
