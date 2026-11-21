//! Retail `sh2pc.exe` byte-for-byte parity tests for `SH2-114` static stage tables (Stages `16`
//! & `25..=58`) and riddle data tables. Skips cleanly when the retail rip is not symlinked at the
//! workspace root (GitHub Actions CI).

use std::path::{Path, PathBuf};

use sh2_core::config::ActionDifficulty;
use sh2_core::events::*;
use sh2_core::inventory::InventoryManager;
use sh2_core::types::{EventListEntry, EventTriggerKind};

/// Resolves `rel` under the workspace-root retail rip symlink (`CARGO_MANIFEST_DIR/../..`).
fn retail_path(rel: &str) -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip")
        .join(rel);
    p.exists().then_some(p)
}

fn u16_le(pe: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([pe[o], pe[o + 1]])
}
fn i16_le(pe: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([pe[o], pe[o + 1]])
}
fn u32_le(pe: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([pe[o], pe[o + 1], pe[o + 2], pe[o + 3]])
}
fn i32_le(pe: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([pe[o], pe[o + 1], pe[o + 2], pe[o + 3]])
}

#[test]
fn retail_sh2pc_binary_and_stage_assets_match_static_tables_bit_for_bit() {
    let Some(exe_path) = retail_path("Silent Hill 2/sh2pc.exe") else {
        return;
    };
    let pe = std::fs::read(exe_path).expect("read sh2pc.exe");
    let jt_off = 0x0049_5048usize - 0x0040_0000;

    assert_eq!(
        f32::from_bits(u32_le(&pe, 0x002a_91e8)),
        RETAIL_LOOK_RADIUS_MM
    );
    assert_eq!(
        &pe[0x004c_a414..0x004c_a434],
        BRIEFCASE_TRUNK_DRUMS.as_flattened()
    );
    // Gravestone quad extents (`0x008c4670`) used by `gravestone_board_verts`.
    let ext = [0usize, 2, 4, 6].map(|k| i16_le(&pe, 0x004c_4670 + k));
    assert_eq!(ext, GRAVESTONE_BOARD_VERT_EXTENTS);
    let boards = [
        GravestoneBoard::White,
        GravestoneBoard::Black,
        GravestoneBoard::Red,
    ];
    for (idx, b) in boards.into_iter().enumerate() {
        let o = 0x004c_4650 + idx * 8;
        let uv = gravestone_board_uv(b, false);
        let exp = [
            u16_le(&pe, o),
            u16_le(&pe, o + 2),
            u16_le(&pe, o + 4),
            u16_le(&pe, o + 6),
        ];
        assert_eq!([uv.u1, uv.v1, uv.u2, uv.v2], exp.map(f32::from));
    }

    for &stage_id in &ALL_ACT2_STAGE_IDS {
        let table = stage_table_act2(stage_id).expect("stage table");
        let case_va = u32_le(&pe, jt_off + ((stage_id as usize) - 1) * 4);
        let stg_va = u32_le(&pe, (case_va as usize) - 0x0040_0000 + 1);
        assert_eq!(stg_va, table.retail_stage_data_va, "Stage {stage_id} VA");

        let h_off = (stg_va as usize) - 0x0040_0000;
        let h = &table.header;
        let exp_hdr = [
            h.ev_list_ptr,
            h.ev_pos_ptr,
            h.ev_prog_ptr,
            h.item_list_ptr,
            h.model_list_ptr,
            h.enemy_list_ptr,
            h.stage_init_fn,
            h.room_init_fn,
            h.all_time_fn,
            h.glb_crd as u32,
            h.sound_call_after_load_fn,
            h.bgm_control_fn,
            h.chara_data_clear_fn,
        ];
        // Retail 64-byte `Stage_Data`: words 0..=9 linear, then `+0x3c`, `+0x30`, `+0x38`.
        const RETAIL_WORDS: [usize; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 12, 14];
        for (&w, &exp) in RETAIL_WORDS.iter().zip(exp_hdr.iter()) {
            assert_eq!(
                u32_le(&pe, h_off + w * 4),
                exp,
                "Stage {stage_id} word[{w}]"
            );
        }

        if h.ev_list_ptr >= 0x0040_1000 {
            let ev_off = (h.ev_list_ptr as usize) - 0x0040_0000;
            for (i, &e) in table.event_list[..table.active_event_count()]
                .iter()
                .enumerate()
            {
                let b = ev_off + i * 16;
                let bin = EventListEntry {
                    flag: u32_le(&pe, b),
                    cond: u32_le(&pe, b + 4),
                    rslt0: u32_le(&pe, b + 8),
                    rslt1: u32_le(&pe, b + 12),
                };
                assert_eq!(e, bin);
            }
        }
        if h.ev_pos_ptr >= 0x0040_1000 && !table.pos_list.is_empty() {
            let p_off = (h.ev_pos_ptr as usize) - 0x0040_0000;
            assert_eq!(&pe[p_off..p_off + table.pos_list.len()], table.pos_list);
        }
        if h.ev_prog_ptr >= 0x0040_1000 {
            let pr_off = (h.ev_prog_ptr as usize) - 0x0040_0000;
            for (i, &va) in table.ev_prog_vas.iter().enumerate() {
                assert_eq!(u32_le(&pe, pr_off + i * 4), va);
            }
        }
        if h.item_list_ptr >= 0x0040_1000 {
            let gi_off = (h.item_list_ptr as usize) - 0x0040_0000;
            for (i, &it) in table.retail_items.iter().enumerate() {
                let b = gi_off + i * 16;
                let bin = RetailItemSpawn {
                    pos_x_bits: u32_le(&pe, b),
                    pos_z_bits: u32_le(&pe, b + 4),
                    pos_y_hi16: u16_le(&pe, b + 8),
                    rot_y_hi16: u16_le(&pe, b + 10),
                    flags_packed: u32_le(&pe, b + 12),
                };
                assert_eq!(it, bin);
            }
        }
        if h.enemy_list_ptr >= 0x0040_1000 {
            let en_off = (h.enemy_list_ptr as usize) - 0x0040_0000;
            for (i, &en) in table.retail_enemies.iter().enumerate() {
                let b = en_off + i * 20;
                let bin = RetailEnemySpawn {
                    kind: i16_le(&pe, b),
                    id: i16_le(&pe, b + 2),
                    pos_x: i32_le(&pe, b + 4),
                    pos_z: i32_le(&pe, b + 8),
                    pos_y: i16_le(&pe, b + 12),
                    rot_y: i16_le(&pe, b + 14),
                    cond_type: u16_le(&pe, b + 16),
                    diff_cond: u16_le(&pe, b + 18),
                };
                assert_eq!(en, bin);
            }
        }
    }

    for dir in ["ps", "ob", "rr", "hp", "bw"] {
        if let Some(p) = retail_path(&format!("Silent Hill 2/data/bg/{dir}")) {
            assert!(p.is_dir());
        }
    }
}

#[test]
fn retail_trg10_key_doors_fire_on_action_while_carrying_key_or_menu_item_use() {
    // All 9 retail `trg == 10` (`UseOrActionCarryingItem`, `0x0048e1c1` & `0x0048e1ee`) doors across Act 2:
    let expected_trg10_doors: [(u16, usize, u8); 9] = [
        (26, 12, 38), // Hospital BF_B: Basement storeroom key (38)
        (39, 6, 46),  // Hotel BF_F: Bar key (46, lantern lit)
        (39, 7, 46),  // Hotel BF_F: Bar key (46, lantern unlit message)
        (41, 27, 44), // Hotel 2F_F: Room 204 key (44)
        (41, 53, 45), // Hotel 2F_F: Employee elevator key (45)
        (42, 6, 48),  // Hotel 3F_F: Hotel stairway key (48)
        (42, 10, 43), // Hotel 3F_F: Room 312 key (43)
        (46, 8, 25),  // Hotel 3F_B: Dog key (25)
        (56, 33, 82), // Wish 2F: Acacia key (82)
    ];

    // 10-byte `ev_pos` point at `(0, ~0, 0)` and player at `(0, 0, -200)` facing `+Z` (`0.0`).
    let ps2_origin_point = [0u8; 10];
    let action_no_item = EventEvalInput {
        player_pos: [0.0, 0.0, -200.0],
        player_rot_y: 0.0,
        ev_active: 0,
        action_pressed: true,
        map_disabled_pressed: false,
        room_changed: false,
        stage_changed: false,
        used_item_id: None,
        is_demo_mode: false,
        action_difficulty: ActionDifficulty::Normal,
        player_can_search: true,
    };

    for (stage_id, ev_idx, key_id) in expected_trg10_doors {
        let table = stage_table_act2(stage_id).expect("stage table");
        let raw = table.event_list[ev_idx];
        assert_eq!(
            RetailTriggerKind::from_entry(&raw),
            Some(RetailTriggerKind::UseOrActionCarryingItem)
        );
        assert_eq!(raw.required_item_id(), key_id);

        // Pin `ev_pos_offset = 0` and `shape = Point (0)` so `evaluate_event_entry` tests the point at origin.
        let mut raw_at_origin = raw;
        raw_at_origin.cond &= !0x0fff_f000;

        let mut flags = EventFlags::new();
        if raw.flag_0() != 0 {
            flags.set(raw.flag_0() as usize, !raw.flag_0_inverted());
        }
        if raw.flag_1() != 0 {
            flags.set(raw.flag_1() as usize, !raw.flag_1_inverted());
        }

        // 1. Without carrying the key, pressing Action must NOT fire.
        let empty_inv = InventoryManager::new();
        assert!(!RetailTriggerKind::UseOrActionCarryingItem.matches_input(
            key_id,
            &action_no_item,
            &empty_inv
        ));
        let norm_missing =
            normalize_retail_event_entry_for_input(raw_at_origin, &action_no_item, &empty_inv);
        assert_eq!(
            norm_missing.trigger_kind(),
            Some(EventTriggerKind::UseItemLook)
        );
        let mut ts = EventTriggerState::new();
        assert_eq!(
            evaluate_event_entry(
                0,
                &norm_missing,
                &ps2_origin_point,
                &flags,
                &action_no_item,
                &mut ts
            ),
            0.0
        );

        // 2. Carrying the key and pressing Action with `used_item_id == None` MUST fire (`ActionLook`).
        let mut carried_inv = InventoryManager::new();
        carried_inv.give_raw(key_id, 1);
        assert!(RetailTriggerKind::UseOrActionCarryingItem.matches_input(
            key_id,
            &action_no_item,
            &carried_inv
        ));
        let norm_carried =
            normalize_retail_event_entry_for_input(raw_at_origin, &action_no_item, &carried_inv);
        assert_eq!(
            norm_carried.trigger_kind(),
            Some(EventTriggerKind::ActionLook)
        );
        assert!(
            evaluate_event_entry(
                0,
                &norm_carried,
                &ps2_origin_point,
                &flags,
                &action_no_item,
                &mut ts
            ) > 0.0,
            "stage {stage_id} ev[{ev_idx}] key {key_id} failed action-while-carrying fire"
        );
        let adapted_list = table.engine_event_list_for_input(&action_no_item, &carried_inv);
        assert_eq!(
            adapted_list[ev_idx].trigger_kind(),
            Some(EventTriggerKind::ActionLook)
        );

        // 3. Explicit menu item use (`used_item_id == Some(key_id)`, `action_pressed == false`) MUST also fire (`UseItemLook`).
        let menu_use = EventEvalInput {
            action_pressed: false,
            used_item_id: Some(key_id),
            ..action_no_item
        };
        assert!(RetailTriggerKind::UseOrActionCarryingItem.matches_input(
            key_id,
            &menu_use,
            &carried_inv
        ));
        let norm_menu =
            normalize_retail_event_entry_for_input(raw_at_origin, &menu_use, &carried_inv);
        assert_eq!(
            norm_menu.trigger_kind(),
            Some(EventTriggerKind::UseItemLook)
        );
        assert!(
            evaluate_event_entry(0, &norm_menu, &ps2_origin_point, &flags, &menu_use, &mut ts)
                > 0.0
        );
    }
}
