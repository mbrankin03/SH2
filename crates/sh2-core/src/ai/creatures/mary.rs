//! Mary / Maria Final Boss (`chr/bos`, kind `0x206`, internal type `0x0b`) AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_bos.c`):
//!   - `0x0013fa10` `enBOSInitData` — Phase 1 hovering (`state=0`) vs Phase 2 floor (`spawn=0x0d, state=5`),
//!     empty-ammo pity timer (`1800` ticks on `<Hard`, `18000` on `Hard+`).
//!   - `0x0013fbb0` `enBOSCtrlAutomatic` — 6-state dispatch (`0x002a9ad0`).
//!   - `0x0013fcb0` `Approach`, `0x0013ff00` `Distance`, `0x00140090` `Generate` (spawns 1 leader + 32 moths),
//!     `0x00140450` `Attack` (strangulation grab/hug), `0x00140720` `Attack2` (tentacle whip), `0x00140820` `Dead`.
//!   - `0x00140df0` `enBOSMoveExec` — vertical hover spring-damper centered at `Y = -17500.0` (`[-17700, -17300]`).
//! - Retail PC (`sh2pc.exe`) drift:
//!   - Attack IDs (`+6` shift): Grab `0x3b->0x41`, Hug `0x3c->0x42`, Tentacle `0x3d->0x43`, Moth grab `0x3e->0x44`, Moth hug `0x3f->0x45`.
//!   - HP (`0x004b4b80`): `[1200, 4000, 8000, 20000, 30000]` (SLUS `[5000, 7500, 10000, 20000, 40000]`).
//!   - MoveSpeed (`0x004b4a20`): `500.0 * [0.6, 0.8, 1.0, 3.0, 4.0]` (SLUS `[0.7, 0.8, 1.0, 1.5, 2.0]`).
//!   - Approach offset `600.0 -> 550.0` mm, Strangulation min dist `500.0 -> 400.0` mm (`0x004b4790`).
//!   - Floor obstacles (`0x00890050`, `0x00890060`): `bed_pos (58800, -16000, 58700, r=1000)`, `stair_pos (60900, -16000, 55600, r=1000)`.
//!   - Damage `0x15..0x16`: `10.0x -> 2.0x`, `0x0e`/`0x11`: `5.0x` (`0x004b4d90`).

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail PC Mary Boss HP & endurance table (`sh2pc 0x004b4b80`; SLUS `0x002a9a90` had `[5000, 7500, 10000, 20000, 40000]`).
pub const BOS_HP_PC: [f32; 5] = [1200.0, 4000.0, 8000.0, 20_000.0, 30_000.0];
/// Retail PC move speed multipliers (`sh2pc 0x004b4a20`, × `500.0` mm/s).
pub const BOS_MOVE_SPEED_MULT_PC: [f32; 5] = [0.6, 0.8, 1.0, 3.0, 4.0];

/// Hover center Y coordinate (`-17500.0` mm) and soft bounds (`[-17700, -17300]`, `enBOSMoveExec` `SLUS 0x00140df0`).
pub const BOS_HOVER_CENTER_Y_MM: f32 = -17_500.0;
pub const BOS_HOVER_MIN_Y_MM: f32 = -17_700.0;
pub const BOS_HOVER_MAX_Y_MM: f32 = -17_300.0;

/// Retail PC approach target offset (`550.0` mm; SLUS was `600.0`).
pub const BOS_APPROACH_OFFSET_PC_MM: f32 = 550.0;
/// Retail PC strangulation min/max distance (`400.0..700.0` mm, `sh2pc 0x004b4790`; SLUS min was `500.0`).
pub const BOS_STRANGLE_MIN_DIST_PC_MM: f32 = 400.0;
pub const BOS_STRANGLE_MAX_DIST_MM: f32 = 700.0;
/// Floor obstacle circles preventing strangulation (`sh2pc 0x00890050`, `0x00890060`).
pub const BOS_BED_OBSTACLE_PC: (Vec3, f32) = (Vec3::new(58_800.0, -16_000.0, 58_700.0), 1000.0);
pub const BOS_STAIR_OBSTACLE_PC: (Vec3, f32) = (Vec3::new(60_900.0, -16_000.0, 55_600.0), 1000.0);

/// Retail PC attack IDs (`+6` over SLUS `0x3b..=0x3f`).
pub const BOS_ATTACK_ID_STRANGLE_GRAB_PC: u16 = 0x41;
pub const BOS_ATTACK_ID_STRANGLE_HUG_PC: u16 = 0x42;
pub const BOS_ATTACK_ID_TENTACLE_PC: u16 = 0x43;
pub const BOS_ATTACK_ID_MOTH_GRAB_PC: u16 = 0x44;
pub const BOS_ATTACK_ID_MOTH_HUG_PC: u16 = 0x45;

/// Mary Final Boss state enum matching `0x002a9ad0` (`enBOSCtrlAutomatic` `SLUS 0x0013fbb0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum BosState {
    #[default]
    Approach = 0,
    Distance = 1,
    Generate = 2,
    Attack = 3,
    Attack2 = 4,
    Dead = 5,
}

/// Mary / Maria Final Boss AI controller (`en_bos.c`, kind `0x206`).
#[derive(Debug, Clone)]
pub struct MaryBossController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub move_angle: f32,
    /// `false` = Phase 1 hovering combat, `true` = Phase 2 floor execution (`spawn == 0x0d`).
    pub is_phase2_floor: bool,
    pub state: BosState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub cur_horiz_speed: f32,
    pub ang_vel: f32,
    pub vel_y_hover: f32,
    /// `0` = sway, `1` = leveling/moth, `2` = attack locked (`work+0xcc`).
    pub hover_mode: u8,
    pub timer_ticks: f32,
    /// Empty-ammo pity timer (`1800` ticks `<Hard`, `18000` ticks `Hard+`, `enBOSCheckPlayerLastBullet` `0x00140960`).
    pub last_bullet_timer_ticks: f32,
    pub moth_swarm_active: bool,
    pub is_hugging_player: bool,
    pub active_attack_id: Option<u16>,
    /// Set on Phase 1 defeat (`_DAT_0112646c |= 0x4000000`).
    pub phase1_cleared_event: bool,
    /// Set on Phase 2 final blow (`_DAT_0112646c |= 0x10000000`).
    pub ending_triggered_event: bool,
}

impl MaryBossController {
    /// Construct a new Mary Final Boss controller (`enBOSInitData` `SLUS 0x0013fa10` / `sh2pc 0x004b4b80`).
    pub fn new(
        id: u32,
        position: Vec3,
        yaw: f32,
        is_phase2_floor: bool,
        difficulty: DifficultyLevel,
    ) -> Self {
        let idx = difficulty_index(difficulty);
        let hp = BOS_HP_PC[idx];
        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            move_angle: normalize_angle(yaw),
            is_phase2_floor,
            state: if is_phase2_floor {
                BosState::Dead
            } else {
                BosState::Approach
            },
            sub_state: 0,
            stats: CreatureStats::new(hp, hp, 0.0),
            cur_horiz_speed: 0.0,
            ang_vel: 0.0,
            vel_y_hover: 0.0,
            hover_mode: 0,
            timer_ticks: 0.0,
            last_bullet_timer_ticks: if idx < 3 { 1800.0 } else { 18_000.0 },
            moth_swarm_active: false,
            is_hugging_player: false,
            active_attack_id: None,
            phase1_cleared_event: false,
            ending_triggered_event: false,
        }
    }

    /// Move speed in mm/s (`enBOSGetMoveSpeed` `SLUS 0x00141270` / `sh2pc 0x004b4a20`).
    pub fn move_speed_mm_s(&self, difficulty: DifficultyLevel) -> f32 {
        let base = 500.0 * BOS_MOVE_SPEED_MULT_PC[difficulty_index(difficulty)];
        if self.sub_state == 2 {
            base * 1.5
        } else {
            base
        }
    }

    /// Returns `true` if boss XZ is over the bed or stairwell obstacle (`enBOSCheckFloor` `SLUS 0x00140d10` / `sh2pc 0x00890050`).
    pub fn over_floor_obstacle(&self) -> bool {
        xz_dist(self.position, BOS_BED_OBSTACLE_PC.0) < BOS_BED_OBSTACLE_PC.1
            || xz_dist(self.position, BOS_STAIR_OBSTACLE_PC.0) < BOS_STAIR_OBSTACLE_PC.1
    }

    /// Evaluate `enBOSCanAttackPlayer` (`SLUS 0x00140a20` / `sh2pc 0x004b4790`).
    ///
    /// Returns `2` (strangulation eligible), `1` (tentacle slash), or `0` (out of range).
    pub fn can_attack_player(&self, target: &PerceivableTarget) -> u8 {
        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let ang_to_player = normalize_angle(dx.atan2(dz) - self.yaw).abs();
        if ang_to_player >= 0.087_266_46 {
            return 0;
        }
        let facing_diff = normalize_angle(target.yaw - self.yaw).abs();
        if dist > BOS_STRANGLE_MIN_DIST_PC_MM
            && dist < BOS_STRANGLE_MAX_DIST_MM
            && facing_diff > 1.396_263_4
        {
            2
        } else if dist < 600.0 {
            1
        } else {
            0
        }
    }

    /// Advance Mary Boss AI by `dt` seconds (`enBOSCtrlAutomatic` `SLUS 0x0013fbb0`).
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        player_ammo_empty: bool,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        let ticks = dt * 60.0;
        self.active_attack_id = None;

        // Empty-ammo pity timer (`enBOSCheckPlayerLastBullet` `0x00140960` / `sh2pc 0x004b4740`).
        if !self.is_phase2_floor && self.state != BosState::Dead && player_ammo_empty {
            self.last_bullet_timer_ticks -= ticks;
            if self.last_bullet_timer_ticks <= 0.0 {
                self.moth_swarm_active = false;
                self.phase1_cleared_event = true;
                self.transition(BosState::Dead);
                return;
            }
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let player_dir = dx.atan2(dz);
        let mut ang_accel = 0.0_f32;

        match self.state {
            BosState::Approach => {
                self.hover_mode = 0;
                let goal = Vec3::new(
                    target.position.x + target.yaw.sin() * BOS_APPROACH_OFFSET_PC_MM,
                    self.position.y,
                    target.position.z + target.yaw.cos() * BOS_APPROACH_OFFSET_PC_MM,
                );
                let gdir = (goal.x - self.position.x).atan2(goal.z - self.position.z);
                ang_accel = normalize_angle(gdir - self.move_angle) * 0.05;

                match self.can_attack_player(target) {
                    2 => {
                        self.transition(if self.over_floor_obstacle() {
                            BosState::Attack2
                        } else {
                            BosState::Attack
                        });
                        return;
                    }
                    1 => {
                        self.transition(BosState::Attack2);
                        return;
                    }
                    _ if dist >= 1500.0 && !self.moth_swarm_active => {
                        self.transition(BosState::Generate);
                        return;
                    }
                    _ => {}
                }
            }

            BosState::Distance => {
                if self.sub_state == 0 {
                    self.timer_ticks = 120.0;
                    self.sub_state = 1;
                }
                let retreat = Vec3::new(
                    target.position.x - player_dir.sin() * 1500.0,
                    self.position.y,
                    target.position.z - player_dir.cos() * 1500.0,
                );
                let rdir = (retreat.x - self.position.x).atan2(retreat.z - self.position.z);
                ang_accel = normalize_angle(rdir - self.move_angle) * 0.05;

                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 || self.can_attack_player(target) != 0 {
                    self.transition(BosState::Approach);
                }
            }

            BosState::Generate => match self.sub_state {
                0 => {
                    self.moth_swarm_active = true;
                    self.hover_mode = 1;
                    self.timer_ticks = 90.0;
                    self.sub_state = 1;
                }
                1 => {
                    self.active_attack_id = Some(BOS_ATTACK_ID_MOTH_GRAB_PC);
                    self.timer_ticks -= ticks;
                    if self.is_hugging_player {
                        self.sub_state = 2;
                    } else if self.timer_ticks <= 0.0 {
                        self.moth_swarm_active = false;
                        self.transition(BosState::Distance);
                    }
                }
                2 => {
                    self.active_attack_id = Some(BOS_ATTACK_ID_MOTH_HUG_PC);
                    if !self.is_hugging_player {
                        self.moth_swarm_active = false;
                        self.hover_mode = 0;
                        self.transition(BosState::Approach);
                    }
                }
                _ => self.transition(BosState::Approach),
            },

            BosState::Attack => match self.sub_state {
                0 => {
                    self.hover_mode = 2;
                    self.timer_ticks = 35.0;
                    self.sub_state = 1;
                }
                1 => {
                    self.active_attack_id = Some(BOS_ATTACK_ID_STRANGLE_GRAB_PC);
                    let lunge = Vec3::new(
                        target.position.x + target.yaw.sin() * BOS_APPROACH_OFFSET_PC_MM,
                        self.position.y,
                        target.position.z + target.yaw.cos() * BOS_APPROACH_OFFSET_PC_MM,
                    );
                    self.position.x += (lunge.x - self.position.x) * (dt * 4.0).min(1.0);
                    self.position.z += (lunge.z - self.position.z) * (dt * 4.0).min(1.0);
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        if self.is_hugging_player {
                            self.sub_state = 3;
                        } else {
                            self.timer_ticks = 30.0;
                            self.sub_state = 4;
                        }
                    }
                }
                3 => {
                    self.active_attack_id = Some(BOS_ATTACK_ID_STRANGLE_HUG_PC);
                    if !self.is_hugging_player {
                        self.timer_ticks = 30.0;
                        self.sub_state = 4;
                    }
                }
                _ => {
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.hover_mode = 0;
                        self.cur_horiz_speed = 0.0;
                        self.transition(BosState::Distance);
                    }
                }
            },

            BosState::Attack2 => {
                if self.sub_state == 0 {
                    self.timer_ticks = 40.0;
                    self.sub_state = 1;
                }
                self.active_attack_id = Some(BOS_ATTACK_ID_TENTACLE_PC);
                self.timer_ticks -= ticks;
                if self.timer_ticks <= 0.0 {
                    self.transition(BosState::Approach);
                }
            }

            BosState::Dead => {
                if self.is_phase2_floor {
                    if self.sub_state == 1 {
                        self.timer_ticks -= ticks;
                        if self.timer_ticks <= 0.0 {
                            self.ending_triggered_event = true;
                            self.sub_state = 2;
                        }
                    }
                } else {
                    if self.sub_state == 0 {
                        self.timer_ticks = 60.0;
                        self.sub_state = 1;
                    }
                    self.timer_ticks -= ticks;
                    if self.timer_ticks <= 0.0 {
                        self.phase1_cleared_event = true;
                        self.sub_state = 2;
                    }
                }
                return;
            }
        }

        // Hover physics (`enBOSMoveExec` `SLUS 0x00140df0`).
        if self.hover_mode < 2 {
            self.ang_vel = self.ang_vel * 0.95 + ang_accel;
            let desired_move = normalize_angle(self.move_angle + self.ang_vel);
            let mstep = normalize_angle(desired_move - self.move_angle)
                .clamp(-0.034_906_585 * ticks, 0.034_906_585 * ticks);
            self.move_angle = normalize_angle(self.move_angle + mstep);

            let max_spd = self.move_speed_mm_s(difficulty);
            self.cur_horiz_speed = (self.cur_horiz_speed + dt * 250.0).min(max_spd);
            self.position.x += self.move_angle.sin() * self.cur_horiz_speed * dt;
            self.position.z += self.move_angle.cos() * self.cur_horiz_speed * dt;
        }

        // Facing yaw tracks player at 1°/tick (`0.017453292` rad/tick, skipped during strangle loop).
        if !(self.state == BosState::Attack && self.sub_state == 3) {
            let ystep = normalize_angle(player_dir - self.yaw)
                .clamp(-0.017_453_292 * ticks, 0.017_453_292 * ticks);
            self.yaw = normalize_angle(self.yaw + ystep);
        }

        // Vertical spring-damper hover around `Y = -17500.0` (`[-17700, -17300]`).
        let force = (BOS_HOVER_CENTER_Y_MM - self.position.y) * 0.2;
        self.vel_y_hover =
            ((self.vel_y_hover + force) - (self.position.y - BOS_HOVER_CENTER_Y_MM) * 0.01) * 0.98;
        if (self.position.y < BOS_HOVER_MIN_Y_MM && self.vel_y_hover < 0.0)
            || (self.position.y >= BOS_HOVER_MAX_Y_MM && self.vel_y_hover > 0.0)
        {
            self.vel_y_hover *= 0.5;
        }
        if self.position.y <= BOS_HOVER_MAX_Y_MM || self.vel_y_hover < 0.0 {
            self.position.y += self.vel_y_hover * dt;
        }
    }

    /// Apply incoming damage (`enBOSCheckDamage` `SLUS 0x00140bc0` / `sh2pc 0x004b4d90`).
    pub fn receive_hit(&mut self, ap: f32, damage_kind: u8, is_spray: bool, spray_power: u8) {
        if self.is_hugging_player {
            return;
        }
        if self.is_phase2_floor {
            if self.sub_state == 0 {
                self.timer_ticks = 120.0;
                self.sub_state = 1;
            }
            return;
        }
        if self.state == BosState::Dead {
            return;
        }
        if is_spray && spray_power >= 2 {
            self.stats.health = 0.0;
            self.moth_swarm_active = false;
            self.transition(BosState::Dead);
            return;
        }

        // Retail PC `0x004b4d90`: 0x15..=0x16 deal 2.0x (SLUS was 10.0x), 0x0e/0x11 deal 5.0x.
        let mult = match damage_kind {
            0x15 | 0x16 => 2.0,
            0x0e | 0x11 => 5.0,
            _ => 1.0,
        };
        self.stats.health = (self.stats.health - ap * mult).max(0.0);
        if self.stats.health <= 0.0 {
            self.moth_swarm_active = false;
            self.transition(BosState::Dead);
        } else if matches!(self.state, BosState::Distance) {
            self.transition(BosState::Approach);
        } else if matches!(self.state, BosState::Attack | BosState::Attack2) {
            self.transition(BosState::Distance);
        }
    }

    fn transition(&mut self, next: BosState) {
        self.state = next;
        self.sub_state = 0;
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
    use std::f32::consts::PI;

    #[test]
    fn test_mary_boss_obstacle_fallback_and_pity_timer() {
        // Positioned over bed_pos (58800, -17500, 58700) -> strangulation falls back to tentacle Attack2.
        let mut mary = MaryBossController::new(
            1,
            Vec3::new(58_800.0, BOS_HOVER_CENTER_Y_MM, 58_700.0),
            0.0,
            false,
            DifficultyLevel::Normal,
        );
        assert!(mary.over_floor_obstacle());
        let player = PerceivableTarget::new(
            Vec3::new(58_800.0, BOS_HOVER_CENTER_Y_MM, 59_250.0),
            PI,
            true,
            false,
        );
        mary.update(&player, false, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(mary.state, BosState::Attack2);

        // Empty-ammo pity timer auto-clears Phase 1.
        mary.last_bullet_timer_ticks = 1.0;
        mary.update(&player, true, DifficultyLevel::Normal, 1.0 / 60.0);
        assert!(mary.phase1_cleared_event);
        assert_eq!(mary.state, BosState::Dead);
    }
}
