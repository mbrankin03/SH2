//! Retail `sh2pc.exe` byte-for-byte parity tests for `SH2-114` static stage tables (Stages `16`
//! & `25..=58`) and riddle data tables. Skips cleanly when the retail rip is not symlinked at the
//! workspace root (GitHub Actions CI).

use std::path::{Path, PathBuf};

use sh2_core::events::*;
use sh2_core::types::EventListEntry;

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
