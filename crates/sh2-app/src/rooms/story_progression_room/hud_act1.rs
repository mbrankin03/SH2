//! Tab 1 (`[F1]`): Act 1 Visual Cutaway Puzzles & Stage Riddles (`SH2-113`).

use super::hud_act1_panel::{draw_quiz_chest_bottom_card, render_act1_right_panel};
use super::hud_common::{
    blit_cutaway_viewport, draw_card, draw_progress_bar, txt_left, txt_left_lines, CYAN, GREEN,
    MUTED, ORANGE, WHITE, YELLOW,
};
use super::Act1VisualPuzzle;
use crate::state::AppState;
use sh2_core::events::stages_act1::{
    act1_coin_puzzle_quad_layout, act1_read_safe_dial_pos, format_act1_carbon_paper_code,
    format_act1_wallet_safe_memo, Act1DeskCoinKind, ACT1_FLAG_LOUISE_KEY1_OPEN,
    ACT1_FLAG_LOUISE_KEY2_OPEN, ACT1_FLAG_LOUISE_KEYPAD_OPEN, ACT1_FLAG_LOUISE_WHEEL_OPEN,
    ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED, ACT1_FLAG_SAFE_UNLOCKED, ACT1_FLAG_WALL_BLOOD_CODE_READ,
};
use sh2_core::events::{PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL};
use sh2_core::{
    is_coin_desk_solved, original_sheet_dimensions, riddle_level_index, OverlaySpriteQuad,
};

pub(crate) fn render_act1_tab(state: &mut AppState) {
    let active_pz = state.story_progression_room.act1_puzzle;
    let pref = state.story_progression_room.prefer_sh2ee_cutaways;
    let fixes = state.story_progression_room.act1_fixes;

    let puzzles = [
        (Act1VisualPuzzle::SafeDial, "[1] W2F Safe Dial"),
        (Act1VisualPuzzle::CoinDesk, "[2] 105 Coin Desk"),
        (Act1VisualPuzzle::LouiseBox, "[3] S14 Louise Box"),
        (Act1VisualPuzzle::QuizChest, "[4] Quiz & Radio"),
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

    let (base_pic, quads) = build_act1_cutaway_quads(state, active_pz);
    let (vp_x, vp_y, vp_w, vp_h) = (24usize, 78usize, 560usize, 420usize);
    blit_cutaway_viewport(state, vp_x, vp_y, vp_w, vp_h, base_pic, pref, &quads);

    draw_act1_viewport_hud_overlays(state, active_pz, vp_x, vp_y, vp_w, vp_h, fixes);
    render_act1_right_panel(state, active_pz);
}

fn build_act1_cutaway_quads(
    state: &mut AppState,
    active_pz: Act1VisualPuzzle,
) -> (&'static str, Vec<OverlaySpriteQuad>) {
    match active_pz {
        Act1VisualPuzzle::SafeDial => eval_standard_overlay(state, "apt_p_safe_close"),
        Act1VisualPuzzle::CoinDesk => {
            let fixes = state.story_progression_room.act1_fixes;
            let (sw, sh) = original_sheet_dimensions("apt/p_desk_coin_coin");
            let (sw, sh) = (sw as f32, sh as f32);
            let mut quads = Vec::new();
            let coins = [
                (0usize, Act1DeskCoinKind::Snake),
                (1, Act1DeskCoinKind::OldMan),
                (2, Act1DeskCoinKind::Prisoner),
            ];
            for slot in 0..5usize {
                for &(c_idx, coin_kind) in &coins {
                    if !state.sim_core.scenario.flags.get(126 + 5 * c_idx + slot) {
                        continue;
                    }
                    if let Some(l) = act1_coin_puzzle_quad_layout(coin_kind, slot, fixes) {
                        let [x0, y0, x1, y1] = l.bounds.map(|b| (b as f32) / 16.0);
                        let (u0, v0, u1, v1) = (
                            (l.uv[0] / 16.0) / sw,
                            (l.uv[1] / 16.0) / sh,
                            (l.uv[2] / 16.0) / sw,
                            (l.uv[3] / 16.0) / sh,
                        );
                        quads.push(OverlaySpriteQuad {
                            sheet_name: "apt/p_desk_coin_coin",
                            vertices: [
                                (x0, y0, u0, v0),
                                (x1, y0, u1, v0),
                                (x1, y1, u1, v1),
                                (x0, y1, u0, v1),
                            ],
                            alpha: 1.0,
                        });
                    }
                }
            }
            ("apt_p_desk_coin", quads)
        }
        Act1VisualPuzzle::LouiseBox => {
            let pic = if state.story_progression_room.act1_show_wall_blood_still {
                "hsp_p_boxnumber"
            } else {
                "hsp_p_box"
            };
            eval_standard_overlay(state, pic)
        }
        Act1VisualPuzzle::QuizChest => eval_standard_overlay(state, "hsp_p_quiz"),
    }
}

fn eval_standard_overlay(
    state: &mut AppState,
    pic: &'static str,
) -> (&'static str, Vec<OverlaySpriteQuad>) {
    let s = &mut state.sim_core.scenario;
    let r_idx = riddle_level_index(s.riddle_difficulty);
    s.cutaway_overlay.set_picture(Some(pic), &s.flags);
    let q = s
        .cutaway_overlay
        .evaluate_frame(&s.flags, &s.puzzle_state, r_idx, 0)
        .to_vec();
    (pic, q)
}

fn draw_act1_viewport_hud_overlays(
    state: &mut AppState,
    active_pz: Act1VisualPuzzle,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    vp_h: usize,
    fixes: sh2_core::events::stages_act1::Act1VanillaFixesConfig,
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

    match active_pz {
        Act1VisualPuzzle::SafeDial => {
            let riddle = state.sim_core.scenario.riddle_difficulty;
            let memo = format_act1_wallet_safe_memo(&state.sim_core.scenario.puzzle_state, riddle);
            let dial = act1_read_safe_dial_pos(&state.sim_core.scenario.flags);
            let open = state.sim_core.scenario.flags.get(ACT1_FLAG_SAFE_UNLOCKED);
            let raw: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()
                [PZ_SAFE_CODE..PZ_SAFE_CODE + 4]
                .try_into()
                .unwrap_or([0; 4]);
            let prog = [0, 1, 2, 3]
                .map(|i| state.sim_core.scenario.puzzle_state.raw()[PZ_SAFE_PROG + i] as i8);
            draw_card(
                state,
                vp_x + 12,
                vp_y + 10,
                260,
                44,
                [10, 14, 22, 220],
                YELLOW,
            );
            let notch_str = format!("DIAL NOTCH: {:02} / 20", dial + 1);
            let hint_str = if open {
                "SAFE UNLOCKED! (+40 Ammo)"
            } else {
                "Turn [Left/Right] or [Space]"
            };
            txt_left_lines(
                state,
                vp_x + 20,
                vp_y + 16,
                18,
                &[
                    (YELLOW, &notch_str),
                    (if open { GREEN } else { CYAN }, hint_str),
                ],
            );
            let done = prog.iter().filter(|&&v| v >= 0).count();
            draw_progress_bar(state, 34, 576, 536, 16, done as f32 / 4.0, GREEN, CYAN);
            let l1 = format!("  Clue ({riddle:?}): \"{memo}\"");
            let l2 = format!(
                "  Target Notches: {:02} >> {:02} << {:02} >> {:02}",
                raw[0] + 1,
                raw[1] + 1,
                raw[2] + 1,
                raw[3] + 1
            );
            let l3 = format!("  Progress Stage: {done}/4 tumblers matched ({prog:?})");
            txt_left_lines(
                state,
                34,
                514,
                20,
                &[
                    (YELLOW, "WALLET MEMO CLUE CARD (Rm 203 Toilet):"),
                    (GREEN, &l1),
                    (WHITE, &l2),
                ],
            );
            txt_left_lines(
                state,
                34,
                600,
                20,
                &[
                    (CYAN, &l3),
                    (
                        MUTED,
                        "  [Left/Right] Rotate Dial | [Space] Step Next Target | [R] Re-seed",
                    ),
                ],
            );
        }
        Act1VisualPuzzle::CoinDesk => {
            let sel_slot = state.story_progression_room.act1_coin_slot;
            let solved = is_coin_desk_solved(
                &state.sim_core.scenario.flags,
                state.sim_core.scenario.riddle_difficulty,
            );
            state.menu_rasterizer.draw_rect_outline(
                vp_x + 76 + sel_slot * 84,
                vp_y + 150,
                74,
                96,
                2,
                YELLOW,
            );
            draw_card(
                state,
                vp_x + 12,
                vp_y + 10,
                390,
                28,
                [10, 14, 22, 225],
                CYAN,
            );
            let align_lbl = if fixes.coin_puzzle_alignment_fix {
                "ALIGNMENT [V]: SH2:EE CoinPuzzleFix (Centered Slots)"
            } else {
                "ALIGNMENT [V]: Vanilla PS2 0x008e0538 (Per-Coin Offset)"
            };
            txt_left(
                state,
                vp_x + 20,
                vp_y + 18,
                if fixes.coin_puzzle_alignment_fix {
                    GREEN
                } else {
                    ORANGE
                },
                align_lbl,
            );
            let s_names: [&str; 5] = std::array::from_fn(|h| {
                if state.sim_core.scenario.flags.get(126 + h) {
                    "Snake"
                } else if state.sim_core.scenario.flags.get(131 + h) {
                    "OldMan"
                } else if state.sim_core.scenario.flags.get(136 + h) {
                    "Prisoner"
                } else {
                    "Empty"
                }
            });
            let l0 = format!(
                "RM 105 FIVE-HOLE COIN DESK (Selected Slot #{}/5):",
                sel_slot + 1
            );
            let l1 = format!(
                "  Slots: [1:{}] [2:{}] [3:{}] [4:{}] [5:{}]",
                s_names[0], s_names[1], s_names[2], s_names[3], s_names[4]
            );
            let l2 = format!(
                "  Desk Status: {} | Lyne House Key (#31)={}",
                if solved {
                    "SOLVED (Flag 142)"
                } else {
                    "Unsolved"
                },
                state.sim_core.scenario.inventory_mgr.has_item(31)
            );
            txt_left_lines(
                state,
                34,
                514,
                20,
                &[
                    (YELLOW, &l0),
                    (WHITE, &l1),
                    (if solved { GREEN } else { ORANGE }, &l2),
                    (
                        CYAN,
                        "  Press [V] to watch coin quads visually shift on the desk!",
                    ),
                    (
                        WHITE,
                        "  [Left/Right] Select Slot 1..5 | [Z] Snake | [X] OldMan | [C] Prisoner",
                    ),
                    (
                        MUTED,
                        "  [Space] Auto-Solve for Active Riddle Level | [BackSpace/D] Clear Slot",
                    ),
                ],
            );
        }
        Act1VisualPuzzle::LouiseBox => {
            let sel_w = state.story_progression_room.act1_louise_wheel_idx;
            let show_wall = state.story_progression_room.act1_show_wall_blood_still;
            let carbon = format_act1_carbon_paper_code(&state.sim_core.scenario.puzzle_state);
            let w: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()[PZ_WHEEL..PZ_WHEEL + 4]
                .try_into()
                .unwrap_or([0; 4]);
            let g: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()
                [PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
                .try_into()
                .unwrap_or([0; 4]);
            let fl = &state.sim_core.scenario.flags;
            let (f192, f203, f197, f198, f193, f195) = (
                fl.get(ACT1_FLAG_WALL_BLOOD_CODE_READ),
                fl.get(ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED),
                fl.get(ACT1_FLAG_LOUISE_WHEEL_OPEN),
                fl.get(ACT1_FLAG_LOUISE_KEYPAD_OPEN),
                fl.get(ACT1_FLAG_LOUISE_KEY1_OPEN),
                fl.get(ACT1_FLAG_LOUISE_KEY2_OPEN),
            );
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
                if show_wall {
                    "VIEWING: 3F Special Treatment Wall Blood Code (hsp_p_boxnumber)"
                } else {
                    "VIEWING: 2F Rm S14 Louise Locked Box (hsp_p_box + pboxkey01)"
                },
            );
            let l0 = format!(
                "BROOKHAVEN S14 LOUISE BOX (Selected Wheel #{}/4):",
                sel_w + 1
            );
            let l1 = format!(
                "  Rotary Wheels: [{}, {}, {}, {}] vs Wall Blood Goal: [{}, {}, {}, {}]",
                w[0] + 1,
                w[1] + 1,
                w[2] + 1,
                w[3] + 1,
                g[0] + 1,
                g[1] + 1,
                g[2] + 1,
                g[3] + 1
            );
            let l2 = format!(
                "  Carbon Paper Keypad: \"{carbon}\" (Lock={f198}) | Keys(193/195)={f193}/{f195}"
            );
            let l3 = format!(
                "  WallBlood(192)={f192} | RoofPHPush(203)={f203} (HP={:.0}) | WheelLock={f197}",
                state.story_progression_room.act1_james_hp
            );
            txt_left_lines(
                state,
                34,
                514,
                20,
                &[
                    (YELLOW, &l0),
                    (WHITE, &l1),
                    (CYAN, &l2),
                    (if f197 { GREEN } else { ORANGE }, &l3),
                    (WHITE, "  [Up/Down] Wheel 1..4 | [Left/Right] Turn Digit | [B] Wall Blood Still"),
                    (MUTED, "  [P] Roof PH Push (Flag 203) | [E] Exit Anti-Bruteforce Scramble | [Space] Unlock All"),
                ],
            );
        }
        Act1VisualPuzzle::QuizChest => {
            draw_quiz_chest_bottom_card(state, vp_x, vp_y, vp_w, vp_h);
        }
    }
}
