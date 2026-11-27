//! Act 1 Wood Side & Blue Creek Apartments (`Stages 6..=13`) `ev_prog` table & bespoke coroutine handlers.
//! All `va`, `demo_no`, `msg_id`, `se_ids`, `flags_set`, `flags_clr`, and `ending_bit`
//! fields are extracted 1:1 from retail `sh2pc.exe` `Stage_Data::ev_prog`.

use super::StaticProgSpec;

/// Binary-verified `sh2pc.exe` `ev_prog` descriptors for Act 1 Wood Side & Blue Creek Apartments (`Stages 6..=13`).
pub const ACT1_APARTMENTS_SPECS: &[StaticProgSpec] = &[
    StaticProgSpec {
        stage: 6,
        prog_no: 1,
        va: 0x0059ca20,
        demo_no: Some(13),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 6,
        prog_no: 2,
        va: 0x0059cc00,
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
        stage: 6,
        prog_no: 3,
        va: 0x0059cd30,
        demo_no: None,
        msg_id: Some(0),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 1,
        va: 0x00599d90,
        demo_no: None,
        msg_id: Some(7),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 2,
        va: 0x00599f90,
        demo_no: None,
        msg_id: Some(4),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 3,
        va: 0x0059bff0,
        demo_no: None,
        msg_id: Some(11),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 4,
        va: 0x0059a4d0,
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
        stage: 7,
        prog_no: 5,
        va: 0x0059c2e0,
        demo_no: Some(12),
        msg_id: None,
        se_ids: &[0x3e83],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 6,
        va: 0x0059a4e0,
        demo_no: Some(12),
        msg_id: Some(6),
        se_ids: &[],
        flags_set: &[77, 82],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 7,
        va: 0x0059a840,
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
        stage: 7,
        prog_no: 8,
        va: 0x0059a860,
        demo_no: Some(11),
        msg_id: Some(2),
        se_ids: &[],
        flags_set: &[540],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 9,
        va: 0x0059abb0,
        demo_no: Some(11),
        msg_id: Some(3),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 10,
        va: 0x0059ade0,
        demo_no: None,
        msg_id: Some(15),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 11,
        va: 0x0059b010,
        demo_no: Some(15),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 12,
        va: 0x0059b130,
        demo_no: Some(10),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 13,
        va: 0x0059b1f0,
        demo_no: None,
        msg_id: Some(0),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 14,
        va: 0x0059b2f0,
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
        stage: 7,
        prog_no: 15,
        va: 0x0059b350,
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
        stage: 7,
        prog_no: 16,
        va: 0x0059b360,
        demo_no: None,
        msg_id: Some(26),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 17,
        va: 0x0059b560,
        demo_no: None,
        msg_id: Some(26),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 18,
        va: 0x0059b760,
        demo_no: None,
        msg_id: Some(26),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 7,
        prog_no: 19,
        va: 0x0059b960,
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
        stage: 7,
        prog_no: 20,
        va: 0x0059b980,
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
        stage: 8,
        prog_no: 1,
        va: 0x00599b10,
        demo_no: Some(9),
        msg_id: Some(3),
        se_ids: &[],
        flags_set: &[1655],
        flags_clr: &[540],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 8,
        prog_no: 2,
        va: 0x00599d60,
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
        stage: 9,
        prog_no: 1,
        va: 0x00599020,
        demo_no: Some(14),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 9,
        prog_no: 2,
        va: 0x005992c0,
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
        stage: 9,
        prog_no: 3,
        va: 0x005992d0,
        demo_no: None,
        msg_id: None,
        se_ids: &[],
        flags_set: &[1656],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 10,
        prog_no: 1,
        va: 0x005986a0,
        demo_no: None,
        msg_id: Some(3),
        se_ids: &[],
        flags_set: &[609],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 10,
        prog_no: 2,
        va: 0x00598a60,
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
        stage: 10,
        prog_no: 3,
        va: 0x00597fb0,
        demo_no: Some(18),
        msg_id: None,
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 10,
        prog_no: 4,
        va: 0x005982d0,
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
        stage: 10,
        prog_no: 5,
        va: 0x005982e0,
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
        stage: 10,
        prog_no: 6,
        va: 0x005982f0,
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
        stage: 10,
        prog_no: 7,
        va: 0x00598300,
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
        stage: 11,
        prog_no: 1,
        va: 0x00596a40,
        demo_no: None,
        msg_id: Some(0),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: true,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 11,
        prog_no: 2,
        va: 0x00596bb0,
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
        stage: 11,
        prog_no: 3,
        va: 0x00596bd0,
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
        stage: 11,
        prog_no: 4,
        va: 0x00596be0,
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
        stage: 11,
        prog_no: 5,
        va: 0x00597540,
        demo_no: Some(17),
        msg_id: Some(2),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 11,
        prog_no: 6,
        va: 0x005977d0,
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
        stage: 11,
        prog_no: 7,
        va: 0x00597930,
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
        stage: 12,
        prog_no: 1,
        va: 0x00596000,
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
        stage: 12,
        prog_no: 2,
        va: 0x00596020,
        demo_no: None,
        msg_id: Some(2),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 12,
        prog_no: 3,
        va: 0x00596030,
        demo_no: None,
        msg_id: Some(3),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 12,
        prog_no: 4,
        va: 0x00596040,
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
        stage: 12,
        prog_no: 5,
        va: 0x00596050,
        demo_no: Some(19),
        msg_id: None,
        se_ids: &[0xeae3],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 12,
        prog_no: 6,
        va: 0x005962f0,
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
        stage: 12,
        prog_no: 7,
        va: 0x00596310,
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
        stage: 13,
        prog_no: 1,
        va: 0x00595800,
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
        stage: 13,
        prog_no: 2,
        va: 0x00595810,
        demo_no: None,
        msg_id: Some(0),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: true,
    },
    StaticProgSpec {
        stage: 13,
        prog_no: 3,
        va: 0x00595bb0,
        demo_no: None,
        msg_id: Some(1),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 13,
        prog_no: 4,
        va: 0x00595bc0,
        demo_no: Some(16),
        msg_id: Some(4),
        se_ids: &[],
        flags_set: &[],
        flags_clr: &[],
        ending_bit: None,
        clears_flag_set: false,
        locks_player: false,
    },
    StaticProgSpec {
        stage: 13,
        prog_no: 5,
        va: 0x00595d80,
        demo_no: Some(16),
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
use crate::world::{PushableObjectKind, FLAG_APART_CLOCK_SOLVED};

use super::super::{
    is_clock_solved, step_clock_needles, EventFlags, ProgContext, PuzzleState, PROG_KEY_A,
    PROG_KEY_B,
};
use super::ProgTickOut;

/// Bespoke interactive handlers for Act 1 Apartments (`Stages 6..=13`).
#[allow(clippy::too_many_arguments)]
pub(super) fn tick_bespoke(
    p: &mut ProgContext,
    down: u32,
    held: u32,
    dt: f32,
    flags: &mut EventFlags,
    puzzle: &mut PuzzleState,
    inventory: &mut InventoryManager,
    overlay: &mut CutawayOverlayEngine,
    out: &mut ProgTickOut,
) -> bool {
    match (p.stage, p.prog_no) {
        // Stage 7 #1: Wood Side E 2F scream jumpscare (`EvProgAnyoneCry`, `0x0059b1f0`)
        (7, 1) => {
            match p.sub {
                0 => {
                    p.locked = true;
                    p.timer = 0.0;
                    out.sound_effects.push(0x9c49);
                    p.sub = 0x1b;
                    out.finished = false;
                }
                0x1b => {
                    p.timer += dt;
                    if p.timer >= 2.0 {
                        out.active_message_id = Some(0);
                        if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                            out.active_message_id = None;
                            p.sub = 0x0d;
                        }
                    }
                    out.finished = false;
                }
                0x0d => {
                    p.locked = false;
                    out.finished = true;
                }
                _ => out.finished = true,
            }
            true
        }
        // Stage 7 #4: Room 208 closed clock inspection (`s07_clock_time`)
        (7, 4) => {
            match p.step {
                0 => {
                    p.locked = true;
                    p.flag_set = false;
                    overlay.set_picture(Some("apt_clock_close"), flags);
                    out.active_message_id = Some(58);
                    p.step = 1;
                    out.finished = false;
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        out.active_message_id = None;
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
        // Stage 7 #5: Use Clock Key on Room 208 clock (`s07_use_clock_key`)
        (7, 5) => {
            p.locked = true;
            inventory.item_lose(28);
            out.sound_effects.push(11033);
            flags.set(71, true);
            p.locked = false;
            out.finished = true;
            true
        }
        // Stage 7 #6: Room 208 open clock hands puzzle (`s07_try_move_clock`, `0x0059a690`)
        (7, 6) => {
            match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("apt_clock_open"), flags);
                    p.timer = 0.0;
                    p.step = 1;
                    out.finished = false;
                }
                1 => {
                    if let Some(se) = step_clock_needles(puzzle, held, dt, &mut p.timer) {
                        out.sound_effects.push(se);
                    }
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        if is_clock_solved(puzzle) {
                            out.sound_effects.push(0x2b1b);
                            flags.set(FLAG_APART_CLOCK_SOLVED, true);
                        } else {
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
        // Stage 7 #7: Push Room 208 Grandfather Clock (`PushableObjectKind::Apart208GrandfatherClock`)
        (7, 7) => {
            p.locked = true;
            if PushableObjectKind::Apart208GrandfatherClock.trigger_push(flags) {
                out.sound_effects.push(0x2b1b);
            } else {
                p.flag_set = false;
            }
            p.locked = false;
            out.finished = true;
            true
        }
        // Stage 7 #17: Wood Side E 2F Room 206 thud (`EvProgMonkeyKick206`, `0x0059b980`)
        (7, 17) => {
            if !flags.get(69) {
                out.sound_effects.push(0x9c47);
            }
            out.finished = true;
            true
        }
        _ => false,
    }
}
