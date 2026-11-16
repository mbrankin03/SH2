//! Over-the-Shoulder (OTS) 3D orbit camera controller (`SH2-93`).
//!
//! Positions a damped third-person camera over James's or Maria's right (or toggleable left)
//! shoulder with right-stick, mouse, and gyro (`SH2-87`) pitch/yaw control, `.cld` spring-arm
//! collision, and look-coupled flashlight & ranged weapon aiming (`ready_weapon`).
//!
//! # Coordinate & Angle Units
//! - Positions/distances: Silent Hill 2 world units (`500.0` units = `1.0 m`, `+Y` downward).
//! - Angles: Radians (`yaw = 0` faces `+Z`, `+yaw` turns screen-right; `pitch < 0` looks upward `-Y`).
//! - Simulation rate: Deterministic `60 Hz` ticks (`dt = 1/60 s`).

use glam::{Mat3, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::camera::CameraSnapshot;
use crate::collision::CollisionWorld;
use crate::timing::lerp_angle_shortest;

use super::spring_arm::{
    solve_ots_spring_arm, sweep_ray_against_cld, OtsBoomQuery, SpringArmConfig, SpringArmState,
};

/// Shoulder alignment side for the Over-the-Shoulder 3D camera (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OtsShoulderSide {
    /// Camera sits over the character's right shoulder (character framed on the left).
    #[default]
    Right,
    /// Camera sits over the character's left shoulder (character framed on the right).
    Left,
}

impl OtsShoulderSide {
    /// User-facing label for the Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Right => "Right Shoulder",
            Self::Left => "Left Shoulder",
        }
    }

    /// Lateral sign multiplier (`+1.0` for right shoulder, `-1.0` for left shoulder).
    #[inline]
    pub const fn lateral_sign(self) -> f32 {
        match self {
            Self::Right => 1.0,
            Self::Left => -1.0,
        }
    }

    /// Toggles between right and left shoulder framing.
    #[inline]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
        }
    }

    /// Next cyclical shoulder option.
    #[inline]
    pub const fn next(self) -> Self {
        self.toggle()
    }

    /// Previous cyclical shoulder option.
    #[inline]
    pub const fn prev(self) -> Self {
        self.toggle()
    }
}

/// User preference for the subtle Over-the-Shoulder / First-Person aiming reticle (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReticleMode {
    /// Subtle convergence dot appears only while holding Ready Weapon (`R2`).
    #[default]
    AimOnly,
    /// Faint diegetic center dot remains visible during modern camera exploration and aiming.
    AlwaysSubtle,
    /// Laser-free / reticle-free framing (relying purely on flashlight beam convergence).
    Off,
}

impl ReticleMode {
    /// User-facing label for the Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::AimOnly => "Aim Only",
            Self::AlwaysSubtle => "Subtle Always",
            Self::Off => "Off (Diegetic Only)",
        }
    }

    /// Cycle forward through reticle modes.
    pub const fn next(self) -> Self {
        match self {
            Self::AimOnly => Self::AlwaysSubtle,
            Self::AlwaysSubtle => Self::Off,
            Self::Off => Self::AimOnly,
        }
    }

    /// Cycle backward through reticle modes.
    pub const fn prev(self) -> Self {
        match self {
            Self::AimOnly => Self::Off,
            Self::AlwaysSubtle => Self::AimOnly,
            Self::Off => Self::AlwaysSubtle,
        }
    }
}

/// Evaluated HUD reticle state emitted by the modern camera rig (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OtsReticleState {
    /// Reticle hidden (`ClassicPs2Rail`, `ReticleMode::Off`, or hip exploration in `AimOnly`).
    #[default]
    Hidden,
    /// Subtle diegetic dot with normalized opacity (`0.0..=1.0`) and bloom spread (`0.0..=1.0`).
    SubtleDot {
        /// Opacity alpha (`0.0..=1.0`).
        opacity: f32,
        /// Normalized spread radius (`0.0` = tight settled aim, `1.0` = moving).
        spread: f32,
    },
}

/// Combat aiming phase of the Over-the-Shoulder camera (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OtsAimPhase {
    /// Relaxed exploration framing.
    #[default]
    Exploration,
    /// Tightened weapon-ready aiming stance (`ready_weapon_pressed`).
    WeaponReadied,
}

/// Configuration parameters for the Over-the-Shoulder 3D camera (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtsCameraConfig {
    /// Vertical offset of the shoulder pivot from feet in SH2 world units (`-745.0` = ~1.49 m up).
    pub shoulder_height_y: f32,
    /// Horizontal offset from spine centerline to right/left shoulder in SH2 units (`140.0` = 28 cm).
    pub lateral_shoulder_offset: f32,
    /// Boom distance behind shoulder during exploration in SH2 units (`640.0` = 1.28 m).
    pub hip_boom_distance: f32,
    /// Boom distance behind shoulder while aiming (`ready_weapon`) in SH2 units (`430.0` = 0.86 m).
    pub aim_boom_distance: f32,
    /// Vertical field-of-view during exploration in radians (`~56 deg`).
    pub hip_fov_y_radians: f32,
    /// Vertical field-of-view while aiming in radians (`~43 deg`).
    pub aim_fov_y_radians: f32,
    /// Minimum pitch angle in radians (negative looks upward along `-Y`).
    pub min_pitch_radians: f32,
    /// Maximum pitch angle in radians (positive looks downward along `+Y`).
    pub max_pitch_radians: f32,
    /// Right-stick horizontal orbit speed in radians per second at 1.0x sensitivity.
    pub stick_yaw_speed_rad_per_sec: f32,
    /// Right-stick vertical pitch speed in radians per second at 1.0x sensitivity.
    pub stick_pitch_speed_rad_per_sec: f32,
    /// Exponential follow-damping rate (`0.0..=1.0`) for shoulder anchor tracking per 60 Hz tick.
    pub pivot_smoothing_rate: f32,
    /// Maximum raycast distance in SH2 units for flashlight/weapon aim convergence (`8500.0` = 17 m).
    pub max_convergence_distance: f32,
}

impl Default for OtsCameraConfig {
    fn default() -> Self {
        Self {
            shoulder_height_y: -745.0,
            lateral_shoulder_offset: 140.0,
            hip_boom_distance: 640.0,
            aim_boom_distance: 430.0,
            hip_fov_y_radians: 56.0_f32.to_radians(),
            aim_fov_y_radians: 43.0_f32.to_radians(),
            min_pitch_radians: -1.08,
            max_pitch_radians: 1.02,
            stick_yaw_speed_rad_per_sec: 2.65,
            stick_pitch_speed_rad_per_sec: 1.85,
            pivot_smoothing_rate: 0.32,
            max_convergence_distance: 8500.0,
        }
    }
}

///Builds a singularity-free unit orientation quaternion whose local `+Z` axis equals `dir_unit`
/// in SH2 `+Y`-down world space, compatible with [`crate::timing::FlashlightSweepState`].
#[inline]
pub fn beam_quat_from_direction(dir_unit: Vec3, fallback_yaw: f32) -> Quat {
    let d = dir_unit.normalize_or(Vec3::new(fallback_yaw.sin(), 0.0, fallback_yaw.cos()));
    let world_up = Vec3::new(0.0, -1.0, 0.0);
    let mut right = d.cross(world_up);
    if right.length_squared() < 1e-6 {
        let (sy, cy) = fallback_yaw.sin_cos();
        right = Vec3::new(cy, 0.0, -sy);
    } else {
        right = right.normalize();
    }
    let local_up = d.cross(right).normalize_or(Vec3::Y);
    Quat::from_mat3(&Mat3::from_cols(right, local_up, d)).normalize()
}

/// Stateful Over-the-Shoulder 3D camera rig (`SH2-93`).
#[derive(Debug, Clone)]
pub struct OtsCameraRig {
    /// Tunable OTS orbit & framing parameters.
    pub config: OtsCameraConfig,
    /// Active shoulder alignment (`Right` or `Left`).
    pub shoulder_side: OtsShoulderSide,
    /// Reticle visibility preference.
    pub reticle_mode: ReticleMode,
    /// Current combat aiming stance (`Exploration` vs `WeaponReadied`).
    pub aim_phase: OtsAimPhase,
    /// Orbit horizontal yaw angle in radians (`0.0` = `+Z`).
    pub orbit_yaw: f32,
    /// Orbit vertical pitch angle in radians (`< 0.0` looks up `-Y`, `> 0.0` looks down `+Y`).
    pub orbit_pitch: f32,
    /// Smoothed lateral shoulder blend factor (`-1.0` = full Left, `+1.0` = full Right).
    pub shoulder_blend: f32,
    /// Smoothed aiming zoom blend factor (`0.0` = Exploration, `1.0` = WeaponReadied).
    pub aim_blend: f32,
    /// Smoothed spine root anchor in world units.
    pub smoothed_spine_pos: Vec3,
    /// Spring-arm boom occlusion solver state.
    pub spring_arm: SpringArmState,
}

impl Default for OtsCameraRig {
    fn default() -> Self {
        Self::new(OtsCameraConfig::default())
    }
}

impl OtsCameraRig {
    /// Constructs a new Over-the-Shoulder camera rig with default right-shoulder framing.
    pub fn new(config: OtsCameraConfig) -> Self {
        let boom = config.hip_boom_distance;
        Self {
            config,
            shoulder_side: OtsShoulderSide::Right,
            reticle_mode: ReticleMode::AimOnly,
            aim_phase: OtsAimPhase::Exploration,
            orbit_yaw: 0.0,
            orbit_pitch: 0.06,
            shoulder_blend: 1.0,
            aim_blend: 0.0,
            smoothed_spine_pos: Vec3::new(0.0, config.shoulder_height_y, 0.0),
            spring_arm: SpringArmState::new(boom),
        }
    }

    /// Snaps the OTS rig immediately behind `(player_pos, player_yaw)` (used on room cuts & mode entry).
    pub fn reset_to_player(&mut self, player_pos: Vec3, player_yaw: f32) {
        self.orbit_yaw = player_yaw;
        self.orbit_pitch = 0.06;
        self.shoulder_blend = self.shoulder_side.lateral_sign();
        self.aim_blend = 0.0;
        self.aim_phase = OtsAimPhase::Exploration;
        self.smoothed_spine_pos = player_pos + Vec3::new(0.0, self.config.shoulder_height_y, 0.0);
        self.spring_arm.reset(self.config.hip_boom_distance);
    }

    /// Toggles the active shoulder (`Right <-> Left`).
    pub fn toggle_shoulder(&mut self) {
        self.shoulder_side = self.shoulder_side.toggle();
    }

    /// Unit forward look vector in `+Y`-down world space for current `(orbit_yaw, orbit_pitch)`.
    #[inline]
    pub fn view_forward(&self) -> Vec3 {
        let (sy, cy) = self.orbit_yaw.sin_cos();
        let (sp, cp) = self.orbit_pitch.sin_cos();
        Vec3::new(sy * cp, sp, cy * cp).normalize_or(Vec3::Z)
    }

    /// Steps the OTS orbit, `.cld` spring-arm boom, reticle, and convergence ray for one 60 Hz tick.
    ///
    /// Returns `(snapshot, aim_origin, aim_dir, reticle_state)`.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        linear_speed: f32,
        right_stick_x: f32,
        right_stick_y: f32,
        extra_yaw_delta_rad: f32,
        extra_pitch_delta_rad: f32,
        sensitivity: f32,
        ready_weapon: bool,
        quick_turn_pressed: bool,
        shoulder_swap_pressed: bool,
        collision: &CollisionWorld,
        spring_cfg: &SpringArmConfig,
    ) -> (CameraSnapshot, Vec3, Vec3, OtsReticleState) {
        let dt = 1.0 / 60.0;
        if shoulder_swap_pressed {
            self.toggle_shoulder();
        }
        if quick_turn_pressed {
            self.orbit_yaw =
                lerp_angle_shortest(self.orbit_yaw, self.orbit_yaw + std::f32::consts::PI, 1.0);
        }

        self.aim_phase = if ready_weapon {
            OtsAimPhase::WeaponReadied
        } else {
            OtsAimPhase::Exploration
        };
        let aim_scale = if ready_weapon { 0.65 } else { 1.0 };
        let sens = sensitivity.clamp(0.2, 4.0) * aim_scale;

        let yaw_step = right_stick_x * self.config.stick_yaw_speed_rad_per_sec * sens * dt
            + extra_yaw_delta_rad;
        let pitch_step = -right_stick_y * self.config.stick_pitch_speed_rad_per_sec * sens * dt
            + extra_pitch_delta_rad;

        self.orbit_yaw = (self.orbit_yaw + yaw_step).rem_euclid(std::f32::consts::TAU);
        if self.orbit_yaw > std::f32::consts::PI {
            self.orbit_yaw -= std::f32::consts::TAU;
        }
        self.orbit_pitch = (self.orbit_pitch + pitch_step)
            .clamp(self.config.min_pitch_radians, self.config.max_pitch_radians);

        // Gentle behind-the-back follow assist only when walking without manual look or aiming
        let manual_look = right_stick_x.abs() > 0.02
            || right_stick_y.abs() > 0.02
            || extra_yaw_delta_rad.abs() > 1e-4
            || extra_pitch_delta_rad.abs() > 1e-4;
        if !manual_look && !ready_weapon && linear_speed > 80.0 {
            let _ = player_yaw; // Keep free 3D orbit decoupled unless gently centering
            self.orbit_pitch += (0.06 - self.orbit_pitch) * 0.015;
        }

        let target_shoulder = self.shoulder_side.lateral_sign();
        self.shoulder_blend += (target_shoulder - self.shoulder_blend) * 0.22;

        let target_aim = if ready_weapon { 1.0 } else { 0.0 };
        self.aim_blend += (target_aim - self.aim_blend) * 0.24;

        let raw_spine = player_pos + Vec3::new(0.0, self.config.shoulder_height_y, 0.0);
        self.smoothed_spine_pos = self
            .smoothed_spine_pos
            .lerp(raw_spine, self.config.pivot_smoothing_rate.clamp(0.05, 1.0));

        let (sy, cy) = self.orbit_yaw.sin_cos();
        let right_horiz = Vec3::new(cy, 0.0, -sy);
        let lateral = self.config.lateral_shoulder_offset * self.shoulder_blend;
        let desired_pivot = self.smoothed_spine_pos + right_horiz * lateral;

        let forward = self.view_forward();
        let boom_dir = -forward;
        let desired_boom = self.config.hip_boom_distance
            + (self.config.aim_boom_distance - self.config.hip_boom_distance) * self.aim_blend;

        let boom_query = OtsBoomQuery {
            spine_anchor: self.smoothed_spine_pos,
            desired_shoulder_pivot: desired_pivot,
            boom_dir_unit: boom_dir,
            desired_boom_distance: desired_boom,
            player_floor_y: player_pos.y,
        };
        let (safe_pivot, eye) =
            solve_ots_spring_arm(&boom_query, collision, spring_cfg, &mut self.spring_arm);

        let fov_y = self.config.hip_fov_y_radians
            + (self.config.aim_fov_y_radians - self.config.hip_fov_y_radians) * self.aim_blend;
        let look_target = eye + forward * 1000.0;

        // Converge chest/weapon ray onto the world point under the OTS camera center ray
        let far_probe = eye + forward * self.config.max_convergence_distance;
        let (convergence_pt, _) = sweep_ray_against_cld(eye, far_probe, collision, 0.0, 0.0);
        let chest_origin = Vec3::new(
            player_pos.x + sy * 45.0,
            player_pos.y - 680.0,
            player_pos.z + cy * 45.0,
        );
        let aim_origin = safe_pivot.lerp(chest_origin, 0.5);
        let aim_dir = (convergence_pt - aim_origin).normalize_or(forward);

        let speed_norm = (linear_speed / 1750.0).clamp(0.0, 1.0);
        let reticle = match self.reticle_mode {
            ReticleMode::Off => OtsReticleState::Hidden,
            ReticleMode::AimOnly if self.aim_blend > 0.05 => OtsReticleState::SubtleDot {
                opacity: (self.aim_blend * 0.78).clamp(0.0, 1.0),
                spread: speed_norm * 0.5,
            },
            ReticleMode::AimOnly => OtsReticleState::Hidden,
            ReticleMode::AlwaysSubtle => OtsReticleState::SubtleDot {
                opacity: 0.28 + 0.50 * self.aim_blend,
                spread: speed_norm * (1.0 - 0.5 * self.aim_blend),
            },
        };

        let snapshot = CameraSnapshot {
            eye,
            target: look_target,
            up: Vec3::new(0.0, -1.0, 0.0),
            fov_y,
        };
        (snapshot, aim_origin, aim_dir, reticle)
    }
}
