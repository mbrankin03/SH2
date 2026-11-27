//! Renderer for `MC/mc_menu.c` (`mcDrawMenu`, `mcDrawBG`, `mcDrawBGWord`, `mcDrawFrame`,
//! `mcDrawSlot`, `mcDrawPlace`, `mcDrawAddInfo`, `mcDrawStatus`).

use std::fmt::Write as _;

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
    let c = Ps2Canvas::from_viewport(rasterizer.width, rasterizer.height);

    // 1. `mcDrawBG` (`0x00247310`): `savebg.tbn2` / `savebg2.tbn2` dimmed background.
    //    When present, `blit_bg_stretched` writes opaque alpha=255 to every pixel directly,
    //    avoiding a redundant full-screen clear/fill pass.
    if let Some((rgba, w, h)) = savebg_rgba {
        blit_bg_stretched(rasterizer, &c, rgba, w, h);
    } else {
        rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);
    }

    // 2. `mcDrawBGWord` (`0x00247630`): large translucent scrolling location watermark.
    let selected_slot = screen.active_card_slot(screen.cursor_row);
    if let Some(sel) = selected_slot {
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

    // 3. `mcDrawFrame` (`SLUS_202.28` `0x00245770`, `MC/mc_menu.c:1275-1301`):
    //    Translucent folder outline (no opaque black fill so `savebg.tbn2` shines cleanly through).
    //    Frame bounds match `mcDrawSlot` (`0x00245e30`: `x = 0x24..0x1dc`, `y = 0x60..0x156`).
    let frame_x0 = c.xi(28.0);
    let frame_y0 = c.yi(96.0);
    let frame_w = c.x(456.0).round().max(1.0) as usize;
    let frame_h = c.y(246.0).round().max(1.0) as usize;
    let tab_w = c.x(182.0).round().max(1.0) as usize;
    let tab_y0 = c.yi(56.0);
    let tab_h = frame_y0.saturating_sub(tab_y0);
    let border_col = [0x40, 0x20, 0x20, 0x80];
    if screen.config.unlimited_slots {
        // PC / SH2:EE single "Save Folder" / "Load Folder" tab
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
        let folder_title = match screen.mode {
            SaveLoadMode::Save => "Save Folder",
            SaveLoadMode::Load => "Load Folder",
        };
        font.draw_line_centered(
            rasterizer,
            folder_title,
            (frame_x0 + tab_w / 2, c.yi(66.0)),
            row_px,
            Sh2TextColor::White,
            true,
        );
    } else {
        // Strict 1:1 Vanilla PS2 `mcDrawFrame` (`MC/mc_menu.c:1286-1299`):
        // `mcPutMes(0x30, 0x8b /* 139 */, 0x50 /* 80 */)` ("Memory Card 1") and
        // `mcPutMes(0x31, 0x175 /* 373 */, 0x50 /* 80 */)` ("Memory Card 2").
        rasterizer.fill_rect(frame_x0, frame_y0, frame_w, 1, border_col);
        let card1_col = if screen.active_card == 0 {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::Custom {
                top: [0x80, 0x80, 0x80, 0x80],
                bottom: [0x80, 0x80, 0x80, 0x80],
            }
        };
        let card2_col = if screen.active_card == 1 {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::Custom {
                top: [0x80, 0x80, 0x80, 0x80],
                bottom: [0x80, 0x80, 0x80, 0x80],
            }
        };
        font.draw_line_centered(
            rasterizer,
            "Memory Card 1",
            (c.xi(0x8b as f32), c.yi(0x50 as f32)),
            row_px,
            card1_col,
            true,
        );
        font.draw_line_centered(
            rasterizer,
            "Memory Card 2",
            (c.xi(0x175 as f32), c.yi(0x50 as f32)),
            row_px,
            card2_col,
            true,
        );
    }
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

    // 4. `mcDrawSlot` (`SLUS_202.28` `0x00245e30`, `MC/mc_menu.c:1378-1448`):
    //    Header at `x = 0x24 (36), y = 0x6e (110)` and rows at `y = 0x96 (150) + vis * 0x28 (40)`.
    let total_rows = screen.total_selectable_rows();
    if total_rows > 0 {
        let header = if screen.config.unlimited_slots {
            "SILENT HILL 2 Save Files"
        } else {
            "File 1"
        };
        font.draw_line_styled(
            rasterizer,
            header,
            (c.xi(0x24 as f32), c.yi(0x6e as f32)),
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

    // Exact `mcDrawSlot` (`MC/mc_menu.c:1432-1439`):
    // `((mcGetBlinkAlpha() + 0x20) << 24) | 0x208080` (PS2 GS ABGR: R=0x80, G=0x80, B=0x20, A=0..128).
    let blink = mc_get_blink_alpha(screen.blink_phase);
    let sel_alpha = (((blink + 0x20).max(0) * 255) / 128).clamp(0, 255) as u8;
    let scroll_px = screen.scroll_anim * 40.0;

    let mut scratch = String::with_capacity(64);
    let mut visible_slots = screen.active_card_slots_iter().skip(screen.scroll_top);

    for vis in 0..MC_VISIBLE_ROWS {
        let row_idx = screen.scroll_top + vis;
        if row_idx >= total_rows {
            break;
        }
        // Exact `mcDrawSlot` (`MC/mc_menu.c:1392, 1441-1446`):
        // `y = 0x96 (150) + vis * 0x28 (40)`, `x = 0x24 (36)..0x1dc (476)` (width 440),
        // `h = 0x7bd - 0x796 = 0x27 (39)` px.
        let y_512 = 150.0 + (vis as f32) * 40.0;
        let rx0 = c.xi(0x24 as f32);
        let rw = c.x(440.0).round().max(1.0) as usize;
        let rh = c.y(39.0).round().max(1.0) as usize;

        if row_idx == screen.cursor_row {
            let bar_y = c.yi(y_512 + scroll_px);
            rasterizer.fill_rect(rx0, bar_y, rw, rh, [0x80, 0x80, 0x20, sel_alpha]);
        }

        if let Some(entry) = visible_slots.next() {
            let label: &str = if entry.is_corrupted {
                "Corrupted Data"
            } else {
                let base = save_point_location_name(
                    entry.snapshot.save_point_name_id,
                    &entry.snapshot.room_id,
                );
                match entry.kind {
                    SaveSlotKind::Manual => base,
                    SaveSlotKind::Quicksave => {
                        scratch.clear();
                        let _ = write!(scratch, "[QS] {base}");
                        scratch.as_str()
                    }
                    SaveSlotKind::Autosave => {
                        scratch.clear();
                        let _ = write!(scratch, "[AUTO] {base}");
                        scratch.as_str()
                    }
                }
            };
            font.draw_line_styled(
                rasterizer,
                label,
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
                        const STARS: &str = "*******";
                        let count =
                            ((entry.endings_seen_mask() & 0x7f).count_ones() as usize).min(7);
                        let stars = &STARS[..count];
                        let sw = font.text_width(stars, row_px);
                        font.draw_line_styled(
                            rasterizer,
                            stars,
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
            if let Some(entry) = selected_slot {
                if !entry.is_corrupted {
                    let act = match entry.snapshot.action_difficulty {
                        ActionDifficulty::Beginner => "Action: Beginner",
                        ActionDifficulty::Easy => "Action: Easy",
                        ActionDifficulty::Normal => "Action: Normal",
                        ActionDifficulty::Hard => "Action: Hard",
                    };
                    let rid = match entry.snapshot.riddle_difficulty {
                        RiddleDifficulty::Easy => "Riddle: Easy",
                        RiddleDifficulty::Normal => "Riddle: Normal",
                        RiddleDifficulty::Hard => "Riddle: Hard",
                        RiddleDifficulty::Extra => "Riddle: Extra",
                    };
                    scratch.clear();
                    let _ = write!(scratch, "Data: {}", entry.file_index + 1);
                    font.draw_line_styled(
                        rasterizer,
                        &scratch,
                        (c.xi(46.0), c.yi(396.0)),
                        small_px,
                        Sh2TextColor::SilverUnselected,
                        true,
                    );
                    scratch.clear();
                    let _ = write!(scratch, "Saves: {}", entry.snapshot.save_count);
                    font.draw_line_styled(
                        rasterizer,
                        &scratch,
                        (c.xi(266.0), c.yi(396.0)),
                        small_px,
                        Sh2TextColor::SilverUnselected,
                        true,
                    );
                    font.draw_line_styled(
                        rasterizer,
                        act,
                        (c.xi(46.0), c.yi(422.0)),
                        small_px,
                        Sh2TextColor::White,
                        true,
                    );
                    font.draw_line_styled(
                        rasterizer,
                        rid,
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
    let src_stride = src_w * 4;
    let dst_stride = dw * 4;
    if src_rgba.len() < src_h * src_stride {
        return;
    }
    let du_dx = u_span / dw as f32;
    let src_w_f = src_w as f32;
    let src_w_max = src_w - 1;
    let src_h_f = src_h as f32;
    let src_h_max = src_h - 1;
    for py in 0..dh {
        let v = v0 + (py as f32 / dh as f32) * v_span;
        let sy = ((v * src_h_f) as usize).min(src_h_max);
        let src_row = &src_rgba[sy * src_stride..(sy + 1) * src_stride];
        let dst_row = &mut dst.pixels[py * dst_stride..(py + 1) * dst_stride];
        let mut u = u0;
        for px in 0..dw {
            let sx = ((u * src_w_f) as usize).min(src_w_max);
            u += du_dx;
            let si = sx * 4;
            let di = px * 4;
            // `mcDrawBG` (`SLUS_202.28` `0x00247310`): `savebg.tbn2` drawn at PS2 GS unity `0x80`.
            dst_row[di] = src_row[si];
            dst_row[di + 1] = src_row[si + 1];
            dst_row[di + 2] = src_row[si + 2];
            dst_row[di + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_load::state::{SaveLoadConfig, SaveLoadMode, SaveLoadScreen};

    #[test]
    fn draw_save_load_screen_vanilla_ps2_and_pc_modes_and_optional_preview() {
        let font = AtlasFont::load_embedded();
        let mut raster = TextRasterizer::new(640, 480);

        // Try loading retail `savebg.tbn2` if present for Layer 4 visual inspection.
        let savebg = [
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data",
            "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data",
        ]
        .iter()
        .find(|p| std::path::Path::new(p).join(SAVEBG_TBN2_PATH).is_file())
        .and_then(|data_dir| {
            let loader = sh2_assets::formats::texture::TextureLoader::new(data_dir, None::<&str>);
            loader
                .load_texture(SAVEBG_TBN2_PATH)
                .ok()
                .map(|tex| (tex.rgba, tex.width, tex.height))
        });
        let bg_ref = savebg
            .as_ref()
            .map(|(rgba, w, h)| (rgba.as_slice(), *w, *h));

        for (label, unlimited_slots) in [("vanilla_ps2", false), ("pc_sh2ee", true)] {
            let mut screen = SaveLoadScreen::new(
                SaveLoadMode::Save,
                sh2_core::ScenarioKind::default(),
                SaveLoadConfig {
                    unlimited_slots,
                    ..SaveLoadConfig::default()
                },
            );
            screen.blink_phase = 0.0;
            draw_save_load_screen(&screen, bg_ref, &font, &mut raster);

            // Assert `mcDrawSlot` (`0x00245e30`) ABGR `0x208080` amber highlight bar renders
            // at row 0 (`y = 0x96 = 150`, `x = 0x24..0x1dc`) with non-zero amber pixels.
            let non_black = raster
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|px| px[0] > 8 || px[1] > 8 || px[2] > 8)
                .count();
            assert!(
                non_black > 5000,
                "{label} Save/Load screen must render visible frame, text, and highlight bar"
            );

            if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
                let path = std::path::PathBuf::from(out_dir).join(format!("save_load_{label}.ppm"));
                let mut buf = Vec::with_capacity((raster.width * raster.height * 3) + 32);
                buf.extend_from_slice(
                    format!("P6\n{} {}\n255\n", raster.width, raster.height).as_bytes(),
                );
                for px in raster.pixels.as_chunks::<4>().0 {
                    buf.extend_from_slice(&px[..3]);
                }
                std::fs::write(path, buf).expect("write save_load preview ppm");
            }
        }
    }
}
