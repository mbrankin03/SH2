//! Bubble Head Nurse (`chr/nse`, kind 0x207) state machine and AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_nse.c` (`enNSECtrlWander`, `enNSECtrlPrecaution`, `enNSECtrlChase`, `enNSECtrlAttack`, `enNSECtrlDamage`, `enNSECtrlDown`)
//! - PC executable `sh2pc.exe`:
//!   - `0x4bb390`: Nurse state table (`NSE_WANDER`, `NSE_PRECAUTION`, `NSE_CHASE`, `NSE_ATTACK`, `NSE_DAMAGE`, `NSE_CONFUSE`, `NSE_DOWN`, `NSE_DYING`).
//!   - `0x892e18`: Sight table (4000/4000/3000/1000 by light, 80° view angle, attack 1500 within 5° or 900 within 30°).
//!   - Pipe swing attacks: reach 400–600 units, AP 12–18.
//!   - Erratic twitching locomotion: staggered burst steps alternating with sudden pauses.

use crate::ai::creatures::anim::CreatureAnimationController;
use crate::ai::creatures::{apply_stage_hp_multipliers, CreatureStats, EnemySpecies, StagePlace};
use crate::ai::pathfinding::{
    check_path_ray_with_source, EnemyPathState, PathEnemyType, RoomForbiddenAreas,
};
use crate::ai::sensory::{
    evaluate_sensory_with_wall_source, normalize_angle, nse_walk_speed_scale, AiWallSource,
    DifficultyLevel, PerceivableTarget, RoomLighting, SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sh2_assets::formats::cld::WallSegment;

/// Authentic `SLUS_202.28` `enNSEGetFeelRange` (`0x00159ba0`, `0x2aa720`) proximity feel ranges (mm).
pub const NSE_FEEL_RANGE_MM: [f32; 4] = [1000.0, 1250.0, 1500.0, 1500.0];

/// Authentic `SLUS_202.28` `enNSEGetAttackProbability` (`0x00159c20`, `0x2aa740`) strike probability.
pub const NSE_ATTACK_PROBABILITY: [f32; 4] = [0.1, 0.5, 0.7, 0.9];

/// Authentic `SLUS_202.28` `enNSEGetAttackSpeed` (`0x00159c70`, `0x2aa760`) attack speed multipliers.
pub const NSE_ATTACK_SPEED_SCALE: [f32; 4] = [0.5, 0.8, 1.0, 1.1];

/// Authentic `SLUS_202.28` `enNSEGetRotSpeed` (`0x00159d10`, `0x2aa780`) turn scale × `0.05235988` rad/tick.
pub const NSE_ROT_SPEED_SCALE: [f32; 4] = [0.5, 0.8, 1.0, 1.2];

/// Base rotation quantum per 60 Hz tick (`0.05235988` rad = 3°/tick, `enNSEGetRotSpeed` `0x00159d10`).
pub const NSE_ROT_BASE_RAD_PER_TICK: f32 = 0.052_359_88;

/// Authentic `SLUS_202.28` `enNSESetDownTime` (`0x00159d70`, `0x2aa798`) knockdown durations in 60 Hz ticks.
pub const NSE_DOWN_TIME_TICKS: [i16; 4] = [600, 480, 360, 300];

/// Authentic Nurse state enum (`0x4bb390`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NseState {
    #[default]
    Wander = 0,
    Precaution = 1,
    Chase = 2,
    Attack = 3,
    Damage = 4,
    Confuse = 5,
    Down = 6,
    Dying = 7,
}

/// Bubble Head Nurse controller and state machine.
#[derive(Debug, Clone)]
pub struct NurseController {
    pub id: u32,
    pub position: Vec3,
    pub origin_pos: Vec3,
    pub yaw: f32,
    pub state: NseState,
    pub stats: CreatureStats,
    pub alert_timer: f32,
    pub state_timer: f32,
    pub twitch_timer: f32,
    /// Current speed modulation ramp (`work + 0xa0`, ramped `±0.01/tick` toward `speed_target`, `enNSECtrlWander`/`Chase`).
    pub speed_mod: f32,
    /// Target speed modulation (`work + 0xa4`, `1.0..1.5` in `enNSECtrlWander` `0x00157bf0`).
    pub speed_target: f32,
    /// Deterministic roll `[0.0, 1.0)` for `enNSECtrlAttack` (`0x001583a0`) probability & lunge selection.
    pub attack_roll: f32,
    /// Number of consecutive combo strikes executed in the current `Attack` state (`enNSECtrlAttack` `0x001583a0`).
    pub attack_combo_count: u8,
    /// Confuse countdown timer in 60 Hz ticks (`600 - 90 * mode`, `enNSECtrlConfuse` `SLUS 0x001589a0`).
    pub confuse_timer_ticks: f32,
    pub sub_state: u8,
    /// True for the `0x20b` Town Tough / Dark Nurse variant (`enNSEInitData` `0x001577b0`,
    /// uses retail attack IDs `48`/`49` instead of `46`/`47` in `enNSECtrlAttack` `0x001583a0`).
    pub is_dark_variant: bool,
    /// True when the active attack is a lunge strike (`anime == 4` -> retail `47`/`49`;
    /// close strike `anime == 3` -> retail `46`/`48`, `enNSECtrlAttack` `0x001583a0` lines 471–488).
    pub is_lunge_attack: bool,
    /// Obstacle-avoidance pathfinding state (`enInitPath` `0x00144aa0` / `enSetPath` `0x00144ae0`).
    pub path_state: EnemyPathState,
    pub sensory: SensoryProfile,
    pub anim: CreatureAnimationController,
}

impl NurseController {
    /// Create a Bubble Head Nurse (`NSE`, `0x207`) at default stage tier
    /// (`enNSEInitData`, `SLUS_202.28` `0x001577b0`).
    ///
    /// - HP table (`0x2aa680`, `i16[5]`): `[400.0, 500.0, 800.0, 1500.0]` (`× 1.2` for `0x20b` variant)
    /// - Endurance table (`0x2aa690`): `[10.0, 190.0, 285.0, 380.0]`
    /// - Auto-recovery (`enNSEAutoRecovery` `0x00159a90`, `0x2aa6f0`): `[0.0, 10.0, 30.0, 60.0]`
    pub fn new(id: u32, position: Vec3, yaw: f32, difficulty: DifficultyLevel) -> Self {
        Self::new_for_stage(id, position, yaw, difficulty, false, StagePlace::Default)
    }

    /// Create a Bubble Head Nurse with optional `0x20b` dark-nurse `1.2x` HP/Endurance multiplier
    /// and `enSetHP` (`0x001420c0`) stage multipliers.
    pub fn new_for_stage(
        id: u32,
        position: Vec3,
        yaw: f32,
        difficulty: DifficultyLevel,
        is_dark_variant: bool,
        place: StagePlace,
    ) -> Self {
        const NSE_HP: [f32; 4] = [400.0, 500.0, 800.0, 1500.0];
        const NSE_END: [f32; 4] = [10.0, 190.0, 285.0, 380.0];
        const NSE_REC: [f32; 4] = [0.0, 10.0, 30.0, 60.0];
        let idx = difficulty.as_index();
        let mult = if is_dark_variant { 1.2 } else { 1.0 };
        let (hp, end) = apply_stage_hp_multipliers(
            EnemySpecies::Nurse,
            NSE_HP[idx] * mult,
            NSE_END[idx] * mult,
            place,
        );

        Self {
            id,
            position,
            origin_pos: position,
            yaw,
            state: NseState::Wander,
            stats: CreatureStats::new(hp, end, NSE_REC[idx]),
            alert_timer: 0.0,
            state_timer: 0.0,
            twitch_timer: 0.0,
            speed_mod: 1.0,
            speed_target: 1.0,
            attack_roll: 0.0,
            attack_combo_count: 0,
            confuse_timer_ticks: 0.0,
            sub_state: 0,
            is_dark_variant,
            is_lunge_attack: false,
            path_state: EnemyPathState::new(yaw),
            sensory: SensoryProfile::nurse(),
            anim: CreatureAnimationController::new_nurse(),
        }
    }

    /// Retail PC `sh2_attack_list` ID for this Nurse's current attack (`enNSECtrlAttack`
    /// `SLUS_202.28` `0x001583a0` lines 471–488, +6 shifted):
    /// - Normal Nurse (`0x207`): `46` (`0x2e`, proto `0x28`) close strike / `47` (`0x2f`, proto `0x29`) lunge
    /// - Dark Nurse (`0x20b`): `48` (`0x30`, proto `0x2a`) close strike / `49` (`0x31`, proto `0x2b`) lunge
    #[inline]
    pub const fn active_attack_id(&self) -> u16 {
        match (self.is_dark_variant, self.is_lunge_attack) {
            (false, false) => 46,
            (false, true) => 47,
            (true, false) => 48,
            (true, true) => 49,
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

    /// Update Nurse AI with `.cld` wall LOS (`enCheckHitEyes` `0x00145540`), `enSetPath`
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
        if self.state == NseState::Dying {
            return;
        }

        self.stats.update(dt);
        self.state_timer += dt;
        self.twitch_timer += dt;

        if self.stats.health <= 0.0 {
            self.state = NseState::Dying;
            return;
        }

        if self.state == NseState::Down {
            if self.stats.down_timer <= 0.0 {
                self.state = NseState::Chase;
                self.state_timer = 0.0;
            }
            return;
        }

        if self.stats.flinch_timer > 0.0 {
            self.state = NseState::Damage;
            return;
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let target_dir = dx.atan2(dz);

        let (notice, alert) = evaluate_sensory_with_wall_source(
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
        self.alert_timer = alert;

        let diff_idx = difficulty.as_index();
        let feel_range = NSE_FEEL_RANGE_MM[diff_idx];
        let rot_speed_rad_s = NSE_ROT_SPEED_SCALE[diff_idx] * NSE_ROT_BASE_RAD_PER_TICK * 60.0;

        match self.state {
            NseState::Wander => {
                // `enNSECtrlWander` (`SLUS_202.28` `0x00157bf0` lines 234-312):
                // Direct Attack if in attack/lunge range; Chase if seen; Precaution if within feel_range or sound.
                if matches!(
                    notice,
                    SensoryNotice::InAttackRange | SensoryNotice::InLungeRange
                ) {
                    self.state = NseState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.attack_combo_count = 0;
                } else if notice == SensoryNotice::VisualContact {
                    self.state = NseState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                } else if dist < feel_range || notice == SensoryNotice::AuditoryContact {
                    self.state = NseState::Precaution;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                } else {
                    // Speed ramp `±0.01/tick` (`enNSECtrlWander` `0x00157bf0:282-291`)
                    let step =
                        (self.speed_target - self.speed_mod).clamp(-0.01, 0.01) * (dt * 60.0);
                    self.speed_mod = (self.speed_mod + step).clamp(1.0, 1.5);
                    let walk_speed = 480.0
                        * self.speed_mod
                        * nse_walk_speed_scale(difficulty, self.stats.hp_rate);
                    self.position.x += self.yaw.sin() * walk_speed * dt;
                    self.position.z += self.yaw.cos() * walk_speed * dt;
                }
            }

            NseState::Precaution => {
                // `enNSECtrlPrecaution` (`SLUS_202.28` `0x00157fd0` lines 340-368):
                // Immediate transitions on `enNSECanSeePlayer` (`>=2` -> Attack, `1` -> Chase, `dist > feel_range` -> Wander).
                self.turn_towards(target_dir, rot_speed_rad_s * dt);
                if matches!(
                    notice,
                    SensoryNotice::InAttackRange | SensoryNotice::InLungeRange
                ) {
                    self.state = NseState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.attack_combo_count = 0;
                } else if notice == SensoryNotice::VisualContact {
                    self.state = NseState::Chase;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                } else if dist > feel_range && notice == SensoryNotice::None {
                    self.state = NseState::Wander;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            NseState::Chase => {
                // `enNSECtrlChase` (`SLUS_202.28` `0x00158140` lines 394-436)
                if matches!(
                    notice,
                    SensoryNotice::InAttackRange | SensoryNotice::InLungeRange
                ) || (wall_source.is_empty() && dist <= self.sensory.attack_ranges[diff_idx])
                {
                    self.state = NseState::Attack;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    self.attack_combo_count = 0;
                    return;
                }
                if notice == SensoryNotice::None && dist > feel_range {
                    self.state = NseState::Wander;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                    return;
                }

                let steer_dir =
                    if !wall_source.is_empty() || forbidden.is_some_and(|f| !f.rects.is_empty()) {
                        let ticks = (dt * 60.0).round().max(1.0) as i16;
                        self.path_state.cur_angle = self.yaw;
                        self.path_state.set_path(
                            PathEnemyType::Nurse,
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

                // `enCalcSpeedRate` + `±0.01/tick` ramp + `enNSEGetWalkSpeed` (`0x00158140:418-431`)
                self.speed_target = normalize_angle(target_dir - self.yaw)
                    .cos()
                    .clamp(0.25, 1.0);
                let step = (self.speed_target - self.speed_mod).clamp(-0.01, 0.01) * (dt * 60.0);
                self.speed_mod = (self.speed_mod + step).clamp(0.25, 1.0);
                let speed_scale = nse_walk_speed_scale(difficulty, self.stats.hp_rate);
                let walk_speed = 650.0 * self.speed_mod * speed_scale;
                self.position.x += self.yaw.sin() * walk_speed * dt;
                self.position.z += self.yaw.cos() * walk_speed * dt;
            }

            NseState::Attack => {
                // `enNSECtrlAttack` (`SLUS_202.28` `0x001583a0` lines 457-531):
                // - `sub_state == 0`: roll against `NSE_ATTACK_PROBABILITY[mode]` (`0x2aa740`).
                //   If `< prob`, select close strike (`anime 3`, `is_lunge_attack = false`) vs lunge
                //   (`anime 4`, `is_lunge_attack = true` when `InLungeRange` && (`dist > 900.0` || roll < 0.5))
                //   -> `sub_state = 1`. Otherwise hesitation (`anime 2`) -> `sub_state = 2`.
                // - `sub_state == 1`: active strike (`46/47` normal, `48/49` dark); tracks player on
                //   Hard (`mode > 2`) or close strike (`!is_lunge_attack`). When swing finishes, if
                //   player remains in attack/lunge range, loops to `sub_state = 0` for multi-hit combo!
                // - `sub_state == 2`: hesitation wait; re-evaluates `enNSECanSeePlayer` when done.
                let atk_speed =
                    NSE_ATTACK_SPEED_SCALE[diff_idx] * (self.stats.hp_rate / 200.0 + 0.5);
                // Transitional clip-end stand-in: `enNSECtrlAttack` (`0x001583f0`) advances
                // `sub_state == 1` on `.anm` completion (`*(short *)(work + 0x14) == -1`) once
                // animation playback state is plumbed into `update()`.
                let swing_duration = (1.0 / atk_speed.max(0.25)).clamp(0.55, 1.6);
                let in_strike_range = matches!(
                    notice,
                    SensoryNotice::InAttackRange | SensoryNotice::InLungeRange
                ) || (wall_source.is_empty()
                    && dist <= self.sensory.attack_ranges[diff_idx]);

                match self.sub_state {
                    0 => {
                        if self.attack_roll < NSE_ATTACK_PROBABILITY[diff_idx] {
                            self.is_lunge_attack = notice == SensoryNotice::InLungeRange
                                && (dist > 900.0 || self.attack_roll < 0.5);
                            self.sub_state = 1;
                            self.state_timer = 0.0;
                            self.attack_combo_count = self.attack_combo_count.saturating_add(1);
                        } else {
                            self.sub_state = 2;
                            self.state_timer = 0.0;
                        }
                    }
                    1 => {
                        if difficulty == DifficultyLevel::Hard || !self.is_lunge_attack {
                            self.turn_towards(target_dir, rot_speed_rad_s * dt);
                        }
                        if self.state_timer >= swing_duration {
                            if in_strike_range {
                                // Loop back to sub_state 0 for combo continuation (`en_nse.c:503`)
                                self.sub_state = 0;
                                self.state_timer = 0.0;
                            } else {
                                self.state = NseState::Chase;
                                self.sub_state = 0;
                                self.state_timer = 0.0;
                            }
                        }
                    }
                    _ => {
                        // Transitional clip-end stand-in (`0.5s`): `enNSECtrlAttack` (`0x001583f0`)
                        // exits hesitation (`anime 2`) on `.anm` completion (`*(short *)(work + 0x14) == -1`)
                        // once animation playback state is plumbed into `update()`.
                        if self.state_timer >= 0.5 {
                            if in_strike_range {
                                self.sub_state = 0;
                                self.state_timer = 0.0;
                            } else {
                                self.state = NseState::Chase;
                                self.sub_state = 0;
                                self.state_timer = 0.0;
                            }
                        }
                    }
                }
            }

            NseState::Damage if self.stats.flinch_timer <= 0.0 => {
                self.state = NseState::Chase;
                self.state_timer = 0.0;
            }

            NseState::Confuse => {
                // `enNSECtrlConfuse` (`SLUS 0x001589a0` / `sh2pc 0x004bb250`):
                // stunned for `600 - 90 * mode` ticks, then transitions to Precaution.
                if self.sub_state == 0 {
                    self.confuse_timer_ticks = (600 - 90 * difficulty.as_index() as i32) as f32;
                    self.sub_state = 2;
                }
                self.confuse_timer_ticks -= dt * 60.0;
                if self.confuse_timer_ticks <= 0.0 {
                    self.state = NseState::Precaution;
                    self.sub_state = 0;
                    self.state_timer = 0.0;
                }
            }

            NseState::Damage | NseState::Down | NseState::Dying => {}
        }

        // Synchronize and update skeletal animation controller
        self.anim.sync_nurse(self.state);
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
            self.state = NseState::Dying;
        } else if knocked_down {
            self.state = NseState::Down;
        } else {
            self.state = NseState::Damage;
        }
        self.anim.sync_nurse(self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nurse_lifecycle() {
        let mut nurse = NurseController::new(1, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        assert_eq!(nurse.state, NseState::Wander);

        // At 1000 mm straight ahead (`InAttackRange`), `enNSECtrlWander` (`0x00157bf0`) enters `Attack`.
        let close = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1000.0), 0.0, false, false);
        nurse.update(&close, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(nurse.state, NseState::Attack);

        // Behind within feel range (1500 mm < 2500 mm Normal feel range) enters `Precaution`.
        let mut nurse_alert = NurseController::new(2, Vec3::ZERO, 0.0, DifficultyLevel::Normal);
        let behind = PerceivableTarget::new(Vec3::new(0.0, 0.0, -1500.0), 0.0, false, false);
        nurse_alert.update(&behind, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(nurse_alert.state, NseState::Precaution);
    }

    #[test]
    fn test_nurse_confuse_timer_to_precaution() {
        let mut nurse = NurseController::new(2, Vec3::ZERO, 0.0, DifficultyLevel::Hard);
        nurse.state = NseState::Confuse;
        nurse.sub_state = 0;
        // Behind within Hard feel range (3000 mm) so Precaution holds after Confuse expires.
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, -2000.0), 0.0, false, false);
        // Hard mode: 600 - 90*3 = 330 ticks.
        for _ in 0..330 {
            nurse.update(
                &target,
                RoomLighting::Lit,
                DifficultyLevel::Hard,
                1.0 / 60.0,
            );
        }
        assert_eq!(nurse.state, NseState::Precaution);
    }

    #[test]
    fn test_nurse_attack_combo_probability_table() {
        // `enNSECtrlAttack` (`SLUS_202.28` `0x00158550` / `en_nse.c:481`):
        // [Beginner=0.1, Easy=0.5, Normal=0.7, Hard=0.9]
        assert_eq!(NSE_ATTACK_PROBABILITY, [0.1, 0.5, 0.7, 0.9]);
        let mut nurse = NurseController::new(3, Vec3::ZERO, 0.0, DifficultyLevel::Hard);
        nurse.state = NseState::Attack;
        nurse.sub_state = 0;
        nurse.attack_roll = 0.2; // < 0.9 Hard probability -> commits to attack
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 900.0), 0.0, false, false);
        nurse.update(
            &target,
            RoomLighting::Lit,
            DifficultyLevel::Hard,
            1.0 / 60.0,
        );
        assert_eq!(nurse.state, NseState::Attack);
        assert!(nurse.sub_state > 0);
    }
}
