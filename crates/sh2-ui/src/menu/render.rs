//! Authentic 4:3-scaled Title Screen, Scenario, Side-by-Side Difficulty, Credits, and Test Room
//! rasterization (`SLUS_202.28` `Event/title.c` `0x001c3490..0x001c45a0` & `sh2pc.exe` `0x00496950..0x00497960`).
//!
//! Coordinate space:
//! - Centered PS2/PC virtual space `[-256..+256] × [-224..+224]` (512×448) mapped into a 4:3
//!   pillarboxed viewport inside any target [`TextRasterizer`] resolution.

use super::state::{TitleStateMachine, ACTION_DIFFICULTY_ROWS, RIDDLE_DIFFICULTY_ROWS};
use super::types::{MainMenuOption, TestRoomKind, TitlePhase};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::button_atlas::{AuthenticButtonSprite, ButtonSpriteSheet};
use crate::font::TextRasterizer;
use sh2_core::scenario::ScenarioKind;

/// Virtual 4:3 viewport projection mapping centered PS2 512×448 coordinates (`[-256..+256]`,
/// `[-224..+224]`) into pixel coordinates inside `rast`.
#[derive(Debug, Clone, Copy)]
struct Viewport43 {
    center_x: f32,
    center_y: f32,
    scale_x: f32,
    scale_y: f32,
    x_offset: f32,
    base_w: f32,
}

impl Viewport43 {
    fn from_rasterizer(rast: &TextRasterizer) -> Self {
        let w = rast.width as f32;
        let h = rast.height as f32;
        let target_4_3_w = h * (4.0 / 3.0);
        let (x_offset, base_w) = if target_4_3_w <= w {
            ((w - target_4_3_w) * 0.5, target_4_3_w)
        } else {
            (0.0, w)
        };
        Self {
            center_x: x_offset + base_w * 0.5,
            center_y: h * 0.5,
            scale_x: base_w / 512.0,
            scale_y: h / 448.0,
            x_offset,
            base_w,
        }
    }

    /// Maps centered PS2 512×448 `(cx, cy)` to screen pixel `(px, py)`.
    fn centered_to_px(&self, cx: f32, cy: f32) -> (i32, i32) {
        (
            (self.center_x + cx * self.scale_x).round() as i32,
            (self.center_y + cy * self.scale_y).round() as i32,
        )
    }

    /// Scales a 2048×2048 HD `start01e.tex` sprite (4× PS2 512×512 texels) into target pixels.
    fn hd_sprite_scale(&self) -> f32 {
        self.scale_y * 0.25
    }
}

/// Draws the animated `SILENT HILL 2` title logo overlay (`TitleSpr[0]` in `start01e.tex`)
/// locked pixel-for-pixel onto `sh2e/pic/etc/start00.tex` (`2732×2048`) at any window aspect.
///
/// Uses `SLUS_202.28` `titleDrawTitle` (`0x001c3490`) & `titleChangeColor` (`0x001c4740`, `7.5s` period).
pub fn draw_pulsing_title_logo_for_bg(
    sm: &TitleStateMachine,
    atlas: &ButtonSpriteSheet,
    rast: &mut TextRasterizer,
    bg_uv_rect: (f32, f32, f32, f32),
    bg_vp: (f32, f32, f32, f32),
    ui_vp: (f32, f32, f32, f32),
) {
    draw_pulsing_title_logo_with_style(sm, atlas, rast, bg_uv_rect, bg_vp, ui_vp, true);
}

/// Draws the animated `SILENT HILL 2` title logo overlay (`TitleSpr[0]` in `start01e.tex` or
/// `sh2e/ps2/etc/start01.tex`) locked pixel-for-pixel onto either `sh2e/pic/etc/start00.tex`
/// (`enhanced_style == true`) or `sh2e/ps2/etc/start00.tex` (`enhanced_style == false`).
pub fn draw_pulsing_title_logo_with_style(
    sm: &TitleStateMachine,
    atlas: &ButtonSpriteSheet,
    rast: &mut TextRasterizer,
    bg_uv_rect: (f32, f32, f32, f32),
    bg_vp: (f32, f32, f32, f32),
    ui_vp: (f32, f32, f32, f32),
    enhanced_style: bool,
) {
    const PERIOD_SECS: f32 = 7.5;
    let phase = (sm.elapsed_timer % PERIOD_SECS) / PERIOD_SECS;
    // Smooth cosine cycle: 0 -> 1 -> 0 over 7.5s, dropping to alpha=0.0 at trough so the
    // pure black letters in `start00.tex` show completely.
    let s = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
    let rgb = 1.0 - s * 0.90;
    let alpha = 1.0 - s;

    let (u0, v0, u1, v1) = bg_uv_rect;
    let du = (u1 - u0).max(1e-4);
    let dv = (v1 - v0).max(1e-4);

    let (sprite, norm_x, norm_w, norm_y, norm_h, src_w, src_h) = if enhanced_style {
        // Measured directly from `sh2e/pic/etc/start00.tex` (`2732×2048`, black logo `x=421..2310, y=606..1062`,
        // glyph height `456px`) vs `start01e.tex` (`2048×384`, white logo `x=82..1972, y=29..365`, glyph height `336px`).
        (
            AuthenticButtonSprite::TitleLogo,
            ((342.0 / 2732.0) - u0) / du,
            (2048.0 / 2732.0) / du,
            ((566.5 / 2048.0) - v0) / dv,
            (528.0 / 2048.0) / dv,
            2048.0f32,
            384.0f32,
        )
    } else {
        // Measured directly from `sh2e/ps2/etc/start00.tex` (`512×512`, dark logo `x=19..491, y=150..220`)
        // vs `sh2e/ps2/etc/start01.tex` (`512×96`, white logo `x=17..490, y=12..83`).
        let (ow, oh) = if atlas.orig_logo_rgba.is_some() {
            (
                atlas.orig_logo_width.min(512) as f32,
                atlas.orig_logo_height.min(96) as f32,
            )
        } else {
            (2048.0, 244.0)
        };
        (
            AuthenticButtonSprite::TitleLogoOriginal,
            ((2.0 / 512.0) - u0) / du,
            1.0 / du,
            ((138.0 / 512.0) - v0) / dv,
            (96.0 / 512.0) / dv,
            ow,
            oh,
        )
    };

    let (bg_x, bg_y, bg_w, bg_h) = bg_vp;
    let (ui_x, ui_y, ui_w, ui_h) = ui_vp;
    let ui_w = ui_w.max(1.0);
    let ui_h = ui_h.max(1.0);

    // Map exact screen-space logo rect inside `bg_vp` into `ui_vp` (`rast` pixel space).
    let logo_screen_x = bg_x + norm_x * bg_w;
    let logo_screen_y = bg_y + norm_y * bg_h;
    let logo_screen_w = norm_w * bg_w;
    let logo_screen_h = norm_h * bg_h;

    let rast_w = rast.width as f32;
    let rast_h = rast.height as f32;
    let lx = (((logo_screen_x - ui_x) / ui_w) * rast_w).round() as i32;
    let ly = (((logo_screen_y - ui_y) / ui_h) * rast_h).round() as i32;
    let scale_x = ((logo_screen_w / ui_w) * rast_w) / src_w;
    let scale_y = ((logo_screen_h / ui_h) * rast_h) / src_h;

    atlas.blit_sprite_xy(
        sprite,
        &mut rast.pixels,
        rast.width,
        rast.height,
        lx,
        ly,
        scale_x,
        scale_y,
        [rgb, rgb, rgb, alpha],
    );
}

/// Draws the authentic title screen text/sprite elements onto `rast` (`titleDrawMainMenu`
/// `0x001c38c0` / `sh2pc` `0x004971d0`).
pub fn draw_title_screen(
    sm: &TitleStateMachine,
    button_sprites: Option<&ButtonSpriteSheet>,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let vp = Viewport43::from_rasterizer(rast);
    let center_x = vp.center_x.round() as usize;

    match sm.phase {
        TitlePhase::SplashIntro => {
            if let Some(logo) = sm.boot_sequence.current_logo() {
                let y = vp.centered_to_px(0.0, 0.0).1.max(0) as usize;
                let font_sz = (18.0 * vp.scale_y).round().max(12.0) as usize;
                font.draw_line_centered(
                    rast,
                    logo.fallback_title(),
                    (center_x, y),
                    font_sz,
                    Sh2TextColor::White,
                    true,
                );
            }
        }
        TitlePhase::TitleLogo => {
            // Driven by continuous `sm.pulse_phase_rad` (`_DAT_0116d964`, not frozen `splash_timer`!).
            let pulse = sm.selection_pulse_alpha();
            if let Some(atlas) = button_sprites {
                let rect = AuthenticButtonSprite::StartPrompt.atlas_rect();
                let scale = vp.hd_sprite_scale() * 1.1;
                let w = (rect.width as f32 * scale) as i32;
                let (cx, py) = vp.centered_to_px(0.0, 152.0);
                atlas.blit_sprite(
                    AuthenticButtonSprite::StartPrompt,
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    cx - (w / 2),
                    py,
                    scale,
                    [1.0, 1.0, 1.0, pulse],
                );
            } else {
                let (_, py) = vp.centered_to_px(0.0, 156.0);
                let font_sz = (16.0 * vp.scale_y).round().max(12.0) as usize;
                font.draw_line_centered(
                    rast,
                    "PRESS ANY KEY OR ENTER TO START",
                    (center_x, py.max(0) as usize),
                    font_sz,
                    Sh2TextColor::GoldSelected,
                    true,
                );
            }
        }
        TitlePhase::MainMenu => {
            let slots = sm.visible_main_menu_options();
            let pulse_alpha = sm.selection_pulse_alpha();

            if let Some(atlas) = button_sprites {
                // `titleDrawMainMenu` (`0x001c38c0`): slot Y starts at +74, step +22 in 512×448 centered space.
                // `TitleSpr[0xb]` (`0x0033b880`) glow cursor height is `29`, and width is `asStack_42[idx + 0x19] + 8`
                // (`Load=104`, `Continue=164`, `NewGame=168`, `Options=150`, `Movies=124`, `Exit=128`).
                let btn_scale = vp.hd_sprite_scale();
                let start_cy = if slots.len() > 4 { 52.0 } else { 74.0 };
                let step_cy = 22.0;

                for (idx, opt) in slots.iter().enumerate() {
                    let is_selected = idx == sm.main_menu_index;
                    let cy = start_cy + idx as f32 * step_cy;
                    let (cx, py) = vp.centered_to_px(0.0, cy);

                    let (sprite, glow_virt_w) = match opt {
                        MainMenuOption::Load => (AuthenticButtonSprite::Load, 104.0_f32),
                        MainMenuOption::Continue => (AuthenticButtonSprite::Continue, 164.0_f32),
                        MainMenuOption::NewGame => (AuthenticButtonSprite::NewGame, 168.0_f32),
                        MainMenuOption::Options => (AuthenticButtonSprite::Options, 150.0_f32),
                        MainMenuOption::Movies => (AuthenticButtonSprite::Movies, 124.0_f32),
                        MainMenuOption::Exit => (AuthenticButtonSprite::Exit, 128.0_f32),
                    };

                    let rect = sprite.atlas_rect();
                    let scaled_w = (rect.width as f32 * btn_scale) as i32;
                    let btn_h = (rect.height as f32 * btn_scale) as i32;
                    let btn_x = cx - (scaled_w / 2);

                    if is_selected {
                        let glow_w = (glow_virt_w * vp.scale_x).round() as i32;
                        let glow_h = (29.0 * vp.scale_y).round() as i32;
                        ButtonSpriteSheet::draw_analytic_glow_cursor(
                            &mut rast.pixels,
                            rast.width,
                            rast.height,
                            cx,
                            py + (btn_h / 2),
                            glow_w,
                            glow_h,
                            [1.0, 1.0, 1.0, pulse_alpha],
                        );
                    }

                    let tint = if is_selected {
                        [1.0f32, 1.0, 1.0, pulse_alpha]
                    } else {
                        [0.72f32, 0.72, 0.75, 0.75]
                    };

                    atlas.blit_sprite(
                        sprite,
                        &mut rast.pixels,
                        rast.width,
                        rast.height,
                        btn_x,
                        py,
                        btn_scale,
                        tint,
                    );
                }
            } else {
                let start_cy = if slots.len() > 4 { 52.0 } else { 74.0 };
                let step_cy = 22.0;
                let font_sz = (17.0 * vp.scale_y).round().max(12.0) as usize;
                for (idx, opt) in slots.iter().enumerate() {
                    let is_selected = idx == sm.main_menu_index;
                    let (_, py) = vp.centered_to_px(0.0, start_cy + idx as f32 * step_cy);
                    let style = if is_selected {
                        Sh2TextColor::GoldSelected
                    } else {
                        Sh2TextColor::SilverUnselected
                    };
                    let full_label = if is_selected {
                        format!("> {}", opt.label())
                    } else {
                        format!("  {}", opt.label())
                    };
                    font.draw_line_centered(
                        rast,
                        &full_label,
                        (center_x, py.max(0) as usize),
                        font_sz,
                        style,
                        true,
                    );
                }
            }
        }
        TitlePhase::Credits => {
            let hdr_sz = (20.0 * vp.scale_y).round().max(14.0) as usize;
            let row_sz = (15.0 * vp.scale_y).round().max(11.0) as usize;
            let foot_sz = (12.0 * vp.scale_y).round().max(10.0) as usize;

            let (_, y_hdr) = vp.centered_to_px(0.0, -76.0);
            let (_, y_dir) = vp.centered_to_px(0.0, -20.0);
            let (_, y_cgi) = vp.centered_to_px(0.0, 12.0);
            let (_, y_mus) = vp.centered_to_px(0.0, 44.0);
            let (_, y_foot) = vp.centered_to_px(0.0, 164.0);

            font.draw_line_centered(
                rast,
                "SILENT HILL 2 CREDITS",
                (center_x, y_hdr.max(0) as usize),
                hdr_sz,
                Sh2TextColor::AmberHeader,
                true,
            );
            font.draw_line_centered(
                rast,
                "DIRECTOR: MASASHI TSUBOYAMA",
                (center_x, y_dir.max(0) as usize),
                row_sz,
                Sh2TextColor::White,
                true,
            );
            font.draw_line_centered(
                rast,
                "CGI & CHARACTER DESIGN: TAKAYOSHI SATO",
                (center_x, y_cgi.max(0) as usize),
                row_sz,
                Sh2TextColor::White,
                true,
            );
            font.draw_line_centered(
                rast,
                "MUSIC & SOUND DESIGN: AKIRA YAMAOKA",
                (center_x, y_mus.max(0) as usize),
                row_sz,
                Sh2TextColor::White,
                true,
            );
            font.draw_line_centered(
                rast,
                "[Press Esc / Cancel to Return]",
                (center_x, y_foot.max(0) as usize),
                foot_sz,
                Sh2TextColor::MutedFooter,
                true,
            );
        }
        TitlePhase::ExitConfirm => {
            // Same retail `sh2pc.exe` `FUN_00407610` (`0x00407610`) 512×448 confirmation geometry:
            // prompt at `(256, 232)` (`0, +8` centered), Yes at `(226, 266)` (`-30, +42`), No at `(286, 266)` (`+30, +42`).
            let hdr_sz = (22.0 * vp.scale_y).round().max(14.0) as usize;
            let item_sz = (18.0 * vp.scale_y).round().max(12.0) as usize;
            let pop_dx = (2.0 * vp.scale_x).round().max(1.0) as usize;
            let pop_dy = (2.0 * vp.scale_y).round().max(1.0) as usize;

            let (px, py) = vp.centered_to_px(0.0, 8.0);
            font.draw_line_centered(
                rast,
                "Exit the game?",
                (px.max(0) as usize, py.max(0) as usize),
                hdr_sz,
                Sh2TextColor::White,
                true,
            );

            let (yx, yy) = vp.centered_to_px(-30.0, 42.0);
            let (nx, ny) = vp.centered_to_px(30.0, 42.0);
            let yes_pos = (yx.max(0) as usize, yy.max(0) as usize);
            let no_pos = (nx.max(0) as usize, ny.max(0) as usize);

            let unselected = Sh2TextColor::Custom {
                top: [0x7f, 0x7f, 0x7f, 0x60],
                bottom: [0x7f, 0x7f, 0x7f, 0x60],
            };
            font.draw_line_centered(rast, "Yes", yes_pos, item_sz, unselected, false);
            font.draw_line_centered(rast, "No", no_pos, item_sz, unselected, false);

            let (sel_label, sel_pos) = if sm.exit_confirm_index == 0 {
                ("Yes", yes_pos)
            } else {
                ("No", no_pos)
            };
            font.draw_line_centered(
                rast,
                sel_label,
                (
                    sel_pos.0.saturating_sub(pop_dx),
                    sel_pos.1.saturating_sub(pop_dy),
                ),
                item_sz,
                Sh2TextColor::White,
                false,
            );
        }
        _ => {}
    }
}

/// Draws the authentic scenario selection screen onto `rast` (`sh2pc.exe` `titleDrawScenarioMenu`
/// `0x00497960`).
pub fn draw_scenario_selection(
    sm: &TitleStateMachine,
    button_sprites: Option<&ButtonSpriteSheet>,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let vp = Viewport43::from_rasterizer(rast);
    let center_x = vp.center_x.round() as usize;
    let pulse_alpha = sm.selection_pulse_alpha();

    if let Some(atlas) = button_sprites {
        let scale = vp.hd_sprite_scale();

        // `sh2pc.exe` `FUN_00497960` (`0x00497960`): draws Main Scenario (`Y = +76` / `0xffffff4c`? centered)
        // and Sub Scenario (`Y = +108`) with glow widths `0x1fc` (508) / `0x198` (408) and height `22`.
        let scenarios = [
            (0, AuthenticButtonSprite::MainScenario, 508.0_f32),
            (1, AuthenticButtonSprite::SubScenario, 408.0_f32),
        ];
        for (idx, sprite, glow_virt_w) in &scenarios {
            let is_selected = *idx == sm.scenario_index;
            let (cx, py) = vp.centered_to_px(0.0, 76.0 + *idx as f32 * 32.0);
            let rect = sprite.atlas_rect();
            let w = (rect.width as f32 * scale) as i32;
            let btn_h = (rect.height as f32 * scale) as i32;

            if is_selected {
                let glow_w = (glow_virt_w * vp.scale_x).round() as i32;
                let glow_h = (22.0 * vp.scale_y).round() as i32;
                ButtonSpriteSheet::draw_analytic_glow_cursor(
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    cx,
                    py + (btn_h / 2),
                    glow_w,
                    glow_h,
                    [1.0, 1.0, 1.0, pulse_alpha],
                );
            }

            let tint = if is_selected {
                [1.0f32, 1.0, 1.0, pulse_alpha]
            } else {
                [0.72f32, 0.72, 0.75, 0.75]
            };
            atlas.blit_sprite(
                *sprite,
                &mut rast.pixels,
                rast.width,
                rast.height,
                cx - (w / 2),
                py,
                scale,
                tint,
            );
        }
    } else {
        let hdr_sz = (18.0 * vp.scale_y).round().max(12.0) as usize;
        let row_sz = (15.0 * vp.scale_y).round().max(11.0) as usize;
        let (_, y_hdr) = vp.centered_to_px(0.0, 48.0);
        font.draw_line_centered(
            rast,
            "SELECT SCENARIO",
            (center_x, y_hdr.max(0) as usize),
            hdr_sz,
            Sh2TextColor::AmberHeader,
            true,
        );
        let scenarios = [
            "LETTER FROM SILENT HEAVEN (JAMES)",
            "BORN FROM A WISH (MARIA)",
        ];
        for (idx, name) in scenarios.iter().enumerate() {
            let is_selected = idx == sm.scenario_index;
            let (_, py) = vp.centered_to_px(0.0, 92.0 + idx as f32 * 30.0);
            let style = if is_selected {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::SilverUnselected
            };
            let line = if is_selected {
                format!("> {}", name)
            } else {
                format!("  {}", name)
            };
            font.draw_line_centered(
                rast,
                &line,
                (center_x, py.max(0) as usize),
                row_sz,
                style,
                true,
            );
        }

        let (_, y_foot) = vp.centered_to_px(0.0, 190.0);
        let foot_sz = (11.0 * vp.scale_y).round().max(9.0) as usize;
        font.draw_line_centered(
            rast,
            "[Up/Down: Select | Enter: Confirm | Esc: Back]",
            (center_x, y_foot.max(0) as usize),
            foot_sz,
            Sh2TextColor::MutedFooter,
            true,
        );
    }
}

/// Draws the authentic side-by-side Action & Riddle difficulty selection screen
/// (`SLUS_202.28` `titleDrawSubMenu` `0x001c3e10` / `sh2pc.exe` `0x004974d0`).
///
/// Exact 512×448 centered coordinates:
/// - Action column center: `X = -100` (`header left -196 + 96`), or `0` (`-96 + 96`) in Born From a Wish.
/// - Riddle column center: `X = +107` (`header left +11 + 96`), hidden in Born From a Wish.
/// - Header Y: `+60` (`0x3c`); rows: Hard `+80` (`0x50`), Normal `+98` (`0x62`), Easy `+115` (`0x73`),
///   Beginner `+134` (`0x86`, Action column only).
pub fn draw_difficulty_selection(
    sm: &TitleStateMachine,
    button_sprites: Option<&ButtonSpriteSheet>,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let vp = Viewport43::from_rasterizer(rast);
    let is_action = sm.phase == TitlePhase::ActionDifficultySelect;
    let is_bfaw = sm.selected_scenario == ScenarioKind::BornFromAWish;
    let pulse_alpha = sm.selection_pulse_alpha();

    // Exact centered X coordinates from `titleDrawSubMenu` (`0x001c3e10` / `sh2pc` `0x004974d0`).
    let action_cx = if is_bfaw { 0.0 } else { -100.0 };
    let riddle_cx = 107.0;
    const HEADER_CY: f32 = 60.0;
    const ROW_CYS: [f32; 4] = [80.0, 98.0, 115.0, 134.0];

    let action_sprites = [
        AuthenticButtonSprite::Hard,
        AuthenticButtonSprite::Normal,
        AuthenticButtonSprite::Easy,
        AuthenticButtonSprite::Beginner,
    ];
    let riddle_sprites = [
        AuthenticButtonSprite::Hard,
        AuthenticButtonSprite::Normal,
        AuthenticButtonSprite::Easy,
    ];

    if let Some(atlas) = button_sprites {
        let scale = vp.hd_sprite_scale();

        // Action Level header + 4 rows (Hard, Normal, Easy, Beginner)
        let hdr_rect = AuthenticButtonSprite::ActionLevel.atlas_rect();
        let hdr_w = (hdr_rect.width as f32 * scale) as i32;
        let (ax, hy) = vp.centered_to_px(action_cx, HEADER_CY);
        atlas.blit_sprite(
            AuthenticButtonSprite::ActionLevel,
            &mut rast.pixels,
            rast.width,
            rast.height,
            ax - hdr_w / 2,
            hy,
            scale,
            if is_action {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.65, 0.65, 0.68, 0.75]
            },
        );

        for (idx, (&diff, &sprite)) in ACTION_DIFFICULTY_ROWS
            .iter()
            .zip(action_sprites.iter())
            .enumerate()
        {
            let is_selected = diff == sm.action_difficulty;
            // `titleDrawSubMenu` (`0x001c3e10`, `param_1 == 1` lines 1324-1332): once Action Level is
            // confirmed and Riddle Level is being chosen, hide unselected Action rows so only the
            // confirmed Action selection remains visible with its locked glow.
            if !is_action && !is_selected {
                continue;
            }
            let (px, py) = vp.centered_to_px(action_cx, ROW_CYS[idx]);
            let rect = sprite.atlas_rect();
            let w = (rect.width as f32 * scale) as i32;
            let h = (rect.height as f32 * scale) as i32;

            if is_selected {
                let glow_w = (120.0 * vp.scale_x).round() as i32;
                // `titleDrawSubMenu` (`0x001c3e10` line 1299 / 1337): `TitleSpr._362_2_ -= 7` (`29 - 7 = 22`).
                let glow_h = (22.0 * vp.scale_y).round() as i32;
                let glow_alpha = if is_action { pulse_alpha } else { 0.85 };
                ButtonSpriteSheet::draw_analytic_glow_cursor(
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    px,
                    py + h / 2,
                    glow_w,
                    glow_h,
                    [1.0, 1.0, 1.0, glow_alpha],
                );
            }

            let tint = if is_selected && is_action {
                [1.0, 0.95, 0.75, pulse_alpha]
            } else if is_selected {
                [0.9, 0.9, 0.92, 0.9]
            } else {
                [0.65, 0.65, 0.68, 0.7]
            };
            atlas.blit_sprite(
                sprite,
                &mut rast.pixels,
                rast.width,
                rast.height,
                px - w / 2,
                py,
                scale,
                tint,
            );
        }

        // Riddle Level column (hidden in Born From a Wish per `sh2pc` `0x004974d0`)
        if !is_bfaw {
            let hdr_rect = AuthenticButtonSprite::RiddleLevel.atlas_rect();
            let hdr_w = (hdr_rect.width as f32 * scale) as i32;
            let (rx, hy) = vp.centered_to_px(riddle_cx, HEADER_CY);
            atlas.blit_sprite(
                AuthenticButtonSprite::RiddleLevel,
                &mut rast.pixels,
                rast.width,
                rast.height,
                rx - hdr_w / 2,
                hy,
                scale,
                if !is_action {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.65, 0.65, 0.68, 0.75]
                },
            );

            for (idx, (&diff, &sprite)) in RIDDLE_DIFFICULTY_ROWS
                .iter()
                .zip(riddle_sprites.iter())
                .enumerate()
            {
                let is_selected = diff == sm.riddle_difficulty;
                let (px, py) = vp.centered_to_px(riddle_cx, ROW_CYS[idx]);
                let rect = sprite.atlas_rect();
                let w = (rect.width as f32 * scale) as i32;
                let h = (rect.height as f32 * scale) as i32;

                if is_selected && !is_action {
                    let glow_w = (120.0 * vp.scale_x).round() as i32;
                    let glow_h = (22.0 * vp.scale_y).round() as i32;
                    ButtonSpriteSheet::draw_analytic_glow_cursor(
                        &mut rast.pixels,
                        rast.width,
                        rast.height,
                        px,
                        py + h / 2,
                        glow_w,
                        glow_h,
                        [1.0, 1.0, 1.0, pulse_alpha],
                    );
                }

                let tint = if is_selected && !is_action {
                    [1.0, 0.95, 0.75, pulse_alpha]
                } else if is_selected {
                    [0.9, 0.9, 0.92, 0.9]
                } else {
                    [0.65, 0.65, 0.68, 0.7]
                };
                atlas.blit_sprite(
                    sprite,
                    &mut rast.pixels,
                    rast.width,
                    rast.height,
                    px - w / 2,
                    py,
                    scale,
                    tint,
                );
            }
        }
    } else {
        let hdr_sz = (15.0 * vp.scale_y).round().max(11.0) as usize;
        let row_sz = (14.0 * vp.scale_y).round().max(10.0) as usize;

        let (ax, hy) = vp.centered_to_px(action_cx, HEADER_CY);
        font.draw_line_centered(
            rast,
            "ACTION LEVEL",
            (ax.max(0) as usize, hy.max(0) as usize),
            hdr_sz,
            if is_action {
                Sh2TextColor::AmberHeader
            } else {
                Sh2TextColor::SilverUnselected
            },
            true,
        );
        for (idx, &diff) in ACTION_DIFFICULTY_ROWS.iter().enumerate() {
            let is_sel = diff == sm.action_difficulty;
            let (px, py) = vp.centered_to_px(action_cx, ROW_CYS[idx]);
            let label = if is_sel && is_action {
                format!("> {} <", diff.label().to_uppercase())
            } else {
                diff.label().to_uppercase()
            };
            let color = if is_sel && is_action {
                Sh2TextColor::GoldSelected
            } else if is_sel {
                Sh2TextColor::White
            } else {
                Sh2TextColor::SilverUnselected
            };
            font.draw_line_centered(
                rast,
                &label,
                (px.max(0) as usize, py.max(0) as usize),
                row_sz,
                color,
                true,
            );
        }

        if !is_bfaw {
            let (rx, hy) = vp.centered_to_px(riddle_cx, HEADER_CY);
            font.draw_line_centered(
                rast,
                "RIDDLE LEVEL",
                (rx.max(0) as usize, hy.max(0) as usize),
                hdr_sz,
                if !is_action {
                    Sh2TextColor::AmberHeader
                } else {
                    Sh2TextColor::SilverUnselected
                },
                true,
            );
            for (idx, &diff) in RIDDLE_DIFFICULTY_ROWS.iter().enumerate() {
                let is_sel = diff == sm.riddle_difficulty;
                let (px, py) = vp.centered_to_px(riddle_cx, ROW_CYS[idx]);
                let label = if is_sel && !is_action {
                    format!("> {} <", diff.label().to_uppercase())
                } else {
                    diff.label().to_uppercase()
                };
                let color = if is_sel && !is_action {
                    Sh2TextColor::GoldSelected
                } else if is_sel {
                    Sh2TextColor::White
                } else {
                    Sh2TextColor::SilverUnselected
                };
                font.draw_line_centered(
                    rast,
                    &label,
                    (px.max(0) as usize, py.max(0) as usize),
                    row_sz,
                    color,
                    true,
                );
            }
        }

        let (_, y_foot) = vp.centered_to_px(0.0, 190.0);
        let foot_sz = (11.0 * vp.scale_y).round().max(9.0) as usize;
        let footer_text = if is_action && !is_bfaw {
            "[Up/Down: Change | Enter: Next | Esc: Back]"
        } else {
            "[Up/Down: Change | Enter: Start Game | Esc: Back]"
        };
        font.draw_line_centered(
            rast,
            footer_text,
            (vp.center_x.round() as usize, y_foot.max(0) as usize),
            foot_sz,
            Sh2TextColor::MutedFooter,
            true,
        );
    }
}

/// Draws the test room selection stage list onto `rast` styled identically to `look_board` (`0x002171a0`).
pub fn draw_test_rooms_screen(sm: &TitleStateMachine, font: &AtlasFont, rast: &mut TextRasterizer) {
    let vp = Viewport43::from_rasterizer(rast);
    let scale_x = vp.base_w / 640.0;
    let scale_y = rast.height as f32 / 480.0;

    let map_pts = |pts: &[(f32, f32)]| -> Vec<(i32, i32)> {
        pts.iter()
            .map(|(x, y)| {
                (
                    (vp.x_offset + x * scale_x).round() as i32,
                    (y * scale_y).round() as i32,
                )
            })
            .collect()
    };

    let panel_pts = map_pts(&[
        (48.0, 64.0),
        (593.0, 64.0),
        (604.0, 76.0),
        (604.0, 407.0),
        (36.0, 407.0),
        (36.0, 76.0),
    ]);
    let box_pts = map_pts(&[
        (36.0, 414.0),
        (604.0, 414.0),
        (604.0, 436.0),
        (580.0, 461.0),
        (60.0, 461.0),
        (36.0, 436.0),
    ]);
    let outline_pts = map_pts(&[
        (55.0, 58.0),
        (586.0, 58.0),
        (612.0, 83.0),
        (612.0, 440.0),
        (586.0, 467.0),
        (55.0, 467.0),
        (30.0, 440.0),
        (30.0, 83.0),
    ]);

    // `look_board` (`0x002171a0`): fill `0x20808080`, border `0x30d06060` (ABGR -> RGBA).
    let fill_color = [0x80, 0x80, 0x80, 0x40];
    let border_color = [0x60, 0x60, 0xd0, 0x60];

    crate::options::draw_polygon(rast, &panel_pts, Some(fill_color), Some(border_color));
    crate::options::draw_polygon(rast, &box_pts, Some(fill_color), Some(border_color));
    crate::options::draw_polygon(rast, &outline_pts, None, Some(border_color));

    let title_x = rast.width / 2;
    let title_y = (28.0 * scale_y).round() as usize;

    font.draw_line_centered(
        rast,
        "<   T E S T   R O O M S   >",
        (title_x, title_y),
        (22.0 * scale_y).round() as usize,
        Sh2TextColor::White,
        true,
    );

    let count = TestRoomKind::ALL.len();
    let start_y = 80.0;
    let end_y = 384.0;
    let step_y = if count > 1 {
        ((end_y - start_y) / (count - 1) as f32).min(38.0) * scale_y
    } else {
        38.0 * scale_y
    };
    let base_y = start_y * scale_y;
    let center_panel_x = (vp.x_offset + 320.0 * scale_x).round() as usize;
    let font_size = (14.0 * scale_y).round().max(10.0) as usize;

    for (idx, room) in TestRoomKind::ALL.iter().enumerate() {
        let y = (base_y + idx as f32 * step_y).round() as usize;
        let is_selected = idx == sm.test_room_index;
        let style = if is_selected {
            Sh2TextColor::White
        } else {
            Sh2TextColor::SilverUnselected
        };
        let label = room.label();
        if is_selected {
            font.draw_line_centered(
                rast,
                label,
                (center_panel_x.saturating_sub(2), y.saturating_sub(2)),
                font_size,
                Sh2TextColor::White,
                false,
            );
        }
        font.draw_line_centered(rast, label, (center_panel_x, y), font_size, style, true);
    }

    let help_x = (vp.x_offset + 90.0 * scale_x).round() as usize;
    let help_y = (436.0 * scale_y).round() as usize;
    let selected_room = TestRoomKind::ALL
        .get(sm.test_room_index)
        .copied()
        .unwrap_or_default();
    font.draw_line_styled(
        rast,
        selected_room.description(),
        (help_x, help_y),
        font_size,
        Sh2TextColor::White,
        true,
    );
}

/// Draws the retail PC (`sh2pc.exe` / SH2:EE) white pointer arrow cursor with a crisp black outline
/// onto `rast` at normalized coordinates `(norm_x, norm_y)` (`[0.0..1.0]`).
pub fn draw_pc_mouse_cursor(rast: &mut TextRasterizer, norm_x: f32, norm_y: f32) {
    if !(0.0..=1.0).contains(&norm_x) || !(0.0..=1.0).contains(&norm_y) {
        return;
    }
    let tip_x = (norm_x * rast.width as f32).round() as i32;
    let tip_y = (norm_y * rast.height as f32).round() as i32;
    let scale = ((rast.height as f32 / 448.0).round() as i32).max(1);

    // Classic 18×24 angled pointer arrow template (`1` = black outline, `2` = white fill).
    const CURSOR_ROWS: [&str; 19] = [
        "1000000000000",
        "1100000000000",
        "1210000000000",
        "1221000000000",
        "1222100000000",
        "1222210000000",
        "1222221000000",
        "1222222100000",
        "1222222210000",
        "1222222221000",
        "1222222222100",
        "1222222111110",
        "1222122100000",
        "1221122100000",
        "1210012210000",
        "1100012210000",
        "1000001221000",
        "0000001221000",
        "0000000110000",
    ];

    for (ry, row) in CURSOR_ROWS.iter().enumerate() {
        for (rx, ch) in row.bytes().enumerate() {
            let color = match ch {
                b'1' => [0x00, 0x00, 0x00, 0xff],
                b'2' => [0xf4, 0xf4, 0xf4, 0xff],
                _ => continue,
            };
            let px0 = tip_x + (rx as i32) * scale;
            let py0 = tip_y + (ry as i32) * scale;
            for sy in 0..scale {
                let py = py0 + sy;
                if py < 0 || py >= rast.height as i32 {
                    continue;
                }
                for sx in 0..scale {
                    let px = px0 + sx;
                    if px >= 0 && px < rast.width as i32 {
                        let idx = (py as usize * rast.width + px as usize) * 4;
                        rast.pixels[idx..idx + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
}
