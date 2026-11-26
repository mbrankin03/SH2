//! Keyboard input routing for 3D Story Progression Checkpoints (`[PgUp/PgDn]`), Proximity Door/Cutaway
//! Triggers (`[E/Enter]`), NG+ Secret Weapons (`[7/8]`), UFO Blue Gem (`[U]`), and Ending Judge (`[L]`) (`SH2-65`).

use winit::keyboard::KeyCode;

use super::story_progression_3d::{
    configure_checkpoint_tab_and_puzzle, STORY_3D_CHECKPOINTS, STORY_3D_CHECKPOINT_COUNT,
};
use super::story_stage_loader::load_story_checkpoint_3d_stage;
use crate::rooms::story_progression_room::{
    ensure_demo_scenario_seeded, ensure_ufo_texture_cached,
    input::{handle_endings_key, handle_ufo_letter_key, reroll_riddle_difficulty},
    input_act1::handle_act1_key,
    input_act2::handle_act2_key,
    play_confirm_se, play_cursor_se,
};
use crate::state::AppState;
use sh2_core::endings::{use_blue_gem, CHAINSAW_ATTACK_SPECS, FLAG_SPAWN_BLUE_GEM, ROOM_HOTEL_312};
use sh2_ui::menu::TestRoomKind;

/// Intercepts 3D story progression keys (`[PgUp/PgDn]`, `[E/Enter]`, `[7/8/U/L]`, and modal controls).
/// Returns `true` if the key event was consumed by the 3D story progression layer.
pub(crate) fn handle_3d_story_key_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) -> bool {
    ensure_demo_scenario_seeded(state);

    // 1. If a 3D cutaway/UFO/Ending modal is open, route input to the modal first
    if state.story_progression_room.modal_3d_open {
        if !pressed || repeat {
            return true;
        }
        return handle_open_modal_key(state, key_code);
    }

    if !pressed || repeat {
        return false;
    }

    // 2. Step through the 12 Story Stage Checkpoints with [PgUp] / [PgDn] from anywhere in 3D gameplay
    if matches!(key_code, KeyCode::PageDown | KeyCode::PageUp) {
        let step = if key_code == KeyCode::PageDown {
            1
        } else {
            STORY_3D_CHECKPOINT_COUNT - 1
        };
        let next_idx =
            (state.story_progression_room.checkpoint_3d_idx + step) % STORY_3D_CHECKPOINT_COUNT;
        apply_3d_checkpoint_selection(state, next_idx);
        return true;
    }

    // 3. Global 3D NG+ Secret Weapon & Ending shortcuts ([7] Chainsaw, [8] Hyper Spray, [U] Blue Gem UFO, [L] Ending Judge & Mary's Letter)
    match key_code {
        KeyCode::Digit7 => {
            trigger_3d_chainsaw_action(state);
            return true;
        }
        KeyCode::Digit8 => {
            trigger_3d_hyper_spray_action(state);
            return true;
        }
        KeyCode::KeyU => {
            trigger_3d_blue_gem_ufo_modal(state);
            return true;
        }
        KeyCode::KeyL if !state.key_look_right => {
            apply_3d_checkpoint_selection(state, 11);
            open_checkpoint_modal(state, 11);
            return true;
        }
        KeyCode::KeyE | KeyCode::Enter | KeyCode::NumpadEnter => {
            trigger_active_checkpoint_action(state);
            return true;
        }
        _ => {}
    }

    // Checkpoint 8 (`Stage 40/41` key doors) direct 3D hotkeys (`[K]` toggle key, `[A]` action unlock, `[[ / ]]` cycle door)
    let cp_idx = state.story_progression_room.checkpoint_3d_idx % STORY_3D_CHECKPOINT_COUNT;
    if cp_idx == 7 && matches!(key_code, KeyCode::BracketLeft | KeyCode::BracketRight) {
        handle_act2_key(state, key_code);
        return true;
    }

    false
}

fn close_story_modal(state: &mut AppState) {
    state.story_progression_room.modal_3d_open = false;
    let flags = state.sim_core.scenario.flags.clone();
    state
        .sim_core
        .scenario
        .cutaway_overlay
        .set_picture(None, &flags);
}

fn apply_3d_checkpoint_selection(state: &mut AppState, next_idx: usize) {
    state.story_progression_room.checkpoint_3d_idx = next_idx;
    close_story_modal(state);
    let spec = &STORY_3D_CHECKPOINTS[next_idx];
    configure_checkpoint_tab_and_puzzle(state, next_idx);

    if state.sim_core.scenario.current_room != spec.room_id
        || state.active_test_room != spec.target_room
    {
        load_story_checkpoint_3d_stage(state, next_idx);
    }
    state.story_progression_room.status_banner = format!(
        "Checkpoint [{:02}/12] {}: {} — {}",
        next_idx + 1,
        spec.stage_badge,
        spec.title,
        spec.prompt_hint
    );
    play_cursor_se(state);
}

fn trigger_active_checkpoint_action(state: &mut AppState) {
    if state.active_test_room == TestRoomKind::RestroomStart
        && state.story_progression_room.checkpoint_3d_idx == 0
    {
        let p_pos = state.sim_core.scenario.player_position;
        let near_mirror =
            p_pos.x < -19250.0 && !state.story_progression_room.restroom_mirror_inspected;
        if near_mirror {
            state.story_progression_room.restroom_mirror_inspected = true;
            state.story_progression_room.status_banner =
                "\"James stares into the dirty restroom mirror...\" Press [E / Enter] to Exit Restroom Door -> Observation Deck (ob01.map)!".to_string();
            play_confirm_se(state);
            return;
        }
        state.story_progression_room.restroom_mirror_inspected = true;
        apply_3d_checkpoint_selection(state, 1);
        state.story_progression_room.status_banner =
            "Exiting Restroom (er01.map) -> Observation Deck (ob01.map) [Stage 3 stg_ovservation]! Press [E] at Blue Car to inspect Map!".to_string();
        if let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_ref()) {
            let _ = bank.play_se_id(eng, 0x2b17, 0.95);
        } else {
            play_confirm_se(state);
        }
        return;
    }

    let cp_idx = state.story_progression_room.checkpoint_3d_idx % STORY_3D_CHECKPOINT_COUNT;
    if cp_idx == 0 {
        apply_3d_checkpoint_selection(state, 1);
        state.story_progression_room.status_banner =
            "Exiting Restroom (er01.map) -> Observation Deck (ob01.map) [Stage 3 stg_ovservation]!"
                .to_string();
        play_confirm_se(state);
        return;
    }

    if cp_idx == 1 {
        if state.sim_core.scenario.flags.get(28) {
            apply_3d_checkpoint_selection(state, 2);
            open_checkpoint_modal(state, 2);
            return;
        }
        open_checkpoint_modal(state, 1);
        return;
    }

    open_checkpoint_modal(state, cp_idx);
}

fn open_checkpoint_modal(state: &mut AppState, cp_idx: usize) {
    state.clear_held_gameplay_keys();
    state.story_progression_room.modal_3d_open = true;
    configure_checkpoint_tab_and_puzzle(state, cp_idx);

    match cp_idx {
        1 => {
            state
                .sim_core
                .scenario
                .cutaway_overlay
                .set_picture(Some("out_p_incar"), &state.sim_core.scenario.flags);
            state.sim_core.scenario.cutaway_overlay.set_incar_alpha(
                if state.sim_core.scenario.flags.get(28) {
                    0.0
                } else {
                    1.0
                },
            );
            state.story_progression_room.status_banner =
                "Stage 3 Blue Car Seat (out_p_incar.tex): Press [E / Enter / Space] to Take Map of Silent Hill (Flag 28, SE 0x2b17)!".to_string();
        }
        2 => {
            state.story_progression_room.status_banner =
                "Opened Stage 7/11 Safe Dial Cutaway (apt_p_safe_close.tex): [Left/Right] Rotate Dial | [Space] Step Target | [Esc/E] Return to 3D".to_string();
        }
        3 => {
            state.story_progression_room.status_banner =
                "Opened Stage 10 Coin Desk Cutaway (apt_p_desk_coin.tex): [Left/Right] Slot | [Z/X/C] Coin | [V] Vanilla vs SH2:EE | [Esc/E] 3D".to_string();
        }
        4 => {
            state.story_progression_room.status_banner =
                "Opened Stage 19/20 S14 Louise Box (hsp_p_box.tex): [Up/Down] Wheel | [Left/Right] Digit | [B] Wall Blood | [P] Roof PH | [Esc] 3D".to_string();
        }
        5 => {
            state.story_progression_room.status_banner =
                "Opened Stage 25/27 Trick Chest Quiz (hsp_p_quiz.tex): [Up/Down] Row | [Left/Right] Answer | [Enter] Submit | [P] Poison | [Esc/E] 3D".to_string();
        }
        6 => {
            state.story_progression_room.status_banner =
                "Opened Stage 31/34 Labyrinth Face Cube & Gallows: [Arrows] Rotate Cube | [Z/X] Noose | [Space] 3 Tablets | [Esc/E] 3D".to_string();
        }
        7 => {
            state.story_progression_room.status_banner =
                "Opened Stage 40/41 Hotel Briefcase & 9 trg==10 Key-Doors: [Arrows] Drum | [ [/] ] Door | [K] Key | [A] Action Unlock | [Esc/E] 3D".to_string();
        }
        8 => {
            state.story_progression_room.status_banner =
                "Opened Stage 42/45 Hotel VCR, Egg Doors & Cupboard: [Z/X] Eggs | [C] VCR | [M] Music Box | [W] Cupboard Weight | [Esc/E] 3D".to_string();
        }
        9 => {
            state.story_progression_room.status_banner =
                "Opened Stage 55 Baldwin Mansion Gravestone (add_p_teien.tex): [Z/X/C] Rotate Boards | [V] 0x008c4670 vs Fix | [Space] Solve | [Esc/E] 3D".to_string();
        }
        10 => {
            let tex_path = state
                .story_progression_room
                .ufo_player
                .current_texture_path();
            ensure_ufo_texture_cached(&mut state.story_progression_room, tex_path);
            ensure_ufo_texture_cached(&mut state.story_progression_room, "data/pic/ufo/hj07.tex");
            state.story_progression_room.status_banner =
                "Opened 35-Slide Blue Gem UFO Ending Comic (ufo/hj01..07.tex): [Left/Right] Slide | [Space] Pause | [Esc/E] Return to 3D".to_string();
        }
        _ => {
            let tex_path = state
                .story_progression_room
                .ufo_player
                .current_texture_path();
            ensure_ufo_texture_cached(&mut state.story_progression_room, tex_path);
            state.story_progression_room.status_banner =
                "Opened Live Ending Judge (EvProgEndingJudge), 10-Star Rank & Mary's 16.5px/s Letter: [1..9] Ending Actions | [PgUp/PgDn] Ending | [Esc/E] 3D".to_string();
        }
    }
    play_confirm_se(state);
}

fn handle_open_modal_key(state: &mut AppState, key_code: KeyCode) -> bool {
    let cp_idx = state.story_progression_room.checkpoint_3d_idx % STORY_3D_CHECKPOINT_COUNT;

    if key_code == KeyCode::Escape || (key_code == KeyCode::KeyE && cp_idx != 1 && cp_idx != 4) {
        close_story_modal(state);
        state.story_progression_room.status_banner =
            "Closed Story Cutaway Overlay -> Returned to 3D Walking! ([PgUp/PgDn] Step Checkpoints, [E] Open Cutaway)".to_string();
        play_cursor_se(state);
        return true;
    }

    if key_code == KeyCode::KeyR {
        reroll_riddle_difficulty(state);
        return true;
    }
    if key_code == KeyCode::KeyT {
        state.story_progression_room.prefer_sh2ee_cutaways =
            !state.story_progression_room.prefer_sh2ee_cutaways;
        play_confirm_se(state);
        return true;
    }

    match cp_idx {
        1 => {
            if matches!(
                key_code,
                KeyCode::KeyE | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space
            ) {
                if !state.sim_core.scenario.flags.get(28) {
                    state.sim_core.scenario.flags.set(28, true);
                    state.sim_core.scenario.cutaway_overlay.set_incar_alpha(0.0);
                    if let (Some(eng), Some(bank)) =
                        (state.audio_engine.as_mut(), state.audio_bank.as_ref())
                    {
                        let _ = bank.play_se_id(eng, 0x2b17, 0.95);
                    } else {
                        play_confirm_se(state);
                    }
                    state.story_progression_room.status_banner =
                        "Got the Map of Silent Hill! (Flag 28 set, SE 0x2b17) — Press [E / Enter / Esc] to close & advance!".to_string();
                } else {
                    close_story_modal(state);
                    state.story_progression_room.status_banner =
                        "Map of Silent Hill in inventory (Flag 28)! Press [E] at Gate or [PgDn] for next Story Checkpoint!".to_string();
                    play_cursor_se(state);
                }
            } else if key_code == KeyCode::KeyS {
                state.sim_core.scenario.flags.set(28, false);
                state.sim_core.scenario.cutaway_overlay.set_incar_alpha(1.0);
                state.story_progression_room.status_banner =
                    "Reset Flag 28 (Map returned to Blue Car seat)".to_string();
                play_cursor_se(state);
            }
            true
        }
        2..=5 => {
            handle_act1_key(state, key_code);
            true
        }
        6..=9 => {
            handle_act2_key(state, key_code);
            true
        }
        10 => {
            if key_code == KeyCode::Digit7 {
                trigger_3d_chainsaw_action(state);
            } else if key_code == KeyCode::Digit8 {
                trigger_3d_hyper_spray_action(state);
            } else if key_code == KeyCode::KeyU {
                trigger_3d_blue_gem_ufo_modal(state);
            } else {
                handle_ufo_letter_key(state, key_code);
            }
            true
        }
        _ => {
            if matches!(
                key_code,
                KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Space
                    | KeyCode::Enter
                    | KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
            ) {
                handle_ufo_letter_key(state, key_code);
            } else {
                handle_endings_key(state, key_code);
            }
            true
        }
    }
}

fn trigger_3d_chainsaw_action(state: &mut AppState) {
    state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::Chainsaw;
    apply_3d_checkpoint_selection(state, 10);
    handle_endings_key(state, KeyCode::KeyC);
    let spec = &CHAINSAW_ATTACK_SPECS[state.story_progression_room.chainsaw_attack_idx];
    if let (Some(eng), Some(bank)) = (state.audio_engine.as_mut(), state.audio_bank.as_ref()) {
        let _ = bank.play_se_id(eng, spec.sfx_id, 0.90);
    }
}

fn trigger_3d_hyper_spray_action(state: &mut AppState) {
    state.combat_arena.player.weapon = sh2_core::combat::WeaponGrip::HyperSpray;
    apply_3d_checkpoint_selection(state, 10);
    handle_endings_key(state, KeyCode::KeyH);
}

fn trigger_3d_blue_gem_ufo_modal(state: &mut AppState) {
    apply_3d_checkpoint_selection(state, 10);
    handle_endings_key(state, KeyCode::Digit8);
    if state.sim_core.scenario.flags.get(727) || state.story_progression_room.blue_gem_step == 0 {
        state.sim_core.scenario.flags.set(FLAG_SPAWN_BLUE_GEM, true);
        let _ = use_blue_gem(42, ROOM_HOTEL_312, true, &mut state.sim_core.scenario.flags);
    }
    open_checkpoint_modal(state, 10);
}
