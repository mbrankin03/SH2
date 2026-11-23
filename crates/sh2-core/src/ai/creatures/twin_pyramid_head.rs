//! Twin Pyramid Heads (`chr/oni`, kind `0x209`, internal type `0x05`) paired boss AI (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype (`SLUS_202.28`, `docs/decomp/slus/Enemy/en_oni.c`):
//!   - `0x00228b00` `enONIInitData` — links twin partners (`partner_ptr`, `twin_idx`), sets `variant = 2`.
//!   - `0x00228dd0` `enONICtrlAutomatic` — 11-state dispatch (`0x0034c830`), `+200.0/s` endurance regen after slow timer.
//!   - `0x00229500` `Chase`, `0x002297b0` `BothAttack` (60° flanking pincer at 2500 mm, 1.2x speed),
//!     `0x00229cf0` `Wait`, `0x0022a120` `Attack` (Spear thrust `0x31`), `0x0022a2f0` `Attack3` (Spear spin `0x32`, 2..5 loops),
//!     `0x0022ac80` `Suicide` (synchronized walk to `suicide_point[0..1]` -> `_DAT_0112646c |= 8`).
//!   - `0x0022b370` `enONISetDamage` — shared HP overflow transfer to partner (`0.5x` SLUS -> `0.75x` PC).
//! - Retail PC (`sh2pc.exe`) drift:
//!   - Attack IDs (`+6` shift): Spear thrust `0x31 -> 0x37` (`0x004be330`), Spear spin `0x32 -> 0x38` (`0x004be4f0`).
//!   - HP (`0x004bc2e0`): `[1200, 5000, 9000, 20000, 30000]` (SLUS `[3000, 7500, 10000, 20000, 40000]`).
//!   - Endurance (`0x004bc2e0`): `[300, 500, 1300, 1500, 2000]` (SLUS `[500, 700, 1000, 2000, 5000]`).
//!   - Survival countdown timer (`0x004bc425`): `[60, 180, 300, 3600, 3600]` seconds × pair count.
//!   - Partner separation for `Attack3` spin (`0x004bcb40`): `750.0 -> 1100.0` mm.
//!   - Shared HP overflow (`0x004bc8b0`): `0.5x -> 0.75x`.

use crate::ai::creatures::CreatureStats;
use crate::ai::sensory::{normalize_angle, DifficultyLevel, PerceivableTarget};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_3, FRAC_PI_4, FRAC_PI_6};

/// Retail PC Twin Pyramid Head HP per twin (`sh2pc 0x004bc2e0`; SLUS `0x0034c7d0` had `[3000, 7500, 10000, 20000, 40000]`).
pub const ONI_HP_PC: [f32; 5] = [1200.0, 5000.0, 9000.0, 20_000.0, 30_000.0];
/// Retail PC Twin Pyramid Head endurance (`sh2pc 0x004bc2e0`; SLUS `0x0034c7f0` had `[500, 700, 1000, 2000, 5000]`).
pub const ONI_ENDURANCE_PC: [f32; 5] = [300.0, 500.0, 1300.0, 1500.0, 2000.0];
/// Retail PC walk speed multipliers (`sh2pc 0x004bc0a0`).
pub const ONI_WALK_SPEED_MULT_PC: [f32; 5] = [0.6, 0.8, 1.0, 1.0, 1.0];
/// Retail PC rotation speed multipliers (`sh2pc 0x004bc1e0`, × `0.05235988` rad/tick).
pub const ONI_ROT_SPEED_MULT_PC: [f32; 5] = [0.5, 0.7, 1.0, 1.2, 1.5];
/// Retail PC slow-timer ticks before `+200.0/s` endurance recovery (`sh2pc 0x004bc220`).
pub const ONI_SLOW_TIME_PC: [f32; 5] = [180.0, 90.0, 60.0, 40.0, 30.0];
/// Retail PC survival countdown in seconds per twin (`sh2pc 0x004bc425`).
pub const ONI_SURVIVAL_TIMER_SEC_PC: [f32; 5] = [60.0, 180.0, 300.0, 3600.0, 3600.0];

/// Retail PC spear attack IDs (`+6` over SLUS `0x31`/`0x32`).
pub const ONI_ATTACK_ID_SPEAR_THRUST_PC: u16 = 0x37;
pub const ONI_ATTACK_ID_SPEAR_SPIN_PC: u16 = 0x38;
/// Partner separation required for `Attack3` spin (`sh2pc 0x004bcb40`; SLUS was `750.0`).
pub const ONI_SPIN_PARTNER_SEP_PC_MM: f32 = 1100.0;
/// Shared HP overflow transfer ratio to partner (`sh2pc 0x004bc8b0`; SLUS was `0.5`).
pub const ONI_SHARED_OVERFLOW_PC: f32 = 0.75;

/// Twin Pyramid Head suicide center coordinates & target yaws (`suicide_point[2]` `SLUS 0x0034c7b0`).
pub const ONI_SUICIDE_POINTS: [(Vec3, f32); 2] = [
    (Vec3::new(-19_344.45, 0.0, 21_723.15), -FRAC_PI_6),
    (Vec3::new(-19_622.8, 0.0, 22_746.35), FRAC_PI_4),
];

/// Twin Pyramid Head state enum matching `0x0034c830` (`enONICtrlAutomatic` `SLUS 0x00228dd0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum OniState {
    Wander = 0,
    #[default]
    Chase = 1,
    BothAttack = 2,
    Wait = 3,
    Berserk = 4,
    Attack = 5,
    Attack3 = 6,
    Damage = 7,
    Confuse = 8,
    Suicide = 9,
    Dead = 10,
}

/// Snapshot of partner twin state passed during paired coordination (`partner_ptr` at `work+0xa8`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwinPartnerSnapshot {
    pub position: Vec3,
    pub state: OniState,
    pub sub_state: u8,
    pub player_dist: f32,
    pub is_dead: bool,
}

/// Twin Pyramid Head AI controller (`en_oni.c`, kind `0x209`).
#[derive(Debug, Clone)]
pub struct TwinPyramidHeadController {
    pub id: u32,
    pub position: Vec3,
    pub yaw: f32,
    /// `0` = first twin, `1` = second twin (`work+0xa0`).
    pub twin_idx: usize,
    /// `0` = normal, `1` = berserk/labyrinth, `2` = twin boss (`work+0x05`).
    pub variant: u8,
    pub state: OniState,
    pub sub_state: u8,
    pub stats: CreatureStats,
    pub slow_timer_ticks: f32,
    pub state_timer_ticks: f32,
    /// Number of spin loops (`2..=5`) for `Attack3` (`work+0xa4`).
    pub spin_loops_remaining: u8,
    /// Retail PC survival countdown timer (`0x0094e520`).
    pub survival_timer_sec: f32,
    pub active_attack_id: Option<u16>,
    /// Set when both twins reach `Suicide` `sub_state == 4` (`_DAT_0112646c |= 8`).
    pub suicide_cutscene_ready: bool,
}

impl TwinPyramidHeadController {
    /// Construct a Twin Pyramid Head controller (`enONIInitData` `SLUS 0x00228b00` / `sh2pc 0x004bc2e0`).
    pub fn new(
        id: u32,
        position: Vec3,
        yaw: f32,
        twin_idx: usize,
        difficulty: DifficultyLevel,
    ) -> Self {
        let idx = difficulty_index(difficulty);
        Self {
            id,
            position,
            yaw: normalize_angle(yaw),
            twin_idx: twin_idx.min(1),
            variant: 2,
            state: OniState::Chase,
            sub_state: 0,
            stats: CreatureStats::new(ONI_HP_PC[idx], ONI_ENDURANCE_PC[idx], 200.0),
            slow_timer_ticks: 0.0,
            state_timer_ticks: 0.0,
            spin_loops_remaining: 0,
            survival_timer_sec: ONI_SURVIVAL_TIMER_SEC_PC[idx] * 2.0,
            active_attack_id: None,
            suicide_cutscene_ready: false,
        }
    }

    /// Snapshot this twin's state for its partner.
    pub fn snapshot(&self, player_pos: Vec3) -> TwinPartnerSnapshot {
        TwinPartnerSnapshot {
            position: self.position,
            state: self.state,
            sub_state: self.sub_state,
            player_dist: xz_dist(self.position, player_pos),
            is_dead: self.stats.health <= 0.0,
        }
    }

    /// Endurance-modulated speed scale (`enONIGetSpeed` `SLUS 0x0022bc90`): `(end/max)*0.75 + 0.25`.
    pub fn speed_scale(&self) -> f32 {
        let ratio = if self.stats.max_endurance > 0.0 {
            (self.stats.endurance / self.stats.max_endurance).clamp(0.0, 1.0)
        } else {
            1.0
        };
        ratio * 0.75 + 0.25
    }

    /// Evaluate `enONICanSeePlayer` (`SLUS 0x0022b520` / `sh2pc 0x004bcb40`).
    ///
    /// Returns `4` (Spin `Attack3`), `2` (Thrust `Attack`), `1` (Chase/Blocked by partner attacking), `0` none.
    pub fn can_see_player(&self, target_pos: Vec3, partner: Option<TwinPartnerSnapshot>) -> u8 {
        if let Some(p) = partner {
            if matches!(p.state, OniState::Attack | OniState::Attack3) {
                return 1; // Prevents simultaneous attacks (`enONICanSeePlayer` `0x0022b520`).
            }
        }
        let dx = target_pos.x - self.position.x;
        let dz = target_pos.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let ang = normalize_angle(dx.atan2(dz) - self.yaw).abs();

        let partner_far =
            partner.is_none_or(|p| xz_dist(self.position, p.position) > ONI_SPIN_PARTNER_SEP_PC_MM);
        let in_inner_arena = self.position.x > -22_100.0
            && self.position.x < -17_900.0
            && self.position.z > 19_500.0
            && self.position.z < 24_500.0;

        if self.variant == 2 && dist < 1100.0 && partner_far && in_inner_arena {
            return 4;
        }
        if dist < 1250.0 && ang < 0.174_532_92 {
            return 2;
        }
        1
    }

    /// Advance Twin Pyramid Head AI by `dt` seconds (`enONICtrlAutomatic` `SLUS 0x00228dd0`).
    pub fn update(
        &mut self,
        target: &PerceivableTarget,
        partner: Option<TwinPartnerSnapshot>,
        difficulty: DifficultyLevel,
        dt: f32,
    ) {
        if self.state == OniState::Dead {
            return;
        }

        let ticks = dt * 60.0;
        let idx = difficulty_index(difficulty);
        self.active_attack_id = None;

        // Endurance recovery (`+200.0/s` once slow_timer_ticks <= 0, `0x00228dd0`).
        if self.slow_timer_ticks > 0.0 {
            self.slow_timer_ticks = (self.slow_timer_ticks - ticks).max(0.0);
        } else {
            self.stats.endurance =
                (self.stats.endurance + dt * 200.0).min(self.stats.max_endurance);
        }

        // Retail PC survival timer (`sh2pc 0x004bc425`): triggers Suicide when expired.
        if self.variant == 2 && self.state != OniState::Suicide {
            self.survival_timer_sec = (self.survival_timer_sec - dt).max(0.0);
            if self.survival_timer_sec <= 0.0
                || (self.stats.health <= 0.0 && partner.is_none_or(|p| p.is_dead))
            {
                self.transition(OniState::Suicide);
                return;
            }
        }

        let dx = target.position.x - self.position.x;
        let dz = target.position.z - self.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let player_dir = dx.atan2(dz);
        let rot_step = 0.052_359_88 * ONI_ROT_SPEED_MULT_PC[idx] * ticks;
        let walk_spd = 420.0 * self.speed_scale() * ONI_WALK_SPEED_MULT_PC[idx];
        let see = self.can_see_player(target.position, partner);

        match self.state {
            OniState::Wander => {
                self.position.x += self.yaw.sin() * walk_spd * dt;
                self.position.z += self.yaw.cos() * walk_spd * dt;
                if see >= 2 {
                    self.transition(OniState::Attack);
                } else if see == 1 {
                    self.transition(OniState::Chase);
                }
            }

            OniState::Chase => {
                self.turn_towards(player_dir, rot_step);
                if see == 4 {
                    self.transition(OniState::Attack3);
                    return;
                }
                if see == 2 {
                    self.transition(OniState::Attack);
                    return;
                }
                // Paired flanking handoff (`enONICtrlChase` `SLUS 0x00229500`):
                // farther twin switches to BothAttack when closer twin is also chasing.
                if let Some(p) = partner {
                    if p.state == OniState::Chase && p.player_dist < dist {
                        self.transition(OniState::BothAttack);
                        return;
                    }
                }
                let sprint = if self.variant == 2
                    && idx > 1
                    && normalize_angle(self.yaw - player_dir).abs() < 0.087_266_46
                {
                    1.5
                } else {
                    1.0
                };
                self.position.x += self.yaw.sin() * walk_spd * sprint * dt;
                self.position.z += self.yaw.cos() * walk_spd * sprint * dt;
            }

            OniState::BothAttack => {
                if see == 4 {
                    self.transition(OniState::Attack3);
                    return;
                }
                if see == 2 {
                    let partner_far = partner.is_none_or(|p| {
                        xz_dist(self.position, p.position) > ONI_SPIN_PARTNER_SEP_PC_MM
                    });
                    self.transition(if partner_far {
                        OniState::Attack3
                    } else {
                        OniState::Attack
                    });
                    return;
                }
                if dist > 4000.0 || partner.is_some_and(|p| dist < p.player_dist) {
                    self.transition(OniState::Chase);
                    return;
                }

                // Flanking pincer: 60° (`1.0471976` rad) off partner's angle at 2500 mm, 1.2x speed (`0x002297b0`).
                let flank_goal = if let Some(p) = partner {
                    let partner_dir =
                        (target.position.x - p.position.x).atan2(target.position.z - p.position.z);
                    let ang_diff = normalize_angle(player_dir - partner_dir);
                    let sign = if ang_diff >= 0.0 { 1.0 } else { -1.0 };
                    let flank_ang = normalize_angle(partner_dir - sign * FRAC_PI_3);
                    Vec3::new(
                        target.position.x + flank_ang.sin() * 2500.0,
                        self.position.y,
                        target.position.z + flank_ang.cos() * 2500.0,
                    )
                } else {
                    target.position
                };
                let fdx = flank_goal.x - self.position.x;
                let fdz = flank_goal.z - self.position.z;
                self.turn_towards(fdx.atan2(fdz), rot_step);
                self.position.x += self.yaw.sin() * walk_spd * 1.2 * dt;
                self.position.z += self.yaw.cos() * walk_spd * 1.2 * dt;
            }

            OniState::Wait => {
                self.turn_towards(player_dir, rot_step);
                if see == 4 {
                    self.transition(OniState::Attack3);
                } else if see == 2 {
                    self.transition(OniState::Attack);
                } else if dist > 2500.0 {
                    self.transition(OniState::Chase);
                }
            }

            OniState::Berserk => {
                self.turn_towards(player_dir, rot_step);
                if see >= 2 {
                    self.transition(OniState::Attack);
                    return;
                }
                self.position.x += self.yaw.sin() * walk_spd * 1.5 * dt;
                self.position.z += self.yaw.cos() * walk_spd * 1.5 * dt;
            }

            OniState::Attack => {
                if self.sub_state == 0 {
                    self.state_timer_ticks = 45.0;
                    self.sub_state = 1;
                }
                self.active_attack_id = Some(ONI_ATTACK_ID_SPEAR_THRUST_PC);
                if idx > 2 || dist < 500.0 {
                    self.turn_towards(player_dir, rot_step);
                }
                self.state_timer_ticks -= ticks;
                if self.state_timer_ticks <= 0.0 {
                    self.transition(if partner.is_some() && see > 1 {
                        OniState::Wait
                    } else {
                        OniState::Chase
                    });
                }
            }

            OniState::Attack3 => match self.sub_state {
                0 => {
                    self.state_timer_ticks = 20.0;
                    self.sub_state = 1;
                }
                1 => {
                    self.state_timer_ticks -= ticks;
                    if self.state_timer_ticks <= 0.0 {
                        self.spin_loops_remaining = 3; // deterministic middle of 2..=5 loops
                        self.state_timer_ticks = 30.0;
                        self.sub_state = 2;
                    }
                }
                2 => {
                    self.active_attack_id = Some(ONI_ATTACK_ID_SPEAR_SPIN_PC);
                    self.state_timer_ticks -= ticks;
                    if self.state_timer_ticks <= 0.0 {
                        self.spin_loops_remaining = self.spin_loops_remaining.saturating_sub(1);
                        if self.spin_loops_remaining == 0 || dist > 2500.0 {
                            self.state_timer_ticks = 25.0;
                            self.sub_state = 3;
                        } else {
                            self.state_timer_ticks = 30.0;
                        }
                    }
                }
                _ => {
                    self.state_timer_ticks -= ticks;
                    if self.state_timer_ticks <= 0.0 {
                        self.transition(OniState::Chase);
                    }
                }
            },

            OniState::Damage => {
                self.state_timer_ticks -= ticks;
                if self.state_timer_ticks <= 0.0 {
                    if self.variant == 2
                        && self.stats.health <= 0.0
                        && partner.is_none_or(|p| p.is_dead)
                    {
                        self.transition(OniState::Suicide);
                    } else {
                        self.transition(OniState::Chase);
                    }
                }
            }

            OniState::Confuse => {
                if self.sub_state == 0 {
                    self.state_timer_ticks = (600 - 90 * idx as i32) as f32;
                    self.sub_state = 2;
                }
                self.state_timer_ticks -= ticks;
                if self.state_timer_ticks <= 0.0 {
                    self.transition(OniState::Wander);
                }
            }

            OniState::Suicide => {
                let (goal_pos, goal_yaw) = ONI_SUICIDE_POINTS[self.twin_idx];
                match self.sub_state {
                    0 | 1 => {
                        self.sub_state = 1;
                        let gdx = goal_pos.x - self.position.x;
                        let gdz = goal_pos.z - self.position.z;
                        let gdist = (gdx * gdx + gdz * gdz).sqrt();
                        if gdist < 50.0 {
                            self.position = goal_pos;
                            self.sub_state = 3;
                        } else {
                            self.turn_towards(gdx.atan2(gdz), rot_step);
                            self.position.x += self.yaw.sin() * walk_spd * dt;
                            self.position.z += self.yaw.cos() * walk_spd * dt;
                        }
                    }
                    3 => {
                        self.turn_towards(goal_yaw, rot_step);
                        if normalize_angle(goal_yaw - self.yaw).abs() < 0.02 {
                            self.yaw = goal_yaw;
                            self.sub_state = 4;
                        }
                    }
                    _ => {
                        if partner.is_none_or(|p| p.state == OniState::Suicide && p.sub_state == 4)
                        {
                            self.suicide_cutscene_ready = true;
                        }
                    }
                }
            }

            OniState::Dead => {}
        }
    }

    /// Apply incoming damage (`enONISetDamage` `SLUS 0x0022b370` / `sh2pc 0x004bc8b0`).
    ///
    /// Returns the overflow damage (`* 0.75` on retail PC) to apply to the partner twin if this twin's HP drops below `0.0`.
    pub fn receive_hit(
        &mut self,
        ap: f32,
        damage_kind: u8,
        is_spray: bool,
        spray_power: u8,
        difficulty: DifficultyLevel,
    ) -> f32 {
        if matches!(self.state, OniState::Suicide | OniState::Dead) {
            return 0.0;
        }
        // Retail PC `0x004bc8b0`: ignores self/partner spear attacks 0x37 & 0x38.
        if matches!(damage_kind, 0x37 | 0x38) {
            return 0.0;
        }
        let idx = difficulty_index(difficulty);

        if is_spray {
            if spray_power >= 2 {
                self.stats.health = 0.0;
                return 0.0;
            }
            self.transition(OniState::Confuse);
            return 0.0;
        }

        let mult = match damage_kind {
            0x06 => 1.5,
            0x11 | 0x12 => 5.0,
            _ => 1.0,
        };
        let dmg = ap * mult;
        let next_hp = self.stats.health - dmg;
        let overflow = if next_hp < 0.0 {
            self.stats.health = 0.0;
            self.stats.endurance = 0.0;
            (-next_hp) * ONI_SHARED_OVERFLOW_PC
        } else {
            self.stats.health = next_hp;
            if matches!(damage_kind, 0x0e | 0x11) {
                self.stats.endurance = 0.0;
            } else {
                self.stats.endurance = (self.stats.endurance - dmg).max(0.0);
            }
            0.0
        };

        // Light hits (1, 2, 0xc, 0xd) do not reset slow timer or stagger in Twin Boss mode (`0x004bc8b0`).
        if !matches!(damage_kind, 1 | 2 | 0x0c | 0x0d) {
            self.slow_timer_ticks = ONI_SLOW_TIME_PC[idx];
            if self.stats.endurance <= 0.0 && self.state != OniState::Attack3 {
                self.stats.endurance = self.stats.max_endurance;
                self.state = OniState::Damage;
                self.sub_state = 1;
                self.state_timer_ticks = 20.0;
            }
        }
        overflow
    }

    fn transition(&mut self, next: OniState) {
        self.state = next;
        self.sub_state = 0;
    }

    fn turn_towards(&mut self, desired_yaw: f32, max_step: f32) {
        let diff = normalize_angle(desired_yaw - self.yaw);
        let step = diff.clamp(-max_step, max_step);
        self.yaw = normalize_angle(self.yaw + step);
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

    #[test]
    fn test_twin_ph_paired_flanking_and_shared_hp_overflow() {
        let mut t0 = TwinPyramidHeadController::new(
            1,
            Vec3::new(-20_000.0, 0.0, 20_000.0),
            0.0,
            0,
            DifficultyLevel::Normal,
        );
        let mut t1 = TwinPyramidHeadController::new(
            2,
            Vec3::new(-20_000.0, 0.0, 23_000.0),
            0.0,
            1,
            DifficultyLevel::Normal,
        );
        let player = PerceivableTarget::new(Vec3::new(-20_000.0, 0.0, 20_800.0), 0.0, true, false);

        // t1 is farther than t0 (2200 vs 800), so t1 hands off to BothAttack flanking.
        let s0 = t0.snapshot(player.position);
        t1.update(&player, Some(s0), DifficultyLevel::Normal, 1.0 / 60.0);
        assert_eq!(t1.state, OniState::BothAttack);

        // Overflow damage transfers 75% to partner (`sh2pc 0x004bc8b0`).
        t0.stats.health = 100.0;
        let overflow = t0.receive_hit(500.0, 0x05, false, 0, DifficultyLevel::Normal);
        assert!((overflow - 300.0).abs() < 1e-3);
    }
}
