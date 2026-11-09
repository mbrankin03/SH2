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
            let inv_alpha = 255 - alpha;
            let dst_r = self.pixels[offset] as u32;
            let dst_g = self.pixels[offset + 1] as u32;
            let dst_b = self.pixels[offset + 2] as u32;
            let dst_a = self.pixels[offset + 3] as u32;

            self.pixels[offset] = ((color[0] as u32 * alpha + dst_r * inv_alpha) / 255) as u8;
            self.pixels[offset + 1] = ((color[1] as u32 * alpha + dst_g * inv_alpha) / 255) as u8;
            self.pixels[offset + 2] = ((color[2] as u32 * alpha + dst_b * inv_alpha) / 255) as u8;
            self.pixels[offset + 3] = (alpha + (dst_a * inv_alpha) / 255).min(255) as u8;
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
    pub fn fill_rect(&mut self, x0: usize, y0: usize, w: usize, h: usize, color: [u8; 4]) {
        let x1 = (x0 + w).min(self.width);
        let y1 = (y0 + h).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend_pixel(x, y, color);
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
        if dst_w == 0
            || dst_h == 0
            || src_w == 0
            || src_h == 0
            || src_rgba.len() < src_w * src_h * 4
            || alpha_mul <= 0.0
        {
            return;
        }
        let a_scale = alpha_mul.clamp(0.0, 1.0);
        let x_end = (dst_x + dst_w).min(self.width);
        let y_end = (dst_y + dst_h).min(self.height);

        for y in dst_y..y_end {
            let v = ((y - dst_y) as f32 + 0.5) / (dst_h as f32);
            let sy = ((v * (src_h as f32)).floor() as usize).min(src_h - 1);
            for x in dst_x..x_end {
                let u = ((x - dst_x) as f32 + 0.5) / (dst_w as f32);
                let sx = ((u * (src_w as f32)).floor() as usize).min(src_w - 1);
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
        if src_w == 0
            || src_h == 0
            || src_rgba.len() < src_w * src_h * 4
            || alpha_mul <= 0.0
        {
            return;
        }
        let min_x = p0
            .0
            .min(p1.0)
            .min(p2.0)
            .floor()
            .max(0.0) as usize;
        let max_x = (p0.0.max(p1.0).max(p2.0).ceil().max(0.0) as usize).min(self.width);
        let min_y = p0
            .1
            .min(p1.1)
            .min(p2.1)
            .floor()
            .max(0.0) as usize;
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
}
