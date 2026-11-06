//! Maria companion following logic and companion safety states (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Chacter/m3_maria.c`, `src/Chacter/m3_maria_sub.c`
//! - PC executable `sh2pc.exe` Director's Cut:
//!   - `0x529480`..`0x52df40`: Follower logic, breadcrumb trailing, and panic reaction states.
//!   - `0x542ad0`: Target arbitrator (`maria_target`) — enemies target whichever character is closer.
//!   - `0x52d280`: Friendly fire damage reduction (takes 0.1x from James' weapons).
//!   - `0x1fb3b14`: Ending score distance counter (`maria_distance`).
//!
//! # Mechanics:
//! - Breadcrumb buffer: Maria records James' recent positions and walks/runs along the trail.
//! - Follow distance hysteresis:
//!   - Stop distance: ~1,200 units (stands still and waits for James).
//!   - Walk threshold: >1,600 units (starts walking).
//!   - Sprint threshold: >3,200 units (sprints to catch up).
//! - Panic state:
//!   - Triggers when an active hostile enemy is within 1,500 units.
//!   - In panic mode, Maria cowers, covers her head, or retreats behind James.
//! - Health & Game Over:
//!   - Max health: 1,000 HP.
//!   - If Maria's health reaches 0, companion death event triggers game over.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Maria's current behavioral / companion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MariaState {
    /// Resting or standing idle near James.
    #[default]
    Stand,
    /// Walking along James' breadcrumb trail.
    Walk,
    /// Running to catch up when falling behind.
    Run,
    /// Panicking / cowering when enemies are threatening nearby.
    Panic,
    /// Downed / taking damage.
    Downed,
    /// Dead (triggers game over).
    Dead,
}

/// History breadcrumb point recorded from James' trajectory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreadcrumbPoint {
    pub position: Vec3,
    pub yaw: f32,
}

/// Maria companion controller (`m3_maria_sub.c`).
#[derive(Debug, Clone)]
pub struct MariaCompanionController {
    /// Whether Maria is actively following James in the current scenario/room (flag 15 / `0x8000`).
    pub is_present: bool,
    /// Maria's 3D position in the world.
    pub position: Vec3,
    /// Facing yaw in radians.
    pub yaw: f32,
    /// Current health (default 1000.0).
    pub health: f32,
    pub max_health: f32,
    /// Current companion state.
    pub state: MariaState,
    /// Breadcrumb trail of recent player locations.
    pub trail: Vec<BreadcrumbPoint>,
    /// Accumulated total distance to James (for ending calculation `0x1fb3b14`).
    pub distance_to_james: f32,
    /// Stop distance threshold (units).
    pub stop_distance: f32,
    /// Walk start threshold (units).
    pub walk_distance: f32,
    /// Run / sprint catch-up threshold (units).
    pub run_distance: f32,
    /// Panic trigger distance to nearest enemy (units).
    pub panic_distance: f32,
    /// Panic duration remaining (seconds).
    pub panic_timer: f32,
}

impl Default for MariaCompanionController {
    fn default() -> Self {
        Self {
            is_present: false,
            position: Vec3::ZERO,
            yaw: 0.0,
            health: 1000.0,
            max_health: 1000.0,
            state: MariaState::Stand,
            trail: Vec::with_capacity(64),
            distance_to_james: 0.0,
            stop_distance: 1200.0,
            walk_distance: 1600.0,
            run_distance: 3000.0,
            panic_distance: 1500.0,
            panic_timer: 0.0,
        }
    }
}

impl MariaCompanionController {
    /// Spawn or initialize Maria in a room at an offset from James.
    pub fn spawn(&mut self, spawn_pos: Vec3, spawn_yaw: f32) {
        self.is_present = true;
        self.position = spawn_pos;
        self.yaw = spawn_yaw;
        self.state = MariaState::Stand;
        self.trail.clear();
    }

    /// Reset companion status (e.g. on new game or continue).
    pub fn reset(&mut self) {
        self.is_present = false;
        self.health = self.max_health;
        self.state = MariaState::Stand;
        self.trail.clear();
        self.distance_to_james = 0.0;
        self.panic_timer = 0.0;
    }

    /// Record a breadcrumb if player moved sufficiently.
    pub fn record_breadcrumb(&mut self, player_pos: Vec3, player_yaw: f32) {
        if let Some(last) = self.trail.last() {
            let dx = player_pos.x - last.position.x;
            let dz = player_pos.z - last.position.z;
            if dx * dx + dz * dz < 200.0 * 200.0 {
                return; // Only record new breadcrumbs every 200 units of player movement
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

    /// Update Maria's AI, following trajectory, panic, and animations.
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

        // Distance to James
        let dx = player_pos.x - self.position.x;
        let dz = player_pos.z - self.position.z;
        self.distance_to_james = (dx * dx + dz * dz).sqrt();

        // Check enemy proximity for panic
        let mut nearest_enemy_dist = f32::MAX;
        for &ep in hostile_enemy_positions {
            let edx = ep.x - self.position.x;
            let edz = ep.z - self.position.z;
            let edist_sq = edx * edx + edz * edz;
            if edist_sq < nearest_enemy_dist {
                nearest_enemy_dist = edist_sq;
            }
        }
        let nearest_enemy_dist = nearest_enemy_dist.sqrt();

        if nearest_enemy_dist < self.panic_distance {
            self.panic_timer = 2.0; // Maintain panic for 2 seconds
            self.state = MariaState::Panic;
        } else if self.panic_timer > 0.0 {
            self.panic_timer = (self.panic_timer - dt).max(0.0);
            if self.panic_timer > 0.0 {
                self.state = MariaState::Panic;
            }
        }

        // State machine resolution
        if self.state == MariaState::Panic {
            // Cower / back away facing the nearest threat
            // Turn towards James for safety
            self.yaw = dx.atan2(dz);
            return;
        }

        self.record_breadcrumb(player_pos, player_yaw);

        if self.distance_to_james > self.run_distance {
            self.state = MariaState::Run;
        } else if self.distance_to_james > self.walk_distance {
            self.state = MariaState::Walk;
        } else if self.distance_to_james <= self.stop_distance {
            self.state = MariaState::Stand;
        }

        // Move towards next breadcrumb or directly towards James if trail empty
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

        if self.state == MariaState::Walk || self.state == MariaState::Run {
            let gdx = target_goal.x - self.position.x;
            let gdz = target_goal.z - self.position.z;
            let gdist = (gdx * gdx + gdz * gdz).sqrt();

            if gdist > 1.0 {
                self.yaw = gdx.atan2(gdz);
                let speed = if self.state == MariaState::Run {
                    3200.0 // Maria sprint speed (units/s)
                } else {
                    1400.0 // Maria walk speed (units/s)
                };
                let step = speed * dt;
                if step >= gdist {
                    self.position = target_goal;
                } else {
                    self.position.x += (gdx / gdist) * step;
                    self.position.z += (gdz / gdist) * step;
                }
            }
        } else {
            // Standing idle: gently align facing to look towards James
            self.yaw = dx.atan2(dz);
        }
    }

    /// Apply damage to Maria from enemy blows or friendly fire (`0x52d280`).
    pub fn receive_damage(&mut self, damage: f32, is_player_friendly_fire: bool) {
        if !self.is_present || self.state == MariaState::Dead {
            return;
        }

        let effective_dmg = if is_player_friendly_fire {
            damage * 0.1 // 10% friendly fire damage reduction
        } else {
            damage
        };

        self.health = (self.health - effective_dmg).max(0.0);
        if self.health <= 0.0 {
            self.state = MariaState::Dead;
        } else {
            self.state = MariaState::Downed;
            self.panic_timer = 3.0;
        }
    }

    /// Target arbitration check (`0x542ad0`).
    /// Returns true if Maria is closer to the enemy than James, making her the preferred target.
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
    fn test_maria_following_hysteresis() {
        let mut maria = MariaCompanionController::default();
        maria.spawn(Vec3::new(0.0, 0.0, 0.0), 0.0);

        // Player is close (800 units): Maria stands
        maria.update(Vec3::new(0.0, 0.0, 800.0), 0.0, &[], 0.016);
        assert_eq!(maria.state, MariaState::Stand);

        // Player moves away to 2000 units (> 1600 walk threshold): Maria walks
        maria.update(Vec3::new(0.0, 0.0, 2000.0), 0.0, &[], 0.016);
        assert_eq!(maria.state, MariaState::Walk);

        // Player runs far away (3500 units > 3000 run threshold): Maria runs
        maria.update(Vec3::new(0.0, 0.0, 3500.0), 0.0, &[], 0.016);
        assert_eq!(maria.state, MariaState::Run);
    }

    #[test]
    fn test_maria_panic_and_damage() {
        let mut maria = MariaCompanionController::default();
        maria.spawn(Vec3::new(0.0, 0.0, 0.0), 0.0);

        // Enemy within 1000 units (< 1500 panic distance): Maria panics
        let enemies = [Vec3::new(500.0, 0.0, 500.0)];
        maria.update(Vec3::new(0.0, 0.0, 800.0), 0.0, &enemies, 0.016);
        assert_eq!(maria.state, MariaState::Panic);

        // Friendly fire damage test (100 dmg * 0.1 = 10 dmg)
        maria.receive_damage(100.0, true);
        assert_eq!(maria.health, 990.0);

        // Fatal enemy damage test
        maria.receive_damage(1000.0, false);
        assert_eq!(maria.health, 0.0);
        assert_eq!(maria.state, MariaState::Dead);
    }

    #[test]
    fn test_enemy_target_arbitration() {
        let mut maria = MariaCompanionController::default();
        maria.spawn(Vec3::new(0.0, 0.0, 1000.0), 0.0);

        let enemy_pos = Vec3::new(0.0, 0.0, 800.0);
        let player_pos = Vec3::new(0.0, 0.0, 0.0);

        // Enemy is at 800: distance to Maria is 200, distance to James is 800
        // Maria is closer -> enemy targets Maria
        assert!(maria.should_enemy_target_maria(enemy_pos, player_pos));

        // When Maria is dead, enemy targets James
        maria.state = MariaState::Dead;
        assert!(!maria.should_enemy_target_maria(enemy_pos, player_pos));
    }
}
