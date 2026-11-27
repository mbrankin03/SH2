//! In-engine 3D cutscene (`DramaDemoPlayer`) stepping, voice sync, and sequence chaining.

use std::time::Duration;

use crate::assets::{load_cutscene_voice_wav_for_source, open_drama_demo_session};
use crate::state::AppState;
use sh2_core::{CutsceneCameraSnapshot, CutsceneSequenceStep, DRAMA_DEMO_CATALOG};

pub(super) fn step_cutscene_player(
    state: &mut AppState,
    dt: Duration,
    is_paused: bool,
) -> Option<CutsceneCameraSnapshot> {
    let mut cutscene_camera_override = None;

    if !is_paused {
        let subs_enabled = state.menu_ctrl.options_menu.draft_config.game.subtitles;
        let voice_pos_secs = state
            .audio_engine
            .as_ref()
            .and_then(|eng| eng.voice_playback_position_secs());
        let mut voice_to_play: Option<(u16, usize, Option<sh2_assets::WavEntry>)> = None;
        let mut stop_voice_now = false;
        let mut finished_handoff = None;

        if let Some(ref mut session) = state.drama_demo_session {
            let mode = session.player.widescreen_mode;
            session.player.set_viewport_dimensions(
                state.surface_config.width as f32,
                state.surface_config.height as f32,
                mode,
            );
            session
                .player
                .tick_with_voice_sync(dt.as_secs_f32(), subs_enabled, voice_pos_secs);
            // Fire mid-cutscene stage hooks (e.g. `stg_ovservation.c` `0x005a0936..0x005a0955`:
            // sets `flag(517)` / `flag_620 |= 0x20` once `tenbou` passes frame `1400.0`, switching
            // `EvBgmControl` `0x005a1390` from Situation `4` to `14` Mute so `forest/bgm_114_b.aix`
            // Track 1 fades out instead of overlapping `voice.afs[118]` narration/music, `SH2-160`).
            sh2_core::campaign::cutscene_handoff::tick_demo_stage_hooks(
                u32::from(session.player.info.demo_no),
                Some(session.player.msg_frame.max(session.player.scene_frame)),
                &mut state.sim_core.scenario.flags,
            );
            if session.player.pending_voice_trigger {
                session.player.pending_voice_trigger = false;
                if let Some(afs_idx) = session.player.info.voice_afs_index() {
                    voice_to_play = Some((
                        session.player.info.demo_no,
                        afs_idx,
                        session.preloaded_voice_wav.take(),
                    ));
                }
            }
            if session.player.pending_voice_stop {
                session.player.pending_voice_stop = false;
                stop_voice_now = true;
            }
            if let Some((mut actor_pos, actor_yaw)) = session.player.player_handoff_transform() {
                actor_pos.y = state
                    .sim_core
                    .collision
                    .sample_floor_height(actor_pos.x, actor_pos.z, actor_pos.y, 1500.0, 1500.0)
                    .unwrap_or(actor_pos.y);
                state.sim_core.snap_player_transform(actor_pos, actor_yaw);
            }
            if !session.player.locks_gameplay_input() {
                finished_handoff = session.player.player_handoff_transform();
            } else {
                cutscene_camera_override = Some(session.player.camera);
            }
        }

        if let Some((demo_no, afs_idx, preloaded)) = voice_to_play {
            let loaded_on_demand = preloaded.is_none();
            let wav_opt = preloaded.or_else(|| {
                load_cutscene_voice_wav_for_source(afs_idx, state.bgm_manager.audio_pack_source)
            });
            if let Some(wav) = wav_opt {
                if let Some(ref mut eng) = state.audio_engine {
                    match eng.play_cutscene_voice(&wav) {
                        Ok(()) => {
                            tracing::info!(
                                "[Cutscene Voice] Started DramaDemo #{} voice.afs[{}] ({} Hz, {} ch, {} bytes, preloaded={})",
                                demo_no,
                                afs_idx,
                                wav.sample_rate,
                                wav.channels,
                                wav.data.len(),
                                !loaded_on_demand,
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[Cutscene Voice] Failed to play DramaDemo #{} voice.afs[{}]: {}",
                                demo_no,
                                afs_idx,
                                e
                            );
                        }
                    }
                }
                // `DramaDemoMain` (`SLUS_202.28` `0x001b0f00` line 108 / `sh2pc.exe` `0x004a2110`
                // line 1418): reset delta-frame timer (`shResetDF`) after synchronous stream load
                // so any disk-read latency never leaks into the next frame's `dt`.
                if loaded_on_demand {
                    state.last_frame = std::time::Instant::now();
                }
            }
        }
        if stop_voice_now {
            if let Some(ref mut eng) = state.audio_engine {
                eng.stop_voice(Duration::from_millis(50));
            }
        }
        if let Some((final_pos, final_yaw)) = finished_handoff {
            state
                .sim_core
                .handoff_player_after_cutscene(final_pos, final_yaw);
            // If this cutscene is part of a multi-segment `.dds` sequence chain (e.g. `papa_agl_a` -> `papa_agl`,
            // `piano_lau_b` -> `piano_lau_e`, `bp_edi_1` -> `bp_edi_2`), automatically chain into the next in-engine step!
            let next_catalog_idx = state.drama_demo_session.as_ref().and_then(|s| {
                let cur_no = s.player.info.demo_no;
                let chain = s.player.info.sequence_chain;
                let pos_in_chain = chain.iter().position(|step| {
                    matches!(step, CutsceneSequenceStep::InEngineDemo { demo_no } if *demo_no == cur_no)
                })?;
                if let Some(CutsceneSequenceStep::InEngineDemo { demo_no: next_no }) =
                    chain.get(pos_in_chain + 1)
                {
                    DRAMA_DEMO_CATALOG.iter().position(|d| d.demo_no == *next_no)
                } else {
                    None
                }
            });
            if let Some(next_idx) = next_catalog_idx {
                state.selected_drama_demo_idx = next_idx;
                state.drama_demo_session = open_drama_demo_session(
                    &state.render_ctx,
                    next_idx,
                    false,
                    state.loaded_texture_quality,
                    state.bgm_manager.audio_pack_source,
                );
                if let Some(ref s) = state.drama_demo_session {
                    cutscene_camera_override = Some(s.player.camera);
                }
            } else {
                state.drama_demo_session = None;
            }
        } else if state
            .drama_demo_session
            .as_ref()
            .is_some_and(|s| !s.player.locks_gameplay_input())
        {
            state.drama_demo_session = None;
            state.sim_core.sync_cutscene_handoff(None);
        }
    } else if let Some(ref session) = state.drama_demo_session {
        if session.player.locks_gameplay_input() {
            cutscene_camera_override = Some(session.player.camera);
        }
    }

    cutscene_camera_override
}
