//! Core Event Sequence Manager & State Machine (`EventMainStandard`, `EventExec*`, `EventProgressCheck`
//! in `SLUS_202.28` `src/Event/event.c` at `0x001a77b0..0x001ab190`).

pub mod exec_door_move;
pub mod exec_msg_item_save;
pub mod progression;

pub use exec_door_move::{
    event_exec_door, event_exec_flag, event_exec_move, is_connected_stage_pair,
};
pub use exec_msg_item_save::{
    event_exec_common_fail_message, event_exec_item, event_exec_message, event_exec_save,
};
pub use progression::{event_progress_check, light_spot_on_off_check};

use crate::inventory::InventoryManager;
use crate::types::{EventListEntry, EventResultKind, EventTriggerKind, ItemListEntry};

use super::triggers::{
    event_check, event_exec_sub_flag_set, EventEvalInput, EventTriggerState, EVENT_ITEM_INDEX_BASE,
};
use super::EventFlags;

/// Execution mode (`ev_m_step` at `0x011263d0` in `EventMainStandard`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum EventExecMode {
    /// `0`: Idle; runs `EventCheck` every frame.
    #[default]
    Idle = 0,
    /// `1`: `EventExecFlag` (instant set/clear flag).
    Flag = 1,
    /// `2`: `EventExecMove` (door/room/stage transition).
    Move = 2,
    /// `3`: `EventExecMove` phase of a `MovePlusProgram` (`rslt == 6`) event.
    MoveThenProgram = 3,
    /// `4`: `EventExecProgram` (`ev_prog` stage script).
    Program = 4,
    /// `5`: `EventExecMessage` (stage `.mes` dialog).
    Message = 5,
    /// `6`: `EventExecDoor` (locked, unlocked, or broken door feedback).
    Door = 6,
    /// `7`: `EventExecSave` (red square save point flow).
    Save = 7,
    /// `8`: `EventExecItem` (`Item_List` 3D pickup sequence).
    Item = 8,
    /// `9`: `EventExecItemUseFail` (`"I can't use this here."`, common message 1).
    ItemUseFail = 9,
    /// `10`: `EventExecChizuFail` (`"It's too dark to read the map."`, common message 2).
    MapDisabledFail = 10,
    /// `11`: Post-completion cooldown step before returning to `Idle`.
    Complete = 11,
}

/// Core PS2 `event.c` state machine registers (`0x01126340..0x011263d4` and `0x011b6720..0x011b672c`).
#[derive(Debug, Clone)]
pub struct EventStepState {
    /// `ev_s_step` (`0x011263b8`): Fine sub-step inside an operation.
    pub ev_s_step: i32,
    /// `ev_p_step` (`0x011263c0`): Primary step inside a stage `ev_prog` coroutine.
    pub ev_p_step: i32,
    /// `ev_e_step` (`0x011263c8`): Step inside an `EventExec*` handler (`0..=7`).
    pub ev_e_step: i32,
    /// `ev_m_step` (`0x011263d0`): Top-level `EventMainStandard` execution mode.
    pub ev_m_step: EventExecMode,
    /// `ev_timer` (`0x011263d4`): Elapsed timer in seconds for timed sequence steps.
    pub ev_timer: f32,
    /// `ev_active` (`0x011263a0`): `0` = idle, `1` = non-blocking background event, `2` = blocking event.
    pub ev_active: u8,
    /// `ev_cancel` (`0x011263b0`): Set by [`EventStepState::cancel`] (`EventCancel`) to abort/skip active step.
    pub ev_cancel: bool,
    /// `ev_prog_flag_set` (`0x011263a8`): Whether `EventExecProgram` sets `result_flag` upon completion.
    pub ev_prog_flag_set: bool,
    /// `ev_work_no` (`0x01126380`): Active `Event_List` index (`0..0x400`) or `Item_List` index (`>= 0x400`).
    pub ev_work_no: Option<usize>,
    /// `ev_close_se` (`0x01126340`): Door-close sound effect ID played after a `Move` fade-in.
    pub ev_close_se: i16,
    /// `se_pos` (`0x01126350`): 3D world position for door open/close sound effects.
    pub se_pos: [f32; 3],
    /// Destination position computed by `EventResultMovePosition` (`0x011b6720..0x011b6728`).
    pub result_move_pos: [f32; 3],
    /// Destination yaw angle (`rot_y` in radians) computed by `EventResultMovePosition` (`0x011b672c`).
    pub result_move_rot_y: f32,
}

impl Default for EventStepState {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStepState {
    pub const fn new() -> Self {
        Self {
            ev_s_step: 0,
            ev_p_step: 0,
            ev_e_step: 0,
            ev_m_step: EventExecMode::Idle,
            ev_timer: 0.0,
            ev_active: 0,
            ev_cancel: false,
            ev_prog_flag_set: true,
            ev_work_no: None,
            ev_close_se: 0,
            se_pos: [0.0, 0.0, 0.0],
            result_move_pos: [0.0, 0.0, 0.0],
            result_move_rot_y: 0.0,
        }
    }

    /// Resets the step state (`EventInit`).
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Signals cancellation of the currently active message/item/save step (`EventCancel` at `0x001a9a70`).
    #[inline]
    pub fn cancel(&mut self) {
        self.ev_cancel = true;
    }

    #[inline]
    pub(crate) fn set_e_step(&mut self, step: i32) {
        self.ev_e_step = step;
        self.ev_p_step = 0;
        self.ev_s_step = 0;
    }
}

/// 3D positional sound effect emitted by an `EventExec*` handler (`SeCallPos`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventPositionalSound {
    pub se_id: i16,
    pub volume: f32,
    pub pos: [f32; 3],
    pub channel: u8,
}

/// Stage/room transition parameters produced by `EventExecMove` (`0x001aa670`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageTransitionCommand {
    /// Destination stage ID (`0` if moving between rooms within the current stage).
    pub dest_stage: u8,
    /// Stage transition / fade mode (`(rslt0 >> 5) & 0x7f`).
    pub stage_change_mode: u8,
    /// `true` if `(current_stage, dest_stage)` is in [`crate::types::CONNECT_STAGE_PAIRS`] (`bit 0x08` at `0x010f08a4`).
    pub is_connected_stage: bool,
    /// Observation mode flag (`(rslt0 >> 4) & 1`, `bit 0x04` at `0x010f08a4`).
    pub observation_mode: bool,
    /// Player destination position `[x, y, z]`.
    pub dest_pos: [f32; 3],
    /// Player destination yaw angle in radians.
    pub dest_rot_y: f32,
}

/// Side effects emitted during a single frame of `EventMainStandard` / `EventExec*`.
#[derive(Debug, Clone, Default)]
pub struct EventEngineEffects {
    /// `Some(true)` when `SCNowPlayableEventSwitch(jms, 1)` locks player control;
    /// `Some(false)` when `SCNowPlayableEventSwitch(jms, 0)` unlocks player control.
    pub player_lock_change: Option<bool>,
    /// Common `.mes` message ID displayed via `fontMessageNum(common_msg, id)`.
    pub common_message: Option<u16>,
    /// Stage `.mes` message ID displayed via `fontMessageNum(stage_msg, id)`.
    pub stage_message: Option<u16>,
    /// `true` if `fontClear()` was called this frame.
    pub clear_message: bool,
    /// Non-positional 2D sound effects triggered via `SeCall`.
    pub sound_effects: Vec<u32>,
    /// 3D positional sound effects triggered via `SeCallPos`.
    pub positional_sounds: Vec<EventPositionalSound>,
    /// Screen fade command (`ScreenEffectFadeStart(kind, 0.0)`), if any.
    pub fade_command: Option<u8>,
    /// Stage/room transition command issued by `EventExecMove`, if any.
    pub stage_transition: Option<StageTransitionCommand>,
    /// Save point location name ID set by `SetSavePointName`, if any.
    pub save_point_name_id: Option<u8>,
    /// Player pickup animation requested by `PlayerEventAnimeSet` (`0x4e21` crouch or `0x65` stand).
    pub player_pickup_anim: Option<u16>,
    /// 3D item character model deleted on pickup `(chr_id, base_flag_id)`.
    pub deleted_item_model: Option<(i32, u16)>,
    /// Stage `ev_prog` index (`1..`) invoked when `ev_m_step == Program`.
    pub active_prog_index: Option<u8>,
}

/// High-level `EventMainStandard` (`SLUS_202.28` `0x001a77b0`) coordinator combining
/// [`EventStepState`] and [`EventTriggerState`].
#[derive(Debug, Clone, Default)]
pub struct EventEngine {
    pub step_state: EventStepState,
    pub trigger_state: EventTriggerState,
}

impl EventEngine {
    pub const fn new() -> Self {
        Self {
            step_state: EventStepState::new(),
            trigger_state: EventTriggerState::new(),
        }
    }

    pub fn reset(&mut self) {
        self.step_state.reset();
        self.trigger_state.reset();
    }

    /// Executes one frame of `EventMainStandard` (`SLUS_202.28` `0x001a77b0..0x001a7db0`).
    #[allow(clippy::too_many_arguments)]
    pub fn tick_standard(
        &mut self,
        ev_list: &[EventListEntry],
        item_list: &[ItemListEntry],
        ev_pos: &[u8],
        current_stage: u8,
        eval_input: &EventEvalInput,
        message_dismissed: bool,
        anim_success_frame: bool,
        anim_ended: bool,
        prog_finished: bool,
        flags: &mut EventFlags,
        inventory: &mut InventoryManager,
    ) -> EventEngineEffects {
        let mut effects = EventEngineEffects::default();

        if self.step_state.ev_m_step == EventExecMode::Idle {
            let mut check_input = *eval_input;
            check_input.ev_active = self.step_state.ev_active;
            if let Some(work_no) = event_check(
                ev_list,
                item_list,
                ev_pos,
                flags,
                &check_input,
                &mut self.trigger_state,
            ) {
                self.step_state.ev_work_no = Some(work_no);
                self.step_state.set_e_step(0);
                self.step_state.ev_cancel = false;

                if work_no < EVENT_ITEM_INDEX_BASE {
                    if let Some(entry) = ev_list.get(work_no) {
                        self.step_state.ev_active = match entry.trigger_kind() {
                            Some(
                                EventTriggerKind::TouchContinuous
                                | EventTriggerKind::TouchEdge
                                | EventTriggerKind::RoomInit
                                | EventTriggerKind::StageInit,
                            ) => 1,
                            _ => 2,
                        };

                        if matches!(
                            entry.trigger_kind(),
                            Some(EventTriggerKind::UseItemLook | EventTriggerKind::UseItemTouch)
                        ) && entry.consume_item()
                        {
                            inventory.item_lose(entry.required_item_id());
                        }

                        self.step_state.ev_m_step = match entry.result_kind() {
                            Some(EventResultKind::FlagSet | EventResultKind::FlagClear) => {
                                EventExecMode::Flag
                            }
                            Some(EventResultKind::Move) => EventExecMode::Move,
                            Some(EventResultKind::MovePlusProgram) => {
                                EventExecMode::MoveThenProgram
                            }
                            Some(EventResultKind::Program) => EventExecMode::Program,
                            Some(EventResultKind::Message) => EventExecMode::Message,
                            Some(
                                EventResultKind::DoorLocked
                                | EventResultKind::DoorBroken
                                | EventResultKind::DoorUnlocked,
                            ) => EventExecMode::Door,
                            Some(EventResultKind::SavePoint) => EventExecMode::Save,
                            None => EventExecMode::Idle,
                        };
                    }
                } else {
                    self.step_state.ev_active = 2;
                    self.step_state.ev_m_step = EventExecMode::Item;
                }
            } else if eval_input.used_item_id.is_some() {
                self.step_state.ev_active = 2;
                self.step_state.set_e_step(0);
                self.step_state.ev_m_step = EventExecMode::ItemUseFail;
            }
        }

        let completed = match self.step_state.ev_m_step {
            EventExecMode::Idle => false,
            EventExecMode::Flag => {
                if let Some(entry) = self.step_state.ev_work_no.and_then(|i| ev_list.get(i)) {
                    event_exec_flag(entry, flags)
                } else {
                    true
                }
            }
            EventExecMode::Move | EventExecMode::MoveThenProgram => {
                if let Some(entry) = self.step_state.ev_work_no.and_then(|i| ev_list.get(i)) {
                    event_exec_move(
                        entry,
                        ev_pos,
                        current_stage,
                        &mut self.step_state,
                        flags,
                        &mut effects,
                    )
                } else {
                    true
                }
            }
            EventExecMode::Program => {
                if let Some(entry) = self.step_state.ev_work_no.and_then(|i| ev_list.get(i)) {
                    if self.step_state.ev_e_step == 0 {
                        event_exec_sub_flag_set(entry, flags);
                        self.step_state.ev_prog_flag_set = true;
                        self.step_state.set_e_step(2);
                    }
                    effects.active_prog_index = Some(entry.prog_index());
                    if prog_finished {
                        let result_flag = entry.result_flag() as usize;
                        if self.step_state.ev_prog_flag_set && result_flag != 0 {
                            flags.set(result_flag, true);
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            EventExecMode::Message => {
                if let Some(entry) = self.step_state.ev_work_no.and_then(|i| ev_list.get(i)) {
                    event_exec_message(
                        entry,
                        &mut self.step_state,
                        flags,
                        message_dismissed,
                        &mut effects,
                    )
                } else {
                    true
                }
            }
            EventExecMode::Door => {
                if let Some(entry) = self.step_state.ev_work_no.and_then(|i| ev_list.get(i)) {
                    event_exec_door(
                        entry,
                        ev_pos,
                        &mut self.step_state,
                        flags,
                        message_dismissed,
                        &mut effects,
                    )
                } else {
                    true
                }
            }
            EventExecMode::Save => {
                if let Some(entry) = self.step_state.ev_work_no.and_then(|i| ev_list.get(i)) {
                    event_exec_save(
                        entry,
                        &mut self.step_state,
                        flags,
                        message_dismissed,
                        &mut effects,
                    )
                } else {
                    true
                }
            }
            EventExecMode::Item => {
                if let Some(item) = self
                    .step_state
                    .ev_work_no
                    .and_then(|i| i.checked_sub(EVENT_ITEM_INDEX_BASE))
                    .and_then(|idx| item_list.get(idx))
                {
                    event_exec_item(
                        item,
                        eval_input.player_pos[1],
                        anim_success_frame,
                        anim_ended,
                        message_dismissed,
                        &mut self.step_state,
                        flags,
                        inventory,
                        &mut effects,
                    )
                } else {
                    true
                }
            }
            EventExecMode::ItemUseFail => event_exec_common_fail_message(
                1,
                &mut self.step_state,
                message_dismissed,
                &mut effects,
            ),
            EventExecMode::MapDisabledFail => event_exec_common_fail_message(
                2,
                &mut self.step_state,
                message_dismissed,
                &mut effects,
            ),
            EventExecMode::Complete => true,
        };

        if completed {
            self.step_state.ev_m_step = EventExecMode::Idle;
            self.step_state.set_e_step(0);
            self.step_state.ev_active = 0;
            self.step_state.ev_cancel = false;
            self.step_state.ev_work_no = None;
        }

        effects
    }
}
