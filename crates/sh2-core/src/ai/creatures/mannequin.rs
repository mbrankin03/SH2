//! Mannequin (`chr/mkn`, kind 0x201) state machine and AI behaviors (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_mkn.c` (`enMKNCtrlMannequin`, `enMKNCtrlWander`, `enMKNCtrlChase`, `enMKNCtrlAround`, `enMKNCtrlAttack`)
//! - PC executable `sh2pc.exe`:
//!   - `0x4c0d70`..`0x4c38dc`: Mannequin control logic.
//!   - `0x4c18f0`: `MKN_STAY` - motionless pose. Wakes when flashlight beam enters 2,250 units and 40° angle, or player approaches within 900 units (`0x4c0eb0`). Alerts nearby mannequins within 1,000 units.
//!   - `0x4c2420`: `MKN_WANDER` - wander targets 1,500 units away, stays within 5,000 units of spawn.
//!   - `0x4c2950`: `MKN_CHASE` - advances at player.
//!   - `0x4c2a90`: `MKN_CIRCLE` - when line of sight is broken, flanks to a point 1,000 units away, 120° off player's facing.
//!   - `0x4c2c70`: `MKN_ATTACK` - alternates arm swing (attack 44) and kick (attack 45).
//!
//! # Speeds:
//! - Walk root motion: 19.2 units per frame (`0x893070` anime 1, 30 fps = 576 units/s).
//! - Normal health: 490 HP (70% of Lying Figure).

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::creatures::{apply_stage_hp_multipliers, CreatureStats, EnemySpecies, StagePlace};
use crate::ai::pathfinding::{check_path_ray, EnemyPathState, PathEnemyType, RoomForbiddenAreas};
use crate::ai::sensory::{
    check_hit_eyes, evaluate_sensory_with_los, is_lit_by_flashlight, mkn_walk_speed_scale,
    normalize_angle, DifficultyLevel, PerceivableTarget, RoomLighting, SensoryNotice,
    SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sh2_assets::formats::cld::WallSegment;

/// Authentic `SLUS_202.28` `enMKNGetFeelRange` (`0x00157330`, `0x2aa560`) feel ranges (mm).
pub const MKN_FEEL_RANGE_MM: [f32; 4] = [400.0, 750.0, 1000.0, 1500.0];

/// Authentic `SLUS_202.28` `enMKNGetAttackProbability` (`0x001573a0`, `0x2aa580`).
pub const MKN_ATTACK_PROBABILITY: [f32; 4] = [0.02, 0.5, 0.8, 0.9];

/// Authentic `SLUS_202.28` `enMKNGetRepertAttackProbability` (`0x00157410`, `0x2aa5a0`).
pub const MKN_REPEAT_ATTACK_PROBABILITY: [f32; 4] = [0.0, 0.0, 0.2, 0.6];

/// Authentic `SLUS_202.28` `enMKNGetRotSpeed` (`0x00157500`, `0x2aa5e0`) turn scale × `0.05235988` rad/tick.
pub const MKN_ROT_SPEED_SCALE: [f32; 4] = [0.5, 0.8, 1.0, 1.2];

/// Base rotation quantum per 60 Hz tick (`0.05235988` rad = 3°/tick, `enMKNGetRotSpeed` `0x00157500`).
pub const MKN_ROT_BASE_RAD_PER_TICK: f32 = 0.052_359_88;

/// Leash return timer in 60 Hz ticks (`enSetTimer(param_1, 300)`, `enMKNCtrlWander` `0x00154a10:824`).
pub const MKN_WANDER_RETURN_TICKS: f32 = 300.0;

/// Authentic Mannequin state enum (`0x4c3790`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MknState {
    #[default]
    Stay = 0,
    Event = 1,
    Fall = 2,
    Fall2 = 3,
    Wander = 4,
    Precaution = 5,
    Chase = 6,
    Circle = 7,
    Attack = 8,
    Damage = 9,
    Confuse = 10,
    Down = 11,
    Dying = 12,
}

/// Mannequin controller and state machine.
#[derive(Debug, Clone)]
pub struct MannequinController {
    pub id: u32,
    pub position: Vec3,
    pub origin_pos: Vec3,
    pub yaw: f32,
    pub state: MknState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub alert_timer: f32,
    pub state_timer: f32,
    pub is_kick_next: bool,
    /// Set by `enEventDriven(2)` to wake a mannequin in [`MknState::Event`] (`enMKNCtrlMannequin2` `0x00153e40`).
    pub event_triggered: bool,
    /// Set by animation driver when current clip finishes (`*(i32*)(work + 0x14) == -1`).
    pub anim_finished: bool,
    /// Vertical velocity in mm/s (Y-down gravity `+9800.0` mm/s²) for [`MknState::Fall`] / [`MknState::Fall2`].
    pub vel_y: f32,
    /// Exact binary timer in 60 Hz ticks (`30` landing pause in `Fall`/`Fall2`, `600 - 90 * mode` in `Confuse`).
    pub timer_ticks: f32,
    /// Optional nearby enemy position for Hyper Spray friendly-fire in [`MknState::Confuse`] (`0x00155b20`).
    pub confuse_friendly_target: Option<Vec3>,
    /// Obstacle-avoidance pathfinding state (`enInitPath` `0x00144aa0` / `enSetPath` `0x00144ae0`).
    pub path_state: EnemyPathState,
    pub sensory: SensoryProfile,
    pub anim: CreatureAnimationController,
}

impl MannequinController {
    /// Create a Mannequin (`MKN`) with default `1.0x` HP/Endurance (`enMKNInitData`, `SLUS_202.28` `0x00153410`).
    ///
    /// - HP table (`0x2aa480`): `[400.0, 400.0, 700.0, 1500.0]`
    /// - Endurance table (`0x2aa4a0`): `[10.0, 190.0, 285.0, 380.0]`
    /// - Auto-recovery (`enMKNAutoRecovery` `0x00157220`, `0x2aa528`): `[0.0, 10.0, 30.0, 60.0]`
    ///
    /// Note: The `0.7x` multiplier applies ONLY to subtype `9` (`work_type == 9`, see [`Self::new_with_subtype`]).
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        Self::new_with_subtype(id, position, yaw, difficulty, 0, StagePlace::Default)
    }

    /// Create a Mannequin with explicit `enMKNInitData` (`0x00153410`) `work_type` subtype and stage place.
    ///
    /// Subtype multipliers (`0x00153410`): `1 => 1.2x`, `2 => 0.5x`, `3|4 => 0.3x`, `9 => 0.7x`, default `1.0x`.
    pub fn new_with_subtype(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
        subtype: u32,
        place: StagePlace,
    ) -> Self {
        const MKN_HP: [f32; 4] = [400.0, 400.0, 700.0, 1500.0];
        const MKN_END: [f32; 4] = [10.0, 190.0, 285.0, 380.0];
        const MKN_REC: [f32; 4] = [0.0, 10.0, 30.0, 60.0];
        let idx = difficulty.as_index();
        let mult = match subtype {
            1 => 1.2,
            2 => 0.5,
            3 | 4 => 0.3,
            9 => 0.7,
            _ => 1.0,
        };
        let (hp, end) = apply_stage_hp_multipliers(
            EnemySpecies::Mannequin,
            MKN_HP[idx] * mult,
            MKN_END[idx] * mult,
            place,
        );

        Self {
            id,
            position,
            origin_pos: position,
            yaw,
            state: MknState::Stay, // Default dormant/freeze in dark
            sub_state: 0,
            stats: CreatureStats::new(hp, end, MKN_REC[idx]),
            alert_timer: 0.0,
            state_timer: 0.0,
            is_kick_next: false,
            event_triggered: false,
            anim_finished: false,
            vel_y: 0.0,
            timer_ticks: 0.0,
            confuse_friendly_target: None,
            path_state: EnemyPathState::new(yaw),
            sensory: SensoryProfile::mannequin(),
            anim: CreatureAnimationController::new_mannequin(),
        }
    }

    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        self.update_with_world(target, lighting, difficulty, &[], None, None, 0.0, dt);
    }

    /// Update Mannequin AI with `.cld` wall LOS (`enCheckHitEyes` `0x00145540`), `enSetPath`
    /// (`0x00144ae0`), [`RoomForbiddenAreas`], and tribe alerts (`enCommunicateTribe` `0x00146e40`,
    /// `en_mkn.c:395`).
    #[allow(clippy::too_many_arguments)]
    pub fn update_with_world(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        walls: &[WallSegment],
        forbidden: Option<&RoomForbiddenAreas>,
        tribe_alert_pos: Option<Vec3>,
        path_sway_sample: f32,
        dt: f32,
    ) {
        if self.state == MknState::Dying {
            return;
        }

        self.stats.update(dt);
        self.state_timer += dt;

        if self.stats.health <= 0.0 {
            self.state = MknState::Dying;
            return;
        }

        if self.state == MknState::Down {
            if self.stats.down_timer <= 0.0 {
                self.state = MknState::Chase;
                self.state_timer = 0.0;
            }
            return;
        }

        if self.stats.flinch_timer > 0.0 {
            self.state = MknState::Damage;
            return;
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        // Flashlight beam detection for waking up from STAY (`0x4c0eb0`), gated by `.cld` wall LOS
        let los_clear =
            walls.is_empty() || check_hit_eyes(self.position, target.position, walls) < 0.0;
        let lit_by_player = los_clear
            && is_lit_by_flashlight(
                self.position,
                target.position,
                target.yaw,
                target.is_flashlight_on,
            );

        let diff_idx = difficulty.as_index();
        let feel_range = MKN_FEEL_RANGE_MM[diff_idx];
        let rot_speed_rad_s = MKN_ROT_SPEED_SCALE[diff_idx] * MKN_ROT_BASE_RAD_PER_TICK * 60.0;
        let (notice, alert) = evaluate_sensory_with_los(
            &self.sensory,
            self.position,
            self.yaw,
            self.alert_timer,
            target,
            lighting,
            difficulty,
            walls,
            dt,
        );
        self.alert_timer = alert;

        match self.state {
            MknState::Stay => {
                // Mannequin stands completely still like a prop until:
                // 1) Flashlight shines directly on it within 2,250 units (unoccluded)
                // 2) Player approaches within 900 units (unoccluded)
                // 3) Nearby Mannequin broadcasts tribe alert (`enCommunicateTribe(2)`, `en_mkn.c:395`)
                if (lit_by_player && dist < 2250.0)
                    || (los_clear && dist < 900.0)
                    || tribe_alert_pos.is_some()
                {
                    if los_clear && dist <= 1000.0 {
                        self.state = MknState::Attack;
                    } else {
                        self.state = MknState::Precaution;
                    }
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                }
            }

            MknState::Wander => {
                // `enMKNCtrlWander` (`SLUS_202.28` `0x00154a10` lines 780-855):
                // 5000mm leash with 300-tick return timer; direct Attack (`2`), Chase (`1`), or Precaution (`< feel_range`).
                let odx = self.position.x - self.origin_pos.x;
                let odz = self.position.z - self.origin_pos.z;
                if odx * odx + odz * odz > 5000.0 * 5000.0 {
                    self.turn_towards((-odx).atan2(-odz), rot_speed_rad_s * dt);
                    if self.state_timer >= MKN_WANDER_RETURN_TICKS / 60.0 {
                        self.origin_pos = self.position;
                        self.state_timer = 0.0;
                    }
                }

                let walk_speed = 576.0 * mkn_walk_speed_scale(difficulty, self.stats.hp_rate);
                self.position.x += self.yaw.sin() * walk_speed * dt;
                self.position.z += self.yaw.cos() * walk_speed * dt;

                if notice == SensoryNotice::InAttackRange {
                    self.state = MknState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                } else if notice == SensoryNotice::VisualContact {
                    self.state = MknState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                } else if dist < feel_range || tribe_alert_pos.is_some() {
                    self.state = MknState::Precaution;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                }
            }

            MknState::Precaution => {
                // `enMKNCtrlPrecaution` (`SLUS_202.28` `0x00155040` lines 877-899):
                // Turns at `enMKNGetRotSpeed`; Attack if `2`, Chase if `1 && dist > 350.0`, Wander if `dist > feel_range`.
                self.turn_towards(target_dir, rot_speed_rad_s * dt);
                if notice == SensoryNotice::InAttackRange {
                    self.state = MknState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                } else if notice == SensoryNotice::VisualContact && dist > 350.0 {
                    self.state = MknState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                } else if dist > feel_range
                    && notice == SensoryNotice::None
                    && self.state_timer >= 0.5
                {
                    self.state = MknState::Wander;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            MknState::Chase => {
                // `enMKNCtrlChase` (`SLUS_202.28` `0x001551a0` lines 928-965)
                let steer_dir =
                    if !walls.is_empty() || forbidden.is_some_and(|f| !f.rects.is_empty()) {
                        let ticks = (dt * 60.0).round().max(1.0) as i16;
                        self.path_state.cur_angle = self.yaw;
                        self.path_state.set_path(
                            PathEnemyType::Mannequin,
                            self.position,
                            target.position,
                            path_sway_sample,
                            ticks,
                            |a, b| check_path_ray(a, b, 150.0, walls, forbidden),
                        );
                        self.path_state.target_angle
                    } else {
                        target_dir
                    };
                self.turn_towards(steer_dir, rot_speed_rad_s * dt);

                if dist <= self.sensory.attack_ranges[diff_idx] {
                    let diff = (normalize_angle(target_dir - self.yaw)).abs();
                    if diff <= self.sensory.attack_half_angles[diff_idx] {
                        self.state = MknState::Attack;
                        self.sub_state = 0;
                        self.state_timer = 0.0;
                        return;
                    }
                }

                // HP-scaled walk speed (`enMKNGetWalkSpeed` `0x00157290` × `enCalcSpeedRate`)
                let speed_rate = normalize_angle(target_dir - self.yaw)
                    .cos()
                    .clamp(0.25, 1.0);
                let walk_speed =
                    576.0 * mkn_walk_speed_scale(difficulty, self.stats.hp_rate) * speed_rate;
                self.position.x += self.yaw.sin() * walk_speed * dt;
                self.position.z += self.yaw.cos() * walk_speed * dt;

                // `enMKNCtrlChase` (`0x001551a0:948-963`):
                // If lost sight & `dist > feel_range`: `Circle` (`Around`) on Normal/Hard, else `Wander`.
                if notice == SensoryNotice::None && dist > feel_range {
                    self.state =
                        if matches!(difficulty, DifficultyLevel::Normal | DifficultyLevel::Hard) {
                            MknState::Circle
                        } else {
                            MknState::Wander
                        };
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                } else if dist < 350.0 {
                    self.state = MknState::Precaution;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            MknState::Circle => {
                // `enMKNCtrlAround` (`SLUS_202.28` `0x001553d0` lines 997-1037):
                // Flank point 1000mm away at `±120°` (`2.0943952` rad) from player's facing.
                let rel = normalize_angle(target_dir - target.yaw);
                let flank_angle = if rel < 0.0 {
                    target.yaw + 2.094_395_2
                } else {
                    target.yaw - 2.094_395_2
                };
                let flank_goal = Vec3::new(
                    target.position.x + flank_angle.sin() * 1000.0,
                    target.position.y,
                    target.position.z + flank_angle.cos() * 1000.0,
                );
                let fdx = flank_goal.x - self.position.x;
                let fdz = flank_goal.z - self.position.z;
                let fdist = (fdx * fdx + fdz * fdz).sqrt();
                self.turn_towards(fdx.atan2(fdz), rot_speed_rad_s * dt);

                let walk_speed = 576.0 * mkn_walk_speed_scale(difficulty, self.stats.hp_rate);
                self.position.x += self.yaw.sin() * walk_speed * dt;
                self.position.z += self.yaw.cos() * walk_speed * dt;

                if notice == SensoryNotice::InAttackRange {
                    self.state = MknState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                } else if fdist < 500.0 || dist < feel_range {
                    self.state = MknState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                } else if dist > 3500.0 {
                    self.state = MknState::Wander;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            MknState::Attack => {
                // Attack animation duration ~1.0s
                if self.state_timer >= 1.0 {
                    self.is_kick_next = !self.is_kick_next; // Alternate arm swing and kick
                    self.state = MknState::Chase;
                    self.state_timer = 0.0;
                }
            }

            MknState::Damage if self.stats.flinch_timer <= 0.0 => {
                self.state = MknState::Chase;
                self.state_timer = 0.0;
            }

            MknState::Event => {
                // `enMKNCtrlMannequin2` (`SLUS 0x00153e40`): lies still until `enEventDriven(2)` triggers stand-up.
                match self.sub_state {
                    0 | 1 => {
                        self.sub_state = 1;
                        if self.event_triggered {
                            self.anim_finished = false;
                            self.sub_state = 2;
                        }
                    }
                    _ => {
                        if self.anim_finished {
                            self.anim_finished = false;
                            self.state = MknState::Precaution;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                }
            }

            MknState::Fall => {
                // `enMKNCtrlWaitFall` (`SLUS 0x00154010`): drops when player enters 3000 mm,
                // aims 1000 mm ahead of player (capped at 2500 mm/s XZ), gravity 9800 mm/s² (Y-down).
                match self.sub_state {
                    0 => {
                        if dist < 3000.0 {
                            self.vel_y = 0.0;
                            self.sub_state = 1;
                        }
                    }
                    1 => {
                        let drop_x = target.position.x + target.yaw.sin() * 1000.0;
                        let drop_z = target.position.z + target.yaw.cos() * 1000.0;
                        let ddx = drop_x - self.position.x;
                        let ddz = drop_z - self.position.z;
                        let dlen = (ddx * ddx + ddz * ddz).sqrt();
                        if dist >= 550.0 && dlen > 1e-3 {
                            let step_spd = dlen.min(2500.0);
                            self.position.x += (ddx / dlen) * step_spd * dt;
                            self.position.z += (ddz / dlen) * step_spd * dt;
                        }
                        self.vel_y += dt * 9800.0;
                        self.position.y += self.vel_y * dt;
                        if self.position.y >= target.position.y {
                            self.position.y = target.position.y;
                            self.vel_y = 0.0;
                            self.timer_ticks = 30.0;
                            self.sub_state = 2;
                        }
                    }
                    _ => {
                        self.timer_ticks -= dt * 60.0;
                        if self.timer_ticks <= 0.0 {
                            self.stats.health =
                                (self.stats.health + 100.0).min(self.stats.max_health);
                            self.state = MknState::Chase;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                }
            }

            MknState::Fall2 => {
                // `enMKNCtrlWaitJump` (`SLUS 0x00154590`): parabolic leap (`vel_y = -7500` mm/s, fwd `2000` mm/s, gravity `9800` mm/s²).
                match self.sub_state {
                    0 => {
                        if dist < 3000.0 {
                            self.vel_y = -7500.0;
                            self.sub_state = 1;
                        }
                    }
                    1 => {
                        self.position.x += self.yaw.sin() * 2000.0 * dt;
                        self.position.z += self.yaw.cos() * 2000.0 * dt;
                        self.vel_y += dt * 9800.0;
                        self.position.y += self.vel_y * dt;
                        if self.vel_y >= 0.0 && self.position.y >= target.position.y {
                            self.position.y = target.position.y;
                            self.vel_y = 0.0;
                            self.timer_ticks = 30.0;
                            self.sub_state = 2;
                        }
                    }
                    _ => {
                        self.timer_ticks -= dt * 60.0;
                        if self.timer_ticks <= 0.0 {
                            self.stats.health =
                                (self.stats.health + 100.0).min(self.stats.max_health);
                            self.state = MknState::Chase;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                }
            }

            MknState::Confuse => {
                // `enMKNCtrlConfuse` (`SLUS 0x00155b20`): Hyper Spray friendly-fire if another enemy
                // is within `|dY| < 250` & `dist_xz < 1100` (2 swings), else stunned `600 - 90 * mode` ticks.
                match self.sub_state {
                    0 => {
                        if let Some(other) = self.confuse_friendly_target {
                            let odx = other.x - self.position.x;
                            let odz = other.z - self.position.z;
                            let odist = (odx * odx + odz * odz).sqrt();
                            if (other.y - self.position.y).abs() < 250.0 && odist < 1100.0 {
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
                            self.state = MknState::Precaution;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                    4 | 5 => {
                        if let Some(other) = self.confuse_friendly_target {
                            let odx = other.x - self.position.x;
                            let odz = other.z - self.position.z;
                            self.turn_towards(odx.atan2(odz), 3.5 * dt);
                        }
                        if self.anim_finished {
                            self.anim_finished = false;
                            self.sub_state += 1;
                            self.is_kick_next = !self.is_kick_next;
                            if self.sub_state >= 6 {
                                self.confuse_friendly_target = None;
                                self.state = MknState::Precaution;
                                self.sub_state = 0;
                                self.state_timer = 0.0;
                            }
                        }
                    }
                    _ => {
                        self.state = MknState::Precaution;
                        self.sub_state = 0;
                    }
                }
            }

            MknState::Damage | MknState::Down | MknState::Dying => {}
        }

        // Synchronize and advance skeletal animation controller
        self.anim.sync_mannequin(self.state, self.is_kick_next);
        self.anim.update(dt, self.yaw);
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }

    pub fn receive_hit(&mut self, ap: f32, sp: f32, roll: f32) {
        let knocked_down = self.stats.take_damage(ap, sp, roll);
        if self.stats.health <= 0.0 {
            self.state = MknState::Dying;
        } else if knocked_down {
            self.state = MknState::Down;
        } else {
            self.state = MknState::Damage;
        }
        self.anim.sync_mannequin(self.state, self.is_kick_next);
    }

    /// SH2:EE `Patches/WoodsideMannequinState.cpp` (`0x0059BA35`):
    /// In Wood Side Room 205 (`0x17`), if James has not yet acquired the flashlight,
    /// corrects a mannequin mistakenly marked dead (`MknState::Dying`) back to dormant (`MknState::Stay`).
    pub fn fix_woodside_205_mannequin_state(
        &mut self,
        room_id: u16,
        flashlight_acquired: bool,
        enable_sh2ee_world_fixes: bool,
    ) {
        const R_APT_E_RM_205: u16 = 0x17;
        if enable_sh2ee_world_fixes
            && room_id == R_APT_E_RM_205
            && !flashlight_acquired
            && self.state == MknState::Dying
        {
            self.state = MknState::Stay;
        }
    }
}

/// SH2:EE `Patches/RotatingMannequinGlitch.cpp` (`0x0048CBC5`):
/// In Wood Side 2F Hallway (`R_APT_E_HALLWAY_2F = 0x15`) once the flashlight is acquired,
/// transitions stuck mannequin animation state `0x206` to `0x207`.
#[must_use]
pub const fn fix_rotating_mannequin_glitch(
    room_id: u16,
    flashlight_acquired: bool,
    anim_state: u32,
    enable_sh2ee_world_fixes: bool,
) -> u32 {
    const R_APT_E_HALLWAY_2F: u16 = 0x15;
    if enable_sh2ee_world_fixes
        && room_id == R_APT_E_HALLWAY_2F
        && flashlight_acquired
        && anim_state == 0x206
    {
        0x207
    } else {
        anim_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_mannequin_dormant_stay_and_wake() {
        let mut mkn =
            MannequinController::new(1, Vec3::new(0.0, 0.0, 0.0), 0.0, DifficultyLevel::Normal);
        assert_eq!(mkn.state, MknState::Stay);

        // Player is at 2000 units with flashlight OFF: Mannequin stays dormant
        let target_dark = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2000.0), PI, false, false);
        mkn.update(
            &target_dark,
            RoomLighting::NightLightOff,
            DifficultyLevel::Normal,
            0.1,
        );
        assert_eq!(mkn.state, MknState::Stay);

        // Player shines flashlight towards mannequin from 2000 units (facing PI = -Z towards 0,0,0)
        let target_lit = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2000.0), PI, true, false);
        mkn.update(
            &target_lit,
            RoomLighting::NightIndoorLightOn,
            DifficultyLevel::Normal,
            0.1,
        );
        assert_eq!(mkn.state, MknState::Precaution);
    }

    #[test]
    fn test_mannequin_event_fall_jump_and_confuse_friendly_fire() {
        let mut mkn = MannequinController::new(
            2,
            Vec3::new(0.0, -2000.0, 0.0),
            0.0,
            DifficultyLevel::Normal,
        );
        mkn.state = MknState::Fall;
        let player = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2500.0), 0.0, false, false);
        for _ in 0..80 {
            mkn.update(
                &player,
                RoomLighting::Lit,
                DifficultyLevel::Normal,
                1.0 / 60.0,
            );
        }
        assert!(matches!(mkn.state, MknState::Chase | MknState::Attack));
        assert_eq!(mkn.position.y, 0.0);

        // Fall2 (parabolic leap) wakes at <3000 mm, rises with vel_y = -7500, and lands into Chase.
        let mut jumper = MannequinController::new(
            3,
            Vec3::new(0.0, -2000.0, 0.0),
            0.0,
            DifficultyLevel::Normal,
        );
        jumper.state = MknState::Fall2;
        for _ in 0..160 {
            jumper.update(
                &player,
                RoomLighting::Lit,
                DifficultyLevel::Normal,
                1.0 / 60.0,
            );
        }
        assert_eq!(jumper.position.y, 0.0);
        assert!(matches!(
            jumper.state,
            MknState::Chase | MknState::Attack | MknState::Circle
        ));

        // Event stand-up completes when stand-up animation finishes (`*(i32*)(work+0x14) == -1`).
        mkn.state = MknState::Event;
        mkn.sub_state = 0;
        mkn.event_triggered = true;
        mkn.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        mkn.anim_finished = true;
        mkn.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert_eq!(mkn.state, MknState::Precaution);

        // Confuse with nearby enemy triggers friendly-fire sub_state 4.
        mkn.state = MknState::Confuse;
        mkn.sub_state = 0;
        mkn.confuse_friendly_target = Some(mkn.position + Vec3::new(500.0, 0.0, 0.0));
        mkn.update(
            &player,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert_eq!(mkn.sub_state, 4);
    }

    #[test]
    fn woodside_205_mannequin_and_rotating_mannequin_fixes_respect_toggle() {
        let mut mkn = MannequinController::new(10, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        mkn.state = MknState::Dying;

        // Vanilla leaves Dying untouched
        mkn.fix_woodside_205_mannequin_state(0x17, false, false);
        assert_eq!(mkn.state, MknState::Dying);

        // SH2:EE resets Dying -> Stay (dormant) in Room 205 (0x17) before flashlight pickup
        mkn.fix_woodside_205_mannequin_state(0x17, false, true);
        assert_eq!(mkn.state, MknState::Stay);

        // RotatingMannequinGlitch.cpp: 0x206 -> 0x207 in R_APT_E_HALLWAY_2F (0x15) with flashlight
        assert_eq!(
            fix_rotating_mannequin_glitch(0x15, true, 0x206, false),
            0x206
        );
        assert_eq!(
            fix_rotating_mannequin_glitch(0x15, true, 0x206, true),
            0x207
        );
    }
}
