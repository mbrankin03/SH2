//! 1:1 PS2 / SH2:EE Memo List & Display renderer (`Event/memo.c`).
//!
//! # Ground Truth Citations (`SLUS_202.28` & `sh2pc.exe`)
//! - `MemoSelect` (`0x00230810` / `0x0049a0c0`):
//!   - `MemoPictureBaseDraw(0x60)` (`0x60 / 0x80 = 0.75` brightness)
//!   - `< 12` memos: centered vertically at `y = 256 - count * 16 + i * 32` (`512x512` space), alpha `0x80`
//!   - `>= 12` memos: 13 circular rows at `y = 48 + row * 32`, edge alpha fade `0x10 / 0x20 / 0x40 / 0x80`
//! - `MemoSelectBarDraw` (`0x002315c0` / `0x00499180`):
//!   - Green highlight bar `RGBA(0x20, 0x60, 0x20, 0x40)` (`0x40 / 0x80 = 50%` alpha = `128/255`),
//!     half-width `text_width / 2 + 4`, height `32` in `512x512` virtual space.
//! - `MemoDisplay` (`0x00230c70` / `0x0049a750`) & `MemoFix.cpp` (`0x0049914d`):
//!   - Synchronized brightness `(dim_alpha * 64.0) as u8 + 0x40` (`0x80 -> 0x40`) across both
//!     `MemoPictureBaseDraw` and `MemoPictureLayerDraw`.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::map::MapCanvasClipper;
use crate::memo::overlays::{apply_memo_dynamic_codes, build_memo_layer_quads, MemoLayerQuad};
use crate::memo::{MemoPhase, MemoReader};
use sh2_assets::formats::mes::MessageTable;
use sh2_core::config::RiddleDifficulty;
use sh2_core::events::{EventFlags, PuzzleState};

/// Optional decoded retail RGBA textures for the active memo view.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoRenderTextures<'a> {
    /// Decoded backdrop `.tex` (`pic/etc/p_memo.tex` or entry-specific inspection still).
    pub backdrop_rgba: Option<(&'a [u8], usize, usize)>,
    /// Decoded puzzle overlay layer `.tex` (e.g. `p_safe_close2.tex`, `p_boxnumber_2.tex`, etc.).
    pub layer_rgba: Option<(&'a [u8], usize, usize)>,
}

/// Optional puzzle/flag context for `MemoPictureLayerDraw*` overlays and wallet/keypad substitution.
#[derive(Debug, Clone, Copy)]
pub struct MemoRenderContext<'a> {
    /// Active `EventFlags` (used for SafeLock, Hair, AngelRing, Tablet overlays).
    pub flags: &'a EventFlags,
    /// Active `PuzzleState` (used for Guruguru overlay, Wallet safe code, Keypad code).
    pub puzzle: &'a PuzzleState,
    /// Active riddle difficulty (used for `MemoMessageWallet`).
    pub riddle: RiddleDifficulty,
}

/// Renders the 1:1 PS2 / SH2:EE Memo List or Multi-Page Memo Reader onto `rasterizer`.
pub fn draw_memo_reader(
    reader: &MemoReader,
    mes_table: Option<&MessageTable>,
    textures: MemoRenderTextures<'_>,
    ctx: Option<MemoRenderContext<'_>>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    if !reader.is_open {
        return;
    }

    let vw = rasterizer.width as f32;
    let vh = rasterizer.height as f32;
    let clipper = MapCanvasClipper::new_4x3(vw, vh);
    let cl = clipper.layout.canvas_left;
    let ct = clipper.layout.canvas_top;
    let cw = clipper.layout.canvas_right - clipper.layout.canvas_left;
    let ch = clipper.layout.canvas_bottom - clipper.layout.canvas_top;

    // 1. sh2gfw_Black_Clear()
    rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);

    // 2. Synchronized base brightness (`0x60` in List; `(dim_alpha * 64) + 0x40` in Display)
    let brightness_ps2 = reader.backdrop_brightness_ps2();
    let brightness_mul = (brightness_ps2 as f32) / 128.0;

    // 3. MemoPictureBaseDraw(brightness_ps2)
    if let Some((rgba, bw, bh)) = textures.backdrop_rgba {
        let tris = [
            (cl, ct, 0.0, 0.0),
            (cl + cw, ct, 1.0, 0.0),
            (cl + cw, ct + ch, 1.0, 1.0),
            (cl, ct, 0.0, 0.0),
            (cl + cw, ct + ch, 1.0, 1.0),
            (cl, ct + ch, 0.0, 1.0),
        ];
        rasterizer.blit_overlay_triangles(&tris, rgba, bw, bh, brightness_mul);
    }

    let scale_x = cw / 512.0;
    let scale_y = ch / 512.0;
    let text_px = ((22.0 * scale_y).round() as usize).clamp(12, 34);

    match reader.phase {
        MemoPhase::List => {
            draw_memo_select_list(
                reader, mes_table, cl, ct, cw, scale_x, scale_y, text_px, font, rasterizer,
            );
        }
        MemoPhase::ViewingImage | MemoPhase::ViewingText => {
            let Some(entry) = reader.selected_entry() else {
                clipper.draw_black_pillarbox_bars(rasterizer);
                return;
            };
            // 4. MemoPictureLayerDraw(brightness_ps2) (synchronized brightness per MemoFix.cpp)
            if let (Some(c), Some((lrgba, lw, lh))) = (ctx, textures.layer_rgba) {
                let (nat_w, nat_h) = match entry.index {
                    0x12 => (128.0, 128.0),        // p_safe_close2.tex
                    0x20 => (512.0, 256.0),        // p_boxnumber_2.tex
                    0x21 => (256.0, 64.0),         // p_hair_hair.tex
                    0x23 => (64.0, 64.0),          // p_female_ring.tex
                    0x29 => (256.0, 256.0),        // p_tablet_plate.tex
                    0x35 | 0x36 => (512.0, 512.0), // maker.tex / teien2.tex
                    _ => (lw.max(1) as f32, lh.max(1) as f32),
                };
                let quads = build_memo_layer_quads(entry.index, c.flags, c.puzzle);
                for q in &quads {
                    draw_layer_quad(
                        q,
                        cl,
                        ct,
                        cw,
                        ch,
                        lrgba,
                        lw,
                        lh,
                        nat_w,
                        nat_h,
                        brightness_mul,
                        rasterizer,
                    );
                }
            }
            // 5. Text pages (only in ViewingText; ViewingImage shows pure still + overlays)
            if reader.phase == MemoPhase::ViewingText {
                let pages = MemoReader::entry_pages(entry, mes_table);
                let page_idx = reader.current_page.min(pages.len().saturating_sub(1));
                let raw_text = &pages[page_idx];
                let page_text = if let Some(c) = ctx {
                    apply_memo_dynamic_codes(entry.index, raw_text, c.puzzle, c.riddle)
                } else {
                    raw_text.clone()
                };
                draw_memo_text_page(&page_text, cl, ct, cw, ch, text_px, font, rasterizer);
            }
        }
    }

    // 6. Enforce pure black pillarboxes outside the 4:3 canvas
    clipper.draw_black_pillarbox_bars(rasterizer);
}

/// Renders `MemoSelect` (`0x00230810` / `0x0049a0c0`) 11-row centered or 13-row circular list
/// with exact edge alpha fade (`0x10 / 0x20 / 0x40 / 0x80`) and green `MemoSelectBarDraw`
/// (`0x002315c0`, `RGBA(0x20, 0x60, 0x20, 0x40)`).
#[allow(clippy::too_many_arguments)]
fn draw_memo_select_list(
    reader: &MemoReader,
    mes_table: Option<&MessageTable>,
    cl: f32,
    ct: f32,
    cw: f32,
    scale_x: f32,
    scale_y: f32,
    text_px: usize,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    let count = reader.entries.len();
    if count == 0 {
        return;
    }
    let center_x = (cl + cw * 0.5).round() as usize;
    let bar_h = (32.0 * scale_y).round().max(text_px as f32 + 4.0) as usize;
    let pad_x = (4.0 * scale_x).round().max(3.0) as usize;

    let rows = reader.visible_list_rows();
    for row in rows {
        let entry = reader.entries[row.entry_idx];
        let title = MemoReader::entry_title(entry, mes_table);
        let vy = (ct + (row.y_ps2 as f32) * scale_y).round().max(0.0) as usize;

        if row.is_selected {
            // MemoSelectBarDraw (0x002315c0): green bar RGBA(0x20, 0x60, 0x20, 0x40 -> alpha 128/255)
            let tw = font.text_width(title, text_px);
            let half_w = tw / 2 + pad_x;
            let bx = center_x.saturating_sub(half_w);
            rasterizer.fill_rect(bx, vy, half_w * 2, bar_h, [0x20, 0x60, 0x20, 128]);
        }

        // Convert PS2 0..0x80 alpha to 0..255 RGBA alpha
        let rgba_a = ((row.alpha_ps2 as u16 * 255) / 128).min(255) as u8;
        let tw = font.text_width(title, text_px);
        let tx = center_x.saturating_sub(tw / 2);
        let ty = vy + bar_h.saturating_sub(text_px) / 2;
        font.draw_line(
            rasterizer,
            title,
            (tx, ty),
            text_px,
            [235, 235, 235, rgba_a],
            true,
        );
    }
}

/// Blits a single `MemoLayerQuad` onto `rasterizer` in the active 4:3 canvas.
#[allow(clippy::too_many_arguments)]
fn draw_layer_quad(
    quad: &MemoLayerQuad,
    cl: f32,
    ct: f32,
    cw: f32,
    ch: f32,
    layer_rgba: &[u8],
    lw: usize,
    lh: usize,
    nat_w: f32,
    nat_h: f32,
    brightness_mul: f32,
    rasterizer: &mut TextRasterizer,
) {
    let mut v = [(0.0f32, 0.0f32); 4];
    for (i, (px, py)) in quad.corners_ps2.iter().enumerate() {
        v[i] = (
            cl + ((*px + 256.0) / 512.0) * cw,
            ct + ((*py + 256.0) / 512.0) * ch,
        );
    }
    let u0 = quad.uv_px[0] / nat_w;
    let v0 = quad.uv_px[1] / nat_h;
    let u1 = quad.uv_px[2] / nat_w;
    let v1 = quad.uv_px[3] / nat_h;
    let tris = [
        (v[0].0, v[0].1, u0, v0),
        (v[1].0, v[1].1, u1, v0),
        (v[3].0, v[3].1, u1, v1),
        (v[0].0, v[0].1, u0, v0),
        (v[3].0, v[3].1, u1, v1),
        (v[2].0, v[2].1, u0, v1),
    ];
    rasterizer.blit_overlay_triangles(&tris, layer_rgba, lw, lh, brightness_mul);
}

/// Draws centered multi-line `.mes` memo page text over the dimmed backdrop (`fontMessageNum`).
#[allow(clippy::too_many_arguments)]
fn draw_memo_text_page(
    page_text: &str,
    cl: f32,
    ct: f32,
    cw: f32,
    ch: f32,
    text_px: usize,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    let lines: Vec<&str> = page_text.lines().take(12).collect();
    if lines.is_empty() {
        return;
    }
    let step = text_px + (text_px / 3).max(4);
    let total_h = lines.len() * step;
    let start_y = (ct + (ch - total_h as f32).max(0.0) * 0.5).round() as usize;
    let center_x = (cl + cw * 0.5).round() as usize;
    for (i, line) in lines.iter().enumerate() {
        font.draw_line_centered(
            rasterizer,
            line,
            (center_x, start_y + i * step),
            text_px,
            Sh2TextColor::White,
            true,
        );
    }
}
