//! Unified Interactive Story Stages (1–58), Riddles, Ending Point Judge, 10-Star Ranking,
//! Secret NG+ Weapons, UFO 35-Slide Comic Player & Mary's `.mes` Letter Scroller (`SH2-65`).
//!
//! Decomposed into `< 400`-line submodules (`AGENTS.md` §1 & §2):
//! - [`mod`]: [`StoryDemoTab`], [`StoryProgressionRoomState`], and [`StoryProgressionRoomController`].
//! - [`input`]: Tab switching (`[F1]`–`[F4]`, `[Tab]`) and per-tab interactive controls.
//! - [`hud_act1`]: Tab 1 (`[F1]`) Act 1 Stages 1–27 & Riddles (`SH2-113`).
//! - [`hud_act2`]: Tab 2 (`[F2]`) Act 2 & *Born from a Wish* Stages 25–58 & Riddles (`SH2-114`).
//! - [`hud_endings`]: Tab 3 (`[F3]`) Live Ending Judge, 10-Star Rank & NG+ Weapons + Tab 4 (`[F4]`)
//!   UFO 35-Slide Comic `.tex` Viewer & Mary's Epilogue Letter Scroller (`SH2-118`).

pub(crate) mod hud_act1;
pub(crate) mod hud_act2;
pub(crate) mod hud_endings;
pub(crate) mod input;
pub(crate) mod input_stages;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::{DecodedTexture, TextureLoader};
use sh2_audio::chainsaw_sfx::{ChainsawAudioOutput, ChainsawSoundController, ChainsawTickInput};
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::endings::{
    EndingPointTracker, GameEnding, HyperSprayCanState, HyperSprayTickResult, PlayingInfo,
    RankingRuleset, UfoSlideShowPlayer,
};
use sh2_core::events::stages_act1::{
    Act1HospitalWingKeypad, Act1LouiseBoxState, Act1VanillaFixesConfig,
};
use sh2_core::events::stages_act2::{
    init_briefcase_drums_if_needed, Act2EnhancementToggles, BoardRotation, ElevatorCupboardState,
    HotelMusicBoxPuzzle, WishGravestonePuzzle,
};
use sh2_core::{init_new_game, ClearData, Sh2Rng};
use sh2_ui::credits::EndingCreditsSequence;

/// Active sub-studio tab in [`StoryProgressionRoomController`] (`[F1]`–`[F4]` or `[Tab]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StoryDemoTab {
    /// `[F1]`: Act 1 Stages 1–27, W2F Safe Dial, Rm 105 Coin Desk, and Brookhaven Hospital Riddles (`SH2-113`).
    #[default]
    Act1StagesAndRiddles,
    /// `[F2]`: Act 2 & *Born from a Wish* Stages 25–58, `trg == 10` Key Doors, Prison/Labyrinth/Hotel/Gravestone Riddles (`SH2-114`).
    Act2StagesAndRiddles,
    /// `[F3]`: Live Ending Point Evaluator (`playing_info.c`), 10-Star Ranking, Hyper Spray & Chainsaw (`SH2-118`).
    EndingJudgeAndRanking,
    /// `[F4]`: UFO 35-Slide Comic Player (`UFO_TEXTURE_PATHS`) & Mary's Full `.mes` Epilogue Letter + Credits (`SH2-118`).
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

/// Canonical 9 retail `trg == 10` (`UseOrActionCarryingItem`, `sh2pc.exe` `0x0048e1c1` & `0x0048e1ee`)
/// key-locked doors across Act 2 & *Born from a Wish* (`(stage_id, ev_index, key_item_id, label)`).
pub(crate) const ACT2_TRG10_KEY_DOORS: [(u16, usize, u8, &str); 9] = [
    (26, 12, 38, "Hosp BF_B: Basement Storeroom Key (38)"),
    (39, 6, 46, "Hotel BF_F: Bar Key (46, Lantern Lit)"),
    (39, 7, 46, "Hotel BF_F: Bar Key (46, Lantern Unlit)"),
    (41, 27, 44, "Hotel 2F_F: Room 204 Key (44)"),
    (41, 53, 45, "Hotel 2F_F: Employee Elevator Key (45)"),
    (42, 6, 48, "Hotel 3F_F: Hotel Stairway Key (48)"),
    (42, 10, 43, "Hotel 3F_F: Room 312 Key (43)"),
    (46, 8, 25, "Hotel 3F_B: Dog Key (25)"),
    (56, 33, 82, "Wish 2F: Baldwin Acacia Key (82)"),
];

/// Runtime state for [`sh2_ui::menu::TestRoomKind::StoryStagesAndEndingsRoom`] (`SH2-65`).
pub(crate) struct StoryProgressionRoomState {
    pub active_tab: StoryDemoTab,
    pub seed_counter: u32,
    pub rng: Sh2Rng,
    // Tab 1 (Act 1 - SH2-113)
    pub act1_stage_idx: usize,
    pub act1_fixes: Act1VanillaFixesConfig,
    pub act1_wing_keypad: Act1HospitalWingKeypad,
    pub act1_louise_box: Act1LouiseBoxState,
    pub act1_radio_quiz_ticks: f32,
    pub act1_james_hp: f32,
    pub act1_ph_hp_ratio: f32,
    pub act1_ph_zone: u8,
    // Tab 2 (Act 2 - SH2-114)
    pub act2_stage_idx: usize,
    pub act2_toggles: Act2EnhancementToggles,
    pub act2_trg10_door_idx: usize,
    pub act2_carrying_key: bool,
    pub act2_trg10_last_result: String,
    pub act2_music_box: HotelMusicBoxPuzzle,
    pub act2_cupboard: ElevatorCupboardState,
    pub act2_gravestone: WishGravestonePuzzle,
    pub act2_gravestone_rot: BoardRotation,
    pub act2_corridor_room_idx: usize,
    // Tab 3 (Ending Judge, 10-Star Rank & NG+ Secret Weapons - SH2-118)
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
    // Tab 4 (UFO 35-Slide Comic & Mary's Letter + Staff Credits - SH2-118)
    pub ufo_player: UfoSlideShowPlayer,
    pub ufo_paused: bool,
    pub ufo_tex_cache: HashMap<&'static str, DecodedTexture>,
    pub selected_credits_ending_idx: usize,
    pub credits_seq: EndingCreditsSequence,
    pub credits_paused: bool,
    pub extended_cemetery_draw_dist: bool,
    pub status_banner: String,
}

impl Default for StoryProgressionRoomState {
    fn default() -> Self {
        let mut s = Self {
            active_tab: StoryDemoTab::Act1StagesAndRiddles,
            seed_counter: 1,
            rng: Sh2Rng::new(0x2001_0924),
            act1_stage_idx: 10, // Stage 11 (`stg_apart_w2f.c` Rm 203 Safe & Wallet)
            act1_fixes: Act1VanillaFixesConfig::enhanced(),
            act1_wing_keypad: Act1HospitalWingKeypad::new(),
            act1_louise_box: Act1LouiseBoxState::new(),
            act1_radio_quiz_ticks: 220.0,
            act1_james_hp: 100.0,
            act1_ph_hp_ratio: 1.0,
            act1_ph_zone: 1,
            act2_stage_idx: 16, // Stage 41 (`stage_hotel_2f_f`)
            act2_toggles: Act2EnhancementToggles::enhanced(),
            act2_trg10_door_idx: 0,
            act2_carrying_key: true,
            act2_trg10_last_result: "Press [A] to test Action-while-carrying (0x0048e1ee) or [U] for Menu Use (0x0048e1c1)".to_string(),
            act2_music_box: HotelMusicBoxPuzzle::new(),
            act2_cupboard: ElevatorCupboardState::new(),
            act2_gravestone: WishGravestonePuzzle::new(),
            act2_gravestone_rot: BoardRotation::Deg0,
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
            status_banner: "SH2-65 Unified Demo Ready: [F1] Act 1 (SH2-113) | [F2] Act 2 (SH2-114) | [F3] Endings & NG+ (SH2-118) | [F4] UFO Comic & Mary's Letter".to_string(),
        };
        s.playing_info.time = 7_200.0; // 2h 00m
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
        _dt: Duration,
        dt_sec: f32,
        _aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        _is_in_game: bool,
    ) {
        if !state.sim_core.scenario.flags.get(405) {
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
        } else if !is_paused
            && state.story_progression_room.active_tab == StoryDemoTab::EndingJudgeAndRanking
        {
            let rm = &mut state.story_progression_room;
            if !matches!(
                rm.chainsaw_sfx.idle_state,
                sh2_audio::chainsaw_sfx::ChainsawIdleState::Inactive
            ) {
                rm.last_chainsaw_audio = Some(rm.chainsaw_sfx.tick(ChainsawTickInput {
                    dt_secs: dt_sec,
                    james_pos_mm: glam::Vec3::ZERO,
                    weapon_volume: 0.85,
                    attack_active: false,
                    current_anim_frame: 1,
                    anim_speed_q11: 2048,
                    hit_this_frame: false,
                    enable_chainsaw_sound_fix: rm.chainsaw_fix_enabled,
                }));
            }
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
        draw_header_and_tabs(state);
        match state.story_progression_room.active_tab {
            StoryDemoTab::Act1StagesAndRiddles => hud_act1::render_act1_tab(state),
            StoryDemoTab::Act2StagesAndRiddles => hud_act2::render_act2_tab(state),
            StoryDemoTab::EndingJudgeAndRanking => hud_endings::render_endings_judge_tab(state),
            StoryDemoTab::UfoAndMaryLetterCredits => hud_endings::render_ufo_and_letter_tab(state),
        }
        state.finish_hud_overlay(encoder, surface_view);
    }
}

pub(super) fn draw_header_and_tabs(state: &mut AppState) {
    let yellow = [245, 210, 65, 255];
    let cyan = [105, 215, 235, 255];
    let green = [120, 230, 150, 255];
    let muted = [155, 160, 175, 255];
    let orange = [255, 170, 80, 255];

    state
        .menu_rasterizer
        .fill_rect(8, 8, 1264, 704, [8, 10, 16, 238]);
    state
        .menu_rasterizer
        .draw_rect_outline(8, 8, 1264, 704, 2, [70, 88, 118, 255]);

    state.menu_rasterizer.draw_text(
        "SILENT HILL 2 - STORY STAGES (1-58), RIDDLES, ENDINGS, NG+ WEAPONS & CREDITS STUDIO [SH2-65]",
        20,
        16,
        2,
        yellow,
        true,
    );

    let tabs = [
        (
            StoryDemoTab::Act1StagesAndRiddles,
            "[F1] Act 1 Stages 1-27 & Riddles (SH2-113)",
        ),
        (
            StoryDemoTab::Act2StagesAndRiddles,
            "[F2] Act 2 Stages 25-58 & Riddles (SH2-114)",
        ),
        (
            StoryDemoTab::EndingJudgeAndRanking,
            "[F3] Ending Judge, 10-Star & NG+ (SH2-118)",
        ),
        (
            StoryDemoTab::UfoAndMaryLetterCredits,
            "[F4] UFO 35-Slide & Mary Letter (SH2-118)",
        ),
    ];
    for (idx, (tab, title)) in tabs.iter().enumerate() {
        let bx = 20 + idx * 312;
        let active = state.story_progression_room.active_tab == *tab;
        state.menu_rasterizer.fill_rect(
            bx,
            40,
            304,
            24,
            if active {
                [28, 52, 74, 255]
            } else {
                [16, 20, 28, 255]
            },
        );
        state.menu_rasterizer.draw_rect_outline(
            bx,
            40,
            304,
            24,
            1,
            if active { green } else { [60, 70, 90, 255] },
        );
        state.menu_rasterizer.draw_text(
            title,
            bx + 8,
            46,
            1,
            if active { green } else { muted },
            true,
        );
    }

    state.menu_rasterizer.draw_text(
        &format!("STATUS: {}", state.story_progression_room.status_banner),
        20,
        674,
        1,
        orange,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[F1-F4 / Tab] Switch Tab | [PgUp/PgDn] Cycle Stage/Ending | [R] Cycle Riddle Difficulty | [V] Toggle Vanilla PS2 vs SH2:EE | [Esc] Exit",
        20,
        692,
        1,
        cyan,
        true,
    );
}

pub(super) fn ensure_ufo_texture_cached(
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

pub(super) fn play_cursor_se(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
    }
}

pub(super) fn play_confirm_se(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
    }
}

pub(super) fn cycle_riddle_difficulty(d: RiddleDifficulty) -> RiddleDifficulty {
    match d {
        RiddleDifficulty::Easy => RiddleDifficulty::Normal,
        RiddleDifficulty::Normal => RiddleDifficulty::Hard,
        RiddleDifficulty::Hard => RiddleDifficulty::Extra,
        RiddleDifficulty::Extra => RiddleDifficulty::Easy,
    }
}
