//! Tab 1 (`[F1]`): Act 1 Stages 1–27 & Riddles (`SH2-113`) HUD overlay.

use crate::state::AppState;
use sh2_core::events::stages_act1::{
    act1_coin_puzzle_quad_layout, act1_eval_red_pyramid_walk_speed_q12,
    act1_hospital_wing_keypad_goal, act1_is_old_man_coin_prop_visible, act1_read_safe_dial_pos,
    act1_should_james_look_at_prop, act1_step_elevator_radio_quiz, decode_act1_enemy_spawn,
    decode_act1_ev_pos_anchor, format_act1_carbon_paper_code, format_act1_wallet_safe_memo,
    Act1DeskCoinKind, Act1VanillaFixesConfig, ACT1_ALL_STAGES, ACT1_FLAG_FIRE_ESCAPE_UNLOCKED,
    ACT1_FLAG_LOUISE_WHEEL_OPEN, ACT1_FLAG_OLD_MAN_COIN_SUB, ACT1_FLAG_OLD_MAN_COIN_TAKEN,
    ACT1_FLAG_QUIZ_CHEST_SOLVED, ACT1_FLAG_QUIZ_CHEST_WRONG, ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED,
    ACT1_FLAG_SAFE_UNLOCKED, ACT1_FLAG_WALL_BLOOD_CODE_READ, ACT1_FLAG_WING_KEYPAD_UNLOCKED,
    ACT1_PROP_KIND_TEDDY_BEAR,
};
use sh2_core::events::{PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL};

const YELLOW: [u8; 4] = [245, 210, 65, 255];
const CYAN: [u8; 4] = [105, 215, 235, 255];
const GREEN: [u8; 4] = [120, 230, 150, 255];
const WHITE: [u8; 4] = [235, 235, 235, 255];
const ORANGE: [u8; 4] = [255, 170, 80, 255];
const MUTED: [u8; 4] = [165, 170, 185, 255];

fn txt(state: &mut AppState, x: usize, y: usize, color: [u8; 4], s: &str) {
    state.menu_rasterizer.draw_text(s, x, y, 1, color, true);
}

pub(super) fn render_act1_tab(state: &mut AppState) {
    let idx = state.story_progression_room.act1_stage_idx % ACT1_ALL_STAGES.len();
    let stg = &ACT1_ALL_STAGES[idx];
    let h = &stg.header;

    let lx = 22usize;
    txt(
        state,
        lx,
        74,
        YELLOW,
        &format!(
            "--- ACT 1 STAGE TABLE [PgUp/PgDn]: Stage {:02}/27 ({:?}) ---",
            stg.stage_id as u8, stg.stage_id
        ),
    );
    txt(
        state,
        lx,
        92,
        CYAN,
        &format!(
            "Stage_Data VAs : PS2 SLUS_202.28=0x{:08X} | PC sh2pc.exe=0x{:08X}",
            stg.ps2_stage_data_addr, stg.pc_stage_data_addr
        ),
    );
    txt(
        state,
        lx,
        108,
        WHITE,
        &format!(
            "Header Ptrs    : ev_list=0x{:08X} | ev_pos=0x{:08X} | ev_prog=0x{:08X}",
            h.ev_list_ptr, h.ev_pos_ptr, h.ev_prog_ptr
        ),
    );
    txt(
        state,
        lx,
        124,
        WHITE,
        &format!(
            "Entity Ptrs    : gi_list=0x{:08X} | mdl_list=0x{:08X} | en_list=0x{:08X}",
            h.item_list_ptr, h.model_list_ptr, h.enemy_list_ptr
        ),
    );
    txt(
        state,
        lx,
        140,
        WHITE,
        &format!(
            "Callbacks      : stage_init=0x{:08X} | room_init=0x{:08X} | all_time=0x{:08X}",
            h.stage_init_fn, h.room_init_fn, h.all_time_fn
        ),
    );
    txt(state, lx, 156, MUTED, &format!("Audio/Clear    : sound_load=0x{:08X} | bgm_ctrl=0x{:08X} | chara_clr=0x{:08X} | glb_crd={}", h.sound_call_after_load_fn, h.bgm_control_fn, h.chara_data_clear_fn, h.glb_crd));
    txt(state, lx, 174, GREEN, &format!("Counts         : Event_List={} (+sentinel) | ev_pos={}B | ev_prog={} | Items={} | Models={} | Enemies={}", stg.event_list.len().saturating_sub(1), stg.ev_pos.len(), stg.ev_prog_addrs.len(), stg.item_list.len(), stg.model_list.len(), stg.enemy_list.len()));

    txt(
        state,
        lx,
        196,
        YELLOW,
        "Sample Event_List & Decoded ev_pos Anchors (decode_act1_ev_pos_anchor):",
    );
    for (i, ev) in stg.event_list.iter().take(4).enumerate() {
        if ev.is_sentinel() {
            break;
        }
        let anchor_str = ev
            .shape_kind()
            .and_then(|sh| decode_act1_ev_pos_anchor(stg.ev_pos, ev.ev_pos_offset(), sh))
            .map(|([x, y, z], p)| format!("pos=({x:.0},{y:.0},{z:.0}) p={p:.0}"))
            .unwrap_or_else(|| "no-shape".to_string());
        txt(
            state,
            lx,
            212 + i * 16,
            WHITE,
            &format!(
                "  ev[{i:02}]: flag=0x{:08X} cond=0x{:08X} rslt0=0x{:08X} | {anchor_str}",
                ev.flag, ev.cond, ev.rslt0
            ),
        );
    }

    txt(
        state,
        lx,
        282,
        YELLOW,
        "Decoded 20B Enemy_List Spawns (decode_act1_enemy_spawn):",
    );
    if stg.enemy_list.is_empty() {
        txt(state, lx, 298, MUTED, "  (No enemy spawns in this stage)");
    } else {
        for (i, raw_en) in stg.enemy_list.iter().take(4).enumerate() {
            let d = decode_act1_enemy_spawn(raw_en);
            txt(state, lx, 298 + i * 16, CYAN, &format!("  en[{i:02}]: kind=0x{:04X} id={:02} pos=({}, {}, {})mm rot={} st=0x{:04X} cond=0x{:04X}", d.kind, d.id, d.pos_x_mm, d.pos_y_mm, d.pos_z_mm, d.rot_y, d.status, d.condition));
        }
    }

    txt(
        state,
        lx,
        368,
        YELLOW,
        "Sample Item_List (gi_list) & Model_List (mdl_list):",
    );
    for (i, it) in stg.item_list.iter().take(3).enumerate() {
        txt(
            state,
            lx,
            384 + i * 16,
            WHITE,
            &format!(
                "  gi[{i:02}]: pos=({:.0}, {}, {:.0})mm kind_idx={} diff={} extra={} flag={}",
                it.pos_x,
                it.pos_y,
                it.pos_z,
                it.item_kind_index(),
                it.difficulty_condition(),
                it.extra_condition(),
                it.flag_id()
            ),
        );
    }
    for (i, m) in stg.model_list.iter().take(2).enumerate() {
        txt(
            state,
            lx,
            434 + i * 16,
            MUTED,
            &format!(
                "  mdl[{i:02}]: kind=0x{:04X} flag_off={} flag_on={} pos=({:.0},{:.0},{:.0})",
                m.kind, m.flag_0, m.flag_1, m.pos[0], m.pos[1], m.pos[2]
            ),
        );
    }

    render_act1_riddles_panel(state, idx);
}

fn render_act1_riddles_panel(state: &mut AppState, idx: usize) {
    let rx = 636usize;
    let riddle = state.sim_core.scenario.riddle_difficulty;
    let fixes = state.story_progression_room.act1_fixes;
    let memo = format_act1_wallet_safe_memo(&state.sim_core.scenario.puzzle_state, riddle);
    let dial = act1_read_safe_dial_pos(&state.sim_core.scenario.flags);
    let raw_code: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()
        [PZ_SAFE_CODE..PZ_SAFE_CODE + 4]
        .try_into()
        .unwrap_or([0; 4]);
    let raw_prog: [i8; 4] = [
        state.sim_core.scenario.puzzle_state.raw()[PZ_SAFE_PROG] as i8,
        state.sim_core.scenario.puzzle_state.raw()[PZ_SAFE_PROG + 1] as i8,
        state.sim_core.scenario.puzzle_state.raw()[PZ_SAFE_PROG + 2] as i8,
        state.sim_core.scenario.puzzle_state.raw()[PZ_SAFE_PROG + 3] as i8,
    ];
    let safe_open = state.sim_core.scenario.flags.get(ACT1_FLAG_SAFE_UNLOCKED);

    txt(
        state,
        rx,
        74,
        YELLOW,
        &format!("--- ACT 1 INTERACTIVE RIDDLES (Riddle={riddle:?}, Fixes=[V]) ---"),
    );
    txt(
        state,
        rx,
        98,
        CYAN,
        "1. W2F RM 203 SAFE DIAL ([Left/Right] Rotate | [Space] Step Target | [R] Difficulty):",
    );
    txt(
        state,
        rx,
        116,
        WHITE,
        &format!(
            "   Wallet Memo : \"{memo}\" (1-based notches: [{}, {}, {}, {}])",
            raw_code[0] + 1,
            raw_code[1] + 1,
            raw_code[2] + 1,
            raw_code[3] + 1
        ),
    );
    txt(state, rx, 134, if safe_open { GREEN } else { ORANGE }, &format!("   Active Dial : Notch {:02} (0-based {:02}) | i8[4] PZ_SAFE_PROG={raw_prog:?} | Open(108)={safe_open}", dial + 1, dial));

    let van_snake = act1_coin_puzzle_quad_layout(
        Act1DeskCoinKind::Snake,
        0,
        Act1VanillaFixesConfig::vanilla_ps2(),
    );
    let act_snake = act1_coin_puzzle_quad_layout(Act1DeskCoinKind::Snake, 0, fixes);
    let coin_vis = act1_is_old_man_coin_prop_visible(&state.sim_core.scenario.flags, fixes);
    let teddy_look = act1_should_james_look_at_prop(
        ACT1_PROP_KIND_TEDDY_BEAR,
        &state.sim_core.scenario.flags,
        fixes,
    );
    let ph_zone = state.story_progression_room.act1_ph_zone;
    let ph_hp = state.story_progression_room.act1_ph_hp_ratio;
    let ph_q12 = act1_eval_red_pyramid_walk_speed_q12(ph_hp * 500.0, 500.0, 1.0, ph_zone);

    txt(
        state,
        rx,
        162,
        CYAN,
        "2. RM 105 COIN DESK QUADS (0x008e0538), OLD MAN COIN FIX & PH SPEED ([V] / [C]):",
    );
    if let (Some(v), Some(a)) = (van_snake, act_snake) {
        txt(
            state,
            rx,
            180,
            WHITE,
            &format!(
                "   Snake Slot0 : Vanilla={:?} UV={:.0}..{:.0} | Active={:?} (Fix={})",
                v.bounds, v.uv[1], v.uv[3], a.bounds, fixes.coin_puzzle_alignment_fix
            ),
        );
    }
    let fl_95 = state.sim_core.scenario.flags.get(95);
    let fl_96 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_OLD_MAN_COIN_TAKEN);
    let fl_118 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_OLD_MAN_COIN_SUB);
    let fl_93 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_FIRE_ESCAPE_UNLOCKED);
    txt(state, rx, 198, WHITE, &format!("   OldManCoin  : PropVisible={coin_vis} (Fix={}, Juice95={fl_95}, Taken96={fl_96}, Sub118={fl_118})", fixes.old_man_coin_fix));
    txt(state, rx, 216, GREEN, &format!("   FireEscape  : Key(30)->Flag 93={fl_93} | TeddyLook={teddy_look} | enREDGetWalkSpeed(Zone{ph_zone}, HP={:.0}%)=Q12 {ph_q12}", ph_hp * 100.0));

    let wing_goal = act1_hospital_wing_keypad_goal(riddle);
    let wing_digits = state.story_progression_room.act1_wing_keypad.digits;
    let wing_unl = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_WING_KEYPAD_UNLOCKED);
    let carbon = format_act1_carbon_paper_code(&state.sim_core.scenario.puzzle_state);
    let wheels: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()[PZ_WHEEL..PZ_WHEEL + 4]
        .try_into()
        .unwrap_or([0; 4]);
    let wheel_goals: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()
        [PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
        .try_into()
        .unwrap_or([0; 4]);
    let fl_192 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_WALL_BLOOD_CODE_READ);
    let fl_203 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_ROOF_PYRAMID_HEAD_PUSHED);
    let fl_197 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_LOUISE_WHEEL_OPEN);
    let mut fl_prev = state.sim_core.scenario.flags.clone();
    let quiz_ticks = state.story_progression_room.act1_radio_quiz_ticks;
    let (active_sub, _) = act1_step_elevator_radio_quiz(quiz_ticks, false, &mut fl_prev);
    let q1 = u8::from(state.sim_core.scenario.flags.get(234))
        | (u8::from(state.sim_core.scenario.flags.get(235)) << 1);
    let q2 = u8::from(state.sim_core.scenario.flags.get(236))
        | (u8::from(state.sim_core.scenario.flags.get(237)) << 1);
    let q3 = u8::from(state.sim_core.scenario.flags.get(238))
        | (u8::from(state.sim_core.scenario.flags.get(239)) << 1);
    let fl_240 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_QUIZ_CHEST_SOLVED);
    let fl_241 = state
        .sim_core
        .scenario
        .flags
        .get(ACT1_FLAG_QUIZ_CHEST_WRONG);
    let jms_hp = state.story_progression_room.act1_james_hp;

    txt(
        state,
        rx,
        246,
        CYAN,
        "3. BROOKHAVEN HOSPITAL NORMAL & OTHERWORLD RIDDLES ([1] [2] [3] [4]):",
    );
    txt(state, rx, 266, if wing_unl { GREEN } else { WHITE }, &format!("   [1] 3F Wing Keypad : Goal={wing_goal:?} | Buffer={wing_digits:?} | Unlocked(Flag 181)={wing_unl}"));
    txt(
        state,
        rx,
        286,
        WHITE,
        &format!(
            "   [2] S14 Louise Box : Carbon={carbon} | Wheel={wheels:?} vs Goal={wheel_goals:?}"
        ),
    );
    txt(state, rx, 304, if fl_197 { GREEN } else { ORANGE }, &format!("       WallBlood(Flag 192)={fl_192} | RoofPHPush(Flag 203)={fl_203} | WheelOpen(Flag 197)={fl_197}"));
    txt(state, rx, 326, WHITE, &format!("   [3] Elevator Quiz  : Tick={quiz_ticks:.0}/5175.0 (42 Subtitle Windows @ 0x008d5618) | ActiveMsg={active_sub:?}"));
    txt(state, rx, 346, if fl_240 { GREEN } else { ORANGE }, &format!("   [4] 3F Quiz Chest  : Buttons=({q1}, {q2}, {q3}) vs Target=(3, 1, 3) | Solved(240)={fl_240} | Poison(241)={fl_241} | HP={jms_hp:.0}"));

    txt(
        state,
        22,
        482,
        YELLOW,
        "--- ALL 27 ACT 1 STAGES ([PgUp / PgDn] to Inspect) ---",
    );
    for (i, d) in ACT1_ALL_STAGES.iter().enumerate() {
        let sel = i == idx;
        txt(
            state,
            22 + (i / 9) * 415,
            500 + (i % 9) * 18,
            if sel { GREEN } else { MUTED },
            &format!(
                "{} {:02}: {:<22} (ev={:02} gi={:02} en={:02})",
                if sel { ">>" } else { "  " },
                d.stage_id as u8,
                format!("{:?}", d.stage_id),
                d.event_list.len().saturating_sub(1),
                d.item_list.len(),
                d.enemy_list.len()
            ),
        );
    }
}
