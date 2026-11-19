//! Keyboard controls for [`super::StoryProgressionRoomController`] (`SH2-65`).

use winit::keyboard::KeyCode;

use super::input_stages::{handle_act1_key, handle_act2_key};
use super::{cycle_riddle_difficulty, play_confirm_se, play_cursor_se, StoryDemoTab};
use crate::state::AppState;
use sh2_audio::chainsaw_sfx::ChainsawTickInput;
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::endings::{
    try_trigger_dog_ending, use_blue_gem, ClearEndKind, GameEnding, RankingRuleset,
    CHAINSAW_ATTACK_SPECS, FLAG_ANGELAS_KNIFE_EXAMINED, FLAG_HOSPITAL_ROOF_DIARY,
    FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK, FLAG_HOTEL_READING_ROOM_TAPE,
    FLAG_LABYRINTH_MARIA_REVISIT, FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE,
    FLAG_NEELYS_BAR_SECOND_MESSAGE, FLAG_SPAWN_BLUE_GEM, FLAG_SPAWN_DOG_KEY, ITEM_LETTER_FROM_MARY,
    ITEM_PHOTO_OF_MARY, REBIRTH_RITUAL_ITEM_IDS, ROOM_DARK_HOSPITAL_GARDEN, ROOM_HOTEL_312,
    ROOM_HOTEL_3F_OBSERVATION_CORRIDOR, ROOM_HOTEL_CORRIDOR_MONOLOGUE, ROOM_TOLUCA_LAKE_DOCK,
};
use sh2_core::events::stages_act2::init_briefcase_drums_if_needed;
use sh2_core::{init_new_game, ClearData, Sh2Rng};
use sh2_ui::credits::EndingCreditsSequence;
use sh2_ui::menu::TitlePhase;

pub(super) fn handle_story_room_key_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) {
    if !pressed || repeat {
        return;
    }
    if matches!(key_code, KeyCode::Escape | KeyCode::Backspace) {
        state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
        return;
    }
    if let Some(tab) = match key_code {
        KeyCode::F1 => Some(StoryDemoTab::Act1StagesAndRiddles),
        KeyCode::F2 => Some(StoryDemoTab::Act2StagesAndRiddles),
        KeyCode::F3 => Some(StoryDemoTab::EndingJudgeAndRanking),
        KeyCode::F4 => Some(StoryDemoTab::UfoAndMaryLetterCredits),
        KeyCode::Tab => Some(state.story_progression_room.active_tab.next()),
        _ => None,
    } {
        state.story_progression_room.active_tab = tab;
        play_cursor_se(state);
        return;
    }
    if key_code == KeyCode::KeyR {
        reroll_riddle_difficulty(state);
        return;
    }

    match state.story_progression_room.active_tab {
        StoryDemoTab::Act1StagesAndRiddles => handle_act1_key(state, key_code),
        StoryDemoTab::Act2StagesAndRiddles => handle_act2_key(state, key_code),
        StoryDemoTab::EndingJudgeAndRanking => handle_endings_key(state, key_code),
        StoryDemoTab::UfoAndMaryLetterCredits => handle_ufo_letter_key(state, key_code),
    }
}

fn reroll_riddle_difficulty(state: &mut AppState) {
    let next = cycle_riddle_difficulty(state.sim_core.scenario.riddle_difficulty);
    let rm = &mut state.story_progression_room;
    rm.seed_counter = rm.seed_counter.wrapping_add(1);
    rm.rng = Sh2Rng::new(0x2001_0924u32.wrapping_add(rm.seed_counter * 7919));
    let mut clear = ClearData::default();
    let req = if next == RiddleDifficulty::Extra {
        clear.cleared_levels = ClearData::ALL_BASE_RIDDLES_CLEARED;
        clear.endings_seen =
            ClearData::ENDING_LEAVE | ClearData::ENDING_IN_WATER | ClearData::ENDING_MARIA;
        RiddleDifficulty::Hard
    } else {
        next
    };
    let eff = init_new_game(
        &mut state.sim_core.scenario.flags,
        &mut state.sim_core.scenario.puzzle_state,
        &mut rm.rng,
        req,
        ActionDifficulty::Normal,
        clear,
    );
    init_briefcase_drums_if_needed(
        &mut state.sim_core.scenario.flags,
        &mut state.sim_core.scenario.puzzle_state,
        &mut rm.rng,
    );
    state.sim_core.scenario.riddle_difficulty = eff;
    rm.playing_info.riddle_level = eff as u8;
    rm.status_banner = format!(
        "Cycled Riddle Difficulty -> {eff:?} (Seed #{})",
        rm.seed_counter
    );
    play_confirm_se(state);
}

fn handle_endings_key(state: &mut AppState, key_code: KeyCode) {
    let s = &mut state.sim_core.scenario;
    let rm = &mut state.story_progression_room;
    match key_code {
        KeyCode::Digit1 => {
            rm.point_tracker.tick_low_hp(10.0, 100.0, 140.0);
            for f in [
                FLAG_HOSPITAL_ROOF_DIARY,
                FLAG_HOTEL_READING_ROOM_TAPE,
                FLAG_NEELYS_BAR_SECOND_MESSAGE,
            ] {
                s.flags.set(f, true);
            }
            play_confirm_se(state);
        }
        KeyCode::Digit2 => {
            let _ = rm.point_tracker.record_heal(80.0, 150.0, 100.0);
            for id in [ITEM_PHOTO_OF_MARY, ITEM_LETTER_FROM_MARY, 24] {
                rm.point_tracker.record_item_examined(id);
            }
            s.flags.set(FLAG_ANGELAS_KNIFE_EXAMINED, true);
            play_confirm_se(state);
        }
        KeyCode::Digit3 => {
            rm.point_tracker.tick_maria_escort(400.0, false, 650.0);
            s.flags.set(FLAG_LABYRINTH_MARIA_REVISIT, true);
            s.flags.set(FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK, true);
            play_confirm_se(state);
        }
        KeyCode::Digit4 => {
            let _ = rm
                .point_tracker
                .tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 90.0);
            s.flags.set(FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE, true);
            play_confirm_se(state);
        }
        KeyCode::Digit5 => {
            let next = match rm.playing_info.clear_end_kind.0 {
                0x00 => ClearEndKind::LEAVE,
                0x01 => ClearEndKind::LEAVE | ClearEndKind::MARIA,
                0x03 => ClearEndKind::LEAVE | ClearEndKind::MARIA | ClearEndKind::IN_WATER,
                0x07 => 0x7F,
                _ => 0x00,
            };
            rm.playing_info.clear_end_kind = ClearEndKind(next);
            rm.playing_info.clear_end_number = rm.playing_info.clear_end_kind.base_endings_count();
            rm.playing_info.clearcount = u16::from(rm.playing_info.clear_end_number);
            rm.playing_info.cleared_difficulty_mask = if next != 0 { 0xC0 } else { 0 };
            play_confirm_se(state);
        }
        KeyCode::Digit6 => {
            let has_all = REBIRTH_RITUAL_ITEM_IDS
                .iter()
                .all(|&id| s.inventory_mgr.has_item(id));
            for id in REBIRTH_RITUAL_ITEM_IDS {
                if has_all {
                    s.inventory_mgr.item_lose(id);
                } else {
                    s.inventory_mgr.give_raw(id, 1);
                }
            }
            play_confirm_se(state);
        }
        KeyCode::Digit7 => {
            s.flags.set(FLAG_SPAWN_DOG_KEY, true);
            let _ =
                try_trigger_dog_ending(46, ROOM_HOTEL_3F_OBSERVATION_CORRIDOR, true, &mut s.flags);
            play_confirm_se(state);
        }
        KeyCode::Digit8 => {
            s.flags.set(FLAG_SPAWN_BLUE_GEM, true);
            s.flags.set(476, false);
            let (stg, room) = match rm.blue_gem_step % 3 {
                0 => {
                    for f in [725, 726, 727] {
                        s.flags.set(f, false);
                    }
                    (22, ROOM_DARK_HOSPITAL_GARDEN)
                }
                1 => (38, ROOM_TOLUCA_LAKE_DOCK),
                _ => (42, ROOM_HOTEL_312),
            };
            let _ = use_blue_gem(stg, room, true, &mut s.flags);
            rm.blue_gem_step = (rm.blue_gem_step + 1) % 3;
            play_confirm_se(state);
        }
        KeyCode::KeyG => {
            rm.ranking_ruleset = match rm.ranking_ruleset {
                RankingRuleset::PcDirectorCut => RankingRuleset::Ps2Original,
                RankingRuleset::Ps2Original => RankingRuleset::PcDirectorCut,
            };
            rm.playing_info.savecount = if rm.playing_info.savecount == 0 {
                12
            } else {
                0
            };
            play_cursor_se(state);
        }
        KeyCode::KeyH => {
            rm.playing_info.spray_pow = if rm.playing_info.spray_pow >= 2 {
                -1
            } else {
                rm.playing_info.spray_pow + 1
            };
            let mut hp = 100.0;
            let tick = rm.spray_can.tick_spray(
                2.5,
                rm.playing_info.spray_pow,
                rm.playing_info.battle_level,
                &mut hp,
                100.0,
            );
            if tick.triggered_james_cough {
                rm.spray_can.refill();
            }
            rm.last_spray_tick = Some(tick);
            play_confirm_se(state);
        }
        KeyCode::KeyC => {
            rm.chainsaw_attack_idx = (rm.chainsaw_attack_idx + 1) % CHAINSAW_ATTACK_SPECS.len();
            if matches!(
                rm.chainsaw_sfx.idle_state,
                sh2_audio::chainsaw_sfx::ChainsawIdleState::Inactive
            ) {
                rm.chainsaw_sfx.start_idle(rm.chainsaw_fix_enabled);
            }
            rm.last_chainsaw_audio = Some(rm.chainsaw_sfx.tick(ChainsawTickInput {
                dt_secs: 0.55,
                james_pos_mm: glam::Vec3::ZERO,
                weapon_volume: 0.85,
                attack_active: true,
                current_anim_frame: 0,
                anim_speed_q11: 2048,
                hit_this_frame: true,
                enable_chainsaw_sound_fix: rm.chainsaw_fix_enabled,
            }));
            play_confirm_se(state);
        }
        KeyCode::KeyV => {
            rm.prevent_first_run_chainsaw = !rm.prevent_first_run_chainsaw;
            rm.spray_60fps_fix = !rm.spray_60fps_fix;
            rm.chainsaw_fix_enabled = !rm.chainsaw_fix_enabled;
            play_confirm_se(state);
        }
        _ => {}
    }
}

fn handle_ufo_letter_key(state: &mut AppState, key_code: KeyCode) {
    let rm = &mut state.story_progression_room;
    const ALL_ENDINGS: [GameEnding; 7] = [
        GameEnding::Leave,
        GameEnding::Maria,
        GameEnding::InWater,
        GameEnding::Rebirth,
        GameEnding::Dog,
        GameEnding::Ufo,
        GameEnding::BornFromAWish,
    ];
    match key_code {
        KeyCode::Space => {
            rm.ufo_paused = !rm.ufo_paused;
            rm.credits_paused = rm.ufo_paused;
            play_confirm_se(state);
        }
        KeyCode::ArrowRight | KeyCode::ArrowLeft => {
            let step = if key_code == KeyCode::ArrowRight {
                1
            } else {
                34
            };
            rm.ufo_player.slide_index = (rm.ufo_player.slide_index + step) % 35;
            rm.ufo_player.slide_elapsed_secs = 0.0;
            rm.ufo_player.finished = false;
            play_cursor_se(state);
        }
        KeyCode::PageDown | KeyCode::PageUp => {
            let step = if key_code == KeyCode::PageDown {
                1
            } else {
                ALL_ENDINGS.len() - 1
            };
            rm.selected_credits_ending_idx =
                (rm.selected_credits_ending_idx + step) % ALL_ENDINGS.len();
            rm.credits_seq =
                EndingCreditsSequence::start(ALL_ENDINGS[rm.selected_credits_ending_idx]);
            play_confirm_se(state);
        }
        KeyCode::Enter => {
            rm.credits_seq.tick(0.0, true);
            play_confirm_se(state);
        }
        KeyCode::KeyV => {
            rm.extended_cemetery_draw_dist = !rm.extended_cemetery_draw_dist;
            play_confirm_se(state);
        }
        _ => {}
    }
}
