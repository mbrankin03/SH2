//! Main Scenario Ending Stages (`Stages 48..=52`) `ev_prog` table & bespoke coroutine handlers.
//! All `va`, `demo_no`, `msg_id`, `se_ids`, `flags_set`, `flags_clr`, and `ending_bit`
//! fields are extracted 1:1 from retail `sh2pc.exe` `Stage_Data::ev_prog`.

use super::StaticProgSpec;

/// Binary-verified `sh2pc.exe` `ev_prog` descriptors for Main Scenario Ending Stages (`Stages 48..=52`).
pub const ENDINGS_SPECS: &[StaticProgSpec] = &[
    StaticProgSpec {
        stage: 48,
        prog_no: 1,
        va: 0x0043fc70,
        demo_no: Some(88),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 48,
        prog_no: 2,
        va: 0x0043ff00,
        demo_no: Some(87),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 48,
        prog_no: 3,
        va: 0x00440050,
        demo_no: None,
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 48,
        prog_no: 4,
        va: 0x00440070,
        demo_no: Some(89),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 49,
        prog_no: 1,
        va: 0x00440ac0,
        demo_no: Some(91),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 49,
        prog_no: 2,
        va: 0x00440d80,
        demo_no: Some(87),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 50,
        prog_no: 1,
        va: 0x0043e900,
        demo_no: Some(94),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 50,
        prog_no: 2,
        va: 0x0043eba0,
        demo_no: Some(87),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 50,
        prog_no: 3,
        va: 0x0043ecf0,
        demo_no: None,
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 50,
        prog_no: 4,
        va: 0x0043ed20,
        demo_no: Some(95),
        msg_id: None,
        se_ids: &[0xeab2],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 51,
        prog_no: 1,
        va: 0x00574690,
        demo_no: Some(96),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 51,
        prog_no: 2,
        va: 0x00574930,
        demo_no: Some(87),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 52,
        prog_no: 1,
        va: 0x00574430,
        demo_no: Some(98),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: Some(0x10),
        clears_flag_set: false,
        locks_player: false,
    },
];

use super::super::{EventFlags, ProgContext};
use super::ProgTickOut;

/// Retail Dog Ending `DramaDemo` number (`sh2pc.exe` `0x00574496`: `push $0x8c5fa4; call 0x4a2110`,
/// `*(u32*)0x008c5fa4 == 0x62` = `98`).
pub const DOG_ENDING_DEMO_NO: u32 = 0x62;

/// Bespoke handlers for Main Scenario Ending Stages (`Stages 48..=52`).
pub(super) fn tick_bespoke(
    p: &mut ProgContext,
    _down: u32,
    _flags: &mut EventFlags,
    out: &mut ProgTickOut,
) -> bool {
    match (p.stage, p.prog_no) {
        // Stage 48 (`stage_end_recovery` / Leave ending, `STAGE_END_RECOVERY_PROGS`)
        (48, 1..=4) => {
            out.triggered_ending = Some(0x01);
            out.finished = true;
            true
        }
        // Stage 49 (`stage_end_maria` / Maria ending, `STAGE_END_MARIA_PROGS`)
        (49, 1..=2) => {
            out.triggered_ending = Some(0x02);
            out.finished = true;
            true
        }
        // Stage 50 (`stage_end_suicide` / In Water ending, `STAGE_END_SUICIDE_PROGS`)
        (50, 1..=4) => {
            out.triggered_ending = Some(0x04);
            out.finished = true;
            true
        }
        // Stage 51 (`stage_end_rebirth` / Rebirth ending, `STAGE_END_REBIRTH_PROGS`)
        (51, 1..=2) => {
            out.triggered_ending = Some(0x08);
            out.finished = true;
            true
        }
        // Stage 52 #1 (`stage_end_dog` / Dog Ending lever coroutine, `sh2pc.exe` `0x00574430..0x005745c9`):
        // Step 0 (`0x57447b`): lock player, request Dog Room cutscene `demo_no = 98` (`0x62`),
        // record `ClearData::ENDING_DOG` (`0x10`, `0x005745ba: push $0x10; call 0x48d330`).
        (52, 1) => {
            p.locked = true;
            out.requested_demo = Some(DOG_ENDING_DEMO_NO);
            out.triggered_ending = Some(0x10);
            p.locked = false;
            out.finished = true;
            true
        }
        _ => false,
    }
}
