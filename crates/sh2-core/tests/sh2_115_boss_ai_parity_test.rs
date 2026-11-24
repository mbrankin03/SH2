//! Deterministic golden-vector parity & wiring tests for `SH2-115`
//! (Boss & Creature AI Parity & Wiring).

use glam::Vec3;
use sh2_assets::formats::cld::{CldFile, FloorTriangle};
use sh2_core::ai::creatures::pyramid_head::{
    PyramidHeadController, RedState, RED_AP_BATTLE_END_POINTS, RED_STAIRWELL_TIMER_EXTRA_TICKS,
    RED_STAIRWELL_TIMER_TICKS,
};
use sh2_core::ai::creatures::EnemySpecies;
use sh2_core::ai::sensory::{DifficultyLevel, PerceivableTarget, RoomLighting};
use sh2_core::ai::{ActiveEnemy, AiBossEvents, AiSubsystem, EnemySpawnVariant};
use sh2_core::campaign::CampaignEnemySpawn;
use sh2_core::collision::CollisionWorld;
use sh2_core::combat::attack_list::AttackSpec;
use sh2_core::combat::{CombatController, GrappleSession};
use sh2_core::events::stages_act1::{act1_eval_bgm_control, Act1StageId};
use sh2_core::events::EventFlags;
use sh2_core::scenario::{ScenarioKind, ScenarioState};

#[test]
fn test_ph_stairwell_timer_damage_acceleration_and_siren_clear() {
    // Easy/Normal/Hard -> 10800 ticks (`0x2a30`), Hard -> 18000 ticks (`enREDInitData` `0x0015d410`).
    let ph_normal = PyramidHeadController::new_with_difficulty(
        1,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
        true,
    );
    assert_eq!(ph_normal.stairwell_timer_ticks, RED_STAIRWELL_TIMER_TICKS);

    let ph_hard =
        PyramidHeadController::new_with_difficulty(2, Vec3::ZERO, 0.0, DifficultyLevel::Hard, true);
    assert_eq!(
        ph_hard.stairwell_timer_ticks,
        RED_STAIRWELL_TIMER_EXTRA_TICKS
    );

    // Timer expiry transitions automatically to `RedState::Leave` and scales max_health * 1.5 (`0x0015d6d0`).
    let mut ph_timer = PyramidHeadController::new_with_difficulty(
        3,
        Vec3::new(98_850.0, -1700.0, -99_400.0),
        0.0,
        DifficultyLevel::Normal,
        true,
    );
    let initial_max_hp = ph_timer.stats.max_health;
    ph_timer.stairwell_timer_ticks = 1.0;
    let player = PerceivableTarget::new(Vec3::new(98_850.0, -1700.0, -100_500.0), 0.0, true, false);
    ph_timer.update(
        &player,
        RoomLighting::Lit,
        DifficultyLevel::Normal,
        1.0 / 60.0,
    );
    assert_eq!(ph_timer.state, RedState::Leave);
    assert!((ph_timer.stats.max_health - initial_max_hp * 1.5).abs() < 1e-3);

    // Damage-induced HP depletion (`enREDSetDamage` `0x0015f480`) transitions immediately to `RedState::Leave`.
    let mut ph_dmg = PyramidHeadController::new_with_difficulty(
        4,
        RED_AP_BATTLE_END_POINTS[5],
        0.0,
        DifficultyLevel::Normal,
        true,
    );
    ph_dmg.receive_hit(ph_dmg.stats.health + 10.0, 0.0);
    assert_eq!(ph_dmg.state, RedState::Leave);

    // Reaching `ap_boss_end_point[5]` sets `battle_end_complete`, which clears `ApartStair` siren (`flag(526)`).
    ph_dmg.sub_state = 9;
    ph_dmg.update(
        &player,
        RoomLighting::Lit,
        DifficultyLevel::Normal,
        1.0 / 60.0,
    );
    assert!(ph_dmg.battle_end_complete);

    let mut scenario = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
    scenario.flags.set(529, true);
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::ApartStair, &scenario.flags, 0),
        1
    );
    let events = AiBossEvents {
        ph_battle_end_complete: true,
        ..Default::default()
    };
    scenario.apply_boss_events(&events);
    assert!(scenario.flags.get(148));
    assert!(scenario.flags.get(526));
    assert_eq!(
        act1_eval_bgm_control(Act1StageId::ApartStair, &scenario.flags, 0),
        0
    );
}

#[test]
fn test_spawn_variants_and_manifest_spawning() {
    let mut ai = AiSubsystem::default();
    let spawns = [
        CampaignEnemySpawn {
            kind: 0x20b, // Town Tough Nurse (`1.2x` HP)
            id: 10,
            position_mm: [0.0, 0.0, 0.0],
            rot_y_raw: 0,
            status: 0,
            condition: 0,
        },
        CampaignEnemySpawn {
            kind: 0x207, // Standard Nurse
            id: 11,
            position_mm: [1000.0, 0.0, 0.0],
            rot_y_raw: 0,
            status: 0,
            condition: 0,
        },
        CampaignEnemySpawn {
            kind: 0x204, // Abstract Daddy boss (`sub_type == 6`)
            id: 12,
            position_mm: [2000.0, 0.0, 0.0],
            rot_y_raw: 0,
            status: 6,
            condition: 0,
        },
        CampaignEnemySpawn {
            kind: 0x205, // Eddie Room 2 (`sub_type == 6`)
            id: 13,
            position_mm: [3000.0, 0.0, 0.0],
            rot_y_raw: 0,
            status: 6,
            condition: 0,
        },
        CampaignEnemySpawn {
            kind: 0x206, // Mary Boss Phase 2 floor (`sub_type == 0xd`)
            id: 14,
            position_mm: [4000.0, 0.0, 0.0],
            rot_y_raw: 0,
            status: 0x0d,
            condition: 0,
        },
    ];
    ai.spawn_from_manifest(&spawns, DifficultyLevel::Normal);
    assert_eq!(ai.enemies.len(), 5);

    let dark_hp = ai.enemies[0].stats().max_health;
    let std_hp = ai.enemies[1].stats().max_health;
    assert!((dark_hp - std_hp * 1.2).abs() < 1e-3);

    match &ai.enemies[2].controller {
        ActiveEnemy::AbstractDaddy(p) => assert!(p.is_boss),
        other => panic!("expected AbstractDaddy, got {other:?}"),
    }
    match &ai.enemies[3].controller {
        ActiveEnemy::Eddie(e) => assert_eq!(e.phase, 1),
        other => panic!("expected Eddie, got {other:?}"),
    }
    match &ai.enemies[4].controller {
        ActiveEnemy::MaryBoss(m) => assert!(m.is_phase2_floor),
        other => panic!("expected MaryBoss, got {other:?}"),
    }

    // Also verify non-boss Pyramid Head (`sub_type != 6`) gets invincible `1_000_000.0` HP.
    ai.spawn_enemy_with_variant(
        EnemySpecies::PyramidHead,
        99,
        Vec3::ZERO,
        0.0,
        EnemySpawnVariant {
            is_dark_nurse: false,
            sub_type: 0,
        },
    );
    assert!((ai.enemies[5].stats().max_health - 1_000_000.0).abs() < 1e-3);
}

#[test]
fn test_dual_ph_shared_hp_overflow_in_ai_subsystem() {
    let mut ai = AiSubsystem::default();
    ai.spawn_enemy(EnemySpecies::TwinPyramidHead, 101, Vec3::ZERO, 0.0);
    ai.spawn_enemy(
        EnemySpecies::TwinPyramidHead,
        102,
        Vec3::new(1000.0, 0.0, 0.0),
        0.0,
    );

    ai.enemies[0].stats_mut().health = 100.0;
    let partner_initial_hp = ai.enemies[1].stats().health;

    let atk = AttackSpec {
        id: 20, // Handgun
        kind: 2,
        ap: 500.0,
        sp: 0.0,
        min_range: 0.0,
        max_range: 10_000.0,
        first_hit_frame: 0,
        last_hit_frame: 1,
        sfx_id: 0,
    };
    ai.apply_hit_to_enemy(0, &atk, [0.0, 1.0]);

    assert_eq!(ai.enemies[0].stats().health, 0.0);
    // Overflow = (500.0 - 100.0) * 0.75 = 300.0 (`enONISetDamage` `0x0022b370` / `sh2pc 0x004bc8b0`).
    assert!((ai.enemies[1].stats().health - (partner_initial_hp - 300.0)).abs() < 1e-3);
}

#[test]
fn test_grapple_session_keys_off_species_not_instance_id() {
    let mut ai = AiSubsystem::default();
    ai.spawn_enemy(
        EnemySpecies::AbstractDaddy,
        999,
        Vec3::new(0.0, 0.0, 300.0),
        0.0,
    );
    ai.enemies[0].set_attacking(true);

    let mut player = CombatController::new(false, false);
    let mut player_yaw = 0.0;
    let session = GrappleSession::try_initiate(
        0,
        &mut ai.enemies[0],
        &mut player,
        Vec3::ZERO,
        &mut player_yaw,
        2,
    );
    assert!(
        session.is_some(),
        "GrappleSession must initiate by EnemySpecies even when instance id == 999"
    );
}

#[test]
fn test_mandarin_cld_grating_probe_and_dual_ph_eggs() {
    let mut cld = CldFile::default();
    // Solid concrete floor (`material == 0`) at `0..2000` in X/Z.
    cld.floors.push(FloorTriangle {
        v: [
            [-1000.0, 0.0, -1000.0],
            [2000.0, 0.0, -1000.0],
            [2000.0, 0.0, 2000.0],
        ],
        material: 0,
    });
    cld.floors.push(FloorTriangle {
        v: [
            [-1000.0, 0.0, -1000.0],
            [2000.0, 0.0, 2000.0],
            [-1000.0, 0.0, 2000.0],
        ],
        material: 0,
    });
    let world = CollisionWorld::from_cld(&cld);

    let mut ai = AiSubsystem::default();
    ai.spawn_enemy(EnemySpecies::Mandarin, 50, Vec3::ZERO, 0.0);
    // Probe at `(0, 0, 500)` hits `material == 0` -> `mandarin_over_grating == false`, so Mandarin should not attack.
    ai.update_filtered_with_collision(
        Vec3::new(0.0, 0.0, 400.0),
        0.0,
        true,
        false,
        false,
        None,
        Some(&world),
        1.0 / 60.0,
    );
    if let ActiveEnemy::Mandarin(m) = &ai.enemies[0].controller {
        assert!(m.active_attack_id.is_none());
    }

    // Dual PH suicide event grants Rust-Colored Egg (72) & Scarlet Egg (73).
    let mut scenario = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
    let events = AiBossEvents {
        twin_ph_suicide_ready: true,
        ..Default::default()
    };
    scenario.apply_boss_events(&events);
    assert!(scenario.inventory_mgr.has_item(72));
    assert!(scenario.inventory_mgr.has_item(73));
    let _ = EventFlags::new();
}
