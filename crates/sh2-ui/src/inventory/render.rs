//! 1:1 PS2 Status & Inventory renderer (`Item/otn_itemmain.c` in `SLUS_202.28`).
//!
//! All layout coordinates are expressed directly in Silent Hill 2's native `512x448` PS2 logical
//! coordinate space centered inside a 4:3 canvas (`ps2_ui_canvas_rect`) so every element matches
//! `SLUS_202.28` and `CommandWindowMouseFix.cpp` 1:1 without horizontal stretching on 16:9/21:9:
//! - **Top-left `Status` 3D snapshot (`look_hp` @ `0x001d9b6c..0x001d9bfc`):**
//!   `(-230, -175)..(-90, -70)` centered → `(26, 49, 140, 105)` in `512x448`.
//! - **Electric-blue horizontal glow bars (`lookline` @ `0x001d6d2c` `0x00f06030` = `RGB(48, 96, 240)`
//!   & `sprite` @ `0x001d8c80..0x001d8d58`):** 3 columns of width `140` at `x0 = 26, 186, 346`,
//!   top row `y = 224 - 189 = 35`, bottom row `y = 224 + 189 = 413`.
//! - **Top headers (`font_print` @ `0x001d5cb4..0x001d5cfc`, `y = 0x32 = 50`):**
//!   `Status` (`0x0c`, `x = 69`), `Equipment` (`0x0d`, `x = 205`), `Command` (`0x0e`, `x = 368`).
//! - **Command list (`font_print` @ `0x001d5e14..0x001d5e38` & `CommandWindowMouseFix.cpp` `0x00472428`):**
//!   `x = 0x168 = 360`, 3-row mouse bounds `y in 63..139` with row step `25` (`0x78 - 0x5f`).
//! - **7-slot circular carousel (`item_position` @ `0x001d3270` + `TgsItemPitureDraw` @ `0x00209260`):**
//!   floating `itemmenu2.tex` sprites framed by the electric-blue selection reticle with `- N -` quantity.
//! - **Item Name (`0x14 = 20, 0x145 = 325`) & Multi-Line Description (`0x2d = 45, 0x168 = 360`)**
//!   from `font_print` (`0x001d5b50..0x001d5b90`).
//! - **Bottom navigation row (`font_print` @ `0x001d5c50..0x001d5ca8` & `item_msg_e.mes` `0x09..0x0b`,
//!   `y = 0x1ac = 428`):** `Option` (`x = 64`), `Map` (`x = 240`), `Memo` (`x = 391`).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::atlas::{item_icon_rect, project_carousel_slot};
use crate::inventory::health_ecg::{
    ECG_GS_ALPHA, STATUS_SNAPSHOT_H_448, STATUS_SNAPSHOT_W_512, STATUS_SNAPSHOT_X_512,
    STATUS_SNAPSHOT_Y_448,
};
use crate::inventory::{
    CornerButton, InventoryFocus, InventoryPlayerSnapshot, InventoryScreen,
    CAROUSEL_UNSELECTED_SCALE, CMD_WIN_MOUSE_X_MAX, CMD_WIN_MOUSE_X_MIN, CMD_WIN_MOUSE_Y_MIN,
};
use sh2_core::{item_metadata, InventoryManager};

/// Authentic PS2 `lookline` electric-blue RGB (`0x00f06030` at `SLUS_202.28` `0x001d6d2c`).
const SH2_BLUE_BAR_RGB: [u8; 3] = [48, 96, 240];

/// Native PS2 2D UI framebuffer width (`SLUS_202.28` `font_print` `0x001d4c00`).
const PS2_UI_WIDTH: f32 = 512.0;
/// Native PS2 2D UI framebuffer height (`SLUS_202.28` `font_print` `0x001d4c00`).
const PS2_UI_HEIGHT: f32 = 448.0;

/// Column bar X origins in `512x448` space (`sprite` `0x001d8c80..0x001d8c84`: `-230 + 256 + col * 160`).
const SPRITE_BAR_X0: [f32; 3] = [26.0, 186.0, 346.0];
/// Column bar width in `512x448` space (`sprite` `0x001d8c84..0x001d8d58`: `-90 - (-230) = 140`).
const SPRITE_BAR_WIDTH: f32 = STATUS_SNAPSHOT_W_512;
/// Top row bar center Y in `512x448` space (`sprite` `0x001d8ce8..0x001d8cf4`: `224 - 189 = 35`).
const SPRITE_TOP_BAR_CY: f32 = 35.0;
/// Bottom row bar center Y in `512x448` space (`sprite` `0x001d8ce8..0x001d8cf4`: `224 + 189 = 413`).
const SPRITE_BOT_BAR_CY: f32 = 413.0;

/// Command list row vertical step in `512x448` space (`font_print` `0x001d5e18..0x001d5e34`: `0x78 - 0x5f = 25`).
const COMMAND_ROW_STEP_Y: f32 = 25.0;
/// Command list text X in `512x448` space (`font_print` `0x001d5e14`: `0x168 = 360`).
const COMMAND_TEXT_X: f32 = 360.0;

/// Full-texture `512x448` PS2 UI coordinate transform.
///
/// The rasterizer texture itself represents the 4:3 PS2 frame and is blitted into
/// `ps2_ui_canvas_rect(surface_w, surface_h)` by `sh2-app`, guaranteeing 1:1 alignment with
/// `HealthEcgState::snapshot_viewport_rect` across every monitor aspect ratio (4:3, 16:10,
/// 16:9, 21:9, 32:9, and portrait).
#[derive(Clone, Copy)]
struct Ps2Canvas {
    tex_w: f32,
    tex_h: f32,
    sx: f32,
    sy: f32,
}

impl Ps2Canvas {
    fn from_viewport(w: usize, h: usize) -> Self {
        let tex_w = w.max(1) as f32;
        let tex_h = h.max(1) as f32;
        Self {
            tex_w,
            tex_h,
            sx: tex_w / PS2_UI_WIDTH,
            sy: tex_h / PS2_UI_HEIGHT,
        }
    }

    fn x(&self, px: f32) -> f32 {
        px * self.sx
    }

    fn y(&self, py: f32) -> f32 {
        py * self.sy
    }

    fn xi(&self, px: f32) -> usize {
        self.x(px).round().max(0.0) as usize
    }

    fn yi(&self, py: f32) -> usize {
        self.y(py).round().max(0.0) as usize
    }

    /// Converts a desired physical-square height (in texture pixels) and `aspect` (`w / h`) into
    /// texture-space width so that when this texture is blitted into a 4:3 viewport, the rendered
    /// element has exact `aspect` on screen regardless of `(tex_w, tex_h)`.
    fn width_for_height(&self, height_px: f32, aspect: f32) -> f32 {
        (height_px / self.tex_h) * (aspect * 0.75) * self.tex_w
    }
}

/// Renders the authentic 1:1 PS2 Status & Inventory screen onto `rasterizer`.
///
/// The rasterizer begins fully transparent so the frozen 3D gameplay snapshot blitted into
/// `HealthEcgState::snapshot_viewport_rect` remains visible inside the top-left `Status` box
/// while `look_hp` applies its health tint over it.
#[allow(clippy::too_many_arguments)]
pub fn draw_inventory_screen(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    snapshot: &InventoryPlayerSnapshot,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    examine_tex_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    if !screen.is_open {
        return;
    }

    let c = Ps2Canvas::from_viewport(rasterizer.width, rasterizer.height);

    // 1. Top-left `Status` health tint wash over the live frozen 3D game snapshot (`look_hp` `0x001d9660`)
    screen
        .health_ecg
        .draw_snapshot_tint(snapshot.hp, snapshot.max_hp, rasterizer);

    // 2. Top row headers with glowing blue bars (`font_print` `y = 50`: Status | Equipment | Command)
    draw_top_headers(font, rasterizer, c);

    // 3. Equipped weapon icon from `itemmenu2.tex` + ammo count beneath `Equipment`
    draw_equipped_weapon_panel(snapshot, item_atlas_rgba, font, rasterizer, c);

    // 4. Command list beneath `Command` (`font_print` `0x001d5e14` & `CommandWindowMouseFix.cpp`)
    draw_command_column(screen, inventory, snapshot, font, rasterizer, c);

    // 5. 7-slot floating `itemmenu2.tex` circular carousel + electric-blue selection reticle
    draw_carousel_arc(screen, inventory, item_atlas_rgba, font, rasterizer, c);

    // 6. Item Name (`20, 325`) & Multi-Line Description (`45, 360`) from `font_print` (`0x001d5b50`)
    draw_item_footer(screen, font, rasterizer, c);

    // 7. Bottom navigation row with glowing blue bars (`font_print` `y = 428`: Option | Map | Memo)
    draw_bottom_nav_bar(screen, snapshot, font, rasterizer, c);

    // 8. Close-up Examine Overlay (when `Look` command was selected)
    if screen.focus == InventoryFocus::ExamineOverlay {
        draw_examine_overlay(screen, examine_tex_rgba, font, rasterizer, c);
    }
}

/// Draws a Silent Hill 2 electric-blue horizontal glow bar (`lookline` `0x001d6d2c` `0x00f06030`).
fn draw_sh2_blue_bar(
    rasterizer: &mut TextRasterizer,
    x0: usize,
    cy: usize,
    w: usize,
    h: usize,
    bright: bool,
) {
    let [r, g, b] = SH2_BLUE_BAR_RGB;
    let half_h = (h / 2).max(1);
    let peak_a = if bright {
        255.0f32
    } else {
        ECG_GS_ALPHA as f32
    };
    for dy in 0..h {
        let dist_y = ((dy as f32 + 0.5) - half_h as f32).abs() / (half_h as f32);
        let y_falloff = (1.0 - dist_y).clamp(0.0, 1.0);
        let py = cy.saturating_sub(half_h) + dy;
        for dx in 0..w {
            let t = dx as f32 / w.max(1) as f32;
            let x_falloff = (t * 6.0).min((1.0 - t) * 6.0).clamp(0.0, 1.0);
            let a = (peak_a * y_falloff * x_falloff).round() as u8;
            if a > 0 {
                rasterizer.blend_pixel(x0 + dx, py, [r, g, b, a]);
            }
        }
    }
}

fn draw_top_headers(font: &AtlasFont, rasterizer: &mut TextRasterizer, c: Ps2Canvas) {
    // `font_print` (`0x001d5cb4..0x001d5cfc`, `y = 0x32 = 50` baseline in `512x448`):
    // `0x0c` = "Status" (`0x45 = 69`), `0x0d` = "Equipment" (`0xcd = 205`), `0x0e` = "Command" (`0x170 = 368`)
    let font_px = ((20.0 * c.sy).round() as usize).max(12);
    let bar_h = ((8.0 * c.sy).round() as usize).max(4);
    let bar_w = (SPRITE_BAR_WIDTH * c.sx).round() as usize;
    let y_bar = c.yi(SPRITE_TOP_BAR_CY);
    let y_text = c.yi(50.0).saturating_sub(font_px);

    let cols: [(&str, f32, f32); 3] = [
        ("Status", 69.0, SPRITE_BAR_X0[0]),
        ("Equipment", 205.0, SPRITE_BAR_X0[1]),
        ("Command", 368.0, SPRITE_BAR_X0[2]),
    ];
    for (label, text_x, bar_x) in cols {
        draw_sh2_blue_bar(rasterizer, c.xi(bar_x), y_bar, bar_w, bar_h, false);
        font.draw_line_styled(
            rasterizer,
            label,
            (c.xi(text_x), y_text),
            font_px,
            Sh2TextColor::White,
            true,
        );
    }
}

fn draw_equipped_weapon_panel(
    snapshot: &InventoryPlayerSnapshot,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // Center of middle `Equipment` column (`sprite` col 1: `186 + 140/2 = 256`, `look_hp` vertical center `49 + 105/2 = 101.5`)
    let cx = c.x(SPRITE_BAR_X0[1] + SPRITE_BAR_WIDTH * 0.5);
    let cy = c.y(STATUS_SNAPSHOT_Y_448 + STATUS_SNAPSHOT_H_448 * 0.5);
    let Some(weapon_id) = snapshot.equipped_weapon_item_id else {
        return;
    };

    if let (Some((atlas, aw, ah)), Some(rect)) = (item_atlas_rgba, item_icon_rect(weapon_id)) {
        let sprite_h = (STATUS_SNAPSHOT_H_448 * CAROUSEL_UNSELECTED_SCALE) * c.sy;
        let sprite_w = c.width_for_height(sprite_h, rect.aspect());
        let (u0, v0, u1, v1) = rect.normalized_uv();
        let x0 = cx - sprite_w * 0.5;
        let y0 = cy - sprite_h * 0.5;
        let x1 = cx + sprite_w * 0.5;
        let y1 = cy + sprite_h * 0.5;
        let tris = [
            (x0, y0, u0, v0),
            (x1, y0, u1, v0),
            (x1, y1, u1, v1),
            (x0, y0, u0, v0),
            (x1, y1, u1, v1),
            (x0, y1, u0, v1),
        ];
        rasterizer.blit_overlay_triangles(&tris, atlas, aw, ah, 1.0);
    } else if let Some(meta) = item_metadata(weapon_id) {
        let font_sm = ((14.0 * c.sy).round() as usize).clamp(10, 22);
        font.draw_line_centered(
            rasterizer,
            meta.name,
            (cx as usize, cy as usize),
            font_sm,
            Sh2TextColor::SilverUnselected,
            true,
        );
    }

    if let Some(loaded) = snapshot.equipped_magazine_rounds {
        let font_sm = ((15.0 * c.sy).round() as usize).clamp(10, 24);
        font.draw_line_centered(
            rasterizer,
            &format!("- {} -", loaded),
            (
                c.xi(SPRITE_BAR_X0[1] + SPRITE_BAR_WIDTH),
                c.yi(STATUS_SNAPSHOT_Y_448 + STATUS_SNAPSHOT_H_448)
                    .saturating_sub(font_sm),
            ),
            font_sm,
            Sh2TextColor::White,
            true,
        );
    }
}

fn draw_command_column(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    snapshot: &InventoryPlayerSnapshot,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    let cmds = if screen.focus == InventoryFocus::CommandWindow {
        screen.active_commands.clone()
    } else if let Some(id) = screen.selected_item_id() {
        screen.commands_for_item(id, inventory, snapshot)
    } else {
        return;
    };

    // Exact `font_print` (`0x001d5e14`: `x = 360`, row step `25`) aligned strictly inside
    // `CommandWindowMouseFix.cpp` (`0x00472428`: `x in 356..439`, `y in 63..139`, rows `< 88`, `< 114`).
    let text_x = c.xi(COMMAND_TEXT_X);
    let bar_x = c.xi(CMD_WIN_MOUSE_X_MIN);
    let bar_w = ((CMD_WIN_MOUSE_X_MAX - CMD_WIN_MOUSE_X_MIN) * c.sx).round() as usize;
    let row_h = (COMMAND_ROW_STEP_Y * c.sy).round() as usize;
    let y0 = c.yi(CMD_WIN_MOUSE_Y_MIN);
    let font_px = ((18.0 * c.sy).round() as usize).max(12);
    let in_cmd = screen.focus == InventoryFocus::CommandWindow;

    for (idx, cmd) in cmds.iter().enumerate() {
        let ry = y0 + idx * row_h;
        let is_sel = in_cmd && idx == screen.command_index;
        if is_sel {
            draw_sh2_blue_bar(
                rasterizer,
                bar_x,
                ry + font_px / 2,
                bar_w,
                ((8.0 * c.sy).round() as usize).max(4),
                true,
            );
        }
        let color = if is_sel {
            Sh2TextColor::White
        } else if in_cmd {
            Sh2TextColor::SilverUnselected
        } else {
            Sh2TextColor::MutedFooter
        };
        font.draw_line_styled(rasterizer, cmd.label(), (text_x, ry), font_px, color, true);
    }
}

fn draw_carousel_arc(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // Center of `512x448` PS2 screen (`256, 224`)
    let cx = c.x(PS2_UI_WIDTH * 0.5);
    let cy = c.y(PS2_UI_HEIGHT * 0.5);
    let font_px = ((16.0 * c.sy).round() as usize).clamp(10, 24);

    if screen.owned_items.is_empty() {
        font.draw_line_centered(
            rasterizer,
            "(No items carried)",
            (cx as usize, cy as usize),
            font_px,
            Sh2TextColor::MutedFooter,
            true,
        );
        return;
    }

    // Square center selection reticle (`STATUS_SNAPSHOT_H_448 = 105` tall, aspect 1:1 on screen),
    // colored strictly with `SH2_BLUE_BAR_RGB` (`lookline` `0x00f06030`) and `ECG_GS_ALPHA` (`0xa0`).
    let ret_h_f = STATUS_SNAPSHOT_H_448 * c.sy;
    let ret_h = ret_h_f.round() as usize;
    let ret_w = c.width_for_height(ret_h_f, 1.0).round() as usize;
    let rx = (cx as usize).saturating_sub(ret_w / 2);
    let ry = (cy as usize).saturating_sub(ret_h / 2);
    let [br, bg, bb] = SH2_BLUE_BAR_RGB;
    let ret_a = if matches!(
        screen.focus,
        InventoryFocus::Carousel | InventoryFocus::CombineTarget
    ) {
        255
    } else {
        ECG_GS_ALPHA
    };
    let border = (c.sy * 0.6).round().max(1.0) as usize;
    rasterizer.draw_rect_outline(rx, ry, ret_w, ret_h, border, [br, bg, bb, ret_a]);
    let offset = (2.0 * c.sy).round().max(1.0) as usize;
    rasterizer.draw_rect_outline(
        rx.saturating_sub(offset),
        ry.saturating_sub(offset),
        ret_w,
        ret_h,
        border,
        [br, bg, bb, ret_a / 2],
    );

    let total = screen.owned_items.len();
    let draw_order: &[isize] = if total >= 7 {
        &[-3, 3, -2, 2, -1, 1, 0]
    } else if total >= 5 {
        &[-2, 2, -1, 1, 0]
    } else if total >= 3 {
        &[-1, 1, 0]
    } else if total == 2 {
        &[1, 0]
    } else {
        &[0]
    };

    // Scale `project_carousel_slot` X offset from 640-wide reference into `512x448` space (`512/640 = 0.8`)
    let x_640_to_512 = PS2_UI_WIDTH / 640.0;

    for &ofs in draw_order {
        let idx = ((screen.selected_index as isize + ofs).rem_euclid(total as isize)) as usize;
        let item_id = screen.owned_items[idx];
        let slot_scale = if ofs == 0 {
            screen.selected_scale
        } else {
            CAROUSEL_UNSELECTED_SCALE
        };
        let proj = project_carousel_slot(ofs as f32, slot_scale);
        let px = cx + proj.offset_x_640 * x_640_to_512 * c.sx;
        let py = cy;
        let alpha = if ofs == 0 { 1.0 } else { proj.alpha };
        let sprite_h = STATUS_SNAPSHOT_H_448 * proj.scale * c.sy;

        if let (Some((atlas, aw, ah)), Some(rect)) = (item_atlas_rgba, item_icon_rect(item_id)) {
            let sprite_w = c.width_for_height(sprite_h, rect.aspect());
            let (u0, v0, u1, v1) = rect.normalized_uv();
            let x0 = px - sprite_w * 0.5;
            let y0 = py - sprite_h * 0.5;
            let x1 = px + sprite_w * 0.5;
            let y1 = py + sprite_h * 0.5;
            let tris = [
                (x0, y0, u0, v0),
                (x1, y0, u1, v0),
                (x1, y1, u1, v1),
                (x0, y0, u0, v0),
                (x1, y1, u1, v1),
                (x0, y1, u0, v1),
            ];
            rasterizer.blit_overlay_triangles(&tris, atlas, aw, ah, alpha);
        } else {
            let name = item_metadata(item_id).map(|m| m.name).unwrap_or("Item");
            let short: String = name.chars().take(12).collect();
            font.draw_line_centered(
                rasterizer,
                &short,
                (
                    px.max(16.0) as usize,
                    (py - font_px as f32 * 0.5).max(0.0) as usize,
                ),
                font_px,
                if ofs == 0 {
                    Sh2TextColor::White
                } else {
                    Sh2TextColor::MutedFooter
                },
                true,
            );
        }

        // Centered `- N -` quantity inside bottom of blue reticle
        if ofs == 0 {
            let count = inventory.count(item_id);
            if count > 1 || item_metadata(item_id).is_some_and(|m| m.is_stackable()) {
                font.draw_line_centered(
                    rasterizer,
                    &format!("- {} -", count),
                    (cx as usize, ry + ret_h.saturating_sub(font_px + 4)),
                    font_px,
                    Sh2TextColor::White,
                    true,
                );
            }
        }
    }
}

fn draw_item_footer(
    screen: &InventoryScreen,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // Exact `SLUS_202.28` `font_print` (`0x001d5b50..0x001d5b90`) in `512x448`:
    // Item Name at `(0x14 = 20, 0x145 = 325)`, Description at `(0x2d = 45, 0x168 = 360)`
    let title_px = ((18.0 * c.sy).round() as usize).max(12);
    let body_px = ((15.0 * c.sy).round() as usize).max(10);
    let name_x = c.xi(20.0);
    let name_y = c.yi(325.0).saturating_sub(title_px);
    let desc_x = c.xi(45.0);
    let desc_y = c.yi(360.0).saturating_sub(body_px);

    if let Some(meta) = screen.selected_item_id().and_then(item_metadata) {
        font.draw_line_styled(
            rasterizer,
            meta.name,
            (name_x, name_y),
            title_px,
            Sh2TextColor::White,
            true,
        );
        let mut line_y = desc_y;
        for line in meta.description.lines().take(3) {
            font.draw_line_styled(
                rasterizer,
                line,
                (desc_x, line_y),
                body_px,
                Sh2TextColor::SilverUnselected,
                true,
            );
            line_y += body_px + 4;
        }
    }

    if let Some(ref toast) = screen.status_toast {
        font.draw_line_styled(
            rasterizer,
            toast,
            (desc_x, c.yi(395.0)),
            ((14.0 * c.sy).round() as usize).clamp(10, 20),
            Sh2TextColor::Green,
            true,
        );
    }
}

fn draw_bottom_nav_bar(
    screen: &InventoryScreen,
    snapshot: &InventoryPlayerSnapshot,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // Exact `SLUS_202.28` `font_print` (`0x001d5c50..0x001d5ca8`, `y = 0x1ac = 428` baseline in `512x448`):
    // `0x09` = "Option" (`0x40 = 64`), `0x0a` = "Map" (`0xf0 = 240`), `0x0b` = "Memo" (`0x187 = 391`)
    let font_px = ((20.0 * c.sy).round() as usize).max(12);
    let bar_h = ((8.0 * c.sy).round() as usize).max(4);
    let bar_w = (SPRITE_BAR_WIDTH * c.sx).round() as usize;
    let y_bar = c.yi(SPRITE_BOT_BAR_CY);
    let y_text = c.yi(428.0).saturating_sub(font_px);

    let text_x: [f32; 3] = [64.0, 240.0, 391.0];

    for (col_idx, btn) in CornerButton::ALL.iter().copied().enumerate() {
        let is_selected = screen.focus == InventoryFocus::Corner(btn);
        let disabled = btn == CornerButton::Memo && !snapshot.has_any_memo;

        draw_sh2_blue_bar(
            rasterizer,
            c.xi(SPRITE_BAR_X0[col_idx]),
            y_bar,
            bar_w,
            bar_h,
            is_selected,
        );
        let text_color = if disabled {
            Sh2TextColor::MutedFooter
        } else if is_selected {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::White
        };
        font.draw_line_styled(
            rasterizer,
            btn.label(),
            (c.xi(text_x[col_idx]), y_text),
            font_px,
            text_color,
            true,
        );
    }
}

fn draw_examine_overlay(
    screen: &InventoryScreen,
    examine_tex_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    let w = rasterizer.width;
    let h = rasterizer.height;
    rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 240]);

    if let Some((rgba, tw, th)) = examine_tex_rgba {
        let max_h = (300.0 * c.sy).round() as usize;
        let max_w = ((max_h as f32) * (tw as f32 / th.max(1) as f32)).round() as usize;
        let dx = w.saturating_sub(max_w) / 2;
        let dy = c.yi(STATUS_SNAPSHOT_X_512);
        rasterizer.blit_rgba_scaled(dx, dy, max_w, max_h, rgba, tw, th, 1.0);
    }

    let title_px = ((18.0 * c.sy).round() as usize).clamp(12, 28);
    let body_px = ((14.0 * c.sy).round() as usize).clamp(10, 22);
    let text_y0 = c.yi(325.0);

    if let Some(meta) = screen.selected_item_id().and_then(item_metadata) {
        font.draw_line_centered(
            rasterizer,
            meta.name,
            (w / 2, text_y0),
            title_px,
            Sh2TextColor::White,
            true,
        );
        let text = meta.examine_text.unwrap_or(meta.description);
        for (i, line) in text.lines().take(4).enumerate() {
            font.draw_line_centered(
                rasterizer,
                line,
                (w / 2, text_y0 + title_px + 8 + i * (body_px + 4)),
                body_px,
                Sh2TextColor::SilverUnselected,
                true,
            );
        }
    }
}
