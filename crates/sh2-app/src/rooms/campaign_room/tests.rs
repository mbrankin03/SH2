//! Deterministic unit, retail-asset, and headless visual preview tests for unified Campaign
//! `.mes` event messages and 2D cutaway overlays (`SH2-157`).

use crate::rooms::story_progression_room::cutaway_cache::CutawayLruCache;
use std::path::PathBuf;

use super::cutaway::{
    compute_cutaway_draw_rect, has_baked_ps2_letterbox_bars, render_cutaway_frame_into_rasterizer,
    CutawayCompositor, PS2_CUTAWAY_LETTERBOX_HEIGHT_RATIO,
};
use super::messages::{
    draw_campaign_message, load_mes_table, stage_mes_rel_path, CampaignMessageSource,
    CampaignMessageState, COMMON_MES_REL_PATH,
};
use crate::rooms::story_progression_room::hud_common::blit_cutaway_into_rasterizer;
use sh2_assets::formats::mes::{DialogueMessage, MessageTable};
use sh2_assets::{AssetPathResolver, CutawayTextureAsset, DecodedTexture};
use sh2_core::campaign::{CampaignOrchestrator, CampaignPhase, CampaignTickInput};
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::{
    CutawayAspectMode, CutawayOverlayEngine, CutawayQuality, EventFlags, FullscreenImageLayout,
    PuzzleState, SimulationCore,
};
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
fn test_sh2_188_first_toilet_voice_afs_loader_respects_audio_pack_and_decodes_non_silent_pcm() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    for pack in [
        sh2_core::AudioPackSource::EnhancedLossless,
        sh2_core::AudioPackSource::Ps2Original,
    ] {
        let wav = crate::assets::load_cutscene_voice_wav_for_source(117, pack)
            .unwrap_or_else(|| panic!("voice.afs[117] must decode for {pack:?}"));
        assert_eq!(wav.sample_rate, 44_100);
        assert_eq!(wav.channels, 2);
        assert_eq!(wav.bits_per_sample, 16);

        // Speech window (`11.0s..14.6s`, frames `576..684`) must contain non-silent 16-bit PCM.
        let bytes_per_sec = 44_100 * 2 * 2;
        let speech_end = wav.data.len().min(14 * bytes_per_sec + bytes_per_sec / 2);
        let speech_slice = &wav.data[11 * bytes_per_sec..speech_end];
        let peak = speech_slice
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c).unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(
            peak > 8_000,
            "voice.afs[117] ({pack:?}) speech window must have strong non-silent PCM (peak={peak})"
        );
    }
}

#[test]
fn test_program_driven_message_and_clear_lifecycle() {
    let stage_tbl = MessageTable {
        messages: vec![
            DialogueMessage {
                id: 0,
                pages: vec!["Stage msg 0".to_string()],
                colored_pages: Vec::new(),
                text: "Stage msg 0".to_string(),
            },
            DialogueMessage {
                id: 1,
                pages: vec!["Page A".to_string(), "Page B".to_string()],
                colored_pages: Vec::new(),
                text: "Page A\x0CPage B".to_string(),
            },
        ],
    };
    let common_tbl = MessageTable {
        messages: vec![DialogueMessage {
            id: 0,
            pages: vec!["It's locked.".to_string()],
            colored_pages: Vec::new(),
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

    let mut cache = CutawayLruCache::default();
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

#[test]
fn test_cutaway_baked_letterbox_detection_and_1080p_draw_rect_parity() {
    // Construct a synthetic 512x512 cutaway with 64px top/bottom black padding (y=0..64, 448..512)
    // and lit active 512x384 art in y=64..448, matching `out_p_incar.tex` (`PictureDraw` 0x001ae360).
    let mut letterboxed = vec![0u8; 512 * 512 * 4];
    for y in 64..448 {
        for x in 0..512 {
            let idx = (y * 512 + x) * 4;
            letterboxed[idx] = 80;
            letterboxed[idx + 1] = 110;
            letterboxed[idx + 2] = 140;
            letterboxed[idx + 3] = 255;
        }
    }
    assert!(has_baked_ps2_letterbox_bars(&letterboxed, 512, 512));

    // A full-bleed 512x512 texture (no top/bottom black padding) must return false.
    let full_bleed = vec![96u8; 512 * 512 * 4];
    assert!(!has_baked_ps2_letterbox_bars(&full_bleed, 512, 512));

    let asset = CutawayTextureAsset {
        texture: DecodedTexture {
            width: 512,
            height: 512,
            rgba: letterboxed,
        },
        original_width: 512,
        original_height: 512,
        is_sh2ee_hd: false,
    };

    // Original 4:3 Pillarbox at 1920x1080: canvas is x=[240..1680], y=[0..1080].
    // Active 512x384 band (v=0.125..0.875) must map to y=[0..1080] (draw_h = 1080 * 512/384 = 1440).
    let layout_4x3 = FullscreenImageLayout::compute(
        1920.0,
        1080.0,
        "out_p_incar",
        CutawayAspectMode::Fit4x3Pillarbox,
    );
    let xf_4x3 = compute_cutaway_draw_rect(1920.0, 1080.0, &layout_4x3, &asset);
    assert!((xf_4x3.offset_x - 240.0).abs() < 0.01);
    assert!((xf_4x3.offset_y - (-180.0)).abs() < 0.01);
    assert!((xf_4x3.draw_width - 1440.0).abs() < 0.01);
    assert!(
        (xf_4x3.draw_height - 1080.0 * PS2_CUTAWAY_LETTERBOX_HEIGHT_RATIO).abs() < 0.01,
        "Expected draw_height=1440.0, got {}",
        xf_4x3.draw_height
    );

    // Widescreen Fill at 1920x1080: draw_x=0, draw_y=-180, draw_w=1920, draw_h=1440 -> full edge-to-edge!
    let layout_16x9 = FullscreenImageLayout::compute(
        1920.0,
        1080.0,
        "out_p_incar",
        CutawayAspectMode::Fill16x9WidescreenCrop,
    );
    let xf_16x9 = compute_cutaway_draw_rect(1920.0, 1080.0, &layout_16x9, &asset);
    assert!((xf_16x9.offset_x - 0.0).abs() < 0.01);
    assert!((xf_16x9.offset_y - (-180.0)).abs() < 0.01);
    assert!((xf_16x9.draw_width - 1920.0).abs() < 0.01);
    assert!((xf_16x9.draw_height - 1440.0).abs() < 0.01);
}

#[test]
fn test_out_p_incar_native_1080p_pillarbox_and_widescreen_fill() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    let flags = EventFlags::new();
    let puzzle = PuzzleState::default();
    let mut overlay = CutawayOverlayEngine::new();
    overlay.set_picture(Some("out_p_incar"), &flags);
    overlay.set_incar_alpha(1.0);
    let quads = overlay.evaluate_frame(&flags, &puzzle, 1, 0).to_vec();
    assert_eq!(quads.len(), 1);

    let mut cache = CutawayLruCache::default();

    // 1. Original 4:3 Pillarbox at native 1920x1080: fills 100% height (y=0..1079) inside x=240..1679,
    //    with pure black pillarboxes outside x=240..1680 and ZERO top/bottom windowboxing.
    let mut rast_4x3 = TextRasterizer::new(1920, 1080);
    render_cutaway_frame_into_rasterizer(
        &mut rast_4x3,
        &mut cache,
        "out_p_incar",
        CutawayQuality::Sh2eeHd,
        CutawayAspectMode::Fit4x3Pillarbox,
        &quads,
    );

    let lit_in_rect = |r: &TextRasterizer, x0: usize, y0: usize, x1: usize, y1: usize| -> usize {
        let mut count = 0usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * r.width + x) * 4;
                if r.pixels[i] > 10 || r.pixels[i + 1] > 10 || r.pixels[i + 2] > 10 {
                    count += 1;
                }
            }
        }
        count
    };

    // Verify top & bottom bands inside the 4:3 canvas are lit (no top/bottom black bars).
    assert!(
        lit_in_rect(&rast_4x3, 240, 0, 1680, 16) > 500,
        "4:3 pillarbox cutaway must fill top edge y=0..16 with zero letterbox gap"
    );
    assert!(
        lit_in_rect(&rast_4x3, 240, 1064, 1680, 1080) > 500,
        "4:3 pillarbox cutaway must fill bottom edge y=1064..1080 with zero letterbox gap"
    );
    // Verify left & right pillarbox bars are pure black.
    assert_eq!(
        lit_in_rect(&rast_4x3, 0, 0, 239, 1080),
        0,
        "Left 4:3 pillarbox bar must remain pure black"
    );
    assert_eq!(
        lit_in_rect(&rast_4x3, 1681, 0, 1920, 1080),
        0,
        "Right 4:3 pillarbox bar must remain pure black"
    );

    // 2. Widescreen Fill at native 1920x1080: fills all 4 edges (0..1920 x 0..1080) with zero bars.
    let mut rast_16x9 = TextRasterizer::new(1920, 1080);
    render_cutaway_frame_into_rasterizer(
        &mut rast_16x9,
        &mut cache,
        "out_p_incar",
        CutawayQuality::Sh2eeHd,
        CutawayAspectMode::Fill16x9WidescreenCrop,
        &quads,
    );
    assert!(
        lit_in_rect(&rast_16x9, 0, 0, 1920, 16) > 500,
        "16:9 Widescreen Fill must fill top edge y=0..16"
    );
    assert!(
        lit_in_rect(&rast_16x9, 0, 1064, 1920, 1080) > 500,
        "16:9 Widescreen Fill must fill bottom edge y=1064..1080"
    );
    assert!(
        lit_in_rect(&rast_16x9, 0, 0, 16, 1080) > 500,
        "16:9 Widescreen Fill must fill left edge x=0..16"
    );
    assert!(
        lit_in_rect(&rast_16x9, 1904, 0, 1920, 1080) > 500,
        "16:9 Widescreen Fill must fill right edge x=1904..1920"
    );

    if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
        let dir = PathBuf::from(out_dir);
        let _ = std::fs::create_dir_all(&dir);
        write_ppm(&dir.join("sh2_162_out_p_incar_1080p_4x3.ppm"), &rast_4x3);
        write_ppm(&dir.join("sh2_162_out_p_incar_1080p_16x9.ppm"), &rast_16x9);
    }
}

#[test]
fn test_campaign_stage3_action_only_triggers_car_door_within_retail_event_check_look() {
    // Start a unified Campaign session directly in Stage 3 (`stg_ovservation`, `ob01`) with the
    // intro cutscene flag `33` already set so `ActiveGameplay` evaluates `EventMainStandard`.
    let mut sim = SimulationCore::new(sh2_core::scenario::ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 3;
    sim.scenario.current_room = "ob01".to_string();
    sim.scenario.flags.set(33, true);

    let mut orch = CampaignOrchestrator::from_saved_stage(
        3,
        Some("ob01".to_string()),
        [0.0, 0.0, 0.0],
        0.0,
        OutdoorStreamingMode::ClassicPs2LoadBg2x2,
        false,
    );
    // Advance past RoomLoading / FadeIn into ActiveGameplay.
    for _ in 0..120 {
        let eff = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        if eff.load_room.is_some() {
            orch.notify_room_assets_loaded();
        }
        if matches!(orch.phase, CampaignPhase::ActiveGameplay) {
            break;
        }
    }
    assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));

    // 1. Standing far away from the Blue Car (0, 0, 0) and pressing Action [E] must NOT open
    //    `out_p_incar` or grant the Map of Silent Hill.
    sim.snap_player_transform(glam::Vec3::new(0.0, 0.0, 0.0), 0.0);
    let action_input = CampaignTickInput {
        action_pressed: true,
        ..CampaignTickInput::default()
    };
    let _ = orch.step(&mut sim, &action_input, 1.0 / 60.0);
    assert_eq!(
        sim.scenario.cutaway_overlay.active_picture(),
        None,
        "Pressing [E] away from car must never trigger out_p_incar cutaway in Campaign"
    );
    assert!(!sim.scenario.flags.get(24) && !sim.scenario.flags.get(28));

    // 2. Standing at the retail car door trigger `(15650, 0, -4650)` facing the car with
    //    `flag(24) == false` and pressing Action [E] MUST trigger Stage 3 `EvProg` #2 (`0x005a09b0`)
    //    -> `EvSubPictureDisplay` fade-out/fade-in -> `out_p_incar` cutaway WITH `out/p_incar_map`
    //    at `incar_alpha == 1.0`.
    sim.snap_player_transform(glam::Vec3::new(15650.0, 0.0, -4650.0), 0.0);
    let _ = orch.step(&mut sim, &action_input, 1.0 / 60.0);
    for _ in 0..140 {
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        if sim.scenario.cutaway_overlay.active_picture() == Some("out_p_incar")
            && orch.fade.is_fully_clear()
        {
            break;
        }
    }
    assert_eq!(
        sim.scenario.cutaway_overlay.active_picture(),
        Some("out_p_incar"),
        "Pressing [E] at retail car door (15650, 0, -4650) must open out_p_incar cutaway"
    );
    assert!(
        orch.fade.is_fully_clear(),
        "Cutaway fade-in must complete before accepting interaction input"
    );
    assert!(
        (sim.scenario.cutaway_overlay.incar_alpha() - 1.0).abs() < 1e-5,
        "First [E] press at car before pickup must run ev_prog[2] (0x005a09b0) with incar_alpha == 1.0, got {}",
        sim.scenario.cutaway_overlay.incar_alpha()
    );
    let quads = sim.scenario.cutaway_overlay.evaluate_frame(
        &sim.scenario.flags,
        &sim.scenario.puzzle_state,
        1,
        0,
    );
    assert_eq!(
        quads.len(),
        1,
        "First [E] press must emit the out/p_incar_map overlay sprite quad"
    );
    assert_eq!(quads[0].sheet_name, "out/p_incar_map");

    // 3. Complete `ev_prog[2]`: [E] -> message 28 + SE 0x2b17, [E] -> 1.0s map fade ->
    //    `EvSubPictureDisplay` exit fade-out/fade-in -> sets `flag(24)`.
    let eff_msg = orch.step(&mut sim, &action_input, 1.0 / 60.0);
    assert_eq!(eff_msg.stage_message, Some(28));
    assert!(eff_msg.sound_effects.contains(&0x2b17));
    let _ = orch.step(&mut sim, &action_input, 1.0 / 60.0);
    for _ in 0..200 {
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        if sim.scenario.cutaway_overlay.active_picture().is_none() && orch.fade.is_fully_clear() {
            break;
        }
    }
    assert!(
        sim.scenario.flags.get(24),
        "Completing ev_prog[2] must set flag(24) (Map of Silent Hill obtained)"
    );
    assert_eq!(sim.scenario.cutaway_overlay.active_picture(), None);

    // 4. Re-examining the car after `flag(24) == true` MUST trigger `ev_prog[3]` (`0x005a0ca0`)
    //    -> `EvSubPictureDisplay` fade-out/fade-in -> empty `out_p_incar` cutaway (`incar_alpha == 0.0`).
    let _ = orch.step(&mut sim, &action_input, 1.0 / 60.0);
    for _ in 0..140 {
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        if sim.scenario.cutaway_overlay.active_picture() == Some("out_p_incar") {
            break;
        }
    }
    assert_eq!(
        sim.scenario.cutaway_overlay.active_picture(),
        Some("out_p_incar")
    );
    assert!(
        sim.scenario.cutaway_overlay.incar_alpha().abs() < 1e-5,
        "Second [E] press after map pickup must run ev_prog[3] (0x005a0ca0) with incar_alpha == 0.0"
    );
}

#[test]
fn test_cutaway_compositor_dirty_row_cache_matches_full_render_across_incar_alpha_fade() {
    let flags = EventFlags::new();
    let puzzle = PuzzleState::default();
    let mut overlay = CutawayOverlayEngine::new();
    overlay.set_picture(Some("out_p_incar"), &flags);

    let mut cache = CutawayLruCache::default();
    let quality = CutawayQuality::OriginalPs2;
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        // Deterministic synthetic 512x512 letterboxed cutaway + 256x256 map sprite fallback
        // when retail assets are not mounted.
        let mut base_rgba = vec![0u8; 512 * 512 * 4];
        for y in 64..448 {
            for x in 0..512 {
                let i = (y * 512 + x) * 4;
                base_rgba[i] = ((x * 255) / 512) as u8;
                base_rgba[i + 1] = ((y * 255) / 512) as u8;
                base_rgba[i + 2] = 96;
                base_rgba[i + 3] = 255;
            }
        }
        cache.insert(
            ("out_p_incar".to_string(), quality),
            CutawayTextureAsset {
                texture: DecodedTexture {
                    width: 512,
                    height: 512,
                    rgba: base_rgba,
                },
                original_width: 512,
                original_height: 512,
                is_sh2ee_hd: false,
            },
        );
        let mut sprite_rgba = vec![0u8; 256 * 256 * 4];
        for y in 0..256 {
            for x in 0..256 {
                let i = (y * 256 + x) * 4;
                sprite_rgba[i] = 220;
                sprite_rgba[i + 1] = 195;
                sprite_rgba[i + 2] = 140;
                sprite_rgba[i + 3] = 255;
            }
        }
        cache.insert(
            ("out/p_incar_map".to_string(), quality),
            CutawayTextureAsset {
                texture: DecodedTexture {
                    width: 256,
                    height: 256,
                    rgba: sprite_rgba,
                },
                original_width: 256,
                original_height: 256,
                is_sh2ee_hd: false,
            },
        );
    }

    for aspect_mode in [
        CutawayAspectMode::Fit4x3Pillarbox,
        CutawayAspectMode::Fill16x9WidescreenCrop,
    ] {
        let width = 1280u32;
        let height = 720u32;
        let mut compositor = CutawayCompositor::new(width, height);
        let mut reference = TextRasterizer::new(width as usize, height as usize);

        for (step, &alpha) in [1.0f32, 0.5, 0.0].iter().enumerate() {
            overlay.set_incar_alpha(alpha);
            let quads = overlay.evaluate_frame(&flags, &puzzle, 1, 0).to_vec();

            let dirty = compositor.update(&mut cache, "out_p_incar", quality, aspect_mode, &quads);
            let (dy0, dy1) = dirty.expect("changing alpha must report a dirty row span");
            if step == 0 {
                assert_eq!((dy0, dy1), (0, height as usize));
            } else {
                assert!(
                    dy0 < dy1 && (dy1 - dy0) < height as usize,
                    "Expected partial dirty row band on alpha={alpha} ({aspect_mode:?}), got ({dy0}, {dy1})"
                );
            }

            // Repeating the exact same quads must be a no-op (None).
            assert_eq!(
                compositor.update(&mut cache, "out_p_incar", quality, aspect_mode, &quads),
                None,
                "Identical frame must skip compositing and GPU upload"
            );

            render_cutaway_frame_into_rasterizer(
                &mut reference,
                &mut cache,
                "out_p_incar",
                quality,
                aspect_mode,
                &quads,
            );
            assert_eq!(
                compositor.rasterizer.pixels, reference.pixels,
                "Cached dirty-row cutaway pixels must be bit-identical to full render at alpha={alpha} ({aspect_mode:?})"
            );
        }
    }
}

/// Real retail-asset integration test for `SH2-191`: verifies the full live Stage 4 Graveyard
/// Angela cutscene (`haka_agl.dds` / `demo_no = 3`, `ev_prog[2]` `0x005a0570`) chain using
/// retail assets on disk:
/// - `ACT1_FOREST_EVENT_LIST[12]` (`0x008e6ff8`) triggers `CampaignPhase::DramaDemo { demo_no: 3 }`,
/// - `DRAMA_DEMO_CATALOG` resolves `demo_no = 3` -> `"haka_agl"` (`data/demo/haka_agl/haka_agl.dds`),
/// - `DramaDemoPlayer` parses real `haka_agl.dds` with both James (`hhh_jms_pos`) and Angela
///   (`agl_pos`) visible and fires `pending_voice_trigger` at `stream_start = 21` (`0xea95`),
/// - `voice.afs[53]` decodes non-silent 16-bit PCM for both `EnhancedLossless` and `Ps2Original`,
/// - `sh2_audio::select_bgm_situation(4, ...)` selects Situation `4` (`forest/bgm_114_b.aix`)
///   during `demo_no = 3` and Situation `1` once `flag(36)` is committed,
/// - Natural playback completion returns to `ActiveGameplay` with `flag(36)` set and zero
///   re-trigger loop on subsequent gameplay ticks.
#[test]
fn test_sh2_191_angela_graveyard_haka_agl_live_campaign_e2e_assets_audio_and_no_retrigger() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    // 1. Verify catalog entry & retail `.dds` + `.mes` + `voice.afs[53]` assets on disk.
    let catalog_idx = sh2_core::DRAMA_DEMO_CATALOG
        .iter()
        .position(|d| d.demo_no == 3)
        .expect("demo_no = 3 (haka_agl) must exist in DRAMA_DEMO_CATALOG");
    let info = sh2_core::DRAMA_DEMO_CATALOG[catalog_idx];
    assert_eq!(info.id, "haka_agl");
    assert_eq!(info.stream_no, 60053);
    assert_eq!(info.voice_afs_index(), Some(53));
    assert_eq!(info.stream_start, 21);

    let dds_bytes = std::fs::read(resolver.game_root.join(info.dds_path))
        .expect("read retail data/demo/haka_agl/haka_agl.dds");
    let script = sh2_assets::formats::demo::parse_demo_dds(&dds_bytes).expect("parse haka_agl.dds");
    let mes_bytes = std::fs::read(resolver.game_root.join(info.mes_path))
        .expect("read retail stage_forest_msg_e.mes");
    let mes_table =
        sh2_assets::formats::mes::parse_mes(&mes_bytes).expect("parse stage_forest_msg_e.mes");
    assert!(
        mes_table.get_text(usize::from(info.msg_start)).is_some(),
        "stage_forest_msg_e.mes must contain haka_agl msg_start={}",
        info.msg_start
    );

    for pack in [
        sh2_core::AudioPackSource::EnhancedLossless,
        sh2_core::AudioPackSource::Ps2Original,
    ] {
        let wav = crate::assets::load_cutscene_voice_wav_for_source(53, pack)
            .unwrap_or_else(|| panic!("voice.afs[53] (0xea95) must decode for {pack:?}"));
        assert_eq!(wav.bits_per_sample, 16);
        assert!(
            wav.data.len() > 100_000,
            "voice.afs[53] ({pack:?}) must contain full cutscene dialogue PCM"
        );
        let peak = wav
            .data
            .as_chunks::<2>()
            .0
            .iter()
            .step_by(64)
            .map(|c| i16::from_le_bytes(*c).unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(
            peak > 4_000,
            "voice.afs[53] ({pack:?}) must contain non-silent speech PCM (peak={peak})"
        );
    }

    // 2. Drive live `CampaignOrchestrator` into `ACT1_FOREST_EVENT_LIST[12]` trigger polygon.
    let forest =
        sh2_core::events::stages_act1::act1_stage_descriptor(4).expect("Stage 4 Forest descriptor");
    let entry12 = forest.event_list[12];
    let shape12 = sh2_core::events::triggers::decode_ev_pos_shape(
        forest.ev_pos,
        entry12.ev_pos_offset(),
        entry12.shape_kind().expect("shape kind"),
    )
    .expect("decode ACT1_FOREST_EVENT_LIST[12]");
    let trigger_pos = match shape12 {
        sh2_core::events::triggers::EvPosShape::Triangle { y, vertices } => [
            (vertices[0][0] + vertices[1][0] + vertices[2][0]) / 3.0,
            y,
            (vertices[0][1] + vertices[1][1] + vertices[2][1]) / 3.0,
        ],
        sh2_core::events::triggers::EvPosShape::Quad { y, vertices } => [
            (vertices[0][0] + vertices[1][0] + vertices[2][0] + vertices[3][0]) * 0.25,
            y,
            (vertices[0][1] + vertices[1][1] + vertices[2][1] + vertices[3][1]) * 0.25,
        ],
        other => panic!("unexpected shape: {other:?}"),
    };

    let mut sim = SimulationCore::new(sh2_core::ScenarioKind::LetterFromSilentHeaven);
    sim.scenario.current_stage = 4;
    sim.scenario.current_room = "ca11".to_string();
    sim.snap_player_transform(glam::Vec3::from_array(trigger_pos), 0.0);

    let mut orch = CampaignOrchestrator::from_saved_stage(
        4,
        Some("ca11".to_string()),
        trigger_pos,
        0.0,
        OutdoorStreamingMode::default(),
        false,
    );
    orch.phase = CampaignPhase::ActiveGameplay;
    orch.fade = sh2_core::campaign::ScreenFadeState::default();
    orch.player_locked = false;

    let idle = CampaignTickInput::default();
    let _ = orch.step(&mut sim, &idle, 1.0 / 60.0);
    let eff_start = orch.step(&mut sim, &idle, 1.0 / 60.0);
    let started = eff_start
        .start_drama_demo
        .expect("graveyard entry must start haka_agl");
    assert_eq!(started.demo_no, 3);
    assert_eq!(started.demo_id, Some("haka_agl"));

    // 3. Verify `sh2_audio` BGM routing during `demo_no = 3`: Page 47 (`forest/bgm_114_b.aix`)
    //    and Situation 4 (`EvBgmControl` `0x005a0260`).
    let mut bgm_flags = sh2_audio::BgmEventFlags::from_event_flag_bytes(
        sim.scenario.flags.as_bytes(),
        false,
        true,
        false,
    );
    bgm_flags.active_demo_no = Some(3);
    assert_eq!(sh2_audio::pick_bgm_page(4, 0, &bgm_flags), 47);
    assert_eq!(sh2_audio::eval_stage_ev_bgm_control(4, 0, &bgm_flags), 4);

    // 4. Step real `DramaDemoPlayer` for `haka_agl.dds` until voice triggers & completion.
    let mut player = sh2_core::DramaDemoPlayer::new(info, script, false);
    assert!(
        player.actors.iter().any(|a| a.actor_name == "agl_pos"),
        "haka_agl.dds must include Angela actor (agl_pos)"
    );
    assert!(
        player.actors.iter().any(|a| a.actor_name == "hhh_jms_pos"),
        "haka_agl.dds must include James actor (hhh_jms_pos)"
    );
    let mut voice_triggered = false;
    for _ in 0..30 {
        player.tick(1.0 / 30.0, true);
        if player.pending_voice_trigger {
            voice_triggered = true;
            player.pending_voice_trigger = false;
            break;
        }
    }
    assert!(
        voice_triggered,
        "haka_agl DramaDemoPlayer must fire pending_voice_trigger at stream_start=21"
    );

    // Advance to natural EOF and hand off back to `CampaignOrchestrator`.
    player.skip();
    assert_eq!(player.step, sh2_core::cutscene::DramaDemoStep::Finished);
    let eff_end = orch.step(
        &mut sim,
        &CampaignTickInput {
            drama_demo_finished: true,
            ..CampaignTickInput::default()
        },
        1.0 / 60.0,
    );
    assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));
    assert!(sim
        .scenario
        .flags
        .get(sh2_core::events::STG4_FLAG_ANGELA_MET));
    assert!(eff_end.start_drama_demo.is_none());

    // Post-cutscene BGM shifts to Situation 1 (`flag(36)` set, `active_demo_no = None`).
    let post_bgm_flags = sh2_audio::BgmEventFlags::from_event_flag_bytes(
        sim.scenario.flags.as_bytes(),
        false,
        false,
        false,
    );
    assert_eq!(
        sh2_audio::eval_stage_ev_bgm_control(4, 0, &post_bgm_flags),
        1
    );

    // Confirm zero re-triggering on subsequent live gameplay ticks.
    for _ in 0..5 {
        let eff = orch.step(&mut sim, &idle, 1.0 / 60.0);
        assert!(eff.start_drama_demo.is_none());
        assert!(matches!(orch.phase, CampaignPhase::ActiveGameplay));
    }
}

/// Real retail-asset integration test for `SH2-191` follow-up: verifies both post-cutscene
/// Angela graveyard conversation variants (`haka_agl_h.dds` `ev_prog[3]` `0x0059f4a0` / `0x008e76d8`
/// and `haka_agl_i.dds` `ev_prog[4]` `0x0059f5c0` / `0x008e7720`) resolve unambiguously in
/// `DRAMA_DEMO_CATALOG`, parse real retail `.dds`/`.mes` assets with exact shared `.anm`/`.cls`
/// frame offsets (`3284` / `3741`), decode non-silent `voice.afs[54]` & `voice.afs[55]` PCM, and
/// play + finish cleanly via `CampaignOrchestrator` without looping.
#[test]
fn test_sh2_191_angela_graveyard_repeat_talk_haka_agl_h_and_i_retail_assets() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return;
    }

    let cases = [
        ("haka_agl_h", 38u16, 60054i32, 54usize, 3284u16, 457usize),
        ("haka_agl_i", 41u16, 60055i32, 55usize, 3741u16, 272usize),
    ];

    for (id, expected_msg_start, expected_stream, expected_afs, expected_off, expected_frames) in
        cases
    {
        let info = sh2_core::DRAMA_DEMO_CATALOG
            .iter()
            .find(|d| d.id == id)
            .unwrap_or_else(|| panic!("{id} must exist in DRAMA_DEMO_CATALOG"));
        assert_eq!(info.demo_no, 4);
        assert_eq!(info.msg_start, expected_msg_start);
        assert_eq!(info.stream_no, expected_stream);
        assert_eq!(info.voice_afs_index(), Some(expected_afs));
        assert_eq!(info.stream_start, 0);
        assert_eq!(info.anm_frame_offset, expected_off);

        let dds_bytes = std::fs::read(resolver.game_root.join(info.dds_path))
            .unwrap_or_else(|e| panic!("read {}: {e}", info.dds_path));
        let script = sh2_assets::formats::demo::parse_demo_dds(&dds_bytes)
            .unwrap_or_else(|e| panic!("parse {}: {e}", info.dds_path));
        assert_eq!(script.header.total_frames as usize, expected_frames);

        let mes_bytes = std::fs::read(resolver.game_root.join(info.mes_path))
            .expect("read stage_forest_msg_e.mes");
        let mes_table =
            sh2_assets::formats::mes::parse_mes(&mes_bytes).expect("parse stage_forest_msg_e.mes");
        assert!(
            mes_table.get_text(usize::from(info.msg_start)).is_some(),
            "{id} msg_start={} must exist in stage_forest_msg_e.mes",
            info.msg_start
        );

        let wav = crate::assets::load_cutscene_voice_wav_for_source(
            expected_afs,
            sh2_core::AudioPackSource::EnhancedLossless,
        )
        .unwrap_or_else(|| panic!("voice.afs[{expected_afs}] must decode for {id}"));
        assert_eq!(wav.bits_per_sample, 16);
        let peak = wav
            .data
            .as_chunks::<2>()
            .0
            .iter()
            .step_by(64)
            .map(|c| i16::from_le_bytes(*c).unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(
            peak > 4_000,
            "voice.afs[{expected_afs}] ({id}) must contain non-silent speech PCM (peak={peak})"
        );

        let mut player = sh2_core::DramaDemoPlayer::new(*info, script, false);
        let jms = player
            .actors
            .iter()
            .find(|a| a.actor_name == "hhh_jms_pos")
            .unwrap_or_else(|| panic!("{id} must include hhh_jms_pos"));
        let agl = player
            .actors
            .iter()
            .find(|a| a.actor_name == "agl_pos")
            .unwrap_or_else(|| panic!("{id} must include agl_pos"));
        assert_eq!(jms.anm_frame_offset, expected_off);
        assert_eq!(agl.cls_frame_offset, expected_off);
        assert_eq!(agl.anm_frame_offset, expected_off + 156);

        // `stream_start = 0` must trigger voice playback cleanly on early frames.
        let mut voice_triggered = false;
        for _ in 0..5 {
            player.tick(1.0 / 30.0, true);
            if player.pending_voice_trigger {
                voice_triggered = true;
                break;
            }
        }
        assert!(voice_triggered, "{id} must fire pending_voice_trigger");
    }
}
