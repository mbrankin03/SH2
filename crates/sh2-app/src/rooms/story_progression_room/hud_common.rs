//! Shared 2D text truncation, cutaway `.tex` loading, and viewport overlay compositing helpers
//! for [`super::StoryProgressionRoomController`] (`SH2-65`).

use std::collections::HashMap;
use std::path::PathBuf;

use crate::state::AppState;
use sh2_assets::{CutawayTextureAsset, CutawayViewportTransform, DecodedTexture, TextureLoader};
use sh2_core::{OverlaySpriteQuad, PillarboxViewportLayout};

pub(crate) const YELLOW: [u8; 4] = [245, 210, 65, 255];
pub(crate) const CYAN: [u8; 4] = [105, 215, 235, 255];
pub(crate) const GREEN: [u8; 4] = [120, 230, 150, 255];
pub(crate) const WHITE: [u8; 4] = [235, 235, 235, 255];
pub(crate) const ORANGE: [u8; 4] = [255, 170, 80, 255];
pub(crate) const RED: [u8; 4] = [245, 95, 95, 255];
pub(crate) const MUTED: [u8; 4] = [165, 170, 185, 255];

/// Left column maximum character count (`560px / 8px = 70` max; capped at `66` so `x=24..552 < 592`).
pub(super) const LEFT_COL_MAX_CHARS: usize = 66;
/// Right panel maximum character count (`620px / 8px = 77` max; capped at `74` so `x=620..1212 < 1252`).
pub(super) const RIGHT_COL_MAX_CHARS: usize = 74;
/// Tab 3 translucent right HUD maximum character count (`515px / 8px = 64`; capped at `62` so `x=742..1238 < 1252`).
pub(super) const TAB3_HUD_MAX_CHARS: usize = 62;

/// Truncates `text` to at most `max_chars` ASCII characters so `TextRasterizer::draw_text`
/// never spills across column boundaries (`x=592` on left, `x=1252` on right).
pub(super) fn clip_str(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        text.chars().take(max_chars).collect()
    }
}

/// Draws a single `scale=1` (`8x8`) text line strictly truncated to `max_chars`.
pub(crate) fn draw_text_clipped(
    state: &mut AppState,
    x: usize,
    y: usize,
    max_chars: usize,
    color: [u8; 4],
    text: &str,
) {
    let clipped = clip_str(text, max_chars);
    state
        .menu_rasterizer
        .draw_text(&clipped, x, y, 1, color, true);
}

/// Draws a left-column (`x=24..592`) text line strictly capped at [`LEFT_COL_MAX_CHARS`].
pub(crate) fn txt_left(state: &mut AppState, x: usize, y: usize, color: [u8; 4], text: &str) {
    let avail = 588usize.saturating_sub(x) / 8;
    draw_text_clipped(state, x, y, avail.min(LEFT_COL_MAX_CHARS), color, text);
}

/// Draws a right-column (`x=612..1256`) text line strictly capped at [`RIGHT_COL_MAX_CHARS`].
pub(crate) fn txt_right(state: &mut AppState, x: usize, y: usize, color: [u8; 4], text: &str) {
    let avail = 1252usize.saturating_sub(x) / 8;
    draw_text_clipped(state, x, y, avail.min(RIGHT_COL_MAX_CHARS), color, text);
}

/// Draws a Tab 3 right-HUD (`x=730..1265`) text line strictly capped at [`TAB3_HUD_MAX_CHARS`].
pub(crate) fn txt_tab3(state: &mut AppState, x: usize, y: usize, color: [u8; 4], text: &str) {
    let avail = 1252usize.saturating_sub(x) / 8;
    draw_text_clipped(state, x, y, avail.min(TAB3_HUD_MAX_CHARS), color, text);
}

/// Draws a styled card panel with background fill and 1px or 2px border.
pub(crate) fn draw_card(
    state: &mut AppState,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    bg: [u8; 4],
    border: [u8; 4],
) {
    state.menu_rasterizer.fill_rect(x, y, w, h, bg);
    state
        .menu_rasterizer
        .draw_rect_outline(x, y, w, h, 1, border);
}

/// Draws a multi-line stack of left-column (`x=24..588`) text rows separated by `step_y` px.
pub(crate) fn txt_left_lines(
    state: &mut AppState,
    x: usize,
    start_y: usize,
    step_y: usize,
    rows: &[([u8; 4], &str)],
) {
    for (i, &(col, line)) in rows.iter().enumerate() {
        txt_left(state, x, start_y + i * step_y, col, line);
    }
}

/// Draws a horizontal progress bar inside `(x, y, w, h)` with fill ratio `ratio` (`0.0..=1.0`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_progress_bar(
    state: &mut AppState,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    ratio: f32,
    fill_color: [u8; 4],
    border_color: [u8; 4],
) {
    state
        .menu_rasterizer
        .fill_rect(x, y, w, h, [18, 22, 32, 240]);
    let fill_w = ((w as f32) * ratio.clamp(0.0, 1.0)).round() as usize;
    if fill_w > 0 {
        state
            .menu_rasterizer
            .fill_rect(x, y, fill_w.min(w), h, fill_color);
    }
    state
        .menu_rasterizer
        .draw_rect_outline(x, y, w, h, 1, border_color);
}

/// Loads a retail cutaway still or sprite sheet `.tex` from `sh2e/pic/` or `data/pic/`.
pub(super) fn load_cutaway_tex(
    pic_or_sheet: &str,
    prefer_sh2ee: bool,
) -> Option<CutawayTextureAsset> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.clone(),
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    for base in &base_roots {
        let loader = TextureLoader::new(base.join("data"), Some(base.join("sh2e")));
        if let Ok(tex) = loader.load_cutaway_texture(pic_or_sheet, prefer_sh2ee) {
            return Some(tex);
        }
    }
    None
}

/// Ensures `pic_or_sheet` is present in `cache` and returns a reference if available.
pub(super) fn ensure_cutaway_cached(
    cache: &mut HashMap<(String, bool), CutawayTextureAsset>,
    pic_or_sheet: &str,
    prefer_sh2ee: bool,
) {
    let key = (pic_or_sheet.to_string(), prefer_sh2ee);
    if cache.contains_key(&key) {
        return;
    }
    let asset = load_cutaway_tex(pic_or_sheet, prefer_sh2ee).unwrap_or(CutawayTextureAsset {
        texture: DecodedTexture {
            width: 1,
            height: 1,
            rgba: vec![28, 34, 48, 255],
        },
        original_width: 512,
        original_height: 512,
        is_sh2ee_hd: prefer_sh2ee,
    });
    cache.insert(key, asset);
}

/// Blits a `560x420` cutaway still (`base_pic`) at `(vp_x, vp_y, vp_w, vp_h)` along with
/// custom `OverlaySpriteQuad` overlays and SH2:EE pillarbox bars.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blit_cutaway_viewport(
    state: &mut AppState,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    vp_h: usize,
    base_pic: &str,
    prefer_sh2ee: bool,
    quads: &[OverlaySpriteQuad],
) -> CutawayViewportTransform {
    state
        .menu_rasterizer
        .fill_rect(vp_x - 2, vp_y - 2, vp_w + 4, vp_h + 4, [0, 0, 0, 255]);
    state
        .menu_rasterizer
        .draw_rect_outline(vp_x - 2, vp_y - 2, vp_w + 4, vp_h + 4, 2, YELLOW);

    ensure_cutaway_cached(
        &mut state.story_progression_room.cutaway_cache,
        base_pic,
        prefer_sh2ee,
    );
    for q in quads {
        ensure_cutaway_cached(
            &mut state.story_progression_room.cutaway_cache,
            q.sheet_name,
            prefer_sh2ee,
        );
    }

    let mut vp_transform = CutawayViewportTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        draw_width: vp_w as f32,
        draw_height: vp_h as f32,
        scale_x: 1.0,
        scale_y: 1.0,
    };

    if let Some(base_tex) = state
        .story_progression_room
        .cutaway_cache
        .get(&(base_pic.to_string(), prefer_sh2ee))
    {
        vp_transform = base_tex.compute_viewport_transform(vp_w as f32, vp_h as f32);
        let dst_x = vp_x + (vp_transform.offset_x.round().max(0.0) as usize);
        let dst_y = vp_y + (vp_transform.offset_y.round().max(0.0) as usize);
        let dst_w = (vp_transform.draw_width.round().max(1.0) as usize).min(vp_w);
        let dst_h = (vp_transform.draw_height.round().max(1.0) as usize).min(vp_h);
        let (rgba, tw, th) = (
            base_tex.texture.rgba.clone(),
            base_tex.texture.width,
            base_tex.texture.height,
        );
        state
            .menu_rasterizer
            .blit_rgba_scaled(dst_x, dst_y, dst_w, dst_h, &rgba, tw, th, 1.0);
    }

    let draw_x = (vp_x as f32) + vp_transform.offset_x;
    let draw_y = (vp_y as f32) + vp_transform.offset_y;
    for quad in quads {
        if let Some(sheet_tex) = state
            .story_progression_room
            .cutaway_cache
            .get(&(quad.sheet_name.to_string(), prefer_sh2ee))
        {
            let tris = quad.to_viewport_triangles(
                draw_x,
                draw_y,
                vp_transform.draw_width,
                vp_transform.draw_height,
            );
            let (rgba, sw, sh) = (
                sheet_tex.texture.rgba.clone(),
                sheet_tex.texture.width,
                sheet_tex.texture.height,
            );
            state
                .menu_rasterizer
                .blit_overlay_triangles(&tris, &rgba, sw, sh, quad.alpha);
        }
    }

    let content_aspect = vp_transform.draw_width.max(1.0) / vp_transform.draw_height.max(1.0);
    let pillar = PillarboxViewportLayout::for_aspect(vp_w as f32, vp_h as f32, content_aspect);
    for bar in &pillar.black_bars {
        if bar.is_non_empty() {
            state.menu_rasterizer.fill_rect(
                vp_x + (bar.x0.floor().max(0.0) as usize),
                vp_y + (bar.y0.floor().max(0.0) as usize),
                bar.width().ceil() as usize,
                bar.height().ceil() as usize,
                [0, 0, 0, 255],
            );
        }
    }

    vp_transform
}

/// Draws the top header bar, 4 clickable/keyed tab pills (`[F1]`–`[F4]`), and bottom status bar.
pub(super) fn draw_header_and_tabs(state: &mut AppState) {
    use super::StoryDemoTab;
    if state.story_progression_room.active_tab == StoryDemoTab::EndingJudgeAndRanking {
        state
            .menu_rasterizer
            .fill_rect(8, 8, 1264, 58, [8, 10, 16, 200]);
        state
            .menu_rasterizer
            .draw_rect_outline(8, 8, 1264, 58, 1, [70, 88, 118, 220]);
        state
            .menu_rasterizer
            .fill_rect(8, 666, 1264, 46, [8, 10, 16, 205]);
        state
            .menu_rasterizer
            .draw_rect_outline(8, 666, 1264, 46, 1, [70, 88, 118, 220]);
    } else {
        state
            .menu_rasterizer
            .fill_rect(8, 8, 1264, 704, [8, 10, 16, 238]);
        state
            .menu_rasterizer
            .draw_rect_outline(8, 8, 1264, 704, 2, [70, 88, 118, 255]);
    }

    draw_text_clipped(
        state,
        20,
        15,
        152,
        YELLOW,
        "SILENT HILL 2 - INTERACTIVE PUZZLES, 3D ARENA, ENDING JUDGE & CREDITS STUDIO [SH2-65]",
    );

    let tabs = [
        (
            StoryDemoTab::Act1StagesAndRiddles,
            "[F1] Act 1 Cutaway Puzzles (SH2-113)",
        ),
        (
            StoryDemoTab::Act2StagesAndRiddles,
            "[F2] Act 2 Puzzles & Doors (SH2-114)",
        ),
        (
            StoryDemoTab::EndingJudgeAndRanking,
            "[F3] 3D Stage & Ending Judge (SH2-118)",
        ),
        (
            StoryDemoTab::UfoAndMaryLetterCredits,
            "[F4] UFO Comic & Mary Letter (SH2-118)",
        ),
    ];
    for (idx, (tab, title)) in tabs.iter().enumerate() {
        let bx = 20 + idx * 312;
        let active = state.story_progression_room.active_tab == *tab;
        let bg = if active {
            [28, 52, 74, 245]
        } else {
            [16, 20, 28, 230]
        };
        let fg = if active { GREEN } else { [60, 70, 90, 255] };
        state.menu_rasterizer.fill_rect(bx, 34, 304, 24, bg);
        state
            .menu_rasterizer
            .draw_rect_outline(bx, 34, 304, 24, 1, fg);
        draw_text_clipped(
            state,
            bx + 8,
            41,
            36,
            if active { GREEN } else { MUTED },
            title,
        );
    }

    let status_line = format!("STATUS: {}", state.story_progression_room.status_banner);
    draw_text_clipped(state, 20, 673, 152, ORANGE, &status_line);
    draw_text_clipped(
        state,
        20,
        691,
        152,
        CYAN,
        "[F1-F4/Tab] Switch Tab | [1-4] Select Puzzle/Action | [PgUp/PgDn] Stage/Ending | [R] Difficulty | [V] Vanilla PS2 vs SH2:EE | [Esc] Exit",
    );
}
