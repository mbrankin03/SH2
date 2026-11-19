//! Tab 3 (`[F3]`): Live Ending Point Evaluator, 10-Star Ranking & NG+ Secret Weapons (`SH2-118`)
//! and Tab 4 (`[F4]`): UFO 35-Slide Comic Player & Mary's Full `.mes` Letter Scroller + Staff Credits.

use crate::state::AppState;
use sh2_audio::chainsaw_sfx::{compute_chainsaw_wav_slice, SE_CHAINSAW_IDLE_LOOP};
use sh2_core::endings::{
    evaluate_ngplus_secret_spawns, evaluate_spray_enemy_reaction, evaluate_star_ranking,
    leave_ending_cemetery_draw_distance, visible_letter_entries_at, EndingEvalFlags,
    HyperSprayColor, SprayParticleTimingConfig, CHAINSAW_ATTACK_SPECS, CREDITS_BIK_DURATION_SECS,
    END_BIK_DURATION_SECS, END_DOG_BIK_DURATION_SECS, END_WISH_BIK_DURATION_SECS,
    LETTER_MSG_E_ENTRIES, LETTER_SCROLL_SPEED_PX_PER_SEC, LETTER_W0_MSG_E_ENTRIES,
    LETTER_W1_MSG_E_ENTRIES, REBIRTH_RITUAL_ITEM_IDS, UFO_SLIDE_TABLE, WATER_BIK_DURATION_SECS,
};

const YELLOW: [u8; 4] = [245, 210, 65, 255];
const CYAN: [u8; 4] = [105, 215, 235, 255];
const GREEN: [u8; 4] = [120, 230, 150, 255];
const WHITE: [u8; 4] = [235, 235, 235, 255];
const ORANGE: [u8; 4] = [255, 170, 80, 255];

fn txt(state: &mut AppState, x: usize, y: usize, color: [u8; 4], s: &str) {
    state.menu_rasterizer.draw_text(s, x, y, 1, color, true);
}

pub(super) fn render_endings_judge_tab(state: &mut AppState) {
    let rebirth_cnt = REBIRTH_RITUAL_ITEM_IDS
        .iter()
        .filter(|&&id| state.sim_core.scenario.inventory_mgr.has_item(id))
        .count() as u8;
    let eval_flags = EndingEvalFlags::from_event_flags(&state.sim_core.scenario.flags, rebirth_cnt);
    let report = state
        .story_progression_room
        .point_tracker
        .evaluate(&state.story_progression_room.playing_info, eval_flags);

    let lx = 22usize;
    txt(
        state,
        lx,
        74,
        YELLOW,
        "--- LIVE ENDING POINT EVALUATOR (EvProgEndingJudge @ 0x00576490) ---",
    );
    txt(
        state,
        lx,
        96,
        GREEN,
        &format!(
            "WINNING ENDING : {:?}   (Leave={} pts | Maria={} pts | In Water={} pts)",
            report.selected_ending,
            report.leave_points,
            report.maria_points,
            report.in_water_points
        ),
    );
    txt(
        state,
        lx,
        116,
        CYAN,
        "Start Base Pts : Leave=+5 (0x0057649f) | Maria=+3 (0x005764a8) | In Water=+2 (0x005764ad)",
    );

    let pt = state.story_progression_room.point_tracker.clone();
    let pi = state.story_progression_room.playing_info.clone();
    let fl_11 = state.sim_core.scenario.flags.get(11);
    let fl_516 = state.sim_core.scenario.flags.get(516);
    let fl_622 = state.sim_core.scenario.flags.get(622);
    let fl_725 = state.sim_core.scenario.flags.get(725);
    let fl_726 = state.sim_core.scenario.flags.get(726);
    let fl_727 = state.sim_core.scenario.flags.get(727);
    let prev_cs = state.story_progression_room.prevent_first_run_chainsaw;

    txt(state, lx, 140, WHITE, &format!("[1] Low-HP & In Water Flags : Weighted={:.1}s (>60/+1, >240/+1) | Diary(204)={} Tape(500)={} Neely(615)={}", pt.low_hp_weighted_secs, eval_flags.hospital_roof_diary_read, eval_flags.hotel_reading_room_tape_heard, eval_flags.neelys_bar_second_message_read));
    txt(state, lx, 162, WHITE, &format!("[2] Overheal & Item Inspect : OverhealRatio={:.2} (>2.0/+1 Leave) | MaryPhoto/Letter={}/2 | Knife(630)={}", pt.overheal_ratio_sum, pt.mary_item_examine_count, eval_flags.angelas_knife_examined));
    txt(state, lx, 184, WHITE, &format!("[3] Maria Escort & Damage   : Close(<1m)={:.0}s Far(>5m)={:.0}s Bumps={} | JmsDmg={:.0} EnDmg={:.0} S3(629)={}", pt.maria_close_secs, pt.maria_far_secs, pt.maria_bump_count, pi.mar_damage_by_jms, pi.mar_damage_by_enemy, eval_flags.hospital_s3_maria_second_check));
    txt(state, lx, 206, WHITE, &format!("[4] Room 0xba Mary Monologue: State={:?} | Flag 624={} (2400 frames / 80s, voice 0xEAD8)", pt.corridor_monologue, eval_flags.mary_corridor_monologue_complete));
    txt(state, lx, 228, CYAN, &format!("[5] Prior ClearEndKind Mask : 0x{:02X} (Leave={} Maria={} InWater={} Rebirth={} Dog={} UFO={} Wish={})", pi.clear_end_kind.0, pi.clear_end_kind.contains(0x01), pi.clear_end_kind.contains(0x02), pi.clear_end_kind.contains(0x04), pi.clear_end_kind.contains(0x08), pi.clear_end_kind.contains(0x10), pi.clear_end_kind.contains(0x20), pi.clear_end_kind.contains(0x40)));
    txt(state, lx, 250, if rebirth_cnt == 4 { GREEN } else { WHITE }, &format!("[6] Rebirth 4 Ritual Items  : {rebirth_cnt}/4 Held (74 Lost Memories, 75 Crimson Ceremony, 76 Chrism, 77 Goblet)"));
    txt(state, lx, 272, if fl_622 { GREEN } else { WHITE }, &format!("[7] Dog Ending (Item 25)    : Spawn(Flag 11)={fl_11} | Unlocked(Flag 516)={fl_516} | EndingDog(Flag 622)={fl_622}"));
    txt(state, lx, 294, if fl_727 { GREEN } else { ORANGE }, &format!("[8] Blue Gem UFO (Item 78)  : Stg22 Garden(725)={fl_725} -> Stg38 Dock(726)={fl_726} -> Stg42 Rm312 UFO(727)={fl_727}"));

    let ng = evaluate_ngplus_secret_spawns(
        pi.clear_end_kind,
        pi.clear_end_number,
        pi.battle_level,
        pi.cleared_difficulty_mask,
        prev_cs,
    );
    txt(
        state,
        lx,
        328,
        YELLOW,
        "--- NG+ SECRET SPAWN EVALUATOR (FlagInit @ 0x0048ab52..0x0048abc3) ---",
    );
    txt(
        state,
        lx,
        348,
        CYAN,
        &format!(
            "PreventChainsawSpawn[V]={prev_cs} | Chainsaw(Flag 12)={} | HyperSpray(Flag 631)={}",
            ng.spawn_chainsaw, ng.spawn_hyper_spray
        ),
    );
    txt(
        state,
        lx,
        366,
        WHITE,
        &format!(
            "DogKey(Flag 11)={} | BlueGem(Flag 723)={} | RebirthItems(Flag 10)={}",
            ng.spawn_dog_key, ng.spawn_blue_gem, ng.spawn_ngplus_bonus_items
        ),
    );

    render_ranking_and_weapons_panel(state, &pi);
}

fn render_ranking_and_weapons_panel(state: &mut AppState, pi: &sh2_core::endings::PlayingInfo) {
    let rx = 636usize;
    let ruleset = state.story_progression_room.ranking_ruleset;
    let rank = evaluate_star_ranking(pi, ruleset);
    txt(
        state,
        rx,
        74,
        YELLOW,
        &format!("--- 10-STAR RANKING (player_result.c) [G]: Ruleset={ruleset:?} ---"),
    );
    txt(
        state,
        rx,
        96,
        GREEN,
        &format!(
            "TOTAL SCORE : {}/100 ({} Big Stars + {} Small Stars) -> Hyper Spray Tier={}",
            rank.total_points,
            rank.big_stars(),
            rank.small_stars(),
            rank.spray_pow
        ),
    );
    txt(
        state,
        rx,
        116,
        WHITE,
        &format!(
            "Saves ({})={}pt | Endings={}pt | Action={}pt | Riddle={}pt | Items({}+{})={}pt",
            pi.savecount,
            rank.save_points,
            rank.ending_points,
            rank.action_level_points,
            rank.riddle_level_points,
            pi.item_get,
            pi.hidden_item_get.count_ones(),
            rank.item_points
        ),
    );
    txt(
        state,
        rx,
        134,
        WHITE,
        &format!(
            "ShotKills({})={}pt | FightKills({})={}pt | Time={}pt | Boat={}pt | Dmg={:.0}->{}pt",
            pi.kill_by_shot,
            rank.shooting_kill_points,
            pi.kill_by_fight,
            rank.fighting_kill_points,
            rank.clear_time_points,
            rank.boat_time_points,
            pi.jms_damage_total,
            rank.damage_points
        ),
    );

    let color = HyperSprayColor::from_spray_pow(pi.spray_pow);
    let reaction = evaluate_spray_enemy_reaction(pi.spray_pow, false);
    let spray_fix = state.story_progression_room.spray_60fps_fix;
    let rem_secs = state.story_progression_room.spray_can.remaining_secs;
    let p_cfg = SprayParticleTimingConfig::resolve(spray_fix);
    txt(
        state,
        rx,
        166,
        YELLOW,
        "--- HYPER SPRAY MIST TIER, CONE TAPER & SPRAYEFFECT.CPP ([H] / [V]) ---",
    );
    txt(state, rx, 186, CYAN, &format!("Active Mist [H] : {color:?} (spray_pow={}, ABGR=0x{:08X}) | EnemyReaction={reaction:?}", pi.spray_pow, color.packed_abgr()));
    let rgba = color.rgba();
    state
        .menu_rasterizer
        .fill_rect(rx, 204, 80, 18, [rgba[0], rgba[1], rgba[2], 255]);
    state
        .menu_rasterizer
        .draw_rect_outline(rx, 204, 80, 18, 1, WHITE);
    txt(state, rx + 90, 206, WHITE, &format!("Canister={rem_secs:.1}/20.0s (Taper<10s) | SprayEffect60FPS[V]={spray_fix} (spd={:.2}/{:.2}, y={:.0}, stride={})", p_cfg.speed_factor_state_0_1, p_cfg.speed_factor_state_2, p_cfg.y_adjust, p_cfg.spawn_frame_stride));

    let cs_idx = state.story_progression_room.chainsaw_attack_idx;
    let cs_fix = state.story_progression_room.chainsaw_fix_enabled;
    let cs_state = state.story_progression_room.chainsaw_sfx.idle_state;
    let cs_out = state.story_progression_room.last_chainsaw_audio;
    txt(
        state,
        rx,
        242,
        YELLOW,
        "--- CHAINSAW ATTACKS (0x00629000 [19..=23]) & CHAINSAW_SFX ([C] / [V]) ---",
    );
    for (i, sp) in CHAINSAW_ATTACK_SPECS.iter().enumerate() {
        let sel = i == cs_idx;
        txt(
            state,
            rx,
            260 + i * 16,
            if sel { GREEN } else { WHITE },
            &format!(
                "{} Attack #{:02}: AP={:4.0} range=0..{:.0}mm frames={:02}..{:02} sfx=0x{:04X}",
                if sel { ">>" } else { "  " },
                sp.id,
                sp.ap,
                sp.max_range,
                sp.first_hit_frame,
                sp.last_hit_frame,
                sp.sfx_id
            ),
        );
    }
    let slice_48k =
        compute_chainsaw_wav_slice(SE_CHAINSAW_IDLE_LOOP, 48_000, 0x30000, false, cs_fix);
    txt(
        state,
        rx,
        346,
        CYAN,
        &format!(
            "ChainsawSoundFix[V]={cs_fix} | IdleState={cs_state:?} | 48kHz 0x1642 start=0x{:X}",
            slice_48k.pcm_start_byte_offset
        ),
    );
    if let Some(out) = cs_out {
        txt(state, rx, 364, ORANGE, &format!("Mixer Output : start(0x2B24)={:.2} loop(0x1642)={:.2} atk(0x2B25)={:?} hit(0x2B26)={:?}", out.idle_start_volume, out.idle_loop_volume, out.trigger_attack_sfx_volume, out.trigger_hit_sfx_volume));
    }
}

pub(super) fn render_ufo_and_letter_tab(state: &mut AppState) {
    let lx = 22usize;
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

    txt(
        state,
        lx,
        74,
        YELLOW,
        &format!(
            "--- UFO 35-SLIDE COMIC PLAYER (0x008c9490): Slide {:02}/35 [Left/Right/Space] ---",
            slide_idx + 1
        ),
    );
    txt(
        state,
        lx,
        92,
        CYAN,
        &format!(
            "Texture[{}]: {tex_path} | Dur={:.1}s (Elapsed={el:.1}s) | BeamOverlay={} | SubMsg={}",
            slide.texture_index, slide.duration_secs, slide.overlay_mode, slide.subtitle_msg_index
        ),
    );

    let (vp_x, vp_y, vp_w, vp_h) = (24usize, 114usize, 560usize, 400usize);
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
        txt(
            state,
            vp_x + 40,
            vp_y + 180,
            ORANGE,
            &format!("(Retail UFO texture '{tex_path}' not found on disk)"),
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
    txt(state, lx, 524, GREEN, &format!("Total Slides={} | StaffRoll(27..34)={in_staff} | Stream=0xEAFC (voice.afs[156]) | Paused={paused}", UFO_SLIDE_TABLE.len()));

    let rx = 620usize;
    let desc = state.story_progression_room.credits_seq.descriptor;
    let phase = state.story_progression_room.credits_seq.phase;
    let trig_voice = state
        .story_progression_room
        .credits_seq
        .last_triggered_voice_stream;
    let draw_dist = leave_ending_cemetery_draw_distance(
        state.story_progression_room.extended_cemetery_draw_dist,
    );

    txt(
        state,
        rx,
        74,
        YELLOW,
        &format!(
            "--- EPILOGUE LETTER & STAFF CREDITS [PgUp/PgDn]: {:?} ([Enter] Next Phase) ---",
            desc.ending
        ),
    );
    txt(
        state,
        rx,
        92,
        CYAN,
        &format!(
            "Phase={phase:?} | Theme=\"{}\" | LeaveDrawDist[V]={draw_dist:.0}mm",
            desc.music_theme.title()
        ),
    );
    txt(
        state,
        rx,
        110,
        WHITE,
        &format!(
            "Letter .mes={:?} (35/33/35 lines @ {:.1}px/s) | VoiceStream={:?} (Triggered={:?})",
            desc.letter_mes_path,
            LETTER_SCROLL_SPEED_PX_PER_SEC,
            desc.letter_voice_stream_id.map(|id| format!("0x{id:04X}")),
            trig_voice.map(|id| format!("0x{id:04X}"))
        ),
    );
    txt(state, rx, 128, WHITE, &format!("Bink FMVs: epilogue={:?} (track {}) | credits={:?} ({CREDITS_BIK_DURATION_SECS:.2}s) | end={END_BIK_DURATION_SECS:.2}s | water={WATER_BIK_DURATION_SECS:.2}s | dog={END_DOG_BIK_DURATION_SECS:.2}s | wish={END_WISH_BIK_DURATION_SECS:.2}s", desc.epilogue_fmv, desc.end_bik_audio_track, desc.credits_fmv));

    let (lbox_x, lbox_y, lbox_w, lbox_h) = (620usize, 150usize, 630usize, 364usize);
    state
        .menu_rasterizer
        .fill_rect(lbox_x, lbox_y, lbox_w, lbox_h, [12, 14, 22, 255]);
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
        _ => (&LETTER_MSG_E_ENTRIES[..], (state.sim_time * 1.8) % 240.0),
    };
    for (vy, entry) in visible_letter_entries_at(entries, elapsed, 512) {
        let sy = lbox_y as i32 + ((vy as f32) * (lbox_h as f32 / 512.0)) as i32;
        for (li, line) in entry.text.lines().enumerate() {
            let py = sy + (li as i32) * 18;
            if py >= (lbox_y as i32 + 6) && py <= (lbox_y + lbox_h - 20) as i32 {
                txt(
                    state,
                    lbox_x + 16,
                    py as usize,
                    WHITE,
                    &format!("[+{}px] {line}", entry.f8_spacing_px),
                );
            }
        }
    }
    txt(state, rx, 524, GREEN, &format!("Letter Tables: letter_msg_e={} entries | letter_w1_msg_e={} entries | letter_w0_msg_e={} entries", LETTER_MSG_E_ENTRIES.len(), LETTER_W1_MSG_E_ENTRIES.len(), LETTER_W0_MSG_E_ENTRIES.len()));
}
