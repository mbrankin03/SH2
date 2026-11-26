//! 1:1 PS2 Status & Inventory renderer (`Item/otn_itemmain.c` in `SLUS_202.28`).
//!
//! All layout coordinates are expressed directly in Silent Hill 2's native `512x448` PS2 logical
//! coordinate space centered inside a 4:3 canvas (`ps2_ui_canvas_rect`) so every element matches
//! `SLUS_202.28` and `CommandWindowMouseFix.cpp` 1:1 without horizontal stretching on 16:9/21:9:
//! - **Top-left `Status` 3D snapshot (`look_hp` @ `0x001d9660`):**
//!   `(-230, -175)..(-90, -70)` centered → `(26, 49, 140, 105)` in `512x448`, with neutral-to-red
//!   modulation and `10 - 10*(hp/max)` randomized 2px glitch scanlines.
//! - **6-layer tapering cyan-blue header/footer bars (`sprite` @ `0x001d8890`, `scolor = 60`):**
//!   3 columns of width `140` at `x0 = 26, 186, 346`, top `y = 35`, bottom `y = 413`, pulsing
//!   toward bright white-cyan exclusively on the active tab.
//! - **Top headers (`font_print` @ `0x001d5cb4..0x001d5cfc`, `y = 0x32 = 50`):**
//!   `Status`, `Equipment`, `Command`.
//! - **Command list (`look_command` @ `0x001d8f50` & `look_volume` @ `0x001d9480`):**
//!   `x = 0x168 = 360`, `26.0` px row pitch, full 140px (`346..486`) 6-layer green-teal eased glow
//!   bar, and 16-bar green Radio volume meter in `InventoryFocus::RadioVolume`.
//! - **7-slot circular carousel (`item_position` @ `0x001d3270`, `lookline` @ `0x001d6c30`,
//!   `item_allay` @ `0x001d7640`, `stock_line` @ `0x001da170`, `look_combine` @ `0x001da490`):**
//!   floating `itemmenu2.tex` sprites framed by the single `RGB(48,96,240)` reticle + active
//!   L-bracket corners, pulsing double-chevron L/R arrows, `0xffa0a0a0` flanking bars for bare
//!   counts and Flashlight/Radio `"ON"`/`"OFF"` badges, and 3 green-tinted combine preview slots.
//! - **Item Name (`0x14 = 20, 0x145 = 325`) & Multi-Line Description (`0x2d = 45, 0x168 = 360`)**
//!   from `font_print` (`0x001d5b50..0x001d5b90`).
//! - **Bottom navigation row (`font_print` @ `0x001d5c50..0x001d5ca8`, `y = 0x1ac = 428`):**
//!   `Option`, `Map`, `Memo`.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::inventory::atlas::{item_icon_rect, project_carousel_slot};
use crate::inventory::health_ecg::{ECG_GS_ALPHA, STATUS_SNAPSHOT_H_448, STATUS_SNAPSHOT_Y_448};
use crate::inventory::widgets::{
    draw_item_allay, draw_look_combine, draw_look_command, draw_look_volume, draw_lookline,
    draw_sprite_bar, draw_stock_line, stock_line_width_for_count, Ps2Canvas,
    LOOK_COMMAND_ROW_PITCH, PS2_UI_HEIGHT, PS2_UI_WIDTH, SPRITE_BAR_WIDTH, SPRITE_BAR_X0,
    SPRITE_BOT_BAR_CY, SPRITE_TOP_BAR_CY, STOCK_LINE_WIDTH_ON_OFF,
};
use crate::inventory::{
    CornerButton, InventoryFocus, InventoryPlayerSnapshot, InventoryScreen,
    CAROUSEL_UNSELECTED_SCALE, CMD_WIN_MOUSE_Y_MIN,
};
use sh2_core::inventory::ITEM_ID_RADIO;
use sh2_core::lighting::darkness::ITEM_ID_FLASHLIGHT;
use sh2_core::{item_metadata, InventoryManager};

/// Command list text X in `512x448` space (`font_print` `0x001d5e14`: `0x168 = 360`).
const COMMAND_TEXT_X: f32 = 360.0;

/// Renders the authentic 1:1 PS2 Status & Inventory screen onto `rasterizer`.
///
/// The rasterizer begins fully transparent so the frozen 3D gameplay snapshot blitted into
/// `HealthEcgState::snapshot_viewport_rect` remains visible inside the top-left `Status` box
/// while `look_hp` applies its health modulation and glitch scanlines over it.
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

    // Close-up Examine Overlay (`Look` command): render exclusively on pure black with zero
    // bleed-through from the underlying Status/Command/Carousel screen.
    if screen.focus == InventoryFocus::ExamineOverlay {
        draw_examine_overlay(screen, examine_tex_rgba, font, rasterizer, c);
        return;
    }

    // 1. Top-left `Status` health modulation + glitch scanlines (`look_hp` `0x001d9660`)
    screen
        .health_ecg
        .draw_snapshot_tint(snapshot.hp, snapshot.max_hp, rasterizer);

    // 2. Top row headers with 6-layer tapering cyan-blue bars (`sprite` `0x001d8890`)
    draw_top_headers(screen, font, rasterizer, c);

    // 3. Equipped weapon icon from `itemmenu2.tex` + bare ammo count flanked by `stock_line`
    draw_equipped_weapon_panel(snapshot, item_atlas_rgba, font, rasterizer, c);

    // 4. Command list with eased 6-layer green-teal glow (`look_command` `0x001d8f50`) or
    //    16-bar green Radio volume meter (`look_volume` `0x001d9480`)
    draw_command_column(screen, inventory, snapshot, font, rasterizer, c);

    // 5. 7-slot floating `itemmenu2.tex` carousel, `lookline` reticle + L-brackets,
    //    `item_allay` double-chevron arrows, and `stock_line` counts / ON-OFF badges
    draw_carousel_arc(
        screen,
        inventory,
        snapshot,
        item_atlas_rgba,
        font,
        rasterizer,
        c,
    );

    // 6. 3 green-tinted combine preview slots at bottom-right (`look_combine` `0x001da490`)
    draw_look_combine(
        rasterizer,
        c,
        screen.combine_slots,
        screen.combine_fade,
        item_atlas_rgba,
    );

    // 7. Item Name (`20, 325`) & Multi-Line Description (`45, 360`) from `font_print` (`0x001d5b50`)
    draw_item_footer(screen, font, rasterizer, c);

    // 8. Bottom navigation row with 6-layer tapering cyan-blue bars (`sprite` `0x001d8890`)
    draw_bottom_nav_bar(screen, snapshot, font, rasterizer, c);
}

fn draw_top_headers(
    screen: &InventoryScreen,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // `font_print` (`0x001d5cb4..0x001d5cfc`, `y = 0x32 = 50` in `512x448`) centered over each
    // `sprite` (`0x001d8890`) 6-layer cyan-blue bar (`x0 = 26, 186, 346`, `w = 140`, `cy = 35`).
    let font_px = ((22.0 * c.sy).round() as usize).max(12);
    let y_bar = c.yi(SPRITE_TOP_BAR_CY);
    let y_text = y_bar.saturating_sub(font_px / 2);

    let cmd_active = matches!(
        screen.focus,
        InventoryFocus::CommandWindow | InventoryFocus::RadioVolume
    );
    let cols: [(&str, f32, bool); 3] = [
        ("Status", SPRITE_BAR_X0[0], false),
        ("Equipment", SPRITE_BAR_X0[1], false),
        ("Command", SPRITE_BAR_X0[2], cmd_active),
    ];
    for (label, bar_x, active) in cols {
        draw_sprite_bar(
            rasterizer,
            c,
            bar_x,
            SPRITE_TOP_BAR_CY,
            active,
            screen.tab_pulse,
        );
        let cx = c.xi(bar_x + SPRITE_BAR_WIDTH * 0.5);
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
    c: Ps2Canvas,
) {
    // Center of middle `Equipment` column (`sprite` col 1: `186 + 140/2 = 256`,
    // `look_hp` vertical center `49 + 105/2 = 101.5`)
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

    // `font_print` (`0x001d4c00` lines 2482–2493) + `stock_line(width, 1)` (`0x001da170`):
    // bare magazine count flanked by two 2px `0xffa0a0a0` bars (`0x1f` / `0x2d`).
    if let Some(loaded) = snapshot.equipped_magazine_rounds {
        let width_512 = stock_line_width_for_count(loaded);
        let (mid_x, mid_y) = draw_stock_line(rasterizer, c, width_512, true);
        let font_sm = ((15.0 * c.sy).round() as usize).clamp(10, 24);
        font.draw_line_centered(
            rasterizer,
            &loaded.to_string(),
            (c.xi(mid_x), c.yi(mid_y).saturating_sub(font_sm / 2)),
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
    let font_px = ((18.0 * c.sy).round() as usize).max(12);
    let text_x = c.xi(COMMAND_TEXT_X);

    // `command_main` case 7 (`DAT_0116e3e4 == 1`, `0x001d0790`) + `look_volume` (`0x001d9480`):
    // render `"Volume"` header at `font_print` `(0x15e = 350, 0x5a = 90)` and 16-bar green meter at `y = 104..124`.
    if screen.focus == InventoryFocus::RadioVolume {
        font.draw_line_styled(
            rasterizer,
            "Volume",
            (c.xi(350.0), c.yi(90.0).saturating_sub(font_px)),
            font_px,
            Sh2TextColor::White,
            true,
        );
        draw_look_volume(rasterizer, c, snapshot.radio_volume);
        return;
    }

    let cmds = if screen.focus == InventoryFocus::CommandWindow {
        screen.active_commands.clone()
    } else if let Some(id) = screen.selected_item_id() {
        screen.commands_for_item(id, inventory, snapshot)
    } else {
        return;
    };

    // Exact `look_command` (`0x001d8f50`): smoothly eased Y via `move_near`, `26.0` px/row pitch,
    // full 140px Command column (`x = 90..230` centered = `346..486`), 6-layer green-teal glow.
    let y0_448 = CMD_WIN_MOUSE_Y_MIN;
    let in_cmd = screen.focus == InventoryFocus::CommandWindow;
    if screen.command_glow > 0.0 && !cmds.is_empty() {
        let half_font_448 = (font_px as f32 / c.sy) * 0.5;
        let bar_cy_448 = y0_448 + half_font_448 + screen.command_cursor_y * LOOK_COMMAND_ROW_PITCH;
        draw_look_command(rasterizer, c, bar_cy_448, screen.command_glow);
    }

    for (idx, cmd) in cmds.iter().enumerate() {
        let ry = c.yi(y0_448 + (idx as f32) * LOOK_COMMAND_ROW_PITCH);
        let is_sel = in_cmd && idx == screen.command_index;
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

#[allow(clippy::too_many_arguments)]
fn draw_carousel_arc(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    snapshot: &InventoryPlayerSnapshot,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // Center of `512x448` PS2 screen (`256, 224`)
    let cx = c.x(PS2_UI_WIDTH * 0.5);
    let cy = c.y(PS2_UI_HEIGHT * 0.5);
    let font_px = ((16.0 * c.sy).round() as usize).clamp(10, 24);

    // `lookline` (`SLUS_202.28` `0x001d6c30`, `sh2pc.exe` `FUN_0046af30` `0x0046af30`):
    // Draw before the empty-inventory early return so a focused bottom `CornerButton` still renders
    // its blue selection reticle even when no items are carried.
    let (ret_x0_512, ret_y0_448, ret_x1_512, ret_y1_448) = match screen.focus {
        InventoryFocus::Carousel | InventoryFocus::CombineTarget => (201.0, 159.0, 311.0, 289.0),
        InventoryFocus::CommandWindow | InventoryFocus::RadioVolume => (336.0, 21.0, 496.0, 144.0),
        InventoryFocus::Corner(CornerButton::Option) => (16.0, 397.0, 176.0, 427.0),
        InventoryFocus::Corner(CornerButton::Memo) => (176.0, 397.0, 336.0, 427.0),
        InventoryFocus::Corner(CornerButton::Map) => (336.0, 397.0, 496.0, 427.0),
        InventoryFocus::ExamineOverlay => (201.0, 159.0, 311.0, 289.0),
    };
    let rx = c.xi(ret_x0_512);
    let ry = c.yi(ret_y0_448);
    let ret_w = c.xi(ret_x1_512).saturating_sub(rx);
    let ret_h = c.yi(ret_y1_448).saturating_sub(ry);
    let show_corner_brackets = matches!(
        screen.focus,
        InventoryFocus::Carousel
            | InventoryFocus::CommandWindow
            | InventoryFocus::RadioVolume
            | InventoryFocus::Corner(_)
    );
    let ret_a = if matches!(
        screen.focus,
        InventoryFocus::Carousel
            | InventoryFocus::CombineTarget
            | InventoryFocus::CommandWindow
            | InventoryFocus::RadioVolume
            | InventoryFocus::Corner(_)
    ) {
        0xff
    } else {
        ECG_GS_ALPHA
    };
    draw_lookline(
        rasterizer,
        c,
        rx,
        ry,
        ret_w,
        ret_h,
        ret_a,
        show_corner_brackets,
    );

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

    // `item_allay` (`0x001d7640`): pulsing double-chevron L/R arrows flanking carousel reticle
    if screen.owned_items.len() > 1 {
        draw_item_allay(rasterizer, c, screen.arrow_pulse, screen.arrow_fade);
    }

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

        // `stock_line` (`0x001da170`) & `font_print` (`0x001d4c00` lines 2444–2481):
        // bare number flanked by two 2px `0xffa0a0a0` bars, or Flashlight/Radio `"ON"`/`"OFF"` badge.
        if ofs == 0 {
            if item_id == ITEM_ID_FLASHLIGHT || item_id == ITEM_ID_RADIO {
                let is_on = if item_id == ITEM_ID_FLASHLIGHT {
                    snapshot.flashlight_on
                } else {
                    snapshot.radio_on
                };
                let (_mid_x, mid_y) =
                    draw_stock_line(rasterizer, c, STOCK_LINE_WIDTH_ON_OFF, false);
                // `font_print` (`0x001d4c00` lines 2463–2480): `"ON"` at `x = 264`, `"OFF"` at `x = 261`,
                // vertically centered on the `stock_line` (`y = 274..276`) bar midpoint `mid_y`.
                let (label, text_x_512) = if is_on { ("ON", 264.0) } else { ("OFF", 261.0) };
                font.draw_line_styled(
                    rasterizer,
                    label,
                    (c.xi(text_x_512), c.yi(mid_y).saturating_sub(font_px / 2)),
                    font_px,
                    Sh2TextColor::White,
                    true,
                );
            } else {
                let count = inventory.count(item_id);
                if count > 1 || item_metadata(item_id).is_some_and(|m| m.is_stackable()) {
                    let width_512 = stock_line_width_for_count(count as u32);
                    let (mid_x, mid_y) = draw_stock_line(rasterizer, c, width_512, false);
                    font.draw_line_centered(
                        rasterizer,
                        &count.to_string(),
                        (c.xi(mid_x), c.yi(mid_y).saturating_sub(font_px / 2)),
                        font_px,
                        Sh2TextColor::White,
                        true,
                    );
                }
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
    // Exact `SLUS_202.28` `font_print` (`0x001d5c50..0x001d5ca8`: `Option | Map | Memo`) centered
    // over each `sprite` (`0x001d8890`) 6-layer tapering cyan-blue bar (`cy = 413`).
    let font_px = ((22.0 * c.sy).round() as usize).max(12);
    let y_bar = c.yi(SPRITE_BOT_BAR_CY);
    let y_text = y_bar.saturating_sub(font_px / 2);

    for (col_idx, btn) in CornerButton::ALL.iter().copied().enumerate() {
        let is_selected = screen.focus == InventoryFocus::Corner(btn);
        let disabled = btn == CornerButton::Memo && !snapshot.has_any_memo;

        draw_sprite_bar(
            rasterizer,
            c,
            SPRITE_BAR_X0[col_idx],
            SPRITE_BOT_BAR_CY,
            is_selected,
            screen.tab_pulse,
        );
        let text_color = if disabled {
            Sh2TextColor::MutedFooter
        } else if is_selected {
            Sh2TextColor::GoldSelected
        } else {
            Sh2TextColor::White
        };
        let cx = c.xi(SPRITE_BAR_X0[col_idx] + SPRITE_BAR_WIDTH * 0.5);
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
    c: Ps2Canvas,
) {
    let w = rasterizer.width;
    let h = rasterizer.height;
    rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 255]);

    if let Some((rgba, tw, th)) = examine_tex_rgba {
        let max_h = (270.0 * c.sy).round() as usize;
        let max_w = c
            .width_for_height(max_h as f32, tw as f32 / th.max(1) as f32)
            .round() as usize;
        let dx = w.saturating_sub(max_w) / 2;
        let dy = c.yi(36.0);
        rasterizer.blit_rgba_scaled(dx, dy, max_w, max_h, rgba, tw, th, 1.0);
    }

    let title_px = ((20.0 * c.sy).round() as usize).max(12);
    let body_px = ((16.0 * c.sy).round() as usize).max(10);
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
        let raw_text = meta.examine_text.unwrap_or(meta.description);
        let pages: Vec<&str> = raw_text.split('\x0c').collect();
        let active_page = pages
            .get(screen.examine_page.min(pages.len().saturating_sub(1)))
            .copied()
            .unwrap_or("");
        for (i, raw_line) in active_page.lines().take(6).enumerate() {
            // Strip any stray ASCII control bytes (`< 0x20`) so `.mes` control codes never leak as `?`
            let clean_line: String = raw_line.chars().filter(|&ch| !ch.is_control()).collect();
            font.draw_line_centered(
                rasterizer,
                &clean_line,
                (
                    w / 2,
                    text_y0 + title_px + (8.0 * c.sy).round() as usize + i * (body_px + 6),
                ),
                body_px,
                Sh2TextColor::SilverUnselected,
                true,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_inventory_and_examine_overlay_opaque_black() {
        let mut inv = InventoryManager::new();
        for id in [1u8, 2, 4, 18, 19, 20, 21] {
            inv.item_get_now(id);
        }
        let mut screen = InventoryScreen::new();
        screen.open(&inv);
        let snap = InventoryPlayerSnapshot::default();
        let font = AtlasFont::shared(None);

        let mut rast = TextRasterizer::new(1280, 960);
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        // Top-left Status snapshot area must stay transparent at 100% HP so 3D snapshot shows through
        assert_eq!(rast.pixels[(200 * 1280 + 160) * 4 + 3], 0);

        if let Ok(out_path) = std::env::var("SH2_PREVIEW_OUT") {
            let _ = image::save_buffer(&out_path, &rast.pixels, 1280, 960, image::ColorType::Rgba8);
        }

        // Examine overlay must fill opaque black with zero bleed-through from inventory chrome
        screen.focus = InventoryFocus::ExamineOverlay;
        rast.clear();
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        assert_eq!(rast.pixels[3], 255);
    }

    #[test]
    fn test_draw_inventory_all_parity_modes() {
        let mut inv = InventoryManager::new();
        for id in [1u8, 4, 18, 19, 24] {
            inv.item_get_now(id);
        }
        let mut screen = InventoryScreen::new();
        screen.open(&inv);
        let snap = InventoryPlayerSnapshot {
            hp: 35.0,
            max_hp: 100.0,
            equipped_weapon_item_id: Some(4),
            equipped_magazine_rounds: Some(10),
            flashlight_on: true,
            radio_on: true,
            radio_volume: 10,
            has_any_memo: true,
            ..Default::default()
        };
        let font = AtlasFont::shared(None);
        let mut rast = TextRasterizer::new(512, 448);

        // 1. CommandWindow: green-teal `look_command` glow + active `Command` bar pulse
        screen.focus = InventoryFocus::CommandWindow;
        screen.active_commands = screen.commands_for_item(24, &inv, &snap);
        screen.command_index = 1;
        for _ in 0..10 {
            screen.tick(1.0 / 30.0, &snap);
        }
        screen.tab_pulse = 1.0; // Peak active pulse -> bright white-cyan on Command bar
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        // Status bar (col 0, x=26..166, y=35) must remain inactive cyan-blue [0, 60, 180]
        let status_bar_px = &rast.pixels[(35 * 512 + 40) * 4..][..3];
        assert_eq!(status_bar_px, &[0, 60, 180]);
        // Command bar (col 2, x=346..486, y=35) must pulse bright white-cyan [240, 240, 240]
        let cmd_bar_px = &rast.pixels[(35 * 512 + 355) * 4..][..3];
        assert_eq!(cmd_bar_px, &[240, 240, 240]);
        // Green-teal `look_command` bar spans full column (check x = 470, well past old 439 cutoff)
        let cmd_glow_px = &rast.pixels[(98 * 512 + 470) * 4..][..3];
        assert!(
            cmd_glow_px[1] > cmd_glow_px[2] && cmd_glow_px[0] == 0,
            "expected green-teal glow at x=470, got {cmd_glow_px:?}"
        );

        // 2. RadioVolume: 16-bar green meter
        screen.focus = InventoryFocus::RadioVolume;
        rast.clear();
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        assert_eq!(&rast.pixels[(114 * 512 + 362) * 4..][..3], &[0, 0xb0, 0]);
        assert_eq!(
            &rast.pixels[(114 * 512 + (361 + 15 * 7 + 1)) * 4..][..3],
            &[0, 0x30, 0]
        );

        // 3. CombineTarget: 3 green-tinted combine preview slots
        screen.focus = InventoryFocus::CombineTarget;
        screen.combine_first_item_id = Some(24);
        for _ in 0..15 {
            screen.tick(1.0 / 30.0, &snap);
        }
        rast.clear();
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        let combine_px = &rast.pixels[(290 * 512 + 370) * 4..][..4];
        assert_eq!(combine_px[3], 160);
        assert!(combine_px[1] > combine_px[0]);

        // 4. Optional Layer 4 visual snapshots for SH2-152 (CommandWindow + Letter Examine)
        if let Ok(out_dir) = std::env::var("SH2_INV_BEAT4_DIR") {
            let _ = std::fs::create_dir_all(&out_dir);
            let mut inv_mary = InventoryManager::new();
            inv_mary.item_get_now(20); // Photo of Mary
            inv_mary.item_get_now(21); // Letter from Mary
            let mut scr = InventoryScreen::new();
            scr.open(&inv_mary);
            let snap_mary = InventoryPlayerSnapshot::default();
            let mut big = TextRasterizer::new(1024, 768);

            // CommandWindow focused on Photo of Mary (matches inv_02.png)
            scr.focus = InventoryFocus::CommandWindow;
            scr.active_commands = scr.commands_for_item(20, &inv_mary, &snap_mary);
            scr.command_index = 0;
            for _ in 0..10 {
                scr.tick(1.0 / 30.0, &snap_mary);
            }
            big.fill_rect(0, 0, 1024, 768, [0, 0, 0, 255]);
            draw_inventory_screen(&scr, &inv_mary, &snap_mary, None, None, font, &mut big);
            let _ = image::save_buffer(
                format!("{out_dir}/inv_cmd_window.png"),
                &big.pixels,
                1024,
                768,
                image::ColorType::Rgba8,
            );

            // ExamineOverlay on Letter from Mary (page 0, zero '?' control-code leak)
            scr.selected_index = 1;
            scr.focus = InventoryFocus::ExamineOverlay;
            scr.examine_page = 0;
            big.clear();
            draw_inventory_screen(&scr, &inv_mary, &snap_mary, None, None, font, &mut big);
            let _ = image::save_buffer(
                format!("{out_dir}/inv_letter_examine.png"),
                &big.pixels,
                1024,
                768,
                image::ColorType::Rgba8,
            );
        }
    }
}
