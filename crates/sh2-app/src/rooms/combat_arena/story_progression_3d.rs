//! 12 Playable 3D Story Progression Checkpoints (`[PgUp]` / `[PgDn]`) and Per-Frame Timers (`SH2-65`).

use crate::rooms::story_progression_room::{
    ensure_demo_scenario_seeded, ensure_ufo_texture_cached, Act1VisualPuzzle, Act2VisualPuzzle,
    StoryDemoTab,
};
use crate::state::AppState;
use sh2_audio::chainsaw_sfx::ChainsawTickInput;
use sh2_core::events::stages_act1::act1_step_elevator_radio_quiz;
use sh2_ui::menu::TestRoomKind;

/// Total playable 3D story progression checkpoints (`[PgUp]` / `[PgDn]`).
pub(crate) const STORY_3D_CHECKPOINT_COUNT: usize = 12;

/// Descriptor for each of the 12 playable 3D story progression checkpoints (`SH2-65`).
pub(crate) struct Story3dCheckpointSpec {
    pub index: usize,
    pub stage_badge: &'static str,
    pub title: &'static str,
    pub prompt_hint: &'static str,
    pub target_room: TestRoomKind,
    pub room_id: &'static str,
    pub map_rel_path: &'static str,
    pub cld_rel_path: &'static str,
    pub room_display_label: &'static str,
}

pub(crate) const STORY_3D_CHECKPOINTS: [Story3dCheckpointSpec; STORY_3D_CHECKPOINT_COUNT] = [
    Story3dCheckpointSpec {
        index: 0,
        stage_badge: "Stage 2 (er01.map)",
        title: "Opening Restroom Overlook (stg_toilet)",
        prompt_hint: "[E] Inspect Mirror / Exit Restroom Door -> Observation Deck (ob01.map)",
        target_room: TestRoomKind::RestroomStart,
        room_id: "er01",
        map_rel_path: "bg/er/er01.map",
        cld_rel_path: "bg/er/er01.cld",
        room_display_label: "Opening Restroom (er01.map)",
    },
    Story3dCheckpointSpec {
        index: 1,
        stage_badge: "Stage 3 (ob01.map)",
        title: "Toluca Lake Observation Deck (stg_ovservation)",
        prompt_hint:
            "[E / Enter] Inspect Blue Car Seat (out_p_incar.tex) & Take Map of Silent Hill",
        target_room: TestRoomKind::ObservationDeck,
        room_id: "ob01",
        map_rel_path: "bg/ob/ob01.map",
        cld_rel_path: "bg/ob/ob01.cld",
        room_display_label: "Observation Deck (ob01.map)",
    },
    Story3dCheckpointSpec {
        index: 2,
        stage_badge: "Stage 7/11 (ap34.map / apt_p_safe_close.tex)",
        title: "Wood Side / Blue Creek Rm 203 Safe & Wallet Clue",
        prompt_hint: "[E / Enter] Open Safe Dial Cutaway ([Left/Right] / [Space] turn dial)",
        target_room: TestRoomKind::ObservationDeck,
        room_id: "ap34",
        map_rel_path: "bg/ap/ap34.map",
        cld_rel_path: "bg/ap/ap34.cld",
        room_display_label: "Wood Side Apartments 2F (ap34.map)",
    },
    Story3dCheckpointSpec {
        index: 3,
        stage_badge: "Stage 10 (ap84.map / apt_p_desk_coin.tex)",
        title: "Blue Creek Rm 105 Five-Hole Coin Desk Riddle",
        prompt_hint:
            "[E / Enter] Open 5-Hole Coin Desk ([Z/X/C] place coin, [V] Vanilla vs SH2:EE)",
        target_room: TestRoomKind::ObservationDeck,
        room_id: "ap84",
        map_rel_path: "bg/ap/ap84.map",
        cld_rel_path: "bg/ap/ap84.cld",
        room_display_label: "Blue Creek Apartments 1F (ap84.map)",
    },
    Story3dCheckpointSpec {
        index: 4,
        stage_badge: "Stage 19/20 (hp25.map / hsp_p_box.tex)",
        title: "Brookhaven Hospital 2F Rm S14 Louise Locked Box",
        prompt_hint:
            "[E / Enter] Open 4-Wheel Louise Box ([Up/Down] wheel, [B] Blood, [P] Roof PH)",
        target_room: TestRoomKind::CombatArena,
        room_id: "hp25",
        map_rel_path: "bg/hp/hp25.map",
        cld_rel_path: "bg/hp/hp25.cld",
        room_display_label: "Brookhaven Hospital 2F (hp25.map)",
    },
    Story3dCheckpointSpec {
        index: 5,
        stage_badge: "Stage 25/27 (hp150.map / hsp_p_quiz.tex)",
        title: "Otherworld Hospital Elevator Radio Quiz & 3F Trick Chest",
        prompt_hint:
            "[E / Enter] Open 3x3 Trick Chest Quiz ((3,1,3) vs Poison Gas + Radio Subtitles)",
        target_room: TestRoomKind::CombatArena,
        room_id: "hp150",
        map_rel_path: "bg/hp/hp150.map",
        cld_rel_path: "bg/hp/hp150.cld",
        room_display_label: "Otherworld Hospital 3F (hp150.map)",
    },
    Story3dCheckpointSpec {
        index: 6,
        stage_badge: "Stage 31/34 (ps66.map / dls_p_manacles_on.tex)",
        title: "Toluca Prison Gallows & Labyrinth Rotating Face Cube",
        prompt_hint: "[E / Enter] Open Face Cube (0x008d14d4 OPEN/SEALED), 6 Hanged Men & Gallows",
        target_room: TestRoomKind::HotelBoilerRoom,
        room_id: "ps66",
        map_rel_path: "bg/ps/ps66.map",
        cld_rel_path: "bg/ps/ps66.cld",
        room_display_label: "Toluca Prison South (ps66.map)",
    },
    Story3dCheckpointSpec {
        index: 7,
        stage_badge: "Stage 40/41 (rr49.map / htl_p_trank.tex)",
        title: "Lakeview Hotel 9 Key-Doors (trg == 10) & Rm 204 Briefcase",
        prompt_hint:
            "[E / Enter] Open 4-Drum Briefcase | [K] Toggle Key | [A] Action Unlock trg==10",
        target_room: TestRoomKind::HotelBoilerRoom,
        room_id: "rr49",
        map_rel_path: "bg/rr/rr49.map",
        cld_rel_path: "bg/rr/rr49.cld",
        room_display_label: "Lakeview Hotel 2F (rr49.map)",
    },
    Story3dCheckpointSpec {
        index: 8,
        stage_badge: "Stage 42/45 (rr91.map / htl_p_video.tex)",
        title: "Hotel Rm 312 VCR Tape, Egg Doors & 2F Weight Cupboard",
        prompt_hint:
            "[E / Enter] Open VCR & Egg Doors Cutaway | [W] Transfer Inventory to Cupboard",
        target_room: TestRoomKind::HotelBoilerRoom,
        room_id: "rr91",
        map_rel_path: "bg/rr/rr91.map",
        cld_rel_path: "bg/rr/rr91.cld",
        room_display_label: "Lakeview Hotel 3F Rm 312 (rr91.map)",
    },
    Story3dCheckpointSpec {
        index: 9,
        stage_badge: "Stage 55 (ma01.map / add_p_teien.tex)",
        title: "Born from a Wish: Baldwin Mansion 3-Board Gravestone",
        prompt_hint: "[E / Enter] Open 3-Board Tombstone ([Z/X/C] rotate, [V] 0x008c4670 vs Fix)",
        target_room: TestRoomKind::HotelBoilerRoom,
        room_id: "ma01",
        map_rel_path: "bg2/ma/ma01.map",
        cld_rel_path: "bg2/ma/ma01.cld",
        room_display_label: "Baldwin Mansion 1F (ma01.map)",
    },
    Story3dCheckpointSpec {
        index: 10,
        stage_badge: "Stage 4 NG+ & UFO (ca11.map / ufo/hj01..07.tex)",
        title: "Chainsaw Log Stage, Hyper Spray & 35-Slide Blue Gem UFO Comic",
        prompt_hint:
            "[7] Equip/Swing Chainsaw | [8] Hyper Spray | [U / E] Channel Blue Gem & UFO Comic",
        target_room: TestRoomKind::CombatArena,
        room_id: "ca11",
        map_rel_path: "bg/ca/ca11.map",
        cld_rel_path: "bg/ca/ca11.cld",
        room_display_label: "Forest Trail Chainsaw Stage (ca11.map)",
    },
    Story3dCheckpointSpec {
        index: 11,
        stage_badge: "Stage 47..52 Final Ending Judge (ru73.map)",
        title: "EvProgEndingJudge, 10-Star Rank & Mary's Scrolling Letter",
        prompt_hint:
            "[E / L] Open Live Ending Judge (Leave/Maria/InWater + 10-Star) & Mary's Letter",
        target_room: TestRoomKind::ObservationDeck,
        room_id: "ru73",
        map_rel_path: "bg/ru/ru73.map",
        cld_rel_path: "bg/ru/ru73.cld",
        room_display_label: "Lakeview Hotel Final Boss Roof (ru73.map)",
    },
];

/// Returns the active [`Story3dCheckpointSpec`] for `state`.
pub(crate) fn active_3d_checkpoint(state: &AppState) -> &'static Story3dCheckpointSpec {
    let idx = state.story_progression_room.checkpoint_3d_idx % STORY_3D_CHECKPOINT_COUNT;
    &STORY_3D_CHECKPOINTS[idx]
}

/// Synchronizes `checkpoint_3d_idx` when entering `RestroomStart` or `ObservationDeck` directly.
pub(crate) fn sync_checkpoint_with_room_entry(state: &mut AppState, new_room: TestRoomKind) {
    ensure_demo_scenario_seeded(state);
    if new_room == TestRoomKind::RestroomStart {
        state.story_progression_room.checkpoint_3d_idx = 0;
    } else if new_room == TestRoomKind::ObservationDeck {
        state.story_progression_room.checkpoint_3d_idx = 1;
    }
}

/// Advances timers for active 3D story overlays (Car Map fade, Radio Quiz ticks, Chainsaw rev SFX, UFO slides, Mary's Letter scroll).
pub(crate) fn tick_3d_story_progression(state: &mut AppState, dt_sec: f32, is_paused: bool) {
    ensure_demo_scenario_seeded(state);
    if is_paused || dt_sec <= 0.0 {
        return;
    }

    state
        .sim_core
        .scenario
        .cutaway_overlay
        .tick(dt_sec, &state.sim_core.scenario.flags);

    if state.story_progression_room.act1_poison_flash_timer > 0.0 {
        state.story_progression_room.act1_poison_flash_timer =
            (state.story_progression_room.act1_poison_flash_timer - dt_sec).max(0.0);
    }

    let cp_idx = state.story_progression_room.checkpoint_3d_idx % STORY_3D_CHECKPOINT_COUNT;
    if cp_idx == 5 && state.story_progression_room.modal_3d_open {
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

    if !matches!(
        state.story_progression_room.chainsaw_sfx.idle_state,
        sh2_audio::chainsaw_sfx::ChainsawIdleState::Inactive
    ) {
        let rm = &mut state.story_progression_room;
        let out = rm.chainsaw_sfx.tick(ChainsawTickInput {
            dt_secs: dt_sec,
            james_pos_mm: state.sim_core.scenario.player_position,
            weapon_volume: 0.85,
            attack_active: false,
            current_anim_frame: 1,
            anim_speed_q11: 2048,
            hit_this_frame: false,
            enable_chainsaw_sound_fix: rm.chainsaw_fix_enabled,
        });
        rm.last_chainsaw_audio = Some(out);
        let fix_enabled = rm.chainsaw_fix_enabled;
        crate::audio_dispatch::dispatch_chainsaw_audio(state, out, fix_enabled);
    }

    if state.story_progression_room.modal_3d_open && (cp_idx == 10 || cp_idx == 11) {
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
}

/// Configures the active puzzle/tab state for `next_idx` (`0..12`).
pub(crate) fn configure_checkpoint_tab_and_puzzle(state: &mut AppState, next_idx: usize) {
    match next_idx {
        2 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act1StagesAndRiddles;
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::SafeDial;
        }
        3 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act1StagesAndRiddles;
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::CoinDesk;
        }
        4 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act1StagesAndRiddles;
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::LouiseBox;
        }
        5 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act1StagesAndRiddles;
            state.story_progression_room.act1_puzzle = Act1VisualPuzzle::QuizChest;
        }
        6 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act2StagesAndRiddles;
            state.story_progression_room.act2_puzzle = Act2VisualPuzzle::LabyrinthAndGallows;
        }
        7 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act2StagesAndRiddles;
            state.story_progression_room.act2_puzzle = Act2VisualPuzzle::HotelBriefcase;
        }
        8 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act2StagesAndRiddles;
            state.story_progression_room.act2_puzzle = Act2VisualPuzzle::HotelEggsAndVcr;
        }
        9 => {
            state.story_progression_room.active_tab = StoryDemoTab::Act2StagesAndRiddles;
            state.story_progression_room.act2_puzzle = Act2VisualPuzzle::WishGravestone;
        }
        10 => {
            state.story_progression_room.active_tab = StoryDemoTab::UfoAndMaryLetterCredits;
        }
        11 => {
            state.story_progression_room.active_tab = StoryDemoTab::EndingJudgeAndRanking;
        }
        _ => {}
    }
}
