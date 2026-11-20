//! Active-gameplay (`EventMain` `0x001a7810` / `EventMainStandard` `0x001a7b50`) and
//! new-game prologue (`PlayableMain` step `0` `0x0020b414`) handlers for [`CampaignOrchestrator`].

use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::events::engine::{EventEngine, EventEngineEffects, StageTransitionCommand};
use crate::events::stages_act1::act1_stage_descriptor;
use crate::events::stages_act2::stage_table_act2;
use crate::events::triggers::EventEvalInput;
use crate::events::{ClearData, Sh2Rng};
use crate::scenario::ScenarioKind;
use crate::simulation::SimulationCore;
use crate::world::OutdoorTileStreamer;

use super::io::{CampaignEffects, CampaignTickInput};
use super::retail_events::evaluate_retail_stage_events;
use super::stage_loader::{resolve_outdoor_room_id, resolve_stage_manifest};
use super::state::{
    CampaignPhase, ConnectStep, OverlayMenuKind, RoomLoadTarget, ScreenFadeState,
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
}

/// Executes `PlayableMain` step `0` (`0x0020b414`: `FlagInit`, `ItemDataInit`,
/// `ScreenEffectFadeStart(2)`) and returns the initial [`CampaignPhase::RoomLoading`] phase.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_prologue(
    sim: &mut SimulationCore,
    event_engine: &mut EventEngine,
    outdoor_streamer: &mut OutdoorTileStreamer,
    fade: &mut ScreenFadeState,
    scenario: ScenarioKind,
    action_difficulty: ActionDifficulty,
    riddle_difficulty: RiddleDifficulty,
    effects: &mut CampaignEffects,
) -> CampaignPhase {
    sim.select_scenario(scenario);
    sim.scenario.action_difficulty = action_difficulty;
    sim.scenario.riddle_difficulty = riddle_difficulty;
    if scenario == ScenarioKind::LetterFromSilentHeaven {
        let mut rng = Sh2Rng::new(0x1999_0901);
        let _ = crate::events::init_new_game(
            &mut sim.scenario.flags,
            &mut sim.scenario.puzzle_state,
            &mut rng,
            riddle_difficulty,
            action_difficulty,
            ClearData::default(),
        );
    }

    let start_stage = match scenario {
        ScenarioKind::LetterFromSilentHeaven => MAIN_SCENARIO_START_STAGE,
        ScenarioKind::BornFromAWish => WISH_SCENARIO_START_STAGE,
    };
    let manifest = resolve_stage_manifest(start_stage, None, false, true);
    event_engine.reset();
    outdoor_streamer.reset();
    fade.set_black_immediate();
    effects.show_loading_screen = Some(manifest.stage_name.to_string());
    CampaignPhase::RoomLoading {
        target: RoomLoadTarget {
            stage_id: start_stage,
            room_override: Some(manifest.room_id),
            spawn_pos: sim.scenario.player_position.to_array(),
            spawn_yaw: sim.scenario.player_yaw,
            is_connected_stage: false,
            show_loading_screen: true,
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
    play_time_secs: &mut f64,
    player_locked: &mut bool,
    input: &CampaignTickInput,
    dt_sec: f32,
    effects: &mut CampaignEffects,
) -> GameplayStepOutcome {
    if dt_sec > 0.0 {
        *play_time_secs += f64::from(dt_sec);
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

    // 3. Evaluate `EventMainStandard` (`0x001a7b50`) against the active stage's `Event_List`
    let stage_id = sim.scenario.current_stage;
    let eval_input = EventEvalInput {
        player_pos: sim.scenario.player_position.to_array(),
        player_rot_y: sim.scenario.player_yaw,
        action_pressed: input.action_pressed,
        used_item_id: (input.item_use_id != 0).then_some(input.item_use_id),
        action_difficulty: sim.scenario.action_difficulty,
        ..EventEvalInput::default()
    };

    let ev_effects = tick_stage_events(event_engine, sim, stage_id, &eval_input, input);
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

    if let Some(cmd) = ev_effects.stage_transition {
        return GameplayStepOutcome::RequestTransition {
            current_stage: stage_id,
            command: cmd,
        };
    }

    // 4. Plan outdoor multi-tile streaming (`LoadBgSync` `0x0020b240`) and switch active outdoor
    //    tile (`.map`, `.cld`, `.cam`) seamlessly when crossing a 40,000 mm `BlockNumber` cell.
    let plan = outdoor_streamer.plan(
        stage_id,
        sim.scenario.player_position.x,
        sim.scenario.player_position.z,
    );
    if plan.area.is_some() {
        if let Some(tile_room_id) =
            resolve_outdoor_room_id(stage_id, sim.scenario.player_position.to_array())
        {
            if !sim
                .scenario
                .current_room
                .eq_ignore_ascii_case(&tile_room_id)
            {
                let manifest = resolve_stage_manifest(stage_id, Some(&tile_room_id), true, false);
                let _ = sim.transition_stage_with_camera(
                    stage_id,
                    manifest.stage_name,
                    Some(&tile_room_id),
                    sim.scenario.player_position,
                    sim.scenario.player_yaw,
                    input.prefer_sh2ee,
                );
                effects.load_room = Some(manifest);
            }
        }
        effects.outdoor_plan = Some(plan);
    }

    effects.allow_gameplay_sim = !*player_locked;
    GameplayStepOutcome::Continue
}

fn tick_stage_events(
    _event_engine: &mut EventEngine,
    sim: &mut SimulationCore,
    stage_id: u16,
    eval_input: &EventEvalInput,
    _input: &CampaignTickInput,
) -> EventEngineEffects {
    if stage_id <= 27 {
        if let Some(act1) = act1_stage_descriptor(stage_id as u8) {
            return evaluate_retail_stage_events(
                act1.event_list,
                act1.ev_pos,
                stage_id as u8,
                eval_input,
                &mut sim.scenario.flags,
                &sim.scenario.inventory_mgr,
            );
        }
    }
    if let Some(act2) = stage_table_act2(stage_id) {
        return evaluate_retail_stage_events(
            act2.event_list,
            act2.pos_list,
            stage_id as u8,
            eval_input,
            &mut sim.scenario.flags,
            &sim.scenario.inventory_mgr,
        );
    }
    EventEngineEffects::default()
}
