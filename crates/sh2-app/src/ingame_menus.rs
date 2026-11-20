//! Thin runtime wiring for the 1:1 PS2 / SH2:EE Single-Screen In-Game Menus (`SH2-116`):
//! - Status & Inventory Screen (`sh2_ui::inventory`, `Item/otn_itemmain.c`)
//! - Fullscreen Paper Map (`sh2_ui::map`, `Event/chizu.c`)
//! - Collected Memo Reader (`sh2_ui::memo`, `Event/memo.c`)
//! - Inventory -> Options sub-menu transition
//!
//! Opening any of these overlays freezes 60 Hz world simulation while **preserving** ambient
//! room BGM playback (`PatchInventoryBGM.cpp` @ `0x005166e8`).

use winit::keyboard::KeyCode;

use crate::menu_events::{emit_menu_sound, sync_audio_config};
use crate::state::AppState;
use sh2_assets::formats::mes::{parse_mes, MessageTable};
use sh2_assets::{AssetPathResolver, TextureLoader};
use sh2_core::combat::WeaponGrip;
use sh2_ui::inventory::{
    draw_inventory_screen, InventoryAction, InventoryPlayerSnapshot, InventoryScreen,
};
use sh2_ui::map::{
    chizu_can_read_map, draw_paper_map, map_spec, MapReadCheck, PaperMapAction, PaperMapViewer,
};
use sh2_ui::memo::{
    draw_memo_reader, memo_command_check, MemoAction, MemoReader, DEFAULT_MEMO_BACKDROP_TEX,
};
use sh2_ui::menu::{MenuNavAction, MenuSoundCue};
use sh2_ui::options::{OptionsMenu, OptionsSoundCue};
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
    memo_mes_loaded: bool,
    /// Cached parsed `memo_msg_e.mes` / `memo_x_msg_e.mes` message table.
    memo_messages: Option<MessageTable>,
    /// Cached decoded map sheet `(map_id, width, height, rgba)`.
    cached_map_sheet: Option<(u8, usize, usize, Vec<u8>)>,
    /// Cached decoded memo backdrop `(rel_path, width, height, rgba)`.
    cached_memo_backdrop: Option<(&'static str, usize, usize, Vec<u8>)>,
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

/// Builds a fresh [`InventoryPlayerSnapshot`] from `AppState`.
pub(crate) fn build_player_snapshot(state: &AppState) -> InventoryPlayerSnapshot {
    let weapon_item_id = weapon_grip_to_item_id(state.combat_arena.player.weapon);
    let (mag_rounds, mag_cap, reserve_ammo) = match state.combat_arena.player.weapon {
        WeaponGrip::Handgun => (
            Some(state.combat_arena.player.loaded_ammo()),
            Some(WeaponGrip::Handgun.magazine_capacity()),
            Some(state.sim_core.scenario.inventory_mgr.count(5)),
        ),
        WeaponGrip::Shotgun => (
            Some(state.combat_arena.player.loaded_ammo()),
            Some(WeaponGrip::Shotgun.magazine_capacity()),
            Some(state.sim_core.scenario.inventory_mgr.count(7)),
        ),
        WeaponGrip::HuntingRifle => (
            Some(state.combat_arena.player.loaded_ammo()),
            Some(WeaponGrip::HuntingRifle.magazine_capacity()),
            Some(state.sim_core.scenario.inventory_mgr.count(9)),
        ),
        WeaponGrip::MariaRevolver => (
            Some(state.combat_arena.player.loaded_ammo()),
            Some(WeaponGrip::MariaRevolver.magazine_capacity()),
            Some(state.sim_core.scenario.inventory_mgr.count(11)),
        ),
        _ => (None, None, None),
    };

    InventoryPlayerSnapshot {
        hp: state.sim_core.scenario.health,
        max_hp: state.sim_core.scenario.max_health,
        equipped_weapon_item_id: weapon_item_id,
        equipped_magazine_rounds: mag_rounds,
        equipped_magazine_capacity: mag_cap,
        equipped_reserve_ammo: reserve_ammo,
        flashlight_on: state.sim_core.scenario.is_flashlight_active(),
        radio_on: true,
        has_any_memo: memo_command_check(&state.sim_core.scenario.flags),
    }
}

/// Opens the Status / Inventory screen (`Tab` / `I` / gamepad `Select`), freezing world simulation
/// while keeping room BGM active (`PatchInventoryBGM.cpp`).
pub(crate) fn open_inventory(state: &mut AppState) {
    state.clear_held_gameplay_keys();
    state
        .ingame_menus
        .inventory
        .open(&state.sim_core.scenario.inventory_mgr);
    emit_menu_sound(state, MenuSoundCue::Confirm, "Inventory");
    tracing::info!("Opened Single-Screen Status / Inventory Menu (otn_itemmain.c).");
}

/// Opens the Fullscreen Paper Map directly (`M` / gamepad `Triangle`) if owned and lit.
pub(crate) fn open_map_direct(state: &mut AppState, from_inventory: bool) {
    let default_map_id = if state.active_maria { 36 } else { 1 };
    let is_dark_room = false;
    let flashlight_active = state.sim_core.scenario.is_flashlight_active();

    // Ensure Town Map (flag 24) is checkable; in sandbox/debug rooms allow fallback viewing if unlocked
    match chizu_can_read_map(
        default_map_id,
        &state.sim_core.scenario.flags,
        is_dark_room,
        flashlight_active,
    ) {
        MapReadCheck::Allowed => {
            open_map_sheet(state, default_map_id, from_inventory);
        }
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
            // In non-campaign sandbox rooms, still allow opening sheet 1 if player explicitly presses M
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

fn open_memo_reader(state: &mut AppState) {
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

    // 1. Options sub-screen opened from Inventory -> OPTION
    if state.ingame_menus.options_from_inventory {
        handle_options_from_inventory_key(state, key_code);
        return;
    }

    // 2. Fullscreen Paper Map (`Event/chizu.c`)
    if state.ingame_menus.map_viewer.is_open {
        let nav = match key_code {
            KeyCode::KeyW | KeyCode::ArrowUp => Some(MenuNavAction::Up),
            KeyCode::KeyS | KeyCode::ArrowDown => Some(MenuNavAction::Down),
            KeyCode::KeyA | KeyCode::ArrowLeft => Some(MenuNavAction::Left),
            KeyCode::KeyD | KeyCode::ArrowRight => Some(MenuNavAction::Right),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => Some(MenuNavAction::Confirm),
            KeyCode::Escape | KeyCode::Backspace | KeyCode::KeyM => Some(MenuNavAction::Cancel),
            _ => None,
        };
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

    // 3. Memo Reader (`Event/memo.c`)
    if state.ingame_menus.memo_reader.is_open {
        let nav = match key_code {
            KeyCode::KeyW | KeyCode::ArrowUp => Some(MenuNavAction::Up),
            KeyCode::KeyS | KeyCode::ArrowDown => Some(MenuNavAction::Down),
            KeyCode::KeyA | KeyCode::ArrowLeft => Some(MenuNavAction::Left),
            KeyCode::KeyD | KeyCode::ArrowRight => Some(MenuNavAction::Right),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => Some(MenuNavAction::Confirm),
            KeyCode::Escape | KeyCode::Backspace => Some(MenuNavAction::Cancel),
            _ => None,
        };
        if let Some(nav) = nav {
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

    // 4. Status / Inventory Screen (`Item/otn_itemmain.c`)
    if state.ingame_menus.inventory.is_open {
        let nav = match key_code {
            KeyCode::KeyW | KeyCode::ArrowUp => Some(MenuNavAction::Up),
            KeyCode::KeyS | KeyCode::ArrowDown => Some(MenuNavAction::Down),
            KeyCode::KeyA | KeyCode::ArrowLeft => Some(MenuNavAction::Left),
            KeyCode::KeyD | KeyCode::ArrowRight => Some(MenuNavAction::Right),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => Some(MenuNavAction::Confirm),
            KeyCode::Escape | KeyCode::Backspace | KeyCode::Tab | KeyCode::KeyI => {
                Some(MenuNavAction::Cancel)
            }
            KeyCode::KeyM => {
                open_map_direct(state, true);
                return;
            }
            _ => None,
        };
        if let Some(nav) = nav {
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
        draw_options_screen(
            &state.menu_ctrl.options_menu,
            font,
            &mut state.menu_rasterizer,
        );
    } else if state.ingame_menus.map_viewer.is_open {
        let sheet = state
            .ingame_menus
            .cached_map_sheet
            .as_ref()
            .filter(|(id, _, _, _)| *id == state.ingame_menus.map_viewer.current_map_id)
            .map(|(_, w, h, rgba)| (rgba.as_slice(), *w, *h));
        let pos = state.sim_core.scenario.player_position;
        // Map normalized player position gently around center until per-stage bounds are wired
        let norm_x = (0.5 + pos.x / 60_000.0).clamp(0.1, 0.9);
        let norm_y = (0.5 + pos.z / 60_000.0).clamp(0.1, 0.9);
        draw_paper_map(
            &state.ingame_menus.map_viewer,
            &state.sim_core.scenario.flags,
            (norm_x, norm_y),
            state.sim_core.scenario.player_yaw,
            sheet,
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
        let snapshot = build_player_snapshot(state);
        draw_inventory_screen(
            &state.ingame_menus.inventory,
            &state.sim_core.scenario.inventory_mgr,
            &snapshot,
            false,
            font,
            &mut state.menu_rasterizer,
        );
    }

    state.finish_hud_overlay(encoder, surface_view);
}

fn handle_options_from_inventory_key(state: &mut AppState, key_code: KeyCode) {
    let sound = match key_code {
        KeyCode::KeyW | KeyCode::ArrowUp => {
            state.menu_ctrl.options_menu.navigate_vertical(-1);
            MenuSoundCue::CursorMove
        }
        KeyCode::KeyS | KeyCode::ArrowDown => {
            state.menu_ctrl.options_menu.navigate_vertical(1);
            MenuSoundCue::CursorMove
        }
        KeyCode::KeyA | KeyCode::ArrowLeft => {
            state.menu_ctrl.options_menu.adjust_value(-1);
            MenuSoundCue::CursorMove
        }
        KeyCode::KeyD | KeyCode::ArrowRight => {
            state.menu_ctrl.options_menu.adjust_value(1);
            MenuSoundCue::CursorMove
        }
        KeyCode::KeyQ => {
            state.menu_ctrl.options_menu.switch_tab(false);
            MenuSoundCue::CursorMove
        }
        KeyCode::KeyE => {
            state.menu_ctrl.options_menu.switch_tab(true);
            MenuSoundCue::CursorMove
        }
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
            let (saved, cue) = state.menu_ctrl.options_menu.commit_changes();
            state.menu_ctrl.config_applied_pending = Some(saved.clone());
            sync_audio_config(state, &saved);
            let _ = saved.save_to_file(&state.menu_ctrl.config_path);
            if cue == OptionsSoundCue::ConfirmSave {
                MenuSoundCue::Confirm
            } else {
                MenuSoundCue::None
            }
        }
        KeyCode::Escape | KeyCode::Backspace => {
            state.menu_ctrl.options_menu.discard_changes();
            state.ingame_menus.options_from_inventory = false;
            state
                .ingame_menus
                .inventory
                .open(&state.sim_core.scenario.inventory_mgr);
            MenuSoundCue::Cancel
        }
        _ => MenuSoundCue::None,
    };
    emit_menu_sound(state, sound, "Inventory Options");
}

fn apply_inventory_action(state: &mut AppState, action: InventoryAction) {
    match action {
        InventoryAction::None | InventoryAction::Close => {}
        InventoryAction::OpenMap => open_map_direct(state, true),
        InventoryAction::OpenMemo => open_memo_reader(state),
        InventoryAction::OpenOptions => {
            state.ingame_menus.inventory.close();
            let fresh_devices = sh2_audio::AudioEngine::enumerate_output_devices();
            state.menu_ctrl.options_menu = OptionsMenu::new_with_audio_packs(
                state.menu_ctrl.options_menu.original_config.clone(),
                state.menu_ctrl.audio_pack_status,
            );
            state
                .menu_ctrl
                .options_menu
                .set_audio_devices(fresh_devices);
            state.ingame_menus.options_from_inventory = true;
        }
        InventoryAction::UseHealing(item_id) => {
            let hp = state.sim_core.scenario.health;
            let max_hp = state.sim_core.scenario.max_health;
            if let Some(res) = state
                .sim_core
                .scenario
                .inventory_mgr
                .use_healing_item(item_id, hp, max_hp)
            {
                state.sim_core.scenario.health = res.new_hp;
                state.sim_core.scenario.sync_legacy_views();
                state
                    .ingame_menus
                    .inventory
                    .refresh_owned_items(&state.sim_core.scenario.inventory_mgr);
                state.ingame_menus.inventory.show_toast(format!(
                    "Recovered health ({:.0}/{:.0}).",
                    res.new_hp, max_hp
                ));
            }
        }
        InventoryAction::EquipWeapon(item_id) => {
            if let Some(grip) = item_id_to_weapon_grip(item_id) {
                state.combat_arena.player.equip_weapon(grip);
                state
                    .ingame_menus
                    .inventory
                    .show_toast(format!("Equipped {:?}.", grip));
            }
        }
        InventoryAction::UnequipWeapon => {
            state.combat_arena.player.equip_weapon(WeaponGrip::None);
            state
                .ingame_menus
                .inventory
                .show_toast("Unequipped weapon.");
        }
        InventoryAction::ReloadFirearm {
            ammo_id,
            firearm_id,
        } => {
            if let Some(grip) = item_id_to_weapon_grip(firearm_id) {
                let prev_weapon = state.combat_arena.player.weapon;
                state.combat_arena.player.equip_weapon(grip);
                let avail = state.sim_core.scenario.inventory_mgr.count(ammo_id) as u32;
                let cap = grip.magazine_capacity();
                let loaded = state.combat_arena.player.loaded_ammo();
                let need = cap.saturating_sub(loaded).min(avail);
                if need > 0 {
                    state
                        .sim_core
                        .scenario
                        .inventory_mgr
                        .give_raw(ammo_id, -(need as i32));
                    let transferred = state.combat_arena.player.execute_reload();
                    if transferred == 0 {
                        // Fallback if combat reserve was empty: sync reserve then execute_reload
                        state
                            .ingame_menus
                            .inventory
                            .show_toast(format!("Loaded {} rounds.", need));
                    } else {
                        state
                            .ingame_menus
                            .inventory
                            .show_toast(format!("Reloaded {} rounds.", transferred));
                    }
                    state.sim_core.scenario.sync_legacy_views();
                    state
                        .ingame_menus
                        .inventory
                        .refresh_owned_items(&state.sim_core.scenario.inventory_mgr);
                } else {
                    state
                        .ingame_menus
                        .inventory
                        .show_toast("Magazine is already full.");
                }
                if prev_weapon != grip && prev_weapon != WeaponGrip::None {
                    state.combat_arena.player.equip_weapon(prev_weapon);
                }
            }
        }
        InventoryAction::ToggleFlashlight => {
            let active = state.sim_core.scenario.toggle_flashlight();
            state.ingame_menus.inventory.show_toast(if active {
                "Flashlight switched ON."
            } else {
                "Flashlight switched OFF."
            });
        }
        InventoryAction::ToggleRadio => {
            state
                .ingame_menus
                .inventory
                .show_toast("Radio state toggled.");
        }
        InventoryAction::UseKeyOrPuzzleItem(_item_id) => {
            state
                .ingame_menus
                .inventory
                .show_toast("Can't use this here.");
        }
        InventoryAction::CombinedItems { result_id, .. } => {
            state
                .ingame_menus
                .inventory
                .show_toast(format!("Combined items into combo #{result_id}."));
        }
    }
}

fn ensure_map_sheet_loaded(state: &mut AppState, map_id: u8) {
    if state
        .ingame_menus
        .cached_map_sheet
        .as_ref()
        .is_some_and(|(id, _, _, _)| *id == map_id)
    {
        return;
    }
    let Some(spec) = map_spec(map_id) else {
        return;
    };
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(tex) = loader.load_texture(spec.base_tex_path) {
        state.ingame_menus.cached_map_sheet = Some((map_id, tex.width, tex.height, tex.rgba));
    }
}

fn ensure_memo_assets_loaded(state: &mut AppState, backdrop_path: &'static str) {
    let resolver = AssetPathResolver::resolve();
    if !state.ingame_menus.memo_mes_loaded {
        state.ingame_menus.memo_mes_loaded = true;
        let candidates = [
            resolver.data_dir.join("etc/message/memo_x_msg_e.mes"),
            resolver.data_dir.join("etc/message/memo_msg_e.mes"),
        ];
        for path in candidates {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(table) = parse_mes(&bytes) {
                    state.ingame_menus.memo_messages = Some(table);
                    break;
                }
            }
        }
    }
    if state
        .ingame_menus
        .cached_memo_backdrop
        .as_ref()
        .is_some_and(|(p, _, _, _)| *p == backdrop_path)
    {
        return;
    }
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(tex) = loader
        .load_texture(backdrop_path)
        .or_else(|_| loader.load_texture(DEFAULT_MEMO_BACKDROP_TEX))
    {
        state.ingame_menus.cached_memo_backdrop =
            Some((backdrop_path, tex.width, tex.height, tex.rgba));
    }
}

fn weapon_grip_to_item_id(grip: WeaponGrip) -> Option<u8> {
    match grip {
        WeaponGrip::None => None,
        WeaponGrip::Handgun => Some(4),
        WeaponGrip::Shotgun => Some(6),
        WeaponGrip::HuntingRifle => Some(8),
        WeaponGrip::WoodenPlank => Some(12),
        WeaponGrip::SteelPipe => Some(13),
        WeaponGrip::Chainsaw => Some(14),
        WeaponGrip::GreatKnife => Some(15),
        WeaponGrip::HyperSpray => Some(16),
        WeaponGrip::MariaRevolver => Some(80),
        WeaponGrip::MariaChineseCleaver => Some(79),
    }
}

fn item_id_to_weapon_grip(item_id: u8) -> Option<WeaponGrip> {
    match item_id {
        4 => Some(WeaponGrip::Handgun),
        6 => Some(WeaponGrip::Shotgun),
        8 => Some(WeaponGrip::HuntingRifle),
        12 => Some(WeaponGrip::WoodenPlank),
        13 => Some(WeaponGrip::SteelPipe),
        14 => Some(WeaponGrip::Chainsaw),
        15 => Some(WeaponGrip::GreatKnife),
        16 => Some(WeaponGrip::HyperSpray),
        79 => Some(WeaponGrip::MariaChineseCleaver),
        80 => Some(WeaponGrip::MariaRevolver),
        _ => None,
    }
}
