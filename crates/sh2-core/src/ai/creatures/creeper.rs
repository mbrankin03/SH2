//! Creeper cockroach swarm AI (`chr/tyu`, kind `0x202`) — 1:1 `SLUS_202.28` `Enemy/en_tyu.c`.
//!
//! # Ground Truth (`SLUS_202.28` `Enemy/en_tyu.c`):
//! - `enTYUInitData` (`0x00165000`): HP `[1.0, 250.0, 400.0, 600.0]` (`0x2ab2a0`),
//!   Endurance `[1.0, 100.0, 150.0, 240.0]` (`0x2ab2c0`), initial state by subtype (`0/2 => Straight`,
//!   `1/3 => OnWall`, `4 => OnCeiling`, `5 => Dead`).
//! - `enTYUCtrlAutomatic` (`0x00165220`): 9-state FSM:
//!   `0 = Straight`, `1 = Chase`, `2 = Gathering`, `3 = Escape`, `4 = OnWall`,
//!   `5 = OnCeiling`, `6 = Attack`, `7 = Down`, `8 = Dead`.
//! - `enTYUCanSeePlayer` (`0x001679c0`): Beginner (`mode == 0`) returns `0`; `dist < 150.0 &&
//!   angle < PI/3` (`1.0471976`) returns `2` (Attack); else `dist <= TYU_SEE_RANGE[mode]`
//!   (`[0.0, 500.0, 600.0, 750.0]`, `0x2ab330`) returns `1` (Chase).
//! - `enTYUGetSpeed` (`0x00167bf0`): `[0.8, 0.9, 1.0, 1.1] * 1250.0` mm/s (`0x2ab350`).
//! - `enTYUSetAttackTime` (`0x00167c50`, `0x2ab368`): `[60, 40, 30, 20]` ticks.
//! - `enTYUSetStopTime` (`0x00167cb0`, `0x2ab378`): `[60, 40, 30, 20]` ticks.
//! - `enTYUSetDownTime` (`0x00167d10`, `0x2ab388`): `[600, 500, 400, 300]` ticks.
//! - `enTYUCtrlDown` (`0x00166330`): reduces HP/endurance rather than insta-killing; on death waits
//!   `300` ticks unless instant-death; on recovery adds `+100.0` HP (`0x42c80000`) and enters
//!   `Escape` if endurance `<= 0` else `Chase`.

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_3};

/// Sight detection ranges by difficulty (`enTYUCanSeePlayer` `SLUS_202.28` `0x001679c0`, `0x2ab330`).
pub const TYU_SEE_RANGE_MM: [f32; 4] = [0.0, 500.0, 600.0, 750.0];

/// Base HP by difficulty (`enTYUInitData` `SLUS_202.28` `0x00165000`, `0x2ab2a0`).
pub const TYU_HP: [f32; 4] = [1.0, 250.0, 400.0, 600.0];

/// Base Endurance by difficulty (`enTYUInitData` `SLUS_202.28` `0x00165000`, `0x2ab2c0`).
pub const TYU_ENDURANCE: [f32; 4] = [1.0, 100.0, 150.0, 240.0];

/// Speed scale by difficulty (`enTYUGetSpeed` `SLUS_202.28` `0x00167bf0`, `0x2ab350`).
pub const TYU_SPEED_SCALE: [f32; 4] = [0.8, 0.9, 1.0, 1.1];

/// Base movement speed in mm/s (`enTYUGetSpeed` `SLUS_202.28` `0x00167bf0`).
pub const TYU_BASE_SPEED_MM_S: f32 = 1250.0;

/// Attack duration in 60 Hz ticks (`enTYUSetAttackTime` `SLUS_202.28` `0x00167c50`, `0x2ab368`).
pub const TYU_ATTACK_TIME_TICKS: [i16; 4] = [60, 40, 30, 20];

/// Hit-stop duration in 60 Hz ticks when endurance > 0 (`enTYUSetStopTime` `0x00167cb0`, `0x2ab378`).
pub const TYU_STOP_TIME_TICKS: [i16; 4] = [60, 40, 30, 20];

/// Knockdown duration in 60 Hz ticks when endurance <= 0 (`enTYUSetDownTime` `0x00167d10`, `0x2ab388`).
pub const TYU_DOWN_TIME_TICKS: [i16; 4] = [600, 500, 400, 300];

/// Death fade timer in 60 Hz ticks when not instant-death (`enTYUCtrlDown` `0x00166330` lines 804, 825, 887).
pub const TYU_DEATH_TIMER_TICKS: i16 = 300;

/// Angular turn speed per 60 Hz tick (`0x3db2b8c2 = 0.08726646` rad = 5°/tick in `enTYUCtrlStraight`/`Chase`/`Escape`).
pub const TYU_ROT_SPEED_RAD_PER_TICK: f32 = 0.087_266_46;

/// Melee attack trigger range in mm (`enTYUCanSeePlayer` `0x001679c0` line 1587).
pub const TYU_ATTACK_RANGE_MM: f32 = 150.0;

/// Melee attack half-angle cone in radians (`PI / 3 = 1.0471976`, `enTYUCanSeePlayer` `0x001679c0` line 1587).
pub const TYU_ATTACK_HALF_ANGLE_RAD: f32 = FRAC_PI_3;

/// Authentic 9-state Creeper FSM (`enTYUCtrlAutomatic` `SLUS_202.28` `0x00165220`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TyuState {
    /// `0`: `enTYUCtrlStraight` (`0x001653f0`) — floor wander / straight crawl.
    #[default]
    Straight = 0,
    /// `1`: `enTYUCtrlChase` (`0x00165760`) — actively pursuing player.
    Chase = 1,
    /// `2`: `enTYUCtrlGathering` (`0x00165930`) — converging on tribe alert (`enCommunicateTribe(3)`).
    Gathering = 2,
    /// `3`: `enTYUCtrlEscape` (`0x00165b10`) — orthogonal escape (`player_dir + PI/2`) after attack or knockdown.
    Escape = 3,
    /// `4`: `enTYUCtrlOnWall` (`0x00165d80`) — crawling on vertical wall (`height <= player_y - 2000.0`).
    OnWall = 4,
    /// `5`: `enTYUCtrlOnCeiling` (`0x00166050`) — crawling inverted on ceiling (`rot_z = PI`).
    OnCeiling = 5,
    /// `6`: `enTYUCtrlAttack` (`0x00166200`) — ankle bite (retail attack ID `57`, proto `0x33`).
    Attack = 6,
    /// `7`: `enTYUCtrlDown` (`0x00166330`) — hit-stop, floor knockdown, or dying countdown.
    Down = 7,
    /// `8`: `enTYUCtrlDead` (`0x00166870`) — permanently dead.
    Dead = 8,
}

#[derive(Debug, Clone)]
pub struct CreeperController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub state: TyuState,
    /// Sub-state byte (`*(i8*)(work + 4)` in `en_tyu.c`).
    pub sub_state: i8,
    /// Placement subtype (`*(u8*)(work + 5)`: `0/2` floor, `1/3` wall, `4` ceiling, `5` dead).
    pub subtype: u8,
    /// Countdown timer in 60 Hz ticks (`work + 0x94`).
    pub timer_ticks: f32,
    /// Optional tribe gathering position (`enCommunicateTribe(3)`).
    pub gather_target: Option<Vec3>,
    /// Deterministic RNG roll `[0.0, 1.0)` for `enTYUCtrlAttack` `0.1` repeat check.
    pub attack_repeat_roll: f32,
    pub difficulty: DifficultyLevel,
    pub stats: CreatureStats,
    pub speed: f32,
    pub anim: CreatureAnimationController,
}

impl CreeperController {
    /// Create a Creeper (`TYU`, `0x202`) at Normal difficulty (`enTYUInitData` `SLUS_202.28` `0x00165000`).
    pub fn new(id: u32, position: Vec3, yaw: f32) -> Self {
        Self::new_with_difficulty(id, position, yaw, DifficultyLevel::Normal)
    }

    /// Create a Creeper (`TYU`) with exact `SLUS_202.28` difficulty tables and subtype (`0..=5`).
    pub fn new_with_subtype(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
        subtype: u8,
    ) -> Self {
        let idx = difficulty.as_index();
        let initial_state = match subtype {
            5 => TyuState::Dead,
            4 => TyuState::OnCeiling,
            1 | 3 => TyuState::OnWall,
            _ => TyuState::Straight,
        };
        Self {
            id,
            position,
            yaw,
            state: initial_state,
            sub_state: 0,
            subtype,
            timer_ticks: 0.0,
            gather_target: None,
            attack_repeat_roll: 0.5,
            difficulty,
            stats: CreatureStats::new(TYU_HP[idx], TYU_ENDURANCE[idx], 0.0),
            speed: TYU_SPEED_SCALE[idx] * TYU_BASE_SPEED_MM_S,
            anim: CreatureAnimationController::new_creeper(),
        }
    }

    /// Create a Creeper (`TYU`) with exact `SLUS_202.28` difficulty tables (`enTYUInitData` `0x00165000`).
    pub fn new_with_difficulty(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
    ) -> Self {
        Self::new_with_subtype(id, position, yaw, difficulty, 0)
    }

    /// Evaluates `enTYUCanSeePlayer` (`SLUS_202.28` `0x001679c0` lines 1561-1600):
    /// - `0`: not detected (always `0` on Beginner `mode == 0`)
    /// - `1`: within `TYU_SEE_RANGE_MM[mode]` -> `Chase`
    /// - `2`: within `150.0` mm and `angle_diff < PI/3` (`1.0471976`) -> `Attack`
    #[must_use]
    pub fn can_see_player(&self, target: &PerceivableTarget) -> u8 {
        if self.difficulty == DifficultyLevel::Beginner {
            return 0;
        }
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let player_dir = dx.atan2(dz);
        let angle_diff = normalize_angle(player_dir - self.yaw).abs();
        if dist < TYU_ATTACK_RANGE_MM && angle_diff < TYU_ATTACK_HALF_ANGLE_RAD {
            2
        } else if dist <= TYU_SEE_RANGE_MM[self.difficulty.as_index()] {
            1
        } else {
            0
        }
    }

    /// Steps one tick of `enTYUCtrlAutomatic` (`SLUS_202.28` `0x00165220`).
    pub fn update(&mut self, target: &PerceivableTarget, dt: f32) {
        if self.state == TyuState::Dead {
            return;
        }

        let ticks = dt * 60.0;
        let max_turn = TYU_ROT_SPEED_RAD_PER_TICK * ticks;
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let player_dir = dx.atan2(dz);

        match self.state {
            TyuState::Straight => {
                // `enTYUCtrlStraight` (`0x001653f0`)
                if let Some(gather) = self.gather_target {
                    let _ = gather;
                    self.state = TyuState::Gathering;
                    self.sub_state = 0;
                    return;
                }
                self.position.x += self.yaw.sin() * self.speed * dt;
                self.position.z += self.yaw.cos() * self.speed * dt;
                match self.can_see_player(target) {
                    2 => {
                        self.state = TyuState::Attack;
                        self.sub_state = 0;
                    }
                    1 => {
                        self.state = TyuState::Chase;
                        self.sub_state = 0;
                    }
                    _ => {}
                }
            }

            TyuState::Chase => {
                // `enTYUCtrlChase` (`0x00165760`)
                self.turn_towards(player_dir, max_turn);
                let speed_rate = normalize_angle(player_dir - self.yaw)
                    .cos()
                    .clamp(0.25, 1.0);
                self.position.x += self.yaw.sin() * self.speed * speed_rate * dt;
                self.position.z += self.yaw.cos() * self.speed * speed_rate * dt;
                match self.can_see_player(target) {
                    2 => {
                        self.state = TyuState::Attack;
                        self.sub_state = 0;
                    }
                    0 => {
                        self.state = TyuState::Straight;
                        self.sub_state = 0;
                    }
                    _ => {}
                }
            }

            TyuState::Gathering => {
                // `enTYUCtrlGathering` (`0x00165930`)
                if let Some(goal) = self.gather_target {
                    let gdx = goal.x - self.position.x;
                    let gdz = goal.z - self.position.z;
                    let gdir = gdx.atan2(gdz);
                    self.turn_towards(gdir, max_turn);
                    self.position.x += self.yaw.sin() * self.speed * dt;
                    self.position.z += self.yaw.cos() * self.speed * dt;
                    match self.can_see_player(target) {
                        2 => {
                            self.state = TyuState::Attack;
                            self.sub_state = 0;
                        }
                        1 => {
                            self.state = TyuState::Chase;
                            self.sub_state = 0;
                        }
                        _ => {}
                    }
                } else {
                    self.state = TyuState::Straight;
                    self.sub_state = 0;
                }
            }

            TyuState::Escape => {
                // `enTYUCtrlEscape` (`0x00165b10` lines 521-543): flee orthogonal (`player_dir + PI/2`)
                let escape_dir = normalize_angle(player_dir + FRAC_PI_2);
                self.turn_towards(escape_dir, max_turn);
                self.position.x += self.yaw.sin() * self.speed * dt;
                self.position.z += self.yaw.cos() * self.speed * dt;
                if self.can_see_player(target) == 0 {
                    self.state = TyuState::Straight;
                    self.sub_state = 0;
                }
            }

            TyuState::OnWall => {
                // `enTYUCtrlOnWall` (`0x00165d80` lines 649-655): climb vertically, turn back if > 2000mm above player
                if self.position.y < target.position.y - 2000.0 {
                    self.yaw = std::f32::consts::PI;
                }
                self.position.y += self.yaw.cos() * self.speed * dt;
                if self.position.y >= target.position.y {
                    self.position.y = target.position.y;
                    self.subtype = 2;
                    self.state = TyuState::Straight;
                    self.sub_state = 0;
                }
            }

            TyuState::OnCeiling => {
                // `enTYUCtrlOnCeiling` (`0x00166050`)
                self.position.x += self.yaw.sin() * self.speed * dt;
                self.position.z += self.yaw.cos() * self.speed * dt;
            }

            TyuState::Attack => {
                // `enTYUCtrlAttack` (`0x00166200` lines 735-763):
                // Attack timer `[60, 40, 30, 20]` ticks (`0x2ab368`). On expiry: if still in
                // attack range, 10% repeat (`< 0.1`), else `Escape`; if in sight `Chase`; else `Straight`.
                if self.sub_state == 0 {
                    self.timer_ticks = TYU_ATTACK_TIME_TICKS[self.difficulty.as_index()] as f32;
                    self.sub_state = 1;
                }
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    match self.can_see_player(target) {
                        2 => {
                            if self.attack_repeat_roll < 0.1 {
                                self.sub_state = 0;
                            } else {
                                self.state = TyuState::Escape;
                                self.sub_state = 0;
                            }
                        }
                        1 => {
                            self.state = TyuState::Chase;
                            self.sub_state = 0;
                        }
                        _ => {
                            self.state = TyuState::Straight;
                            self.sub_state = 0;
                        }
                    }
                }
            }

            TyuState::Down => {
                // `enTYUCtrlDown` (`0x00166330`):
                // - `sub_state == -1 / -2`: falling from wall/ceiling at `1500.0` mm/s (`0x44bb8000`)
                // - `sub_state == 1`: stunned/knocked down; when timer expires, if endurance <= 0
                //   adds `+100.0` HP (`0x42c80000`) and enters `Escape`, else enters `Chase`.
                // - `sub_state == 2`: dying countdown (`300` ticks) -> `Dead`.
                if self.sub_state < 0 {
                    self.position.y += 1500.0 * dt;
                    if self.position.y >= target.position.y {
                        self.position.y = target.position.y;
                        self.subtype = if self.subtype == 3 { 2 } else { 0 };
                        self.sub_state = 1;
                    }
                } else {
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        if self.sub_state == 2 {
                            self.state = TyuState::Dead;
                            self.sub_state = 0;
                        } else if self.stats.endurance <= 0.0 {
                            self.stats.health =
                                (self.stats.health + 100.0).min(self.stats.max_health);
                            self.stats.endurance = self.stats.max_endurance;
                            self.state = TyuState::Escape;
                            self.sub_state = 0;
                        } else {
                            self.state = TyuState::Chase;
                            self.sub_state = 0;
                        }
                    }
                }
            }

            TyuState::Dead => {}
        }
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }

    /// Applies weapon damage matching `enTYUCtrlDown` (`SLUS_202.28` `0x00166330` lines 857-904):
    /// reduces HP and Endurance rather than insta-killing!
    /// - If hit while `OnWall` / `OnCeiling`: enters `Down` with `sub_state = -2` (fall to floor).
    /// - If `health <= 0`: instant death transitions to `Dead`, otherwise enters `Down` (`sub_state = 2`, `300` ticks).
    /// - If alive: sets `TYU_DOWN_TIME_TICKS` when `endurance <= 0` else `TYU_STOP_TIME_TICKS`, `sub_state = 1`.
    pub fn receive_damage(&mut self, ap: f32, sp: f32, instant_death: bool) {
        if self.state == TyuState::Dead {
            return;
        }
        let was_elevated = matches!(self.state, TyuState::OnWall | TyuState::OnCeiling);
        self.stats.health = (self.stats.health - ap).max(0.0);
        self.stats.endurance = (self.stats.endurance - sp).max(0.0);
        self.stats.hp_rate = if self.stats.max_health > 0.0 {
            (self.stats.health / self.stats.max_health) * 100.0
        } else {
            0.0
        };

        if self.stats.health <= 0.0 {
            if instant_death {
                self.state = TyuState::Dead;
                self.sub_state = 0;
            } else {
                self.state = TyuState::Down;
                self.sub_state = 2;
                self.timer_ticks = TYU_DEATH_TIMER_TICKS as f32;
            }
            return;
        }

        let idx = self.difficulty.as_index();
        self.state = TyuState::Down;
        self.timer_ticks = if self.stats.endurance <= 0.0 {
            TYU_DOWN_TIME_TICKS[idx] as f32
        } else {
            TYU_STOP_TIME_TICKS[idx] as f32
        };
        self.sub_state = if was_elevated { -2 } else { 1 };
    }

    /// Convenience wrapper applying a standard melee stomp/hit (`ap = 100.0, sp = 100.0`) without
    /// bypassing HP (fixes the legacy insta-kill stub).
    pub fn receive_hit(&mut self) {
        self.receive_damage(100.0, 100.0, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creeper_respects_hp_and_down_recovery_from_en_tyu() {
        let mut tyu =
            CreeperController::new_with_difficulty(1, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        assert_eq!(tyu.stats.health, 400.0);
        assert_eq!(tyu.stats.endurance, 150.0);
        assert_eq!(tyu.state, TyuState::Straight);

        // Non-lethal hit enters Down (sub_state = 1, stop timer = 30 ticks) — NOT instant death!
        tyu.receive_damage(100.0, 50.0, false);
        assert_eq!(tyu.state, TyuState::Down);
        assert_eq!(tyu.sub_state, 1);
        assert_eq!(tyu.stats.health, 300.0);
        assert_eq!(tyu.timer_ticks, 30.0);

        // After 30 ticks with endurance > 0, recovers to Chase
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 400.0), 0.0, false, false);
        for _ in 0..31 {
            tyu.update(&target, 1.0 / 60.0);
        }
        assert_eq!(tyu.state, TyuState::Chase);

        // Knockdown (endurance <= 0) uses 400-tick down timer and recovers +100 HP into Escape
        tyu.receive_damage(150.0, 200.0, false);
        assert_eq!(tyu.state, TyuState::Down);
        assert_eq!(tyu.timer_ticks, 400.0);
        for _ in 0..401 {
            tyu.update(&target, 1.0 / 60.0);
        }
        assert_eq!(tyu.state, TyuState::Escape);
        assert_eq!(tyu.stats.health, 250.0);
    }

    #[test]
    fn creeper_attack_and_escape_match_en_tyu_can_see_player() {
        let mut tyu =
            CreeperController::new_with_difficulty(2, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        // Within 150mm & forward cone -> Attack
        let close = PerceivableTarget::new(Vec3::new(0.0, 0.0, 120.0), 0.0, false, false);
        assert_eq!(tyu.can_see_player(&close), 2);
        tyu.update(&close, 1.0 / 60.0);
        assert_eq!(tyu.state, TyuState::Attack);
        // Attack lasts 30 ticks on Normal, then transitions to Escape (repeat_roll 0.5 >= 0.1)
        for _ in 0..32 {
            tyu.update(&close, 1.0 / 60.0);
        }
        assert_eq!(tyu.state, TyuState::Escape);
    }
}
