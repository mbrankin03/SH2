//! Keyboard input handling and quick-step / secondary action triggers for `HorrorTriggersRoom` (`SH2-47`).

use winit::keyboard::KeyCode;

use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::types::DOOR_SE_TABLE;
use sh2_core::{SpatialSoundCommand, StageHookContext, StageId};
use sh2_ui::menu::TitlePhase;

use super::audio_dispatch::apply_spatial_sound_command;
use super::scenarios::{active_scenario_anchor, init_horror_scenario, HORROR_SCENARIOS};

pub(crate) fn handle_horror_room_key_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) {
    if pressed && !repeat {
        match key_code {
            KeyCode::Escape | KeyCode::Backspace => {
                state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                tracing::info!(
                    "Returned from Stage Events & Horror Triggers Room to Test Rooms menu."
                );
                return;
            }
            KeyCode::Digit1
            | KeyCode::Digit2
            | KeyCode::Digit3
            | KeyCode::Digit4
            | KeyCode::Digit5
            | KeyCode::Digit6
            | KeyCode::Digit7
            | KeyCode::Digit8 => {
                state.horror_triggers_room.scenario_idx = match key_code {
                    KeyCode::Digit1 => 0,
                    KeyCode::Digit2 => 1,
                    KeyCode::Digit3 => 2,
                    KeyCode::Digit4 => 3,
                    KeyCode::Digit5 => 4,
                    KeyCode::Digit6 => 5,
                    KeyCode::Digit7 => 6,
                    _ => 7,
                };
                init_horror_scenario(state, false);
                if let Some(eng) = state.audio_engine.as_mut() {
                    let _ =
                        play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
                }
                return;
            }
            KeyCode::Tab | KeyCode::PageDown => {
                state.horror_triggers_room.scenario_idx =
                    (state.horror_triggers_room.scenario_idx + 1) % HORROR_SCENARIOS.len();
                init_horror_scenario(state, false);
                return;
            }
            KeyCode::PageUp => {
                state.horror_triggers_room.scenario_idx =
                    if state.horror_triggers_room.scenario_idx == 0 {
                        HORROR_SCENARIOS.len() - 1
                    } else {
                        state.horror_triggers_room.scenario_idx - 1
                    };
                init_horror_scenario(state, false);
                return;
            }
            KeyCode::KeyR => {
                init_horror_scenario(state, true);
                return;
            }
            KeyCode::KeyE | KeyCode::Enter => {
                state.horror_triggers_room.pending_action_down = true;
                return;
            }
            KeyCode::Space => {
                trigger_scenario_quick_step(state);
                return;
            }
            KeyCode::KeyK => {
                trigger_scenario_secondary_action(state);
                return;
            }
            KeyCode::KeyT => {
                let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
                if idx == 5 {
                    let p_loc = state.sim_core.scenario.player_position;
                    state.horror_triggers_room.stage_anchor_override =
                        Some([-58_082.0 - p_loc.x, 0.0, 19_800.0 - p_loc.z]);
                    state.sim_core.scenario.flags.set(69, true);
                    let out = state
                        .horror_triggers_room
                        .dispatcher
                        .on_sound_call_after_load(
                            StageId::ApartE2f,
                            0x18,
                            &mut state.sim_core.scenario.flags,
                        );
                    for cmd in out.sounds {
                        apply_spatial_sound_command(state, cmd);
                    }
                } else {
                    state.horror_triggers_room.door_se_idx =
                        (state.horror_triggers_room.door_se_idx + 1) % DOOR_SE_TABLE.len();
                    let d_idx = state.horror_triggers_room.door_se_idx;
                    let ds = DOOR_SE_TABLE[d_idx];
                    let preview_se = if state.horror_triggers_room.sandbox_has_key {
                        ds.unlock
                    } else {
                        ds.lock
                    } as u16 as u32;
                    if preview_se != 0 {
                        apply_spatial_sound_command(
                            state,
                            SpatialSoundCommand::CallPos {
                                se_id: preview_se,
                                volume: 1.0,
                                pos: [0.0, -500.0, 2200.0],
                                channel: 0,
                            },
                        );
                    }
                    state.horror_triggers_room.last_event_log = format!(
                        "Selected DOOR_SE_TABLE[{}]: open=0x{:04X}, close=0x{:04X}, lock=0x{:04X}, unlock=0x{:04X}",
                        d_idx, ds.open, ds.close, ds.lock, ds.unlock
                    );
                }
                return;
            }
            KeyCode::KeyV => {
                state.horror_triggers_room.show_3d_guides =
                    !state.horror_triggers_room.show_3d_guides;
                return;
            }
            _ => {}
        }
    }

    CombatArenaRoomController::handle_key_input(state, key_code, pressed, repeat);
}

pub(crate) fn trigger_scenario_quick_step(state: &mut AppState) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let anchor = active_scenario_anchor(state);

    match idx {
        0 => {
            state.sim_core.scenario.flags.set(648, true);
            state.sim_core.scenario.flags.set(649, true);
            let z_local = (state
                .horror_triggers_room
                .dispatcher
                .prison_n_bathroom_z_threshold
                - anchor[2])
                + 150.0;
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_local), 0.0);
            apply_spatial_sound_command(
                state,
                SpatialSoundCommand::CallPos {
                    se_id: 0x427D,
                    volume: 1.0,
                    pos: [-138_700.0, -50.0, -21_000.0],
                    channel: 0,
                },
            );
        }
        1 => {
            state.sim_core.scenario.flags.set(547, false);
            let x_local = (state
                .horror_triggers_room
                .dispatcher
                .apart_w2f_room209_x_threshold
                - anchor[0])
                - 150.0;
            state.sim_core.snap_player_transform(
                glam::Vec3::new(x_local, 0.0, 0.0),
                -std::f32::consts::FRAC_PI_2,
            );
        }
        2 => {
            state.sim_core.scenario.flags.set(552, false);
            state
                .horror_triggers_room
                .dispatcher
                .apart_e3fe_footstep_timer = 0.0;
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, 0.0), std::f32::consts::PI);
        }
        3 => {
            state
                .horror_triggers_room
                .dispatcher
                .prison_s_prisoner_timer = 0.0;
        }
        4 => {
            if !state
                .horror_triggers_room
                .dispatcher
                .apart_w2f_corridor_armed
            {
                let z_arm = (-6500.0 - anchor[2]) + 150.0;
                state
                    .sim_core
                    .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_arm), 0.0);
                if let Some(eng) = state.audio_engine.as_mut() {
                    let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
                }
                state.horror_triggers_room.last_event_log =
                    "Step 1: Crossed Z > -6500 -> apart_w2f_corridor_armed = true! Press [Space] again to back below Z < -6800"
                        .to_string();
            } else {
                state.sim_core.scenario.flags.set(553, false);
                let z_fire = (-6800.0 - anchor[2]) - 150.0;
                state
                    .sim_core
                    .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_fire), std::f32::consts::PI);
            }
        }
        5 => {
            let p_loc = state.sim_core.scenario.player_position;
            state.horror_triggers_room.stage_anchor_override =
                Some([-99_400.0 - p_loc.x, 0.0, 25_200.0 - p_loc.z]);
            state.sim_core.scenario.prog_runner.start(7, 1, 71);
            state.horror_triggers_room.last_event_log =
                "Started StageProgramRunner (7, 1) EvProgAnyoneCry -> SE 0x9C49 Scream + 2.0s Message!"
                    .to_string();
        }
        6 => {
            state.horror_triggers_room.stage_anchor_override = None;
            state
                .horror_triggers_room
                .dispatcher
                .society_staircase_timer = 0.0;
        }
        _ => {
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, 1850.0), 0.0);
            state.horror_triggers_room.event_engine.reset();
            state.horror_triggers_room.pending_action_down = true;
        }
    }
}

pub(crate) fn trigger_scenario_secondary_action(state: &mut AppState) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    match idx {
        0 => {
            let anchor = active_scenario_anchor(state);
            state.sim_core.scenario.flags.set(648, true);
            state.sim_core.scenario.flags.set(649, false);
            let z_local = (state
                .horror_triggers_room
                .dispatcher
                .prison_n_bathroom_z_threshold
                - anchor[2])
                + 150.0;
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_local), 0.0);
        }
        3 => {
            state.sim_core.scenario.flags.set(288, true);
            state.sim_core.scenario.flags.set(289, true);
            state.sim_core.scenario.flags.set(290, false);
            state.horror_triggers_room.dispatcher.prison_s_bars_timer = 0.0;
            let p_world = active_scenario_anchor(state);
            apply_spatial_sound_command(
                state,
                SpatialSoundCommand::CallPos {
                    se_id: 0x427B,
                    volume: 0.9,
                    pos: [p_world[0], -300.0, p_world[2] + 800.0],
                    channel: 0,
                },
            );
            state.horror_triggers_room.last_event_log =
                "Set Flags 288 & 289 ON -> Started Prison South Oppressor Cell Door Timer + SFX 0x427B"
                    .to_string();
        }
        5 => {
            let p_loc = state.sim_core.scenario.player_position;
            state.horror_triggers_room.stage_anchor_override =
                Some([-99_000.0 - p_loc.x, 0.0, 14_600.0 - p_loc.z]);
            state.sim_core.scenario.flags.set(69, false);
            state.sim_core.scenario.prog_runner.start(7, 17, 91);
            state.horror_triggers_room.last_event_log =
                "Started StageProgramRunner (7, 17) EvProgMonkeyKick206 -> SE 0x9C47 Room 206 Kick!"
                    .to_string();
        }
        6 => {
            let p_loc = state.sim_core.scenario.player_position;
            state.horror_triggers_room.stage_anchor_override =
                Some([-20_500.0 - p_loc.x, 0.0, -18_900.0 - p_loc.z]);
            state.sim_core.scenario.flags.set(551, false);
            let ctx = StageHookContext {
                stage: StageId::Society,
                room_id: 0x5C,
                dt: 1.0 / 60.0,
                player_pos: [-19_000.0, 0.0, -18_900.0],
                player_rot_y: state.sim_core.scenario.player_yaw,
                active_sounds: state.horror_triggers_room.active_sounds_array(),
            };
            let out = state.horror_triggers_room.dispatcher.on_all_time(
                &ctx,
                &mut state.sim_core.scenario.flags,
                &mut state.horror_triggers_room.rng,
            );
            for cmd in out.sounds {
                apply_spatial_sound_command(state, cmd);
            }
        }
        _ => {
            state.horror_triggers_room.sandbox_has_key =
                !state.horror_triggers_room.sandbox_has_key;
            if let Some(eng) = state.audio_engine.as_mut() {
                let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
            }
            state.horror_triggers_room.last_event_log = format!(
                "Toggled Key Item #1 possession: {} (Press [Space / E] to test lock vs unlock SE)",
                state.horror_triggers_room.sandbox_has_key
            );
        }
    }
}
