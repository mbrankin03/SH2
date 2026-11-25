//! Unified Interactive Story Stages (1–58), Visual Cutaway Riddles, Playable 3D Arena + Live
//! Ending Point Judge, 10-Star Ranking, Secret NG+ Weapons, UFO 35-Slide Comic Theater & Mary's
//! `.mes` Epilogue Letter Cinema (`SH2-65`).

pub(crate) mod hud_act1;
pub(crate) mod hud_act1_panel;
pub(crate) mod hud_act2;
pub(crate) mod hud_act2_panel;
pub(crate) mod hud_common;
pub(crate) mod hud_endings;
pub(crate) mod hud_ufo_letter;
pub(crate) mod input;
pub(crate) mod input_act1;
pub(crate) mod input_act2;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::{CutawayTextureAsset, DecodedTexture, TextureLoader};
use sh2_audio::chainsaw_sfx::{ChainsawAudioOutput, ChainsawSoundController, ChainsawTickInput};
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::endings::{
    EndingPointTracker, GameEnding, HyperSprayCanState, HyperSprayTickResult, PlayingInfo,
    RankingRuleset, UfoSlideShowPlayer,
};
use sh2_core::events::stages_act1::{
    act1_step_elevator_radio_quiz, Act1HospitalWingKeypad, Act1LouiseBoxState,
    Act1VanillaFixesConfig,
};
use sh2_core::events::stages_act2::{
    init_briefcase_drums_if_needed, Act2EnhancementToggles, BoardRotation, ElevatorCupboardState,
    GravestoneBoard, HotelMusicBoxPuzzle, WishGravestonePuzzle, ITEM_BLACK_BOARD, ITEM_RED_BOARD,
    ITEM_WHITE_BOARD,
};
use sh2_core::{init_new_game, ClearData, Sh2Rng};
use sh2_ui::credits::EndingCreditsSequence;

/// Active sub-studio tab in [`StoryProgressionRoomController`] (`[F1]`–`[F4]` or `[Tab]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StoryDemoTab {
    #[default]
    Act1StagesAndRiddles,
    Act2StagesAndRiddles,
    EndingJudgeAndRanking,
    UfoAndMaryLetterCredits,
}

impl StoryDemoTab {
    pub const fn next(self) -> Self {
        match self {
            Self::Act1StagesAndRiddles => Self::Act2StagesAndRiddles,
            Self::Act2StagesAndRiddles => Self::EndingJudgeAndRanking,
            Self::EndingJudgeAndRanking => Self::UfoAndMaryLetterCredits,
            Self::UfoAndMaryLetterCredits => Self::Act1StagesAndRiddles,
        }
    }
}

/// Selectable visual cutaway puzzles in Tab 1 (`[1]`–`[4]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Act1VisualPuzzle {
    #[default]
    SafeDial,
    CoinDesk,
    LouiseBox,
    QuizChest,
}

/// Selectable visual cutaway puzzles in Tab 2 (`[1]`–`[4]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Act2VisualPuzzle {
    #[default]
    WishGravestone,
    HotelBriefcase,
    HotelEggsAndVcr,
    LabyrinthAndGallows,
}

/// Canonical 9 retail `trg == 10` (`UseOrActionCarryingItem`, `sh2pc.exe` `0x0048e1c1` & `0x0048e1ee`)
/// key-locked doors across Act 2 & *Born from a Wish* (`(stage_id, ev_index, key_item_id, label)`).
pub(crate) const ACT2_TRG10_KEY_DOORS: [(u16, usize, u8, &str); 9] = [
    (26, 12, 38, "Storeroom Door (Key #38)"),
    (39, 6, 46, "Bar Venus Tears Door (Key #46, Lit)"),
    (39, 7, 46, "Bar Venus Tears Door (Key #46, Dark)"),
    (41, 27, 44, "Hotel Rm 204 Door (Key #44)"),
    (41, 53, 45, "Employee Elevator Door (Key #45)"),
    (42, 6, 48, "Hotel 3F Stairway Gate (Key #48)"),
    (42, 10, 43, "Hotel Rm 312 Door (Key #43)"),
    (46, 8, 25, "Observation Room (Dog Key #25)"),
    (56, 33, 82, "Baldwin Mansion Acacia Door (Key #82)"),
];

/// Runtime state for [`sh2_ui::menu::TestRoomKind::StoryStagesAndEndingsRoom`] (`SH2-65`).
pub(crate) struct StoryProgressionRoomState {
    pub active_tab: StoryDemoTab,
    pub seed_counter: u32,
    pub rng: Sh2Rng,
    pub cutaway_cache: HashMap<(String, sh2_core::CutawayQuality), CutawayTextureAsset>,
    pub prefer_sh2ee_cutaways: bool,
    pub act1_puzzle: Act1VisualPuzzle,
    pub act1_stage_idx: usize,
    pub act1_fixes: Act1VanillaFixesConfig,
    pub act1_coin_slot: usize,
    pub act1_louise_wheel_idx: usize,
    pub act1_show_wall_blood_still: bool,
    pub act1_quiz_row: usize,
    pub act1_wing_keypad: Act1HospitalWingKeypad,
    pub act1_louise_box: Act1LouiseBoxState,
    pub act1_radio_quiz_ticks: f32,
    pub act1_james_hp: f32,
    pub act1_poison_flash_timer: f32,
    pub act1_ph_hp_ratio: f32,
    pub act1_ph_zone: u8,
    pub act2_puzzle: Act2VisualPuzzle,
    pub act2_stage_idx: usize,
    pub act2_toggles: Act2EnhancementToggles,
    pub act2_drum_idx: usize,
    pub act2_show_marker_still: bool,
    pub act2_show_vcr_still: bool,
    pub act2_show_manacles_still: bool,
    pub act2_hangman_selected_slot: usize,
    pub act2_trg10_door_idx: usize,
    pub act2_carrying_key: bool,
    pub act2_trg10_unlocked: Option<bool>,
    pub act2_trg10_last_result: String,
    pub act2_music_box: HotelMusicBoxPuzzle,
    pub act2_cupboard: ElevatorCupboardState,
    pub act2_gravestone: WishGravestonePuzzle,
    pub act2_gravestone_rotations: [BoardRotation; 3],
    pub act2_corridor_room_idx: usize,
    pub playing_info: PlayingInfo,
    pub point_tracker: EndingPointTracker,
    pub ranking_ruleset: RankingRuleset,
    pub prevent_first_run_chainsaw: bool,
    pub spray_60fps_fix: bool,
    pub spray_can: HyperSprayCanState,
    pub last_spray_tick: Option<HyperSprayTickResult>,
    pub chainsaw_fix_enabled: bool,
    pub chainsaw_attack_idx: usize,
    pub chainsaw_sfx: ChainsawSoundController,
    pub last_chainsaw_audio: Option<ChainsawAudioOutput>,
    pub blue_gem_step: u8,
    pub ufo_player: UfoSlideShowPlayer,
    pub ufo_paused: bool,
    pub ufo_tex_cache: HashMap<&'static str, DecodedTexture>,
    pub selected_credits_ending_idx: usize,
    pub credits_seq: EndingCreditsSequence,
    pub credits_paused: bool,
    pub extended_cemetery_draw_dist: bool,
    pub status_banner: String,
    /// Current 3D story progression checkpoint index (`0..12`, stepped with `[PgUp]` / `[PgDn]` in 3D gameplay).
    pub checkpoint_3d_idx: usize,
    /// Whether the interactive `.tex` cutaway / UFO / Ending Judge modal is currently popped up over the 3D world (`[E]` / `[Enter]` / `[ESC]`).
    pub modal_3d_open: bool,
    /// Whether James has inspected the dirty restroom mirror in `RestroomStart` (`er01.map`) before exiting.
    pub restroom_mirror_inspected: bool,
}

impl Default for StoryProgressionRoomState {
    fn default() -> Self {
        let mut s = Self {
            active_tab: StoryDemoTab::Act1StagesAndRiddles,
            seed_counter: 1,
            rng: Sh2Rng::new(0x2001_0924),
            cutaway_cache: HashMap::new(),
            prefer_sh2ee_cutaways: true,
            act1_puzzle: Act1VisualPuzzle::SafeDial,
            act1_stage_idx: 10,
            act1_fixes: Act1VanillaFixesConfig::enhanced(),
            act1_coin_slot: 0,
            act1_louise_wheel_idx: 0,
            act1_show_wall_blood_still: false,
            act1_quiz_row: 0,
            act1_wing_keypad: Act1HospitalWingKeypad::new(),
            act1_louise_box: Act1LouiseBoxState::new(),
            act1_radio_quiz_ticks: 240.0,
            act1_james_hp: 100.0,
            act1_poison_flash_timer: 0.0,
            act1_ph_hp_ratio: 1.0,
            act1_ph_zone: 1,
            act2_puzzle: Act2VisualPuzzle::WishGravestone,
            act2_stage_idx: 16,
            act2_toggles: Act2EnhancementToggles::enhanced(),
            act2_drum_idx: 0,
            act2_show_marker_still: false,
            act2_show_vcr_still: false,
            act2_show_manacles_still: false,
            act2_hangman_selected_slot: 0,
            act2_trg10_door_idx: 0,
            act2_carrying_key: true,
            act2_trg10_unlocked: Some(true),
            act2_trg10_last_result:
                "READY: Press [A] Action while carrying key or [U] Inventory Use".to_string(),
            act2_music_box: HotelMusicBoxPuzzle::new(),
            act2_cupboard: ElevatorCupboardState::new(),
            act2_gravestone: WishGravestonePuzzle::new(),
            act2_gravestone_rotations: [
                BoardRotation::Right90,
                BoardRotation::Deg0,
                BoardRotation::Left90,
            ],
            act2_corridor_room_idx: 0,
            playing_info: PlayingInfo::default(),
            point_tracker: EndingPointTracker::default(),
            ranking_ruleset: RankingRuleset::PcDirectorCut,
            prevent_first_run_chainsaw: true,
            spray_60fps_fix: true,
            spray_can: HyperSprayCanState::new(),
            last_spray_tick: None,
            chainsaw_fix_enabled: true,
            chainsaw_attack_idx: 0,
            chainsaw_sfx: ChainsawSoundController::new(),
            last_chainsaw_audio: None,
            blue_gem_step: 0,
            ufo_player: UfoSlideShowPlayer::new(),
            ufo_paused: false,
            ufo_tex_cache: HashMap::new(),
            selected_credits_ending_idx: 0,
            credits_seq: EndingCreditsSequence::start(GameEnding::Leave),
            credits_paused: false,
            extended_cemetery_draw_dist: true,
            status_banner:
                "Stage 2 (er01.map): Press [E] to Inspect Mirror / Exit Restroom -> Observation Deck | [PgUp/PgDn] 12 Story Checkpoints".to_string(),
            checkpoint_3d_idx: 0,
            modal_3d_open: false,
            restroom_mirror_inspected: false,
        };
        s.playing_info.time = 7_200.0;
        s.playing_info.boat_clear_time = 75.0;
        s.playing_info.item_get = 160;
        s.playing_info.hidden_item_get = 0b0111_1111;
        s.playing_info.kill_by_shot = 75;
        s.playing_info.kill_by_fight = 75;
        s.playing_info.jms_damage_total = 320.0;
        s
    }
}

pub(crate) struct StoryProgressionRoomController;

impl RoomController for StoryProgressionRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        input::handle_story_room_key_input(state, key_code, pressed, repeat);
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        dt_sec: f32,
        aspect: f32,
        grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        ensure_demo_scenario_seeded(state);

        if !is_paused {
            if state.story_progression_room.act1_poison_flash_timer > 0.0 {
                state.story_progression_room.act1_poison_flash_timer =
                    (state.story_progression_room.act1_poison_flash_timer - dt_sec).max(0.0);
            }
            if state.story_progression_room.active_tab == StoryDemoTab::Act1StagesAndRiddles
                && state.story_progression_room.act1_puzzle == Act1VisualPuzzle::QuizChest
            {
                let rm = &mut state.story_progression_room;
                rm.act1_radio_quiz_ticks += dt_sec * 30.0;
                if rm.act1_radio_quiz_ticks >= 5150.0 {
                    rm.act1_radio_quiz_ticks = 218.0;
                }
                let _ = act1_step_elevator_radio_quiz(
                    rm.act1_radio_quiz_ticks,
                    false,
                    &mut state.sim_core.scenario.flags,
                );
            }
        }

        if state.story_progression_room.active_tab == StoryDemoTab::EndingJudgeAndRanking {
            if !is_paused {
                let rm = &mut state.story_progression_room;
                if !matches!(
                    rm.chainsaw_sfx.idle_state,
                    sh2_audio::chainsaw_sfx::ChainsawIdleState::Inactive
                ) {
                    rm.last_chainsaw_audio = Some(rm.chainsaw_sfx.tick(ChainsawTickInput {
                        dt_secs: dt_sec,
                        james_pos_mm: state.sim_core.scenario.player_position,
                        weapon_volume: 0.85,
                        attack_active: false,
                        current_anim_frame: 1,
                        anim_speed_q11: 2048,
                        hit_this_frame: false,
                        enable_chainsaw_sound_fix: rm.chainsaw_fix_enabled,
                    }));
                }
            }
            CombatArenaRoomController::tick_and_render_3d(
                state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
            );
            return;
        }

        if !is_paused
            && state.story_progression_room.active_tab == StoryDemoTab::UfoAndMaryLetterCredits
        {
            if !state.story_progression_room.ufo_paused {
                state.story_progression_room.ufo_player.tick(dt_sec, false);
            }
            if !state.story_progression_room.credits_paused {
                state.story_progression_room.credits_seq.tick(dt_sec, false);
            }
            let tex_path = state
                .story_progression_room
                .ufo_player
                .current_texture_path();
            ensure_ufo_texture_cached(&mut state.story_progression_room, tex_path);
            ensure_ufo_texture_cached(&mut state.story_progression_room, "data/pic/ufo/hj07.tex");
        }

        state.render_ctx.render_scene(
            Some(&state.gpu_map),
            Some(&state.map_texture_bind_groups),
            &[],
        );
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        state.begin_hud_overlay();
        hud_common::draw_header_and_tabs(state);
        match state.story_progression_room.active_tab {
            StoryDemoTab::Act1StagesAndRiddles => hud_act1::render_act1_tab(state),
            StoryDemoTab::Act2StagesAndRiddles => hud_act2::render_act2_tab(state),
            StoryDemoTab::EndingJudgeAndRanking => hud_endings::render_endings_judge_tab(state),
            StoryDemoTab::UfoAndMaryLetterCredits => {
                hud_ufo_letter::render_ufo_and_letter_tab(state)
            }
        }
        state.finish_hud_overlay(encoder, surface_view);
    }
}

pub(crate) fn ensure_demo_scenario_seeded(state: &mut AppState) {
    // Never pollute a real Campaign playthrough with debug-room puzzle boards or mid-game flags.
    if matches!(
        state.active_launch_target,
        sh2_ui::menu::LaunchTarget::Campaign { .. }
    ) {
        return;
    }
    if state.sim_core.scenario.flags.get(405) {
        return;
    }
    let mut rng = Sh2Rng::new(0x2001_0924);
    let _ = init_new_game(
        &mut state.sim_core.scenario.flags,
        &mut state.sim_core.scenario.puzzle_state,
        &mut rng,
        state.sim_core.scenario.riddle_difficulty,
        ActionDifficulty::Normal,
        ClearData::default(),
    );
    init_briefcase_drums_if_needed(
        &mut state.sim_core.scenario.flags,
        &mut state.sim_core.scenario.puzzle_state,
        &mut rng,
    );
    for f in [126, 133, 140, 234, 237, 238, 239, 380, 381, 388] {
        state.sim_core.scenario.flags.set(f, true);
    }
    for id in [ITEM_WHITE_BOARD, ITEM_BLACK_BOARD, ITEM_RED_BOARD] {
        state.sim_core.scenario.inventory_mgr.give_raw(id, 1);
    }
    for (b, rot) in [
        (GravestoneBoard::White, BoardRotation::Right90),
        (GravestoneBoard::Black, BoardRotation::Deg0),
        (GravestoneBoard::Red, BoardRotation::Left90),
    ] {
        let _ = state.story_progression_room.act2_gravestone.place_board(
            b,
            rot,
            &mut state.sim_core.scenario.flags,
            &mut state.sim_core.scenario.inventory_mgr,
        );
    }
}

pub(crate) fn ensure_ufo_texture_cached(
    rm: &mut StoryProgressionRoomState,
    rel_path: &'static str,
) {
    if rm.ufo_tex_cache.contains_key(rel_path) {
        return;
    }
    let stripped = rel_path.strip_prefix("data/").unwrap_or(rel_path);
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.clone(),
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    for base in &base_roots {
        let loader = TextureLoader::new(base.join("data"), Some(base.join("sh2e")));
        if let Ok(tex) = loader.load_texture(stripped) {
            rm.ufo_tex_cache.insert(rel_path, tex);
            return;
        }
    }
}

pub(crate) fn play_cursor_se(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
    }
}

pub(crate) fn play_confirm_se(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
    }
}

pub(crate) fn cycle_riddle_difficulty(d: RiddleDifficulty) -> RiddleDifficulty {
    match d {
        RiddleDifficulty::Easy => RiddleDifficulty::Normal,
        RiddleDifficulty::Normal => RiddleDifficulty::Hard,
        RiddleDifficulty::Hard => RiddleDifficulty::Extra,
        RiddleDifficulty::Extra => RiddleDifficulty::Easy,
    }
}
