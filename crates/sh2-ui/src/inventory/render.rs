//! 1:1 PS2 / SH2:EE Single-Screen Status & Inventory renderer (`Item/otn_itemmain.c`).
//!
//! Renders:
//! - Dark atmospheric veil over the frozen 3D world (ambient BGM preserved per `PatchInventoryBGM.cpp`).
//! - Top-left **Health ECG Status Box** (`look_hp` @ `0x001d9660`) + equipped weapon/ammo readout.
//! - Authentic **3D Circular Arc Item Carousel** (`item_position` @ `0x001d3270` + `TgsItemPitureDraw`
//!   @ `0x00209260`) blitting crisp sprites from `itemmenu2.tex` (`sh2e/pic/etc/itemmenu2.tex` 4K HD
//!   or `data/pic/etc/itemmenu2.tex`) with depth-attenuated scale and opacity — ZERO wireframe boxes.
//! - Clean PS2 **Command Pop-up Window** (`CommandWindowMouseFix.cpp` hitboxes).
//! - Clean horizontal divider (`lookline` @ `0x001d6c30`) + bottom **Item Name & Multi-Line Description**
//!   footer (properly splitting `\n` across rows).
//! - Borderless PS2 **Corner Navigation Labels** (`MEMO`, `MAP`, `OPTION`, `EXIT`).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::atlas::{item_icon_rect, project_carousel_slot};
use crate::inventory::{
    CornerButton, InventoryFocus, InventoryPlayerSnapshot, InventoryScreen,
    CAROUSEL_UNSELECTED_SCALE, CMD_WIN_MOUSE_X_MAX, CMD_WIN_MOUSE_X_MIN, CMD_WIN_MOUSE_Y_MAX,
    CMD_WIN_MOUSE_Y_MIN,
};
use sh2_core::{item_metadata, InventoryManager};

/// Renders the authentic 1:1 PS2 / SH2:EE single-screen Status & Inventory menu onto `rasterizer`.
///
/// `item_atlas_rgba` is the decoded `sh2e/pic/etc/itemmenu2.tex` (4K HD) or `data/pic/etc/itemmenu2.tex`
/// sprite atlas `(rgba, width, height)`.
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

    // 1. Dark atmospheric veil over frozen 3D world (`otn_itemmain.c` `look_blackscr` `0x001dae60`)
    rasterizer.fill_rect(0, 0, w, h, [4, 5, 7, 178]);

    // 2. Top-left Health ECG Status Box (`look_hp` @ 0x001d9660)
    screen
        .health_ecg
        .draw(snapshot.hp, snapshot.max_hp, font, rasterizer);

    // 3. Equipped Weapon & Ammo Readout (beneath ECG box)
    draw_equipped_weapon_panel(snapshot, font, rasterizer, sx, sy);

    // 4. Four Corner Navigation Labels (`MEMO`, `MAP`, `OPTION`, `EXIT`)
    draw_corner_buttons(screen, snapshot, font, rasterizer, sx, sy);

    // 5. Authentic 3D Circular Arc Item Carousel (`item_position` + `TgsItemPitureDraw`)
    draw_carousel_arc(screen, inventory, item_atlas_rgba, font, rasterizer, sx, sy);

    // 6. Floating Command Pop-up Window (when active)
    if screen.focus == InventoryFocus::CommandWindow {
        draw_command_window(screen, font, rasterizer, sx, sy);
    }

    // 7. Bottom Item Name & Multi-Line Description Footer (`lookline` + `item_x_msg_e.mes`)
    draw_item_footer(screen, inventory, font, rasterizer, sx, sy);

    // 8. Close-up Examine Overlay (when `Look` command was selected)
    if screen.focus == InventoryFocus::ExamineOverlay {
        draw_examine_overlay(screen, examine_tex_rgba, font, rasterizer, sy);
    }
}

fn draw_equipped_weapon_panel(
    snapshot: &InventoryPlayerSnapshot,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let x = (90.0 * sx).round() as usize;
    let y = (148.0 * sy).round() as usize;
    let font_sm = ((13.0 * sy).round() as usize).clamp(10, 22);

    let weapon_name = snapshot
        .equipped_weapon_item_id
        .and_then(item_metadata)
        .map(|m| m.name)
        .unwrap_or("Unarmed");

    font.draw_line_styled(
        rasterizer,
        &format!("EQUIP: {}", weapon_name),
        (x, y),
        font_sm,
        Sh2TextColor::SilverUnselected,
        true,
    );

    if let (Some(loaded), Some(cap)) = (
        snapshot.equipped_magazine_rounds,
        snapshot.equipped_magazine_capacity,
    ) {
        let reserve = snapshot.equipped_reserve_ammo.unwrap_or(0);
        font.draw_line_styled(
            rasterizer,
            &format!("AMMO: {}/{}  (+{})", loaded, cap, reserve),
            (x, y + font_sm + 4),
            font_sm,
            Sh2TextColor::GoldSelected,
            true,
        );
    }
}

fn draw_corner_buttons(
    screen: &InventoryScreen,
    snapshot: &InventoryPlayerSnapshot,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let font_px = ((16.0 * sy).round() as usize).clamp(12, 26);
    for btn in CornerButton::ALL {
        let (bx0, by0, bx1, by1) = btn.hitbox_640x480();
        let x0 = (bx0 * sx).round() as usize;
        let y0 = (by0 * sy).round() as usize;
        let bw = ((bx1 - bx0) * sx).round().max(32.0) as usize;
        let bh = ((by1 - by0) * sy).round().max(18.0) as usize;

        let is_selected = screen.focus == InventoryFocus::Corner(btn);
        let disabled = btn == CornerButton::Memo && !snapshot.has_any_memo;

        // Authentic PS2 borderless corner text with soft glow underline on selection
        if is_selected {
            rasterizer.fill_rect(
                x0 + 4,
                y0 + bh.saturating_sub(3),
                bw.saturating_sub(8),
                2,
                [235, 200, 110, 220],
            );
        }
        let text_color = if disabled {
            Sh2TextColor::MutedFooter
        } else if is_selected {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::SilverUnselected
        };

        font.draw_line_centered(
            rasterizer,
            btn.label(),
            (x0 + bw / 2, y0 + bh.saturating_sub(font_px) / 2),
            font_px,
            text_color,
            true,
        );
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
    let cy = 248.0 * sy;
    let font_px = ((14.0 * sy).round() as usize).clamp(10, 22);

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

    let total = screen.owned_items.len();
    // Draw back-to-front so center item (`ofs == 0`) always paints on top (`item_position` depth order)
    let draw_order: &[isize] = if total >= 5 {
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
        // Subtle vertical arc curvature matching PS2 camera pitch
        let py = cy - (ofs.abs() as f32) * 6.0 * sy;
        let alpha = if ofs == 0 { 1.0 } else { proj.alpha.max(0.32) };

        let sprite_h = 128.0 * proj.scale * sy;

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
            // Fallback label only if `itemmenu2.tex` is missing from disk (e.g. headless CI)
            let name = item_metadata(item_id).map(|m| m.name).unwrap_or("Item");
            let short: String = name.chars().take(14).collect();
            font.draw_line_centered(
                rasterizer,
                &short,
                (
                    px.max(16.0) as usize,
                    (py - font_px as f32 * 0.5).max(0.0) as usize,
                ),
                font_px,
                if ofs == 0 {
                    Sh2TextColor::GoldSelected
                } else {
                    Sh2TextColor::MutedFooter
                },
                true,
            );
        }

        // Stack count badge (bottom-right of selected stackable item)
        if ofs == 0 {
            let count = inventory.count(item_id);
            if count > 1 || item_metadata(item_id).is_some_and(|m| m.is_stackable()) {
                font.draw_line_styled(
                    rasterizer,
                    &format!("x{}", count),
                    ((px + 44.0 * sx) as usize, (py + 42.0 * sy) as usize),
                    font_px,
                    Sh2TextColor::GoldSelected,
                    true,
                );
            }
        }
    }
}

fn draw_command_window(
    screen: &InventoryScreen,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let x0 = (CMD_WIN_MOUSE_X_MIN * sx).round() as usize;
    let y0 = (CMD_WIN_MOUSE_Y_MIN * sy).round() as usize;
    let w = ((CMD_WIN_MOUSE_X_MAX - CMD_WIN_MOUSE_X_MIN) * sx)
        .round()
        .max(84.0) as usize;
    let h = ((CMD_WIN_MOUSE_Y_MAX - CMD_WIN_MOUSE_Y_MIN) * sy)
        .round()
        .max(66.0) as usize;

    rasterizer.fill_rect(x0, y0, w, h, [8, 10, 12, 232]);
    rasterizer.draw_rect_outline(x0, y0, w, h, 1, [205, 185, 130, 235]);

    let row_h = (h / screen.active_commands.len().max(1)).max(18);
    let font_px = ((15.0 * sy).round() as usize).clamp(11, 24);
    for (idx, cmd) in screen.active_commands.iter().enumerate() {
        let ry = y0 + idx * row_h;
        let is_sel = idx == screen.command_index;
        if is_sel {
            rasterizer.fill_rect(
                x0 + 2,
                ry + 2,
                w.saturating_sub(4),
                row_h.saturating_sub(4),
                [64, 52, 32, 195],
            );
        }
        font.draw_line_centered(
            rasterizer,
            cmd.label(),
            (x0 + w / 2, ry + row_h.saturating_sub(font_px) / 2),
            font_px,
            if is_sel {
                Sh2TextColor::GoldSelected
            } else {
                Sh2TextColor::White
            },
            true,
        );
    }
}

fn draw_item_footer(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let x0 = (44.0 * sx).round() as usize;
    let y0 = (356.0 * sy).round() as usize;
    let w = (412.0 * sx).round() as usize;

    // Authentic PS2 horizontal rule (`lookline` @ `0x001d6c30`) — no heavy box outline
    rasterizer.fill_rect(x0, y0, w, 1, [160, 155, 140, 175]);

    let title_px = ((17.0 * sy).round() as usize).clamp(12, 26);
    let body_px = ((14.0 * sy).round() as usize).clamp(10, 22);

    if let Some(item_id) = screen.selected_item_id() {
        if let Some(meta) = item_metadata(item_id) {
            let count = inventory.count(item_id);
            let header = if meta.is_stackable() || count > 1 {
                format!("{}  (x{})", meta.name, count)
            } else {
                meta.name.to_string()
            };
            font.draw_line_styled(
                rasterizer,
                &header,
                (x0 + 4, y0 + 8),
                title_px,
                Sh2TextColor::GoldSelected,
                true,
            );
            // Split `\n` across individual lines so AtlasFont never renders `?` for newlines
            let mut line_y = y0 + 14 + title_px;
            for line in meta.description.lines().take(3) {
                font.draw_line_styled(
                    rasterizer,
                    line,
                    (x0 + 4, line_y),
                    body_px,
                    Sh2TextColor::White,
                    true,
                );
                line_y += body_px + 4;
            }
        }
    }

    if let Some(ref toast) = screen.status_toast {
        font.draw_line_styled(
            rasterizer,
            toast,
            (x0 + 4, (444.0 * sy).round() as usize),
            body_px,
            Sh2TextColor::Green,
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
    rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 220]);

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
            Sh2TextColor::GoldSelected,
            true,
        );
        let text = meta.examine_text.unwrap_or(meta.description);
        for (i, line) in text.lines().take(4).enumerate() {
            font.draw_line_centered(
                rasterizer,
                line,
                (w / 2, text_y0 + title_px + 8 + i * (body_px + 5)),
                body_px,
                Sh2TextColor::White,
                true,
            );
        }
    }
}
