//! 3D spatial sound command dispatch (`SeCallPos`, `SePosMove`, `SePosSet`, `SeVolume`, `SeCall`)
//! for `HorrorTriggersRoom` (`SH2-47`).

use crate::state::AppState;
use sh2_core::SpatialSoundCommand;

use super::scenarios::{active_scenario_anchor, HORROR_SCENARIOS};

pub(crate) fn apply_spatial_sound_command(state: &mut AppState, cmd: SpatialSoundCommand) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let p_local = state.sim_core.scenario.player_position;
    let p_yaw = state.sim_core.scenario.player_yaw;

    match cmd {
        SpatialSoundCommand::Call2D { se_id, volume } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;
            state.horror_triggers_room.mark_se_playing(se_id, 5.0);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_mut(), state.audio_bank.as_ref())
            {
                let _ = bank.play_se_id(eng, se_id, volume);
            }
            state.horror_triggers_room.last_event_log =
                format!("SeCall(0x{se_id:04X}, vol={volume:.2}) -> Triggered 2D Horror SFX");
        }
        SpatialSoundCommand::CallPos {
            se_id,
            volume,
            pos,
            channel,
        } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;

            // For Scenario 4 (`PrisonS` Room `0x72` random cell block `[-146000..-134000, 10400..32000]`),
            // keep the stage anchor within `~1600u` of the newly spawned cell so `"Ritual..."` (`0x4682`)
            // is always clearly audible with authentic directional stereo panning.
            if idx == 3 && se_id == 0x4682 {
                state.horror_triggers_room.stage_anchor_override =
                    Some([pos[0] + 900.0 - p_local.x, 0.0, pos[2] - 1200.0 - p_local.z]);
            }

            let anchor = active_scenario_anchor(state);
            let p_world = glam::Vec3::new(
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            );

            // For Room 209 whisper (`0x9C70`), `SeCallPos` passes `[0,0,0]` and `SePosMove` immediately sets the 2500u orbit pos
            let emit_world = if se_id == 0x9C70 && pos == [0.0, 0.0, 0.0] {
                let ang = state
                    .horror_triggers_room
                    .dispatcher
                    .apart_w2f_whisper_angle
                    + p_yaw;
                [
                    p_world.x + ang.sin() * 2500.0,
                    p_world.y,
                    p_world.z + ang.cos() * 2500.0,
                ]
            } else {
                pos
            };
            state.horror_triggers_room.active_emitter_world_pos = emit_world;
            let dur = if se_id == 0x9C70 { 6.5 } else { 5.0 };
            state.horror_triggers_room.mark_se_playing(se_id, dur);

            let emit_vec = glam::Vec3::from_array(emit_world);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_mut(), state.audio_bank.as_mut())
            {
                if let Ok(eval) =
                    bank.play_se_id_3d_tracked(eng, se_id, p_world, p_yaw, emit_vec, volume)
                {
                    state.horror_triggers_room.last_event_log = format!(
                        "SeCallPos(0x{:04X}, ch={}) @ [{:.0}, {:.0}, {:.0}] -> dist={:.0}u, gain={:.0}%, pan={:+.2}",
                        se_id,
                        channel,
                        emit_world[0],
                        emit_world[1],
                        emit_world[2],
                        eval.distance,
                        eval.effective_gain * 100.0,
                        eval.stereo_pan
                    );
                }
            }
        }
        SpatialSoundCommand::PosSet {
            se_id,
            volume,
            pos,
            channel: _,
        }
        | SpatialSoundCommand::PosMove { se_id, volume, pos } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;
            state.horror_triggers_room.active_emitter_world_pos = pos;
            let anchor = active_scenario_anchor(state);
            let p_world = glam::Vec3::new(
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            );
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_ref(), state.audio_bank.as_mut())
            {
                let _ = bank.update_se_id_3d(
                    eng,
                    se_id,
                    p_world,
                    p_yaw,
                    glam::Vec3::from_array(pos),
                    volume,
                );
            }
        }
        SpatialSoundCommand::SetVolume { se_id, volume } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;
            let anchor = active_scenario_anchor(state);
            let p_world = glam::Vec3::new(
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            );
            let emit_pos =
                glam::Vec3::from_array(state.horror_triggers_room.active_emitter_world_pos);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_ref(), state.audio_bank.as_mut())
            {
                let _ = bank.update_se_id_3d(eng, se_id, p_world, p_yaw, emit_pos, volume);
            }
        }
    }
}
