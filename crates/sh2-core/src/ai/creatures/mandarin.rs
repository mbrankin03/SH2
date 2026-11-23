//! Mandarin (`chr/arm`, kind `0x20a`, internal type `0x0a`) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_arm.c`):
//!   - `0x002396b0` `enARMInitData` — hanging under-grating creature, size `(300, 500, 600, 600)`.
//!   - `0x00239820` `enARMCtrlAutomatic` — 4-state dispatch (`0x0034ea50`): Wander, Turn, Damage, Dead.
//!   - `0x00239a00` `enARMCtrlWander` — hand-over-hand traversal, attack streak cap (`>4` -> 180-tick cooldown).
//!   - `0x00239d50` `enARMCtrlTurn` — 3-step turn avoidance (`0.05235988` rad/f).
//!   - `0x0023a400` `enARMCanSeePlayer` — 2500 mm / 120° sight, 1000 mm attack range.
//!   - `0x0023ab30` `enARMGetMoveSpeed` — `mult[mode] * (hp/250.0 + 0.6) * (dist>5000 ? 4.0 : 1.0)`.
//! - Retail PC (`sh2pc.exe`) drift:
//!   - HP/Endurance (`0x004acc06`): Beginner `500.0 -> 300.0` (`[300, 800, 1200, 2500, 4000]`).
//!   - Attack IDs (`+6` shift): Left/Right tongue-arm strike `0x40`/`0x41` -> `0x46`/`0x47` (`0x004ad26b`, `0x004ad38e`).
//!   - Forward floor check distance (`0x004ace60`): `1000.0 -> 500.0` mm.

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail PC Mandarin HP & endurance table (`sh2pc 0x004acc06`; SLUS `0x0034ea10` had `500.0` for Beginner).
pub const ARM_HP_PC: [f32; 5] = [300.0, 800.0, 1200.0, 2500.0, 4000.0];
/// Mandarin move speed multipliers (`SLUS 0x0034ea60` / `sh2pc 0x004acb13`).
pub const ARM_MOVE_SPEED_MULT: [f32; 5] = [0.7, 0.8, 1.0, 1.2, 1.5];

/// Sight distance (`2500.0` mm) & half-angle (`120°` = `2.0943952` rad, `enARMCanSeePlayer` `0x0023a400`).
pub const ARM_SIGHT_DIST_MM: f32 = 2500.0;
pub const ARM_SIGHT_HALF_ANGLE_RAD: f32 = 2.094_395_2;
/// Attack reach (`1000.0` mm, `enARMCanSeePlayer` `0x0023a400`).
pub const ARM_ATTACK_DIST_MM: f32 = 1000.0;
/// Retail PC forward grating check distance (`500.0` mm, `sh2pc 0x004ace60`; SLUS was `1000.0`).
pub const ARM_FORWARD_FLOOR_CHECK_PC_MM: f32 = 500.0;
/// Cooldown ticks after >4 consecutive attacks or losing target (`0xb4` = `180` ticks, `enARMCtrlWander` `0x00239a00`).
pub const ARM_COOLDOWN_TICKS: f32 = 180.0;
/// Turn rate (`3°` = `0.05235988` rad/tick, `enARMCtrlWander` / `enARMCtrlTurn`).
pub const ARM_ROT_STEP_RAD: f32 = 0.052_359_88;

/// Retail PC attack IDs (`+6` over SLUS `0x40`/`0x41`, `sh2pc 0x004ad26b` / `0x004ad38e`).
pub const ARM_ATTACK_ID_LEFT_PC: u16 = 0x46;
pub const ARM_ATTACK_ID_RIGHT_PC: u16 = 0x47;

/// Mandarin state machine enum matching `0x0034ea50` (`enARMCtrlAutomatic` `SLUS 0x00239820`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum ArmState {
    #[default]
    Wander = 0,
    Turn = 1,
    Damage = 2,
    Dead = 3,
}

/// Mandarin AI controller (`en_arm.c`, kind `0x20a`).
#[derive(Debug, Clone)]
pub struct MandarinController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub state: ArmState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    /// Active swinging hand (`0` = left/bone `0xb`, `1` = right/bone `0xc`, `None` = idle).
    pub active_hand: Option<u8>,
    /// Consecutive attack count (`work+0xc9`); >4 triggers 180-tick cooldown.
    pub attack_streak: u8,
    /// Turn direction `+1.0` or `-1.0` (`work+0xcd`).
    pub turn_dir: f32,
    pub cooldown_ticks: f32,
    pub step_timer_ticks: f32,
    pub active_attack_id: Option<u16>,
    /// True once `enARMCtrlDead` 60-tick timer expires (`enDeleteCharacter` `0x0023a210`).
    pub ready_to_delete: bool,
}

impl MandarinController {
    /// Construct a new Mandarin controller (`enARMInitData` `SLUS 0x002396b0` / `sh2pc 0x004acc06`).
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        let idx = difficulty_index(difficulty);
        let hp = ARM_HP_PC[idx];
        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            state: ArmState::Wander,
            sub_state: 0,
            stats: CreatureStats::new(hp, hp, 0.0),
            active_hand: None,
            attack_streak: 0,
            turn_dir: 1.0,
            cooldown_ticks: 0.0,
            step_timer_ticks: 0.0,
            active_attack_id: None,
            ready_to_delete: false,
        }
    }

    /// Compute authentic animation/movement speed rate (`enARMGetMoveSpeed` `SLUS 0x0023ab30`).
    pub fn move_speed_rate(&self, difficulty: DifficultyLevel, player_dist_mm: f32) -> f32 {
        let idx = difficulty_index(difficulty);
        let far_boost = if player_dist_mm > 5000.0 { 4.0 } else { 1.0 };
        ARM_MOVE_SPEED_MULT[idx] * (self.stats.health / 250.0 + 0.6) * far_boost
    }

    /// Evaluate `enARMCanSeePlayer` (`SLUS 0x0023a400` / `sh2pc 0x004ace60`).
    ///
    /// Returns `2` (attack), `1` (spotted within 2500 mm & 120°), or `0` (not seen).
    pub fn can_see_player(&self, target_pos: Vec3, over_grating: bool) -> u8 {
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let angle_diff = normalize_angle(dx.atan2(dz) - self.yaw).abs();
        if dist <= ARM_SIGHT_DIST_MM && angle_diff <= ARM_SIGHT_HALF_ANGLE_RAD {
            if dist < ARM_ATTACK_DIST_MM && over_grating {
                2
            } else {
                1
            }
        } else {
            0
        }
    }

    /// Advance Mandarin AI by `dt` seconds (`enARMCtrlAutomatic` `SLUS 0x00239820`).
    pub fn update(&mut self, target: &PerceivableTarget, difficulty: DifficultyLevel, dt: f32) {
        let ticks = dt * 60.0;
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let player_dir = dx.atan2(dz);

        self.active_attack_id = None;

        match self.state {
            ArmState::Wander => {
                if self.cooldown_ticks > 0.0 {
                    self.cooldown_ticks = (self.cooldown_ticks - ticks).max(0.0);
                }
                self.step_timer_ticks -= ticks;

                if self.sub_state == 0 || self.step_timer_ticks <= 0.0 {
                    let see = self.can_see_player(target.position, true);
                    let mut next_sub = match see {
                        2 => {
                            self.attack_streak = self.attack_streak.saturating_add(1);
                            3
                        }
                        1 => {
                            self.attack_streak = 0;
                            if self.cooldown_ticks <= 0.0 {
                                2
                            } else {
                                1
                            }
                        }
                        _ => {
                            self.attack_streak = 0;
                            1
                        }
                    };

                    if (self.sub_state > 1 && self.sub_state != next_sub && next_sub < 3)
                        || self.attack_streak > 4
                    {
                        self.cooldown_ticks = ARM_COOLDOWN_TICKS;
                        self.attack_streak = 0;
                        next_sub = 1;
                    }

                    self.sub_state = next_sub;
                    self.active_hand = Some(match self.active_hand {
                        Some(0) => 1,
                        _ => 0,
                    });
                    self.step_timer_ticks = 30.0;
                }

                if self.sub_state == 3 {
                    self.active_attack_id = Some(if self.active_hand == Some(0) {
                        ARM_ATTACK_ID_LEFT_PC
                    } else {
                        ARM_ATTACK_ID_RIGHT_PC
                    });
                }
                if self.sub_state >= 2 {
                    self.turn_towards(player_dir, ARM_ROT_STEP_RAD * ticks);
                }

                let speed = self.move_speed_rate(difficulty, dist).min(4.0) * 300.0;
                self.position.x += self.yaw.sin() * speed * dt;
                self.position.z += self.yaw.cos() * speed * dt;
            }

            ArmState::Turn => {
                self.yaw = normalize_angle(self.yaw + self.turn_dir * ARM_ROT_STEP_RAD * ticks);
                self.step_timer_ticks -= ticks;
                if self.step_timer_ticks <= 0.0 {
                    self.sub_state = self.sub_state.saturating_add(1);
                    self.step_timer_ticks = 30.0;
                    if self.sub_state >= 4 {
                        self.state = ArmState::Wander;
                        self.sub_state = 0;
                    }
                }
            }

            ArmState::Damage => {
                self.step_timer_ticks -= ticks;
                if self.step_timer_ticks <= 0.0 {
                    if self.stats.health <= 0.0 {
                        self.state = ArmState::Dead;
                        self.sub_state = 0;
                        self.step_timer_ticks = 60.0;
                    } else {
                        self.state = ArmState::Wander;
                        self.sub_state = 0;
                        self.cooldown_ticks = 0.0;
                        self.attack_streak = 0;
                    }
                }
            }

            ArmState::Dead => {
                self.step_timer_ticks -= ticks;
                if self.step_timer_ticks <= 0.0 {
                    self.ready_to_delete = true;
                }
            }
        }
    }

    /// Apply incoming damage (`enARMCtrlDamage` `SLUS 0x0023a000`).
    pub fn receive_hit(&mut self, ap: f32) {
        if self.state == ArmState::Dead {
            return;
        }
        self.stats.health = (self.stats.health - ap).max(0.0);
        self.state = ArmState::Damage;
        self.sub_state = 1;
        self.step_timer_ticks = 25.0;
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }
}

fn difficulty_index(d: DifficultyLevel) -> usize {
    match d {
        DifficultyLevel::Beginner => 0,
        DifficultyLevel::Easy => 1,
        DifficultyLevel::Normal => 2,
        DifficultyLevel::Hard => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mandarin_attack_streak_cooldown_and_death() {
        let mut arm = MandarinController::new(1, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        assert_eq!(arm.stats.max_health, 1200.0);

        let close = PerceivableTarget::new(Vec3::new(0.0, 0.0, 600.0), 0.0, true, false);
        // 5 step cycles: first 4 attack, 5th triggers 180-tick cooldown (`attack_streak > 4`).
        for _ in 0..5 {
            arm.step_timer_ticks = 0.0;
            arm.update(&close, DifficultyLevel::Normal, 1.0 / 60.0);
        }
        assert!(arm.cooldown_ticks > 0.0);
        assert_eq!(arm.sub_state, 1);

        // Lethal damage -> Damage -> Dead -> ready_to_delete after 60 ticks.
        arm.receive_hit(1500.0);
        assert_eq!(arm.state, ArmState::Damage);
        for _ in 0..95 {
            arm.update(&close, DifficultyLevel::Normal, 1.0 / 60.0);
        }
        assert_eq!(arm.state, ArmState::Dead);
        assert!(arm.ready_to_delete);
    }
}
