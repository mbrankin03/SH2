//! Integration & 1:1 parity tests for `SH2-118`:
//! - Live ending point evaluation (`playing_info.c` / `EvProgEndingJudge` `0x00576490`)
//! - 10-star ranking & Hyper Spray color tiers (`player_result.c` `0x00539c00..0x0053a1a9`)
//! - NG+ secret weapons (`Chainsaw` & `Hyper Spray`, `PreventChainsawSpawn.cpp`, `SprayEffect.cpp`)
//! - Special endings (`Rebirth`, `Dog`, and 3-location `Blue Gem` -> `UFO` comic slideshow)
//! - Epilogue Mary letter stanzas, `LeaveEndingDrawDistance.cpp`, per-ending credits themes, and retail assets.

use std::path::{Path, PathBuf};

use sh2_core::endings::*;
use sh2_core::events::{ClearData, EventFlags};
use sh2_core::inventory::item_metadata;

fn find_retail_game_root() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    candidates.into_iter().find(|p| p.join("data").is_dir())
}

#[test]
fn baseline_starting_weights_and_unseen_tie_breaker_favor_leave() {
    let tracker = EndingPointTracker::default();
    let playing = PlayingInfo::default();
    let report = tracker.evaluate(&playing, EndingEvalFlags::default());

    // Leave = 5; Maria = 3 + 1 (<3 bumps) + 1 (0 damage taken -> Maria protected) = 5; In Water = 2.
    // Both Leave and Maria are unseen and tied at 5 -> Leave checked first at 0x00576677 wins!
    assert_eq!(report.leave_points, 5);
    assert_eq!(report.maria_points, 5);
    assert_eq!(report.in_water_points, 2);
    assert_eq!(report.selected_ending, MainEndingKind::Leave);

    // If Leave was already seen on a prior run, unseen Maria biases +1 AND wins ties!
    let ng_plus = PlayingInfo {
        clear_end_kind: ClearEndKind(ClearEndKind::LEAVE),
        ..Default::default()
    };
    let ng_report = tracker.evaluate(&ng_plus, EndingEvalFlags::default());
    assert_eq!(ng_report.leave_points, 5);
    assert_eq!(ng_report.maria_points, 6);
    assert_eq!(ng_report.in_water_points, 3);
    assert_eq!(ng_report.selected_ending, MainEndingKind::Maria);
}

#[test]
fn in_water_accumulators_and_suicidal_clues_select_in_water() {
    let mut tracker = EndingPointTracker::default();
    let playing = PlayingInfo {
        jms_damage_total: 400.0,
        mar_damage_by_enemy: 50.0, // 50 / 400 = 0.125 >= 0.03 -> no Maria protection point
        ..Default::default()
    };

    // HP >= 50% does not accumulate low-HP weighted time
    tracker.tick_low_hp(60.0, 100.0, 500.0);
    assert_eq!(tracker.low_hp_weighted_secs, 0.0);

    // At near-zero HP (ratio = 0.0 -> 1 - sin(0) = 1.0), 250s exceeds both 60s and 240s (+2 In Water)
    tracker.tick_low_hp(0.0, 100.0, 250.0);
    assert!(tracker.low_hp_weighted_secs > LOW_HP_WEIGHTED_SECS_TIER_2);

    // Examine Angela's Knife (24) and Mary's Photo + Letter (20, 21 -> -1 Maria)
    tracker.record_item_examined(ITEM_ANGELAS_KNIFE);
    tracker.record_item_examined(ITEM_PHOTO_OF_MARY);
    tracker.record_item_examined(ITEM_LETTER_FROM_MARY);
    tracker.tick_maria_escort(2500.0, true, 1.0);
    tracker.tick_maria_escort(2500.0, true, 1.0);
    tracker.tick_maria_escort(2500.0, true, 1.0); // 3 bumps -> loses <3 bonus

    let flags = EndingEvalFlags {
        hospital_roof_diary_read: true,
        hotel_reading_room_tape_heard: true,
        neelys_bar_second_message_read: true,
        ..Default::default()
    };

    let report = tracker.evaluate(&playing, flags);
    // In Water: 2 (start) + 2 (low HP > 240) + 1 (knife) + 1 (roof) + 1 (tape) + 1 (Neely's Bar) = 8
    assert_eq!(report.in_water_points, 8);
    assert_eq!(report.selected_ending, MainEndingKind::InWater);
}

#[test]
fn maria_escort_overheal_friendly_fire_and_corridor_monologue_behave_authentically() {
    let mut tracker = EndingPointTracker::default();
    let mut playing = PlayingInfo {
        battle_level: 2, // Normal -> Maria HP scale = 80.0
        ..Default::default()
    };

    // Overheal: 3 heals from 40 -> +100 on max 100 => each adds 0.4, 6 heals => 2.4 (> 2.0 -> +1 Leave)
    for _ in 0..6 {
        let clamped = tracker.record_heal(40.0, 100.0, 100.0);
        assert_eq!(clamped, 100.0);
    }
    assert!(tracker.overheal_ratio_sum > OVERHEAL_RATIO_THRESHOLD);

    // Maria close > 600s (+1) vs far > 300s (-1) & friendly fire > 40.0 (-2)
    tracker.tick_maria_escort(800.0, false, 605.0);
    playing.record_maria_damage_by_attack(15, 45.0); // player pipe attack (< 0x2a) -> friendly fire
    playing.record_maria_damage_by_attack(50, 10.0); // enemy attack (>= 0x2a) -> enemy damage
    assert_eq!(playing.mar_damage_by_jms, 45.0);
    assert_eq!(playing.mar_damage_by_enemy, 10.0);

    // Mary's hallway monologue in Room 0xba: skipping early forfeits Flag 624
    let mut skipped_tracker = EndingPointTracker::default();
    assert!(!skipped_tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 4.0));
    assert!(!skipped_tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 20.0));
    assert!(!skipped_tracker.tick_mary_corridor_monologue(0x00bb, 1.0));
    assert_eq!(
        skipped_tracker.corridor_monologue,
        MaryCorridorMonologueState::SkippedEarly
    );

    // Staying full 3.5s delay + 80.0s (2400 frames at 30 Hz) awards Flag 624 (+1 Leave, +1 In Water)
    assert!(!tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 3.5));
    assert!(tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 80.1));
    assert_eq!(
        tracker.corridor_monologue,
        MaryCorridorMonologueState::Completed
    );

    let report = tracker.evaluate(&playing, EndingEvalFlags::default());
    // Leave: 5 + 1 (overheal) + 1 (corridor) = 7
    assert_eq!(report.leave_points, 7);
    assert_eq!(report.selected_ending, MainEndingKind::Leave);
}

#[test]
fn clear_data_roundtrip_and_all_seen_tie_priority() {
    let clear = ClearData {
        endings_seen: ClearData::ENDING_LEAVE
            | ClearData::ENDING_IN_WATER
            | ClearData::ENDING_MARIA
            | ClearData::ENDING_DOG
            | ClearData::ENDING_UFO
            | ClearData::ENDING_BORN_FROM_A_WISH,
        ..Default::default()
    };

    let kind = ClearEndKind::from_clear_data(clear);
    assert!(kind.contains(ClearEndKind::LEAVE));
    assert!(kind.contains(ClearEndKind::MARIA));
    assert!(kind.contains(ClearEndKind::IN_WATER));
    assert!(kind.contains(ClearEndKind::DOG));
    assert!(kind.contains(ClearEndKind::UFO));
    assert!(kind.contains(ClearEndKind::BORN_FROM_A_WISH));
    assert_eq!(kind.base_endings_count(), 3);
    assert_eq!(kind.to_clear_data_bits(), clear.endings_seen);

    // Tie between Maria (5) and Leave (5) when all 3 base endings are already seen -> Leave wins!
    let tracker = EndingPointTracker::default();
    let playing = PlayingInfo {
        clear_end_kind: ClearEndKind(
            ClearEndKind::LEAVE | ClearEndKind::MARIA | ClearEndKind::IN_WATER,
        ),
        ..Default::default()
    };
    let report = tracker.evaluate(&playing, EndingEvalFlags::default());
    assert_eq!(report.leave_points, report.maria_points);
    assert_eq!(report.selected_ending, MainEndingKind::Leave);
}

#[test]
fn ten_star_ranking_and_hyper_spray_tiers_match_binary() {
    let mut perfect = PlayingInfo {
        savecount: 2,
        clear_end_kind: ClearEndKind(
            ClearEndKind::LEAVE
                | ClearEndKind::MARIA
                | ClearEndKind::IN_WATER
                | ClearEndKind::REBIRTH
                | ClearEndKind::DOG,
        ), // 5 * 4 = 20
        battle_level: 3, // Hard = 5
        riddle_level: 3, // Extra = 5
        item_get: 160,   // 8 + 7 hidden = 15
        hidden_item_get: 0b0111_1111,
        kill_by_shot: 75,        // 15
        kill_by_fight: 75,       // 15
        time: 175.0 * 60.0,      // <= 180m = 10
        boat_clear_time: 79.5,   // <= 80s = 5
        jms_damage_total: 490.0, // <= 500 = 5
        ..Default::default()
    };

    let ps2_rank = evaluate_star_ranking(&perfect, RankingRuleset::Ps2Original);
    assert_eq!(ps2_rank.total_points, 100);
    assert_eq!(ps2_rank.big_stars(), 10);
    assert_eq!(ps2_rank.small_stars(), 0);
    assert_eq!(ps2_rank.spray_pow, 2);
    assert_eq!(
        HyperSprayColor::from_spray_pow(ps2_rank.spray_pow),
        HyperSprayColor::Green
    );
    assert_eq!(HyperSprayColor::Green.packed_abgr(), 0xFF40_FF40);
    assert_eq!(
        evaluate_spray_enemy_reaction(2, false),
        SprayEnemyReaction::InstantKillOrBossStagger { instant_kill: true }
    );
    assert_eq!(
        evaluate_spray_enemy_reaction(2, true),
        SprayEnemyReaction::InstantKillOrBossStagger {
            instant_kill: false
        }
    );

    // Save-heavy run: PS2 penalizes >30 saves (-5 -> 95 = Yellow), PC awards flat +5 (100 = Green)
    perfect.savecount = 40;
    let ps2_many_saves = evaluate_star_ranking(&perfect, RankingRuleset::Ps2Original);
    let pc_many_saves = evaluate_star_ranking(&perfect, RankingRuleset::PcDirectorCut);
    assert_eq!(ps2_many_saves.total_points, 95);
    assert_eq!(ps2_many_saves.spray_pow, 1);
    assert_eq!(HyperSprayColor::from_spray_pow(1), HyperSprayColor::Yellow);
    assert_eq!(pc_many_saves.total_points, 100);
    assert_eq!(pc_many_saves.spray_pow, 2);

    // Low rank <= 20 -> Purple (-1), mid rank -> White (0)
    assert_eq!(resolve_spray_power(20), -1);
    assert_eq!(HyperSprayColor::from_spray_pow(-1), HyperSprayColor::Purple);
    assert_eq!(resolve_spray_power(21), 0);
    assert_eq!(HyperSprayColor::from_spray_pow(0), HyperSprayColor::White);
}

#[test]
fn chainsaw_and_hyper_spray_vanilla_vs_enhanced_toggles_and_cough_mechanic() {
    // First-playthrough Easy mode: vanilla PC bug spawns Chainsaw; PreventChainsawSpawn fix suppresses it
    let vanilla_first = evaluate_ngplus_secret_spawns(ClearEndKind(0), 0, 1, 0, false);
    let fixed_first = evaluate_ngplus_secret_spawns(ClearEndKind(0), 0, 1, 0, true);
    assert!(vanilla_first.spawn_chainsaw);
    assert!(!fixed_first.spawn_chainsaw);

    // NG+ after Leave + Born from a Wish + Action Hard clear unlocks Chainsaw, Blue Gem, Hyper Spray
    let ngplus = evaluate_ngplus_secret_spawns(
        ClearEndKind(ClearEndKind::LEAVE | ClearEndKind::BORN_FROM_A_WISH),
        1,
        3,
        0x80,
        true,
    );
    assert!(ngplus.spawn_chainsaw);
    assert!(ngplus.spawn_blue_gem);
    assert!(ngplus.spawn_hyper_spray);
    assert!(!ngplus.spawn_dog_key);
    assert!(try_pickup_chainsaw(
        4,
        ROOM_FOREST_TRAIL_CHAINSAW,
        true,
        false
    ));
    assert!(!try_pickup_chainsaw(
        4,
        ROOM_FOREST_TRAIL_CHAINSAW,
        true,
        true
    ));

    // Hyper Spray canister: 20s duration, drains HP on Hard (battle_level = 3), triggers James cough at 0s
    let mut can = HyperSprayCanState::new();
    let mut hp = 100.0;
    let tick1 = can.tick_spray(12.0, 0, 3, &mut hp, 100.0);
    assert_eq!(tick1.color, HyperSprayColor::White);
    assert!((tick1.sfx_volume - HYPER_SPRAY_INITIAL_SFX_VOLUME).abs() < 1e-5);
    assert!((hp - 88.0).abs() < 1e-5);
    assert!(!tick1.triggered_james_cough);

    // Remaining = 8.0s (< 10.0s taper!)
    let tick2 = can.tick_spray(8.0, 0, 3, &mut hp, 100.0);
    assert!((tick2.effective_max_range_mm - 800.0).abs() < 1e-3);
    assert!(tick2.triggered_james_cough);
    assert!((hp - 80.0).abs() < 1e-5);

    // SprayEffect.cpp 30 FPS vanilla vs 60 FPS fix toggle
    let vanilla_spray = SprayParticleTimingConfig::resolve(false);
    let fixed_spray = SprayParticleTimingConfig::resolve(true);
    assert_eq!(vanilla_spray.speed_factor_state_0_1, 0.30);
    assert_eq!(fixed_spray.speed_factor_state_0_1, 0.15);
    assert!(vanilla_spray.should_spawn_particle(1, 3));
    assert!(!fixed_spray.should_spawn_particle(1, 3));
    assert!(fixed_spray.should_spawn_particle(2, 3));
}

#[test]
fn rebirth_dog_and_blue_gem_three_location_ufo_sequence_work_end_to_end() {
    // Verify catalog item IDs match binary item table
    assert_eq!(
        item_metadata(ITEM_CHAINSAW).expect("chainsaw").name,
        "Chainsaw"
    );
    assert_eq!(
        item_metadata(ITEM_HYPER_SPRAY).expect("spray").name,
        "Hyper spray"
    );
    assert_eq!(
        item_metadata(ITEM_DOG_KEY).expect("dog key").name,
        "Dog key"
    );
    assert_eq!(
        item_metadata(ITEM_BLUE_GEM).expect("blue gem").name,
        "Blue Gem"
    );

    // 1. Rebirth 4 items override point evaluation
    let rebirth_count = count_rebirth_ritual_items(&[
        ITEM_WHITE_CHRISM,
        ITEM_BOOK_LOST_MEMORIES,
        ITEM_OBSIDIAN_GOBLET,
        ITEM_BOOK_CRIMSON_CEREMONY,
    ]);
    assert_eq!(rebirth_count, 4);
    let mut flags = EventFlags::new();
    let eval_flags = EndingEvalFlags::from_event_flags(&flags, rebirth_count);
    let report = EndingPointTracker::default().evaluate(&PlayingInfo::default(), eval_flags);
    assert_eq!(report.selected_ending, MainEndingKind::Rebirth);

    // 2. Dog Key in Hotel 3F Observation Corridor (Stage 46, Room 0x006e)
    flags.set(FLAG_SPAWN_DOG_KEY, true);
    assert!(try_trigger_dog_ending(
        46,
        ROOM_HOTEL_3F_OBSERVATION_CORRIDOR,
        true,
        &mut flags
    ));
    assert!(flags.get(FLAG_OBSERVATION_ROOM_UNLOCKED));
    assert!(flags.get(FLAG_ENDING_DOG));

    // 3. Blue Gem 3-location sequence: Hospital Garden -> Toluca Dock -> Room 312 (blocked if VCR watched!)
    flags.set(FLAG_SPAWN_BLUE_GEM, true);
    assert_eq!(
        use_blue_gem(38, ROOM_TOLUCA_LAKE_DOCK, true, &mut flags),
        BlueGemUseResult::NoEffect
    );
    assert_eq!(
        use_blue_gem(22, ROOM_DARK_HOSPITAL_GARDEN, true, &mut flags),
        BlueGemUseResult::Pulsed(BlueGemLocation::HospitalGarden)
    );
    assert_eq!(
        use_blue_gem(38, ROOM_TOLUCA_LAKE_DOCK, true, &mut flags),
        BlueGemUseResult::Pulsed(BlueGemLocation::TolucaLakeDock)
    );

    // If videotape was already watched (Flag 476), Blue Gem in Room 312 has no effect
    flags.set(FLAG_HOTEL_VIDEOTAPE_WATCHED, true);
    assert_eq!(
        use_blue_gem(42, ROOM_HOTEL_312, true, &mut flags),
        BlueGemUseResult::NoEffect
    );

    // Before watching videotape (!Flag 476), Room 312 triggers UFO ending!
    flags.set(FLAG_HOTEL_VIDEOTAPE_WATCHED, false);
    assert_eq!(
        use_blue_gem(42, ROOM_HOTEL_312, true, &mut flags),
        BlueGemUseResult::TriggeredUfoEnding
    );
    assert!(flags.get(FLAG_ENDING_UFO));

    // Step through 35-slide UFO comic + silent-film staff roll
    let mut player = UfoSlideShowPlayer::new();
    assert_eq!(player.current_texture_path(), "data/pic/ufo/hj01.tex");
    player.tick(55.0, false);
    assert!(player.is_in_staff_credits());
    assert!(player.current_texture_path().contains("staff0"));
    player.tick(30.0, false);
    assert!(player.finished);
}

#[test]
fn epilogue_draw_distance_credits_tracks_and_retail_assets_match_pc_rip() {
    assert_eq!(leave_ending_cemetery_draw_distance(false), 30_000.0);
    assert_eq!(leave_ending_cemetery_draw_distance(true), 40_000.0);

    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Leave).music_theme,
        CreditsMusicTheme::OverdoseDelusion
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Maria).music_theme,
        CreditsMusicTheme::Promise
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::InWater).music_theme,
        CreditsMusicTheme::AngelsThanatos
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Rebirth).music_theme,
        CreditsMusicTheme::TheReverseWill
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Dog).music_theme,
        CreditsMusicTheme::DogBlooperPolka
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::Ufo).music_theme,
        CreditsMusicTheme::SilentHillUfoArrangement
    );
    assert_eq!(
        credits_descriptor_for_ending(GameEnding::BornFromAWish).music_theme,
        CreditsMusicTheme::LovePsalm
    );

    if let Some(root) = find_retail_game_root() {
        for tex_rel in UFO_TEXTURE_PATHS
            .iter()
            .chain(BlueGemLocation::HospitalGarden.vignette_textures())
            .chain(BlueGemLocation::TolucaLakeDock.vignette_textures())
        {
            let full = root.join(tex_rel);
            assert!(
                Path::new(&full).is_file(),
                "Missing retail UFO texture: {}",
                full.display()
            );
        }

        for desc in ENDING_CREDITS_TABLE {
            if let Some(ep) = desc.epilogue_fmv {
                let p = root.join("data/movie").join(ep);
                assert!(
                    p.is_file(),
                    "Missing retail epilogue Bink movie: {}",
                    p.display()
                );
            }
            if let Some(cr) = desc.credits_fmv {
                let p = root.join("data/movie").join(cr);
                assert!(
                    p.is_file(),
                    "Missing retail credits Bink movie: {}",
                    p.display()
                );
            }
        }

        let voice_afs = root.join("data/sound/adx/voice/voice.afs");
        assert!(
            voice_afs.is_file(),
            "Missing retail voice.afs: {}",
            voice_afs.display()
        );
    }
}
