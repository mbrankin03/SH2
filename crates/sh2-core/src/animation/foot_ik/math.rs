//! Pure 2-bone analytical IK math, matrix helpers, and surface-normal alignment (`SH2-85`).

use glam::{Mat4, Quat, Vec3};

use super::{LegChainSpec, MAX_ANKLE_TILT_RAD};

/// Converts a row-major `[[f32; 4]; 4]` matrix (used by `sh2-assets`) into `glam::Mat4`.
#[inline]
pub(super) fn mat4_from_row_major(m: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&m).transpose()
}

/// Converts a `glam::Mat4` back into row-major `[[f32; 4]; 4]`.
#[inline]
pub(super) fn mat4_to_row_major(m: Mat4) -> [[f32; 4]; 4] {
    m.transpose().to_cols_array_2d()
}

/// Extracts the translation vector from a row-major `[[f32; 4]; 4]` bone matrix.
#[inline]
pub(super) fn bone_translation(m: &[[f32; 4]; 4]) -> Vec3 {
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
pub(super) fn solve_single_leg(
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

    let target_ankle = Vec3::new(ankle_pos.x, target_ankle_y, ankle_pos.z);
    let to_target = target_ankle - hip_pos;
    let target_dist = to_target.length();
    if target_dist < 1e-3 {
        return;
    }

    let cur_axis = (ankle_pos - hip_pos).normalize_or_zero();
    let mut bend_normal = thigh_vec.cross(shin_vec).normalize_or_zero();
    if bend_normal.length_squared() < 1e-6 {
        bend_normal = Vec3::X;
    } else if bend_normal.dot(Vec3::X) < 0.0 {
        bend_normal = -bend_normal;
    }

    let (desired_hip_angle, _) = solve_two_bone_ik_angles(l1, l2, target_dist);
    let target_dir = to_target / target_dist;

    let tangent = bend_normal.cross(target_dir).normalize_or_zero();
    let desired_thigh_dir = if tangent.length_squared() > 1e-6 {
        let cand_a =
            (target_dir * desired_hip_angle.cos() + tangent * desired_hip_angle.sin()).normalize();
        let cand_b =
            (target_dir * desired_hip_angle.cos() - tangent * desired_hip_angle.sin()).normalize();
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
