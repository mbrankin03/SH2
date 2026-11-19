//! Automatic `.dds` real-time cutscene (`DramaDemoMain` `0x001b0f00`) and `.bik`/`.pss` FMV
//! (`EvSubMovieStart` `0x001ad780`) trigger evaluation & zero-smear post-cutscene camera handoff
//! (`shCharacterSetPosAfterDemo` `0x0011f770` + `VC_RD_WARP_IN_F`).

use std::collections::HashSet;

use glam::Vec3;

use crate::cutscene::{
    find_drama_demo_by_no, CS_INTRO_BATHROOM_DEMO_NO, CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO,
};
use crate::event_camera::{EventCameraSystem, PostSequenceRestorePolicy, VC_ALL_WARP_FLAGS};
use crate::events::EventFlags;
use crate::simulation::SimulationCore;

/// Request emitted by [`CampaignOrchestrator`](super::CampaignOrchestrator) to start a `.dds`
/// real-time 3D cutscene (`DramaDemoInit` `0x001b1130`).
#[derive(Debug, Clone, PartialEq)]
pub struct DramaDemoTrigger {
    /// Retail drama demo number in `DRAMA_DEMO_CATALOG` (e.g. `1` = `first_toilet`, `2` = `tenbou`).
    pub demo_no: u32,
    /// Optional catalog short ID if present in `DRAMA_DEMO_CATALOG`.
    pub demo_id: Option<&'static str>,
    /// Optional post-cutscene `CharaSetPos` (`shCharacterSetPosAfterDemo` `0x0011f770`) position.
    pub post_pos: Option<[f32; 3]>,
    /// Optional post-cutscene player yaw in radians.
    pub post_yaw: Option<f32>,
    /// Camera rail restoration policy when returning to gameplay.
    pub restore_policy: PostSequenceRestorePolicy,
}

/// Request emitted by [`CampaignOrchestrator`](super::CampaignOrchestrator) to start a pre-rendered
/// `.bik`/`.pss` FMV (`PlayableMain` steps `13..=14` `0x0020bb28..0x0020bc73`).
#[derive(Debug, Clone, PartialEq)]
pub struct FmvTrigger {
    /// Canonical retail movie identifier (e.g. `"open_voc"`, `"deai"`, `"murder"`, `"ending"`).
    pub movie_id: &'static str,
    /// Optional destination stage to transition into once the FMV finishes.
    pub post_stage: Option<u16>,
    /// Optional destination player position after the FMV finishes.
    pub post_pos: Option<[f32; 3]>,
    /// Optional destination player yaw after the FMV finishes.
    pub post_yaw: Option<f32>,
}

/// Stage-entry / flag-driven automatic cutscene rule matching `Stage_Data.p_demo`
/// (`EventMain` `0x001a7aa4`) and `Event_List` `Always` triggers (`stg_toilet.c`,
/// `stg_ovservation.c`, `stg_end_*.c`).
#[derive(Debug, Clone, Copy)]
pub struct AutoDemoRule {
    /// Retail stage ID (`1..=58`).
    pub stage_id: u16,
    /// Required flag that must be `false` (unset) to fire, and is set upon completion.
    pub completion_flag: usize,
    /// Drama demo number in `DRAMA_DEMO_CATALOG`.
    pub demo_no: u32,
    /// Optional post-cutscene `CharaSetPos` `[x, y, z]` in PS2 mm.
    pub post_pos: Option<[f32; 3]>,
    /// Optional post-cutscene facing yaw in radians.
    pub post_yaw: Option<f32>,
}

/// Verified stage-entry automatic `DramaDemo` rules from `SLUS_202.28` / `sh2pc.exe`:
/// - Stage `2` (`stg_toilet.c` `0x008e7ce8` entry 0: flag `0x0020` = 32, `demo_no = 1` `first_toilet`).
/// - Stage `3` (`stg_ovservation.c` `0x008e7948` entry 0: flag `0x0021` = 33, `demo_no = 2` `tenbou`).
pub const AUTO_STAGE_DEMO_RULES: &[AutoDemoRule] = &[
    AutoDemoRule {
        stage_id: 2,
        completion_flag: 0x0020,
        demo_no: CS_INTRO_BATHROOM_DEMO_NO as u32,
        // Post-mirror spawn in `er01.map` (`ScenarioState::new` / `stg_toilet.c`).
        post_pos: Some([-19_606.6, 0.0, 20_429.55]),
        post_yaw: Some(-std::f32::consts::FRAC_PI_2),
    },
    AutoDemoRule {
        stage_id: 3,
        completion_flag: 0x0021,
        demo_no: CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO as u32,
        post_pos: None,
        post_yaw: None,
    },
];

/// Evaluates whether the current `stage_id` has an unplayed automatic stage-entry `DramaDemo`
/// (`EventMain` `0x001a7aa4` / `Event_List` `Always` entry).
#[must_use]
pub fn evaluate_auto_stage_demo(
    stage_id: u16,
    flags: &EventFlags,
    triggered_demos: &HashSet<u32>,
) -> Option<DramaDemoTrigger> {
    for rule in AUTO_STAGE_DEMO_RULES {
        if rule.stage_id == stage_id
            && !flags.get(rule.completion_flag)
            && !triggered_demos.contains(&rule.demo_no)
        {
            let demo_id = find_drama_demo_by_no(rule.demo_no as u16).map(|d| d.id);
            return Some(DramaDemoTrigger {
                demo_no: rule.demo_no,
                demo_id,
                post_pos: rule.post_pos,
                post_yaw: rule.post_yaw,
                restore_policy: PostSequenceRestorePolicy::ReevaluateAtPlayerPose,
            });
        }
    }
    None
}

/// Applies the post-cutscene `shCharacterSetPosAfterDemo` (`SLUS_202.28` `0x0011f770`) and
/// `VC_RD_WARP_IN_F` / `VC_ALL_WARP_FLAGS` camera rail reset so gameplay resumes with zero
/// sub-frame interpolation smear across the cutscene-to-gameplay cut.
pub fn apply_post_cutscene_handoff(
    sim: &mut SimulationCore,
    event_camera: &mut EventCameraSystem,
    post_pos: Option<[f32; 3]>,
    post_yaw: Option<f32>,
    restore_policy: PostSequenceRestorePolicy,
) {
    let target_pos = post_pos.map_or(sim.scenario.player_position, Vec3::from_array);
    let target_yaw = post_yaw.unwrap_or(sim.scenario.player_yaw);

    // 1. `shCharacterSetPosAfterDemo` (`0x0011f770`): snap player + prev_player + flashlight sweep.
    sim.snap_player_transform(target_pos, target_yaw);

    // 2. Exit cutscene handoff cleanly and assert `VC_ALL_WARP_FLAGS` (`VC_RD_WARP_IN_F` semantics)
    //    so both PS2 `.cam` rail camera and modern OTS/FPS cameras snap with `prev == curr`.
    event_camera.script_camera.vc_flags |= VC_ALL_WARP_FLAGS;
    let _ = restore_policy;
    sim.camera.reset_to_player(target_pos, target_yaw);
    sim.camera.prev_snapshot = sim.camera.curr_snapshot;
    sim.modern_camera.reset_to_player(
        sim.scenario.character,
        target_pos,
        target_yaw,
        &sim.camera.curr_snapshot,
    );
}
