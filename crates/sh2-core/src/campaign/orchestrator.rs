//! Unified start-to-credits Campaign Game Loop orchestrator (`gamemain.c` / `PlayableMain`
//! `0x0020b3c0` / `connectMain` `0x0020bf80` / `EventMain` `0x001a7810`).
//!
//! Coordinates `SimulationCore`, `EventEngine`, `EventCameraSystem`, `OutdoorTileStreamer`,
//! `ScreenFadeState`, and `RoomLoadManifest` generation without any `wgpu`/`winit`/`kira`
//! dependencies (`AGENTS.md` §1).

use std::collections::HashSet;

use crate::config::{ActionDifficulty, OutdoorStreamingMode, RiddleDifficulty};
use crate::cutscene::{find_drama_demo_by_no, CutsceneSequenceStep};
use crate::event_camera::{EventCameraSystem, PostSequenceRestorePolicy};
use crate::events::engine::{EventEngine, StageTransitionCommand};
use crate::events::is_connected_stage_pair;
use crate::scenario::ScenarioKind;
use crate::simulation::SimulationCore;
use crate::world::OutdoorTileStreamer;

use super::connect::{step_connect_pipeline, ConnectRequest, ConnectStepOutcome};
use super::cutscene_handoff::{
    apply_post_cutscene_handoff, DramaDemoTrigger, FmvTrigger, AUTO_STAGE_DEMO_RULES,
};
use super::gameplay::{run_active_gameplay, run_prologue, GameplayStepOutcome};
use super::io::{CampaignEffects, CampaignTickInput};
use super::stage_loader::{
    resolve_room_id_for_stage_pos, resolve_stage_manifest, RoomLoadManifest,
};
use super::state::{
    CampaignPhase, ConnectStep, RoomLoadTarget, ScreenFadeState, PS2_DEFAULT_FADE_SPEED,
};

/// Unified start-to-credits Campaign Game Loop orchestrator (`gamemain.c` / `PlayableMain`
/// `0x0020b3c0` / `connectMain` `0x0020bf80`).
#[derive(Debug)]
pub struct CampaignOrchestrator {
    /// Current typed lifecycle phase (`Prologue` .. `EndingCredits`).
    pub phase: CampaignPhase,
    /// `EventMainStandard` (`0x001a7b50`) trigger & action executor.
    pub event_engine: EventEngine,
    /// Cutscene & scripted event camera authority (`EventCameraSystem`).
    pub event_camera: EventCameraSystem,
    /// Outdoor `LoadBg` multi-tile streaming planner (`loadbg_all.c`).
    pub outdoor_streamer: OutdoorTileStreamer,
    /// Deterministic screen fade ramp (`ScreenEffectManager` `0x0020df30`).
    pub fade: ScreenFadeState,
    /// Toggleable SH2:EE `DelayedFadeIn.cpp` (`-0.133334f` black hold) vs 1:1 vanilla PS2 fade-in.
    pub delayed_fade_in_fix: bool,
    /// Cumulative campaign play time in seconds (`GameTimerCountUp` `0x002902d0`).
    pub play_time_secs: f64,
    /// Most recently loaded stage/room manifest.
    pub active_manifest: Option<RoomLoadManifest>,
    /// Set of `demo_no` cutscenes already triggered during this session.
    pub triggered_demos: HashSet<u32>,
    /// `true` while `EventEngine` or a cutscene locks player locomotion.
    pub player_locked: bool,
}

impl CampaignOrchestrator {
    /// Starts a fresh New Game campaign session (`PlayableMain` step `0` `0x0020b414`),
    /// entering [`CampaignPhase::Prologue`] with immediate black screen (`ScreenEffectFadeStart(2)`).
    #[must_use]
    pub fn new_game(
        scenario: ScenarioKind,
        action_difficulty: ActionDifficulty,
        riddle_difficulty: RiddleDifficulty,
        streaming_mode: OutdoorStreamingMode,
        delayed_fade_in_fix: bool,
    ) -> Self {
        Self {
            phase: CampaignPhase::Prologue {
                scenario,
                action_difficulty,
                riddle_difficulty,
            },
            event_engine: EventEngine::new(),
            event_camera: EventCameraSystem::new(),
            outdoor_streamer: OutdoorTileStreamer::new(streaming_mode),
            fade: ScreenFadeState::new_black(),
            delayed_fade_in_fix,
            play_time_secs: 0.0,
            active_manifest: None,
            triggered_demos: HashSet::new(),
            player_locked: true,
        }
    }

    /// Starts a campaign session from an existing stage/room/pose (e.g. `Continue` / Save Load),
    /// entering [`CampaignPhase::RoomLoading`] directly at `connectMain` step `0`.
    #[must_use]
    pub fn from_saved_stage(
        stage_id: u16,
        room_override: Option<String>,
        spawn_pos: [f32; 3],
        spawn_yaw: f32,
        streaming_mode: OutdoorStreamingMode,
        delayed_fade_in_fix: bool,
    ) -> Self {
        Self {
            phase: CampaignPhase::RoomLoading {
                target: RoomLoadTarget {
                    stage_id,
                    room_override,
                    spawn_pos,
                    spawn_yaw,
                    is_connected_stage: false,
                    show_loading_screen: true,
                },
                connect_step: ConnectStep::StageOverlay,
                assets_ready: false,
            },
            event_engine: EventEngine::new(),
            event_camera: EventCameraSystem::new(),
            outdoor_streamer: OutdoorTileStreamer::new(streaming_mode),
            fade: ScreenFadeState::new_black(),
            delayed_fade_in_fix,
            play_time_secs: 0.0,
            active_manifest: None,
            triggered_demos: HashSet::new(),
            player_locked: true,
        }
    }

    /// Notifies the orchestrator that the thin `sh2-app` shell has finished loading the room's
    /// `.map`, `.cld`, GPU textures, and audio banks (`lisSync` / `fsSync` `0x0020c154`).
    pub fn notify_room_assets_loaded(&mut self) {
        match &mut self.phase {
            CampaignPhase::RoomLoading { assets_ready, .. }
            | CampaignPhase::DoorTransition { assets_ready, .. } => {
                *assets_ready = true;
            }
            _ => {}
        }
    }

    /// Initiates a scripted or door-triggered stage/room transition (`EventExecMove` `0x001aa670`).
    pub fn request_transition(
        &mut self,
        current_stage: u16,
        mut cmd: StageTransitionCommand,
        room_override: Option<String>,
    ) {
        let target_stage = if cmd.dest_stage != 0 {
            cmd.dest_stage as u16
        } else {
            current_stage
        };
        if cmd.dest_stage != 0 {
            cmd.is_connected_stage = is_connected_stage_pair(current_stage as u8, cmd.dest_stage);
        }
        let resolved_room = room_override
            .is_none()
            .then(|| resolve_room_id_for_stage_pos(target_stage, cmd.dest_pos))
            .flatten();
        let effective_room = room_override.as_deref().or(resolved_room.as_deref());
        let manifest =
            resolve_stage_manifest(target_stage, effective_room, cmd.is_connected_stage, false);
        self.fade.start_fade_out(PS2_DEFAULT_FADE_SPEED);
        self.player_locked = true;
        self.phase = CampaignPhase::DoorTransition {
            command: cmd,
            resolved_room: manifest.room_id,
            connect_step: ConnectStep::StageOverlay,
            fade_out_complete: self.fade.is_fully_black(),
            assets_ready: false,
            loading_screen_active: false,
        };
    }

    /// Explicitly triggers a `.dds` real-time 3D cutscene (`DramaDemoMain` `0x001b0f00`).
    pub fn start_drama_demo(&mut self, trigger: DramaDemoTrigger, effects: &mut CampaignEffects) {
        self.start_drama_demo_at_cursor(trigger, 0, effects);
    }

    fn start_drama_demo_at_cursor(
        &mut self,
        trigger: DramaDemoTrigger,
        chain_cursor: usize,
        effects: &mut CampaignEffects,
    ) {
        self.triggered_demos.insert(trigger.demo_no);
        self.player_locked = true;
        self.phase = CampaignPhase::DramaDemo {
            demo_no: trigger.demo_no,
            post_pos: trigger.post_pos,
            post_yaw: trigger.post_yaw,
            restore_policy: trigger.restore_policy,
            chain_cursor,
        };
        effects.start_drama_demo = Some(trigger);
    }

    /// Explicitly triggers a pre-rendered `.bik` FMV (`PlayableMain` steps `13..=14`).
    pub fn start_fmv(&mut self, trigger: FmvTrigger, effects: &mut CampaignEffects) {
        self.player_locked = true;
        self.phase = CampaignPhase::FmvPlayback {
            movie_id: trigger.movie_id,
            post_stage: trigger.post_stage,
            post_pos: trigger.post_pos,
            post_yaw: trigger.post_yaw,
            post_demo: trigger.post_demo.clone(),
            completion_flag: trigger.completion_flag,
            chain_cursor: trigger.chain_cursor,
        };
        effects.start_fmv = Some(trigger);
    }

    /// Transitions into `EndingCredits` (`GameendMain` `0x001c4830` -> `ResultMain` `0x00293e40`).
    pub fn trigger_ending(&mut self, ending_stage: u16) {
        self.player_locked = true;
        self.fade.start_fade_in(PS2_DEFAULT_FADE_SPEED, false);
        self.phase = CampaignPhase::EndingCredits {
            ending_stage,
            showing_results: false,
        };
    }

    /// Advances the unified campaign loop by one frame (`PlayableMain` `0x0020b3c0`).
    pub fn step(
        &mut self,
        sim: &mut SimulationCore,
        input: &CampaignTickInput,
        dt_sec: f32,
    ) -> CampaignEffects {
        self.fade.tick(dt_sec);
        let mut effects = CampaignEffects {
            fade_alpha: self.fade.black_alpha(),
            ..CampaignEffects::default()
        };

        match self.phase.clone() {
            CampaignPhase::Prologue {
                scenario,
                action_difficulty,
                riddle_difficulty,
            } => {
                self.phase = run_prologue(
                    sim,
                    &mut self.event_engine,
                    &mut self.outdoor_streamer,
                    &mut self.fade,
                    scenario,
                    action_difficulty,
                    riddle_difficulty,
                    &mut effects,
                );
            }
            CampaignPhase::RoomLoading {
                target,
                connect_step,
                assets_ready,
            } => {
                let req = ConnectRequest {
                    target_stage: target.stage_id,
                    room_override: target.room_override.as_deref(),
                    spawn_pos: target.spawn_pos,
                    spawn_yaw: target.spawn_yaw,
                    is_connected_stage: target.is_connected_stage,
                    show_loading_screen: target.show_loading_screen,
                    door_command: None,
                    prefer_sh2ee: input.prefer_sh2ee,
                };
                let outcome = step_connect_pipeline(
                    sim,
                    &mut self.event_engine,
                    &mut self.fade,
                    &mut self.active_manifest,
                    &self.triggered_demos,
                    self.delayed_fade_in_fix,
                    connect_step,
                    assets_ready,
                    target.show_loading_screen,
                    &req,
                    &mut effects,
                );
                self.finish_connect_outcome(outcome, &mut effects, |next_step, _| {
                    CampaignPhase::RoomLoading {
                        target,
                        connect_step: next_step,
                        assets_ready,
                    }
                });
            }
            CampaignPhase::ActiveGameplay => {
                if let Some(ref manifest) = self.active_manifest {
                    effects.active_room_pickups = crate::world::resolve_active_room_pickups(
                        &manifest.room_pickups,
                        &sim.scenario.flags,
                        sim.scenario.action_difficulty,
                        manifest.is_outdoor,
                        sim.scenario.player_position.to_array(),
                    );
                }
                let outcome = run_active_gameplay(
                    sim,
                    &mut self.event_engine,
                    &mut self.outdoor_streamer,
                    &mut self.fade,
                    &mut self.play_time_secs,
                    &mut self.player_locked,
                    input,
                    dt_sec,
                    &mut effects,
                );
                match outcome {
                    GameplayStepOutcome::Continue => {}
                    GameplayStepOutcome::EnterGameOver => {
                        self.phase = CampaignPhase::GameOver;
                    }
                    GameplayStepOutcome::OpenOverlay(kind) => {
                        self.phase = CampaignPhase::OverlayMenu(kind);
                    }
                    GameplayStepOutcome::RequestTransition {
                        current_stage,
                        command,
                    } => {
                        self.request_transition(current_stage, command, None);
                    }
                    GameplayStepOutcome::TriggerEnding(ending_stage) => {
                        self.trigger_ending(ending_stage);
                    }
                }
            }
            CampaignPhase::DoorTransition {
                command,
                resolved_room,
                connect_step,
                mut fade_out_complete,
                assets_ready,
                loading_screen_active,
            } => {
                if !fade_out_complete {
                    if !self.fade.is_fully_black() {
                        effects.fade_alpha = self.fade.black_alpha();
                        return effects;
                    }
                    fade_out_complete = true;
                }
                let target_stage = if command.dest_stage != 0 {
                    command.dest_stage as u16
                } else {
                    sim.scenario.current_stage
                };
                let is_inter_stage =
                    command.dest_stage != 0 && target_stage != sim.scenario.current_stage;
                let req = ConnectRequest {
                    target_stage,
                    room_override: Some(&resolved_room),
                    spawn_pos: command.dest_pos,
                    spawn_yaw: command.dest_rot_y,
                    is_connected_stage: command.is_connected_stage,
                    show_loading_screen: is_inter_stage,
                    door_command: Some(command),
                    prefer_sh2ee: input.prefer_sh2ee,
                };
                let outcome = step_connect_pipeline(
                    sim,
                    &mut self.event_engine,
                    &mut self.fade,
                    &mut self.active_manifest,
                    &self.triggered_demos,
                    self.delayed_fade_in_fix,
                    connect_step,
                    assets_ready,
                    loading_screen_active,
                    &req,
                    &mut effects,
                );
                self.finish_connect_outcome(outcome, &mut effects, |next_step, ls_active| {
                    CampaignPhase::DoorTransition {
                        command,
                        resolved_room,
                        connect_step: next_step,
                        fade_out_complete,
                        assets_ready,
                        loading_screen_active: ls_active,
                    }
                });
            }
            CampaignPhase::DramaDemo {
                demo_no,
                post_pos,
                post_yaw,
                restore_policy,
                chain_cursor,
            } => {
                if input.cancel_pressed {
                    for rule in AUTO_STAGE_DEMO_RULES {
                        if rule.demo_no == demo_no {
                            sim.scenario.flags.set(rule.completion_flag, true);
                        }
                    }
                    apply_post_cutscene_handoff(
                        sim,
                        &mut self.event_camera,
                        post_pos,
                        post_yaw,
                        restore_policy,
                    );
                    self.player_locked = false;
                    self.phase = CampaignPhase::ActiveGameplay;
                    effects.allow_gameplay_sim = true;
                } else if input.drama_demo_finished {
                    let next_step = find_drama_demo_by_no(demo_no as u16)
                        .and_then(|info| info.sequence_chain.get(chain_cursor + 1).copied());
                    match next_step {
                        Some(CutsceneSequenceStep::FmvMovie { movie_id }) => {
                            let post_demo = find_drama_demo_by_no(demo_no as u16)
                                .and_then(|info| info.sequence_chain.get(chain_cursor + 2).copied())
                                .and_then(|step| match step {
                                    CutsceneSequenceStep::InEngineDemo { demo_no: next_demo } => {
                                        Some(Box::new(DramaDemoTrigger {
                                            demo_no: u32::from(next_demo),
                                            demo_id: find_drama_demo_by_no(next_demo).map(|d| d.id),
                                            post_pos,
                                            post_yaw,
                                            restore_policy,
                                        }))
                                    }
                                    _ => None,
                                });
                            self.start_fmv(
                                FmvTrigger {
                                    movie_id,
                                    post_stage: None,
                                    post_pos,
                                    post_yaw,
                                    post_demo,
                                    completion_flag: None,
                                    chain_cursor: chain_cursor + 1,
                                },
                                &mut effects,
                            );
                        }
                        Some(CutsceneSequenceStep::InEngineDemo { demo_no: next_demo }) => {
                            self.start_drama_demo_at_cursor(
                                DramaDemoTrigger {
                                    demo_no: u32::from(next_demo),
                                    demo_id: find_drama_demo_by_no(next_demo).map(|d| d.id),
                                    post_pos,
                                    post_yaw,
                                    restore_policy,
                                },
                                chain_cursor + 1,
                                &mut effects,
                            );
                        }
                        _ => {
                            for rule in AUTO_STAGE_DEMO_RULES {
                                if rule.demo_no == demo_no {
                                    sim.scenario.flags.set(rule.completion_flag, true);
                                }
                            }
                            apply_post_cutscene_handoff(
                                sim,
                                &mut self.event_camera,
                                post_pos,
                                post_yaw,
                                restore_policy,
                            );
                            self.player_locked = false;
                            self.phase = CampaignPhase::ActiveGameplay;
                            effects.allow_gameplay_sim = true;
                        }
                    }
                }
            }
            CampaignPhase::FmvPlayback {
                post_stage,
                post_pos,
                post_yaw,
                post_demo,
                completion_flag,
                chain_cursor,
                ..
            } => {
                if input.cancel_pressed {
                    if let Some(flag) = completion_flag {
                        sim.scenario.flags.set(flag, true);
                    }
                    if let Some(next_stage) = post_stage {
                        let cmd = StageTransitionCommand {
                            dest_stage: next_stage as u8,
                            stage_change_mode: next_stage as u8,
                            is_connected_stage: is_connected_stage_pair(
                                sim.scenario.current_stage as u8,
                                next_stage as u8,
                            ),
                            observation_mode: false,
                            dest_pos: post_pos
                                .unwrap_or_else(|| sim.scenario.player_position.to_array()),
                            dest_rot_y: post_yaw.unwrap_or(sim.scenario.player_yaw),
                        };
                        self.request_transition(sim.scenario.current_stage, cmd, None);
                    } else {
                        apply_post_cutscene_handoff(
                            sim,
                            &mut self.event_camera,
                            post_pos,
                            post_yaw,
                            PostSequenceRestorePolicy::ReevaluateAtPlayerPose,
                        );
                        self.player_locked = false;
                        self.phase = CampaignPhase::ActiveGameplay;
                        effects.allow_gameplay_sim = true;
                    }
                } else if input.fmv_finished {
                    if let Some(flag) = completion_flag {
                        sim.scenario.flags.set(flag, true);
                    }
                    if let Some(demo) = post_demo {
                        self.start_drama_demo_at_cursor(*demo, chain_cursor + 1, &mut effects);
                    } else if let Some(next_stage) = post_stage {
                        let cmd = StageTransitionCommand {
                            dest_stage: next_stage as u8,
                            stage_change_mode: next_stage as u8,
                            is_connected_stage: is_connected_stage_pair(
                                sim.scenario.current_stage as u8,
                                next_stage as u8,
                            ),
                            observation_mode: false,
                            dest_pos: post_pos
                                .unwrap_or_else(|| sim.scenario.player_position.to_array()),
                            dest_rot_y: post_yaw.unwrap_or(sim.scenario.player_yaw),
                        };
                        self.request_transition(sim.scenario.current_stage, cmd, None);
                    } else {
                        apply_post_cutscene_handoff(
                            sim,
                            &mut self.event_camera,
                            post_pos,
                            post_yaw,
                            PostSequenceRestorePolicy::ReevaluateAtPlayerPose,
                        );
                        self.player_locked = false;
                        self.phase = CampaignPhase::ActiveGameplay;
                        effects.allow_gameplay_sim = true;
                    }
                }
            }
            CampaignPhase::OverlayMenu(kind) => {
                if kind.advances_game_timer() && dt_sec > 0.0 {
                    self.play_time_secs += f64::from(dt_sec);
                }
                if input.overlay_toggle == Some(kind) || input.cancel_pressed {
                    self.phase = CampaignPhase::ActiveGameplay;
                }
            }
            CampaignPhase::GameOver => {
                if input.confirm_terminal_exit || input.action_pressed {
                    effects.return_to_title = true;
                }
            }
            CampaignPhase::EndingCredits {
                ending_stage,
                showing_results,
            } => {
                if input.confirm_terminal_exit || input.action_pressed {
                    if !showing_results {
                        self.phase = CampaignPhase::EndingCredits {
                            ending_stage,
                            showing_results: true,
                        };
                    } else {
                        effects.return_to_title = true;
                    }
                }
            }
        }

        effects.fade_alpha = self.fade.black_alpha();
        effects
    }

    fn finish_connect_outcome<F>(
        &mut self,
        outcome: ConnectStepOutcome,
        effects: &mut CampaignEffects,
        make_continue: F,
    ) where
        F: FnOnce(ConnectStep, bool) -> CampaignPhase,
    {
        match outcome {
            ConnectStepOutcome::Continue {
                next_step,
                loading_screen_active,
            } => {
                self.phase = make_continue(next_step, loading_screen_active);
            }
            ConnectStepOutcome::StartDemo(demo) => {
                self.start_drama_demo(demo, effects);
            }
            ConnectStepOutcome::StartFmv(fmv) => {
                self.start_fmv(fmv, effects);
            }
            ConnectStepOutcome::StartEnding(stage) => {
                self.trigger_ending(stage);
            }
            ConnectStepOutcome::EnterGameplay => {
                self.player_locked = false;
                self.phase = CampaignPhase::ActiveGameplay;
                effects.allow_gameplay_sim = true;
            }
        }
    }
}
