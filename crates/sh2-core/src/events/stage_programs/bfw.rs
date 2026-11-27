//! Born from a Wish Sub-Scenario (`Stages 53..=58`) `ev_prog` table & bespoke coroutine handlers.
//! All `va`, `demo_no`, `msg_id`, `se_ids`, `flags_set`, `flags_clr`, and `ending_bit`
//! fields are extracted 1:1 from retail `sh2pc.exe` `Stage_Data::ev_prog`.

use super::StaticProgSpec;

/// Binary-verified `sh2pc.exe` `ev_prog` descriptors for Born from a Wish Sub-Scenario (`Stages 53..=58`).
pub const BFW_SPECS: &[StaticProgSpec] = &[
    StaticProgSpec {
        stage: 53,
        prog_no: 1,
        va: 0x00573ef0,
        demo_no: Some(99),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 53,
        prog_no: 2,
        va: 0x005743c0,
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
        stage: 54,
        prog_no: 1,
        va: 0x00573890,
        demo_no: Some(105),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: Some(0x40),
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 1,
        va: 0x00571ae0,
        demo_no: None,
        msg_id: Some(72),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 2,
        va: 0x00571af0,
        demo_no: None,
        msg_id: Some(74),
        se_ids: &[],
        flags_set: &[674],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 3,
        va: 0x00571cd0,
        demo_no: None,
        msg_id: Some(75),
        se_ids: &[],
        flags_set: &[675],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 4,
        va: 0x00571d60,
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
        stage: 55,
        prog_no: 5,
        va: 0x00571d70,
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
        stage: 55,
        prog_no: 6,
        va: 0x00572d40,
        demo_no: None,
        msg_id: Some(91),
        se_ids: &[0x4291],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 7,
        va: 0x00572420,
        demo_no: None,
        msg_id: Some(80),
        se_ids: &[0x428f],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 8,
        va: 0x00572630,
        demo_no: Some(102),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 9,
        va: 0x00572750,
        demo_no: Some(103),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 55,
        prog_no: 10,
        va: 0x00572830,
        demo_no: Some(104),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 56,
        prog_no: 1,
        va: 0x00570f50,
        demo_no: Some(100),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 56,
        prog_no: 2,
        va: 0x00571050,
        demo_no: Some(101),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 56,
        prog_no: 3,
        va: 0x00571120,
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
        stage: 56,
        prog_no: 4,
        va: 0x00571130,
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
        stage: 56,
        prog_no: 5,
        va: 0x00571150,
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
        stage: 56,
        prog_no: 6,
        va: 0x00571160,
        demo_no: None,
        msg_id: None,
        se_ids: &[0x4293],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 56,
        prog_no: 7,
        va: 0x005711a0,
        demo_no: None,
        msg_id: Some(36),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 56,
        prog_no: 8,
        va: 0x00571410,
        demo_no: None,
        msg_id: Some(38),
        se_ids: &[0x428f],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 56,
        prog_no: 9,
        va: 0x00571620,
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
        stage: 56,
        prog_no: 10,
        va: 0x00571640,
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
        stage: 56,
        prog_no: 11,
        va: 0x00571690,
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
        stage: 56,
        prog_no: 12,
        va: 0x005716b0,
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
        stage: 56,
        prog_no: 13,
        va: 0x005716d0,
        demo_no: None,
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 57,
        prog_no: 1,
        va: 0x00570da0,
        demo_no: None,
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 58,
        prog_no: 1,
        va: 0x00570c70,
        demo_no: None,
        msg_id: Some(4),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 58,
        prog_no: 2,
        va: 0x00570c80,
        demo_no: None,
        msg_id: Some(3),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 58,
        prog_no: 3,
        va: 0x00570d20,
        demo_no: None,
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
];

use crate::cutaway::CutawayOverlayEngine;
use crate::inventory::InventoryManager;

use super::super::{
    evaluate_baldwin_tombstone_boards, EventFlags, ProgContext, PROG_KEY_A, PROG_KEY_B,
};
use super::ProgTickOut;

/// Bespoke interactive handlers for *Born from a Wish* (`Stages 53..=58`).
pub(super) fn tick_bespoke(
    p: &mut ProgContext,
    down: u32,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    overlay: &mut CutawayOverlayEngine,
    out: &mut ProgTickOut,
) -> bool {
    match (p.stage, p.prog_no) {
        // Stage 53 #1: Born from a Wish start (`s53_wish_start`)
        (53, 1) => {
            inventory.item_get_now(10); // Revolver
            inventory.item_get_now(11); // Revolver bullets x2 boxes
            inventory.item_get_now(11);
            inventory.item_get_now(2); // First-aid kit
            out.finished = true;
            true
        }
        // Stage 53 #2: Born from a Wish Chinese cleaver pickup (`s53_get_cleaver`)
        (53, 2) => {
            inventory.item_get_now(17);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        // Stage 54 #1: Born from a Wish ending (`s54_end_wish`, `0x00573b8b`: `push $0x40; call 0x48d330`)
        (54, 1) => {
            out.triggered_ending = Some(0x40);
            out.finished = true;
            true
        }
        // Stage 55 #1: Baldwin Mansion map pickup (`s55_get_map`)
        (55, 1) => {
            match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("add_mapboldwin"), flags);
                    p.step = 1;
                    out.finished = false;
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        out.sound_effects.push(0x2b17);
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        out.finished = true;
                    } else {
                        out.finished = false;
                    }
                }
                _ => out.finished = true,
            }
            true
        }
        // Stage 55 #4: Baldwin Mansion Black Board pickup (`s55_get_black_board`)
        (55, 4) => {
            inventory.item_get_now(80);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        // Stage 55 #5: Baldwin Mansion Red Board pickup (`s55_get_red_board`)
        (55, 5) => {
            inventory.item_get_now(81);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        // Stage 55 #6: Baldwin Mansion garden tombstone board puzzle (`s55_board_puzzle`)
        (55, 6) => {
            match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("add_p_teien"), flags);
                    p.step = 1;
                    out.finished = false;
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        if !evaluate_baldwin_tombstone_boards(flags, inventory) {
                            p.flag_set = false;
                        }
                        p.locked = false;
                        out.finished = true;
                    } else {
                        out.finished = false;
                    }
                }
                _ => out.finished = true,
            }
            true
        }
        // Stage 55 #8: Give Birthday card to Ernest (`s55_ernest_card`)
        (55, 8) => {
            inventory.item_lose(84);
            out.finished = true;
            true
        }
        // Stage 55 #10: Give White liquid to Ernest (`s55_ernest_liquid`)
        (55, 10) => {
            inventory.item_lose(85);
            out.finished = true;
            true
        }
        // Stage 56 #3: Baldwin Mansion White Board pickup (`s56_get_white_board`)
        (56, 3) => {
            inventory.item_get_now(79);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        // Stage 56 #4: Use Acacia key (`s56_use_acacia_key`)
        (56, 4) => {
            inventory.item_lose(82);
            out.sound_effects.push(11011);
            out.finished = true;
            true
        }
        // Stage 56 #5: Matches pickup (`s56_get_matches`)
        (56, 5) => {
            inventory.item_get_now(83);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        // Stage 56 #6: Light attic candlestick with Matches (`s56_light_candle`)
        (56, 6) => {
            inventory.item_lose(83);
            out.sound_effects.push(21000);
            flags.set(712, true);
            out.finished = true;
            true
        }
        // Stage 56 #7: Birthday card pickup (`s56_birthday_card`)
        (56, 7) => {
            inventory.item_get_now(84);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        // Stage 58 #3: Blue Creek White liquid pickup (`s58_get_white_liquid`)
        (58, 3) => {
            inventory.item_get_now(85);
            out.sound_effects.push(0x2b21);
            out.finished = true;
            true
        }
        _ => false,
    }
}
