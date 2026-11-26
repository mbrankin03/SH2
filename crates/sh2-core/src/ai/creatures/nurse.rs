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
use crate::ai::pathfinding::{check_path_ray, EnemyPathState, PathEnemyType, RoomForbiddenAreas};
use crate::ai::sensory::{
    evaluate_sensory_with_los, normalize_angle, nse_walk_speed_scale, DifficultyLevel,
    PerceivableTarget, RoomLighting, SensoryNotice, SensoryProfile,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sh2_assets::formats::cld::WallSegment;

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
            NseState::Wander => {
                let odx = self.position.x - self.origin_pos.x;
                let odz = self.position.z - self.origin_pos.z;
                if odx * odx + odz * odz > 4000.0 * 4000.0 {
                    self.yaw = (-odx).atan2(-odz);
                }

                // Staggered twitch advance
                let twitch_speed = if (self.twitch_timer % 1.5) < 0.9 {
                    480.0
                } else {
                    60.0 // Twitch hesitation
                };
                self.position.x += self.yaw.sin() * twitch_speed * dt;
                self.position.z += self.yaw.cos() * twitch_speed * dt;

                if notice != SensoryNotice::None {
                    self.state = NseState::Precaution;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                }
            }

            NseState::Precaution => {
                self.turn_towards(target_dir, 3.2 * dt);
                if self.state_timer > 1.2
                    || matches!(
                        notice,
                        SensoryNotice::InAttackRange | SensoryNotice::InLungeRange
                    )
                {
                    self.state = NseState::Chase;
                    self.state_timer = 0.0;
                    self.path_state.init(self.yaw);
                }
            }

            NseState::Chase => {
                let steer_dir =
                    if !walls.is_empty() || forbidden.is_some_and(|f| !f.rects.is_empty()) {
                        let ticks = (dt * 60.0).round().max(1.0) as i16;
                        self.path_state.cur_angle = self.yaw;
                        self.path_state.set_path(
                            PathEnemyType::Nurse,
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
                self.turn_towards(steer_dir, 3.2 * dt);

                if matches!(
                    notice,
                    SensoryNotice::InAttackRange | SensoryNotice::InLungeRange
                ) || (walls.is_empty()
                    && dist <= self.sensory.attack_ranges[difficulty.as_index()])
                {
                    self.is_lunge_attack = notice == SensoryNotice::InLungeRange;
                    self.state = NseState::Attack;
                    self.state_timer = 0.0;
                    return;
                }

                // HP-scaled twitch advance (`enNSEGetWalkSpeed` `0x00159b00`)
                let speed_scale = nse_walk_speed_scale(difficulty, self.stats.hp_rate);
                let twitch_speed = if (self.twitch_timer % 1.2) < 0.8 {
                    650.0 * speed_scale
                } else {
                    120.0 * speed_scale
                };
                self.position.x += self.yaw.sin() * twitch_speed * dt;
                self.position.z += self.yaw.cos() * twitch_speed * dt;

                if notice == SensoryNotice::None && self.state_timer > 5.0 {
                    self.state = NseState::Wander;
                    self.state_timer = 0.0;
                }
            }

            NseState::Attack => {
                // Pipe swing attack duration ~1.1s
                if self.state_timer >= 1.1 {
                    self.state = NseState::Chase;
                    self.state_timer = 0.0;
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

        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1000.0), 0.0, false, false);
        nurse.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.1);
        assert_eq!(nurse.state, NseState::Precaution);
    }

    #[test]
    fn test_nurse_confuse_timer_to_precaution() {
        let mut nurse = NurseController::new(2, Vec3::ZERO, 0.0, DifficultyLevel::Hard);
        nurse.state = NseState::Confuse;
        nurse.sub_state = 0;
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 5000.0), 0.0, false, false);
        // Hard mode: 600 - 90*3 = 330 ticks.
        for _ in 0..335 {
            nurse.update(
                &target,
                RoomLighting::Lit,
                DifficultyLevel::Hard,
                1.0 / 60.0,
            );
        }
        assert_eq!(nurse.state, NseState::Precaution);
    }
}
