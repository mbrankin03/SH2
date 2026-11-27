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

use std::borrow::Cow;

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::fingerprint::UiFingerprintHasher;
use crate::font::TextRasterizer;
use crate::inventory::atlas::{item_icon_rect, project_carousel_slot};
use crate::inventory::health_ecg::{ECG_GS_ALPHA, STATUS_SNAPSHOT_H_448, STATUS_SNAPSHOT_Y_448};
use crate::inventory::widgets::{
    draw_item_allay, draw_look_combine, draw_look_command, draw_look_volume, draw_look_zanzo,
    draw_lookline, draw_sprite_bar, draw_stock_line, stock_line_width_for_count, Ps2Canvas,
    LOOK_COMMAND_ROW_PITCH, PS2_UI_HEIGHT, PS2_UI_WIDTH, SPRITE_BAR_WIDTH, SPRITE_BAR_X0,
    SPRITE_BOT_BAR_CY, SPRITE_TOP_BAR_CY, STOCK_LINE_WIDTH_ON_OFF,
};
use crate::inventory::{
    CornerButton, ExaminePhase, InventoryFocus, InventoryPlayerSnapshot, InventoryScreen,
    CAROUSEL_UNSELECTED_SCALE, CMD_WIN_MOUSE_Y_MIN,
};
use sh2_core::inventory::ITEM_ID_RADIO;
use sh2_core::lighting::darkness::ITEM_ID_FLASHLIGHT;
use sh2_core::{item_metadata, InventoryManager};

/// Command list text X in `512x448` space (`font_print` `0x001d5e14`: `0x168 = 360`).
const COMMAND_TEXT_X: f32 = 360.0;

/// Formats a `u32` into a stack buffer with zero heap allocation.
#[inline]
fn fmt_u32_stack(mut val: u32, buf: &mut [u8; 16]) -> &str {
    let mut i = buf.len();
    if val == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while val > 0 {
            i -= 1;
            buf[i] = b'0' + (val % 10) as u8;
            val /= 10;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("0")
}

/// Computes a deterministic 64-bit state fingerprint for [`draw_inventory_screen`] (`SH2-177`).
///
/// Captures every field affecting rendered pixels: carousel selection & animation offset,
/// focus zone & reticle/trail rects, command list & glow, health ECG sweep/pulse/static frame,
/// examine overlay phase/dim/popup rect, fade scrim, player snapshot, and texture identities.
#[must_use]
pub fn inventory_render_fingerprint(
    screen: &InventoryScreen,
    inventory: &InventoryManager,
    snapshot: &InventoryPlayerSnapshot,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    examine_tex_rgba: Option<(&[u8], usize, usize)>,
    viewport: (usize, usize),
) -> u64 {
    let mut h = UiFingerprintHasher::with_tag(0x494E_5645_4E54_4F52);
    h.write_usize(viewport.0);
    h.write_usize(viewport.1);
    h.write_bool(screen.is_open);
    h.write_usize(screen.owned_items.len());
    h.write_bytes(&screen.owned_items);
    h.write_usize(screen.selected_index);
    match screen.focus {
        InventoryFocus::Carousel => h.write_u8(0),
        InventoryFocus::CommandWindow => h.write_u8(1),
        InventoryFocus::CombineTarget => h.write_u8(2),
        InventoryFocus::ExamineOverlay => h.write_u8(3),
        InventoryFocus::RadioVolume => h.write_u8(4),
        InventoryFocus::Corner(btn) => {
            h.write_u8(5);
            h.write_u8(btn as u8);
        }
    }
    h.write_usize(screen.active_commands.len());
    for &cmd in &screen.active_commands {
        h.write_u8(cmd as u8);
    }
    h.write_usize(screen.command_index);
    for slot in screen.combine_slots {
        h.write_u8(slot.unwrap_or(0));
    }
    h.write_f32(screen.combine_fade);
    h.write_f32(screen.selected_scale);
    h.write_f32(screen.arrow_pulse);
    h.write_f32(screen.arrow_fade);
    h.write_f32(screen.tab_pulse);
    h.write_f32(screen.command_cursor_y);
    h.write_f32(screen.command_glow);
    h.write_u8(screen.radio_volume);
    h.write_f32(screen.health_ecg.sweep_x);
    h.write_f32(screen.health_ecg.pulse_timer);
    h.write_f32(screen.health_ecg.pulse_brightness);
    h.write_u32(screen.health_ecg.static_frame);
    match screen.status_toast.as_deref() {
        Some(t) => {
            h.write_u8(1);
            h.write_str(t);
        }
        None => h.write_u8(0),
    }
    h.write_usize(screen.examine_page);
    h.write_u8(screen.examine_phase as u8);
    h.write_f32(screen.examine_rgb);
    for v in screen.examine_popup_rect {
        h.write_f32(v);
    }
    h.write_u8(screen.examine_coin_side);
    h.write_f32(screen.carousel_anim_offset);
    for v in screen.focus_rect {
        h.write_f32(v);
    }
    for v in screen.prev_focus_rect {
        h.write_f32(v);
    }
    h.write_f32(screen.screen_fade_alpha);
    h.write_f32(snapshot.hp);
    h.write_f32(snapshot.max_hp);
    h.write_u8(snapshot.equipped_weapon_item_id.unwrap_or(0));
    h.write_u32(snapshot.equipped_magazine_rounds.unwrap_or(u32::MAX));
    h.write_bool(snapshot.flashlight_on);
    h.write_bool(snapshot.radio_on);
    h.write_u8(snapshot.radio_volume);
    h.write_bool(snapshot.has_any_memo);
    if let Some(sel_id) = screen.selected_item_id() {
        h.write_u8(sel_id);
        h.write_u16(inventory.count(sel_id));
    }
    h.write_opt_texture(item_atlas_rgba);
    h.write_opt_texture(examine_tex_rgba);
    h.finish()
}

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

    // Fullscreen `examine_main` (`0x001d3a00`, `command_kind == 9`: Letter from Mary, Laura's Letter,
    // Rebirth books) renders exclusively on pure black. Inline popup (`command_kind == 0xb/8`: Photo of
    // Mary, etc.) keeps the Inventory screen visible underneath and draws the `200x200` framed popup on top.
    if screen.focus == InventoryFocus::ExamineOverlay
        && matches!(
            screen.examine_phase,
            ExaminePhase::FullscreenInspect | ExaminePhase::FullscreenReading
        )
    {
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

    // 9. Inline `200x200` Examine Popup (`sh2pc.exe` `FUN_0046af30` `case 0xc`: Photo of Mary, etc.)
    if screen.focus == InventoryFocus::ExamineOverlay
        && matches!(
            screen.examine_phase,
            ExaminePhase::InlinePopup | ExaminePhase::InlineClosing
        )
    {
        draw_inline_examine_popup(screen, item_atlas_rgba, examine_tex_rgba, rasterizer, c);
    }

    // 10. Inventory fade-in / fade-out scrim (`SLUS_202.28` `itemmain` `0x001cdb40` & `item_fade_out` `0x001db0e0`)
    if screen.screen_fade_alpha > 0.001 {
        let a = (screen.screen_fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, a]);
    }
}

fn draw_top_headers(
    screen: &InventoryScreen,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    // `font_print` (`0x001d5cb4..0x001d5cfc`, `sh2pc_00470000.c:2614` `fontCrushOff()` -> `FontSize[0]` = `20x30`)
    // centered over each `sprite` (`0x001d8890`) 6-layer cyan-blue bar (`x0 = 26, 186, 346`, `w = 140`, `cy = 35`).
    let font_px = ((30.0 * c.sy).round() as usize).max(14);
    let x_scale = c.sx / c.sy;
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
        font.draw_line_centered_aspect(
            rasterizer,
            label,
            (cx, y_text),
            font_px,
            x_scale,
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
        rasterizer.blit_rgba_subrect_scaled((x0, y0, x1, y1), (u0, v0, u1, v1), atlas, aw, ah, 1.0);
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
        let mut num_buf = [0u8; 16];
        let num_str = fmt_u32_stack(loaded, &mut num_buf);
        font.draw_line_centered(
            rasterizer,
            num_str,
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
    // `sh2pc_00470000.c:2614` (`fontCrushOff()` -> `FontSize[0]` = `20x30` in `512x448` space).
    let font_px = ((28.0 * c.sy).round() as usize).max(12);
    let x_scale = c.sx / c.sy;
    let text_x = c.xi(COMMAND_TEXT_X);

    // `command_main` case 7 (`DAT_0116e3e4 == 1`, `0x001d0790`) + `look_volume` (`0x001d9480`):
    // render `"Volume"` header at `font_print` `(0x15e = 350, 0x5a = 90)` and 16-bar green meter at `y = 104..124`.
    if screen.focus == InventoryFocus::RadioVolume {
        font.draw_line_styled_aspect(
            rasterizer,
            "Volume",
            (c.xi(350.0), c.yi(90.0).saturating_sub(font_px)),
            font_px,
            x_scale,
            Sh2TextColor::White,
            true,
        );
        draw_look_volume(rasterizer, c, snapshot.radio_volume);
        return;
    }

    let cmds: Cow<'_, [crate::inventory::ItemCommand]> =
        if screen.focus == InventoryFocus::CommandWindow {
            Cow::Borrowed(screen.active_commands.as_slice())
        } else if let Some(id) = screen.selected_item_id() {
            Cow::Owned(screen.commands_for_item(id, inventory, snapshot))
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
        font.draw_line_styled_aspect(
            rasterizer,
            cmd.label(),
            (text_x, ry),
            font_px,
            x_scale,
            color,
            true,
        );
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

    // `lookline` (`SLUS_202.28` `0x001d6c30`, `otn_itemmain.c:386`: hidden during `0xc`/`0xd` ExamineOverlay)
    // + `look_zanzo` (`0x001daab0`) motion-trail quad while `focus_rect` glides between targets (`FUN_0046af30`).
    if screen.focus != InventoryFocus::ExamineOverlay {
        draw_look_zanzo(rasterizer, c, screen.prev_focus_rect, screen.focus_rect);
        let [ret_x0_512, ret_y0_448, ret_x1_512, ret_y1_448] = screen.focus_rect;
        let rx = c.xi(ret_x0_512);
        let ry = c.yi(ret_y0_448);
        let ret_w = c.xi(ret_x1_512).saturating_sub(rx);
        let ret_h = c.yi(ret_y1_448).saturating_sub(ry);
        // `otn_itemmain.c:2611`: outer L-brackets appear only once the reticle settles (`DAT_0116e3e6 == 0`).
        let is_settled = screen.is_focus_rect_settled();
        let show_corner_brackets = is_settled
            && matches!(
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
    }

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
        let proj = project_carousel_slot(ofs as f32 + screen.carousel_anim_offset, slot_scale);
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
            rasterizer.blit_rgba_subrect_scaled(
                (x0, y0, x1, y1),
                (u0, v0, u1, v1),
                atlas,
                aw,
                ah,
                alpha,
            );
        } else {
            let name = item_metadata(item_id).map(|m| m.name).unwrap_or("Item");
            let end = name.char_indices().nth(12).map_or(name.len(), |(i, _)| i);
            let short = &name[..end];
            font.draw_line_centered(
                rasterizer,
                short,
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
                    let mut count_buf = [0u8; 16];
                    let count_str = fmt_u32_stack(count as u32, &mut count_buf);
                    font.draw_line_centered(
                        rasterizer,
                        count_str,
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
    // Retail PC `sh2pc.exe` `font_print` (`sh2pc_00470000.c:2272, 2280`) in `512x480` space
    // (`c.y(py_448) = (py_448 + 16) * (h / 480)`): Item Name at `(20, 325)` -> `325 - 16 = 309`,
    // Description at `(45, 360)` -> `360 - 16 = 344` (`c.sx / c.sy = 1.25` in 4:3).
    let title_px = ((26.0 * c.sy).round() as usize).max(12);
    let body_px = ((24.0 * c.sy).round() as usize).max(11);
    let x_scale = c.sx / c.sy;
    let name_x = c.xi(20.0);
    let name_y = c.yi(309.0).saturating_sub(title_px);
    let desc_x = c.xi(45.0);
    let desc_y = c.yi(344.0).saturating_sub(body_px);

    if let Some(meta) = screen.selected_item_id().and_then(item_metadata) {
        font.draw_line_styled_aspect(
            rasterizer,
            meta.name,
            (name_x, name_y),
            title_px,
            x_scale,
            Sh2TextColor::White,
            true,
        );
        let mut line_y = desc_y;
        for line in meta.description.lines().take(3) {
            font.draw_line_styled_aspect(
                rasterizer,
                line,
                (desc_x, line_y),
                body_px,
                x_scale,
                Sh2TextColor::White,
                true,
            );
            line_y += body_px + 4;
        }
    }

    if let Some(ref toast) = screen.status_toast {
        font.draw_line_styled_aspect(
            rasterizer,
            toast,
            (desc_x, c.yi(395.0)),
            ((16.0 * c.sy).round() as usize).clamp(10, 22),
            x_scale,
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
    // Retail PC `sh2pc.exe` (`0x00472c20..0x00472c50`, `fontCrushOff()` -> `FontSize[0]` = `20x30`):
    // `Options | Memo | Map` centered over each `sprite` (`0x001d8890`) 6-layer cyan-blue bar (`cy = 413`).
    let font_px = ((30.0 * c.sy).round() as usize).max(14);
    let x_scale = c.sx / c.sy;
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
        font.draw_line_centered_aspect(
            rasterizer,
            btn.label(),
            (cx, y_text),
            font_px,
            x_scale,
            text_color,
            true,
        );
    }
}

/// Renders the inline `200x200` framed Examine popup over the Inventory screen
/// (`sh2pc.exe` `FUN_0046af30` `case 0xc` & `case 0xd`, `SLUS_202.28` `examine2_main` `0x001d4040`).
///
/// Stretches `examine_tex_rgba` directly to the animated 4-edge quad `screen.examine_popup_rect`
/// (`PictureDraw` `otn_itemmain.c:1826-1839`) so dismissing (`case 0xd`) collapses vertically into
/// the 4 px horizontal slit `(201, 172)..(311, 176)` like a 3D vertical card flip.
fn draw_inline_examine_popup(
    screen: &InventoryScreen,
    item_atlas_rgba: Option<(&[u8], usize, usize)>,
    examine_tex_rgba: Option<(&[u8], usize, usize)>,
    rasterizer: &mut TextRasterizer,
    c: Ps2Canvas,
) {
    let [rx0, ry0, rx1, ry1] = screen.examine_popup_rect;
    let x0 = c.xi(rx0);
    let y0 = c.yi(ry0);
    let x1 = c.xi(rx1);
    let y1 = c.yi(ry1);
    let w = x1.saturating_sub(x0).max(1);
    let h = y1.saturating_sub(y0).max(1);
    let border = (c.sy * 0.45).round().max(1.0) as usize;

    // Opaque black popup fill (`examine2_main` `0x001d4040`)
    rasterizer.fill_rect(x0, y0, w, h, [0, 0, 0, 255]);

    if let Some((rgba, tw, th)) = examine_tex_rgba {
        // `examine2_main` (`otn_itemmain.c:1826-1839`): `PictureDraw` stretches directly to the
        // animated quad `(x0, y0)..(x1, y1)`, producing the vertical flip-collapse in `case 0xd`.
        rasterizer.blit_rgba_scaled(x0, y0, w, h, rgba, tw, th, 1.0);
    } else if let (Some((atlas, aw, ah)), Some(rect)) = (
        item_atlas_rgba,
        screen.selected_item_id().and_then(item_icon_rect),
    ) {
        let (u0, v0, u1, v1) = rect.normalized_uv();
        rasterizer.blit_rgba_subrect_scaled(
            (x0 as f32, y0 as f32, (x0 + w) as f32, (y0 + h) as f32),
            (u0, v0, u1, v1),
            atlas,
            aw,
            ah,
            1.0,
        );
    }

    // `examine2_main` (`SLUS_202.28` `0x001d4040`, `otn_itemmain.c:1857-1970`, `lcolor = 45` at `0x003400c8`):
    // 4 outward-expanding + 4 inward-shrinking concentric 1 px feathered gray borders with
    // `v = 45 * (3 - i) + 0x1e` (`165, 120, 75, 30`) and GS alpha `0x20` (`64 / 255`).
    const LCOLOR: usize = 45;
    for i in 0..4usize {
        let v = (LCOLOR * (3 - i) + 0x1e).min(255) as u8;
        let col = [v, v, v, 64];
        let step_x = ((i as f32) * c.sx).round() as usize;
        let step_y = ((i as f32) * c.sy).round() as usize;
        // Outward layer (`-i` / `+i`)
        let ox = x0.saturating_sub(step_x);
        let oy = y0.saturating_sub(step_y);
        let ow = w.saturating_add(step_x * 2);
        let oh = h.saturating_add(step_y * 2);
        rasterizer.draw_rect_outline(ox, oy, ow, oh, border, col);
        // Inward layer (`+i` / `-i`)
        if w > step_x * 2 + 1 && h > step_y * 2 + 1 {
            let ix = x0 + step_x;
            let iy = y0 + step_y;
            let iw = w - step_x * 2;
            let ih = h - step_y * 2;
            rasterizer.draw_rect_outline(ix, iy, iw, ih, border, col);
        }
    }
}

/// Renders the 2-phase fullscreen `examine_main` (`SLUS_202.28` `0x001d3a00`) screen:
/// - Phase 0 (`FullscreenInspect`): full-height `PictureDraw` at `100%` brightness (`_examine_rgb = 0x80`),
///   zero text (`fontClear()`).
/// - Phase 1 (`FullscreenReading`): smoothly dims the image to `0x45 / 0x80` (`69/128`) and overlays the
///   active `\x0c` page centered directly over the image (`0xFFFA` `\a`) in `fontSetColor(1)`
///   (`item_msg_e.mes` byte `0x2298`: `0xFF01` -> `RGB(192, 192, 255)`), left-aligned block-centered.
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
        // Fullscreen `PictureDraw` (`otn_itemmain.c:1699, 1729`): fill viewport height preserving aspect
        let aspect = tw as f32 / th.max(1) as f32;
        let mut dh = h as f32;
        let mut dw = c.width_for_height(dh, aspect);
        if dw > w as f32 {
            let scale = w as f32 / dw.max(1.0);
            dw *= scale;
            dh *= scale;
        }
        let dst_w = dw.round().max(1.0) as usize;
        let dst_h = dh.round().max(1.0) as usize;
        let dx = w.saturating_sub(dst_w) / 2;
        let dy = h.saturating_sub(dst_h) / 2;
        rasterizer.blit_rgba_scaled(dx, dy, dst_w, dst_h, rgba, tw, th, 1.0);

        // Dim image smoothly from `0x80` (128) -> `0x45` (69) during Phase 1 (`otn_itemmain.c:1678-1699`)
        if screen.examine_phase == ExaminePhase::FullscreenReading {
            let brightness = (screen.examine_rgb / 128.0).clamp(0.0, 1.0);
            let dim_a = ((1.0 - brightness) * 255.0).round().clamp(0.0, 255.0) as u8;
            if dim_a > 0 {
                rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, dim_a]);
            }
        }
    }

    if screen.examine_phase == ExaminePhase::FullscreenReading {
        // `item_msg_e.mes` byte `0x2298`: `0xFFFA` (`\a` vertical center) + `0xFF01` (`fontSetColor(1)` ->
        // `FONT_COLOR_PALETTE_PS2[1] = [96, 96, 128]` in PS2 0..128 = `RGB(192, 192, 255)` in 8-bit).
        let body_px = ((30.0 * c.sy).round() as usize).max(14);
        let x_scale = c.sx / c.sy;
        let line_pitch = ((32.0 * c.sy).round() as usize).max(body_px + 2);
        let letter_color = Sh2TextColor::Custom {
            top: [192, 192, 255, 255],
            bottom: [192, 192, 255, 255],
        };

        if let Some(meta) = screen.selected_item_id().and_then(item_metadata) {
            let raw_text = meta.examine_text.unwrap_or(meta.description);
            let mut active_page = "";
            for (idx, page) in raw_text.split('\x0c').enumerate() {
                active_page = page;
                if idx == screen.examine_page {
                    break;
                }
            }
            let mut clean_lines: [Cow<'_, str>; 6] = std::array::from_fn(|_| Cow::Borrowed(""));
            let mut line_count = 0usize;
            for l in active_page.lines().take(6) {
                clean_lines[line_count] = if l.chars().any(char::is_control) {
                    Cow::Owned(l.chars().filter(|&ch| !ch.is_control()).collect())
                } else {
                    Cow::Borrowed(l)
                };
                line_count += 1;
            }
            if line_count > 0 {
                // `MesAlign::BlockCenter` (`\l`): compute max line width, left-align lines at `(w - max_w)/2`,
                // vertically centered (`\a` `0xFFFA`) directly over the dimmed examine image.
                let max_w = clean_lines[..line_count]
                    .iter()
                    .map(|l| font.text_width_aspect(l, body_px, x_scale))
                    .max()
                    .unwrap_or(0);
                let block_x = w.saturating_sub(max_w) / 2;
                let total_h = line_count * line_pitch;
                let block_y0 = h.saturating_sub(total_h) / 2;
                for (i, line) in clean_lines[..line_count].iter().enumerate() {
                    font.draw_line_styled_aspect(
                        rasterizer,
                        line,
                        (block_x, block_y0 + i * line_pitch),
                        body_px,
                        x_scale,
                        letter_color,
                        true,
                    );
                }
            }
        }
    }

    // Apply `0.5` s fade-in / fade-out scrim (`item_examine_fade_out` `0x001db170` & `examine_main` `0x001d3a00:1704`)
    if screen.screen_fade_alpha > 0.001 {
        let a = (screen.screen_fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, a]);
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
        screen.screen_fade_alpha = 0.0;
        let snap = InventoryPlayerSnapshot::default();
        let font = AtlasFont::shared(None);

        let mut rast = TextRasterizer::new(1280, 960);
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        // Top-left Status snapshot area receives retail `sh2pc.exe` (`160/255` base dim + `0x1680FF80`
        // green health tint) at 100% HP with straight-alpha compositing (`G > R == B`, `alpha > 0`).
        let status_px = &rast.pixels[(200 * 1280 + 160) * 4..][..4];
        assert!(status_px[3] > 0 && status_px[3] < 255);
        assert!(status_px[1] > status_px[0] && status_px[0] == status_px[2]);

        if let Ok(out_path) = std::env::var("SH2_PREVIEW_OUT") {
            let _ = image::save_buffer(&out_path, &rast.pixels, 1280, 960, image::ColorType::Rgba8);
        }

        // Fullscreen Examine overlay (`Letter from Mary`) must fill opaque black with zero bleed-through
        screen.focus = InventoryFocus::ExamineOverlay;
        screen.examine_phase = ExaminePhase::FullscreenReading;
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
        screen.screen_fade_alpha = 0.0;
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
        let mut rast = TextRasterizer::new(512, 480);

        // 1. CommandWindow: green-teal `look_command` glow + active `Command` bar pulse
        screen.focus = InventoryFocus::CommandWindow;
        screen.active_commands = screen.commands_for_item(24, &inv, &snap);
        screen.command_index = 1;
        for _ in 0..10 {
            screen.tick(1.0 / 30.0, &snap);
        }
        screen.tab_pulse = 1.0; // Peak active pulse -> bright white-cyan on Command bar
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        // Status bar (col 0, x=26..166, y=35+16=51 in 512x480) must remain inactive cyan-blue [0, 60, 180]
        let status_bar_px = &rast.pixels[(51 * 512 + 40) * 4..][..3];
        assert_eq!(status_bar_px, &[0, 60, 180]);
        // Command bar (col 2, x=346..486, y=35+16=51) must pulse bright white-cyan [240, 240, 240]
        let cmd_bar_px = &rast.pixels[(51 * 512 + 355) * 4..][..3];
        assert_eq!(cmd_bar_px, &[240, 240, 240]);
        // Green-teal `look_command` bar spans full column (check x = 470, y = 98+16 = 114)
        let cmd_glow_px = &rast.pixels[(114 * 512 + 470) * 4..][..3];
        assert!(
            cmd_glow_px[1] > cmd_glow_px[2] && cmd_glow_px[0] == 0,
            "expected green-teal glow at x=470, got {cmd_glow_px:?}"
        );

        // 2. RadioVolume: 16-bar green meter (y = 114+16 = 130)
        screen.focus = InventoryFocus::RadioVolume;
        rast.clear();
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        assert_eq!(&rast.pixels[(130 * 512 + 362) * 4..][..3], &[0, 0xb0, 0]);
        assert_eq!(
            &rast.pixels[(130 * 512 + (361 + 15 * 7 + 1)) * 4..][..3],
            &[0, 0x30, 0]
        );

        // 3. CombineTarget: 3 green-tinted combine preview slots (y = 290+16 = 306)
        screen.focus = InventoryFocus::CombineTarget;
        screen.combine_first_item_id = Some(24);
        for _ in 0..15 {
            screen.tick(1.0 / 30.0, &snap);
        }
        rast.clear();
        draw_inventory_screen(&screen, &inv, &snap, None, None, font, &mut rast);
        let combine_px = &rast.pixels[(306 * 512 + 370) * 4..][..4];
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

            // ExamineOverlay on Letter from Mary (Phase 1 reading, periwinkle-blue centered over letter)
            scr.selected_index = 1;
            scr.focus = InventoryFocus::ExamineOverlay;
            scr.examine_phase = ExaminePhase::FullscreenReading;
            scr.examine_rgb = 69.0;
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
