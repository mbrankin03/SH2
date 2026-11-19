//! Shared `connectMain` (`SLUS_202.28` `0x0020bf80`, jump table `0x003962d0`) 8-step stage & room
//! transition sequencer used by both initial `RoomLoading` and live `DoorTransition` phases.

use std::collections::HashSet;

use glam::Vec3;

use crate::events::engine::{EventEngine, StageTransitionCommand};
use crate::simulation::SimulationCore;

use super::cutscene_handoff::{evaluate_auto_stage_demo, DramaDemoTrigger};
use super::stage_loader::{resolve_stage_manifest, RoomLoadManifest};
use super::state::{ConnectStep, ScreenFadeState, PS2_DEFAULT_FADE_SPEED};
use super::CampaignEffects;

/// Parameters describing a `connectMain` transition target.
pub(crate) struct ConnectRequest<'a> {
    pub(crate) target_stage: u16,
    pub(crate) room_override: Option<&'a str>,
    pub(crate) spawn_pos: [f32; 3],
    pub(crate) spawn_yaw: f32,
    pub(crate) is_connected_stage: bool,
    pub(crate) show_loading_screen: bool,
    pub(crate) door_command: Option<StageTransitionCommand>,
    pub(crate) prefer_sh2ee: bool,
}

/// Outcome of advancing `connectMain` by one step.
pub(crate) enum ConnectStepOutcome {
    /// Stay in the loading/transition phase at `next_step` with updated `loading_screen_active`.
    Continue {
        next_step: ConnectStep,
        loading_screen_active: bool,
    },
    /// `connectMain` step `7` completed; transition into an automatic stage-entry `DramaDemo`.
    StartDemo(DramaDemoTrigger),
    /// `connectMain` step `7` completed on an ending stage (`48..=52`); start `EndingCredits`.
    StartEnding(u16),
    /// `connectMain` step `7` completed; enter `ActiveGameplay`.
    EnterGameplay,
}

/// Advances one step of `connectMain` (`0x0020bf80`: `StgOverlay` -> `connectStageInit` ->
/// `lisSync` -> `shCharacterSetPosAfterDemo` -> `connectRoomInit` -> `lisSync` ->
/// `connectPlayableInit` -> `Complete` + `ScreenEffectFadeStart(4)`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn step_connect_pipeline(
    sim: &mut SimulationCore,
    event_engine: &mut EventEngine,
    fade: &mut ScreenFadeState,
    active_manifest: &mut Option<RoomLoadManifest>,
    triggered_demos: &HashSet<u32>,
    delayed_fade_in_fix: bool,
    connect_step: ConnectStep,
    assets_ready: bool,
    loading_screen_active: bool,
    req: &ConnectRequest<'_>,
    effects: &mut CampaignEffects,
) -> ConnectStepOutcome {
    match connect_step {
        ConnectStep::StageOverlay => {
            let manifest = resolve_stage_manifest(
                req.target_stage,
                req.room_override,
                req.is_connected_stage,
                req.show_loading_screen,
            );
            let mut ls_active = loading_screen_active;
            if req.show_loading_screen && !ls_active {
                effects.show_loading_screen = Some(manifest.stage_name.to_string());
                ls_active = true;
            }
            *active_manifest = Some(manifest);
            ConnectStepOutcome::Continue {
                next_step: ConnectStep::StageInit,
                loading_screen_active: ls_active,
            }
        }
        ConnectStep::StageInit => {
            event_engine.reset();
            ConnectStepOutcome::Continue {
                next_step: ConnectStep::WaitStageSync,
                loading_screen_active,
            }
        }
        ConnectStep::WaitStageSync => ConnectStepOutcome::Continue {
            next_step: ConnectStep::PlacePlayer,
            loading_screen_active,
        },
        ConnectStep::PlacePlayer => {
            sim.snap_player_transform(Vec3::from_array(req.spawn_pos), req.spawn_yaw);
            ConnectStepOutcome::Continue {
                next_step: ConnectStep::RoomInit,
                loading_screen_active,
            }
        }
        ConnectStep::RoomInit => {
            let manifest = active_manifest.clone().unwrap_or_else(|| {
                resolve_stage_manifest(
                    req.target_stage,
                    req.room_override,
                    req.is_connected_stage,
                    req.show_loading_screen,
                )
            });
            if let Some(cmd) = req.door_command {
                let _ = sim.apply_stage_transition(&cmd, Some(&manifest.room_id), req.prefer_sh2ee);
            } else {
                let _ = sim.transition_stage_with_camera(
                    manifest.stage_id,
                    manifest.stage_name,
                    Some(&manifest.room_id),
                    Vec3::from_array(req.spawn_pos),
                    req.spawn_yaw,
                    req.prefer_sh2ee,
                );
            }
            effects.load_room = Some(manifest);
            ConnectStepOutcome::Continue {
                next_step: ConnectStep::WaitRoomSync,
                loading_screen_active,
            }
        }
        ConnectStep::WaitRoomSync => {
            if assets_ready {
                ConnectStepOutcome::Continue {
                    next_step: ConnectStep::PlayableInit,
                    loading_screen_active,
                }
            } else {
                ConnectStepOutcome::Continue {
                    next_step: ConnectStep::WaitRoomSync,
                    loading_screen_active,
                }
            }
        }
        ConnectStep::PlayableInit => {
            effects.bgm_stage_id = Some(req.target_stage as u32);
            ConnectStepOutcome::Continue {
                next_step: ConnectStep::Complete,
                loading_screen_active,
            }
        }
        ConnectStep::Complete => {
            if loading_screen_active {
                effects.hide_loading_screen = true;
            }
            fade.start_fade_in(PS2_DEFAULT_FADE_SPEED, delayed_fade_in_fix);

            if let Some(demo) =
                evaluate_auto_stage_demo(req.target_stage, &sim.scenario.flags, triggered_demos)
            {
                ConnectStepOutcome::StartDemo(demo)
            } else if (48..=52).contains(&req.target_stage) {
                ConnectStepOutcome::StartEnding(req.target_stage)
            } else {
                ConnectStepOutcome::EnterGameplay
            }
        }
    }
}
