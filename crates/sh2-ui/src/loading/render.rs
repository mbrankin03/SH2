//! Software rasterizer rendering for both retail PC footstep loading screens
//! (`sh2pc.exe` `FUN_00406be0`, `FUN_004c87e0`) and PS2 prototype jittered text
//! (`SLUS_202.28` `now_loading.c` `NowLoadingDraw` `0x00299170`).

use super::sprite::FootstepSprite;
use super::{
    FootstepPrint, LoadingScreen, LoadingScreenMode, FOOTSTEP_UV_MAX, FOOTSTEP_UV_MIN,
    NOW_LOADING_NORM_X, NOW_LOADING_NORM_Y,
};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;

impl LoadingScreen {
    /// Renders the active loading screen mode onto `rast`:
    /// - [`LoadingScreenMode::RetailPcFootsteps`]: `sh2pc.exe` `FUN_00406be0` / `FUN_004c87e0`
    /// - [`LoadingScreenMode::Ps2NowLoadingJitter`]: `SLUS_202.28` `NowLoadingDraw` (`0x00299170`)
    pub fn render(&self, font: &AtlasFont, rast: &mut TextRasterizer) {
        // Fill opaque pitch-black backdrop (`#000000`, `FUN_00406ab0` / `NowLoadingCheck`)
        let (chunks, _) = rast.pixels.as_chunks_mut::<4>();
        for px in chunks {
            *px = [0, 0, 0, 255];
        }

        let scale_x = rast.width as f32 / 512.0;
        let scale_y = rast.height as f32 / 480.0;

        match self.mode {
            LoadingScreenMode::RetailPcFootsteps => {
                self.render_retail_pc_footsteps(font, rast, scale_x, scale_y);
            }
            LoadingScreenMode::Ps2NowLoadingJitter => {
                self.render_ps2_now_loading_jitter(font, rast, scale_x, scale_y);
            }
        }
    }

    fn render_retail_pc_footsteps(
        &self,
        font: &AtlasFont,
        rast: &mut TextRasterizer,
        scale_x: f32,
        scale_y: f32,
    ) {
        let sprite = FootstepSprite::get();

        for step in &self.footsteps {
            let scr_x = (step.x * scale_x).round() as i32;
            let scr_y = (step.y * scale_y).round() as i32;
            let quad_h_px = (step.scale * 2.0) * scale_y;
            let print_scale = quad_h_px / sprite.height.max(1) as f32;
            draw_footstep_silhouette(rast, sprite, scr_x, scr_y, step, print_scale);
        }

        // `"NOW LOADING..."` (`common_msg_e.mes` ID 25) at `(480/640 * W, 424/480 * H)`, `fontWide(16, 24)`
        let text_x = (rast.width as f32 * NOW_LOADING_NORM_X).round() as usize;
        let text_y = (rast.height as f32 * NOW_LOADING_NORM_Y).round() as usize;
        let text_h = (24.0 * scale_y * 0.7).round().max(11.0) as usize;

        if self.load_completed
            && self.elapsed_seconds >= self.min_duration
            && self.require_press_to_continue
        {
            let blink_visible = (self.elapsed_seconds * 2.5).sin() > -0.2;
            if blink_visible {
                font.draw_line_centered(
                    rast,
                    "PRESS [ENTER / A] TO CONTINUE",
                    (rast.width / 2, text_y),
                    text_h,
                    Sh2TextColor::White,
                    true,
                );
            }
        } else {
            font.draw_line_centered(
                rast,
                "NOW LOADING...",
                (text_x.min(rast.width.saturating_sub(90)), text_y),
                text_h,
                Sh2TextColor::White,
                true,
            );
        }
    }

    /// Renders `SLUS_202.28` `now_loading.c` `NowLoadingDraw` (`0x00299170`):
    /// `fontSetColorDirect(0x80, 0x80, 0x80, 0x80)` (`[255, 255, 255, alpha]`)
    /// `fontPrintStr("\c\hNow loading", (count * 0xce5fdd) % 5 + 0xfe, (count * 0x2f6f) % 5 + 0xfe)`
    /// with 4.0s fade decay (`ScreenEffectFadeStart(4.0f, 1)` at `0x00298fc0`).
    fn render_ps2_now_loading_jitter(
        &self,
        font: &AtlasFont,
        rast: &mut TextRasterizer,
        scale_x: f32,
        scale_y: f32,
    ) {
        let alpha_u8 = (self.ps2_text_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        if alpha_u8 > 0 {
            let text_x = (self.ps2_text_x_512 as f32 * scale_x).round().max(0.0) as usize;
            let text_y = (self.ps2_text_y_480 as f32 * scale_y).round().max(0.0) as usize;
            // `\h` half-size font in `SLUS_202.28` (`0x39c5e0` = `"\c\hNow loading"`).
            let text_h = (24.0 * scale_y * 0.7).round().max(11.0) as usize;
            let col = [255, 255, 255, alpha_u8];
            font.draw_line_centered(
                rast,
                "Now loading",
                (text_x, text_y),
                text_h,
                Sh2TextColor::Custom {
                    top: col,
                    bottom: col,
                },
                false,
            );
        }
    }
}

/// Blits a footstep silhouette using exact `sh2pc.exe` UV crop `[3/64..61/64]` (`0x00890af0` / `0x00890b10`).
fn draw_footstep_silhouette(
    rast: &mut TextRasterizer,
    sprite: &FootstepSprite,
    center_x: i32,
    center_y: i32,
    step: &FootstepPrint,
    scale: f32,
) {
    let spr_w = sprite.width as f32;
    let spr_h = sprite.height as f32;
    let rad = ((spr_w * spr_w + spr_h * spr_h).sqrt() * scale * 0.5).ceil() as i32;

    let x_start = (center_x - rad).max(0);
    let x_end = (center_x + rad).min(rast.width as i32 - 1);
    let y_start = (center_y - rad).max(0);
    let y_end = (center_y + rad).min(rast.height as i32 - 1);

    let cos_a = (-step.angle).cos();
    let sin_a = (-step.angle).sin();
    let inv_scale = 1.0 / scale.max(0.001);
    let uv_span = FOOTSTEP_UV_MAX - FOOTSTEP_UV_MIN;

    for y in y_start..=y_end {
        let dy = (y - center_y) as f32 * inv_scale;
        for x in x_start..=x_end {
            let dx = (x - center_x) as f32 * inv_scale;
            let local_x = dx * cos_a - dy * sin_a;
            let local_y = dx * sin_a + dy * cos_a;

            let norm_x = (local_x / spr_w) + 0.5;
            let norm_y = (local_y / spr_h) + 0.5;
            if !(0.0..1.0).contains(&norm_x) || !(0.0..1.0).contains(&norm_y) {
                continue;
            }

            let u = if step.is_right {
                FOOTSTEP_UV_MAX - norm_x * uv_span
            } else {
                FOOTSTEP_UV_MIN + norm_x * uv_span
            };
            let v = FOOTSTEP_UV_MIN + norm_y * uv_span;

            let fx = (u * spr_w - 0.5).clamp(0.0, spr_w - 1.0);
            let fy = (v * spr_h - 0.5).clamp(0.0, spr_h - 1.0);
            let x0 = (fx.floor() as u32).min(sprite.width - 1);
            let y0 = (fy.floor() as u32).min(sprite.height - 1);
            let x1 = (x0 + 1).min(sprite.width - 1);
            let y1 = (y0 + 1).min(sprite.height - 1);
            let rx = fx - x0 as f32;
            let ry = fy - y0 as f32;

            let sample_premul = |tx: u32, ty: u32| -> [f32; 4] {
                let i = ((ty * sprite.width + tx) * 4) as usize;
                let a = sprite.rgba[i + 3] as f32;
                let a_norm = a / 255.0;
                [
                    sprite.rgba[i] as f32 * a_norm,
                    sprite.rgba[i + 1] as f32 * a_norm,
                    sprite.rgba[i + 2] as f32 * a_norm,
                    a,
                ]
            };

            let p00 = sample_premul(x0, y0);
            let p10 = sample_premul(x1, y0);
            let p01 = sample_premul(x0, y1);
            let p11 = sample_premul(x1, y1);

            let w00 = (1.0 - rx) * (1.0 - ry);
            let w10 = rx * (1.0 - ry);
            let w01 = (1.0 - rx) * ry;
            let w11 = rx * ry;

            let interp_a = p00[3] * w00 + p10[3] * w10 + p01[3] * w01 + p11[3] * w11;
            if interp_a <= 0.001 {
                continue;
            }
            let interp_r = p00[0] * w00 + p10[0] * w10 + p01[0] * w01 + p11[0] * w11;
            let interp_g = p00[1] * w00 + p10[1] * w10 + p01[1] * w01 + p11[1] * w11;
            let interp_b = p00[2] * w00 + p10[2] * w10 + p01[2] * w01 + p11[2] * w11;

            // `sh2pc.exe` `FUN_004c87e0`: `D3DBLEND_SRCALPHA` / `D3DBLEND_INVSRCALPHA` over black
            // with full `footmark.tbn2` RGBA texture (`D3DTA_TEXTURE`) modulated by `step.alpha`.
            let effective_alpha = (interp_a / 255.0) * step.alpha;
            let inv_alpha = 1.0 - effective_alpha;
            let dest_idx = (y as usize * rast.width + x as usize) * 4;
            let er = rast.pixels[dest_idx] as f32;
            let eg = rast.pixels[dest_idx + 1] as f32;
            let eb = rast.pixels[dest_idx + 2] as f32;

            rast.pixels[dest_idx] = (interp_r * step.alpha + er * inv_alpha).min(255.0) as u8;
            rast.pixels[dest_idx + 1] = (interp_g * step.alpha + eg * inv_alpha).min(255.0) as u8;
            rast.pixels[dest_idx + 2] = (interp_b * step.alpha + eb * inv_alpha).min(255.0) as u8;
            rast.pixels[dest_idx + 3] = 255;
        }
    }
}
