//! Stage 3 (`stg_ovservation.c` / Observation Deck `ob01`) `ev_prog[1..=6]` handlers.
//!
//! Disassembled 1:1 from `sh2pc.exe` `ACT1_OBSERVATION_EV_PROGS` (`0x008e7a18`):
//! - `[1]` `0x005a0860`: `tenbou` intro cutscene (`demo_no = 2`, flag `33`, post-pose `(19350.25, 0.0, -507.74)`,
//!   flags `1504` & `1553`, mid-demo `1400.0` frame BGM switch flag `517`).
//! - `[2]` `0x005a09b0`: Car seat Map of Silent Hill pickup (`out_p_incar` + `out/p_incar_map`,
//!   `SeCall(0x2b17)`, stage message `28` (`0x1c`), `1.0 s` alpha fade, sets flag `24` (`0x18`)).
//! - `[3]` `0x005a0ca0`: Re-examining car seat after map pickup (`out_p_incar` empty + stage message `3`).
//! - `[4]` `0x005a0cb0`: Walking toward forest stairs without map (`!flag(24)`): locks player,
//!   stage message `2`, auto-walks James `-500.0 mm` along Z (`0x0062d230`), unlocks player, sets flag `34` (`0x22`).
//! - `[5]` `0x005a0de0`: Walking toward blocked road/tunnel: locks player, stage message `0`,
//!   auto-walks James `-500.0 mm` along X (`0x0062d230`), unlocks player.
//! - `[6]` `0x005a0f10`: Extra demo `0x5d` (`93`, triggered when `flag(512) && !flag(513)`), sets flag `513`.

use crate::cutaway::CutawayOverlayEngine;

use super::{EventFlags, ProgContext, PROG_KEY_A, PROG_KEY_B};

/// `tenbou` intro cutscene demo number (`sh2pc.exe` `0x005a0860` / `DRAMA_DEMO_CATALOG`).
pub const STG3_TENBOU_DEMO_NO: u32 = 2;
/// Extra Observation Deck demo number (`0x5d` = `93`, `sh2pc.exe` `0x005a0f1c`: `mov [0x1f7a7c4], 0x5d`).
pub const STG3_EXTRA_DEMO_NO: u32 = 0x5d;
/// Sound effect ID played when taking the Map of Silent Hill (`0x005a0ae9`: `push 0x2b17; call 0x515580`).
pub const STG3_SE_MAP_PICKUP: u32 = 0x2b17;

/// Stage 3 message ID `0`: blocked road/tunnel warning (`0x005a0e24`: `push 0x0; call 0x4a22d0`).
pub const STG3_MSG_BLOCKED_ROAD: i32 = 0;
/// Stage 3 message ID `2`: leaving without map warning (`0x005a0cf4`: `push 0x2; call 0x4a22d0`).
pub const STG3_MSG_STAIRS_NO_MAP: i32 = 2;
/// Stage 3 message ID `3`: empty car seat inspection (`0x005a0ca0`: `push 0x3; call 0x4a3600`).
pub const STG3_MSG_CAR_EMPTY: i32 = 3;
/// Stage 3 message ID `28` (`0x1c`): `"I got a map of Silent Hill."` (`0x005a0b47`: `push 0x1c; call 0x4a22d0`).
pub const STG3_MSG_GOT_MAP: i32 = 0x1c;

/// Event flag `24` (`0x18`): Map of Silent Hill taken from James's car (`ACT1_OBSERVATION_EVENT_LIST[1]`,
/// `EvRoomInit` `0x005a1353`: `0x01f7a9e3 & 1`).
pub const STG3_FLAG_HAS_MAP: usize = 24;
/// Event flag `33` (`0x21`): `tenbou` intro cutscene completed (`ACT1_OBSERVATION_EVENT_LIST[0]`).
pub const STG3_FLAG_TENBOU_DONE: usize = 33;
/// Event flag `34` (`0x22`): warned at forest stairs without map (`ACT1_OBSERVATION_EVENT_LIST[6]`).
pub const STG3_FLAG_STAIRS_WARNED: usize = 34;
/// Event flag `512` (`0x200`): prerequisite for extra Observation Deck demo `0x5d` (`ACT1_OBSERVATION_EVENT_LIST[11]`).
pub const STG3_FLAG_EXTRA_DEMO_REQ: usize = 512;
/// Event flag `513` (`0x201`): extra Observation Deck demo `0x5d` completed (`ACT1_OBSERVATION_EVENT_LIST[11]`).
pub const STG3_FLAG_EXTRA_DEMO_DONE: usize = 513;
/// Post-`tenbou` flag `1504` (`0x5e0`, `sh2pc.exe` `0x005a08c8`: `0x01f7aa9c |= 0x1`).
pub const STG3_FLAG_TENBOU_POST_A: usize = 1504;
/// Post-`tenbou` flag `1553` (`0x611`, `sh2pc.exe` `0x005a08c2`: `0x01f7aaa0 |= 0x20000`).
pub const STG3_FLAG_TENBOU_POST_B: usize = 1553;

/// Car map alpha fade duration in seconds (`0x005a0a12`: `0x3f800000` = `1.0f`, `0x005a0b82`: `0x62eddc` = `0.0f`).
pub const STG3_MAP_FADE_DURATION_SECS: f32 = 1.0;
/// Boundary walkback distance in PS2 mm (`sh2pc.exe` `0x0062d230` = `500.0f`, subtracted along Z at
/// `0x005a0d33` and along X at `0x005a0e51`).
pub const STG3_WALKBACK_DISTANCE_MM: f32 = 500.0;

/// Per-frame output returned by [`tick_stage3_prog`].
#[derive(Debug, Default)]
pub(super) struct Stage3TickOut {
    pub finished: bool,
    pub active_message_id: Option<i32>,
    pub sound_effect: Option<u32>,
    pub requested_demo: Option<u32>,
    pub walkback_delta: Option<[f32; 3]>,
}

/// Advances one of `stg_ovservation.c`'s `ev_prog[1..=6]` state machines by a single frame.
pub(super) fn tick_stage3_prog(
    p: &mut ProgContext,
    down: u32,
    dt: f32,
    active_message_id: Option<i32>,
    flags: &mut EventFlags,
    overlay: &mut CutawayOverlayEngine,
) -> Stage3TickOut {
    let mut out = Stage3TickOut {
        active_message_id,
        ..Stage3TickOut::default()
    };

    match p.prog_no {
        // Stage 3 #1 (`0x005a0860`): `tenbou` intro cutscene (`demo_no = 2`, completion flag `33`).
        // Also writes post-demo flags `1504` (`0x01f7aa9c |= 0x1`) & `1553` (`0x01f7aaa0 |= 0x20000`).
        1 => {
            p.locked = true;
            out.requested_demo = Some(STG3_TENBOU_DEMO_NO);
            flags.set(STG3_FLAG_TENBOU_DONE, true);
            flags.set(STG3_FLAG_TENBOU_POST_A, true);
            flags.set(STG3_FLAG_TENBOU_POST_B, true);
            p.locked = false;
            out.finished = true;
        }
        // Stage 3 #2 (`0x005a09b0`): Car Map pickup (`out_p_incar` + `out/p_incar_map`).
        // Retail `0x005a09b0` jump-table flow:
        //   step 0 (`0x5a09c9`): lock player, open `out_p_incar` with `p_incar_map` alpha 1.0, timer = 1.0s
        //   step 1 (`0x5a0a8c`): wait for A/B -> `SeCall(0x2b17)`, open stage message `28` (`0x1c`)
        //   step 2 (`0x5a0b0e`): wait for A/B to dismiss stage message `28` -> begin 1.0s map fade
        //   step 3 (`0x5a0b71` -> `0x5a0c33`): fade `timer` `1.0 -> 0.0`, hide picture, unlock, set flag `24`
        2 => match p.step {
            0 => {
                p.locked = true;
                overlay.set_picture(Some("out_p_incar"), flags);
                overlay.set_incar_alpha(1.0);
                p.timer = STG3_MAP_FADE_DURATION_SECS;
                p.step = 1;
            }
            1 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.sound_effect = Some(STG3_SE_MAP_PICKUP);
                    out.active_message_id = Some(STG3_MSG_GOT_MAP);
                    p.step = 2;
                }
            }
            2 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = None;
                    p.timer = STG3_MAP_FADE_DURATION_SECS;
                    p.step = 3;
                }
            }
            3 => {
                p.timer = (p.timer - dt).max(0.0);
                overlay.set_incar_alpha(p.timer / STG3_MAP_FADE_DURATION_SECS);
                if p.timer <= 0.0 {
                    overlay.set_picture(None, flags);
                    flags.set(STG3_FLAG_HAS_MAP, true);
                    p.locked = false;
                    out.finished = true;
                }
            }
            _ => out.finished = true,
        },
        // Stage 3 #3 (`0x005a0ca0`): Re-examining car after map pickup (`EvSubPicture("out_p_incar", 3)`).
        3 => match p.step {
            0 => {
                p.locked = true;
                p.flag_set = false;
                overlay.set_picture(Some("out_p_incar"), flags);
                overlay.set_incar_alpha(0.0);
                p.step = 1;
            }
            1 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = Some(STG3_MSG_CAR_EMPTY);
                    p.step = 2;
                }
            }
            2 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = None;
                    overlay.set_picture(None, flags);
                    p.locked = false;
                    out.finished = true;
                }
            }
            _ => out.finished = true,
        },
        // Stage 3 #4 (`0x005a0cb0`): Walking toward forest stairs without map (`!flag(24)`).
        // Locks player, shows stage message `2`, auto-walks James `-500.0 mm` along Z (`0x0062d230`),
        // unlocks player, and sets flag `34` (`0x22`).
        4 => match p.step {
            0 => {
                p.locked = true;
                out.active_message_id = Some(STG3_MSG_STAIRS_NO_MAP);
                p.step = 1;
            }
            1 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = None;
                    out.walkback_delta = Some([0.0, 0.0, -STG3_WALKBACK_DISTANCE_MM]);
                    flags.set(STG3_FLAG_STAIRS_WARNED, true);
                    p.locked = false;
                    out.finished = true;
                }
            }
            _ => out.finished = true,
        },
        // Stage 3 #5 (`0x005a0de0`): Walking toward blocked road/tunnel.
        // Locks player, shows stage message `0`, auto-walks James `-500.0 mm` along X (`0x0062d230`),
        // unlocks player (does not set a completion flag).
        5 => match p.step {
            0 => {
                p.locked = true;
                p.flag_set = false;
                out.active_message_id = Some(STG3_MSG_BLOCKED_ROAD);
                p.step = 1;
            }
            1 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = None;
                    out.walkback_delta = Some([-STG3_WALKBACK_DISTANCE_MM, 0.0, 0.0]);
                    p.locked = false;
                    out.finished = true;
                }
            }
            _ => out.finished = true,
        },
        // Stage 3 #6 (`0x005a0f10`): Extra Observation Deck demo `0x5d` (`93`),
        // gated on `flag(512) && !flag(513)` (`ACT1_OBSERVATION_EVENT_LIST[11]`), sets `flag(513)`.
        6 => {
            p.locked = true;
            if flags.get(STG3_FLAG_EXTRA_DEMO_REQ) && !flags.get(STG3_FLAG_EXTRA_DEMO_DONE) {
                out.requested_demo = Some(STG3_EXTRA_DEMO_NO);
                flags.set(STG3_FLAG_EXTRA_DEMO_DONE, true);
            } else {
                p.flag_set = false;
            }
            p.locked = false;
            out.finished = true;
        }
        _ => {
            out.finished = true;
        }
    }

    out
}
