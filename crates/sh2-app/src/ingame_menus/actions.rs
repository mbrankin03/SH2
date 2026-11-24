//! Action dispatch & lazy retail asset loaders for [`super::InGameMenuState`].

use winit::keyboard::KeyCode;

use crate::ingame_menus::{open_map_direct, open_memo_reader};
use crate::menu_events::{emit_menu_sound, sync_audio_config};
use crate::state::AppState;
use sh2_assets::formats::mes::parse_mes;
use sh2_assets::{AssetPathResolver, TextureLoader};
use sh2_core::combat::{CombatController, WeaponGrip};
use sh2_ui::inventory::{InventoryAction, InventoryPlayerSnapshot};
use sh2_ui::map::{map_spec, CHIZU_CURSOR_TEX_PATH};
use sh2_ui::memo::{memo_command_check, DEFAULT_MEMO_BACKDROP_TEX};
use sh2_ui::menu::MenuSoundCue;
use sh2_ui::options::{OptionsMenu, OptionsSoundCue};

/// Builds a fresh [`InventoryPlayerSnapshot`] from `AppState`.
pub(crate) fn build_player_snapshot(state: &AppState) -> InventoryPlayerSnapshot {
    let weapon_item_id = CombatController::weapon_grip_to_item_id(state.combat_arena.player.weapon);
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
        radio_on: state.sim_core.scenario.radio_on,
        radio_volume: state.ingame_menus.inventory.radio_volume,
        has_any_memo: memo_command_check(&state.sim_core.scenario.flags),
    }
}

pub(crate) fn handle_options_from_inventory_key(state: &mut AppState, key_code: KeyCode) {
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

pub(crate) fn apply_inventory_action(state: &mut AppState, action: InventoryAction) {
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
            if let Some(grip) = CombatController::weapon_grip_from_item_id(item_id) {
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
            if let Some(grip) = CombatController::weapon_grip_from_item_id(firearm_id) {
                let avail = state.sim_core.scenario.inventory_mgr.count(ammo_id) as u32;
                let loaded = state.combat_arena.player.load_magazine_rounds(grip, avail);
                if loaded > 0 {
                    state
                        .sim_core
                        .scenario
                        .inventory_mgr
                        .give_raw(ammo_id, -(loaded as i32));
                    state.sim_core.scenario.sync_legacy_views();
                    state
                        .ingame_menus
                        .inventory
                        .refresh_owned_items(&state.sim_core.scenario.inventory_mgr);
                    state
                        .ingame_menus
                        .inventory
                        .show_toast(format!("Reloaded {} rounds.", loaded));
                } else {
                    state
                        .ingame_menus
                        .inventory
                        .show_toast("Magazine is already full.");
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
            let active = state.sim_core.scenario.toggle_radio();
            state.ingame_menus.inventory.show_toast(if active {
                "Radio switched ON."
            } else {
                "Radio switched OFF."
            });
        }
        InventoryAction::SetRadioVolume(vol) => {
            state.ingame_menus.inventory.radio_volume = vol.min(15);
        }
        InventoryAction::UseKeyOrPuzzleItem(_item_id) => {
            state
                .ingame_menus
                .inventory
                .show_toast("Can't use this here.");
        }
        InventoryAction::CombinedItems { result_id, .. } => {
            if state.sim_core.scenario.inventory_mgr.has_item(result_id) {
                state.ingame_menus.inventory.active_combo_id = Some(result_id);
                state
                    .ingame_menus
                    .inventory
                    .show_toast(format!("Combined items ready (combo #{result_id})."));
            }
        }
    }
}

pub(crate) fn ensure_map_sheet_loaded(state: &mut AppState, map_id: u8) {
    let Some(spec) = map_spec(map_id) else {
        return;
    };
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());

    if !state
        .ingame_menus
        .cached_map_sheet
        .as_ref()
        .is_some_and(|(id, _, _, _)| *id == map_id)
    {
        if let Ok(tex) = loader.load_texture(spec.base_tex_path) {
            state.ingame_menus.cached_map_sheet = Some((map_id, tex.width, tex.height, tex.rgba));
        }
    }
    if !state
        .ingame_menus
        .cached_map_marks
        .as_ref()
        .is_some_and(|(id, _, _, _)| *id == map_id)
    {
        if let Ok(tex) = loader.load_texture(spec.mark_tex_path) {
            state.ingame_menus.cached_map_marks = Some((map_id, tex.width, tex.height, tex.rgba));
        }
    }
    if state.ingame_menus.cached_map_cursor.is_none() {
        if let Ok(tex) = loader.load_texture(CHIZU_CURSOR_TEX_PATH) {
            state.ingame_menus.cached_map_cursor = Some((tex.width, tex.height, tex.rgba));
        }
    }
}

pub(crate) fn ensure_memo_assets_loaded(
    state: &mut AppState,
    backdrop_path: &'static str,
    layer_path: Option<&'static str>,
) {
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
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if !state
        .ingame_menus
        .cached_memo_backdrop
        .as_ref()
        .is_some_and(|(p, _, _, _)| *p == backdrop_path)
    {
        if let Ok(tex) = loader
            .load_texture(backdrop_path)
            .or_else(|_| loader.load_texture(DEFAULT_MEMO_BACKDROP_TEX))
        {
            state.ingame_menus.cached_memo_backdrop =
                Some((backdrop_path, tex.width, tex.height, tex.rgba));
        }
    }
    match layer_path {
        Some(lp) => {
            if !state
                .ingame_menus
                .cached_memo_layer
                .as_ref()
                .is_some_and(|(p, _, _, _)| *p == lp)
            {
                if let Ok(tex) = loader.load_texture(lp) {
                    state.ingame_menus.cached_memo_layer =
                        Some((lp, tex.width, tex.height, tex.rgba));
                }
            }
        }
        None => {
            state.ingame_menus.cached_memo_layer = None;
        }
    }
}

/// Lazily loads `pic/etc/itemmenu2.tex` (automatically preferring 4K HD `sh2e/pic/etc/itemmenu2.tex`
/// over original `data/pic/etc/itemmenu2.tex`) for the 1:1 PS2 `TgsItemPitureDraw` item carousel.
pub(crate) fn ensure_item_atlas_loaded(state: &mut AppState) {
    if state.ingame_menus.item_atlas_loaded {
        return;
    }
    state.ingame_menus.item_atlas_loaded = true;
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(tex) = loader.load_texture("pic/etc/itemmenu2.tex") {
        tracing::info!(
            "Loaded authentic itemmenu2.tex sprite atlas ({}x{}).",
            tex.width,
            tex.height
        );
        state.ingame_menus.cached_item_atlas = Some((tex.width, tex.height, tex.rgba));
    }
}

/// Lazily loads the close-up 2D inspection still (`meta.inspect_texture_path`) when `Look` is opened.
pub(crate) fn ensure_examine_tex_loaded(state: &mut AppState, item_id: u8) {
    if state
        .ingame_menus
        .cached_examine_tex
        .as_ref()
        .is_some_and(|(id, _, _, _)| *id == item_id)
    {
        return;
    }
    let Some(tex_path) = sh2_core::item_metadata(item_id).and_then(|m| m.inspect_texture_path)
    else {
        return;
    };
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(cut) = loader.load_cutaway_texture(tex_path, true) {
        state.ingame_menus.cached_examine_tex = Some((
            item_id,
            cut.texture.width,
            cut.texture.height,
            cut.texture.rgba,
        ));
    }
}

/// Lazily loads `pic/etc/gameover1.tex` for `GameoverMain` (`0x001c4c70` / `0x00497ad0`).
pub(crate) fn ensure_gameover_tex_loaded(state: &mut AppState) {
    if state.ingame_menus.gameover_tex_loaded {
        return;
    }
    state.ingame_menus.gameover_tex_loaded = true;
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(tex) = loader.load_texture(sh2_ui::GAMEOVER_TEX_PATH) {
        state.ingame_menus.cached_gameover_tex = Some((tex.width, tex.height, tex.rgba));
    }
}

/// Lazily loads `pic/etc/RedCrossIcon.png` (`sh2pc.exe` `0x004f6380`).
pub(crate) fn ensure_red_cross_tex_loaded(state: &mut AppState) {
    if state.ingame_menus.red_cross_tex_loaded {
        return;
    }
    state.ingame_menus.red_cross_tex_loaded = true;
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(tex) = loader.load_texture(sh2_ui::RED_CROSS_ICON_PATH) {
        state.ingame_menus.cached_red_cross_tex = Some((tex.width, tex.height, tex.rgba));
    }
}

/// Lazily loads `menu/mc/savebg.tbn2` (Main) or `menu/mc/savebg2.tbn2` (Born from a Wish, `0x0044b3e0`).
pub(crate) fn ensure_savebg_loaded(
    state: &mut AppState,
    scenario: sh2_core::scenario::ScenarioKind,
) {
    if state.ingame_menus.savebg_tex_loaded == Some(scenario) {
        return;
    }
    state.ingame_menus.savebg_tex_loaded = Some(scenario);
    let path = match scenario {
        sh2_core::scenario::ScenarioKind::LetterFromSilentHeaven => sh2_ui::SAVEBG_TBN2_PATH,
        sh2_core::scenario::ScenarioKind::BornFromAWish => sh2_ui::SAVEBG2_TBN2_PATH,
    };
    let resolver = AssetPathResolver::resolve();
    let loader = TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
    if let Ok(tex) = loader.load_texture(path) {
        state.ingame_menus.cached_savebg_tex = Some((tex.width, tex.height, tex.rgba));
    }
}
