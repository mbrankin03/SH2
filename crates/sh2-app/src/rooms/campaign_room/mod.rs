//! Thin shell controller for the unified start-to-credits Campaign Game Loop (`SH2-112`).
//!
//! All lifecycle state transitions, `connectMain` staging, `EventMainStandard` evaluation,
//! cutscene handoffs, and outdoor `LoadBg` planning live in `sh2_core::campaign::CampaignOrchestrator`.

pub(crate) mod cutaway;
pub(crate) mod loader;
pub(crate) mod messages;
#[cfg(test)]
mod tests;

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::assets::open_drama_demo_session;
use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::fmv_room::{
    clear_ldr_output_black, fade_out_fmv_audio, open_fmv_session, render_fmv_subtitles,
    stop_active_fmv_session_with_bgm_policy, tick_and_render_fmv_frame,
};
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
        state.campaign_credits_seq = None;
        state.bgm_stage_override = None;
        state.bgm_manager.stop_all(state.audio_engine.as_mut());
        state.se_pool.reset();
        crate::audio_dispatch::sync_se_pool_3d_bank(state);

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
        state.campaign_loaded_room = None;
        state.campaign_pending_input = Default::default();
        state.campaign_messages.reset();
        state
            .sim_core
            .scenario
            .cutaway_overlay
            .set_picture(None, &state.sim_core.scenario.flags);
        tracing::info!(
            "[Campaign] Started unified campaign session (scenario={:?}, continue={})",
            scenario,
            is_continue
        );
    }

    fn apply_effects(state: &mut AppState, effects: CampaignEffects) -> bool {
        let mut performed_sync_load = false;
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

        let prefer_sh2ee = state
            .menu_ctrl
            .options_menu
            .draft_config
            .video
            .texture_quality
            != sh2_core::config::WorldTextureQuality::OriginalPs2;

        if let Some(manifest) = effects.load_room {
            if u32::from(manifest.stage_id) != state.bgm_manager.stage_id {
                state.se_pool.reset();
            }
            loader::load_manifest_into_app_state(state, &manifest);
            state.campaign_messages.clear_active();
            state
                .campaign_messages
                .ensure_tables_loaded(manifest.stage_id, prefer_sh2ee);
            crate::audio_dispatch::sync_se_pool_3d_bank(state);
            if let Some(ref mut ls) = state.loading_screen {
                ls.mark_completed();
            }
            if let Some(ref mut orch) = state.campaign_orchestrator {
                orch.notify_room_assets_loaded();
            }
            performed_sync_load = true;
        }

        if effects.hide_loading_screen {
            state.loading_screen = None;
        }

        let stage_id = state.sim_core.scenario.current_stage;
        let prog_running = state.sim_core.scenario.prog_runner.is_running();
        state.campaign_messages.apply_effects(
            stage_id,
            prefer_sh2ee,
            effects.stage_message,
            effects.common_message,
            effects.clear_message,
            prog_running,
        );

        let _ = effects.bgm_stage_id;

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
            crate::audio_dispatch::sync_se_pool_3d_bank(state);
            let se_listener = sh2_audio::SeListenerContext {
                camera_pos: state.sim_core.active_camera_eye(1.0),
                player_pos: state.sim_core.scenario.player_position,
                wvs_matrix: state.sim_core.active_view_matrix(1.0),
                room_id: state.bgm_manager.room_id as u16,
            };
            let mut pool_actions = Vec::new();
            for ps in &effects.positional_sounds {
                if ps.se_id > 0 {
                    let pos = glam::Vec3::from_array(ps.pos);
                    state.se_pool.call_pos(
                        ps.volume,
                        ps.se_id as u32,
                        Some(pos),
                        sh2_audio::SE_FLAG_PRIO_1,
                        &se_listener,
                        &mut pool_actions,
                    );
                }
            }
            crate::audio_dispatch::dispatch_se_pool_actions(state, &pool_actions);
        }

        if let Some(demo) = effects.start_drama_demo {
            let catalog_idx = DRAMA_DEMO_CATALOG
                .iter()
                .position(|d| u32::from(d.demo_no) == demo.demo_no)
                .unwrap_or(0);
            state.selected_drama_demo_idx = catalog_idx;
            let already_preloaded = state
                .drama_demo_session
                .as_ref()
                .is_some_and(|s| s.catalog_index == catalog_idx);
            if !already_preloaded {
                state.drama_demo_session =
                    open_drama_demo_session(&state.render_ctx, catalog_idx, false);
                let tex_quality = state
                    .menu_ctrl
                    .options_menu
                    .draft_config
                    .video
                    .texture_quality;
                state.apply_texture_quality(tex_quality);
                performed_sync_load = true;
            }
        }

        if let Some(fmv) = effects.start_fmv {
            // Preload any chained `post_demo` (e.g. `first_toilet.dds` after `toilet.bik`)
            // BEFORE starting FMV playback while the screen is black, matching PS2 `connectMain`
            // RAM residency so FMV EOF transitions into frame 0 of the `.dds` cutscene with
            // zero disk/GPU hitch (`SH2-150`).
            if let Some(ref post_demo) = fmv.post_demo {
                let catalog_idx = DRAMA_DEMO_CATALOG
                    .iter()
                    .position(|d| u32::from(d.demo_no) == post_demo.demo_no)
                    .unwrap_or(0);
                let already_preloaded = state
                    .drama_demo_session
                    .as_ref()
                    .is_some_and(|s| s.catalog_index == catalog_idx);
                if !already_preloaded {
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
                    performed_sync_load = true;
                }
            }
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
                performed_sync_load = true;
            }
        }

        if effects.return_to_title {
            state.campaign_orchestrator = None;
            state.campaign_loaded_room = None;
            state.campaign_messages.reset();
            state.exit_room_to_menu(TitlePhase::MainMenu);
        }

        // Reset wall-clock frame timer after synchronous asset/FMV/cutscene loads (`shSetDFZero`
        // `SLUS_202.28` `0x00220220`) so a 200-1000 ms disk load stall never leaks into the next
        // frame's `dt_sec` and devours the `ScreenFadeState` fade-in in a single tick (`SH2-150`).
        if performed_sync_load {
            state.last_frame = std::time::Instant::now();
        }
        performed_sync_load
    }
}

impl RoomController for CampaignRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        let in_fmv = state
            .campaign_orchestrator
            .as_ref()
            .is_some_and(|o| matches!(o.phase, CampaignPhase::FmvPlayback { .. }));
        if in_fmv {
            if pressed
                && !repeat
                && matches!(
                    key_code,
                    KeyCode::Escape
                        | KeyCode::Backspace
                        | KeyCode::Space
                        | KeyCode::Enter
                        | KeyCode::NumpadEnter
                )
            {
                state.campaign_pending_input.cancel_pressed = true;
            }
            return;
        }

        let cutaway_or_prog_active =
            cutaway::has_active_cutaway(state) || state.sim_core.scenario.prog_runner.is_running();

        if pressed && !repeat {
            if matches!(
                key_code,
                KeyCode::KeyE | KeyCode::Enter | KeyCode::Space | KeyCode::Backspace
            ) && state.campaign_messages.handle_dismiss_input()
            {
                state.campaign_pending_input.message_dismissed = true;
                return;
            }
            match key_code {
                KeyCode::KeyE | KeyCode::Enter | KeyCode::Space => {
                    state.campaign_pending_input.action_pressed = true;
                    state.campaign_pending_input.message_dismissed = true;
                    state.campaign_pending_input.confirm_terminal_exit = true;
                    if cutaway_or_prog_active || key_code != KeyCode::Space {
                        return;
                    }
                }
                KeyCode::Backspace => {
                    state.campaign_pending_input.cancel_pressed = true;
                    if cutaway_or_prog_active {
                        return;
                    }
                }
                KeyCode::KeyM => {
                    state.campaign_pending_input.overlay_toggle = Some(OverlayMenuKind::Map);
                    return;
                }
                KeyCode::KeyI | KeyCode::Tab => {
                    state.campaign_pending_input.overlay_toggle = Some(OverlayMenuKind::Inventory);
                    return;
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

        // Sync live options toggles into the orchestrator and reload active room textures on demand
        // if the player switches `WorldTextureQuality` in Options mid-campaign.
        let current_tex_quality = state
            .menu_ctrl
            .options_menu
            .draft_config
            .video
            .texture_quality;
        if let Some((manifest, loaded_quality)) = state.campaign_loaded_room.clone() {
            if loaded_quality != current_tex_quality {
                loader::load_manifest_into_app_state(state, &manifest);
            }
        }
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
        tick_input.enable_sh2ee_world_fixes = state
            .menu_ctrl
            .options_menu
            .draft_config
            .game
            .enable_sh2ee_world_fixes;
        let in_drama_demo = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::DramaDemo { .. })
        );
        tick_input.drama_demo_finished = in_drama_demo
            && state
                .drama_demo_session
                .as_ref()
                .is_none_or(|s| s.player.step == DramaDemoStep::Finished);
        let was_fmv = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::FmvPlayback { .. })
        );
        let was_fmv_playing = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::FmvPlayback {
                exit_state: sh2_core::campaign::FmvExitState::Playing,
                ..
            })
        );
        tick_input.fmv_finished = state.fmv_session.is_none() && was_fmv_playing;

        if let Some(ref mut ls) = state.loading_screen {
            ls.tick(dt_sec);
        }

        // Clamp single-frame campaign delta to PS2 `shSetDF(2)` (`2.0 / 60.0` s, `SLUS_202.28`
        // `0x00220170`) so any OS/load hiccup cannot consume an entire fade ramp in one tick.
        let campaign_dt_sec = dt_sec.min(2.0 / 60.0);
        let effects = if let Some(mut orch) = state.campaign_orchestrator.take() {
            orch.outdoor_streamer.set_mode(streaming_mode);
            orch.delayed_fade_in_fix = delayed_fade_in;
            let eff = if is_paused {
                CampaignEffects::default()
            } else {
                orch.step(&mut state.sim_core, &tick_input, campaign_dt_sec)
            };
            state.campaign_orchestrator = Some(orch);
            eff
        } else {
            CampaignEffects::default()
        };

        let is_fmv_now = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::FmvPlayback { .. })
        );
        let now_fmv_fading_skip = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::FmvPlayback {
                exit_state: sh2_core::campaign::FmvExitState::FadingOutSkip,
                ..
            })
        );
        if was_fmv_playing && now_fmv_fading_skip {
            let fade_dur =
                Duration::from_secs_f32(1.0 / sh2_core::campaign::PS2_DEFAULT_FADE_SPEED);
            fade_out_fmv_audio(state, fade_dur);
        }
        if was_fmv && !is_fmv_now {
            stop_active_fmv_session_with_bgm_policy(state, false);
        }

        let allow_sim = effects.allow_gameplay_sim;
        let performed_sync_load = Self::apply_effects(state, effects);

        match state.campaign_orchestrator.as_ref().map(|o| &o.phase) {
            Some(CampaignPhase::GameOver) => {
                state.ingame_menus.gameover.tick(dt_sec);
            }
            Some(CampaignPhase::ActiveGameplay) => {
                state.ingame_menus.red_cross.tick(
                    dt_sec,
                    state.sim_core.scenario.health,
                    state.sim_core.scenario.max_health,
                );
            }
            Some(CampaignPhase::OverlayMenu(OverlayMenuKind::Save)) => {
                if let Some(ref mut sl) = state.ingame_menus.save_load {
                    sl.tick(dt_sec);
                }
            }
            Some(CampaignPhase::OverlayMenu(OverlayMenuKind::Map)) => {
                if !state.ingame_menus.map_viewer.is_open {
                    crate::ingame_menus::open_map_direct(state, false);
                }
            }
            Some(CampaignPhase::OverlayMenu(OverlayMenuKind::Inventory)) => {
                if !state.ingame_menus.inventory.is_open {
                    crate::ingame_menus::open_inventory(state);
                }
            }
            Some(CampaignPhase::OverlayMenu(OverlayMenuKind::Memo)) => {
                if !state.ingame_menus.memo_reader.is_open {
                    crate::ingame_menus::open_memo_reader(state);
                }
            }
            Some(CampaignPhase::EndingCredits {
                ending_stage,
                showing_results: false,
            }) => {
                let stage = *ending_stage;
                let seq = state.campaign_credits_seq.get_or_insert_with(|| {
                    let ending = match stage {
                        48 => sh2_core::endings::GameEnding::Leave,
                        49 => sh2_core::endings::GameEnding::Maria,
                        50 => sh2_core::endings::GameEnding::InWater,
                        51 => sh2_core::endings::GameEnding::Rebirth,
                        52 => sh2_core::endings::GameEnding::Dog,
                        54 => sh2_core::endings::GameEnding::BornFromAWish,
                        _ => sh2_core::endings::GameEnding::Leave,
                    };
                    sh2_ui::credits::EndingCreditsSequence::start(ending)
                });
                seq.tick(dt_sec, false);
            }
            _ => {}
        }

        let now_in_drama_demo = matches!(
            state.campaign_orchestrator.as_ref().map(|o| &o.phase),
            Some(CampaignPhase::DramaDemo { .. })
        );
        if (in_drama_demo && !now_in_drama_demo) || (was_fmv && !is_fmv_now && !now_in_drama_demo) {
            state.drama_demo_session = None;
            if let Some(ref mut eng) = state.audio_engine {
                eng.stop_voice(Duration::from_millis(50));
            }
        }

        let fmv_exit_state = match state.campaign_orchestrator.as_ref().map(|o| &o.phase) {
            Some(CampaignPhase::FmvPlayback { exit_state, .. }) => Some(*exit_state),
            _ => None,
        };
        if let Some(exit_state) = fmv_exit_state {
            let keep_bgm_active = state
                .fmv_session
                .as_ref()
                .and_then(|s| sh2_audio::cutscene_bgm::find_cutscene_audio_mapping(s.entry.id))
                .is_some_and(|m| {
                    m.bgm_action == sh2_audio::cutscene_bgm::CutsceneBgmAction::KeepActive
                });
            if keep_bgm_active && is_in_game {
                crate::rooms::combat_arena::render::step_audio_managers(state, dt, is_paused);
            }
            if state.fmv_session.is_none() {
                clear_ldr_output_black(state);
            } else {
                let effective_dt = match exit_state {
                    sh2_core::campaign::FmvExitState::Playing => dt_sec,
                    sh2_core::campaign::FmvExitState::FadingOutSkip => 0.0,
                };
                let _ = tick_and_render_fmv_frame(state, effective_dt, aspect, grain_pct, false);
            }
        } else {
            // During `DramaDemo`, keep `is_paused` false (unless user opened Pause Menu) so
            // `step_cutscene_player` advances the `.dds` cutscene while `locks_gameplay_input()`
            // suppresses player locomotion.
            let render_paused = if now_in_drama_demo {
                is_paused
            } else {
                is_paused || !allow_sim
            };
            let (render_dt, render_dt_sec) = if performed_sync_load {
                (Duration::ZERO, 0.0)
            } else {
                (dt, campaign_dt_sec)
            };
            CombatArenaRoomController::tick_and_render_3d(
                state,
                render_dt,
                render_dt_sec,
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
        let phase_snapshot = orch.phase.clone();
        let play_time_secs = orch.play_time_secs;

        let font = sh2_ui::AtlasFont::shared(None);
        match phase_snapshot {
            CampaignPhase::GameOver => {
                crate::ingame_menus::ensure_gameover_tex_loaded(state);
                state.begin_hud_overlay();
                let tex = state
                    .ingame_menus
                    .cached_gameover_tex
                    .as_ref()
                    .map(|(w, h, rgba)| (rgba.as_slice(), *w, *h));
                sh2_ui::draw_gameover_screen(
                    &state.ingame_menus.gameover,
                    tex,
                    font,
                    &mut state.menu_rasterizer,
                );
                state.finish_hud_overlay(encoder, surface_view);
                return;
            }
            CampaignPhase::OverlayMenu(OverlayMenuKind::Save) => {
                let scenario = state.menu_ctrl.title_sm.selected_scenario;
                crate::ingame_menus::ensure_savebg_loaded(state, scenario);
                if state.ingame_menus.save_load.is_none() {
                    let g = &state.menu_ctrl.options_menu.original_config.game;
                    let cfg = sh2_ui::SaveLoadConfig {
                        unlimited_slots: g.enable_unlimited_save_slots,
                        quicksave_autosave: g.enable_quicksave_autosave,
                        save_thumbnails: g.enable_save_thumbnails,
                    };
                    let mut sl =
                        sh2_ui::SaveLoadScreen::new(sh2_ui::SaveLoadMode::Save, scenario, cfg);
                    let repo =
                        sh2_core::campaign::SaveSlotRepository::new(&state.menu_ctrl.save_dir);
                    sl.set_slots(repo.scan_slots(cfg.unlimited_slots, cfg.quicksave_autosave));
                    state.ingame_menus.save_load = Some(sl);
                }
                state.begin_hud_overlay();
                let bg = state
                    .ingame_menus
                    .cached_savebg_tex
                    .as_ref()
                    .map(|(w, h, rgba)| (rgba.as_slice(), *w, *h));
                if let Some(ref sl) = state.ingame_menus.save_load {
                    sh2_ui::draw_save_load_screen(sl, bg, font, &mut state.menu_rasterizer);
                }
                state.finish_hud_overlay(encoder, surface_view);
                return;
            }
            CampaignPhase::EndingCredits {
                showing_results: true,
                ending_stage,
            } => {
                let clear_profile =
                    sh2_core::campaign::ClearDataProfile::load_from_dir(&state.menu_ctrl.save_dir);
                let action = state.menu_ctrl.title_sm.action_difficulty;
                let riddle = state.menu_ctrl.title_sm.riddle_difficulty;
                let preset = state
                    .menu_ctrl
                    .options_menu
                    .original_config
                    .detect_enhanced_preset();
                let ruleset = if preset == sh2_core::config::EnhancedPreset::Ps2Authentic {
                    sh2_core::endings::RankingRuleset::Ps2Original
                } else {
                    sh2_core::endings::RankingRuleset::PcDirectorCut
                };
                let mut info = state.sim_core.scenario.playing_info.clone();
                info.stage = ending_stage as u8;
                info.time = play_time_secs as f32;
                info.savecount = info.savecount.max(
                    state
                        .ingame_menus
                        .save_load
                        .as_ref()
                        .and_then(|sl| sl.slots.iter().map(|s| s.snapshot.save_count).max())
                        .unwrap_or(0) as u16,
                );
                info.clear_end_kind =
                    sh2_core::endings::ClearEndKind::from_clear_data(clear_profile.clear_data);
                info.clear_end_number = clear_profile.clear_data.clear_count;
                info.battle_level = match action {
                    sh2_core::config::ActionDifficulty::Beginner => 0,
                    sh2_core::config::ActionDifficulty::Easy => 1,
                    sh2_core::config::ActionDifficulty::Normal => 2,
                    sh2_core::config::ActionDifficulty::Hard => 3,
                };
                info.riddle_level = match riddle {
                    sh2_core::config::RiddleDifficulty::Easy => 0,
                    sh2_core::config::RiddleDifficulty::Normal => 1,
                    sh2_core::config::RiddleDifficulty::Hard => 2,
                    sh2_core::config::RiddleDifficulty::Extra => 3,
                };
                info.item_get = info.item_get.max(
                    state
                        .sim_core
                        .scenario
                        .inventory_mgr
                        .total_items_picked_up(),
                );
                info.hidden_item_get = state.sim_core.scenario.inventory_mgr.secret_items_mask();
                let results =
                    sh2_ui::ResultsScreen::from_playing_info(&info, action, riddle, ruleset);
                state.begin_hud_overlay();
                sh2_ui::draw_results_screen(&results, font, &mut state.menu_rasterizer);
                state.finish_hud_overlay(encoder, surface_view);
                return;
            }
            CampaignPhase::EndingCredits {
                showing_results: false,
                ..
            } => {
                state.begin_hud_overlay();
                if let Some(ref seq) = state.campaign_credits_seq {
                    seq.render(&mut state.menu_rasterizer, font);
                }
                state.finish_hud_overlay(encoder, surface_view);
                return;
            }
            CampaignPhase::FmvPlayback { .. } => {
                state.begin_hud_overlay();
                render_fmv_subtitles(state);
                if fade_alpha > 0.005 {
                    let w = state.menu_rasterizer.width;
                    let h = state.menu_rasterizer.height;
                    let a = (fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
                    state.menu_rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, a]);
                }
                state.finish_hud_overlay(encoder, surface_view);
                return;
            }
            CampaignPhase::OverlayMenu(_) => {
                crate::ingame_menus::render_overlay(state, encoder, surface_view);
                return;
            }
            _ => {}
        }

        let show_red_cross = matches!(phase_snapshot, CampaignPhase::ActiveGameplay)
            && state.ingame_menus.red_cross.alpha_u8() > 0;
        let has_cutaway = cutaway::has_active_cutaway(state);
        let active_msg = state
            .campaign_messages
            .active_page_text()
            .map(str::to_string);

        if fade_alpha <= 0.005 && !show_red_cross && !has_cutaway && active_msg.is_none() {
            CombatArenaRoomController::render_hud(state, encoder, surface_view);
            return;
        }

        if has_cutaway {
            cutaway::render_campaign_cutaway_native(state, encoder, surface_view);
        }

        let needs_hud_pass =
            (show_red_cross && !has_cutaway) || active_msg.is_some() || fade_alpha > 0.005;
        if needs_hud_pass {
            if show_red_cross && !has_cutaway {
                crate::ingame_menus::ensure_red_cross_tex_loaded(state);
            }
            state.begin_hud_overlay();
            if show_red_cross && !has_cutaway {
                let rc_tex = state
                    .ingame_menus
                    .cached_red_cross_tex
                    .as_ref()
                    .map(|(w, h, rgba)| (rgba.as_slice(), *w, *h));
                state
                    .ingame_menus
                    .red_cross
                    .draw(rc_tex, &mut state.menu_rasterizer);
            }
            if let Some(ref text) = active_msg {
                messages::draw_campaign_message(&mut state.menu_rasterizer, text);
            }
            if fade_alpha > 0.005 {
                let w = state.menu_rasterizer.width;
                let h = state.menu_rasterizer.height;
                let a = (fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
                state.menu_rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, a]);
            }
            state.finish_hud_overlay(encoder, surface_view);
        }
    }
}
