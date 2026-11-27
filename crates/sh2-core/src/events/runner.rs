//! Step-based Stage Program Runner (`ev_prog` / `EventExecProgram`).

use crate::config::RiddleDifficulty;
use crate::cutaway::CutawayOverlayEngine;
use crate::endings::{EndingPointTracker, MainEndingKind, PlayingInfo};
use crate::inventory::InventoryManager;
use crate::world::MultiFloorElevatorPanel;

use super::{riddle_level_index, EventFlags, PuzzleState};

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
    /// Sound effects requested to stop during the current frame (`SeStop`, e.g. `0x4268`).
    stop_sound_effects: Vec<u32>,
    /// 3D positional sound effects queued during the current frame (`SeCallPos`, e.g. `0x426b`).
    positional_sound_effects: Vec<(u32, [f32; 3])>,
    /// Cutscene demo ID requested by `p_demo`, if any.
    requested_demo: Option<u32>,
    /// Save point ID requested to open the Save Menu (`SavePointSet` + `mode = 9`, e.g. Stage 4 Well `2`).
    open_save_point_id: Option<u16>,
    /// Sub-scenario / main game ending bit triggered by `p_game_end`, if any.
    triggered_ending: Option<u8>,
    /// Main ending selected by `EvProgEndingJudge` (`Stage 44 #6`, `0x00576490`), if evaluated.
    evaluated_main_ending: Option<MainEndingKind>,
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
        self.stop_sound_effects.clear();
        self.positional_sound_effects.clear();
        self.requested_demo = None;
        self.open_save_point_id = None;
        self.pending_walkback_delta = None;
    }

    /// Aborts any running stage program and hides any active cutaway still.
    pub fn abort(&mut self, overlay: &mut CutawayOverlayEngine, flags: &EventFlags) {
        self.active = None;
        self.active_message_id = None;
        self.stop_sound_effects.clear();
        self.positional_sound_effects.clear();
        self.open_save_point_id = None;
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

    /// Returns sound effect IDs requested to stop (`SeStop`) during the last `tick`.
    pub fn stop_sound_effects(&self) -> &[u32] {
        &self.stop_sound_effects
    }

    /// Returns 3D positional sound effects `(se_id, pos_mm)` triggered during the last `tick`.
    pub fn positional_sound_effects(&self) -> &[(u32, [f32; 3])] {
        &self.positional_sound_effects
    }

    /// Consumes and returns any save point ID requesting the Save Menu (`SavePointSet` + `mode = 9`).
    pub fn take_open_save_point_id(&mut self) -> Option<u16> {
        self.open_save_point_id.take()
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

    /// Returns the main ending evaluated by `EvProgEndingJudge` (`Stage 44 #6`, `0x00576490`).
    pub fn evaluated_main_ending(&self) -> Option<MainEndingKind> {
        self.evaluated_main_ending
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
        self.tick_with_campaign(
            down, held, dt, flags, puzzle, inventory, overlay, riddle, None, None, None,
        )
    }

    /// Advances the active stage program with full campaign context (`PlayingInfo`,
    /// `EndingPointTracker`, and `MultiFloorElevatorPanel` slot) for `EvProgEndingJudge`
    /// (`Stage 44 #6`, `0x00576490`) and multi-floor elevator panels.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_with_campaign(
        &mut self,
        down: u32,
        held: u32,
        dt: f32,
        flags: &mut EventFlags,
        puzzle: &mut PuzzleState,
        inventory: &mut InventoryManager,
        overlay: &mut CutawayOverlayEngine,
        riddle: RiddleDifficulty,
        playing_info: Option<&PlayingInfo>,
        ending_tracker: Option<&EndingPointTracker>,
        active_elevator: Option<&mut Option<MultiFloorElevatorPanel>>,
    ) -> bool {
        self.sound_effects.clear();
        self.stop_sound_effects.clear();
        self.positional_sound_effects.clear();
        let Some(mut p) = self.active.take() else {
            return false;
        };

        let out = super::stage_programs::dispatch_stage_program(
            &mut p,
            down,
            held,
            dt,
            self.active_message_id,
            flags,
            puzzle,
            inventory,
            overlay,
            riddle,
            playing_info,
            ending_tracker,
            active_elevator,
        );
        self.active_message_id = out.active_message_id;
        self.sound_effects.extend(out.sound_effects);
        self.stop_sound_effects.extend(out.stop_sound_effects);
        self.positional_sound_effects
            .extend(out.positional_sound_effects);
        if let Some(demo_no) = out.requested_demo {
            self.requested_demo = Some(demo_no);
        }
        if let Some(save_id) = out.open_save_point_id {
            self.open_save_point_id = Some(save_id);
        }
        if let Some(delta) = out.walkback_delta {
            self.pending_walkback_delta = Some(delta);
        }
        if let Some(ending_bit) = out.triggered_ending {
            self.triggered_ending = Some(ending_bit);
        }
        if let Some(main_ending) = out.evaluated_main_ending {
            self.evaluated_main_ending = Some(main_ending);
        }

        if out.finished {
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
