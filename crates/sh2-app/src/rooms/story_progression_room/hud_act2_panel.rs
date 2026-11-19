//! Right-side Interactive Key-Door Unlock Simulator (`trg == 10`) & Act 2 Stage 25–58 summary card
//! for Tab 2 (`[F2]`, `SH2-114`).

use super::hud_common::{draw_card, txt_right, CYAN, GREEN, MUTED, RED, WHITE, YELLOW};
use super::{Act2VisualPuzzle, ACT2_TRG10_KEY_DOORS};
use crate::state::AppState;
use sh2_core::events::stages_act2::{
    is_hotel_2f_teleport_corridor_room, stage_table_act2, ALL_ACT2_STAGE_IDS,
};

pub(super) fn render_act2_right_panel(state: &mut AppState, _active_pz: Act2VisualPuzzle) {
    let rx = 612usize;
    let rw = 644usize;

    // Top Card: Interactive Key-Door Unlock Simulator (`trg == 10`) (y=78..354)
    draw_card(
        state,
        rx,
        78,
        rw,
        276,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    let door_idx = state.story_progression_room.act2_trg10_door_idx % ACT2_TRG10_KEY_DOORS.len();
    let carrying = state.story_progression_room.act2_carrying_key;
    let unlocked = state.story_progression_room.act2_trg10_unlocked;
    let (stg_id, _ev_idx, key_id, door_label) = ACT2_TRG10_KEY_DOORS[door_idx];

    txt_right(
        state,
        rx + 12,
        88,
        YELLOW,
        "INTERACTIVE KEY-DOOR UNLOCK SIMULATOR (Retail trg == 10)",
    );
    txt_right(
        state,
        rx + 12,
        108,
        CYAN,
        &format!(
            "Selected Door [{}/9] ([ [ / ] ] to Cycle): {door_label}",
            door_idx + 1
        ),
    );
    txt_right(
        state,
        rx + 12,
        128,
        if carrying { GREEN } else { RED },
        &format!(
            "Inventory State [K]: {} (Stage {stg_id})",
            if carrying {
                format!("CARRYING Key #{key_id} in Pocket")
            } else {
                format!("NOT CARRYING Key #{key_id} (Dropped)")
            }
        ),
    );

    let (banner_bg, banner_border, banner_col) = match unlocked {
        Some(true) => ([18, 58, 34, 255], GREEN, GREEN),
        Some(false) => ([68, 20, 24, 255], RED, RED),
        None => ([22, 28, 40, 255], CYAN, WHITE),
    };
    draw_card(state, rx + 12, 148, rw - 24, 42, banner_bg, banner_border);
    let last_res = state.story_progression_room.act2_trg10_last_result.clone();
    txt_right(state, rx + 20, 156, banner_col, &last_res);
    txt_right(
        state,
        rx + 20,
        172,
        WHITE,
        "[A] Walk Up & Press Action (Auto-Use)  |  [U] Use Key from Inventory Menu",
    );

    for (i, &(_, _, kid, lbl)) in ACT2_TRG10_KEY_DOORS.iter().enumerate() {
        let col_x = if i < 5 { rx + 14 } else { rx + 332 };
        let row_y = 200 + (i % 5) * 18;
        let sel = i == door_idx;
        let short_lbl = lbl.split(" (").next().unwrap_or(lbl);
        txt_right(
            state,
            col_x,
            row_y,
            if sel { GREEN } else { MUTED },
            &format!("{}{} (#{kid})", if sel { ">>" } else { "  " }, short_lbl),
        );
    }

    let room_byte = 0x29 + (state.story_progression_room.act2_corridor_room_idx as u8);
    let teleports = is_hotel_2f_teleport_corridor_room(room_byte);
    txt_right(
        state,
        rx + 12,
        298,
        if teleports { GREEN } else { WHITE },
        &format!(
            "Otherworld Hotel 2F Hallway [N]: Room 0x{room_byte:02X} -> {}",
            if teleports {
                "TELEPORTS (Non-Euclidean Warp!)"
            } else {
                "Normal Hallway Door"
            }
        ),
    );
    txt_right(
        state,
        rx + 12,
        318,
        MUTED,
        "([ [ / ] ] Pick Door | [K] Pick Up/Drop Key | [A] Action | [U] Menu Use)",
    );

    // Bottom Card: Clean Human-Readable Act 2 & Wish Stage Summary Card (y=366..656)
    draw_card(
        state,
        rx,
        366,
        rw,
        290,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    let idx = state.story_progression_room.act2_stage_idx % ALL_ACT2_STAGE_IDS.len();
    let stage_id = ALL_ACT2_STAGE_IDS[idx];
    if let Some(stg) = stage_table_act2(stage_id) {
        let toggles = state.story_progression_room.act2_toggles;
        txt_right(
            state,
            rx + 12,
            376,
            YELLOW,
            &format!(
                "ACT 2 & WISH STAGE INSPECTOR [PgUp/PgDn]: Stage {:02} - {}",
                stg.stage_id,
                act2_stage_human_title(stg.stage_id)
            ),
        );
        txt_right(
            state,
            rx + 12,
            398,
            GREEN,
            &format!(
                "Triggers: {} Events | Items: {} | Enemies: {} | Scripts: {}",
                stg.active_event_count(),
                stg.retail_items.len(),
                stg.retail_enemies.len(),
                stg.ev_prog_vas.len()
            ),
        );
        txt_right(
            state,
            rx + 12,
            418,
            CYAN,
            &format!(
                "Active Fixes [V]: GravestoneAlign={} | MusicBoxVol={} | Prisoner60Hz={}",
                toggles.gravestone_boards_alignment_fix,
                toggles.music_box_volume_fix,
                toggles.prisoner_timer_60fps_fix
            ),
        );

        let start = idx
            .saturating_sub(4)
            .min(ALL_ACT2_STAGE_IDS.len().saturating_sub(10));
        for row in 0..10usize {
            let s_i = start + row;
            if s_i >= ALL_ACT2_STAGE_IDS.len() {
                break;
            }
            let sid = ALL_ACT2_STAGE_IDS[s_i];
            if let Some(d) = stage_table_act2(sid) {
                let sel = s_i == idx;
                txt_right(
                    state,
                    rx + 16,
                    444 + row * 20,
                    if sel { GREEN } else { MUTED },
                    &format!(
                        "{} Stage {:02}: {:<28} | Events:{:02} Items:{:02} Enemies:{:02}",
                        if sel { ">>" } else { "  " },
                        sid,
                        act2_stage_human_title(sid),
                        d.active_event_count(),
                        d.retail_items.len(),
                        d.retail_enemies.len()
                    ),
                );
            }
        }
    }
}

pub(super) fn draw_labyrinth_and_gallows_card(state: &mut AppState, vp_x: usize, vp_y: usize) {
    use super::hud_common::{txt_left, txt_left_lines, ORANGE};
    use sh2_core::events::stages_act2::{
        decode_hangman_digits, effective_prisoner_step_threshold, hangman_innocent_slot,
        sync_labyrinth_face_cube, FLAG_SCAFFOLD_ALL_SET,
    };

    let toggles = state.story_progression_room.act2_toggles;
    let mut fl_copy = state.sim_core.scenario.flags.clone();
    let cube = sync_labyrinth_face_cube(&mut fl_copy);
    let digits = decode_hangman_digits(&state.sim_core.scenario.puzzle_state);
    let innocent = hangman_innocent_slot(&state.sim_core.scenario.puzzle_state);
    let sel_noose = state.story_progression_room.act2_hangman_selected_slot;
    let solved_hangman = (0..6).any(|i| state.sim_core.scenario.flags.get(343 + i));
    let fl_304 = state.sim_core.scenario.flags.get(FLAG_SCAFFOLD_ALL_SET);

    draw_card(
        state,
        vp_x + 12,
        vp_y + 10,
        536,
        92,
        [10, 14, 24, 232],
        YELLOW,
    );
    let face_name = match cube.plane {
        0 => "FRONT (Green Eyes)",
        1 => "UP (Top Crown)",
        2 => "BACK (Upside-Down)",
        _ => "DOWN (Chin)",
    };
    let c1 = format!(
        "ROTATING FACE CUBE: Face={face_name} | Roll={} | Conn={}",
        cube.roll, cube.connectivity_level
    );
    let c2 = format!(
        "DOORWAY STATUS: {} (Maria Cutscene Path={})",
        if cube.passage_open {
            "OPEN -> Passage to Maria Cell Unlocked (Flag 325)"
        } else {
            "SEALED -> Rotate Cube with [Up/Down/Left/Right]!"
        },
        if cube.maria_path_open {
            "OPEN"
        } else {
            "CLOSED"
        }
    );
    txt_left_lines(
        state,
        vp_x + 20,
        vp_y + 18,
        20,
        &[
            (YELLOW, &c1),
            (if cube.passage_open { GREEN } else { RED }, &c2),
        ],
    );
    txt_left(state, vp_x + 20, vp_y + 60, CYAN, "6 NOOSES:");
    for (n, &dig) in digits.iter().enumerate() {
        let nx = vp_x + 100 + n * 70;
        let is_inn = innocent == Some(n);
        let is_sel = n == sel_noose;
        let bg = if is_sel {
            [36, 62, 88, 255]
        } else {
            [18, 22, 32, 255]
        };
        let fg = if is_inn {
            GREEN
        } else if is_sel {
            YELLOW
        } else {
            [70, 80, 100, 255]
        };
        draw_card(state, nx, vp_y + 56, 64, 38, bg, fg);
        let n_hdr = format!("#{} d={dig}", n + 1);
        txt_left_lines(
            state,
            nx + 6,
            vp_y + 62,
            16,
            &[
                (if is_inn { GREEN } else { WHITE }, &n_hdr),
                (
                    if is_inn { GREEN } else { MUTED },
                    if is_inn { "INNOCENT" } else { "Guilty" },
                ),
            ],
        );
    }

    let thresh = effective_prisoner_step_threshold(45, 60, toggles.prisoner_timer_60fps_fix);
    let l1 = format!(
        "  6 Hanged Men: Innocent Slot={:?} | Selected=#{} | Solved={solved_hangman}",
        innocent.map(|s| s + 1),
        sel_noose + 1
    );
    let l2 =
        format!("  Prison Gallows 3 Tablets(Flag 304)={fl_304} | 60FPS Prisoner Timer={thresh}f");
    txt_left_lines(
        state,
        34,
        514,
        20,
        &[
            (
                YELLOW,
                "LABYRINTH ROTATING CUBE, 6 HANGED MEN & PRISON GALLOWS:",
            ),
            (if solved_hangman { GREEN } else { WHITE }, &l1),
            (if fl_304 { GREEN } else { ORANGE }, &l2),
            (
                WHITE,
                "  [Up/Down/Left/Right] Rotate Labyrinth Face Cube | [B] Swap Still",
            ),
            (
                CYAN,
                "  [Z/X] Select Noose 1..6 | [Enter] Pull Noose | [Space] Insert 3 Tablets",
            ),
            (
                MUTED,
                "  (Innocent Man is the slot where PZ_HANGMAN digit == 5)",
            ),
        ],
    );
}

fn act2_stage_human_title(stage_id: u16) -> &'static str {
    match stage_id {
        16 => "Hospital Escape Corridor",
        25 => "Otherworld Hospital B1",
        26 => "Otherworld Hospital B1 Alt",
        27 => "Hospital Basement Passage",
        28 => "Night South Vale Streets",
        29 => "Historical Society & Pit",
        30 => "Toluca Prison North",
        31 => "Toluca Prison South",
        32 => "Toluca Prison Basement",
        33 => "Labyrinth West",
        34 => "Labyrinth East",
        35 => "Labyrinth North & Maria Cell",
        36 => "Abstract Daddy & Graveyard",
        37 => "Eddie Boss Freezer",
        38 => "Toluca Lake Boat & Dock",
        39 => "Lakeview Hotel B1 & Bar",
        40 => "Lakeview Hotel 1F Lobby",
        41 => "Lakeview Hotel 2F Wings",
        42 => "Lakeview Hotel 3F & Rm 312",
        43 => "Otherworld Hotel B1",
        44 => "Otherworld Hotel 1F",
        45 => "Otherworld Hotel 2F",
        46 => "Otherworld Hotel 3F",
        47 => "Burnt Hotel & 9 Save Squares",
        48 => "Final Boss Rooftop Arena",
        49 => "Ending: Leave",
        50 => "Ending: Maria",
        51 => "Ending: In Water",
        52 => "Ending: Rebirth",
        53 => "Wish: Heaven's Night",
        54 => "Wish: Night Town Streets",
        55 => "Wish: Baldwin Mansion 1F",
        56 => "Wish: Baldwin Mansion 2F",
        57 => "Wish: Mansion Blue Creek",
        58 => "Wish: Mansion Graveyard",
        _ => "Act 2 Stage",
    }
}
