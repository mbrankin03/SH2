//! `const` constructors used by the static `tables_*.rs` stage descriptors to transcribe retail
//! `sh2pc.exe` `Event_List`, `Item_List`, `Enemy_List`, and 64-byte `Stage_Data` records.

use super::{RetailEnemySpawn, RetailItemSpawn};
use crate::events::triggers::EventEvalInput;
use crate::inventory::InventoryManager;
use crate::types::{EventListEntry, StageDataHeader};

pub(crate) const fn ev(flag: u32, cond: u32, rslt0: u32, rslt1: u32) -> EventListEntry {
    EventListEntry {
        flag,
        cond,
        rslt0,
        rslt1,
    }
}

pub(crate) const fn gi(
    pos_x_bits: u32,
    pos_z_bits: u32,
    pos_y_hi16: u16,
    rot_y_hi16: u16,
    flags_packed: u32,
) -> RetailItemSpawn {
    RetailItemSpawn {
        pos_x_bits,
        pos_z_bits,
        pos_y_hi16,
        rot_y_hi16,
        flags_packed,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn en(
    kind: i16,
    id: i16,
    pos_x: i32,
    pos_z: i32,
    pos_y: i16,
    rot_y: i16,
    cond_type: u16,
    diff_cond: u16,
) -> RetailEnemySpawn {
    RetailEnemySpawn {
        kind,
        id,
        pos_x,
        pos_z,
        pos_y,
        rot_y,
        cond_type,
        diff_cond,
    }
}

/// Builds a [`StageDataHeader`] from the retail PC 64-byte (`16 x u32`) `Stage_Data` record.
///
/// Arguments are passed in [`StageDataHeader`] field order, NOT retail word order. Retail words
/// `0..=9` map linearly; the three trailing callbacks are taken from:
/// - `sound_call_after_load_fn` = `word[15]` (`+0x3c`, called once after load at `0x00402129`),
/// - `bgm_control_fn` = `word[12]` (`+0x30`, SH2:EE `MusicBoxVolume.cpp` `Hotel2FStageAddr + 0x30`),
/// - `chara_data_clear_fn` = `word[14]` (`+0x38`, called by `0x00492f80` before the `0x818`-byte clear).
///
/// Retail-only words `10` (`+0x28` overlay flag), `11` (`+0x2c` room/door table) and `13`
/// (`+0x34` data pointer) have no PS2 `StageDataHeader` counterpart and are not stored.
#[allow(clippy::too_many_arguments)]
pub(crate) const fn hdr(
    ev_list_ptr: u32,
    ev_pos_ptr: u32,
    ev_prog_ptr: u32,
    item_list_ptr: u32,
    model_list_ptr: u32,
    enemy_list_ptr: u32,
    stage_init_fn: u32,
    room_init_fn: u32,
    all_time_fn: u32,
    glb_crd: i32,
    sound_call_after_load_fn: u32,
    bgm_control_fn: u32,
    chara_data_clear_fn: u32,
) -> StageDataHeader {
    StageDataHeader {
        ev_list_ptr,
        ev_pos_ptr,
        ev_prog_ptr,
        item_list_ptr,
        model_list_ptr,
        enemy_list_ptr,
        stage_init_fn,
        room_init_fn,
        all_time_fn,
        glb_crd,
        sound_call_after_load_fn,
        bgm_control_fn,
        chara_data_clear_fn,
    }
}

/// Retail PC `Event_List` trigger discriminants (`(cond >> 28) & 0xf`, decoded by `EventCheck`
/// at `sh2pc.exe` `0x0048e1a9..0x0048e35b` via jump tables `0x0048e5b4` and `0x0048e5d8`/`0x0048e5e4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RetailTriggerKind {
    /// `0`: Unconditional every frame whenever `flag_0`/`flag_1` hold (`0x0048e1de` & `0x0048e2cb`).
    Always0 = 0,
    /// `1`: Unconditional every frame whenever `flag_0`/`flag_1` hold (`0x0048e1de` & `0x0048e2cb`).
    Always1 = 1,
    /// `2`: Action button pressed, then look (`shape 0..=10`) or area (`shape 11..=13`, `0x0048e1ea`).
    ActionLookOrArea = 2,
    /// `3`: Look (`shape 0..=10`) or area (`shape 11..=13`) without Action button (`0x0048e2f4`).
    Look = 3,
    /// `4`: Area (`shape 11..=13`) without Action button (`0x0048e324`).
    Area = 4,
    /// `5`: Menu item use (`used_item_id == Some(item_id)`), then look or area (`0x0048e1b3`).
    UseItemLook = 5,
    /// `6`: Menu item use (`used_item_id == Some(item_id)`), then area (`0x0048e1bc`).
    UseItemArea = 6,
    /// `7`: Action button pressed while carrying `item_id` in `0x01f7a7e0` (`0x0048e1ee`).
    ActionCarryingItem = 7,
    /// `8`: Carrying `item_id` in `0x01f7a7e0`, then look or area (`0x0048e1f6`).
    CarryingItemLook = 8,
    /// `9`: Carrying `item_id` in `0x01f7a7e0`, then area (`0x0048e1f6`).
    CarryingItemArea = 9,
    /// `10`: Menu item use (`0x0048e1c1`) OR Action button pressed while carrying `item_id` (`0x0048e1ee`).
    UseOrActionCarryingItem = 10,
}

impl RetailTriggerKind {
    /// Converts a 4-bit retail trigger nibble (`0..=10`) into [`RetailTriggerKind`].
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Always0),
            1 => Some(Self::Always1),
            2 => Some(Self::ActionLookOrArea),
            3 => Some(Self::Look),
            4 => Some(Self::Area),
            5 => Some(Self::UseItemLook),
            6 => Some(Self::UseItemArea),
            7 => Some(Self::ActionCarryingItem),
            8 => Some(Self::CarryingItemLook),
            9 => Some(Self::CarryingItemArea),
            10 => Some(Self::UseOrActionCarryingItem),
            _ => None,
        }
    }

    /// Extracts the [`RetailTriggerKind`] from a raw retail [`EventListEntry`].
    pub const fn from_entry(entry: &EventListEntry) -> Option<Self> {
        Self::from_raw(((entry.cond >> 28) & 0xf) as u8)
    }

    /// Evaluates the button, used-item, and carried-item (`0x01f7a7e0`) pre-filter (`0x0048e1af..0x0048e218`).
    pub fn matches_input(
        self,
        required_item_id: u8,
        input: &EventEvalInput,
        inventory: &InventoryManager,
    ) -> bool {
        if let Some(used_id) = input.used_item_id {
            return matches!(
                self,
                Self::UseItemLook | Self::UseItemArea | Self::UseOrActionCarryingItem
            ) && used_id == required_item_id;
        }
        match self {
            Self::Always0 | Self::Always1 | Self::Look | Self::Area => true,
            Self::ActionLookOrArea => input.action_pressed,
            Self::UseItemLook | Self::UseItemArea => false,
            Self::ActionCarryingItem | Self::UseOrActionCarryingItem => {
                input.action_pressed && inventory.has_item(required_item_id)
            }
            Self::CarryingItemLook | Self::CarryingItemArea => inventory.has_item(required_item_id),
        }
    }
}

/// Maps a retail `sh2pc.exe` [`EventListEntry`] (`0x0048e5b4` & `0x0048e7a4`) onto `SLUS_202.28`
/// trigger and result discriminants expected by [`crate::events::EventEngine::tick_standard`].
///
/// Retail `trg == 0` and `trg == 1` both map to `Always` (`9`): in `EventCheck`
/// (`0x0048e1db..0x0048e2cb`) both fall through the button/item jump (`ecx = trg - 2 > 8u`) and
/// the spatial jump (`ja 0x0048e572`), firing every frame whenever `flag_0`/`flag_1` hold.
/// For carried-item triggers (`7..=10`, including `trg == 10` key doors that fire on either menu
/// item use or pressing Action while carrying the key), prefer
/// [`normalize_retail_event_entry_for_input`].
pub const fn normalize_retail_event_entry(entry: EventListEntry) -> EventListEntry {
    if entry.is_sentinel() {
        return entry;
    }
    let trg = match (entry.cond >> 28) & 0xf {
        0 | 1 => 9,
        2 | 7 => 2,
        3 | 4 | 8 | 9 => 0,
        5 | 10 => 6,
        6 => 8,
        other => other,
    };
    let rslt = match (entry.rslt0 >> 28) & 0xf {
        3 => 5,
        4 => 3,
        5 => 6,
        6 => 4,
        other => other,
    };
    EventListEntry {
        flag: entry.flag,
        cond: (entry.cond & 0x0fff_ffff) | (trg << 28),
        rslt0: (entry.rslt0 & 0x0fff_ffff) | (rslt << 28),
        rslt1: entry.rslt1,
    }
}

/// Maps a retail `sh2pc.exe` [`EventListEntry`] onto `SLUS_202.28` trigger/result nibbles while
/// resolving carried-item (`0x01f7a7e0`) vs menu-used-item (`used_item_id`) branches for
/// `trg == 7..=10` (`sh2pc.exe` `0x0048e1af..0x0048e218`).
///
/// In particular, for `trg == 10` (`UseOrActionCarryingItem`, used by all 9 Act 2 key-locked
/// doors), when no item is being used from the menu (`input.used_item_id.is_none()`) and the
/// player presses Action while carrying `entry.required_item_id()`, `trg` normalizes to
/// `ActionLook` (`2`); otherwise it normalizes to `UseItemLook` (`6`).
pub fn normalize_retail_event_entry_for_input(
    entry: EventListEntry,
    input: &EventEvalInput,
    inventory: &InventoryManager,
) -> EventListEntry {
    if entry.is_sentinel() {
        return entry;
    }
    let base = normalize_retail_event_entry(entry);
    let req_item = entry.required_item_id();
    let carried = input.used_item_id.is_none() && inventory.has_item(req_item);
    let trg = match (entry.cond >> 28) & 0xf {
        7 => {
            if carried {
                2
            } else {
                6
            }
        }
        8 | 9 => {
            if carried {
                0
            } else {
                6
            }
        }
        10 => {
            if carried && input.action_pressed {
                2
            } else {
                6
            }
        }
        _ => return base,
    };
    EventListEntry {
        cond: (base.cond & 0x0fff_ffff) | (trg << 28),
        ..base
    }
}
