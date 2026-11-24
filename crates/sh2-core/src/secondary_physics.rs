//! Procedural secondary bone spring-damper physics for cloth, skirts, coat tails, and soft bodies.
//!
//! # Physical Simulation Model
//! In Silent Hill 2, secondary attachments such as James' jacket coat tails (`hll_jms.mdl` / `hhh_jms.mdl`)
//! and Maria's skirt and chest (`lll_mar.mdl` / `hhh_mar.mdl`) react dynamically to linear
//! acceleration, velocity inertia, and rotational yaw angular velocity.
//!
//! Each bone node is modeled as a damped torsional spring:
//! $$I \ddot{\theta} + c \dot{\theta} + k \theta = \tau_{\text{inertial}}$$
//!
//! Where:
//! - $\theta = (\text{pitch}, \text{yaw}, \text{roll})$: Angular deflection from keyframed local pose.
//! - $k$: Spring stiffness restoring deflection back toward zero.
//! - $c$: Critical damping coefficient preventing oscillation.
//! - $\tau_{\text{inertial}}$: Driving inertial torque derived from character movement and turning rate.

use glam::{FloatExt, Mat4, Quat, Vec3};
use std::collections::HashMap;

/// Configuration parameters for a secondary physics spring-damper joint.
#[derive(Debug, Clone, Copy)]
pub struct SecondaryJointConfig {
    /// Torsional spring stiffness ($k \ge 0$).
    pub stiffness: f32,
    /// Damping ratio ($c \ge 0$).
    pub damping: f32,
    /// Linear movement inertia coupling factor.
    pub linear_coupling: f32,
    /// Angular turn rate inertia coupling factor.
    pub angular_coupling: f32,
    /// Minimum allowed pitch deflection in radians.
    pub min_pitch: f32,
    /// Maximum allowed pitch deflection in radians.
    pub max_pitch: f32,
    /// Maximum allowed roll/yaw deflection magnitude in radians.
    pub max_sway: f32,
}

impl Default for SecondaryJointConfig {
    fn default() -> Self {
        Self {
            stiffness: 120.0,
            damping: 18.0,
            linear_coupling: 0.0035,
            angular_coupling: 0.35,
            min_pitch: -0.28,
            max_pitch: 0.40,
            max_sway: 0.22,
        }
    }
}

impl SecondaryJointConfig {
    /// Specialized preset for James Sunderland's outer jacket coat tails and side flaps.
    pub const fn james_jacket() -> Self {
        Self {
            stiffness: 110.0,
            damping: 17.5,
            linear_coupling: 0.0040,
            angular_coupling: 0.38,
            min_pitch: -0.25,
            max_pitch: 0.35,
            max_sway: 0.20,
        }
    }

    /// Specialized preset for Maria's skirt cloth flap bones.
    pub const fn maria_skirt() -> Self {
        Self {
            stiffness: 95.0,
            damping: 15.0,
            linear_coupling: 0.0050,
            angular_coupling: 0.42,
            min_pitch: -0.32,
            max_pitch: 0.45,
            max_sway: 0.28,
        }
    }

    /// Specialized preset for subtle soft-body jiggle on Maria's chest/torso bones.
    pub const fn maria_chest() -> Self {
        Self {
            stiffness: 160.0,
            damping: 24.0,
            linear_coupling: 0.0020,
            angular_coupling: 0.18,
            min_pitch: -0.10,
            max_pitch: 0.12,
            max_sway: 0.08,
        }
    }
}

/// Dynamic simulation state of an individual secondary joint.
#[derive(Debug, Clone, Copy, Default)]
pub struct SecondaryJointState {
    /// Previous 60 Hz tick angular deflection $(\text{pitch}, \text{yaw}, \text{roll})$ in radians.
    pub prev_deflection: Vec3,
    /// Current 60 Hz tick angular deflection $(\text{pitch}, \text{yaw}, \text{roll})$ in radians.
    pub deflection: Vec3,
    /// Angular velocity $(\dot{\text{pitch}}, \dot{\text{yaw}}, \dot{\text{roll}})$ in radians/sec.
    pub angular_velocity: Vec3,
}

/// Analytical thigh-capsule collision rig for preventing cloth/skirt clipping (`SH2-85`).
///
/// Bone indices and radii are derived from retail `.mdl` bind-pose geometry (`hll_jms.mdl` /
/// `hhh_jms.mdl` and `lll_mar.mdl`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThighCapsuleSpec {
    /// Left hip/thigh bone index.
    pub left_hip: usize,
    /// Left knee bone index.
    pub left_knee: usize,
    /// Right hip/thigh bone index.
    pub right_hip: usize,
    /// Right knee bone index.
    pub right_knee: usize,
    /// Analytical thigh capsule radius in SH2 mm.
    pub radius_mm: f32,
    /// Cloth bone indices tested against both thigh capsules.
    pub cloth_bones: &'static [usize],
}

impl ThighCapsuleSpec {
    /// James Sunderland jacket coat-tail collision against thighs (`hll_jms.mdl` / `hhh_jms.mdl`,
    /// `4->6` left thigh, `5->7` right thigh, `70.0` mm core radius inside `85.93` mm thigh mesh
    /// and `74.7` mm bind-pose clearance on bone `23`).
    pub const JAMES: Self = Self {
        left_hip: 4,
        left_knee: 6,
        right_hip: 5,
        right_knee: 7,
        radius_mm: 70.0,
        cloth_bones: &[11, 12, 16, 17, 23, 24],
    };

    /// Maria skirt collision against thighs (`lll_mar.mdl`, `3->7` left thigh, `5->8` right thigh,
    /// `35.0` mm inner core radius inside `52.88` mm thigh mesh and `36.0` mm bind-pose clearance
    /// on skirt bone `14`).
    pub const MARIA: Self = Self {
        left_hip: 3,
        left_knee: 7,
        right_hip: 5,
        right_knee: 8,
        radius_mm: 35.0,
        cloth_bones: &[13, 14, 18, 19],
    };
}

/// Returns the closest point to `p` on the finite line segment `[a, b]`.
#[must_use]
pub fn closest_point_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 {
        return a;
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

/// Computes the outward push vector if `bone_pos` penetrates capsule `([a, b], radius)`.
#[must_use]
pub fn resolve_capsule_penetration(bone_pos: Vec3, a: Vec3, b: Vec3, radius: f32) -> Option<Vec3> {
    let closest = closest_point_on_segment(bone_pos, a, b);
    let delta = bone_pos - closest;
    let dist_sq = delta.length_squared();
    if dist_sq >= radius * radius {
        return None;
    }
    let dist = dist_sq.sqrt();
    let dir = if dist > 1e-4 { delta / dist } else { Vec3::Z };
    Some(dir * (radius - dist))
}

/// Multi-joint secondary physics solver managing real-time bone perturbations.
#[derive(Debug, Clone)]
pub struct SecondaryBonePhysics {
    /// Active joint configurations keyed by skeleton bone index.
    configs: HashMap<usize, SecondaryJointConfig>,
    /// Simulation state keyed by skeleton bone index.
    states: HashMap<usize, SecondaryJointState>,
    /// Optional analytical thigh-capsule collision specification.
    thigh_capsules: Option<ThighCapsuleSpec>,
    /// Previous character world position for velocity estimation.
    last_position: Vec3,
    /// Filtered character linear velocity in character-local coordinates.
    filtered_local_velocity: Vec3,
    /// Previous character yaw angle for turning rate estimation.
    last_yaw: f32,
    /// Filtered character turning yaw rate in radians/sec.
    filtered_yaw_rate: f32,
}

impl Default for SecondaryBonePhysics {
    fn default() -> Self {
        Self::new()
    }
}

impl SecondaryBonePhysics {
    /// Creates a new empty secondary physics solver.
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            states: HashMap::new(),
            thigh_capsules: None,
            last_position: Vec3::ZERO,
            filtered_local_velocity: Vec3::ZERO,
            last_yaw: 0.0,
            filtered_yaw_rate: 0.0,
        }
    }

    /// Configures standard presets for James Sunderland (`hll_jms.mdl` / `hhh_jms.mdl`).
    ///
    /// Tracks coat tail bones (11, 12, 16, 17, 23, 24) and jacket flaps (27, 28, 31, 32).
    pub fn for_james() -> Self {
        let mut sim = Self::new();
        let jacket = SecondaryJointConfig::james_jacket();

        // Jacket coat tails (roots 11, 12, mid 16, 17, lower tips 23, 24)
        for &bone in &[11, 12, 16, 17, 23, 24] {
            sim.register_bone(bone, jacket);
        }
        // Lower jacket side flaps (27, 28, 31, 32)
        for &bone in &[27, 28, 31, 32] {
            sim.register_bone(bone, jacket);
        }
        sim.thigh_capsules = Some(ThighCapsuleSpec::JAMES);

        sim
    }

    /// Configures standard presets for Maria (`lll_mar.mdl` / `hhh_mar.mdl`).
    ///
    /// Tracks skirt cloth bones (12, 17) and chest/torso bones (13, 14, 18, 19).
    pub fn for_maria() -> Self {
        let mut sim = Self::new();
        let skirt = SecondaryJointConfig::maria_skirt();
        let chest = SecondaryJointConfig::maria_chest();

        // Maria skirt cloth bones
        for &bone in &[12, 17] {
            sim.register_bone(bone, skirt);
        }
        // Maria chest/torso soft-body jiggle bones
        for &bone in &[13, 14, 18, 19] {
            sim.register_bone(bone, chest);
        }
        sim.thigh_capsules = Some(ThighCapsuleSpec::MARIA);

        sim
    }

    /// Registers a bone index with custom spring-damper joint settings.
    pub fn register_bone(&mut self, bone_index: usize, config: SecondaryJointConfig) {
        self.configs.insert(bone_index, config);
        self.states.entry(bone_index).or_default();
    }

    /// Advances the simulation clock by `dt` seconds given character world position and orientation.
    pub fn step(&mut self, dt: f32, current_position: Vec3, current_yaw: f32) {
        if dt <= 0.0001 {
            return;
        }

        // 1. Calculate world displacement and project into character-local coordinate space
        let world_delta = current_position - self.last_position;
        self.last_position = current_position;

        let raw_world_vel = world_delta / dt;
        let (s_yaw, c_yaw) = current_yaw.sin_cos();

        // Local X = Lateral, Local Z = Forward/Backward
        let local_vx = raw_world_vel.x * c_yaw - raw_world_vel.z * s_yaw;
        let local_vz = raw_world_vel.x * s_yaw + raw_world_vel.z * c_yaw;
        let target_local_vel = Vec3::new(local_vx, raw_world_vel.y, local_vz);

        // Exponential smoothing filter for velocity
        let vel_alpha = (dt * 15.0).clamp(0.0, 1.0);
        self.filtered_local_velocity = self
            .filtered_local_velocity
            .lerp(target_local_vel, vel_alpha);

        // 2. Calculate turning yaw rate
        let mut yaw_delta = (current_yaw - self.last_yaw) % std::f32::consts::TAU;
        if yaw_delta > std::f32::consts::PI {
            yaw_delta -= std::f32::consts::TAU;
        } else if yaw_delta < -std::f32::consts::PI {
            yaw_delta += std::f32::consts::TAU;
        }
        self.last_yaw = current_yaw;

        let raw_yaw_rate = yaw_delta / dt;
        let yaw_alpha = (dt * 18.0).clamp(0.0, 1.0);
        self.filtered_yaw_rate = self
            .filtered_yaw_rate
            .lerp(raw_yaw_rate.clamp(-8.0, 8.0), yaw_alpha);

        // 3. Integrate spring-damper dynamics for each registered bone
        for (&bone, config) in &self.configs {
            let state = self.states.entry(bone).or_default();
            state.prev_deflection = state.deflection;

            // Driving inertial forces:
            // Forward movement pitches flaps backward; turning rate causes lateral roll/sway
            let driving_pitch = self.filtered_local_velocity.z * config.linear_coupling;
            let driving_roll = self.filtered_yaw_rate * config.angular_coupling
                + self.filtered_local_velocity.x * config.linear_coupling;

            let target_torque = Vec3::new(driving_pitch, 0.0, driving_roll);

            // Hooke's Law spring acceleration: a = -k * x - c * v + torque
            let spring_force =
                -config.stiffness * state.deflection - config.damping * state.angular_velocity;
            let total_accel = spring_force + target_torque;

            // Semi-implicit Euler integration
            state.angular_velocity += total_accel * dt;
            state.deflection += state.angular_velocity * dt;

            // Clamp deflection to physical bounds
            state.deflection.x = state.deflection.x.clamp(config.min_pitch, config.max_pitch);
            state.deflection.z = state.deflection.z.clamp(-config.max_sway, config.max_sway);
            state.deflection.y = state.deflection.y.clamp(-config.max_sway, config.max_sway);
        }
    }

    /// Applies secondary deflection transforms to the local bind matrices prior to forward kinematics.
    pub fn apply_to_locals(&self, locals: &mut [[[f32; 4]; 4]]) {
        self.apply_to_locals_interpolated(locals, 1.0);
    }

    /// Applies sub-frame Slerp-interpolated secondary deflection transforms between the previous
    /// and current 60 Hz simulation ticks (`alpha` in `[0.0, 1.0]`).
    pub fn apply_to_locals_interpolated(&self, locals: &mut [[[f32; 4]; 4]], alpha: f32) {
        let t = alpha.clamp(0.0, 1.0);
        for (&bone, state) in &self.states {
            if bone >= locals.len() {
                continue;
            }

            if state.prev_deflection.length_squared() < 1e-6
                && state.deflection.length_squared() < 1e-6
            {
                continue;
            }

            // Convert previous and current Euler deflections (pitch X, yaw Y, roll Z) into unit quaternions
            // and interpolate along the shortest geodesic arc via Slerp.
            let q_prev = Quat::from_euler(
                glam::EulerRot::ZYX,
                state.prev_deflection.z,
                state.prev_deflection.y,
                state.prev_deflection.x,
            );
            let q_curr = Quat::from_euler(
                glam::EulerRot::ZYX,
                state.deflection.z,
                state.deflection.y,
                state.deflection.x,
            );
            let rot_delta = q_prev.slerp(q_curr, t);
            let rot_mat = Mat4::from_quat(rot_delta);

            let mut orig = Mat4::from_cols_array_2d(&locals[bone]);
            // Modulate upper-left 3x3 rotation: M_new = M_orig * R_delta
            orig *= rot_mat;
            locals[bone] = orig.to_cols_array_2d();
        }
    }

    /// Advances the simulation clock by `dt` seconds only when `enabled` is `true`
    /// (`GameOptions::enable_secondary_physics`, default `false` for 1:1 PS2 parity).
    ///
    /// When `enabled` is `false`, resets all spring deflections to resting zero so toggling
    /// the option off at runtime immediately restores the authentic PS2 keyframed pose.
    pub fn step_if_enabled(
        &mut self,
        enabled: bool,
        dt: f32,
        current_position: Vec3,
        current_yaw: f32,
    ) {
        if enabled {
            self.step(dt, current_position, current_yaw);
        } else {
            self.reset(current_position, current_yaw);
        }
    }

    /// Applies sub-frame Slerp-interpolated secondary deflection transforms only when `enabled` is `true`
    /// (`GameOptions::enable_secondary_physics`, default `false` for 1:1 PS2 parity).
    pub fn apply_to_locals_if_enabled(
        &self,
        enabled: bool,
        locals: &mut [[[f32; 4]; 4]],
        alpha: f32,
    ) {
        if enabled {
            self.apply_to_locals_interpolated(locals, alpha);
        }
    }

    /// Resets all bone deflections and velocities to resting state.
    pub fn reset(&mut self, current_position: Vec3, current_yaw: f32) {
        self.last_position = current_position;
        self.filtered_local_velocity = Vec3::ZERO;
        self.last_yaw = current_yaw;
        self.filtered_yaw_rate = 0.0;
        for state in self.states.values_mut() {
            *state = SecondaryJointState::default();
        }
    }

    /// Resolves analytical thigh-capsule collisions on evaluated model-space `pose_world`
    /// matrices (`SH2-85`) only when `enabled` is `true`.
    pub fn resolve_thigh_collisions_if_enabled(
        &self,
        enabled: bool,
        pose_world: &mut [[[f32; 4]; 4]],
        parents: &[u8],
    ) {
        if enabled {
            self.resolve_thigh_collisions_into(pose_world, parents);
        }
    }

    /// Pushes penetrating cloth/skirt bones outside the character's analytical thigh capsules.
    pub fn resolve_thigh_collisions_into(&self, pose_world: &mut [[[f32; 4]; 4]], parents: &[u8]) {
        let Some(spec) = self.thigh_capsules else {
            return;
        };
        let max_leg = spec
            .left_hip
            .max(spec.left_knee)
            .max(spec.right_hip)
            .max(spec.right_knee);
        if max_leg >= pose_world.len() || parents.len() < pose_world.len() {
            return;
        }
        let pos_of = |m: &[[f32; 4]; 4]| Vec3::new(m[0][3], m[1][3], m[2][3]);
        let l_hip = pos_of(&pose_world[spec.left_hip]);
        let l_knee = pos_of(&pose_world[spec.left_knee]);
        let r_hip = pos_of(&pose_world[spec.right_hip]);
        let r_knee = pos_of(&pose_world[spec.right_knee]);

        for &bone in spec.cloth_bones {
            if bone >= pose_world.len() {
                continue;
            }
            let mut bone_pos = pos_of(&pose_world[bone]);
            let mut total_push = Vec3::ZERO;
            for (a, b) in [(l_hip, l_knee), (r_hip, r_knee)] {
                if let Some(push) = resolve_capsule_penetration(bone_pos, a, b, spec.radius_mm) {
                    bone_pos += push;
                    total_push += push;
                }
            }
            if total_push.length_squared() > 1e-8 {
                pose_world[bone][0][3] += total_push.x;
                pose_world[bone][1][3] += total_push.y;
                pose_world[bone][2][3] += total_push.z;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secondary_physics_presets_and_step() {
        let mut sim_james = SecondaryBonePhysics::for_james();
        assert!(sim_james.configs.contains_key(&11)); // coat tail root left
        assert!(sim_james.configs.contains_key(&12)); // coat tail root right
        assert!(sim_james.configs.contains_key(&16)); // mid tail left
        assert!(sim_james.configs.contains_key(&17)); // mid tail right
        assert!(sim_james.configs.contains_key(&23)); // lower tip left
        assert!(sim_james.configs.contains_key(&24)); // lower tip right

        let sim_maria = SecondaryBonePhysics::for_maria();
        assert!(sim_maria.configs.contains_key(&12)); // skirt left
        assert!(sim_maria.configs.contains_key(&17)); // skirt right
        assert!(sim_maria.configs.contains_key(&13)); // chest
        assert!(sim_maria.configs.contains_key(&14)); // chest

        // Test forward motion reaction
        let dt = 1.0 / 60.0;
        let p0 = Vec3::new(0.0, 0.0, 0.0);
        let p1 = Vec3::new(0.0, 0.0, 25.0); // Moving forward (+Z)

        sim_james.step(dt, p0, 0.0);
        sim_james.step(dt, p1, 0.0);

        let st11 = sim_james.states.get(&11).unwrap();
        // Moving forward causes pitch deflection due to inertia
        assert!(st11.deflection.x != 0.0 || st11.angular_velocity.x != 0.0);

        // Test applying transforms to locals
        let mut locals = vec![[[0.0; 4]; 4]; 40];
        for loc in &mut locals {
            loc[0][0] = 1.0;
            loc[1][1] = 1.0;
            loc[2][2] = 1.0;
            loc[3][3] = 1.0;
        }

        sim_james.apply_to_locals(&mut locals);
        // Bone 11 should have been modified
        assert_ne!(locals[11][0][0], 0.0);
    }

    #[test]
    fn test_secondary_physics_native_upgrade_toggle_gate() {
        use crate::config::{AppConfig, EnhancedPreset, GameOptions};

        // Default config must disable synthetic secondary physics for 1:1 PS2 parity
        assert!(!GameOptions::default().enable_secondary_physics);
        let mut cfg = AppConfig::default();
        cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
        assert!(!cfg.game.enable_secondary_physics);
        cfg.apply_enhanced_preset(EnhancedPreset::NativeEnhanced);
        assert!(cfg.game.enable_secondary_physics);

        let mut sim = SecondaryBonePhysics::for_james();
        let dt = 1.0 / 60.0;
        sim.step_if_enabled(false, dt, Vec3::ZERO, 0.0);
        sim.step_if_enabled(false, dt, Vec3::new(0.0, 0.0, 50.0), 0.5);

        let mut locals = vec![
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            40
        ];
        let orig = locals.clone();
        sim.apply_to_locals_if_enabled(false, &mut locals, 1.0);
        assert_eq!(
            locals, orig,
            "Disabled secondary physics must leave bind pose matrices untouched"
        );
    }

    #[test]
    fn test_thigh_capsule_collision_james_and_maria() {
        let mut pose = vec![
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            36
        ];
        let parents = vec![255u8; 36];

        // Place James left thigh (4->6) at X=-52, Y=-472..-258, Z=-30
        pose[4][0][3] = -52.0;
        pose[4][1][3] = -472.0;
        pose[4][2][3] = -30.0;
        pose[6][0][3] = -44.0;
        pose[6][1][3] = -258.0;
        pose[6][2][3] = -30.0;
        // Place James coat tail bone 23 inside the left thigh capsule (only 10 mm away)
        pose[23][0][3] = -52.0;
        pose[23][1][3] = -360.0;
        pose[23][2][3] = -20.0;

        let sim_james = SecondaryBonePhysics::for_james();
        sim_james.resolve_thigh_collisions_if_enabled(true, &mut pose, &parents);
        let pushed_pos = Vec3::new(pose[23][0][3], pose[23][1][3], pose[23][2][3]);
        let closest = closest_point_on_segment(
            pushed_pos,
            Vec3::new(-52.0, -472.0, -30.0),
            Vec3::new(-44.0, -258.0, -30.0),
        );
        assert!(
            pushed_pos.distance(closest) >= ThighCapsuleSpec::JAMES.radius_mm - 1e-3,
            "Coat tail bone 23 must be pushed outside James thigh capsule"
        );

        // Place Maria left thigh (3->7) and penetrating skirt bone 13
        pose[3][0][3] = -39.0;
        pose[3][1][3] = -461.0;
        pose[3][2][3] = -10.0;
        pose[7][0][3] = -36.0;
        pose[7][1][3] = -286.0;
        pose[7][2][3] = -2.0;
        pose[13][0][3] = -39.0;
        pose[13][1][3] = -363.0;
        pose[13][2][3] = -1.0;

        let sim_maria = SecondaryBonePhysics::for_maria();
        let before_maria = pose[13];
        sim_maria.resolve_thigh_collisions_if_enabled(false, &mut pose, &parents);
        assert_eq!(
            pose[13], before_maria,
            "Disabled toggle must not move skirt"
        );

        sim_maria.resolve_thigh_collisions_if_enabled(true, &mut pose, &parents);
        let pushed_maria = Vec3::new(pose[13][0][3], pose[13][1][3], pose[13][2][3]);
        let closest_m = closest_point_on_segment(
            pushed_maria,
            Vec3::new(-39.0, -461.0, -10.0),
            Vec3::new(-36.0, -286.0, -2.0),
        );
        assert!(
            pushed_maria.distance(closest_m) >= ThighCapsuleSpec::MARIA.radius_mm - 1e-3,
            "Skirt bone 13 must be pushed outside Maria thigh capsule"
        );
    }
}
