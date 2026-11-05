//! Proportional renderer for the retail Silent Hill 2 text font (`sh2e/font/font000.png`).
//!
//! The Enhanced Edition atlas (`Patches/Fonts.cpp`, `loadExternalFontData`) is a grid of
//! `44 x 64` cells (`width / 44` columns, `height / 64` rows) holding two fonts back to back: the
//! first half is the normal font (`type 0`), the second half the small font (`type 1`). A
//! glyph's cell index is the message byte code, i.e. `chr - 0x20` for ASCII, row-major
//! (`col = id % columns`, `row = id / columns`). Glyph widths come from the pixel bounds of each
//! cell (the game keeps a per-character width table in the exe; the atlas bounds reproduce the
//! proportional spacing) plus the game's 2 px letter spacing (scaled to the 2x atlas).
//!
//! Only the 95 printable ASCII glyphs of the normal font are kept (alpha plane), so the 4400x9600
//! atlas is dropped right after loading. When the retail file is missing (CI), callers fall back
//! to the embedded 8x8 font through [`draw_subtitle`].

use crate::font::TextRasterizer;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const CELL_W: usize = 44;
const CELL_H: usize = 64;
const FIRST_CHAR: u32 = 0x20;
const GLYPH_COUNT: usize = 95;
/// Letter spacing in atlas pixels (`letterSpc = 2` at the game's 22 px cell, doubled for the atlas).
const LETTER_SPACING: usize = 4;
/// Space advance in atlas pixels (`spaceSize = 7` at the game's cell size, doubled).
const SPACE_ADVANCE: usize = 14;

#[derive(Clone)]
struct Glyph {
    /// Alpha plane, `CELL_W * CELL_H`.
    alpha: Vec<u8>,
    /// First and last opaque column (`min > max` for an empty glyph).
    min_x: usize,
    max_x: usize,
}

/// Retail atlas font (normal type 0 glyphs, printable ASCII).
pub struct AtlasFont {
    glyphs: Vec<Glyph>,
}

impl AtlasFont {
    /// Builds the font from an RGBA8 atlas of `width x height` pixels.
    pub fn from_rgba(width: usize, height: usize, rgba: &[u8]) -> Option<Self> {
        let cols = width / CELL_W;
        let rows = height / CELL_H;
        if cols == 0 || rows == 0 || rgba.len() < width * height * 4 {
            return None;
        }
        let mut glyphs = Vec::with_capacity(GLYPH_COUNT);
        for id in 0..GLYPH_COUNT {
            let (col, row) = (id % cols, id / cols);
            if row >= rows {
                return None;
            }
            let mut alpha = vec![0u8; CELL_W * CELL_H];
            let (mut min_x, mut max_x) = (usize::MAX, 0usize);
            for y in 0..CELL_H {
                for x in 0..CELL_W {
                    let src = ((row * CELL_H + y) * width + col * CELL_W + x) * 4 + 3;
                    let a = rgba[src];
                    alpha[y * CELL_W + x] = a;
                    if a > 24 {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                }
            }
            glyphs.push(Glyph {
                alpha,
                min_x,
                max_x,
            });
        }
        Some(Self { glyphs })
    }

    /// Loads and decodes a PNG atlas such as `font000.png`.
    pub fn load_png(path: &Path) -> Option<Self> {
        let img = image::open(path).ok()?.to_rgba8();
        Self::from_rgba(img.width() as usize, img.height() as usize, img.as_raw())
    }

    /// `<root>/sh2e/font/font000.png` inside the retail rip.
    pub fn retail_font_path(root: &Path) -> PathBuf {
        root.join("sh2e/font/font000.png")
    }

    /// Process-wide font loaded once from the retail rip at `root` (None when absent/undecodable).
    pub fn shared(root: &Path) -> Option<&'static AtlasFont> {
        static FONT: OnceLock<Option<AtlasFont>> = OnceLock::new();
        FONT.get_or_init(|| Self::load_png(&Self::retail_font_path(root)))
            .as_ref()
    }

    fn glyph(&self, ch: char) -> Option<&Glyph> {
        let c = ch as u32;
        let id = if (FIRST_CHAR..FIRST_CHAR + GLYPH_COUNT as u32).contains(&c) {
            (c - FIRST_CHAR) as usize
        } else {
            ('?' as u32 - FIRST_CHAR) as usize
        };
        self.glyphs.get(id)
    }

    /// Advance of one character in atlas pixels.
    fn advance(&self, ch: char) -> usize {
        if ch == ' ' {
            return SPACE_ADVANCE;
        }
        match self.glyph(ch) {
            Some(g) if g.min_x <= g.max_x => g.max_x - g.min_x + 1 + LETTER_SPACING,
            _ => SPACE_ADVANCE,
        }
    }

    /// Width in destination pixels of `text` (one line) rendered `px_height` tall.
    pub fn text_width(&self, text: &str, px_height: usize) -> usize {
        let adv: usize = text.chars().map(|c| self.advance(c)).sum();
        adv * px_height / CELL_H
    }

    fn blend(rast: &mut TextRasterizer, x: usize, y: usize, color: [u8; 4], cover: f32) {
        if x >= rast.width || y >= rast.height || cover <= 0.0 {
            return;
        }
        let a = (cover * color[3] as f32 / 255.0).clamp(0.0, 1.0);
        let o = (y * rast.width + x) * 4;
        let da = rast.pixels[o + 3] as f32 / 255.0;
        let out_a = a + da * (1.0 - a);
        if out_a <= 0.0 {
            return;
        }
        for (c, &src) in color.iter().enumerate().take(3) {
            let d = rast.pixels[o + c] as f32;
            let v = (src as f32 * a + d * da * (1.0 - a)) / out_a;
            rast.pixels[o + c] = v.round().clamp(0.0, 255.0) as u8;
        }
        rast.pixels[o + 3] = (out_a * 255.0).round() as u8;
    }

    fn draw_glyph(
        g: &Glyph,
        rast: &mut TextRasterizer,
        x: usize,
        y: usize,
        px_height: usize,
        color: [u8; 4],
    ) {
        if g.min_x > g.max_x {
            return;
        }
        let step = CELL_H as f32 / px_height as f32; // source pixels per destination pixel
        let w = ((g.max_x - g.min_x + 1) as f32 / step).ceil() as usize;
        for dy in 0..px_height {
            let sy0 = dy as f32 * step;
            let sy1 = sy0 + step;
            for dx in 0..w {
                let sx0 = g.min_x as f32 + dx as f32 * step;
                let sx1 = sx0 + step;
                let mut sum = 0.0;
                let mut area = 0.0;
                let mut sy = sy0.floor() as usize;
                while (sy as f32) < sy1 && sy < CELL_H {
                    let wy = (sy1.min(sy as f32 + 1.0) - sy0.max(sy as f32)).max(0.0);
                    let mut sx = sx0.floor() as usize;
                    while (sx as f32) < sx1 {
                        let wx = (sx1.min(sx as f32 + 1.0) - sx0.max(sx as f32)).max(0.0);
                        let a = if sx < CELL_W {
                            g.alpha[sy * CELL_W + sx] as f32 / 255.0
                        } else {
                            0.0
                        };
                        sum += a * wx * wy;
                        area += wx * wy;
                        sx += 1;
                    }
                    sy += 1;
                }
                if area > 0.0 {
                    Self::blend(rast, x + dx, y + dy, color, sum / area);
                }
            }
        }
    }

    /// Draws one line of text with its top-left at `pos`; optional drop shadow like the menus.
    pub fn draw_line(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        color: [u8; 4],
        shadow: bool,
    ) {
        let (x, y) = pos;
        let px_height = px_height.max(4);
        let off = (px_height / 16).max(1);
        let passes: &[(usize, [u8; 4])] =
            &[(off, [0, 0, 0, (color[3] as f32 * 0.85) as u8]), (0, color)];
        for &(o, col) in passes.iter().skip(if shadow { 0 } else { 1 }) {
            let mut cx = x;
            for ch in text.chars() {
                if ch != ' ' {
                    if let Some(g) = self.glyph(ch) {
                        Self::draw_glyph(g, rast, cx + o, y + o, px_height, col);
                    }
                }
                cx += self.advance(ch) * px_height / CELL_H;
            }
        }
    }
}

/// Draws a (possibly multi-line) subtitle centred on `center_x` with its last line ending at
/// `bottom_y`. Uses the retail atlas font from the rip at `game_root` when available and falls back
/// to the embedded 8x8 font otherwise. `px_height` is the line height in pixels.
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
    let line_h = px_height + px_height / 4;
    let top = bottom_y.saturating_sub(line_h * lines.len());
    let font = game_root.and_then(AtlasFont::shared);
    for (i, line) in lines.iter().enumerate() {
        let y = top + i * line_h;
        match font {
            Some(f) => {
                let w = f.text_width(line, px_height);
                f.draw_line(
                    rast,
                    line,
                    (center_x.saturating_sub(w / 2), y),
                    px_height,
                    color,
                    true,
                );
            }
            None => {
                let scale = (px_height / 16).max(1);
                let w = line.chars().count() * 8 * scale;
                rast.draw_text(line, center_x.saturating_sub(w / 2), y, scale, color, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_atlas() -> (usize, usize, Vec<u8>) {
        let (w, h) = (CELL_W * 10, CELL_H * 10);
        let mut px = vec![0u8; w * h * 4];
        // glyph id 33 ('A'): a 10 px wide opaque block at x=5..15
        let (col, row) = (33 % 10, 33 / 10);
        for y in 10..50 {
            for x in 5..15 {
                px[((row * CELL_H + y) * w + col * CELL_W + x) * 4 + 3] = 255;
            }
        }
        (w, h, px)
    }

    #[test]
    fn proportional_widths_and_draw() {
        let (w, h, px) = synthetic_atlas();
        let f = AtlasFont::from_rgba(w, h, &px).unwrap();
        assert_eq!(f.advance('A'), 10 + LETTER_SPACING);
        assert_eq!(f.advance(' '), SPACE_ADVANCE);
        let mut r = TextRasterizer::new(64, 64);
        f.draw_line(&mut r, "A", (4, 4), 32, [255, 255, 255, 255], false);
        assert!(r.pixels.chunks(4).any(|p| p[3] > 200));
    }

    #[test]
    fn subtitle_falls_back_without_font() {
        let mut r = TextRasterizer::new(320, 120);
        draw_subtitle(
            &mut r,
            Some(Path::new("/nonexistent/sh2")),
            "Hi\nthere",
            160,
            110,
            32,
            [255; 4],
        );
        assert!(r.pixels.chunks(4).any(|p| p[3] != 0));
    }

    #[test]
    fn retail_font_renders_when_present() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
        let Some(f) = AtlasFont::load_png(&AtlasFont::retail_font_path(&root)) else {
            return; // retail assets absent (CI)
        };
        assert!(f.advance('i') < f.advance('W'));
        let mut r = TextRasterizer::new(400, 60);
        f.draw_line(&mut r, "Mary...", (4, 4), 32, [255; 4], true);
        assert!(r.pixels.chunks(4).filter(|p| p[3] > 128).count() > 100);
    }
}
