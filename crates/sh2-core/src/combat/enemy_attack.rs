//! Authentic enemy-to-player attack dispatchers (`shBattleAttackByEnemy*`, 1:1 PS2/PC parity).
//!
//! # Ground Truth & Binary References
//! - **`SLUS_202.28` (`docs/decomp/slus/Chacter/sh_character_battle.c`):**
//!   - `shBattleAttackByEnemySlash` (`0x00122310`): multi-frame swept blade quad (`attack_type == 1`).
//!   - `shBattleAttackByEnemyStrike` (`0x00122550`): single-hit swept melee quad (`attack_type == 2`).
//!   - `shBattleAttackByEnemyFog` (`0x00122950`): 5-ray expanding acid cone (`1.8` indoor / `3.6` outdoor × `500.0`).
//!   - `shBattleAttackByEnemyBite` (`0x00122b80`): empty no-op stub.
//!   - `shBattleAttackByEnemyHug` (`0x00122b90`): swept grapple initiation (`attack_type == 6`).
//!   - `shBattleAttackByEnemyNeedle` (`0x00122d10`): 2-ray needle/tentacle probe (`attack_type == 3`, ARM/IKE).
//!   - `shBattleAttackByEnemyShot` (`0x00122eb0`): single-shot firearm ray (`attack_type == 3`, EDB).
//!   - `shBattleExecAttackQueue` (`0x00123760` lines 1784–1810): enemy attack type dispatch switch.
//!   - `shBattleAttackHitCheckInit` (`0x00123130`): resets `sfx_played`, `prev_weapon_tip`, and `hit_already`.
//! - **`sh2pc.exe` Retail Drift:**
//!   - Retail `sh2_attack_list` shifts all enemy attack IDs by **+6** compared to `SLUS_202.28`
//!     (due to Maria's Born From a Wish weapons occupying IDs `25..=30`).

use super::attack_list::AttackSpec;
use super::hitboxes::{generate_hyper_spray_rays, CombatHitbox, SweptBladeQuad};
use super::targeting::{check_guard_success, GuardState};
use crate::ai::creatures::{EnemySize, EnemySpecies};
use glam::Vec3;

/// Enemy attack dispatch variant selected by `shBattleExecAttackQueue` (`0x00123760`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyAttackDispatcher {
    /// `shBattleAttackByEnemySlash` (`0x00122310`).
    Slash,
    /// `shBattleAttackByEnemyStrike` (`0x00122550`).
    Strike,
    /// `shBattleAttackByEnemyFog` (`0x00122950`).
    Fog,
    /// `shBattleAttackByEnemyBite` (`0x00122b80`, no-op).
    Bite,
    /// `shBattleAttackByEnemyHug` (`0x00122b90`).
    Hug,
    /// `shBattleAttackByEnemyNeedle` (`0x00122d10`).
    Needle,
    /// `shBattleAttackByEnemyShot` (`0x00122eb0`).
    Shot,
}

/// Per-enemy attack hit-check work state (`shBattleAttackHitCheckInit` `0x00123130`).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnemyAttackWork {
    /// `chara + 0x140`: set to `true` once a single-hit attack (`Strike`, `Fog`, `Shot`) fires/connects.
    pub hit_already: bool,
    /// `chara + 0x184`: set to `true` once the swing/spray/needle SFX has been emitted.
    pub sfx_played: bool,
    /// `chara + 0x150`: previous-frame weapon tip for swept quad construction.
    pub prev_weapon_tip: Option<Vec3>,
}

impl EnemyAttackWork {
    /// Reset attack work at the start of an enemy attack (`shBattleAttackHitCheckInit` `0x00123130`).
    pub fn init(&mut self) {
        self.hit_already = false;
        self.sfx_played = false;
        self.prev_weapon_tip = None;
    }
}

/// Guard/Block input state passed into [`dispatch_enemy_attack_ex`]
/// (`PlayerChectGuardSuccess` `SLUS_202.28` `0x001101d0` / `sh2pc.exe` `0x00534210`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnemyAttackGuardInput {
    /// Rising-edge Guard/Block trigger (`shPadTrigger`).
    pub guard_triggered: bool,
    /// Held Guard/Block button state (`shPadPress`).
    pub guard_held: bool,
    /// True when the player's current combat mode is eligible to guard (`lower_st < 0x14 && lower_st != 0x11`).
    pub player_can_guard: bool,
}

/// Retail PC `sh2_attack_list` ID for Pyramid Head's Great Knife overhead chop instant-kill
/// (`shBattleDamageRevise` `SLUS_202.28` `0x001205a0` `0x2d` -> `sh2pc.exe` `0x005434a0` `0x33` = `51`).
pub const PH_OVERHEAD_CHOP_ATTACK_ID: u32 = 51;

/// Chip-damage multiplier applied on successful Guard/Block
/// (`SLUS_202.28` `Chacter/m3_play.c:1444`: `hp -= incoming * 0.1`).
pub const GUARD_CHIP_DAMAGE_SCALE: f32 = 0.1;

/// Result of evaluating one frame of an enemy attack against the player.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EnemyAttackOutcome {
    /// Whether the attack geometry intersected the player's hitbox.
    pub hit_player: bool,
    /// Damage AP to apply to James (`atk.ap * 0.1` when guarded per `m3_play.c:1444`, `0.0` if missed).
    pub damage_ap: f32,
    /// Retail `sh2_attack_list` ID of the attack.
    pub attack_id: u32,
    /// True if a `Hug` (`attack_type == 6`) connected and should initiate a `GrappleSession`.
    pub is_hug: bool,
    /// True if a `Fog` (`attack_type == 4`) acid spit connected.
    pub is_fog: bool,
    /// True if `PlayerChectGuardSuccess` (`0x001101d0`) blocked the hit (10% chip damage applies).
    pub guarded: bool,
    /// True if Pyramid Head's overhead chop (`51` retail / `0x2d` proto) connected and must force
    /// `hp = -1.0` (`0xbf800000`, `shBattleDamageRevise` `0x001205a0` / `sh2pc.exe` `0x005434a0`).
    pub instant_kill: bool,
    /// Optional PS2 SFX ID triggered this frame (e.g. `12000` acid spit, `0x4719` Eddie shot,
    /// `0x2fdc`/`0x2fdd` randomized needle).
    pub sfx_id: Option<u32>,
}

/// Select the authentic `shBattleAttackByEnemy*` dispatcher (`shBattleExecAttackQueue` `0x00123760`).
///
/// Accounts for the retail PC **+6** enemy attack ID shift (`sh2pc.exe` `0x00629000`).
pub fn select_enemy_dispatcher(atk: &AttackSpec) -> Option<EnemyAttackDispatcher> {
    match atk.kind {
        1 => Some(EnemyAttackDispatcher::Slash),
        2 => Some(EnemyAttackDispatcher::Strike),
        3 => match atk.id {
            // Retail 70/71 (proto 0x40/0x41 = 64/65): Mandarin/Flesh Lips needle
            70 | 71 => Some(EnemyAttackDispatcher::Needle),
            // Retail 57/58 (proto 0x33/0x34 = 51/52): Eddie revolver shot
            57 | 58 => Some(EnemyAttackDispatcher::Shot),
            // Retail 47/49/55 (proto 0x29/0x2b/0x31 = 41/43/49): Nurse/PH strike
            _ => Some(EnemyAttackDispatcher::Strike),
        },
        4 => Some(EnemyAttackDispatcher::Fog),
        5 => Some(EnemyAttackDispatcher::Bite),
        6 => Some(EnemyAttackDispatcher::Hug),
        _ => None,
    }
}

/// Evaluate `shBattleAttackByEnemyFog` (`0x00122950`) instantaneous spray range at `anim_frame`.
///
/// Exact formula (`0x00122950` lines 1274–1280):
/// `((anim_frame - first_hit_frame) * scale * 500.0) / (last_hit_frame - first_hit_frame)`
/// where `scale = 3.6` outdoor, `1.8` indoor.
pub fn enemy_fog_range_at_frame(atk: &AttackSpec, anim_frame: u8, is_outdoor: bool) -> f32 {
    if anim_frame < atk.first_hit_frame || anim_frame > atk.last_hit_frame {
        return 0.0;
    }
    let scale = if is_outdoor { 3.6 } else { 1.8 };
    let span = atk
        .last_hit_frame
        .saturating_sub(atk.first_hit_frame)
        .max(1) as f32;
    let elapsed = (anim_frame - atk.first_hit_frame) as f32;
    (elapsed * scale * 500.0) / span
}

/// Dispatch an active enemy attack against James (`shBattleExecAttackQueue` `0x00123760`
/// + `shBattleAttackByEnemy{Slash,Strike,Fog,Hug,Needle,Shot}` `0x00122310..0x00122eb0`).
///
/// Convenience wrapper over [`dispatch_enemy_attack_ex`] using `player_guard.guard_armed` as
/// held-only guard input and `rng_bits = 0`. Callers with live controller input (such as
/// [`crate::combat::CombatArena`]) should use [`dispatch_enemy_attack_ex`].
#[allow(clippy::too_many_arguments)]
pub fn dispatch_enemy_attack(
    species: EnemySpecies,
    atk: &AttackSpec,
    work: &mut EnemyAttackWork,
    anim_frame: u8,
    enemy_pos: Vec3,
    enemy_yaw: f32,
    player_pos: Vec3,
    player_yaw: f32,
    player_guard: &mut GuardState,
    difficulty: u8,
    is_outdoor: bool,
) -> EnemyAttackOutcome {
    let guard_input = EnemyAttackGuardInput {
        guard_triggered: false,
        guard_held: player_guard.guard_armed,
        player_can_guard: true,
    };
    dispatch_enemy_attack_ex(
        species,
        atk,
        work,
        anim_frame,
        enemy_pos,
        enemy_yaw,
        player_pos,
        player_yaw,
        player_guard,
        guard_input,
        difficulty,
        is_outdoor,
        0,
    )
}

/// Full-fidelity enemy attack dispatcher accepting explicit Guard/Block inputs
/// (`PlayerChectGuardSuccess` `0x001101d0`) and deterministic `rng_bits` for Needle SFX
/// randomization (`(shRandI() >> 10 & 1) + 0x2fdc` in `shBattleAttackByEnemyNeedle` `0x00122d10`).
#[allow(clippy::too_many_arguments)]
pub fn dispatch_enemy_attack_ex(
    species: EnemySpecies,
    atk: &AttackSpec,
    work: &mut EnemyAttackWork,
    anim_frame: u8,
    enemy_pos: Vec3,
    enemy_yaw: f32,
    player_pos: Vec3,
    player_yaw: f32,
    player_guard: &mut GuardState,
    guard_input: EnemyAttackGuardInput,
    difficulty: u8,
    is_outdoor: bool,
    rng_bits: u32,
) -> EnemyAttackOutcome {
    let Some(dispatcher) = select_enemy_dispatcher(atk) else {
        return EnemyAttackOutcome::default();
    };

    let forward = Vec3::new(enemy_yaw.sin(), 0.0, enemy_yaw.cos());
    // Derive fallback attack origin from authentic `enSetSize` (`0x001426e0`) center_y and body radius
    // (approximating `shGetEnemyAttackStartPos` `0x00122150` when skeletal bone matrices are absent).
    let enemy_size = EnemySize::standing_for(species);
    let attack_origin =
        enemy_pos + Vec3::new(0.0, -enemy_size.center_y, 0.0) + forward * enemy_size.radius;
    // James standing collision cylinder (`enGetPlayerSize` = 200.0 mm radius, 1700.0 mm tall).
    let player_hitbox = CombatHitbox::new_standing(player_pos, 200.0, 1700.0);

    let in_window = anim_frame >= atk.first_hit_frame && anim_frame <= atk.last_hit_frame;
    let mut outcome = EnemyAttackOutcome {
        attack_id: atk.id,
        ..EnemyAttackOutcome::default()
    };

    match dispatcher {
        EnemyAttackDispatcher::Slash => {
            // `shBattleAttackByEnemySlash` (`0x00122310`): swept blade quad every active frame.
            let quad = SweptBladeQuad::new(
                attack_origin,
                forward,
                atk.min_range,
                atk.max_range,
                work.prev_weapon_tip,
            );
            work.prev_weapon_tip = Some(quad.curr_tip);

            if in_window {
                if !work.sfx_played {
                    if matches!(atk.id, 50 | 51) {
                        outcome.sfx_id = Some(0x3eea);
                    }
                    work.sfx_played = true;
                }
                if player_hitbox.intersects_swept_quad(&quad) {
                    apply_hit_with_guard(
                        &mut outcome,
                        atk,
                        enemy_yaw,
                        player_yaw,
                        player_guard,
                        guard_input,
                        difficulty,
                    );
                }
            }
        }

        EnemyAttackDispatcher::Strike => {
            // `shBattleAttackByEnemyStrike` (`0x00122550`): single-hit swept melee quad.
            let prev_tip = work.prev_weapon_tip;
            let quad = SweptBladeQuad::new(
                attack_origin,
                forward,
                atk.min_range,
                atk.max_range,
                prev_tip,
            );
            work.prev_weapon_tip = Some(quad.curr_tip);

            if in_window && !work.hit_already {
                if !work.sfx_played {
                    outcome.sfx_id = strike_sfx_for_retail_attack_id(atk.id);
                    work.sfx_played = true;
                }
                if prev_tip.is_some() && player_hitbox.intersects_swept_quad(&quad) {
                    work.hit_already = true;
                    apply_hit_with_guard(
                        &mut outcome,
                        atk,
                        enemy_yaw,
                        player_yaw,
                        player_guard,
                        guard_input,
                        difficulty,
                    );
                }
            }
        }

        EnemyAttackDispatcher::Fog => {
            // `shBattleAttackByEnemyFog` (`0x00122950`): 5-ray acid cone with expanding reach.
            if in_window && !work.hit_already {
                let range = enemy_fog_range_at_frame(atk, anim_frame, is_outdoor);
                if !work.sfx_played {
                    outcome.sfx_id = Some(12_000);
                    work.sfx_played = true;
                }
                if range > 0.0 {
                    let rays = generate_hyper_spray_rays(forward);
                    for dir in rays {
                        if player_hitbox
                            .intersects_ray(attack_origin, dir, 0.0, range)
                            .is_some()
                        {
                            work.hit_already = true;
                            outcome.is_fog = true;
                            apply_hit_with_guard(
                                &mut outcome,
                                atk,
                                enemy_yaw,
                                player_yaw,
                                player_guard,
                                guard_input,
                                difficulty,
                            );
                            break;
                        }
                    }
                }
            }
        }

        EnemyAttackDispatcher::Bite => {
            // `shBattleAttackByEnemyBite` (`0x00122b80`): no-op.
        }

        EnemyAttackDispatcher::Hug => {
            // `shBattleAttackByEnemyHug` (`0x00122b90`): swept grab check.
            let prev_tip = work.prev_weapon_tip;
            let quad = SweptBladeQuad::new(
                attack_origin,
                forward,
                atk.min_range,
                atk.max_range,
                prev_tip,
            );
            work.prev_weapon_tip = Some(quad.curr_tip);

            if prev_tip.is_some() && player_hitbox.intersects_swept_quad(&quad) {
                outcome.hit_player = true;
                outcome.is_hug = true;
            }
        }

        EnemyAttackDispatcher::Needle => {
            // `shBattleAttackByEnemyNeedle` (`0x00122d10` / `sh2pc 0x00540000.c:3371`):
            // 2-ray needle/tentacle probe with randomized SFX `(shRandI() >> 10 & 1) + 0x2fdc`
            // (`0x2fdc` or `0x2fdd`).
            let right = Vec3::new(enemy_yaw.cos(), 0.0, -enemy_yaw.sin());
            if !work.sfx_played {
                outcome.sfx_id = Some(0x2fdc + ((rng_bits >> 10) & 1));
                work.sfx_played = true;
            }
            // Derive dual arm/tentacle lateral offset from `enSetSize` (`0x001426e0`) body radius.
            let lateral_offset = enemy_size.radius * 0.5;
            for side in [-1.0_f32, 1.0_f32] {
                let origin = attack_origin + right * (side * lateral_offset);
                if player_hitbox
                    .intersects_ray(origin, forward, atk.min_range, atk.max_range)
                    .is_some()
                {
                    apply_hit_with_guard(
                        &mut outcome,
                        atk,
                        enemy_yaw,
                        player_yaw,
                        player_guard,
                        guard_input,
                        difficulty,
                    );
                    break;
                }
            }
        }

        EnemyAttackDispatcher::Shot => {
            // `shBattleAttackByEnemyShot` (`0x00122eb0`): single-shot firearm ray.
            if in_window && !work.hit_already {
                work.hit_already = true;
                if species == EnemySpecies::Eddie {
                    outcome.sfx_id = Some(0x4719);
                }
                if player_hitbox
                    .intersects_ray(attack_origin, forward, atk.min_range, atk.max_range)
                    .is_some()
                {
                    apply_hit_with_guard(
                        &mut outcome,
                        atk,
                        enemy_yaw,
                        player_yaw,
                        player_guard,
                        guard_input,
                        difficulty,
                    );
                }
            }
            if anim_frame > atk.last_hit_frame {
                work.hit_already = false;
            }
        }
    }

    outcome
}

fn apply_hit_with_guard(
    outcome: &mut EnemyAttackOutcome,
    atk: &AttackSpec,
    enemy_yaw: f32,
    player_yaw: f32,
    player_guard: &mut GuardState,
    guard_input: EnemyAttackGuardInput,
    difficulty: u8,
) {
    if check_guard_success(
        player_guard,
        true,
        difficulty,
        guard_input.guard_triggered,
        guard_input.guard_held,
        guard_input.player_can_guard,
        atk.id,
        player_yaw,
        enemy_yaw,
    ) {
        // `SLUS_202.28` `Chacter/m3_play.c:1440-1448`: successful Guard reduces incoming damage
        // to 10% chip damage (`incoming * 0.1`), NOT `0.0`.
        outcome.hit_player = true;
        outcome.guarded = true;
        outcome.damage_ap = atk.ap * GUARD_CHIP_DAMAGE_SCALE;
    } else {
        outcome.hit_player = true;
        outcome.damage_ap = atk.ap;
        // `shBattleDamageRevise` (`SLUS_202.28` `0x001205a0` / `sh2pc.exe` `0x005434a0`):
        // Pyramid Head Great Knife overhead chop (`0x2d` proto / `0x33` = `51` retail) writes
        // `-1.0f` (`0xbf800000`) directly to player HP (instant kill).
        if atk.id == PH_OVERHEAD_CHOP_ATTACK_ID {
            outcome.instant_kill = true;
        }
    }
}

/// Retail-shifted strike SFX table from `shBattleAttackByEnemyStrike` (`0x00122550` lines 1200–1228).
fn strike_sfx_for_retail_attack_id(retail_id: u32) -> Option<u32> {
    match retail_id {
        67 => Some(0x49a2),      // proto 0x3d (Mary Boss)
        65 => Some(0x49a4),      // proto 0x3b (Mary Boss)
        60 => Some(0x4848),      // proto 0x36 (Flesh Lips)
        62 => Some(0x4844),      // proto 0x38 (Flesh Lips)
        56 => Some(0x3eed),      // proto 0x32 (Pyramid Head)
        55 => Some(0x3eec),      // proto 0x31 (Pyramid Head)
        47 | 49 => Some(0x2f45), // proto 0x29/0x2b (Nurse)
        46 | 48 => Some(0x2f46), // proto 0x28/0x2a (Nurse)
        _ => None,
    }
}
