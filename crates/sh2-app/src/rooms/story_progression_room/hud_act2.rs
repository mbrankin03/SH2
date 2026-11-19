//! Tab 2 (`[F2]`): Act 2 & *Born from a Wish* Stages 25–58 & Riddles (`SH2-114`) HUD overlay.

use super::ACT2_TRG10_KEY_DOORS;
use crate::state::AppState;
use sh2_core::events::stages_act2::{
    decode_hangman_digits, decode_retail_ev_pos_shape, effective_prisoner_step_threshold,
    gravestone_board_uv, gravestone_board_verts, hangman_innocent_slot,
    is_hotel_2f_teleport_corridor_room, selected_briefcase_word, stage_table_act2,
    sync_labyrinth_face_cube, GravestoneBoard, RetailTriggerKind, ALL_ACT2_STAGE_IDS,
    BRIEFCASE_TRUNK_DRUMS, FLAG_BRIEFCASE_UNLOCKED, FLAG_ELEVATOR_WEIGHT_OK, FLAG_MUSIC_BOX_SOLVED,
    FLAG_SCAFFOLD_ALL_SET, FLAG_WISH_GRAVESTONE_SOLVED,
};
use sh2_core::events::PZ_DRUM;

const YELLOW: [u8; 4] = [245, 210, 65, 255];
const CYAN: [u8; 4] = [105, 215, 235, 255];
const GREEN: [u8; 4] = [120, 230, 150, 255];
const WHITE: [u8; 4] = [235, 235, 235, 255];
const ORANGE: [u8; 4] = [255, 170, 80, 255];
const MUTED: [u8; 4] = [165, 170, 185, 255];

fn txt(state: &mut AppState, x: usize, y: usize, color: [u8; 4], s: &str) {
    state.menu_rasterizer.draw_text(s, x, y, 1, color, true);
}

pub(super) fn render_act2_tab(state: &mut AppState) {
    let idx = state.story_progression_room.act2_stage_idx % ALL_ACT2_STAGE_IDS.len();
    let stage_id = ALL_ACT2_STAGE_IDS[idx];
    let Some(stg) = stage_table_act2(stage_id) else {
        return;
    };
    let h = &stg.header;

    let lx = 22usize;
    txt(
        state,
        lx,
        74,
        YELLOW,
        &format!(
            "--- ACT 2 & WISH STAGE [PgUp/PgDn]: Stage {:02} ({} / {:?}) ---",
            stg.stage_id, stg.name, stg.stage_kind
        ),
    );
    txt(
        state,
        lx,
        92,
        CYAN,
        &format!(
            "64B Stage_Data : PC VA=0x{:08X} | PS2 VA=0x{:08X} | glb_crd={}",
            stg.retail_stage_data_va, stg.ps2_stage_data_va, h.glb_crd
        ),
    );
    txt(
        state,
        lx,
        108,
        WHITE,
        &format!(
            "Words [0..5]   : ev_list=0x{:08X} | ev_pos=0x{:08X} | ev_prog=0x{:08X}",
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
            "Words [6..8]   : stage_init=0x{:08X} | room_init=0x{:08X} | all_time=0x{:08X}",
            h.stage_init_fn, h.room_init_fn, h.all_time_fn
        ),
    );
    txt(
        state,
        lx,
        156,
        GREEN,
        &format!(
            "Words [15,12,14]: sound_load(+0x3c)=0x{:08X} | bgm(+0x30)=0x{:08X} | clr(+0x38)=0x{:08X}",
            h.sound_call_after_load_fn, h.bgm_control_fn, h.chara_data_clear_fn
        ),
    );
    txt(
        state,
        lx,
        174,
        WHITE,
        &format!(
            "Counts         : Event_List={} (+sentinel) | ev_pos={}B | ev_prog={} | Items={} | Enemies={}",
            stg.active_event_count(),
            stg.pos_list.len(),
            stg.ev_prog_vas.len(),
            stg.retail_items.len(),
            stg.retail_enemies.len()
        ),
    );

    txt(
        state,
        lx,
        196,
        YELLOW,
        "Sample Event_List & Half-Float ev_pos Shapes (decode_retail_ev_pos_shape):",
    );
    for (i, ev) in stg.event_list.iter().take(3).enumerate() {
        if ev.is_sentinel() {
            break;
        }
        let r_trg = RetailTriggerKind::from_entry(ev);
        let sh_ok = ev
            .shape_kind()
            .and_then(|sh| decode_retail_ev_pos_shape(stg.pos_list, ev.ev_pos_offset(), sh))
            .is_some();
        txt(
            state,
            lx,
            212 + i * 16,
            WHITE,
            &format!(
                "  ev[{i:02}]: cond=0x{:08X} (trg={r_trg:?}) rslt0=0x{:08X} shape_ok={sh_ok}",
                ev.cond, ev.rslt0
            ),
        );
    }

    txt(
        state,
        lx,
        264,
        YELLOW,
        "Sample RetailItemSpawn (16B) & RetailEnemySpawn (20B):",
    );
    for (i, it) in stg.retail_items.iter().take(2).enumerate() {
        txt(
            state,
            lx,
            280 + i * 16,
            CYAN,
            &format!(
                "  gi[{i:02}]: pos=({:.0},{:.0},{:.0})mm mdl={:?} kind={} flag={}",
                it.pos_x(),
                it.pos_y(),
                it.pos_z(),
                it.pickup_model_kind(),
                it.item_kind_index(),
                it.flag_id()
            ),
        );
    }
    for (i, en) in stg.retail_enemies.iter().take(2).enumerate() {
        txt(
            state,
            lx,
            314 + i * 16,
            WHITE,
            &format!(
                "  en[{i:02}]: kind=0x{:04X} id={} defeat_flag={:?} pos=({},{},{})mm",
                en.kind,
                en.id,
                en.defeat_flag_id(),
                en.pos_x,
                en.pos_y,
                en.pos_z
            ),
        );
    }

    let door_idx = state.story_progression_room.act2_trg10_door_idx;
    let carrying = state.story_progression_room.act2_carrying_key;
    let last_res = state.story_progression_room.act2_trg10_last_result.clone();
    let (_, _, key_id, door_label) = ACT2_TRG10_KEY_DOORS[door_idx];
    txt(
        state,
        lx,
        356,
        YELLOW,
        "--- INTERACTIVE TRG==10 KEY-DOOR TESTER (All 9 Act 2 Key Doors) ---",
    );
    txt(
        state,
        lx,
        374,
        CYAN,
        &format!("Selected [Left/Right] ({}/9): {door_label}", door_idx + 1),
    );
    txt(
        state,
        lx,
        392,
        if carrying { GREEN } else { ORANGE },
        &format!(
            "Carrying Key #{key_id} [K]: {carrying} | [A] Test Action Press (0x0048e1ee) | [U] Test Menu Use (0x0048e1c1)"
        ),
    );
    txt(state, lx, 410, WHITE, &format!("Result: {last_res}"));

    render_act2_riddles_panel(state, idx);
}

fn render_act2_riddles_panel(state: &mut AppState, idx: usize) {
    let rx = 636usize;
    let mut fl_copy = state.sim_core.scenario.flags.clone();
    let cube = sync_labyrinth_face_cube(&mut fl_copy);
    let toggles = state.story_progression_room.act2_toggles;
    let prisoner_thresh =
        effective_prisoner_step_threshold(45, 60, toggles.prisoner_timer_60fps_fix);
    let fl_301 = state.sim_core.scenario.flags.get(301);
    let fl_302 = state.sim_core.scenario.flags.get(302);
    let fl_303 = state.sim_core.scenario.flags.get(303);
    let fl_304 = state.sim_core.scenario.flags.get(FLAG_SCAFFOLD_ALL_SET);

    txt(
        state,
        rx,
        74,
        YELLOW,
        "--- ACT 2 & BORN FROM A WISH INTERACTIVE RIDDLES ([1]..[8], [V]) ---",
    );
    txt(
        state,
        rx,
        98,
        if fl_304 { GREEN } else { WHITE },
        &format!(
            "[1] PRISON GALLOWS : Tablets(301..303)=[{fl_301},{fl_302},{fl_303}] AllSet(304)={fl_304} | PrisonerTimer(60Hz)={prisoner_thresh} ticks (Fix={})",
            toggles.prisoner_timer_60fps_fix
        ),
    );
    txt(
        state,
        rx,
        122,
        if cube.passage_open { GREEN } else { ORANGE },
        &format!(
            "[2] LABYRINTH CUBE : plane={} roll={} itr={} pitch={:.2} yaw={:.2} | check={} -> Flag325={} Flag326={}",
            cube.plane,
            cube.roll,
            cube.itr,
            cube.pitch_rad,
            cube.yaw_rad,
            cube.connectivity_level,
            cube.passage_open,
            cube.maria_path_open
        ),
    );

    let digits = decode_hangman_digits(&state.sim_core.scenario.puzzle_state);
    let innocent = hangman_innocent_slot(&state.sim_core.scenario.puzzle_state);
    let solved_hangman = (0..6).any(|i| state.sim_core.scenario.flags.get(343 + i));
    txt(
        state,
        rx,
        146,
        if solved_hangman { GREEN } else { WHITE },
        &format!(
            "[3] 6 HANGED MEN   : PZ_HANGMAN={digits:?} | Innocent(digit==5)=Slot {innocent:?} | Solved={solved_hangman}"
        ),
    );

    let riddle = state.sim_core.scenario.riddle_difficulty;
    let fl_472 = state.sim_core.scenario.flags.get(FLAG_MUSIC_BOX_SOLVED);
    txt(
        state,
        rx,
        170,
        if fl_472 { GREEN } else { WHITE },
        &format!(
            "[4] HOTEL MUSIC BOX: Level={riddle:?} | Solved(Flag 472)={fl_472} | MusicBoxVolumeFix={} (Bank={})",
            toggles.music_box_volume_fix,
            if toggles.music_box_volume_fix { 4 } else { 0 }
        ),
    );

    let fl_446 = state.sim_core.scenario.flags.get(446);
    let fl_400 = state.sim_core.scenario.flags.get(FLAG_ELEVATOR_WEIGHT_OK);
    let ammo = state.sim_core.scenario.inventory_mgr.count(5);
    txt(
        state,
        rx,
        194,
        if fl_400 { GREEN } else { ORANGE },
        &format!(
            "[5] 2F CUPBOARD    : ShelfHasItems(446)={fl_446} | WeightOK(Flag 400)={fl_400} | HandgunAmmo={ammo}"
        ),
    );

    let word = selected_briefcase_word(&state.sim_core.scenario.flags);
    let raw_drums: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()[PZ_DRUM..PZ_DRUM + 4]
        .try_into()
        .unwrap_or([0; 4]);
    let cur_chars = [
        BRIEFCASE_TRUNK_DRUMS[0][(raw_drums[0] & 7) as usize] as char,
        BRIEFCASE_TRUNK_DRUMS[1][(raw_drums[1] & 7) as usize] as char,
        BRIEFCASE_TRUNK_DRUMS[2][(raw_drums[2] & 7) as usize] as char,
        BRIEFCASE_TRUNK_DRUMS[3][(raw_drums[3] & 7) as usize] as char,
    ];
    let fl_413 = state.sim_core.scenario.flags.get(FLAG_BRIEFCASE_UNLOCKED);
    txt(
        state,
        rx,
        218,
        if fl_413 { GREEN } else { WHITE },
        &format!(
            "[6] 204 BRIEFCASE  : Target=\"{word}\" | Drums=\"{}{}{}{}\" ({raw_drums:?}) | Open(413)={fl_413}",
            cur_chars[0], cur_chars[1], cur_chars[2], cur_chars[3]
        ),
    );

    let room_byte = 0x29 + (state.story_progression_room.act2_corridor_room_idx as u8);
    let teleports = is_hotel_2f_teleport_corridor_room(room_byte);
    txt(
        state,
        rx,
        242,
        if teleports { GREEN } else { WHITE },
        &format!(
            "[7] 2F_B TELEPORT  : Room 0x{room_byte:02X} Teleports={teleports} (Active rooms: 0x29, 0x2D, 0x31, 0x39)"
        ),
    );

    let fix = toggles.gravestone_boards_alignment_fix;
    let rot = state.story_progression_room.act2_gravestone_rot;
    let uv_w = gravestone_board_uv(GravestoneBoard::White, fix);
    let verts = gravestone_board_verts(rot, fix);
    let fl_704 = state
        .sim_core
        .scenario
        .flags
        .get(FLAG_WISH_GRAVESTONE_SOLVED);
    let has_acacia = state.sim_core.scenario.inventory_mgr.has_item(82);
    txt(
        state,
        rx,
        266,
        if fl_704 { GREEN } else { WHITE },
        &format!(
            "[8] WISH GRAVESTONE: Solved(704)={fl_704} | AcaciaKey(82)={has_acacia} | GravestoneBoardsFix[V]={fix}"
        ),
    );
    txt(
        state,
        rx,
        284,
        CYAN,
        &format!(
            "    White UV=({:.0},{:.0})..({:.0},{:.0}) | Rot={rot:?} Verts={verts:?}",
            uv_w.u1, uv_w.v1, uv_w.u2, uv_w.v2
        ),
    );

    txt(
        state,
        22,
        438,
        YELLOW,
        "--- ALL 35 ACT 2 & BORN FROM A WISH STAGES ([PgUp / PgDn] to Inspect) ---",
    );
    for (i, &sid) in ALL_ACT2_STAGE_IDS.iter().enumerate() {
        let sel = i == idx;
        if let Some(t) = stage_table_act2(sid) {
            txt(
                state,
                22 + (i / 12) * 415,
                456 + (i % 12) * 17,
                if sel { GREEN } else { MUTED },
                &format!(
                    "{} {:02}: {:<22} (ev={:02} gi={:02} en={:02})",
                    if sel { ">>" } else { "  " },
                    sid,
                    t.name,
                    t.active_event_count(),
                    t.retail_items.len(),
                    t.retail_enemies.len()
                ),
            );
        }
    }
}
