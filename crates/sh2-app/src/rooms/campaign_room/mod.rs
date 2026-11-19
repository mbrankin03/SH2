//! Thin shell controller for the unified start-to-credits Campaign Game Loop (`SH2-112`).
//!
//! All lifecycle state transitions, `connectMain` staging, `EventMainStandard` evaluation,
//! cutscene handoffs, and outdoor `LoadBg` planning live in `sh2_core::campaign::CampaignOrchestrator`.

pub(crate) mod loader;

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::assets::open_drama_demo_session;
use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::fmv_room::{open_fmv_session, FmvRoomController};
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_core::campaign::{CampaignEffects, CampaignOrchestrator, CampaignPhase, OverlayMenuKind};
use sh2_core::cutscene::{DramaDemoStep, DRAMA_DEMO_CATALOG};
use sh2_debug::FMV_CATALOG;
use sh2_ui::menu::TitlePhase;
use sh2_ui::LoadingScreen;

/// Thin [`RoomController`] bridging [`CampaignOrchestrator`] to `wgpu`, audio, FMV, and UI.
pub(crate) struct CampaignRoomController;

impl CampaignRoomController {
    /// Initializes a fresh [`CampaignOrchestrator`] on `AppState` when launching a campaign
    /// from the Title Screen (`NewGame` or `Continue`).
    pub(crate) fn start_session(state: &mut AppState, is_continue: bool) {
        let scenario = state.menu_ctrl.title_sm.selected_scenario;
        let action_diff = state.menu_ctrl.title_sm.action_difficulty;
        let riddle_diff = state.menu_ctrl.title_sm.riddle_difficulty;
        let streaming_mode = state
            .menu_ctrl
            .options_menu
            .draft_config
            .video
            .outdoor_streaming_mode;
        let delayed_fade_in = state
            .menu_ctrl
            .options_menu
            .draft_config
            .game
            .enable_delayed_fade_in;

        state.drama_demo_session = None;
        state.fmv_session = None;
        state.bgm_stage_override = None;
        state.bgm_manager.stop_all(state.audio_engine.as_mut());

        let orchestrator = if is_continue {
            let stage = state.sim_core.scenario.current_stage.max(2);
            let room = Some(state.sim_core.scenario.current_room.clone());
            let pos = state.sim_core.scenario.player_position.to_array();
            let yaw = state.sim_core.scenario.player_yaw;
            CampaignOrchestrator::from_saved_stage(
                stage,
                room,
                pos,
                yaw,
                streaming_mode,
                delayed_fade_in,
            )
        } else {
            CampaignOrchestrator::new_game(
                scenario,
                action_diff,
                riddle_diff,
                streaming_mode,
                delayed_fade_in,
            )
        };

        state.campaign_orchestrator = Some(orchestrator);
        state.campaign_pending_input = Default::default();
        tracing::info!(
            "[Campaign] Started unified campaign session (scenario={:?}, continue={})",
            scenario,
            is_continue
        );
    }

    fn apply_effects(state: &mut AppState, effects: CampaignEffects) {
        if let Some(label) = effects.show_loading_screen {
            let min_dur = state
                .menu_ctrl
                .options_menu
                .draft_config
                .debug
                .min_loading_screen_seconds as f32;
            let require_press = state
                .menu_ctrl
                .options_menu
                .draft_config
                .debug
                .require_press_to_continue;
            if !state
                .menu_ctrl
                .options_menu
                .draft_config
                .debug
                .instant_room_transitions
            {
                state.loading_screen = Some(LoadingScreen::new(&label, min_dur, require_press));
            }
        }

        if let Some(manifest) = effects.load_room {
            loader::load_manifest_into_app_state(state, &manifest);
            if let Some(ref mut ls) = state.loading_screen {
                ls.mark_completed();
            }
            if let Some(ref mut orch) = state.campaign_orchestrator {
                orch.notify_room_assets_loaded();
            }
        }

        if effects.hide_loading_screen {
            state.loading_screen = None;
        }

        if let Some(stage_id) = effects.bgm_stage_id {
            state.bgm_stage_override = Some(stage_id);
        }

        // Dispatch 2D non-positional SFX (`shSdCall` / `SeCall`) and 3D positional SFX (`SeCallPos`).
        if !effects.sound_effects.is_empty() {
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_mut(), state.audio_bank.as_ref())
            {
                for se_id in &effects.sound_effects {
                    let _ = bank.play_se_id(eng, *se_id, 1.0);
                }
            }
        }
        if !effects.positional_sounds.is_empty() {
            let p_local = state.sim_core.scenario.player_position;
            let (listener_yaw, listener_pitch) = state.sim_core.active_listener_yaw_pitch(1.0);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_mut(), state.audio_bank.as_mut())
            {
                for ps in &effects.positional_sounds {
                    if ps.se_id > 0 {
                        let _ = bank.play_se_id_3d_tracked_with_occlusion(
                            eng,
                            ps.se_id as u32,
                            p_local,
                            listener_yaw,
                            listener_pitch,
                            state.active_maria,
                            glam::Vec3::from_array(ps.pos),
                            ps.volume,
                            Some(&state.sim_core.collision),
                        );
                    }
                }
            }
        }

        if let Some(demo) = effects.start_drama_demo {
            let catalog_idx = DRAMA_DEMO_CATALOG
                .iter()
                .position(|d| u32::from(d.demo_no) == demo.demo_no)
                .unwrap_or(0);
            state.selected_drama_demo_idx = catalog_idx;
            state.drama_demo_session =
                open_drama_demo_session(&state.render_ctx, catalog_idx, false);
            let tex_quality = state
                .menu_ctrl
                .options_menu
                .draft_config
                .video
                .texture_quality;
            state.apply_texture_quality(tex_quality);
        }

        if let Some(fmv) = effects.start_fmv {
            if let Some((idx, entry)) = FMV_CATALOG
                .iter()
                .enumerate()
                .find(|(_, m)| m.id.eq_ignore_ascii_case(fmv.movie_id))
            {
                state.fmv_viewer.selected_movie_index = idx;
                state.loaded_fmv_movie_idx = idx;
                let source = state.fmv_viewer.video_source;
                let audio_pack = state.bgm_manager.audio_pack_source;
                state.fmv_session = open_fmv_session(
                    &state.render_ctx.device,
                    &state.render_ctx.queue,
                    &state.fmv_pipeline,
                    &mut state.audio_engine,
                    &mut state.fmv_viewer,
                    entry,
                    source,
                    audio_pack,
                );
            }
        }

        if effects.return_to_title {
            state.campaign_orchestrator = None;
            state.exit_room_to_menu(TitlePhase::MainMenu);
        }
    }
}

impl RoomController for CampaignRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        if pressed && !repeat {
            match key_code {
                KeyCode::KeyE | KeyCode::Enter | KeyCode::Space => {
                    state.campaign_pending_input.action_pressed = true;
                    state.campaign_pending_input.message_dismissed = true;
                    state.campaign_pending_input.confirm_terminal_exit = true;
                }
                KeyCode::Backspace => {
                    state.campaign_pending_input.cancel_pressed = true;
                }
                KeyCode::KeyM => {
                    state.campaign_pending_input.overlay_toggle = Some(OverlayMenuKind::Map);
                }
                KeyCode::KeyI | KeyCode::Tab => {
                    state.campaign_pending_input.overlay_toggle = Some(OverlayMenuKind::Inventory);
                }
                _ => {}
            }
        }
        CombatArenaRoomController::handle_key_input(state, key_code, pressed, repeat);
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        dt_sec: f32,
        aspect: f32,
        grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        if state.campaign_orchestrator.is_none() {
            Self::start_session(state, false);
        }

        // Sync live options toggles into the orchestrator.
        let streaming_mode = state
            .menu_ctrl
            .options_menu
            .draft_config
            .video
            .outdoor_streaming_mode;
        let delayed_fade_in = state
            .menu_ctrl
            .options_menu
            .draft_config
            .game
            .enable_delayed_fade_in;

        let mut tick_input = std::mem::take(&mut state.campaign_pending_input);
        tick_input.prefer_sh2ee = state
            .menu_ctrl
            .options_menu
            .draft_config
            .video
            .texture_quality
            != sh2_core::config::WorldTextureQuality::OriginalPs2;
        let in_drama_demo = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::DramaDemo { .. })
        );
        tick_input.drama_demo_finished = in_drama_demo
            && state
                .drama_demo_session
                .as_ref()
                .is_none_or(|s| s.player.step == DramaDemoStep::Finished);
        tick_input.fmv_finished = state.fmv_session.is_none()
            && matches!(
                state.campaign_orchestrator.as_ref().map(|o| &o.phase),
                Some(CampaignPhase::FmvPlayback { .. })
            );

        if let Some(ref mut ls) = state.loading_screen {
            ls.tick(dt_sec);
        }

        let effects = if let Some(mut orch) = state.campaign_orchestrator.take() {
            orch.outdoor_streamer.set_mode(streaming_mode);
            orch.delayed_fade_in_fix = delayed_fade_in;
            let eff = if is_paused {
                CampaignEffects::default()
            } else {
                orch.step(&mut state.sim_core, &tick_input, dt_sec)
            };
            state.campaign_orchestrator = Some(orch);
            eff
        } else {
            CampaignEffects::default()
        };

        let allow_sim = effects.allow_gameplay_sim;
        Self::apply_effects(state, effects);

        let now_in_drama_demo = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::DramaDemo { .. })
        );
        if in_drama_demo && !now_in_drama_demo {
            state.drama_demo_session = None;
            if let Some(ref mut eng) = state.audio_engine {
                eng.stop_voice(Duration::from_millis(50));
            }
        }

        let is_fmv = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::FmvPlayback { .. })
        );
        if is_fmv {
            FmvRoomController::tick_and_render_3d(
                state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
            );
        } else {
            // During `DramaDemo`, keep `is_paused` false (unless user opened Pause Menu) so
            // `step_cutscene_player` advances the `.dds` cutscene while `locks_gameplay_input()`
            // suppresses player locomotion.
            let render_paused = if now_in_drama_demo {
                is_paused
            } else {
                is_paused || !allow_sim
            };
            CombatArenaRoomController::tick_and_render_3d(
                state,
                dt,
                dt_sec,
                aspect,
                grain_pct,
                render_paused,
                is_in_game,
            );
        }
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        let Some(ref orch) = state.campaign_orchestrator else {
            return;
        };
        let fade_alpha = orch.fade.black_alpha();
        let status_line: Option<String> = match &orch.phase {
            CampaignPhase::OverlayMenu(kind) => {
                Some(format!("CAMPAIGN OVERLAY: {:?}  [M/I/ESC to Resume]", kind))
            }
            CampaignPhase::GameOver => {
                Some("GAME OVER — Press [ENTER] / [E] to return to Title".to_string())
            }
            CampaignPhase::EndingCredits {
                ending_stage,
                showing_results,
            } => Some(if *showing_results {
                format!(
                    "RESULTS (Stage {}) — Press [ENTER] to return to Title",
                    ending_stage
                )
            } else {
                format!(
                    "ENDING CREDITS (Stage {}) — Press [ENTER] for Results",
                    ending_stage
                )
            }),
            _ => None,
        };

        if fade_alpha <= 0.005 && status_line.is_none() {
            CombatArenaRoomController::render_hud(state, encoder, surface_view);
            return;
        }

        state.begin_hud_overlay();
        if fade_alpha > 0.005 {
            let w = state.menu_rasterizer.width;
            let h = state.menu_rasterizer.height;
            let a = (fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            state.menu_rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, a]);
        }
        if let Some(ref line) = status_line {
            state
                .menu_rasterizer
                .draw_text(line, 40, 40, 2, [235, 225, 200, 255], true);
        }
        state.finish_hud_overlay(encoder, surface_view);
    }
}
