//! Tab 2 (`[F2]`): Act 2 & *Born from a Wish* Visual Puzzles (`SH2-114`).

use super::hud_act2_panel::{draw_labyrinth_and_gallows_card, render_act2_right_panel};
use super::hud_common::{
    blit_cutaway_viewport, draw_card, txt_left, txt_left_lines, CYAN, GREEN, MUTED, ORANGE, WHITE,
    YELLOW,
};
use super::Act2VisualPuzzle;
use crate::state::AppState;
use sh2_core::events::stages_act2::{
    gravestone_board_uv, gravestone_board_verts, selected_briefcase_word, GravestoneBoard,
    BRIEFCASE_TRUNK_DRUMS, FLAG_BRIEFCASE_UNLOCKED, FLAG_ELEVATOR_WEIGHT_OK, FLAG_MUSIC_BOX_SOLVED,
    FLAG_SCAFFOLD_ALL_SET, FLAG_WISH_GRAVESTONE_SOLVED, GRAVESTONE_BOARD_VERT_EXTENTS,
};
use sh2_core::events::PZ_DRUM;
use sh2_core::{original_sheet_dimensions, riddle_level_index, OverlaySpriteQuad};

pub(crate) fn render_act2_tab(state: &mut AppState) {
    let active_pz = state.story_progression_room.act2_puzzle;
    let pref = state.story_progression_room.prefer_sh2ee_cutaways;

    let puzzles = [
        (Act2VisualPuzzle::WishGravestone, "[1] Wish Gravestone"),
        (Act2VisualPuzzle::HotelBriefcase, "[2] 204 Briefcase"),
        (Act2VisualPuzzle::HotelEggsAndVcr, "[3] Egg Doors & VCR"),
        (Act2VisualPuzzle::LabyrinthAndGallows, "[4] Cube & Gallows"),
    ];
    for (i, (pz, label)) in puzzles.iter().enumerate() {
        let bx = 24 + i * 141;
        let sel = active_pz == *pz;
        let bg = if sel {
            [34, 62, 88, 255]
        } else {
            [16, 20, 28, 255]
        };
        let fg = if sel { YELLOW } else { [60, 70, 90, 255] };
        draw_card(state, bx, 58, 137, 18, bg, fg);
        txt_left(state, bx + 4, 63, if sel { YELLOW } else { MUTED }, label);
    }

    let (base_pic, quads) = build_act2_cutaway_quads(state, active_pz);
    let (vp_x, vp_y, vp_w, vp_h) = (24usize, 78usize, 560usize, 420usize);
    blit_cutaway_viewport(state, vp_x, vp_y, vp_w, vp_h, base_pic, pref, &quads);

    draw_act2_viewport_hud_overlays(state, active_pz, vp_x, vp_y);
    render_act2_right_panel(state, active_pz);
}

fn build_act2_cutaway_quads(
    state: &mut AppState,
    active_pz: Act2VisualPuzzle,
) -> (&'static str, Vec<OverlaySpriteQuad>) {
    match active_pz {
        Act2VisualPuzzle::WishGravestone => {
            let fix = state
                .story_progression_room
                .act2_toggles
                .gravestone_boards_alignment_fix;
            let (sw, sh) = original_sheet_dimensions("add/teien2");
            let (sw, sh) = (sw as f32, sh as f32);
            let mut quads = Vec::new();
            let [x0, y0, x1, y1] = GRAVESTONE_BOARD_VERT_EXTENTS.map(|v| (v as f32) / 16.0);

            if !state
                .sim_core
                .scenario
                .flags
                .get(FLAG_WISH_GRAVESTONE_SOLVED)
            {
                let (u0, v0, u1, v1) = (160.0 / sw, 0.0 / sh, 319.0 / sw, 207.0 / sh);
                quads.push(OverlaySpriteQuad {
                    sheet_name: "add/teien2",
                    vertices: [
                        (x0, y0, u0, v0),
                        (x1, y0, u1, v0),
                        (x1, y1, u1, v1),
                        (x0, y1, u0, v1),
                    ],
                    alpha: 1.0,
                });
            }

            let boards = [
                (0usize, GravestoneBoard::White),
                (1, GravestoneBoard::Black),
                (2, GravestoneBoard::Red),
            ];
            for &(idx, board) in &boards {
                if state
                    .sim_core
                    .scenario
                    .inventory_mgr
                    .has_item(board.item_id())
                {
                    continue;
                }
                let rot = state.story_progression_room.act2_gravestone_rotations[idx];
                let uv = gravestone_board_uv(board, fix);
                let verts = gravestone_board_verts(rot, fix);
                let [tl, tr, bl, br] =
                    verts.map(|[vx, vy]| ((vx as f32) / 16.0, (vy as f32) / 16.0));
                let (u0, v0, u1, v1) = (
                    (uv.u1 / 16.0) / sw,
                    (uv.v1 / 16.0) / sh,
                    (uv.u2 / 16.0) / sw,
                    (uv.v2 / 16.0) / sh,
                );
                quads.push(OverlaySpriteQuad {
                    sheet_name: "add/teien2",
                    vertices: [
                        (tl.0, tl.1, u0, v0),
                        (tr.0, tr.1, u1, v0),
                        (br.0, br.1, u1, v1),
                        (bl.0, bl.1, u0, v1),
                    ],
                    alpha: 1.0,
                });
            }
            ("add_p_teien", quads)
        }
        Act2VisualPuzzle::HotelBriefcase => {
            let pic = if state.story_progression_room.act2_show_marker_still {
                "htl_p_marker"
            } else {
                "htl_p_trank"
            };
            state.sim_core.scenario.flags.set(405, true);
            eval_act2_overlay(state, pic, 0)
        }
        Act2VisualPuzzle::HotelEggsAndVcr => {
            let pic = if state.story_progression_room.act2_show_vcr_still {
                "htl_p_video"
            } else {
                "htl_p_egghollow"
            };
            eval_act2_overlay(state, pic, 1)
        }
        Act2VisualPuzzle::LabyrinthAndGallows => {
            let pic = if state.story_progression_room.act2_show_manacles_still {
                "dls_p_manacles_on"
            } else {
                "htl_p_hanging"
            };
            let s = &mut state.sim_core.scenario;
            let r_idx = riddle_level_index(s.riddle_difficulty);
            s.cutaway_overlay.set_picture(Some(pic), &s.flags);
            let alpha = if s.flags.get(FLAG_SCAFFOLD_ALL_SET) {
                1.0
            } else {
                0.0
            };
            s.cutaway_overlay.set_manacles_alpha(alpha);
            let q = s
                .cutaway_overlay
                .evaluate_frame(&s.flags, &s.puzzle_state, r_idx, 0)
                .to_vec();
            (pic, q)
        }
    }
}

fn eval_act2_overlay(
    state: &mut AppState,
    pic: &'static str,
    extra: i32,
) -> (&'static str, Vec<OverlaySpriteQuad>) {
    let s = &mut state.sim_core.scenario;
    let r_idx = riddle_level_index(s.riddle_difficulty);
    s.cutaway_overlay.set_picture(Some(pic), &s.flags);
    let q = s
        .cutaway_overlay
        .evaluate_frame(&s.flags, &s.puzzle_state, r_idx, extra)
        .to_vec();
    (pic, q)
}

fn draw_act2_viewport_hud_overlays(
    state: &mut AppState,
    active_pz: Act2VisualPuzzle,
    vp_x: usize,
    vp_y: usize,
) {
    draw_card(
        state,
        24,
        506,
        560,
        150,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    let toggles = state.story_progression_room.act2_toggles;

    match active_pz {
        Act2VisualPuzzle::WishGravestone => {
            let fix = toggles.gravestone_boards_alignment_fix;
            let solved = state
                .sim_core
                .scenario
                .flags
                .get(FLAG_WISH_GRAVESTONE_SOLVED);
            let rots = state.story_progression_room.act2_gravestone_rotations;
            draw_card(
                state,
                vp_x + 12,
                vp_y + 10,
                420,
                28,
                [10, 14, 22, 225],
                CYAN,
            );
            let align_lbl = if fix {
                "ALIGNMENT [V]: SH2:EE GravestoneBoardsFix (Rotated Quads Aligned)"
            } else {
                "ALIGNMENT [V]: Vanilla PC 0x008c4670 (Rotated Quads Shifted)"
            };
            txt_left(
                state,
                vp_x + 20,
                vp_y + 18,
                if fix { GREEN } else { ORANGE },
                align_lbl,
            );
            let l1 = format!(
                "  Placed Boards: White={:?} | Black={:?} | Red={:?}",
                rots[0], rots[1], rots[2]
            );
            let l2 = format!(
                "  Cavity Status: {} | Acacia Key (#82) in Inventory={}",
                if solved {
                    "SOLVED (Flag 704 Unlocked!)"
                } else {
                    "Unsolved (Match Black==White & Red==Left90)"
                },
                state.sim_core.scenario.inventory_mgr.has_item(82)
            );
            txt_left_lines(
                state,
                34,
                514,
                20,
                &[
                    (
                        YELLOW,
                        "BORN FROM A WISH: BALDWIN MANSION 3-BOARD GRAVESTONE:",
                    ),
                    (WHITE, &l1),
                    (if solved { GREEN } else { ORANGE }, &l2),
                    (
                        CYAN,
                        "  Press [V] to watch rotated board quads snap into alignment!",
                    ),
                    (
                        WHITE,
                        "  [Z] Place/Rotate White | [X] Black | [C] Red | [B] Pop Top (LIFO)",
                    ),
                    (
                        MUTED,
                        "  [Space] Auto-Solve 3 Boards & Claim Acacia Key (#82)",
                    ),
                ],
            );
        }
        Act2VisualPuzzle::HotelBriefcase => {
            let sel_d = state.story_progression_room.act2_drum_idx;
            let show_marker = state.story_progression_room.act2_show_marker_still;
            let word = selected_briefcase_word(&state.sim_core.scenario.flags);
            let raw: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()[PZ_DRUM..PZ_DRUM + 4]
                .try_into()
                .unwrap_or([0; 4]);
            let cur = [0, 1, 2, 3].map(|i| BRIEFCASE_TRUNK_DRUMS[i][(raw[i] & 7) as usize] as char);
            let open = state.sim_core.scenario.flags.get(FLAG_BRIEFCASE_UNLOCKED);

            draw_card(
                state,
                vp_x + 12,
                vp_y + 10,
                410,
                28,
                [10, 14, 22, 225],
                YELLOW,
            );
            txt_left(
                state,
                vp_x + 20,
                vp_y + 18,
                YELLOW,
                if show_marker {
                    "VIEWING: Rm 204 Photo & Thinner Code (htl_p_marker + maker)"
                } else {
                    "VIEWING: Rm 204 Briefcase 4-Drum Lock (htl_p_trank + dram)"
                },
            );
            let l0 = format!(
                "HOTEL RM 204 BRIEFCASE 4-DRUM LOCK (Selected Drum #{}/4):",
                sel_d + 1
            );
            let l1 = format!(
                "  Target Photo Word: \"{word}\"  |  Current Drums: \"{} {} {} {}\"",
                cur[0], cur[1], cur[2], cur[3]
            );
            let l2 = format!(
                "  Briefcase Lock(413)={} | Thinner Wiped Photo(411)={}",
                open,
                state.sim_core.scenario.flags.get(411)
            );
            txt_left_lines(
                state,
                34,
                514,
                20,
                &[
                    (YELLOW, &l0),
                    (GREEN, &l1),
                    (if open { GREEN } else { ORANGE }, &l2),
                    (
                        WHITE,
                        "  [Up/Down] Select Drum 1..4 | [Left/Right] Rotate Drum Letter",
                    ),
                    (
                        CYAN,
                        "  [B] Toggle Briefcase Drums vs Photo Marker Still (htl_p_marker)",
                    ),
                    (
                        MUTED,
                        "  [Space] Snap All 4 Drums to Target Word & Unlock Cinderella Box",
                    ),
                ],
            );
        }
        Act2VisualPuzzle::HotelEggsAndVcr => {
            let show_vcr = state.story_progression_room.act2_show_vcr_still;
            let fl = &state.sim_core.scenario.flags;
            let (rust_egg, scarlet_egg, vhs_in, mb_solved, shelf_has, weight_ok) = (
                fl.get(380),
                fl.get(381),
                fl.get(388),
                fl.get(FLAG_MUSIC_BOX_SOLVED),
                fl.get(446),
                fl.get(FLAG_ELEVATOR_WEIGHT_OK),
            );
            draw_card(
                state,
                vp_x + 12,
                vp_y + 10,
                410,
                28,
                [10, 14, 22, 225],
                CYAN,
            );
            txt_left(
                state,
                vp_x + 20,
                vp_y + 18,
                CYAN,
                if show_vcr {
                    "VIEWING: Hotel Rm 312 VCR Deck (htl_p_video + p_video02)"
                } else {
                    "VIEWING: Otherworld Hotel Egg Doors (htl_p_egghollow + egg)"
                },
            );
            let l1 = format!(
                "  Egg Doors [Z/X]: RustEgg={rust_egg} | ScarletEgg={scarlet_egg} | VCR Tape [C]={vhs_in}"
            );
            let l2 = format!(
                "  Lobby 3-Seat Music Box [M]: Solved(Flag 472)={mb_solved} (VolFix={})",
                toggles.music_box_volume_fix
            );
            let l3 = format!(
                "  2F Elevator Weight Cupboard [W]: ItemsOnShelf(446)={shelf_has} -> WeightOK={weight_ok}"
            );
            txt_left_lines(
                state,
                34,
                514,
                20,
                &[
                    (
                        YELLOW,
                        "HOTEL EGG DOORS, RM 312 VCR, MUSIC BOX & ELEVATOR CUPBOARD:",
                    ),
                    (WHITE, &l1),
                    (if mb_solved { GREEN } else { WHITE }, &l2),
                    (if weight_ok { GREEN } else { ORANGE }, &l3),
                    (
                        WHITE,
                        "  [Z] Toggle Rust Egg | [X] Scarlet Egg | [C] VCR Tape | [B] Swap Still",
                    ),
                    (
                        MUTED,
                        "  [M] Solve 3 Music Boxes | [W] Transfer All Inventory to/from Cupboard",
                    ),
                ],
            );
        }
        Act2VisualPuzzle::LabyrinthAndGallows => {
            draw_labyrinth_and_gallows_card(state, vp_x, vp_y);
        }
    }
}
