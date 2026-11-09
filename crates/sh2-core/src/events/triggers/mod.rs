//! Spatial Event Trigger Volumes, Look/Touch Geometry Evaluators, and `EventCheck` Selection
//! (`SLUS_202.28` `src/Event/event.c` at `0x001a7dc0..0x001a9be0`).

pub mod look_raycast;
pub mod shapes;

pub use look_raycast::{
    event_check_look, event_check_look_line, event_check_look_point, item_check_look_point,
};
pub use shapes::{
    decode_ev_pos_shape, event_check_in, event_position_set, event_result_move_position,
    player_height_matches_event_y, EvPosShape,
};

use crate::config::ActionDifficulty;
use crate::types::{EventListEntry, EventTriggerKind, ItemListEntry};

use super::EventFlags;

/// Maximum number of `Event_List` entries tracked per stage for rising-edge triggers (`cond = 1` / `7`).
pub const MAX_STAGE_EVENT_ENTRIES: usize = 256;

/// Index offset in `ev_work_no` (`0x01126380`) distinguishing `Item_List` pickups (`>= 0x400`)
/// from `Event_List` entries (`0..0x400`).
pub const EVENT_ITEM_INDEX_BASE: usize = 0x400;

/// Sentinel Y threshold (`45000.0`, `0x472fc800`): any `ev_pos` or `Item_List` with `y >= 45000.0`
/// ignores the vertical floor height window check.
pub const EVENT_IGNORE_HEIGHT_Y: f32 = 45_000.0;

/// Applies the secondary flag toggle side effects of `EventExecSubFlagSet`
/// (`SLUS_202.28` `0x001a9a90`).
pub fn event_exec_sub_flag_set(entry: &EventListEntry, flags: &mut EventFlags) {
    let f0 = entry.flag_0() as usize;
    if f0 != 0 && entry.flag_0_toggle_on_exec() {
        flags.set(f0, !entry.flag_0_inverted());
    }
    let f1 = entry.flag_1() as usize;
    if f1 != 0 && entry.flag_1_toggle_on_exec() {
        flags.set(f1, !entry.flag_1_inverted());
    }
}

/// Evaluates whether the two flag conditions (`flag_0` and `flag_1`) on `entry` are satisfied
/// (`EventCheck` at `SLUS_202.28` `0x001a7e60..0x001a7fb0`).
pub fn check_event_flag_conditions(entry: &EventListEntry, flags: &EventFlags) -> bool {
    let f0 = entry.flag_0() as usize;
    if f0 != 0 {
        let is_set = flags.get(f0);
        if entry.flag_0_inverted() {
            if is_set {
                return false;
            }
        } else if !is_set {
            return false;
        }
    }

    let f1 = entry.flag_1() as usize;
    if f1 != 0 {
        let is_set = flags.get(f1);
        if entry.flag_1_inverted() {
            if is_set {
                return false;
            }
        } else if !is_set {
            return false;
        }
    }

    true
}

/// Evaluates `EventItemConditionCheck` (`SLUS_202.28` `0x001ab1a0`) for a stage `Item_List` entry.
pub fn event_item_condition_check(
    item: &ItemListEntry,
    action_difficulty: ActionDifficulty,
    flags: &EventFlags,
) -> bool {
    let cond_diff = item.difficulty_condition();
    let diff_blocked = match action_difficulty {
        ActionDifficulty::Easy => matches!(cond_diff, 2 | 3 | 6),
        ActionDifficulty::Normal => matches!(cond_diff, 1 | 3 | 5),
        ActionDifficulty::Hard => matches!(cond_diff, 1 | 2 | 4),
    };
    if diff_blocked {
        return false;
    }

    match item.extra_condition() {
        1 => flags.get(219),
        2 => flags.get(108),
        3 => flags.get(208),
        _ => true,
    }
}

/// Per-frame input context for `EventCheck` (`SLUS_202.28` `0x001a7dc0`).
#[derive(Debug, Clone, Copy)]
pub struct EventEvalInput {
    pub player_pos: [f32; 3],
    pub player_rot_y: f32,
    /// `ev_active` (`0` = idle, `1` = non-blocking background event, `2` = blocking event).
    pub ev_active: u8,
    /// `true` if the Action/Search button (`pad->trg & 0x4000`) was pressed this frame.
    pub action_pressed: bool,
    /// `true` if the Map button was pressed this frame while map viewing is unavailable.
    pub map_disabled_pressed: bool,
    /// `true` on the first frame after entering a new room (`RoomChange != 0`).
    pub room_changed: bool,
    /// `true` on the first frame after loading a new stage (`StageChange != 0`).
    pub stage_changed: bool,
    /// Currently used inventory item ID (`use_item_id != 0` when using an item from the menu).
    pub used_item_id: Option<u8>,
    /// `true` if the game is currently running an automated attract/demo sequence.
    pub is_demo_mode: bool,
    /// Current action difficulty (for `Item_List` filtering).
    pub action_difficulty: ActionDifficulty,
    /// `true` if player controls/stamina allow search interactions.
    pub player_can_search: bool,
}

impl Default for EventEvalInput {
    fn default() -> Self {
        Self {
            player_pos: [0.0, 0.0, 0.0],
            player_rot_y: 0.0,
            ev_active: 0,
            action_pressed: false,
            map_disabled_pressed: false,
            room_changed: false,
            stage_changed: false,
            used_item_id: None,
            is_demo_mode: false,
            action_difficulty: ActionDifficulty::Normal,
            player_can_search: true,
        }
    }
}

/// Persistent rising-edge state for `cond = 1` (`TouchEdge`, `0x011264b0`) and
/// `cond = 7` (`ZoneEnterEdge`, `0x011266b0`).
#[derive(Debug, Clone)]
pub struct EventTriggerState {
    touch_edge_latched: [bool; MAX_STAGE_EVENT_ENTRIES],
    zone_edge_latched: [bool; MAX_STAGE_EVENT_ENTRIES],
}

impl Default for EventTriggerState {
    fn default() -> Self {
        Self::new()
    }
}

impl EventTriggerState {
    pub const fn new() -> Self {
        Self {
            touch_edge_latched: [false; MAX_STAGE_EVENT_ENTRIES],
            zone_edge_latched: [false; MAX_STAGE_EVENT_ENTRIES],
        }
    }

    pub fn reset(&mut self) {
        self.touch_edge_latched = [false; MAX_STAGE_EVENT_ENTRIES];
        self.zone_edge_latched = [false; MAX_STAGE_EVENT_ENTRIES];
    }
}

/// Evaluates a single `EventListEntry` at `index` and returns its trigger weight (`> 0.0` if triggered).
pub fn evaluate_event_entry(
    index: usize,
    entry: &EventListEntry,
    ev_pos: &[u8],
    flags: &EventFlags,
    input: &EventEvalInput,
    trigger_state: &mut EventTriggerState,
) -> f32 {
    if entry.is_sentinel() {
        return 0.0;
    }
    if input.is_demo_mode && entry.disabled_in_demo() {
        return 0.0;
    }
    if !check_event_flag_conditions(entry, flags) {
        return 0.0;
    }

    let Some(trigger_kind) = entry.trigger_kind() else {
        return 0.0;
    };
    let offset = entry.ev_pos_offset();

    match trigger_kind {
        EventTriggerKind::TouchContinuous => {
            if input.ev_active == 2 {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                1.0
            } else {
                0.0
            }
        }
        EventTriggerKind::TouchEdge => {
            if input.ev_active == 2 {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            let w = if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                1.0
            } else {
                0.0
            };
            let inside = w > 0.0;
            let latched = trigger_state
                .touch_edge_latched
                .get(index)
                .copied()
                .unwrap_or(false);
            if let Some(slot) = trigger_state.touch_edge_latched.get_mut(index) {
                *slot = inside;
            }
            if inside && !latched {
                w
            } else {
                0.0
            }
        }
        EventTriggerKind::ActionLook => {
            if input.ev_active != 0 || !input.action_pressed || !input.player_can_search {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    true,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::ActionTouch => {
            if input.ev_active != 0 || !input.action_pressed || !input.player_can_search {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::RoomInit => {
            if input.ev_active != 2 && input.room_changed {
                100.0
            } else {
                0.0
            }
        }
        EventTriggerKind::StageInit => {
            if input.ev_active != 2 && input.stage_changed {
                100.0
            } else {
                0.0
            }
        }
        EventTriggerKind::UseItemLook => {
            if input.ev_active != 0 || !input.player_can_search {
                return 0.0;
            }
            if input.used_item_id != Some(entry.required_item_id()) {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    true,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::ZoneEnterEdge => {
            if input.ev_active != 0 {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            let w = if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                1.0
            } else {
                0.0
            };
            let inside = w > 0.0;
            let latched = trigger_state
                .zone_edge_latched
                .get(index)
                .copied()
                .unwrap_or(false);
            if let Some(slot) = trigger_state.zone_edge_latched.get_mut(index) {
                *slot = inside;
            }
            if inside && !latched {
                w
            } else {
                0.0
            }
        }
        EventTriggerKind::UseItemTouch => {
            if input.ev_active != 0 || !input.player_can_search {
                return 0.0;
            }
            if input.used_item_id != Some(entry.required_item_id()) {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::Always => {
            if input.ev_active == 0 {
                100.0
            } else {
                0.0
            }
        }
        EventTriggerKind::MapDisabled => {
            if input.ev_active == 0 && input.map_disabled_pressed {
                100.0
            } else {
                0.0
            }
        }
    }
}

/// Evaluates `EventCheck` (`SLUS_202.28` `0x001a7dc0`) across the stage's `Event_List` and `Item_List`.
///
/// Returns `Some(work_no)` for the highest-weight matching candidate:
/// - `0..0x400`: index into `ev_list`
/// - `0x400..`: `0x400 + index` into `item_list`
pub fn event_check(
    ev_list: &[EventListEntry],
    item_list: &[ItemListEntry],
    ev_pos: &[u8],
    flags: &EventFlags,
    input: &EventEvalInput,
    trigger_state: &mut EventTriggerState,
) -> Option<usize> {
    let mut best_weight = 0.0f32;
    let mut best_index = None;

    for (idx, entry) in ev_list.iter().enumerate() {
        if entry.is_sentinel() {
            break;
        }
        let weight = evaluate_event_entry(idx, entry, ev_pos, flags, input, trigger_state);
        if weight > best_weight {
            best_weight = weight;
            best_index = Some(idx);
        }
    }

    if input.ev_active == 0 && input.action_pressed && input.player_can_search {
        for (idx, item) in item_list.iter().enumerate() {
            if item.is_sentinel() {
                break;
            }
            if !event_item_condition_check(item, input.action_difficulty, flags) {
                continue;
            }
            let base_flag = item.flag_id() as usize;
            // Already picked up if flag (base_flag + 1) is set
            if flags.get(base_flag + 1) {
                continue;
            }
            let vis_mode = item.visibility_mode();
            if vis_mode == 1 && !flags.get(base_flag) {
                continue;
            }
            if vis_mode == 2 && flags.get(base_flag) {
                continue;
            }
            let weight = item_check_look_point(item, input.player_pos, input.player_rot_y);
            if weight > best_weight {
                best_weight = weight;
                best_index = Some(EVENT_ITEM_INDEX_BASE + idx);
            }
        }
    }

    best_index
}
