//! Renderer for `MC/mc_menu.c` (`mcDrawMenu`, `mcDrawBG`, `mcDrawBGWord`, `mcDrawFrame`,
//! `mcDrawSlot`, `mcDrawPlace`, `mcDrawAddInfo`, `mcDrawStatus`).

use super::state::{
    format_mc_time, mc_get_blink_alpha, save_point_location_name, SaveAddInfoMode, SaveLoadMode,
    SaveLoadScreen, SaveLoadStep, YesNoChoice, MC_VISIBLE_ROWS,
};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::widgets::Ps2Canvas;
use sh2_core::campaign::SaveSlotKind;
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};

/// Retail PC Save/Load background for Main Scenario (`sh2pc.exe` `0x0044b3e0`).
pub const SAVEBG_TBN2_PATH: &str = "menu/mc/savebg.tbn2";
/// Retail PC Save/Load background for *Born from a Wish* (`sh2pc.exe` `0x0044b3e0`).
pub const SAVEBG2_TBN2_PATH: &str = "menu/mc/savebg2.tbn2";

/// `tag_curve1` (`SLUS_202.28` `0x0034f270`, `mcDrawFrame` `0x00245770`).
pub const TAG_CURVE_1: [i8; 10] = [3, 18, 5, 10, 8, 5, 13, 1, 16, 0];
/// `tag_curve2` (`SLUS_202.28` `0x0034f280`, `mcDrawFrame` `0x00245770`).
pub const TAG_CURVE_2: [i8; 8] = [0, 8, 1, 3, 3, 1, 8, 0];

const COLOR_RED: Sh2TextColor = Sh2TextColor::Custom {
    top: [220, 48, 48, 255],
    bottom: [160, 24, 24, 255],
};

/// Renders the 1:1 PS2 `MC/mc_menu.c` Save/Load screen onto `rasterizer`.
pub fn draw_save_load_screen(
    screen: &SaveLoadScreen,
    savebg_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    rasterizer.clear();
    rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);
    let c = Ps2Canvas::from_viewport(rasterizer.width, rasterizer.height);

    // 1. `mcDrawBG` (`0x00247310`): `savebg.tbn2` / `savebg2.tbn2` dimmed background.
    if let Some((rgba, w, h)) = savebg_rgba {
        blit_bg_stretched(rasterizer, &c, rgba, w, h);
    }

    // 2. `mcDrawBGWord` (`0x00247630`): large translucent scrolling location watermark.
    let card_slots = screen.active_card_slots();
    if let Some(sel) = card_slots.get(screen.cursor_row) {
        let loc = save_point_location_name(sel.snapshot.save_point_name_id, &sel.snapshot.room_id);
        let big_px = ((44.0 * c.sy).round() as usize).max(20);
        let scroll_y = 166.0 - (screen.scroll_anim * 180.0 * 1.5);
        font.draw_line_centered(
            rasterizer,
            loc,
            (c.xi(256.0), c.yi(scroll_y)),
            big_px,
            Sh2TextColor::Custom {
                top: [200, 200, 200, 0x20],
                bottom: [180, 180, 180, 0x20],
            },
            false,
        );
    }

    let title_px = ((22.0 * c.sy).round() as usize).max(13);
    let row_px = ((20.0 * c.sy).round() as usize).max(12);
    let small_px = ((18.0 * c.sy).round() as usize).max(11);

    // 3. `mcDrawFrame` (`0x00245770` / PC `sh2pc.exe` Save Folder):
    //    Subtle translucent folder tab + thin outline border with NO opaque black fill so
    //    `savebg.tbn2` (James's red portrait) shines cleanly through.
    let frame_x0 = c.xi(28.0);
    let frame_y0 = c.yi(96.0);
    let frame_w = c.x(456.0).round().max(1.0) as usize;
    let frame_h = c.y(246.0).round().max(1.0) as usize;
    let tab_w = c.x(182.0).round().max(1.0) as usize;
    let tab_y0 = c.yi(56.0);
    let tab_h = frame_y0.saturating_sub(tab_y0);
    let border_col = [56, 18, 14, 150];
    // Folder tab outline (top + left + right) and subtle main list outline
    rasterizer.fill_rect(frame_x0, tab_y0, tab_w, 1, border_col);
    rasterizer.fill_rect(frame_x0, tab_y0, 1, tab_h, border_col);
    rasterizer.fill_rect(
        frame_x0 + tab_w.saturating_sub(1),
        tab_y0,
        1,
        tab_h,
        border_col,
    );
    rasterizer.fill_rect(
        frame_x0 + tab_w,
        frame_y0,
        frame_w.saturating_sub(tab_w),
        1,
        border_col,
    );
    rasterizer.fill_rect(
        frame_x0,
        frame_y0 + frame_h.saturating_sub(1),
        frame_w,
        1,
        border_col,
    );
    rasterizer.fill_rect(frame_x0, frame_y0, 1, frame_h, border_col);
    rasterizer.fill_rect(
        frame_x0 + frame_w.saturating_sub(1),
        frame_y0,
        1,
        frame_h,
        border_col,
    );

    let folder_title = if screen.config.unlimited_slots {
        match screen.mode {
            SaveLoadMode::Save => "Save Folder",
            SaveLoadMode::Load => "Load Folder",
        }
    } else {
        "Save Folder"
    };
    font.draw_line_centered(
        rasterizer,
        folder_title,
        (frame_x0 + tab_w / 2, c.yi(66.0)),
        row_px,
        Sh2TextColor::White,
        true,
    );

    // 4. `mcDrawSlot` (`0x00245e30` / PC `sh2pc.exe`): "SILENT HILL 2 Save Files" header + rows.
    let total_rows = screen.total_selectable_rows();
    if total_rows > 0 {
        font.draw_line_styled(
            rasterizer,
            "SILENT HILL 2 Save Files",
            (c.xi(36.0), c.yi(108.0)),
            title_px,
            Sh2TextColor::White,
            true,
        );
    } else {
        font.draw_line_centered(
            rasterizer,
            "No Save Data",
            (c.xi(256.0), c.yi(0xeb as f32)),
            title_px,
            Sh2TextColor::GoldSelected,
            true,
        );
    }

    let blink = mc_get_blink_alpha(screen.blink_phase);
    let sel_alpha = (((blink + 0x20) * 2).clamp(96, 176)) as u8;
    let scroll_px = screen.scroll_anim * 40.0;

    for vis in 0..MC_VISIBLE_ROWS {
        let row_idx = screen.scroll_top + vis;
        if row_idx >= total_rows {
            break;
        }
        let y_512 = 148.0 + (vis as f32) * 38.0;
        let rx0 = c.xi(36.0);
        let rw = c.x(440.0).round().max(1.0) as usize;
        let rh = c.y(28.0).round().max(1.0) as usize;

        if row_idx == screen.cursor_row {
            // `mcDrawSlot` (`0x00245e30`): PS2 GS ABGR `0x208080` = R:0x80, G:0x80, B:0x20
            // (warm translucent amber/gold highlight bar with clean 45° chamfered top corners).
            let bar_y = c.yi(y_512 + scroll_px);
            let bevel = (c.y(4.0).round() as usize).max(2).min(rh / 2);
            for dy in 0..rh {
                let inset = bevel.saturating_sub(dy);
                let line_w = rw.saturating_sub(inset * 2);
                if line_w > 0 {
                    rasterizer.fill_rect(
                        rx0 + inset,
                        bar_y + dy,
                        line_w,
                        1,
                        [128, 112, 32, sel_alpha],
                    );
                }
            }
        }

        if let Some(entry) = card_slots.get(row_idx) {
            let label = if entry.is_corrupted {
                "Corrupted Data".to_string()
            } else {
                let base = save_point_location_name(
                    entry.snapshot.save_point_name_id,
                    &entry.snapshot.room_id,
                );
                match entry.kind {
                    SaveSlotKind::Manual => base.to_string(),
                    SaveSlotKind::Quicksave => format!("[QS] {base}"),
                    SaveSlotKind::Autosave => format!("[AUTO] {base}"),
                }
            };
            font.draw_line_styled(
                rasterizer,
                &label,
                (c.xi(44.0), c.yi(y_512 + 6.0)),
                row_px,
                Sh2TextColor::White,
                true,
            );

            if !entry.is_corrupted {
                match screen.add_info_mode {
                    SaveAddInfoMode::PlayTime => {
                        let t = format_mc_time(entry.snapshot.play_time_secs);
                        font.draw_line_styled(
                            rasterizer,
                            &t,
                            (c.xi(330.0), c.yi(y_512 + 6.0)),
                            row_px,
                            Sh2TextColor::White,
                            true,
                        );
                    }
                    SaveAddInfoMode::SecondaryTime => {
                        let t = format_mc_time(entry.snapshot.secondary_timer_secs);
                        font.draw_line_styled(
                            rasterizer,
                            &t,
                            (c.xi(330.0), c.yi(y_512 + 6.0)),
                            row_px,
                            Sh2TextColor::GoldSelected,
                            true,
                        );
                    }
                    SaveAddInfoMode::EndingsCleared => {
                        let count = (entry.endings_seen_mask() & 0x7f).count_ones() as usize;
                        let stars = "*".repeat(count.min(7));
                        let sw = font.text_width(&stars, row_px);
                        font.draw_line_styled(
                            rasterizer,
                            &stars,
                            (c.xi(438.0).saturating_sub(sw), c.yi(y_512 + 6.0)),
                            row_px,
                            Sh2TextColor::GoldSelected,
                            true,
                        );
                    }
                }
            }
        } else {
            font.draw_line_styled(
                rasterizer,
                "Save Possible",
                (c.xi(44.0), c.yi(y_512 + 6.0)),
                title_px,
                Sh2TextColor::White,
                true,
            );
        }
    }

    // 5. `mcDrawStatus` (`0x00246c30`): bottom status box or Yes/No prompt.
    match screen.step {
        SaveLoadStep::ConfirmOverwrite | SaveLoadStep::ConfirmDelete => {
            let prompt = if screen.step == SaveLoadStep::ConfirmOverwrite {
                "Do you wish to overwrite?"
            } else {
                "Delete this save data?"
            };
            font.draw_line_centered(
                rasterizer,
                prompt,
                (c.xi(256.0), c.yi(0x194 as f32)),
                row_px,
                Sh2TextColor::White,
                true,
            );
            let yes_col = if screen.yes_no == YesNoChoice::Yes {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::White
            };
            let no_col = if screen.yes_no == YesNoChoice::No {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::White
            };
            font.draw_line_centered(
                rasterizer,
                "Yes",
                (c.xi(200.0), c.yi(0x1c0 as f32)),
                row_px,
                yes_col,
                true,
            );
            font.draw_line_centered(
                rasterizer,
                "No",
                (c.xi(312.0), c.yi(0x1c0 as f32)),
                row_px,
                no_col,
                true,
            );
        }
        SaveLoadStep::SaveDone => {
            font.draw_line_centered(
                rasterizer,
                "Save completed.",
                (c.xi(256.0), c.yi(0x194 as f32)),
                row_px,
                Sh2TextColor::GoldSelected,
                true,
            );
        }
        SaveLoadStep::DeleteDone => {
            font.draw_line_centered(
                rasterizer,
                "Delete completed.",
                (c.xi(256.0), c.yi(0x194 as f32)),
                row_px,
                Sh2TextColor::GoldSelected,
                true,
            );
        }
        SaveLoadStep::LoadError => {
            font.draw_line_centered(
                rasterizer,
                "Load failed (corrupted data).",
                (c.xi(256.0), c.yi(0x194 as f32)),
                row_px,
                COLOR_RED,
                true,
            );
        }
        SaveLoadStep::SelectSlot | SaveLoadStep::LoadDone => {
            if let Some(entry) = card_slots.get(screen.cursor_row) {
                if !entry.is_corrupted {
                    let act = match entry.snapshot.action_difficulty {
                        ActionDifficulty::Beginner => "Beginner",
                        ActionDifficulty::Easy => "Easy",
                        ActionDifficulty::Normal => "Normal",
                        ActionDifficulty::Hard => "Hard",
                    };
                    let rid = match entry.snapshot.riddle_difficulty {
                        RiddleDifficulty::Easy => "Easy",
                        RiddleDifficulty::Normal => "Normal",
                        RiddleDifficulty::Hard => "Hard",
                        RiddleDifficulty::Extra => "Extra",
                    };
                    font.draw_line_styled(
                        rasterizer,
                        &format!("Data: {}", entry.file_index + 1),
                        (c.xi(46.0), c.yi(396.0)),
                        small_px,
                        Sh2TextColor::SilverUnselected,
                        true,
                    );
                    font.draw_line_styled(
                        rasterizer,
                        &format!("Saves: {}", entry.snapshot.save_count),
                        (c.xi(266.0), c.yi(396.0)),
                        small_px,
                        Sh2TextColor::SilverUnselected,
                        true,
                    );
                    font.draw_line_styled(
                        rasterizer,
                        &format!("Action: {act}"),
                        (c.xi(46.0), c.yi(422.0)),
                        small_px,
                        Sh2TextColor::White,
                        true,
                    );
                    font.draw_line_styled(
                        rasterizer,
                        &format!("Riddle: {rid}"),
                        (c.xi(266.0), c.yi(422.0)),
                        small_px,
                        Sh2TextColor::White,
                        true,
                    );
                }
            } else if screen.mode == SaveLoadMode::Save {
                font.draw_line_centered(
                    rasterizer,
                    "32 KB needed for new game save",
                    (c.xi(256.0), c.yi(358.0)),
                    row_px,
                    Sh2TextColor::White,
                    true,
                );
            }
        }
    }
}

fn blit_bg_stretched(
    dst: &mut TextRasterizer,
    _c: &Ps2Canvas,
    src_rgba: &[u8],
    src_w: usize,
    src_h: usize,
) {
    if src_w == 0 || src_h == 0 || dst.width == 0 || dst.height == 0 {
        return;
    }
    // Fill full viewport edge-to-edge (no black side bars) using aspect-preserving cover crop
    // so James's red-tinted portrait (`savebg.tbn2`) keeps authentic proportions on 16:9 / 4:3.
    let dst_aspect = dst.width as f32 / dst.height as f32;
    let src_aspect = src_w as f32 / src_h as f32;
    let (u0, u_span, v0, v_span) = if dst_aspect > src_aspect {
        let vis_v = src_aspect / dst_aspect;
        (0.0, 1.0, (1.0 - vis_v) * 0.5, vis_v)
    } else {
        let vis_u = dst_aspect / src_aspect;
        ((1.0 - vis_u) * 0.5, vis_u, 0.0, 1.0)
    };
    let dw = dst.width;
    let dh = dst.height;
    for py in 0..dh {
        let v = v0 + (py as f32 / dh as f32) * v_span;
        let sy = ((v * src_h as f32) as usize).min(src_h - 1);
        for px in 0..dw {
            let u = u0 + (px as f32 / dw as f32) * u_span;
            let sx = ((u * src_w as f32) as usize).min(src_w - 1);
            let si = (sy * src_w + sx) * 4;
            if si + 3 >= src_rgba.len() {
                continue;
            }
            dst.blend_pixel(
                px,
                py,
                [
                    (u16::from(src_rgba[si]) * 3 / 4) as u8,
                    (u16::from(src_rgba[si + 1]) * 3 / 4) as u8,
                    (u16::from(src_rgba[si + 2]) * 3 / 4) as u8,
                    255,
                ],
            );
        }
    }
}
