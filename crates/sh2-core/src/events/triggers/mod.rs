//! Spatial Event Trigger Volumes, Look/Touch Geometry Evaluators, and `EventCheck` Selection
//! (`SLUS_202.28` `src/Event/event.c` at `0x001a7ed0..0x001a9be0`, `sh2pc.exe` `0x0048e070`).

pub mod look_raycast;
pub mod shapes;

pub use look_raycast::{
    event_check_look, event_check_look_line, event_check_look_point, item_check_look_point,
    EVENT_LOOK_BACK_OFFSET_MM, EVENT_LOOK_HALF_ANGLE_RAD, EVENT_LOOK_RADIUS_MM,
    EVENT_LOOK_RADIUS_SQ_MM2, ITEM_LOOK_MAX_DY_MM, ITEM_LOOK_MIN_DY_MM,
};
pub use shapes::{
    decode_ev_pos_shape, event_check_in, event_position_set, event_result_move_position,
    player_height_matches_event_y, EvPosShape,
};

use crate::config::ActionDifficulty;
use crate::inventory::InventoryManager;
use crate::types::{EventListEntry, EventShapeKind, EventTriggerKind, ItemListEntry};

use super::EventFlags;

/// Index offset in `ev_active` (`SLUS_202.28` `0x001a7ed0` / `sh2pc.exe` `0x0048e525`)
/// distinguishing `Item_List` pickups (`>= 0x400`) from `Event_List` entries (`0..0x400`).
pub const EVENT_ITEM_INDEX_BASE: usize = 0x400;

/// Sentinel Y threshold (`45000.0`, `0x472fc800`): any `ev_pos` with `y >= 45000.0`
/// ignores the vertical floor height window check (`EventCheckLook` `0x001a8780`, `EventCheckIn` `0x001a8d70`).
pub const EVENT_IGNORE_HEIGHT_Y: f32 = 45_000.0;

/// Applies the secondary flag toggle side effects of `EventExecSubFlagSet`
/// (`SLUS_202.28` `0x001a9a90`).
///
/// When `toggle_on_exec` (selector `2` / `5`) is set, `EventExecSubFlagSet` sets the flag if its
/// required condition was CLEAR (`selector 1 / 4 == 0`), or clears it if its required condition
/// was SET (`selector 1 / 4 == 1`).
pub fn event_exec_sub_flag_set(entry: &EventListEntry, flags: &mut EventFlags) {
    let f0 = entry.flag_0() as usize;
    if f0 != 0 && entry.flag_0_toggle_on_exec() {
        flags.set(f0, !entry.flag_0_expected_set());
    }
    let f1 = entry.flag_1() as usize;
    if f1 != 0 && entry.flag_1_toggle_on_exec() {
        flags.set(f1, !entry.flag_1_expected_set());
    }
}

/// Evaluates whether the two flag conditions (`flag_0` and `flag_1`) on `entry` are satisfied
/// (`EventCheck` at `SLUS_202.28` `0x001a7ed0` / `sh2pc.exe` `0x0048e070`).
///
/// Bit 31 (`flag_0_expected_set`) and Bit 15 (`flag_1_expected_set`) encode the required boolean
/// state: `false` (`0`) requires the flag to be CLEAR (`!flag`), `true` (`1`) requires it SET.
pub fn check_event_flag_conditions(entry: &EventListEntry, flags: &EventFlags) -> bool {
    let f0 = entry.flag_0() as usize;
    let f1 = entry.flag_1() as usize;
    (f0 == 0 || flags.get(f0) == entry.flag_0_expected_set())
        && (f1 == 0 || flags.get(f1) == entry.flag_1_expected_set())
}

/// Evaluates `EventItemConditionCheck` (`SLUS_202.28` `0x001ab1a0`, `sh2pc.exe` `0x0048c670`)
/// for a stage `Item_List` entry.
pub fn event_item_condition_check(
    item: &ItemListEntry,
    action_difficulty: ActionDifficulty,
    flags: &EventFlags,
) -> bool {
    let cond_diff = item.difficulty_condition();
    let diff_blocked = match action_difficulty {
        ActionDifficulty::Beginner | ActionDifficulty::Easy => matches!(cond_diff, 2 | 3 | 6),
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

/// Per-frame input context for `EventCheck` (`SLUS_202.28` `0x001a7ed0`, `sh2pc.exe` `0x0048e070`).
#[derive(Debug, Clone, Copy)]
pub struct EventEvalInput {
    /// Player world position `[x, y, z]` in PS2 millimeters (`_sh2jms + 0x20..0x28`).
    pub player_pos: [f32; 3],
    /// Player yaw rotation in radians (`_sh2jms + 0x34`).
    pub player_rot_y: f32,
    /// `true` if the Action/Search button (`PlayerEventButtonCheck(0)`) was pressed this frame.
    pub action_pressed: bool,
    /// Currently used inventory item ID (`param_2 != 0` when using an item from the menu).
    pub used_item_id: Option<u8>,
    /// `true` when in a dark / night area (`sh2gfw_Get_NightOrDay() != 0`, `SLUS_202.28` `0x001a80a8`).
    pub is_dark_room: bool,
    /// `true` when James's flashlight is on (`LightSpotOnOffCheck() != 0`, `SLUS_202.28` `0x001aad70`).
    pub flashlight_on: bool,
    /// Current action difficulty (for `Item_List` filtering in `EventItemConditionCheck`).
    pub action_difficulty: ActionDifficulty,
    /// `true` if player controls/stamina allow search interactions (`param_1 != 0`).
    pub player_can_search: bool,
}

impl Default for EventEvalInput {
    fn default() -> Self {
        Self {
            player_pos: [0.0, 0.0, 0.0],
            player_rot_y: 0.0,
            action_pressed: false,
            used_item_id: None,
            is_dark_room: false,
            flashlight_on: true,
            action_difficulty: ActionDifficulty::Normal,
            player_can_search: true,
        }
    }
}

impl EventEvalInput {
    /// Returns `true` when light-gated events (`(cond >> 2) & 1 != 0`) are allowed to trigger
    /// (`SLUS_202.28` `0x001a80a8..0x001a80ec`, `sh2pc.exe` `0x0048e150..0x0048e1a9`):
    /// either `sh2gfw_Get_NightOrDay() == 0` (`!is_dark_room`) or `LightSpotOnOffCheck() != 0` (`flashlight_on`).
    #[inline]
    pub const fn has_light(&self) -> bool {
        !self.is_dark_room || self.flashlight_on
    }
}

#[inline]
const fn is_area_shape(shape: EventShapeKind) -> bool {
    matches!(
        shape,
        EventShapeKind::Rectangle | EventShapeKind::Triangle | EventShapeKind::Quad
    )
}

/// Evaluates a single `EventListEntry` against authentic `EventCheck` (`SLUS_202.28` `0x001a7ed0`,
/// `sh2pc.exe` `0x0048e070`) rules.
///
/// Returns `true` if this entry matches on the current frame.
pub fn evaluate_event_entry(
    entry: &EventListEntry,
    ev_pos: &[u8],
    flags: &EventFlags,
    inventory: &InventoryManager,
    input: &EventEvalInput,
) -> bool {
    if entry.is_sentinel() {
        return false;
    }
    // `SLUS_202.28` `0x001a80ec`, `sh2pc.exe` `0x0048e1a9`: dark-room light gating `(cond >> 2) & 1`.
    if !input.has_light() && entry.requires_light() {
        return false;
    }

    let Some(trigger_kind) = entry.trigger_kind() else {
        return false;
    };

    let action_active = input.action_pressed && input.player_can_search;
    let req_item = entry.required_item_id();

    // Button / used-item / carried-item pre-filter (`SLUS_202.28` `0x001a8108..0x001a81a8`,
    // `sh2pc.exe` `0x0048e1af..0x0048e218`).
    let prefilter_ok = if let Some(used_id) = input.used_item_id {
        matches!(
            trigger_kind,
            EventTriggerKind::UseItemLook
                | EventTriggerKind::UseItemIn
                | EventTriggerKind::ActionHasItemOrUseItem
        ) && used_id == req_item
    } else {
        match trigger_kind {
            EventTriggerKind::Unconditional0
            | EventTriggerKind::Unconditional1
            | EventTriggerKind::PassiveLook
            | EventTriggerKind::PassiveIn => true,
            EventTriggerKind::Action => action_active,
            EventTriggerKind::UseItemLook | EventTriggerKind::UseItemIn => false,
            EventTriggerKind::ActionHasItem | EventTriggerKind::ActionHasItemOrUseItem => {
                action_active && inventory.has_item(req_item)
            }
            EventTriggerKind::PassiveHasItemLook | EventTriggerKind::PassiveHasItemIn => {
                inventory.has_item(req_item)
            }
        }
    };
    if !prefilter_ok {
        return false;
    }

    // Condition flags (`SLUS_202.28` `0x001a81a8..0x001a82e0`, `sh2pc.exe` `0x0048e220..0x0048e2d0`).
    if !check_event_flag_conditions(entry, flags) {
        return false;
    }

    // Spatial shape check (`SLUS_202.28` `0x001a82e4..0x001a8520`, `sh2pc.exe` `0x0048e2d8..0x0048e356`).
    let offset = entry.ev_pos_offset();
    match trigger_kind {
        EventTriggerKind::Unconditional0 | EventTriggerKind::Unconditional1 => true,
        EventTriggerKind::Action
        | EventTriggerKind::ActionHasItem
        | EventTriggerKind::ActionHasItemOrUseItem => {
            let Some(shape) = entry.shape_kind() else {
                return false;
            };
            if is_area_shape(shape) {
                event_check_in(ev_pos, offset, shape, input.player_pos)
            } else {
                event_check_look(ev_pos, offset, shape, input.player_pos, input.player_rot_y)
            }
        }
        EventTriggerKind::PassiveLook
        | EventTriggerKind::UseItemLook
        | EventTriggerKind::PassiveHasItemLook => {
            let Some(shape) = entry.shape_kind() else {
                return false;
            };
            if is_area_shape(shape) {
                return false;
            }
            event_check_look(ev_pos, offset, shape, input.player_pos, input.player_rot_y)
        }
        EventTriggerKind::PassiveIn
        | EventTriggerKind::UseItemIn
        | EventTriggerKind::PassiveHasItemIn => {
            let Some(shape) = entry.shape_kind() else {
                return false;
            };
            if !is_area_shape(shape) {
                return false;
            }
            event_check_in(ev_pos, offset, shape, input.player_pos)
        }
    }
}

/// Evaluates `EventCheck` (`SLUS_202.28` `0x001a7ed0`, `sh2pc.exe` `0x0048e070`) across the stage's
/// `Event_List` and `Item_List` using authentic first-match-wins semantics.
///
/// Returns `Some(work_no)` for the first matching candidate:
/// - `0..0x400`: index into `ev_list`
/// - `0x400..`: `0x400 + index` into `item_list`
pub fn event_check(
    ev_list: &[EventListEntry],
    item_list: &[ItemListEntry],
    ev_pos: &[u8],
    flags: &EventFlags,
    inventory: &InventoryManager,
    input: &EventEvalInput,
) -> Option<usize> {
    for (idx, entry) in ev_list.iter().enumerate() {
        if entry.is_sentinel() {
            break;
        }
        if evaluate_event_entry(entry, ev_pos, flags, inventory, input) {
            return Some(idx);
        }
    }

    if input.used_item_id.is_none() && input.action_pressed && input.player_can_search {
        for (idx, item) in item_list.iter().enumerate() {
            if item.is_sentinel() {
                break;
            }
            let base_flag = item.flag_id() as usize;
            // `SLUS_202.28` `0x001a7ed0` lines 634-635 & `sh2pc.exe` `0x0048e3b0`:
            // available only when both `flag(base_flag) == 0` and `flag(base_flag + 1) == 0`.
            if flags.get(base_flag) || flags.get(base_flag + 1) {
                continue;
            }
            if !event_item_condition_check(item, input.action_difficulty, flags) {
                continue;
            }
            if item_check_look_point(item, input.player_pos, input.player_rot_y)
                && input.has_light()
            {
                return Some(EVENT_ITEM_INDEX_BASE + idx);
            }
        }
    }

    None
}
