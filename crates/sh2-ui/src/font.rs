//! Minimal embedded 8x8 bitmap font and text rasterizer for UI menus and options.

/// 8x8 standard ASCII font patterns (32..=126).
/// Each glyph is 8 bytes, representing 8 rows of 1-bit pixels.
#[rustfmt::skip]
const FONT_8X8: [[u8; 8]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // ' '
    [0x18, 0x3c, 0x3c, 0x18, 0x18, 0x00, 0x18, 0x00], // '!'
    [0x66, 0x66, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00], // '"'
    [0x6c, 0x6c, 0xfe, 0x6c, 0xfe, 0x6c, 0x6c, 0x00], // '#'
    [0x18, 0x3e, 0x60, 0x3c, 0x06, 0x7c, 0x18, 0x00], // '$'
    [0x00, 0x63, 0x66, 0x0c, 0x18, 0x33, 0x63, 0x00], // '%'
    [0x1c, 0x36, 0x1c, 0x3b, 0x6e, 0x66, 0x3b, 0x00], // '&'
    [0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00], // '''
    [0x0c, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0c, 0x00], // '('
    [0x30, 0x18, 0x0c, 0x0c, 0x0c, 0x18, 0x30, 0x00], // ')'
    [0x00, 0x66, 0x3c, 0xff, 0x3c, 0x66, 0x00, 0x00], // '*'
    [0x00, 0x18, 0x18, 0x7e, 0x18, 0x18, 0x00, 0x00], // '+'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30], // ','
    [0x00, 0x00, 0x00, 0x7e, 0x00, 0x00, 0x00, 0x00], // '-'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00], // '.'
    [0x03, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x40, 0x00], // '/'
    [0x3c, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x3c, 0x00], // '0'
    [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7e, 0x00], // '1'
    [0x3c, 0x66, 0x06, 0x0c, 0x18, 0x30, 0x7e, 0x00], // '2'
    [0x3c, 0x66, 0x06, 0x1c, 0x06, 0x66, 0x3c, 0x00], // '3'
    [0x0c, 0x1c, 0x34, 0x64, 0x7e, 0x04, 0x0e, 0x00], // '4'
    [0x7e, 0x60, 0x7c, 0x06, 0x06, 0x66, 0x3c, 0x00], // '5'
    [0x1c, 0x30, 0x60, 0x7c, 0x66, 0x66, 0x3c, 0x00], // '6'
    [0x7e, 0x06, 0x0c, 0x18, 0x30, 0x30, 0x30, 0x00], // '7'
    [0x3c, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x3c, 0x00], // '8'
    [0x3c, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38, 0x00], // '9'
    [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00], // ':'
    [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x30, 0x00], // ';'
    [0x06, 0x0c, 0x18, 0x30, 0x18, 0x0c, 0x06, 0x00], // '<'
    [0x00, 0x00, 0x7e, 0x00, 0x7e, 0x00, 0x00, 0x00], // '='
    [0x60, 0x30, 0x18, 0x0c, 0x18, 0x30, 0x60, 0x00], // '>'
    [0x3c, 0x66, 0x06, 0x0c, 0x18, 0x00, 0x18, 0x00], // '?'
    [0x3c, 0x66, 0x6e, 0x6e, 0x60, 0x62, 0x3c, 0x00], // '@'
    [0x18, 0x3c, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x00], // 'A'
    [0x7c, 0x66, 0x66, 0x7c, 0x66, 0x66, 0x7c, 0x00], // 'B'
    [0x3c, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3c, 0x00], // 'C'
    [0x78, 0x6c, 0x66, 0x66, 0x66, 0x6c, 0x78, 0x00], // 'D'
    [0x7e, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x7e, 0x00], // 'E'
    [0x7e, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x60, 0x00], // 'F'
    [0x3c, 0x66, 0x60, 0x6e, 0x66, 0x66, 0x3a, 0x00], // 'G'
    [0x66, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x66, 0x00], // 'H'
    [0x3c, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00], // 'I'
    [0x0e, 0x06, 0x06, 0x06, 0x06, 0x66, 0x3c, 0x00], // 'J'
    [0x66, 0x6c, 0x78, 0x70, 0x78, 0x6c, 0x66, 0x00], // 'K'
    [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7e, 0x00], // 'L'
    [0x63, 0x77, 0x7f, 0x6b, 0x63, 0x63, 0x63, 0x00], // 'M'
    [0x66, 0x76, 0x7e, 0x7e, 0x6e, 0x66, 0x66, 0x00], // 'N'
    [0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00], // 'O'
    [0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60, 0x60, 0x00], // 'P'
    [0x3c, 0x66, 0x66, 0x66, 0x6a, 0x6c, 0x36, 0x00], // 'Q'
    [0x7c, 0x66, 0x66, 0x7c, 0x6c, 0x66, 0x66, 0x00], // 'R'
    [0x3c, 0x66, 0x60, 0x3c, 0x06, 0x66, 0x3c, 0x00], // 'S'
    [0x7e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00], // 'T'
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00], // 'U'
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x00], // 'V'
    [0x63, 0x63, 0x63, 0x6b, 0x7f, 0x77, 0x63, 0x00], // 'W'
    [0x66, 0x66, 0x3c, 0x18, 0x3c, 0x66, 0x66, 0x00], // 'X'
    [0x66, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x18, 0x00], // 'Y'
    [0x7e, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x7e, 0x00], // 'Z'
    [0x3c, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3c, 0x00], // '['
    [0x40, 0x60, 0x30, 0x18, 0x0c, 0x06, 0x02, 0x00], // '\'
    [0x3c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x3c, 0x00], // ']'
    [0x10, 0x38, 0x6c, 0xc6, 0x00, 0x00, 0x00, 0x00], // '^'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff], // '_'
    [0x30, 0x18, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00], // '`'
    [0x00, 0x00, 0x3c, 0x06, 0x3e, 0x66, 0x3b, 0x00], // 'a'
    [0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x7c, 0x00], // 'b'
    [0x00, 0x00, 0x3c, 0x66, 0x60, 0x66, 0x3c, 0x00], // 'c'
    [0x06, 0x06, 0x3e, 0x66, 0x66, 0x66, 0x3e, 0x00], // 'd'
    [0x00, 0x00, 0x3c, 0x66, 0x7e, 0x60, 0x3c, 0x00], // 'e'
    [0x1c, 0x30, 0x78, 0x30, 0x30, 0x30, 0x30, 0x00], // 'f'
    [0x00, 0x00, 0x3b, 0x66, 0x66, 0x3e, 0x06, 0x3c], // 'g'
    [0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x00], // 'h'
    [0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x3c, 0x00], // 'i'
    [0x06, 0x00, 0x0e, 0x06, 0x06, 0x66, 0x3c, 0x00], // 'j'
    [0x60, 0x60, 0x66, 0x6c, 0x78, 0x6c, 0x66, 0x00], // 'k'
    [0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00], // 'l'
    [0x00, 0x00, 0x66, 0x7f, 0x7f, 0x6b, 0x63, 0x00], // 'm'
    [0x00, 0x00, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x00], // 'n'
    [0x00, 0x00, 0x3c, 0x66, 0x66, 0x66, 0x3c, 0x00], // 'o'
    [0x00, 0x00, 0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60], // 'p'
    [0x00, 0x00, 0x3e, 0x66, 0x66, 0x3e, 0x06, 0x06], // 'q'
    [0x00, 0x00, 0x7c, 0x66, 0x60, 0x60, 0x60, 0x00], // 'r'
    [0x00, 0x00, 0x3e, 0x60, 0x3c, 0x06, 0x7c, 0x00], // 's'
    [0x18, 0x18, 0x7e, 0x18, 0x18, 0x18, 0x0e, 0x00], // 't'
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3b, 0x00], // 'u'
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x00], // 'v'
    [0x00, 0x00, 0x63, 0x6b, 0x7f, 0x3e, 0x36, 0x00], // 'w'
    [0x00, 0x00, 0x66, 0x3c, 0x18, 0x3c, 0x66, 0x00], // 'x'
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x3e, 0x06, 0x3c], // 'y'
    [0x00, 0x00, 0x7e, 0x0c, 0x18, 0x30, 0x7e, 0x00], // 'z'
    [0x0e, 0x18, 0x18, 0x70, 0x18, 0x18, 0x0e, 0x00], // '{'
    [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00], // '|'
    [0x70, 0x18, 0x18, 0x0e, 0x18, 0x18, 0x70, 0x00], // '}'
    [0x76, 0xdc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // '~'
];

/// Text overlay buffer rasterizing 8x8 font glyphs with scale, shadow, and color.
pub struct TextRasterizer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl TextRasterizer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; width * height * 4],
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    /// Blends an RGBA pixel into the buffer using standard alpha compositing.
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let alpha = color[3] as u32;
        if alpha == 0 {
            return;
        }
        let offset = (y * self.width + x) * 4;
        if alpha == 255 {
            self.pixels[offset..offset + 4].copy_from_slice(&color);
        } else {
            let dst_a = self.pixels[offset + 3] as u32;
            if dst_a == 0 {
                // Straight-alpha output (matching `AtlasFont::blend` and GPU `ALPHA_BLENDING`):
                // writing premultiplied RGB onto a transparent pixel would double-multiply by alpha on GPU blit.
                self.pixels[offset..offset + 4].copy_from_slice(&color);
            } else {
                let inv_alpha = 255 - alpha;
                let out_a = (alpha + (dst_a * inv_alpha) / 255).clamp(1, 255);
                let dst_weight = (dst_a * inv_alpha) / 255;
                for (c, &src) in color.iter().enumerate().take(3) {
                    let dst = self.pixels[offset + c] as u32;
                    self.pixels[offset + c] =
                        ((src as u32 * alpha + dst * dst_weight) / out_a).min(255) as u8;
                }
                self.pixels[offset + 3] = out_a as u8;
            }
        }
    }

    /// Draw string with drop shadow at (x, y) with a given scale factor.
    pub fn draw_text(
        &mut self,
        text: &str,
        x: usize,
        y: usize,
        scale: usize,
        color: [u8; 4],
        shadow: bool,
    ) {
        if shadow {
            let shadow_color = [0, 0, 0, (color[3] as f32 * 0.8) as u8];
            self.draw_string_raw(text, x + scale, y + scale, scale, shadow_color);
        }
        self.draw_string_raw(text, x, y, scale, color);
    }

    fn draw_string_raw(
        &mut self,
        text: &str,
        mut cx: usize,
        cy: usize,
        scale: usize,
        color: [u8; 4],
    ) {
        for ch in text.chars() {
            let code = ch as usize;
            if (32..=126).contains(&code) {
                let glyph = &FONT_8X8[code - 32];
                for (row_idx, &row) in glyph.iter().enumerate() {
                    for col_idx in 0..8 {
                        if (row & (0x80 >> col_idx)) != 0 {
                            for dy in 0..scale {
                                for dx in 0..scale {
                                    let px = cx + col_idx * scale + dx;
                                    let py = cy + row_idx * scale + dy;
                                    if px < self.width && py < self.height {
                                        let offset = (py * self.width + px) * 4;
                                        self.pixels[offset] = color[0];
                                        self.pixels[offset + 1] = color[1];
                                        self.pixels[offset + 2] = color[2];
                                        self.pixels[offset + 3] = color[3];
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cx += 8 * scale;
        }
    }

    /// Fills a solid or semi-transparent axis-aligned rectangle `(x0, y0, w, h)`.
    ///
    /// Uses a direct row-slice fill fast path when `color[3] == 255` (opaque) and skips
    /// work entirely when `color[3] == 0`.
    pub fn fill_rect(&mut self, x0: usize, y0: usize, w: usize, h: usize, color: [u8; 4]) {
        let alpha = color[3] as u32;
        if alpha == 0 || w == 0 || h == 0 || x0 >= self.width || y0 >= self.height {
            return;
        }
        let x1 = x0.saturating_add(w).min(self.width);
        let y1 = y0.saturating_add(h).min(self.height);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        if alpha == 255 {
            if x0 == 0 && x1 == self.width {
                let start = y0 * self.width * 4;
                let end = y1 * self.width * 4;
                for px in self.pixels[start..end].as_chunks_mut::<4>().0 {
                    *px = color;
                }
            } else {
                let row_stride = self.width * 4;
                for y in y0..y1 {
                    let row_start = y * row_stride + x0 * 4;
                    let row_end = y * row_stride + x1 * 4;
                    for px in self.pixels[row_start..row_end].as_chunks_mut::<4>().0 {
                        *px = color;
                    }
                }
            }
            return;
        }

        let inv_alpha = 255 - alpha;
        let row_stride = self.width * 4;
        let src0 = color[0] as u32 * alpha;
        let src1 = color[1] as u32 * alpha;
        let src2 = color[2] as u32 * alpha;
        for y in y0..y1 {
            let row_start = y * row_stride + x0 * 4;
            let row_end = y * row_stride + x1 * 4;
            for px in self.pixels[row_start..row_end].as_chunks_mut::<4>().0 {
                let dst_a = px[3] as u32;
                if dst_a == 0 {
                    *px = color;
                } else {
                    let dst_weight = (dst_a * inv_alpha) / 255;
                    let out_a = (alpha + dst_weight).clamp(1, 255);
                    px[0] = ((src0 + px[0] as u32 * dst_weight) / out_a).min(255) as u8;
                    px[1] = ((src1 + px[1] as u32 * dst_weight) / out_a).min(255) as u8;
                    px[2] = ((src2 + px[2] as u32 * dst_weight) / out_a).min(255) as u8;
                    px[3] = out_a as u8;
                }
            }
        }
    }

    /// Draws a rectangular outline `(x0, y0, w, h)` with the given `thickness`.
    pub fn draw_rect_outline(
        &mut self,
        x0: usize,
        y0: usize,
        w: usize,
        h: usize,
        thickness: usize,
        color: [u8; 4],
    ) {
        if w == 0 || h == 0 || thickness == 0 {
            return;
        }
        let t = thickness.min(w).min(h);
        self.fill_rect(x0, y0, w, t, color);
        self.fill_rect(x0, y0 + h - t, w, t, color);
        self.fill_rect(x0, y0 + t, t, h.saturating_sub(2 * t), color);
        self.fill_rect(x0 + w - t, y0 + t, t, h.saturating_sub(2 * t), color);
    }

    /// Blits an RGBA8 texture (`src_rgba`, `src_w x src_h`) scaled into `(dst_x, dst_y, dst_w, dst_h)`.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_rgba_scaled(
        &mut self,
        dst_x: usize,
        dst_y: usize,
        dst_w: usize,
        dst_h: usize,
        src_rgba: &[u8],
        src_w: usize,
        src_h: usize,
        alpha_mul: f32,
    ) {
        if dst_w == 0 || dst_h == 0 {
            return;
        }
        self.blit_rgba_subrect_scaled(
            (
                dst_x as f32,
                dst_y as f32,
                (dst_x + dst_w) as f32,
                (dst_y + dst_h) as f32,
            ),
            (0.0, 0.0, 1.0, 1.0),
            src_rgba,
            src_w,
            src_h,
            alpha_mul,
        );
    }

    /// Fast-path axis-aligned sub-rectangle RGBA8 blitter mapping normalized source UVs
    /// `(u0, v0, u1, v1)` into destination screen rect `(dst_x0, dst_y0, dst_x1, dst_y1)`.
    ///
    /// Uses a direct-write fast path when `alpha_mul >= 1.0` and the sampled source pixel is
    /// fully opaque (`alpha == 255`), avoiding per-pixel triangle barycentric evaluation.
    /// Produces pixel-identical output to [`Self::blit_overlay_triangles`] for axis-aligned quads.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_rgba_subrect_scaled(
        &mut self,
        dst_rect: (f32, f32, f32, f32),
        uv_rect: (f32, f32, f32, f32),
        src_rgba: &[u8],
        src_w: usize,
        src_h: usize,
        alpha_mul: f32,
    ) {
        let (x0, y0, x1, y1) = dst_rect;
        let (u0, v0, u1, v1) = uv_rect;
        let dst_w = x1 - x0;
        let dst_h = y1 - y0;
        if dst_w <= 1e-4
            || dst_h <= 1e-4
            || src_w == 0
            || src_h == 0
            || src_rgba.len() < src_w * src_h * 4
            || alpha_mul <= 0.0
        {
            return;
        }

        let start_x = ((x0 - 0.5).ceil().max(0.0) as usize).min(self.width);
        let end_x = (((x1 - 0.5).floor() + 1.0).max(0.0) as usize).min(self.width);
        let start_y = ((y0 - 0.5).ceil().max(0.0) as usize).min(self.height);
        let end_y = (((y1 - 0.5).floor() + 1.0).max(0.0) as usize).min(self.height);
        if start_x >= end_x || start_y >= end_y {
            return;
        }

        let a_scale = alpha_mul.clamp(0.0, 1.0);
        let opaque_mul = a_scale >= 1.0 - 1e-6;
        let du_dx = (u1 - u0) / dst_w;
        let dv_dy = (v1 - v0) / dst_h;
        let src_w_f = src_w as f32;
        let src_h_f = src_h as f32;
        let src_w_max = src_w - 1;
        let src_h_max = src_h - 1;
        let dst_stride = self.width * 4;
        let src_stride = src_w * 4;

        for y in start_y..end_y {
            let py = y as f32 + 0.5;
            let v = (v0 + (py - y0) * dv_dy).clamp(0.0, 1.0);
            let sy = ((v * src_h_f) as usize).min(src_h_max);
            let src_row = &src_rgba[sy * src_stride..(sy + 1) * src_stride];
            let dst_row = &mut self.pixels[y * dst_stride..(y + 1) * dst_stride];

            for x in start_x..end_x {
                let px = x as f32 + 0.5;
                let u = (u0 + (px - x0) * du_dx).clamp(0.0, 1.0);
                let sx = ((u * src_w_f) as usize).min(src_w_max);
                let s_ofs = sx * 4;
                let raw_a = src_row[s_ofs + 3];
                if raw_a == 0 {
                    continue;
                }
                let d_ofs = x * 4;
                if opaque_mul && raw_a == 255 {
                    dst_row[d_ofs..d_ofs + 4].copy_from_slice(&src_row[s_ofs..s_ofs + 4]);
                    continue;
                }
                let sa = if opaque_mul {
                    raw_a
                } else {
                    (f32::from(raw_a) * a_scale).round() as u8
                };
                if sa == 0 {
                    continue;
                }
                let alpha = u32::from(sa);
                if alpha == 255 {
                    dst_row[d_ofs] = src_row[s_ofs];
                    dst_row[d_ofs + 1] = src_row[s_ofs + 1];
                    dst_row[d_ofs + 2] = src_row[s_ofs + 2];
                    dst_row[d_ofs + 3] = 255;
                } else {
                    let dst_a = u32::from(dst_row[d_ofs + 3]);
                    if dst_a == 0 {
                        dst_row[d_ofs] = src_row[s_ofs];
                        dst_row[d_ofs + 1] = src_row[s_ofs + 1];
                        dst_row[d_ofs + 2] = src_row[s_ofs + 2];
                        dst_row[d_ofs + 3] = sa;
                    } else {
                        let inv_alpha = 255 - alpha;
                        let dst_weight = (dst_a * inv_alpha) / 255;
                        let out_a = (alpha + dst_weight).clamp(1, 255);
                        dst_row[d_ofs] = ((u32::from(src_row[s_ofs]) * alpha
                            + u32::from(dst_row[d_ofs]) * dst_weight)
                            / out_a)
                            .min(255) as u8;
                        dst_row[d_ofs + 1] = ((u32::from(src_row[s_ofs + 1]) * alpha
                            + u32::from(dst_row[d_ofs + 1]) * dst_weight)
                            / out_a)
                            .min(255) as u8;
                        dst_row[d_ofs + 2] = ((u32::from(src_row[s_ofs + 2]) * alpha
                            + u32::from(dst_row[d_ofs + 2]) * dst_weight)
                            / out_a)
                            .min(255) as u8;
                        dst_row[d_ofs + 3] = out_a as u8;
                    }
                }
            }
        }
    }

    /// Rasterizes a 6-vertex triangle pair `[(x, y, u, v); 6]` (such as produced by
    /// `OverlaySpriteQuad::to_viewport_triangles`) sampling from `src_rgba` (`src_w x src_h`).
    pub fn blit_overlay_triangles(
        &mut self,
        tris: &[(f32, f32, f32, f32); 6],
        src_rgba: &[u8],
        src_w: usize,
        src_h: usize,
        alpha_mul: f32,
    ) {
        self.rasterize_textured_triangle(
            tris[0], tris[1], tris[2], src_rgba, src_w, src_h, alpha_mul,
        );
        self.rasterize_textured_triangle(
            tris[3], tris[4], tris[5], src_rgba, src_w, src_h, alpha_mul,
        );
    }

    /// Rasterizes a single textured 2D triangle `p0, p1, p2` where each vertex is `(x, y, u, v)`.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_textured_triangle(
        &mut self,
        p0: (f32, f32, f32, f32),
        p1: (f32, f32, f32, f32),
        p2: (f32, f32, f32, f32),
        src_rgba: &[u8],
        src_w: usize,
        src_h: usize,
        alpha_mul: f32,
    ) {
        if src_w == 0 || src_h == 0 || src_rgba.len() < src_w * src_h * 4 || alpha_mul <= 0.0 {
            return;
        }
        let min_x = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as usize;
        let max_x = (p0.0.max(p1.0).max(p2.0).ceil().max(0.0) as usize).min(self.width);
        let min_y = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as usize;
        let max_y = (p0.1.max(p1.1).max(p2.1).ceil().max(0.0) as usize).min(self.height);

        if min_x >= max_x || min_y >= max_y {
            return;
        }

        let edge = |ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32| -> f32 {
            (px - ax) * (by - ay) - (py - ay) * (bx - ax)
        };

        let area = edge(p0.0, p0.1, p1.0, p1.1, p2.0, p2.1);
        if area.abs() < 1e-4 {
            return;
        }
        let inv_area = 1.0 / area;
        let a_scale = alpha_mul.clamp(0.0, 1.0);

        for y in min_y..max_y {
            let py = (y as f32) + 0.5;
            for x in min_x..max_x {
                let px = (x as f32) + 0.5;
                let w0 = edge(p1.0, p1.1, p2.0, p2.1, px, py) * inv_area;
                let w1 = edge(p2.0, p2.1, p0.0, p0.1, px, py) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                    let u = (w0 * p0.2 + w1 * p1.2 + w2 * p2.2).clamp(0.0, 1.0);
                    let v = (w0 * p0.3 + w1 * p1.3 + w2 * p2.3).clamp(0.0, 1.0);
                    let sx = ((u * (src_w as f32)).floor() as usize).min(src_w - 1);
                    let sy = ((v * (src_h as f32)).floor() as usize).min(src_h - 1);
                    let s_ofs = (sy * src_w + sx) * 4;
                    let sa = ((src_rgba[s_ofs + 3] as f32) * a_scale).round() as u8;
                    if sa > 0 {
                        self.blend_pixel(
                            x,
                            y,
                            [
                                src_rgba[s_ofs],
                                src_rgba[s_ofs + 1],
                                src_rgba[s_ofs + 2],
                                sa,
                            ],
                        );
                    }
                }
            }
        }
    }

    /// Draws an integer Bresenham line segment from `(x0, y0)` to `(x1, y1)` with `thickness` pixels.
    pub fn draw_line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        thickness: usize,
        color: [u8; 4],
    ) {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let half = (thickness.max(1) / 2) as i32;
        let t = thickness.max(1);

        loop {
            let px = (x - half).max(0) as usize;
            let py = (y - half).max(0) as usize;
            if x >= -half && y >= -half {
                self.fill_rect(px, py, t, t, color);
            }
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Rasterizes a solid-color 2D triangle given screen-space vertices `[(x, y); 3]`.
    pub fn fill_triangle(&mut self, verts: [(f32, f32); 3], color: [u8; 4]) {
        let [p0, p1, p2] = verts;
        let min_x = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as usize;
        let max_x = (p0.0.max(p1.0).max(p2.0).ceil().max(0.0) as usize).min(self.width);
        let min_y = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as usize;
        let max_y = (p0.1.max(p1.1).max(p2.1).ceil().max(0.0) as usize).min(self.height);
        if min_x >= max_x || min_y >= max_y {
            return;
        }
        let edge = |ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32| -> f32 {
            (px - ax) * (by - ay) - (py - ay) * (bx - ax)
        };
        let area = edge(p0.0, p0.1, p1.0, p1.1, p2.0, p2.1);
        if area.abs() < 1e-4 {
            return;
        }
        let inv_area = 1.0 / area;
        for y in min_y..max_y {
            let py = (y as f32) + 0.5;
            for x in min_x..max_x {
                let px = (x as f32) + 0.5;
                let w0 = edge(p1.0, p1.1, p2.0, p2.1, px, py) * inv_area;
                let w1 = edge(p2.0, p2.1, p0.0, p0.1, px, py) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                    self.blend_pixel(x, y, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_rect_opaque_and_translucent_match_blend_pixel_exactly() {
        type FillCase = ([u8; 4], (usize, usize, usize, usize), [u8; 4]);
        let cases: &[FillCase] = &[
            // Full-width opaque fill over colored background
            ([10, 20, 30, 180], (0, 0, 32, 24), [0, 0, 0, 255]),
            // Partial opaque fill
            ([40, 80, 120, 255], (5, 3, 17, 11), [210, 140, 60, 255]),
            // Translucent fill over transparent background
            ([0, 0, 0, 0], (2, 4, 20, 15), [100, 180, 240, 96]),
            // Translucent fill over existing semi-transparent pixels
            ([50, 90, 130, 140], (4, 2, 18, 16), [220, 60, 40, 110]),
            // Zero-alpha no-op
            ([12, 34, 56, 200], (0, 0, 32, 24), [255, 255, 255, 0]),
        ];

        for &(bg, (x0, y0, w, h), fg) in cases {
            let mut fast = TextRasterizer::new(32, 24);
            let mut ref_r = TextRasterizer::new(32, 24);
            for y in 0..24 {
                for x in 0..32 {
                    fast.blend_pixel(x, y, bg);
                    ref_r.blend_pixel(x, y, bg);
                }
            }

            fast.fill_rect(x0, y0, w, h, fg);
            let x1 = (x0 + w).min(ref_r.width);
            let y1 = (y0 + h).min(ref_r.height);
            for y in y0..y1 {
                for x in x0..x1 {
                    ref_r.blend_pixel(x, y, fg);
                }
            }
            assert_eq!(fast.pixels, ref_r.pixels);
        }
    }

    #[test]
    fn blit_rgba_subrect_scaled_matches_blit_overlay_triangles() {
        let (sw, sh) = (16usize, 16usize);
        let mut src = vec![0u8; sw * sh * 4];
        for y in 0..sh {
            for x in 0..sw {
                let o = (y * sw + x) * 4;
                src[o] = (x * 16) as u8;
                src[o + 1] = (y * 16) as u8;
                src[o + 2] = ((x + y) * 8) as u8;
                src[o + 3] = if (x + y) % 3 == 0 {
                    255
                } else if (x + y) % 3 == 1 {
                    160
                } else {
                    0
                };
            }
        }

        for &alpha_mul in &[1.0f32, 0.65f32] {
            let mut fast = TextRasterizer::new(64, 48);
            let mut ref_r = TextRasterizer::new(64, 48);
            fast.fill_rect(0, 0, 64, 48, [24, 28, 32, 255]);
            ref_r.fill_rect(0, 0, 64, 48, [24, 28, 32, 255]);

            let (x0, y0, x1, y1) = (8.0f32, 6.0f32, 52.0f32, 42.0f32);
            let (u0, v0, u1, v1) = (0.125f32, 0.25f32, 0.875f32, 0.9375f32);
            fast.blit_rgba_subrect_scaled(
                (x0, y0, x1, y1),
                (u0, v0, u1, v1),
                &src,
                sw,
                sh,
                alpha_mul,
            );

            // Exact per-pixel axis-aligned reference (single blend per pixel, no triangle diagonal double-blend)
            let a_scale = alpha_mul.clamp(0.0, 1.0);
            for y in 0..48usize {
                let py = y as f32 + 0.5;
                if py < y0 || py > y1 {
                    continue;
                }
                let v = (v0 + ((py - y0) / (y1 - y0)) * (v1 - v0)).clamp(0.0, 1.0);
                let sy = ((v * sh as f32).floor() as usize).min(sh - 1);
                for x in 0..64usize {
                    let px = x as f32 + 0.5;
                    if px < x0 || px > x1 {
                        continue;
                    }
                    let u = (u0 + ((px - x0) / (x1 - x0)) * (u1 - u0)).clamp(0.0, 1.0);
                    let sx = ((u * sw as f32).floor() as usize).min(sw - 1);
                    let s_ofs = (sy * sw + sx) * 4;
                    let sa = (src[s_ofs + 3] as f32 * a_scale).round() as u8;
                    if sa > 0 {
                        ref_r.blend_pixel(x, y, [src[s_ofs], src[s_ofs + 1], src[s_ofs + 2], sa]);
                    }
                }
            }

            assert_eq!(
                fast.pixels, ref_r.pixels,
                "blit_rgba_subrect_scaled differed from reference at alpha_mul={alpha_mul}"
            );
        }

        // Also verify exact match against `blit_overlay_triangles` for fully opaque textures
        let mut opaque_src = src.clone();
        for px in opaque_src.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
        let mut fast_op = TextRasterizer::new(64, 48);
        let mut tri_op = TextRasterizer::new(64, 48);
        let (x0, y0, x1, y1) = (8.0f32, 6.0f32, 52.0f32, 42.0f32);
        let (u0, v0, u1, v1) = (0.125f32, 0.25f32, 0.875f32, 0.9375f32);
        fast_op.blit_rgba_subrect_scaled(
            (x0, y0, x1, y1),
            (u0, v0, u1, v1),
            &opaque_src,
            sw,
            sh,
            1.0,
        );
        let tris = [
            (x0, y0, u0, v0),
            (x1, y0, u1, v0),
            (x1, y1, u1, v1),
            (x0, y0, u0, v0),
            (x1, y1, u1, v1),
            (x0, y1, u0, v1),
        ];
        tri_op.blit_overlay_triangles(&tris, &opaque_src, sw, sh, 1.0);
        assert_eq!(fast_op.pixels, tri_op.pixels);
    }
}
