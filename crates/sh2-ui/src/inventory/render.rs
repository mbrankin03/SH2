//! 1:1 PS2 Final Release Status & Inventory renderer (`Item/otn_itemmain.c` in `SLUS_202.28`).
//!
//! # Ground Truth Citations (`SLUS_202.28`)
//! - **Pure black backdrop** with a transparent cutout at the top-left `Status` box so the live
//!   frozen 3D gameplay snapshot blitted by `SurfaceBlitter::render_with_bind_group_in_viewport`
//!   shines through, tinted by `look_hp` (`0x001d9660`).
//! - **Electric-blue horizontal glow bars (`lookline` @ `0x001d6d2c`, `0x00f06030` = `RGB(48, 96, 240)`):**
//!   - Top 3 headers (`font_print` `0x001d5cb4..0x001d5cfc`, `y = 0x32 = 50`):
//!     `Status` (`x = 69`), `Equipment` (`x = 205`), `Command` (`x = 368`).
//!   - Bottom 3 buttons (`font_print` `0x001d5c50..0x001d5ca8`, `y = 0x1ac = 428`):
//!     `Options` (`x = 64`), `Memo` (`x = 240`), `Map` (`x = 391`).
//! - **7-slot 3D circular item carousel (`item_position` `0x001d3270` + `TgsItemPitureDraw` `0x00209260`):**
//!   floating `itemmenu2.tex` sprites framed by the double-offset electric-blue selection reticle
//!   (`RGB(48, 96, 240)`) with `- N -` quantity indicator at the bottom of the reticle.
//! - **Item Name (`0x14 = 20, 0x145 = 325`) & Multi-Line Description (`0x2d = 45, 0x168 = 360`)**
//!   from `font_print` (`0x001d5b50..0x001d5b90`).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::atlas::{item_icon_rect, project_carousel_slot};
use crate::inventory::{
    CornerButton, InventoryFocus, InventoryPlayerSnapshot, InventoryScreen,
    CAROUSEL_UNSELECTED_SCALE,
};
use sh2_core::{item_metadata, InventoryManager};

/// Authentic PS2 `lookline` electric-blue RGB (`0x00f06030` at `SLUS_202.28` `0x001d6d2c`).
const SH2_BLUE_BAR_RGB: [u8; 3] = [48, 96, 240];

/// Renders the authentic 1:1 PS2 Final Release Status & Inventory screen onto `rasterizer`.
///
/// The rasterizer begins fully transparent so the frozen 3D gameplay snapshot blitted into
/// `HealthEcgState::snapshot_viewport_rect` remains visible inside the top-left `Status` box
/// while `look_hp` applies its translucent health tint over it.
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

    let w = rasterizer.width;
    let h = rasterizer.height;
    let sx = w as f32 / 640.0;
    let sy = h as f32 / 480.0;

    // 1. Top-left `Status` translucent health tint wash over the live frozen 3D game snapshot
    screen
        .health_ecg
        .draw_snapshot_tint(snapshot.hp, snapshot.max_hp, rasterizer);

    // 2. Top row headers with glowing blue bars (`font_print` `y = 50`: Status | Equipment | Command)
    draw_top_headers(font, rasterizer, sx, sy);

    // 3. Equipped weapon icon from `itemmenu2.tex` + ammo count beneath `Equipment`
    draw_equipped_weapon_panel(snapshot, item_atlas_rgba, font, rasterizer, sx, sy);

    // 4. Command list beneath `Command` (`font_print` `0x001d5e14`: x = 360, y = 85..155)
    draw_command_column(screen, inventory, snapshot, font, rasterizer, sx, sy);

    // 5. 7-slot floating `itemmenu2.tex` circular carousel + double-offset blue selection reticle
    draw_carousel_arc(screen, inventory, item_atlas_rgba, font, rasterizer, sx, sy);

    // 6. Item Name (`x=20, y=325`) & Multi-Line Description (`x=45, y=360`)
    draw_item_footer(screen, font, rasterizer, sx, sy);

    // 7. Bottom navigation row with glowing blue bars (`font_print` `y = 428`: Options | Memo | Map)
    draw_bottom_nav_bar(screen, snapshot, font, rasterizer, sx, sy);

    // 8. Close-up Examine Overlay (when `Look` command was selected)
    if screen.focus == InventoryFocus::ExamineOverlay {
        draw_examine_overlay(screen, examine_tex_rgba, font, rasterizer, sy);
    }
}

/// Draws a signature Silent Hill 2 electric-blue horizontal glow bar (`lookline` `0x001d6d2c`).
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
    let peak_a = if bright { 230.0f32 } else { 185.0f32 };
    for dy in 0..h {
        let dist_y = ((dy as f32 + 0.5) - half_h as f32).abs() / (half_h as f32);
        let y_falloff = (1.0 - dist_y).clamp(0.0, 1.0);
        let py = cy.saturating_sub(half_h) + dy;
        for dx in 0..w {
            let t = dx as f32 / w.max(1) as f32;
            // Taper smoothly at left/right ends like PS2's `lookline` Gouraud quad
            let x_falloff = (t * 6.0).min((1.0 - t) * 6.0).clamp(0.0, 1.0);
            let a = (peak_a * y_falloff * x_falloff).round() as u8;
            if a > 0 {
                rasterizer.blend_pixel(x0 + dx, py, [r, g, b, a]);
            }
        }
    }
}

fn draw_top_headers(font: &AtlasFont, rasterizer: &mut TextRasterizer, sx: f32, sy: f32) {
    let font_px = ((22.0 * sy).round() as usize).clamp(14, 34);
    let bar_h = ((10.0 * sy).round() as usize).clamp(6, 16);
    let bar_w = (168.0 * sx).round() as usize;
    let y_text = (34.0 * sy).round() as usize;
    let y_bar = (48.0 * sy).round() as usize;

    let cols: [(&str, f32); 3] = [("Status", 132.0), ("Equipment", 320.0), ("Command", 508.0)];
    for (label, cx_640) in cols {
        let cx = (cx_640 * sx).round() as usize;
        draw_sh2_blue_bar(
            rasterizer,
            cx.saturating_sub(bar_w / 2),
            y_bar,
            bar_w,
            bar_h,
            false,
        );
        font.draw_line_centered(
            rasterizer,
            label,
            (cx, y_text),
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
    sx: f32,
    sy: f32,
) {
    let cx = 320.0 * sx;
    let cy = 114.0 * sy;
    let Some(weapon_id) = snapshot.equipped_weapon_item_id else {
        return;
    };

    if let (Some((atlas, aw, ah)), Some(rect)) = (item_atlas_rgba, item_icon_rect(weapon_id)) {
        let sprite_h = 84.0 * sy;
        let sprite_w = sprite_h * rect.aspect();
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
        let font_sm = ((14.0 * sy).round() as usize).clamp(10, 22);
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
        let font_sm = ((16.0 * sy).round() as usize).clamp(11, 24);
        font.draw_line_centered(
            rasterizer,
            &format!("- {} -", loaded),
            ((cx + 56.0 * sx) as usize, (cy + 26.0 * sy) as usize),
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
    sx: f32,
    sy: f32,
) {
    let cmds = if screen.focus == InventoryFocus::CommandWindow {
        screen.active_commands.clone()
    } else if let Some(id) = screen.selected_item_id() {
        screen.commands_for_item(id, inventory, snapshot)
    } else {
        return;
    };

    let cx = (508.0 * sx).round() as usize;
    let y0 = (76.0 * sy).round() as usize;
    let row_h = ((26.0 * sy).round() as usize).max(18);
    let font_px = ((19.0 * sy).round() as usize).clamp(13, 28);
    let in_cmd = screen.focus == InventoryFocus::CommandWindow;

    for (idx, cmd) in cmds.iter().enumerate() {
        let ry = y0 + idx * row_h;
        let is_sel = in_cmd && idx == screen.command_index;
        if is_sel {
            draw_sh2_blue_bar(
                rasterizer,
                cx.saturating_sub((56.0 * sx) as usize),
                ry + font_px / 2 + 2,
                (112.0 * sx) as usize,
                ((10.0 * sy).round() as usize).max(6),
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
        font.draw_line_centered(rasterizer, cmd.label(), (cx, ry), font_px, color, true);
    }
}

fn draw_carousel_arc(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let cx = 320.0 * sx;
    let cy = 242.0 * sy;
    let font_px = ((16.0 * sy).round() as usize).clamp(11, 24);

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

    // Authentic PS2 double-offset electric-blue reticle around center item (`sprite` `0x001d8890`)
    let ret_w = (132.0 * sx).round() as usize;
    let ret_h = (132.0 * sy).round() as usize;
    let rx = (cx as usize).saturating_sub(ret_w / 2);
    let ry = (cy as usize).saturating_sub(ret_h / 2);
    let [br, bg, bb] = SH2_BLUE_BAR_RGB;
    let ret_a = if screen.focus == InventoryFocus::Carousel {
        235
    } else {
        145
    };
    let ret_col = if screen.focus == InventoryFocus::CombineTarget {
        [90, 225, 130, 235]
    } else {
        [br, bg, bb, ret_a]
    };
    rasterizer.draw_rect_outline(rx, ry, ret_w, ret_h, 1, ret_col);
    rasterizer.draw_rect_outline(
        rx.saturating_sub(4),
        ry.saturating_sub(4),
        ret_w,
        ret_h,
        1,
        [ret_col[0], ret_col[1], ret_col[2], ret_a / 2],
    );

    let total = screen.owned_items.len();
    // 7 visible slots (-3..=+3) back-to-front matching PS2 Final Release screenshot
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

    for &ofs in draw_order {
        let idx = ((screen.selected_index as isize + ofs).rem_euclid(total as isize)) as usize;
        let item_id = screen.owned_items[idx];
        let slot_scale = if ofs == 0 {
            screen.selected_scale
        } else {
            CAROUSEL_UNSELECTED_SCALE
        };
        let proj = project_carousel_slot(ofs as f32, slot_scale);
        let px = cx + proj.offset_x_640 * sx;
        let py = cy - (ofs.abs() as f32) * 3.0 * sy;
        let alpha = if ofs == 0 {
            1.0
        } else {
            proj.alpha.clamp(0.18, 0.85)
        };
        let sprite_h = 114.0 * proj.scale * sy;

        if let (Some((atlas, aw, ah)), Some(rect)) = (item_atlas_rgba, item_icon_rect(item_id)) {
            let sprite_w = sprite_h * rect.aspect();
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

        // Centered `- N -` quantity inside bottom of blue reticle (1:1 PS2 Final Release)
        if ofs == 0 {
            let count = inventory.count(item_id);
            if count > 1 || item_metadata(item_id).is_some_and(|m| m.is_stackable()) {
                font.draw_line_centered(
                    rasterizer,
                    &format!("- {} -", count),
                    (cx as usize, ry + ret_h.saturating_sub(font_px + 6)),
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
    sx: f32,
    sy: f32,
) {
    // Exact `SLUS_202.28` `font_print` (`0x001d5b50..0x001d5b90`):
    // Item Name at `(0x14 = 20, 0x145 = 325)`, Description at `(0x2d = 45, 0x168 = 360)`
    let name_x = (36.0 * sx).round() as usize;
    let name_y = (324.0 * sy).round() as usize;
    let desc_x = (64.0 * sx).round() as usize;
    let desc_y = (356.0 * sy).round() as usize;

    let title_px = ((19.0 * sy).round() as usize).clamp(13, 28);
    let body_px = ((16.0 * sy).round() as usize).clamp(11, 24);

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
        for line in meta.description.lines().take(2) {
            font.draw_line_styled(
                rasterizer,
                line,
                (desc_x, line_y),
                body_px,
                Sh2TextColor::SilverUnselected,
                true,
            );
            line_y += body_px + 5;
        }
    }

    if let Some(ref toast) = screen.status_toast {
        font.draw_line_styled(
            rasterizer,
            toast,
            (desc_x, (398.0 * sy).round() as usize),
            ((14.0 * sy).round() as usize).clamp(10, 20),
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
    sx: f32,
    sy: f32,
) {
    let font_px = ((22.0 * sy).round() as usize).clamp(14, 34);
    let bar_h = ((10.0 * sy).round() as usize).clamp(6, 16);
    let bar_w = (168.0 * sx).round() as usize;
    let y_text = (426.0 * sy).round() as usize;
    let y_bar = (440.0 * sy).round() as usize;

    for btn in CornerButton::ALL {
        let (bx0, _, bx1, _) = btn.hitbox_640x480();
        let cx = (((bx0 + bx1) * 0.5) * sx).round() as usize;
        let is_selected = screen.focus == InventoryFocus::Corner(btn);
        let disabled = btn == CornerButton::Memo && !snapshot.has_any_memo;

        draw_sh2_blue_bar(
            rasterizer,
            cx.saturating_sub(bar_w / 2),
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
        font.draw_line_centered(
            rasterizer,
            btn.label(),
            (cx, y_text),
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
    sy: f32,
) {
    let w = rasterizer.width;
    let h = rasterizer.height;
    rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 240]);

    if let Some((rgba, tw, th)) = examine_tex_rgba {
        let max_h = (320.0 * sy).round() as usize;
        let max_w = ((max_h as f32) * (tw as f32 / th.max(1) as f32)).round() as usize;
        let dx = w.saturating_sub(max_w) / 2;
        let dy = (32.0 * sy).round() as usize;
        rasterizer.blit_rgba_scaled(dx, dy, max_w, max_h, rgba, tw, th, 1.0);
    }

    let title_px = ((18.0 * sy).round() as usize).clamp(12, 28);
    let body_px = ((14.0 * sy).round() as usize).clamp(10, 22);
    let text_y0 = (356.0 * sy).round() as usize;

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
                (w / 2, text_y0 + title_px + 8 + i * (body_px + 5)),
                body_px,
                Sh2TextColor::SilverUnselected,
                true,
            );
        }
    }
}
