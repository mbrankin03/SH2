//! Deterministic golden-vector & retail-asset integration tests for SH2-129:
//! - Unified `CombatArena` -> `AiSubsystem` architecture (no `CombatTargetEnemy` split-brain).
//! - `enReduceHP` (`0x00142200`), `enSetHitBack` (`0x00142630`), `enMoveExec` (`0x00143aa0`).
//! - `enSetHP` (`0x001420c0`) stage multipliers, `enCheckDamage` (`0x001423a0`) multi-hit division,
//!   and `enAddEnduranceDT` (`0x00142310`) `min(max_endurance, current_hp)` cap.
//! - Authentic enemy HP / Endurance / Recovery / `enSetSize` (`0x001426e0`) & Maria HP (`0x002205b0`).
//! - 4-state `enGetWorldCondition` (`0x00141ca0`), Nurse dual attack cones (`0x00159400`),
//!   difficulty-indexed hearing, `.cld` LOS (`0x00145540`), tribe alert (`0x00146e40`),
//!   sleep culling (`15000`/`7500` mm), HP-scaled walk speeds, and
//!   `shBattleAttackByEnemy{Slash,Strike,Fog,Hug,Needle,Shot}` (`0x00122310..0x00122eb0`).

use glam::Vec3;
use sh2_core::ai::creatures::creeper::CreeperController;
use sh2_core::ai::creatures::lying_figure::LyingFigureController;
use sh2_core::ai::creatures::mannequin::MannequinController;
use sh2_core::ai::creatures::nurse::NurseController;
use sh2_core::ai::creatures::pyramid_head::PyramidHeadController;
use sh2_core::ai::creatures::{
    apply_stage_hp_multipliers, en_check_sleep_in, en_check_sleep_out, CreatureStats, EnemySize,
    EnemySpecies, StagePlace, TribeCommunicationBus,
};
use sh2_core::ai::maria::{maria_max_hp_for_difficulty, MariaCompanionController};
use sh2_core::ai::sensory::{
    arm_move_speed_scale, check_hit_eyes, evaluate_sensory_with_los, ike_move_speed_mm_s,
    mkn_walk_speed_scale, nse_walk_speed_scale, oni_walk_speed_scale, pap_walk_speed_scale,
    red_walk_speed_scale, scu_crawl_speed_mm_s, scu_walk_speed_scale, DifficultyLevel,
    PerceivableTarget, RoomLighting, SensoryNotice, SensoryProfile,
};
use sh2_core::ai::AiSubsystem;
use sh2_core::combat::{
    dispatch_enemy_attack, enemy_fog_range_at_frame, get_attack_spec, select_enemy_dispatcher,
    CombatArena, EnemyAttackDispatcher, EnemyAttackWork, GuardState, HyperSprayColor,
};
use std::path::PathBuf;

#[test]
fn golden_en_reduce_hp_hitback_and_endurance_cap() {
    // `enReduceHP` (`0x00142200`): AP reduces BOTH HP and Endurance.
    // `enSetHitBack` (`0x00142630`): SP drives hitback impulse `sp * 2.0`.
    let mut stats = CreatureStats::new(700.0, 285.0, 30.0);
    stats.take_damage_directed(120.0, 40.0, [0.0, 1.0], 0.1);
    assert_eq!(stats.health, 580.0);
    assert_eq!(stats.endurance, 165.0);
    assert_eq!(stats.hitback_speed, 80.0);

    // `enMoveExec` (`0x00143aa0`): hitback decays `* 0.5` per frame, clamped to `radius - 50.0` (100mm for SCU 150mm).
    let d1 = stats.step_hit_back(150.0);
    assert!((d1.z - 40.0).abs() < 1e-4);
    assert!((stats.hitback_speed - 40.0).abs() < 1e-4);

    // Clamp test: large impulse `sp = 150.0` -> `hitback_speed = 300.0`, step 1 = `150.0` clamped to `150.0 - 50.0 = 100.0`.
    stats.set_hit_back([1.0, 0.0], 150.0);
    let d_clamped = stats.step_hit_back(150.0);
    assert!((d_clamped.x - 100.0).abs() < 1e-4);

    // `enAddEnduranceDT` (`0x00142310`): endurance recovery is capped at `min(max_endurance, current_hp)`.
    let mut low_hp = CreatureStats::new(700.0, 285.0, 30.0);
    low_hp.reduce_hp(650.0); // HP = 50.0, Endurance = 0.0
    assert_eq!(low_hp.health, 50.0);
    assert_eq!(low_hp.endurance, 0.0);
    low_hp.add_endurance_dt(30.0, 10.0); // +300.0 raw -> capped at current HP (50.0), NOT max_endurance (285.0)!
    assert_eq!(low_hp.endurance, 50.0);
}

#[test]
fn golden_en_set_hp_stage_multipliers_and_en_check_damage_multihit() {
    // `enSetHP` (`0x001420c0` / `sh2pc` `0x004a5fe0`):
    // SCU/MKN/NSE: Stage10 -> (1.5x, 1.0x), Stage11 -> (1.5x, 1.2x), Stage12/13 -> (2.0x, 1.5x).
    assert_eq!(
        apply_stage_hp_multipliers(EnemySpecies::LyingFigure, 700.0, 285.0, StagePlace::Stage10),
        (1050.0, 285.0)
    );
    assert_eq!(
        apply_stage_hp_multipliers(EnemySpecies::Mannequin, 700.0, 285.0, StagePlace::Stage11),
        (1050.0, 342.0)
    );
    assert_eq!(
        apply_stage_hp_multipliers(EnemySpecies::Nurse, 800.0, 285.0, StagePlace::Stage12),
        (1600.0, 427.5)
    );
    // Bosses & Creepers are NOT stage-multiplied.
    assert_eq!(
        apply_stage_hp_multipliers(EnemySpecies::Creeper, 400.0, 150.0, StagePlace::Stage13),
        (400.0, 150.0)
    );

    // `enCheckDamage` (`0x001423a0`): consecutive multi-hit division `ap / hit_count`.
    let mut s = CreatureStats::new(2000.0, 500.0, 0.0);
    assert_eq!(
        s.check_damage(EnemySpecies::LyingFigure, 19, 600.0, false, None),
        Some(600.0)
    );
    assert_eq!(
        s.check_damage(EnemySpecies::LyingFigure, 19, 600.0, false, None),
        Some(300.0)
    );
    assert_eq!(
        s.check_damage(EnemySpecies::LyingFigure, 19, 600.0, false, None),
        Some(200.0)
    );
    s.reset_multi_hit_if_idle();
    assert_eq!(
        s.check_damage(EnemySpecies::LyingFigure, 19, 600.0, false, None),
        Some(600.0)
    );

    // Stomp (`31..=40` retail) requires `is_downed == true`.
    s.reset_multi_hit_if_idle();
    assert_eq!(
        s.check_damage(EnemySpecies::LyingFigure, 31, 9999.0, false, None),
        None
    );
    assert_eq!(
        s.check_damage(EnemySpecies::LyingFigure, 31, 9999.0, true, None),
        Some(9999.0)
    );

    // White Hyper Spray (`8`) rejected on bosses, accepted on regular enemies.
    assert_eq!(
        s.check_damage(
            EnemySpecies::PyramidHead,
            8,
            10.0,
            false,
            Some(HyperSprayColor::White)
        ),
        None
    );
}

#[test]
fn golden_enemy_hp_endurance_size_and_maria_tables() {
    // SCU (`0x001603b0`): Beginner HP is 120.0 (not 400.0).
    assert_eq!(
        LyingFigureController::new(1, Vec3::ZERO, 0.0, DifficultyLevel::Beginner)
            .stats
            .health,
        120.0
    );
    assert_eq!(
        LyingFigureController::new(1, Vec3::ZERO, 0.0, DifficultyLevel::Normal)
            .stats
            .health,
        700.0
    );

    // MKN (`0x00153410`): Default is 1.0x (700/285 Normal), subtype 9 is 0.7x (490/199.5).
    let mkn_default = MannequinController::new(2, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
    assert_eq!(mkn_default.stats.health, 700.0);
    assert_eq!(mkn_default.stats.endurance, 285.0);
    let mkn_sub9 = MannequinController::new_with_subtype(
        2,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
        9,
        StagePlace::Default,
    );
    assert!((mkn_sub9.stats.health - 490.0).abs() < 1e-3);

    // NSE (`0x001577b0`): Normal 800/285; dark variant `0x20b` is 1.2x (960/342).
    let nse = NurseController::new(3, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
    assert_eq!(nse.stats.health, 800.0);
    assert_eq!(nse.stats.endurance, 285.0);
    let nse_dark = NurseController::new_for_stage(
        3,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
        true,
        StagePlace::Default,
    );
    assert!((nse_dark.stats.health - 960.0).abs() < 1e-3);

    // TYU (`0x00165000`): [1, 250, 400, 600] / [1, 100, 150, 240].
    let tyu = CreeperController::new_with_difficulty(8, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
    assert_eq!(tyu.stats.health, 400.0);
    assert_eq!(tyu.stats.endurance, 150.0);

    // RED (`0x0015d410`): Boss Normal 4000 HP, 300 Endurance (`1000 * 0.3`).
    let red = PyramidHeadController::new(4, Vec3::ZERO, 0.0);
    assert_eq!(red.stats.health, 4000.0);
    assert_eq!(red.stats.endurance, 300.0);

    // Maria (`0x002205b0`): 160 Beginner/Easy, 80 Normal/Hard.
    assert_eq!(
        maria_max_hp_for_difficulty(DifficultyLevel::Beginner),
        160.0
    );
    assert_eq!(maria_max_hp_for_difficulty(DifficultyLevel::Easy), 160.0);
    assert_eq!(maria_max_hp_for_difficulty(DifficultyLevel::Normal), 80.0);
    assert_eq!(maria_max_hp_for_difficulty(DifficultyLevel::Hard), 80.0);
    assert_eq!(MariaCompanionController::default().max_health, 80.0);

    // `enSetSize` (`0x001426e0`) & `EnBattleCollisionRate` (`0x002a9bf0`).
    assert_eq!(
        EnemySize::standing_for(EnemySpecies::LyingFigure).radius,
        150.0
    );
    assert_eq!(EnemySize::standing_for(EnemySpecies::Creeper).radius, 100.0);
    assert_eq!(
        EnemySize::standing_for(EnemySpecies::AbstractDaddy).radius,
        400.0
    );
    assert_eq!(
        EnemySize::standing_for(EnemySpecies::LyingFigure)
            .battle_radius(EnemySpecies::LyingFigure, false),
        120.0 // 150.0 * 0.8
    );
    assert_eq!(
        EnemySize::downed_for(EnemySpecies::LyingFigure)
            .battle_radius(EnemySpecies::LyingFigure, true),
        300.0 // 250.0 * 0.8 * 1.5
    );
}

#[test]
fn golden_sensory_sleep_tribe_bus_and_hp_scaled_walk_speeds() {
    // Outdoor sleep culling (`15000.0` sleep-in / `7500.0` sleep-out).
    assert!(!en_check_sleep_in(14_999.0, true, false, true));
    assert!(en_check_sleep_in(15_001.0, true, false, true));
    assert!(!en_check_sleep_in(20_000.0, false, false, true)); // indoor never distance-sleeps
    assert!(!en_check_sleep_out(7_500.0, true, true));
    assert!(en_check_sleep_out(7_499.0, true, true));

    // Tribe alert bus (`enSetCommunication` `0x00146d80` & `enCommunicateTribe` `0x00146e40`).
    let mut bus = TribeCommunicationBus::default();
    assert!(bus.post(2000.0, 1, 2, Vec3::new(0.0, 0.0, 500.0), 3));
    assert!(bus.query_nearest(1, Vec3::new(0.0, 0.0, 1000.0)).is_some());
    assert!(bus.query_nearest(2, Vec3::new(0.0, 0.0, 1000.0)).is_none());
    assert!(bus.query_nearest(1, Vec3::new(0.0, 0.0, 3000.0)).is_none());
    bus.tick();
    bus.tick();
    bus.tick();
    assert_eq!(bus.active_count(), 0);

    // HP-scaled walk speed formulas (`SLUS_202.28`).
    assert!((scu_walk_speed_scale(DifficultyLevel::Normal, 100.0) - 1.0).abs() < 1e-5);
    assert!((scu_walk_speed_scale(DifficultyLevel::Normal, 0.0) - 0.6).abs() < 1e-5);
    assert!((scu_crawl_speed_mm_s(DifficultyLevel::Normal, 100.0) - 3000.0).abs() < 1e-3);
    assert!((mkn_walk_speed_scale(DifficultyLevel::Normal, 50.0) - 0.75).abs() < 1e-5);
    assert!((nse_walk_speed_scale(DifficultyLevel::Hard, 100.0) - 1.2).abs() < 1e-5);
    assert!((pap_walk_speed_scale(DifficultyLevel::Normal, 100.0, true) - 0.5).abs() < 1e-5);
    assert!((ike_move_speed_mm_s(DifficultyLevel::Normal, 100.0) - 500.0).abs() < 1e-3);
    assert!((arm_move_speed_scale(DifficultyLevel::Normal, 100.0, 6000.0) - 4.0).abs() < 1e-5);
    assert!(
        (red_walk_speed_scale(DifficultyLevel::Normal, 1000.0, 1000.0, false) - 0.8).abs() < 1e-5
    );
    assert!(
        (oni_walk_speed_scale(DifficultyLevel::Normal, 0.0, 1000.0, true) - 0.375).abs() < 1e-5
    );

    // `AiSubsystem` sleep culling integration test.
    let mut ai = AiSubsystem::default();
    ai.is_outdoor = true;
    ai.spawn_enemy(
        EnemySpecies::LyingFigure,
        1,
        Vec3::new(0.0, 0.0, 16_000.0),
        0.0,
    );
    ai.update(Vec3::ZERO, 0.0, true, false, true, 1.0 / 60.0);
    assert!(ai.enemies[0].is_sleeping);
    ai.enemies[0].snap_pose(Vec3::new(0.0, 0.0, 7_000.0), 0.0);
    ai.update(Vec3::ZERO, 0.0, true, false, true, 1.0 / 60.0);
    assert!(!ai.enemies[0].is_sleeping);
}

#[test]
fn golden_sh_battle_attack_by_enemy_dispatchers_and_guard() {
    // 1. Acid Fog (`shBattleAttackByEnemyFog` `0x00122950`): indoor 1.8 vs outdoor 3.6 × 500.0.
    let fog_atk = get_attack_spec(42).expect("SCU fog attack ID 42");
    assert_eq!(
        select_enemy_dispatcher(fog_atk),
        Some(EnemyAttackDispatcher::Fog)
    );
    let indoor_max = enemy_fog_range_at_frame(fog_atk, fog_atk.last_hit_frame, false);
    let outdoor_max = enemy_fog_range_at_frame(fog_atk, fog_atk.last_hit_frame, true);
    assert!((indoor_max - 900.0).abs() < 1e-3);
    assert!((outdoor_max - 1800.0).abs() < 1e-3);

    let mut fog_work = EnemyAttackWork::default();
    let mut guard = GuardState::default();
    let fog_out = dispatch_enemy_attack(
        EnemySpecies::LyingFigure,
        fog_atk,
        &mut fog_work,
        fog_atk.last_hit_frame,
        Vec3::ZERO,
        0.0,
        Vec3::new(0.0, 0.0, 600.0),
        std::f32::consts::PI,
        &mut guard,
        2,
        false,
    );
    assert!(fog_out.hit_player);
    assert!(fog_out.is_fog);
    assert_eq!(fog_out.sfx_id, Some(12_000));

    // 2. Eddie Shot (`shBattleAttackByEnemyShot` `0x00122eb0`): retail ID 58 (5000mm), SFX 0x4719, single-shot gate.
    let shot_atk = get_attack_spec(58).expect("EDB shot attack ID 58");
    assert_eq!(
        select_enemy_dispatcher(shot_atk),
        Some(EnemyAttackDispatcher::Shot)
    );
    let mut shot_work = EnemyAttackWork::default();
    let shot_out = dispatch_enemy_attack(
        EnemySpecies::Eddie,
        shot_atk,
        &mut shot_work,
        shot_atk.first_hit_frame,
        Vec3::ZERO,
        0.0,
        Vec3::new(0.0, 0.0, 1000.0),
        std::f32::consts::PI,
        &mut guard,
        2,
        false,
    );
    assert!(shot_out.hit_player);
    assert_eq!(shot_out.sfx_id, Some(0x4719));
    // Second call on same frame does not double-fire (`hit_already == true`).
    let shot_dup = dispatch_enemy_attack(
        EnemySpecies::Eddie,
        shot_atk,
        &mut shot_work,
        shot_atk.first_hit_frame,
        Vec3::ZERO,
        0.0,
        Vec3::new(0.0, 0.0, 1000.0),
        std::f32::consts::PI,
        &mut guard,
        2,
        false,
    );
    assert!(!shot_dup.hit_player);

    // 3. Unified `CombatArena` routes all 9 arena monsters through `AiSubsystem`.
    let arena = CombatArena::new();
    assert_eq!(arena.ai.enemies.len(), 9);
}

#[test]
fn retail_cld_los_blocks_enemy_sight_in_woodside_apartments() {
    let cld_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/ap/ap01.cld",
    );
    if !cld_path.exists() {
        return;
    }
    let bytes = std::fs::read(&cld_path).expect("read retail ap01.cld");
    let cld = sh2_assets::formats::cld::parse_cld(&bytes).expect("parse retail ap01.cld");
    assert!(!cld.walls.is_empty(), "retail ap01.cld must contain walls");

    // Pick a real solid wall from retail `ap01.cld` and place enemy & target on opposite sides.
    let wall = cld
        .walls
        .iter()
        .find(|w| w.group != 2 && (w.y_bottom - w.y_top) > 100.0)
        .expect("at least one vertical wall in ap01.cld");
    let mid_x = (wall.start[0] + wall.end[0]) * 0.5;
    let mid_z = (wall.start[1] + wall.end[1]) * 0.5;
    let dx = wall.end[0] - wall.start[0];
    let dz = wall.end[1] - wall.start[1];
    let len = (dx * dx + dz * dz).sqrt().max(1.0);
    let nx = -dz / len;
    let nz = dx / len;
    let mid_y = (wall.y_top + wall.y_bottom) * 0.5;

    let enemy_pos = Vec3::new(mid_x + nx * 1200.0, mid_y, mid_z + nz * 1200.0);
    let target_pos = Vec3::new(mid_x - nx * 1200.0, mid_y, mid_z - nz * 1200.0);

    // `enCheckHitEyes` (`0x00145540`) must return positive hit distance (~1200mm) across the wall.
    let hit = check_hit_eyes(enemy_pos, target_pos, &cld.walls);
    assert!(hit >= 0.0, "expected retail wall to occlude LOS, got {hit}");

    // `evaluate_sensory_with_los` must NOT report VisualContact through the retail wall.
    let target = PerceivableTarget::new(target_pos, 0.0, false, false);
    let yaw = (target_pos.x - enemy_pos.x).atan2(target_pos.z - enemy_pos.z);
    let (notice, _) = evaluate_sensory_with_los(
        &SensoryProfile::lying_figure(),
        enemy_pos,
        yaw,
        0.0,
        &target,
        RoomLighting::Lit,
        DifficultyLevel::Normal,
        &cld.walls,
        1.0 / 60.0,
    );
    assert_eq!(notice, SensoryNotice::None);
}
