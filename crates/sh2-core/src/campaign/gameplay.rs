//! Active-gameplay (`EventMain` `0x001a7810` / `EventMainStandard` `0x001a7b50`) and
//! new-game prologue (`PlayableMain` step `0` `0x0020b414`) handlers for [`CampaignOrchestrator`].

use glam::Vec3;

use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::cutscene::find_drama_demo_by_no;
use crate::event_camera::PostSequenceRestorePolicy;
use crate::events::engine::{EventEngine, EventEngineEffects, StageTransitionCommand};
use crate::events::stages_act1::act1_stage_descriptor;
use crate::events::stages_act2::stage_table_act2;
use crate::events::triggers::EventEvalInput;
use crate::events::{
    ClearData, Sh2Rng, PROG_KEY_A, PROG_KEY_B, PROG_KEY_DOWN, PROG_KEY_LEFT, PROG_KEY_RIGHT,
    PROG_KEY_UP,
};
use crate::scenario::ScenarioKind;
use crate::simulation::SimulationCore;
use crate::types::ItemListEntry;
use crate::world::OutdoorTileStreamer;

use super::cutscene_handoff::{DramaDemoTrigger, AUTO_STAGE_DEMO_RULES};
use super::io::{CampaignEffects, CampaignTickInput};
use super::retail_events::tick_retail_stage_events;
use super::stage_loader::{
    resolve_outdoor_area_block, resolve_stage_manifest, room_id_matches_outdoor_block,
};
use super::state::{
    CampaignPhase, ConnectStep, CutawayFadeState, OverlayMenuKind, RoomLoadTarget, ScreenFadeState,
    MAIN_SCENARIO_START_STAGE, PS2_DEFAULT_FADE_SPEED, SE_GAME_OVER, WISH_SCENARIO_START_STAGE,
};

/// Outcome of a single [`step_active_gameplay`] tick.
pub(crate) enum GameplayStepOutcome {
    /// Remain in [`CampaignPhase::ActiveGameplay`].
    Continue,
    /// Player HP reached `<= 0.0` (`PlayerEventJamesDeadly` `0x001a78a8`): enter `GameOver`.
    EnterGameOver,
    /// Player opened an overlay menu (`Map`, `Inventory`, `Options`, `Memo`, `Pause`).
    OpenOverlay(OverlayMenuKind),
    /// `EventExecMove` (`0x001aa670`) requested a door/room/stage transition.
    RequestTransition {
        current_stage: u16,
        command: StageTransitionCommand,
    },
    /// `StageProgramRunner` (`ev_prog`) requested launching a real-time `.dds` cutscene (`p_demo`).
    StartDemo(DramaDemoTrigger),
    /// `p_game_end` triggered an ending sequence (`GameendMain` `0x001c4830`).
    TriggerEnding(u16),
}

/// Executes `PlayableMain` step `0` (`0x0020b414`: `FlagInit` `sh2pc.exe` `0x0048aa3e..0x0048af60`,
/// `ItemDataInit`, `ScreenEffectFadeStart(2)`) and returns the initial [`CampaignPhase::RoomLoading`] phase.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_prologue(
    sim: &mut SimulationCore,
    event_engine: &mut EventEngine,
    outdoor_streamer: &mut OutdoorTileStreamer,
    fade: &mut ScreenFadeState,
    scenario: ScenarioKind,
    action_difficulty: ActionDifficulty,
    riddle_difficulty: RiddleDifficulty,
    clear_data: ClearData,
    rng_seed: u32,
) -> CampaignPhase {
    sim.select_scenario(scenario);
    sim.scenario.action_difficulty = action_difficulty;
    sim.scenario.riddle_difficulty = riddle_difficulty;
    sim.scenario.ending_tracker = crate::endings::EndingPointTracker::default();
    match scenario {
        ScenarioKind::LetterFromSilentHeaven => {
            let mut rng = Sh2Rng::new(rng_seed);
            let effective_riddle = crate::events::init_new_game(
                &mut sim.scenario.flags,
                &mut sim.scenario.puzzle_state,
                &mut rng,
                riddle_difficulty,
                action_difficulty,
                clear_data,
            );
            // Propagate Extra Riddle unlock (`sh2pc.exe` `0x0048aa78`: `movb $0x3, 0x1dbbff5`).
            sim.scenario.riddle_difficulty = effective_riddle;
        }
        ScenarioKind::BornFromAWish => {
            // `sh2pc.exe` `FlagInit` (`0x0048aa3e`): `orl $0x1000020, 0x1f7a9e0` (flags 5 & 24).
            crate::events::init_new_game_wish(
                &mut sim.scenario.flags,
                &mut sim.scenario.puzzle_state,
            );
        }
    }

    let start_stage = match scenario {
        ScenarioKind::LetterFromSilentHeaven => MAIN_SCENARIO_START_STAGE,
        ScenarioKind::BornFromAWish => WISH_SCENARIO_START_STAGE,
    };
    // Retail SH2 / SH2:EE transitions New Game straight from black screen into opening FMV
    // (`toilet.bik`) without displaying the `NOW LOADING...` footstep screen (`SH2-150`).
    let show_loading_screen = false;
    let manifest = resolve_stage_manifest(start_stage, None, false, show_loading_screen);
    event_engine.reset();
    outdoor_streamer.reset();
    fade.set_black_immediate();
    CampaignPhase::RoomLoading {
        target: RoomLoadTarget {
            stage_id: start_stage,
            room_override: Some(manifest.room_id),
            spawn_pos: sim.scenario.player_position.to_array(),
            spawn_yaw: sim.scenario.player_yaw,
            is_connected_stage: false,
            show_loading_screen,
        },
        connect_step: ConnectStep::StageOverlay,
        assets_ready: false,
    }
}

/// Executes one frame of [`CampaignPhase::ActiveGameplay`] (`PlayableMain` step `4` `0x0020b73c` /
/// `EventMain` `0x001a7810`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_active_gameplay(
    sim: &mut SimulationCore,
    event_engine: &mut EventEngine,
    outdoor_streamer: &mut OutdoorTileStreamer,
    fade: &mut ScreenFadeState,
    cutaway_fade: &mut CutawayFadeState,
    play_time_secs: &mut f64,
    player_locked: &mut bool,
    scratch_stage_items: &mut Vec<ItemListEntry>,
    cached_stage_items_id: &mut Option<u16>,
    input: &CampaignTickInput,
    dt_sec: f32,
    effects: &mut CampaignEffects,
) -> GameplayStepOutcome {
    if dt_sec > 0.0 {
        *play_time_secs += f64::from(dt_sec);
        // `sh2pc.exe` `0x0048eccb..0x0048ed1a`: accumulate low-HP urgency (`< 0.5` HP ratio).
        sim.scenario.ending_tracker.tick_low_hp(
            sim.scenario.health,
            sim.scenario.max_health,
            dt_sec,
        );
        // `sh2pc.exe` `0x0048ed1e..0x0048ed8c`: accumulate Maria escort proximity & bump count
        // whenever Maria is accompanying (`flag 15` / `ACT1_FLAG_MARIA_ACCOMPANYING`).
        if sim.scenario.flags.get(15) {
            if let Some(dist_mm) = input.maria_distance_mm {
                sim.scenario
                    .ending_tracker
                    .tick_maria_escort(dist_mm, input.maria_bumped, dt_sec);
            }
        }
        // `sh2pc.exe` `0x005767e0` (`EvAllTimeFunc` Stage 44 Room `0xba`): Mary's hallway monologue.
        let mut numeric_room = super::room_name::room_name_for_stage_pos(
            sim.scenario.current_stage,
            sim.scenario.player_position.to_array(),
        );
        if numeric_room == 0 {
            numeric_room = crate::fog::room_id_from_stage_code(&sim.scenario.current_room) as u16;
        }
        if sim
            .scenario
            .ending_tracker
            .tick_mary_corridor_monologue(numeric_room, dt_sec)
        {
            sim.scenario
                .flags
                .set(crate::endings::FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE, true);
        }
        // `SLUS_202.28` `EventMain` (`0x001a785c`) / `sh2pc.exe` (`0x0048e81b`) + SH2:EE
        // `Patches/FlashlightClockPush.cpp` Room 208 clock push `sys.flag` mask.
        let cam_y = sim.active_camera_eye(1.0).y;
        sim.scenario.event_main_sys_flags &= crate::world::event_main_sys_flag_mask(
            numeric_room,
            cam_y,
            input.enable_sh2ee_world_fixes,
        );
    }

    // 1. Check `PlayerEventJamesDeadly` (`0x001a78a8`) -> `GameoverMain` (`0x001c4c70`)
    if sim.scenario.health <= 0.0 {
        *player_locked = true;
        fade.start_fade_out(PS2_DEFAULT_FADE_SPEED);
        effects.sound_effects.push(SE_GAME_OVER);
        effects.clear_message = true;
        return GameplayStepOutcome::EnterGameOver;
    }

    // 2. Check overlay menu toggle (`PlayerEventButtonCheck` `0x001a78cc` / `PauseSetType` `0x0020b854`)
    if let Some(menu_kind) = input.overlay_toggle {
        return GameplayStepOutcome::OpenOverlay(menu_kind);
    }

    // 3. Advance cutaway black fade-out/fade-in wrapper (`SLUS_202.28` `EvSubPictureDisplay`
    //    `0x001aca00` steps `0 -> 4 -> 7` on entry and `6 -> 0xb` on exit).
    match cutaway_fade {
        CutawayFadeState::Idle => {}
        CutawayFadeState::FadingOutToCutaway { .. } => {
            *player_locked = true;
            if fade.is_fully_black() {
                if let CutawayFadeState::FadingOutToCutaway {
                    picture,
                    incar_alpha,
                } = std::mem::replace(cutaway_fade, CutawayFadeState::Idle)
                {
                    sim.scenario
                        .cutaway_overlay
                        .set_picture(Some(&picture), &sim.scenario.flags);
                    sim.scenario.cutaway_overlay.set_incar_alpha(incar_alpha);
                    fade.start_fade_in(PS2_DEFAULT_FADE_SPEED, false);
                }
            }
            return GameplayStepOutcome::Continue;
        }
        CutawayFadeState::FadingOutToGameplay { .. } => {
            *player_locked = true;
            if fade.is_fully_black() {
                sim.scenario
                    .cutaway_overlay
                    .set_picture(None, &sim.scenario.flags);
                *player_locked = false;
                fade.start_fade_in(PS2_DEFAULT_FADE_SPEED, false);
                *cutaway_fade = CutawayFadeState::Idle;
            }
            return GameplayStepOutcome::Continue;
        }
    }

    // 4. Advance active `StageProgramRunner` (`ev_prog` / `EventExecProgram` `0x001a9e00`),
    //    or evaluate `EventMainStandard` (`0x001a7b50`) against the active stage's `Event_List` + `Item_List`.
    let stage_id = sim.scenario.current_stage;
    if sim.scenario.prog_runner.is_running() {
        // Gate pad input while fading in a cutaway (`EvSubPictureDisplay` `0x001aca00` step `7`
        // waits for `ScreenEffectFadeCheck() != 0` before checking `shPadTrigger`).
        let fade_blocks_input =
            sim.scenario.cutaway_overlay.active_picture().is_some() && !fade.is_fully_clear();
        let dir_mask = PROG_KEY_UP | PROG_KEY_DOWN | PROG_KEY_LEFT | PROG_KEY_RIGHT;
        let (down, held) = if fade_blocks_input {
            (0, 0)
        } else {
            let down = (if input.action_pressed { PROG_KEY_A } else { 0 })
                | (if input.cancel_pressed { PROG_KEY_B } else { 0 })
                | (input.prog_dir_down & dir_mask);
            let held = down | (input.prog_dir_held & dir_mask);
            (down, held)
        };
        let riddle = sim.scenario.riddle_difficulty;
        let prev_cutaway = sim
            .scenario
            .cutaway_overlay
            .active_picture()
            .map(str::to_owned);
        let (
            still_running,
            walkback_delta,
            requested_demo,
            requested_demo_post_pose,
            open_save_point_id,
            ending_bit,
        ) = {
            let scen = &mut sim.scenario;
            let still_running = scen.prog_runner.tick_with_campaign(
                down,
                held,
                dt_sec,
                &mut scen.flags,
                &mut scen.puzzle_state,
                &mut scen.inventory_mgr,
                &mut scen.cutaway_overlay,
                riddle,
                Some(&scen.playing_info),
                Some(&scen.ending_tracker),
                Some(&mut scen.active_elevator),
            );
            *player_locked = still_running && scen.prog_runner.is_player_locked();
            effects
                .sound_effects
                .extend_from_slice(scen.prog_runner.sound_effects());
            effects
                .stop_sound_effects
                .extend_from_slice(scen.prog_runner.stop_sound_effects());
            for &(se_id, pos) in scen.prog_runner.positional_sound_effects() {
                effects
                    .positional_sounds
                    .push(crate::events::engine::EventPositionalSound {
                        se_id: se_id as i16,
                        pos,
                        volume: 1.0,
                        channel: 0,
                    });
            }
            if let Some(msg_id) = scen.prog_runner.active_message_id() {
                if msg_id >= 0 {
                    effects.stage_message = Some(msg_id as u16);
                }
            } else if !still_running {
                effects.clear_message = true;
            }
            (
                still_running,
                scen.prog_runner.take_walkback_delta(),
                scen.prog_runner.take_requested_demo(),
                scen.prog_runner.take_requested_demo_post_pose(),
                scen.prog_runner.take_open_save_point_id(),
                scen.prog_runner.take_triggered_ending(),
            )
        };
        let next_cutaway = sim
            .scenario
            .cutaway_overlay
            .active_picture()
            .map(str::to_owned);
        let incar_alpha = sim.scenario.cutaway_overlay.incar_alpha();
        if prev_cutaway.is_none() {
            if let Some(picture) = next_cutaway {
                // `EvSubPictureDisplay` step `0 -> 4`: fade 3D gameplay out to black before
                // revealing the cutaway still and fading it in.
                sim.scenario
                    .cutaway_overlay
                    .set_picture(None, &sim.scenario.flags);
                *player_locked = true;
                fade.start_fade_out(PS2_DEFAULT_FADE_SPEED);
                *cutaway_fade = CutawayFadeState::FadingOutToCutaway {
                    picture,
                    incar_alpha,
                };
            }
        } else if let Some(picture) = prev_cutaway {
            if next_cutaway.is_none() && open_save_point_id.is_none() {
                // `EvSubPictureDisplay` step `6 -> 0xb`: hold cutaway still while fading out
                // to black, then hide it and fade 3D gameplay back in.
                sim.scenario
                    .cutaway_overlay
                    .set_picture(Some(&picture), &sim.scenario.flags);
                sim.scenario.cutaway_overlay.set_incar_alpha(incar_alpha);
                *player_locked = true;
                fade.start_fade_out(PS2_DEFAULT_FADE_SPEED);
                *cutaway_fade = CutawayFadeState::FadingOutToGameplay {
                    picture,
                    incar_alpha,
                };
            }
        }
        if let Some(delta) = walkback_delta {
            let target = sim.scenario.player_position + Vec3::from_array(delta);
            sim.snap_player_transform(target, sim.scenario.player_yaw);
        }
        if let Some(demo_no) = requested_demo {
            let rule = AUTO_STAGE_DEMO_RULES.iter().find(|r| r.demo_no == demo_no);
            let (post_pos, post_yaw) = if let Some((pos, yaw)) = requested_demo_post_pose {
                (Some(pos), Some(yaw))
            } else {
                (rule.and_then(|r| r.post_pos), rule.and_then(|r| r.post_yaw))
            };
            return GameplayStepOutcome::StartDemo(DramaDemoTrigger {
                demo_no,
                demo_id: find_drama_demo_by_no(demo_no as u16).map(|d| d.id),
                post_pos,
                post_yaw,
                restore_policy: PostSequenceRestorePolicy::ReevaluateAtPlayerPose,
            });
        }
        if !still_running {
            event_engine.finish_active_program();
        }
        if open_save_point_id.is_some() {
            return GameplayStepOutcome::OpenOverlay(OverlayMenuKind::Save);
        }
        if let Some(ending_bit) = ending_bit {
            let target_ending_stage = match ending_bit {
                0x01 => 48,
                0x02 => 49,
                0x04 => 50,
                0x08 => 51,
                0x10 => 52,
                0x40 => 54,
                _ => stage_id,
            };
            return GameplayStepOutcome::TriggerEnding(target_ending_stage);
        }
    } else {
        let eval_input = EventEvalInput {
            player_pos: sim.scenario.player_position.to_array(),
            player_rot_y: sim.scenario.player_yaw,
            action_pressed: input.action_pressed,
            used_item_id: (input.item_use_id != 0).then_some(input.item_use_id),
            is_dark_room: sim.scenario.is_current_stage_dark(),
            flashlight_on: sim.scenario.is_flashlight_active(),
            action_difficulty: sim.scenario.action_difficulty,
            ..EventEvalInput::default()
        };

        let ev_effects = tick_stage_events(
            event_engine,
            sim,
            stage_id,
            scratch_stage_items,
            cached_stage_items_id,
            &eval_input,
            input,
        );
        if let Some(lock) = ev_effects.player_lock_change {
            *player_locked = lock;
        }
        effects.common_message = ev_effects.common_message;
        effects.stage_message = ev_effects.stage_message;
        effects.clear_message = ev_effects.clear_message;
        effects.sound_effects.extend(ev_effects.sound_effects);
        effects
            .positional_sounds
            .extend(ev_effects.positional_sounds);

        if let Some(prog_no) = ev_effects.active_prog_index {
            sim.scenario.prog_runner.start(
                stage_id as u8,
                prog_no,
                ev_effects.active_prog_completion_flag,
            );
            *player_locked = sim.scenario.prog_runner.is_player_locked();
        }

        if ev_effects.save_point_name_id.is_some() {
            return GameplayStepOutcome::OpenOverlay(OverlayMenuKind::Save);
        }

        if let Some(cmd) = ev_effects.stage_transition {
            return GameplayStepOutcome::RequestTransition {
                current_stage: stage_id,
                command: cmd,
            };
        }
    }

    // Keep PlayingInfo item counters synchronized with InventoryManager.
    sim.scenario.playing_info.item_get = sim.scenario.inventory_mgr.total_items_picked_up();
    sim.scenario.playing_info.hidden_item_get = sim.scenario.inventory_mgr.secret_items_mask();

    // 4. Plan outdoor multi-tile streaming (`LoadBgSync` `0x0020b240` / `loadBgAll_PrepareAround`
    //    `0x002715d0`) and seamlessly update the active outdoor tile room ID + `.cam` rails when
    //    crossing a 20,000 mm `BlockNumber` cell without firing a destructive single-tile
    //    `effects.load_room` sync reload (`SH2-183`).
    //    Zero-alloc fast path: only emits `outdoor_plan` when `(stage_id, gx, gz, mode, quadrant)`
    //    or residency changed, and compares `(area, block_no)` without `format!`.
    let player_pos_arr = sim.scenario.player_position.to_array();
    let mut tile_changed = false;
    if let Some((area, block_no)) = resolve_outdoor_area_block(stage_id, player_pos_arr) {
        if !room_id_matches_outdoor_block(&sim.scenario.current_room, area, block_no) {
            let tile_room_id = format!("{}{block_no:02}", area.prefix());
            sim.scenario.set_current_room(&tile_room_id);
            tile_changed = true;
        }
    }
    if let Some(plan) = outdoor_streamer.plan_if_changed(
        stage_id,
        sim.scenario.player_position.x,
        sim.scenario.player_position.z,
    ) {
        let _ = sim.sync_outdoor_camera_plan(&plan, input.prefer_sh2ee);
        effects.outdoor_plan = Some(plan);
    } else if tile_changed {
        let active_room = sim.scenario.current_room.clone();
        let _ = sim.switch_outdoor_tile_room(&active_room, input.prefer_sh2ee);
    }

    effects.allow_gameplay_sim = !*player_locked;
    GameplayStepOutcome::Continue
}

fn tick_stage_events(
    event_engine: &mut EventEngine,
    sim: &mut SimulationCore,
    stage_id: u16,
    scratch_stage_items: &mut Vec<ItemListEntry>,
    cached_stage_items_id: &mut Option<u16>,
    eval_input: &EventEvalInput,
    input: &CampaignTickInput,
) -> EventEngineEffects {
    if stage_id <= 27 {
        if let Some(act1) = act1_stage_descriptor(stage_id as u8) {
            if *cached_stage_items_id != Some(stage_id) {
                scratch_stage_items.clear();
                scratch_stage_items.extend(act1.item_list.iter().map(|e| e.to_item_list_entry()));
                *cached_stage_items_id = Some(stage_id);
            }
            return tick_retail_stage_events(
                event_engine,
                act1.event_list,
                scratch_stage_items,
                act1.ev_pos,
                stage_id as u8,
                eval_input,
                input.message_dismissed,
                &mut sim.scenario.flags,
                &mut sim.scenario.inventory_mgr,
            );
        }
    }
    if let Some(act2) = stage_table_act2(stage_id) {
        if *cached_stage_items_id != Some(stage_id) {
            scratch_stage_items.clear();
            scratch_stage_items.extend(act2.retail_items.iter().map(|e| e.to_ps2_clamped()));
            *cached_stage_items_id = Some(stage_id);
        }
        return tick_retail_stage_events(
            event_engine,
            act2.event_list,
            scratch_stage_items,
            act2.pos_list,
            stage_id as u8,
            eval_input,
            input.message_dismissed,
            &mut sim.scenario.flags,
            &mut sim.scenario.inventory_mgr,
        );
    }
    EventEngineEffects::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{CampaignOrchestrator, CampaignPhase};
    use crate::config::OutdoorStreamingMode;

    #[test]
    fn cutaway_entry_and_exit_fade_out_and_fade_in_like_slus_001aca00() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim.scenario.current_stage = 3;
        let mut orch = CampaignOrchestrator::from_saved_stage(
            3,
            Some("ob01".to_string()),
            [15856.0, 0.0, -4050.0],
            0.0,
            OutdoorStreamingMode::default(),
            false,
        );
        orch.phase = CampaignPhase::ActiveGameplay;
        orch.fade = ScreenFadeState::default();
        orch.player_locked = false;

        // Start stage 3 prog #2 (`out_p_incar` car map pickup) directly.
        sim.scenario.prog_runner.start(3, 2, 24);
        let idle_input = CampaignTickInput::default();

        // Tick 1: prog_runner step 0 requests `out_p_incar`; gameplay wrapper intercepts it,
        // keeps overlay hidden during fade-out, locks player, and starts fading to black.
        let eff = orch.step(&mut sim, &idle_input, 1.0 / 60.0);
        assert!(sim.scenario.cutaway_overlay.active_picture().is_none());
        assert!(matches!(
            orch.cutaway_fade,
            CutawayFadeState::FadingOutToCutaway { .. }
        ));
        assert!(orch.player_locked);
        assert!(eff.fade_alpha <= 0.05);

        // Advance ~0.9s (> 1.0 / 1.2s): fade reaches black, reveals `out_p_incar`, starts fade-in.
        let mut saw_mid_fade_out = false;
        for _ in 0..60 {
            let e = orch.step(&mut sim, &idle_input, 1.0 / 60.0);
            if e.fade_alpha > 0.2 && e.fade_alpha < 0.8 {
                saw_mid_fade_out = true;
            }
            if sim.scenario.cutaway_overlay.active_picture() == Some("out_p_incar") {
                break;
            }
        }
        assert!(saw_mid_fade_out, "should smoothly fade out before cutaway");
        assert_eq!(
            sim.scenario.cutaway_overlay.active_picture(),
            Some("out_p_incar")
        );
        assert_eq!(orch.cutaway_fade, CutawayFadeState::Idle);
        assert!(
            orch.fade.black_alpha() >= 0.99,
            "newly revealed cutaway should start fading in from black"
        );

        // Input during fade-in must be gated (`EvSubPictureDisplay` step 7).
        let press_a = CampaignTickInput {
            action_pressed: true,
            ..CampaignTickInput::default()
        };
        let _ = orch.step(&mut sim, &press_a, 1.0 / 60.0);
        assert_eq!(sim.scenario.prog_runner.active_message_id(), None);

        // Finish fade-in (~0.9s) -> screen clear, cutaway interactive.
        for _ in 0..60 {
            let _ = orch.step(&mut sim, &idle_input, 1.0 / 60.0);
        }
        assert!(orch.fade.is_fully_clear());

        // Press A to get map message, press A again to dismiss, then run 1.0s map fade.
        let eff_msg = orch.step(&mut sim, &press_a, 1.0 / 60.0);
        assert_eq!(eff_msg.stage_message, Some(28));
        let _ = orch.step(&mut sim, &press_a, 1.0 / 60.0);
        for _ in 0..65 {
            let _ = orch.step(&mut sim, &idle_input, 1.0 / 60.0);
            if matches!(
                orch.cutaway_fade,
                CutawayFadeState::FadingOutToGameplay { .. }
            ) {
                break;
            }
        }
        assert!(
            matches!(
                orch.cutaway_fade,
                CutawayFadeState::FadingOutToGameplay { .. }
            ),
            "closing cutaway should fade out to black before returning to 3D gameplay"
        );
        assert_eq!(
            sim.scenario.cutaway_overlay.active_picture(),
            Some("out_p_incar"),
            "cutaway should remain visible while fading out to black"
        );

        // Advance ~0.9s -> reaches black, hides cutaway, unlocks player, starts gameplay fade-in.
        for _ in 0..60 {
            let _ = orch.step(&mut sim, &idle_input, 1.0 / 60.0);
            if sim.scenario.cutaway_overlay.active_picture().is_none() {
                break;
            }
        }
        assert!(sim.scenario.cutaway_overlay.active_picture().is_none());
        assert!(!orch.player_locked);
        assert_eq!(orch.cutaway_fade, CutawayFadeState::Idle);
        assert!(
            orch.fade.black_alpha() >= 0.99,
            "returning to 3D gameplay should start fading in from black"
        );
    }

    #[test]
    fn sh2_166_overlay_inventory_item_use_transitions_to_gameplay_and_executes_event() {
        use crate::campaign::retail_events::evaluate_retail_stage_events;
        use crate::events::stages_act1::act1_stage_descriptor;
        use crate::events::stages_act2::{
            decode_retail_ev_pos_shape, normalize_retail_event_entry_for_input,
        };
        use crate::events::triggers::EvPosShape;
        use crate::types::EventTriggerKind;

        // Locate a real retail `UseItemLook` event in Act 1 (e.g. Wood Side / Blue Creek / Hospital)
        // whose flag conditions are satisfied at initial state so we test real retail dispatch.
        let mut found = None;
        for stage_id in 4u8..=27u8 {
            let Some(desc) = act1_stage_descriptor(stage_id) else {
                continue;
            };
            let dummy_input = EventEvalInput::default();
            let dummy_inv = crate::inventory::InventoryManager::new();
            let dummy_flags = crate::events::EventFlags::new();
            for &raw in desc.event_list {
                if raw.is_sentinel() {
                    break;
                }
                let entry = normalize_retail_event_entry_for_input(raw, &dummy_input, &dummy_inv);
                if entry.trigger_kind() != Some(EventTriggerKind::UseItemLook) {
                    continue;
                }
                if !crate::events::triggers::check_event_flag_conditions(&entry, &dummy_flags) {
                    continue;
                }
                let item_id = entry.required_item_id();
                if item_id == 0 {
                    continue;
                }
                let Some(shape) = entry.shape_kind().and_then(|sk| {
                    decode_retail_ev_pos_shape(desc.ev_pos, entry.ev_pos_offset(), sk)
                }) else {
                    continue;
                };
                let pos = match shape {
                    EvPosShape::Point { x, y, z } => [x, y, z],
                    EvPosShape::Polyline { y, vertices, len } => {
                        if len > 0 {
                            let [x, z] = vertices[0];
                            [x, y, z]
                        } else {
                            continue;
                        }
                    }
                    EvPosShape::Rectangle { x, y, z, w, d } => [x + w * 0.5, y, z + d * 0.5],
                    EvPosShape::Triangle { y, vertices } => [
                        (vertices[0][0] + vertices[1][0] + vertices[2][0]) / 3.0,
                        y,
                        (vertices[0][1] + vertices[1][1] + vertices[2][1]) / 3.0,
                    ],
                    EvPosShape::Quad { y, vertices } => [
                        (vertices[0][0] + vertices[1][0] + vertices[2][0] + vertices[3][0]) * 0.25,
                        y,
                        (vertices[0][1] + vertices[1][1] + vertices[2][1] + vertices[3][1]) * 0.25,
                    ],
                };
                // Confirm that evaluate_retail_stage_events actually fires for this entry at `pos`.
                let mut probe_flags = crate::events::EventFlags::new();
                let mut probe_inv = crate::inventory::InventoryManager::new();
                probe_inv.item_get_now(item_id);
                let probe_in = EventEvalInput {
                    player_pos: pos,
                    player_rot_y: 0.0,
                    used_item_id: Some(item_id),
                    ..EventEvalInput::default()
                };
                let probe_eff = evaluate_retail_stage_events(
                    desc.event_list,
                    desc.ev_pos,
                    stage_id,
                    &probe_in,
                    &mut probe_flags,
                    &mut probe_inv,
                );
                if probe_eff.common_message.is_some()
                    || probe_eff.stage_message.is_some()
                    || probe_eff.active_prog_index.is_some()
                    || probe_eff.stage_transition.is_some()
                    || probe_inv.count(item_id) == 0
                {
                    found = Some((stage_id, item_id, pos));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        let (stage_id, item_id, pos) =
            found.expect("expected at least one reachable retail UseItemLook event in Act 1");

        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim.scenario.current_stage = u16::from(stage_id);
        sim.snap_player_transform(Vec3::from_array(pos), 0.0);
        sim.scenario.inventory_mgr.item_get_now(item_id);

        let mut orch = CampaignOrchestrator::from_saved_stage(
            u16::from(stage_id),
            None,
            pos,
            0.0,
            OutdoorStreamingMode::default(),
            false,
        );
        orch.phase = CampaignPhase::OverlayMenu(OverlayMenuKind::Inventory);
        orch.fade = ScreenFadeState::default();
        orch.player_locked = false;

        // Dispatch `item_use_id` while `CampaignOrchestrator` is in `OverlayMenu(Inventory)`:
        // must transition out of `OverlayMenu(Inventory)` and immediately execute the item event.
        let use_input = CampaignTickInput {
            item_use_id: item_id,
            ..CampaignTickInput::default()
        };
        let eff = orch.step(&mut sim, &use_input, 1.0 / 60.0);
        assert!(
            !matches!(
                orch.phase,
                CampaignPhase::OverlayMenu(OverlayMenuKind::Inventory)
            ),
            "item use from Inventory must exit OverlayMenu(Inventory), got {:?}",
            orch.phase
        );
        assert!(
            eff.common_message.is_some()
                || eff.stage_message.is_some()
                || sim.scenario.prog_runner.is_running()
                || sim.scenario.inventory_mgr.count(item_id) == 0
                || matches!(orch.phase, CampaignPhase::DoorTransition { .. }),
            "expected retail UseItemLook event for item {item_id} on stage {stage_id} to execute"
        );

        // Next idle tick must remain out of `OverlayMenu(Inventory)` (no re-open loop).
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        assert!(!matches!(
            orch.phase,
            CampaignPhase::OverlayMenu(OverlayMenuKind::Inventory)
        ));
    }

    #[test]
    fn sh2_183_outdoor_tile_crossing_emits_plan_without_destructive_load_room() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim.scenario.current_stage = 4;
        sim.scenario.current_room = "ca11".to_string();
        // Start in `ca11` (cell (0, -1): X in [0, 20000), Z in [-20000, 0)).
        sim.snap_player_transform(Vec3::new(10_000.0, 0.0, -10_000.0), 0.0);

        let mut orch = CampaignOrchestrator::from_saved_stage(
            4,
            Some("ca11".to_string()),
            [10_000.0, 0.0, -10_000.0],
            0.0,
            OutdoorStreamingMode::ClassicPs2LoadBg2x2,
            false,
        );
        orch.phase = CampaignPhase::ActiveGameplay;
        orch.fade = ScreenFadeState::default();
        orch.player_locked = false;

        let idle = CampaignTickInput::default();
        let eff1 = orch.step(&mut sim, &idle, 1.0 / 60.0);
        assert!(
            eff1.outdoor_plan.is_some(),
            "first outdoor gameplay tick must emit an outdoor streaming plan"
        );
        assert!(
            eff1.load_room.is_none(),
            "staying in ca11 must not trigger load_room"
        );

        // Cross the 20,000 mm tile boundary south into `ca12` (cell (0, -2): Z = -20,500 mm).
        sim.scenario.player_position = Vec3::new(10_000.0, 0.0, -20_500.0);
        let eff2 = orch.step(&mut sim, &idle, 1.0 / 60.0);
        assert_eq!(
            sim.scenario.current_room, "ca12",
            "crossing into cell (0, -2) must seamlessly update current_room to ca12"
        );
        assert!(
            eff2.load_room.is_none(),
            "crossing an outdoor tile seam must NOT emit destructive effects.load_room"
        );
        let plan2 = eff2
            .outdoor_plan
            .expect("crossing into ca12 must emit an updated outdoor_plan");
        assert!(plan2.desired_blocks.contains(&12));
    }
}
