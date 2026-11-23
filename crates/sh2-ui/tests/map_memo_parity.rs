//! Layer 3 golden-vector tests & Layer 4 headless retail-asset visual snapshot tests
//! for `SH2-138` (1:1 Paper Map `Event/chizu.c` & Memo Reader `Event/memo.c`).

use sh2_assets::formats::mes::parse_mes;
use sh2_assets::{AssetPathResolver, TextureLoader};
use sh2_core::config::RiddleDifficulty;
use sh2_core::events::{EventFlags, PuzzleState, PZ_BOX_CODE, PZ_SAFE_CODE};
use sh2_ui::atlas_font::AtlasFont;
use sh2_ui::font::TextRasterizer;
use sh2_ui::map::{
    active_markers_for_map, chizu_arrow_yaw_offset, chizu_connect_check,
    chizu_current_position_check, chizu_select, chizu_should_draw_player_arrow, draw_paper_map,
    MapDirection, PaperMapPlayerState, PaperMapTextures, PaperMapViewer, CHIZU_CURSOR_TEX_PATH,
    CHIZU_MARKERS, CHZ_CRT_BLOCK_COUNT,
};
use sh2_ui::memo::{
    apply_memo_dynamic_codes, build_memo_layer_quads, draw_memo_reader, format_memo_keypad_code,
    format_memo_wallet_codes, resolve_memo_textures, MemoDisplayMode, MemoPhase, MemoReader,
    MemoRenderContext, MemoRenderTextures, DEFAULT_MEMO_BACKDROP_TEX, MEMO_CATALOG, STATUE_BOX_TEX,
};
use sh2_ui::menu::MenuNavAction;

#[test]
fn test_chizu_select_multifloor_and_upgrades() {
    let mut flags = EventFlags::new();
    // Wood Side stairwell 0x0f: 1F / 2F / 3F thresholds
    assert_eq!(chizu_select(0x0f, [0.0, -500.0, 0.0], &flags), 4);
    assert_eq!(chizu_select(0x0f, [0.0, -1500.0, 0.0], &flags), 5);
    assert_eq!(chizu_select(0x0f, [0.0, -3000.0, 0.0], &flags), 6);

    // Combined Apartment upgrade (flags 25 & 26): 4..=6 -> 9..=11, 7..=8 -> 9..=10
    flags.set(25, true);
    flags.set(26, true);
    assert_eq!(chizu_select(0x0f, [0.0, -500.0, 0.0], &flags), 9);
    assert_eq!(chizu_select(0x0f, [0.0, -1500.0, 0.0], &flags), 10);
    assert_eq!(chizu_select(0x0f, [0.0, -3000.0, 0.0], &flags), 11);
    assert_eq!(chizu_select(0x20, [0.0, -500.0, -20000.0], &flags), 9); // 7 + 2
    assert_eq!(chizu_select(0x20, [0.0, -1500.0, -20000.0], &flags), 10); // 8 + 2

    // Brookhaven Hospital elevator 0x2a & Otherworld 0x46
    flags.set(17, true);
    assert_eq!(chizu_select(0x2a, [0.0, 0.0, 0.0], &flags), 12);
    assert_eq!(chizu_select(0x46, [0.0, 0.0, 0.0], &flags), 14);
    flags.set(18, true);
    assert_eq!(chizu_select(0x2a, [0.0, 0.0, 0.0], &flags), 13);
    assert_eq!(chizu_select(0x46, [0.0, 0.0, 0.0], &flags), 15);

    // Hotel stairwell 0x92 + Employee upgrade (flag 31): 21/22 -> 29/30
    assert_eq!(chizu_select(0x92, [0.0, -500.0, 0.0], &flags), 21);
    assert_eq!(chizu_select(0x92, [0.0, -2000.0, 0.0], &flags), 22);
    flags.set(31, true);
    assert_eq!(chizu_select(0x92, [0.0, -500.0, 0.0], &flags), 29);
    assert_eq!(chizu_select(0x92, [0.0, -2000.0, 0.0], &flags), 30);

    // Otherworld Hotel stairwell 0xab (retail PC 32/33)
    assert_eq!(chizu_select(0xab, [0.0, -500.0, 0.0], &flags), 32);
    assert_eq!(chizu_select(0xab, [0.0, -1000.0, 0.0], &flags), 33);
}

#[test]
fn test_chizu_current_position_check_table_and_rotations() {
    assert_eq!(CHZ_CRT_BLOCK_COUNT, 214);
    let flags = EventFlags::new();

    // Room 1 (Restroom): static pin
    let (x1, y1) = chizu_current_position_check(1, 1, [9999.0, 0.0, 9999.0], &flags);
    assert!((x1 - 218.4004).abs() < 1e-2);
    assert!((y1 - 170.8404).abs() < 1e-2);

    // Toluca Prison (room 0x66): swapped axes (ry, -rx) and -pi/2 arrow yaw offset
    let (px, py) = chizu_current_position_check(0x66, 16, [1000.0, 0.0, 2000.0], &flags);
    assert!(px.is_finite() && py.is_finite());
    assert!((chizu_arrow_yaw_offset(16, 0x66) + std::f32::consts::FRAC_PI_2).abs() < 1e-5);

    // Labyrinth (room 0x78): +pi/2 arrow yaw offset
    assert!((chizu_arrow_yaw_offset(18, 0x78) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);

    // Map 1 arrow visibility guard: visible in rooms 1..=3, hidden in room 4
    assert!(chizu_should_draw_player_arrow(1, 1, 1, &flags));
    assert!(!chizu_should_draw_player_arrow(1, 1, 4, &flags));
}

#[test]
fn test_memo_all_five_overlays_and_suppression() {
    let mut flags = EventFlags::new();
    let mut puzzle = PuzzleState::default();

    // 1. SafeLock (index 0x12): suppressed without flag 607, drawn with flag 607 + 5-bit dial
    assert_eq!(
        resolve_memo_textures(&MEMO_CATALOG[0x12], &flags),
        (DEFAULT_MEMO_BACKDROP_TEX, None)
    );
    assert!(build_memo_layer_quads(0x12, &flags, &puzzle).is_empty());
    flags.set(607, true);
    flags.set(0x67, true); // dial = 1
    let safe_quads = build_memo_layer_quads(0x12, &flags, &puzzle);
    assert_eq!(safe_quads.len(), 1);
    assert_eq!(safe_quads[0].uv_px, [0.0, 0.0, 127.0, 127.0]);

    // 2. Guruguru (index 0x20): 4 tumbler quads from puzzle.raw()[8..12]
    puzzle.raw_mut()[8..12].copy_from_slice(&[0, 3, 5, 8]);
    let guru_quads = build_memo_layer_quads(0x20, &flags, &puzzle);
    assert_eq!(guru_quads.len(), 4);

    // 3. Hair (index 0x21): suppressed without flag 171
    assert_eq!(
        resolve_memo_textures(&MEMO_CATALOG[0x21], &flags),
        (DEFAULT_MEMO_BACKDROP_TEX, None)
    );
    flags.set(171, true);
    let hair_quads = build_memo_layer_quads(0x21, &flags, &puzzle);
    assert_eq!(hair_quads.len(), 1);
    assert!((hair_quads[0].corners_ps2[0].0 - (-71.0)).abs() < 1e-4);
    assert!((hair_quads[0].corners_ps2[3].0 - 185.0).abs() < 1e-4);

    // 4. AngelRing (index 0x23): suppressed without flags 227/228, draws 1 or 2 rings
    assert_eq!(
        resolve_memo_textures(&MEMO_CATALOG[0x23], &flags),
        (DEFAULT_MEMO_BACKDROP_TEX, None)
    );
    flags.set(227, true);
    assert_eq!(build_memo_layer_quads(0x23, &flags, &puzzle).len(), 1);
    flags.set(228, true);
    assert_eq!(build_memo_layer_quads(0x23, &flags, &puzzle).len(), 2);

    // 5. Tablet (index 0x29): draws 0..3 tablets based on flags 301, 302, 303
    assert!(build_memo_layer_quads(0x29, &flags, &puzzle).is_empty());
    flags.set(301, true);
    flags.set(303, true);
    assert_eq!(build_memo_layer_quads(0x29, &flags, &puzzle).len(), 2);

    // Retail Statue Box swap (index 0x24): flag 257 clear + flag 260 set -> statuebox.tex
    flags.set(260, true);
    assert_eq!(
        resolve_memo_textures(&MEMO_CATALOG[0x24], &flags).0,
        STATUE_BOX_TEX
    );
}

#[test]
fn test_memo_wallet_and_keypad_dynamic_formatting() {
    let mut puzzle = PuzzleState::default();
    // Safe raw bytes [4, 11, 14, 19] -> dial values [5, 12, 15, 20]
    puzzle.raw_mut()[PZ_SAFE_CODE..PZ_SAFE_CODE + 4].copy_from_slice(&[4, 11, 14, 19]);
    assert_eq!(
        format_memo_wallet_codes(&puzzle, RiddleDifficulty::Normal),
        ["05", "12", "15", "20"]
    );
    assert_eq!(
        format_memo_wallet_codes(&puzzle, RiddleDifficulty::Hard),
        ["5", "X2", "VV5", "XX"]
    );
    assert_eq!(
        format_memo_wallet_codes(&puzzle, RiddleDifficulty::Extra),
        ["5", "c", "f", "k"]
    );

    // Keypad 4-nibble BCD code at PZ_BOX_CODE
    puzzle.write_u16(PZ_BOX_CODE, 0x7335);
    assert_eq!(format_memo_keypad_code(&puzzle), "7335");
    assert_eq!(
        apply_memo_dynamic_codes(0x1d, "Code: <e0>", &puzzle, RiddleDifficulty::Normal),
        "Code: 7335"
    );
}

#[test]
fn test_memo_3mode_dimming_13row_select_and_persistence() {
    let mut flags = EventFlags::new();
    // Collect 15 memos (including mode-2 entry 0x0d = flag 74)
    for entry in MEMO_CATALOG.iter().take(15) {
        flags.set(entry.flag_id as usize, true);
    }
    let mut reader = MemoReader::new();
    reader.open(&flags);
    assert_eq!(reader.entries.len(), 15);
    assert_eq!(reader.backdrop_brightness_ps2(), 0x60);

    // 13 visible rows with exact edge alpha fade 0x10 / 0x20 / 0x40 / 0x80
    let rows = reader.visible_list_rows();
    assert_eq!(rows.len(), 13);
    assert_eq!(rows[0].alpha_ps2, 0x10);
    assert_eq!(rows[1].alpha_ps2, 0x20);
    assert_eq!(rows[2].alpha_ps2, 0x40);
    assert_eq!(rows[6].alpha_ps2, 0x80);
    assert_eq!(rows[10].alpha_ps2, 0x40);
    assert_eq!(rows[11].alpha_ps2, 0x20);
    assert_eq!(rows[12].alpha_ps2, 0x10);

    // Navigate to mode-2 entry 0x0d (index 13 in entries)
    for _ in 0..13 {
        reader.handle_nav(MenuNavAction::Down, &flags, None);
    }
    assert_eq!(
        reader.selected_entry().unwrap().display_mode,
        MemoDisplayMode::ImageThenText
    );
    // Confirm opens ViewingImage at 0x80 brightness
    reader.handle_nav(MenuNavAction::Confirm, &flags, None);
    assert_eq!(reader.phase, MemoPhase::ViewingImage);
    assert_eq!(reader.backdrop_brightness_ps2(), 0x80);

    // Confirm transitions to ViewingText and dims 0x80 -> 0x40 at 1.0/s
    reader.handle_nav(MenuNavAction::Confirm, &flags, None);
    assert_eq!(reader.phase, MemoPhase::ViewingText);
    for _ in 0..30 {
        reader.tick(1.0 / 60.0);
    }
    assert!((reader.dim_alpha - 0.5).abs() < 1e-3);
    assert_eq!(reader.backdrop_brightness_ps2(), 0x60);
    for _ in 0..30 {
        reader.tick(1.0 / 60.0);
    }
    assert_eq!(reader.backdrop_brightness_ps2(), 0x40);

    // Close & reopen persists DAT_0122b313 (last_selected_catalog_index == 13)
    reader.close();
    assert_eq!(reader.last_selected_catalog_index, 13);
    reader.open(&flags);
    assert_eq!(reader.selected_entry().unwrap().index, 13);
}

#[test]
fn test_retail_map_and_memo_visual_snapshots() {
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    let Ok(outmape) = loader.load_texture("pic/map/outmape.tex") else {
        // Retail data directory not present in CI; skip visual render assertions cleanly.
        return;
    };
    let outmapmark = loader
        .load_texture("pic/map/outmapmark.tex")
        .expect("outmapmark.tex");
    let carsol = loader
        .load_texture(CHIZU_CURSOR_TEX_PATH)
        .expect("carsol.tex");
    assert!(carsol.width >= 256 && carsol.height >= 128 && carsol.width == carsol.height * 2);

    // 1. Render Paper Map (South Vale East, 1280x720 16:9 pillarboxed)
    let mut flags = EventFlags::new();
    flags.set(24, true); // Town map owned
    flags.set(25, true); // Wood Side owned -> Right connect arrow active
    for m in CHIZU_MARKERS.iter().filter(|m| m.map_id == 2).take(12) {
        flags.set(m.flag_id as usize, true);
    }
    assert!(active_markers_for_map(2, &flags).len() >= 12);
    assert!(chizu_connect_check(2, MapDirection::Right, &flags, 2, false).is_some());

    let mut viewer = PaperMapViewer::new();
    viewer.open_at_player(2, 2, Some((0.0, 0.0)), false, false);
    let mut rast = TextRasterizer::new(1280, 720);
    draw_paper_map(
        &viewer,
        &flags,
        PaperMapPlayerState {
            room_id: 4,
            pos_ps2: (-12.7, 66.0),
            yaw_rad: 0.4,
        },
        PaperMapTextures {
            sheet_rgba: Some((&outmape.rgba, outmape.width, outmape.height)),
            mark_atlas_rgba: Some((&outmapmark.rgba, outmapmark.width, outmapmark.height)),
            cursor_rgba: Some((&carsol.rgba, carsol.width, carsol.height)),
        },
        &mut rast,
    );
    // Verify pillarbox bar on far left is pure black
    assert_eq!(&rast.pixels[0..4], &[0, 0, 0, 255]);

    let out_dir = std::env::var("SH2_PREVIEW_OUT").ok();
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/paper_map_town_east.png"), &rast);
    }

    // 2. Render Memo Select 13-row list with retail memo_x_msg_e.mes & p_memo.tex
    let mes_bytes = std::fs::read(resolver.data_dir.join("etc/message/memo_x_msg_e.mes"))
        .expect("memo_x_msg_e");
    let mes_table = parse_mes(&mes_bytes).expect("parse memo_x_msg_e.mes");
    let p_memo = loader
        .load_texture(DEFAULT_MEMO_BACKDROP_TEX)
        .expect("p_memo.tex");
    let font = AtlasFont::shared(None);

    let mut mflags = EventFlags::new();
    for entry in MEMO_CATALOG.iter().take(20) {
        mflags.set(entry.flag_id as usize, true);
    }
    let mut memo = MemoReader::new();
    memo.open(&mflags);
    for _ in 0..5 {
        memo.handle_nav(MenuNavAction::Down, &mflags, Some(&mes_table));
    }
    let mut mrast = TextRasterizer::new(960, 720);
    draw_memo_reader(
        &memo,
        Some(&mes_table),
        MemoRenderTextures {
            backdrop_rgba: Some((&p_memo.rgba, p_memo.width, p_memo.height)),
            layer_rgba: None,
        },
        None,
        font,
        &mut mrast,
    );
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/memo_select_13rows.png"), &mrast);
    }

    // 3. Render Guruguru (Louise's Box) puzzle overlay
    let box_base = loader
        .load_texture("pic/hsp/p_boxnumber.tex")
        .expect("p_boxnumber.tex");
    let box_layer = loader
        .load_texture("pic/hsp/p_boxnumber_2.tex")
        .expect("p_boxnumber_2.tex");
    let mut gflags = EventFlags::new();
    gflags.set(MEMO_CATALOG[0x20].flag_id as usize, true);
    let mut gpuzzle = PuzzleState::default();
    gpuzzle.raw_mut()[8..12].copy_from_slice(&[1, 4, 6, 8]);
    let mut gmemo = MemoReader::new();
    gmemo.open(&gflags);
    gmemo.handle_nav(MenuNavAction::Confirm, &gflags, Some(&mes_table));
    let mut grast = TextRasterizer::new(960, 720);
    draw_memo_reader(
        &gmemo,
        Some(&mes_table),
        MemoRenderTextures {
            backdrop_rgba: Some((&box_base.rgba, box_base.width, box_base.height)),
            layer_rgba: Some((&box_layer.rgba, box_layer.width, box_layer.height)),
        },
        Some(MemoRenderContext {
            flags: &gflags,
            puzzle: &gpuzzle,
            riddle: RiddleDifficulty::Normal,
        }),
        font,
        &mut grast,
    );
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/memo_guruguru_overlay.png"), &grast);
    }

    // 4. Render Toluca Prison 3-Tablet puzzle overlay
    let tab_base = loader
        .load_texture("pic/dls/p_tablet.tex")
        .expect("p_tablet.tex");
    let tab_layer = loader
        .load_texture("pic/dls/p_tablet_plate.tex")
        .expect("p_tablet_plate.tex");
    let mut tflags = EventFlags::new();
    tflags.set(MEMO_CATALOG[0x29].flag_id as usize, true);
    tflags.set(301, true);
    tflags.set(302, true);
    tflags.set(303, true);
    let mut tmemo = MemoReader::new();
    tmemo.open(&tflags);
    tmemo.handle_nav(MenuNavAction::Confirm, &tflags, Some(&mes_table));
    let mut trast = TextRasterizer::new(960, 720);
    draw_memo_reader(
        &tmemo,
        Some(&mes_table),
        MemoRenderTextures {
            backdrop_rgba: Some((&tab_base.rgba, tab_base.width, tab_base.height)),
            layer_rgba: Some((&tab_layer.rgba, tab_layer.width, tab_layer.height)),
        },
        Some(MemoRenderContext {
            flags: &tflags,
            puzzle: &gpuzzle,
            riddle: RiddleDifficulty::Normal,
        }),
        font,
        &mut trast,
    );
    if let Some(ref dir) = out_dir {
        save_png(&format!("{dir}/memo_tablet_overlay.png"), &trast);
    }
}

fn save_png(path: &str, rast: &TextRasterizer) {
    let img =
        image::RgbaImage::from_raw(rast.width as u32, rast.height as u32, rast.pixels.clone())
            .expect("rgba image buffer");
    img.save(path).expect("save snapshot png");
}
