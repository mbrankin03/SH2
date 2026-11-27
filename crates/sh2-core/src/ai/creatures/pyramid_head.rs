//! Pyramid Head (`chr/red`, `chr/oni`, kind 0x208/0x209) AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_red.c`, `src/Enemy/en_oni.c`
//! - PC executable `sh2pc.exe`:
//!   - `0x4c0b00`: Pyramid Head state table (`RED_WANDER`, `RED_CHASE`, `RED_HUNT`, `RED_ATTACK`, `RED_GRAB`, `RED_STAND`).
//!   - Unflinching relentless advance (immune to standard stagger/flinch).
//!   - Great Knife execution swings (AP 1000 - instant kill) and choke grab attacks.

use crate::ai::creatures::CreatureStats;
use crate::ai::pathfinding::{
    check_path_ray_with_source, EnemyPathState, PathEnemyType, RoomForbiddenAreas,
};
use crate::ai::sensory::{
    evaluate_sensory_with_wall_source, normalize_angle, AiWallSource, DifficultyLevel,
    PerceivableTarget, RoomLighting, SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sh2_assets::formats::cld::WallSegment;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RedState {
    #[default]
    Wander = 0,
    Chase = 1,
    Hunt = 2,
    Stairs = 3,
    Attack = 4,
    Grab = 5,
    Confuse = 6,
    Leave = 7,
    Stand = 8,
}

/// Blue Creek Apartments boss stairwell waypoints (`ap_boss_point[3]` `SLUS 0x002aa9d0`).
pub const RED_AP_STAIR_POINTS: [Vec3; 3] = [
    Vec3::new(100_000.0, -1700.0, -101_000.0),
    Vec3::new(98_850.0, -1700.0, -99_400.0),
    Vec3::new(99_200.0, -1700.0, -99_000.0),
];

/// Blue Creek Apartments boss retreat waypoints (`ap_boss_end_point[6]` `SLUS 0x002aaa00` / `sh2pc 0x00890418`, 0 diffs).
pub const RED_AP_BATTLE_END_POINTS: [Vec3; 6] = [
    Vec3::new(98_850.0, -1700.0, -99_400.0),
    Vec3::new(98_800.0, -1700.0, -99_000.0),
    Vec3::new(101_136.0, -850.0, -98_992.0),
    Vec3::new(101_136.0, -850.0, -99_820.0),
    Vec3::new(98_800.0, 0.0, -99_800.0),
    Vec3::new(98_800.0, 0.0, -98_620.0),
];

/// Retail PC Pyramid Head Great Knife & seize/choke attack IDs (`+6` over SLUS `0x2c..0x30`, `sh2pc 0x004c03a0`/`0x004c0530`).
pub const RED_ATTACK_ID_SWING_PC: u16 = 0x32;
pub const RED_ATTACK_ID_CLEAVE_PC: u16 = 0x33;
pub const RED_ATTACK_ID_SEIZE_PC: u16 = 0x35;
pub const RED_ATTACK_ID_CHOKE_PC: u16 = 0x36;

/// Blue Creek Apartments stairwell survival timer (`work+0xa4` in `enREDInitData` `SLUS 0x0015d410`).
/// - Easy/Normal/Hard (`mode < 3`): `0x2a30` = `10800` ticks (180 s at 60 Hz).
/// - Extra (`mode >= 3`): `18000` ticks (300 s at 60 Hz).
pub const RED_STAIRWELL_TIMER_TICKS: f32 = 10_800.0;
pub const RED_STAIRWELL_TIMER_EXTRA_TICKS: f32 = 18_000.0;

#[derive(Debug, Clone)]
pub struct PyramidHeadController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub state: RedState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    /// True for Blue Creek Apartments boss encounter (`sub_type == 6` / `work+0x05 == 2`, `enREDInitData` `0x0015d410`).
    pub is_boss_encounter: bool,
    /// Stairwell survival countdown in 60 Hz ticks (`work+0xa4` in `enREDCtrlAutomatic` `0x0015d6d0`).
    pub stairwell_timer_ticks: f32,
    /// Set by animation driver when current clip finishes (`*(i32*)(work + 0x14) == -1`).
    pub anim_finished: bool,
    pub state_timer: f32,
    /// Exact binary timer in 60 Hz ticks for Confuse (`600 - 90 * mode`) and Leave stair-guard (`300` ticks).
    pub timer_ticks: f32,
    /// True while choking the player in [`RedState::Grab`] (`enREDCtrlSeize` `0x0015e7d0`).
    pub is_hugging_player: bool,
    /// Optional nearby enemy for Hyper Spray friendly-fire in [`RedState::Confuse`] (`0x0015eb40`).
    pub confuse_friendly_target: Option<Vec3>,
    /// Active attack ID (e.g. `0x35` seize, `0x36` choke).
    pub active_attack_id: Option<u16>,
    /// Set when [`RedState::Leave`] reaches `ap_boss_end_point[5]` (`_DAT_01126440 |= 0x40000`).
    pub battle_end_complete: bool,
    /// Obstacle-avoidance pathfinding state (`enInitPath` `0x00144aa0` / `enSetPath` `0x00144ae0`).
    pub path_state: EnemyPathState,
    pub sensory: SensoryProfile,
}

impl PyramidHeadController {
    /// Create Pyramid Head (`RED`, `0x208`) with authentic Normal boss HP/Endurance
    /// (`enREDInitData` `SLUS_202.28` `0x0015d410`).
    pub fn new(id: u32, position: Vec3, yaw: f32) -> Self {
        Self::new_with_difficulty(id, position, yaw, DifficultyLevel::Normal, true)
    }

    /// Create Pyramid Head (`RED`) with exact `SLUS_202.28` `enREDInitData` (`0x0015d410`) tables:
    /// - Boss (subtype 6) HP (`0x2aaa70`): `[1200.0, 3000.0, 4000.0, 5000.0]`
    /// - Non-boss invincible wander HP (`0x49742400`): `1_000_000.0`
    /// - Endurance (`0x2aaa90`): `[500.0, 700.0, 1000.0, 2000.0]` (`× 0.3` in apartment boss subtype 6)
    /// - Stairwell timer `work+0xa4`: `0x2a30` (`10800` ticks) if `mode < 3`, else `18000` ticks
    pub fn new_with_difficulty(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
        is_boss_encounter: bool,
    ) -> Self {
        const RED_HP: [f32; 4] = [1200.0, 3000.0, 4000.0, 5000.0];
        const RED_END: [f32; 4] = [500.0, 700.0, 1000.0, 2000.0];
        let idx = difficulty.as_index();
        let (hp, end) = if is_boss_encounter {
            (RED_HP[idx], RED_END[idx] * 0.3)
        } else {
            (1_000_000.0, RED_END[idx])
        };
        let stairwell_timer_ticks = if !is_boss_encounter {
            0.0
        } else if idx >= 3 {
            RED_STAIRWELL_TIMER_EXTRA_TICKS
        } else {
            RED_STAIRWELL_TIMER_TICKS
        };

        Self {
            id,
            position,
            yaw,
            state: RedState::Wander,
            sub_state: 0,
            stats: CreatureStats::new(hp, end, 0.0),
            is_boss_encounter,
            stairwell_timer_ticks,
            anim_finished: false,
            state_timer: 0.0,
            timer_ticks: 0.0,
            is_hugging_player: false,
            confuse_friendly_target: None,
            active_attack_id: None,
            battle_end_complete: false,
            path_state: EnemyPathState::new(yaw),
            sensory: SensoryProfile::pyramid_head(),
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        self.update_with_world(target, lighting, difficulty, &[], None, 0.0, dt);
    }

    /// Update Pyramid Head AI with `.cld` wall LOS (`enCheckHitEyes` `0x00145540`), `enSetPath`
    /// (`0x00144ae0`), and [`RoomForbiddenAreas`].
    #[allow(clippy::too_many_arguments)]
    pub fn update_with_world(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        walls: &[WallSegment],
        forbidden: Option<&RoomForbiddenAreas>,
        path_sway_sample: f32,
        dt: f32,
    ) {
        self.update_with_wall_source(
            target,
            lighting,
            difficulty,
            AiWallSource::Slice(walls),
            forbidden,
            path_sway_sample,
            dt,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_with_wall_source(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        wall_source: AiWallSource<'_>,
        forbidden: Option<&RoomForbiddenAreas>,
        path_sway_sample: f32,
        dt: f32,
    ) {
        self.stats.update(dt);
        self.state_timer += dt;

        // `enREDCtrlAutomatic` (`SLUS_202.28` `0x0015d6d0`):
        // When `variant == 2` (`is_boss_encounter`) and `state != Leave` (`7`), decrement `work+0xa4`
        // by `shGetDF()` (`dt * 60.0`); on expiry (`< 1`), transition to `Leave` and scale max HP by `1.5`.
        if self.is_boss_encounter && self.state != RedState::Leave {
            self.stairwell_timer_ticks -= dt * 60.0;
            if self.stairwell_timer_ticks <= 0.0 {
                self.stairwell_timer_ticks = 0.0;
                self.state = RedState::Leave;
                self.sub_state = 0;
                self.stats.max_health *= 1.5;
                self.active_attack_id = None;
            }
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        let (notice, _) = evaluate_sensory_with_wall_source(
            &self.sensory,
            self.position,
            self.yaw,
            0.0,
            target,
            lighting,
            difficulty,
            wall_source,
            dt,
        );

        match self.state {
            RedState::Wander => {
                // Relentless slow march (280 units/s)
                self.position.x += self.yaw.sin() * 280.0 * dt;
                self.position.z += self.yaw.cos() * 280.0 * dt;

                if notice != SensoryNotice::None {
                    self.state = RedState::Chase;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                }
            }

            RedState::Chase => {
                let steer_dir =
                    if !wall_source.is_empty() || forbidden.is_some_and(|f| !f.rects.is_empty()) {
                        let ticks = (dt * 60.0).round().max(1.0) as i16;
                        self.path_state.cur_angle = self.yaw;
                        self.path_state.set_path(
                            PathEnemyType::PyramidHead,
                            self.position,
                            target.position,
                            path_sway_sample,
                            ticks,
                            |a, b| check_path_ray_with_source(a, b, 150.0, wall_source, forbidden),
                        );
                        self.path_state.target_angle
                    } else {
                        target_dir
                    };
                self.turn_towards(steer_dir, 1.8 * dt);

                // Overhead execution swing reach: 1800 units (cleave `0x33` in lunge cone, swing `0x32` close)
                if dist <= 1800.0 && (wall_source.is_empty() || notice != SensoryNotice::None) {
                    self.active_attack_id = Some(if notice == SensoryNotice::InLungeRange {
                        RED_ATTACK_ID_CLEAVE_PC
                    } else {
                        RED_ATTACK_ID_SWING_PC
                    });
                    self.state = RedState::Attack;
                    self.state_timer = 0.0;
                    return;
                }

                // Slow unstoppable march (340 units/s) dragging Great Knife
                self.position.x += self.yaw.sin() * 340.0 * dt;
                self.position.z += self.yaw.cos() * 340.0 * dt;
            }

            RedState::Attack => {
                // Retail PC `0x004c03a0`: tracks player at 1°/tick (`0.017453292` rad) on Hard during cleave.
                if difficulty == DifficultyLevel::Hard {
                    self.turn_towards(target_dir, 0.017_453_292 * dt * 60.0);
                }
                if self.state_timer >= 2.2 {
                    self.state = RedState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            RedState::Hunt => {
                // `enREDCtrlBerserk` (`SLUS 0x0015deb0`): approaches to 250 mm offset from target, then attacks.
                let goal = if dist < 250.0 {
                    target.position
                } else {
                    target.position + (self.position - target.position) * (250.0 / dist)
                };
                let gdx = goal.x - self.position.x;
                let gdz = goal.z - self.position.z;
                self.turn_towards(gdx.atan2(gdz), 1.8 * dt);
                if dist <= 1250.0 && normalize_angle(target_dir - self.yaw).abs() < 0.174_532_92 {
                    self.state = RedState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    return;
                }
                self.position.x += self.yaw.sin() * 340.0 * dt;
                self.position.z += self.yaw.cos() * 340.0 * dt;
            }

            RedState::Stairs => {
                // `enREDCtrlStair` (`SLUS 0x0015e0a0`): traverses `ap_boss_point[0..3]`.
                if target.position.z < -99_400.0 && self.sub_state < 5 {
                    self.state = RedState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    return;
                }
                if self.sub_state < 4
                    && dist < 1500.0
                    && normalize_angle(target_dir - self.yaw).abs() < 0.122_173_05
                {
                    self.state = RedState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    return;
                }
                let wp = match self.sub_state {
                    0 | 1 => {
                        self.sub_state = 1;
                        RED_AP_STAIR_POINTS[1]
                    }
                    2 => RED_AP_STAIR_POINTS[2],
                    3 => target.position,
                    5 => RED_AP_STAIR_POINTS[1],
                    _ => RED_AP_STAIR_POINTS[0],
                };
                let wdx = wp.x - self.position.x;
                let wdz = wp.z - self.position.z;
                let wdist = (wdx * wdx + wdz * wdz).sqrt();
                self.turn_towards(wdx.atan2(wdz), 1.8 * dt);
                self.position.x += self.yaw.sin() * 340.0 * dt;
                self.position.z += self.yaw.cos() * 340.0 * dt;
                if wdist < 50.0 {
                    if self.sub_state == 1 {
                        self.sub_state = 2;
                    } else if self.sub_state == 2 {
                        self.sub_state = 3;
                    } else if self.sub_state == 5 {
                        self.sub_state = 6;
                    } else if self.sub_state >= 6 {
                        self.state = RedState::Chase;
                        self.sub_state = 0;
                    }
                }
            }

            RedState::Grab => {
                // `enREDCtrlSeize` (`SLUS 0x0015e7d0` / `sh2pc 0x004c0530`): reach (`0x35`), choke (`0x36`), release.
                self.active_attack_id = None;
                match self.sub_state {
                    0 => {
                        self.anim_finished = false;
                        self.sub_state = 1;
                    }
                    1 => {
                        self.active_attack_id = Some(RED_ATTACK_ID_SEIZE_PC);
                        // Tracks player at 10°/tick (`0.17453292` rad/tick, `0x0015e7d0`).
                        self.turn_towards(target_dir, 0.174_532_92 * dt * 60.0);
                        if self.anim_finished {
                            self.anim_finished = false;
                            self.sub_state = if self.is_hugging_player { 3 } else { 4 };
                        }
                    }
                    3 => {
                        self.active_attack_id = Some(RED_ATTACK_ID_CHOKE_PC);
                        if !self.is_hugging_player || !target.is_alive {
                            self.anim_finished = false;
                            self.sub_state = 4;
                        }
                    }
                    _ => {
                        if self.anim_finished {
                            self.anim_finished = false;
                            self.state = RedState::Chase;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                }
            }

            RedState::Confuse => {
                // `enREDCtrlConfuse` (`SLUS 0x0015eb40`): friendly-fire within `|dY| < 250` & `dist_xz < 900`,
                // else stunned for `600 - 90 * mode` ticks -> `Wander`.
                match self.sub_state {
                    0 => {
                        if let Some(other) = self.confuse_friendly_target {
                            let odx = other.x - self.position.x;
                            let odz = other.z - self.position.z;
                            let odist = (odx * odx + odz * odz).sqrt();
                            if (other.y - self.position.y).abs() < 250.0 && odist < 900.0 {
                                self.anim_finished = false;
                                self.sub_state = 4;
                                return;
                            }
                        }
                        self.timer_ticks = (600 - 90 * difficulty.as_index() as i32) as f32;
                        self.sub_state = 2;
                    }
                    2 => {
                        self.timer_ticks -= dt * 60.0;
                        if self.timer_ticks <= 0.0 {
                            self.state = RedState::Wander;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                    4 | 5 => {
                        if let Some(other) = self.confuse_friendly_target {
                            let odx = other.x - self.position.x;
                            let odz = other.z - self.position.z;
                            self.turn_towards(odx.atan2(odz), 1.8 * dt);
                        }
                        if self.anim_finished {
                            self.anim_finished = false;
                            self.sub_state += 1;
                            if self.sub_state >= 6 {
                                self.confuse_friendly_target = None;
                                self.state = RedState::Wander;
                                self.sub_state = 0;
                            }
                        }
                    }
                    _ => {
                        self.state = RedState::Wander;
                        self.sub_state = 0;
                    }
                }
            }

            RedState::Leave => {
                // `enREDCtrlBattleEnd` (`SLUS 0x0015eeb0` / `sh2pc 0x004c0a70`):
                // 300-tick stair guard (with sub 2/3 counter-attacks `0x32`/`0x33`) -> walks `ap_boss_end_point[0..6]`.
                self.active_attack_id = None;
                match self.sub_state {
                    0 => {
                        self.timer_ticks = 300.0;
                        self.sub_state = 1;
                    }
                    1 => {
                        self.turn_towards(target_dir, 1.8 * dt);
                        if target.position.z > -99_400.0 && dist < 1500.0 {
                            // Retail PC `0x004c0a70`: stair-guard counter-attack sub 2 (swing) vs sub 3 (cleave).
                            self.anim_finished = false;
                            self.sub_state = if dist <= 900.0
                                && target.position.x <= 99_000.0
                                && self.position.x >= 99_000.0
                            {
                                2
                            } else {
                                3
                            };
                            return;
                        }
                        self.timer_ticks -= dt * 60.0;
                        if self.timer_ticks <= 0.0 && target.position.z < -99_400.0 {
                            self.sub_state = if self.position.z < -99_400.0 {
                                4
                            } else if self.position.z < -99_150.0 {
                                5
                            } else {
                                6
                            };
                        }
                    }
                    2 | 3 => {
                        self.active_attack_id = Some(if self.sub_state == 2 {
                            RED_ATTACK_ID_SWING_PC
                        } else {
                            RED_ATTACK_ID_CLEAVE_PC
                        });
                        if self.anim_finished && target.position.z <= -99_400.0 {
                            self.anim_finished = false;
                            self.sub_state = 1;
                        }
                    }
                    4..=9 => {
                        if self.sub_state <= 6
                            && target.position.z > -100_000.0
                            && self.position.x < 100_000.0
                            && dist < 1500.0
                        {
                            // Retail PC `0x004c0a70`: interrupts stair walk to counter-attack approaching player.
                            self.anim_finished = false;
                            self.sub_state = if dist <= 900.0 { 2 } else { 3 };
                            return;
                        }
                        let wp_idx = (self.sub_state - 4) as usize;
                        let wp = RED_AP_BATTLE_END_POINTS[wp_idx];
                        let wdx = wp.x - self.position.x;
                        let wdz = wp.z - self.position.z;
                        let wdist = (wdx * wdx + wdz * wdz).sqrt();
                        self.turn_towards(wdx.atan2(wdz), 1.8 * dt);
                        self.position.x += self.yaw.sin() * 340.0 * dt;
                        self.position.z += self.yaw.cos() * 340.0 * dt;
                        if self.sub_state == 9 {
                            self.position.y += 500.0 * dt;
                        }
                        if wdist < 50.0 {
                            if self.sub_state < 9 {
                                self.sub_state += 1;
                            } else {
                                self.battle_end_complete = true;
                            }
                        }
                    }
                    _ => {}
                }
            }

            RedState::Stand => {
                // `enREDCtrlOnlyWalk` (`SLUS 0x0015f3a0` / `sh2pc 0x004c0f50`): scripted walk-only.
                self.position.x += self.yaw.sin() * 280.0 * dt;
                self.position.z += self.yaw.cos() * 280.0 * dt;
            }
        }
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }

    /// Apply incoming hit (`enREDSetDamage` `SLUS_202.28` `0x0015f480`).
    ///
    /// In `variant == 2` (`is_boss_encounter`), `enReduceHP` drains HP and `enCheckDeath` (`<= 0.0`)
    /// immediately transitions Pyramid Head into [`RedState::Leave`] (`state = 7, sub_state = 0`).
    pub fn receive_hit(&mut self, ap: f32, _sp: f32) {
        // Pyramid Head never flinches or falls down from gunfire or melee swings
        self.stats.health = (self.stats.health - ap).max(0.0);
        if self.is_boss_encounter && self.stats.health <= 0.0 && self.state != RedState::Leave {
            self.state = RedState::Leave;
            self.sub_state = 0;
            self.active_attack_id = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pyramid_head_grab_confuse_and_leave() {
        let mut red = PyramidHeadController::new(1, Vec3::new(98_850.0, -1700.0, -99_400.0), 0.0);
        let player =
            PerceivableTarget::new(Vec3::new(98_850.0, -1700.0, -100_500.0), 0.0, true, false);

        red.state = RedState::Grab;
        red.is_hugging_player = true;
        red.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        red.anim_finished = true;
        red.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        red.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert_eq!(red.active_attack_id, Some(RED_ATTACK_ID_CHOKE_PC));

        // Leave stair-guard counter-attack (sub 2/3) when player approaches z > -99400.
        red.state = RedState::Leave;
        red.sub_state = 1;
        let stair_player =
            PerceivableTarget::new(Vec3::new(98_850.0, -1700.0, -99_000.0), 0.0, true, false);
        red.update(
            &stair_player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert!(matches!(red.sub_state, 2 | 3));

        red.state = RedState::Confuse;
        red.sub_state = 0;
        red.confuse_friendly_target = Some(red.position + Vec3::new(400.0, 0.0, 0.0));
        red.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert_eq!(red.sub_state, 4);

        red.state = RedState::Leave;
        red.sub_state = 9;
        red.position = RED_AP_BATTLE_END_POINTS[5];
        red.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert!(red.battle_end_complete);
    }
}
