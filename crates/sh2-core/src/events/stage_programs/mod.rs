//! Modular `ev_prog` stage coroutine library covering all 58 retail stages (`Stages 1..=58`).
//!
//! Decomposed into cohesive act/region submodules (`AGENTS.md` §2):
//! - [`act1_intro_town`]: Stages `1, 2, 4, 5, 14` (Stage `3` handled by [`super::runner_stage3`]).
//! - [`act1_apartments`]: Stages `6..=13` (Wood Side & Blue Creek Apartments + clock/pushables).
//! - [`act1_bowling_hospital`]: Stages `15..=27` (Bowling, Heaven's Night, Brookhaven Normal/Otherworld).
//! - [`act2_society_prison`]: Stages `28..=33` (Historical Society, Delusion, Toluca Prison).
//! - [`act2_labyrinth_lake`]: Stages `34..=38` (Labyrinth, Eddie Boss, Toluca Lake).
//! - [`act2_hotel`]: Stages `39..=47` (Lakeview Hotel Normal/Otherworld + `EvProgEndingJudge` `0x00576490`).
//! - [`endings`]: Stages `48..=52` (Leave, Maria, In Water, Rebirth, Dog Ending `0x00574430`).
//! - [`bfw`]: Stages `53..=58` (*Born from a Wish* sub-scenario).

pub mod act1_apartments;
pub mod act1_bowling_hospital;
pub mod act1_intro_town;
pub mod act2_hotel;
pub mod act2_labyrinth_lake;
pub mod act2_society_prison;
pub mod bfw;
pub mod endings;

use crate::config::RiddleDifficulty;
use crate::cutaway::CutawayOverlayEngine;
use crate::endings::{EndingPointTracker, MainEndingKind, PlayingInfo};
use crate::inventory::InventoryManager;
use crate::world::MultiFloorElevatorPanel;

use super::{EventFlags, ProgContext, PuzzleState, PROG_KEY_A, PROG_KEY_B};

/// Binary-verified descriptor for a standard `sh2pc.exe` `ev_prog` coroutine (`event_sub.c`
/// patterns: `EvSubDemo` `0x4a2110`, `EvSubMessage` `0x4a22d0`, `EvSubQuestion` `0x4a2330`,
/// `EvSubPictureMessage` `0x4a3600`, `SeCall` `0x515580`, `ClearDataRecordEnding` `0x48d330`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticProgSpec {
    /// Retail stage number (`1..=58`).
    pub stage: u8,
    /// 1-based `ev_prog` index in `Stage_Data::ev_prog`.
    pub prog_no: u8,
    /// Exact `sh2pc.exe` virtual address of the `ev_prog` function.
    pub va: u32,
    /// Optional `DramaDemo` number (`0x4a2110` / `0x1f7a7c4`) requested by this program.
    pub demo_no: Option<u32>,
    /// Optional stage `.mes` message ID (`0x4a22d0` / `0x4a2330` / `0x4a3600`).
    pub msg_id: Option<i32>,
    /// Sound effect IDs triggered via `SeCall` (`0x515580`) / `shSdCall` (`0x515110`).
    pub se_ids: &'static [u32],
    /// Event flag indices set (`orl ..., 0x1f7a9e0+`) by this program.
    pub flags_set: &'static [u16],
    /// Event flag indices cleared (`andl ..., 0x1f7a9e0+`) by this program.
    pub flags_clr: &'static [u16],
    /// Optional `ClearData` ending bit recorded via `0x48d330`.
    pub ending_bit: Option<u8>,
    /// `true` when the program clears `ev_prog_flag_set` (`0x1f7a954 = 0`) so `completion_flag` is not set.
    pub clears_flag_set: bool,
    /// `true` when the program locks player movement (`JmsEventMode` `0x5389c0`).
    pub locks_player: bool,
}

/// Per-frame output returned by [`dispatch_stage_program`].
#[derive(Debug, Default)]
pub(crate) struct ProgTickOut {
    pub finished: bool,
    pub active_message_id: Option<i32>,
    pub sound_effects: Vec<u32>,
    pub stop_sound_effects: Vec<u32>,
    pub positional_sound_effects: Vec<(u32, [f32; 3])>,
    pub requested_demo: Option<u32>,
    pub open_save_point_id: Option<u16>,
    pub walkback_delta: Option<[f32; 3]>,
    pub triggered_ending: Option<u8>,
    pub evaluated_main_ending: Option<MainEndingKind>,
}

/// Looks up the binary-verified [`StaticProgSpec`] for `(stage, prog_no)` across all 58 stages.
#[must_use]
pub fn find_static_prog_spec(stage: u8, prog_no: u8) -> Option<&'static StaticProgSpec> {
    let slice: &'static [StaticProgSpec] = match stage {
        1 | 2 | 4 | 5 | 14 => act1_intro_town::ACT1_INTRO_TOWN_SPECS,
        6..=13 => act1_apartments::ACT1_APARTMENTS_SPECS,
        15..=27 => act1_bowling_hospital::ACT1_BOWLING_HOSPITAL_SPECS,
        28..=33 => act2_society_prison::ACT2_SOCIETY_PRISON_SPECS,
        34..=38 => act2_labyrinth_lake::ACT2_LABYRINTH_LAKE_SPECS,
        39..=47 => act2_hotel::ACT2_HOTEL_SPECS,
        48..=52 => endings::ENDINGS_SPECS,
        53..=58 => bfw::BFW_SPECS,
        _ => return None,
    };
    slice
        .iter()
        .find(|s| s.stage == stage && s.prog_no == prog_no)
}

/// Executes a [`StaticProgSpec`] coroutine using Team Silent's standard `event_sub.c` step flow.
pub(crate) fn tick_static_prog_spec(
    spec: &StaticProgSpec,
    p: &mut ProgContext,
    down: u32,
    flags: &mut EventFlags,
    out: &mut ProgTickOut,
) {
    match p.step {
        0 => {
            if spec.locks_player || spec.msg_id.is_some() || spec.demo_no.is_some() {
                p.locked = true;
            }
            if spec.clears_flag_set {
                p.flag_set = false;
            }
            for &se in spec.se_ids {
                out.sound_effects.push(se);
            }
            for &f in spec.flags_set {
                flags.set(usize::from(f), true);
            }
            for &f in spec.flags_clr {
                flags.set(usize::from(f), false);
            }
            if let Some(demo_no) = spec.demo_no {
                out.requested_demo = Some(demo_no);
            }
            if let Some(ending_bit) = spec.ending_bit {
                out.triggered_ending = Some(ending_bit);
            }
            if let Some(msg_id) = spec.msg_id {
                out.active_message_id = Some(msg_id);
                p.step = 1;
                out.finished = false;
            } else {
                p.locked = false;
                out.finished = true;
            }
        }
        1 => {
            if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                out.active_message_id = None;
                p.locked = false;
                out.finished = true;
            } else {
                out.finished = false;
            }
        }
        _ => {
            p.locked = false;
            out.finished = true;
        }
    }
}

/// Dispatches a running `(p.stage, p.prog_no)` coroutine for one frame.
///
/// Bespoke interactive puzzles, pushable objects, multi-floor elevators, `EvProgEndingJudge`,
/// and the Dog Ending lever sequence are handled by their respective module's `tick_*` hook
/// before falling back to the 1:1 binary-verified [`StaticProgSpec`] table for that act.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_stage_program(
    p: &mut ProgContext,
    down: u32,
    held: u32,
    dt: f32,
    active_message_id: Option<i32>,
    flags: &mut EventFlags,
    puzzle: &mut PuzzleState,
    inventory: &mut InventoryManager,
    overlay: &mut CutawayOverlayEngine,
    riddle: RiddleDifficulty,
    playing_info: Option<&PlayingInfo>,
    ending_tracker: Option<&EndingPointTracker>,
    active_elevator: Option<&mut Option<MultiFloorElevatorPanel>>,
) -> ProgTickOut {
    let mut out = ProgTickOut {
        active_message_id,
        ..ProgTickOut::default()
    };

    // Stage 3 (`stg_ovservation.c` #1..=#6) uses its dedicated 1:1 runner.
    if p.stage == 3 && (1..=6).contains(&p.prog_no) {
        let s3 =
            super::runner_stage3::tick_stage3_prog(p, down, dt, active_message_id, flags, overlay);
        out.finished = s3.finished;
        out.active_message_id = s3.active_message_id;
        if let Some(se) = s3.sound_effect {
            out.sound_effects.push(se);
        }
        out.requested_demo = s3.requested_demo;
        out.walkback_delta = s3.walkback_delta;
        return out;
    }

    // Stage 4 (`stg_forest.c` #1..=#7) uses its dedicated 1:1 runner (`sh2pc.exe` `0x0059f2b0..0x005a0804`).
    if p.stage == 4 && (1..=7).contains(&p.prog_no) {
        let s4 = super::runner_stage4::tick_stage4_prog(
            p,
            down,
            active_message_id,
            flags,
            inventory,
            overlay,
        );
        out.finished = s4.finished;
        out.active_message_id = s4.active_message_id;
        out.sound_effects = s4.sound_effects;
        out.stop_sound_effects = s4.stop_sound_effects;
        out.positional_sound_effects = s4.positional_sound_effects;
        out.requested_demo = s4.requested_demo;
        out.open_save_point_id = s4.open_save_point_id;
        out.triggered_ending = s4.triggered_ending;
        return out;
    }

    // Check bespoke interactive / puzzle / ending handlers first.
    let handled = match p.stage {
        1 | 2 | 4 | 5 | 14 => act1_intro_town::tick_bespoke(p, down, flags, &mut out),
        6..=13 => act1_apartments::tick_bespoke(
            p, down, held, dt, flags, puzzle, inventory, overlay, &mut out,
        ),
        15..=27 => act1_bowling_hospital::tick_bespoke(
            p,
            down,
            flags,
            inventory,
            overlay,
            active_elevator,
            &mut out,
        ),
        28..=33 => act2_society_prison::tick_bespoke(p, down, flags, puzzle, overlay, &mut out),
        34..=38 => act2_labyrinth_lake::tick_bespoke(p, down, flags, puzzle, overlay, &mut out),
        39..=47 => act2_hotel::tick_bespoke(
            p,
            down,
            flags,
            inventory,
            riddle,
            playing_info,
            ending_tracker,
            active_elevator,
            &mut out,
        ),
        48..=52 => endings::tick_bespoke(p, down, flags, &mut out),
        53..=58 => bfw::tick_bespoke(p, down, flags, inventory, overlay, &mut out),
        _ => false,
    };

    if handled {
        return out;
    }

    if let Some(spec) = find_static_prog_spec(p.stage, p.prog_no) {
        tick_static_prog_spec(spec, p, down, flags, &mut out);
        return out;
    }

    let _ = riddle;
    out.finished = true;
    out
}
