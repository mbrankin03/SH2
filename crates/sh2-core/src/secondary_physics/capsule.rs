//! Analytical thigh-capsule collision for secondary cloth/skirt bones (`SH2-85`).

use glam::Vec3;

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

/// Pushes penetrating cloth/skirt bones outside the character's analytical thigh capsules.
pub(super) fn resolve_thigh_collisions(
    spec: ThighCapsuleSpec,
    pose_world: &mut [[[f32; 4]; 4]],
    parents: &[u8],
) {
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
