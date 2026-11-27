//! Deterministic unit tests for SH2-177 UI dirty-checking fingerprints.

use sh2_core::config::{ActionDifficulty, AppConfig, RiddleDifficulty};
use sh2_core::endings::{PlayingInfo, RankingRuleset};
use sh2_core::events::{EventFlags, PuzzleState};
use sh2_core::InventoryManager;
use sh2_ui::inventory::{inventory_render_fingerprint, InventoryPlayerSnapshot, InventoryScreen};
use sh2_ui::map::{
    paper_map_render_fingerprint, PaperMapPlayerState, PaperMapTextures, PaperMapViewer,
};
use sh2_ui::memo::{memo_render_fingerprint, MemoReader, MemoRenderContext, MemoRenderTextures};
use sh2_ui::options::{options_render_fingerprint, OptionsMenu, OptionsSubScreen};
use sh2_ui::pause::{pause_render_fingerprint, PauseMenu};
use sh2_ui::results::{results_render_fingerprint, ResultsScreen};

#[test]
fn options_fingerprint_stable_when_static_and_invalidates_on_changes() {
    let mut menu = OptionsMenu::new(AppConfig::default());
    let vp = (1920, 1440);
    let fp0 = options_render_fingerprint(&menu, vp);
    let fp0_repeat = options_render_fingerprint(&menu, vp);
    assert_eq!(
        fp0, fp0_repeat,
        "static options screen must keep identical fingerprint"
    );

    // Viewport change invalidates
    assert_ne!(fp0, options_render_fingerprint(&menu, (1280, 960)));

    // Cursor move invalidates
    menu.selected_index = 1;
    let fp1 = options_render_fingerprint(&menu, vp);
    assert_ne!(fp0, fp1);

    // Brightness subscreen animation frame invalidates (`anim_frame % 40`)
    menu.open_sub_screen(OptionsSubScreen::BrightnessCalibration);
    let fp_b0 = options_render_fingerprint(&menu, vp);
    menu.anim_frame = 1;
    let fp_b1 = options_render_fingerprint(&menu, vp);
    assert_ne!(fp_b0, fp_b1);
}

#[test]
fn pause_fingerprint_stable_and_invalidates_on_cursor_or_viewport() {
    let mut pause = PauseMenu::new();
    pause.open(true, "Observation Deck", 120.0);
    let fp0 = pause_render_fingerprint(&pause, false, (1920, 1440));
    assert_eq!(fp0, pause_render_fingerprint(&pause, false, (1920, 1440)));
    assert_ne!(fp0, pause_render_fingerprint(&pause, true, (1920, 1440)));
    assert_ne!(fp0, pause_render_fingerprint(&pause, false, (1280, 720)));

    pause.selected_index = 2;
    assert_ne!(fp0, pause_render_fingerprint(&pause, false, (1920, 1440)));
}

#[test]
fn paper_map_fingerprint_tracks_zoom_pan_player_and_textures() {
    let mut viewer = PaperMapViewer {
        is_open: true,
        current_map_id: 1,
        ..Default::default()
    };
    let flags = EventFlags::new();
    let mut player = PaperMapPlayerState {
        room_id: 1,
        pos_ps2: (10.0, -20.0),
        yaw_rad: 0.5,
    };
    let tex_buf = vec![255u8; 16];
    let tex_none = PaperMapTextures::default();
    let tex_some = PaperMapTextures {
        sheet_rgba: Some((&tex_buf, 2, 2)),
        mark_atlas_rgba: None,
        cursor_rgba: None,
    };

    let fp0 = paper_map_render_fingerprint(&viewer, &flags, player, tex_none, (1920, 1080));
    assert_eq!(
        fp0,
        paper_map_render_fingerprint(&viewer, &flags, player, tex_none, (1920, 1080))
    );
    assert_ne!(
        fp0,
        paper_map_render_fingerprint(&viewer, &flags, player, tex_some, (1920, 1080))
    );

    viewer.zoom_t = 0.25;
    assert_ne!(
        fp0,
        paper_map_render_fingerprint(&viewer, &flags, player, tex_none, (1920, 1080))
    );

    viewer.zoom_t = 0.0;
    player.yaw_rad = 1.25;
    assert_ne!(
        fp0,
        paper_map_render_fingerprint(&viewer, &flags, player, tex_none, (1920, 1080))
    );
}

#[test]
fn memo_and_results_fingerprints_are_stable_and_reactive() {
    let flags = EventFlags::new();
    let puzzle = PuzzleState::new();
    let mut reader = MemoReader::new();
    reader.is_open = true;
    let ctx = Some(MemoRenderContext {
        flags: &flags,
        puzzle: &puzzle,
        riddle: RiddleDifficulty::Normal,
    });
    let tex = MemoRenderTextures::default();

    let m0 = memo_render_fingerprint(&reader, false, tex, ctx, (1920, 1080));
    assert_eq!(
        m0,
        memo_render_fingerprint(&reader, false, tex, ctx, (1920, 1080))
    );
    reader.dim_alpha = 0.5;
    assert_ne!(
        m0,
        memo_render_fingerprint(&reader, false, tex, ctx, (1920, 1080))
    );

    let info = PlayingInfo::default();
    let res = ResultsScreen::from_playing_info(
        &info,
        ActionDifficulty::Normal,
        RiddleDifficulty::Normal,
        RankingRuleset::PcDirectorCut,
    );
    let r0 = results_render_fingerprint(&res, (1920, 1080));
    assert_eq!(r0, results_render_fingerprint(&res, (1920, 1080)));
    assert_ne!(r0, results_render_fingerprint(&res, (1280, 720)));
}

#[test]
fn inventory_fingerprint_invalidates_on_animation_and_cursor_changes() {
    let mut inv = InventoryManager::new();
    inv.item_get_now(1);
    inv.item_get_now(2);
    let mut screen = InventoryScreen::new();
    screen.open(&inv);
    let snap = InventoryPlayerSnapshot::default();

    let fp0 = inventory_render_fingerprint(&screen, &inv, &snap, None, None, (1920, 1440));
    assert_eq!(
        fp0,
        inventory_render_fingerprint(&screen, &inv, &snap, None, None, (1920, 1440))
    );

    screen.tab_pulse = 0.4;
    let fp_pulse = inventory_render_fingerprint(&screen, &inv, &snap, None, None, (1920, 1440));
    assert_ne!(fp0, fp_pulse);

    screen.tab_pulse = 0.0;
    screen.selected_index = 1;
    let fp_sel = inventory_render_fingerprint(&screen, &inv, &snap, None, None, (1920, 1440));
    assert_ne!(fp0, fp_sel);
}
