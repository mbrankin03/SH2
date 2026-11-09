//! Per-frame `EventEngine` + `StageEventDispatcher` simulation step and 3D wireframe trigger guide
//! rendering for `HorrorTriggersRoom` (`SH2-47`).

use std::time::Duration;

use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_audio::{SFX_MAX_DISTANCE, SFX_MIN_DISTANCE};
use sh2_core::{EventEvalInput, SpatialSoundCommand, StageHookContext};
use sh2_debug::DebugLineVertex;

use super::audio_dispatch::apply_spatial_sound_command;
use super::scenarios::{
    active_scenario_anchor, build_scenario_event_tables, init_horror_scenario, HORROR_SCENARIOS,
};

pub(crate) fn tick_and_render_horror_room_3d(
    state: &mut AppState,
    dt: Duration,
    dt_sec: f32,
    aspect: f32,
    grain_pct: u32,
    is_paused: bool,
    is_in_game: bool,
) {
    if !state.horror_triggers_room.initialized {
        init_horror_scenario(state, false);
    }

    if !is_paused {
        state.horror_triggers_room.tick_se_timers(dt_sec);

        let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
        let spec = HORROR_SCENARIOS[idx];
        let anchor = active_scenario_anchor(state);
        let p_local = state.sim_core.scenario.player_position;
        let p_yaw = state.sim_core.scenario.player_yaw;
        let p_world = [
            anchor[0] + p_local.x,
            anchor[1] + p_local.y,
            anchor[2] + p_local.z,
        ];

        // 1. Evaluate declarative `EventEngine::tick_standard`
        let action_down = std::mem::take(&mut state.horror_triggers_room.pending_action_down);
        if idx == 0 || idx == 7 {
            let (ev_list, ev_pos) = build_scenario_event_tables(
                idx,
                state.horror_triggers_room.door_se_idx,
                state.horror_triggers_room.sandbox_has_key,
            );
            let eval_in = EventEvalInput {
                player_pos: [p_local.x, p_local.y, p_local.z],
                player_rot_y: p_yaw,
                action_pressed: action_down,
                ..Default::default()
            };
            let mut inv = state.sim_core.scenario.inventory_mgr.clone();
            let effects = state.horror_triggers_room.event_engine.tick_standard(
                &ev_list,
                &[],
                &ev_pos,
                1,
                &eval_in,
                true,
                true,
                true,
                true,
                &mut state.sim_core.scenario.flags,
                &mut inv,
            );
            if idx == 0 && action_down {
                state.sim_core.scenario.flags.set(648, true);
                state.sim_core.scenario.flags.set(649, false);
                state.horror_triggers_room.event_engine.reset();
                apply_spatial_sound_command(
                    state,
                    SpatialSoundCommand::CallPos {
                        se_id: 0x2EF3,
                        volume: 0.95,
                        pos: [-138_700.0, -200.0, -21_000.0],
                        channel: 0,
                    },
                );
                state.horror_triggers_room.last_event_log = format!(
                    "Knocked on Bathroom Stall Door (0x2EF3) -> Flag 648 ON! Walk North across Z > {:.0} (or press [Space])",
                    state
                        .horror_triggers_room
                        .dispatcher
                        .prison_n_bathroom_z_threshold
                );
            } else if idx == 7 {
                for snd in &effects.positional_sounds {
                    apply_spatial_sound_command(
                        state,
                        SpatialSoundCommand::CallPos {
                            se_id: snd.se_id as u16 as u32,
                            volume: snd.volume,
                            pos: snd.pos,
                            channel: snd.channel,
                        },
                    );
                }
                if let Some(msg) = effects.stage_message {
                    state.horror_triggers_room.last_event_log = format!(
                        "EventEngine triggered Stage Message #{msg} (Quad Flag 700={})",
                        state.sim_core.scenario.flags.get(700)
                    );
                    state.horror_triggers_room.event_engine.reset();
                } else if let Some(c_msg) = effects.common_message {
                    state.horror_triggers_room.last_event_log = format!(
                        "Door Interaction -> Common Message #{c_msg} (Door Unlocked Flag 701={}, Door SE #{})",
                        state.sim_core.scenario.flags.get(701),
                        state.horror_triggers_room.door_se_idx
                    );
                    state.horror_triggers_room.event_engine.reset();
                }
            }
        }

        // 2. Advance `StageProgramRunner` if active
        if state.sim_core.scenario.prog_runner.is_running() {
            let s = &mut state.sim_core.scenario;
            s.prog_runner.tick(
                sh2_core::PROG_KEY_A,
                0,
                dt_sec,
                &mut s.flags,
                &mut s.puzzle_state,
                &mut s.inventory_mgr,
                &mut s.cutaway_overlay,
                s.riddle_difficulty,
            );
            let pending_ses: Vec<u32> = s.prog_runner.sound_effects().to_vec();
            for se_id in pending_ses {
                let emit_pos = if se_id == 0x9C49 {
                    [-99_400.0, -500.0, 26_400.0]
                } else {
                    [-99_000.0, -500.0, 13_600.0]
                };
                apply_spatial_sound_command(
                    state,
                    SpatialSoundCommand::CallPos {
                        se_id,
                        volume: 1.0,
                        pos: emit_pos,
                        channel: 6,
                    },
                );
            }
        }

        // 3. Advance per-frame `StageEventDispatcher::on_all_time`
        if let Some(stage_id) = spec.stage_id {
            let prev_648 = state.sim_core.scenario.flags.get(648);
            let prev_649 = state.sim_core.scenario.flags.get(649);
            let active_sounds = state.horror_triggers_room.active_sounds_array();
            let ctx = StageHookContext {
                stage: stage_id,
                room_id: spec.room_id,
                dt: dt_sec,
                player_pos: p_world,
                player_rot_y: p_yaw,
                active_sounds,
            };
            let out = state.horror_triggers_room.dispatcher.on_all_time(
                &ctx,
                &mut state.sim_core.scenario.flags,
                &mut state.horror_triggers_room.rng,
            );
            for cmd in out.sounds {
                apply_spatial_sound_command(state, cmd);
            }
            if idx == 0
                && prev_648
                && !prev_649
                && !state.sim_core.scenario.flags.get(648)
                && !state.sim_core.scenario.flags.get(649)
            {
                state.horror_triggers_room.last_event_log =
                    "Crossed Z > s_thr: 40% RNG roll disarmed Flag 648 (No Jumpscare)! Press [Space] for guaranteed Jumpscare or [K] to re-roll"
                        .to_string();
            }
        }

        // 4. Continuously update stereo panning & distance attenuation of any active 3D emitter
        if !state.horror_triggers_room.active_se_timers.is_empty() {
            let last_se = state.horror_triggers_room.last_se_id;
            let last_vol = state.horror_triggers_room.last_se_vol;
            let emit_pos =
                glam::Vec3::from_array(state.horror_triggers_room.active_emitter_world_pos);
            let p_world_vec = glam::Vec3::from_array(p_world);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_ref(), state.audio_bank.as_mut())
            {
                let _ = bank.update_se_id_3d(eng, last_se, p_world_vec, p_yaw, emit_pos, last_vol);
            }
        }
    }

    CombatArenaRoomController::tick_and_render_3d(
        state, dt, dt_sec, aspect, grain_pct, is_paused, false,
    );

    if is_in_game {
        let (player_pos, _) = state
            .sim_core
            .interpolated_player_pose(state.sim_core.interpolation_alpha());
        let mut skeletal_draw = Vec::new();
        let active_char_ref = if state.active_maria {
            state.maria.as_ref().or(state.character.as_ref())
        } else {
            state.character.as_ref().or(state.maria.as_ref())
        };
        if let Some(c) = active_char_ref {
            if !state.skin_matrices_scratch.is_empty() {
                skeletal_draw.push((
                    &c.gpu_mesh,
                    state.skin_matrices_scratch.as_slice(),
                    Some(c.texture_bind_groups.as_slice()),
                ));
            }
        }

        if state.horror_triggers_room.show_3d_guides {
            let mut debug_lines = build_horror_debug_lines(state);
            let chara_lines = sh2_debug::build_chara_collision_debug_lines(
                player_pos.into(),
                state.sim_core.collision_config.radius,
                state.sim_core.collision_config.height,
                16,
                [1.0, 0.9, 0.1, 1.0],
            );
            debug_lines.extend(chara_lines);

            state.render_ctx.render_scene_with_debug_lines(
                Some(&state.gpu_map),
                Some(&state.map_texture_bind_groups),
                &skeletal_draw,
                &debug_lines,
            );
        } else {
            state.render_ctx.render_scene(
                Some(&state.gpu_map),
                Some(&state.map_texture_bind_groups),
                &skeletal_draw,
            );
        }
    }
}

/// Builds 3D wireframe guides for the active horror scenario.
pub(crate) fn build_horror_debug_lines(state: &AppState) -> Vec<DebugLineVertex> {
    let mut lines = Vec::with_capacity(512);
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let anchor = active_scenario_anchor(state);
    let p_local = state.sim_core.scenario.player_position;

    let cyan = [0.25, 0.90, 1.0, 0.95];
    let red = [1.0, 0.25, 0.30, 0.95];
    let green = [0.25, 0.95, 0.45, 0.95];
    let gold = [1.0, 0.85, 0.20, 0.95];
    let y = -6.0f32;

    match idx {
        0 => {
            let stall_col = if state.sim_core.scenario.flags.get(648) {
                green
            } else {
                gold
            };
            push_circle_xz(&mut lines, 0.0, -1000.0, y, 650.0, 24, stall_col);
            let z_thr_local = state
                .horror_triggers_room
                .dispatcher
                .prison_n_bathroom_z_threshold
                - anchor[2];
            let line_col = if state.sim_core.scenario.flags.get(649) {
                red
            } else if state.sim_core.scenario.flags.get(648) {
                green
            } else {
                cyan
            };
            lines.push(DebugLineVertex::new([-2500.0, y, z_thr_local], line_col));
            lines.push(DebugLineVertex::new([2500.0, y, z_thr_local], line_col));
        }
        1 => {
            let x_thr_local = state
                .horror_triggers_room
                .dispatcher
                .apart_w2f_room209_x_threshold
                - anchor[0];
            lines.push(DebugLineVertex::new([x_thr_local, y, -2500.0], cyan));
            lines.push(DebugLineVertex::new([x_thr_local, y, 2500.0], cyan));
            push_circle_xz(&mut lines, p_local.x, p_local.z, y, 2500.0, 36, gold);
        }
        2 => {
            let z_thr_local = 17_000.0 - anchor[2];
            lines.push(DebugLineVertex::new([-2500.0, y, z_thr_local], cyan));
            lines.push(DebugLineVertex::new([2500.0, y, z_thr_local], cyan));
        }
        4 => {
            let z_arm = -6500.0 - anchor[2];
            let z_fire = -6800.0 - anchor[2];
            lines.push(DebugLineVertex::new([-2500.0, y, z_arm], green));
            lines.push(DebugLineVertex::new([2500.0, y, z_arm], green));
            lines.push(DebugLineVertex::new([-2500.0, y, z_fire], red));
            lines.push(DebugLineVertex::new([2500.0, y, z_fire], red));
        }
        6 => {
            lines.push(DebugLineVertex::new([-2500.0, y, 0.0], cyan));
            lines.push(DebugLineVertex::new([2500.0, y, 0.0], cyan));
        }
        7 => {
            let door_col = if state.horror_triggers_room.sandbox_has_key
                || state.sim_core.scenario.flags.get(701)
            {
                green
            } else {
                red
            };
            lines.push(DebugLineVertex::new([-800.0, y, 2200.0], door_col));
            lines.push(DebugLineVertex::new([800.0, y, 2200.0], door_col));
            lines.push(DebugLineVertex::new([0.0, y, 2200.0], door_col));
            lines.push(DebugLineVertex::new([0.0, y, 1650.0], door_col));

            push_circle_xz(&mut lines, -2000.0, 0.0, y, 850.0, 24, cyan);

            let q_col = if state.sim_core.scenario.flags.get(700) {
                green
            } else {
                cyan
            };
            let corners = [
                [1400.0, y, -800.0],
                [1400.0, y, 800.0],
                [2800.0, y, 800.0],
                [2800.0, y, -800.0],
            ];
            for i in 0..4 {
                lines.push(DebugLineVertex::new(corners[i], q_col));
                lines.push(DebugLineVertex::new(corners[(i + 1) % 4], q_col));
            }
        }
        _ => {}
    }

    let emit_w = state.horror_triggers_room.active_emitter_world_pos;
    let emit_local_x = (emit_w[0] - anchor[0]).clamp(-5200.0, 5200.0);
    let emit_local_z = (emit_w[2] - anchor[2]).clamp(-7200.0, 7200.0);

    let beacon = sh2_debug::build_chara_collision_debug_lines(
        [emit_local_x, 0.0, emit_local_z],
        180.0,
        1400.0,
        16,
        [1.0, 0.55, 0.15, 1.0],
    );
    lines.extend(beacon);
    push_circle_xz(
        &mut lines,
        emit_local_x,
        emit_local_z,
        -4.0,
        SFX_MIN_DISTANCE,
        32,
        [0.2, 0.95, 0.4, 0.85],
    );
    push_circle_xz(
        &mut lines,
        emit_local_x,
        emit_local_z,
        -4.0,
        SFX_MAX_DISTANCE,
        36,
        [0.95, 0.25, 0.25, 0.65],
    );

    lines
}

fn push_circle_xz(
    lines: &mut Vec<DebugLineVertex>,
    cx: f32,
    cz: f32,
    y: f32,
    radius: f32,
    segs: usize,
    color: [f32; 4],
) {
    for i in 0..segs {
        let a0 = (i as f32) * std::f32::consts::TAU / (segs as f32);
        let a1 = ((i + 1) as f32) * std::f32::consts::TAU / (segs as f32);
        lines.push(DebugLineVertex::new(
            [cx + a0.cos() * radius, y, cz + a0.sin() * radius],
            color,
        ));
        lines.push(DebugLineVertex::new(
            [cx + a1.cos() * radius, y, cz + a1.sin() * radius],
            color,
        ));
    }
}
