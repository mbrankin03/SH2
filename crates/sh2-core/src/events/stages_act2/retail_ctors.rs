//! `const` constructors used by the static `tables_*.rs` stage descriptors to transcribe retail
//! `sh2pc.exe` `Event_List`, `Item_List`, `Enemy_List`, and 64-byte `Stage_Data` records.

use super::{RetailEnemySpawn, RetailItemSpawn};
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
