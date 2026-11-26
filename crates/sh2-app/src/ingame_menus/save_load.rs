//! Multi-slot Save/Load screen orchestration (`MC/mc_menu.c`, `SH2-144`).

use winit::keyboard::KeyCode;

use super::actions::ensure_savebg_loaded;
use super::key_to_nav;
use crate::menu_events::emit_menu_sound;
use crate::state::AppState;
use sh2_ui::menu::MenuSoundCue;

/// Opens the multi-slot `SaveLoadScreen` (`MC/mc_menu.c`) in `mode` (`Save` or `Load`).
pub(crate) fn open_save_load(state: &mut AppState, mode: sh2_ui::SaveLoadMode) {
    state.clear_held_gameplay_keys();
    let scenario = state.menu_ctrl.title_sm.selected_scenario;
    ensure_savebg_loaded(state, scenario);
    let g = &state.menu_ctrl.options_menu.original_config.game;
    let cfg = sh2_ui::SaveLoadConfig {
        unlimited_slots: g.enable_unlimited_save_slots,
        quicksave_autosave: g.enable_quicksave_autosave,
        save_thumbnails: g.enable_save_thumbnails,
    };
    let mut sl = sh2_ui::SaveLoadScreen::new(mode, scenario, cfg);
    let repo = sh2_core::campaign::SaveSlotRepository::new(&state.menu_ctrl.save_dir);
    sl.set_slots(repo.scan_slots(cfg.unlimited_slots, cfg.quicksave_autosave));
    state.ingame_menus.save_load = Some(sl);
    emit_menu_sound(state, MenuSoundCue::Confirm, "SaveLoad");
}

fn build_campaign_save_snapshot(
    state: &AppState,
    save_dir: &std::path::Path,
    prev_count: u32,
) -> sh2_core::campaign::CampaignSaveSnapshot {
    let clear_profile = sh2_core::campaign::ClearDataProfile::load_from_dir(save_dir);
    let play_time_secs = state
        .campaign_orchestrator
        .as_ref()
        .map_or(state.menu_ctrl.pause_menu.play_time_secs, |o| {
            o.play_time_secs
        });
    sh2_core::campaign::CampaignSaveSnapshot {
        scenario: state.menu_ctrl.title_sm.selected_scenario,
        stage_id: state.sim_core.scenario.current_stage.max(2),
        room_id: state.sim_core.scenario.current_room.clone(),
        player_position: state.sim_core.scenario.player_position.to_array(),
        player_yaw: state.sim_core.scenario.player_yaw,
        action_difficulty: state.menu_ctrl.title_sm.action_difficulty,
        riddle_difficulty: state.menu_ctrl.title_sm.riddle_difficulty,
        play_time_secs,
        save_count: prev_count.saturating_add(1),
        save_version: sh2_core::campaign::SAVE_FORMAT_VERSION,
        save_point_name_id: 0,
        secondary_timer_secs: 0.0,
        player_hp: state.sim_core.scenario.health,
        player_max_hp: state.sim_core.scenario.max_health,
        clear_data: clear_profile.clear_data,
        previous_total_rank: clear_profile.previous_total_rank,
        spray_pow: clear_profile.spray_pow,
        event_flags_words: state.sim_core.scenario.flags.to_words(),
        inventory_items: state.sim_core.scenario.inventory_mgr.owned_items(),
        thumbnail_rgba: None,
    }
}

fn apply_loaded_snapshot(
    state: &mut AppState,
    snapshot: &sh2_core::campaign::CampaignSaveSnapshot,
) {
    state.sim_core.scenario.current_stage = snapshot.stage_id;
    state.sim_core.scenario.current_room = snapshot.room_id.clone();
    state.sim_core.scenario.player_position = glam::Vec3::from_array(snapshot.player_position);
    state.sim_core.scenario.player_yaw = snapshot.player_yaw;
    state.sim_core.scenario.health = snapshot.player_hp;
    state.sim_core.scenario.max_health = snapshot.player_max_hp;
    if !snapshot.event_flags_words.is_empty() {
        state
            .sim_core
            .scenario
            .flags
            .from_words(&snapshot.event_flags_words);
    }
    if !snapshot.inventory_items.is_empty() {
        state
            .sim_core
            .scenario
            .inventory_mgr
            .restore_items(&snapshot.inventory_items);
    }
    state.menu_ctrl.title_sm.selected_scenario = snapshot.scenario;
    state.menu_ctrl.title_sm.action_difficulty = snapshot.action_difficulty;
    state.menu_ctrl.title_sm.riddle_difficulty = snapshot.riddle_difficulty;
    state.menu_ctrl.pause_menu.play_time_secs = snapshot.play_time_secs;
    state.ingame_menus.save_load = None;
    let target = sh2_ui::menu::LaunchTarget::Campaign {
        scenario: snapshot.scenario,
        is_continue: true,
    };
    state.menu_ctrl.title_sm.launch_target = target;
    state.active_launch_target = target;
    if !state.menu_ctrl.in_game_active {
        state.menu_ctrl.in_game_active = true;
    }
    if let Some(ref mut engine) = state.audio_engine {
        engine.set_bgm_paused(false);
    }
    if let Some(ref mut bank) = state.audio_bank {
        bank.set_spatial_handles_paused(false);
    }
    crate::rooms::campaign_room::CampaignRoomController::start_session(state, true);
    if let Some(ref mut orch) = state.campaign_orchestrator {
        orch.play_time_secs = snapshot.play_time_secs;
    }
    state.menu_ctrl.revision = state.menu_ctrl.revision.wrapping_add(1);
}

pub(super) fn handle_save_load_key(state: &mut AppState, key_code: KeyCode) {
    let nav = match key_code {
        KeyCode::Delete | KeyCode::KeyX => {
            if let Some(ref mut sl) = state.ingame_menus.save_load {
                sl.request_delete();
            }
            return;
        }
        KeyCode::Tab => {
            if let Some(ref mut sl) = state.ingame_menus.save_load {
                sl.cycle_add_info();
            }
            return;
        }
        KeyCode::KeyQ => {
            if let Some(ref mut sl) = state.ingame_menus.save_load {
                sl.switch_card(0);
            }
            return;
        }
        KeyCode::KeyE => {
            if let Some(ref mut sl) = state.ingame_menus.save_load {
                sl.switch_card(1);
            }
            return;
        }
        _ => key_to_nav(key_code, false, false),
    };
    let Some(nav) = nav else {
        return;
    };
    let action = {
        let Some(ref mut sl) = state.ingame_menus.save_load else {
            return;
        };
        sl.handle_nav(nav)
    };
    apply_save_load_action(state, action);
}

pub(super) fn handle_save_load_mouse_move(
    state: &mut AppState,
    mouse_x: f32,
    mouse_y: f32,
    vw: f32,
    vh: f32,
) {
    let moved = state
        .ingame_menus
        .save_load
        .as_mut()
        .is_some_and(|sl| sl.handle_mouse_move(mouse_x, mouse_y, vw, vh));
    if moved {
        emit_menu_sound(state, MenuSoundCue::CursorMove, "SaveLoad");
    }
}

pub(super) fn handle_save_load_mouse_click(
    state: &mut AppState,
    mouse_x: f32,
    mouse_y: f32,
    vw: f32,
    vh: f32,
) {
    let (changed, action) = match state.ingame_menus.save_load.as_mut() {
        Some(sl) => sl.handle_mouse_click(mouse_x, mouse_y, vw, vh),
        None => return,
    };
    if changed && action == sh2_ui::SaveLoadAction::None {
        emit_menu_sound(state, MenuSoundCue::CursorMove, "SaveLoad");
    }
    apply_save_load_action(state, action);
}

pub(super) fn handle_save_load_right_click(state: &mut AppState) {
    let action = match state.ingame_menus.save_load.as_mut() {
        Some(sl) => sl.handle_nav(sh2_ui::menu::MenuNavAction::Cancel),
        None => return,
    };
    emit_menu_sound(state, MenuSoundCue::Cancel, "SaveLoad");
    apply_save_load_action(state, action);
}

pub(super) fn handle_save_load_mouse_wheel(state: &mut AppState, steps: i32) {
    let moved = state
        .ingame_menus
        .save_load
        .as_mut()
        .is_some_and(|sl| sl.handle_mouse_wheel(steps));
    if moved {
        emit_menu_sound(state, MenuSoundCue::CursorMove, "SaveLoad");
    }
}

fn apply_save_load_action(state: &mut AppState, action: sh2_ui::SaveLoadAction) {
    let save_dir = state.menu_ctrl.save_dir.clone();
    let repo = sh2_core::campaign::SaveSlotRepository::new(&save_dir);
    let g = &state.menu_ctrl.options_menu.original_config.game;
    let (unlimited, quick_auto) = (g.enable_unlimited_save_slots, g.enable_quicksave_autosave);

    match action {
        sh2_ui::SaveLoadAction::WriteSlot {
            card_index,
            file_index,
            kind,
        } => {
            let prev_count = state
                .ingame_menus
                .save_load
                .as_ref()
                .and_then(|sl| {
                    sl.slots
                        .iter()
                        .find(|s| s.card_index == card_index && s.file_index == file_index)
                        .map(|s| s.snapshot.save_count)
                })
                .unwrap_or(0);
            let snapshot = build_campaign_save_snapshot(state, &save_dir, prev_count);
            if repo
                .write_slot(card_index, file_index, kind, &snapshot)
                .is_ok()
            {
                state.sim_core.scenario.playing_info.savecount = state
                    .sim_core
                    .scenario
                    .playing_info
                    .savecount
                    .saturating_add(1);
                state
                    .menu_ctrl
                    .title_sm
                    .set_save_availability(sh2_ui::menu::SaveAvailability::LoadAndContinue);
                if let Some(ref mut sl) = state.ingame_menus.save_load {
                    sl.set_slots(repo.scan_slots(unlimited, quick_auto));
                    sl.status_message = Some("GAME SAVED".to_string());
                }
                emit_menu_sound(state, MenuSoundCue::Confirm, "SaveLoad");
            }
        }
        sh2_ui::SaveLoadAction::LoadSlot {
            card_index,
            file_index,
            kind,
        } => {
            let entries = repo.scan_slots(unlimited, quick_auto);
            if let Some(entry) = entries.into_iter().find(|e| {
                e.card_index == card_index && e.file_index == file_index && e.kind == kind
            }) {
                if !entry.is_corrupted {
                    apply_loaded_snapshot(state, &entry.snapshot);
                    emit_menu_sound(state, MenuSoundCue::Confirm, "SaveLoad");
                }
            }
        }
        sh2_ui::SaveLoadAction::DeleteSlot {
            card_index,
            file_index,
        } => {
            if repo.delete_slot(card_index, file_index).is_ok() {
                if let Some(ref mut sl) = state.ingame_menus.save_load {
                    sl.set_slots(repo.scan_slots(unlimited, quick_auto));
                }
                emit_menu_sound(state, MenuSoundCue::Confirm, "SaveLoad");
            }
        }
        sh2_ui::SaveLoadAction::Close => {
            state.ingame_menus.save_load = None;
            if !state.menu_ctrl.in_game_active
                && state.menu_ctrl.title_sm.phase == sh2_ui::menu::TitlePhase::LoadMenu
            {
                state
                    .menu_ctrl
                    .title_sm
                    .show_phase_immediately(sh2_ui::menu::TitlePhase::MainMenu);
            }
            if let Some(ref mut eng) = state.audio_engine {
                eng.set_bgm_paused(false);
            }
            if let Some(ref mut bank) = state.audio_bank {
                bank.set_spatial_handles_paused(false);
            }
            if let Some(ref mut orch) = state.campaign_orchestrator {
                if matches!(
                    orch.phase,
                    sh2_core::campaign::CampaignPhase::OverlayMenu(
                        sh2_core::campaign::OverlayMenuKind::Save
                    )
                ) {
                    orch.phase = sh2_core::campaign::CampaignPhase::ActiveGameplay;
                }
            }
            emit_menu_sound(state, MenuSoundCue::Cancel, "SaveLoad");
        }
        sh2_ui::SaveLoadAction::None
        | sh2_ui::SaveLoadAction::ExportJson { .. }
        | sh2_ui::SaveLoadAction::ExportBinary { .. } => {}
    }
}
