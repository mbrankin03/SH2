//! Proportional renderer for the authentic Silent Hill 2 text font (`font000.png` / embedded fallback).
//!
//! The font atlas is a grid of `44 x 64` cells holding the retail normal font (`type 0`). A glyph's
//! cell index is the message byte code, i.e. `chr - 0x20` for ASCII.
//!
//! # Authentic Character Proportions & Spacing
//! Silent Hill 2's engine (`sh2pc.exe` @ `0x803920` and PS2 prototype `SLUS_202.28` @ `0x002BAF48`)
//! stores a 224-byte proportional width table for the 20x30 Normal font (`FONT_DATA`). In the
//! PC / PS2 engine:
//! - Character advance is `width + 2` (or `spaceSize = 7` for space).
//! - For the 2x resolution font atlas (`44 x 64` cell), widths and letter spacing scale 2x:
//!   `advance_atlas = (RETAIL_FONT0_WIDTHS[id] * 2) + LETTER_SPACING` (with `LETTER_SPACING = 4`, `SPACE = 14`).
//! - The authentic 224-glyph font image is embedded directly in `sh2-ui` so high-fidelity proportional
//!   text renders out-of-the-box regardless of external file layouts or mod configurations.

use crate::font::TextRasterizer;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const CELL_W: usize = 44;
const CELL_H: usize = 64;
const FIRST_CHAR: u32 = 0x20;
const GLYPH_COUNT: usize = 224;

/// Authentic Silent Hill 2 text color palette from retail engine `0x47eca0` color table.
/// Each palette entry defines a `(top_color, bottom_color)` RGBA pair for two-tone gradient shading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sh2TextColor {
    /// Index 0: Pure / Crisp White
    White,
    /// Selected interactive menu item: Authentic vibrant Amber / Gold highlight.
    GoldSelected,
    /// Unselected interactive menu item: Authentic dim Silver / White with subtle drop gradient.
    SilverUnselected,
    /// Header / Title: Amber / Gold header
    AmberHeader,
    /// Muted prompt footer / instructions: Subtle gray
    MutedFooter,
    /// Cyan info / stats
    Cyan,
    /// Green status / playback
    Green,
    /// Custom RGBA pair
    Custom { top: [u8; 4], bottom: [u8; 4] },
}

impl Sh2TextColor {
    /// Returns the `(top_color, bottom_color)` RGBA values for this color style.
    pub const fn to_gradient(self) -> ([u8; 4], [u8; 4]) {
        match self {
            Self::White => ([240, 240, 240, 255], [210, 210, 210, 255]),
            // Authentic SH2 selected menu item: bright gold top fading into warm amber bottom
            Self::GoldSelected => ([255, 230, 90, 255], [220, 160, 30, 255]),
            // Authentic SH2 unselected menu item: silver-white top fading into soft shadow gray bottom
            Self::SilverUnselected => ([210, 210, 215, 220], [140, 140, 145, 200]),
            // Amber title header
            Self::AmberHeader => ([255, 215, 70, 255], [230, 170, 40, 255]),
            // Muted footer prompt text
            Self::MutedFooter => ([160, 160, 165, 210], [120, 120, 125, 190]),
            // Cyan debug / info
            Self::Cyan => ([120, 225, 245, 255], [70, 170, 210, 255]),
            // Green debug / status
            Self::Green => ([120, 240, 140, 255], [70, 190, 90, 255]),
            Self::Custom { top, bottom } => (top, bottom),
        }
    }
}
const LETTER_SPACING: usize = 4;
/// Space advance in atlas pixels (`spaceSize = 7` at the game's cell size, doubled).
const SPACE_ADVANCE: usize = 14;

/// The authentic 224-byte proportional character width table extracted from `sh2pc.exe` at `0x803920`
/// and verified against PS2 prototype `SLUS_202.28` `FontDataTable[0]` at `0x002BAF48`.
pub const RETAIL_FONT0_WIDTHS: [u8; 224] = [
    0, 5, 7, 10, 9, 14, 13, 4, 6, 6, 7, 10, 5, 6, 5, 10, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 5, 5, 10,
    10, 10, 8, 13, 13, 11, 12, 13, 11, 10, 13, 14, 6, 6, 12, 11, 16, 14, 13, 10, 13, 11, 9, 11, 13,
    13, 17, 11, 11, 11, 6, 10, 6, 10, 9, 6, 9, 10, 8, 11, 8, 6, 10, 10, 5, 5, 10, 5, 15, 10, 9, 10,
    10, 7, 7, 6, 10, 10, 14, 9, 10, 9, 6, 10, 6, 10, 0, 9, 0, 5, 9, 9, 17, 9, 9, 6, 19, 9, 6, 17,
    0, 0, 0, 0, 5, 5, 9, 9, 10, 9, 17, 6, 17, 7, 6, 14, 0, 0, 11, 0, 5, 9, 9, 10, 9, 10, 9, 6, 13,
    6, 9, 10, 6, 11, 9, 7, 9, 6, 6, 6, 10, 11, 5, 6, 6, 6, 9, 13, 13, 13, 8, 13, 13, 13, 13, 13,
    13, 16, 12, 11, 11, 11, 11, 6, 6, 6, 6, 13, 14, 13, 13, 13, 13, 13, 10, 14, 13, 13, 13, 13, 11,
    10, 10, 9, 9, 9, 9, 9, 9, 13, 8, 8, 8, 8, 8, 5, 5, 5, 6, 9, 10, 9, 9, 9, 9, 9, 9, 10, 10, 10,
    10, 10, 10, 10, 0,
];

/// Embedded grayscale atlas PNG for the 224 glyphs of the normal font (32 KB).
const EMBEDDED_FONT_PNG: &[u8] = include_bytes!("../assets/font_normal.png");

#[derive(Clone)]
struct Glyph {
    /// Alpha plane, `CELL_W * CELL_H`.
    alpha: Vec<u8>,
    /// First and last opaque column (`min > max` for an empty glyph).
    min_x: usize,
    max_x: usize,
    /// Proportional advance in atlas pixels.
    advance: usize,
}

/// Retail atlas font (normal type 0 glyphs).
pub struct AtlasFont {
    glyphs: Vec<Glyph>,
}

impl AtlasFont {
    /// Builds the font from an RGBA8 or Grayscale atlas of `width x height` pixels.
    pub fn from_image_bytes(
        width: usize,
        height: usize,
        channels: usize,
        data: &[u8],
    ) -> Option<Self> {
        let cols = width / CELL_W;
        let rows = height / CELL_H;
        if cols == 0 || rows == 0 || data.len() < width * height * channels {
            return None;
        }
        let total = (cols * rows).min(GLYPH_COUNT);
        let mut glyphs = Vec::with_capacity(total);
        #[allow(clippy::needless_range_loop)]
        for id in 0..total {
            let (col, row) = (id % cols, id / cols);
            let mut alpha = vec![0u8; CELL_W * CELL_H];
            let (mut min_x, mut max_x) = (usize::MAX, 0usize);
            for y in 0..CELL_H {
                for x in 0..CELL_W {
                    let src = ((row * CELL_H + y) * width + col * CELL_W + x) * channels;
                    let a = if channels >= 4 {
                        data[src + 3]
                    } else {
                        data[src]
                    };
                    alpha[y * CELL_W + x] = a;
                    if a > 24 {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                }
            }
            let advance = if id == 0 {
                SPACE_ADVANCE
            } else if id < RETAIL_FONT0_WIDTHS.len() && RETAIL_FONT0_WIDTHS[id] > 0 {
                (RETAIL_FONT0_WIDTHS[id] as usize * 2) + LETTER_SPACING
            } else if min_x <= max_x {
                (max_x - min_x + 1) + LETTER_SPACING
            } else {
                SPACE_ADVANCE
            };

            glyphs.push(Glyph {
                alpha,
                min_x,
                max_x,
                advance,
            });
        }
        Some(Self { glyphs })
    }

    /// Loads and decodes an external PNG atlas such as `font000.png`.
    pub fn load_png(path: &Path) -> Option<Self> {
        let img = image::open(path).ok()?.to_rgba8();
        Self::from_image_bytes(img.width() as usize, img.height() as usize, 4, img.as_raw())
    }

    /// Loads the built-in embedded font atlas.
    pub fn load_embedded() -> Self {
        let img = image::load_from_memory(EMBEDDED_FONT_PNG)
            .expect("embedded font_normal.png must be valid")
            .to_luma8();
        Self::from_image_bytes(img.width() as usize, img.height() as usize, 1, img.as_raw())
            .expect("embedded font atlas should parse")
    }

    /// `<root>/sh2e/font/font000.png` inside the retail rip.
    pub fn retail_font_path(root: &Path) -> PathBuf {
        root.join("sh2e/font/font000.png")
    }

    /// Process-wide font loaded from candidate game roots or falling back to the embedded authentic atlas.
    pub fn shared(game_root: Option<&Path>) -> &'static AtlasFont {
        static FONT: OnceLock<AtlasFont> = OnceLock::new();
        FONT.get_or_init(|| {
            // 1. Try provided game_root
            if let Some(root) = game_root {
                let p = Self::retail_font_path(root);
                if p.is_file() {
                    if let Some(f) = Self::load_png(&p) {
                        return f;
                    }
                }
            }
            // 2. Try known workspace candidate paths
            let candidates = [
                PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
                PathBuf::from("/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
            ];
            for c in &candidates {
                let p = Self::retail_font_path(c);
                if p.is_file() {
                    if let Some(f) = Self::load_png(&p) {
                        return f;
                    }
                }
            }
            // 3. Fallback to authentic embedded font atlas
            Self::load_embedded()
        })
    }

    fn glyph(&self, ch: char) -> Option<&Glyph> {
        let c = ch as u32;
        let id = if (FIRST_CHAR..FIRST_CHAR + self.glyphs.len() as u32).contains(&c) {
            (c - FIRST_CHAR) as usize
        } else {
            ('?' as u32).saturating_sub(FIRST_CHAR) as usize
        };
        self.glyphs.get(id)
    }

    /// Advance of one character in atlas pixels.
    pub fn advance(&self, ch: char) -> usize {
        if ch == ' ' {
            return SPACE_ADVANCE;
        }
        match self.glyph(ch) {
            Some(g) => g.advance,
            None => SPACE_ADVANCE,
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

    /// Draws a string styled with an authentic SH2 text color (with vertical two-tone gradient and drop shadow).
    pub fn draw_line_styled(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        style: Sh2TextColor,
        shadow: bool,
    ) {
        let gradient = style.to_gradient();
        self.draw_line_gradient(rast, text, pos, px_height, gradient, shadow);
    }

    /// Draws a string centered horizontally at `center_x` with top edge at `y`.
    pub fn draw_line_centered(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        style: Sh2TextColor,
        shadow: bool,
    ) {
        let (center_x, y) = pos;
        let w = self.text_width(text, px_height);
        let x = center_x.saturating_sub(w / 2);
        self.draw_line_styled(rast, text, (x, y), px_height, style, shadow);
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
        self.draw_line_gradient(rast, text, pos, px_height, (color, color), shadow);
    }

    /// Draws one line of text with its top-left at `pos` with vertical two-tone gradient shading
    /// (interpolating from `gradient.0` at the top edge to `gradient.1` at the bottom edge)
    /// and optional drop shadow (`shadow`).
    pub fn draw_line_gradient(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        gradient: ([u8; 4], [u8; 4]),
        shadow: bool,
    ) {
        let (top_color, bottom_color) = gradient;
        let (x, y) = pos;
        let px_height = px_height.max(4);
        let off = (px_height / 14).max(1);

        // Pass 0: drop shadow (if enabled)
        if shadow {
            let shadow_alpha = (top_color[3].max(bottom_color[3]) as f32 * 0.85) as u8;
            let shadow_col = [0, 0, 0, shadow_alpha];
            let mut cx = x + off;
            let cy = y + off;
            for ch in text.chars() {
                if ch != ' ' {
                    if let Some(g) = self.glyph(ch) {
                        Self::draw_glyph(g, rast, cx, cy, px_height, shadow_col);
                    }
                }
                cx += self.advance(ch) * px_height / CELL_H;
            }
        }

        // Pass 1: gradient text
        let mut cx = x;
        for ch in text.chars() {
            if ch != ' ' {
                if let Some(g) = self.glyph(ch) {
                    Self::draw_glyph_gradient(g, rast, cx, y, px_height, top_color, bottom_color);
                }
            }
            cx += self.advance(ch) * px_height / CELL_H;
        }
    }

    fn draw_glyph_gradient(
        g: &Glyph,
        rast: &mut TextRasterizer,
        x: usize,
        y: usize,
        px_height: usize,
        top_color: [u8; 4],
        bottom_color: [u8; 4],
    ) {
        if g.min_x > g.max_x {
            return;
        }
        let step = CELL_H as f32 / px_height as f32; // source pixels per destination pixel
        let w = ((g.max_x - g.min_x + 1) as f32 / step).ceil() as usize;
        let max_dy = (px_height.saturating_sub(1)).max(1) as f32;

        for dy in 0..px_height {
            let t = (dy as f32 / max_dy).clamp(0.0, 1.0);
            let row_color = [
                (top_color[0] as f32 * (1.0 - t) + bottom_color[0] as f32 * t).round() as u8,
                (top_color[1] as f32 * (1.0 - t) + bottom_color[1] as f32 * t).round() as u8,
                (top_color[2] as f32 * (1.0 - t) + bottom_color[2] as f32 * t).round() as u8,
                (top_color[3] as f32 * (1.0 - t) + bottom_color[3] as f32 * t).round() as u8,
            ];

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
                    Self::blend(rast, x + dx, y + dy, row_color, sum / area);
                }
            }
        }
    }
}

/// Draws a (possibly multi-line) subtitle centred on `center_x` with its last line ending at
/// `bottom_y`. Uses the authentic proportional Silent Hill 2 font (via `AtlasFont::shared`).
/// `px_height` is the line height in pixels.
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
    let font = AtlasFont::shared(game_root);
    for (i, line) in lines.iter().enumerate() {
        let y = top + i * line_h;
        let w = font.text_width(line, px_height);
        font.draw_line(
            rast,
            line,
            (center_x.saturating_sub(w / 2), y),
            px_height,
            color,
            true,
        );
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
        let f = AtlasFont::from_image_bytes(w, h, 4, &px).unwrap();
        // 'A' has retail width 13, scaled 2x = 26, plus LETTER_SPACING = 30
        assert_eq!(f.advance('A'), 13 * 2 + LETTER_SPACING);
        assert_eq!(f.advance(' '), SPACE_ADVANCE);
        let mut r = TextRasterizer::new(64, 64);
        f.draw_line(&mut r, "A", (4, 4), 32, [255, 255, 255, 255], false);
        assert!(r.pixels.chunks(4).any(|p| p[3] > 200));
    }

    #[test]
    fn test_embedded_font_renders() {
        let font = AtlasFont::load_embedded();
        assert!(font.advance('i') < font.advance('W'));
        let mut r = TextRasterizer::new(400, 60);
        font.draw_line(&mut r, "Mary...", (4, 4), 32, [255; 4], true);
        assert!(r.pixels.chunks(4).any(|p| p[3] != 0));
    }

    #[test]
    fn test_gradient_and_styled_text() {
        let font = AtlasFont::load_embedded();
        let mut r = TextRasterizer::new(300, 60);
        font.draw_line_styled(
            &mut r,
            "NEW GAME",
            (20, 10),
            32,
            Sh2TextColor::GoldSelected,
            true,
        );
        assert!(r.pixels.chunks(4).any(|p| p[3] > 0));

        // Test centered drawing
        let mut r2 = TextRasterizer::new(400, 60);
        font.draw_line_centered(
            &mut r2,
            "SILENT HILL 2",
            (200, 10),
            28,
            Sh2TextColor::AmberHeader,
            true,
        );
        assert!(r2.pixels.chunks(4).any(|p| p[3] > 0));
    }
}
