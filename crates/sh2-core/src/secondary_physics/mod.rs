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

mod capsule;
#[cfg(test)]
mod tests;

pub use capsule::{closest_point_on_segment, resolve_capsule_penetration, ThighCapsuleSpec};

use glam::{FloatExt, Quat, Vec3};
use sh2_assets::formats::anm::{mat4_inverse, mat4_mul, quat_to_matrix};
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

        let world_delta = current_position - self.last_position;
        self.last_position = current_position;

        let raw_world_vel = world_delta / dt;
        let (s_yaw, c_yaw) = current_yaw.sin_cos();

        let local_vx = raw_world_vel.x * c_yaw - raw_world_vel.z * s_yaw;
        let local_vz = raw_world_vel.x * s_yaw + raw_world_vel.z * c_yaw;
        let target_local_vel = Vec3::new(local_vx, raw_world_vel.y, local_vz);

        let vel_alpha = (dt * 15.0).clamp(0.0, 1.0);
        self.filtered_local_velocity = self
            .filtered_local_velocity
            .lerp(target_local_vel, vel_alpha);

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

        for (&bone, config) in &self.configs {
            let state = self.states.entry(bone).or_default();
            state.prev_deflection = state.deflection;

            let driving_pitch = self.filtered_local_velocity.z * config.linear_coupling;
            let driving_roll = self.filtered_yaw_rate * config.angular_coupling
                + self.filtered_local_velocity.x * config.linear_coupling;

            let target_torque = Vec3::new(driving_pitch, 0.0, driving_roll);
            let spring_force =
                -config.stiffness * state.deflection - config.damping * state.angular_velocity;
            let total_accel = spring_force + target_torque;

            state.angular_velocity += total_accel * dt;
            state.deflection += state.angular_velocity * dt;

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
            let rot_3x3 = quat_to_matrix(rot_delta.to_array());
            compose_row_major_rot3(&mut locals[bone], rot_3x3);
        }
    }

    /// Applies sub-frame Slerp-interpolated secondary deflection transforms **after**
    /// `evaluate_pose_into` on evaluated row-major model-space `pose_world` matrices,
    /// propagating each bone's pivot rotation to its descendants regardless of bone index
    /// ordering without corrupting translations or being overwritten by keyframed rotations.
    pub fn apply_to_pose_interpolated(
        &self,
        pose_world: &mut [[[f32; 4]; 4]],
        parents: &[u8],
        alpha: f32,
    ) {
        let t = alpha.clamp(0.0, 1.0);
        let n = pose_world.len();
        for bone in 0..n {
            let Some(state) = self.states.get(&bone) else {
                continue;
            };
            if state.prev_deflection.length_squared() < 1e-6
                && state.deflection.length_squared() < 1e-6
            {
                continue;
            }

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
            let rot_3x3 = quat_to_matrix(rot_delta.to_array());

            let w_old = pose_world[bone];
            let mut w_new = w_old;
            compose_row_major_rot3(&mut w_new, rot_3x3);
            let pivot_xform = mat4_mul(w_new, mat4_inverse(w_old));
            pose_world[bone] = w_new;

            for (idx, slot) in pose_world.iter_mut().enumerate() {
                if idx != bone && is_descendant_of(idx, bone, parents) {
                    *slot = mat4_mul(pivot_xform, *slot);
                }
            }
        }
    }

    /// Applies sub-frame Slerp-interpolated secondary deflection transforms to evaluated
    /// `pose_world` matrices only when `enabled` is `true` (`GameOptions::enable_secondary_physics`).
    pub fn apply_to_pose_if_enabled(
        &self,
        enabled: bool,
        pose_world: &mut [[[f32; 4]; 4]],
        parents: &[u8],
        alpha: f32,
    ) {
        if enabled {
            self.apply_to_pose_interpolated(pose_world, parents, alpha);
        }
    }

    /// Advances the simulation clock by `dt` seconds only when `enabled` is `true`
    /// (`GameOptions::enable_secondary_physics`, default `false` for 1:1 PS2 parity).
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

    /// Applies sub-frame Slerp-interpolated secondary deflection transforms only when `enabled` is `true`.
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
        if let Some(spec) = self.thigh_capsules {
            capsule::resolve_thigh_collisions(spec, pose_world, parents);
        }
    }
}

#[inline]
fn compose_row_major_rot3(mat: &mut [[f32; 4]; 4], rot_3x3: [[f32; 3]; 3]) {
    let orig = *mat;
    for r in 0..3 {
        for c in 0..3 {
            mat[r][c] = orig[r][0] * rot_3x3[0][c]
                + orig[r][1] * rot_3x3[1][c]
                + orig[r][2] * rot_3x3[2][c];
        }
    }
}

#[inline]
fn is_descendant_of(mut idx: usize, ancestor: usize, parents: &[u8]) -> bool {
    let mut depth = 0usize;
    while idx < parents.len() && depth < parents.len() {
        let p = parents[idx];
        if p == 255 {
            return false;
        }
        let p_usize = p as usize;
        if p_usize == ancestor {
            return true;
        }
        idx = p_usize;
        depth += 1;
    }
    false
}
