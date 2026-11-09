//! Core Event Sequence Manager & State Machine (`EventMainStandard`, `EventExec*`, `EventProgressCheck`
//! in `SLUS_202.28` `src/Event/event.c` at `0x001a77b0..0x001ab190`).

use crate::inventory::InventoryManager;
use crate::types::{
    EventListEntry, EventResultKind, EventTriggerKind, ItemListEntry, CONNECT_STAGE_PAIRS,
    DOOR_SE_TABLE, ITEM_KIND_TABLE,
};

use super::triggers::{
    event_check, event_exec_sub_flag_set, event_position_set, event_result_move_position,
    EventEvalInput, EventTriggerState, EVENT_ITEM_INDEX_BASE,
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
    fn set_e_step(&mut self, step: i32) {
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
    /// `true` if `(current_stage, dest_stage)` is in [`CONNECT_STAGE_PAIRS`] (`bit 0x08` at `0x010f08a4`).
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

/// Executes `EventExecFlag` (`SLUS_202.28` `0x001a9bf0`): sets (`rslt == 1`) or clears (`rslt == 2`)
/// `entry.result_flag()` and applies `EventExecSubFlagSet`.
pub fn event_exec_flag(entry: &EventListEntry, flags: &mut EventFlags) -> bool {
    let set_value = entry.result_kind() == Some(EventResultKind::FlagSet);
    let target_flag = entry.result_flag() as usize;
    flags.set(target_flag, set_value);
    event_exec_sub_flag_set(entry, flags);
    true
}

/// Executes one frame of `EventExecMessage` (`SLUS_202.28` `0x001a9cc0`).
///
/// Returns `true` when the message has been dismissed (`message_dismissed` or `state.ev_cancel`).
pub fn event_exec_message(
    entry: &EventListEntry,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_e_step == 0 {
        effects.stage_message = Some(entry.message_or_save_id() as u16);
        let result_flag = entry.result_flag() as usize;
        if result_flag != 0 {
            flags.set(result_flag, true);
        }
        event_exec_sub_flag_set(entry, flags);
        effects.player_lock_change = Some(true);
        state.set_e_step(2);
    }

    if message_dismissed || state.ev_cancel {
        effects.clear_message = true;
        effects.player_lock_change = Some(false);
        true
    } else {
        false
    }
}

/// Executes one frame of `EventExecDoor` (`SLUS_202.28` `0x001a9f20`).
///
/// Handles locked (`rslt == 7`), broken (`rslt == 8`), and unlocked (`rslt == 9`) doors,
/// playing the 3D positional door sound at `ev_pos` (`pos.y -= 500.0`) and showing the
/// appropriate common or stage message.
pub fn event_exec_door(
    entry: &EventListEntry,
    ev_pos: &[u8],
    state: &mut EventStepState,
    flags: &mut EventFlags,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_e_step == 0 {
        let door_idx = (entry.dest_stage_or_door_se() as usize).min(DOOR_SE_TABLE.len() - 1);
        let door_se = DOOR_SE_TABLE[door_idx];
        let mut pos = entry
            .shape_kind()
            .and_then(|s| event_position_set(ev_pos, entry.ev_pos_offset(), s))
            .unwrap_or([0.0, 0.0, 0.0]);
        pos[1] -= 500.0;

        let msg_id = entry.message_or_save_id();
        let se_id = match entry.result_kind() {
            Some(EventResultKind::DoorLocked) => {
                if msg_id == 0xff {
                    effects.common_message = Some(5); // "It's locked."
                } else {
                    effects.stage_message = Some(msg_id as u16);
                }
                door_se.lock
            }
            Some(EventResultKind::DoorUnlocked) => {
                if msg_id == 0xff {
                    effects.common_message = Some(6); // "The lock is unlocked."
                } else if msg_id == 0xfe {
                    effects.common_message = Some(7); // "The door is unlocked."
                } else {
                    effects.stage_message = Some(msg_id as u16);
                }
                door_se.unlock
            }
            _ => {
                effects.common_message = Some(4); // "The lock is broken. I can't open this door."
                door_se.broken
            }
        };

        if se_id != 0 {
            effects.positional_sounds.push(EventPositionalSound {
                se_id,
                volume: 1.0,
                pos,
                channel: 0,
            });
        }

        let result_flag = entry.result_flag() as usize;
        if result_flag != 0 {
            flags.set(result_flag, true);
        }
        event_exec_sub_flag_set(entry, flags);
        effects.player_lock_change = Some(true);
        state.set_e_step(2);
    }

    if message_dismissed || state.ev_cancel {
        effects.clear_message = true;
        effects.player_lock_change = Some(false);
        true
    } else {
        false
    }
}

/// Executes one frame of `EventExecItem` (`SLUS_202.28` `0x001aa260`) for a stage `Item_List` pickup.
#[allow(clippy::too_many_arguments)]
pub fn event_exec_item(
    item: &ItemListEntry,
    player_y: f32,
    anim_success_frame: bool,
    anim_ended: bool,
    message_dismissed: bool,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    effects: &mut EventEngineEffects,
) -> bool {
    let item_y = item.pos_y as f32;
    let is_crouch_pickup = (player_y - item_y) < 100.0;

    match state.ev_e_step {
        0 => {
            effects.player_lock_change = Some(true);
            if is_crouch_pickup {
                effects.player_pickup_anim = Some(0x4e21);
                state.set_e_step(4);
            } else {
                effects.player_pickup_anim = Some(0x0065);
                state.set_e_step(1);
            }
            false
        }
        4 => {
            if state.ev_cancel {
                state.set_e_step(6);
            } else if anim_success_frame {
                state.set_e_step(1);
            }
            false
        }
        1 => {
            let kind_idx = (item.item_kind_index() as usize).min(ITEM_KIND_TABLE.len() - 1);
            let kind = ITEM_KIND_TABLE[kind_idx];
            let base_flag = item.flag_id() as usize;
            flags.set(base_flag + 1, true);
            if kind.item_id > 0 {
                inventory.item_get_now(kind.item_id as u8);
            }
            effects.common_message = Some(kind.msg_id);
            state.set_e_step(2);
            false
        }
        2 => {
            if message_dismissed || state.ev_cancel {
                effects.clear_message = true;
                effects.sound_effects.push(0x2b21);
                let kind_idx = (item.item_kind_index() as usize).min(ITEM_KIND_TABLE.len() - 1);
                let kind = ITEM_KIND_TABLE[kind_idx];
                effects.deleted_item_model = Some((kind.chr_id, item.flag_id()));
                if is_crouch_pickup {
                    state.set_e_step(5);
                } else {
                    state.set_e_step(6);
                }
            }
            false
        }
        5 => {
            if anim_ended || state.ev_cancel {
                state.set_e_step(6);
            }
            false
        }
        6 => {
            effects.player_lock_change = Some(false);
            true
        }
        _ => true,
    }
}

/// Checks if `(stage_a, stage_b)` is a connected sub-stage pair in [`CONNECT_STAGE_PAIRS`]
/// (`EventExecMove` at `0x001aa8c8..0x001aa944`).
pub fn is_connected_stage_pair(stage_a: u8, stage_b: u8) -> bool {
    let a = stage_a as i16;
    let b = stage_b as i16;
    CONNECT_STAGE_PAIRS
        .iter()
        .take_while(|&&(x, _)| x != 0)
        .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
}

/// Executes one frame of `EventExecMove` (`SLUS_202.28` `0x001aa670`).
pub fn event_exec_move(
    entry: &EventListEntry,
    ev_pos: &[u8],
    current_stage: u8,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_e_step {
        0 => {
            event_exec_sub_flag_set(entry, flags);
            let result_flag = entry.result_flag() as usize;
            if result_flag != 0 && state.ev_m_step != EventExecMode::MoveThenProgram {
                flags.set(result_flag, true);
            }

            if let Some(dest_shape) = entry.dest_shape_kind() {
                if let Some((pos, rot_y)) =
                    event_result_move_position(ev_pos, entry.dest_ev_pos_offset(), dest_shape)
                {
                    state.result_move_pos = pos;
                    state.result_move_rot_y = rot_y;
                }
            }

            // Trigger-side door open sound
            let mut open_pos = entry
                .shape_kind()
                .and_then(|s| event_position_set(ev_pos, entry.ev_pos_offset(), s))
                .unwrap_or([0.0, 0.0, 0.0]);
            open_pos[1] -= 500.0;

            let door_idx = (entry.dest_stage_or_door_se() as usize).min(DOOR_SE_TABLE.len() - 1);
            let door_se = DOOR_SE_TABLE[door_idx];
            if door_se.open != 0 {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id: door_se.open,
                    volume: 1.0,
                    pos: open_pos,
                    channel: 0,
                });
            }
            state.ev_close_se = door_se.close;

            // Destination-side door close sound position
            let dest_shape_as_ev = entry
                .dest_shape_kind()
                .and_then(|ds| crate::types::EventShapeKind::from_raw(ds as u8));
            let mut close_pos = dest_shape_as_ev
                .and_then(|s| event_position_set(ev_pos, entry.dest_ev_pos_offset(), s))
                .unwrap_or(state.result_move_pos);
            close_pos[1] -= 500.0;
            state.se_pos = close_pos;

            let stage_change_mode = entry.stage_change_mode();
            let dest_stage = if stage_change_mode != 0 {
                stage_change_mode
            } else {
                current_stage
            };
            let is_connected =
                stage_change_mode != 0 && is_connected_stage_pair(current_stage, dest_stage);

            effects.stage_transition = Some(StageTransitionCommand {
                dest_stage: if stage_change_mode != 0 {
                    dest_stage
                } else {
                    0
                },
                stage_change_mode,
                is_connected_stage: is_connected,
                observation_mode: entry.observation_mode(),
                dest_pos: state.result_move_pos,
                dest_rot_y: state.result_move_rot_y,
            });

            effects.player_lock_change = Some(true);
            state.set_e_step(3);
            effects.fade_command = Some(1);
            false
        }
        3 => {
            if state.ev_close_se != 0 {
                effects.positional_sounds.push(EventPositionalSound {
                    se_id: state.ev_close_se,
                    volume: 1.0,
                    pos: state.se_pos,
                    channel: 0,
                });
            }
            effects.player_lock_change = Some(false);
            effects.fade_command = Some(4);

            if state.ev_m_step == EventExecMode::MoveThenProgram {
                state.ev_m_step = EventExecMode::Program;
                state.set_e_step(0);
                effects.active_prog_index = Some(entry.prog_index());
                false
            } else {
                true
            }
        }
        _ => false,
    }
}

/// Executes one frame of `EventExecSave` (`SLUS_202.28` `0x001aab10`).
///
/// Checks `flag(5)` (`0x01126430` bit 5): on the player's first save interaction, displays
/// common message `8` before setting `flag(5)` and transitioning to the save screen (`step 7`).
pub fn event_exec_save(
    entry: &EventListEntry,
    state: &mut EventStepState,
    flags: &mut EventFlags,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_e_step {
        0 => {
            effects.player_lock_change = Some(true);
            if flags.get(5) {
                state.set_e_step(7);
            } else {
                effects.common_message = Some(8);
                state.set_e_step(1);
            }
            false
        }
        1 => {
            if message_dismissed || state.ev_cancel {
                effects.clear_message = true;
                if state.ev_cancel {
                    state.set_e_step(6);
                } else {
                    flags.set(5, true);
                    state.set_e_step(7);
                }
            }
            false
        }
        7 => {
            effects.fade_command = Some(11);
            effects.save_point_name_id = Some(entry.message_or_save_id());
            effects.sound_effects.push(0x2743);
            state.set_e_step(6);
            false
        }
        6 => {
            effects.fade_command = Some(4);
            effects.player_lock_change = Some(false);
            true
        }
        _ => false,
    }
}

/// Executes one frame of `EventExecChizuFail` (`SLUS_202.28` `0x001aae40`) or `EventExecItemUseFail`.
pub fn event_exec_common_fail_message(
    common_msg_id: u16,
    state: &mut EventStepState,
    message_dismissed: bool,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_e_step == 0 {
        effects.common_message = Some(common_msg_id);
        effects.player_lock_change = Some(true);
        state.set_e_step(2);
    }

    if message_dismissed || state.ev_cancel {
        effects.clear_message = true;
        effects.player_lock_change = Some(false);
        true
    } else {
        false
    }
}

/// Evaluates `LightSpotOnOffCheck` (`SLUS_202.28` `0x001aad70`):
/// returns `true` when the player possesses the Flashlight (`has_flashlight`), the flashlight
/// toggle is switched on (`flashlight_switched_on`), and global flag `272` (`0x01126450` bit 16,
/// flashlight disabled/dead battery) is NOT set.
#[inline]
pub fn light_spot_on_off_check(
    has_flashlight: bool,
    flashlight_switched_on: bool,
    flags: &EventFlags,
) -> bool {
    has_flashlight && flashlight_switched_on && !flags.get(272)
}

/// Evaluates `EventProgressCheck` (`SLUS_202.28` `0x001aaf00`), returning the early/mid-game
/// story progression milestone (`0..=10`) based on `stage_glb_crd`, `room_id`, `player_pos`,
/// and `EventFlags`.
pub fn event_progress_check(
    stage_glb_crd: i32,
    room_id: u8,
    player_pos: [f32; 3],
    flags: &EventFlags,
) -> i32 {
    // 1. Forest trail / graveyard (`glb_crd == 6` and `!flag(32)`)
    if stage_glb_crd == 6 && !flags.get(32) {
        return 1;
    }
    // 2. East South Vale streets (`glb_crd == 1` and `!flag(36)` and `px > -20000` and `pz > -20000`)
    if stage_glb_crd == 1
        && !flags.get(36)
        && player_pos[0] > -20_000.0
        && player_pos[2] > -20_000.0
    {
        return 2;
    }
    // 3. Martin Street / apartment gate (`glb_crd == 2` and `!flag(43)`)
    if stage_glb_crd == 2 && !flags.get(43) {
        return 3;
    }
    // 4. Wood Side Apartments flashlight / corpse (`flag(69) && !flag(70)`)
    if flags.get(69) && !flags.get(70) {
        return 4;
    }
    // 5. Wood Side Apartments Room 208 (`flag(69) && room_id == 0x18`)
    if flags.get(69) && room_id == 0x18 {
        return 5;
    }
    // 6. West South Vale / Rosewater Park (`glb_crd == 3` and `!flag(150)`)
    if stage_glb_crd == 3 && !flags.get(150) {
        return 6;
    }
    // 7. After meeting Maria at Rosewater Park (`flag(150) && !flag(152)`)
    if flags.get(150) && !flags.get(152) {
        return 7;
    }
    // 8. Bowling alley / Heaven's Night (`glb_crd == 3 && flag(152) && !flag(157)`)
    if stage_glb_crd == 3 && flags.get(152) && !flags.get(157) {
        return 8;
    }
    // 9. Pursuing Laura toward Brookhaven (`glb_crd == 3 && flag(162) && !flag(163)`)
    if stage_glb_crd == 3 && flags.get(162) && !flags.get(163) {
        return 9;
    }
    // 10. Lakeview Hotel (`flag(379) && !flag(380)`)
    if flags.get(379) && !flags.get(380) {
        return 10;
    }
    0
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
