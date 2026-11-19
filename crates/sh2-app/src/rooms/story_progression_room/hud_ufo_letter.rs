//! Tab 4 (`[F4]`): Visual UFO 35-Slide Comic Theater & Mary's Scrolling Letter Cinema (`SH2-118`).
//!
//! Left Visual Viewport (`560x420` at `x=24, y=78`) renders the retail `data/pic/ufo/hj01..07.tex`,
//! `words01..07.tex`, and `staff01..07.tex` comic slides across all 35 `UFO_SLIDE_TABLE` entries,
//! while the Right Letter & Credits Cinema Box (`x=612..1256, y=78..498`) scrolls Mary's `.mes`
//! epilogue letter upward at `16.5` px/s inside strict bounds (`y=86..488, x=624..1244`).

use super::hud_common::{
    draw_card, draw_progress_bar, txt_left, txt_right, CYAN, GREEN, MUTED, ORANGE, WHITE, YELLOW,
};
use crate::state::AppState;
use sh2_core::endings::{
    leave_ending_cemetery_draw_distance, visible_letter_entries_at, CREDITS_BIK_DURATION_SECS,
    END_BIK_DURATION_SECS, END_DOG_BIK_DURATION_SECS, END_WISH_BIK_DURATION_SECS,
    LETTER_MSG_E_ENTRIES, LETTER_SCROLL_SPEED_PX_PER_SEC, LETTER_W0_MSG_E_ENTRIES,
    LETTER_W1_MSG_E_ENTRIES, UFO_SLIDE_TABLE, WATER_BIK_DURATION_SECS,
};

pub(crate) fn render_ufo_and_letter_tab(state: &mut AppState) {
    let slide_idx = state.story_progression_room.ufo_player.slide_index;
    let slide = state.story_progression_room.ufo_player.current_slide();
    let tex_path = state
        .story_progression_room
        .ufo_player
        .current_texture_path();
    let el = state.story_progression_room.ufo_player.slide_elapsed_secs;
    let in_staff = state
        .story_progression_room
        .ufo_player
        .is_in_staff_credits();
    let paused = state.story_progression_room.ufo_paused;

    txt_left(
        state,
        24,
        62,
        YELLOW,
        &format!(
            "UFO 35-SLIDE COMIC THEATER: Slide {:02}/35 [Space / Left / Right]",
            slide_idx + 1
        ),
    );

    // Left Visual Viewport (560x420 at x=24, y=78)
    let (vp_x, vp_y, vp_w, vp_h) = (24usize, 78usize, 560usize, 420usize);
    state
        .menu_rasterizer
        .fill_rect(vp_x - 2, vp_y - 2, vp_w + 4, vp_h + 4, [0, 0, 0, 255]);
    state
        .menu_rasterizer
        .draw_rect_outline(vp_x - 2, vp_y - 2, vp_w + 4, vp_h + 4, 2, YELLOW);

    if let Some(tex) = state.story_progression_room.ufo_tex_cache.get(tex_path) {
        let (rgba, tw, th) = (tex.rgba.clone(), tex.width, tex.height);
        state
            .menu_rasterizer
            .blit_rgba_scaled(vp_x, vp_y, vp_w, vp_h, &rgba, tw, th, 1.0);
    } else {
        txt_left(
            state,
            vp_x + 24,
            vp_y + 190,
            ORANGE,
            &format!("Loading UFO slide '{tex_path}'..."),
        );
    }
    if slide.overlay_mode > 0 {
        if let Some(beam) = state
            .story_progression_room
            .ufo_tex_cache
            .get("data/pic/ufo/hj07.tex")
        {
            let (rgba, bw, bh) = (beam.rgba.clone(), beam.width, beam.height);
            let alpha = if slide.overlay_mode == 1 { 0.65 } else { 0.90 };
            state
                .menu_rasterizer
                .blit_rgba_scaled(vp_x, vp_y, vp_w, vp_h, &rgba, bw, bh, alpha);
        }
    }

    // Visual Slide Progress Bar & Card Below Left Viewport (y=510..655)
    draw_card(
        state,
        24,
        510,
        560,
        145,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    let slide_ratio = ((slide_idx as f32) + (el / slide.duration_secs.max(0.1)).clamp(0.0, 1.0))
        / (UFO_SLIDE_TABLE.len() as f32);
    draw_progress_bar(state, 36, 522, 536, 16, slide_ratio, YELLOW, CYAN);
    txt_left(
        state,
        36,
        546,
        CYAN,
        &format!(
            "Slide {:02}/35: {tex_path} ({el:.1}s / {:.1}s) | BeamOverlay={}",
            slide_idx + 1,
            slide.duration_secs,
            slide.overlay_mode
        ),
    );
    txt_left(
        state,
        36,
        568,
        GREEN,
        &format!(
            "Section: {} | Audio Stream: 0xEAFC (voice.afs[156]) | Paused={paused}",
            if in_staff {
                "Silent Hill 1 & 2 Staff Roll (Slides 28..35)"
            } else {
                "Harry Mason & James UFO Abduction Comic (Slides 01..27)"
            }
        ),
    );
    txt_left(
        state,
        36,
        590,
        WHITE,
        &format!(
            "Dialogue Subtitle Index: Msg #{} (stage_end_recovery_msg_e.mes)",
            slide.subtitle_msg_index
        ),
    );
    txt_left(
        state,
        36,
        616,
        MUTED,
        "[Space] Play/Pause Slideshow | [Left/Right] Step Slide 1..=35",
    );

    // Right Letter & Credits Cinema Box (x=612..1256, y=78..498)
    let desc = state.story_progression_room.credits_seq.descriptor;
    let phase = state.story_progression_room.credits_seq.phase;
    txt_right(
        state,
        612,
        62,
        YELLOW,
        &format!(
            "MARY'S LETTER & CREDITS CINEMA [PgUp/PgDn]: {:?} ([Enter] Next Phase)",
            desc.ending
        ),
    );

    let (lbox_x, lbox_y, lbox_w, lbox_h) = (612usize, 78usize, 644usize, 420usize);
    state
        .menu_rasterizer
        .fill_rect(lbox_x, lbox_y, lbox_w, lbox_h, [6, 8, 14, 255]);
    state
        .menu_rasterizer
        .draw_rect_outline(lbox_x, lbox_y, lbox_w, lbox_h, 2, CYAN);

    let (entries, elapsed) = match phase {
        sh2_ui::credits::CreditsSequencePhase::MaryLetterScroll {
            entries,
            elapsed_secs,
            ..
        } => (entries, elapsed_secs),
        sh2_ui::credits::CreditsSequencePhase::PreludeFmv { elapsed_secs, .. } => {
            (LETTER_W1_MSG_E_ENTRIES, elapsed_secs)
        }
        _ => (&LETTER_MSG_E_ENTRIES[..], (state.sim_time * 12.0) % 240.0),
    };

    // Strictly clip every letter line inside y=86..488 and x=624..1244 (max 74 chars)
    for (vy, entry) in visible_letter_entries_at(entries, elapsed, 512) {
        let sy = lbox_y as i32 + ((vy as f32) * (lbox_h as f32 / 512.0)) as i32;
        for (li, line) in entry.text.lines().enumerate() {
            let py = sy + (li as i32) * 18;
            if (86..=480).contains(&py) {
                txt_right(state, 624, py as usize, WHITE, line);
            }
        }
    }

    // Bottom Card (x=612..1256, y=510..655) showing active ending theme, voiceover stream, & .bik FMV duration
    draw_card(
        state,
        612,
        510,
        644,
        145,
        [14, 18, 28, 245],
        [70, 90, 120, 255],
    );
    let draw_dist = leave_ending_cemetery_draw_distance(
        state.story_progression_room.extended_cemetery_draw_dist,
    );
    let voice_str = desc
        .letter_voice_stream_id
        .map(|id| format!("0x{id:04X}"))
        .unwrap_or_else(|| "None".to_string());
    let letter_path = desc.letter_mes_path.unwrap_or("letter_msg_e.mes");

    txt_right(
        state,
        624,
        520,
        YELLOW,
        &format!("ACTIVE ENDING: {:?} | Phase: {phase:?}", desc.ending),
    );
    txt_right(
        state,
        624,
        542,
        GREEN,
        &format!(
            "Ending Theme : \"{}\" | Voiceover Stream: {voice_str}",
            desc.music_theme.title()
        ),
    );
    txt_right(
        state,
        624,
        564,
        CYAN,
        &format!(
            "Letter Script: {letter_path} ({} entries @ {:.1} px/s)",
            LETTER_MSG_E_ENTRIES.len(),
            LETTER_SCROLL_SPEED_PX_PER_SEC
        ),
    );
    txt_right(
        state,
        624,
        586,
        WHITE,
        &format!(
            "Bink FMVs    : Epilogue={:?} (Track {}) | Credits={:?} ({CREDITS_BIK_DURATION_SECS:.1}s)",
            desc.epilogue_fmv, desc.end_bik_audio_track, desc.credits_fmv
        ),
    );
    txt_right(
        state,
        624,
        608,
        MUTED,
        &format!(
            "FMV Durations: end={END_BIK_DURATION_SECS:.1}s | water={WATER_BIK_DURATION_SECS:.1}s | dog={END_DOG_BIK_DURATION_SECS:.1}s | wish={END_WISH_BIK_DURATION_SECS:.1}s | DrawDist[V]={draw_dist:.0}mm"
        ),
    );
    txt_right(
        state,
        624,
        628,
        MUTED,
        &format!(
            "[PgUp/PgDn] Switch Ending | [Enter] Advance Phase | Tables: {}/{}/{} lines",
            LETTER_MSG_E_ENTRIES.len(),
            LETTER_W1_MSG_E_ENTRIES.len(),
            LETTER_W0_MSG_E_ENTRIES.len()
        ),
    );
}
