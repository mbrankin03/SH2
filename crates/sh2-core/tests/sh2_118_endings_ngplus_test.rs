//! Behavioral & 1:1 parity unit/integration tests for `SH2-118`:
//! - Live ending point evaluation & `EvProgEndingJudge` tie-breakers (`0x00576490..0x0057672a`)
//! - 10-star ranking & Hyper Spray color tiers (`player_result.c` `0x00539c00..0x0053a1a9`)
//! - NG+ secret weapons & `sh2_attack_list[8, 9, 19..=23]` (`0x0038cb90` PS2 / `0x00629000` PC)
//! - Special endings (`Rebirth`, `Dog`, and 3-location `Blue Gem` -> `UFO` comic slideshow).

use sh2_core::endings::*;
use sh2_core::events::{ClearData, EventFlags};
use sh2_core::inventory::item_metadata;

#[test]
fn baseline_starting_weights_and_unseen_tie_breaker_favor_leave() {
    let tracker = EndingPointTracker::default();
    let playing = PlayingInfo::default();
    let report = tracker.evaluate(&playing, EndingEvalFlags::default());

    assert_eq!(report.leave_points, 5);
    assert_eq!(report.maria_points, 5);
    assert_eq!(report.in_water_points, 2);
    assert_eq!(report.selected_ending, MainEndingKind::Leave);

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
        mar_damage_by_enemy: 50.0,
        ..Default::default()
    };

    tracker.tick_low_hp(60.0, 100.0, 500.0);
    assert_eq!(tracker.low_hp_weighted_secs, 0.0);
    tracker.tick_low_hp(0.0, 100.0, 250.0);
    assert!(tracker.low_hp_weighted_secs > LOW_HP_WEIGHTED_SECS_TIER_2);

    tracker.record_item_examined(ITEM_ANGELAS_KNIFE);
    tracker.record_item_examined(ITEM_PHOTO_OF_MARY);
    tracker.record_item_examined(ITEM_LETTER_FROM_MARY);
    for _ in 0..3 {
        tracker.tick_maria_escort(2500.0, true, 1.0);
    }

    let flags = EndingEvalFlags {
        hospital_roof_diary_read: true,
        hotel_reading_room_tape_heard: true,
        neelys_bar_second_message_read: true,
        ..Default::default()
    };
    let report = tracker.evaluate(&playing, flags);
    assert_eq!(report.in_water_points, 8);
    assert_eq!(report.selected_ending, MainEndingKind::InWater);
}

#[test]
fn maria_escort_overheal_friendly_fire_and_corridor_monologue_behave_authentically() {
    let mut tracker = EndingPointTracker::default();
    let mut playing = PlayingInfo {
        battle_level: 2,
        ..Default::default()
    };

    for _ in 0..6 {
        assert_eq!(tracker.record_heal(40.0, 100.0, 100.0), 100.0);
    }
    assert!(tracker.overheal_ratio_sum > OVERHEAL_RATIO_THRESHOLD);

    tracker.tick_maria_escort(800.0, false, 605.0);
    playing.record_maria_damage_by_attack(15, 45.0);
    playing.record_maria_damage_by_attack(50, 10.0);
    assert_eq!(playing.mar_damage_by_jms, 45.0);
    assert_eq!(playing.mar_damage_by_enemy, 10.0);

    let mut skipped_tracker = EndingPointTracker::default();
    assert!(!skipped_tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 4.0));
    assert!(!skipped_tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 20.0));
    assert!(!skipped_tracker.tick_mary_corridor_monologue(0x00bb, 1.0));
    assert_eq!(
        skipped_tracker.corridor_monologue,
        MaryCorridorMonologueState::SkippedEarly
    );

    assert!(!tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 3.5));
    assert!(tracker.tick_mary_corridor_monologue(ROOM_HOTEL_CORRIDOR_MONOLOGUE, 80.1));
    assert_eq!(
        tracker.corridor_monologue,
        MaryCorridorMonologueState::Completed
    );

    let report = tracker.evaluate(&playing, EndingEvalFlags::default());
    assert_eq!(report.leave_points, 7);
    assert_eq!(report.selected_ending, MainEndingKind::Leave);
}

#[test]
fn clear_data_roundtrip_and_exact_0x00576677_tie_breakers() {
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
    assert_eq!(kind.base_endings_count(), 3);
    assert_eq!(kind.to_clear_data_bits(), clear.endings_seen);

    // 1. All 3 base endings seen (`0x005766c9..0x005766d9`):
    // a) `leave == maria > in_water` (5, 5, 2) -> Leave wins (`0x005766d1..0x005766d9`).
    let all_seen = PlayingInfo {
        clear_end_kind: ClearEndKind(
            ClearEndKind::LEAVE | ClearEndKind::MARIA | ClearEndKind::IN_WATER,
        ),
        ..Default::default()
    };
    let tie_lm = EndingPointTracker::default().evaluate(&all_seen, EndingEvalFlags::default());
    assert_eq!(
        (
            tie_lm.leave_points,
            tie_lm.maria_points,
            tie_lm.in_water_points
        ),
        (5, 5, 2)
    );
    assert_eq!(tie_lm.selected_ending, MainEndingKind::Leave);

    // b) 3-way tie (5, 5, 5) and `leave == in_water > maria` (5, 4, 5) -> In Water checked first (`0x005766c9`) wins!
    let three_way_flags = EndingEvalFlags {
        hospital_roof_diary_read: true,
        hotel_reading_room_tape_heard: true,
        neelys_bar_second_message_read: true,
        ..Default::default()
    };
    let tie_3way = EndingPointTracker::default().evaluate(&all_seen, three_way_flags);
    assert_eq!(
        (
            tie_3way.leave_points,
            tie_3way.maria_points,
            tie_3way.in_water_points
        ),
        (5, 5, 5)
    );
    assert_eq!(tie_3way.selected_ending, MainEndingKind::InWater);

    let mut tracker_lw = EndingPointTracker::default();
    tracker_lw.record_item_examined(ITEM_PHOTO_OF_MARY);
    tracker_lw.record_item_examined(ITEM_LETTER_FROM_MARY);
    let tie_lw = tracker_lw.evaluate(&all_seen, three_way_flags);
    assert_eq!(
        (
            tie_lw.leave_points,
            tie_lw.maria_points,
            tie_lw.in_water_points
        ),
        (5, 4, 5)
    );
    assert_eq!(tie_lw.selected_ending, MainEndingKind::InWater);

    // 2. `seen_leave && seen_maria && !seen_water` (`0x005766b5..0x005766c7`):
    // When `leave == maria > in_water` (5, 5, 4), `je 0x00576711` awards the unseen In Water ending!
    let leave_maria_seen = PlayingInfo {
        clear_end_kind: ClearEndKind(ClearEndKind::LEAVE | ClearEndKind::MARIA),
        ..Default::default()
    };
    let unseen_water =
        EndingPointTracker::default().evaluate(&leave_maria_seen, EndingEvalFlags::default());
    assert_eq!(
        (
            unseen_water.leave_points,
            unseen_water.maria_points,
            unseen_water.in_water_points
        ),
        (5, 5, 4)
    );
    assert_eq!(unseen_water.selected_ending, MainEndingKind::InWater);
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
        ),
        battle_level: 3,
        riddle_level: 3,
        item_get: 160,
        hidden_item_get: 0b0111_1111,
        kill_by_shot: 75,
        kill_by_fight: 75,
        time: 175.0 * 60.0,
        boat_clear_time: 79.5,
        jms_damage_total: 490.0,
        ..Default::default()
    };

    let ps2_rank = evaluate_star_ranking(&perfect, RankingRuleset::Ps2Original);
    assert_eq!(ps2_rank.total_points, 100);
    assert_eq!(
        (
            ps2_rank.big_stars(),
            ps2_rank.small_stars(),
            ps2_rank.spray_pow
        ),
        (10, 0, 2)
    );
    assert_eq!(
        HyperSprayColor::from_spray_pow(2).packed_abgr(),
        0xFF40_FF40
    );
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

    perfect.savecount = 40;
    let ps2_many_saves = evaluate_star_ranking(&perfect, RankingRuleset::Ps2Original);
    let pc_many_saves = evaluate_star_ranking(&perfect, RankingRuleset::PcDirectorCut);
    assert_eq!(
        (ps2_many_saves.total_points, ps2_many_saves.spray_pow),
        (95, 1)
    );
    assert_eq!(
        (pc_many_saves.total_points, pc_many_saves.spray_pow),
        (100, 2)
    );
    assert_eq!((resolve_spray_power(20), resolve_spray_power(21)), (-1, 0));
}

#[test]
fn chainsaw_and_hyper_spray_attack_list_toggles_and_cough_match_binary() {
    assert_eq!(
        (ATK_HYPER_SPRAY_STAND.ap, ATK_HYPER_SPRAY_STAND.sp),
        (1.0, 0.0)
    );
    assert_eq!(
        (ATK_HYPER_SPRAY_WALK.ap, ATK_HYPER_SPRAY_WALK.sp),
        (1.0, 0.0)
    );
    assert_eq!(CHAINSAW_ATTACK_SPECS.len(), 5);
    assert_eq!(
        (
            ATK_CHAINSAW_SWING_1.id,
            ATK_CHAINSAW_SWING_1.ap,
            ATK_CHAINSAW_SWING_1.sp,
            ATK_CHAINSAW_SWING_1.max_range
        ),
        (19, 600.0, 0.0, 725.0)
    );
    assert_eq!(
        (
            ATK_CHAINSAW_SWING_2.id,
            ATK_CHAINSAW_SWING_2.ap,
            ATK_CHAINSAW_SWING_2.sp,
            ATK_CHAINSAW_SWING_2.max_range
        ),
        (20, 600.0, 0.0, 725.0)
    );
    assert_eq!(
        (
            ATK_CHAINSAW_OVERHEAD_1.id,
            ATK_CHAINSAW_OVERHEAD_1.ap,
            ATK_CHAINSAW_OVERHEAD_1.sp,
            ATK_CHAINSAW_OVERHEAD_1.first_hit_frame,
            ATK_CHAINSAW_OVERHEAD_1.last_hit_frame
        ),
        (21, 1000.0, 0.0, 17, 18)
    );
    assert_eq!(
        (
            ATK_CHAINSAW_OVERHEAD_2.id,
            ATK_CHAINSAW_OVERHEAD_2.ap,
            ATK_CHAINSAW_OVERHEAD_2.sp,
            ATK_CHAINSAW_OVERHEAD_2.first_hit_frame,
            ATK_CHAINSAW_OVERHEAD_2.last_hit_frame
        ),
        (22, 1000.0, 0.0, 18, 19)
    );
    assert_eq!(
        (
            ATK_CHAINSAW_THRUST.id,
            ATK_CHAINSAW_THRUST.ap,
            ATK_CHAINSAW_THRUST.sp,
            ATK_CHAINSAW_THRUST.max_range
        ),
        (23, 250.0, 0.0, 525.0)
    );

    let vanilla_first = evaluate_ngplus_secret_spawns(ClearEndKind(0), 0, 1, 0, false);
    let fixed_first = evaluate_ngplus_secret_spawns(ClearEndKind(0), 0, 1, 0, true);
    assert!(vanilla_first.spawn_chainsaw && !fixed_first.spawn_chainsaw);

    let ngplus = evaluate_ngplus_secret_spawns(
        ClearEndKind(ClearEndKind::LEAVE | ClearEndKind::BORN_FROM_A_WISH),
        1,
        3,
        0x80,
        true,
    );
    assert!(
        ngplus.spawn_chainsaw
            && ngplus.spawn_blue_gem
            && ngplus.spawn_hyper_spray
            && !ngplus.spawn_dog_key
    );
    assert!(try_pickup_chainsaw(
        4,
        ROOM_FOREST_TRAIL_CHAINSAW,
        true,
        false
    ));

    let mut can = HyperSprayCanState::new();
    let mut hp = 100.0;
    let tick1 = can.tick_spray(12.0, 0, 3, &mut hp, 100.0);
    assert!((tick1.sfx_volume - HYPER_SPRAY_INITIAL_SFX_VOLUME).abs() < 1e-5);
    assert!((hp - 88.0).abs() < 1e-5);
    let tick2 = can.tick_spray(8.0, 0, 3, &mut hp, 100.0);
    assert!((tick2.effective_max_range_mm - 800.0).abs() < 1e-3);
    assert!(tick2.triggered_james_cough);

    let vanilla_spray = SprayParticleTimingConfig::resolve(false);
    let fixed_spray = SprayParticleTimingConfig::resolve(true);
    assert_eq!(
        (
            vanilla_spray.speed_factor_state_0_1,
            fixed_spray.speed_factor_state_0_1
        ),
        (0.30, 0.15)
    );
    assert!(vanilla_spray.should_spawn_particle(1, 3) && !fixed_spray.should_spawn_particle(1, 3));
}

#[test]
fn rebirth_dog_and_blue_gem_three_location_ufo_sequence_work_end_to_end() {
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

    let rebirth_count = count_rebirth_ritual_items(&REBIRTH_RITUAL_ITEM_IDS);
    let mut flags = EventFlags::new();
    let eval_flags = EndingEvalFlags::from_event_flags(&flags, rebirth_count);
    assert_eq!(
        EndingPointTracker::default()
            .evaluate(&PlayingInfo::default(), eval_flags)
            .selected_ending,
        MainEndingKind::Rebirth
    );

    flags.set(FLAG_SPAWN_DOG_KEY, true);
    assert!(try_trigger_dog_ending(
        46,
        ROOM_HOTEL_3F_OBSERVATION_CORRIDOR,
        true,
        &mut flags
    ));
    assert!(flags.get(FLAG_OBSERVATION_ROOM_UNLOCKED) && flags.get(FLAG_ENDING_DOG));

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
    flags.set(FLAG_HOTEL_VIDEOTAPE_WATCHED, true);
    assert_eq!(
        use_blue_gem(42, ROOM_HOTEL_312, true, &mut flags),
        BlueGemUseResult::NoEffect
    );
    flags.set(FLAG_HOTEL_VIDEOTAPE_WATCHED, false);
    assert_eq!(
        use_blue_gem(42, ROOM_HOTEL_312, true, &mut flags),
        BlueGemUseResult::TriggeredUfoEnding
    );

    let mut player = UfoSlideShowPlayer::new();
    player.tick(55.0, false);
    assert!(player.is_in_staff_credits());
    player.tick(30.0, false);
    assert!(player.finished);
}
