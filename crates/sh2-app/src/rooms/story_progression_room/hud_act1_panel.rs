//! Right-side interactive controls & Act 1 Stage 1–27 summary card for Tab 1 (`[F1]`, `SH2-113`).

use super::hud_common::{draw_card, txt_right, CYAN, GREEN, MUTED, WHITE, YELLOW};
use super::Act1VisualPuzzle;
use crate::state::AppState;
use sh2_core::events::stages_act1::{
    act1_eval_red_pyramid_walk_speed_q12, act1_hospital_wing_keypad_goal,
    act1_is_old_man_coin_prop_visible, act1_should_james_look_at_prop, ACT1_ALL_STAGES,
    ACT1_FLAG_FIRE_ESCAPE_UNLOCKED, ACT1_FLAG_WING_KEYPAD_UNLOCKED, ACT1_PROP_KIND_TEDDY_BEAR,
};

pub(super) fn render_act1_right_panel(state: &mut AppState, active_pz: Act1VisualPuzzle) {
    let rx = 612usize;
    let rw = 644usize;
    let riddle = state.sim_core.scenario.riddle_difficulty;
    let fixes = state.story_progression_room.act1_fixes;

    // Top Card: Interactive Controls & State for the Active Act 1 Puzzle (y=78..354)
    draw_card(
        state,
        rx,
        78,
        rw,
        276,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    txt_right(
        state,
        rx + 12,
        88,
        YELLOW,
        &format!("ACT 1 PUZZLE CONTROLS & STATE (Riddle={riddle:?} | [1-4] Switch Puzzle)"),
    );
    txt_right(
        state,
        rx + 12,
        108,
        CYAN,
        &format!(
            "Active Puzzle: {:?} | Asset Source [T]: {}",
            active_pz,
            if state.story_progression_room.prefer_sh2ee_cutaways {
                "SH2:EE HD (sh2e/pic)"
            } else {
                "Original PC (data/pic)"
            }
        ),
    );

    let coin_vis = act1_is_old_man_coin_prop_visible(&state.sim_core.scenario.flags, fixes);
    let teddy_look = act1_should_james_look_at_prop(
        ACT1_PROP_KIND_TEDDY_BEAR,
        &state.sim_core.scenario.flags,
        fixes,
    );
    let wing_goal = act1_hospital_wing_keypad_goal(riddle);
    let wing_digits = state.story_progression_room.act1_wing_keypad.digits;
    let wing_unl = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_WING_KEYPAD_UNLOCKED);
    let ph_zone = state.story_progression_room.act1_ph_zone;
    let ph_hp = state.story_progression_room.act1_ph_hp_ratio;
    let ph_q12 = act1_eval_red_pyramid_walk_speed_q12(ph_hp * 500.0, 500.0, 1.0, ph_zone);

    txt_right(
        state,
        rx + 12,
        134,
        WHITE,
        "[1] W2F Rm 203 Safe : [Left/Right] Turn Dial | [Space] Step Next Tumbler",
    );
    txt_right(
        state,
        rx + 12,
        154,
        WHITE,
        "[2] Rm 105 Coin Desk: [Left/Right] Slot 1..5 | [Z/X/C] Snake/OldMan/Prisoner",
    );
    txt_right(
        state,
        rx + 12,
        174,
        WHITE,
        "[3] S14 Louise Box  : [Up/Down] Wheel 1..4 | [Left/Right] Digit | [B/P/E]",
    );
    txt_right(
        state,
        rx + 12,
        194,
        WHITE,
        "[4] 3F Quiz Chest   : [Up/Down] Q1..Q3 | [Left/Right] Ans 1..3 | [Enter]",
    );

    txt_right(
        state,
        rx + 12,
        222,
        YELLOW,
        "ADDITIONAL ACT 1 STAGE RIDDLES & SH2:EE VANILLA FIX TOGGLES ([V] / [K]):",
    );
    txt_right(
        state,
        rx + 12,
        242,
        if wing_unl { GREEN } else { WHITE },
        &format!(
            "3F Patient Wing Keypad [K]: Entered={wing_digits:?} vs Goal={wing_goal:?} (Open={wing_unl})"
        ),
    );
    txt_right(
        state,
        rx + 12,
        262,
        CYAN,
        &format!(
            "Trash Chute OldManCoin Visible={coin_vis} (OldManCoinFix={}) | FireEsc={}",
            fixes.old_man_coin_fix,
            state
                .sim_core
                .scenario
                .flags
                .get(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED)
        ),
    );
    txt_right(
        state,
        rx + 12,
        282,
        WHITE,
        &format!(
            "2F Locker TeddyBear HeadLook={teddy_look} (TeddyBearLookFix={})",
            fixes.teddy_bear_look_fix
        ),
    );
    txt_right(
        state,
        rx + 12,
        302,
        GREEN,
        &format!(
            "Basement Passage Pyramid Head: Zone {ph_zone}, HP={:.0}% -> WalkSpeed Q12={ph_q12}",
            ph_hp * 100.0
        ),
    );
    txt_right(
        state,
        rx + 12,
        324,
        MUTED,
        "[V] Toggle Vanilla PS2 vs SH2:EE Fixes | [K] Enter 3F Wing Code & Chute Juice",
    );

    // Bottom Card: Clean Human-Readable Act 1 Stage Summary Card (y=366..656)
    draw_card(
        state,
        rx,
        366,
        rw,
        290,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    let idx = state.story_progression_room.act1_stage_idx % ACT1_ALL_STAGES.len();
    let stg = &ACT1_ALL_STAGES[idx];
    let ev_cnt = stg.event_list.len().saturating_sub(1);
    let prog_cnt = stg.ev_prog_addrs.len();

    txt_right(
        state,
        rx + 12,
        376,
        YELLOW,
        &format!(
            "ACT 1 STAGE INSPECTOR [PgUp/PgDn]: Stage {:02}/27 - {}",
            stg.stage_id as u8,
            act1_stage_human_title(stg.stage_id as u8)
        ),
    );
    txt_right(
        state,
        rx + 12,
        398,
        GREEN,
        &format!(
            "Triggers: {ev_cnt} Events ({prog_cnt} Scripts) | Items: {} | Enemies: {} | Props: {}",
            stg.item_list.len(),
            stg.enemy_list.len(),
            stg.model_list.len()
        ),
    );
    txt_right(
        state,
        rx + 12,
        418,
        CYAN,
        &format!(
            "Active Fixes [V]: CoinAlign={} | OldManCoin={} | TeddyLook={}",
            fixes.coin_puzzle_alignment_fix, fixes.old_man_coin_fix, fixes.teddy_bear_look_fix
        ),
    );

    let start = idx
        .saturating_sub(4)
        .min(ACT1_ALL_STAGES.len().saturating_sub(10));
    for row in 0..10usize {
        let s_i = start + row;
        if s_i >= ACT1_ALL_STAGES.len() {
            break;
        }
        let d = &ACT1_ALL_STAGES[s_i];
        let sel = s_i == idx;
        let sid = d.stage_id as u8;
        txt_right(
            state,
            rx + 16,
            444 + row * 20,
            if sel { GREEN } else { MUTED },
            &format!(
                "{} Stage {:02}: {:<28} | Events:{:02} Items:{:02} Enemies:{:02}",
                if sel { ">>" } else { "  " },
                sid,
                act1_stage_human_title(sid),
                d.event_list.len().saturating_sub(1),
                d.item_list.len(),
                d.enemy_list.len()
            ),
        );
    }
}

fn quiz_host_subtitle_text(msg_id: Option<u8>) -> (&'static str, &'static str) {
    match msg_id.unwrap_or(32) {
        32..=38 => (
            "\"Welcome to Trick or Treat! Answer all 3 questions about Silent Hill\"",
            "\"to claim your fabulous prize in the 3rd floor store room!\"",
        ),
        39..=50 => (
            "\"Q1: Lakeside Amusement Park! What is part of the park?\"",
            "\"1) Haunted House   2) Rollercoaster   3) Merry-Go-Round [Answer: 3]\"",
        ),
        51..=61 => (
            "\"Q2: Who murdered Billy & Miriam Locane in cold blood?\"",
            "\"1) Walter Sullivan [Answer: 1]   2) Scott Fairbanks   3) Eric Gein\"",
        ),
        _ => (
            "\"Q3: Which street connects Wiltse Road toward Toluca Lake?\"",
            "\"1) Bachman Rd   2) Munson St   3) Nathan Avenue [Answer: 3]\"",
        ),
    }
}

pub(super) fn draw_quiz_chest_bottom_card(
    state: &mut AppState,
    vp_x: usize,
    vp_y: usize,
    vp_w: usize,
    vp_h: usize,
) {
    use super::hud_common::{txt_left, txt_left_lines, ORANGE, RED};
    use sh2_core::events::stages_act1::{
        act1_step_elevator_radio_quiz, ACT1_FLAG_QUIZ_CHEST_SOLVED, ACT1_FLAG_QUIZ_CHEST_WRONG,
    };

    let sel_row = state.story_progression_room.act1_quiz_row;
    let q1 = u8::from(state.sim_core.scenario.flags.get(234))
        | (u8::from(state.sim_core.scenario.flags.get(235)) << 1);
    let q2 = u8::from(state.sim_core.scenario.flags.get(236))
        | (u8::from(state.sim_core.scenario.flags.get(237)) << 1);
    let q3 = u8::from(state.sim_core.scenario.flags.get(238))
        | (u8::from(state.sim_core.scenario.flags.get(239)) << 1);
    let solved = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_QUIZ_CHEST_SOLVED);
    let wrong = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_QUIZ_CHEST_WRONG);

    if state.story_progression_room.act1_poison_flash_timer > 0.0 {
        let a =
            (state.story_progression_room.act1_poison_flash_timer * 140.0).clamp(30.0, 160.0) as u8;
        state
            .menu_rasterizer
            .fill_rect(vp_x, vp_y, vp_w, vp_h, [40, 190, 55, a]);
    }

    let mut fl_tmp = state.sim_core.scenario.flags.clone();
    let ticks = state.story_progression_room.act1_radio_quiz_ticks;
    let (sub_id, _) = act1_step_elevator_radio_quiz(ticks, false, &mut fl_tmp);
    let host_line = quiz_host_subtitle_text(sub_id);
    let status_col = if solved {
        GREEN
    } else if wrong {
        RED
    } else {
        YELLOW
    };

    draw_card(
        state,
        vp_x + 12,
        vp_y + 10,
        440,
        28,
        [10, 14, 22, 225],
        CYAN,
    );
    txt_left(
        state,
        vp_x + 20,
        vp_y + 18,
        status_col,
        &format!(
            "QUIZ CHEST: Q1={q1} Q2={q2} Q3={q3} (Row #{}) | HP={:.0}",
            sel_row + 1,
            state.story_progression_room.act1_james_hp
        ),
    );
    txt_left(
        state,
        34,
        514,
        YELLOW,
        &format!(
            "TRICK-OR-TREAT RADIO HOST (Stream 0xEACB, Tick {ticks:.0}/5175, Msg {sub_id:?}):"
        ),
    );
    draw_card(state, 34, 532, 540, 42, [8, 12, 18, 255], CYAN);
    txt_left_lines(
        state,
        42,
        540,
        16,
        &[(GREEN, host_line.0), (WHITE, host_line.1)],
    );
    let res_txt = if solved {
        "  CHEST UNLOCKED (SE 0x4DB6): +5x Shotgun Shells (#7) & +2x Ampoules (#3)!"
    } else if wrong {
        "  POISON GAS TRAP (SE 0x4DB7): 60% Max HP damage dealt! Press [Enter] to retry."
    } else {
        "  Target Answers: Q1=3 (Merry-Go-Round), Q2=1 (Walter Sullivan), Q3=3 (Nathan Ave)"
    };
    txt_left_lines(
        state,
        34,
        582,
        20,
        &[
            (
                if solved {
                    GREEN
                } else if wrong {
                    RED
                } else {
                    ORANGE
                },
                res_txt,
            ),
            (
                WHITE,
                "  [Up/Down] Select Row Q1..Q3 | [Left/Right] Answer 1..3 | [Enter] Submit",
            ),
            (
                MUTED,
                "  [Space] Auto-Set Winning (3, 1, 3) | [P] Trigger Wrong Answer Poison Gas!",
            ),
        ],
    );
}

fn act1_stage_human_title(stage_id: u8) -> &'static str {
    match stage_id {
        1 => "System Dummy Stage",
        2 => "Trial Silent Hill Town",
        3 => "Observation Deck & Forest",
        4 => "East South Vale Streets",
        5 => "West South Vale Streets",
        6 => "Wood Side Apartments 1F",
        7 => "Wood Side Apartments 2F",
        8 => "Wood Side Apartments 3F",
        9 => "Wood Side Stairwell",
        10 => "Blue Creek Apartments 1F",
        11 => "Blue Creek Apartments 2F",
        12 => "Wood Side Courtyard & Pool",
        13 => "Apartment Trash Courtyard",
        14 => "Pete's Bowl-O-Rama",
        15 => "Heaven's Night Club",
        16 => "Hospital Escape Corridor",
        17 => "Night Town Streets",
        18 => "Brookhaven Hospital 1F",
        19 => "Brookhaven Hospital 2F",
        20 => "Brookhaven Hospital 3F",
        21 => "Brookhaven Hospital Roof",
        22 => "Otherworld Hospital 1F",
        23 => "Otherworld Hospital 2F",
        24 => "Otherworld Hospital 3F",
        25 => "Otherworld Hospital B1",
        26 => "Otherworld Hospital B1 Alt",
        27 => "Hospital Basement Passage",
        _ => "Act 1 Stage",
    }
}
