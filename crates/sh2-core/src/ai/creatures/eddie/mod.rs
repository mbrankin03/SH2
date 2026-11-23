//! Eddie Dombrowski Boss (`chr/edb`, kind `0x205`) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_edb.c`):
//!   - `0x001494c0` `enEDBInitData` — Room 1 (`phase=0, ammo=5`) vs Room 2 Meat Locker (`phase=1, ammo=6`).
//!   - `0x001496c0` `enEDBCtrlAutomatic` — 10-state dispatch (`0x002aa0f0`).
//!   - Exact binary timers: `Away` `120` (`0x78`), `Hidden` `60`/`600`, `Escape` `60`/`mode*60+180`,
//!     `Punch` `90` (`0x5a`), `Dead` `90` (`0x5a`). Animation clips complete via `anim_finished` (`*(i32*)(work+0x14) == -1`).
//! - Retail PC (`sh2pc.exe`) drift: see [`tables`].

pub mod tables;

pub use tables::{
    HangingMeatPendulum, EDB_AIM_ROT_MULT_PC, EDB_ATTACK_ID_PUNCH_PC, EDB_ATTACK_ID_SHOOT_PC,
    EDB_AUTO_RECOVERY, EDB_DEF_AFFORD, EDB_ENDURANCE, EDB_HP_ROOM1_PC, EDB_HP_ROOM2_PC,
    EDB_PUNCH_LIMIT_PC, EDB_ROOM1_DOOR_POS, EDB_ROOM2_CORNERS, EDB_RUN_SPEED_MULT_PC,
    EDB_TURN_STEP_RAD, EDB_WALK_SPEED_MULT_PC,
};

use crate::ai::creatures::{xz_dist, CreatureStats};
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI};

/// Eddie Boss state machine enum matching `0x002aa0f0` (`enEDBCtrlAutomatic` `SLUS 0x001496c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum EdbState {
    #[default]
    Approach = 0,
    Away = 1,
    Hidden = 2,
    Escape = 3,
    Search = 4,
    Shoot = 5,
    Punch = 6,
    Damage = 7,
    Escape2 = 8,
    Dead = 9,
}

/// Eddie Boss AI controller (`en_edb.c`, kind `0x205`).
#[derive(Debug, Clone)]
pub struct EddieController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    /// `0` = Room 1 corridor, `1` = Room 2 meat locker (`work+0x05`).
    pub phase: u8,
    pub state: EdbState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    /// Set by animation driver when current clip finishes (`*(i32*)(work + 0x14) == -1`).
    pub anim_finished: bool,
    /// Speed multiplier recovering at `+0.5/s` up to `1.0` (`work+0xb8`, slowed to `0.5` on hits).
    pub speed_scale: f32,
    /// Revolver chamber rounds (`5` in Room 1, `6` in Room 2, `work+0xbc`).
    pub ammo: u8,
    /// Remaining aggression budget before retreating (`work+0xc2`).
    pub afford: i8,
    /// Consecutive punch counter (`work+0xc4`).
    pub punch_count: u8,
    /// Collision radius (`200.0` normal, widens to `250.0` while aiming on retail PC `0x004b34a0`).
    pub radius_mm: f32,
    /// Exact binary state timer in 60 Hz ticks (`Away 120`, `Hidden 60/600`, `Escape 60/mode*60+180`, `Punch 90`, `Dead 90`).
    pub timer_ticks: f32,
    pub target_corner_idx: usize,
    pub mark_ring: [i8; 3],
    pub mark_idx: usize,
    pub active_attack_id: Option<u16>,
    /// Set when Room 1 flee finishes (`_DAT_0112645c |= 0x200000`).
    pub room1_escaped_event: bool,
    /// Set when Room 2 death finishes (`_DAT_0112645c |= 0x400000`).
    pub room2_dead_event: bool,
}

impl EddieController {
    /// Construct a new Eddie Boss controller (`enEDBInitData` `SLUS 0x001494c0` / `sh2pc 0x004b1360`).
    pub fn new(
        id: u32,
        position: Vec3,
        yaw: f32,
        is_room2: bool,
        difficulty: DifficultyLevel,
    ) -> Self {
        let idx = difficulty.as_index();
        let hp = if is_room2 {
            EDB_HP_ROOM2_PC[idx]
        } else {
            EDB_HP_ROOM1_PC[idx]
        };
        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            phase: u8::from(is_room2),
            state: if is_room2 {
                EdbState::Search
            } else {
                EdbState::Approach
            },
            sub_state: 0,
            stats: CreatureStats::new(hp, EDB_ENDURANCE[idx], EDB_AUTO_RECOVERY[idx]),
            anim_finished: false,
            speed_scale: 1.0,
            ammo: if is_room2 { 6 } else { 5 },
            afford: EDB_DEF_AFFORD[idx],
            punch_count: 0,
            radius_mm: 200.0,
            timer_ticks: 0.0,
            target_corner_idx: 0,
            mark_ring: [-1, -1, -1],
            mark_idx: 0,
            active_attack_id: None,
            room1_escaped_event: false,
            room2_dead_event: false,
        }
    }

    /// Walk speed in mm/s (`enEDBGetWalkSpeed` `SLUS 0x0014d9f0` / `sh2pc 0x004b1050`).
    pub fn walk_speed(&self, difficulty: DifficultyLevel) -> f32 {
        1000.0 * self.speed_scale * EDB_WALK_SPEED_MULT_PC[difficulty.as_index()]
    }

    /// Run speed in mm/s (`enEDBGetRunSpeed` `SLUS 0x0014da80` / `sh2pc 0x004b10b0`).
    pub fn run_speed(&self, difficulty: DifficultyLevel) -> f32 {
        1400.0 * self.speed_scale * EDB_RUN_SPEED_MULT_PC[difficulty.as_index()]
    }

    /// Evaluate sight (`enEDBCanSeePlayer` `SLUS 0x0014ce50`).
    pub fn can_see_player(&self, target_pos: Vec3) -> u8 {
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dist = xz_dist(self.position, target_pos);
        if dist > 5000.0 {
            return 0;
        }
        let ang = normalize_angle(dx.atan2(dz) - self.yaw).abs();
        if dist < 550.0 && ang < FRAC_PI_6 {
            3
        } else if dist < 4000.0 && ang < FRAC_PI_6 {
            2
        } else if ang <= 1.396_263_4 {
            1
        } else {
            0
        }
    }

    /// Advance Eddie Boss AI by `dt` seconds (`enEDBCtrlAutomatic` `SLUS 0x001496c0`).
    pub fn update(&mut self, target: &PerceivableTarget, difficulty: DifficultyLevel, dt: f32) {
        let ticks = dt * 60.0;
        let idx = difficulty.as_index();
        self.speed_scale = (self.speed_scale + dt * 0.5).min(1.0);
        self.active_attack_id = None;
        self.radius_mm = 200.0;

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = xz_dist(self.position, target.position);
        let player_dir = dx.atan2(dz);
        let see = self.can_see_player(target.position);
        let walk_spd = self.walk_speed(difficulty);
        let run_spd = self.run_speed(difficulty);
        let hp_pct = if self.stats.max_health > 0.0 {
            (self.stats.health / self.stats.max_health) * 100.0
        } else {
            0.0
        };

        match self.state {
            EdbState::Approach => {
                self.stats.update(dt);
                if see == 2 && dist >= 1000.0 {
                    self.transition(EdbState::Shoot);
                    return;
                }
                if dist < 1000.0 {
                    self.transition(EdbState::Punch);
                    return;
                }
                self.turn_towards(player_dir, EDB_TURN_STEP_RAD * ticks);
                self.position.x += self.yaw.sin() * walk_spd * dt;
                self.position.z += self.yaw.cos() * walk_spd * dt;
            }

            EdbState::Away => {
                if self.sub_state == 0 {
                    // `enEDBCtrlAway` `0x00149970`: timer `0x78` = `120` ticks.
                    self.timer_ticks = 120.0;
                    self.sub_state = 1;
                }
                let away_dir = normalize_angle(player_dir + PI);
                self.turn_towards(away_dir, EDB_TURN_STEP_RAD * ticks);
                self.position.x += self.yaw.sin() * walk_spd * dt;
                self.position.z += self.yaw.cos() * walk_spd * dt;
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    if dist > 1500.0 {
                        self.transition(EdbState::Shoot);
                    } else if dist <= 600.0 {
                        self.transition(EdbState::Punch);
                    } else {
                        self.transition(EdbState::Approach);
                    }
                }
            }

            EdbState::Hidden => match self.sub_state {
                0 => {
                    // `enEDBCtrlHidden` `0x00149e30`: timer `0x3c` = `60` ticks.
                    self.timer_ticks = 60.0;
                    self.sub_state = 1;
                }
                1 => {
                    if dist < 1000.0 {
                        self.transition(EdbState::Punch);
                        return;
                    }
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.add_afford(idx);
                        if self.ammo == 0 {
                            self.anim_finished = false;
                            self.sub_state = 4;
                        } else {
                            self.timer_ticks = 600.0;
                            self.sub_state = 5;
                        }
                    } else {
                        self.position.x += self.yaw.sin() * run_spd * dt;
                        self.position.z += self.yaw.cos() * run_spd * dt;
                    }
                }
                4 => {
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.ammo = 6;
                        self.timer_ticks = 600.0;
                        self.sub_state = 5;
                    }
                }
                _ => {
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.transition(EdbState::Search);
                    } else if see == 3 {
                        self.transition(EdbState::Punch);
                    } else if see == 2 {
                        self.transition(EdbState::Shoot);
                    } else if see == 1 {
                        self.turn_towards(player_dir, EDB_TURN_STEP_RAD * ticks);
                    }
                }
            },

            EdbState::Escape => match self.sub_state {
                0 => {
                    self.target_corner_idx = self.pick_unmarked_corner(target.position);
                    self.timer_ticks = 60.0;
                    self.sub_state = 2;
                }
                2 => {
                    let corner = EDB_ROOM2_CORNERS[self.target_corner_idx];
                    let cdx = corner.x - self.position.x;
                    let cdz = corner.z - self.position.z;
                    if xz_dist(self.position, corner) < 150.0 {
                        self.set_mark(self.target_corner_idx as i8);
                        self.add_afford(idx);
                        if self.ammo == 0 {
                            self.anim_finished = false;
                            self.sub_state = 3;
                        } else if idx < 2 {
                            self.transition(EdbState::Hidden);
                        } else {
                            self.timer_ticks = (idx as f32) * 60.0 + 180.0;
                            self.sub_state = 6;
                        }
                        return;
                    }
                    self.turn_towards(cdx.atan2(cdz), EDB_TURN_STEP_RAD * ticks);
                    self.position.x += self.yaw.sin() * run_spd * dt;
                    self.position.z += self.yaw.cos() * run_spd * dt;
                    if see > 1 {
                        self.transition(EdbState::Punch);
                    }
                }
                3 => {
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.ammo = 6;
                        self.timer_ticks = (idx as f32) * 60.0 + 180.0;
                        self.sub_state = 6;
                    }
                }
                _ => {
                    let aim_rot = EDB_TURN_STEP_RAD * EDB_AIM_ROT_MULT_PC[idx] * ticks;
                    self.turn_towards(player_dir, aim_rot);
                    self.timer_ticks -= ticks;
                    if see == 3 {
                        self.transition(EdbState::Punch);
                    } else if self.timer_ticks <= 0.0 || dist < 1500.0 {
                        self.afford = EDB_DEF_AFFORD[idx];
                        self.transition(if see < 1 {
                            EdbState::Search
                        } else {
                            EdbState::Punch
                        });
                    } else if see > 1 {
                        self.transition(EdbState::Shoot);
                    }
                }
            },

            EdbState::Search => {
                if self.sub_state == 0 {
                    self.mark_ring = [-1, -1, -1];
                    self.sub_state = 1;
                }
                match see {
                    3 => self.transition(EdbState::Punch),
                    2 => self.transition(EdbState::Shoot),
                    1 if self.punch_count < EDB_PUNCH_LIMIT_PC[idx] => {
                        self.transition(EdbState::Punch)
                    }
                    0 if dist < 1500.0 => self.transition(EdbState::Punch),
                    _ => {
                        self.turn_towards(player_dir, EDB_TURN_STEP_RAD * ticks);
                        self.position.x += self.yaw.sin() * 1000.0 * dt;
                        self.position.z += self.yaw.cos() * 1000.0 * dt;
                    }
                }
            }

            EdbState::Shoot => match self.sub_state {
                0 => {
                    self.anim_finished = false;
                    if self.ammo == 0 {
                        self.sub_state = 5;
                    } else {
                        self.add_afford(idx);
                        self.sub_state = 2;
                    }
                }
                2 => {
                    // Retail PC `0x004b34a0`: widens collision radius from 200 -> 250 mm while aiming.
                    self.radius_mm = 250.0;
                    let aim_rot = EDB_TURN_STEP_RAD * EDB_AIM_ROT_MULT_PC[idx] * ticks;
                    self.turn_towards(player_dir, aim_rot);
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.ammo = self.ammo.saturating_sub(1);
                        self.punch_count = 0;
                        self.sub_state = 3;
                    }
                }
                3 => {
                    self.active_attack_id = Some(EDB_ATTACK_ID_SHOOT_PC);
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.add_afford(idx);
                        if self.phase == 0 {
                            self.transition(EdbState::Approach);
                        } else if self.afford <= 0 || self.ammo == 0 || see < 2 {
                            // Retail PC `0x004b34a0`: low-HP hide threshold raised 20% -> 30%.
                            self.transition(if hp_pct < 30.0 {
                                EdbState::Hidden
                            } else {
                                EdbState::Search
                            });
                        } else {
                            self.sub_state = 2;
                        }
                    }
                }
                _ => {
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.ammo = 6;
                        self.sub_state = 0;
                    }
                }
            },

            EdbState::Punch => match self.sub_state {
                0 => {
                    if self.punch_count >= EDB_PUNCH_LIMIT_PC[idx] {
                        self.transition(EdbState::Shoot);
                    } else if see == 3 {
                        self.anim_finished = false;
                        self.sub_state = 3;
                    } else {
                        // `enEDBCtrlPunch` `0x0014b6a0`: timer `0x5a` = `90` ticks.
                        self.timer_ticks = 90.0;
                        self.sub_state = 1;
                    }
                }
                1 => {
                    if see == 3 {
                        self.anim_finished = false;
                        self.sub_state = 3;
                        return;
                    }
                    self.turn_towards(player_dir, EDB_TURN_STEP_RAD * ticks);
                    self.position.x += self.yaw.sin() * run_spd * dt;
                    self.position.z += self.yaw.cos() * run_spd * dt;
                    self.timer_ticks -= ticks;
                    if self.phase == 0 && self.timer_ticks <= 0.0 {
                        self.transition(EdbState::Away);
                    }
                }
                _ => {
                    self.turn_towards(player_dir, EDB_TURN_STEP_RAD * ticks);
                    self.active_attack_id = Some(EDB_ATTACK_ID_PUNCH_PC);
                    if self.anim_finished {
                        self.anim_finished = false;
                        self.punch_count = self.punch_count.saturating_add(1);
                        self.add_afford(idx);
                        if self.phase == 0 {
                            self.transition(EdbState::Away);
                        } else if self.afford <= 0 {
                            self.transition(EdbState::Escape);
                        } else {
                            self.transition(EdbState::Shoot);
                        }
                    }
                }
            },

            EdbState::Damage => {
                if self.anim_finished {
                    self.anim_finished = false;
                    if self.phase == 1 && self.stats.health <= 0.0 {
                        self.transition(EdbState::Dead);
                    } else if self.phase == 0 && self.stats.health <= 0.0 {
                        self.transition(EdbState::Escape2);
                    } else if self.phase == 0 {
                        self.transition(EdbState::Away);
                    } else if self.afford <= 0 {
                        self.transition(EdbState::Hidden);
                    } else if dist < 1500.0 {
                        self.transition(EdbState::Punch);
                    } else {
                        self.transition(EdbState::Shoot);
                    }
                }
            }

            EdbState::Escape2 => {
                let ddx = EDB_ROOM1_DOOR_POS.x - self.position.x;
                let ddz = EDB_ROOM1_DOOR_POS.z - self.position.z;
                let door_dist = xz_dist(self.position, EDB_ROOM1_DOOR_POS);
                let door_dir = ddx.atan2(ddz);
                if door_dist < 500.0 && normalize_angle(door_dir - self.yaw).abs() < FRAC_PI_2 {
                    self.room1_escaped_event = true;
                    return;
                }
                self.turn_towards(door_dir, EDB_TURN_STEP_RAD * ticks);
                self.position.x += self.yaw.sin() * 1400.0 * dt;
                self.position.z += self.yaw.cos() * 1400.0 * dt;
            }

            EdbState::Dead => {
                if self.sub_state == 0 {
                    // `enEDBCtrlDead` `0x0014c750`: timer `0x5a` = `90` ticks.
                    self.timer_ticks = 90.0;
                    self.sub_state = 1;
                }
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    self.room2_dead_event = true;
                }
            }
        }
    }

    /// Apply incoming damage (`enEDBSetDamage` `SLUS 0x0014cc60` / `sh2pc 0x004b0c60`).
    pub fn receive_hit(
        &mut self,
        ap: f32,
        damage_kind: u8,
        is_spray: bool,
        spray_power: u8,
        difficulty: DifficultyLevel,
    ) {
        if self.state == EdbState::Dead {
            return;
        }
        let idx = difficulty.as_index();

        if is_spray {
            if spray_power >= 2 {
                self.stats.health = 0.0;
                self.transition(if self.phase == 0 {
                    EdbState::Escape2
                } else {
                    EdbState::Dead
                });
                return;
            }
            if spray_power == 1 {
                self.speed_scale = 0.1;
            }
        }

        let mut mult = match damage_kind {
            0x13..=0x16 => 2.0,
            0x17 => 5.0,
            _ => 1.0,
        };

        let aiming = self.state == EdbState::Shoot && self.sub_state == 2;
        let punching = self.state == EdbState::Punch && self.sub_state <= 3;
        if idx > 1 && aiming && matches!(damage_kind, 1 | 2 | 0x0c | 0x0d) {
            mult *= 0.4;
            self.stats.health = (self.stats.health - ap * mult).max(0.0);
            if self.stats.health > 0.0 {
                return;
            }
        } else if punching && matches!(damage_kind, 1 | 2 | 0x0c | 0x0d | 0x0f | 0x12) {
            mult *= 0.5;
            if self.phase == 1 && idx < 3 {
                self.speed_scale = 0.5;
            }
            self.stats.health = (self.stats.health - ap * mult).max(0.0);
            if self.stats.health > 0.0 {
                return;
            }
        } else {
            self.stats.health = (self.stats.health - ap * mult).max(0.0);
            self.stats.endurance = (self.stats.endurance - ap * mult).max(0.0);
        }

        self.afford = self.afford.saturating_sub(1);

        let staggers = idx == 0
            || self.phase == 0
            || self.stats.health <= 0.0
            || self.stats.endurance <= 0.0
            || matches!(damage_kind, 0x11 | 0x13..=0x18);

        if staggers {
            if self.stats.endurance <= 0.0 {
                self.stats.endurance = self.stats.max_endurance;
            }
            self.speed_scale = 1.0;
            self.state = EdbState::Damage;
            self.sub_state = 1;
            self.anim_finished = false;
        } else {
            self.speed_scale = 0.5;
        }
    }

    fn add_afford(&mut self, idx: usize) {
        self.afford = self.afford.saturating_add(3).min(EDB_DEF_AFFORD[idx]);
    }

    fn set_mark(&mut self, id: i8) {
        self.mark_ring[self.mark_idx] = id;
        self.mark_idx = (self.mark_idx + 1) % self.mark_ring.len();
    }

    fn pick_unmarked_corner(&self, player_pos: Vec3) -> usize {
        EDB_ROOM2_CORNERS
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.mark_ring.contains(&(*i as i8)))
            .max_by(|(_, a), (_, b)| a.distance(player_pos).total_cmp(&b.distance(player_pos)))
            .map_or(0, |(i, _)| i)
    }

    fn transition(&mut self, next: EdbState) {
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

    #[test]
    fn test_eddie_aiming_radius_and_super_armor() {
        let mut edb = EddieController::new(1, Vec3::ZERO, 0.0, true, DifficultyLevel::Normal);
        assert_eq!(edb.stats.max_health, 5000.0);
        assert_eq!(edb.ammo, 6);

        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2000.0), 0.0, true, false);
        edb.update(&target, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(edb.state, EdbState::Shoot);
        edb.update(&target, DifficultyLevel::Normal, 1.0 / 60.0);
        edb.update(&target, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(edb.radius_mm, 250.0);

        let hp_before = edb.stats.health;
        edb.receive_hit(100.0, 1, false, 0, DifficultyLevel::Normal);
        assert_eq!(edb.state, EdbState::Shoot);
        assert!((edb.stats.health - (hp_before - 40.0)).abs() < 1e-3);
    }
}
