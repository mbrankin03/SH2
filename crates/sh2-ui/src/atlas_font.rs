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

/// Full atlas glyph cell height in virtual `512×480` pixels (`SH2:EE` `Patches/Fonts.cpp`
/// `charH = 32`, `Common/Settings.h` `CustomFontCharHeight = 32`; `2×` = `CELL_H = 64`).
pub const PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL: f32 = 32.0;

/// Horizontal pixel aspect ratio mapping `512×480` virtual font pixels onto a square-pixel `4:3`
/// display (`(4/3) * (480 / 512) = 5/4 = 1.25`, `sh2pc.exe` `FUN_00481630` `0x00481630`).
pub const PS2_FONT_PIXEL_ASPECT_4X3: f32 = 1.25;

/// Default subtitle RGBA color: pure white from `SLUS_202.28` `fontSetColor(0)` (`0x001b73a0`,
/// table `@1369` at `0x002baec0`: PS2 GS `0x808080` = unity `1.0` white).
pub const PS2_SUBTITLE_COLOR_WHITE: [u8; 4] = [255, 255, 255, 255];

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
            Self::White => ([255, 255, 255, 255], [255, 255, 255, 255]),
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
                PathBuf::from("../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
                PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
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
        self.text_width_aspect(text, px_height, 1.0)
    }

    /// Width in destination pixels of `text` rendered `px_height` tall with horizontal pixel aspect `x_scale`
    /// (e.g. `c.sx / c.sy = 7/6` when rasterizing PS2 `512x448` UI into a square-pixel `4:3` buffer).
    pub fn text_width_aspect(&self, text: &str, px_height: usize, x_scale: f32) -> usize {
        let x_scale = x_scale.max(0.25);
        let adv: f32 = text
            .chars()
            .map(|c| (self.advance(c) as f32 * px_height as f32 / CELL_H as f32) * x_scale)
            .sum();
        adv.round() as usize
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
        x_scale: f32,
        color: [u8; 4],
    ) {
        if g.min_x > g.max_x {
            return;
        }
        let step_y = CELL_H as f32 / px_height as f32; // source pixels per destination Y pixel
        let step_x = step_y / x_scale.max(0.25); // source pixels per destination X pixel
        let w = ((g.max_x - g.min_x + 1) as f32 / step_x).ceil() as usize;
        for dy in 0..px_height {
            let sy0 = dy as f32 * step_y;
            let sy1 = sy0 + step_y;
            for dx in 0..w {
                let sx0 = g.min_x as f32 + dx as f32 * step_x;
                let sx1 = sx0 + step_x;
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
        self.draw_line_styled_aspect(rast, text, pos, px_height, 1.0, style, shadow);
    }

    /// Draws a styled string with horizontal pixel aspect ratio `x_scale`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_line_styled_aspect(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        x_scale: f32,
        style: Sh2TextColor,
        shadow: bool,
    ) {
        let gradient = style.to_gradient();
        self.draw_line_gradient_aspect(rast, text, pos, px_height, x_scale, gradient, shadow);
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
        self.draw_line_centered_aspect(rast, text, pos, px_height, 1.0, style, shadow);
    }

    /// Draws a horizontally centered string with horizontal pixel aspect ratio `x_scale`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_line_centered_aspect(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        x_scale: f32,
        style: Sh2TextColor,
        shadow: bool,
    ) {
        let (center_x, y) = pos;
        let w = self.text_width_aspect(text, px_height, x_scale);
        let x = center_x.saturating_sub(w / 2);
        self.draw_line_styled_aspect(rast, text, (x, y), px_height, x_scale, style, shadow);
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
        self.draw_line_gradient_aspect(rast, text, pos, px_height, 1.0, (color, color), shadow);
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
        self.draw_line_gradient_aspect(rast, text, pos, px_height, 1.0, gradient, shadow);
    }

    /// Draws one line of gradient-shaded text with horizontal pixel aspect ratio `x_scale`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_line_gradient_aspect(
        &self,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        x_scale: f32,
        gradient: ([u8; 4], [u8; 4]),
        shadow: bool,
    ) {
        let (top_color, bottom_color) = gradient;
        let (x, y) = pos;
        let px_height = px_height.max(4);
        let x_scale = x_scale.max(0.25);
        // 1:1 PS2/PC drop shadow (`SLUS_202.28` `fontFlush` `0x001b7530`, `sh2pc.exe`
        // `FUN_00482160` `0x00482160`): `+0x10` Q4 subpixels = `(+1, +1)` virtual pixels in
        // `512×480` space, pure black (`_DAT_01157cc4 = 0x000000`) at 100% foreground alpha.
        let virtual_px_y = px_height as f32 / PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL;
        let off_x = (virtual_px_y * x_scale).round().max(1.0) as usize;
        let off_y = virtual_px_y.round().max(1.0) as usize;

        // Pass 0: drop shadow (if enabled)
        if shadow {
            let shadow_alpha = top_color[3].max(bottom_color[3]);
            let shadow_col = [0, 0, 0, shadow_alpha];
            let mut cx = (x + off_x) as f32;
            let cy = y + off_y;
            for ch in text.chars() {
                if ch != ' ' {
                    if let Some(g) = self.glyph(ch) {
                        Self::draw_glyph(
                            g,
                            rast,
                            cx.round() as usize,
                            cy,
                            px_height,
                            x_scale,
                            shadow_col,
                        );
                    }
                }
                cx += (self.advance(ch) as f32 * px_height as f32 / CELL_H as f32) * x_scale;
            }
        }

        // Pass 1: gradient text
        let mut cx = x as f32;
        for ch in text.chars() {
            if ch != ' ' {
                if let Some(g) = self.glyph(ch) {
                    Self::draw_glyph_gradient(
                        g,
                        rast,
                        cx.round() as usize,
                        y,
                        px_height,
                        x_scale,
                        top_color,
                        bottom_color,
                    );
                }
            }
            cx += (self.advance(ch) as f32 * px_height as f32 / CELL_H as f32) * x_scale;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_glyph_gradient(
        g: &Glyph,
        rast: &mut TextRasterizer,
        x: usize,
        y: usize,
        px_height: usize,
        x_scale: f32,
        top_color: [u8; 4],
        bottom_color: [u8; 4],
    ) {
        if g.min_x > g.max_x {
            return;
        }
        let step_y = CELL_H as f32 / px_height as f32; // source pixels per destination Y pixel
        let step_x = step_y / x_scale.max(0.25); // source pixels per destination X pixel
        let w = ((g.max_x - g.min_x + 1) as f32 / step_x).ceil() as usize;
        let max_dy = (px_height.saturating_sub(1)).max(1) as f32;

        for dy in 0..px_height {
            let t = (dy as f32 / max_dy).clamp(0.0, 1.0);
            let row_color = [
                (top_color[0] as f32 * (1.0 - t) + bottom_color[0] as f32 * t).round() as u8,
                (top_color[1] as f32 * (1.0 - t) + bottom_color[1] as f32 * t).round() as u8,
                (top_color[2] as f32 * (1.0 - t) + bottom_color[2] as f32 * t).round() as u8,
                (top_color[3] as f32 * (1.0 - t) + bottom_color[3] as f32 * t).round() as u8,
            ];

            let sy0 = dy as f32 * step_y;
            let sy1 = sy0 + step_y;
            for dx in 0..w {
                let sx0 = g.min_x as f32 + dx as f32 * step_x;
                let sx1 = sx0 + step_x;
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
