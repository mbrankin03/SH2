//! 1:1 PS2 `Item/otn_itemmain.c` Inventory Visual Primitives (`SLUS_202.28` & `sh2pc.exe`).
//!
//! Every coordinate, color, taper, and animation rate in this module is extracted directly from
//! `SLUS_202.28` `Item/otn_itemmain.c` (`docs/decomp/slus/Item/otn_itemmain.c`) and cross-checked
//! against retail PC `sh2pc.exe` (`docs/decomp/sh2pc/sh2pc_00460000.c` & `sh2pc_00470000.c`).
//!
//! # Coordinate System
//! PS2 `otn_itemmain.c` emits GS primitives in centered `512x448` coordinates (`x in -256..+256`,
//! `y in -224..+224`). Converting to top-left `512x448` logical pixels adds `(+256.0, +224.0)`.

use crate::font::TextRasterizer;
use crate::inventory::atlas::item_icon_rect;
use crate::inventory::health_ecg::STATUS_SNAPSHOT_W_512;

/// Native PS2 2D UI framebuffer width (`SLUS_202.28` `font_print` `0x001d4c00`).
pub const PS2_UI_WIDTH: f32 = 512.0;
/// Native PS2 2D UI framebuffer height (`SLUS_202.28` `font_print` `0x001d4c00`).
pub const PS2_UI_HEIGHT: f32 = 448.0;
/// Half of `PS2_UI_WIDTH` (`256.0`) for centered-to-top-left PS2 conversion.
pub const PS2_CENTER_X: f32 = 256.0;
/// Half of `PS2_UI_HEIGHT` (`224.0`) for centered-to-top-left PS2 conversion.
pub const PS2_CENTER_Y: f32 = 224.0;

/// Authentic PS2 `lookline` electric-blue RGB (`0x00f06030` at `SLUS_202.28` `0x001d6d2c`).
pub const SH2_BLUE_LINE_RGB: [u8; 3] = [48, 96, 240];

/// Base cyan-blue intensity `scolor` in `SLUS_202.28` (`0x003400d0` = `60` = `0x3c`),
/// used by `sprite` (`0x001d8890`).
pub const SPRITE_SCOLOR: i32 = 60;
/// Number of vertical glow layers in `sprite` (`0x001d8890`) and `look_command` (`0x001d8f50`).
pub const GLOW_BAR_LAYERS: usize = 6;
/// Column bar X origins in `512x448` space (`sprite` `0x001d8c80..0x001d8c84`: `-230 + 256 + col * 160`).
pub const SPRITE_BAR_X0: [f32; 3] = [26.0, 186.0, 346.0];
/// Column bar width in `512x448` space (`sprite` `0x001d8c84..0x001d8d58`: `-90 - (-230) = 140`).
pub const SPRITE_BAR_WIDTH: f32 = STATUS_SNAPSHOT_W_512;
/// Top row bar center Y in `512x448` space (`sprite` `0x001d8ce8..0x001d8cf4`: `224 - 189 = 35`).
pub const SPRITE_TOP_BAR_CY: f32 = 35.0;
/// Bottom row bar center Y in `512x448` space (`sprite` `0x001d8ce8..0x001d8cf4`: `224 + 189 = 413`).
pub const SPRITE_BOT_BAR_CY: f32 = 413.0;

/// Maximum fade intensity `_DAT_0116e3d8` for `look_command` (`itemmainmain` `0x001ce508`, `0x28 = 40`).
pub const LOOK_COMMAND_MAX_GLOW: f32 = 40.0;
/// Per-30Hz-frame fade step for `look_command` (`itemmainmain` `0x001ce4ec`, `±6`).
pub const LOOK_COMMAND_GLOW_STEP: f32 = 6.0;
/// Command highlight row vertical pitch in `512x448` space (`look_command` `0x001d91a0`, `26.0`).
pub const LOOK_COMMAND_ROW_PITCH: f32 = 26.0;
/// Command highlight left X in `512x448` space (`look_command` `0x001d9188`: `90.0 + 256.0 = 346.0`).
pub const LOOK_COMMAND_X0: f32 = 346.0;
/// Command highlight right X in `512x448` space (`look_command` `0x001d9254`: `230.0 + 256.0 = 486.0`).
pub const LOOK_COMMAND_X1: f32 = 486.0;

/// Maximum fade `_DAT_0116e618` for `item_allay` arrows (`itemmainmain` `0x001ce578`, `32.0`).
pub const ITEM_ALLAY_MAX_FADE: f32 = 32.0;
/// Per-30Hz-frame fade step for `item_allay` (`itemmainmain` `0x001ce558`, `±4.0`).
pub const ITEM_ALLAY_FADE_STEP: f32 = 4.0;
/// Brightness triangle-wave wrap threshold (`item_allay` `0x001d7648`, `0x15e = 350`).
pub const ITEM_ALLAY_PULSE_MAX: f32 = 350.0;
/// Brightness triangle-wave reset floor (`item_allay` `0x001d7658`, `0x32 = 50`).
pub const ITEM_ALLAY_PULSE_MIN: f32 = 50.0;
/// Brightness triangle-wave peak (`item_allay` `0x001d7668`, `199`).
pub const ITEM_ALLAY_PULSE_PEAK: f32 = 199.0;
/// Brightness triangle-wave advance rate (`item_allay` `0x001d7688`: `6.0 * 30.0 = 180.0 / s`).
pub const ITEM_ALLAY_PULSE_RATE: f32 = 180.0;
/// Outline RGB for `item_allay` chevrons (`item_allay` `0x001d7a98`, `0x808080`).
pub const ITEM_ALLAY_OUTLINE_RGB: [u8; 3] = [0x80, 0x80, 0x80];

/// `stock_line` (`0x001da170`) bar color `0xffa0a0a0` (`RGBA(160, 160, 160, 255)`).
pub const STOCK_LINE_RGBA: [u8; 4] = [0xa0, 0xa0, 0xa0, 0xff];
/// `stock_line` width parameter for 1-digit numbers (`font_print` `0x001d6754`, `0x1f = 31`).
pub const STOCK_LINE_WIDTH_1DIGIT: f32 = 31.0;
/// `stock_line` width parameter for 2-digit numbers (`font_print` `0x001d6784`, `0x2d = 45`).
pub const STOCK_LINE_WIDTH_2DIGIT: f32 = 45.0;
/// `stock_line` width parameter for 3-digit numbers (`font_print` `0x001d67a8`, `0x3b = 59`).
pub const STOCK_LINE_WIDTH_3DIGIT: f32 = 59.0;
/// `stock_line` width parameter for Flashlight/Radio `"ON"`/`"OFF"` badge (`font_print` `0x001d681c`, `0x32 = 50`).
pub const STOCK_LINE_WIDTH_ON_OFF: f32 = 50.0;

/// `look_combine` (`0x001da490`) slot fill RGB on PS2 (`0x103010` = `RGB(16, 48, 16)`).
/// Note: retail PC `sh2pc.exe` (`0x004700c0`) brightens this slightly to `0x204020` (`RGB(32, 64, 32)`).
pub const LOOK_COMBINE_FILL_RGB_PS2: [u8; 3] = [0x10, 0x30, 0x10];
/// `look_combine` (`0x001da490` / `sh2pc 0x004700c0`) slot border RGB (`0x408040` = `RGB(64, 128, 64)`).
pub const LOOK_COMBINE_BORDER_RGB: [u8; 3] = [0x40, 0x80, 0x40];
/// `look_combine` (`0x001da490`) icon scale (`0x3ecccccd = 0.4`).
pub const LOOK_COMBINE_ICON_SCALE: f32 = 0.4;
/// `look_combine` (`0x001da490`) fade rate (`4.0 / s`, `_DAT_0116e64c`).
pub const LOOK_COMBINE_FADE_RATE: f32 = 4.0;

/// `look_volume` (`0x001d9480`) total number of Radio volume bars (`0x10 = 16`, levels `0..=15`).
pub const RADIO_VOLUME_BARS: usize = 16;
/// `look_volume` (`0x001d9480`) active bar RGB (`0x2000b000` -> `RGB(0, 176, 0)`, PS2 `PRIM = 6`).
pub const RADIO_VOLUME_ACTIVE_RGB: [u8; 3] = [0x00, 0xb0, 0x00];
/// `look_volume` (`0x001d9480`) inactive bar RGB (`0x20003000` -> `RGB(0, 48, 0)`, PS2 `PRIM = 6`).
pub const RADIO_VOLUME_INACTIVE_RGB: [u8; 3] = [0x00, 0x30, 0x00];

/// Full-texture `512x448` PS2 UI coordinate transform.
///
/// The rasterizer texture itself represents the 4:3 PS2 frame and is blitted into
/// `ps2_ui_canvas_rect(surface_w, surface_h)` by `sh2-app`, guaranteeing 1:1 alignment with
/// `HealthEcgState::snapshot_viewport_rect` across every monitor aspect ratio.
#[derive(Clone, Copy)]
pub struct Ps2Canvas {
    pub tex_w: f32,
    pub tex_h: f32,
    pub sx: f32,
    pub sy: f32,
}

impl Ps2Canvas {
    pub fn from_viewport(w: usize, h: usize) -> Self {
        let tex_w = w.max(1) as f32;
        let tex_h = h.max(1) as f32;
        Self {
            tex_w,
            tex_h,
            sx: tex_w / PS2_UI_WIDTH,
            sy: tex_h / PS2_UI_HEIGHT,
        }
    }

    pub fn x(&self, px: f32) -> f32 {
        px * self.sx
    }

    pub fn y(&self, py: f32) -> f32 {
        py * self.sy
    }

    pub fn xi(&self, px: f32) -> usize {
        self.x(px).round().max(0.0) as usize
    }

    pub fn yi(&self, py: f32) -> usize {
        self.y(py).round().max(0.0) as usize
    }

    /// Converts a centered PS2 coordinate `(cx, cy)` (`-256..+256`, `-224..+224`) to texture pixels.
    pub fn centered(&self, cx: f32, cy: f32) -> (f32, f32) {
        (self.x(cx + PS2_CENTER_X), self.y(cy + PS2_CENTER_Y))
    }

    /// Converts a desired physical-square height (in texture pixels) and `aspect` (`w / h`) into
    /// texture-space width so that when this texture is blitted into a 4:3 viewport, the rendered
    /// element has exact `aspect` on screen regardless of `(tex_w, tex_h)`.
    pub fn width_for_height(&self, height_px: f32, aspect: f32) -> f32 {
        (height_px / self.tex_h) * (aspect * 0.75) * self.tex_w
    }
}

/// Exact 1:1 port of `SLUS_202.28` `move_near` (`0x001d34e0`, `Item/otn_itemmain.c`).
///
/// Advances `current` toward `target` using proportional easing `((target - current) / div)`
/// plus a constant minimum floor `speed`, scaled to 30 Hz PS2 ticks (`dt * 30.0`), and clamps
/// to `target` on overshoot.
pub fn move_near(div: f32, speed: f32, current: f32, target: f32, dt: f32) -> f32 {
    let ticks = dt * 30.0;
    if current < target {
        let next = current + ((target - current) / div) * ticks + speed * ticks;
        if next >= target {
            target
        } else {
            next
        }
    } else if current > target {
        let next = current - ((current - target) / div) * ticks - speed * ticks;
        if next <= target {
            target
        } else {
            next
        }
    } else {
        target
    }
}

/// Computes the `item_allay` (`0x001d7640`) brightness triangle-wave (`50..=199`) from `phase`.
pub fn item_allay_brightness(phase: f32) -> u8 {
    let v = if phase > ITEM_ALLAY_PULSE_PEAK {
        400.0 - phase
    } else {
        phase
    };
    v.round().clamp(0.0, 255.0) as u8
}

/// Helper to draw a line segment between two floating-point texture coordinates.
#[inline]
fn draw_segment(
    rasterizer: &mut TextRasterizer,
    a: (f32, f32),
    b: (f32, f32),
    thickness: usize,
    color: [u8; 4],
) {
    rasterizer.draw_line(
        a.0.round() as i32,
        a.1.round() as i32,
        b.0.round() as i32,
        b.1.round() as i32,
        thickness,
        color,
    );
}

/// Renders the `SLUS_202.28` `item_allay` (`0x001d7640`) pulsing double-chevron L/R arrows
/// flanking the center carousel reticle.
///
/// Geometry in centered `512x448` PS2 space:
/// - Right pair: `(78, ±11)-(95, 0)` and `(95, ±11)-(112, 0)` (`0x4e, 0x5f, 0x70`, `±0xb`).
/// - Left pair: `(-78, ±11)-(-95, 0)` and `(-95, ±11)-(-112, 0)` (`-0x4e, -0x5f, -0x70`, `±0xb`).
/// - Fill: grayscale triangle-wave `50..199` at `180/s`, GS alpha `fade` (`0..32` -> `0..64` in 8-bit).
/// - Outline: `0x808080` (`RGB(128, 128, 128)`), GS alpha `fade * 5` (`0..160` -> `0..255` in 8-bit).
pub fn draw_item_allay(rasterizer: &mut TextRasterizer, c: Ps2Canvas, pulse_phase: f32, fade: f32) {
    if (fade as i32) <= 0 {
        return;
    }
    let lum = item_allay_brightness(pulse_phase);
    // PS2 GS alpha `0..0x80` maps linearly to 8-bit `0..255` (`* 2`).
    let fill_a = (fade * 2.0).round().clamp(0.0, 255.0) as u8;
    let outline_a = (fade * 5.0 * 2.0).round().clamp(0.0, 255.0) as u8;
    let fill_rgba = [lum, lum, lum, fill_a];
    let outline_rgba = [
        ITEM_ALLAY_OUTLINE_RGB[0],
        ITEM_ALLAY_OUTLINE_RGB[1],
        ITEM_ALLAY_OUTLINE_RGB[2],
        outline_a,
    ];

    let chevrons: [[(f32, f32); 3]; 4] = [
        // Right inner & outer (`0x001d7720..0x001d7850`)
        [(78.0, -11.0), (78.0, 11.0), (95.0, 0.0)],
        [(95.0, -11.0), (95.0, 11.0), (112.0, 0.0)],
        // Left inner & outer (`0x001d7900..0x001d7a10`)
        [(-78.0, -11.0), (-78.0, 11.0), (-95.0, 0.0)],
        [(-95.0, -11.0), (-95.0, 11.0), (-112.0, 0.0)],
    ];

    let thickness = (c.sy * 0.75).round().max(1.0) as usize;
    for tri in chevrons {
        let p0 = c.centered(tri[0].0, tri[0].1);
        let p1 = c.centered(tri[1].0, tri[1].1);
        let p2 = c.centered(tri[2].0, tri[2].1);
        rasterizer.fill_triangle([p0, p1, p2], fill_rgba);
        draw_segment(rasterizer, p0, p1, thickness, outline_rgba);
        draw_segment(rasterizer, p1, p2, thickness, outline_rgba);
        draw_segment(rasterizer, p2, p0, thickness, outline_rgba);
    }
}

/// Renders the `SLUS_202.28` `lookline` (`0x001d6c30`) electric-blue selection reticle:
/// - **One** closed 4-vertex rectangle in `0x00f06030` (`RGB(48, 96, 240)`).
/// - When `show_corner_brackets` is true (active `Carousel`, `alpha == 0xff`, not mid-transition /
///   combine / examine), adds **two outer L-bracket corner accents** at `(-6, -6)` (TL) and
///   `(+6, +5)` (BR) spanning half-width and half-height (`0x001d6fb0..0x001d7630`).
#[allow(clippy::too_many_arguments)]
pub fn draw_lookline(
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
    rx: usize,
    ry: usize,
    ret_w: usize,
    ret_h: usize,
    alpha: u8,
    show_corner_brackets: bool,
) {
    let [r, g, b] = SH2_BLUE_LINE_RGB;
    // Retail PC `sh2pc.exe` `FUN_004707b0` (`0x004707b0:508`): `iVar7 << 0x18 | 0x3060f0` passes
    // PS2's `0..0x80` alpha unshifted into D3D8 `0..0xFF` (`0x80` = 50% opacity) as a crisp 1 px hairline.
    let d3d_alpha = (alpha as u16 / 2) as u8;
    let color = [r, g, b, d3d_alpha];
    let border = (c.sy * 0.45).round().max(1.0) as usize;

    // 1. Single closed rectangle (`lookline` `0x001d6d2c..0x001d6f8c`)
    rasterizer.draw_rect_outline(rx, ry, ret_w, ret_h, border, color);

    // 2. Outer L-bracket corner accents when active (`lookline` `0x001d6fb0..0x001d7630`)
    if !show_corner_brackets || alpha != 0xff {
        return;
    }

    let x0 = rx as f32;
    let y0 = ry as f32;
    let x1 = (rx + ret_w) as f32;
    let y1 = (ry + ret_h) as f32;
    let half_w = ret_w as f32 * 0.5;
    let half_h = ret_h as f32 * 0.5;
    let ox6 = 6.0 * c.sx;
    let oy6 = 6.0 * c.sy;
    let oy5 = 5.0 * c.sy;

    // Top-left L-bracket: `(x0 - 6, y0 + h/2) -> (x0 - 6, y0 - 6) -> (x0 + w/2, y0 - 6)`
    let tl_mid_y = (x0 - ox6, y0 + half_h);
    let tl_corner = (x0 - ox6, y0 - oy6);
    let tl_mid_x = (x0 + half_w, y0 - oy6);
    draw_segment(rasterizer, tl_mid_y, tl_corner, border, color);
    draw_segment(rasterizer, tl_corner, tl_mid_x, border, color);

    // Bottom-right L-bracket: `(x0 + w/2, y1 + 5) -> (x1 + 6, y1 + 5) -> (x1 + 6, y0 + h/2)`
    let br_mid_x = (x0 + half_w, y1 + oy5);
    let br_corner = (x1 + ox6, y1 + oy5);
    let br_mid_y = (x1 + ox6, y0 + half_h);
    draw_segment(rasterizer, br_mid_x, br_corner, border, color);
    draw_segment(rasterizer, br_corner, br_mid_y, border, color);
}

/// Computes the exact `SLUS_202.28` `sprite` (`0x001d8890`) RGB color for layer `layer` (`0..6`).
///
/// - `c = scolor - layer * (scolor / 6)` with `scolor = 60` (`0x003400d0`).
/// - Inactive: `R = 0, G = c, B = 3 * c`.
/// - Active: pulses via triangle wave `p = if tab_pulse < 1.0 { tab_pulse } else { 2.0 - tab_pulse }`:
///   `R = 4 * c * p`, `G = c + 3 * c * p`, `B = 3 * c + c * p` (white-cyan at `p = 1.0`).
pub fn sprite_layer_rgb(layer: usize, active: bool, tab_pulse: f32) -> [u8; 3] {
    let c = (SPRITE_SCOLOR as f32 - layer as f32 * (SPRITE_SCOLOR as f32 / 6.0)) as i32;
    let c = c.max(0);
    if !active {
        return [0, c.clamp(0, 255) as u8, (c * 3).clamp(0, 255) as u8];
    }
    let p = if tab_pulse < 1.0 {
        tab_pulse.clamp(0.0, 1.0)
    } else {
        (2.0 - tab_pulse).clamp(0.0, 1.0)
    };
    let r = (p * (c * 4) as f32).clamp(0.0, 255.0) as u8;
    let g = (c as f32 + p * (c * 3) as f32).clamp(0.0, 255.0) as u8;
    let b = ((c * 3) as f32 + p * c as f32).clamp(0.0, 255.0) as u8;
    [r, g, b]
}

/// Renders one 6-layer tapering cyan-blue header/footer bar (`SLUS_202.28` `sprite` `0x001d8890`).
///
/// Each layer `i = 0..6` tapers X inward by `(i * i) / 10` px and draws two 1px horizontal strips
/// at `cy_448 + i` and `cy_448 - i - 1` (non-overlapping opaque `PRIM = 6` sprites on PS2).
pub fn draw_sprite_bar(
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
    x0_512: f32,
    cy_448: f32,
    active: bool,
    tab_pulse: f32,
) {
    let strip_h = c.sy.ceil().max(1.0) as usize;
    for i in 0..GLOW_BAR_LAYERS {
        let [r, g, b] = sprite_layer_rgb(i, active, tab_pulse);
        let taper = ((i * i) / 10) as f32;
        let lx0 = c.xi(x0_512 + taper);
        let lx1 = c.xi(x0_512 + SPRITE_BAR_WIDTH - taper);
        let w = lx1.saturating_sub(lx0);
        if w == 0 {
            continue;
        }
        let rgba = [r, g, b, 255];
        let y_pos = c.yi(cy_448 + i as f32);
        let y_neg = c.yi(cy_448 - i as f32 - 1.0);
        rasterizer.fill_rect(lx0, y_pos, w, strip_h, rgba);
        rasterizer.fill_rect(lx0, y_neg, w, strip_h, rgba);
    }
}

/// Computes the exact `SLUS_202.28` `look_command` (`0x001d8f50`) green-teal RGB for layer `layer` (`0..6`).
///
/// `R = 0, G = (v / 2) * (6 - layer), B = (v / 4) * (6 - layer)` where `v` is `_DAT_0116e3d8` (`0..=40`).
pub fn look_command_layer_rgb(layer: usize, glow: f32) -> [u8; 3] {
    let v = (glow.round() as i32).clamp(0, LOOK_COMMAND_MAX_GLOW as i32);
    let weight = (GLOW_BAR_LAYERS as i32 - layer as i32).max(0);
    let g = ((v / 2) * weight).clamp(0, 255) as u8;
    let b = ((v / 4) * weight).clamp(0, 255) as u8;
    [0, g, b]
}

/// Renders the `SLUS_202.28` `look_command` (`0x001d8f50`) 6-layer green-teal command highlight bar
/// spanning the full 140px Command column (`x = 90..230` centered = `346..486` in `512x448`).
pub fn draw_look_command(
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
    center_y_448: f32,
    glow: f32,
) {
    if (glow.round() as i32) <= 0 {
        return;
    }
    let strip_h = c.sy.ceil().max(1.0) as usize;
    for i in 0..GLOW_BAR_LAYERS {
        let [r, g, b] = look_command_layer_rgb(i, glow);
        if g == 0 && b == 0 {
            continue;
        }
        let taper = ((i * i) / 10) as f32;
        let lx0 = c.xi(LOOK_COMMAND_X0 + taper);
        let lx1 = c.xi(LOOK_COMMAND_X1 - taper);
        let w = lx1.saturating_sub(lx0);
        if w == 0 {
            continue;
        }
        let rgba = [r, g, b, 255];
        let y_pos = c.yi(center_y_448 + i as f32);
        let y_neg = c.yi(center_y_448 - i as f32 - 1.0);
        rasterizer.fill_rect(lx0, y_pos, w, strip_h, rgba);
        rasterizer.fill_rect(lx0, y_neg, w, strip_h, rgba);
    }
}

/// Returns the `SLUS_202.28` `font_print` (`0x001d4c00` lines 2449–2460) `stock_line` width
/// parameter (`0x1f` / `0x2d` / `0x3b`) for a 1-, 2-, or 3+-digit count.
pub const fn stock_line_width_for_count(count: u32) -> f32 {
    if count < 10 {
        STOCK_LINE_WIDTH_1DIGIT
    } else if count < 100 {
        STOCK_LINE_WIDTH_2DIGIT
    } else {
        STOCK_LINE_WIDTH_3DIGIT
    }
}

/// Renders the `SLUS_202.28` `stock_line` (`0x001da170`) pair of 2px-tall `0xffa0a0a0` horizontal
/// flanking bars and returns the centered `(mid_x_512, mid_y_448)` text anchor between them.
///
/// - `equipment_mode == false` (`param_2 == 0`, carousel item count or Flashlight/Radio ON/OFF):
///   right bar `[294..306]`, left bar `[294 - width .. 306 - width]`, `y = [274..276]` (`0x32..0x34` centered).
/// - `equipment_mode == true` (`param_2 == 1`, equipped firearm magazine count):
///   right bar `[320..332]`, left bar `[320 - width .. 332 - width]`, `y = [144..146]` (`-0x50..-0x4e` centered).
pub fn draw_stock_line(
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
    width_512: f32,
    equipment_mode: bool,
) -> (f32, f32) {
    let (base_left, base_right, y_top, y_bot) = if equipment_mode {
        // `0x40..0x4c` (+256 = 320..332), `-0x50..-0x4e` (+224 = 144..146)
        (320.0, 332.0, 144.0, 146.0)
    } else {
        // `0x26..0x32` (+256 = 294..306), `0x32..0x34` (+224 = 274..276)
        (294.0, 306.0, 274.0, 276.0)
    };
    let bar_h = ((y_bot - y_top) * c.sy).round().max(1.0) as usize;
    let py = c.yi(y_top);
    for i in 0..2 {
        let shift = (i as f32) * width_512;
        let x0 = c.xi(base_left - shift);
        let x1 = c.xi(base_right - shift);
        let w = x1.saturating_sub(x0).max(1);
        rasterizer.fill_rect(x0, py, w, bar_h, STOCK_LINE_RGBA);
    }
    // Midpoint of the inner gap `[base_right - width_512 .. base_left]`
    let gap_mid_x = ((base_right - width_512) + base_left) * 0.5;
    let gap_mid_y = (y_top + y_bot) * 0.5;
    (gap_mid_x, gap_mid_y)
}

/// Renders the `SLUS_202.28` `look_combine` (`0x001da490` / `sh2pc 0x004700c0`) 3 green-tinted
/// combine preview slots at bottom-right (`x = [100 + i*50 .. 140 + i*50]`, `y = [45..95]` centered)
/// with `0.4`-scale item icons.
pub fn draw_look_combine(
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
    slots: [Option<u8>; 3],
    fade: f32,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
) {
    if fade <= 0.001 {
        return;
    }
    // PS2 GS alpha `(fade * 80.0)` in `0..128` -> `(fade * 160.0)` in `0..255`.
    let alpha = (fade * 160.0).round().clamp(0.0, 255.0) as u8;
    let fill_rgba = [
        LOOK_COMBINE_FILL_RGB_PS2[0],
        LOOK_COMBINE_FILL_RGB_PS2[1],
        LOOK_COMBINE_FILL_RGB_PS2[2],
        alpha,
    ];
    let border_rgba = [
        LOOK_COMBINE_BORDER_RGB[0],
        LOOK_COMBINE_BORDER_RGB[1],
        LOOK_COMBINE_BORDER_RGB[2],
        alpha,
    ];
    let border = (c.sy * 0.85).round().max(1.0) as usize;

    for (i, slot_item) in slots.iter().copied().enumerate() {
        let sx0 = 100.0 + (i as f32) * 50.0 + PS2_CENTER_X;
        let sx1 = 140.0 + (i as f32) * 50.0 + PS2_CENTER_X;
        let sy0 = 45.0 + PS2_CENTER_Y;
        let sy1 = 95.0 + PS2_CENTER_Y;

        let x0 = c.xi(sx0);
        let y0 = c.yi(sy0);
        let w = c.xi(sx1).saturating_sub(x0).max(1);
        let h = c.yi(sy1).saturating_sub(y0).max(1);
        rasterizer.fill_rect(x0, y0, w, h, fill_rgba);
        rasterizer.draw_rect_outline(x0, y0, w, h, border, border_rgba);

        if let (Some(item_id), Some((atlas, aw, ah))) = (slot_item, item_atlas_rgba) {
            if let Some(rect) = item_icon_rect(item_id) {
                let cx = c.x((sx0 + sx1) * 0.5);
                let cy = c.y((sy0 + sy1) * 0.5);
                let icon_h = (105.0 * LOOK_COMBINE_ICON_SCALE) * c.sy;
                let icon_w = c.width_for_height(icon_h, rect.aspect());
                let (u0, v0, u1, v1) = rect.normalized_uv();
                let ix0 = cx - icon_w * 0.5;
                let iy0 = cy - icon_h * 0.5;
                let ix1 = cx + icon_w * 0.5;
                let iy1 = cy + icon_h * 0.5;
                let tris = [
                    (ix0, iy0, u0, v0),
                    (ix1, iy0, u1, v0),
                    (ix1, iy1, u1, v1),
                    (ix0, iy0, u0, v0),
                    (ix1, iy1, u1, v1),
                    (ix0, iy1, u0, v1),
                ];
                rasterizer.blit_overlay_triangles(&tris, atlas, aw, ah, fade);
            }
        }
    }
}

/// Renders the `SLUS_202.28` `look_volume` (`0x001d9480` / `sh2pc 0x0046e9e0`) 16-bar green Radio
/// volume meter inside the Command column:
/// - `i = 0..16`, `x = [105 + i*7 .. 109 + i*7]` centered (`361 + i*7 .. 365 + i*7` in `512x448`),
/// - `y = [-120 .. -100]` centered (`104..124` in `512x448`),
/// - Active (`i <= radio_volume`): `RGB(0, 176, 0)` (`0x2000b000`); inactive: `RGB(0, 48, 0)` (`0x20003000`).
pub fn draw_look_volume(rasterizer: &mut TextRasterizer, c: Ps2Canvas, radio_volume: u8) {
    let vol = radio_volume.min((RADIO_VOLUME_BARS - 1) as u8) as usize;
    let y0 = c.yi(-120.0 + PS2_CENTER_Y);
    let y1 = c.yi(-100.0 + PS2_CENTER_Y);
    let h = y1.saturating_sub(y0).max(1);

    for i in 0..RADIO_VOLUME_BARS {
        let cx0 = 105.0 + (i as f32) * 7.0 + PS2_CENTER_X;
        let cx1 = 109.0 + (i as f32) * 7.0 + PS2_CENTER_X;
        let x0 = c.xi(cx0);
        let w = c.xi(cx1).saturating_sub(x0).max(1);
        let rgb = if i <= vol {
            RADIO_VOLUME_ACTIVE_RGB
        } else {
            RADIO_VOLUME_INACTIVE_RGB
        };
        rasterizer.fill_rect(x0, y0, w, h, [rgb[0], rgb[1], rgb[2], 255]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_near_matches_slus_001d34e0() {
        // Increasing: cur=0, tgt=1000, div=2.0, speed=0.2, dt=1/30 -> 0 + 500 + 0.2 = 500.2
        let v = move_near(2.0, 0.2, 0.0, 1000.0, 1.0 / 30.0);
        assert!((v - 500.2).abs() < 1e-3, "got {v}");
        // Decreasing: cur=1000, tgt=0 -> 1000 - 500 - 0.2 = 499.8
        let v2 = move_near(2.0, 0.2, 1000.0, 0.0, 1.0 / 30.0);
        assert!((v2 - 499.8).abs() < 1e-3, "got {v2}");
        // Clamp on overshoot
        assert_eq!(move_near(2.0, 0.2, 999.9, 1000.0, 1.0 / 30.0), 1000.0);
    }

    #[test]
    fn test_sprite_and_look_command_color_vectors() {
        // `sprite` (`0x001d8890`, `scolor = 60` @ `0x003400d0`):
        // Inactive layer 0: R=0, G=60, B=180; layer 5: R=0, G=10, B=30
        assert_eq!(sprite_layer_rgb(0, false, 0.5), [0, 60, 180]);
        assert_eq!(sprite_layer_rgb(5, false, 0.5), [0, 10, 30]);
        // Active layer 0 at peak (tab_pulse = 1.0): R=240, G=240, B=240
        assert_eq!(sprite_layer_rgb(0, true, 1.0), [240, 240, 240]);
        // Active layer 0 at trough (tab_pulse = 0.0 or 2.0): matches inactive [0, 60, 180]
        assert_eq!(sprite_layer_rgb(0, true, 0.0), [0, 60, 180]);
        assert_eq!(sprite_layer_rgb(0, true, 2.0), [0, 60, 180]);

        // `look_command` (`0x001d8f50`, max glow = 40):
        // Layer 0: R=0, G=(40/2)*6 = 120, B=(40/4)*6 = 60
        assert_eq!(look_command_layer_rgb(0, 40.0), [0, 120, 60]);
        // Layer 5: R=0, G=20, B=10
        assert_eq!(look_command_layer_rgb(5, 40.0), [0, 20, 10]);
        // Glow 0 -> [0, 0, 0]
        assert_eq!(look_command_layer_rgb(0, 0.0), [0, 0, 0]);
    }

    #[test]
    fn test_item_allay_brightness_triangle_wave() {
        assert_eq!(item_allay_brightness(50.0), 50);
        assert_eq!(item_allay_brightness(199.0), 199);
        assert_eq!(item_allay_brightness(200.0), 200);
        assert_eq!(item_allay_brightness(250.0), 150);
        assert_eq!(item_allay_brightness(350.0), 50);
    }

    #[test]
    fn test_stock_line_widths() {
        assert_eq!(stock_line_width_for_count(0), 31.0);
        assert_eq!(stock_line_width_for_count(9), 31.0);
        assert_eq!(stock_line_width_for_count(10), 45.0);
        assert_eq!(stock_line_width_for_count(99), 45.0);
        assert_eq!(stock_line_width_for_count(100), 59.0);
    }

    #[test]
    fn test_widget_pixel_outputs() {
        let mut rast = TextRasterizer::new(512, 448);
        let c = Ps2Canvas::from_viewport(512, 448);

        // `look_volume` with level 7 -> bar 0 active (0,176,0), bar 15 inactive (0,48,0)
        draw_look_volume(&mut rast, c, 7);
        let px_active = &rast.pixels[(114 * 512 + 362) * 4..][..4];
        let px_inactive = &rast.pixels[(114 * 512 + (361 + 15 * 7 + 1)) * 4..][..4];
        assert_eq!(px_active, &[0, 0xb0, 0, 255]);
        assert_eq!(px_inactive, &[0, 0x30, 0, 255]);

        // `look_combine` slot 0 at (356..396, 269..319) with fade = 1.0 (`alpha = 160` blended over transparent)
        rast.clear();
        draw_look_combine(&mut rast, c, [Some(1), None, None], 1.0, None);
        let px_fill = &rast.pixels[(290 * 512 + 370) * 4..][..4];
        assert_eq!(px_fill[3], 160);
        assert!(px_fill[1] > px_fill[0] && px_fill[1] > px_fill[2]); // Green-dominant `0x103010`
    }
}
