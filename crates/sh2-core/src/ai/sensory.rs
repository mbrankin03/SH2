//! Sensory awareness model for Silent Hill 2 enemies (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c`, `src/Enemy/en_scu.c`, `src/Enemy/en_mkn.c`, `src/Enemy/en_nse.c`
//! - PC executable `sh2pc.exe`:
//!   - `0x4a9aa0`: Sensory awareness detection routine (`sight_check`).
//!   - `0x8931b8`: Lying Figure (`EnSCU`) sensory parameters.
//!   - `0x8930f0`: Mannequin (`EnMKN`) sensory parameters.
//!   - `0x892e18`: Bubble Head Nurse (`EnNSE`) sensory parameters.
//!   - `0x542cd0`: Flashlight beam cone vs enemy target evaluation.
//!
//! # Mechanics:
//! - Vision ranges depend on illumination state:
//!   - Lit environment: up to 5,000 units.
//!   - Dark environment + flashlight ON: 3,000–4,000 units.
//!   - Dark environment + flashlight OFF: 750–1,000 units (near-blind).
//! - View cone: half-angle typically 60° (MKN), 70° (SCU), 80° (NSE).
//! - Flashlight alert accumulator: builds up (+dt up to 3.0s) when illuminated; decays when unlit.
//! - Hearing: proximity check based on player noise (footsteps, sprinting) and game difficulty:
//!   - Beginner/Easy: 500–750 units.
//!   - Normal: 1,000–1,200 units.
//!   - Hard: 1,500 units.

use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Illumination state of the environment / room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RoomLighting {
    /// Room has ambient lights on.
    #[default]
    Lit,
    /// Room is dark, but player's flashlight is switched on.
    DarkLightOn,
    /// Room is dark and flashlight is switched off.
    DarkLightOff,
}

/// Difficulty level influencing perception and hearing sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DifficultyLevel {
    Beginner,
    Easy,
    #[default]
    Normal,
    Hard,
}

/// Information about a perceivable character (James or Maria) passed to sensory routines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerceivableTarget {
    pub position: Vec3,
    pub yaw: f32,
    pub is_flashlight_on: bool,
    pub is_running: bool,
    pub is_alive: bool,
}

impl PerceivableTarget {
    pub fn new(position: Vec3, yaw: f32, is_flashlight_on: bool, is_running: bool) -> Self {
        Self {
            position,
            yaw,
            is_flashlight_on,
            is_running,
            is_alive: true,
        }
    }
}

/// Sensory check output flags / state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensoryNotice {
    /// Target is completely undetected.
    None,
    /// Target detected by sight (within vision cone and distance).
    VisualContact,
    /// Target detected by hearing (proximity hearing threshold).
    AuditoryContact,
    /// Target is in close combat / attack strike range.
    InAttackRange,
}

/// Sensory configuration profile per enemy species.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensoryProfile {
    /// Max sight distance in lit room (units).
    pub sight_lit: f32,
    /// Max sight distance in dark room with flashlight on (units).
    pub sight_dark_light_on: f32,
    /// Max sight distance in dark room with flashlight off (units).
    pub sight_dark_light_off: f32,
    /// Field of view half-angle in radians.
    pub view_half_angle: f32,
    /// Hearing proximity range by difficulty: [Beginner, Easy, Normal, Hard].
    pub hearing_ranges: [f32; 4],
    /// Attack range threshold (units).
    pub attack_range: f32,
    /// Attack angle alignment threshold (radians).
    pub attack_half_angle: f32,
}

impl SensoryProfile {
    /// Lying Figure (scu) profile (`0x8931b8`).
    pub const fn lying_figure() -> Self {
        Self {
            sight_lit: 5000.0,
            sight_dark_light_on: 4000.0,
            sight_dark_light_off: 750.0,
            view_half_angle: 70.0 * PI / 180.0, // 1.2217 rad
            hearing_ranges: [0.0, 750.0, 1000.0, 1500.0],
            attack_range: 1200.0,                // Acid spit reach on Normal
            attack_half_angle: 5.0 * PI / 180.0, // Straight ahead
        }
    }

    /// Mannequin (mkn) profile (`0x8930f0`).
    pub const fn mannequin() -> Self {
        Self {
            sight_lit: 5000.0,
            sight_dark_light_on: 3000.0,
            sight_dark_light_off: 900.0,
            view_half_angle: 60.0 * PI / 180.0, // 1.0472 rad
            hearing_ranges: [0.0, 750.0, 1000.0, 1500.0],
            attack_range: 1100.0,
            attack_half_angle: 15.0 * PI / 180.0,
        }
    }

    /// Bubble Head Nurse (nse) profile (`0x892e18`).
    pub const fn nurse() -> Self {
        Self {
            sight_lit: 4000.0,
            sight_dark_light_on: 4000.0,
            sight_dark_light_off: 1000.0,
            view_half_angle: 80.0 * PI / 180.0, // 1.3963 rad
            hearing_ranges: [500.0, 750.0, 1200.0, 1500.0],
            attack_range: 1500.0,
            attack_half_angle: 30.0 * PI / 180.0,
        }
    }

    /// Pyramid Head (red) profile.
    pub const fn pyramid_head() -> Self {
        Self {
            sight_lit: 8000.0,
            sight_dark_light_on: 6000.0,
            sight_dark_light_off: 2000.0,
            view_half_angle: 90.0 * PI / 180.0,
            hearing_ranges: [1000.0, 1500.0, 2000.0, 2500.0],
            attack_range: 1800.0,
            attack_half_angle: 45.0 * PI / 180.0,
        }
    }

    /// Creeper (tyu) profile.
    pub const fn creeper() -> Self {
        Self {
            sight_lit: 3000.0,
            sight_dark_light_on: 2000.0,
            sight_dark_light_off: 500.0,
            view_half_angle: 85.0 * PI / 180.0,
            hearing_ranges: [400.0, 600.0, 800.0, 1000.0],
            attack_range: 300.0,
            attack_half_angle: 60.0 * PI / 180.0,
        }
    }
}

/// Normalizes an angle into (-PI, PI] range.
pub fn normalize_angle(mut rad: f32) -> f32 {
    while rad > PI {
        rad -= 2.0 * PI;
    }
    while rad <= -PI {
        rad += 2.0 * PI;
    }
    rad
}

/// Evaluates whether the target is directly illuminating the enemy with their flashlight (`0x542cd0`).
pub fn is_lit_by_flashlight(
    enemy_pos: Vec3,
    target_pos: Vec3,
    target_yaw: f32,
    target_light_on: bool,
) -> bool {
    if !target_light_on {
        return false;
    }
    let dx = enemy_pos.x - target_pos.x;
    let dz = enemy_pos.z - target_pos.z;
    let dist_sq = dx * dx + dz * dz;
    // Max flashlight beam reaches 6000 units
    if dist_sq > 6000.0 * 6000.0 {
        return false;
    }

    let angle_to_enemy = dx.atan2(dz);
    let diff = (normalize_angle(angle_to_enemy - target_yaw)).abs();
    // Flashlight cone half-angle is ~0.42 radians (~24 degrees)
    diff < 0.42
}

/// Evaluates sensory contact of an enemy observing a target (`0x4a9aa0` `sight_check`).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_sensory(
    profile: &SensoryProfile,
    enemy_pos: Vec3,
    enemy_yaw: f32,
    mut alert_timer: f32,
    target: &PerceivableTarget,
    lighting: RoomLighting,
    difficulty: DifficultyLevel,
    dt: f32,
) -> (SensoryNotice, f32) {
    if !target.is_alive {
        return (SensoryNotice::None, 0.0);
    }

    let dx = target.position.x - enemy_pos.x;
    let dz = target.position.z - enemy_pos.z;
    let dist = (dx * dx + dz * dz).sqrt();

    let angle_to_target = dx.atan2(dz);
    let facing_diff = (normalize_angle(angle_to_target - enemy_yaw)).abs();

    let is_lit = is_lit_by_flashlight(
        enemy_pos,
        target.position,
        target.yaw,
        target.is_flashlight_on,
    );

    // Update alert accumulator
    if is_lit {
        alert_timer = (alert_timer + dt).min(3.0);
    } else {
        alert_timer = (alert_timer - dt).max(0.0);
    }

    // Determine sight cutoff distance based on lighting
    let sight_max = match lighting {
        RoomLighting::Lit => profile.sight_lit,
        RoomLighting::DarkLightOn => {
            if is_lit || target.is_flashlight_on {
                profile.sight_dark_light_on
            } else {
                profile.sight_dark_light_off
            }
        }
        RoomLighting::DarkLightOff => profile.sight_dark_light_off,
    };

    let diff_idx = match difficulty {
        DifficultyLevel::Beginner => 0,
        DifficultyLevel::Easy => 1,
        DifficultyLevel::Normal => 2,
        DifficultyLevel::Hard => 3,
    };

    let hear_range = profile.hearing_ranges[diff_idx] * if target.is_running { 1.5 } else { 1.0 };

    // Check attack reach first if facing
    if dist <= profile.attack_range && facing_diff <= profile.attack_half_angle {
        return (SensoryNotice::InAttackRange, alert_timer);
    }

    // Visual contact: within distance and inside sight cone (or alerted by flashlight)
    if dist <= sight_max && (facing_diff <= profile.view_half_angle || alert_timer > 1.0) {
        return (SensoryNotice::VisualContact, alert_timer);
    }

    // Auditory contact: omnidirectional hearing check
    if dist <= hear_range {
        return (SensoryNotice::AuditoryContact, alert_timer);
    }

    (SensoryNotice::None, alert_timer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flashlight_beam_detection() {
        let player_pos = Vec3::new(0.0, 0.0, 0.0);
        let player_yaw = 0.0; // Facing +Z
        let enemy_pos_ahead = Vec3::new(0.0, 0.0, 2000.0);
        let enemy_pos_behind = Vec3::new(0.0, 0.0, -2000.0);

        assert!(is_lit_by_flashlight(
            enemy_pos_ahead,
            player_pos,
            player_yaw,
            true
        ));
        assert!(!is_lit_by_flashlight(
            enemy_pos_ahead,
            player_pos,
            player_yaw,
            false
        ));
        assert!(!is_lit_by_flashlight(
            enemy_pos_behind,
            player_pos,
            player_yaw,
            true
        ));
    }

    #[test]
    fn test_sensory_scu_dark_vs_lit() {
        let profile = SensoryProfile::lying_figure();
        let enemy_pos = Vec3::new(0.0, 0.0, 0.0);
        let enemy_yaw = 0.0; // Facing +Z
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2500.0), PI, false, false);

        // In lit room: 2500 <= 5000 -> Visual contact
        let (notice_lit, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &target,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_lit, SensoryNotice::VisualContact);

        // In dark room without light: 2500 > 750 sight and 2500 > 1000 hear -> None
        let (notice_dark, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &target,
            RoomLighting::DarkLightOff,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_dark, SensoryNotice::None);
    }

    #[test]
    fn test_sensory_hearing_running() {
        let profile = SensoryProfile::mannequin();
        let enemy_pos = Vec3::new(0.0, 0.0, 0.0);
        let enemy_yaw = 0.0; // Facing +Z
                             // Target is behind the enemy (Z = -1200)
        let walking_target =
            PerceivableTarget::new(Vec3::new(0.0, 0.0, -1200.0), 0.0, false, false);
        let running_target = PerceivableTarget::new(Vec3::new(0.0, 0.0, -1200.0), 0.0, false, true);

        // Walking: Normal hear range is 1000 -> Out of hearing range
        let (notice_walk, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &walking_target,
            RoomLighting::DarkLightOff,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_walk, SensoryNotice::None);

        // Running: Hear range is 1000 * 1.5 = 1500 -> Auditory contact detected
        let (notice_run, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &running_target,
            RoomLighting::DarkLightOff,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_run, SensoryNotice::AuditoryContact);
    }
}
