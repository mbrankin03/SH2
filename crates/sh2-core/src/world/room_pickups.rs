//! 3D Room Item Pickup Spawner & Culling (`Event/chara_admin.c` `ConnectCharaWorkAdminIn`
//! `0x001cb8c0`, `ConnectCharaWorkAdminOut` `0x001cafe0`, `Chacter/bg_obj_stay.c`
//! `StayObjectFunction` `0x00102eb0`).
//!
//! Resolves active 3D `.mdl` pickup instances (`0x0700`, `0x0701`, `0x0703`, `0x0723`, `0x0724`,
//! `0x0733`, `0x0749`) from a stage's `Item_List` (`CampaignPickupSpawn`) for rendering and
//! `EventExecItem` deletion.

use crate::campaign::CampaignPickupSpawn;
use crate::config::ActionDifficulty;
use crate::events::triggers::event_item_condition_check;
use crate::events::EventFlags;
use crate::types::event::{char_to_float2, pickup_model_rel_path, ItemListEntry, ITEM_KIND_TABLE};

use super::bg_obj::{is_within_outdoor_prop_range, StayObjectState};

/// Active 3D room pickup instance ready for rendering in the current stage/room.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveRoomPickup {
    /// Model ID (`0x0700..=0x0749` from `ITEM_KIND_TABLE`).
    pub chr_id: u16,
    /// Pickup event flag ID (`Item_List` `flag & 0x3fff`).
    pub flag_id: u16,
    /// Relative `.mdl` path under `data/` (e.g. `"chr/item/x_drink.mdl"`).
    pub asset_rel_path: &'static str,
    /// World position `[x, y, z]` in PS2 mm.
    pub position_mm: [f32; 3],
    /// World Y rotation in radians (`CharToFloat2` at `Item_List + 0x0a`, `0x001cb480`).
    pub rot_y_rad: f32,
    /// `StayObjectFunction` (`0x00102eb0`) state (`shadow_y_mm = y - 150.0`).
    pub stay_state: StayObjectState,
}

/// Evaluates which `Item_List` 3D pickups should be active in the current frame
/// (`ConnectCharaWorkAdminIn` `0x001cb8c0` / `ConnectCharaWorkAdminOut` `0x001cafe0`).
#[must_use]
pub fn resolve_active_room_pickups(
    spawns: &[CampaignPickupSpawn],
    flags: &EventFlags,
    difficulty: ActionDifficulty,
    is_outdoor: bool,
    player_pos_mm: [f32; 3],
) -> Vec<ActiveRoomPickup> {
    let mut out = Vec::new();
    for spawn in spawns {
        if spawn.item_kind_index == 0 {
            continue;
        }
        let flag_id = spawn.flag_id as usize;
        // `ConnectCharaWorkAdminIn` (`0x001cb8c0`): skip if `flag(id)` or `flag(id + 1)` is set.
        if flags.get(flag_id) || flags.get(flag_id + 1) {
            continue;
        }
        let kind_idx = (spawn.item_kind_index as usize).min(ITEM_KIND_TABLE.len() - 1);
        let kind = ITEM_KIND_TABLE[kind_idx];
        if kind.chr_id <= 0 {
            continue;
        }
        let Some(asset_rel_path) = pickup_model_rel_path(kind.chr_id) else {
            continue;
        };

        // Reconstruct a packed `ItemListEntry` to reuse `event_item_condition_check` (`0x001ab1a0`),
        // preserving `extra_condition` in bits `20..=24` (`ItemListElement` selector `2`).
        let packed_flag = ((u32::from(spawn.item_kind_index) & 0x7) << 29)
            | ((u32::from(spawn.difficulty_condition) & 0x7) << 26)
            | ((u32::from(spawn.extra_condition) & 0x1f) << 20)
            | (u32::from(spawn.flag_id) & 0x3fff);
        let entry = ItemListEntry {
            pos_x: spawn.position_mm[0],
            pos_z: spawn.position_mm[2],
            pos_y: 0,
            rot_y: spawn.rot_y_raw as u16,
            flag: packed_flag,
        };
        if !event_item_condition_check(&entry, difficulty, flags) {
            continue;
        }

        let chr_id = kind.chr_id as u16;
        if is_outdoor && !is_within_outdoor_prop_range(chr_id, spawn.position_mm, player_pos_mm) {
            continue;
        }

        let rot_y_rad = char_to_float2((spawn.rot_y_raw as u16).to_le_bytes());
        let mut stay_state = StayObjectState::default();
        stay_state.step(spawn.position_mm[1]);

        out.push(ActiveRoomPickup {
            chr_id,
            flag_id: spawn.flag_id,
            asset_rel_path,
            position_mm: spawn.position_mm,
            rot_y_rad,
            stay_state,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::event::float_to_char2;

    fn make_spawn(kind: u8, flag_id: u16, pos: [f32; 3], diff_cond: u8) -> CampaignPickupSpawn {
        let rot_raw = i16::from_le_bytes(float_to_char2(1.5));
        CampaignPickupSpawn {
            position_mm: pos,
            rot_y_raw: rot_raw,
            item_kind_index: kind,
            difficulty_condition: diff_cond,
            extra_condition: 0,
            flag_id,
        }
    }

    #[test]
    fn resolves_active_pickups_and_respects_flags_and_outdoor_culling() {
        let spawns = vec![
            make_spawn(4, 100, [100.0, 50.0, 100.0], 0), // Health drink (0x0700)
            make_spawn(1, 102, [20_000.0, 0.0, 0.0], 0), // Handgun bullets far away
            make_spawn(7, 104, [300.0, 10.0, 200.0], 0), // Revolver bullets (0x0749)
        ];

        let mut flags = EventFlags::new();
        let active_indoor = resolve_active_room_pickups(
            &spawns,
            &flags,
            ActionDifficulty::Normal,
            false,
            [0.0, 0.0, 0.0],
        );
        assert_eq!(active_indoor.len(), 3);
        assert_eq!(active_indoor[0].chr_id, 0x0700);
        assert_eq!(active_indoor[0].asset_rel_path, "chr/item/x_drink.mdl");
        assert!((active_indoor[0].stay_state.shadow_y_mm - (-100.0)).abs() < 1e-3);
        assert_eq!(active_indoor[2].chr_id, 0x0749);
        assert_eq!(
            active_indoor[2].asset_rel_path,
            "chr2/item/x_bullt_revolver.mdl"
        );

        // Outdoor culling: 20,000 mm > 2500 + 8000 = 10,500 mm
        let active_outdoor = resolve_active_room_pickups(
            &spawns,
            &flags,
            ActionDifficulty::Normal,
            true,
            [0.0, 0.0, 0.0],
        );
        assert_eq!(active_outdoor.len(), 2);

        // Picking up flag 100 sets flag 101 -> removed from active list.
        flags.set(101, true);
        let after_pickup = resolve_active_room_pickups(
            &spawns,
            &flags,
            ActionDifficulty::Normal,
            true,
            [0.0, 0.0, 0.0],
        );
        assert_eq!(after_pickup.len(), 1);
        assert_eq!(after_pickup[0].flag_id, 104);
    }
}
