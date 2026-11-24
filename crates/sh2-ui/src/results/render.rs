//! Renderer for the 10-Star Post-Game Results Screen (`SH2_common/result.c` `ResultMain` `0x00293e40`).

use super::state::ResultsScreen;
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;

/// `result_message` IDs from `SLUS_202.28` (`0x0037d1b0`).
pub const RESULT_MSG_IDS: [u16; 15] = [1, 2, 6, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
/// Right-aligned label X in `512x480` space (`0xf0 = 240` at `0x00293e40`).
pub const RESULT_LABEL_X_512: f32 = 240.0;
/// Left-aligned value X in `512x480` space (`0x100 = 256` at `0x00293e40`).
pub const RESULT_VALUE_X_512: f32 = 256.0;
/// First row Y in `512x480` space (`0x48 = 72` at `0x00293e40`).
pub const RESULT_ROW_Y0_512: f32 = 72.0;
/// Row pitch in `512x480` space (`0x1a = 26` at `0x00293e40`).
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

    // `ResultMain` (`0x00293e40`) places Big Stars at `y = 0x1b0 (432)` and Small Stars at
    // `y = 0x1ca (458)`, so map `512x480` virtual coordinates cleanly onto the viewport.
    let sx = (rasterizer.width.max(1) as f32) / 512.0;
    let sy = (rasterizer.height.max(1) as f32) / 480.0;
    let xi = |x: f32| (x * sx).round().max(0.0) as usize;
    let yi = |y: f32| (y * sy).round().max(0.0) as usize;

    let title_px = ((24.0 * sy).round() as usize).max(14);
    let row_px = ((16.0 * sy).round() as usize).max(10);

    // Header `"RESULTS"` (`fontSetColor(7)` Blue->Green gradient, `font_print(0, 0x100, 0x20, 1)`).
    font.draw_line_centered(
        rasterizer,
        "RESULTS",
        (xi(256.0), yi(32.0)),
        title_px,
        COLOR_HEADER_BLUE_GREEN,
        true,
    );

    let label_x = xi(RESULT_LABEL_X_512);
    let value_x = xi(RESULT_VALUE_X_512);

    for (idx, row) in screen.rows.iter().enumerate() {
        let y = yi(RESULT_ROW_Y0_512 + (idx as f32) * RESULT_ROW_PITCH_512);
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

    // Row 14 (`Your Rank`): Big Stars (`total / 10`) + Small Stars (`total % 10`) in `fontSetColor(4)` Yellow
    // (`0x138` star glyph in `ResultMain` `0x00293e40`).
    let big = screen.breakdown.big_stars() as usize;
    let small = screen.breakdown.small_stars() as usize;
    if screen.breakdown.total_points < 10 {
        let r = (7.0 * sy).max(4.0);
        let pitch = (16.0 * sx).max(9.0);
        let cy = (444.0 * sy).round();
        for i in 0..small {
            let cx = (value_x as f32) + r + (i as f32) * pitch;
            draw_five_point_star(rasterizer, cx, cy, r, [255, 220, 72, 255]);
        }
    } else {
        let big_r = (10.0 * sy).max(6.0);
        let big_pitch = (22.0 * sx).max(12.0);
        let big_cy = (442.0 * sy).round();
        for i in 0..big {
            let cx = (value_x as f32) + big_r + (i as f32) * big_pitch;
            draw_five_point_star(rasterizer, cx, big_cy, big_r, [255, 220, 72, 255]);
        }
        if small > 0 {
            let small_r = (6.5 * sy).max(4.0);
            let small_pitch = (15.0 * sx).max(9.0);
            let small_cy = (464.0 * sy).round();
            for i in 0..small {
                let cx = (value_x as f32) + small_r + (i as f32) * small_pitch;
                draw_five_point_star(rasterizer, cx, small_cy, small_r, [255, 220, 72, 255]);
            }
        }
    }
}

/// Rasterizes a 5-pointed star (`0x138` glyph in `ResultMain` `0x00293e40`) centered at `(cx, cy)`.
fn draw_five_point_star(rast: &mut TextRasterizer, cx: f32, cy: f32, outer_r: f32, color: [u8; 4]) {
    let inner_r = outer_r * 0.4;
    let mut verts = [(0.0f32, 0.0f32); 10];
    for (i, v) in verts.iter_mut().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + (i as f32) * (std::f32::consts::PI / 5.0);
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        *v = (cx + r * angle.cos(), cy + r * angle.sin());
    }
    let min_x = ((cx - outer_r).floor().max(0.0) as usize).min(rast.width);
    let max_x = ((cx + outer_r).ceil().max(0.0) as usize).min(rast.width);
    let min_y = ((cy - outer_r).floor().max(0.0) as usize).min(rast.height);
    let max_y = ((cy + outer_r).ceil().max(0.0) as usize).min(rast.height);

    for py in min_y..max_y {
        let y = py as f32 + 0.5;
        for px in min_x..max_x {
            let x = px as f32 + 0.5;
            if point_in_polygon(x, y, &verts) {
                rast.blend_pixel(px, py, color);
            }
        }
    }
}

fn point_in_polygon(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}
