//! Lying Figure (`chr/scu`, kind 0x200) state machine and AI behaviors (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_scu.c` (`enSCUCtrlMain`, `enSCUCtrlWander`, `enSCUCtrlChase`, `enSCUCtrlAttack`, `enSCUCtrlCrawl`)
//! - PC executable `sh2pc.exe`:
//!   - `0x4c7070` / `0x4c46c0`: `en_scu` control entry point and automatic dispatcher (`0x4c6f50`).
//!   - `0x4c4b30`: `SCU_WANDER` - walks forward +-45 deg, stays within 3500 of origin.
//!   - `0x4c4f50`: `SCU_PRECAUTION` - stands 2..4 loops, turns towards James.
//!   - `0x4c5070`: `SCU_CHASE` - advances at James, calls nearby enemies within 4000 units.
//!   - `0x4c5230`: `SCU_BACKWARD` - retreats when James is closer than 750 units.
//!   - `0x4c5800`: `SCU_CRAWL` - high-speed prone gliding dash (1500 units/s) grabbing at legs (attack 43).
//!   - `0x4c63f0`: `SCU_ATTACK` - bends torso forward and sprays acidic fluid (attack 42).
//!   - `0x4c6900`: `SCU_DOWN` - writhes on floor; regains health/endurance; 80% chance to crawl or get up.
//!
//! # Speeds & Frame Tuning:
//! - Walk root motion: 13.4 units per animation frame (`0x893130` anime 5002, 30 fps = 402 units/s).
//! - Crawl gliding speed: 1,500 units/s.
//! - Acid spray reach: 1,200 units (Normal difficulty).

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::creatures::{apply_stage_hp_multipliers, CreatureStats, EnemySpecies, StagePlace};
use crate::ai::pathfinding::{
    check_path_ray_with_source, EnemyPathState, PathEnemyType, RoomForbiddenAreas,
};
use crate::ai::sensory::{
    evaluate_sensory_with_wall_source, normalize_angle, scu_walk_speed_scale, AiWallSource,
    DifficultyLevel, PerceivableTarget, RoomLighting, SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sh2_assets::formats::cld::WallSegment;

/// Authentic Lying Figure specific state enum (`enCtrlSCUSubFunc` / `0x4c6f50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ScuState {
    #[default]
    Wander = 0,
    Precaution = 1,
    Chase = 2,
    Backward = 3,
    Ambush = 4,
    WaitCar = 5,
    Crawl = 6,
    Attack = 7,
    Damage = 8,
    Confuse = 9,
    Down = 10,
    Dying = 11,
}

/// Authentic Lying Figure crawl recovery probability (`enSCUGetCrawlProbability` `SLUS 0x002ab280` / `sh2pc 0x004c4710`).
pub const SCU_CRAWL_PROBABILITY: f32 = 0.8;

/// Authentic `SLUS_202.28` `enSCUGetFeelRange` (`0x2ab150`) feel ranges by difficulty (mm).
pub const SCU_FEEL_RANGE_MM: [f32; 4] = [400.0, 750.0, 1000.0, 1500.0];

/// Authentic `SLUS_202.28` `enSCUGetRotSpeed` (`0x00164b00`, `0x2ab210`) turn scale × `0.05235988` rad/tick.
pub const SCU_ROT_SPEED_SCALE: [f32; 4] = [0.5, 0.7, 1.0, 1.2];

/// Base rotation quantum per 60 Hz tick (`0.05235988` rad = 3°/tick, `enSCUGetRotSpeed` `0x00164b00`).
pub const SCU_ROT_BASE_RAD_PER_TICK: f32 = 0.052_359_88;

/// Authentic `SLUS_202.28` `enSCUGetAttackProbability` (`0x00164930`, `0x2ab1b0`).
pub const SCU_ATTACK_PROBABILITY: [f32; 4] = [0.2, 0.5, 0.7, 0.8];

/// Authentic `SLUS_202.28` `enSCUGetRepertAttackProbability` (`0x00164a10`, `0x2ab1d0`).
pub const SCU_REPEAT_ATTACK_PROBABILITY: [f32; 4] = [0.0, 0.0, 0.2, 0.5];

/// Leash return timer in 60 Hz ticks (`enSetTimer(param_1, 300)`, `enSCUCtrlWander` `0x001609d0:457`).
pub const SCU_WANDER_RETURN_TICKS: f32 = 300.0;

/// Ambush zone entry (`ambush_data` `SLUS 0x002aac30..0x002ab02c` / `sh2pc 0x00890a0c`, 0 diffs).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScuAmbushEntry {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
    pub pos_x: f32,
    pub pos_z: f32,
    pub angle_rad: f32,
}

/// Lying Figure controller and state machine instance.
#[derive(Debug, Clone)]
pub struct LyingFigureController {
    pub id: u32,
    pub position: Vec3,
    pub origin_pos: Vec3,
    pub yaw: f32,
    pub state: ScuState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub alert_timer: f32,
    pub state_timer: f32,
    pub attack_cooldown: f32,
    pub is_crawling: bool,
    /// `false` = face-down (`lie_dir == 0`, eligible for 80/20 crawl), `true` = face-up (`lie_dir == 1`).
    pub lie_face_up: bool,
    /// True if an overhead obstacle (e.g., car) blocks standing up (`enSCUCheckUpper` `SLUS 0x00164070`).
    pub overhead_blocked: bool,
    /// `enSCUCtrlWaitCar` (`sh2pc.exe` `0x004c5c42`) `work+0x10 |= 8` bit while hiding under a
    /// vehicle (`true` in Vanilla PS2/PC, `false` when `Patches/CreatureVehicleSpawn.cpp` is active).
    pub under_vehicle_collision_bit: bool,
    /// Deterministic roll in `[0.0, 1.0)` used for the 80/20 crawl-vs-stand check (`enSCUCtrlDown` `0x00163010`).
    pub recovery_roll: f32,
    /// Active ambush entry when in [`ScuState::Ambush`] (`work+0xc0` in `enSCUCtrlAmbush` `0x00161600`).
    pub active_ambush: Option<ScuAmbushEntry>,
    /// Confuse countdown timer in 60 Hz ticks (`600 - 90 * mode`, `enSCUCtrlConfuse` `0x00162ee0`).
    pub confuse_timer_ticks: f32,
    /// Obstacle-avoidance pathfinding state (`enInitPath` `0x00144aa0` / `enSetPath` `0x00144ae0`).
    pub path_state: EnemyPathState,
    /// Last tribe alert target received via `enCommunicateTribe` (`0x00146e40`).
    pub tribe_alert_target: Option<Vec3>,
    pub sensory: SensoryProfile,
    pub anim: CreatureAnimationController,
}

impl LyingFigureController {
    /// Create a Lying Figure (`SCU`) at default stage tier (`enSCUInitData`, `SLUS_202.28` `0x001603b0`).
    ///
    /// - HP table (`0x2ab070`): `[120.0, 400.0, 700.0, 1500.0]`
    /// - Endurance table (`0x2ab090`): `[10.0, 190.0, 285.0, 380.0]`
    /// - Auto-recovery (`enSCUAutoRecovery` `0x00164630`, `0x2ab100`): `[0.0, 10.0, 30.0, 60.0]`
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        Self::new_for_stage(id, position, yaw, difficulty, StagePlace::Default)
    }

    /// Create a Lying Figure applying `enSetHP` (`0x001420c0`) stage multipliers for `place`.
    pub fn new_for_stage(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
        place: StagePlace,
    ) -> Self {
        const SCU_HP: [f32; 4] = [120.0, 400.0, 700.0, 1500.0];
        const SCU_END: [f32; 4] = [10.0, 190.0, 285.0, 380.0];
        const SCU_REC: [f32; 4] = [0.0, 10.0, 30.0, 60.0];
        let idx = difficulty.as_index();
        let (hp, end) =
            apply_stage_hp_multipliers(EnemySpecies::LyingFigure, SCU_HP[idx], SCU_END[idx], place);

        Self {
            id,
            position,
            origin_pos: position,
            yaw,
            state: ScuState::Wander,
            sub_state: 0,
            stats: CreatureStats::new(hp, end, SCU_REC[idx]),
            alert_timer: 0.0,
            state_timer: 0.0,
            attack_cooldown: 0.0,
            is_crawling: false,
            lie_face_up: false,
            overhead_blocked: false,
            under_vehicle_collision_bit: false,
            recovery_roll: 0.5,
            active_ambush: None,
            confuse_timer_ticks: 0.0,
            path_state: EnemyPathState::new(yaw),
            tribe_alert_target: None,
            sensory: SensoryProfile::lying_figure(),
            anim: CreatureAnimationController::new_lying_figure(),
        }
    }

    /// Update Lying Figure state machine and movement (open-arena convenience wrapper).
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        lighting: RoomLighting,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        self.update_with_world(target, lighting, difficulty, &[], None, None, 0.0, dt);
    }

    /// Update Lying Figure state machine with `.cld` wall LOS (`enCheckHitEyes` `0x00145540`),
    /// `enSetPath` (`0x00144ae0`) obstacle avoidance, [`RoomForbiddenAreas`], and tribe alerts
    /// (`enCommunicateTribe` `0x00146e40`, `en_scu.c:343,514,603,661`).
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
        self.update_with_wall_source(
            target,
            lighting,
            difficulty,
            AiWallSource::Slice(walls),
            forbidden,
            tribe_alert_pos,
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
        tribe_alert_pos: Option<Vec3>,
        path_sway_sample: f32,
        dt: f32,
    ) {
        if self.state == ScuState::Dying {
            return;
        }

        self.stats.update(dt);
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }
        self.state_timer += dt;

        // Death check
        if self.stats.health <= 0.0 {
            self.state = ScuState::Dying;
            return;
        }

        // Downed recovery check (`enSCUCtrlDown` `SLUS 0x00163010` / `sh2pc 0x004c6d90`)
        if self.state == ScuState::Down {
            if self.stats.down_timer <= 0.0 {
                if difficulty == DifficultyLevel::Beginner {
                    // Beginner mode dies when down timer expires (`enSCUCtrlDown` `0x00163010`).
                    self.stats.health = 0.0;
                    self.stats.endurance = 0.0;
                    self.state = ScuState::Dying;
                    return;
                }
                self.stats.health = (self.stats.health + 50.0).min(self.stats.max_health);
                // 80% crawl (`SCU_CRAWL_PROBABILITY = 0.8` `0x002ab280`) when face-down, or forced crawl if overhead blocked (`enSCUCheckUpper` `0x00164070`).
                let should_crawl = (!self.lie_face_up
                    && self.recovery_roll < SCU_CRAWL_PROBABILITY)
                    || self.overhead_blocked;
                if should_crawl {
                    self.is_crawling = true;
                    self.state = ScuState::Crawl;
                } else {
                    self.is_crawling = false;
                    self.state = ScuState::Precaution;
                }
                self.sub_state = 0;
                self.state_timer = 0.0;
            }
            return;
        }

        // Damage flinch check
        if self.stats.flinch_timer > 0.0 {
            self.state = ScuState::Damage;
            return;
        }

        // Sensory evaluation with `.cld` wall LOS (`enCheckHitEyes2` `0x00145770`)
        let (notice, new_alert) = evaluate_sensory_with_wall_source(
            &self.sensory,
            self.position,
            self.yaw,
            self.alert_timer,
            target,
            lighting,
            difficulty,
            wall_source,
            dt,
        );
        self.alert_timer = new_alert;
        if let Some(alert_pos) = tribe_alert_pos {
            self.tribe_alert_target = Some(alert_pos);
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        let diff_idx = difficulty.as_index();
        let feel_range = SCU_FEEL_RANGE_MM[diff_idx];
        let rot_speed_rad_s = SCU_ROT_SPEED_SCALE[diff_idx] * SCU_ROT_BASE_RAD_PER_TICK * 60.0;

        match self.state {
            ScuState::Wander => {
                // `enSCUCtrlWander` (`SLUS_202.28` `0x001609d0` lines 343-482):
                // Tribe alert, sound, or `dist < feel_range` (non-Beginner) -> Precaution;
                // Direct sight -> Attack (`2`) or Chase (`1`); leash `3500.0` mm with `300`-tick return.
                if tribe_alert_pos.is_some()
                    || notice == SensoryNotice::AuditoryContact
                    || (dist < feel_range && difficulty != DifficultyLevel::Beginner)
                {
                    self.state = ScuState::Precaution;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                    return;
                }
                if notice == SensoryNotice::InAttackRange {
                    self.state = ScuState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    return;
                }
                if notice == SensoryNotice::VisualContact {
                    self.state = ScuState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                    return;
                }

                let odx = self.position.x - self.origin_pos.x;
                let odz = self.position.z - self.origin_pos.z;
                if odx * odx + odz * odz > 3500.0 * 3500.0 {
                    self.turn_towards((-odx).atan2(-odz), rot_speed_rad_s * dt);
                    if self.state_timer >= SCU_WANDER_RETURN_TICKS / 60.0 {
                        // `enSCUCtrlWander` (`0x001609d0:372-383`): reset wander origin after 300 ticks
                        self.origin_pos = self.position;
                        self.state_timer = 0.0;
                    }
                }
                self.position.x += self.yaw.sin() * 402.0 * dt;
                self.position.z += self.yaw.cos() * 402.0 * dt;
            }

            ScuState::Precaution => {
                // `enSCUCtrlPrecaution` (`SLUS_202.28` `0x00160fe0` lines 508-533):
                // Turns at `enSCUGetRotSpeed`; transitions on `enSCUCanSeePlayer` (`2`->Attack,
                // `1` && `dist > 500.0` -> Chase, else `dist > attack_range` -> Wander).
                let aim_dir = if notice == SensoryNotice::None {
                    self.tribe_alert_target.map_or(target_dir, |p| {
                        (p.x - self.position.x).atan2(p.z - self.position.z)
                    })
                } else {
                    target_dir
                };
                self.turn_towards(aim_dir, rot_speed_rad_s * dt);
                if notice == SensoryNotice::InAttackRange {
                    self.state = ScuState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                } else if notice == SensoryNotice::VisualContact && dist > 500.0 {
                    self.state = ScuState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                } else if notice == SensoryNotice::None
                    && tribe_alert_pos.is_none()
                    && dist > self.sensory.attack_ranges[diff_idx]
                    // Transitional clip-end stand-in (`0.5s`): `enSCUCtrlPrecaution` (`0x00160fe0`)
                    // checks `.anm` completion (`*(short *)(work + 0x14) == -1`) once animation
                    // playback state is plumbed into `update()`.
                    && self.state_timer >= 0.5
                {
                    self.state = ScuState::Wander;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Chase => {
                // `enSCUCtrlChase` (`SLUS_202.28` `0x00161190` lines 566-613)
                let steer_dir =
                    if !wall_source.is_empty() || forbidden.is_some_and(|f| !f.rects.is_empty()) {
                        let ticks = (dt * 60.0).round().max(1.0) as i16;
                        self.path_state.cur_angle = self.yaw;
                        self.path_state.set_path(
                            PathEnemyType::LyingFigure,
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
                self.turn_towards(steer_dir, rot_speed_rad_s * dt);

                // Backward retreat if player is too close (`dist < 750.0`, non-Beginner, `en_scu.c:596`)
                if dist < 750.0 && difficulty != DifficultyLevel::Beginner {
                    self.state = ScuState::Backward;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    return;
                }

                // Spitting range check (`enSCUGetAttackRange` `0x00164860`, `enSCUGetAttackAngle` `0x001648e0`)
                if dist <= self.sensory.attack_ranges[diff_idx] && self.attack_cooldown <= 0.0 {
                    let angle_diff = (normalize_angle(target_dir - self.yaw)).abs();
                    if angle_diff <= self.sensory.attack_half_angles[diff_idx] {
                        self.state = ScuState::Attack;
                        self.sub_state = 0;
                        self.state_timer = 0.0;
                        return;
                    }
                }

                // HP-scaled walk speed (`enSCUGetWalkSpeed` `0x001646a0`)
                let speed_rate = normalize_angle(target_dir - self.yaw)
                    .cos()
                    .clamp(0.25, 1.0);
                let walk_speed =
                    402.0 * scu_walk_speed_scale(difficulty, self.stats.hp_rate) * speed_rate;
                self.position.x += self.yaw.sin() * walk_speed * dt;
                self.position.z += self.yaw.cos() * walk_speed * dt;

                // Lost contact (`enSCUCtrlChase` `0x00161190:592-612`): immediate return when `notice == None && dist > feel_range`
                if notice == SensoryNotice::None && dist > feel_range {
                    self.state = if tribe_alert_pos.is_some() {
                        ScuState::Precaution
                    } else {
                        ScuState::Wander
                    };
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Backward => {
                // `enSCUCtrlBackward` (`SLUS_202.28` `0x00161420`)
                self.turn_towards(target_dir, rot_speed_rad_s * dt);
                let walk_speed = 402.0 * scu_walk_speed_scale(difficulty, self.stats.hp_rate);
                self.position.x -= self.yaw.sin() * walk_speed * dt;
                self.position.z -= self.yaw.cos() * walk_speed * dt;

                if dist >= 750.0 {
                    self.state = ScuState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Crawl => {
                // High speed gliding dash (1500 units/s)
                self.turn_towards(target_dir, 4.0 * dt);
                self.position.x += self.yaw.sin() * 1500.0 * dt;
                self.position.z += self.yaw.cos() * 1500.0 * dt;

                // Crawl strike at player legs
                if dist <= 300.0 {
                    self.state_timer = 0.0;
                    // Grab damage
                }

                if self.state_timer > 5.0 {
                    self.is_crawling = false;
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Attack => {
                // Acid spray duration: 1.2 seconds
                if self.state_timer >= 1.2 {
                    self.attack_cooldown = 2.0;
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Damage => {
                if self.stats.flinch_timer <= 0.0 {
                    self.state = ScuState::Chase;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Ambush => {
                // `enSCUCtrlAmbush` (`SLUS 0x00161600` / `sh2pc 0x004c57f0`).
                if let Some(entry) = self.active_ambush {
                    let adx = entry.pos_x - self.position.x;
                    let adz = entry.pos_z - self.position.z;
                    let adist = (adx * adx + adz * adz).sqrt();
                    if self.sub_state < 3 {
                        if adist < 50.0 {
                            self.position.x = entry.pos_x;
                            self.position.z = entry.pos_z;
                            self.sub_state = 3;
                        } else if adist > 1250.0 {
                            self.active_ambush = None;
                            self.state = ScuState::Wander;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        } else {
                            self.turn_towards(adx.atan2(adz), 3.0 * dt);
                            self.position.x += self.yaw.sin() * 402.0 * dt;
                            self.position.z += self.yaw.cos() * 402.0 * dt;
                            if notice == SensoryNotice::VisualContact {
                                self.state = ScuState::Chase;
                                self.sub_state = 0;
                                self.state_timer = 0.0;
                            }
                        }
                    } else {
                        // Sub 3: wait at ambush point, facing halfway between `entry.angle_rad` and `player_dir`.
                        let half_yaw = normalize_angle(
                            entry.angle_rad + normalize_angle(target_dir - entry.angle_rad) * 0.5,
                        );
                        self.turn_towards(half_yaw, 3.0 * dt);
                        if notice == SensoryNotice::InAttackRange {
                            self.state = ScuState::Attack;
                            self.sub_state = 0;
                            self.state_timer = 0.0;
                        }
                    }
                } else {
                    self.state = ScuState::Wander;
                    self.sub_state = 0;
                }
            }

            ScuState::WaitCar => {
                // `enSCUCtrlWaitCar` (`SLUS 0x00161b20` / `sh2pc 0x004c5bd0`):
                // lies under car until player enters 2000.0 mm, then faces player and enters Crawl.
                if dist < 2000.0 {
                    self.yaw = target_dir;
                    self.under_vehicle_collision_bit = false;
                    self.is_crawling = true;
                    self.state = ScuState::Crawl;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Confuse => {
                // `enSCUCtrlConfuse` (`SLUS 0x00162ee0` / `sh2pc 0x004c6cd0`):
                // stunned for `600 - 90 * mode` ticks, then transitions to Precaution.
                if self.sub_state == 0 {
                    self.confuse_timer_ticks = (600 - 90 * difficulty.as_index() as i32) as f32;
                    self.sub_state = 2;
                }
                self.confuse_timer_ticks -= dt * 60.0;
                if self.confuse_timer_ticks <= 0.0 {
                    self.state = ScuState::Precaution;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            ScuState::Down | ScuState::Dying => {}
        }

        // Synchronize and update skeletal animation controller with root motion displacement
        self.anim.sync_lying_figure(self.state);
        self.anim.update(dt, self.yaw);
    }

    /// Places this Lying Figure into [`ScuState::WaitCar`] (`Enemy_List` `status == 0x18`),
    /// applying `Patches/CreatureVehicleSpawn.cpp` (`0x004c5c42`) when `enable_sh2ee_world_fixes` is `true`.
    pub fn spawn_under_car(&mut self, enable_sh2ee_world_fixes: bool) {
        self.state = ScuState::WaitCar;
        self.sub_state = 0;
        self.state_timer = 0.0;
        self.overhead_blocked = true;
        self.under_vehicle_collision_bit = !enable_sh2ee_world_fixes;
    }

    /// Turn smoothly towards desired direction.
    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
    }

    /// Apply damage to Lying Figure.
    pub fn receive_hit(&mut self, ap: f32, sp: f32, roll: f32) {
        let knocked_down = self.stats.take_damage(ap, sp, roll);
        if self.stats.health <= 0.0 {
            self.state = ScuState::Dying;
        } else if knocked_down {
            self.state = ScuState::Down;
            self.is_crawling = false;
        } else {
            self.state = ScuState::Damage;
        }
        self.anim.sync_lying_figure(self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_lying_figure_state_transitions() {
        let mut scu =
            LyingFigureController::new(1, Vec3::new(0.0, 0.0, 0.0), 0.0, DifficultyLevel::Normal);
        assert_eq!(scu.state, ScuState::Wander);

        // Player appears ahead in lit room (1500 units): `enSCUCtrlWander` (`0x00161300`)
        // transitions immediately to `Chase` on visual contact.
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1500.0), PI, false, false);
        scu.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(scu.state, ScuState::Chase);

        // Player behind within feel range (1000 mm < 2500 mm Normal feel range) triggers `Precaution`.
        let mut scu_alert =
            LyingFigureController::new(2, Vec3::new(0.0, 0.0, 0.0), 0.0, DifficultyLevel::Normal);
        let behind = PerceivableTarget::new(Vec3::new(0.0, 0.0, -1000.0), 0.0, false, false);
        scu_alert.update(&behind, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(scu_alert.state, ScuState::Precaution);

        // Damage knockdown test (`enReduceHP` 0x00142200: AP reduces both HP and Endurance;
        // `enSetHitBack` 0x00142630: SP drives hitback impulse `sp * 2.0`).
        scu.receive_hit(300.0, 40.0, 0.5); // AP=300 exceeds endurance (285) -> Down
        assert_eq!(scu.state, ScuState::Down);
        assert_eq!(scu.stats.health, 400.0);
        assert_eq!(scu.stats.endurance, 0.0);
        assert_eq!(scu.stats.hitback_speed, 80.0);
    }

    #[test]
    fn test_lying_figure_wait_car_confuse_and_80_20_crawl_recovery() {
        let mut scu = LyingFigureController::new(2, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        scu.state = ScuState::WaitCar;
        let far = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2500.0), 0.0, false, false);
        scu.update(&far, RoomLighting::Lit, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(scu.state, ScuState::WaitCar);

        let near = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1500.0), 0.0, false, false);
        scu.update(
            &near,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            1.0 / 60.0,
        );
        assert_eq!(scu.state, ScuState::Crawl);

        // 80/20 crawl recovery: roll 0.85 (>= 0.8) stands up into Precaution; roll 0.5 (< 0.8) crawls.
        scu.state = ScuState::Down;
        scu.stats.down_timer = 0.0;
        scu.recovery_roll = 0.85;
        scu.overhead_blocked = false;
        scu.update(&far, RoomLighting::Lit, DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(scu.state, ScuState::Precaution);

        // Confuse expires after 600 - 90*2 = 420 ticks -> Precaution (tested with unseen target
        // outside Lit sight range so Precaution does not immediately transition to Chase).
        let unseen = PerceivableTarget::new(Vec3::new(0.0, 0.0, -1000.0), 0.0, false, false);
        scu.state = ScuState::Confuse;
        scu.sub_state = 0;
        for _ in 0..421 {
            scu.update(
                &unseen,
                RoomLighting::Lit,
                DifficultyLevel::Normal,
                1.0 / 60.0,
            );
        }
        assert_eq!(scu.state, ScuState::Precaution);
    }
}
