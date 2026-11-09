//! 2D HUD overlay rendering for [`super::InventoryRoomController`] (`SH2-15`).

use crate::state::AppState;
use sh2_core::{item_metadata, CATALOG_ITEM_COUNT, ITEM_CATALOG, ITEM_COMBOS, MAX_ITEM_STACK};

pub(super) fn render_inventory_room_hud(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    state.begin_hud_overlay();

    let yellow = [245, 210, 65, 255];
    let cyan = [105, 215, 235, 255];
    let green = [120, 230, 150, 255];
    let white = [235, 235, 235, 255];
    let orange = [255, 165, 75, 255];
    let muted = [165, 165, 175, 255];

    state
        .menu_rasterizer
        .fill_rect(12, 10, 720, 698, [8, 10, 14, 215]);
    state
        .menu_rasterizer
        .draw_rect_outline(12, 10, 720, 698, 2, [70, 85, 110, 255]);

    state.menu_rasterizer.draw_text(
        "SILENT HILL 2 - 85-ITEM CATALOG, 3D/2D INSPECT & COMBINATIONS ROOM [SH2-15]",
        22,
        18,
        2,
        yellow,
        true,
    );

    let inv = &state.sim_core.scenario.inventory_mgr;
    state.menu_rasterizer.draw_text(
        &format!(
            "OWNED KINDS: {} | PICKED UP: {} | SECRET MASK: 0x{:02X} ({}/8) | BULLET ADJUST [B]: {}x",
            inv.owned_items().len(),
            inv.total_items_picked_up(),
            inv.secret_items_mask(),
            inv.secret_items_count(),
            inv.bullet_adjust()
        ),
        22,
        44,
        1,
        green,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "SIMULATED HP [-/=]: {:.0}/{:.0} | OVERHEAL SCORE: {:.2} | AMPOULE STAMINA: {:.1}s | MARY EXAMINES: {}/2 | FLAG 630: {}",
            state.inventory_room.simulated_hp,
            state.inventory_room.max_hp,
            inv.overheal_score(),
            inv.ampoule_stamina_timer(),
            inv.mary_examine_count(),
            state.sim_core.scenario.flags.get(630)
        ),
        22,
        62,
        1,
        cyan,
        true,
    );

    // Selected Item Detail + 2D `.tex` Preview Box
    let sel_id = state.inventory_room.selected_item_id;
    if let Some(meta) = item_metadata(sel_id) {
        let count = inv.count(sel_id);
        state.menu_rasterizer.draw_text(
            &format!(
                "SELECTED ITEM [{:>2}/85]: #{} {}  |  Category: {:?}  |  Owned: {} / {}",
                sel_id, meta.id, meta.name, meta.category, count, MAX_ITEM_STACK
            ),
            22,
            86,
            1,
            yellow,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!("DESCRIPTION : {}", meta.description.replace('\n', " / ")),
            22,
            104,
            1,
            white,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "EXAMINE [E] : {}",
                meta.examine_text
                    .unwrap_or("(No examine text)")
                    .replace('\n', " / ")
            ),
            22,
            122,
            1,
            orange,
            true,
        );
        let mdl_status = if state.inventory_room.item_3d_asset.is_some() {
            "LOADED (Spinning 3D Viewport ->)"
        } else {
            "None / 2D-Only"
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "3D MODEL    : {} ({})",
                meta.model_path.unwrap_or("None"),
                mdl_status
            ),
            22,
            140,
            1,
            cyan,
            true,
        );
        let tex_status = state
            .inventory_room
            .item_2d_tex
            .as_ref()
            .map(|(w, h, _, fmt)| format!("{w}x{h} {fmt}"))
            .unwrap_or_else(|| "None".to_string());
        state.menu_rasterizer.draw_text(
            &format!(
                "2D TEXTURE  : {} ({})",
                meta.inspect_texture_path.unwrap_or("None"),
                tex_status
            ),
            22,
            158,
            1,
            green,
            true,
        );
    }

    // Draw 2D `.tex` inspect preview box in top-right of the HUD panel (or right side)
    let preview_x = 980usize;
    let preview_y = 28usize;
    let preview_size = 220usize;
    state.menu_rasterizer.fill_rect(
        preview_x - 6,
        preview_y - 6,
        preview_size + 12,
        preview_size + 32,
        [12, 15, 22, 230],
    );
    state.menu_rasterizer.draw_rect_outline(
        preview_x - 6,
        preview_y - 6,
        preview_size + 12,
        preview_size + 32,
        2,
        yellow,
    );
    if let Some((tw, th, ref rgba, _)) = state.inventory_room.item_2d_tex {
        state.menu_rasterizer.blit_rgba_scaled(
            preview_x,
            preview_y,
            preview_size,
            preview_size,
            rgba,
            tw as usize,
            th as usize,
            1.0,
        );
    } else {
        state.menu_rasterizer.draw_text(
            "NO 2D .TEX",
            preview_x + 54,
            preview_y + 100,
            1,
            muted,
            true,
        );
    }
    state.menu_rasterizer.draw_text(
        "2D .TEX INSPECT PREVIEW",
        preview_x + 18,
        preview_y + preview_size + 6,
        1,
        yellow,
        true,
    );

    // 13-Row Scrollable Window into the 85-Item Catalog
    state.menu_rasterizer.draw_text(
        "--- 85-ITEM CATALOG BROWSER ([Up/Down/PgUp/PgDn] Select | [Enter/G] Get | [Del/X] Drop | [F] 990 Stack) ---",
        22,
        184,
        1,
        yellow,
        true,
    );

    let window_rows = 13usize;
    let half = window_rows / 2;
    let cur_idx = (sel_id as usize).saturating_sub(1);
    let start_idx = cur_idx
        .saturating_sub(half)
        .min(CATALOG_ITEM_COUNT.saturating_sub(window_rows));

    for row in 0..window_rows {
        let idx = start_idx + row;
        if idx >= CATALOG_ITEM_COUNT {
            break;
        }
        let item = &ITEM_CATALOG[idx];
        let cnt = inv.count(item.id);
        let is_sel = item.id == sel_id;
        let y = 204 + row * 17;
        if is_sel {
            state
                .menu_rasterizer
                .fill_rect(18, y - 2, 704, 16, [45, 65, 95, 220]);
        }
        let col = if is_sel {
            yellow
        } else if cnt > 0 {
            green
        } else {
            white
        };
        let prefix = if is_sel { ">>" } else { "  " };
        let mdl_tag = if item.model_path.is_some() {
            "3D"
        } else {
            "--"
        };
        let tex_tag = if item.inspect_texture_path.is_some() {
            "2D"
        } else {
            "--"
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "{} #{:>2} {:<24} | {:<15} | [{}/{}] | Qty: {:>3}",
                prefix,
                item.id,
                item.name,
                format!("{:?}", item.category),
                mdl_tag,
                tex_tag,
                cnt
            ),
            22,
            y,
            1,
            col,
            true,
        );
    }

    // 11-Recipe ITEM_COMBOS Workbench (86..=96)
    state.menu_rasterizer.draw_text(
        "--- 11 ITEM COMBINATION RECIPES (86..=96) ([Left/Right] Select | [C] Grant & Combine | [K] Use Result) ---",
        22,
        434,
        1,
        yellow,
        true,
    );

    for (idx, combo) in ITEM_COMBOS.iter().enumerate() {
        let y = 454 + idx * 17;
        let is_sel = idx == state.inventory_room.selected_combo_idx;
        if is_sel {
            state
                .menu_rasterizer
                .fill_rect(18, y - 2, 704, 16, [55, 45, 85, 220]);
        }
        let col = if is_sel { cyan } else { white };
        let prefix = if is_sel { ">>" } else { "  " };
        let p0 = item_metadata(combo.parts[0]).map(|m| m.name).unwrap_or("-");
        let p1 = item_metadata(combo.parts[1]).map(|m| m.name).unwrap_or("-");
        let p2 = if combo.parts[2] != 0 {
            item_metadata(combo.parts[2]).map(|m| m.name).unwrap_or("-")
        } else {
            "-"
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "{} Combo #{}: [{}+{}{}] -> {}",
                prefix,
                combo.result_id,
                p0,
                p1,
                if combo.parts[2] != 0 {
                    format!("+{p2}")
                } else {
                    String::new()
                },
                combo_description(combo.result_id)
            ),
            22,
            y,
            1,
            col,
            true,
        );
    }

    // Action Log & Footer Controls
    state.menu_rasterizer.draw_text(
        &format!("LAST ACTION: {}", state.inventory_room.last_action_log),
        22,
        652,
        1,
        orange,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[Up/Down/PgUp/PgDn] Item 1..85 | [Enter/G] Pick Up | [Del/X] Drop | [F] Set 990 | [B] Bullet Adjust (1x..3x)",
        22,
        672,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[U] Use Heal (#1..3) | [-/=] HP | [E] Examine | [Left/Right + C/K] Combinations | [T] Spin | [Esc] Exit",
        22,
        690,
        1,
        muted,
        true,
    );

    state.finish_hud_overlay(encoder, surface_view);
}

pub(super) fn combo_description(result_id: u8) -> &'static str {
    match result_id {
        86 => "Hospital Box Both Keys (Purple Bull + Lapis Eye)",
        87 => "Shower Drain Hook (Piece of Hair + Bent Needle)",
        88 => "Hospital Door Lady Rings (Copper Ring + Lead Ring)",
        89 => "2 Prison Tablets (Oppressor + Gluttonous Pig)",
        90 => "2 Prison Tablets (Oppressor + Seductress)",
        91 => "2 Prison Tablets (Gluttonous Pig + Seductress)",
        92 => "All 3 Prison Gallows Tablets (59 + 60 + 61)",
        93 => "Incomplete Trapdoor Pair (Horseshoe + Lighter, retains parts)",
        94 => "Incomplete Trapdoor Pair (Horseshoe + Wax Doll, retains parts)",
        95 => "Incomplete Trapdoor Pair (Lighter + Wax Doll, retains parts)",
        96 => "Complete Prison Trapdoor Handle (Wax Doll + Horseshoe + Lighter)",
        _ => "Combination Item",
    }
}
