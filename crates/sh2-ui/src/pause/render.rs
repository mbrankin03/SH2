//! Retail PC / PS2 Pause Menu rendering (`FUN_00407300` `0x00407300`, `FUN_00407610` `0x00407610`).

use super::{PauseMenu, PauseMenuOption, PauseSubPhase};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::fingerprint::UiFingerprintHasher;
use crate::font::TextRasterizer;

/// Retail PC `sh2pc.exe` Pause Menu 512×480 virtual coordinates (`FUN_00407300` `0x00407300`
/// and `FUN_00407610` `0x00407610`).
pub const PAUSE_VIRTUAL_W: f32 = 512.0;
pub const PAUSE_VIRTUAL_H: f32 = 480.0;
/// `"PAUSED"` header position `(0x100, 0xa4)` in `FUN_00407300` (`0x00407300`).
pub const PAUSE_HEADER_XY: (f32, f32) = (256.0, 164.0);
/// First pause item Y (`0xe6` = `230`) and row pitch (`0x1e` = `30`) in `FUN_00407300` (`0x00407300`).
pub const PAUSE_ITEM_START_Y: f32 = 230.0;
pub const PAUSE_ITEM_PITCH_Y: f32 = 30.0;
/// Quit-confirm prompt `(0x100, 0xe8)` and Yes/No `(0xe2/0x11e, 0x10a)` in `FUN_00407610` (`0x00407610`).
pub const PAUSE_QUIT_PROMPT_XY: (f32, f32) = (256.0, 232.0);
pub const PAUSE_QUIT_YES_XY: (f32, f32) = (226.0, 266.0);
pub const PAUSE_QUIT_NO_XY: (f32, f32) = (286.0, 266.0);

/// Computes a deterministic 64-bit state fingerprint for [`draw_pause_screen_with_scrim`] (`SH2-177`).
#[must_use]
pub fn pause_render_fingerprint(
    pause_menu: &PauseMenu,
    draw_scrim: bool,
    viewport: (usize, usize),
) -> u64 {
    let mut h = UiFingerprintHasher::with_tag(0x5041_5553_454D_454E);
    h.write_usize(viewport.0);
    h.write_usize(viewport.1);
    h.write_bool(draw_scrim);
    h.write_bool(pause_menu.is_open);
    h.write_u8(pause_menu.sub_phase as u8);
    h.write_usize(pause_menu.selected_index);
    h.write_usize(pause_menu.quit_confirm_index);
    h.write_bool(pause_menu.can_save);
    match pause_menu.status_message.as_deref() {
        Some(msg) => {
            h.write_u8(1);
            h.write_str(msg);
        }
        None => h.write_u8(0),
    }
    h.finish()
}

/// Renders the authentic Pause Menu overlay into `rasterizer`, scaled cleanly into a 4:3
/// pillarboxed viewport using exact retail `sh2pc.exe` 512×448 virtual coordinates
/// (`FUN_00407300` `0x00407300`, `FUN_00407610` `0x00407610`).
pub fn draw_pause_screen(
    pause_menu: &PauseMenu,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    draw_pause_screen_with_scrim(pause_menu, font, rasterizer, true);
}

/// Same as [`draw_pause_screen`], allowing the caller (`sh2-app`) to disable the inner 4:3 dim
/// scrim when a full-screen widescreen backdrop scrim has already been rendered across the
/// entire swapchain.
pub fn draw_pause_screen_with_scrim(
    pause_menu: &PauseMenu,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    draw_scrim: bool,
) {
    if !pause_menu.is_open {
        return;
    }

    let w = rasterizer.width;
    let h = rasterizer.height;

    // 4:3 pillarboxed projection from 512×448 retail PC virtual coordinates
    let target_4_3_w = h as f32 * (4.0 / 3.0);
    let (x_offset, base_w) = if target_4_3_w <= w as f32 {
        ((w as f32 - target_4_3_w) * 0.5, target_4_3_w)
    } else {
        (0.0, w as f32)
    };
    let scale_x = base_w / PAUSE_VIRTUAL_W;
    let scale_y = h as f32 / PAUSE_VIRTUAL_H;
    let to_px = |vx: f32, vy: f32| -> (usize, usize) {
        (
            (x_offset + vx * scale_x).round().max(0.0) as usize,
            (vy * scale_y).round().max(0.0) as usize,
        )
    };

    // 1. Optional semi-transparent dark backdrop scrim over paused frame (`sh2gfw_Set_PauseRetain`)
    if draw_scrim {
        rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 96]);
    }

    // `FUN_00407300` (`0x00407300`) uses `FontSize[0]` (`20x30` in `512x480`, `x_scale = scale_x / scale_y`).
    let font_px = (30.0 * scale_y).round().max(14.0) as usize;
    let sub_size = (20.0 * scale_y).round().max(10.0) as usize;
    let x_scale = scale_x / scale_y;
    let pop_dx = (2.0 * scale_x).round().max(1.0) as usize;
    let pop_dy = (2.0 * scale_y).round().max(1.0) as usize;

    match pause_menu.sub_phase {
        PauseSubPhase::Main => {
            // `"PAUSED"` at `(0x100, 0xa4)` (`256, 164`) — `FUN_00407300` `0x00407300:4750-4752`
            font.draw_line_centered_aspect(
                rasterizer,
                "PAUSED",
                to_px(PAUSE_HEADER_XY.0, PAUSE_HEADER_XY.1),
                font_px,
                x_scale,
                Sh2TextColor::White,
                true,
            );

            // Exact `sh2pc.exe` `FUN_00407300` (`0x00407300:4753, 4758`) `fontSetColorDirect` PS2 `0..0x80`
            // colors converted to 8-bit `0..255`:
            // - enabled: `(0x7f, 0x7f, 0x7f, 0x60)` -> `[254, 254, 254, 192]`
            // - disabled Save: `(0x55, 0x55, 0x55, 0x60)` -> `[170, 170, 170, 192]`
            let unselected_color = Sh2TextColor::Custom {
                top: [254, 254, 254, 192],
                bottom: [254, 254, 254, 192],
            };
            let disabled_color = Sh2TextColor::Custom {
                top: [170, 170, 170, 192],
                bottom: [170, 170, 170, 192],
            };

            // 5 items at `(256, 230 + idx * 30)` (`0xe6, 0x104, 0x122, 0x140, 0x15e`)
            for (idx, opt) in PauseMenuOption::ALL.iter().enumerate() {
                let vy = PAUSE_ITEM_START_Y + idx as f32 * PAUSE_ITEM_PITCH_Y;
                let pos = to_px(256.0, vy);
                let is_selected = idx == pause_menu.selected_index;
                let is_disabled = *opt == PauseMenuOption::Save && !pause_menu.can_save;

                let base_color = if is_disabled {
                    disabled_color
                } else {
                    unselected_color
                };

                // Unselected base pass (`alpha = 0x60/0x80 = 192`)
                font.draw_line_centered_aspect(
                    rasterizer,
                    opt.label(),
                    pos,
                    font_px,
                    x_scale,
                    base_color,
                    true,
                );

                // Selected double-draw pop pass at `(-2, -2)` (`0xfe, vy - 2`, `alpha = 0xff`)
                if is_selected && !is_disabled {
                    font.draw_line_centered_aspect(
                        rasterizer,
                        opt.label(),
                        (pos.0.saturating_sub(pop_dx), pos.1.saturating_sub(pop_dy)),
                        font_px,
                        x_scale,
                        Sh2TextColor::White,
                        false,
                    );
                }
            }

            if let Some(ref msg) = pause_menu.status_message {
                font.draw_line_centered_aspect(
                    rasterizer,
                    msg,
                    to_px(256.0, 392.0),
                    sub_size,
                    x_scale,
                    Sh2TextColor::GoldSelected,
                    true,
                );
            }
        }
        PauseSubPhase::QuitConfirm => {
            // `"Exit to main menu?"` at `(0x100, 0xe8)` (`256, 232`) — `FUN_00407610` `0x00407610`
            font.draw_line_centered_aspect(
                rasterizer,
                "Exit to main menu?",
                to_px(PAUSE_QUIT_PROMPT_XY.0, PAUSE_QUIT_PROMPT_XY.1),
                font_px,
                x_scale,
                Sh2TextColor::White,
                true,
            );

            let yes_pos = to_px(PAUSE_QUIT_YES_XY.0, PAUSE_QUIT_YES_XY.1);
            let no_pos = to_px(PAUSE_QUIT_NO_XY.0, PAUSE_QUIT_NO_XY.1);

            // Base unselected pass (`FUN_00407610` `0x00407610`: PS2 `0..0x80` `(0x7f, 0x7f, 0x7f, 0x60)`)
            let unselected_color = Sh2TextColor::Custom {
                top: [254, 254, 254, 192],
                bottom: [254, 254, 254, 192],
            };
            font.draw_line_centered_aspect(
                rasterizer,
                "Yes",
                yes_pos,
                font_px,
                x_scale,
                unselected_color,
                true,
            );
            font.draw_line_centered_aspect(
                rasterizer,
                "No",
                no_pos,
                font_px,
                x_scale,
                unselected_color,
                true,
            );

            // Selected pop pass at `(-2, -2)` (`0xe0, 0x108` or `0x11c, 0x108`, `alpha = 0xff`)
            let (sel_label, sel_pos) = if pause_menu.quit_confirm_index == 0 {
                ("Yes", yes_pos)
            } else {
                ("No", no_pos)
            };
            font.draw_line_centered_aspect(
                rasterizer,
                sel_label,
                (
                    sel_pos.0.saturating_sub(pop_dx),
                    sel_pos.1.saturating_sub(pop_dy),
                ),
                font_px,
                x_scale,
                Sh2TextColor::White,
                false,
            );
        }
        PauseSubPhase::InOptionsMenu => {}
    }
}
