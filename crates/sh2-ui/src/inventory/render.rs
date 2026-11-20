//! 2D overlay renderer for the 1:1 PS2 / SH2:EE Single-Screen Status & Inventory Menu.
//!
//! Renders:
//! - Semi-transparent dark backdrop over frozen 3D world (keeping ambient BGM active).
//! - Top-left **Health ECG Status Box** (`look_hp` @ `0x001d9660`).
//! - Equipped weapon & magazine/reserve ammo readout.
//! - Horizontal **Item Carousel** strip (`itemmainmain` @ `0x001cddd0`, center slot highlighted).
//! - Floating **Command Pop-up Window** (`CommandWindowMouseFix.cpp` hitboxes).
//! - Bottom **Item Name & Description Footer** (`item_x_msg_e.mes` / `item_metadata`).
//! - Four **Corner Navigation Buttons** (`MEMO`, `MAP`, `OPTION`, `EXIT`).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::{
    CornerButton, InventoryFocus, InventoryPlayerSnapshot, InventoryScreen, CMD_WIN_MOUSE_X_MAX,
    CMD_WIN_MOUSE_X_MIN, CMD_WIN_MOUSE_Y_MAX, CMD_WIN_MOUSE_Y_MIN,
};
use sh2_core::{item_metadata, InventoryManager};

/// Renders the complete single-screen Status / Inventory menu onto `rasterizer`.
///
/// When `has_3d_item_preview` is `true`, the center carousel viewport leaves a transparent window
/// so the 3D spinning `.mdl` item rendered by the GPU pass shines through cleanly.
pub fn draw_inventory_screen(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    snapshot: &InventoryPlayerSnapshot,
    has_3d_item_preview: bool,
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

    // 1. Dark atmospheric veil over frozen 3D world
    let veil_alpha = if has_3d_item_preview { 145 } else { 210 };
    rasterizer.fill_rect(0, 0, w, h, [6, 8, 10, veil_alpha]);

    // 2. Top-left Health ECG Status Box (`look_hp` @ 0x001d9660)
    screen
        .health_ecg
        .draw(snapshot.hp, snapshot.max_hp, font, rasterizer);

    // 3. Equipped Weapon & Ammo Readout (below ECG box)
    draw_equipped_weapon_panel(snapshot, font, rasterizer, sx, sy);

    // 4. Four Corner Navigation Buttons (`MEMO`, `MAP`, `OPTION`, `EXIT`)
    draw_corner_buttons(screen, snapshot, font, rasterizer, sx, sy);

    // 5. Center Carousel Strip (owned items, center slot emphasized)
    draw_carousel_strip(
        screen,
        inventory,
        has_3d_item_preview,
        font,
        rasterizer,
        sx,
        sy,
    );

    // 6. Floating Command Pop-up Window (when active)
    if screen.focus == InventoryFocus::CommandWindow {
        draw_command_window(screen, font, rasterizer, sx, sy);
    }

    // 7. Bottom Item Name & Description Footer (`item_x_msg_e.mes`)
    draw_item_footer(screen, inventory, font, rasterizer, sx, sy);

    // 8. Close-up Examine Overlay (when `Look` command was selected)
    if screen.focus == InventoryFocus::ExamineOverlay {
        draw_examine_overlay(screen, font, rasterizer, sx, sy);
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
        Sh2TextColor::White,
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
    let font_px = ((15.0 * sy).round() as usize).clamp(11, 26);
    for btn in CornerButton::ALL {
        let (bx0, by0, bx1, by1) = btn.hitbox_640x480();
        let x0 = (bx0 * sx).round() as usize;
        let y0 = (by0 * sy).round() as usize;
        let bw = ((bx1 - bx0) * sx).round().max(32.0) as usize;
        let bh = ((by1 - by0) * sy).round().max(18.0) as usize;

        let is_selected = screen.focus == InventoryFocus::Corner(btn);
        let disabled = btn == CornerButton::Memo && !snapshot.has_any_memo;

        let bg = if is_selected {
            [52, 46, 36, 225]
        } else {
            [18, 20, 22, 185]
        };
        let border = if is_selected {
            [235, 205, 130, 255]
        } else {
            [95, 95, 95, 190]
        };
        let text_color = if disabled {
            Sh2TextColor::MutedFooter
        } else if is_selected {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::White
        };

        rasterizer.fill_rect(x0, y0, bw, bh, bg);
        rasterizer.draw_rect_outline(x0, y0, bw, bh, if is_selected { 2 } else { 1 }, border);
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

fn draw_carousel_strip(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    has_3d_item_preview: bool,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let cx = (320.0 * sx).round() as usize;
    let cy = (245.0 * sy).round() as usize;
    let font_px = ((14.0 * sy).round() as usize).clamp(10, 22);

    if screen.owned_items.is_empty() {
        font.draw_line_centered(
            rasterizer,
            "(No items carried)",
            (cx, cy),
            font_px,
            Sh2TextColor::MutedFooter,
            true,
        );
        return;
    }

    // Center frame box for the active 3D rotating item (`item_screen_obj.c`)
    let box_w = (156.0 * sx * screen.selected_scale).round() as usize;
    let box_h = (128.0 * sy * screen.selected_scale).round() as usize;
    let bx = cx.saturating_sub(box_w / 2);
    let by = cy.saturating_sub(box_h / 2);
    let border_col = if screen.focus == InventoryFocus::CombineTarget {
        [130, 220, 140, 255]
    } else if screen.focus == InventoryFocus::Carousel {
        [220, 195, 125, 255]
    } else {
        [120, 120, 120, 200]
    };

    if !has_3d_item_preview {
        rasterizer.fill_rect(bx, by, box_w, box_h, [18, 22, 26, 210]);
    }
    rasterizer.draw_rect_outline(bx, by, box_w, box_h, 2, border_col);

    // Draw neighboring carousel slots (-2, -1, 0, +1, +2)
    let total = screen.owned_items.len();
    let offsets: &[isize] = if total >= 5 {
        &[-2, -1, 0, 1, 2]
    } else if total >= 3 {
        &[-1, 0, 1]
    } else {
        &[0]
    };

    for &ofs in offsets {
        let idx = ((screen.selected_index as isize + ofs).rem_euclid(total as isize)) as usize;
        let item_id = screen.owned_items[idx];
        let name = item_metadata(item_id).map(|m| m.name).unwrap_or("Item");
        let count = inventory.count(item_id);

        if ofs == 0 {
            if !has_3d_item_preview {
                font.draw_line_centered(
                    rasterizer,
                    name,
                    (cx, cy.saturating_sub(font_px / 2)),
                    font_px,
                    Sh2TextColor::White,
                    true,
                );
            }
            if count > 1 || item_metadata(item_id).is_some_and(|m| m.is_stackable()) {
                font.draw_line_styled(
                    rasterizer,
                    &format!("x{}", count),
                    (
                        bx + box_w.saturating_sub((44.0 * sx) as usize),
                        by + box_h.saturating_sub(font_px + 6),
                    ),
                    font_px,
                    Sh2TextColor::GoldSelected,
                    true,
                );
            }
        } else {
            let slot_x = (cx as isize + (ofs as f32 * 116.0 * sx) as isize).max(16) as usize;
            let sw = (88.0 * sx).round() as usize;
            let sh = (64.0 * sy).round() as usize;
            let sx0 = slot_x.saturating_sub(sw / 2);
            let sy0 = cy.saturating_sub(sh / 2);
            rasterizer.fill_rect(sx0, sy0, sw, sh, [14, 16, 18, 175]);
            rasterizer.draw_rect_outline(sx0, sy0, sw, sh, 1, [85, 85, 85, 160]);
            let short: String = name.chars().take(11).collect();
            font.draw_line_centered(
                rasterizer,
                &short,
                (slot_x, cy.saturating_sub(font_px / 2)),
                ((11.0 * sy).round() as usize).clamp(9, 18),
                Sh2TextColor::MutedFooter,
                true,
            );
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

    rasterizer.fill_rect(x0, y0, w, h, [12, 14, 16, 238]);
    rasterizer.draw_rect_outline(x0, y0, w, h, 2, [215, 195, 130, 255]);

    let row_h = (h / screen.active_commands.len().max(1)).max(18);
    let font_px = ((15.0 * sy).round() as usize).clamp(11, 24);
    for (idx, cmd) in screen.active_commands.iter().enumerate() {
        let ry = y0 + idx * row_h;
        let is_sel = idx == screen.command_index;
        if is_sel {
            rasterizer.fill_rect(
                x0 + 3,
                ry + 2,
                w.saturating_sub(6),
                row_h.saturating_sub(4),
                [68, 56, 36, 210],
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
    let y0 = (352.0 * sy).round() as usize;
    let w = (412.0 * sx).round() as usize;
    let h = (100.0 * sy).round() as usize;

    rasterizer.fill_rect(x0, y0, w, h, [10, 12, 14, 215]);
    rasterizer.draw_rect_outline(x0, y0, w, h, 1, [95, 95, 95, 190]);

    let title_px = ((16.0 * sy).round() as usize).clamp(12, 26);
    let body_px = ((13.0 * sy).round() as usize).clamp(10, 20);

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
                (x0 + 12, y0 + 10),
                title_px,
                Sh2TextColor::GoldSelected,
                true,
            );
            font.draw_line_styled(
                rasterizer,
                meta.description,
                (x0 + 12, y0 + 14 + title_px),
                body_px,
                Sh2TextColor::White,
                true,
            );
        }
    }

    if let Some(ref toast) = screen.status_toast {
        font.draw_line_styled(
            rasterizer,
            toast,
            (x0 + 12, y0 + h.saturating_sub(body_px + 10)),
            body_px,
            Sh2TextColor::Green,
            true,
        );
    }
}

fn draw_examine_overlay(
    screen: &InventoryScreen,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    sx: f32,
    sy: f32,
) {
    let x0 = (80.0 * sx).round() as usize;
    let y0 = (120.0 * sy).round() as usize;
    let w = (480.0 * sx).round() as usize;
    let h = (240.0 * sy).round() as usize;

    rasterizer.fill_rect(x0, y0, w, h, [8, 10, 12, 242]);
    rasterizer.draw_rect_outline(x0, y0, w, h, 2, [220, 200, 145, 255]);

    let title_px = ((18.0 * sy).round() as usize).clamp(12, 28);
    let body_px = ((14.0 * sy).round() as usize).clamp(10, 22);

    if let Some(meta) = screen.selected_item_id().and_then(item_metadata) {
        font.draw_line_centered(
            rasterizer,
            meta.name,
            (x0 + w / 2, y0 + 18),
            title_px,
            Sh2TextColor::GoldSelected,
            true,
        );
        let text = meta.examine_text.unwrap_or(meta.description);
        for (i, line) in text.lines().take(6).enumerate() {
            font.draw_line_styled(
                rasterizer,
                line,
                (x0 + 20, y0 + 58 + i * (body_px + 6)),
                body_px,
                Sh2TextColor::White,
                true,
            );
        }
    }

    font.draw_line_centered(
        rasterizer,
        "[Confirm / Cancel] Close",
        (x0 + w / 2, y0 + h.saturating_sub(body_px + 14)),
        body_px,
        Sh2TextColor::MutedFooter,
        true,
    );
}
