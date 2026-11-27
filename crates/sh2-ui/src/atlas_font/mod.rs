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

mod cache;

use crate::font::TextRasterizer;
use cache::{CachedGlyphMask, GlyphCacheKey, GlyphMaskCache};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const CELL_W: usize = 44;
const CELL_H: usize = 64;
const FIRST_CHAR: u32 = 0x20;
const GLYPH_COUNT: usize = 224;

/// Full atlas glyph cell height in virtual `512×480` pixels (`SH2:EE` `Patches/Fonts.cpp`
/// `charH = 32`, `Common/Settings.h` `CustomFontCharHeight = 32`; `2×` = `CELL_H = 64`).
pub const PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL: f32 = 32.0;

pub use crate::subtitle::{
    compute_ps2_subtitle_layout, draw_subtitle, draw_subtitle_ps2, ps2_subtitle_top_y_480,
    Ps2SubtitleLayout, PS2_FONT_GS_Y_ORIGIN, PS2_FONT_PIXEL_ASPECT_4X3, PS2_FONT_VIRTUAL_HEIGHT,
    PS2_FONT_VIRTUAL_WIDTH, PS2_NORMAL_FONT_HEIGHT, PS2_NORMAL_FONT_WIDTH,
    PS2_SUBTITLE_COLOR_WHITE, PS2_SUBTITLE_Y_ANCHOR_GS,
};

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
const EMBEDDED_FONT_PNG: &[u8] = include_bytes!("../../assets/font_normal.png");

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
    mask_cache: Mutex<GlyphMaskCache>,
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
        Some(Self {
            glyphs,
            mask_cache: Mutex::new(GlyphMaskCache::default()),
        })
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

    fn glyph_index(&self, ch: char) -> Option<usize> {
        let c = ch as u32;
        let id = if (FIRST_CHAR..FIRST_CHAR + self.glyphs.len() as u32).contains(&c) {
            (c - FIRST_CHAR) as usize
        } else {
            ('?' as u32).saturating_sub(FIRST_CHAR) as usize
        };
        (id < self.glyphs.len()).then_some(id)
    }

    fn glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyph_index(ch).and_then(|id| self.glyphs.get(id))
    }

    /// Returns the number of currently cached resampled glyph coverage masks.
    #[must_use]
    pub fn glyph_cache_len(&self) -> usize {
        self.mask_cache.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// Clears all cached resampled glyph coverage masks.
    pub fn clear_glyph_cache(&self) {
        if let Ok(mut c) = self.mask_cache.lock() {
            c.clear();
        }
    }

    /// Computes a quantized Q12 (`0..=4096`) coverage mask for `g` at `(px_height, x_scale)`.
    fn rasterize_glyph_mask(g: &Glyph, px_height: usize, x_scale: f32) -> CachedGlyphMask {
        if g.min_x > g.max_x {
            return CachedGlyphMask {
                width: 0,
                height: 0,
                coverage: Vec::new(),
            };
        }
        let step_y = CELL_H as f32 / px_height as f32;
        let step_x = step_y / x_scale.max(0.25);
        let w = ((g.max_x - g.min_x + 1) as f32 / step_x).ceil() as usize;
        let mut coverage = vec![0u16; w * px_height];
        for dy in 0..px_height {
            let sy0 = dy as f32 * step_y;
            let sy1 = sy0 + step_y;
            let row_ofs = dy * w;
            for dx in 0..w {
                let sx0 = g.min_x as f32 + dx as f32 * step_x;
                let sx1 = sx0 + step_x;
                let mut sum = 0.0f32;
                let mut area = 0.0f32;
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
                if sum > 0.0 && area > 0.0 {
                    coverage[row_ofs + dx] = ((sum / area).clamp(0.0, 1.0) * 4096.0).round() as u16;
                }
            }
        }
        CachedGlyphMask {
            width: w,
            height: px_height,
            coverage,
        }
    }

    /// Looks up or computes and caches the resampled coverage mask for `(glyph_id, px_height, x_scale)`.
    fn cached_mask(&self, glyph_id: usize, px_height: usize, x_scale: f32) -> Arc<CachedGlyphMask> {
        let clamped_scale = x_scale.max(0.25);
        let key = GlyphCacheKey::new(glyph_id, px_height, clamped_scale);
        if let Ok(mut cache) = self.mask_cache.lock() {
            if let Some(hit) = cache.get(key) {
                return hit;
            }
        }
        let mask = Arc::new(Self::rasterize_glyph_mask(
            &self.glyphs[glyph_id],
            px_height,
            clamped_scale,
        ));
        if let Ok(mut cache) = self.mask_cache.lock() {
            cache.insert(key, Arc::clone(&mask));
        }
        mask
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

    /// Integer fixed-point alpha over-blend of `color` modulated by Q12 `cov_q12` (`0..=4096`).
    #[inline]
    fn blend_fixed_row(dst_row: &mut [u8], x: usize, color: [u8; 4], cov_q12: u16) {
        const FULL_Q12_255: u32 = 4096 * 255;
        let src_w = u32::from(cov_q12) * u32::from(color[3]);
        if src_w == 0 {
            return;
        }
        let o = x * 4;
        if src_w >= FULL_Q12_255 {
            dst_row[o] = color[0];
            dst_row[o + 1] = color[1];
            dst_row[o + 2] = color[2];
            dst_row[o + 3] = 255;
            return;
        }
        let dst_a = u32::from(dst_row[o + 3]);
        if dst_a == 0 {
            dst_row[o] = color[0];
            dst_row[o + 1] = color[1];
            dst_row[o + 2] = color[2];
            dst_row[o + 3] = ((src_w + 2048) / 4096).min(255) as u8;
            return;
        }
        let dst_w = ((FULL_Q12_255 - src_w) * dst_a + 127) / 255;
        let out_w = src_w + dst_w;
        let src_w64 = u64::from(src_w);
        let dst_w64 = u64::from(dst_w);
        let out_w64 = u64::from(out_w);
        let half_w64 = out_w64 / 2;
        dst_row[o] = ((u64::from(color[0]) * src_w64 + u64::from(dst_row[o]) * dst_w64 + half_w64)
            / out_w64)
            .min(255) as u8;
        dst_row[o + 1] =
            ((u64::from(color[1]) * src_w64 + u64::from(dst_row[o + 1]) * dst_w64 + half_w64)
                / out_w64)
                .min(255) as u8;
        dst_row[o + 2] =
            ((u64::from(color[2]) * src_w64 + u64::from(dst_row[o + 2]) * dst_w64 + half_w64)
                / out_w64)
                .min(255) as u8;
        dst_row[o + 3] = ((out_w + 2048) / 4096).min(255) as u8;
    }

    fn blit_cached_mask_solid(
        mask: &CachedGlyphMask,
        rast: &mut TextRasterizer,
        x: usize,
        y: usize,
        color: [u8; 4],
    ) {
        if mask.width == 0 || mask.height == 0 || color[3] == 0 {
            return;
        }
        let max_dy = mask.height.min(rast.height.saturating_sub(y));
        let max_dx = mask.width.min(rast.width.saturating_sub(x));
        if max_dy == 0 || max_dx == 0 {
            return;
        }
        let stride = rast.width * 4;
        for dy in 0..max_dy {
            let src_row = &mask.coverage[dy * mask.width..dy * mask.width + max_dx];
            let dst_row = &mut rast.pixels[(y + dy) * stride..(y + dy + 1) * stride];
            for (dx, &cov) in src_row.iter().enumerate() {
                if cov != 0 {
                    Self::blend_fixed_row(dst_row, x + dx, color, cov);
                }
            }
        }
    }

    fn blit_cached_mask_gradient(
        mask: &CachedGlyphMask,
        rast: &mut TextRasterizer,
        x: usize,
        y: usize,
        top_color: [u8; 4],
        bottom_color: [u8; 4],
    ) {
        if mask.width == 0 || mask.height == 0 {
            return;
        }
        if top_color == bottom_color {
            Self::blit_cached_mask_solid(mask, rast, x, y, top_color);
            return;
        }
        let max_dy = mask.height.min(rast.height.saturating_sub(y));
        let max_dx = mask.width.min(rast.width.saturating_sub(x));
        if max_dy == 0 || max_dx == 0 {
            return;
        }
        let max_row_f = (mask.height.saturating_sub(1)).max(1) as f32;
        let stride = rast.width * 4;
        for dy in 0..max_dy {
            let t = (dy as f32 / max_row_f).clamp(0.0, 1.0);
            let row_color = [
                (top_color[0] as f32 * (1.0 - t) + bottom_color[0] as f32 * t).round() as u8,
                (top_color[1] as f32 * (1.0 - t) + bottom_color[1] as f32 * t).round() as u8,
                (top_color[2] as f32 * (1.0 - t) + bottom_color[2] as f32 * t).round() as u8,
                (top_color[3] as f32 * (1.0 - t) + bottom_color[3] as f32 * t).round() as u8,
            ];
            if row_color[3] == 0 {
                continue;
            }
            let src_row = &mask.coverage[dy * mask.width..dy * mask.width + max_dx];
            let dst_row = &mut rast.pixels[(y + dy) * stride..(y + dy + 1) * stride];
            for (dx, &cov) in src_row.iter().enumerate() {
                if cov != 0 {
                    Self::blend_fixed_row(dst_row, x + dx, row_color, cov);
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

        // Pass 0: drop shadow (if enabled) — populates glyph coverage mask cache
        if shadow {
            let shadow_alpha = top_color[3].max(bottom_color[3]);
            let shadow_col = [0, 0, 0, shadow_alpha];
            let mut cx = (x + off_x) as f32;
            let cy = y + off_y;
            for ch in text.chars() {
                if ch != ' ' {
                    if let Some(id) = self.glyph_index(ch) {
                        let mask = self.cached_mask(id, px_height, x_scale);
                        Self::blit_cached_mask_solid(
                            &mask,
                            rast,
                            cx.round() as usize,
                            cy,
                            shadow_col,
                        );
                    }
                }
                cx += (self.advance(ch) as f32 * px_height as f32 / CELL_H as f32) * x_scale;
            }
        }

        // Pass 1: gradient text — reuses cached coverage masks from Pass 0
        let mut cx = x as f32;
        for ch in text.chars() {
            if ch != ' ' {
                if let Some(id) = self.glyph_index(ch) {
                    let mask = self.cached_mask(id, px_height, x_scale);
                    Self::blit_cached_mask_gradient(
                        &mask,
                        rast,
                        cx.round() as usize,
                        y,
                        top_color,
                        bottom_color,
                    );
                }
            }
            cx += (self.advance(ch) as f32 * px_height as f32 / CELL_H as f32) * x_scale;
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

    /// Uncached f32 reference resampling matching the pre-cache `AtlasFont` algorithm.
    #[allow(clippy::too_many_arguments)]
    fn reference_draw_line_gradient_uncached(
        font: &AtlasFont,
        rast: &mut TextRasterizer,
        text: &str,
        pos: (usize, usize),
        px_height: usize,
        x_scale: f32,
        gradient: ([u8; 4], [u8; 4]),
        shadow: bool,
    ) {
        fn blend_ref(rast: &mut TextRasterizer, x: usize, y: usize, color: [u8; 4], cover: f32) {
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

        let (top_color, bottom_color) = gradient;
        let (x, y) = pos;
        let px_height = px_height.max(4);
        let x_scale = x_scale.max(0.25);
        let virtual_px_y = px_height as f32 / PS2_FONT_ATLAS_CELL_HEIGHT_VIRTUAL;
        let off_x = (virtual_px_y * x_scale).round().max(1.0) as usize;
        let off_y = virtual_px_y.round().max(1.0) as usize;
        let step_y = CELL_H as f32 / px_height as f32;
        let step_x = step_y / x_scale;

        let draw_pass = |rast: &mut TextRasterizer,
                         start_x: usize,
                         start_y: usize,
                         top_c: [u8; 4],
                         bot_c: [u8; 4]| {
            let max_dy = (px_height.saturating_sub(1)).max(1) as f32;
            let mut cx = start_x as f32;
            for ch in text.chars() {
                if ch != ' ' {
                    if let Some(g) = font.glyph(ch) {
                        if g.min_x <= g.max_x {
                            let w = ((g.max_x - g.min_x + 1) as f32 / step_x).ceil() as usize;
                            for dy in 0..px_height {
                                let t = (dy as f32 / max_dy).clamp(0.0, 1.0);
                                let row_c = [
                                    (top_c[0] as f32 * (1.0 - t) + bot_c[0] as f32 * t).round()
                                        as u8,
                                    (top_c[1] as f32 * (1.0 - t) + bot_c[1] as f32 * t).round()
                                        as u8,
                                    (top_c[2] as f32 * (1.0 - t) + bot_c[2] as f32 * t).round()
                                        as u8,
                                    (top_c[3] as f32 * (1.0 - t) + bot_c[3] as f32 * t).round()
                                        as u8,
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
                                        let wy = (sy1.min(sy as f32 + 1.0) - sy0.max(sy as f32))
                                            .max(0.0);
                                        let mut sx = sx0.floor() as usize;
                                        while (sx as f32) < sx1 {
                                            let wx = (sx1.min(sx as f32 + 1.0)
                                                - sx0.max(sx as f32))
                                            .max(0.0);
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
                                        blend_ref(
                                            rast,
                                            cx.round() as usize + dx,
                                            start_y + dy,
                                            row_c,
                                            sum / area,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                cx += (font.advance(ch) as f32 * px_height as f32 / CELL_H as f32) * x_scale;
            }
        };

        if shadow {
            let shadow_a = top_color[3].max(bottom_color[3]);
            let shadow_c = [0, 0, 0, shadow_a];
            draw_pass(rast, x + off_x, y + off_y, shadow_c, shadow_c);
        }
        draw_pass(rast, x, y, top_color, bottom_color);
    }

    #[test]
    fn cached_atlas_font_pixel_parity_against_uncached() {
        let font = AtlasFont::load_embedded();
        font.clear_glyph_cache();

        let text = "SILENT HILL 2: Restless Dreams";
        let mut cached = TextRasterizer::new(512, 80);
        let mut uncached = TextRasterizer::new(512, 80);
        // Non-transparent dark slate background to exercise full alpha compositing
        cached.fill_rect(0, 0, 512, 80, [36, 42, 48, 220]);
        uncached.fill_rect(0, 0, 512, 80, [36, 42, 48, 220]);

        font.draw_line_styled_aspect(
            &mut cached,
            text,
            (12, 16),
            34,
            1.25,
            Sh2TextColor::GoldSelected,
            true,
        );
        reference_draw_line_gradient_uncached(
            &font,
            &mut uncached,
            text,
            (12, 16),
            34,
            1.25,
            Sh2TextColor::GoldSelected.to_gradient(),
            true,
        );

        let mut max_diff = 0u8;
        for (&a, &b) in cached.pixels.iter().zip(uncached.pixels.iter()) {
            max_diff = max_diff.max(a.abs_diff(b));
        }
        assert!(
            max_diff <= 1,
            "cached AtlasFont diverged from uncached float reference by {max_diff} LSB (expected <= 1)"
        );
    }
}
