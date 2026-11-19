//! Tab 3 (`[F3]`): Playable 3D Stage & Secret Weapons Arena + Live Ending Judge HUD (`SH2-118`).
//!
//! Leaves the left `720px` completely clear so the playable 3D stage and James/Maria are in full
//! view, while rendering a compact translucent HUD panel on the right (`x=730..1265, y=70..655`).

use super::hud_common::{
    draw_card, draw_progress_bar, txt_left, txt_tab3, CYAN, GREEN, MUTED, ORANGE, WHITE, YELLOW,
};
use crate::state::AppState;
use sh2_core::endings::{
    evaluate_ngplus_secret_spawns, evaluate_spray_enemy_reaction, evaluate_star_ranking,
    EndingEvalFlags, HyperSprayColor, CHAINSAW_ATTACK_SPECS, REBIRTH_RITUAL_ITEM_IDS,
};

pub(crate) fn render_endings_judge_tab(state: &mut AppState) {
    // Small top-left 3D Controls pill so the user knows they can walk around in 3D while leaving x=0..720 clear
    draw_card(
        state,
        20,
        68,
        490,
        42,
        [10, 14, 22, 165],
        [70, 95, 130, 210],
    );
    let char_name = if state.active_maria { "Maria" } else { "James" };
    let pos = state.sim_core.scenario.player_position;
    txt_left(
        state,
        28,
        74,
        GREEN,
        &format!(
            "LIVE 3D ARENA ({char_name}): Pos=({:.0}, {:.0})mm | Flashlight[F]={}",
            pos.x, pos.z, state.sim_core.scenario.flashlight_on
        ),
    );
    txt_left(
        state,
        28,
        92,
        CYAN,
        "[WASD+Shift] Walk/Run | [Space] Attack | [C] Chainsaw | [H] Hyper Spray",
    );

    // Compact Translucent Right HUD Panel (x=730..1265, y=70..655)
    let (hx, hy, hw, hh) = (730usize, 70usize, 535usize, 585usize);
    draw_card(
        state,
        hx,
        hy,
        hw,
        hh,
        [10, 14, 22, 196],
        [80, 105, 140, 240],
    );

    // Section 1: Active Secret Weapons (Chainsaw & Hyper Spray) (y=78..258)
    txt_tab3(
        state,
        hx + 12,
        78,
        YELLOW,
        "SECRET NG+ WEAPONS ARENA ([C] Chainsaw | [H] Hyper Spray)",
    );

    let cs_idx = state.story_progression_room.chainsaw_attack_idx % CHAINSAW_ATTACK_SPECS.len();
    let spec = &CHAINSAW_ATTACK_SPECS[cs_idx];
    let cs_state = state.story_progression_room.chainsaw_sfx.idle_state;
    let cs_fix = state.story_progression_room.chainsaw_fix_enabled;
    txt_tab3(
        state,
        hx + 12,
        98,
        GREEN,
        &format!(
            "[C] Chainsaw Attack #{:02}: AP={:.0} | Range={:.0}mm | SFX=0x{:04X}",
            spec.id, spec.ap, spec.max_range, spec.sfx_id
        ),
    );
    draw_progress_bar(
        state,
        hx + 12,
        116,
        340,
        14,
        (spec.ap / 1000.0).clamp(0.0, 1.0),
        ORANGE,
        YELLOW,
    );
    txt_tab3(
        state,
        hx + 360,
        116,
        WHITE,
        &format!("{:.0}/1000 AP", spec.ap),
    );
    txt_tab3(
        state,
        hx + 12,
        136,
        CYAN,
        &format!(
            "    Rev State: {cs_state:?} | Frames={}..{} | SoundFix[V]={cs_fix}",
            spec.first_hit_frame, spec.last_hit_frame
        ),
    );

    let pi = state.story_progression_room.playing_info.clone();
    let color = HyperSprayColor::from_spray_pow(pi.spray_pow);
    let reaction = evaluate_spray_enemy_reaction(pi.spray_pow, false);
    let rem_secs = state.story_progression_room.spray_can.remaining_secs;
    let rgba = color.rgba();
    txt_tab3(
        state,
        hx + 12,
        160,
        YELLOW,
        &format!(
            "[H] Hyper Spray Rank Mist: {color:?} (Tier {}) -> {reaction:?}",
            pi.spray_pow
        ),
    );
    state
        .menu_rasterizer
        .fill_rect(hx + 12, 178, 56, 18, [rgba[0], rgba[1], rgba[2], 255]);
    state
        .menu_rasterizer
        .draw_rect_outline(hx + 12, 178, 56, 18, 1, WHITE);
    draw_progress_bar(
        state,
        hx + 78,
        179,
        274,
        16,
        (rem_secs / 20.0).clamp(0.0, 1.0),
        [rgba[0], rgba[1], rgba[2], 235],
        WHITE,
    );
    txt_tab3(
        state,
        hx + 360,
        180,
        WHITE,
        &format!("Can: {rem_secs:.1}/20s"),
    );

    let ruleset = state.story_progression_room.ranking_ruleset;
    let rank = evaluate_star_ranking(&pi, ruleset);
    let ng = evaluate_ngplus_secret_spawns(
        pi.clear_end_kind,
        pi.clear_end_number,
        pi.battle_level,
        pi.cleared_difficulty_mask,
        state.story_progression_room.prevent_first_run_chainsaw,
    );
    txt_tab3(
        state,
        hx + 12,
        204,
        GREEN,
        &format!(
            "[G] 10-Star Rank ({ruleset:?}): {}/100 ({} Big + {} Small Stars)",
            rank.total_points,
            rank.big_stars(),
            rank.small_stars()
        ),
    );
    txt_tab3(
        state,
        hx + 12,
        222,
        MUTED,
        &format!(
            "    NG+ Spawns: Chainsaw={} Spray={} DogKey={} Gem={} Rebirth={}",
            ng.spawn_chainsaw,
            ng.spawn_hyper_spray,
            ng.spawn_dog_key,
            ng.spawn_blue_gem,
            ng.spawn_ngplus_bonus_items
        ),
    );

    // Section 2: Live Ending Judge Score Bars (`EvProgEndingJudge`) (y=248..650)
    let rebirth_cnt = REBIRTH_RITUAL_ITEM_IDS
        .iter()
        .filter(|&&id| state.sim_core.scenario.inventory_mgr.has_item(id))
        .count() as u8;
    let eval_flags = EndingEvalFlags::from_event_flags(&state.sim_core.scenario.flags, rebirth_cnt);
    let report = state
        .story_progression_room
        .point_tracker
        .evaluate(&pi, eval_flags);

    txt_tab3(
        state,
        hx + 12,
        248,
        YELLOW,
        "LIVE ENDING JUDGE SCORE BARS (EvProgEndingJudge @ 0x00576490)",
    );

    draw_card(state, hx + 12, 266, hw - 24, 30, [20, 52, 38, 240], GREEN);
    txt_tab3(
        state,
        hx + 22,
        275,
        GREEN,
        &format!(
            "WINNING ENDING: {:?}  (Leave={} | Maria={} | InWater={})",
            report.selected_ending,
            report.leave_points,
            report.maria_points,
            report.in_water_points
        ),
    );

    let max_pts = (report
        .leave_points
        .max(report.maria_points)
        .max(report.in_water_points)
        .max(10)) as f32;
    let bars = [
        ("LEAVE    ", report.leave_points, GREEN),
        ("MARIA    ", report.maria_points, [235, 115, 210, 255]),
        ("IN WATER ", report.in_water_points, CYAN),
    ];
    for (i, &(label, pts, col)) in bars.iter().enumerate() {
        let by = 304 + i * 24;
        txt_tab3(state, hx + 14, by + 2, WHITE, label);
        let ratio = (pts.max(0) as f32) / max_pts;
        draw_progress_bar(state, hx + 96, by, 330, 16, ratio, col, WHITE);
        txt_tab3(state, hx + 436, by + 2, col, &format!("{pts:+} pts"));
    }

    let fl_516 = state.sim_core.scenario.flags.get(516);
    let fl_622 = state.sim_core.scenario.flags.get(622);
    let gem_cnt = u8::from(state.sim_core.scenario.flags.get(725))
        + u8::from(state.sim_core.scenario.flags.get(726))
        + u8::from(state.sim_core.scenario.flags.get(727));

    draw_card(
        state,
        hx + 12,
        382,
        164,
        38,
        if rebirth_cnt == 4 {
            [20, 56, 36, 240]
        } else {
            [22, 26, 36, 230]
        },
        if rebirth_cnt == 4 { GREEN } else { MUTED },
    );
    txt_tab3(state, hx + 20, 388, YELLOW, "[6] REBIRTH ITEMS");
    txt_tab3(
        state,
        hx + 20,
        404,
        if rebirth_cnt == 4 { GREEN } else { WHITE },
        &format!("{rebirth_cnt}/4 Ritual Held"),
    );

    draw_card(
        state,
        hx + 184,
        382,
        164,
        38,
        if fl_622 {
            [20, 56, 36, 240]
        } else {
            [22, 26, 36, 230]
        },
        if fl_622 { GREEN } else { MUTED },
    );
    txt_tab3(state, hx + 192, 388, YELLOW, "[7] DOG ENDING");
    txt_tab3(
        state,
        hx + 192,
        404,
        if fl_622 { GREEN } else { WHITE },
        if fl_622 {
            "UNLOCKED (Rm 3F)"
        } else if fl_516 {
            "Door Open"
        } else {
            "Locked (Key #25)"
        },
    );

    draw_card(
        state,
        hx + 356,
        382,
        166,
        38,
        if gem_cnt == 3 {
            [20, 56, 36, 240]
        } else {
            [22, 26, 36, 230]
        },
        if gem_cnt == 3 { GREEN } else { ORANGE },
    );
    txt_tab3(state, hx + 364, 388, YELLOW, "[8] UFO BLUE GEM");
    txt_tab3(
        state,
        hx + 364,
        404,
        if gem_cnt == 3 { GREEN } else { ORANGE },
        &format!("Step {gem_cnt}/3 Channel"),
    );

    let pt = state.story_progression_room.point_tracker.clone();
    txt_tab3(
        state,
        hx + 12,
        430,
        YELLOW,
        "TRIGGER GAMEPLAY ENDING ACTIONS ([1..=8] Live Score Update):",
    );
    txt_tab3(
        state,
        hx + 12,
        450,
        WHITE,
        &format!(
            "[1] In Water: LowHP={:.0}s | Diary={} Tape={} NeelyBar={}",
            pt.low_hp_weighted_secs,
            eval_flags.hospital_roof_diary_read,
            eval_flags.hotel_reading_room_tape_heard,
            eval_flags.neelys_bar_second_message_read
        ),
    );
    txt_tab3(
        state,
        hx + 12,
        470,
        WHITE,
        &format!(
            "[2] Leave/InWater: Overheal={:.1} | MaryPhoto/Letter={}/2 | Knife={}",
            pt.overheal_ratio_sum, pt.mary_item_examine_count, eval_flags.angelas_knife_examined
        ),
    );
    txt_tab3(
        state,
        hx + 12,
        490,
        WHITE,
        &format!(
            "[3] Maria Escort: Close={:.0}s | S3Check={} | LabyrinthRevisit={}",
            pt.maria_close_secs,
            eval_flags.hospital_s3_maria_second_check,
            eval_flags.labyrinth_maria_room_revisited
        ),
    );
    txt_tab3(
        state,
        hx + 12,
        510,
        WHITE,
        &format!(
            "[4] Hotel Hallway Mary Monologue: {:?} (Complete={})",
            pt.corridor_monologue, eval_flags.mary_corridor_monologue_complete
        ),
    );
    txt_tab3(
        state,
        hx + 12,
        530,
        CYAN,
        &format!(
            "[5] Cycle Prior ClearEndKind Mask: 0x{:02X} ({} Base Endings Cleared)",
            pi.clear_end_kind.0, pi.clear_end_number
        ),
    );
    txt_tab3(
        state,
        hx + 12,
        550,
        WHITE,
        "[6] Toggle All 4 Rebirth Ritual Items (Crimson/Chrism/Goblet/Memories)",
    );
    txt_tab3(
        state,
        hx + 12,
        570,
        WHITE,
        "[7] Use Dog Key (#25) at Observation Room | [8] Step Blue Gem (1->2->3)",
    );
    txt_tab3(
        state,
        hx + 12,
        594,
        GREEN,
        "[9] Reset Ending Scores | [M] Swap James/Maria | [T] Toggle 4K Textures",
    );
    txt_tab3(
        state,
        hx + 12,
        614,
        MUTED,
        "[G] Toggle PS2 vs PC 10-Star Ruleset | [V] Toggle Camera / Weapon Fixes",
    );
}
