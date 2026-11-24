//! Renderer for `GameoverMain` (`0x001c4c70`) / `draw_opd_wowk_main` (`0x001c4de0`).

use super::state::{GameOverPhase, GameOverScreen};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::widgets::Ps2Canvas;

/// Retail PC / PS2 Game Over texture path (`sh2pc.exe` `0x007ba718` / `FUN_00497ad0` `0x00497ad0`).
pub const GAMEOVER_TEX_PATH: &str = "pic/etc/gameover1.tex";

/// Renders the Game Over screen onto `rasterizer` using `gameover1.tex` if available,
/// or a crisp fallback `"GAME OVER"` title if texture bytes were not loaded.
pub fn draw_gameover_screen(
    screen: &GameOverScreen,
    gameover_tex_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    rasterizer.clear();
    rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);
    if screen.phase == GameOverPhase::Done || screen.ps2_alpha == 0 {
        return;
    }

    let c = Ps2Canvas::from_viewport(rasterizer.width, rasterizer.height);
    let alpha = screen.rgba_alpha();

    if let Some((tex_rgba, tex_w, tex_h)) = gameover_tex_rgba {
        // `opd_work_init` (`0x001c4a70`) & `draw_opd_wowk_main` (`0x001c4de0`):
        // PS2 `gameover1.tex` is 512x512 (`u=0..160`, `v=0..48`, `opd_w` strips at `v=(band+1)*48`);
        // SH2:EE HD `gameover1.tex` scales 512x512 -> 2048x2048 (`uv_scale = tex_w / 512`).
        let uv_scale_x = (tex_w as f32 / 512.0).max(1.0);
        let uv_scale_y = (tex_h as f32 / 512.0).max(1.0);
        let src_w = ((160.0 * uv_scale_x).round() as usize).min(tex_w).max(1);
        let src_h = ((48.0 * uv_scale_y).round() as usize).min(tex_h).max(1);
        let strip_h = ((47.0 * uv_scale_y).round() as usize).min(tex_h).max(1);

        // Screen quad: `-80..+80` x `-24..+23` (`160x48` centered at `(256, 224)` in `512x448`).
        let dst_x = c.xi(256.0 - 80.0);
        let dst_w = c.x(160.0).round().max(1.0) as usize;
        let dst_h = c.y(48.0).round().max(1.0) as usize;

        // 1. 6 jittering background strips (`opd_w`, `rand() % 30 - 15` vertical jitter).
        if (screen.phase as u8) >= (GameOverPhase::Hold as u8) {
            for (i, &jitter) in screen.strip_jitter.iter().enumerate() {
                let band = (i % 2) + 1;
                let src_y0 = (((band as f32) * 48.0 * uv_scale_y).round() as usize)
                    .min(tex_h.saturating_sub(strip_h));
                let dst_y = c.yi(224.0 - 24.0 + jitter);
                let strip_alpha = (u16::from(alpha) * 2 / 3) as u8;
                blit_tex_region_tinted(
                    rasterizer,
                    tex_rgba,
                    tex_w,
                    0,
                    src_y0,
                    src_w,
                    strip_h,
                    dst_x,
                    dst_y,
                    dst_w,
                    dst_h,
                    strip_alpha,
                );
            }
        }

        // 2. Main `org_title` (`u=0..160, v=0..48` scaled by `uv_scale`, centered at `(256, 224)`).
        let dst_y = c.yi(224.0 - 24.0);
        blit_tex_region_tinted(
            rasterizer, tex_rgba, tex_w, 0, 0, src_w, src_h, dst_x, dst_y, dst_w, dst_h, alpha,
        );
    } else {
        let font_px = ((30.0 * c.sy).round() as usize).max(16);
        let col = Sh2TextColor::Custom {
            top: [200, 40, 40, alpha],
            bottom: [128, 16, 16, alpha],
        };
        font.draw_line_centered(
            rasterizer,
            "GAME OVER",
            (c.xi(256.0), c.yi(224.0).saturating_sub(font_px / 2)),
            font_px,
            col,
            true,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_tex_region_tinted(
    dst: &mut TextRasterizer,
    src_rgba: &[u8],
    src_stride_w: usize,
    src_x0: usize,
    src_y0: usize,
    src_w: usize,
    src_h: usize,
    dst_x: usize,
    dst_y: usize,
    dst_w: usize,
    dst_h: usize,
    alpha_scale: u8,
) {
    if dst_w == 0 || dst_h == 0 || src_w == 0 || src_h == 0 {
        return;
    }
    let a_mul = f32::from(alpha_scale) / 255.0;
    for dy in 0..dst_h {
        let py = dst_y + dy;
        if py >= dst.height {
            break;
        }
        let sy = src_y0 + (dy * src_h) / dst_h;
        for dx in 0..dst_w {
            let px = dst_x + dx;
            if px >= dst.width {
                break;
            }
            let sx = src_x0 + (dx * src_w) / dst_w;
            let si = (sy * src_stride_w + sx) * 4;
            if si + 3 >= src_rgba.len() {
                continue;
            }
            let sa = (f32::from(src_rgba[si + 3]) * a_mul).round() as u8;
            if sa > 0 {
                dst.blend_pixel(
                    px,
                    py,
                    [src_rgba[si], src_rgba[si + 1], src_rgba[si + 2], sa],
                );
            }
        }
    }
}
