//! SH2-128 1:1 Retail Combat Parity Golden-Vector & Binary Integration Tests.
//!
//! Verifies all 6 acceptance criteria of SH2-128 against `SLUS_202.28` (`docs/decomp/slus/Chacter/`)
//! and Retail PC `sh2pc.exe` (`0x00629000` / `0x006ec380`):
//! 1. Complete 72-entry Retail PC `sh2_attack_list` (+6 Born From a Wish ID shift).
//! 2. Missing weapons: Great Knife (19-22, multi-hit), Chainsaw (23-24, multi-hit),
//!    Hyper Spray (8-9, 5-ray cone + taper + shake recharge + color stun), Pipe Lunge (18),
//!    Maria Revolver (25-26), Maria Chinese Cleaver (28-30, multi-hit).
//! 3. Swept melee blade quad (`min_range..max_range`) & 10-pellet 3D cone Shotgun (30° half-angle).
//! 4. Weapon SFX IDs: Handgun `11028`, per-gun dry-fire `11030`/`11052`/`11049`.
//! 5. Guard/Block (`PlayerChectGuardSuccess`), Auto-Aim (`shBattleGetTargetEnemy` /
//!    `shBattleChangeTargetEnemy` / `PlayerCheckAimingToEnemy`), LOS-gated stomp (`600.0` mm).
//! 6. Grapple breakout parity (`PlayerCheckHuggingButton`: zero passive decay, no `SPEC_CREEPER`,
//!    Retail PC +6 strike/hug IDs, `6.0` DPS hold).

use glam::Vec3;
use sh2_core::combat::{
    aim_turn_toward_target, check_guard_success, cycle_auto_aim_target, find_near_deadly_target,
    generate_hyper_spray_rays, generate_shotgun_cone_rays, get_attack_spec, get_grapple_spec,
    hyper_spray_effective_range, is_guardable_attack, select_auto_aim_target, AttackDispatchKind,
    BestiaryId, CombatArena, CombatController, CombatHitbox, CombatMode, CombatRng, GuardState,
    HyperSprayColor, SweptBladeQuad, TargetCandidate, WeaponGrip, ATK_CHAINSAW_LIGHT,
    ATK_CHAINSAW_THRUST, ATK_GREAT_KNIFE_OVERHEAD_1, ATK_GREAT_KNIFE_SWING_1, ATK_HANDGUN,
    ATK_HUNTING_RIFLE, ATK_HYPER_SPRAY_1, ATK_MARIA_CLEAVER_COMBO, ATK_MARIA_CLEAVER_HEAVY,
    ATK_MARIA_CLEAVER_LIGHT, ATK_MARIA_REVOLVER_1, ATK_MARIA_REVOLVER_2, ATK_PIPE_LUNGE,
    ATK_SHOTGUN, AUTO_AIM_ANGLE_DIVISOR, AUTO_AIM_TURN_SPEED, HYPER_SPRAY_HALF_ANGLE_RAD,
    PS2_HURIHODOKI_TIMER, SE_HANDGUN_DRY_FIRE, SE_HANDGUN_SHOT, SE_HYPER_SPRAY_SHOT,
    SE_RIFLE_DRY_FIRE, SE_RIFLE_SHOT, SE_SHOTGUN_DRY_FIRE, SE_SHOTGUN_SHOT, SH2_ATTACK_LIST,
    SHOTGUN_CONE_HALF_ANGLE_RAD, SHOTGUN_PELLET_COUNT, SPEC_ABSTRACT_DADDY, SPEC_FLESH_LIPS,
    SPEC_MARY_BOSS, SPEC_MARY_MOTH_SWARM, SPEC_PYRAMID_HEAD, STOMP_TARGET_MAX_DIST_MM,
};
use std::f32::consts::{FRAC_PI_2, PI};
use std::path::Path;

#[test]
fn sh2_attack_list_has_72_retail_entries_and_correct_dispatch() {
    assert_eq!(SH2_ATTACK_LIST.len(), 72);
    assert!(get_attack_spec(71).is_some());
    assert!(get_attack_spec(72).is_none());

    // Firearms & Hyper Spray
    assert_eq!(SH2_ATTACK_LIST[1].ap, 120.0);
    assert_eq!(SH2_ATTACK_LIST[2].kind, 3);
    assert_eq!(
        ATK_HANDGUN.dispatch_kind(),
        AttackDispatchKind::GunshotTypeA
    );
    assert_eq!(
        ATK_SHOTGUN.dispatch_kind(),
        AttackDispatchKind::GunshotTypeB
    );
    assert_eq!(ATK_HUNTING_RIFLE.kind, 7);
    assert_eq!(ATK_HUNTING_RIFLE.min_range, -350.0);
    assert_eq!(ATK_HUNTING_RIFLE.max_range, 9000.0);
    assert_eq!(
        ATK_HYPER_SPRAY_1.dispatch_kind(),
        AttackDispatchKind::HumanFog
    );
    assert_eq!(ATK_HYPER_SPRAY_1.min_range, -125.0);
    assert_eq!(ATK_HYPER_SPRAY_1.max_range, 1000.0);

    // Pipe Lunge, Great Knife, Chainsaw
    assert_eq!(ATK_PIPE_LUNGE.id, 18);
    assert_eq!(ATK_PIPE_LUNGE.kind, 3);
    assert_eq!(ATK_PIPE_LUNGE.ap, 130.0);
    assert_eq!(ATK_PIPE_LUNGE.sp, 50.0);
    assert!(!ATK_PIPE_LUNGE.is_multi_hit());

    assert_eq!(ATK_GREAT_KNIFE_SWING_1.id, 19);
    assert_eq!(ATK_GREAT_KNIFE_SWING_1.ap, 600.0);
    assert_eq!(ATK_GREAT_KNIFE_SWING_1.max_range, 725.0);
    assert!(ATK_GREAT_KNIFE_SWING_1.is_multi_hit());
    assert_eq!(ATK_GREAT_KNIFE_OVERHEAD_1.id, 21);
    assert_eq!(ATK_GREAT_KNIFE_OVERHEAD_1.ap, 1000.0);
    assert!(ATK_GREAT_KNIFE_OVERHEAD_1.is_multi_hit());

    assert_eq!(ATK_CHAINSAW_LIGHT.id, 23);
    assert_eq!(ATK_CHAINSAW_LIGHT.ap, 250.0);
    assert_eq!(ATK_CHAINSAW_LIGHT.max_range, 525.0);
    assert!(ATK_CHAINSAW_LIGHT.is_multi_hit());
    assert_eq!(ATK_CHAINSAW_THRUST.id, 24);
    assert_eq!(ATK_CHAINSAW_THRUST.kind, 3);
    assert_eq!(ATK_CHAINSAW_THRUST.ap, 300.0);
    assert!(ATK_CHAINSAW_THRUST.is_multi_hit());

    // Born From a Wish Maria weapons (+6 shift)
    assert_eq!(ATK_MARIA_REVOLVER_1.id, 25);
    assert_eq!(ATK_MARIA_REVOLVER_1.ap, 150.0);
    assert_eq!(ATK_MARIA_REVOLVER_1.max_range, 7500.0);
    assert_eq!(ATK_MARIA_REVOLVER_2.id, 26);
    assert_eq!(ATK_MARIA_CLEAVER_LIGHT.id, 28);
    assert_eq!(ATK_MARIA_CLEAVER_LIGHT.ap, 60.0);
    assert_eq!(ATK_MARIA_CLEAVER_LIGHT.min_range, -100.0);
    assert_eq!(ATK_MARIA_CLEAVER_LIGHT.max_range, 250.0);
    assert!(ATK_MARIA_CLEAVER_LIGHT.is_multi_hit());
    assert_eq!(ATK_MARIA_CLEAVER_COMBO.ap, 35.0);
    assert_eq!(ATK_MARIA_CLEAVER_HEAVY.ap, 60.0);

    // Kicks (31..=35), Stomps (36..=40), Enemy attacks (42..=71)
    assert_eq!(
        SH2_ATTACK_LIST[31].dispatch_kind(),
        AttackDispatchKind::HumanFinish
    );
    assert_eq!(
        SH2_ATTACK_LIST[40].dispatch_kind(),
        AttackDispatchKind::HumanFinish
    );
    assert_eq!(SH2_ATTACK_LIST[41].id, 0);
    assert_eq!(
        SH2_ATTACK_LIST[42].dispatch_kind(),
        AttackDispatchKind::Enemy
    );
    assert_eq!(SH2_ATTACK_LIST[54].kind, 6);
    assert_eq!(SH2_ATTACK_LIST[54].ap, 0.2);
    assert_eq!(SH2_ATTACK_LIST[69].kind, 6);
    assert_eq!(SH2_ATTACK_LIST[69].ap, 0.05);
}

#[test]
fn sh2_attack_list_matches_retail_sh2pc_exe_binary_if_present() {
    let exe_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2pc.exe");
    if !exe_path.exists() {
        return;
    }

    let data = std::fs::read(&exe_path).expect("read sh2pc.exe");
    let pe_off = u32::from_le_bytes(data[0x3c..0x40].try_into().unwrap()) as usize;
    let num_sec = u16::from_le_bytes(data[pe_off + 6..pe_off + 8].try_into().unwrap()) as usize;
    let opt_sz = u16::from_le_bytes(data[pe_off + 20..pe_off + 22].try_into().unwrap()) as usize;
    let img_base = u32::from_le_bytes(data[pe_off + 52..pe_off + 56].try_into().unwrap());
    let sec_off = pe_off + 24 + opt_sz;

    let va_to_off = |va: u32| -> usize {
        let rva = va - img_base;
        for i in 0..num_sec {
            let s = sec_off + i * 40;
            let vsize = u32::from_le_bytes(data[s + 8..s + 12].try_into().unwrap());
            let vaddr = u32::from_le_bytes(data[s + 12..s + 16].try_into().unwrap());
            let rsize = u32::from_le_bytes(data[s + 16..s + 20].try_into().unwrap());
            let rptr = u32::from_le_bytes(data[s + 20..s + 24].try_into().unwrap());
            if rva >= vaddr && rva < vaddr + vsize.max(rsize) {
                return (rptr + (rva - vaddr)) as usize;
            }
        }
        panic!("VA {:#x} not found in PE sections", va);
    };

    for base_va in [0x0062_9000u32, 0x006e_c380u32] {
        let off = va_to_off(base_va);
        for (i, spec) in SH2_ATTACK_LIST.iter().enumerate() {
            let o = off + i * 0x24;
            let id = u16::from_le_bytes(data[o..o + 2].try_into().unwrap()) as u32;
            let kind = u16::from_le_bytes(data[o + 2..o + 4].try_into().unwrap()) as u8;
            let ap = f32::from_le_bytes(data[o + 4..o + 8].try_into().unwrap());
            let sp = f32::from_le_bytes(data[o + 8..o + 12].try_into().unwrap());
            let min_range = f32::from_le_bytes(data[o + 12..o + 16].try_into().unwrap());
            let max_range = f32::from_le_bytes(data[o + 16..o + 20].try_into().unwrap());
            let first_hit_frame = data[o + 0x20];
            let last_hit_frame = data[o + 0x21];

            assert_eq!(spec.id, id, "entry {i} id mismatch at {base_va:#x}");
            assert_eq!(spec.kind, kind, "entry {i} kind mismatch at {base_va:#x}");
            assert_eq!(spec.ap, ap, "entry {i} ap mismatch at {base_va:#x}");
            assert_eq!(spec.sp, sp, "entry {i} sp mismatch at {base_va:#x}");
            assert_eq!(spec.min_range, min_range, "entry {i} min_range mismatch");
            assert_eq!(spec.max_range, max_range, "entry {i} max_range mismatch");
            assert_eq!(
                spec.first_hit_frame, first_hit_frame,
                "entry {i} start frame"
            );
            assert_eq!(spec.last_hit_frame, last_hit_frame, "entry {i} end frame");
        }
    }
}

#[test]
fn weapon_sfx_and_per_gun_dry_fire_parity() {
    let dt = 1.0 / 60.0;

    // 1. Handgun shot = 11028 (0x2b14), dry-fire = 11030 (0x2b16)
    let mut ctrl = CombatController::new(false, false);
    ctrl.equip_weapon(WeaponGrip::Handgun);
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_HANDGUN_SHOT));

    ctrl.mode = CombatMode::Ready;
    ctrl.loaded_rounds[0] = 0;
    ctrl.spare_rounds[0] = 0;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_HANDGUN_DRY_FIRE));

    // 2. Shotgun shot = 11051 (0x2b2b), dry-fire = 11052 (0x2b2c)
    ctrl.equip_weapon(WeaponGrip::Shotgun);
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_SHOTGUN_SHOT));

    ctrl.mode = CombatMode::Ready;
    ctrl.loaded_rounds[1] = 0;
    ctrl.spare_rounds[1] = 0;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_SHOTGUN_DRY_FIRE));

    // 3. Hunting Rifle shot = 11048 (0x2b28), dry-fire = 11049 (0x2b29)
    ctrl.equip_weapon(WeaponGrip::HuntingRifle);
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_RIFLE_SHOT));

    ctrl.mode = CombatMode::Ready;
    ctrl.loaded_rounds[2] = 0;
    ctrl.spare_rounds[2] = 0;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_RIFLE_DRY_FIRE));

    // 4. Maria Revolver shot = 11028, dry-fire = 11030
    ctrl.equip_weapon(WeaponGrip::MariaRevolver);
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_HANDGUN_SHOT));
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_MARIA_REVOLVER_2.id);

    ctrl.mode = CombatMode::Ready;
    ctrl.loaded_rounds[3] = 0;
    ctrl.spare_rounds[3] = 0;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    assert_eq!(ctrl.pending_sfx, Some(SE_HANDGUN_DRY_FIRE));
}

#[test]
fn missing_weapons_great_knife_chainsaw_pipe_lunge_and_maria_cleaver() {
    let dt = 1.0 / 60.0;

    // Pipe Lunge dispatch (ID 18) on stationary light tap
    let mut ctrl = CombatController::new(false, false);
    ctrl.equip_weapon(WeaponGrip::SteelPipe);
    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    ctrl.handle_combat_inputs(true, false, false, false, dt);
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_PIPE_LUNGE.id);

    // Great Knife multi-hit (ID 19 horizontal 600 AP, ID 21 overhead 1000 AP)
    let mut arena = CombatArena::new();
    arena.player.equip_weapon(WeaponGrip::GreatKnife);
    arena.player.mode = CombatMode::Ready;
    // Place Pyramid Head (high HP) within Great Knife 725mm blade reach
    arena.ai.enemies[7].snap_pose(Vec3::new(0.0, 0.0, 600.0), 0.0);
    let hp_before = arena.ai.enemies[7].stats().health;

    arena.tick(true, true, true, dt);
    arena.tick(true, false, false, dt);
    assert_eq!(arena.player.active_attack.unwrap().id, 19);

    // Two distinct active frames (10 and 11) register multi-hit damage with `enCheckDamage`
    // (`0x001423a0`) consecutive-hit division: hit 1 = 600/1 = 600.0, hit 2 = 600/2 = 300.0.
    arena.player.anim_frame = 10.0;
    arena.tick(true, false, false, dt);
    let hp_after_f10 = arena.ai.enemies[7].stats().health;
    assert_eq!(hp_before - hp_after_f10, 600.0);

    arena.player.anim_frame = 11.0;
    arena.tick(true, false, false, dt);
    let hp_after_f11 = arena.ai.enemies[7].stats().health;
    assert_eq!(hp_after_f10 - hp_after_f11, 300.0);

    // Chainsaw light (ID 23) & heavy thrust (ID 24)
    ctrl.mode = CombatMode::Ready;
    ctrl.equip_weapon(WeaponGrip::Chainsaw);
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    ctrl.handle_combat_inputs(true, false, false, false, dt);
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_CHAINSAW_LIGHT.id);

    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    for _ in 0..15 {
        ctrl.handle_combat_inputs(true, false, true, false, dt);
    }
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_CHAINSAW_THRUST.id);

    // Maria Chinese Cleaver light (ID 28) & heavy (ID 30)
    ctrl.mode = CombatMode::Ready;
    ctrl.equip_weapon(WeaponGrip::MariaChineseCleaver);
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    ctrl.handle_combat_inputs(true, false, false, false, dt);
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_MARIA_CLEAVER_LIGHT.id);

    ctrl.mode = CombatMode::Ready;
    ctrl.handle_combat_inputs(true, true, true, false, dt);
    for _ in 0..15 {
        ctrl.handle_combat_inputs(true, false, true, false, dt);
    }
    assert_eq!(ctrl.active_attack.unwrap().id, ATK_MARIA_CLEAVER_HEAVY.id);
}

#[test]
fn hyper_spray_cone_taper_shake_recharge_and_color_stun() {
    // 1. 5-ray cone geometry (`shGetHumanAttackSprayPos` 0x00121630):
    //    - Ray 0: center (+Z)
    //    - Rays 1 & 3 (pitch ±0.41887903): unnormalized (0, ±sin(0.41887903), 1) -> angle = atan(sin(0.41887903))
    //    - Rays 2 & 4 (yaw ±0.41887903): exact 0.41887903 rad
    let rays = generate_hyper_spray_rays(Vec3::Z);
    assert!((rays[0] - Vec3::Z).length() < 1e-5);
    let expected_pitch_angle = HYPER_SPRAY_HALF_ANGLE_RAD.sin().atan();
    for &idx in &[1usize, 3] {
        let angle = rays[idx].dot(Vec3::Z).clamp(-1.0, 1.0).acos();
        assert!((angle - expected_pitch_angle).abs() < 1e-4);
    }
    for &idx in &[2usize, 4] {
        let angle = rays[idx].dot(Vec3::Z).clamp(-1.0, 1.0).acos();
        assert!((angle - HYPER_SPRAY_HALF_ANGLE_RAD).abs() < 1e-4);
    }

    // 2. Range taper below 10.0s (`shBattleAttackByHumanFog` 0x00121a40)
    assert_eq!(
        hyper_spray_effective_range(20.0, -125.0, 1000.0),
        (-125.0, 1000.0)
    );
    assert_eq!(
        hyper_spray_effective_range(10.0, -125.0, 1000.0),
        (-125.0, 1000.0)
    );
    assert_eq!(
        hyper_spray_effective_range(5.0, -125.0, 1000.0),
        (-62.5, 500.0)
    );
    assert_eq!(hyper_spray_effective_range(0.0, -125.0, 1000.0), (0.0, 0.0));

    // 3. Shake recharge (`upper_attack` 0x0021ef50): 200 -> 199 -> 19.9s at frame 7.0 (0x1fe)
    let mut ctrl = CombatController::new(false, false);
    ctrl.equip_weapon(WeaponGrip::HyperSpray);
    ctrl.mode = CombatMode::Ready;
    ctrl.hyper_spray_charge = 0.0;
    ctrl.handle_combat_inputs(true, true, true, false, 1.0 / 60.0);
    assert_eq!(ctrl.mode, CombatMode::Reload);
    assert_eq!(ctrl.reload_transfer_frame, 7.0);
    ctrl.anim_frame = 7.0;
    ctrl.handle_combat_inputs(true, false, false, false, 1.0 / 60.0);
    assert_eq!(ctrl.hyper_spray_shakes, 199);
    assert!((ctrl.hyper_spray_charge - 19.9).abs() < 1e-4);

    // 4. Color tiers (`GameSaveSprayPower` 0x00290c20)
    assert_eq!(HyperSprayColor::from_rank_score(0), HyperSprayColor::Purple);
    assert_eq!(HyperSprayColor::from_rank_score(6), HyperSprayColor::White);
    assert_eq!(
        HyperSprayColor::from_rank_score(80),
        HyperSprayColor::Yellow
    );
    assert_eq!(
        HyperSprayColor::from_rank_score(100),
        HyperSprayColor::Green
    );
    assert_eq!(HyperSprayColor::Green.particle_abgr(), 0xff40_ff40);

    // 5. Color stun & Green lethality in CombatArena
    let mut arena = CombatArena::new();
    arena.player.equip_weapon(WeaponGrip::HyperSpray);
    arena.player.mode = CombatMode::Ready;
    arena.player.hyper_spray_color = HyperSprayColor::White;
    // White spray does NOT affect bosses (Pyramid Head = ai.enemies[7])
    arena.ai.enemies[7].snap_pose(Vec3::new(0.0, 0.0, 600.0), 0.0);
    let ph_hp = arena.ai.enemies[7].stats().health;
    arena.tick(true, true, true, 1.0 / 60.0);
    assert_eq!(arena.player.pending_sfx, Some(SE_HYPER_SPRAY_SHOT));
    arena.player.anim_frame = 8.0;
    arena.tick(true, false, true, 1.0 / 60.0);
    assert_eq!(arena.ai.enemies[7].stats().health, ph_hp);

    // Green spray kills regular enemy (Lying Figure = ai.enemies[0]) immediately
    arena.ai.enemies[7].snap_pose(Vec3::new(0.0, 0.0, 5800.0), 0.0);
    arena.ai.enemies[0].snap_pose(Vec3::new(0.0, 0.0, 600.0), 0.0);
    arena.player.hyper_spray_color = HyperSprayColor::Green;
    arena.player.anim_frame = 9.0;
    arena.tick(true, false, true, 1.0 / 60.0);
    assert!(arena.ai.enemies[0].is_dead());
}

#[test]
fn swept_melee_blade_quad_and_3d_shotgun_cone_geometry() {
    // 1. Swept melee blade quad catches enemy swept across arc, rejects enemy beyond max_range
    let origin = Vec3::new(0.0, -1000.0, 200.0);
    let prev_tip = origin + Vec3::new(-300.0, 0.0, 200.0);
    let quad = SweptBladeQuad::new(origin, Vec3::Z, -150.0, 350.0, Some(prev_tip));

    let swept_enemy = CombatHitbox::new_standing(Vec3::new(-120.0, 0.0, 320.0), 100.0, 1700.0);
    assert!(swept_enemy.intersects_swept_quad(&quad));

    // Enemy at Z=1100 is well beyond Plank's 350mm reach (old static 1600mm segment would falsely hit!)
    let far_enemy = CombatHitbox::new_standing(Vec3::new(0.0, 0.0, 1100.0), 150.0, 1700.0);
    assert!(!far_enemy.intersects_swept_quad(&quad));

    // Degenerate first-frame quad (`prev_tip = None` -> `prev_tip == curr_tip`) must NOT falsely hit
    // collinear distant enemies ahead or behind the player!
    let first_frame_quad = SweptBladeQuad::new(origin, Vec3::Z, -150.0, 350.0, None);
    let collinear_far_ahead =
        CombatHitbox::new_standing(Vec3::new(0.0, 0.0, 5000.0), 150.0, 1700.0);
    let collinear_far_behind =
        CombatHitbox::new_standing(Vec3::new(0.0, 0.0, -5000.0), 150.0, 1700.0);
    let collinear_in_range = CombatHitbox::new_standing(Vec3::new(0.0, 0.0, 450.0), 150.0, 1700.0);
    assert!(!collinear_far_ahead.intersects_swept_quad(&first_frame_quad));
    assert!(!collinear_far_behind.intersects_swept_quad(&first_frame_quad));
    assert!(collinear_in_range.intersects_swept_quad(&first_frame_quad));

    // 2. Shotgun 10-pellet 3D cone (`shBattleAttackByHumanGunshotTypeB` 0x00120f30)
    let mut rng_a = CombatRng::new(0x1234_5678);
    let mut rng_b = CombatRng::new(0x1234_5678);
    let rays_a = generate_shotgun_cone_rays(Vec3::Z, &mut rng_a);
    let rays_b = generate_shotgun_cone_rays(Vec3::Z, &mut rng_b);
    assert_eq!(rays_a.len(), SHOTGUN_PELLET_COUNT);
    assert_eq!(rays_a, rays_b);

    let mut has_vertical_spread = false;
    for r in rays_a {
        assert!((r.length() - 1.0).abs() < 1e-5);
        let angle = r.dot(Vec3::Z).clamp(-1.0, 1.0).acos();
        assert!(
            angle <= SHOTGUN_CONE_HALF_ANGLE_RAD + 1e-4,
            "pellet angle {} exceeded 30 deg ({})",
            angle,
            SHOTGUN_CONE_HALF_ANGLE_RAD
        );
        if r.y.abs() > 0.01 {
            has_vertical_spread = true;
        }
    }
    assert!(
        has_vertical_spread,
        "shotgun cone must have 3D vertical spread"
    );
}

#[test]
fn guard_block_auto_aim_and_los_gated_stomp_parity() {
    // 1. Guard/Block (`PlayerChectGuardSuccess` 0x001101d0 / sh2pc 0x00534210)
    assert!(is_guardable_attack(42, 0)); // Acid spray: Beginner/Easy only
    assert!(is_guardable_attack(42, 1));
    assert!(!is_guardable_attack(42, 2)); // Not guardable on Normal/Hard
    assert!(is_guardable_attack(44, 3)); // Strike guardable on Hard
    assert!(!is_guardable_attack(51, 0)); // PH overhead chop not guardable

    // Normal difficulty requires rising edge trigger + hold, facing attacker (> PI/2)
    let mut gs = GuardState::default();
    assert!(!check_guard_success(
        &mut gs, true, 2, false, true, true, 44, 0.0, PI
    ));
    assert!(check_guard_success(
        &mut gs, true, 2, true, true, true, 44, 0.0, PI
    ));
    // Continues guarding while held
    assert!(check_guard_success(
        &mut gs, true, 2, false, true, true, 44, 0.0, PI
    ));
    // Fails if facing same direction as attacker (attacked from behind: delta = 0 <= PI/2)
    assert!(!check_guard_success(
        &mut gs, true, 2, false, true, true, 44, 0.0, 0.0
    ));
    // Boundary: exactly PI/2 fails (strictly > PI/2)
    assert!(!check_guard_success(
        &mut gs, true, 2, false, true, true, 44, 0.0, FRAC_PI_2
    ));
    // Clears latch when no enemy attack is active
    assert!(!check_guard_success(
        &mut gs, false, 2, false, true, true, 44, 0.0, PI
    ));
    assert!(!gs.guard_armed);

    // 2. Auto-Aim (`shBattleGetTargetEnemy` 0x00124390 & `shBattleChangeTargetEnemy` 0x001244d0)
    let candidates = [
        TargetCandidate {
            index: 0,
            position: Vec3::new(0.0, 0.0, 400.0),
            is_downed: true,
            is_dead: false,
        },
        TargetCandidate {
            index: 1,
            position: Vec3::new(300.0, 0.0, 1000.0),
            is_downed: false,
            is_dead: false,
        },
        TargetCandidate {
            index: 2,
            position: Vec3::new(-400.0, 0.0, 900.0),
            is_downed: false,
            is_dead: false,
        },
    ];
    // Prefers upright enemy (index 2, dist ~984) over closer downed enemy (index 0, dist 400)
    assert_eq!(
        select_auto_aim_target(Vec3::ZERO, &candidates, 5000.0, |_| true),
        Some(2)
    );
    // Falls back to downed enemy if upright enemies are occluded
    assert_eq!(
        select_auto_aim_target(Vec3::ZERO, &candidates, 5000.0, |p| p.z < 500.0),
        Some(0)
    );
    // Cycle right (+1) from index 2 selects index 1 (rel_yaw > 0)
    assert_eq!(
        cycle_auto_aim_target(Vec3::ZERO, 0.0, Some(2), 1, &candidates, 5000.0, |_| true),
        Some(1)
    );
    // Cycle left (-1) from index 1 selects index 2 (rel_yaw < 0)
    assert_eq!(
        cycle_auto_aim_target(Vec3::ZERO, 0.0, Some(1), -1, &candidates, 5000.0, |_| true),
        Some(2)
    );

    // 3. Auto-Aim yaw step (`PlayerCheckAimingToEnemy` 0x0021c7a0)
    let dt = 1.0 / 60.0;
    let new_yaw = aim_turn_toward_target(Vec3::ZERO, 0.0, Vec3::new(1000.0, 0.0, 0.0), dt);
    let expected_step = (FRAC_PI_2 / AUTO_AIM_ANGLE_DIVISOR) * dt * AUTO_AIM_TURN_SPEED;
    assert!((new_yaw - expected_step).abs() < 1e-5);

    // 4. LOS-gated Stomp (`shBattleGetNearDeadlyTargetEnemy` 0x00124800, 600.0 mm)
    let stomp_ok = [TargetCandidate {
        index: 0,
        position: Vec3::new(0.0, 0.0, STOMP_TARGET_MAX_DIST_MM),
        is_downed: true,
        is_dead: false,
    }];
    assert_eq!(
        find_near_deadly_target(Vec3::ZERO, &stomp_ok, |_| true),
        Some(0)
    );
    assert_eq!(
        find_near_deadly_target(Vec3::ZERO, &stomp_ok, |_| false),
        None
    );

    let stomp_too_far = [TargetCandidate {
        index: 0,
        position: Vec3::new(0.0, 0.0, STOMP_TARGET_MAX_DIST_MM + 1.0),
        is_downed: true,
        is_dead: false,
    }];
    assert_eq!(
        find_near_deadly_target(Vec3::ZERO, &stomp_too_far, |_| true),
        None
    );
}

#[test]
fn grapple_retail_ids_6dps_no_creeper_and_zero_passive_decay() {
    // Retail PC +6 strike/hug IDs (`sh2pc.exe` 0x00629000 / 0x005340b0)
    assert_eq!(
        (
            SPEC_PYRAMID_HEAD.strike_id,
            SPEC_PYRAMID_HEAD.hug_id,
            SPEC_PYRAMID_HEAD.hold_dps
        ),
        (53, 54, 6.0)
    );
    assert_eq!(
        (
            SPEC_FLESH_LIPS.strike_id,
            SPEC_FLESH_LIPS.hug_id,
            SPEC_FLESH_LIPS.hold_dps
        ),
        (60, 61, 6.0)
    );
    assert_eq!(
        (
            SPEC_ABSTRACT_DADDY.strike_id,
            SPEC_ABSTRACT_DADDY.hug_id,
            SPEC_ABSTRACT_DADDY.hold_dps
        ),
        (63, 64, 6.0)
    );
    assert_eq!(
        (
            SPEC_MARY_BOSS.strike_id,
            SPEC_MARY_BOSS.hug_id,
            SPEC_MARY_BOSS.hold_dps
        ),
        (65, 66, 6.0)
    );
    assert_eq!(
        (
            SPEC_MARY_MOTH_SWARM.strike_id,
            SPEC_MARY_MOTH_SWARM.hug_id,
            SPEC_MARY_MOTH_SWARM.hold_dps
        ),
        (68, 69, 1.5)
    );

    // Retail PC Beginner row matches Easy (`0x005340b0`)
    assert_eq!(PS2_HURIHODOKI_TIMER[0], [0.35, 0.40, 0.50, 0.65, 0.50]);

    // Fabricated Creeper grapple removed
    assert!(get_grapple_spec(BestiaryId::Creeper).is_none());

    // Zero passive decay in GrappleSession::update over 2 full seconds with no button inputs
    let mut arena = CombatArena::new();
    let pap_idx = 6;
    arena.ai.enemies[pap_idx].snap_pose(Vec3::new(0.0, 0.0, 350.0), 0.0);
    arena.ai.enemies[pap_idx].set_attacking(true);
    arena.tick(false, false, false, 1.0 / 60.0);
    assert_eq!(arena.player.mode, CombatMode::Held);
    let initial_gauge = arena.grapple_session.as_ref().unwrap().struggle_gauge;
    let initial_hp = arena.player.health;

    // Advance 60 ticks (1.0 second) with zero struggle input
    for _ in 0..60 {
        arena.tick(false, false, false, 1.0 / 60.0);
    }
    assert_eq!(arena.player.mode, CombatMode::Held);
    assert_eq!(
        arena.grapple_session.as_ref().unwrap().struggle_gauge,
        initial_gauge
    );
    // 6.0 DPS hold damage applied over 1.0 second
    assert!((initial_hp - arena.player.health - 6.0).abs() < 0.2);
}
