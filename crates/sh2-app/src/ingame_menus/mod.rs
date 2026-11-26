//! Thin runtime wiring for the 1:1 PS2 / SH2:EE Single-Screen In-Game Menus (`SH2-116`, `SH2-138`):
//! - Status & Inventory Screen (`sh2_ui::inventory`, `Item/otn_itemmain.c`)
//! - Fullscreen Paper Map (`sh2_ui::map`, `Event/chizu.c`)
//! - Collected Memo Reader (`sh2_ui::memo`, `Event/memo.c`)
//! - Inventory -> Options sub-menu transition
//!
//! Opening any of these overlays freezes 60 Hz world simulation while **preserving** ambient
//! room BGM playback (`PatchInventoryBGM.cpp` @ `0x005166e8`).

mod actions;
mod save_load;

pub(crate) use actions::{
    apply_inventory_action, build_player_snapshot, ensure_gameover_tex_loaded,
    ensure_red_cross_tex_loaded, ensure_savebg_loaded,
};
use actions::{
    ensure_examine_tex_loaded, ensure_item_atlas_loaded, ensure_map_sheet_loaded,
    ensure_memo_assets_loaded, handle_options_from_inventory_key,
};
pub(crate) use save_load::open_save_load;
use save_load::{
    handle_save_load_key, handle_save_load_mouse_click, handle_save_load_mouse_move,
    handle_save_load_mouse_wheel, handle_save_load_right_click,
};

use winit::keyboard::KeyCode;

use crate::menu_events::emit_menu_sound;
use crate::state::AppState;
use sh2_assets::formats::mes::MessageTable;
use sh2_ui::inventory::{
    draw_inventory_screen, InventoryAction, InventoryPlayerSnapshot, InventoryScreen,
};
use sh2_ui::map::{
    chizu_can_read_map, chizu_current_position_check, chizu_possession_check, chizu_select,
    draw_paper_map, stage_to_default_room_id, MapReadCheck, PaperMapAction, PaperMapPlayerState,
    PaperMapTextures, PaperMapViewer, CHIZU_PAPER_SE_ID,
};
use sh2_ui::memo::{
    draw_memo_reader, MemoAction, MemoReader, MemoRenderContext, MemoRenderTextures,
    DEFAULT_MEMO_BACKDROP_TEX,
};
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
    /// Held directional keys `[Up, Down, Left, Right]` for continuous `307.2/384.0` px/s map panning.
    pub held_map_pan: [bool; 4],
    /// True when Options was opened via the Inventory `OPTION` corner button.
    pub options_from_inventory: bool,
    /// Options subscreen fade-in / fade-out scrim alpha (`1.0` = solid black, `0.0` = visible;
    /// `SLUS_202.28` `otn_option.c:2038, 2056` `ScreenEffectFadeStart(0x3f800000, 4/1)` = `1.0` s).
    pub options_fade_alpha: f32,
    /// True while Options opened from Inventory is fading to black before returning to Inventory.
    pub options_closing_to_inventory: bool,
    /// Whether `memo_msg_e.mes` has been attempted to load from disk.
    pub(crate) memo_mes_loaded: bool,
    /// Cached parsed `memo_msg_e.mes` / `memo_x_msg_e.mes` message table.
    pub(crate) memo_messages: Option<MessageTable>,
    /// Cached decoded map sheet `(map_id, width, height, rgba)`.
    pub(crate) cached_map_sheet: Option<(u8, usize, usize, Vec<u8>)>,
    /// Cached decoded map marker atlas `(map_id, width, height, rgba)`.
    pub(crate) cached_map_marks: Option<(u8, usize, usize, Vec<u8>)>,
    /// Cached decoded `pic/etc/carsol.tex` (`256x128` connect arrows + player heading arrow).
    pub(crate) cached_map_cursor: Option<(usize, usize, Vec<u8>)>,
    /// Cached decoded memo backdrop `(rel_path, width, height, rgba)`.
    pub(crate) cached_memo_backdrop: Option<(&'static str, usize, usize, Vec<u8>)>,
    /// Cached decoded memo puzzle overlay layer `(rel_path, width, height, rgba)`.
    pub(crate) cached_memo_layer: Option<(&'static str, usize, usize, Vec<u8>)>,
    /// Whether `pic/etc/itemmenu2.tex` has been attempted to load from disk.
    pub(crate) item_atlas_loaded: bool,
    /// Cached decoded `itemmenu2.tex` (`4096x4096` SH2:EE HD or `1024x1024` PS2/PC) `(width, height, rgba)`.
    pub(crate) cached_item_atlas: Option<(usize, usize, Vec<u8>)>,
    /// Cached decoded close-up examine `.tex` `(item_id, coin_side, width, height, rgba)`.
    pub(crate) cached_examine_tex: Option<(u8, u8, usize, usize, Vec<u8>)>,
    /// Game Over screen state (`GameoverMain` `0x001c4c70`).
    pub gameover: sh2_ui::GameOverScreen,
    /// Low-Health Red Cross Icon state (`sh2pc.exe` `0x00476280`).
    pub red_cross: sh2_ui::RedCrossIconState,
    /// Save/Load screen state (`MC/mc_menu.c`).
    pub save_load: Option<sh2_ui::SaveLoadScreen>,
    pub(crate) gameover_tex_loaded: bool,
    pub(crate) cached_gameover_tex: Option<(usize, usize, Vec<u8>)>,
    pub(crate) red_cross_tex_loaded: bool,
    pub(crate) cached_red_cross_tex: Option<(usize, usize, Vec<u8>)>,
    pub(crate) savebg_tex_loaded: Option<sh2_core::scenario::ScenarioKind>,
    pub(crate) cached_savebg_tex: Option<(usize, usize, Vec<u8>)>,
}

impl InGameMenuState {
    /// Returns `true` if Inventory, Paper Map, Memo Reader, Save/Load, or Inventory->Options is open.
    pub fn is_any_open(&self) -> bool {
        self.inventory.is_open
            || self.map_viewer.is_open
            || self.memo_reader.is_open
            || self.options_from_inventory
            || self.save_load.is_some()
    }

    /// Closes all single-screen in-game menus cleanly (e.g. when returning to Main Menu).
    pub fn close_all(&mut self) {
        self.inventory.close();
        self.map_viewer.close();
        self.memo_reader.close();
        self.held_map_pan = [false; 4];
        self.options_from_inventory = false;
        self.options_closing_to_inventory = false;
        self.options_fade_alpha = 0.0;
        self.save_load = None;
    }

    /// Advances active menu animations (Health ECG sweep/pulse, carousel rotation, map zoom/pan/transcription, memo dim, save/load blink).
    ///
    /// Returns any deferred [`InventoryAction`] triggered when an Inventory fade-to-black transition
    /// finishes (`otn_itemmain.c` `item_fade_out` `0x001db0e0`).
    pub fn tick(
        &mut self,
        dt: f32,
        snapshot: &InventoryPlayerSnapshot,
    ) -> (Option<InventoryAction>, bool) {
        let deferred_action = if self.inventory.is_open {
            self.inventory.tick(dt, snapshot)
        } else {
            None
        };
        if self.map_viewer.is_open {
            let axis_x = (self.held_map_pan[3] as i32 - self.held_map_pan[2] as i32) as f32;
            let axis_y = (self.held_map_pan[1] as i32 - self.held_map_pan[0] as i32) as f32;
            if axis_x != 0.0 || axis_y != 0.0 {
                self.map_viewer.pan_continuous(axis_x, axis_y, dt);
            }
            self.map_viewer.tick(dt);
        }
        if self.memo_reader.is_open {
            self.memo_reader.tick(dt);
        }
        let mut return_to_inventory = false;
        if self.options_from_inventory {
            let dt_clamped = dt.clamp(0.0, 0.1);
            if self.options_closing_to_inventory {
                // `otn_option.c:2056` (`fade_out`): `1.0` s fade to black before returning to Inventory
                self.options_fade_alpha = (self.options_fade_alpha + dt_clamped).min(1.0);
                if self.options_fade_alpha >= 1.0 {
                    self.options_closing_to_inventory = false;
                    self.options_from_inventory = false;
                    return_to_inventory = true;
                }
            } else if self.options_fade_alpha > 0.0 {
                // `otn_option.c:2038` (`fade_in`): `1.0` s fade in from black
                self.options_fade_alpha = (self.options_fade_alpha - dt_clamped).max(0.0);
            }
        }
        if let Some(ref mut sl) = self.save_load {
            sl.tick(dt);
        }
        (deferred_action, return_to_inventory)
    }
}

/// Emits `SeCall(1.0, 0x2b17)` paper rustle SFX (`ChizuMain` `0x001c6e8c` / `sh2pc.exe` `0x0049e560`).
fn play_map_paper_sfx(state: &mut AppState) {
    if let (Some(ref mut eng), Some(ref bank)) = (&mut state.audio_engine, &state.audio_bank) {
        let _ = bank.play_se_id(eng, CHIZU_PAPER_SE_ID, 1.0);
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

/// Opens the Fullscreen Paper Map directly (`M` / gamepad `Triangle`) using `ChizuSelect`
/// (`0x001c7040` / `sh2pc.exe` `0x0049ac70`) if owned and lit.
pub(crate) fn open_map_direct(state: &mut AppState, from_inventory: bool) {
    let pos = state.sim_core.scenario.player_position;
    let room_id =
        stage_to_default_room_id(state.sim_core.scenario.current_stage, state.active_maria);
    let selected_map_id = chizu_select(
        room_id,
        [pos.x, pos.y, pos.z],
        &state.sim_core.scenario.flags,
    );
    let is_dark_room = state.sim_core.scenario.is_current_stage_dark();
    let flashlight_active = state.sim_core.scenario.is_flashlight_active();

    // Exact ChizuMain (`0x001c6b90` lines 48-54) fallback: if current area map is unowned,
    // fall back to Map 1 (Town Overall) when flag 24 is set.
    let target_map_id = if selected_map_id != 0
        && chizu_possession_check(selected_map_id, &state.sim_core.scenario.flags)
    {
        selected_map_id
    } else if state.sim_core.scenario.flags.get(24) {
        1
    } else if selected_map_id != 0 {
        selected_map_id
    } else if state.active_maria {
        36
    } else {
        1
    };

    match chizu_can_read_map(
        target_map_id,
        &state.sim_core.scenario.flags,
        is_dark_room,
        flashlight_active,
    ) {
        MapReadCheck::Allowed => open_map_sheet(
            state,
            target_map_id,
            selected_map_id,
            room_id,
            from_inventory,
        ),
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
                open_map_sheet(
                    state,
                    target_map_id,
                    selected_map_id,
                    room_id,
                    from_inventory,
                );
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

fn open_map_sheet(
    state: &mut AppState,
    map_id: u8,
    selected_map_id: u8,
    room_id: usize,
    from_inventory: bool,
) {
    state.clear_held_gameplay_keys();
    state.ingame_menus.held_map_pan = [false; 4];
    state.ingame_menus.inventory.close();
    let pos = state.sim_core.scenario.player_position;
    let player_ps2 = chizu_current_position_check(
        room_id,
        map_id,
        [pos.x, pos.y, pos.z],
        &state.sim_core.scenario.flags,
    );
    state.ingame_menus.map_viewer.open_at_player(
        map_id,
        selected_map_id,
        Some(player_ps2),
        state.active_maria,
        from_inventory,
    );
    state.ingame_menus.map_viewer.start_fade_in();
    ensure_map_sheet_loaded(state, map_id);
    play_map_paper_sfx(state);
}

pub(crate) fn open_memo_reader(state: &mut AppState) {
    state.ingame_menus.inventory.close();
    ensure_memo_assets_loaded(state, DEFAULT_MEMO_BACKDROP_TEX, None);
    state
        .ingame_menus
        .memo_reader
        .open(&state.sim_core.scenario.flags);
    state.ingame_menus.memo_reader.start_fade_in();
    emit_menu_sound(state, MenuSoundCue::Confirm, "Memo");
}

/// Handles keyboard input while any single-screen in-game menu is open.
pub(crate) fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool) {
    // Track held directional keys for continuous paper map panning (`ChizuControl` 307.2/384.0 px/s)
    if state.ingame_menus.map_viewer.is_open {
        match key_code {
            KeyCode::KeyW | KeyCode::ArrowUp => state.ingame_menus.held_map_pan[0] = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => state.ingame_menus.held_map_pan[1] = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => state.ingame_menus.held_map_pan[2] = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => state.ingame_menus.held_map_pan[3] = pressed,
            _ => {}
        }
    }

    if !pressed {
        return;
    }

    if state.ingame_menus.options_from_inventory {
        handle_options_from_inventory_key(state, key_code);
        return;
    }

    if state.ingame_menus.save_load.is_some() {
        handle_save_load_key(state, key_code);
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
                PaperMapAction::MapSheetChanged(new_id) => {
                    ensure_map_sheet_loaded(state, new_id);
                    play_map_paper_sfx(state);
                }
                PaperMapAction::Close => {
                    state.ingame_menus.held_map_pan = [false; 4];
                    if return_to_inv {
                        state
                            .ingame_menus
                            .inventory
                            .open(&state.sim_core.scenario.inventory_mgr);
                    } else if let Some(ref mut orch) = state.campaign_orchestrator {
                        // `gamemain.c:448` (`ScreenEffectFadeStart(0x3f99999a, 4)`): fade gameplay back in
                        orch.fade.start_fade_in(
                            sh2_core::campaign::PS2_DEFAULT_FADE_SPEED,
                            orch.delayed_fade_in_fix,
                        );
                    }
                }
                PaperMapAction::None => {}
            }
        }
        return;
    }

    if state.ingame_menus.memo_reader.is_open {
        if let Some(nav) = key_to_nav(key_code, false, false) {
            let (cue, action) = state.ingame_menus.memo_reader.handle_nav(
                nav,
                &state.sim_core.scenario.flags,
                state.ingame_menus.memo_messages.as_ref(),
            );
            emit_menu_sound(state, cue, "Memo");
            match action {
                MemoAction::TexturesChanged {
                    backdrop_path,
                    layer_path,
                } => ensure_memo_assets_loaded(state, backdrop_path, layer_path),
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

/// Routes mouse cursor movement when Inventory or Inventory->Options is open (`CommandWindowMouseFix.cpp`).
pub(crate) fn handle_mouse_move(state: &mut AppState, mouse_x: f32, mouse_y: f32) {
    let vw = state.surface_config.width.max(1) as f32;
    let vh = state.surface_config.height.max(1) as f32;
    if state.ingame_menus.options_from_inventory && !state.ingame_menus.options_closing_to_inventory
    {
        let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(vw, vh);
        if cw > 0.0 && ch > 0.0 {
            let ps2_x = (mouse_x - ox) * 512.0 / cw;
            let ps2_y = (mouse_y - oy) * 448.0 / ch;
            if state.menu_ctrl.options_menu.handle_mouse_move(ps2_x, ps2_y) {
                state.menu_ctrl.revision = state.menu_ctrl.revision.wrapping_add(1);
                emit_menu_sound(state, MenuSoundCue::CursorMove, "Inventory Options");
            }
        }
        return;
    }
    if state.ingame_menus.save_load.is_some() {
        handle_save_load_mouse_move(state, mouse_x, mouse_y, vw, vh);
        return;
    }
    if state.ingame_menus.inventory.is_open {
        state
            .ingame_menus
            .inventory
            .handle_mouse_move(mouse_x, mouse_y, vw, vh);
    }
}

/// Routes left mouse click when Inventory, Inventory->Options, or Save/Load is open.
pub(crate) fn handle_mouse_click(state: &mut AppState, mouse_x: f32, mouse_y: f32) {
    let vw = state.surface_config.width.max(1) as f32;
    let vh = state.surface_config.height.max(1) as f32;
    if state.ingame_menus.options_from_inventory && !state.ingame_menus.options_closing_to_inventory
    {
        let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(vw, vh);
        if cw > 0.0 && ch > 0.0 {
            let ps2_x = (mouse_x - ox) * 512.0 / cw;
            let ps2_y = (mouse_y - oy) * 448.0 / ch;
            if let Some(cue) = state
                .menu_ctrl
                .options_menu
                .handle_mouse_click(ps2_x, ps2_y)
            {
                let (saved_cfg, _) = state.menu_ctrl.options_menu.commit_changes();
                state.menu_ctrl.config_applied_pending = Some(saved_cfg.clone());
                let _ = saved_cfg.save_to_file(&state.menu_ctrl.config_path);
                crate::menu_events::sync_audio_config(state, &saved_cfg);
                if cue == sh2_ui::options::OptionsSoundCue::ValueChange {
                    actions::play_options_volume_or_cursor_sfx(state);
                } else {
                    emit_menu_sound(state, MenuSoundCue::Confirm, "Inventory Options");
                }
            }
        }
        return;
    }
    if state.ingame_menus.save_load.is_some() {
        handle_save_load_mouse_click(state, mouse_x, mouse_y, vw, vh);
        return;
    }
    if state.ingame_menus.inventory.is_open {
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

/// Routes right mouse click (Cancel / Back) when Save/Load is open.
pub(crate) fn handle_mouse_right_click(state: &mut AppState) {
    if state.ingame_menus.save_load.is_some() {
        handle_save_load_right_click(state);
    }
}

/// Routes mouse scroll wheel when Inventory->Options or Save/Load is open.
pub(crate) fn handle_mouse_wheel(state: &mut AppState, steps: i32) {
    if steps == 0 {
        return;
    }
    if state.ingame_menus.save_load.is_some() {
        handle_save_load_mouse_wheel(state, steps);
        return;
    }
    if !state.ingame_menus.options_from_inventory || state.ingame_menus.options_closing_to_inventory
    {
        return;
    }
    let vw = state.surface_config.width.max(1) as f32;
    let vh = state.surface_config.height.max(1) as f32;
    let hover_ps2 = state.last_cursor_pos.and_then(|(mx, my)| {
        let (ox, oy, cw, ch) = sh2_ui::inventory::ps2_ui_canvas_rect(vw, vh);
        if cw > 0.0 && ch > 0.0 {
            Some(((mx as f32 - ox) * 512.0 / cw, (my as f32 - oy) * 448.0 / ch))
        } else {
            None
        }
    });
    let cue = state
        .menu_ctrl
        .options_menu
        .handle_mouse_wheel(steps, hover_ps2);
    if cue != sh2_ui::options::OptionsSoundCue::None {
        state.menu_ctrl.revision = state.menu_ctrl.revision.wrapping_add(1);
        emit_menu_sound(state, MenuSoundCue::CursorMove, "Inventory Options");
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
        // `otn_option.c` (`0x002170c0`): solid black fullscreen backdrop + native widescreen
        // Options UI (`menu_rasterizer`) with full-viewport `1.0` s fade-in scrim.
        state.draw_fullscreen_dim_scrim(encoder, surface_view, 255);
        draw_options_screen(
            &state.menu_ctrl.options_menu,
            font,
            &mut state.menu_rasterizer,
        );
        if let Some((mx, my)) = state.last_cursor_pos {
            let norm_x = (mx as f32) / state.surface_config.width.max(1) as f32;
            let norm_y = (my as f32) / state.surface_config.height.max(1) as f32;
            sh2_ui::menu::draw_pc_mouse_cursor(&mut state.menu_rasterizer, norm_x, norm_y);
        }
        if state.ingame_menus.options_fade_alpha > 0.001 {
            let a = (state.ingame_menus.options_fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            let rw = state.menu_rasterizer.width;
            let rh = state.menu_rasterizer.height;
            state.menu_rasterizer.fill_rect(0, 0, rw, rh, [0, 0, 0, a]);
        }
        state.finish_hud_overlay(encoder, surface_view);
        return;
    } else if state.ingame_menus.save_load.is_some() {
        let scenario = state.menu_ctrl.title_sm.selected_scenario;
        ensure_savebg_loaded(state, scenario);
        let bg = state
            .ingame_menus
            .cached_savebg_tex
            .as_ref()
            .map(|(w, h, rgba)| (rgba.as_slice(), *w, *h));
        if let Some(ref sl) = state.ingame_menus.save_load {
            sh2_ui::draw_save_load_screen(sl, bg, font, &mut state.menu_rasterizer);
        }
        if let Some((mx, my)) = state.last_cursor_pos {
            let norm_x = (mx as f32) / state.surface_config.width.max(1) as f32;
            let norm_y = (my as f32) / state.surface_config.height.max(1) as f32;
            sh2_ui::menu::draw_pc_mouse_cursor(&mut state.menu_rasterizer, norm_x, norm_y);
        }
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
        let cursor = state
            .ingame_menus
            .cached_map_cursor
            .as_ref()
            .map(|(w, h, rgba)| (rgba.as_slice(), *w, *h));
        let pos = state.sim_core.scenario.player_position;
        let room_id =
            stage_to_default_room_id(state.sim_core.scenario.current_stage, state.active_maria);
        let pos_ps2 = chizu_current_position_check(
            room_id,
            cur_id,
            [pos.x, pos.y, pos.z],
            &state.sim_core.scenario.flags,
        );
        draw_paper_map(
            &state.ingame_menus.map_viewer,
            &state.sim_core.scenario.flags,
            PaperMapPlayerState {
                room_id,
                pos_ps2,
                yaw_rad: state.sim_core.scenario.player_yaw,
            },
            PaperMapTextures {
                sheet_rgba: sheet,
                mark_atlas_rgba: marks,
                cursor_rgba: cursor,
            },
            &mut state.menu_rasterizer,
        );
    } else if state.ingame_menus.memo_reader.is_open {
        let backdrop = state
            .ingame_menus
            .cached_memo_backdrop
            .as_ref()
            .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h));
        let layer = state
            .ingame_menus
            .cached_memo_layer
            .as_ref()
            .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h));
        draw_memo_reader(
            &state.ingame_menus.memo_reader,
            state.ingame_menus.memo_messages.as_ref(),
            MemoRenderTextures {
                backdrop_rgba: backdrop,
                layer_rgba: layer,
            },
            Some(MemoRenderContext {
                flags: &state.sim_core.scenario.flags,
                puzzle: &state.sim_core.scenario.puzzle_state,
                riddle: state.sim_core.scenario.riddle_difficulty,
            }),
            font,
            &mut state.menu_rasterizer,
        );
    } else if state.ingame_menus.inventory.is_open {
        let coin_side = state.ingame_menus.inventory.examine_coin_side;
        if state.ingame_menus.inventory.focus == sh2_ui::inventory::InventoryFocus::ExamineOverlay {
            if let Some(id) = state.ingame_menus.inventory.selected_item_id() {
                ensure_examine_tex_loaded(state, id, coin_side);
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
                    .filter(|(id, side, _, _, _)| *id == sel_id && *side == coin_side)
                    .map(|(_, _, w, h, rgba)| (rgba.as_slice(), *w, *h))
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
        let vp = sh2_ui::inventory::ps2_ui_canvas_rect(
            state.surface_config.width as f32,
            state.surface_config.height as f32,
        );
        if let Some((mx, my)) = state.last_cursor_pos {
            let norm_x = (mx as f32 - vp.0) / vp.2.max(1.0);
            let norm_y = (my as f32 - vp.1) / vp.3.max(1.0);
            sh2_ui::menu::draw_pc_mouse_cursor(&mut state.inventory_rasterizer, norm_x, norm_y);
        }
        state.finish_inventory_overlay_in_viewport(encoder, surface_view, vp);
        return;
    }

    state.finish_hud_overlay(encoder, surface_view);
}
