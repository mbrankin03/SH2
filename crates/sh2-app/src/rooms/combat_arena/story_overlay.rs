//! 2D In-Game Story Action Bar (`y = 635..708`), Cutscene Skip Banner, and Centered Interactive
//! `.tex` Cutaway / UFO / Ending Judge Modal Overlay for 3D Gameplay (`SH2-65`).

use super::story_progression_3d::{active_3d_checkpoint, STORY_3D_CHECKPOINT_COUNT};
use crate::rooms::story_progression_room::{
    hud_act1::render_act1_tab,
    hud_act2::render_act2_tab,
    hud_common::{
        blit_cutaway_viewport, draw_card, draw_progress_bar, draw_text_clipped, txt_right, CYAN,
        GREEN, MUTED, ORANGE, WHITE, YELLOW,
    },
    hud_endings::render_endings_judge_tab,
    hud_ufo_letter::render_ufo_and_letter_tab,
};
use crate::state::AppState;
use sh2_core::endings::{
    evaluate_spray_enemy_reaction, visible_letter_entries_at, HyperSprayColor,
    CHAINSAW_ATTACK_SPECS, LETTER_MSG_E_ENTRIES, LETTER_SCROLL_SPEED_PX_PER_SEC,
};
use sh2_core::riddle_level_index;
use sh2_ui::menu::TestRoomKind;

/// Renders the 3D Story Progression HUD layer:
/// 1. Bright Cutscene Skip Banner while a `.dds` cutscene (`first_toilet`, `tenbou`, etc.) is playing.
/// 2. Bottom In-Game Story Action Bar (`y = 635..708`) while walking around in 3D.
/// 3. Centered Interactive `.tex` Cutaway / UFO / Ending Judge Modal Overlay when popped up with `[E]` / `[Enter]`.
pub(crate) fn render_3d_story_overlay(state: &mut AppState) {
    let cutscene_locked = state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input());

    if cutscene_locked {
        draw_cutscene_skip_banner(state);
        return;
    }

    if state.story_progression_room.modal_3d_open {
        draw_interactive_story_modal_overlay(state);
    }

    draw_bottom_story_action_bar(state);
}

fn draw_cutscene_skip_banner(state: &mut AppState) {
    let bx = 250usize;
    let by = 18usize;
    let bw = 780usize;
    let bh = 42usize;
    draw_card(state, bx, by, bw, bh, [16, 28, 44, 240], YELLOW);
    draw_text_clipped(
        state,
        bx + 76,
        by + 8,
        88,
        YELLOW,
        ">>> PRESS [ENTER] TO SKIP CUTSCENE & PLAY IN 3D <<<",
    );
    draw_text_clipped(
        state,
        bx + 44,
        by + 24,
        92,
        CYAN,
        "([Enter / Esc / G] Skip into Live 3D Room | [PgUp/PgDn] 12 Story Checkpoints | [E] Action)",
    );
}

fn draw_bottom_story_action_bar(state: &mut AppState) {
    let (bx, by, bw, bh) = (12usize, 635usize, 1256usize, 73usize);
    draw_card(
        state,
        bx,
        by,
        bw,
        bh,
        [10, 14, 24, 236],
        [85, 115, 155, 255],
    );

    let cp = active_3d_checkpoint(state);
    let cp_num = cp.index + 1;
    let proximity_prompt = active_proximity_action_prompt(state, cp.index);

    draw_text_clipped(
        state,
        bx + 12,
        by + 7,
        152,
        YELLOW,
        &format!(
            "STORY CHECKPOINT [{cp_num:02}/{STORY_3D_CHECKPOINT_COUNT}] ([PgUp/PgDn] Step): {} — {}",
            cp.stage_badge, cp.title
        ),
    );

    draw_text_clipped(state, bx + 12, by + 24, 152, GREEN, &proximity_prompt);

    let status = state.story_progression_room.status_banner.clone();
    draw_text_clipped(
        state,
        bx + 12,
        by + 41,
        152,
        ORANGE,
        &format!("EVENT: {status}"),
    );

    let weapon_summary = active_secret_weapons_summary(state);
    draw_text_clipped(
        state,
        bx + 12,
        by + 57,
        152,
        CYAN,
        &format!(
            "[E/Enter] Interact/Cutaway | [PgUp/PgDn] 12 Checkpoints | [7] Chainsaw | [8] Hyper Spray | [U] UFO Gem | [L] Ending Judge & Letter | {weapon_summary}"
        ),
    );
}

fn active_proximity_action_prompt(state: &AppState, cp_idx: usize) -> String {
    if state.story_progression_room.modal_3d_open {
        return ">>> [ESC / E] Close Overlay & Return to 3D Walking | Use Arrow Keys / [Space] / [V] Inside Cutaway <<<".to_string();
    }

    if state.active_test_room == TestRoomKind::RestroomStart && cp_idx == 0 {
        let p_pos = state.sim_core.scenario.player_position;
        if p_pos.x < -19250.0 && !state.story_progression_room.restroom_mirror_inspected {
            return "PROXIMITY ACTION [E / Enter]: Inspect Dirty Restroom Mirror (\"James stares into the mirror...\")".to_string();
        }
        return "PROXIMITY ACTION [E / Enter]: Exit Restroom Door (er01.map) -> Observation Deck (ob01.map) [Stage 3 stg_ovservation]!".to_string();
    }

    if state.active_test_room == TestRoomKind::ObservationDeck && cp_idx == 1 {
        if !state.sim_core.scenario.flags.get(28) {
            return "PROXIMITY ACTION [E / Enter]: Inspect Blue Car Seat (out_p_incar.tex) & Take Map of Silent Hill (Flag 28, SE 0x2b17)!".to_string();
        }
        return "PROXIMITY ACTION [E / Enter]: Map of Silent Hill Taken (Flag 28=true)! Press [E / Enter] at Observation Deck Gate to Advance Story!".to_string();
    }

    let cp = active_3d_checkpoint(state);
    format!("ACTIVE ACTION: {}", cp.prompt_hint)
}

fn active_secret_weapons_summary(state: &AppState) -> String {
    let cs_spec = &CHAINSAW_ATTACK_SPECS
        [state.story_progression_room.chainsaw_attack_idx % CHAINSAW_ATTACK_SPECS.len()];
    let spray_col =
        HyperSprayColor::from_spray_pow(state.story_progression_room.playing_info.spray_pow);
    let react =
        evaluate_spray_enemy_reaction(state.story_progression_room.playing_info.spray_pow, false);
    format!(
        "Weapon={:?} (CS#{:02} {:.0}AP | Spray={spray_col:?}/{react:?})",
        state.combat_arena.player.weapon, cs_spec.id, cs_spec.ap
    )
}

fn draw_interactive_story_modal_overlay(state: &mut AppState) {
    let cp_idx = state.story_progression_room.checkpoint_3d_idx % STORY_3D_CHECKPOINT_COUNT;

    // Top modal banner over the 3D scene
    draw_card(state, 12, 8, 1256, 46, [12, 22, 36, 245], YELLOW);
    let cp = active_3d_checkpoint(state);
    draw_text_clipped(
        state,
        24,
        14,
        150,
        YELLOW,
        &format!(
            "3D INTERACTIVE STORY CUTAWAY [{:02}/12]: {} — {}  >>> [ESC / E] Close Overlay & Return to 3D Walking <<<",
            cp.index + 1,
            cp.stage_badge,
            cp.title
        ),
    );
    draw_text_clipped(
        state,
        24,
        32,
        150,
        CYAN,
        &format!(
            "{} | [R] Cycle Riddle Difficulty ({:?}) | [T] Toggle SH2:EE HD vs PS2 Cutaway",
            cp.prompt_hint, state.sim_core.scenario.riddle_difficulty
        ),
    );

    match cp_idx {
        1 => draw_stage3_incar_map_modal(state),
        2..=5 => {
            draw_card(
                state,
                12,
                54,
                1256,
                578,
                [8, 10, 18, 224],
                [70, 95, 130, 245],
            );
            render_act1_tab(state);
        }
        6..=9 => {
            draw_card(
                state,
                12,
                54,
                1256,
                578,
                [8, 10, 18, 224],
                [70, 95, 130, 245],
            );
            render_act2_tab(state);
        }
        10 => {
            draw_card(
                state,
                12,
                54,
                1256,
                578,
                [8, 10, 18, 224],
                [70, 95, 130, 245],
            );
            render_ufo_and_letter_tab(state);
        }
        _ => {
            render_endings_judge_tab(state);
            draw_mary_letter_inset_over_3d(state);
        }
    }
}

fn draw_stage3_incar_map_modal(state: &mut AppState) {
    let (mx, my, mw, mh) = (320usize, 62usize, 640usize, 560usize);
    draw_card(state, mx, my, mw, mh, [10, 14, 24, 242], YELLOW);

    let map_taken = state.sim_core.scenario.flags.get(28);
    let pref = state.story_progression_room.prefer_sh2ee_cutaways;
    let r_idx = riddle_level_index(state.sim_core.scenario.riddle_difficulty);
    state
        .sim_core
        .scenario
        .cutaway_overlay
        .set_picture(Some("out_p_incar"), &state.sim_core.scenario.flags);
    state
        .sim_core
        .scenario
        .cutaway_overlay
        .set_incar_alpha(if map_taken { 0.0 } else { 1.0 });
    let quads = state
        .sim_core
        .scenario
        .cutaway_overlay
        .evaluate_frame(
            &state.sim_core.scenario.flags,
            &state.sim_core.scenario.puzzle_state,
            r_idx,
            0,
        )
        .to_vec();

    let (vp_x, vp_y, vp_w, vp_h) = (mx + 40, my + 32, 560usize, 420usize);
    blit_cutaway_viewport(state, vp_x, vp_y, vp_w, vp_h, "out_p_incar", pref, &quads);

    draw_text_clipped(
        state,
        mx + 40,
        my + 10,
        70,
        YELLOW,
        "STAGE 3 OBSERVATION DECK: JAMES'S BLUE CAR SEAT (out_p_incar.tex)",
    );

    let msg = if map_taken {
        "\"Got the map of Silent Hill.\" (Flag 28=true, Item #19, SE 0x2b17)"
    } else {
        "Map of Silent Hill on the seat (out/p_incar_map @ -48.375, -31.625)."
    };
    draw_text_clipped(
        state,
        mx + 24,
        my + 464,
        74,
        if map_taken { GREEN } else { CYAN },
        msg,
    );
    draw_text_clipped(
        state,
        mx + 24,
        my + 486,
        74,
        WHITE,
        if map_taken {
            "Press [E / Enter / Esc] to Return to 3D Deck (or [S] Reset Flag 28)"
        } else {
            "Press [E / Enter / Space] to Take Map! (Plays SE 0x2b17 & Flag 28)"
        },
    );
    draw_progress_bar(
        state,
        mx + 24,
        my + 512,
        mw - 48,
        16,
        if map_taken { 1.0 } else { 0.0 },
        GREEN,
        CYAN,
    );
    draw_text_clipped(
        state,
        mx + 24,
        my + 534,
        74,
        MUTED,
        "[ESC] Close Overlay & Return to 3D | [PgDn] Next Story Checkpoint",
    );
}

fn draw_mary_letter_inset_over_3d(state: &mut AppState) {
    let (lx, ly, lw, lh) = (20usize, 118usize, 690usize, 505usize);
    draw_card(state, lx, ly, lw, lh, [8, 10, 18, 225], CYAN);
    let desc = state.story_progression_room.credits_seq.descriptor;
    let phase = state.story_progression_room.credits_seq.phase;
    draw_text_clipped(
        state,
        lx + 14,
        ly + 10,
        82,
        YELLOW,
        &format!(
            "MARY'S FULL EPILOGUE LETTER & CREDITS ({:?} @ {LETTER_SCROLL_SPEED_PX_PER_SEC:.1} px/s | [PgUp/PgDn] Ending):",
            desc.ending
        ),
    );
    let elapsed = match phase {
        sh2_ui::credits::CreditsSequencePhase::MaryLetterScroll { elapsed_secs, .. } => {
            elapsed_secs
        }
        _ => (state.sim_time * 14.0) % 240.0,
    };
    for (vy, entry) in visible_letter_entries_at(&LETTER_MSG_E_ENTRIES, elapsed, 440) {
        let sy = (ly + 36) as i32 + ((vy as f32) * 0.95) as i32;
        for (li, line) in entry.text.lines().enumerate() {
            let py = sy + (li as i32) * 18;
            if ((ly + 34) as i32..=(ly + lh - 46) as i32).contains(&py) {
                draw_text_clipped(state, lx + 18, py as usize, 80, WHITE, line);
            }
        }
    }
    txt_right(
        state,
        lx + 14,
        ly + lh - 24,
        GREEN,
        &format!(
            "Theme: \"{}\" | [Space] Pause Letter | [1..9] Update Ending Judge | [ESC/E] Return to 3D",
            desc.music_theme.title()
        ),
    );
}
