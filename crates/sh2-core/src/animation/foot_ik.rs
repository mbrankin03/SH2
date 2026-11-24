//! Analytical 2-bone Foot IK on stairs and slopes (`SH2-85`).
//!
//! # Ground Truth & Binary References
//! - **`PlayerCheckFootLine` (`SLUS_202.28` `Chacter/m3_play.c` `0x001078f0`):**
//!   - Casts vertical floor probes from James's shin/knee bones (`0x0d = 13`, `0x0e = 14` in
//!     `hll_jms.mdl` / `hhh_jms.mdl`) using `clCheckHitEyesOnlyFloor` (`0x00138f10`) across
//!     `[foot_y - 250.0, foot_y + 750.0]` mm (`+Y` down).
//! - **`shGetJamesFootPos` (`SLUS_202.28` `Chacter/m3_play.c` `0x00110b60`):**
//!   - Reads James's ankle (`0x1a = 26`) and toe (`0x1e = 30`) world positions.
//! - **`MariaCheckFootLine` (`SLUS_202.28` `Chacter/m3_maria_sub.c` `0x002262f0`):**
//!   - Casts identical `[-250.0, +750.0]` mm `clCheckHitEyesOnlyFloor` probes from Maria's
//!     leg bones (`7`, `8` in `lll_mar.mdl`).
//! - **Native-Upgrade Gating (`AGENTS.md` Rule 3.4):**
//!   - Strictly gated behind `GameOptions::enable_foot_ik` (default `false` in `Ps2Authentic`
//!     and `Sh2ee`, `true` in `NativeEnhanced`) so 1:1 PS2 keyframed animations remain untouched
//!     when disabled.

use glam::{Mat4, Quat, Vec3};

use crate::collision::{CollisionController, CollisionWorld};

/// Upward floor-probe offset in SH2 world mm (`-250.0` along `+Y`-down,
/// `PlayerCheckFootLine` `0x001078f0` / `MariaCheckFootLine` `0x002262f0`).
pub const FOOT_PROBE_UP_MM: f32 = 250.0;

/// Downward floor-probe offset in SH2 world mm (`+750.0` along `+Y`-down,
/// `PlayerCheckFootLine` `0x001078f0` / `MariaCheckFootLine` `0x002262f0`).
pub const FOOT_PROBE_DOWN_MM: f32 = 750.0;

/// Maximum vertical foot adjustment in mm, matching `PlayerSetHeight` (`0x00107aa0`)
/// step/freefall threshold (`250.0` mm).
pub const MAX_FOOT_IK_STEP_MM: f32 = 250.0;

/// Maximum ankle surface-normal tilt (`pi / 4` = 45 degrees) to prevent hyper-flexion on walls.
pub const MAX_ANKLE_TILT_RAD: f32 = std::f32::consts::FRAC_PI_4;

/// Exponential smoothing slew rate (`20.0` s^-1, `0x41a00000`, matching
/// `PlayerCheckNeckAngle` `0x00108e10`).
pub const FOOT_IK_SMOOTH_SPEED: f32 = 20.0;

/// Bone index chain for one character leg in `.mdl` skeleton hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegChainSpec {
    /// Upper-leg hip/thigh bone index (`parent == lower_root`).
    pub hip: usize,
    /// Upper-knee bone index (`parent == hip`).
    pub knee: usize,
    /// Lower-knee/shin bone index (`parent == knee`, probed by `PlayerCheckFootLine`).
    pub shin: usize,
    /// Ankle joint bone index (`parent == shin`).
    pub ankle: usize,
    /// Toe/sole tip bone index (`parent == ankle`).
    pub toe: usize,
}

/// Complete two-leg IK rig specification for a character skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FootIkRigSpec {
    /// Upper-body root bone (`parent == 255`, `0` in `hll_jms.mdl` & `lll_mar.mdl`).
    pub upper_root: usize,
    /// Lower-body pelvis root bone (`parent == 255`, `1` in `hll_jms.mdl` & `lll_mar.mdl`).
    pub lower_root: usize,
    /// Left leg bone chain.
    pub left: LegChainSpec,
    /// Right leg bone chain.
    pub right: LegChainSpec,
}

impl FootIkRigSpec {
    /// James Sunderland rig verified from `hll_jms.mdl` (41 bones) & `hhh_jms.mdl` (71 bones),
    /// matching `PlayerCheckFootLine` (`0x001078f0`) and `shGetJamesFootPos` (`0x00110b60`).
    pub const JAMES: Self = Self {
        upper_root: 0,
        lower_root: 1,
        left: LegChainSpec {
            hip: 4,
            knee: 6,
            shin: 13,
            ankle: 25,
            toe: 29,
        },
        right: LegChainSpec {
            hip: 5,
            knee: 7,
            shin: 14,
            ankle: 26,
            toe: 30,
        },
    };

    /// Maria rig verified from `lll_mar.mdl` (36 bones), matching `MariaCheckFootLine` (`0x002262f0`).
    pub const MARIA: Self = Self {
        upper_root: 0,
        lower_root: 1,
        left: LegChainSpec {
            hip: 3,
            knee: 7,
            shin: 10,
            ankle: 20,
            toe: 24,
        },
        right: LegChainSpec {
            hip: 5,
            knee: 8,
            shin: 15,
            ankle: 22,
            toe: 26,
        },
    };

    /// Returns `true` if `parents` matches both left and right leg chains of this rig.
    #[must_use]
    pub fn matches_parents(&self, parents: &[u8]) -> bool {
        let chain_ok = |c: LegChainSpec| {
            let max_b = c.hip.max(c.knee).max(c.shin).max(c.ankle).max(c.toe);
            if max_b >= parents.len() {
                return false;
            }
            parents[c.hip] as usize == self.lower_root
                && parents[c.knee] as usize == c.hip
                && parents[c.shin] as usize == c.knee
                && parents[c.ankle] as usize == c.shin
                && parents[c.toe] as usize == c.ankle
        };
        self.upper_root < parents.len()
            && self.lower_root < parents.len()
            && chain_ok(self.left)
            && chain_ok(self.right)
    }
}

/// Output of a single floor probe at a foot's world XZ position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootGroundSample {
    /// Vertical offset relative to character `player_pos.y` (`+Y` down, clamped to `[-250, +250]` mm).
    pub delta_y_mm: f32,
    /// Upward-pointing surface normal in world space (`normal.y <= 0.0` in SH2 `+Y`-down coords).
    pub world_normal: Vec3,
}

/// Converts a row-major `[[f32; 4]; 4]` matrix (used by `sh2-assets`) into `glam::Mat4`.
#[inline]
fn mat4_from_row_major(m: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&m).transpose()
}

/// Converts a `glam::Mat4` back into row-major `[[f32; 4]; 4]`.
#[inline]
fn mat4_to_row_major(m: Mat4) -> [[f32; 4]; 4] {
    m.transpose().to_cols_array_2d()
}

/// Extracts the translation vector from a row-major `[[f32; 4]; 4]` bone matrix.
#[inline]
fn bone_translation(m: &[[f32; 4]; 4]) -> Vec3 {
    Vec3::new(m[0][3], m[1][3], m[2][3])
}

/// Transforms a model-space point into SH2 world space using `player_pos` and `player_yaw`.
#[inline]
pub fn model_to_world_pos(model_pos: Vec3, player_pos: Vec3, player_yaw: f32) -> Vec3 {
    let (s, c) = player_yaw.sin_cos();
    Vec3::new(
        model_pos.x * c + model_pos.z * s + player_pos.x,
        model_pos.y + player_pos.y,
        -model_pos.x * s + model_pos.z * c + player_pos.z,
    )
}

/// Transforms a world-space direction vector into model-local space (inverse `player_yaw`).
#[inline]
pub fn world_to_model_dir(world_dir: Vec3, player_yaw: f32) -> Vec3 {
    let (s, c) = player_yaw.sin_cos();
    Vec3::new(
        world_dir.x * c - world_dir.z * s,
        world_dir.y,
        world_dir.x * s + world_dir.z * c,
    )
}

/// Computes the analytical Law-of-Cosines `(hip_angle_rad, knee_bend_rad)` for a 2-bone leg
/// with thigh length `l1`, shin length `l2`, and hip-to-target distance `target_dist`.
///
/// `hip_angle_rad` is the interior angle between `hip->target` and `hip->knee` (`>= 0`).
/// `knee_bend_rad` is the flexion angle at the knee (`0` = straight, `> 0` = bent).
#[must_use]
pub fn solve_two_bone_ik_angles(l1: f32, l2: f32, target_dist: f32) -> (f32, f32) {
    let min_d = (l1 - l2).abs() + 1e-3;
    let max_d = (l1 + l2 - 1e-3).max(min_d);
    let d = target_dist.clamp(min_d, max_d);

    let cos_hip = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let cos_knee = ((l1 * l1 + l2 * l2 - d * d) / (2.0 * l1 * l2)).clamp(-1.0, 1.0);

    let hip_angle = cos_hip.acos();
    let knee_interior = cos_knee.acos();
    (hip_angle, (std::f32::consts::PI - knee_interior).max(0.0))
}

/// Probes the `.cld` floor directly beneath a foot's world XZ coordinate using
/// `clCheckHitEyesOnlyFloor` (`SLUS_202.28` `0x00138f10`) across
/// `[player_pos.y - 250.0, player_pos.y + 750.0]` (`PlayerCheckFootLine` `0x001078f0`).
#[must_use]
pub fn probe_foot_ground(
    world: &CollisionWorld,
    foot_world_xz: Vec3,
    player_pos_y: f32,
) -> Option<FootGroundSample> {
    let origin = Vec3::new(
        foot_world_xz.x,
        player_pos_y - FOOT_PROBE_UP_MM,
        foot_world_xz.z,
    );
    let target = Vec3::new(
        foot_world_xz.x,
        player_pos_y + FOOT_PROBE_DOWN_MM,
        foot_world_xz.z,
    );
    let hit = world.check_hit_eyes_only_floor(origin, target)?;
    let delta_y_mm = (hit.point.y - player_pos_y).clamp(-MAX_FOOT_IK_STEP_MM, MAX_FOOT_IK_STEP_MM);
    let mut normal = hit.normal.normalize_or_zero();
    if normal.length_squared() < 1e-6 {
        normal = Vec3::NEG_Y;
    } else if normal.y > 0.0 {
        normal = -normal;
    }
    Some(FootGroundSample {
        delta_y_mm,
        world_normal: normal,
    })
}

/// Returns `true` if `bone` is `root` or descends from `root` in `parents`.
fn is_bone_or_descendant(parents: &[u8], bone: usize, root: usize) -> bool {
    let mut cur = bone;
    for _ in 0..parents.len() {
        if cur == root {
            return true;
        }
        let p = parents[cur];
        if p == 255 || (p as usize) >= parents.len() || p as usize == cur {
            return false;
        }
        cur = p as usize;
    }
    false
}

/// Rotates `pivot_bone` and all its descendants around `pivot_pos` by `q` in model space.
fn rotate_subtree_around(
    pose_world: &mut [[[f32; 4]; 4]],
    parents: &[u8],
    pivot_bone: usize,
    pivot_pos: Vec3,
    q: Quat,
) {
    if q.is_near_identity() {
        return;
    }
    let q_mat = Mat4::from_quat(q);
    for (b, slot) in pose_world.iter_mut().enumerate() {
        if !is_bone_or_descendant(parents, b, pivot_bone) {
            continue;
        }
        let m = mat4_from_row_major(*slot);
        let pos = m.w_axis.truncate();
        let new_pos = pivot_pos + q * (pos - pivot_pos);
        let mut rotated = q_mat * m;
        rotated.w_axis = new_pos.extend(1.0);
        *slot = mat4_to_row_major(rotated);
    }
}

/// Solves analytical 2-bone leg IK + ankle normal alignment on a single leg in model space.
fn solve_single_leg(
    pose_world: &mut [[[f32; 4]; 4]],
    parents: &[u8],
    chain: LegChainSpec,
    target_ankle_y: f32,
    model_normal: Vec3,
) {
    let hip_pos = bone_translation(&pose_world[chain.hip]);
    let knee_pos = bone_translation(&pose_world[chain.knee]);
    let ankle_pos = bone_translation(&pose_world[chain.ankle]);

    let thigh_vec = knee_pos - hip_pos;
    let shin_vec = ankle_pos - knee_pos;
    let l1 = thigh_vec.length();
    let l2 = shin_vec.length();
    if l1 < 1e-3 || l2 < 1e-3 {
        return;
    }

    // Preserve animated horizontal XZ placement of the ankle while targeting the probed floor Y.
    let target_ankle = Vec3::new(ankle_pos.x, target_ankle_y, ankle_pos.z);
    let to_target = target_ankle - hip_pos;
    let target_dist = to_target.length();
    if target_dist < 1e-3 {
        return;
    }

    // Determine sagittal bend-plane normal from current FK pose (fallback to +X lateral axis).
    let cur_axis = (ankle_pos - hip_pos).normalize_or_zero();
    let mut bend_normal = thigh_vec.cross(shin_vec).normalize_or_zero();
    if bend_normal.length_squared() < 1e-6 {
        bend_normal = Vec3::X;
    } else if bend_normal.dot(Vec3::X) < 0.0 {
        bend_normal = -bend_normal;
    }

    let (desired_hip_angle, _) = solve_two_bone_ik_angles(l1, l2, target_dist);
    let target_dir = to_target / target_dist;

    // Desired knee direction lies in the plane perpendicular to `bend_normal` at `desired_hip_angle`
    // toward the anterior (-Z in SH2 / knee-forward) side.
    let tangent = bend_normal.cross(target_dir).normalize_or_zero();
    let desired_thigh_dir = if tangent.length_squared() > 1e-6 {
        let cand_a =
            (target_dir * desired_hip_angle.cos() + tangent * desired_hip_angle.sin()).normalize();
        let cand_b =
            (target_dir * desired_hip_angle.cos() - tangent * desired_hip_angle.sin()).normalize();
        // Pick the solution whose knee bends forward/matches the animated knee side.
        let knee_hint = (knee_pos - (hip_pos + cur_axis * (l1 * 0.5))).normalize_or_zero();
        if knee_hint.length_squared() > 1e-6 {
            if cand_a.dot(knee_hint) >= cand_b.dot(knee_hint) {
                cand_a
            } else {
                cand_b
            }
        } else if cand_a.z <= cand_b.z {
            cand_a
        } else {
            cand_b
        }
    } else {
        target_dir
    };

    // 1. Rotate hip so `hip -> knee` aligns with `desired_thigh_dir`.
    let cur_thigh_dir = thigh_vec / l1;
    let hip_rot = Quat::from_rotation_arc(cur_thigh_dir, desired_thigh_dir);
    rotate_subtree_around(pose_world, parents, chain.hip, hip_pos, hip_rot);

    // 2. Rotate knee so `knee -> ankle` aims at `target_ankle`.
    let new_knee_pos = bone_translation(&pose_world[chain.knee]);
    let new_ankle_pos = bone_translation(&pose_world[chain.ankle]);
    let cur_shin_dir = (new_ankle_pos - new_knee_pos).normalize_or_zero();
    let desired_shin_dir = (target_ankle - new_knee_pos).normalize_or_zero();
    if cur_shin_dir.length_squared() > 1e-6 && desired_shin_dir.length_squared() > 1e-6 {
        let knee_rot = Quat::from_rotation_arc(cur_shin_dir, desired_shin_dir);
        rotate_subtree_around(pose_world, parents, chain.knee, new_knee_pos, knee_rot);
    }

    // 3. Align ankle & toe with the probed floor surface normal (clamped to MAX_ANKLE_TILT_RAD).
    let align_rot = clamped_normal_alignment(model_normal, MAX_ANKLE_TILT_RAD);
    let final_ankle_pos = bone_translation(&pose_world[chain.ankle]);
    rotate_subtree_around(pose_world, parents, chain.ankle, final_ankle_pos, align_rot);
}

/// Computes the shortest-arc rotation from flat upward `Vec3::NEG_Y` to `model_normal`,
/// clamped to `max_angle_rad`.
#[must_use]
pub fn clamped_normal_alignment(model_normal: Vec3, max_angle_rad: f32) -> Quat {
    let n = model_normal.normalize_or_zero();
    if n.length_squared() < 1e-6 {
        return Quat::IDENTITY;
    }
    let n_up = if n.y > 0.0 { -n } else { n };
    let q = Quat::from_rotation_arc(Vec3::NEG_Y, n_up);
    let (axis, angle) = q.to_axis_angle();
    if angle > max_angle_rad && axis.length_squared() > 1e-6 {
        Quat::from_axis_angle(axis, max_angle_rad)
    } else {
        q
    }
}

/// Stateful analytical 2-bone Foot IK solver with exponential smoothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootIkSolver {
    /// Target character leg skeleton specification.
    pub rig: FootIkRigSpec,
    /// Smoothed vertical pelvis offset in mm (`>= 0.0` in `+Y` down).
    pub smoothed_pelvis_y: f32,
    /// Smoothed left foot target vertical delta in mm.
    pub smoothed_left_y: f32,
    /// Smoothed right foot target vertical delta in mm.
    pub smoothed_right_y: f32,
    /// Smoothed left floor normal in world space (`Vec3::NEG_Y` when flat).
    pub smoothed_left_normal: Vec3,
    /// Smoothed right floor normal in world space (`Vec3::NEG_Y` when flat).
    pub smoothed_right_normal: Vec3,
}

impl FootIkSolver {
    /// Creates a new Foot IK solver for the given [`FootIkRigSpec`].
    #[must_use]
    pub const fn new(rig: FootIkRigSpec) -> Self {
        Self {
            rig,
            smoothed_pelvis_y: 0.0,
            smoothed_left_y: 0.0,
            smoothed_right_y: 0.0,
            smoothed_left_normal: Vec3::NEG_Y,
            smoothed_right_normal: Vec3::NEG_Y,
        }
    }

    /// Creates a Foot IK solver configured for James Sunderland.
    #[must_use]
    pub const fn for_james() -> Self {
        Self::new(FootIkRigSpec::JAMES)
    }

    /// Creates a Foot IK solver configured for Maria.
    #[must_use]
    pub const fn for_maria() -> Self {
        Self::new(FootIkRigSpec::MARIA)
    }

    /// Resets smoothed pelvis and foot offsets to flat rest state.
    pub fn reset(&mut self) {
        self.smoothed_pelvis_y = 0.0;
        self.smoothed_left_y = 0.0;
        self.smoothed_right_y = 0.0;
        self.smoothed_left_normal = Vec3::NEG_Y;
        self.smoothed_right_normal = Vec3::NEG_Y;
    }

    /// Applies Foot IK to `pose_world` only when `enabled` is `true` (`GameOptions::enable_foot_ik`).
    /// When `false`, resets state and leaves `pose_world` untouched for 1:1 PS2 parity.
    #[allow(clippy::too_many_arguments)]
    pub fn solve_if_enabled(
        &mut self,
        enabled: bool,
        world: &CollisionWorld,
        player_pos: Vec3,
        player_yaw: f32,
        dt: f32,
        parents: &[u8],
        pose_world: &mut [[[f32; 4]; 4]],
    ) {
        if !enabled {
            self.reset();
            return;
        }
        self.solve_into_world(world, player_pos, player_yaw, dt, parents, pose_world);
    }

    /// Solves analytical 2-bone Foot IK against a [`CollisionController`].
    pub fn solve_into(
        &mut self,
        collision: &CollisionController,
        player_pos: Vec3,
        player_yaw: f32,
        dt: f32,
        parents: &[u8],
        pose_world: &mut [[[f32; 4]; 4]],
    ) {
        self.solve_into_world(
            &collision.world,
            player_pos,
            player_yaw,
            dt,
            parents,
            pose_world,
        );
    }

    /// Solves analytical 2-bone Foot IK against a [`CollisionWorld`].
    pub fn solve_into_world(
        &mut self,
        world: &CollisionWorld,
        player_pos: Vec3,
        player_yaw: f32,
        dt: f32,
        parents: &[u8],
        pose_world: &mut [[[f32; 4]; 4]],
    ) {
        if pose_world.len() < parents.len() || !self.rig.matches_parents(parents) {
            return;
        }

        let left_ankle_orig = bone_translation(&pose_world[self.rig.left.ankle]);
        let right_ankle_orig = bone_translation(&pose_world[self.rig.right.ankle]);

        let left_world = model_to_world_pos(left_ankle_orig, player_pos, player_yaw);
        let right_world = model_to_world_pos(right_ankle_orig, player_pos, player_yaw);

        let left_hit = probe_foot_ground(world, left_world, player_pos.y);
        let right_hit = probe_foot_ground(world, right_world, player_pos.y);

        let target_left_y = left_hit.map_or(0.0, |h| h.delta_y_mm);
        let target_right_y = right_hit.map_or(0.0, |h| h.delta_y_mm);
        let target_left_n = left_hit.map_or(Vec3::NEG_Y, |h| h.world_normal);
        let target_right_n = right_hit.map_or(Vec3::NEG_Y, |h| h.world_normal);

        // Pelvis drops by the lower foot's downward delta (`+Y` down -> max positive delta).
        let target_pelvis_y = target_left_y.max(target_right_y).max(0.0);

        let alpha = if dt > 0.0 {
            (dt * FOOT_IK_SMOOTH_SPEED).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.smoothed_pelvis_y += (target_pelvis_y - self.smoothed_pelvis_y) * alpha;
        self.smoothed_left_y += (target_left_y - self.smoothed_left_y) * alpha;
        self.smoothed_right_y += (target_right_y - self.smoothed_right_y) * alpha;
        self.smoothed_left_normal = self
            .smoothed_left_normal
            .lerp(target_left_n, alpha)
            .normalize_or_zero();
        self.smoothed_right_normal = self
            .smoothed_right_normal
            .lerp(target_right_n, alpha)
            .normalize_or_zero();

        if self.smoothed_pelvis_y.abs() > 1e-4 {
            for slot in pose_world.iter_mut() {
                slot[1][3] += self.smoothed_pelvis_y;
            }
        }

        let left_model_n = world_to_model_dir(self.smoothed_left_normal, player_yaw);
        let right_model_n = world_to_model_dir(self.smoothed_right_normal, player_yaw);

        solve_single_leg(
            pose_world,
            parents,
            self.rig.left,
            left_ankle_orig.y + self.smoothed_left_y,
            left_model_n,
        );
        solve_single_leg(
            pose_world,
            parents,
            self.rig.right,
            right_ankle_orig.y + self.smoothed_right_y,
            right_model_n,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::CollisionConfig;
    use sh2_assets::formats::cld::FloorTriangle;
    use std::path::Path;

    fn identity_pose_from_positions(positions: &[Vec3]) -> Vec<[[f32; 4]; 4]> {
        positions
            .iter()
            .map(|p| {
                [
                    [1.0, 0.0, 0.0, p.x],
                    [0.0, 1.0, 0.0, p.y],
                    [0.0, 0.0, 1.0, p.z],
                    [0.0, 0.0, 0.0, 1.0],
                ]
            })
            .collect()
    }

    fn james_test_skeleton() -> (Vec<u8>, Vec<[[f32; 4]; 4]>) {
        let mut parents = vec![255u8; 31];
        parents[4] = 1;
        parents[6] = 4;
        parents[13] = 6;
        parents[25] = 13;
        parents[29] = 25;
        parents[5] = 1;
        parents[7] = 5;
        parents[14] = 7;
        parents[26] = 14;
        parents[30] = 26;

        let mut positions = vec![Vec3::ZERO; 31];
        positions[0] = Vec3::new(0.0, -562.01, -21.35);
        positions[1] = Vec3::new(0.0, -562.01, -21.35);
        positions[4] = Vec3::new(-52.11, -472.44, -31.48);
        positions[6] = Vec3::new(-44.38, -257.55, -23.96);
        positions[13] = Vec3::new(-43.55, -235.95, -26.88);
        positions[25] = Vec3::new(-36.38, -52.81, -50.17);
        positions[29] = Vec3::new(-40.47, 0.31, -0.11);
        positions[5] = Vec3::new(52.11, -472.44, -31.48);
        positions[7] = Vec3::new(44.40, -257.60, -22.53);
        positions[14] = Vec3::new(43.57, -236.02, -25.52);
        positions[26] = Vec3::new(36.38, -52.81, -50.17);
        positions[30] = Vec3::new(40.47, 0.31, -0.11);
        (parents, identity_pose_from_positions(&positions))
    }

    #[test]
    fn test_two_bone_law_of_cosines_golden_vector() {
        // Right triangle: l1 = 300, l2 = 400, d = 500 -> knee interior = 90 deg, bend = 90 deg
        let (hip_a, knee_b) = solve_two_bone_ik_angles(300.0, 400.0, 500.0);
        assert!((knee_b - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        assert!((hip_a - (400.0_f32 / 500.0).asin()).abs() < 1e-4);
    }

    #[test]
    fn test_foot_ik_on_stair_step_and_slope() {
        let (parents, mut pose) = james_test_skeleton();
        let orig_left_ankle_y = bone_translation(&pose[25]).y;
        let orig_right_ankle_y = bone_translation(&pose[26]).y;

        // Build a world where Left foot (X = -36.38) stands on an upper stair (Y = -120.0)
        // and Right foot (X = +36.38) stands on a lower stair (Y = +80.0).
        let mut world = CollisionWorld::default();
        world.floors.push(FloorTriangle::new(
            [
                [-300.0, -120.0, -300.0],
                [-1.0, -120.0, -300.0],
                [-1.0, -120.0, 300.0],
            ],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [
                [-300.0, -120.0, -300.0],
                [-1.0, -120.0, 300.0],
                [-300.0, -120.0, 300.0],
            ],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [
                [1.0, 80.0, -300.0],
                [300.0, 80.0, -300.0],
                [300.0, 80.0, 300.0],
            ],
            0,
        ));
        world.floors.push(FloorTriangle::new(
            [
                [1.0, 80.0, -300.0],
                [300.0, 80.0, 300.0],
                [1.0, 80.0, 300.0],
            ],
            0,
        ));
        let ctrl = CollisionController::new(world, CollisionConfig::default());

        let mut solver = FootIkSolver::for_james();
        // dt = 0.0 applies instant convergence (alpha = 1.0)
        solver.solve_into(&ctrl, Vec3::ZERO, 0.0, 0.0, &parents, &mut pose);

        // Pelvis must drop by +80 mm to allow right foot to reach Y = +80
        assert!((solver.smoothed_pelvis_y - 80.0).abs() < 1e-3);
        let new_left_ankle_y = bone_translation(&pose[25]).y;
        let new_right_ankle_y = bone_translation(&pose[26]).y;
        assert!((new_left_ankle_y - (orig_left_ankle_y - 120.0)).abs() < 2.0);
        assert!((new_right_ankle_y - (orig_right_ankle_y + 80.0)).abs() < 2.0);
    }

    #[test]
    fn test_foot_ik_toggle_off_restores_vanilla_ps2() {
        let (parents, orig_pose) = james_test_skeleton();
        let mut pose = orig_pose.clone();
        let world = CollisionWorld::default();
        let mut solver = FootIkSolver::for_james();
        solver.smoothed_pelvis_y = 42.0;

        solver.solve_if_enabled(
            false,
            &world,
            Vec3::ZERO,
            0.0,
            1.0 / 60.0,
            &parents,
            &mut pose,
        );
        assert_eq!(pose, orig_pose);
        assert_eq!(solver.smoothed_pelvis_y, 0.0);
    }

    #[test]
    fn test_retail_james_and_maria_mdl_foot_ik_chains() {
        let root = Path::new("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
        if !root.exists() {
            return;
        }
        let jms_bytes = std::fs::read(root.join("data/chr/jms/hll_jms.mdl")).unwrap();
        let jms = sh2_assets::formats::mdls::parse_mdl(&jms_bytes).unwrap();
        assert!(FootIkRigSpec::JAMES.matches_parents(&jms.parents));

        let mar_bytes = std::fs::read(root.join("data/chr/mar/lll_mar.mdl")).unwrap();
        let mar = sh2_assets::formats::mdls::parse_mdl(&mar_bytes).unwrap();
        assert!(FootIkRigSpec::MARIA.matches_parents(&mar.parents));

        // Probe retail Apartment stairwell `ap10.cld` (or `er01.cld`) with FootIkSolver
        let cld_path = root.join("data/bg/er/er01.cld");
        if cld_path.exists() {
            let cld_bytes = std::fs::read(cld_path).unwrap();
            let cld = sh2_assets::formats::cld::parse_cld(&cld_bytes).unwrap();
            let world = CollisionWorld::from_cld(&cld);
            let mut pose = jms.initial_matrices.clone();
            let mut solver = FootIkSolver::for_james();
            solver.solve_into_world(&world, Vec3::ZERO, 0.0, 0.0, &jms.parents, &mut pose);
            assert!(pose[25][1][3].is_finite());
            assert!(pose[26][1][3].is_finite());
        }
    }
}
