//! Mandarin (`chr/arm`, kind `0x20a`, internal type `0x0a`) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_arm.c`):
//!   - `0x002396b0` `enARMInitData` — hanging under-grating creature, size `(300, 500, 600, 600)`.
//!   - `0x00239820` `enARMCtrlAutomatic` — 4-state dispatch (`0x0034ea50`): Wander, Turn, Damage, Dead.
//!   - `0x00239a00` `enARMCtrlWander` — hand-over-hand traversal, attack streak cap (`>4` -> 180-tick cooldown).
//!   - `0x00239d50` `enARMCtrlTurn` — 3-step turn avoidance (`0.05235988` rad/f) triggered by `enARMCheckNearOther` (`0x0023a570`).
//!   - `0x0023a400` `enARMCanSeePlayer` — 2500 mm / 120° sight, 1000 mm attack range, grating floor checks.
//!   - `0x0023ab30` `enARMGetMoveSpeed` — Q12 anim speed `mult[mode] * (hp/250.0 + 0.6) * (dist>5000 ? 4.0 : 1.0) * 4096.0`.
//! - Retail PC (`sh2pc.exe`) drift:
//!   - HP/Endurance (`0x004acc06`): Beginner `500.0 -> 300.0` (`[300, 800, 1200, 2500, 4000]`).
//!   - Attack IDs (`+6` shift): Left/Right tongue-arm strike `0x40`/`0x41` -> `0x46`/`0x47` (`0x004ad26b`, `0x004ad38e`).
//!   - Forward floor check distance (`0x004ace60`): `1000.0 -> 500.0` mm.

use crate::ai::creatures::{xz_dist, CreatureStats};
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;

/// Retail PC Mandarin HP & endurance table (`sh2pc 0x004acc06`).
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
/// Other-Mandarin proximity threshold (`1000.0` mm, `enARMCheckNearOther` `SLUS 0x0023a570`).
pub const ARM_NEAR_OTHER_DIST_MM: f32 = 1000.0;
/// Cooldown ticks after >4 consecutive attacks or losing target (`0xb4` = `180` ticks, `enARMCtrlWander` `0x00239a00`).
pub const ARM_COOLDOWN_TICKS: f32 = 180.0;
/// Turn rate (`3°` = `0.05235988` rad/tick, `enARMCtrlWander` / `enARMCtrlTurn`).
pub const ARM_ROT_STEP_RAD: f32 = 0.052_359_88;
/// Dead deletion delay (`60` ticks, `enARMCtrlDead` `SLUS 0x0023a210`).
pub const ARM_DEAD_DELETE_TICKS: f32 = 60.0;

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
    /// Q12 animation playback speed (`*(i16*)(work + 0xc)`, `enARMGetMoveSpeed` `0x0023ab30`).
    pub anim_speed_q12: i16,
    /// Set by animation driver when current step/damage clip finishes (`*(i32*)(work + 0x14) == -1`).
    pub anim_finished: bool,
    /// Root-motion forward displacement in mm for the current tick.
    pub root_motion_mm: f32,
    /// Active swinging hand (`0` = left/bone `0xb`, `1` = right/bone `0xc`, `None` = idle).
    pub active_hand: Option<u8>,
    /// Consecutive attack count (`work+0xc9`); >4 triggers 180-tick cooldown.
    pub attack_streak: u8,
    /// Turn direction `+1.0` or `-1.0` (`work+0xcd`).
    pub turn_dir: f32,
    pub cooldown_ticks: f32,
    pub dead_timer_ticks: f32,
    pub active_attack_id: Option<u16>,
    /// True once `enARMCtrlDead` 60-tick timer expires (`enDeleteCharacter` `0x0023a210`).
    pub ready_to_delete: bool,
}

impl MandarinController {
    /// Construct a new Mandarin controller (`enARMInitData` `SLUS 0x002396b0` / `sh2pc 0x004acc06`).
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        let hp = ARM_HP_PC[difficulty.as_index()];
        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            state: ArmState::Wander,
            sub_state: 0,
            stats: CreatureStats::new(hp, hp, 0.0),
            anim_speed_q12: 4096,
            anim_finished: false,
            root_motion_mm: 0.0,
            active_hand: None,
            attack_streak: 0,
            turn_dir: 1.0,
            cooldown_ticks: 0.0,
            dead_timer_ticks: 0.0,
            active_attack_id: None,
            ready_to_delete: false,
        }
    }

    /// Compute authentic animation speed rate (`enARMGetMoveSpeed` `SLUS 0x0023ab30`).
    pub fn move_speed_rate(&self, difficulty: DifficultyLevel, player_dist_mm: f32) -> f32 {
        let far_boost = if player_dist_mm > 5000.0 { 4.0 } else { 1.0 };
        ARM_MOVE_SPEED_MULT[difficulty.as_index()] * (self.stats.health / 250.0 + 0.6) * far_boost
    }

    /// World position ahead used for the retail PC grating floor probe (`sh2pc 0x004ace60`: `500.0` mm).
    pub fn forward_grating_probe_pos(&self) -> Vec3 {
        Vec3::new(
            self.position.x + self.yaw.sin() * ARM_FORWARD_FLOOR_CHECK_PC_MM,
            self.position.y,
            self.position.z + self.yaw.cos() * ARM_FORWARD_FLOOR_CHECK_PC_MM,
        )
    }

    /// Evaluate `enARMCheckNearOther` (`SLUS 0x0023a570`): returns `Some(+1.0 | -1.0)` turn direction
    /// if another Mandarin is within `1000.0` mm 3D and within `±90°` ahead.
    pub fn check_near_other(&self, other_pos: Vec3) -> Option<f32> {
        if self.position.distance(other_pos) >= ARM_NEAR_OTHER_DIST_MM {
            return None;
        }
        let dir = (other_pos.x - self.position.x).atan2(other_pos.z - self.position.z);
        let rel = normalize_angle(dir - self.yaw);
        if rel.abs() < FRAC_PI_2 {
            Some(if rel >= 0.0 { 1.0 } else { -1.0 })
        } else {
            None
        }
    }

    /// Evaluate `enARMCanSeePlayer` (`SLUS 0x0023a400` / `sh2pc 0x004ace60`).
    pub fn can_see_player(&self, target_pos: Vec3, over_grating: bool) -> u8 {
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dist = xz_dist(self.position, target_pos);
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
    ///
    /// - `over_grating`: result of floor material checks at hands and [`Self::forward_grating_probe_pos`].
    /// - `near_other_turn_dir`: result of [`Self::check_near_other`] against sibling Mandarins (`0x0023a570`).
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        over_grating: bool,
        near_other_turn_dir: Option<f32>,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        let ticks = dt * 60.0;
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = xz_dist(self.position, target.position);
        let player_dir = dx.atan2(dz);

        self.active_attack_id = None;

        match self.state {
            ArmState::Wander => {
                if self.cooldown_ticks > 0.0 {
                    self.cooldown_ticks = (self.cooldown_ticks - ticks).max(0.0);
                }

                if self.sub_state == 0 || self.anim_finished {
                    self.anim_finished = false;
                    if let Some(turn_dir) = near_other_turn_dir {
                        // `enARMCtrlWander` `0x00239a00`: transition to Turn when another Mandarin is ahead.
                        self.turn_dir = turn_dir;
                        self.state = ArmState::Turn;
                        self.sub_state = 0;
                        self.cooldown_ticks = 0.0;
                        self.attack_streak = 0;
                        return;
                    }

                    let see = self.can_see_player(target.position, over_grating);
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

                self.anim_speed_q12 = (self.move_speed_rate(difficulty, dist) * 4096.0) as i16;
                self.apply_root_motion();
            }

            ArmState::Turn => {
                let mut speed_q12 = (self.move_speed_rate(difficulty, dist) * 4096.0) as i16;
                if dist > 5000.0 {
                    // `enARMCtrlTurn` `0x00239d50`: divides anim speed by 4 when player > 5000 mm.
                    speed_q12 >>= 2;
                }
                self.anim_speed_q12 = speed_q12;
                self.yaw = normalize_angle(self.yaw + self.turn_dir * ARM_ROT_STEP_RAD * ticks);
                self.apply_root_motion();

                if self.sub_state == 0 {
                    self.sub_state = 1;
                } else if self.anim_finished {
                    self.anim_finished = false;
                    self.sub_state = self.sub_state.saturating_add(1);
                    if self.sub_state >= 4 {
                        self.state = ArmState::Wander;
                        self.sub_state = 0;
                    }
                }
            }

            ArmState::Damage => {
                if self.anim_finished {
                    self.anim_finished = false;
                    if self.stats.health <= 0.0 {
                        self.state = ArmState::Dead;
                        self.sub_state = 0;
                        self.dead_timer_ticks = ARM_DEAD_DELETE_TICKS;
                    } else {
                        self.state = ArmState::Wander;
                        self.sub_state = 0;
                        self.cooldown_ticks = 0.0;
                        self.attack_streak = 0;
                    }
                }
            }

            ArmState::Dead => {
                self.dead_timer_ticks -= ticks;
                if self.dead_timer_ticks <= 0.0 {
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
        self.anim_finished = false;
    }

    fn apply_root_motion(&mut self) {
        if self.root_motion_mm != 0.0 {
            self.position.x += self.yaw.sin() * self.root_motion_mm;
            self.position.z += self.yaw.cos() * self.root_motion_mm;
            self.root_motion_mm = 0.0;
        }
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mandarin_attack_streak_turn_avoidance_and_death() {
        let mut arm = MandarinController::new(1, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        assert_eq!(arm.stats.max_health, 1200.0);

        let close = PerceivableTarget::new(Vec3::new(0.0, 0.0, 600.0), 0.0, true, false);
        // 5 step cycles: first 4 attack, 5th triggers 180-tick cooldown (`attack_streak > 4`).
        for _ in 0..5 {
            arm.anim_finished = true;
            arm.update(&close, true, None, DifficultyLevel::Normal, 1.0 / 60.0);
        }
        assert!(arm.cooldown_ticks > 0.0);
        assert_eq!(arm.sub_state, 1);

        // Sibling Mandarin within 1000 mm ahead triggers ArmState::Turn (`enARMCheckNearOther`).
        let turn_dir = arm.check_near_other(Vec3::new(100.0, 0.0, 500.0));
        assert!(turn_dir.is_some());
        arm.anim_finished = true;
        arm.update(&close, true, turn_dir, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(arm.state, ArmState::Turn);

        // Lethal damage -> Damage -> Dead -> ready_to_delete after 60 ticks.
        arm.receive_hit(1500.0);
        assert_eq!(arm.state, ArmState::Damage);
        arm.anim_finished = true;
        arm.update(&close, true, None, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(arm.state, ArmState::Dead);
        for _ in 0..65 {
            arm.update(&close, true, None, DifficultyLevel::Normal, 1.0 / 60.0);
        }
        assert!(arm.ready_to_delete);
    }
}
