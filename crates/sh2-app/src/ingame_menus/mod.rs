//! Thin runtime wiring for the 1:1 PS2 / SH2:EE Single-Screen In-Game Menus (`SH2-116`):
//! - Status & Inventory Screen (`sh2_ui::inventory`, `Item/otn_itemmain.c`)
//! - Fullscreen Paper Map (`sh2_ui::map`, `Event/chizu.c`)
//! - Collected Memo Reader (`sh2_ui::memo`, `Event/memo.c`)
//! - Inventory -> Options sub-menu transition
//!
//! Opening any of these overlays freezes 60 Hz world simulation while **preserving** ambient
//! room BGM playback (`PatchInventoryBGM.cpp` @ `0x005166e8`).

mod actions;

pub(crate) use actions::build_player_snapshot;
use actions::{
    apply_inventory_action, ensure_examine_tex_loaded, ensure_item_atlas_loaded,
    ensure_map_sheet_loaded, ensure_memo_assets_loaded, handle_options_from_inventory_key,
};

use winit::keyboard::KeyCode;

use crate::menu_events::emit_menu_sound;
use crate::state::AppState;
use sh2_assets::formats::mes::MessageTable;
use sh2_ui::inventory::{draw_inventory_screen, InventoryPlayerSnapshot, InventoryScreen};
use sh2_ui::map::{
    chizu_can_read_map, chizu_world_to_sheet_norm, draw_paper_map, MapReadCheck, PaperMapAction,
    PaperMapViewer,
};
use sh2_ui::memo::{draw_memo_reader, MemoAction, MemoReader, DEFAULT_MEMO_BACKDROP_TEX};
use sh2_ui::menu::{MenuNavAction, MenuSoundCue};
use sh2_ui::{draw_options_screen, AtlasFont};

/// Runtime state & lazy retail asset caches for single-screen Inventory, Paper Map & Memo Reader.
#[derive(Default)]
pub(crate) struct InGameMenuState {
    /// Status & Inventory screen (`otn_itemmain.c`).
    pub inventory: InventoryScreen,
    /// Fullscreen Paper Map viewer (`chizu.c`).
    pub map_viewer: PaperMapViewer,
    /// Collected Memo list & multi-page reader (`memo.c`).
    pub memo_reader: MemoReader,
    /// True when Options was opened via the Inventory `OPTION` corner button.
    pub options_from_inventory: bool,
    /// Whether `memo_msg_e.mes` has been attempted to load from disk.
    pub(crate) memo_mes_loaded: bool,
    /// Cached parsed `memo_msg_e.mes` / `memo_x_msg_e.mes` message table.
    pub(crate) memo_messages: Option<MessageTable>,
    /// Cached decoded map sheet `(map_id, width, height, rgba)`.
    pub(crate) cached_map_sheet: Option<(u8, usize, usize, Vec<u8>)>,
    /// Cached decoded map marker atlas `(map_id, width, height, rgba)`.
    pub(crate) cached_map_marks: Option<(u8, usize, usize, Vec<u8>)>,
    /// Cached decoded memo backdrop `(rel_path, width, height, rgba)`.
    pub(crate) cached_memo_backdrop: Option<(&'static str, usize, usize, Vec<u8>)>,
    /// Whether `pic/etc/itemmenu2.tex` has been attempted to load from disk.
    pub(crate) item_atlas_loaded: bool,
    /// Cached decoded `itemmenu2.tex` (`4096x4096` SH2:EE HD or `1024x1024` PS2/PC) `(width, height, rgba)`.
    pub(crate) cached_item_atlas: Option<(usize, usize, Vec<u8>)>,
    /// Cached decoded close-up examine `.tex` `(item_id, width, height, rgba)`.
    pub(crate) cached_examine_tex: Option<(u8, usize, usize, Vec<u8>)>,
}

impl InGameMenuState {
    /// Returns `true` if Inventory, Paper Map, Memo Reader, or Inventory->Options is open.
    pub fn is_any_open(&self) -> bool {
        self.inventory.is_open
            || self.map_viewer.is_open
            || self.memo_reader.is_open
            || self.options_from_inventory
    }

    /// Closes all single-screen in-game menus cleanly (e.g. when returning to Main Menu).
    pub fn close_all(&mut self) {
        self.inventory.close();
        self.map_viewer.close();
        self.memo_reader.close();
        self.options_from_inventory = false;
    }

    /// Advances active menu animations (Health ECG sweep/pulse, carousel rotation, map zoom/transcription, memo fade).
    pub fn tick(&mut self, dt: f32, snapshot: &InventoryPlayerSnapshot) {
        if self.inventory.is_open {
            self.inventory.tick(dt, snapshot);
        }
        if self.map_viewer.is_open {
            self.map_viewer.tick(dt);
        }
        if self.memo_reader.is_open {
            self.memo_reader.tick(dt);
        }
    }
}

/// Opens the Status / Inventory screen (`Tab` / `I` / gamepad `Select`), freezing world simulation
/// while keeping room BGM active (`PatchInventoryBGM.cpp`).
pub(crate) fn open_inventory(state: &mut AppState) {
    state.clear_held_gameplay_keys();
    ensure_item_atlas_loaded(state);
    state
        .ingame_menus
        .inventory
        .open(&state.sim_core.scenario.inventory_mgr);
    emit_menu_sound(state, MenuSoundCue::Confirm, "Inventory");
}

/// Opens the Fullscreen Paper Map directly (`M` / gamepad `Triangle`) if owned and lit.
pub(crate) fn open_map_direct(state: &mut AppState, from_inventory: bool) {
    let default_map_id = if state.active_maria { 36 } else { 1 };
    let is_dark_room = state.sim_core.scenario.is_current_stage_dark();
    let flashlight_active = state.sim_core.scenario.is_flashlight_active();

    match chizu_can_read_map(
        default_map_id,
        &state.sim_core.scenario.flags,
        is_dark_room,
        flashlight_active,
    ) {
        MapReadCheck::Allowed => open_map_sheet(state, default_map_id, from_inventory),
        MapReadCheck::TooDark => {
            emit_menu_sound(state, MenuSoundCue::Cancel, "Map");
            if from_inventory {
                state
                    .ingame_menus
                    .inventory
                    .show_toast("It's too dark to read the map here.");
            }
            tracing::info!("Map blocked (EventExecChizuFail): too dark without flashlight.");
        }
        MapReadCheck::NoMapOwned => {
            if !matches!(
                state.active_launch_target,
                sh2_ui::menu::LaunchTarget::Campaign { .. }
            ) {
                open_map_sheet(state, default_map_id, from_inventory);
            } else {
                emit_menu_sound(state, MenuSoundCue::Cancel, "Map");
                if from_inventory {
                    state
                        .ingame_menus
                        .inventory
                        .show_toast("You don't have a map for this area.");
                }
            }
        }
    }
}

fn open_map_sheet(state: &mut AppState, map_id: u8, from_inventory: bool) {
    state.clear_held_gameplay_keys();
    state.ingame_menus.inventory.close();
    state.ingame_menus.map_viewer.open(map_id, from_inventory);
    ensure_map_sheet_loaded(state, map_id);
    emit_menu_sound(state, MenuSoundCue::Confirm, "Map");
}

pub(crate) fn open_memo_reader(state: &mut AppState) {
    state.ingame_menus.inventory.close();
    ensure_memo_assets_loaded(state, DEFAULT_MEMO_BACKDROP_TEX);
    state
        .ingame_menus
        .memo_reader
        .open(&state.sim_core.scenario.flags);
    emit_menu_sound(state, MenuSoundCue::Confirm, "Memo");
}

/// Handles keyboard input while any single-screen in-game menu is open.
pub(crate) fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool) {
    if !pressed {
        return;
    }

    if state.ingame_menus.options_from_inventory {
        handle_options_from_inventory_key(state, key_code);
        return;
    }

    if state.ingame_menus.map_viewer.is_open {
        let nav = key_to_nav(key_code, true, false);
        if let Some(nav) = nav {
            let return_to_inv = state.ingame_menus.map_viewer.opened_from_inventory;
            let (cue, action) = state
                .ingame_menus
                .map_viewer
                .handle_nav(nav, &state.sim_core.scenario.flags);
            emit_menu_sound(state, cue, "Map");
            match action {
                PaperMapAction::MapSheetChanged(new_id) => ensure_map_sheet_loaded(state, new_id),
                PaperMapAction::Close if return_to_inv => {
                    state
                        .ingame_menus
                        .inventory
                        .open(&state.sim_core.scenario.inventory_mgr);
                }
                _ => {}
            }
        }
        return;
    }

    if state.ingame_menus.memo_reader.is_open {
        if let Some(nav) = key_to_nav(key_code, false, false) {
            let (cue, action) = state
                .ingame_menus
                .memo_reader
                .handle_nav(nav, state.ingame_menus.memo_messages.as_ref());
            emit_menu_sound(state, cue, "Memo");
            match action {
                MemoAction::BackdropChanged(path) => ensure_memo_assets_loaded(state, path),
                MemoAction::Close => {
                    state
                        .ingame_menus
                        .inventory
                        .open(&state.sim_core.scenario.inventory_mgr);
                }
                MemoAction::None => {}
            }
        }
        return;
    }

    if state.ingame_menus.inventory.is_open {
        if key_code == KeyCode::KeyM {
            open_map_direct(state, true);
            return;
        }
        if let Some(nav) = key_to_nav(key_code, false, true) {
            let snapshot = build_player_snapshot(state);
            let (cue, action) = state.ingame_menus.inventory.handle_nav(
                nav,
                &state.sim_core.scenario.inventory_mgr,
                &snapshot,
            );
            emit_menu_sound(state, cue, "Inventory");
            apply_inventory_action(state, action);
        }
    }
}

fn key_to_nav(key_code: KeyCode, close_on_m: bool, close_on_inv: bool) -> Option<MenuNavAction> {
    match key_code {
        KeyCode::KeyW | KeyCode::ArrowUp => Some(MenuNavAction::Up),
        KeyCode::KeyS | KeyCode::ArrowDown => Some(MenuNavAction::Down),
        KeyCode::KeyA | KeyCode::ArrowLeft => Some(MenuNavAction::Left),
        KeyCode::KeyD | KeyCode::ArrowRight => Some(MenuNavAction::Right),
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => Some(MenuNavAction::Confirm),
        KeyCode::Escape | KeyCode::Backspace => Some(MenuNavAction::Cancel),
        KeyCode::KeyM if close_on_m => Some(MenuNavAction::Cancel),
        KeyCode::Tab | KeyCode::KeyI if close_on_inv => Some(MenuNavAction::Cancel),
        _ => None,
    }
}

/// Routes mouse cursor movement when Inventory is open (`CommandWindowMouseFix.cpp`).
pub(crate) fn handle_mouse_move(state: &mut AppState, mouse_x: f32, mouse_y: f32) {
    if state.ingame_menus.inventory.is_open {
        let vw = state.surface_config.width.max(1) as f32;
        let vh = state.surface_config.height.max(1) as f32;
        state
            .ingame_menus
            .inventory
            .handle_mouse_move(mouse_x, mouse_y, vw, vh);
    }
}

/// Routes left mouse click when Inventory is open.
pub(crate) fn handle_mouse_click(state: &mut AppState, mouse_x: f32, mouse_y: f32) {
    if state.ingame_menus.inventory.is_open {
        let vw = state.surface_config.width.max(1) as f32;
        let vh = state.surface_config.height.max(1) as f32;
        let snapshot = build_player_snapshot(state);
        let (cue, action) = state.ingame_menus.inventory.handle_mouse_click(
            mouse_x,
            mouse_y,
            vw,
            vh,
            &state.sim_core.scenario.inventory_mgr,
            &snapshot,
        );
        emit_menu_sound(state, cue, "Inventory");
        apply_inventory_action(state, action);
    }
}

/// Renders whichever single-screen in-game menu overlay is currently active.
pub(crate) fn render_overlay(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    if !state.ingame_menus.is_any_open() {
        return;
    }

    state.begin_hud_overlay();
    let font = AtlasFont::shared(None);

    if state.ingame_menus.options_from_inventory {
        state.inventory_rasterizer.clear();
        draw_options_screen(
            &state.menu_ctrl.options_menu,
            font,
            &mut state.inventory_rasterizer,
        );
        let vp = state.ps2_ui_viewport();
        state.finish_inventory_overlay_in_viewport(encoder, surface_view, vp);
        return;
    } else if state.ingame_menus.map_viewer.is_open {
        let cur_id = state.ingame_menus.map_viewer.current_map_id;
        let sheet = state
            .ingame_menus
            .cached_map_sheet
            .as_ref()
            .filter(|(id, _, _, _)| *id == cur_id)
            .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h));
        let marks = state
            .ingame_menus
            .cached_map_marks
            .as_ref()
            .filter(|(id, _, _, _)| *id == cur_id)
            .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h));
        let pos = state.sim_core.scenario.player_position;
        let norm_pos = chizu_world_to_sheet_norm(pos.x, pos.z);
        draw_paper_map(
            &state.ingame_menus.map_viewer,
            &state.sim_core.scenario.flags,
            norm_pos,
            state.sim_core.scenario.player_yaw,
            sheet,
            marks,
            font,
            &mut state.menu_rasterizer,
        );
    } else if state.ingame_menus.memo_reader.is_open {
        let backdrop = state
            .ingame_menus
            .cached_memo_backdrop
            .as_ref()
            .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h));
        draw_memo_reader(
            &state.ingame_menus.memo_reader,
            state.ingame_menus.memo_messages.as_ref(),
            backdrop,
            font,
            &mut state.menu_rasterizer,
        );
    } else if state.ingame_menus.inventory.is_open {
        if state.ingame_menus.inventory.focus == sh2_ui::inventory::InventoryFocus::ExamineOverlay {
            if let Some(id) = state.ingame_menus.inventory.selected_item_id() {
                ensure_examine_tex_loaded(state, id);
            }
        }
        let snapshot = build_player_snapshot(state);
        let item_atlas = state
            .ingame_menus
            .cached_item_atlas
            .as_ref()
            .map(|(w, h, rgba)| (rgba.as_slice(), *w, *h));
        let examine_tex = state
            .ingame_menus
            .inventory
            .selected_item_id()
            .and_then(|sel_id| {
                state
                    .ingame_menus
                    .cached_examine_tex
                    .as_ref()
                    .filter(|(id, _, _, _)| *id == sel_id)
                    .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h))
            });
        state.inventory_rasterizer.clear();
        draw_inventory_screen(
            &state.ingame_menus.inventory,
            &state.sim_core.scenario.inventory_mgr,
            &snapshot,
            item_atlas,
            examine_tex,
            font,
            &mut state.inventory_rasterizer,
        );
        // Blit the dedicated 4:3 (`1280x960`) inventory texture into the exact centered 4:3
        // swapchain viewport so glyphs have 1:1 square pixels and align bit-for-bit with
        // `HealthEcgState::snapshot_viewport_rect` on ALL monitors (4:3, 16:10, 16:9, 21:9, 32:9, portrait).
        let vp = sh2_ui::inventory::ps2_ui_canvas_rect(
            state.surface_config.width as f32,
            state.surface_config.height as f32,
        );
        state.finish_inventory_overlay_in_viewport(encoder, surface_view, vp);
        return;
    }

    state.finish_hud_overlay(encoder, surface_view);
}
