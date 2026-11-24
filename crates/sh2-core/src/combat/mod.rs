//! Authentic Silent Hill 2 combat and weapon mechanics (PS2/PC 1:1 parity).
//!
//! # Parity & Ground Truth References
//! - **`SLUS_202.28` (`docs/decomp/slus/Chacter/`):**
//!   - `sh_character_battle.c`: `shBattleAttackByHumanGunshotTypeA` (`0x00120cf0`),
//!     `shBattleAttackByHumanGunshotTypeB` (`0x00120f30`), `shBattleAttackByHumanFightType` (`0x00121370`),
//!     `shGetHumanAttackSprayPos` (`0x00121630`), `shBattleAttackByHumanFog` (`0x00121a40`),
//!     `shBattleAttackByHumanFinish` (`0x00121f40`), `shBattleExecAttackQueue` (`0x00123760`).
//!   - `sh_character_status.c`: `shBattleGetTargetEnemy` (`0x00124390`),
//!     `shBattleChangeTargetEnemy` (`0x001244d0`), `shBattleGetNearDeadlyTargetEnemy` (`0x00124800`).
//!   - `m3_play.c`: `PlayerCheckHuggingAttack` (`0x00110070`), `PlayerChectGuardSuccess` (`0x001101d0`),
//!     `PlayerRequestAttack` (`0x00110530`).
//!   - `m3_play_common.c`: `PlayerCheckHuggingButton` (`0x0021c360`), `PlayerCheckAimingToEnemy` (`0x0021c7a0`),
//!     `PlayerCheckAttack` (`0x0021c920`), `upper_attack` (`0x0021ef50`).
//! - **`sh2pc.exe` Director's Cut (`docs/decomp/sh2pc/`):**
//!   - `0x00629000` / `0x006ec380`: Complete 72-entry `sh2_attack_list` (with +6 Born From a Wish shift).
//!   - `0x005340b0` (`PlayerCheckHuggingAttack`), `0x00534210` (`PlayerChectGuardSuccess`),
//!     `0x005343a0` (`PlayerRequestAttack`), `0x00543bc0` (Shotgun), `0x00543ec0` (Melee multi-hit).

pub mod arena;
pub mod attack_list;
pub mod controller;
pub mod enemy_attack;
pub mod grapple;
pub mod hitboxes;
pub mod targeting;

pub use arena::CombatArena;
pub use attack_list::{
    get_attack_spec, AttackDispatchKind, AttackSpec, HyperSprayColor, WeaponGrip,
    ATK_CHAINSAW_LIGHT, ATK_CHAINSAW_THRUST, ATK_GREAT_KNIFE_OVERHEAD_1,
    ATK_GREAT_KNIFE_OVERHEAD_2, ATK_GREAT_KNIFE_SWING_1, ATK_GREAT_KNIFE_SWING_2, ATK_HANDGUN,
    ATK_HANDGUN_STANDING, ATK_HUNTING_RIFLE, ATK_HYPER_SPRAY_1, ATK_HYPER_SPRAY_2, ATK_KICKS,
    ATK_MARIA_CLEAVER_COMBO, ATK_MARIA_CLEAVER_HEAVY, ATK_MARIA_CLEAVER_LIGHT,
    ATK_MARIA_REVOLVER_1, ATK_MARIA_REVOLVER_2, ATK_PIPE_COMBO, ATK_PIPE_LUNGE, ATK_PIPE_OVERHEAD,
    ATK_PIPE_SWING, ATK_PLANK_COMBO, ATK_PLANK_HEAVY, ATK_PLANK_LIGHT, ATK_SHOTGUN, ATK_STOMPS,
    SE_CHAINSAW_ATTACK, SE_FIREARM_RELOAD, SE_HANDGUN_DRY_FIRE, SE_HANDGUN_SHOT,
    SE_HYPER_SPRAY_SHOT, SE_MELEE_SWISH, SE_PIPE_OR_FINISH, SE_PLANK_SWING, SE_RIFLE_DRY_FIRE,
    SE_RIFLE_SHOT, SE_SHOTGUN_DRY_FIRE, SE_SHOTGUN_SHOT, SH2_ATTACK_LIST,
};
pub use controller::{
    CombatController, CombatInputSnapshot, CombatMode, HOLD_TIMER_THRESHOLD,
    PS2_PRESSURE_HEAVY_THRESHOLD,
};
pub use enemy_attack::{
    dispatch_enemy_attack, enemy_fog_range_at_frame, select_enemy_dispatcher,
    EnemyAttackDispatcher, EnemyAttackOutcome, EnemyAttackWork,
};
pub use grapple::{
    compute_creature_grapple_transform, compute_grapple_coupling, get_grapple_spec, BestiaryId,
    GrappleAnchor, GrappleFacingMode, GrapplePhase, GrappleSession, GrappleSpec, PS2_HUGGING_POS,
    PS2_HURIHODOKI_TIMER, SPEC_ABSTRACT_DADDY, SPEC_FLESH_LIPS, SPEC_MARY_BOSS,
    SPEC_MARY_MOTH_SWARM, SPEC_PYRAMID_HEAD,
};
pub use hitboxes::{
    generate_hyper_spray_rays, generate_shotgun_cone_rays, hyper_spray_effective_range,
    CombatHitbox, CombatRng, SweptBladeQuad, HYPER_SPRAY_HALF_ANGLE_RAD,
    HYPER_SPRAY_INITIAL_SHAKES, HYPER_SPRAY_MAX_CHARGE_SECS, HYPER_SPRAY_STEP_MM,
    HYPER_SPRAY_TAPER_THRESHOLD_SECS, SHOTGUN_CONE_HALF_ANGLE_RAD, SHOTGUN_PELLET_COUNT,
};
pub use targeting::{
    aim_turn_toward_target, check_guard_success, cycle_auto_aim_target, find_near_deadly_target,
    is_guardable_attack, select_auto_aim_target, wrap_angle_pi, GuardState, TargetCandidate,
    AUTO_AIM_ANGLE_DIVISOR, AUTO_AIM_TURN_SPEED, GUARD_MIN_FACING_DELTA_RAD, MAX_TARGET_SLOTS,
    STOMP_TARGET_MAX_DIST_MM,
};

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_combat_controller_initialization() {
        let combat = CombatController::new(false, false);
        assert_eq!(combat.health, 100.0);
        assert_eq!(combat.max_health, 100.0);
        assert_eq!(combat.weapon, WeaponGrip::WoodenPlank);
        assert_eq!(combat.mode, CombatMode::Free);
        assert_eq!(combat.loaded_rounds[0], 10);
        assert_eq!(combat.loaded_rounds[1], 6);
        assert_eq!(combat.loaded_rounds[2], 4);
    }

    #[test]
    fn test_ammo_consumption_and_reload() {
        let mut combat = CombatController::new(false, false);
        combat.equip_weapon(WeaponGrip::Handgun);
        assert_eq!(combat.loaded_ammo(), 10);
        assert_eq!(combat.spare_ammo(), 30);

        for _ in 0..10 {
            assert!(combat.consume_ammo());
        }
        assert_eq!(combat.loaded_ammo(), 0);
        assert!(!combat.consume_ammo());

        let reloaded = combat.execute_reload();
        assert_eq!(reloaded, 10);
        assert_eq!(combat.loaded_ammo(), 10);
        assert_eq!(combat.spare_ammo(), 20);
    }

    #[test]
    fn test_low_health_indicator_formula() {
        let mut combat = CombatController::new(false, false);
        let dt = 1.0 / 60.0;

        assert_eq!(combat.update_low_health_alpha(dt), 0.0);

        combat.health = 25.0;
        let alpha = combat.update_low_health_alpha(dt);
        assert!(alpha > 0.5 && alpha <= 1.0);
    }

    #[test]
    fn test_grab_struggle_meter_zero_passive_decay() {
        let mut combat = CombatController::new(false, false);
        combat.apply_grab(1.0);
        assert_eq!(combat.mode, CombatMode::Held);

        // Zero button presses -> ZERO passive decay (`PlayerCheckHuggingButton` 0x0021c360)
        combat.process_struggle_inputs(0, 1.0 / 30.0, false);
        assert_eq!(combat.struggle_meter, 1.0);
        assert_eq!(combat.mode, CombatMode::Held);

        // 30 button presses at 30 Hz (dt = 1/30) drain exactly 1.0 second
        combat.process_struggle_inputs(30, 1.0 / 30.0, false);
        assert_eq!(combat.struggle_meter, 0.0);
        assert_eq!(combat.mode, CombatMode::Hurt);
    }

    #[test]
    fn test_tap_vs_hold_attack_states() {
        let mut combat = CombatController::new(false, false);
        let dt = 1.0 / 60.0;

        combat.handle_combat_inputs(true, false, false, false, dt);
        combat.mode = CombatMode::Ready;

        // Quick tap (< 0.2s) -> Light swing
        combat.handle_combat_inputs(true, true, true, false, dt);
        combat.handle_combat_inputs(true, false, false, false, dt);
        assert_eq!(combat.mode, CombatMode::Swing);
        assert!(!combat.is_heavy_attack);
        assert_eq!(combat.active_attack.unwrap().id, ATK_PLANK_LIGHT.id);

        // 0.2s hold -> Heavy downswing
        combat.mode = CombatMode::Ready;
        combat.handle_combat_inputs(true, true, true, false, dt);
        for _ in 0..15 {
            combat.handle_combat_inputs(true, false, true, false, dt);
        }
        assert_eq!(combat.mode, CombatMode::Swing);
        assert!(combat.is_heavy_attack);
        assert_eq!(combat.active_attack.unwrap().id, ATK_PLANK_HEAVY.id);
    }

    #[test]
    fn test_combat_arena_melee_and_gun_hit_resolution() {
        let mut arena = CombatArena::new();
        let dt = 1.0 / 60.0;

        assert_eq!(arena.ai.enemies.len(), 9);
        assert_eq!(arena.ai.enemies[0].stats().health, 700.0);

        // 1. Ready handgun and fire raycast at Lying Figure in front (Z=1500)
        arena.player.equip_weapon(WeaponGrip::Handgun);
        arena.player.mode = CombatMode::Ready;

        arena.tick(true, true, true, dt);
        assert_eq!(arena.player.mode, CombatMode::Shot);
        assert_eq!(arena.player.pending_sfx, Some(SE_HANDGUN_SHOT));

        arena.player.anim_frame = 2.0;
        arena.tick(true, false, false, dt);
        assert_eq!(arena.ai.enemies[0].stats().health, 580.0);

        arena.player.anim_ended = true;
        arena.tick(true, false, false, dt);
        assert_eq!(arena.player.mode, CombatMode::Ready);

        // 2. Move Lying Figure within authentic Wooden Plank swept blade reach
        // (weapon_origin Z=200 + max_range 350 = 550mm; enemy at Z=500mm)
        arena.ai.enemies[0].snap_pose(Vec3::new(0.0, 0.0, 500.0), 0.0);
        arena.player.equip_weapon(WeaponGrip::WoodenPlank);
        arena.player.mode = CombatMode::Ready;

        arena.tick(true, true, true, dt);
        for _ in 0..15 {
            arena.tick(true, false, true, dt);
        }
        assert_eq!(arena.player.mode, CombatMode::Swing);
        assert!(arena.player.is_heavy_attack);

        arena.player.anim_frame = 8.0;
        arena.attack_hit_registered = false;
        arena.tick(true, false, false, dt);
        assert_eq!(arena.ai.enemies[0].stats().health, 440.0);
    }

    #[test]
    fn test_downed_enemy_stomp_finish() {
        let mut arena = CombatArena::new();
        let dt = 1.0 / 60.0;

        arena.ai.enemies[0].force_downed(10.0);
        arena.ai.enemies[0].snap_pose(Vec3::new(0.0, 0.0, 300.0), 0.0);
        assert!(arena.has_downed_enemy_at_feet());

        arena.player.equip_weapon(WeaponGrip::Handgun);
        arena.player.mode = CombatMode::Ready;

        arena.tick(true, true, true, dt);
        for _ in 0..15 {
            arena.tick(true, false, true, dt);
        }
        assert_eq!(arena.player.mode, CombatMode::Stomp);
        assert!(arena.player.active_attack.is_some());

        arena.player.anim_frame = 3.0;
        arena.attack_hit_registered = false;
        arena.tick(true, false, true, dt);
        assert!(arena.ai.enemies[0].is_dead());
    }

    #[test]
    fn test_enemy_player_and_enemy_enemy_collision_separation() {
        let mut arena = CombatArena::new();
        arena.player_position = Vec3::ZERO;
        arena.ai.enemies[0].snap_pose(Vec3::new(100.0, 0.0, 0.0), 0.0);
        let initial_player_x = arena.player_position.x;
        let initial_enemy_x = arena.ai.enemies[0].position().x;

        arena.resolve_character_separations();

        // James 200mm + SCU `enSetSize` 150mm = 350mm separation
        let separation = (arena.player_position.x - arena.ai.enemies[0].position().x).abs();
        assert!(separation >= 349.0, "separation was {}", separation);
        assert!(arena.player_position.x < initial_player_x);
        assert!(arena.ai.enemies[0].position().x > initial_enemy_x);

        arena.ai.enemies[1].snap_pose(Vec3::new(1000.0, 0.0, 0.0), 0.0);
        arena.ai.enemies[3].snap_pose(Vec3::new(1100.0, 0.0, 0.0), 0.0);
        arena.resolve_character_separations();

        // MKN 150mm + NSE 150mm = 300mm separation
        let enemy_sep = (arena.ai.enemies[1].position().x - arena.ai.enemies[3].position().x).abs();
        assert!(enemy_sep >= 299.0, "enemy_sep was {}", enemy_sep);
    }

    #[test]
    fn test_grapple_binding_anchor_and_breakout_recoil() {
        let mut arena = CombatArena::new();
        let dt = 1.0 / 30.0;

        let pap_idx = 6;
        assert_eq!(arena.ai.enemies[pap_idx].id(), 5);

        arena.player_position = Vec3::ZERO;
        arena.player_yaw = 0.0;
        arena.ai.enemies[pap_idx].snap_pose(Vec3::new(0.0, 0.0, 400.0), 0.0);
        arena.ai.enemies[pap_idx].set_attacking(true);

        arena.tick(false, false, false, dt);

        assert_eq!(arena.player.mode, CombatMode::Held);
        assert_eq!(arena.grappling_enemy_idx, Some(pap_idx));
        assert!(arena.ai.enemies[pap_idx].is_grappling_player);

        let pap_z = arena.ai.enemies[pap_idx].position().z;
        assert!((pap_z - 328.0524).abs() < 1e-2, "Pap Z was {}", pap_z);

        // 30 presses at 30 Hz drain the 1.0s Normal Abstract Daddy hold gauge
        arena.player.process_struggle_inputs(30, dt, false);
        assert_eq!(arena.player.mode, CombatMode::Hurt);

        arena.tick(false, false, false, dt);

        assert!(arena.grappling_enemy_idx.is_none());
        assert!(!arena.ai.enemies[pap_idx].is_grappling_player);
        assert!(!arena.ai.enemies[pap_idx].is_attacking());
        assert!(arena.ai.enemies[pap_idx].position().z > 328.0524 + 200.0);
        assert!(arena.ai.enemies[pap_idx].stats().flinch_timer > 0.0);
    }

    #[test]
    fn test_all_grappling_enemies_grab_and_release() {
        let dt = 1.0 / 30.0;
        // Authentic grappling enemies in SH2 (`PlayerCheckHuggingAttack` 0x00110070 / 0x005340b0):
        // Creeper (id 8) does NOT have a Type 6 hug grapple.
        assert!(get_grapple_spec(BestiaryId::Creeper).is_none());

        let test_cases = [
            (7, 4, 300.0), // Pyramid Head (53/54)
            (6, 5, 450.0), // Abstract Daddy (63/64)
            (5, 6, 350.0), // Flesh Lips (60/61)
            (8, 9, 450.0), // Mary Boss (65/66)
        ];

        for (enemy_idx, expected_id, spawn_dist) in test_cases {
            let mut arena = CombatArena::new();
            assert_eq!(arena.ai.enemies[enemy_idx].id(), expected_id);

            arena.player_position = Vec3::ZERO;
            arena.player_yaw = 0.0;
            arena.player.mode = CombatMode::Free;

            arena.ai.enemies[enemy_idx]
                .snap_pose(Vec3::new(0.0, 0.0, spawn_dist), std::f32::consts::PI);
            arena.ai.enemies[enemy_idx].set_attacking(true);

            arena.tick(false, false, false, dt);

            assert_eq!(arena.player.mode, CombatMode::Held);
            assert_eq!(arena.grappling_enemy_idx, Some(enemy_idx));
            assert!(arena.ai.enemies[enemy_idx].is_grappling_player);

            arena.player.process_struggle_inputs(45, dt, false);
            arena.tick(false, false, false, dt);

            assert_eq!(arena.player.mode, CombatMode::Hurt);
            assert!(arena.grappling_enemy_idx.is_none());
            assert!(!arena.ai.enemies[enemy_idx].is_grappling_player);
        }
    }

    #[test]
    fn test_weapon_item_id_mappings_and_magazine_reload_conservation() {
        use crate::inventory::{ammo_for_firearm, firearm_for_ammo, item_metadata};

        let expected: &[(u8, WeaponGrip, &str)] = &[
            (4, WeaponGrip::Handgun, "Handgun"),
            (6, WeaponGrip::Shotgun, "Shotgun"),
            (8, WeaponGrip::HuntingRifle, "Hunting rifle"),
            (10, WeaponGrip::MariaRevolver, "Revolver"),
            (12, WeaponGrip::HyperSpray, "Hyper spray"),
            (13, WeaponGrip::WoodenPlank, "Wooden plank"),
            (14, WeaponGrip::SteelPipe, "Steel pipe"),
            (15, WeaponGrip::GreatKnife, "Great knife"),
            (16, WeaponGrip::Chainsaw, "Chainsaw"),
            (17, WeaponGrip::MariaChineseCleaver, "Chinese cleaver"),
        ];
        for &(id, grip, name) in expected {
            assert_eq!(CombatController::weapon_grip_from_item_id(id), Some(grip));
            assert_eq!(CombatController::weapon_grip_to_item_id(grip), Some(id));
            assert_eq!(item_metadata(id).unwrap().name, name);
        }

        for (gun_id, ammo_id) in [(4, 5), (6, 7), (8, 9), (10, 11)] {
            assert_eq!(ammo_for_firearm(gun_id), Some(ammo_id));
            assert_eq!(firearm_for_ammo(ammo_id), Some(gun_id));
        }

        let mut ctrl = CombatController::new(false, false);
        ctrl.equip_weapon(WeaponGrip::None);
        ctrl.loaded_rounds[0] = 3;
        ctrl.spare_rounds[0] = 0;
        let loaded = ctrl.load_magazine_rounds(WeaponGrip::Handgun, 5);
        assert_eq!(loaded, 5);
        assert_eq!(ctrl.loaded_rounds[0], 8);
        assert_eq!(ctrl.weapon, WeaponGrip::None);

        let loaded2 = ctrl.load_magazine_rounds(WeaponGrip::Handgun, 10);
        assert_eq!(loaded2, 2);
        assert_eq!(ctrl.loaded_rounds[0], 10);
    }
}
