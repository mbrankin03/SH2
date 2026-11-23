//! In-engine 3D cutscene (`DramaDemoPlayer`) stepping, voice sync, sequence chaining, and cutscene
//! spotlight uniform construction.

use std::time::Duration;

use crate::assets::{load_cutscene_voice_wav, open_drama_demo_session};
use crate::state::AppState;
use sh2_assets::formats::demo::{DemoLightKind, DemoLightState};
use sh2_core::{CutsceneCameraSnapshot, CutsceneSequenceStep, DRAMA_DEMO_CATALOG};

pub(super) fn step_cutscene_player(
    state: &mut AppState,
    dt: Duration,
    is_paused: bool,
) -> (Option<CutsceneCameraSnapshot>, Option<DemoLightState>) {
    let mut cutscene_camera_override = None;
    let mut cutscene_light_override = None;

    if !is_paused {
        let subs_enabled = state.menu_ctrl.options_menu.draft_config.game.subtitles;
        let voice_pos_secs = state
            .audio_engine
            .as_ref()
            .and_then(|eng| eng.voice_playback_position_secs());
        let mut voice_afs_to_play = None;
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
            if session.player.pending_voice_trigger {
                session.player.pending_voice_trigger = false;
                voice_afs_to_play = session.player.info.voice_afs_index();
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
                cutscene_light_override = session.player.primary_active_light().copied();
            }
        }

        if let Some(afs_idx) = voice_afs_to_play {
            if let Some(wav) = load_cutscene_voice_wav(afs_idx) {
                if let Some(ref mut eng) = state.audio_engine {
                    let _ = eng.play_cutscene_voice(&wav);
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
                state.drama_demo_session =
                    open_drama_demo_session(&state.render_ctx, next_idx, false);
                if let Some(ref s) = state.drama_demo_session {
                    cutscene_camera_override = Some(s.player.camera);
                    cutscene_light_override = s.player.primary_active_light().copied();
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
            cutscene_light_override = session.player.primary_active_light().copied();
        }
    }

    (cutscene_camera_override, cutscene_light_override)
}

pub(super) fn build_cutscene_spotlight_uniforms(
    state: &AppState,
    cs_light: DemoLightState,
    player_pos: glam::Vec3,
) -> sh2_renderer::light::SpotlightUniforms {
    let ambient_tint = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input())
        .map(|s| s.player.atmosphere.ambient_tint_rgba)
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let l_pos = glam::Vec3::from_array(cs_light.position);
    let l_tgt = glam::Vec3::from_array(cs_light.interest);
    let (_near_decay, far_decay) = match cs_light.kind {
        DemoLightKind::Point | DemoLightKind::Spot => {
            sh2_core::demo_light_decay_bounds(cs_light.decay[1])
        }
        DemoLightKind::Directional => (3000.0, 7500.0),
    };
    let boost = (far_decay / 3500.0).clamp(0.25, 0.55);
    let (pos, dir, inner_cos, outer_cos) = match cs_light.kind {
        DemoLightKind::Spot => (
            l_pos,
            (l_tgt - l_pos).normalize_or(glam::Vec3::Z),
            cs_light.cone[0].cos().clamp(0.1, 0.99),
            cs_light.cone[1].cos().clamp(0.05, 0.95),
        ),
        DemoLightKind::Point => (
            l_pos,
            (player_pos - l_pos).normalize_or(glam::Vec3::Y),
            0.25,
            -0.65,
        ),
        DemoLightKind::Directional => {
            let safe_d = (-l_pos).normalize_or(glam::Vec3::Y);
            (player_pos - safe_d * 50_000.0, safe_d, -1.0, -1.0)
        }
    };
    let effective_far = far_decay.max(4500.0);
    sh2_renderer::light::SpotlightUniforms {
        position: [pos.x, pos.y, pos.z, 1.0],
        direction_inner: [dir.x, dir.y, dir.z, inner_cos],
        color_outer: [
            cs_light.color[0].max(0.20) * boost,
            cs_light.color[1].max(0.20) * boost,
            cs_light.color[2].max(0.20) * boost,
            outer_cos,
        ],
        ambient_tint,
        fill_and_range: [0.0, effective_far, 0.0, 0.0],
        ps2_falloff_params: sh2_renderer::light::compute_ps2_falloff_params(
            sh2_renderer::light::PS2_DEFAULT_FLASHLIGHT_NEAR_MM,
            effective_far,
        ),
        ..Default::default()
    }
}
