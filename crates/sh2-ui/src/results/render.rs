//! Renderer for the 10-Star Post-Game Results Screen (`SH2_common/result.c` `ResultMain` `0x00293e40`).

use super::state::ResultsScreen;
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::widgets::Ps2Canvas;

/// `result_message` IDs from `SLUS_202.28` (`0x0037d1b0`).
pub const RESULT_MSG_IDS: [u16; 15] = [1, 2, 6, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
/// Right-aligned label X in `512x448` space (`0xf0 = 240` at `0x00293e40`).
pub const RESULT_LABEL_X_512: f32 = 240.0;
/// Left-aligned value X in `512x448` space (`0x100 = 256` at `0x00293e40`).
pub const RESULT_VALUE_X_512: f32 = 256.0;
/// First row Y in `512x448` space (`0x48 = 72` at `0x00293e40`).
pub const RESULT_ROW_Y0_512: f32 = 72.0;
/// Row pitch in `512x448` space (`0x1a = 26` at `0x00293e40`).
pub const RESULT_ROW_PITCH_512: f32 = 26.0;

/// PS2 `fontSetColor(7)` Blue->Green header gradient (`0x002baec0`).
const COLOR_HEADER_BLUE_GREEN: Sh2TextColor = Sh2TextColor::Custom {
    top: [128, 128, 255, 255],
    bottom: [128, 255, 128, 255],
};
/// PS2 `fontSetColor(10)` White->Dark Grey label gradient (`0x002baec0`).
const COLOR_LABEL_WHITE_GREY: Sh2TextColor = Sh2TextColor::Custom {
    top: [230, 230, 230, 255],
    bottom: [96, 96, 96, 255],
};
/// PS2 `color2()` / `fontSetColor(4)` Yellow (`0x002baec0`).
const COLOR_MAX_YELLOW: Sh2TextColor = Sh2TextColor::GoldSelected;

/// Renders the 1:1 PS2 `ResultMain` (`0x00293e40`) 10-Star Results screen onto `rasterizer`.
pub fn draw_results_screen(
    screen: &ResultsScreen,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    // `clear_screen()` (`0x00293d20`): pure black background.
    rasterizer.clear();
    rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);

    let c = Ps2Canvas::from_viewport(rasterizer.width, rasterizer.height);
    let title_px = ((24.0 * c.sy).round() as usize).max(14);
    let row_px = ((16.0 * c.sy).round() as usize).max(10);
    let big_star_px = ((22.0 * c.sy).round() as usize).max(12);
    let small_star_px = ((14.0 * c.sy).round() as usize).max(9);

    // Header `"RESULTS"` (`fontSetColor(7)` Blue->Green gradient, `font_print(0, 0x100, 0x20, 1)`).
    font.draw_line_centered(
        rasterizer,
        "RESULTS",
        (c.xi(256.0), c.yi(24.0)),
        title_px,
        COLOR_HEADER_BLUE_GREEN,
        true,
    );

    let label_x = c.xi(RESULT_LABEL_X_512);
    let value_x = c.xi(RESULT_VALUE_X_512);

    for (idx, row) in screen.rows.iter().enumerate() {
        let y = c.yi(RESULT_ROW_Y0_512 + (idx as f32) * RESULT_ROW_PITCH_512);
        // Right-aligned label (`fontSetColor(10)` White->Grey).
        let lw = font.text_width(row.label, row_px);
        font.draw_line_styled(
            rasterizer,
            row.label,
            (label_x.saturating_sub(lw), y),
            row_px,
            COLOR_LABEL_WHITE_GREY,
            true,
        );

        if idx < 14 {
            // `color1()` = White, `color2()` = Yellow (`fontSetColor(4)`).
            let color = if row.is_max_rank {
                COLOR_MAX_YELLOW
            } else {
                Sh2TextColor::White
            };
            font.draw_line_styled(rasterizer, &row.value, (value_x, y), row_px, color, true);
        }
    }

    // Row 14 (`Your Rank`): Big Stars (`total / 10`) + Small Stars (`total % 10`) in `fontSetColor(4)` Yellow.
    let big = screen.breakdown.big_stars() as usize;
    let small = screen.breakdown.small_stars() as usize;
    if screen.breakdown.total_points < 10 {
        let stars: String = "*".repeat(small);
        font.draw_line_styled(
            rasterizer,
            &stars,
            (value_x, c.yi(436.0)),
            small_star_px,
            COLOR_MAX_YELLOW,
            true,
        );
    } else {
        let big_str: String = "*".repeat(big);
        font.draw_line_styled(
            rasterizer,
            &big_str,
            (value_x, c.yi(432.0)),
            big_star_px,
            COLOR_MAX_YELLOW,
            true,
        );
        if small > 0 {
            let small_str: String = "*".repeat(small);
            font.draw_line_styled(
                rasterizer,
                &small_str,
                (
                    value_x,
                    c.yi(458.0)
                        .min(rasterizer.height.saturating_sub(small_star_px + 2)),
                ),
                small_star_px,
                COLOR_MAX_YELLOW,
                true,
            );
        }
    }
}
