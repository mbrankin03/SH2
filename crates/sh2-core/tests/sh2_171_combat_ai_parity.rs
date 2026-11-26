//! Deterministic golden-vector & integration tests for SH2-171:
//! - Exact retail PC `sh2_attack_list` IDs across all 12 species/bosses (`EnemyActor::active_attack_id`).
//! - 10% Guard chip damage (`m3_play.c:1440-1448`), rising-edge Normal/Hard vs held Easy, and
//!   `player_can_guard` eligibility (`PlayerChectGuardSuccess` `0x001101d0` / `sh2pc 0x00534210`).
//! - Downed enemy hitbox single `1.5x` radius scaling & exact `200.0` mm height (`enSetHitColumn` `0x001440c0`).
//! - Wired AI `.cld` wall LOS (`enCheckHitEyes` `0x00145540`), `EnemyPathState::set_path` (`0x00144ae0`)
//!   + `RoomForbiddenAreas` (`0x00147000`), and `TribeCommunicationBus` (`0x00146d80`/`0x00146e40`).
//! - Hyper Spray -> `Confuse` state transitions (`enCheckSpray` `0x001425a0` / `en*CtrlConfuse`).
//! - Pyramid Head overhead chop instant-kill (`hp = -1.0` on retail `51`, `shBattleDamageRevise` `0x001205a0`),
//!   friendly-fire immunities (SCU vs `42`, Creeper vs `57`, `enCheckDamage` `0x001423a0`), and
//!   Mandarin Needle SFX randomization `0x2fdc`/`0x2fdd` (`shBattleAttackByEnemyNeedle` `0x00122d10`).

use glam::Vec3;
use sh2_assets::formats::cld::WallSegment;
use sh2_core::ai::creatures::abstract_daddy::{
    AbstractDaddyController, PapState, PAP_ATTACK_ID_GRAB_PC, PAP_ATTACK_ID_HUG_PC,
};
use sh2_core::ai::creatures::creeper::CreeperController;
use sh2_core::ai::creatures::eddie::{
    EdbState, EddieController, EDB_ATTACK_ID_PUNCH_PC, EDB_ATTACK_ID_SHOOT_PC,
};
use sh2_core::ai::creatures::flesh_lips::{
    FleshLipsController, IkeState, IKE_ATTACK_ID_GRAB_PC, IKE_ATTACK_ID_HUG_PC,
    IKE_ATTACK_ID_LUNGE_PC,
};
use sh2_core::ai::creatures::insects::{InsectState, RoomInsectController};
use sh2_core::ai::creatures::lying_figure::{LyingFigureController, ScuState};
use sh2_core::ai::creatures::mandarin::{
    MandarinController, ARM_ATTACK_ID_LEFT_PC, ARM_ATTACK_ID_RIGHT_PC,
};
use sh2_core::ai::creatures::mannequin::{MannequinController, MknState};
use sh2_core::ai::creatures::mary::{
    BosState, MaryBossController, BOS_ATTACK_ID_MOTH_GRAB_PC, BOS_ATTACK_ID_MOTH_HUG_PC,
    BOS_ATTACK_ID_STRANGLE_GRAB_PC, BOS_ATTACK_ID_STRANGLE_HUG_PC, BOS_ATTACK_ID_TENTACLE_PC,
};
use sh2_core::ai::creatures::nurse::{NseState, NurseController};
use sh2_core::ai::creatures::pyramid_head::{
    PyramidHeadController, RedState, RED_ATTACK_ID_CHOKE_PC, RED_ATTACK_ID_CLEAVE_PC,
    RED_ATTACK_ID_SEIZE_PC, RED_ATTACK_ID_SWING_PC,
};
use sh2_core::ai::creatures::twin_pyramid_head::{
    OniState, TwinPyramidHeadController, ONI_ATTACK_ID_SPEAR_SPIN_PC, ONI_ATTACK_ID_SPEAR_THRUST_PC,
};
use sh2_core::ai::creatures::{CreatureStats, EnemySpecies, StagePlace};
use sh2_core::ai::pathfinding::PathSteerState;
use sh2_core::ai::sensory::{DifficultyLevel, RoomLighting};
use sh2_core::ai::{ActiveEnemy, AiSubsystem, EnemyActor};
use sh2_core::collision::CollisionWorld;
use sh2_core::combat::{
    dispatch_enemy_attack_ex, get_attack_spec, CombatArena, CombatInputSnapshot, CombatMode,
    EnemyAttackGuardInput, EnemyAttackWork, GuardState, HyperSprayColor, ATK_HYPER_SPRAY_1,
    GUARD_CHIP_DAMAGE_SCALE, PH_OVERHEAD_CHOP_ATTACK_ID,
};

#[test]
fn golden_all_species_and_boss_active_attack_ids() {
    // 1. Lying Figure (`SCU`, `en_scu.c:1161` proto `0x24` -> retail `42`)
    let mut scu = EnemyActor::new(ActiveEnemy::LyingFigure(LyingFigureController::new(
        1,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    scu.set_attacking(true);
    assert_eq!(scu.active_attack_id(), Some(42));

    // 2. Mannequin (`MKN`, `en_mkn.c:325,328` proto `0x26`/`0x27` -> retail `44`/`45`)
    let mut mkn = EnemyActor::new(ActiveEnemy::Mannequin(MannequinController::new(
        2,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    mkn.set_attacking(true);
    assert_eq!(mkn.active_attack_id(), Some(44));
    if let ActiveEnemy::Mannequin(ref mut e) = mkn.controller {
        e.is_kick_next = true;
    }
    assert_eq!(mkn.active_attack_id(), Some(45));

    // 3. Nurse (`NSE`, `en_nse.c:471-488` proto `0x28..=0x2b` -> retail `46..=49`)
    let mut nse = EnemyActor::new(ActiveEnemy::Nurse(NurseController::new_for_stage(
        3,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
        false,
        StagePlace::Default,
    )));
    nse.set_attacking(true);
    assert_eq!(nse.active_attack_id(), Some(46));
    if let ActiveEnemy::Nurse(ref mut e) = nse.controller {
        e.is_lunge_attack = true;
    }
    assert_eq!(nse.active_attack_id(), Some(47));

    let mut dark_nse = EnemyActor::new(ActiveEnemy::Nurse(NurseController::new_for_stage(
        4,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
        true,
        StagePlace::Default,
    )));
    dark_nse.set_attacking(true);
    assert_eq!(dark_nse.active_attack_id(), Some(48));
    if let ActiveEnemy::Nurse(ref mut e) = dark_nse.controller {
        e.is_lunge_attack = true;
    }
    assert_eq!(dark_nse.active_attack_id(), Some(49));

    // 4. Creeper (`TYU`, `en_tyu.c:740` proto `0x33` -> retail `57`)
    let mut tyu = EnemyActor::new(ActiveEnemy::Creeper(CreeperController::new(
        5,
        Vec3::ZERO,
        0.0,
    )));
    tyu.set_attacking(true);
    assert_eq!(tyu.active_attack_id(), Some(57));

    // 5. Pyramid Head (`RED`, `en_red.c` retail `50`/`51`/`53`/`54`)
    let mut red = EnemyActor::new(ActiveEnemy::PyramidHead(PyramidHeadController::new(
        6,
        Vec3::ZERO,
        0.0,
    )));
    for &(state, id) in &[
        (RedState::Attack, RED_ATTACK_ID_SWING_PC),
        (RedState::Attack, RED_ATTACK_ID_CLEAVE_PC),
        (RedState::Grab, RED_ATTACK_ID_SEIZE_PC),
        (RedState::Grab, RED_ATTACK_ID_CHOKE_PC),
    ] {
        if let ActiveEnemy::PyramidHead(ref mut e) = red.controller {
            e.state = state;
            e.active_attack_id = Some(id);
        }
        assert_eq!(red.active_attack_id(), Some(u32::from(id)));
    }

    // 6. Twin Pyramid Head (`ONI`, `en_oni.c` retail `55`/`56`)
    let mut oni = EnemyActor::new(ActiveEnemy::TwinPyramidHead(
        TwinPyramidHeadController::new(7, Vec3::ZERO, 0.0, 0, DifficultyLevel::Normal),
    ));
    for &(state, id) in &[
        (OniState::Attack, ONI_ATTACK_ID_SPEAR_THRUST_PC),
        (OniState::Attack3, ONI_ATTACK_ID_SPEAR_SPIN_PC),
    ] {
        if let ActiveEnemy::TwinPyramidHead(ref mut e) = oni.controller {
            e.state = state;
            e.active_attack_id = Some(id);
        }
        assert_eq!(oni.active_attack_id(), Some(u32::from(id)));
    }

    // 7. Eddie (`EDB`, `en_edb.c` retail `58`/`59`)
    let mut edb = EnemyActor::new(ActiveEnemy::Eddie(EddieController::new(
        8,
        Vec3::ZERO,
        0.0,
        false,
        DifficultyLevel::Normal,
    )));
    for &(state, id) in &[
        (EdbState::Shoot, EDB_ATTACK_ID_SHOOT_PC),
        (EdbState::Punch, EDB_ATTACK_ID_PUNCH_PC),
    ] {
        if let ActiveEnemy::Eddie(ref mut e) = edb.controller {
            e.state = state;
            e.active_attack_id = Some(id);
        }
        assert_eq!(edb.active_attack_id(), Some(u32::from(id)));
    }

    // 8. Flesh Lips (`IKE`, `en_ike.c` retail `60`/`61`/`62`)
    let mut ike = EnemyActor::new(ActiveEnemy::FleshLips(FleshLipsController::new(
        9,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    for &id in &[
        IKE_ATTACK_ID_GRAB_PC,
        IKE_ATTACK_ID_HUG_PC,
        IKE_ATTACK_ID_LUNGE_PC,
    ] {
        if let ActiveEnemy::FleshLips(ref mut e) = ike.controller {
            e.state = IkeState::Attack;
            e.active_attack_id = Some(id);
        }
        assert_eq!(ike.active_attack_id(), Some(u32::from(id)));
    }

    // 9. Abstract Daddy (`PAP`, `en_pap.c` retail `63`/`64`)
    let mut pap = EnemyActor::new(ActiveEnemy::AbstractDaddy(AbstractDaddyController::new(
        10,
        Vec3::ZERO,
        0.0,
        false,
        DifficultyLevel::Normal,
    )));
    for &id in &[PAP_ATTACK_ID_GRAB_PC, PAP_ATTACK_ID_HUG_PC] {
        if let ActiveEnemy::AbstractDaddy(ref mut e) = pap.controller {
            e.state = PapState::Attack;
            e.active_attack_id = Some(id);
        }
        assert_eq!(pap.active_attack_id(), Some(u32::from(id)));
    }

    // 10. Mary Boss (`BOS`, `en_bos.c` retail `65..=69`)
    let mut bos = EnemyActor::new(ActiveEnemy::MaryBoss(MaryBossController::new(
        11,
        Vec3::ZERO,
        0.0,
        false,
        DifficultyLevel::Normal,
    )));
    for &id in &[
        BOS_ATTACK_ID_STRANGLE_GRAB_PC,
        BOS_ATTACK_ID_STRANGLE_HUG_PC,
        BOS_ATTACK_ID_TENTACLE_PC,
        BOS_ATTACK_ID_MOTH_GRAB_PC,
        BOS_ATTACK_ID_MOTH_HUG_PC,
    ] {
        if let ActiveEnemy::MaryBoss(ref mut e) = bos.controller {
            e.state = BosState::Attack;
            e.active_attack_id = Some(id);
        }
        assert_eq!(bos.active_attack_id(), Some(u32::from(id)));
    }

    // 11. Mandarin (`ARM`, `en_arm.c` retail `70`/`71`)
    let mut arm = EnemyActor::new(ActiveEnemy::Mandarin(MandarinController::new(
        12,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    for &(hand, id) in &[(0u8, ARM_ATTACK_ID_LEFT_PC), (1u8, ARM_ATTACK_ID_RIGHT_PC)] {
        if let ActiveEnemy::Mandarin(ref mut e) = arm.controller {
            e.active_hand = Some(hand);
            e.active_attack_id = Some(id);
        }
        assert_eq!(arm.active_attack_id(), Some(u32::from(id)));
    }

    // 12. RoomInsect (`en_insect.c`): never calls `enAttackCheck` -> `None`.
    let mut ins = EnemyActor::new(ActiveEnemy::RoomInsect(RoomInsectController::new(
        13,
        Vec3::ZERO,
        0.0,
        true,
    )));
    if let ActiveEnemy::RoomInsect(ref mut e) = ins.controller {
        e.state = InsectState::Attack;
    }
    assert_eq!(ins.active_attack_id(), None);
}

#[test]
fn golden_guard_ten_percent_chip_damage_and_input_rules() {
    // Mannequin arm swing (`44`, AP = 18.0, guardable on all difficulties).
    let mkn_atk = get_attack_spec(44).expect("MKN attack 44");
    let enemy_pos = Vec3::ZERO;
    let player_pos = Vec3::new(0.0, 0.0, 350.0);

    // 1. Normal difficulty (`2`): held WITHOUT rising-edge trigger fails to guard -> full 18.0 AP.
    let mut guard = GuardState::default();
    let mut work = EnemyAttackWork::default();
    // Seed prev_weapon_tip on frame before active window
    let _ = dispatch_enemy_attack_ex(
        EnemySpecies::Mannequin,
        mkn_atk,
        &mut work,
        mkn_atk.first_hit_frame - 1,
        enemy_pos,
        0.0,
        player_pos,
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: false,
            guard_held: true,
            player_can_guard: true,
        },
        2,
        false,
        0,
    );
    let unguarded = dispatch_enemy_attack_ex(
        EnemySpecies::Mannequin,
        mkn_atk,
        &mut work,
        mkn_atk.first_hit_frame,
        enemy_pos,
        0.0,
        player_pos,
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: false,
            guard_held: true,
            player_can_guard: true,
        },
        2,
        false,
        0,
    );
    assert!(unguarded.hit_player);
    assert!(!unguarded.guarded);
    assert!((unguarded.damage_ap - 18.0).abs() < 1e-4);

    // 2. Normal difficulty (`2`): rising-edge trigger + held + eligible -> 10% chip damage (`1.8` AP).
    work.init();
    let _ = dispatch_enemy_attack_ex(
        EnemySpecies::Mannequin,
        mkn_atk,
        &mut work,
        mkn_atk.first_hit_frame - 1,
        enemy_pos,
        0.0,
        player_pos,
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: true,
            guard_held: true,
            player_can_guard: true,
        },
        2,
        false,
        0,
    );
    let guarded = dispatch_enemy_attack_ex(
        EnemySpecies::Mannequin,
        mkn_atk,
        &mut work,
        mkn_atk.first_hit_frame,
        enemy_pos,
        0.0,
        player_pos,
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: true,
            guard_held: true,
            player_can_guard: true,
        },
        2,
        false,
        0,
    );
    assert!(guarded.hit_player);
    assert!(guarded.guarded);
    assert!((guarded.damage_ap - mkn_atk.ap * GUARD_CHIP_DAMAGE_SCALE).abs() < 1e-4);

    // 3. Ineligible player mode (`player_can_guard = false`): fails to guard -> full 18.0 AP.
    work.init();
    let _ = dispatch_enemy_attack_ex(
        EnemySpecies::Mannequin,
        mkn_atk,
        &mut work,
        mkn_atk.first_hit_frame - 1,
        enemy_pos,
        0.0,
        player_pos,
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: true,
            guard_held: true,
            player_can_guard: false,
        },
        2,
        false,
        0,
    );
    let blocked_by_state = dispatch_enemy_attack_ex(
        EnemySpecies::Mannequin,
        mkn_atk,
        &mut work,
        mkn_atk.first_hit_frame,
        enemy_pos,
        0.0,
        player_pos,
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: true,
            guard_held: true,
            player_can_guard: false,
        },
        2,
        false,
        0,
    );
    assert!(blocked_by_state.hit_player);
    assert!(!blocked_by_state.guarded);
    assert!((blocked_by_state.damage_ap - 18.0).abs() < 1e-4);

    // 4. Easy difficulty (`1`): SCU acid fog (`42`, AP = 7.0) is guardable by holding -> 0.7 AP chip.
    let fog_atk = get_attack_spec(42).expect("SCU fog 42");
    let mut fog_work = EnemyAttackWork::default();
    let mut easy_guard = GuardState::default();
    let easy_out = dispatch_enemy_attack_ex(
        EnemySpecies::LyingFigure,
        fog_atk,
        &mut fog_work,
        fog_atk.last_hit_frame,
        enemy_pos,
        0.0,
        Vec3::new(0.0, 0.0, 500.0),
        std::f32::consts::PI,
        &mut easy_guard,
        EnemyAttackGuardInput {
            guard_triggered: false,
            guard_held: true,
            player_can_guard: true,
        },
        1,
        false,
        0,
    );
    assert!(easy_out.hit_player);
    assert!(easy_out.guarded);
    assert!((easy_out.damage_ap - 0.7).abs() < 1e-4);

    // 5. End-to-end `CombatArena` applies 10% chip damage without putting James into `CombatMode::Hurt`.
    let mut arena = CombatArena::new();
    arena.ai.clear_enemies();
    arena.ai.difficulty = DifficultyLevel::Easy;
    arena
        .ai
        .spawn_enemy(EnemySpecies::LyingFigure, 1, Vec3::ZERO, 0.0);
    arena.ai.enemies[0].set_attacking(true);
    arena.player_position = Vec3::new(0.0, 0.0, 500.0);
    arena.player_yaw = std::f32::consts::PI;
    let hp_before = arena.player.health;
    // Tick through the Lying Figure's acid spray animation into active frames 9..=22 (0.3s..0.73s).
    for _ in 0..40 {
        arena.tick_with_collision_input(
            CombatInputSnapshot {
                guard_pressed: false,
                guard_held: true,
                ..Default::default()
            },
            None,
            1.0 / 60.0,
        );
    }
    assert!(
        (arena.player.health - (hp_before - 0.7)).abs() < 1e-3,
        "expected 10% guard chip damage (0.7), hp went from {hp_before} to {}",
        arena.player.health
    );
    assert_ne!(arena.player.mode, CombatMode::Hurt);
}

#[test]
fn golden_downed_hitbox_single_1_5x_radius_and_200mm_height() {
    let mut scu = EnemyActor::new(ActiveEnemy::LyingFigure(LyingFigureController::new(
        1,
        Vec3::new(100.0, 0.0, 200.0),
        0.0,
        DifficultyLevel::Normal,
    )));
    scu.force_downed(8.0);
    let hb = scu.hitbox();
    assert!(hb.is_downed);
    // `enSetNewSize` (250.0) * `EnBattleCollisionRate` (0.8) * downed (1.5) = 300.0 (NOT 450.0!).
    assert!(
        (hb.radius - 300.0).abs() < 1e-3,
        "downed SCU radius must be 300.0 mm, got {}",
        hb.radius
    );
    // Exact `enSetNewSize` downed height = 200.0 mm (`top_y = -200.0`, NOT `-60.0`!).
    assert!(
        (hb.top_y - (-200.0)).abs() < 1e-3,
        "downed SCU top_y must be -200.0 mm, got {}",
        hb.top_y
    );

    let mut nse = EnemyActor::new(ActiveEnemy::Nurse(NurseController::new(
        2,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    nse.force_downed(8.0);
    let nse_hb = nse.hitbox();
    // 200.0 * 0.8 * 1.5 = 240.0 mm, height 200.0 mm.
    assert!((nse_hb.radius - 240.0).abs() < 1e-3);
    assert!((nse_hb.top_y - (-200.0)).abs() < 1e-3);
}

#[test]
fn golden_ai_cld_los_pathfinding_forbidden_areas_and_tribe_bus() {
    // 1. `.cld` wall blocks `LyingFigure` visual detection through `AiSubsystem`.
    let mut world = CollisionWorld::new();
    world.walls.push(WallSegment {
        start: [-2000.0, 1000.0],
        end: [2000.0, 1000.0],
        y_top: -2500.0,
        y_bottom: 500.0,
        group: 1,
        material: 0,
    });

    let mut ai = AiSubsystem::default();
    ai.lighting = RoomLighting::Lit;
    ai.spawn_enemy(EnemySpecies::LyingFigure, 1, Vec3::ZERO, 0.0);
    // Player 2000mm ahead, behind wall at Z=1000 (outside 1000mm Normal feel range)
    ai.update_filtered_with_collision(
        Vec3::new(0.0, 0.0, 2000.0),
        std::f32::consts::PI,
        false,
        false,
        false,
        None,
        Some(&world),
        1.0 / 60.0,
    );
    if let ActiveEnemy::LyingFigure(ref scu) = ai.enemies[0].controller {
        assert_eq!(
            scu.state,
            ScuState::Wander,
            "SCU must remain in Wander when .cld wall occludes LOS"
        );
    } else {
        panic!("expected LyingFigure");
    }

    // 2. `EnemyPathState::set_path` steers around `.cld` wall when chasing.
    if let ActiveEnemy::LyingFigure(ref mut scu) = ai.enemies[0].controller {
        scu.state = ScuState::Chase;
    }
    ai.update_filtered_with_collision(
        Vec3::new(0.0, 0.0, 2000.0),
        std::f32::consts::PI,
        false,
        false,
        false,
        None,
        Some(&world),
        1.0 / 60.0,
    );
    if let ActiveEnemy::LyingFigure(ref scu) = ai.enemies[0].controller {
        assert_ne!(
            scu.path_state.state,
            PathSteerState::Clear,
            "enSetPath must engage obstacle avoidance against .cld wall"
        );
    }

    // 3. `RoomForbiddenAreas` sets `player_in_forbidden` flag when player stands inside room `0x07` rect.
    ai.set_room_forbidden_areas(1, 0x07, true);
    ai.update_filtered_with_collision(
        Vec3::new(83_000.0, 0.0, -70_000.0),
        0.0,
        false,
        false,
        false,
        None,
        None,
        1.0 / 60.0,
    );
    assert!(ai.forbidden_areas.flags.player_in_forbidden);

    // 4. `tribe_bus`: SCU entering Chase broadcasts 4000mm/120t alert (`tribe_id = 1`) that wakes
    //    a second occluded SCU into Precaution.
    let mut tribe_ai = AiSubsystem::default();
    tribe_ai.lighting = RoomLighting::Lit;
    // SCU #1 sees player unobstructed; SCU #2 is 2500mm away behind a wall.
    tribe_ai.spawn_enemy(EnemySpecies::LyingFigure, 1, Vec3::ZERO, 0.0);
    tribe_ai.spawn_enemy(
        EnemySpecies::LyingFigure,
        2,
        Vec3::new(0.0, 0.0, -2500.0),
        std::f32::consts::PI,
    );
    if let ActiveEnemy::LyingFigure(ref mut scu1) = tribe_ai.enemies[0].controller {
        scu1.state = ScuState::Precaution;
        scu1.state_timer = 1.6; // ready to transition into Chase on next tick
    }
    tribe_ai.update_filtered_with_collision(
        Vec3::new(0.0, 0.0, 1800.0),
        std::f32::consts::PI,
        false,
        false,
        false,
        None,
        None,
        1.0 / 60.0,
    );
    assert_eq!(
        tribe_ai.tribe_bus.active_count(),
        1,
        "SCU entering Chase must post tribe alert"
    );
    // Next tick: SCU #2 receives tribe alert and wakes from Wander -> Precaution
    tribe_ai.update_filtered_with_collision(
        Vec3::new(0.0, 0.0, 1800.0),
        std::f32::consts::PI,
        false,
        false,
        false,
        None,
        None,
        1.0 / 60.0,
    );
    if let ActiveEnemy::LyingFigure(ref scu2) = tribe_ai.enemies[1].controller {
        assert_eq!(scu2.state, ScuState::Precaution);
    }
}

#[test]
fn golden_hyper_spray_confuse_transitions() {
    // Regular enemies: White spray -> `Confuse`; Green spray -> instant kill.
    let mut scu = EnemyActor::new(ActiveEnemy::LyingFigure(LyingFigureController::new(
        1,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    assert!(scu.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::White));
    assert!(matches!(
        scu.controller,
        ActiveEnemy::LyingFigure(ref e) if e.state == ScuState::Confuse && e.sub_state == 0
    ));

    let mut mkn = EnemyActor::new(ActiveEnemy::Mannequin(MannequinController::new(
        2,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    assert!(mkn.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::Yellow));
    assert!(matches!(
        mkn.controller,
        ActiveEnemy::Mannequin(ref e) if e.state == MknState::Confuse && e.sub_state == 0
    ));

    let mut nse = EnemyActor::new(ActiveEnemy::Nurse(NurseController::new(
        3,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    assert!(nse.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::White));
    assert!(matches!(
        nse.controller,
        ActiveEnemy::Nurse(ref e) if e.state == NseState::Confuse && e.sub_state == 0
    ));

    let mut pap = EnemyActor::new(ActiveEnemy::AbstractDaddy(AbstractDaddyController::new(
        4,
        Vec3::ZERO,
        0.0,
        false,
        DifficultyLevel::Normal,
    )));
    assert!(pap.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::White));
    assert!(matches!(
        pap.controller,
        ActiveEnemy::AbstractDaddy(ref e) if e.state == PapState::Confuse && e.sub_state == 0
    ));

    // Bosses (PH & Twin PH): White spray rejected; Green spray -> `Confuse`.
    let mut red = EnemyActor::new(ActiveEnemy::PyramidHead(PyramidHeadController::new(
        5,
        Vec3::ZERO,
        0.0,
    )));
    assert!(!red.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::White));
    assert!(red.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::Green));
    assert!(matches!(
        red.controller,
        ActiveEnemy::PyramidHead(ref e) if e.state == RedState::Confuse && e.sub_state == 0
    ));

    let mut oni = EnemyActor::new(ActiveEnemy::TwinPyramidHead(
        TwinPyramidHeadController::new(6, Vec3::ZERO, 0.0, 0, DifficultyLevel::Normal),
    ));
    assert!(oni.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::Purple));
    assert!(matches!(
        oni.controller,
        ActiveEnemy::TwinPyramidHead(ref e) if e.state == OniState::Confuse && e.sub_state == 0
    ));

    // Green spray is still lethal to regular enemies.
    let mut scu_green = EnemyActor::new(ActiveEnemy::LyingFigure(LyingFigureController::new(
        7,
        Vec3::ZERO,
        0.0,
        DifficultyLevel::Normal,
    )));
    assert!(scu_green.receive_hyper_spray_hit(&ATK_HYPER_SPRAY_1, HyperSprayColor::Green));
    assert!(scu_green.is_dead());
}

#[test]
fn golden_ph_cleave_instant_kill_friendly_fire_and_needle_sfx() {
    // 1. PH overhead chop (`51` retail / `0x2d` proto) -> `instant_kill = true` (`hp = -1.0`).
    let cleave = get_attack_spec(PH_OVERHEAD_CHOP_ATTACK_ID as usize).expect("PH cleave 51");
    let mut work = EnemyAttackWork::default();
    let mut guard = GuardState::default();
    let out = dispatch_enemy_attack_ex(
        EnemySpecies::PyramidHead,
        cleave,
        &mut work,
        cleave.first_hit_frame,
        Vec3::ZERO,
        0.0,
        Vec3::new(0.0, 0.0, 400.0),
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput {
            guard_triggered: true,
            guard_held: true,
            player_can_guard: true,
        },
        2,
        false,
        0,
    );
    assert!(out.hit_player);
    assert!(!out.guarded);
    assert!(out.instant_kill);

    let mut arena = CombatArena::new();
    arena.player.health = 200.0; // Easy HP pool
    arena.player.apply_instant_kill();
    assert_eq!(arena.player.health, -1.0);
    assert_eq!(arena.player.mode, CombatMode::Dying);

    // 2. Friendly-fire immunities (`enCheckDamage` `0x001423a0` / `sh2pc 0x004a92ee`):
    //    SCU vs `42` (acid fog) and Creeper vs `57` (bite) return `None`.
    let mut stats = CreatureStats::new(500.0, 200.0, 0.0);
    assert_eq!(
        stats.check_damage(EnemySpecies::LyingFigure, 42, 7.0, false, None),
        None
    );
    assert_eq!(
        stats.check_damage(EnemySpecies::Creeper, 57, 5.0, false, None),
        None
    );
    // Cross-species is NOT immune.
    assert_eq!(
        stats.check_damage(EnemySpecies::Mannequin, 42, 7.0, false, None),
        Some(7.0)
    );

    // 3. Mandarin Needle SFX randomization (`shBattleAttackByEnemyNeedle` `0x00122d10`):
    //    `(rng_bits >> 10 & 1) + 0x2fdc` -> `0x2fdc` or `0x2fdd`.
    let needle = get_attack_spec(70).expect("ARM needle 70");
    let mut w0 = EnemyAttackWork::default();
    let out0 = dispatch_enemy_attack_ex(
        EnemySpecies::Mandarin,
        needle,
        &mut w0,
        needle.first_hit_frame,
        Vec3::ZERO,
        0.0,
        Vec3::new(0.0, 0.0, 300.0),
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput::default(),
        2,
        false,
        0,
    );
    assert_eq!(out0.sfx_id, Some(0x2fdc));

    let mut w1 = EnemyAttackWork::default();
    let out1 = dispatch_enemy_attack_ex(
        EnemySpecies::Mandarin,
        needle,
        &mut w1,
        needle.first_hit_frame,
        Vec3::ZERO,
        0.0,
        Vec3::new(0.0, 0.0, 300.0),
        std::f32::consts::PI,
        &mut guard,
        EnemyAttackGuardInput::default(),
        2,
        false,
        1 << 10,
    );
    assert_eq!(out1.sfx_id, Some(0x2fdd));
}
