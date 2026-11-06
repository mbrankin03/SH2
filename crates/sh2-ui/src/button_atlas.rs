//! Authentic 2D Title Button Atlas & Glyph Rasterizer from `start01e.tex`.
//! Ground truth references:
//! - PS2 July 13, 2001 Prototype: `title.c` (`TitleSpr` at `0x0033b880`, `titleDrawMainMenu`, `titleDrawSprite`).
//! - Retail PC / SH2:EE: `start01e.tex` (2048x2048 32-bit RGBA sprite atlas).

/// Bounding rectangle in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Identifiers for authentic sprite buttons in `start01e.tex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticButtonSprite {
    Load,
    Continue,
    NewGame,
    Options,
    Movies,
    Exit,
    ActionLevel,
    RiddleLevel,
    Hard,
    Normal,
    Easy,
    Beginner,
    GlowCursor,
    MainScenario,
    SubScenario,
    StartPrompt,
    TestRooms,
}

impl AuthenticButtonSprite {
    /// Exact trimmed pixel rects inside the 2048x2048 `start01e.tex` atlas.
    pub fn atlas_rect(&self) -> AtlasRect {
        match self {
            Self::Load => AtlasRect {
                x: 273,
                y: 399,
                width: 263,
                height: 98,
            },
            Self::Continue => AtlasRect {
                x: 170,
                y: 531,
                width: 466,
                height: 98,
            },
            Self::NewGame => AtlasRect {
                x: 161,
                y: 655,
                width: 487,
                height: 98,
            },
            Self::Options => AtlasRect {
                x: 191,
                y: 783,
                width: 422,
                height: 98,
            },
            Self::Movies => AtlasRect {
                x: 268,
                y: 909,
                width: 265,
                height: 98,
            },
            Self::Exit => AtlasRect {
                x: 299,
                y: 1685,
                width: 226,
                height: 97,
            },
            Self::ActionLevel => AtlasRect {
                x: 887,
                y: 401,
                width: 556,
                height: 92,
            },
            Self::RiddleLevel => AtlasRect {
                x: 897,
                y: 1045,
                width: 540,
                height: 91,
            },
            Self::Hard => AtlasRect {
                x: 1060,
                y: 530,
                width: 225,
                height: 90,
            },
            Self::Normal => AtlasRect {
                x: 1009,
                y: 657,
                width: 329,
                height: 92,
            },
            Self::Easy => AtlasRect {
                x: 1068,
                y: 789,
                width: 211,
                height: 92,
            },
            Self::Beginner => AtlasRect {
                x: 977,
                y: 917,
                width: 375,
                height: 91,
            },
            Self::GlowCursor => AtlasRect {
                x: 792,
                y: 1665,
                width: 757,
                height: 126,
            },
            Self::MainScenario => AtlasRect {
                x: 86,
                y: 1804,
                width: 1400,
                height: 99,
            },
            Self::SubScenario => AtlasRect {
                x: 246,
                y: 1932,
                width: 1073,
                height: 99,
            },
            Self::StartPrompt => AtlasRect {
                x: 0,
                y: 0,
                width: 1102,
                height: 84,
            },
            Self::TestRooms => AtlasRect {
                x: 0,
                y: 0,
                width: 390,
                height: 98,
            },
        }
    }
}

/// Loaded button sprite sheet buffer (2048x2048 RGBA8).
pub struct ButtonSpriteSheet {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub start_prompt_rgba: Option<Vec<u8>>,
    pub start_prompt_width: usize,
    pub start_prompt_height: usize,
    pub test_rooms_rgba: Option<Vec<u8>>,
    pub test_rooms_width: usize,
    pub test_rooms_height: usize,
}

impl ButtonSpriteSheet {
    /// Load from raw RGBA8 pixels (2048x2048).
    pub fn new(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        let (prompt_rgba, p_w, p_h) = Self::load_default_start_prompt();
        let (test_rooms_rgba, tr_w, tr_h) = Self::load_default_test_rooms();
        Self {
            width,
            height,
            rgba,
            start_prompt_rgba: prompt_rgba,
            start_prompt_width: p_w,
            start_prompt_height: p_h,
            test_rooms_rgba,
            test_rooms_width: tr_w,
            test_rooms_height: tr_h,
        }
    }

    fn load_default_start_prompt() -> (Option<Vec<u8>>, usize, usize) {
        let bytes = include_bytes!("../assets/start_prompt.png");
        if let Ok(img) = image::load_from_memory(bytes) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            (Some(rgba.into_raw()), w as usize, h as usize)
        } else {
            (None, 0, 0)
        }
    }

    fn load_default_test_rooms() -> (Option<Vec<u8>>, usize, usize) {
        let bytes = include_bytes!("../assets/test_rooms_btn.png");
        if let Ok(img) = image::load_from_memory(bytes) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            (Some(rgba.into_raw()), w as usize, h as usize)
        } else {
            (None, 0, 0)
        }
    }

    /// Blit a button sprite with alpha-blending and tint onto a target RGBA8 buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_sprite(
        &self,
        sprite: AuthenticButtonSprite,
        dst: &mut [u8],
        dst_width: usize,
        dst_height: usize,
        dst_x: i32,
        dst_y: i32,
        scale: f32,
        tint: [f32; 4],
    ) {
        let is_prompt = sprite == AuthenticButtonSprite::StartPrompt;
        let is_test_rooms = sprite == AuthenticButtonSprite::TestRooms;
        let (src_bytes, src_w, rect) = if is_prompt {
            if let Some(ref p_bytes) = self.start_prompt_rgba {
                (
                    p_bytes.as_slice(),
                    self.start_prompt_width,
                    AtlasRect {
                        x: 0,
                        y: 0,
                        width: self.start_prompt_width,
                        height: self.start_prompt_height,
                    },
                )
            } else {
                return;
            }
        } else if is_test_rooms {
            if let Some(ref tr_bytes) = self.test_rooms_rgba {
                (
                    tr_bytes.as_slice(),
                    self.test_rooms_width,
                    AtlasRect {
                        x: 0,
                        y: 0,
                        width: self.test_rooms_width,
                        height: self.test_rooms_height,
                    },
                )
            } else {
                return;
            }
        } else {
            (self.rgba.as_slice(), self.width, sprite.atlas_rect())
        };

        let scaled_w = (rect.width as f32 * scale).round() as usize;
        let scaled_h = (rect.height as f32 * scale).round() as usize;

        for dy in 0..scaled_h {
            let out_y = dst_y + dy as i32;
            if out_y < 0 || out_y >= dst_height as i32 {
                continue;
            }

            let continuous_y = rect.y as f32 + (dy as f32 / scale);
            let y0 = (continuous_y.floor() as usize).min(rect.y + rect.height - 1);
            let y1 = (y0 + 1).min(rect.y + rect.height - 1);
            let fy = continuous_y - y0 as f32;

            for dx in 0..scaled_w {
                let out_x = dst_x + dx as i32;
                if out_x < 0 || out_x >= dst_width as i32 {
                    continue;
                }

                let continuous_x = rect.x as f32 + (dx as f32 / scale);
                let x0 = (continuous_x.floor() as usize).min(rect.x + rect.width - 1);
                let x1 = (x0 + 1).min(rect.x + rect.width - 1);
                let fx = continuous_x - x0 as f32;

                let idx00 = (y0 * src_w + x0) * 4;
                let idx10 = (y0 * src_w + x1) * 4;
                let idx01 = (y1 * src_w + x0) * 4;
                let idx11 = (y1 * src_w + x1) * 4;

                let sample = |c: usize| -> f32 {
                    let v00 = src_bytes[idx00 + c] as f32;
                    let v10 = src_bytes[idx10 + c] as f32;
                    let v01 = src_bytes[idx01 + c] as f32;
                    let v11 = src_bytes[idx11 + c] as f32;
                    let top = v00 * (1.0 - fx) + v10 * fx;
                    let bot = v01 * (1.0 - fx) + v11 * fx;
                    top * (1.0 - fy) + bot * fy
                };

                let s_r = sample(0) * tint[0];
                let s_g = sample(1) * tint[1];
                let s_b = sample(2) * tint[2];
                let s_a = (sample(3) / 255.0) * tint[3];

                if s_a <= 0.005 {
                    continue;
                }

                let dst_idx = (out_y as usize * dst_width + out_x as usize) * 4;
                let d_r = dst[dst_idx] as f32;
                let d_g = dst[dst_idx + 1] as f32;
                let d_b = dst[dst_idx + 2] as f32;
                let d_a = dst[dst_idx + 3] as f32 / 255.0;

                // Alpha composite (over operator)
                let out_a = s_a + d_a * (1.0 - s_a);
                if out_a > 0.0 {
                    let out_r = (s_r * s_a + d_r * d_a * (1.0 - s_a)) / out_a;
                    let out_g = (s_g * s_a + d_g * d_a * (1.0 - s_a)) / out_a;
                    let out_b = (s_b * s_a + d_b * d_a * (1.0 - s_a)) / out_a;

                    dst[dst_idx] = out_r.clamp(0.0, 255.0) as u8;
                    dst[dst_idx + 1] = out_g.clamp(0.0, 255.0) as u8;
                    dst[dst_idx + 2] = out_b.clamp(0.0, 255.0) as u8;
                    dst[dst_idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    /// Renders an authentic 1:1 PS2 procedural elliptical selection glow cursor directly onto `dst`.
    ///
    /// The PS2 original authored the selection highlight as a pure white horizontal ellipse with a
    /// solid white core (`r <= 0.45`) and an exponential Gaussian-like soft falloff (`exp(-4.2 * t^1.4)`),
    /// feathered across the outer 55% of its bounds.
    ///
    /// To eliminate 8-bit quantized contour banding and stepping on dark backdrops (such as Toluca
    /// Lake water or options panels), this method evaluates the exact continuous distance field and
    /// applies 8x8 Bayer spatial matrix dithering across the alpha composite channel.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_analytic_glow_cursor(
        dst: &mut [u8],
        dst_width: usize,
        dst_height: usize,
        center_x: i32,
        center_y: i32,
        glow_width: i32,
        glow_height: i32,
        tint: [f32; 4],
    ) {
        if glow_width <= 0 || glow_height <= 0 {
            return;
        }

        let half_w = glow_width as f32 * 0.5;
        let half_h = glow_height as f32 * 0.5;
        let rx = half_w * 0.965;
        let ry = half_h * 0.965;
        if rx <= 0.001 || ry <= 0.001 {
            return;
        }

        let min_x = (center_x - (glow_width / 2)).max(0);
        let max_x = (center_x + (glow_width / 2)).min(dst_width as i32 - 1);
        let min_y = (center_y - (glow_height / 2)).max(0);
        let max_y = (center_y + (glow_height / 2)).min(dst_height as i32 - 1);

        if min_x > max_x || min_y > max_y {
            return;
        }

        // Standard 8x8 Bayer matrix for spatial alpha dithering
        const BAYER_8X8: [[f32; 8]; 8] = [
            [
                0.0 / 64.0,
                32.0 / 64.0,
                8.0 / 64.0,
                40.0 / 64.0,
                2.0 / 64.0,
                34.0 / 64.0,
                10.0 / 64.0,
                42.0 / 64.0,
            ],
            [
                48.0 / 64.0,
                16.0 / 64.0,
                56.0 / 64.0,
                24.0 / 64.0,
                50.0 / 64.0,
                18.0 / 64.0,
                58.0 / 64.0,
                26.0 / 64.0,
            ],
            [
                12.0 / 64.0,
                44.0 / 64.0,
                4.0 / 64.0,
                36.0 / 64.0,
                14.0 / 64.0,
                46.0 / 64.0,
                6.0 / 64.0,
                38.0 / 64.0,
            ],
            [
                60.0 / 64.0,
                28.0 / 64.0,
                52.0 / 64.0,
                20.0 / 64.0,
                62.0 / 64.0,
                30.0 / 64.0,
                54.0 / 64.0,
                22.0 / 64.0,
            ],
            [
                3.0 / 64.0,
                35.0 / 64.0,
                11.0 / 64.0,
                43.0 / 64.0,
                1.0 / 64.0,
                33.0 / 64.0,
                9.0 / 64.0,
                41.0 / 64.0,
            ],
            [
                51.0 / 64.0,
                19.0 / 64.0,
                59.0 / 64.0,
                27.0 / 64.0,
                49.0 / 64.0,
                17.0 / 64.0,
                57.0 / 64.0,
                25.0 / 64.0,
            ],
            [
                15.0 / 64.0,
                47.0 / 64.0,
                7.0 / 64.0,
                39.0 / 64.0,
                13.0 / 64.0,
                45.0 / 64.0,
                5.0 / 64.0,
                37.0 / 64.0,
            ],
            [
                63.0 / 64.0,
                31.0 / 64.0,
                55.0 / 64.0,
                23.0 / 64.0,
                61.0 / 64.0,
                29.0 / 64.0,
                53.0 / 64.0,
                21.0 / 64.0,
            ],
        ];

        let s_r = 255.0 * tint[0];
        let s_g = 255.0 * tint[1];
        let s_b = 255.0 * tint[2];
        let base_alpha = tint[3];

        for out_y in min_y..=max_y {
            let dy = out_y as f32 - center_y as f32;
            let ny = dy / ry;
            let ny_sq = ny * ny;
            if ny_sq >= 1.15 {
                continue;
            }

            let bayer_row = &BAYER_8X8[(out_y as usize) & 7];
            let row_offset = out_y as usize * dst_width;

            for out_x in min_x..=max_x {
                let dx = out_x as f32 - center_x as f32;
                let nx = dx / rx;
                let r_sq = nx * nx + ny_sq;
                if r_sq >= 1.15 {
                    continue;
                }

                let r = r_sq.sqrt();
                let analytic_alpha = if r <= 0.45 {
                    1.0f32
                } else if r >= 1.05 {
                    0.0f32
                } else {
                    let t = (r - 0.45) / 0.55;
                    (-4.2 * t.powf(1.4)).exp()
                };

                if analytic_alpha <= 0.002 {
                    continue;
                }

                // Spatial Bayer dither offset in range [-0.5 / 255, +0.5 / 255]
                let dither = (bayer_row[(out_x as usize) & 7] - 0.5) / 255.0;
                let s_a = ((analytic_alpha * base_alpha) + dither).clamp(0.0, 1.0);
                if s_a <= 0.004 {
                    continue;
                }

                let dst_idx = (row_offset + out_x as usize) * 4;
                let d_r = dst[dst_idx] as f32;
                let d_g = dst[dst_idx + 1] as f32;
                let d_b = dst[dst_idx + 2] as f32;
                let d_a = dst[dst_idx + 3] as f32 / 255.0;

                // Alpha composite (over operator)
                let out_a = s_a + d_a * (1.0 - s_a);
                if out_a > 0.0 {
                    let blended_r = (s_r * s_a + d_r * d_a * (1.0 - s_a)) / out_a;
                    let blended_g = (s_g * s_a + d_g * d_a * (1.0 - s_a)) / out_a;
                    let blended_b = (s_b * s_a + d_b * d_a * (1.0 - s_a)) / out_a;

                    dst[dst_idx] = blended_r.clamp(0.0, 255.0) as u8;
                    dst[dst_idx + 1] = blended_g.clamp(0.0, 255.0) as u8;
                    dst[dst_idx + 2] = blended_b.clamp(0.0, 255.0) as u8;
                    dst[dst_idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}
