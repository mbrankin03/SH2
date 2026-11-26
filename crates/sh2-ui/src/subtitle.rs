//! 1:1 PS2 / SH2:EE subtitle layout, safe-area anchor, and rendering (`SLUS_202.28`
//! `fontPrintStrMain` `0x001b4a10`, `sh2pc.exe` `FUN_0047faa0` `0x0047faa0` & `FUN_00481630`
//! `0x00481630`).

use std::path::Path;

use crate::atlas_font::{AtlasFont, PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL};
use crate::font::TextRasterizer;

/// Virtual horizontal resolution of the PS2/PC font coordinate system (`sh2pc.exe` `FUN_00481630`
/// `0x00481630`: `0x001953125 = 1.0 / 512.0`, `X ∈ [0x700, 0x900]`).
pub const PS2_FONT_VIRTUAL_WIDTH: f32 = 512.0;

/// Virtual vertical resolution of the PC font coordinate system (`sh2pc.exe` `FUN_00481630`
/// `0x00481630`: `0.0020833334 = 1.0 / 480.0`, `Y_origin = 0x710 = 1808.0`).
pub const PS2_FONT_VIRTUAL_HEIGHT: f32 = 480.0;

/// Virtual GS Y-origin subtracted when mapping PS2/PC font coordinates to `0..480` screen space
/// (`sh2pc.exe` `FUN_00481630` `0x00481630`: `1808.0 = 0x710`).
pub const PS2_FONT_GS_Y_ORIGIN: i32 = 0x710;

/// Default bottom-centered subtitle Y anchor in PS2 GS coordinates
/// (`SLUS_202.28` `fontPrintStrMain` `0x001b4a10`, `sh2pc.exe` `FUN_0047faa0` `0x0047faa0`).
pub const PS2_SUBTITLE_Y_ANCHOR_GS: i32 = 0x8DC;

/// Base Normal font glyph width in virtual `512×480` pixels (`SLUS_202.28` `FontSize[0]` at
/// `0x002bacd8`, `sh2pc.exe` `0x00803898`, SH2:EE `NormalFontWidth`).
pub const PS2_NORMAL_FONT_WIDTH: i32 = 20;

/// Base Normal font line height in virtual `512×480` pixels (`SLUS_202.28` `FontSize[0]` at
/// `0x002bacd8`, `sh2pc.exe` `0x00803898`, SH2:EE `NormalFontHeight`).
pub const PS2_NORMAL_FONT_HEIGHT: i32 = 30;

/// Horizontal pixel aspect ratio mapping `512×480` virtual font pixels onto a square-pixel `4:3`
/// display (`(4/3) * (480 / 512) = 5/4 = 1.25`, `sh2pc.exe` `FUN_00481630` `0x00481630`).
pub const PS2_FONT_PIXEL_ASPECT_4X3: f32 = 1.25;

/// Default subtitle RGBA color: pure white from `SLUS_202.28` `fontSetColor(0)` (`0x001b73a0`,
/// table `@1369` at `0x002baec0`: PS2 GS `0x808080` = unity `1.0` white).
pub const PS2_SUBTITLE_COLOR_WHITE: [u8; 4] = [255, 255, 255, 255];

/// Computed 1:1 PS2/SH2:EE subtitle layout metrics for a destination rasterizer height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ps2SubtitleLayout {
    /// Atlas cell pixel height (`32 * canvas_h / 480`) passed to [`AtlasFont`] draw methods so
    /// the `30`-virtual-px glyph body and `RETAIL_FONT0_WIDTHS` advances map 1:1 with `sh2pc.exe`
    /// `FUN_00481630` (`0x00481630`).
    pub px_height: usize,
    /// Line-to-line vertical step in pixels (`30 * canvas_h / 480`, exact `uStack_f0` advance in
    /// `SLUS_202.28` `fontPrintStrMain` `0x001b4a10`).
    pub line_step_px: usize,
    /// Top Y pixel coordinate of line 0 in destination canvas space.
    pub top_y_px: usize,
    /// Horizontal pixel aspect ratio (`1.25` for square-pixel `4:3` / SH2:EE widescreen UI).
    pub x_scale: f32,
}

/// Computes the exact 1:1 subtitle top-Y in `0..480` virtual screen coordinates from
/// `SLUS_202.28` `fontPrintStrMain` (`0x001b4a10`) and retail `sh2pc.exe` `FUN_0047faa0`
/// (`0x0047faa0`).
///
/// Prototype vs Retail PC drift:
/// `SLUS_202.28` uses `0x8DC - ((line_count + 4) * 30) / 2` unconditionally. Retail `sh2pc.exe`
/// (`0x0047faa0`) preserves that exact formula for `1..=3` lines (`y_480 = 385, 370, 355`, centered
/// at `y = 400 / 480`) and adds `- ((line_count - 3) * 30) / 2` for `line_count > 3` so 4+ line
/// blocks stay pinned at the `445 / 480` bottom safe area.
#[must_use]
pub const fn ps2_subtitle_top_y_480(line_count: usize) -> i32 {
    let n = if line_count == 0 {
        1
    } else {
        line_count as i32
    };
    let mut y_gs = PS2_SUBTITLE_Y_ANCHOR_GS - ((n + 4) * PS2_NORMAL_FONT_HEIGHT) / 2;
    if n > 3 {
        y_gs -= ((n - 3) * PS2_NORMAL_FONT_HEIGHT) / 2;
    }
    y_gs - PS2_FONT_GS_Y_ORIGIN
}

/// Computes the 1:1 PS2/SH2:EE [`Ps2SubtitleLayout`] for a destination canvas of height
/// `canvas_height` and a subtitle block with `line_count` lines.
#[must_use]
pub fn compute_ps2_subtitle_layout(canvas_height: usize, line_count: usize) -> Ps2SubtitleLayout {
    let scale_y = canvas_height.max(1) as f32 / PS2_FONT_VIRTUAL_HEIGHT;
    let px_height = (PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL * scale_y)
        .round()
        .max(8.0) as usize;
    let line_step_px = (PS2_NORMAL_FONT_HEIGHT as f32 * scale_y).round().max(8.0) as usize;
    let top_y_px = (ps2_subtitle_top_y_480(line_count).max(0) as f32 * scale_y).round() as usize;
    Ps2SubtitleLayout {
        px_height,
        line_step_px,
        top_y_px,
        x_scale: PS2_FONT_PIXEL_ASPECT_4X3,
    }
}

/// Draws a (possibly multi-line) subtitle using the exact 1:1 PS2/SH2:EE layout, glyph scale,
/// horizontal `4:3` pixel aspect (`1.25`), line spacing (`30 / 480`), bottom safe-area anchor
/// (`0x8DC` in `fontPrintStrMain` `0x001b4a10` / `sh2pc.exe` `0x0047faa0`), and `(+1, +1)` virtual
/// pixel drop shadow (`fontFlush` `0x001b7530`).
pub fn draw_subtitle_ps2(
    rast: &mut TextRasterizer,
    game_root: Option<&Path>,
    text: &str,
    color: [u8; 4],
) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return;
    }
    let layout = compute_ps2_subtitle_layout(rast.height, lines.len());
    let center_x = rast.width / 2;
    let font = AtlasFont::shared(game_root);
    for (i, line) in lines.iter().enumerate() {
        let y = layout.top_y_px + i * layout.line_step_px;
        let w = font.text_width_aspect(line, layout.px_height, layout.x_scale);
        font.draw_line_gradient_aspect(
            rast,
            line,
            (center_x.saturating_sub(w / 2), y),
            layout.px_height,
            layout.x_scale,
            (color, color),
            true,
        );
    }
}

/// Draws a (possibly multi-line) subtitle centred on `center_x` with its last line ending at
/// `bottom_y`. Uses the authentic proportional Silent Hill 2 font (via `AtlasFont::shared`) with
/// 1:1 line step (`30 / 32` of atlas cell height matching `fontPrintStrMain` `0x001b4a10`).
pub fn draw_subtitle(
    rast: &mut TextRasterizer,
    game_root: Option<&Path>,
    text: &str,
    center_x: usize,
    bottom_y: usize,
    px_height: usize,
    color: [u8; 4],
) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return;
    }
    let line_h = ((px_height as f32) * (PS2_NORMAL_FONT_HEIGHT as f32)
        / PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL)
        .round()
        .max(1.0) as usize;
    let top = bottom_y.saturating_sub(line_h * lines.len());
    let font = AtlasFont::shared(game_root);
    for (i, line) in lines.iter().enumerate() {
        let y = top + i * line_h;
        let w = font.text_width_aspect(line, px_height, PS2_FONT_PIXEL_ASPECT_4X3);
        font.draw_line_gradient_aspect(
            rast,
            line,
            (center_x.saturating_sub(w / 2), y),
            px_height,
            PS2_FONT_PIXEL_ASPECT_4X3,
            (color, color),
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps2_subtitle_top_y_480_matches_slus_and_sh2pc_golden_vectors() {
        // SLUS_202.28 fontPrintStrMain (0x001b4a10) & sh2pc.exe FUN_0047faa0 (0x0047faa0):
        // y_480 = (0x8DC - ((n + 4) * 30) / 2) - 0x710, with PC 4+ line clamp.
        assert_eq!(ps2_subtitle_top_y_480(1), 385);
        assert_eq!(ps2_subtitle_top_y_480(2), 370);
        assert_eq!(ps2_subtitle_top_y_480(3), 355);
        // Retail PC 4+ line clamp pins bottom at 445/480:
        assert_eq!(ps2_subtitle_top_y_480(4), 325);
        assert_eq!(ps2_subtitle_top_y_480(4) + 4 * PS2_NORMAL_FONT_HEIGHT, 445);
        assert_eq!(ps2_subtitle_top_y_480(5), 295);
        assert_eq!(ps2_subtitle_top_y_480(5) + 5 * PS2_NORMAL_FONT_HEIGHT, 445);
    }

    #[test]
    fn compute_ps2_subtitle_layout_scales_linearly_with_canvas_height() {
        let l480 = compute_ps2_subtitle_layout(480, 1);
        assert_eq!(l480.px_height, 32);
        assert_eq!(l480.line_step_px, 30);
        assert_eq!(l480.top_y_px, 385);
        assert!((l480.x_scale - 1.25).abs() < 1e-6);

        let l720 = compute_ps2_subtitle_layout(720, 2);
        assert_eq!(l720.px_height, 48);
        assert_eq!(l720.line_step_px, 45);
        assert_eq!(l720.top_y_px, 555);

        let l1440 = compute_ps2_subtitle_layout(1440, 2);
        assert_eq!(l1440.px_height, 96);
        assert_eq!(l1440.line_step_px, 90);
        assert_eq!(l1440.top_y_px, 1110);
    }

    #[test]
    fn draw_subtitle_ps2_renders_pure_white_and_black_drop_shadow() {
        let mut r = TextRasterizer::new(1920, 1440);
        draw_subtitle_ps2(
            &mut r,
            None,
            "Mary...\nCould you really be in this town?",
            PS2_SUBTITLE_COLOR_WHITE,
        );

        let mut has_white = false;
        let mut has_black_shadow = false;
        let mut min_y = usize::MAX;
        let mut max_y = 0usize;
        for y in 0..r.height {
            for x in 0..r.width {
                let o = (y * r.width + x) * 4;
                let px = &r.pixels[o..o + 4];
                if px[3] > 200 {
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                    if px[0] > 240 && px[1] > 240 && px[2] > 240 {
                        has_white = true;
                    }
                    if px[0] < 16 && px[1] < 16 && px[2] < 16 {
                        has_black_shadow = true;
                    }
                }
            }
        }
        assert!(has_white, "expected pure white foreground glyph pixels");
        assert!(
            has_black_shadow,
            "expected pure black 100%-alpha drop shadow pixels"
        );
        // 2-line layout at 1440p spans y = 1110..1296 (with built-in cell top/bottom leading).
        assert!(
            min_y >= 1110 && max_y <= 1300,
            "unexpected vertical bounds {min_y}..{max_y}"
        );

        // Optional visual verification snapshots when SH2_SUBTITLE_SNAPSHOT_DIR is set.
        if let Ok(dir) = std::env::var("SH2_SUBTITLE_SNAPSHOT_DIR") {
            let out_dir = Path::new(&dir);
            let render_snapshot = |filename: &str, text: &str| {
                let mut canvas = TextRasterizer::new(1920, 1440);
                // Dark moody observation-deck slate background so white text + black drop shadow
                // are clearly inspectable side-by-side.
                canvas.fill_rect(0, 0, 1920, 1440, [42, 48, 52, 255]);
                draw_subtitle_ps2(&mut canvas, None, text, PS2_SUBTITLE_COLOR_WHITE);
                let img = image::RgbaImage::from_raw(1920, 1440, canvas.pixels)
                    .expect("valid RGBA buffer");
                img.save(out_dir.join(filename))
                    .expect("save subtitle snapshot PNG");
            };
            render_snapshot(
                "sh2_163_subtitle_single_line_1440p.png",
                "Mary... Could you really be in this town?",
            );
            render_snapshot(
                "sh2_163_subtitle_multi_line_1440p.png",
                "In my restless dreams,\nI see that town.\nSilent Hill.",
            );
        }
    }
}
