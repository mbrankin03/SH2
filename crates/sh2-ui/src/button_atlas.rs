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
}

impl AuthenticButtonSprite {
    /// Exact trimmed pixel rects inside the 2048x2048 `start01e.tex` atlas.
    pub fn atlas_rect(&self) -> AtlasRect {
        match self {
            Self::Load => AtlasRect {
                x: 264,
                y: 411,
                width: 246,
                height: 73,
            },
            Self::Continue => AtlasRect {
                x: 165,
                y: 539,
                width: 447,
                height: 78,
            },
            Self::NewGame => AtlasRect {
                x: 152,
                y: 666,
                width: 472,
                height: 78,
            },
            Self::Options => AtlasRect {
                x: 185,
                y: 799,
                width: 404,
                height: 68,
            },
            Self::Exit => AtlasRect {
                x: 290,
                y: 1692,
                width: 200,
                height: 83,
            },
            Self::ActionLevel => AtlasRect {
                x: 890,
                y: 411,
                width: 510,
                height: 73,
            },
            Self::RiddleLevel => AtlasRect {
                x: 888,
                y: 1053,
                width: 507,
                height: 76,
            },
            Self::Hard => AtlasRect {
                x: 1051,
                y: 539,
                width: 209,
                height: 74,
            },
            Self::Normal => AtlasRect {
                x: 1000,
                y: 666,
                width: 300,
                height: 75,
            },
            Self::Easy => AtlasRect {
                x: 1060,
                y: 799,
                width: 185,
                height: 68,
            },
            Self::Beginner => AtlasRect {
                x: 968,
                y: 926,
                width: 360,
                height: 71,
            },
            Self::GlowCursor => AtlasRect {
                x: 843,
                y: 1687,
                width: 621,
                height: 88,
            },
            Self::MainScenario => AtlasRect {
                x: 84,
                y: 1813,
                width: 1378,
                height: 81,
            },
            Self::SubScenario => AtlasRect {
                x: 241,
                y: 1942,
                width: 1054,
                height: 80,
            },
            Self::StartPrompt => AtlasRect {
                x: 0,
                y: 0,
                width: 824,
                height: 70,
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
}

impl ButtonSpriteSheet {
    /// Load from raw RGBA8 pixels (2048x2048).
    pub fn new(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        let (prompt_rgba, p_w, p_h) = Self::load_default_start_prompt();
        Self {
            width,
            height,
            rgba,
            start_prompt_rgba: prompt_rgba,
            start_prompt_width: p_w,
            start_prompt_height: p_h,
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

            let src_y = rect.y + ((dy as f32 / scale) as usize).min(rect.height - 1);

            for dx in 0..scaled_w {
                let out_x = dst_x + dx as i32;
                if out_x < 0 || out_x >= dst_width as i32 {
                    continue;
                }

                let src_x = rect.x + ((dx as f32 / scale) as usize).min(rect.width - 1);
                let src_idx = (src_y * src_w + src_x) * 4;
                let dst_idx = (out_y as usize * dst_width + out_x as usize) * 4;

                let s_r = src_bytes[src_idx] as f32 * tint[0];
                let s_g = src_bytes[src_idx + 1] as f32 * tint[1];
                let s_b = src_bytes[src_idx + 2] as f32 * tint[2];
                let s_a = (src_bytes[src_idx + 3] as f32 / 255.0) * tint[3];

                if s_a <= 0.005 {
                    continue;
                }

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
}
