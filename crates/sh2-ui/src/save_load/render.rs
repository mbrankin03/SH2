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

    let title_px = ((20.0 * c.sy).round() as usize).max(12);
    let row_px = ((16.0 * c.sy).round() as usize).max(10);
    let small_px = ((14.0 * c.sy).round() as usize).max(9);

    // 3. `mcDrawFrame` (`0x00245770`): Slot 1 / Slot 2 tabs & bordered list frame (`x=21..491, y=100..370`).
    let frame_x0 = c.xi(21.0);
    let frame_y0 = c.yi(100.0);
    let frame_w = c.x(470.0).round().max(1.0) as usize;
    let frame_h = c.y(270.0).round().max(1.0) as usize;
    rasterizer.fill_rect(frame_x0, frame_y0, frame_w, frame_h, [12, 16, 22, 196]);
    draw_rect_outline(
        rasterizer,
        frame_x0,
        frame_y0,
        frame_w,
        frame_h,
        [110, 120, 140, 220],
    );

    if !screen.config.unlimited_slots {
        let slot1_col = if screen.active_card == 0 {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::SilverUnselected
        };
        let slot2_col = if screen.active_card == 1 {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::SilverUnselected
        };
        font.draw_line_centered(
            rasterizer,
            "Memory Card 1",
            (c.xi(0x8b as f32), c.yi(68.0)),
            row_px,
            slot1_col,
            true,
        );
        font.draw_line_centered(
            rasterizer,
            "Memory Card 2",
            (c.xi(0x175 as f32), c.yi(68.0)),
            row_px,
            slot2_col,
            true,
        );
    } else {
        let mode_title = match screen.mode {
            SaveLoadMode::Save => "SAVE GAME (UNLIMITED)",
            SaveLoadMode::Load => "LOAD GAME (UNLIMITED)",
        };
        font.draw_line_centered(
            rasterizer,
            mode_title,
            (c.xi(256.0), c.yi(64.0)),
            title_px,
            Sh2TextColor::GoldSelected,
            true,
        );
    }

    // 4. `mcDrawSlot` (`0x00245e30`): File N header + 5 visible rows (`y = 0x96 + row * 0x28`).
    let total_rows = screen.total_selectable_rows();
    if total_rows > 0 {
        let file_num = screen.cursor_row + 1;
        font.draw_line_styled(
            rasterizer,
            &format!("File {file_num}"),
            (c.xi(0x24 as f32), c.yi(0x6e as f32)),
            row_px,
            Sh2TextColor::GoldSelected,
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
    let sel_alpha = ((blink + 0x20) * 2).clamp(0, 255) as u8;
    let scroll_px = screen.scroll_anim * 40.0;

    for vis in 0..MC_VISIBLE_ROWS {
        let row_idx = screen.scroll_top + vis;
        if row_idx >= total_rows {
            break;
        }
        let y_512 = 150.0 + (vis as f32) * 40.0;
        let rx0 = c.xi(36.0);
        let ry0 = c.yi(y_512);
        let rw = c.x(440.0).round().max(1.0) as usize;
        let rh = c.y(36.0).round().max(1.0) as usize;

        rasterizer.fill_rect(rx0, ry0, rw, rh, [24, 30, 42, 160]);
        if row_idx == screen.cursor_row {
            // `mcDrawSlot` (`0x00245e30`): blinking cyan bar `(alpha + 0x20) << 24 | 0x208080`.
            let bar_y = c.yi(y_512 + scroll_px);
            rasterizer.fill_rect(rx0, bar_y, rw, rh, [32, 128, 128, sel_alpha]);
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
                (c.xi(44.0), c.yi(y_512 + 8.0)),
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
                            (c.xi(330.0), c.yi(y_512 + 8.0)),
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
                            (c.xi(330.0), c.yi(y_512 + 8.0)),
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
                            (c.xi(438.0).saturating_sub(sw), c.yi(y_512 + 8.0)),
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
                "New Save",
                (c.xi(44.0), c.yi(y_512 + 8.0)),
                row_px,
                Sh2TextColor::GoldSelected,
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
            }
        }
    }
}

fn blit_bg_stretched(
    dst: &mut TextRasterizer,
    c: &Ps2Canvas,
    src_rgba: &[u8],
    src_w: usize,
    src_h: usize,
) {
    if src_w == 0 || src_h == 0 {
        return;
    }
    let dw = c.x(512.0).round().max(1.0) as usize;
    let dh = c.y(448.0).round().max(1.0) as usize;
    let x0 = c.xi(0.0);
    let y0 = c.yi(0.0);
    for dy in 0..dh {
        let py = y0 + dy;
        if py >= dst.height {
            break;
        }
        let sy = (dy * src_h) / dh;
        for dx in 0..dw {
            let px = x0 + dx;
            if px >= dst.width {
                break;
            }
            let sx = (dx * src_w) / dw;
            let si = (sy * src_w + sx) * 4;
            if si + 3 >= src_rgba.len() {
                continue;
            }
            dst.blend_pixel(
                px,
                py,
                [
                    src_rgba[si] / 2,
                    src_rgba[si + 1] / 2,
                    src_rgba[si + 2] / 2,
                    255,
                ],
            );
        }
    }
}

fn draw_rect_outline(
    rast: &mut TextRasterizer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: [u8; 4],
) {
    if w == 0 || h == 0 {
        return;
    }
    rast.fill_rect(x, y, w, 1, color);
    rast.fill_rect(x, y + h.saturating_sub(1), w, 1, color);
    rast.fill_rect(x, y, 1, h, color);
    rast.fill_rect(x + w.saturating_sub(1), y, 1, h, color);
}
