//! Layer 3 golden-vector tests & Layer 4 headless retail-asset visual snapshot tests
//! for `SH2-82` (Save/Load `MC/mc_menu.c`, Game Over `GameoverMain`, 10-Star Results
//! `ResultMain` & Clear Data NG+, Low-Health Red Cross `0x00476280`, and 3D Item Orbit toggle).

use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::formats::mes::parse_mes;
use sh2_assets::{AssetPathResolver, TextureLoader};
use sh2_core::campaign::{
    CampaignSaveSnapshot, ClearDataProfile, SaveSlotEntry, SaveSlotKind, SaveSlotRepository,
    SAVE_FORMAT_VERSION,
};
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::endings::{ClearEndKind, PlayingInfo, RankingRuleset};
use sh2_core::events::ClearData;
use sh2_core::scenario::ScenarioKind;
use sh2_ui::atlas_font::AtlasFont;
use sh2_ui::font::TextRasterizer;
use sh2_ui::gameover::{draw_gameover_screen, GameOverPhase, GameOverScreen, GAMEOVER_TEX_PATH};
use sh2_ui::inventory::{InventoryFocus, InventoryScreen, RedCrossIconState, RED_CROSS_ICON_PATH};
use sh2_ui::menu::MenuNavAction;
use sh2_ui::results::{draw_results_screen, ResultsScreen};
use sh2_ui::save_load::{
    draw_save_load_screen, mc_get_blink_alpha, SaveAddInfoMode, SaveLoadAction, SaveLoadConfig,
    SaveLoadMode, SaveLoadScreen, SaveLoadStep, SAVEBG_TBN2_PATH, TAG_CURVE_1, TAG_CURVE_2,
};

fn save_png(path: &str, rast: &TextRasterizer) {
    let img =
        image::RgbaImage::from_raw(rast.width as u32, rast.height as u32, rast.pixels.clone())
            .expect("valid rgba buffer");
    img.save(path).expect("save png");
}

fn sample_entry(
    card: usize,
    file: usize,
    save_point_id: u16,
    secs: f64,
    corrupted: bool,
) -> SaveSlotEntry {
    SaveSlotEntry {
        card_index: card,
        file_index: file,
        kind: SaveSlotKind::Manual,
        is_corrupted: corrupted,
        snapshot: CampaignSaveSnapshot {
            scenario: ScenarioKind::LetterFromSilentHeaven,
            stage_id: 4,
            room_id: "ap01".to_string(),
            player_position: [0.0, 0.0, 0.0],
            player_yaw: 0.0,
            action_difficulty: ActionDifficulty::Normal,
            riddle_difficulty: RiddleDifficulty::Normal,
            play_time_secs: secs,
            save_count: 2,
            save_version: SAVE_FORMAT_VERSION,
            save_point_name_id: save_point_id,
            secondary_timer_secs: 120.0,
            player_hp: 100.0,
            player_max_hp: 100.0,
            clear_data: ClearData {
                cleared_levels: ClearData::ALL_BASE_RIDDLES_CLEARED,
                endings_seen: ClearData::ENDING_LEAVE | ClearData::ENDING_IN_WATER,
                clear_count: 3,
            },
            previous_total_rank: 85,
            spray_pow: 1,
            event_flags_words: Vec::new(),
            inventory_items: Vec::new(),
            thumbnail_rgba: None,
        },
    }
}

#[test]
fn test_mc_menu_blink_curves_and_navigation() {
    // `mcGetBlinkAlpha` (`0x00245440`): 0..0.5 -> 0..32, 0.5..1.0 -> 32..0
    assert_eq!(mc_get_blink_alpha(0.0), 0);
    assert_eq!(mc_get_blink_alpha(0.25), 16);
    assert_eq!(mc_get_blink_alpha(0.5), 32);
    assert_eq!(mc_get_blink_alpha(0.75), 16);
    assert_eq!(mc_get_blink_alpha(1.0), 0);

    // Exact `tag_curve1` & `tag_curve2` (`0x0034f270` / `0x0034f280`)
    assert_eq!(TAG_CURVE_1, [3, 18, 5, 10, 8, 5, 13, 1, 16, 0]);
    assert_eq!(TAG_CURVE_2, [0, 8, 1, 3, 3, 1, 8, 0]);

    let mut screen = SaveLoadScreen::new(
        SaveLoadMode::Save,
        ScenarioKind::LetterFromSilentHeaven,
        SaveLoadConfig::default(),
    );
    screen.set_slots(vec![
        sample_entry(0, 0, 2, 1845.0, false),
        sample_entry(0, 1, 6, 4210.0, false),
        sample_entry(1, 0, 12, 9100.0, true),
    ]);

    assert_eq!(screen.active_card, 0);
    assert_eq!(screen.total_selectable_rows(), 3); // 2 existing + 1 New Save

    // Confirm on existing slot -> ConfirmOverwrite -> Yes -> WriteSlot
    assert_eq!(
        screen.handle_nav(MenuNavAction::Confirm),
        SaveLoadAction::None
    );
    assert_eq!(screen.step, SaveLoadStep::ConfirmOverwrite);
    assert_eq!(screen.handle_nav(MenuNavAction::Left), SaveLoadAction::None);
    assert!(matches!(
        screen.handle_nav(MenuNavAction::Confirm),
        SaveLoadAction::WriteSlot {
            card_index: 0,
            file_index: 0,
            ..
        }
    ));

    // Strict PS2 mode blocks export
    assert_eq!(screen.request_export(false), SaveLoadAction::None);

    // Native unlimited mode enables export
    screen.config.unlimited_slots = true;
    screen.step = SaveLoadStep::SelectSlot;
    assert!(matches!(
        screen.request_export(true),
        SaveLoadAction::ExportBinary {
            card_index: 0,
            file_index: 0
        }
    ));

    // Cycle `SaveAddInfoMode` (`mcSelectData` `0x00244a50`)
    assert_eq!(screen.add_info_mode, SaveAddInfoMode::PlayTime);
    screen.cycle_add_info();
    assert_eq!(screen.add_info_mode, SaveAddInfoMode::SecondaryTime);
    screen.cycle_add_info();
    assert_eq!(screen.add_info_mode, SaveAddInfoMode::EndingsCleared);
}

#[test]
fn test_gameover_state_machine_exact_ps2_frames() {
    let mut go = GameOverScreen::new();
    assert_eq!(go.phase, GameOverPhase::DelayBeforeFadeIn);
    assert_eq!(go.ps2_alpha, 0);

    for _ in 0..31 {
        go.step_30hz_frame();
        assert_eq!(go.phase, GameOverPhase::DelayBeforeFadeIn);
    }
    go.step_30hz_frame();
    assert_eq!(go.phase, GameOverPhase::FadeIn);

    // 24 frames * 4 = 96 (0x60) -> Hold
    for _ in 0..24 {
        go.step_30hz_frame();
    }
    assert_eq!(go.phase, GameOverPhase::Hold);
    assert_eq!(go.ps2_alpha, 0x60);

    // 64 frames hold -> FadeOut
    for _ in 0..64 {
        go.step_30hz_frame();
    }
    assert_eq!(go.phase, GameOverPhase::FadeOut);

    // 12 frames * 8 = 96 -> Done
    for _ in 0..12 {
        go.step_30hz_frame();
    }
    assert_eq!(go.phase, GameOverPhase::Done);
    assert!(go.is_done());
}

#[test]
fn test_results_10_star_and_clear_data_ngplus() {
    let info = PlayingInfo {
        clear_end_kind: ClearEndKind(
            ClearEndKind::LEAVE
                | ClearEndKind::MARIA
                | ClearEndKind::IN_WATER
                | ClearEndKind::REBIRTH
                | ClearEndKind::DOG,
        ),
        battle_level: 3,       // Hard -> +5
        riddle_level: 3,       // Extra -> +5
        hidden_item_get: 0xff, // +8 points
        item_get: 140,         // 140/20 = 7 -> 15 points
        savecount: 1,          // +5 points
        time: 6500.0,          // < 120m -> +10 points
        walk_distance: 500.0 * 1000.0 * 2.5,
        run_distance: 500.0 * 1000.0 * 6.0,
        kill_by_shot: 150,     // +15 points
        kill_by_fight: 150,    // +15 points
        boat_clear_time: 55.0, // <= 60s -> +5 points
        boat_max_speed: 500.0 * 3.5,
        jms_damage_total: 120.0, // <= 200 -> +5 points
        ..Default::default()
    };

    let screen = ResultsScreen::from_playing_info(
        &info,
        ActionDifficulty::Hard,
        RiddleDifficulty::Extra,
        RankingRuleset::Ps2Original,
    );
    assert_eq!(screen.breakdown.total_points, 100);
    assert_eq!(screen.breakdown.big_stars(), 10);
    assert_eq!(screen.breakdown.small_stars(), 0);
    assert_eq!(screen.breakdown.spray_pow, 2); // Green Hyper Spray

    assert_eq!(screen.rows[6].value, "2.50km");
    assert_eq!(screen.rows[7].value, "6.00km");
    assert_eq!(screen.rows[12].value, "3.50m/s");

    // Clear Data NG+ persistence (`ClearDataProfile`)
    let mut profile = ClearDataProfile::default();
    profile.record_completion(
        ClearData::ENDING_LEAVE,
        RiddleDifficulty::Easy,
        ActionDifficulty::Easy,
        screen.breakdown.total_points,
        screen.breakdown.spray_pow,
    );
    profile.clear_data.record_ending(
        ClearData::ENDING_IN_WATER,
        RiddleDifficulty::Normal,
        ActionDifficulty::Normal,
    );
    profile.clear_data.record_ending(
        ClearData::ENDING_MARIA,
        RiddleDifficulty::Hard,
        ActionDifficulty::Hard,
    );
    assert!(profile.clear_data.is_extra_riddle_unlocked());
    assert_eq!(profile.spray_pow, 2);
}

#[test]
fn test_red_cross_and_free_item_orbit_toggle() {
    let mut rc = RedCrossIconState::new();
    rc.tick(1.0 / 30.0, 80.0, 100.0);
    assert_eq!(rc.alpha_u8(), 0);

    rc.tick(1.0 / 30.0, 20.0, 100.0);
    assert!(rc.alpha_u8() > 0);
    assert!((rc.pulse - 0.97).abs() < 1e-4);

    // Free 3D item orbit toggle gating
    let mut inv = InventoryScreen::new();
    inv.is_open = true;
    inv.focus = InventoryFocus::ExamineOverlay;
    assert!(!inv.enable_free_item_orbit);
    inv.handle_nav(
        MenuNavAction::Left,
        &sh2_core::InventoryManager::new(),
        &Default::default(),
    );
    assert_eq!(inv.examine_yaw_rad, 0.0); // Strict PS2 mode ignores orbit

    inv.enable_free_item_orbit = true;
    inv.handle_nav(
        MenuNavAction::Left,
        &sh2_core::InventoryManager::new(),
        &Default::default(),
    );
    assert!((inv.examine_yaw_rad + 0.25).abs() < 1e-5);
}

#[test]
fn test_retail_assets_and_layer4_png_snapshots() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.is_dir() {
        return;
    }
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    let font = AtlasFont::shared(None);

    // Verify retail `.mes` & `.mdl` & `.tex` / `.tbn2` on disk
    let result_mes =
        parse_mes(&std::fs::read(resolver.data_dir.join("etc/message/result_msg_e.mes")).unwrap())
            .unwrap();
    assert!(result_mes.messages.len() >= 24);
    let mc_mes =
        parse_mes(&std::fs::read(resolver.data_dir.join("etc/message/m_card_msg_e.mes")).unwrap())
            .unwrap();
    assert!(mc_mes.messages.len() >= 60);
    let handgun_mdl =
        parse_mdl(&std::fs::read(resolver.data_dir.join("chr/item/x_handgun.mdl")).unwrap())
            .unwrap();
    assert!(handgun_mdl.bone_count > 0);

    let savebg = loader.load_texture(SAVEBG_TBN2_PATH).expect("savebg.tbn2");
    let gameover_tex = loader
        .load_texture(GAMEOVER_TEX_PATH)
        .expect("gameover1.tex");
    let red_cross_tex = loader
        .load_texture(RED_CROSS_ICON_PATH)
        .expect("RedCrossIcon.png");

    // SaveLoadRepository roundtrip in temp dir
    let repo_dir = std::env::temp_dir().join("sh2_82_repo_test");
    let _ = std::fs::remove_dir_all(&repo_dir);
    let repo = SaveSlotRepository::new(&repo_dir);
    let e0 = sample_entry(0, 0, 2, 1845.0, false);
    repo.write_slot(0, 0, SaveSlotKind::Manual, &e0.snapshot)
        .unwrap();
    assert_eq!(repo.scan_slots(false, false).len(), 1);
    let _ = std::fs::remove_dir_all(&repo_dir);

    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok();

    // 1. Save/Load screen (Vanilla PS2 mode)
    let mut sl = SaveLoadScreen::new(
        SaveLoadMode::Save,
        ScenarioKind::LetterFromSilentHeaven,
        SaveLoadConfig::default(),
    );
    sl.set_slots(vec![
        sample_entry(0, 0, 2, 1845.0, false),
        sample_entry(0, 1, 6, 4210.0, false),
        sample_entry(0, 2, 12, 9100.0, false),
    ]);
    let mut rast_sl = TextRasterizer::new(960, 720);
    draw_save_load_screen(
        &sl,
        Some((&savebg.rgba, savebg.width, savebg.height)),
        font,
        &mut rast_sl,
    );
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/save_load_ps2_vanilla.png"), &rast_sl);
    }

    // 2. Game Over screen (Peak Hold)
    let go = GameOverScreen::at_peak_hold();
    let mut rast_go = TextRasterizer::new(960, 720);
    draw_gameover_screen(
        &go,
        Some((&gameover_tex.rgba, gameover_tex.width, gameover_tex.height)),
        font,
        &mut rast_go,
    );
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/gameover_ps2_vanilla.png"), &rast_go);
    }

    // 3. 10-Star Results screen
    let info = PlayingInfo {
        clear_end_kind: ClearEndKind(
            ClearEndKind::LEAVE
                | ClearEndKind::MARIA
                | ClearEndKind::IN_WATER
                | ClearEndKind::REBIRTH
                | ClearEndKind::DOG,
        ),
        battle_level: 3,
        riddle_level: 3,
        hidden_item_get: 0x07,
        item_get: 100,
        savecount: 1,
        time: 6500.0,
        walk_distance: 500.0 * 1000.0 * 2.5,
        run_distance: 500.0 * 1000.0 * 6.0,
        kill_by_shot: 150,
        kill_by_fight: 150,
        boat_clear_time: 55.0,
        boat_max_speed: 500.0 * 3.5,
        jms_damage_total: 120.0,
        ..Default::default()
    };
    let res = ResultsScreen::from_playing_info(
        &info,
        ActionDifficulty::Hard,
        RiddleDifficulty::Extra,
        RankingRuleset::Ps2Original,
    );
    let mut rast_res = TextRasterizer::new(960, 720);
    draw_results_screen(&res, font, &mut rast_res);
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/results_10_star_ps2.png"), &rast_res);
    }

    // 4. Low-Health Red Cross Icon
    let mut rc = RedCrossIconState::new();
    rc.tick(1.0 / 30.0, 20.0, 100.0);
    let mut rast_rc = TextRasterizer::new(960, 720);
    rast_rc.fill_rect(0, 0, 960, 720, [12, 14, 16, 255]);
    rc.draw(
        Some((
            &red_cross_tex.rgba,
            red_cross_tex.width,
            red_cross_tex.height,
        )),
        &mut rast_rc,
    );
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/red_cross_low_hp.png"), &rast_rc);
    }
}
