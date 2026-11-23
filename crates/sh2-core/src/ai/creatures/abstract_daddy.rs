//! Abstract Daddy (`chr/pap`, kind `0x204`, internal type `0x04`) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_pap.c`):
//!   - `0x0015a060` `enPAPInitData` — regular vs boss HP/endurance, sizes, initial state.
//!   - `0x0015a2f0` `enPAPCtrlAutomatic` — 10-state dispatch table (`0x002aa8e0`).
//!   - `0x0015a4b0` `Wander`, `0x0015a8c0` `Straight`, `0x0015ac10` `Chase`, `0x0015ae80` `Rush`,
//!     `0x0015b160` `Attack`, `0x0015b640` `Damage`, `0x0015b9c0` `Confuse`, `0x0015baf0` `Down`,
//!     `0x0015c430` `LastMove`.
//!   - `0x0015c690` `enPAPCanSeePlayer` — 2500 mm sight, 10° cone, `player_radius + 405.0 (-100.0 in Rush)` grab range.
//!   - `0x0015cc70` `enPAPGetWalkSpeed` — computes Q12 animation playback speed (`rate * 4096.0`, Rush `5120.0`).
//!   - `0x00142200` `enReduceHP` — incoming AP reduces both HP and Endurance.
//! - Retail PC (`sh2pc.exe`) drift:
//!   - Boss HP (`0x004b6874`): `[900, 2500, 5000, 7000, 10000]` (SLUS `[1000, 4000, 5000, 7000, 10000]`).
//!   - Attack IDs (`+6` shift): Grab `0x39 -> 0x3f` (`0x004b76da`), Hug `0x3a -> 0x40` (`0x004b7821`).
//!   - Rot speed (`0x004b5de0`): `[0.5, 0.8, 1.0, 2.0, 2.0] * 0.02617994` rad/tick.
//!   - Rush trigger (`0x004b78f0`): `mode > 1` (Normal+, vs SLUS `mode > 2`).
//!   - `enPAPCtrlDown` case 3 Beginner fix (`0x004b7be0`): writes `state = 9` instead of SLUS `0x0b`.

use crate::ai::creatures::{xz_dist, CreatureStats};
use crate::ai::pathfinding::EnemyPathState;
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Regular hotel Abstract Daddy HP table (`SLUS 0x002aa840`).
pub const PAP_REGULAR_HP: [f32; 5] = [500.0, 750.0, 1000.0, 2000.0, 4000.0];
/// Regular hotel Abstract Daddy endurance table (`SLUS 0x002aa860`).
pub const PAP_REGULAR_ENDURANCE: [f32; 5] = [150.0, 285.0, 380.0, 500.0, 600.0];
/// Boss Abstract Daddy HP table — retail PC (`sh2pc 0x004b6874`; SLUS `0x002aa880` had `[1000, 4000, ...]`).
pub const PAP_BOSS_HP_PC: [f32; 5] = [900.0, 2500.0, 5000.0, 7000.0, 10_000.0];
/// Boss Abstract Daddy endurance table (`SLUS 0x002aa8a0`).
pub const PAP_BOSS_ENDURANCE: [f32; 5] = [300.0, 400.0, 600.0, 800.0, 1000.0];
/// Endurance recovery per second (`SLUS 0x002aa908` `enPAPAutoRecovery` `0x0015cc00`).
pub const PAP_AUTO_RECOVERY: [f32; 5] = [0.0, 10.0, 30.0, 60.0, 100.0];
/// Walk speed multipliers (`SLUS 0x002aa920` `enPAPGetWalkSpeed` `0x0015cc70`).
pub const PAP_WALK_SPEED_MULT: [f32; 5] = [0.7, 0.8, 1.0, 1.2, 1.5];
/// Retail PC rotation speed multipliers (`sh2pc 0x004b5de0`, × `0.02617994` rad/tick).
pub const PAP_ROT_SPEED_MULT_PC: [f32; 5] = [0.5, 0.8, 1.0, 2.0, 2.0];
/// Base retail PC rotation step (`1.5°` = `0.02617994` rad/tick, `sh2pc 0x004b5de0`).
pub const PAP_ROT_STEP_PC_RAD: f32 = 0.026_179_94;
/// Regular down duration in ticks (`SLUS 0x002aa978` `enPAPSetDownTime` `0x0015ce10`).
pub const PAP_REGULAR_DOWN_TICKS: [i32; 5] = [900, 480, 360, 300, 240];
/// Boss down duration in ticks (`SLUS 0x002aa988` `enPAPSetDownTime` `0x0015ce10`).
pub const PAP_BOSS_DOWN_TICKS: [i32; 5] = [180, 180, 120, 60, 60];

/// Sight distance in mm (`enPAPCanSeePlayer` `SLUS 0x0015c690`).
pub const PAP_SIGHT_DIST_MM: f32 = 2500.0;
/// Sight & attack cone half-angle (`10°` = `0.17453292` rad, `enPAPCanSeePlayer` `0x0015c690`).
pub const PAP_SIGHT_HALF_ANGLE_RAD: f32 = 0.174_532_92;
/// Base grab reach offset added to player radius (`405.0` mm, `enPAPCanSeePlayer` `0x0015c690`).
pub const PAP_ATTACK_RANGE_OFFSET_MM: f32 = 405.0;
/// Reach reduction during Rush state (`100.0` mm, `enPAPCanSeePlayer` `0x0015c690`).
pub const PAP_RUSH_RANGE_REDUCTION_MM: f32 = 100.0;
/// Rush duration in ticks (`300` ticks, `enPAPCtrlRush` `0x0015ae80`).
pub const PAP_RUSH_DURATION_TICKS: f32 = 300.0;
/// Retail PC grab attempt attack ID (`0x3f`; SLUS `0x39`, `0x0015b160` / `0x004b76da`).
pub const PAP_ATTACK_ID_GRAB_PC: u16 = 0x3f;
/// Retail PC hug attack ID (`0x40`; SLUS `0x3a`, `0x0015b160` / `0x004b7821`).
pub const PAP_ATTACK_ID_HUG_PC: u16 = 0x40;

/// Abstract Daddy state machine enum matching `0x002aa8e0` (`enPAPCtrlAutomatic` `SLUS 0x0015a2f0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum PapState {
    #[default]
    Wander = 0,
    Straight = 1,
    Chase = 2,
    Rush = 3,
    Attack = 4,
    Damage = 5,
    Confuse = 6,
    Down = 7,
    LastMove = 8,
    Dying = 9,
}

/// Abstract Daddy AI controller (`en_pap.c`, kind `0x204`).
#[derive(Debug, Clone)]
pub struct AbstractDaddyController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    /// `true` for the Labyrinth Boss (`spawn_kind == 6`), `false` for Hotel corridor encounters.
    pub is_boss: bool,
    pub state: PapState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub path: EnemyPathState,
    /// Q12 animation playback speed (`*(i16*)(work + 0xc)`, `4096 = 1.0x`, `enPAPGetWalkSpeed` `0x0015cc70`).
    pub anim_speed_q12: i16,
    /// Set to `true` by the animation driver when current clip finishes (`*(i32*)(work + 0x14) == -1`).
    pub anim_finished: bool,
    /// Root-motion forward displacement in mm for the current tick (from `.anm` playback).
    pub root_motion_mm: f32,
    /// Active collision radius in mm (`400.0` normal, `200.0` rush, `150.0` attack, `0.0` hug, `300.0` down).
    pub radius_mm: f32,
    /// Countdown timer in 60 Hz ticks (`enSetTimer` / `enReduceTimer`: Rush `300`, Confuse `600 - 90*mode`, Down table).
    pub timer_ticks: f32,
    /// Current attack ID being checked (`0x3f` grab, `0x40` hug), if any.
    pub active_attack_id: Option<u16>,
    /// Set when `enPAPCtrlLastMove` finishes (`event_flag_01126458 |= 0x4000`).
    pub boss_defeated_event: bool,
    /// True while hugging/grabbing the player (`enCheckHuggedPlayer`).
    pub is_hugging_player: bool,
    /// Last damage kind received (used to decide `Chase` vs `Rush` on light hits `1`/`2`).
    pub last_damage_kind: u8,
    /// Current wander target point (`pos + forward * 1500.0`).
    pub wander_target: Vec3,
}

impl AbstractDaddyController {
    /// Construct a new Abstract Daddy controller (`enPAPInitData` `SLUS 0x0015a060` / `sh2pc 0x004b6874`).
    pub fn new(
        id: u32,
        position: Vec3,
        yaw: f32,
        is_boss: bool,
        difficulty: DifficultyLevel,
    ) -> Self {
        let idx = difficulty.as_index();
        let (hp, end) = if is_boss {
            (PAP_BOSS_HP_PC[idx], PAP_BOSS_ENDURANCE[idx])
        } else {
            (PAP_REGULAR_HP[idx], PAP_REGULAR_ENDURANCE[idx])
        };
        let initial_state = if is_boss {
            PapState::Chase
        } else {
            PapState::Straight
        };

        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            is_boss,
            state: initial_state,
            sub_state: 0,
            stats: CreatureStats::new(hp, end, PAP_AUTO_RECOVERY[idx]),
            path: EnemyPathState::new(yaw),
            anim_speed_q12: 4096,
            anim_finished: false,
            root_motion_mm: 0.0,
            radius_mm: 400.0,
            timer_ticks: 0.0,
            active_attack_id: None,
            boss_defeated_event: false,
            is_hugging_player: false,
            last_damage_kind: 0,
            wander_target: position + Vec3::new(yaw.sin() * 1500.0, 0.0, yaw.cos() * 1500.0),
        }
    }

    /// Compute authentic walk speed scale (`enPAPGetWalkSpeed` `SLUS 0x0015cc70`).
    pub fn walk_speed_scale(&self, difficulty: DifficultyLevel) -> f32 {
        let idx = difficulty.as_index();
        let boss_factor = if self.is_boss { 0.5 } else { 1.0 };
        PAP_WALK_SPEED_MULT[idx] * (self.stats.health / 200.0 + 0.5) * boss_factor
    }

    /// Evaluate sight and grab reach (`enPAPCanSeePlayer` `SLUS 0x0015c690`).
    ///
    /// Returns `2` (attack range), `1` (spotted within 2500 mm & 10° cone), or `0` (not seen).
    pub fn can_see_player(&self, target_pos: Vec3, player_radius_mm: f32) -> u8 {
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > PAP_SIGHT_DIST_MM {
            return 0;
        }
        let dir = dx.atan2(dz);
        let angle_diff = normalize_angle(dir - self.yaw).abs();
        if angle_diff > PAP_SIGHT_HALF_ANGLE_RAD {
            return 0;
        }
        let rush_penalty = if self.state == PapState::Rush {
            PAP_RUSH_RANGE_REDUCTION_MM
        } else {
            0.0
        };
        if dist < (player_radius_mm + PAP_ATTACK_RANGE_OFFSET_MM) - rush_penalty {
            2
        } else {
            1
        }
    }

    /// Advance Abstract Daddy AI by `dt` seconds (`enPAPCtrlAutomatic` `SLUS 0x0015a2f0`).
    ///
    /// `player_radius_mm` corresponds to `enGetPlayerSize()` (`200.0` mm for James).
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        player_radius_mm: f32,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        if self.state == PapState::Dying {
            return;
        }

        let ticks = dt * 60.0;
        let idx = difficulty.as_index();
        let rot_step = PAP_ROT_SPEED_MULT_PC[idx] * PAP_ROT_STEP_PC_RAD * ticks;
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = xz_dist(self.position, target.position);
        let player_dir = dx.atan2(dz);
        let see = self.can_see_player(target.position, player_radius_mm);

        self.active_attack_id = None;

        match self.state {
            PapState::Wander => {
                self.stats.update(dt);
                if see == 2 {
                    self.transition(PapState::Attack);
                    return;
                }
                if see == 1 || dist < 1000.0 {
                    self.transition(PapState::Chase);
                    return;
                }
                if self.sub_state == 0 {
                    self.path.init(self.yaw);
                    self.wander_target = self.position
                        + Vec3::new(self.yaw.sin() * 1500.0, 0.0, self.yaw.cos() * 1500.0);
                    self.sub_state = 1;
                }
                self.anim_speed_q12 = (self.walk_speed_scale(difficulty) * 4096.0) as i16;
                let tdx = self.wander_target.x - self.position.x;
                let tdz = self.wander_target.z - self.position.z;
                self.turn_towards(tdx.atan2(tdz), rot_step);
                self.apply_root_motion();
            }

            PapState::Straight => {
                self.stats.update(dt);
                if see == 2 {
                    self.transition(PapState::Attack);
                    return;
                }
                if self.sub_state == 0 {
                    self.path.init(self.yaw);
                    self.sub_state = 1;
                }
                self.anim_speed_q12 = (self.walk_speed_scale(difficulty) * 4096.0) as i16;
                self.apply_root_motion();
            }

            PapState::Chase => {
                self.turn_towards(player_dir, rot_step);
                self.anim_speed_q12 = (self.walk_speed_scale(difficulty) * 4096.0) as i16;
                if see == 2 {
                    self.transition(PapState::Attack);
                    return;
                }
                if see == 0 && !self.is_boss && dist > PAP_SIGHT_DIST_MM {
                    self.transition(PapState::Wander);
                    return;
                }
                self.apply_root_motion();
            }

            PapState::Rush => {
                if self.sub_state == 0 {
                    self.radius_mm = 200.0;
                    self.timer_ticks = PAP_RUSH_DURATION_TICKS;
                    self.sub_state = 1;
                }
                self.turn_towards(player_dir, rot_step);
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    self.radius_mm = 400.0;
                    self.transition(PapState::Chase);
                    return;
                }
                if see == 2 {
                    self.transition(PapState::Attack);
                    return;
                }
                // `enPAPCtrlRush` `SLUS 0x0015ae80`: `5120.0` Q12 scale (`1.25x` walk speed).
                self.anim_speed_q12 = (self.walk_speed_scale(difficulty) * 5120.0) as i16;
                self.apply_root_motion();
            }

            PapState::Attack => match self.sub_state {
                0 => {
                    self.radius_mm = 150.0;
                    self.anim_finished = false;
                    self.sub_state = 1;
                }
                1 => {
                    self.active_attack_id = Some(PAP_ATTACK_ID_GRAB_PC);
                    // Rotates toward player at 0.05235988 rad/f (`3°`, `enPAPCtrlAttack` `0x0015b160`).
                    self.turn_towards(player_dir, 0.052_359_88 * ticks);
                    if self.is_hugging_player {
                        self.radius_mm = 0.0;
                        let target_pos = Vec3::new(
                            target.position.x - player_dir.sin() * 150.0,
                            self.position.y,
                            target.position.z - player_dir.cos() * 150.0,
                        );
                        let pull = (target_pos - self.position) * 4.0;
                        self.position.x += pull.x * dt;
                        self.position.z += pull.z * dt;
                    }
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.sub_state = if self.is_hugging_player { 2 } else { 4 };
                    }
                }
                2 => {
                    self.active_attack_id = Some(PAP_ATTACK_ID_HUG_PC);
                    if !self.is_hugging_player {
                        self.anim_finished = false;
                        self.sub_state = 3;
                    }
                }
                3 => {
                    // Recoil at -500.0 mm/s (`enPAPCtrlAttack` `SLUS 0x0015b160`).
                    self.position.x -= self.yaw.sin() * 500.0 * dt;
                    self.position.z -= self.yaw.cos() * 500.0 * dt;
                    if self.anim_finished {
                        self.finish_attack();
                    }
                }
                _ => {
                    if self.anim_finished {
                        self.finish_attack();
                    }
                }
            },

            PapState::Damage => {
                if self.anim_finished {
                    self.anim_finished = false;
                    if self.sub_state == 2 {
                        self.transition(PapState::Down);
                    } else if (self.is_boss || idx > 1) && matches!(self.last_damage_kind, 1 | 2) {
                        // Retail PC `0x004b78f0`: Normal+ or Boss enters Rush after light hits 1/2.
                        self.transition(PapState::Rush);
                    } else {
                        self.transition(PapState::Chase);
                    }
                }
            }

            PapState::Confuse => {
                if self.sub_state == 0 {
                    self.timer_ticks = (600 - 90 * idx as i32) as f32;
                    self.sub_state = 2;
                }
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    self.transition(PapState::Wander);
                }
            }

            PapState::Down => {
                if self.sub_state == 0 {
                    self.radius_mm = 300.0;
                    if self.stats.health <= 0.0 {
                        if self.is_boss {
                            self.transition(PapState::LastMove);
                        } else {
                            self.transition(PapState::Dying);
                        }
                        return;
                    }
                    // Uses `enPAPSetDownTime` (`SLUS 0x0015ce10`: `PAP_BOSS_DOWN_TICKS` / `PAP_REGULAR_DOWN_TICKS`).
                    self.timer_ticks = if self.is_boss {
                        PAP_BOSS_DOWN_TICKS[idx] as f32
                    } else {
                        PAP_REGULAR_DOWN_TICKS[idx] as f32
                    };
                    self.sub_state = 1;
                }
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    if idx == 0 && !self.is_boss {
                        // Retail PC `0x004b7be0` Beginner fix (SLUS erroneously wrote 0x0b).
                        self.stats.health = 0.0;
                        self.stats.endurance = 0.0;
                        self.transition(PapState::Dying);
                    } else {
                        self.radius_mm = 400.0;
                        self.stats.health = (self.stats.health + 100.0).min(self.stats.max_health);
                        self.transition(PapState::Chase);
                    }
                }
            }

            PapState::LastMove => {
                if self.anim_finished {
                    self.boss_defeated_event = true;
                    self.transition(PapState::Dying);
                }
            }

            PapState::Dying => {}
        }
    }

    /// Apply incoming hit (`enReduceHP` `SLUS 0x00142200` reduces both HP and endurance by `ap`;
    /// `enPAPCtrlRush` `0x0015ae80` applies `0.4x` armor on light hits; Hyper Spray `sh2pc 0x004b5c90`).
    pub fn receive_hit(
        &mut self,
        ap: f32,
        damage_kind: u8,
        is_spray: bool,
        spray_power: u8,
        random_roll: f32,
    ) {
        if self.state == PapState::Dying || self.is_hugging_player {
            return;
        }
        self.last_damage_kind = damage_kind;

        if is_spray {
            if spray_power >= 2 {
                self.stats.health = 0.0;
                self.stats.endurance = 0.0;
                self.transition(PapState::Down);
                return;
            }
            self.stats.health = (self.stats.health - ap).max(0.0);
            if spray_power != 0 {
                self.transition(PapState::Confuse);
            }
            return;
        }

        if self.state == PapState::Rush && matches!(damage_kind, 1 | 2) {
            self.stats.health = (self.stats.health - ap * 0.4).max(0.0);
            if self.stats.health > 0.0 {
                return;
            }
        } else {
            // `enReduceHP` (`SLUS 0x00142200`): AP reduces both HP and Endurance.
            self.stats.health = (self.stats.health - ap).max(0.0);
            self.stats.endurance = (self.stats.endurance - ap).max(0.0);
        }

        let knocked_down = self.stats.health <= 0.0
            || self.stats.endurance <= 0.0
            || (self.stats.endurance < 100.0 && self.stats.endurance * 0.002 + 0.8 < random_roll);

        self.state = PapState::Damage;
        self.sub_state = if knocked_down { 2 } else { 1 };
        self.anim_finished = false;
        if knocked_down {
            self.radius_mm = 300.0;
        }
    }

    fn apply_root_motion(&mut self) {
        if self.root_motion_mm != 0.0 {
            self.position.x += self.yaw.sin() * self.root_motion_mm;
            self.position.z += self.yaw.cos() * self.root_motion_mm;
            self.root_motion_mm = 0.0;
        }
    }

    fn finish_attack(&mut self) {
        self.radius_mm = 400.0;
        self.is_hugging_player = false;
        self.anim_finished = false;
        let next = if self.is_boss {
            PapState::Chase
        } else {
            PapState::Wander
        };
        self.transition(next);
    }

    fn transition(&mut self, next: PapState) {
        self.state = next;
        self.sub_state = 0;
        self.anim_finished = false;
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
    use std::f32::consts::PI;

    #[test]
    fn test_abstract_daddy_boss_init_and_rush_armor() {
        let mut pap =
            AbstractDaddyController::new(1, Vec3::ZERO, 0.0, true, DifficultyLevel::Normal);
        assert_eq!(pap.state, PapState::Chase);
        assert_eq!(pap.stats.max_health, 5000.0);
        assert_eq!(pap.stats.max_endurance, 600.0);

        pap.receive_hit(50.0, 1, false, 0, 0.1);
        assert_eq!(pap.state, PapState::Damage);
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2000.0), 0.0, true, false);
        pap.anim_finished = true;
        pap.update(&target, 200.0, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(pap.state, PapState::Rush);

        // In Rush, light hit deals 0.4x (40.0) without interrupting Rush.
        let hp_before = pap.stats.health;
        pap.receive_hit(100.0, 1, false, 0, 0.99);
        assert_eq!(pap.state, PapState::Rush);
        assert!((pap.stats.health - (hp_before - 40.0)).abs() < 1e-3);
    }

    #[test]
    fn test_abstract_daddy_sight_and_grab_attack() {
        let mut pap =
            AbstractDaddyController::new(2, Vec3::ZERO, 0.0, false, DifficultyLevel::Normal);
        assert_eq!(pap.can_see_player(Vec3::new(0.0, 0.0, 500.0), 200.0), 2);
        assert_eq!(pap.can_see_player(Vec3::new(0.0, 0.0, 2600.0), 200.0), 0);

        let close = PerceivableTarget::new(Vec3::new(0.0, 0.0, 500.0), PI, true, false);
        pap.update(&close, 200.0, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(pap.state, PapState::Attack);
    }
}
