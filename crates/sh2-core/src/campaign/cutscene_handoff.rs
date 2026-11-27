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
    /// Canonical retail movie identifier (e.g. `"toilet"`, `"open"`, `"deai"`, `"murder"`, `"end"`).
    pub movie_id: &'static str,
    /// Optional destination stage to transition into once the FMV finishes.
    pub post_stage: Option<u16>,
    /// Optional destination player position after the FMV finishes.
    pub post_pos: Option<[f32; 3]>,
    /// Optional destination player yaw after the FMV finishes.
    pub post_yaw: Option<f32>,
    /// Optional real-time `.dds` [`DramaDemoTrigger`] to run after natural FMV completion
    /// (e.g. `toilet.bik` -> `first_toilet` per `sh2pc.exe` `0x005a1594`, `deai.bik` -> `mar_deai_e`).
    pub post_demo: Option<Box<DramaDemoTrigger>>,
    /// Optional event flag to set when the FMV completes or is skipped (`0x0020` for `stg_toilet`).
    pub completion_flag: Option<usize>,
    /// Next step cursor inside `DramaDemoPlayInfo::sequence_chain` if chained from a `DramaDemo`.
    pub chain_cursor: usize,
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

/// Post-`tenbou` spawn position `[x, y, z]` in PS2 mm (`sh2pc.exe` `0x008e7b38`, passed to
/// `CharaSetPos` `0x005396c0` at `0x005a08b2`).
pub const TENBOU_POST_POS: [f32; 3] = [19_350.25, 0.0, -507.74];
/// Post-`tenbou` player facing yaw in radians (`sh2pc.exe` `0x005a08aa`: `push 0x0` -> `0.0f`
/// written to `chara + 0x30` & `chara + 0xe0` in `FUN_005396c0`).
pub const TENBOU_POST_YAW: f32 = 0.0;
/// `tenbou` mid-cutscene frame threshold (`1400.0`, `sh2pc.exe` `0x006b61d8`) compared against
/// `drama_demo_frame` (`0x01f7a7b8`) at `0x005a0936..0x005a0955` to switch Observation Deck BGM.
pub const TENBOU_BGM_SWITCH_FRAME: f32 = 1400.0;
/// Event flag `517` (`0x01f7aa20 |= 0x20` at `sh2pc.exe` `0x005a094e`) set once `tenbou` passes
/// frame `1400.0`, switching `EvBgmControl` (`0x005a1390`) from `0x04` to `0x0e`.
pub const TENBOU_BGM_SWITCH_FLAG: usize = 517;
/// Post-`tenbou` flag `1504` (`0x5e0`, `sh2pc.exe` `0x005a08c8`: `0x01f7aa9c |= 0x1`).
pub const TENBOU_POST_FLAG_A: usize = 1504;
/// Post-`tenbou` flag `1553` (`0x611`, `sh2pc.exe` `0x005a08c2`: `0x01f7aaa0 |= 0x20000`).
pub const TENBOU_POST_FLAG_B: usize = 1553;

/// Verified stage-entry automatic `DramaDemo` rules from `SLUS_202.28` / `sh2pc.exe`:
/// - Stage `2` (`stg_toilet.c` `0x008e7ce8` entry 0: flag `0x0020` = 32, `demo_no = 1` `first_toilet`,
///   preceded by `toilet.bik` via [`evaluate_auto_stage_fmv`]).
/// - Stage `3` (`stg_ovservation.c` `0x008e7948` entry 0: flag `0x0021` = 33, `demo_no = 2` `tenbou`,
///   post-demo `CharaSetPos` `0x005a08b2` -> `0x008e7b38` `(19350.25, 0.0, -507.74)`, yaw `0.0`).
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
        post_pos: Some(TENBOU_POST_POS),
        post_yaw: Some(TENBOU_POST_YAW),
    },
];

/// Evaluates per-frame mid-cutscene stage hooks during an active `DramaDemo`.
///
/// For `tenbou` (`demo_no == 2`, `stg_ovservation.c` `0x005a0936..0x005a0955`), sets
/// `flag(517)` (`0x01f7aa20 |= 0x20`) as soon as `demo_frame > 1400.0` (`0x006b61d8`),
/// causing `EvBgmControl` (`0x005a1390`) to switch from `0x04` to `0x0e`.
pub fn tick_demo_stage_hooks(demo_no: u32, demo_frame: Option<f32>, flags: &mut EventFlags) {
    if demo_no == CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO as u32
        && demo_frame.is_some_and(|frame| frame > TENBOU_BGM_SWITCH_FRAME)
    {
        flags.set(TENBOU_BGM_SWITCH_FLAG, true);
    }
}

/// Applies stage-specific post-cutscene flag writes when a `DramaDemo` finishes or is skipped.
///
/// - `tenbou` (`demo_no == 2`, `stg_ovservation.c` `0x005a08b7..0x005a08d8`): sets
///   `flag(1504)` (`0x01f7aa9c |= 0x1`) and `flag(1553)` (`0x01f7aaa0 |= 0x20000`).
/// - `haka_agl` (`demo_no == 3`, `stg_forest.c` `0x005a07a0..0x005a07b0`): sets `flag(36)`
///   (`ACT1_FOREST_EVENT_LIST[12..=17]` completion flag).
pub fn apply_post_demo_stage_flags(demo_no: u32, flags: &mut EventFlags) {
    if demo_no == CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO as u32 {
        flags.set(TENBOU_POST_FLAG_A, true);
        flags.set(TENBOU_POST_FLAG_B, true);
    } else if demo_no == crate::events::STG4_ANGELA_GRAVE_DEMO_NO {
        flags.set(crate::events::STG4_FLAG_ANGELA_MET, true);
    }
}

/// Applies stage-specific post-cutscene flag writes including RNG-driven flags:
/// - All deterministic writes in [`apply_post_demo_stage_flags`], plus
/// - `haka_agl` (`demo_no == 3`, `stg_forest.c` `0x005a07a0..0x005a07a9`): 50% RNG coin flip
///   (`(rand() & 1) != 0`) setting `flag(37)` (`0x01f7a9e4 |= 0x20`) to pick Angela's next
///   post-cutscene graveyard conversation branch (`ev_prog[3]` vs `ev_prog[4]`).
/// - `haka_agl_h` / `haka_agl_i` (`demo_no == 4`, `stg_forest.c` `0x0059f52e..0x0059f553` &
///   `0x0059f64e..0x0059f673`): re-rolls `flag(37)` (`(rand() & 1) != 0`, setting or clearing bit 5).
pub fn apply_post_demo_stage_flags_with_rng(
    demo_no: u32,
    flags: &mut EventFlags,
    rng: &mut crate::events::Sh2Rng,
) {
    apply_post_demo_stage_flags(demo_no, flags);
    if demo_no == crate::events::STG4_ANGELA_GRAVE_DEMO_NO {
        if (rng.next_i32() & 1) != 0 {
            flags.set(crate::events::STG4_FLAG_ANGELA_TALK_VARIANT, true);
        }
    } else if demo_no == crate::events::STG4_ANGELA_TALK_DEMO_NO {
        flags.set(
            crate::events::STG4_FLAG_ANGELA_TALK_VARIANT,
            (rng.next_i32() & 1) != 0,
        );
    }
}

/// Evaluates whether the current `stage_id` has an unplayed automatic stage-entry FMV
/// (`sh2pc.exe` `0x005a14a0` `stg_toilet` `ev_prog[1]`: plays `toilet.bik` before `first_toilet`).
#[must_use]
pub fn evaluate_auto_stage_fmv(
    stage_id: u16,
    flags: &EventFlags,
    triggered_demos: &HashSet<u32>,
) -> Option<FmvTrigger> {
    if stage_id == 2
        && !flags.get(0x0020)
        && !triggered_demos.contains(&(CS_INTRO_BATHROOM_DEMO_NO as u32))
    {
        let post_pos = Some([-19_606.6, 0.0, 20_429.55]);
        let post_yaw = Some(-std::f32::consts::FRAC_PI_2);
        return Some(FmvTrigger {
            movie_id: "toilet",
            post_stage: None,
            post_pos,
            post_yaw,
            post_demo: Some(Box::new(DramaDemoTrigger {
                demo_no: CS_INTRO_BATHROOM_DEMO_NO as u32,
                demo_id: find_drama_demo_by_no(CS_INTRO_BATHROOM_DEMO_NO).map(|d| d.id),
                post_pos,
                post_yaw,
                restore_policy: PostSequenceRestorePolicy::ReevaluateAtPlayerPose,
            })),
            completion_flag: Some(0x0020),
            chain_cursor: 0,
        });
    }
    None
}

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
