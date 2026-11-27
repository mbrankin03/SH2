//! Maria companion following logic and companion safety states — 1:1 `SLUS_202.28` `Chacter/m3_maria_sub.c`.
//!
//! # Ground Truth (`SLUS_202.28` `Chacter/m3_maria_sub.c`):
//! - 7 main states (`MariaUpdateStatusExecPhase1` `0x00226e40` / `func_list_main`):
//!   `0 = Stand` (`mar_main_stand` `0x00224b40`), `1 = CloseTo` (`mar_main_close_to` `0x00224bd0`),
//!   `2 = Alert` (`mar_main_alert` `0x00224cf0`), `3 = Discover` (`mar_main_discover` `0x00224d60`),
//!   `4 = Recover` (`mar_main_recover` `0x00224d70`), `5 = Boredom` (`mar_main_boredom` `0x00224e00`),
//!   `6 = Damaged` (`mar_main_damaged` `0x00224e90`).
//! - 9 sub-states (`mar_sub_*` `0x00224ef0..0x00225690`):
//!   `0 = Stand`, `1 = Relax`, `2 = RelaxOff`, `3 = Afraid`, `4 = Tired`,
//!   `5 = OneStep`, `6 = Walk`, `7 = Run`, `8 = Damage`.
//! - Exact follow distance thresholds (`mar_main_close_to` `0x00224bd0` & `MariaUpdateStatusExecPhase1` `0x00226e40`):
//!   - `> 1000.0` mm (or LOS blocked): `CloseTo` -> `Walk`
//!   - `>= 2000.0` mm: `CloseTo` -> `Run`
//!   - `< 1000.0` mm (with LOS): `Stand`
//! - Exact speeds (`mar_sub_walk` `0x002253b0`, `mar_sub_run` `0x00225540`, `MariaSpeedDownToStand` `0x00223e30`):
//!   - Walk speed factor `1.4` (`+2.0*dt` accel / `-4.0*dt` decel), Run speed factor `3.5` (`+3.0*dt` accel),
//!     deceleration to stand `-10.0*dt` from `Run` else `-8.0*dt`.
//! - Exact timers: `6.0s` idle -> `Boredom`/`Alert`, `10.0s` relax cycle (`mar_sub_relax` `0x00224f70`),
//!   `> 399` fatigue ticks -> `Recover`/`Tired` (`MariaUpdateStatusExecPhase1` `0x00226e40:1913`),
//!   `2.0s` invulnerability after damage (`mar_muteki_set` `0x002249c0`), `+1.0 HP/s` passive recovery
//!   (`MariaCheckDamage` `0x002270d0:2051`), `0.1x` friendly fire (`MariaCheckDamage` `0x002270d0:2026`).
//! - Health (`shCharacterMariaWorkInitAtGameStart` `SLUS_202.28` `0x002205b0`):
//!   `160.0` HP (Beginner / Easy), `80.0` HP (Normal / Hard).

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::sensory::{normalize_angle, DifficultyLevel};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Exact `mar_main_close_to` (`0x00224bd0:685`) / `MariaUpdateStatusExecPhase1` (`0x00226e40:1909`) walk/stop threshold (mm).
pub const MARIA_WALK_DISTANCE_MM: f32 = 1000.0;

/// Exact `mar_main_close_to` (`0x00224bd0:683`) run threshold (mm).
pub const MARIA_RUN_DISTANCE_MM: f32 = 2000.0;

/// Exact `MariaUpdateStatusExecPhase1` (`0x00226e40:1917,1926`) idle threshold for `Alert` / `Boredom` (seconds).
pub const MARIA_IDLE_THRESHOLD_SECS: f32 = 6.0;

/// Exact `mar_sub_relax` (`0x00224f70:853`) relax cycle duration (seconds).
pub const MARIA_RELAX_CYCLE_SECS: f32 = 10.0;

/// Exact `MariaUpdateStatusExecPhase1` (`0x00226e40:1913`) fatigue tick threshold (`399 < fatigue` -> `Recover`).
pub const MARIA_FATIGUE_TRIGGER_TICKS: i32 = 400;

/// Exact `mar_muteki_set` (`0x002249c0:600`) post-damage invulnerability timer (`0x40000000 = 2.0` seconds).
pub const MARIA_INVULN_SECS: f32 = 2.0;

/// Exact `mar_sub_walk` (`0x002253b0:971`) max walk speed parameter (`1.4` -> `1400.0` mm/s).
pub const MARIA_WALK_SPEED_PARAM: f32 = 1.4;

/// Exact `mar_sub_run` (`0x00225540:1019`) max run speed parameter (`3.5` -> `3500.0` mm/s).
pub const MARIA_RUN_SPEED_PARAM: f32 = 3.5;

/// Authentic Maria starting/max HP (`shCharacterMariaWorkInitAtGameStart`, `SLUS_202.28` `0x002205b0`).
pub const fn maria_max_hp_for_difficulty(difficulty: DifficultyLevel) -> f32 {
    match difficulty {
        DifficultyLevel::Beginner | DifficultyLevel::Easy => 160.0,
        DifficultyLevel::Normal | DifficultyLevel::Hard => 80.0,
    }
}

/// Authentic 7 main states (`MariaUpdateStatusExecPhase1` `SLUS_202.28` `0x00226e40` / `func_list_main`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MariaMainState {
    #[default]
    Stand = 0,
    CloseTo = 1,
    Alert = 2,
    Discover = 3,
    Recover = 4,
    Boredom = 5,
    Damaged = 6,
}

/// Authentic 9 sub-states (`mar_sub_*` `SLUS_202.28` `0x00224ef0..0x00225690`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MariaSubState {
    #[default]
    Stand = 0,
    Relax = 1,
    RelaxOff = 2,
    Afraid = 3,
    Tired = 4,
    OneStep = 5,
    Walk = 6,
    Run = 7,
    Damage = 8,
}

/// High-level companion animation/behavior state (mapped directly from [`MariaSubState`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MariaState {
    #[default]
    Stand,
    Walk,
    Run,
    Panic,
    Downed,
    Dead,
}

/// History breadcrumb point recorded from James' trajectory (`MariaSetStraightRoot` `0x00225bb0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreadcrumbPoint {
    pub position: Vec3,
    pub yaw: f32,
}

/// Maria companion controller (`Chacter/m3_maria_sub.c`).
#[derive(Debug, Clone)]
pub struct MariaCompanionController {
    /// Whether Maria is actively following James in the current scenario/room (flag 15 / `0x8000`).
    pub is_present: bool,
    pub position: Vec3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    /// High-level animation state derived from [`Self::sub_state`].
    pub state: MariaState,
    /// Authentic 7-state main FSM (`DAT_011b6be8`, `0x00226e40`).
    pub main_state: MariaMainState,
    /// Authentic 9-state sub FSM (`DAT_011b6bea`, `0x00224ef0..0x00225690`).
    pub sub_state: MariaSubState,
    /// Relax animation phase (`DAT_011b6e34`: `0`, `1`, `2`, `mar_sub_relax` `0x00224f70`).
    pub relax_phase: u8,
    /// Current locomotion speed parameter (`*(f32*)(work + 0xac)`: `0.0..=3.5`).
    pub speed_param: f32,
    /// Idle timer (`_DAT_011b6e10`, seconds; `>= 6.0` triggers `Alert` or `Boredom`).
    pub idle_timer: f32,
    /// Move timer (`_DAT_011b6e14`, seconds).
    pub move_timer: f32,
    /// Relax timer (`_DAT_011b6e1c`, seconds; `>= 10.0` triggers relax phase 2).
    pub relax_timer: f32,
    /// Post-damage invulnerability timer (`_DAT_011b6e18`, `2.0s` in `mar_muteki_set` `0x002249c0`).
    pub invuln_timer: f32,
    /// Fatigue tick counter (`_DAT_011b6e2c`, `> 399` triggers `Recover`/`Tired`).
    pub fatigue_ticks: i32,
    /// Maximum fatigue capacity (`_DAT_011b6e30`, default `600`).
    pub max_fatigue_ticks: i32,
    /// Whether Maria has direct line-of-sight to James (`DAT_011b6e3c`).
    pub has_los_to_james: bool,
    pub trail: Vec<BreadcrumbPoint>,
    pub distance_to_james: f32,
    /// Stop/walk threshold (`1000.0` mm, `mar_main_close_to` `0x00224bd0:685`).
    pub stop_distance: f32,
    /// Walk threshold (`1000.0` mm, `mar_main_close_to` `0x00224bd0:685`).
    pub walk_distance: f32,
    /// Run threshold (`2000.0` mm, `mar_main_close_to` `0x00224bd0:683`).
    pub run_distance: f32,
    /// Panic/Alert enemy proximity threshold (`1500.0` mm).
    pub panic_distance: f32,
    pub panic_timer: f32,
    pub anim: CreatureAnimationController,
}

impl Default for MariaCompanionController {
    fn default() -> Self {
        Self::new(DifficultyLevel::Normal)
    }
}

impl MariaCompanionController {
    /// Create Maria companion controller initialized for `difficulty`
    /// (`shCharacterMariaWorkInitAtGameStart` `0x002205b0`: `160.0` Beginner/Easy, `80.0` Normal/Hard).
    pub fn new(difficulty: DifficultyLevel) -> Self {
        let hp = maria_max_hp_for_difficulty(difficulty);
        Self {
            is_present: false,
            position: Vec3::ZERO,
            yaw: 0.0,
            health: hp,
            max_health: hp,
            state: MariaState::Stand,
            main_state: MariaMainState::Stand,
            sub_state: MariaSubState::Stand,
            relax_phase: 0,
            speed_param: 0.0,
            idle_timer: 0.0,
            move_timer: 0.0,
            relax_timer: 0.0,
            invuln_timer: 0.0,
            fatigue_ticks: 0,
            max_fatigue_ticks: 600,
            has_los_to_james: true,
            trail: Vec::with_capacity(64),
            distance_to_james: 0.0,
            stop_distance: MARIA_WALK_DISTANCE_MM,
            walk_distance: MARIA_WALK_DISTANCE_MM,
            run_distance: MARIA_RUN_DISTANCE_MM,
            panic_distance: 1500.0,
            panic_timer: 0.0,
            anim: CreatureAnimationController::new_maria(),
        }
    }

    pub fn spawn(&mut self, spawn_pos: Vec3, spawn_yaw: f32) {
        self.is_present = true;
        self.position = spawn_pos;
        self.yaw = spawn_yaw;
        self.state = MariaState::Stand;
        self.main_state = MariaMainState::Stand;
        self.sub_state = MariaSubState::Stand;
        self.speed_param = 0.0;
        self.trail.clear();
    }

    pub fn reset(&mut self) {
        self.is_present = false;
        self.health = self.max_health;
        self.state = MariaState::Stand;
        self.main_state = MariaMainState::Stand;
        self.sub_state = MariaSubState::Stand;
        self.relax_phase = 0;
        self.speed_param = 0.0;
        self.idle_timer = 0.0;
        self.move_timer = 0.0;
        self.relax_timer = 0.0;
        self.invuln_timer = 0.0;
        self.fatigue_ticks = 0;
        self.trail.clear();
        self.distance_to_james = 0.0;
        self.panic_timer = 0.0;
    }

    pub fn record_breadcrumb(&mut self, player_pos: Vec3, player_yaw: f32) {
        if let Some(last) = self.trail.last() {
            let dx = player_pos.x - last.position.x;
            let dz = player_pos.z - last.position.z;
            if dx * dx + dz * dz < 200.0 * 200.0 {
                return;
            }
        }
        if self.trail.len() >= 64 {
            self.trail.remove(0);
        }
        self.trail.push(BreadcrumbPoint {
            position: player_pos,
            yaw: player_yaw,
        });
    }

    /// Smooth yaw convergence (`MariaBodyAngleCloseToTarget` `0x00223cb0`: `dt * 15.0 * (diff / 3.0915928)`).
    fn turn_towards_maria(&mut self, target_yaw: f32, dt: f32) {
        let diff = normalize_angle(self.yaw - target_yaw);
        let step = dt * 15.0 * (diff / 3.091_592_8);
        if (diff >= 0.0 && diff - step <= 0.0) || (diff < 0.0 && diff - step >= 0.0) {
            self.yaw = target_yaw;
        } else {
            self.yaw = normalize_angle(self.yaw - step);
        }
    }

    /// Decelerates `speed_param` to `0.0` (`MariaSpeedDownToStand` `0x00223e30`).
    fn speed_down_to_stand(&mut self, dt: f32) {
        let rate = if self.sub_state == MariaSubState::Run {
            10.0
        } else {
            8.0
        };
        self.speed_param = (self.speed_param - dt * rate).max(0.0);
    }

    /// Maps `(main_state, sub_state)` to [`MariaState`] for skeletal animation sync.
    fn sync_high_level_state(&mut self) {
        if self.health <= 0.0 {
            self.state = MariaState::Dead;
            return;
        }
        self.state = match self.sub_state {
            MariaSubState::Damage => MariaState::Downed,
            MariaSubState::Afraid => MariaState::Panic,
            MariaSubState::Run => MariaState::Run,
            MariaSubState::Walk => MariaState::Walk,
            MariaSubState::Stand
            | MariaSubState::Relax
            | MariaSubState::RelaxOff
            | MariaSubState::Tired
            | MariaSubState::OneStep => MariaState::Stand,
        };
    }

    /// Update Maria's 7-main / 9-sub FSM (`MariaUpdateStatusInitial` `0x00226cd0` +
    /// `MariaUpdateStatusExecPhase1` `0x00226e40` + `mar_main_*` / `mar_sub_*`).
    pub fn update(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        hostile_enemy_positions: &[Vec3],
        dt: f32,
    ) {
        if !self.is_present || self.state == MariaState::Dead {
            return;
        }

        let dt_ticks = (dt * 60.0).round().max(1.0) as i32;
        if self.invuln_timer > 0.0 {
            self.invuln_timer = (self.invuln_timer - dt).max(0.0);
        }
        // Passive health regeneration (`MariaCheckDamage` `0x002270d0:2051`: `hp += _dtf`)
        self.health = (self.health + dt).min(self.max_health);

        let dx = player_pos.x - self.position.x;
        let dz = player_pos.z - self.position.z;
        self.distance_to_james = (dx * dx + dz * dz).sqrt();

        let mut nearest_enemy_dist_sq = f32::MAX;
        for &ep in hostile_enemy_positions {
            let edx = ep.x - self.position.x;
            let edz = ep.z - self.position.z;
            let d2 = edx * edx + edz * edz;
            if d2 < nearest_enemy_dist_sq {
                nearest_enemy_dist_sq = d2;
            }
        }
        let enemy_nearby = nearest_enemy_dist_sq.sqrt() < self.panic_distance;

        if enemy_nearby {
            self.panic_timer = 2.0;
        } else if self.panic_timer > 0.0 {
            self.panic_timer = (self.panic_timer - dt).max(0.0);
        }

        // Phase 1: Select Main State (`MariaUpdateStatusExecPhase1` `0x00226e40`)
        if self.main_state == MariaMainState::Damaged {
            if self.invuln_timer <= 0.0 {
                self.main_state = MariaMainState::Stand;
                self.sub_state = MariaSubState::Stand;
            }
        } else if enemy_nearby || self.panic_timer > 0.0 {
            self.main_state = MariaMainState::Alert;
        } else if self.distance_to_james > self.walk_distance || !self.has_los_to_james {
            self.main_state = MariaMainState::CloseTo;
        } else if self.fatigue_ticks >= MARIA_FATIGUE_TRIGGER_TICKS {
            self.main_state = MariaMainState::Recover;
        } else if self.idle_timer >= MARIA_IDLE_THRESHOLD_SECS {
            self.main_state = MariaMainState::Boredom;
        } else {
            self.main_state = MariaMainState::Stand;
        }

        // Phase 2: Resolve Sub State (`mar_main_*` `0x00224b40..0x00224e90`)
        match self.main_state {
            MariaMainState::Stand => {
                if self.sub_state == MariaSubState::Relax {
                    self.sub_state = MariaSubState::RelaxOff;
                } else {
                    self.sub_state = MariaSubState::Stand;
                }
            }
            MariaMainState::CloseTo => {
                // `mar_main_close_to` (`0x00224bd0`): >= 2000mm -> Run (7), >= 1000mm or !LOS -> Walk (6), else Stand (0)
                if self.sub_state == MariaSubState::Relax {
                    self.sub_state = MariaSubState::RelaxOff;
                } else if self.distance_to_james >= self.run_distance {
                    self.sub_state = MariaSubState::Run;
                } else if self.distance_to_james >= self.walk_distance || !self.has_los_to_james {
                    self.sub_state = MariaSubState::Walk;
                } else {
                    self.sub_state = MariaSubState::Stand;
                }
            }
            MariaMainState::Alert => {
                self.sub_state = MariaSubState::Afraid;
            }
            MariaMainState::Discover => {
                self.sub_state = MariaSubState::Stand;
            }
            MariaMainState::Recover => {
                self.sub_state = MariaSubState::Tired;
            }
            MariaMainState::Boredom => {
                self.sub_state = MariaSubState::Relax;
            }
            MariaMainState::Damaged => {
                self.sub_state = MariaSubState::Damage;
            }
        }

        // Sub-state execution (`mar_sub_*` `0x00224ef0..0x00225690`)
        self.record_breadcrumb(player_pos, player_yaw);
        let target_goal = if let Some(&first) = self.trail.first() {
            let tdx = first.position.x - self.position.x;
            let tdz = first.position.z - self.position.z;
            if tdx * tdx + tdz * tdz < 300.0 * 300.0 {
                self.trail.remove(0);
            }
            first.position
        } else {
            player_pos
        };

        match self.sub_state {
            MariaSubState::Stand | MariaSubState::RelaxOff | MariaSubState::OneStep => {
                self.idle_timer += dt;
                self.move_timer = 0.0;
                self.relax_timer = 0.0;
                self.fatigue_ticks = (self.fatigue_ticks - dt_ticks).max(0);
                self.speed_down_to_stand(dt);
                self.turn_towards_maria(dx.atan2(dz), dt);
            }
            MariaSubState::Relax => {
                self.idle_timer += dt;
                self.relax_timer += dt;
                self.fatigue_ticks = (self.fatigue_ticks - dt_ticks).max(0);
                self.speed_down_to_stand(dt);
                if self.relax_phase == 1 && self.relax_timer >= MARIA_RELAX_CYCLE_SECS {
                    self.relax_phase = 2;
                    self.relax_timer = 0.0;
                } else if self.relax_phase == 0 {
                    self.relax_phase = 1;
                }
            }
            MariaSubState::Afraid => {
                self.idle_timer += dt;
                self.fatigue_ticks = (self.fatigue_ticks - dt_ticks).max(0);
                self.speed_down_to_stand(dt);
                self.turn_towards_maria(dx.atan2(dz), dt);
            }
            MariaSubState::Tired => {
                self.idle_timer += dt;
                self.fatigue_ticks = (self.fatigue_ticks - dt_ticks).max(0);
                self.speed_down_to_stand(dt);
                if self.fatigue_ticks < self.max_fatigue_ticks / 2 {
                    self.main_state = MariaMainState::Stand;
                    self.sub_state = MariaSubState::Stand;
                }
            }
            MariaSubState::Walk => {
                self.idle_timer = 0.0;
                self.move_timer += dt;
                if self.speed_param > MARIA_WALK_SPEED_PARAM {
                    self.speed_param = (self.speed_param - dt * 4.0).max(MARIA_WALK_SPEED_PARAM);
                } else {
                    self.speed_param = (self.speed_param + dt * 2.0).min(MARIA_WALK_SPEED_PARAM);
                }
                let gdx = target_goal.x - self.position.x;
                let gdz = target_goal.z - self.position.z;
                let gdist = (gdx * gdx + gdz * gdz).sqrt();
                if gdist > 1.0 {
                    self.turn_towards_maria(gdx.atan2(gdz), dt);
                    let step = (MARIA_WALK_SPEED_PARAM * 1000.0) * dt;
                    if step >= gdist {
                        self.position = target_goal;
                    } else {
                        self.position.x += (gdx / gdist) * step;
                        self.position.z += (gdz / gdist) * step;
                    }
                }
            }
            MariaSubState::Run => {
                self.idle_timer = 0.0;
                self.move_timer += dt;
                self.fatigue_ticks = (self.fatigue_ticks + dt_ticks).min(self.max_fatigue_ticks);
                self.speed_param = (self.speed_param + dt * 3.0).min(MARIA_RUN_SPEED_PARAM);
                let gdx = target_goal.x - self.position.x;
                let gdz = target_goal.z - self.position.z;
                let gdist = (gdx * gdx + gdz * gdz).sqrt();
                if gdist > 1.0 {
                    self.turn_towards_maria(gdx.atan2(gdz), dt);
                    let step = (MARIA_RUN_SPEED_PARAM * 1000.0) * dt;
                    if step >= gdist {
                        self.position = target_goal;
                    } else {
                        self.position.x += (gdx / gdist) * step;
                        self.position.z += (gdz / gdist) * step;
                    }
                }
            }
            MariaSubState::Damage => {
                self.speed_down_to_stand(dt);
            }
        }

        self.sync_high_level_state();
        self.anim.sync_maria(self.state);
        self.anim.update(dt, self.yaw);
    }

    /// Apply damage to Maria (`MariaCheckDamage` `SLUS_202.28` `0x002270d0`: `0.1x` friendly fire,
    /// `mar_muteki_set` `2.0s` invulnerability).
    pub fn receive_damage(&mut self, damage: f32, is_player_friendly_fire: bool) {
        if !self.is_present || self.state == MariaState::Dead || self.invuln_timer > 0.0 {
            return;
        }
        let effective_dmg = if is_player_friendly_fire {
            damage * 0.1
        } else {
            damage
        };
        self.health = (self.health - effective_dmg).max(0.0);
        if self.health <= 0.0 {
            self.state = MariaState::Dead;
            self.main_state = MariaMainState::Damaged;
            self.sub_state = MariaSubState::Damage;
        } else {
            self.main_state = MariaMainState::Damaged;
            self.sub_state = MariaSubState::Damage;
            self.invuln_timer = MARIA_INVULN_SECS;
            self.state = MariaState::Downed;
        }
        self.anim.sync_maria(self.state);
    }

    /// Target arbitration check (`0x542ad0`).
    pub fn should_enemy_target_maria(&self, enemy_pos: Vec3, player_pos: Vec3) -> bool {
        if !self.is_present || self.state == MariaState::Dead || self.state == MariaState::Downed {
            return false;
        }
        let m_dx = self.position.x - enemy_pos.x;
        let m_dz = self.position.z - enemy_pos.z;
        let m_dist_sq = m_dx * m_dx + m_dz * m_dz;
        let p_dx = player_pos.x - enemy_pos.x;
        let p_dz = player_pos.z - enemy_pos.z;
        let p_dist_sq = p_dx * p_dx + p_dz * p_dz;
        m_dist_sq < p_dist_sq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maria_following_hysteresis_matches_mar_main_close_to() {
        let mut maria = MariaCompanionController::default();
        maria.spawn(Vec3::new(0.0, 0.0, 0.0), 0.0);

        // < 1000mm: Stand
        maria.update(Vec3::new(0.0, 0.0, 800.0), 0.0, &[], 0.016);
        assert_eq!(maria.main_state, MariaMainState::Stand);
        assert_eq!(maria.sub_state, MariaSubState::Stand);
        assert_eq!(maria.state, MariaState::Stand);

        // 1500mm (> 1000mm walk threshold, < 2000mm run threshold): CloseTo -> Walk
        maria.update(Vec3::new(0.0, 0.0, 1500.0), 0.0, &[], 0.016);
        assert_eq!(maria.main_state, MariaMainState::CloseTo);
        assert_eq!(maria.sub_state, MariaSubState::Walk);
        assert_eq!(maria.state, MariaState::Walk);

        // 2200mm (>= 2000mm run threshold): CloseTo -> Run
        maria.update(Vec3::new(0.0, 0.0, 2200.0), 0.0, &[], 0.016);
        assert_eq!(maria.main_state, MariaMainState::CloseTo);
        assert_eq!(maria.sub_state, MariaSubState::Run);
        assert_eq!(maria.state, MariaState::Run);
    }

    #[test]
    fn test_maria_panic_boredom_fatigue_and_damage() {
        let mut maria = MariaCompanionController::default();
        maria.spawn(Vec3::new(0.0, 0.0, 0.0), 0.0);

        // Enemy within 1500mm -> Alert / Afraid (Panic)
        let enemies = [Vec3::new(500.0, 0.0, 500.0)];
        maria.update(Vec3::new(0.0, 0.0, 800.0), 0.0, &enemies, 0.016);
        assert_eq!(maria.main_state, MariaMainState::Alert);
        assert_eq!(maria.sub_state, MariaSubState::Afraid);
        assert_eq!(maria.state, MariaState::Panic);

        // Fatigue >= 400 -> Recover / Tired
        maria.panic_timer = 0.0;
        maria.fatigue_ticks = 450;
        maria.update(Vec3::new(0.0, 0.0, 800.0), 0.0, &[], 0.016);
        assert_eq!(maria.main_state, MariaMainState::Recover);
        assert_eq!(maria.sub_state, MariaSubState::Tired);

        // Friendly fire 0.1x (`MariaCheckDamage` 0x002270d0)
        maria.health = 80.0;
        maria.receive_damage(100.0, true);
        assert_eq!(maria.health, 70.0);
        assert_eq!(maria.main_state, MariaMainState::Damaged);
        assert_eq!(maria.sub_state, MariaSubState::Damage);

        // Hit while `invuln_timer > 0.0` (`mar_muteki_set`) is ignored
        maria.receive_damage(100.0, false);
        assert_eq!(maria.health, 70.0);

        // Once invulnerability expires, fatal enemy damage kills Maria
        maria.invuln_timer = 0.0;
        maria.receive_damage(100.0, false);
        assert_eq!(maria.health, 0.0);
        assert_eq!(maria.state, MariaState::Dead);
    }

    #[test]
    fn test_enemy_target_arbitration() {
        let mut maria = MariaCompanionController::default();
        maria.spawn(Vec3::new(0.0, 0.0, 1000.0), 0.0);
        assert!(maria.should_enemy_target_maria(Vec3::new(0.0, 0.0, 800.0), Vec3::ZERO));
        maria.state = MariaState::Dead;
        assert!(!maria.should_enemy_target_maria(Vec3::new(0.0, 0.0, 800.0), Vec3::ZERO));
    }
}
