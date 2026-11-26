//! Step-based Stage Program Runner (`ev_prog` / `EventExecProgram`).

use crate::config::RiddleDifficulty;
use crate::cutaway::CutawayOverlayEngine;
use crate::inventory::InventoryManager;

use super::{
    evaluate_baldwin_tombstone_boards, is_clock_solved, riddle_level_index, step_clock_needles,
    EventFlags, PuzzleState, PROG_KEY_A, PROG_KEY_B,
};

/// Execution context for a running stage program (`Prog` in `prog_int.h`).
#[derive(Debug, Clone)]
pub struct ProgContext {
    /// Stage number (`2..=58`).
    pub stage: u8,
    /// Program index in the stage's `ev_prog` table (`1..`).
    pub prog_no: u8,
    /// Completion flag (`rslt1 & 0x3fff`) set when the program finishes with `flag_set == true`.
    pub completion_flag: u16,
    /// `ev_prog_flag_set` (`0x1f7d554`): cleared by inspection programs that should not set `completion_flag`.
    pub flag_set: bool,
    /// `JmsEventMode(jms, 1)`: whether player movement/control is locked by the running program.
    pub locked: bool,
    /// Primary coroutine step (`ev_p_step` at `0x1f7d548`).
    pub step: i32,
    /// Sub-step within the current operation (`ev_s_step` at `0x1f7d544`).
    pub sub: i32,
    /// Elapsed timer in seconds (`0x1f7d564`).
    pub timer: f32,
    /// Last question/choice answer (`0` = Yes / first choice).
    pub answer: i32,
    /// Per-program scratch registers (`work[4]`).
    pub work: [i32; 4],
}

impl ProgContext {
    pub fn new(stage: u8, prog_no: u8, completion_flag: u16) -> Self {
        Self {
            stage,
            prog_no,
            completion_flag,
            flag_set: true,
            locked: false,
            step: 0,
            sub: 0,
            timer: 0.0,
            answer: 0,
            work: [0; 4],
        }
    }
}

/// Step-based stage program runner (`EventExecProgram`) coordinating `ProgContext`, `EventFlags`,
/// `PuzzleState`, `InventoryManager`, and `CutawayOverlayEngine`.
#[derive(Debug, Clone, Default)]
pub struct StageProgramRunner {
    /// Currently running stage program context, if any.
    active: Option<ProgContext>,
    /// Currently displayed stage message ID (`-1` if none).
    active_message_id: Option<i32>,
    /// Sound effects queued during the current frame (`p_se`).
    sound_effects: Vec<u32>,
    /// Cutscene demo ID requested by `p_demo`, if any.
    requested_demo: Option<u32>,
    /// Sub-scenario / main game ending bit triggered by `p_game_end`, if any.
    triggered_ending: Option<u8>,
    /// Auto-walk displacement delta `[dx, dy, dz]` in PS2 mm requested by boundary warning
    /// programs (`0x005a0cb0` `-500.0 mm` Z, `0x005a0de0` `-500.0 mm` X via `0x0053b3c0`).
    pending_walkback_delta: Option<[f32; 3]>,
}

impl StageProgramRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts stage program `(stage, prog_no)` with `completion_flag` (`rslt1 & 0x3fff`).
    pub fn start(&mut self, stage: u8, prog_no: u8, completion_flag: u16) {
        self.active = Some(ProgContext::new(stage, prog_no, completion_flag));
        self.active_message_id = None;
        self.sound_effects.clear();
        self.requested_demo = None;
        self.pending_walkback_delta = None;
    }

    /// Aborts any running stage program and hides any active cutaway still.
    pub fn abort(&mut self, overlay: &mut CutawayOverlayEngine, flags: &EventFlags) {
        self.active = None;
        self.active_message_id = None;
        self.pending_walkback_delta = None;
        overlay.set_picture(None, flags);
    }

    /// Returns `true` if a stage program is currently executing.
    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    /// Returns `true` if the player is locked (`JmsEventMode(jms, 1)`) by the active program.
    pub fn is_player_locked(&self) -> bool {
        self.active.as_ref().is_some_and(|p| p.locked)
    }

    /// Returns a reference to the active `ProgContext`, if any.
    pub fn context(&self) -> Option<&ProgContext> {
        self.active.as_ref()
    }

    /// Returns the currently open stage message ID, if any.
    pub fn active_message_id(&self) -> Option<i32> {
        self.active_message_id
    }

    /// Returns the sound effects triggered during the last `tick`.
    pub fn sound_effects(&self) -> &[u32] {
        &self.sound_effects
    }

    /// Returns any ending triggered by `p_game_end`.
    pub fn triggered_ending(&self) -> Option<u8> {
        self.triggered_ending
    }

    /// Consumes and returns any cutscene demo ID requested by `p_demo`.
    pub fn take_requested_demo(&mut self) -> Option<u32> {
        self.requested_demo.take()
    }

    /// Consumes and returns any ending bit triggered by `p_game_end`.
    pub fn take_triggered_ending(&mut self) -> Option<u8> {
        self.triggered_ending.take()
    }

    /// Consumes and returns any auto-walk displacement delta `[dx, dy, dz]` in PS2 mm emitted by
    /// boundary warning programs (`stg_ovservation.c` `0x005a0cb0` / `0x005a0de0`).
    pub fn take_walkback_delta(&mut self) -> Option<[f32; 3]> {
        self.pending_walkback_delta.take()
    }

    /// Resolves the retail `sh2pc.exe` `ev_prog` function VA for `(stage, prog_no)` from
    /// `stages_act1::act1_stage_descriptor` (`ev_prog_addrs`) or `stages_act2::stage_table_act2`
    /// (`ev_prog_vas`). Returns `None` if `prog_no == 0` or the entry is null/out-of-bounds.
    #[must_use]
    pub fn resolve_ev_prog_va(stage: u8, prog_no: u8) -> Option<u32> {
        let idx = usize::from(prog_no);
        if idx == 0 {
            return None;
        }
        let va = if stage <= 27 {
            super::stages_act1::act1_stage_descriptor(stage)?
                .ev_prog_addrs
                .get(idx)
                .copied()?
        } else {
            super::stages_act2::stage_table_act2(u16::from(stage))?
                .ev_prog_vas
                .get(idx)
                .copied()?
        };
        (va != 0).then_some(va)
    }

    /// Advances the active stage program by one frame (`dt` seconds, `down` newly pressed keys,
    /// `held` currently held keys). Returns `true` while the program continues running, or `false`
    /// when the program finishes (setting `completion_flag` if `flag_set` is still `true`).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        down: u32,
        held: u32,
        dt: f32,
        flags: &mut EventFlags,
        puzzle: &mut PuzzleState,
        inventory: &mut InventoryManager,
        overlay: &mut CutawayOverlayEngine,
        riddle: RiddleDifficulty,
    ) -> bool {
        self.sound_effects.clear();
        let Some(mut p) = self.active.take() else {
            return false;
        };

        let finished = match (p.stage, p.prog_no) {
            // Stage 2 #1: Opening observation deck restroom start (s02_game_start, `0x005a14a0`)
            (2, 1) => {
                p.locked = true;
                self.requested_demo = Some(201);
                flags.set(18, true);
                p.locked = false;
                true
            }
            // Stage 2 #3: Restroom stall / door prompt (`0x005a1710`: `EvSubMessage(5)` / `EvSubQuestion(1)`)
            (2, 3) => match p.step {
                0 => {
                    p.locked = true;
                    p.flag_set = false;
                    self.active_message_id = Some(5);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = None;
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 2 #4: Restroom mirror inspection (`0x005a1830`: `EvSubMessage(6)`)
            (2, 4) => match p.step {
                0 => {
                    p.locked = true;
                    p.flag_set = false;
                    self.active_message_id = Some(6);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = None;
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 3 #1..#6 (`stg_ovservation.c`, `ACT1_OBSERVATION_EV_PROGS` `0x008e7a18`)
            (3, 1..=6) => {
                let out = super::runner_stage3::tick_stage3_prog(
                    &mut p,
                    down,
                    dt,
                    self.active_message_id,
                    flags,
                    overlay,
                );
                self.active_message_id = out.active_message_id;
                if let Some(se) = out.sound_effect {
                    self.sound_effects.push(se);
                }
                if let Some(demo_no) = out.requested_demo {
                    self.requested_demo = Some(demo_no);
                }
                if let Some(delta) = out.walkback_delta {
                    self.pending_walkback_delta = Some(delta);
                }
                out.finished
            }
            // Stage 7 #1: Wood Side E 2F scream jumpscare (EvProgAnyoneCry, 0x59b1f0)
            // Locks player, starts stage anim 0x4e22, plays SE 0x9c49 at (-99400, -500, 26400),
            // waits 2.0s, shows stage message 0 ("What was that!?"), and unlocks on dismissal.
            (7, 1) => match p.sub {
                0 => {
                    p.locked = true;
                    p.timer = 0.0;
                    self.sound_effects.push(0x9c49);
                    p.sub = 0x1b;
                    false
                }
                0x1b => {
                    p.timer += dt;
                    if p.timer >= 2.0 {
                        self.active_message_id = Some(0);
                        if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                            self.active_message_id = None;
                            p.sub = 0x0d;
                        }
                    }
                    false
                }
                0x0d => {
                    p.locked = false;
                    true
                }
                _ => true,
            },
            // Stage 7 #4: Room 208 closed clock inspection (s07_clock_time)
            (7, 4) => match p.step {
                0 => {
                    p.locked = true;
                    p.flag_set = false;
                    overlay.set_picture(Some("apt_clock_close"), flags);
                    self.active_message_id = Some(58);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.active_message_id = None;
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 7 #5: Use Clock Key on Room 208 clock (s07_use_clock_key)
            (7, 5) => {
                p.locked = true;
                inventory.item_lose(28); // Clock key
                self.sound_effects.push(11033);
                flags.set(71, true);
                p.locked = false;
                true
            }
            // Stage 7 #6: Room 208 open clock hands puzzle (s07_try_move_clock, 0x59a690)
            (7, 6) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("apt_clock_open"), flags);
                    p.timer = 0.0;
                    p.step = 1;
                    false
                }
                1 => {
                    if let Some(se) = step_clock_needles(puzzle, held, dt, &mut p.timer) {
                        self.sound_effects.push(se);
                    }
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        if is_clock_solved(puzzle) {
                            self.sound_effects.push(0x2b1b);
                            // `FLAG_APART_CLOCK_SOLVED` (`81`, `0x51`): aligned with
                            // `world::pushable_objects::FLAG_APART_CLOCK_SOLVED` and
                            // `sh2pc.exe` `0x0059c32a` (`test $0x20000, 0x1f7a9e8`).
                            flags.set(crate::world::FLAG_APART_CLOCK_SOLVED, true);
                        } else {
                            p.flag_set = false;
                        }
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 7 #17: Wood Side E 2F Room 206 thud (EvProgMonkeyKick206, 0x59b980)
            (7, 17) => {
                if !flags.get(69) {
                    self.sound_effects.push(0x9c47);
                }
                true
            }
            // Stage 19 #4: Hospital 3F carbon paper memo (s19_carbon_number)
            (19, 4) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("hsp_p_carbon"), flags);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 20 #5: Hospital 3F wall blood symbols (s20_guruguru_number)
            (20, 5) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("hsp_p_boxnumber"), flags);
                    flags.set(203, true);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 20 #10: Shower drain key fishing with combined Bent needle + Piece of hair (87)
            (20, 10) => {
                p.locked = true;
                inventory.item_lose(87);
                inventory.item_get_now(37); // Elevator key
                flags.set(210, true);
                self.sound_effects.push(0x2b21);
                p.locked = false;
                true
            }
            // Stage 53 #1: Born from a Wish start (s53_wish_start)
            (53, 1) => {
                inventory.item_get_now(10); // Revolver
                inventory.item_get_now(11); // Revolver bullets x2 boxes
                inventory.item_get_now(11);
                inventory.item_get_now(2); // First-aid kit
                true
            }
            // Stage 53 #2: Born from a Wish Chinese cleaver pickup (s53_get_cleaver)
            (53, 2) => {
                inventory.item_get_now(17);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 54 #1: Born from a Wish ending (s54_end_wish)
            (54, 1) => {
                self.triggered_ending = Some(0x40);
                true
            }
            // Stage 55 #1: Baldwin Mansion map pickup (s55_get_map)
            (55, 1) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("add_mapboldwin"), flags);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        self.sound_effects.push(0x2b17);
                        overlay.set_picture(None, flags);
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 55 #4: Baldwin Mansion Black Board pickup (s55_get_black_board)
            (55, 4) => {
                inventory.item_get_now(80);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 55 #5: Baldwin Mansion Red Board pickup (s55_get_red_board)
            (55, 5) => {
                inventory.item_get_now(81);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 55 #6: Baldwin Mansion garden tombstone board puzzle (s55_board_puzzle)
            (55, 6) => match p.step {
                0 => {
                    p.locked = true;
                    overlay.set_picture(Some("add_p_teien"), flags);
                    p.step = 1;
                    false
                }
                1 => {
                    if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                        overlay.set_picture(None, flags);
                        if !evaluate_baldwin_tombstone_boards(flags, inventory) {
                            p.flag_set = false;
                        }
                        p.locked = false;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            },
            // Stage 55 #8: Give Birthday card to Ernest (s55_ernest_card)
            (55, 8) => {
                inventory.item_lose(84);
                true
            }
            // Stage 55 #10: Give White liquid to Ernest (s55_ernest_liquid)
            (55, 10) => {
                inventory.item_lose(85);
                true
            }
            // Stage 56 #3: Baldwin Mansion White Board pickup (s56_get_white_board)
            (56, 3) => {
                inventory.item_get_now(79);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 56 #4: Use Acacia key (s56_use_acacia_key)
            (56, 4) => {
                inventory.item_lose(82);
                self.sound_effects.push(11011);
                true
            }
            // Stage 56 #5: Matches pickup (s56_get_matches)
            (56, 5) => {
                inventory.item_get_now(83);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 56 #6: Light attic candlestick with Matches (s56_light_candle)
            (56, 6) => {
                inventory.item_lose(83);
                self.sound_effects.push(21000);
                flags.set(712, true);
                true
            }
            // Stage 56 #7: Birthday card pickup (s56_birthday_card)
            (56, 7) => {
                inventory.item_get_now(84);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 58 #3: Blue Creek White liquid pickup (s58_get_white_liquid)
            (58, 3) => {
                inventory.item_get_now(85);
                self.sound_effects.push(0x2b21);
                true
            }
            // Stage 48 (`stage_end_recovery` / Leave ending, `STAGE_END_RECOVERY_PROGS`)
            (48, 1..=4) => {
                self.triggered_ending = Some(0x01);
                true
            }
            // Stage 49 (`stage_end_maria` / Maria ending, `STAGE_END_MARIA_PROGS`)
            (49, 1..=2) => {
                self.triggered_ending = Some(0x02);
                true
            }
            // Stage 50 (`stage_end_suicide` / In Water ending, `STAGE_END_SUICIDE_PROGS`)
            (50, 1..=4) => {
                self.triggered_ending = Some(0x04);
                true
            }
            // Stage 51 (`stage_end_rebirth` / Rebirth ending, `STAGE_END_REBIRTH_PROGS`)
            (51, 1..=2) => {
                self.triggered_ending = Some(0x08);
                true
            }
            // Stage 52 (`stage_end_dog` / Dog ending, `0x00574430`)
            (52, 1) => {
                self.triggered_ending = Some(0x10);
                true
            }
            _ => {
                let _va = Self::resolve_ev_prog_va(p.stage, p.prog_no);
                let _ = riddle;
                true
            }
        };

        if finished {
            if p.flag_set && p.completion_flag != 0 {
                flags.set(p.completion_flag as usize, true);
            }
            false
        } else {
            overlay.evaluate_frame(flags, puzzle, riddle_level_index(riddle), 0);
            self.active = Some(p);
            true
        }
    }
}
