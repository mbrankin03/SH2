//! Guard/Block, Auto-Aim Target Selection, Target Cycling, and LOS-Gated Stomp Detection.
//!
//! # Ground Truth & Binary References
//! - **`PlayerChectGuardSuccess` (`SLUS_202.28` `0x001101d0` / `sh2pc.exe` `0x00534210`):**
//!   Guard activation by difficulty mode (hold on Beginner/Easy, trigger+hold on Normal/Hard),
//!   retail PC guardable attack ID set (`0x2a` Easy-only; `0x2c..=0x31, 0x34, 0x3b, 0x3e, 0x43` all
//!   difficulties), and head-on facing check (`|wrap_pi(attacker_yaw - player_yaw)| > PI/2`).
//! - **`shBattleCheckTargetChara` (`SLUS_202.28` `0x00123f70`):**
//!   Up to `20` (`0x14`) distance-sorted target candidates.
//! - **`shBattleGetTargetEnemy` (`SLUS_202.28` `0x00124390`):**
//!   Nearest living enemy with clear LOS, prioritizing upright (`!is_downed`) enemies over downed ones.
//! - **`shBattleChangeTargetEnemy` (`SLUS_202.28` `0x001244d0`):**
//!   Cycles to nearest living enemy != current target on the requested side (`+1` right / `-1` left)
//!   passing LOS.
//! - **`PlayerCheckAimingToEnemy` (`SLUS_202.28` `0x0021c7a0`):**
//!   Auto-aim yaw interpolation: `step = (delta / 3.0915928) * dt * 15.0`.
//! - **`shBattleGetNearDeadlyTargetEnemy` (`SLUS_202.28` `0x00124800`):**
//!   Nearest downed living enemy within `600.0` mm passing clear LOS (`clCheckHitEyes`).

use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Maximum number of target slots tracked by `sh2_target_info` (`0x14` = `20`, `0x00123f70`).
pub const MAX_TARGET_SLOTS: usize = 20;
/// Maximum distance in PS2 world mm for stomp/kick finisher targeting
/// (`shBattleGetNearDeadlyTargetEnemy`, `SLUS_202.28` `0x00124800` line 353).
pub const STOMP_TARGET_MAX_DIST_MM: f32 = 600.0;
/// Angular divisor constant in `PlayerCheckAimingToEnemy` (`SLUS_202.28` `0x0021c7a0` line 356).
pub const AUTO_AIM_ANGLE_DIVISOR: f32 = 3.091_592_8;
/// Angular speed multiplier in `PlayerCheckAimingToEnemy` (`SLUS_202.28` `0x0021c7a0` line 356).
pub const AUTO_AIM_TURN_SPEED: f32 = 15.0;
/// Minimum facing-angle delta between attacker and player for Guard/Block success
/// (`1.5707964` rad = `PI / 2`, `PlayerChectGuardSuccess` `0x001101d0` / `0x00534210`).
pub const GUARD_MIN_FACING_DELTA_RAD: f32 = FRAC_PI_2;

/// Wrap an angle in radians to `[-PI, PI]` (`shAngleRegulate`).
#[inline]
pub fn wrap_angle_pi(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= TAU;
    }
    while angle < -PI {
        angle += TAU;
    }
    angle
}

/// Persistent guard latch state (`DAT_003c83b7` / `DAT_01fb80b7` in `PlayerChectGuardSuccess`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GuardState {
    /// Whether the guard latch is currently armed (`DAT_003c83b7` / `DAT_01fb80b7`).
    pub guard_armed: bool,
}

/// Returns `true` if `attack_id` (Retail PC `sh2_attack_list` index `0..=71`) can be blocked
/// at the given `difficulty` (`0` = Beginner, `1` = Easy, `2` = Normal, `3` = Hard).
///
/// Verified 1:1 against `sh2pc.exe` `0x00534210` lines 1661–1675 (Retail PC +6 shifted from
/// `SLUS_202.28` `0x001101d0` lines 5094–5110):
/// - `42` (`0x2a`, Lying Figure acid spray): guardable on Beginner/Easy (`difficulty <= 1`) only.
/// - `44, 45, 46, 47, 48, 49, 52, 59, 62, 67` (`0x2c..=0x31, 0x34, 0x3b, 0x3e, 0x43`):
///   guardable on all difficulties.
pub const fn is_guardable_attack(attack_id: u32, difficulty: u8) -> bool {
    match attack_id {
        42 => difficulty <= 1,
        44 | 45 | 46 | 47 | 48 | 49 | 52 | 59 | 62 | 67 => true,
        _ => false,
    }
}

/// Evaluate `PlayerChectGuardSuccess` (`SLUS_202.28` `0x001101d0` / `sh2pc.exe` `0x00534210`).
///
/// - `enemy_attack_active`: result of `shBattleCheckAttackByEnemy()`.
/// - `difficulty`: `0` = Beginner, `1` = Easy, `2` = Normal, `3` = Hard.
/// - `guard_triggered`: rising-edge trigger of Guard/Block button (`shPadTrigger`).
/// - `guard_held`: held state of Guard/Block button (`shPadPress`).
/// - `player_can_guard`: `true` when player state is eligible (`lower_st < 0x14 && lower_st != 0x11`).
/// - `incoming_attack_id`: Retail PC `sh2_attack_list` ID of the incoming enemy strike.
/// - `player_yaw`: player facing yaw in radians.
/// - `attacker_yaw`: attacker facing yaw in radians.
#[allow(clippy::too_many_arguments)]
pub fn check_guard_success(
    state: &mut GuardState,
    enemy_attack_active: bool,
    difficulty: u8,
    guard_triggered: bool,
    guard_held: bool,
    player_can_guard: bool,
    incoming_attack_id: u32,
    player_yaw: f32,
    attacker_yaw: f32,
) -> bool {
    if !enemy_attack_active {
        state.guard_armed = false;
        return false;
    }

    if difficulty <= 1 {
        // Beginner / Easy (mode 1): holding Guard arms block continuously.
        state.guard_armed = guard_held;
    } else {
        // Normal / Hard (mode 2): must press Guard on rising edge and continue holding.
        if !state.guard_armed {
            if guard_triggered {
                state.guard_armed = true;
            }
        } else if !guard_held {
            state.guard_armed = false;
        }
    }

    if !state.guard_armed || !player_can_guard {
        return false;
    }

    if !is_guardable_attack(incoming_attack_id, difficulty) {
        return false;
    }

    let delta = wrap_angle_pi(attacker_yaw - player_yaw).abs();
    delta > GUARD_MIN_FACING_DELTA_RAD
}

/// Lightweight target descriptor used by `shBattleGetTargetEnemy`, `shBattleChangeTargetEnemy`,
/// and `shBattleGetNearDeadlyTargetEnemy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetCandidate {
    /// Index of the enemy in the caller's entity list.
    pub index: usize,
    /// World-space position of the enemy (PS2 mm).
    pub position: Vec3,
    /// Whether the enemy is currently downed/knocked down (`*(u32 *)(chara + 0x170) & 4 != 0`).
    pub is_downed: bool,
    /// Whether the enemy is dead (`*(u32 *)(chara + 0x170) & 2 != 0`).
    pub is_dead: bool,
}

/// Build up to `MAX_TARGET_SLOTS` (`20`) living target indices sorted ascending by 3D distance
/// from `player_pos` (`shBattleCheckTargetChara`, `SLUS_202.28` `0x00123f70`).
fn sorted_living_candidates(
    player_pos: Vec3,
    candidates: &[TargetCandidate],
    max_range: f32,
) -> Vec<(usize, TargetCandidate, f32)> {
    let mut list: Vec<(usize, TargetCandidate, f32)> = candidates
        .iter()
        .copied()
        .filter(|c| !c.is_dead)
        .filter_map(|c| {
            let dist = (c.position - player_pos).length();
            if dist.is_finite() && dist <= max_range {
                Some((c.index, c, dist))
            } else {
                None
            }
        })
        .collect();
    list.sort_by(|a, b| a.2.total_cmp(&b.2));
    list.truncate(MAX_TARGET_SLOTS);
    list
}

/// Select the primary auto-aim target (`shBattleGetTargetEnemy`, `SLUS_202.28` `0x00124390`).
///
/// Iterates nearest-first through living enemies within `max_range`, checking `has_los(enemy_pos)`.
/// Immediately returns the nearest **upright** (`!is_downed`) enemy with LOS; if none are upright,
/// falls back to the nearest **downed** enemy with LOS.
pub fn select_auto_aim_target<F>(
    player_pos: Vec3,
    candidates: &[TargetCandidate],
    max_range: f32,
    mut has_los: F,
) -> Option<usize>
where
    F: FnMut(Vec3) -> bool,
{
    let sorted = sorted_living_candidates(player_pos, candidates, max_range);
    let mut downed_fallback: Option<usize> = None;

    for (idx, cand, _dist) in sorted {
        if !has_los(cand.position) {
            continue;
        }
        if !cand.is_downed {
            return Some(idx);
        }
        if downed_fallback.is_none() {
            downed_fallback = Some(idx);
        }
    }

    downed_fallback
}

/// Cycle the active auto-aim target left (`direction == -1`) or right (`direction == 1`)
/// (`shBattleChangeTargetEnemy`, `SLUS_202.28` `0x001244d0`).
///
/// Finds the nearest living enemy `!= current_target` whose signed relative yaw
/// `wrap_angle_pi(atan2(dx, dz) - player_yaw)` is positive (`direction == 1`) or negative
/// (`direction == -1`) and has clear LOS.
pub fn cycle_auto_aim_target<F>(
    player_pos: Vec3,
    player_yaw: f32,
    current_target: Option<usize>,
    direction: i8,
    candidates: &[TargetCandidate],
    max_range: f32,
    mut has_los: F,
) -> Option<usize>
where
    F: FnMut(Vec3) -> bool,
{
    if direction != 1 && direction != -1 {
        return None;
    }

    let sorted = sorted_living_candidates(player_pos, candidates, max_range);
    for (idx, cand, _dist) in sorted {
        if current_target == Some(idx) {
            continue;
        }
        let dx = cand.position.x - player_pos.x;
        let dz = cand.position.z - player_pos.z;
        let to_enemy_yaw = dx.atan2(dz);
        let rel_yaw = wrap_angle_pi(to_enemy_yaw - player_yaw);

        let side_ok = (direction == 1 && rel_yaw > 0.0) || (direction == -1 && rel_yaw < 0.0);
        if side_ok && has_los(cand.position) {
            return Some(idx);
        }
    }

    None
}

/// Smoothly rotate `player_yaw` toward `target_pos` (`PlayerCheckAimingToEnemy`,
/// `SLUS_202.28` `0x0021c7a0` lines 351–373).
///
/// Formula: `step = (wrap_angle_pi(player_yaw - target_yaw) / 3.0915928) * dt * 15.0`,
/// clamping cleanly to `target_yaw` without overshoot.
pub fn aim_turn_toward_target(player_pos: Vec3, player_yaw: f32, target_pos: Vec3, dt: f32) -> f32 {
    let target_yaw = (target_pos.x - player_pos.x).atan2(target_pos.z - player_pos.z);
    let delta = wrap_angle_pi(player_yaw - target_yaw);
    let step = (delta / AUTO_AIM_ANGLE_DIVISOR) * dt * AUTO_AIM_TURN_SPEED;

    let new_yaw = if delta >= 0.0 {
        if delta - step <= 0.0 {
            target_yaw
        } else {
            player_yaw - step
        }
    } else if delta - step >= 0.0 {
        target_yaw
    } else {
        player_yaw - step
    };

    wrap_angle_pi(new_yaw)
}

/// Find the nearest downed enemy eligible for stomp/kick finisher
/// (`shBattleGetNearDeadlyTargetEnemy`, `SLUS_202.28` `0x00124800`).
///
/// Requires `is_downed && !is_dead`, 3D distance `<= 600.0` mm (`STOMP_TARGET_MAX_DIST_MM`),
/// AND clear LOS (`has_los(enemy_pos)`).
pub fn find_near_deadly_target<F>(
    player_pos: Vec3,
    candidates: &[TargetCandidate],
    mut has_los: F,
) -> Option<usize>
where
    F: FnMut(Vec3) -> bool,
{
    let sorted = sorted_living_candidates(player_pos, candidates, STOMP_TARGET_MAX_DIST_MM);
    for (idx, cand, dist) in sorted {
        if cand.is_downed && dist <= STOMP_TARGET_MAX_DIST_MM && has_los(cand.position) {
            return Some(idx);
        }
    }
    None
}
