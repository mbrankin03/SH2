//! Flesh Lips (`chr/ike`, kind `0x203`, internal type `0x07`) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_ike.c`):
//!   - `0x00150b50` `enIKEInitData` — ceiling wait (`y < -2140.0`), active hover (`y = -1640.0`), size `(150, 650, 500, 600)`.
//!   - `0x00150cd0` `enIKECtrlAutomatic` — 11-state dispatch table (`0x002aa3b0`).
//!   - `0x00150d90` `enIKECtrlWait`, `0x00150fe0` `enIKECtrlAppearance`, `0x00151160` `enIKECtrlMove`,
//!     `0x001516a0` `enIKECtrlChase`, `0x00151b90` `enIKECtrlKick`, `0x00151d10` `enIKECtrlSwing`,
//!     `0x00151f70` `enIKECtrlAttack`, `0x00152300` `enIKECtrlAttack2`, `0x00152480` `enIKECtrlTakePlayer`,
//!     `0x00152670` `enIKECtrlDamage`, `0x001528d0` `enIKECtrlDead`.
//! - Retail PC (`sh2pc.exe`) drift:
//!   - HP/Endurance (`0x004b8b95`): `[500, 2000, 3000, 5000, 8000]` (vs SLUS `[1000, 1500, 2000, 4000, 5000]`).
//!   - Attack IDs (`+6` shift): Grab `0x36`->`0x3c`, Hug `0x37`->`0x3d`, Lunge `0x38`->`0x3e`.
//!   - `enIKECanSeePlayer2` (`0x004b8970`): removed `500.0` mm lower bound for `Attack2` (`dist < 1000.0`).
//!   - `enIKECtrlDead` (`0x004b9df0`): adds 10-tick delay before setting battle-clear event flag `0x40000`.

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, PI};

/// Retail PC Flesh Lips HP & endurance table (`sh2pc 0x004b8b95`; SLUS `0x002aa370` was `[1000, 1500, 2000, 4000, 5000]`).
pub const IKE_HP_PC: [f32; 5] = [500.0, 2000.0, 3000.0, 5000.0, 8000.0];
/// Flesh Lips move speed multipliers (`SLUS 0x002aa3e0` / `sh2pc 0x004b8a90`).
pub const IKE_MOVE_SPEED_MULT: [f32; 5] = [0.5, 0.8, 1.0, 1.2, 1.5];

/// Ceiling wait threshold Y coordinate (`enIKEInitData` `SLUS 0x00150b50`).
pub const IKE_WAIT_Y_THRESHOLD_MM: f32 = -2140.0;
/// Active ceiling-hang Y coordinate (`enIKECtrlAppearance` `SLUS 0x00150fe0`).
pub const IKE_ACTIVE_Y_MM: f32 = -1640.0;
/// Descent velocity during Appearance (`+250.0` mm/s in Y-down space, `0x00150fe0`).
pub const IKE_DESCENT_VEL_Y: f32 = 250.0;
/// Wait spawn timer (`0x2a30` = `10800` ticks, `enIKECtrlWait` `0x00150d90`).
pub const IKE_WAIT_TIMER_TICKS: f32 = 10_800.0;

/// Retail PC attack IDs (`+6` over SLUS `0x36..=0x38`).
pub const IKE_ATTACK_ID_GRAB_PC: u16 = 0x3c;
pub const IKE_ATTACK_ID_HUG_PC: u16 = 0x3d;
pub const IKE_ATTACK_ID_LUNGE_PC: u16 = 0x3e;

/// 7 ceiling spawn points `(x, y, z, rot_y)` (`appearance_point[7]` `SLUS 0x002aa300`).
pub const IKE_APPEARANCE_POINTS: [(Vec3, f32); 7] = [
    (Vec3::new(-141_100.0, -3000.0, -20_400.0), FRAC_PI_2),
    (Vec3::new(-141_100.0, -3000.0, -19_800.0), FRAC_PI_2),
    (Vec3::new(-140_200.0, -3000.0, -18_500.0), PI),
    (Vec3::new(-139_600.0, -3000.0, -18_500.0), PI),
    (Vec3::new(-138_900.0, -3000.0, -19_600.0), -FRAC_PI_2),
    (Vec3::new(-138_900.0, -3000.0, -20_400.0), -FRAC_PI_2),
    (Vec3::new(-140_000.0, -3000.0, -21_500.0), 0.0),
];

/// Flesh Lips state machine enum matching `0x002aa3b0` (`enIKECtrlAutomatic` `SLUS 0x00150cd0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum IkeState {
    Wait = 0,
    #[default]
    Appearance = 1,
    Move = 2,
    Chase = 3,
    Kick = 4,
    Swing = 5,
    Attack = 6,
    Attack2 = 7,
    TakePlayer = 8,
    Damage = 9,
    Dead = 10,
}

/// Flesh Lips AI controller (`en_ike.c`, kind `0x203`).
#[derive(Debug, Clone)]
pub struct FleshLipsController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    /// Pitch oscillation angle in radians (`enIKECtrlSwing` `0x00151d10`).
    pub pitch: f32,
    pub state: IkeState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub timer_ticks: f32,
    /// Cardinal travel quadrant `0..=3` (`work+0xbc`).
    pub dir_quad: u8,
    /// Swing oscillation phase in radians (`enIKECtrlSwing`).
    pub swing_phase: f32,
    /// Repeated shotgun hit counter (`work+0xbf` in `enIKECtrlDamage` `0x00152670`).
    pub shotgun_hit_cnt: u8,
    pub is_hugging_player: bool,
    pub active_attack_id: Option<u16>,
    /// Set when the last Flesh Lips completes its death exit (`_DAT_01f7a9f8 |= 0x40000`).
    pub battle_cleared_event: bool,
}

impl FleshLipsController {
    /// Construct a new Flesh Lips controller (`enIKEInitData` `SLUS 0x00150b50` / `sh2pc 0x004b8b95`).
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        let idx = difficulty_index(difficulty);
        let hp = IKE_HP_PC[idx];
        let state = if position.y < IKE_WAIT_Y_THRESHOLD_MM {
            IkeState::Wait
        } else {
            IkeState::Appearance
        };
        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            pitch: 0.0,
            state,
            sub_state: 0,
            stats: CreatureStats::new(hp, hp, 0.0),
            timer_ticks: 0.0,
            dir_quad: 0,
            swing_phase: 0.0,
            shotgun_hit_cnt: 0,
            is_hugging_player: false,
            active_attack_id: None,
            battle_cleared_event: false,
        }
    }

    /// Compute authentic hover movement speed in mm/s (`enIKEGetMoveSpeed` `SLUS 0x00153280`).
    pub fn move_speed_mm_s(&self, difficulty: DifficultyLevel) -> f32 {
        let idx = difficulty_index(difficulty);
        IKE_MOVE_SPEED_MULT[idx] * (self.stats.health / 500.0 + 0.8) * 500.0
    }

    /// Evaluate `enIKECanSeePlayer` (`SLUS 0x00152f50`): `2` grab (`<50`), `1` chase (`<=1000`), `0` none.
    pub fn can_see_player_move(&self, target_pos: Vec3) -> u8 {
        let dist = xz_dist(self.position, target_pos);
        if dist < 50.0 {
            2
        } else if dist <= 1000.0 {
            1
        } else {
            0
        }
    }

    /// Evaluate `enIKECanSeePlayer2` (`SLUS 0x00153000` / `sh2pc 0x004b8970`):
    /// `2` grab (`<50`), `3` lunge (`<1000`, Normal+, `10°` cone), `1` chase (`<=2500`), `0` lost.
    pub fn can_see_player_chase(&self, target_pos: Vec3, difficulty: DifficultyLevel) -> u8 {
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < 50.0 {
            return 2;
        }
        if dist > 2500.0 {
            return 0;
        }
        let angle_diff = normalize_angle(dx.atan2(dz) - self.yaw).abs();
        let idx = difficulty_index(difficulty);
        // Retail PC `0x004b8970`: removed SLUS `dist > 500.0` lower bound for Attack2.
        if dist < 1000.0 && idx > 1 && angle_diff < 0.174_532_92 {
            3
        } else {
            1
        }
    }

    /// Advance Flesh Lips AI by `dt` seconds (`enIKECtrlAutomatic` `SLUS 0x00150cd0`).
    pub fn update(&mut self, target: &PerceivableTarget, difficulty: DifficultyLevel, dt: f32) {
        let ticks = dt * 60.0;
        let speed = self.move_speed_mm_s(difficulty);
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let player_dir = dx.atan2(dz);

        self.active_attack_id = None;

        match self.state {
            IkeState::Wait => {
                if self.sub_state == 0 {
                    self.timer_ticks = IKE_WAIT_TIMER_TICKS;
                    self.sub_state = 1;
                }
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    // Pick farthest appearance point from player (`enIKECtrlWait` `0x00150d90`).
                    if let Some((pt, ang)) =
                        IKE_APPEARANCE_POINTS.iter().max_by(|(a, _), (b, _)| {
                            xz_dist(*a, target.position).total_cmp(&xz_dist(*b, target.position))
                        })
                    {
                        self.position = *pt;
                        self.yaw = *ang;
                    }
                    self.transition(IkeState::Appearance);
                }
            }

            IkeState::Appearance => match self.sub_state {
                0 => {
                    if self.position.y >= IKE_ACTIVE_Y_MM {
                        self.transition(IkeState::Move);
                    } else {
                        self.sub_state = 1;
                    }
                }
                1 => {
                    self.position.y += IKE_DESCENT_VEL_Y * dt;
                    if self.position.y >= IKE_ACTIVE_Y_MM {
                        self.position.y = IKE_ACTIVE_Y_MM;
                        self.timer_ticks = 60.0;
                        self.sub_state = 2;
                    }
                }
                _ => {
                    self.position.x += self.yaw.sin() * speed * dt;
                    self.position.z += self.yaw.cos() * speed * dt;
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.transition(IkeState::Move);
                    }
                }
            },

            IkeState::Move => {
                let path_ang = normalize_angle(self.yaw + f32::from(self.dir_quad) * FRAC_PI_2);
                self.position.x += path_ang.sin() * speed * dt;
                self.position.z += path_ang.cos() * speed * dt;

                match self.can_see_player_move(target.position) {
                    2 => self.transition(IkeState::Attack),
                    1 => self.transition(IkeState::Chase),
                    _ => {}
                }
            }

            IkeState::Chase => {
                // Rotates at 0.017453292 rad/f (`1°`, `enIKECtrlChase` `0x001516a0`).
                self.turn_towards(player_dir, 0.017_453_292 * ticks);
                self.position.x += self.yaw.sin() * speed * dt;
                self.position.z += self.yaw.cos() * speed * dt;

                match self.can_see_player_chase(target.position, difficulty) {
                    3 => self.transition(IkeState::Attack2),
                    2 => self.transition(IkeState::Attack),
                    0 => self.transition(IkeState::Move),
                    _ => {}
                }
            }

            IkeState::Kick => {
                // Hover-position at 750.0 mm from player (`enIKECtrlKick` `0x00151b90`).
                self.turn_towards(player_dir, 0.017_453_292 * ticks);
                let vel = 750.0 - dist;
                self.position.x += self.yaw.sin() * vel * dt;
                self.position.z += self.yaw.cos() * vel * dt;

                match self.can_see_player_chase(target.position, difficulty) {
                    3 => self.transition(IkeState::Attack2),
                    2 => self.transition(IkeState::Attack),
                    _ if dist > 1000.0 => self.transition(IkeState::Move),
                    _ => {}
                }
            }

            IkeState::Swing => {
                if self.sub_state == 0 {
                    self.swing_phase = 0.0;
                    self.sub_state = 1;
                }
                self.swing_phase += dt * PI;
                match self.sub_state {
                    1 => {
                        self.pitch = (self.swing_phase * 2.0).sin() * 0.174_532_92;
                        if self.swing_phase > FRAC_PI_2 {
                            self.swing_phase -= FRAC_PI_2;
                            self.sub_state = 2;
                        }
                    }
                    2 => {
                        self.pitch = -self.swing_phase.sin() * 0.698_131_7;
                        if self.swing_phase > PI {
                            self.swing_phase -= PI;
                            self.sub_state = 3;
                        }
                    }
                    3 => {
                        self.pitch = self.swing_phase.sin() * 0.698_131_7;
                        if self.swing_phase > PI {
                            self.swing_phase -= PI;
                            self.sub_state = 4;
                        }
                    }
                    _ => {
                        self.pitch = -(self.swing_phase * 2.0).sin() * 0.174_532_92;
                        if self.swing_phase > FRAC_PI_2 {
                            self.pitch = 0.0;
                            self.transition(IkeState::Move);
                        }
                    }
                }
            }

            IkeState::Attack => match self.sub_state {
                0 => {
                    self.timer_ticks = 40.0;
                    self.sub_state = 1;
                }
                1 => {
                    self.active_attack_id = Some(IKE_ATTACK_ID_GRAB_PC);
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.shotgun_hit_cnt = 0;
                        if self.is_hugging_player {
                            self.sub_state = 3;
                        } else {
                            self.timer_ticks = 30.0;
                            self.sub_state = 4;
                        }
                    }
                }
                3 => {
                    self.active_attack_id = Some(IKE_ATTACK_ID_HUG_PC);
                    if !self.is_hugging_player {
                        self.timer_ticks = 30.0;
                        self.sub_state = 4;
                    }
                }
                _ => {
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.transition(IkeState::Move);
                    }
                }
            },

            IkeState::Attack2 => {
                if self.sub_state == 0 {
                    self.timer_ticks = 45.0;
                    self.sub_state = 1;
                }
                self.active_attack_id = Some(IKE_ATTACK_ID_LUNGE_PC);
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    self.shotgun_hit_cnt = 0;
                    self.transition(IkeState::Move);
                }
            }

            IkeState::TakePlayer => {
                self.active_attack_id = Some(IKE_ATTACK_ID_HUG_PC);
                match self.sub_state {
                    0 | 1 => {
                        self.sub_state = 1;
                        self.position.z -= 500.0 * dt;
                        if self.position.z < -21_450.0 {
                            self.position.z = -21_500.0;
                            self.sub_state = 2;
                        }
                    }
                    _ => {
                        self.position.y -= 500.0 * dt;
                    }
                }
            }

            IkeState::Damage => {
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    self.position.y = IKE_ACTIVE_Y_MM;
                    if self.stats.health <= 0.0 {
                        self.transition(IkeState::Dead);
                    } else {
                        self.transition(IkeState::Chase);
                    }
                }
            }

            IkeState::Dead => match self.sub_state {
                0 => {
                    self.sub_state = 1;
                }
                1 => {
                    let outside = self.position.x < -140_500.0
                        || self.position.x > -139_300.0
                        || self.position.z < -20_500.0
                        || self.position.z > -19_300.0;
                    if outside {
                        // Retail PC `0x004b9df0`: 10-tick delay before battle clear event.
                        self.timer_ticks = 10.0;
                        self.sub_state = 6;
                    } else {
                        self.position.x += self.yaw.sin() * speed * dt;
                        self.position.z += self.yaw.cos() * speed * dt;
                    }
                }
                6 => {
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.battle_cleared_event = true;
                        self.sub_state = 7;
                    }
                }
                _ => {}
            },
        }
    }

    /// Apply incoming hit (`enIKECtrlDamage` `SLUS 0x00152670`).
    ///
    /// Repeated shotgun hits (`damage_kind == 0x11`) divide damage by `shotgun_hit_cnt` and
    /// trigger `Kick` evasion after >4 consecutive hits.
    pub fn receive_hit(&mut self, ap: f32, damage_kind: u8, is_spray: bool) {
        if self.state == IkeState::Dead || self.is_hugging_player {
            return;
        }
        if is_spray {
            self.stats.health = 0.0;
            self.stats.endurance = 0.0;
            self.transition(IkeState::Dead);
            return;
        }

        let effective_ap = if damage_kind == 0x11 {
            self.shotgun_hit_cnt = self.shotgun_hit_cnt.saturating_add(1);
            let div = f32::from(self.shotgun_hit_cnt);
            if self.shotgun_hit_cnt > 4 {
                if self.shotgun_hit_cnt > 9 {
                    self.shotgun_hit_cnt = 0;
                }
                self.stats.health = (self.stats.health - ap / div).max(0.0);
                if self.stats.health > 0.0 {
                    self.transition(IkeState::Kick);
                } else {
                    self.transition(IkeState::Dead);
                }
                return;
            }
            ap / div
        } else {
            self.shotgun_hit_cnt = 0;
            ap
        };

        self.stats.health = (self.stats.health - effective_ap).max(0.0);
        self.state = IkeState::Damage;
        self.sub_state = 1;
        self.timer_ticks = 20.0;
    }

    fn transition(&mut self, next: IkeState) {
        self.state = next;
        self.sub_state = 0;
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }
}

fn xz_dist(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
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
    fn test_flesh_lips_ceiling_descent_and_shotgun_diminishing_returns() {
        let mut ike = FleshLipsController::new(
            1,
            Vec3::new(-140_000.0, -2000.0, -20_000.0),
            0.0,
            DifficultyLevel::Normal,
        );
        assert_eq!(ike.state, IkeState::Appearance);
        assert_eq!(ike.stats.max_health, 3000.0);

        let target =
            PerceivableTarget::new(Vec3::new(-140_000.0, 0.0, -18_000.0), 0.0, true, false);
        // Descend from -2000 to -1640 at 250 mm/s (~1.44s) + 1.0s forward glide
        for _ in 0..160 {
            ike.update(&target, DifficultyLevel::Normal, 1.0 / 60.0);
        }
        assert_eq!(ike.position.y, IKE_ACTIVE_Y_MM);
        assert_eq!(ike.state, IkeState::Move);

        // 5 consecutive shotgun hits (0x11) trigger Kick evasion.
        for _ in 0..5 {
            ike.receive_hit(100.0, 0x11, false);
        }
        assert_eq!(ike.state, IkeState::Kick);
    }
}
